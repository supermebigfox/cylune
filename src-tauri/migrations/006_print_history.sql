CREATE TABLE media_assets (
    asset_id TEXT PRIMARY KEY,
    storage_path TEXT NOT NULL UNIQUE,
    mime_type TEXT NOT NULL,
    width INTEGER CHECK (width IS NULL OR width > 0),
    height INTEGER CHECK (height IS NULL OR height > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE print_projects (
    project_id TEXT PRIMARY KEY,
    source_hash TEXT NOT NULL,
    source_file_name TEXT NOT NULL,
    source_path TEXT,
    imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    plate_count INTEGER NOT NULL CHECK (plate_count > 0),
    cover_asset_id TEXT REFERENCES media_assets(asset_id) ON DELETE SET NULL
);

CREATE TABLE print_plates (
    plate_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES print_projects(project_id) ON DELETE RESTRICT,
    plate_index INTEGER NOT NULL CHECK (plate_index > 0),
    display_name TEXT,
    thumbnail_asset_id TEXT REFERENCES media_assets(asset_id) ON DELETE SET NULL,
    estimated_seconds INTEGER CHECK (estimated_seconds IS NULL OR estimated_seconds >= 0),
    max_layer INTEGER NOT NULL CHECK (max_layer >= 0),
    parsed_json TEXT NOT NULL,
    UNIQUE(project_id, plate_index)
);

ALTER TABLE print_jobs
ADD COLUMN plate_id TEXT REFERENCES print_plates(plate_id) ON DELETE RESTRICT;

CREATE INDEX print_projects_imported_at
ON print_projects(imported_at DESC, project_id);

CREATE INDEX print_plates_project_id_plate_index
ON print_plates(project_id, plate_index);

CREATE INDEX print_jobs_plate_id
ON print_jobs(plate_id, created_at, job_id);
