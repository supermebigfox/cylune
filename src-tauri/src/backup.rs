use crate::imports::PrintState;
use crate::{
    db::AppDatabase,
    domain::{Confidence, JobOutcome},
    error::{AppError, Result},
    parser::{
        gcode::{GcodeReport, LayerUsage},
        FilamentProfile, ParsedPrintFile,
    },
};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const BACKUP_SCHEMA_VERSION: u32 = 1;
const SAFE_SETTINGS: &[&str] = &["theme", "locale", "notifications_enabled"];

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
    parsed: BackupParsed,
    parse_count: u32,
    created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupParsed {
    filaments: Vec<BackupProfile>,
    gcode: BackupGcode,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupProfile {
    tool: u8,
    preset_id: String,
    brand: String,
    material: String,
    series: String,
    color_hex: String,
    diameter_mm: f64,
    density_g_cm3: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupGcode {
    layers: Vec<BackupLayer>,
    totals_mm: BTreeMap<u8, f64>,
    max_layer: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupLayer {
    layer: u32,
    cumulative_mm: BTreeMap<u8, f64>,
    confidence: Confidence,
}

impl TryFrom<ParsedPrintFile> for BackupParsed {
    type Error = AppError;
    fn try_from(value: ParsedPrintFile) -> Result<Self> {
        Ok(Self {
            filaments: value
                .filaments
                .into_iter()
                .map(|p| BackupProfile {
                    tool: p.tool,
                    preset_id: p.preset_id,
                    brand: p.brand,
                    material: p.material,
                    series: p.series,
                    color_hex: p.color_hex,
                    diameter_mm: p.diameter_mm,
                    density_g_cm3: p.density_g_cm3,
                })
                .collect(),
            gcode: BackupGcode {
                layers: value
                    .gcode
                    .layers
                    .into_iter()
                    .map(|l| BackupLayer {
                        layer: l.layer,
                        cumulative_mm: l.cumulative_mm,
                        confidence: l.confidence,
                    })
                    .collect(),
                totals_mm: value.gcode.totals_mm,
                max_layer: value.gcode.max_layer,
            },
        })
    }
}
impl From<BackupParsed> for ParsedPrintFile {
    fn from(value: BackupParsed) -> Self {
        Self {
            filaments: value
                .filaments
                .into_iter()
                .map(|p| FilamentProfile {
                    tool: p.tool,
                    preset_id: p.preset_id,
                    brand: p.brand,
                    material: p.material,
                    series: p.series,
                    color_hex: p.color_hex,
                    diameter_mm: p.diameter_mm,
                    density_g_cm3: p.density_g_cm3,
                    unknown_fields: BTreeMap::new(),
                })
                .collect(),
            gcode: GcodeReport {
                layers: value
                    .gcode
                    .layers
                    .into_iter()
                    .map(|l| LayerUsage {
                        layer: l.layer,
                        cumulative_mm: l.cumulative_mm,
                        confidence: l.confidence,
                    })
                    .collect(),
                totals_mm: value.gcode.totals_mm,
                max_layer: value.gcode.max_layer,
            },
        }
    }
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
        parse_cache: read_parse_cache(db)?,
        jobs: rows(db, "SELECT job_id,source_hash,outcome,settlement_version,created_at FROM print_jobs ORDER BY job_id", |r| Ok(JobRow{job_id:r.get(0)?,source_hash:r.get(1)?,outcome:r.get(2)?,settlement_version:r.get(3)?,created_at:r.get(4)?}))?,
        mappings: rows(db, "SELECT job_id,tool,spool_id,slot_number FROM job_mappings ORDER BY job_id,tool", |r| Ok(MappingRow{job_id:r.get(0)?,tool:r.get(1)?,spool_id:r.get(2)?,slot_number:r.get(3)?}))?,
        consumption: rows(db, "SELECT job_id,spool_id,settlement_version,consumed_grams,confidence,slot_number FROM job_consumption ORDER BY job_id,spool_id,settlement_version", |r| Ok(ConsumptionRow{job_id:r.get(0)?,spool_id:r.get(1)?,settlement_version:r.get(2)?,consumed_grams:r.get(3)?,confidence:r.get(4)?,slot_number:r.get(5)?}))?,
        ledger: rows(db, "SELECT event_id,idempotency_key,spool_id,job_id,settlement_version,event_type,delta_grams,confidence,reverses_event_id,created_at FROM ledger_events ORDER BY CASE event_type WHEN 'reversal' THEN 1 ELSE 0 END,created_at,event_id", |r| Ok(LedgerRow{event_id:r.get(0)?,idempotency_key:r.get(1)?,spool_id:r.get(2)?,job_id:r.get(3)?,settlement_version:r.get(4)?,event_type:r.get(5)?,delta_grams:r.get(6)?,confidence:r.get(7)?,reverses_event_id:r.get(8)?,created_at:r.get(9)?}))?,
        settings: rows(db, "SELECT setting_key,setting_value FROM app_settings ORDER BY setting_key", |r| Ok(SettingRow{key:r.get(0)?,value:r.get(1)?}))?.into_iter().filter(|row| SAFE_SETTINGS.contains(&row.key.as_str())).collect(),
    })
}

#[cfg(test)]
fn export_json_for_test(db: &mut AppDatabase) -> Result<String> {
    serde_json::to_string(&read_backup(db)?).map_err(|_| AppError::InvalidFile)
}

fn read_parse_cache(db: &AppDatabase) -> Result<Vec<ParseRow>> {
    let raw=rows(db,"SELECT source_hash,parsed_json,parse_count,created_at FROM parse_cache ORDER BY source_hash",|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u32>(2)?,r.get::<_,String>(3)?)))?;
    raw.into_iter()
        .map(|(source_hash, json, parse_count, created_at)| {
            let parsed: ParsedPrintFile =
                serde_json::from_str(&json).map_err(|_| AppError::InvalidFile)?;
            Ok(ParseRow {
                source_hash,
                parsed: BackupParsed::try_from(parsed)?,
                parse_count,
                created_at,
            })
        })
        .collect()
}

