//! ChannelDispatch — 按 profile 通道路由的 AiClient 实现。
//!
//! 组合根把 CLI 适配器与 HTTP 适配器都注册进来，真实通道 = 本分发器；
//! AiCache 装饰器包裹在本实现外侧（对全部调用方透明）。

use crate::client::{AiClient, AiEvent, AiRequest};
use crate::profile::AiProfileRow;
use async_trait::async_trait;
use shared::AppError;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct ChannelDispatch {
    pub cli: Arc<dyn AiClient>,
    pub http: Arc<dyn AiClient>,
}

#[async_trait]
impl AiClient for ChannelDispatch {
    async fn ask(
        &self,
        profile: &AiProfileRow,
        secret: Option<String>,
        req: AiRequest,
    ) -> Result<mpsc::Receiver<AiEvent>, AppError> {
        match profile.channel.as_str() {
            "cli" => self.cli.ask(profile, secret, req).await,
            "http" => self.http.ask(profile, secret, req).await,
            other => Err(AppError::invalid_param(format!("未知 AI 通道: {other}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::StubClient;

    #[tokio::test]
    async fn routes_by_channel() {
        let d = ChannelDispatch {
            cli: Arc::new(StubClient::of(vec![AiEvent::Done { text: "cli".into(), cached: false }])),
            http: Arc::new(StubClient::of(vec![AiEvent::Done { text: "http".into(), cached: false }])),
        };
        let req = AiRequest {
            system: String::new(),
            user: String::new(),
            feature: "f".into(),
            prompt_tpl_version: "1".into(),
            timeout_secs: 5,
            max_tokens: None,
        };
        let p_cli = AiProfileRow {
            id: 1, name: "c".into(), channel: "cli".into(), cli_kind: Some("codex".into()),
            command_path: None, base_url: None, api_key_ref: None, model: "m".into(),
            extra_json: None, cli_version: None, config_fingerprint: None, is_active: true,
        };
        let p_http = AiProfileRow { channel: "http".into(), ..p_cli.clone() };
        let mut rx = d.ask(&p_cli, None, req.clone()).await.unwrap();
        while let Some(e) = rx.recv().await {
            if let AiEvent::Done { text, .. } = e { assert_eq!(text, "cli"); break; }
        }
        let mut rx = d.ask(&p_http, None, req.clone()).await.unwrap();
        while let Some(e) = rx.recv().await {
            if let AiEvent::Done { text, .. } = e { assert_eq!(text, "http"); break; }
        }
        let p_bad = AiProfileRow { channel: "other".into(), ..p_cli };
        assert!(d.ask(&p_bad, None, req).await.is_err());
    }
}
