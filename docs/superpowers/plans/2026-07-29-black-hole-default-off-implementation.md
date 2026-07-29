# Black Hole Default-Off Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a fresh installation start with the desktop black hole off, expose persistent on/off controls, and request macOS Screen Recording permission on the first user-initiated enable.

**Architecture:** Keep the existing Rust `PetMode::Real/Lite` persistence format to preserve compatibility, but reinterpret it as the requested master state in the UI. Derive effective visibility from `mode == Real && visible` at every native and menu boundary, while retaining Lite internally as the capture fallback. The React settings page sends atomic mode-and-visibility patches so enable, disable, hide, and restart behavior cannot drift apart.

**Tech Stack:** React 18, TypeScript, Vitest, Testing Library, Tauri 2, Rust, rusqlite, macOS ScreenCaptureKit permission bridge

## Global Constraints

- A fresh installation starts with the black hole off.
- The first user action that turns it on invokes the existing macOS Screen Recording permission request path.
- Once enabled or disabled, the requested state survives app restarts.
- Existing users with a saved `Real` mode but no `visible` key remain enabled and visible.
- The existing “隐藏黑洞 / 显示黑洞” control remains and is disabled while the master state is off.
- No user-facing locale may expose “轻量模式”, “輕量模式”, or “lightweight mode”.
- Internal `PetMode::Lite` and the compatibility renderer remain available.
- Do not change the approved black-hole shader, appearance, motion, ingestion, rejection, or jet animation.

---

## File Structure

- `src-tauri/src/pet/store.rs`: Load and persist backward-compatible default state.
- `src-tauri/src/pet/mod.rs`: Own shared enabled/effective-visibility semantics and permission-request decision.
- `src-tauri/src/pet/native.rs`: Convert requested state to safe native visibility.
- `src-tauri/src/pet/runtime.rs`: Apply settings and prevent menu actions from showing a disabled black hole.
- `src-tauri/src/tray.rs`: Keep the menu-bar hide/show item availability and label consistent.
- `src-tauri/src/lib.rs`: Seed the tray with enabled and effective-visible state.
- `src/features/settings/Pet.tsx`: Render the on/off control and atomic settings transitions.
- `src/features/settings/Pet.test.tsx`: Verify user-visible behavior and optimistic rollback.
- `src/lib/tauri.ts`: Make demo/default behavior match a fresh installation.
- `src/i18n/locales/{zh-CN,zh-TW,en}.json`: Provide new status labels and compatibility copy.

---

### Task 1: Backward-Compatible Fresh-Install Defaults

**Files:**
- Modify: `src-tauri/src/pet/store.rs`
- Test: `src-tauri/src/pet/store.rs`

**Interfaces:**
- Consumes: existing `PetStore::load(&AppDatabase) -> Result<PetSettings>`
- Produces: missing `pet_mode` loads as `Lite`; missing `pet_visible` derives from the loaded mode

- [ ] **Step 1: Write failing store tests**

Update the existing default assertion to expect `visible: false`, then add migration fixtures:

```rust
#[test]
fn existing_real_mode_without_visibility_remains_enabled_and_visible() {
    let db = AppDatabase::open_in_memory().unwrap();
    db.connection
        .execute(
            "INSERT INTO app_settings(setting_key, setting_value) VALUES ('pet_mode', 'real')",
            [],
        )
        .unwrap();

    let loaded = PetStore::load(&db).unwrap();
    assert_eq!(loaded.mode, PetMode::Real);
    assert!(loaded.visible);
}

#[test]
fn explicit_hidden_real_mode_remains_hidden() {
    let db = AppDatabase::open_in_memory().unwrap();
    db.connection
        .execute_batch(
            "INSERT INTO app_settings(setting_key, setting_value) VALUES ('pet_mode', 'real');
             INSERT INTO app_settings(setting_key, setting_value) VALUES ('pet_visible', 'false');",
        )
        .unwrap();

    let loaded = PetStore::load(&db).unwrap();
    assert_eq!(loaded.mode, PetMode::Real);
    assert!(!loaded.visible);
}
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml pet::store::tests -- --nocapture
```

