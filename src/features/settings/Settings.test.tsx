import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import { api, type TauriApi } from "../../lib/tauri";
import { Theme } from "../../theme/Theme";
import { Settings } from "./Settings";

beforeEach(async () => {
  await setLocale("zh-CN");
});

function renderSettings(
  apiClient: TauriApi,
  dialogs: {
    watch(): Promise<string | null>;
    importBackup(filter: string): Promise<string | null>;
    exportBackup(filter: string, name: string): Promise<string | null>;
  },
) {
  return render(
    <Theme>
      <Settings apiClient={apiClient} dialogs={dialogs} />
    </Theme>,
  );
}

it("guards backup restore against double clicks and keeps localized dialog copy", async () => {
  let finish!: () => void;
  const importBackup = vi.fn(
    () => new Promise<string>((resolve) => {
      finish = () => resolve("/tmp/pre-restore.json");
    }),
  );
  const apiClient = {
    ...api,
    mode: "tauri",
    getWatchFolder: vi.fn(async () => null),
    importBackup,
  } as TauriApi;
  const dialogs = {
    watch: vi.fn(async () => null),
    importBackup: vi.fn(async () => "/tmp/backup.json"),
    exportBackup: vi.fn(async () => null),
  };
  renderSettings(apiClient, dialogs);

  const restore = screen.getByRole("button", { name: "恢复备份" });
  fireEvent.click(restore);
  fireEvent.click(restore);

  await waitFor(() => expect(importBackup).toHaveBeenCalledTimes(1));
  expect(dialogs.importBackup).toHaveBeenCalledWith("CYLUNE JSON 备份");
  expect(restore).toBeDisabled();
  await act(async () => finish());
  await waitFor(() => expect(restore).not.toBeDisabled());
});

it("shows a localized retryable error instead of rejecting a watch action", async () => {
  const apiClient = {
    ...api,
    mode: "tauri",
    getWatchFolder: vi.fn(async () => null),
    setWatchFolder: vi.fn(async () => {
      throw { code: "invalid_file" };
    }),
  } as TauriApi;
  const dialogs = {
    watch: vi.fn(async () => "/missing/folder"),
    importBackup: vi.fn(async () => null),
    exportBackup: vi.fn(async () => null),
  };
  renderSettings(apiClient, dialogs);

  fireEvent.click(screen.getByRole("button", { name: "启用" }));

  expect(await screen.findByRole("alert")).toHaveTextContent("无法识别这个文件");
  expect(screen.getByRole("button", { name: "启用" })).not.toBeDisabled();
});

it("includes the localized desktop black hole controls in the settings page", () => {
  const apiClient = {
    ...api,
    mode: "demo",
    getWatchFolder: vi.fn(async () => null),
  } as TauriApi;
  const dialogs = {
    watch: vi.fn(async () => null),
    importBackup: vi.fn(async () => null),
    exportBackup: vi.fn(async () => null),
  };
  renderSettings(apiClient, dialogs);

  expect(screen.getByRole("heading", { name: "桌面黑洞" })).toBeVisible();
  expect(screen.getByLabelText("黑洞尺寸")).toHaveAttribute("step", "4");
});
