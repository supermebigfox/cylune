# Official Color Identity and Plate Retry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Match sliced display colors to all official Bambu catalog identities, omit zero-use tools, and create an idempotent single-plate retry after failed or cancelled settlement.

**Architecture:** Rust owns catalog-aware color resolution, preview filtering, retry persistence, and idempotency. The existing Tauri API exposes one `retry_print_job` command; React asks after a successful failed/cancelled settlement and opens the returned one-plate project. The checked-in Bambu catalog remains the source of official color truth.

**Tech Stack:** Rust, rusqlite, serde, Tauri 2, React 18, TypeScript, Vitest, Testing Library, SQLite migrations.

---

### Task 1: Resolve sliced colors to official catalog identities

**Files:**
- Modify: `src-tauri/src/imports.rs`
- Test: `src-tauri/src/imports.rs`

- [ ] **Step 1: Write failing tests**

Add tests proving that a mounted PLA Basic 10400 spool matches `#FFFF00`, that the known white/red/blue/yellow sliced aliases select their corresponding `catalog_id`, and that a color from another preset base is never selected.

```rust
#[test]
fn sliced_display_yellow_resolves_to_official_pla_basic_10400() {
    // Create a mounted catalog spool with catalog_id bambu:GFA00:10400
    // and official color #F4EE2A, then match a Bambu PLA Basic profile
    // whose display color is #FFFF00.
    assert_eq!(service.matching_spools(&profile).unwrap(), vec![yellow]);
}
```

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test sliced_display_yellow_resolves_to_official_pla_basic_10400 -- --nocapture
```

Expected: FAIL because matching does not yet resolve the project color to a catalog identity.

- [ ] **Step 3: Implement catalog identity resolution**

Parse `../../src/catalog/bambu.json` once through a small internal catalog representation. For a profile, filter entries by normalized `presetBase`; find the nearest catalog color using all entry colors; accept only a unique result inside the existing safety threshold; then query mounted, non-archived spools by `catalog_id`. Keep exact preset/base matching ahead of this resolver and legacy nearest-color matching behind it.

- [ ] **Step 4: Verify GREEN and full catalog integrity**

Run:

```bash
cargo test sliced_display_yellow_resolves_to_official_pla_basic_10400 -- --nocapture
cargo test every_official_catalog_color_matches_its_loaded_physical_spool -- --nocapture
```

Expected: PASS.

### Task 2: Remove zero-use tools from every plate preview

**Files:**
- Modify: `src-tauri/src/imports.rs`
- Modify: `src-tauri/src/history/mod.rs`
- Test: `src-tauri/src/imports.rs`
- Test: `src-tauri/src/history/mod.rs`

- [ ] **Step 1: Write failing tests**

Create a parsed plate with tool 0 positive usage and tool 1 missing from `totals_mm`. Assert import preview and history detail contain only tool 0. Add a second test asserting a plate with no positive-use tool is rejected.

```rust
assert_eq!(preview.plates[0].filaments.len(), 1);
assert_eq!(preview.plates[0].filaments[0].tool, 0);
```

- [ ] **Step 2: Verify RED**

Run focused imports and history tests. Expected: FAIL because both paths currently clone every configured filament.

- [ ] **Step 3: Implement one shared positive-use rule**

Use a finite positive threshold of `1e-9` millimetres when building previews and history summaries. Do not rewrite parsed G-code or source 3MF settings. Reject a plate that has no usable filament.

- [ ] **Step 4: Verify GREEN**

Run the focused tests plus all parser/import/history tests. Expected: PASS.

### Task 3: Persist an idempotent single-plate retry

**Files:**
- Create: `src-tauri/migrations/008_plate_retry.sql`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/imports.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db.rs`
- Test: `src-tauri/src/imports.rs`

- [ ] **Step 1: Write migration and service failure tests**

Tests must require `print_jobs.retry_of_job_id`, a unique non-null retry source, and `PrintService::retry_print_job(source_job_id)` behavior:

