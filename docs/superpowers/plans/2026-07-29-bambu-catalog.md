# Bambu Filament Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the free-form spool form with an offline Bambu Lab material → series → official color picker backed by 45 filament types and 306 official color entries.

**Architecture:** A checked-in normalized catalog supplies localized metadata and multi-color swatches without a network dependency. React owns catalog browsing and roll naming, while a nullable SQLite metadata extension preserves catalog identity, full color arrays, backups, and machine-independent 3MF matching without breaking legacy spools.

**Tech Stack:** React 18, TypeScript 5.6, Vitest, Testing Library, Tauri 2, Rust, rusqlite, SQLite, Node.js catalog generator, OpenCC-JS as a generation-only development dependency.

## Global Constraints

- Bambu Lab original filament only in this release.
- The catalog snapshot contains exactly 45 official filament types and 306 official color entries from Bambu Studio 2.8.0.50.
- Temporarily sold-out entries remain selectable; no runtime stock request is allowed.
- Simplified Chinese shows official Bambu Chinese names, Traditional Chinese shows converted reviewed names, and English shows official English names.
- Solid, gradient, dual-color, and multi-color entries retain all official colors in order.
- Multiple rolls of the same catalog entry remain separate spool records with separate balances.
- Existing spool rows and schema-version-1 backups remain readable.
- Existing `color_hex` remains the primary color and existing settlement semantics do not change.
- 3MF matching supports exact preset IDs, normalized preset bases, and legacy fallback in that order.
- No new runtime dependency is added; OpenCC-JS is used only by the catalog update script.
- User-facing source files use concise names.
- The approved desktop black-hole appearance, capture, ingestion, rejection, and jet animation are unchanged.

---

### Task 1: Generate and validate the official catalog snapshot

**Files:**
- Create: `scripts/catalog.mjs`
- Create: `src/catalog/bambu.json`
- Create: `src/catalog/bambu.ts`
- Create: `src/catalog/bambu.test.ts`
- Modify: `package.json`
- Modify: `package-lock.json`

**Interfaces:**
- Consumes: Bambu Studio
  `/Applications/BambuStudio.app/Contents/Resources/profiles/BBL/filament/filaments_color_codes.json`
  and the adjacent BBL profile directory.
- Produces:
  `FilamentColor`, `bambuColors`, `materialGroups()`, `seriesFor(group)`,
  `colorsFor(group, series)`, `colorById(id)`, and
  `searchColors(entries, query, locale)` from `src/catalog/bambu.ts`.

- [ ] **Step 1: Write the failing catalog integrity tests**

```ts
import {
  bambuColors, colorsFor, materialGroups, searchColors, seriesFor,
} from "./bambu";

test("contains the complete Bambu snapshot", () => {
  expect(new Set(bambuColors.map((item) => item.sourceType)).size).toBe(45);
  expect(bambuColors).toHaveLength(306);
  expect(new Set(bambuColors.map((item) => item.id)).size).toBe(306);
});

test("keeps localized names, official codes, and every visual color", () => {
  for (const item of bambuColors) {
    expect(item.colorCode).toMatch(/^\d{5}$/);
    expect(item.names["zh-CN"]).not.toBe("");
    expect(item.names["zh-TW"]).not.toBe("");
    expect(item.names.en).not.toBe("");
    expect(item.colors.length).toBeGreaterThan(0);
    item.colors.forEach((color) => expect(color).toMatch(/^#[0-9A-F]{6}$/));
  }
});

test("normalizes retail gradient and multi-color series", () => {
  expect(colorsFor("PLA", "Basic Gradient").some((item) => item.colorCode === "10907")).toBe(true);
  expect(colorsFor("PLA", "Silk Multi-Color").some((item) => item.colors.length > 1)).toBe(true);
  expect(seriesFor("PLA")).toEqual(expect.arrayContaining(["Basic", "Matte", "Silk+", "Pure"]));
  expect(materialGroups()).toEqual(expect.arrayContaining(["PLA", "PETG", "TPU", "支撑材料"]));
});

test("searches official Chinese name and five-digit code", () => {
  expect(searchColors(bambuColors, "玉石白", "zh-CN")[0].colorCode).toBe("10100");
  expect(searchColors(bambuColors, "10100", "zh-CN")[0].names["zh-CN"]).toBe("玉石白");
});
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
npm test -- --run src/catalog/bambu.test.ts
```

Expected: FAIL because `src/catalog/bambu.ts` does not exist.