Expected: `defaults_are_safe_and_valid` and the missing-visibility migration test fail because the current parser defaults `visible` to `true` independently of mode.

- [ ] **Step 3: Derive a missing visibility value from the loaded mode**

Load `mode` before constructing `PetSettings` and replace the independent parser default:

```rust
pub fn load(database: &AppDatabase) -> Result<PetSettings> {
    let mode = parse_mode(setting(database, "pet_mode")?)?;
    let settings = PetSettings {
        mode,
        visual_style: parse_visual_style(setting(database, "pet_visual_style")?)?,
        size: parse_size(setting(database, "pet_size")?)?,
        fps: parse_fps(setting(database, "pet_fps")?)?,
        visible: parse_visible(setting(database, "pet_visible")?, mode)?,
        x: parse_coordinate(setting(database, "pet_x")?)?,
        y: parse_coordinate(setting(database, "pet_y")?)?,
        display_id: parse_display_id(setting(database, "pet_display_id")?)?,
    };
    validate(&settings)?;
    Ok(settings)
}

fn parse_visible(value: Option<String>, mode: PetMode) -> Result<bool> {
    match value.as_deref() {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        None => Ok(mode == PetMode::Real),
        Some(_) => Err(AppError::InvalidPetSettings),
    }
}
```

- [ ] **Step 4: Run the focused tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml pet::store::tests -- --nocapture
```

Expected: all store tests pass.

- [ ] **Step 5: Commit the default and migration behavior**

```bash
git add src-tauri/src/pet/store.rs
git commit -m "fix: default desktop black hole to off"
```

---

### Task 2: On/Off Settings UI and Localized Copy

**Files:**
- Modify: `src/features/settings/Pet.test.tsx`
- Modify: `src/features/settings/Pet.tsx`
- Modify: `src/lib/tauri.ts`
- Modify: `src/i18n/locales/zh-CN.json`
- Modify: `src/i18n/locales/zh-TW.json`
- Modify: `src/i18n/locales/en.json`

**Interfaces:**
- Consumes: `TauriApi.setPetSettings(patch: PetSettingsPatch)`
- Produces: On sends `{ mode: "real", visible: true }`; Off sends `{ mode: "lite", visible: false }`; hide/show remains visibility-only while enabled

- [ ] **Step 1: Write failing component tests for the master state**

Change the frontend fixture to `visible: false` and add behavior assertions:

```tsx
it("starts off and disables hide or show for fresh settings", async () => {
  renderPet(petApi({ mode: "lite", visible: false }));

  expect(await screen.findByRole("button", { name: "Turn off" }))
    .toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Show black hole" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "Lightweight mode" }))
    .not.toBeInTheDocument();
});

it("turns on atomically and unlocks visibility controls", async () => {
  const apiClient = petApi({ mode: "lite", visible: false });
  renderPet(apiClient);

  fireEvent.click(await screen.findByRole("button", { name: "Turn on" }));

  await waitFor(() => {
    expect(apiClient.setPetSettings)
      .toHaveBeenLastCalledWith({ mode: "real", visible: true });
  });
  expect(screen.getByRole("button", { name: "Hide black hole" })).toBeEnabled();
});

it("turns off atomically and disables visibility controls", async () => {
  const apiClient = petApi({ mode: "real", visible: true });
  renderPet(apiClient);

  fireEvent.click(await screen.findByRole("button", { name: "Turn off" }));

  await waitFor(() => {
    expect(apiClient.setPetSettings)
      .toHaveBeenLastCalledWith({ mode: "lite", visible: false });
  });
  expect(screen.getByRole("button", { name: "Show black hole" })).toBeDisabled();
});
```

Update serialization and rollback tests to expect the same atomic patches. Update fallback-copy cases to require compatibility wording without “lightweight mode”.

- [ ] **Step 2: Run the component tests and verify RED**

Run:

```bash
npm test -- --run src/features/settings/Pet.test.tsx
```

Expected: tests fail because the current buttons are “Real distortion / Lightweight mode”, visibility starts true, and mode writes are not atomic.

- [ ] **Step 3: Implement the UI state transitions**

Change the frontend fallback to hidden and replace the two mode click handlers:

```tsx
const defaultPet: PetSettings = {
  mode: "lite",
  visual_style: "gargantua",
  size: 220,
  fps: "auto",
  visible: false,
  x: null,
  y: null,
  display_id: null,
  effective_mode: "lite",
  permission: "unavailable",
  fallback_reason: null,
};

