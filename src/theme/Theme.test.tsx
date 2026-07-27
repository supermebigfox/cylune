import React from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Theme, THEME_KEY, useTheme } from "./Theme";

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
  Object.defineProperty(window, "matchMedia", {
    configurable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches,
      media: query,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      addListener: vi.fn(),
      removeListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
}

describe("Theme", () => {
  beforeEach(() => {
    localStorage.clear();
    delete document.documentElement.dataset.theme;
    installMatchMedia(false);
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

});
