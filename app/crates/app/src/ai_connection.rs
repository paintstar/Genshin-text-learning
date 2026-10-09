use crate::state::AppState;
use ai::AiProfileRow;
use shared::AppError;
use std::collections::BTreeMap;

const VERIFIED_KEY: &str = "ai.verified_profiles";

pub fn is_verified(state: &AppState, profile: &AiProfileRow) -> Result<bool, AppError> {
    state.store.with_read(|connection| {
        let saved: BTreeMap<String, String> =
            store::SettingsKvStore::get(connection, VERIFIED_KEY)?
                .and_then(|value| serde_json::from_str(&value).ok())
                .unwrap_or_default();
        Ok(profile
            .config_fingerprint
            .as_ref()
            .is_some_and(|fingerprint| saved.get(&profile.id.to_string()) == Some(fingerprint)))
    })
}

pub fn record(state: &AppState, id: i64, fingerprint: Option<&str>) -> Result<(), AppError> {
    state.store.with_write(|connection| {
        let mut saved: BTreeMap<String, String> =
            store::SettingsKvStore::get(connection, VERIFIED_KEY)?
                .and_then(|value| serde_json::from_str(&value).ok())
                .unwrap_or_default();
        match fingerprint {
            Some(fingerprint) => {
                saved.insert(id.to_string(), fingerprint.to_string());
            }
            None => {
                saved.remove(&id.to_string());
            }
        }
        let value =
            serde_json::to_string(&saved).map_err(|_| AppError::internal("测试结果保存失败"))?;
        store::SettingsKvStore::set(connection, VERIFIED_KEY, &value)
    })
}

pub async fn test_profile(
    state: &AppState,
    id: i64,
) -> Result<shared::dto::AiTestResult, AppError> {
    let profile = state
        .store
        .with_read(|connection| ai::AiProfileRegistry::list(connection))?
        .into_iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| AppError::invalid_param("助手配置不存在"))?;
    let result: Result<(), AppError> = async {
        let mut updated = profile.clone();
        if profile.channel == "cli" {
            let kind = profile
                .cli_kind
                .as_deref()
                .and_then(ai::cli::CliKind3::from_str)
                .ok_or_else(|| AppError::invalid_param("请选择 CLI 类型"))?;
            let command = profile
                .command_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .unwrap_or_else(|| kind.as_str());
            updated.cli_version = Some(state.ai_guard.cli_version(kind, command).await?);
            if kind == ai::cli::CliKind3::Opencode {
                let launch = state.ai_guard.prepare_launch(kind, command).await?;
                updated.model = state
                    .ai_guard
                    .resolve_opencode_model(command, &profile.model, &launch)
                    .await?;
            }
        }
        ai::connection::verify_connection(
            state.ai_client_raw.as_ref(),
            &updated,
            state.secret_for(&profile),
        )
        .await?;
        let current = state
            .store
            .with_read(|connection| ai::AiProfileRegistry::list(connection))?
            .into_iter()
            .find(|current| current.id == id)
            .ok_or_else(|| AppError::invalid_param("测试期间配置已删除"))?;
        if current.config_fingerprint != profile.config_fingerprint {
            return Err(AppError::invalid_param("测试期间配置已修改，请重新测试"));
        }
        state.store.with_write(|connection| {
            ai::AiProfileRegistry::save(connection, &updated, profile.config_fingerprint.as_deref())
        })?;
        let verified = state
            .store
            .with_read(|connection| ai::AiProfileRegistry::list(connection))?
            .into_iter()
            .find(|profile| profile.id == id)
            .and_then(|profile| profile.config_fingerprint)
            .ok_or_else(|| AppError::internal("助手配置读取失败"))?;
        record(state, id, Some(&verified))?;
        Ok(())
    }
    .await;
    match result {
        Ok(()) => Ok(shared::dto::AiTestResult {
            ok: true,
            message: "所选模型已返回完整回答，可以启用此配置".into(),
            raw_output: None,
        }),
        Err(error) => {
            record(state, id, None)?;
            Ok(shared::dto::AiTestResult {
                ok: false,
                message: error.message,
                raw_output: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::composition::{compose, ComposeArgs};
    use ai::client::StubClient;
    use ai::AiEvent;
    use std::sync::Arc;

    #[tokio::test]
    async fn verification_requires_a_real_reply_and_tracks_configuration_changes() {
        let temporary = tempfile::tempdir().unwrap();
        let state = compose(ComposeArgs {
            data_dir: temporary.path().to_path_buf(),
            dict_db_path: temporary.path().join("missing.db"),
            source_base_url: "stub://test".into(),
            fetch_interval_ms: 0,
            secret_vault: Some(Arc::new(store::InMemoryVault::default())),
            source_override: Some(Arc::new(kb::port::StubSource::default())),
        })
        .unwrap();
        let mut state = Arc::try_unwrap(state).ok().unwrap();
        let mut profile = AiProfileRow {
            id: 0,
            name: "测试助手".into(),
            channel: "http".into(),
            cli_kind: None,
            command_path: None,
            base_url: Some("http://example.test/v1".into()),
            api_key_ref: None,
            model: "test-model".into(),
            extra_json: None,
            cli_version: None,
            config_fingerprint: None,
            is_active: false,
        };
        let id = state
            .store
            .with_write(|connection| ai::AiProfileRegistry::save(connection, &profile, None))
            .unwrap();
        for events in [
            vec![],
            vec![AiEvent::Done {
                text: String::new(),
                cached: false,
            }],
            vec![AiEvent::Done {
                text: "缓存回答".into(),
                cached: true,
            }],
            vec![AiEvent::Failed(AppError::ai_channel("模型不存在"))],
        ] {
            state.ai_client_raw = Arc::new(StubClient::of(events));
            assert!(!test_profile(&state, id).await.unwrap().ok);
        }
        state.ai_client_raw = Arc::new(StubClient::of(vec![AiEvent::Done {
            text: "测试回答".into(),
            cached: false,
        }]));
        assert!(test_profile(&state, id).await.unwrap().ok);
        profile = state
            .store
            .with_read(|connection| ai::AiProfileRegistry::list(connection))
            .unwrap()
            .remove(0);
        assert!(is_verified(&state, &profile).unwrap());
        profile.model = "another-model".into();
        state
            .store
            .with_write(|connection| {
                ai::AiProfileRegistry::save(
                    connection,
                    &profile,
                    profile.config_fingerprint.as_deref(),
                )
            })
            .unwrap();
        let changed = state
            .store
            .with_read(|connection| ai::AiProfileRegistry::list(connection))
            .unwrap()
            .remove(0);
        assert!(!is_verified(&state, &changed).unwrap());
    }
}
