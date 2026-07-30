use crate::error::Result;
use rusqlite::{params, Connection};
use std::path::Path;

const INITIAL_MIGRATION: &str = include_str!("../migrations/001_init.sql");
const LEDGER_CREATION_MIGRATION: &str = include_str!("../migrations/002_ledger_creation.sql");
const PRINT_JOBS_MIGRATION: &str = include_str!("../migrations/003_print_jobs.sql");
const REPEAT_JOBS_MIGRATION: &str = include_str!("../migrations/004_repeat_jobs.sql");
const CATALOG_INDEX_MIGRATION: &str = include_str!("../migrations/005_catalog.sql");
const PRINT_HISTORY_MIGRATION: &str = include_str!("../migrations/006_print_history.sql");
const CATALOG_COLUMN_MIGRATIONS: [(&str, &str); 5] = [
    (
        "catalog_id",
        "ALTER TABLE spools ADD COLUMN catalog_id TEXT",
    ),
    (
        "color_name",
        "ALTER TABLE spools ADD COLUMN color_name TEXT",
    ),
    (
        "color_code",
        "ALTER TABLE spools ADD COLUMN color_code TEXT",
    ),
    (
        "color_hexes",
        "ALTER TABLE spools ADD COLUMN color_hexes TEXT",
    ),
    (
        "preset_base",
        "ALTER TABLE spools ADD COLUMN preset_base TEXT",
    ),
];

pub struct AppDatabase {
    pub(crate) connection: Connection,
}

impl AppDatabase {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self> {
        connection.execute_batch(INITIAL_MIGRATION)?;
        if !ledger_supports_creation(&connection)? {
            connection.execute_batch(LEDGER_CREATION_MIGRATION)?;
        }
        if table_exists(&connection, "job_consumption")?
            && !table_exists(&connection, "job_imports")?
            && !table_exists(&connection, "parse_cache")?
        {
            connection.execute_batch(PRINT_JOBS_MIGRATION)?;
        }
        if table_exists(&connection, "job_imports")? && !table_exists(&connection, "parse_cache")? {
            connection.execute_batch(REPEAT_JOBS_MIGRATION)?;
        }
        ensure_catalog_schema(&mut connection)?;
        ensure_print_history_schema(&mut connection)?;
        Ok(Self { connection })
    }

    pub fn table_exists(&self, table: &str) -> Result<bool> {
        let exists = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![table],
            |row| row.get::<_, i64>(0),
        )?;

        Ok(exists != 0)
    }
}

fn table_exists(connection: &Connection, table: &str) -> Result<bool> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
        params![table],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists != 0)
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> Result<bool> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2)",
        params![table, column],
        |row| row.get::<_, i64>(0),
    )?;
    Ok(exists != 0)
}

fn ensure_catalog_schema(connection: &mut Connection) -> Result<()> {
    let transaction = connection.transaction()?;
    for (column, migration) in CATALOG_COLUMN_MIGRATIONS {
        if !column_exists(&transaction, "spools", column)? {
            transaction.execute(migration, [])?;
        }
    }
    transaction.execute_batch(CATALOG_INDEX_MIGRATION)?;
    transaction.commit()?;
    Ok(())
}