- [ ] **Step 3: Add the generation-only Traditional Chinese dependency**

Run:

```bash
npm install --save-dev opencc-js
```

Expected: `opencc-js` appears only in `devDependencies`, and the lock file is
updated.

- [ ] **Step 4: Implement the catalog generator**

`scripts/catalog.mjs` must:

- accept the color JSON path and BBL filament profile directory as its two CLI
  arguments;
- recursively resolve a profile's `inherits` chain to find its exact
  `filament_type`;
- normalize eight-digit RGBA values to uppercase six-digit RGB values;
- use OpenCC `Converter({ from: "cn", to: "tw" })` for `zh-TW`;
- derive stable IDs as `bambu:<fila_id>:<fila_color_code>`;
- classify `PLA Basic` gradients as `Basic Gradient`;
- classify non-solid `PLA Silk` entries as `Silk Multi-Color`;
- classify original solid `PLA Silk` and `PLA Tough` as classic;
- map PVA and every `Support ...` source type to `支撑材料`;
- write deterministic, source-type/color-code-sorted JSON.

The generated record must have this exact shape:

```ts
export interface FilamentColor {
  id: string;
  brand: "Bambu Lab";
  filamentId: string;
  sourceType: string;
  materialGroup: string;
  material: string;
  series: string;
  presetBase: string;
  colorCode: string;
  colorType: "solid" | "gradient" | "dual" | "multi";
  colors: string[];
  names: { "zh-CN": string; "zh-TW": string; en: string };
  classic: boolean;
}
```

- [ ] **Step 5: Generate and check in the snapshot**

Run:

```bash
node scripts/catalog.mjs \
  '/Applications/BambuStudio.app/Contents/Resources/profiles/BBL/filament/filaments_color_codes.json' \
  '/Applications/BambuStudio.app/Contents/Resources/profiles/BBL/filament'
```

Expected: `src/catalog/bambu.json` is created with 306 records, source version
`02.08.00.50`, and no dependency on `.firecrawl`.

- [ ] **Step 6: Implement typed selectors and search**

`src/catalog/bambu.ts` must import the JSON, export the interface above, and use
the explicit material order shown below. Within a material, current series sort
before classic series and each group retains deterministic source order.

```ts
export type CatalogLocale = "zh-CN" | "zh-TW" | "en";
export const bambuColors = snapshot.entries as FilamentColor[];
const GROUP_ORDER = [
  "PLA", "PETG", "ABS", "ASA", "TPU", "PC",
  "PA", "PET", "PPA", "PPS", "支撑材料",
];

export const materialGroups = () =>
  GROUP_ORDER.filter((group) =>
    bambuColors.some((item) => item.materialGroup === group));

export const seriesFor = (group: string) =>
  [...new Set(bambuColors
    .filter((item) => item.materialGroup === group)
    .map((item) => item.series))];

export const colorsFor = (group: string, series: string) =>
  bambuColors.filter((item) =>
    item.materialGroup === group && item.series === series);

export const colorById = (id: string | null | undefined) =>
  id ? bambuColors.find((item) => item.id === id) : undefined;

export function searchColors(
  entries: FilamentColor[],
  query: string,
  locale: CatalogLocale,
) {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return entries;
  return entries.filter((item) => [
    item.names[locale], item.names.en, item.colorCode, ...item.colors,
  ].some((value) => value.toLocaleLowerCase().includes(needle)));
}
```

- [ ] **Step 7: Run catalog tests and the TypeScript build**

Run:

```bash
npm test -- --run src/catalog/bambu.test.ts
npm run build
```

Expected: both commands PASS.

- [ ] **Step 8: Commit the catalog**

```bash
git add scripts/catalog.mjs src/catalog package.json package-lock.json
git commit -m "feat: add Bambu filament catalog"
```

---

### Task 2: Add a reusable solid and multi-color swatch

**Files:**
- Create: `src/components/Swatch.tsx`
- Create: `src/components/Swatch.test.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: an ordered `string[]` of RGB hex colors.
- Produces: `<Swatch colors={string[]} size?: "small" | "large" />`.

- [ ] **Step 1: Write failing visual semantics tests**

```tsx
import { render, screen } from "@testing-library/react";
import { Swatch } from "./Swatch";

test("renders a solid color without a gradient", () => {
  render(<Swatch colors={["#FFFFFF"]} />);
  expect(screen.getByTestId("swatch")).toHaveStyle({ background: "#FFFFFF" });
});

