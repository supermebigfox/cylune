# BlackHoleTrash Safe Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver complete `Gargantua` / `Fusion` black-hole appearances from 120–900 px, including desktop distortion and every swallow/failure/fallback state, while converting the upstream delete action into an acknowledged, source-preserving print-file import.

**Architecture:** Keep the existing Tauri/Rust business services, two-window AppKit hit-target architecture, ScreenCaptureKit-to-IOSurface path, Metal renderer, settings, pending-job state, and settlement ledger. Tasks 1–2 already established exact capture mapping and the pinned Schwarzschild/Gargantua path. The remaining work expands logical sizing while capping the internal drawable, adds Fusion as a material branch inside the existing 152-byte uniform ABI, completes the independently tested faller/drop acknowledgment path, and admits a user preview only after the complete Real/Lite, permission, motion, failure/cancel, dual-style, and large-size matrix passes.

**Tech Stack:** Tauri 2, Rust, SQLite, Objective-C++17, AppKit, ScreenCaptureKit, Metal/MetalKit, C ABI, React/Vitest, Rust tests, standalone C++ assertions.

## Global Constraints

- Approved design: `docs/superpowers/specs/2026-07-28-black-hole-import-design.md`.
- Normative optical source: BlackHoleTrash commit `229d93213cd3e57364b4c6655cfb2c75b7ea4d18`; `Gargantua` remains its unchanged default.
- macOS interaction/animation source: blackhole-mac commit `f719aa1139ecc49a728cbb8fac2e60fcfa51996e`; it does not replace BlackHoleTrash as the optical reference.
- Fusion material/parameter source: blackhole-timer commit `f3cc9cc349540ad6d274cd8074cf050b9b0c0200`; Fusion reuses the BlackHoleTrash trace and is not a second optical implementation.
- The current cyan/violet/rose `spectral_ring` and reciprocal-radius lens are replacement targets, not compatibility requirements.
- `pet_size` remains the visual-panel side and stays within `120..=900` logical pixels; presets are exactly `300/600/900`, and legacy `360` is an explicit 40%-of-maximum regression point.
- The shadow radius is `0.075 × pet_size`; the circular hit radius is `max(22 px, 1.15 × shadow_radius)`.
- Real mode always captures a square of side `1.60 × pet_size`; edge clipping must preserve panel-to-capture UV alignment even above 360.
- The Metal internal drawable logical side is `min(pet_size, 360)`; the AppKit panel, hit geometry, capture geometry, persisted size, and user-facing size remain `pet_size`.
- `PetRenderUniforms` remains exactly 152 bytes. `visual_style` reuses the first existing trailing padding word at offset 140, and `PetConfig` reuses its 64-byte trailing padding; neither ABI may grow.
- `Gargantua = 0` and `Fusion = 1` use the same trace, capture UV, event horizon, animation state, pending draw, Lite/Real path, and hit geometry.
- Only an external file URL released inside the circular core can request an import.
- Each drag accepts exactly the first supported regular non-symlink file in pasteboard order.
- Accepted extensions remain `.gcode.3mf`, `.3mf`, and `.gcode`; `PrintService` remains the final format validator.
- A successful `performDragOperation` only means the request was submitted. The card cannot cross the horizon until Rust acknowledges the same generation after task persistence.
- The source file must never be deleted, trashed, moved, renamed, overwritten, or written.
- Import cannot change spool balances; existing settlement remains the only deduction path.
- Standard motion uses a 4.6-second faller, crossing at `u = 0.82`, plus a 0.90-second BlackHoleTrash absorption jet; the visual state returns to Idle at 4.672 seconds.
- Reduce Motion uses one 0.15-second fade/pulse with no orbit, shear, fragments, jet, or shockwave.
- Auto FPS is 30 in Idle, 60 during hover/import/swallow/reject/settlement, and 0 while hidden; fixed 30/60 behavior remains unchanged.
- Lite mode keeps the selected Gargantua/Fusion hole, disk, file card, swallow, error,
  settlement, and pending indicators without capturing desktop pixels.
- Absent, denied, revoked, restart-required, or failed screen recording automatically makes
  Lite the effective mode.
- Metal failure keeps the existing Core Animation fallback and safe import behavior.
- Screen frames, file paths, thumbnails, and file contents must not enter JavaScript, SQLite, ordinary logs, or network traffic.
- The user fixture `/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf` is read only through `BAMBU_SMOKE_3MF`; never copy or commit it.
- Add complete MIT attribution for GreenScreen410 and Jack Zhang before release.
- Add active Fusion attribution for s13k/blackhole-timer at the pinned commit before release.
- Every task starts red, implements the minimum scoped behavior, ends green, and receives an independent commit.
- Tasks 1 and 2 are already complete and must not be rewritten or replayed.
- Do not present another user preview until Tasks 3–6 are green and the complete preview matrix has recorded Real desktop distortion, 4.6 s swallow, 0.90 s jet, failure, cancellation, Reduce Motion, Lite/Real, both styles, 600/900 sizes, and screen-recording authorization/restart behavior.

## File Map

### Geometry, capture, and native state

- `src-tauri/native/mac/bridge.h`: C ABI structs, event-specific callback value, drop-result constants, renderer uniforms.
- `src-tauri/native/mac/pet_lifecycle.h`: panel/capture geometry, circular hit test, display-edge mapping.
- `src-tauri/native/mac/pet_drop_state.h`: create; generation-aware hover/import/faller/reject state with deterministic sampling.
- `src-tauri/native/mac/pet_lifecycle_test.cc`: pure C++ geometry, animation, stale-generation, FPS, and lifecycle tests.
- `src-tauri/native/mac/pet_render_state.h`: renderer activity and non-drop settlement timing.
- `src-tauri/native/mac/pet_visual_state.h`: pending dots and settlement signal mapping.
- `src-tauri/native/mac/pet.mm`: AppKit panels, circular mouse/drop hit testing, pasteboard validation, drop sessions, CA fallback, uniform assembly.
- `src-tauri/native/mac/capture.mm`: keep ScreenCaptureKit lifecycle; consume the expanded/clipped region from `pet.mm`.
- `src-tauri/native/mac/render.mm`: Metal pipelines, uniform ABI checks, synthetic render harness.
- `src-tauri/native/mac/shader.metal`: BlackHoleTrash optical port, procedural card/faller/fragments, jet and Lite compositing.
- `src-tauri/build.rs`: rerun when the new state header changes.
- `src-tauri/src/pet/mod.rs`: `PetVisualStyle` and the expanded 120–900 setting contract.
- `src-tauri/src/pet/store.rs`: validate/persist the expanded size and visual style without adding either to business backup.
- `src/features/settings/Pet.tsx`: exact 300/600/900 presets and Gargantua/Fusion selector.
- `src/features/settings/Pet.test.tsx`: settings boundary, preset, style, and permission fallback UI tests.
- `src/lib/tauri.ts`: matching TypeScript pet setting/style contract.
- `src/i18n/locales/{zh-CN,zh-TW,en}.json`: user-visible style and permission/restart labels.

### Rust acknowledgment and file safety

- `src-tauri/src/pet/native.rs`: matching C ABI layouts, callback generation, `finish_drop`, render test options.
- `src-tauri/src/pet/input.rs`: regular/non-symlink path validation and first-supported selection.
- `src-tauri/src/pet/runtime.rs`: generation-carrying native event, import acknowledgment, pending-state refresh.
- `src-tauri/src/imports.rs`: stable non-symlink read before task transaction commit.
- `src-tauri/src/error.rs`: keep the existing `file_not_stable` and `invalid_file` codes.

### Verification and release

- `THIRD_PARTY_NOTICES.md`: pinned BlackHoleTrash and blackhole-mac MIT notices and modification descriptions.
- `docs/install-mac.md`: safe-import and Reduce Motion behavior.
- `docs/qa-black-hole.md`: exact BlackHoleTrash visual, drag safety, display-edge, FPS, Lite, permission, and source-preservation matrix.
- `src-tauri/src/settlement.rs`: extend the existing ignored real-file smoke only if an assertion required by this spec is absent.

---

## Task dependencies and preview gate

```text
Task 1 capture geometry ─┐
                         ├─> Task 3 size/style ABI ─> Task 4 complete animation ─┐
Task 2 Gargantua port ───┘                                                       ├─> Task 6 complete preview gate ─> Task 7 release evidence
Task 1 capture geometry ────────────────> Task 5 safe acknowledgment ────────────┘
Task 4 animation state ─────────────────> Task 5 safe acknowledgment
```

- Tasks 1 and 2 are complete at commits `99435e5` and `28bc82b`; their sections below remain
  unchanged as execution evidence.
- Task 3 depends on Tasks 1–2 and establishes the expanded settings, capped drawable, and dual
  style ABI that every later visual test consumes.
- Task 4 depends on Task 3 and completes every visual success/reject/cancel/Reduce Motion state
  for both styles and both render modes.
- Task 5 depends on Tasks 1 and 4 and connects those states to generation-safe Rust persistence.
- Task 6 depends on Tasks 3–5 and is the only task allowed to produce the next user preview.
- Task 7 depends on Task 6 and records final release, licensing, documentation, and full-suite
  evidence.

### Task 1: Capture geometry, edge mapping, and circular hit target

