#[cfg(test)]
mod tests {
    use super::{FileStability, PrintService, ToolMapping};
    use crate::{
        db::AppDatabase,
        domain::Confidence,
        inventory::{InventoryService, NewSpool},
    };
    use std::path::PathBuf;
    use std::time::Duration;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn new_spool(preset_id: &str, color_hex: &str) -> NewSpool {
        NewSpool {
            display_name: format!("{preset_id} {color_hex}"),
            preset_id: Some(preset_id.to_owned()),
            brand: "Bambu Lab".to_owned(),
            material: "PLA".to_owned(),
            series: if preset_id.contains("Matte") {
                "Matte".to_owned()
            } else {
                "Basic".to_owned()
            },
            color_hex: color_hex.to_owned(),
            remaining_grams: 1000.0,
        }
    }

    #[test]
    fn duplicate_import_reuses_one_persisted_parse() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);

        let first = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();
        let second = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();

        assert_eq!(first.job_id, second.job_id);
        assert_eq!(first.source_hash, second.source_hash);
        assert_eq!(service.parse_result_count(&first.source_hash).unwrap(), 1);
    }

    #[test]
    fn unique_exact_profile_candidate_is_suggested() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let white = inventory
            .create_spool(new_spool("Bambu PLA Basic @BBL A1", "#FF0000"))
            .unwrap();
        let database = inventory.into_database();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);

        let preview = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();
        let basic = preview
            .filaments
            .iter()
            .find(|filament| filament.tool == 0)
            .unwrap();

        assert_eq!(basic.suggested_spool_id, Some(white));
        assert_eq!(basic.confidence, Confidence::Exact);
    }

    #[test]
    fn identical_exact_candidates_require_user_confirmation() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let first = inventory
            .create_spool(new_spool("Bambu PLA Basic @BBL A1", "#FF0000"))
            .unwrap();
        let second = inventory
            .create_spool(new_spool("Bambu PLA Basic @BBL A1", "#FF0000"))
            .unwrap();
        let database = inventory.into_database();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);

        let preview = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();
        let basic = preview
            .filaments
            .iter()
            .find(|filament| filament.tool == 0)
            .unwrap();

        assert_eq!(basic.suggested_spool_id, None);
        assert_eq!(basic.candidate_spool_ids, vec![first, second]);
        assert_eq!(basic.confidence, Confidence::NeedsConfirmation);
    }

    #[test]
    fn confirmed_mappings_capture_current_slot_numbers() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let basic = inventory
            .create_spool(new_spool("Bambu PLA Basic @BBL A1", "#FF0000"))
            .unwrap();
        let matte = inventory
            .create_spool(new_spool("Bambu PLA Matte @BBL A1", "#00FF00"))
            .unwrap();
        inventory.mount_spool(2, basic).unwrap();
        inventory.mount_spool(4, matte).unwrap();
        let database = inventory.into_database();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let preview = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();

        service
            .confirm_job_mapping(
                preview.job_id,
                vec![
                    ToolMapping {
                        tool: 0,
                        spool_id: basic,
                    },
                    ToolMapping {
                        tool: 1,
                        spool_id: matte,
                    },
                ],
            )
            .unwrap();

        let mappings = service.job_mappings(preview.job_id).unwrap();
        assert_eq!(mappings[0].slot_number, Some(2));
        assert_eq!(mappings[1].slot_number, Some(4));
    }

    #[test]
    fn file_stability_requires_matching_size_and_modified_time() {
        let first = FileStability {
            size: 123,
            modified_nanos: 456,
        };
        assert!(first.is_same_as(&first));
        assert!(!first.is_same_as(&FileStability {
            size: 124,
            modified_nanos: 456,
        }));
    }
}
use crate::{
    db::AppDatabase,
    domain::Confidence,
    error::{AppError, Result},
    parser::{parse_3mf, FilamentProfile, ParsedPrintFile},
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::Path,
    sync::Mutex,
    thread,
    time::{Duration, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStability {
    pub size: u64,
    pub modified_nanos: u128,
}

impl FileStability {
    fn read(path: &Path) -> Result<Self> {
        let metadata = fs::metadata(path)?;
        let modified_nanos = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AppError::InvalidFile)?
            .as_nanos();
        Ok(Self {
            size: metadata.len(),
            modified_nanos,
        })
    }

    pub fn is_same_as(&self, other: &Self) -> bool {
        self == other
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilamentPreview {
    pub tool: u8,
    pub profile: FilamentProfile,
    pub total_grams: f64,
    pub candidate_spool_ids: Vec<Uuid>,
    pub suggested_spool_id: Option<Uuid>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPreview {
    pub job_id: Uuid,
    pub source_hash: String,
    pub source_file_name: String,
    pub filaments: Vec<FilamentPreview>,
    pub max_layer: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolMapping {
    pub tool: u8,
    pub spool_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedMapping {
    pub tool: u8,
    pub spool_id: Uuid,
    pub slot_number: Option<u8>,
}

pub struct PrintService {
    pub(crate) database: AppDatabase,
    stability_delay: Duration,
}

pub type PrintState = Mutex<PrintService>;

impl PrintService {
    pub fn new(database: AppDatabase) -> Self {
        Self::with_stability_delay(database, Duration::from_millis(750))
    }

    pub fn with_stability_delay(database: AppDatabase, stability_delay: Duration) -> Self {
        Self {
            database,
            stability_delay,
        }
    }

    pub fn import_print_file(&mut self, path: &Path) -> Result<ImportPreview> {
        self.ensure_stable(path)?;
        let source_hash = sha256(path)?;

        if let Some((job_id, file_name, parsed)) = self.persisted_parse(&source_hash)? {
            return self.preview(job_id, source_hash, file_name, &parsed);
        }

        let parsed = parse_3mf(path)?;
        let job_id = Uuid::new_v4();
        let source_file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or(AppError::InvalidFile)?
            .to_owned();
        let parsed_json = serde_json::to_string(&parsed)
            .map_err(|error| AppError::Database(error.to_string()))?;
        let transaction = self.database.connection.transaction()?;
        transaction.execute(
            "INSERT INTO print_jobs (job_id, source_hash, source_file_name) VALUES (?1, ?2, ?3)",
            params![job_id.to_string(), source_hash, source_file_name],
        )?;
        transaction.execute(
            "INSERT INTO job_imports (job_id, parsed_json, parse_count) VALUES (?1, ?2, 1)",
            params![job_id.to_string(), parsed_json],
        )?;
        transaction.commit()?;

        self.preview(job_id, source_hash, source_file_name, &parsed)
    }

    pub fn parse_result_count(&self, source_hash: &str) -> Result<u32> {
        self.database
            .connection
            .query_row(
                "SELECT COALESCE(SUM(job_imports.parse_count), 0) FROM job_imports JOIN print_jobs USING (job_id) WHERE print_jobs.source_hash = ?1",
                params![source_hash],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn confirm_job_mapping(&mut self, job_id: Uuid, mappings: Vec<ToolMapping>) -> Result<()> {
        let parsed = self.parsed_job(job_id)?;
        let expected_tools = parsed
            .filaments
            .iter()
            .map(|profile| profile.tool)
            .collect::<BTreeSet<_>>();
        let actual_tools = mappings
            .iter()
            .map(|mapping| mapping.tool)
            .collect::<BTreeSet<_>>();
        if mappings.len() != expected_tools.len() || actual_tools != expected_tools {
            return Err(AppError::InvalidMapping);
        }

        let transaction = self.database.connection.transaction()?;
        for mapping in &mappings {
            let status: Option<String> = transaction
                .query_row(
                    "SELECT status FROM spools WHERE spool_id = ?1",
                    params![mapping.spool_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            if status.as_deref().is_none_or(|status| status == "archived") {
                return Err(AppError::InvalidMapping);
            }
        }

        transaction.execute(
            "DELETE FROM job_mappings WHERE job_id = ?1",
            params![job_id.to_string()],
        )?;
        for mapping in mappings {
            let slot_number: Option<u8> = transaction
                .query_row(
                    "SELECT slot_number FROM ams_slots WHERE spool_id = ?1",
                    params![mapping.spool_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            transaction.execute(
                "INSERT INTO job_mappings (job_id, tool, spool_id, slot_number) VALUES (?1, ?2, ?3, ?4)",
                params![
                    job_id.to_string(),
                    mapping.tool,
                    mapping.spool_id.to_string(),
                    slot_number,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn job_mappings(&self, job_id: Uuid) -> Result<Vec<SavedMapping>> {
        let mut statement = self.database.connection.prepare(
            "SELECT tool, spool_id, slot_number FROM job_mappings WHERE job_id = ?1 ORDER BY tool",
        )?;
        let mappings: Vec<SavedMapping> = statement
            .query_map(params![job_id.to_string()], |row| {
                let spool_id: String = row.get(1)?;
                Ok(SavedMapping {
                    tool: row.get(0)?,
                    spool_id: spool_id.parse().map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?,
                    slot_number: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(mappings)
    }

    pub(crate) fn parsed_job(&self, job_id: Uuid) -> Result<ParsedPrintFile> {
        let json: Option<String> = self
            .database
            .connection
            .query_row(
                "SELECT parsed_json FROM job_imports WHERE job_id = ?1",
                params![job_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let json = json.ok_or(AppError::InvalidJob)?;
        serde_json::from_str(&json).map_err(|error| AppError::Database(error.to_string()))
    }

    fn ensure_stable(&self, path: &Path) -> Result<()> {
        let first = FileStability::read(path)?;
        thread::sleep(self.stability_delay);
        let second = FileStability::read(path)?;
        if first.is_same_as(&second) {
            Ok(())
        } else {
            Err(AppError::FileNotStable)
        }
    }

    fn persisted_parse(
        &self,
        source_hash: &str,
    ) -> Result<Option<(Uuid, String, ParsedPrintFile)>> {
        let row: Option<(String, String, String)> = self
            .database
            .connection
            .query_row(
                "SELECT print_jobs.job_id, print_jobs.source_file_name, job_imports.parsed_json FROM print_jobs JOIN job_imports USING (job_id) WHERE print_jobs.source_hash = ?1",
                params![source_hash],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        row.map(|(job_id, file_name, json)| {
            let job_id = job_id
                .parse()
                .map_err(|_| AppError::Database("invalid job id".to_owned()))?;
            let parsed = serde_json::from_str(&json)
                .map_err(|error| AppError::Database(error.to_string()))?;
            Ok((job_id, file_name, parsed))
        })
        .transpose()
    }

    fn preview(
        &self,
        job_id: Uuid,
        source_hash: String,
        source_file_name: String,
        parsed: &ParsedPrintFile,
    ) -> Result<ImportPreview> {
        let mut filaments = Vec::with_capacity(parsed.filaments.len());
        for profile in &parsed.filaments {
            let candidates = self.matching_spools(profile)?;
            let (suggested_spool_id, confidence) = match candidates.as_slice() {
                [only] => (Some(*only), Confidence::Exact),
                _ => (None, Confidence::NeedsConfirmation),
            };
            let total_mm = parsed
                .gcode
                .totals_mm
                .get(&profile.tool)
                .copied()
                .unwrap_or(0.0);
            filaments.push(FilamentPreview {
                tool: profile.tool,
                profile: profile.clone(),
                total_grams: profile.grams_for_length_mm(total_mm),
                candidate_spool_ids: candidates,
                suggested_spool_id,
                confidence,
            });
        }
        Ok(ImportPreview {
            job_id,
            source_hash,
            source_file_name,
            filaments,
            max_layer: parsed.gcode.max_layer,
        })
    }

    fn matching_spools(&self, profile: &FilamentProfile) -> Result<Vec<Uuid>> {
        let mut statement = self.database.connection.prepare(
            "SELECT spool_id FROM spools WHERE status <> 'archived' AND preset_id = ?1 AND material = ?2 AND series = ?3 AND UPPER(color_hex) = UPPER(?4) ORDER BY created_at, spool_id",
        )?;
        let candidates: Vec<Uuid> = statement
            .query_map(
                params![
                    profile.preset_id,
                    profile.material,
                    profile.series,
                    profile.color_hex,
                ],
                |row| row.get::<_, String>(0),
            )?
            .map(|value| {
                value?.parse().map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(candidates)
    }
}

fn sha256(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) fn with_print<T>(
    state: tauri::State<'_, PrintState>,
    operation: impl FnOnce(&mut PrintService) -> Result<T>,
) -> Result<T> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    operation(&mut service)
}

#[tauri::command]
pub fn import_print_file(
    path: String,
    state: tauri::State<'_, PrintState>,
) -> Result<ImportPreview> {
    with_print(state, |service| service.import_print_file(Path::new(&path)))
}

#[tauri::command]
pub fn confirm_job_mapping(
    job_id: Uuid,
    mappings: Vec<ToolMapping>,
    state: tauri::State<'_, PrintState>,
) -> Result<()> {
    with_print(state, |service| {
        service.confirm_job_mapping(job_id, mappings)
    })
}
