#[cfg(test)]
mod tests {
    use super::{FileStability, ImportState, PrintService, ToolMapping};
    use crate::{
        db::AppDatabase,
        domain::Confidence,
        inventory::{InventoryService, NewSpool},
    };
    use std::path::PathBuf;
    use std::time::Duration;
    use std::{fs, fs::File, io::Write};

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
    fn settled_duplicate_requires_confirmation_then_creates_a_fresh_job_from_one_parse() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let basic = inventory
            .create_spool(new_spool("Bambu PLA Basic @BBL A1", "#FF0000"))
            .unwrap();
        let matte = inventory
            .create_spool(new_spool("Bambu PLA Matte @BBL A1", "#00FF00"))
            .unwrap();
        let database = inventory.into_database();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = fixture("bambu_multicolor.3mf");
        let first = service.import_print_file(&path).unwrap();
        assert_eq!(first.state, ImportState::New);
        service
            .confirm_job_mapping(
                first.job_id,
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
        let first_settlement = service
            .settle_job(first.job_id, crate::domain::JobOutcome::Success)
            .unwrap();
        let after_first = [
            service.spool_balance(basic).unwrap(),
            service.spool_balance(matte).unwrap(),
        ];

        let duplicate = service.import_print_file(&path).unwrap();

        assert_eq!(duplicate.job_id, first.job_id);
        assert_eq!(duplicate.state, ImportState::NewPrintConfirmationRequired);
        assert_eq!(
            serde_json::to_value(&duplicate).unwrap()["state"],
            "new_print_confirmation_required"
        );
        assert_eq!(
            [
                service.spool_balance(basic).unwrap(),
                service.spool_balance(matte).unwrap(),
            ],
            after_first
        );

        let second = service.confirm_new_print(&first.source_hash).unwrap();
        assert_ne!(second.job_id, first.job_id);
        assert_eq!(second.state, ImportState::New);
        assert_eq!(service.parse_result_count(&first.source_hash).unwrap(), 1);
        service
            .confirm_job_mapping(
                second.job_id,
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
        let second_settlement = service
            .settle_job(second.job_id, crate::domain::JobOutcome::Success)
            .unwrap();

        for first_item in first_settlement.consumption {
            let second_item = second_settlement
                .consumption
                .iter()
                .find(|item| item.spool_id == first_item.spool_id)
                .unwrap();
            assert!((first_item.grams - second_item.grams).abs() < 1e-9);
        }
        assert!(service.spool_balance(basic).unwrap() < after_first[0]);
        assert!(service.spool_balance(matte).unwrap() < after_first[1]);
    }

    #[test]
    fn repeated_unsettled_import_reuses_the_pending_job() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let first = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();
        let repeated = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();

        assert_eq!(repeated.job_id, first.job_id);
        assert_eq!(repeated.state, ImportState::ExistingPending);
        assert_eq!(service.job_count(&first.source_hash).unwrap(), 1);
    }

    #[test]
    fn pending_job_can_be_reopened_by_id_without_source_file_access() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let imported = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();
        let reopened = service.get_job_preview(imported.job_id).unwrap();
        assert_eq!(reopened.job_id, imported.job_id);
        assert_eq!(reopened.filaments, imported.filaments);
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