```rust
let retry = service.retry_print_job(failed_job).unwrap();
assert_eq!(retry.plates.len(), 1);
assert_ne!(retry.project_id, original.project_id);
assert_eq!(service.retry_print_job(failed_job).unwrap().project_id, retry.project_id);
```

Also assert rejection for pending, successful, estimated, skipped, or reversed-source jobs; inherited mappings; no copied settlement/consumption/ledger rows; and selection of the original plate index from cached parsed data.

- [ ] **Step 2: Verify RED**

Run the focused DB and imports tests. Expected: FAIL because the column, migration, and method do not exist.

- [ ] **Step 3: Implement migration and transaction**

Add nullable `retry_of_job_id TEXT REFERENCES print_jobs(job_id) ON DELETE RESTRICT` and a unique partial index for non-null values. In one transaction, validate the source outcome is failed/cancelled and not reversed, return an existing retry if present, load the source plate from `parse_cache`, create a new one-plate project/plate/job, set `retry_of_job_id`, and copy `job_mappings`. Reuse the persisted media asset; do not copy settlement tables.

- [ ] **Step 4: Expose the command**

Add `retry_print_job(job_id: Uuid) -> ImportProjectPreview` to Tauri command registration.

- [ ] **Step 5: Verify GREEN**

Run focused tests and all Rust tests. Expected: PASS.

### Task 4: Ask after failed or cancelled settlement

**Files:**
- Modify: `src/lib/tauri.ts`
- Modify: `src/App.tsx`
- Modify: `src/i18n.ts`
- Modify: `src/App.test.tsx`
- Modify: `src/lib/tauri.test.ts`

- [ ] **Step 1: Write failing frontend tests**

Assert `retryPrintJob(jobId)` invokes `retry_print_job` with camelCase payload. In the app test, settle with `failed` or `cancelled`, accept the confirmation, and assert the returned project opens. Assert success and estimated settlements do not ask; rejecting the confirmation creates no retry.

- [ ] **Step 2: Verify RED**

Run:

```bash
npm test -- --run src/lib/tauri.test.ts src/App.test.tsx
```

Expected: FAIL because the API and post-settlement confirmation do not exist.

- [ ] **Step 3: Implement the minimal interaction**

After `settleJob` succeeds, if `outcome.kind` is `failed` or `cancelled`, ask with localized text. On confirmation call `retryPrintJob(jobId)`, clear stale result state, refresh inventory/history, and open the returned project and its sole plate. On rejection keep the original result visible.

- [ ] **Step 4: Verify GREEN**

Run the focused tests and the complete frontend suite. Expected: PASS.

### Task 5: Refresh official snapshot and run real regression

**Files:**
- Regenerate: `src/catalog/bambu.json`
- Test: `src/catalog/bambu.test.ts`

- [ ] **Step 1: Regenerate from installed official data**

Run:

```bash
node scripts/catalog.mjs "/Applications/BambuStudio.app/Contents/Resources/profiles/BBL/filament/filaments_color_codes.json" "/Applications/BambuStudio.app/Contents/Resources/profiles/BBL/filament"
```

Expected: source version remains `02.08.00.50`; any diff is an official data refresh, not hand-edited aliases.

- [ ] **Step 2: Run catalog, full frontend, and full Rust suites**

Run `npm test -- --run`, `npm run build`, and `cargo test`. Expected: all PASS.

- [ ] **Step 3: Real longbow regression**

Slice `/Users/robin/Desktop/长弓/长弓X 2.3完整版A.3mf` through the real ignored smoke test using P2S. Assert five plates import, each preview contains only positive-use tools, no `0.0` gram mapping appears, and official yellow alias resolution passes independently.

- [ ] **Step 4: Package once**

Run `npm run release:mac`, verify `codesign --verify --deep --strict 发布/CYLUNE.app`, launch only the new app, and confirm one process.
