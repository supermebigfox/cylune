# Black Hole Routing and Ephemeral Slicing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route unsliced 3MF files dropped into the black hole to Quick Slice, and keep Bambu's generated G-code only long enough to persist print metrics and compressed thumbnails.

**Architecture:** Rust classifies black-hole drops by 3MF contents and emits a dedicated `open-slice` navigation event for project archives. Quick Slice sends only the original input and selected presets; `SlicerService` owns a private per-task output, imports it with the original source identity, then removes the task directory on every terminal path.

**Tech Stack:** Tauri 2, Rust, React 19, TypeScript, Vitest, rusqlite, Bambu Studio CLI.

---

## File Map

- `src-tauri/src/pet/runtime.rs`: classify black-hole drops and emit slice navigation without creating a print project.
- `src/App.tsx`: consume `open-slice`, clear stale import errors, and hand the dropped path to persistent Quick Slice.
- `src/App.test.tsx`: cover black-hole navigation and absence of the old unsliced error.
- `src/features/slice/Slice.tsx`: remove destination selection and submit a metadata-only slice request.
- `src/features/slice/Slice.test.tsx`: prove the form has no output controls and requests no destination.
- `src/lib/dialog.ts`, `src/lib/dialog.test.ts`: remove the obsolete slice save dialog.
- `src/lib/tauri.ts`, `src/lib/tauri.test.ts`: keep the Tauri request boundary destination-free.
- `src-tauri/src/slicer/mod.rs`: resolve high-level requests without a caller-controlled output path.
- `src-tauri/src/slicer/command.rs`: make low-level slice requests private-output-only.
- `src-tauri/src/slicer/runtime.rs`: import directly from private output, preserve original source identity, and clean task directories.
- `src-tauri/src/imports.rs`: add generated-project import with separate content and display/source paths.

### Task 1: Route unsliced black-hole drops to Quick Slice

**Files:**
- Modify: `src-tauri/src/pet/runtime.rs`
- Modify: `src/App.tsx`
- Test: `src-tauri/src/pet/runtime.rs`
- Test: `src/App.test.tsx`

- [ ] **Step 1: Write failing Rust classification test**

Add a project-only 3MF test that calls `handle_file_drop` and expects a new signal without database mutation:

```rust
let signal = handle_file_drop(&mut core.service, &project_only).unwrap();
assert_eq!(signal, PetSignal::SliceRequested { path: canonical });
assert_eq!(count(&core.service, "print_projects"), 0);
```

- [ ] **Step 2: Run the Rust test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml pet::runtime::tests::unsliced_black_hole_drop_requests_slicing -- --exact`

Expected: FAIL because `PetSignal::SliceRequested` does not exist and unsliced input returns `unsliced_project`.

- [ ] **Step 3: Implement content-based drop routing**

Add the signal and classify after `DropValidation`:

```rust
PetSignal::SliceRequested { path: PathBuf }
```

```rust
let inspection = inspect_3mf_content(&validation.canonical_path)?;
if inspection.kind == ThreeMfKind::Unsliced {
    return Ok(PetSignal::SliceRequested { path: validation.canonical_path });
}
```

Treat this signal as `NativeDropResult::Accepted`, show the main window, and emit `open-slice` with the canonical path. Do not update pending job counts or emit `pet-import-error`.

- [ ] **Step 4: Write failing React navigation test**

Subscribe a test `DesktopApp` to `open-slice`, emit `"/tmp/project.3mf"`, and assert:

```tsx
expect(await screen.findByRole("heading", { name: "project.3mf" })).toBeVisible();
expect(screen.getByRole("button", { name: "切片" })).toHaveAttribute("aria-current", "page");
expect(screen.queryByText("这个项目尚未切片")).not.toBeInTheDocument();
```

- [ ] **Step 5: Run the React test and verify RED**

Run: `npm test -- --run src/App.test.tsx -t "routes an unsliced black-hole drop"`

Expected: FAIL because `open-slice` is not registered.

- [ ] **Step 6: Implement `open-slice` consumption**

Extend `DesktopEventName` and register a handler that clears the old error, mounts Slice, increments the input nonce, and switches to `slice`.

- [ ] **Step 7: Run focused tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml pet::runtime::tests`

Run: `npm test -- --run src/App.test.tsx`

Expected: PASS.

