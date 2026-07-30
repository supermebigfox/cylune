import { invoke as tauriInvoke } from "@tauri-apps/api/core";

export type SpoolStatus = "available" | "assigned" | "empty" | "archived";
export type Confidence = "exact" | "estimated" | "needs_confirmation";
export type ImportState =
  | "new"
  | "existing_pending"
  | "new_print_confirmation_required";
export type PetMode = "real" | "lite";
export type PetFps = "auto" | "fps30" | "fps60";
export type PetVisualStyle = "gargantua" | "fusion";
export interface PetSettings {
  mode: PetMode;
  visual_style: PetVisualStyle;
  size: number;
  fps: PetFps;
  visible: boolean;
  x: number | null;
  y: number | null;
  display_id: number | null;
  effective_mode: PetMode;
  permission: "unavailable" | "not_determined" | "denied" | "restart_required" | "granted";
  fallback_reason: string | null;
}
export type PetSettingsPatch = Partial<PetSettings> & { reset_position?: boolean };

export interface Spool {
  spool_id: string;
  display_name: string;
  preset_id: string | null;
  preset_base: string | null;
  catalog_id: string | null;
  brand: string;
  material: string;
  series: string;
  color_name: string | null;
  color_code: string | null;
  color_hex: string;
  color_hexes: string[];
  remaining_grams: number;
  status: SpoolStatus;
}

export interface NewSpool {
  display_name: string;
  preset_id?: string | null;
  preset_base?: string | null;
  catalog_id?: string | null;
  brand: string;
  material: string;
  series: string;
  color_name?: string | null;
  color_code?: string | null;
  color_hex: string;
  color_hexes?: string[];
  remaining_grams: number;
}

export interface SlotView {
  slot_number: 1 | 2 | 3 | 4;
  spool_id: string | null;
  spool: Spool | null;
}

export interface SlotAssignment {
  slot_number: 1 | 2 | 3 | 4;
  spool_id: string | null;
}

export interface FilamentProfile {
  tool: number;
  preset_id: string;
  brand: string;
  material: string;
  series: string;
  color_hex: string;
  diameter_mm: number;
  density_g_cm3: number;
}

export interface FilamentPreview {
  tool: number;
  profile: FilamentProfile;
  total_grams: number;
  candidate_spool_ids: string[];
  suggested_spool_id: string | null;
  confidence: Confidence;
}

export interface ImportPreview {
  job_id: string;
  source_hash: string;
  source_file_name: string;
  filaments: FilamentPreview[];
  max_layer: number;
  state: ImportState;
}

export type HistoryFilter = "pending" | "history";
export type PlateStatus =
  | "pending_mapping"
  | "ready"
  | "success"
  | "failed"
  | "cancelled"
  | "estimated"
  | "skipped";

export interface PrintPlateSummary {
  plate_id: string;
  project_id: string;
  plate_index: number;
  display_name: string | null;
  thumbnail_asset_id: string | null;
  thumbnail_url: string | null;
  estimated_seconds: number | null;
  max_layer: number;
  status: PlateStatus;
}

export interface PrintProjectSummary {
  project_id: string;
  source_file_name: string;
  imported_at: string;
  plate_count: number;
  total_estimated_seconds: number | null;
  cover_asset_id: string | null;
  cover_url: string | null;
  plates: PrintPlateSummary[];
}

export interface PrintProjectDetail {
  project_id: string;
  source_hash: string;
  source_file_name: string;
  source_path: string | null;
  imported_at: string;
  plate_count: number;
  total_estimated_seconds: number | null;
  cover_asset_id: string | null;
  cover_url: string | null;
  plates: PrintPlateSummary[];
}

export interface ImportPlatePreview {
  plate_id: string;
  job_id: string;
  plate_index: number;
  thumbnail_url: string | null;
  estimated_seconds: number | null;
  max_layer: number;
  filaments: FilamentPreview[];
  status: PlateStatus;
}

export interface ImportProjectPreview {
  project_id: string;
  source_hash: string;
  source_file_name: string;
  imported_at: string;
  plates: ImportPlatePreview[];
  state: ImportState;
}

