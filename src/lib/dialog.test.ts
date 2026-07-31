import { expect, it, vi } from "vitest";
import {
  pickBackupDestination,
  pickBackupToImport,
  pickThreeMf,
} from "./dialog";

it("opens one 3MF picker for projects and sliced archives", async () => {
  const openDialog = vi.fn(async () => "/Users/robin/model.3mf");

  const path = await pickThreeMf("3MF 文件", openDialog);

  expect(path).toBe("/Users/robin/model.3mf");
  expect(openDialog).toHaveBeenCalledWith({
    multiple: false,
    directory: false,
    filters: [{ name: "3MF 文件", extensions: ["3mf"] }],
  });
});

it("treats cancel and unexpected multiple selection as no file", async () => {
  expect(await pickThreeMf("3MF 文件", async () => null)).toBeNull();
  expect(await pickThreeMf("3MF 文件", async () => ["a.3mf", "b.3mf"])).toBeNull();
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
