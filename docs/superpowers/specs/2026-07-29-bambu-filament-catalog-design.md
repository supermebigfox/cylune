# Bambu Filament Catalog Design

**Status:** Approved for implementation  
**Date:** 2026-07-29

## Goal

Replace the current free-form “添加耗材” form and native color picker with an
offline Bambu Lab catalog picker:

```text
材质大类 → 官方系列 → 官方颜色 → 当前卷信息
```

The first release covers Bambu Lab original filament only. It must:

- expose every official filament type and color in the catalog snapshot;
- keep temporarily out-of-stock colors selectable;
- show official Chinese color names in the Chinese interface;
- represent solid, gradient, dual-color, and multi-color filaments accurately;
- preserve separate inventory records for multiple rolls with identical
  material and color;
- remain compatible with automatic `.gcode.3mf` matching and settlement.

## Source of Truth

The catalog snapshot is built from two official Bambu Lab sources:

1. `filaments_color_codes.json` bundled with Bambu Studio 2.8.0.50. It contains
   official filament identifiers, Chinese and English color names, five-digit
   color codes, color types, and one or more RGBA values.
2. The Bambu Lab filament store collection, used to distinguish current retail
   product series from classic or superseded series.

The initial normalized snapshot contains:

- 45 official filament types;
- 306 official solid, gradient, dual-color, or multi-color entries.

The data is checked into the application and works offline. The user does not
need Bambu Studio installed at runtime. The snapshot carries a catalog version
and source version so it can be updated deliberately in later releases.

Temporary store availability never removes an entry. Classic official entries,
such as the original PLA Silk and PLA Tough, remain selectable and receive a
“经典款” badge instead of being silently omitted.

Bundles and hardware are not separate filament series:

- the reusable spool is excluded;
- starter packs and CMYK bundles contribute their underlying filament colors
  but are not presented as new material types.

Support filaments and engineering filaments are included because they are
consumables, even when they are unsuitable for the user's current A1 printer.

## Catalog Model

Each normalized catalog color has the following shape:

```ts
interface FilamentCatalogColor {
  id: string;
  manufacturer: "Bambu Lab";
  filamentId: string;
  materialGroup: string;
  material: string;
  series: string;
  presetBase: string;
  colorCode: string;
  colorType: "solid" | "gradient" | "dual" | "multi";
  colors: string[];
  names: {
    "zh-CN": string;
    "zh-TW": string;
    en: string;
  };
  classic?: boolean;
}
```

The stable `id` combines the manufacturer filament identifier and official
color code. It does not depend on translated text.

`materialGroup` is the first-level browsing category, for example PLA, PETG,
ABS, ASA, TPU, PC, PA, PET, PPA, PPS, or 支撑材料. `material` remains the exact
material value used by Bambu Studio, such as `PLA-CF`, so imported print
profiles can still be matched correctly.

The retail taxonomy is normalized for browsing:

- PLA Basic solid colors appear under `Basic`.
- PLA Basic gradient colors appear under `Basic Gradient`.
- current PLA Silk+ colors appear under `Silk+`.
- non-solid original PLA Silk colors appear under `Silk Multi-Color`.
- superseded original PLA Silk and PLA Tough entries retain their original
  series and are marked as classic.

The same principle is applied to other store series whose retail grouping is
more specific than the raw Bambu Studio filament type.

## Persisted Spool Metadata

The spool record gains nullable catalog metadata:

```text
catalog_id
color_name
color_code
color_hexes
preset_base
```

- `catalog_id` identifies the selected official entry.
- `color_name` stores the localized official name selected at creation time.
- `color_code` stores the official five-digit Bambu color code.
- `color_hexes` stores the complete ordered color list as JSON.
- `preset_base` stores the machine-independent Bambu preset name.

The existing `color_hex` field remains and stores the first/primary color. This
preserves compatibility with existing UI, backups, and 3MF matching while the
new `color_hexes` field provides accurate multi-color presentation.

Existing spool records remain valid. Their new fields are nullable and the UI
falls back to the existing brand, material, series, and `color_hex` values.

Backup export and restore include all new fields. Older backups without these
fields continue to import.

## Add-Filament Interaction

The current always-visible inline form is replaced with an “添加耗材” button
that opens a large, keyboard-accessible dialog.

### 1. Choose material

