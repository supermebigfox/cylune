import snapshot from "./bambu.json";

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

export type CatalogLocale = "zh-CN" | "zh-TW" | "en";

export const bambuColors = snapshot.entries as FilamentColor[];

const GROUP_ORDER = [
  "PLA", "PETG", "ABS", "ASA", "TPU", "PC",
  "PA", "PET", "PPA", "PPS", "支撑材料",
];

export const materialGroups = () =>
  GROUP_ORDER.filter((group) =>
    bambuColors.some((item) => item.materialGroup === group));

export const colorCountForMaterial = (group: string) =>
  bambuColors.filter((item) => item.materialGroup === group).length;

export const seriesIsClassic = (group: string, series: string) => {
  const entries = bambuColors.filter((item) =>
    item.materialGroup === group && item.series === series);
  return entries.length > 0 && entries.every((item) => item.classic);
};

export const seriesFor = (group: string) => {
  const series = [...new Set(bambuColors
    .filter((item) => item.materialGroup === group)
    .map((item) => item.series))];
  return [
    ...series.filter((item) => !seriesIsClassic(group, item)),
    ...series.filter((item) => seriesIsClassic(group, item)),
  ];
};

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
