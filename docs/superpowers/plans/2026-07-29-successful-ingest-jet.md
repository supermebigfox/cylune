# CYLUNE Successful Ingest Jet Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a short blue-white bipolar jet after CYLUNE completely swallows a supported 3MF/G-code file, while preserving the existing eject path and keeping the black-hole diameter unchanged.

**Architecture:** Keep timing and branch decisions in the macOS native host, expose pure timing/effect helpers for deterministic tests, and pass one normalized success-jet progress value into the existing Metal renderer. The shader synthesizes both jet lobes inside the existing transparent full-screen pane; it does not add windows, particle processes, or source-file mutations.

**Tech Stack:** Objective-C++/AppKit drag destination, C/C++ pure helper headers, Metal Shading Language, Rust/Tauri build pipeline, shell-driven native regression tests.

## Global Constraints

- Supported inputs are `.3mf`, `.gcode.3mf`, and `.gcode`.
- Other ordinary files keep the existing 0.62-second eject animation and never show the jet.
- Swallow duration remains 0.74 seconds; jet duration is exactly 0.50 seconds.
- Hover rotation rate is 2.4× and pull gain is 1.7×; visual diameter and drop-target diameter do not change.
- The jet uses a near-white core with cool-blue/cyan falloff and no circular ripple or three-line guidance overlay.
- With no hover, ingest, eject, or success jet active, the existing black-hole colors, silhouette, accretion disk, brightness, transparent boundary, visual size, and baseline motion must remain unchanged.
- `successJetProgress == 0` must produce the same base-frame output as the pre-jet shader; the jet is additive only while its progress is greater than zero.
- The source file is never moved, deleted, overwritten, or copied by the visual layer.
- Hiding, sleeping, quitting, or starting a new drop clears an unfinished jet.

---

### Task 1: Finish the hover boost contract

**Files:**
- Modify: `src-tauri/native/mac/tiyda/black_hole_params.h`
- Modify: `src-tauri/native/mac/tiyda/black_hole_params_test.cc`
- Modify: `src-tauri/native/mac/tiyda/BlackHoleDesktop.h`
- Modify: `src-tauri/native/mac/tiyda/MetalBlackHoleView.m`
- Modify: `src-tauri/native/mac/tiyda/BlackHole.metal`
- Modify: `src-tauri/native/mac/pet.mm`

**Interfaces:**
- Produces: `BHHoverEffect BHResolveHoverEffect(float progress)`
- Produces: `double BHAdvanceAnimationTime(double animationTime, double elapsedSeconds, float rotationRate)`
- Produces: `float BHHoverVisualDiameter(float visualDiameterPixels, BHHoverEffect effect)`
- Produces: `MetalBlackHoleView.blackHolePullGain`

- [ ] **Step 1: Run the focused hover-effect test**

Run:

```bash
clang++ -std=c++17 -I src-tauri/native/mac/tiyda \
  src-tauri/native/mac/tiyda/black_hole_params_test.cc \
  -o /tmp/cylune-black-hole-params-test &&
  /tmp/cylune-black-hole-params-test
```

Expected: PASS. The current working tree already contains the red-green implementation for this approved prerequisite.

- [ ] **Step 2: Verify the host consumes the effect without changing size**

The production mapping must remain:

```objc
const BHHoverEffect hoverEffect =
    BHResolveHoverEffect(_dropHovering ? 1.0f : 0.0f);
view.blackHoleSize = BHHoverVisualDiameter(_visualSize, hoverEffect);
view.blackHoleSpeed = hoverEffect.rotationRate;
view.blackHolePullGain = hoverEffect.pullGain;
```

The shader flow gain must consume `pullGain`, while time is advanced continuously in `MetalBlackHoleView.m` so entering or leaving the target does not jump to an unrelated animation phase.

- [ ] **Step 3: Commit the hover prerequisite**

