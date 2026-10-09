//! CLI 启动环境：在临时目录运行，保留用户的模型服务与认证配置，阻止工具调用。

use crate::cli::CliKind3;
use serde_json::{json, Value};
use shared::AppError;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;

pub struct LaunchEnv {
    pub command: String,
    pub dir: PathBuf,
    pub envs: HashMap<String, String>,
    pub default_model: Option<String>,
    isolated_opencode: bool,
    _keep: tempfile::TempDir,
}

impl LaunchEnv {
    pub fn apply(&self, command: &mut Command) {
        if self.isolated_opencode {
            // 用户指定的配置已在读取阶段解析；运行时只加载服务配置，不再加载插件等来源。
            command
                .env_remove("OPENCODE_CONFIG")
                .env_remove("OPENCODE_CONFIG_DIR");
        }
        command
            .envs(&self.envs)
            .current_dir(&self.dir)
            .kill_on_drop(true);
    }
}

#[derive(Clone, Default)]
pub struct CliIsolationGuard;

impl CliIsolationGuard {
    pub fn new(_app_data_dir: &std::path::Path) -> Self {
        Self
    }

    pub async fn build_launch_env(&self, kind: CliKind3) -> Result<LaunchEnv, AppError> {
        let temp = tempfile::tempdir().map_err(|_| AppError::internal("临时目录创建失败"))?;
        let mut envs = HashMap::new();
        if kind == CliKind3::Opencode {
            envs.insert("OPENCODE_DISABLE_PROJECT_CONFIG".into(), "true".into());
        }
        Ok(LaunchEnv {
            command: kind.as_str().to_string(),
            dir: temp.path().to_path_buf(),
            envs,
            default_model: None,
            isolated_opencode: false,
            _keep: temp,
        })
    }

    pub async fn prepare_launch(
        &self,
        kind: CliKind3,
        command_path: &str,
    ) -> Result<LaunchEnv, AppError> {
        let mut launch = self.build_launch_env(kind).await?;
        let executable = crate::executable::resolve(kind, Some(command_path)).await?;
        launch.command = executable.path;
        if let Some(path) = executable.search_path {
            launch
                .envs
                .insert("PATH".into(), path.to_string_lossy().into_owned());
        }
        let command_path = launch.command.clone();
        if kind != CliKind3::Opencode {
            return Ok(launch);
        }
        // 用用户现有环境读取生效配置，不打印或保存含服务凭据的原始输出。
        let user_config = read_opencode_config(&command_path, &launch).await?;
        let safe = opencode_service_config(&user_config);
        launch.default_model = safe.get("model").and_then(Value::as_str).map(str::to_owned);
        let config_root = launch.dir.join("config");
        std::fs::create_dir(&config_root)
            .map_err(|_| AppError::internal("临时配置目录创建失败"))?;
        launch
            .envs
            .insert("XDG_CONFIG_HOME".into(), config_root.display().to_string());
        launch
            .envs
            .insert("OPENCODE_CONFIG_CONTENT".into(), safe.to_string());
        launch.isolated_opencode = true;
        // 检查实际生效配置，避免受系统管理配置等更高优先级来源影响。
        assert_opencode_resolved(&read_opencode_config(&command_path, &launch).await?)?;
        Ok(launch)
    }

    pub async fn preflight_opencode(&self, command_path: &str) -> Result<(), AppError> {
        self.prepare_launch(CliKind3::Opencode, command_path)
            .await
            .map(|_| ())
    }

    pub async fn cli_version(
        &self,
        kind: CliKind3,
        command_path: &str,
    ) -> Result<String, AppError> {
        let executable = crate::executable::resolve(kind, Some(command_path)).await?;
        let mut command = Command::new(&executable.path);
        if let Some(path) = executable.search_path {
            command.env("PATH", path);
        }
        let output = command_output(command.arg("--version"), 15).await?;
        if !output.status.success() {
            return Err(AppError::ai_channel(format!(
                "{} 版本检查失败，请检查命令路径",
                kind.as_str()
            )));
        }
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if version.is_empty() {
            return Err(AppError::ai_channel("CLI 未返回版本信息"));
        }
        Ok(version)
    }

    pub async fn opencode_models(&self, command_path: &str) -> Result<Vec<String>, AppError> {
        let launch = self
            .prepare_launch(CliKind3::Opencode, command_path)
            .await?;
        models_in_launch(&launch.command, &launch).await
    }

    pub async fn resolve_opencode_model(
        &self,
        _command_path: &str,
        model: &str,
        launch: &LaunchEnv,
    ) -> Result<String, AppError> {
        let model = if model.trim().is_empty() {
            launch.default_model.as_deref().unwrap_or("")
        } else {
            model.trim()
        };
        if model.is_empty() {
            return Err(AppError::invalid_param(
                "OpenCode 未设置默认模型，请读取模型列表后选择模型",
            ));
        }
        if model.contains('/') {
            return Ok(model.to_string());
        }
        let matches: Vec<_> = models_in_launch(&launch.command, launch)
            .await?
            .into_iter()
            .filter(|id| id.rsplit_once('/').is_some_and(|(_, name)| name == model))
            .collect();
        match matches.as_slice() {
            [id] => Ok(id.clone()),
            [] => Err(AppError::invalid_param(format!(
                "OpenCode 没有找到模型「{model}」，请读取模型列表并选择完整标识"
            ))),
            _ => Err(AppError::invalid_param(format!(
                "模型「{model}」对应多个服务，请选择完整标识：{}",
                matches.join("、")
            ))),
        }
    }
}