fn ensure_print_history_schema(connection: &mut Connection) -> Result<()> {
    if table_exists(connection, "print_projects")? {
        return Ok(());
    }

    let transaction = connection.unchecked_transaction()?;
    transaction.execute_batch(PRINT_HISTORY_MIGRATION)?;
    let legacy_groups = {
        let mut statement = transaction.prepare(
            "SELECT
                jobs.source_hash,
                cache.source_file_name,
                cache.parsed_json,
                MIN(jobs.created_at)
             FROM print_jobs AS jobs
             JOIN parse_cache AS cache ON cache.source_hash = jobs.source_hash
             GROUP BY jobs.source_hash, cache.source_file_name, cache.parsed_json
             ORDER BY jobs.source_hash",
        )?;
        let groups = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        groups
    };

    for (source_hash, source_file_name, parsed_json, imported_at) in legacy_groups {
        let project_id = uuid::Uuid::new_v4().to_string();
        let plate_id = uuid::Uuid::new_v4().to_string();
        transaction.execute(
            "INSERT INTO print_projects (
                project_id,
                source_hash,
                source_file_name,
                imported_at,
                plate_count
             ) VALUES (?1, ?2, ?3, ?4, 1)",
            params![project_id, source_hash, source_file_name, imported_at],
        )?;
        transaction.execute(
            "INSERT INTO print_plates (
                plate_id,
                project_id,
                plate_index,
                max_layer,
                parsed_json
             ) VALUES (?1, ?2, 1, ?3, ?4)",
            params![
                plate_id,
                project_id,
                parsed_max_layer(&parsed_json),
                parsed_json
            ],
        )?;
        transaction.execute(
            "UPDATE print_jobs SET plate_id = ?1 WHERE source_hash = ?2",
            params![plate_id, source_hash],
        )?;
    }

    transaction.commit()?;
    Ok(())
}

