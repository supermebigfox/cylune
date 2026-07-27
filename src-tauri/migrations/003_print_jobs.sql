BEGIN;

ALTER TABLE spools ADD COLUMN preset_id TEXT;

CREATE TABLE job_imports (
    job_id TEXT PRIMARY KEY REFERENCES print_jobs(job_id) ON DELETE RESTRICT,
    parsed_json TEXT NOT NULL,
    parse_count INTEGER NOT NULL DEFAULT 1 CHECK (parse_count = 1)
);

CREATE TABLE job_mappings (
    job_id TEXT NOT NULL REFERENCES print_jobs(job_id) ON DELETE RESTRICT,
    tool INTEGER NOT NULL CHECK (tool BETWEEN 0 AND 255),
    spool_id TEXT NOT NULL REFERENCES spools(spool_id) ON DELETE RESTRICT,
    slot_number INTEGER CHECK (slot_number BETWEEN 1 AND 4),
    PRIMARY KEY (job_id, tool)
);

ALTER TABLE job_consumption ADD COLUMN slot_number INTEGER CHECK (slot_number BETWEEN 1 AND 4);

COMMIT;
