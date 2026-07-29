import { X } from "@phosphor-icons/react";
import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type FormEvent,
} from "react";
import {
  colorsFor,
  materialGroups,
  searchColors,
  seriesFor,
  type FilamentColor,
} from "../../catalog/bambu";
import { Swatch } from "../../components/Swatch";
import { t, useLocale } from "../../i18n";
import type { NewSpool, Spool } from "../../lib/tauri";

const FOCUSABLE_SELECTOR = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled]):not([type='hidden'])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

function focusableElements(dialog: HTMLElement): HTMLElement[] {
  return Array.from(
    dialog.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
  ).filter((element) => {
    if (element.tabIndex < 0) return false;
    if (element.closest("[hidden], [aria-hidden='true']")) return false;

    let current: HTMLElement | null = element;
    while (current && dialog.contains(current)) {
      const style = window.getComputedStyle(current);
      if (style.display === "none" || style.visibility === "hidden") {
        return false;
      }
      current = current.parentElement;
    }
    return true;
  });
}

function backgroundRoots(dialog: HTMLElement): HTMLElement[] {
  let dialogHost = dialog;
  while (
    dialogHost.parentElement
    && dialogHost.parentElement !== document.body
  ) {
    dialogHost = dialogHost.parentElement;
  }

  return Array.from(document.body.children).filter(
    (element): element is HTMLElement =>
      element instanceof HTMLElement && element !== dialogHost,
  );
}

