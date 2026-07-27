import { ArrowsClockwise, Eye, EyeSlash } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { t, useLocale } from "../../i18n";
import { api, type PetFps, type PetMode, type PetSettings, type PetSettingsPatch, type TauriApi } from "../../lib/tauri";

const defaultPet: PetSettings = {
  mode: "lite", size: 220, fps: "auto", visible: true, x: null, y: null,
  display_id: null, effective_mode: "lite", permission: "unavailable",
  fallback_reason: "native_not_started",
};

export function Pet({ apiClient = api }: { apiClient?: TauriApi }) {
  const locale = useLocale();
  const copy = (key: string) => t(key, {}, locale);
  const [settings, setSettings] = useState<PetSettings>(defaultPet);
  const [error, setError] = useState<string | null>(null);
  const confirmed = useRef(defaultPet);
  const current = useRef(defaultPet);
  const writes = useRef<Promise<void>>(Promise.resolve());
  const changeVersion = useRef(0);

  useEffect(() => {
    if (apiClient.mode === "demo") return;
    let active = true;
    void apiClient.getPetSettings?.().then((value) => {
      if (!active) return;
      confirmed.current = value;
      current.current = value;
      setSettings(value);
    }).catch(() => {
      if (active) setError(copy("pet.saveError"));
    });
    return () => { active = false; };
  }, [apiClient, locale]);

  const save = (patch: PetSettingsPatch) => {
    const version = ++changeVersion.current;
    const next = { ...current.current, ...patch };
    current.current = next;
    setSettings(next);
    setError(null);
    writes.current = writes.current.catch(() => undefined).then(async () => {
      if (!apiClient.setPetSettings) return;
      try {
        const saved = await apiClient.setPetSettings(patch);
        confirmed.current = saved;
        if (version === changeVersion.current) {
          current.current = saved;
          setSettings(saved);
        }
      } catch {
        if (version === changeVersion.current) {
          current.current = confirmed.current;
          setSettings(confirmed.current);
        }
        setError(copy("pet.saveError"));
      }
    });
  };

  const permission = settings.permission === "denied"
    ? copy("pet.permissionDenied")
    : settings.permission === "restart_required" ? copy("pet.restartRequired") : null;

  return <section className="setting-group pet-settings" aria-labelledby="pet-title">
    <div className="pet-heading"><div><h2 id="pet-title">{copy("pet.title")}</h2><p>{copy("pet.powerHint")}</p></div></div>
    <div className="pet-controls">
      <fieldset><legend>{copy("pet.mode")}</legend><div className="segmented">
        <button aria-pressed={settings.mode === "real"} className={settings.mode === "real" ? "active" : ""} onClick={() => save({ mode: "real" })}>{copy("pet.real")}</button>
        <button aria-pressed={settings.mode === "lite"} className={settings.mode === "lite" ? "active" : ""} onClick={() => save({ mode: "lite" })}>{copy("pet.lite")}</button>
      </div></fieldset>
      <fieldset><legend>{copy("pet.size")}</legend><div className="pet-size-presets">
        {([160, 220, 300] as const).map((size, index) => <button key={size} aria-pressed={settings.size === size} className={settings.size === size ? "active" : ""} onClick={() => save({ size })}>{copy(["pet.small", "pet.medium", "pet.large"][index])}</button>)}
      </div><input aria-label={copy("pet.size")} min="120" max="360" step="4" type="range" value={settings.size} onChange={(event) => save({ size: Number(event.target.value) })} /></fieldset>
      <fieldset><legend>{copy("pet.fps")}</legend><div className="segmented three">
        {(["auto", "fps30", "fps60"] as const).map((fps) => <button key={fps} aria-pressed={settings.fps === fps} className={settings.fps === fps ? "active" : ""} onClick={() => save({ fps })}>{copy(`pet.${fps}`)}</button>)}
      </div></fieldset>
    </div>
    <div className="pet-actions">
      <button className="secondary small" onClick={() => save({ visible: !settings.visible })}>{settings.visible ? <EyeSlash size={16} /> : <Eye size={16} />}{settings.visible ? copy("pet.hide") : copy("pet.show")}</button>
      <button className="ghost small" onClick={() => save({ reset_position: true })}><ArrowsClockwise size={16} />{copy("pet.reset")}</button>
    </div>
    {permission ? <p className="pet-notice">{permission}</p> : null}
    {settings.fallback_reason ? <p className="pet-fallback">{settings.fallback_reason}</p> : null}
    {error ? <p className="setting-message" role="alert">{error}</p> : null}
  </section>;
}
