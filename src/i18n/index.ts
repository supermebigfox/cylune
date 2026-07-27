import { useSyncExternalStore } from "react";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";

export type SupportedLocale = "zh-CN" | "zh-TW" | "en";

export const supportedLocales: SupportedLocale[] = ["zh-CN", "zh-TW", "en"];
export const LOCALE_KEY = "bambu-spools.locale";

const resources = { "zh-CN": zhCN, "zh-TW": zhTW, en } as const;
const listeners = new Set<() => void>();

function persistedLocale(): SupportedLocale | null {
  if (typeof localStorage === "undefined") return null;
  const value = localStorage.getItem(LOCALE_KEY);
  return supportedLocales.includes(value as SupportedLocale)
    ? (value as SupportedLocale)
    : null;
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
  languages: readonly string[] =
    typeof navigator === "undefined" ? [] : navigator.languages,
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

export function getLocale(): SupportedLocale {
  return currentLocale;
}

export async function setLocale(locale: SupportedLocale): Promise<void> {
  if (!supportedLocales.includes(locale)) {
    throw new TypeError(`Unsupported locale: ${String(locale)}`);
  }
  localStorage.setItem(LOCALE_KEY, locale);
  currentLocale = locale;
  document.documentElement.lang = locale;
  listeners.forEach((listener) => listener());
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
