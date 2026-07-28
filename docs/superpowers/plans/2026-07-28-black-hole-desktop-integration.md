# Black Hole Desktop Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace CYLUNE's current macOS black-hole visual with the approved `tiyda/blackhole-desktop` renderer while adding only manual dragging, live size adjustment, and safe print-file import.

**Architecture:** One transparent visual pane covers each active display and renders the pinned upstream Metal effect. A single global screen-space center drives every pane, while one separate circular hit-target window handles black-hole dragging and Finder file drops. Each pane captures its complete display, so dragging changes only GPU uniforms and never re-aims a cropped capture.

**Tech Stack:** Objective-C++, AppKit, Metal, ScreenCaptureKit, IOSurface, Rust/Tauri, React/Vitest, C++17 native tests.

## Global Constraints

- Pin visual behavior to `tiyda/blackhole-desktop` commit `03e74a5cf2522748993aca679cdc6027c7b19697`.
- Preserve the upstream shader constants and default appearance without recoloring or adding a particle system.
- Add only manual dragging, live size adjustment, and supported-file import.
- Never copy or invoke upstream Trash/recycle behavior.
- Accept only `.3mf`, `.gcode.3mf`, and `.gcode`; never move or delete the source file.
- Visual panes remain mouse-transparent; only the circular core target accepts gestures and file URLs.
- The center may reach physical display edges, corners, the menu-bar region, the Dock region, and display seams.
- Preserve CYLUNE's existing 120–900 size range and persisted setting.
- Retain the MIT notices for tiyda and s13k in the packaged application.

---

### Task 1: Full-display center and geometry contract

**Files:**
- Modify: `src-tauri/native/mac/pet_lifecycle.h`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`

**Interfaces:**
- Produces: `PetScreenPoint`, `PetDisplayIndexForPoint`, `PetCenterUVForDisplay`, `PetFullDisplayCaptureRegion`, `PetRecoverCenter`.
- Consumes: existing `PetScreenFrame` and `PetCaptureRegion`.

- [ ] **Step 1: Write failing native geometry tests**

Add literal tests that prove the behavior rather than the implementation:

```cpp
static void center_selects_display_and_maps_negative_coordinates() {
  const PetScreenFrame displays[] = {
      {0.0, 0.0, 1440.0, 900.0, 2.0, 10},
      {-1920.0, 0.0, 1920.0, 1080.0, 1.0, 20},
  };
  assert(PetDisplayIndexForPoint({720.0, 450.0}, displays, 2, 1) == 0);
  assert(PetDisplayIndexForPoint({-960.0, 540.0}, displays, 2, 0) == 1);
  const auto uv = PetCenterUVForDisplay({-960.0, 540.0}, displays[1]);
  assert(close_to(uv.x, 0.5));
  assert(close_to(uv.y, 0.5));
}

static void exact_display_seam_keeps_current_display() {
  const PetScreenFrame displays[] = {
      {0.0, 0.0, 1440.0, 900.0, 2.0, 10},
      {1440.0, 0.0, 1920.0, 1080.0, 1.0, 20},
  };
  assert(PetDisplayIndexForPoint({1440.0, 400.0}, displays, 2, 0) == 0);
  assert(PetDisplayIndexForPoint({1440.0, 400.0}, displays, 2, 1) == 1);
  assert(PetDisplayIndexForPoint({1440.1, 400.0}, displays, 2, 0) == 1);
}

static void full_display_capture_never_changes_with_center() {
  const PetScreenFrame display = {-1920.0, 0.0, 1920.0, 1080.0, 2.0, 20};
  const PetCaptureRegion region = PetFullDisplayCaptureRegion(display);
  assert(region.display_id == 20);
  assert(close_to(region.source_x, 0.0));
  assert(close_to(region.source_y, 0.0));
  assert(close_to(region.source_width, 1920.0));
  assert(close_to(region.source_height, 1080.0));
  assert(region.pixel_width == 3840);
  assert(region.pixel_height == 2160);
}

