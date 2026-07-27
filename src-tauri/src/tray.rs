use crate::{
    error::{AppError, Result},
    imports::PrintState,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::sync::Mutex;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub fn is_supported_print_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".gcode.3mf") || lower.ends_with(".3mf") || lower.ends_with(".gcode")
}

pub struct Debouncer {
    window: Duration,
    seen: HashMap<PathBuf, Instant>,
}
impl Debouncer {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            seen: HashMap::new(),
        }
    }
    pub fn accept(&mut self, path: &Path, now: Instant) -> bool {
        let accepted = self
            .seen
            .get(path)
            .is_none_or(|last| now.duration_since(*last) >= self.window);
        if accepted {
            self.seen.insert(path.to_path_buf(), now);
        }
        accepted
    }
}

pub struct WatchState(pub Mutex<Option<RecommendedWatcher>>);
pub struct PendingNavigation(pub Mutex<Option<String>>);
pub struct NativeMenuState {
    open: tauri::menu::MenuItem<tauri::Wry>,
    quit: tauri::menu::MenuItem<tauri::Wry>,
}

#[derive(Clone, Serialize)]
struct WatchImportEvent {
    ok: bool,
    job_id: Option<String>,
    code: Option<String>,
}

#[tauri::command]
pub fn set_watch_folder(
    app: tauri::AppHandle,
    path: Option<String>,
    state: tauri::State<'_, WatchState>,
    print_state: tauri::State<'_, PrintState>,
) -> Result<Option<String>> {
    let mut active = state
        .0
        .lock()
        .map_err(|_| AppError::Database("watch lock poisoned".into()))?;
    let service = print_state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?;
    if let Some(ref candidate) = path {
        let folder = PathBuf::from(candidate);
        if !folder.is_absolute() || !folder.is_dir() {
            return Err(AppError::InvalidFile);
        }
        let app_handle = app.clone();
        let watched_root = folder.clone();
        let mut debouncer = Debouncer::new(Duration::from_secs(2));
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else {
                    return;
                };
                for file in event.paths {
                    if file.parent() != Some(watched_root.as_path())
                        || !is_supported_print_path(&file)
                        || !debouncer.accept(&file, Instant::now())
                    {
                        continue;
                    }
                    let result = app_handle
                        .state::<PrintState>()
                        .lock()
                        .ok()
                        .and_then(|mut service| service.import_print_file(&file).ok());
                    match result {
                        Some(preview) => {
                            let _ = app_handle.emit(
                                "watch-import",
                                WatchImportEvent {
                                    ok: true,
                                    job_id: Some(preview.job_id.to_string()),
                                    code: None,
                                },
                            );
                            let _ = app_handle
                                .notification()
                                .builder()
                                .title("Spool Keeper")
                                .body("Task awaiting settlement")
                                .show();
                        }
                        None => {
                            let _ = app_handle.emit(
                                "watch-import",
                                WatchImportEvent {
                                    ok: false,
                                    job_id: None,
                                    code: Some("import_failed".into()),
                                },
                            );
                        }
                    }
                }
            })
            .map_err(|error| AppError::Io(error.to_string()))?;
        watcher
            .watch(&folder, RecursiveMode::NonRecursive)
            .map_err(|error| AppError::Io(error.to_string()))?;
        service.database.connection.execute("INSERT INTO app_settings(setting_key,setting_value) VALUES('watch_folder',?1) ON CONFLICT(setting_key) DO UPDATE SET setting_value=excluded.setting_value,updated_at=CURRENT_TIMESTAMP", [candidate])?;
        service.database.connection.execute("INSERT INTO app_settings(setting_key,setting_value) VALUES('watch_enabled','true') ON CONFLICT(setting_key) DO UPDATE SET setting_value='true',updated_at=CURRENT_TIMESTAMP", [])?;
        *active = Some(watcher);
    } else {
        *active = None;
        service.database.connection.execute(
            "DELETE FROM app_settings WHERE setting_key IN ('watch_folder','watch_enabled')",
            [],
        )?;
    }
    Ok(path)
}