The first section shows material-group cards with concise names and roll counts.
Choosing a group updates the available series immediately.

### 2. Choose series

The second section shows only series belonging to the chosen material group,
such as Basic, Matte, Silk+, Pure, Tough+, CF, Galaxy, or Glow. Classic series
show a quiet “经典款” badge. No series is hidden because of store stock state.

Changing the material clears the previous series and color selection. Changing
the series clears only the color selection.

### 3. Choose color

The color area is a searchable grid. Every tile shows:

- the visual swatch;
- the localized color name;
- the official five-digit color code.

Solid colors use a single circular swatch. Gradient, dual-color, and multi-color
entries render all official colors in order rather than collapsing them into a
single average color.

Search matches localized name, English name, official color code, and hex
value. Color selection is by official entry, not by the operating system color
picker.

### 4. Complete roll details

After selecting a color, the dialog shows a compact summary and asks for:

- current remaining weight, defaulting to 1000 g;
- an optional custom roll name.

If the name is blank, the app generates one from the series and localized color
name. If an active roll with the same catalog entry already exists, the next
roll receives a short numeric suffix. The user can still give every roll a
custom name.

Submitting creates exactly one spool record with a unique spool ID. Identical
official colors never share remaining weight or settlement history.

The dialog retains the selected material and series after a successful add so a
user entering several related rolls can continue quickly, while the color and
roll-specific fields reset.

## Localization

- Simplified Chinese uses Bambu Studio's official `zh` color name.
- Traditional Chinese uses a reviewed Traditional Chinese catalog value.
- English uses Bambu Studio's official English color name.
- Material and series technical names such as PLA, PETG, Basic, Matte, and
  Silk+ remain recognizable across locales.
- The current Chinese interface never falls back to an exposed English-only
  color name when an official Chinese name exists.

## 3MF Matching

Machine suffixes vary between Bambu printer profiles, so catalog matching must
not depend solely on an exact string such as `Bambu PLA Basic @BBL A1`.

Candidate matching uses this order:

1. exact preset ID, exact material/series, and primary color;
2. normalized `preset_base`, exact material, and primary color;
3. existing legacy material/series/color fallback for records created before
   the catalog migration.

The matcher strips the machine suffix after `@` to obtain the preset base. This
allows an official catalog roll to match an imported A1 profile without making
the catalog itself A1-only.

Multi-color filaments retain their full display colors, but 3MF matching uses
the primary color stored by the slicer because 3MF filament metadata currently
provides one `filament_colour` value per tool.

Automatic matching never merges inventory records. When several compatible
rolls exist, they remain separate candidate choices and the existing mapping
flow chooses one roll.

## Availability and Compatibility

Store stock is intentionally not fetched at selection time:

- temporary sold-out colors remain present;
- the add flow works offline;
- a store outage cannot remove a user's material choice.

Printer and AMS compatibility may be displayed as informational badges in a
later release, but incompatibility does not hide catalog entries in this
release.

## Error Handling

- Invalid catalog rows are rejected during catalog validation, not at user
  selection time.
- The confirm button stays disabled until a material, series, color, and valid
  positive remaining weight are present.
- A failed native create command keeps the dialog and user selections open and
  shows the existing error treatment.
- Unknown legacy spools continue to display with their existing single color.
- Missing localized text falls back to Simplified Chinese for Chinese locales
  and English otherwise.

## Verification

Automated coverage must include:

- catalog integrity: 45 filament types and 306 color entries;
- stable and unique catalog IDs;
- valid five-digit color codes and one or more valid hex colors per entry;
- Simplified Chinese, Traditional Chinese, and English names;
- current versus classic series classification;
- material changes reset series and color;
- series changes reset color;
- search by Chinese name and official color code;
- correct rendering for solid, gradient, dual-color, and multi-color swatches;
- no native `<input type="color">` in the add flow;
- default and custom roll naming;
- two identical catalog rolls remain separate records;
- migration of existing spool records;
- backup round-trip with and without new catalog fields;
- exact and normalized 3MF preset matching;
- existing settlement, inventory, black-hole ingestion, and packaging tests
  continue to pass.

## Out of Scope

- third-party filament catalogs;
- user-defined manufacturers, materials, series, or colors;
- live stock or price synchronization;
- direct RFID/AMS reading;
- cloud account synchronization;
- changing the approved desktop black-hole appearance or ingestion animation.