**Files:**
- Modify: `src-tauri/native/mac/bridge.h`
- Modify: `src-tauri/native/mac/pet_lifecycle.h`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/render.mm`
- Modify: `src-tauri/src/pet/native.rs`

**Interfaces:**
- Consumes: existing `PetPanelFrame`, `PetScreenFrame`, `PetCaptureRegion`, `PetConfig.size`, visual/core child-window lifecycle.
- Produces: `PetEffectGeometry PetEffectGeometryForSize(double size)`.
- Produces: `bool PetPointInsideCore(double x, double y, PetEffectGeometry geometry)`.
- Produces: `PetCaptureRegion PetCaptureRegionForPanel(PetPanelFrame panel, PetScreenFrame display)` with `panel_origin_uv[2]` and `panel_extent_uv[2]`.
- Produces: `PetRenderUniforms.capture_origin_uv` and `PetRenderUniforms.capture_extent_uv` for Task 2.

- [ ] **Step 1: Replace the old geometry assertions with failing exact geometry and capture-mapping tests**

In `pet_lifecycle_test.cc`, replace the two assumptions that the core target is the
inscribed square with these tests and add the edge cases:

```cpp
static bool close_to(double lhs, double rhs, double epsilon = 1e-6) {
  return fabs(lhs - rhs) <= epsilon;
}

static void approved_geometry_uses_a_small_circular_core() {
  const PetEffectGeometry small = PetEffectGeometryForSize(120.0);
  const PetEffectGeometry medium = PetEffectGeometryForSize(220.0);
  const PetEffectGeometry large = PetEffectGeometryForSize(360.0);
  assert(close_to(small.shadow_radius, 9.0));
  assert(close_to(medium.shadow_radius, 16.5));
  assert(close_to(large.shadow_radius, 27.0));
  assert(close_to(small.hit_radius, 22.0));
  assert(close_to(medium.hit_radius, 22.0));
  assert(close_to(large.hit_radius, 31.05));
  assert(PetPointInsideCore(60.0, 60.0, small));
  assert(PetPointInsideCore(81.9, 60.0, small));
  assert(!PetPointInsideCore(82.1, 60.0, small));
  assert(!PetPointInsideCore(0.0, 0.0, small));
}

static void centered_capture_maps_the_panel_into_the_middle_five_eighths() {
  const PetPanelFrame panel = {100.0, 300.0, 220.0, 220.0};
  const PetScreenFrame display = {0.0, 0.0, 1440.0, 900.0, 2.0, 42};
  const PetCaptureRegion region = PetCaptureRegionForPanel(panel, display);
  assert(close_to(region.source_x, 34.0));
  assert(close_to(region.source_y, 314.0));
  assert(close_to(region.source_width, 352.0));
  assert(close_to(region.source_height, 352.0));
  assert(region.pixel_width == 704);
  assert(region.pixel_height == 704);
  assert(close_to(region.panel_origin_uv[0], 0.1875));
  assert(close_to(region.panel_origin_uv[1], 0.1875));
  assert(close_to(region.panel_extent_uv[0], 0.625));
  assert(close_to(region.panel_extent_uv[1], 0.625));
}

static void left_edge_capture_does_not_stretch_the_desktop() {
  const PetPanelFrame panel = {0.0, 300.0, 220.0, 220.0};
  const PetScreenFrame display = {0.0, 0.0, 1440.0, 900.0, 2.0, 42};
  const PetCaptureRegion region = PetCaptureRegionForPanel(panel, display);
  assert(close_to(region.source_x, 0.0));
  assert(close_to(region.source_width, 286.0));
  assert(close_to(region.panel_origin_uv[0], 0.0));
  assert(close_to(region.panel_extent_uv[0], 220.0 / 286.0));
  assert(close_to(region.panel_origin_uv[1], 66.0 / 352.0));
  assert(close_to(region.panel_extent_uv[1], 220.0 / 352.0));
}
```

Call all three functions from `main()`. Add equivalent right, top, and bottom tests by
using these exact cases:

```cpp
const PetCaptureRegion right = PetCaptureRegionForPanel(
    {1220.0, 300.0, 220.0, 220.0}, display);
assert(close_to(right.source_x, 1154.0));
assert(close_to(right.source_width, 286.0));
assert(close_to(right.panel_origin_uv[0], 66.0 / 286.0));
assert(close_to(right.panel_extent_uv[0], 220.0 / 286.0));

const PetCaptureRegion top = PetCaptureRegionForPanel(
    {610.0, 680.0, 220.0, 220.0}, display);
assert(close_to(top.source_y, 0.0));
assert(close_to(top.source_height, 286.0));
assert(close_to(top.panel_origin_uv[1], 0.0));
assert(close_to(top.panel_extent_uv[1], 220.0 / 286.0));

const PetCaptureRegion bottom = PetCaptureRegionForPanel(
    {610.0, 0.0, 220.0, 220.0}, display);
assert(close_to(bottom.source_y, 614.0));
assert(close_to(bottom.source_height, 286.0));
assert(close_to(bottom.panel_origin_uv[1], 66.0 / 286.0));
assert(close_to(bottom.panel_extent_uv[1], 220.0 / 286.0));
```

- [ ] **Step 2: Run the native test and verify the red state**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
/tmp/bambu-pet-native-test
```

Expected: compilation fails because `PetEffectGeometry`,
`PetEffectGeometryForSize`, `PetPointInsideCore`, and the UV fields do not exist.

- [ ] **Step 3: Add the exact capture fields and pure geometry functions**

Extend `PetCaptureRegion` in `bridge.h`:

```c
typedef struct {
  uint32_t display_id;
  double source_x;
  double source_y;
  double source_width;
  double source_height;
  uint32_t pixel_width;
  uint32_t pixel_height;
  float panel_origin_uv[2];
  float panel_extent_uv[2];
} PetCaptureRegion;
```

Replace `PetEventHorizonGeometryForEffectDiameter` with these pure functions in
`pet_lifecycle.h`:

```cpp
struct PetEffectGeometry {
  double panel_side;
  double shadow_radius;
  double hit_radius;
};

inline PetEffectGeometry PetEffectGeometryForSize(double size) {
  const double side = std::max(0.0, size);
  const double shadow = side * 0.075;
  return {side, shadow, std::max(22.0, shadow * 1.15)};
}

inline bool PetPointInsideCore(double x, double y,
                               PetEffectGeometry geometry) {
  const double dx = x - geometry.panel_side * 0.5;
  const double dy = y - geometry.panel_side * 0.5;
  return std::isfinite(dx) && std::isfinite(dy) &&
         std::hypot(dx, dy) <= geometry.hit_radius;
}
```

Rewrite `PetCaptureRegionForPanel` with `requested_side = panel.width * 1.60`.
Compute the panel origin in ScreenCaptureKit top-left coordinates, divide the panel offset and
size by the clipped capture width/height, and return zero UV values only when a dimension is
zero.

- [ ] **Step 4: Pass capture mapping to every render without changing capture lifecycle**

Store the last configured `PetCaptureRegion` on `BPPetHost`, expose it to `BPPetView`, and fill
the uniforms exactly:

```objc
uniforms.capture_origin_uv[0] = _captureRegion.panel_origin_uv[0];
uniforms.capture_origin_uv[1] = _captureRegion.panel_origin_uv[1];
uniforms.capture_extent_uv[0] = _captureRegion.panel_extent_uv[0];
uniforms.capture_extent_uv[1] = _captureRegion.panel_extent_uv[1];
```

For Lite mode or a missing capture, use `{0, 0}` and `{1, 1}`. Update the matching test-only
Rust uniform layout in `native.rs` and the `static_assert` offsets in `render.mm`.

In `BPCoreHitTargetView`, use the pure circle calculation for mouse and drag locations.
Return `nil`/`NSDragOperationNone` outside the circle even when the point lies inside the
square child panel. Keep the visual panel click-through and keep the child-window lifecycle
unchanged.

- [ ] **Step 5: Run focused native and Rust ABI tests**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
/tmp/bambu-pet-native-test
cd src-tauri
cargo test pet::native::tests -- --nocapture
```

Expected: all native assertions and Rust ABI/layout tests pass.

- [ ] **Step 6: Commit the independently testable geometry change**

```bash
git add src-tauri/native/mac/bridge.h \
  src-tauri/native/mac/pet_lifecycle.h \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  src-tauri/native/mac/pet.mm \
  src-tauri/native/mac/render.mm \
  src-tauri/src/pet/native.rs
git commit -m "fix: align black hole capture geometry"
```

### Task 2: Port the pinned BlackHoleTrash Gargantua renderer to Metal

**Files:**
- Modify: `src-tauri/native/mac/bridge.h`
- Modify: `src-tauri/native/mac/shader.metal`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/render.mm`
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `THIRD_PARTY_NOTICES.md`

**Interfaces:**
- Consumes: Task 1 `capture_origin_uv`, `capture_extent_uv`, `PetEffectGeometry.shadow_radius`.
- Produces: a 152-byte `PetRenderUniforms`/`PetUniforms` ABI with named Gargantua parameters.
- Produces: `pet_fragment` with a 48-step Schwarzschild near path and fitted weak-deflection far path.
- Produces: real mode from captured desktop texture and Lite mode from transparent background.
- Preserves: `pet_pending_vertex`/`pet_pending_fragment`, one instanced pending-dot draw, renderer fallback and the synthetic RGBA harness.

