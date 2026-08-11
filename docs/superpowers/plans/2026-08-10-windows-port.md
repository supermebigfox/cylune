# CYLUNE Windows Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a Windows 10 22H2/Windows 11 x64 CYLUNE installer whose core features and desktop black-hole behavior match the sealed macOS release without changing `src-tauri/native/mac/**`.

**Architecture:** Keep the React/Tauri UI, Rust business services, and stable `PetNativeConfig` C ABI. Add a Windows-only Win32 shell under `src-tauri/native/windows`, using Direct3D 11, DXGI Desktop Duplication, DirectComposition, HLSL, and OLE drag/drop; isolate Windows packaging and Bambu Studio discovery behind platform-specific configuration and pure testable helpers.

**Tech Stack:** React 18, TypeScript, Vitest, Tauri 2, Rust 2021, C++17, Win32, Direct3D 11, DXGI 1.2, DirectComposition, HLSL shader model 5, NSIS.

## Global Constraints

- `src-tauri/native/mac/**` is read-only and its Git tree hash must not change.
- Windows source lives in `src-tauri/native/windows/**` and is compiled only for `target_os = "windows"`.
- Minimum supported OS is Windows 10 22H2 x64; Windows 11 x64 is the primary target.
- The first distributable is an NSIS setup executable; MSI follows only after the NSIS release is stable.
- Background distortion must consume a live GPU desktop texture, never a periodic CPU screenshot.
- Unsupported capture degrades to animated particles without holding a stale desktop frame.
- Existing 3MF/G-code parsing, settlement, database, and macOS tests remain green.
- Do not commit `node_modules`, Rust targets, private slice output, recordings, `result.json`, or user media.

---

### Task 0: Seal the copied functional baseline on the Windows branch

**Files:**
- Commit unchanged copies already present in: `src-tauri/src/parser/gcode.rs`
- Commit unchanged copies already present in: `src-tauri/src/parser/three_mf.rs`
- Commit unchanged copies already present in: `src-tauri/src/slicer/project.rs`
- Commit unchanged copies already present in: `src-tauri/src/slicer/runtime.rs`
- Commit unchanged copies already present in: `src/features/slice/Slice.test.tsx`
- Commit unchanged copies already present in: `src/features/slice/Slice.tsx`

**Interfaces:**
- Consumes: the six verified but uncommitted files copied from the sealed macOS workspace.
- Produces: a clean Windows-branch baseline containing the current parser and slicing behavior.

- [ ] **Step 1: Verify the copied diff is byte-for-byte equal to the macOS workspace diff**

Run `cmp` once for each of the six explicit source/target pairs:

```bash
cmp /Users/robin/Desktop/耗材管理/src-tauri/src/parser/gcode.rs src-tauri/src/parser/gcode.rs
cmp /Users/robin/Desktop/耗材管理/src-tauri/src/parser/three_mf.rs src-tauri/src/parser/three_mf.rs
cmp /Users/robin/Desktop/耗材管理/src-tauri/src/slicer/project.rs src-tauri/src/slicer/project.rs
cmp /Users/robin/Desktop/耗材管理/src-tauri/src/slicer/runtime.rs src-tauri/src/slicer/runtime.rs
cmp /Users/robin/Desktop/耗材管理/src/features/slice/Slice.test.tsx src/features/slice/Slice.test.tsx
cmp /Users/robin/Desktop/耗材管理/src/features/slice/Slice.tsx src/features/slice/Slice.tsx
```

Expected: all six `cmp` commands exit 0 and print nothing.

- [ ] **Step 2: Run the established baseline**

Run:

```bash
npm test -- --run
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: 213 frontend tests pass; 302 Rust tests pass and 7 environment tests remain ignored.

- [ ] **Step 3: Commit only the six baseline files**

```bash
git add src-tauri/src/parser/gcode.rs src-tauri/src/parser/three_mf.rs \
  src-tauri/src/slicer/project.rs src-tauri/src/slicer/runtime.rs \
  src/features/slice/Slice.test.tsx src/features/slice/Slice.tsx
git commit -m "fix: preserve current slicing and multicolor parsing"
```

### Task 1: Add isolated Windows configuration and release output

**Files:**
- Create: `src-tauri/tauri.windows.conf.json`
- Create: `scripts/release-windows.mjs`
- Create: `scripts/release-windows.test.mjs`
- Modify: `package.json`
- Test: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/tray.rs`

**Interfaces:**
- Consumes: Tauri automatic platform-config merge and `rustTargetDir()`.
- Produces: `releaseWindowsBundle({ bundleRoot, releaseRoot })` and `npm run release:windows`.
- Produces: `tray_icon_bytes(TrayPlatform)` so Windows uses the full-color product icon while macOS keeps `trayTemplate.png`.

- [ ] **Step 1: Write failing configuration and release-script tests**

