# Project-Native Slicing and Progress Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make CYLUNE slice with the selected printer hardware while preserving every process and filament setting embedded in the 3MF, and show real 0–100% slicing progress on macOS.

**Architecture:** Replace the current “quick overrides” request with a narrow request containing only the input, saved printer, and mismatch confirmation. Resolve and load only the target machine profile; leave process and filament settings inside the 3MF at their native Bambu priority. Stream Bambu’s macOS debug callback through a focused parser that reproduces Bambu’s own multi-plate total-progress formula, then publish monotonic Tauri progress events.

**Tech Stack:** Rust 2021, Tauri 2, React 18, TypeScript, Vitest/Testing Library, Bambu Studio 2.8 CLI.

---

## File map

- `src-tauri/src/slicer/command.rs`: build a machine-only Bambu command; no process, filament, or fast-override materialization.
- `src-tauri/src/slicer/catalog.rs`: safely resolve exactly one official machine profile for a saved printer.
- `src-tauri/src/slicer/mod.rs`: expose the reduced Tauri request and enforce machine-mismatch confirmation.
- `src-tauri/src/slicer/progress.rs`: parse streamed macOS Bambu callback lines and calculate monotonic total progress.
- `src-tauri/src/slicer/runtime.rs`: stream stdout into the progress parser, update task state/events, preserve cancellation and cleanup.
- `src-tauri/src/error.rs`: classify Bambu process incompatibility separately from generic CLI failures.
- `src/lib/tauri.ts`: mirror the reduced request contract.
- `src/features/slice/Slice.tsx`: remove editable slicing controls; show read-only 3MF and selected-printer summaries plus mismatch confirmation.
- `src/features/slice/Slice.css`: style the read-only summary and determinate progress bar.
- `src/features/slice/Slice.test.tsx`, `src/lib/tauri.test.ts`, Rust module tests: protect the new behavior.
- `src/i18n/locales/{zh-CN,zh-TW,en}.json`, `src/i18n/i18n.test.ts`: stable process-incompatibility message.

### Task 1: Reduce the slicing contract to machine selection only

**Files:**
- Modify: `src-tauri/src/slicer/command.rs`
- Modify: `src-tauri/src/slicer/catalog.rs`
- Modify: `src-tauri/src/slicer/mod.rs`
- Modify: `src/lib/tauri.ts`
- Test: `src-tauri/src/slicer/command.rs`
- Test: `src-tauri/src/slicer/catalog.rs`
- Test: `src-tauri/src/slicer/mod.rs`
- Test: `src/lib/tauri.test.ts`

- [ ] **Step 1: Write failing Rust command tests**

Add tests that construct a request with only `machine_settings` and assert the exact argument boundary:

```rust
#[test]
fn loads_only_the_target_machine_and_leaves_3mf_process_and_filaments_native() {
    let fixture = Fixture::new();
    let args = build_bambu_args(&fixture.project_native_request(), &fixture.temporary_output)
        .unwrap();
    assert!(args.windows(2).any(|pair| {
        pair[0] == "--load-settings" && pair[1] == fixture.machine.as_os_str()
    }));
    assert!(!args.iter().any(|arg| arg == "--load-filaments"));
    assert!(!args.iter().any(|arg| {
        let value = arg.to_string_lossy();
        value.contains("effective-process") || value.contains("effective-filament")
    }));
}
```

- [ ] **Step 2: Run the focused Rust tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml slicer::command::tests::loads_only_the_target_machine_and_leaves_3mf_process_and_filaments_native -- --exact`

Expected: FAIL because `SliceRequest` still requires process/filament/override fields and `build_bambu_args` emits both high-priority profile options.

- [ ] **Step 3: Write failing request-resolution tests**

Change the expected public request to:

```rust
FastSliceRequest {
    input_path: fixture.input.clone(),
    printer_id: fixture.printer().printer_id,
    confirm_printer_mismatch: false,
}
```

Assert that the resolved request contains the canonical target machine path, sets `estimate_mode` only on a confirmed machine mismatch, and contains no process/filament override fields.

- [ ] **Step 4: Implement the reduced Rust request and machine resolver**

Define the internal request as:

```rust
pub struct SliceRequest {
    pub printer: SavedPrinter,
    pub input: PathBuf,
    pub plate_selection: PlateSelection,
    pub estimate_mode: bool,
    pub machine_settings: PathBuf,
}
```

Add `resolve_machine_path(profiles_root, printer) -> Result<PathBuf>` in `catalog.rs`, reusing `load_catalog_data` so path canonicalization, symlink rejection, exact model matching, and nozzle matching stay identical to catalog loading. Make `build_bambu_args` emit `--load-settings <machine-path>` but neither `--load-filaments` nor a process profile.

- [ ] **Step 5: Reduce the TypeScript request contract and its bridge test**

Use:

```ts
export interface SliceStartRequest {
  input_path: string;
  printer_id: string;
  confirm_printer_mismatch: boolean;
}
```

Update `src/lib/tauri.test.ts` so `startSlice` is expected to invoke Tauri with only those three fields.

- [ ] **Step 6: Run focused Rust and TypeScript tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml slicer::command
cargo test --manifest-path src-tauri/Cargo.toml slicer::task6_tests
npm test -- --run src/lib/tauri.test.ts
```

