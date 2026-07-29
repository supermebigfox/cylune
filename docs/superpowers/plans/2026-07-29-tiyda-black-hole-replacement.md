# CYLUNE tiyda Black Hole Replacement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace CYLUNE’s custom lens/capture black-hole stack with the rendering and live-background architecture from `tiyda/blackhole-desktop` at upstream commit `03e74a5`, while preserving CYLUNE’s fixed manual position, settings, and source-preserving file import.

**Architecture:** Vendor the upstream Metal shader and `MTKView` renderer as a small native component. `pet.mm` becomes a thin host that owns one transparent visual pane per display and one compact drag/drop target at the persisted black-hole center. The active pane uses the upstream renderer; it captures the current application background with `SCScreenshotManager` on macOS 14+, excludes CYLUNE’s own windows, and falls back to wallpaper instead of reusing a stale frame. Rust keeps its existing settings, event, validation, and import pipeline.

**Tech Stack:** Rust/Tauri, Objective-C++, Objective-C, AppKit, Metal/MetalKit, ScreenCaptureKit, XCTest-style native assertions, Cargo tests, macOS ad-hoc signing.

## Global Constraints

- The visual reference is only `tiyda/blackhole-desktop` commit `03e74a5`; do not use the earlier `rrrjqy66/BlackHoleTrash` experiment.
- Preserve the upstream ray-tracing, accretion-disk, boundary, hover-ring, and time-animation formulas. The only shader behavior change is replacing the upstream sine-driven center with a host-provided fixed center.
- Never set animation speed to zero to hold position. Position and animation time are independent.
- Remove CYLUNE’s custom `SCStream`/`IOSurface`, lens, particle, and shader implementation; do not leave a second renderer behind as a fallback.
- Maintain exactly one logical black hole globally. Each display may own a pane, but only the pane containing the persisted center draws it.
- Default visual level is above normal application windows. Moving the black hole across Finder icons never imports them.
- Import happens only after a Finder drag enters the drop target and the user releases a supported regular file. Never delete, move, or trash the source.
- Supported inputs remain `.3mf`, `.gcode`, and `.gcode.3mf`.
- Preserve settings: size `300...900`, FPS `30`, `60`, or automatic, show/hide, style, position, display, and reset.
- Map CYLUNE `Fusion` to upstream style `0` (“Default”) and `Gargantua` to upstream style `1`.
- Preserve the project’s macOS 10.15 deployment target. Gate `SCScreenshotManager` behind `@available(macOS 14.0, *)`; older systems render the animated upstream black hole over wallpaper.
- Preserve both the upstream MIT license and its third-party MIT notice in source and in the application bundle.

---

## Task 1: Vendor the Upstream Renderer and Legal Notices

**Files:**

- Create: `src-tauri/native/mac/tiyda/BlackHoleDesktop.h`
- Create: `src-tauri/native/mac/tiyda/MetalBlackHoleView.m`
- Create: `src-tauri/native/mac/tiyda/BlackHole.metal`
- Create: `src-tauri/native/mac/tiyda/LICENSE`
- Create: `src-tauri/native/mac/tiyda/THIRD_PARTY_NOTICES.md`
- Test: `src-tauri/native/mac/tiyda/source_provenance_test.sh`

- [ ] **Step 1: Write the failing source-provenance test**

Create a shell test that verifies:

```sh
test -f src-tauri/native/mac/tiyda/BlackHole.metal
test -f src-tauri/native/mac/tiyda/MetalBlackHoleView.m
grep -Fq '03e74a5' src-tauri/native/mac/tiyda/THIRD_PARTY_NOTICES.md
grep -Fq 'MIT License' src-tauri/native/mac/tiyda/LICENSE
grep -Fq 'ghostty-blackhole' src-tauri/native/mac/tiyda/THIRD_PARTY_NOTICES.md
```

- [ ] **Step 2: Run the test and confirm it fails**

Run:

```bash
sh src-tauri/native/mac/tiyda/source_provenance_test.sh
```

Expected: failure because the vendored files do not exist.

- [ ] **Step 3: Add the minimal upstream files**

Transcribe the exact renderer, shader, license, and notices from:

```text
.superpowers/sdd/2026-07-28-black-hole-import/upstream/tiyda-blackhole-desktop
```

Add a provenance paragraph to `THIRD_PARTY_NOTICES.md` naming:

```text
Source: https://github.com/tiyda/blackhole-desktop
Vendored commit: 03e74a5
Local integration: CYLUNE supplies a fixed center and imports files instead of recycling them.
```

Expose a small host API in `BlackHoleDesktop.h`:

```objc
typedef NS_ENUM(uint32_t, BHStyle) {
  BHStyleDefault = 0,
  BHStyleGargantua = 1,
};

@interface MetalBlackHoleView : MTKView
@property(nonatomic) CGPoint blackHoleCenterInScreen;
@property(nonatomic) CGFloat blackHoleSize;
@property(nonatomic) float blackHoleBrightness;
@property(nonatomic) float blackHoleSpeed;
@property(nonatomic) BHStyle blackHoleStyle;
- (void)setCaptureEnabled:(BOOL)enabled;
- (void)setTargetFramesPerSecond:(NSInteger)fps;
- (void)refreshBackgroundNow;
@end
```

- [ ] **Step 4: Run the provenance test and confirm it passes**

Run:

```bash
sh src-tauri/native/mac/tiyda/source_provenance_test.sh
```

Expected: exit `0`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/native/mac/tiyda
git commit -m "vendor: add tiyda black hole renderer"
```

---

## Task 2: Make the Upstream Black Hole Fixed in Space but Dynamically Animated

**Files:**

- Modify: `src-tauri/native/mac/tiyda/BlackHole.metal`
- Modify: `src-tauri/native/mac/tiyda/MetalBlackHoleView.m`
- Modify: `src-tauri/native/mac/tiyda/BlackHoleDesktop.h`
- Create: `src-tauri/native/mac/tiyda/black_hole_params.h`
- Create: `src-tauri/native/mac/tiyda/black_hole_params_test.cc`

- [ ] **Step 1: Write failing parameter-mapping tests**

Define a plain C++ helper that can be tested without an AppKit window:

```cpp
struct BHHostSettings {
  float centerX;
  float centerY;
  float size;
  uint32_t fpsMode;
  uint32_t cyluneStyle;
};

struct BHResolvedSettings {
  float centerX;
  float centerY;
  float size;
  uint32_t framesPerSecond;
  uint32_t upstreamStyle;
};

BHResolvedSettings BHResolveSettings(BHHostSettings input,
                                     uint32_t displayRefreshRate);
```

Assert:

- `Fusion (0)` resolves to upstream style `0`.
- `Gargantua (1)` resolves to upstream style `1`.
- explicit 30 and 60 FPS remain 30 and 60.
- automatic FPS resolves to the display rate clamped to `30...120`.
- center is unchanged by repeated resolution.
- size is clamped to `300...900`.

- [ ] **Step 2: Run the native test and confirm it fails**

Run:

```bash
clang++ -std=c++17 src-tauri/native/mac/tiyda/black_hole_params_test.cc \
  -o /tmp/cylune-black-hole-params-test
```

Expected: compilation failure because the helper does not exist.

- [ ] **Step 3: Implement fixed-center parameters**

Extend the upstream Metal parameter block:

```metal
struct RenderParams {
    float2 resolution;
    float time;
    float size;
    float brightness;
    float speed;
    int style;
    float2 center;
};
```

Replace only:

```metal
float2 center=float2(0.57+0.19*sin(t*.13), 0.62+0.12*sin(t*.17+2.0));
```

with:

```metal
float2 center = clamp(P.center, float2(0.0), float2(1.0));
```

Keep `P.time`, disk motion, ray tracing, and all style formulas unchanged. Convert the AppKit screen coordinate to pane-relative normalized Metal coordinates before writing `P.center`.

- [ ] **Step 4: Implement and pass the mapping tests**

Run:

```bash
clang++ -std=c++17 src-tauri/native/mac/tiyda/black_hole_params_test.cc \
  -o /tmp/cylune-black-hole-params-test &&
/tmp/cylune-black-hole-params-test
```

Expected: exit `0`.

- [ ] **Step 5: Add a shader contract test**

Extend `source_provenance_test.sh` to assert:

```sh
grep -Fq 'float2 center;' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'float2 center = clamp(P.center' src-tauri/native/mac/tiyda/BlackHole.metal
! grep -Fq '0.57+0.19*sin' src-tauri/native/mac/tiyda/BlackHole.metal
grep -Fq 'P.time' src-tauri/native/mac/tiyda/BlackHole.metal
```

Run the test and confirm it passes.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/native/mac/tiyda
git commit -m "feat: keep upstream black hole animated at fixed position"
```