const enabled = settings.mode === "real";

<button
  aria-pressed={enabled}
  className={enabled ? "active" : ""}
  onClick={() => save({ mode: "real", visible: true })}
>
  {copy("pet.on")}
</button>
<button
  aria-pressed={!enabled}
  className={!enabled ? "active" : ""}
  onClick={() => save({ mode: "lite", visible: false })}
>
  {copy("pet.off")}
</button>
```

Disable visibility while off without removing reset-position:

```tsx
<button
  className="secondary small"
  disabled={!enabled}
  onClick={() => save({ visible: !settings.visible })}
>
  {settings.visible ? <EyeSlash size={16} /> : <Eye size={16} />}
  {settings.visible ? copy("pet.hide") : copy("pet.show")}
</button>
```

- [ ] **Step 4: Update demo defaults and all three locales**

Use these exact selector labels:

```json
// zh-CN
"mode": "黑洞状态",
"on": "开启黑洞",
"off": "关闭黑洞"

// zh-TW
"mode": "黑洞狀態",
"on": "開啟黑洞",
"off": "關閉黑洞"

// en
"mode": "Black hole status",
"on": "Turn on",
"off": "Turn off"
```

Replace fallback messages with compatibility wording:

```json
// en examples
"captureUnavailable": "Desktop capture is unavailable; the black hole is using a compatible background",
"platformUnsupported": "Live distortion is unavailable on this platform; the black hole is using a compatible background",
"permissionNotDetermined": "Turn on the black hole to request Screen Recording access",
"permissionDenied": "Screen Recording access is off; the black hole is using a compatible background",
"captureFailed": "Desktop capture stopped unexpectedly; the black hole is using a compatible background",
"metalUnavailable": "Metal is unavailable; the black hole is using a compatible background"
```

Use these Simplified Chinese fallback strings:

```json
"captureUnavailable": "桌面捕获当前不可用，黑洞正在使用兼容背景",
"platformUnsupported": "此平台不支持实时扭曲，黑洞正在使用兼容背景",
"permissionNotDetermined": "开启黑洞以申请屏幕录制权限",
"permissionDenied": "屏幕录制权限未开启，黑洞正在使用兼容背景",
"captureFailed": "桌面捕获意外停止，黑洞正在使用兼容背景",
"metalUnavailable": "Metal 当前不可用，黑洞正在使用兼容背景"
```

Use these Traditional Chinese fallback strings:

```json
"captureUnavailable": "桌面擷取目前無法使用，黑洞正在使用相容背景",
"platformUnsupported": "此平台不支援即時扭曲，黑洞正在使用相容背景",
"permissionNotDetermined": "開啟黑洞以要求螢幕錄製權限",
"permissionDenied": "未開啟螢幕錄製權限，黑洞正在使用相容背景",
"captureFailed": "桌面擷取意外停止，黑洞正在使用相容背景",
"metalUnavailable": "Metal 目前無法使用，黑洞正在使用相容背景"
```

Remove the unused `pet.real` and `pet.lite` keys. Set the demo
`PetSettings.visible` default to `false`.

- [ ] **Step 5: Run focused frontend tests and verify GREEN**

Run:

```bash
npm test -- --run src/features/settings/Pet.test.tsx src/i18n/i18n.test.ts src/lib/tauri.test.ts
```

Expected: all selected tests pass and no rendered copy exposes lightweight mode.

- [ ] **Step 6: Commit the UI behavior**

```bash
git add src/features/settings/Pet.tsx src/features/settings/Pet.test.tsx src/lib/tauri.ts src/i18n/locales/zh-CN.json src/i18n/locales/zh-TW.json src/i18n/locales/en.json
git commit -m "feat: add persistent black hole power control"
```

---

### Task 3: Permission, Runtime, and Menu-Bar State

**Files:**
- Modify: `src-tauri/src/pet/mod.rs`
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/src/pet/runtime.rs`
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/pet/native.rs`
- Test: `src-tauri/src/pet/runtime.rs`
- Test: `src-tauri/src/pet/mod.rs`

**Interfaces:**
- Consumes: saved `PetSettings`
- Produces: `PetSettings::enabled() -> bool`, `PetSettings::effective_visibility() -> bool`, permission request only for an explicit `Real` patch, and disabled tray visibility controls while off

- [ ] **Step 1: Write failing tests for effective visibility and permission intent**

Add semantic tests:

```rust
#[test]
fn lite_requested_mode_is_never_effectively_visible() {
    let settings = PetSettings {
        mode: PetMode::Lite,
        visual_style: PetVisualStyle::Gargantua,
        size: 220,
        fps: PetFps::Auto,
        visible: true,
        x: None,
        y: None,
        display_id: None,
    };

    assert!(!settings.enabled());
    assert!(!settings.effective_visibility());
    assert_eq!(PetNativeConfig::from_settings(&settings, false).visible, 0);
}

