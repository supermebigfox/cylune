use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spool {
    pub spool_id: Uuid,
    pub display_name: String,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LedgerEvent {
    pub event_id: Uuid,
    pub idempotency_key: String,
    pub spool_id: Uuid,
    pub job_id: Option<Uuid>,
    pub settlement_version: Option<u32>,
    pub delta_grams: f64,
    pub confidence: Confidence,
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
    use super::{Spool, SpoolStatus};
    use uuid::Uuid;

    #[test]
    fn spool_preserves_its_independent_identity_and_remaining_weight() {
        let spool_id = Uuid::new_v4();
        let spool = Spool {
            spool_id,
            display_name: "Left AMS spool".to_owned(),
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
}
