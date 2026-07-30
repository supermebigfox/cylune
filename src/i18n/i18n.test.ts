import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";
import {
  LOCALE_KEY,
  applyStoredLocale,
  syncDocumentLocale,
  detectLocale,
  getLocale,
  setLocale,
  supportedLocales,
} from ".";

const stableErrorCodes = [
  "archived_spool",
  "bambu_studio_missing",
  "database",
  "duplicate_job",
  "file_not_stable",
  "insufficient_filament",
  "invalid_file",
  "invalid_job",
  "invalid_mapping",
  "invalid_slot",
  "io",
  "output_exists",
  "slicer_cancelled",
  "slicer_failed",
  "slicer_incompatible",
  "slicer_profiles_missing",
  "slot_conflict",
  "standalone_gcode_profiles_required",
  "unknown_gcode",
  "unsliced_project",
];

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

  afterEach(() => {
    vi.restoreAllMocks();
    syncDocumentLocale(getLocale());
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

  it("matches every stable Rust error code exactly", () => {
    for (const locale of [zhCN, zhTW, en]) {
      expect(Object.keys(locale.errors).sort()).toEqual(stableErrorCodes);
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
    expect(document.documentElement.lang).toBe("zh-TW");
  });

  it("falls back to language detection when storage access is denied", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });

    expect(detectLocale(["en-US"])).toBe("en");
  });

  it("updates runtime locale when persistence is denied", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });

    await expect(setLocale("en")).resolves.toBeUndefined();
    expect(getLocale()).toBe("en");
    expect(document.documentElement.lang).toBe("en");
  });

  it("safely skips document updates when no document is available", () => {
    expect(() => syncDocumentLocale("en", null)).not.toThrow();
  });

  it("applies the detected locale to the document during initialization", async () => {
    const descriptor = Object.getOwnPropertyDescriptor(navigator, "languages");
    Object.defineProperty(navigator, "languages", {
      configurable: true,
      value: ["zh-Hant-TW"],
    });
    localStorage.clear();
    document.documentElement.lang = "";
    vi.resetModules();

    const fresh = await import("./index");

    expect(fresh.getLocale()).toBe("zh-TW");
    expect(document.documentElement.lang).toBe("zh-TW");

    if (descriptor) Object.defineProperty(navigator, "languages", descriptor);
  });

  it("applies locale changes received from another WebView", async () => {
    await setLocale("zh-CN");

    expect(applyStoredLocale("en")).toBe(true);
    expect(getLocale()).toBe("en");
    expect(document.documentElement.lang).toBe("en");
    expect(applyStoredLocale("unsupported")).toBe(false);
  });
});
