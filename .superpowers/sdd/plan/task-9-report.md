# Task 9 report

## Outcome

Implemented and hardened the macOS local prototype shell: normal Dock window plus a separate 380×460 menu-bar popover, file intake, explicit non-recursive watch folder, localized local notifications, versioned privacy-filtered JSON backup/exact transactional restore, final app/template icons, packaging metadata, and installation notes.

## TDD evidence

- RED: `cargo test ... backup::tests` failed because `export_to_path` / `import_from_path` did not exist.
- GREEN: backup tests cover secret/path/raw G-code exclusion, exact snapshot restore, populated round trip + duplicate idempotency, schema validation, and invalid restore rollback.
- RED: `TrayDrop.test.tsx` failed because the component did not exist.
- GREEN: 2 component tests pass: mixed supported/unsupported drop and one-at-a-time import/open-job flow.
- RED→GREEN: standalone `.gcode` profile-safety test, pending-job database-reopen test, watcher extension/debounce/stability-retry tests.

## Implemented boundaries

- Backups contain schema version, spools, exact four slots, a typed sanitized parse representation, jobs, mappings, consumption, immutable ledger, and an explicit allowlist of settings. Watch paths, job/source filenames, parser unknown fields, raw G-code, tokens, and credentials are excluded.
- Restore writes an automatic pre-restore JSON beside the selected file, deeply validates schema/UUIDs/types/FKs/ledger relationships/balances, and replaces the business data with the exact snapshot in one SQLite transaction while preserving runtime-only settings.
- Watcher accepts only one canonical user-selected absolute directory, watches non-recursively, rejects files resolving outside it, persists/restarts the setting, clears a missing saved directory, filters supported extensions, debounces native duplicates, and retries files that are still being written.
- The main window consumes watcher events without overwriting an unsettled task. A second watched task is queued behind an explicit localized banner.
- Notification content is generic, follows the selected locale, and contains no filename, model, path, token, or credentials.
- `.gcode.3mf` and sliced `.3mf` use the parser. Standalone `.gcode` is accepted at the UI boundary but returns localized `standalone_gcode_profiles_required` before hashing, job creation, or inventory mutation.
- Tray left-click toggles and positions the popover below the icon; losing focus hides it. Main close hides rather than exits. Menu Open/Quit labels update on locale changes; explicit Quit terminates.
- Opening a tray job stores pending navigation in SQLite before showing/focusing main. The preview survives a process restart and the pending navigation is consumed once.
- Theme and locale changes synchronize between the main and menu-bar WebViews. Native menu labels, tooltip, and notifications use the persisted locale at launch.
- Backup/watch/settings actions are guarded against duplicate clicks and display localized retryable errors.
- Dialog permissions are limited to the main window.

## Verification

- `npm test -- --run`: 59/59 passed.
- `npm run build`: passed.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 72 passed, 0 failed, 1 opt-in real-file smoke ignored in the ordinary suite.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: passed.
- `cargo check --manifest-path src-tauri/Cargo.toml`: passed.
- Real read-only user fixture smoke with a temporary in-memory database: passed. It parsed 14 layers/four Bambu PLA Basic colors and verified success, idempotency, reversal, failed at layer 6, cancelled at layer 6, and estimated 50% (nearest layer 7). The source file was neither modified nor copied.
- Final app build passed after hardening: `src-tauri/target/release/bundle/macos/拓竹耗材管家.app`.
- DMG was attempted once. The native `bundle_dmg.sh` stopped producing output, so it was interrupted after a bounded wait; no valid final DMG is claimed. The `.app` remains intact.
- Bundled binary launched with `SPOOL_KEEPER_DATA_DIR` pointed at a clean temporary directory and remained running without stderr/crash for four seconds. The process was then stopped.

## Manual/native limitations

- The environment returned an all-black macOS screen capture and UI automation did not return accessibility data, so light/dark popover screenshots and automated tray-click/close/reopen assertions could not be captured. These native interactions are implemented and compile in the bundled application, but should receive a short hands-on pass on the user's Mac.
- The app is unsigned and not notarized; `docs/install-mac.md` documents the correct Gatekeeper path without making signing claims.
