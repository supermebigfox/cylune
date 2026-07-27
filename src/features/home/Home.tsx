import { ArrowRight, FolderOpen, WarningCircle } from "@phosphor-icons/react";
import { t, useLocale } from "../../i18n";
import type { SlotView, Spool } from "../../lib/tauri";

export function Home({ slots, spools, pendingJobs, onImport }: {
  slots: SlotView[];
  spools: Spool[];
  pendingJobs: number;
  onImport(): void;
}) {
  const locale = useLocale();
  const copy = (key: string, values: Record<string, string | number> = {}) => t(key, values, locale);
  const low = spools.filter((spool) => spool.remaining_grams > 0 && spool.remaining_grams < 150).length;

  return (
    <section className="page home-page" aria-labelledby="home-title">
      <div className="welcome">
        <div className="welcome-copy">
          <h1 id="home-title">{copy("home.greeting")}</h1>
          <p>{copy("app.tagline")}</p>
        </div>
        <button className="import-card" type="button" aria-label={copy("import.title")} onClick={onImport}>
          <span className="import-icon"><FolderOpen size={26} weight="duotone" /></span>
          <span><strong>{copy("import.title")}</strong><small>{copy("home.importHint")}</small></span>
          <ArrowRight size={20} />
        </button>
      </div>

      <div className="summary-strip" aria-label={copy("home.summary")}>
        <span>{copy("home.inventoryCount", { count: spools.length })}</span>
        <span className={low ? "warn" : ""}>{low ? <WarningCircle size={17} weight="fill" /> : null}{copy("home.lowCount", { count: low })}</span>
        <span>{copy("home.pendingCount", { count: pendingJobs })}</span>
      </div>

      <div className="section-heading">
        <div><h2>{copy("slots.title")}</h2><p>{copy("home.slotsHint")}</p></div>
      </div>
      <div className="slot-grid">
        {slots.slice(0, 4).map((slot) => {
          const grams = slot.spool?.remaining_grams ?? 0;
          const progress = Math.max(0, Math.min(100, grams / 10));
          return (
            <article className={`slot-card ${slot.spool ? "filled" : "empty"}`} data-testid="ams-slot" key={slot.slot_number}>
              <header><span>{copy("slots.slot", { number: slot.slot_number })}</span><b>{slot.spool ? copy("slots.mounted") : copy("home.awaiting")}</b></header>
              {slot.spool ? (
                <>
                  <div className="spool-identity">
                    <i className="swatch large" style={{ "--swatch": slot.spool.color_hex } as React.CSSProperties} />
                    <div><strong>{slot.spool.display_name}</strong><small>{slot.spool.material} {slot.spool.series}</small></div>
                  </div>
                  <div className="balance-row"><strong className="data">{slot.spool.remaining_grams.toFixed(1)} g</strong><span>{Math.round(progress)}%</span></div>
                  <div className="meter" aria-label={copy("slots.remaining", { grams: slot.spool.remaining_grams.toFixed(1) })}><i style={{ transform: `scaleX(${progress / 100})` }} /></div>
                </>
              ) : (
                <div className="slot-empty"><span>{slot.slot_number}</span><p>{copy("slots.empty")}</p><small>{copy("home.emptySlotHint")}</small></div>
              )}
            </article>
          );
        })}
      </div>
    </section>
  );
}
