//! 离线剧情导入：先完整检查，随后按任务事务更新；个人学习数据不被替换。

use crate::state::AppState;
use kb::pack;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use shared::dto::{StoryImportReport, StoryResourceInfo};
use shared::{AppError, GameLang};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

// 压缩包与展开量分别受限，避免误选大文件或异常下载占满本地磁盘。
const MAX_DOWNLOAD_BYTES: u64 = 1024 * 1024 * 1024;

pub fn resource_info(state: &AppState) -> Result<Option<StoryResourceInfo>, AppError> {
    let raw = state
        .store
        .with_read(|c| store::SettingsKvStore::get(c, "resources.story_info"))?;
    raw.filter(|v| !v.is_empty())
        .map(|v| serde_json::from_str(&v).map_err(|_| AppError::integrity("本地剧情资源信息损坏")))
        .transpose()
}

fn io_error(_: impl std::fmt::Display) -> AppError {
    AppError::resource_missing("无法读写剧情资源文件，请检查文件和可用磁盘空间")
}

pub fn import_file(state: &Arc<AppState>, path: &Path) -> Result<StoryImportReport, AppError> {
    import_path(state, path, false)
}

pub fn import_bundled(
    state: &Arc<AppState>,
    path: &Path,
) -> Result<Option<StoryImportReport>, AppError> {
    let header = pack::PackReader::new(File::open(path).map_err(io_error)?).header()?;
    if resource_info(state)?.is_some_and(|info| info.created_at >= header.created_at) {
        return Ok(None);
    }
    import_path(state, path, true).map(Some)
}

fn import_path(
    state: &Arc<AppState>,
    path: &Path,
    only_newer: bool,
) -> Result<StoryImportReport, AppError> {
    let mut source = File::open(path).map_err(io_error)?;
    if source.metadata().map_err(io_error)?.len() > MAX_DOWNLOAD_BYTES {
        return Err(AppError::invalid_param("剧情资源包过大"));
    }
    // 使用自己的临时副本，两次读取之间原文件修改也不会混入不同版本。
    let mut staged = tempfile::NamedTempFile::new_in(state.store.data_dir()).map_err(io_error)?;
    std::io::copy(
        &mut Read::by_ref(&mut source).take(MAX_DOWNLOAD_BYTES + 1),
        &mut staged,
    )
    .map_err(io_error)?;
    if staged.as_file().metadata().map_err(io_error)?.len() > MAX_DOWNLOAD_BYTES {
        return Err(AppError::invalid_param("剧情资源包过大"));
    }
    staged
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(io_error)?;
    import_staged(state, staged.as_file_mut(), only_newer)
}

