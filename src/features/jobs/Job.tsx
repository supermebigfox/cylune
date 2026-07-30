import { ArrowCounterClockwise, CheckCircle, FileText, Trash, Warning } from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Swatch } from "../../components/Swatch";
import { t, useLocale } from "../../i18n";
import type { ImportPreview, JobOutcome, SettlementResult, Spool, ToolMapping } from "../../lib/tauri";

type OutcomeChoice = "success" | "failed" | "cancelled" | "estimated";
const emptyMappings: Record<number, string> = {};

export function Job({ preview, spools, initialMappings = emptyMappings, settled = false, result, busy = false, embedded = false, onConfirmMapping, onSettle, onConfirmNewPrint, onDiscard, onReverse }: {
  preview: ImportPreview | null;
  spools: Spool[];
  initialMappings?: Record<number, string>;
  settled?: boolean;
  result?: SettlementResult | null;
  busy?: boolean;
  embedded?: boolean;
  onConfirmMapping(jobId: string, mappings: ToolMapping[]): boolean | void | Promise<boolean | void>;
  onSettle(jobId: string, outcome: JobOutcome): boolean | void | Promise<boolean | void>;
  onConfirmNewPrint(sourceHash: string): boolean | void | Promise<boolean | void>;
  onDiscard?(jobId: string): boolean | void | Promise<boolean | void>;
  onReverse(jobId: string): boolean | void | Promise<boolean | void>;
}) {
  const locale = useLocale();
  const copy = (key: string, values: Record<string, string | number> = {}) => t(key, values, locale);
  const suggested = useMemo(() => Object.fromEntries(preview?.filaments.flatMap((filament) => filament.suggested_spool_id ? [[filament.tool, filament.suggested_spool_id]] : []) ?? []), [preview]);
  const restoredMappings = useMemo(() => ({ ...suggested, ...initialMappings }), [initialMappings, suggested]);
  const [mappings, setMappings] = useState<Record<number, string>>(restoredMappings);
  const [outcome, setOutcome] = useState<OutcomeChoice>("success");
  const [layer, setLayer] = useState("1");
  const [percent, setPercent] = useState("50");
  const [mapped, setMapped] = useState(Object.keys(initialMappings).length > 0);
  const previousJobId = useRef(preview?.job_id);

  useEffect(() => {
    if (previousJobId.current === preview?.job_id) return;
    previousJobId.current = preview?.job_id;
    setMappings(restoredMappings);
    setMapped(Object.keys(initialMappings).length > 0);
    setOutcome("success");
    setLayer("1");
    setPercent("50");
  }, [initialMappings, preview?.job_id, restoredMappings]);

  if (!preview) return <section className="page jobs-empty" aria-labelledby="jobs-title"><h1 id="jobs-title">{copy("jobs.title")}</h1><div className="empty-state"><FileText size={36} weight="duotone" /><h2>{copy("jobs.empty")}</h2><p>{copy("jobs.emptyHint")}</p></div></section>;

  const submitMappings = async () => {
    const selected = preview.filaments.map((filament) => ({ tool: filament.tool, spool_id: mappings[filament.tool] })).filter((item): item is ToolMapping => Boolean(item.spool_id));
    if (selected.length !== preview.filaments.length) return;
    const succeeded = await onConfirmMapping(preview.job_id, selected);
    if (succeeded !== false) setMapped(true);
  };
  const submitOutcome = () => {
    if (outcome === "success") return onSettle(preview.job_id, { kind: "success" });
    if (outcome === "estimated") return onSettle(preview.job_id, { kind: "estimated", progress_percent: Number(percent) });
    return onSettle(preview.job_id, { kind: outcome, stop_layer: Math.max(0, Number(layer) - 1) });
  };

  return <section
    className={embedded ? "job-workspace" : "page job-page"}
    aria-label={embedded ? copy("project.jobWorkspace") : undefined}
    aria-labelledby={embedded ? undefined : "jobs-title"}
  >
    {!embedded ? <div className="page-heading"><div><h1 id="jobs-title">{copy("jobs.title")}</h1><p>{copy("jobs.jobHint")}</p></div><span className="file-status"><CheckCircle size={18} weight="fill" />{copy("import.ready")}</span></div> : null}
    {!embedded ? <article className="job-file">
      <div className="file-icon"><FileText size={28} weight="duotone" /></div>
      <div><h2>{preview.source_file_name}</h2><p>{copy("jobs.layerCount", { count: preview.max_layer })}</p></div>
      <div className="job-total"><span>{copy("jobs.plannedTotal")}</span><strong className="data">{preview.filaments.reduce((sum, item) => sum + item.total_grams, 0).toFixed(1)} {copy("common.grams")}</strong></div>
    </article> : null}

    {preview.state === "new_print_confirmation_required" ? <div className="callout warning"><Warning size={21} weight="fill" /><div><strong>{copy("import.duplicate")}</strong><p>{copy("jobs.repeatHint")}</p></div><button className="secondary" disabled={busy} onClick={() => onConfirmNewPrint(preview.source_hash)}>{busy ? copy("common.saving") : copy("jobs.confirmNew")}</button></div> : null}

    {preview.state !== "new_print_confirmation_required" ? <div className="job-columns">
      <div className="mapping-panel">
        <div className="panel-title"><h2>{copy("import.mapping")}</h2><span>{preview.filaments.length}</span></div>
        {preview.filaments.map((filament) => {
          const exactCandidates = filament.candidate_spool_ids.map((id) => spools.find((spool) => spool.spool_id === id)).filter((spool): spool is Spool => Boolean(spool));
          const candidates = exactCandidates.length ? exactCandidates : spools.filter((spool) => spool.status !== "archived" && spool.remaining_grams > 0);
          return <fieldset className={`tool-map ${!mappings[filament.tool] ? "needs-choice" : ""}`} key={filament.tool}>
            <legend><Swatch colors={[filament.profile.color_hex]} /><span><strong>{filament.profile.preset_id}</strong><small>{copy("jobs.tool", { number: filament.tool + 1 })}</small></span><em className="tool-grams">{`${filament.total_grams.toFixed(1)} ${copy("common.grams")}`}</em></legend>
            {exactCandidates.length > 1 ? <p className="mapping-warning">{copy("jobs.multipleCandidates", { count: exactCandidates.length })}</p> : null}
            {!exactCandidates.length ? <p className="mapping-warning">{copy("jobs.mismatchChoice")}</p> : null}
            <div className="radio-list">{candidates.map((spool) => <label key={spool.spool_id}><input disabled={busy} aria-label={copy("jobs.spoolChoice", { name: spool.display_name, grams: spool.remaining_grams.toFixed(1), unit: copy("common.grams") })} type="radio" name={`tool-${filament.tool}`} checked={mappings[filament.tool] === spool.spool_id} onChange={() => { setMappings({ ...mappings, [filament.tool]: spool.spool_id }); setMapped(false); }} /><Swatch colors={spool.color_hexes?.length ? spool.color_hexes : [spool.color_hex]} /><span><strong>{spool.display_name}</strong><small className="data">{copy("jobs.spoolMeta", { grams: spool.remaining_grams.toFixed(1), unit: copy("common.grams"), id: spool.spool_id })}</small></span></label>)}</div>
            {!candidates.length ? <p className="inline-error">{copy("jobs.noCandidate")}</p> : null}
          </fieldset>;
        })}
        <button className="secondary full" disabled={busy || preview.filaments.some((item) => !mappings[item.tool])} onClick={submitMappings}>{busy ? copy("common.saving") : mapped ? copy("jobs.mappingConfirmed") : copy("jobs.confirmMapping")}</button>
      </div>

      <div className="settle-panel">
        <h2>{copy("settlement.title")}</h2>
        <fieldset className="outcomes"><legend>{copy("settlement.outcome")}</legend>
          {(["success", "failed", "cancelled", "estimated"] as OutcomeChoice[]).map((value) => { const label = copy(`settlement.${value === "success" ? "complete" : value === "estimated" ? "estimate" : value}`); return <label className={outcome === value ? "selected" : ""} key={value}><input disabled={busy} aria-label={label} type="radio" name="outcome" checked={outcome === value} onChange={() => setOutcome(value)} />{label}{value === "estimated" ? <b className="estimate-badge">{copy("common.estimated")}</b> : null}</label>; })}
        </fieldset>
        {(outcome === "failed" || outcome === "cancelled") ? <label className="field">{copy("settlement.lastLayer")}<input disabled={busy} aria-label={copy("settlement.lastLayer")} type="number" min="1" max={preview.max_layer} value={layer} onChange={(event) => setLayer(event.target.value)} /><small>{copy("settlement.layerHint", { count: preview.max_layer })}</small></label> : null}
        {outcome === "estimated" ? <label className="field">{copy("settlement.progress")}<div className="range-line"><input disabled={busy} aria-label={copy("settlement.progress")} type="range" min="1" max="99" value={percent} onChange={(event) => setPercent(event.target.value)} /><strong className="data">{copy("settlement.percentValue", { percent })}</strong></div><small>{copy("settlement.estimatedUsage")}</small></label> : null}
        <button className="primary full" disabled={busy || !mapped} onClick={submitOutcome}>{busy ? copy("common.saving") : copy("settlement.confirmDeduction")}</button>
        {!mapped ? <p className="helper">{copy("jobs.mapBeforeSettle")}</p> : null}
        {settled || result ? <div className="settled-result"><CheckCircle size={22} weight="fill" /><div><strong>{copy("jobs.settledTitle")}</strong><p>{result ? copy("jobs.settledAmount", { grams: result.consumption.reduce((sum, item) => sum + item.grams, 0).toFixed(1) }) : copy("jobs.settledHint")}</p></div></div> : null}
        {settled || result ? <button className="ghost full" disabled={busy} onClick={() => onReverse(preview.job_id)}><ArrowCounterClockwise size={17} />{busy ? copy("common.saving") : copy("jobs.reverse")}</button> : null}
        {!settled && !result && onDiscard ? <button className="ghost full" disabled={busy} onClick={() => {
          if (window.confirm(copy("jobs.discardConfirm"))) void onDiscard(preview.job_id);
        }}><Trash size={17} />{busy ? copy("common.saving") : copy("jobs.discardImport")}</button> : null}
      </div>
    </div> : null}
  </section>;
}
