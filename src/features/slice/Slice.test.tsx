import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import {
  Slice,
  type SliceApi,
  type SliceCompleteEvent,
  type SliceErrorEvent,
  type SliceEventName,
  type SliceEventSubscriber,
  type SliceInspection,
  type SliceProgressEvent,
  type SliceRequest,
} from "./Slice";

const p2s = {
  printer_id: "printer-p2s",
  display_name: "我的 P2S",
  model_key: "Bambu Lab P2S",
  nozzle_diameter: 0.4,
  default_plate: "supertack",
  ams_kind: "ams",
  is_default: true,
  is_available: true,
};

const inspection: SliceInspection = {
  kind: "unsliced",
  file_name: "月球灯.3mf",
  plate_count: 2,
  embedded_model_key: "Bambu Lab P2S",
  embedded_nozzle_diameter: 0.4,
  tools: [
    {
      tool: 0,
      label: "颜色 1",
      material: "PLA Basic",
      color_hex: "#FFFEFC",
      embedded_filament_key: "pla-basic-white",
    },
    {
      tool: 1,
      label: "颜色 2",
      material: "PLA Basic",
      color_hex: "#252733",
      embedded_filament_key: "pla-basic-black",
    },
  ],
};

type EventPayloads = {
  "slice-progress": SliceProgressEvent;
  "slice-complete": SliceCompleteEvent;
  "slice-error": SliceErrorEvent;
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function fixture({
  inspected = inspection,
  cancel = async () => undefined,
  getTask = async () => ({
    task_id: "slice-task-1",
    state: "cancelled" as const,
    phase: "slicing" as const,
    percent: null,
    project_id: null,
    error_code: "slicer_cancelled",
  }),
}: {
  inspected?: SliceInspection;
  cancel?: (taskId: string) => Promise<void>;
  getTask?: SliceApi["getSliceTask"];
} = {}) {
  const handlers = new Map<SliceEventName, Set<(payload: unknown) => void>>();
  const subscribeEvent: SliceEventSubscriber = vi.fn(async (name, handler) => {
    const listeners = handlers.get(name) ?? new Set();
    listeners.add(handler);
    handlers.set(name, listeners);
    return () => listeners.delete(handler);
  });
  const startSlice = vi.fn(async (_request: SliceRequest) => ({
    task_id: "slice-task-1",
    state: "running" as const,
    phase: "preparing" as const,
    percent: null,
    project_id: null,
    error_code: null,
  }));
  const openInBambuStudio = vi.fn(async () => undefined);
  const api: SliceApi = {
    listSavedPrinters: vi.fn(async () => [p2s]),
    inspect3mf: vi.fn(async () => inspected),
    startSlice,
    cancelSlice: vi.fn(cancel),
    getSliceTask: vi.fn(getTask),
    openInBambuStudio,
  };
  const emit = <Name extends SliceEventName>(name: Name, payload: EventPayloads[Name]) => {
    handlers.get(name)?.forEach((handler) => handler(payload));
  };
  return { api, subscribeEvent, emit, startSlice, openInBambuStudio };
}

function renderSlice({
  client = fixture(),
  pickInput = vi.fn(async () => "/Users/robin/Desktop/月球灯.3mf"),
  onProjectComplete = vi.fn(),
  onSlicedFile = vi.fn(),
  onFormLockChange = vi.fn(),
} = {}) {
  render(
    <Slice
      api={client.api}
      pickInput={pickInput}
      subscribeEvent={client.subscribeEvent}
      onProjectComplete={onProjectComplete}
      onSlicedFile={onSlicedFile}
      onFormLockChange={onFormLockChange}
    />,
  );
  return { client, pickInput, onProjectComplete, onSlicedFile, onFormLockChange };
}

async function prepareReadyForm() {
  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: "选择 3MF" }));
  await screen.findByRole("heading", { name: "月球灯.3mf" });
  return user;
}

beforeEach(() => setLocale("zh-CN"));

it("starts metadata slicing without asking for an output file", async () => {
  const { client } = renderSlice();
  const user = await prepareReadyForm();

  expect(screen.queryByRole("button", { name: "选择输出位置" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "开始后台切片" })).toBeEnabled();
  await user.click(screen.getByRole("button", { name: "开始后台切片" }));

  await waitFor(() => expect(client.startSlice).toHaveBeenCalledWith(
    expect.not.objectContaining({ output_path: expect.anything() }),
  ));
});

