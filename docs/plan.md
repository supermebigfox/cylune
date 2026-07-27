# 拓竹耗材管家 macOS 本地原型 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付一个可安装的 macOS Tauri App，能够从正常主窗口或菜单栏浮窗导入已切片 3MF、识别拓竹原厂耗材配置、管理不限数量的独立耗材卷与四个 AMS Lite 槽位，并以简体中文、繁体中文或英语按成功、失败或取消结果更新库存流水。

**Architecture:** React/TypeScript 负责主窗口、菜单栏 WebView、国际化和主题界面，Tauri/Rust 负责菜单栏图标、窗口生命周期、文件系统、流式 3MF/G-code 解析、SQLite 和原生通知。所有模型文件留在本机；本计划只完成离线 Mac 原型，不实现账号、网页或云同步。

**Tech Stack:** Tauri 2、Rust stable、React 19、TypeScript 5、Vite、SQLite/rusqlite、zip、notify、i18next/react-i18next、Motion、Phosphor Icons、Vitest、React Testing Library。

## Global Constraints

- 第一版只处理由电脑端 Bambu Studio 生成并导入本 App 的切片任务。
- 只支持一台 A1 和一套四槽 AMS Lite，但耗材库卷数不设上限。
- 每卷耗材拥有独立 `spool_id`、余额和流水；相同品牌、系列、材质和颜色的卷不得合并。
- 3MF/G-code、模型网格和缩略图不得离开本机。
- 成功任务按完整 G-code 结算；失败/取消按停止层结算；仅百分比时必须标记估算。
- 未取得已切片 3MF/G-code 的任务不得产生分色扣减。
- Rust 解析器必须流式读取 G-code，不得把完整 G-code 解压到内存。
- macOS App 不连接打印机、不读取 AMS RFID、不调用拓竹云接口。
- 每个任务和流水事件必须幂等；撤销通过反向流水实现，不删除历史。
- App 同时保留正常 Dock 主程序和 macOS 菜单栏快捷入口；菜单栏浮窗不替代完整主窗口。
- 所有 App 自有可见文案必须来自 `zh-CN`、`zh-TW`、`en` 三套语言资源；Bambu 官方预设名、用户卷名和文件名保持原值。
- 首次主题跟随 macOS 外观；手动切换日间或夜间后持久化到 `bambu-spools.theme`，切换不得中断解析。
- 品牌图标使用已确认的“四根耗材汇入一个喷嘴”概念；不得采用圆环、旋转叶片或类似 Chrome 的构图。
- 视觉参数固定为 `DESIGN_VARIANCE=7`、`MOTION_INTENSITY=5`、`VISUAL_DENSITY=6`；动效必须支持 `prefers-reduced-motion`。
- 展示字体只用于标题、品牌语和关键空状态；正文、表单、表格和数值使用高可读系统字体。
- 项目目录、文件名和代码标识在不损失含义的前提下保持简洁，不重复目录已经表达的上下文，也不使用难懂缩写；此规则不限制应用界面文案。

---

## File Map