- [ ] **Step 1: Add failing visual-structure tests to the macOS render harness**

Extend `TestRenderOptions` with capture mapping defaults and add helpers/tests in
`src-tauri/src/pet/native.rs`:

```rust
#[cfg(target_os = "macos")]
fn rgba_at(pixels: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
    let offset = (y * width + x) * 4;
    pixels[offset..offset + 4].try_into().unwrap()
}

#[cfg(target_os = "macos")]
fn warm(pixel: [u8; 4]) -> bool {
    pixel[0] > pixel[2] && pixel[0] > 80 && pixel[1] > 45 && pixel[3] > 20
}

#[cfg(target_os = "macos")]
#[test]
fn gargantua_has_a_black_shadow_warm_disk_and_two_lensed_arcs() {
    let input = checkerboard_rgba(256, 256);
    let output = super::test_render_with_options(
        &input,
        256,
        256,
        TestRenderOptions { mode: TestRenderMode::Real, ..Default::default() },
    ).unwrap().pixels;
    let center = rgba_at(&output, 256, 128, 128);
    assert!(center[0] < 16 && center[1] < 16 && center[2] < 16);
    assert!(center[3] > 240);
    let upper = (70..124).flat_map(|y| (72..184).map(move |x| (x, y)))
        .filter(|&(x, y)| warm(rgba_at(&output, 256, x, y))).count();
    let lower = (132..186).flat_map(|y| (72..184).map(move |x| (x, y)))
        .filter(|&(x, y)| warm(rgba_at(&output, 256, x, y))).count();
    assert!(upper > 24, "missing upper lensed disk arc");
    assert!(lower > 24, "missing lower lensed disk arc");
}

#[cfg(target_os = "macos")]
#[test]
fn gargantua_has_no_legacy_spectral_ring_function() {
    let source = include_str!("../../native/mac/shader.metal");
    assert!(!source.contains("spectral_ring"));
    assert!(source.contains("kGeodesicSteps = 48"));
    assert!(source.contains("shade_crossing"));
    assert!(source.contains("weak_deflection_background"));
}
```

Keep and strengthen `synthetic_capture_drives_distortion_in_the_annulus_outside_the_horizon`
by asserting that at least one vertical and one horizontal checkerboard boundary move while
the four output corners stay transparent.

- [ ] **Step 2: Run the focused renderer tests and verify failure**

Run:

```bash
cd src-tauri
cargo test pet::native::tests::gargantua_has_a_black_shadow_warm_disk_and_two_lensed_arcs -- --nocapture
cargo test pet::native::tests::gargantua_has_no_legacy_spectral_ring_function -- --nocapture
```

Expected: the first test fails on missing warm upper/lower arcs and the second fails because
the current shader still contains `spectral_ring`.

- [ ] **Step 3: Define one named, aligned uniform contract**

Replace the renderer uniform body in `bridge.h` with this exact order and mirror it in MSL and
Rust:

```c
typedef struct {
  float viewport_px[2];
  float capture_origin_uv[2];
  float capture_extent_uv[2];
  float time_seconds;
  float hole_radius_uv;
  float temperature;
  float inclination;
  float roll;
  float disk_inner;
  float disk_outer;
  float disk_opacity;
  float doppler;
  float beaming;
  float gain;
  float contrast;
  float wind;
  float speed;
  float exposure;
  float stars;
  float spin;
  float spin_phase;
  float drop_origin_uv[2];
  float drop_progress;
  float absorption_progress;
  float success_progress;
  float error_progress;
  uint32_t pending_count;
  uint32_t mode;
  uint32_t reduce_motion;
  uint32_t drop_phase;
  uint32_t file_kind;
  uint32_t _padding[3];
} PetRenderUniforms;
```

Assert `sizeof(PetRenderUniforms) == 152`, `offsetof(capture_origin_uv) == 8`,
`offsetof(temperature) == 32`, `offsetof(drop_origin_uv) == 96`, and
`offsetof(pending_count) == 120` in `render.mm`. Add matching Rust size/offset assertions.

In `pet.mm`, assign the fixed approved values:

```objc
uniforms.hole_radius_uv = 0.075f;
uniforms.temperature = 4500.0f;
uniforms.inclination = 1.52f;
uniforms.roll = 0.10f;
uniforms.disk_inner = 2.2f;
uniforms.disk_outer = 7.0f;
uniforms.disk_opacity = 0.85f;
uniforms.doppler = 0.35f;
uniforms.beaming = 2.0f;
uniforms.gain = 1.4f;
uniforms.contrast = 0.5f;
uniforms.wind = 7.0f;
uniforms.speed = 5.0f;
uniforms.exposure = 1.20f;
uniforms.stars = 0.0f;
uniforms.spin = 0.0f;
```

- [ ] **Step 4: Port the pinned WGSL optical path to MSL**

Use the pinned source:

```text
https://github.com/rrrjqy66/BlackHoleTrash/blob/229d93213cd3e57364b4c6655cfb2c75b7ea4d18/src/black_hole_trash.wgsl
```

Port the following source units without substituting the current radial approximation:

```metal
constant float kLensDepth = 13.0f;
constant int kGeodesicSteps = 48;
constant float kCriticalImpact = 2.5980762f;

float3 blackbody(float temperature);
float3 captured_background(float2 local_uv,
                           texture2d<float> capture,
                           sampler capture_sampler,
                           constant PetUniforms &uniforms);
float4 shade_crossing(float3 position, float3 velocity,
                      float3 normal, float3 disk_axis,
                      constant PetUniforms &uniforms,
                      float transmittance);
float3 weak_deflection_background(float2 p, float b,
                                  texture2d<float> capture,
                                  sampler capture_sampler,
                                  constant PetUniforms &uniforms);
float3 trace_schwarzschild(float2 p,
                          texture2d<float> capture,
                          sampler capture_sampler,
                          constant PetUniforms &uniforms,
                          thread float &alpha);
```

`trace_schwarzschild` starts at `x = float3(pr, Z0)` and
`v = float3(0, 0, -1)`, uses `h2 = dot(pr, pr)`, steps with
`dt = clamp(0.16 * r, 0.03, 1.5)`, performs kick-drift-kick acceleration
`-1.5 * h2 * x / (r2 * r2 * r)`, accumulates every disk-plane sign crossing,
marks `r2 < 1.0` as captured, and treats a budget-exhausted `dot(x, x) < 4.0`
ray as captured.

For the far path, copy the pinned finite-camera deflection fit and apply it to
`capture_origin_uv + local_panel_uv * capture_extent_uv`; do not replace it with
`0.055 / (radius - event_horizon)`. For Lite mode, call the same disk and
shadow code with a zero-alpha background. Return premultiplied alpha and force
the four panel corners to zero alpha with a smooth outer mask.

- [ ] **Step 5: Replace source attribution at the shader boundary**

Put this concise notice at the start of `shader.metal`:

```text
Black-hole optics are a Metal port of rrrjqy66/BlackHoleTrash
commit 229d93213cd3e57364b4c6655cfb2c75b7ea4d18 (MIT).
Original copyright: Copyright (c) 2026 GreenScreen410.
This application replaces recycling with acknowledged local import.
Full notices: THIRD_PARTY_NOTICES.md.
```

Add the complete GreenScreen410 MIT text, pinned URL, and WGSL-to-MSL modification statement
to `THIRD_PARTY_NOTICES.md`. Do not remove older historical notices in this task.

- [ ] **Step 6: Run visual, ABI, native, and complete Rust tests**

Run:

```bash
cd src-tauri
cargo test pet::native::tests -- --nocapture
cargo test
cd ..
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
/tmp/bambu-pet-native-test
```

Expected: Gargantua structure, capture distortion, transparent corners, uniform layout,
native lifecycle, and all Rust business tests pass.

- [ ] **Step 7: Commit the optical port**

```bash
git add src-tauri/native/mac/bridge.h \
  src-tauri/native/mac/shader.metal \
  src-tauri/native/mac/pet.mm \
  src-tauri/native/mac/render.mm \
  src-tauri/src/pet/native.rs \
  THIRD_PARTY_NOTICES.md
git commit -m "feat: port BlackHoleTrash rendering to Metal"
```

### Task 3: Expand size, cap the drawable, and add the Fusion appearance

**Files:**
- Modify: `src-tauri/src/pet/mod.rs`
- Modify: `src-tauri/src/pet/store.rs`
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/native/mac/bridge.h`
- Modify: `src-tauri/native/mac/pet_lifecycle.h`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/render.mm`
- Modify: `src-tauri/native/mac/shader.metal`
- Modify: `src/features/settings/Pet.tsx`
- Modify: `src/features/settings/Pet.test.tsx`
- Modify: `src/lib/tauri.ts`
- Modify: `src/i18n/locales/zh-CN.json`
- Modify: `src/i18n/locales/zh-TW.json`
- Modify: `src/i18n/locales/en.json`
- Modify: `THIRD_PARTY_NOTICES.md`

**Interfaces:**
- Consumes: Task 1 `PetEffectGeometryForSize` and `PetCaptureRegionForPanel`, Task 2's
  152-byte `PetRenderUniforms` and single Schwarzschild trace.
- Produces: `PetVisualStyle::{Gargantua, Fusion}` serialized as
  `"gargantua" | "fusion"` and native values `0 | 1`.
