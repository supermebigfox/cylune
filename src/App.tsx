import { Disc, GearSix, House, Plus, Tray } from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Mark } from "./brand/Mark";
import { Home } from "./features/home/Home";
import { Job } from "./features/jobs/Job";
import { Settings } from "./features/settings/Settings";
import { Spools } from "./features/spools/Spools";
import type { CreateSpoolResult } from "./features/spools/Add";
import { t, useLocale } from "./i18n";
import { pickSliced3mf } from "./lib/dialog";
import { api, demoPreview, demoSlots, demoSpools, type ImportPreview, type JobOutcome, type NewSpool, type SettlementResult, type SlotAssignment, type SlotView, type Spool as SpoolData, type TauriApi, type ToolMapping } from "./lib/tauri";
import { Theme } from "./theme/Theme";
import { listen } from "@tauri-apps/api/event";

type Page = "home" | "spools" | "jobs" | "settings";
type DesktopEventName =
  | "open-job"
  | "watch-import"
  | "open-overview"
  | "pet-import-error";
type DesktopEventSubscriber = (
  name: DesktopEventName,
  handler: (payload: unknown) => void,
) => Promise<() => void>;
type WatchImportEvent = {
  ok: boolean;
  job_id: string | null;
  code: string | null;
};

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

export function DesktopApp({ apiClient = api, pickFile = pickSliced3mf, subscribeEvent = subscribeDesktopEvent }: {
  apiClient?: TauriApi;
  pickFile?: (filterName: string) => Promise<string | null>;
  subscribeEvent?: DesktopEventSubscriber;
}) {
  const locale = useLocale();
  const copy = (key: string) => t(key, {}, locale);
  const [page, setPage] = useState<Page>("home");
  const [spools, setSpools] = useState<SpoolData[]>(apiClient.mode === "demo" ? demoSpools.map((spool) => ({ ...spool })) : []);
  const [slotAssignments, setSlotAssignments] = useState<SlotAssignment[]>(apiClient.mode === "demo" ? demoSlots.map((slot) => ({ ...slot })) : [1, 2, 3, 4].map((slot_number) => ({ slot_number: slot_number as 1 | 2 | 3 | 4, spool_id: null })));
  const [preview, setPreview] = useState<ImportPreview | null>(apiClient.mode === "demo" ? demoPreview : null);
  const [queuedPreview, setQueuedPreview] = useState<ImportPreview | null>(null);
  const [settled, setSettled] = useState(false);
  const [result, setResult] = useState<SettlementResult | null>(null);
  const [loading, setLoading] = useState(apiClient.mode === "tauri");
  const [error, setError] = useState<string | null>(null);
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const busyRef = useRef(false);
  const hasPendingPreview = useRef(Boolean(preview));

  const loadInventory = async () => {
    const [nextSpools, nextSlots] = await Promise.all([apiClient.listSpools(), apiClient.listSlots()]);
    setSpools(nextSpools);
    setSlotAssignments(nextSlots);
  };
  const refresh = async () => {
    try {
      await loadInventory();
      setError(null);
    }
    catch { setError(copy("errors.database")); }
    finally { setLoading(false); }
  };
  useEffect(() => { if (apiClient.mode === "tauri") void refresh(); }, []);
  useEffect(() => {
    hasPendingPreview.current = Boolean(preview && !settled);
  }, [preview, settled]);
  useEffect(() => {
    if (apiClient.mode !== "tauri" || !apiClient.getJobPreview) return;
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    const openJob = async (jobId: string, source: "navigation" | "watch") => {
      try {
        const next = await apiClient.getJobPreview!(jobId);
        if (disposed) return;
        if (source === "watch" && hasPendingPreview.current) {
          setQueuedPreview(next);
          return;
        }
        setPreview(next);
        setSettled(false);
        setResult(null);
        if (source === "navigation") setPage("jobs");
      } catch {
        if (!disposed) setError(copy("errors.invalid_job"));
      }
    };
    void Promise.all([
      subscribeEvent("open-job", (payload) => {
        if (typeof payload === "string") {
          void openJob(payload, "navigation").then(() => apiClient.takePendingJob?.());
        }
      }),
      subscribeEvent("watch-import", (payload) => {
        const event = payload as Partial<WatchImportEvent>;
        if (event.ok && typeof event.job_id === "string") {
          void openJob(event.job_id, "watch");
        } else {
          setError(copy(`errors.${errorCode({ code: event.code })}`));
        }
      }),
      subscribeEvent("open-overview", () => {
        setPage("home");
      }),
      subscribeEvent("pet-import-error", (payload) => {
        setError(copy(`errors.${errorCode({ code: payload })}`));
      }),
    ]).then((stops) => {
      if (disposed) stops.forEach((stop) => stop());
      else {
        unlisteners.push(...stops);
        void apiClient.takePendingJob?.().then((jobId) => {
          if (!disposed && jobId) void openJob(jobId, "navigation");
        });
      }
    });
    return () => {
      disposed = true;
      unlisteners.forEach((stop) => stop());
    };
  }, [apiClient, locale, subscribeEvent]);

  const slots = useMemo<SlotView[]>(() => {
    return slotAssignments.map((slot) => ({ ...slot, spool: spools.find((spool) => spool.spool_id === slot.spool_id) ?? null }));
  }, [slotAssignments, spools]);
  const slotBySpool = useMemo(() => Object.fromEntries(slots.filter((slot) => slot.spool_id).map((slot) => [slot.spool_id as string, slot.slot_number])), [slots]);
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
      const succeeded = await runAction(
        "create",
        async () => {
          await apiClient.createSpool(spool);
          await loadInventory();
        },
        (message) => {
          failureMessage = message;
        },
      );
      return succeeded
        ? { ok: true }
        : { ok: false, error: failureMessage };
    },
    calibrate: (spoolId: string, grams: number) => runAction("calibrate", async () => { await apiClient.calibrateSpool(spoolId, grams); await loadInventory(); }),
    archive: (spoolId: string) => runAction("archive", async () => { await apiClient.archiveSpool(spoolId); await loadInventory(); }),
    mount: (spoolId: string, slot: number) => runAction("mount", async () => { await apiClient.mountSpool(slot, spoolId); await loadInventory(); }),
    unmount: (slot: number) => runAction("unmount", async () => { await apiClient.unmountSlot(slot); await loadInventory(); }),
    move: (spoolId: string, slot: number) => runAction("move", async () => { await apiClient.moveSpool(spoolId, slot); await loadInventory(); }),
    map: (jobId: string, mappings: ToolMapping[]) => runAction("map", () => apiClient.confirmJobMapping(jobId, mappings)),
    settle: (jobId: string, outcome: JobOutcome) => runAction("settle", async () => { const next = await apiClient.settleJob(jobId, outcome); setResult(next); setSettled(true); await loadInventory(); }),
    repeat: (sourceHash: string) => runAction("repeat", async () => { setPreview(await apiClient.confirmNewPrint(sourceHash)); setSettled(false); setResult(null); }),
    reverse: (jobId: string) => runAction("reverse", async () => { await apiClient.reverseSettlement(jobId); setSettled(false); setResult(null); await loadInventory(); }),
  };
  const openImport = () => runAction("import", async () => {
    if (apiClient.mode === "demo") {
      setPreview(demoPreview);
      setSettled(false);
      setResult(null);
      setPage("jobs");
      return;
    }
    const path = await pickFile(copy("import.filterName"));
    if (!path) return;
    setPreview(await apiClient.importPrintFile(path));
    setSettled(false);
    setResult(null);
    setPage("jobs");
  });
  const openQueuedPreview = () => {
    if (!queuedPreview) return;
    setPreview(queuedPreview);
    setQueuedPreview(null);
    setSettled(false);
    setResult(null);
    setPage("jobs");
  };
  const busy = busyAction !== null;
  const nav = [
    ["home", House, "nav.home"], ["spools", Disc, "nav.spools"], ["jobs", Tray, "nav.jobs"], ["settings", GearSix, "nav.settings"],
  ] as const;

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand-lockup"><Mark label={copy("brand.mark")} size={38} /><div className="brand-copy"><h1>{copy("app.name")}</h1><span>{copy("app.localMode")}</span></div></div>
      <nav aria-label={copy("common.mainNav")}>{nav.map(([id, Icon, key]) => <button disabled={busy} className={page === id ? "active" : ""} key={id} onClick={() => setPage(id)}><Icon size={20} weight={page === id ? "fill" : "regular"} /><span>{copy(key)}</span></button>)}</nav>
      <button className="sidebar-import" disabled={busy} onClick={openImport}><Plus size={18} weight="bold" />{busyAction === "import" ? copy("import.reading") : copy("import.title")}</button>
      <div className="privacy-note"><span>{apiClient.mode === "demo" ? copy("app.demoMode") : copy("app.localMode")}</span><small>{copy("app.privateNote")}</small></div>
    </aside>
    <main className="content">
      {error ? <div className="app-error" role="alert">{error}<button onClick={refresh}>{copy("common.retry")}</button></div> : null}
      {queuedPreview ? <div className="app-error watch-ready" role="status"><span>{copy("settings.watchedJobReady")}</span><button onClick={openQueuedPreview}>{copy("settings.openWatchedJob")}</button></div> : null}
      {loading ? <div className="skeleton-page" aria-label={copy("common.loading")}><i /><i /><i /></div> : null}
      {page === "home" ? <Home slots={slots} spools={spools} pendingJobs={(preview && !settled ? 1 : 0) + (queuedPreview ? 1 : 0)} busy={busy} importing={busyAction === "import"} onImport={openImport} /> : null}
      {page === "spools" ? <Spools spools={spools} slotBySpool={slotBySpool} busy={busy} onCreate={actions.create} onCalibrate={actions.calibrate} onArchive={actions.archive} onMount={actions.mount} onUnmount={actions.unmount} onMove={actions.move} /> : null}
      {page === "jobs" ? <Job preview={preview} spools={spools} settled={settled} result={result} busy={busy} onConfirmMapping={actions.map} onSettle={actions.settle} onConfirmNewPrint={actions.repeat} onReverse={actions.reverse} /> : null}
      {page === "settings" ? <Settings apiClient={apiClient} onRestored={refresh} /> : null}
    </main>
  </div>;
}

export function App() {
  return <Theme><DesktopApp /></Theme>;
}
