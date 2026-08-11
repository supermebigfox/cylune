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
  visible: false,
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

it("starts off and disables hide or show for fresh settings", async () => {
  renderPet(petApi({ mode: "lite", visible: false }));

  expect(await screen.findByRole("button", { name: "Turn off" }))
    .toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Show black hole" })).toBeDisabled();
  expect(screen.queryByRole("button", { name: "Lightweight mode" }))
    .not.toBeInTheDocument();
});

it("turns on atomically and unlocks visibility controls", async () => {
  const apiClient = petApi({ mode: "lite", visible: false });
  renderPet(apiClient);

  fireEvent.click(await screen.findByRole("button", { name: "Turn on" }));

  await waitFor(() => {
    expect(apiClient.setPetSettings)
      .toHaveBeenLastCalledWith({ mode: "real", visible: true });
  });
  expect(screen.getByRole("button", { name: "Hide black hole" })).toBeEnabled();
});

it("turns off atomically and disables visibility controls", async () => {
  const apiClient = petApi({ mode: "real", visible: true });
  renderPet(apiClient);

  fireEvent.click(await screen.findByRole("button", { name: "Turn off" }));

  await waitFor(() => {
    expect(apiClient.setPetSettings)
      .toHaveBeenLastCalledWith({ mode: "lite", visible: false });
  });
  expect(screen.getByRole("button", { name: "Show black hole" })).toBeDisabled();
});

it("saves enabled state size fps and visibility immediately", async () => {
  const api = petApi({ mode: "lite", size: 220, fps: "auto", visible: false });
  renderPet(api);
  await screen.findByRole("heading", { name: "Desktop black hole" });
  fireEvent.click(screen.getByRole("button", { name: "Turn on" }));
  fireEvent.change(screen.getByLabelText("Black hole size"), { target: { value: "280" } });
  fireEvent.click(screen.getByRole("button", { name: "60 FPS" }));
  fireEvent.click(screen.getByRole("button", { name: "Hide black hole" }));
  await waitFor(() => {
    expect(api.setPetSettings).toHaveBeenNthCalledWith(1, { mode: "real", visible: true });
    expect(api.setPetSettings).toHaveBeenNthCalledWith(2, { size: 280 });
    expect(api.setPetSettings).toHaveBeenNthCalledWith(3, { fps: "fps60" });
    expect(api.setPetSettings).toHaveBeenNthCalledWith(4, { visible: false });
  });
});

it("offers the exact scalable size presets and both visual styles", async () => {
  const api = petApi({ mode: "lite", size: 220, fps: "auto", visible: false });
  renderPet(api);
  const slider = await screen.findByLabelText("Black hole size");
  expect(screen.getByRole("button", { name: "300 px" })).toBeVisible();
  expect(screen.getByRole("button", { name: "600 px" })).toBeVisible();
  expect(screen.getByRole("button", { name: "900 px" })).toBeVisible();
  expect(slider).toHaveAttribute("min", "120");
  expect(slider).toHaveAttribute("max", "900");
  expect(screen.getByRole("button", { name: "Gargantua" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "300 px" }));
  fireEvent.click(screen.getByRole("button", { name: "600 px" }));
  fireEvent.click(screen.getByRole("button", { name: "900 px" }));
  fireEvent.click(screen.getByRole("button", { name: "Fusion" }));
  await waitFor(() => {
    expect(api.setPetSettings).toHaveBeenNthCalledWith(1, { size: 300 });
    expect(api.setPetSettings).toHaveBeenNthCalledWith(2, { size: 600 });
    expect(api.setPetSettings).toHaveBeenNthCalledWith(3, { size: 900 });
    expect(api.setPetSettings).toHaveBeenLastCalledWith({ visual_style: "fusion" });
  });
});