Add a Rust test that parses both config files and asserts:

```rust
#[test]
fn windows_bundle_is_isolated_from_the_sealed_macos_targets() {
    let base: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let windows: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.windows.conf.json")).unwrap();
    assert_eq!(base["bundle"]["targets"], serde_json::json!(["app", "dmg"]));
    assert_eq!(windows["bundle"]["targets"], serde_json::json!(["nsis"]));
    assert_eq!(windows["bundle"]["windows"]["nsis"]["installMode"], "currentUser");
}
```

Add a pure tray asset test:

```rust
#[test]
fn each_desktop_platform_keeps_its_intended_tray_art() {
    assert_eq!(tray_icon_bytes(TrayPlatform::MacOs), include_bytes!("../icons/trayTemplate.png"));
    assert_eq!(tray_icon_bytes(TrayPlatform::Windows), include_bytes!("../icons/icon.png"));
}
```

Create `scripts/release-windows.test.mjs`:

```javascript
import { mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { expect, test } from "vitest";
import { releaseWindowsBundle } from "./release-windows.mjs";

test("publishes the one NSIS setup without leaving target artifacts", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const releaseRoot = join(root, "发布-Windows");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  await writeFile(join(bundleRoot, "nsis", "CYLUNE_0.1.0_x64-setup.exe"), "fixture");
  const published = await releaseWindowsBundle({ bundleRoot, releaseRoot });
  expect(published).toBe(join(releaseRoot, "CYLUNE-Setup.exe"));
  expect(await readFile(published, "utf8")).toBe("fixture");
});
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
npm test -- --run scripts/release-windows.test.mjs
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml \
  config_tests::windows_bundle_is_isolated_from_the_sealed_macos_targets
```

Expected: both fail because the files and export do not exist.

- [ ] **Step 3: Implement the platform config and publisher**

Create `src-tauri/tauri.windows.conf.json` with:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "bundle": {
    "targets": ["nsis"],
    "windows": {
      "nsis": {
        "installMode": "currentUser"
      }
    }
  }
}
```

Implement `releaseWindowsBundle` so it requires exactly one `*-setup.exe`, copies it to `发布-Windows/CYLUNE-Setup.exe`, and removes only the source bundle file. Add:

```json
"release:windows": "node scripts/release-windows.mjs"
```

to `package.json`.

Add `TrayPlatform` and select it with `cfg!(target_os = "windows")` inside `tray::setup`. Do not change menu labels, click behavior, single-instance handling, or the macOS template bytes.

- [ ] **Step 4: Run focused and full tests**

Run:

```bash
npm test -- --run scripts/release-windows.test.mjs scripts/release-mac.test.mjs
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml \
  config_tests tray::tests::each_desktop_platform
```

Expected: all pass and `release-mac.test.mjs` remains unchanged.

- [ ] **Step 5: Commit**

```bash
git add package.json scripts/release-windows.mjs scripts/release-windows.test.mjs \
  src-tauri/tauri.windows.conf.json src-tauri/src/lib.rs
git add src-tauri/src/tray.rs
git commit -m "build: add isolated Windows release configuration"
```

### Task 2: Make Bambu Studio discovery platform-specific

**Files:**
- Create: `src-tauri/src/slicer/install_layout.rs`
- Modify: `src-tauri/src/slicer/mod.rs`
- Modify: `src-tauri/src/slicer/discovery.rs`
- Modify: `src-tauri/src/slicer/runtime.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/features/slice/Slice.tsx`
- Modify: `src/features/slice/Slice.test.tsx`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/slicer/install_layout.rs`
- Test: `src-tauri/src/slicer/discovery.rs`

**Interfaces:**
- Produces: `InstallPlatform::{MacOs, Windows}`, `resolve_selected_install(path, platform) -> Result<BambuInstallation>`, and Windows registry candidate enumeration.
- Produces: `set_bambu_studio_path(path: String)`, which is exposed only when the backend reports Windows and persists a validated `BambuStudio.exe` path.
- Preserves: `InstallationDiscovery::new(explicit_app)` and `BambuInstallation { executable, profiles_root }` for all callers.

- [ ] **Step 1: Write failing pure layout tests**

Create fixtures and assert these exact layouts:

```rust
#[test]
fn resolves_a_windows_executable_and_neighbor_resources() {
    let fixture = InstallFixture::windows("C:/Program Files/Bambu Studio");
    let found = resolve_selected_install(&fixture.executable, InstallPlatform::Windows).unwrap();
    assert_eq!(found.executable, fixture.executable.canonicalize().unwrap());
    assert_eq!(
        found.profiles_root,
        fixture.root.join("resources/profiles").canonicalize().unwrap()
    );
}

#[test]
fn macos_bundle_resolution_stays_unchanged() {
    let fixture = InstallFixture::macos();
    let found = resolve_selected_install(&fixture.app, InstallPlatform::MacOs).unwrap();
    assert_eq!(found.executable, fixture.app.join("Contents/MacOS/BambuStudio").canonicalize().unwrap());
}
```

