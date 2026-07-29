import { expect, it, vi } from "vitest";
import { pickBackupDestination, pickBackupToImport, pickSliced3mf } from "./dialog";

it("opens a single sliced 3MF picker and returns the selected native path", async () => {
  const openDialog = vi.fn(async () => "/Users/robin/model.gcode.3mf");

  const path = await pickSliced3mf("已切片 3MF", openDialog);

  expect(path).toBe("/Users/robin/model.gcode.3mf");
  expect(openDialog).toHaveBeenCalledWith({
    multiple: false,
    directory: false,
    filters: [{ name: "已切片 3MF", extensions: ["3mf"] }],
  });
});

it("treats cancel and unexpected multiple selection as no file", async () => {
  expect(await pickSliced3mf("已切片 3MF", async () => null)).toBeNull();
  expect(await pickSliced3mf("已切片 3MF", async () => ["a.3mf", "b.3mf"])).toBeNull();
});

it("uses localized backup labels and filenames", async () => {
  const openDialog = vi.fn(async () => "/tmp/backup.json");
  const saveDialog = vi.fn(async () => "/tmp/backup.json");

  expect(await pickBackupToImport("耗材备份", openDialog as never)).toBe("/tmp/backup.json");
  expect(await pickBackupDestination("耗材备份", "CYLUNE-备份.json", saveDialog as never)).toBe("/tmp/backup.json");
  expect(openDialog).toHaveBeenCalledWith({
    multiple: false,
    directory: false,
    filters: [{ name: "耗材备份", extensions: ["json"] }],
  });
  expect(saveDialog).toHaveBeenCalledWith({
    defaultPath: "CYLUNE-备份.json",
    filters: [{ name: "耗材备份", extensions: ["json"] }],
  });
});
