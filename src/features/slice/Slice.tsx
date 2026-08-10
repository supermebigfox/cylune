import {
  ArrowClockwise,
  CheckCircle,
  Cube,
  FileArrowUp,
  FolderOpen,
  Info,
  Palette,
  Play,
  Printer,
  SlidersHorizontal,
  Stack,
  StopCircle,
  WarningCircle,
  XCircle,
} from "@phosphor-icons/react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type DragEvent,
  type FormEvent,
  type ReactNode,
} from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { useLocale, type SupportedLocale } from "../../i18n";
import "./Slice.css";

export type SlicePhase =
  | "preparing"
  | "slicing"
  | "validating"
  | "importing"
  | "complete";

export type SliceTaskState = "running" | "completed" | "failed" | "cancelled";

export interface SlicePrinter {
  printer_id: string;
  display_name: string;
  model_key: string;
  nozzle_diameter: number;
  default_plate: string;
  ams_kind: string;
  is_default: boolean;
  is_available: boolean;
}

export interface SliceToolInspection {
  tool: number;
  label?: string | null;
  material?: string | null;
  color_hex?: string | null;
  embedded_filament_key?: string | null;
}

export interface SliceInspection {
  kind: "unsliced" | "sliced";
  file_name: string;
  plate_count: number;
  embedded_model_key?: string | null;
  embedded_nozzle_diameter?: number | null;
  embedded_process_key?: string | null;
  embedded_plate_key?: string | null;
  embedded_infill_density?: number | null;
  embedded_support_enabled?: boolean | null;
  tools: SliceToolInspection[];
}

export interface SliceRequest {
  input_path: string;
  printer_id: string;
  confirm_printer_mismatch: boolean;
}

export interface SliceTask {
  task_id: string;
  state: SliceTaskState;
  phase: SlicePhase;
  percent: number | null;
  project_id: string | null;
  error_code: string | null;
}

export interface SliceProgressEvent {
  task_id: string;
  phase: SlicePhase;
  percent: number | null;
}

export interface SliceCompleteEvent {
  task_id: string;
  project_id: string;
}

export interface SliceErrorEvent {
  task_id: string;
  code: string;
  message?: string | null;
}

export type SliceEventName = "slice-progress" | "slice-complete" | "slice-error";
export type SliceEventSubscriber = (
  name: SliceEventName,
  handler: (payload: unknown) => void,
) => Promise<() => void> | (() => void);

export type SliceFileDropEvent =
  | { type: "enter"; paths: string[]; position: { x: number; y: number } }
  | { type: "over"; position: { x: number; y: number } }
  | { type: "drop"; paths: string[]; position: { x: number; y: number } }
  | { type: "leave" };

export type SliceFileDropSubscriber = (
  handler: (event: SliceFileDropEvent) => void,
) => Promise<() => void> | (() => void);

export interface SliceApi {
  listSavedPrinters(): Promise<SlicePrinter[]>;
  inspect3mf(path: string): Promise<SliceInspection>;
  startSlice(request: SliceRequest): Promise<SliceTask>;
  cancelSlice(taskId: string): Promise<void>;
  getSliceTask(taskId: string): Promise<SliceTask>;
  openInBambuStudio(path: string): Promise<void>;
  getDesktopPlatform(): Promise<"macos" | "windows" | "unsupported">;
  setBambuStudioPath(path: string): Promise<void>;
}

export interface SliceProps {
  api: SliceApi;
  pickInput(): Promise<string | null>;
  pickBambuStudio?(): Promise<string | null>;
  subscribeEvent: SliceEventSubscriber;
  subscribeFileDrop?: SliceFileDropSubscriber;
  onProjectComplete(projectId: string): void;
  onSlicedFile?(path: string): void;
  initialInputPath?: string | null;
  initialInputNonce?: number;
  preferredPrinterId?: string | null;
  preferredPrinterNonce?: number;
  active?: boolean;
  activeTask?: SliceTask | null;
  onTaskChange?(task: SliceTask | null): void;
  onFormLockChange?(locked: boolean): void;
}

type ViewState =
  | "idle"
  | "inspecting"
  | "ready"
  | "starting"
  | "running"
  | "stopping"
  | "cancelled"
  | "failed"
  | "complete";

type CopyKey = keyof typeof COPY["zh-CN"];

