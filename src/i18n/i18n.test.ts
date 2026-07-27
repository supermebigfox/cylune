import { beforeEach, describe, expect, it } from "vitest";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";
import {
  LOCALE_KEY,
  detectLocale,
  getLocale,
  setLocale,
  supportedLocales,
} from ".";

function flattenKeys(value: unknown, prefix = ""): string[] {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return [prefix];
  }

  return Object.entries(value)
    .flatMap(([key, child]) =>
      flattenKeys(child, prefix ? `${prefix}.${key}` : key),
    )
    .sort();
}

describe("localization", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("keeps all locale key sets identical", () => {
    expect(flattenKeys(zhTW)).toEqual(flattenKeys(zhCN));
    expect(flattenKeys(en)).toEqual(flattenKeys(zhCN));
  });

  it("covers the supported product surfaces in every locale", () => {
    const requiredGroups = [
      "nav",
      "slots",
      "spools",
      "jobs",
      "import",
      "settlement",
      "settings",
      "notifications",
      "errors",
      "tray",
    ];

    for (const locale of [zhCN, zhTW, en]) {
      for (const group of requiredGroups) {
        expect(locale).toHaveProperty(group);
      }
    }
  });

  it("detects exact and primary language matches in preference order", () => {
    expect(detectLocale(["fr-FR", "zh-TW", "en-US"])).toBe("zh-TW");
    expect(detectLocale(["en-GB", "zh-CN"])).toBe("en");
    expect(detectLocale(["zh-Hant-HK"])).toBe("zh-TW");
    expect(detectLocale(["zh-SG"])).toBe("zh-CN");
    expect(detectLocale(["de-DE"])).toBe("zh-CN");
  });

  it("prefers a valid persisted locale over system languages", () => {
    localStorage.setItem(LOCALE_KEY, "en");

    expect(detectLocale(["zh-CN"])).toBe("en");
  });

  it("persists and applies locale changes at runtime", async () => {
    await setLocale("zh-TW");

    expect(localStorage.getItem(LOCALE_KEY)).toBe("zh-TW");
    expect(getLocale()).toBe("zh-TW");
    expect(supportedLocales).toEqual(["zh-CN", "zh-TW", "en"]);
  });
});
