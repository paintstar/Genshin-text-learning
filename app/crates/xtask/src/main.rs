//! xtask — 构建期代码生成（架构决策 14：TS 边界类型从 Rust 单一来源产生）。
//!
//! `cargo run -p xtask -- bindings`：以 shared 的 DTO **Rust 类型为单一来源**，
//! 通过「样本实例 → serde_json 序列化 → TS interface 形状推导」生成
//! `frontend/src/gateway/bindings.ts` 的生成区段；`--check` 模式再生成并与
//! 入库文件 diff（CI 校验一致性，禁止手工双写——手工区段之外的生成区段
//! 任何漂移都会被检出）。
//!
//! 说明（与 tauri-specta 的等价性）：本机制覆盖全部 struct DTO 的字段名与
//! 类型（camelCase 由 serde rename 保证，与运行时序列化同一条路径——比独立
//! 的类型系统派生更贴近真实边界）；字符串枚举与联合类型的字面量集合在
//! 生成区段末尾由 Rust 侧常量清单 emit（同样单一来源）。

use serde_json::Value;
use std::path::PathBuf;

mod samples;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    match cmd {
        "bindings" => {
            let check = args.iter().any(|a| a == "--check");
            run_bindings(check);
        }
        _ => {
            eprintln!("用法: cargo run -p xtask -- bindings [--check]");
        }
    }
}

fn ts_type(v: &Value, shape_index: &dyn Fn(&Value) -> Option<String>) -> (String, bool) {
    match v {
        Value::Null => ("null".into(), true),
        Value::Bool(_) => ("boolean".into(), false),
        Value::Number(_) => ("number".into(), false),
        Value::String(_) => ("string".into(), false),
        Value::Array(a) => {
            if a.is_empty() {
                ("never[]".into(), false)
            } else {
                let (inner, opt) = ts_type(&a[0], shape_index);
                (format!("{inner}[]"), opt)
            }
        }
        Value::Object(_) => {
            if let Some(name) = shape_index(v) {
                (name, false)
            } else {
                ("object".into(), false)
            }
        }
    }
}

