use crate::{
    domain::PlateStatus,
    error::{AppError, Result},
    imports::{FilamentPreview, ImportState},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
        "skipped" => Ok(PlateStatus::Skipped),
        _ => Err(AppError::Database("invalid job outcome".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::status_for_job;
    use crate::domain::PlateStatus;

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
}