test("keeps every gradient color in order", () => {
  render(<Swatch colors={["#8EC9E9", "#E7C1D5"]} />);
  expect(screen.getByTestId("swatch").getAttribute("style"))
    .toContain("linear-gradient(135deg, #8EC9E9 0%, #E7C1D5 100%)");
});
```

- [ ] **Step 2: Run the tests and verify RED**

Run:

```bash
npm test -- --run src/components/Swatch.test.tsx
```

Expected: FAIL because `Swatch` does not exist.

- [ ] **Step 3: Implement the minimal swatch**

```tsx
export function Swatch({ colors, size = "small" }: {
  colors: string[];
  size?: "small" | "large";
}) {
  const safe = colors.length ? colors : ["#888888"];
  const background = safe.length === 1
    ? safe[0]
    : `linear-gradient(135deg, ${safe.map((color, index) =>
      `${color} ${(index / (safe.length - 1)) * 100}%`).join(", ")})`;
  return <i data-testid="swatch" aria-hidden="true"
    className={`swatch swatch-${size}`} style={{ background }} />;
}
```

Update `.swatch` to use its inline background while preserving the existing
border and theme contrast. Add `.swatch-small` and `.swatch-large` size rules.

- [ ] **Step 4: Run swatch tests**

Run:

```bash
npm test -- --run src/components/Swatch.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit the swatch**

```bash
git add src/components/Swatch.tsx src/components/Swatch.test.tsx src/styles.css
git commit -m "feat: render multi-color filament swatches"
```

---

### Task 3: Extend the frontend spool contract without breaking demo mode

**Files:**
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/tauri.test.ts`
- Modify: `src/App.test.tsx`
- Modify: `src/features/home/Home.test.tsx`
- Modify: `src/features/jobs/Job.test.tsx`
- Modify: `src/features/spools/Spools.test.tsx`

**Interfaces:**
- Consumes: catalog metadata selected by the add dialog.
- Produces: `Spool` and `NewSpool` fields
  `catalog_id`, `color_name`, `color_code`, `color_hexes`, and `preset_base`.

- [ ] **Step 1: Write a failing demo round-trip test**

```ts
test("demo API preserves official catalog metadata", async () => {
  const client = createTauriApi(undefined, {});
  const input: NewSpool = {
    display_name: "玉石白 · PLA Basic",
    preset_id: "Bambu PLA Basic",
    preset_base: "Bambu PLA Basic",
    catalog_id: "bambu:GFA00:10100",
    brand: "Bambu Lab",
    material: "PLA",
    series: "Basic",
    color_name: "玉石白",
    color_code: "10100",
    color_hex: "#FFFFFF",
    color_hexes: ["#FFFFFF"],
    remaining_grams: 1000,
  };

  await client.createSpool(input);
  expect(await client.listSpools()).toContainEqual(
    expect.objectContaining(input),
  );
});
```

Use the existing demo-client factory pattern in `src/lib/tauri.test.ts`; do not
mock list results separately from the real demo implementation.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
npm test -- --run src/lib/tauri.test.ts
```

Expected: FAIL because the new properties do not exist on `NewSpool`.

- [ ] **Step 3: Extend TypeScript types and legacy fallbacks**

Add nullable metadata to `Spool` and optional metadata to `NewSpool`:

```ts
catalog_id: string | null;
color_name: string | null;
color_code: string | null;
color_hexes: string[];
preset_base: string | null;
```

The demo `createSpool` implementation must normalize absent legacy values:

```ts
const color_hexes = input.color_hexes?.length
  ? input.color_hexes
  : [input.color_hex];
```

Update demo spools and every test fixture to use null metadata and
`color_hexes: [color_hex]` unless the test explicitly covers catalog data.

- [ ] **Step 4: Run frontend contract tests**

Run:

```bash
npm test -- --run src/lib/tauri.test.ts src/App.test.tsx \
  src/features/home/Home.test.tsx src/features/jobs/Job.test.tsx \
  src/features/spools/Spools.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit the frontend contract**

```bash
git add src/lib/tauri.ts src/lib/tauri.test.ts src/App.test.tsx \
  src/features/home/Home.test.tsx src/features/jobs/Job.test.tsx \
  src/features/spools/Spools.test.tsx