static void disconnected_center_recovers_only_when_off_every_display() {
  const PetScreenFrame display = {0.0, 0.0, 1440.0, 900.0, 2.0, 10};
  const auto retained = PetRecoverCenter({0.0, 900.0}, &display, 1);
  assert(close_to(retained.x, 0.0));
  assert(close_to(retained.y, 900.0));
  const auto recovered = PetRecoverCenter({4000.0, 3000.0}, &display, 1);
  assert(close_to(recovered.x, 720.0));
  assert(close_to(recovered.y, 450.0));
}
```

- [ ] **Step 2: Run the native test and verify RED**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac src-tauri/native/mac/pet_lifecycle_test.cc -o /tmp/cylune-pet-lifecycle-test
```

Expected: compilation fails because the five new geometry interfaces do not exist.

- [ ] **Step 3: Implement the minimal pure geometry helpers**

Add the named structs/functions to `pet_lifecycle.h`. Use full `NSScreen.frame` coordinates, keep the current display when a point lies exactly on its inclusive seam, return normalized UV without clamping, and recover to the primary display center only if no display contains the saved center.

- [ ] **Step 4: Run the native test and verify GREEN**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac src-tauri/native/mac/pet_lifecycle_test.cc -o /tmp/cylune-pet-lifecycle-test
/tmp/cylune-pet-lifecycle-test
```

Expected: exit code 0.

- [ ] **Step 5: Commit the geometry contract**

```bash
git add src-tauri/native/mac/pet_lifecycle.h src-tauri/native/mac/pet_lifecycle_test.cc
git commit -m "feat: add full-display black hole geometry"
```

---

### Task 2: Approved upstream Metal renderer

**Files:**
- Modify: `src-tauri/native/mac/bridge.h`
- Modify: `src-tauri/native/mac/shader.metal`
- Modify: `src-tauri/native/mac/render.mm`
- Modify: `src-tauri/src/pet/native.rs`

**Interfaces:**
- Consumes: `PetRenderUniforms.time_seconds`, `hole_radius_uv`, `visual_style`, and captured desktop texture.
- Produces: `PetRenderUniforms.center_uv[2]`; the pinned upstream lens, accretion disk, photon ring, and transparency behavior.

- [ ] **Step 1: Write failing offscreen-render tests**

Extend `TestRenderOptions` with `center_uv` and add macOS tests:

```rust
fn opaque_horizon_centroid_x(pixels: &[u8], width: usize) -> f64 {
    let mut weighted_x = 0.0;
    let mut count = 0_u64;
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        if pixel[3] > 240 && pixel[0] < 8 && pixel[1] < 8 && pixel[2] < 8 {
            weighted_x += (index % width) as f64 + 0.5;
            count += 1;
        }
    }
    assert!(count > 0, "renderer produced no opaque horizon pixels");
    weighted_x / count as f64
}

#[test]
#[cfg(target_os = "macos")]
fn cpu_center_uniform_moves_the_horizon_without_moving_the_capture() {
    let input = checkerboard_rgba(256, 256);
    let left = test_render_with_options(
        &input, 256, 256,
        TestRenderOptions { center_uv: [0.30, 0.50], ..Default::default() },
    ).unwrap().pixels;
    let right = test_render_with_options(
        &input, 256, 256,
        TestRenderOptions { center_uv: [0.70, 0.50], ..Default::default() },
    ).unwrap().pixels;
    assert!(opaque_horizon_centroid_x(&left, 256) < 110.0);
    assert!(opaque_horizon_centroid_x(&right, 256) > 146.0);
}