Also test a Windows install directory, missing profiles, symlink/reparse-point rejection, and fallback candidate order.

Add a Slice component test that receives `bambu_studio_missing` on Windows, chooses `C:\\Apps\\Bambu Studio\\BambuStudio.exe`, invokes `set_bambu_studio_path` once, and retries slicing. Add a second test proving the chooser is absent when the backend platform is `macos`.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml slicer::install_layout
```

Expected: compile failure because `install_layout` does not exist.

- [ ] **Step 3: Implement pure layout resolution**

Use this public contract:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPlatform {
    MacOs,
    Windows,
}

pub fn resolve_selected_install(
    selected: &Path,
    platform: InstallPlatform,
) -> Result<BambuInstallation>;
```

For Windows, accept either `BambuStudio.exe` or its installation directory and require `resources/profiles` beside the executable. For macOS, preserve `Contents/MacOS/BambuStudio` and `Contents/Resources/profiles` exactly.

- [ ] **Step 4: Add Windows candidate discovery**

Under `[target.'cfg(windows)'.dependencies]` add `winreg = "0.55"`. Query uninstall keys in HKCU and HKLM, then append standard candidates under `ProgramFiles` and `LOCALAPPDATA`. Canonicalize and validate every result before returning it; manual selection remains first priority.

Add `get_desktop_platform() -> "macos" | "windows" | "unsupported"` and `set_bambu_studio_path(path)` Tauri commands. Validation must finish before writing `bambu_studio_path` into `app_settings` or updating `SlicerService`. On startup, read the saved setting and pass it to `SlicerService::for_app`. In `Slice.tsx`, show the executable chooser only when the returned platform is `windows` and slicing failed with `bambu_studio_missing`; the macOS render tree remains unchanged.

- [ ] **Step 5: Run full slicer tests**

Run:

```bash
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml slicer::
npm test -- --run src/features/slice/Slice.test.tsx
```

Expected: all slicer tests pass, including the unchanged macOS fixture expectations.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock \
  src-tauri/src/slicer/mod.rs src-tauri/src/slicer/discovery.rs \
  src-tauri/src/slicer/install_layout.rs src-tauri/src/slicer/runtime.rs \
  src-tauri/src/lib.rs src/features/slice/Slice.tsx \
  src/features/slice/Slice.test.tsx
git commit -m "feat: discover Bambu Studio on Windows"
```

### Task 3: Hide Windows slicer processes and preserve cancellation

**Files:**
- Create: `src-tauri/src/slicer/process_options.rs`
- Modify: `src-tauri/src/slicer/mod.rs`
- Modify: `src-tauri/src/slicer/runtime.rs`
- Test: `src-tauri/src/slicer/process_options.rs`
- Test: `src-tauri/src/slicer/runtime.rs`

**Interfaces:**
- Produces: `configure_background_command(command: &mut Command, platform: ProcessPlatform)`.
- Consumes: existing `RunningSlice` cancellation and private task-directory lifecycle.

- [ ] **Step 1: Write failing command-policy tests**

```rust
#[test]
fn windows_cli_uses_no_console_window() {
    assert_eq!(
        creation_flags(ProcessPlatform::Windows),
        CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
    );
}

#[test]
fn macos_cli_requires_no_windows_flags() {
    assert_eq!(creation_flags(ProcessPlatform::MacOs), 0);
}
```

Add a runtime test whose fixture process creates a child process; cancellation must terminate the process tree, emit `Cancelled` once, and remove the task directory.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml \
  slicer::process_options slicer::runtime::tests::windows_cancellation
```

Expected: missing module/test implementation.

- [ ] **Step 3: Implement the process policy**

On Windows call `CommandExt::creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP)` for CLI slicing and `CREATE_NEW_PROCESS_GROUP` for the explicit “open in Bambu Studio” GUI action. Store the Windows process ID in `RunningSlice` and terminate its job/process tree on cancel; keep the current Unix `Child::kill` path unchanged.

- [ ] **Step 4: Verify cancellation and cleanup**

Run:

```bash
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml \
  slicer::runtime::tests::cancellation
```

Expected: cancellation remains bounded to ten seconds, no import occurs, and all private output is removed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/slicer/mod.rs src-tauri/src/slicer/runtime.rs \
  src-tauri/src/slicer/process_options.rs
