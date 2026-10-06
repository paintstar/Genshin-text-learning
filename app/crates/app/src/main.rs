//! Tauri 应用入口（组合根装配 + command 注册）。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use app_lib::composition::{compose, ComposeArgs};
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // 数据目录：系统应用数据目录（GLL_DATA_DIR 可覆盖，供开发/测试）。
            let data_dir = match std::env::var("GLL_DATA_DIR") {
                Ok(d) => std::path::PathBuf::from(d),
                Err(_) => app
                    .path()
                    .app_data_dir()
                    .map(|p| p.join("data"))
                    .unwrap_or_else(|_| std::path::PathBuf::from("./gll-data")),
            };
            // 词典资源：随安装包 resource 分发（GLL_DICT_DB 可覆盖，供开发）。
            let dict_db_path = match std::env::var("GLL_DICT_DB") {
                Ok(p) => std::path::PathBuf::from(p),
                Err(_) => app
                    .path()
                    .resource_dir()
                    .map(|p| p.join("resources/dict.db"))
                    .unwrap_or_else(|_| std::path::PathBuf::from("resources/dict.db")),
            };
            let state = compose(ComposeArgs {
                data_dir,
                dict_db_path,
                source_base_url: fetcher::DEFAULT_BASE_URL.to_string(),
                fetch_interval_ms: 1000,
                secret_vault: None,
                source_override: None,
            })
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_lib::commands::settings_get,
            app_lib::commands::settings_set,
            app_lib::commands::terms_accept,
            app_lib::commands::terms_status,
            app_lib::commands::search_quests,
            app_lib::commands::get_quest_overview,
            app_lib::commands::open_sub_quest_graph,
            app_lib::commands::fetch_job_status,
            app_lib::commands::cancel_fetch_job,
            app_lib::commands::overview_page,
            app_lib::commands::bootstrap_index_sync,
            app_lib::commands::update_check,
            app_lib::commands::update_refresh,
            app_lib::commands::batch_sync_start,
            app_lib::commands::batch_sync_cancel,
            app_lib::commands::dict_search,
            app_lib::commands::dict_term_add,
            app_lib::commands::dict_term_list,
            app_lib::commands::dict_term_delete,
            app_lib::commands::note_save,
            app_lib::commands::note_delete,
            app_lib::commands::note_set_user_note,
            app_lib::commands::notes_recent,
            app_lib::commands::notes_by_task,
            app_lib::commands::progress_save,
            app_lib::commands::progress_load,
            app_lib::commands::override_save,
            app_lib::commands::override_resolve,
            app_lib::commands::ai_state,
            app_lib::commands::ai_profile_list,
            app_lib::commands::ai_profile_save,
            app_lib::commands::ai_profile_delete,
            app_lib::commands::ai_profile_activate,
            app_lib::commands::ai_test_connection,
            app_lib::commands::ai_ask_start,
            app_lib::commands::ai_ask_cancel,
            app_lib::commands::ai_note_write_generated,
            app_lib::commands::ai_reading_override_save,
            app_lib::commands::ai_conversation_save,
            app_lib::commands::ai_conversation_append,
            app_lib::commands::ai_conversation_list,
            app_lib::commands::ai_conversation_read,
            app_lib::commands::ai_conversation_delete,
            app_lib::commands::ai_cache_clear,
            app_lib::commands::backup_export,
            app_lib::commands::restore_prepare,
            app_lib::commands::restore_cancel_pending,
            app_lib::commands::restore_undo_last,
            app_lib::commands::app_init,
            app_lib::commands::read_text_rows,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