git commit -m "feat: carry spool catalog metadata"
```

---

### Task 4: Persist catalog metadata through SQLite and Rust

**Files:**
- Create: `src-tauri/migrations/005_catalog.sql`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/domain.rs`
- Modify: `src-tauri/src/inventory.rs`
- Modify: Rust spool fixtures in `src-tauri/src/imports.rs`
- Modify: Rust spool fixtures in `src-tauri/src/settlement.rs`
- Modify: Rust spool fixtures in `src-tauri/src/backup.rs`
- Modify: Rust spool fixtures in `src-tauri/src/pet/runtime.rs`

**Interfaces:**
- Consumes: extended `NewSpool` from the Tauri command.
- Produces: extended Rust `Spool` values and nullable SQLite columns.

- [ ] **Step 1: Write failing migration and inventory tests**

Add to `src-tauri/src/db.rs`:

```rust
#[test]
fn catalog_migration_adds_nullable_spool_metadata() {
    let database = AppDatabase::open_in_memory().unwrap();
    for column in [
        "catalog_id", "color_name", "color_code", "color_hexes", "preset_base",
    ] {
        assert!(column_exists(&database.connection, "spools", column).unwrap());
    }
}
```

Add to `src-tauri/src/inventory.rs`:

```rust
#[test]
fn create_spool_round_trips_catalog_metadata() {
    let mut service = InventoryService::new(AppDatabase::open_in_memory().unwrap());
    let id = service.create_spool(NewSpool {
        display_name: "多巴胺 · PLA Basic Gradient".into(),
        preset_id: Some("Bambu PLA Basic".into()),
        preset_base: Some("Bambu PLA Basic".into()),
        catalog_id: Some("bambu:GFA00:10907".into()),
        brand: "Bambu Lab".into(),
        material: "PLA".into(),
        series: "Basic Gradient".into(),
        color_name: Some("多巴胺（粉蓝渐变）".into()),
        color_code: Some("10907".into()),
        color_hex: "#8EC9E9".into(),
        color_hexes: vec!["#8EC9E9".into(), "#E7C1D5".into()],
        remaining_grams: 1000.0,
    }).unwrap();

    let spool = service.get_spool(id).unwrap();
    assert_eq!(spool.color_code.as_deref(), Some("10907"));
    assert_eq!(spool.color_hexes, vec!["#8EC9E9", "#E7C1D5"]);
}
```

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cd src-tauri
cargo test catalog_migration_adds_nullable_spool_metadata
cargo test create_spool_round_trips_catalog_metadata
```

Expected: compilation fails because the migration and Rust fields do not exist.

- [ ] **Step 3: Add the idempotent migration**

`005_catalog.sql`:

```sql
ALTER TABLE spools ADD COLUMN catalog_id TEXT;
ALTER TABLE spools ADD COLUMN color_name TEXT;
ALTER TABLE spools ADD COLUMN color_code TEXT;
ALTER TABLE spools ADD COLUMN color_hexes TEXT;
ALTER TABLE spools ADD COLUMN preset_base TEXT;
CREATE INDEX IF NOT EXISTS idx_spools_catalog ON spools(catalog_id);
CREATE INDEX IF NOT EXISTS idx_spools_preset_base
    ON spools(preset_base, material, color_hex);
```

In `db.rs`, add `CATALOG_MIGRATION`, implement
`column_exists(connection, table, column)`, and apply the migration when
`catalog_id` is absent.

- [ ] **Step 4: Extend Rust domain and inventory serialization**

Use these exact types on both `Spool` and `NewSpool`:

```rust
pub catalog_id: Option<String>,
pub color_name: Option<String>,
pub color_code: Option<String>,
#[serde(default)]
pub color_hexes: Vec<String>,
pub preset_base: Option<String>,
```

Before insert, normalize an empty color array to `vec![color_hex.clone()]`.
Serialize `color_hexes` with `serde_json::to_string`. When reading a legacy
NULL or invalid empty array, fall back to `vec![color_hex.clone()]`.

Update `INSERT`, `SELECT`, and row indexes in `create_spool`, `get_spool`,
`list_spools`, `spool_in_transaction`, and `spool_from_row`.

Update every Rust `Spool { ... }` and `NewSpool { ... }` literal found by:

```bash
rg -n 'Spool \\{|NewSpool \\{' src-tauri/src
```

Legacy fixtures receive `None` for nullable fields and
`vec![color_hex.clone()]` for `color_hexes`.

- [ ] **Step 5: Run Rust domain, database, and inventory tests**

Run:

```bash
cd src-tauri
cargo fmt
cargo test domain::tests
cargo test db::tests
cargo test inventory::tests
```

Expected: PASS.

- [ ] **Step 6: Commit persistence**

```bash
git add src-tauri/migrations/005_catalog.sql src-tauri/src
git commit -m "feat: persist filament catalog metadata"
```

---

### Task 5: Preserve catalog metadata in new and old backups

**Files:**
- Modify: `src-tauri/src/backup.rs`

**Interfaces:**
- Consumes: database spool catalog columns from Task 4.
- Produces: backup schema version 2 while accepting schema versions 1 and 2.

- [ ] **Step 1: Write failing backup compatibility tests**

Add a schema-version-2 round-trip assertion to the existing backup test:

```rust
assert_eq!(exported["schema_version"], 2);
assert_eq!(exported["spools"][0]["color_code"], "10100");
assert_eq!(
    exported["spools"][0]["color_hexes"],
    serde_json::json!(["#FFFFFF"])
);
```

Add a version-1 compatibility test that removes the five catalog fields from
the exported spool object, changes `schema_version` to `1`, imports it, and
asserts:

```rust
let restored = service.get_spool(spool_id).unwrap();
assert_eq!(restored.catalog_id, None);
assert_eq!(restored.color_hexes, vec!["#FFFFFF"]);
```

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cd src-tauri
cargo test backup::tests
```

