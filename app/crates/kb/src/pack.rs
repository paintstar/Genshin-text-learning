//! 剧情资源包：gzip 压缩的 JSON Lines，首行为目录，之后每行是一个双语任务。
//! 流式读取把内存占用限制在单个任务；不解压文件路径，也不包含用户数据库。

use crate::parser::{parse_detail, parse_index, ParsedDetail, QuestIndexEntry};
use flate2::read::MultiGzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use shared::AppError;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};

pub const FORMAT_VERSION: u32 = 1;
// 资源只包含文本。限制异常单条与总展开量，防止损坏包耗尽内存或磁盘。
const MAX_RECORD_BYTES: u64 = 64 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackHeader {
    pub format_version: u32,
    pub data_version: String,
    pub created_at: i64,
    pub quest_count: usize,
    #[serde(default)]
    pub fixture: bool,
    pub index: PackIndex,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackIndex {
    pub jp: Box<RawValue>,
    pub chs: Box<RawValue>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackQuest {
    pub quest_id: i64,
    pub jp: Box<RawValue>,
    pub chs: Box<RawValue>,
    pub jp_sha256: String,
    pub chs_sha256: String,
}

pub struct PackReader<R: Read> {
    reader: BufReader<MultiGzDecoder<R>>,
    unpacked: u64,
}

impl<R: Read> PackReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(MultiGzDecoder::new(reader)),
            unpacked: 0,
        }
    }

    fn line<T: serde::de::DeserializeOwned>(&mut self) -> Result<Option<T>, AppError> {
        let mut bytes = Vec::new();
        let count = (&mut self.reader)
            .take(MAX_RECORD_BYTES + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|_| AppError::integrity("剧情资源包不完整或压缩数据损坏"))?;
        if count == 0 {
            return Ok(None);
        }
        self.unpacked += count as u64;
        if count as u64 > MAX_RECORD_BYTES || self.unpacked > MAX_UNPACKED_BYTES {
            return Err(AppError::integrity("剧情资源包超过允许的文本大小"));
        }
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| AppError::integrity("剧情资源包记录格式错误"))
    }

    pub fn header(&mut self) -> Result<PackHeader, AppError> {
        let header: PackHeader = self
            .line()?
            .ok_or_else(|| AppError::integrity("剧情资源包为空"))?;
        if header.format_version != FORMAT_VERSION {
            return Err(AppError::integrity("剧情资源格式不兼容，请更新应用"));
        }
        if header.data_version.trim().is_empty()
            || header.data_version.len() > 128
            || header.quest_count == 0
            || header.quest_count > 100_000
        {
            return Err(AppError::integrity("剧情资源版本或任务数量无效"));
        }
        Ok(header)
    }

    pub fn next_quest(&mut self) -> Result<Option<PackQuest>, AppError> {
        self.line()
    }
}

impl PackQuest {
    pub fn parse(&self) -> Result<(ParsedDetail, ParsedDetail), AppError> {
        for (raw, expected) in [(&self.jp, &self.jp_sha256), (&self.chs, &self.chs_sha256)] {
            if format!("{:x}", Sha256::digest(raw.get().as_bytes())) != *expected {
                return Err(AppError::integrity(format!(
                    "任务 {} 的剧情文件校验失败",
                    self.quest_id
                )));
            }
            let value: serde_json::Value = serde_json::from_str(raw.get())
                .map_err(|_| AppError::integrity("剧情 JSON 无效"))?;
            if let Some(id) = value
                .get("data")
                .and_then(|d| d.get("id"))
                .and_then(|v| v.as_i64())
            {
                if id != self.quest_id {
                    return Err(AppError::integrity("剧情内容与任务编号不一致"));
                }
            }
        }
        Ok((
            parse_detail(self.jp.get().as_bytes())?,
            parse_detail(self.chs.get().as_bytes())?,
        ))
    }
}

pub struct PackInspection {
    pub header: PackHeader,
    pub jp_index: Vec<QuestIndexEntry>,
    pub chs_index: Vec<QuestIndexEntry>,
    pub degraded: usize,
}

/// 完整检查必须在任何写入前执行，包括最后一条记录和 gzip 尾部校验。
pub fn inspect(reader: impl Read) -> Result<PackInspection, AppError> {
    let mut reader = PackReader::new(reader);
    let header = reader.header()?;
    let jp_index = parse_index(header.index.jp.get().as_bytes())?;
    let chs_index = parse_index(header.index.chs.get().as_bytes())?;
    let index_ids = |entries: &[QuestIndexEntry]| -> Result<HashSet<i64>, AppError> {
        let ids: HashSet<_> = entries.iter().map(|e| e.quest_id).collect();
        if ids.len() != entries.len()
            || ids.len() != header.quest_count
            || entries.iter().any(|e| {
                e.quest_id <= 0
                    || e.chapter_title
                        .as_deref()
                        .is_none_or(|t| t.trim().is_empty() || t.contains("$UNRELEASED"))
            })
        {
            return Err(AppError::integrity("资源目录包含重复、缺失或未发布任务"));
        }
        Ok(ids)
    };
    let ids = index_ids(&jp_index)?;
    if ids != index_ids(&chs_index)? {
        return Err(AppError::integrity("中日资源目录不一致"));
    }
    let mut seen = HashSet::new();
    let mut degraded = 0;
    while let Some(quest) = reader.next_quest()? {
        if !ids.contains(&quest.quest_id) || !seen.insert(quest.quest_id) {
            return Err(AppError::integrity("剧情正文包含目录外或重复任务"));
        }
        let (jp, chs) = quest.parse()?;
        let classification = crate::AlignClassifier::classify(&jp, &chs, "jp", "chs");
        if !crate::AlignClassifier::is_ok(&classification) {
            degraded += 1;
        }
    }
    if seen != ids {
        return Err(AppError::integrity("剧情资源包正文缺失"));
    }
    Ok(PackInspection {
        header,
        jp_index,
        chs_index,
        degraded,
    })
}