git commit -m "feat: run and cancel Windows slicing in background"
```

### Task 4: Add the Windows native ABI and build boundary

**Files:**
- Create: `src-tauri/native/windows/bridge.h`
- Create: `src-tauri/native/windows/pet_bridge.cpp`
- Create: `src-tauri/native/windows/pet_bridge_test.cc`
- Create: `src-tauri/native/windows/BlackHole.hlsl`
- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/src/pet/native.rs`
- Test: `src-tauri/src/pet/native.rs`

**Interfaces:**
- Produces the existing C ABI symbols: `pet_create`, `pet_destroy`, `pet_apply`, `pet_show`, `pet_hide`, `pet_reset`, `pet_signal`, `pet_finish_drop`, `pet_capture_state`, `pet_renderer_state`, `pet_abi_version`.
- Consumes the unchanged 64-byte `PetNativeConfig` and callback kinds 1 through 10.

- [ ] **Step 1: Write failing ABI-boundary tests**

Add Rust source-boundary assertions:

```rust
#[test]
fn windows_and_macos_use_separate_native_sources() {
    const BUILD_RS: &str = include_str!("../../build.rs");
    const WINDOWS_BRIDGE: &str = include_str!("../../native/windows/bridge.h");
    assert!(BUILD_RS.contains("native/windows/pet_bridge.cpp"));
    assert!(WINDOWS_BRIDGE.contains("typedef struct"));
    assert!(WINDOWS_BRIDGE.contains("uint8_t visual_style"));
    assert!(!WINDOWS_BRIDGE.contains("metal_source"));
    assert!(!WINDOWS_BRIDGE.contains("MetalBlackHoleView"));
}
```

Create a C++ static-layout test:

```cpp
static_assert(sizeof(PetConfig) == 64);
static_assert(offsetof(PetConfig, display_id) == 40);
static_assert(offsetof(PetConfig, visual_style) == 62);
int main() { return pet_abi_version() == 1 ? 0 : 1; }
```

- [ ] **Step 2: Run and verify RED**

Run:

```bash
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml \
  pet::native::tests::windows_and_macos_use_separate_native_sources
```

Expected: missing Windows bridge files.

- [ ] **Step 3: Implement cfg-isolated Rust FFI**

Keep the body of the existing `#[cfg(target_os = "macos")] mod platform` byte-for-byte. Add a `#[cfg(target_os = "windows")] mod platform` implementing the same nine `Handle` methods and `abi_version()`. Change only the fallback attribute to `#[cfg(not(any(target_os = "macos", target_os = "windows")))]`. The Windows `Handle::new` embeds `../../native/windows/BlackHole.hlsl` and passes it through the same creation boundary.

- [ ] **Step 4: Compile the Windows bridge only on Windows**

In `build.rs` add a `windows` branch that compiles C++17 sources and links `user32`, `ole32`, `shell32`, `d3d11`, `dxgi`, `dcomp`, `dwmapi` and `d3dcompiler`. Keep the complete existing `macos` branch unchanged.

- [ ] **Step 5: Verify Mac and Windows compilation gates**

Run on macOS:

```bash
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml pet::native
git diff d640d92 -- src-tauri/native/mac
```

Expected: native tests pass and the second command prints nothing.

Run on Windows:

```powershell
npm run test:rust -- pet::native
```

Expected: the Windows C++ bridge links and reports ABI version 1.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/build.rs src-tauri/src/pet/native.rs src-tauri/native/windows
git commit -m "feat: add Windows native pet ABI"
```

### Task 5: Implement the Win32 transparent pet window and display geometry

**Files:**
- Create: `src-tauri/native/windows/window.h`
- Create: `src-tauri/native/windows/window.cpp`
- Create: `src-tauri/native/windows/window_state.h`
- Create: `src-tauri/native/windows/window_state_test.cc`
- Modify: `src-tauri/native/windows/pet_bridge.cpp`
- Modify: `src-tauri/build.rs`

**Interfaces:**
- Produces: `PetWindow::create(callback)`, `apply(config)`, `show()`, `hide()`, `reset()`, and a message-loop owner thread.
- Emits: Clicked, Moved, DisplayChanged, Sleep, and Wake using the established callback values.

- [ ] **Step 1: Write failing pure geometry and hit-test tests**

```cpp
int main() {
  const DisplayInfo left{1, -1920, 0, 1920, 1080, 1.0};
  const DisplayInfo right{2, 0, 0, 3840, 2160, 2.0};
  const auto placed = ClampPetOrigin({3700, 2080}, 600, {left, right});
  assert(placed.displayId == 2);
  assert(placed.x <= 3840 - 600 - 16);
  assert(HitTestPet({300, 300}, 600) == PetHit::Drag);
  assert(HitTestPet({5, 5}, 600) == PetHit::Transparent);
}
```

Add tests for negative monitor coordinates, per-monitor DPI conversion, display removal, 300/900 size limits, and finite coordinates.

- [ ] **Step 2: Compile and verify RED on Windows**

Run:

```powershell
cl /std:c++17 /EHsc src-tauri/native/windows/window_state_test.cc /Fe:$env:TEMP\cylune-window-state.exe
```

Expected: missing `window_state.h`.

- [ ] **Step 3: Implement the pure window state**

Define:

```cpp
enum class PetHit { Transparent, Drag };
struct LogicalPoint { double x; double y; };
struct DisplayInfo {
  uint64_t id;
  double x;
  double y;
  double width;
  double height;
  double scale;
};
struct Placement {
  uint64_t displayId;
  double x;
  double y;
  double size;
};
Placement ClampPetOrigin(LogicalPoint origin, double size,
                         const std::vector<DisplayInfo>& displays);
