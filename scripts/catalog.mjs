import { readFile, readdir, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { Converter } from "opencc-js";

const [colorFile, profileDirectory] = process.argv.slice(2);

if (!colorFile || !profileDirectory) {
  throw new Error("Usage: node scripts/catalog.mjs <colors.json> <BBL-filament-directory>");
}

const toTraditional = Converter({ from: "cn", to: "tw" });
const profiles = new Map();

for (const file of await readdir(profileDirectory)) {
  if (!file.endsWith(".json")) continue;
  const path = join(profileDirectory, file);
  try {
    profiles.set(basename(file, ".json"), JSON.parse(await readFile(path, "utf8")));
  } catch {
    // The catalog only relies on valid Bambu profile JSON files.
  }
}

function profileKey(value) {
  return value
    .replace(/^Bambu\s+/i, "")
    .replace(/\s+@base$/i, "")
    .replace(/[^a-z0-9]/gi, "")
    .toLowerCase();
}

function baseProfileFor(sourceType) {
  const expected = profileKey(sourceType);
  for (const profile of profiles.values()) {
    if (typeof profile.name === "string" && /\s@base$/i.test(profile.name) && profileKey(profile.name) === expected) {
      return profile;
    }
  }
  throw new Error(`No Bambu base profile found for ${sourceType}`);
}

function filamentTypeFor(profile, visited = new Set()) {
  if (Array.isArray(profile.filament_type) && profile.filament_type[0]) {
    return String(profile.filament_type[0]);
  }
  if (typeof profile.filament_type === "string" && profile.filament_type) {
    return profile.filament_type;
  }
  if (!profile.inherits || visited.has(profile.inherits)) {
    throw new Error(`Could not resolve filament_type for ${profile.name ?? "profile"}`);
  }
  visited.add(profile.inherits);
  const parent = profiles.get(profile.inherits);
  if (!parent) throw new Error(`Missing inherited profile ${profile.inherits}`);
  return filamentTypeFor(parent, visited);
}

function materialGroupFor(sourceType, material) {
  if (sourceType === "PVA" || sourceType.startsWith("Support ")) return "支撑材料";
  if (sourceType.startsWith("PLA")) return "PLA";
  if (sourceType.startsWith("PETG")) return "PETG";
  if (sourceType.startsWith("ABS")) return "ABS";
  if (sourceType.startsWith("ASA")) return "ASA";
  if (sourceType.startsWith("TPU")) return "TPU";
  if (sourceType.startsWith("PC")) return "PC";
  if (sourceType.startsWith("PA")) return "PA";
  if (sourceType.startsWith("PET")) return "PET";
  if (sourceType.startsWith("PPA")) return "PPA";
  if (sourceType.startsWith("PPS")) return "PPS";
  return material.split("-")[0];
}

function colorTypeFor(type, colors) {
  if (type === "单色") return "solid";
  if (type === "渐变色") return "gradient";
  return colors.length === 2 ? "dual" : "multi";
}

function seriesFor(sourceType, colorType) {
  if (sourceType === "PLA Basic") return colorType === "gradient" ? "Basic Gradient" : "Basic";
  if (sourceType === "PLA Silk") return colorType === "solid" ? "Silk" : "Silk Multi-Color";
  if (sourceType.startsWith("Support ") || sourceType === "PVA") return sourceType;
  const group = materialGroupFor(sourceType, sourceType);
  return sourceType.startsWith(`${group} `) ? sourceType.slice(group.length + 1) : sourceType;
}

function sourceVersionFor(directory) {
  const infoPath = resolve(directory, "../../../..", "Info.plist");
  return readFile(infoPath, "utf8").then((contents) => {
    const match = contents.match(/<key>CFBundleShortVersionString<\/key>\s*<string>([^<]+)<\/string>/);
    return match?.[1] ?? "unknown";
  }).catch(() => "unknown");
}

const source = JSON.parse(await readFile(colorFile, "utf8"));
const entries = source.data.map((item) => {
  const profile = baseProfileFor(item.fila_type);
  const colors = item.fila_color.map((color) => color.slice(0, 7).toUpperCase());
  const colorType = colorTypeFor(item.fila_color_type, colors);
  const material = filamentTypeFor(profile);

  return {
    id: `bambu:${item.fila_id}:${item.fila_color_code}`,
    brand: "Bambu Lab",
    filamentId: item.fila_id,
    sourceType: item.fila_type,
    materialGroup: materialGroupFor(item.fila_type, material),
    material,
    series: seriesFor(item.fila_type, colorType),
    presetBase: profile.name,
    colorCode: item.fila_color_code,
    colorType,
    colors,
    names: {
      "zh-CN": item.fila_color_name.zh,
      "zh-TW": toTraditional(item.fila_color_name.zh),
      en: item.fila_color_name.en,
    },
    classic: (item.fila_type === "PLA Silk" || item.fila_type === "PLA Tough") && colorType === "solid",
  };
}).sort((left, right) =>
  left.sourceType === right.sourceType
    ? (left.colorCode > right.colorCode) - (left.colorCode < right.colorCode)
    : (left.sourceType > right.sourceType) - (left.sourceType < right.sourceType));

const snapshot = {
  sourceVersion: await sourceVersionFor(profileDirectory),
  entries,
};

await writeFile(resolve("src/catalog/bambu.json"), `${JSON.stringify(snapshot, null, 2)}\n`);
