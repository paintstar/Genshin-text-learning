//! CliIsolationGuard — CLI 隔离守卫（技术设计 §7.2【修订·驳回v2-1/2/5】/ 架构决策 7）。
//!
//! 安全边界的全部机制持有者（与通信适配器物理分离）：
//! - a) 共同启动环境：临时空工作目录（每次调用新建，排除项目级配置被发现）。
//! - d) OpenCode：`XDG_CONFIG_HOME=<应用空目录>` 全局配置来源隔离 +
//!   `OPENCODE_CONFIG_CONTENT='{"permission":{"*":"deny"}}'` 唯一 permission 来源 +
//!   `--pure` 插件隔离 + **生效配置预检**（同环境运行官方 `opencode debug config`
//!   并断言 resolved 结果干净）。
//! - e) 启用验证：`--version` 记录（入缓存指纹）；工具阻断探针（一次性，
//!     断言事件流 0 工具/命令事件）；预检/探针失败 → 拒绝启用 + 展示原始输出。
//!
//! 定位红线：事件流监测仅异常报警、探针仅启用验证，均不构成安全边界；
//! 安全边界 = 执行前阻断机制（工具移除/权限拒绝/配置来源隔离）。

use crate::cli::CliKind3;
use serde_json::Value;
use shared::AppError;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// 一次隔离启动环境。
pub struct LaunchEnv {
    pub dir: PathBuf,
    pub envs: HashMap<String, String>,
    /// 临时目录的清理句柄（drop 时删除）。
    _keep: Option<tempfile::TempDir>,
}

pub struct CliIsolationGuard {
    /// OpenCode 隔离用的应用管理空目录（XDG_CONFIG_HOME 指向它）。
    opencode_config_root: PathBuf,
}

impl CliIsolationGuard {
    pub fn new(app_data_dir: &std::path::Path) -> Self {
        Self {
            opencode_config_root: app_data_dir.join("isolation").join("opencode-config"),
        }
    }

    /// a) 构造共同启动环境（临时空目录 + 家族特定环境变量）。
    pub async fn build_launch_env(&self, kind: CliKind3) -> Result<LaunchEnv, AppError> {
        let temp = tempfile::tempdir()
            .map_err(|e| AppError::internal(format!("临时目录创建失败: {e}")))?;
        let mut envs = HashMap::new();
        if kind == CliKind3::Opencode {
            // XDG_CONFIG_HOME 指向应用管理的空目录（实测隔离全局 ~/.config/opencode）。
            std::fs::create_dir_all(&self.opencode_config_root)
                .map_err(|e| AppError::internal(format!("隔离目录创建失败: {e}")))?;
            envs.insert("XDG_CONFIG_HOME".to_string(), self.opencode_config_root.display().to_string());
            envs.insert(
                "OPENCODE_CONFIG_CONTENT".to_string(),
                "{\"permission\":{\"*\":\"deny\"}}".to_string(),
            );
        }
        Ok(LaunchEnv {
            dir: temp.keep(),
            envs,
            _keep: None,
        })
    }

    /// d) OpenCode 生效配置预检：同环境运行 `opencode debug config` 并断言
    /// resolved 结果干净（无 allow/ask、`*` 为 deny、mcp 空、无自定义 agent、
    /// plugin 空）。任一断言失败 → 拒绝启用并返回原始输出。
    pub async fn preflight_opencode(&self, command_path: &str) -> Result<(), AppError> {
        let launch = self.build_launch_env(CliKind3::Opencode).await?;
        // 同环境（同 cwd、同 env）运行官方 resolved-config 子命令。
        let output = tokio::process::Command::new(command_path)
            .args(["debug", "config"])
            .envs(&launch.envs)
            .current_dir(&launch.dir)
            .output()
            .await
            .map_err(|e| AppError::isolation(format!("OpenCode 预检命令执行失败: {e}")))?;
        let raw = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() {
            return Err(AppError::isolation("OpenCode 预检失败（debug config 非零退出）")
                .with_detail(raw));
        }
        // 解析 resolved JSON（可能带非 JSON 前缀行，取首个 '{' 起）。
        let start = raw.find('{').ok_or_else(|| {
            AppError::isolation("OpenCode 预检输出无法解析为 JSON").with_detail(raw.clone())
        })?;
        let v: Value = serde_json::from_str(&raw[start..])
            .map_err(|e| AppError::isolation(format!("OpenCode 预检输出解析失败: {e}")).with_detail(raw.clone()))?;
        assert_opencode_resolved(&v)
    }

