#[cfg(test)]
mod tests {
    use super::{FileStability, ImportState, PrintService, ToolMapping};
    use crate::{
        db::AppDatabase,
        domain::Confidence,
        inventory::{InventoryService, NewSpool},
        media::MediaStore,
    };
    use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
    use rusqlite::params;
    use std::path::PathBuf;
    use std::time::Duration;
    use std::{fs, fs::File, io::Write};
    use zip::write::FileOptions;

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

    fn two_plate_fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cylune-two-plate-import-{}.3mf",
            uuid::Uuid::new_v4()
        ));
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = FileOptions::default();
        archive
            .start_file("Metadata/filament_settings.config", options)
            .unwrap();
        archive
            .write_all(
                br##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##,
            )
            .unwrap();
        for plate_index in [1, 2] {
            archive
                .start_file(format!("Metadata/plate_{plate_index}.gcode"), options)
                .unwrap();
            archive
                .write_all(
                    format!(
                        "; total layer number: {}\nM83\n; LAYER:0\nG1 E{}\n",
                        plate_index + 1,
                        plate_index
                    )
                    .as_bytes(),
                )
                .unwrap();
        }
        archive.finish().unwrap();
        path
    }

    fn two_plate_fixture_with_shared_thumbnail() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cylune-two-plate-thumbnail-import-{}.3mf",
            uuid::Uuid::new_v4()
        ));
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = FileOptions::default();
        archive
            .start_file("Metadata/filament_settings.config", options)
            .unwrap();
        archive
            .write_all(
                br##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##,
            )
            .unwrap();
        let preferred_thumbnail = png_pixel([12, 80, 240, 255]);
        archive
            .start_file("Auxiliaries/.thumbnails/thumbnail_middle.png", options)
            .unwrap();
        archive.write_all(&preferred_thumbnail).unwrap();
        archive
            .start_file("Auxiliaries/.thumbnails/thumbnail_3mf.png", options)
            .unwrap();
        archive.write_all(&png_pixel([120, 70, 220, 255])).unwrap();
        for plate_index in [1, 2] {
            let plate_thumbnail = if plate_index == 1 {
                png_pixel([220, 48, 60, 255])
            } else {
                png_pixel([250, 190, 20, 255])
            };
            archive
                .start_file(format!("Metadata/plate_{plate_index}.png"), options)
                .unwrap();
            archive.write_all(&plate_thumbnail).unwrap();
            archive
                .start_file(format!("Metadata/plate_{plate_index}.gcode"), options)
                .unwrap();
            archive
                .write_all(b"; total layer number: 1\nM83\n; LAYER:0\nG1 E1\n")
                .unwrap();
        }
        archive.finish().unwrap();
        path
    }

    fn png_pixel(rgba: [u8; 4]) -> Vec<u8> {
        let mut thumbnail = Vec::new();
        PngEncoder::new(&mut thumbnail)
            .write_image(&rgba, 1, 1, ColorType::Rgba8.into())
            .unwrap();
        thumbnail
    }

    fn media_file_count(root: &std::path::Path) -> usize {
        let media = root.join("media");
        fs::read_dir(media)
            .unwrap()
            .flat_map(|entry| fs::read_dir(entry.unwrap().path()).unwrap())
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.file_type().unwrap().is_file())
            .count()
    }

    fn count(service: &PrintService, table: &str) -> u32 {
        service
            .database
            .connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    #[test]
    fn two_plate_import_creates_one_project_and_two_jobs() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let ledger_before = count(&service, "ledger_events");

        let preview = service.import_print_project(&path).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(preview.plates.len(), 2);
        assert_eq!(count(&service, "print_projects"), 1);
        assert_eq!(count(&service, "print_plates"), 2);
        assert_eq!(count(&service, "print_jobs"), 2);
        assert_eq!(count(&service, "parse_cache"), 1);
        assert_eq!(count(&service, "ledger_events"), ledger_before);
    }

    #[test]
    fn repeated_project_import_continues_the_pending_batch() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();

        let first = service.import_print_project(&path).unwrap();
        let repeated = service.import_print_project(&path).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(repeated.project_id, first.project_id);
        assert_eq!(
            repeated
                .plates
                .iter()
                .map(|plate| plate.job_id)
                .collect::<Vec<_>>(),
            first
                .plates
                .iter()
                .map(|plate| plate.job_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(repeated.state, ImportState::ExistingPending);
        assert_eq!(count(&service, "print_projects"), 1);
        assert_eq!(count(&service, "parse_cache"), 1);
    }

    #[test]
    fn settled_duplicate_requires_confirmation_then_creates_a_new_project_from_cache() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let first = service.import_print_project(&path).unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs
                 SET outcome = '{\"kind\":\"success\"}', settlement_version = 1
                 WHERE plate_id IN (
                    SELECT plate_id FROM print_plates WHERE project_id = ?1
                 )",
                [first.project_id.to_string()],
            )
            .unwrap();

        let duplicate = service.import_print_project(&path).unwrap();
        let confirmed = service
            .confirm_new_project(&first.source_hash, &path)
            .unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(duplicate.project_id, first.project_id);
        assert_eq!(duplicate.state, ImportState::NewPrintConfirmationRequired);
        assert_ne!(confirmed.project_id, first.project_id);
        assert_eq!(confirmed.state, ImportState::New);
        assert_eq!(confirmed.plates.len(), 2);
        assert_eq!(count(&service, "print_projects"), 2);
        assert_eq!(count(&service, "print_jobs"), 4);
        assert_eq!(count(&service, "parse_cache"), 1);
    }

    #[test]
    fn confirm_new_project_reuses_any_existing_pending_batch() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let imported = service.import_print_project(&path).unwrap();

        let confirmed = service
            .confirm_new_project(&imported.source_hash, &path)
            .unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(confirmed.project_id, imported.project_id);
        assert_eq!(confirmed.state, ImportState::ExistingPending);
        assert_eq!(count(&service, "print_projects"), 1);
        assert_eq!(count(&service, "print_jobs"), 2);
    }

    #[test]
    fn confirmed_project_retry_returns_the_same_new_pending_batch() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let imported = service.import_print_project(&path).unwrap();
        service.database.connection.execute("UPDATE print_jobs SET outcome='{\"kind\":\"success\"}',settlement_version=1 WHERE plate_id IN(SELECT plate_id FROM print_plates WHERE project_id=?1)",[imported.project_id.to_string()]).unwrap();

        let first = service
            .confirm_new_project(&imported.source_hash, &path)
            .unwrap();
        let retry = service
            .confirm_new_project(&imported.source_hash, &path)
            .unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(retry.project_id, first.project_id);
        assert_eq!(retry.state, ImportState::ExistingPending);
        assert_eq!(count(&service, "print_projects"), 2);
        assert_eq!(count(&service, "print_jobs"), 4);
    }

    #[test]
    fn confirm_new_project_rejects_a_path_with_a_different_source_hash() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let imported = service.import_print_project(&path).unwrap();
        service.database.connection.execute("UPDATE print_jobs SET outcome='{\"kind\":\"success\"}',settlement_version=1 WHERE plate_id IN(SELECT plate_id FROM print_plates WHERE project_id=?1)",[imported.project_id.to_string()]).unwrap();

        let error = service
            .confirm_new_project(&imported.source_hash, &fixture("project_only.3mf"))
            .unwrap_err();

        fs::remove_file(path).unwrap();
        assert_eq!(error.code(), "invalid_file");
        assert_eq!(count(&service, "print_projects"), 1);
    }

    #[test]
    fn confirm_new_project_rejects_a_source_changed_after_hashing() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = std::env::temp_dir().join(format!(
            "cylune-confirm-changing-{}.3mf",
            uuid::Uuid::new_v4()
        ));
        fs::copy(fixture("bambu_multicolor.3mf"), &path).unwrap();
        let imported = service.import_print_project(&path).unwrap();
        service.database.connection.execute("UPDATE print_jobs SET outcome='{\"kind\":\"success\"}',settlement_version=1 WHERE plate_id IN(SELECT plate_id FROM print_plates WHERE project_id=?1)",[imported.project_id.to_string()]).unwrap();
        service.before_final_stability_check = Some(Box::new(|path| {
            fs::OpenOptions::new()
                .append(true)
                .open(path)
                .unwrap()
                .write_all(b"changed")
                .unwrap();
        }));

        let error = service
            .confirm_new_project(&imported.source_hash, &path)
            .unwrap_err();

        fs::remove_file(path).unwrap();
        assert_eq!(error.code(), "file_not_stable");
        assert_eq!(count(&service, "print_projects"), 1);
    }

    #[test]
    fn legacy_confirm_reuses_cached_media_without_source_file_access() {
        let database = AppDatabase::open_in_memory().unwrap();
        let media_root =
            std::env::temp_dir().join(format!("cylune-legacy-confirm-{}", uuid::Uuid::new_v4()));
        let media_store = MediaStore::new(media_root.clone()).unwrap();
        let mut service = PrintService::with_media_store_and_stability_delay(
            database,
            media_store,
            Duration::ZERO,
        );
        let path = two_plate_fixture_with_shared_thumbnail();
        let imported = service.import_print_project(&path).unwrap();
        service.database.connection.execute("UPDATE print_jobs SET outcome='{\"kind\":\"success\"}',settlement_version=1 WHERE plate_id IN(SELECT plate_id FROM print_plates WHERE project_id=?1)",[imported.project_id.to_string()]).unwrap();
        fs::remove_file(&path).unwrap();

        service.confirm_new_print(&imported.source_hash).unwrap();
        let newest = service
            .latest_project_id(&imported.source_hash)
            .unwrap()
            .unwrap();
        let confirmed = service.get_project_preview(newest).unwrap();

        assert_eq!(
            confirmed.plates[0].thumbnail_url,
            imported.plates[0].thumbnail_url
        );
        assert_eq!(count(&service, "media_assets"), 2);
        fs::remove_dir_all(media_root).unwrap();
    }

    #[test]
    fn reimport_backfills_missing_legacy_project_thumbnails_and_source_path() {
        let database = AppDatabase::open_in_memory().unwrap();
        let media_root =
            std::env::temp_dir().join(format!("cylune-media-backfill-{}", uuid::Uuid::new_v4()));
        let media_store = MediaStore::new(media_root.clone()).unwrap();
        let mut service = PrintService::with_media_store_and_stability_delay(
            database,
            media_store,
            Duration::ZERO,
        );
        let path = two_plate_fixture_with_shared_thumbnail();
        let imported = service.import_print_project(&path).unwrap();
        let cached_json: String = service
            .database
            .connection
            .query_row(
                "SELECT parsed_json FROM parse_cache WHERE source_hash = ?1",
                [&imported.source_hash],
                |row| row.get(0),
            )
            .unwrap();
        let mut cached: crate::parser::ParsedProjectV2 =
            serde_json::from_str(&cached_json).unwrap();
        for plate in &mut cached.plates {
            plate.thumbnail_entries = vec![format!("Metadata/plate_{}.png", plate.plate_index)];
        }
        service
            .database
            .connection
            .execute(
                "UPDATE parse_cache SET parsed_json = ?1 WHERE source_hash = ?2",
                params![
                    serde_json::to_string(&cached).unwrap(),
                    imported.source_hash
                ],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_projects
                 SET source_path = NULL, cover_asset_id = NULL
                 WHERE project_id = ?1",
                [imported.project_id.to_string()],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_plates SET thumbnail_asset_id = NULL WHERE project_id = ?1",
                [imported.project_id.to_string()],
            )
            .unwrap();
        service
            .database
            .connection
            .execute("DELETE FROM media_assets", [])
            .unwrap();
        let projects_before = count(&service, "print_projects");
        let plates_before = count(&service, "print_plates");
        let jobs_before = count(&service, "print_jobs");
        let consumption_before = count(&service, "job_consumption");
        let ledger_before = count(&service, "ledger_events");

        let reopened = service.import_print_project(&path).unwrap();

        assert_eq!(reopened.project_id, imported.project_id);
        assert_eq!(reopened.state, ImportState::ExistingPending);
        assert!(reopened
            .plates
            .iter()
            .all(|plate| plate.thumbnail_url.is_some()));
        assert_eq!(count(&service, "print_projects"), projects_before);
        assert_eq!(count(&service, "print_plates"), plates_before);
        assert_eq!(count(&service, "print_jobs"), jobs_before);
        assert_eq!(count(&service, "job_consumption"), consumption_before);
        assert_eq!(count(&service, "ledger_events"), ledger_before);
        assert_eq!(count(&service, "media_assets"), 2);
        let (source_path, cover_asset_id): (String, String) = service
            .database
            .connection
            .query_row(
                "SELECT source_path, cover_asset_id
                 FROM print_projects
                 WHERE project_id = ?1",
                [imported.project_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(source_path, path.to_string_lossy());
        let plate_assets: Vec<String> = {
            let mut statement = service
                .database
                .connection
                .prepare(
                    "SELECT thumbnail_asset_id
                     FROM print_plates
                     WHERE project_id = ?1
                     ORDER BY plate_index",
                )
                .unwrap();
            statement
                .query_map([imported.project_id.to_string()], |row| row.get(0))
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap()
        };
        assert_eq!(plate_assets.len(), 2);
        assert_eq!(plate_assets[0], cover_asset_id);
        assert_ne!(plate_assets[0], plate_assets[1]);
        let relative_path: String = service
            .database
            .connection
            .query_row(
                "SELECT relative_path FROM media_assets WHERE asset_id = ?1",
                [&cover_asset_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            fs::read(media_root.join(relative_path)).unwrap(),
            png_pixel([220, 48, 60, 255])
        );
        let refreshed: crate::parser::ParsedProjectV2 = service
            .persisted_project(&imported.source_hash)
            .unwrap()
            .unwrap();
        assert!(refreshed
            .plates
            .iter()
            .all(
                |plate| plate.thumbnail_entries.first().is_some_and(|entry| {
                    entry == &format!("Metadata/plate_{}.png", plate.plate_index)
                })
            ));
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(media_root).unwrap();
    }

    #[test]
    fn reimport_replaces_existing_legacy_thumbnail_associations() {
        let database = AppDatabase::open_in_memory().unwrap();
        let media_root =
            std::env::temp_dir().join(format!("cylune-media-upgrade-{}", uuid::Uuid::new_v4()));
        let media_store = MediaStore::new(media_root.clone()).unwrap();
        let mut service = PrintService::with_media_store_and_stability_delay(
            database,
            media_store,
            Duration::ZERO,
        );
        let path = two_plate_fixture_with_shared_thumbnail();
        let imported = service.import_print_project(&path).unwrap();
        let legacy = MediaStore::new(media_root.clone())
            .unwrap()
            .extract_image(&path, "Auxiliaries/.thumbnails/thumbnail_middle.png")
            .unwrap()
            .unwrap();
        service
            .database
            .connection
            .execute(
                "INSERT OR IGNORE INTO media_assets (
                    asset_id, relative_path, mime_type, byte_size, width, height
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    &legacy.asset_id,
                    &legacy.relative_path,
                    &legacy.mime_type,
                    legacy.byte_size,
                    legacy.width,
                    legacy.height,
                ],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_projects SET cover_asset_id = ?1 WHERE project_id = ?2",
                params![&legacy.asset_id, imported.project_id.to_string()],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_plates SET thumbnail_asset_id = ?1 WHERE project_id = ?2",
                params![&legacy.asset_id, imported.project_id.to_string()],
            )
            .unwrap();
        let projects_before = count(&service, "print_projects");
        let jobs_before = count(&service, "print_jobs");

        let reopened = service.import_print_project(&path).unwrap();

        assert_eq!(reopened.project_id, imported.project_id);
        assert_eq!(count(&service, "print_projects"), projects_before);
        assert_eq!(count(&service, "print_jobs"), jobs_before);
        let (distinct_assets, legacy_assets): (u32, u32) = service
            .database
            .connection
            .query_row(
                "SELECT COUNT(DISTINCT thumbnail_asset_id),
                        SUM(thumbnail_asset_id = ?1)
                 FROM print_plates
                 WHERE project_id = ?2",
                params![&legacy.asset_id, imported.project_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(distinct_assets, 2);
        assert_eq!(legacy_assets, 0);
        let cover_asset_id: String = service
            .database
            .connection
            .query_row(
                "SELECT cover_asset_id FROM print_projects WHERE project_id = ?1",
                [imported.project_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(cover_asset_id, legacy.asset_id);
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(media_root).unwrap();
    }

    #[test]
    fn reimport_recreates_a_missing_content_addressed_thumbnail_file() {
        let database = AppDatabase::open_in_memory().unwrap();
        let media_root =
            std::env::temp_dir().join(format!("cylune-media-repair-{}", uuid::Uuid::new_v4()));
        let media_store = MediaStore::new(media_root.clone()).unwrap();
        let mut service = PrintService::with_media_store_and_stability_delay(
            database,
            media_store,
            Duration::ZERO,
        );
        let path = two_plate_fixture_with_shared_thumbnail();
        let imported = service.import_print_project(&path).unwrap();
        let relative_path: String = service
            .database
            .connection
            .query_row(
                "SELECT assets.relative_path
                 FROM print_plates AS plates
                 JOIN media_assets AS assets ON assets.asset_id = plates.thumbnail_asset_id
                 WHERE plates.project_id = ?1 AND plates.plate_index = 1",
                [imported.project_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        let thumbnail_path = media_root.join(relative_path);
        fs::remove_file(&thumbnail_path).unwrap();
        assert!(!thumbnail_path.exists());
        let projects_before = count(&service, "print_projects");
        let jobs_before = count(&service, "print_jobs");

        let reopened = service.import_print_project(&path).unwrap();

        assert_eq!(reopened.project_id, imported.project_id);
        assert!(thumbnail_path.is_file());
        assert_eq!(count(&service, "print_projects"), projects_before);
        assert_eq!(count(&service, "print_jobs"), jobs_before);
        assert_eq!(count(&service, "media_assets"), 2);
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(media_root).unwrap();
    }

    #[test]
    fn reimport_rejects_cached_plate_identity_drift_before_attaching_media() {
        let database = AppDatabase::open_in_memory().unwrap();
        let media_root =
            std::env::temp_dir().join(format!("cylune-media-identity-{}", uuid::Uuid::new_v4()));
        let media_store = MediaStore::new(media_root.clone()).unwrap();
        let mut service = PrintService::with_media_store_and_stability_delay(
            database,
            media_store,
            Duration::ZERO,
        );
        let path = two_plate_fixture_with_shared_thumbnail();
        let imported = service.import_print_project(&path).unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs
                 SET outcome = '{\"kind\":\"success\"}', settlement_version = 1
                 WHERE plate_id IN (
                    SELECT plate_id FROM print_plates WHERE project_id = ?1
                 )",
                [imported.project_id.to_string()],
            )
            .unwrap();
        let mut cached = service
            .persisted_project(&imported.source_hash)
            .unwrap()
            .unwrap();
        cached.plates[0].plate_index = 99;
        let broken_json = serde_json::to_string(&cached).unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE parse_cache SET parsed_json = ?1 WHERE source_hash = ?2",
                params![&broken_json, &imported.source_hash],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_projects
                 SET source_path = NULL, cover_asset_id = NULL
                 WHERE project_id = ?1",
                [imported.project_id.to_string()],
            )
            .unwrap();

        let error = service.import_print_project(&path).unwrap_err();

        assert_eq!(error.code(), "database");
        let (source_path, cover_asset_id): (Option<String>, Option<String>) = service
            .database
            .connection
            .query_row(
                "SELECT source_path, cover_asset_id
                 FROM print_projects
                 WHERE project_id = ?1",
                [imported.project_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(source_path.is_none());
        assert!(cover_asset_id.is_none());
        let persisted_json: String = service
            .database
            .connection
            .query_row(
                "SELECT parsed_json FROM parse_cache WHERE source_hash = ?1",
                [&imported.source_hash],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persisted_json, broken_json);
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(media_root).unwrap();
    }

    #[test]
    fn cached_project_versions_other_than_two_are_rejected() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let imported = service.import_print_project(&path).unwrap();
        service.discard_project(imported.project_id).unwrap();
        service.database.connection.execute("UPDATE parse_cache SET parsed_json=json_set(parsed_json,'$.version',3) WHERE source_hash=?1",[imported.source_hash]).unwrap();

        let error = service.import_print_project(&path).unwrap_err();

        fs::remove_file(path).unwrap();
        assert_eq!(error.code(), "invalid_file");
        assert_eq!(count(&service, "print_projects"), 0);
    }

    #[test]
    fn distinct_plate_media_and_confirmed_reimport_are_content_deduplicated() {
        let database = AppDatabase::open_in_memory().unwrap();
        let media_root =
            std::env::temp_dir().join(format!("cylune-project-media-{}", uuid::Uuid::new_v4()));
        let media_store = MediaStore::new(media_root.clone()).unwrap();
        let mut service = PrintService::with_media_store_and_stability_delay(
            database,
            media_store,
            Duration::ZERO,
        );
        let path = two_plate_fixture_with_shared_thumbnail();

        let first = service.import_print_project(&path).unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs
                 SET outcome = '{\"kind\":\"success\"}', settlement_version = 1
                 WHERE plate_id IN (
                    SELECT plate_id FROM print_plates WHERE project_id = ?1
                 )",
                [first.project_id.to_string()],
            )
            .unwrap();
        let confirmed = service
            .confirm_new_project(&first.source_hash, &path)
            .unwrap();

        fs::remove_file(path).unwrap();
        assert_ne!(first.plates[0].thumbnail_url, first.plates[1].thumbnail_url);
        assert_eq!(
            confirmed.plates[0].thumbnail_url,
            first.plates[0].thumbnail_url
        );
        assert_eq!(count(&service, "media_assets"), 2);
        assert_eq!(media_file_count(&media_root), 2);
        fs::remove_dir_all(media_root).unwrap();
    }

    #[test]
    fn discard_project_atomically_removes_pending_batch_and_preserves_cache_and_ledger() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let spool_id = inventory
            .create_spool(new_spool("Bambu PLA Basic", "#FFFFFF"))
            .unwrap();
        let database = inventory.into_database();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let preview = service.import_print_project(&path).unwrap();
        service
            .database
            .connection
            .execute(
                "INSERT INTO job_mappings (job_id, tool, spool_id)
                 VALUES (?1, 0, ?2)",
                [preview.plates[0].job_id.to_string(), spool_id.to_string()],
            )
            .unwrap();
        let ledger_before = count(&service, "ledger_events");

        service.discard_project(preview.project_id).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(count(&service, "print_projects"), 0);
        assert_eq!(count(&service, "print_plates"), 0);
        assert_eq!(count(&service, "print_jobs"), 0);
        assert_eq!(count(&service, "job_mappings"), 0);
        assert_eq!(count(&service, "parse_cache"), 1);
        assert_eq!(count(&service, "ledger_events"), ledger_before);
    }

    #[test]
    fn discard_project_rejects_any_settled_plate_without_mutation() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let preview = service.import_print_project(&path).unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs
                 SET outcome = '{\"kind\":\"success\"}', settlement_version = 1
                 WHERE job_id = ?1",
                [preview.plates[0].job_id.to_string()],
            )
            .unwrap();
        let before = (
            count(&service, "print_projects"),
            count(&service, "print_plates"),
            count(&service, "print_jobs"),
            count(&service, "parse_cache"),
            count(&service, "ledger_events"),
        );

        let error = service.discard_project(preview.project_id).unwrap_err();

        fs::remove_file(path).unwrap();
        assert_eq!(error.code(), "invalid_job");
        assert_eq!(
            (
                count(&service, "print_projects"),
                count(&service, "print_plates"),
                count(&service, "print_jobs"),
                count(&service, "parse_cache"),
                count(&service, "ledger_events"),
            ),
            before
        );
    }

    #[test]
    fn skip_plate_is_only_allowed_after_the_project_can_no_longer_be_discarded() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let imported = service.import_print_project(&path).unwrap();
        let skipped_plate = imported.plates[1].plate_id;

        let early_error = service.skip_plate(skipped_plate).unwrap_err();
        assert_eq!(early_error.code(), "invalid_job");
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs
                 SET outcome = '{\"kind\":\"success\"}', settlement_version = 1
                 WHERE job_id = ?1",
                [imported.plates[0].job_id.to_string()],
            )
            .unwrap();
        let ledger_before = count(&service, "ledger_events");

        service.skip_plate(skipped_plate).unwrap();
        let preview = service.get_project_preview(imported.project_id).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(
            preview.plates[1].status,
            crate::domain::PlateStatus::Skipped
        );
        assert_eq!(count(&service, "job_consumption"), 0);
        assert_eq!(count(&service, "ledger_events"), ledger_before);
        assert_eq!(
            service
                .discard_project(imported.project_id)
                .unwrap_err()
                .code(),
            "invalid_job"
        );
    }

    #[test]
    fn legacy_import_print_file_returns_plate_one_from_the_project_batch() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();

        let legacy = service.import_print_file(&path).unwrap();
        let project_id: String = service
            .database
            .connection
            .query_row(
                "SELECT project_id
                 FROM print_plates
                 WHERE plate_id = (
                    SELECT plate_id FROM print_jobs WHERE job_id = ?1
                 )",
                [legacy.job_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        let project = service
            .get_project_preview(project_id.parse().unwrap())
            .unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(legacy.job_id, project.plates[0].job_id);
        assert_eq!(legacy.max_layer, project.plates[0].max_layer);
        assert_eq!(count(&service, "print_projects"), 1);
        assert_eq!(count(&service, "print_jobs"), 2);
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
    fn project_preview_restores_each_plates_saved_mappings() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let spool = inventory
            .create_spool(new_spool("Bambu PLA Basic", "#FFFFFF"))
            .unwrap();
        let database = inventory.into_database();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let path = two_plate_fixture();
        let imported = service.import_print_project(&path).unwrap();
        service
            .confirm_job_mapping(
                imported.plates[1].job_id,
                vec![ToolMapping {
                    tool: 0,
                    spool_id: spool,
                }],
            )
            .unwrap();

        let reopened = service.get_project_preview(imported.project_id).unwrap();

        fs::remove_file(path).unwrap();
        assert!(reopened.plates[0].mappings.is_empty());
        assert_eq!(
            reopened.plates[1].mappings,
            vec![ToolMapping {
                tool: 0,
                spool_id: spool,
            }]
        );
        assert_eq!(reopened.plates[1].status, crate::domain::PlateStatus::Ready);
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
    history::{status_for_job, ImportPlatePreview, ImportProjectPreview},
    media::{MediaAsset, MediaStore},
    parser::{
        parse_3mf_project, preset_base, FilamentProfile, ParsedPlate, ParsedPrintFile,
        ParsedProjectV2,
    },
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
    media_store: Option<MediaStore>,
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
            media_store: None,
            #[cfg(test)]
            before_final_stability_check: None,
        }
    }

    pub fn with_media_store_and_stability_delay(
        database: AppDatabase,
        media_store: MediaStore,
        stability_delay: Duration,
    ) -> Self {
        Self {
            database,
            stability_delay,
            media_store: Some(media_store),
            #[cfg(test)]
            before_final_stability_check: None,
        }
    }

    pub fn with_media_store(database: AppDatabase, media_store: MediaStore) -> Self {
        Self::with_media_store_and_stability_delay(
            database,
            media_store,
            Duration::from_millis(750),
        )
    }

    pub fn import_print_project(&mut self, path: &Path) -> Result<ImportProjectPreview> {
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("gcode"))
        {
            return Err(AppError::StandaloneGcodeProfilesRequired);
        }
        let stability = self.ensure_stable(path)?;
        let source_hash = sha256(path)?;
        if let Some(parsed) = self.persisted_project(&source_hash)? {
            if let Some(preview) = self.continue_project(&source_hash)? {
                let (parsed, media, refreshed_json) =
                    self.recover_project_media(&source_hash, path, &parsed)?;
                #[cfg(test)]
                self.run_before_final_stability_check(path);
                self.ensure_unchanged(path, stability)?;
                self.attach_project_media(
                    preview.project_id,
                    &source_hash,
                    path,
                    &parsed,
                    &media,
                    refreshed_json.as_deref(),
                )?;
                return self.project_preview_from_database(
                    preview.project_id,
                    ImportState::ExistingPending,
                );
            }
            if let Some(project_id) = self.latest_project_id(&source_hash)? {
                let (parsed, media, refreshed_json) =
                    self.recover_project_media(&source_hash, path, &parsed)?;
                #[cfg(test)]
                self.run_before_final_stability_check(path);
                self.ensure_unchanged(path, stability)?;
                self.attach_project_media(
                    project_id,
                    &source_hash,
                    path,
                    &parsed,
                    &media,
                    refreshed_json.as_deref(),
                )?;
                let preview = self.project_preview_from_database(
                    project_id,
                    ImportState::NewPrintConfirmationRequired,
                )?;
                return Ok(preview);
            }
            for plate in &parsed.plates {
                validate_plate_profiles(plate)?;
            }
            let source_file_name: String = self.database.connection.query_row(
                "SELECT source_file_name FROM parse_cache WHERE source_hash = ?1",
                [&source_hash],
                |row| row.get(0),
            )?;
            let (parsed, media, refreshed_json) =
                self.recover_project_media(&source_hash, path, &parsed)?;
            #[cfg(test)]
            self.run_before_final_stability_check(path);
            self.ensure_unchanged(path, stability)?;
            if let Some(parsed_json) = refreshed_json {
                self.database.connection.execute(
                    "UPDATE parse_cache SET parsed_json = ?1 WHERE source_hash = ?2",
                    params![parsed_json, &source_hash],
                )?;
            }
            return self.create_project_from_parsed(
                source_hash,
                source_file_name,
                path.to_string_lossy().into_owned(),
                &parsed,
                ImportState::New,
                None,
                &media,
            );
        }
        let source_file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or(AppError::InvalidFile)?
            .to_owned();
        let parsed = parse_3mf_project(path)?;
        for plate in &parsed.plates {
            validate_plate_profiles(plate)?;
        }
        let parsed_json = serde_json::to_string(&parsed)
            .map_err(|error| AppError::Database(error.to_string()))?;
        let media = self.extract_project_media(path, &parsed)?;
        #[cfg(test)]
        self.run_before_final_stability_check(path);
        self.ensure_unchanged(path, stability)?;

        let source_path = path.to_string_lossy().into_owned();
        self.create_project_from_parsed(
            source_hash,
            source_file_name,
            source_path,
            &parsed,
            ImportState::New,
            Some(parsed_json),
            &media,
        )
    }

    pub fn continue_project(&self, source_hash: &str) -> Result<Option<ImportProjectPreview>> {
        let project_id: Option<String> = self
            .database
            .connection
            .query_row(
                "SELECT projects.project_id
                 FROM print_projects AS projects
                 WHERE projects.source_hash = ?1
                   AND EXISTS (
                       SELECT 1
                       FROM print_plates AS plates
                       JOIN print_jobs AS jobs ON jobs.plate_id = plates.plate_id
                       WHERE plates.project_id = projects.project_id
                         AND jobs.outcome IS NULL
                   )
                 ORDER BY projects.rowid DESC
                 LIMIT 1",
                [source_hash],
                |row| row.get(0),
            )
            .optional()?;
        project_id
            .map(|project_id| {
                let project_id = parse_uuid(&project_id)?;
                self.project_preview_from_database(project_id, ImportState::ExistingPending)
            })
            .transpose()
    }

    pub fn get_project_preview(&self, project_id: Uuid) -> Result<ImportProjectPreview> {
        let has_pending: bool = self.database.connection.query_row(
            "SELECT EXISTS(
                    SELECT 1
                    FROM print_plates AS plates
                    JOIN print_jobs AS jobs ON jobs.plate_id = plates.plate_id
                    WHERE plates.project_id = ?1
                      AND jobs.outcome IS NULL
                 )",
            [project_id.to_string()],
            |row| row.get(0),
        )?;
        self.project_preview_from_database(
            project_id,
            if has_pending {
                ImportState::ExistingPending
            } else {
                ImportState::NewPrintConfirmationRequired
            },
        )
    }

    pub fn confirm_new_project(
        &mut self,
        source_hash: &str,
        source_path: &Path,
    ) -> Result<ImportProjectPreview> {
        if let Some(project) = self.continue_project(source_hash)? {
            return Ok(project);
        }
        let stability = self.ensure_stable(source_path)?;
        if sha256(source_path)? != source_hash {
            return Err(AppError::InvalidFile);
        }
        let parsed = self
            .persisted_project(source_hash)?
            .ok_or(AppError::InvalidJob)?;
        for plate in &parsed.plates {
            validate_plate_profiles(plate)?;
        }
        let source_file_name: String = self.database.connection.query_row(
            "SELECT source_file_name FROM parse_cache WHERE source_hash = ?1",
            [source_hash],
            |row| row.get(0),
        )?;
        let mut media = self.persisted_project_media(source_hash, &parsed)?;
        if media.iter().any(Option::is_none) {
            let extracted = self.extract_project_media(source_path, &parsed)?;
            for (saved, extracted) in media.iter_mut().zip(extracted) {
                if saved.is_none() {
                    *saved = extracted;
                }
            }
        }
        #[cfg(test)]
        self.run_before_final_stability_check(source_path);
        self.ensure_unchanged(source_path, stability)?;
        self.create_project_from_parsed(
            source_hash.to_owned(),
            source_file_name,
            source_path.to_string_lossy().into_owned(),
            &parsed,
            ImportState::New,
            None,
            &media,
        )
    }

    pub fn discard_project(&mut self, project_id: Uuid) -> Result<()> {
        let transaction = self.database.connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM print_projects WHERE project_id = ?1
             )",
            [project_id.to_string()],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::InvalidJob);
        }
        let unsafe_to_discard: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM print_plates AS plates
                JOIN print_jobs AS jobs ON jobs.plate_id = plates.plate_id
                WHERE plates.project_id = ?1
                  AND (
                    jobs.outcome IS NOT NULL
                    OR jobs.settlement_version > 0
                    OR EXISTS (
                        SELECT 1 FROM job_consumption
                        WHERE job_id = jobs.job_id
                    )
                    OR EXISTS (
                        SELECT 1 FROM ledger_events
                        WHERE job_id = jobs.job_id
                    )
                  )
             )",
            [project_id.to_string()],
            |row| row.get(0),
        )?;
        if unsafe_to_discard {
            return Err(AppError::InvalidJob);
        }
        transaction.execute(
            "DELETE FROM app_settings
             WHERE setting_key = 'pending_job_id'
               AND setting_value IN (
                    SELECT jobs.job_id
                    FROM print_jobs AS jobs
                    JOIN print_plates AS plates ON plates.plate_id = jobs.plate_id
                    WHERE plates.project_id = ?1
               )",
            [project_id.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM job_mappings
             WHERE job_id IN (
                SELECT jobs.job_id
                FROM print_jobs AS jobs
                JOIN print_plates AS plates ON plates.plate_id = jobs.plate_id
                WHERE plates.project_id = ?1
             )",
            [project_id.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM print_jobs
             WHERE plate_id IN (
                SELECT plate_id FROM print_plates WHERE project_id = ?1
             )",
            [project_id.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM print_plates WHERE project_id = ?1",
            [project_id.to_string()],
        )?;
        transaction.execute(
            "DELETE FROM print_projects WHERE project_id = ?1",
            [project_id.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn skip_plate(&mut self, plate_id: Uuid) -> Result<()> {
        let transaction = self.database.connection.transaction()?;
        let row: Option<(String, String, Option<String>)> = transaction
            .query_row(
                "SELECT plates.project_id, jobs.job_id, jobs.outcome
                 FROM print_plates AS plates
                 JOIN print_jobs AS jobs ON jobs.plate_id = plates.plate_id
                 WHERE plates.plate_id = ?1",
                [plate_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((project_id, job_id, outcome)) = row else {
            return Err(AppError::InvalidJob);
        };
        if outcome.is_some() {
            return Err(AppError::InvalidJob);
        }
        let discard_is_unsafe: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1
                FROM print_plates AS plates
                JOIN print_jobs AS jobs ON jobs.plate_id = plates.plate_id
                WHERE plates.project_id = ?1
                  AND (
                    jobs.outcome IS NOT NULL
                    OR jobs.settlement_version > 0
                    OR EXISTS (
                        SELECT 1 FROM job_consumption
                        WHERE job_id = jobs.job_id
                    )
                    OR EXISTS (
                        SELECT 1 FROM ledger_events
                        WHERE job_id = jobs.job_id
                    )
                  )
             )",
            [&project_id],
            |row| row.get(0),
        )?;
        if !discard_is_unsafe {
            return Err(AppError::InvalidJob);
        }
        let changed = transaction.execute(
            "UPDATE print_jobs
             SET outcome = '{\"kind\":\"skipped\"}'
             WHERE job_id = ?1
               AND outcome IS NULL
               AND settlement_version = 0
               AND NOT EXISTS (
                    SELECT 1 FROM job_consumption WHERE job_id = ?1
               )",
            [&job_id],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidJob);
        }
        transaction.execute(
            "DELETE FROM app_settings
             WHERE setting_key = 'pending_job_id'
               AND setting_value = ?1",
            [&job_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn create_project_from_parsed(
        &mut self,
        source_hash: String,
        source_file_name: String,
        source_path: String,
        parsed: &ParsedProjectV2,
        state: ImportState,
        parse_cache_json: Option<String>,
        media: &[Option<MediaAsset>],
    ) -> Result<ImportProjectPreview> {
        let project_id = Uuid::new_v4();
        let transaction = self.database.connection.transaction()?;
        if let Some(parsed_json) = parse_cache_json {
            transaction.execute(
                "INSERT INTO parse_cache (
                    source_hash,
                    source_file_name,
                    parsed_json,
                    parse_count
                 ) VALUES (?1, ?2, ?3, 1)",
                params![source_hash, source_file_name, parsed_json],
            )?;
        }
        for asset in media.iter().flatten() {
            transaction.execute(
                "INSERT OR IGNORE INTO media_assets (
                    asset_id,
                    relative_path,
                    mime_type,
                    byte_size,
                    width,
                    height
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    asset.asset_id,
                    asset.relative_path,
                    asset.mime_type,
                    asset.byte_size,
                    asset.width,
                    asset.height,
                ],
            )?;
        }
        let cover_asset_id = media.iter().flatten().next().map(|asset| &asset.asset_id);
        transaction.execute(
            "INSERT INTO print_projects (
                project_id,
                source_hash,
                source_file_name,
                source_path,
                plate_count,
                cover_asset_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                project_id.to_string(),
                source_hash,
                source_file_name,
                source_path,
                parsed.plates.len() as u32,
                cover_asset_id,
            ],
        )?;

        let mut identities = Vec::with_capacity(parsed.plates.len());
        for (index, plate) in parsed.plates.iter().enumerate() {
            let plate_id = Uuid::new_v4();
            let job_id = Uuid::new_v4();
            let thumbnail_asset_id = media
                .get(index)
                .and_then(Option::as_ref)
                .map(|asset| &asset.asset_id);
            let plate_json = serde_json::to_string(&ParsedPrintFile {
                filaments: plate.filaments.clone(),
                gcode: plate.gcode.clone(),
            })
            .map_err(|error| AppError::Database(error.to_string()))?;
            transaction.execute(
                "INSERT INTO print_plates (
                    plate_id,
                    project_id,
                    plate_index,
                    display_name,
                    thumbnail_asset_id,
                    estimated_seconds,
                    max_layer,
                    parsed_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    plate_id.to_string(),
                    project_id.to_string(),
                    plate.plate_index,
                    plate.display_name,
                    thumbnail_asset_id,
                    plate.estimated_seconds,
                    plate.gcode.max_layer,
                    plate_json,
                ],
            )?;
            transaction.execute(
                "INSERT INTO print_jobs (
                    job_id,
                    source_hash,
                    source_file_name,
                    plate_id
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    job_id.to_string(),
                    source_hash,
                    source_file_name,
                    plate_id.to_string(),
                ],
            )?;
            identities.push((plate_id, job_id));
        }
        let imported_at = transaction.query_row(
            "SELECT imported_at FROM print_projects WHERE project_id = ?1",
            [project_id.to_string()],
            |row| row.get::<_, String>(0),
        )?;
        transaction.commit()?;

        self.project_preview(
            project_id,
            source_hash,
            source_file_name,
            imported_at,
            parsed,
            &identities,
            state,
            media,
        )
    }

    fn extract_project_media(
        &self,
        source_path: &Path,
        parsed: &ParsedProjectV2,
    ) -> Result<Vec<Option<MediaAsset>>> {
        let Some(store) = &self.media_store else {
            return Ok(vec![None; parsed.plates.len()]);
        };
        parsed
            .plates
            .iter()
            .map(|plate| {
                for entry in &plate.thumbnail_entries {
                    if let Some(asset) = store.extract_image(source_path, entry)? {
                        return Ok(Some(asset));
                    }
                }
                Ok(None)
            })
            .collect()
    }

    fn persisted_project_media(
        &self,
        source_hash: &str,
        parsed: &ParsedProjectV2,
    ) -> Result<Vec<Option<MediaAsset>>> {
        parsed
            .plates
            .iter()
            .map(|plate| {
                self.database
                    .connection
                    .query_row(
                        "SELECT assets.asset_id, assets.relative_path, assets.mime_type,
                                assets.width, assets.height, assets.byte_size
                         FROM print_projects AS projects
                         JOIN print_plates AS plates USING(project_id)
                         JOIN media_assets AS assets ON assets.asset_id = plates.thumbnail_asset_id
                         WHERE projects.source_hash = ?1
                           AND plates.plate_index = ?2
                         ORDER BY projects.rowid DESC
                         LIMIT 1",
                        params![source_hash, plate.plate_index],
                        |row| {
                            Ok(MediaAsset {
                                asset_id: row.get(0)?,
                                relative_path: row.get(1)?,
                                mime_type: row.get(2)?,
                                width: row.get(3)?,
                                height: row.get(4)?,
                                byte_size: row.get(5)?,
                            })
                        },
                    )
                    .optional()
                    .map_err(Into::into)
            })
            .collect()
    }

    fn recover_project_media(
        &self,
        source_hash: &str,
        source_path: &Path,
        cached: &ParsedProjectV2,
    ) -> Result<(ParsedProjectV2, Vec<Option<MediaAsset>>, Option<String>)> {
        let mut media = self.persisted_project_media(source_hash, cached)?;
        if self.media_store.is_none() {
            return Ok((cached.clone(), media, None));
        }

        // Reparse the user-selected source even when the database already has a
        // media row. This upgrades legacy monochrome choices and lets extraction
        // recreate a content-addressed file that was removed outside the app.
        let reparsed = parse_3mf_project(source_path)?;
        for plate in &reparsed.plates {
            validate_plate_profiles(plate)?;
        }
        let cached_plate_indices = cached
            .plates
            .iter()
            .map(|plate| plate.plate_index)
            .collect::<Vec<_>>();
        let parsed_plate_indices = reparsed
            .plates
            .iter()
            .map(|plate| plate.plate_index)
            .collect::<Vec<_>>();
        if cached_plate_indices != parsed_plate_indices || reparsed.plates.len() != media.len() {
            return Err(AppError::Database(
                "cached project plate identity mismatch".to_owned(),
            ));
        }
        let mut parsed = cached.clone();
        for (saved_plate, reparsed_plate) in parsed.plates.iter_mut().zip(&reparsed.plates) {
            saved_plate.thumbnail_entries = reparsed_plate.thumbnail_entries.clone();
        }
        let extracted = self.extract_project_media(source_path, &parsed)?;
        for (saved, fresh) in media.iter_mut().zip(extracted) {
            if fresh.is_some() {
                *saved = fresh;
            }
        }
        let refreshed_json = serde_json::to_string(&parsed)
            .map_err(|error| AppError::Database(error.to_string()))?;
        Ok((parsed, media, Some(refreshed_json)))
    }

    fn attach_project_media(
        &mut self,
        project_id: Uuid,
        source_hash: &str,
        source_path: &Path,
        parsed: &ParsedProjectV2,
        media: &[Option<MediaAsset>],
        refreshed_parse_json: Option<&str>,
    ) -> Result<()> {
        if parsed.plates.len() != media.len() {
            return Err(AppError::Database(
                "project media plate count mismatch".to_owned(),
            ));
        }
        let transaction = self.database.connection.transaction()?;
        for asset in media.iter().flatten() {
            transaction.execute(
                "INSERT OR IGNORE INTO media_assets (
                    asset_id, relative_path, mime_type, byte_size, width, height
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    asset.asset_id,
                    asset.relative_path,
                    asset.mime_type,
                    asset.byte_size,
                    asset.width,
                    asset.height,
                ],
            )?;
        }
        for (plate, asset) in parsed.plates.iter().zip(media) {
            let Some(asset) = asset else { continue };
            let changed = transaction.execute(
                "UPDATE print_plates
                 SET thumbnail_asset_id = ?1
                 WHERE project_id = ?2 AND plate_index = ?3",
                params![asset.asset_id, project_id.to_string(), plate.plate_index],
            )?;
            if changed != 1 {
                return Err(AppError::Database(
                    "project media plate identity mismatch".to_owned(),
                ));
            }
        }
        let cover_asset_id = media.iter().flatten().next().map(|asset| &asset.asset_id);
        let changed = transaction.execute(
            "UPDATE print_projects
             SET source_path = ?1,
                 cover_asset_id = COALESCE(?2, cover_asset_id)
             WHERE project_id = ?3",
            params![
                source_path.to_string_lossy().into_owned(),
                cover_asset_id,
                project_id.to_string(),
            ],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidJob);
        }
        if let Some(parsed_json) = refreshed_parse_json {
            transaction.execute(
                "UPDATE parse_cache SET parsed_json = ?1 WHERE source_hash = ?2",
                params![parsed_json, source_hash],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn import_print_file(&mut self, path: &Path) -> Result<ImportPreview> {
        legacy_preview(self.import_print_project(path)?)
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
        let project_id: Option<String> = self
            .database
            .connection
            .query_row(
                "SELECT plates.project_id
                 FROM print_jobs AS jobs
                 JOIN print_plates AS plates ON plates.plate_id = jobs.plate_id
                 WHERE jobs.job_id = ?1",
                [job_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(project_id) = project_id {
            return self.discard_project(parse_uuid(&project_id)?);
        }
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
        let parsed = self.parsed_job(job_id)?;
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
        if let Some(project) = self.continue_project(source_hash)? {
            return legacy_preview(project);
        }
        let source_path: Option<String> = self
            .database
            .connection
            .query_row(
                "SELECT source_path
                 FROM print_projects
                 WHERE source_hash = ?1
                 ORDER BY rowid DESC
                 LIMIT 1",
                [source_hash],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        let parsed = self
            .persisted_project(source_hash)?
            .ok_or(AppError::InvalidJob)?;
        let source_file_name: String = self.database.connection.query_row(
            "SELECT source_file_name FROM parse_cache WHERE source_hash = ?1",
            [source_hash],
            |row| row.get(0),
        )?;
        let media = self.persisted_project_media(source_hash, &parsed)?;
        legacy_preview(self.create_project_from_parsed(
            source_hash.to_owned(),
            source_file_name,
            source_path.unwrap_or_default(),
            &parsed,
            ImportState::New,
            None,
            &media,
        )?)
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
        let row: Option<(String, Option<String>)> = self
            .database
            .connection
            .query_row(
                "SELECT jobs.source_hash, plates.parsed_json
                 FROM print_jobs AS jobs
                 LEFT JOIN print_plates AS plates ON plates.plate_id = jobs.plate_id
                 WHERE jobs.job_id = ?1",
                params![job_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (source_hash, plate_json) = row.ok_or(AppError::InvalidJob)?;
        if let Some(json) = plate_json {
            return serde_json::from_str(&json)
                .map_err(|error| AppError::Database(error.to_string()));
        }
        self.persisted_parse(&source_hash)?
            .map(|(_, parsed)| parsed)
            .ok_or(AppError::InvalidJob)
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
            let parsed = if let Ok(parsed) = serde_json::from_str::<ParsedPrintFile>(&json) {
                parsed
            } else {
                let project: ParsedProjectV2 = serde_json::from_str(&json)
                    .map_err(|error| AppError::Database(error.to_string()))?;
                if project.version != 2 {
                    return Err(AppError::InvalidFile);
                }
                let plate = project
                    .plates
                    .into_iter()
                    .next()
                    .ok_or(AppError::InvalidJob)?;
                ParsedPrintFile {
                    filaments: plate.filaments,
                    gcode: plate.gcode,
                }
            };
            Ok((file_name, parsed))
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

    fn project_preview(
        &self,
        project_id: Uuid,
        source_hash: String,
        source_file_name: String,
        imported_at: String,
        parsed: &ParsedProjectV2,
        identities: &[(Uuid, Uuid)],
        state: ImportState,
        media: &[Option<MediaAsset>],
    ) -> Result<ImportProjectPreview> {
        let mut plates = Vec::with_capacity(parsed.plates.len());
        for (index, (plate, (plate_id, job_id))) in parsed.plates.iter().zip(identities).enumerate()
        {
            let parsed_file = ParsedPrintFile {
                filaments: plate.filaments.clone(),
                gcode: plate.gcode.clone(),
            };
            let legacy = self.preview(
                *job_id,
                source_hash.clone(),
                source_file_name.clone(),
                &parsed_file,
                state,
            )?;
            plates.push(ImportPlatePreview {
                plate_id: *plate_id,
                job_id: *job_id,
                plate_index: plate.plate_index,
                thumbnail_url: media
                    .get(index)
                    .and_then(Option::as_ref)
                    .map(|asset| asset.relative_path.clone()),
                estimated_seconds: plate.estimated_seconds,
                max_layer: plate.gcode.max_layer,
                filaments: legacy.filaments,
                mappings: Vec::new(),
                status: crate::domain::PlateStatus::PendingMapping,
            });
        }
        Ok(ImportProjectPreview {
            project_id,
            source_hash,
            source_file_name,
            imported_at,
            plates,
            state,
        })
    }

    fn project_preview_from_database(
        &self,
        project_id: Uuid,
        state: ImportState,
    ) -> Result<ImportProjectPreview> {
        let (source_hash, source_file_name, imported_at): (String, String, String) = self
            .database
            .connection
            .query_row(
                "SELECT source_hash, source_file_name, imported_at
                 FROM print_projects
                 WHERE project_id = ?1",
                [project_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(AppError::InvalidJob)?;
        let parsed = self
            .persisted_project(&source_hash)?
            .ok_or(AppError::InvalidJob)?;
        let mut statement = self.database.connection.prepare(
            "SELECT
                plates.plate_id,
                jobs.job_id,
                plates.plate_index,
                assets.relative_path,
                jobs.outcome,
                (SELECT COUNT(*) FROM job_mappings WHERE job_id = jobs.job_id)
             FROM print_plates AS plates
             JOIN print_jobs AS jobs ON jobs.plate_id = plates.plate_id
             LEFT JOIN media_assets AS assets ON assets.asset_id = plates.thumbnail_asset_id
             WHERE plates.project_id = ?1
             ORDER BY plates.plate_index",
        )?;
        let rows = statement
            .query_map([project_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u32>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, u32>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);

        let mut plates = Vec::with_capacity(rows.len());
        for (plate_id, job_id, plate_index, thumbnail_url, outcome, mapping_count) in rows {
            let plate = parsed
                .plates
                .iter()
                .find(|plate| plate.plate_index == plate_index)
                .ok_or_else(|| AppError::Database("missing cached plate".to_owned()))?;
            let parsed_file = ParsedPrintFile {
                filaments: plate.filaments.clone(),
                gcode: plate.gcode.clone(),
            };
            let job_id = parse_uuid(&job_id)?;
            let legacy = self.preview(
                job_id,
                source_hash.clone(),
                source_file_name.clone(),
                &parsed_file,
                state,
            )?;
            let mappings = self
                .job_mappings(job_id)?
                .into_iter()
                .map(|mapping| ToolMapping {
                    tool: mapping.tool,
                    spool_id: mapping.spool_id,
                })
                .collect();
            plates.push(ImportPlatePreview {
                plate_id: parse_uuid(&plate_id)?,
                job_id,
                plate_index,
                thumbnail_url,
                estimated_seconds: plate.estimated_seconds,
                max_layer: plate.gcode.max_layer,
                filaments: legacy.filaments,
                mappings,
                status: status_for_job(outcome.as_deref(), mapping_count, plate.filaments.len())?,
            });
        }
        Ok(ImportProjectPreview {
            project_id,
            source_hash,
            source_file_name,
            imported_at,
            plates,
            state,
        })
    }

    fn persisted_project(&self, source_hash: &str) -> Result<Option<ParsedProjectV2>> {
        let json: Option<String> = self
            .database
            .connection
            .query_row(
                "SELECT parsed_json FROM parse_cache WHERE source_hash = ?1",
                [source_hash],
                |row| row.get(0),
            )
            .optional()?;
        json.map(|json| {
            if let Ok(project) = serde_json::from_str::<ParsedProjectV2>(&json) {
                if project.version != 2 {
                    return Err(AppError::InvalidFile);
                }
                return Ok(project);
            }
            let legacy: ParsedPrintFile = serde_json::from_str(&json)
                .map_err(|error| AppError::Database(error.to_string()))?;
            Ok(ParsedProjectV2 {
                version: 2,
                plates: vec![ParsedPlate {
                    plate_index: 1,
                    display_name: None,
                    estimated_seconds: legacy.gcode.declared_estimated_seconds,
                    thumbnail_entries: Vec::new(),
                    filaments: legacy.filaments,
                    gcode: legacy.gcode,
                }],
            })
        })
        .transpose()
    }

    fn latest_project_id(&self, source_hash: &str) -> Result<Option<Uuid>> {
        let project_id: Option<String> = self
            .database
            .connection
            .query_row(
                "SELECT project_id
                 FROM print_projects
                 WHERE source_hash = ?1
                 ORDER BY rowid DESC
                 LIMIT 1",
                [source_hash],
                |row| row.get(0),
            )
            .optional()?;
        project_id
            .map(|project_id| parse_uuid(&project_id))
            .transpose()
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

fn validate_plate_profiles(parsed: &ParsedPlate) -> Result<()> {
    validate_profiles(&ParsedPrintFile {
        filaments: parsed.filaments.clone(),
        gcode: parsed.gcode.clone(),
    })
}

fn parse_uuid(value: &str) -> Result<Uuid> {
    value
        .parse()
        .map_err(|_| AppError::Database("invalid uuid".to_owned()))
}

fn legacy_preview(project: ImportProjectPreview) -> Result<ImportPreview> {
    let plate = project
        .plates
        .into_iter()
        .find(|plate| plate.plate_index == 1)
        .ok_or(AppError::InvalidJob)?;
    Ok(ImportPreview {
        job_id: plate.job_id,
        source_hash: project.source_hash,
        source_file_name: project.source_file_name,
        filaments: plate.filaments,
        max_layer: plate.max_layer,
        state: project.state,
    })
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
pub fn import_print_project(
    path: String,
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, crate::pet::runtime::PetRuntime>,
) -> Result<ImportProjectPreview> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    let preview = service.import_print_project(Path::new(&path))?;
    let summary = service.pending_summary()?;
    let job_id = preview
        .plates
        .first()
        .map(|plate| plate.job_id)
        .ok_or(AppError::InvalidJob)?;
    drop(service);
    runtime.refresh_pending(
        summary,
        Some(crate::pet::runtime::PetSignal::ImportSucceeded {
            job_id,
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
pub fn discard_project(
    project_id: Uuid,
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, crate::pet::runtime::PetRuntime>,
) -> Result<()> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    service.discard_project(project_id)?;
    let summary = service.pending_summary()?;
    drop(service);
    runtime.refresh_pending(summary, None);
    Ok(())
}

#[tauri::command]
pub fn skip_plate(
    plate_id: Uuid,
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, crate::pet::runtime::PetRuntime>,
) -> Result<()> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    service.skip_plate(plate_id)?;
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
pub fn confirm_new_project(
    source_hash: String,
    source_path: String,
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, crate::pet::runtime::PetRuntime>,
) -> Result<ImportProjectPreview> {
    let mut service = state
        .lock()
        .map_err(|_| AppError::Database("print service lock poisoned".to_owned()))?;
    let preview = service.confirm_new_project(&source_hash, Path::new(&source_path))?;
    let summary = service.pending_summary()?;
    let job_id = preview
        .plates
        .first()
        .map(|plate| plate.job_id)
        .ok_or(AppError::InvalidJob)?;
    drop(service);
    runtime.refresh_pending(
        summary,
        Some(crate::pet::runtime::PetSignal::ImportSucceeded {
            job_id,
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

#[tauri::command]
pub fn get_project_preview(
    project_id: Uuid,
    state: tauri::State<'_, PrintState>,
) -> Result<ImportProjectPreview> {
    state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?
        .get_project_preview(project_id)
}
