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
}