#[test]
fn real_requested_mode_preserves_the_users_hidden_choice() {
    let settings = PetSettings {
        mode: PetMode::Real,
        visual_style: PetVisualStyle::Gargantua,
        size: 220,
        fps: PetFps::Auto,
        visible: false,
        x: None,
        y: None,
        display_id: None,
    };

    assert!(settings.enabled());
    assert!(!settings.effective_visibility());
}

#[test]
fn only_an_explicit_turn_on_patch_requests_permission() {
    assert!(should_request_permission(&PetSettingsPatch {
        mode: Some(PetMode::Real),
        visible: Some(true),
        ..Default::default()
    }));
    assert!(!should_request_permission(&PetSettingsPatch {
        mode: Some(PetMode::Lite),
        visible: Some(false),
        ..Default::default()
    }));
    assert!(!should_request_permission(&PetSettingsPatch {
        visible: Some(true),
        ..Default::default()
    }));
}
```

Add this runtime regression:

```rust
#[test]
fn disabled_mode_cannot_show_even_with_a_stale_visible_flag() {
    let mut core = RuntimeCore::for_test_with_mapped_fixture();
    core.apply_settings(PetSettings {
        mode: PetMode::Lite,
        visual_style: PetVisualStyle::Gargantua,
        size: 220,
        fps: PetFps::Auto,
        visible: true,
        x: None,
        y: None,
        display_id: None,
    });

    assert_eq!(core.last_native_config().visible, 0);
}
```

- [ ] **Step 2: Run focused Rust tests and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml pet::native::tests pet::runtime::tests pet::tests -- --nocapture
```

If Cargo accepts only one filter, run the three filters separately. Expected: compile or assertion failure because enabled/effective-visibility helpers do not yet exist and native visibility mirrors the raw flag.

- [ ] **Step 3: Implement shared state semantics**

Add to `src-tauri/src/pet/mod.rs`:

```rust
impl PetSettings {
    pub fn enabled(&self) -> bool {
        self.mode == PetMode::Real
    }

    pub fn effective_visibility(&self) -> bool {
        self.enabled() && self.visible
    }
}

fn should_request_permission(patch: &PetSettingsPatch) -> bool {
    patch.mode == Some(PetMode::Real)
}
```

Use `should_request_permission(&patch)` before passing the patch to the store. In `PetNativeConfig::from_settings`, populate `visible` from `settings.effective_visibility()`.

- [ ] **Step 4: Synchronize runtime and tray with master state**

Replace raw visibility synchronization with:

```rust
crate::tray::sync_pet_state(
    app,
    settings.enabled(),
    settings.effective_visibility(),
);
```

Make runtime `is_visible()` return effective visibility and make `set_visible()` return without persisting or showing when requested mode is `Lite`.

Change the tray setup signature to receive both values:

```diff
-pub fn setup(app: &tauri::App, initial_locale: &str, pet_visible: bool) -> tauri::Result<()> {
+pub fn setup(app: &tauri::App, initial_locale: &str, pet_enabled: bool, pet_visible: bool) -> tauri::Result<()> {
```