- Produces: `double PetDrawableLogicalSide(double pet_size)` returning
  `min(clamp(pet_size, 120, 900), 360)`.
- Produces: `PetRenderUniforms.visual_style` at offset 140 by consuming one existing
  `_padding[3]` word; total uniform size remains 152 bytes.
- Produces: `PetConfig.visual_style` at offset 62 by consuming existing tail padding; total
  config size remains 64 bytes.
- Preserves: `capture_side = 1.60 × pet_size`, normalized capture UV, event-horizon physics,
  circular hit geometry, Real/Lite behavior, animation state, and pending draw count.

- [ ] **Step 1: Add failing Rust and UI tests for the exact setting contract**

In `src-tauri/src/pet/store.rs`, add:

```rust
#[test]
fn pet_size_accepts_the_expanded_range_and_rejects_outside_values() {
    assert!(parse_size(Some("120".to_owned())).is_ok());
    assert!(parse_size(Some("360".to_owned())).is_ok());
    assert!(parse_size(Some("600".to_owned())).is_ok());
    assert!(parse_size(Some("900".to_owned())).is_ok());
    assert!(parse_size(Some("119".to_owned())).is_err());
    assert!(parse_size(Some("901".to_owned())).is_err());
}

#[test]
fn pet_visual_style_round_trips_and_defaults_to_gargantua() {
    assert_eq!(parse_visual_style(None).unwrap(), PetVisualStyle::Gargantua);
    assert_eq!(parse_visual_style(Some("fusion".to_owned())).unwrap(),
               PetVisualStyle::Fusion);
    assert!(parse_visual_style(Some("inferno".to_owned())).is_err());
}
```

In `src/features/settings/Pet.test.tsx`, assert all three visible preset buttons and both
styles:

```tsx
expect(screen.getByRole("button", { name: "300 px" })).toBeVisible();
expect(screen.getByRole("button", { name: "600 px" })).toBeVisible();
expect(screen.getByRole("button", { name: "900 px" })).toBeVisible();
expect(screen.getByLabelText("Black hole size")).toHaveAttribute("min", "120");
expect(screen.getByLabelText("Black hole size")).toHaveAttribute("max", "900");
await userEvent.click(screen.getByRole("button", { name: "Fusion" }));
expect(api.setPetSettings).toHaveBeenLastCalledWith({ visual_style: "fusion" });
```

- [ ] **Step 2: Add failing native tests for large geometry, capture, and drawable cap**

Append to `pet_lifecycle_test.cc`:

```cpp
static void large_sizes_keep_logical_geometry_but_cap_the_drawable() {
  const PetEffectGeometry six = PetEffectGeometryForSize(600.0);
  const PetEffectGeometry nine = PetEffectGeometryForSize(900.0);
  assert(close_to(six.panel_side, 600.0));
  assert(close_to(six.shadow_radius, 45.0));
  assert(close_to(nine.panel_side, 900.0));
  assert(close_to(nine.shadow_radius, 67.5));
  assert(close_to(PetDrawableLogicalSide(300.0), 300.0));
  assert(close_to(PetDrawableLogicalSide(360.0), 360.0));
  assert(close_to(PetDrawableLogicalSide(600.0), 360.0));
  assert(close_to(PetDrawableLogicalSide(900.0), 360.0));

  const PetScreenFrame display = {0.0, 0.0, 2560.0, 1600.0, 2.0, 42};
  const PetCaptureRegion capture = PetCaptureRegionForPanel(
      {800.0, 350.0, 900.0, 900.0}, display);
  assert(close_to(capture.source_width, 1440.0));
  assert(close_to(capture.source_height, 1440.0));
  assert(close_to(capture.panel_extent_uv[0], 0.625));
  assert(close_to(capture.panel_extent_uv[1], 0.625));
}
```

Run the Rust, UI, and native focused tests. Expected: the old 360 validator rejects 600/900,
the old presets are still visible, and `PetDrawableLogicalSide`/style fields do not exist.

- [ ] **Step 3: Implement settings and preserve both native ABIs**

Add the Rust enum and setting field:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PetVisualStyle { Gargantua, Fusion }
```

Validate `120..=900`, persist `pet_visual_style`, and use Gargantua when the key is absent so
existing databases remain compatible. The UI range is `120..900 step=4`; replace the old
three preset values with exactly `300`, `600`, and `900`.

Use the existing padding without changing sizes:

```c
typedef struct {
  /* existing PetConfig fields through request_permission at offset 61 */
  uint8_t visual_style; /* offset 62 */
  uint8_t _reserved;    /* offset 63 */
} PetConfig;

typedef struct {
  /* existing PetRenderUniforms fields through file_kind at offset 136 */
  uint32_t visual_style; /* offset 140 */
  uint32_t _padding[2];  /* offsets 144 and 148 */
} PetRenderUniforms;
```

Add C, Objective-C++, MSL, and Rust assertions for sizes `64`/`152` and offsets `62`/`140`.
Zero remains Gargantua; reject config values greater than one.

- [ ] **Step 4: Cap only the Metal drawable, never the panel or capture**

Add:

```cpp
inline double PetDrawableLogicalSide(double pet_size) {
  return std::min(360.0, std::clamp(pet_size, 120.0, 900.0));
}
```

When applying config, keep the AppKit visual panel and hit geometry at `pet_size`, keep
`PetCaptureRegionForPanel` at `1.60 × pet_size`, and set:

```objc
const CGFloat drawableLogicalSide = PetDrawableLogicalSide(config.size);
metalView.drawableSize = CGSizeMake(drawableLogicalSide * backingScale,
                                   drawableLogicalSide * backingScale);
```

The view scales that drawable to the full `pet_size × pet_size` panel. Do not replace `S` with
`D` in capture, event-horizon, point conversion, drag origin, or persisted coordinates.

- [ ] **Step 5: Add Fusion as one resolved material branch**

Resolve the exact Fusion uniforms on the CPU:

```text
temperature=5200, inclination=1.535, roll=0.04,
disk_inner=1.9, disk_outer=8.0, disk_opacity=0.88,
doppler=0.45, beaming=2.2, gain=2.0, contrast=0.65,
wind=7.0, speed=4.0, exposure=1.35, stars=0.0, spin=0.0
```

In `shade_crossing`, retain the Gargantua branch unchanged and use Fusion's wider band,
`density = band × (0.62 + 0.58 × streaks)`, and a 12% mix toward
`float3(1.0, 0.91, 0.70)`. Add a Fusion-only rim outside `kCriticalImpact` and start its outer
alpha feather at panel radius `0.42`; Gargantua remains `0.46`. Both end at `0.495`.

Do not add another trace, texture sample sequence, geometry pipeline, or animation state.
The style branch completes before absorption/success/error overlays.

- [ ] **Step 6: Lock style, large-size, and physical invariants with synthetic tests**

Add tests asserting:

```text
uniform size = 152; visual_style offset = 140
config size = 64; visual_style offset = 62
style 0 → 1 → 0 returns byte-identical Gargantua frames
both styles change annulus checksum when checkerboard input is inverted
both styles keep center RGB 0/0/0 and event-horizon area within 1%
Fusion variance_x / variance_y >= 2.2
Fusion warm annulus has R > G > B, G/R 0.78..0.96, B/R 0.45..0.82
Fusion emission coverage is 1.15..1.80 × Gargantua with <12% saturated pixels
Fusion alpha from panel radius 0.42..0.495 is monotonic and corners are transparent
300/360/600/900 output uses drawable logical sides 300/360/360/360
```

Run:

```bash
npm test -- src/features/settings/Pet.test.tsx src/lib/tauri.test.ts
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
/tmp/bambu-pet-native-test
cd src-tauri
cargo test pet::store::tests -- --nocapture
cargo test pet::native::tests -- --nocapture
```

Expected: all focused tests pass without launching the app or requesting screen-recording
permission.

- [ ] **Step 7: Attribute the active Fusion source and commit**

Update `THIRD_PARTY_NOTICES.md` so blackhole-timer is an active Fusion material/parameter
source at commit `f3cc9cc349540ad6d274cd8074cf050b9b0c0200`, retain its full MIT text and
`Copyright (c) 2026 s13k <s13k@pm.me>`, and state that browser/Pomodoro behavior was not
copied.

```bash
git add src-tauri/src/pet/mod.rs src-tauri/src/pet/store.rs \
  src-tauri/src/pet/native.rs src-tauri/native/mac/bridge.h \
  src-tauri/native/mac/pet_lifecycle.h \
  src-tauri/native/mac/pet_lifecycle_test.cc src-tauri/native/mac/pet.mm \
  src-tauri/native/mac/render.mm src-tauri/native/mac/shader.metal \
  src/features/settings/Pet.tsx src/features/settings/Pet.test.tsx \
  src/lib/tauri.ts src/i18n/locales/zh-CN.json \
  src/i18n/locales/zh-TW.json src/i18n/locales/en.json \
  THIRD_PARTY_NOTICES.md
git commit -m "feat: add scalable Fusion black hole style"
```

### Task 4: Complete file-card faller, jet, cancellation, Lite, and Reduce Motion

**Files:**
- Create: `src-tauri/native/mac/pet_drop_state.h`
- Modify: `src-tauri/native/mac/bridge.h`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`
- Modify: `src-tauri/native/mac/pet_render_state.h`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/render.mm`
- Modify: `src-tauri/native/mac/shader.metal`
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/build.rs`
- Modify: `THIRD_PARTY_NOTICES.md`