export interface PendingNavigationTarget {
  project_id: string | null;
  plate_id: string | null;
  job_id: string;
}

export interface ToolMapping {
  tool: number;
  spool_id: string;
}

export type JobOutcome =
  | { kind: "success" }
  | { kind: "failed"; stop_layer: number }
  | { kind: "cancelled"; stop_layer: number }
  | { kind: "estimated"; progress_percent: number };

export interface Consumption {
  spool_id: string;
  grams: number;
  confidence: Confidence;
  slot_number: number | null;
}

export interface SettlementResult {
  job_id: string;
  outcome: JobOutcome;
  settlement_version: number;
  selected_layer: number | null;
  confidence: Confidence;
  consumption: Consumption[];
}

export interface ReversalResult {
  job_id: string;
  settlement_version: number;
  already_reversed: boolean;
  restored: Consumption[];
}

type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export interface TauriApi {
  readonly mode: "tauri" | "demo";
  createSpool(spool: NewSpool): Promise<string>;
  mountSpool(slotNumber: number, spoolId: string): Promise<void>;
  unmountSlot(slotNumber: number): Promise<void>;
  moveSpool(spoolId: string, destinationSlot: number): Promise<void>;
  calibrateSpool(spoolId: string, grams: number): Promise<void>;
  archiveSpool(spoolId: string): Promise<void>;
  listSpools(): Promise<Spool[]>;
  listSlots(): Promise<SlotAssignment[]>;
  importPrintFile(path: string): Promise<ImportPreview>;
  confirmJobMapping(jobId: string, mappings: ToolMapping[]): Promise<void>;
  confirmNewPrint(sourceHash: string): Promise<ImportPreview>;
  discardPendingJob(jobId: string): Promise<void>;
  getJobPreview?(jobId: string): Promise<ImportPreview>;
  listPrintProjects(filter: HistoryFilter): Promise<PrintProjectSummary[]>;
  getPrintProject(projectId: string): Promise<PrintProjectDetail>;
  importPrintProject(path: string): Promise<ImportProjectPreview>;
  discardProject(projectId: string): Promise<void>;
  skipPlate(plateId: string): Promise<void>;
  confirmNewProject(sourceHash: string, sourcePath: string): Promise<ImportProjectPreview>;
  takePendingNavigation(): Promise<PendingNavigationTarget | null>;
  settleJob(jobId: string, outcome: JobOutcome): Promise<SettlementResult>;
  reverseSettlement(jobId: string): Promise<ReversalResult>;
  exportBackup?(path: string): Promise<string>;
  importBackup?(path: string): Promise<string>;
  setWatchFolder?(path: string | null): Promise<string | null>;
  getWatchFolder?(): Promise<string | null>;
  openMain?(): Promise<void>;
  openJobInMain?(jobId: string): Promise<void>;
  takePendingJob?(): Promise<string | null>;
  getPetSettings?(): Promise<PetSettings>;
  setPetSettings?(patch: PetSettingsPatch): Promise<PetSettings>;
}

export const demoSpools: Spool[] = [
  ["white-01", "玉白 PLA", "#FFFEFC", 782.6, "assigned"],
  ["red-01", "热烈红 PLA", "#FE3D36", 463.8, "assigned"],
  ["blue-01", "钴蓝 PLA", "#1C4EBB", 721.3, "assigned"],
  ["yellow-01", "柠檬黄 PLA", "#FFFD0D", 136.5, "available"],
  ["black-01", "曜石黑 PLA #A", "#252733", 612.4, "available"],
  ["black-02", "曜石黑 PLA #B", "#252733", 88.7, "available"],
].map(([id, name, color, grams, status]) => ({
  spool_id: String(id),
  display_name: String(name),
  preset_id: "Bambu PLA Basic @BBL A1",
  preset_base: null,
  catalog_id: null,
  brand: "Bambu Lab",
  material: "PLA",
  series: "Basic",
  color_name: null,
  color_code: null,
  color_hex: String(color),
  color_hexes: [String(color)],
  remaining_grams: Number(grams),
  status: status as SpoolStatus,
}));

