import { CubeFocus, Disc, GearSix, House, Plus, Printer, Tray } from "@phosphor-icons/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Mark } from "./brand/Mark";
import { Home } from "./features/home/Home";
import { History } from "./features/jobs/History";
import { Project } from "./features/jobs/Project";
import { MainNav, type MainNavItem } from "./features/nav/Nav";
import { Printers } from "./features/printers/Printers";
import {
  Slice,
  type SliceEventName,
  type SliceTask as SliceUiTask,
} from "./features/slice/Slice";
import { Settings } from "./features/settings/Settings";
import { Spools } from "./features/spools/Spools";
import type { CreateSpoolResult } from "./features/spools/Add";
import { t, useLocale } from "./i18n";
import { pickThreeMf } from "./lib/dialog";
import {
  api,
  demoSlots,
  demoSpools,
  type ImportPreview,
  type ImportProjectPreview,
  type JobOutcome,
  type NewSpool,
  type PrintProjectDetail,
  type PrintProjectSummary,
  type SettlementResult,
  type SlotAssignment,
  type SlotView,
  type Spool as SpoolData,
  type TauriApi,
  type ToolMapping,
} from "./lib/tauri";
import { Theme } from "./theme/Theme";

type Page = "home" | "spools" | "printers" | "slice" | "jobs" | "settings";
type DesktopEventName =
  | "open-job"
  | "open-project"
  | "confirm-new-project"
  | "watch-import"
  | "open-slice"
  | "open-overview"
  | "pet-import-error"
  | SliceEventName;
type DesktopEventSubscriber = (
  name: DesktopEventName,
  handler: (payload: unknown) => void,
) => Promise<() => void>;
type ProjectNavigation = { project_id: string; plate_id?: string | null };
type RepeatCandidate = {
  project_id: string;
  source_hash: string;
  source_path: string;
};
type WatchImportEvent = {
  ok: boolean;
  project_id: string | null;
  plate_id: string | null;
  state: ImportProjectPreview["state"] | null;
  source_hash: string | null;
  source_path: string | null;
  code: string | null;
};

const pendingPlateStatuses = new Set(["pending_mapping", "ready"]);

const subscribeDesktopEvent: DesktopEventSubscriber = async (name, handler) => {
  if (!("__TAURI_INTERNALS__" in globalThis)) return () => undefined;
  return listen(name, (event) => handler(event.payload));
};

const stableErrorCodes = new Set([
  "archived_spool", "database", "duplicate_job", "file_not_stable",
  "insufficient_filament", "invalid_file", "invalid_job", "invalid_mapping",
  "invalid_slot", "io", "slot_conflict", "unknown_gcode", "unsliced_project",
  "standalone_gcode_profiles_required",
]);

function errorCode(error: unknown) {
  let candidate = error;
  if (typeof candidate === "string") {
    try { candidate = JSON.parse(candidate); }
    catch { return "io"; }
  }
  if (candidate && typeof candidate === "object" && "code" in candidate) {
    const code = String((candidate as { code: unknown }).code);
    if (stableErrorCodes.has(code)) return code;
  }
  return "io";
}

function isPendingProject(project: PrintProjectSummary | PrintProjectDetail) {
  return project.plates.some((plate) => pendingPlateStatuses.has(plate.status));
}

function platePreview(
  project: PrintProjectDetail | null,
  preview: ImportProjectPreview | null,
  plateId: string | null,
): { plateId: string; value: ImportPreview } | null {
  if (!project || !preview || !plateId) return null;
  const plate = preview.plates.find((item) => item.plate_id === plateId);
  if (!plate) return null;
  return {
    plateId,
    value: {
      job_id: plate.job_id,
      source_hash: preview.source_hash,
      source_file_name: preview.source_file_name,
      max_layer: plate.max_layer,
      filaments: plate.filaments,
      state: preview.state,
    },
  };
}

function selectedPlateId(
  project: PrintProjectDetail,
  requestedPlateId?: string | null,
) {
  if (requestedPlateId && project.plates.some((plate) => plate.plate_id === requestedPlateId)) {
    return requestedPlateId;
  }
  return project.plates.find((plate) => pendingPlateStatuses.has(plate.status))?.plate_id
    ?? project.plates[0]?.plate_id
    ?? null;
}

