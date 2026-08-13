import {
  ArrowRight,
  CheckCircle,
  PencilSimple,
  Plus,
  Printer,
  Star,
  Trash,
  WarningCircle,
  X,
} from "@phosphor-icons/react";
import {
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type FormEvent,
} from "react";
import { createPortal } from "react-dom";
import { t, useLocale } from "../../i18n";
import {
  api,
  type PrinterProfile,
  type SavePrinter,
  type SavedPrinter,
  type TauriApi,
} from "../../lib/tauri";

type PrinterApi = Pick<
  TauriApi,
  | "listAvailablePrinters"
  | "listSavedPrinters"
  | "savePrinter"
  | "deletePrinter"
  | "setDefaultPrinter"
>;

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
      if (style.display === "none" || style.visibility === "hidden") return false;
      current = current.parentElement;
    }
    return true;
  });
}

function backgroundRoots(dialog: HTMLElement): HTMLElement[] {
  let dialogHost = dialog;
  while (dialogHost.parentElement && dialogHost.parentElement !== document.body) {
    dialogHost = dialogHost.parentElement;
  }
  return Array.from(document.body.children).filter(
    (element): element is HTMLElement =>
      element instanceof HTMLElement && element !== dialogHost,
  );
}

function stableErrorCode(error: unknown) {
  let candidate = error;
  if (typeof candidate === "string") {
    try { candidate = JSON.parse(candidate); }
    catch { return "io"; }
  }
  if (candidate && typeof candidate === "object" && "code" in candidate) {
    return String((candidate as { code: unknown }).code);
  }
  return "io";
}

function initialDraft(
  profiles: PrinterProfile[],
  printer: SavedPrinter | null,
): SavePrinter {
  const profile = profiles.find((item) => item.model_key === printer?.model_key)
    ?? profiles[0];
  return {
    printer_id: printer?.printer_id,
    display_name: printer?.display_name ?? "",
    model_key: printer?.model_key ?? profile?.model_key ?? "",
    nozzle_diameter: printer?.nozzle_diameter ?? profile?.nozzle_diameters[0] ?? 0.4,
    default_plate: printer?.default_plate ?? profile?.plate_keys[0] ?? "",
    ams_kind: printer?.ams_kind ?? "none",
    is_default: printer?.is_default ?? false,
  };
}