fn import_staged(
    state: &Arc<AppState>,
    file: &mut File,
    only_newer: bool,
) -> Result<StoryImportReport, AppError> {
    // 导入、批量下载、单任务刷新都按任务事务提交；导入之间使用独立互斥锁。
    let _guard = state
        .story_import
        .lock()
        .map_err(|_| AppError::internal("资源导入状态不可用"))?;
    if only_newer {
        let header = pack::PackReader::new(&mut *file).header()?;
        file.seek(SeekFrom::Start(0)).map_err(io_error)?;
        if let Some(resource) =
            resource_info(state)?.filter(|info| info.created_at >= header.created_at)
        {
            return Ok(StoryImportReport {
                resource,
                imported: 0,
                unchanged: header.quest_count,
                degraded: 0,
            });
        }
    }
    let inspection = pack::inspect(&mut *file)?;
    if inspection.header.fixture && !cfg!(debug_assertions) {
        return Err(AppError::integrity("这是开发测试资源，请使用正式剧情包"));
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    let mut reader = pack::PackReader::new(file);
    let header = reader.header()?;
    state.store.with_write(|c| {
        kb::IndexIngestor::ingest_index(c, GameLang::Jp, &inspection.jp_index)?;
        kb::IndexIngestor::ingest_index(c, GameLang::Chs, &inspection.chs_index)
    })?;
    let mut imported = 0;
    let mut unchanged = 0;
    while let Some(quest) = reader.next_quest()? {
        let (jp, chs) = quest.parse()?;
        let jp_hash = kb::hash::content_hash(&jp);
        let chs_hash = kb::hash::content_hash(&chs);
        let same = state.store.with_read(|c| {
            Ok(kb::query::raw_meta(c, quest.quest_id, GameLang::Jp)?
                .is_some_and(|(_, h, _)| h == jp_hash)
                && kb::query::raw_meta(c, quest.quest_id, GameLang::Chs)?
                    .is_some_and(|(_, h, _)| h == chs_hash))
        })?;
        if same {
            crate::services::revalidate_provenance(state, quest.quest_id)?;
            unchanged += 1;
            continue;
        }
        state.store.with_write(|c| {
            kb::QuestIngestor::ingest_detail(
                c,
                quest.quest_id,
                (
                    &jp,
                    &kb::ingest::RawArchive {
                        bytes: quest.jp.get().as_bytes().to_vec(),
                        validator: None,
                    },
                ),
                (
                    &chs,
                    &kb::ingest::RawArchive {
                        bytes: quest.chs.get().as_bytes().to_vec(),
                        validator: None,
                    },
                ),
            )
        })?;
        crate::services::revalidate_provenance(state, quest.quest_id)?;
        imported += 1;
    }
    let resource = StoryResourceInfo {
        data_version: header.data_version,
        quest_count: header.quest_count,
        created_at: header.created_at,
        imported_at: store::now_secs(),
    };
    let json =
        serde_json::to_string(&resource).map_err(|_| AppError::internal("资源信息保存失败"))?;
    state
        .store
        .with_write(|c| store::SettingsKvStore::set(c, "resources.story_info", &json))?;
    Ok(StoryImportReport {
        resource,
        imported,
        unchanged,
        degraded: inspection.degraded,
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseManifest {
    format_version: u32,
    data_version: String,
    pack_url: String,
    sha256: String,
    size: u64,
}

/// 清单下载地址由用户或维护端配置，私有仓库凭据不进入应用。
pub async fn update(state: &Arc<AppState>) -> Result<StoryImportReport, AppError> {
    let json = state
        .store
        .with_read(|c| store::SettingsKvStore::get(c, "resources.update_urls"))?
        .unwrap_or_else(|| "[]".into());
    let urls: Vec<String> =
        serde_json::from_str(&json).map_err(|_| AppError::invalid_param("资源更新地址格式错误"))?;
    if urls.is_empty() {
        return Err(AppError::resource_missing(
            "尚未配置资源更新地址，可以先导入离线剧情包",
        ));
    }
    let client = reqwest::Client::builder()
        .user_agent(fetcher::USER_AGENT)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() >= 10
                || url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|_| AppError::network("资源下载客户端初始化失败"))?;
    let mut last_error = AppError::network("资源更新源暂时不可用，已有剧情仍可阅读");
    for url in urls {
        match download(&client, state, &url).await {
            Ok(None) => {
                let resource = resource_info(state)?
                    .ok_or_else(|| AppError::internal("本地资源信息不可用"))?;
                return Ok(StoryImportReport {
                    unchanged: resource.quest_count,
                    resource,
                    imported: 0,
                    degraded: 0,
                });
            }
            Ok(Some((mut staged, manifest))) => {
                let st = state.clone();
                let report = tokio::task::spawn_blocking(move || {
                    // 比对头部与下载清单，避免入口切换时混入其他版本。
                    let header = pack::PackReader::new(staged.as_file_mut()).header()?;
                    if header.data_version != manifest.data_version {
                        return Err(AppError::integrity("资源包与发布版本不一致"));
                    }
                    staged
                        .as_file_mut()
                        .seek(SeekFrom::Start(0))
                        .map_err(io_error)?;
                    import_staged(&st, staged.as_file_mut(), false)
                })
                .await
                .map_err(|_| AppError::internal("资源导入任务中断"))?;
                match report {
                    Ok(report) => return Ok(report),
                    Err(error) => last_error = error,
                }
            }
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

fn https_url(raw: &str) -> Result<reqwest::Url, AppError> {
    let url = reqwest::Url::parse(raw).map_err(|_| AppError::invalid_param("资源更新地址无效"))?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::invalid_param(
            "资源更新地址必须使用 HTTPS，且不能包含凭据",
        ));
    }
    Ok(url)
}

async fn download(
    client: &reqwest::Client,
    state: &AppState,
    url: &str,
) -> Result<Option<(tempfile::NamedTempFile, ReleaseManifest)>, AppError> {
    let mut response = client
        .get(https_url(url)?)
        .send()
        .await
        .map_err(|_| AppError::network("无法连接资源更新源，已有剧情仍可阅读"))?;
    if !response.status().is_success() {
        return Err(AppError::data_source("资源更新清单暂时不可用"));
    }
    // 清单只含版本和下载信息，应当很小。
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::network("资源清单下载中断"))?
    {
        if bytes.len() + chunk.len() > 64 * 1024 {
            return Err(AppError::integrity("资源清单过大"));
        }
        bytes.extend_from_slice(&chunk);
    }
    let manifest: ReleaseManifest =
        serde_json::from_slice(&bytes).map_err(|_| AppError::integrity("资源清单格式错误"))?;
    if manifest.format_version != pack::FORMAT_VERSION
        || manifest.size == 0
        || manifest.size > MAX_DOWNLOAD_BYTES
        || manifest.sha256.len() != 64
        || !manifest.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(AppError::integrity("资源清单不兼容或文件信息无效"));
    }
    if resource_info(state)?.is_some_and(|info| info.data_version == manifest.data_version) {
        return Ok(None);
    }
    let mut response = client
        .get(https_url(&manifest.pack_url)?)
        .send()
        .await
        .map_err(|_| AppError::network("剧情资源下载失败，已有剧情仍可阅读"))?;
    if !response.status().is_success() {
        return Err(AppError::data_source("剧情资源包暂时不可用"));
    }
    let mut staged = tempfile::NamedTempFile::new_in(state.store.data_dir()).map_err(io_error)?;
    let mut size = 0u64;
    let mut hash = Sha256::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::network("剧情资源下载中断"))?
    {
        size += chunk.len() as u64;
        if size > manifest.size {
            return Err(AppError::integrity("剧情资源文件大小不一致"));
        }
        hash.update(&chunk);
        staged.write_all(&chunk).map_err(io_error)?;
    }
    if size != manifest.size || format!("{:x}", hash.finalize()) != manifest.sha256.to_lowercase() {
        return Err(AppError::integrity("剧情资源包下载不完整或校验失败"));
    }
    staged
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(io_error)?;
    Ok(Some((staged, manifest)))
}