---

## Task 3: Preserve Live Application Background Capture Without Frozen Frames

**Files:**

- Modify: `src-tauri/native/mac/tiyda/MetalBlackHoleView.m`
- Modify: `src-tauri/native/mac/tiyda/BlackHoleDesktop.h`
- Create: `src-tauri/native/mac/tiyda/capture_policy.h`
- Create: `src-tauri/native/mac/tiyda/capture_policy_test.cc`

- [ ] **Step 1: Write failing capture-policy tests**

Define:

```cpp
enum class BHCaptureResult {
  kFreshFrame,
  kPermissionDenied,
  kUnavailable,
  kTransientFailure,
};

struct BHCaptureDecision {
  bool useScreenTexture;
  bool clearPreviousScreenTexture;
  bool useWallpaperFallback;
};

BHCaptureDecision BHDecideCapture(BHCaptureResult result);
```

Assert:

- fresh frame uses the current screen texture.
- permission denied, unavailable, and transient failure clear the prior screen texture.
- every non-fresh result uses wallpaper fallback.
- no result is allowed to keep a stale captured frame.

- [ ] **Step 2: Run the native test and confirm it fails**

Run:

```bash
clang++ -std=c++17 src-tauri/native/mac/tiyda/capture_policy_test.cc \
  -o /tmp/cylune-capture-policy-test
```

Expected: compilation failure because the policy does not exist.

- [ ] **Step 3: Port the upstream capture loop**

In `MetalBlackHoleView.m`:

- update the application background approximately every `0.10` seconds;
- use `SCScreenshotManager captureImageWithFilter:configuration:completionHandler:` only inside an `@available(macOS 14.0, *)` guard;
- build the filter from the current display;
- exclude every window owned by the current process;
- use the current display scale and native pixel dimensions;
- convert each successful `CGImageRef` to a new Metal texture;
- atomically replace the prior texture only when the fresh frame succeeds;
- on permission denial or capture failure, clear the screen texture and use wallpaper;
- never preserve a failed frame as a frozen live background;
- stop the timer and discard textures when the view is hidden or deallocated.

- [ ] **Step 4: Implement and pass capture-policy tests**

Run:

```bash
clang++ -std=c++17 src-tauri/native/mac/tiyda/capture_policy_test.cc \
  -o /tmp/cylune-capture-policy-test &&
/tmp/cylune-capture-policy-test
```

Expected: exit `0`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/native/mac/tiyda
git commit -m "fix: refresh live black hole background without stale frames"
```

---

## Task 4: Replace `pet.mm` with a Thin Fixed-Position Host

**Files:**

- Replace: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/pet_drop_state.h`
- Modify: `src-tauri/native/mac/pet_lifecycle.h`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`
- Create: `src-tauri/native/mac/pet_position.h`
- Create: `src-tauri/native/mac/pet_position_test.cc`

- [ ] **Step 1: Write failing host-state tests**

Add plain C++ tests for:

- the active display is chosen from the persisted black-hole center;
- exactly one pane is active at a time;
- a manual drag clamps the target to the union of all connected display frames, not one display;
- a screen-crossing drag updates `displayId`;
- no elapsed-time/timer input can mutate position;
- reset chooses the primary display center;
- hover without an active file drag never creates a drop generation;
- a valid release creates exactly one generation and blocks the next drop until `pet_finish_drop`.

- [ ] **Step 2: Run the host tests and confirm they fail**

Run:

```bash
clang++ -std=c++17 \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  src-tauri/native/mac/pet_position_test.cc \
  -o /tmp/cylune-pet-host-test
