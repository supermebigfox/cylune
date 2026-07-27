pub mod db;
pub mod domain;
pub mod error;
pub mod imports;
pub mod inventory;
pub mod parser;
pub mod settlement;

use crate::{
    db::AppDatabase,
    imports::{PrintService, PrintState},
    inventory::{InventoryService, InventoryState},
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let database_path = data_dir.join("inventory.sqlite");
            let inventory_database = AppDatabase::open(&database_path)?;
            let print_database = AppDatabase::open(&database_path)?;
            app.manage(InventoryState::new(InventoryService::new(
                inventory_database,
            )));
            app.manage(PrintState::new(PrintService::new(print_database)));
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
            imports::import_print_file,
            imports::confirm_job_mapping,
            settlement::settle_job,
            settlement::reverse_settlement,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
