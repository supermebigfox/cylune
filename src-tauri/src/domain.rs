use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spool {
    pub spool_id: Uuid,
    pub display_name: String,
    pub preset_id: Option<String>,
    pub catalog_id: Option<String>,
    pub color_name: Option<String>,
    pub color_code: Option<String>,
    #[serde(default)]
    pub color_hexes: Vec<String>,
    pub preset_base: Option<String>,
    pub brand: String,
    pub material: String,
    pub series: String,
    pub color_hex: String,
    pub remaining_grams: f64,
    pub status: SpoolStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpoolStatus {
    Available,
    Assigned,
    Empty,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotAssignment {
    pub slot_number: u8,
    pub spool_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintJob {
    pub job_id: Uuid,
    pub source_hash: String,
    pub source_file_name: String,
    pub outcome: Option<JobOutcome>,
    pub settlement_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintProjectSummary {
    pub project_id: Uuid,
    pub source_file_name: String,
    pub imported_at: String,
    pub plate_count: u32,
    pub total_estimated_seconds: Option<u32>,
    pub cover_asset_id: Option<String>,
    pub cover_url: Option<String>,
    pub plates: Vec<PrintPlateSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintProjectDetail {
    pub project_id: Uuid,
    pub source_hash: String,
    pub source_file_name: String,
    pub source_path: Option<String>,
    pub imported_at: String,
    pub plate_count: u32,
    pub total_estimated_seconds: Option<u32>,
    pub cover_asset_id: Option<String>,
    pub cover_url: Option<String>,
    pub plates: Vec<PrintPlateSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintPlateSummary {
    pub plate_id: Uuid,
    pub project_id: Uuid,
    pub plate_index: u32,
    pub display_name: Option<String>,
    pub thumbnail_asset_id: Option<String>,
    pub thumbnail_url: Option<String>,
    pub estimated_seconds: Option<u32>,
    pub max_layer: u32,
    pub status: PlateStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlateStatus {
    PendingMapping,
    Ready,
    Success,
    Failed,
    Cancelled,
    Estimated,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEvent {
    pub event_id: Uuid,
    pub idempotency_key: String,
    pub spool_id: Uuid,
    pub job_id: Option<Uuid>,
    pub settlement_version: Option<u32>,
    pub event_type: LedgerEventType,
    pub delta_grams: f64,
    pub confidence: Confidence,
    pub reverses_event_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerEventType {
    Creation,
    Settlement,
    Reversal,
    Adjustment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Exact,
    Estimated,
    NeedsConfirmation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobOutcome {
    Success,
    Failed { stop_layer: u32 },
    Cancelled { stop_layer: u32 },
    Estimated { progress_percent: f32 },
}

#[cfg(test)]
mod tests {
    use super::{
        LedgerEvent, LedgerEventType, PlateStatus, PrintPlateSummary, PrintProjectDetail,
        PrintProjectSummary, Spool, SpoolStatus,
    };
    use uuid::Uuid;

    #[test]
    fn spool_preserves_its_independent_identity_and_remaining_weight() {
        let spool_id = Uuid::new_v4();
        let spool = Spool {
            spool_id,
            display_name: "Left AMS spool".to_owned(),
            preset_id: Some("Bambu PLA Basic @BBL A1".to_owned()),
            catalog_id: None,
            color_name: None,
            color_code: None,
            color_hexes: vec!["#FFFFFF".to_owned()],
            preset_base: None,
            brand: "Bambu Lab".to_owned(),
            material: "PLA".to_owned(),
            series: "Basic".to_owned(),
            color_hex: "#FFFFFF".to_owned(),
            remaining_grams: 712.5,
            status: SpoolStatus::Available,
        };

        let value = serde_json::to_value(spool).unwrap();
        assert_eq!(value["spool_id"], spool_id.to_string());
        assert_eq!(value["remaining_grams"], 712.5);
        assert_eq!(value["status"], "available");
    }

    #[test]
    fn ledger_event_serializes_its_type_and_reversed_event_id() {
        let reversed_event_id = Uuid::new_v4();
        let event = LedgerEvent {
            event_id: Uuid::new_v4(),
            idempotency_key: "reverse-settlement-1".to_owned(),
            spool_id: Uuid::new_v4(),
            job_id: Some(Uuid::new_v4()),
            settlement_version: Some(1),
            event_type: LedgerEventType::Reversal,
            delta_grams: 18.2,
            confidence: super::Confidence::Exact,
            reverses_event_id: Some(reversed_event_id),
        };

        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["event_type"], "reversal");
        assert_eq!(value["reverses_event_id"], reversed_event_id.to_string());
    }

    #[test]
    fn ledger_event_serializes_a_creation_baseline() {
        let event = LedgerEvent {
            event_id: Uuid::new_v4(),
            idempotency_key: "spool-baseline-1".to_owned(),
            spool_id: Uuid::new_v4(),
            job_id: None,
            settlement_version: None,
            event_type: LedgerEventType::Creation,
            delta_grams: 1000.0,
            confidence: super::Confidence::Exact,
            reverses_event_id: None,
        };

        assert_eq!(
            serde_json::to_value(event).unwrap()["event_type"],
            "creation"
        );
    }

    #[test]
    fn print_project_history_dtos_serialize_ids_and_plate_status() {
        let project_id = "11111111-1111-4111-8111-111111111111".parse().unwrap();
        let plate_id = "22222222-2222-4222-8222-222222222222".parse().unwrap();
        let project = PrintProjectDetail {
            project_id,
            source_hash: "legacy-source-hash".to_owned(),
            source_file_name: "legacy.gcode.3mf".to_owned(),
            source_path: None,
            imported_at: "2026-07-30 10:00:00".to_owned(),
            plate_count: 1,
            total_estimated_seconds: Some(900),
            cover_asset_id: None,
            cover_url: None,
            plates: vec![PrintPlateSummary {
                plate_id,
                project_id,
                plate_index: 1,
                display_name: Some("Main plate".to_owned()),
                thumbnail_asset_id: None,
                thumbnail_url: None,
                estimated_seconds: Some(900),
                max_layer: 42,
                status: PlateStatus::Success,
            }],
        };
        let summary = PrintProjectSummary {
            project_id,
            source_file_name: "legacy.gcode.3mf".to_owned(),
            imported_at: "2026-07-30 10:00:00".to_owned(),
            plate_count: 1,
            total_estimated_seconds: Some(900),
            cover_asset_id: None,
            cover_url: None,
            plates: project.plates.clone(),
        };

        let detail_value = serde_json::to_value(project).unwrap();
        let summary_value = serde_json::to_value(summary).unwrap();

        assert_eq!(detail_value["project_id"], project_id.to_string());
        assert_eq!(detail_value["plates"][0]["plate_id"], plate_id.to_string());
        assert_eq!(detail_value["plates"][0]["status"], "success");
        assert_eq!(summary_value["plate_count"], 1);
    }

    #[test]
    fn plate_status_serializes_the_complete_history_vocabulary() {
        let serialized = [
            PlateStatus::PendingMapping,
            PlateStatus::Ready,
            PlateStatus::Success,
            PlateStatus::Failed,
            PlateStatus::Cancelled,
            PlateStatus::Estimated,
            PlateStatus::Skipped,
        ]
        .into_iter()
        .map(|status| serde_json::to_value(status).unwrap())
        .collect::<Vec<_>>();

        assert_eq!(
            serialized,
            vec![
                "pending_mapping",
                "ready",
                "success",
                "failed",
                "cancelled",
                "estimated",
                "skipped",
            ]
        );
    }
}