PetHit HitTestPet(LogicalPoint point, double side);
```

Use the same 16 logical-pixel safe inset and center-weighted circular hit target as the macOS behavior.

- [ ] **Step 4: Implement the Win32 owner window**

Create a dedicated STA thread, call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` before window creation, and use:

```cpp
const DWORD style = WS_POPUP;
const DWORD exStyle = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
```

Return `HTTRANSPARENT` outside the interactive region and `HTCLIENT` inside. Handle `WM_DPICHANGED`, `WM_DISPLAYCHANGE`, `WM_POWERBROADCAST`, pointer drag messages, and explicit shutdown. Apply `WDA_EXCLUDEFROMCAPTURE` after creating the top-level HWND.

- [ ] **Step 5: Verify lifecycle behavior**

Run the C++ test and `npm run test:rust -- pet::runtime` on Windows. Expected: moving emits positions in logical coordinates, hiding stops frame scheduling, and shutdown joins the owner thread within the existing bound.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/native/windows/window.h src-tauri/native/windows/window.cpp \
  src-tauri/native/windows/window_state.h src-tauri/native/windows/window_state_test.cc \
  src-tauri/native/windows/pet_bridge.cpp src-tauri/build.rs
git commit -m "feat: add Windows desktop pet window"
```

### Task 6: Implement OLE drag/drop and generation-safe ingest state

**Files:**
- Create: `src-tauri/native/windows/drop_target.h`
- Create: `src-tauri/native/windows/drop_target.cpp`
- Create: `src-tauri/native/windows/drop_state.h`
- Create: `src-tauri/native/windows/drop_state_test.cc`
- Modify: `src-tauri/native/windows/window.cpp`
- Modify: `src-tauri/native/windows/pet_bridge.cpp`
- Modify: `src-tauri/build.rs`

**Interfaces:**
- Produces: an `IDropTarget` registered only on the black-hole HWND and `DropSession::{enter, leave, submit, finish}`.
- Emits: DropEntered, DropExited, and FileDropped with UTF-8 absolute paths and a nonzero generation.

- [ ] **Step 1: Write failing state-machine tests**

```cpp
int main() {
  DropSession state;
  const auto generation = state.enter(L"C:\\prints\\mask.3mf", FileKind::ThreeMf);
  assert(generation != 0);
  assert(state.submit(generation, L"C:\\prints\\mask.3mf"));
  assert(!state.enter(L"C:\\prints\\second.3mf", FileKind::ThreeMf));
  assert(!state.finish(generation + 1, PET_DROP_ACCEPTED));
  assert(state.finish(generation, PET_DROP_ACCEPTED));
  assert(!state.waitingForAck());
}
```

Add tests proving that any regular file receives an ingest generation, unsupported files can finish as rejected, stale acknowledgements are ignored, and window movement never creates a drop session.

- [ ] **Step 2: Compile and verify RED**

Run:

```powershell
cl /std:c++17 /EHsc src-tauri/native/windows/drop_state_test.cc /Fe:$env:TEMP\cylune-drop-state.exe
```

Expected: missing `drop_state.h`.

- [ ] **Step 3: Implement OLE extraction and exact hit acceptance**

Implement `IDropTarget::DragEnter/DragOver/DragLeave/Drop`. Read `CF_HDROP`, accept only absolute ordinary files, convert UTF-16 paths with `WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, ...)`, and call the callback only when the pointer lies inside `PetDropTargetSide(size) * 0.48`. Return `DROPEFFECT_COPY` for a valid file and `DROPEFFECT_NONE` otherwise.

- [ ] **Step 4: Connect acknowledgements and visual activity**

`pet_finish_drop(generation, result)` must finish only the current generation. Accepted results trigger swallow + success jet; rejected results trigger swallow + eject. `DragLeave` cancels hover without playing an animation. Hover changes target FPS to 60 without changing size.

- [ ] **Step 5: Verify business integration**

Run:

```bash
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml \
  pet::input pet::runtime::tests::rejected_import_ack \
  pet::runtime::tests::successful_pet_import
