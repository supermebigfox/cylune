import { Disc, GearSix, House, Plus, Tray } from "@phosphor-icons/react";
import { useEffect, useMemo, useState } from "react";
import { Mark } from "./brand/Mark";
import { Home } from "./features/home/Home";
import { Job } from "./features/jobs/Job";
import { Settings } from "./features/settings/Settings";
import { Spools } from "./features/spools/Spools";
import { t, useLocale } from "./i18n";
import { api, demoPreview, demoSpools, type ImportPreview, type JobOutcome, type NewSpool, type SettlementResult, type SlotView, type Spool as SpoolData, type ToolMapping } from "./lib/tauri";
import { Theme } from "./theme/Theme";

type Page = "home" | "spools" | "jobs" | "settings";

function DesktopApp() {
  const locale = useLocale();
  const copy = (key: string) => t(key, {}, locale);
  const [page, setPage] = useState<Page>("home");
  const [spools, setSpools] = useState<SpoolData[]>(api.mode === "demo" ? demoSpools.map((spool) => ({ ...spool })) : []);
  const [preview, setPreview] = useState<ImportPreview | null>(api.mode === "demo" ? demoPreview : null);
  const [settled, setSettled] = useState(false);
  const [result, setResult] = useState<SettlementResult | null>(null);
  const [loading, setLoading] = useState(api.mode === "tauri");
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try { setSpools(await api.listSpools()); setError(null); }
    catch { setError(copy("errors.database")); }
    finally { setLoading(false); }
  };
  useEffect(() => { if (api.mode === "tauri") void refresh(); }, []);

  const slots = useMemo<SlotView[]>(() => {
    const mounted = api.mode === "demo" ? spools.filter((spool) => spool.status === "assigned") : [];
    return [1, 2, 3, 4].map((number) => ({ slot_number: number as 1 | 2 | 3 | 4, spool_id: mounted[number - 1]?.spool_id ?? null, spool: mounted[number - 1] ?? null }));
  }, [spools]);
  const slotBySpool = useMemo(() => Object.fromEntries(slots.filter((slot) => slot.spool_id).map((slot) => [slot.spool_id as string, slot.slot_number])), [slots]);
  const actions = {
    create: async (spool: NewSpool) => { await api.createSpool(spool); await refresh(); },
    calibrate: async (spoolId: string, grams: number) => { await api.calibrateSpool(spoolId, grams); await refresh(); },
    archive: async (spoolId: string) => { await api.archiveSpool(spoolId); await refresh(); },
    mount: async (spoolId: string, slot: number) => { await api.mountSpool(slot, spoolId); await refresh(); },
    map: async (jobId: string, mappings: ToolMapping[]) => api.confirmJobMapping(jobId, mappings),
    settle: async (jobId: string, outcome: JobOutcome) => { const next = await api.settleJob(jobId, outcome); setResult(next); setSettled(true); await refresh(); },
    repeat: async (sourceHash: string) => { setPreview(await api.confirmNewPrint(sourceHash)); setSettled(false); setResult(null); },
    reverse: async (jobId: string) => { await api.reverseSettlement(jobId); setSettled(false); setResult(null); await refresh(); },
  };
  const openImport = () => { if (api.mode === "demo") setPreview(demoPreview); setPage("jobs"); };
  const nav = [
    ["home", House, "nav.home"], ["spools", Disc, "nav.spools"], ["jobs", Tray, "nav.jobs"], ["settings", GearSix, "nav.settings"],
  ] as const;

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand-lockup"><Mark label={copy("brand.mark")} size={38} /><div className="brand-copy"><h1>{copy("app.name")}</h1><span>{copy("app.localMode")}</span></div></div>
      <nav aria-label={copy("common.mainNav")}>{nav.map(([id, Icon, key]) => <button className={page === id ? "active" : ""} key={id} onClick={() => setPage(id)}><Icon size={20} weight={page === id ? "fill" : "regular"} /><span>{copy(key)}</span></button>)}</nav>
      <button className="sidebar-import" onClick={openImport}><Plus size={18} weight="bold" />{copy("import.title")}</button>
      <div className="privacy-note"><span>{api.mode === "demo" ? copy("app.demoMode") : copy("app.localMode")}</span><small>{copy("app.privateNote")}</small></div>
    </aside>
    <main className="content">
      {error ? <div className="app-error" role="alert">{error}<button onClick={refresh}>{copy("common.retry")}</button></div> : null}
      {loading ? <div className="skeleton-page" aria-label={copy("common.loading")}><i /><i /><i /></div> : null}
      {page === "home" ? <Home slots={slots} spools={spools} pendingJobs={preview && !settled ? 1 : 0} onImport={openImport} /> : null}
      {page === "spools" ? <Spools spools={spools} slotBySpool={slotBySpool} onCreate={actions.create} onCalibrate={actions.calibrate} onArchive={actions.archive} onMount={actions.mount} /> : null}
      {page === "jobs" ? <Job preview={preview} spools={spools} settled={settled} result={result} onConfirmMapping={actions.map} onSettle={actions.settle} onConfirmNewPrint={actions.repeat} onReverse={actions.reverse} /> : null}
      {page === "settings" ? <Settings /> : null}
    </main>
  </div>;
}

export function App() {
  return <Theme><DesktopApp /></Theme>;
}
