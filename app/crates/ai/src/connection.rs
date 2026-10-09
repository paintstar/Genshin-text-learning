use crate::{AiClient, AiEvent, AiProfileRow, AiRequest};
use shared::AppError;
use std::time::Duration;

/// 连接测试与功能调用走相同通道，只接受完整、非空、未缓存的真实模型回答。
pub async fn verify_connection(
    client: &dyn AiClient,
    profile: &AiProfileRow,
    secret: Option<String>,
) -> Result<(), AppError> {
    tokio::time::timeout(Duration::from_secs(90), async {
        let mut events = client
            .ask(
                profile,
                secret,
                AiRequest {
                    system: "你是语言助手。只输出简短文字，不要调用任何工具。".into(),
                    user: "请回复：连接测试成功".into(),
                    feature: "connection_test".into(),
                    prompt_tpl_version: "2".into(),
                    timeout_secs: 60,
                    max_tokens: Some(32),
                },
            )
            .await?;
        while let Some(event) = events.recv().await {
            match event {
                AiEvent::Done {
                    text,
                    cached: false,
                } if !text.trim().is_empty() => return Ok(()),
                AiEvent::Done { .. } => {
                    return Err(AppError::ai_channel("连接测试没有收到真实、完整的模型回答"))
                }
                AiEvent::Failed(error) => return Err(error),
                AiEvent::Delta(_) => {}
            }
        }
        Err(AppError::ai_channel("连接测试未收到完整模型回答"))
    })
    .await
    .map_err(|_| AppError::ai_channel("连接测试超时，请检查 CLI 配置与网络"))?
}
