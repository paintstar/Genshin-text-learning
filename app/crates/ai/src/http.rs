//! HttpChatAdapter — OpenAI 兼容 HTTP 适配器（技术设计 §7.1【修订·驳回5】）。
//!
//! 云端 API 与本地大模型（ollama）共用同一实现：base_url 归一化 + SSE 流式解析。
//! deepseek 与 ollama 只是不同预设，不是两个适配器。
//!
//! base_url 约定（全代码库仅此一处拼接）：
//! - `base_url` 一律指「含版本前缀的 API 根地址」（deepseek 预设
//!   `https://api.deepseek.com/v1`、ollama 预设 `http://localhost:11434/v1`）；
//! - 适配器只追加 `/chat/completions`；
//! - 保存时归一化：去首尾空白与尾部 `/`；以 `/chat/completions` 结尾则剥离；
//!   已知预设主机未带路径时自动补 `/v1`；其余自定义地址保留路径前缀原样。

use crate::client::{AiClient, AiEvent, AiRequest};
use async_trait::async_trait;
use futures_util::StreamExt;
use shared::{error_chain, AppError};
use std::time::Duration;
use tokio::sync::mpsc;

/// base_url 归一化（保存时执行；技术设计 §7.1 修订·驳回5）。
pub fn normalize_base_url(input: &str) -> String {
    let mut s = input.trim().trim_end_matches('/').to_string();
    if s.ends_with("/chat/completions") {
        s = s[..s.len() - "/chat/completions".len()]
            .trim_end_matches('/')
            .to_string();
    }
    // 已知预设主机：未带路径时自动补 /v1。
    let known = [
        ("https://api.deepseek.com", "https://api.deepseek.com/v1"),
        ("http://api.deepseek.com", "http://api.deepseek.com/v1"),
        ("https://localhost:11434", "https://localhost:11434/v1"),
        ("http://localhost:11434", "http://localhost:11434/v1"),
        ("https://127.0.0.1:11434", "https://127.0.0.1:11434/v1"),
        ("http://127.0.0.1:11434", "http://127.0.0.1:11434/v1"),
    ];
    for (host, full) in known {
        if s == host {
            s = full.to_string();
            break;
        }
    }
    s
}

/// 拼接完整 endpoint（适配器仅追加 /chat/completions）。
pub fn endpoint(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

pub struct HttpChatAdapter {
    client: reqwest::Client,
}

impl Default for HttpChatAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpChatAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("GenshinLangLearning/0.1.0")
                .build()
                .expect("http client"),
        }
    }
}

#[async_trait]
impl AiClient for HttpChatAdapter {
    async fn ask(
        &self,
        profile: &crate::profile::AiProfileRow,
        secret: Option<String>,
        req: AiRequest,
    ) -> Result<mpsc::Receiver<AiEvent>, AppError> {
        let base = profile
            .base_url
            .clone()
            .ok_or_else(|| AppError::invalid_param("HTTP 通道缺少 base_url"))?;
        let url = endpoint(&normalize_base_url(&base));
        let api_key = secret
            .or_else(|| profile.api_key_ref.clone())
            .ok_or_else(|| AppError::invalid_param("HTTP 通道缺少 API 密钥（设置页录入）"))?;
        let model = profile.model.clone();
        let timeout = req.timeout_secs;
        let max_tokens = req.max_tokens;
        let http = self.client.clone();

        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            let mut body = serde_json::json!({
                "model": model,
                "stream": true,
                "messages": [
                    { "role": "system", "content": req.system },
                    { "role": "user", "content": req.user }
                ]
            });
            if let Some(mt) = max_tokens {
                body["max_tokens"] = serde_json::json!(mt);
            }
            let result: Result<(), AppError> = async {
                let send = tokio::time::timeout(
                    Duration::from_secs(timeout),
                    http.post(&url).bearer_auth(&api_key).json(&body).send(),
                )
                .await
                .map_err(|_| AppError::network("AI 请求超时"))?
                .map_err(|e| AppError::network(format!("AI 请求失败: {}", error_chain(&e))))?;
                let status = send.status();
                if !status.is_success() {
                    let text = send.text().await.unwrap_or_default();
                    return Err(AppError::ai_channel(format!("AI 服务返回 {status}")).with_detail(text));
                }
                let mut stream = send.bytes_stream();
                let mut buf = String::new();
                let mut full = String::new();
                loop {
                    let chunk = tokio::time::timeout(
                        Duration::from_secs(timeout),
                        stream.next(),
                    )
                    .await
                    .map_err(|_| AppError::network("AI 流式读取超时"))?;
                    match chunk {
                        None => break,
                        Some(Ok(bytes)) => {
                            buf.push_str(&String::from_utf8_lossy(&bytes));
                            // SSE：按行解析 data: 前缀。
                            while let Some(pos) = buf.find('\n') {
                                let line: String = buf.drain(..=pos).collect();
                                let line = line.trim_end();
                                if let Some(data) = line.strip_prefix("data:") {
                                    let data = data.trim();
                                    if data == "[DONE]" {
                                        continue;
                                    }
                                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                                        if let Some(delta) =
                                            v.pointer("/choices/0/delta/content").and_then(|x| x.as_str())
                                        {
                                            if !delta.is_empty() {
                                                let _ = tx.send(AiEvent::Delta(delta.to_string())).await;
                                                full.push_str(delta);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Some(Err(e)) => return Err(AppError::network(format!("AI 流读取失败: {e}"))),
                    }
                }
                if full.trim().is_empty() {
                    return Err(AppError::ai_channel("AI 通道返回空响应（不落缓存）"));
                }
                let _ = tx.send(AiEvent::Done { text: full, cached: false }).await;
                Ok(())
            }
            .await;
            if let Err(e) = result {
                let _ = tx.send(AiEvent::Failed(e)).await;
            }
        });
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_rules() {
        // 尾斜杠 / 完整端点粘贴 / 预设主机自动补 /v1 / 自定义路径原样。
        assert_eq!(normalize_base_url("https://api.deepseek.com/v1/"), "https://api.deepseek.com/v1");
        assert_eq!(
            normalize_base_url("https://api.deepseek.com/v1/chat/completions"),
            "https://api.deepseek.com/v1"
        );
        assert_eq!(normalize_base_url("http://localhost:11434"), "http://localhost:11434/v1");
        assert_eq!(normalize_base_url("https://gw.example.com/llm/v1"), "https://gw.example.com/llm/v1");
        // 不产生 /v1/v1/。
        assert_eq!(endpoint(&normalize_base_url("http://localhost:11434/v1")), "http://localhost:11434/v1/chat/completions");
        assert_eq!(
            endpoint(&normalize_base_url("https://api.deepseek.com/v1")),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }
}
