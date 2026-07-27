import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import { api, type PetSettings, type TauriApi } from "../../lib/tauri";
import { Theme } from "../../theme/Theme";
import { Pet } from "./Pet";

const defaultPet: PetSettings = {
  mode: "lite",
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
  await waitFor(() => expect(api.setPetSettings).toHaveBeenLastCalledWith(
    expect.objectContaining({ fps: "fps60" }),
  ));
});

it("uses the exact 120 to 360 size range", async () => {
  const api = petApi({ mode: "lite", size: 220, fps: "auto", visible: true });
  renderPet(api);
  const slider = await screen.findByLabelText("Black hole size");
  expect(slider).toHaveAttribute("min", "120");
  expect(slider).toHaveAttribute("max", "360");
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
