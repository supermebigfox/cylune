# Task 9 report

## Outcome

Implemented the final macOS local prototype shell: normal Dock window plus a separate 380×460 menu-bar popover, file intake, explicit non-recursive watch folder, local notification, versioned privacy-filtered JSON backup/transactional restore, final app/template icons, packaging metadata, and installation notes.

## TDD evidence

- RED: `cargo test ... backup::tests` failed because `export_to_path` / `import_from_path` did not exist.
- GREEN: 3 backup tests pass: secret/source-file exclusion, populated round trip + duplicate idempotency, invalid restore rollback.
- RED: `TrayDrop.test.tsx` failed because the component did not exist.
- GREEN: 2 component tests pass: mixed supported/unsupported drop and one-at-a-time import/open-job flow.
- RED→GREEN: standalone `.gcode` profile-safety test, pending-job reopen test, watcher extension/debounce tests.

## Implemented boundaries

- Backups contain schema version, spools, exact four slots, parse cache, jobs, mappings, consumption, immutable ledger, and an explicit allowlist of settings. Job/source filenames are redacted; source files are never scanned or embedded. Token/password/secret/credential keys are denied.
- Restore writes an automatic pre-restore JSON beside the selected file, validates schema/UUIDs/types/FKs/ledger balances, restores in one SQLite transaction, and deduplicates IDs, idempotency keys, and source hashes.
- Watcher accepts only one user-selected absolute directory, watches non-recursively, persists/restarts the setting, safely replaces the prior watcher only after the new watcher starts, filters supported extensions, debounces native duplicates, and calls the existing two-stat 750 ms import service.
- Notification content is generic (`Task awaiting settlement`) and contains no filename, model, path, token, or credentials.
- `.gcode.3mf` and sliced `.3mf` use the parser. Standalone `.gcode` is accepted at the UI boundary but returns localized `standalone_gcode_profiles_required` before hashing, job creation, or inventory mutation.
- Tray left-click toggles and positions the popover below the icon; losing focus hides it. Main close hides rather than exits. Menu Open/Quit labels update on locale changes; explicit Quit terminates.
- Opening a tray job stores pending navigation, shows/focuses main, emits an event, and main rehydrates the preview from SQLite.

## Verification

- `npm test -- --run`: 51/51 passed.
- `npm run build`: passed.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 68 passed, 0 failed, 1 opt-in real-file smoke ignored in the ordinary suite (67-pass full run plus the final unknown-field backup test run focused after RED).
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: passed.
- `cargo check --manifest-path src-tauri/Cargo.toml`: passed.
- Real read-only user fixture smoke with a temporary in-memory database: passed. It parsed 14 layers/four Bambu PLA Basic colors and verified success, idempotency, reversal, failed at layer 6, cancelled at layer 6, and estimated 50% (nearest layer 7). The source file was neither modified nor copied.
- Final app build passed: `src-tauri/target/release/bundle/macos/拓竹耗材管家.app` (about 14 MB).
- DMG was attempted once. The native `bundle_dmg.sh` stopped producing output, so it was interrupted after a bounded wait; no valid final DMG is claimed. The `.app` remains intact.
- Bundled binary launched with `SPOOL_KEEPER_DATA_DIR` pointed at a clean temporary directory and remained running without stderr/crash. The process was then stopped and temporary data removed.

## Manual/native limitations

- The environment returned an all-black macOS screen capture and UI automation did not return accessibility data, so light/dark popover screenshots and automated tray-click/close/reopen assertions could not be captured. These native interactions are implemented and compile in the bundled application, but should receive a short hands-on pass on the user's Mac.
- The app is unsigned and not notarized; `docs/install-mac.md` documents the correct Gatekeeper path without making signing claims.
