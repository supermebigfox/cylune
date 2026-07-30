# Print History and Multi-Plate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add project-grouped print history, multi-plate sliced 3MF parsing, persisted thumbnails, and per-plate settlement without changing existing inventory truth.

**Architecture:** Parse one archive into a versioned `ParsedProjectV2` containing independent plates. Persist each import batch as a project with plates and one job per plate, while keeping parse/media assets content-addressed and reusable. Keep settlement plate-scoped by linking existing jobs to plates; expose history through a dedicated service and focused React components.

**Tech Stack:** Rust, rusqlite, zip, serde, sha2, Tauri 2, React 18, TypeScript, Vitest, Testing Library, SQLite.

## Global Constraints

- Preserve all existing spool balances, AMS assignments, immutable ledger events, mappings, reversals, and backup restores.
- A project is one import/print batch; a plate is one physical plate; a job is one print attempt for one plate.
- Each plate settles independently; no project-level deduction is allowed.
- Before any plate settles, discarding removes the whole batch without inventory mutation; afterward, remaining plates can only be skipped.
- Store extracted images under app data as content-addressed files; never store large image BLOBs in SQLite.
- Do not copy original `.3mf` or `.gcode.3mf` files into history storage.
- Continue computing deducted grams from positive G-code extrusion, including purge and tool-change consumption.
- Support simplified Chinese, traditional Chinese, English, light theme, dark theme, keyboard navigation, and screen-reader labels.
- macOS is the release target; interfaces must not encode macOS-only paths so Windows can be added later.

---

## File Structure

- Create `src-tauri/migrations/006_print_history.sql`: project, plate, media schema and job linkage.
- Create `src-tauri/src/history/mod.rs`: history service, database queries, discard/skip rules, Tauri commands.
- Create `src-tauri/src/media.rs`: archive image validation and content-addressed storage.
- Modify `src-tauri/src/parser/mod.rs`: versioned project and plate types.
- Modify `src-tauri/src/parser/three_mf.rs`: enumerate all plates and correlate metadata.
- Modify `src-tauri/src/parser/gcode.rs`: capture declared duration/layer metadata while preserving extrusion calculation.
- Modify `src-tauri/src/db.rs`: run migration 006 and backfill legacy jobs transactionally.
- Modify `src-tauri/src/imports.rs`: create/reopen project batches and one job per plate.
- Modify `src-tauri/src/settlement.rs`: resolve parsed data through the job's plate.
- Modify `src-tauri/src/backup.rs`: backup schema v3 for projects, plates, media metadata, and legacy restore.
- Modify `src-tauri/src/lib.rs`: manage media root and register history commands.
- Create `src/features/jobs/History.tsx`: pending/history sections and project cards.
- Create `src/features/jobs/Project.tsx`: project and plate detail.
- Modify `src/features/jobs/Job.tsx`: operate on a selected plate preview.
- Modify `src/lib/tauri.ts`: history/project/plate API contracts.
- Modify `src/App.tsx`: navigation state and history loading.
- Modify `src/styles.css` and locale JSON files: visual and localized presentation.

---

### Task 1: Versioned Multi-Plate Parser Contract

**Files:**
- Modify: `src-tauri/src/parser/mod.rs`
- Modify: `src-tauri/src/parser/three_mf.rs`
- Modify: `src-tauri/src/parser/gcode.rs`
- Test: `src-tauri/src/parser/three_mf.rs`
- Test: `src-tauri/src/parser/gcode.rs`

**Interfaces:**
- Produces: `parse_3mf_project(path: &Path) -> Result<ParsedProjectV2>`
- Produces: `ParsedProjectV2 { plates: Vec<ParsedPlate> }`
- Produces: `ParsedPlate { plate_index, display_name, estimated_seconds, thumbnail_entries, filaments, gcode }`
- Preserves: `parse_3mf(path) -> Result<ParsedPrintFile>` as a temporary single-plate compatibility wrapper until Task 4 migrates consumers.

- [ ] **Step 1: Write failing tests for numeric plate ordering and independent reports**

