# Black Hole Desktop Pet Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有 macOS 耗材管家中加入可跨屏拖动、可接收切片文件、可显示待结算状态并能真实扭曲桌面画面的黑洞桌宠。

**Architecture:** React 设置页只管理用户可见配置，Rust `PetController` 负责持久化、导入、待结算数量和生命周期；macOS Objective-C++ 模块负责透明 `NSPanel`、系统文件拖放、ScreenCaptureKit 与 Metal。Rust 与原生层只通过 `bridge.h` 中的稳定 C ABI 通信，屏幕帧始终留在原生内存/GPU，不进入 JavaScript、SQLite 或日志。

**Tech Stack:** Tauri 2、React 18、TypeScript、Rust、SQLite、Objective-C++/AppKit、ScreenCaptureKit、Metal、Vitest、Rust tests。

## Global Constraints

- macOS 优先；Windows 第一版不实现捕获和渲染，但 Rust 控制器与原生桥接接口不得写死为 macOS 业务逻辑。
- 保留当前 macOS 10.15 最低部署版本；真实模式只在 macOS 12.3 及以上启用 ScreenCaptureKit，较旧系统自动使用轻量模式。
- 黑洞尺寸必须限制在 `120..=360` 逻辑像素。
- 帧率只允许 `auto`、`30`、`60`；自动模式空闲 30 FPS、交互 60 FPS、隐藏 0 FPS。
- 只有 `NSDraggingDestination::performDragOperation` 收到的外部文件 URL 才能发起导入；移动黑洞窗口不得查询 Finder、桌面目录、鼠标下文件或当前选区。
- 一次拖放只导入第一个受支持的普通文件；`.gcode.3mf`、含 G-code 的 `.3mf` 和现有受限 `.gcode` 继续由 `PrintService` 做最终验证。
- 黑洞导入只能创建或打开打印任务，不能修改耗材余额；只有现有 `settle_job` 可以扣料。
- 屏幕捕获必须排除本应用，关闭音频、麦克风和光标；画面不得保存、上传、写入数据库或日志。
- `pet_mode`、`pet_size`、`pet_fps`、`pet_visible`、`pet_x`、`pet_y`、`pet_display_id` 是设备设置，不得加入业务备份。
- Shader 移植时保留原 MIT 版权文本，并在 `THIRD_PARTY_NOTICES.md` 说明来源和修改。
- 用户测试文件 `/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf` 只允许通过环境变量读取；不得复制、修改、上传或提交到仓库。
- 文件名保持简短；原生文件使用 `pet.*`、`bridge.*`、`capture.*`、`render.*`、`shader.metal`。
- 每个任务遵循红灯测试、最小实现、绿灯测试、独立提交；不得把失败测试留给下一任务。

## File Map

### Rust

- `src-tauri/src/pet/mod.rs`：平台无关的设置、状态机、控制器和 Tauri 命令。
- `src-tauri/src/pet/store.rs`：只读写七个 `pet_*` 设备设置。
- `src-tauri/src/pet/input.rs`：鼠标/文件拖放事件归约与首个支持文件筛选。
- `src-tauri/src/pet/geom.rs`：多显示器选择、坐标转换、安全位置和捕获矩形。
- `src-tauri/src/pet/native.rs`：macOS C ABI 包装；非 macOS 使用无捕获空实现。
- `src-tauri/src/pet/runtime.rs`：原生回调队列、导入、主窗口导航、待结算同步和生命周期。
- `src-tauri/src/tray.rs`：菜单栏左键召回/隐藏，右键打开、重置、隐藏和退出。
- `src-tauri/src/lib.rs`：注册单实例、`PetController`、命令与应用事件。
- `src-tauri/build.rs`：仅在 macOS 编译 Objective-C++，弱链接 ScreenCaptureKit，链接图形框架。

### macOS native

- `src-tauri/native/mac/bridge.h`：唯一的 C ABI 契约。
- `src-tauri/native/mac/pet.mm`：透明 `NSPanel`、点击/移动、文件拖放和屏幕事件。
- `src-tauri/native/mac/capture.mm`：权限、显示器枚举、区域捕获和休眠恢复。
- `src-tauri/native/mac/render.mm`：Metal 设备、纹理、帧率和渲染降级。
- `src-tauri/native/mac/shader.metal`：轻量与真实黑洞 Shader。

### Frontend and product

- `src/lib/tauri.ts`：`PetSettings`、`PetStatus` 与命令客户端。
- `src/features/settings/Pet.tsx`：黑洞类型、尺寸、帧率、显示/隐藏、重置与权限状态。
- `src/features/settings/Pet.test.tsx`：设置交互和无效值测试。
- `src/features/settings/Settings.tsx`：挂载桌面黑洞设置组。
- `src/i18n/locales/{zh-CN,zh-TW,en}.json`：三套完整文案。
- `src/main.tsx`、`src/features/tray/*`、`src/styles.css`：删除被原生桌宠替代的矩形菜单栏页面与样式。
- `src-tauri/tauri.conf.json`：删除 `menubar` WebView，保留主窗口并声明 macOS 权限说明。
- `THIRD_PARTY_NOTICES.md`：Shader 来源、MIT 文本和移植说明。
- `docs/install-mac.md`：屏幕录制权限、单实例、升级与轻量回退说明。
- `docs/qa-black-hole.md`：人工验收矩阵。

---

### Task 1: Pet settings, validation, and device-only persistence

**Files:**
- Create: `src-tauri/src/pet/mod.rs`
- Create: `src-tauri/src/pet/store.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/backup.rs`