/// 只转交模型服务配置。登录凭据仍由 OpenCode 自己从用户数据目录读取。
fn opencode_service_config(source: &Value) -> Value {
    let mut safe = json!({"permission": {"*": "deny"}, "mcp": {}, "agent": {}, "plugin": [], "instructions": [], "share": "disabled", "autoupdate": false});
    for key in [
        "provider",
        "model",
        "small_model",
        "enabled_providers",
        "disabled_providers",
    ] {
        if let Some(value) = source.get(key) {
            safe[key] = value.clone();
        }
    }
    safe
}

async fn read_opencode_config(command_path: &str, launch: &LaunchEnv) -> Result<Value, AppError> {
    let mut command = Command::new(command_path);
    command.args(["debug", "config", "--pure"]);
    launch.apply(&mut command);
    let output = command_output(&mut command, 30).await?;
    if !output.status.success() {
        return Err(AppError::ai_channel(
            "OpenCode 配置读取失败，请确认命令路径及 CLI 配置有效",
        ));
    }
    let raw = String::from_utf8_lossy(&output.stdout);
    let start = raw
        .find('{')
        .ok_or_else(|| AppError::ai_channel("OpenCode 未返回可读取的配置"))?;
    serde_json::from_str(&raw[start..])
        .map_err(|_| AppError::ai_channel("OpenCode 配置输出格式不受支持，请检查 CLI 版本"))
}

async fn models_in_launch(command_path: &str, launch: &LaunchEnv) -> Result<Vec<String>, AppError> {
    let mut command = Command::new(command_path);
    command.args(["models", "--pure"]);
    launch.apply(&mut command);
    let output = command_output(&mut command, 30).await?;
    if !output.status.success() {
        return Err(AppError::ai_channel(
            "读取 OpenCode 模型列表失败，请先在 OpenCode 中配置模型服务",
        ));
    }
    let mut models: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| line.contains('/') && !line.contains(char::is_whitespace))
        .map(str::to_owned)
        .collect();
    models.sort();
    models.dedup();
    Ok(models)
}

pub(crate) async fn command_output(
    command: &mut Command,
    timeout_secs: u64,
) -> Result<std::process::Output, AppError> {
    command.kill_on_drop(true);
    tokio::time::timeout(Duration::from_secs(timeout_secs), command.output())
        .await
        .map_err(|_| AppError::ai_channel("CLI 检查超时，已停止本次检查"))?
        .map_err(|_| AppError::ai_channel("CLI 无法启动，请检查命令或可执行文件路径"))
}

pub fn assert_opencode_resolved(v: &Value) -> Result<(), AppError> {
    let all_denied = v
        .get("permission")
        .and_then(Value::as_object)
        .is_some_and(|permission| {
            permission.get("*").and_then(Value::as_str) == Some("deny")
                && permission
                    .values()
                    .all(|value| value.as_str() == Some("deny"))
        });
    if !all_denied {
        return Err(AppError::isolation(
            "OpenCode 未能关闭工具权限，请检查系统管理配置",
        ));
    }
    for key in ["mcp", "agent"] {
        if v.get(key)
            .is_some_and(|value| !value.as_object().is_some_and(|object| object.is_empty()))
        {
            return Err(AppError::isolation(
                "OpenCode 仍加载工具或自定义代理配置，无法用于语言助手",
            ));
        }
    }
    if v.get("plugin")
        .is_some_and(|value| !value.as_array().is_some_and(|array| array.is_empty()))
    {
        return Err(AppError::isolation(
            "OpenCode 仍加载外部插件，无法用于语言助手",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn temporary_directory_is_removed_after_launch() {
        let guard = CliIsolationGuard;
        let launch = guard.build_launch_env(CliKind3::Claude).await.unwrap();
        let path = launch.dir.clone();
        assert!(path.exists());
        drop(launch);
        assert!(!path.exists());
    }

    #[test]
    fn user_model_services_survive_tool_isolation() {
        let source = json!({"provider": {"custom": {"models": {"model-a": {"name": "模型 A"}}, "options": {"apiKey": "test-secret"}}}, "model": "custom/model-a", "permission": {"bash": "allow"}, "agent": {"custom": {}}, "mcp": {"server": {}}, "plugin": ["plugin"]});
        let safe = opencode_service_config(&source);
        assert_eq!(safe["provider"], source["provider"]);
        assert_eq!(safe["model"], source["model"]);
        assert_opencode_resolved(&safe).unwrap();
        assert!(assert_opencode_resolved(&source).is_err());
    }

    #[tokio::test]
    #[ignore = "需本机 OpenCode：设置 GLL_REAL_OPENCODE 后运行"]
    async fn preflight_against_real_opencode_cli() {
        let path = std::env::var("GLL_REAL_OPENCODE").expect("需设置 GLL_REAL_OPENCODE");
        CliIsolationGuard.preflight_opencode(&path).await.unwrap();
    }
}