Create a two-plate ZIP in the existing test helper. Plate 1 uses tool 0 and plate 3 uses tool 1. Assert literal ordering and metadata:

```rust
#[test]
fn parses_every_sliced_plate_in_numeric_order() {
    let path = fixture_with_plates(&[(3, 7200, 7), (1, 3600, 3)]);
    let project = parse_3mf_project(&path).unwrap();
    assert_eq!(project.plates.iter().map(|p| p.plate_index).collect::<Vec<_>>(), vec![1, 3]);
    assert_eq!(project.plates[0].estimated_seconds, Some(3600));
    assert_eq!(project.plates[0].gcode.max_layer, 3);
    assert_eq!(project.plates[1].estimated_seconds, Some(7200));
    assert_eq!(project.plates[1].gcode.max_layer, 7);
}
```

- [ ] **Step 2: Run the parser test and confirm RED**

Run: `cargo test parses_every_sliced_plate_in_numeric_order -- --nocapture`

Expected: compile failure because `parse_3mf_project` and `ParsedProjectV2` do not exist.

- [ ] **Step 3: Add the versioned types and enumerate `Metadata/plate_N.gcode`**

Add these production types in `parser/mod.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedProjectV2 {
    pub version: u8,
    pub plates: Vec<ParsedPlate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedPlate {
    pub plate_index: u32,
    pub display_name: Option<String>,
    pub estimated_seconds: Option<u32>,
    pub thumbnail_entries: Vec<String>,
    pub filaments: Vec<FilamentProfile>,
    pub gcode: gcode::GcodeReport,
}
```

In `three_mf.rs`, accept only exact `Metadata/plate_<digits>.gcode`, collect `(plate_index, entry_name)`, sort by index, reject duplicate indices, and parse every entry independently. Associate `plate_N.json`, `plate_N.png`, `plate_N_small.png`, `plate_no_light_N.png`, and the matching `<plate index="N">` from `slice_info.config`.

- [ ] **Step 4: Parse declared time and layer comments without changing extrusion totals**

Extend `GcodeReport` with defaulted fields:

```rust
#[serde(default)]
pub declared_estimated_seconds: Option<u32>,
#[serde(default)]
pub declared_total_layers: Option<u32>,
```

Recognize `; total estimated time: 5h 5m 7s` and `; total layer number: 14`. Prefer `slice_info.config prediction`; use the G-code value only when the XML metadata is missing. Keep `max_layer` derived from observed layer markers.

- [ ] **Step 5: Add missing-image, noncontiguous-plate, and single-plate compatibility tests**

Assert:

```rust
assert_eq!(project.plates[0].thumbnail_entries, Vec::<String>::new());
assert_eq!(parse_3mf(&single_plate_path).unwrap().gcode.max_layer, 14);
```

Also assert a project with no valid plate G-code returns `AppError::UnslicedProject`.

- [ ] **Step 6: Run parser tests and commit**

Run: `cargo test parser:: -- --nocapture`

Expected: all parser tests pass.

Commit:

```bash
git add src-tauri/src/parser
git commit -m "feat: parse every sliced 3mf plate"
```

---

### Task 2: Transactional History Schema and Legacy Backfill