export const demoSlots: SlotAssignment[] = [
  { slot_number: 1, spool_id: "white-01" },
  { slot_number: 2, spool_id: "red-01" },
  { slot_number: 3, spool_id: "blue-01" },
  { slot_number: 4, spool_id: null },
];

export const demoPreview: ImportPreview = {
  job_id: "demo-mask-job",
  source_hash: "demo-mask-hash",
  source_file_name: "萨莫面具-布莱克.gcode.3mf",
  max_layer: 186,
  state: "new",
  filaments: demoSpools.slice(0, 4).map((spool, tool) => ({
    tool,
    profile: {
      tool,
      preset_id: "Bambu PLA Basic @BBL A1",
      brand: spool.brand,
      material: spool.material,
      series: spool.series,
      color_hex: spool.color_hex,
      diameter_mm: 1.75,
      density_g_cm3: 1.26,
    },
    total_grams: [26.4, 8.7, 11.2, 4.3][tool],
    candidate_spool_ids: [spool.spool_id],
    suggested_spool_id: spool.spool_id,
    confidence: "exact",
  })),
};

const demoProjectId = "demo-mask-project";
const demoProjectSourceHash = "demo-mask-project-hash";
const demoProjectImportedAt = "2026-07-30T04:00:00Z";
const demoProjectPlates: PrintPlateSummary[] = [
  {
    plate_id: "demo-mask-plate-1",
    project_id: demoProjectId,
    plate_index: 1,
    display_name: "面具前片",
    thumbnail_asset_id: "demo-mask-plate-1-thumbnail",
    thumbnail_url: "/demo/plates/mask-1.png",
    estimated_seconds: 5400,
    max_layer: 186,
    status: "pending_mapping",
  },
  {
    plate_id: "demo-mask-plate-2",
    project_id: demoProjectId,
    plate_index: 2,
    display_name: "面具后片",
    thumbnail_asset_id: "demo-mask-plate-2-thumbnail",
    thumbnail_url: "/demo/plates/mask-2.png",
    estimated_seconds: 4200,
    max_layer: 154,
    status: "ready",
  },
];