```

Expected: moving the pet never imports, supported files reach existing import/slice routing, and unsupported files do not change balances.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/native/windows/drop_target.h src-tauri/native/windows/drop_target.cpp \
  src-tauri/native/windows/drop_state.h src-tauri/native/windows/drop_state_test.cc \
  src-tauri/native/windows/window.cpp src-tauri/native/windows/pet_bridge.cpp \
  src-tauri/build.rs
git commit -m "feat: add Windows black-hole file ingestion"
```

### Task 7: Add Direct3D 11 and DirectComposition rendering

**Files:**
- Create: `src-tauri/native/windows/renderer.h`
- Create: `src-tauri/native/windows/renderer.cpp`
- Create: `src-tauri/native/windows/render_state.h`
- Create: `src-tauri/native/windows/render_state_test.cc`
- Modify: `src-tauri/native/windows/BlackHole.hlsl`
- Modify: `src-tauri/native/windows/window.cpp`
- Modify: `src-tauri/native/windows/pet_bridge.cpp`
- Modify: `src-tauri/build.rs`

**Interfaces:**
- Produces: `BlackHoleRenderer::create(hwnd, hlsl)`, `resize(pixelWidth, pixelHeight)`, `render(frame)`, `setVisible(bool)`, and `shutdown()`.
- Consumes: premultiplied-alpha swap chain, `PetNativeConfig`, animation time, hover progress, ingest progress, pending count, and optional desktop `ID3D11ShaderResourceView`.

- [ ] **Step 1: Write failing renderer-state tests**

```cpp
int main() {
  RenderState state;
  state.apply(Config{.fps = 0, .visible = true});
  assert(state.targetFps(60) == 30);
  state.setHover(true);
  assert(state.targetFps(60) == 60);
  assert(state.visualDiameter() == state.configuredDiameter());
  state.setVisible(false);
  assert(state.targetFps(60) == 0);
}
```

Add deterministic tests for 30/60/automatic FPS, elapsed-time clamp at 0.1 seconds, clockwise positive animation time, size 300–900, hover pull gain 1.0–1.7, and hover rotation 1.0–2.4.

- [ ] **Step 2: Verify RED**

Compile `render_state_test.cc` with MSVC. Expected: missing `render_state.h`.

- [ ] **Step 3: Create the transparent composition pipeline**

Create a D3D11 device with BGRA support, a `DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL` composition swap chain using `DXGI_FORMAT_B8G8R8A8_UNORM` and `DXGI_ALPHA_MODE_PREMULTIPLIED`, attach it to an `IDCompositionVisual` and `IDCompositionTarget`, and clear every frame to transparent black before drawing a full-screen triangle.

- [ ] **Step 4: Compile HLSL at runtime**

Pass the embedded source from Rust and compile `vs_main` and `ps_main` with `D3DCompile` for `vs_5_0` and `ps_5_0`. Compilation errors set renderer state to unavailable and are logged without exposing shader source to the UI.

- [ ] **Step 5: Port the sealed Metal math without redesign**

Map Metal types and functions directly:

| Metal | HLSL |
|---|---|
| `float2/float3/float4` | `float2/float3/float4` |
| `mix(a,b,t)` | `lerp(a,b,t)` |
| `fract(v)` | `frac(v)` |
| `atan2(y,x)` | `atan2(y,x)` |
| `texture.sample(sampler, uv)` | `texture.Sample(samplerState, uv)` |

Keep all numeric constants, Gargantua/Fusion style mapping, clockwise time sign, alpha falloff, hover gain, swallow/eject/jet durations, and center-light color values equal to `BlackHole.metal` and the existing parameter headers.

- [ ] **Step 6: Verify render startup on Windows**

Run `npm run tauri dev -- --no-watch`, enable the black hole, and verify the renderer state becomes ready, transparent corners do not form a rectangular black window, and CPU stays responsive at 30 and 60 FPS.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/native/windows/renderer.h src-tauri/native/windows/renderer.cpp \
  src-tauri/native/windows/render_state.h src-tauri/native/windows/render_state_test.cc \
  src-tauri/native/windows/BlackHole.hlsl src-tauri/native/windows/window.cpp \
  src-tauri/native/windows/pet_bridge.cpp src-tauri/build.rs