const COPY = {
  "zh-CN": {
    title: "快速切片",
    hint: "调用本机 Bambu Studio 的切片引擎，全程在后台完成，不会自动打开 Studio。",
    select3mf: "选择 3MF",
    change3mf: "更换 3MF",
    clearSetup: "取消本次切片",
    dropTitle: "把普通 3MF 放到这里",
    dropHint: "支持拖放或从访达选择；已切片文件会直接进入打印任务。",
    inspecting: "正在检查 3MF…",
    inspectionHint: "正在读取内嵌机型、打印盘和耗材信息",
    invalidExtension: "请选择一个 .3mf 文件",
    slicedFile: "这是已切片的文件，请直接导入打印任务。",
    noPrinters: "还没有可用的打印机，请先在“我的打印机”中保存一台设备。",
    printer: "目标打印机",
    printerSection: "打印机",
    processSection: "工艺",
    process: "工艺与层高",
    plate: "打印板",
    infill: "填充率",
    infillUnit: "%",
    support: "生成支撑",
    supportHint: "由 Bambu Studio 根据当前工艺生成支撑结构",
    materialSection: "逐工具耗材",
    materialHint: "直接使用 3MF 保存的耗材设置，不在 CYLUNE 中替换。",
    nativeSettings: "3MF 项目设置",
    nativeSettingsHint: "以下内容只读；切片时会按 3MF 原样使用。",
    supportOn: "已开启",
    supportOff: "已关闭",
    unknownValue: "未提供",
    toolFallback: "颜色 {{number}}",
    filamentForTool: "{{tool}} 耗材",
    start: "开始后台切片",
    starting: "正在启动…",
    cancel: "取消切片",
    stopping: "正在停止…",
    cancelled: "切片已取消",
    cancelledHint: "Bambu Studio 后台进程已经退出，项目设置没有被修改。",
    retry: "重新尝试",
    chooseBambuStudio: "选择 BambuStudio.exe",
    openStudio: "使用 Bambu Studio 打开",
    failedTitle: "Bambu Studio 无法完成这个项目",
    failedHint: "可以重新尝试，或手动交给 Bambu Studio 检查；软件绝不会自动打开它。",
    loadFailed: "无法读取本地切片配置",
    openFailed: "无法打开 Bambu Studio",
    mismatchTitle: "目标机型与项目不同",
    mismatch: "项目内嵌机型是 {{embedded}}，当前目标是 {{target}}。请确认后再切片。",
    mismatchConfirm: "确认改用{{target}}",
    compatible: "与{{target}}兼容",
    unknownMachine: "项目没有内嵌机型，将使用{{target}}的官方配置",
    unavailablePrinter: "当前 Bambu Studio 中找不到这台打印机的官方配置",
    oneProject: "{{count}} 个打印盘 · 切片后仍是 1 个项目",
    onePlate: "1 个打印盘 · 切片后是 1 个项目",
    projectTogether: "所有打印盘会保存在同一个项目中",
    preparing: "正在准备切片环境",
    slicing: "正在切片",
    validating: "正在验证切片结果",
    importing: "正在导入打印任务",
    complete: "切片完成",
    completeHint: "彩色预览和每盘数据已合并到同一个项目。",
    percent: "{{percent}}%",
    progressLabel: "切片进度",
    loadingPresets: "正在加载官方快速预设…",
    embedded: "3MF 内嵌",
  },
  "zh-TW": {
    title: "快速切片",
    hint: "呼叫本機 Bambu Studio 的切片引擎，全程在背景完成，不會自動開啟 Studio。",
    select3mf: "選擇 3MF",
    change3mf: "更換 3MF",
    clearSetup: "取消本次切片",
    dropTitle: "把普通 3MF 放到這裡",
    dropHint: "支援拖放或從 Finder 選擇；已切片檔案會直接進入列印任務。",
    inspecting: "正在檢查 3MF…",
    inspectionHint: "正在讀取內嵌機型、列印盤和耗材資訊",
    invalidExtension: "請選擇一個 .3mf 檔案",
    slicedFile: "這是已切片的檔案，請直接匯入列印任務。",
    noPrinters: "還沒有可用的印表機，請先在「我的印表機」中儲存一台裝置。",
    printer: "目標印表機",
    printerSection: "印表機",
    processSection: "工藝",
    process: "工藝與層高",
    plate: "列印板",
    infill: "填充率",
    infillUnit: "%",
    support: "產生支撐",
    supportHint: "由 Bambu Studio 依照目前工藝產生支撐結構",
    materialSection: "逐工具耗材",
    materialHint: "直接使用 3MF 儲存的耗材設定，不在 CYLUNE 中替換。",
    nativeSettings: "3MF 專案設定",
    nativeSettingsHint: "以下內容唯讀；切片時會依照 3MF 原樣使用。",
    supportOn: "已開啟",
    supportOff: "已關閉",
    unknownValue: "未提供",
    toolFallback: "顏色 {{number}}",
    filamentForTool: "{{tool}}耗材",
    start: "開始背景切片",
    starting: "正在啟動…",
    cancel: "取消切片",
    stopping: "正在停止…",
    cancelled: "切片已取消",
    cancelledHint: "Bambu Studio 背景程序已經結束，專案設定沒有被修改。",
    retry: "重新嘗試",
    chooseBambuStudio: "選擇 BambuStudio.exe",
    openStudio: "使用 Bambu Studio 開啟",
    failedTitle: "Bambu Studio 無法完成這個專案",
    failedHint: "可以重新嘗試，或手動交給 Bambu Studio 檢查；軟體絕不會自動開啟它。",
    loadFailed: "無法讀取本機切片設定",
    openFailed: "無法開啟 Bambu Studio",
    mismatchTitle: "目標機型與專案不同",
    mismatch: "專案內嵌機型是 {{embedded}}，目前目標是 {{target}}。請確認後再切片。",
    mismatchConfirm: "確認改用{{target}}",
    compatible: "與{{target}}相容",
    unknownMachine: "專案沒有內嵌機型，將使用{{target}}的官方設定",
    unavailablePrinter: "目前 Bambu Studio 中找不到這台印表機的官方設定",
    oneProject: "{{count}} 個列印盤 · 切片後仍是 1 個專案",
    onePlate: "1 個列印盤 · 切片後是 1 個專案",
    projectTogether: "所有列印盤會儲存在同一個專案中",
    preparing: "正在準備切片環境",
    slicing: "正在切片",
    validating: "正在驗證切片結果",
    importing: "正在匯入列印任務",
    complete: "切片完成",
    completeHint: "彩色預覽和每盤資料已合併到同一個專案。",
    percent: "{{percent}}%",
    progressLabel: "切片進度",
    loadingPresets: "正在載入官方快速預設…",
    embedded: "3MF 內嵌",
  },
  en: {
    title: "Quick Slice",
    hint: "Uses Bambu Studio's local slicing engine in the background without opening Studio automatically.",
    select3mf: "Choose 3MF",
    change3mf: "Change 3MF",
    clearSetup: "Cancel this slice",
    dropTitle: "Drop an unsliced 3MF here",
    dropHint: "Drop a file or choose one from Finder. Sliced files go straight to Print Jobs.",
    inspecting: "Inspecting 3MF…",
    inspectionHint: "Reading the embedded machine, plates, and filament information",
    invalidExtension: "Choose a .3mf file",
    slicedFile: "This file is already sliced. Import it directly into Print Jobs.",
    noPrinters: "No available printer yet. Save one in My Printers first.",
    printer: "Target printer",
    printerSection: "Printer",
    processSection: "Process",
    process: "Process and layer height",
    plate: "Build plate",
    infill: "Infill density",
    infillUnit: "%",
    support: "Generate supports",
    supportHint: "Bambu Studio generates supports using the selected process",
    materialSection: "Filament by tool",
    materialHint: "Uses the filament settings saved in the 3MF without replacing them in CYLUNE.",
    nativeSettings: "3MF project settings",
    nativeSettingsHint: "These values are read-only and are used exactly as saved in the 3MF.",
    supportOn: "Enabled",
    supportOff: "Disabled",
    unknownValue: "Not provided",
    toolFallback: "Color {{number}}",
    filamentForTool: "Filament for {{tool}}",
    start: "Start background slicing",
    starting: "Starting…",
    cancel: "Cancel slicing",
    stopping: "Stopping…",
    cancelled: "Slicing cancelled",
    cancelledHint: "The Bambu Studio background process has exited without changing the project settings.",
    retry: "Try again",
    chooseBambuStudio: "Choose BambuStudio.exe",
    openStudio: "Open in Bambu Studio",
    failedTitle: "Bambu Studio couldn't slice this project",
    failedHint: "Try again or explicitly open it in Bambu Studio. CYLUNE never opens it automatically.",
    loadFailed: "Couldn't load the local slicing configuration",
    openFailed: "Couldn't open Bambu Studio",
    mismatchTitle: "The target printer is different",
    mismatch: "This project embeds {{embedded}}, while the target is {{target}}. Confirm before slicing.",
    mismatchConfirm: "Confirm using {{target}}",
    compatible: "Compatible with {{target}}",
    unknownMachine: "No machine is embedded; CYLUNE will use the official {{target}} profile",
    unavailablePrinter: "Bambu Studio no longer contains this printer profile",
    oneProject: "{{count}} plates · still 1 project after slicing",
    onePlate: "1 plate · 1 project after slicing",
    projectTogether: "All plates stay together in one project",
    preparing: "Preparing the slicing environment",
    slicing: "Slicing",
    validating: "Validating the sliced result",
    importing: "Importing the print job",
    complete: "Slicing complete",
    completeHint: "Color previews and every plate are together in one project.",
    percent: "{{percent}}%",
    progressLabel: "Slicing progress",
    loadingPresets: "Loading official quick presets…",
    embedded: "Embedded in 3MF",
  },
} as const;