**Interfaces:**
- Produces: `PetMode::{Real, Lite}`, `PetFps::{Auto, Fps30, Fps60}`, `CapturePermission`, `PetSettings`, `PetStatus`, `PetView`, `PetSettingsPatch`.
- Produces: `PetStore::load(&AppDatabase) -> Result<PetSettings>` and `PetStore::apply(&AppDatabase, PetSettingsPatch) -> Result<PetSettings>`.
- Produces Tauri commands: `get_pet_settings -> PetView` and `set_pet_settings -> PetView`; `PetView` is the stable frontend response used by later native tasks.

- [ ] **Step 1: Write failing unit tests for defaults, validation, persistence, and backup exclusion**

Add tests beside `pet/store.rs` and extend the existing backup test:

```rust
#[test]
fn defaults_are_safe_and_valid() {
    let db = AppDatabase::open_in_memory().unwrap();
    assert_eq!(PetStore::load(&db).unwrap(), PetSettings {
        mode: PetMode::Lite,
        size: 220,
        fps: PetFps::Auto,
        visible: true,
        x: None,
        y: None,
        display_id: None,
    });
}

#[test]
fn rejects_size_and_unknown_enum_values_without_partial_write() {
    let db = AppDatabase::open_in_memory().unwrap();
    assert_eq!(
        PetStore::apply(&db, PetSettingsPatch { size: Some(119), ..Default::default() })
            .unwrap_err().code(),
        "invalid_pet_settings"
    );
    assert_eq!(PetStore::load(&db).unwrap().size, 220);
}

#[test]
fn pet_coordinates_are_not_exported() {
    let mut db = AppDatabase::open_in_memory().unwrap();
    db.connection.execute(
        "INSERT INTO app_settings(setting_key,setting_value) VALUES
         ('pet_x','400'),('pet_y','220'),('pet_display_id','9')",
        [],
    ).unwrap();
    let json = export_json_for_test(&mut db).unwrap();
    assert!(!json.contains("pet_x"));
    assert!(!json.contains("pet_display_id"));
}
```

- [ ] **Step 2: Run the focused tests and verify the red state**

Run:

```bash
cd src-tauri
cargo test pet::store::tests -- --nocapture
cargo test backup::tests::pet_coordinates_are_not_exported -- --nocapture
```

Expected: compilation fails because `pet`, `PetStore` and `invalid_pet_settings` do not exist.

- [ ] **Step 3: Implement the typed settings and one-transaction patch**