#[test]
#[cfg(target_os = "macos")]
fn approved_accretion_disk_keeps_animating_while_center_is_stationary() {
    let input = checkerboard_rgba(256, 256);
    let first = test_render_with_options(
        &input, 256, 256,
        TestRenderOptions { time_seconds: 0.0, ..Default::default() },
    ).unwrap().pixels;
    let later = test_render_with_options(
        &input, 256, 256,
        TestRenderOptions { time_seconds: 1.5, ..Default::default() },
    ).unwrap().pixels;
    assert!(different_pixel_count(&first, &later) > 400);
    assert_preview_corners_are_transparent(&first, 256, "approved renderer");
}
```

The production mutations caught are a hard-coded center, a missing time term, or an opaque full-screen clear.

- [ ] **Step 2: Run the two tests and verify RED**

Run:

```bash
cd src-tauri
cargo test pet::native::tests::cpu_center_uniform_moves_the_horizon_without_moving_the_capture -- --exact
cargo test pet::native::tests::approved_accretion_disk_keeps_animating_while_center_is_stationary -- --exact
```

Expected: compilation fails because `center_uv` and the new behavior are missing.

- [ ] **Step 3: Port the pinned renderer with one interface change**

Port `Sources/BlackholeDesktop/BlackHole.metal` from commit `03e74a5` into `shader.metal`. Preserve its preset values, 40-step ray integration, time-driven disk noise, alpha mask, and background sampling. Replace only:

```metal
float2 center=float2(0.57+0.19*sin(t*.13), 0.62+0.12*sin(t*.17+2.0));
```

with:

```metal
float2 center = P.center_uv;
```

Map CYLUNE style 0 to upstream Defaults and style 1 to upstream Gargantua. Keep existing import feedback overlays after the upstream base color calculation, but do not alter idle pixels.

- [ ] **Step 4: Pass center through the native render ABI**

Add `float center_uv[2]` to `PetRenderUniforms`, mirror it in `TestNativeRenderUniforms`, and initialize it to `[0.5, 0.5]` in test defaults. Update size/offset assertions for the C/Rust layout.

- [ ] **Step 5: Run renderer tests and the full Rust suite**

Run:

```bash
cd src-tauri
cargo test pet::native::tests::cpu_center_uniform_moves_the_horizon_without_moving_the_capture -- --exact
cargo test pet::native::tests::approved_accretion_disk_keeps_animating_while_center_is_stationary -- --exact
cargo test
```

Expected: both focused tests and the full suite pass.

- [ ] **Step 6: Commit the renderer**

```bash
git add src-tauri/native/mac/bridge.h src-tauri/native/mac/shader.metal src-tauri/native/mac/render.mm src-tauri/src/pet/native.rs
git commit -m "feat: port approved black hole renderer"
```

---

### Task 3: One full-display capture per visual pane

**Files:**
- Modify: `src-tauri/native/mac/capture.mm`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`

**Interfaces:**
- Consumes: `PetFullDisplayCaptureRegion(PetScreenFrame)`.
- Produces: each display pane owns a capture handle whose region changes only when its display topology changes.

- [ ] **Step 1: Replace cropped-capture expectations with a failing full-display test**

Add:

```cpp
static void moving_center_does_not_reconfigure_full_display_capture() {
  const PetScreenFrame display = {0.0, 0.0, 1512.0, 982.0, 2.0, 42};
  const PetCaptureRegion first = PetFullDisplayCaptureRegion(display);
  const PetCaptureRegion after_drag = PetFullDisplayCaptureRegion(display);
  PetCaptureConfigurationGate gate;
  assert(gate.should_configure({0, true, first}, false));
  assert(!gate.should_configure({0, true, after_drag}, false));
}
```

Delete old test invocations whose required behavior is cropped re-aiming.

- [ ] **Step 2: Run native tests and verify RED**

Run the native compile-and-run command from Task 1.

Expected: failure because old cropped-capture tests or implementation expectations remain.

- [ ] **Step 3: Configure complete display streams**

Make each visual pane call `mac_capture_configure` with `PetFullDisplayCaptureRegion(display)`. Remove same-display `updateConfiguration` re-aiming from drag handling. A display change may create/destroy a pane and therefore start/stop its stream; changing only center or size must not.

- [ ] **Step 4: Keep capture exclusion and fallback**

Continue excluding CYLUNE-owned windows. Preserve the existing one-frame IOSurface gate, screen-recording permission state, wallpaper/lite fallback, sleep/wake stop, and orderly shutdown.

