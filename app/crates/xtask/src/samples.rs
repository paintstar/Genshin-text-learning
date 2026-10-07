//! DTO 样本实例（形状单一来源）：每个跨 FFI 边界的 struct DTO 一个样本。
//! 样本经 serde 序列化（与运行时同一路径）推导 TS interface——字段名/
//! 可空性/嵌套形状与 Rust 定义强一致。

use serde_json::{json, Value};

pub fn samples() -> Vec<(&'static str, Value)> {
    vec![
        ("lang_text", json!({ "lang": "jp", "text": "テスト" })),
        (
            "quest_summary",
            json!({
                "questId": 1702, "questType": "aq", "chapterNum": "第七章",
                "route": "white-night", "chapterCount": 3,
                "titles": [{ "lang": "jp", "text": "白夜国浮世画天夢" }],
                "hasCachedBody": false, "alignStatus": null
            }),
        ),
        (
            "sub_quest_brief",
            json!({ "subQuestId": "0", "sort": 0, "titles": [{ "lang": "jp", "text": "序幕" }], "descs": [{ "lang": "jp", "text": "説明" }], "hasProgress": false }),
        ),
        (
            "block_brief",
            json!({ "subQuestId": "0", "stepId": "0", "treeNo": 0, "initDialogId": "101", "treeOrder": 0 }),
        ),
        (
            "block_key",
            json!({ "subQuestId": "0", "stepId": "0", "treeNo": 0 }),
        ),
        (
            "quest_overview",
            json!({ "summary": null, "subs": [{ "subQuestId": "0", "sort": 0, "titles": [{ "lang": "jp", "text": "x" }], "descs": [], "hasProgress": false }],
                "trees": [{ "subQuestId": "0", "stepId": "0", "treeNo": 0, "initDialogId": "1", "treeOrder": 0 }],
                "blockOrder": [{ "subQuestId": "0", "stepId": "0", "treeNo": 0 }],
                "conflicts": [{ "id": 1, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": null, "kind": "edge", "detailJson": "{}" }] }),
        ),
        (
            "node_dto",
            json!({
                "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "101",
                "kind": "choice", "displaySeq": 0, "branchDepth": 0, "isJoin": false,
                "status": "ok", "danglingNext": null
            }),
        ),
        (
            "row_dto",
            json!({
                "opt": { "questId": 1702, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "101", "optIndex": 0 },
                "role": "パイモン", "text": "こんにちは", "nextDialogId": "102", "lang": "jp"
            }),
        ),
        (
            "conflict_brief",
            json!({ "id": 1, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "101", "kind": "edge", "detailJson": "{}" }),
        ),
        (
            "graph_snapshot",
            json!({
                "questId": 1702, "subQuestId": "0", "followLang": "jp",
                "trees": [{ "subQuestId": "0", "stepId": "0", "treeNo": 0, "initDialogId": "1", "treeOrder": 0 }],
                "blockOrder": [{ "subQuestId": "0", "stepId": "0", "treeNo": 0 }],
                "nodes": [{ "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "1", "kind": "talk", "displaySeq": 0, "branchDepth": 0, "isJoin": false, "status": "ok", "danglingNext": null }],
                "rows": [{ "opt": { "questId": 1702, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "101", "optIndex": 0 }, "role": "パイモン", "text": "こんにちは", "nextDialogId": "102", "lang": "jp" }],
                "alignStatus": "ok",
                "conflicts": [{ "id": 1, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": null, "kind": "edge", "detailJson": "{}" }],
                "subs": [{ "subQuestId": "0", "sort": 0, "titles": [], "descs": [], "hasProgress": false }]
            }),
        ),
        (
            "fetch_job_status",
            serde_json::to_value(shared::dto::FetchJobStatus {
                handle: 1,
                quest_id: 1702,
                state: shared::dto::FetchJobState::Fetching,
                error: None,
                completed: 0,
                total: 3,
                phase: "正在下载日文剧情".into(),
            })
            .unwrap(),
        ),
        (
            "open_quest_result",
            json!({ "cached": false, "snapshot": null, "job": null }),
        ),
        (
            "update_report",
            json!({ "newQuests": [{ "questId": 1, "questType": null, "chapterNum": null, "route": null, "chapterCount": 0, "titles": [], "hasCachedBody": false, "alignStatus": null }],
                "changed": [{ "questId": 1, "questType": null, "chapterNum": null, "route": null, "chapterCount": 0, "titles": [], "hasCachedBody": false, "alignStatus": null }],
                "unknownBody": [{ "questId": 1, "questType": null, "chapterNum": null, "route": null, "chapterCount": 0, "titles": [], "hasCachedBody": false, "alignStatus": null }] }),
        ),
        (
            "refresh_outcome",
            json!({ "questId": 1702, "outcome": "rebuilt", "alignStatus": "ok", "error": null }),
        ),
        (
            "sync_progress",
            serde_json::to_value(shared::dto::SyncProgress {
                handle: 1,
                done: 0,
                total: 0,
                current_quest_title: None,
                failed_count: 0,
                current_job: None,
            })
            .unwrap(),
        ),
        (
            "batch_sync_report",
            serde_json::to_value(shared::dto::BatchSyncReport {
                handle: 1,
                total: 0,
                succeeded: 0,
                failed: vec![],
                cancelled: false,
            })
            .unwrap(),
        ),
        (
            "batch_sync_status",
            serde_json::to_value(shared::dto::BatchSyncStatus {
                progress: shared::dto::SyncProgress {
                    handle: 1,
                    done: 0,
                    total: 0,
                    current_quest_title: None,
                    failed_count: 0,
                    current_job: None,
                },
                report: None,
            })
            .unwrap(),
        ),
        (
            "sync_failure",
            json!({ "questId": 1, "title": null, "reason": "" }),
        ),
        (
            "candidate_form",
            json!({ "form": "食べる", "formKind": "base", "sourceNote": "还原形" }),
        ),
        ("gloss_dto", json!({ "lang": "zh", "texts": ["吃"] })),
        (
            "dict_entry_dto",
            json!({
                "headword": "食べる", "readingKana": "たべる", "pos": ["動詞"],
                "glosses": [{ "lang": "zh", "texts": ["吃"] }],
                "source": "zhwiktionary", "common": true,
                "matchedForm": "食べる", "matchedFormKind": "surface",
                "matchedSourceNote": null, "termTexts": null
            }),
        ),
        (
            "dict_search_result",
            json!({ "entries": [{ "headword": "食べる", "readingKana": "たべる", "pos": ["動詞"], "glosses": [{ "lang": "zh", "texts": ["吃"] }], "source": "zhwiktionary", "common": true, "matchedForm": "食べる", "matchedFormKind": "surface", "matchedSourceNote": null, "termTexts": null }], "dictAvailable": true }),
        ),
        (
            "note_dto",
            json!({
                "id": 1, "kind": "word",
                "optRef": { "questId": 1702, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "101", "optIndex": 0 },
                "termText": null, "termReading": null, "termBase": null,
                "contextText": null, "contextRole": null, "contextNext": null,
                "analysisSnapshotJson": null, "provenanceStale": false, "staleReason": null,
                "userNote": null, "tags": ["生词"], "origin": "user", "aiGeneratedJson": null,
                "createdAt": 0, "updatedAt": 0
            }),
        ),
        (
            "save_note_input",
            json!({
                "kind": "word",
                "optRef": { "questId": 0, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "0", "optIndex": 0 },
                "termText": null, "termReading": null, "termBase": null,
                "contextText": null, "contextRole": null, "contextNext": null,
                "contextIsChoice": false, "analysisSnapshotJson": null,
                "userNote": null, "tags": ["生词"]
            }),
        ),
        (
            "task_notes_group",
            json!({ "questId": 1, "questTitle": null, "notes": [{ "id": 1, "kind": "word", "optRef": { "questId": 1, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "1", "optIndex": 0 }, "termText": null, "termReading": null, "termBase": null, "contextText": null, "contextRole": null, "contextNext": null, "analysisSnapshotJson": null, "provenanceStale": false, "staleReason": null, "userNote": null, "tags": [], "origin": "user", "aiGeneratedJson": null, "createdAt": 0, "updatedAt": 0 }] }),
        ),
        (
            "notes_by_task",
            json!({ "groups": [{ "questId": 1, "questTitle": "任务", "notes": [{ "id": 1, "kind": "word", "optRef": { "questId": 1, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "1", "optIndex": 0 }, "termText": "x", "termReading": null, "termBase": null, "contextText": null, "contextRole": null, "contextNext": null, "analysisSnapshotJson": null, "provenanceStale": false, "staleReason": null, "userNote": null, "tags": [], "origin": "user", "aiGeneratedJson": null, "createdAt": 0, "updatedAt": 0 }] }] }),
        ),
        (
            "reading_progress_dto",
            json!({
                "questId": 1702, "subQuestId": "0", "stepId": "0", "treeNo": 0,
                "dialogId": "101", "optIndex": 0, "pathStackJson": "[]", "updatedAt": 0
            }),
        ),
        (
            "override_hit_dto",
            json!({ "reading": "いこう", "scope": "dialog" }),
        ),
        (
            "ai_test_result",
            json!({ "ok": true, "message": "", "rawOutput": null }),
        ),
        (
            "ai_profile_dto",
            json!({
                "id": 1, "name": "deepseek", "channel": "http", "cliKind": null,
                "commandPath": null, "baseUrl": "https://api.deepseek.com/v1",
                "model": "deepseek-chat", "extraJson": null, "cliVersion": null,
                "configFingerprint": null, "isActive": true, "hasSecret": true
            }),
        ),
        (
            "ai_profile_input",
            json!({
                "id": null, "name": "", "channel": "cli", "cliKind": "codex",
                "commandPath": "codex", "baseUrl": null, "model": "",
                "extraJson": null, "apiKey": null
            }),
        ),
        (
            "ai_stream_event",
            json!({ "requestId": 1, "kind": "delta", "text": "", "cached": false, "error": null }),
        ),
        (
            "ai_conversation_dto",
            json!({ "id": 1, "title": "", "questId": 1, "createdAt": 0, "messages": [{ "id": 1, "role": "user", "content": "", "createdAt": 0 }] }),
        ),
        (
            "ai_message_dto",
            json!({ "id": 1, "role": "user", "content": "", "createdAt": 0 }),
        ),
        ("table_count", json!({ "label": "笔记", "count": 0 })),
        (
            "backup_summary",
            json!({ "path": "", "schemaVersion": 5, "counts": [{ "label": "", "count": 0 }], "createdAt": 0 }),
        ),
        (
            "restore_check",
            json!({
                "path": "", "integrityOk": true, "schemaVersion": 5, "compatible": true,
                "sha256": "", "counts": [{ "label": "", "count": 0 }], "fileSize": 0
            }),
        ),
        (
            "app_init_info",
            json!({
                "indexReady": false, "termsAccepted": false, "aiAvailability": "unconfigured",
                "dictAvailable": true, "schemaVersion": 5, "pendingRestore": false
            }),
        ),
        (
            "dlg_loc",
            json!({ "questId": 1702, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "101" }),
        ),
        (
            "opt_ref",
            json!({ "questId": 1702, "subQuestId": "0", "stepId": "0", "treeNo": 0, "dialogId": "101", "optIndex": 0 }),
        ),
        (
            "app_error",
            json!({ "kind": "network", "message": "", "detail": null }),
        ),
    ]
}

/// 字面量联合类型（枚举的 TS 侧字面量集合；与 shared 枚举 serde 值一致）。
pub fn literal_enums() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("node_kind", vec!["talk", "choice", "narration"]),
        (
            "node_align_status",
            vec!["ok", "missing_side", "conflict", "dangling"],
        ),
        (
            "fetch_job_state",
            vec!["queued", "fetching", "done", "failed", "cancelled"],
        ),
        ("form_kind", vec!["surface", "base", "reading"]),
        (
            "dict_source",
            vec!["term", "zhwiktionary", "jmdict", "jmnedict"],
        ),
        ("override_scope", vec!["dialog", "quest", "global"]),
        (
            "ai_availability",
            vec![
                "unconfigured",
                "configured_available",
                "configured_unavailable",
            ],
        ),
        ("ai_channel", vec!["cli", "http"]),
        ("cli_kind", vec!["claude", "codex", "opencode"]),
        (
            "app_error_kind",
            vec![
                "network",
                "data_source",
                "data_integrity",
                "resource_missing",
                "isolation",
                "ai_channel",
                "cancelled",
                "invalid_param",
                "internal",
            ],
        ),
    ]
}