function demoApi(): TauriApi {
  let spools = demoSpools.map((spool) => ({ ...spool }));
  let slots = demoSlots.map((slot) => ({ ...slot }));
  let projectPlates = demoProjectPlates.map((plate) => ({ ...plate }));
  let projectDiscarded = false;
  let pet: PetSettings = {
    mode: "lite", visual_style: "gargantua", size: 220, fps: "auto", visible: false, x: null, y: null,
    display_id: null, effective_mode: "lite", permission: "unavailable",
    fallback_reason: null,
  };
  const refreshDemoStatuses = () => {
    const mounted = new Set(slots.flatMap((slot) => slot.spool_id ? [slot.spool_id] : []));
    spools = spools.map((spool) => ({
      ...spool,
      status: spool.remaining_grams <= 0 ? "empty" : mounted.has(spool.spool_id) ? "assigned" : "available",
    }));
  };
  const projectSummary = (): PrintProjectSummary => ({
    project_id: demoProjectId,
    source_file_name: demoPreview.source_file_name,
    imported_at: demoProjectImportedAt,
    plate_count: projectPlates.length,
    total_estimated_seconds: projectPlates.reduce(
      (total, plate) => total + (plate.estimated_seconds ?? 0),
      0,
    ),
    cover_asset_id: "demo-mask-cover",
    cover_url: "/demo/plates/mask-cover.png",
    plates: projectPlates.map((plate) => ({ ...plate })),
  });
  const projectDetail = (): PrintProjectDetail => ({
    ...projectSummary(),
    source_hash: demoProjectSourceHash,
    source_path: "/demo/萨莫面具-布莱克.gcode.3mf",
  });
  const projectPreview = (path?: string): ImportProjectPreview => ({
    project_id: demoProjectId,
    source_hash: demoProjectSourceHash,
    source_file_name: path?.split(/[\\/]/).pop() || demoPreview.source_file_name,
    imported_at: demoProjectImportedAt,
    plates: projectPlates.map((plate, index) => ({
      plate_id: plate.plate_id,
      job_id: "demo-mask-job-" + String(index + 1),
      plate_index: plate.plate_index,
      thumbnail_url: plate.thumbnail_url,
      estimated_seconds: plate.estimated_seconds,
      max_layer: plate.max_layer,
      filaments: demoPreview.filaments
        .slice(index * 2, index * 2 + 2)
        .map((filament) => ({
          ...filament,
          profile: { ...filament.profile },
          candidate_spool_ids: [...filament.candidate_spool_ids],
        })),
      status: plate.status,
    })),
    state: "new",
  });
  return {
    mode: "demo",
    async createSpool(input) {
      const id = `demo-${spools.length + 1}`;
      const color_hexes = input.color_hexes?.length ? input.color_hexes : [input.color_hex];
      spools = [...spools, {
        ...input,
        preset_id: input.preset_id ?? null,
        preset_base: input.preset_base ?? null,
        catalog_id: input.catalog_id ?? null,
        color_name: input.color_name ?? null,
        color_code: input.color_code ?? null,
        color_hexes,
        spool_id: id,
        status: input.remaining_grams > 0 ? "available" : "empty",
      }];
      return id;
    },
    async mountSpool(slotNumber, spoolId) {
      if (slots.some((slot) => slot.spool_id === spoolId)) throw { code: "slot_conflict" };
      slots = slots.map((slot) => slot.slot_number === slotNumber ? { ...slot, spool_id: spoolId } : slot);
      refreshDemoStatuses();
    },
    async unmountSlot(slotNumber) {
      slots = slots.map((slot) => slot.slot_number === slotNumber ? { ...slot, spool_id: null } : slot);
      refreshDemoStatuses();
    },
    async moveSpool(spoolId, destinationSlot) {
      const source = slots.find((slot) => slot.spool_id === spoolId);
      const destination = slots.find((slot) => slot.slot_number === destinationSlot);
      if (!source || !destination) throw { code: "slot_conflict" };
      const displaced = destination.spool_id;
      slots = slots.map((slot) => slot.slot_number === source.slot_number
        ? { ...slot, spool_id: displaced }
        : slot.slot_number === destinationSlot
          ? { ...slot, spool_id: spoolId }
          : slot);
      refreshDemoStatuses();
    },
    async calibrateSpool(spoolId, grams) {
      spools = spools.map((spool) => spool.spool_id === spoolId ? { ...spool, remaining_grams: grams, status: grams > 0 ? spool.status : "empty" } : spool);
    },
    async archiveSpool(spoolId) {
      spools = spools.filter((spool) => spool.spool_id !== spoolId);
    },
    async listSpools() { return spools.map((spool) => ({ ...spool })); },
    async listSlots() { return slots.map((slot) => ({ ...slot })); },
    async importPrintFile(path) { return { ...demoPreview, source_file_name: path.split(/[\\/]/).pop() || demoPreview.source_file_name }; },
    async confirmJobMapping() {},
    async confirmNewPrint() { return { ...demoPreview, job_id: "demo-mask-job-2" }; },
    async discardPendingJob() {},
    async listPrintProjects(filter) {
      if (projectDiscarded) return [];
      const isPending = projectPlates.some((plate) =>
        plate.status === "pending_mapping" || plate.status === "ready",
      );
      return filter === "pending" ? (isPending ? [projectSummary()] : []) : (isPending ? [] : [projectSummary()]);
    },
    async getPrintProject(projectId) {
      if (projectDiscarded || projectId !== demoProjectId) throw { code: "invalid_job" };
      return projectDetail();
    },
    async importPrintProject(path) { return projectPreview(path); },
    async discardProject() { projectDiscarded = true; },
    async skipPlate(plateId) {
      projectPlates = projectPlates.map((plate) =>
        plate.plate_id === plateId ? { ...plate, status: "skipped" } : plate,
      );
    },
    async confirmNewProject(_sourceHash, sourcePath) { return projectPreview(sourcePath); },
    async takePendingNavigation() {
      const plate = projectPlates.find((item) =>
        item.status === "pending_mapping" || item.status === "ready",
      );
      return plate ? {
        project_id: demoProjectId,
        plate_id: plate.plate_id,
        job_id: "demo-mask-job-" + String(plate.plate_index),
      } : null;
    },
    async settleJob(jobId, outcome) {
      return { job_id: jobId, outcome, settlement_version: 1, selected_layer: null, confidence: outcome.kind === "estimated" ? "estimated" : "exact", consumption: [] };
    },
    async reverseSettlement(jobId) { return { job_id: jobId, settlement_version: 1, already_reversed: false, restored: [] }; },
    async getPetSettings() { return { ...pet }; },
    async setPetSettings(patch) {
      const { reset_position, ...settings } = patch;
      pet = { ...pet, ...settings, ...(reset_position ? { x: null, y: null, display_id: null } : {}) };
      return { ...pet };
    },
  };
}

