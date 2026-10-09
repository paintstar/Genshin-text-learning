//! CLI 通道适配器（技术设计 §7.1/§7.2、§7.5【修订·完善3】）。
//!
//! canonical 启动参数（执行前阻断，由 CliIsolationGuard 构造）：
//! - Claude Code：`claude -p --output-format stream-json --verbose
//!   --include-partial-messages --tools "" --disallowedTools "mcp__*"
//!   --strict-mcp-config --mcp-config '{"mcpServers":{}}' --setting-sources "project" --model <m>`
//! - Codex：`codex exec --skip-git-repo-check --ignore-user-config --ignore-rules
//!   --ephemeral --sandbox read-only --disable shell_tool --json --model <m>`
//! - OpenCode：读取用户的模型服务配置后，在临时配置目录中关闭工具、插件和 MCP，
//!   使用 `opencode run --format json --pure --dir <临时目录> --model <服务商/模型>`。
//!
//! 事件提取（三家统一原则）：增量事件仅供展示；终态事件（Claude `result` /
//! Codex `turn.completed` / OpenCode `step_finish`）是完成判定与缓存落库的唯一依据。
//! 解析失败、事件序列异常（无终态、进程非零退出）→ 明确错误，
//! **不降级展示 stdout 尾部**。
//!
//! 运行时事件流监测（定位：仅异常发现，不构成安全边界）：任何工具调用/命令
//! 执行类事件 → 立即 kill 进程、丢弃结果、报隔离疑似失效错误。

use crate::client::{AiClient, AiEvent, AiRequest};
use crate::guard::CliIsolationGuard;
use crate::profile::AiProfileRow;
use async_trait::async_trait;
use shared::AppError;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// CLI 三家。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliKind3 {
    Claude,
    Codex,
    Opencode,
}

impl CliKind3 {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(CliKind3::Claude),
            "codex" => Some(CliKind3::Codex),
            "opencode" => Some(CliKind3::Opencode),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            CliKind3::Claude => "claude",
            CliKind3::Codex => "codex",
            CliKind3::Opencode => "opencode",
        }
    }
}

/// 构造 canonical 启动参数（隔离环境由守卫提供；此处仅参数序列）。
pub fn canonical_args(kind: CliKind3, model: &str) -> Vec<String> {
    match kind {
        CliKind3::Claude => vec![
            "-p".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
            "--include-partial-messages".into(),
            "--tools".into(),
            "".into(),
            "--disallowedTools".into(),
            "mcp__*".into(),
            "--strict-mcp-config".into(),
            "--mcp-config".into(),
            "{\"mcpServers\":{}}".into(),
            "--setting-sources".into(),
            "project".into(),
            "--model".into(),
            model.into(),
        ],
        CliKind3::Codex => vec![
            "exec".into(),
            "--skip-git-repo-check".into(),
            "--ignore-user-config".into(),
            "--ignore-rules".into(),
            "--ephemeral".into(),
            "--sandbox".into(),
            "read-only".into(),
            "--disable".into(),
            "shell_tool".into(),
            "--json".into(),
            "--model".into(),
            model.into(),
        ],
        CliKind3::Opencode => vec![
            "run".into(),
            "--format".into(),
            "json".into(),
            "--pure".into(),
            "--dir".into(),
            // dir 由守卫的临时目录注入（占位由调用方追加覆盖）。
            String::new(),
            "--model".into(),
            model.into(),
        ],
    }
}

/// 一行 NDJSON 事件的解析产物。
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedEvent {
    Delta(String),
    /// 终态（携带最终文本）。
    Final(String),
    Finished,
    Failed(String),
    /// 工具/命令执行类事件（监测层 → kill + 报警）。
    ToolUse {
        tool: String,
    },
    /// 可识别但与文本无关的事件。
    Ignored,
}

/// 解析单行事件（每家 CLI 一个策略；三家语义契约统一：增量/终态/工具）。
pub fn parse_event_line(kind: CliKind3, line: &str) -> ParsedEvent {
    let v: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return ParsedEvent::Ignored, // 坏行由调用方计数（诊断日志）
    };
    match kind {
        CliKind3::Claude => parse_claude(&v),
        CliKind3::Codex => parse_codex(&v),
        CliKind3::Opencode => parse_opencode(&v),
    }
}