Expected: FAIL because export is version 1 and `SpoolRow` lacks catalog fields.

- [ ] **Step 3: Implement versioned backup compatibility**

Set `BACKUP_SCHEMA_VERSION` to `2`, add the five fields to `SpoolRow`, and mark
them with `#[serde(default)]`. Represent `color_hexes` as `Vec<String>` in JSON.

Validation accepts only versions 1 and 2:

```rust
if !matches!(backup.schema_version, 1 | BACKUP_SCHEMA_VERSION)
    || backup.slots.len() != 4
{
    return Err(AppError::InvalidFile);
}
```

Export selects every catalog column. Restore inserts every catalog column and
falls back to `[color_hex]` when `color_hexes` is empty.

- [ ] **Step 4: Run backup and database tests**

Run:

```bash
cd src-tauri
cargo test backup::tests
cargo test db::tests
```

Expected: PASS.

- [ ] **Step 5: Commit backup compatibility**

```bash
git add src-tauri/src/backup.rs
git commit -m "feat: back up filament catalog metadata"
```

---

### Task 6: Match 3MF profiles by exact preset, preset base, then legacy fields

**Files:**
- Modify: `src-tauri/src/parser/mod.rs`
- Modify: `src-tauri/src/parser/three_mf.rs`
- Modify: `src-tauri/src/imports.rs`

**Interfaces:**
- Consumes: imported `FilamentProfile.preset_id` and persisted
  `Spool.preset_base`.
- Produces: `preset_base(value: &str) -> &str` and ordered candidate lookup.

- [ ] **Step 1: Write failing preset normalization tests**

Add to `src-tauri/src/parser/mod.rs`:

```rust
#[test]
fn removes_only_the_machine_suffix_from_a_preset() {
    assert_eq!(
        preset_base("Bambu PLA Basic @BBL A1"),
        "Bambu PLA Basic"
    );
    assert_eq!(preset_base("Bambu PLA Basic"), "Bambu PLA Basic");
}
```

Add import-service tests with three independently created spools:

1. exact `preset_id = Bambu PLA Basic @BBL A1`;
2. `preset_base = Bambu PLA Basic`, but a different or base-only `preset_id`;
3. legacy NULL `preset_base` with matching material, series, and color.

