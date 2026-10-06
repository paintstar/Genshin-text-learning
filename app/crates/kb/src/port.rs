//! QuestSource 端口（架构 §3.2，决策 1）。
//!
//! 端口定义于消费域 kb（依赖倒置：fetcher 依赖 kb 实现该端口）。
//! **条件请求语义属端口契约的一部分**：详情取数入参携带调用方保存的
//! HTTP 校验元数据（ETag / Last-Modified），端口返回「未变化」或
//! 「新响应字节 + 新校验元数据」之一；任何实现不得绕开端口直连源站。
//!
//! 端口止步于「原始响应 + 元数据」——结构解析、内容摘要与派生行落库
//! 由 kb 的入库服务执行，不属于数据源职责。
//!
//! 关联类型（原始响应字节 + HTTP 缓存元数据 DTO 等）随端口定义于本 crate，
//! 不进 shared（不跨 FFI 边界，架构决策 1/14 注记排除项）。

use async_trait::async_trait;
use shared::{AppError, GameLang};

/// HTTP 条件请求校验元数据（随 quest_raw 保存）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HttpValidator {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

impl HttpValidator {
    pub fn is_empty(&self) -> bool {
        self.etag.is_none() && self.last_modified.is_none()
    }
}

/// 原始响应字节 + HTTP 缓存元数据。
#[derive(Debug, Clone)]
pub struct RawResponse {
    pub bytes: Vec<u8>,
    pub validator: Option<HttpValidator>,
}

/// 详情取数结果：未变化（304）或新响应。
#[derive(Debug, Clone)]
pub enum FetchDetailOutcome {
    NotModified,
    Modified(RawResponse),
}

/// 数据源端口：「按语言取任务索引 / 取任务详情」。
#[async_trait]
pub trait QuestSource: Send + Sync {
    async fn fetch_index(&self, lang: GameLang) -> Result<RawResponse, AppError>;

    async fn fetch_detail(
        &self,
        quest_id: i64,
        lang: GameLang,
        validator: Option<HttpValidator>,
    ) -> Result<FetchDetailOutcome, AppError>;
}

/// 进程内测试替身：按 (quest_id, lang) 预置响应，可注入故障与 304 行为。
pub struct StubSource {
    pub index: std::sync::Mutex<std::collections::HashMap<GameLang, Vec<u8>>>,
    pub details: std::sync::Mutex<std::collections::HashMap<(i64, GameLang), Vec<u8>>>,
    /// 设为 Some 时，所有调用返回该错误（网络故障注入）。
    pub fail_with: std::sync::Mutex<Option<shared::AppError>>,
    /// 对指定 (quest_id, lang) 强制返回 NotModified。
    pub force_not_modified: std::sync::Mutex<std::collections::HashSet<(i64, GameLang)>>,
}

impl Default for StubSource {
    fn default() -> Self {
        Self {
            index: std::sync::Mutex::new(std::collections::HashMap::new()),
            details: std::sync::Mutex::new(std::collections::HashMap::new()),
            fail_with: std::sync::Mutex::new(None),
            force_not_modified: std::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }
}

#[async_trait]
impl QuestSource for StubSource {
    async fn fetch_index(&self, lang: GameLang) -> Result<RawResponse, AppError> {
        if let Some(e) = self.fail_with.lock().unwrap().clone() {
            return Err(e);
        }
        let bytes = self
            .index
            .lock()
            .unwrap()
            .get(&lang)
            .cloned()
            .ok_or_else(|| AppError::data_source(format!("stub: 无索引 {lang}")))?;
        Ok(RawResponse { bytes, validator: None })
    }

    async fn fetch_detail(
        &self,
        quest_id: i64,
        lang: GameLang,
        _validator: Option<HttpValidator>,
    ) -> Result<FetchDetailOutcome, AppError> {
        if let Some(e) = self.fail_with.lock().unwrap().clone() {
            return Err(e);
        }
        if self.force_not_modified.lock().unwrap().contains(&(quest_id, lang)) {
            return Ok(FetchDetailOutcome::NotModified);
        }
        let bytes = self
            .details
            .lock()
            .unwrap()
            .get(&(quest_id, lang))
            .cloned()
            .ok_or_else(|| AppError::data_source(format!("stub: 无详情 {quest_id}/{lang}")))?;
        Ok(FetchDetailOutcome::Modified(RawResponse { bytes, validator: None }))
    }
}
