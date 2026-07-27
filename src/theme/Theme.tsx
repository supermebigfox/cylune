import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
} from "react";

export type ThemeMode = "light" | "dark";
export type ThemeContextValue = {
  theme: ThemeMode;
  setTheme(theme: ThemeMode): void;
  toggleTheme(): void;
};

export const THEME_KEY = "bambu-spools.theme";

const ThemeContext = createContext<ThemeContextValue | null>(null);
const useBrowserLayoutEffect =
  typeof window === "undefined" ? useEffect : useLayoutEffect;

type ThemeWindow = Pick<Window, "matchMedia">;
type ThemeDocument = Pick<Document, "documentElement">;

function browserWindow(): ThemeWindow | null {
  try {
    return typeof window === "undefined" ? null : window;
  } catch {
    return null;
  }
}

function browserDocument(): ThemeDocument | null {
  try {
    return typeof document === "undefined" ? null : document;
  } catch {
    return null;
  }
}

function storedTheme(): ThemeMode | null {
  try {
    if (typeof localStorage === "undefined") return null;
    const value = localStorage.getItem(THEME_KEY);
    return value === "light" || value === "dark" ? value : null;
  } catch {
    return null;
  }
}

function persistTheme(theme: ThemeMode): void {
  try {
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(THEME_KEY, theme);
    }
  } catch {
    // Manual theme changes remain available when persistence is denied.
  }
}

function darkMedia(host = browserWindow()): MediaQueryList | null {
  try {
    return host && typeof host.matchMedia === "function"
      ? host.matchMedia("(prefers-color-scheme: dark)")
      : null;
  } catch {
    return null;
  }
}

export function getSystemTheme(host: ThemeWindow | null = browserWindow()): ThemeMode {
  return darkMedia(host)?.matches ? "dark" : "light";
}

export function syncThemeDocument(
  theme: ThemeMode,
  target: ThemeDocument | null = browserDocument(),
): void {
  try {
    if (!target) return;
    target.documentElement.dataset.theme = theme;
    target.documentElement.style.colorScheme = theme;
  } catch {
    // A missing or restricted document must not break theme state.
  }
}

export function Theme({ children }: { children: ReactNode }) {
  const [{ manual, theme }, setState] = useState(() => {
    const stored = storedTheme();
    return {
      manual: stored !== null,
      theme: stored ?? getSystemTheme(),
    };
  });

  useBrowserLayoutEffect(() => {
    syncThemeDocument(theme);
  }, [theme]);

  useEffect(() => {
    if (manual) return;
    const media = darkMedia();
    if (!media) return;
    const followSystem = (event: MediaQueryListEvent) => {
      setState({ manual: false, theme: event.matches ? "dark" : "light" });
    };
    if (typeof media.addEventListener === "function") {
      media.addEventListener("change", followSystem);
      return () => media.removeEventListener("change", followSystem);
    }
    media.addListener(followSystem);
    return () => media.removeListener(followSystem);
  }, [manual]);

  useEffect(() => {
    const host = typeof window === "undefined" ? null : window;
    if (!host) return;
    const syncStoredTheme = (event: StorageEvent) => {
      if (event.key !== THEME_KEY) return;
      if (event.newValue === "light" || event.newValue === "dark") {
        setState({ manual: true, theme: event.newValue });
      }
    };
    host.addEventListener("storage", syncStoredTheme);
    return () => host.removeEventListener("storage", syncStoredTheme);
  }, []);

  const setTheme = useCallback((next: ThemeMode) => {
    persistTheme(next);
    setState({ manual: true, theme: next });
  }, []);

  const toggleTheme = useCallback(() => {
    setState((current) => {
      const next = current.theme === "light" ? "dark" : "light";
      persistTheme(next);
      return { manual: true, theme: next };
    });
  }, []);

  const value = useMemo(
    () => ({ theme, setTheme, toggleTheme }),
    [setTheme, theme, toggleTheme],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const value = useContext(ThemeContext);
  if (!value) throw new Error("useTheme must be used within Theme");
  return value;
}
