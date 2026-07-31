import React, { useState } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MainNav, type MainNavItem } from "./Nav";

type Page = "home" | "spools" | "printers" | "slice" | "jobs" | "settings";

function Icon({ name }: { name: string }) {
  return <svg aria-hidden="true" data-icon={name} viewBox="0 0 24 24"><path d="M4 12h16" /></svg>;
}

const items: readonly MainNavItem<Page>[] = [
  { id: "home", label: "概览", icon: <Icon name="home" /> },
  { id: "spools", label: "耗材库", icon: <Icon name="spools" /> },
  { id: "printers", label: "打印机", icon: <Icon name="printers" /> },
  { id: "slice", label: "切片", icon: <Icon name="slice" />, badge: "48%" },
  { id: "jobs", label: "打印任务", icon: <Icon name="jobs" /> },
];

const settings: MainNavItem<Page> = {
  id: "settings",
  label: "设置",
  icon: <Icon name="settings" />,
};

function installViewport(narrow: boolean) {
  vi.stubGlobal("matchMedia", vi.fn((query: string) => ({
    matches: query === "(max-width: 560px)" ? narrow : false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })));
}

function Harness({ initial = "home" as Page }: { initial?: Page }) {
  const [activeId, setActiveId] = useState<Page>(initial);
  return <MainNav
    activeId={activeId}
    items={items}
    settingsItem={settings}
    onNavigate={setActiveId}
    brand={{ mark: <span aria-hidden="true">◉</span>, name: "CYLUNE", subtitle: "本地模式" }}
    importAction={{ label: "导入 3MF", icon: <Icon name="import" />, onClick: vi.fn() }}
    privacy={{ title: "本地模式", description: "文件不会离开这台 Mac。" }}
    ariaLabel="主导航"
    menuLabel="打开导航"
    closeMenuLabel="关闭导航"
  />;
}

describe("MainNav", () => {
  beforeEach(() => installViewport(false));

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("uses one shared liquid-glass indicator for every destination", () => {
    render(<Harness />);

    expect(screen.getAllByTestId("nav-active-indicator")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "概览" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "耗材库" })).not.toHaveAttribute("aria-current");
    expect(document.querySelectorAll(".cylune-nav__item.active, .cylune-nav__item--active")).toHaveLength(0);
  });

  it("keeps the requested order and places Settings alone in the footer", () => {
    render(<Harness />);

    const navigation = screen.getByRole("navigation", { name: "主导航" });
    const primary = navigation.querySelector(".cylune-nav__primary");
    const footer = navigation.querySelector(".cylune-nav__footer");
    expect(primary).not.toBeNull();
    expect(footer).not.toBeNull();
    expect(within(primary as HTMLElement).getAllByRole("button").map((button) =>
      button.querySelector(".cylune-nav__label")?.textContent,
    )).toEqual([
      "概览", "耗材库", "打印机", "切片", "打印任务",
    ]);
    expect(within(footer as HTMLElement).getByRole("button", { name: "设置" })).toBeVisible();
    expect(within((footer as HTMLElement).lastElementChild as HTMLElement).getByRole("button", { name: "设置" })).toBeVisible();
  });

  it("moves selection through its controlled callback and exposes a badge", async () => {
    const user = userEvent.setup();
    render(<Harness />);

    const slice = screen.getByRole("button", { name: "切片 48%" });
    expect(within(slice).getByText("48%")).toHaveClass("cylune-nav__badge");
    await user.click(slice);

    expect(slice).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "概览" })).not.toHaveAttribute("aria-current");
    expect(screen.getAllByTestId("nav-active-indicator")).toHaveLength(1);
  });

  it("renders accessible icon-rail tooltips without replacing button labels", () => {
    render(<Harness />);

    const printer = screen.getByRole("button", { name: "打印机" });
    const tooltipId = printer.getAttribute("aria-describedby");
    expect(tooltipId).toBeTruthy();
    expect(document.getElementById(tooltipId as string)).toHaveAttribute("role", "tooltip");
    expect(document.getElementById(tooltipId as string)).toHaveTextContent("打印机");
  });

  it("opens a real narrow drawer, closes on Escape, and restores menu focus", async () => {
    installViewport(true);
    const user = userEvent.setup();
    const focus = vi.spyOn(HTMLElement.prototype, "focus");
    render(<Harness />);

    const opener = screen.getByRole("button", { name: "打开导航" });
    expect(screen.queryByRole("dialog", { name: "主导航" })).not.toBeInTheDocument();
    await user.click(opener);

    const drawer = screen.getByRole("dialog", { name: "主导航" });
    expect(drawer).toBeVisible();
    expect(within(drawer).getAllByRole("button").filter((button) => button.classList.contains("cylune-nav__item"))).toHaveLength(6);
    expect(screen.getByRole("button", { name: "概览" })).toHaveFocus();
    expect(focus).toHaveBeenCalledWith({ preventScroll: true });

    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: "设置" })).toHaveFocus();

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "主导航" })).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "打开导航" })).toHaveFocus();
  });

  it("closes the narrow drawer after navigation and when its scrim is clicked", async () => {
    installViewport(true);
    const user = userEvent.setup();
    render(<Harness />);

    await user.click(screen.getByRole("button", { name: "打开导航" }));
    await user.click(screen.getByRole("button", { name: "耗材库" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "主导航" })).not.toBeInTheDocument());

    await user.click(screen.getByRole("button", { name: "打开导航" }));
    expect(screen.getByRole("button", { name: "耗材库" })).toHaveAttribute("aria-current", "page");
    fireEvent.click(screen.getByTestId("nav-drawer-scrim"));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "主导航" })).not.toBeInTheDocument());
  });

  it("does not require ResizeObserver in older WebViews or tests", () => {
    vi.stubGlobal("ResizeObserver", undefined);
    expect(() => render(<Harness initial="settings" />)).not.toThrow();
    expect(screen.getByRole("button", { name: "设置" })).toHaveAttribute("aria-current", "page");
  });
});
