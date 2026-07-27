use crate::error::Result;
use rusqlite::{params, Connection};
use std::path::Path;

const INITIAL_MIGRATION: &str = include_str!("../migrations/001_init.sql");

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

    fn from_connection(connection: Connection) -> Result<Self> {
        connection.execute_batch(INITIAL_MIGRATION)?;
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

#[cfg(test)]
mod tests {
    use super::AppDatabase;

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
}