fn parse_claude(v: &serde_json::Value) -> ParsedEvent {
    let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match ty {
        "stream_event" => {
            if v.pointer("/event/delta/type").and_then(|x| x.as_str()) == Some("text_delta") {
                match v.pointer("/event/delta/text").and_then(|x| x.as_str()) {
                    Some(t) if !t.is_empty() => ParsedEvent::Delta(t.to_string()),
                    _ => ParsedEvent::Ignored,
                }
            } else {
                ParsedEvent::Ignored
            }
        }
        "result" => {
            if v.get("is_error").and_then(|value| value.as_bool()) == Some(true)
                || v.get("subtype")
                    .and_then(|value| value.as_str())
                    .is_some_and(|subtype| subtype != "success")
            {
                return ParsedEvent::Failed(cli_failure_hint(&v.to_string()));
            }
            // 终态：末行 result 携带完整结果。
            let text = v
                .get("result")
                .and_then(|r| r.as_str())
                .or_else(|| v.get("text").and_then(|t| t.as_str()))
                .unwrap_or("");
            ParsedEvent::Final(text.to_string())
        }
        // 工具调用事件（assistant 消息内 tool_use 块）。
        "assistant" => {
            if let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) {
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                            return ParsedEvent::ToolUse {
                                tool: name.to_string(),
                            };
                        }
                    }
                }
            }
            ParsedEvent::Ignored
        }
        _ => ParsedEvent::Ignored,
    }
}

fn parse_codex(v: &serde_json::Value) -> ParsedEvent {
    let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match ty {
        "item.completed" => {
            let item_ty = v
                .pointer("/item/type")
                .and_then(|t| t.as_str())
                .unwrap_or("");
            match item_ty {
                "agent_message" => {
                    let text = v
                        .pointer("/item/text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    ParsedEvent::Delta(text.to_string())
                }
                "command_execution" => ParsedEvent::ToolUse {
                    tool: "command_execution".into(),
                },
                _ => ParsedEvent::Ignored,
            }
        }
        "turn.completed" => ParsedEvent::Finished,
        "turn.failed" | "error" => ParsedEvent::Failed(cli_failure_hint(&v.to_string())),
        "item.started" | "turn.started" | "thread.started" => ParsedEvent::Ignored,
        _ => {
            // 宽容：未知事件若携带 text 字段按增量处理，否则忽略。
            if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                if !t.is_empty() {
                    return ParsedEvent::Delta(t.to_string());
                }
            }
            ParsedEvent::Ignored
        }
    }
}

fn parse_opencode(v: &serde_json::Value) -> ParsedEvent {
    let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match ty {
        "text" => v
            .pointer("/part/text")
            .and_then(|text| text.as_str())
            .map(|text| ParsedEvent::Delta(text.to_string()))
            .unwrap_or(ParsedEvent::Ignored),
        "step_finish" => match v.pointer("/part/reason").and_then(|reason| reason.as_str()) {
            Some("stop") => ParsedEvent::Finished,
            Some("length") => {
                ParsedEvent::Failed("模型回答达到长度限制，请调整模型配置后重试".into())
            }
            _ => ParsedEvent::Ignored,
        },
        "error" => ParsedEvent::Failed(cli_failure_hint(&v.to_string())),
        "tool_use" => ParsedEvent::ToolUse {
            tool: v
                .pointer("/part/tool")
                .and_then(|tool| tool.as_str())
                .unwrap_or("unknown")
                .to_string(),
        },
        "assistant" | "message.updated" | "message.completed" => {
            if let Some(parts) = v.pointer("/info/parts").and_then(|p| p.as_array()) {
                let mut text = String::new();
                for part in parts {
                    match part.get("type").and_then(|t| t.as_str()) {
                        Some("text") => {
                            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                                text.push_str(t);
                            }
                        }
                        Some("tool") => {
                            if let Some(tool) = part.get("tool").and_then(|t| t.as_str()) {
                                return ParsedEvent::ToolUse {
                                    tool: tool.to_string(),
                                };
                            }
                        }
                        _ => {}
                    }
                }
                if !text.is_empty() {
                    return ParsedEvent::Final(text);
                }
            }
            ParsedEvent::Ignored
        }
        "tool" => ParsedEvent::ToolUse {
            tool: v
                .get("tool")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown")
                .to_string(),
        },
        _ => ParsedEvent::Ignored,
    }
}

/// CLI 错误可能包含请求头或服务配置，只向界面传递可操作的错误类别。
pub fn cli_failure_hint(raw: &str) -> String {
    let error = raw.to_lowercase();
    if error.contains("certificate") || error.contains("cert_verify") {
        "模型服务的证书校验失败，请检查系统信任证书或 CLI 的证书配置".into()
    } else if error.contains("401")
        || error.contains("unauthorized")
        || error.contains("api key")
        || error.contains("apikey")
        || error.contains("authentication")
    {
        "模型认证失败，请先在 CLI 中登录或检查模型服务的密钥".into()
    } else if error.contains("modelnotfound")
        || error.contains("model not found")
        || error.contains("providernotfound")
        || error.contains("unknown model")
    {
        "模型不存在或服务未配置，请读取模型列表并选择完整模型标识".into()
    } else if error.contains("429") || error.contains("rate limit") || error.contains("quota") {
        "模型服务额度不足或请求过于频繁，请稍后重试或检查额度".into()
    } else if error.contains("connection")
        || error.contains("fetch failed")
        || error.contains("timeout")
        || error.contains("econn")
    {
        "无法连接模型服务，请检查网络及 CLI 的服务配置".into()
    } else {
        "CLI 调用失败，请检查 CLI 配置、模型及版本".into()
    }
}

