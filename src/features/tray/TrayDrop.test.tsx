import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
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

it("deduplicates only a short drop burst and allows deliberate repeat printing", async()=>{
  vi.useFakeTimers();
  const onImport=vi.fn(async()=>({job_id:"job",source_file_name:"repeat.gcode.3mf"}));
  render(<TrayDrop onImport={onImport} onOpenJob={vi.fn()}/>);const zone=screen.getByTestId("menu-dropzone");
  await act(async()=>{fireEvent.drop(zone,{dataTransfer:{files:[file("repeat.gcode.3mf")]}});await Promise.resolve();});
  fireEvent.drop(zone,{dataTransfer:{files:[file("repeat.gcode.3mf")]}});expect(onImport).toHaveBeenCalledTimes(1);
  await act(async()=>{vi.advanceTimersByTime(2100);fireEvent.drop(zone,{dataTransfer:{files:[file("repeat.gcode.3mf")]}});await Promise.resolve();});
  expect(onImport).toHaveBeenCalledTimes(2);vi.useRealTimers();
});

it("replays a transform-only entrance class whenever the native popover opens",async()=>{
  let show!:()=>void;const subscribe=vi.fn(async(handler:()=>void)=>{show=handler;return()=>undefined});
  render(<TrayDrop onImport={vi.fn()} onOpenJob={vi.fn()} subscribeVisibility={subscribe}/>);
  await waitFor(()=>expect(subscribe).toHaveBeenCalled());act(()=>show());
  expect(screen.getByTestId("tray-popover")).toHaveClass("entering");
  fireEvent.animationEnd(screen.getByTestId("tray-popover"));expect(screen.getByTestId("tray-popover")).not.toHaveClass("entering");
  act(()=>show());expect(screen.getByTestId("tray-popover")).toHaveClass("entering");
});
