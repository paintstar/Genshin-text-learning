use ai::cli::{CliAdapter, CliKind3};
use ai::{AiClient, AiEvent, AiProfileRow, AiRequest, CliIsolationGuard};
use std::sync::Arc;

fn profile(command: String, kind: &str, model: &str) -> AiProfileRow {
    AiProfileRow {
        id: 1,
        name: "测试".into(),
        channel: "cli".into(),
        cli_kind: Some(kind.into()),
        command_path: Some(command),
        base_url: None,
        api_key_ref: None,
        model: model.into(),
        extra_json: None,
        cli_version: None,
        config_fingerprint: None,
        is_active: false,
    }
}

#[cfg(unix)]
#[tokio::test]
async fn cli_rejects_empty_incomplete_and_failed_processes() {
    use std::os::unix::fs::PermissionsExt;
    let temporary = tempfile::tempdir().unwrap();
    let command = temporary.path().join("test-cli");
    let adapter = CliAdapter {
        guard: Arc::new(CliIsolationGuard),
    };
    for (kind, events, exit, success) in [
        ("claude", "", 0, false),
        ("claude", "plain output", 0, false),
        ("claude", r#"{"type":"result","result":"回答"}"#, 1, false),
        (
            "codex",
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"回答"}}"#,
            0,
            false,
        ),
        (
            "claude",
            r#"{"type":"result","result":"回答","subtype":"success"}"#,
            0,
            true,
        ),
    ] {
        std::fs::write(
            &command,
            format!("#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{events}'\nexit {exit}\n"),
        )
        .unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
        let result = ai::connection::verify_connection(
            &adapter,
            &profile(command.display().to_string(), kind, "test-model"),
            None,
        )
        .await;
        assert_eq!(result.is_ok(), success, "{kind}: {events}, exit={exit}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn timeout_stops_the_owned_cli_process() {
    use std::os::unix::fs::PermissionsExt;
    let temporary = tempfile::tempdir().unwrap();
    let command = temporary.path().join("test-cli");
    std::fs::write(&command, "#!/bin/sh\ncat >/dev/null\nexec sleep 10\n").unwrap();
    std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
    let adapter = CliAdapter {
        guard: Arc::new(CliIsolationGuard),
    };
    let start = std::time::Instant::now();
    let mut events = adapter
        .ask(
            &profile(command.display().to_string(), "claude", "test-model"),
            None,
            AiRequest {
                system: String::new(),
                user: "测试".into(),
                feature: "test".into(),
                prompt_tpl_version: "1".into(),
                timeout_secs: 1,
                max_tokens: None,
            },
        )
        .await
        .unwrap();
    assert!(matches!(events.recv().await, Some(AiEvent::Failed(_))));
    assert!(start.elapsed() < std::time::Duration::from_secs(4));
}

#[tokio::test]
#[ignore = "需本机 OpenCode 与已配置的模型服务，会调用一次模型"]
async fn actual_opencode_connection_and_invalid_model() {
    let command = std::env::var("GLL_REAL_OPENCODE").expect("需设置 GLL_REAL_OPENCODE");
    let model = std::env::var("GLL_REAL_MODEL").unwrap_or_default();
    let guard = Arc::new(CliIsolationGuard);
    let launch = guard
        .prepare_launch(CliKind3::Opencode, &command)
        .await
        .unwrap();
    let resolved = guard
        .resolve_opencode_model(&command, &model, &launch)
        .await
        .unwrap();
    let adapter = CliAdapter { guard };
    ai::connection::verify_connection(
        &adapter,
        &profile(command.clone(), "opencode", &resolved),
        None,
    )
    .await
    .unwrap();
    println!("真实 OpenCode 连接测试通过");
    let provider = resolved.split_once('/').unwrap().0;
    assert!(ai::connection::verify_connection(
        &adapter,
        &profile(
            command,
            "opencode",
            &format!("{provider}/gll-nonexistent-model")
        ),
        None
    )
    .await
    .is_err());
    println!("不存在的模型正确返回失败");
}
