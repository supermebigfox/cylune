# 拓竹耗材管家 macOS 本地原型 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付一个可安装的 macOS Tauri App，能够本地导入已切片 3MF、识别拓竹原厂耗材配置、管理不限数量的独立耗材卷与四个 AMS Lite 槽位，并按成功、失败或取消结果更新库存流水。

**Architecture:** React/TypeScript 负责桌面界面，Tauri/Rust 负责文件系统、流式 3MF/G-code 解析、SQLite 和原生通知。所有模型文件留在本机；本计划只完成离线 Mac 原型，不实现账号、网页或云同步。

**Tech Stack:** Tauri 2、Rust stable、React 19、TypeScript 5、Vite、SQLite/rusqlite、zip、notify、Vitest、React Testing Library。

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

---

## File Map

- `package.json`：前端、测试和 Tauri 脚本。
- `src/main.tsx`：React 入口。
- `src/App.tsx`：顶层路由和应用壳。
- `src/styles.css`：全局设计系统和响应式布局。
- `src/lib/tauri.ts`：类型安全的 Tauri command 包装。
- `src/features/dashboard/Dashboard.tsx`：四槽、待结算任务和提醒。
- `src/features/spools/SpoolLibrary.tsx`：不限数量耗材库与筛选。
- `src/features/jobs/JobReview.tsx`：导入结果、槽位匹配和结算。
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

### Task 7: React 桌面界面与 Tauri command 绑定

**Files:**
- Create: `src/lib/tauri.ts`
- Create: `src/features/dashboard/Dashboard.tsx`
- Create: `src/features/spools/SpoolLibrary.tsx`
- Create: `src/features/jobs/JobReview.tsx`
- Create: `src/features/settings/Settings.tsx`
- Create: `src/features/dashboard/Dashboard.test.tsx`
- Create: `src/features/jobs/JobReview.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: Task 5–6 的 Tauri commands。
- Produces: 首页、耗材库、任务、设置四个界面和类型安全调用层。

- [ ] **Step 1: 写首页失败测试**

```tsx
render(<Dashboard slots={fourSlots} pendingJobs={[]} />);
expect(screen.getAllByTestId("ams-slot")).toHaveLength(4);
expect(screen.getByText("耗材库 6 卷")).toBeVisible();
```

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- --run src/features/dashboard/Dashboard.test.tsx src/features/jobs/JobReview.test.tsx`

Expected: FAIL，因为界面不存在。

- [ ] **Step 3: 实现桌面信息架构**

首页第一屏展示四槽、待结算任务和低库存；耗材库支持状态/材质/颜色筛选；任务页展示模型、冲刷、总用量、置信度和具体卷映射；设置页展示监控文件夹和本地数据位置。

- [ ] **Step 4: 实现关键交互**

拖入文件后打开 `JobReview`；同款多卷用单选列表要求选择具体 `spool_id`；成功、失败、取消按钮有明确确认；失败表单要求停止层，百分比路径显示“估算”标签。

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

### Task 8: 文件夹监听、通知、备份与 macOS 打包

**Files:**
- Modify: `src-tauri/src/imports.rs`
- Create: `src-tauri/src/backup.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src/features/settings/Settings.tsx`
- Create: `docs/user/mac-installation.md`
- Test: `src-tauri/src/backup.rs`

**Interfaces:**
- Consumes: 导入服务、数据库、设置页。
- Produces: `set_watch_folder`、`export_backup`、`import_backup`、系统通知和 `.app/.dmg`。

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

- [ ] **Step 3: 实现监控文件夹和通知**

只监听用户明确选择的单一文件夹；仅接收 `.3mf`、`.gcode.3mf` 和 `.gcode`；新文件稳定后进入导入队列；解析完成后发送“任务等待结算”系统通知。

- [ ] **Step 4: 实现 JSON 备份和事务恢复**

备份包含 schema 版本、耗材卷、槽位、任务、流水和非敏感设置；导入前创建自动备份；事件 ID 和源哈希去重；恢复失败整体回滚。

- [ ] **Step 5: 全量验证**

Run: `npm test -- --run`

Expected: PASS。

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: PASS。

Run: `npm run tauri build`

Expected: 生成可启动的 `.app` 和 `.dmg`；首次启动无需 Docker、Java 或浏览器插件。

- [ ] **Step 6: 手工验收清单**

在干净测试数据库上完成：新建两卷相同黑色 PLA、分别装入和拆下、导入双色 3MF、确认具体卷、成功结算、撤销、失败按层结算、断网无影响、导出和恢复备份。

- [ ] **Step 7: 提交**

```bash
git add src src-tauri docs/user/mac-installation.md
git commit -m "feat: package the macOS local prototype"
```

---

## Plan Self-Review Result

- Spec coverage: Mac 原型、独立耗材卷、四槽映射、原厂预设识别、精确/估算结算、离线、备份、通知和打包均有对应任务。
- Explicitly deferred: 网页、账号、设备配对、云同步、Windows 安装包和 Handy 手机任务；这些属于后续独立计划。
- Placeholder scan: 未发现占位标记、模糊错误处理步骤或未定义接口。
- Type consistency: `Spool`、`JobOutcome`、`Confidence`、`ParsedPrintFile`、`ImportPreview` 和 command 名称在所有任务中保持一致。