it("serializes rapid changes before starting the next server write", async () => {
  let finishFirst!: (value: PetSettings) => void;
  const api = petApi({});
  api.setPetSettings = vi.fn().mockImplementationOnce(() => new Promise<PetSettings>((resolve) => {
    finishFirst = resolve;
  })).mockResolvedValueOnce({ ...defaultPet, mode: "real", fps: "fps60", visible: true });
  renderPet(api);
  await screen.findByRole("heading", { name: "Desktop black hole" });
  fireEvent.click(screen.getByRole("button", { name: "Turn on" }));
  fireEvent.click(screen.getByRole("button", { name: "60 FPS" }));

  await waitFor(() => expect(api.setPetSettings).toHaveBeenCalledTimes(1));
  finishFirst({ ...defaultPet, mode: "real", visible: true });
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

  fireEvent.click(screen.getByRole("button", { name: "Turn on" }));
  await waitFor(() => expect(api.setPetSettings)
    .toHaveBeenLastCalledWith({ mode: "real", visible: true }));
  resolveLoad(defaultPet);

  expect(await screen.findByRole("button", { name: "Hide black hole" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Hide black hole" }));
  await waitFor(() => expect(api.setPetSettings).toHaveBeenLastCalledWith({ visible: false }));
});

it("rolls back optimistic changes and shows a localized error when saving fails", async () => {
  const api = petApi({ mode: "lite" });
  api.setPetSettings = vi.fn(async () => {
    throw new Error("offline");
  });
  renderPet(api);
  await screen.findByRole("heading", { name: "Desktop black hole" });
  fireEvent.click(screen.getByRole("button", { name: "Turn on" }));

  expect(screen.getByRole("button", { name: "Turn on" })).toHaveAttribute("aria-pressed", "true");
  expect(await screen.findByRole("alert")).toHaveTextContent("Could not save desktop black hole settings");
  expect(screen.getByRole("button", { name: "Turn off" })).toHaveAttribute("aria-pressed", "true");
});

it.each([
  ["native_not_started", "unavailable", "Desktop capture is unavailable; the black hole is using a compatible background"],
  ["platform_unsupported", "unavailable", "Live distortion is unavailable on this platform; the black hole is using a compatible background"],
  ["permission_not_determined", "not_determined", "Turn on the black hole to request Screen Recording access"],
  ["permission_denied", "denied", "Screen Recording access is off; the black hole is using a compatible background"],
  ["permission_restart_required", "restart_required", "Permission changed. Restart the app."],
  ["capture_failed", "granted", "Desktop capture stopped unexpectedly; the black hole is using a compatible background"],
  ["metal_unavailable", "granted", "Metal is unavailable; the black hole is using a compatible background"],
  ["direct3d_unavailable", "granted", "Direct3D is unavailable; the black hole is using a compatible background"],
  ["presentation_unavailable", "granted", "The black hole window cannot be shown right now. Turn it off, then on again."],
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

it.each([
  [
    "denied",
    "permission_denied",
    "Screen Recording access is off; the black hole is using a compatible background",
  ],
  [
    "restart_required",
    "permission_restart_required",
    "Permission changed. Restart the app.",
  ],
] as const)("keeps Real requested while %s capture uses the explicit Lite fallback", async (
  permission,
  fallbackReason,
  instruction,
) => {
  renderPet(petApi({
    mode: "real",
    effective_mode: "lite",
    permission,
    fallback_reason: fallbackReason,
  }));

  expect(await screen.findByText(instruction)).toBeVisible();
  expect(screen.getByRole("button", { name: "Turn on" }))
    .toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Turn off" }))
    .toHaveAttribute("aria-pressed", "false");
});

it.each([
  ["zh-CN", "开启黑洞", "关闭黑洞"],
  ["zh-TW", "開啟黑洞", "關閉黑洞"],
  ["en", "Turn on", "Turn off"],
] as const)("renders the black hole power control in %s", async (locale, on, off) => {
  await setLocale(locale);
  renderPet(petApi({ mode: "lite", visible: false }));

  expect(await screen.findByRole("button", { name: on })).toBeVisible();
  expect(screen.getByRole("button", { name: off })).toBeVisible();
});
