import { invoke as tauriInvoke } from "@tauri-apps/api/core";

export type SpoolStatus = "available" | "assigned" | "empty" | "archived";
export type Confidence = "exact" | "estimated" | "needs_confirmation";
export type ImportState =
  | "new"
  | "existing_pending"
  | "new_print_confirmation_required";

export interface Spool {
  spool_id: string;
  display_name: string;
  preset_id: string | null;
  brand: string;
  material: string;
  series: string;
  color_hex: string;
  remaining_grams: number;
  status: SpoolStatus;
}

export interface NewSpool {
  display_name: string;
  preset_id?: string | null;
  brand: string;
  material: string;
  series: string;
  color_hex: string;
  remaining_grams: number;
}

export interface SlotView {
  slot_number: 1 | 2 | 3 | 4;
  spool_id: string | null;
  spool: Spool | null;
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
  importPrintFile(path: string): Promise<ImportPreview>;
  confirmJobMapping(jobId: string, mappings: ToolMapping[]): Promise<void>;
  confirmNewPrint(sourceHash: string): Promise<ImportPreview>;
  settleJob(jobId: string, outcome: JobOutcome): Promise<SettlementResult>;
  reverseSettlement(jobId: string): Promise<ReversalResult>;
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
  brand: "Bambu Lab",
  material: "PLA",
  series: "Basic",
  color_hex: String(color),
  remaining_grams: Number(grams),
  status: status as SpoolStatus,
}));

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

function demoApi(): TauriApi {
  let spools = demoSpools.map((spool) => ({ ...spool }));
  return {
    mode: "demo",
    async createSpool(input) {
      const id = `demo-${spools.length + 1}`;
      spools = [...spools, { ...input, preset_id: input.preset_id ?? null, spool_id: id, status: input.remaining_grams > 0 ? "available" : "empty" }];
      return id;
    },
    async mountSpool(slotNumber, spoolId) {
      void slotNumber;
      spools = spools.map((spool) => spool.spool_id === spoolId ? { ...spool, status: "assigned" } : spool);
    },
    async unmountSlot(slotNumber) { void slotNumber; },
    async moveSpool(spoolId, destinationSlot) { void spoolId; void destinationSlot; },
    async calibrateSpool(spoolId, grams) {
      spools = spools.map((spool) => spool.spool_id === spoolId ? { ...spool, remaining_grams: grams, status: grams > 0 ? spool.status : "empty" } : spool);
    },
    async archiveSpool(spoolId) {
      spools = spools.filter((spool) => spool.spool_id !== spoolId);
    },
    async listSpools() { return spools.map((spool) => ({ ...spool })); },
    async importPrintFile(path) { return { ...demoPreview, source_file_name: path.split(/[\\/]/).pop() || demoPreview.source_file_name }; },
    async confirmJobMapping() {},
    async confirmNewPrint() { return { ...demoPreview, job_id: "demo-mask-job-2" }; },
    async settleJob(jobId, outcome) {
      return { job_id: jobId, outcome, settlement_version: 1, selected_layer: null, confidence: outcome.kind === "estimated" ? "estimated" : "exact", consumption: [] };
    },
    async reverseSettlement(jobId) { return { job_id: jobId, settlement_version: 1, already_reversed: false, restored: [] }; },
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
    importPrintFile: (path) => call<ImportPreview>("import_print_file", { path }),
    confirmJobMapping: (jobId, mappings) => call<void>("confirm_job_mapping", { jobId, mappings }),
    confirmNewPrint: (sourceHash) => call<ImportPreview>("confirm_new_print", { sourceHash }),
    settleJob: (jobId, outcome) => call<SettlementResult>("settle_job", { jobId, outcome }),
    reverseSettlement: (jobId) => call<ReversalResult>("reverse_settlement", { jobId }),
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
