use crate::{
    error::{AppError, Result},
    imports::PrintState,
};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayPlatform {
    MacOs,
    Windows,
}

pub fn tray_icon_bytes(platform: TrayPlatform) -> &'static [u8] {
    match platform {
        TrayPlatform::MacOs => include_bytes!("../icons/trayTemplate.png"),
        TrayPlatform::Windows => include_bytes!("../icons/icon.png"),
    }
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
) -> Result<crate::history::ImportProjectPreview> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match service.import_print_project(path) {
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
            open: "Open CYLUNE",
            reset: "Reset black hole position",
            show: "Show black hole",
            hide: "Hide black hole",
            quit: "Quit",
            tooltip: "CYLUNE",
            notification_title: "CYLUNE",
            notification_body: "A print is awaiting settlement",
        },
        "zh-TW" => NativeCopy {
            open: "開啟 CYLUNE",
            reset: "重設黑洞位置",
            show: "顯示黑洞",
            hide: "隱藏黑洞",
            quit: "結束",
            tooltip: "CYLUNE",
            notification_title: "CYLUNE",
            notification_body: "有一個列印任務等待結算",
        },
        _ => NativeCopy {
            open: "打开 CYLUNE",
            reset: "重置黑洞位置",
            show: "显示黑洞",
            hide: "隐藏黑洞",
            quit: "退出",
            tooltip: "CYLUNE",
            notification_title: "CYLUNE",
            notification_body: "有一个打印任务等待结算",
        },
    }
}

