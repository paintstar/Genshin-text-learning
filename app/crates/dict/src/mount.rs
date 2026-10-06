//! DictMount — 词典只读挂载器（技术设计 §4.2-4【修订·再审议1】/ 架构 §3.2、
//! 遗留问题 2：ATTACH URI 路径构造规则）。
//!
//! 构造规则：对资源路径先做规范化，再按 SQLite URI 规则生成 `file:` URI
//! （`?`→`%3f`、`#`→`%23`、空格→`%20` 等百分号编码；Windows 反斜杠转正斜杠
//! 并按盘符形态处理；相对路径先以当前工作目录绝对化——SQLite URI 要求 `file://`
//! 之后为 authority（仅允许空串或 localhost），相对路径首段会被当作 authority
//! 拒绝报 `invalid uri authority`，故统一产出 `file:///绝对路径` 形态）；
//! 挂载语句 `ATTACH DATABASE 'file:{uri}?mode=ro' AS dict_ro`；
//! 挂载后执行只读断言（对挂载库的写入尝试必败）；资源缺失显式报错。

use rusqlite::Connection;
use shared::AppError;
use std::path::{Component, Path};

/// 对路径段做 SQLite URI 百分号编码（保留 `/`、字母数字与常见安全字符）。
fn percent_encode(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for ch in path.chars() {
        match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '/' | '-' | '_' | '.' | '~' => out.push(ch),
            _ => {
                let mut buf = [0u8; 4];
                for b in ch.encode_utf8(&mut buf).as_bytes() {
                    out.push_str(&format!("%{b:02x}"));
                }
            }
        }
    }
    out
}

/// 构造 ATTACH 用的 `file:` URI（不含 mode 参数）。
///
/// 相对路径以当前工作目录绝对化后再构造（读取 cwd 失败显式报错，不猜测）：
/// SQLite URI 规则要求 `file://` 后为 authority（仅允许空串或 localhost）再接
/// `/` 起始的 path，`file://{相对路径}` 会被判 `invalid uri authority`。
/// Windows 盘符形态统一输出三斜杠 `file:///C:/…`。
pub fn build_attach_uri(path: &Path) -> Result<String, AppError> {
    // 盘符形态（含非 Windows 平台按字面传入的盘符串）直接产出三斜杠 URI：
    // 盘符路径在 Windows 上本就是绝对路径；此判定先于相对路径绝对化，
    // 保证跨平台传入盘符串时输出形态稳定、不随 cwd 变化。
    let raw = render_normalized(path);
    if is_windows_drive(&raw) {
        return Ok(format!("file:///{}", percent_encode(&raw)));
    }
    // 相对路径绝对化（绝对路径保持原样）。
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| {
                AppError::resource_missing(format!(
                    "词典路径无法绝对化（读取当前工作目录失败: {e}）"
                ))
            })?
            .join(path)
    };
    let s = render_normalized(&absolute);
    if is_windows_drive(&s) {
        // Windows 上与 cwd 拼接后带盘符前缀的形态：`C:/…` → `file:///C:/…`。
        Ok(format!("file:///{}", percent_encode(&s)))
    } else {
        // 绝对化后必以 `/` 起始：`file:///…`（空 authority + path）。
        Ok(format!("file://{}", percent_encode(&s)))
    }
}

/// 词法规范化：消除 `./`、重复分隔符（保留 Windows 盘符形态），反斜杠转正斜杠。
fn render_normalized(path: &Path) -> String {
    let mut normalized = PathBufLike::default();
    for comp in path.components() {
        match comp {
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => {
                normalized.push(comp)
            }
            Component::CurDir => {}
            Component::ParentDir => normalized.pop(),
        }
    }
    normalized.render().replace('\\', "/")
}

fn is_windows_drive(s: &str) -> bool {
    s.len() >= 2 && s.as_bytes()[1] == b':' && s.as_bytes()[0].is_ascii_alphabetic()
}

#[derive(Default)]
struct PathBufLike {
    parts: Vec<String>,
    absolute: bool,
    prefix: Option<String>,
}

impl PathBufLike {
    fn push(&mut self, comp: Component) {
        match comp {
            Component::RootDir => self.absolute = true,
            Component::Prefix(p) => self.prefix = Some(p.as_os_str().to_string_lossy().to_string()),
            Component::Normal(n) => self.parts.push(n.to_string_lossy().to_string()),
            _ => {}
        }
    }
    fn pop(&mut self) {
        self.parts.pop();
    }
    fn render(&self) -> String {
        let mut s = String::new();
        if let Some(p) = &self.prefix {
            s.push_str(p);
            s.push('/');
        } else if self.absolute {
            s.push('/');
        }
        s.push_str(&self.parts.join("/"));
        s
    }
}

pub struct DictMount;

