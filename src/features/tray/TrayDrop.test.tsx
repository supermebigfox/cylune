import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import { TrayDrop } from "./TrayDrop";

function file(name: string) { return new File(["fixture"], name); }

beforeEach(async () => setLocale("zh-CN"));

it("accepts supported intake files and rejects unsupported files individually", async () => {
  const onImport = vi.fn(async () => ({ job_id: "job-1", source_file_name: "plate.gcode.3mf" }));
  render(<TrayDrop onImport={onImport} onOpenJob={vi.fn()} />);
  fireEvent.drop(screen.getByTestId("menu-dropzone"), { dataTransfer: { files: [file("plate.gcode.3mf"), file("raw.gcode"), file("notes.pdf")] } });
  await waitFor(() => expect(onImport).toHaveBeenCalledTimes(1));
  expect(onImport).toHaveBeenCalledWith("plate.gcode.3mf");
  expect(screen.getByText("不支持 notes.pdf")).toBeVisible();
});

it("imports one file at a time and opens the resulting job in the main window", async () => {
  let finish!: (value: { job_id: string; source_file_name: string }) => void;
  const onImport = vi.fn(() => new Promise<{ job_id: string; source_file_name: string }>((resolve) => { finish = resolve; }));
  const onOpenJob = vi.fn();
  render(<TrayDrop onImport={onImport} onOpenJob={onOpenJob} />);
  const zone = screen.getByTestId("menu-dropzone");
  fireEvent.drop(zone, { dataTransfer: { files: [file("first.gcode.3mf")] } });
  fireEvent.drop(zone, { dataTransfer: { files: [file("second.gcode.3mf")] } });
  expect(onImport).toHaveBeenCalledTimes(1);
  finish({ job_id: "job-9", source_file_name: "first.gcode.3mf" });
  expect(await screen.findByText("文件已读取，可以核对耗材卷")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "查看并绑定耗材卷" }));
  expect(onOpenJob).toHaveBeenCalledWith("job-9");
});
