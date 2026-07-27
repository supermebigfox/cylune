import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App, DesktopApp } from "./App";
import { setLocale } from "./i18n";
import { demoPreview, type Spool, type TauriApi } from "./lib/tauri";

const persistedSpool: Spool = {
  spool_id: "spool-persisted",
  display_name: "持久化蓝色 PLA",
  preset_id: "Bambu PLA Basic @BBL A1",
  brand: "Bambu Lab",
  material: "PLA",
  series: "Basic",
  color_hex: "#1C4EBB",
  remaining_grams: 504.2,
  status: "assigned",
};

function fakeTauriApi(overrides: Partial<TauriApi> = {}): TauriApi {
  return {
    mode: "tauri",
    createSpool: async () => "new-spool",
    mountSpool: async () => undefined,
    unmountSlot: async () => undefined,
    moveSpool: async () => undefined,
    calibrateSpool: async () => undefined,
    archiveSpool: async () => undefined,
    listSpools: async () => [persistedSpool],
    listSlots: async () => [
      { slot_number: 1, spool_id: null },
      { slot_number: 2, spool_id: null },
      { slot_number: 3, spool_id: persistedSpool.spool_id },
      { slot_number: 4, spool_id: null },
    ],
    importPrintFile: async () => { throw new Error("unused"); },
    confirmJobMapping: async () => undefined,
    confirmNewPrint: async () => { throw new Error("unused"); },
    settleJob: async () => { throw new Error("unused"); },
    reverseSettlement: async () => ({ job_id: "job", settlement_version: 1, already_reversed: false, restored: [] }),
    ...overrides,
  };
}

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

  it("restores exact persisted slot numbers instead of inferring spool order", async () => {
    render(<DesktopApp apiClient={fakeTauriApi()} pickFile={async () => null} />);

    const slots = await screen.findAllByTestId("ams-slot");

    expect(within(slots[0]).queryByText("持久化蓝色 PLA")).not.toBeInTheDocument();
    expect(within(slots[2]).getByText("持久化蓝色 PLA")).toBeVisible();
  });

  it("refreshes both spool and slot truth after unmounting", async () => {
    const unmountSlot = vi.fn(async () => undefined);
    const listSpools = vi.fn(async () => [persistedSpool]);
    const listSlots = vi.fn(async () => [
      { slot_number: 1 as const, spool_id: null },
      { slot_number: 2 as const, spool_id: null },
      { slot_number: 3 as const, spool_id: persistedSpool.spool_id },
      { slot_number: 4 as const, spool_id: null },
    ]);
    render(<DesktopApp apiClient={fakeTauriApi({ unmountSlot, listSpools, listSlots })} pickFile={async () => null} />);
    await screen.findByText("持久化蓝色 PLA");
    fireEvent.click(screen.getByRole("button", { name: "耗材库" }));
    fireEvent.click(screen.getByRole("button", { name: "从 AMS 拆下" }));

    await waitFor(() => expect(unmountSlot).toHaveBeenCalledWith(3));
    expect(listSpools).toHaveBeenCalledTimes(2);
    expect(listSlots).toHaveBeenCalledTimes(2);
  });

  it("selects and imports a sliced 3MF from the main window", async () => {
    const pickFile = vi.fn(async () => "/Users/robin/Desktop/model.gcode.3mf");
    const importPrintFile = vi.fn(async () => ({ ...demoPreview, source_file_name: "model.gcode.3mf" }));
    render(<DesktopApp apiClient={fakeTauriApi({ importPrintFile })} pickFile={pickFile} />);
    await screen.findByText("持久化蓝色 PLA");

    fireEvent.click(screen.getAllByRole("button", { name: "导入切片文件" })[1]);

    expect(await screen.findByText("model.gcode.3mf")).toBeVisible();
    expect(pickFile).toHaveBeenCalledTimes(1);
    expect(importPrintFile).toHaveBeenCalledWith("/Users/robin/Desktop/model.gcode.3mf");
  });

  it("supplies the native picker label from the current locale", async () => {
    await setLocale("zh-TW");
    const pickFile = vi.fn(async (_filterName: string) => null);
    render(<DesktopApp apiClient={fakeTauriApi()} pickFile={pickFile} />);
    await screen.findByText("持久化蓝色 PLA");

    fireEvent.click(screen.getAllByRole("button", { name: "匯入切片檔案" })[0]);

    await waitFor(() => expect(pickFile).toHaveBeenCalledWith("已切片 3MF 檔案"));
  });

  it("prevents duplicate imports while busy and translates a stable rejected error", async () => {
    let rejectImport: (reason: unknown) => void = () => undefined;
    const importPrintFile = vi.fn(() => new Promise<never>((_resolve, reject) => { rejectImport = reject; }));
    render(<DesktopApp apiClient={fakeTauriApi({ importPrintFile })} pickFile={async () => "/tmp/bad.3mf"} />);
    await screen.findByText("持久化蓝色 PLA");
    const importButton = screen.getAllByRole("button", { name: "导入切片文件" })[0];

    fireEvent.click(importButton);
    fireEvent.click(importButton);
    await waitFor(() => expect(importPrintFile).toHaveBeenCalledTimes(1));
    expect(screen.getAllByRole("button", { name: "正在读取颜色与预计用量…" }).every((button) => button.hasAttribute("disabled"))).toBe(true);

    await act(() => rejectImport({ code: "invalid_file" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("无法识别这个文件");
    expect(screen.getAllByRole("button", { name: "导入切片文件" })[0]).toBeEnabled();
  });
});