**Files:**
- Create: `src-tauri/migrations/006_print_history.sql`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/domain.rs`
- Test: `src-tauri/src/db.rs`

**Interfaces:**
- Consumes: `ParsedProjectV2` JSON from Task 1.
- Produces: tables `print_projects`, `print_plates`, `media_assets`; nullable `print_jobs.plate_id`.
- Produces: domain DTOs `PrintProjectSummary`, `PrintProjectDetail`, `PrintPlateSummary`, `PlateStatus`.

- [ ] **Step 1: Write a failing migration test using a populated legacy database**

Build the existing pre-history schema, insert one spool, one parse cache row, one settled job, mappings, consumption, and ledger events, then open it with `AppDatabase::from_connection`. Assert the old job has a non-null plate and unchanged ledger count:

```rust
let plate_id: String = database.connection.query_row(
    "SELECT plate_id FROM print_jobs WHERE job_id = '22222222-2222-4222-8222-222222222222'",
    [], |row| row.get(0)
).unwrap();
assert!(!plate_id.is_empty());
assert_eq!(ledger_count(&database.connection), 2);
```

- [ ] **Step 2: Run the migration test and confirm RED**

Run: `cargo test print_history_migration_backfills_legacy_jobs_without_touching_ledger -- --nocapture`

Expected: SQL error because `plate_id` does not exist.

- [ ] **Step 3: Add migration 006**

The migration creates:

```sql
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
```

Create `media_assets` before the two tables, add `plate_id` to `print_jobs`, and index project import time, plate project/index, and jobs by plate. Use Rust backfill inside the same unchecked transaction because UUIDs cannot be generated by portable SQLite.

- [ ] **Step 4: Implement idempotent migration and backfill**

In `db.rs`, detect `print_projects` rather than assuming a migration version table. For each legacy source/job group, insert one project and plate 1, copy the cached parsed JSON into the plate, and update jobs. Never rewrite ledger rows.

- [ ] **Step 5: Add rollback and reopen tests**

Force a later backfill statement to fail and assert no history tables or `plate_id` column remain. Reopen a successfully migrated database twice and assert counts remain stable.

- [ ] **Step 6: Run database tests and commit**

Run: `cargo test db::tests -- --nocapture`

Commit:

```bash
git add src-tauri/migrations/006_print_history.sql src-tauri/src/db.rs src-tauri/src/domain.rs
git commit -m "feat: add print project history schema"
```

---

### Task 3: Safe Content-Addressed Thumbnail Storage

**Files:**
- Create: `src-tauri/src/media.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/media.rs`

**Interfaces:**
- Produces: `MediaStore::new(root: PathBuf) -> Result<Self>`
- Produces: `MediaStore::extract_image(archive_path: &Path, entry: &str) -> Result<Option<MediaAsset>>`
- Produces: `MediaAsset { asset_id, relative_path, mime_type, width, height, byte_size }`

- [ ] **Step 1: Write failing tests for deduplication and unsafe paths**

Use a temporary ZIP with one PNG and assert two extractions create one file. Add `../escape.png` and assert `AppError::InvalidFile` without any file outside the root.

- [ ] **Step 2: Run tests and confirm RED**

Run: `cargo test media::tests -- --nocapture`

Expected: compile failure because module `media` is missing.

- [ ] **Step 3: Implement bounded image extraction**

Validate normalized archive entry names, PNG/JPEG signatures, a 16 MiB compressed-entry limit, and decoded dimensions. Hash bytes with SHA-256 and write atomically to `media/<prefix>/<hash>.<ext>`. If the final file exists, validate its length and reuse it.

- [ ] **Step 4: Add corrupted-image and missing-entry tests**

Missing entries return `Ok(None)`; invalid image bytes return `AppError::InvalidFile`; neither writes `media_assets` rows.

- [ ] **Step 5: Register the media root and commit**

In Tauri setup, create `<app_data_dir>/media`, manage a shared `MediaStore`, and keep the existing database directory behavior.

Run: `cargo test media::tests -- --nocapture`

Commit:

```bash
git add src-tauri/src/media.rs src-tauri/src/lib.rs
git commit -m "feat: persist content addressed print thumbnails"
```

---

### Task 4: Import Projects and Create One Job Per Plate

**Files:**
- Create: `src-tauri/src/history/mod.rs`
- Modify: `src-tauri/src/imports.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/imports.rs`
- Test: `src-tauri/src/history/mod.rs`

**Interfaces:**
- Consumes: `ParsedProjectV2`, `MediaStore`, history schema.
- Produces: `import_print_project(path: &Path) -> Result<ImportProjectPreview>`
- Produces: `ImportProjectPreview { project_id, source_hash, source_file_name, imported_at, plates, state }`
- Produces: `ImportPlatePreview { plate_id, job_id, plate_index, thumbnail_url, estimated_seconds, max_layer, filaments, status }`

- [ ] **Step 1: Write a failing two-plate import test**

Import a two-plate fixture and assert one project, two plates, two jobs, one parse-cache row, and no ledger mutation:

```rust
assert_eq!(preview.plates.len(), 2);
assert_eq!(count(&db, "print_projects"), 1);
assert_eq!(count(&db, "print_plates"), 2);
assert_eq!(count(&db, "print_jobs"), 2);
assert_eq!(count(&db, "ledger_events"), ledger_before);
```

- [ ] **Step 2: Run test and confirm RED**

Run: `cargo test two_plate_import_creates_one_project_and_two_jobs -- --nocapture`

Expected: `import_print_project` is missing.

- [ ] **Step 3: Implement transactional project import**

Perform file stability check and hash, reuse or create the versioned parse cache, extract assets before the SQL transaction, then atomically insert project, plates, and jobs. Insert media metadata with `INSERT OR IGNORE`. Do not create mappings or ledger events.

- [ ] **Step 4: Implement duplicate rules**

`continue_project(source_hash)` returns the newest batch with pending plates. `confirm_new_project(source_hash, source_path)` creates a new project and jobs from cache after explicit confirmation. It must not duplicate media files.

- [ ] **Step 5: Implement discard and skip rules**

Replace job-only discard with `discard_project(project_id)`. In one transaction, reject if any linked job has a settlement or consumption, delete pending mappings/jobs/plates/project, preserve parse cache and immutable ledger. Add `skip_plate(plate_id)` that records `skipped` without consumption only when the project can no longer be discarded safely.

- [ ] **Step 6: Expose Tauri commands and compatibility wrapper**

Register `import_print_project`, `get_project_preview`, `confirm_new_project`, `discard_project`, and `skip_plate`. Keep `import_print_file` temporarily returning plate 1 for tray/pet callers; migrate those callers in Task 5.

- [ ] **Step 7: Run import/history tests and commit**

Run: `cargo test imports::tests history::tests -- --nocapture`

Commit:

```bash
git add src-tauri/src/imports.rs src-tauri/src/history src-tauri/src/lib.rs
git commit -m "feat: import sliced files as plate projects"
```

---

### Task 5: Plate-Scoped Settlement, Tray, and Black-Hole Import

**Files:**
- Modify: `src-tauri/src/settlement.rs`
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/pet/runtime.rs`
- Test: `src-tauri/src/settlement.rs`
- Test: `src-tauri/src/tray.rs`
- Test: `src-tauri/src/pet/runtime.rs`