fn valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}
fn validate(b: &Backup) -> Result<()> {
    if b.schema_version != BACKUP_SCHEMA_VERSION || b.slots.len() != 4 {
        return Err(AppError::InvalidFile);
    }
    let unique = |values: Vec<&str>| {
        let mut set = HashSet::new();
        values.into_iter().all(|v| set.insert(v))
    };
    if !unique(b.spools.iter().map(|s| s.spool_id.as_str()).collect())
        || !unique(
            b.parse_cache
                .iter()
                .map(|p| p.source_hash.as_str())
                .collect(),
        )
        || !unique(b.jobs.iter().map(|j| j.job_id.as_str()).collect())
        || !unique(b.ledger.iter().map(|e| e.event_id.as_str()).collect())
        || !unique(
            b.ledger
                .iter()
                .map(|e| e.idempotency_key.as_str())
                .collect(),
        )
        || !unique(b.settings.iter().map(|s| s.key.as_str()).collect())
    {
        return Err(AppError::InvalidFile);
    }
    let spool_ids: HashSet<_> = b.spools.iter().map(|s| s.spool_id.as_str()).collect();
    for spool in &b.spools {
        if !valid_uuid(&spool.spool_id)
            || !finite_nonnegative(spool.remaining_grams)
            || !matches!(
                spool.status.as_str(),
                "available" | "assigned" | "empty" | "archived"
            )
            || unsafe_stamp(&spool.created_at)
        {
            return Err(AppError::InvalidFile);
        }
    }
    let mut mounted = HashSet::new();
    for (index, slot) in b.slots.iter().enumerate() {
        if slot.slot_number as usize != index + 1
            || slot
                .spool_id
                .as_deref()
                .is_some_and(|id| !spool_ids.contains(id) || !mounted.insert(id))
        {
            return Err(AppError::InvalidFile);
        }
    }
    let parse_by_hash: HashMap<_, _> = b
        .parse_cache
        .iter()
        .map(|p| (p.source_hash.as_str(), p))
        .collect();
    for cached in &b.parse_cache {
        if !valid_hash(&cached.source_hash)
            || cached.parse_count != 1
            || unsafe_stamp(&cached.created_at)
            || validate_parsed(&cached.parsed).is_err()
        {
            return Err(AppError::InvalidFile);
        }
    }
    let job_ids: HashSet<_> = b.jobs.iter().map(|j| j.job_id.as_str()).collect();
    let job_by_id: HashMap<_, _> = b.jobs.iter().map(|j| (j.job_id.as_str(), j)).collect();
    for job in &b.jobs {
        if !valid_uuid(&job.job_id)
            || !parse_by_hash.contains_key(job.source_hash.as_str())
            || unsafe_stamp(&job.created_at)
        {
            return Err(AppError::InvalidFile);
        }
        if let Some(outcome) = &job.outcome {
            if serde_json::from_str::<JobOutcome>(outcome).is_err() || job.settlement_version == 0 {
                return Err(AppError::InvalidFile);
            }
        } else if job.settlement_version != 0 {
            return Err(AppError::InvalidFile);
        }
    }
    let mut mapping_keys = HashSet::new();
    for mapping in &b.mappings {
        let Some(job) = job_by_id.get(mapping.job_id.as_str()) else {
            return Err(AppError::InvalidFile);
        };
        if !spool_ids.contains(mapping.spool_id.as_str())
            || !mapping_keys.insert((&mapping.job_id, mapping.tool))
            || mapping.slot_number.is_some_and(|s| !(1..=4).contains(&s))
        {
            return Err(AppError::InvalidFile);
        }
        let parsed = &parse_by_hash[job.source_hash.as_str()].parsed;
        if !parsed.filaments.iter().any(|p| p.tool == mapping.tool) {
            return Err(AppError::InvalidFile);
        }
    }
    let mut consumption_keys = HashSet::new();
    for item in &b.consumption {
        let Some(job) = job_by_id.get(item.job_id.as_str()) else {
            return Err(AppError::InvalidFile);
        };
        if !spool_ids.contains(item.spool_id.as_str())
            || item.settlement_version == 0
            || item.settlement_version > job.settlement_version
            || !finite_nonnegative(item.consumed_grams)
            || !valid_confidence(&item.confidence)
            || item.slot_number.is_some_and(|s| !(1..=4).contains(&s))
            || !consumption_keys.insert((&item.job_id, &item.spool_id, item.settlement_version))
        {
            return Err(AppError::InvalidFile);
        }
    }
    let event_by_id: HashMap<_, _> = b.ledger.iter().map(|e| (e.event_id.as_str(), e)).collect();
    for event in &b.ledger {
        if !valid_uuid(&event.event_id)
            || !spool_ids.contains(event.spool_id.as_str())
            || !event.delta_grams.is_finite()
            || !valid_confidence(&event.confidence)
            || unsafe_stamp(&event.created_at)
        {
            return Err(AppError::InvalidFile);
        }
        match event.event_type.as_str() {
            "creation" => {
                if event.job_id.is_some()
                    || event.settlement_version.is_some()
                    || event.reverses_event_id.is_some()
                    || event.delta_grams < 0.0
                {
                    return Err(AppError::InvalidFile);
                }
            }
            "adjustment" => {
                if event.job_id.is_some()
                    || event.settlement_version.is_some()
                    || event.reverses_event_id.is_some()
                    || event.delta_grams == 0.0
                {
                    return Err(AppError::InvalidFile);
                }
            }
            "settlement" => {
                if event
                    .job_id
                    .as_deref()
                    .is_none_or(|id| !job_ids.contains(id))
                    || event.settlement_version.is_none_or(|v| v == 0)
                    || event.reverses_event_id.is_some()
                    || event.delta_grams >= 0.0
                {
                    return Err(AppError::InvalidFile);
                }
            }
            "reversal" => {
                let Some(original) = event
                    .reverses_event_id
                    .as_deref()
                    .and_then(|id| event_by_id.get(id))
                else {
                    return Err(AppError::InvalidFile);
                };
                if original.event_type != "settlement"
                    || original.spool_id != event.spool_id
                    || original.job_id != event.job_id
                    || original.settlement_version != event.settlement_version
                    || (original.delta_grams + event.delta_grams).abs() > 1e-6
                    || event.delta_grams <= 0.0
                {
                    return Err(AppError::InvalidFile);
                }
            }
            _ => return Err(AppError::InvalidFile),
        }
    }
    for item in &b.consumption {
        if !b.ledger.iter().any(|e| {
            e.event_type == "settlement"
                && e.job_id.as_deref() == Some(&item.job_id)
                && e.spool_id == item.spool_id
                && e.settlement_version == Some(item.settlement_version)
                && (e.delta_grams + item.consumed_grams).abs() < 1e-6
                && e.confidence == item.confidence
        }) {
            return Err(AppError::InvalidFile);
        }
    }
    for spool in &b.spools {
        let balance: f64 = b
            .ledger
            .iter()
            .filter(|e| e.spool_id == spool.spool_id)
            .map(|e| e.delta_grams)
            .sum();
        if (balance - spool.remaining_grams).abs() > 1e-6 {
            return Err(AppError::InvalidFile);
        }
        let expected = if spool.status == "archived" {
            "archived"
        } else if spool.remaining_grams <= 0.0 {
            "empty"
        } else if mounted.contains(spool.spool_id.as_str()) {
            "assigned"
        } else {
            "available"
        };
        if spool.status != expected {
            return Err(AppError::InvalidFile);
        }
    }
    for setting in &b.settings {
        if !SAFE_SETTINGS.contains(&setting.key.as_str())
            || contains_sensitive(&setting.key)
            || unsafe_setting(setting)
        {
            return Err(AppError::InvalidFile);
        }
    }
    Ok(())
}

