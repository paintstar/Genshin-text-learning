//! AiClient 端口（架构 §3.2，决策 1）。
//!
//! 一次调用 = 一段带上下文的问答：输入 system/user 提示，输出 markdown 文本流
//! + 终态完成事件，支持取消与超时。统一以 markdown 文本为输出契约——能力差异
//! 容忍（不要求弱模型严格 JSON）；流式返回经 mpsc 通道（增量/终态/失败三通道，
//! 架构 §6.3：增量仅供展示，终态是完成判定与缓存落库的唯一依据）。

use async_trait::async_trait;
use shared::AppError;
use tokio::sync::mpsc;

/// 一次 AI 问答请求。feature 与 prompt_tpl_version 参与缓存键（§7.4）。
#[derive(Debug, Clone)]
pub struct AiRequest {
    pub system: String,
    pub user: String,
    /// 功能标识（提示词目录条目 id）。
    pub feature: String,
    /// 提示词模板版本（模板改动时 bump，避免旧缓存污染）。
    pub prompt_tpl_version: String,
    pub timeout_secs: u64,
    pub max_tokens: Option<u32>,
}

/// 流事件（三通道封闭集合）。
#[derive(Debug, Clone)]
pub enum AiEvent {
    /// 增量文本（渐进渲染用）。
    Delta(String),
    /// 终态完成（完成判定与缓存落库唯一依据；cached 标注缓存命中）。
    Done { text: String, cached: bool },
    /// 失败/取消（取消不落缓存、不报故障）。
    Failed(AppError),
}

#[async_trait]
pub trait AiClient: Send + Sync {
    /// 发起一次问答，返回事件流接收端。调用方持有 Receiver 直至终态。
    async fn ask(
        &self,
        profile: &crate::profile::AiProfileRow,
        secret: Option<String>,
        req: AiRequest,
    ) -> Result<mpsc::Receiver<AiEvent>, AppError>;
}

/// 测试替身：可编程事件序列。
pub struct StubClient {
    pub events: std::sync::Mutex<Vec<AiEvent>>,
}

impl StubClient {
    pub fn of(events: Vec<AiEvent>) -> Self {
        Self {
            events: std::sync::Mutex::new(events),
        }
    }
}

#[async_trait]
impl AiClient for StubClient {
    async fn ask(
        &self,
        _profile: &crate::profile::AiProfileRow,
        _secret: Option<String>,
        _req: AiRequest,
    ) -> Result<mpsc::Receiver<AiEvent>, AppError> {
        let (tx, rx) = mpsc::channel(64);
        let events = self.events.lock().unwrap().clone();
        tokio::spawn(async move {
            for e in events {
                if tx.send(e).await.is_err() {
                    break;
                }
            }
        });
        Ok(rx)
    }
}
