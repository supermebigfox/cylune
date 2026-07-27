import { Bell, Database, FolderSimple, Moon, ShieldCheck, Sun } from "@phosphor-icons/react";
import { setLocale, supportedLocales, t, useLocale, type SupportedLocale } from "../../i18n";
import { useTheme } from "../../theme/Theme";

export function Settings() {
  const locale = useLocale();
  const { theme, setTheme } = useTheme();
  const copy = (key: string) => t(key, {}, locale);
  return <section className="page settings-page" aria-labelledby="settings-title">
    <div className="page-heading"><div><h1 id="settings-title">{copy("settings.title")}</h1><p>{copy("settings.hint")}</p></div></div>
    <div className="settings-layout">
      <div className="settings-main">
        <section className="setting-group"><h2>{copy("settings.language")}</h2><div className="segmented three">{supportedLocales.map((item) => <button className={locale === item ? "active" : ""} key={item} onClick={() => setLocale(item as SupportedLocale)}>{copy(`locale.${item === "zh-CN" ? "zhCN" : item === "zh-TW" ? "zhTW" : "en"}`)}</button>)}</div></section>
        <section className="setting-group"><h2>{copy("settings.appearance")}</h2><div className="segmented"><button className={theme === "light" ? "active" : ""} onClick={() => setTheme("light")}><Sun size={17} />{copy("settings.light")}</button><button className={theme === "dark" ? "active" : ""} onClick={() => setTheme("dark")}><Moon size={17} />{copy("settings.dark")}</button></div></section>
        <section className="setting-group future"><div><FolderSimple size={21} /><span><h2>{copy("settings.watchFolder")}</h2><p>{copy("settings.watchHint")}</p></span></div><b>{copy("settings.notActive")}</b></section>
        <section className="setting-group future"><div><Bell size={21} /><span><h2>{copy("settings.notifications")}</h2><p>{copy("settings.notificationHint")}</p></span></div><b>{copy("settings.notActive")}</b></section>
        <section className="setting-group future"><div><Database size={21} /><span><h2>{copy("settings.backup")}</h2><p>{copy("settings.backupHint")}</p></span></div><b>{copy("settings.notActive")}</b></section>
      </div>
      <aside className="privacy-card"><ShieldCheck size={32} weight="duotone" /><h2>{copy("settings.localTitle")}</h2><p>{copy("settings.localPrivacy")}</p><dl><div><dt>{copy("settings.data")}</dt><dd>inventory.sqlite</dd></div><div><dt>{copy("settings.network")}</dt><dd>{copy("settings.offline")}</dd></div></dl></aside>
    </div>
  </section>;
}