it("uses the preferred machine, keeps the embedded project read-only, reports progress, and opens the completed project", async () => {
  const user = userEvent.setup();
  const onProjectComplete = vi.fn();
  const client = fixture({
    inspected: {
      ...inspection,
      embedded_process_key: "Bambu_Lumina",
      embedded_plate_key: "Textured PEI Plate",
      embedded_infill_density: 100,
      embedded_support_enabled: true,
    },
  });
  const { onFormLockChange } = renderSlice({ client, onProjectComplete });

  await user.click(await screen.findByRole("button", { name: "选择 3MF" }));
  expect(await screen.findByText("2 个打印盘 · 切片后仍是 1 个项目")).toBeVisible();
  expect(screen.getByText("我的 P2S · Bambu Lab P2S · 0.4 mm")).toBeVisible();
  expect(screen.getByText("Bambu_Lumina")).toBeVisible();
  expect(screen.getByText("Textured PEI Plate")).toBeVisible();
  expect(screen.getByText("100%")).toBeVisible();
  expect(screen.getByText("已开启")).toBeVisible();
  expect(screen.queryByRole("combobox", { name: "目标打印机" })).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "工艺与层高" })).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "打印板" })).not.toBeInTheDocument();
  expect(screen.queryByRole("spinbutton", { name: "填充率" })).not.toBeInTheDocument();
  expect(screen.queryByRole("checkbox", { name: "生成支撑" })).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "颜色 2 耗材" })).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "开始后台切片" }));
  await waitFor(() => expect(client.startSlice).toHaveBeenCalledWith({
    input_path: "/Users/robin/Desktop/月球灯.3mf",
    printer_id: "printer-p2s",
    confirm_printer_mismatch: false,
  }));

  expect(screen.getByRole("button", { name: "更换 3MF" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "取消切片" })).toBeEnabled();
  await waitFor(() => expect(onFormLockChange).toHaveBeenLastCalledWith(true));

  act(() => client.emit("slice-progress", {
    task_id: "slice-task-1",
    phase: "slicing",
    percent: 46,
  }));
  expect(await screen.findByText("46%")).toBeVisible();
  expect(screen.getByRole("progressbar")).toHaveAttribute("value", "46");

  act(() => client.emit("slice-complete", {
    task_id: "slice-task-1",
    project_id: "project-moon-lamp",
  }));
  await waitFor(() => expect(onProjectComplete).toHaveBeenCalledWith("project-moon-lamp"));
  expect(onFormLockChange).toHaveBeenLastCalledWith(false);
});

it("shows inspection state and accepts a dropped desktop file path", async () => {
  const inspecting = deferred<SliceInspection>();
  const client = fixture();
  client.api.inspect3mf = vi.fn(() => inspecting.promise);
  renderSlice({ client });
  await screen.findByRole("button", { name: "选择 3MF" });

  const file = new File(["project"], "月球灯.3mf", { type: "model/3mf" });
  Object.defineProperty(file, "path", { value: "/Users/robin/Desktop/月球灯.3mf" });
  fireEvent.drop(screen.getByTestId("slice-drop-zone"), {
    dataTransfer: { files: [file], getData: () => "" },
  });

  expect(await screen.findByText("正在检查 3MF…")).toBeVisible();
  expect(client.api.inspect3mf).toHaveBeenCalledWith("/Users/robin/Desktop/月球灯.3mf");
  inspecting.resolve(inspection);
  expect(await screen.findByRole("heading", { name: "月球灯.3mf" })).toBeVisible();
});

it("refreshes saved printers whenever the persistent slicing page becomes active again", async () => {
  const client = fixture();
  client.api.listSavedPrinters = vi.fn()
    .mockResolvedValueOnce([])
    .mockResolvedValue([p2s]);
  const props = {
    api: client.api,
    pickInput: vi.fn(async () => "/Users/robin/Desktop/月球灯.3mf"),
    subscribeEvent: client.subscribeEvent,
    onProjectComplete: vi.fn(),
  };
  const { rerender } = render(<Slice {...props} active />);

  await userEvent.click(await screen.findByRole("button", { name: "选择 3MF" }));
  expect(await screen.findByText(/还没有可用的打印机/)).toBeVisible();

  rerender(<Slice {...props} active={false} />);
  rerender(<Slice {...props} active />);

  await waitFor(() => expect(client.api.listSavedPrinters).toHaveBeenCalledTimes(2));
  expect(screen.getByText("我的 P2S · Bambu Lab P2S · 0.4 mm")).toBeVisible();
});