git commit -m "feat: render the Windows black hole with Direct3D"
```

### Task 8: Add live desktop capture, self-exclusion, and recovery

**Files:**
- Create: `src-tauri/native/windows/capture.h`
- Create: `src-tauri/native/windows/capture.cpp`
- Create: `src-tauri/native/windows/capture_state.h`
- Create: `src-tauri/native/windows/capture_state_test.cc`
- Modify: `src-tauri/native/windows/renderer.cpp`
- Modify: `src-tauri/native/windows/window.cpp`
- Modify: `src-tauri/native/windows/pet_bridge.cpp`
- Modify: `src-tauri/build.rs`

**Interfaces:**
- Produces: `DesktopCapture::start(display)`, `acquire(timeout)`, `switchDisplay(display)`, `stop(deadline)`, and `CaptureDecision reduce(CaptureEvent)`.
- Emits capture Ready/Failed and preserves renderer Ready when only distortion degrades.

- [ ] **Step 1: Write failing capture-reducer tests**

```cpp
int main() {
  CaptureMachine machine;
  assert(machine.reduce(CaptureEvent::Start) == CaptureAction::CreateDuplication);
  assert(machine.reduce(CaptureEvent::FrameReady) == CaptureAction::PublishFrame);
  assert(machine.reduce(CaptureEvent::AccessLost) == CaptureAction::RecreateDuplication);
  assert(machine.reduce(CaptureEvent::Sleep) == CaptureAction::ReleaseAll);
  assert(machine.reduce(CaptureEvent::Wake) == CaptureAction::EnumerateDisplays);
}
```

Add tests for timeout-without-stale-frame, display rotation, monitor switch, device removed, shutdown deadline, and failure-to-particle-only degradation.

- [ ] **Step 2: Compile and verify RED**

Run MSVC against `capture_state_test.cc`. Expected: missing `capture_state.h`.

- [ ] **Step 3: Implement GPU-only desktop capture**

Use `IDXGIOutput1::DuplicateOutput` on the adapter owning the selected monitor. `AcquireNextFrame` returns an `ID3D11Texture2D`; crop the current black-hole rectangle into a same-device texture and expose an SRV to the renderer. Apply output rotation before sampling. Release every acquired frame before the next call.

- [ ] **Step 4: Prevent recursive capture**

Call:

```cpp
SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
```

before capture begins and verify the return value. If exclusion or duplication is unavailable, clear the desktop SRV and render procedural particles/accretion only. Never retain a desktop SRV from an older monitor, pre-sleep frame, or failed device generation.

- [ ] **Step 5: Recover from display and device events**

Recreate duplication after `DXGI_ERROR_ACCESS_LOST`, `DXGI_ERROR_DEVICE_REMOVED`, `WM_DISPLAYCHANGE`, wake, or monitor migration. The capture thread waits at most 16 ms for a frame and observes a stop event, allowing `pet_destroy` to finish within the established shutdown result codes.

- [ ] **Step 6: Verify live distortion**

On Windows, place the black hole over a scrolling browser and move it continuously across two displays. Expected: sampled content changes every frame, distortion follows without one-frame snapshots, there is no recursive copy of the black hole, and moving to a new display does not show the old display.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/native/windows/capture.h src-tauri/native/windows/capture.cpp \
  src-tauri/native/windows/capture_state.h src-tauri/native/windows/capture_state_test.cc \
  src-tauri/native/windows/renderer.cpp src-tauri/native/windows/window.cpp \
  src-tauri/native/windows/pet_bridge.cpp src-tauri/build.rs
git commit -m "feat: distort the live Windows desktop"
```

### Task 9: Lock visual and animation parity

**Files:**
- Create: `src-tauri/native/windows/animation.h`
- Create: `src-tauri/native/windows/animation_test.cc`
- Create: `docs/qa-windows-black-hole.md`
- Create: `scripts/check-mac-native-seal.mjs`
- Create: `scripts/check-mac-native-seal.test.mjs`
- Modify: `src-tauri/native/windows/BlackHole.hlsl`
- Modify: `src-tauri/native/windows/renderer.cpp`
- Modify: `package.json`

**Interfaces:**
- Produces: deterministic `AnimationUniforms ResolveAnimation(state, elapsed)` and `npm run check:mac-seal`.
- Consumes: the exact macOS reference commit `d640d92` for native-file sealing and approved macOS recordings for perceptual comparison.

- [ ] **Step 1: Write failing duration and seal tests**

```cpp
int main() {
  assert(SwallowProgress(0.74) == 1.0);
  assert(EjectProgress(0.74) == 0.0);
  assert(EjectProgress(1.36) == 1.0);
  assert(SuccessJetProgress(1.24) == 1.0);
  assert(HoverEffect(1.0).rotationRate == 2.4f);
  assert(HoverEffect(1.0).pullGain == 1.7f);
}
```

The Node seal test must fail if any path under `src-tauri/native/mac` differs from `d640d92` and pass when only `native/windows` changes.

- [ ] **Step 2: Verify RED**

Run:

```bash
npm test -- --run scripts/check-mac-native-seal.test.mjs
```

and compile `animation_test.cc` on Windows. Expected: missing implementations.

- [ ] **Step 3: Implement the exact animation constants**

Use swallow 0.74 s, eject 0.62 s, success jet 0.50 s, smoothstep easing, orbit scale exponent 1.18, hover rotation `1.0 + 1.4p`, hover pull `1.0 + 0.7p`, and unchanged visual diameter. Keep clockwise rotation and dynamic center light in every active state.

