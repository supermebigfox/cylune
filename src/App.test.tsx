import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { setLocale } from "./i18n";

describe("App localization", () => {
  beforeEach(async () => {
    await setLocale("zh-CN");
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("shows the localized prototype shell", () => {
    render(<App />);
    expect(
      screen.getByRole("img", { name: "拓竹耗材管家图标" }),
    ).toBeVisible();
    expect(
      screen.getByRole("heading", { name: "拓竹耗材管家" }),
    ).toBeVisible();
    expect(screen.getByText("本地模式")).toBeVisible();
  });

  it("switches English and Traditional Chinese copy without reloading", async () => {
    render(<App />);

    await act(() => setLocale("en"));
    expect(
      screen.getByRole("heading", { name: "Bambu Spool Keeper" }),
    ).toBeVisible();
    expect(screen.getByText("Local mode")).toBeVisible();
    expect(document.documentElement.lang).toBe("en");

    await act(() => setLocale("zh-TW"));
    expect(
      screen.getByRole("img", { name: "拓竹耗材管家圖示" }),
    ).toBeVisible();
    expect(screen.getByText("本機模式")).toBeVisible();
    expect(document.documentElement.lang).toBe("zh-TW");
  });

  it("notifies rendered subscribers when locale persistence is denied", async () => {
    render(<App />);
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("denied", "SecurityError");
    });

    await act(() => setLocale("en"));

    expect(
      screen.getByRole("heading", { name: "Bambu Spool Keeper" }),
    ).toBeVisible();
    expect(document.documentElement.lang).toBe("en");
  });

  it("navigates across the four localized desktop sections", async () => {
    render(<App />);

    expect(screen.getByRole("navigation", { name: "主导航" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "耗材库" }));
    expect(screen.getByRole("heading", { name: "我的耗材库" })).toBeVisible();

    await act(() => setLocale("en"));
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeVisible();
    expect(screen.getByText("Your print files never leave this Mac.")).toBeVisible();
  });
});