**Interfaces:**
- Consumes: job-to-plate links and `ParsedPlate` from Task 4.
- Produces: unchanged `settle_job(job_id, outcome)` public command with plate-specific internals.
- Produces: project-level pending summary with newest project and pending plate count.

- [ ] **Step 1: Write a failing cross-plate settlement isolation test**

Map plate 1 and plate 2 to different spools, settle only plate 2, and assert plate 1 balances and job state remain unchanged.

- [ ] **Step 2: Run test and confirm RED**

Run: `cargo test settling_one_plate_never_consumes_another_plate -- --nocapture`

Expected: current parsed-job lookup cannot distinguish plate payloads.

- [ ] **Step 3: Resolve parsed data by `print_jobs.plate_id`**

Change `parsed_job(job_id)` to join `print_jobs -> print_plates` and deserialize that plate's `parsed_json`. Keep settlement and reversal event keys based on the existing job ID so ledger idempotency is preserved.

- [ ] **Step 4: Migrate tray watcher and black-hole runtime**

A dropped multi-plate file imports one project. Notification text reports the plate count. Pending navigation opens the project and selects the first pending plate. A second drop of a settled source requests a new project confirmation rather than silently printing again.

- [ ] **Step 5: Run affected tests and commit**

Run: `cargo test settlement::tests tray::tests pet::runtime::tests -- --nocapture`

Commit:

```bash
git add src-tauri/src/settlement.rs src-tauri/src/tray.rs src-tauri/src/pet/runtime.rs
git commit -m "feat: settle and navigate print plates independently"
```