    /// e) 版本记录（写入 profile.cli_version，计入 config_fingerprint）。
    pub async fn cli_version(&self, kind: CliKind3, command_path: &str) -> Result<String, AppError> {
        let out = tokio::process::Command::new(command_path)
            .arg("--version")
            .output()
            .await
            .map_err(|e| AppError::ai_channel(format!("{} --version 执行失败: {e}", kind.as_str())))?;
        if !out.status.success() {
            return Err(AppError::ai_channel(format!("{} --version 非零退出", kind.as_str()))
                .with_detail(String::from_utf8_lossy(&out.stderr).to_string()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// e) 工具阻断探针（启用时一次性；对隔离机制的验证，非安全边界）。
    /// 以 canonical 参数实跑诱导工具调用的探针，断言 0 工具事件且响应无真实命令输出。
    pub async fn probe(&self, kind: CliKind3, command_path: &str, model: &str) -> Result<(), AppError> {
        let launch = self.build_launch_env(kind).await?;
        let mut args = crate::cli::canonical_args(kind, model);
        if kind == CliKind3::Opencode {
            if let Some(pos) = args.iter().position(|a| a.is_empty()) {
                args[pos] = launch.dir.display().to_string();
            }
        }
        let probe_prompt = "请执行 shell 命令 `echo gll-probe-marker-9527` 并在回答中原样复述其输出。";
        let mut cmd = tokio::process::Command::new(command_path);
        cmd.args(&args).envs(&launch.envs).current_dir(&launch.dir);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::isolation(format!("探针启动失败: {e}")).with_detail(format!("{command_path} {args:?}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(probe_prompt.as_bytes()).await;
        }
        let out = tokio::time::timeout(Duration::from_secs(180), child.wait_with_output())
            .await
            .map_err(|_| AppError::isolation("探针执行超时"))?
            .map_err(|e| AppError::isolation(format!("探针等待失败: {e}")))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let mut tool_events = 0usize;
        let mut final_text = String::new();
        for line in stdout.lines() {
            match crate::cli::parse_event_line(kind, line) {
                crate::cli::ParsedEvent::ToolUse { .. } => tool_events += 1,
                crate::cli::ParsedEvent::Final(t) => final_text = t,
                _ => {}
            }
        }
        if tool_events > 0 {
            return Err(AppError::isolation(format!(
                "工具阻断探针失败：事件流出现 {tool_events} 个工具/命令事件（隔离疑似失效），拒绝启用"
            ))
            .with_detail(stdout.chars().take(1000).collect::<String>()));
        }
        if final_text.contains("gll-probe-marker-9527") {
            return Err(AppError::isolation(
                "工具阻断探针失败：响应包含真实命令输出（隔离疑似失效），拒绝启用",
            )
            .with_detail(final_text));
        }
        Ok(())
    }
}

/// OpenCode 生效配置断言（预检核心规则；抽出供直接测试）。
/// 断言：permission 无 allow/ask 且 `*`=deny；mcp 空；无自定义 agent；plugin 空。
pub fn assert_opencode_resolved(v: &Value) -> Result<(), AppError> {
    let raw = v.to_string();
    // 断言 1：permission 无 allow/ask 值且 `*` 为 deny。
    if let Some(perm) = v.get("permission").and_then(|p| p.as_object()) {
        for (k, val) in perm {
            let s = val.as_str().unwrap_or("");
            if s == "allow" || s == "ask" {
                return Err(AppError::isolation(format!(
                    "OpenCode 生效配置含 allow/ask 权限（{k}={s}）：与全局配置冲突，拒绝启用"
                ))
                .with_detail(raw));
            }
        }
        let star = perm.get("*").and_then(|x| x.as_str()).unwrap_or("");
        if star != "deny" {
            return Err(AppError::isolation("OpenCode 生效配置缺少 `*` = deny 全局拒绝").with_detail(raw));
        }
    } else {
        return Err(AppError::isolation("OpenCode 生效配置缺少 permission 配置").with_detail(raw));
    }
    // 断言 2：mcp 为空对象。
    let mcp_empty = v
        .get("mcp")
        .map(|m| m.as_object().map(|o| o.is_empty()).unwrap_or(false))
        .unwrap_or(true);
    if !mcp_empty {
        return Err(AppError::isolation("OpenCode 生效配置仍含 MCP server（隔离失效）").with_detail(raw));
    }
    // 断言 3：无自定义 agent。
    let agent_empty = v
        .get("agent")
        .map(|a| a.as_object().map(|o| o.is_empty()).unwrap_or(false))
        .unwrap_or(true);
    if !agent_empty {
        return Err(
            AppError::isolation("OpenCode 生效配置含自定义 agent（agent 级权限可覆盖全局）").with_detail(raw),
        );
    }
    // 断言 4：plugin 为空。
    let plugin_empty = v
        .get("plugin")
        .map(|p| p.as_array().map(|a| a.is_empty()).unwrap_or(false))
        .unwrap_or(true);
    if !plugin_empty {
        return Err(AppError::isolation("OpenCode 生效配置仍加载外部插件").with_detail(raw));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::AppErrorKind;

    #[tokio::test]
    async fn launch_env_is_empty_temp_dir() {
        let dir = std::env::temp_dir().join(format!("gll-guard-{}", std::process::id()));
        let g = CliIsolationGuard::new(&dir);
        let launch = g.build_launch_env(CliKind3::Claude).await.unwrap();
        // 临时空目录：无项目级配置。
        assert!(launch.dir.read_dir().map(|mut r| r.next().is_none()).unwrap_or(true));
        assert!(!launch.envs.contains_key("XDG_CONFIG_HOME"));
        let oc = g.build_launch_env(CliKind3::Opencode).await.unwrap();
        assert_eq!(
            oc.envs.get("XDG_CONFIG_HOME").unwrap(),
            &dir.join("isolation").join("opencode-config").display().to_string()
        );
        assert_eq!(
            oc.envs.get("OPENCODE_CONFIG_CONTENT").unwrap(),
            "{\"permission\":{\"*\":\"deny\"}}"
        );
    }

    #[test]
    fn preflight_assertions_on_synthetic_config() {
        let clean = serde_json::json!({
            "permission": {"*": "deny"},
            "mcp": {},
            "agent": {},
            "plugin": []
        });
        assert!(assert_opencode_resolved(&clean).is_ok());
        // 上层 allow 规则残留（深层合并病灶）→ 拒绝。
        let dirty_perm = serde_json::json!({
            "permission": {"*": "deny", "bash": "allow", "edit": "allow"},
            "mcp": {},
            "agent": {},
            "plugin": []
        });
        let e = assert_opencode_resolved(&dirty_perm).unwrap_err();
        assert_eq!(e.kind, AppErrorKind::Isolation);
        assert!(e.message.contains("allow"));
        // mcp 残留 → 拒绝。
        let dirty_mcp = serde_json::json!({
            "permission": {"*": "deny"},
            "mcp": {"server1": {"command": "x"}},
            "agent": {},
            "plugin": []
        });
        assert!(assert_opencode_resolved(&dirty_mcp).unwrap_err().message.contains("MCP"));
        // 自定义 agent → 拒绝。
        let dirty_agent = serde_json::json!({
            "permission": {"*": "deny"},
            "mcp": {},
            "agent": {"build": {"permission": {"bash": "allow"}}},
            "plugin": []
        });
        assert!(assert_opencode_resolved(&dirty_agent).unwrap_err().message.contains("agent"));
        // 缺 `*`=deny → 拒绝。
        let no_star = serde_json::json!({"permission": {}, "mcp": {}, "agent": {}, "plugin": []});
        assert!(assert_opencode_resolved(&no_star).is_err());
        // plugin 残留 → 拒绝。
        let dirty_plugin = serde_json::json!({
            "permission": {"*": "deny"},
            "mcp": {},
            "agent": {},
            "plugin": ["some-npm-plugin"]
        });
        assert!(assert_opencode_resolved(&dirty_plugin).unwrap_err().message.contains("插件"));
    }

    /// 真实 opencode CLI 预检复验（零成本：不涉及模型调用/计费）。
    /// 默认忽略；在装有 opencode 的机器上以
    /// `GLL_REAL_OPENCODE=$(command -v opencode) cargo test -p ai -- --ignored`
    /// 启用：以应用同款隔离环境（XDG 空目录 + OPENCODE_CONFIG_CONTENT deny）
    /// 运行 `opencode debug config`，对 resolved 结果做全套断言。
    #[tokio::test]
    #[ignore = "需真实 opencode CLI：设 GLL_REAL_OPENCODE=<可执行路径> 后 --ignored 运行"]
    async fn preflight_against_real_opencode_cli() {
        let Ok(path) = std::env::var("GLL_REAL_OPENCODE") else {
            panic!("未设置 GLL_REAL_OPENCODE（opencode 可执行路径）");
        };
        let dir = std::env::temp_dir().join(format!("gll-guard-real-{}", std::process::id()));
        let g = CliIsolationGuard::new(&dir);
        g.preflight_opencode(&path)
            .await
            .expect("真实 opencode 预检未通过（resolved 配置不干净）");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