function makeCopy(locale: SupportedLocale) {
  return (key: CopyKey, values: Record<string, string | number> = {}) =>
    Object.entries(values).reduce(
      (message, [name, value]) => message.split(`{{${name}}}`).join(String(value)),
      COPY[locale][key] as string,
    );
}

function stableError(error: unknown): string {
  if (typeof error === "string") {
    try { return stableError(JSON.parse(error)); }
    catch { return error; }
  }
  if (error && typeof error === "object") {
    const candidate = error as { code?: unknown; message?: unknown };
    if (typeof candidate.message === "string") return candidate.message;
    if (typeof candidate.code === "string") return candidate.code;
  }
  return "unknown";
}

function fileName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

function droppedPath(event: DragEvent<HTMLElement>): string | null {
  const file = event.dataTransfer.files?.[0] as (File & { path?: string }) | undefined;
  if (file?.path) return file.path;
  const text = event.dataTransfer.getData("text/uri-list")
    || event.dataTransfer.getData("text/plain");
  if (!text) return null;
  const first = text.split(/\r?\n/).find((line) => line && !line.startsWith("#"));
  if (!first) return null;
  try {
    return first.startsWith("file://") ? decodeURIComponent(new URL(first).pathname) : first;
  } catch {
    return first;
  }
}

