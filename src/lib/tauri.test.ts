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
  await api.getProjectPreview?.("project-1");
  await api.importPrintProject("/prints/mask.3mf");
  await api.discardProject("project-1");
  await api.skipPlate("plate-2");
  await api.confirmNewProject("hash-1", "/prints/mask.3mf");
  await api.takePendingNavigation();
  await api.getSettlementResult?.("job-1");

  expect(invoke).toHaveBeenNthCalledWith(1, "list_print_projects", {
    filter: "pending",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "get_print_project", {
    projectId: "project-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "get_project_preview", {
    projectId: "project-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(4, "import_print_project", {
    path: "/prints/mask.3mf",
  });
  expect(invoke).toHaveBeenNthCalledWith(5, "discard_project", {
    projectId: "project-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(6, "skip_plate", {
    plateId: "plate-2",
  });
  expect(invoke).toHaveBeenNthCalledWith(7, "confirm_new_project", {
    sourceHash: "hash-1",
    sourcePath: "/prints/mask.3mf",
  });
  expect(invoke).toHaveBeenNthCalledWith(8, "take_pending_navigation");
  expect(invoke).toHaveBeenNthCalledWith(9, "get_settlement_result", {
    jobId: "job-1",
  });
});

it("demo history keeps a two-plate project tied to existing spool identities", async () => {
  const api = createTauriApi(undefined, {});

  const [project] = await api.listPrintProjects("pending");
  const detail = await api.getPrintProject(project.project_id);

  expect(project.plate_count).toBe(2);
  expect(project.cover_url).toMatch(/mask(?:-[\w]+)?\.png$/);
  expect(project.plates).toEqual([
    expect.objectContaining({
      plate_id: "demo-mask-plate-1",
      thumbnail_url: project.cover_url,
      status: "pending_mapping",
      filaments: expect.arrayContaining([expect.objectContaining({
        profile: expect.objectContaining({ color_hex: "#FFFEFC" }),
        total_grams: 26.4,
      })]),
    }),
    expect.objectContaining({
      plate_id: "demo-mask-plate-2",
      thumbnail_url: project.cover_url,
      status: "ready",
    }),
  ]);
  expect(detail.plates).toEqual(project.plates);
  expect((await api.importPrintProject("/prints/mask.3mf")).plates[0].filaments[0]
    .candidate_spool_ids).toContain("white-01");
});

it("demo settlement deducts each mapped spool and reversal restores it", async () => {
  const api = createTauriApi(undefined, {});
  const before = await api.listSpools();
  const blueBefore = before.find((spool) => spool.spool_id === "blue-01")!;
  const yellowBefore = before.find((spool) => spool.spool_id === "yellow-01")!;

  const settled = await api.settleJob("demo-mask-job-2", { kind: "success" });

  expect(settled.consumption).toEqual([
    expect.objectContaining({ spool_id: "blue-01", grams: 11.2, slot_number: 3 }),
    expect.objectContaining({ spool_id: "yellow-01", grams: 4.3, slot_number: null }),
  ]);
  const after = await api.listSpools();
  expect(after.find((spool) => spool.spool_id === "blue-01")!.remaining_grams)
    .toBeCloseTo(blueBefore.remaining_grams - 11.2);
  expect(after.find((spool) => spool.spool_id === "yellow-01")!.remaining_grams)
    .toBeCloseTo(yellowBefore.remaining_grams - 4.3);

  const reversal = await api.reverseSettlement("demo-mask-job-2");
  expect(reversal.restored).toEqual(settled.consumption);
  const restored = await api.listSpools();
  expect(restored.find((spool) => spool.spool_id === "blue-01")!.remaining_grams)
    .toBeCloseTo(blueBefore.remaining_grams);
  expect(restored.find((spool) => spool.spool_id === "yellow-01")!.remaining_grams)
    .toBeCloseTo(yellowBefore.remaining_grams);
});

