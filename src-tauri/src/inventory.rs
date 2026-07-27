use crate::{
    db::AppDatabase,
    domain::{Confidence, LedgerEventType, Spool, SpoolStatus},
    error::{AppError, Result},
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewSpool {
    pub display_name: String,
    pub brand: String,
    pub material: String,
    pub series: String,
    pub color_hex: String,
    pub remaining_grams: f64,
}

pub struct InventoryService {
    database: AppDatabase,
}

pub type InventoryState = Mutex<InventoryService>;

impl InventoryService {
    pub fn new(database: AppDatabase) -> Self {
        Self { database }
    }

    pub fn create_spool(&mut self, spool: NewSpool) -> Result<Uuid> {
        let spool_id = Uuid::new_v4();
        let status = if spool.remaining_grams == 0.0 {
            "empty"
        } else {
            "available"
        };
        self.database.connection.execute(
            "INSERT INTO spools (spool_id, display_name, brand, material, series, color_hex, remaining_grams, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                spool_id.to_string(),
                spool.display_name,
                spool.brand,
                spool.material,
                spool.series,
                spool.color_hex,
                spool.remaining_grams,
                status,
            ],
        )?;
        Ok(spool_id)
    }

    pub fn get_spool(&self, spool_id: Uuid) -> Result<Spool> {
        self.database.connection.query_row(
            "SELECT spool_id, display_name, brand, material, series, color_hex, remaining_grams, status FROM spools WHERE spool_id = ?1",
            params![spool_id.to_string()],
            spool_from_row,
        ).map_err(Into::into)
    }

    pub fn calibrate_spool(&mut self, spool_id: Uuid, grams: f64) -> Result<()> {
        let transaction = self.database.connection.transaction()?;
        let (current, status): (f64, String) = transaction.query_row(
            "SELECT remaining_grams, status FROM spools WHERE spool_id = ?1",
            params![spool_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if status == "archived" {
            return Err(AppError::ArchivedSpool);
        }
        let delta = grams - current;
        let is_mounted: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM ams_slots WHERE spool_id = ?1)",
            params![spool_id.to_string()],
            |row| row.get(0),
        )?;
        let next_status = if grams == 0.0 {
            "empty"
        } else if is_mounted {
            "assigned"
        } else {
            "available"
        };

        if delta != 0.0 {
            transaction.execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    Uuid::new_v4().to_string(),
                    Uuid::new_v4().to_string(),
                    spool_id.to_string(),
                    event_type_name(LedgerEventType::Adjustment),
                    delta,
                    confidence_name(Confidence::Exact),
                ],
            )?;
            transaction.execute(
                "UPDATE spools SET remaining_grams = ?1, status = ?2 WHERE spool_id = ?3",
                params![grams, next_status, spool_id.to_string()],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn mount_spool(&mut self, slot_number: u8, spool_id: Uuid) -> Result<()> {
        let transaction = self.database.connection.transaction()?;
        ensure_slot_exists(&transaction, slot_number)?;

        let spool = spool_in_transaction(&transaction, spool_id)?;
        if spool.status == SpoolStatus::Archived {
            return Err(AppError::ArchivedSpool);
        }
        let assigned_slot: Option<u8> = transaction
            .query_row(
                "SELECT slot_number FROM ams_slots WHERE spool_id = ?1",
                params![spool_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if assigned_slot.is_some() {
            return Err(AppError::SlotConflict);
        }

        transaction.execute(
            "UPDATE spools SET status = 'available' WHERE spool_id = (SELECT spool_id FROM ams_slots WHERE slot_number = ?1)",
            params![slot_number],
        )?;
        transaction.execute(
            "UPDATE ams_slots SET spool_id = ?1, assigned_at = CURRENT_TIMESTAMP WHERE slot_number = ?2",
            params![spool_id.to_string(), slot_number],
        )?;
        transaction.execute(
            "UPDATE spools SET status = 'assigned' WHERE spool_id = ?1",
            params![spool_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn slot_spool(&self, slot_number: u8) -> Result<Option<Uuid>> {
        self.database
            .connection
            .query_row(
                "SELECT spool_id FROM ams_slots WHERE slot_number = ?1",
                params![slot_number],
                |row| row.get::<_, Option<String>>(0),
            )
            .map_err(Into::into)
            .and_then(|spool_id| match spool_id {
                Some(spool_id) => spool_id
                    .parse()
                    .map(Some)
                    .map_err(|_| AppError::Database("invalid spool id".to_owned())),
                None => Ok(None),
            })
    }

    pub fn unmount_slot(&mut self, slot_number: u8) -> Result<()> {
        let transaction = self.database.connection.transaction()?;
        ensure_slot_exists(&transaction, slot_number)?;
        let spool_id: Option<String> = transaction.query_row(
            "SELECT spool_id FROM ams_slots WHERE slot_number = ?1",
            params![slot_number],
            |row| row.get(0),
        )?;

        transaction.execute(
            "UPDATE ams_slots SET spool_id = NULL, assigned_at = NULL WHERE slot_number = ?1",
            params![slot_number],
        )?;
        if let Some(spool_id) = spool_id {
            transaction.execute(
                "UPDATE spools SET status = CASE WHEN remaining_grams = 0 THEN 'empty' ELSE 'available' END WHERE spool_id = ?1",
                params![spool_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn move_spool(&mut self, spool_id: Uuid, destination_slot: u8) -> Result<()> {
        let transaction = self.database.connection.transaction()?;
        ensure_slot_exists(&transaction, destination_slot)?;
        let source_slot: Option<u8> = transaction
            .query_row(
                "SELECT slot_number FROM ams_slots WHERE spool_id = ?1",
                params![spool_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let source_slot = source_slot.ok_or(AppError::SlotConflict)?;
        if source_slot == destination_slot {
            return Ok(());
        }
        let displaced_spool: Option<String> = transaction.query_row(
            "SELECT spool_id FROM ams_slots WHERE slot_number = ?1",
            params![destination_slot],
            |row| row.get(0),
        )?;

        transaction.execute(
            "UPDATE ams_slots SET spool_id = NULL, assigned_at = NULL WHERE slot_number IN (?1, ?2)",
            params![source_slot, destination_slot],
        )?;
        transaction.execute(
            "UPDATE ams_slots SET spool_id = ?1, assigned_at = CURRENT_TIMESTAMP WHERE slot_number = ?2",
            params![spool_id.to_string(), destination_slot],
        )?;
        if let Some(displaced_spool) = displaced_spool {
            transaction.execute(
                "UPDATE ams_slots SET spool_id = ?1, assigned_at = CURRENT_TIMESTAMP WHERE slot_number = ?2",
                params![displaced_spool, source_slot],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn archive_spool(&mut self, spool_id: Uuid) -> Result<()> {
        let transaction = self.database.connection.transaction()?;
        spool_in_transaction(&transaction, spool_id)?;
        let is_loaded: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM ams_slots WHERE spool_id = ?1)",
            params![spool_id.to_string()],
            |row| row.get(0),
        )?;
        if is_loaded {
            return Err(AppError::SlotConflict);
        }
        transaction.execute(
            "UPDATE spools SET status = 'archived' WHERE spool_id = ?1",
            params![spool_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_spools(&self) -> Result<Vec<Spool>> {
        let mut statement = self.database.connection.prepare(
            "SELECT spool_id, display_name, brand, material, series, color_hex, remaining_grams, status FROM spools WHERE status <> 'archived' ORDER BY created_at, spool_id",
        )?;
        let spools = statement
            .query_map([], spool_from_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(spools)
    }
}

fn ensure_slot_exists(transaction: &rusqlite::Transaction<'_>, slot_number: u8) -> Result<()> {
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM ams_slots WHERE slot_number = ?1)",
        params![slot_number],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(AppError::InvalidSlot)
    }
}

fn spool_in_transaction(transaction: &rusqlite::Transaction<'_>, spool_id: Uuid) -> Result<Spool> {
    transaction.query_row(
        "SELECT spool_id, display_name, brand, material, series, color_hex, remaining_grams, status FROM spools WHERE spool_id = ?1",
        params![spool_id.to_string()],
        spool_from_row,
    ).map_err(Into::into)
}

fn spool_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Spool> {
    let status: String = row.get(7)?;
    Ok(Spool {
        spool_id: row.get::<_, String>(0)?.parse().map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        display_name: row.get(1)?,
        brand: row.get(2)?,
        material: row.get(3)?,
        series: row.get(4)?,
        color_hex: row.get(5)?,
        remaining_grams: row.get(6)?,
        status: spool_status(&status)?,
    })
}

fn spool_status(value: &str) -> rusqlite::Result<SpoolStatus> {
    match value {
        "available" => Ok(SpoolStatus::Available),
        "assigned" => Ok(SpoolStatus::Assigned),
        "empty" => Ok(SpoolStatus::Empty),
        "archived" => Ok(SpoolStatus::Archived),
        _ => Err(rusqlite::Error::FromSqlConversionFailure(
            7,
            rusqlite::types::Type::Text,
            "unknown spool status".into(),
        )),
    }
}

fn event_type_name(event_type: LedgerEventType) -> &'static str {
    match event_type {
        LedgerEventType::Settlement => "settlement",
        LedgerEventType::Reversal => "reversal",
        LedgerEventType::Adjustment => "adjustment",
    }
}

fn confidence_name(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Exact => "exact",
        Confidence::Estimated => "estimated",
        Confidence::NeedsConfirmation => "needs_confirmation",
    }
}

fn with_inventory<T>(
    inventory: tauri::State<'_, InventoryState>,
    operation: impl FnOnce(&mut InventoryService) -> Result<T>,
) -> Result<T> {
    let mut service = inventory
        .lock()
        .map_err(|_| AppError::Database("inventory lock poisoned".to_owned()))?;
    operation(&mut service)
}

#[tauri::command]
pub fn create_spool(spool: NewSpool, inventory: tauri::State<'_, InventoryState>) -> Result<Uuid> {
    with_inventory(inventory, |service| service.create_spool(spool))
}

#[tauri::command]
pub fn mount_spool(
    slot_number: u8,
    spool_id: Uuid,
    inventory: tauri::State<'_, InventoryState>,
) -> Result<()> {
    with_inventory(inventory, |service| {
        service.mount_spool(slot_number, spool_id)
    })
}

#[tauri::command]
pub fn unmount_slot(slot_number: u8, inventory: tauri::State<'_, InventoryState>) -> Result<()> {
    with_inventory(inventory, |service| service.unmount_slot(slot_number))
}

#[tauri::command]
pub fn move_spool(
    spool_id: Uuid,
    destination_slot: u8,
    inventory: tauri::State<'_, InventoryState>,
) -> Result<()> {
    with_inventory(inventory, |service| {
        service.move_spool(spool_id, destination_slot)
    })
}

#[tauri::command]
pub fn calibrate_spool(
    spool_id: Uuid,
    grams: f64,
    inventory: tauri::State<'_, InventoryState>,
) -> Result<()> {
    with_inventory(inventory, |service| {
        service.calibrate_spool(spool_id, grams)
    })
}

#[tauri::command]
pub fn archive_spool(spool_id: Uuid, inventory: tauri::State<'_, InventoryState>) -> Result<()> {
    with_inventory(inventory, |service| service.archive_spool(spool_id))
}

#[tauri::command]
pub fn list_spools(inventory: tauri::State<'_, InventoryState>) -> Result<Vec<Spool>> {
    with_inventory(inventory, |service| service.list_spools())
}

#[cfg(test)]
mod tests {
    use super::{InventoryService, NewSpool};
    use crate::db::AppDatabase;
    use uuid::Uuid;

    fn new_bambu_black() -> NewSpool {
        NewSpool {
            display_name: "Bambu PLA Basic Black".to_owned(),
            brand: "Bambu Lab".to_owned(),
            material: "PLA".to_owned(),
            series: "Basic".to_owned(),
            color_hex: "#000000".to_owned(),
            remaining_grams: 1000.0,
        }
    }

    #[test]
    fn identical_spools_keep_independent_balances() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);

        let a = service.create_spool(new_bambu_black()).unwrap();
        let b = service.create_spool(new_bambu_black()).unwrap();
        service.calibrate_spool(a, 620.0).unwrap();

        assert_eq!(service.get_spool(a).unwrap().remaining_grams, 620.0);
        assert_eq!(service.get_spool(b).unwrap().remaining_grams, 1000.0);
        assert_ne!(a, b);
    }

    #[test]
    fn a_zero_gram_spool_starts_empty() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let spool = service
            .create_spool(NewSpool {
                remaining_grams: 0.0,
                ..new_bambu_black()
            })
            .unwrap();

        assert_eq!(
            service.get_spool(spool).unwrap().status,
            crate::domain::SpoolStatus::Empty
        );
    }

    #[test]
    fn mounting_replaces_the_slot_occupant_in_one_transaction() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let old = service.create_spool(new_bambu_black()).unwrap();
        let replacement = service.create_spool(new_bambu_black()).unwrap();
        service.mount_spool(1, old).unwrap();

        service.mount_spool(1, replacement).unwrap();

        assert_eq!(service.slot_spool(1).unwrap(), Some(replacement));
        assert_eq!(
            service.get_spool(old).unwrap().status,
            crate::domain::SpoolStatus::Available
        );
        assert_eq!(
            service.get_spool(replacement).unwrap().status,
            crate::domain::SpoolStatus::Assigned
        );
    }

    #[test]
    fn unmounting_preserves_balance_and_returns_the_spool_to_library() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let spool = service.create_spool(new_bambu_black()).unwrap();
        service.mount_spool(3, spool).unwrap();

        service.unmount_slot(3).unwrap();

        assert_eq!(service.slot_spool(3).unwrap(), None);
        let spool = service.get_spool(spool).unwrap();
        assert_eq!(spool.remaining_grams, 1000.0);
        assert_eq!(spool.status, crate::domain::SpoolStatus::Available);
    }

    #[test]
    fn moving_to_an_occupied_slot_swaps_the_two_spools() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let first = service.create_spool(new_bambu_black()).unwrap();
        let second = service.create_spool(new_bambu_black()).unwrap();
        service.mount_spool(1, first).unwrap();
        service.mount_spool(2, second).unwrap();

        service.move_spool(first, 2).unwrap();

        assert_eq!(service.slot_spool(1).unwrap(), Some(second));
        assert_eq!(service.slot_spool(2).unwrap(), Some(first));
        assert_eq!(
            service.get_spool(first).unwrap().status,
            crate::domain::SpoolStatus::Assigned
        );
        assert_eq!(
            service.get_spool(second).unwrap().status,
            crate::domain::SpoolStatus::Assigned
        );
    }

    #[test]
    fn calibration_appends_only_the_difference_as_an_adjustment_event() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let spool = service
            .create_spool(NewSpool {
                remaining_grams: 650.0,
                ..new_bambu_black()
            })
            .unwrap();

        service.calibrate_spool(spool, 628.0).unwrap();

        let event = service
            .database
            .connection
            .query_row(
                "SELECT event_type, delta_grams FROM ledger_events WHERE spool_id = ?1",
                rusqlite::params![spool.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)),
            )
            .unwrap();
        assert_eq!(event, ("adjustment".to_owned(), -22.0));
        assert_eq!(service.get_spool(spool).unwrap().remaining_grams, 628.0);
    }

    #[test]
    fn archiving_a_loaded_spool_fails_without_changing_its_slot() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let spool = service.create_spool(new_bambu_black()).unwrap();
        service.mount_spool(4, spool).unwrap();

        let error = service.archive_spool(spool).unwrap_err();

        assert_eq!(error.code(), "slot_conflict");
        assert_eq!(service.slot_spool(4).unwrap(), Some(spool));
        assert_eq!(
            service.get_spool(spool).unwrap().status,
            crate::domain::SpoolStatus::Assigned
        );
    }

    #[test]
    fn archived_spools_are_hidden_from_the_default_list() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let archived = service.create_spool(new_bambu_black()).unwrap();
        let visible = service.create_spool(new_bambu_black()).unwrap();

        service.archive_spool(archived).unwrap();

        assert_eq!(
            service.get_spool(archived).unwrap().status,
            crate::domain::SpoolStatus::Archived
        );
        assert_eq!(
            service.list_spools().unwrap(),
            vec![service.get_spool(visible).unwrap()]
        );
    }

    #[test]
    fn failed_calibration_rolls_back_its_adjustment_event_and_balance() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let spool = service.create_spool(new_bambu_black()).unwrap();

        assert!(service.calibrate_spool(spool, -1.0).is_err());

        assert_eq!(service.get_spool(spool).unwrap().remaining_grams, 1000.0);
        let event_count: u8 = service
            .database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM ledger_events WHERE spool_id = ?1",
                rusqlite::params![spool.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_count, 0);
    }

    #[test]
    fn recalibrating_an_empty_unmounted_spool_restores_availability() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let spool = service.create_spool(new_bambu_black()).unwrap();
        service.calibrate_spool(spool, 0.0).unwrap();

        service.calibrate_spool(spool, 80.0).unwrap();

        let spool = service.get_spool(spool).unwrap();
        assert_eq!(spool.remaining_grams, 80.0);
        assert_eq!(spool.status, crate::domain::SpoolStatus::Available);
    }

    #[test]
    fn linked_reversal_events_conserve_their_original_delta() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = InventoryService::new(database);
        let spool = service.create_spool(new_bambu_black()).unwrap();
        let settlement = Uuid::new_v4();
        service
            .database
            .connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence) VALUES (?1, ?2, ?3, 'settlement', -125.0, 'exact')",
                rusqlite::params![settlement.to_string(), Uuid::new_v4().to_string(), spool.to_string()],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence, reverses_event_id) VALUES (?1, ?2, ?3, 'reversal', 125.0, 'exact', ?4)",
                rusqlite::params![Uuid::new_v4().to_string(), Uuid::new_v4().to_string(), spool.to_string(), settlement.to_string()],
            )
            .unwrap();

        let net_delta: f64 = service
            .database
            .connection
            .query_row(
                "SELECT SUM(delta_grams) FROM ledger_events WHERE spool_id = ?1",
                rusqlite::params![spool.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(net_delta, 0.0);
    }
}