impl DictMount {
    /// 挂载 dict.db 到连接（只读）。资源缺失 → ResourceMissing（不静默降级）。
    pub fn mount(conn: &Connection, dict_db_path: &Path) -> Result<(), AppError> {
        if !dict_db_path.exists() {
            return Err(AppError::resource_missing(format!(
                "词典资源缺失: {}（安装可能损坏；词典功能将停用）",
                dict_db_path.display()
            )));
        }
        let uri = build_attach_uri(dict_db_path)?;
        let sql = format!("ATTACH DATABASE '{uri}?mode=ro' AS dict_ro");
        conn.execute_batch(&sql)
            .map_err(|e| AppError::resource_missing(format!("词典挂载失败: {e}")).with_detail(sql))?;
        // 只读断言：对挂载库的写入尝试必败。
        let write_probe = conn.execute_batch("CREATE TABLE dict_ro.__ro_probe(x)");
        match write_probe {
            Ok(()) => {
                // 不应发生（mode=ro 下写入必败）；防御性 detach 再报错。
                let _ = conn.execute_batch("DETACH DATABASE dict_ro");
                return Err(AppError::resource_missing(
                    "词典挂载只读断言失败：挂载库意外可写（URI 构造错误）",
                ));
            }
            Err(_) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_dict_db(path: &Path) {
        let c = Connection::open(path).unwrap();
        c.execute_batch(
            "CREATE TABLE dict_entry(entry_id INTEGER PRIMARY KEY, source TEXT, headword TEXT, reading_kana TEXT, pos_json TEXT, common INTEGER);
             CREATE TABLE dict_index(form TEXT, form_kind TEXT, entry_id INTEGER);
             CREATE TABLE dict_gloss(entry_id INTEGER, lang TEXT, gloss_json TEXT, PRIMARY KEY(entry_id, lang));
             INSERT INTO dict_index VALUES ('テスト', 'kana', 1);
             INSERT INTO dict_entry VALUES (1, 'zhwiktionary', 'テスト', 'テスト', '[\"名詞\"]', 1);
             INSERT INTO dict_gloss VALUES (1, 'zh', '[\"测试；试验\"]');",
        )
        .unwrap();
    }

    #[test]
    fn uri_encoding_special_chars() {
        // ? # 空格等特殊字符路径。
        let p = Path::new("/opt/my app/dic?t.db#1");
        let uri = build_attach_uri(p).unwrap();
        assert!(uri.contains("%20"), "space encoded: {uri}");
        assert!(uri.contains("%3f") || uri.contains("%3F"), "question encoded: {uri}");
        assert!(uri.contains("%23"), "hash encoded: {uri}");
        assert!(uri.starts_with("file://"));
    }

    #[test]
    fn windows_drive_form() {
        let p = Path::new("C:\\Program Files\\app\\dict.db");
        let uri = build_attach_uri(p).unwrap();
        // 盘符形态：file:///C...（冒号可被编码为 %3a——SQLite URI 百分号解码后等价）。
        assert!(uri.starts_with("file:///C"), "windows drive: {uri}");
        assert!(uri.contains("/Program%20Files/"), "space encoded: {uri}");
    }

    #[test]
    fn relative_path_absolutized_and_mounts() {
        // 相对路径输入 → 产出以 `file:///` 起始的绝对 URI 且可挂载。
        // （修复前 else 分支产出 `file://{相对路径}`，SQLite 判 `invalid uri authority`
        // 拒绝挂载。相对输入以测试二进制 cwd = crate 根（crates/dict）为基准指向
        // `../../target`，不改动进程 cwd，避免与并行用例互扰。）
        let dir = PathBuf::from("../../target")
            .join(format!("gll-dict-rel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dict_path = dir.join("dict rel.db"); // 含空格，兼测编码。
        make_dict_db(&dict_path);

        let rel_str = format!(
            "../../target/gll-dict-rel-{}/dict rel.db",
            std::process::id()
        );
        let rel = Path::new(&rel_str);
        let uri = build_attach_uri(rel).unwrap();
        assert!(uri.starts_with("file:///"), "absolutized: {uri}");
        // `../../` 被词法解析进绝对路径（不再作为相对段出现）。
        assert!(uri.contains("/target/gll-dict-rel-"), "resolved: {uri}");
        assert!(uri.contains("%20"), "space encoded: {uri}");

        // 可挂载：真实 ATTACH（mode=ro）+ 只读断言 + 跨库读取。
        let conn = Connection::open(dir.join("app.db")).unwrap();
        DictMount::mount(&conn, rel).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM dict_ro.dict_entry", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mounts_readonly_and_write_fails() {
        let dir = std::env::temp_dir().join(format!("gll-dict-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dict_path = dir.join("dict db v1.db"); // 含空格。
        make_dict_db(&dict_path);

        let conn = Connection::open(dir.join("app.db")).unwrap();
        DictMount::mount(&conn, &dict_path).unwrap();
        // 跨库读取成功。
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM dict_ro.dict_entry", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        // 对挂载库的写入必败。
        assert!(conn.execute("INSERT INTO dict_ro.dict_entry VALUES (9,'x','y',NULL,NULL,0)", []).is_err());
        assert!(conn.execute_batch("CREATE TABLE dict_ro.t9(x)").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_resource_reports_clearly() {
        let conn = Connection::open_in_memory().unwrap();
        let e = DictMount::mount(&conn, Path::new("/nonexistent/dict.db")).unwrap_err();
        assert_eq!(e.kind, shared::AppErrorKind::ResourceMissing);
        assert!(e.message.contains("词典资源缺失"));
    }
}
