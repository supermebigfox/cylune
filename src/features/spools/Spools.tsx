import { Archive, ArrowDown, Funnel, MagnifyingGlass, Plus, Scales } from "@phosphor-icons/react";
import { useMemo, useState } from "react";
import { t, useLocale } from "../../i18n";
import type { NewSpool, Spool, SpoolStatus } from "../../lib/tauri";

export function Spools({ spools, slotBySpool, onCreate, onCalibrate, onArchive, onMount }: {
  spools: Spool[];
  slotBySpool: Record<string, number>;
  onCreate(spool: NewSpool): Promise<void>;
  onCalibrate(spoolId: string, grams: number): void | Promise<void>;
  onArchive(spoolId: string): void | Promise<void>;
  onMount(spoolId: string, slot: number): void | Promise<void>;
}) {
  const locale = useLocale();
  const copy = (key: string, values: Record<string, string | number> = {}) => t(key, values, locale);
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState<SpoolStatus | "all">("all");
  const [material, setMaterial] = useState("all");
  const [color, setColor] = useState("all");
  const [calibrating, setCalibrating] = useState<string | null>(null);
  const [mounting, setMounting] = useState<string | null>(null);
  const [slot, setSlot] = useState("1");
  const [grams, setGrams] = useState("");
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState<NewSpool>({ display_name: "", brand: "Bambu Lab", material: "PLA", series: "Basic", color_hex: "#FF645A", remaining_grams: 1000 });
  const filtered = useMemo(() => spools.filter((spool) => {
    const haystack = `${spool.display_name} ${spool.brand} ${spool.material} ${spool.series} ${spool.color_hex}`.toLowerCase();
    return haystack.includes(query.toLowerCase()) && (status === "all" || spool.status === status) && (material === "all" || spool.material === material) && (color === "all" || spool.color_hex.toUpperCase() === color);
  }), [color, material, query, spools, status]);
  const materials = [...new Set(spools.map((spool) => spool.material))];
  const colors = [...new Set(spools.map((spool) => spool.color_hex.toUpperCase()))];

  const submitCalibration = async () => {
    if (!calibrating || !Number.isFinite(Number(grams))) return;
    await onCalibrate(calibrating, Number(grams));
    setCalibrating(null);
  };

  return (
    <section className="page" aria-labelledby="spools-title">
      <div className="page-heading"><div><h1 id="spools-title">{copy("spools.title")}</h1><p>{copy("spools.libraryHint")}</p></div><button className="primary" onClick={() => setCreating(true)}><Plus size={18} weight="bold" />{copy("spools.add")}</button></div>
      <div className="toolbar">
        <label className="search"><MagnifyingGlass size={18} /><span className="sr-only">{copy("spools.search")}</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={copy("spools.searchPlaceholder")} /></label>
        <label><Funnel size={17} /><span className="sr-only">{copy("spools.status")}</span><select value={status} onChange={(event) => setStatus(event.target.value as SpoolStatus | "all")}><option value="all">{copy("common.all")}</option><option value="available">{copy("spools.available")}</option><option value="assigned">{copy("spools.assigned")}</option><option value="empty">{copy("spools.depleted")}</option></select></label>
        <label><span className="sr-only">{copy("spools.material")}</span><select value={material} onChange={(event) => setMaterial(event.target.value)}><option value="all">{copy("spools.allMaterials")}</option>{materials.map((item) => <option key={item}>{item}</option>)}</select></label>
        <label><span className="sr-only">{copy("spools.colorFilter")}</span><select value={color} onChange={(event) => setColor(event.target.value)}><option value="all">{copy("spools.allColors")}</option>{colors.map((item) => <option key={item} value={item}>{item}</option>)}</select></label>
      </div>

      {creating ? <form className="inline-form" onSubmit={async (event) => { event.preventDefault(); await onCreate(draft); setCreating(false); }}>
        <div><h2>{copy("spools.add")}</h2><p>{copy("spools.createHint")}</p></div>
        <label>{copy("spools.name")}<input required value={draft.display_name} onChange={(event) => setDraft({ ...draft, display_name: event.target.value })} /></label>
        <label>{copy("spools.material")}<select value={draft.material} onChange={(event) => setDraft({ ...draft, material: event.target.value })}><option>PLA</option><option>PETG</option><option>TPU</option></select></label>
        <label>{copy("spools.color")}<input type="color" value={draft.color_hex} onChange={(event) => setDraft({ ...draft, color_hex: event.target.value.toUpperCase() })} /></label>
        <label>{copy("spools.remaining")}<input type="number" min="0" step="0.1" value={draft.remaining_grams} onChange={(event) => setDraft({ ...draft, remaining_grams: Number(event.target.value) })} /></label>
        <div className="form-actions"><button type="button" className="ghost" onClick={() => setCreating(false)}>{copy("common.cancel")}</button><button className="primary" type="submit">{copy("common.save")}</button></div>
      </form> : null}

      {filtered.length ? <div className="spool-table" role="table" aria-label={copy("spools.title")}>
        <div className="table-head" role="row"><span>{copy("spools.name")}</span><span>{copy("spools.profile")}</span><span>{copy("spools.remaining")}</span><span>{copy("spools.location")}</span><span>{copy("spools.status")}</span><span>{copy("spools.actions")}</span></div>
        {filtered.map((spool) => <div className="spool-row" role="row" key={spool.spool_id}>
          <div className="spool-name"><i className="swatch" style={{ "--swatch": spool.color_hex } as React.CSSProperties} /><span><strong>{spool.display_name}</strong><small className="data">{spool.spool_id}</small></span></div>
          <span>{spool.brand}<small>{spool.material} {spool.series}</small></span>
          <strong className="data">{spool.remaining_grams.toFixed(1)} {copy("common.grams")}</strong>
          <span>{slotBySpool[spool.spool_id] ? `AMS ${slotBySpool[spool.spool_id]}` : copy("nav.spools")}</span>
          <span><b className={`status status-${spool.status}`}>{copy(`spools.${spool.status === "empty" ? "depleted" : spool.status}`)}</b></span>
          <span className="row-actions"><button className="icon-button" title={copy("spools.mount")} aria-label={copy("spools.mount")} disabled={spool.status !== "available"} onClick={() => { setMounting(spool.spool_id); setSlot("1"); }}><ArrowDown size={18} /></button><button className="icon-button" title={copy("spools.calibrate")} aria-label={copy("spools.calibrate")} onClick={() => { setCalibrating(spool.spool_id); setGrams(String(spool.remaining_grams)); }}><Scales size={18} /></button><button className="icon-button" title={copy("spools.archive")} aria-label={copy("spools.archive")} disabled={spool.status === "assigned"} onClick={() => onArchive(spool.spool_id)}><Archive size={18} /></button></span>
          {calibrating === spool.spool_id ? <div className="calibrate-pop"><label>{copy("spools.newWeight")}<input type="number" min="0" step="0.1" value={grams} onChange={(event) => setGrams(event.target.value)} /></label><button className="primary small" onClick={submitCalibration}>{copy("spools.saveCalibration")}</button></div> : null}
          {mounting === spool.spool_id ? <div className="calibrate-pop"><label>{copy("spools.slot")}<select value={slot} onChange={(event) => setSlot(event.target.value)}>{[1, 2, 3, 4].map((item) => <option key={item} value={item}>AMS {item}</option>)}</select></label><button className="primary small" onClick={async () => { await onMount(spool.spool_id, Number(slot)); setMounting(null); }}>{copy("spools.confirmMount")}</button></div> : null}
        </div>)}
      </div> : <div className="empty-state"><h2>{copy("spools.empty")}</h2><p>{copy("spools.emptyHint")}</p></div>}
    </section>
  );
}
