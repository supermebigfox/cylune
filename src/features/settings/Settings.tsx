import { Bell, Database, DownloadSimple, FolderSimple, Moon, ShieldCheck, Sun, UploadSimple } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { setLocale, supportedLocales, t, useLocale, type SupportedLocale } from "../../i18n";
import { pickBackupDestination, pickBackupToImport, pickWatchFolder } from "../../lib/dialog";
import { api, type TauriApi } from "../../lib/tauri";
import { useTheme } from "../../theme/Theme";
import { Pet } from "./Pet";

type SettingsDialogs = {
  watch(): Promise<string | null>;
  importBackup(filterName: string): Promise<string | null>;
  exportBackup(filterName: string, defaultName: string): Promise<string | null>;
};

const defaultDialogs: SettingsDialogs = {
  watch: pickWatchFolder,
  importBackup: pickBackupToImport,
  exportBackup: pickBackupDestination,
};

function stableErrorCode(error: unknown): string {
  let value = error;
  if (typeof value === "string") {
    try { value = JSON.parse(value); }
    catch { return "io"; }
  }
  if (value && typeof value === "object" && "code" in value) {
    const code = String((value as { code: unknown }).code);
    if (/^[a-z_]+$/.test(code)) return code;
  }
  return "io";
}

export function Settings({ apiClient=api, onRestored, dialogs=defaultDialogs }: {
  apiClient?: TauriApi;
  onRestored?:()=>void|Promise<void>;
  dialogs?: SettingsDialogs;
}) {
  const locale = useLocale();
  const { theme, setTheme } = useTheme();
  const copy = (key: string) => t(key, {}, locale);
  const [watchFolder,setWatchFolder]=useState<string|null>(null);
  const [message,setMessage]=useState<string|null>(null);
  const [error,setError]=useState<string|null>(null);
  const [busy,setBusy]=useState(false);
  const busyRef=useRef(false);
  const run=async(operation:()=>Promise<void>)=>{
    if(busyRef.current)return false;
    busyRef.current=true;setBusy(true);setError(null);setMessage(null);
    try{await operation();return true;}
    catch(reason){setError(copy(`errors.${stableErrorCode(reason)}`));return false;}
    finally{busyRef.current=false;setBusy(false);}
  };
  useEffect(()=>{void apiClient.getWatchFolder?.().then(setWatchFolder).catch((reason)=>setError(copy(`errors.${stableErrorCode(reason)}`)))},[apiClient,locale]);
  const chooseWatch=()=>run(async()=>{const folder=await dialogs.watch();if(folder&&apiClient.setWatchFolder){setWatchFolder(await apiClient.setWatchFolder(folder));setMessage(copy("settings.watchEnabled"));}});
  const disableWatch=()=>run(async()=>{await apiClient.setWatchFolder?.(null);setWatchFolder(null);setMessage(copy("settings.watchDisabled"));});
  const exportNow=()=>run(async()=>{const path=await dialogs.exportBackup(copy("settings.backupFilter"),copy("settings.backupFileName"));if(path&&apiClient.exportBackup){await apiClient.exportBackup(path);setMessage(copy("settings.backupDone"));}});
  const importNow=()=>run(async()=>{const path=await dialogs.importBackup(copy("settings.backupFilter"));if(path&&apiClient.importBackup){await apiClient.importBackup(path);await onRestored?.();setMessage(copy("settings.restoreDone"));}});
  return <section className="page settings-page" aria-labelledby="settings-title">
    <div className="page-heading"><div><h1 id="settings-title">{copy("settings.title")}</h1><p>{copy("settings.hint")}</p></div></div>
    <div className="settings-layout">
      <div className="settings-main">
        <section className="setting-group"><h2>{copy("settings.language")}</h2><div className="segmented three">{supportedLocales.map((item) => <button className={locale === item ? "active" : ""} key={item} onClick={() => setLocale(item as SupportedLocale)}>{copy(`locale.${item === "zh-CN" ? "zhCN" : item === "zh-TW" ? "zhTW" : "en"}`)}</button>)}</div></section>
        <section className="setting-group"><h2>{copy("settings.appearance")}</h2><div className="segmented"><button className={theme === "light" ? "active" : ""} onClick={() => setTheme("light")}><Sun size={17} />{copy("settings.light")}</button><button className={theme === "dark" ? "active" : ""} onClick={() => setTheme("dark")}><Moon size={17} />{copy("settings.dark")}</button></div></section>
        <Pet apiClient={apiClient} />
        <section className="setting-group action-setting"><div><FolderSimple size={21} /><span><h2>{copy("settings.watchFolder")}</h2><p>{watchFolder ?? copy("settings.watchHint")}</p></span></div><div className="setting-actions"><button disabled={busy} onClick={chooseWatch}>{watchFolder?copy("settings.change"):copy("settings.enable")}</button>{watchFolder?<button disabled={busy} onClick={disableWatch}>{copy("settings.disable")}</button>:null}</div></section>
        <section className="setting-group action-setting"><div><Bell size={21} /><span><h2>{copy("settings.notifications")}</h2><p>{copy("settings.notificationHint")}</p></span></div><b>{copy("settings.localNotifications")}</b></section>
        <section className="setting-group action-setting"><div><Database size={21} /><span><h2>{copy("settings.backup")}</h2><p>{copy("settings.backupHint")}</p></span></div><div className="setting-actions"><button disabled={busy} onClick={exportNow}><DownloadSimple size={16}/>{copy("settings.export")}</button><button disabled={busy} onClick={importNow}><UploadSimple size={16}/>{copy("settings.restore")}</button></div></section>
        {busy?<p className="setting-message" role="status">{copy("settings.working")}</p>:null}
        {error?<p className="setting-message" role="alert">{error}</p>:null}
        {message?<p className="setting-message" role="status">{message}</p>:null}
      </div>
      <aside className="privacy-card"><ShieldCheck size={32} weight="duotone" /><h2>{copy("settings.localTitle")}</h2><p>{copy("settings.localPrivacy")}</p><dl><div><dt>{copy("settings.data")}</dt><dd>inventory.sqlite</dd></div><div><dt>{copy("settings.network")}</dt><dd>{copy("settings.offline")}</dd></div></dl></aside>
    </div>
  </section>;
}
