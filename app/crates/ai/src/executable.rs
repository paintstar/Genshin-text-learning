//! CLI 定位：先用应用环境，再读取用户登录 shell 的 PATH，最后检查常见安装目录。

use crate::cli::CliKind3;
use shared::AppError;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

pub struct CliExecutable {
    pub path: String,
    pub search_path: Option<OsString>,
}

pub async fn resolve(kind: CliKind3, configured: Option<&str>) -> Result<CliExecutable, AppError> {
    let name = configured
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| kind.as_str());
    let home = home_directory();
    // 显式文件路径始终优先；错误路径不能悄悄切换到另一套安装。
    if Path::new(name).is_absolute() || name.contains('/') || name.contains('\\') {
        let path = if let Some(rest) = name.strip_prefix("~/") {
            home.as_ref()
                .map(|home| home.join(rest))
                .unwrap_or_else(|| PathBuf::from(name))
        } else {
            PathBuf::from(name)
        };
        let path = executable_file(&path)
            .ok_or_else(|| AppError::ai_channel("所选 CLI 文件不存在或不可执行，请重新选择文件"))?;
        let search_path = if is_script(&path) {
            login_shell_path(name, home.as_deref())
                .await
                .map(|(_, path)| path)
        } else {
            None
        };
        return Ok(CliExecutable {
            path: path.to_string_lossy().into_owned(),
            search_path,
        });
    }
    if let Some(path) = std::env::var_os("PATH").and_then(|path| find_in_path(name, &path)) {
        let search_path = if is_script(&path) {
            login_shell_path(name, home.as_deref())
                .await
                .map(|(_, path)| path)
        } else {
            None
        };
        return Ok(CliExecutable {
            path: path.to_string_lossy().into_owned(),
            search_path,
        });
    }
    if let Some((candidate, search_path)) = login_shell_path(name, home.as_deref()).await {
        if let Some(path) =
            executable_file(Path::new(&candidate)).or_else(|| find_in_path(name, &search_path))
        {
            return Ok(CliExecutable {
                path: path.to_string_lossy().into_owned(),
                search_path: Some(search_path),
            });
        }
    }
    for directory in installation_directories(home.as_deref()) {
        if let Some(path) = find_in_directory(name, &directory) {
            let mut directories = vec![directory];
            if let Some(current) = std::env::var_os("PATH") {
                directories.extend(std::env::split_paths(&current));
            }
            return Ok(CliExecutable {
                path: path.to_string_lossy().into_owned(),
                search_path: std::env::join_paths(directories).ok(),
            });
        }
    }
    Err(AppError::ai_channel(format!(
        "未找到 {}，请先安装对应 CLI，或点击「选择文件」指定程序",
        kind.as_str()
    )))
}

fn home_directory() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn executable_file(path: &Path) -> Option<PathBuf> {
    let metadata = path.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    // fnm 的临时 shell 路径会在 shell 退出后删除，返回真实文件位置。
    std::fs::canonicalize(path).ok()
}

fn is_script(path: &Path) -> bool {
    use std::io::Read;
    let mut prefix = [0u8; 2];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut prefix))
        .is_ok()
        && prefix == *b"#!"
}

fn find_in_path(name: &str, search_path: &std::ffi::OsStr) -> Option<PathBuf> {
    std::env::split_paths(search_path)
        .filter(|directory| !directory.as_os_str().is_empty())
        .find_map(|directory| find_in_directory(name, &directory))
}

fn find_in_directory(name: &str, directory: &Path) -> Option<PathBuf> {
    if let Some(path) = executable_file(&directory.join(name)) {
        return Some(path);
    }
    #[cfg(windows)]
    {
        let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
        for extension in extensions.split(';') {
            if let Some(path) = executable_file(&directory.join(format!("{name}{extension}"))) {
                return Some(path);
            }
        }
    }
    None
}

async fn login_shell_path(name: &str, home: Option<&Path>) -> Option<(String, OsString)> {
    if cfg!(windows) {
        return None;
    }
    let shell = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/bin/sh"));
    // 命令名通过位置参数传入，不拼入 shell 代码；仅解析带标记的两项输出。
    let script = r#"printf '\036GLL_COMMAND:%s\037\n' "$(command -v -- "$1" 2>/dev/null)"; printf '\036GLL_PATH:%s\037\n' "$PATH""#;
    let mut command = Command::new(shell);
    command
        .args(["-lic", script, "gll-cli-detect", name])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    if let Some(home) = home {
        command.current_dir(home);
    }
    let output = tokio::time::timeout(Duration::from_secs(8), command.output())
        .await
        .ok()?
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let field = |marker: &str| {
        stdout
            .split_once(marker)?
            .1
            .split_once('\u{1f}')
            .map(|(value, _)| value.to_string())
    };
    let candidate = field("\u{1e}GLL_COMMAND:")?;
    let path = field("\u{1e}GLL_PATH:")?;
    Some((candidate, OsString::from(path)))
}

fn installation_directories(home: Option<&Path>) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    for variable in ["FNM_DIR", "NVM_DIR", "VOLTA_HOME", "APPDATA"] {
        if let Some(root) = std::env::var_os(variable).map(PathBuf::from) {
            directories.extend([
                root.join("bin"),
                root.join("aliases/default/bin"),
                root.join("npm"),
            ]);
        }
    }
    if let Some(home) = home {
        // 这些是安装工具的标准目录，没有绑定用户名、Node 版本或机器路径。
        for path in [
            ".local/bin",
            ".opencode/bin",
            ".volta/bin",
            ".npm-global/bin",
            ".local/share/fnm/aliases/default/bin",
            "Library/Application Support/fnm/aliases/default/bin",
        ] {
            directories.push(home.join(path));
        }
    }
    #[cfg(unix)]
    directories.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
    ]);
    directories
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::{fs::symlink, fs::PermissionsExt};

    #[tokio::test]
    async fn explicit_paths_require_executable_files_and_never_fall_back() {
        let temporary = tempfile::tempdir().unwrap();
        let file = temporary.path().join("local-cli");
        std::fs::write(&file, b"test executable").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(resolve(CliKind3::Opencode, file.to_str()).await.is_err());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            resolve(CliKind3::Opencode, file.to_str())
                .await
                .unwrap()
                .path,
            file.canonicalize().unwrap().to_string_lossy()
        );
        assert!(resolve(
            CliKind3::Opencode,
            temporary.path().join("missing").to_str()
        )
        .await
        .is_err());
    }

    #[test]
    fn path_search_returns_stable_target_instead_of_temporary_symlink() {
        let temporary = tempfile::tempdir().unwrap();
        let file = temporary.path().join("installed-cli");
        std::fs::write(&file, b"test executable").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).unwrap();
        let bin = temporary.path().join("shell-bin");
        std::fs::create_dir(&bin).unwrap();
        symlink(&file, bin.join("opencode")).unwrap();
        let path = std::env::join_paths([temporary.path().join("missing"), bin]).unwrap();
        assert_eq!(
            find_in_path("opencode", &path),
            Some(file.canonicalize().unwrap())
        );
    }
}