```bash
git add \
  src-tauri/native/mac/pet.mm \
  src-tauri/native/mac/tiyda/BlackHole.metal \
  src-tauri/native/mac/tiyda/BlackHoleDesktop.h \
  src-tauri/native/mac/tiyda/MetalBlackHoleView.m \
  src-tauri/native/mac/tiyda/black_hole_params.h \
  src-tauri/native/mac/tiyda/black_hole_params_test.cc
git commit -m "tune: intensify black hole while a file approaches"
```

---

### Task 2: Define success-jet timing independently of rendering

**Files:**
- Modify: `src-tauri/native/mac/pet_ingest_animation.h`
- Modify: `src-tauri/native/mac/pet_lifecycle_test.cc`

**Interfaces:**
- Produces: `constexpr double kPetSuccessJetDurationSeconds = 0.50`
- Produces: `double PetSuccessJetProgress(double elapsedSinceDrop)`
- Consumes: `kPetSwallowDurationSeconds`

- [ ] **Step 1: Write the failing timing test**

Add these assertions to `pet_lifecycle_test.cc`:

```cpp
assert(PetSuccessJetProgress(kPetSwallowDurationSeconds) == 0.0);
assert(std::abs(PetSuccessJetProgress(
                    kPetSwallowDurationSeconds +
                    kPetSuccessJetDurationSeconds * 0.5) -
                0.5) <
       1e-9);
assert(PetSuccessJetProgress(kPetSwallowDurationSeconds +
                            kPetSuccessJetDurationSeconds) == 1.0);
```

- [ ] **Step 2: Run the test and verify RED**

```bash
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/cylune-pet-lifecycle-test
```

Expected: FAIL because `kPetSuccessJetDurationSeconds` and `PetSuccessJetProgress` do not exist.

- [ ] **Step 3: Implement the normalized progress helper**

Add to `pet_ingest_animation.h`:

```cpp
constexpr double kPetSuccessJetDurationSeconds = 0.50;

inline double PetSuccessJetProgress(double elapsedSeconds) {
  if (elapsedSeconds <= kPetSwallowDurationSeconds) return 0.0;
  if (elapsedSeconds >=
      kPetSwallowDurationSeconds + kPetSuccessJetDurationSeconds) {
    return 1.0;
  }
  return PetClampUnit((elapsedSeconds - kPetSwallowDurationSeconds) /
                      kPetSuccessJetDurationSeconds);
}
```

- [ ] **Step 4: Run the test and verify GREEN**

```bash
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/cylune-pet-lifecycle-test &&
  /tmp/cylune-pet-lifecycle-test
```

Expected: PASS.

- [ ] **Step 5: Commit the timing contract**

```bash
git add src-tauri/native/mac/pet_ingest_animation.h \
  src-tauri/native/mac/pet_lifecycle_test.cc
git commit -m "test: define successful ingest jet timing"
```

---

### Task 3: Drive a supported-only success jet from the native drop host

**Files:**
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/tiyda/BlackHoleDesktop.h`
- Modify: `src-tauri/native/mac/tiyda/MetalBlackHoleView.m`
- Modify: `src-tauri/native/mac/tiyda/source_provenance_test.sh`

**Interfaces:**
- Produces: `MetalBlackHoleView.blackHoleSuccessJetProgress`
- Produces: `-[BHPetHost setSuccessJetProgress:]`
- Consumes: `PetSuccessJetProgress(double elapsedSeconds)`
- Consumes: `PET_FILE_3MF`, `PET_FILE_GCODE`, and `PET_FILE_OTHER`

- [ ] **Step 1: Add a failing native source contract**

Extend `source_provenance_test.sh`:

```bash
grep -Fq 'blackHoleSuccessJetProgress' \
  src-tauri/native/mac/tiyda/BlackHoleDesktop.h
grep -Fq 'successJetProgress' \
  src-tauri/native/mac/tiyda/MetalBlackHoleView.m