it("cancels an unstarted slicing setup and returns to the empty drop zone", async () => {
  const { client } = renderSlice();
  const user = await prepareReadyForm();

  await user.click(screen.getByRole("button", { name: "取消本次切片" }));

  expect(screen.getByRole("button", { name: "选择 3MF" })).toBeVisible();
  expect(screen.queryByRole("heading", { name: "月球灯.3mf" })).not.toBeInTheDocument();
  expect(client.startSlice).not.toHaveBeenCalled();
});

it("requires explicit confirmation when the embedded machine differs from the target printer", async () => {
  const client = fixture({
    inspected: {
      ...inspection,
      embedded_model_key: "Bambu Lab A1",
      embedded_nozzle_diameter: 0.4,
    },
  });
  renderSlice({ client });
  const user = await prepareReadyForm();

  expect(screen.getByRole("alert")).toHaveTextContent("项目内嵌机型是 Bambu Lab A1");
  expect(screen.getByRole("button", { name: "开始后台切片" })).toBeDisabled();
  await user.click(screen.getByRole("checkbox", { name: "确认改用我的 P2S" }));
  expect(screen.getByRole("button", { name: "开始后台切片" })).toBeEnabled();
});

it("keeps the preferred printer while leaving embedded filament settings untouched", async () => {
  const client = fixture({
    inspected: {
      ...inspection,
      embedded_model_key: "Bambu Lab X2D",
      embedded_nozzle_diameter: 0.4,
      tools: [{
        tool: 0,
        label: "Bambu PLA Basic @BBL X2D 0.4 nozzle",
        material: "PLA",
        color_hex: "#F5547C",
        embedded_filament_key: "Bambu PLA Basic @BBL X2D 0.4 nozzle",
      }],
    },
  });
  renderSlice({ client });
  const user = await prepareReadyForm();

  expect(screen.getByText("我的 P2S · Bambu Lab P2S · 0.4 mm")).toBeVisible();
  expect(screen.getByText("Bambu PLA Basic @BBL X2D 0.4 nozzle")).toBeVisible();
  expect(screen.queryByRole("combobox", {
    name: "Bambu PLA Basic @BBL X2D 0.4 nozzle 耗材",
  })).not.toBeInTheDocument();

  await user.click(screen.getByRole("checkbox", { name: "确认改用我的 P2S" }));
  await user.click(screen.getByRole("button", { name: "开始后台切片" }));

  await waitFor(() => expect(client.startSlice).toHaveBeenCalledWith({
    input_path: "/Users/robin/Desktop/月球灯.3mf",
    printer_id: "printer-p2s",
    confirm_printer_mismatch: true,
  }));
});

it("shows compatible embedded process, plate, infill, and support as read-only values", async () => {
  const embedded = {
    ...inspection,
    embedded_process_key: "fine-012",
    embedded_plate_key: "textured",
    embedded_infill_density: 27,
    embedded_support_enabled: true,
  } as SliceInspection & {
    embedded_process_key: string;
    embedded_plate_key: string;
    embedded_infill_density: number;
    embedded_support_enabled: boolean;
  };
  renderSlice({ client: fixture({ inspected: embedded }) });

  await prepareReadyForm();

  expect(screen.getByText("fine-012")).toBeVisible();
  expect(screen.getByText("textured")).toBeVisible();
  expect(screen.getByText("27%")).toBeVisible();
  expect(screen.getByText("已开启")).toBeVisible();
  expect(screen.queryByRole("combobox", { name: "工艺与层高" })).not.toBeInTheDocument();
  expect(screen.queryByRole("combobox", { name: "打印板" })).not.toBeInTheDocument();
  expect(screen.queryByRole("spinbutton", { name: "填充率" })).not.toBeInTheDocument();
  expect(screen.queryByRole("checkbox", { name: "生成支撑" })).not.toBeInTheDocument();
});

