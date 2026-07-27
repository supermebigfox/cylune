import { CheckCircle, ClockCounterClockwise, FolderOpen, SpinnerGap, UploadSimple, WarningCircle } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { Mark } from "../../brand/Mark";
import { t, useLocale } from "../../i18n";

type ImportedJob = { job_id: string; source_file_name: string };
type DropState = "idle" | "hover" | "parsing" | "success" | "unsliced" | "error";
type VisibilitySubscriber = (handler: () => void) => Promise<() => void>;

const subscribeToVisibility: VisibilitySubscriber = async (handler) => {
  if (!("__TAURI_INTERNALS__" in globalThis)) return () => undefined;
  return listen("tray-opened", handler);
};

export function isSupportedPrintPath(path: string): boolean {
  const name = path.toLowerCase();
  return name.endsWith(".gcode.3mf") || name.endsWith(".3mf") || name.endsWith(".gcode");
}

export function TrayDrop({ onImport, onOpenJob, onOpenMain, subscribeVisibility = subscribeToVisibility }: {
  onImport: (path: string) => Promise<ImportedJob>;
  onOpenJob: (jobId: string) => void | Promise<void>;
  onOpenMain?: () => void | Promise<void>;
  subscribeVisibility?: VisibilitySubscriber;
}) {
  const locale = useLocale();
  const copy = (key: string, values: Record<string, string | number> = {}) => t(key, values, locale);
  const [state, setState] = useState<DropState>("idle");
  const [message, setMessage] = useState<string | null>(null);
  const [job, setJob] = useState<ImportedJob | null>(null);
  const [entering, setEntering] = useState(false);
  const busy = useRef(false);
  const seen = useRef(new Set<string>());
  const dedupeTimers = useRef(new Set<ReturnType<typeof setTimeout>>());

  const importPaths = async (paths: string[]) => {
    const unsupported = paths.filter((path) => !isSupportedPrintPath(path));
    if (unsupported.length) setMessage(unsupported.map((path) => copy("tray.unsupported", { name: path.split(/[\\/]/).pop() ?? path })).join(" · "));
    const path = paths.find((candidate) => isSupportedPrintPath(candidate) && !seen.current.has(candidate));
    if (!path || busy.current) return;
    busy.current = true;
    seen.current.add(path);
    setState("parsing");
    setJob(null);
    try {
      const imported = await onImport(path);
      setJob(imported);
      setState("success");
      const timer = setTimeout(() => {
        seen.current.delete(path);
        dedupeTimers.current.delete(timer);
      }, 2_000);
      dedupeTimers.current.add(timer);
    } catch (error) {
      const code = typeof error === "object" && error && "code" in error ? String((error as { code: unknown }).code) : "io";
      setState(code === "unsliced_project" ? "unsliced" : "error");
      setMessage(copy(code === "standalone_gcode_profiles_required" ? "tray.gcodeNeedsProfile" : `errors.${code}`));
      seen.current.delete(path);
    } finally {
      busy.current = false;
    }
  };
  useEffect(()=>{
    if (!("__TAURI_INTERNALS__" in globalThis)) return;
    let disposed=false; let unlisten:(()=>void)|undefined;
    void getCurrentWebviewWindow().onDragDropEvent((event)=>{
      if(event.payload.type==="enter"&&!busy.current)setState("hover");
      if(event.payload.type==="leave"&&!busy.current)setState("idle");
      if(event.payload.type==="drop")void importPaths(event.payload.paths);
    }).then((stop)=>{if(disposed)stop();else unlisten=stop;});
    return()=>{disposed=true;unlisten?.();};
  },[locale]);
  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void subscribeVisibility(() => setEntering(true)).then((stop) => {
      if (disposed) stop();
      else unsubscribe = stop;
    });
    return () => {
      disposed = true;
      unsubscribe?.();
      dedupeTimers.current.forEach(clearTimeout);
      dedupeTimers.current.clear();
    };
  }, [subscribeVisibility]);

  const icon = state === "parsing" ? <SpinnerGap className="spin" size={34} /> : state === "success" ? <CheckCircle size={34} weight="fill" /> : state === "error" || state === "unsliced" ? <WarningCircle size={34} weight="fill" /> : <UploadSimple size={34} weight="duotone" />;
  const title = state === "parsing" ? copy("import.reading") : state === "success" ? copy("import.ready") : state === "unsliced" ? copy("import.unsliced") : copy("tray.dropTitle");

  return <main
    className={`tray-popover${entering ? " entering" : ""}`}
    data-testid="tray-popover"
    onAnimationEnd={() => setEntering(false)}
  >
    <header><Mark label={copy("brand.mark")} size={30} /><div><strong>{copy("app.name")}</strong><span>{copy("app.localMode")}</span></div><button className="tray-open" onClick={onOpenMain} aria-label={copy("tray.open")}><FolderOpen size={19} /></button></header>
    <section
      className={`menu-dropzone state-${state}`}
      data-testid="menu-dropzone"
      onDragEnter={(event) => { event.preventDefault(); if (!busy.current) setState("hover"); }}
      onDragOver={(event) => event.preventDefault()}
      onDragLeave={() => { if (!busy.current) setState("idle"); }}
      onDrop={(event) => { event.preventDefault(); if (!("__TAURI_INTERNALS__" in globalThis)) void importPaths(Array.from(event.dataTransfer.files).map((file) => file.name)); }}
    >
      <div className="tray-drop-icon">{icon}</div>
      <h1>{title}</h1>
      <p>{state === "idle" || state === "hover" ? copy("tray.dropHint") : job?.source_file_name ?? message}</p>
      {job ? <button className="tray-primary" onClick={() => onOpenJob(job.job_id)}>{copy("tray.bind")}</button> : null}
    </section>
    {message ? <div className="tray-message" role="status">{message}</div> : null}
    <footer><ClockCounterClockwise size={17} /><span>{copy("tray.recent")}</span><i>{copy("app.privateNote")}</i></footer>
  </main>;
}
