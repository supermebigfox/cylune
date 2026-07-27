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

function storedTheme(): ThemeMode | null {
  const value = localStorage.getItem(THEME_KEY);
  return value === "light" || value === "dark" ? value : null;
}

function systemTheme(): ThemeMode {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function Theme({ children }: { children: ReactNode }) {
  const [manual, setManual] = useState(() => storedTheme() !== null);
  const [theme, setMode] = useState<ThemeMode>(() => storedTheme() ?? systemTheme());

  useLayoutEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.style.colorScheme = theme;
  }, [theme]);

  useEffect(() => {
    if (manual) return;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const followSystem = (event: MediaQueryListEvent) => {
      setMode(event.matches ? "dark" : "light");
    };
    media.addEventListener("change", followSystem);
    return () => media.removeEventListener("change", followSystem);
  }, [manual]);

  const setTheme = useCallback((next: ThemeMode) => {
    localStorage.setItem(THEME_KEY, next);
    setManual(true);
    setMode(next);
  }, []);

  const toggleTheme = useCallback(() => {
    setManual(true);
    setMode((current) => {
      const next = current === "light" ? "dark" : "light";
      localStorage.setItem(THEME_KEY, next);
      return next;
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