```

- [ ] **Step 2: Run the contract and verify RED**

```bash
bash src-tauri/native/mac/tiyda/source_provenance_test.sh
```

Expected: FAIL because the success-jet parameter is not present.

- [ ] **Step 3: Add the renderer property and CPU parameter**

Add to `BlackHoleDesktop.h`:

```objc
@property(nonatomic) float blackHoleSuccessJetProgress;
```

Append the matching float to `RenderParams` in `MetalBlackHoleView.m`, initialize the property to zero, and populate:

```objc
.successJetProgress = _blackHoleSuccessJetProgress,
```

- [ ] **Step 4: Separate the jet lifetime from the import acknowledgement**

Add host state in `pet.mm`:

```objc
BOOL _successJetActive;
CFAbsoluteTime _successJetStartedAt;
CGFloat _successJetProgress;
BOOL _importCallbackSent;
```

At the start of every accepted drop, clear the prior jet and set `_importCallbackSent = NO`.

When the swallow reaches 0.74 seconds for a supported file:

```objc
if (!_importCallbackSent) {
  _importCallbackSent = YES;
  _successJetActive = YES;
  _successJetStartedAt = CFAbsoluteTimeGetCurrent();
  _callback(kPetCallbackFileDropped,
            _pendingDropPath.fileSystemRepresentation, 0, 0,
            _pendingDropGeneration);
}
```

While `_successJetActive`, update its normalized progress every timer tick. The import acknowledgement may clear the drop session and file icon, but it must not stop the jet. At 0.50 seconds, set progress to zero, mark the jet inactive, and invalidate the timer if no ingest/eject work remains.

The unsupported branch must continue directly from swallow to eject and must never set `_successJetActive`.

- [ ] **Step 5: Clear the jet on lifecycle interruption**

Call the same zeroing helper from:

- `hide`
- `workspaceWillSleep:`
- `shutdown`
- the beginning of a new accepted drop

Do not alter `_visualSize`, `PetDropTargetSide`, or the source URL.

- [ ] **Step 6: Run native tests and verify GREEN**

```bash
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/cylune-pet-lifecycle-test &&
  /tmp/cylune-pet-lifecycle-test &&
  bash src-tauri/native/mac/tiyda/source_provenance_test.sh
```

Expected: PASS.

- [ ] **Step 7: Commit native jet state**

```bash
git add \
  src-tauri/native/mac/pet.mm \
  src-tauri/native/mac/tiyda/BlackHoleDesktop.h \
  src-tauri/native/mac/tiyda/MetalBlackHoleView.m \
  src-tauri/native/mac/tiyda/source_provenance_test.sh
git commit -m "feat: drive a successful ingest jet"
```

---

### Task 4: Render the blue-white bipolar jet in Metal

**Files:**
- Modify: `src-tauri/native/mac/tiyda/BlackHole.metal`
- Modify: `src-tauri/native/mac/tiyda/source_provenance_test.sh`

**Interfaces:**
- Consumes: `Params.successJetProgress`
- Produces: blue-white premultiplied-looking RGB contribution and expanded alpha only while the progress is between zero and one

- [ ] **Step 1: Append the Metal parameter with the same CPU order**

First add the shader contract:

```bash
grep -Fq 'successJetProgress' src-tauri/native/mac/tiyda/BlackHole.metal
```

Run `bash src-tauri/native/mac/tiyda/source_provenance_test.sh` and verify it fails before changing the shader.

The `Params` tail must be:

```metal
float ingestProgress, ejectProgress, pullGain, successJetProgress;
```

- [ ] **Step 2: Add a bounded bipolar-jet helper**

Add a helper that derives two tapered lobes from black-hole-local coordinates:

```metal
float4 successJet(float2 p, float rh, float t, float progress) {
    float burst=sin(3.14159265*clamp(progress,0.0,1.0));
    float2 axis=normalize(float2(-0.12,1.0));
    float2 tangent=float2(axis.y,-axis.x);
    float axial=dot(p,axis)/max(rh,.0001);
    float lateral=abs(dot(p,tangent))/max(rh,.0001);
    float reach=.85+4.25*smoothstep(0.0,.58,progress);
    float coneWidth=.055+.055*abs(axial);
    float core=exp(-pow(lateral/max(coneWidth,.001),2.0)*3.4);
    float lobe=smoothstep(.55,.88,abs(axial))*
               (1.0-smoothstep(reach-.45,reach,abs(axial)));
    float particles=.58+.42*noise(float2(lateral*34.0-t*5.2,
                                         abs(axial)*7.0-t*9.0));
    float alpha=core*lobe*particles*burst;
    float whiteCore=exp(-pow(lateral/max(coneWidth*.38,.001),2.0)*4.0);
    float3 color=mix(float3(.04,.32,1.0),float3(.72,.94,1.0),whiteCore);
    return float4(color*alpha,alpha);
}
```

- [ ] **Step 3: Composite the jet on every fragment return path**

Evaluate `successJet` before the ray-tracing early return. Replace the transparent cutoff with `if (mask < 0.002 && jet.a < 0.002) return float4(0);`. Combine the jet RGB after every desktop sample and set final alpha to `max(mask, jet.a)` so the lobes can extend beyond the accretion disk without expanding `rh` or `P.size`.

Do not change any existing preset, base color, disk, lensing, mask, radius, brightness, or time expression. When `P.successJetProgress` is zero, `jet.rgb` and `jet.a` must both be exactly zero, making each revised return algebraically equivalent to its pre-jet result.

For the final return:

```metal
float4 jet=successJet(p,rh,t,P.successJetProgress);
float3 finalColor=mix(lit,float3(0),shadowEdge)+jet.rgb;
return float4(finalColor,max(mask,jet.a));
```

Apply the same additive jet contribution to the far-field early return.

- [ ] **Step 4: Build the macOS app to compile the shader**

```bash
npm run tauri build -- --bundles app
```

Expected: release build exits zero and produces `src-tauri/target/release/bundle/macos/CYLUNE.app`.

- [ ] **Step 5: Commit the shader**

```bash
git add src-tauri/native/mac/tiyda/BlackHole.metal \
  src-tauri/native/mac/tiyda/source_provenance_test.sh
