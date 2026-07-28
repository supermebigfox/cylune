use crate::{
    domain::{Confidence, JobOutcome},
    error::{AppError, Result},
    imports::{with_print, PrintService, PrintState},
    inventory::status_for,
};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Consumption {
    pub spool_id: Uuid,
    pub grams: f64,
    pub confidence: Confidence,
    pub slot_number: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SettlementResult {
    pub job_id: Uuid,
    pub outcome: JobOutcome,
    pub settlement_version: u32,
    pub selected_layer: Option<u32>,
    pub confidence: Confidence,
    pub consumption: Vec<Consumption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReversalResult {
    pub job_id: Uuid,
    pub settlement_version: u32,
    pub already_reversed: bool,
    pub restored: Vec<Consumption>,
}

impl PrintService {
    fn current_settlement_version(&self, job_id: Uuid) -> Result<u32> {
        self.database
            .connection
            .query_row(
                "SELECT settlement_version FROM print_jobs WHERE job_id = ?1",
                params![job_id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(AppError::InvalidJob)
    }

    pub fn settle_job(&mut self, job_id: Uuid, outcome: JobOutcome) -> Result<SettlementResult> {
        if let Some(saved) = self.saved_settlement(job_id)? {
            if saved.outcome == outcome {
                return Ok(saved);
            }
            return Err(AppError::DuplicateJob);
        }

        let parsed = self.parsed_job(job_id)?;
        let mappings = self.job_mappings(job_id)?;
        if mappings.len() != parsed.filaments.len() {
            return Err(AppError::InvalidMapping);
        }
        let mapping_by_tool = mappings
            .into_iter()
            .map(|mapping| (mapping.tool, mapping))
            .collect::<BTreeMap<_, _>>();
        let (selected_mm, selected_layer, confidence) = usage_for_outcome(&parsed.gcode, &outcome)?;

        let mut profiles_by_tool = BTreeMap::new();
        for profile in &parsed.filaments {
            if profiles_by_tool.insert(profile.tool, profile).is_some() {
                return Err(AppError::InvalidMapping);
            }
        }
        for (tool, millimeters) in &selected_mm {
            if *millimeters > 0.0
                && (!profiles_by_tool.contains_key(tool) || !mapping_by_tool.contains_key(tool))
            {
                return Err(AppError::InvalidMapping);
            }
        }

        let mut grouped = BTreeMap::<Uuid, Consumption>::new();
        for profile in profiles_by_tool.into_values() {
            let mapping = mapping_by_tool
                .get(&profile.tool)
                .ok_or(AppError::InvalidMapping)?;
            let grams =
                profile.grams_for_length_mm(selected_mm.get(&profile.tool).copied().unwrap_or(0.0));
            if grams <= 0.0 {
                continue;
            }
            let item = grouped.entry(mapping.spool_id).or_insert(Consumption {
                spool_id: mapping.spool_id,
                grams: 0.0,
                confidence,
                slot_number: mapping.slot_number,
            });
            item.grams += grams;
        }
        let consumption = grouped.into_values().collect::<Vec<_>>();
        let settlement_version = 1;
        let outcome_json = serde_json::to_string(&outcome)
            .map_err(|error| AppError::Database(error.to_string()))?;
        let transaction = self.database.connection.transaction()?;

        for item in &consumption {
            let status: Option<String> = transaction
                .query_row(
                    "SELECT status FROM spools WHERE spool_id = ?1",
                    params![item.spool_id.to_string()],
                    |row| row.get(0),
                )
                .optional()?;
            if status.as_deref().is_none_or(|status| status == "archived") {
                return Err(AppError::InvalidMapping);
            }
            let balance = ledger_balance(&transaction, item.spool_id)?;
            if balance + 1e-9 < item.grams {
                return Err(AppError::InsufficientFilament);
            }
        }

        for item in &consumption {
            transaction.execute(
                "INSERT INTO job_consumption (job_id, spool_id, settlement_version, consumed_grams, confidence, slot_number) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    job_id.to_string(),
                    item.spool_id.to_string(),
                    settlement_version,
                    item.grams,
                    confidence_name(item.confidence),
                    item.slot_number,
                ],
            )?;
            transaction.execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, job_id, settlement_version, event_type, delta_grams, confidence) VALUES (?1, ?2, ?3, ?4, ?5, 'settlement', ?6, ?7)",
                params![
                    Uuid::new_v4().to_string(),
                    format!("settlement-{job_id}-{settlement_version}-{}", item.spool_id),
                    item.spool_id.to_string(),
                    job_id.to_string(),
                    settlement_version,
                    -item.grams,
                    confidence_name(item.confidence),
                ],
            )?;
            refresh_balance(&transaction, item.spool_id)?;
        }
        transaction.execute(
            "UPDATE print_jobs SET outcome = ?1, settlement_version = ?2 WHERE job_id = ?3",
            params![outcome_json, settlement_version, job_id.to_string()],
        )?;
        transaction.commit()?;

        Ok(SettlementResult {
            job_id,
            outcome,
            settlement_version,
            selected_layer,
            confidence,
            consumption,
        })
    }

    pub fn reverse_settlement(&mut self, job_id: Uuid) -> Result<ReversalResult> {
        let settled = self.saved_settlement(job_id)?.ok_or(AppError::InvalidJob)?;
        let already_reversed: bool = self.database.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM ledger_events WHERE job_id = ?1 AND settlement_version = ?2 AND event_type = 'reversal')",
            params![job_id.to_string(), settled.settlement_version],
            |row| row.get(0),
        )?;
        if already_reversed {
            return Ok(ReversalResult {
                job_id,
                settlement_version: settled.settlement_version,
                already_reversed: true,
                restored: settled.consumption,
            });
        }

        let transaction = self.database.connection.transaction()?;
        for item in &settled.consumption {
            let event: (String, f64, String) = transaction.query_row(
                "SELECT event_id, delta_grams, confidence FROM ledger_events WHERE job_id = ?1 AND spool_id = ?2 AND settlement_version = ?3 AND event_type = 'settlement'",
                params![
                    job_id.to_string(),
                    item.spool_id.to_string(),
                    settled.settlement_version,
                ],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
            transaction.execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, job_id, settlement_version, event_type, delta_grams, confidence, reverses_event_id) VALUES (?1, ?2, ?3, ?4, ?5, 'reversal', ?6, ?7, ?8)",
                params![
                    Uuid::new_v4().to_string(),
                    format!("reversal-{job_id}-{}-{}", settled.settlement_version, item.spool_id),
                    item.spool_id.to_string(),
                    job_id.to_string(),
                    settled.settlement_version,
                    -event.1,
                    event.2,
                    event.0,
                ],
            )?;
            refresh_balance(&transaction, item.spool_id)?;
        }
        transaction.commit()?;

        Ok(ReversalResult {
            job_id,
            settlement_version: settled.settlement_version,
            already_reversed: false,
            restored: settled.consumption,
        })
    }

    pub fn spool_balance(&self, spool_id: Uuid) -> Result<f64> {
        self.database
            .connection
            .query_row(
                "SELECT COALESCE(SUM(delta_grams), 0.0) FROM ledger_events WHERE spool_id = ?1",
                params![spool_id.to_string()],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn settlement_event_count(&self, job_id: Uuid) -> Result<u32> {
        self.event_count(job_id, "settlement")
    }

    pub fn reversal_event_count(&self, job_id: Uuid) -> Result<u32> {
        self.event_count(job_id, "reversal")
    }

    fn event_count(&self, job_id: Uuid, event_type: &str) -> Result<u32> {
        self.database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM ledger_events WHERE job_id = ?1 AND event_type = ?2",
                params![job_id.to_string(), event_type],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    fn saved_settlement(&self, job_id: Uuid) -> Result<Option<SettlementResult>> {
        let row: Option<(String, u32)> = self
            .database
            .connection
            .query_row(
                "SELECT outcome, settlement_version FROM print_jobs WHERE job_id = ?1 AND outcome IS NOT NULL",
                params![job_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((outcome_json, settlement_version)) = row else {
            return Ok(None);
        };
        let outcome: JobOutcome = serde_json::from_str(&outcome_json)
            .map_err(|error| AppError::Database(error.to_string()))?;
        let (_, selected_layer, confidence) =
            usage_for_outcome(&self.parsed_job(job_id)?.gcode, &outcome)?;
        let mut statement = self.database.connection.prepare(
            "SELECT spool_id, consumed_grams, confidence, slot_number FROM job_consumption WHERE job_id = ?1 AND settlement_version = ?2 ORDER BY spool_id",
        )?;
        let consumption = statement
            .query_map(params![job_id.to_string(), settlement_version], |row| {
                let spool_id: String = row.get(0)?;
                let confidence: String = row.get(2)?;
                Ok(Consumption {
                    spool_id: spool_id.parse().map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?,
                    grams: row.get(1)?,
                    confidence: parse_confidence(&confidence)?,
                    slot_number: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Some(SettlementResult {
            job_id,
            outcome,
            settlement_version,
            selected_layer,
            confidence,
            consumption,
        }))
    }
}

fn usage_for_outcome(
    report: &crate::parser::gcode::GcodeReport,
    outcome: &JobOutcome,
) -> Result<(BTreeMap<u8, f64>, Option<u32>, Confidence)> {
    match outcome {
        JobOutcome::Success => Ok((report.totals_mm.clone(), None, Confidence::Exact)),
        JobOutcome::Failed { stop_layer } | JobOutcome::Cancelled { stop_layer } => {
            let usage = report
                .layers
                .iter()
                .find(|usage| usage.layer == *stop_layer)
                .ok_or(AppError::InvalidJob)?;
            Ok((
                usage.cumulative_mm.clone(),
                Some(*stop_layer),
                Confidence::Exact,
            ))
        }
        JobOutcome::Estimated { progress_percent } => {
            if !progress_percent.is_finite()
                || *progress_percent < 0.0
                || *progress_percent > 100.0
                || report.layers.is_empty()
            {
                return Err(AppError::InvalidJob);
            }
            let last = report.layers.len() - 1;
            let index = ((*progress_percent as f64 / 100.0) * last as f64).round() as usize;
            let usage = &report.layers[index];
            Ok((
                usage.cumulative_mm.clone(),
                Some(usage.layer),
                Confidence::Estimated,
            ))
        }
    }
}

fn ledger_balance(transaction: &Transaction<'_>, spool_id: Uuid) -> Result<f64> {
    transaction
        .query_row(
            "SELECT COALESCE(SUM(delta_grams), 0.0) FROM ledger_events WHERE spool_id = ?1",
            params![spool_id.to_string()],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn refresh_balance(transaction: &Transaction<'_>, spool_id: Uuid) -> Result<()> {
    let balance = ledger_balance(transaction, spool_id)?;
    let current_status: String = transaction.query_row(
        "SELECT status FROM spools WHERE spool_id = ?1",
        params![spool_id.to_string()],
        |row| row.get(0),
    )?;
    let mounted: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM ams_slots WHERE spool_id = ?1)",
        params![spool_id.to_string()],
        |row| row.get(0),
    )?;
    let status = status_for(current_status == "archived", mounted, balance);
    transaction.execute(
        "UPDATE spools SET remaining_grams = ?1, status = ?2 WHERE spool_id = ?3",
        params![balance.max(0.0), status, spool_id.to_string()],
    )?;
    Ok(())
}

fn confidence_name(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Exact => "exact",
        Confidence::Estimated => "estimated",
        Confidence::NeedsConfirmation => "needs_confirmation",
    }
}

fn parse_confidence(value: &str) -> rusqlite::Result<Confidence> {
    match value {
        "exact" => Ok(Confidence::Exact),
        "estimated" => Ok(Confidence::Estimated),
        "needs_confirmation" => Ok(Confidence::NeedsConfirmation),
        _ => Err(rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            "unknown confidence".into(),
        )),
    }
}

#[tauri::command]
pub fn settle_job(
    job_id: Uuid,
    outcome: JobOutcome,
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, crate::pet::runtime::PetRuntime>,
) -> Result<SettlementResult> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    let before = service.pending_summary()?;
    let before_version = service.current_settlement_version(job_id)?;
    let result = service.settle_job(job_id, outcome)?;
    let after = service.pending_summary()?;
    drop(service);
    let signal = crate::pet::runtime::pending_transition(
        before.count,
        after.count,
        before_version != result.settlement_version,
    );
    runtime.refresh_pending(after, signal);
    Ok(result)
}

#[tauri::command]
pub fn reverse_settlement(
    job_id: Uuid,
    state: tauri::State<'_, PrintState>,
) -> Result<ReversalResult> {
    with_print(state, |service| service.reverse_settlement(job_id))
}

#[cfg(test)]
mod tests {
    use crate::{
        db::AppDatabase,
        domain::{Confidence, JobOutcome},
        imports::{PrintService, ToolMapping},
        inventory::{InventoryService, NewSpool},
    };
    use rusqlite::params;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use uuid::Uuid;

    fn settlement_fixture() -> PathBuf {
        settlement_fixture_with_gcode(b"M83\n; LAYER:0\nT0\nG1 E10\nT1\nG1 E20\n; LAYER:1\nT0\nG1 E5\nT1\nG1 E10\n; LAYER:2\nT0\nG1 E5\nT1\nG1 E10\n")
    }

    fn settlement_fixture_with_gcode(gcode: &[u8]) -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "bambu-pools-settlement-{}-{}.3mf",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::FileOptions::default();
        archive
            .start_file("Metadata/project_settings.config", options)
            .unwrap();
        archive
            .write_all(
                br##"{"filament_settings_id":["Bambu PLA Basic @BBL A1","Bambu PLA Matte @BBL A1"],"filament_type":["PLA","PLA"],"filament_colour":["#FF0000","#00FF00"],"filament_diameter":["1.75","1.75"],"filament_density":["1.24","1.24"]}"##,
            )
            .unwrap();
        archive
            .start_file("Metadata/plate_1.gcode", options)
            .unwrap();
        archive.write_all(gcode).unwrap();
        archive.finish().unwrap();
        path
    }

    fn prepared_service() -> (PrintService, uuid::Uuid, [uuid::Uuid; 2]) {
        prepared_service_with_balances([1000.0, 1000.0])
    }

    fn prepared_service_with_balances(
        balances: [f64; 2],
    ) -> (PrintService, uuid::Uuid, [uuid::Uuid; 2]) {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let basic = inventory
            .create_spool(NewSpool {
                display_name: "Basic white".to_owned(),
                preset_id: Some("Bambu PLA Basic @BBL A1".to_owned()),
                brand: "Bambu Lab".to_owned(),
                material: "PLA".to_owned(),
                series: "Basic".to_owned(),
                color_hex: "#FF0000".to_owned(),
                remaining_grams: balances[0],
            })
            .unwrap();
        let matte = inventory
            .create_spool(NewSpool {
                display_name: "Matte red".to_owned(),
                preset_id: Some("Bambu PLA Matte @BBL A1".to_owned()),
                brand: "Bambu Lab".to_owned(),
                material: "PLA".to_owned(),
                series: "Matte".to_owned(),
                color_hex: "#00FF00".to_owned(),
                remaining_grams: balances[1],
            })
            .unwrap();
        inventory.mount_spool(1, basic).unwrap();
        inventory.mount_spool(3, matte).unwrap();
        let database = inventory.into_database();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let fixture = settlement_fixture();
        let preview = service.import_print_file(&fixture).unwrap();
        fs::remove_file(fixture).unwrap();
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
        (service, preview.job_id, [basic, matte])
    }

    #[test]
    fn successful_job_deducts_full_exact_usage_from_each_concrete_spool() {
        let (mut service, job_id, spools) = prepared_service();

        let result = service.settle_job(job_id, JobOutcome::Success).unwrap();

        assert_eq!(result.confidence, Confidence::Exact);
        assert_eq!(result.settlement_version, 1);
        assert_eq!(result.consumption.len(), 2);
        for spool_id in spools {
            let used = result
                .consumption
                .iter()
                .find(|item| item.spool_id == spool_id)
                .unwrap();
            assert!(used.grams > 0.0);
            assert!(
                (service.spool_balance(spool_id).unwrap() - (1000.0 - used.grams)).abs() < 1e-9
            );
        }
    }

    #[test]
    fn failed_and_cancelled_jobs_use_the_requested_layer_cumulative() {
        for outcome in [
            JobOutcome::Failed { stop_layer: 0 },
            JobOutcome::Cancelled { stop_layer: 0 },
        ] {
            let (mut service, job_id, _) = prepared_service();
            let result = service.settle_job(job_id, outcome).unwrap();

            assert_eq!(result.selected_layer, Some(0));
            assert_eq!(result.confidence, Confidence::Exact);
            assert!(result
                .consumption
                .iter()
                .all(|consumption| consumption.grams >= 0.0));
        }
    }

    #[test]
    fn progress_percentage_uses_nearest_layer_and_marks_estimated() {
        let (mut service, job_id, _) = prepared_service();

        let result = service
            .settle_job(
                job_id,
                JobOutcome::Estimated {
                    progress_percent: 50.0,
                },
            )
            .unwrap();

        assert_eq!(result.selected_layer, Some(1));
        assert_eq!(result.confidence, Confidence::Estimated);
    }

    #[test]
    fn repeated_same_settlement_returns_original_without_double_deduction() {
        let (mut service, job_id, spools) = prepared_service();
        let first = service.settle_job(job_id, JobOutcome::Success).unwrap();
        let balances = spools.map(|spool| service.spool_balance(spool).unwrap());

        let repeated = service.settle_job(job_id, JobOutcome::Success).unwrap();

        assert_eq!(repeated, first);
        assert_eq!(
            spools.map(|spool| service.spool_balance(spool).unwrap()),
            balances
        );
        assert_eq!(service.settlement_event_count(job_id).unwrap(), 2);
    }

    #[test]
    fn reversal_appends_equal_opposite_events_and_second_call_is_idempotent() {
        let (mut service, job_id, spools) = prepared_service();
        let settled = service.settle_job(job_id, JobOutcome::Success).unwrap();

        let reversed = service.reverse_settlement(job_id).unwrap();
        let repeated = service.reverse_settlement(job_id).unwrap();

        assert!(!reversed.already_reversed);
        assert!(repeated.already_reversed);
        assert_eq!(reversed.restored, settled.consumption);
        assert_eq!(
            spools.map(|spool| service.spool_balance(spool).unwrap()),
            [1000.0, 1000.0]
        );
        assert_eq!(service.reversal_event_count(job_id).unwrap(), 2);
    }

    #[test]
    fn settlement_is_atomic_when_any_spool_has_insufficient_filament() {
        let (mut service, job_id, spools) = prepared_service_with_balances([0.0001, 1000.0]);

        let error = service.settle_job(job_id, JobOutcome::Success).unwrap_err();

        assert_eq!(error.code(), "insufficient_filament");
        assert_eq!(service.settlement_event_count(job_id).unwrap(), 0);
        assert_eq!(service.spool_balance(spools[1]).unwrap(), 1000.0);
    }

    #[test]
    fn unknown_positive_usage_tool_fails_without_any_mutation() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let first = inventory
            .create_spool(NewSpool {
                display_name: "T0".to_owned(),
                preset_id: None,
                brand: "Bambu Lab".to_owned(),
                material: "PLA".to_owned(),
                series: "Basic".to_owned(),
                color_hex: "#FF0000".to_owned(),
                remaining_grams: 1000.0,
            })
            .unwrap();
        let second = inventory
            .create_spool(NewSpool {
                display_name: "T1".to_owned(),
                preset_id: None,
                brand: "Bambu Lab".to_owned(),
                material: "PLA".to_owned(),
                series: "Matte".to_owned(),
                color_hex: "#00FF00".to_owned(),
                remaining_grams: 1000.0,
            })
            .unwrap();
        let database = inventory.into_database();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let fixture =
            settlement_fixture_with_gcode(b"M83\n; LAYER:0\nT0\nG1 E10\nT1\nG1 E20\nT2\nG1 E30\n");
        let preview = service.import_print_file(&fixture).unwrap();
        fs::remove_file(fixture).unwrap();
        service
            .confirm_job_mapping(
                preview.job_id,
                vec![
                    ToolMapping {
                        tool: 0,
                        spool_id: first,
                    },
                    ToolMapping {
                        tool: 1,
                        spool_id: second,
                    },
                ],
            )
            .unwrap();

        let error = service
            .settle_job(preview.job_id, JobOutcome::Success)
            .unwrap_err();

        assert_eq!(error.code(), "invalid_mapping");
        assert_eq!(service.settlement_event_count(preview.job_id).unwrap(), 0);
        assert_eq!(service.spool_balance(first).unwrap(), 1000.0);
        assert_eq!(service.spool_balance(second).unwrap(), 1000.0);
    }

    #[test]
    fn archive_after_mapping_rejects_settlement_atomically() {
        let (mut service, job_id, spools) = prepared_service();
        service
            .database
            .connection
            .execute(
                "UPDATE ams_slots SET spool_id = NULL WHERE spool_id = ?1",
                params![spools[0].to_string()],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE spools SET status = 'archived' WHERE spool_id = ?1",
                params![spools[0].to_string()],
            )
            .unwrap();

        let error = service.settle_job(job_id, JobOutcome::Success).unwrap_err();

        assert_eq!(error.code(), "invalid_mapping");
        assert_eq!(service.settlement_event_count(job_id).unwrap(), 0);
        assert_eq!(service.spool_balance(spools[0]).unwrap(), 1000.0);
        assert_eq!(service.spool_balance(spools[1]).unwrap(), 1000.0);
    }

    #[test]
    fn reversal_restores_balance_without_unarchiving_spool() {
        let (mut service, job_id, spools) = prepared_service();
        service.settle_job(job_id, JobOutcome::Success).unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE ams_slots SET spool_id = NULL WHERE spool_id = ?1",
                params![spools[0].to_string()],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE spools SET status = 'archived' WHERE spool_id = ?1",
                params![spools[0].to_string()],
            )
            .unwrap();

        service.reverse_settlement(job_id).unwrap();

        let status: String = service
            .database
            .connection
            .query_row(
                "SELECT status FROM spools WHERE spool_id = ?1",
                params![spools[0].to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "archived");
        assert_eq!(service.spool_balance(spools[0]).unwrap(), 1000.0);
    }

    #[test]
    #[ignore = "requires BAMBU_SMOKE_3MF to point to a local user-owned sliced file"]
    fn smoke_real_sliced_file_from_environment() {
        let path = std::env::var_os("BAMBU_SMOKE_3MF")
            .map(PathBuf::from)
            .expect("set BAMBU_SMOKE_3MF");
        let metadata_before = std::fs::metadata(&path).unwrap();
        let source_hash_before = crate::imports::sha256(&path).unwrap();
        let probe = crate::parser::parse_3mf(&path).unwrap();
        assert_eq!(probe.filaments.len(), 4);

        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let expected_spools = probe
            .filaments
            .iter()
            .map(|profile| {
                let spool_id = inventory
                    .create_spool(NewSpool {
                        display_name: format!("Import baseline tool {}", profile.tool),
                        preset_id: Some(profile.preset_id.clone()),
                        brand: profile.brand.clone(),
                        material: profile.material.clone(),
                        series: profile.series.clone(),
                        color_hex: profile.color_hex.clone(),
                        remaining_grams: 100_000.0,
                    })
                    .unwrap();
                (profile.tool, spool_id)
            })
            .collect::<Vec<_>>();
        let database = inventory.into_database();
        let mut import_service = PrintService::with_stability_delay(database, Duration::ZERO);
        let balances_before = balance_rows(&import_service);
        assert_eq!(balances_before.len(), probe.filaments.len());
        assert!(balances_before
            .iter()
            .all(|(_, remaining_grams)| *remaining_grams > 0.0));
        let ledger_count_before = ledger_event_count(&import_service);
        assert_eq!(ledger_count_before as usize, probe.filaments.len());
        let ledger_before = ledger_rows(&import_service);
        assert_eq!(ledger_before.len(), probe.filaments.len());
        let pending_before = import_service.pending_summary().unwrap();
        assert_eq!(pending_before.count, 0);
        assert_eq!(pending_before.newest_job_id, None);

        let preview = import_service.import_print_file(&path).unwrap();
        assert_eq!(balance_rows(&import_service), balances_before);
        assert_eq!(ledger_event_count(&import_service), ledger_count_before);
        assert_eq!(ledger_rows(&import_service), ledger_before);
        let pending_after = import_service.pending_summary().unwrap();
        assert_eq!(pending_after.count, 1);
        assert_eq!(pending_after.newest_job_id, Some(preview.job_id));
        for filament in &preview.filaments {
            let expected_spool_id = expected_spools
                .iter()
                .find_map(|(tool, spool_id)| (*tool == filament.tool).then_some(*spool_id))
                .unwrap();
            assert_eq!(filament.candidate_spool_ids, vec![expected_spool_id]);
            assert_eq!(filament.suggested_spool_id, Some(expected_spool_id));
        }
        let metadata_after = std::fs::metadata(&path).unwrap();
        assert_eq!(metadata_before.len(), metadata_after.len());
        assert_eq!(source_hash_before, crate::imports::sha256(&path).unwrap());

        assert_eq!(preview.filaments.len(), probe.filaments.len());
        let middle_layer = probe.gcode.max_layer.saturating_sub(1) / 2;
        println!("real_file_layers={}", probe.gcode.max_layer);
        for profile in &probe.filaments {
            let total_mm = probe
                .gcode
                .totals_mm
                .get(&profile.tool)
                .copied()
                .unwrap_or(0.0);
            println!(
                "tool={} color={} preset={} total_grams={:.6}",
                profile.tool,
                profile.color_hex,
                profile.preset_id,
                profile.grams_for_length_mm(total_mm)
            );
        }

        for (label, outcome) in [
            ("success", JobOutcome::Success),
            (
                "failed",
                JobOutcome::Failed {
                    stop_layer: middle_layer,
                },
            ),
            (
                "cancelled",
                JobOutcome::Cancelled {
                    stop_layer: middle_layer,
                },
            ),
            (
                "estimated_50_percent",
                JobOutcome::Estimated {
                    progress_percent: 50.0,
                },
            ),
        ] {
            let (mut service, job_id) = real_file_service(&path);
            let balances_before_settlement = balance_rows(&service);
            let result = service.settle_job(job_id, outcome.clone()).unwrap();
            let balances_after_settlement = balance_rows(&service);
            assert_eq!(result.consumption.len(), 4);
            assert!(result.consumption.iter().all(|item| item.grams > 0.0));
            assert_ne!(balances_after_settlement, balances_before_settlement);
            println!(
                "{label}: selected_layer={:?}, confidence={:?}, consumption={:?}",
                result.selected_layer, result.confidence, result.consumption
            );

            if matches!(outcome, JobOutcome::Success) {
                let repeated = service.settle_job(job_id, outcome).unwrap();
                assert_eq!(repeated, result);
                assert_eq!(balance_rows(&service), balances_after_settlement);
                let reversal = service.reverse_settlement(job_id).unwrap();
                let second_reversal = service.reverse_settlement(job_id).unwrap();
                assert!(!reversal.already_reversed);
                assert_eq!(reversal.restored, result.consumption);
                assert!(second_reversal.already_reversed);
                assert_eq!(balance_rows(&service), balances_before_settlement);
                println!(
                    "success_idempotent=true, reversal_restored={:?}, second_reversal_already_reversed={}",
                    reversal.restored, second_reversal.already_reversed
                );
            }
        }
    }

    fn balance_rows(service: &PrintService) -> Vec<(String, f64)> {
        let mut statement = service
            .database
            .connection
            .prepare("SELECT spool_id, remaining_grams FROM spools ORDER BY spool_id")
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    type SmokeLedgerRow = (
        String,
        String,
        String,
        Option<String>,
        Option<u32>,
        String,
        f64,
        String,
        Option<String>,
        String,
    );

    fn ledger_event_count(service: &PrintService) -> u32 {
        service
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM ledger_events", [], |row| row.get(0))
            .unwrap()
    }

    fn ledger_rows(service: &PrintService) -> Vec<SmokeLedgerRow> {
        let mut statement = service
            .database
            .connection
            .prepare(
                "SELECT event_id, idempotency_key, spool_id, job_id, settlement_version, event_type, delta_grams, confidence, reverses_event_id, created_at FROM ledger_events ORDER BY event_id",
            )
            .unwrap();
        statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                ))
            })
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    fn real_file_service(path: &std::path::Path) -> (PrintService, Uuid) {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let preview = service.import_print_file(path).unwrap();
        let mut mappings = Vec::new();
        for filament in &preview.filaments {
            let spool_id = Uuid::new_v4();
            service
                .database
                .connection
                .execute(
                    "INSERT INTO spools (spool_id, display_name, preset_id, brand, material, series, color_hex, remaining_grams, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 100000.0, 'available')",
                    params![
                        spool_id.to_string(),
                        format!("Smoke tool {}", filament.tool),
                        filament.profile.preset_id,
                        filament.profile.brand,
                        filament.profile.material,
                        filament.profile.series,
                        filament.profile.color_hex,
                    ],
                )
                .unwrap();
            service
                .database
                .connection
                .execute(
                    "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence) VALUES (?1, ?2, ?3, 'creation', 100000.0, 'exact')",
                    params![
                        Uuid::new_v4().to_string(),
                        format!("smoke-baseline-{spool_id}"),
                        spool_id.to_string(),
                    ],
                )
                .unwrap();
            if filament.tool < 4 {
                service
                    .database
                    .connection
                    .execute(
                        "UPDATE ams_slots SET spool_id = ?1 WHERE slot_number = ?2",
                        params![spool_id.to_string(), filament.tool + 1],
                    )
                    .unwrap();
            }
            mappings.push(ToolMapping {
                tool: filament.tool,
                spool_id,
            });
        }
        service
            .confirm_job_mapping(preview.job_id, mappings)
            .unwrap();
        (service, preview.job_id)
    }
}