```bash
git add src-tauri/src/pet/runtime.rs src/App.tsx src/App.test.tsx
git commit -m "fix: route black hole projects to slicing"
```

### Task 2: Remove user-facing slice output selection

**Files:**
- Modify: `src/features/slice/Slice.tsx`
- Modify: `src/features/slice/Slice.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/lib/dialog.ts`
- Modify: `src/lib/dialog.test.ts`

- [ ] **Step 1: Write failing form/API test**

Prepare a valid project and assert the start button becomes enabled without choosing an output. After submitting, assert the exact request has no `output_path` key:

```tsx
expect(screen.queryByRole("button", { name: "选择输出位置" })).not.toBeInTheDocument();
await user.click(screen.getByRole("button", { name: "开始后台切片" }));
expect(client.startSlice).toHaveBeenCalledWith(expect.not.objectContaining({ output_path: expect.anything() }));
```

- [ ] **Step 2: Run the Slice test and verify RED**

Run: `npm test -- --run src/features/slice/Slice.test.tsx -t "starts metadata slicing without asking for an output"`

Expected: FAIL because the output section is rendered and `formValid` requires `outputPath`.

- [ ] **Step 3: Remove destination state and UI**

Delete `pickOutput`, `outputPath`, `chooseOutput`, output copy keys, `suggestedOutputName`, and the output form section. Remove `output_path` from the TypeScript `SliceRequest`; let validity depend only on input, printer, process, plate, filament and mismatch confirmation.

- [ ] **Step 4: Remove the obsolete dialog boundary**

Delete `pickSliceDestination` and its test. Remove `pickSliceOutput` from `DesktopApp` props and every App test setup.

- [ ] **Step 5: Run focused tests and commit**

Run: `npm test -- --run src/features/slice/Slice.test.tsx src/App.test.tsx src/lib/dialog.test.ts`

Expected: PASS.

```bash
git add src/features/slice/Slice.tsx src/features/slice/Slice.test.tsx src/App.tsx src/App.test.tsx src/lib/dialog.ts src/lib/dialog.test.ts
git commit -m "feat: make slicing output internal"
```

### Task 3: Import private output with original project identity

**Files:**
- Modify: `src-tauri/src/imports.rs`
- Modify: `src-tauri/src/slicer/runtime.rs`
- Test: `src-tauri/src/imports.rs`
- Test: `src-tauri/src/slicer/runtime.rs`

- [ ] **Step 1: Write failing generated-import identity test**

Copy a sliced fixture to a generated path and call the wished-for API:

```rust
let preview = service.import_generated_project(&generated, &original).unwrap();
assert_eq!(preview.source_file_name, "original-model.3mf");
assert_eq!(project_source_path(&service, preview.project_id), original.to_string_lossy());
assert!(preview.plates.iter().all(|plate| plate.thumbnail_url.is_some()));
```

- [ ] **Step 2: Run the import test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml imports::tests::generated_project_uses_original_identity -- --exact`

Expected: FAIL because `import_generated_project` does not exist.

- [ ] **Step 3: Implement separate content and source identity**

Add:

```rust
pub fn import_generated_project(
    &mut self,
    generated_path: &Path,
    original_path: &Path,
) -> Result<ImportProjectPreview>
```

Refactor the current import internals so hashing, parsing, stability and media extraction use `generated_path`, while `source_file_name` and the `print_projects.source_path` value come from `original_path`. Never persist `generated_path`.

- [ ] **Step 4: Write failing runtime cleanup/persistence test**

Update the real `ProjectImporter` test double to record both paths and assert after completion:

```rust
assert_eq!(importer.original_path(), fixture.input);
assert!(!task_root.exists());
assert!(!destination.exists());
assert!(stored_thumbnail.is_file());
```

- [ ] **Step 5: Run runtime test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml slicer::runtime::tests::validates_imports_private_output_and_removes_it -- --exact`

Expected: FAIL because runtime publishes a destination and importer receives only that path.

- [ ] **Step 6: Change the importer boundary**

Change the trait to:

```rust
fn import_project(&self, generated_path: &Path, original_path: &Path) -> Result<Uuid>;
```

Make `TauriSliceImporter` lock `PrintState`, call `import_generated_project`, refresh the Pet pending summary, and return the project id.

- [ ] **Step 7: Import before cleanup without publishing**

In `finish_success`, validate `temporary_output`, set phase to importing, import it with `request.input` as original identity, and set completion. Delete `PublishedOutput`, `publish_output`, backup/rollback code and explicit destination handling.