function PrinterDialog({
  profiles,
  printer,
  busy,
  error,
  onClose,
  onSave,
}: {
  profiles: PrinterProfile[];
  printer: SavedPrinter | null;
  busy: boolean;
  error: string | null;
  onClose(): void;
  onSave(draft: SavePrinter): Promise<void>;
}) {
  const locale = useLocale();
  const copy = (key: string) => t(key, {}, locale);
  const titleId = useId();
  const dialog = useRef<HTMLDivElement>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;
  const [draft, setDraft] = useState(() => initialDraft(profiles, printer));

  const catalogProfile = useMemo(
    () => profiles.find((profile) => profile.model_key === draft.model_key),
    [draft.model_key, profiles],
  );
  const savedProfile = printer && printer.model_key === draft.model_key
    ? {
      model_key: printer.model_key,
      display_name: printer.model_key,
      nozzle_diameters: [printer.nozzle_diameter],
      plate_keys: [printer.default_plate],
    }
    : undefined;
  const selectedProfile = catalogProfile ?? savedProfile;
  const modelOptions = catalogProfile || !printer
    ? profiles
    : [savedProfile!, ...profiles];
  const valid = Boolean(
    draft.display_name.trim()
      && selectedProfile
      && selectedProfile.nozzle_diameters.includes(draft.nozzle_diameter)
      && selectedProfile.plate_keys.includes(draft.default_plate),
  );

  useEffect(() => {
    if (!dialog.current) return;
    const dialogElement = dialog.current;
    const activeElement = document.activeElement;
    const opener = activeElement instanceof HTMLElement && activeElement !== document.body
      ? activeElement
      : null;
    const background = backgroundRoots(dialogElement).map((element) => ({
      element,
      inert: element.getAttribute("inert"),
    }));
    background.forEach(({ element }) => element.setAttribute("inert", ""));

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
      const outside = !(active instanceof HTMLElement && focusable.includes(active));
      if (event.shiftKey && (active === first || outside)) {
        event.preventDefault();
        event.stopPropagation();
        last.focus();
      } else if (!event.shiftKey && (active === last || outside)) {
        event.preventDefault();
        event.stopPropagation();
        first.focus();
      }
    };
    document.addEventListener("keydown", handleKeyDown, true);
    closeButton.current?.focus();
    return () => {
      document.removeEventListener("keydown", handleKeyDown, true);
      background.forEach(({ element, inert }) => {
        if (inert === null) element.removeAttribute("inert");
        else element.setAttribute("inert", inert);
      });
      if (opener?.isConnected) opener.focus();
    };
  }, []);

  const chooseModel = (modelKey: string) => {
    const profile = profiles.find((item) => item.model_key === modelKey);
    setDraft((current) => ({
      ...current,
      model_key: modelKey,
      nozzle_diameter: profile?.nozzle_diameters[0] ?? current.nozzle_diameter,
      default_plate: profile?.plate_keys[0] ?? current.default_plate,
    }));
  };
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (valid && !busy) void onSave({
      ...draft,
      display_name: draft.display_name.trim(),
    });
  };

  return <div
    ref={dialog}
    role="dialog"
    aria-modal="true"
    aria-labelledby={titleId}
    className="printer-dialog"
  >
    <form onSubmit={submit}>
      <header>
        <div><Printer size={22} weight="duotone" /><h2 id={titleId}>{copy(printer ? "printers.edit" : "printers.add")}</h2></div>
        <button ref={closeButton} type="button" className="icon-button" aria-label={copy("common.close")} onClick={onClose}><X size={18} weight="bold" /></button>
      </header>
      <div className="printer-form-grid">
        <label className="printer-field printer-field-wide">
          <span>{copy("printers.name")}</span>
          <input autoComplete="off" maxLength={80} value={draft.display_name} disabled={busy} onChange={(event) => setDraft((current) => ({ ...current, display_name: event.target.value }))} />
        </label>
        <label className="printer-field printer-field-wide">
          <span>{copy("printers.model")}</span>
          <select value={draft.model_key} disabled={busy || !modelOptions.length} onChange={(event) => chooseModel(event.target.value)}>
            {modelOptions.map((profile) => <option key={profile.model_key} value={profile.model_key}>{profile.display_name}</option>)}
          </select>
        </label>
        <label className="printer-field">
          <span>{copy("printers.nozzle")}</span>
          <select value={String(draft.nozzle_diameter)} disabled={busy || !selectedProfile} onChange={(event) => setDraft((current) => ({ ...current, nozzle_diameter: Number(event.target.value) }))}>
            {(selectedProfile?.nozzle_diameters ?? [draft.nozzle_diameter]).map((diameter) => <option key={diameter} value={diameter}>{diameter.toFixed(1)} mm</option>)}
          </select>
        </label>
        <label className="printer-field">
          <span>{copy("printers.plate")}</span>
          <select value={draft.default_plate} disabled={busy || !selectedProfile} onChange={(event) => setDraft((current) => ({ ...current, default_plate: event.target.value }))}>
            {(selectedProfile?.plate_keys ?? [draft.default_plate]).map((plate) => <option key={plate} value={plate}>{plate}</option>)}
          </select>
        </label>
        <label className="printer-field printer-field-wide">
          <span>{copy("printers.ams")}</span>
          <select value={draft.ams_kind} disabled={busy} onChange={(event) => setDraft((current) => ({ ...current, ams_kind: event.target.value }))}>
            <option value="none">{copy("printers.amsNone")}</option>
            <option value="ams">AMS</option>
            <option value="ams_lite">AMS Lite</option>
          </select>
        </label>
      </div>
      <label className="printer-default-check">
        <input type="checkbox" aria-label={copy("printers.makeDefault")} checked={draft.is_default} disabled={busy} onChange={(event) => setDraft((current) => ({ ...current, is_default: event.target.checked }))} />
        <span><strong>{copy("printers.makeDefault")}</strong><small>{copy("printers.defaultHint")}</small></span>
      </label>
      {error ? <p className="inline-dialog-error" role="alert">{error}</p> : null}
      <div className="form-actions">
        <button type="button" className="ghost" disabled={busy} onClick={onClose}>{copy("common.cancel")}</button>
        <button type="submit" className="primary" disabled={busy || !valid}>{busy ? copy("common.saving") : copy("printers.save")}</button>
      </div>
    </form>
  </div>;
}

function amsLabel(printer: SavedPrinter, copy: (key: string) => string) {
  if (printer.ams_kind === "ams") return "AMS";
  if (printer.ams_kind === "ams_lite") return "AMS Lite";
  return copy("printers.amsNone");
}