const subscribeTauriFileDrop: SliceFileDropSubscriber = async (handler) => {
  if (!("__TAURI_INTERNALS__" in globalThis)) return () => undefined;
  return getCurrentWebview().onDragDropEvent((event) => handler(event.payload));
};

const pickBambuStudioExecutable = async (): Promise<string | null> => {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "BambuStudio.exe", extensions: ["exe"] }],
  });
  return typeof selected === "string" ? selected : null;
};

function isInsideDropZone(
  position: { x: number; y: number },
  zone: HTMLElement,
): boolean {
  // Wry reports NSView point coordinates on macOS even though Tauri's public
  // type names them PhysicalPosition. Windows reports client pixels.
  const macOS = /Macintosh|Mac OS X/.test(navigator.userAgent);
  const scale = macOS
    ? 1
    : window.devicePixelRatio > 0
      ? window.devicePixelRatio
      : 1;
  const x = position.x / scale;
  const y = position.y / scale;
  const bounds = zone.getBoundingClientRect();
  return x >= bounds.left && x <= bounds.right && y >= bounds.top && y <= bounds.bottom;
}

function isThreeMf(path: string): boolean {
  return path.toLowerCase().endsWith(".3mf");
}

function phaseKey(phase: SlicePhase): CopyKey {
  return phase;
}

function normalizedPercent(value: number | null | undefined): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

function viewForTask(task: SliceTask): ViewState {
  if (task.state === "completed") return "complete";
  if (task.state === "failed") return "failed";
  if (task.state === "cancelled") return "cancelled";
  return "running";
}

function defaultPrinter(printers: SlicePrinter[], preferred?: string | null) {
  return printers.find((printer) => printer.printer_id === preferred && printer.is_available)
    ?? printers.find((printer) => printer.is_default && printer.is_available)
    ?? printers.find((printer) => printer.is_available)
    ?? printers[0]
    ?? null;
}

function toolsFor(inspection: SliceInspection, copy: ReturnType<typeof makeCopy>) {
  return inspection.tools.length ? inspection.tools : [{
    tool: 0,
    label: copy("toolFallback", { number: 1 }),
    material: null,
    color_hex: null,
    embedded_filament_key: null,
  }];
}

function SectionTitle({ icon, title, hint }: {
  icon: ReactNode;
  title: string;
  hint?: string;
}) {
  return <header className="slice-section-title">
    <span>{icon}</span>
    <div><h2>{title}</h2>{hint ? <p>{hint}</p> : null}</div>
  </header>;
}

