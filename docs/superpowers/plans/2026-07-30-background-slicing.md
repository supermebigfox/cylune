# Background Bambu Studio Slicing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users send an unsliced project 3MF through a concise CYLUNE workflow that silently invokes the installed Bambu Studio CLI and imports the validated multi-plate result into print history.

**Architecture:** A platform-neutral `SlicerAdapter` discovers Bambu Studio and official presets, builds process arguments without a shell, owns a cancellable child process, and emits stable progress events. A printer service stores the user's local device choices. The React slicing flow collects only fast settings, selects a destination, monitors the job, and hands successful output to the project importer built in the print-history plan.

**Tech Stack:** Rust `std::process`, serde_json, rusqlite, Tauri events, React 18, TypeScript, Vitest, Testing Library; Bambu Studio CLI installed separately.

## Global Constraints

- Implement after `2026-07-30-print-history-multiplate.md`; consume `import_print_project` rather than duplicating import logic.
- Normal slicing must not open the Bambu Studio GUI.
- Use the installed Bambu Studio binary and official local profiles; do not bundle or reimplement the slicer.
- Do not show or save Bambu Studio version or a complete slicing-settings snapshot in print history.
- Fast settings are printer, nozzle, plate, layer-height preset, infill, support, and per-tool filament only.
- Never silently substitute a different printer, nozzle, plate, process, or filament profile.
- Pass child-process arguments as an array; never interpolate an input path into a shell command.
- Write to a private temporary directory first, validate the result, then atomically publish to the user-selected destination.
- Never overwrite an existing destination without explicit user confirmation.
- Cancelling or exiting terminates the child and removes incomplete temporary output.
- macOS ships first; isolate discovery/launch details behind an adapter for later Windows support.

---

## File Structure

- Create `src-tauri/migrations/007_printers.sql`: local printer library.
- Create `src-tauri/src/printers.rs`: device CRUD and official preset discovery.
- Create `src-tauri/src/slicer/mod.rs`: public slicing service and state.
- Create `src-tauri/src/slicer/discovery.rs`: platform-specific installation discovery.
- Create `src-tauri/src/slicer/command.rs`: typed CLI argument construction.
- Create `src-tauri/src/slicer/runtime.rs`: child lifecycle, progress, cancellation, temp output.
- Modify `src-tauri/src/error.rs`: stable slicing error codes.
- Modify `src-tauri/src/lib.rs`: managed services and Tauri commands/events.
- Modify `src-tauri/src/backup.rs`: backup schema v3 printer rows.
- Create `src/features/settings/Printers.tsx`: printer library UI.
- Create `src/features/jobs/Slice.tsx`: fast slicing flow.
- Modify `src/features/settings/Settings.tsx`, `src/App.tsx`, `src/lib/dialog.ts`, `src/lib/tauri.ts`, styles, and locales.

---

### Task 1: Official Bambu Studio and Preset Discovery

**Files:**
- Create: `src-tauri/src/slicer/mod.rs`
- Create: `src-tauri/src/slicer/discovery.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/slicer/discovery.rs`

**Interfaces:**
- Produces: `BambuInstallation { executable: PathBuf, profiles_root: PathBuf }`
- Produces: `InstallationDiscovery::discover() -> Result<BambuInstallation>`
- macOS candidates: `/Applications/BambuStudio.app/Contents/MacOS/BambuStudio` and user-selected `.app`.
- Windows adapter remains a trait implementation point; it is not shipped in this phase.

- [ ] **Step 1: Write failing discovery tests with a fake app bundle**

Create a temporary `BambuStudio.app/Contents/MacOS/BambuStudio` executable and `Contents/Resources/profiles` directory. Assert both canonical paths are returned. Assert missing profiles produce `AppError::SlicerProfilesMissing`.

- [ ] **Step 2: Run test and confirm RED**

Run: `cargo test slicer::discovery::tests -- --nocapture`

- [ ] **Step 3: Implement discovery without launching the app**

Use metadata checks only. Reject symlinked executable/profile roots. Allow an explicit path saved in app settings, then fall back to the system Applications path.

- [ ] **Step 4: Add stable errors and register module**

