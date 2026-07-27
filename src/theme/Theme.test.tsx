import React from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  getSystemTheme,
  syncThemeDocument,
  Theme,
  THEME_KEY,
  useTheme,
} from "./Theme";

function ThemeProbe() {
  const { theme, setTheme, toggleTheme } = useTheme();

  return (
    <div data-testid="probe" data-mode={theme}>
      <button onClick={() => setTheme("dark")}>dark</button>
      <button onClick={toggleTheme}>toggle</button>
    </div>
  );
}

function installMatchMedia(matches: boolean) {
  const media = {
    matches,
    media: "(prefers-color-scheme: dark)",
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  };
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn().mockReturnValue(media),
  });
  return media;
}

describe("Theme", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
    installMatchMedia(false);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("uses the macOS dark preference on first run", () => {
    installMatchMedia(true);

    render(
      <Theme>
        <ThemeProbe />
      </Theme>,
    );

    expect(screen.getByTestId("probe")).toHaveAttribute("data-mode", "dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem(THEME_KEY)).toBeNull();
  });

  it("prefers a persisted manual theme over the system", () => {
    installMatchMedia(true);
    localStorage.setItem(THEME_KEY, "light");

    render(
      <Theme>
        <ThemeProbe />
      </Theme>,
    );

    expect(screen.getByTestId("probe")).toHaveAttribute("data-mode", "light");
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("persists a manual theme and toggles it without remounting children", () => {
    let mounts = 0;

    function StatefulProbe() {
      const theme = useTheme();
      const [count, setCount] = React.useState(0);
      React.useEffect(() => {
        mounts += 1;
      }, []);

      return (
        <>
          <span data-testid="mode">{theme.theme}</span>
          <span data-testid="count">{count}</span>
          <button onClick={() => setCount((value) => value + 1)}>count</button>
          <button onClick={() => theme.setTheme("dark")}>dark</button>
          <button onClick={theme.toggleTheme}>toggle</button>
        </>
      );
    }

    render(
      <Theme>
        <StatefulProbe />
      </Theme>,
    );
    fireEvent.click(screen.getByRole("button", { name: "count" }));
    fireEvent.click(screen.getByRole("button", { name: "dark" }));

    expect(screen.getByTestId("mode")).toHaveTextContent("dark");
    expect(screen.getByTestId("count")).toHaveTextContent("1");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(localStorage.getItem(THEME_KEY)).toBe("dark");
    expect(mounts).toBe(1);

    fireEvent.click(screen.getByRole("button", { name: "toggle" }));
    expect(screen.getByTestId("mode")).toHaveTextContent("light");
    expect(localStorage.getItem(THEME_KEY)).toBe("light");
  });

  it("falls back to the system when storage reads are denied", () => {
    installMatchMedia(true);
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });

    render(
      <Theme>
        <ThemeProbe />
      </Theme>,
    );

    expect(screen.getByTestId("probe")).toHaveAttribute("data-mode", "dark");
  });

  it("changes theme even when storage writes are denied", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });
    render(
      <Theme>
        <ThemeProbe />
      </Theme>,
    );

    expect(() =>
      fireEvent.click(screen.getByRole("button", { name: "dark" })),
    ).not.toThrow();
    expect(screen.getByTestId("probe")).toHaveAttribute("data-mode", "dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

  it("defaults safely when matchMedia is unavailable", () => {
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: undefined,
    });

    render(
      <Theme>
        <ThemeProbe />
      </Theme>,
    );

    expect(screen.getByTestId("probe")).toHaveAttribute("data-mode", "light");
  });

  it("removes the system theme listener on unmount", () => {
    const media = installMatchMedia(false);
    const view = render(
      <Theme>
        <ThemeProbe />
      </Theme>,
    );

    view.unmount();

    expect(media.addEventListener).toHaveBeenCalledWith(
      "change",
      expect.any(Function),
    );
    expect(media.removeEventListener).toHaveBeenCalledWith(
      "change",
      expect.any(Function),
    );
  });

  it("has document and window safe theme helpers", () => {
    expect(getSystemTheme(null)).toBe("light");
    expect(() => syncThemeDocument("dark", null)).not.toThrow();
  });

  it("synchronizes a manual theme received from another WebView", async () => {
    render(
      <Theme>
        <ThemeProbe />
      </Theme>,
    );

    await act(async () => {
      window.dispatchEvent(new StorageEvent("storage", {
        key: THEME_KEY,
        newValue: "dark",
      }));
    });

    expect(screen.getByTestId("probe")).toHaveAttribute("data-mode", "dark");
    expect(document.documentElement.dataset.theme).toBe("dark");
  });

});
