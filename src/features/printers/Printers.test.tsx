import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import type {
  PrinterProfile,
  SavePrinter,
  SavedPrinter,
} from "../../lib/tauri";
import { Printers } from "./Printers";

const p2s: PrinterProfile = {
  model_key: "Bambu Lab P2S",
  display_name: "Bambu Lab P2S",
  nozzle_diameters: [0.2, 0.4, 0.6, 0.8],
  plate_keys: ["Cool Plate", "Supertack Plate", "Textured PEI Plate"],
};

function printerApi({
  profiles = [p2s],
  initial = [],
}: {
  profiles?: PrinterProfile[];
  initial?: SavedPrinter[];
} = {}) {
  let saved = initial.map((printer) => ({ ...printer }));
  const listAvailablePrinters = vi.fn(async () => profiles);
  const listSavedPrinters = vi.fn(async () => saved.map((printer) => ({ ...printer })));
  const savePrinter = vi.fn(async (draft: SavePrinter) => {
    const next: SavedPrinter = {
      printer_id: draft.printer_id ?? "printer-p2s",
      display_name: draft.display_name,
      model_key: draft.model_key,
      nozzle_diameter: draft.nozzle_diameter,
      default_plate: draft.default_plate,
      ams_kind: draft.ams_kind,
      is_default: draft.is_default,
      is_available: true,
    };
    saved = draft.is_default
      ? saved.map((printer) => ({ ...printer, is_default: false }))
      : saved;
    saved = [...saved.filter((printer) => printer.printer_id !== next.printer_id), next];
    return { ...next, is_available: false };
  });
  const deletePrinter = vi.fn(async (printerId: string) => {
    saved = saved.filter((printer) => printer.printer_id !== printerId);
  });
  const setDefaultPrinter = vi.fn(async (printerId: string) => {
    saved = saved.map((printer) => ({
      ...printer,
      is_default: printer.printer_id === printerId,
    }));
  });
  return {
    api: {
      mode: "tauri",
      listAvailablePrinters,
      listSavedPrinters,
      savePrinter,
      deletePrinter,
      setDefaultPrinter,
    },
    listSavedPrinters,
    savePrinter,
  };
}

beforeEach(() => setLocale("zh-CN"));

it("adds My P2S with an official nozzle, plate, AMS and default selection", async () => {
  const user = userEvent.setup();
  const client = printerApi();
  render(<Printers apiClient={client.api} />);

  await user.click(await screen.findByRole("button", { name: "添加打印机" }));
  const close = screen.getByRole("button", { name: "关闭" });
  expect(close).toHaveFocus();
  await user.type(screen.getByLabelText("打印机名称"), "我的 P2S");
  await user.selectOptions(screen.getByLabelText("打印机型号"), "Bambu Lab P2S");
  await user.selectOptions(screen.getByLabelText("喷嘴直径"), "0.4");
  await user.selectOptions(screen.getByLabelText("打印板"), "Supertack Plate");
  await user.selectOptions(screen.getByLabelText("供料系统"), "ams");
  await user.click(screen.getByRole("checkbox", { name: "设为默认打印机" }));
  await user.click(screen.getByRole("button", { name: "保存打印机" }));

  await waitFor(() => expect(client.savePrinter).toHaveBeenCalledWith({
    printer_id: undefined,
    display_name: "我的 P2S",
    model_key: "Bambu Lab P2S",
    nozzle_diameter: 0.4,
    default_plate: "Supertack Plate",
    ams_kind: "ams",
    is_default: true,
  }));
  expect(await screen.findByRole("heading", { name: "我的 P2S" })).toBeVisible();
  expect(screen.getByText("默认打印机")).toBeVisible();
  expect(client.listSavedPrinters).toHaveBeenCalledTimes(2);
});

it("keeps unavailable saved printers manageable while preventing a slice", async () => {
  const user = userEvent.setup();
  const unavailable: SavedPrinter = {
    printer_id: "missing-model",
    display_name: "工作室旧机器",
    model_key: "Bambu Lab Missing",
    nozzle_diameter: 0.4,
    default_plate: "Cool Plate",
    ams_kind: "none",
    is_default: false,
    is_available: false,
  };
  const client = printerApi({ profiles: [], initial: [unavailable] });
  render(<Printers apiClient={client.api} />);

  expect(await screen.findByRole("heading", { name: "工作室旧机器" })).toBeVisible();
  expect(screen.getByText("当前 Bambu Studio 中找不到这套官方配置")).toBeVisible();
  expect(screen.getByRole("button", { name: "设为默认" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "使用此打印机切片" })).toBeDisabled();

  await user.click(screen.getByRole("button", { name: "编辑打印机" }));
  const name = screen.getByLabelText("打印机名称");
  await user.clear(name);
  await user.type(name, "工作室旧机器（改名）");
  expect(screen.getByLabelText("喷嘴直径")).toBeEnabled();
  expect(screen.getByLabelText("打印板")).toBeEnabled();
  await user.click(screen.getByRole("button", { name: "保存打印机" }));

  await waitFor(() => expect(client.savePrinter).toHaveBeenCalledWith({
    printer_id: "missing-model",
    display_name: "工作室旧机器（改名）",
    model_key: "Bambu Lab Missing",
    nozzle_diameter: 0.4,
    default_plate: "Cool Plate",
    ams_kind: "none",
    is_default: false,
  }));
});

it("traps focus, closes with Escape, and restores focus to the add button", async () => {
  const user = userEvent.setup();
  const client = printerApi();
  render(<Printers apiClient={client.api} />);
  const opener = await screen.findByRole("button", { name: "添加打印机" });

  await user.click(opener);
  expect(screen.getByRole("button", { name: "关闭" })).toHaveFocus();
  await user.tab({ shift: true });
  expect(screen.getByRole("button", { name: "取消" })).toHaveFocus();
  await user.keyboard("{Escape}");

  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(opener).toHaveFocus();
});