/// CLI 适配器（通信）；隔离环境构造在 CliIsolationGuard（安全关注点分离）。
pub struct CliAdapter {
    pub guard: std::sync::Arc<CliIsolationGuard>,
}

#[async_trait]
impl AiClient for CliAdapter {
    async fn ask(
        &self,
        profile: &AiProfileRow,
        _secret: Option<String>,
        req: AiRequest,
    ) -> Result<mpsc::Receiver<AiEvent>, AppError> {
        let kind = profile
            .cli_kind
            .as_deref()
            .and_then(CliKind3::from_str)
            .ok_or_else(|| AppError::invalid_param("CLI 通道缺少 CLI 类型"))?;
        let command_path = profile
            .command_path
            .as_deref()
            .filter(|path| !path.trim().is_empty())
            .unwrap_or_else(|| kind.as_str())
            .to_string();
        let launch = self.guard.prepare_launch(kind, &command_path).await?;
        let command_path = launch.command.clone();
        let model = if kind == CliKind3::Opencode {
            self.guard
                .resolve_opencode_model(&command_path, &profile.model, &launch)
                .await?
        } else {
            profile.model.clone()
        };
        let mut args = canonical_args(kind, &model);
        if kind == CliKind3::Opencode {
            if let Some(position) = args.iter().position(String::is_empty) {
                args[position] = launch.dir.display().to_string();
            }
        }
        let prompt = format!("{}\n\n{}", req.system, req.user);
        let timeout = Duration::from_secs(req.timeout_secs.max(1));
        let (tx, rx) = mpsc::channel(64);
        tokio::spawn(async move {
            // 整个启动环境移入任务，确保临时配置持续存在到子进程结束。
            let launch = launch;
            let mut command = Command::new(&command_path);
            command
                .args(args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            launch.apply(&mut command);
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(_) => {
                    let _ = tx
                        .send(AiEvent::Failed(AppError::ai_channel(
                            "CLI 无法启动，请检查命令或可执行文件路径",
                        )))
                        .await;
                    return;
                }
            };
            let mut stdout = BufReader::new(child.stdout.take().expect("stdout piped")).lines();
            let mut stderr = child.stderr.take().expect("stderr piped");
            let stderr_task = tokio::spawn(async move {
                let mut kept = Vec::new();
                let mut buffer = [0u8; 4096];
                while let Ok(count) = stderr.read(&mut buffer).await {
                    if count == 0 {
                        break;
                    }
                    let retain = count.min(16384usize.saturating_sub(kept.len()));
                    kept.extend_from_slice(&buffer[..retain]);
                }
                String::from_utf8_lossy(&kept).to_string()
            });
            let result = tokio::select! {
                _ = tx.closed() => {
                    let _ = child.kill().await;
                    stderr_task.abort();
                    return;
                }
                result = tokio::time::timeout(timeout, async {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(prompt.as_bytes()).await;
                    }
                    let mut text = String::new();
                    let mut finished = false;
                    while let Some(line) = stdout.next_line().await.map_err(|_| AppError::ai_channel("CLI 输出读取失败"))? {
                        match parse_event_line(kind, &line) {
                            ParsedEvent::Delta(delta) => {
                                text.push_str(&delta);
                                let _ = tx.send(AiEvent::Delta(delta)).await;
                            }
                            ParsedEvent::Final(final_text) => { text = final_text; finished = true; }
                            ParsedEvent::Finished => finished = true,
                            ParsedEvent::Failed(message) => return Err(AppError::ai_channel(message)),
                            ParsedEvent::ToolUse { .. } => return Err(AppError::isolation("语言助手尝试调用工具，已停止本次请求")),
                            ParsedEvent::Ignored => {}
                        }
                    }
                    let status = child.wait().await.map_err(|_| AppError::ai_channel("CLI 进程等待失败"))?;
                    if !status.success() {
                        return Err(AppError::ai_channel("CLI 非正常退出，请检查模型、登录状态及 CLI 版本"));
                    }
                    if !finished || text.trim().is_empty() {
                        return Err(AppError::ai_channel("CLI 未返回完整的模型回答，本次请求失败"));
                    }
                    Ok(text)
                }) => match result {
                    Ok(result) => result,
                    Err(_) => Err(AppError::ai_channel("模型调用超时，已停止本次请求")),
                },
            };
            if result.is_err() {
                let _ = child.kill().await;
            }
            let mut stderr_task = stderr_task;
            let diagnostic = tokio::time::timeout(Duration::from_secs(2), &mut stderr_task)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default();
            stderr_task.abort();
            match result {
                Ok(text) => {
                    let _ = tx
                        .send(AiEvent::Done {
                            text,
                            cached: false,
                        })
                        .await;
                }
                Err(error) => {
                    let error = if error.message.starts_with("CLI 非正常退出")
                        && !diagnostic.trim().is_empty()
                    {
                        AppError::ai_channel(cli_failure_hint(&diagnostic))
                    } else {
                        error
                    };
                    let _ = tx.send(AiEvent::Failed(error)).await;
                }
            }
        });
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_event_extraction() {
        // 增量：stream_event + text_delta。
        assert_eq!(
            parse_event_line(
                CliKind3::Claude,
                r#"{"type":"stream_event","event":{"delta":{"type":"text_delta","text":"こん"}}}"#
            ),
            ParsedEvent::Delta("こん".into())
        );
        // 终态：result。
        assert_eq!(
            parse_event_line(CliKind3::Claude, r#"{"type":"result","result":"最终回答"}"#),
            ParsedEvent::Final("最终回答".into())
        );
        // 工具事件 → 监测。
        assert!(matches!(
            parse_event_line(
                CliKind3::Claude,
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}"#
            ),
            ParsedEvent::ToolUse { .. }
        ));
    }