Assert that the matcher returns exact candidates when any exist, otherwise
base candidates, otherwise legacy candidates.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cd src-tauri
cargo test removes_only_the_machine_suffix_from_a_preset
cargo test matching_spools
```

Expected: FAIL because `preset_base` and ordered fallback do not exist.

- [ ] **Step 3: Implement the shared preset-base helper**

```rust
pub(crate) fn preset_base(value: &str) -> &str {
    value.split_once(" @").map_or(value, |(base, _)| base).trim()
}
```

Use it in `three_mf.rs` when deriving brand/series so parsing and inventory
matching share the same suffix rule.

- [ ] **Step 4: Implement ordered matching**

`matching_spools` performs three queries and stops at the first non-empty
result:

1. non-archived rows with exact `preset_id`, material, series, and primary
   color;
2. non-archived rows with exact `preset_base`, material, and primary color;
3. non-archived rows with NULL `preset_base`, material, series, and primary
   color.

Every query orders by row ID so duplicate compatible rolls remain separate,
stable choices.

- [ ] **Step 5: Run parser and import tests**

Run:

```bash
cd src-tauri
cargo test parser::three_mf::tests
cargo test imports::tests
```

Expected: PASS.

- [ ] **Step 6: Commit normalized matching**

```bash
git add src-tauri/src/parser src-tauri/src/imports.rs
git commit -m "feat: match spools across printer presets"
```

---

### Task 7: Build the catalog-driven add dialog

**Files:**
- Create: `src/features/spools/Add.tsx`
- Create: `src/features/spools/Add.test.tsx`
- Modify: `src/i18n/locales/zh-CN.json`
- Modify: `src/i18n/locales/zh-TW.json`
- Modify: `src/i18n/locales/en.json`

**Interfaces:**
- Consumes: `bambuColors`, existing `Spool[]`, `busy`, and
  `onCreate(spool: NewSpool)`.
- Produces:

```ts
export function Add(props: {
  open: boolean;
  spools: Spool[];
  busy: boolean;
  onClose(): void;
  onCreate(spool: NewSpool): Promise<boolean | void>;
}): JSX.Element | null;
```

- [ ] **Step 1: Write failing progressive-selection tests**

```tsx
test("selects material, series, and an official Chinese color", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn().mockResolvedValue(true);
  render(<Add open spools={[]} busy={false}
    onClose={() => undefined} onCreate={onCreate} />);

  await user.click(screen.getByRole("button", { name: "PLA" }));
  await user.click(screen.getByRole("button", { name: "Basic" }));
  await user.click(screen.getByRole("button", { name: /玉石白.*10100/ }));
  await user.click(screen.getByRole("button", { name: "保存" }));

  expect(onCreate).toHaveBeenCalledWith(expect.objectContaining({
    catalog_id: "bambu:GFA00:10100",
    color_name: "玉石白",
    color_code: "10100",
    color_hex: "#FFFFFF",
    color_hexes: ["#FFFFFF"],
    preset_base: "Bambu PLA Basic",
  }));
});

test("has no operating-system color input", () => {
  const { container } = render(<Add open spools={[]} busy={false}
    onClose={() => undefined} onCreate={vi.fn()} />);
  expect(container.querySelector('input[type="color"]')).toBeNull();
});

test("changing material clears downstream choices", async () => {
  const user = userEvent.setup();
  render(<Add open spools={[]} busy={false}
    onClose={() => undefined} onCreate={vi.fn()} />);
  await user.click(screen.getByRole("button", { name: "PLA" }));
  await user.click(screen.getByRole("button", { name: "Basic" }));
  await user.click(screen.getByRole("button", { name: /玉石白.*10100/ }));
  await user.click(screen.getByRole("button", { name: "PETG" }));
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
});

test("closes from the keyboard without saving", async () => {
  const user = userEvent.setup();
  const onClose = vi.fn();
  const onCreate = vi.fn();
  render(<Add open spools={[]} busy={false}
    onClose={onClose} onCreate={onCreate} />);
  await user.keyboard("{Escape}");
  expect(onClose).toHaveBeenCalledOnce();
  expect(onCreate).not.toHaveBeenCalled();
});
```

Install `@testing-library/user-event` as a dev dependency if it is not already
present:

```bash
npm install --save-dev @testing-library/user-event
```

- [ ] **Step 2: Run and verify RED**

Run:

```bash
npm test -- --run src/features/spools/Add.test.tsx
```

Expected: FAIL because `Add.tsx` does not exist.

- [ ] **Step 3: Implement dialog state and selection**

The dialog keeps these states:

```ts
const [group, setGroup] = useState<string | null>(null);
const [series, setSeries] = useState<string | null>(null);
const [selected, setSelected] = useState<FilamentColor | null>(null);
const [query, setQuery] = useState("");
const [name, setName] = useState("");
const [grams, setGrams] = useState("1000");
```

Material change sets `series`, `selected`, and `query` to null/empty. Series
change clears `selected` and `query`. The dialog renders material cards, then
series cards, then the searchable color grid. Every color button's accessible
name includes localized name and color code.

Use `<Swatch colors={entry.colors} />` for every tile and the selected summary.
The container uses `role="dialog"`, `aria-modal="true"`, and a labelled heading.
Opening moves focus to the close button; Escape calls `onClose`; closing never
submits a partial selection.

- [ ] **Step 4: Implement exact roll creation and duplicate naming**

Default name:

```ts
const base = `${selected.names[locale]} · ${selected.materialGroup} ${selected.series}`;
const duplicates = spools.filter((spool) =>
  spool.catalog_id === selected.id && spool.status !== "archived").length;