**Interfaces:**
- Consumes: Task 3 size/style ABI, capped drawable, and Gargantua/Fusion base renders.
- Produces: `PetDropState`, `PetDropSnapshot`, `PetDropPhase`, and generation-aware animation methods.
- Produces: procedural `PET_FILE_3MF` and `PET_FILE_GCODE` cards; no filename, path, thumbnail, or file texture.
- Produces: 4.6-second faller, one crossing at `u = 0.82`, 0.90-second absorption jet, impact/afterglow, 0.42-second rejection, and 0.15-second reduced-motion path.
- Produces native view methods for Task 5: `beginImportWait`, `finishImport`, `cancelImport`.
- Maps the design's composite states explicitly: `Hidden` remains in `PetWindowLifecycle`,
  `PetDragging` remains in `BPPetHost`, `SettlementPulse` remains in
  `PetRenderAnimationState`, and the external-file states live in `PetDropPhase`.

- [ ] **Step 1: Add failing deterministic animation tests**

Create the test-facing contract in `pet_drop_state.h` and add these calls to
`pet_lifecycle_test.cc`:

```cpp
static void import_wait_never_crosses_before_acknowledgment() {
  PetDropState state;
  assert(state.begin_wait(7, {0.72f, 0.44f}, PET_FILE_3MF, 10.0));
  const PetDropSnapshot waiting = state.sample(110.0, false);
  assert(waiting.phase == PetDropPhase::kImportPending);
  assert(waiting.faller_progress == 0.0f);
  assert(waiting.absorption_progress == 0.0f);
  assert(!waiting.deliver_once);
}

static void accepted_import_runs_the_complete_reference_timing() {
  PetDropState state;
  assert(state.begin_wait(7, {0.72f, 0.44f}, PET_FILE_3MF, 10.0));
  assert(state.finish(7, PET_DROP_ACCEPTED, 20.0));
  assert(state.sample(22.30, false).faller_progress > 0.49f);
  assert(state.sample(22.30, false).faller_progress < 0.51f);
  const PetDropSnapshot crossing = state.sample(23.772, false);
  assert(crossing.deliver_once);
  assert(crossing.absorption_progress == 0.0f);
  assert(!state.sample(23.773, false).deliver_once);
  const PetDropSnapshot jet_mid = state.sample(24.222, false);
  assert(jet_mid.absorption_progress > 0.49f);
  assert(jet_mid.absorption_progress < 0.51f);
  assert(state.sample(24.671, false).phase == PetDropPhase::kSwallow);
  assert(state.sample(24.673, false).phase == PetDropPhase::kIdle);
}

static void stale_ack_and_reduced_motion_are_bounded() {
  PetDropState state;
  assert(state.begin_wait(12, {0.6f, 0.5f}, PET_FILE_GCODE, 1.0));
  assert(!state.finish(11, PET_DROP_ACCEPTED, 2.0));
  assert(state.sample(20.0, false).phase == PetDropPhase::kImportPending);
  assert(state.finish(12, PET_DROP_ACCEPTED, 21.0));
  const PetDropSnapshot reduced = state.sample(21.075, true);
  assert(reduced.reduced_fade > 0.49f && reduced.reduced_fade < 0.51f);
  assert(reduced.fragment_count == 0);
  assert(reduced.absorption_progress == 0.0f);
  assert(state.sample(21.151, true).phase == PetDropPhase::kIdle);
}

static void rejected_import_recoils_without_delivery() {
  PetDropState state;
  assert(state.begin_wait(4, {0.8f, 0.5f}, PET_FILE_3MF, 1.0));
  assert(state.finish(4, PET_DROP_REJECTED, 2.0));
  const PetDropSnapshot recoil = state.sample(2.18, false);
  assert(recoil.phase == PetDropPhase::kImportRejected);
  assert(recoil.error_progress > 0.42f && recoil.error_progress < 0.44f);
  assert(!recoil.deliver_once);
  assert(recoil.absorption_progress == 0.0f);
  assert(state.sample(2.421, false).phase == PetDropPhase::kIdle);
}

static void cancellation_clears_every_visual_without_delivery() {
  PetDropState state;
  assert(state.begin_wait(8, {0.8f, 0.5f}, PET_FILE_3MF, 1.0));
  state.cancel();
  const PetDropSnapshot cancelled = state.sample(100.0, false);
  assert(cancelled.phase == PetDropPhase::kIdle);
  assert(cancelled.fragment_count == 0);
  assert(cancelled.absorption_progress == 0.0f);
  assert(!cancelled.deliver_once);
  assert(!state.finish(8, PET_DROP_ACCEPTED, 101.0));
}
```

- [ ] **Step 2: Compile the native test and verify the red state**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
```

Expected: compilation fails because `pet_drop_state.h`, its enums, and its methods do not exist.

- [ ] **Step 3: Implement the isolated drop-animation state**

Use these exact public shapes in `pet_drop_state.h`:

```cpp
enum class PetDropPhase : uint32_t {
  kIdle = 0,
  kExternalHoverValid = 1,
  kImportPending = 2,
  kSwallow = 3,
  kImportRejected = 4,
};

struct PetDropOrigin { float x; float y; };

struct PetDropSnapshot {
  PetDropPhase phase;
  uint64_t generation;
  PetDropOrigin origin;
  uint32_t file_kind;
  float hover_progress;
  float faller_progress;
  float absorption_progress;
  float error_progress;
  float reduced_fade;
  uint32_t fragment_count;
  bool deliver_once;
};
```

`begin_wait` accepts only Idle/hover and a nonzero generation. `finish` requires the current
generation. Accepted standard motion uses `faller = elapsed / 4.6`; absorption begins at
`4.6 × 0.82 = 3.772` and lasts 0.90 seconds. `deliver_once` is emitted by a latch on the first
sample at or after 3.772 seconds. Rejected motion lasts 0.42 seconds. Reduce Motion accepted
or rejected lasts 0.15 seconds and never sets fragments or absorption. `cancel` returns to
Idle and invalidates the generation.

- [ ] **Step 4: Draw the generic card, faller stages, fragments, and reference jet**

Add `PET_FILE_NONE = 0`, `PET_FILE_3MF = 1`, and `PET_FILE_GCODE = 2` to `bridge.h`.
Add one instanced card/fragments pipeline or include their SDF in the base pass, while keeping
pending dots as a separate instanced pass.

Implement these shader stage boundaries:

```metal
float approach = smoothstep(0.00f, 0.25f, u);
float stretch = smoothstep(0.20f, 0.55f, u);
float fragment = smoothstep(0.45f, 0.72f, u);
float merge = smoothstep(0.70f, 0.88f, u);
float fade = 1.0f - smoothstep(0.88f, 1.00f, u);
uint fragment_count = u >= 0.45f && u < 0.88f ? 12u : 0u;
```

Port `blackhole-mac` Faller path/radial/shear equations from the pinned commit, but use only
the programmatic card color and never `NSWorkspace.icon(forFile:)`. Port BlackHoleTrash
`absorption_jet_overlay` from the pinned commit with:

```metal
float attack = smoothstep(0.0f, 0.13f, progress);
float decay = 1.0f - smoothstep(0.45f, 1.0f, progress);
float extension = smoothstep(0.0f, 0.24f, progress);
float shock_progress = smoothstep(0.02f, 0.72f, progress);
float flash_decay = 1.0f - smoothstep(0.0f, 0.28f, progress);
```

Use energy `1.0`. Add impact constants `attack = 0.06`, `decay = 0.90`,
`lifetime = 4.0`, `feed_decay = 3.2`, and `feed_lifetime = 14.0`.
Do not implement batch growth or cursor graphics.

- [ ] **Step 5: Wire the state into Metal, Lite, and Core Animation fallback**

`BPPetView` owns `PetDropState` and exposes:

```objc
- (BOOL)beginImportWait:(uint64_t)generation
                 origin:(NSPoint)origin
               fileKind:(uint32_t)fileKind;
- (BOOL)finishImport:(uint64_t)generation result:(uint32_t)result;
- (void)cancelImport;
```

Each display-link tick samples the state and fills `drop_origin_uv`, `drop_progress`,
`absorption_progress`, `error_progress`, `drop_phase`, and `file_kind`.
Treat ImportPending/Swallow/ImportRejected as signal activity for Auto 60 FPS.
Window hide, sleep, destroy, drag exit before submission, and explicit import cancellation call
`cancelImport`, invalidate the generation, remove card/fragments/jet/error overlays, and cannot
emit `deliver_once`.

For Lite mode, call the same card and animation shader on a transparent background.
For Metal-unavailable CA fallback, create one generic rounded `CAShapeLayer` card and use:

```objc
standard.duration = 4.6;
standard.keyTimes = @[ @0.0, @0.25, @0.55, @0.72, @0.88, @1.0 ];
reduced.duration = 0.15;
rejected.duration = 0.42;
```

CA fallback does not need geodesics or fragments, but success must remain gated by
`finishImport`.

- [ ] **Step 6: Add Jack Zhang attribution and run animation/render tests**

Append the full blackhole-mac MIT text with
`Copyright (c) 2026 Jack Zhang`, pinned URL, and the Faller/Impacts adaptation statement to
`THIRD_PARTY_NOTICES.md`.

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
/tmp/bambu-pet-native-test
cd src-tauri
cargo test pet::native::tests -- --nocapture
cargo test
```

