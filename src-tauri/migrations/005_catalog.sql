ALTER TABLE spools ADD COLUMN catalog_id TEXT;
ALTER TABLE spools ADD COLUMN color_name TEXT;
ALTER TABLE spools ADD COLUMN color_code TEXT;
ALTER TABLE spools ADD COLUMN color_hexes TEXT;
ALTER TABLE spools ADD COLUMN preset_base TEXT;
CREATE INDEX IF NOT EXISTS idx_spools_catalog ON spools(catalog_id);
CREATE INDEX IF NOT EXISTS idx_spools_preset_base
    ON spools(preset_base, material, color_hex);
