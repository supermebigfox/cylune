import {
  bambuColors, colorCountForMaterial, colorsFor, materialGroups, searchColors,
  seriesFor, seriesIsClassic,
} from "./bambu";

test("contains the complete Bambu snapshot", () => {
  expect(new Set(bambuColors.map((item) => item.sourceType)).size).toBe(45);
  expect(bambuColors).toHaveLength(306);
  expect(new Set(bambuColors.map((item) => item.id)).size).toBe(306);
});

test("keeps every snapshot preset base machine-independent", () => {
  for (const item of bambuColors) {
    expect(item.presetBase).not.toMatch(/\s@/);
  }
  expect(
    bambuColors.find((item) => item.id === "bambu:GFA00:10100")?.presetBase,
  ).toBe("Bambu PLA Basic");
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

test("counts every official color entry in a material group", () => {
  expect(colorCountForMaterial("PLA")).toBe(178);
  expect(colorCountForMaterial("PETG")).toBe(42);
});

test("puts current PLA series before classic series without reordering either group", () => {
  expect(seriesFor("PLA")).toEqual([
    "Aero",
    "Basic",
    "Basic Gradient",
    "Dynamic",
    "Galaxy",
    "Glow",
    "Lite",
    "Marble",
    "Matte",
    "Metal",
    "Pure",
    "Silk Multi-Color",
    "Silk+",
    "Sparkle",
    "Tough+",
    "Translucent",
    "Wood",
    "PLA-CF",
    "Silk",
    "Tough",
  ]);
});

test("classifies only non-empty all-classic series as classic", () => {
  expect(seriesIsClassic("PLA", "Silk")).toBe(true);
  expect(seriesIsClassic("PLA", "Silk+")).toBe(false);
  expect(seriesIsClassic("PLA", "Unknown")).toBe(false);
});

test("searches official Chinese name and five-digit code", () => {
  expect(searchColors(bambuColors, "玉石白", "zh-CN")[0].colorCode).toBe("10100");
  expect(searchColors(bambuColors, "10100", "zh-CN")[0].names["zh-CN"]).toBe("玉石白");
});