- `package.json`：前端、测试和 Tauri 脚本。
- `src/main.tsx`：React 入口。
- `src/App.tsx`：顶层路由和应用壳。
- `src/styles.css`：全局设计系统和响应式布局。
- `src/assets/brand/filament-mark.svg`：彩色品牌图标源文件。
- `src/assets/brand/filament-mark-template.svg`：macOS 菜单栏单色模板源文件。
- `src/assets/fonts/SmileySans-Oblique.woff2`：简中和英语标题展示字体。
- `src/assets/fonts/OFL.txt`：展示字体许可证。
- `src/brand/Mark.tsx`：可访问的品牌图标组件。
- `src/i18n/index.ts`：语言检测、切换和类型定义。
- `src/i18n/locales/zh-CN.json`：简体中文文案。
- `src/i18n/locales/zh-TW.json`：繁体中文文案。
- `src/i18n/locales/en.json`：英语文案。
- `src/theme/Theme.tsx`：主题检测、切换和持久化。
- `src/features/tray/TrayDrop.tsx`：菜单栏拖放与最近任务浮窗。
- `src/lib/tauri.ts`：类型安全的 Tauri command 包装。
- `src/features/home/Home.tsx`：四槽、待结算任务和提醒。
- `src/features/spools/Spools.tsx`：不限数量耗材库与筛选。
- `src/features/jobs/Job.tsx`：导入结果、槽位匹配和结算。
- `src/features/settings/Settings.tsx`：监控文件夹、通知和备份。
- `src/test/setup.ts`：前端测试环境。
- `src-tauri/Cargo.toml`：Rust 依赖。
- `src-tauri/tauri.conf.json`：App 标识、窗口和打包配置。
- `src-tauri/src/lib.rs`：Tauri command 注册和应用状态。
- `src-tauri/src/domain.rs`：稳定领域类型。
- `src-tauri/src/error.rs`：统一错误类型与前端错误码。
- `src-tauri/src/db.rs`：SQLite 连接、迁移和事务入口。
- `src-tauri/src/inventory.rs`：耗材卷、槽位和不可变流水。
- `src-tauri/src/parser/gcode.rs`：逐行 G-code 解析器。
- `src-tauri/src/parser/three_mf.rs`：3MF ZIP、元数据和 G-code 条目读取。
- `src-tauri/src/parser/mod.rs`：解析接口与共享类型。
- `src-tauri/src/settlement.rs`：成功、停止层和百分比结算。
- `src-tauri/src/imports.rs`：文件稳定检测、哈希、重复导入和文件夹监听。
- `src-tauri/src/tray.rs`：macOS 菜单栏图标、浮窗定位和主窗口跳转。
- `src-tauri/src/backup.rs`：非敏感数据导出与恢复。
- `src-tauri/migrations/001_init.sql`：本地数据库初始结构。
- `src-tauri/tests/fixtures/`：人工构造且不含用户模型的黄金样本。

---

### Task 1: Tauri 工程与本地健康检查

**Files:**
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `index.html`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/styles.css`
- Create: `src/test/setup.ts`
- Create: `src/App.test.tsx`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: macOS Command Line Tools；Rust stable 需通过官方 rustup 安装。
- Produces: `npm run dev`、`npm test`、`npm run tauri dev`、`npm run tauri build`。

- [ ] **Step 1: 安装并验证 Rust stable**

Run: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal`

Run: `rustc --version`

Run: `cargo --version`

Expected: 两个命令均输出 stable 版本且退出码为 0。

- [ ] **Step 2: 写前端失败测试**

```tsx
import { render, screen } from "@testing-library/react";
import { App } from "./App";

it("shows the local prototype shell", () => {
  render(<App />);
  expect(screen.getByRole("heading", { name: "拓竹耗材管家" })).toBeVisible();
  expect(screen.getByText("本地模式")).toBeVisible();
});
```

- [ ] **Step 3: 运行测试确认失败**

Run: `npm install`

Run: `npm test -- --run src/App.test.tsx`

Expected: FAIL，因为 `App` 尚未导出或没有标题。

- [ ] **Step 4: 实现最小应用壳与 Tauri 配置**

```tsx
export function App() {
  return <main><p>本地模式</p><h1>拓竹耗材管家</h1></main>;
}
```

Tauri bundle identifier 使用 `com.local.bambuspools`，窗口标题使用“拓竹耗材管家”，默认尺寸 `1180x760`，最小尺寸 `900x640`。

- [ ] **Step 5: 验证前端与桌面启动**

Run: `npm test -- --run src/App.test.tsx`

Expected: PASS。

Run: `npm run tauri dev`

Expected: macOS 窗口打开并显示标题；关闭窗口后命令正常退出。

- [ ] **Step 6: 提交**

```bash
git add package.json package-lock.json vite.config.ts tsconfig.json index.html src src-tauri
git commit -m "feat: scaffold macOS filament manager"
```

### Task 2: 领域类型、SQLite 迁移与错误边界