- [ ] **Step 5: Run native and Rust tests**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac src-tauri/native/mac/pet_lifecycle_test.cc -o /tmp/cylune-pet-lifecycle-test
/tmp/cylune-pet-lifecycle-test
cd src-tauri && cargo test
```

Expected: all tests pass.

- [ ] **Step 6: Commit full-display capture**

```bash
git add src-tauri/native/mac/capture.mm src-tauri/native/mac/pet.mm src-tauri/native/mac/pet_lifecycle_test.cc
git commit -m "fix: capture complete displays for black hole"
```

---

### Task 4: Multi-display visual panes and manual center dragging

**Files:**
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/pet_lifecycle.h`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`

**Interfaces:**
- Consumes: Task 1 geometry helpers and Task 2 `center_uv`.
- Produces: `BPDisplayPane` instances, one global `centerScreenPoint`, one standalone `BPCoreHitTargetView`.

- [ ] **Step 1: Write failing pane-position tests**

Add tests for these observable contracts:

```cpp
static void edge_centers_are_not_clamped() {
  const PetScreenFrame display = {0.0, 0.0, 1440.0, 900.0, 2.0, 10};
  for (PetScreenPoint point : {
           PetScreenPoint{0.0, 0.0},
           PetScreenPoint{1440.0, 0.0},
           PetScreenPoint{0.0, 900.0},
           PetScreenPoint{1440.0, 900.0},
       }) {
    const auto recovered = PetRecoverCenter(point, &display, 1);
    assert(close_to(recovered.x, point.x));
    assert(close_to(recovered.y, point.y));
  }
}

static void adjacent_panes_receive_complementary_seam_centers() {
  const PetScreenFrame left = {0.0, 0.0, 1440.0, 900.0, 2.0, 10};
  const PetScreenFrame right = {1440.0, 0.0, 1920.0, 1080.0, 1.0, 20};
  const auto left_uv = PetCenterUVForDisplay({1440.0, 450.0}, left);
  const auto right_uv = PetCenterUVForDisplay({1440.0, 450.0}, right);
  assert(close_to(left_uv.x, 1.0));
  assert(close_to(right_uv.x, 0.0));
}
```

- [ ] **Step 2: Run native tests and verify RED**

Run the native compile-and-run command. Expected: failure until the edge-inclusive recovery and pane mapping are implemented consistently.

- [ ] **Step 3: Refactor the native host into display panes**

Create one full-frame borderless `NSPanel` per `NSScreen`. Each panel:

- uses `screen.frame`, not `visibleFrame`;
- is transparent, shadowless, non-activating, and `ignoresMouseEvents = YES`;
- joins all Spaces and is full-screen auxiliary;
- owns one renderer view and one capture handle;
- receives center UV and physical radius from the host on every frame.

The host, not an individual pane, owns persisted position and display selection.

- [ ] **Step 4: Make the core target independent**

Remove the child-window relationship between the visual panel and the core target. Position the core target directly around `centerScreenPoint`. On `mouseDown`, store the cursor-to-center offset; on `mouseDragged`, set the new global center without clamping; on `mouseUp`, emit one persisted position callback.

- [ ] **Step 5: Rebuild panes on screen topology changes**

On `NSApplicationDidChangeScreenParametersNotification`, preserve the center if it belongs to any current display. Otherwise recover it with `PetRecoverCenter`. Recreate only the affected pane/capture set.

- [ ] **Step 6: Run native and Rust tests**

Run the commands from Task 3. Expected: all pass.

- [ ] **Step 7: Commit panes and dragging**

```bash
git add src-tauri/native/mac/pet.mm src-tauri/native/mac/pet_lifecycle.h src-tauri/native/mac/pet_lifecycle_test.cc
git commit -m "feat: add unrestricted black hole dragging"
```

---

### Task 5: Live size adjustment and safe import target

**Files:**
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/pet_lifecycle.h`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`
- Modify: `src-tauri/src/pet/input.rs`
- Modify: `src/features/settings/Pet.test.tsx`

**Interfaces:**
- Consumes: existing `PetConfig.size`, Rust import callbacks, and `BPCoreHitTargetView`.
- Produces: live radius/hit-target updates without pane/capture restart; exactly-once supported-file import.

- [ ] **Step 1: Write failing native size tests**

Add:

```cpp
static void size_changes_radius_without_changing_pane_or_center() {
  const auto small = PetEffectGeometryForSize(300.0);
  const auto large = PetEffectGeometryForSize(900.0);
  assert(close_to(small.shadow_radius, 22.5));
  assert(close_to(large.shadow_radius, 67.5));
  assert(close_to(small.hit_radius, 25.875));
  assert(close_to(large.hit_radius, 77.625));
}
```

The mutation caught is reintroducing the former 360-point drawable cap.

- [ ] **Step 2: Write a failing frontend immediate-save test**

Update the existing settings test to exercise 300, 600, and 900 and assert the exact `setPetSettings({ size })` calls. The test must use the real `Pet` component and mock only the Tauri process boundary.

- [ ] **Step 3: Verify RED**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac src-tauri/native/mac/pet_lifecycle_test.cc -o /tmp/cylune-pet-lifecycle-test
npm test -- --run src/features/settings/Pet.test.tsx
```

