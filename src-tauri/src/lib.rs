pub mod backup;
pub mod db;
pub mod domain;
pub mod error;
pub mod history;
pub mod imports;
pub mod inventory;
pub mod media;
pub mod parser;
pub mod pet;
pub mod settlement;
pub mod tray;

use crate::{
    db::AppDatabase,
    imports::{PrintService, PrintState},
    inventory::{InventoryService, InventoryState},
    media::MediaStore,
};
use rusqlite::OptionalExtension;
use tauri::Manager;

fn service_instance_recall(app: &tauri::AppHandle) {
    let recall = app.state::<pet::runtime::InstanceRecall>();
    let Some(request) = recall.pending_request() else {
        return;
    };
    let Some(runtime) = app.try_state::<pet::runtime::PetRuntime>() else {
        return;
    };
    for action in pet::runtime::second_launch_actions() {
        match action {
            pet::runtime::InstanceAction::ShowMain => tray::show_main(app),
            pet::runtime::InstanceAction::ShowPet => runtime.show(),
        }
    }
    recall.mark_completed(request);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(pet::runtime::InstanceRecall::default())
        .plugin(tauri_plugin_single_instance::init(
            |app, _args, _cwd| {
                app.state::<pet::runtime::InstanceRecall>().request();
                service_instance_recall(app);
            },
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data_dir = std::env::var_os("SPOOL_KEEPER_DATA_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or(app.path().app_data_dir()?);
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(data_dir.join("media"))?;
            let database_path = data_dir.join("inventory.sqlite");
            let inventory_database = AppDatabase::open(&database_path)?;
            let print_database = AppDatabase::open(&database_path)?;
            let initial_pet_settings = pet::PetStore::load(&print_database)?;
            let media_store = MediaStore::new(data_dir.clone())?;
            let print_service =
                PrintService::with_media_store(print_database, media_store.clone());
            let initial_pending = print_service.pending_summary()?;
            let pet_enabled = initial_pet_settings.enabled();
            let pet_visible = initial_pet_settings.effective_visibility();
            let saved_watch: Option<String> = print_service.database.connection.query_row("SELECT setting_value FROM app_settings WHERE setting_key='watch_folder' AND EXISTS(SELECT 1 FROM app_settings WHERE setting_key='watch_enabled' AND setting_value='true')",[],|row|row.get(0)).optional()?;
            let initial_locale: String = print_service
                .database
                .connection
                .query_row(
                    "SELECT setting_value FROM app_settings WHERE setting_key='locale'",
                    [],
                    |row| row.get(0),
                )
                .optional()?
                .filter(|locale: &String| matches!(locale.as_str(), "zh-CN" | "zh-TW" | "en"))
                .unwrap_or_else(|| "zh-CN".to_owned());
            app.manage(InventoryState::new(InventoryService::new(
                inventory_database,
            )));
            app.manage(PrintState::new(print_service));
            app.manage(media_store);
            app.manage(tray::WatchState(std::sync::Mutex::new(None)));
            tray::setup(app, &initial_locale, pet_enabled, pet_visible)?;
            let pet_runtime = pet::runtime::PetRuntime::start(
                app.handle().clone(),
                initial_pet_settings,
                initial_pending,
            )?;
            app.manage(pet_runtime);
            service_instance_recall(app.handle());
            if let Some(folder) = saved_watch {
                if tray::set_watch_folder(
                    app.handle().clone(),
                    Some(folder),
                    app.state(),
                    app.state(),
                )
                .is_err()
                {
                    let print_state = app.state::<PrintState>();
                    if let Ok(service) = print_state.lock() {
                        let _ = service.database.connection.execute(
                            "DELETE FROM app_settings WHERE setting_key IN ('watch_folder','watch_enabled')",
                            [],
                        );
                    };
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            inventory::create_spool,
            inventory::mount_spool,
            inventory::unmount_slot,
            inventory::move_spool,
            inventory::calibrate_spool,
            inventory::archive_spool,
            inventory::list_spools,
            inventory::list_slots,
            imports::import_print_file,
            imports::import_print_project,
            imports::confirm_job_mapping,
            imports::discard_pending_job,
            imports::discard_project,
            imports::skip_plate,
            imports::confirm_new_print,
            imports::confirm_new_project,
            imports::get_job_preview,
            imports::get_project_preview,
            history::list_print_projects,
            history::get_print_project,
            settlement::settle_job,
            settlement::reverse_settlement,
            backup::export_backup,
            backup::import_backup,
            pet::get_pet_settings,
            pet::set_pet_settings,
            tray::set_watch_folder,
            tray::get_watch_folder,
            tray::open_main,
            tray::open_job_in_main,
            tray::take_pending_job,
            tray::take_pending_navigation,
            tray::set_native_locale,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(runtime) = app.try_state::<pet::runtime::PetRuntime>() {
                    runtime.shutdown();
                }
            }
        });
}

#[cfg(test)]
mod config_tests {
    #[test]
    fn asset_protocol_is_limited_to_app_data_media() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json must remain valid JSON");
        let asset = &config["app"]["security"]["assetProtocol"];

        assert_eq!(asset["enable"], true);
        assert_eq!(asset["scope"], serde_json::json!(["$APPDATA/media/**"]));
    }
}