Add `bambu_studio_missing`, `slicer_profiles_missing`, `slicer_incompatible`, `slicer_failed`, `slicer_cancelled`, and `output_exists` to `AppError::code()` and frontend error localization.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test slicer::discovery::tests error::tests -- --nocapture`

Commit:

```bash
git add src-tauri/src/slicer src-tauri/src/error.rs src-tauri/src/lib.rs src/i18n/locales
git commit -m "feat: discover installed Bambu Studio"
```

---

### Task 2: Printer Library and Official Profile Catalog

**Files:**
- Create: `src-tauri/migrations/007_printers.sql`
- Create: `src-tauri/src/printers.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/backup.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/printers.rs`
- Test: `src-tauri/src/backup.rs`

**Interfaces:**
- Produces: `PrinterProfile { model_key, display_name, nozzle_diameters, plate_keys }`
- Produces: `SavedPrinter { printer_id, display_name, model_key, nozzle_diameter, default_plate, ams_kind, is_default }`
- Produces commands: `list_available_printers`, `list_saved_printers`, `save_printer`, `delete_printer`, `set_default_printer`.

- [ ] **Step 1: Write failing profile inheritance tests**

Build a miniature BBL profile tree with a base machine and a P2S 0.4 nozzle child. Assert the returned model key, display name, nozzle `0.4`, and supported plates are literal values from the resolved profile.

- [ ] **Step 2: Run test and confirm RED**

Run: `cargo test printers::tests -- --nocapture`

- [ ] **Step 3: Add migration 007 and CRUD invariants**

Create the `printers` table from the design. Enforce one default printer with a partial unique index:

```sql
CREATE UNIQUE INDEX one_default_printer ON printers(is_default) WHERE is_default = 1;
```

Setting a default clears the old default and sets the new one in one transaction. Deleting the default selects no replacement automatically.

- [ ] **Step 4: Parse official profiles with explicit inheritance resolution**

Read only JSON under the discovered `profiles` root. Resolve `inherits` with a visited set and reject cycles. Return only Bambu Lab machine profiles; do not treat third-party vendor profiles as supported printers in this phase.

- [ ] **Step 5: Add backup round-trip**

Extend backup v3 with `#[serde(default)] printers: Vec<PrinterRow>` but not external profile files. The default is required so backups produced after the history plan but before printer support still restore. Restore saved devices even when their current profile is absent; mark availability at query time.

- [ ] **Step 6: Run tests and commit**

Run: `cargo test printers::tests backup::tests db::tests -- --nocapture`

Commit:

```bash
git add src-tauri/migrations/007_printers.sql src-tauri/src/printers.rs src-tauri/src/db.rs src-tauri/src/backup.rs src-tauri/src/lib.rs
git commit -m "feat: add local printer library"
```

---

### Task 3: Typed, Shell-Free CLI Command Builder

**Files:**
- Create: `src-tauri/src/slicer/command.rs`
- Test: `src-tauri/src/slicer/command.rs`

**Interfaces:**
- Consumes: a `SavedPrinter` and resolved machine/process/filament file paths.
- Produces: `SliceRequest { input, destination, plate_selection, machine_settings, process_settings, filament_settings, fast_overrides }`.
- Produces: `build_bambu_args(request: &SliceRequest, temporary_output: &Path) -> Result<Vec<OsString>>`.

- [ ] **Step 1: Write failing exact-argument tests**

Use paths containing spaces and shell metacharacters. Assert a literal vector beginning with:

```rust
vec![
  "--slice", "0", "--debug", "2", "--load-settings",
  "/profiles/P2S machine.json;/profiles/0.20 Standard.json",
  "--load-filaments", "/profiles/PLA Basic.json",
  "--export-3mf", "/tmp/out.gcode.3mf",
  "/models/a model;not-a-command.3mf"
]
```

No element may contain quoting added by CYLUNE.

- [ ] **Step 2: Run test and confirm RED**

Run: `cargo test slicer::command::tests -- --nocapture`

- [ ] **Step 3: Implement validated request and argument construction**

Require existing regular input/settings files, `.3mf` input, `.gcode.3mf` destination, nonempty filament list, plate selection `0` for all plates, and finite infill in `0..=100`. Translate fast overrides to individual `--key=value` arguments only from an allowlist.

- [ ] **Step 4: Add incompatible-profile tests**

Reject missing machine/process files, mismatched filament count, NaN, an existing destination without overwrite authorization, and a destination equal to the input path.

- [ ] **Step 5: Run tests and commit**

Run: `cargo test slicer::command::tests -- --nocapture`

Commit:

