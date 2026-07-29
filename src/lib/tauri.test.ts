import { expect, it, vi } from "vitest";
import { createTauriApi, type NewSpool, type PetSettings } from "./tauri";

const pet: PetSettings = {
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

it("passes Rust command names and snake-case payloads through the typed adapter", async () => {
  const invoke = vi.fn(async () => ({ job_id: "job-1" }));
  const api = createTauriApi(invoke);

  await api.mountSpool(3, "spool-blue");
  await api.settleJob("job-1", { kind: "failed", stop_layer: 18 });
  await api.confirmNewPrint("hash-1");

  expect(invoke).toHaveBeenNthCalledWith(1, "mount_spool", {
    slotNumber: 3,
    spoolId: "spool-blue",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "settle_job", {
    jobId: "job-1",
    outcome: { kind: "failed", stop_layer: 18 },
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "confirm_new_print", {
    sourceHash: "hash-1",
  });
});

it("provides deterministic browser data without activating demo mode in Tauri", async () => {
  const browserApi = createTauriApi(undefined, {});
  const tauriInvoke = vi.fn(async () => []);
  const tauriApi = createTauriApi(tauriInvoke, { __TAURI_INTERNALS__: {} });

  expect((await browserApi.listSpools()).length).toBeGreaterThan(4);
  await tauriApi.listSpools();
  expect(tauriInvoke).toHaveBeenCalledWith("list_spools", undefined);
});

it("demo API preserves official catalog metadata", async () => {
  const client = createTauriApi(undefined, {});
  const input: NewSpool = {
    display_name: "玉石白 · PLA Basic",
    preset_id: "Bambu PLA Basic",
    preset_base: "Bambu PLA Basic",
    catalog_id: "bambu:GFA00:10100",
    brand: "Bambu Lab",
    material: "PLA",
    series: "Basic",
    color_name: "玉石白",
    color_code: "10100",
    color_hex: "#FFFFFF",
    color_hexes: ["#FFFFFF"],
    remaining_grams: 1000,
  };

  await client.createSpool(input);

  expect(await client.listSpools()).toContainEqual(
    expect.objectContaining(input),
  );
});

it("reads persisted AMS slots through the typed command boundary", async () => {
  const invoke = vi.fn(async () => [
    { slot_number: 1, spool_id: null },
    { slot_number: 2, spool_id: "spool-red" },
    { slot_number: 3, spool_id: null },
    { slot_number: 4, spool_id: null },
  ]);
  const api = createTauriApi(invoke);

  const slots = await api.listSlots();

  expect(slots[1]).toEqual({ slot_number: 2, spool_id: "spool-red" });
  expect(invoke).toHaveBeenCalledWith("list_slots", undefined);
});

it("reads and patches desktop black hole settings through the typed command boundary", async () => {
  const invoke = vi.fn(async () => pet);
  const api = createTauriApi(invoke);

  await api.getPetSettings?.();
  await api.setPetSettings?.({ mode: "real", size: 280, reset_position: true });

  expect(invoke).toHaveBeenNthCalledWith(1, "get_pet_settings", undefined);
  expect(invoke).toHaveBeenNthCalledWith(2, "set_pet_settings", {
    patch: { mode: "real", size: 280, reset_position: true },
  });
});