Expected: all focused tests PASS.

- [ ] **Step 7: Commit the contract change**

```bash
git add src-tauri/src/slicer/command.rs src-tauri/src/slicer/catalog.rs src-tauri/src/slicer/mod.rs src/lib/tauri.ts src/lib/tauri.test.ts
git commit -m "refactor: preserve native 3mf slicing settings"
```

### Task 2: Make the slicing page read-only except for machine mismatch confirmation

**Files:**
- Modify: `src/features/slice/Slice.tsx`
- Modify: `src/features/slice/Slice.css`
- Test: `src/features/slice/Slice.test.tsx`

- [ ] **Step 1: Replace the editable-form test with failing one-click tests**

Add assertions that the process, plate, infill, support, filament, and printer comboboxes do not exist; the selected default printer and embedded values are visible; and submission is exact:

```ts
expect(screen.queryByRole("combobox", { name: "工艺与层高" })).not.toBeInTheDocument();
expect(screen.queryByRole("spinbutton", { name: "填充率" })).not.toBeInTheDocument();
expect(screen.queryByRole("checkbox", { name: "生成支撑" })).not.toBeInTheDocument();
await user.click(screen.getByRole("button", { name: "开始后台切片" }));
expect(client.startSlice).toHaveBeenCalledWith({
  input_path: "/Users/robin/Desktop/月球灯.3mf",
  printer_id: "printer-p2s",
  confirm_printer_mismatch: false,
});
```

Keep a separate test proving an A1/X2D project disables the start button until “确认改用我的 P2S” is checked.

- [ ] **Step 2: Run the slice component test and verify RED**

Run: `npm test -- --run src/features/slice/Slice.test.tsx`

Expected: FAIL because the current page renders and submits manual quick overrides.

- [ ] **Step 3: Remove override state and preset loading from `Slice.tsx`**

Delete `catalog`, `processKey`, `plateKey`, `infill`, `support`, filament selection, touched flags, `presetForTool`, and the `listSlicePresets` effect. Keep `defaultPrinter`, the saved-printer refresh, mismatch calculation/confirmation, inspection, cancellation, task restoration, and sliced-file routing.

Make validity depend only on an unsliced input, an available selected default printer, and mismatch confirmation when required.

- [ ] **Step 4: Render a read-only project summary**

Render concise rows for target printer/nozzle, embedded process, plate, infill, support, and each embedded color/material when present. Do not manufacture values when the 3MF omitted them; display the localized “3MF 内嵌” fallback.

- [ ] **Step 5: Update copy and CSS without changing the global visual system**

Reuse the existing rounded cards, spacing, colors, and liquid-glass navigation. Add only focused `.slice-summary`, `.slice-summary-row`, and `.slice-tool-readonly` styles. Update retry/cancel copy so it no longer says settings can be edited in CYLUNE.

- [ ] **Step 6: Run focused frontend tests and verify GREEN**

Run: `npm test -- --run src/features/slice/Slice.test.tsx src/App.test.tsx`

Expected: all slice and global-navigation tests PASS.

- [ ] **Step 7: Commit the read-only slicing page**

```bash
git add src/features/slice/Slice.tsx src/features/slice/Slice.css src/features/slice/Slice.test.tsx src/App.test.tsx
git commit -m "feat: make project slicing one click"
```

### Task 3: Stream real Bambu progress on macOS

**Files:**
- Create: `src-tauri/src/slicer/progress.rs`
- Modify: `src-tauri/src/slicer/mod.rs`
- Modify: `src-tauri/src/slicer/command.rs`
- Modify: `src-tauri/src/slicer/runtime.rs`
- Test: `src-tauri/src/slicer/progress.rs`
- Test: `src-tauri/src/slicer/runtime.rs`

- [ ] **Step 1: Write failing parser tests from Bambu’s actual log format**