```bash
git add src-tauri/src/slicer/command.rs
git commit -m "feat: build safe Bambu slicing commands"
```

---

### Task 4: Cancellable Slicing Runtime and Output Validation

**Files:**
- Create: `src-tauri/src/slicer/runtime.rs`
- Modify: `src-tauri/src/slicer/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/slicer/runtime.rs`

**Interfaces:**
- Produces: `SlicerService::start(request) -> Result<SliceTask>`
- Produces: `SlicerService::cancel(task_id: Uuid) -> Result<()>`
- Produces events: `slice-progress`, `slice-complete`, `slice-error` with `task_id`.
- Consumes: `import_print_project` after validation.

- [ ] **Step 1: Write a fake CLI fixture and failing success test**

The fake executable writes known progress lines and copies a valid sliced fixture to the requested output. Assert events arrive in order and the final file is atomically published only after `parse_3mf_project` succeeds.

- [ ] **Step 2: Run test and confirm RED**

Run: `cargo test slicer::runtime::tests -- --nocapture`

- [ ] **Step 3: Implement child ownership and staged output**

Spawn with `Command::new(executable).args(args)`, piped stdout/stderr, and no shell. Keep child handles in `Mutex<HashMap<Uuid, RunningSlice>>`. Write to `app_cache_dir/slices/<task_id>/output.gcode.3mf`; validate stability, archive readability, and at least one plate before rename/copy to destination.

- [ ] **Step 4: Implement progress and cancellation**

Parse only documented progress messages. When no percentage is available, emit phases `preparing`, `slicing`, `validating`, `importing`, `complete` with `percent: null`. Cancellation kills and waits for the child before removing the task directory, then emits `slicer_cancelled`.

- [ ] **Step 5: Add crash, invalid-output, cancel, and output-exists tests**

Assert no history project is created for nonzero exit, truncated ZIP, cancellation, or destination collision. Assert stderr is bounded to 64 KiB and private absolute paths are not returned through serialized errors.

- [ ] **Step 6: Register Tauri commands and commit**

Commands: `start_slice`, `cancel_slice`, `get_slice_task`.

Run: `cargo test slicer::runtime::tests -- --nocapture`

Commit:

```bash
git add src-tauri/src/slicer src-tauri/src/lib.rs
git commit -m "feat: run cancellable background slicing"
```

---

### Task 5: Printer Settings UI