const display_name = name.trim() || (duplicates ? `${base} #${duplicates + 1}` : base);
```

Submission constructs:

```ts
const draft: NewSpool = {
  display_name,
  preset_id: selected.presetBase,
  preset_base: selected.presetBase,
  catalog_id: selected.id,
  brand: selected.brand,
  material: selected.material,
  series: selected.series,
  color_name: selected.names[locale],
  color_code: selected.colorCode,
  color_hex: selected.colors[0],
  color_hexes: selected.colors,
  remaining_grams: Number(grams),
};
```

The save button is disabled until the selection is complete and grams is a
finite positive number. A successful create closes the dialog, clears color,
query, custom name, and grams back to 1000, but retains material and series.
A failed create leaves every field open.

- [ ] **Step 5: Add localized dialog copy**

Add exact keys under `spools` for:

```text
chooseMaterial, chooseSeries, chooseColor, searchColors, colorCode,
classic, selectedColor, customName, customNameHint, noColors
```

Use natural Simplified Chinese, Traditional Chinese, and English copy. Catalog
names come from `bambu.json`, not locale JSON.

- [ ] **Step 6: Run dialog and localization tests**

Run:

```bash
npm test -- --run src/features/spools/Add.test.tsx src/i18n/i18n.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit the dialog**

```bash
git add src/features/spools/Add.tsx src/features/spools/Add.test.tsx \
  src/i18n package.json package-lock.json
git commit -m "feat: add official filament picker"
```

---

### Task 8: Integrate the dialog into the spool library

**Files:**
- Modify: `src/features/spools/Spools.tsx`
- Modify: `src/features/spools/Spools.test.tsx`
- Modify: `src/styles.css`

**Interfaces:**
- Consumes: `Add` from Task 7 and `Swatch` from Task 2.
- Produces: the finished spool-library add flow and catalog-aware row display.

- [ ] **Step 1: Write failing integration tests**

```tsx
test("opens the catalog dialog instead of the inline color form", async () => {
  const user = userEvent.setup();
  const { container } = renderSpools();
  await user.click(screen.getByRole("button", { name: "添加耗材" }));
  expect(screen.getByRole("dialog", { name: "添加耗材" })).toBeVisible();
  expect(container.querySelector(".inline-form")).toBeNull();
  expect(container.querySelector('input[type="color"]')).toBeNull();
});

test("creates identical official colors as separate rolls", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn().mockResolvedValue(true);
  renderSpools({ spools: [officialWhite], onCreate });
  await openAndChooseJadeWhite(user);
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(onCreate).toHaveBeenCalledWith(expect.objectContaining({
    display_name: "玉石白 · PLA Basic #2",
    catalog_id: "bambu:GFA00:10100",
  }));
});
```

- [ ] **Step 2: Run and verify RED**

Run:

```bash
npm test -- --run src/features/spools/Spools.test.tsx
```

Expected: FAIL because the inline form is still present.

- [ ] **Step 3: Replace the inline form and update list metadata**

Remove `draft` and the entire `.inline-form`. Render `Add` once and keep it
mounted with `open={creating}`.

Use:

```tsx
<Swatch colors={spool.color_hexes?.length
  ? spool.color_hexes
  : [spool.color_hex]} />
```

Rows show `color_name` and `color_code` when present. Search haystacks include
both fields. Resolve the current-locale display name with
`colorById(spool.catalog_id)?.names[locale] ?? spool.color_name`, so switching
language updates official names without losing the persisted fallback. Legacy
rows retain their existing display.

- [ ] **Step 4: Style the responsive catalog dialog**

Add focused classes:

```text
.modal-backdrop, .catalog-dialog, .catalog-head, .catalog-steps,
.catalog-section, .catalog-cards, .catalog-card, .color-grid,
.color-card, .color-selected, .catalog-summary, .catalog-fields
```

Desktop uses a large centered dialog capped below the menu/title area. Color
cards remain soft, rounded, and dopamine-colored without hard industrial
panels. At widths below 760 px, sections stack and the color grid becomes two
columns. The footer remains visible while the catalog body scrolls.

Respect both light and dark theme variables and `prefers-reduced-motion`.

- [ ] **Step 5: Run spool tests and build**

Run:

