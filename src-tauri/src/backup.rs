use crate::imports::PrintState;
use crate::{
    db::AppDatabase,
    domain::{Confidence, JobOutcome},
    error::{AppError, Result},
    parser::{
        gcode::{GcodeReport, LayerUsage},
        FilamentProfile, ParsedPlate, ParsedPrintFile, ParsedProjectV2,
    },
};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;
use zip::{write::FileOptions, ZipArchive, ZipWriter};

pub const BACKUP_SCHEMA_VERSION: u32 = 3;
const SAFE_SETTINGS: &[&str] = &["theme", "locale", "notifications_enabled"];
const BACKUP_MANIFEST: &str = "backup.json";
const MAX_BACKUP_MEDIA_BYTES: u64 = 16 * 1024 * 1024;

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
    #[serde(default)]
    media_files_included: bool,
    spools: Vec<SpoolRow>,
    slots: Vec<SlotRow>,
    parse_cache: Vec<ParseRow>,
    #[serde(default)]
    media: Vec<MediaRow>,
    #[serde(default)]
    projects: Vec<ProjectRow>,
    #[serde(default)]
    plates: Vec<PlateRow>,
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
    #[serde(default)]
    catalog_id: Option<String>,
    #[serde(default)]
    color_name: Option<String>,
    #[serde(default)]
    color_code: Option<String>,
    #[serde(default)]
    color_hexes: Vec<String>,
    #[serde(default)]
    preset_base: Option<String>,
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
    parsed: BackupCachedParse,
    parse_count: u32,
    created_at: String,
}
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum BackupCachedParse {
    Project(BackupProject),
    Legacy(BackupParsed),
}

impl<'de> Deserialize<'de> for BackupCachedParse {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("version").is_some() {
            serde_json::from_value(value)
                .map(Self::Project)
                .map_err(serde::de::Error::custom)
        } else {
            serde_json::from_value(value)
                .map(Self::Legacy)
                .map_err(serde::de::Error::custom)
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupProject {
    version: u8,
    plates: Vec<BackupProjectPlate>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BackupProjectPlate {
    plate_index: u32,
    display_name: Option<String>,
    estimated_seconds: Option<u32>,
    thumbnail_entries: Vec<String>,
    filaments: Vec<BackupProfile>,
    gcode: BackupGcode,
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
    #[serde(default)]
    declared_estimated_seconds: Option<u32>,
    #[serde(default)]
    declared_total_layers: Option<u32>,
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
                declared_estimated_seconds: value.gcode.declared_estimated_seconds,
                declared_total_layers: value.gcode.declared_total_layers,
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
                declared_estimated_seconds: value.gcode.declared_estimated_seconds,
                declared_total_layers: value.gcode.declared_total_layers,
            },
        }
    }
}

impl TryFrom<ParsedProjectV2> for BackupProject {
    type Error = AppError;

