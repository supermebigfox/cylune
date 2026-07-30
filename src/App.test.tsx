import React from "react";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App, DesktopApp } from "./App";
import { setLocale } from "./i18n";
import { demoPreview, type Spool, type TauriApi } from "./lib/tauri";

const persistedSpool: Spool = {
  spool_id: "spool-persisted",
  display_name: "持久化蓝色 PLA",
  preset_id: "Bambu PLA Basic @BBL A1",
  preset_base: null,
  catalog_id: null,
  brand: "Bambu Lab",
  material: "PLA",
  series: "Basic",
  color_name: null,
  color_code: null,
  color_hex: "#1C4EBB",
  color_hexes: ["#1C4EBB"],
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
    discardPendingJob: async () => undefined,
    listPrintProjects: async () => [],
    getPrintProject: async () => { throw new Error("unused"); },
    importPrintProject: async () => { throw new Error("unused"); },
    discardProject: async () => undefined,
    skipPlate: async () => undefined,
    confirmNewProject: async () => { throw new Error("unused"); },
    takePendingNavigation: async () => null,
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
      screen.getByRole("img", { name: "CYLUNE 图标" }),
    ).toBeVisible();
    expect(
      screen.getByRole("heading", { name: "CYLUNE" }),
    ).toBeVisible();
    expect(screen.getByText("本地模式")).toBeVisible();
  });

  it("switches English and Traditional Chinese copy without reloading", async () => {
    render(<App />);

    await act(() => setLocale("en"));
    expect(
      screen.getByRole("heading", { name: "CYLUNE" }),
    ).toBeVisible();
    expect(screen.getByText("Local mode")).toBeVisible();
    expect(document.documentElement.lang).toBe("en");

    await act(() => setLocale("zh-TW"));
    expect(
      screen.getByRole("img", { name: "CYLUNE 圖示" }),
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
      screen.getByRole("heading", { name: "CYLUNE" }),
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

  it("keeps spool creation failures accessible inside the portal dialog and refreshes them on retry", async () => {
    const user = userEvent.setup();
    let rejectRetry: (reason: unknown) => void = () => undefined;
    const createSpool = vi.fn()
      .mockRejectedValueOnce({ code: "database" })
      .mockImplementationOnce(() => new Promise<string>((_resolve, reject) => {
        rejectRetry = reject;
      }));
    render(
      <DesktopApp
        apiClient={fakeTauriApi({ createSpool })}
        pickFile={async () => null}
      />,
    );
    await screen.findByText("持久化蓝色 PLA");

    await user.click(screen.getByRole("button", { name: "耗材库" }));
    await user.click(screen.getByRole("button", { name: "添加一卷耗材" }));
    await user.click(screen.getByRole("button", { name: "PLA" }));
    await user.click(screen.getByRole("button", { name: "Basic" }));
    await user.click(screen.getByRole("button", { name: /玉石白.*10100/ }));
    await user.type(screen.getByLabelText("自定义名称"), "失败后保留");
    await user.clear(screen.getByLabelText("当前剩余量"));
    await user.type(screen.getByLabelText("当前剩余量"), "900");
    await user.click(screen.getByRole("button", { name: "保存" }));

    const dialog = screen.getByRole("dialog", { name: "添加一卷耗材" });
    const dialogAlert = await within(dialog).findByRole("alert");
    expect(dialog).toBeVisible();
    expect(dialogAlert).toHaveTextContent("本地数据暂时无法读取");
    expect(dialogAlert.closest("[inert]")).toBeNull();
    expect(
      within(dialog).getByRole("button", { name: /玉石白.*10100/ }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(within(dialog).getByLabelText("自定义名称")).toHaveValue(
      "失败后保留",
    );
    expect(within(dialog).getByLabelText("当前剩余量")).toHaveValue(900);

    await user.click(within(dialog).getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument(),
    );
    await act(async () => {
      rejectRetry({ code: "invalid_slot" });
    });

    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "AMS Lite 槽位编号无效",
    );
    await user.click(within(dialog).getByRole("button", { name: "关闭" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "AMS Lite 槽位编号无效",
    );
    expect(screen.getByRole("button", { name: "重试" })).toBeEnabled();

    await user.click(screen.getByRole("button", { name: "添加一卷耗材" }));
    const reopenedDialog = screen.getByRole("dialog", {
      name: "添加一卷耗材",
    });
    expect(within(reopenedDialog).queryByRole("alert")).not.toBeInTheDocument();
  });

  it("does not carry a late create failure into a new dialog session", async () => {
    const user = userEvent.setup();
    let rejectCreate: (reason: unknown) => void = () => undefined;
    const createSpool = vi.fn(() => new Promise<string>((_resolve, reject) => {
      rejectCreate = reject;
    }));
    render(
      <DesktopApp
        apiClient={fakeTauriApi({ createSpool })}
        pickFile={async () => null}
      />,
    );
    await screen.findByText("持久化蓝色 PLA");

    await user.click(screen.getByRole("button", { name: "耗材库" }));
    await user.click(screen.getByRole("button", { name: "添加一卷耗材" }));
    await user.click(screen.getByRole("button", { name: "PLA" }));
    await user.click(screen.getByRole("button", { name: "Basic" }));
    await user.click(screen.getByRole("button", { name: /玉石白.*10100/ }));
    await user.click(screen.getByRole("button", { name: "保存" }));
    await user.click(screen.getByRole("button", { name: "关闭" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );

    await act(async () => {
      rejectCreate({ code: "database" });
    });
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "本地数据暂时无法读取",
    );
    expect(screen.getByRole("button", { name: "重试" })).toBeEnabled();

    await user.click(screen.getByRole("button", { name: "添加一卷耗材" }));
    const reopenedDialog = screen.getByRole("dialog", {
      name: "添加一卷耗材",
    });
    expect(within(reopenedDialog).queryByRole("alert")).not.toBeInTheDocument();
  });

  it("resets a closed pending draft before a late create success", async () => {
    const user = userEvent.setup();
    let resolveCreate: (spoolId: string) => void = () => undefined;
    const createSpool = vi.fn(() => new Promise<string>((resolve) => {
      resolveCreate = resolve;
    }));
    render(
      <DesktopApp
        apiClient={fakeTauriApi({ createSpool })}
        pickFile={async () => null}
      />,
    );
    await screen.findByText("持久化蓝色 PLA");

    await user.click(screen.getByRole("button", { name: "耗材库" }));
    await user.click(screen.getByRole("button", { name: "添加一卷耗材" }));
    await user.click(screen.getByRole("button", { name: "PLA" }));
    await user.click(screen.getByRole("button", { name: "Basic" }));
    await user.type(screen.getByLabelText("搜索颜色"), "10100");
    await user.click(screen.getByRole("button", { name: /玉石白.*10100/ }));
    await user.type(screen.getByLabelText("自定义名称"), "不应重复的旧卷");
    await user.clear(screen.getByLabelText("当前剩余量"));
    await user.type(screen.getByLabelText("当前剩余量"), "900");
    await user.click(screen.getByRole("button", { name: "保存" }));
    await user.click(screen.getByRole("button", { name: "关闭" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );

    await act(async () => {
      resolveCreate("new-spool");
    });
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "添加一卷耗材" }),
      ).toBeEnabled(),
    );
    await user.click(screen.getByRole("button", { name: "添加一卷耗材" }));

    const reopenedDialog = screen.getByRole("dialog", {
      name: "添加一卷耗材",
    });
    expect(
      within(reopenedDialog).getByRole("button", { name: "PLA" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      within(reopenedDialog).getByRole("button", { name: "Basic" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      within(reopenedDialog).getByRole("button", { name: /玉石白.*10100/ }),
    ).toHaveAttribute("aria-pressed", "false");
    expect(within(reopenedDialog).getByLabelText("搜索颜色")).toHaveValue("");
    expect(within(reopenedDialog).getByLabelText("自定义名称")).toHaveValue("");
    expect(within(reopenedDialog).getByLabelText("当前剩余量")).toHaveValue(
      1000,
    );
    expect(
      within(reopenedDialog).getByRole("button", { name: "保存" }),
    ).toBeDisabled();
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

  it("discards an imported draft without deducting filament", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const discardPendingJob = vi.fn(async () => undefined);
    const importPrintFile = vi.fn(async () => ({ ...demoPreview, source_file_name: "wrong.gcode.3mf" }));
    render(<DesktopApp apiClient={fakeTauriApi({ importPrintFile, discardPendingJob })} pickFile={async () => "/tmp/wrong.gcode.3mf"} />);
    await screen.findByText("持久化蓝色 PLA");

    fireEvent.click(screen.getAllByRole("button", { name: "导入切片文件" })[0]);
    expect(await screen.findByText("wrong.gcode.3mf")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "取消此次导入" }));

    await waitFor(() => expect(discardPendingJob).toHaveBeenCalledWith(demoPreview.job_id));
    expect(await screen.findByText("还没有导入打印任务")).toBeVisible();
  });

  it("keeps the imported draft visible when discarding fails", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const importPrintFile = vi.fn(async () => ({ ...demoPreview, source_file_name: "keep.gcode.3mf" }));
    const discardPendingJob = vi.fn(async () => { throw { code: "database" }; });
    render(<DesktopApp apiClient={fakeTauriApi({ importPrintFile, discardPendingJob })} pickFile={async () => "/tmp/keep.gcode.3mf"} />);
    await screen.findByText("持久化蓝色 PLA");

    fireEvent.click(screen.getAllByRole("button", { name: "导入切片文件" })[0]);
    expect(await screen.findByText("keep.gcode.3mf")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "取消此次导入" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("本地数据暂时无法读取");
    expect(screen.getByText("keep.gcode.3mf")).toBeVisible();
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

  it("queues a watched print instead of replacing an unsettled preview", async () => {
    const handlers = new Map<string, (payload: unknown) => void>();
    const subscribeEvent = vi.fn(async (
      name: "open-job" | "watch-import" | "open-overview" | "pet-import-error",
      handler: (payload: unknown) => void,
    ) => {
      handlers.set(name, handler);
      return () => handlers.delete(name);
    });
    const getJobPreview = vi.fn(async (jobId: string) => ({
      ...demoPreview,
      job_id: jobId,
      source_file_name: `${jobId}.gcode.3mf`,
    }));
    render(<DesktopApp
      apiClient={fakeTauriApi({ getJobPreview })}
      pickFile={async () => null}
      subscribeEvent={subscribeEvent}
    />);
    await waitFor(() => expect(handlers.has("watch-import")).toBe(true));

    await act(async () => {
      handlers.get("watch-import")?.({ ok: true, job_id: "first", code: null });
    });
    fireEvent.click(screen.getByRole("button", { name: "打印任务" }));
    expect(await screen.findByText("first.gcode.3mf")).toBeVisible();

    await act(async () => {
      handlers.get("watch-import")?.({ ok: true, job_id: "second", code: null });
    });
    expect(await screen.findByText("监测文件夹发现了一个待结算任务")).toBeVisible();
    expect(screen.getByText("first.gcode.3mf")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "查看任务" }));
    expect(await screen.findByText("second.gcode.3mf")).toBeVisible();
  });

  it("handles pet overview navigation and stable import errors", async () => {
    const handlers = new Map<string, (payload: unknown) => void>();
    const subscribeEvent = vi.fn(async (
      name: "open-job" | "watch-import" | "open-overview" | "pet-import-error",
      handler: (payload: unknown) => void,
    ) => {
      handlers.set(name, handler);
      return () => handlers.delete(name);
    });
    render(<DesktopApp
      apiClient={fakeTauriApi({ getJobPreview: async () => demoPreview })}
      pickFile={async () => null}
      subscribeEvent={subscribeEvent}
    />);
    await waitFor(() => expect(handlers.has("open-overview")).toBe(true));
    fireEvent.click(screen.getByRole("button", { name: "耗材库" }));
    expect(screen.getByRole("heading", { name: "我的耗材库" })).toBeVisible();

    await act(async () => {
      handlers.get("open-overview")?.(null);
      handlers.get("pet-import-error")?.("unsliced_project");
    });

    expect(screen.getByRole("heading", { name: "今天想打印点什么？" })).toBeVisible();
    expect(screen.getByRole("alert")).toHaveTextContent("这个项目尚未切片");
  });

  it("does not consume persisted pet navigation during StrictMode cleanup", async () => {
    const takePendingJob = vi.fn()
      .mockResolvedValueOnce("persisted-job")
      .mockResolvedValue(null);
    const getJobPreview = vi.fn(async () => ({
      ...demoPreview,
      job_id: "persisted-job",
      source_file_name: "persisted.gcode.3mf",
    }));
    render(
      <React.StrictMode>
        <DesktopApp
          apiClient={fakeTauriApi({ getJobPreview, takePendingJob })}
          pickFile={async () => null}
        />
      </React.StrictMode>,
    );

    expect(await screen.findByText("persisted.gcode.3mf")).toBeVisible();
    expect(getJobPreview).toHaveBeenCalledWith("persisted-job");
  });
});