fn contains_sensitive(value: &str) -> bool {
    ["token", "password", "secret", "credential"]
        .iter()
        .any(|needle| value.to_ascii_lowercase().contains(needle))
}

fn finite_nonnegative(v: f64) -> bool {
    v.is_finite() && v >= 0.0
}
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_confidence(value: &str) -> bool {
    matches!(value, "exact" | "estimated" | "needs_confirmation")
}
fn unsafe_stamp(value: &str) -> bool {
    value.is_empty() || value.contains('/') || value.contains('\\')
}
fn unsafe_setting(s: &SettingRow) -> bool {
    match s.key.as_str() {
        "locale" => !matches!(s.value.as_str(), "zh-CN" | "zh-TW" | "en"),
        "theme" => !matches!(s.value.as_str(), "light" | "dark"),
        "notifications_enabled" => !matches!(s.value.as_str(), "true" | "false"),
        _ => true,
    }
}
fn validate_parsed(parsed: &BackupParsed) -> Result<()> {
    let tools: HashSet<_> = parsed.filaments.iter().map(|p| p.tool).collect();
    if tools.len() != parsed.filaments.len()
        || parsed.filaments.is_empty()
        || parsed.filaments.iter().any(|p| {
            !p.diameter_mm.is_finite()
                || p.diameter_mm <= 0.0
                || !p.density_g_cm3.is_finite()
                || p.density_g_cm3 <= 0.0
        })
    {
        return Err(AppError::InvalidFile);
    }
    if parsed.gcode.max_layer as usize != parsed.gcode.layers.len()
        || parsed.gcode.layers.iter().enumerate().any(|(i, l)| {
            l.layer as usize != i
                || l.cumulative_mm
                    .iter()
                    .any(|(tool, v)| !tools.contains(tool) || !finite_nonnegative(*v))
        })
        || parsed
            .gcode
            .totals_mm
            .iter()
            .any(|(tool, v)| !tools.contains(tool) || !finite_nonnegative(*v))
    {
        return Err(AppError::InvalidFile);
    }
    let mut previous: BTreeMap<u8, f64> = BTreeMap::new();
    for layer in &parsed.gcode.layers {
        for (tool, value) in &layer.cumulative_mm {
            if *value + 1e-9 < previous.get(tool).copied().unwrap_or(0.0)
                || *value > parsed.gcode.totals_mm.get(tool).copied().unwrap_or(0.0) + 1e-9
            {
                return Err(AppError::InvalidFile);
            }
            previous.insert(*tool, *value);
        }
    }
    Ok(())
}

