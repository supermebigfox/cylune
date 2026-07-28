import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import { api, type PetSettings, type TauriApi } from "../../lib/tauri";
import { Theme } from "../../theme/Theme";
import { Pet } from "./Pet";

const defaultPet: PetSettings = {
  mode: "lite",
  visual_style: "gargantua",
  size: 220,
  fps: "auto",
  visible: true,
  x: null,
  y: null,
  display_id: null,
  effective_mode: "lite",
  permission: "unavailable",
  fallback_reason: "native_not_started",
};

beforeEach(async () => {
  await setLocale("en");
});

function petApi(overrides: Partial<PetSettings>): TauriApi {
  let current = { ...defaultPet, ...overrides };
  return {
    ...api,
    mode: "tauri",
    getPetSettings: vi.fn(async () => current),
    setPetSettings: vi.fn(async (patch) => (current = { ...current, ...patch })),
  } as TauriApi;
}

function renderPet(apiClient: TauriApi) {
  return render(<Theme><Pet apiClient={apiClient} /></Theme>);
}

it("saves mode size fps and visibility immediately", async () => {
  const api = petApi({ mode: "lite", size: 220, fps: "auto", visible: true });
  renderPet(api);
  await screen.findByRole("heading", { name: "Desktop black hole" });
  fireEvent.click(screen.getByRole("button", { name: "Real distortion" }));
  fireEvent.change(screen.getByLabelText("Black hole size"), { target: { value: "280" } });
  fireEvent.click(screen.getByRole("button", { name: "60 FPS" }));
  fireEvent.click(screen.getByRole("button", { name: "Hide black hole" }));
  await waitFor(() => {
    expect(api.setPetSettings).toHaveBeenNthCalledWith(1, { mode: "real" });
    expect(api.setPetSettings).toHaveBeenNthCalledWith(2, { size: 280 });
    expect(api.setPetSettings).toHaveBeenNthCalledWith(3, { fps: "fps60" });
    expect(api.setPetSettings).toHaveBeenNthCalledWith(4, { visible: false });
  });
});

it("offers the exact scalable size presets and both visual styles", async () => {
  const api = petApi({ mode: "lite", size: 220, fps: "auto", visible: true });
  renderPet(api);
  const slider = await screen.findByLabelText("Black hole size");
  expect(screen.getByRole("button", { name: "300 px" })).toBeVisible();
  expect(screen.getByRole("button", { name: "600 px" })).toBeVisible();
  expect(screen.getByRole("button", { name: "900 px" })).toBeVisible();
  expect(slider).toHaveAttribute("min", "120");
  expect(slider).toHaveAttribute("max", "900");
  expect(screen.getByRole("button", { name: "Gargantua" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Fusion" }));
  await waitFor(() => {
    expect(api.setPetSettings).toHaveBeenLastCalledWith({ visual_style: "fusion" });
  });
});

it("serializes rapid changes before starting the next server write", async () => {
  let finishFirst!: (value: PetSettings) => void;
  const api = petApi({});
  api.setPetSettings = vi.fn().mockImplementationOnce(() => new Promise<PetSettings>((resolve) => {
    finishFirst = resolve;
  })).mockResolvedValueOnce({ ...defaultPet, mode: "real", fps: "fps60" });
  renderPet(api);
  await screen.findByRole("heading", { name: "Desktop black hole" });
  fireEvent.click(screen.getByRole("button", { name: "Real distortion" }));
  fireEvent.click(screen.getByRole("button", { name: "60 FPS" }));

  await waitFor(() => expect(api.setPetSettings).toHaveBeenCalledTimes(1));
  finishFirst({ ...defaultPet, mode: "real" });
  await waitFor(() => expect(api.setPetSettings).toHaveBeenLastCalledWith({ fps: "fps60" }));
});

it("ignores a stale initial load after a newer save", async () => {
  let resolveLoad!: (value: PetSettings) => void;
  const api = petApi({});
  api.getPetSettings = vi.fn(() => new Promise<PetSettings>((resolve) => {
    resolveLoad = resolve;
  }));
  api.setPetSettings = vi.fn(async (patch) => ({ ...defaultPet, ...patch }));
  renderPet(api);
  await screen.findByRole("heading", { name: "Desktop black hole" });
  await waitFor(() => expect(api.getPetSettings).toHaveBeenCalledTimes(1));

  fireEvent.click(screen.getByRole("button", { name: "Hide black hole" }));
  await waitFor(() => expect(api.setPetSettings).toHaveBeenLastCalledWith({ visible: false }));
  resolveLoad(defaultPet);

  expect(await screen.findByRole("button", { name: "Show black hole" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Show black hole" }));
  await waitFor(() => expect(api.setPetSettings).toHaveBeenLastCalledWith({ visible: true }));
});

it("rolls back optimistic changes and shows a localized error when saving fails", async () => {
  const api = petApi({ mode: "lite" });
  api.setPetSettings = vi.fn(async () => {
    throw new Error("offline");
  });
  renderPet(api);
  await screen.findByRole("heading", { name: "Desktop black hole" });
  fireEvent.click(screen.getByRole("button", { name: "Real distortion" }));

  expect(screen.getByRole("button", { name: "Real distortion" })).toHaveAttribute("aria-pressed", "true");
  expect(await screen.findByRole("alert")).toHaveTextContent("Could not save desktop black hole settings");
  expect(screen.getByRole("button", { name: "Lightweight mode" })).toHaveAttribute("aria-pressed", "true");
});

it.each([
  ["native_not_started", "unavailable", "Desktop capture is not available; lightweight mode is active"],
  ["platform_unsupported", "unavailable", "Real distortion is unavailable on this platform; lightweight mode is active"],
  ["permission_not_determined", "not_determined", "Choose Real distortion to request Screen Recording access"],
  ["permission_denied", "denied", "Screen recording permission is off; lightweight mode is active"],
  ["permission_restart_required", "restart_required", "Permission changed. Restart the app."],
  ["capture_failed", "granted", "Desktop capture stopped unexpectedly; lightweight mode is still active"],
  ["metal_unavailable", "granted", "Metal is unavailable; lightweight mode is active"],
] as const)("localizes stable fallback %s instead of rendering its raw code", async (
  fallbackReason,
  permission,
  expected,
) => {
  renderPet(petApi({
    mode: "real",
    effective_mode: "lite",
    fallback_reason: fallbackReason,
    permission,
  }));

  expect(await screen.findByText(expected)).toBeVisible();
  expect(screen.queryByText(fallbackReason)).not.toBeInTheDocument();
});