Add synthetic matrix cases for
`Gargantua/Fusion × Real/Lite × standard/Reduce Motion × pending/success/reject/cancel`.
Expected: exact timing, stale-ack, cancellation cleanup, both appearances, both render modes,
Reduce Motion, native lifecycle, and all Rust tests pass. In every standard success cell the
faller is 4.6 seconds and jet 0.90 seconds; reject/cancel cells contain no jet or delivery.

- [ ] **Step 7: Commit the complete visual animation**

```bash
git add src-tauri/native/mac/pet_drop_state.h \
  src-tauri/native/mac/bridge.h \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  src-tauri/native/mac/pet_render_state.h \
  src-tauri/native/mac/pet.mm \
  src-tauri/native/mac/render.mm \
  src-tauri/native/mac/shader.metal \
  src-tauri/src/pet/native.rs \
  src-tauri/build.rs \
  THIRD_PARTY_NOTICES.md
git commit -m "feat: add complete black hole swallow animation"
```

### Task 5: Generation-safe native drop and Rust import acknowledgment

**Files:**
- Modify: `src-tauri/native/mac/bridge.h`
- Modify: `src-tauri/native/mac/pet_drop_state.h`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/src/pet/input.rs`
- Modify: `src-tauri/src/pet/runtime.rs`
- Modify: `src-tauri/src/imports.rs`

**Interfaces:**
- Consumes: Task 4 `beginImportWait`/`finishImport`, existing `PrintService::import_print_file`, `confirm_new_print`, pending summary, notifications, and main-window navigation.
- Produces native callback event: `FileDropped { generation: u64, path: PathBuf }`.
- Produces C ABI: `void pet_finish_drop(void *handle, uint64_t generation, uint32_t result)`.
- Produces Rust: `DropValidation::read(&Path) -> Result<DropValidation>` and `NativePet::finish_drop(generation, result)`.
- Preserves: current duplicate-pending, settled-reprint, standalone-G-code, pending-dot, and inventory semantics.

- [ ] **Step 1: Add failing Rust tests for symlinks, generations, source preservation, and acknowledgment order**

In `pet/input.rs`, add:

```rust
#[cfg(unix)]
#[test]
fn symbolic_links_are_not_ordinary_drop_files() {
    use std::os::unix::fs::symlink;
    let directory = temp_drop_dir();
    fs::create_dir_all(&directory).unwrap();
    let target = directory.join("target.gcode.3mf");
    let link = directory.join("link.gcode.3mf");
    fs::write(&target, b"fixture").unwrap();
    symlink(&target, &link).unwrap();
    assert!(validate_supported_drop_path(&link).is_err());
    fs::remove_dir_all(directory).unwrap();
}
```

In `pet/runtime.rs`, change the recording native to retain drop completions and add:

```rust
#[test]
fn success_ack_uses_the_same_generation_and_follows_task_persistence() {
    let mut core = RuntimeCore::for_test_with_mapped_fixture();
    let generation = 41;
    let event = NativeEvent::FileDropped {
        generation,
        path: core.fixture.clone(),
    };
    core.handle(event);
    assert_eq!(core.pending_summary().unwrap().count, 1);
    assert_eq!(core.drop_results(), vec![(generation, NativeDropResult::Accepted)]);
}

#[test]
fn rejected_import_ack_does_not_change_pending_or_balances() {
    let mut core = RuntimeCore::for_test_with_mapped_fixture();
    let before = core.balance_rows();
    core.handle(NativeEvent::FileDropped {
        generation: 9,
        path: fixture("project_only.3mf"),
    });
    assert_eq!(core.pending_summary().unwrap().count, 0);
    assert_eq!(core.balance_rows(), before);
    assert_eq!(core.drop_results(), vec![(9, NativeDropResult::Rejected)]);
}

#[test]
fn successful_pet_import_does_not_modify_the_source() {
    let source = fixture("bambu_multicolor.3mf");
    let bytes_before = std::fs::read(&source).unwrap();
    let metadata_before = std::fs::metadata(&source).unwrap();
    let mut service = mapped_service();
    handle_file_drop(&mut service, &source).unwrap();
    let metadata_after = std::fs::metadata(&source).unwrap();
    assert_eq!(std::fs::read(&source).unwrap(), bytes_before);
    assert_eq!(metadata_after.len(), metadata_before.len());
    assert_eq!(metadata_after.modified().unwrap(), metadata_before.modified().unwrap());
}
```

Update callback ownership tests to assert the callback copies both the path and the event-specific
generation value.

- [ ] **Step 2: Add failing pure native session tests**

Add a pure `PetDropSession` to `pet_drop_state.h` and tests:

```cpp
static void a_drop_session_requires_the_same_generation_path_and_core_point() {
  PetDropSession session;
  const uint64_t generation =
      session.enter("/tmp/first.gcode.3mf", PET_FILE_3MF);
  assert(generation != 0);
  assert(session.can_submit(generation, "/tmp/first.gcode.3mf", true));
  assert(!session.can_submit(generation + 1, "/tmp/first.gcode.3mf", true));
  assert(!session.can_submit(generation, "/tmp/second.gcode.3mf", true));
  assert(!session.can_submit(generation, "/tmp/first.gcode.3mf", false));
  assert(session.submit(generation, "/tmp/first.gcode.3mf", true));
  assert(!session.submit(generation, "/tmp/first.gcode.3mf", true));
}
```

- [ ] **Step 3: Run focused tests and verify the red state**

Run:

```bash
cd src-tauri
cargo test pet::input::tests::symbolic_links_are_not_ordinary_drop_files -- --nocapture
cargo test pet::runtime::tests::success_ack_uses_the_same_generation_and_follows_task_persistence -- --nocapture
cargo test pet::runtime::tests::successful_pet_import_does_not_modify_the_source -- --nocapture
cd ..
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
```

Expected: failures for missing validation, generation-bearing event, drop result method, and
native session.

- [ ] **Step 4: Implement two-pass AppKit validation without any delete operation**

Keep `PetCallback` binary-compatible but rename its final parameter to `event_value`; it is a
display ID for move/display events and a generation for file-drop events.

In `BPCoreHitTargetView`, implement one candidate reader used by both enter/update and drop:

```objc
typedef struct {
  BOOL valid;
  uint32_t fileKind;
} BPDropCandidateKind;

// For each NSURL in pasteboard order:
// 1. require fileURL and absolute path;
// 2. lstat(path.fileSystemRepresentation, &status) == 0;
// 3. require S_ISREG(status.st_mode) and reject S_ISLNK(status.st_mode);
// 4. accept .gcode.3mf, .3mf, or .gcode;
// 5. stop at the first supported ordinary file.
```

`draggingEntered` creates a generation and saves the exact path only when the cursor is inside
the circle. `draggingUpdated` returns Copy only while the current candidate and point still
match. `draggingExited` cancels the session. `performDragOperation` re-reads the pasteboard,
re-runs `lstat`, compares generation and exact path, enters ImportPending, and invokes:

```objc
self.callback(kPetCallbackFileDropped,
              path.fileSystemRepresentation,
              0.0, 0.0, generation);
```

Delete no file. Do not call `NSFileManager` move/remove, `NSWorkspace.recycleURLs`, Apple
Events, shell commands, or `IFileOperation`.

- [ ] **Step 5: Add the C/Rust acknowledgment API**

Add to `bridge.h`:

```c
enum {
  PET_DROP_ACCEPTED = 1,
  PET_DROP_REJECTED = 2,
};

void pet_finish_drop(void *handle, uint64_t generation, uint32_t result);
```

Add matching Rust:

```rust
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDropResult {
    Accepted = 1,
    Rejected = 2,
}

impl NativePet {
    pub fn finish_drop(&self, generation: u64, result: NativeDropResult) {
        platform::finish_drop(self.handle, generation, result as u32);
    }
}
```

Extend `RuntimeNative` with the same method. Change the callback mapping to:

```rust
NativeEvent::FileDropped {
    generation: event_value,
    path: PathBuf::from(owned_payload),
}
```

Native `pet_finish_drop` dispatches to the main thread and calls Task 4 `finishImport`; stale
generations return without visual changes.

- [ ] **Step 6: Revalidate ordinary files and file stability inside Rust**

In `pet/input.rs`, make validation exact:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropValidation {
    pub canonical_path: PathBuf,
    pub size: u64,
    pub modified_nanos: u128,
}

pub fn validate_supported_drop_path(path: &Path) -> io::Result<DropValidation> {
    let link = std::fs::symlink_metadata(path)?;
    if link.file_type().is_symlink() || !link.file_type().is_file()
        || !path.is_absolute() || !is_supported_print_path(path)
    {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "unsupported drop"));
    }
    let canonical_path = path.canonicalize()?;
    let metadata = std::fs::metadata(&canonical_path)?;
    let modified_nanos = metadata.modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid mtime"))?
        .as_nanos();
    Ok(DropValidation { canonical_path, size: metadata.len(), modified_nanos })
}
```

