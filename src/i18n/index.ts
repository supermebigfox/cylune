import { useSyncExternalStore } from "react";
import { invoke } from "@tauri-apps/api/core";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";

export type SupportedLocale = "zh-CN" | "zh-TW" | "en";

export const supportedLocales: SupportedLocale[] = ["zh-CN", "zh-TW", "en"];
export const LOCALE_KEY = "bambu-spools.locale";

const resources = { "zh-CN": zhCN, "zh-TW": zhTW, en } as const;
const listeners = new Set<() => void>();

type LocaleDocument = Pick<Document, "documentElement">;

function browserDocument(): LocaleDocument | null {
  try {
    return typeof document === "undefined" ? null : document;
  } catch {
    return null;
  }
}

function browserLanguages(): readonly string[] {
  try {
    if (typeof navigator === "undefined") return [];
    if (navigator.languages?.length) return navigator.languages;
    return navigator.language ? [navigator.language] : [];
  } catch {
    return [];
  }
}

function persistedLocale(): SupportedLocale | null {
  try {
    if (typeof localStorage === "undefined") return null;
    const value = localStorage.getItem(LOCALE_KEY);
    return supportedLocales.includes(value as SupportedLocale)
      ? (value as SupportedLocale)
      : null;
  } catch {
    return null;
  }
}

function persistLocale(locale: SupportedLocale): void {
  try {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(LOCALE_KEY, locale);
    }
  } catch {
    // Runtime language changes remain available when persistence is denied.
  }
}

function matchLanguage(tag: string): SupportedLocale | null {
  const normalized = tag.trim().toLowerCase().replace(/_/g, "-");
  if (normalized === "zh-cn") return "zh-CN";
  if (normalized === "zh-tw") return "zh-TW";
  if (normalized === "en") return "en";
  if (normalized.startsWith("en-")) return "en";
  if (/^zh-(hant|tw|hk|mo)(-|$)/.test(normalized)) return "zh-TW";
  if (normalized === "zh" || normalized.startsWith("zh-")) return "zh-CN";
  return null;
}

export function detectLocale(
  languages: readonly string[] = browserLanguages(),
): SupportedLocale {
  const persisted = persistedLocale();
  if (persisted) return persisted;

  for (const language of languages) {
    const match = matchLanguage(language);
    if (match) return match;
  }
  return "zh-CN";
}

let currentLocale = detectLocale();

export function syncDocumentLocale(
  locale: SupportedLocale,
  target: LocaleDocument | null = browserDocument(),
): void {
  try {
    if (target) target.documentElement.lang = locale;
  } catch {
    // A missing or restricted document must not break local state.
  }
}

syncDocumentLocale(currentLocale);

function notifyLocale(locale: SupportedLocale): void {
  currentLocale = locale;
  syncDocumentLocale(locale);
  listeners.forEach((listener) => listener());
}

export function applyStoredLocale(value: string | null): boolean {
  if (!supportedLocales.includes(value as SupportedLocale)) return false;
  const locale = value as SupportedLocale;
  if (locale !== currentLocale) notifyLocale(locale);
  return true;
}

try {
  if (typeof window !== "undefined") {
    window.addEventListener("storage", (event) => {
      if (event.key === LOCALE_KEY) applyStoredLocale(event.newValue);
    });
  }
} catch {
  // Cross-window synchronization is optional in non-browser contexts.
}

function syncNativeLocale(locale: SupportedLocale): void {
  if ("__TAURI_INTERNALS__" in globalThis) {
    void invoke("set_native_locale", { locale }).catch(() => undefined);
  }
}

syncNativeLocale(currentLocale);

export function getLocale(): SupportedLocale {
  return currentLocale;
}

export async function setLocale(locale: SupportedLocale): Promise<void> {
  if (!supportedLocales.includes(locale)) {
    throw new TypeError(`Unsupported locale: ${String(locale)}`);
  }
  persistLocale(locale);
  notifyLocale(locale);
  syncNativeLocale(locale);
}

export function useLocale(): SupportedLocale {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    getLocale,
    () => "zh-CN",
  );
}

export function t(
  key: string,
  values: Record<string, string | number> = {},
  locale = currentLocale,
): string {
  const raw = key.split(".").reduce<unknown>((value, part) => {
    if (!value || typeof value !== "object") return undefined;
    return (value as Record<string, unknown>)[part];
  }, resources[locale]);

  if (typeof raw !== "string") return key;
  return Object.entries(values).reduce(
    (message, [name, value]) =>
      message.split(`{{${name}}}`).join(String(value)),
    raw,
  );
}
