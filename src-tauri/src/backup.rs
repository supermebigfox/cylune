use crate::imports::PrintState;
use crate::{
    db::AppDatabase,
    error::{AppError, Result},
};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const BACKUP_SCHEMA_VERSION: u32 = 1;
const SAFE_SETTINGS: &[&str] = &[
    "theme",
    "locale",
    "watch_folder",
    "watch_enabled",
    "notifications_enabled",
];

#[tauri::command]
pub fn export_backup(path: String, state: tauri::State<'_, PrintState>) -> Result<String> {
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?;
    Ok(export_to_path(&service.database, Path::new(&path))?
        .to_string_lossy()
        .into_owned())
}

#[tauri::command]
pub fn import_backup(path: String, state: tauri::State<'_, PrintState>) -> Result<String> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?;
    Ok(import_from_path(&mut service.database, Path::new(&path))?
        .to_string_lossy()
        .into_owned())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Backup {
    schema_version: u32,
    spools: Vec<SpoolRow>,
    slots: Vec<SlotRow>,
    parse_cache: Vec<ParseRow>,
    jobs: Vec<JobRow>,
    mappings: Vec<MappingRow>,
    consumption: Vec<ConsumptionRow>,
    ledger: Vec<LedgerRow>,
    settings: Vec<SettingRow>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpoolRow {
    spool_id: String,
    display_name: String,
    preset_id: Option<String>,
    brand: String,
    material: String,
    series: String,
    color_hex: String,
    remaining_grams: f64,
    status: String,
    created_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SlotRow {
    slot_number: u8,
    spool_id: Option<String>,
    assigned_at: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParseRow {
    source_hash: String,
    parsed_json: String,
    parse_count: u32,
    created_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobRow {
    job_id: String,
    source_hash: String,
    outcome: Option<String>,
    settlement_version: u32,
    created_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingRow {
    job_id: String,
    tool: u8,
    spool_id: String,
    slot_number: Option<u8>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsumptionRow {
    job_id: String,
    spool_id: String,
    settlement_version: u32,
    consumed_grams: f64,
    confidence: String,
    slot_number: Option<u8>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LedgerRow {
    event_id: String,
    idempotency_key: String,
    spool_id: String,
    job_id: Option<String>,
    settlement_version: Option<u32>,
    event_type: String,
    delta_grams: f64,
    confidence: String,
    reverses_event_id: Option<String>,
    created_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingRow {
    key: String,
    value: String,
}

pub fn export_to_path(database: &AppDatabase, path: &Path) -> Result<PathBuf> {
    let backup = read_backup(database)?;
    let json = serde_json::to_vec_pretty(&backup)
        .map_err(|error| AppError::Database(error.to_string()))?;
    let parent = path.parent().ok_or(AppError::InvalidFile)?;
    if !parent.is_dir() {
        return Err(AppError::InvalidFile);
    }
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    fs::write(&temporary, json)?;
    fs::rename(&temporary, path)?;
    Ok(path.to_path_buf())
}

pub fn import_from_path(database: &mut AppDatabase, path: &Path) -> Result<PathBuf> {
    let bytes = fs::read(path)?;
    let backup: Backup = serde_json::from_slice(&bytes).map_err(|_| AppError::InvalidFile)?;
    validate(&backup)?;
    let automatic = path.with_file_name(format!("spool-keeper-auto-{}.json", Uuid::new_v4()));
    export_to_path(database, &automatic)?;
    let transaction = database.connection.transaction()?;
    restore(&transaction, &backup)?;
    let violations: Option<String> = transaction
        .query_row("PRAGMA foreign_key_check", [], |row| row.get(0))
        .optional()?;
    if violations.is_some() {
        return Err(AppError::InvalidFile);
    }
    validate_balances(&transaction)?;
    transaction.commit()?;
    Ok(automatic)
}

fn rows<T, F>(database: &AppDatabase, sql: &str, mut map: F) -> Result<Vec<T>>
where
    F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut statement = database.connection.prepare(sql)?;
    let result = statement
        .query_map([], |row| map(row))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(result)
}

fn read_backup(db: &AppDatabase) -> Result<Backup> {
    Ok(Backup {
        schema_version: BACKUP_SCHEMA_VERSION,
        spools: rows(db, "SELECT spool_id,display_name,preset_id,brand,material,series,color_hex,remaining_grams,status,created_at FROM spools ORDER BY spool_id", |r| Ok(SpoolRow{spool_id:r.get(0)?,display_name:r.get(1)?,preset_id:r.get(2)?,brand:r.get(3)?,material:r.get(4)?,series:r.get(5)?,color_hex:r.get(6)?,remaining_grams:r.get(7)?,status:r.get(8)?,created_at:r.get(9)?}))?,
        slots: rows(db, "SELECT slot_number,spool_id,assigned_at FROM ams_slots ORDER BY slot_number", |r| Ok(SlotRow{slot_number:r.get(0)?,spool_id:r.get(1)?,assigned_at:r.get(2)?}))?,
        parse_cache: rows(db, "SELECT source_hash,parsed_json,parse_count,created_at FROM parse_cache ORDER BY source_hash", |r| Ok(ParseRow{source_hash:r.get(0)?,parsed_json:r.get(1)?,parse_count:r.get(2)?,created_at:r.get(3)?}))?,
        jobs: rows(db, "SELECT job_id,source_hash,outcome,settlement_version,created_at FROM print_jobs ORDER BY job_id", |r| Ok(JobRow{job_id:r.get(0)?,source_hash:r.get(1)?,outcome:r.get(2)?,settlement_version:r.get(3)?,created_at:r.get(4)?}))?,
        mappings: rows(db, "SELECT job_id,tool,spool_id,slot_number FROM job_mappings ORDER BY job_id,tool", |r| Ok(MappingRow{job_id:r.get(0)?,tool:r.get(1)?,spool_id:r.get(2)?,slot_number:r.get(3)?}))?,
        consumption: rows(db, "SELECT job_id,spool_id,settlement_version,consumed_grams,confidence,slot_number FROM job_consumption ORDER BY job_id,spool_id,settlement_version", |r| Ok(ConsumptionRow{job_id:r.get(0)?,spool_id:r.get(1)?,settlement_version:r.get(2)?,consumed_grams:r.get(3)?,confidence:r.get(4)?,slot_number:r.get(5)?}))?,
        ledger: rows(db, "SELECT event_id,idempotency_key,spool_id,job_id,settlement_version,event_type,delta_grams,confidence,reverses_event_id,created_at FROM ledger_events ORDER BY CASE event_type WHEN 'reversal' THEN 1 ELSE 0 END,created_at,event_id", |r| Ok(LedgerRow{event_id:r.get(0)?,idempotency_key:r.get(1)?,spool_id:r.get(2)?,job_id:r.get(3)?,settlement_version:r.get(4)?,event_type:r.get(5)?,delta_grams:r.get(6)?,confidence:r.get(7)?,reverses_event_id:r.get(8)?,created_at:r.get(9)?}))?,
        settings: rows(db, "SELECT setting_key,setting_value FROM app_settings ORDER BY setting_key", |r| Ok(SettingRow{key:r.get(0)?,value:r.get(1)?}))?.into_iter().filter(|row| SAFE_SETTINGS.contains(&row.key.as_str())).collect(),
    })
}

fn valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}
fn validate(b: &Backup) -> Result<()> {
    if b.schema_version != BACKUP_SCHEMA_VERSION || b.slots.len() != 4 {
        return Err(AppError::InvalidFile);
    }
    if b.spools.iter().any(|s| {
        !valid_uuid(&s.spool_id)
            || !s.remaining_grams.is_finite()
            || s.remaining_grams < 0.0
            || !matches!(
                s.status.as_str(),
                "available" | "assigned" | "empty" | "archived"
            )
    }) {
        return Err(AppError::InvalidFile);
    }
    if b.slots.iter().enumerate().any(|(i, s)| {
        s.slot_number as usize != i + 1 || s.spool_id.as_deref().is_some_and(|id| !valid_uuid(id))
    }) {
        return Err(AppError::InvalidFile);
    }
    if b.jobs
        .iter()
        .any(|j| !valid_uuid(&j.job_id) || j.source_hash.len() != 64)
        || b.ledger.iter().any(|e| {
            !valid_uuid(&e.event_id)
                || !valid_uuid(&e.spool_id)
                || e.job_id.as_deref().is_some_and(|id| !valid_uuid(id))
                || e.reverses_event_id
                    .as_deref()
                    .is_some_and(|id| !valid_uuid(id))
                || !e.delta_grams.is_finite()
        })
    {
        return Err(AppError::InvalidFile);
    }
    if b.settings
        .iter()
        .any(|s| !SAFE_SETTINGS.contains(&s.key.as_str()) || contains_sensitive(&s.key))
    {
        return Err(AppError::InvalidFile);
    }
    Ok(())
}

fn contains_sensitive(value: &str) -> bool {
    ["token", "password", "secret", "credential"]
        .iter()
        .any(|needle| value.to_ascii_lowercase().contains(needle))
}

fn restore(tx: &Transaction<'_>, b: &Backup) -> Result<()> {
    for s in &b.spools {
        tx.execute("INSERT OR IGNORE INTO spools(spool_id,display_name,preset_id,brand,material,series,color_hex,remaining_grams,status,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![s.spool_id,s.display_name,s.preset_id,s.brand,s.material,s.series,s.color_hex,s.remaining_grams,s.status,s.created_at])?;
    }
    for p in &b.parse_cache {
        tx.execute("INSERT OR IGNORE INTO parse_cache(source_hash,source_file_name,parsed_json,parse_count,created_at) VALUES(?1,'restored-print',?2,?3,?4)",params![p.source_hash,p.parsed_json,p.parse_count,p.created_at])?;
    }
    for j in &b.jobs {
        tx.execute("INSERT OR IGNORE INTO print_jobs(job_id,source_hash,source_file_name,outcome,settlement_version,created_at) VALUES(?1,?2,'restored-print',?3,?4,?5)",params![j.job_id,j.source_hash,j.outcome,j.settlement_version,j.created_at])?;
    }
    for m in &b.mappings {
        tx.execute("INSERT OR IGNORE INTO job_mappings(job_id,tool,spool_id,slot_number) VALUES(?1,?2,?3,?4)",params![m.job_id,m.tool,m.spool_id,m.slot_number])?;
    }
    for c in &b.consumption {
        tx.execute("INSERT OR IGNORE INTO job_consumption(job_id,spool_id,settlement_version,consumed_grams,confidence,slot_number) VALUES(?1,?2,?3,?4,?5,?6)",params![c.job_id,c.spool_id,c.settlement_version,c.consumed_grams,c.confidence,c.slot_number])?;
    }
    for e in &b.ledger {
        tx.execute("INSERT OR IGNORE INTO ledger_events(event_id,idempotency_key,spool_id,job_id,settlement_version,event_type,delta_grams,confidence,reverses_event_id,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![e.event_id,e.idempotency_key,e.spool_id,e.job_id,e.settlement_version,e.event_type,e.delta_grams,e.confidence,e.reverses_event_id,e.created_at])?;
    }
    for s in &b.settings {
        tx.execute("INSERT INTO app_settings(setting_key,setting_value) VALUES(?1,?2) ON CONFLICT(setting_key) DO UPDATE SET setting_value=excluded.setting_value,updated_at=CURRENT_TIMESTAMP",params![s.key,s.value])?;
    }
    for slot in &b.slots {
        tx.execute(
            "UPDATE ams_slots SET spool_id=?1,assigned_at=?2 WHERE slot_number=?3",
            params![slot.spool_id, slot.assigned_at, slot.slot_number],
        )?;
    }
    Ok(())
}

fn validate_balances(tx: &Transaction<'_>) -> Result<()> {
    let mut statement=tx.prepare("SELECT s.spool_id,s.remaining_grams,COALESCE(SUM(l.delta_grams),0) FROM spools s LEFT JOIN ledger_events l ON l.spool_id=s.spool_id GROUP BY s.spool_id")?;
    let invalid = statement
        .query_map([], |r| Ok((r.get::<_, f64>(1)?, r.get::<_, f64>(2)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .any(|(cached, ledger)| (cached - ledger).abs() > 0.000_001);
    if invalid {
        Err(AppError::InvalidFile)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{export_to_path, import_from_path};
    use crate::{
        db::AppDatabase,
        inventory::{InventoryService, NewSpool},
    };
    use rusqlite::params;

    fn populated() -> AppDatabase {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        inventory
            .create_spool(NewSpool {
                display_name: "Cloud White".into(),
                preset_id: Some("Bambu PLA Basic @BBL A1".into()),
                brand: "Bambu Lab".into(),
                material: "PLA".into(),
                series: "Basic".into(),
                color_hex: "#FFFEFC".into(),
                remaining_grams: 812.5,
            })
            .unwrap();
        let database = inventory.into_database();
        database.connection.execute("INSERT INTO app_settings(setting_key, setting_value) VALUES ('theme', 'dark'), ('device_token', 'never-export-me'), ('watch_folder', '/tmp/slices')", []).unwrap();
        database
    }

    #[test]
    fn backup_is_versioned_and_excludes_secrets_and_source_files() {
        let database = populated();
        let path = std::env::temp_dir().join(format!("spool-backup-{}.json", uuid::Uuid::new_v4()));
        export_to_path(&database, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["spools"].as_array().unwrap().len(), 1);
        assert!(!text.contains("never-export-me"));
        assert!(!text.contains("device_token"));
        assert!(!text.contains(".3mf\""));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn round_trip_and_duplicate_restore_keep_one_immutable_baseline() {
        let source = populated();
        let path = std::env::temp_dir().join(format!("spool-backup-{}.json", uuid::Uuid::new_v4()));
        export_to_path(&source, &path).unwrap();
        let mut target = AppDatabase::open_in_memory().unwrap();
        import_from_path(&mut target, &path).unwrap();
        import_from_path(&mut target, &path).unwrap();
        let counts = (
            target
                .connection
                .query_row("SELECT COUNT(*) FROM spools", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            target
                .connection
                .query_row("SELECT COUNT(*) FROM ledger_events", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
        );
        assert_eq!(counts, (1, 1));
        assert_eq!(
            target
                .connection
                .query_row(
                    "SELECT setting_value FROM app_settings WHERE setting_key='theme'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
            "dark"
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_restore_rolls_back_a_populated_database() {
        let mut database = populated();
        let before = database
            .connection
            .query_row("SELECT remaining_grams FROM spools", [], |row| {
                row.get::<_, f64>(0)
            })
            .unwrap();
        let path = std::env::temp_dir().join(format!("bad-backup-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(
            &path,
            r#"{"schema_version":1,"spools":[{"spool_id":"not-a-uuid"}]}"#,
        )
        .unwrap();
        assert!(import_from_path(&mut database, &path).is_err());
        assert_eq!(
            database
                .connection
                .query_row("SELECT remaining_grams FROM spools", [], |row| row
                    .get::<_, f64>(0))
                .unwrap(),
            before
        );
        assert_eq!(
            database
                .connection
                .query_row("SELECT COUNT(*) FROM ledger_events", params![], |row| row
                    .get::<_, u32>(
                    0
                ))
                .unwrap(),
            1
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn restore_rejects_unknown_sensitive_fields() {
        let source = populated();
        let path =
            std::env::temp_dir().join(format!("tainted-backup-{}.json", uuid::Uuid::new_v4()));
        export_to_path(&source, &path).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["device_token"] = serde_json::json!("must-not-enter-database");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let mut target = AppDatabase::open_in_memory().unwrap();
        assert!(import_from_path(&mut target, &path).is_err());
        assert_eq!(
            target
                .connection
                .query_row("SELECT COUNT(*) FROM spools", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            0
        );
        std::fs::remove_file(path).unwrap();
    }
}
