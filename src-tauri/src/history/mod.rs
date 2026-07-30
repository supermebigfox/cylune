use crate::{
    domain::{PlateStatus, PrintPlateSummary, PrintProjectDetail, PrintProjectSummary},
    error::{AppError, Result},
    imports::{FilamentPreview, ImportState, PrintService, PrintState},
    parser::ParsedPrintFile,
};
use rusqlite::{params, params_from_iter};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryFilter {
    Pending,
    History,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportProjectPreview {
    pub project_id: Uuid,
    pub source_hash: String,
    pub source_file_name: String,
    pub imported_at: String,
    pub plates: Vec<ImportPlatePreview>,
    pub state: ImportState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportPlatePreview {
    pub plate_id: Uuid,
    pub job_id: Uuid,
    pub plate_index: u32,
    pub thumbnail_url: Option<String>,
    pub estimated_seconds: Option<u32>,
    pub max_layer: u32,
    pub filaments: Vec<FilamentPreview>,
    pub status: PlateStatus,
}

pub(crate) fn status_for_job(
    outcome: Option<&str>,
    mapping_count: u32,
    filament_count: usize,
) -> Result<PlateStatus> {
    let Some(outcome) = outcome else {
        return Ok(if mapping_count == filament_count as u32 {
            PlateStatus::Ready
        } else {
            PlateStatus::PendingMapping
        });
    };
    let kind = serde_json::from_str::<serde_json::Value>(outcome)
        .ok()
        .and_then(|value| value.get("kind")?.as_str().map(str::to_owned))
        .ok_or_else(|| AppError::Database("invalid job outcome".to_owned()))?;
    match kind.as_str() {
        "success" => Ok(PlateStatus::Success),
        "failed" => Ok(PlateStatus::Failed),
        "cancelled" => Ok(PlateStatus::Cancelled),
        "estimated" => Ok(PlateStatus::Estimated),
        "skipped" => Ok(PlateStatus::Skipped),
        _ => Err(AppError::Database("invalid job outcome".to_owned())),
    }
}

#[derive(Debug)]
struct ProjectAggregate {
    project_id: String,
    source_hash: String,
    source_file_name: String,
    source_path: Option<String>,
    imported_at: String,
    plate_count: u32,
    total_estimated_seconds: Option<u32>,
    cover_asset_id: Option<String>,
    cover_relative_path: Option<String>,
}

fn asset_url(app_data_root: &Path, relative_path: Option<String>) -> Option<String> {
    relative_path.and_then(|relative_path| {
        let absolute = app_data_root.join(relative_path);
        let absolute = absolute.to_str()?;
        let mut encoded = String::with_capacity(absolute.len());
        for byte in absolute.bytes() {
            if byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
                )
            {
                encoded.push(char::from(byte));
            } else {
                use std::fmt::Write as _;
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
        #[cfg(any(target_os = "windows", target_os = "android"))]
        return Some(format!("http://asset.localhost/{encoded}"));
        #[cfg(not(any(target_os = "windows", target_os = "android")))]
        Some(format!("asset://localhost/{encoded}"))
    })
}

fn app_data_root(service: &PrintService) -> Result<PathBuf> {
    let database_path: String =
        service
            .database
            .connection
            .query_row("PRAGMA database_list", [], |row| row.get(2))?;
    if database_path.is_empty() {
        return Err(AppError::Database(
            "history media requires a file-backed database".to_owned(),
        ));
    }
    PathBuf::from(database_path)
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::Database("invalid history database path".to_owned()))
}

fn parse_uuid(value: &str) -> Result<Uuid> {
    value
        .parse()
        .map_err(|_| AppError::Database("invalid history uuid".to_owned()))
}

fn project_aggregates(
    service: &PrintService,
    filter: Option<HistoryFilter>,
    project_id: Option<Uuid>,
) -> Result<Vec<ProjectAggregate>> {
    let filter = filter.map(|filter| match filter {
        HistoryFilter::Pending => "pending",
        HistoryFilter::History => "history",
    });
    let project_id = project_id.map(|id| id.to_string());
    let mut statement = service.database.connection.prepare(
        "SELECT
            projects.project_id,
            projects.source_hash,
            projects.source_file_name,
            projects.source_path,
            projects.imported_at,
            COUNT(plates.plate_id),
            SUM(plates.estimated_seconds),
            projects.cover_asset_id,
            cover.relative_path
         FROM print_projects AS projects
         JOIN print_plates AS plates ON plates.project_id = projects.project_id
         LEFT JOIN media_assets AS cover
           ON cover.asset_id = projects.cover_asset_id
         WHERE (?1 IS NULL OR projects.project_id = ?1)
           AND (
             ?2 IS NULL
             OR EXISTS (
               SELECT 1
               FROM print_plates AS filtered_plates
               JOIN print_jobs AS filtered_jobs
                 ON filtered_jobs.plate_id = filtered_plates.plate_id
               WHERE filtered_plates.project_id = projects.project_id
                 AND (
                   (?2 = 'pending' AND filtered_jobs.outcome IS NULL)
                   OR (?2 = 'history' AND filtered_jobs.outcome IS NOT NULL)
                 )
             )
           )
         GROUP BY
            projects.project_id,
            projects.source_hash,
            projects.source_file_name,
            projects.source_path,
            projects.imported_at,
            projects.cover_asset_id,
            cover.relative_path
         ORDER BY projects.imported_at DESC, projects.project_id",
    )?;
    let rows = statement
        .query_map(params![project_id, filter], |row| {
            Ok(ProjectAggregate {
                project_id: row.get(0)?,
                source_hash: row.get(1)?,
                source_file_name: row.get(2)?,
                source_path: row.get(3)?,
                imported_at: row.get(4)?,
                plate_count: row.get(5)?,
                total_estimated_seconds: row.get(6)?,
                cover_asset_id: row.get(7)?,
                cover_relative_path: row.get(8)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn plate_summaries(
    service: &PrintService,
    project_ids: &[String],
    app_data_root: &Path,
) -> Result<HashMap<String, Vec<PrintPlateSummary>>> {
    if project_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let placeholders = (1..=project_ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "WITH mapping_counts AS (
           SELECT job_id, COUNT(*) AS mapping_count
           FROM job_mappings
           GROUP BY job_id
         ),
         current_jobs AS (
           SELECT jobs.*
           FROM print_jobs AS jobs
           WHERE NOT EXISTS (
             SELECT 1
             FROM print_jobs AS newer
             WHERE newer.plate_id = jobs.plate_id
               AND (
                 newer.created_at > jobs.created_at
                 OR (
                   newer.created_at = jobs.created_at
                   AND newer.job_id > jobs.job_id
                 )
               )
           )
         )
         SELECT
           plates.plate_id,
           plates.project_id,
           plates.plate_index,
           plates.display_name,
           plates.thumbnail_asset_id,
           thumbnails.relative_path,
           plates.estimated_seconds,
           plates.max_layer,
           jobs.outcome,
           COALESCE(mapping_counts.mapping_count, 0),
           plates.parsed_json
         FROM print_plates AS plates
         JOIN current_jobs AS jobs ON jobs.plate_id = plates.plate_id
         LEFT JOIN mapping_counts ON mapping_counts.job_id = jobs.job_id
         LEFT JOIN media_assets AS thumbnails
           ON thumbnails.asset_id = plates.thumbnail_asset_id
         WHERE plates.project_id IN ({placeholders})
         ORDER BY plates.project_id, plates.plate_index"
    );
    let mut statement = service.database.connection.prepare(&sql)?;
    let rows = statement
        .query_map(params_from_iter(project_ids.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<u32>>(6)?,
                row.get::<_, u32>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, u32>(9)?,
                row.get::<_, String>(10)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let mut grouped = HashMap::<String, Vec<PrintPlateSummary>>::new();
    for (
        plate_id,
        project_id,
        plate_index,
        display_name,
        thumbnail_asset_id,
        thumbnail_relative_path,
        estimated_seconds,
        max_layer,
        outcome,
        mapping_count,
        parsed_json,
    ) in rows
    {
        let parsed: ParsedPrintFile = serde_json::from_str(&parsed_json)
            .map_err(|_| AppError::Database("invalid plate history".to_owned()))?;
        let status = status_for_job(outcome.as_deref(), mapping_count, parsed.filaments.len())?;
        grouped
            .entry(project_id.clone())
            .or_default()
            .push(PrintPlateSummary {
                plate_id: parse_uuid(&plate_id)?,
                project_id: parse_uuid(&project_id)?,
                plate_index,
                display_name,
                thumbnail_asset_id,
                thumbnail_url: asset_url(app_data_root, thumbnail_relative_path),
                estimated_seconds,
                max_layer,
                status,
            });
    }
    Ok(grouped)
}

impl PrintService {
    pub fn list_print_projects(&self, filter: HistoryFilter) -> Result<Vec<PrintProjectSummary>> {
        let app_data_root = app_data_root(self)?;
        let aggregates = project_aggregates(self, Some(filter), None)?;
        let project_ids = aggregates
            .iter()
            .map(|project| project.project_id.clone())
            .collect::<Vec<_>>();
        let mut plates_by_project = plate_summaries(self, &project_ids, &app_data_root)?;
        aggregates
            .into_iter()
            .map(|project| {
                let project_id = parse_uuid(&project.project_id)?;
                Ok(PrintProjectSummary {
                    project_id,
                    source_file_name: project.source_file_name,
                    imported_at: project.imported_at,
                    plate_count: project.plate_count,
                    total_estimated_seconds: project.total_estimated_seconds,
                    cover_asset_id: project.cover_asset_id,
                    cover_url: asset_url(&app_data_root, project.cover_relative_path),
                    plates: plates_by_project
                        .remove(&project.project_id)
                        .unwrap_or_default(),
                })
            })
            .collect()
    }

    pub fn get_print_project(&self, project_id: Uuid) -> Result<PrintProjectDetail> {
        let app_data_root = app_data_root(self)?;
        let mut aggregates = project_aggregates(self, None, Some(project_id))?;
        let project = aggregates.pop().ok_or(AppError::InvalidJob)?;
        let mut plates_by_project = plate_summaries(
            self,
            std::slice::from_ref(&project.project_id),
            &app_data_root,
        )?;
        Ok(PrintProjectDetail {
            project_id,
            source_hash: project.source_hash,
            source_file_name: project.source_file_name,
            source_path: project.source_path,
            imported_at: project.imported_at,
            plate_count: project.plate_count,
            total_estimated_seconds: project.total_estimated_seconds,
            cover_asset_id: project.cover_asset_id,
            cover_url: asset_url(&app_data_root, project.cover_relative_path),
            plates: plates_by_project
                .remove(&project.project_id)
                .unwrap_or_default(),
        })
    }
}

#[tauri::command]
pub fn list_print_projects(
    filter: HistoryFilter,
    state: tauri::State<'_, PrintState>,
) -> Result<Vec<PrintProjectSummary>> {
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    service.list_print_projects(filter)
}

#[tauri::command]
pub fn get_print_project(
    project_id: Uuid,
    state: tauri::State<'_, PrintState>,
) -> Result<PrintProjectDetail> {
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    service.get_print_project(project_id)
}

#[cfg(test)]
mod tests {
    use super::{status_for_job, HistoryFilter};
    use crate::{
        db::AppDatabase,
        domain::PlateStatus,
        imports::PrintService,
        parser::{gcode::GcodeReport, ParsedPrintFile},
    };
    use rusqlite::params;
    use std::{collections::BTreeMap, fs, path::Path};
    use uuid::Uuid;

    fn insert_project(
        database: &AppDatabase,
        project_id: Uuid,
        source_hash: &str,
        file_name: &str,
        imported_at: &str,
        estimated_seconds: [u32; 2],
        outcomes: [Option<&str>; 2],
        asset_id: Option<&str>,
    ) {
        let parsed = serde_json::to_string(&ParsedPrintFile {
            filaments: Vec::new(),
            gcode: GcodeReport {
                layers: Vec::new(),
                totals_mm: BTreeMap::new(),
                max_layer: 4,
                declared_estimated_seconds: None,
                declared_total_layers: None,
            },
        })
        .unwrap();
        if let Some(asset_id) = asset_id {
            database
                .connection
                .execute(
                    "INSERT INTO media_assets (
                        asset_id, relative_path, mime_type, byte_size, width, height
                     ) VALUES (?1, ?2, 'image/png', 1, 1, 1)",
                    params![
                        asset_id,
                        format!("media/{}/{}.png", &asset_id[..2], asset_id)
                    ],
                )
                .unwrap();
        }
        database
            .connection
            .execute(
                "INSERT INTO print_projects (
                    project_id, source_hash, source_file_name, source_path,
                    imported_at, plate_count, cover_asset_id
                 ) VALUES (?1, ?2, ?3, NULL, ?4, 2, ?5)",
                params![
                    project_id.to_string(),
                    source_hash,
                    file_name,
                    imported_at,
                    asset_id
                ],
            )
            .unwrap();
        for (offset, (seconds, outcome)) in estimated_seconds.into_iter().zip(outcomes).enumerate()
        {
            let plate_id = Uuid::new_v4();
            database
                .connection
                .execute(
                    "INSERT INTO print_plates (
                        plate_id, project_id, plate_index, display_name,
                        thumbnail_asset_id, estimated_seconds, max_layer, parsed_json
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 4, ?7)",
                    params![
                        plate_id.to_string(),
                        project_id.to_string(),
                        offset as u32 + 1,
                        format!("Plate {}", offset + 1),
                        asset_id,
                        seconds,
                        parsed
                    ],
                )
                .unwrap();
            database
                .connection
                .execute(
                    "INSERT INTO print_jobs (
                        job_id, source_hash, source_file_name, outcome,
                        settlement_version, created_at, plate_id
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        Uuid::new_v4().to_string(),
                        source_hash,
                        file_name,
                        outcome,
                        u32::from(outcome.is_some()),
                        imported_at,
                        plate_id.to_string()
                    ],
                )
                .unwrap();
        }
    }

    #[test]
    fn job_state_maps_to_the_six_plate_status_words() {
        assert_eq!(
            status_for_job(None, 0, 1).unwrap(),
            PlateStatus::PendingMapping
        );
        assert_eq!(status_for_job(None, 1, 1).unwrap(), PlateStatus::Ready);
        assert_eq!(
            status_for_job(Some(r#"{"kind":"success"}"#), 0, 1).unwrap(),
            PlateStatus::Success
        );
        assert_eq!(
            status_for_job(Some(r#"{"kind":"failed","stop_layer":2}"#), 0, 1).unwrap(),
            PlateStatus::Failed
        );
        assert_eq!(
            status_for_job(Some(r#"{"kind":"cancelled","stop_layer":2}"#), 0, 1).unwrap(),
            PlateStatus::Cancelled
        );
        assert_eq!(
            status_for_job(Some(r#"{"kind":"skipped"}"#), 0, 1).unwrap(),
            PlateStatus::Skipped
        );
    }

    #[test]
    fn estimated_settlement_remains_visible_in_history() {
        assert_eq!(
            status_for_job(
                Some(r#"{"kind":"estimated","progress_percent":42.5}"#),
                0,
                1
            )
            .unwrap(),
            PlateStatus::Estimated
        );
    }

    #[test]
    fn asset_url_encodes_the_absolute_app_data_media_path() {
        assert_eq!(
            super::asset_url(
                Path::new("/tmp/CYLUNE media"),
                Some("media/aa/hash #1.png".to_owned())
            )
            .as_deref(),
            Some("asset://localhost/%2Ftmp%2FCYLUNE%20media%2Fmedia%2Faa%2Fhash%20%231.png")
        );
    }

    #[test]
    fn history_lists_projects_once_and_summarizes_plates() {
        let root = std::env::temp_dir().join(format!("cylune-history-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let database = AppDatabase::open(root.join("inventory.sqlite")).unwrap();
        let pending_id = Uuid::new_v4();
        let partial_id = Uuid::new_v4();
        let media_hash = "a".repeat(64);
        insert_project(
            &database,
            pending_id,
            &"1".repeat(64),
            "pending.3mf",
            "2026-07-30 10:00:00",
            [120, 180],
            [None, None],
            Some(&media_hash),
        );
        insert_project(
            &database,
            partial_id,
            &"2".repeat(64),
            "partial.3mf",
            "2026-07-30 11:00:00",
            [300, 420],
            [Some(r#"{"kind":"success"}"#), None],
            None,
        );
        let service = PrintService::new(database);

        let pending = service.list_print_projects(HistoryFilter::Pending).unwrap();
        let history = service.list_print_projects(HistoryFilter::History).unwrap();

        let pending_project = pending
            .iter()
            .find(|project| project.project_id == pending_id)
            .unwrap();
        assert_eq!(
            pending
                .iter()
                .filter(|project| project.project_id == pending_id)
                .count(),
            1
        );
        assert_eq!(pending_project.plate_count, 2);
        assert_eq!(pending_project.total_estimated_seconds, Some(300));
        assert_eq!(pending_project.plates.len(), 2);
        let expected_url = super::asset_url(
            &fs::canonicalize(&root).unwrap(),
            Some(format!("media/{}/{}.png", &media_hash[..2], media_hash)),
        )
        .unwrap();
        assert_eq!(
            pending_project.cover_url.as_deref(),
            Some(expected_url.as_str())
        );
        assert!(pending_project
            .plates
            .iter()
            .all(|plate| plate.status == PlateStatus::Ready));
        assert!(pending_project
            .plates
            .iter()
            .all(|plate| { plate.thumbnail_url.as_deref() == Some(expected_url.as_str()) }));

        let partial_project = history
            .iter()
            .find(|project| project.project_id == partial_id)
            .unwrap();
        assert_eq!(
            history
                .iter()
                .filter(|project| project.project_id == partial_id)
                .count(),
            1
        );
        assert_eq!(partial_project.plate_count, 2);
        assert_eq!(partial_project.total_estimated_seconds, Some(720));
        assert_eq!(
            partial_project
                .plates
                .iter()
                .map(|plate| plate.status)
                .collect::<Vec<_>>(),
            vec![PlateStatus::Success, PlateStatus::Ready]
        );

        let detail = service.get_print_project(partial_id).unwrap();
        assert_eq!(detail.project_id, partial_id);
        assert_eq!(detail.plate_count, 2);
        assert_eq!(detail.total_estimated_seconds, Some(720));
        assert_eq!(detail.plates, partial_project.plates);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }
}