**Files:**
- Create: `src-tauri/src/domain.rs`
- Create: `src-tauri/src/error.rs`
- Create: `src-tauri/src/db.rs`
- Create: `src-tauri/migrations/001_init.sql`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db.rs`

**Interfaces:**
- Consumes: Task 1 的 Tauri 工程。
- Produces: `AppDatabase::open(path) -> Result<AppDatabase>`、`Spool`、`SlotAssignment`、`PrintJob`、`LedgerEvent`、`Confidence`、`JobOutcome`。

- [ ] **Step 1: 写迁移失败测试**

```rust
#[test]
fn migration_creates_inventory_tables() {
    let db = AppDatabase::open_in_memory().unwrap();
    for table in ["spools", "ams_slots", "print_jobs", "job_consumption", "ledger_events", "app_settings"] {
        assert!(db.table_exists(table).unwrap(), "missing {table}");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml migration_creates_inventory_tables`

Expected: FAIL，因为 `AppDatabase` 或迁移不存在。

- [ ] **Step 3: 定义领域类型和数据库约束**

```rust
pub struct Spool { pub spool_id: Uuid, pub display_name: String, pub brand: String, pub material: String, pub series: String, pub color_hex: String, pub remaining_grams: f64, pub status: SpoolStatus }
pub enum JobOutcome { Success, Failed { stop_layer: u32 }, Cancelled { stop_layer: u32 }, Estimated { progress_percent: f32 } }
pub enum Confidence { Exact, Estimated, NeedsConfirmation }
```

迁移必须包含四条固定 `ams_slots` 记录（1–4）、`spool_id` 外键、事件唯一 ID、任务结算版本唯一约束和非负重量检查。

- [ ] **Step 4: 实现迁移与统一错误码**

`AppError` 至少包含 `invalid_file`、`unsliced_project`、`unknown_gcode`、`slot_conflict`、`duplicate_job`、`database`、`io`，序列化给前端时不得暴露本机绝对路径之外的敏感内容。

- [ ] **Step 5: 运行数据库测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml db::tests`

Expected: PASS，且四槽初始化测试确认槽位编号恰好为 `[1,2,3,4]`。

- [ ] **Step 6: 提交**

```bash
git add src-tauri
git commit -m "feat: add local inventory database"
```

### Task 3: 流式 G-code 分色挤出解析器

**Files:**
- Create: `src-tauri/src/parser/mod.rs`
- Create: `src-tauri/src/parser/gcode.rs`
- Create: `src-tauri/tests/fixtures/single_color.gcode`
- Create: `src-tauri/tests/fixtures/tool_changes.gcode`
- Test: `src-tauri/src/parser/gcode.rs`

**Interfaces:**
- Consumes: `domain::Confidence`。
- Produces: `parse_gcode<R: BufRead>(reader: R) -> Result<GcodeReport>`；`GcodeReport { layers: Vec<LayerUsage>, totals_mm: BTreeMap<u8, f64>, max_layer: u32 }`。

- [ ] **Step 1: 写绝对挤出和工具切换失败测试**

```rust
#[test]
fn separates_usage_by_tool_and_ignores_retraction() {
    let src = b"M82\nT0\nG1 E10\nG1 E8\nG1 E15\nT1\nG92 E0\nG1 E4\n";
    let report = parse_gcode(&src[..]).unwrap();
    assert_eq!(report.totals_mm[&0], 17.0);
    assert_eq!(report.totals_mm[&1], 4.0);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml parser::gcode::tests`

Expected: FAIL，因为解析器不存在。

- [ ] **Step 3: 实现逐行状态机**

状态机必须处理 `M82`、`M83`、`G92 E`、`Tn`、`G0/G1 E`、回抽、恢复、注释层标记和 CRLF；未知无害指令跳过，缺少任何挤出指令返回 `unknown_gcode`。

- [ ] **Step 4: 添加层累计和相对挤出测试**

```rust
assert_eq!(report.layers[9].cumulative_mm[&0], 42.5);
assert_eq!(report.max_layer, 10);
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml parser::gcode::tests`

Expected: PASS。

- [ ] **Step 5: 添加流式内存约束测试**

使用生成式 `BufRead` 提供 100 万行而不创建完整字符串，断言解析完成且读取器最大单次缓冲小于 1 MiB。

Run: `cargo test --manifest-path src-tauri/Cargo.toml parses_large_stream_with_bounded_buffer -- --nocapture`

Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/parser src-tauri/tests/fixtures
git commit -m "feat: parse multicolor gcode usage"
```

### Task 4: 3MF 容器与拓竹原厂耗材配置

**Files:**
- Create: `src-tauri/src/parser/three_mf.rs`
- Create: `src-tauri/tests/fixtures/bambu_multicolor.3mf`
- Create: `src-tauri/tests/fixtures/project_only.3mf`
- Modify: `src-tauri/src/parser/mod.rs`
- Test: `src-tauri/src/parser/three_mf.rs`

**Interfaces:**
- Consumes: `parse_gcode`。
- Produces: `parse_3mf(path: &Path) -> Result<ParsedPrintFile>`；`FilamentProfile { tool: u8, preset_id: String, brand: String, material: String, series: String, color_hex: String, diameter_mm: f64, density_g_cm3: f64 }`。

- [ ] **Step 1: 写已切片 3MF 失败测试**

```rust
let parsed = parse_3mf(fixture("bambu_multicolor.3mf")).unwrap();
assert_eq!(parsed.filaments[0].material, "PLA");
assert_eq!(parsed.filaments[0].series, "Basic");
assert_eq!(parsed.filaments[1].series, "Matte");
assert_eq!(parsed.gcode.totals_mm.len(), 2);
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml parser::three_mf::tests`

Expected: FAIL，因为 3MF 解析器不存在。

- [ ] **Step 3: 实现 ZIP 条目发现和元数据规范化**

只读取 `Metadata/*.config`、切片信息和 `.gcode` 条目；将 `Bambu PLA Basic` 规范化为 `brand=Bambu Lab, material=PLA, series=Basic`，保留原始预设 ID 和未知字段。

- [ ] **Step 4: 区分项目文件和切片文件**

```rust
let err = parse_3mf(fixture("project_only.3mf")).unwrap_err();
assert_eq!(err.code(), "unsliced_project");
```

Run: `cargo test --manifest-path src-tauri/Cargo.toml parser::three_mf::tests`

Expected: PASS。

- [ ] **Step 5: 验证克数换算**

实现 `grams = length_mm * PI * (diameter_mm / 2)^2 / 1000 * density_g_cm3`，使用固定样本断言误差小于 `0.01g`。

- [ ] **Step 6: 提交**

```bash
git add src-tauri/src/parser src-tauri/tests/fixtures
git commit -m "feat: read Bambu filament profiles from 3mf"
```

### Task 5: 独立耗材卷、四槽映射与库存流水

**Files:**
- Create: `src-tauri/src/inventory.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/inventory.rs`

**Interfaces:**
- Consumes: `AppDatabase`、`Spool`、`LedgerEvent`。
- Produces: `create_spool`、`mount_spool`、`unmount_slot`、`move_spool`、`calibrate_spool`、`archive_spool`、`list_spools` Tauri commands。

- [ ] **Step 1: 写同款同色独立余额失败测试**

```rust
let a = service.create_spool(new_bambu_black()).unwrap();
let b = service.create_spool(new_bambu_black()).unwrap();
service.calibrate_spool(a, 620.0).unwrap();
assert_eq!(service.get_spool(a).unwrap().remaining_grams, 620.0);
assert_eq!(service.get_spool(b).unwrap().remaining_grams, 1000.0);
assert_ne!(a, b);
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml inventory::tests`

Expected: FAIL，因为库存服务不存在。

- [ ] **Step 3: 实现装入、拆下、换卷和调槽事务**

一个槽位最多绑定一卷，同一卷最多绑定一个槽位；换卷必须在同一 SQLite 事务内解除旧绑定并建立新绑定。

- [ ] **Step 4: 实现校准和不可变流水**

校准写入差额事件，例如从 `650g` 校准到 `628g` 时写 `-22g`，不得改写既有消耗事件。

- [ ] **Step 5: 运行库存测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml inventory::tests`

Expected: PASS，包括槽位冲突、归档已装载卷失败和撤销流水守恒。

- [ ] **Step 6: 提交**

```bash
git add src-tauri
git commit -m "feat: manage spool library and AMS slots"
```

### Task 6: 任务导入、实体卷匹配与结算

**Files:**
- Create: `src-tauri/src/settlement.rs`
- Create: `src-tauri/src/imports.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/settlement.rs`
- Test: `src-tauri/src/imports.rs`

**Interfaces:**
- Consumes: `parse_3mf`、库存服务和数据库。
- Produces: `import_print_file(path) -> ImportPreview`、`confirm_job_mapping(job_id, mappings)`、`settle_job(job_id, outcome)`、`reverse_settlement(job_id)`。

- [ ] **Step 1: 写任务幂等和精确结算失败测试**

```rust
let first = service.import_print_file(fixture("bambu_multicolor.3mf")).unwrap();
let second = service.import_print_file(fixture("bambu_multicolor.3mf")).unwrap();
assert_eq!(first.source_hash, second.source_hash);
assert_eq!(service.parse_result_count(first.source_hash).unwrap(), 1);
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml settlement::tests`

Run: `cargo test --manifest-path src-tauri/Cargo.toml imports::tests`

Expected: FAIL，因为导入和结算服务不存在。

- [ ] **Step 3: 实现 SHA-256、文件稳定检测和匹配建议**

文件大小和修改时间连续两次、间隔 750ms 相同后才解析。匹配键为 `preset_id + material + series + color_hex`；唯一候选可建议，多候选必须返回 `NeedsConfirmation`。

- [ ] **Step 4: 实现三种结算**

成功使用总累计；停止层使用该层累计；百分比将百分比映射到最近层并返回 `Confidence::Estimated`。所有扣减绑定具体 `spool_id` 并保存槽位快照。

- [ ] **Step 5: 实现撤销和重复提交保护**

相同任务结算版本再次提交返回原结果；撤销创建等量反向事件；第二次撤销返回 `already_reversed`。

- [ ] **Step 6: 运行服务测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml settlement::tests`

Run: `cargo test --manifest-path src-tauri/Cargo.toml imports::tests`

Expected: PASS。

- [ ] **Step 7: 提交**

```bash
git add src-tauri
git commit -m "feat: import and settle print jobs"
```

### Task 7: 品牌图标、三语言与日夜主题基础

**Files:**
- Create: `src/assets/brand/filament-mark.svg`
- Create: `src/assets/brand/filament-mark-template.svg`
- Create: `src/assets/fonts/SmileySans-Oblique.woff2`
- Create: `src/assets/fonts/OFL.txt`
- Create: `src/brand/Mark.tsx`
- Create: `src/i18n/index.ts`
- Create: `src/i18n/locales/zh-CN.json`
- Create: `src/i18n/locales/zh-TW.json`
- Create: `src/i18n/locales/en.json`
- Create: `src/i18n/i18n.test.ts`
- Create: `src/theme/Theme.tsx`
- Create: `src/theme/Theme.test.tsx`
- Modify: `src/main.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: Task 1 的 React 工程。
- Produces: `SupportedLocale = "zh-CN" | "zh-TW" | "en"`、`setLocale(locale)`、`ThemeMode = "light" | "dark"`、`useTheme()`、`Mark`。

- [ ] **Step 1: 写语言完整性与主题切换失败测试**

```tsx
it("keeps all locale key sets identical", () => {
  expect(flattenKeys(zhTW)).toEqual(flattenKeys(zhCN));
  expect(flattenKeys(en)).toEqual(flattenKeys(zhCN));
});

it("persists a manual dark theme without reloading", async () => {
  render(<Theme><ThemeProbe /></Theme>);
  await userEvent.click(screen.getByRole("button", { name: "深色" }));
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(localStorage.getItem("bambu-spools.theme")).toBe("dark");
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- --run src/i18n/i18n.test.ts src/theme/Theme.test.tsx`

Expected: FAIL，因为语言资源和 `Theme` 不存在。

- [ ] **Step 3: 实现三语言资源与运行时切换**

`SupportedLocale` 只接受 `zh-CN`、`zh-TW`、`en`。首次启动读取 `navigator.languages`，按完整语言标签和语言主标签匹配；无匹配时使用 `zh-CN`。选择保存到 `bambu-spools.locale`，优先级高于系统语言。三套资源至少覆盖导航、四槽、耗材库、任务、导入、结算、设置、通知、错误码和菜单栏操作。

```ts
export type SupportedLocale = "zh-CN" | "zh-TW" | "en";
export const supportedLocales: SupportedLocale[] = ["zh-CN", "zh-TW", "en"];
export async function setLocale(locale: SupportedLocale): Promise<void> {
  localStorage.setItem("bambu-spools.locale", locale);
  await i18n.changeLanguage(locale);
}
```

- [ ] **Step 4: 实现无重载日夜主题**

首次读取 `prefers-color-scheme`；用户手动选择后保存到 `bambu-spools.theme`。主题只通过根元素 `data-theme="light|dark"` 和语义 CSS 变量切换，不重新挂载 App，不修改任务状态。

```ts
export type ThemeMode = "light" | "dark";
export type ThemeContextValue = { theme: ThemeMode; setTheme(theme: ThemeMode): void; toggleTheme(): void };
```

- [ ] **Step 5: 重绘品牌图标与字体资产**

以“四根圆角耗材自上而下汇入一个喷嘴”为唯一轮廓，输出彩色 SVG 与纯黑可模板化 SVG；不得包含文字、圆环或旋转叶片。彩色版只使用 `#FF645A`、`#FFC84A`、`#316BFF`、`#3AD6A0` 和 `#252733`。标题字体使用官方 Smiley Sans 2.0.1 WOFF2 与 OFL 1.1 许可证；繁中缺字回退 `PingFang TC`，正文栈使用 `-apple-system, BlinkMacSystemFont, "Segoe UI", "PingFang SC", "PingFang TC", sans-serif`。

- [ ] **Step 6: 运行基础测试和构建**

Run: `npm test -- --run src/i18n/i18n.test.ts src/theme/Theme.test.tsx`

Expected: PASS，三套语言键集合完全一致，主题选择可持久化。

Run: `npm run build`

Expected: PASS，字体与 SVG 被 Vite 打包。

- [ ] **Step 7: 提交**

```bash
git add src package.json package-lock.json
git commit -m "feat: add brand localization and themes"
```

### Task 8: React 桌面界面与 Tauri command 绑定

**Files:**
- Create: `src/lib/tauri.ts`
- Create: `src/features/home/Home.tsx`
- Create: `src/features/spools/Spools.tsx`
- Create: `src/features/jobs/Job.tsx`
- Create: `src/features/settings/Settings.tsx`
- Create: `src/features/home/Home.test.tsx`
- Create: `src/features/jobs/Job.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: Task 5–6 的 Tauri commands 与 Task 7 的国际化和主题基础。
- Produces: 首页、耗材库、任务、设置四个界面和类型安全调用层。

- [ ] **Step 1: 写首页失败测试**

```tsx
render(<Home slots={fourSlots} pendingJobs={[]} />);
expect(screen.getAllByTestId("ams-slot")).toHaveLength(4);
expect(screen.getByText("耗材库 6 卷")).toBeVisible();
```

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- --run src/features/home/Home.test.tsx src/features/jobs/Job.test.tsx`

Expected: FAIL，因为界面不存在。

- [ ] **Step 3: 实现桌面信息架构**

首页第一屏展示四槽、待结算任务和低库存；耗材库支持状态/材质/颜色筛选；任务页展示模型、冲刷、总用量、置信度和具体卷映射；设置页展示监控文件夹和本地数据位置。

视觉实现固定为柔和多巴胺工具风格：非对称首页信息区、统一 `16px` 卡片圆角、胶囊主按钮、有色但低扩散阴影；多巴胺色只用于真实耗材颜色、状态和关键操作。标题使用展示字体，正文和克数使用系统字体。所有可见文案通过翻译键获取，不在组件中硬编码中文或英文。

- [ ] **Step 4: 实现关键交互**

拖入文件后打开 `Job`；同款多卷用单选列表要求选择具体 `spool_id`；成功、失败、取消按钮有明确确认；失败表单要求停止层，百分比路径显示“估算”标签。

设置页提供简中、繁中、英语选择和日间/夜间开关；切换后不刷新页面。所有按钮提供悬停、按下、键盘焦点、加载、空、错误和禁用状态；动效只改变 `transform` 与 `opacity`，并在 `prefers-reduced-motion: reduce` 时退化为即时切换。

- [ ] **Step 5: 验证前端测试和构建**

Run: `npm test -- --run`

Expected: PASS。

Run: `npm run build`

Expected: TypeScript 和 Vite 构建成功。

- [ ] **Step 6: 提交**

```bash
git add src package.json package-lock.json
git commit -m "feat: add desktop inventory workflows"
```

### Task 9: 菜单栏拖放、文件夹监听、通知、备份与 macOS 打包

**Files:**
- Modify: `src-tauri/src/imports.rs`
- Create: `src-tauri/src/tray.rs`
- Create: `src-tauri/src/backup.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`
- Create: `src/features/tray/TrayDrop.tsx`
- Create: `src/features/tray/TrayDrop.test.tsx`
- Modify: `src/features/settings/Settings.tsx`
- Create: `docs/install-mac.md`
- Test: `src-tauri/src/backup.rs`

**Interfaces:**
- Consumes: 导入服务、数据库、设置页、品牌图标、国际化和主题上下文。
- Produces: `set_watch_folder`、`export_backup`、`import_backup`、菜单栏快捷入口、系统通知和 `.app/.dmg`。

- [ ] **Step 1: 写备份不含敏感文件失败测试**

```rust
let archive = export_backup(&db, temp.path()).unwrap();
assert!(archive.contains("inventory.json"));
assert!(!archive.contains("*.3mf"));
assert!(!archive.contains("device_token"));
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml backup::tests`

Expected: FAIL，因为备份模块不存在。

- [ ] **Step 3: 写菜单栏拖放失败测试**

```tsx
it("accepts only supported sliced file types", async () => {
  render(<TrayDrop onImport={onImport} />);
  fireEvent.drop(screen.getByTestId("menu-dropzone"), {
    dataTransfer: { files: [file("plate.gcode.3mf"), file("notes.pdf")] },
  });
  expect(onImport).toHaveBeenCalledWith("plate.gcode.3mf");
  expect(screen.getByText("不支持 notes.pdf")).toBeVisible();
});
```

Run: `npm test -- --run src/features/tray/TrayDrop.test.tsx`

Expected: FAIL，因为菜单栏拖放组件不存在。

- [ ] **Step 4: 实现菜单栏图标、浮窗和主窗口跳转**

使用 Tauri `TrayIcon` 和 `on_tray_icon_event`。左键单击在图标下方定位并切换 `menubar` WebViewWindow；窗口失焦后隐藏但不销毁。浮窗尺寸固定为 `380x460`，不显示系统标题栏。浮窗支持 `.3mf`、`.gcode.3mf`、`.gcode` 拖放，显示悬停、解析、成功、未切片和错误状态；“查看并绑定料卷”调用 `open_job_in_main(job_id)` 显示主窗口并定位任务。关闭主窗口只隐藏窗口；菜单栏提供“打开主程序”和“退出”。

- [ ] **Step 5: 实现监控文件夹和通知**

只监听用户明确选择的单一文件夹；仅接收 `.3mf`、`.gcode.3mf` 和 `.gcode`；新文件稳定后进入导入队列；解析完成后发送“任务等待结算”系统通知。

- [ ] **Step 6: 实现 JSON 备份和事务恢复**

备份包含 schema 版本、耗材卷、槽位、任务、流水和非敏感设置；导入前创建自动备份；事件 ID 和源哈希去重；恢复失败整体回滚。

- [ ] **Step 7: 全量验证**

Run: `npm test -- --run`

Expected: PASS。

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS。

Run: `npm run tauri build`

Expected: 生成可启动的 `.app` 和 `.dmg`；首次启动无需 Docker、Java 或浏览器插件。

- [ ] **Step 8: 手工验收清单**

在干净测试数据库上完成：新建两卷相同黑色 PLA、分别装入和拆下、从菜单栏拖入双色切片 3MF、确认具体卷、成功结算、撤销、失败按层结算、在简中/繁中/英语间切换、在日间/夜间间切换、关闭主窗口后继续从菜单栏导入、断网无影响、导出和恢复备份。

- [ ] **Step 9: 提交**

```bash
git add src src-tauri docs/install-mac.md
git commit -m "feat: package the macOS local prototype"
```

---

## Plan Self-Review Result

- Spec coverage: Mac 原型、独立耗材卷、四槽映射、原厂预设识别、精确/估算结算、菜单栏拖放、品牌图标、简中/繁中/英语、日夜主题、离线、备份、通知和打包均有对应任务。
- Explicitly deferred: 网页、账号、设备配对、云同步、Windows 安装包和 Handy 手机任务；这些属于后续独立计划。
- Placeholder scan: 未发现占位标记、模糊错误处理步骤或未定义接口。
- Type consistency: `Spool`、`JobOutcome`、`Confidence`、`ParsedPrintFile`、`ImportPreview`、`SupportedLocale`、`ThemeMode` 和 command 名称在所有任务中保持一致。