```bash
npm test -- --run src/features/spools/Spools.test.tsx \
  src/features/spools/Add.test.tsx
npm run build
```

Expected: PASS.

- [ ] **Step 6: Commit integration**

```bash
git add src/features/spools/Spools.tsx \
  src/features/spools/Spools.test.tsx src/styles.css
git commit -m "feat: integrate filament catalog dialog"
```

---

### Task 9: Use full spool colors across inventory surfaces

**Files:**
- Modify: `src/features/home/Home.tsx`
- Modify: `src/features/home/Home.test.tsx`
- Modify: `src/features/jobs/Job.tsx`
- Modify: `src/features/jobs/Job.test.tsx`
- Modify: `src/features/spools/Spools.tsx`

**Interfaces:**
- Consumes: `Spool.color_hexes` and shared `Swatch`.
- Produces: consistent multi-color display on home slots, job candidates, and
  spool rows.

- [ ] **Step 1: Write failing home and job swatch tests**

Create a spool fixture with:

```ts
color_hex: "#8EC9E9",
color_hexes: ["#8EC9E9", "#E7C1D5"],
```

Assert that the rendered swatch style on the home slot and job candidate
contains both colors in a linear gradient.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
npm test -- --run src/features/home/Home.test.tsx \
  src/features/jobs/Job.test.tsx
```

Expected: FAIL because both components still render only `color_hex`.

- [ ] **Step 3: Replace local color circles with `Swatch`**

Use `Swatch` for every persisted spool. Keep a one-color `Swatch` for imported
3MF filament legends because the slicer exposes only one `filament_colour` per
tool.

- [ ] **Step 4: Run affected UI tests**

Run:

```bash
npm test -- --run src/components/Swatch.test.tsx \
  src/features/home/Home.test.tsx src/features/jobs/Job.test.tsx \
  src/features/spools/Spools.test.tsx
```

Expected: PASS.

- [ ] **Step 5: Commit shared presentation**

```bash
git add src/features/home src/features/jobs src/features/spools/Spools.tsx
git commit -m "feat: show complete filament colors"
```

---

### Task 10: Complete regression, source, and real-file verification

**Files:**
- Modify: `docs/design.md`
- Create: `docs/qa-catalog.md`

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified application, updated architecture documentation, and a
  recorded real `.gcode.3mf` regression result.

- [ ] **Step 1: Update architecture and QA documentation**

Document:

- the offline `bambu.json` snapshot and `scripts/catalog.mjs` update command;
- 45 source types and 306 color entries;
- the five nullable spool columns and backup schema version 2;
- exact/base/legacy matching order;
- the material → series → color add flow;
- the fact that black-hole behavior and appearance were not changed.

Record catalog verification in `docs/qa-catalog.md`; do not mix these results
into the existing black-hole QA record.

- [ ] **Step 2: Run the complete frontend suite**

Run:

```bash
npm test -- --run
npm run build
```

Expected: every Vitest test passes and TypeScript/Vite build exits zero.

- [ ] **Step 3: Run the complete native suite**

Run:

```bash
cd src-tauri
cargo fmt -- --check
cargo test
```

Expected: every non-environment-dependent Rust test passes; the existing
ignored real-file/hardware test remains ignored.

- [ ] **Step 4: Run the user's sliced 3MF smoke test**

Run:

```bash
cd src-tauri
BAMBU_SMOKE_3MF='/Users/robin/Desktop/叠色/萨莫面具-布莱克.gcode.3mf' \
  cargo test smoke_real_sliced_file_from_environment -- --ignored --nocapture
```

Expected: the four tools still parse and settle, the source file hash and length
remain unchanged, and no catalog migration changes consumption calculations.

- [ ] **Step 5: Verify catalog source determinism**

Regenerate into the tracked path and require a clean diff:

```bash
node scripts/catalog.mjs \
  '/Applications/BambuStudio.app/Contents/Resources/profiles/BBL/filament/filaments_color_codes.json' \
  '/Applications/BambuStudio.app/Contents/Resources/profiles/BBL/filament'
git diff --exit-code -- src/catalog/bambu.json
```

Expected: exit code 0.

- [ ] **Step 6: Check formatting and workspace scope**

Run:

```bash
git diff --check
git status --short
```

Expected: no whitespace errors and only the intended feature/doc files are
modified.

- [ ] **Step 7: Commit documentation**

```bash
git add docs/design.md docs/qa-catalog.md
git commit -m "docs: record filament catalog verification"
```