Cover single-plate and multi-plate lines:

```rust
#[test]
fn maps_bambu_callback_to_its_total_progress_formula() {
    let mut parser = BambuProgressParser::default();
    parser.observe("Need to slice for plate 0, total plate count 2 partplates!");
    parser.observe("start Print::process for partplate 1");
    assert_eq!(parser.observe("default_status_callback: percent=50, warning_step=-1, message=Generating infill, message_type=0"), Some(25.5));
    parser.observe("start Print::process for partplate 2");
    assert_eq!(parser.observe("default_status_callback: percent=50, warning_step=-1, message=Generating infill, message_type=0"), Some(70.5));
}
```

Also test malformed lines, warnings, repeated/decreasing callback values, clamping, and `will export 3mf` advancing to `97`.

- [ ] **Step 2: Run the parser test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml slicer::progress -- --nocapture`

Expected: FAIL because `progress.rs` and `BambuProgressParser` do not exist.

- [ ] **Step 3: Implement the focused stateful parser**

Create `BambuProgressParser` with `plate_count`, `plate_index`, and `last_percent`. Parse only known Bambu messages. For callback progress, reproduce upstream’s formula:

```rust
let total = if self.plate_count <= 1 {
    3.0 + 0.9 * plate_percent
} else {
    3.0
        + ((self.plate_index.saturating_sub(1) as f64) * 90.0) / self.plate_count as f64
        + (plate_percent * 0.9) / self.plate_count as f64
};
```

Clamp to `0.0..=97.0` before result validation/import; ignore values below `last_percent`.

- [ ] **Step 4: Add failing runtime progress-event tests**

Change the fake Bambu executable to print realistic callback lines with short flushable pauses. Assert `RecordingEvents` receives numeric monotonic progress, task snapshots retain the latest percent, cancel stops later updates, validation emits at least `98`, import emits `99`, and completion emits `100`.

- [ ] **Step 5: Run runtime tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml slicer::runtime::tests -- --nocapture`

Expected: FAIL because stdout is currently discarded and all progress events contain `None`.

- [ ] **Step 6: Stream stdout into the parser and task store**

Change the CLI debug level to `4`. Pass `Arc<SlicerInner>` and `task_id` into the stdout reader. For every line, call `BambuProgressParser::observe`; when it returns a number, update both `SliceTask.percent` and the `slice-progress` event. Keep stderr bounded and never persist or expose raw output.

Use determinate lifecycle values: task creation `0`, preparation `1–3`, real Bambu slicing `3–97`, validation `98`, import `99`, completed `100`.

- [ ] **Step 7: Run parser/runtime tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml slicer::progress
cargo test --manifest-path src-tauri/Cargo.toml slicer::runtime
```

Expected: all tests PASS with monotonic numeric progress.

- [ ] **Step 8: Commit real macOS progress**

```bash
git add src-tauri/src/slicer/progress.rs src-tauri/src/slicer/mod.rs src-tauri/src/slicer/command.rs src-tauri/src/slicer/runtime.rs
git commit -m "feat: stream real bambu slicing progress"
```

### Task 4: Make the frontend progress always determinate

**Files:**
- Modify: `src/features/slice/Slice.tsx`
- Modify: `src/features/slice/Slice.css`
- Modify: `src/App.tsx`
- Test: `src/features/slice/Slice.test.tsx`
- Test: `src/App.test.tsx`

- [ ] **Step 1: Write failing determinate-progress tests**

Assert the initial running view renders `0%` and `<progress value="0" max="100">`; numeric events update both the bar and the navigation badge; validation/import no longer make the bar indeterminate; completion is `100%`; stale or decreasing events do not move the UI backward.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `npm test -- --run src/features/slice/Slice.test.tsx src/App.test.tsx`

Expected: FAIL because current code removes the `value` attribute for null phase progress and accepts decreasing events.

- [ ] **Step 3: Normalize frontend progress**

Store a numeric percentage for every active task. Normalize incoming values to `0..100` and use `Math.max(current, incoming)`. Always render:

```tsx
<b className="data">{copy("percent", { percent: Math.round(progress.percent) })}</b>
<progress aria-label={copy("progressLabel")} max={100} value={progress.percent} />
```

Keep phase text only as secondary accessible status, not as the main progress indicator.

- [ ] **Step 4: Remove indeterminate progress styling**

Delete `.slice-progress-card progress:indeterminate` animation and preserve the existing rounded colored fill.

- [ ] **Step 5: Run focused frontend tests and verify GREEN**

Run: `npm test -- --run src/features/slice/Slice.test.tsx src/App.test.tsx src/features/nav/Nav.test.tsx`

Expected: all tests PASS.

- [ ] **Step 6: Commit determinate UI progress**

```bash
git add src/features/slice/Slice.tsx src/features/slice/Slice.css src/App.tsx src/features/slice/Slice.test.tsx src/App.test.tsx src/features/nav/Nav.test.tsx
git commit -m "feat: show determinate slice progress"
```

### Task 5: Classify incompatible embedded processes

**Files:**
- Modify: `src-tauri/src/error.rs`
- Modify: `src-tauri/src/slicer/runtime.rs`
- Modify: `src/i18n/locales/zh-CN.json`
- Modify: `src/i18n/locales/zh-TW.json`
- Modify: `src/i18n/locales/en.json`
- Modify: `src/i18n/i18n.test.ts`
- Test: `src-tauri/src/slicer/runtime.rs`

- [ ] **Step 1: Write a failing `return_code = -17` classification test**

Write a private `result.json` containing `{"return_code":-17,"error_string":"untrusted details"}` and assert `classify_slicer_failure` returns a new stable `AppError::SlicerProcessIncompatible` without surfacing `error_string`.

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml classifies_bambu_process_incompatibility -- --exact`