```

Expected: compile or assertion failure until the host helpers exist.

- [ ] **Step 3: Implement the per-display visual panes**

Replace the old renderer/capture controller with:

- one borderless, transparent, non-activating `NSPanel` for each `NSScreen`;
- the normal application-floating window level, not desktop-icon level;
- `ignoresMouseEvents = YES` for visual panes;
- one `MetalBlackHoleView` filling each pane;
- only the pane containing the persisted center receives a visible center and draws the black hole;
- all panes remain clipped to their own screen, preventing capture from visually “piercing” another application layer;
- display add/remove callbacks rebuild panes and preserve the nearest valid center.

- [ ] **Step 4: Implement the manual drag/drop target**

Create one compact transparent `NSPanel` around the visible center:

- no automatic movement, sine drift, orbit, gravity, or cursor follower;
- empty-area left drag moves the center and emits `PetCallbackKind::Moved`;
- target movement never enumerates or imports desktop files underneath it;
- `NSDraggingDestination` accepts only Finder-provided file URLs during an active drag session;
- validate extensions before emitting `FileDropped`;
- copy the selected path into callback-owned storage for the duration of the callback;
- keep the target blocked until Rust calls `pet_finish_drop`;
- show the upstream absorption/hover state during `draggingEntered`, `draggingUpdated`, and `performDragOperation`;
- snap back from hover if the drag exits or is cancelled.

- [ ] **Step 5: Preserve lifecycle and settings ABI**

Implement every existing stable `pet_*` function:

```c
void *pet_create(PetCallback callback, const char *metal_source);
void pet_destroy(void *handle);
void pet_set_config(void *handle, PetConfig config);
void pet_set_visible(void *handle, bool visible);
void pet_set_position(void *handle, double x, double y, uint32_t display_id);
void pet_reset_position(void *handle);
void pet_finish_drop(void *handle, uint64_t generation, bool accepted);
uint32_t pet_capture_state(void *handle);
uint32_t pet_renderer_state(void *handle);
uint32_t pet_shutdown_state(void *handle);
```

`metal_source` now contains the vendored upstream shader. On sleep, pause drawing and capture; on wake, recreate display panes, refresh the background, and resume. Destruction must stop capture/draw timers before releasing windows.

- [ ] **Step 6: Run host tests**

Run:

```bash
clang++ -std=c++17 \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  src-tauri/native/mac/pet_position_test.cc \
  -o /tmp/cylune-pet-host-test &&
/tmp/cylune-pet-host-test
```

Expected: exit `0`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/native/mac/pet.mm \
  src-tauri/native/mac/pet_drop_state.h \
  src-tauri/native/mac/pet_lifecycle.h \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  src-tauri/native/mac/pet_position.h \
  src-tauri/native/mac/pet_position_test.cc
git commit -m "feat: host upstream black hole at manual CYLUNE position"
```

---

## Task 5: Remove the Old Renderer and Clean the Native/Rust Boundary

**Files:**

- Delete: `src-tauri/native/mac/capture.mm`
- Delete: `src-tauri/native/mac/render.mm`
- Delete: `src-tauri/native/mac/shader.metal`
- Delete: `src-tauri/native/mac/pet_render_state.h`
- Delete: `src-tauri/native/mac/pet_visual_state.h`
- Modify: `src-tauri/native/mac/bridge.h`
- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/tauri.conf.json`
- Test: `src-tauri/src/pet/native.rs`

- [ ] **Step 1: Write failing Rust source-contract tests**

Add tests that read the native/build source using `include_str!` and assert:

```rust
assert!(BUILD_RS.contains("tiyda/MetalBlackHoleView.m"));
assert!(NATIVE_RS.contains("tiyda/BlackHole.metal"));
assert!(!BUILD_RS.contains("capture.mm"));
assert!(!BUILD_RS.contains("render.mm"));
assert!(!NATIVE_RS.contains("pet_test_render_rgba"));
assert!(!BRIDGE_H.contains("mac_capture_"));
assert!(!BRIDGE_H.contains("mac_renderer_"));
```

- [ ] **Step 2: Run the focused test and confirm it fails**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml pet::native::tests -- --nocapture
```

Expected: one or more new contract assertions fail.

- [ ] **Step 3: Clean the native ABI**

Reduce `bridge.h` to the stable adapter types and `pet_*` functions. Remove:

- `PetCaptureRegion`;
- `PetRenderUniforms`;
- `PetRenderStats`;
- IOSurface includes;
- `mac_capture_*`;
- `mac_renderer_*`;
- old renderer test functions.

- [ ] **Step 4: Update the macOS build**

Compile:

```text
native/mac/pet.mm
native/mac/tiyda/MetalBlackHoleView.m
```

Track:

```text
native/mac/tiyda/BlackHole.metal
native/mac/tiyda/BlackHoleDesktop.h
```

Link AppKit, Metal, MetalKit, QuartzCore, and weak ScreenCaptureKit. Remove CoreMedia, CoreVideo, and IOSurface framework links unless the compiler proves an upstream dependency still needs one.

- [ ] **Step 5: Simplify the Rust native adapter**

Keep the runtime-facing types and `NativePet` methods stable, but:

