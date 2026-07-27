use crate::{
    error::{AppError, Result},
    imports::{PendingSummary, PrintService, PrintState},
    pet::native::{NativePet, NativePetError, PetCallbackKind, PetNativeConfig},
    pet::{PetSettings, PetSettingsPatch, PetStore},
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
}

impl RuntimeState {
    fn config(&self) -> PetNativeConfig {
        let mut config = PetNativeConfig::from_settings(&self.settings);
        config.pending_count = self.pending_count;
        config
    }

    fn apply(&self) -> bool {
        self.pet.apply(self.config())
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
        }));
        state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .apply();
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
        let visible = settings.visible;
        let applied = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.settings = settings;
            state.apply()
        };
        crate::tray::sync_pet_visibility(&self.app, visible);
        applied
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
    state.apply();
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
    }
}

#[cfg(test)]
mod tests {
    use super::{
        copy_native_event, handle_file_drop, pending_transition, persist_native_position,
        second_launch_actions, CallbackRegistration, InstanceAction, InstanceRecall, NativeEvent,
        PetSignal,
    };
    use crate::pet::PetStore;
    use crate::{db::AppDatabase, imports::PrintService};
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
}
