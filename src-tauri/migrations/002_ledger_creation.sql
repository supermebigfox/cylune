PRAGMA foreign_keys = OFF;

DROP TRIGGER IF EXISTS prevent_ledger_event_delete;
DROP TRIGGER IF EXISTS prevent_ledger_event_update;
DROP TRIGGER IF EXISTS require_ledger_reversal_reference;
DROP TRIGGER IF EXISTS prevent_non_reversal_reference;

BEGIN;

ALTER TABLE ledger_events RENAME TO ledger_events_legacy;

CREATE TABLE ledger_events (
    event_id TEXT PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    spool_id TEXT NOT NULL REFERENCES spools(spool_id) ON DELETE RESTRICT,
    job_id TEXT REFERENCES print_jobs(job_id) ON DELETE RESTRICT,
    settlement_version INTEGER CHECK (settlement_version IS NULL OR settlement_version >= 0),
    event_type TEXT NOT NULL CHECK (event_type IN ('creation', 'settlement', 'reversal', 'adjustment')),
    delta_grams REAL NOT NULL CHECK (delta_grams <> 0 OR event_type = 'creation'),
    confidence TEXT NOT NULL CHECK (confidence IN ('exact', 'estimated', 'needs_confirmation')),
    reverses_event_id TEXT UNIQUE REFERENCES ledger_events(event_id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (job_id, spool_id, settlement_version, event_type)
);

INSERT INTO ledger_events (
    event_id,
    idempotency_key,
    spool_id,
    job_id,
    settlement_version,
    event_type,
    delta_grams,
    confidence,
    reverses_event_id,
    created_at
)
SELECT
    event_id,
    idempotency_key,
    spool_id,
    job_id,
    settlement_version,
    event_type,
    delta_grams,
    confidence,
    reverses_event_id,
    created_at
FROM ledger_events_legacy;

DROP TABLE ledger_events_legacy;

COMMIT;

PRAGMA foreign_keys = ON;

CREATE TRIGGER prevent_ledger_event_delete
BEFORE DELETE ON ledger_events
BEGIN
    SELECT RAISE(ABORT, 'ledger events are immutable');
END;

CREATE TRIGGER prevent_ledger_event_update
BEFORE UPDATE ON ledger_events
BEGIN
    SELECT RAISE(ABORT, 'ledger events are immutable');
END;

CREATE TRIGGER require_ledger_reversal_reference
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

CREATE TRIGGER prevent_non_reversal_reference
BEFORE INSERT ON ledger_events
WHEN NEW.event_type <> 'reversal' AND NEW.reverses_event_id IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'only reversal events may reference another event');
END;
