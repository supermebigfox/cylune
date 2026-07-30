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
  await api.discardPendingJob("job-mask");

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
  expect(invoke).toHaveBeenNthCalledWith(4, "discard_pending_job", {
    jobId: "job-mask",
  });
});

it("forwards typed project history commands with their exact camelCase payloads", async () => {
  const invoke = vi.fn(async () => ({}));
  const api = createTauriApi(invoke);

  await api.listPrintProjects("pending");
  await api.getPrintProject("project-1");
  await api.importPrintProject("/prints/mask.3mf");
  await api.discardProject("project-1");
  await api.skipPlate("plate-2");
  await api.confirmNewProject("hash-1", "/prints/mask.3mf");
  await api.takePendingNavigation();

  expect(invoke).toHaveBeenNthCalledWith(1, "list_print_projects", {
    filter: "pending",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "get_print_project", {
    projectId: "project-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "import_print_project", {
    path: "/prints/mask.3mf",
  });
  expect(invoke).toHaveBeenNthCalledWith(4, "discard_project", {
    projectId: "project-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(5, "skip_plate", {
    plateId: "plate-2",
  });
  expect(invoke).toHaveBeenNthCalledWith(6, "confirm_new_project", {
    sourceHash: "hash-1",
    sourcePath: "/prints/mask.3mf",
  });
  expect(invoke).toHaveBeenNthCalledWith(7, "take_pending_navigation");
});

it("demo history keeps a two-plate project tied to existing spool identities", async () => {
  const api = createTauriApi(undefined, {});

  const [project] = await api.listPrintProjects("pending");
  const detail = await api.getPrintProject(project.project_id);

  expect(project.plate_count).toBe(2);
  expect(project.plates).toEqual([
    expect.objectContaining({
      plate_id: "demo-mask-plate-1",
      thumbnail_url: "/demo/plates/mask-1.png",
      status: "pending_mapping",
    }),
    expect.objectContaining({
      plate_id: "demo-mask-plate-2",
      thumbnail_url: "/demo/plates/mask-2.png",
      status: "ready",
    }),
  ]);
  expect(detail.plates).toEqual(project.plates);
  expect((await api.importPrintProject("/prints/mask.3mf")).plates[0].filaments[0]
    .candidate_spool_ids).toContain("white-01");
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
