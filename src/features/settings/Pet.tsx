import { ArrowsClockwise, Eye, EyeSlash } from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import { t, useLocale } from "../../i18n";
import { api, type PetSettings, type PetSettingsPatch, type TauriApi } from "../../lib/tauri";

const defaultPet: PetSettings = {
  mode: "lite", visual_style: "gargantua", size: 220, fps: "auto", visible: false, x: null, y: null,
  display_id: null, effective_mode: "lite", permission: "unavailable",
  fallback_reason: null,
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
    const loadVersion = changeVersion.current;
    void apiClient.getPetSettings?.().then((value) => {
      if (!active || changeVersion.current !== loadVersion) return;
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

  const enabled = settings.mode === "real";
  const statusCopy = !enabled
    ? null
    : settings.permission === "denied"
    ? copy("pet.permissionDenied")
    : settings.permission === "restart_required"
      ? copy("pet.restartRequired")
      : settings.permission === "not_determined"
        ? copy("pet.permissionNotDetermined")
        : settings.fallback_reason === "platform_unsupported"
          ? copy("pet.platformUnsupported")
          : settings.fallback_reason === "capture_failed"
            ? copy("pet.captureFailed")
            : settings.fallback_reason === "metal_unavailable"
              ? copy("pet.metalUnavailable")
            : settings.fallback_reason === "direct3d_unavailable"
              ? copy("pet.direct3dUnavailable")
            : settings.fallback_reason
              ? copy("pet.captureUnavailable")
              : null;

  return <section className="setting-group pet-settings" aria-labelledby="pet-title">
    <div className="pet-heading"><div><h2 id="pet-title">{copy("pet.title")}</h2><p>{copy("pet.powerHint")}</p></div></div>
    <div className="pet-controls">
      <fieldset><legend>{copy("pet.mode")}</legend><div className="segmented">
        <button aria-pressed={enabled} className={enabled ? "active" : ""} onClick={() => save({ mode: "real", visible: true })}>{copy("pet.on")}</button>
        <button aria-pressed={!enabled} className={!enabled ? "active" : ""} onClick={() => save({ mode: "lite", visible: false })}>{copy("pet.off")}</button>
      </div></fieldset>
      <fieldset><legend>{copy("pet.visualStyle")}</legend><div className="segmented">
        {(["gargantua", "fusion"] as const).map((visual_style) => <button key={visual_style} aria-pressed={settings.visual_style === visual_style} className={settings.visual_style === visual_style ? "active" : ""} onClick={() => save({ visual_style })}>{copy(`pet.${visual_style}`)}</button>)}
      </div></fieldset>
      <fieldset><legend>{copy("pet.size")}</legend><div className="pet-size-presets">
        {([300, 600, 900] as const).map((size) => <button key={size} aria-pressed={settings.size === size} className={settings.size === size ? "active" : ""} onClick={() => save({ size })}>{copy(`pet.size${size}`)}</button>)}
      </div><input aria-label={copy("pet.size")} min="120" max="900" step="4" type="range" value={settings.size} onChange={(event) => save({ size: Number(event.target.value) })} /></fieldset>
      <fieldset><legend>{copy("pet.fps")}</legend><div className="segmented three">
        {(["auto", "fps30", "fps60"] as const).map((fps) => <button key={fps} aria-pressed={settings.fps === fps} className={settings.fps === fps ? "active" : ""} onClick={() => save({ fps })}>{copy(`pet.${fps}`)}</button>)}
      </div></fieldset>
    </div>
    <div className="pet-actions">
      <button className="secondary small" disabled={!enabled} onClick={() => save({ visible: !settings.visible })}>{settings.visible ? <EyeSlash size={16} /> : <Eye size={16} />}{settings.visible ? copy("pet.hide") : copy("pet.show")}</button>
      <button className="ghost small" onClick={() => save({ reset_position: true })}><ArrowsClockwise size={16} />{copy("pet.reset")}</button>
    </div>
    {statusCopy ? <p className="pet-notice">{statusCopy}</p> : null}
    {error ? <p className="setting-message" role="alert">{error}</p> : null}
  </section>;
}