export function DesktopApp({
  apiClient = api,
  pickFile = pickThreeMf,
  subscribeEvent = subscribeDesktopEvent,
}: {
  apiClient?: TauriApi;
  pickFile?: (filterName: string) => Promise<string | null>;
  subscribeEvent?: DesktopEventSubscriber;
}) {
  const locale = useLocale();
  const copy = (key: string, values: Record<string, string | number> = {}) => t(key, values, locale);
  const [page, setPage] = useState<Page>("home");
  const [spools, setSpools] = useState<SpoolData[]>(apiClient.mode === "demo" ? demoSpools.map((spool) => ({ ...spool })) : []);
  const [slotAssignments, setSlotAssignments] = useState<SlotAssignment[]>(apiClient.mode === "demo" ? demoSlots.map((slot) => ({ ...slot })) : [1, 2, 3, 4].map((slot_number) => ({ slot_number: slot_number as 1 | 2 | 3 | 4, spool_id: null })));
  const [pendingProjects, setPendingProjects] = useState<PrintProjectSummary[]>([]);
  const [historyProjects, setHistoryProjects] = useState<PrintProjectSummary[]>([]);
  const [activeProject, setActiveProject] = useState<PrintProjectDetail | null>(null);
  const [activePreview, setActivePreview] = useState<ImportProjectPreview | null>(null);
  const [selectedPlate, setSelectedPlate] = useState<string | null>(null);
  const [plateResults, setPlateResults] = useState<Record<string, SettlementResult>>({});
  const [queuedNavigation, setQueuedNavigation] = useState<ProjectNavigation | null>(null);
  const [repeatCandidate, setRepeatCandidate] = useState<RepeatCandidate | null>(null);
  const [sliceMounted, setSliceMounted] = useState(false);
  const [sliceInput, setSliceInput] = useState<{ path: string; nonce: number } | null>(null);
  const [slicePrinter, setSlicePrinter] = useState<{ id: string; nonce: number } | null>(null);
  const [sliceTask, setSliceTask] = useState<SliceUiTask | null>(null);
  const [sliceFormLocked, setSliceFormLocked] = useState(false);
  const [loading, setLoading] = useState(apiClient.mode === "tauri");
  const [error, setError] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const busyRef = useRef(false);
  const projectRequest = useRef(0);
  const activeProjectRef = useRef<PrintProjectDetail | null>(null);

  const loadInventory = useCallback(async () => {
    const [nextSpools, nextSlots] = await Promise.all([apiClient.listSpools(), apiClient.listSlots()]);
    setSpools(nextSpools);
    setSlotAssignments(nextSlots);
  }, [apiClient]);
  const loadProjects = useCallback(async () => {
    const [pending, history] = await Promise.all([
      apiClient.listPrintProjects("pending"),
      apiClient.listPrintProjects("history"),
    ]);
    setPendingProjects(pending);
    setHistoryProjects(history);
  }, [apiClient]);
  const loadProject = useCallback(async (
    projectId: string,
    requestedPlateId?: string | null,
    knownPreview?: ImportProjectPreview | null,
  ) => {
    const request = ++projectRequest.current;
    const [project, preview] = await Promise.all([
      apiClient.getPrintProject(projectId),
      knownPreview === undefined ? apiClient.getProjectPreview?.(projectId) ?? Promise.resolve(null) : Promise.resolve(knownPreview),
    ]);
    if (request !== projectRequest.current) return;
    if (activeProjectRef.current?.project_id !== project.project_id) {
      setPlateResults({});
    }
    activeProjectRef.current = project;
    setActiveProject(project);
    setActivePreview(preview);
    setSelectedPlate(selectedPlateId(project, requestedPlateId));
    setPage("jobs");
  }, [apiClient]);
  const refresh = useCallback(async () => {
    try {
      await Promise.all([loadInventory(), loadProjects()]);
      setError(null);
    }
    catch { setError(t("errors.database", {}, locale)); }
    finally { setLoading(false); }
  }, [loadInventory, loadProjects, locale]);
  const refreshActiveProject = useCallback(async (requestedPlateId?: string | null) => {
    if (!activeProject) return;
    await Promise.all([
      loadProject(activeProject.project_id, requestedPlateId ?? selectedPlate),
      loadProjects(),
    ]);
  }, [activeProject, loadProject, loadProjects, selectedPlate]);

  useEffect(() => { void refresh(); }, [refresh]);
  useEffect(() => {
    if (apiClient.mode !== "tauri") return;
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const openProject = (projectId: string, plateId?: string | null) => {
      void loadProject(projectId, plateId).catch(() => {
        if (!disposed) setError(t("errors.invalid_job", {}, locale));
      });
    };
    const queueOrOpenProject = (target: ProjectNavigation) => {
      const currentProject = activeProjectRef.current;
      if (currentProject && isPendingProject(currentProject)) {
        setQueuedNavigation(target);
        void loadProjects().catch(() => undefined);
      } else {
        openProject(target.project_id, target.plate_id);
      }
    };
    void Promise.all([
      subscribeEvent("open-project", (payload) => {
        const target = payload as Partial<ProjectNavigation>;
        if (typeof target?.project_id === "string") {
          setRepeatCandidate(null);
          openProject(target.project_id, target.plate_id);
        }
      }),
      subscribeEvent("confirm-new-project", (payload) => {
        const target = payload as Partial<ProjectNavigation & RepeatCandidate>;
        if (
          typeof target?.project_id === "string"
          && typeof target.source_hash === "string"
          && typeof target.source_path === "string"
        ) {
          setRepeatCandidate({
            project_id: target.project_id,
            source_hash: target.source_hash,
            source_path: target.source_path,
          });
          openProject(target.project_id, target.plate_id);
        }
      }),
      subscribeEvent("open-job", () => {
        void apiClient.takePendingNavigation().then((target) => {
          if (target?.project_id) queueOrOpenProject({
            project_id: target.project_id,
            plate_id: target.plate_id,
          });
        }).catch(() => undefined);
      }),
      subscribeEvent("watch-import", (payload) => {
        const event = payload as Partial<WatchImportEvent>;
        if (event.ok && typeof event.project_id === "string") {
          if (
            event.state === "new_print_confirmation_required"
            && typeof event.source_hash === "string"
            && typeof event.source_path === "string"
          ) {
            setRepeatCandidate({
              project_id: event.project_id,
              source_hash: event.source_hash,
              source_path: event.source_path,
            });
          } else {
            setRepeatCandidate(null);
          }
          queueOrOpenProject({ project_id: event.project_id, plate_id: event.plate_id });
        } else if (!event.ok) {
          setError(t(`errors.${errorCode({ code: event.code })}`, {}, locale));
        }
      }),
      subscribeEvent("open-overview", () => setPage("home")),
      subscribeEvent("open-slice", (payload) => {
        if (typeof payload !== "string" || !payload) return;
        setError(null);
        setSliceMounted(true);
        setSliceInput((current) => ({ path: payload, nonce: (current?.nonce ?? 0) + 1 }));
        setPage("slice");
      }),
      subscribeEvent("pet-import-error", (payload) => {
        setError(t(`errors.${errorCode({ code: payload })}`, {}, locale));
      }),
    ]).then((stops) => {
      if (disposed) stops.forEach((stop) => stop());
      else {
        unlisteners.push(...stops);
        void apiClient.takePendingNavigation().then((target) => {
          if (!disposed && target?.project_id) openProject(target.project_id, target.plate_id);
        }).catch(() => undefined);
      }
    });
    return () => {
      disposed = true;
      unlisteners.forEach((stop) => stop());
    };
  }, [activeProject, apiClient, loadProject, loadProjects, locale, subscribeEvent]);

  useEffect(() => {
    if (!selectedPlate || !apiClient.getSettlementResult) return;
    const plate = activeProject?.plates.find((item) => item.plate_id === selectedPlate);
    const preview = activePreview?.plates.find((item) => item.plate_id === selectedPlate);
    if (!plate || !preview || pendingPlateStatuses.has(plate.status) || plate.status === "skipped") return;
    let disposed = false;
    void apiClient.getSettlementResult(preview.job_id).then((result) => {
      if (!disposed && result) {
        setPlateResults((current) => ({ ...current, [selectedPlate]: result }));
      }
    }).catch(() => {
      if (!disposed) setError(t("errors.invalid_job", {}, locale));
    });
    return () => { disposed = true; };
  }, [activePreview, activeProject, apiClient, locale, selectedPlate]);

  const slots = useMemo<SlotView[]>(() => {
    return slotAssignments.map((slot) => ({ ...slot, spool: spools.find((spool) => spool.spool_id === slot.spool_id) ?? null }));
  }, [slotAssignments, spools]);
  const slotBySpool = useMemo(() => Object.fromEntries(slots.filter((slot) => slot.spool_id).map((slot) => [slot.spool_id as string, slot.slot_number])), [slots]);
  const pendingPlates = useMemo(() => pendingProjects.reduce(
    (total, project) => total + project.plates.filter((plate) => pendingPlateStatuses.has(plate.status)).length,
    0,
  ), [pendingProjects]);
  const runAction = async (
    key: string,
    operation: () => Promise<void>,
    onFailure?: (message: string) => void,
  ) => {
    if (busyRef.current) return false;
    busyRef.current = true;
    setBusyAction(key);
    setError(null);
    try {
      await operation();
      return true;
    }
    catch (actionError) {
      const message = copy(`errors.${errorCode(actionError)}`);
      setError(message);
      onFailure?.(message);
      return false;
    }
    finally {
      busyRef.current = false;
      setBusyAction(null);
    }
  };
  const actions = {
    create: async (spool: NewSpool): Promise<CreateSpoolResult> => {
      let failureMessage = copy("errors.io");
      const succeeded = await runAction("create", async () => {
        await apiClient.createSpool(spool);
        await loadInventory();
      }, (message) => { failureMessage = message; });
      return succeeded ? { ok: true } : { ok: false, error: failureMessage };
    },
    calibrate: (spoolId: string, grams: number) => runAction("calibrate", async () => { await apiClient.calibrateSpool(spoolId, grams); await loadInventory(); }),
    archive: (spoolId: string) => runAction("archive", async () => { await apiClient.archiveSpool(spoolId); await loadInventory(); }),
    mount: (spoolId: string, slot: number) => runAction("mount", async () => { await apiClient.mountSpool(slot, spoolId); await loadInventory(); }),
    unmount: (slot: number) => runAction("unmount", async () => { await apiClient.unmountSlot(slot); await loadInventory(); }),
    move: (spoolId: string, slot: number) => runAction("move", async () => { await apiClient.moveSpool(spoolId, slot); await loadInventory(); }),
    map: (jobId: string, mappings: ToolMapping[]) => runAction("map", async () => {
      await apiClient.confirmJobMapping(jobId, mappings);
      await refreshActiveProject();
    }),
    settle: (jobId: string, outcome: JobOutcome) => runAction("settle", async () => {
      const result = await apiClient.settleJob(jobId, outcome);
      const settledPlateId = activePreview?.plates.find((plate) => plate.job_id === jobId)?.plate_id
        ?? selectedPlate;
      if (settledPlateId) {
        setPlateResults((current) => ({ ...current, [settledPlateId]: result }));
      }
      await Promise.all([loadInventory(), refreshActiveProject()]);
    }),
    repeat: (sourceHash: string) => runAction("repeat", async () => {
      if (!repeatCandidate || repeatCandidate.source_hash !== sourceHash) {
        throw { code: "invalid_job" };
      }
      const next = await apiClient.confirmNewProject(sourceHash, repeatCandidate.source_path);
      setRepeatCandidate(null);
      await Promise.all([loadInventory(), loadProjects(), loadProject(next.project_id, next.plates[0]?.plate_id, next)]);
    }),
    discard: (_jobId: string) => runAction("discard", async () => {
      if (!activeProject) throw { code: "invalid_job" };
      await apiClient.discardProject(activeProject.project_id);
      activeProjectRef.current = null;
      setActiveProject(null);
      setActivePreview(null);
      setSelectedPlate(null);
      setPlateResults({});
      setRepeatCandidate(null);
      await Promise.all([loadInventory(), loadProjects()]);
    }),
    skip: (plateId: string) => runAction("skip", async () => {
      await apiClient.skipPlate(plateId);
      await refreshActiveProject(plateId);
    }),
    reverse: (jobId: string) => runAction("reverse", async () => {
      await apiClient.reverseSettlement(jobId);
      const reversedPlateId = activePreview?.plates.find((plate) => plate.job_id === jobId)?.plate_id
        ?? selectedPlate;
      if (reversedPlateId) setPlateResults((current) => current[reversedPlateId]
        ? { ...current, [reversedPlateId]: { ...current[reversedPlateId], reversed: true } }
        : current);
      await Promise.all([loadInventory(), refreshActiveProject()]);
    }),
  };
  const importSlicedProject = useCallback(async (path: string) => {
    const next = await apiClient.importPrintProject(path);
    setRepeatCandidate(next.state === "new_print_confirmation_required" ? {
      project_id: next.project_id,
      source_hash: next.source_hash,
      source_path: path,
    } : null);
    await Promise.all([loadInventory(), loadProjects(), loadProject(next.project_id, next.plates[0]?.plate_id, next)]);
  }, [apiClient, loadInventory, loadProject, loadProjects]);
  const openImport = () => runAction("import", async () => {
    const path = await pickFile(copy("import.filterName"));
    if (!path) return;
    const inspection = await apiClient.inspect3mf(path);
    if (inspection.kind === "unsliced") {
      setSliceMounted(true);
      setSliceInput((current) => ({ path, nonce: (current?.nonce ?? 0) + 1 }));
      setPage("slice");
      return;
    }
    await importSlicedProject(path);
  });
  const importFromSlice = (path: string) => {
    void runAction("import", () => importSlicedProject(path));
  };
  const openCompletedSliceProject = useCallback((projectId: string) => {
    setRepeatCandidate(null);
    void Promise.all([
      loadInventory(),
      loadProjects(),
      loadProject(projectId),
    ]).catch(() => setError(t("errors.invalid_job", {}, locale)));
  }, [loadInventory, loadProject, loadProjects, locale]);
  const navigate = useCallback((target: Page) => {
    if (target === "slice") setSliceMounted(true);
    setPage(target);
  }, []);
  const openQueuedProject = () => {
    if (!queuedNavigation) return;
    const target = queuedNavigation;
    setQueuedNavigation(null);
    void loadProject(target.project_id, target.plate_id).catch(() => setError(copy("errors.invalid_job")));
  };
  const selectPlate = (plateId: string) => {
    setSelectedPlate(plateId);
  };
  const busy = busyAction !== null;
  const hasSettledPlate = activeProject?.plates.some((plate) => !pendingPlateStatuses.has(plate.status)) ?? false;
  const activePlatePreview = platePreview(activeProject, activePreview, selectedPlate);
  const activeResult = selectedPlate && plateResults[selectedPlate]
    ? { plateId: selectedPlate, value: plateResults[selectedPlate] }
    : null;
  const activeMappings = useMemo(() => Object.fromEntries(
    activePreview?.plates
      .find((plate) => plate.plate_id === selectedPlate)
      ?.mappings?.map((mapping) => [mapping.tool, mapping.spool_id])
      ?? [],
  ), [activePreview, selectedPlate]);
  const displayedSlicePercent = sliceTask?.state === "running"
    && typeof sliceTask.percent === "number"
    && Number.isFinite(sliceTask.percent)
      ? Math.round(sliceTask.percent)
      : null;
  const sliceBadge = sliceFormLocked
    ? displayedSlicePercent === null ? "1" : `${displayedSlicePercent}%`
    : undefined;
  const navItems: readonly MainNavItem<Page>[] = [
    { id: "home", label: copy("nav.home"), icon: <House size={20} weight={page === "home" ? "fill" : "regular"} /> },
    { id: "spools", label: copy("nav.spools"), icon: <Disc size={20} weight={page === "spools" ? "fill" : "regular"} /> },
    { id: "printers", label: copy("nav.printers"), icon: <Printer size={20} weight={page === "printers" ? "fill" : "regular"} /> },
    {
      id: "slice",
      label: copy("nav.slice"),
      icon: <CubeFocus size={20} weight={page === "slice" ? "fill" : "regular"} />,
      badge: sliceBadge,
      badgeLabel: sliceBadge ? copy("nav.sliceActive", { status: sliceBadge }) : undefined,
    },
    { id: "jobs", label: copy("nav.jobs"), icon: <Tray size={20} weight={page === "jobs" ? "fill" : "regular"} /> },
  ];
  const settingsItem: MainNavItem<Page> = {
    id: "settings",
    label: copy("nav.settings"),
    icon: <GearSix size={20} weight={page === "settings" ? "fill" : "regular"} />,
  };
  const importLocked = busy || sliceFormLocked;

  return <div className="app-shell">
    <MainNav
      activeId={page}
      items={navItems}
      settingsItem={settingsItem}
      onNavigate={navigate}
      brand={{
        mark: <Mark label={copy("brand.mark")} size={38} />,
        name: copy("app.name"),
        subtitle: copy("app.localMode"),
      }}
      importAction={{
        label: busyAction === "import" ? copy("import.reading") : copy("import.title"),
        icon: <Plus size={18} weight="bold" />,
        disabled: importLocked,
        onClick: openImport,
      }}
      privacy={{
        title: apiClient.mode === "demo" ? copy("app.demoMode") : copy("app.localMode"),
        description: copy("app.privateNote"),
      }}
      ariaLabel={copy("common.mainNav")}
      menuLabel={copy("nav.openMenu")}
      closeMenuLabel={copy("nav.closeMenu")}
    />
    <main className="content">
      {error ? <div className="app-error" role="alert">{error}<button onClick={() => void refresh()}>{copy("common.retry")}</button></div> : null}
      {queuedNavigation ? <div className="app-error watch-ready" role="status"><span>{copy("settings.watchedJobReady")}</span><button onClick={openQueuedProject}>{copy("settings.openWatchedJob")}</button></div> : null}
      {loading ? <div className="skeleton-page" aria-label={copy("common.loading")}><i /><i /><i /></div> : null}
      {page === "home" ? <Home slots={slots} spools={spools} pendingJobs={pendingPlates} busy={importLocked} importing={busyAction === "import"} onImport={openImport} /> : null}
      {page === "spools" ? <Spools spools={spools} slotBySpool={slotBySpool} busy={busy} onCreate={actions.create} onCalibrate={actions.calibrate} onArchive={actions.archive} onMount={actions.mount} onUnmount={actions.unmount} onMove={actions.move} /> : null}
      {page === "printers" ? <Printers apiClient={apiClient} onStartSlice={(printer) => {
        setSliceMounted(true);
        setSlicePrinter((current) => ({ id: printer.printer_id, nonce: (current?.nonce ?? 0) + 1 }));
        setPage("slice");
      }} /> : null}
      {sliceMounted ? <div hidden={page !== "slice"}>
        <Slice
          api={apiClient}
          pickInput={() => pickFile(copy("import.filterName"))}
          subscribeEvent={subscribeEvent}
          onProjectComplete={openCompletedSliceProject}
          onSlicedFile={importFromSlice}
          initialInputPath={sliceInput?.path ?? null}
          initialInputNonce={sliceInput?.nonce ?? 0}
          preferredPrinterId={slicePrinter?.id ?? null}
          preferredPrinterNonce={slicePrinter?.nonce ?? 0}
          active={page === "slice"}
          activeTask={sliceTask}
          onTaskChange={setSliceTask}
          onFormLockChange={setSliceFormLocked}
        />
      </div> : null}
      {page === "jobs" ? activeProject ? <Project project={activeProject} selectedPlateId={selectedPlate} preview={activePlatePreview} initialMappings={activeMappings} result={activeResult} repeatSourceHash={repeatCandidate?.project_id === activeProject.project_id ? repeatCandidate.source_hash : null} spools={spools} busy={busy} canDiscardProject={!hasSettledPlate} onBackToHistory={() => { activeProjectRef.current = null; setActiveProject(null); setActivePreview(null); setSelectedPlate(null); setRepeatCandidate(null); }} onSelectPlate={selectPlate} onConfirmMapping={actions.map} onSettle={actions.settle} onConfirmNewPrint={actions.repeat} onDiscard={actions.discard} onSkipPlate={actions.skip} onReverse={actions.reverse} /> : <History pending={pendingProjects} history={historyProjects} onOpenProject={(projectId) => { setRepeatCandidate(null); void loadProject(projectId).catch(() => setError(copy("errors.invalid_job"))); }} /> : null}
      {page === "settings" ? <Settings apiClient={apiClient} onRestored={() => void refresh()} /> : null}
    </main>
  </div>;
}

export function App() {
  return <Theme><DesktopApp /></Theme>;
}
