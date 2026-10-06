//! yatta（Project Amber）适配器 — QuestSource 端口实现。
//!
//! 已验证事实（技术设计 §2.1/附录 A）：
//! - `GET https://gi.yatta.moe/api/v2/{lang}/quest` 与 `/quest/{id}` 返回 200，
//!   `/api/v2/*` 对任意 UA 放行——应用可用自己的 UA 诚实标识、无需伪装浏览器。
//! - 语言代码沿用游戏官方码（首期 jp/chs）。
//! - 条件请求（If-None-Match / If-Modified-Since → 304）经端口契约传入。

use crate::{FetchScheduler, TermsGate, USER_AGENT};
use async_trait::async_trait;
use kb::{FetchDetailOutcome, HttpValidator, QuestSource, RawResponse};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH};
use shared::{AppError, GameLang};
use std::sync::Arc;
use std::time::Duration;

pub struct YattaClient {
    base_url: String,
    client: reqwest::Client,
    scheduler: Arc<FetchScheduler>,
    gate: Arc<TermsGate>,
    /// 高级设置：自定义请求头（Cloudflare 风险对策，技术设计 §2.6-2）。
    custom_headers: std::sync::RwLock<HeaderMap>,
}

impl YattaClient {
    pub fn new(
        base_url: impl Into<String>,
        scheduler: Arc<FetchScheduler>,
        gate: Arc<TermsGate>,
    ) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| AppError::network(format!("HTTP 客户端初始化失败: {e}")))?;
        Ok(Self {
            base_url: base_url.into(),
            client,
            scheduler,
            gate,
            custom_headers: std::sync::RwLock::new(HeaderMap::new()),
        })
    }

    pub fn set_custom_headers(&self, json: &str) -> Result<(), AppError> {
        let map: std::collections::HashMap<String, String> = serde_json::from_str(json)
            .map_err(|e| AppError::invalid_param(format!("自定义请求头 JSON 非法: {e}")))?;
        let mut headers = HeaderMap::new();
        for (k, v) in map {
            let name = HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| AppError::invalid_param(format!("请求头名非法 {k}: {e}")))?;
            let val = HeaderValue::from_str(&v)
                .map_err(|e| AppError::invalid_param(format!("请求头值非法 {k}: {e}")))?;
            headers.insert(name, val);
        }
        *self.custom_headers.write().unwrap() = headers;
        Ok(())
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base_url.trim_end_matches('/'), path)
    }

    fn validator_of(headers: &reqwest::header::HeaderMap) -> Option<HttpValidator> {
        let etag = headers
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let last_modified = headers
            .get("last-modified")
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        if etag.is_none() && last_modified.is_none() {
            None
        } else {
            Some(HttpValidator { etag, last_modified })
        }
    }

    async fn get(
        &self,
        url: &str,
        conditional: Option<&HttpValidator>,
    ) -> Result<reqwest::Response, AppError> {
        let url = url.to_string();
        let cond = conditional.cloned();
        self.scheduler
            .run(|| {
                let client = &self.client;
                let custom = self.custom_headers.read().unwrap().clone();
                let url = url.clone();
                let cond = cond.clone();
                async move {
                    // M0 门禁：每次出站请求前检查（在调度器锁外由 TermsGate 持有状态）。
                    self.gate.check()?;
                    let mut req = client.get(&url).headers(custom);
                    if let Some(v) = &cond {
                        if let Some(etag) = &v.etag {
                            req = req.header(IF_NONE_MATCH, etag);
                        }
                        if let Some(lm) = &v.last_modified {
                            req = req.header(IF_MODIFIED_SINCE, lm);
                        }
                    }
                    let resp = req
                        .send()
                        .await
                        .map_err(|e| AppError::network(format!("请求失败: {e}")))?;
                    match resp.status() {
                        s if s.is_success() => Ok(resp),
                        reqwest::StatusCode::NOT_MODIFIED => Ok(resp),
                        s if s.as_u16() >= 500 => {
                            Err(AppError::data_source(format!("源站错误 {s}")))
                        }
                        s => Err(AppError::data_source(format!("源站拒绝 {s}: {url}"))),
                    }
                }
            })
            .await
    }
}

#[async_trait]
impl QuestSource for YattaClient {
    async fn fetch_index(&self, lang: GameLang) -> Result<RawResponse, AppError> {
        self.gate.check()?;
        let url = self.url(&format!("{}/quest", lang.as_str()));
        let resp = self.get(&url, None).await?;
        let validator = Self::validator_of(resp.headers());
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| AppError::network(format!("读取响应失败: {e}")))?;
        Ok(RawResponse {
            bytes: bytes.to_vec(),
            validator,
        })
    }

    async fn fetch_detail(
        &self,
        quest_id: i64,
        lang: GameLang,
        validator: Option<HttpValidator>,
    ) -> Result<FetchDetailOutcome, AppError> {
        self.gate.check()?;
        let url = self.url(&format!("{}/quest/{}", lang.as_str(), quest_id));
        let resp = self.get(&url, validator.as_ref()).await?;
        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(FetchDetailOutcome::NotModified);
        }
        let new_validator = Self::validator_of(resp.headers());
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| AppError::network(format!("读取响应失败: {e}")))?;
        Ok(FetchDetailOutcome::Modified(RawResponse {
            bytes: bytes.to_vec(),
            validator: new_validator,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_joining() {
        let c = YattaClient::new(
            "https://gi.yatta.moe/api/v2/",
            Arc::new(FetchScheduler::for_tests()),
            Arc::new(TermsGate::new()),
        )
        .unwrap();
        assert_eq!(c.url("jp/quest"), "https://gi.yatta.moe/api/v2/jp/quest");
    }

    #[tokio::test]
    async fn m0_gate_blocks_before_any_request() {
        let gate = Arc::new(TermsGate::new());
        let c = YattaClient::new(
            "https://invalid.example",
            Arc::new(FetchScheduler::for_tests()),
            gate.clone(),
        )
        .unwrap();
        let err = c.fetch_index(GameLang::Jp).await.unwrap_err();
        assert_eq!(err.kind, shared::AppErrorKind::DataSource);
        assert!(err.message.contains("连接"));
    }
}