    #[test]
    fn codex_event_extraction() {
        assert_eq!(
            parse_event_line(
                CliKind3::Codex,
                r#"{"type":"item.completed","item":{"type":"agent_message","text":"NO_TOOL_AVAILABLE"}}"#
            ),
            ParsedEvent::Delta("NO_TOOL_AVAILABLE".into())
        );
        assert!(matches!(
            parse_event_line(
                CliKind3::Codex,
                r#"{"type":"item.completed","item":{"type":"command_execution","command":"echo x"}}"#
            ),
            ParsedEvent::ToolUse { .. }
        ));
        assert_eq!(
            parse_event_line(CliKind3::Codex, r#"{"type":"turn.started"}"#),
            ParsedEvent::Ignored
        );
    }

    #[test]
    fn opencode_event_extraction() {
        assert_eq!(
            parse_event_line(
                CliKind3::Opencode,
                r#"{"type":"text","part":{"text":"语法分析"}}"#
            ),
            ParsedEvent::Delta("语法分析".into())
        );
        assert_eq!(
            parse_event_line(
                CliKind3::Opencode,
                r#"{"type":"step_finish","part":{"reason":"stop"}}"#
            ),
            ParsedEvent::Finished
        );
        assert!(matches!(
            parse_event_line(
                CliKind3::Opencode,
                r#"{"type":"error","error":{"name":"ModelNotFoundError"}}"#
            ),
            ParsedEvent::Failed(_)
        ));
        assert_eq!(
            parse_event_line(
                CliKind3::Opencode,
                r#"{"type":"assistant","info":{"parts":[{"type":"text","text":"回答"}]}}"#
            ),
            ParsedEvent::Final("回答".into())
        );
        assert!(matches!(
            parse_event_line(
                CliKind3::Opencode,
                r#"{"type":"assistant","info":{"parts":[{"type":"tool","tool":"bash"}]}}"#
            ),
            ParsedEvent::ToolUse { .. }
        ));
    }

    #[test]
    fn canonical_args_match_design() {
        let claude = canonical_args(CliKind3::Claude, "sonnet");
        assert!(claude.contains(&"--tools".to_string()));
        // --tools "" 空串在参数序列中。
        let pos = claude.iter().position(|a| a == "--tools").unwrap();
        assert_eq!(claude[pos + 1], "");
        assert!(claude.contains(&"--strict-mcp-config".into()));
        assert!(claude.contains(&"--setting-sources".into()));
        let codex = canonical_args(CliKind3::Codex, "gpt");
        for flag in [
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "--sandbox",
            "read-only",
            "--disable",
            "shell_tool",
            "--json",
        ] {
            assert!(codex.contains(&flag.to_string()), "codex 缺少 {flag}");
        }
        let oc = canonical_args(CliKind3::Opencode, "m");
        assert!(oc.contains(&"--pure".into()));
        assert!(oc.contains(&"--format".into()));
    }

    #[test]
    fn bad_json_lines_are_ignored_not_fatal() {
        assert_eq!(
            parse_event_line(CliKind3::Codex, "not json"),
            ParsedEvent::Ignored
        );
    }
}