Expected: native geometry fails until the full-display size mapping is added.

- [ ] **Step 4: Apply size live**

On `pet_apply`, update every pane's `hole_radius_uv` and immediately resize/reposition the circular hit target around the unchanged center. Do not resize visual panes and do not restart capture.

- [ ] **Step 5: Preserve and verify the existing import path**

Keep the existing file-candidate, session-generation, circular-hit, canonicalization, and Rust `PrintService.import_print_file` flow. Ensure:

- black-hole movement does not create a file-drop event;
- only the hit-target window registers `NSPasteboardTypeFileURL`;
- an external supported-file drop produces one callback;
- rejected, stale, outside-core, directory, symlink, and unsupported drops produce none;
- the source file is untouched.

Run:

```bash
cd src-tauri
cargo test pet::input
cargo test pet::runtime
cd ..
npm test -- --run src/features/settings/Pet.test.tsx
```

Expected: all pass.

- [ ] **Step 6: Commit size and import integration**

```bash
git add src-tauri/native/mac/pet.mm src-tauri/native/mac/pet_lifecycle.h src-tauri/native/mac/pet_lifecycle_test.cc src-tauri/src/pet/input.rs src/features/settings/Pet.test.tsx
git commit -m "feat: connect black hole size and print import"
```

---

### Task 6: Attribution, application build, and visual acceptance

**Files:**
- Modify: `THIRD_PARTY_NOTICES.md`
- Modify: `docs/qa-black-hole.md`

**Interfaces:**
- Consumes: completed native integration and the provided `萨莫面具-布莱克.gcode.3mf`.
- Produces: packaged CYLUNE preview with required attribution and recorded QA evidence.

- [ ] **Step 1: Add exact MIT attribution**

Add the tiyda and s13k notices and license text while retaining existing third-party notices.

- [ ] **Step 2: Run automated verification**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac src-tauri/native/mac/pet_lifecycle_test.cc -o /tmp/cylune-pet-lifecycle-test
/tmp/cylune-pet-lifecycle-test
npm test -- --run
npm run build
cd src-tauri && cargo test
```

Expected: native, frontend, build, and Rust checks all pass without new warnings.

- [ ] **Step 3: Build and launch one CYLUNE preview**

Stop the standalone `BlackholeDesktop` preview before starting CYLUNE, build the macOS app, and verify exactly one CYLUNE process and one menu-bar item are running.

- [ ] **Step 4: Perform visual and interaction QA**

Record these results in `docs/qa-black-hole.md`:

- stationary ten-second animation matches the approved standalone preview;
- manual dragging has no crop lag or transient black rectangle;
- all four edges/corners and every connected display are reachable;
- size 300, 600, and 900 apply immediately;
- moving over Finder files imports nothing;
- dropping `/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf` into the core imports once;
- the source file still exists at the same path.

- [ ] **Step 5: Package the preview**

Replace the previous preview artifact only after the source state used to build it has passed all checks. Keep the filename `CYLUNE-preview.zip`.

- [ ] **Step 6: Commit attribution and QA evidence**

```bash
git add THIRD_PARTY_NOTICES.md docs/qa-black-hole.md
git commit -m "docs: verify approved black hole integration"
```