git commit -m "feat: render a blue-white bipolar ingest jet"
```

---

### Task 5: Regression, signing, and preview handoff

**Files:**
- Verify only: application source and bundle

**Interfaces:**
- Consumes: completed Tasks 1–4
- Produces: one signed and running local preview at `src-tauri/target/release/bundle/macos/CYLUNE.app`

- [ ] **Step 1: Run the full regression suite**

```bash
cd src-tauri && cargo test
cd .. && npm test -- --run
clang++ -std=c++17 -I src-tauri/native/mac/tiyda \
  src-tauri/native/mac/tiyda/black_hole_params_test.cc \
  -o /tmp/cylune-black-hole-params-test &&
  /tmp/cylune-black-hole-params-test
clang++ -std=c++17 -I src-tauri/native/mac \
  src-tauri/native/mac/pet_lifecycle_test.cc \
  -o /tmp/cylune-pet-lifecycle-test &&
  /tmp/cylune-pet-lifecycle-test
bash src-tauri/native/mac/tiyda/source_provenance_test.sh
git diff --check
```

Expected: Rust reports 128 passed and 1 environment-dependent test ignored; Vitest reports 73 passed; both native executables and the source contract exit zero.

- [ ] **Step 2: Build and ad-hoc sign the preview**

```bash
npm run tauri build -- --bundles app
codesign --force --deep --sign - --identifier com.robin.cylune \
  src-tauri/target/release/bundle/macos/CYLUNE.app
codesign --verify --deep --strict --verbose=2 \
  src-tauri/target/release/bundle/macos/CYLUNE.app
```

Expected: bundle is valid on disk and satisfies its designated requirement.

- [ ] **Step 3: Launch exactly one preview instance**

```bash
open src-tauri/target/release/bundle/macos/CYLUNE.app
```

Use Computer Use to close the main CYLUNE window while leaving the desktop black hole running. Confirm one exact bundle executable process.

- [ ] **Step 4: Manual visual acceptance**

Use the user-owned sliced file:

```text
/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf
```

Verify:

- approaching increases speed and pull without changing diameter;
- dropping the sliced file swallows it clockwise and emits one blue-white bipolar jet;
- dropping a PNG or text file swallows then ejects it without a jet;
- leaving the target restores normal motion;
- the source files remain unchanged.