---

### Task 6: History Queries and Backup Schema V3

**Files:**
- Modify: `src-tauri/src/history/mod.rs`
- Modify: `src-tauri/src/backup.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/history/mod.rs`
- Test: `src-tauri/src/backup.rs`

**Interfaces:**
- Produces: `list_print_projects(filter: HistoryFilter) -> Result<Vec<PrintProjectSummary>>`
- Produces: `get_print_project(project_id: Uuid) -> Result<PrintProjectDetail>`
- Produces: Tauri commands with the same names.
- Produces: backup schema version 3 with projects, plates, media metadata, printers excluded until slicing plan Task 2.

- [ ] **Step 1: Write failing query tests for pending/history grouping**

Create one pending two-plate project and one partially settled project. Assert pending results contain the first and history contains the second once, with literal plate counts and summed duration.

- [ ] **Step 2: Run tests and confirm RED**

Run: `cargo test history_lists_projects_once_and_summarizes_plates -- --nocapture`

- [ ] **Step 3: Implement parameterized history queries**

Use SQL aggregation for plate counts and duration, then fetch plate summaries in one additional query for all selected project IDs. Do not issue one query per project. Resolve media URLs as Tauri asset protocol paths from `relative_path`.

- [ ] **Step 4: Write backup v3 round-trip and v2 restore tests**

Assert a v3 backup round trip preserves project IDs, plate IDs, parsed data, media hashes, job links, and ledger. Restore an existing v2 fixture and assert it becomes one single-plate project without changing balances.

- [ ] **Step 5: Implement backup v3 and media archive entries**

Store media files in the backup ZIP under `media/<asset_id>.<ext>` and validate every hash before restore. Do not include source or generated 3MF files.

- [ ] **Step 6: Run tests and commit**

Run: `cargo test history::tests backup::tests -- --nocapture`

Commit:

```bash
git add src-tauri/src/history src-tauri/src/backup.rs src-tauri/src/lib.rs
git commit -m "feat: query and back up print history"
```

---

### Task 7: Typed Frontend History API

