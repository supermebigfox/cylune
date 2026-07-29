import { Archive, ArrowDown, ArrowUp, Funnel, MagnifyingGlass, Plus, Scales } from "@phosphor-icons/react";
import { useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { colorById } from "../../catalog/bambu";
import { Swatch } from "../../components/Swatch";
import { t, useLocale } from "../../i18n";
import type { NewSpool, Spool, SpoolStatus } from "../../lib/tauri";
import { Add, type CreateSpoolResult } from "./Add";

export function Spools({ spools, slotBySpool, busy = false, onCreate, onCalibrate, onArchive, onMount, onUnmount, onMove }: {
  spools: Spool[];
  slotBySpool: Record<string, number>;
  busy?: boolean;
  onCreate(spool: NewSpool): Promise<CreateSpoolResult>;
  onCalibrate(spoolId: string, grams: number): boolean | void | Promise<boolean | void>;
  onArchive(spoolId: string): boolean | void | Promise<boolean | void>;
  onMount(spoolId: string, slot: number): boolean | void | Promise<boolean | void>;
  onUnmount(slot: number): boolean | void | Promise<boolean | void>;
  onMove(spoolId: string, slot: number): boolean | void | Promise<boolean | void>;
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
  const filtered = useMemo(() => spools.filter((spool) => {
    const localizedColorName = colorById(spool.catalog_id)?.names[locale];
    const haystack = `${spool.display_name} ${spool.brand} ${spool.material} ${spool.series} ${spool.color_name ?? ""} ${localizedColorName ?? ""} ${spool.color_code ?? ""} ${spool.color_hex}`.toLowerCase();
    return haystack.includes(query.toLowerCase()) && (status === "all" || spool.status === status) && (material === "all" || spool.material === material) && (color === "all" || spool.color_hex.toUpperCase() === color);
  }), [color, locale, material, query, spools, status]);
  const materials = [...new Set(spools.map((spool) => spool.material))];
  const colors = [...new Set(spools.map((spool) => spool.color_hex.toUpperCase()))];

  const submitCalibration = async () => {
    if (!calibrating || !Number.isFinite(Number(grams))) return;
    const succeeded = await onCalibrate(calibrating, Number(grams));
    if (succeeded !== false) setCalibrating(null);
  };

  return (
    <section className="page" aria-labelledby="spools-title">
      <div className="page-heading"><div><h1 id="spools-title">{copy("spools.title")}</h1><p>{copy("spools.libraryHint")}</p></div><button className="primary" disabled={busy} onClick={() => setCreating(true)}><Plus size={18} weight="bold" />{copy("spools.add")}</button></div>
      <div className="toolbar">
        <label className="search"><MagnifyingGlass size={18} /><span className="sr-only">{copy("spools.search")}</span><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={copy("spools.searchPlaceholder")} /></label>
        <label><Funnel size={17} /><span className="sr-only">{copy("spools.status")}</span><select value={status} onChange={(event) => setStatus(event.target.value as SpoolStatus | "all")}><option value="all">{copy("common.all")}</option><option value="available">{copy("spools.available")}</option><option value="assigned">{copy("spools.assigned")}</option><option value="empty">{copy("spools.depleted")}</option></select></label>
        <label><span className="sr-only">{copy("spools.material")}</span><select value={material} onChange={(event) => setMaterial(event.target.value)}><option value="all">{copy("spools.allMaterials")}</option>{materials.map((item) => <option key={item}>{item}</option>)}</select></label>
        <label><span className="sr-only">{copy("spools.colorFilter")}</span><select value={color} onChange={(event) => setColor(event.target.value)}><option value="all">{copy("spools.allColors")}</option>{colors.map((item) => <option key={item} value={item}>{item}</option>)}</select></label>
      </div>

      {createPortal(<div className="modal-backdrop" hidden={!creating}>
        <div className="catalog-dialog">
          <Add
            open={creating}
            spools={spools}
            busy={busy}
            onClose={() => setCreating(false)}
            onCreate={onCreate}
          />
        </div>
      </div>, document.body)}

      {filtered.length ? <div className="spool-table" role="table" aria-label={copy("spools.title")}>
        <div className="table-head" role="row"><span>{copy("spools.name")}</span><span>{copy("spools.profile")}</span><span>{copy("spools.remaining")}</span><span>{copy("spools.location")}</span><span>{copy("spools.status")}</span><span>{copy("spools.actions")}</span></div>
        {filtered.map((spool) => {
          const colorName = colorById(spool.catalog_id)?.names[locale] ?? spool.color_name;
          return <div className="spool-row" role="row" key={spool.spool_id}>
          <div className="spool-name"><Swatch colors={spool.color_hexes?.length ? spool.color_hexes : [spool.color_hex]} /><span><strong>{spool.display_name}</strong>{(colorName || spool.color_code) ? <small className="spool-color-meta">{colorName ? <span>{colorName}</span> : null}{spool.color_code ? <span className="data">{spool.color_code}</span> : null}</small> : null}<small className="data">{spool.spool_id}</small></span></div>
          <span>{spool.brand}<small>{spool.material} {spool.series}</small></span>
          <strong className="data">{spool.remaining_grams.toFixed(1)} {copy("common.grams")}</strong>
          <span>{slotBySpool[spool.spool_id] ? copy("slots.location", { number: slotBySpool[spool.spool_id] }) : copy("nav.spools")}</span>
          <span><b className={`status status-${spool.status}`}>{copy(`spools.${spool.status === "empty" ? "depleted" : spool.status}`)}</b></span>
          <span className="row-actions">{slotBySpool[spool.spool_id] ? <><button className="icon-button" title={copy("spools.unmount")} aria-label={copy("spools.unmount")} disabled={busy} onClick={() => onUnmount(slotBySpool[spool.spool_id])}><ArrowUp size={18} /></button><button className="icon-button" title={copy("spools.move")} aria-label={copy("spools.move")} disabled={busy} onClick={() => { const current = slotBySpool[spool.spool_id]; setMounting(spool.spool_id); setSlot(String(current === 4 ? 1 : current + 1)); }}><ArrowDown size={18} /></button></> : <button className="icon-button" title={copy("spools.mount")} aria-label={copy("spools.mount")} disabled={busy || spool.status !== "available"} onClick={() => { setMounting(spool.spool_id); setSlot("1"); }}><ArrowDown size={18} /></button>}<button className="icon-button" title={copy("spools.calibrate")} aria-label={copy("spools.calibrate")} disabled={busy} onClick={() => { setCalibrating(spool.spool_id); setGrams(String(spool.remaining_grams)); }}><Scales size={18} /></button><button className="icon-button" title={copy("spools.archive")} aria-label={copy("spools.archive")} disabled={busy || spool.status === "assigned"} onClick={() => onArchive(spool.spool_id)}><Archive size={18} /></button></span>
          {calibrating === spool.spool_id ? <div className="calibrate-pop"><label>{copy("spools.newWeight")}<input disabled={busy} type="number" min="0" step="0.1" value={grams} onChange={(event) => setGrams(event.target.value)} /></label><button className="primary small" disabled={busy} onClick={submitCalibration}>{busy ? copy("common.saving") : copy("spools.saveCalibration")}</button></div> : null}
          {mounting === spool.spool_id ? <div className="calibrate-pop"><label>{copy("spools.slot")}<select disabled={busy} value={slot} onChange={(event) => setSlot(event.target.value)}>{[1, 2, 3, 4].map((item) => <option key={item} value={item}>{copy("slots.location", { number: item })}</option>)}</select></label><button className="primary small" disabled={busy} onClick={async () => { const succeeded = slotBySpool[spool.spool_id] ? await onMove(spool.spool_id, Number(slot)) : await onMount(spool.spool_id, Number(slot)); if (succeeded !== false) setMounting(null); }}>{busy ? copy("common.saving") : copy(slotBySpool[spool.spool_id] ? "spools.confirmMove" : "spools.confirmMount")}</button></div> : null}
        </div>;
        })}
      </div> : <div className="empty-state"><h2>{copy("spools.empty")}</h2><p>{copy("spools.emptyHint")}</p></div>}
    </section>
  );
}
