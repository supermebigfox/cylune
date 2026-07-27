PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS spools (
    spool_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    brand TEXT NOT NULL,
    material TEXT NOT NULL,
    series TEXT NOT NULL,
    color_hex TEXT NOT NULL,
    remaining_grams REAL NOT NULL CHECK (remaining_grams >= 0),
    status TEXT NOT NULL CHECK (status IN ('available', 'assigned', 'empty', 'archived')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS ams_slots (
    slot_number INTEGER PRIMARY KEY CHECK (slot_number BETWEEN 1 AND 4),
    spool_id TEXT UNIQUE REFERENCES spools(spool_id) ON UPDATE CASCADE ON DELETE SET NULL,
    assigned_at TEXT
);

INSERT OR IGNORE INTO ams_slots (slot_number) VALUES (1), (2), (3), (4);

CREATE TABLE IF NOT EXISTS print_jobs (
    job_id TEXT PRIMARY KEY,
    source_hash TEXT NOT NULL UNIQUE,
    source_file_name TEXT NOT NULL,
    outcome TEXT,
    settlement_version INTEGER NOT NULL DEFAULT 0 CHECK (settlement_version >= 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (job_id, settlement_version)
);

CREATE TABLE IF NOT EXISTS job_consumption (
    job_id TEXT NOT NULL REFERENCES print_jobs(job_id) ON DELETE RESTRICT,
    spool_id TEXT NOT NULL REFERENCES spools(spool_id) ON DELETE RESTRICT,
    settlement_version INTEGER NOT NULL CHECK (settlement_version >= 0),
    consumed_grams REAL NOT NULL CHECK (consumed_grams >= 0),
    confidence TEXT NOT NULL CHECK (confidence IN ('exact', 'estimated', 'needs_confirmation')),
    PRIMARY KEY (job_id, spool_id, settlement_version)
);

CREATE TABLE IF NOT EXISTS ledger_events (
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

CREATE TRIGGER IF NOT EXISTS prevent_ledger_event_delete
BEFORE DELETE ON ledger_events
BEGIN
    SELECT RAISE(ABORT, 'ledger events are immutable');
END;

CREATE TRIGGER IF NOT EXISTS prevent_ledger_event_update
BEFORE UPDATE ON ledger_events
BEGIN
    SELECT RAISE(ABORT, 'ledger events are immutable');
END;

CREATE TRIGGER IF NOT EXISTS require_ledger_reversal_reference
BEFORE INSERT ON ledger_events
WHEN NEW.event_type = 'reversal'
    AND (
        NEW.reverses_event_id IS NULL
        OR NOT EXISTS (
            SELECT 1 FROM ledger_events WHERE event_id = NEW.reverses_event_id
        )
    )
BEGIN
    SELECT RAISE(ABORT, 'reversal events must reference an existing event');
END;

CREATE TRIGGER IF NOT EXISTS prevent_non_reversal_reference
BEFORE INSERT ON ledger_events
WHEN NEW.event_type <> 'reversal' AND NEW.reverses_event_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'only reversal events may reference another event');
END;

CREATE TABLE IF NOT EXISTS app_settings (
    setting_key TEXT PRIMARY KEY,
    setting_value TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