- [ ] **Step 8: Run focused Rust tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml imports::tests`

Run: `cargo test --manifest-path src-tauri/Cargo.toml slicer::runtime::tests`

Expected: PASS, with task roots absent after success/failure/cancel.

```bash
git add src-tauri/src/imports.rs src-tauri/src/slicer/runtime.rs
git commit -m "feat: import and clean private slice output"
```

### Task 4: Remove caller-controlled output from Rust requests

**Files:**
- Modify: `src-tauri/src/slicer/mod.rs`
- Modify: `src-tauri/src/slicer/command.rs`
- Modify: `src-tauri/src/slicer/runtime.rs`
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/tauri.test.ts`

- [ ] **Step 1: Write failing Tauri boundary tests**

Assert a serialized `FastSliceRequest` and frontend `start_slice` invocation contain input/presets but no `output_path`, `destination`, or `allow_overwrite` fields.

- [ ] **Step 2: Run boundary tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml slicer::task6_tests`

Run: `npm test -- --run src/lib/tauri.test.ts -t "starts private metadata slicing"`

Expected: FAIL because output fields are still required.

- [ ] **Step 3: Remove output fields from request types**

Delete `FastSliceRequest.output_path`, `SliceRequest.destination`, and `SliceRequest.allow_overwrite`. Keep the private `temporary_output` argument to `build_bambu_args`, and validate only that it is a safe non-existing `.gcode.3mf` inside the service-owned task directory.

- [ ] **Step 4: Update fixtures and runtime callers**

Update every test constructor and real-slice environment test to let `SlicerService` choose the output path. Remove destination collision and overwrite tests that no longer represent a reachable product behavior; retain tests that reject unsafe private paths.

- [ ] **Step 5: Run focused tests and commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml slicer::`

Run: `npm test -- --run src/lib/tauri.test.ts src/features/slice/Slice.test.tsx`

Expected: PASS.

```bash
git add src-tauri/src/slicer src/lib/tauri.ts src/lib/tauri.test.ts src/features/slice
git commit -m "refactor: keep slicer output service-owned"
```

### Task 5: Orphan cleanup, real samples, and final package

**Files:**
- Modify: `src-tauri/src/slicer/runtime.rs`
- Test: `src-tauri/src/slicer/runtime.rs`
- Verify: `/Users/robin/Downloads/Poopy_Bucket_Eco_X2D_V4.3mf`
- Verify: `/Users/robin/Downloads/风洞v3.3mf`

- [ ] **Step 1: Write failing startup orphan cleanup test**

Create stale UUID-named task directories under `<cache>/slices`, initialize `SlicerService`, and assert only those private task directories are removed while unrelated cache files remain.

- [ ] **Step 2: Run the test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml slicer::runtime::tests::startup_removes_only_orphaned_slice_tasks -- --exact`

Expected: FAIL because startup does not scan stale task roots.

- [ ] **Step 3: Implement bounded orphan cleanup**

On service construction, inspect only `<cache>/slices`; reject symlinks, remove regular UUID-named directories, and never traverse or delete the cache root or unrelated children.

- [ ] **Step 4: Run all automated verification**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: 0 failures.

Run: `npm test -- --run`

Expected: 0 failures.

Run: `npm run build`

Expected: exit 0.

Run: `git diff --check`

Expected: no output.

- [ ] **Step 5: Run real single- and multi-plate regressions**

Run the ignored real-slice smoke test with P2S 0.4 mm and `Supertack Plate` for both supplied files. Assert Poopy remains about 221.52 g, 891 layers and 17,243 model seconds; assert 风洞 remains four plates totaling 782.07 g. For both runs assert the private task directory is gone and the media thumbnails remain readable.

- [ ] **Step 6: Build and manually verify one final App**

Run: `npm run release:mac`

Open `/Users/robin/Desktop/耗材管理/发布/CYLUNE.app` and verify:

- black-hole project drop opens Quick Slice without an unsliced error;
- no output location exists;
- cancelling removes private artifacts;
- completing opens the print record with thumbnail and metrics;
- exactly one CYLUNE process and one published `CYLUNE.app` exist.

- [ ] **Step 7: Commit final verification adjustments**

```bash
git add src-tauri/src/slicer/runtime.rs
git commit -m "test: verify ephemeral slicing cleanup"
```