const DROP_LEDGER_TRIGGERS:&str="DROP TRIGGER IF EXISTS prevent_ledger_event_delete;DROP TRIGGER IF EXISTS prevent_ledger_event_update;DROP TRIGGER IF EXISTS require_ledger_reversal_reference;DROP TRIGGER IF EXISTS prevent_non_reversal_reference;";
const CREATE_LEDGER_TRIGGERS: &str = r#"CREATE TRIGGER prevent_ledger_event_delete BEFORE DELETE ON ledger_events BEGIN SELECT RAISE(ABORT,'ledger events are immutable'); END;CREATE TRIGGER prevent_ledger_event_update BEFORE UPDATE ON ledger_events BEGIN SELECT RAISE(ABORT,'ledger events are immutable'); END;CREATE TRIGGER require_ledger_reversal_reference BEFORE INSERT ON ledger_events WHEN NEW.event_type='reversal' AND (NEW.reverses_event_id IS NULL OR NOT EXISTS(SELECT 1 FROM ledger_events WHERE event_id=NEW.reverses_event_id)) BEGIN SELECT RAISE(ABORT,'reversal events must reference an existing event'); END;CREATE TRIGGER prevent_non_reversal_reference BEFORE INSERT ON ledger_events WHEN NEW.event_type<>'reversal' AND NEW.reverses_event_id IS NOT NULL BEGIN SELECT RAISE(ABORT,'only reversal events may reference another event'); END;"#;