export function Add({
  open,
  spools,
  busy,
  onClose,
  onCreate,
}: {
  open: boolean;
  spools: Spool[];
  busy: boolean;
  onClose(): void;
  onCreate(spool: NewSpool): Promise<boolean | void>;
}): JSX.Element | null {
  const locale = useLocale();
  const copy = (key: string) => t(key, {}, locale);
  const titleId = useId();
  const dialog = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const [group, setGroup] = useState<string | null>(null);
  const [series, setSeries] = useState<string | null>(null);
  const [selected, setSelected] = useState<FilamentColor | null>(null);
  const [query, setQuery] = useState("");
  const [name, setName] = useState("");
  const [grams, setGrams] = useState("1000");

  useEffect(() => {
    if (!open || !dialog.current) return;

    const dialogElement = dialog.current;
    const activeElement = document.activeElement;
    const opener =
      activeElement instanceof HTMLElement && activeElement !== document.body
        ? activeElement
        : null;
    const background = backgroundRoots(dialogElement).map((element) => ({
      element,
      inert: element.getAttribute("inert"),
    }));

    for (const { element } of background) {
      element.setAttribute("inert", "");
    }

    const handleKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab") return;

      const focusable = focusableElements(dialogElement);
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      const active = document.activeElement;
      const focusLeftDialog =
        !(active instanceof HTMLElement && focusable.includes(active));

      if (event.shiftKey && (active === first || focusLeftDialog)) {
        event.preventDefault();
        event.stopPropagation();
        last.focus();
      } else if (!event.shiftKey && (active === last || focusLeftDialog)) {
        event.preventDefault();
        event.stopPropagation();
        first.focus();
      }
    };

    document.addEventListener("keydown", handleKeyDown, true);
    closeButton.current?.focus();

    return () => {
      document.removeEventListener("keydown", handleKeyDown, true);
      for (const { element, inert } of background) {
        if (inert === null) element.removeAttribute("inert");
        else element.setAttribute("inert", inert);
      }
      if (opener?.isConnected) opener.focus();
    };
  }, [open]);

  const availableColors = useMemo(() => {
    if (!group || !series) return [];
    return searchColors(colorsFor(group, series), query, locale);
  }, [group, locale, query, series]);

  if (!open) return null;

  const numericGrams = Number(grams);
  const canSave = Boolean(
    group
      && series
      && selected
      && Number.isFinite(numericGrams)
      && numericGrams > 0,
  );

  const chooseGroup = (nextGroup: string) => {
    setGroup(nextGroup);
    setSeries(null);
    setSelected(null);
    setQuery("");
  };

  const chooseSeries = (nextSeries: string) => {
    setSeries(nextSeries);
    setSelected(null);
    setQuery("");
  };

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selected || !canSave) return;

    const base =
      `${selected.names[locale]} · ${selected.materialGroup} ${selected.series}`;
    const duplicates = spools.filter(
      (spool) =>
        spool.catalog_id === selected.id && spool.status !== "archived",
    ).length;
    const display_name =
      name.trim() || (duplicates ? `${base} #${duplicates + 1}` : base);
    const draft: NewSpool = {
      display_name,
      preset_id: selected.presetBase,
      preset_base: selected.presetBase,
      catalog_id: selected.id,
      brand: selected.brand,
      material: selected.material,
      series: selected.series,
      color_name: selected.names[locale],
      color_code: selected.colorCode,
      color_hex: selected.colors[0],
      color_hexes: selected.colors,
      remaining_grams: numericGrams,
    };

    const succeeded = await onCreate(draft);
    if (succeeded === false) return;

    setSelected(null);
    setQuery("");
    setName("");
    setGrams("1000");
    onClose();
  };

  return (
    <div
      ref={dialog}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      className="add-dialog"
    >
      <form onSubmit={submit}>
        <header>
          <h2 id={titleId}>{copy("spools.add")}</h2>
          <button
            ref={closeButton}
            type="button"
            className="icon-button"
            aria-label={copy("common.close")}
            onClick={onClose}
          >
            <X size={18} weight="bold" />
          </button>
        </header>

        <section aria-labelledby={`${titleId}-material`}>
          <h3 id={`${titleId}-material`}>{copy("spools.chooseMaterial")}</h3>
          <div className="add-dialog-options">
            {materialGroups().map((item) => (
              <button
                key={item}
                type="button"
                disabled={busy}
                aria-pressed={group === item}
                onClick={() => chooseGroup(item)}
              >
                {item}
              </button>
            ))}
          </div>
        </section>

        {group ? (
          <section aria-labelledby={`${titleId}-series`}>
            <h3 id={`${titleId}-series`}>{copy("spools.chooseSeries")}</h3>
            <div className="add-dialog-options">
              {seriesFor(group).map((item) => (
                <button
                  key={item}
                  type="button"
                  disabled={busy}
                  aria-pressed={series === item}
                  onClick={() => chooseSeries(item)}
                >
                  {item}
                </button>
              ))}
            </div>
          </section>
        ) : null}

        {group && series ? (
          <section aria-labelledby={`${titleId}-color`}>
            <h3 id={`${titleId}-color`}>{copy("spools.chooseColor")}</h3>
            <label className="search">
              <span className="sr-only">{copy("spools.searchColors")}</span>
              <input
                type="search"
                value={query}
                disabled={busy}
                placeholder={copy("spools.searchColors")}
                onChange={(event) => setQuery(event.target.value)}
              />
            </label>
            {availableColors.length ? (
              <div className="add-dialog-colors">
                {availableColors.map((entry) => (
                  <button
                    key={entry.id}
                    type="button"
                    disabled={busy}
                    aria-pressed={selected?.id === entry.id}
                    aria-label={`${entry.names[locale]} ${copy(
                      "spools.colorCode",
                    )} ${entry.colorCode}`}
                    onClick={() => setSelected(entry)}
                  >
                    <Swatch colors={entry.colors} />
                    <span>{entry.names[locale]}</span>
                    <small>{entry.colorCode}</small>
                    {entry.classic ? (
                      <small>{copy("spools.classic")}</small>
                    ) : null}
                  </button>
                ))}
              </div>
            ) : (
              <p>{copy("spools.noColors")}</p>
            )}
          </section>
        ) : null}

        {selected ? (
          <div className="add-dialog-selected">
            <Swatch colors={selected.colors} size="large" />
            <span>{copy("spools.selectedColor")}</span>
            <strong>{selected.names[locale]}</strong>
            <small>
              {copy("spools.colorCode")} {selected.colorCode}
            </small>
          </div>
        ) : null}

        <div className="field">
          <label htmlFor={`${titleId}-name`}>
            {copy("spools.customName")}
          </label>
          <input
            id={`${titleId}-name`}
            aria-describedby={`${titleId}-name-hint`}
            value={name}
            disabled={busy}
            onChange={(event) => setName(event.target.value)}
          />
          <small id={`${titleId}-name-hint`}>
            {copy("spools.customNameHint")}
          </small>
        </div>

        <div className="field">
          <label htmlFor={`${titleId}-grams`}>
            {copy("spools.remaining")}
          </label>
          <input
            id={`${titleId}-grams`}
            type="number"
            min="0"
            step="0.1"
            value={grams}
            disabled={busy}
            onChange={(event) => setGrams(event.target.value)}
          />
        </div>

        <div className="form-actions">
          <button type="button" className="ghost" onClick={onClose}>
            {copy("common.cancel")}
          </button>
          <button
            type="submit"
            className="primary"
            disabled={busy || !canSave}
          >
            {busy ? copy("common.saving") : copy("common.save")}
          </button>
        </div>
      </form>
    </div>
  );
}