- [ ] **Step 4: Add repeatable visual QA**

Document a 3840×2160 60 FPS capture matrix for dark navy, white browser, moving checkerboard, Explorer icons, multi-monitor drag, ingest, eject, and jet. Each run records both platforms with matching size/style/FPS. Reject any Windows run with a fixed circular lens edge, black rectangular boundary, frozen sampled frame, delayed interior, free-floating artifact, counter-clockwise motion, or changed Mac output.

- [ ] **Step 5: Run parity gates**

Run:

```bash
npm run check:mac-seal
npm test -- --run
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: Mac native seal unchanged; 213 or more frontend tests and 302 or more Rust tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/native/windows/animation.h \
  src-tauri/native/windows/animation_test.cc \
  src-tauri/native/windows/BlackHole.hlsl \
  src-tauri/native/windows/renderer.cpp docs/qa-windows-black-hole.md \
  scripts/check-mac-native-seal.mjs scripts/check-mac-native-seal.test.mjs \
  package.json
git commit -m "test: lock Windows black-hole parity"
```

### Task 10: Windows CI, installer, and final acceptance

**Files:**
- Create: `.github/workflows/windows.yml`
- Create: `docs/install-windows.md`
- Create: `docs/qa-windows-release.md`
- Modify: `THIRD_PARTY_NOTICES.md`
- Modify: `src-tauri/tauri.windows.conf.json`
- Modify: `scripts/release-windows.mjs`

**Interfaces:**
- Produces: a CI-built `CYLUNE-Setup.exe` and a signed release path.
- Consumes: all previous unit, integration, seal, visual, and real-slice tests.

- [ ] **Step 1: Write failing release-policy assertions**

Extend `release-windows.test.mjs` to reject zero or multiple NSIS installers, reject symlinked inputs, never overwrite an existing release, and preserve files outside `bundle/nsis`.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
npm test -- --run scripts/release-windows.test.mjs
```

Expected: the new safety cases fail.

- [ ] **Step 3: Harden the publisher and installer configuration**

Use current-user NSIS installation, the existing `icon.ico`, Chinese/English installer languages, and the WebView2 downloaded bootstrapper. Publish to `发布-Windows/CYLUNE-Setup.exe` only after the build exits zero; refuse overwrite unless the existing output was created by the same release command in the current run.

- [ ] **Step 4: Add Windows CI**

The `windows-2022` job runs:

```yaml
- run: npm ci
- run: npm test -- --run
- run: npm run test:rust
- run: npm run check:mac-seal
- run: npm run tauri build -- --bundles nsis
```

Upload the NSIS artifact and test logs. Do not store signing credentials in the repository; read certificate and password from CI secrets only for tagged releases.

- [ ] **Step 5: Execute the real-machine release matrix**

Complete every row in `docs/qa-windows-release.md`:

- Windows 10 22H2 x64 and Windows 11 x64.
- Intel integrated GPU plus AMD or NVIDIA discrete GPU.
- 100%, 125%, 150%, 200% DPI and mixed-DPI dual displays.
- Install, update, uninstall, single instance, tray, reboot persistence.
- Sleep/wake, lock/unlock, display unplug/replug, primary-display switch.
- Single-plate and multi-plate Bambu 3MF background slicing with cancellation.
- Black-hole hover, ingest, unsupported eject, success jet, hide/show, size, FPS.

Every row records pass/fail, OS build, GPU/driver, DPI, and evidence path.

- [ ] **Step 6: Build the preview installer**

On Windows:

```powershell
npm ci
npm test -- --run
npm run test:rust
npm run release:windows
Get-FileHash .\发布-Windows\CYLUNE-Setup.exe -Algorithm SHA256
```

Expected: one installable preview package plus a recorded SHA-256 digest.

- [ ] **Step 7: Final regression and commit**

Run the complete macOS suite one final time from the sealed Mac workspace and confirm `git diff d640d92 -- src-tauri/native/mac` remains empty in the Windows branch. Commit:

```bash
git add .github/workflows/windows.yml docs/install-windows.md \
  docs/qa-windows-release.md THIRD_PARTY_NOTICES.md \
  src-tauri/tauri.windows.conf.json scripts/release-windows.mjs \
  scripts/release-windows.test.mjs
git commit -m "release: prepare CYLUNE for Windows"
```

## Execution checkpoints

1. After Tasks 0–3: Windows branch has the current app, isolated packaging, and Bambu slicing support.
2. After Tasks 4–6: a movable transparent Windows black-hole window accepts and routes files.
3. After Tasks 7–9: live background distortion and all approved animations pass visual parity.
4. After Task 10: Windows preview installer and QA evidence are ready; signing converts it to a formal release.