it("demo getPrintProject returns only its active project and keeps confirmation available", async () => {
  const api = createTauriApi(undefined, {});
  const [project] = await api.listPrintProjects("pending");

  await expect(api.getPrintProject(project.project_id)).resolves.toMatchObject({
    project_id: project.project_id,
  });
  await expect(api.confirmNewProject("hash-1", "/prints/mask.3mf")).resolves.toMatchObject({
    project_id: project.project_id,
  });
});

it("demo getPrintProject rejects an unknown project ID", async () => {
  const api = createTauriApi(undefined, {});

  await expect(api.getPrintProject("unknown-project")).rejects.toEqual({
    code: "invalid_job",
  });
});

it("demo getPrintProject rejects a discarded project and removes it from pending", async () => {
  const api = createTauriApi(undefined, {});
  const [project] = await api.listPrintProjects("pending");

  await api.discardProject(project.project_id);

  await expect(api.getPrintProject(project.project_id)).rejects.toEqual({
    code: "invalid_job",
  });
  await expect(api.listPrintProjects("pending")).resolves.toEqual([]);
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

it("forwards printer library commands with complete typed payloads", async () => {
  const invoke = vi.fn(async () => []);
  const api = createTauriApi(invoke);
  const draft = {
    printer_id: undefined,
    display_name: "我的 P2S",
    model_key: "Bambu Lab P2S",
    nozzle_diameter: 0.4,
    default_plate: "Supertack Plate",
    ams_kind: "ams",
    is_default: true,
  };

  await api.listAvailablePrinters();
  await api.listSavedPrinters();
  await api.savePrinter(draft);
  await api.setDefaultPrinter("printer-p2s");
  await api.deletePrinter("printer-p2s");

  expect(invoke).toHaveBeenNthCalledWith(1, "list_available_printers", undefined);
  expect(invoke).toHaveBeenNthCalledWith(2, "list_saved_printers", undefined);
  expect(invoke).toHaveBeenNthCalledWith(3, "save_printer", { printer: draft });
  expect(invoke).toHaveBeenNthCalledWith(4, "set_default_printer", {
    printerId: "printer-p2s",
  });
  expect(invoke).toHaveBeenNthCalledWith(5, "delete_printer", {
    printerId: "printer-p2s",
  });
});

it("starts private metadata slicing without exposing output or profile paths", async () => {
  const invoke = vi.fn(async () => ({}));
  const api = createTauriApi(invoke);
  const request = {
    input_path: "/Users/robin/Desktop/月球灯.3mf",
    printer_id: "printer-p2s",
    process_key: "standard-020",
    plate_key: "supertack",
    plate_override: false,
    infill_density: null,
    support_enabled: null,
    filaments: [
      { tool: 0, preset_key: "pla-basic-white", override_project_settings: false },
      { tool: 1, preset_key: "pla-basic-black", override_project_settings: true },
    ],
    confirm_printer_mismatch: false,
    preserve_project_settings: true,
  };

  await api.inspect3mf("/Users/robin/Desktop/月球灯.3mf");
  await api.listSlicePresets("printer-p2s");
  await api.startSlice(request);
  await api.getSliceTask("slice-task-1");
  await api.cancelSlice("slice-task-1");
  await api.openInBambuStudio("/Users/robin/Desktop/月球灯.3mf");

  expect(invoke).toHaveBeenNthCalledWith(1, "inspect_3mf", {
    path: "/Users/robin/Desktop/月球灯.3mf",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "list_slice_presets", {
    printerId: "printer-p2s",
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "start_slice", { request });
  expect(invoke).toHaveBeenNthCalledWith(4, "get_slice_task", {
    taskId: "slice-task-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(5, "cancel_slice", {
    taskId: "slice-task-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(6, "open_in_bambu_studio", {
    path: "/Users/robin/Desktop/月球灯.3mf",
  });
  expect(JSON.stringify(request)).not.toContain("/profiles/");
  expect(request).not.toHaveProperty("output_path");
  expect(request).not.toHaveProperty("destination");
  expect(request).not.toHaveProperty("allow_overwrite");
});