**Files:**
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/tauri.test.ts`

**Interfaces:**
- Consumes: backend DTOs from Tasks 4 and 6.
- Produces: `PrintProjectSummary`, `PrintProjectDetail`, `PrintPlateSummary`, `ImportProjectPreview` TypeScript interfaces.
- Produces: `listPrintProjects`, `getPrintProject`, `importPrintProject`, `discardProject`, `skipPlate`, `confirmNewProject` API methods.

- [ ] **Step 1: Write failing adapter payload tests**

Assert exact camelCase invocation payloads, including:

```ts
await api.getPrintProject("project-1");
expect(invoke).toHaveBeenCalledWith("get_print_project", { projectId: "project-1" });
```

- [ ] **Step 2: Run test and confirm RED**

Run: `npm test -- --run src/lib/tauri.test.ts`

- [ ] **Step 3: Add complete DTOs and command adapter methods**

Mirror all real backend fields; do not use `Partial`. Keep deprecated single-job methods only while `App.tsx` is migrated in Task 9.

- [ ] **Step 4: Extend demo mode with one multi-plate project**

The demo must expose two plate thumbnails/statuses and preserve existing spool identities so current component tests remain meaningful.

- [ ] **Step 5: Run test and commit**

Run: `npm test -- --run src/lib/tauri.test.ts`

Commit:

```bash
git add src/lib/tauri.ts src/lib/tauri.test.ts
git commit -m "feat: add typed print history api"
```

---

### Task 8: Project History and Detail Components

**Files:**
- Create: `src/features/jobs/History.tsx`
- Create: `src/features/jobs/History.test.tsx`
- Create: `src/features/jobs/Project.tsx`
- Create: `src/features/jobs/Project.test.tsx`
- Modify: `src/features/jobs/Job.tsx`
- Modify: `src/features/jobs/Job.test.tsx`
- Modify: `src/styles.css`
- Modify: `src/i18n/locales/zh-CN.json`
- Modify: `src/i18n/locales/zh-TW.json`
- Modify: `src/i18n/locales/en.json`

**Interfaces:**
- Consumes: history DTOs and existing `Job` mapping/settlement callbacks.
- Produces: `History({ pending, history, onOpenProject })`.
- Produces: `Project({ project, selectedPlateId, onSelectPlate, ...jobActions })`.

- [ ] **Step 1: Write failing accessible UI tests**

Test one single-plate and one three-plate project. Assert project names appear once, the multi-plate badge says `共 3 盘`, the imported time is rendered, and clicking opens plate cards with duration, layers, colors, and grams.

- [ ] **Step 2: Run tests and confirm RED**

Run: `npm test -- --run src/features/jobs/History.test.tsx src/features/jobs/Project.test.tsx`

- [ ] **Step 3: Implement history cards and empty states**

Use semantic buttons/articles. The cover image must have localized alt text; missing media uses the CYLUNE mark and visible fallback copy. Do not use color alone for status.

- [ ] **Step 4: Implement project and plate detail**

Render project header, total progress, and one button per plate. Selecting a pending plate mounts the existing `Job` work area with that plate's `ImportPreview`. Settled plates show result and reversal; skipped plates never show deduction controls.

- [ ] **Step 5: Add localized copy and responsive styles**

Use `Intl.DateTimeFormat` and a shared `formatDuration(seconds, locale)` helper tested with literal `5 小时 5 分钟`, `5 小時 5 分鐘`, and `5 hr 5 min` outputs. Add a two-column card grid that collapses below 760 px and remains legible in dark mode.

- [ ] **Step 6: Run component and i18n tests, then commit**

Run: `npm test -- --run src/features/jobs src/i18n/i18n.test.ts`

Commit:

```bash
git add src/features/jobs src/styles.css src/i18n/locales
git commit -m "feat: show project grouped print history"
```

---

### Task 9: App Integration and Real-File Regression

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src/features/home/Home.tsx`
- Modify: `src/features/home/Home.test.tsx`
- Modify: `src-tauri/src/settlement.rs`
- Create: `docs/qa-print-history.md`

**Interfaces:**
- Consumes: all backend/frontend APIs above.
- Produces: full navigation from import to project, plate mapping, settlement, history, and reversal.

- [ ] **Step 1: Write failing App flow tests**

Cover: import two plates, see one project, select plate 2, map it, settle success, return to history, and verify only plate 2 is settled. Cover whole-project discard before settlement and rejection after one plate settles.

- [ ] **Step 2: Run App tests and confirm RED**

Run: `npm test -- --run src/App.test.tsx`

- [ ] **Step 3: Replace single preview state with project state**

Store `projects`, `activeProject`, and `selectedPlateId`; derive pending count from plate statuses. Desktop events carry `project_id` and optionally `plate_id`. Preserve busy-action locking and error localization.

- [ ] **Step 4: Add a real-file smoke assertion**

Extend the ignored `BAMBU_SMOKE_3MF` test to assert the supplied file has one plate, four filaments, 14 layers, `18_307` predicted seconds, and available cover/plate thumbnail candidates. Keep it ignored unless the environment variable is set.

- [ ] **Step 5: Run full verification**

Run:

```bash
npm test -- --run
npm run build
cd src-tauri && cargo fmt -- --check && cargo test
```

Expected: zero failures; only the environment-gated real-file smoke test may be ignored.

- [ ] **Step 6: Write manual QA and commit**

Document single-plate import, multi-plate import, missing thumbnail, cancel/reimport, per-plate settlement, reversal, dark mode, and all three locales.

Commit:

```bash
git add src/App.tsx src/App.test.tsx src/features/home src-tauri/src/settlement.rs docs/qa-print-history.md
git commit -m "feat: complete multi plate print history"
```

- [ ] **Step 7: Build the macOS preview**

Run: `npm run release:mac`

Verify:

```bash
codesign --verify --deep --strict 发布/CYLUNE.app
test ! -e src-tauri/target/release/bundle/macos/CYLUNE.app
```

Commit no generated `.app` or `target` files.
