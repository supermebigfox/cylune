-- Column additions are selected from a static allowlist in db.rs so interrupted
-- versions of this migration can be repaired one missing column at a time.
CREATE INDEX IF NOT EXISTS idx_spools_catalog ON spools(catalog_id);
CREATE INDEX IF NOT EXISTS idx_spools_preset_base
    ON spools(preset_base, material, color_hex);