export function Printers({
  apiClient = api,
  onStartSlice,
}: {
  apiClient?: PrinterApi;
  onStartSlice?(printer: SavedPrinter): void;
}) {
  const locale = useLocale();
  const copy = (key: string) => t(key, {}, locale);
  const [profiles, setProfiles] = useState<PrinterProfile[]>([]);
  const [printers, setPrinters] = useState<SavedPrinter[]>([]);
  const [editing, setEditing] = useState<SavedPrinter | null | undefined>(undefined);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dialogError, setDialogError] = useState<string | null>(null);

  const load = async () => {
    const [available, saved] = await Promise.all([
      apiClient.listAvailablePrinters(),
      apiClient.listSavedPrinters(),
    ]);
    setProfiles(available);
    setPrinters(saved);
  };

  useEffect(() => {
    let disposed = false;
    void Promise.all([
      apiClient.listAvailablePrinters(),
      apiClient.listSavedPrinters(),
    ]).then(([available, saved]) => {
      if (!disposed) {
        setProfiles(available);
        setPrinters(saved);
        setError(null);
      }
    }).catch((loadError) => {
      if (!disposed) setError(copy(`errors.${stableErrorCode(loadError)}`));
    }).finally(() => {
      if (!disposed) setLoading(false);
    });
    return () => { disposed = true; };
  }, [apiClient, locale]);

  const run = async (operation: () => Promise<void>, inDialog = false) => {
    setBusy(true);
    if (inDialog) setDialogError(null);
    else setError(null);
    try {
      await operation();
      return true;
    } catch (actionError) {
      const message = copy(`errors.${stableErrorCode(actionError)}`);
      if (inDialog) setDialogError(message);
      else setError(message);
      return false;
    } finally {
      setBusy(false);
    }
  };
  const save = async (draft: SavePrinter) => {
    const succeeded = await run(async () => {
      await apiClient.savePrinter(draft);
      await load();
    }, true);
    if (succeeded) setEditing(undefined);
  };
  const makeDefault = (printerId: string) => run(async () => {
    await apiClient.setDefaultPrinter(printerId);
    await load();
  });
  const remove = (printer: SavedPrinter) => {
    if (printer.is_default && !window.confirm(copy("printers.deleteDefaultConfirm"))) return;
    void run(async () => {
      await apiClient.deletePrinter(printer.printer_id);
      await load();
    });
  };
  const openDialog = (printer: SavedPrinter | null) => {
    setDialogError(null);
    setEditing(printer);
  };

  return <section className="page printers-page" aria-labelledby="printers-title">
    <div className="page-heading">
      <div><h1 id="printers-title">{copy("printers.title")}</h1><p>{copy("printers.hint")}</p></div>
      <button className="primary" disabled={loading || busy || !profiles.length} onClick={() => openDialog(null)}><Plus size={18} weight="bold" />{copy("printers.add")}</button>
    </div>
    {error ? <div className="printer-page-error" role="alert"><span>{error}</span><button type="button" onClick={() => void load()}>{copy("common.retry")}</button></div> : null}
    {loading ? <div className="printer-loading" aria-label={copy("common.loading")}><i /><i /></div> : null}
    {!loading && !printers.length ? <div className="printer-empty"><span><Printer size={34} weight="duotone" /></span><h2>{copy("printers.empty")}</h2><p>{copy("printers.emptyHint")}</p></div> : null}
    {!loading && printers.length ? <div className="printer-grid">
      {printers.map((printer) => <article className={`printer-card${printer.is_available ? "" : " unavailable"}`} key={printer.printer_id}>
        <header>
          <span className="printer-glyph"><Printer size={28} weight="duotone" /></span>
          <div><h2>{printer.display_name}</h2><p>{printer.model_key}</p></div>
          {printer.is_default ? <span className="printer-default"><Star size={13} weight="fill" />{copy("printers.default")}</span> : null}
        </header>
        <dl>
          <div><dt>{copy("printers.nozzle")}</dt><dd className="data">{printer.nozzle_diameter.toFixed(1)} mm</dd></div>
          <div><dt>{copy("printers.plate")}</dt><dd>{printer.default_plate}</dd></div>
          <div><dt>{copy("printers.ams")}</dt><dd>{amsLabel(printer, copy)}</dd></div>
          <div><dt>{copy("printers.profileStatus")}</dt><dd className={printer.is_available ? "available" : "missing"}>{printer.is_available ? <><CheckCircle size={15} weight="fill" />{copy("printers.available")}</> : <><WarningCircle size={15} weight="fill" />{copy("printers.unavailable")}</>}</dd></div>
        </dl>
        {!printer.is_available ? <div className="printer-warning"><WarningCircle size={18} weight="fill" /><span>{copy("printers.unavailableHint")}</span></div> : null}
        <footer>
          <div>
            <button className="icon-button" type="button" aria-label={copy("printers.edit")} disabled={busy} onClick={() => openDialog(printer)}><PencilSimple size={18} /></button>
            <button className="icon-button danger" type="button" aria-label={copy("printers.delete")} disabled={busy} onClick={() => remove(printer)}><Trash size={18} /></button>
          </div>
          {!printer.is_default ? <button className="ghost small" type="button" disabled={busy} onClick={() => void makeDefault(printer.printer_id)}><Star size={15} />{copy("printers.setDefault")}</button> : null}
          <button className="secondary small" type="button" disabled={busy || !printer.is_available} onClick={() => onStartSlice?.(printer)}>{copy("printers.startSlice")}<ArrowRight size={15} /></button>
        </footer>
      </article>)}
    </div> : null}
    {editing !== undefined ? createPortal(<div className="modal-backdrop"><div className="printer-dialog-shell"><PrinterDialog profiles={profiles} printer={editing} busy={busy} error={dialogError} onClose={() => !busy && setEditing(undefined)} onSave={save} /></div></div>, document.body) : null}
  </section>;
}