Use these exact public shapes:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetMode { Real, Lite }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetFps { Auto, Fps30, Fps60 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetSettings {
    pub mode: PetMode,
    pub size: u16,
    pub fps: PetFps,
    pub visible: bool,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub display_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePermission { Unavailable, NotDetermined, Denied, RestartRequired, Granted }

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PetStatus {
    pub effective_mode: PetMode,
    pub permission: CapturePermission,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PetView {
    #[serde(flatten)]
    pub settings: PetSettings,
    #[serde(flatten)]
    pub status: PetStatus,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PetSettingsPatch {
    pub mode: Option<PetMode>,
    pub size: Option<u16>,
    pub fps: Option<PetFps>,
    pub visible: Option<bool>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub display_id: Option<u64>,
    pub reset_position: Option<bool>,
}
```

`PetStore::apply` must first merge and validate the complete result, then write all changed keys in one SQLite transaction. `reset_position == Some(true)` deletes `pet_x`, `pet_y`, and `pet_display_id`. Before the native runtime exists, commands return `PetStatus { effective_mode: Lite, permission: Unavailable, fallback_reason: Some("native_not_started") }`. Add `AppError::InvalidPetSettings` with stable code `invalid_pet_settings`; do not add any `pet_*` key to `SAFE_SETTINGS`.

- [ ] **Step 4: Register commands and run all Rust tests**

Register:

```rust
pet::get_pet_settings,
pet::set_pet_settings,
```

Run:

```bash
cd src-tauri
cargo test
```

Expected: all non-ignored Rust tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/pet src-tauri/src/lib.rs src-tauri/src/error.rs src-tauri/src/backup.rs
git commit -m "feat: persist black hole settings"
```

### Task 2: Settings UI in three languages

**Files:**
- Create: `src/features/settings/Pet.tsx`
- Create: `src/features/settings/Pet.test.tsx`
- Modify: `src/features/settings/Settings.tsx`
- Modify: `src/features/settings/Settings.test.tsx`
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/tauri.test.ts`
- Modify: `src/i18n/locales/zh-CN.json`
- Modify: `src/i18n/locales/zh-TW.json`
- Modify: `src/i18n/locales/en.json`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: `get_pet_settings` and `set_pet_settings`.
- Produces: TypeScript `PetSettings`, `PetSettingsPatch`, and reusable `<Pet apiClient={...} />`.

- [ ] **Step 1: Write failing API and component tests**

Use the exact frontend types:

```ts
export type PetMode = "real" | "lite";
export type PetFps = "auto" | "fps30" | "fps60";
export interface PetSettings {
  mode: PetMode;
  size: number;
  fps: PetFps;
  visible: boolean;
  x: number | null;
  y: number | null;
  display_id: number | null;
  effective_mode: PetMode;
  permission: "unavailable" | "not_determined" | "denied" | "restart_required" | "granted";
  fallback_reason: string | null;
}
export type PetSettingsPatch = Partial<PetSettings> & { reset_position?: boolean };
```

Core component tests:

```tsx
const defaultPet: PetSettings = {
  mode: "lite",
  size: 220,
  fps: "auto",
  visible: true,
  x: null,
  y: null,
  display_id: null,
  effective_mode: "lite",
  permission: "unavailable",
  fallback_reason: "native_not_started",
};

function petApi(overrides: Partial<PetSettings>): TauriApi {
  let current = { ...defaultPet, ...overrides };
  return {
    ...api,
    mode: "tauri",
    getPetSettings: vi.fn(async () => current),
    setPetSettings: vi.fn(async (patch) => (current = { ...current, ...patch })),
  } as TauriApi;
}

function renderPet(apiClient: TauriApi) {
  return render(<Theme><Pet apiClient={apiClient} /></Theme>);
}

it("saves mode size fps and visibility immediately", async () => {
  const api = petApi({ mode: "lite", size: 220, fps: "auto", visible: true });
  renderPet(api);
  await screen.findByRole("heading", { name: "Desktop black hole" });
  fireEvent.click(screen.getByRole("button", { name: "Real distortion" }));
  fireEvent.change(screen.getByLabelText("Black hole size"), { target: { value: "280" } });
  fireEvent.click(screen.getByRole("button", { name: "60 FPS" }));
  await waitFor(() => expect(api.setPetSettings).toHaveBeenLastCalledWith(
    expect.objectContaining({ fps: "fps60" })
  ));
});

it("uses the exact 120 to 360 size range", async () => {
  const api = petApi({ mode: "lite", size: 220, fps: "auto", visible: true });
  renderPet(api);
  const slider = await screen.findByLabelText("Black hole size");
  expect(slider).toHaveAttribute("min", "120");
  expect(slider).toHaveAttribute("max", "360");
});
```

- [ ] **Step 2: Run focused frontend tests and verify failure**

Run:

```bash
npm test -- src/lib/tauri.test.ts src/features/settings/Pet.test.tsx
```

Expected: failures for missing types, commands and component.

- [ ] **Step 3: Add command clients, optimistic UI with rollback, and accessible controls**

Extend `TauriApi`:

```ts
getPetSettings?(): Promise<PetSettings>;
setPetSettings?(patch: PetSettingsPatch): Promise<PetSettings>;
```

Map commands:

```ts
getPetSettings: () => call<PetSettings>("get_pet_settings"),
setPetSettings: (patch) => call<PetSettings>("set_pet_settings", { patch }),
```

`Pet.tsx` must serialize writes through one promise chain, keep the last server-confirmed value, and roll back with a localized error if a write fails. Render:

- mode segmented control: real/light;
- presets 160/220/300 plus range input `120..360 step=4`;
- frame segmented control: auto/30/60;
- show/hide and reset buttons;
- permission/fallback text returned by Task 6 without opening a rectangular overlay.

- [ ] **Step 4: Add complete zh-CN, zh-TW, and en keys and visual styling**

Add the same key tree to all three locale files:

```json
"pet": {
  "title": "桌面黑洞",
  "mode": "黑洞类型",
  "real": "真实扭曲",
  "lite": "轻量模式",
  "size": "黑洞尺寸",
  "small": "小",
  "medium": "中",
  "large": "大",
  "fps": "帧率",
  "auto": "自动",
  "fps30": "30 FPS",
  "fps60": "60 FPS",
  "show": "显示黑洞",
  "hide": "隐藏黑洞",
  "reset": "重置到主显示器",
  "powerHint": "60 FPS 会增加 GPU 与电量消耗",
  "permissionDenied": "屏幕录制权限未开启，当前使用轻量模式",
  "restartRequired": "权限已更改，请重新启动应用"
}
```

Translate values naturally for zh-TW and en. Use the existing rounded, dopamine-colored design language; no fixed black rectangle is added to the desktop.

- [ ] **Step 5: Run frontend tests and build**

Run:

```bash
npm test
npm run build
```

Expected: all Vitest tests pass and TypeScript/Vite build succeeds.

- [ ] **Step 6: Commit**

```bash
git add src/features/settings src/lib/tauri.ts src/lib/tauri.test.ts src/i18n src/styles.css
git commit -m "feat: add desktop black hole settings"
```

### Task 3: Native bridge and lightweight transparent pet proof

**Files:**
- Create: `src-tauri/native/mac/bridge.h`
- Create: `src-tauri/native/mac/pet.mm`
- Create: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/build.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/pet/mod.rs`

**Interfaces:**
- Consumes: `PetSettings`.
- Produces C ABI: `pet_create`, `pet_destroy`, `pet_apply`, `pet_show`, `pet_hide`, `pet_reset`, `pet_signal`.
- Produces Rust `NativePet` with RAII cleanup and `Send` command dispatch to the main thread.

- [ ] **Step 1: Write a failing bridge layout and lifecycle test**

Define a versioned ABI:

```c
typedef void (*PetCallback)(uint32_t kind, const char *payload,
                            double x, double y, uint64_t display_id);

typedef struct {
  uint32_t abi_version;
  uint32_t mode;
  double size;
  uint32_t fps;
  uint8_t visible;
  uint32_t pending_count;
  uint8_t reduce_motion;
} PetConfig;

void *pet_create(PetCallback callback, const char *metal_source);
void pet_destroy(void *handle);
bool pet_apply(void *handle, PetConfig config);
void pet_show(void *handle);
void pet_hide(void *handle);
void pet_reset(void *handle);
void pet_signal(void *handle, uint32_t signal);
uint32_t pet_abi_version(void);
```

Rust test:

```rust
#[test]
fn native_abi_and_raii_handle_are_stable() {
    assert_eq!(native::abi_version(), 1);
    let pet = NativePet::new(test_callback).unwrap();
    assert!(pet.apply(PetNativeConfig::lite(220.0, PetFps::Auto, true)));
    drop(pet);
}
```

- [ ] **Step 2: Run the macOS native test and verify linkage fails**

Run:

```bash
cd src-tauri
cargo test pet::native::tests::native_abi_and_raii_handle_are_stable -- --nocapture
```

Expected: unresolved module or C symbols.

- [ ] **Step 3: Compile Objective-C++ and link only system frameworks**

Add `cc = "1"` under build dependencies. In `build.rs`, compile `native/mac/pet.mm` with ARC and C++17, then link:

```rust
if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
    cc::Build::new()
        .cpp(true)
        .file("native/mac/pet.mm")
        .flag("-std=c++17")
        .flag("-fobjc-arc")
        .compile("pet_native");
    for framework in ["AppKit", "Metal", "MetalKit", "QuartzCore", "CoreMedia", "CoreVideo"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
}
```

Retain `tauri_build::build()`. Add `cargo:rerun-if-changed` for every native source.

- [ ] **Step 4: Implement a transparent circular `NSPanel` and a non-macOS no-op adapter**

The first native proof must:

- create the panel on the AppKit main thread;
- set `opaque = NO`, clear background, no titlebar, no shadow rectangle;
- set floating level, join all Spaces, and keep only its circular hit region interactive;
- draw a lightweight animated black disk and colored ring using `CAMetalLayer` if available, otherwise Core Animation;
- resize live from 120–360 px;
- destroy observers, display link and panel in `pet_destroy`.

The Rust non-macOS implementation must compile and report `fallback_reason = "platform_unsupported"` without inventing a window.

- [ ] **Step 5: Run native test, Rust suite, and launch a lightweight proof**

Run:

```bash
cd src-tauri
cargo test pet::native::tests::native_abi_and_raii_handle_are_stable -- --nocapture
cargo test
cd ..
npm run tauri dev
```

Expected: tests pass; one borderless circular pet appears and can be hidden without terminating the main app.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/native/mac src-tauri/src/pet/native.rs src-tauri/src/pet/mod.rs src-tauri/build.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: add native macOS pet bridge"
```

### Task 4: Safe movement, click, external file drop, and multi-display geometry

**Files:**
- Create: `src-tauri/src/pet/input.rs`
- Create: `src-tauri/src/pet/geom.rs`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/src/tray.rs`

**Interfaces:**
- Produces: `PetInput`, `PetAction`, `DisplayRect`, `PetPoint`, `safe_origin`, `display_for_pet`, `capture_rect`.
- Native callback kinds: `1=clicked`, `2=moved`, `3=drop_entered`, `4=drop_exited`, `5=file_dropped`, `6=display_changed`.

- [ ] **Step 1: Write failing pure tests for the safety boundary and screen calculations**

```rust
#[test]
fn moving_the_pet_never_emits_an_import() {
    let mut state = InputState::default();
    assert_eq!(state.reduce(PetInput::PointerDown { x: 20.0, y: 20.0 }), vec![]);
    assert_eq!(
        state.reduce(PetInput::PointerMove { x: 240.0, y: 80.0 }),
        vec![PetAction::MoveWindow { dx: 220.0, dy: 60.0 }]
    );
    assert!(state.reduce(PetInput::PointerUp).iter()
        .all(|action| !matches!(action, PetAction::Import(_))));
}

#[test]
fn drop_uses_only_the_first_supported_regular_file() {
    let files = vec![
        DropFile::new("/tmp/readme.txt", true),
        DropFile::new("/tmp/plate.gcode.3mf", true),
        DropFile::new("/tmp/second.3mf", true),
    ];
    assert_eq!(
        InputState::default().reduce(PetInput::FilesDropped(files)),
        vec![PetAction::Import(PathBuf::from("/tmp/plate.gcode.3mf"))]
    );
}

#[test]
fn disconnected_display_clamps_pet_into_primary_safe_area() {
    let primary = DisplayRect::new(7, 0.0, 0.0, 1512.0, 982.0, 2.0);
    assert_eq!(safe_origin(PetPoint::new(5000.0, -80.0), 220.0, &[primary]), PetPoint::new(1276.0, 16.0));
}
```

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```bash
cd src-tauri
cargo test pet:: -- --nocapture
```

Expected: missing input and geometry modules.

- [ ] **Step 3: Implement the pure reducers and geometry**

Movement becomes a drag only after 4 logical pixels. A shorter press emits `PetAction::Click`; pointer movement/up never emits `Import`. `FilesDropped` canonicalizes only the chosen path after checking it is absolute, a regular file, and has a supported suffix.

`display_for_pet` chooses the screen with greatest panel intersection area. `safe_origin` leaves a 16 px visible inset. `capture_rect` expands the panel by a 24% lens margin and clamps to the selected display.

- [ ] **Step 4: Implement AppKit movement and `NSDraggingDestination`**

In `pet.mm`:

```objc
[self registerForDraggedTypes:@[NSPasteboardTypeFileURL]];
```

Only `performDragOperation:` may emit callback kind `5`. `draggingEntered:` and `draggingExited:` emit visual-state callbacks but no file path. Mouse dragging calls `setFrameOrigin:` and emits kind `2` only after mouse-up. Do not call `NSWorkspace`, Finder AppleScript, directory enumeration, Accessibility APIs, or pasteboard reads outside `NSDraggingDestination`.

- [ ] **Step 5: Run unit tests and manual safety proof**

Run:

```bash
cd src-tauri
cargo test pet:: -- --nocapture
cd ..
npm run tauri dev
```

Manually move the pet over a Finder icon: no selection, file read, import event, task, or notification may occur. Then drag two supported files together into the pet: only the first is submitted.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/pet/input.rs src-tauri/src/pet/geom.rs src-tauri/src/pet/native.rs src-tauri/native/mac/pet.mm src-tauri/src/tray.rs
git commit -m "feat: add safe black hole interactions"
```

### Task 5: Runtime integration, pending jobs, tray behavior, and single instance

**Files:**
- Create: `src-tauri/src/pet/runtime.rs`
- Modify: `src-tauri/src/pet/mod.rs`
- Modify: `src-tauri/src/imports.rs`
- Modify: `src-tauri/src/settlement.rs`
- Modify: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src/main.tsx`
- Delete: `src/features/tray/TrayDrop.tsx`
- Delete: `src/features/tray/TrayDrop.test.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: native callback events and existing `PrintService`.
- Produces: `PetRuntime::start`, `PetRuntime::refresh_pending`, `PetRuntime::show`, `hide`, `reset`.
- Produces: query `PrintService::pending_summary() -> Result<PendingSummary>`.
- Produces testable pure entry points `handle_file_drop`, `pending_transition`, and `second_launch_actions`; Tauri callbacks are thin adapters over them.

- [ ] **Step 1: Write failing runtime and single-instance behavior tests**

```rust
#[test]
fn dropped_file_creates_pending_job_without_changing_balances() {
    let db = AppDatabase::open_in_memory().unwrap();
    let mut service = PrintService::with_stability_delay(db, Duration::ZERO);
    let before = balance_rows(&service.database);
    let signal = handle_file_drop(&mut service, &fixture("bambu_multicolor.3mf")).unwrap();
    assert!(matches!(signal, PetSignal::ImportSucceeded { pending_count: 1, .. }));
    assert_eq!(balance_rows(&service.database), before);
}

#[test]
fn settlement_reduces_pending_count_and_flashes_green() {
    assert_eq!(
        pending_transition(1, 0, true),
        Some(PetSignal::SettlementCompleted { pending_count: 0 })
    );
    assert_eq!(pending_transition(0, 0, false), None);
}

#[test]
fn second_instance_only_recalls_existing_windows() {
    assert_eq!(
        second_launch_actions(),
        [InstanceAction::ShowMain, InstanceAction::ShowPet]
    );
}
```

The test module defines:

```rust
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn balance_rows(db: &AppDatabase) -> Vec<(String, f64)> {
    let mut statement = db.connection
        .prepare("SELECT spool_id, remaining_grams FROM spools ORDER BY spool_id").unwrap();
    statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap()
        .collect::<rusqlite::Result<Vec<_>>>().unwrap()
}
```

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```bash
cd src-tauri
cargo test pet::runtime::tests -- --nocapture
```

Expected: missing runtime, signals and pending query.

- [ ] **Step 3: Implement the callback worker and pending summary**

`PetRuntime` must copy callback strings immediately, then send owned `NativeEvent` values through a channel. On `FileDropped`, lock `PrintState`, call `import_print_file`, persist `pending_job_id`, emit `open-job`, and signal either:

```rust
pub enum PetSignal {
    ImportSucceeded { job_id: Uuid, pending_count: u32 },
    ImportFailed { code: String },
    SettlementCompleted { pending_count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceAction { ShowMain, ShowPet }

pub fn handle_file_drop(service: &mut PrintService, path: &Path) -> Result<PetSignal>;
pub fn pending_transition(before: u32, after: u32, settlement_changed: bool) -> Option<PetSignal>;
pub fn second_launch_actions() -> [InstanceAction; 2];
```

Add:

```rust
pub struct PendingSummary {
    pub count: u32,
    pub newest_job_id: Option<Uuid>,
}
```

using `WHERE outcome IS NULL`. A pet click opens `newest_job_id`; if none exists it opens the overview. Import failure logs only the stable error code, never the full path.

- [ ] **Step 4: Publish pending changes only after successful database transactions**

After `settle_job` returns successfully, call `PetRuntime::refresh_pending(SettlementCompleted)`. Repeated idempotent settlement must not double-flash; compare pending count and settlement version before signaling. Reversal does not recreate an unsettled job and therefore does not add an amber point. Import failure emits `pet-import-error` to the main window and a localized system notification containing only the stable error message, never the source path.

- [ ] **Step 5: Replace the old rectangular popover and add single-instance protection**

Add `tauri-plugin-single-instance = "2"` and initialize it before `.setup`:

```rust
.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
    tray::show_main(app);
    app.state::<PetController>().show();
}))
```

Remove the `menubar` window from `tauri.conf.json`, remove the `isMenubar` React branch and delete `TrayDrop`. Tray left-click toggles the pet. Right-click menu contains open, reset, hide/show, quit. Existing locale updates must rename all native menu items.

- [ ] **Step 6: Run all tests and a duplicate-launch smoke**

Run:

```bash
npm test
npm run build
cd src-tauri
cargo test
cargo build
```

Launch the same built binary twice and verify Activity Monitor shows one app process, one tray icon and one pet.

- [ ] **Step 7: Commit**

```bash
git add -A src src-tauri
git commit -m "feat: connect the pet to print jobs"
```

### Task 6: ScreenCaptureKit permission, capture, fallback, and lifecycle

**Files:**
- Create: `src-tauri/native/mac/capture.mm`
- Modify: `src-tauri/native/mac/bridge.h`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/src/pet/runtime.rs`
- Modify: `src-tauri/src/pet/mod.rs`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/build.rs`

**Interfaces:**
- Produces native capture states: `unavailable`, `not_determined`, `denied`, `restart_required`, `ready`, `failed`.
- Produces callbacks: `7=permission_changed`, `8=capture_failed`, `9=sleep`, `10=wake`.
- Populates the stable Rust `PetStatus { effective_mode, permission, fallback_reason }` created in Task 1.

- [ ] **Step 1: Write failing state-machine tests**

```rust
#[test]
fn denied_permission_keeps_the_lite_pet_running() {
    let status = CaptureState::Requested.reduce(CaptureEvent::Denied);
    assert_eq!(status.effective_mode, PetMode::Lite);
    assert_eq!(status.fallback_reason.as_deref(), Some("permission_denied"));
    assert!(status.pet_visible);
}

#[test]
fn hiding_and_sleeping_stop_capture_and_rendering() {
    let mut life = LifeState::active_real();
    assert_eq!(life.reduce(LifeEvent::Hidden), vec![LifeAction::StopCapture, LifeAction::PauseRender]);
    life = LifeState::active_real();
    assert_eq!(life.reduce(LifeEvent::Sleep), vec![LifeAction::StopCapture, LifeAction::PauseRender]);
}

#[test]
fn wake_reenumerates_displays_before_restarting_capture() {
    let mut life = LifeState::sleeping_real();
    assert_eq!(life.reduce(LifeEvent::Wake), vec![LifeAction::EnumerateDisplays, LifeAction::CheckPermission]);
}
```

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```bash
cd src-tauri
cargo test pet::runtime::tests -- --nocapture
```

Expected: missing capture and lifecycle states.

- [ ] **Step 3: Implement permission and availability handling**

Use `@available(macOS 12.3, *)` around every ScreenCaptureKit symbol. Call `CGPreflightScreenCaptureAccess()` before real mode and `CGRequestScreenCaptureAccess()` only after the user explicitly selects real mode. Denial changes only `effective_mode`; it does not overwrite the user’s stored preference or loop the system prompt.

Do not add camera or microphone permission keys: the feature uses neither. macOS Screen Recording authorization is requested through `CGRequestScreenCaptureAccess()` and explained in the app’s localized Settings UI; the current SDK exposes no `NSScreenCaptureUsageDescription` key to merge into `Info.plist`.

- [ ] **Step 4: Implement bounded display capture**

`capture.mm` must:

- enumerate `SCShareableContent`;
- match the current `CGDirectDisplayID`;
- build `SCContentFilter` for that display while excluding this app’s `SCRunningApplication`;
- set `capturesAudio = NO` and `showsCursor = NO`;
- capture only `capture_rect` around the pet at Retina pixel size;
- keep at most the newest `IOSurface` frame;
- never convert a frame to PNG/JPEG or write it to disk.

- [ ] **Step 5: Add sleep, wake, hide, and display-change observers**

Observe workspace sleep/wake and `NSApplicationDidChangeScreenParametersNotification`. Hidden or sleeping state calls `stopCaptureWithCompletionHandler`, pauses rendering, and releases the retained frame. Wake first re-enumerates screens, clamps the panel, rechecks permission, then restarts if visible and real mode remains available.

- [ ] **Step 6: Expose status to Settings and run tests**

Populate the existing `get_pet_settings` response with live native values:

```rust
pub struct PetStatus {
    pub effective_mode: PetMode,
    pub permission: CapturePermission,
    pub fallback_reason: Option<String>,
}
```

Run:

```bash
cd src-tauri
cargo test
cd ..
npm test
npm run build
```

Expected: all tests pass; denying permission leaves a functioning lightweight pet and a localized Settings explanation.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/native/mac src-tauri/src/pet src-tauri/tauri.conf.json src-tauri/build.rs src/features/settings src/lib/tauri.ts
git commit -m "feat: capture the desktop safely"
```

### Task 7: Metal rendering, real distortion, animations, and licensing

**Files:**
- Create: `src-tauri/native/mac/render.mm`
- Create: `src-tauri/native/mac/shader.metal`
- Create: `THIRD_PARTY_NOTICES.md`
- Modify: `src-tauri/native/mac/bridge.h`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/capture.mm`
- Modify: `src-tauri/src/pet/native.rs`
- Modify: `src-tauri/build.rs`

**Interfaces:**
- Consumes: newest ScreenCaptureKit `IOSurface`, `PetConfig`, `PetSignal`.
- Produces: transparent BGRA frame with idle, hover, swallow, success and error states.
- Produces debug C ABI: `pet_test_render_rgba(...) -> uint64_t` checksum for deterministic synthetic-frame tests.

- [ ] **Step 1: Write failing synthetic render tests**

```rust
#[cfg(target_os = "macos")]
#[test]
fn synthetic_checkerboard_is_distorted_only_inside_lens_radius() {
    let input = checkerboard_rgba(128, 128);
    let output = native::test_render_rgba(&input, 128, 128, TestRenderMode::Real).unwrap();
    assert_ne!(checksum_region(&output, 128, 48, 48, 32, 32),
               checksum_region(&input, 128, 48, 48, 32, 32));
    assert_eq!(output[3], 0);
}

#[cfg(target_os = "macos")]
#[test]
fn lite_mode_never_requires_a_capture_texture() {
    let output = native::test_render_rgba(&[], 128, 128, TestRenderMode::Lite).unwrap();
    assert!(output.chunks_exact(4).any(|pixel| pixel[3] > 0));
}

#[test]
fn automatic_fps_tracks_interaction_and_visibility() {
    assert_eq!(target_fps(PetFps::Auto, PetActivity::Idle), 30);
    assert_eq!(target_fps(PetFps::Auto, PetActivity::DropHover), 60);
    assert_eq!(target_fps(PetFps::Auto, PetActivity::Hidden), 0);
}
```

The test module defines deterministic helpers:

```rust
fn checkerboard_rgba(width: usize, height: usize) -> Vec<u8> {
    (0..height).flat_map(|y| (0..width).flat_map(move |x| {
        let value = if ((x / 8) + (y / 8)) % 2 == 0 { 32 } else { 224 };
        [value, value, value, 255]
    })).collect()
}

fn checksum_region(
    pixels: &[u8], width: usize, x: usize, y: usize, w: usize, h: usize
) -> u64 {
    (y..y + h).flat_map(|row| (x..x + w).flat_map(move |column| {
        let offset = (row * width + column) * 4;
        pixels[offset..offset + 4].iter().copied()
    })).fold(0_u64, |sum, byte| sum.wrapping_mul(16777619) ^ u64::from(byte))
}
```

- [ ] **Step 2: Run render tests and verify failure**

Run:

```bash
cd src-tauri
cargo test pet:: -- --nocapture
```

Expected: missing renderer test symbols and FPS reducer.

- [ ] **Step 3: Implement the Metal renderer and runtime Shader compilation**

Load `shader.metal` with Rust `include_str!`, pass the source once to `pet_create`, and compile through `newLibraryWithSource:error:`. Do not copy high-frequency frames through the C ABI.

The Shader uniforms are:

```metal
struct PetUniforms {
  float2 viewport_px;
  float time_seconds;
  float lens_strength;
  float hover_progress;
  float swallow_progress;
  float success_progress;
  float error_progress;
  uint pending_count;
  uint mode;
  uint reduce_motion;
};
```

Real mode samples the captured texture through a bounded lens transform and adds shadow, photon ring and accretion disk. Lite mode draws the same black hole and animations over transparency without sampling the desktop texture.

- [ ] **Step 4: Implement frame pacing and animation signals**

Use `CVDisplayLink` or `CADisplayLink` where available. Fixed 30 skips alternate display frames; fixed 60 renders every available frame; auto follows `target_fps`. Hidden state stops the link.

Signals:

- drop enter: scale to 1.12 and brighten ring;
- import success: 260 ms contraction, flash, spring return;
- pending: every pending task gets one amber orbit point; distribute high counts over concentric rings without dropping or merging tasks;
- settlement: one green ring pulse;
- import failure: one red outward ripple;
- reduced motion: no spring/flash, 150 ms opacity and color transitions.

- [ ] **Step 5: Add license notices before copying Shader logic**

`THIRD_PARTY_NOTICES.md` must include:

- `cabbagehao/blackhole-timer`;
- `s0xDk/ghostty-blackhole`;
- original copyright names and full MIT license text;
- statement that GLSL geodesic/lensing logic was modified and ported to Metal;
- statement that unrelated timer/product code was not copied.

Add the matching notice header to `shader.metal`.

- [ ] **Step 6: Run native tests and inspect render snapshots**

Run:

```bash
cd src-tauri
cargo test pet:: -- --nocapture
cargo test
cd ..
npm run tauri dev
```

Expected: synthetic tests pass; captured checkerboard bends only in the circular lens region; panel corners remain fully transparent.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/native/mac src-tauri/src/pet src-tauri/build.rs THIRD_PARTY_NOTICES.md
git commit -m "feat: render the real black hole"
```

### Task 8: Multi-monitor correctness, live settings, and failure isolation

**Files:**
- Modify: `src-tauri/src/pet/geom.rs`
- Modify: `src-tauri/src/pet/runtime.rs`
- Modify: `src-tauri/src/pet/mod.rs`
- Modify: `src-tauri/native/mac/pet.mm`
- Modify: `src-tauri/native/mac/capture.mm`
- Modify: `src-tauri/native/mac/render.mm`
- Modify: `src/features/settings/Pet.tsx`
- Modify: `src/features/settings/Pet.test.tsx`

**Interfaces:**
- Consumes: settings changes, screen snapshots, native failure callbacks.
- Produces: atomic live reconfiguration and renderer/capture fault containment.

- [ ] **Step 1: Add failing tests for Retina, cross-screen, disconnect, and renderer failure**

```rust
#[test]
fn retina_capture_uses_backing_pixels_without_changing_logical_pet_size() {
    let screen = DisplayRect::new(3, 0.0, 0.0, 1512.0, 982.0, 2.0);
    let rect = capture_rect(PetPoint::new(200.0, 100.0), 220.0, screen);
    assert_eq!(rect.pixel_width, 544);
    assert_eq!(rect.logical_pet_size, 220.0);
}

#[test]
fn greatest_intersection_selects_the_new_display() {
    let screens = [
        DisplayRect::new(1, 0.0, 0.0, 1512.0, 982.0, 2.0),
        DisplayRect::new(2, 1512.0, 0.0, 1920.0, 1080.0, 1.0),
    ];
    assert_eq!(display_for_pet(PetPoint::new(1450.0, 100.0), 220.0, &screens).id, 2);
}

#[test]
fn render_failure_does_not_block_import_or_settlement() {
    let mut core = RuntimeCore::for_test_with_mapped_fixture();
    core.reduce(NativeEvent::RenderFailed("metal_init".into()));
    let imported = core.import_fixture().unwrap();
    core.settle_success(imported.job_id).unwrap();
    assert_eq!(core.pending_summary().unwrap().count, 0);
    assert_eq!(core.status().effective_mode, PetMode::Lite);
}
```

- [ ] **Step 2: Run focused tests and verify failure**

Run:

```bash
cd src-tauri
cargo test pet:: -- --nocapture
```

Expected: incorrect pixel region or missing failure isolation.

- [ ] **Step 3: Make screen switching and persistence atomic**

On mouse-up:

1. enumerate screens;
2. select greatest intersection;
3. clamp logical origin;
4. apply backing scale to capture only;
5. update native capture display/region;
6. persist `pet_x`, `pet_y`, `pet_display_id` together.

On display removal, repeat with the primary screen and preserve a 16 px safe inset. Do not persist intermediate drag positions.

- [ ] **Step 4: Apply settings live without recreating business services**

Mode, size, FPS and visibility changes call `pet_apply` on the main thread. Only mode/display/capture-region changes reconfigure ScreenCaptureKit. Size updates panel frame and textures; FPS updates frame pacing; neither may reopen SQLite or recreate `PrintService`.

- [ ] **Step 5: Isolate native failures**

If Metal initialization or capture fails:

- keep `PetRuntime`, SQLite, import and settlement active;
- release capture frames;
- switch effective mode to lite/Core Animation;
- publish one stable `fallback_reason`;
- avoid retry loops faster than one user action or one wake/display-change event.

`RuntimeCore::for_test_with_mapped_fixture` creates an in-memory database, two 1000 g test spools, imports `tests/fixtures/bambu_multicolor.3mf`, and confirms tool mappings before the test action. It never starts AppKit; its native sink records commands in memory so the same business flow can be asserted deterministically.

- [ ] **Step 6: Run full suites and two-display manual check**

Run:

```bash
npm test
npm run build
cd src-tauri
cargo test
```

Manually drag across every connected display, change size and FPS while moving, disconnect the secondary display, then import and settle a fixture. The pet must remain visible and the ledger must remain correct.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/pet src-tauri/native/mac src/features/settings
git commit -m "fix: harden black hole lifecycle"
```

### Task 9: Real-file verification, packaging, documentation, and final acceptance

**Files:**
- Create: `docs/qa-black-hole.md`
- Modify: `docs/install-mac.md`
- Modify: `docs/design.md`
- Modify: `src-tauri/src/imports.rs`
- Modify: `src-tauri/src/settlement.rs`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: completed desktop pet feature and existing ignored real-file smoke.
- Produces: release `.app`/`.dmg`, reproducible test log, permission instructions and manual acceptance record.

- [ ] **Step 1: Extend the ignored real-file smoke without embedding the user path**

Keep `BAMBU_SMOKE_3MF` as the only input. Add assertions:

```rust
fn balance_rows(service: &PrintService) -> Vec<(String, f64)> {
    let mut statement = service.database.connection
        .prepare("SELECT spool_id, remaining_grams FROM spools ORDER BY spool_id").unwrap();
    statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap()
        .collect::<rusqlite::Result<Vec<_>>>().unwrap()
}

let metadata_before = std::fs::metadata(&path).unwrap();
let source_hash_before = crate::imports::sha256(&path).unwrap();
let balances_before = balance_rows(&service);
let preview = service.import_print_file(&path).unwrap();
assert_eq!(balance_rows(&service), balances_before);
assert!(service.pending_summary().unwrap().count >= 1);
let metadata_after = std::fs::metadata(&path).unwrap();
assert_eq!(metadata_before.len(), metadata_after.len());
assert_eq!(source_hash_before, crate::imports::sha256(&path).unwrap());
```

Change the existing helper to `pub(crate) fn sha256(path: &Path) -> Result<String>` so the settlement smoke can reuse production hashing. The test must not print the full path.

- [ ] **Step 2: Run automated verification including the user-owned fixture**

Run:

```bash
npm test
npm run build
cd src-tauri
cargo test
BAMBU_SMOKE_3MF='/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf' \
  cargo test smoke_real_sliced_file_from_environment -- --ignored --nocapture
```

Expected: frontend and Rust suites pass; real smoke reports four tool usages, success/failed/cancelled/50% results, idempotency and reversal; the file hash and length remain unchanged.

- [ ] **Step 3: Write the exact manual acceptance matrix**

`docs/qa-black-hole.md` must record pass/fail and evidence for:

- move over Finder file without import;
- drop one and multiple supported files;
- unsliced/corrupt/unsupported rejection without task or balance change;
- short click with and without pending jobs;
- real/lite mode, permission grant/deny/revoke/restart;
- 120/360 px limits and 160/220/300 presets;
- auto/30/60 FPS and hidden pause;
- two-screen move, Retina scale, disconnect and reconnect;
- sleep/wake and full-screen Space;
- import animation, amber pending point, green settlement and red failure;
- renderer failure with continuing import/settlement;
- repeated launch with one process/tray/pet;
- 30-minute run with no capture recursion or sustained memory growth.

- [ ] **Step 4: Update installation and privacy documentation**

Explain:

- where to enable Screen Recording in System Settings;
- real mode requires macOS 12.3+, older systems use lite mode;
- no audio, microphone or cursor is captured;
- frames are memory/GPU-only and never saved or uploaded;
- quit old versions and eject temporary DMGs before launching the new build;
- settings and business backups are separate;
- how to switch to lite mode if permission or GPU use is undesirable.

- [ ] **Step 5: Build the release and inspect its metadata**

Run:

```bash
npm run tauri build
codesign --verify --deep --strict src-tauri/target/release/bundle/macos/拓竹耗材管家.app
plutil -p src-tauri/target/release/bundle/macos/拓竹耗材管家.app/Contents/Info.plist
```

Expected: `.app` and `.dmg` exist; signature verification succeeds; there is no `menubar` WebView; one bundle identifier is present and no camera/microphone usage key is added.

- [ ] **Step 6: Launch only the release app and finish the acceptance checklist**

Quit development and mounted-DMG copies first. Launch:

```bash
open '/Users/robin/Desktop/耗材管理/src-tauri/target/release/bundle/macos/拓竹耗材管家.app'
```

Complete every row in `docs/qa-black-hole.md`. Any failing row returns to the responsible task and its focused test before the release is accepted.

- [ ] **Step 7: Commit**

```bash
git add docs src-tauri/src/settlement.rs src-tauri/tauri.conf.json
git commit -m "docs: verify the black hole release"
```

## Final Verification

- [ ] `git status --short` is clean.
- [ ] `npm test` passes.
- [ ] `npm run build` passes.
- [ ] `cargo test` passes with only the intentional real-file smoke ignored during the normal suite.
- [ ] The explicit `BAMBU_SMOKE_3MF` smoke passes and leaves the source file unchanged.
- [ ] Release `.app` starts with one process, one tray icon and one black hole.
- [ ] Real distortion, light fallback, safe file drop, pending status and settlement status pass the manual matrix.
- [ ] No `pet_*` device coordinate or screen frame appears in exported backup, logs or network activity.
