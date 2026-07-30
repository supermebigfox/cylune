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
            catalog_id: None,
            color_name: None,
            color_code: None,
            color_hexes: vec![color_hex.to_owned()],
            preset_base: None,
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
    fn matching_spools_prioritizes_exact_then_preset_base_then_legacy() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);

        let mut exact_spool = new_spool("Bambu PLA Basic @BBL A1", "#FF0000");
        exact_spool.preset_base = Some("Bambu PLA Basic".to_owned());
        let exact = inventory.create_spool(exact_spool).unwrap();

        let mut base_spool = new_spool("Bambu PLA Basic @BBL X1C", "#FF0000");
        base_spool.preset_base = Some("Bambu PLA Basic".to_owned());
        base_spool.series = "Catalog series ignored for base matching".to_owned();
        let base = inventory.create_spool(base_spool).unwrap();

        let mut legacy_spool = new_spool("Legacy PLA profile", "#FF0000");
        legacy_spool.preset_id = None;
        legacy_spool.preset_base = None;
        legacy_spool.series = "Basic".to_owned();
        let legacy = inventory.create_spool(legacy_spool).unwrap();

        let database = inventory.into_database();
        let service = PrintService::with_stability_delay(database, Duration::ZERO);
        let parsed = crate::parser::parse_3mf(&fixture("bambu_multicolor.3mf")).unwrap();
        let profile = parsed
            .filaments
            .iter()
            .find(|profile| profile.tool == 0)
            .unwrap();

        assert_eq!(service.matching_spools(profile).unwrap(), vec![exact]);

        service
            .database
            .connection
            .execute(
                "UPDATE spools SET status = 'archived' WHERE spool_id = ?1",
                [exact.to_string()],
            )
            .unwrap();
        assert_eq!(service.matching_spools(profile).unwrap(), vec![base]);

        service
            .database
            .connection
            .execute(
                "UPDATE spools SET status = 'archived' WHERE spool_id = ?1",
                [base.to_string()],
            )
            .unwrap();
        assert_eq!(service.matching_spools(profile).unwrap(), vec![legacy]);
    }

    #[test]
    fn generated_catalog_base_matches_a_real_sliced_profile() {
        let snapshot: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../src/catalog/bambu.json")).unwrap();
        let catalog_entry = snapshot["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == "bambu:GFA00:10100")
            .unwrap();
        let catalog_base = catalog_entry["presetBase"].as_str().unwrap();
        assert_eq!(catalog_base, "Bambu PLA Basic");

        let parsed = crate::parser::parse_3mf(&fixture("bambu_multicolor.3mf")).unwrap();
        let profile = parsed
            .filaments
            .iter()
            .find(|profile| profile.tool == 0)
            .unwrap();
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let mut catalog_spool = new_spool("Bambu PLA Basic @BBL X1C", &profile.color_hex);
        catalog_spool.preset_base = Some(catalog_base.to_owned());
        catalog_spool.series = "Catalog series ignored for base matching".to_owned();
        let catalog_spool_id = inventory.create_spool(catalog_spool).unwrap();
        let database = inventory.into_database();
        let service = PrintService::with_stability_delay(database, Duration::ZERO);

        assert_eq!(
            service.matching_spools(profile).unwrap(),
            vec![catalog_spool_id]
        );
    }

    #[test]
    fn project_colors_match_the_nearest_loaded_spools_of_the_same_catalog_profile() {
        let parsed = crate::parser::parse_3mf(&fixture("bambu_multicolor.3mf")).unwrap();
        let profile = parsed
            .filaments
            .iter()
            .find(|profile| profile.tool == 0)
            .unwrap()
            .clone();

        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);

        let loaded = ["#FFFFFF", "#F4EE2A", "#C12E1F", "#0A2989"]
            .into_iter()
            .enumerate()
            .map(|(index, color_hex)| {
                let mut spool = new_spool("Bambu PLA Basic", color_hex);
                spool.preset_base = Some("Bambu PLA Basic".to_owned());
                let spool_id = inventory.create_spool(spool).unwrap();
                inventory.mount_spool((index + 1) as u8, spool_id).unwrap();
                spool_id
            })
            .collect::<Vec<_>>();

        let mut unmounted_gradient_red = new_spool("Bambu PLA Basic", "#E94B3C");
        unmounted_gradient_red.preset_base = Some("Bambu PLA Basic".to_owned());
        inventory.create_spool(unmounted_gradient_red).unwrap();

        let database = inventory.into_database();
        let service = PrintService::with_stability_delay(database, Duration::ZERO);

        for (project_color, expected_spool) in [
            ("#FFFEFC", loaded[0]),
            ("#FFFD0D", loaded[1]),
            ("#FE3D36", loaded[2]),
            ("#1C4EBB", loaded[3]),
        ] {
            let mut project_profile = profile.clone();
            project_profile.color_hex = project_color.to_owned();
            assert_eq!(
                service.matching_spools(&project_profile).unwrap(),
                vec![expected_spool],
                "project color {project_color} should map to its loaded physical spool"
            );
        }
    }

    #[test]
    fn every_official_catalog_color_matches_its_loaded_physical_spool() {
        let snapshot: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../src/catalog/bambu.json")).unwrap();
        let parsed = crate::parser::parse_3mf(&fixture("bambu_multicolor.3mf")).unwrap();
        let template = parsed.filaments.first().unwrap();

        for entry in snapshot["entries"].as_array().unwrap() {
            let catalog_id = entry["id"].as_str().unwrap();
            let preset_base = entry["presetBase"].as_str().unwrap();
            let material = entry["material"].as_str().unwrap();
            let series = entry["series"].as_str().unwrap();
            let colors = entry["colors"].as_array().unwrap();

            for color in colors {
                let color_hex = color.as_str().unwrap();
                let database = AppDatabase::open_in_memory().unwrap();
                let mut inventory = InventoryService::new(database);
                let spool_id = inventory
                    .create_spool(NewSpool {
                        display_name: catalog_id.to_owned(),
                        preset_id: Some(preset_base.to_owned()),
                        catalog_id: Some(catalog_id.to_owned()),
                        color_name: None,
                        color_code: entry["colorCode"].as_str().map(str::to_owned),
                        color_hexes: colors
                            .iter()
                            .map(|value| value.as_str().unwrap().to_owned())
                            .collect(),
                        preset_base: Some(preset_base.to_owned()),
                        brand: entry["brand"].as_str().unwrap().to_owned(),
                        material: material.to_owned(),
                        series: series.to_owned(),
                        color_hex: color_hex.to_owned(),
                        remaining_grams: 1000.0,
                    })
                    .unwrap();
                inventory.mount_spool(1, spool_id).unwrap();

                let mut profile = template.clone();
                profile.preset_id = format!("{preset_base} @BBL A1");
                profile.material = material.to_owned();
                profile.series = series.to_owned();
                profile.color_hex = color_hex.to_owned();
                let service =
                    PrintService::with_stability_delay(inventory.into_database(), Duration::ZERO);

                assert_eq!(
                    service.matching_spools(&profile).unwrap(),
                    vec![spool_id],
                    "catalog color {catalog_id} {color_hex} should match"
                );
            }
        }
    }

    #[test]
    fn legacy_at_base_records_match_in_the_base_layer_in_active_row_order() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);

        let mut first_spool = new_spool("Bambu PLA Basic @BBL X1C", "#FF0000");
        first_spool.preset_base = Some("Bambu PLA Basic @base".to_owned());
        first_spool.series = "Catalog series ignored for base matching".to_owned();
        let first = inventory.create_spool(first_spool).unwrap();

        let mut archived_spool = new_spool("Bambu PLA Basic @BBL X1C", "#FF0000");
        archived_spool.preset_base = Some("Bambu PLA Basic @base".to_owned());
        archived_spool.series = "Catalog series ignored for base matching".to_owned();
        let archived = inventory.create_spool(archived_spool).unwrap();

        let mut second_spool = new_spool("Bambu PLA Basic @BBL X1C", "#FF0000");
        second_spool.preset_base = Some("Bambu PLA Basic @base".to_owned());
        second_spool.series = "Catalog series ignored for base matching".to_owned();
        let second = inventory.create_spool(second_spool).unwrap();

        let database = inventory.into_database();
        let service = PrintService::with_stability_delay(database, Duration::ZERO);
        service
            .database
            .connection
            .execute(
                "UPDATE spools SET status = 'archived' WHERE spool_id = ?1",
                [archived.to_string()],
            )
            .unwrap();
        let parsed = crate::parser::parse_3mf(&fixture("bambu_multicolor.3mf")).unwrap();
        let profile = parsed
            .filaments
            .iter()
            .find(|profile| profile.tool == 0)
            .unwrap();

        assert_eq!(
            service.matching_spools(profile).unwrap(),
            vec![first, second]
        );
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
    fn discard_pending_job_removes_only_the_draft_and_keeps_inventory_truth() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let basic = inventory
            .create_spool(new_spool("Bambu PLA Basic @BBL A1", "#FF0000"))
            .unwrap();
        let matte = inventory
            .create_spool(new_spool("Bambu PLA Matte @BBL A1", "#00FF00"))
            .unwrap();
        inventory.mount_spool(1, basic).unwrap();
        inventory.mount_spool(2, matte).unwrap();
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
        service
            .database
            .connection
            .execute(
                "INSERT INTO app_settings(setting_key, setting_value) VALUES('pending_job_id', ?1)",
                [preview.job_id.to_string()],
            )
            .unwrap();
        let balances_before = [
            service.spool_balance(basic).unwrap(),
            service.spool_balance(matte).unwrap(),
        ];
        let ledger_before: u32 = service
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM ledger_events", [], |row| row.get(0))
            .unwrap();

        service.discard_pending_job(preview.job_id).unwrap();

        assert_eq!(service.pending_summary().unwrap().count, 0);
        assert_eq!(service.job_count(&preview.source_hash).unwrap(), 0);
        assert_eq!(service.parse_result_count(&preview.source_hash).unwrap(), 1);
        assert_eq!(
            service
                .database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM job_mappings WHERE job_id = ?1",
                    [preview.job_id.to_string()],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap(),
            0
        );
        assert_eq!(
            service
                .database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM app_settings WHERE setting_key = 'pending_job_id'",
                    [],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap(),
            0
        );
        assert_eq!(
            [
                service.spool_balance(basic).unwrap(),
                service.spool_balance(matte).unwrap(),
            ],
            balances_before
        );
        assert_eq!(
            service
                .database
                .connection
                .query_row("SELECT COUNT(*) FROM ledger_events", [], |row| row
                    .get::<_, u32>(0),)
                .unwrap(),
            ledger_before
        );
        assert_eq!(
            service
                .database
                .connection
                .query_row(
                    "SELECT spool_id FROM ams_slots WHERE slot_number = 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            basic.to_string()
        );
        assert_eq!(
            service
                .database
                .connection
                .query_row(
                    "SELECT spool_id FROM ams_slots WHERE slot_number = 2",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            matte.to_string()
        );
    }

    #[test]
    fn discard_pending_job_rejects_a_settled_job_without_mutation() {
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
        service
            .settle_job(preview.job_id, crate::domain::JobOutcome::Success)
            .unwrap();
        let balances_before = [
            service.spool_balance(basic).unwrap(),
            service.spool_balance(matte).unwrap(),
        ];

        let error = service.discard_pending_job(preview.job_id).unwrap_err();

        assert_eq!(error.code(), "invalid_job");
        assert_eq!(service.job_count(&preview.source_hash).unwrap(), 1);
        assert_eq!(
            [
                service.spool_balance(basic).unwrap(),
                service.spool_balance(matte).unwrap(),
            ],
            balances_before
        );
    }

    #[test]
    fn discarded_file_can_be_imported_again_from_its_cached_parse() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = fixture("bambu_multicolor.3mf");
        let first = service.import_print_file(&path).unwrap();
        service.discard_pending_job(first.job_id).unwrap();

        let second = service.import_print_file(&path).unwrap();

        assert_ne!(second.job_id, first.job_id);
        assert_eq!(second.state, ImportState::New);
        assert_eq!(service.parse_result_count(&second.source_hash).unwrap(), 1);
        assert_eq!(service.job_count(&second.source_hash).unwrap(), 1);
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
    parser::{parse_3mf, preset_base, FilamentProfile, ParsedPrintFile},
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

            let job_id = Uuid::new_v4();
            #[cfg(test)]
            self.run_before_final_stability_check(path);
            self.ensure_unchanged(path, stability)?;
            self.database.connection.execute(
                "INSERT INTO print_jobs (job_id, source_hash, source_file_name) VALUES (?1, ?2, ?3)",
                params![job_id.to_string(), source_hash, file_name],
            )?;
            return self.preview(job_id, source_hash, file_name, &parsed, ImportState::New);
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

    pub fn discard_pending_job(&mut self, job_id: Uuid) -> Result<()> {
        let transaction = self.database.connection.transaction()?;
        let outcome = transaction
            .query_row(
                "SELECT outcome FROM print_jobs WHERE job_id = ?1",
                [job_id.to_string()],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        if outcome != Some(None) {
            return Err(AppError::InvalidJob);
        }
        transaction.execute(
            "DELETE FROM job_mappings WHERE job_id = ?1",
            [job_id.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM app_settings WHERE setting_key = 'pending_job_id' AND setting_value = ?1",
            [job_id.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM print_jobs WHERE job_id = ?1",
            [job_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
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
        let exact = self.spool_ids(
            "SELECT spool_id FROM spools WHERE status <> 'archived' AND preset_id = ?1 AND material = ?2 AND series = ?3 AND UPPER(color_hex) = UPPER(?4) ORDER BY rowid",
            params![
                profile.preset_id,
                profile.material,
                profile.series,
                profile.color_hex,
            ],
        )?;
        if !exact.is_empty() {
            return Ok(exact);
        }

        let base = self.preset_base_spool_ids(profile)?;
        if !base.is_empty() {
            return Ok(base);
        }

        let nearest_loaded = self.nearest_loaded_preset_base_spool_ids(profile)?;
        if !nearest_loaded.is_empty() {
            return Ok(nearest_loaded);
        }

        self.spool_ids(
            "SELECT spool_id FROM spools WHERE status <> 'archived' AND preset_base IS NULL AND material = ?1 AND series = ?2 AND UPPER(color_hex) = UPPER(?3) ORDER BY rowid",
            params![profile.material, profile.series, profile.color_hex],
        )
    }

    fn preset_base_spool_ids(&self, profile: &FilamentProfile) -> Result<Vec<Uuid>> {
        let expected = preset_base(&profile.preset_id);
        let mut statement = self.database.connection.prepare(
            "SELECT spool_id, preset_base FROM spools WHERE status <> 'archived' AND preset_base IS NOT NULL AND material = ?1 AND UPPER(color_hex) = UPPER(?2) ORDER BY rowid",
        )?;
        let rows = statement
            .query_map(params![profile.material, profile.color_hex], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .filter(|(_, stored_base)| preset_base(stored_base) == expected)
            .map(|(spool_id, _)| {
                spool_id.parse().map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                    .into()
                })
            })
            .collect()
    }

    fn nearest_loaded_preset_base_spool_ids(&self, profile: &FilamentProfile) -> Result<Vec<Uuid>> {
        const MAX_COLOR_DISTANCE_SQUARED: u32 = 10_000;

        let Some(target_color) = parse_rgb_hex(&profile.color_hex) else {
            return Ok(Vec::new());
        };
        let expected_base = preset_base(&profile.preset_id);
        let mut statement = self.database.connection.prepare(
            "SELECT s.spool_id, s.preset_base, s.color_hex
             FROM ams_slots AS a
             JOIN spools AS s ON s.spool_id = a.spool_id
             WHERE s.status <> 'archived'
               AND s.preset_base IS NOT NULL
               AND s.material = ?1
             ORDER BY a.slot_number",
        )?;
        let rows = statement
            .query_map([&profile.material], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut best_distance = None;
        let mut best_spool_ids = Vec::new();
        for (spool_id, stored_base, color_hex) in rows {
            if preset_base(&stored_base) != expected_base {
                continue;
            }
            let Some(color) = parse_rgb_hex(&color_hex) else {
                continue;
            };
            let distance = rgb_distance_squared(target_color, color);
            if distance > MAX_COLOR_DISTANCE_SQUARED {
                continue;
            }
            match best_distance {
                Some(best) if distance > best => {}
                Some(best) if distance == best => best_spool_ids.push(spool_id),
                _ => {
                    best_distance = Some(distance);
                    best_spool_ids.clear();
                    best_spool_ids.push(spool_id);
                }
            }
        }

        best_spool_ids
            .into_iter()
            .map(|spool_id| {
                spool_id.parse().map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                    .into()
                })
            })
            .collect()
    }

    fn spool_ids<P>(&self, sql: &str, params: P) -> Result<Vec<Uuid>>
    where
        P: rusqlite::Params,
    {
        let mut statement = self.database.connection.prepare(sql)?;
        let spool_ids = statement
            .query_map(params, |row| row.get::<_, String>(0))?
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
        Ok(spool_ids)
    }
}

fn parse_rgb_hex(value: &str) -> Option<[u8; 3]> {
    let value = value.strip_prefix('#')?;
    if value.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}

fn rgb_distance_squared(left: [u8; 3], right: [u8; 3]) -> u32 {
    left.into_iter()
        .zip(right)
        .map(|(left, right)| {
            let difference = i32::from(left) - i32::from(right);
            (difference * difference) as u32
        })
        .sum()
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
pub fn discard_pending_job(
    job_id: Uuid,
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, crate::pet::runtime::PetRuntime>,
) -> Result<()> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    service.discard_pending_job(job_id)?;
    let summary = service.pending_summary()?;
    drop(service);
    runtime.refresh_pending(summary, None);
    Ok(())
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