In `imports.rs`, make `FileStability::read` use `symlink_metadata`, reject links/non-files,
and return the first stability value from `ensure_stable`. Compare it again after hash/parse
and before any new parse-cache/job transaction. For cached paths, compare again before
returning an existing or new-print preview. A mismatch returns existing
`AppError::FileNotStable` (`file_not_stable`) before a task write.

- [ ] **Step 7: Acknowledge only after the existing task path succeeds**

Change `import_from_pet` to accept `generation`. On success:

```rust
let signal = handle_file_drop(&mut service, path)?;
let (job_id, pending_count) = match signal {
    PetSignal::ImportSucceeded { job_id, pending_count } => (job_id, pending_count),
    _ => unreachable!("file import only returns an import result"),
};
refresh_pending_state(state, PendingSummary {
    count: pending_count,
    newest_job_id: Some(job_id),
}, None);
state_native(state).finish_drop(generation, NativeDropResult::Accepted);
```

On every error, refresh the existing pending summary without an import-success signal, call
`finish_drop(generation, Rejected)`, emit the current localized error event/notification, and
log only `error.code()`. Keep settlement signal code 3 unchanged. Remove the old generic
import-success signal call so one success cannot start two animations.

- [ ] **Step 8: Run native, focused Rust, and complete Rust suites**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
/tmp/bambu-pet-native-test
cd src-tauri
cargo test pet::input::tests -- --nocapture
cargo test pet::runtime::tests -- --nocapture
cargo test imports::tests -- --nocapture
cargo test
```

Expected: sessions reject stale/path/point mismatches, symlinks and directories fail, success
acknowledges the matching generation after persistence, failure never delivers, source bytes
and metadata remain unchanged, balances remain unchanged, and all existing business tests pass.

- [ ] **Step 9: Commit the safe import handshake**

```bash
git add src-tauri/native/mac/bridge.h \
  src-tauri/native/mac/pet_drop_state.h \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  src-tauri/native/mac/pet.mm \
  src-tauri/src/pet/native.rs \
  src-tauri/src/pet/input.rs \
  src-tauri/src/pet/runtime.rs \
  src-tauri/src/imports.rs
git commit -m "fix: acknowledge safe black hole imports"
```

### Task 6: Gate the next preview on the complete state and permission matrix

**Files:**
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/src/pet/runtime.rs`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`
- Modify: `src/features/settings/Pet.test.tsx`
- Modify: `docs/qa-black-hole.md`

**Interfaces:**
- Consumes: Task 3 size/style contract, Task 4 visual state machine, Task 5 acknowledged import,
  existing `PetStatus { effective_mode, permission, fallback_reason }`, synthetic renderer,
  and release application build.
- Produces: automated coverage for every preview cell and a completed
  `docs/qa-black-hole.md` preview matrix with observed result/evidence.
- Produces: the only preview build that may be shown to the user after this plan revision.
- Preserves: no real desktop frame, source path, filename, thumbnail, or screen recording is
  committed as preview evidence.

- [ ] **Step 1: Add failing permission/effective-mode regressions**

In `src-tauri/src/pet/runtime.rs`, exercise every unavailable capture result:

```rust
#[test]
fn every_unavailable_capture_state_uses_lite_without_changing_requested_mode() {
    for (event, permission, reason) in [
        (CaptureEvent::NotDetermined, CapturePermission::NotDetermined,
         "permission_not_determined"),
        (CaptureEvent::Denied, CapturePermission::Denied, "permission_denied"),
        (CaptureEvent::RestartRequired, CapturePermission::RestartRequired,
         "permission_restart_required"),
        (CaptureEvent::Unavailable, CapturePermission::Unavailable,
         "capture_unavailable"),
    ] {
        let status = CaptureState::Requested.reduce(event);
        assert_eq!(status.effective_mode, PetMode::Lite);
        assert_eq!(status.permission, permission);
        assert_eq!(status.fallback_reason.as_deref(), Some(reason));
        assert!(status.pet_visible);
    }
}

#[test]
fn ready_capture_restores_real_and_saved_large_fusion_settings_reload() {
    let db = AppDatabase::open_in_memory().unwrap();
    let saved = PetStore::apply(&db, PetSettingsPatch {
        mode: Some(PetMode::Real),
        size: Some(900),
        visual_style: Some(PetVisualStyle::Fusion),
        ..Default::default()
    }).unwrap();
    let status = capture_status(
        saved.mode,
        NativeCaptureState::Ready,
        NativeRendererState::Ready,
    );
    assert_eq!(status.effective_mode, PetMode::Real);
    let reloaded = PetStore::load(&db).unwrap();
    assert_eq!(reloaded.size, 900);
    assert_eq!(reloaded.visual_style, PetVisualStyle::Fusion);
}
```

The settings UI test must assert that `restart_required` shows an explicit restart instruction,
`denied` shows Lite as active, and neither state silently changes the requested mode from Real.

- [ ] **Step 2: Add one table-driven synthetic preview test**

Add a test table over:

```rust
for style in [Gargantua, Fusion] {
    for mode in [Real, Lite] {
        for size in [300_u32, 600, 900] {
            verify_idle_distortion_or_transparency(style, mode, size);
            verify_standard_success(style, mode, size, 4.6, 0.90);
            verify_rejection_has_no_delivery_or_jet(style, mode, size, 0.42);
            verify_cancellation_is_idle_and_empty(style, mode, size);
            verify_reduced_motion_has_no_fragments_or_jet(style, mode, size, 0.15);
        }
    }
}
```

`verify_idle_distortion_or_transparency` must invert a checkerboard capture and require changed
annulus checksums in Real; Lite must remain independent of capture input with transparent
corners. Each 600/900 case asserts drawable logical side 360 while the capture region remains
`1.60 × size`. Success samples the 4.6-second phase boundaries and 0.90-second jet midpoint;
failure and cancellation assert zero delivery, fragments, impact, and jet.

- [ ] **Step 3: Run the complete automated preview gate before launching**

Run:

```bash
npm test
npm run build
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
/tmp/bambu-pet-native-test
cd src-tauri
cargo test pet::native::tests -- --nocapture
cargo test pet::runtime::tests -- --nocapture
cargo test
cd ..
```

Expected: frontend, native, synthetic Metal, runtime, and full Rust suites pass. Stop here and
do not launch or preview if any cell fails.

- [ ] **Step 4: Build one integrated preview application**

Run:

```bash
npm run tauri build
```

Use only the generated `.app` for the following matrix. Do not provide an earlier Gargantua-only,
Fusion-only, animation-only, or large-size-only preview.

- [ ] **Step 5: Record the complete visual/state matrix**

On a privacy-safe desktop fixture containing text and a straight grid, record pass/fail plus
notes in `docs/qa-black-hole.md` for all of:

```text
Real Gargantua and Real Fusion: desktop letters/grid bend continuously, not circular zoom
Lite Gargantua and Lite Fusion: transparent background, same hole/card/state timing
300, legacy 360, 600, and 900 px: no clipping; 600/900 remain responsive
standard success: full 4.6 s approach/stretch/fragment/merge/crossing
absorption jet: visible for 0.90 s and triggered once at u=0.82
failure: 0.42 s recoil, no delivery/jet/pending increment
cancel by drag exit, hide, sleep, and destroy: card/fragment/jet removed, stale ack ignored
Reduce Motion: 0.15 s fade/pulse, no orbit/stretch/fragments/jet
Auto/30/60 FPS and 1×/2× displays
left/right/top/bottom clipping and two-display crossing
```

Run every animation/state row once per style and once per Real/Lite mode. The matrix may use
the smallest representative size for animation repetition, but both 600 and 900 must complete
one standard success and one cancel in each style.

- [ ] **Step 6: Exercise authorization, denial, revocation, and restart**

Start from a system state where the app lacks Screen Recording permission:

```text
1. Select requested mode Real; verify effective mode is automatically Lite.
2. Click the explicit authorization control once; verify no repeated prompt loop.
3. If macOS reports restart_required, verify the UI says a full quit/restart is required.
4. Quit every app process, relaunch exactly one .app instance, and verify Real desktop
   distortion becomes active while size/style/position/pending state are preserved.
5. Deny once and verify Lite remains fully interactive and safely imports.
6. Revoke an existing grant, return to/restart the app as required, and verify automatic Lite.
7. Re-grant and restart; verify Real returns without resetting 900 px + Fusion.
```

Record the OS version, each reported permission enum, effective mode, whether restart was
required, and observed pass/fail. Do not commit screenshots containing the user's desktop.

- [ ] **Step 7: Commit only after every preview row passes**

```bash
git add src-tauri/src/pet/native.rs src-tauri/src/pet/runtime.rs \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  src/features/settings/Pet.test.tsx docs/qa-black-hole.md
git commit -m "test: gate the complete black hole preview"
```

Expected: this commit does not exist until desktop distortion, 4.6 s swallow, 0.90 s jet,
failure, cancellation, Reduce Motion, Lite/Real, both styles, 600/900, and permission/restart
rows are all complete.

### Task 7: Fixtures, licensing, release build, and final acceptance

**Files:**
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/src/settlement.rs` only if the existing smoke lacks one of the specified assertions
- Modify: `THIRD_PARTY_NOTICES.md`
- Modify: `docs/install-mac.md`
- Modify: `docs/qa-black-hole.md`

