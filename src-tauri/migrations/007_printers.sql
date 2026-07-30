CREATE TABLE IF NOT EXISTS printers (
    printer_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) BETWEEN 1 AND 80),
    model_key TEXT NOT NULL CHECK (length(trim(model_key)) BETWEEN 1 AND 160),
    nozzle_diameter REAL NOT NULL CHECK (nozzle_diameter > 0 AND nozzle_diameter <= 2),
    default_plate TEXT NOT NULL CHECK (length(trim(default_plate)) BETWEEN 1 AND 120),
    ams_kind TEXT NOT NULL CHECK (length(trim(ams_kind)) BETWEEN 1 AND 80),
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS one_default_printer
ON printers(is_default) WHERE is_default = 1;

CREATE INDEX IF NOT EXISTS idx_printers_model
ON printers(model_key, nozzle_diameter);