it("keeps cancel available, shows stopping until the child exits, and then unlocks the form", async () => {
  const stopping = deferred<void>();
  const client = fixture({ cancel: () => stopping.promise });
  renderSlice({ client });
  const user = await prepareReadyForm();
  await user.click(screen.getByRole("button", { name: "开始后台切片" }));

  await user.click(screen.getByRole("button", { name: "取消切片" }));
  expect(screen.getByRole("button", { name: "正在停止…" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "更换 3MF" })).toBeDisabled();

  stopping.resolve();
  expect(await screen.findByText("切片已取消")).toBeVisible();
  expect(screen.getByRole("button", { name: "更换 3MF" })).toBeEnabled();
});

it("uses the authoritative completed task when cancellation loses the import race", async () => {
  const user = userEvent.setup();
  const onProjectComplete = vi.fn();
  const client = fixture({
    getTask: async () => ({
      task_id: "slice-task-1",
      state: "completed",
      phase: "complete",
      percent: 100,
      project_id: "project-after-import",
      error_code: null,
    }),
  });
  renderSlice({ client, onProjectComplete });
  await prepareReadyForm();
  await user.click(screen.getByRole("button", { name: "开始后台切片" }));

  await user.click(screen.getByRole("button", { name: "取消切片" }));

  await waitFor(() => expect(client.api.getSliceTask).toHaveBeenCalledWith("slice-task-1"));
  expect(onProjectComplete).toHaveBeenCalledWith("project-after-import");
  expect(screen.queryByText("切片已取消")).not.toBeInTheDocument();
});

it("shows an indeterminate phase and only opens Bambu Studio after the user clicks the fallback", async () => {
  const client = fixture();
  renderSlice({ client });
  const user = await prepareReadyForm();
  await user.click(screen.getByRole("button", { name: "开始后台切片" }));

  act(() => client.emit("slice-progress", {
    task_id: "slice-task-1",
    phase: "validating",
    percent: null,
  }));
  expect(await screen.findByText("正在验证切片结果")).toBeVisible();
  expect(screen.getByRole("progressbar")).not.toHaveAttribute("value");

  act(() => client.emit("slice-error", {
    task_id: "slice-task-1",
    code: "slicer_failed",
    message: "CLI exited with status 1",
  }));
  const alert = await screen.findByRole("alert");
  expect(within(alert).getByText("Bambu Studio 无法完成这个项目")).toBeVisible();
  expect(client.openInBambuStudio).not.toHaveBeenCalled();

  await user.click(within(alert).getByRole("button", { name: "使用 Bambu Studio 打开" }));
  expect(client.openInBambuStudio).toHaveBeenCalledWith("/Users/robin/Desktop/月球灯.3mf");
});

it("rehydrates an active background task when the user returns to the slicing page", async () => {
  const client = fixture();
  render(<Slice
    api={client.api}
    pickInput={vi.fn(async () => null)}
    subscribeEvent={client.subscribeEvent}
    onProjectComplete={vi.fn()}
    initialInputPath="/Users/robin/Desktop/月球灯.3mf"
    activeTask={{
      task_id: "slice-task-1",
      state: "running",
      phase: "slicing",
      percent: 61,
      project_id: null,
      error_code: null,
    }}
  />);

  expect(await screen.findByRole("heading", { name: "月球灯.3mf" })).toBeVisible();
  expect(screen.getByText("61%")).toBeVisible();
  expect(screen.getByRole("progressbar")).toHaveAttribute("value", "61");
  expect(screen.queryByRole("combobox", { name: "目标打印机" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "取消切片" })).toBeEnabled();
});

it("hands an already-sliced file to the print-job importer instead of opening Studio", async () => {
  const client = fixture({ inspected: { ...inspection, kind: "sliced" } });
  const onSlicedFile = vi.fn();
  const { openInBambuStudio } = client;
  renderSlice({ client, onSlicedFile });

  await userEvent.click(await screen.findByRole("button", { name: "选择 3MF" }));

  await waitFor(() => expect(onSlicedFile).toHaveBeenCalledWith("/Users/robin/Desktop/月球灯.3mf"));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(openInBambuStudio).not.toHaveBeenCalled();
});