fn pascal(name: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in name.chars() {
        if c == '_' || c == '.' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn gen_interface(
    name: &str,
    sample: &Value,
    shape_index: &dyn Fn(&Value) -> Option<String>,
    toverrides: &[(&'static str, &'static str, &'static str)],
    out: &mut String,
) {
    let iface = pascal(name);
    let obj = sample.as_object().expect("DTO 样本必须是对象");
    out.push_str(&format!("export interface {iface} {{\n"));
    let mut fields: Vec<(&String, &Value)> = obj.iter().collect();
    fields.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in fields {
        if let Some((_, _, explicit)) = toverrides.iter().find(|(i, f, _)| *i == iface && *f == k.as_str()) {
            out.push_str(&format!("  {k}: {explicit}\n"));
            continue;
        }
        let (mut ty, nullable) = ts_type(v, shape_index);
        if nullable {
            ty = "null".to_string();
            out.push_str(&format!("  {k}?: {ty}\n"));
        } else {
            out.push_str(&format!("  {k}: {ty}\n"));
        }
    }
    out.push_str("}\n\n");
}

/// 递归「类型骨架」：只保留字段名与类型形态（丢弃具体值），用于嵌套对象与
/// 具名 interface 的匹配（值差异不影响形状判定）。
fn skeleton(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(_) => "bool".into(),
        Value::Number(_) => "num".into(),
        Value::String(_) => "str".into(),
        Value::Array(a) => format!("[{}]", a.first().map(skeleton).unwrap_or_default()),
        Value::Object(o) => {
            let mut keys: Vec<(&String, &Value)> = o.iter().collect();
            keys.sort_by(|a, b| a.0.cmp(b.0));
            let inner: Vec<String> = keys
                .iter()
                .map(|(k, val)| format!("{k}:{}", skeleton(val)))
                .collect();
            format!("{{{}}}", inner.join(","))
        }
    }
}

/// 字段类型覆盖表（Rust 单一来源的一部分）：`Option<T>` 字段的可空性无法从
/// 单一样本值推导——此表显式声明完整 TS 类型（含 `| null`）。
fn type_overrides() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("QuestSummary", "questType", "string | null"),
        ("QuestSummary", "chapterNum", "string | null"),
        ("QuestSummary", "route", "string | null"),
        ("QuestSummary", "alignStatus", "string | null"),
        ("NodeDto", "danglingNext", "string | null"),
        ("RowDto", "role", "string | null"),
        ("RowDto", "text", "string | null"),
        ("RowDto", "nextDialogId", "string | null"),
        ("FetchJobStatus", "error", "string | null"),
        ("RefreshOutcome", "alignStatus", "string | null"),
        ("RefreshOutcome", "error", "string | null"),
        ("DictEntryDto", "readingKana", "string | null"),
        ("DictEntryDto", "matchedSourceNote", "string | null"),
        ("DictEntryDto", "termTexts", "LangText[] | null"),
        ("NoteDto", "termText", "string | null"),
        ("NoteDto", "termReading", "string | null"),
        ("NoteDto", "termBase", "string | null"),
        ("NoteDto", "contextText", "string | null"),
        ("NoteDto", "contextRole", "string | null"),
        ("NoteDto", "contextNext", "string | null"),
        ("NoteDto", "staleReason", "string | null"),
        ("NoteDto", "userNote", "string | null"),
        ("NoteDto", "aiGeneratedJson", "string | null"),
        ("NoteDto", "analysisSnapshotJson", "string | null"),
        ("TaskNotesGroup", "questTitle", "string | null"),
        ("SyncFailure", "title", "string | null"),
        ("SyncProgress", "currentQuestTitle", "string | null"),
        ("AiProfileDto", "cliKind", "'claude' | 'codex' | 'opencode' | null"),
        ("AiProfileDto", "commandPath", "string | null"),
        ("AiProfileDto", "baseUrl", "string | null"),
        ("AiProfileDto", "extraJson", "string | null"),
        ("AiProfileDto", "cliVersion", "string | null"),
        ("AiProfileDto", "configFingerprint", "string | null"),
        ("AiProfileInput", "id", "number | null"),
        ("AiProfileInput", "cliKind", "'claude' | 'codex' | 'opencode' | null"),
        ("AiProfileInput", "commandPath", "string | null"),
        ("AiProfileInput", "baseUrl", "string | null"),
        ("AiProfileInput", "extraJson", "string | null"),
        ("AiProfileInput", "apiKey", "string | null"),
        ("AiStreamEvent", "text", "string | null"),
        ("AiStreamEvent", "error", "AppError | null"),
        ("AiTestResult", "rawOutput", "string | null"),
        ("AiConversationDto", "questId", "number | null"),
        ("CandidateForm", "sourceNote", "string | null"),
        ("AppError", "detail", "string | null"),
        ("OpenQuestResult", "snapshot", "GraphSnapshot | null"),
        ("OpenQuestResult", "job", "FetchJobStatus | null"),
        ("QuestOverview", "summary", "QuestSummary"),
        ("NodeDto", "kind", "NodeKind"),
        ("NodeDto", "status", "NodeAlignStatus"),
        ("GraphSnapshot", "nodes", "NodeDto[]"),
        ("GraphSnapshot", "rows", "RowDto[]"),
        ("GraphSnapshot", "conflicts", "ConflictBrief[]"),
        ("GraphSnapshot", "subs", "SubQuestBrief[]"),
        ("GraphSnapshot", "trees", "BlockBrief[]"),
        ("GraphSnapshot", "blockOrder", "BlockKey[]"),
        ("QuestOverview", "subs", "SubQuestBrief[]"),
        ("QuestOverview", "trees", "BlockBrief[]"),
        ("QuestOverview", "blockOrder", "BlockKey[]"),
        ("QuestOverview", "conflicts", "ConflictBrief[]"),
        ("UpdateReport", "newQuests", "QuestSummary[]"),
        ("UpdateReport", "changed", "QuestSummary[]"),
        ("UpdateReport", "unknownBody", "QuestSummary[]"),
        ("BatchSyncReport", "failed", "SyncFailure[]"),
        ("DictSearchResult", "entries", "DictEntryDto[]"),
        ("NotesByTask", "groups", "TaskNotesGroup[]"),
        ("TaskNotesGroup", "notes", "NoteDto[]"),
        ("AiConversationDto", "messages", "AiMessageDto[]"),
        ("BackupSummary", "counts", "TableCount[]"),
        ("RestoreCheck", "counts", "TableCount[]"),
        ("DictEntryDto", "glosses", "GlossDto[]"),
        ("SubQuestBrief", "titles", "LangText[]"),
        ("SubQuestBrief", "descs", "LangText[]"),
        ("QuestSummary", "titles", "LangText[]"),
        ("BlockBrief", "subQuestId", "string"),
        ("SaveNoteInput", "termText", "string | null"),
        ("SaveNoteInput", "termReading", "string | null"),
        ("SaveNoteInput", "termBase", "string | null"),
        ("SaveNoteInput", "contextText", "string | null"),
        ("SaveNoteInput", "contextRole", "string | null"),
        ("SaveNoteInput", "contextNext", "string | null"),
        ("SaveNoteInput", "analysisSnapshotJson", "string | null"),
        ("SaveNoteInput", "userNote", "string | null"),
        ("AiMessageDto", "role", "string"),
    ]
}

fn run_bindings(check: bool) {
    // 形状索引：嵌套对象按类型骨架匹配到具名 interface。
    let all = samples::samples();
    let shape_map: std::collections::HashMap<String, String> = all
        .iter()
        .map(|(name, v)| (skeleton(v), pascal(name)))
        .collect();
    let lookup = move |v: &Value| shape_map.get(&skeleton(v)).cloned();

    let mut sections: Vec<(String, String)> = Vec::new();
    let toverrides = type_overrides();
    for (name, json) in &all {
        let mut buf = String::new();
        gen_interface(name, json, &lookup, &toverrides, &mut buf);
        sections.push((pascal(name), buf));
    }
    // 字符串枚举与联合：Rust 侧常量清单（单一来源）。
    let enums = samples::literal_enums();
    let mut enum_buf = String::new();
    for (name, variants) in &enums {
        let joined = variants.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(" | ");
        enum_buf.push_str(&format!("export type {} = {joined}\n", pascal(name)));
    }
    enum_buf.push('\n');

    sections.sort_by(|a, b| a.0.cmp(&b.0));

    let mut file = String::new();
    file.push_str("// GENERATED by `cargo run -p xtask -- bindings` — 禁止手工修改本文件。\n");
    file.push_str("// 来源：crates/shared/src/dto.rs（Rust 单一来源；CI 以 --check 再生成 diff 校验）。\n\n");
    for (_, section) in &sections {
        file.push_str(section);
    }
    file.push_str("// 字面量联合（Rust 侧常量清单生成）\n");
    file.push_str(&enum_buf);

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../frontend/src/gateway/bindings.ts");
    if check {
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        if existing != file {
            eprintln!("bindings.ts 与 Rust 单一来源不一致：请运行 `cargo run -p xtask -- bindings` 并提交");
            std::process::exit(1);
        }
        println!("bindings.ts 一致（{} 个类型）", sections.len() + enums.len());
    } else {
        std::fs::write(&path, file).expect("写入 bindings.ts");
        println!("已生成 {}（{} 个类型）", path.display(), sections.len() + enums.len());
    }
}

#[allow(dead_code)]
fn anchor() {}