export function Slice({
  api,
  pickInput,
  pickBambuStudio = pickBambuStudioExecutable,
  subscribeEvent,
  subscribeFileDrop = subscribeTauriFileDrop,
  onProjectComplete,
  onSlicedFile,
  initialInputPath = null,
  initialInputNonce = 0,
  preferredPrinterId = null,
  preferredPrinterNonce = 0,
  active = true,
  activeTask = null,
  onTaskChange,
  onFormLockChange,
}: SliceProps) {
  const locale = useLocale();
  const copy = useMemo(() => makeCopy(locale), [locale]);
  const [view, setView] = useState<ViewState>(() => activeTask ? viewForTask(activeTask) : "idle");
  const [printers, setPrinters] = useState<SlicePrinter[]>([]);
  const [printersLoading, setPrintersLoading] = useState(true);
  const [inputPath, setInputPath] = useState<string | null>(null);
  const [inspection, setInspection] = useState<SliceInspection | null>(null);
  const [selectedPrinterId, setSelectedPrinterId] = useState("");
  const [mismatchConfirmed, setMismatchConfirmed] = useState(false);
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [errorDetail, setErrorDetail] = useState<string | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(activeTask?.error_code ?? null);
  const [desktopPlatform, setDesktopPlatform] = useState<"macos" | "windows" | "unsupported">("unsupported");
  const [openingStudio, setOpeningStudio] = useState(false);
  const [settingStudio, setSettingStudio] = useState(false);
  const [progress, setProgress] = useState<{ phase: SlicePhase; percent: number }>(() => ({
    phase: activeTask?.phase ?? "preparing",
    percent: normalizedPercent(activeTask?.percent),
  }));
  const mounted = useRef(true);
  const taskRef = useRef<SliceTask | null>(activeTask);
  const completedProjects = useRef(new Set<string>());
  const inputHandoffRef = useRef<string | null>(null);
  const preferredPrinterNonceRef = useRef(-1);
  const dropZoneRef = useRef<HTMLButtonElement | null>(null);

  const selectedPrinter = useMemo(
    () => printers.find((printer) => printer.printer_id === selectedPrinterId) ?? null,
    [printers, selectedPrinterId],
  );
  const tools = useMemo(
    () => inspection ? toolsFor(inspection, copy) : [],
    [copy, inspection],
  );
  const embeddedModel = inspection?.embedded_model_key?.trim() || null;
  const embeddedNozzle = inspection?.embedded_nozzle_diameter;
  const printerMismatch = Boolean(selectedPrinter && embeddedModel && (
    embeddedModel !== selectedPrinter.model_key
      || (typeof embeddedNozzle === "number"
        && Math.abs(embeddedNozzle - selectedPrinter.nozzle_diameter) > 0.001)
  ));
  const locked = view === "starting" || view === "running" || view === "stopping";
  const formValid = Boolean(
    inputPath
      && inspection?.kind === "unsliced"
      && selectedPrinter?.is_available
      && (!printerMismatch || mismatchConfirmed),
  );

  const publishTask = (next: SliceTask | null) => {
    taskRef.current = next;
    onTaskChange?.(next);
  };

  const inspectPath = async (path: string, preserveTaskView = false) => {
    if (locked && !preserveTaskView) return;
    if (!isThreeMf(path)) {
      setError(copy("invalidExtension"));
      setErrorDetail(null);
      return;
    }
    if (!preserveTaskView) setView("inspecting");
    setInputPath(path);
    setInspection(null);
    setError(null);
    setErrorDetail(null);
    setErrorCode(null);
    setMismatchConfirmed(false);
    try {
      const result = await api.inspect3mf(path);
      if (!mounted.current) return;
      setInspection(result);
      if (result.kind === "sliced") {
        if (onSlicedFile) {
          setInspection(null);
          setInputPath(null);
          setView("idle");
          onSlicedFile(path);
          return;
        }
        setError(copy("slicedFile"));
        setView("failed");
        return;
      }
      if (!preserveTaskView) setView("ready");
    } catch (inspectError) {
      if (!mounted.current) return;
      setError(copy("loadFailed"));
      setErrorDetail(stableError(inspectError));
      setView("failed");
    }
  };

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  useEffect(() => {
    let disposed = false;
    void api.getDesktopPlatform().then((platform) => {
      if (!disposed) setDesktopPlatform(platform);
    }).catch(() => {
      if (!disposed) setDesktopPlatform("unsupported");
    });
    return () => {
      disposed = true;
    };
  }, [api]);

  useEffect(() => {
    if (!active) return;
    let disposed = false;
    setPrintersLoading(true);
    void api.listSavedPrinters().then((saved) => {
      if (disposed) return;
      setPrinters(saved);
      setSelectedPrinterId((current) => {
        const requested = preferredPrinterNonceRef.current !== preferredPrinterNonce
          ? saved.find((printer) => printer.printer_id === preferredPrinterId && printer.is_available)
          : null;
        if (requested) {
          preferredPrinterNonceRef.current = preferredPrinterNonce;
          return requested.printer_id;
        }
        if (saved.some((printer) => printer.printer_id === current)) return current;
        return defaultPrinter(saved, preferredPrinterId)?.printer_id ?? "";
      });
    }).catch((loadError) => {
      if (!disposed) {
        setError(copy("loadFailed"));
        setErrorDetail(stableError(loadError));
      }
    }).finally(() => {
      if (!disposed) setPrintersLoading(false);
    });
    return () => {
      disposed = true;
    };
  }, [active, api, copy, preferredPrinterId, preferredPrinterNonce]);

  useEffect(() => {
    if (!activeTask) return;
    const percent = taskRef.current?.task_id === activeTask.task_id
      ? Math.max(normalizedPercent(taskRef.current.percent), normalizedPercent(activeTask.percent))
      : normalizedPercent(activeTask.percent);
    taskRef.current = { ...activeTask, percent };
    setProgress({
      phase: activeTask.phase,
      percent,
    });
    setView(viewForTask(activeTask));
    if (activeTask.state === "failed") {
      setError(copy("failedTitle"));
      setErrorCode(activeTask.error_code);
    }
  }, [activeTask, copy]);

  useEffect(() => {
    if (!initialInputPath) return;
    const handoff = `${initialInputNonce}\u0000${initialInputPath}`;
    if (inputHandoffRef.current === handoff) return;
    inputHandoffRef.current = handoff;
    void inspectPath(initialInputPath, Boolean(activeTask));
    // inspectPath deliberately consumes this explicit path/nonce hand-off only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialInputNonce, initialInputPath]);

  useEffect(() => {
    onFormLockChange?.(locked);
    return () => onFormLockChange?.(false);
  }, [locked, onFormLockChange]);

  useEffect(() => {
    if (!active || locked) return;
    let disposed = false;
    let stop: (() => void) | undefined;
    void Promise.resolve(subscribeFileDrop((event) => {
      const zone = dropZoneRef.current;
      if (!zone) return;
      if (event.type === "leave") {
        setDragging(false);
        return;
      }
      const inside = isInsideDropZone(event.position, zone);
      if (event.type === "enter" || event.type === "over") {
        setDragging(inside);
        return;
      }
      setDragging(false);
      if (inside && event.paths[0]) void inspectPath(event.paths[0]);
    })).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    }).catch(() => undefined);
    return () => {
      disposed = true;
      stop?.();
      setDragging(false);
    };
  }, [active, locked, subscribeFileDrop]);

  useEffect(() => {
    let disposed = false;
    const stops: Array<() => void> = [];
    const register = async (name: SliceEventName, handler: (payload: unknown) => void) => {
      const stop = await subscribeEvent(name, handler);
      if (disposed) stop();
      else stops.push(stop);
    };
    void Promise.all([
      register("slice-progress", (payload) => {
        const event = payload as Partial<SliceProgressEvent>;
        if (!taskRef.current || event.task_id !== taskRef.current.task_id) return;
        if (!event.phase) return;
        const percent = Math.max(
          normalizedPercent(taskRef.current.percent),
          normalizedPercent(event.percent),
        );
        setProgress({ phase: event.phase, percent });
        const next = { ...taskRef.current, phase: event.phase, percent };
        publishTask(next);
      }),
      register("slice-complete", (payload) => {
        const event = payload as Partial<SliceCompleteEvent>;
        if (!taskRef.current || event.task_id !== taskRef.current.task_id || !event.project_id) return;
        const next: SliceTask = {
          ...taskRef.current,
          state: "completed",
          phase: "complete",
          percent: 100,
          project_id: event.project_id,
          error_code: null,
        };
        publishTask(next);
        setProgress({ phase: "complete", percent: 100 });
        setView("complete");
        if (!completedProjects.current.has(event.project_id)) {
          completedProjects.current.add(event.project_id);
          onProjectComplete(event.project_id);
        }
      }),
      register("slice-error", (payload) => {
        const event = payload as Partial<SliceErrorEvent>;
        if (!taskRef.current || event.task_id !== taskRef.current.task_id) return;
        if (event.code === "slicer_cancelled") {
          const next: SliceTask = {
            ...taskRef.current,
            state: "cancelled",
            error_code: event.code,
          };
          publishTask(next);
          setView("cancelled");
          return;
        }
        const next: SliceTask = {
          ...taskRef.current,
          state: "failed",
          error_code: event.code ?? "slicer_failed",
        };
        publishTask(next);
        setError(copy("failedTitle"));
        setErrorDetail(event.message ?? event.code ?? null);
        setErrorCode(event.code ?? "slicer_failed");
        setView("failed");
      }),
    ]).catch(() => undefined);
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, [copy, onProjectComplete, onTaskChange, subscribeEvent]);

  const chooseInput = async () => {
    if (locked) return;
    const path = await pickInput();
    if (path) await inspectPath(path);
  };

  const startConfiguredSlice = async () => {
    if (!formValid || locked || !inputPath) return;
    setError(null);
    setErrorDetail(null);
    setErrorCode(null);
    setProgress({ phase: "preparing", percent: 0 });
    setView("starting");
    try {
      const next = await api.startSlice({
        input_path: inputPath,
        printer_id: selectedPrinterId,
        confirm_printer_mismatch: printerMismatch && mismatchConfirmed,
      });
      if (!mounted.current) return;
      const normalizedTask = { ...next, percent: normalizedPercent(next.percent) };
      publishTask(normalizedTask);
      setProgress({
        phase: normalizedTask.phase,
        percent: normalizedTask.percent,
      });
      setView("running");
    } catch (startError) {
      if (!mounted.current) return;
      const code = stableError(startError);
      setError(copy("failedTitle"));
      setErrorDetail(code);
      setErrorCode(code);
      setView("failed");
    }
  };

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    await startConfiguredSlice();
  };

  const chooseBambuStudio = async () => {
    if (desktopPlatform !== "windows" || errorCode !== "bambu_studio_missing" || settingStudio) {
      return;
    }
    const path = await pickBambuStudio();
    if (!mounted.current || !path) return;
    setSettingStudio(true);
    try {
      await api.setBambuStudioPath(path);
      if (mounted.current) await startConfiguredSlice();
    } catch (selectionError) {
      if (mounted.current) {
        const code = stableError(selectionError);
        setError(copy("failedTitle"));
        setErrorDetail(code);
        setErrorCode(code);
        setView("failed");
      }
    } finally {
      if (mounted.current) setSettingStudio(false);
    }
  };

  const cancel = async () => {
    const current = taskRef.current;
    if (!current || view !== "running") return;
    setView("stopping");
    try {
      await api.cancelSlice(current.task_id);
      if (!mounted.current || taskRef.current?.task_id !== current.task_id) return;
      const next = await api.getSliceTask(current.task_id);
      if (!mounted.current || taskRef.current?.task_id !== current.task_id) return;
      const normalizedTask = {
        ...next,
        percent: Math.max(
          normalizedPercent(taskRef.current?.percent),
          normalizedPercent(next.percent),
        ),
      };
      publishTask(normalizedTask);
      setProgress({
        phase: normalizedTask.phase,
        percent: normalizedTask.percent,
      });
      if (next.state === "completed" && next.project_id) {
        onProjectComplete(next.project_id);
        return;
      }
      setView(viewForTask(next));
    } catch (cancelError) {
      if (!mounted.current) return;
      setError(copy("failedTitle"));
      setErrorDetail(stableError(cancelError));
      setView("failed");
    }
  };

  const openStudio = async () => {
    if (!inputPath || openingStudio) return;
    setOpeningStudio(true);
    try {
      await api.openInBambuStudio(inputPath);
    } catch (openError) {
      if (mounted.current) {
        setError(copy("openFailed"));
        setErrorDetail(stableError(openError));
      }
    } finally {
      if (mounted.current) setOpeningStudio(false);
    }
  };

  const handleDrop = (event: DragEvent<HTMLElement>) => {
    event.preventDefault();
    setDragging(false);
    if (locked) return;
    const path = droppedPath(event);
    if (path) void inspectPath(path);
  };

  const resetFailure = () => {
    setError(null);
    setErrorDetail(null);
    setErrorCode(null);
    setView(inspection?.kind === "unsliced" ? "ready" : "idle");
  };

  const clearSetup = () => {
    if (locked) return;
    publishTask(null);
    setView("idle");
    setInputPath(null);
    setInspection(null);
    setError(null);
    setErrorDetail(null);
    setErrorCode(null);
    setMismatchConfirmed(false);
    setProgress({ phase: "preparing", percent: 0 });
  };

  const projectMeta = inspection
    ? copy(inspection.plate_count === 1 ? "onePlate" : "oneProject", {
      count: Math.max(inspection.plate_count, 1),
    })
    : "";
  const nativeValue = (value?: string | null) => value?.trim() || copy("unknownValue");
  const printerValue = selectedPrinter
    ? `${selectedPrinter.display_name} · ${selectedPrinter.model_key} · ${selectedPrinter.nozzle_diameter.toFixed(1)} mm`
    : copy("unknownValue");
  const infillValue = typeof inspection?.embedded_infill_density === "number"
    && Number.isFinite(inspection.embedded_infill_density)
    ? copy("percent", { percent: inspection.embedded_infill_density })
    : copy("unknownValue");
  const supportValue = typeof inspection?.embedded_support_enabled === "boolean"
    ? copy(inspection.embedded_support_enabled ? "supportOn" : "supportOff")
    : copy("unknownValue");

  return <section className="page slice-page" aria-labelledby="slice-title">
    <div className="page-heading slice-heading">
      <div><h1 id="slice-title">{copy("title")}</h1><p>{copy("hint")}</p></div>
      {inspection ? <div className="slice-heading-actions">
        {!locked ? <button className="ghost" type="button" onClick={clearSetup}><XCircle size={18} weight="bold" />{copy("clearSetup")}</button> : null}
        <button className="ghost" type="button" disabled={locked} onClick={() => void chooseInput()}><FileArrowUp size={18} weight="bold" />{copy("change3mf")}</button>
      </div> : null}
    </div>

    {!inspection && view !== "inspecting" ? <button
      ref={dropZoneRef}
      type="button"
      className={`slice-drop${dragging ? " is-dragging" : ""}`}
      data-testid="slice-drop-zone"
      aria-label={copy("select3mf")}
      disabled={locked}
      onClick={() => void chooseInput()}
      onDragEnter={(event) => { event.preventDefault(); if (!locked) setDragging(true); }}
      onDragOver={(event) => event.preventDefault()}
      onDragLeave={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setDragging(false);
      }}
      onDrop={handleDrop}
    >
      <span className="slice-drop-orbit"><FileArrowUp size={34} weight="duotone" /></span>
      <strong>{copy("dropTitle")}</strong>
      <small>{copy("dropHint")}</small>
      <span className="primary slice-drop-action">{copy("select3mf")}</span>
    </button> : null}

    {view === "inspecting" ? <div className="slice-inspecting" aria-live="polite">
      <span className="slice-spinner"><Cube size={34} weight="duotone" /></span>
      <h2>{copy("inspecting")}</h2>
      <p>{copy("inspectionHint")}</p>
    </div> : null}

    {inspection ? <>
      <article className="slice-file-card">
        <span className="slice-file-icon"><Cube size={27} weight="duotone" /></span>
        <div><h2>{inspection.file_name || fileName(inputPath ?? "")}</h2><p>{projectMeta}</p></div>
        <span className="slice-project-pill"><Stack size={14} weight="bold" />1</span>
      </article>

      {view === "running" || view === "stopping" || view === "starting" ? <section className="slice-progress-card" aria-live="polite">
        <div className="slice-progress-copy">
          <span className="slice-pulse" aria-hidden="true" />
          <div><strong>{copy(phaseKey(progress.phase))}</strong><small>{copy("progressLabel")}</small></div>
          <b className="data">{copy("percent", { percent: Math.round(progress.percent) })}</b>
        </div>
        <progress aria-label={copy("progressLabel")} max={100} value={progress.percent} />
        <button type="button" className="ghost small" disabled={view !== "running"} onClick={() => void cancel()}>
          <StopCircle size={16} weight="bold" />{view === "stopping" ? copy("stopping") : copy("cancel")}
        </button>
      </section> : null}

      {view === "cancelled" ? <div className="slice-status-note cancelled" role="status">
        <XCircle size={21} weight="fill" />
        <div><strong>{copy("cancelled")}</strong><small>{copy("cancelledHint")}</small></div>
      </div> : null}

      {view === "complete" ? <div className="slice-status-note complete" role="status">
        <CheckCircle size={22} weight="fill" />
        <div><strong>{copy("complete")}</strong><small>{copy("completeHint")}</small></div>
      </div> : null}

      {error && view === "failed" ? <div className="slice-error" role="alert">
        <WarningCircle size={23} weight="fill" />
        <div><strong>{error === copy("failedTitle") ? error : copy("failedTitle")}</strong><p>{error === copy("failedTitle") ? copy("failedHint") : error}</p>{errorDetail ? <small>{errorDetail}</small> : null}</div>
        <div className="slice-error-actions">
          {inspection.kind === "unsliced" ? <button type="button" className="ghost small" onClick={resetFailure}><ArrowClockwise size={15} />{copy("retry")}</button> : null}
          {desktopPlatform === "windows" && errorCode === "bambu_studio_missing" ? <button type="button" className="secondary small" disabled={settingStudio} onClick={() => void chooseBambuStudio()}><FolderOpen size={15} />{copy("chooseBambuStudio")}</button> : null}
          {inputPath ? <button type="button" className="secondary small" disabled={openingStudio} onClick={() => void openStudio()}><FolderOpen size={15} />{copy("openStudio")}</button> : null}
        </div>
      </div> : null}

      <form className="slice-form" onSubmit={(event) => void submit(event)}>
        <fieldset disabled={locked}>
          <section className="slice-form-section">
            <SectionTitle icon={<Printer size={21} weight="duotone" />} title={copy("printerSection")} />
            <div className="slice-summary-row"><span>{copy("printer")}</span><strong>{printerValue}</strong></div>
            {!printersLoading && !printers.length ? <div className="slice-inline-note warning"><WarningCircle size={17} weight="fill" />{copy("noPrinters")}</div> : null}
            {selectedPrinter && !selectedPrinter.is_available ? <div className="slice-inline-note warning"><WarningCircle size={17} weight="fill" />{copy("unavailablePrinter")}</div> : null}
            {selectedPrinter && printerMismatch ? <div className="slice-mismatch" role="alert">
              <WarningCircle size={20} weight="fill" />
              <div><strong>{copy("mismatchTitle")}</strong><p>{copy("mismatch", { embedded: `${embeddedModel}${typeof embeddedNozzle === "number" ? ` · ${embeddedNozzle.toFixed(1)} mm` : ""}`, target: selectedPrinter.display_name })}</p>
                <label><input type="checkbox" checked={mismatchConfirmed} onChange={(event) => setMismatchConfirmed(event.target.checked)} /><span>{copy("mismatchConfirm", { target: selectedPrinter.display_name })}</span></label>
              </div>
            </div> : null}
            {selectedPrinter && !printerMismatch ? <div className="slice-inline-note compatible"><CheckCircle size={17} weight="fill" />{embeddedModel ? copy("compatible", { target: selectedPrinter.display_name }) : copy("unknownMachine", { target: selectedPrinter.display_name })}</div> : null}
          </section>

          <section className="slice-form-section">
            <SectionTitle icon={<SlidersHorizontal size={21} weight="duotone" />} title={copy("nativeSettings")} hint={copy("nativeSettingsHint")} />
            <div className="slice-summary">
              <div className="slice-summary-row"><span>{copy("process")}</span><strong>{nativeValue(inspection.embedded_process_key)}</strong></div>
              <div className="slice-summary-row"><span>{copy("plate")}</span><strong>{nativeValue(inspection.embedded_plate_key)}</strong></div>
              <div className="slice-summary-row"><span>{copy("infill")}</span><strong>{infillValue}</strong></div>
              <div className="slice-summary-row"><span>{copy("support")}</span><strong>{supportValue}</strong></div>
            </div>
          </section>

          <section className="slice-form-section">
            <SectionTitle icon={<Palette size={21} weight="duotone" />} title={copy("materialSection")} hint={copy("materialHint")} />
            <div className="slice-tools">
              {tools.map((tool) => {
                const toolName = tool.label?.trim() || copy("toolFallback", { number: tool.tool + 1 });
                const filamentName = nativeValue(tool.embedded_filament_key);
                return <div className="slice-tool slice-tool-readonly" key={tool.tool}>
                  <span className="slice-tool-color" style={{ "--tool-color": tool.color_hex || "var(--blue)" } as CSSProperties} />
                  <span className="slice-tool-copy"><strong>{toolName}</strong><small>{tool.material || copy("embedded")}</small></span>
                  <span className="slice-tool-native">{filamentName === toolName ? copy("embedded") : filamentName}</span>
                </div>;
              })}
            </div>
          </section>

          <footer className="slice-form-actions">
            <span><Info size={16} weight="fill" />{copy("projectTogether")}</span>
            <button type="submit" className="primary" disabled={!formValid || locked}><Play size={17} weight="fill" />{view === "starting" ? copy("starting") : copy("start")}</button>
          </footer>
        </fieldset>
      </form>
    </> : null}

    {error && !inspection ? <div className="slice-error standalone" role="alert"><WarningCircle size={22} weight="fill" /><div><strong>{error}</strong>{errorDetail ? <small>{errorDetail}</small> : null}</div></div> : null}
  </section>;
}
