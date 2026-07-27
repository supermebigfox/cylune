import { Bell, Database, DownloadSimple, FolderSimple, Moon, ShieldCheck, Sun, UploadSimple } from "@phosphor-icons/react";
import { useEffect, useState } from "react";
import { setLocale, supportedLocales, t, useLocale, type SupportedLocale } from "../../i18n";
import { pickBackupDestination, pickBackupToImport, pickWatchFolder } from "../../lib/dialog";
import { api, type TauriApi } from "../../lib/tauri";
import { useTheme } from "../../theme/Theme";

export function Settings({ apiClient=api, onRestored }: { apiClient?: TauriApi; onRestored?:()=>void|Promise<void> }) {
  const locale = useLocale();
  const { theme, setTheme } = useTheme();
  const copy = (key: string) => t(key, {}, locale);
  const [watchFolder,setWatchFolder]=useState<string|null>(null);
  const [message,setMessage]=useState<string|null>(null);
  useEffect(()=>{void apiClient.getWatchFolder?.().then(setWatchFolder).catch(()=>undefined)},[apiClient]);
  const chooseWatch=async()=>{const folder=await pickWatchFolder();if(folder&&apiClient.setWatchFolder){setWatchFolder(await apiClient.setWatchFolder(folder));setMessage(copy("settings.watchEnabled"));}};
  const disableWatch=async()=>{await apiClient.setWatchFolder?.(null);setWatchFolder(null);setMessage(copy("settings.watchDisabled"));};
  const exportNow=async()=>{const path=await pickBackupDestination();if(path&&apiClient.exportBackup){await apiClient.exportBackup(path);setMessage(copy("settings.backupDone"));}};
  const importNow=async()=>{const path=await pickBackupToImport();if(path&&apiClient.importBackup){await apiClient.importBackup(path);await onRestored?.();setMessage(copy("settings.restoreDone"));}};
  return <section className="page settings-page" aria-labelledby="settings-title">
    <div className="page-heading"><div><h1 id="settings-title">{copy("settings.title")}</h1><p>{copy("settings.hint")}</p></div></div>
    <div className="settings-layout">
      <div className="settings-main">
        <section className="setting-group"><h2>{copy("settings.language")}</h2><div className="segmented three">{supportedLocales.map((item) => <button className={locale === item ? "active" : ""} key={item} onClick={() => setLocale(item as SupportedLocale)}>{copy(`locale.${item === "zh-CN" ? "zhCN" : item === "zh-TW" ? "zhTW" : "en"}`)}</button>)}</div></section>
        <section className="setting-group"><h2>{copy("settings.appearance")}</h2><div className="segmented"><button className={theme === "light" ? "active" : ""} onClick={() => setTheme("light")}><Sun size={17} />{copy("settings.light")}</button><button className={theme === "dark" ? "active" : ""} onClick={() => setTheme("dark")}><Moon size={17} />{copy("settings.dark")}</button></div></section>
        <section className="setting-group action-setting"><div><FolderSimple size={21} /><span><h2>{copy("settings.watchFolder")}</h2><p>{watchFolder ?? copy("settings.watchHint")}</p></span></div><div className="setting-actions"><button onClick={chooseWatch}>{watchFolder?copy("settings.change"):copy("settings.enable")}</button>{watchFolder?<button onClick={disableWatch}>{copy("settings.disable")}</button>:null}</div></section>
        <section className="setting-group action-setting"><div><Bell size={21} /><span><h2>{copy("settings.notifications")}</h2><p>{copy("settings.notificationHint")}</p></span></div><b>{copy("settings.localNotifications")}</b></section>
        <section className="setting-group action-setting"><div><Database size={21} /><span><h2>{copy("settings.backup")}</h2><p>{copy("settings.backupHint")}</p></span></div><div className="setting-actions"><button onClick={exportNow}><DownloadSimple size={16}/>{copy("settings.export")}</button><button onClick={importNow}><UploadSimple size={16}/>{copy("settings.restore")}</button></div></section>
        {message?<p className="setting-message" role="status">{message}</p>:null}
      </div>
      <aside className="privacy-card"><ShieldCheck size={32} weight="duotone" /><h2>{copy("settings.localTitle")}</h2><p>{copy("settings.localPrivacy")}</p><dl><div><dt>{copy("settings.data")}</dt><dd>inventory.sqlite</dd></div><div><dt>{copy("settings.network")}</dt><dd>{copy("settings.offline")}</dd></div></dl></aside>
    </div>
  </section>;
}