fn parsed_max_layer(parsed_json: &str) -> u64 {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(parsed_json) else {
        return 0;
    };
    value
        .pointer("/gcode/max_layer")
        .or_else(|| value.pointer("/plates/0/gcode/max_layer"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

fn ledger_supports_creation(connection: &Connection) -> Result<bool> {
    let definition: String = connection.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'ledger_events'",
        [],
        |row| row.get(0),
    )?;
    Ok(definition.contains("'creation'"))
}

#[cfg(test)]
mod tests {
    use super::{
        column_exists, table_exists, AppDatabase, INITIAL_MIGRATION, PRINT_JOBS_MIGRATION,
        REPEAT_JOBS_MIGRATION,
    };
    use crate::{domain::SpoolStatus, inventory::InventoryService};
    use rusqlite::{Connection, OptionalExtension};

    const CATALOG_COLUMNS: [&str; 5] = [
        "catalog_id",
        "color_name",
        "color_code",
        "color_hexes",
        "preset_base",
    ];

    fn legacy_connection_with_catalog_sql(sql: &str) -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(INITIAL_MIGRATION).unwrap();
        connection.execute_batch(sql).unwrap();
        connection
    }

    fn assert_all_catalog_columns_exist(connection: &Connection) {
        for column in CATALOG_COLUMNS {
            assert!(
                column_exists(connection, "spools", column).unwrap(),
                "missing catalog column {column}"
            );
        }
    }

    #[test]
    fn migration_creates_inventory_tables() {
        let database = AppDatabase::open_in_memory().unwrap();

        for table in [
            "spools",
            "ams_slots",
            "print_jobs",
            "job_consumption",
            "ledger_events",
            "app_settings",
        ] {
            assert!(database.table_exists(table).unwrap(), "missing {table}");
        }
    }

    #[test]
    fn catalog_migration_adds_nullable_spool_metadata() {
        let database = AppDatabase::open_in_memory().unwrap();
        assert_all_catalog_columns_exist(&database.connection);
    }

    #[test]
    fn catalog_migration_repairs_schema_with_only_catalog_id() {
        let connection =
            legacy_connection_with_catalog_sql("ALTER TABLE spools ADD COLUMN catalog_id TEXT;");

        let database = AppDatabase::from_connection(connection).unwrap();

        assert_all_catalog_columns_exist(&database.connection);
    }

    #[test]
    fn catalog_migration_repairs_schema_with_color_name_but_no_catalog_id() {
        let connection =
            legacy_connection_with_catalog_sql("ALTER TABLE spools ADD COLUMN color_name TEXT;");

        let database = AppDatabase::from_connection(connection).unwrap();

        assert_all_catalog_columns_exist(&database.connection);
    }

    #[test]
    fn catalog_migration_preserves_existing_spool_in_a_partial_schema() {
        let connection = legacy_connection_with_catalog_sql(
            "
            ALTER TABLE spools ADD COLUMN catalog_id TEXT;
            ALTER TABLE spools ADD COLUMN color_name TEXT;
            INSERT INTO spools (
                spool_id,
                display_name,
                catalog_id,
                color_name,
                brand,
                material,
                series,
                color_hex,
                remaining_grams,
                status
            ) VALUES (
                '11111111-1111-4111-8111-111111111111',
                'Existing catalog spool',
                'bambu:GFA00:10100',
                'Jade White',
                'Bambu Lab',
                'PLA',
                'Basic',
                '#FFFFFF',
                812.5,
                'available'
            );
            ",
        );

        let database = AppDatabase::from_connection(connection).unwrap();

        assert_all_catalog_columns_exist(&database.connection);
        let spool = database
            .connection
            .query_row(
                "SELECT display_name, catalog_id, color_name, brand, material, series, color_hex, remaining_grams, status
                 FROM spools
                 WHERE spool_id = '11111111-1111-4111-8111-111111111111'",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, f64>(7)?,
                        row.get::<_, String>(8)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            spool,
            (
                "Existing catalog spool".to_owned(),
                Some("bambu:GFA00:10100".to_owned()),
                Some("Jade White".to_owned()),
                "Bambu Lab".to_owned(),
                "PLA".to_owned(),
                "Basic".to_owned(),
                "#FFFFFF".to_owned(),
                812.5,
                "available".to_owned(),
            )
        );
    }

    #[test]
    fn complete_catalog_schema_can_be_initialized_again_without_side_effects() {
        let database = AppDatabase::open_in_memory().unwrap();
        database
            .connection
            .execute(
                "INSERT INTO spools (
                    spool_id,
                    display_name,
                    preset_id,
                    catalog_id,
                    color_name,
                    color_code,
                    color_hexes,
                    preset_base,
                    brand,
                    material,
                    series,
                    color_hex,
                    remaining_grams,
                    status
                ) VALUES (
                    '11111111-1111-4111-8111-111111111111',
                    'Complete catalog spool',
                    'Bambu PLA Basic @BBL A1',
                    'bambu:GFA00:10100',
                    'Jade White',
                    '10100',
                    '[\"#FFFFFF\"]',
                    'Bambu PLA Basic',
                    'Bambu Lab',
                    'PLA',
                    'Basic',
                    '#FFFFFF',
                    812.5,
                    'available'
                )",
                [],
            )
            .unwrap();
        let before_schema: Vec<(String, String)> = database
            .connection
            .prepare("SELECT name, type FROM pragma_table_info('spools') ORDER BY cid")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();

        let reopened = AppDatabase::from_connection(database.connection).unwrap();

        assert_all_catalog_columns_exist(&reopened.connection);
        let after_schema: Vec<(String, String)> = reopened
            .connection
            .prepare("SELECT name, type FROM pragma_table_info('spools') ORDER BY cid")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(after_schema, before_schema);
        assert_eq!(
            reopened
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM spools
                     WHERE spool_id = '11111111-1111-4111-8111-111111111111'
                       AND catalog_id = 'bambu:GFA00:10100'
                       AND color_name = 'Jade White'
                       AND color_code = '10100'
                       AND color_hexes = '[\"#FFFFFF\"]'
                       AND preset_base = 'Bambu PLA Basic'",
                    [],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn catalog_migration_rolls_back_all_columns_when_a_later_statement_fails() {
        let path = std::env::temp_dir().join(format!(
            "bambu-pools-catalog-rollback-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        {
            let connection = Connection::open(&path).unwrap();
            connection.execute_batch(INITIAL_MIGRATION).unwrap();
            connection
                .execute("CREATE TABLE idx_spools_catalog (marker TEXT)", [])
                .unwrap();
        }

        assert!(AppDatabase::open(&path).is_err());

        let connection = Connection::open(&path).unwrap();
        for column in CATALOG_COLUMNS {
            assert!(
                !column_exists(&connection, "spools", column).unwrap(),
                "catalog migration left {column} behind after rollback"
            );
        }
        drop(connection);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn migration_initializes_exactly_four_ams_slots() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut statement = database
            .connection
            .prepare("SELECT slot_number FROM ams_slots ORDER BY slot_number")
            .unwrap();
        let slots = statement
            .query_map([], |row| row.get::<_, u8>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(slots, vec![1, 2, 3, 4]);
    }

    #[test]
    fn repeat_job_schema_can_be_reopened_after_migration() {
        let path = std::env::temp_dir().join(format!(
            "bambu-pools-reopen-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        drop(AppDatabase::open(&path).unwrap());

        let reopened = AppDatabase::open(&path).unwrap();

        assert!(reopened.table_exists("parse_cache").unwrap());
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn print_job_upgrade_preserves_existing_inventory_jobs_consumption_and_ledger() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(INITIAL_MIGRATION).unwrap();
        connection.execute_batch(PRINT_JOBS_MIGRATION).unwrap();
        connection.execute(
            "INSERT INTO spools (spool_id, display_name, brand, material, series, color_hex, remaining_grams, status) VALUES ('11111111-1111-4111-8111-111111111111', 'PLA', 'Bambu Lab', 'PLA', 'Basic', '#ffffff', 990.0, 'available')",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO print_jobs (job_id, source_hash, source_file_name, outcome, settlement_version) VALUES ('22222222-2222-4222-8222-222222222222', 'old-hash', 'old.gcode.3mf', '{\"kind\":\"success\"}', 1)",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO job_imports (job_id, parsed_json, parse_count) VALUES ('22222222-2222-4222-8222-222222222222', '{\"filaments\":[],\"gcode\":{\"layers\":[],\"totals_mm\":{},\"max_layer\":0}}', 1)",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO job_consumption (job_id, spool_id, settlement_version, consumed_grams, confidence) VALUES ('22222222-2222-4222-8222-222222222222', '11111111-1111-4111-8111-111111111111', 1, 10.0, 'exact')",
            [],
        ).unwrap();
        connection.execute(
            "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, job_id, settlement_version, event_type, delta_grams, confidence) VALUES ('33333333-3333-4333-8333-333333333333', 'old-settlement', '11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222', 1, 'settlement', -10.0, 'exact')",
            [],
        ).unwrap();

        let database = AppDatabase::from_connection(connection).unwrap();

        assert_eq!(
            database.connection.query_row(
                "SELECT COUNT(*) FROM spools WHERE spool_id = '11111111-1111-4111-8111-111111111111'",
                [], |row| row.get::<_, u32>(0),
            ).unwrap(),
            1
        );
        assert_eq!(
            database.connection.query_row(
                "SELECT COUNT(*) FROM print_jobs WHERE job_id = '22222222-2222-4222-8222-222222222222' AND source_hash = 'old-hash'",
                [], |row| row.get::<_, u32>(0),
            ).unwrap(),
            1
        );
        assert_eq!(
            database.connection.query_row(
                "SELECT consumed_grams FROM job_consumption WHERE job_id = '22222222-2222-4222-8222-222222222222'",
                [], |row| row.get::<_, f64>(0),
            ).unwrap(),
            10.0
        );
        assert_eq!(
            database.connection.query_row(
                "SELECT delta_grams FROM ledger_events WHERE event_id = '33333333-3333-4333-8333-333333333333'",
                [], |row| row.get::<_, f64>(0),
            ).unwrap(),
            -10.0
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM parse_cache WHERE source_hash = 'old-hash'",
                    [],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row("PRAGMA foreign_key_check", [], |_| Ok(1))
                .optional()
                .unwrap(),
            None
        );
    }

    #[test]
    fn print_history_migration_backfills_legacy_jobs_without_touching_ledger() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(INITIAL_MIGRATION).unwrap();
        connection.execute_batch(PRINT_JOBS_MIGRATION).unwrap();
        connection.execute_batch(REPEAT_JOBS_MIGRATION).unwrap();
        connection
            .execute(
                "INSERT INTO spools (
                spool_id,
                display_name,
                preset_id,
                brand,
                material,
                series,
                color_hex,
                remaining_grams,
                status
            ) VALUES (
                '11111111-1111-4111-8111-111111111111',
                'Legacy PLA',
                'Bambu PLA Basic @BBL A1',
                'Bambu Lab',
                'PLA',
                'Basic',
                '#ffffff',
                990.0,
                'assigned'
            )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE ams_slots
                 SET spool_id = '11111111-1111-4111-8111-111111111111',
                     assigned_at = CURRENT_TIMESTAMP
                 WHERE slot_number = 1",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO parse_cache (
                source_hash,
                source_file_name,
                parsed_json,
                parse_count
            ) VALUES (
                'legacy-source-hash',
                'legacy.gcode.3mf',
                '{\"filaments\":[],\"gcode\":{\"layers\":[],\"totals_mm\":{},\"max_layer\":0}}',
                1
            )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO print_jobs (
                job_id,
                source_hash,
                source_file_name,
                outcome,
                settlement_version
            ) VALUES (
                '22222222-2222-4222-8222-222222222222',
                'legacy-source-hash',
                'legacy.gcode.3mf',
                '{\"kind\":\"success\"}',
                1
            )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO job_mappings (job_id, tool, spool_id, slot_number)
             VALUES (
                 '22222222-2222-4222-8222-222222222222',
                 0,
                 '11111111-1111-4111-8111-111111111111',
                 1
             )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO job_consumption (
                job_id,
                spool_id,
                settlement_version,
                consumed_grams,
                confidence,
                slot_number
            ) VALUES (
                '22222222-2222-4222-8222-222222222222',
                '11111111-1111-4111-8111-111111111111',
                1,
                10.0,
                'exact',
                1
            )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO ledger_events (
                event_id,
                idempotency_key,
                spool_id,
                event_type,
                delta_grams,
                confidence
            ) VALUES (
                '33333333-3333-4333-8333-333333333333',
                'legacy-creation',
                '11111111-1111-4111-8111-111111111111',
                'creation',
                1000.0,
                'exact'
            )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO ledger_events (
                event_id,
                idempotency_key,
                spool_id,
                job_id,
                settlement_version,
                event_type,
                delta_grams,
                confidence
            ) VALUES (
                '44444444-4444-4444-8444-444444444444',
                'legacy-settlement',
                '11111111-1111-4111-8111-111111111111',
                '22222222-2222-4222-8222-222222222222',
                1,
                'settlement',
                -10.0,
                'exact'
            )",
                [],
            )
            .unwrap();

        let database = AppDatabase::from_connection(connection).unwrap();

        let plate_id: String = database
            .connection
            .query_row(
                "SELECT plate_id FROM print_jobs
                 WHERE job_id = '22222222-2222-4222-8222-222222222222'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!plate_id.is_empty());
        let migrated_plate = database
            .connection
            .query_row(
                "SELECT projects.source_hash, plates.plate_index, plates.max_layer, plates.parsed_json
                 FROM print_plates AS plates
                 JOIN print_projects AS projects USING (project_id)
                 WHERE plates.plate_id = ?1",
                [&plate_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, u32>(1)?,
                        row.get::<_, u32>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            migrated_plate,
            (
                "legacy-source-hash".to_owned(),
                1,
                0,
                "{\"filaments\":[],\"gcode\":{\"layers\":[],\"totals_mm\":{},\"max_layer\":0}}"
                    .to_owned(),
            )
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT remaining_grams FROM spools
                     WHERE spool_id = '11111111-1111-4111-8111-111111111111'",
                    [],
                    |row| row.get::<_, f64>(0),
                )
                .unwrap(),
            990.0
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT spool_id FROM ams_slots WHERE slot_number = 1",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            "11111111-1111-4111-8111-111111111111"
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT COUNT(*) FROM job_mappings
                     WHERE job_id = '22222222-2222-4222-8222-222222222222'
                       AND spool_id = '11111111-1111-4111-8111-111111111111'
                       AND slot_number = 1",
                    [],
                    |row| row.get::<_, u32>(0),
                )
                .unwrap(),
            1
        );
        assert_eq!(
            database
                .connection
                .query_row(
                    "SELECT consumed_grams FROM job_consumption
                     WHERE job_id = '22222222-2222-4222-8222-222222222222'
                       AND spool_id = '11111111-1111-4111-8111-111111111111'
                       AND settlement_version = 1",
                    [],
                    |row| row.get::<_, f64>(0),
                )
                .unwrap(),
            10.0
        );
        assert_eq!(ledger_count(&database.connection), 2);
    }

    fn ledger_count(connection: &Connection) -> u32 {
        connection
            .query_row("SELECT COUNT(*) FROM ledger_events", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn print_history_migration_rolls_back_schema_when_backfill_fails() {
        let path = std::env::temp_dir().join(format!(
            "bambu-pools-print-history-rollback-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        {
            let connection = Connection::open(&path).unwrap();
            create_minimal_legacy_print_history_database(&connection);
            connection
                .execute_batch(
                    "CREATE TRIGGER reject_print_history_backfill
                     BEFORE UPDATE OF plate_id ON print_jobs
                     BEGIN
                         SELECT RAISE(ABORT, 'forced print history backfill failure');
                     END;",
                )
                .unwrap();
        }

        assert!(AppDatabase::open(&path).is_err());

        let connection = Connection::open(&path).unwrap();
        for table in ["media_assets", "print_projects", "print_plates"] {
            assert!(
                !table_exists(&connection, table).unwrap(),
                "history migration left {table} behind after rollback"
            );
        }
        assert!(
            !column_exists(&connection, "print_jobs", "plate_id").unwrap(),
            "history migration left print_jobs.plate_id behind after rollback"
        );
        drop(connection);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn print_history_migration_can_be_reopened_twice_without_duplicate_backfill() {
        let path = std::env::temp_dir().join(format!(
            "bambu-pools-print-history-reopen-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        {
            let connection = Connection::open(&path).unwrap();
            create_minimal_legacy_print_history_database(&connection);
        }

        drop(AppDatabase::open(&path).unwrap());
        for _ in 0..2 {
            let reopened = AppDatabase::open(&path).unwrap();
            let counts = reopened
                .connection
                .query_row(
                    "SELECT
                        (SELECT COUNT(*) FROM print_projects),
                        (SELECT COUNT(*) FROM print_plates),
                        (SELECT COUNT(*) FROM print_jobs WHERE plate_id IS NOT NULL)",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, u32>(0)?,
                            row.get::<_, u32>(1)?,
                            row.get::<_, u32>(2)?,
                        ))
                    },
                )
                .unwrap();
            assert_eq!(counts, (1, 1, 1));
        }

        std::fs::remove_file(path).unwrap();
    }

    fn create_minimal_legacy_print_history_database(connection: &Connection) {
        connection.execute_batch(INITIAL_MIGRATION).unwrap();
        connection.execute_batch(PRINT_JOBS_MIGRATION).unwrap();
        connection.execute_batch(REPEAT_JOBS_MIGRATION).unwrap();
        connection
            .execute(
                "INSERT INTO parse_cache (
                source_hash,
                source_file_name,
                parsed_json,
                parse_count
             ) VALUES (
                'legacy-source-hash',
                'legacy.gcode.3mf',
                '{\"filaments\":[],\"gcode\":{\"layers\":[],\"totals_mm\":{},\"max_layer\":4}}',
                1
             )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO print_jobs (
                job_id,
                source_hash,
                source_file_name,
                outcome,
                settlement_version
             ) VALUES (
                '22222222-2222-4222-8222-222222222222',
                'legacy-source-hash',
                'legacy.gcode.3mf',
                '{\"kind\":\"success\"}',
                1
             )",
                [],
            )
            .unwrap();
    }

    #[test]
    fn migration_rejects_negative_spool_weight_and_missing_slot_spool() {
        let database = AppDatabase::open_in_memory().unwrap();

        assert!(database
            .connection
            .execute(
                "INSERT INTO spools (spool_id, display_name, brand, material, series, color_hex, remaining_grams, status) VALUES ('spool-1', 'PLA', 'Bambu Lab', 'PLA', 'Basic', '#ffffff', -1.0, 'available')",
                [],
            )
            .is_err());
        assert!(database
            .connection
            .execute(
                "UPDATE ams_slots SET spool_id = 'missing' WHERE slot_number = 1",
                []
            )
            .is_err());
    }

    #[test]
    fn migration_prevents_deleting_ledger_history() {
        let database = AppDatabase::open_in_memory().unwrap();
        database
            .connection
            .execute(
                "INSERT INTO spools (spool_id, display_name, brand, material, series, color_hex, remaining_grams, status) VALUES ('spool-1', 'PLA', 'Bambu Lab', 'PLA', 'Basic', '#ffffff', 1000.0, 'available')",
                [],
            )
            .unwrap();
        database
            .connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence) VALUES ('event-1', 'ledger-event-1', 'spool-1', 'adjustment', -10.0, 'exact')",
                [],
            )
            .unwrap();

        assert!(database
            .connection
            .execute("DELETE FROM ledger_events WHERE event_id = 'event-1'", [])
            .is_err());
        let remaining = database
            .connection
            .query_row(
                "SELECT COUNT(*) FROM ledger_events WHERE event_id = 'event-1'",
                [],
                |row| row.get::<_, u8>(0),
            )
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[test]
    fn migration_prevents_rewriting_ledger_history() {
        let database = AppDatabase::open_in_memory().unwrap();
        database
            .connection
            .execute(
                "INSERT INTO spools (spool_id, display_name, brand, material, series, color_hex, remaining_grams, status) VALUES ('spool-1', 'PLA', 'Bambu Lab', 'PLA', 'Basic', '#ffffff', 1000.0, 'available')",
                [],
            )
            .unwrap();
        database
            .connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence) VALUES ('event-1', 'ledger-event-1', 'spool-1', 'adjustment', -10.0, 'exact')",
                [],
            )
            .unwrap();

        assert!(database
            .connection
            .execute(
                "UPDATE ledger_events SET delta_grams = -20.0 WHERE event_id = 'event-1'",
                []
            )
            .is_err());
    }

    #[test]
    fn migration_requires_reversals_to_reference_prior_events() {
        let database = AppDatabase::open_in_memory().unwrap();
        database
            .connection
            .execute(
                "INSERT INTO spools (spool_id, display_name, brand, material, series, color_hex, remaining_grams, status) VALUES ('spool-1', 'PLA', 'Bambu Lab', 'PLA', 'Basic', '#ffffff', 1000.0, 'available')",
                [],
            )
            .unwrap();
        database
            .connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence) VALUES ('settlement-1', 'settlement-key-1', 'spool-1', 'settlement', -10.0, 'exact')",
                [],
            )
            .unwrap();

        assert!(database
            .connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence) VALUES ('invalid-reversal', 'invalid-reversal-key', 'spool-1', 'reversal', 10.0, 'exact')",
                [],
            )
            .is_err());
        assert!(database
            .connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence, reverses_event_id) VALUES ('missing-reversal', 'missing-reversal-key', 'spool-1', 'reversal', 10.0, 'exact', 'missing-event')",
                [],
            )
            .is_err());
        assert!(database
            .connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence, reverses_event_id) VALUES ('reversal-1', 'reversal-key-1', 'spool-1', 'reversal', 10.0, 'exact', 'settlement-1')",
                [],
            )
            .is_ok());
        assert!(database
            .connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence) VALUES ('adjustment-1', 'adjustment-key-1', 'spool-1', 'adjustment', -1.0, 'exact')",
                [],
            )
            .is_ok());
    }

    #[test]
    fn migration_backfills_creation_baselines_for_existing_spools() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE spools (
                    spool_id TEXT PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    brand TEXT NOT NULL,
                    material TEXT NOT NULL,
                    series TEXT NOT NULL,
                    color_hex TEXT NOT NULL,
                    remaining_grams REAL NOT NULL CHECK (remaining_grams >= 0),
                    status TEXT NOT NULL CHECK (status IN ('available', 'assigned', 'empty', 'archived'))
                );
                CREATE TABLE print_jobs (
                    job_id TEXT PRIMARY KEY,
                    source_hash TEXT NOT NULL UNIQUE,
                    source_file_name TEXT NOT NULL,
                    outcome TEXT,
                    settlement_version INTEGER NOT NULL DEFAULT 0 CHECK (settlement_version >= 0),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE (job_id, settlement_version)
                );
                CREATE TABLE ledger_events (
                    event_id TEXT PRIMARY KEY,
                    idempotency_key TEXT NOT NULL UNIQUE,
                    spool_id TEXT NOT NULL REFERENCES spools(spool_id) ON DELETE RESTRICT,
                    job_id TEXT REFERENCES print_jobs(job_id) ON DELETE RESTRICT,
                    settlement_version INTEGER CHECK (settlement_version IS NULL OR settlement_version >= 0),
                    event_type TEXT NOT NULL CHECK (event_type IN ('settlement', 'reversal', 'adjustment')),
                    delta_grams REAL NOT NULL CHECK (delta_grams <> 0),
                    confidence TEXT NOT NULL CHECK (confidence IN ('exact', 'estimated', 'needs_confirmation')),
                    reverses_event_id TEXT UNIQUE REFERENCES ledger_events(event_id) ON DELETE RESTRICT,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE (job_id, spool_id, settlement_version, event_type)
                );
                ",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO spools (spool_id, display_name, brand, material, series, color_hex, remaining_grams, status) VALUES ('11111111-1111-4111-8111-111111111111', 'PLA', 'Bambu Lab', 'PLA', 'Basic', '#ffffff', 800.0, 'available')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO spools (spool_id, display_name, brand, material, series, color_hex, remaining_grams, status) VALUES ('22222222-2222-4222-8222-222222222222', 'PLA', 'Bambu Lab', 'PLA', 'Basic', '#ffffff', 0.0, 'empty')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO ledger_events (event_id, idempotency_key, spool_id, event_type, delta_grams, confidence) VALUES ('prior-adjustment', 'prior-adjustment-key', '11111111-1111-4111-8111-111111111111', 'adjustment', -125.0, 'exact')",
                [],
            )
            .unwrap();
        let database = AppDatabase::from_connection(connection).unwrap();
        let available_total: f64 = database
            .connection
            .query_row(
                "SELECT COALESCE(SUM(delta_grams), 0.0) FROM ledger_events WHERE spool_id = '11111111-1111-4111-8111-111111111111'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let empty_total: f64 = database
            .connection
            .query_row(
                "SELECT COALESCE(SUM(delta_grams), 0.0) FROM ledger_events WHERE spool_id = '22222222-2222-4222-8222-222222222222'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let available_creation: f64 = database
            .connection
            .query_row(
                "SELECT delta_grams FROM ledger_events WHERE spool_id = '11111111-1111-4111-8111-111111111111' AND event_type = 'creation'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let empty_creation: f64 = database
            .connection
            .query_row(
                "SELECT delta_grams FROM ledger_events WHERE spool_id = '22222222-2222-4222-8222-222222222222' AND event_type = 'creation'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(available_creation, 925.0);
        assert_eq!(empty_creation, 0.0);
        assert_eq!(available_total, 800.0);
        assert_eq!(empty_total, 0.0);

        let mut service = InventoryService::new(database);
        let available = "11111111-1111-4111-8111-111111111111".parse().unwrap();
        let empty = "22222222-2222-4222-8222-222222222222".parse().unwrap();
        assert_eq!(service.rebuild_spool_balance(available).unwrap(), 800.0);
        assert_eq!(
            service.get_spool(available).unwrap().status,
            SpoolStatus::Available
        );
        assert_eq!(service.rebuild_spool_balance(empty).unwrap(), 0.0);
        assert_eq!(service.get_spool(empty).unwrap().status, SpoolStatus::Empty);
    }
}
