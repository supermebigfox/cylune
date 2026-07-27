use crate::{
    error::{AppError, Result},
    imports::{ImportState, PendingSummary, PrintService, PrintState},
    pet::native::{
        NativeCaptureState, NativePet, NativePetError, PetCallbackKind, PetNativeConfig,
    },
    pet::{CapturePermission, PetMode, PetSettings, PetSettingsPatch, PetStatus, PetStore},
    tray::persist_pending_job,
};
use std::{
    ffi::{c_char, CStr},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    thread::{self, JoinHandle},
};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

const SIGNAL_IMPORT_SUCCEEDED: u32 = 1;
const SIGNAL_IMPORT_FAILED: u32 = 2;
const SIGNAL_SETTLEMENT_COMPLETED: u32 = 3;

static CALLBACK_SENDER: OnceLock<Mutex<Option<mpsc::Sender<NativeEvent>>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    Unavailable,
    NotDetermined,
    Requested,
    Denied,
    RestartRequired,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureEvent {
    Unavailable,
    NotDetermined,
    Denied,
    RestartRequired,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureSnapshot {
    pub effective_mode: PetMode,
    pub permission: CapturePermission,
    pub fallback_reason: Option<String>,
    pub pet_visible: bool,
}

impl CaptureState {
    pub fn reduce(self, event: CaptureEvent) -> CaptureSnapshot {
        let state = match event {
            CaptureEvent::Unavailable => Self::Unavailable,
            CaptureEvent::NotDetermined => Self::NotDetermined,
            CaptureEvent::Denied => Self::Denied,
            CaptureEvent::RestartRequired => Self::RestartRequired,
            CaptureEvent::Ready => Self::Ready,
            CaptureEvent::Failed => Self::Failed,
        };
        let (effective_mode, permission, fallback_reason) = match state {
            Self::Unavailable => (
                PetMode::Lite,
                CapturePermission::Unavailable,
                Some("capture_unavailable"),
            ),
            Self::NotDetermined | Self::Requested => (
                PetMode::Lite,
                CapturePermission::NotDetermined,
                Some("permission_not_determined"),
            ),
            Self::Denied => (
                PetMode::Lite,
                CapturePermission::Denied,
                Some("permission_denied"),
            ),
            Self::RestartRequired => (
                PetMode::Lite,
                CapturePermission::RestartRequired,
                Some("permission_restart_required"),
            ),
            Self::Ready => (PetMode::Real, CapturePermission::Granted, None),
            Self::Failed => (
                PetMode::Lite,
                CapturePermission::Granted,
                Some("capture_failed"),
            ),
        };
        CaptureSnapshot {
            effective_mode,
            permission,
            fallback_reason: fallback_reason.map(str::to_owned),
            pet_visible: true,
        }
    }
}

impl From<NativeCaptureState> for CaptureEvent {
    fn from(state: NativeCaptureState) -> Self {
        match state {
            NativeCaptureState::Unavailable => Self::Unavailable,
            NativeCaptureState::NotDetermined => Self::NotDetermined,
            NativeCaptureState::Denied => Self::Denied,
            NativeCaptureState::RestartRequired => Self::RestartRequired,
            NativeCaptureState::Ready => Self::Ready,
            NativeCaptureState::Failed => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifeAction {
    StopCapture,
    PauseRender,
    EnumerateDisplays,
    CheckPermission,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifeEvent {
    Hidden,
    Sleep,
    Wake,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifeState {
    sleeping: bool,
}

impl LifeState {
    pub fn active_real() -> Self {
        Self { sleeping: false }
    }

    pub fn sleeping_real() -> Self {
        Self { sleeping: true }
    }

    pub fn reduce(&mut self, event: LifeEvent) -> Vec<LifeAction> {
        match event {
            LifeEvent::Hidden | LifeEvent::Sleep => {
                self.sleeping = matches!(event, LifeEvent::Sleep);
                vec![LifeAction::StopCapture, LifeAction::PauseRender]
            }
            LifeEvent::Wake => {
                self.sleeping = false;
                vec![LifeAction::EnumerateDisplays, LifeAction::CheckPermission]
            }
        }
    }
}

struct CallbackRegistration {
    active: AtomicBool,
}

impl CallbackRegistration {
    fn install(sender: mpsc::Sender<NativeEvent>) -> Self {
        let callback_sender = CALLBACK_SENDER.get_or_init(|| Mutex::new(None));
        *callback_sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(sender);
        Self {
            active: AtomicBool::new(true),
        }
    }

    fn clear(&self) {
        if !self.active.swap(false, Ordering::AcqRel) {
            return;
        }
        if let Some(callback_sender) = CALLBACK_SENDER.get() {
            *callback_sender
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        }
    }
}

impl Drop for CallbackRegistration {
    fn drop(&mut self) {
        self.clear();
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeEvent {
    Clicked,
    Moved { x: f64, y: f64, display_id: u64 },
    DropEntered,
    DropExited,
    FileDropped(PathBuf),
    DisplayChanged { x: f64, y: f64, display_id: u64 },
    PermissionChanged(NativeCaptureState),
    CaptureFailed,
    Sleep,
    Wake,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PetSignal {
    ImportSucceeded { job_id: Uuid, pending_count: u32 },
    ImportFailed { code: String },
    SettlementCompleted { pending_count: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceAction {
    ShowMain,
    ShowPet,
}

#[derive(Default)]
pub struct InstanceRecall {
    progress: Mutex<RecallProgress>,
}

#[derive(Default)]
struct RecallProgress {
    requested: u64,
    completed: u64,
}

impl InstanceRecall {
    pub fn request(&self) -> u64 {
        let mut progress = self
            .progress
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        progress.requested = progress
            .requested
            .checked_add(1)
            .expect("instance recall counter overflowed");
        progress.requested
    }

    pub fn pending_request(&self) -> Option<u64> {
        let progress = self
            .progress
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (progress.requested > progress.completed).then_some(progress.requested)
    }

    pub fn mark_completed(&self, request: u64) {
        let mut progress = self
            .progress
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        progress.completed = progress.completed.max(request);
    }
}

struct RuntimeState {
    pet: NativePet,
    settings: PetSettings,
    pending_count: u32,
    capture_state: NativeCaptureState,
}

impl RuntimeState {
    fn config(&self, request_permission: bool) -> PetNativeConfig {
        let mut config = PetNativeConfig::from_settings(&self.settings, request_permission);
        config.pending_count = self.pending_count;
        config
    }

    fn apply(&mut self, request_permission: bool) -> bool {
        let applied = self.pet.apply(self.config(request_permission));
        self.capture_state = self.pet.capture_state();
        applied
    }

    fn status(&self) -> PetStatus {
        if self.settings.mode == PetMode::Lite {
            return PetStatus {
                effective_mode: PetMode::Lite,
                permission: CapturePermission::Unavailable,
                fallback_reason: None,
            };
        }

        #[cfg(not(target_os = "macos"))]
        return PetStatus {
            effective_mode: PetMode::Lite,
            permission: CapturePermission::Unavailable,
            fallback_reason: Some("platform_unsupported".to_owned()),
        };

        #[cfg(target_os = "macos")]
        {
            let snapshot = CaptureState::Requested.reduce(self.capture_state.into());
            PetStatus {
                effective_mode: snapshot.effective_mode,
                permission: snapshot.permission,
                fallback_reason: snapshot.fallback_reason,
            }
        }
    }
}

pub struct PetRuntime {
    app: AppHandle,
    state: Arc<Mutex<RuntimeState>>,
    callback: CallbackRegistration,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl PetRuntime {
    pub fn start(
        app: AppHandle,
        settings: PetSettings,
        pending: PendingSummary,
    ) -> std::result::Result<Self, NativePetError> {
        let (sender, receiver) = mpsc::channel();
        let callback = CallbackRegistration::install(sender);

        let pet = NativePet::new(native_callback)?;
        let state = Arc::new(Mutex::new(RuntimeState {
            pet,
            settings,
            pending_count: pending.count,
            capture_state: NativeCaptureState::Unavailable,
        }));
        state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .apply(false);
        let worker_app = app.clone();
        let worker_state = Arc::downgrade(&state);
        let worker = thread::Builder::new()
            .name("pet-runtime".to_owned())
            .spawn(move || {
                while let Ok(event) = receiver.recv() {
                    let Some(state) = worker_state.upgrade() else {
                        break;
                    };
                    handle_native_event(&worker_app, &state, event);
                }
            })
            .expect("failed to start pet runtime worker");
        Ok(Self {
            app,
            state,
            callback,
            worker: Mutex::new(Some(worker)),
        })
    }

    pub fn apply(&self, settings: PetSettings) -> bool {
        self.apply_with_permission_request(settings, false)
    }

    pub fn apply_with_permission_request(
        &self,
        settings: PetSettings,
        request_permission: bool,
    ) -> bool {
        let visible = settings.visible;
        let applied = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.settings = settings;
            state.apply(request_permission)
        };
        crate::tray::sync_pet_visibility(&self.app, visible);
        applied
    }

    pub fn status(&self) -> PetStatus {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status()
    }

    pub fn refresh_pending(&self, summary: PendingSummary, signal: Option<PetSignal>) {
        refresh_pending_state(&self.state, summary, signal);
    }

    pub fn show(&self) {
        self.set_visible(true);
    }

    pub fn hide(&self) {
        self.set_visible(false);
    }

    pub fn toggle(&self) {
        if self.is_visible() {
            self.hide();
        } else {
            self.show();
        }
    }

    pub fn reset(&self) {
        let saved = self
            .app
            .state::<PrintState>()
            .lock()
            .map_err(|_| AppError::Database("print lock poisoned".to_owned()))
            .and_then(|service| {
                PetStore::apply(
                    &service.database,
                    PetSettingsPatch {
                        reset_position: Some(true),
                        ..Default::default()
                    },
                )
            });
        match saved {
            Ok(settings) => {
                self.apply(settings);
                self.state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .pet
                    .reset();
            }
            Err(error) => eprintln!("pet reset failed: {}", error.code()),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .settings
            .visible
    }

    pub fn pending_count(&self) -> u32 {
        pending_count_state(&self.state)
    }

    pub fn shutdown(&self) {
        self.callback.clear();
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            let _ = worker.join();
        }
    }

    fn set_visible(&self, visible: bool) {
        let saved = self
            .app
            .state::<PrintState>()
            .lock()
            .map_err(|_| AppError::Database("print lock poisoned".to_owned()))
            .and_then(|service| {
                PetStore::apply(
                    &service.database,
                    PetSettingsPatch {
                        visible: Some(visible),
                        ..Default::default()
                    },
                )
            });
        match saved {
            Ok(settings) => {
                self.apply(settings);
            }
            Err(error) => {
                eprintln!("pet visibility failed: {}", error.code());
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.settings.visible = visible;
                if visible {
                    state.pet.show();
                } else {
                    state.pet.hide();
                }
                crate::tray::sync_pet_visibility(&self.app, visible);
            }
        }
    }
}

impl Drop for PetRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn handle_native_event(app: &AppHandle, state: &Arc<Mutex<RuntimeState>>, event: NativeEvent) {
    match event {
        NativeEvent::Clicked => open_from_pet(app),
        NativeEvent::Moved { x, y, display_id }
        | NativeEvent::DisplayChanged { x, y, display_id } => {
            let persisted = app
                .state::<PrintState>()
                .lock()
                .map_err(|_| AppError::Database("print lock poisoned".to_owned()))
                .and_then(|mut service| persist_native_position(&mut service, x, y, display_id));
            if let Err(error) = persisted {
                eprintln!("pet position failed: {}", error.code());
            }
        }
        NativeEvent::FileDropped(path) => import_from_pet(app, state, &path),
        // Enter/exit animations are applied synchronously by the native hit
        // target; consuming them here keeps business work off AppKit.
        NativeEvent::DropEntered | NativeEvent::DropExited => {}
        NativeEvent::PermissionChanged(capture_state) => {
            state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .capture_state = capture_state;
        }
        NativeEvent::CaptureFailed => {
            state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .capture_state = NativeCaptureState::Failed;
        }
        NativeEvent::Sleep | NativeEvent::Wake => {}
    }
}

fn open_from_pet(app: &AppHandle) {
    let pending = app
        .state::<PrintState>()
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))
        .and_then(|service| {
            let summary = service.pending_summary()?;
            if let Some(job_id) = summary.newest_job_id {
                persist_pending_job(&service.database, &job_id.to_string())?;
            }
            Ok(summary)
        });
    match pending {
        Ok(summary) => {
            crate::tray::show_main(app);
            if let Some(job_id) = summary.newest_job_id {
                let _ = app.emit_to("main", "open-job", job_id.to_string());
            } else {
                let _ = app.emit_to("main", "open-overview", ());
            }
        }
        Err(error) => eprintln!("pet click failed: {}", error.code()),
    }
}

fn import_from_pet(app: &AppHandle, state: &Arc<Mutex<RuntimeState>>, path: &Path) {
    let outcome = app
        .state::<PrintState>()
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))
        .and_then(|mut service| handle_file_drop(&mut service, path));
    match outcome {
        Ok(
            signal @ PetSignal::ImportSucceeded {
                job_id,
                pending_count,
            },
        ) => {
            refresh_pending_state(
                state,
                PendingSummary {
                    count: pending_count,
                    newest_job_id: Some(job_id),
                },
                Some(signal),
            );
            let _ = app.emit_to("main", "open-job", job_id.to_string());
        }
        Ok(_) => {}
        Err(error) => {
            let code = error.code().to_owned();
            eprintln!("pet import failed: {code}");
            let summary = app
                .state::<PrintState>()
                .lock()
                .ok()
                .and_then(|service| service.pending_summary().ok())
                .unwrap_or(PendingSummary {
                    count: pending_count_state(state),
                    newest_job_id: None,
                });
            refresh_pending_state(
                state,
                summary,
                Some(PetSignal::ImportFailed { code: code.clone() }),
            );
            let _ = app.emit_to("main", "pet-import-error", code.clone());
            crate::tray::notify_import_error(app, &code);
        }
    }
}

fn refresh_pending_state(
    state: &Arc<Mutex<RuntimeState>>,
    summary: PendingSummary,
    signal: Option<PetSignal>,
) {
    let mut state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.pending_count = summary.count;
    state.apply(false);
    if let Some(signal) = signal {
        state.pet.signal(match signal {
            PetSignal::ImportSucceeded { .. } => SIGNAL_IMPORT_SUCCEEDED,
            PetSignal::ImportFailed { .. } => SIGNAL_IMPORT_FAILED,
            PetSignal::SettlementCompleted { .. } => SIGNAL_SETTLEMENT_COMPLETED,
        });
    }
}

fn pending_count_state(state: &Arc<Mutex<RuntimeState>>) -> u32 {
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .pending_count
}

extern "C" fn native_callback(kind: u32, payload: *const c_char, x: f64, y: f64, display_id: u64) {
    let Some(event) = copy_native_event(kind, payload, x, y, display_id) else {
        return;
    };
    if let Some(sender) = CALLBACK_SENDER
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|slot| slot.as_ref().cloned())
    {
        let _ = sender.send(event);
    }
}

pub fn handle_file_drop(service: &mut PrintService, path: &Path) -> Result<PetSignal> {
    let preview = service.import_print_file(path)?;
    let preview = if preview.state == ImportState::NewPrintConfirmationRequired {
        service.confirm_new_print(&preview.source_hash)?
    } else {
        preview
    };
    persist_pending_job(&service.database, &preview.job_id.to_string())?;
    let pending_count = service.pending_summary()?.count;
    Ok(PetSignal::ImportSucceeded {
        job_id: preview.job_id,
        pending_count,
    })
}

pub fn persist_native_position(
    service: &mut PrintService,
    x: f64,
    y: f64,
    display_id: u64,
) -> Result<()> {
    PetStore::apply(
        &service.database,
        PetSettingsPatch {
            x: Some(x),
            y: Some(y),
            display_id: Some(display_id),
            ..Default::default()
        },
    )?;
    Ok(())
}

pub fn pending_transition(before: u32, after: u32, settlement_changed: bool) -> Option<PetSignal> {
    (settlement_changed && after < before).then_some(PetSignal::SettlementCompleted {
        pending_count: after,
    })
}

pub fn second_launch_actions() -> [InstanceAction; 2] {
    [InstanceAction::ShowMain, InstanceAction::ShowPet]
}

fn copy_native_event(
    kind: u32,
    payload: *const c_char,
    x: f64,
    y: f64,
    display_id: u64,
) -> Option<NativeEvent> {
    match PetCallbackKind::try_from(kind).ok()? {
        PetCallbackKind::Clicked => Some(NativeEvent::Clicked),
        PetCallbackKind::Moved => Some(NativeEvent::Moved { x, y, display_id }),
        PetCallbackKind::DropEntered => Some(NativeEvent::DropEntered),
        PetCallbackKind::DropExited => Some(NativeEvent::DropExited),
        PetCallbackKind::FileDropped => {
            if payload.is_null() {
                return None;
            }
            // The native callback guarantees this pointer is a valid,
            // NUL-terminated string for the duration of this call only.
            let owned = unsafe { CStr::from_ptr(payload) }
                .to_string_lossy()
                .into_owned();
            Some(NativeEvent::FileDropped(PathBuf::from(owned)))
        }
        PetCallbackKind::DisplayChanged => Some(NativeEvent::DisplayChanged { x, y, display_id }),
        PetCallbackKind::PermissionChanged => {
            if payload.is_null() {
                return None;
            }
            let state = unsafe { CStr::from_ptr(payload) }.to_bytes();
            let state = match state {
                b"unavailable" => NativeCaptureState::Unavailable,
                b"not_determined" => NativeCaptureState::NotDetermined,
                b"denied" => NativeCaptureState::Denied,
                b"restart_required" => NativeCaptureState::RestartRequired,
                b"ready" => NativeCaptureState::Ready,
                b"failed" => NativeCaptureState::Failed,
                _ => return None,
            };
            Some(NativeEvent::PermissionChanged(state))
        }
        PetCallbackKind::CaptureFailed => Some(NativeEvent::CaptureFailed),
        PetCallbackKind::Sleep => Some(NativeEvent::Sleep),
        PetCallbackKind::Wake => Some(NativeEvent::Wake),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        copy_native_event, handle_file_drop, pending_transition, persist_native_position,
        second_launch_actions, CallbackRegistration, CaptureEvent, CaptureState, InstanceAction,
        InstanceRecall, LifeAction, LifeEvent, LifeState, NativeEvent, PetSignal,
    };
    use crate::pet::{PetMode, PetStore};
    use crate::{
        db::AppDatabase,
        domain::JobOutcome,
        imports::{PrintService, ToolMapping},
        inventory::{InventoryService, NewSpool},
    };
    use std::{
        ffi::CString,
        path::{Path, PathBuf},
        sync::mpsc,
        time::Duration,
    };

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn balance_rows(db: &AppDatabase) -> Vec<(String, f64)> {
        let mut statement = db
            .connection
            .prepare("SELECT spool_id, remaining_grams FROM spools ORDER BY spool_id")
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    #[test]
    fn dropped_file_creates_pending_job_without_changing_balances() {
        let db = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(db, Duration::ZERO);
        let before = balance_rows(&service.database);

        let signal = handle_file_drop(&mut service, &fixture("bambu_multicolor.3mf")).unwrap();

        assert!(matches!(
            signal,
            PetSignal::ImportSucceeded {
                pending_count: 1,
                ..
            }
        ));
        assert_eq!(balance_rows(&service.database), before);
    }

    #[test]
    fn dropping_a_settled_file_confirms_a_fresh_pending_job_without_deducting_again() {
        let db = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(db);
        let basic = inventory
            .create_spool(NewSpool {
                display_name: "Basic red".to_owned(),
                preset_id: Some("Bambu PLA Basic @BBL A1".to_owned()),
                brand: "Bambu Lab".to_owned(),
                material: "PLA".to_owned(),
                series: "Basic".to_owned(),
                color_hex: "#FF0000".to_owned(),
                remaining_grams: 1000.0,
            })
            .unwrap();
        let matte = inventory
            .create_spool(NewSpool {
                display_name: "Matte green".to_owned(),
                preset_id: Some("Bambu PLA Matte @BBL A1".to_owned()),
                brand: "Bambu Lab".to_owned(),
                material: "PLA".to_owned(),
                series: "Matte".to_owned(),
                color_hex: "#00FF00".to_owned(),
                remaining_grams: 1000.0,
            })
            .unwrap();
        inventory.mount_spool(1, basic).unwrap();
        inventory.mount_spool(3, matte).unwrap();
        let mut service =
            PrintService::with_stability_delay(inventory.into_database(), Duration::ZERO);
        let path = fixture("bambu_multicolor.3mf");
        let settled = service.import_print_file(&path).unwrap();
        service
            .confirm_job_mapping(
                settled.job_id,
                vec![
                    ToolMapping {
                        tool: 0,
                        spool_id: basic,
                    },
                    ToolMapping {
                        tool: 1,
                        spool_id: matte,
                    },
                ],
            )
            .unwrap();
        service
            .settle_job(settled.job_id, JobOutcome::Success)
            .unwrap();
        let balances_after_settlement = balance_rows(&service.database);

        let signal = handle_file_drop(&mut service, &path).unwrap();

        let PetSignal::ImportSucceeded {
            job_id,
            pending_count,
        } = signal
        else {
            panic!("drop did not produce an import-success signal");
        };
        assert_ne!(job_id, settled.job_id);
        assert_eq!(pending_count, 1);
        assert_eq!(
            service.pending_summary().unwrap().newest_job_id,
            Some(job_id)
        );
        assert_eq!(balance_rows(&service.database), balances_after_settlement);
    }

    #[test]
    fn pending_summary_selects_newest_unsettled_job() {
        let db = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(db, Duration::ZERO);
        let first = service
            .import_print_file(&fixture("bambu_multicolor.3mf"))
            .unwrap();
        service
            .database
            .connection
            .execute(
                "UPDATE print_jobs SET outcome='{\"kind\":\"success\"}', settlement_version=1 WHERE job_id=?1",
                [first.job_id.to_string()],
            )
            .unwrap();
        let newest = service
            .confirm_new_print(&first.source_hash)
            .unwrap()
            .job_id;

        let summary = service.pending_summary().unwrap();

        assert_eq!(summary.count, 1);
        assert_eq!(summary.newest_job_id, Some(newest));
    }

    #[test]
    fn settlement_reduces_pending_count_and_flashes_green_once() {
        assert_eq!(
            pending_transition(1, 0, true),
            Some(PetSignal::SettlementCompleted { pending_count: 0 })
        );
        assert_eq!(pending_transition(0, 0, false), None);
        assert_eq!(pending_transition(0, 0, true), None);
    }

    #[test]
    fn callback_copies_file_path_before_native_storage_expires() {
        let payload = CString::new("/tmp/owned.gcode.3mf").unwrap();

        let event = copy_native_event(5, payload.as_ptr(), 0.0, 0.0, 0).unwrap();
        drop(payload);

        assert_eq!(
            event,
            NativeEvent::FileDropped(Path::new("/tmp/owned.gcode.3mf").to_path_buf())
        );
    }

    #[test]
    fn callback_copies_stable_permission_state_before_native_storage_expires() {
        use crate::pet::native::NativeCaptureState;

        let payload = CString::new("restart_required").unwrap();

        let event = copy_native_event(7, payload.as_ptr(), 0.0, 0.0, 0).unwrap();
        drop(payload);

        assert_eq!(
            event,
            NativeEvent::PermissionChanged(NativeCaptureState::RestartRequired)
        );
    }

    #[test]
    fn dropping_callback_registration_closes_the_native_event_channel() {
        let (sender, receiver) = mpsc::channel();
        let registration = CallbackRegistration::install(sender);

        drop(registration);

        assert!(matches!(
            receiver.recv_timeout(Duration::from_millis(10)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn native_move_persists_position_and_display_for_restart() {
        let db = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(db, Duration::ZERO);

        persist_native_position(&mut service, 144.5, -32.25, 73).unwrap();

        let saved = PetStore::load(&service.database).unwrap();
        assert_eq!(saved.x, Some(144.5));
        assert_eq!(saved.y, Some(-32.25));
        assert_eq!(saved.display_id, Some(73));
    }

    #[test]
    fn second_instance_only_recalls_existing_windows() {
        assert_eq!(
            second_launch_actions(),
            [InstanceAction::ShowMain, InstanceAction::ShowPet]
        );
    }

    #[test]
    fn rapid_recall_during_runtime_setup_is_not_lost() {
        let recall = InstanceRecall::default();

        let first = recall.request();
        let second = recall.request();
        recall.mark_completed(first);

        assert_eq!(recall.pending_request(), Some(second));
        recall.mark_completed(second);
        assert_eq!(recall.pending_request(), None);
    }

    #[test]
    fn denied_permission_keeps_the_lite_pet_running() {
        let status = CaptureState::Requested.reduce(CaptureEvent::Denied);

        assert_eq!(status.effective_mode, PetMode::Lite);
        assert_eq!(status.fallback_reason.as_deref(), Some("permission_denied"));
        assert!(status.pet_visible);
    }

    #[test]
    fn hiding_and_sleeping_stop_capture_and_rendering() {
        let mut life = LifeState::active_real();
        assert_eq!(
            life.reduce(LifeEvent::Hidden),
            vec![LifeAction::StopCapture, LifeAction::PauseRender]
        );

        let mut life = LifeState::active_real();
        assert_eq!(
            life.reduce(LifeEvent::Sleep),
            vec![LifeAction::StopCapture, LifeAction::PauseRender]
        );
    }

    #[test]
    fn wake_reenumerates_displays_before_rechecking_permission() {
        let mut life = LifeState::sleeping_real();

        assert_eq!(
            life.reduce(LifeEvent::Wake),
            vec![LifeAction::EnumerateDisplays, LifeAction::CheckPermission]
        );
    }
}