**Interfaces:**
- Consumes: all Tasks 1–6, existing synthetic renderer, existing ignored `BAMBU_SMOKE_3MF` test, existing release scripts.
- Produces: deterministic optical/animation regression tests, complete MIT notices, user-facing safe-import documentation, and a signed-for-local-testing `.app`/`.dmg`.
- Produces: a completed manual matrix covering the user screenshot, pinned BlackHoleTrash reference, source preservation, edge displays, FPS, Lite, Reduce Motion, permission and lifecycle.

- [ ] **Step 1: Add final structural regressions for every approved visual state**

Add synthetic render tests that drive the exact uniform states:

```rust
#[cfg(target_os = "macos")]
#[test]
fn import_pending_card_stays_outside_the_shadow() {
    let output = test_render_with_options(
        &checkerboard_rgba(256, 256),
        256,
        256,
        TestRenderOptions {
            drop_phase: TestDropPhase::ImportPending,
            file_kind: TestFileKind::ThreeMf,
            drop_origin_uv: [0.78, 0.50],
            ..Default::default()
        },
    ).unwrap().pixels;
    assert!(card_pixels_inside_radius(&output, 256, 0.075) == 0);
    assert!(card_pixels_outside_radius(&output, 256, 0.075) > 16);
}

#[cfg(target_os = "macos")]
#[test]
fn swallow_crossing_has_fragments_and_reference_jet() {
    let output = test_render_with_options(
        &checkerboard_rgba(256, 256),
        256,
        256,
        TestRenderOptions {
            drop_phase: TestDropPhase::Swallow,
            drop_progress: 0.82,
            absorption_progress: 0.50,
            file_kind: TestFileKind::ThreeMf,
            ..Default::default()
        },
    ).unwrap().pixels;
    assert!(fragment_component_count(&output, 256) >= 4);
    assert!(bright_jet_pixels(&output, 256) > 20);
}

#[cfg(target_os = "macos")]
#[test]
fn reduced_motion_has_no_fragments_or_jet() {
    let output = test_render_with_options(
        &checkerboard_rgba(256, 256),
        256,
        256,
        TestRenderOptions {
            drop_phase: TestDropPhase::Swallow,
            drop_progress: 0.50,
            absorption_progress: 0.50,
            reduce_motion: true,
            file_kind: TestFileKind::ThreeMf,
            ..Default::default()
        },
    ).unwrap().pixels;
    assert_eq!(fragment_component_count(&output, 256), 0);
    assert_eq!(bright_jet_pixels(&output, 256), 0);
}
```

Implement helpers as deterministic connected-component/luminance scans over the returned RGBA;
do not write snapshots to the user fixture directory and do not make tests depend on a desktop
screenshot.

- [ ] **Step 2: Run every automated suite and the real read-only fixture**

Run:

```bash
npm test
npm run build
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/bambu-pet-native-test
/tmp/bambu-pet-native-test
cd src-tauri
cargo test
BAMBU_SMOKE_3MF='/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf' \
  cargo test smoke_real_sliced_file_from_environment -- --ignored --nocapture
cd ..
```

Expected: frontend, native, and Rust suites pass; the real fixture reports four tool usages and
all existing settlement outcomes; its SHA-256, byte length, and modification time remain
unchanged.

- [ ] **Step 3: Make the third-party notice complete and internally consistent**

Verify `THIRD_PARTY_NOTICES.md` contains:

```text
rrrjqy66/BlackHoleTrash
commit 229d93213cd3e57364b4c6655cfb2c75b7ea4d18
Copyright (c) 2026 GreenScreen410
WGSL optics and absorption jet ported to Metal; recycling replaced by local import.

ZGhey/blackhole-mac
commit f719aa1139ecc49a728cbb8fac2e60fcfa51996e
Copyright (c) 2026 Jack Zhang
Faller/Impacts timing adapted; optional trash action and source mutation were not copied.

cabbagehao/blackhole-timer
commit f3cc9cc349540ad6d274cd8074cf050b9b0c0200
Copyright (c) 2026 s13k <s13k@pm.me>
Fusion material parameters and feather semantics adapted; browser/Pomodoro behavior was not copied.
```

Each entry must include the complete MIT permission and warranty paragraphs. The file and shader
must agree that BlackHoleTrash is the normative optical/physics source and blackhole-timer is an
active Fusion look source, not a second trace implementation.

- [ ] **Step 4: Update install and QA documentation with exact user-visible behavior**

In `docs/install-mac.md`, state:

```text
把文件拖到黑洞中心只会在本机读取并建立待结算任务；源文件不会被删除、移入废纸篓、
移动、改名或写回。文件只有在解析和任务持久化成功后才会完成吞噬动画。
```

In `docs/qa-black-hole.md`, add checkboxes for:

```text
- Pinned BlackHoleTrash Gargantua and specified Fusion appearance both match their references.
- No cyan/violet/rose legacy ring is visible.
- Desktop text bends continuously through near/far lens regions.
- 120..900 is enforced; presets are 300/600/900; legacy 360 remains a regression point.
- Capture is 1.60 × size while 600/900 use a 360 logical-side internal drawable.
- File waits outside the horizon before Rust success.
- Standard success shows 4.6 s faller and 0.90 s jet.
- Failure recoils and never shows delivery/jet.
- Cancellation removes the card and never shows delivery/jet.
- Reduce Motion completes in 0.15 s without orbit/fragments/jet.
- Dragging the pet across Finder files creates zero imports.
- Multi-file drop imports only the first supported ordinary file.
- Directory, symlink, stale session, changed path, and outside-circle drop are rejected.
- Source SHA-256, length, and modification time are unchanged after success and failure.
- Left/right/top/bottom display edges align with desktop pixels.
- Two-display crossing, 1×/2× scale, Auto/30/60, sleep/wake, hide/show pass.
- Not-determined, denied, revoked, restart-required, capture failure, and Metal unavailable
  retain Lite safe import.
- Permission grant plus complete app restart restores Real without resetting size/style/state.
```

- [ ] **Step 5: Build and inspect the release application**

Run:

```bash
npm run tauri build
```

Expected:

```text
src-tauri/target/release/bundle/macos/拓竹耗材管家.app
src-tauri/target/release/bundle/dmg/拓竹耗材管家_0.1.0_aarch64.dmg
```

Launch only the `.app` copy, verify one process, one menu-bar icon, and one black hole, then
perform the QA matrix on a text/icon desktop background. Record observed pass/fail notes in
`docs/qa-black-hole.md`; no screenshot containing private desktop content is committed.

- [ ] **Step 6: Run a final clean verification**

Run:

```bash
git diff --check
rg -n "spectral_ring|NSWorkspace\\.recycleURLs|removeItemAtURL|IFileOperation" \
  src-tauri/native/mac src-tauri/src
npm test
npm run build
cd src-tauri
cargo test
cd ..
git status --short
```

Expected: `git diff --check` is silent; the source scan finds no legacy ring or delete/recycle
call in the implementation; all tests/builds pass; status contains only the Task 7 documentation
and test changes before commit.

- [ ] **Step 7: Commit the verified release evidence**

```bash
git add src-tauri/src/pet/native.rs \
  src-tauri/src/settlement.rs \
  THIRD_PARTY_NOTICES.md \
  docs/install-mac.md \
  docs/qa-black-hole.md
git commit -m "test: verify BlackHoleTrash safe import release"
```

## Completion Gate

- [ ] All seven task commits exist and each task passed its focused tests before proceeding;
      Tasks 1–2 retain their original completed commits.
- [ ] The current black hole provides complete `Gargantua` and `Fusion` appearances on one
      pinned BlackHoleTrash optical path, not the old rainbow ring or a second fake lens.
- [ ] `pet_size` is `120..=900`, presets are `300/600/900`, and legacy 360 is approximately
      40% of the new maximum.
- [ ] Capture remains `1.60 × pet_size`; the Metal internal drawable logical side caps at 360
      without changing panel, hit, capture, or persisted geometry.
- [ ] `PetRenderUniforms` remains 152 bytes with `visual_style` at offset 140; `PetConfig`
      remains 64 bytes.
- [ ] Capture mapping is correct at all four screen edges and on 1×/2× displays.
- [ ] The central target is circular and pet movement never reads or imports Finder files.
- [ ] Native enter/drop and Rust each revalidate the file; stale generations are ignored.
- [ ] The full swallow starts only after the matching Rust success acknowledgment.
- [ ] Standard, Reduce Motion, Real, Lite, failure, cancellation, pending, settlement, FPS,
      dual-style, and 600/900 lifecycle states pass.
- [ ] The next user preview was not produced until Task 6 recorded desktop distortion,
      4.6-second swallow, 0.90-second jet, failure/cancel, Reduce Motion, Lite/Real, both
      appearances, large sizes, and authorization/restart.
- [ ] Missing, denied, revoked, restart-required, and failed Screen Recording automatically use
      Lite; authorization plus full restart restores Real.
- [ ] Source SHA-256, length and modification time are unchanged in fixture and real-file tests.
- [ ] Existing pending/reprint/settlement/inventory behavior passes without changed balances on import.
- [ ] GreenScreen410, Jack Zhang, and s13k MIT notices and pinned source descriptions are complete.
- [ ] The release `.app` runs as one process with one menu-bar icon and one black hole.