Expected: FAIL because `-17` currently maps to `slicer_failed`.

- [ ] **Step 3: Implement the stable error code and translations**

Map `-17` to `slicer_process_incompatible`. Add concise messages explaining that the 3MF’s embedded process cannot be used with the selected target machine, in simplified Chinese, traditional Chinese, and English. Add the key to the i18n completeness test.

- [ ] **Step 4: Run Rust and i18n tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml classifies_bambu_process_incompatibility
npm test -- --run src/i18n/i18n.test.ts
```

Expected: all tests PASS.

- [ ] **Step 5: Commit incompatibility handling**

```bash
git add src-tauri/src/error.rs src-tauri/src/slicer/runtime.rs src/i18n/locales/zh-CN.json src/i18n/locales/zh-TW.json src/i18n/locales/en.json src/i18n/i18n.test.ts
git commit -m "fix: explain incompatible project processes"
```

### Task 6: Real-file regression, full verification, and one final package

**Files:**
- Modify only if a failing regression identifies an in-scope defect.
- Never modify, delete, stage, or commit root `result.json`.

- [ ] **Step 1: Run the custom-process real-file smoke test**

Run the ignored integration test with:

```bash
CYLUNE_SLICE_INPUT_3MF='/Users/robin/Desktop/叠色/曼尼面具-第九副面具无人佩戴.3mf' \
CYLUNE_EXPECTED_SOURCE_SHA256='cc2247dee3c9bfa2ef27d084684042d6f1eb19ae541736f5c3a5b3ffc0221161' \
CYLUNE_EXPECTED_PLATE_COUNT='1' \
cargo test --manifest-path src-tauri/Cargo.toml smoke_real_slice_validates_output_then_imports_one_project -- --ignored --exact --nocapture
```

Expected: PASS; source hash and mtime unchanged; private temporary output removed.

- [ ] **Step 2: Re-run the known metric fixtures**

Slice Poopy and wind-tunnel projects through the service path, time each run, and assert the parsed values remain approximately `221.52g / 891 layers / 17,243 seconds` for Poopy and `782.07g` total across all wind-tunnel plates. Record elapsed slicing durations in the handoff.

- [ ] **Step 3: Run all automated tests and production frontend build**

Run:

```bash
npm test -- --run
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all frontend and Rust tests PASS; TypeScript/Vite build succeeds.

- [ ] **Step 4: Inspect the worktree before packaging**

Run: `git status --short`

Expected: only the intentionally untracked root `result.json`; no generated test or temporary slicing artifacts.

- [ ] **Step 5: Build the final app once**

Run: `npm run release:mac`

Expected: `/Users/robin/Desktop/耗材管理/发布/CYLUNE.app` exists and the release directory contains no duplicate application.

- [ ] **Step 6: Verify the app signature and launchability**

Run:

```bash
codesign --verify --deep --strict '/Users/robin/Desktop/耗材管理/发布/CYLUNE.app'
open '/Users/robin/Desktop/耗材管理/发布/CYLUNE.app'
```

Expected: signature verification succeeds and one CYLUNE instance launches.

- [ ] **Step 7: Commit any final test-only adjustments**

Stage only source/tests/docs changed for this feature; explicitly exclude `result.json`.

```bash
git status --short
git commit -m "test: verify native project slicing"
```
