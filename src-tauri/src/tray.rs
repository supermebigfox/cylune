use crate::{
    error::{AppError, Result},
    imports::PrintState,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::sync::Mutex;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

pub use crate::pet::input::is_supported_print_path;

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
        self.seen
            .retain(|_, last| now.duration_since(*last) < self.window);
        let accepted = self
            .seen
            .get(path)
            .is_none_or(|last| now.duration_since(*last) >= self.window);
        if accepted {
            self.seen.insert(path.to_path_buf(), now);
        }
        accepted
    }
    pub fn forget(&mut self, path: &Path) {
        self.seen.remove(path);
    }
}

pub fn import_with_retry(
    service: &mut crate::imports::PrintService,
    path: &Path,
    max_attempts: u8,
    retry_delay: Duration,
) -> Result<crate::imports::ImportPreview> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match service.import_print_file(path) {
            Err(AppError::FileNotStable) if attempt < max_attempts => {
                std::thread::sleep(retry_delay)
            }
            result => return result,
        }
    }
}

pub struct WatchState(pub Mutex<Option<RecommendedWatcher>>);
pub struct NativeMenuState {
    open: tauri::menu::MenuItem<tauri::Wry>,
    reset: tauri::menu::MenuItem<tauri::Wry>,
    visibility: tauri::menu::MenuItem<tauri::Wry>,
    quit: tauri::menu::MenuItem<tauri::Wry>,
    tray: tauri::tray::TrayIcon<tauri::Wry>,
    locale: Mutex<String>,
    pet_enabled: Mutex<bool>,
    pet_visible: Mutex<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCopy {
    pub open: &'static str,
    pub reset: &'static str,
    pub show: &'static str,
    pub hide: &'static str,
    pub quit: &'static str,
    pub tooltip: &'static str,
    pub notification_title: &'static str,
    pub notification_body: &'static str,
}

pub fn native_copy(locale: &str) -> NativeCopy {
    match locale {
        "en" => NativeCopy {
            open: "Open Spool Keeper",
            reset: "Reset black hole position",
            show: "Show black hole",
            hide: "Hide black hole",
            quit: "Quit",
            tooltip: "Spool Keeper",
            notification_title: "Spool Keeper",
            notification_body: "A print is awaiting settlement",
        },
        "zh-TW" => NativeCopy {
            open: "開啟耗材管家",
            reset: "重設黑洞位置",
            show: "顯示黑洞",
            hide: "隱藏黑洞",
            quit: "結束",
            tooltip: "耗材管家",
            notification_title: "耗材管家",
            notification_body: "有一個列印任務等待結算",
        },
        _ => NativeCopy {
            open: "打开耗材管家",
            reset: "重置黑洞位置",
            show: "显示黑洞",
            hide: "隐藏黑洞",
            quit: "退出",
            tooltip: "耗材管家",
            notification_title: "耗材管家",
            notification_body: "有一个打印任务等待结算",
        },
    }
}

pub fn import_error_copy(locale: &str, code: &str) -> &'static str {
    match (locale, code) {
        ("en", "unsliced_project") => "This project has not been sliced",
        ("en", "standalone_gcode_profiles_required") => "This G-code has no filament profile",
        ("en", "file_not_stable") => "The file is still being written",
        ("en", "invalid_file") => "The file could not be recognized",
        ("en", "duplicate_job") => "This print job already exists",
        ("en", "invalid_mapping") => "The filament mapping is invalid",
        ("en", "unknown_gcode") => "No measurable extrusion data was found",
        ("en", "database") => "The local database is unavailable",
        ("en", _) => "The file is temporarily unavailable",
        ("zh-TW", "unsliced_project") => "這個專案尚未切片",
        ("zh-TW", "standalone_gcode_profiles_required") => "這個 G-code 沒有耗材設定",
        ("zh-TW", "file_not_stable") => "檔案仍在寫入",
        ("zh-TW", "invalid_file") => "無法辨識這個檔案",
        ("zh-TW", "duplicate_job") => "這個列印任務已經存在",
        ("zh-TW", "invalid_mapping") => "耗材對應無效",
        ("zh-TW", "unknown_gcode") => "沒有找到可計算的擠出資料",
        ("zh-TW", "database") => "本機資料暫時無法讀取",
        ("zh-TW", _) => "檔案暫時無法存取",
        (_, "unsliced_project") => "这个项目尚未切片",
        (_, "standalone_gcode_profiles_required") => "这个 G-code 没有耗材配置",
        (_, "file_not_stable") => "文件仍在写入",
        (_, "invalid_file") => "无法识别这个文件",
        (_, "duplicate_job") => "这个打印任务已经存在",
        (_, "invalid_mapping") => "耗材映射无效",
        (_, "unknown_gcode") => "没有找到可计算的挤出数据",
        (_, "database") => "本地数据暂时无法读取",
        (_, _) => "文件暂时无法访问",
    }
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
        let requested = PathBuf::from(candidate);
        if !requested.is_absolute() || !requested.is_dir() {
            return Err(AppError::InvalidFile);
        }
        let folder = requested
            .canonicalize()
            .map_err(|_| AppError::InvalidFile)?;
        let canonical = folder.to_string_lossy().into_owned();
        let app_handle = app.clone();
        let watched_root = folder.clone();
        let mut debouncer = Debouncer::new(Duration::from_secs(2));
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else {
                    return;
                };
                for file in event.paths {
                    let Ok(file) = file.canonicalize() else {
                        continue;
                    };
                    if file.parent() != Some(watched_root.as_path())
                        || !file.is_file()
                        || !is_supported_print_path(&file)
                        || !debouncer.accept(&file, Instant::now())
                    {
                        continue;
                    }
                    let result = match app_handle.state::<PrintState>().lock() {
                        Ok(mut service) => {
                            import_with_retry(&mut service, &file, 3, Duration::from_millis(250))
                                .and_then(|preview| {
                                    let summary = service.pending_summary()?;
                                    Ok((preview, summary))
                                })
                        }
                        Err(_) => Err(AppError::Database("print lock poisoned".into())),
                    };
                    match result {
                        Ok((preview, summary)) => {
                            app_handle
                                .state::<crate::pet::runtime::PetRuntime>()
                                .refresh_pending(
                                    summary,
                                    Some(crate::pet::runtime::PetSignal::ImportSucceeded {
                                        job_id: preview.job_id,
                                        pending_count: summary.count,
                                    }),
                                );
                            let _ = app_handle.emit(
                                "watch-import",
                                WatchImportEvent {
                                    ok: true,
                                    job_id: Some(preview.job_id.to_string()),
                                    code: None,
                                },
                            );
                            let locale = app_handle
                                .state::<NativeMenuState>()
                                .locale
                                .lock()
                                .map(|value| value.clone())
                                .unwrap_or_else(|_| "zh-CN".to_owned());
                            let copy = native_copy(&locale);
                            let _ = app_handle
                                .notification()
                                .builder()
                                .title(copy.notification_title)
                                .body(copy.notification_body)
                                .show();
                        }
                        Err(error) => {
                            debouncer.forget(&file);
                            let _ = app_handle.emit(
                                "watch-import",
                                WatchImportEvent {
                                    ok: false,
                                    job_id: None,
                                    code: Some(error.code().into()),
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
        service.database.connection.execute("INSERT INTO app_settings(setting_key,setting_value) VALUES('watch_folder',?1) ON CONFLICT(setting_key) DO UPDATE SET setting_value=excluded.setting_value,updated_at=CURRENT_TIMESTAMP", [&canonical])?;
        service.database.connection.execute("INSERT INTO app_settings(setting_key,setting_value) VALUES('watch_enabled','true') ON CONFLICT(setting_key) DO UPDATE SET setting_value='true',updated_at=CURRENT_TIMESTAMP", [])?;
        *active = Some(watcher);
    } else {
        *active = None;
        service.database.connection.execute(
            "DELETE FROM app_settings WHERE setting_key IN ('watch_folder','watch_enabled')",
            [],
        )?;
    }
    Ok(path.map(|_| {
        service
            .database
            .connection
            .query_row(
                "SELECT setting_value FROM app_settings WHERE setting_key='watch_folder'",
                [],
                |r| r.get(0),
            )
            .unwrap_or_default()
    }))
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

pub fn persist_pending_job(database: &crate::db::AppDatabase, job_id: &str) -> Result<()> {
    Uuid::parse_str(job_id).map_err(|_| AppError::InvalidJob)?;
    let exists: bool = database.connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM print_jobs WHERE job_id=?1)",
        params![job_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(AppError::InvalidJob);
    }
    database.connection.execute(
        "INSERT INTO app_settings(setting_key,setting_value) VALUES('pending_job_id',?1)
         ON CONFLICT(setting_key) DO UPDATE SET setting_value=excluded.setting_value,updated_at=CURRENT_TIMESTAMP",
        params![job_id],
    )?;
    Ok(())
}

pub fn take_pending_job_from_database(
    database: &mut crate::db::AppDatabase,
) -> Result<Option<String>> {
    let transaction = database.connection.transaction()?;
    let job_id: Option<String> = transaction
        .query_row(
            "SELECT setting_value FROM app_settings WHERE setting_key='pending_job_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let valid = job_id.filter(|id| {
        Uuid::parse_str(id).is_ok()
            && transaction
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM print_jobs WHERE job_id=?1 AND outcome IS NULL)",
                    params![id],
                    |row| row.get::<_, bool>(0),
                )
                .unwrap_or(false)
    });
    transaction.execute(
        "DELETE FROM app_settings WHERE setting_key='pending_job_id'",
        [],
    )?;
    transaction.commit()?;
    Ok(valid)
}

#[tauri::command]
pub fn open_job_in_main(
    app: tauri::AppHandle,
    job_id: String,
    print_state: tauri::State<'_, PrintState>,
) -> Result<()> {
    let service = print_state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?;
    persist_pending_job(&service.database, &job_id)?;
    show_main(&app);
    app.emit_to("main", "open-job", job_id.clone())
        .map_err(|error| AppError::Io(error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn take_pending_job(print_state: tauri::State<'_, PrintState>) -> Result<Option<String>> {
    let mut service = print_state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?;
    take_pending_job_from_database(&mut service.database)
}

#[tauri::command]
pub fn set_native_locale(
    locale: String,
    state: tauri::State<'_, NativeMenuState>,
    print_state: tauri::State<'_, PrintState>,
) -> Result<()> {
    let locale = if matches!(locale.as_str(), "zh-CN" | "zh-TW" | "en") {
        locale
    } else {
        return Err(AppError::InvalidFile);
    };
    let copy = native_copy(&locale);
    state
        .open
        .set_text(copy.open)
        .map_err(|e| AppError::Io(e.to_string()))?;
    state
        .reset
        .set_text(copy.reset)
        .map_err(|e| AppError::Io(e.to_string()))?;
    let visible = *state
        .pet_visible
        .lock()
        .map_err(|_| AppError::Database("pet visibility lock poisoned".into()))?;
    state
        .visibility
        .set_text(if visible { copy.hide } else { copy.show })
        .map_err(|e| AppError::Io(e.to_string()))?;
    state
        .quit
        .set_text(copy.quit)
        .map_err(|e| AppError::Io(e.to_string()))?;
    state
        .tray
        .set_tooltip(Some(copy.tooltip))
        .map_err(|e| AppError::Io(e.to_string()))?;
    *state
        .locale
        .lock()
        .map_err(|_| AppError::Database("locale lock poisoned".into()))? = locale.clone();
    let service = print_state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?;
    service.database.connection.execute(
        "INSERT INTO app_settings(setting_key,setting_value) VALUES('locale',?1)
         ON CONFLICT(setting_key) DO UPDATE SET setting_value=excluded.setting_value,updated_at=CURRENT_TIMESTAMP",
        params![locale],
    )?;
    Ok(())
}

pub fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn sync_pet_state(app: &tauri::AppHandle, enabled: bool, visible: bool) {
    let state = app.state::<NativeMenuState>();
    if let Ok(mut saved) = state.pet_enabled.lock() {
        *saved = enabled;
    }
    if let Ok(mut saved) = state.pet_visible.lock() {
        *saved = visible;
    }
    let locale = state
        .locale
        .lock()
        .map(|locale| locale.clone())
        .unwrap_or_else(|_| "zh-CN".to_owned());
    let copy = native_copy(&locale);
    let _ = state.visibility.set_enabled(enabled);
    let _ = state
        .visibility
        .set_text(if visible { copy.hide } else { copy.show });
}

pub fn notify_import_error(app: &tauri::AppHandle, code: &str) {
    let locale = app
        .state::<NativeMenuState>()
        .locale
        .lock()
        .map(|locale| locale.clone())
        .unwrap_or_else(|_| "zh-CN".to_owned());
    let copy = native_copy(&locale);
    let _ = app
        .notification()
        .builder()
        .title(copy.notification_title)
        .body(import_error_copy(&locale, code))
        .show();
}

pub fn setup(
    app: &tauri::App,
    initial_locale: &str,
    pet_enabled: bool,
    pet_visible: bool,
) -> tauri::Result<()> {
    use tauri::{
        menu::{Menu, MenuItemBuilder},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        WindowEvent,
    };
    let copy = native_copy(initial_locale);
    let open = MenuItemBuilder::with_id("open", copy.open).build(app)?;
    let reset = MenuItemBuilder::with_id("reset", copy.reset).build(app)?;
    let visibility = MenuItemBuilder::with_id(
        "pet-visibility",
        if pet_visible { copy.hide } else { copy.show },
    )
    .enabled(pet_enabled)
    .build(app)?;
    let quit = MenuItemBuilder::with_id("quit", copy.quit).build(app)?;
    let menu = Menu::with_items(app, &[&open, &reset, &visibility, &quit])?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/trayTemplate.png"))?;
    let tray = TrayIconBuilder::with_id("spool-keeper")
        .icon(icon)
        .icon_as_template(true)
        .tooltip(copy.tooltip)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main(app),
            "reset" => app.state::<crate::pet::runtime::PetRuntime>().reset(),
            "pet-visibility" => app.state::<crate::pet::runtime::PetRuntime>().toggle(),
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
                let _ = rect;
                app.state::<crate::pet::runtime::PetRuntime>().toggle();
            }
        })
        .build(app)?;
    app.manage(NativeMenuState {
        open,
        reset,
        visibility,
        quit,
        tray,
        locale: Mutex::new(initial_locale.to_owned()),
        pet_enabled: Mutex::new(pet_enabled),
        pet_visible: Mutex::new(pet_visible),
    });
    if let Some(main) = app.get_webview_window("main") {
        let close = main.clone();
        main.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = close.hide();
            }
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        import_error_copy, import_with_retry, is_supported_print_path, native_copy,
        persist_pending_job, take_pending_job_from_database, Debouncer,
    };
    use crate::{db::AppDatabase, imports::PrintService};
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        path::Path,
        thread,
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
        debouncer.forget(Path::new("plate.gcode.3mf"));
        assert!(debouncer.accept(
            Path::new("plate.gcode.3mf"),
            now + Duration::from_secs(3) + Duration::from_millis(1)
        ));
    }

    #[test]
    fn file_changed_during_stability_window_is_retried_once_then_imported() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bambu_multicolor.3mf");
        let bytes = fs::read(source).unwrap();
        let path =
            std::env::temp_dir().join(format!("watch-write-{}.gcode.3mf", uuid::Uuid::new_v4()));
        fs::write(&path, &bytes[..32]).unwrap();
        let target = path.clone();
        let full = bytes.clone();
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(15));
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(target)
                .unwrap();
            file.write_all(&full).unwrap();
            file.sync_all().unwrap();
        });
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::from_millis(40));
        let imported =
            import_with_retry(&mut service, &path, 3, Duration::from_millis(15)).unwrap();
        writer.join().unwrap();
        assert_eq!(service.job_count(&imported.source_hash).unwrap(), 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_menu_and_notifications_follow_the_selected_locale() {
        assert_eq!(
            native_copy("en").notification_body,
            "A print is awaiting settlement"
        );
        assert_eq!(native_copy("zh-TW").open, "開啟耗材管家");
        assert_eq!(native_copy("zh-TW").reset, "重設黑洞位置");
        assert_eq!(native_copy("zh-TW").show, "顯示黑洞");
        assert_eq!(native_copy("zh-TW").hide, "隱藏黑洞");
        assert_eq!(native_copy("zh-CN").quit, "退出");
        assert_eq!(
            import_error_copy("en", "unsliced_project"),
            "This project has not been sliced"
        );
        assert_eq!(
            import_error_copy("zh-CN", "invalid_file"),
            "无法识别这个文件"
        );
    }

    #[test]
    fn pending_navigation_survives_database_reopen_and_is_consumed_once() {
        let database_path =
            std::env::temp_dir().join(format!("pending-job-{}.sqlite", uuid::Uuid::new_v4()));
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bambu_multicolor.3mf");
        let job_id = {
            let database = AppDatabase::open(&database_path).unwrap();
            let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
            let preview = service.import_print_file(&fixture).unwrap();
            let job_id = preview.job_id.to_string();
            persist_pending_job(&service.database, &job_id).unwrap();
            job_id
        };

        let mut reopened = AppDatabase::open(&database_path).unwrap();
        assert_eq!(
            take_pending_job_from_database(&mut reopened).unwrap(),
            Some(job_id)
        );
        assert_eq!(take_pending_job_from_database(&mut reopened).unwrap(), None);

        drop(reopened);
        fs::remove_file(database_path).unwrap();
    }

    #[test]
    fn pending_navigation_does_not_reopen_a_settled_job() {
        let database = AppDatabase::open_in_memory().unwrap();
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bambu_multicolor.3mf");
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let job_id = service
            .import_print_file(&fixture)
            .unwrap()
            .job_id
            .to_string();
        persist_pending_job(&service.database, &job_id).unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs SET outcome='{\"kind\":\"success\"}' WHERE job_id=?1",
                [&job_id],
            )
            .unwrap();

        assert_eq!(
            take_pending_job_from_database(&mut service.database).unwrap(),
            None
        );
    }
}