Add `pet_enabled: Mutex<bool>` to `NativeMenuState` and construct the visibility
item with the master state:

```rust
let visibility = MenuItemBuilder::with_id(
    "pet-visibility",
    if pet_visible { copy.hide } else { copy.show },
)
.enabled(pet_enabled)
.build(app)?;
```

Then implement:

```rust
pub fn sync_pet_state(app: &tauri::AppHandle, enabled: bool, visible: bool) {
    let state = app.state::<NativeMenuState>();
    if let Ok(mut saved) = state.pet_enabled.lock() {
        *saved = enabled;
    }
    if let Ok(mut saved) = state.pet_visible.lock() {
        *saved = visible;
    }
    let _ = state.visibility.set_enabled(enabled);
    let locale = state
        .locale
        .lock()
        .map(|locale| locale.clone())
        .unwrap_or_else(|_| "zh-CN".to_owned());
    let copy = native_copy(&locale);
    let _ = state
        .visibility
        .set_text(if visible { copy.hide } else { copy.show });
}
```

At startup, pass `initial_pet_settings.enabled()` and
`initial_pet_settings.effective_visibility()` from `src-tauri/src/lib.rs`.
The menu item and left-click handler may still call `runtime.toggle()`
because runtime now ignores toggles while disabled.

- [ ] **Step 5: Run focused Rust tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml pet::store::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml pet::native::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml pet::runtime::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml tray::tests -- --nocapture
```

Expected: all focused suites pass, including the existing fallback and native appearance tests.

- [ ] **Step 6: Commit runtime and tray consistency**

```bash
git add src-tauri/src/pet/mod.rs src-tauri/src/pet/native.rs src-tauri/src/pet/runtime.rs src-tauri/src/tray.rs src-tauri/src/lib.rs
git commit -m "fix: keep disabled black hole hidden"
```

---

### Task 4: Regression, Build, and App-Bundle Verification

**Files:**
- Verify only; modify a file only if a failing regression identifies a real defect

**Interfaces:**
- Consumes: completed Tasks 1-3
- Produces: a tested macOS application bundle with the new persistent default-off behavior

- [ ] **Step 1: Run formatting and static build checks**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
npm run build
```

Expected: both commands exit 0 with no TypeScript errors.

- [ ] **Step 2: Run complete frontend and Rust suites**

Run:

```bash
npm test -- --run
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
```

Expected: all tests pass; existing ignored hardware/file smoke tests remain ignored unless their environment variable is present.

- [ ] **Step 3: Verify no user-facing lightweight-mode copy remains**

Run:

```bash
rg -n '"(real|lite)"|轻量模式|輕量模式|lightweight mode' src/i18n src/features
```

Expected: no user-facing locale or component matches. Internal Rust/TypeScript enum names are allowed outside those directories.

- [ ] **Step 4: Build the signed local macOS bundle**

Run:

```bash
npm run tauri build
```

Expected: the command exits 0 and produces the CYLUNE `.app` bundle and installer artifacts.

- [ ] **Step 5: Perform the fresh-state and persistence smoke test**

Use a temporary database or a safely isolated app-data profile:

1. Launch with no `pet_mode` or `pet_visible` keys and verify no black hole appears.
2. Open Settings and verify “关闭黑洞” is selected and hide/show is disabled.
3. Click “开启黑洞” and verify macOS requests Screen Recording permission when undecided.
4. Fully quit and relaunch; verify “开启黑洞” remains selected.
5. Hide the black hole, relaunch, and verify it remains enabled but hidden.
6. Select “关闭黑洞”, relaunch, and verify no black hole or capture request appears.
7. Seed only `pet_mode = real`, relaunch, and verify the older enabled state remains visible.

- [ ] **Step 6: Record the verified release state**

Run:

```bash
git status --short
git log -4 --oneline
```

Expected: the worktree contains no uncommitted implementation changes, and the
three task commits are present. If verification exposes a defect, return to the
relevant task, write a failing regression test, implement the minimal fix, rerun
that task's focused suite, and commit the named test and implementation files
from that task before repeating Task 4.