fn restore(tx: &Transaction<'_>, b: &Backup) -> Result<()> {
    tx.execute_batch(DROP_LEDGER_TRIGGERS)?;
    tx.execute_batch("DELETE FROM ledger_events;DELETE FROM job_consumption;DELETE FROM job_mappings;DELETE FROM print_jobs;DELETE FROM parse_cache;DELETE FROM ams_slots;DELETE FROM spools;")?;
    for key in SAFE_SETTINGS {
        tx.execute("DELETE FROM app_settings WHERE setting_key=?1", [key])?;
    }
    for s in &b.spools {
        tx.execute("INSERT INTO spools(spool_id,display_name,preset_id,brand,material,series,color_hex,remaining_grams,status,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![s.spool_id,s.display_name,s.preset_id,s.brand,s.material,s.series,s.color_hex,s.remaining_grams,s.status,s.created_at])?;
    }
    for p in &b.parse_cache {
        let parsed: ParsedPrintFile = p.parsed.clone().into();
        let json = serde_json::to_string(&parsed).map_err(|_| AppError::InvalidFile)?;
        tx.execute("INSERT INTO parse_cache(source_hash,source_file_name,parsed_json,parse_count,created_at) VALUES(?1,'restored-print',?2,?3,?4)",params![p.source_hash,json,p.parse_count,p.created_at])?;
    }
    for j in &b.jobs {
        tx.execute("INSERT INTO print_jobs(job_id,source_hash,source_file_name,outcome,settlement_version,created_at) VALUES(?1,?2,'restored-print',?3,?4,?5)",params![j.job_id,j.source_hash,j.outcome,j.settlement_version,j.created_at])?;
    }
    for m in &b.mappings {
        tx.execute(
            "INSERT INTO job_mappings(job_id,tool,spool_id,slot_number) VALUES(?1,?2,?3,?4)",
            params![m.job_id, m.tool, m.spool_id, m.slot_number],
        )?;
    }
    for c in &b.consumption {
        tx.execute("INSERT INTO job_consumption(job_id,spool_id,settlement_version,consumed_grams,confidence,slot_number) VALUES(?1,?2,?3,?4,?5,?6)",params![c.job_id,c.spool_id,c.settlement_version,c.consumed_grams,c.confidence,c.slot_number])?;
    }
    for e in b
        .ledger
        .iter()
        .filter(|e| e.event_type != "reversal")
        .chain(b.ledger.iter().filter(|e| e.event_type == "reversal"))
    {
        tx.execute("INSERT INTO ledger_events(event_id,idempotency_key,spool_id,job_id,settlement_version,event_type,delta_grams,confidence,reverses_event_id,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![e.event_id,e.idempotency_key,e.spool_id,e.job_id,e.settlement_version,e.event_type,e.delta_grams,e.confidence,e.reverses_event_id,e.created_at])?;
    }
    for s in &b.settings {
        tx.execute(
            "INSERT INTO app_settings(setting_key,setting_value) VALUES(?1,?2)",
            params![s.key, s.value],
        )?;
    }
    for slot in &b.slots {
        tx.execute(
            "INSERT INTO ams_slots(slot_number,spool_id,assigned_at) VALUES(?3,?1,?2)",
            params![slot.spool_id, slot.assigned_at, slot.slot_number],
        )?;
    }
    tx.execute_batch(CREATE_LEDGER_TRIGGERS)?;
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
    use super::{export_json_for_test, export_to_path, import_from_path};
    use crate::{
        db::AppDatabase,
        inventory::{InventoryService, NewSpool},
        parser::{
            gcode::{GcodeReport, LayerUsage},
            FilamentProfile, ParsedPrintFile,
        },
    };
    use rusqlite::params;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    #[test]
    fn pet_coordinates_are_not_exported() {
        let mut db = AppDatabase::open_in_memory().unwrap();
        db.connection
            .execute(
                "INSERT INTO app_settings(setting_key,setting_value) VALUES
                 ('pet_x','400'),('pet_y','220'),('pet_display_id','9')",
                [],
            )
            .unwrap();
        let json = export_json_for_test(&mut db).unwrap();
        assert!(!json.contains("pet_x"));
        assert!(!json.contains("pet_display_id"));
    }

    fn populated() -> AppDatabase {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        inventory
            .create_spool(NewSpool {
                display_name: "Cloud White".into(),
                preset_id: Some("Bambu PLA Basic @BBL A1".into()),
                catalog_id: None,
                color_name: None,
                color_code: None,
                color_hexes: vec!["#FFFEFC".into()],
                preset_base: None,
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
        let mut unknown = BTreeMap::new();
        unknown.insert(
            "device_token".into(),
            serde_json::json!("very-secret-token"),
        );
        unknown.insert(
            "machine_start_gcode".into(),
            serde_json::json!("G1 E999 ; /Users/robin/private.3mf"),
        );
        let parsed = ParsedPrintFile {
            filaments: vec![FilamentProfile {
                tool: 0,
                preset_id: "Bambu PLA Basic @BBL A1".into(),
                brand: "Bambu Lab".into(),
                material: "PLA".into(),
                series: "Basic".into(),
                color_hex: "#FFFEFC".into(),
                diameter_mm: 1.75,
                density_g_cm3: 1.26,
                unknown_fields: unknown,
            }],
            gcode: GcodeReport {
                layers: vec![LayerUsage {
                    layer: 0,
                    cumulative_mm: BTreeMap::from([(0, 10.0)]),
                    confidence: crate::domain::Confidence::Exact,
                }],
                totals_mm: BTreeMap::from([(0, 10.0)]),
                max_layer: 1,
            },
        };
        database.connection.execute("INSERT INTO parse_cache(source_hash,source_file_name,parsed_json,parse_count) VALUES(?1,?2,?3,1)",params!["a".repeat(64),"private.gcode.3mf",serde_json::to_string(&parsed).unwrap()]).unwrap();
        let path = std::env::temp_dir().join(format!("spool-backup-{}.json", uuid::Uuid::new_v4()));
        export_to_path(&database, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["spools"].as_array().unwrap().len(), 1);
        assert!(!text.contains("never-export-me"));
        assert!(!text.contains("device_token"));
        for forbidden in [
            "very-secret-token",
            "machine_start_gcode",
            "/Users/",
            "private.gcode.3mf",
            ".3mf",
            "G1 E999",
            "watch_folder",
            "/tmp/slices",
        ] {
            assert!(!text.contains(forbidden), "backup leaked {forbidden}");
        }
        assert!(value["parse_cache"][0].get("parsed").is_some());
        assert!(value["parse_cache"][0].get("parsed_json").is_none());
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
    fn restore_is_an_exact_snapshot_and_preserves_runtime_settings() {
        let source = populated();
        let expected_id: String = source
            .connection
            .query_row("SELECT spool_id FROM spools", [], |r| r.get(0))
            .unwrap();
        let path = std::env::temp_dir().join(format!("snapshot-{}.json", Uuid::new_v4()));
        export_to_path(&source, &path).unwrap();
        let mut target = populated();
        let mut inventory = InventoryService::new(target);
        inventory
            .create_spool(NewSpool {
                display_name: "Later spool".into(),
                preset_id: None,
                catalog_id: None,
                color_name: None,
                color_code: None,
                color_hexes: vec!["#000000".into()],
                preset_base: None,
                brand: "Bambu Lab".into(),
                material: "PETG".into(),
                series: "Basic".into(),
                color_hex: "#000000".into(),
                remaining_grams: 500.0,
            })
            .unwrap();
        target = inventory.into_database();
        target.connection.execute("INSERT INTO app_settings(setting_key,setting_value) VALUES('watch_enabled','true') ON CONFLICT(setting_key) DO UPDATE SET setting_value='true'",[]).unwrap();
        let automatic = import_from_path(&mut target, &path).unwrap();
        assert_eq!(
            target
                .connection
                .query_row("SELECT COUNT(*) FROM spools", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            target
                .connection
                .query_row("SELECT spool_id FROM spools", [], |r| r.get::<_, String>(0))
                .unwrap(),
            expected_id
        );
        assert_eq!(
            target
                .connection
                .query_row("SELECT remaining_grams FROM spools", [], |r| r
                    .get::<_, f64>(0))
                .unwrap(),
            812.5
        );
        assert_eq!(
            target
                .connection
                .query_row(
                    "SELECT setting_value FROM app_settings WHERE setting_key='watch_enabled'",
                    [],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            "true"
        );
        assert!(
            target
                .connection
                .execute("DELETE FROM ledger_events", [])
                .is_err(),
            "immutable trigger must be restored"
        );
        import_from_path(&mut target, &path).unwrap();
        assert_eq!(
            target
                .connection
                .query_row("SELECT COUNT(*) FROM ledger_events", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            1
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(automatic).unwrap();
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
