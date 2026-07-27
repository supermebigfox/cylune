# Task 8 desktop workflow report

## Review fixes

- Native `.gcode.3mf` picker filter text now comes from the active 简中、繁中 or English locale resource; the dialog adapter no longer contains visible hard-coded English.
- During mapping or settlement mutations, mapping radios, outcome radios, the stopped-layer input, percentage slider, and submit controls are disabled. The synchronous global guard still prevents duplicate backend calls.
- Earlier review fixes remain in place: persisted AMS slot truth, mount/unmount/move refresh, explicit mapping confirmation, duplicate-job gate, stable localized errors, truthful gram display, dark-mode contrast, and 900×640 layout support.

## Verification

- Frontend: 49 tests passed across 9 files.
- Frontend production build: passed (`tsc && vite build`).
- Rust: 60 tests passed, 0 failed, 1 opt-in real-file smoke test ignored by the default suite.
- Rust formatting: passed (`cargo fmt --check`).
- Real-file smoke test previously passed with `萨莫面具-布莱克.gcode.3mf`: 14 layers, four filament tools, exact failed/cancelled layer consumption, idempotent settlement, and reversible deductions.

## TDD evidence for round 2

- Picker adapter test failed before the localized label argument existed, then passed after the adapter accepted and forwarded it.
- App integration test failed while the picker received no locale label, then passed after the active resource string was supplied.
- Busy-state tests failed while settlement controls remained editable, then passed after all relevant inputs were disabled.