**Files:**
- Create: `src/features/settings/Printers.tsx`
- Create: `src/features/settings/Printers.test.tsx`
- Modify: `src/features/settings/Settings.tsx`
- Modify: `src/features/settings/Settings.test.tsx`
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/tauri.test.ts`
- Modify: `src/styles.css`
- Modify: locale JSON files.

**Interfaces:**
- Consumes: printer commands from Task 2.
- Produces: printer list, add/edit dialog, default action, unavailable-profile warning.

- [ ] **Step 1: Write failing UI and adapter tests**

Assert a user can add `我的 P2S`, choose `0.4 mm`, `Supertack Plate`, `AMS`, and make it default. Assert a saved unavailable model is visible with a warning and cannot start slicing.

- [ ] **Step 2: Run tests and confirm RED**

Run: `npm test -- --run src/features/settings/Printers.test.tsx src/lib/tauri.test.ts`

- [ ] **Step 3: Add typed API and printer component**

Use complete DTOs matching Rust. Keep dialog focus trapped using the existing spool-dialog pattern. Delete is recoverable only at the database level and must confirm if the printer is default.

- [ ] **Step 4: Add three-language copy and responsive styles**

Use official profile display names for model names and localized CYLUNE labels for actions/statuses. Do not translate profile identifiers.

- [ ] **Step 5: Run tests and commit**

Run: `npm test -- --run src/features/settings src/lib/tauri.test.ts src/i18n/i18n.test.ts`

Commit:

```bash
git add src/features/settings src/lib/tauri.ts src/lib/tauri.test.ts src/styles.css src/i18n/locales
git commit -m "feat: manage local printers"
```

---

### Task 6: Fast Slicing Flow

**Files:**
- Create: `src/features/jobs/Slice.tsx`
- Create: `src/features/jobs/Slice.test.tsx`
- Modify: `src/lib/dialog.ts`
- Modify: `src/lib/dialog.test.ts`
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/tauri.test.ts`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`
- Modify: `src-tauri/src/slicer/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/styles.css`
- Modify: locale JSON files.

**Interfaces:**
- Consumes: saved printers, available fast presets, `startSlice`, `cancelSlice`, and slicing events.
- Produces: ordinary `.3mf` selection, settings confirmation, output location, progress, cancellation, and project navigation.

- [ ] **Step 1: Write failing end-to-end component tests**

Cover: select unsliced file, default P2S appears, choose layer/process/plate/support/infill/filaments, select destination, start, receive progress phases, receive completed project ID, and navigate to project detail. Also cover user cancellation and CLI failure with “使用 Bambu Studio 打开”.

- [ ] **Step 2: Run tests and confirm RED**

Run: `npm test -- --run src/features/jobs/Slice.test.tsx src/App.test.tsx`

- [ ] **Step 3: Expand file dialogs and distinguish input type**

The picker accepts `.3mf` and `.gcode.3mf`. Inspect the archive through a backend `inspect_3mf` command; never infer sliced state only from the filename. Sliced files go directly to import; unsliced projects open `Slice`.

- [ ] **Step 4: Implement the fast-settings form**

Use a single dialog with sections: target printer, process, plate, material mapping, output. Embedded compatible settings prefill the form. Incompatible embedded machine settings show an explicit mismatch warning and require target confirmation.

- [ ] **Step 5: Implement progress and action locking**

While running, freeze all settings and file inputs. Show true percentage only when present; otherwise show phase text and an indeterminate bar. Cancel remains enabled and changes to “正在停止…” until the backend confirms child exit.

- [ ] **Step 6: Add GUI fallback action**

`open_in_bambu_studio(path)` is a separate explicit command invoked only by the visible user button after an error or from advanced settings. It is never called automatically.

- [ ] **Step 7: Run tests and commit**

Run: `npm test -- --run src/features/jobs/Slice.test.tsx src/App.test.tsx src/lib/dialog.test.ts src/i18n/i18n.test.ts`

Commit:

```bash
git add src/features/jobs/Slice.tsx src/features/jobs/Slice.test.tsx src/lib/dialog.ts src/lib/dialog.test.ts src/lib/tauri.ts src/lib/tauri.test.ts src/App.tsx src/App.test.tsx src-tauri/src/slicer/mod.rs src-tauri/src/lib.rs src/styles.css src/i18n/locales
git commit -m "feat: add fast background slicing flow"
```

---

### Task 7: Real Bambu Studio Smoke Test and Release

**Files:**
- Create: `docs/qa-background-slicing.md`
- Modify: `src-tauri/src/slicer/runtime.rs`
- Modify: `scripts/release-mac.mjs` only if release verification exposes a packaging defect.

**Interfaces:**
- Validates the completed slicing and history integration on the user's installed Bambu Studio and P2S profile.

- [ ] **Step 1: Add an environment-gated real CLI smoke test**

When `BAMBU_STUDIO_SMOKE_3MF` and `BAMBU_STUDIO_SMOKE_OUTPUT` are set, discover `/Applications/BambuStudio.app`, slice all plates, validate output, and assert at least one imported plate. Keep ignored by default so CI does not require external software.

- [ ] **Step 2: Run complete automated verification**

Run:

```bash
npm test -- --run
npm run build
cd src-tauri && cargo fmt -- --check && cargo test
```

Expected: zero failures; external-software tests ignored unless explicitly enabled.

- [ ] **Step 3: Run the local P2S smoke test**

Use a user-owned unsliced 3MF copy and a fresh output path. Verify no Bambu Studio window opens, the output is a readable `.gcode.3mf`, every valid plate enters one project, and the source file checksum is unchanged.

- [ ] **Step 4: Complete manual QA**

Test missing Bambu Studio, missing profile, incompatible embedded printer, destination collision, cancel, CLI crash, empty plate, dark/light modes, and all three languages. Record exact results in `docs/qa-background-slicing.md`.

- [ ] **Step 5: Build and verify the macOS app**

Run:

```bash
npm run release:mac
codesign --verify --deep --strict 发布/CYLUNE.app
test ! -e src-tauri/target/release/bundle/macos/CYLUNE.app
```

- [ ] **Step 6: Commit**

```bash
git add docs/qa-background-slicing.md src-tauri/src/slicer/runtime.rs
git commit -m "test: verify background Bambu slicing"
```

Do not stage `target`, `dist`, generated `.app`, generated `.gcode.3mf`, or user model files.
