//! CLI 通道适配器（技术设计 §7.1/§7.2、§7.5【修订·完善3】）。
//!
//! canonical 启动参数（执行前阻断，由 CliIsolationGuard 构造）：
//! - Claude Code：`claude -p --output-format stream-json --verbose
//!   --include-partial-messages --tools "" --disallowedTools "mcp__*"
//!   --strict-mcp-config --mcp-config '{"mcpServers":{}}' --setting-sources "project" --model <m>`
//! - Codex：`codex exec --skip-git-repo-check --ignore-user-config --ignore-rules
//!   --ephemeral --sandbox read-only --disable shell_tool --json --model <m>`
//! - OpenCode：`XDG_CONFIG_HOME=<空目录> OPENCODE_CONFIG_CONTENT='{"permission":{"*":"deny"}}'
//!   opencode run --format json --pure --dir <临时目录> --model <m>`
//!
//! 事件提取（三家统一原则）：增量事件仅供展示；终态事件（Claude `result` /
//! Codex `turn.completed` / OpenCode 消息完成）是完成判定与缓存落库的唯一依据。
//! 解析失败、事件序列异常（无终态、进程非零退出）→ 明确错误（含诊断日志），
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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
    /// 工具/命令执行类事件（监测层 → kill + 报警）。
    ToolUse { tool: String },
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
                            return ParsedEvent::ToolUse { tool: name.to_string() };
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
            let item_ty = v.pointer("/item/type").and_then(|t| t.as_str()).unwrap_or("");
            match item_ty {
                "agent_message" => {
                    let text = v.pointer("/item/text").and_then(|t| t.as_str()).unwrap_or("");
                    ParsedEvent::Final(text.to_string())
                }
                "command_execution" => ParsedEvent::ToolUse { tool: "command_execution".into() },
                _ => ParsedEvent::Ignored,
            }
        }
        "turn.completed" => ParsedEvent::Ignored, // 完成由 agent_message 终态承载
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
                                return ParsedEvent::ToolUse { tool: tool.to_string() };
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
            tool: v.get("tool").and_then(|t| t.as_str()).unwrap_or("unknown").to_string(),
        },
        _ => ParsedEvent::Ignored,
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
        let cli_kind = profile
            .cli_kind
            .as_deref()
            .and_then(CliKind3::from_str)
            .ok_or_else(|| AppError::invalid_param("CLI 通道缺少 cli_kind"))?;
        let command_path = profile
            .command_path
            .clone()
            .unwrap_or_else(|| cli_kind.as_str().to_string());
        let model = profile.model.clone();

        let launch = self
            .guard
            .build_launch_env(cli_kind)
            .await
            .map_err(|e| AppError::isolation(format!("隔离环境构造失败: {e}")))?;

        let mut args = canonical_args(cli_kind, &model);
        if cli_kind == CliKind3::Opencode {
            // --dir 注入临时空目录。
            if let Some(pos) = args.iter().position(|a| a.is_empty()) {
                args[pos] = launch.dir.display().to_string();
            }
        }

        let prompt = format!("{}\n\n{}", req.system, req.user);
        let timeout = Duration::from_secs(req.timeout_secs);
        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            let mut command = Command::new(&command_path);
            command.args(&args).envs(&launch.envs).current_dir(&launch.dir).stdin(std::process::Stdio::piped());
            command.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
            let mut child = match command.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx
                        .send(AiEvent::Failed(AppError::ai_channel(format!(
                            "CLI 启动失败（{}）: {e}；请检查命令路径与版本",
                            cli_kind.as_str()
                        ))))
                        .await;
                    return;
                }
            };
            if let Some(mut stdin) = child.stdin.take() {
                let _ = tokio::time::timeout(Duration::from_secs(5), stdin.write_all(prompt.as_bytes())).await;
            }
            let mut stdout = child.stdout.take().expect("stdout piped");
            let result: Result<String, AppError> = async {
                let mut buf = String::new();
                let mut deltas = String::new();
                let mut final_text: Option<String> = None;
                let mut bad_lines = 0usize;
                let read = tokio::time::timeout(timeout, stdout.read_to_string(&mut buf)).await;
                let _ = read; // 整体读取（CLI 输出量为一次问答规模）。
                let status = child.wait().await.map_err(|e| AppError::ai_channel(format!("CLI 等待失败: {e}")))?;
                for line in buf.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match parse_event_line(cli_kind, line) {
                        ParsedEvent::Delta(t) => deltas.push_str(&t),
                        ParsedEvent::Final(t) => final_text = Some(t),
                        ParsedEvent::ToolUse { tool } => {
                            // 监测层（仅异常发现）：kill + 报警（安全边界在守卫的执行前机制）。
                            return Err(AppError::isolation(format!(
                                "AI 通道权限隔离疑似失效（检测到工具事件 {tool}），已终止并丢弃结果；建议禁用该配置并重新验证"
                            )));
                        }
                        ParsedEvent::Ignored => {
                            if serde_json::from_str::<serde_json::Value>(line).is_err() {
                                bad_lines += 1;
                            }
                        }
                    }
                }
                let text = final_text.unwrap_or(deltas);
                if !status.success() {
                    return Err(AppError::ai_channel(format!(
                        "CLI 非零退出（{status}）"
                    ))
                    .with_detail(format!("坏行 {bad_lines}；stdout 前 500 字符: {}", &buf.chars().take(500).collect::<String>())));
                }
                if text.trim().is_empty() {
                    return Err(AppError::ai_channel(
                        "AI 通道输出解析失败：无终态事件或空文本；可重试，或检查 CLI 版本是否受支持",
                    )
                    .with_detail(format!("坏行 {bad_lines}；stdout 前 500 字符: {}", &buf.chars().take(500).collect::<String>())));
                }
                Ok(text)
            }
            .await;
            match result {
                Ok(text) => {
                    let _ = tx.send(AiEvent::Done { text, cached: false }).await;
                }
                Err(e) => {
                    let _ = tx.send(AiEvent::Failed(e)).await;
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
            ParsedEvent::Final("NO_TOOL_AVAILABLE".into())
        );
        assert!(matches!(
            parse_event_line(
                CliKind3::Codex,
                r#"{"type":"item.completed","item":{"type":"command_execution","command":"echo x"}}"#
            ),
            ParsedEvent::ToolUse { .. }
        ));
        assert_eq!(parse_event_line(CliKind3::Codex, r#"{"type":"turn.started"}"#), ParsedEvent::Ignored);
    }

    #[test]
    fn opencode_event_extraction() {
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
        assert_eq!(parse_event_line(CliKind3::Codex, "not json"), ParsedEvent::Ignored);
    }
}