pub fn import_notification_body(locale: &str, plate_count: usize) -> String {
    match locale {
        "en" => format!(
            "A project with {plate_count} {} is awaiting settlement",
            if plate_count == 1 { "plate" } else { "plates" }
        ),
        "zh-TW" => format!("有一個包含 {plate_count} 個盤面的列印專案等待結算"),
        _ => format!("有一个包含 {plate_count} 个盘面的打印项目等待结算"),
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
    project_id: Option<String>,
    plate_id: Option<String>,
    job_id: Option<String>,
    plate_count: Option<u32>,
    state: Option<crate::imports::ImportState>,
    source_hash: Option<String>,
    source_path: Option<String>,
    code: Option<String>,
}

fn watch_import_event(
    preview: &crate::history::ImportProjectPreview,
    source_path: &Path,
) -> WatchImportEvent {
    let first_pending = preview.plates.iter().find(|plate| {
        matches!(
            plate.status,
            crate::domain::PlateStatus::PendingMapping | crate::domain::PlateStatus::Ready
        )
    });
    WatchImportEvent {
        ok: true,
        project_id: Some(preview.project_id.to_string()),
        plate_id: first_pending.map(|plate| plate.plate_id.to_string()),
        job_id: first_pending.map(|plate| plate.job_id.to_string()),
        plate_count: Some(preview.plates.len() as u32),
        state: Some(preview.state),
        source_hash: Some(preview.source_hash.clone()),
        source_path: Some(source_path.to_string_lossy().into_owned()),
        code: None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingNavigation {
    pub project_id: Uuid,
    pub plate_id: Uuid,
    pub job_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingNavigationTarget {
    pub project_id: Option<Uuid>,
    pub plate_id: Option<Uuid>,
    pub job_id: Uuid,
}

impl From<PendingNavigation> for PendingNavigationTarget {
    fn from(navigation: PendingNavigation) -> Self {
        Self {
            project_id: Some(navigation.project_id),
            plate_id: Some(navigation.plate_id),
            job_id: navigation.job_id,
        }
    }
}

pub fn pending_navigation_for_job(
    database: &crate::db::AppDatabase,
    job_id: Uuid,
) -> Result<Option<PendingNavigation>> {
    pending_navigation_for_job_in(&database.connection, job_id)
}

fn pending_navigation_for_job_in(
    connection: &rusqlite::Connection,
    job_id: Uuid,
) -> Result<Option<PendingNavigation>> {
    connection
        .query_row(
            "SELECT pending_plates.project_id, pending_plates.plate_id, pending_jobs.job_id
             FROM print_jobs AS requested_jobs
             JOIN print_plates AS requested_plates
               ON requested_plates.plate_id = requested_jobs.plate_id
             JOIN print_plates AS pending_plates
               ON pending_plates.project_id = requested_plates.project_id
             JOIN print_jobs AS pending_jobs
               ON pending_jobs.plate_id = pending_plates.plate_id
             WHERE requested_jobs.job_id = ?1
               AND pending_jobs.outcome IS NULL
             ORDER BY pending_plates.plate_index
             LIMIT 1",
            params![job_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .map(|(project_id, plate_id, job_id)| {
            Ok(PendingNavigation {
                project_id: project_id.parse().map_err(|_| AppError::InvalidJob)?,
                plate_id: plate_id.parse().map_err(|_| AppError::InvalidJob)?,
                job_id: job_id.parse().map_err(|_| AppError::InvalidJob)?,
            })
        })
        .transpose()
}

fn pending_navigation_for_project_in(
    connection: &rusqlite::Connection,
    project_id: Uuid,
) -> Result<Option<PendingNavigation>> {
    connection
        .query_row(
            "SELECT plates.project_id, plates.plate_id, jobs.job_id
             FROM print_plates AS plates
             JOIN print_jobs AS jobs ON jobs.plate_id = plates.plate_id
             WHERE plates.project_id = ?1
               AND jobs.outcome IS NULL
             ORDER BY plates.plate_index
             LIMIT 1",
            params![project_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .map(|(project_id, plate_id, job_id)| {
            Ok(PendingNavigation {
                project_id: project_id.parse().map_err(|_| AppError::InvalidJob)?,
                plate_id: plate_id.parse().map_err(|_| AppError::InvalidJob)?,
                job_id: job_id.parse().map_err(|_| AppError::InvalidJob)?,
            })
        })
        .transpose()
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
                                    if let Some(job_id) = preview
                                        .plates
                                        .iter()
                                        .find(|plate| {
                                            matches!(
                                                plate.status,
                                                crate::domain::PlateStatus::PendingMapping
                                                    | crate::domain::PlateStatus::Ready
                                            )
                                        })
                                        .map(|plate| plate.job_id)
                                    {
                                        persist_pending_job(
                                            &service.database,
                                            &job_id.to_string(),
                                        )?;
                                    }
                                    let summary = service.pending_summary()?;
                                    Ok((preview, summary))
                                })
                        }
                        Err(_) => Err(AppError::Database("print lock poisoned".into())),
                    };
                    match result {
                        Ok((preview, summary)) => {
                            let first_pending = preview.plates.iter().find(|plate| {
                                matches!(
                                    plate.status,
                                    crate::domain::PlateStatus::PendingMapping
                                        | crate::domain::PlateStatus::Ready
                                )
                            });
                            app_handle
                                .state::<crate::pet::runtime::PetRuntime>()
                                .refresh_pending(
                                    summary,
                                    first_pending.map(|plate| {
                                        crate::pet::runtime::PetSignal::ProjectImportSucceeded {
                                            navigation: PendingNavigation {
                                                project_id: preview.project_id,
                                                plate_id: plate.plate_id,
                                                job_id: plate.job_id,
                                            },
                                            plate_count: preview.plates.len() as u32,
                                            pending_count: summary.count,
                                        }
                                    }),
                                );
                            let _ = app_handle
                                .emit("watch-import", watch_import_event(&preview, &file));
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
                                .body(import_notification_body(&locale, preview.plates.len()))
                                .show();
                        }
                        Err(error) => {
                            debouncer.forget(&file);
                            let _ = app_handle.emit(
                                "watch-import",
                                WatchImportEvent {
                                    ok: false,
                                    project_id: None,
                                    plate_id: None,
                                    job_id: None,
                                    plate_count: None,
                                    state: None,
                                    source_hash: None,
                                    source_path: None,
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
    let job_id = Uuid::parse_str(job_id).map_err(|_| AppError::InvalidJob)?;
    let pending_navigation = pending_navigation_for_job(database, job_id)?;
    let value = if let Some(navigation) = pending_navigation {
        serde_json::to_string(&navigation).map_err(|error| AppError::Database(error.to_string()))?
    } else {
        let pending_legacy_job: bool = database.connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM print_jobs
                WHERE job_id = ?1 AND plate_id IS NULL AND outcome IS NULL
             )",
            params![job_id.to_string()],
            |row| row.get(0),
        )?;
        if !pending_legacy_job {
            return Err(AppError::InvalidJob);
        }
        job_id.to_string()
    };
    database.connection.execute(
        "INSERT INTO app_settings(setting_key,setting_value) VALUES('pending_job_id',?1)
         ON CONFLICT(setting_key) DO UPDATE SET setting_value=excluded.setting_value,updated_at=CURRENT_TIMESTAMP",
        params![value],
    )?;
    Ok(())
}

pub fn take_pending_job_from_database(
    database: &mut crate::db::AppDatabase,
) -> Result<Option<String>> {
    Ok(take_pending_navigation_from_database(database)?
        .map(|navigation| navigation.job_id.to_string()))
}

pub fn take_pending_navigation_from_database(
    database: &mut crate::db::AppDatabase,
) -> Result<Option<PendingNavigationTarget>> {
    let transaction = database.connection.transaction()?;
    let saved: Option<String> = transaction
        .query_row(
            "SELECT setting_value FROM app_settings WHERE setting_key='pending_job_id'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let target = if let Some(saved) = saved {
        if let Ok(navigation) = serde_json::from_str::<PendingNavigation>(&saved) {
            pending_navigation_for_project_in(&transaction, navigation.project_id)?.map(Into::into)
        } else if let Ok(job_id) = Uuid::parse_str(&saved) {
            if let Some(navigation) = pending_navigation_for_job_in(&transaction, job_id)? {
                Some(navigation.into())
            } else {
                let pending_legacy_job: bool = transaction.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM print_jobs
                        WHERE job_id = ?1 AND plate_id IS NULL AND outcome IS NULL
                     )",
                    params![job_id.to_string()],
                    |row| row.get(0),
                )?;
                pending_legacy_job.then_some(PendingNavigationTarget {
                    project_id: None,
                    plate_id: None,
                    job_id,
                })
            }
        } else {
            None
        }
    } else {
        None
    };
    transaction.execute(
        "DELETE FROM app_settings WHERE setting_key='pending_job_id'",
        [],
    )?;
    transaction.commit()?;
    Ok(target)
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
    let requested_job_id = Uuid::parse_str(&job_id).map_err(|_| AppError::InvalidJob)?;
    let navigation = pending_navigation_for_job(&service.database, requested_job_id)?;
    show_main(&app);
    if let Some(navigation) = navigation {
        app.emit_to("main", "open-project", navigation.clone())
            .map_err(|error| AppError::Io(error.to_string()))?;
    } else {
        app.emit_to("main", "open-job", job_id)
            .map_err(|error| AppError::Io(error.to_string()))?;
    }
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
pub fn take_pending_navigation(
    print_state: tauri::State<'_, PrintState>,
) -> Result<Option<PendingNavigationTarget>> {
    let mut service = print_state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".into()))?;
    take_pending_navigation_from_database(&mut service.database)
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
    let platform = if cfg!(target_os = "windows") {
        TrayPlatform::Windows
    } else {
        TrayPlatform::MacOs
    };
    let icon = tauri::image::Image::from_bytes(tray_icon_bytes(platform))?;
    let tray = TrayIconBuilder::with_id("cylune")
        .icon(icon)
        .icon_as_template(matches!(platform, TrayPlatform::MacOs))
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
        import_error_copy, import_notification_body, import_with_retry, is_supported_print_path,
        native_copy, persist_pending_job, take_pending_job_from_database,
        take_pending_navigation_from_database, watch_import_event, Debouncer,
    };
    use crate::{
        db::AppDatabase,
        domain::JobOutcome,
        imports::{ImportState, PrintService, ToolMapping},
        inventory::{InventoryService, NewSpool},
    };
    use std::{
        fs::{self, File, OpenOptions},
        io::Write,
        path::Path,
        thread,
        time::{Duration, Instant},
    };

    fn two_plate_fixture() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cylune-tray-two-plate-{}.3mf",
            uuid::Uuid::new_v4()
        ));
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::FileOptions::default();
        archive
            .start_file("Metadata/project_settings.config", options)
            .unwrap();
        archive
            .write_all(
                br##"{"filament_settings_id":["Bambu PLA Basic @BBL A1"],"filament_type":["PLA"],"filament_colour":["#FF0000"],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##,
            )
            .unwrap();
        archive
            .start_file("Metadata/plate_1.gcode", options)
            .unwrap();
        archive.write_all(b"M83\n; LAYER:0\nT0\nG1 E10\n").unwrap();
        archive
            .start_file("Metadata/plate_2.gcode", options)
            .unwrap();
        archive.write_all(b"M83\n; LAYER:0\nT0\nG1 E20\n").unwrap();
        archive.finish().unwrap();
        path
    }

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
        assert_eq!(imported.plates.len(), 1);
        assert_eq!(service.job_count(&imported.source_hash).unwrap(), 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_menu_and_notifications_follow_the_selected_locale() {
        for locale in ["en", "zh-TW", "zh-CN"] {
            let copy = native_copy(locale);
            assert_eq!(copy.tooltip, "CYLUNE");
            assert_eq!(copy.notification_title, "CYLUNE");
            assert!(copy.open.contains("CYLUNE"));
        }
        assert_eq!(
            native_copy("en").notification_body,
            "A print is awaiting settlement"
        );
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
    fn watcher_notification_reports_the_imported_plate_count() {
        assert_eq!(
            import_notification_body("en", 2),
            "A project with 2 plates is awaiting settlement"
        );
        assert!(import_notification_body("zh-TW", 3).contains('3'));
        assert!(import_notification_body("zh-CN", 4).contains('4'));
    }

    #[test]
    fn settled_watcher_drop_exposes_new_project_confirmation() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let fixture = two_plate_fixture();
        let imported = service.import_print_project(&fixture).unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs
                 SET outcome='{\"kind\":\"success\"}', settlement_version=1
                 WHERE plate_id IN (
                    SELECT plate_id FROM print_plates WHERE project_id=?1
                 )",
                [imported.project_id.to_string()],
            )
            .unwrap();
        let duplicate = service.import_print_project(&fixture).unwrap();

        let event = watch_import_event(&duplicate, &fixture);

        fs::remove_file(fixture).unwrap();
        assert!(event.ok);
        assert_eq!(event.project_id, Some(imported.project_id.to_string()));
        assert_eq!(event.plate_count, Some(2));
        assert_eq!(event.state, Some(ImportState::NewPrintConfirmationRequired));
        assert_eq!(event.job_id, None);
        assert_eq!(event.plate_id, None);
        assert_eq!(event.source_hash, Some(imported.source_hash));
        assert!(event.source_path.is_some());
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

    #[test]
    fn project_navigation_selects_its_first_pending_plate() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let fixture = two_plate_fixture();
        let project = service.import_print_project(&fixture).unwrap();
        fs::remove_file(fixture).unwrap();
        let first_job = project.plates[0].job_id.to_string();
        let second_job = project.plates[1].job_id.to_string();

        persist_pending_job(&service.database, &second_job).unwrap();

        assert_eq!(
            take_pending_job_from_database(&mut service.database).unwrap(),
            Some(first_job)
        );
    }

    #[test]
    fn pending_navigation_persists_project_and_first_pending_plate() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let fixture = two_plate_fixture();
        let project = service.import_print_project(&fixture).unwrap();
        fs::remove_file(fixture).unwrap();

        persist_pending_job(&service.database, &project.plates[1].job_id.to_string()).unwrap();

        let saved: String = service
            .database
            .connection
            .query_row(
                "SELECT setting_value FROM app_settings WHERE setting_key='pending_job_id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let navigation: serde_json::Value = serde_json::from_str(&saved).unwrap();
        assert_eq!(
            navigation["project_id"],
            project.project_id.to_string().as_str()
        );
        assert_eq!(
            navigation["plate_id"],
            project.plates[0].plate_id.to_string().as_str()
        );
        assert_eq!(
            navigation["job_id"],
            project.plates[0].job_id.to_string().as_str()
        );
    }

    #[test]
    fn persisted_project_navigation_re_resolves_the_first_pending_plate_after_restart() {
        let database_path = std::env::temp_dir().join(format!(
            "pending-project-navigation-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let fixture = two_plate_fixture();
        let (project_id, second_plate_id, second_job_id) = {
            let database = AppDatabase::open(&database_path).unwrap();
            let mut inventory = InventoryService::new(database);
            let spool_id = inventory
                .create_spool(NewSpool {
                    display_name: "Restart navigation".to_owned(),
                    preset_id: Some("Bambu PLA Basic @BBL A1".to_owned()),
                    catalog_id: None,
                    color_name: None,
                    color_code: None,
                    color_hexes: vec!["#FF0000".to_owned()],
                    preset_base: None,
                    brand: "Bambu Lab".to_owned(),
                    material: "PLA".to_owned(),
                    series: "Basic".to_owned(),
                    color_hex: "#FF0000".to_owned(),
                    remaining_grams: 1000.0,
                })
                .unwrap();
            inventory.mount_spool(1, spool_id).unwrap();
            let mut service =
                PrintService::with_stability_delay(inventory.into_database(), Duration::ZERO);
            let project = service.import_print_project(&fixture).unwrap();
            let first_job_id = project.plates[0].job_id;
            persist_pending_job(&service.database, &first_job_id.to_string()).unwrap();
            service
                .confirm_job_mapping(first_job_id, vec![ToolMapping { tool: 0, spool_id }])
                .unwrap();
            service
                .settle_job(first_job_id, JobOutcome::Success)
                .unwrap();
            (
                project.project_id,
                project.plates[1].plate_id,
                project.plates[1].job_id,
            )
        };

        let mut reopened = AppDatabase::open(&database_path).unwrap();
        let navigation = take_pending_navigation_from_database(&mut reopened)
            .unwrap()
            .unwrap();

        assert_eq!(navigation.project_id, Some(project_id));
        assert_eq!(navigation.plate_id, Some(second_plate_id));
        assert_eq!(navigation.job_id, second_job_id);
        assert_eq!(
            take_pending_navigation_from_database(&mut reopened).unwrap(),
            None
        );

        drop(reopened);
        fs::remove_file(fixture).unwrap();
        fs::remove_file(database_path).unwrap();
    }

    #[test]
    fn raw_uuid_pending_job_remains_a_legacy_navigation_fallback() {
        let database = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(database, Duration::ZERO);
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bambu_multicolor.3mf");
        let job_id = service.import_print_file(&fixture).unwrap().job_id;
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs SET plate_id = NULL WHERE job_id = ?1",
                [job_id.to_string()],
            )
            .unwrap();
        service
            .database
            .connection
            .execute(
                "INSERT INTO app_settings(setting_key, setting_value)
                 VALUES('pending_job_id', ?1)",
                [job_id.to_string()],
            )
            .unwrap();

        let target = take_pending_navigation_from_database(&mut service.database)
            .unwrap()
            .unwrap();

        assert_eq!(target.project_id, None);
        assert_eq!(target.plate_id, None);
        assert_eq!(target.job_id, job_id);
        assert_eq!(
            take_pending_navigation_from_database(&mut service.database).unwrap(),
            None
        );
    }

    #[test]
    fn each_desktop_platform_keeps_its_intended_tray_art() {
        assert_eq!(
            super::tray_icon_bytes(super::TrayPlatform::MacOs),
            include_bytes!("../icons/trayTemplate.png")
        );
        assert_eq!(
            super::tray_icon_bytes(super::TrayPlatform::Windows),
            include_bytes!("../icons/icon.png")
        );
    }
}