function commandApi(invoke: Invoke): TauriApi {
  const call = <T,>(command: string, args?: Record<string, unknown>) => invoke(command, args) as Promise<T>;
  return {
    mode: "tauri",
    createSpool: (spool) => call<string>("create_spool", { spool }),
    mountSpool: (slotNumber, spoolId) => call<void>("mount_spool", { slotNumber, spoolId }),
    unmountSlot: (slotNumber) => call<void>("unmount_slot", { slotNumber }),
    moveSpool: (spoolId, destinationSlot) => call<void>("move_spool", { spoolId, destinationSlot }),
    calibrateSpool: (spoolId, grams) => call<void>("calibrate_spool", { spoolId, grams }),
    archiveSpool: (spoolId) => call<void>("archive_spool", { spoolId }),
    listSpools: () => call<Spool[]>("list_spools", undefined),
    listSlots: () => call<SlotAssignment[]>("list_slots", undefined),
    importPrintFile: (path) => call<ImportPreview>("import_print_file", { path }),
    confirmJobMapping: (jobId, mappings) => call<void>("confirm_job_mapping", { jobId, mappings }),
    confirmNewPrint: (sourceHash) => call<ImportPreview>("confirm_new_print", { sourceHash }),
    discardPendingJob: (jobId) => call<void>("discard_pending_job", { jobId }),
    getJobPreview: (jobId) => call<ImportPreview>("get_job_preview", { jobId }),
    listPrintProjects: (filter) => call<PrintProjectSummary[]>("list_print_projects", { filter }),
    getPrintProject: (projectId) => call<PrintProjectDetail>("get_print_project", { projectId }),
    importPrintProject: (path) => call<ImportProjectPreview>("import_print_project", { path }),
    discardProject: (projectId) => call<void>("discard_project", { projectId }),
    skipPlate: (plateId) => call<void>("skip_plate", { plateId }),
    confirmNewProject: (sourceHash, sourcePath) =>
      call<ImportProjectPreview>("confirm_new_project", { sourceHash, sourcePath }),
    takePendingNavigation: () =>
      invoke("take_pending_navigation") as Promise<PendingNavigationTarget | null>,
    settleJob: (jobId, outcome) => call<SettlementResult>("settle_job", { jobId, outcome }),
    reverseSettlement: (jobId) => call<ReversalResult>("reverse_settlement", { jobId }),
    exportBackup: (path) => call<string>("export_backup", { path }),
    importBackup: (path) => call<string>("import_backup", { path }),
    setWatchFolder: (path) => call<string | null>("set_watch_folder", { path }),
    getWatchFolder: () => call<string | null>("get_watch_folder"),
    openMain: () => call<void>("open_main"),
    openJobInMain: (jobId) => call<void>("open_job_in_main", { jobId }),
    takePendingJob: () => call<string | null>("take_pending_job"),
    getPetSettings: () => call<PetSettings>("get_pet_settings"),
    setPetSettings: (patch) => call<PetSettings>("set_pet_settings", { patch }),
  };
}

export function createTauriApi(
  invoke?: Invoke,
  host: Record<string, unknown> = globalThis as Record<string, unknown>,
): TauriApi {
  if (invoke) return commandApi(invoke);
  if ("__TAURI_INTERNALS__" in host) return commandApi(tauriInvoke as Invoke);
  return demoApi();
}

export const api = createTauriApi();
