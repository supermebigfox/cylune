pub mod db;
pub mod domain;
pub mod error;
pub mod inventory;
pub mod parser;

use crate::{
    db::AppDatabase,
    inventory::{InventoryService, InventoryState},
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let database = AppDatabase::open(data_dir.join("inventory.sqlite"))?;
            app.manage(InventoryState::new(InventoryService::new(database)));
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