    fn try_from(value: ParsedProjectV2) -> Result<Self> {
        let plates = value
            .plates
            .into_iter()
            .map(|plate| {
                let parsed = BackupParsed::try_from(ParsedPrintFile {
                    filaments: plate.filaments,
                    gcode: plate.gcode,
                })?;
                Ok(BackupProjectPlate {
                    plate_index: plate.plate_index,
                    display_name: plate.display_name,
                    estimated_seconds: plate.estimated_seconds,
                    thumbnail_entries: plate.thumbnail_entries,
                    filaments: parsed.filaments,
                    gcode: parsed.gcode,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            version: value.version,
            plates,
        })
    }
}

impl From<BackupProject> for ParsedProjectV2 {
    fn from(value: BackupProject) -> Self {
        Self {
            version: value.version,
            plates: value
                .plates
                .into_iter()
                .map(|plate| {
                    let parsed: ParsedPrintFile = BackupParsed {
                        filaments: plate.filaments,
                        gcode: plate.gcode,
                    }
                    .into();
                    ParsedPlate {
                        plate_index: plate.plate_index,
                        display_name: plate.display_name,
                        estimated_seconds: plate.estimated_seconds,
                        thumbnail_entries: plate.thumbnail_entries,
                        filaments: parsed.filaments,
                        gcode: parsed.gcode,
                    }
                })
                .collect(),
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaRow {
    asset_id: String,
    relative_path: String,
    mime_type: String,
    byte_size: u64,
    width: Option<u32>,
    height: Option<u32>,
    created_at: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectRow {
    project_id: String,
    source_hash: String,
    imported_at: String,
    plate_count: u32,
    cover_asset_id: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlateRow {
    plate_id: String,
    project_id: String,
    plate_index: u32,
    display_name: Option<String>,
    thumbnail_asset_id: Option<String>,
    estimated_seconds: Option<u32>,
    max_layer: u32,
    parsed: BackupParsed,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct JobRow {
    job_id: String,
    source_hash: String,
    outcome: Option<String>,
    settlement_version: u32,
    created_at: String,
    #[serde(default)]
    plate_id: Option<String>,
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
    let mut backup = read_backup(database)?;
    backup.media_files_included = !backup.media.is_empty();
    let json = serde_json::to_vec_pretty(&backup)
        .map_err(|error| AppError::Database(error.to_string()))?;
    let parent = path.parent().ok_or(AppError::InvalidFile)?;
    if !parent.is_dir() {
        return Err(AppError::InvalidFile);
    }
    let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let write_result = (|| -> Result<()> {
        if backup.media_files_included {
            let root = database_root(database)?;
            let file = File::create(&temporary)?;
            let mut archive = ZipWriter::new(file);
            let options =
                FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            archive
                .start_file(BACKUP_MANIFEST, options)
                .map_err(|_| AppError::InvalidFile)?;
            archive.write_all(&json)?;
            for media in &backup.media {
                let bytes = fs::read(root.join(&media.relative_path))?;
                if bytes.len() as u64 != media.byte_size
                    || format!("{:x}", Sha256::digest(&bytes)) != media.asset_id
                {
                    return Err(AppError::InvalidFile);
                }
                archive
                    .start_file(media_archive_name(media)?, options)
                    .map_err(|_| AppError::InvalidFile)?;
                archive.write_all(&bytes)?;
            }
            archive.finish().map_err(|_| AppError::InvalidFile)?;
        } else {
            fs::write(&temporary, json)?;
        }
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result?;
    fs::rename(&temporary, path)?;
    Ok(path.to_path_buf())
}

pub fn import_from_path(database: &mut AppDatabase, path: &Path) -> Result<PathBuf> {
    let bytes = fs::read(path)?;
    let (backup, media_files) = if bytes.starts_with(b"PK\x03\x04") {
        read_backup_archive(&bytes)?
    } else {
        let backup: Backup = serde_json::from_slice(&bytes).map_err(|_| AppError::InvalidFile)?;
        if backup.media_files_included {
            return Err(AppError::InvalidFile);
        }
        (backup, Vec::new())
    };
    validate(&backup)?;
    let automatic = path.with_file_name(format!("cylune-auto-{}.backup", Uuid::new_v4()));
    export_to_path(database, &automatic)?;
    let created_media = persist_restored_media(database, &media_files)?;
    let transaction = match database.connection.transaction() {
        Ok(transaction) => transaction,
        Err(error) => {
            remove_created_media(created_media);
            return Err(error.into());
        }
    };
    let restore_result = (|| -> Result<()> {
        restore(&transaction, &backup)?;
        let violations: Option<String> = transaction
            .query_row("PRAGMA foreign_key_check", [], |row| row.get(0))
            .optional()?;
        if violations.is_some() {
            return Err(AppError::InvalidFile);
        }
        validate_balances(&transaction)?;
        transaction.commit()?;
        Ok(())
    })();
    if restore_result.is_err() {
        remove_created_media(created_media);
    }
    restore_result?;
    Ok(automatic)
}

struct ArchivedMedia {
    relative_path: String,
    bytes: Vec<u8>,
}

fn database_root(database: &AppDatabase) -> Result<PathBuf> {
    let file: String = database
        .connection
        .query_row("PRAGMA database_list", [], |row| row.get(2))?;
    if file.is_empty() {
        return Err(AppError::InvalidFile);
    }
    PathBuf::from(file)
        .parent()
        .map(Path::to_path_buf)
        .ok_or(AppError::InvalidFile)
}

fn media_archive_name(media: &MediaRow) -> Result<String> {
    Ok(format!(
        "media/{}.{}",
        media.asset_id,
        media_extension(media)?
    ))
}

fn media_extension(media: &MediaRow) -> Result<&str> {
    Path::new(&media.relative_path)
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| {
            !extension.is_empty() && extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
        .ok_or(AppError::InvalidFile)
}

fn read_backup_archive(bytes: &[u8]) -> Result<(Backup, Vec<ArchivedMedia>)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(|_| AppError::InvalidFile)?;
    let mut manifest = Vec::new();
    archive
        .by_name(BACKUP_MANIFEST)
        .map_err(|_| AppError::InvalidFile)?
        .read_to_end(&mut manifest)?;
    let backup: Backup = serde_json::from_slice(&manifest).map_err(|_| AppError::InvalidFile)?;
    if !backup.media_files_included {
        return Err(AppError::InvalidFile);
    }
    let mut media_files = Vec::with_capacity(backup.media.len());
    for media in &backup.media {
        let mut file = archive
            .by_name(&media_archive_name(media)?)
            .map_err(|_| AppError::InvalidFile)?;
        if file.is_dir() || file.size() != media.byte_size || file.size() > MAX_BACKUP_MEDIA_BYTES {
            return Err(AppError::InvalidFile);
        }
        let mut media_bytes = Vec::new();
        file.read_to_end(&mut media_bytes)?;
        if format!("{:x}", Sha256::digest(&media_bytes)) != media.asset_id {
            return Err(AppError::InvalidFile);
        }
        media_files.push(ArchivedMedia {
            relative_path: media.relative_path.clone(),
            bytes: media_bytes,
        });
    }
    Ok((backup, media_files))
}

fn persist_restored_media(
    database: &AppDatabase,
    media_files: &[ArchivedMedia],
) -> Result<Vec<PathBuf>> {
    if media_files.is_empty() {
        return Ok(Vec::new());
    }
    let root = database_root(database)?;
    let mut created = Vec::new();
    for media in media_files {
        let result = (|| -> Result<Option<PathBuf>> {
            let destination = root.join(&media.relative_path);
            let parent = destination.parent().ok_or(AppError::InvalidFile)?;
            fs::create_dir_all(parent)?;
            if destination.exists() {
                if fs::read(&destination)? != media.bytes {
                    return Err(AppError::InvalidFile);
                }
                return Ok(None);
            }
            let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            let persist_result = (|| -> Result<()> {
                file.write_all(&media.bytes)?;
                file.sync_all()?;
                fs::rename(&temporary, &destination)?;
                Ok(())
            })();
            if persist_result.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            persist_result?;
            Ok(Some(destination))
        })();
        match result {
            Ok(Some(destination)) => created.push(destination),
            Ok(None) => {}
            Err(error) => {
                remove_created_media(created);
                return Err(error);
            }
        }
    }
    Ok(created)
}

fn remove_created_media(created: Vec<PathBuf>) {
    for path in created {
        let _ = fs::remove_file(path);
    }
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
        media_files_included: false,
        spools: rows(db, "SELECT spool_id,display_name,preset_id,catalog_id,color_name,color_code,color_hexes,preset_base,brand,material,series,color_hex,remaining_grams,status,created_at FROM spools ORDER BY spool_id", |r| {
            let color_hex: String = r.get(11)?;
            let color_hexes = r
                .get::<_, Option<String>>(6)?
                .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
                .filter(|colors| !colors.is_empty())
                .unwrap_or_else(|| vec![color_hex.clone()]);
            Ok(SpoolRow {
                spool_id: r.get(0)?,
                display_name: r.get(1)?,
                preset_id: r.get(2)?,
                catalog_id: r.get(3)?,
                color_name: r.get(4)?,
                color_code: r.get(5)?,
                color_hexes,
                preset_base: r.get(7)?,
                brand: r.get(8)?,
                material: r.get(9)?,
                series: r.get(10)?,
                color_hex,
                remaining_grams: r.get(12)?,
                status: r.get(13)?,
                created_at: r.get(14)?,
            })
        })?,
        slots: rows(db, "SELECT slot_number,spool_id,assigned_at FROM ams_slots ORDER BY slot_number", |r| Ok(SlotRow{slot_number:r.get(0)?,spool_id:r.get(1)?,assigned_at:r.get(2)?}))?,
        parse_cache: read_parse_cache(db)?,
        media: rows(db, "SELECT asset_id,relative_path,mime_type,byte_size,width,height,created_at FROM media_assets ORDER BY asset_id", |r| Ok(MediaRow{asset_id:r.get(0)?,relative_path:r.get(1)?,mime_type:r.get(2)?,byte_size:r.get(3)?,width:r.get(4)?,height:r.get(5)?,created_at:r.get(6)?}))?,
        projects: rows(db, "SELECT project_id,source_hash,imported_at,plate_count,cover_asset_id FROM print_projects ORDER BY project_id", |r| Ok(ProjectRow{project_id:r.get(0)?,source_hash:r.get(1)?,imported_at:r.get(2)?,plate_count:r.get(3)?,cover_asset_id:r.get(4)?}))?,
        plates: read_plates(db)?,
        jobs: rows(db, "SELECT job_id,source_hash,outcome,settlement_version,created_at,plate_id FROM print_jobs ORDER BY job_id", |r| Ok(JobRow{job_id:r.get(0)?,source_hash:r.get(1)?,outcome:r.get(2)?,settlement_version:r.get(3)?,created_at:r.get(4)?,plate_id:r.get(5)?}))?,
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
            let parsed = if let Ok(project) = serde_json::from_str::<ParsedProjectV2>(&json) {
                if project.version != 2 {
                    return Err(AppError::InvalidFile);
                }
                BackupCachedParse::Project(BackupProject::try_from(project)?)
            } else {
                let legacy: ParsedPrintFile =
                    serde_json::from_str(&json).map_err(|_| AppError::InvalidFile)?;
                BackupCachedParse::Legacy(BackupParsed::try_from(legacy)?)
            };
            Ok(ParseRow {
                source_hash,
                parsed,
                parse_count,
                created_at,
            })
        })
        .collect()
}

fn read_plates(db: &AppDatabase) -> Result<Vec<PlateRow>> {
    let raw = rows(
        db,
        "SELECT plate_id,project_id,plate_index,display_name,thumbnail_asset_id,estimated_seconds,max_layer,parsed_json FROM print_plates ORDER BY plate_id",
        |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,u32>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,Option<u32>>(5)?,r.get::<_,u32>(6)?,r.get::<_,String>(7)?)),
    )?;
    raw.into_iter()
        .map(
            |(
                plate_id,
                project_id,
                plate_index,
                display_name,
                thumbnail_asset_id,
                estimated_seconds,
                max_layer,
                json,
            )| {
                let parsed: ParsedPrintFile =
                    serde_json::from_str(&json).map_err(|_| AppError::InvalidFile)?;
                Ok(PlateRow {
                    plate_id,
                    project_id,
                    plate_index,
                    display_name,
                    thumbnail_asset_id,
                    estimated_seconds,
                    max_layer,
                    parsed: BackupParsed::try_from(parsed)?,
                })
            },
        )
        .collect()
}

fn valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok()
}
fn validate(b: &Backup) -> Result<()> {
    if !matches!(b.schema_version, 1 | 2 | BACKUP_SCHEMA_VERSION)
        || (b.schema_version < 3 && b.media_files_included)
        || b.slots.len() != 4
    {
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
        || !unique(b.media.iter().map(|m| m.asset_id.as_str()).collect())
        || !unique(b.media.iter().map(|m| m.relative_path.as_str()).collect())
        || !unique(b.projects.iter().map(|p| p.project_id.as_str()).collect())
        || !unique(b.plates.iter().map(|p| p.plate_id.as_str()).collect())
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
    if b.schema_version < 3
        && (!b.media.is_empty()
            || !b.projects.is_empty()
            || !b.plates.is_empty()
            || b.jobs.iter().any(|job| job.plate_id.is_some())
            || b.parse_cache
                .iter()
                .any(|cached| !matches!(cached.parsed, BackupCachedParse::Legacy(_))))
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
            || validate_cached_parse(&cached.parsed).is_err()
        {
            return Err(AppError::InvalidFile);
        }
    }
    let asset_ids: HashSet<_> = b
        .media
        .iter()
        .map(|asset| asset.asset_id.as_str())
        .collect();
    for asset in &b.media {
        let relative = Path::new(&asset.relative_path);
        let expected_relative_path = valid_hash(&asset.asset_id)
            .then(|| media_extension(asset))
            .transpose()
            .ok()
            .flatten()
            .map(|extension| {
                format!(
                    "media/{}/{}.{}",
                    &asset.asset_id[..2],
                    asset.asset_id,
                    extension
                )
            });
        if !valid_hash(&asset.asset_id)
            || !relative.is_relative()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            || expected_relative_path.as_deref() != Some(asset.relative_path.as_str())
            || asset.mime_type.is_empty()
            || asset.byte_size > MAX_BACKUP_MEDIA_BYTES
            || unsafe_stamp(&asset.created_at)
            || asset.width == Some(0)
            || asset.height == Some(0)
        {
            return Err(AppError::InvalidFile);
        }
    }
    let project_by_id: HashMap<_, _> = b
        .projects
        .iter()
        .map(|project| (project.project_id.as_str(), project))
        .collect();
    for project in &b.projects {
        if !valid_uuid(&project.project_id)
            || !parse_by_hash.contains_key(project.source_hash.as_str())
            || project.plate_count == 0
            || unsafe_stamp(&project.imported_at)
            || project
                .cover_asset_id
                .as_deref()
                .is_some_and(|asset| !asset_ids.contains(asset))
        {
            return Err(AppError::InvalidFile);
        }
    }
    let plate_by_id: HashMap<_, _> = b
        .plates
        .iter()
        .map(|plate| (plate.plate_id.as_str(), plate))
        .collect();
    let mut project_plate_indices = HashSet::new();
    for plate in &b.plates {
        if !valid_uuid(&plate.plate_id)
            || !project_by_id.contains_key(plate.project_id.as_str())
            || plate.plate_index == 0
            || !project_plate_indices.insert((&plate.project_id, plate.plate_index))
            || plate
                .thumbnail_asset_id
                .as_deref()
                .is_some_and(|asset| !asset_ids.contains(asset))
            || plate.max_layer != plate.parsed.gcode.max_layer
            || validate_parsed(&plate.parsed).is_err()
        {
            return Err(AppError::InvalidFile);
        }
    }
    for project in &b.projects {
        let actual = b
            .plates
            .iter()
            .filter(|plate| plate.project_id == project.project_id)
            .count() as u32;
        if actual != project.plate_count {
            return Err(AppError::InvalidFile);
        }
    }
    let job_ids: HashSet<_> = b.jobs.iter().map(|j| j.job_id.as_str()).collect();
    let job_by_id: HashMap<_, _> = b.jobs.iter().map(|j| (j.job_id.as_str(), j)).collect();
    for job in &b.jobs {
        if !valid_uuid(&job.job_id)
            || !parse_by_hash.contains_key(job.source_hash.as_str())
            || unsafe_stamp(&job.created_at)
            || (b.schema_version == 3
                && job.plate_id.as_deref().is_none_or(|plate_id| {
                    plate_by_id.get(plate_id).is_none_or(|plate| {
                        project_by_id[plate.project_id.as_str()].source_hash != job.source_hash
                    })
                }))
        {
            return Err(AppError::InvalidFile);
        }
        if let Some(outcome) = &job.outcome {
            let skipped = serde_json::from_str::<serde_json::Value>(outcome)
                .ok()
                .and_then(|value| value.get("kind")?.as_str().map(|kind| kind == "skipped"))
                .unwrap_or(false);
            if (skipped && job.settlement_version != 0)
                || (!skipped
                    && (serde_json::from_str::<JobOutcome>(outcome).is_err()
                        || job.settlement_version == 0))
            {
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
        let has_tool = if let Some(plate_id) = job.plate_id.as_deref() {
            plate_by_id
                .get(plate_id)
                .ok_or(AppError::InvalidFile)?
                .parsed
                .filaments
                .iter()
                .any(|profile| profile.tool == mapping.tool)
        } else {
            match &parse_by_hash[job.source_hash.as_str()].parsed {
                BackupCachedParse::Legacy(parsed) => parsed
                    .filaments
                    .iter()
                    .any(|profile| profile.tool == mapping.tool),
                BackupCachedParse::Project(project) => {
                    project.plates.first().is_some_and(|plate| {
                        plate
                            .filaments
                            .iter()
                            .any(|profile| profile.tool == mapping.tool)
                    })
                }
            }
        };
        if !has_tool {
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
    for job in &b.jobs {
        let skipped = job.outcome.as_deref().is_some_and(|outcome| {
            serde_json::from_str::<serde_json::Value>(outcome)
                .ok()
                .and_then(|value| value.get("kind")?.as_str().map(|kind| kind == "skipped"))
                .unwrap_or(false)
        });
        if skipped
            && (b.consumption.iter().any(|item| item.job_id == job.job_id)
                || b.ledger
                    .iter()
                    .any(|event| event.job_id.as_deref() == Some(&job.job_id)))
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
fn validate_cached_parse(parsed: &BackupCachedParse) -> Result<()> {
    match parsed {
        BackupCachedParse::Legacy(parsed) => validate_parsed(parsed),
        BackupCachedParse::Project(project) => {
            let mut plate_indices = HashSet::new();
            if project.version != 2
                || project.plates.is_empty()
                || project.plates.iter().any(|plate| {
                    plate.plate_index == 0
                        || !plate_indices.insert(plate.plate_index)
                        || validate_parsed(&BackupParsed {
                            filaments: plate.filaments.clone(),
                            gcode: plate.gcode.clone(),
                        })
                        .is_err()
                })
            {
                Err(AppError::InvalidFile)
            } else {
                Ok(())
            }
        }
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
    tx.execute_batch("DELETE FROM ledger_events;DELETE FROM job_consumption;DELETE FROM job_mappings;DELETE FROM print_jobs;DELETE FROM print_plates;DELETE FROM print_projects;DELETE FROM parse_cache;DELETE FROM media_assets;DELETE FROM ams_slots;DELETE FROM spools;")?;
    for key in SAFE_SETTINGS {
        tx.execute("DELETE FROM app_settings WHERE setting_key=?1", [key])?;
    }
    for s in &b.spools {
        let color_hexes = if s.color_hexes.is_empty() {
            vec![s.color_hex.clone()]
        } else {
            s.color_hexes.clone()
        };
        let color_hexes_json =
            serde_json::to_string(&color_hexes).map_err(|_| AppError::InvalidFile)?;
        tx.execute("INSERT INTO spools(spool_id,display_name,preset_id,catalog_id,color_name,color_code,color_hexes,preset_base,brand,material,series,color_hex,remaining_grams,status,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![s.spool_id,s.display_name,s.preset_id,s.catalog_id,s.color_name,s.color_code,color_hexes_json,s.preset_base,s.brand,s.material,s.series,s.color_hex,s.remaining_grams,s.status,s.created_at])?;
    }
    for p in &b.parse_cache {
        let json = match p.parsed.clone() {
            BackupCachedParse::Project(project) => {
                let parsed: ParsedProjectV2 = project.into();
                serde_json::to_string(&parsed).map_err(|_| AppError::InvalidFile)?
            }
            BackupCachedParse::Legacy(parsed) => {
                let parsed: ParsedPrintFile = parsed.into();
                if b.schema_version < 3 {
                    let project = ParsedProjectV2 {
                        version: 2,
                        plates: vec![ParsedPlate {
                            plate_index: 1,
                            display_name: None,
                            estimated_seconds: parsed.gcode.declared_estimated_seconds,
                            thumbnail_entries: Vec::new(),
                            filaments: parsed.filaments,
                            gcode: parsed.gcode,
                        }],
                    };
                    serde_json::to_string(&project).map_err(|_| AppError::InvalidFile)?
                } else {
                    serde_json::to_string(&parsed).map_err(|_| AppError::InvalidFile)?
                }
            }
        };
        tx.execute("INSERT INTO parse_cache(source_hash,source_file_name,parsed_json,parse_count,created_at) VALUES(?1,'restored-print',?2,?3,?4)",params![p.source_hash,json,p.parse_count,p.created_at])?;
    }
    for media in &b.media {
        tx.execute("INSERT INTO media_assets(asset_id,relative_path,mime_type,byte_size,width,height,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![media.asset_id,media.relative_path,media.mime_type,media.byte_size,media.width,media.height,media.created_at])?;
    }
    let mut legacy_plate_by_hash = HashMap::new();
    if b.schema_version == 3 {
        for project in &b.projects {
            tx.execute("INSERT INTO print_projects(project_id,source_hash,source_file_name,source_path,imported_at,plate_count,cover_asset_id) VALUES(?1,?2,'restored-print',NULL,?3,?4,?5)",params![project.project_id,project.source_hash,project.imported_at,project.plate_count,project.cover_asset_id])?;
        }
        for plate in &b.plates {
            let parsed: ParsedPrintFile = plate.parsed.clone().into();
            let parsed_json = serde_json::to_string(&parsed).map_err(|_| AppError::InvalidFile)?;
            tx.execute("INSERT INTO print_plates(plate_id,project_id,plate_index,display_name,thumbnail_asset_id,estimated_seconds,max_layer,parsed_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![plate.plate_id,plate.project_id,plate.plate_index,plate.display_name,plate.thumbnail_asset_id,plate.estimated_seconds,plate.max_layer,parsed_json])?;
        }
    } else {
        for cached in &b.parse_cache {
            if !b
                .jobs
                .iter()
                .any(|job| job.source_hash == cached.source_hash)
            {
                continue;
            }
            let BackupCachedParse::Legacy(parsed) = cached.parsed.clone() else {
                return Err(AppError::InvalidFile);
            };
            let project_id = Uuid::new_v4().to_string();
            let plate_id = Uuid::new_v4().to_string();
            let imported_at = b
                .jobs
                .iter()
                .filter(|job| job.source_hash == cached.source_hash)
                .map(|job| job.created_at.as_str())
                .min()
                .unwrap_or(&cached.created_at);
            let max_layer = parsed.gcode.max_layer;
            let parsed_file: ParsedPrintFile = parsed.into();
            let parsed_json =
                serde_json::to_string(&parsed_file).map_err(|_| AppError::InvalidFile)?;
            tx.execute("INSERT INTO print_projects(project_id,source_hash,source_file_name,source_path,imported_at,plate_count) VALUES(?1,?2,'restored-print',NULL,?3,1)",params![project_id,cached.source_hash,imported_at])?;
            tx.execute("INSERT INTO print_plates(plate_id,project_id,plate_index,max_layer,parsed_json) VALUES(?1,?2,1,?3,?4)",params![plate_id,project_id,max_layer,parsed_json])?;
            legacy_plate_by_hash.insert(cached.source_hash.clone(), plate_id);
        }
    }
    for j in &b.jobs {
        let plate_id = if b.schema_version == 3 {
            j.plate_id.as_ref().ok_or(AppError::InvalidFile)?
        } else {
            legacy_plate_by_hash
                .get(&j.source_hash)
                .ok_or(AppError::InvalidFile)?
        };
        tx.execute("INSERT INTO print_jobs(job_id,source_hash,source_file_name,outcome,settlement_version,created_at,plate_id) VALUES(?1,?2,'restored-print',?3,?4,?5,?6)",params![j.job_id,j.source_hash,j.outcome,j.settlement_version,j.created_at,plate_id])?;
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
        imports::{PrintService, ToolMapping},
        inventory::{InventoryService, NewSpool},
        parser::{
            gcode::{GcodeReport, LayerUsage},
            FilamentProfile, ParsedPrintFile,
        },
    };
    use rusqlite::params;
    use sha2::{Digest, Sha256};
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::{Read, Write};
    use std::path::PathBuf;
    use std::time::Duration;
    use uuid::Uuid;
    use zip::{write::FileOptions, ZipArchive, ZipWriter};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

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

    fn populate(database: AppDatabase) -> AppDatabase {
        let mut inventory = InventoryService::new(database);
        inventory
            .create_spool(NewSpool {
                display_name: "Cloud White".into(),
                preset_id: Some("Bambu PLA Basic @BBL A1".into()),
                catalog_id: Some("bambu-pla-basic".into()),
                color_name: Some("Jade White".into()),
                color_code: Some("10100".into()),
                color_hexes: vec!["#FFFFFF".into()],
                preset_base: Some("Bambu PLA Basic".into()),
                brand: "Bambu Lab".into(),
                material: "PLA".into(),
                series: "Basic".into(),
                color_hex: "#FFFFFF".into(),
                remaining_grams: 812.5,
            })
            .unwrap();
        let database = inventory.into_database();
        database.connection.execute("INSERT INTO app_settings(setting_key, setting_value) VALUES ('theme', 'dark'), ('device_token', 'never-export-me'), ('watch_folder', '/tmp/slices')", []).unwrap();
        database
    }

    fn populated() -> AppDatabase {
        populate(AppDatabase::open_in_memory().unwrap())
    }

    #[test]
    fn version_three_backup_archives_and_restores_media_file_bytes() {
        let root = std::env::temp_dir().join(format!("cylune-media-source-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = AppDatabase::open(root.join("inventory.sqlite")).unwrap();
        let bytes = b"content-addressed-thumbnail";
        let asset_id = format!("{:x}", Sha256::digest(bytes));
        let relative_path = format!("media/{}/{}.png", &asset_id[..2], asset_id);
        let source_media = root.join(&relative_path);
        fs::create_dir_all(source_media.parent().unwrap()).unwrap();
        fs::write(&source_media, bytes).unwrap();
        source
            .connection
            .execute(
                "INSERT INTO media_assets (
                    asset_id, relative_path, mime_type, byte_size, width, height
                 ) VALUES (?1, ?2, 'image/png', ?3, 1, 1)",
                params![asset_id, relative_path, bytes.len() as u64],
            )
            .unwrap();
        let backup_path = root.join("backup.zip");

        export_to_path(&source, &backup_path).unwrap();

        let mut archive = ZipArchive::new(fs::File::open(&backup_path).unwrap()).unwrap();
        let mut manifest = String::new();
        archive
            .by_name("backup.json")
            .unwrap()
            .read_to_string(&mut manifest)
            .unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(manifest["schema_version"], 3);
        assert_eq!(manifest["media_files_included"], true);
        let mut archived_media = Vec::new();
        archive
            .by_name(&format!("media/{asset_id}.png"))
            .unwrap()
            .read_to_end(&mut archived_media)
            .unwrap();
        assert_eq!(archived_media, bytes);
        drop(archive);

        let target_root =
            std::env::temp_dir().join(format!("cylune-media-target-{}", Uuid::new_v4()));
        fs::create_dir_all(&target_root).unwrap();
        let mut target = AppDatabase::open(target_root.join("inventory.sqlite")).unwrap();
        let automatic = import_from_path(&mut target, &backup_path).unwrap();
        assert_eq!(fs::read(target_root.join(&relative_path)).unwrap(), bytes);
        assert_eq!(
            target
                .connection
                .query_row(
                    "SELECT asset_id, relative_path, byte_size FROM media_assets",
                    [],
                    |row| Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u64>(2)?
                    ))
                )
                .unwrap(),
            (asset_id, relative_path, bytes.len() as u64)
        );

        fs::remove_file(automatic).unwrap();
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(target_root).unwrap();
    }

    #[test]
    fn restore_rejects_media_when_archive_bytes_do_not_match_asset_hash() {
        let root = std::env::temp_dir().join(format!("cylune-hash-source-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = AppDatabase::open(root.join("inventory.sqlite")).unwrap();
        let bytes = b"content-addressed-thumbnail";
        let asset_id = format!("{:x}", Sha256::digest(bytes));
        let relative_path = format!("media/{}/{}.png", &asset_id[..2], asset_id);
        let source_media = root.join(&relative_path);
        fs::create_dir_all(source_media.parent().unwrap()).unwrap();
        fs::write(&source_media, bytes).unwrap();
        source
            .connection
            .execute(
                "INSERT INTO media_assets (
                    asset_id, relative_path, mime_type, byte_size, width, height
                 ) VALUES (?1, ?2, 'image/png', ?3, 1, 1)",
                params![asset_id, relative_path, bytes.len() as u64],
            )
            .unwrap();
        let valid_backup = root.join("valid.zip");
        export_to_path(&source, &valid_backup).unwrap();
        let mut valid_archive = ZipArchive::new(fs::File::open(&valid_backup).unwrap()).unwrap();
        let mut manifest = Vec::new();
        valid_archive
            .by_name("backup.json")
            .unwrap()
            .read_to_end(&mut manifest)
            .unwrap();
        drop(valid_archive);

        let tampered_backup = root.join("tampered.zip");
        let mut archive = ZipWriter::new(fs::File::create(&tampered_backup).unwrap());
        let options = FileOptions::default();
        archive.start_file("backup.json", options).unwrap();
        archive.write_all(&manifest).unwrap();
        archive
            .start_file(format!("media/{asset_id}.png"), options)
            .unwrap();
        let mut tampered = bytes.to_vec();
        tampered[0] ^= 0xff;
        archive.write_all(&tampered).unwrap();
        archive.finish().unwrap();

        let target_root =
            std::env::temp_dir().join(format!("cylune-hash-target-{}", Uuid::new_v4()));
        fs::create_dir_all(&target_root).unwrap();
        let mut target = AppDatabase::open(target_root.join("inventory.sqlite")).unwrap();

        let result = import_from_path(&mut target, &tampered_backup);

        if let Ok(automatic) = &result {
            fs::remove_file(automatic).unwrap();
        }
        assert!(result.is_err());
        assert_eq!(
            target
                .connection
                .query_row("SELECT COUNT(*) FROM media_assets", [], |row| {
                    row.get::<_, u32>(0)
                })
                .unwrap(),
            0
        );
        assert!(!target_root.join(&relative_path).exists());

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(target_root).unwrap();
    }

    #[test]
    fn backup_export_falls_back_for_legacy_color_hexes() {
        let mut database = populated();
        for stored in [None, Some("not-json"), Some("[]")] {
            database
                .connection
                .execute("UPDATE spools SET color_hexes = ?1", params![stored])
                .unwrap();

            let value: serde_json::Value =
                serde_json::from_str(&export_json_for_test(&mut database).unwrap()).unwrap();
            assert_eq!(
                value["spools"][0]["color_hexes"],
                serde_json::json!(["#FFFFFF"])
            );
        }
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
                declared_estimated_seconds: None,
                declared_total_layers: None,
            },
        };
        database.connection.execute("INSERT INTO parse_cache(source_hash,source_file_name,parsed_json,parse_count) VALUES(?1,?2,?3,1)",params!["a".repeat(64),"private.gcode.3mf",serde_json::to_string(&parsed).unwrap()]).unwrap();
        let path = std::env::temp_dir().join(format!("spool-backup-{}.json", uuid::Uuid::new_v4()));
        export_to_path(&database, &path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema_version"], 3);
        assert_eq!(value["spools"].as_array().unwrap().len(), 1);
        assert_eq!(value["spools"][0]["color_code"], "10100");
        assert_eq!(
            value["spools"][0]["color_hexes"],
            serde_json::json!(["#FFFFFF"])
        );
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
    fn version_one_backup_restores_catalog_defaults() {
        let source = populated();
        let spool_id: Uuid = source
            .connection
            .query_row("SELECT spool_id FROM spools", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap()
            .parse()
            .unwrap();
        let path = std::env::temp_dir().join(format!(
            "version-one-spool-backup-{}.json",
            uuid::Uuid::new_v4()
        ));
        export_to_path(&source, &path).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["schema_version"] = serde_json::json!(1);
        let spool = value["spools"][0].as_object_mut().unwrap();
        for field in [
            "catalog_id",
            "color_name",
            "color_code",
            "color_hexes",
            "preset_base",
        ] {
            spool.remove(field);
        }
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let mut target = AppDatabase::open_in_memory().unwrap();
        let automatic = import_from_path(&mut target, &path).unwrap();
        let service = InventoryService::new(target);
        let restored = service.get_spool(spool_id).unwrap();
        assert_eq!(restored.catalog_id, None);
        assert_eq!(restored.color_hexes, vec!["#FFFFFF"]);

        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(automatic).unwrap();
    }

    #[test]
    fn round_trip_and_duplicate_restore_keep_one_immutable_baseline() {
        let source = populated();
        let spool_id: Uuid = source
            .connection
            .query_row("SELECT spool_id FROM spools", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap()
            .parse()
            .unwrap();
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
        let service = InventoryService::new(target);
        let restored = service.get_spool(spool_id).unwrap();
        assert_eq!(restored.catalog_id, Some("bambu-pla-basic".into()));
        assert_eq!(restored.color_name, Some("Jade White".into()));
        assert_eq!(restored.color_code, Some("10100".into()));
        assert_eq!(restored.color_hexes, vec!["#FFFFFF"]);
        assert_eq!(restored.preset_base, Some("Bambu PLA Basic".into()));
        assert_eq!(restored.color_hex, "#FFFFFF");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn version_three_round_trip_preserves_imported_project_media_job_links_and_skipped_state() {
        let source_root =
            std::env::temp_dir().join(format!("cylune-project-v3-source-{}", Uuid::new_v4()));
        fs::create_dir_all(&source_root).unwrap();
        let database = populate(AppDatabase::open(source_root.join("inventory.sqlite")).unwrap());
        let spool_id: String = database
            .connection
            .query_row("SELECT spool_id FROM spools", [], |row| row.get(0))
            .unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let imported = service
            .import_print_project(&fixture("bambu_multicolor.3mf"))
            .unwrap();
        let plate = &imported.plates[0];
        let media_bytes = b"project-thumbnail";
        let asset_id = format!("{:x}", Sha256::digest(media_bytes));
        let relative_path = format!("media/{}/{}.png", &asset_id[..2], asset_id);
        let media_path = source_root.join(&relative_path);
        fs::create_dir_all(media_path.parent().unwrap()).unwrap();
        fs::write(&media_path, media_bytes).unwrap();
        service
            .database
            .connection
            .execute(
                "INSERT INTO media_assets (
                    asset_id, relative_path, mime_type, byte_size, width, height
                 ) VALUES (?1, ?2, 'image/png', ?3, 1, 1)",
                params![asset_id, relative_path, media_bytes.len() as u64],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_projects SET cover_asset_id=?1 WHERE project_id=?2",
                params![asset_id, imported.project_id.to_string()],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_plates SET thumbnail_asset_id=?1 WHERE plate_id=?2",
                params![asset_id, plate.plate_id.to_string()],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "INSERT INTO job_mappings(job_id,tool,spool_id) VALUES(?1,0,?2)",
                params![plate.job_id.to_string(), spool_id],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs SET outcome='{\"kind\":\"skipped\"}' WHERE job_id=?1",
                [plate.job_id.to_string()],
            )
            .unwrap();
        let settled_project = service
            .confirm_new_project(&imported.source_hash, &fixture("bambu_multicolor.3mf"))
            .unwrap();
        let settled_job = settled_project.plates[0].job_id;
        let spool_uuid = spool_id.parse().unwrap();
        service
            .confirm_job_mapping(
                settled_job,
                vec![
                    ToolMapping {
                        tool: 0,
                        spool_id: spool_uuid,
                    },
                    ToolMapping {
                        tool: 1,
                        spool_id: spool_uuid,
                    },
                ],
            )
            .unwrap();
        service
            .settle_job(settled_job, crate::domain::JobOutcome::Success)
            .unwrap();
        service.reverse_settlement(settled_job).unwrap();
        let path = source_root.join("project-v3-backup.zip");

        export_to_path(&service.database, &path).unwrap();
        let mut archive = ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
        let mut manifest = Vec::new();
        archive
            .by_name("backup.json")
            .unwrap()
            .read_to_end(&mut manifest)
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&manifest).unwrap();
        assert_eq!(value["schema_version"], 3);
        assert_eq!(value["media_files_included"], true);
        let mut archived_media = Vec::new();
        archive
            .by_name(&format!("media/{asset_id}.png"))
            .unwrap()
            .read_to_end(&mut archived_media)
            .unwrap();
        assert_eq!(archived_media, media_bytes);
        drop(archive);

        let target_root =
            std::env::temp_dir().join(format!("cylune-project-v3-target-{}", Uuid::new_v4()));
        fs::create_dir_all(&target_root).unwrap();
        let mut target = AppDatabase::open(target_root.join("inventory.sqlite")).unwrap();
        let automatic = import_from_path(&mut target, &path).unwrap();
        let restored = target
            .connection
            .query_row(
                "SELECT projects.project_id, plates.plate_id, jobs.job_id, jobs.outcome,
                    jobs.settlement_version, plates.thumbnail_asset_id, projects.cover_asset_id
             FROM print_projects AS projects
             JOIN print_plates AS plates USING(project_id)
             JOIN print_jobs AS jobs USING(plate_id)
             WHERE jobs.job_id=?1",
                [plate.job_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, u32>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(restored.0, imported.project_id.to_string());
        assert_eq!(restored.1, plate.plate_id.to_string());
        assert_eq!(restored.2, plate.job_id.to_string());
        assert_eq!(restored.3, r#"{"kind":"skipped"}"#);
        assert_eq!(restored.4, 0);
        assert_eq!(restored.5, asset_id);
        assert_eq!(restored.6, asset_id);
        assert_eq!(
            target
                .connection
                .query_row("SELECT COUNT(*) FROM job_consumption", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            target
                .connection
                .query_row("SELECT COUNT(*) FROM ledger_events", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            target
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM ledger_events WHERE event_type='reversal'",
                    [],
                    |row| row.get::<_, u32>(0)
                )
                .unwrap(),
            1
        );
        assert_eq!(
            target
                .connection
                .query_row("SELECT COUNT(*) FROM job_mappings", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            3
        );
        assert_eq!(
            target
                .connection
                .query_row("SELECT remaining_grams FROM spools", [], |row| row
                    .get::<_, f64>(0))
                .unwrap(),
            812.5
        );
        assert_eq!(target.connection.query_row("SELECT COUNT(*) FROM print_jobs WHERE job_id=?1 AND outcome='{\"kind\":\"success\"}' AND settlement_version=1",[settled_job.to_string()],|row|row.get::<_,u32>(0)).unwrap(),1);
        let parsed_json: String = target
            .connection
            .query_row("SELECT parsed_json FROM parse_cache", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&parsed_json).unwrap()["version"],
            2
        );
        assert_eq!(
            fs::read(target_root.join(relative_path)).unwrap(),
            media_bytes
        );

        std::fs::remove_file(automatic).unwrap();
        drop(target);
        drop(service);
        fs::remove_dir_all(source_root).unwrap();
        fs::remove_dir_all(target_root).unwrap();
    }

    #[test]
    fn version_two_backup_with_legacy_at_base_still_matches_after_restore() {
        let source = populated();
        let spool_id: Uuid = source
            .connection
            .query_row("SELECT spool_id FROM spools", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap()
            .parse()
            .unwrap();
        let path = std::env::temp_dir().join(format!("legacy-base-backup-{}.json", Uuid::new_v4()));
        export_to_path(&source, &path).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let spool = value["spools"][0].as_object_mut().unwrap();
        spool.insert(
            "preset_id".to_owned(),
            serde_json::json!("Bambu PLA Basic @BBL X1C"),
        );
        spool.insert(
            "preset_base".to_owned(),
            serde_json::json!("Bambu PLA Basic @base"),
        );
        spool.insert(
            "series".to_owned(),
            serde_json::json!("Catalog series ignored for base matching"),
        );
        spool.insert("color_hex".to_owned(), serde_json::json!("#FF0000"));
        spool.insert("color_hexes".to_owned(), serde_json::json!(["#FF0000"]));
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let mut target = AppDatabase::open_in_memory().unwrap();
        let automatic = import_from_path(&mut target, &path).unwrap();
        let mut service = PrintService::with_stability_delay(target, Duration::ZERO);
        let preview = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();
        let basic = preview
            .filaments
            .iter()
            .find(|filament| filament.tool == 0)
            .unwrap();

        assert_eq!(basic.candidate_spool_ids, vec![spool_id]);
        assert_eq!(basic.suggested_spool_id, Some(spool_id));

        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(automatic).unwrap();
    }

    #[test]
    fn version_two_restore_backfills_one_plate_project_without_changing_ledger_truth() {
        let source = populated();
        let source_hash = "a".repeat(64);
        let job_id = Uuid::new_v4().to_string();
        let parsed = crate::parser::parse_3mf(&fixture("bambu_multicolor.3mf")).unwrap();
        source.connection.execute("INSERT INTO parse_cache(source_hash,source_file_name,parsed_json,parse_count) VALUES(?1,'legacy.gcode.3mf',?2,1)",params![source_hash,serde_json::to_string(&parsed).unwrap()]).unwrap();
        source.connection.execute("INSERT INTO print_jobs(job_id,source_hash,source_file_name) VALUES(?1,?2,'legacy.gcode.3mf')",params![job_id,source_hash]).unwrap();
        let path = std::env::temp_dir().join(format!("legacy-v2-history-{}.json", Uuid::new_v4()));
        export_to_path(&source, &path).unwrap();
        let mut value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        value["schema_version"] = serde_json::json!(2);
        for field in ["media", "projects", "plates"] {
            value.as_object_mut().unwrap().remove(field);
        }
        value["jobs"][0].as_object_mut().unwrap().remove("plate_id");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

        let mut target = AppDatabase::open_in_memory().unwrap();
        let automatic = import_from_path(&mut target, &path).unwrap();
        let counts = target
            .connection
            .query_row(
                "SELECT (SELECT COUNT(*) FROM print_projects),
                    (SELECT COUNT(*) FROM print_plates),
                    (SELECT COUNT(*) FROM print_jobs WHERE plate_id IS NOT NULL),
                    (SELECT COUNT(*) FROM ledger_events)",
                [],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, u32>(2)?,
                        row.get::<_, u32>(3)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(counts, (1, 1, 1, 1));
        let cached: String = target
            .connection
            .query_row(
                "SELECT parsed_json FROM parse_cache WHERE source_hash=?1",
                [source_hash],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&cached).unwrap()["version"],
            2
        );
        assert_eq!(
            target
                .connection
                .query_row("SELECT remaining_grams FROM spools", [], |row| row
                    .get::<_, f64>(0))
                .unwrap(),
            812.5
        );

        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(automatic).unwrap();
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