#[tauri::command]
pub fn get_watch_folder(state: tauri::State<'_, PrintState>) -> Result<Option<String>> {
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?;
    Ok(service.database.connection.query_row("SELECT setting_value FROM app_settings WHERE setting_key='watch_folder' AND EXISTS(SELECT 1 FROM app_settings WHERE setting_key='watch_enabled' AND setting_value='true')",[],|row|row.get(0)).optional()?)
}

#[tauri::command]
pub fn open_main(app: tauri::AppHandle) -> Result<()> {
    show_main(&app);
    Ok(())
}

#[tauri::command]
pub fn open_job_in_main(
    app: tauri::AppHandle,
    job_id: String,
    pending: tauri::State<'_, PendingNavigation>,
) -> Result<()> {
    *pending
        .0
        .lock()
        .map_err(|_| AppError::Database("navigation lock poisoned".into()))? = Some(job_id.clone());
    show_main(&app);
    app.emit_to("main", "open-job", job_id)
        .map_err(|error| AppError::Io(error.to_string()))?;
    if let Some(window) = app.get_webview_window("menubar") {
        let _ = window.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn take_pending_job(pending: tauri::State<'_, PendingNavigation>) -> Result<Option<String>> {
    Ok(pending
        .0
        .lock()
        .map_err(|_| AppError::Database("navigation lock poisoned".into()))?
        .take())
}

#[tauri::command]
pub fn set_native_locale(locale: String, state: tauri::State<'_, NativeMenuState>) -> Result<()> {
    let (open, quit) = match locale.as_str() {
        "en" => ("Open Spool Keeper", "Quit"),
        "zh-TW" => ("開啟耗材管家", "結束"),
        _ => ("打开耗材管家", "退出"),
    };
    state
        .open
        .set_text(open)
        .map_err(|e| AppError::Io(e.to_string()))?;
    state
        .quit
        .set_text(quit)
        .map_err(|e| AppError::Io(e.to_string()))?;
    Ok(())
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    use tauri::{
        menu::{Menu, MenuItemBuilder},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        PhysicalPosition, WindowEvent,
    };
    let open = MenuItemBuilder::with_id("open", "打开耗材管家").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "退出").build(app)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    app.manage(NativeMenuState { open, quit });
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/trayTemplate.png"))?;
    TrayIconBuilder::with_id("spool-keeper")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Spool Keeper")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                rect,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("menubar") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let scale = window.scale_factor().unwrap_or(1.0);
                        let size = window.outer_size().unwrap_or_default();
                        let anchor = rect.position.to_physical::<f64>(scale);
                        let x = anchor.x - (size.width as f64 / 2.0)
                            + (rect.size.to_physical::<f64>(scale).width / 2.0);
                        let y = anchor.y + rect.size.to_physical::<f64>(scale).height;
                        let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;
    if let Some(main) = app.get_webview_window("main") {
        let close = main.clone();
        main.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = close.hide();
            }
        });
    }
    if let Some(menu) = app.get_webview_window("menubar") {
        let blur = menu.clone();
        menu.on_window_event(move |event| {
            if matches!(event, WindowEvent::Focused(false)) {
                let _ = blur.hide();
            }
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_supported_print_path, Debouncer};
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    #[test]
    fn watcher_accepts_only_relevant_extensions() {
        assert!(is_supported_print_path(Path::new("plate.gcode.3mf")));
        assert!(is_supported_print_path(Path::new("plate.3mf")));
        assert!(is_supported_print_path(Path::new("plate.gcode")));
        assert!(!is_supported_print_path(Path::new("plate.stl")));
        assert!(!is_supported_print_path(Path::new("plate.3mf.tmp")));
    }

    #[test]
    fn watcher_debounces_duplicate_native_events() {
        let now = Instant::now();
        let mut debouncer = Debouncer::new(Duration::from_secs(2));
        assert!(debouncer.accept(Path::new("plate.gcode.3mf"), now));
        assert!(!debouncer.accept(
            Path::new("plate.gcode.3mf"),
            now + Duration::from_millis(100)
        ));
        assert!(debouncer.accept(Path::new("plate.gcode.3mf"), now + Duration::from_secs(3)));
    }
}
