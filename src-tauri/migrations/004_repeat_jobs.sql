PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE parse_cache (
    source_hash TEXT PRIMARY KEY,
    source_file_name TEXT NOT NULL,
    parsed_json TEXT NOT NULL,
    parse_count INTEGER NOT NULL DEFAULT 1 CHECK (parse_count = 1),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO parse_cache (source_hash, source_file_name, parsed_json, parse_count)
SELECT print_jobs.source_hash, print_jobs.source_file_name, job_imports.parsed_json, 1
FROM print_jobs
JOIN job_imports USING (job_id)
GROUP BY print_jobs.source_hash;

DROP TABLE job_imports;

CREATE TABLE print_jobs_new (
    job_id TEXT PRIMARY KEY,
    source_hash TEXT NOT NULL,
    source_file_name TEXT NOT NULL,
    outcome TEXT,
    settlement_version INTEGER NOT NULL DEFAULT 0 CHECK (settlement_version >= 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (job_id, settlement_version)
);

INSERT INTO print_jobs_new (
    job_id,
    source_hash,
    source_file_name,
    outcome,
    settlement_version,
    created_at
)
SELECT
    job_id,
    source_hash,
    source_file_name,
    outcome,
    settlement_version,
    created_at
FROM print_jobs;

DROP TABLE print_jobs;
ALTER TABLE print_jobs_new RENAME TO print_jobs;

CREATE INDEX print_jobs_source_hash_created_at
ON print_jobs (source_hash, created_at, job_id);

COMMIT;

PRAGMA foreign_keys = ON;