- change `include_str!("../../native/mac/shader.metal")` to `include_str!("../../native/mac/tiyda/BlackHole.metal")`;
- remove old synthetic render structs and `pet_test_render_rgba`;
- remove tests tied to CYLUNE’s deleted custom shader;
- keep callback marshalling, generation ACK, lifecycle, position, visibility, config, and state handling.

- [ ] **Step 6: Bundle legal notices**

Add to `tauri.conf.json` bundle resources:

```json
[
  "native/mac/tiyda/LICENSE",
  "native/mac/tiyda/THIRD_PARTY_NOTICES.md"
]
```

- [ ] **Step 7: Delete the obsolete files and run focused tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml pet::native::tests -- --nocapture
```

Expected: all focused tests pass.

- [ ] **Step 8: Commit**

```bash
git add -A src-tauri/native/mac src-tauri/build.rs \
  src-tauri/src/pet/native.rs src-tauri/tauri.conf.json
git commit -m "refactor: remove legacy CYLUNE black hole stack"
```

---

## Task 6: Preserve Import Semantics and Verify the Complete Application

**Files:**

- Modify: `src-tauri/src/pet/runtime.rs` only if an integration test exposes a mismatch
- Modify: `src-tauri/src/pet/input.rs` only if an integration test exposes a mismatch
- Modify: `src-tauri/src/pet/native.rs` only if an integration test exposes a mismatch
- Test: `src-tauri/src/pet/runtime.rs`
- Test asset: `/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf`

- [ ] **Step 1: Add or confirm import regression tests**

Cover:

- a supported drop creates one pending import;
- the same generation cannot import twice;
- rejection releases the generation;
- `.gcode.3mf` is treated as supported;
- source path still exists after success and failure;
- an unsupported file never reaches the import pipeline;
- a move callback alone never imports.

- [ ] **Step 2: Run the complete automated test suite**

Run:

```bash
npm test
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
sh src-tauri/native/mac/tiyda/source_provenance_test.sh
clang++ -std=c++17 src-tauri/native/mac/tiyda/black_hole_params_test.cc \
  -o /tmp/cylune-black-hole-params-test &&
/tmp/cylune-black-hole-params-test
clang++ -std=c++17 src-tauri/native/mac/tiyda/capture_policy_test.cc \
  -o /tmp/cylune-capture-policy-test &&
/tmp/cylune-capture-policy-test
```

Expected: every command exits `0`.

- [ ] **Step 3: Build the release application once**

Run:

```bash
npm run tauri build
```

Expected bundle:

```text
src-tauri/target/release/bundle/macos/CYLUNE.app
```

- [ ] **Step 4: Sign with one fixed identity and relaunch one instance**

Quit all prior CYLUNE processes, then sign:

```bash
codesign --force --deep --sign - \
  --identifier com.robin.cylune \
  src-tauri/target/release/bundle/macos/CYLUNE.app
```

Launch exactly one instance and confirm only one matching application process and one logical black hole.

- [ ] **Step 5: Perform live visual acceptance**

Verify on every connected screen:

1. The black hole is visibly animated at rest; disk/space distortion changes over time while its center remains fixed.
2. Dragging it follows the pointer immediately with no stale-frame pause and no large rectangular/black edge.
3. A scrolling browser page or playing video beneath it changes inside the distortion within the next capture refresh; compare screenshots at least `0.3` seconds apart to prove the background, not only the shader, changed.
4. The pane is not clipped to the first display and can cross display boundaries.
5. Moving it over Finder files creates no import.
6. Dragging `/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf` into the target shows the absorption state and creates one CYLUNE import.
7. The source file remains at its original path.
8. Size `300` and `900`, FPS `30`, `60`, and automatic, both visual styles, hide/show, reset, sleep/wake, and relaunch persistence behave correctly.
9. Denying Screen Recording does not freeze the last application frame; the renderer falls back to wallpaper while remaining animated.

- [ ] **Step 6: Inspect bundle notices and final diff**

Run:

```bash
find src-tauri/target/release/bundle/macos/CYLUNE.app -type f \
  \( -name LICENSE -o -name THIRD_PARTY_NOTICES.md \) -print
git status --short
git diff --check
```

Expected: both notices are bundled, no unintended files are modified, and `git diff --check` prints nothing.

- [ ] **Step 7: Commit any integration-only corrections**

```bash
git add -A
git commit -m "test: verify CYLUNE black hole import integration"
```