    #[cfg(unix)]
    #[test]
    fn file_stability_rejects_links_and_non_files() {
        use std::os::unix::fs::symlink;
        let directory =
            std::env::temp_dir().join(format!("bambu-pools-stability-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let file = directory.join("source.3mf");
        let link = directory.join("link.3mf");
        fs::write(&file, b"fixture").unwrap();
        symlink(&file, &link).unwrap();

        assert_eq!(
            FileStability::read(&link).unwrap_err().code(),
            "invalid_file"
        );
        assert_eq!(
            FileStability::read(&directory).unwrap_err().code(),
            "invalid_file"
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn changed_file_is_rejected_before_new_parse_or_job_persistence() {
        let path = std::env::temp_dir().join(format!(
            "bambu-pools-changing-new-{}.3mf",
            uuid::Uuid::new_v4()
        ));
        fs::copy(fixture("bambu_multicolor.3mf"), &path).unwrap();
        let original_hash = super::sha256(&path).unwrap();
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        service.before_final_stability_check = Some(Box::new(|path| {
            fs::OpenOptions::new()
                .append(true)
                .open(path)
                .unwrap()
                .write_all(b"changed")
                .unwrap();
        }));

        let error = service.import_print_file(&path).unwrap_err();

        assert_eq!(error.code(), "file_not_stable");
        assert_eq!(service.parse_result_count(&original_hash).unwrap(), 0);
        assert_eq!(service.job_count(&original_hash).unwrap(), 0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn changed_cached_file_is_rejected_before_returning_a_preview() {
        let path = std::env::temp_dir().join(format!(
            "bambu-pools-changing-cached-{}.3mf",
            uuid::Uuid::new_v4()
        ));
        fs::copy(fixture("bambu_multicolor.3mf"), &path).unwrap();
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let first = service.import_print_file(&path).unwrap();
        service.before_final_stability_check = Some(Box::new(|path| {
            fs::OpenOptions::new()
                .append(true)
                .open(path)
                .unwrap()
                .write_all(b"changed")
                .unwrap();
        }));

        let error = service.import_print_file(&path).unwrap_err();

        assert_eq!(error.code(), "file_not_stable");
        assert_eq!(service.parse_result_count(&first.source_hash).unwrap(), 1);
        assert_eq!(service.job_count(&first.source_hash).unwrap(), 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn standalone_gcode_without_profiles_never_creates_a_job() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let error = service
            .import_print_file(&fixture("single_color.gcode"))
            .unwrap_err();
        assert_eq!(error.code(), "standalone_gcode_profiles_required");
        assert_eq!(service.job_count("unused").unwrap(), 0);
    }

    #[test]
    fn duplicate_profile_tool_ids_are_rejected_before_job_creation() {
        let path = std::env::temp_dir().join(format!(
            "bambu-pools-duplicate-profiles-{}.3mf",
            uuid::Uuid::new_v4()
        ));
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::FileOptions::default();
        let config = br##"{"filament_settings_id":["Bambu PLA Basic @BBL A1"],"filament_type":["PLA"],"filament_colour":["#FF0000"],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##;
        for name in [
            "Metadata/project_settings.config",
            "Metadata/filament_settings.config",
        ] {
            archive.start_file(name, options).unwrap();
            archive.write_all(config).unwrap();
        }
        archive
            .start_file("Metadata/plate_1.gcode", options)
            .unwrap();
        archive.write_all(b"M83\nT0\nG1 E10\n").unwrap();
        archive.finish().unwrap();
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);

        let error = service.import_print_file(&path).unwrap_err();

        fs::remove_file(path).unwrap();
        assert_eq!(error.code(), "invalid_mapping");
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
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(AppError::InvalidFile);
        }
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
    pub state: ImportState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingSummary {
    pub count: u32,
    pub newest_job_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportState {
    New,
    ExistingPending,
    NewPrintConfirmationRequired,
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
    #[cfg(test)]
    before_final_stability_check: Option<Box<dyn FnOnce(&Path) + Send>>,
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
            #[cfg(test)]
            before_final_stability_check: None,
        }
    }

    pub fn import_print_file(&mut self, path: &Path) -> Result<ImportPreview> {
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gcode"))
        {
            return Err(AppError::StandaloneGcodeProfilesRequired);
        }
        let stability = self.ensure_stable(path)?;
        let source_hash = sha256(path)?;

        if let Some((file_name, parsed)) = self.persisted_parse(&source_hash)? {
            validate_profiles(&parsed)?;
            if let Some(job_id) = self.pending_job(&source_hash)? {
                let preview = self.preview(
                    job_id,
                    source_hash,
                    file_name,
                    &parsed,
                    ImportState::ExistingPending,
                )?;
                #[cfg(test)]
                self.run_before_final_stability_check(path);
                self.ensure_unchanged(path, stability)?;
                return Ok(preview);
            }
            if let Some(job_id) = self.latest_job(&source_hash)? {
                let preview = self.preview(
                    job_id,
                    source_hash,
                    file_name,
                    &parsed,
                    ImportState::NewPrintConfirmationRequired,
                )?;
                #[cfg(test)]
                self.run_before_final_stability_check(path);
                self.ensure_unchanged(path, stability)?;
                return Ok(preview);
            }
        }

        let parsed = parse_3mf(path)?;
        validate_profiles(&parsed)?;
        let job_id = Uuid::new_v4();
        let source_file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or(AppError::InvalidFile)?
            .to_owned();
        let parsed_json = serde_json::to_string(&parsed)
            .map_err(|error| AppError::Database(error.to_string()))?;
        #[cfg(test)]
        self.run_before_final_stability_check(path);
        self.ensure_unchanged(path, stability)?;
        let transaction = self.database.connection.transaction()?;
        transaction.execute(
            "INSERT INTO parse_cache (source_hash, source_file_name, parsed_json, parse_count) VALUES (?1, ?2, ?3, 1)",
            params![source_hash, source_file_name, parsed_json],
        )?;
        transaction.execute(
            "INSERT INTO print_jobs (job_id, source_hash, source_file_name) VALUES (?1, ?2, ?3)",
            params![job_id.to_string(), source_hash, source_file_name],
        )?;
        transaction.commit()?;

        self.preview(
            job_id,
            source_hash,
            source_file_name,
            &parsed,
            ImportState::New,
        )
    }

    pub fn pending_summary(&self) -> Result<PendingSummary> {
        let count = self.database.connection.query_row(
            "SELECT COUNT(*) FROM print_jobs WHERE outcome IS NULL",
            [],
            |row| row.get(0),
        )?;
        let newest_job_id: Option<String> = self
            .database
            .connection
            .query_row(
                "SELECT job_id FROM print_jobs WHERE outcome IS NULL ORDER BY rowid DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(PendingSummary {
            count,
            newest_job_id: newest_job_id
                .map(|job_id| {
                    job_id
                        .parse()
                        .map_err(|_| AppError::Database("invalid job id".to_owned()))
                })
                .transpose()?,
        })
    }

    pub fn parse_result_count(&self, source_hash: &str) -> Result<u32> {
        let count = self
            .database
            .connection
            .query_row(
                "SELECT COALESCE(parse_count, 0) FROM parse_cache WHERE source_hash = ?1",
                params![source_hash],
                |row| row.get(0),
            )
            .optional()?;
        Ok(count.unwrap_or(0))
    }

    pub fn job_count(&self, source_hash: &str) -> Result<u32> {
        self.database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM print_jobs WHERE source_hash = ?1",
                params![source_hash],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn get_job_preview(&self, job_id: Uuid) -> Result<ImportPreview> {
        let (source_hash, source_file_name, outcome): (String, String, Option<String>) = self
            .database
            .connection
            .query_row(
                "SELECT source_hash,source_file_name,outcome FROM print_jobs WHERE job_id=?1",
                params![job_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(AppError::InvalidJob)?;
        let (_, parsed) = self
            .persisted_parse(&source_hash)?
            .ok_or(AppError::InvalidJob)?;
        self.preview(
            job_id,
            source_hash,
            source_file_name,
            &parsed,
            if outcome.is_none() {
                ImportState::ExistingPending
            } else {
                ImportState::NewPrintConfirmationRequired
            },
        )
    }

    pub fn confirm_new_print(&mut self, source_hash: &str) -> Result<ImportPreview> {
        let (source_file_name, parsed) = self
            .persisted_parse(source_hash)?
            .ok_or(AppError::InvalidJob)?;
        validate_profiles(&parsed)?;
        if let Some(job_id) = self.pending_job(source_hash)? {
            return self.preview(
                job_id,
                source_hash.to_owned(),
                source_file_name,
                &parsed,
                ImportState::ExistingPending,
            );
        }

        let job_id = Uuid::new_v4();
        self.database.connection.execute(
            "INSERT INTO print_jobs (job_id, source_hash, source_file_name) VALUES (?1, ?2, ?3)",
            params![job_id.to_string(), source_hash, source_file_name],
        )?;
        self.preview(
            job_id,
            source_hash.to_owned(),
            source_file_name,
            &parsed,
            ImportState::New,
        )
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
        if expected_tools.len() != parsed.filaments.len()
            || mappings.len() != expected_tools.len()
            || actual_tools != expected_tools
        {
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
                "SELECT parse_cache.parsed_json FROM print_jobs JOIN parse_cache USING (source_hash) WHERE print_jobs.job_id = ?1",
                params![job_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let json = json.ok_or(AppError::InvalidJob)?;
        serde_json::from_str(&json).map_err(|error| AppError::Database(error.to_string()))
    }

    fn ensure_stable(&self, path: &Path) -> Result<FileStability> {
        let first = FileStability::read(path)?;
        thread::sleep(self.stability_delay);
        let second = FileStability::read(path).map_err(|_| AppError::FileNotStable)?;
        if first.is_same_as(&second) {
            Ok(first)
        } else {
            Err(AppError::FileNotStable)
        }
    }

    fn ensure_unchanged(&self, path: &Path, expected: FileStability) -> Result<()> {
        let current = FileStability::read(path).map_err(|_| AppError::FileNotStable)?;
        if expected.is_same_as(&current) {
            Ok(())
        } else {
            Err(AppError::FileNotStable)
        }
    }

    #[cfg(test)]
    fn run_before_final_stability_check(&mut self, path: &Path) {
        if let Some(hook) = self.before_final_stability_check.take() {
            hook(path);
        }
    }

    fn persisted_parse(&self, source_hash: &str) -> Result<Option<(String, ParsedPrintFile)>> {
        let row: Option<(String, String)> = self
            .database
            .connection
            .query_row(
                "SELECT source_file_name, parsed_json FROM parse_cache WHERE source_hash = ?1",
                params![source_hash],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        row.map(|(file_name, json)| {
            let parsed = serde_json::from_str(&json)
                .map_err(|error| AppError::Database(error.to_string()))?;
            Ok((file_name, parsed))
        })
        .transpose()
    }

    fn pending_job(&self, source_hash: &str) -> Result<Option<Uuid>> {
        self.job_id_query(
            "SELECT job_id FROM print_jobs WHERE source_hash = ?1 AND outcome IS NULL ORDER BY rowid DESC LIMIT 1",
            source_hash,
        )
    }

    fn latest_job(&self, source_hash: &str) -> Result<Option<Uuid>> {
        self.job_id_query(
            "SELECT job_id FROM print_jobs WHERE source_hash = ?1 ORDER BY rowid DESC LIMIT 1",
            source_hash,
        )
    }

    fn job_id_query(&self, sql: &str, source_hash: &str) -> Result<Option<Uuid>> {
        let value: Option<String> = self
            .database
            .connection
            .query_row(sql, params![source_hash], |row| row.get(0))
            .optional()?;
        value
            .map(|value| {
                value
                    .parse()
                    .map_err(|_| AppError::Database("invalid job id".to_owned()))
            })
            .transpose()
    }

    fn preview(
        &self,
        job_id: Uuid,
        source_hash: String,
        source_file_name: String,
        parsed: &ParsedPrintFile,
        state: ImportState,
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
            state,
        })
    }

    fn matching_spools(&self, profile: &FilamentProfile) -> Result<Vec<Uuid>> {
        let mut statement = self.database.connection.prepare(
            "SELECT spool_id FROM spools WHERE status <> 'archived' AND preset_id = ?1 AND material = ?2 AND series = ?3 AND UPPER(color_hex) = UPPER(?4) ORDER BY rowid",
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

fn validate_profiles(parsed: &ParsedPrintFile) -> Result<()> {
    let unique_tools = parsed
        .filaments
        .iter()
        .map(|profile| profile.tool)
        .collect::<BTreeSet<_>>();
    if unique_tools.len() == parsed.filaments.len() {
        Ok(())
    } else {
        Err(AppError::InvalidMapping)
    }
}

pub(crate) fn sha256(path: &Path) -> Result<String> {
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
    runtime: tauri::State<'_, crate::pet::runtime::PetRuntime>,
) -> Result<ImportPreview> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    let preview = service.import_print_file(Path::new(&path))?;
    let summary = service.pending_summary()?;
    drop(service);
    runtime.refresh_pending(
        summary,
        Some(crate::pet::runtime::PetSignal::ImportSucceeded {
            job_id: preview.job_id,
            pending_count: summary.count,
        }),
    );
    Ok(preview)
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

#[tauri::command]
pub fn confirm_new_print(
    source_hash: String,
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, crate::pet::runtime::PetRuntime>,
) -> Result<ImportPreview> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    let preview = service.confirm_new_print(&source_hash)?;
    let summary = service.pending_summary()?;
    drop(service);
    runtime.refresh_pending(
        summary,
        Some(crate::pet::runtime::PetSignal::ImportSucceeded {
            job_id: preview.job_id,
            pending_count: summary.count,
        }),
    );
    Ok(preview)
}

#[tauri::command]
pub fn get_job_preview(job_id: Uuid, state: tauri::State<'_, PrintState>) -> Result<ImportPreview> {
    state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?
        .get_job_preview(job_id)
}
