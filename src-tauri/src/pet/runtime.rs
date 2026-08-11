use crate::{
    error::{AppError, Result},
    imports::{ImportState, PendingSummary, PrintService, PrintState},
    pet::input::DropValidation,
    pet::native::{
        NativeCaptureState, NativeDropResult, NativePet, NativePetError, NativeRendererState,
        NativeShutdownState, PetCallbackKind, PetNativeConfig,
    },
    pet::{CapturePermission, PetMode, PetSettings, PetSettingsPatch, PetStatus, PetStore},
    slicer::{inspect_3mf_content, ThreeMfKind},
    tray::{pending_navigation_for_job, persist_pending_job, PendingNavigation},
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

fn capture_status(
    mode: PetMode,
    capture_state: NativeCaptureState,
    renderer_state: NativeRendererState,
    presentation_unavailable: bool,
) -> PetStatus {
    if mode == PetMode::Lite {
        return PetStatus {
            effective_mode: PetMode::Lite,
            permission: CapturePermission::Unavailable,
            fallback_reason: None,
        };
    }

    let snapshot = CaptureState::Requested.reduce(capture_state.into());
    if renderer_state == NativeRendererState::Unavailable {
        #[cfg(target_os = "windows")]
        let fallback_reason = "direct3d_unavailable";
        #[cfg(not(target_os = "windows"))]
        let fallback_reason = "metal_unavailable";
        return PetStatus {
            effective_mode: PetMode::Lite,
            permission: snapshot.permission,
            fallback_reason: Some(fallback_reason.to_owned()),
        };
    }

    if presentation_unavailable {
        return PetStatus {
            effective_mode: PetMode::Lite,
            permission: snapshot.permission,
            fallback_reason: Some("presentation_unavailable".to_owned()),
        };
    }

    PetStatus {
        effective_mode: snapshot.effective_mode,
        permission: snapshot.permission,
        fallback_reason: snapshot.fallback_reason,
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
    FileDropped { generation: u64, path: PathBuf },
    DisplayChanged { x: f64, y: f64, display_id: u64 },
    PermissionChanged(NativeCaptureState),
    CaptureFailed,
    RendererUnavailable,
    RendererReady,
    PresentationUnavailable,
    PresentationReady,
    Sleep,
    Wake,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PetSignal {
    SliceRequested {
        path: PathBuf,
    },
    ImportSucceeded {
        job_id: Uuid,
        pending_count: u32,
    },
    ProjectImportSucceeded {
        navigation: PendingNavigation,
        plate_count: u32,
        pending_count: u32,
    },
    NewProjectConfirmationRequired {
        project_id: Uuid,
        source_hash: String,
        source_path: PathBuf,
        plate_count: u32,
    },
    ImportFailed {
        code: String,
    },
    SettlementCompleted {
        pending_count: u32,
    },
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

struct NativeOwner<T> {
    native: Option<T>,
}

impl<T> NativeOwner<T> {
    fn new(native: T) -> Self {
        Self {
            native: Some(native),
        }
    }

    fn as_ref(&self) -> Option<&T> {
        self.native.as_ref()
    }

    fn take(&mut self) -> Option<T> {
        self.native.take()
    }
}

trait RuntimeNative: Send {
    // Routine methods must enqueue native work and return without waiting for
    // AppKit because callers may hold RuntimeState's mutex.
    fn apply(&self, config: PetNativeConfig) -> bool;
    fn show(&self);
    fn hide(&self);
    fn reset(&self);
    fn signal(&self, signal: u32);
    fn renderer_state(&self) -> NativeRendererState;
    fn finish_drop(&self, generation: u64, result: NativeDropResult);
    fn shutdown(self: Box<Self>) -> NativeShutdownState;
}

impl RuntimeNative for NativePet {
    fn apply(&self, config: PetNativeConfig) -> bool {
        NativePet::apply(self, config)
    }

    fn show(&self) {
        NativePet::show(self);
    }

    fn hide(&self) {
        NativePet::hide(self);
    }

    fn reset(&self) {
        NativePet::reset(self);
    }

    fn signal(&self, signal: u32) {
        NativePet::signal(self, signal);
    }

    fn renderer_state(&self) -> NativeRendererState {
        NativePet::renderer_state(self)
    }

    fn finish_drop(&self, generation: u64, result: NativeDropResult) {
        NativePet::finish_drop(self, generation, result);
    }

    fn shutdown(self: Box<Self>) -> NativeShutdownState {
        NativePet::shutdown(*self)
    }
}

struct RuntimeState {
    pet: NativeOwner<Box<dyn RuntimeNative>>,
    settings: PetSettings,
    pending_count: u32,
    capture_state: NativeCaptureState,
    renderer_state: NativeRendererState,
    presentation_unavailable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionMerge {
    PreserveRuntime,
    Replace,
}

#[derive(Default)]
struct RuntimeMutation {
    serial: Mutex<()>,
}

impl RuntimeMutation {
    fn apply_settings(
        &self,
        state: &Arc<Mutex<RuntimeState>>,
        incoming: PetSettings,
        request_permission: bool,
        position_merge: PositionMerge,
    ) -> bool {
        let _serial = self
            .serial
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let runtime_position = (
            state.settings.x,
            state.settings.y,
            state.settings.display_id,
        );
        state.settings = incoming;
        if position_merge == PositionMerge::PreserveRuntime {
            state.settings.x = runtime_position.0;
            state.settings.y = runtime_position.1;
            state.settings.display_id = runtime_position.2;
        }
        state.apply(request_permission)
    }

    fn persist_position(
        &self,
        service: &mut PrintService,
        state: &Arc<Mutex<RuntimeState>>,
        x: f64,
        y: f64,
        display_id: u64,
    ) -> Result<()> {
        let _serial = self
            .serial
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        persist_native_position(service, x, y, display_id)?;
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.settings.x = Some(x);
        state.settings.y = Some(y);
        state.settings.display_id = Some(display_id);
        // The native panel already moved, but this re-submission orders the
        // newest position after any concurrent settings snapshot queued from
        // another thread. The capture key suppresses a duplicate stream
        // reconfiguration when the region is already current.
        state.apply(false);
        Ok(())
    }
}

impl RuntimeState {
    fn with_native(
        pet: Box<dyn RuntimeNative>,
        settings: PetSettings,
        pending_count: u32,
        capture_state: NativeCaptureState,
    ) -> Self {
        let renderer_state = pet.renderer_state();
        Self {
            pet: NativeOwner::new(pet),
            settings,
            pending_count,
            capture_state,
            renderer_state,
            presentation_unavailable: false,
        }
    }

    fn config(&self, request_permission: bool) -> PetNativeConfig {
        let mut config = PetNativeConfig::from_settings(&self.settings, request_permission);
        config.set_effective_mode(self.status().effective_mode);
        config.pending_count = self.pending_count;
        config
    }

    fn apply(&mut self, request_permission: bool) -> bool {
        let Some(pet) = self.pet.as_ref() else {
            return false;
        };
        pet.apply(self.config(request_permission))
    }

    fn status(&self) -> PetStatus {
        if self.settings.mode == PetMode::Lite {
            return PetStatus {
                effective_mode: PetMode::Lite,
                permission: CapturePermission::Unavailable,
                fallback_reason: None,
            };
        }

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        return PetStatus {
            effective_mode: PetMode::Lite,
            permission: CapturePermission::Unavailable,
            fallback_reason: Some("platform_unsupported".to_owned()),
        };

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        return capture_status(
            self.settings.mode,
            self.capture_state,
            self.renderer_state,
            self.presentation_unavailable,
        );
    }

    fn reduce_native_status(&mut self, event: &NativeEvent) -> bool {
        match event {
            NativeEvent::PermissionChanged(capture_state) => {
                self.capture_state = *capture_state;
                true
            }
            NativeEvent::CaptureFailed => {
                self.capture_state = NativeCaptureState::Failed;
                true
            }
            NativeEvent::RendererUnavailable => {
                if self.renderer_state == NativeRendererState::Unavailable {
                    return false;
                }
                self.renderer_state = NativeRendererState::Unavailable;
                true
            }
            NativeEvent::RendererReady => {
                if self.renderer_state == NativeRendererState::Ready {
                    return false;
                }
                self.renderer_state = NativeRendererState::Ready;
                true
            }
            NativeEvent::PresentationUnavailable => {
                if self.presentation_unavailable {
                    return false;
                }
                self.presentation_unavailable = true;
                true
            }
            NativeEvent::PresentationReady => {
                if !self.presentation_unavailable {
                    return false;
                }
                self.presentation_unavailable = false;
                true
            }
            _ => false,
        }
    }
}

pub struct PetRuntime {
    app: Option<AppHandle>,
    state: Arc<Mutex<RuntimeState>>,
    mutation: Arc<RuntimeMutation>,
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
        let initial_capture_state = if settings.mode == PetMode::Real {
            NativeCaptureState::NotDetermined
        } else {
            NativeCaptureState::Unavailable
        };
        let state = Arc::new(Mutex::new(RuntimeState::with_native(
            Box::new(pet),
            settings,
            pending.count,
            initial_capture_state,
        )));
        let mutation = Arc::new(RuntimeMutation::default());
        let initial_settings = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .settings
            .clone();
        mutation.apply_settings(&state, initial_settings, false, PositionMerge::Replace);
        let worker_app = app.clone();
        let worker_state = Arc::downgrade(&state);
        let worker_mutation = Arc::clone(&mutation);
        let worker = thread::Builder::new()
            .name("pet-runtime".to_owned())
            .spawn(move || {
                while let Ok(event) = receiver.recv() {
                    let Some(state) = worker_state.upgrade() else {
                        break;
                    };
                    handle_native_event(&worker_app, &state, &worker_mutation, event);
                }
            })
            .expect("failed to start pet runtime worker");
        Ok(Self {
            app: Some(app),
            state,
            mutation,
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
        let enabled = settings.enabled();
        let visible = settings.effective_visibility();
        let applied = self.mutation.apply_settings(
            &self.state,
            settings,
            request_permission,
            PositionMerge::PreserveRuntime,
        );
        crate::tray::sync_pet_state(
            self.app.as_ref().expect("production runtime has an app"),
            enabled,
            visible,
        );
        applied
    }

    pub fn apply_replacing_position(
        &self,
        settings: PetSettings,
        request_permission: bool,
    ) -> bool {
        let enabled = settings.enabled();
        let visible = settings.effective_visibility();
        let applied = self.mutation.apply_settings(
            &self.state,
            settings,
            request_permission,
            PositionMerge::Replace,
        );
        if let Some(app) = self.app.as_ref() {
            crate::tray::sync_pet_state(app, enabled, visible);
        }
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
            .as_ref()
            .expect("production runtime has an app")
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
                self.apply_replacing_position(settings, false);
                let state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(pet) = state.pet.as_ref() {
                    pet.reset();
                }
            }
            Err(error) => eprintln!("pet reset failed: {}", error.code()),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .settings
            .effective_visibility()
    }

    pub fn pending_count(&self) -> u32 {
        pending_count_state(&self.state)
    }

    pub fn shutdown(&self) -> NativeShutdownState {
        // Closing the sender lets the worker drain any in-flight callback and
        // exit before native teardown can synchronize with AppKit.
        self.callback.clear();
        let worker = self
            .worker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(worker) = worker {
            let _ = worker.join();
        }
        let native = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pet
            .take();
        let shutdown_state = native
            .map(|native| native.shutdown())
            .unwrap_or(NativeShutdownState::Complete);
        if shutdown_state != NativeShutdownState::Complete {
            eprintln!("pet shutdown failed: {}", shutdown_state.code());
        }
        shutdown_state
    }

    fn set_visible(&self, visible: bool) {
        if !self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .settings
            .enabled()
        {
            return;
        }
        let saved = self
            .app
            .as_ref()
            .expect("production runtime has an app")
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
                if let Some(pet) = state.pet.as_ref() {
                    if visible {
                        pet.show();
                    } else {
                        pet.hide();
                    }
                }
                crate::tray::sync_pet_state(
                    self.app.as_ref().expect("production runtime has an app"),
                    state.settings.enabled(),
                    visible,
                );
            }
        }
    }
}

impl Drop for PetRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn handle_native_event(
    app: &AppHandle,
    state: &Arc<Mutex<RuntimeState>>,
    mutation: &RuntimeMutation,
    event: NativeEvent,
) {
    match event {
        NativeEvent::Clicked => open_from_pet(app),
        NativeEvent::Moved { x, y, display_id }
        | NativeEvent::DisplayChanged { x, y, display_id } => {
            let persisted = app
                .state::<PrintState>()
                .lock()
                .map_err(|_| AppError::Database("print lock poisoned".to_owned()))
                .and_then(|mut service| {
                    mutation.persist_position(&mut service, state, x, y, display_id)
                });
            if let Err(error) = persisted {
                eprintln!("pet position failed: {}", error.code());
            }
        }
        NativeEvent::FileDropped { generation, path } => {
            import_from_pet(app, state, generation, &path)
        }
        // Enter/exit animations are applied synchronously by the native hit
        // target; consuming them here keeps business work off AppKit.
        NativeEvent::DropEntered | NativeEvent::DropExited => {}
        NativeEvent::PermissionChanged(_)
        | NativeEvent::CaptureFailed
        | NativeEvent::RendererUnavailable
        | NativeEvent::RendererReady
        | NativeEvent::PresentationUnavailable
        | NativeEvent::PresentationReady => {
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.reduce_native_status(&event)
                && native_status_requires_apply(&event)
            {
                // Keep the requested mode in the capture key while applying
                // the effective Lite renderer after permission/capture/Metal
                // failures. The native capture gate suppresses unchanged
                // retries, so unrelated business events cannot spin capture.
                state.apply(false);
            }
        }
        NativeEvent::Sleep | NativeEvent::Wake => {}
    }
}

fn native_status_requires_apply(event: &NativeEvent) -> bool {
    !matches!(
        event,
        NativeEvent::PresentationUnavailable | NativeEvent::PresentationReady
    )
}

#[cfg(test)]
struct RecordingRuntimeNative {
    configs: Arc<Mutex<Vec<PetNativeConfig>>>,
    drop_results: Arc<Mutex<Vec<(u64, NativeDropResult)>>>,
    renderer_state: NativeRendererState,
    _identity: Arc<()>,
}

#[cfg(test)]
impl RuntimeNative for RecordingRuntimeNative {
    fn apply(&self, config: PetNativeConfig) -> bool {
        self.configs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(config);
        true
    }

    fn show(&self) {}
    fn hide(&self) {}
    fn reset(&self) {}
    fn signal(&self, _signal: u32) {}

    fn renderer_state(&self) -> NativeRendererState {
        self.renderer_state
    }

    fn finish_drop(&self, generation: u64, result: NativeDropResult) {
        self.drop_results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((generation, result));
    }

    fn shutdown(self: Box<Self>) -> NativeShutdownState {
        NativeShutdownState::Complete
    }
}

#[cfg(test)]
struct RuntimeCore {
    service: PrintService,
    state: Arc<Mutex<RuntimeState>>,
    configs: Arc<Mutex<Vec<PetNativeConfig>>>,
    drop_results: Arc<Mutex<Vec<(u64, NativeDropResult)>>>,
    native_identity: Arc<()>,
    fixture: PathBuf,
    fixture_mappings: Vec<crate::imports::ToolMapping>,
}

#[cfg(test)]
impl RuntimeCore {
    fn for_test_with_mapped_fixture() -> Self {
        use crate::{
            imports::ToolMapping,
            inventory::{InventoryService, NewSpool},
            pet::PetFps,
        };
        use std::time::Duration;

        let database = crate::db::AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(database);
        let basic = inventory
            .create_spool(NewSpool {
                display_name: "Basic red".to_owned(),
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
        let matte = inventory
            .create_spool(NewSpool {
                display_name: "Matte green".to_owned(),
                preset_id: Some("Bambu PLA Matte @BBL A1".to_owned()),
                catalog_id: None,
                color_name: None,
                color_code: None,
                color_hexes: vec!["#00FF00".to_owned()],
                preset_base: None,
                brand: "Bambu Lab".to_owned(),
                material: "PLA".to_owned(),
                series: "Matte".to_owned(),
                color_hex: "#00FF00".to_owned(),
                remaining_grams: 1000.0,
            })
            .unwrap();
        inventory.mount_spool(1, basic).unwrap();
        inventory.mount_spool(3, matte).unwrap();

        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("bambu_multicolor.3mf");
        let mut service =
            PrintService::with_stability_delay(inventory.into_database(), Duration::ZERO);
        let imported = service.import_print_file(&fixture).unwrap();
        let fixture_mappings = vec![
            ToolMapping {
                tool: 0,
                spool_id: basic,
            },
            ToolMapping {
                tool: 1,
                spool_id: matte,
            },
        ];
        service
            .confirm_job_mapping(imported.job_id, fixture_mappings.clone())
            .unwrap();
        service
            .settle_job(imported.job_id, crate::domain::JobOutcome::Success)
            .unwrap();

        let configs = Arc::new(Mutex::new(Vec::new()));
        let drop_results = Arc::new(Mutex::new(Vec::new()));
        let native_identity = Arc::new(());
        let native = RecordingRuntimeNative {
            configs: Arc::clone(&configs),
            drop_results: Arc::clone(&drop_results),
            renderer_state: NativeRendererState::Ready,
            _identity: Arc::clone(&native_identity),
        };
        let state = Arc::new(Mutex::new(RuntimeState {
            pet: NativeOwner::new(Box::new(native)),
            settings: PetSettings {
                mode: PetMode::Real,
                visual_style: crate::pet::PetVisualStyle::Gargantua,
                size: 220,
                fps: PetFps::Auto,
                visible: true,
                x: None,
                y: None,
                display_id: None,
            },
            pending_count: 0,
            capture_state: NativeCaptureState::Ready,
            renderer_state: NativeRendererState::Ready,
            presentation_unavailable: false,
        }));
        state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .apply(false);

        Self {
            service,
            state,
            configs,
            drop_results,
            native_identity,
            fixture,
            fixture_mappings,
        }
    }

    fn reduce(&mut self, event: NativeEvent) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.reduce_native_status(&event)
            && native_status_requires_apply(&event)
        {
            state.apply(false);
        }
    }

    fn import_fixture(&mut self) -> Result<crate::imports::ImportPreview> {
        let preview = self.service.import_print_file(&self.fixture)?;
        let preview = if preview.state == ImportState::NewPrintConfirmationRequired {
            self.service.confirm_new_print(&preview.source_hash)?
        } else {
            preview
        };
        self.service
            .confirm_job_mapping(preview.job_id, self.fixture_mappings.clone())?;
        Ok(preview)
    }

    fn settle_success(&mut self, job_id: Uuid) -> Result<()> {
        self.service
            .settle_job(job_id, crate::domain::JobOutcome::Success)?;
        Ok(())
    }

    fn pending_summary(&self) -> Result<PendingSummary> {
        self.service.pending_summary()
    }

    fn status(&self) -> PetStatus {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status()
    }

    fn apply_settings(&mut self, settings: PetSettings) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.settings = settings;
        state.apply(false);
    }

    fn last_native_config(&self) -> PetNativeConfig {
        *self
            .configs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .last()
            .expect("recording native received a config")
    }

    fn service_identity(&self) -> usize {
        std::ptr::from_ref(&self.service) as usize
    }

    fn native_identity(&self) -> usize {
        Arc::as_ptr(&self.native_identity) as usize
    }

    fn handle(&mut self, event: NativeEvent) {
        if let NativeEvent::FileDropped { generation, path } = event {
            let _ = process_pet_drop(&mut self.service, &self.state, generation, &path);
        } else {
            self.reduce(event);
        }
    }

    fn drop_results(&self) -> Vec<(u64, NativeDropResult)> {
        self.drop_results
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn balance_rows(&self) -> Vec<(String, f64)> {
        let mut statement = self
            .service
            .database
            .connection
            .prepare("SELECT spool_id, remaining_grams FROM spools ORDER BY spool_id")
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }
}

fn open_from_pet(app: &AppHandle) {
    let pending = app
        .state::<PrintState>()
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))
        .and_then(|service| {
            let summary = service.pending_summary()?;
            let navigation = summary
                .newest_job_id
                .map(|job_id| pending_navigation_for_job(&service.database, job_id))
                .transpose()?
                .flatten();
            if let Some(navigation) = &navigation {
                persist_pending_job(&service.database, &navigation.job_id.to_string())?;
            } else if let Some(job_id) = summary.newest_job_id {
                persist_pending_job(&service.database, &job_id.to_string())?;
            }
            Ok((summary, navigation))
        });
    match pending {
        Ok((summary, navigation)) => {
            crate::tray::show_main(app);
            if let Some(navigation) = navigation {
                let _ = app.emit_to("main", "open-project", navigation.clone());
            } else if let Some(job_id) = summary.newest_job_id {
                let _ = app.emit_to("main", "open-job", job_id.to_string());
            } else {
                let _ = app.emit_to("main", "open-overview", ());
            }
        }
        Err(error) => eprintln!("pet click failed: {}", error.code()),
    }
}

fn import_from_pet(
    app: &AppHandle,
    state: &Arc<Mutex<RuntimeState>>,
    generation: u64,
    path: &Path,
) {
    let outcome = match app.state::<PrintState>().lock() {
        Ok(mut service) => process_pet_drop(&mut service, state, generation, path),
        Err(_) => {
            let error = AppError::Database("print lock poisoned".to_owned());
            refresh_pending_state(
                state,
                PendingSummary {
                    count: pending_count_state(state),
                    newest_job_id: None,
                },
                None,
            );
            finish_native_drop(state, generation, NativeDropResult::Rejected);
            Err(error)
        }
    };
    match outcome {
        Ok(PetSignal::SliceRequested { path }) => {
            crate::tray::show_main(app);
            let _ = app.emit_to("main", "open-slice", path.to_string_lossy().into_owned());
        }
        Ok(PetSignal::ProjectImportSucceeded { navigation, .. }) => {
            crate::tray::show_main(app);
            let _ = app.emit_to("main", "open-project", navigation.clone());
        }
        Ok(PetSignal::NewProjectConfirmationRequired {
            project_id,
            source_hash,
            source_path,
            plate_count,
        }) => {
            crate::tray::show_main(app);
            let _ = app.emit_to(
                "main",
                "confirm-new-project",
                serde_json::json!({
                    "project_id": project_id,
                    "source_hash": source_hash,
                    "source_path": source_path,
                    "plate_count": plate_count,
                }),
            );
        }
        Ok(PetSignal::ImportSucceeded { job_id, .. }) => {
            crate::tray::show_main(app);
            let _ = app.emit_to("main", "open-job", job_id.to_string());
        }
        Ok(_) => unreachable!("file drop returned a non-import signal"),
        Err(error) => {
            let code = error.code().to_owned();
            eprintln!("pet import failed: {}", error.code());
            let _ = app.emit_to("main", "pet-import-error", code.clone());
            crate::tray::notify_import_error(app, &code);
        }
    }
}

fn process_pet_drop(
    service: &mut PrintService,
    state: &Arc<Mutex<RuntimeState>>,
    generation: u64,
    path: &Path,
) -> Result<PetSignal> {
    match handle_file_drop(service, path) {
        Ok(signal @ PetSignal::SliceRequested { .. }) => {
            finish_native_drop(state, generation, NativeDropResult::Accepted);
            Ok(signal)
        }
        Ok(PetSignal::ProjectImportSucceeded {
            navigation,
            plate_count,
            pending_count,
        }) => {
            refresh_pending_state(
                state,
                PendingSummary {
                    count: pending_count,
                    newest_job_id: Some(navigation.job_id),
                },
                None,
            );
            finish_native_drop(state, generation, NativeDropResult::Accepted);
            Ok(PetSignal::ProjectImportSucceeded {
                navigation,
                plate_count,
                pending_count,
            })
        }
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
                None,
            );
            finish_native_drop(state, generation, NativeDropResult::Accepted);
            Ok(signal)
        }
        Ok(signal @ PetSignal::NewProjectConfirmationRequired { .. }) => {
            let summary = service.pending_summary().unwrap_or(PendingSummary {
                count: pending_count_state(state),
                newest_job_id: None,
            });
            refresh_pending_state(state, summary, None);
            finish_native_drop(state, generation, NativeDropResult::Accepted);
            Ok(signal)
        }
        Ok(_) => unreachable!("file import only returns an import result"),
        Err(error) => {
            let summary = service.pending_summary().unwrap_or(PendingSummary {
                count: pending_count_state(state),
                newest_job_id: None,
            });
            refresh_pending_state(state, summary, None);
            finish_native_drop(state, generation, NativeDropResult::Rejected);
            Err(error)
        }
    }
}

fn finish_native_drop(state: &Arc<Mutex<RuntimeState>>, generation: u64, result: NativeDropResult) {
    let state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(pet) = state.pet.as_ref() {
        pet.finish_drop(generation, result);
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
    if let (Some(signal), Some(pet)) = (signal, state.pet.as_ref()) {
        pet.signal(match signal {
            PetSignal::SliceRequested { .. }
            | PetSignal::ImportSucceeded { .. }
            | PetSignal::ProjectImportSucceeded { .. }
            | PetSignal::NewProjectConfirmationRequired { .. } => SIGNAL_IMPORT_SUCCEEDED,
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

extern "C" fn native_callback(kind: u32, payload: *const c_char, x: f64, y: f64, event_value: u64) {
    let Some(event) = copy_native_event(kind, payload, x, y, event_value) else {
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
    let validation = DropValidation::read(path).map_err(|_| AppError::InvalidFile)?;
    if validation
        .canonical_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("3mf"))
    {
        if inspect_3mf_content(&validation.canonical_path)
            .is_ok_and(|inspection| inspection.kind == ThreeMfKind::Unsliced)
        {
            return Ok(PetSignal::SliceRequested {
                path: validation.canonical_path,
            });
        }
    }
    let preview = service.import_print_project(&validation.canonical_path)?;
    if preview.state == ImportState::NewPrintConfirmationRequired {
        return Ok(PetSignal::NewProjectConfirmationRequired {
            project_id: preview.project_id,
            source_hash: preview.source_hash,
            source_path: validation.canonical_path,
            plate_count: preview.plates.len() as u32,
        });
    }
    let first_pending = preview
        .plates
        .iter()
        .find(|plate| {
            matches!(
                plate.status,
                crate::domain::PlateStatus::PendingMapping | crate::domain::PlateStatus::Ready
            )
        })
        .ok_or(AppError::InvalidJob)?;
    let navigation = PendingNavigation {
        project_id: preview.project_id,
        plate_id: first_pending.plate_id,
        job_id: first_pending.job_id,
    };
    persist_pending_job(&service.database, &navigation.job_id.to_string())?;
    let pending_count = service.pending_summary()?.count;
    Ok(PetSignal::ProjectImportSucceeded {
        navigation,
        plate_count: preview.plates.len() as u32,
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
    event_value: u64,
) -> Option<NativeEvent> {
    match PetCallbackKind::try_from(kind).ok()? {
        PetCallbackKind::Clicked => Some(NativeEvent::Clicked),
        PetCallbackKind::Moved => Some(NativeEvent::Moved {
            x,
            y,
            display_id: event_value,
        }),
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
            Some(NativeEvent::FileDropped {
                generation: event_value,
                path: PathBuf::from(owned),
            })
        }
        PetCallbackKind::DisplayChanged => Some(NativeEvent::DisplayChanged {
            x,
            y,
            display_id: event_value,
        }),
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
        PetCallbackKind::CaptureFailed => {
            if !payload.is_null() {
                match unsafe { CStr::from_ptr(payload) }.to_bytes() {
                    b"metal_unavailable" | b"renderer_unavailable" => {
                        return Some(NativeEvent::RendererUnavailable)
                    }
                    b"renderer_ready" => return Some(NativeEvent::RendererReady),
                    b"presentation_unavailable" => {
                        return Some(NativeEvent::PresentationUnavailable)
                    }
                    b"presentation_ready" => return Some(NativeEvent::PresentationReady),
                    _ => {}
                }
            }
            Some(NativeEvent::CaptureFailed)
        }
        PetCallbackKind::Sleep => Some(NativeEvent::Sleep),
        PetCallbackKind::Wake => Some(NativeEvent::Wake),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        capture_status, copy_native_event, handle_file_drop, pending_transition,
        persist_native_position, second_launch_actions, CallbackRegistration, CaptureEvent,
        CaptureState, InstanceAction, InstanceRecall, LifeAction, LifeEvent, LifeState,
        NativeEvent, NativeOwner, PetRuntime, PetSignal, PositionMerge, RecordingRuntimeNative,
        RuntimeCore, RuntimeMutation, RuntimeNative, RuntimeState,
    };
    use crate::pet::native::{
        NativeCaptureState, NativeDropResult, NativeRendererState, NativeShutdownState,
        PetNativeConfig,
    };
    use crate::pet::{
        CapturePermission, PetFps, PetMode, PetSettings, PetSettingsPatch, PetStore, PetVisualStyle,
    };
    use crate::{
        db::AppDatabase,
        domain::JobOutcome,
        imports::{PrintService, ToolMapping},
        inventory::{InventoryService, NewSpool},
    };
    use std::{
        ffi::CString,
        fs::File,
        io::Write,
        path::{Path, PathBuf},
        sync::{atomic::AtomicBool, mpsc, Arc, Barrier, Condvar, Mutex},
        thread,
        time::Duration,
    };

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn two_plate_fixture() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("cylune-pet-two-plate-{}.3mf", uuid::Uuid::new_v4()));
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

    fn unsliced_fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "cylune-pet-unsliced-{}.3mf",
            uuid::Uuid::new_v4()
        ));
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::FileOptions::default();
        for (name, contents) in [
            ("[Content_Types].xml", b"<Types/>".as_slice()),
            ("3D/3dmodel.model", b"<model/>".as_slice()),
            (
                "Metadata/project_settings.config",
                br##"{"printer_model":"Bambu Lab P2S","nozzle_diameter":["0.4"],"filament_settings_id":["Bambu PLA Basic @BBL P2S"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"]}"##.as_slice(),
            ),
            (
                "Metadata/model_settings.config",
                br#"<config><plate><metadata key="plater_id" value="1"/></plate></config>"#.as_slice(),
            ),
        ] {
            archive.start_file(name, options).unwrap();
            archive.write_all(contents).unwrap();
        }
        archive.finish().unwrap();
        path
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

    fn mapped_service() -> PrintService {
        PrintService::with_stability_delay(AppDatabase::open_in_memory().unwrap(), Duration::ZERO)
    }

    #[test]
    fn dropped_file_creates_pending_job_without_changing_balances() {
        let db = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(db, Duration::ZERO);
        let before = balance_rows(&service.database);

        let signal = handle_file_drop(&mut service, &fixture("bambu_multicolor.3mf")).unwrap();

        assert!(matches!(
            signal,
            PetSignal::ProjectImportSucceeded {
                pending_count: 1,
                plate_count: 1,
                ..
            }
        ));
        assert_eq!(balance_rows(&service.database), before);
    }

    #[test]
    fn unsliced_black_hole_drop_requests_slicing_without_creating_a_project() {
        let db = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(db, Duration::ZERO);
        let path = unsliced_fixture();
        let canonical = std::fs::canonicalize(&path).unwrap();

        let signal = handle_file_drop(&mut service, &path).unwrap();

        std::fs::remove_file(path).unwrap();
        assert_eq!(signal, PetSignal::SliceRequested { path: canonical });
        let project_count: u32 = service
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM print_projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(project_count, 0);
    }

    #[test]
    fn dropped_multi_plate_file_imports_one_project_and_reports_plate_count() {
        let db = AppDatabase::open_in_memory().unwrap();
        let mut service = PrintService::with_stability_delay(db, Duration::ZERO);
        let path = two_plate_fixture();

        let signal = handle_file_drop(&mut service, &path).unwrap();

        std::fs::remove_file(path).unwrap();
        let PetSignal::ProjectImportSucceeded {
            navigation,
            plate_count,
            pending_count,
        } = signal
        else {
            panic!("drop did not produce a project import signal");
        };
        assert_eq!(plate_count, 2);
        assert_eq!(pending_count, 2);
        assert_eq!(
            navigation.plate_id,
            service
                .get_project_preview(navigation.project_id)
                .unwrap()
                .plates[0]
                .plate_id
        );
        let project_count: u32 = service
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM print_projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(project_count, 1);
    }

    #[test]
    fn dropping_a_settled_source_requests_confirmation_without_creating_a_project() {
        let db = AppDatabase::open_in_memory().unwrap();
        let mut inventory = InventoryService::new(db);
        let basic = inventory
            .create_spool(NewSpool {
                display_name: "Basic red".to_owned(),
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
        let matte = inventory
            .create_spool(NewSpool {
                display_name: "Matte green".to_owned(),
                preset_id: Some("Bambu PLA Matte @BBL A1".to_owned()),
                catalog_id: None,
                color_name: None,
                color_code: None,
                color_hexes: vec!["#00FF00".to_owned()],
                preset_base: None,
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
        let settled_project = service.import_print_project(&path).unwrap();
        let settled = &settled_project.plates[0];
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

        let PetSignal::NewProjectConfirmationRequired {
            project_id,
            source_hash,
            source_path,
            plate_count,
        } = signal
        else {
            panic!("drop did not request a new project confirmation");
        };
        assert_eq!(project_id, settled_project.project_id);
        assert_eq!(source_hash, settled_project.source_hash);
        assert_eq!(source_path, path);
        assert_eq!(plate_count, 1);
        assert_eq!(service.pending_summary().unwrap().count, 0);
        let project_count: u32 = service
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM print_projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(project_count, 1);
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
        let generation = 73;

        let event = copy_native_event(5, payload.as_ptr(), 0.0, 0.0, generation).unwrap();
        drop(payload);

        assert_eq!(
            event,
            NativeEvent::FileDropped {
                generation,
                path: Path::new("/tmp/owned.gcode.3mf").to_path_buf(),
            }
        );
    }

    #[test]
    fn confirmation_ack_uses_the_same_generation_without_creating_pending_work() {
        let mut core = RuntimeCore::for_test_with_mapped_fixture();
        let generation = 41;
        let event = NativeEvent::FileDropped {
            generation,
            path: core.fixture.clone(),
        };
        core.handle(event);
        assert_eq!(core.pending_summary().unwrap().count, 0);
        let project_count: u32 = core
            .service
            .database
            .connection
            .query_row("SELECT COUNT(*) FROM print_projects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(project_count, 1);
        assert_eq!(
            core.drop_results(),
            vec![(generation, NativeDropResult::Accepted)]
        );
    }

    #[test]
    fn rejected_import_ack_does_not_change_pending_or_balances() {
        let mut core = RuntimeCore::for_test_with_mapped_fixture();
        let before = core.balance_rows();
        core.handle(NativeEvent::FileDropped {
            generation: 9,
            path: fixture("project_only.3mf"),
        });
        assert_eq!(core.pending_summary().unwrap().count, 0);
        assert_eq!(core.balance_rows(), before);
        assert_eq!(core.drop_results(), vec![(9, NativeDropResult::Rejected)]);
    }

    #[test]
    fn successful_pet_import_does_not_modify_the_source() {
        let source = fixture("bambu_multicolor.3mf");
        let bytes_before = std::fs::read(&source).unwrap();
        let metadata_before = std::fs::metadata(&source).unwrap();
        let mut service = mapped_service();
        handle_file_drop(&mut service, &source).unwrap();
        let metadata_after = std::fs::metadata(&source).unwrap();
        assert_eq!(std::fs::read(&source).unwrap(), bytes_before);
        assert_eq!(metadata_after.len(), metadata_before.len());
        assert_eq!(
            metadata_after.modified().unwrap(),
            metadata_before.modified().unwrap()
        );
    }

    #[test]
    fn callback_copies_stable_permission_state_before_native_storage_expires() {
        let payload = CString::new("restart_required").unwrap();

        let event = copy_native_event(7, payload.as_ptr(), 0.0, 0.0, 0).unwrap();
        drop(payload);

        assert_eq!(
            event,
            NativeEvent::PermissionChanged(NativeCaptureState::RestartRequired)
        );
    }

    #[test]
    fn callback_decodes_the_stable_metal_unavailable_failure() {
        let payload = CString::new("metal_unavailable").unwrap();

        let event = copy_native_event(8, payload.as_ptr(), 0.0, 0.0, 0).unwrap();

        assert_eq!(event, NativeEvent::RendererUnavailable);
    }

    #[test]
    fn callback_decodes_renderer_state_transitions_without_new_callback_kinds() {
        let unavailable = CString::new("renderer_unavailable").unwrap();
        let ready = CString::new("renderer_ready").unwrap();
        let presentation_unavailable = CString::new("presentation_unavailable").unwrap();
        let presentation_ready = CString::new("presentation_ready").unwrap();

        assert_eq!(
            copy_native_event(8, unavailable.as_ptr(), 0.0, 0.0, 0),
            Some(NativeEvent::RendererUnavailable)
        );
        assert_eq!(
            copy_native_event(8, ready.as_ptr(), 0.0, 0.0, 0),
            Some(NativeEvent::RendererReady)
        );
        assert_eq!(
            copy_native_event(8, presentation_unavailable.as_ptr(), 0.0, 0.0, 0),
            Some(NativeEvent::PresentationUnavailable)
        );
        assert_eq!(
            copy_native_event(8, presentation_ready.as_ptr(), 0.0, 0.0, 0),
            Some(NativeEvent::PresentationReady)
        );
    }

    #[test]
    fn runtime_state_queries_initial_native_renderer_availability() {
        let native = RecordingRuntimeNative {
            configs: Arc::new(Mutex::new(Vec::new())),
            drop_results: Arc::new(Mutex::new(Vec::new())),
            renderer_state: NativeRendererState::Unavailable,
            _identity: Arc::new(()),
        };
        let state = RuntimeState::with_native(
            Box::new(native),
            PetSettings {
                mode: PetMode::Real,
                visual_style: PetVisualStyle::Gargantua,
                size: 220,
                fps: PetFps::Auto,
                visible: true,
                x: None,
                y: None,
                display_id: None,
            },
            0,
            NativeCaptureState::Ready,
        );

        assert_eq!(state.renderer_state, NativeRendererState::Unavailable);
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
    fn runtime_shutdown_joins_worker_before_native_destroy_during_apply() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Event {
            WorkerExited,
            NativeShutdown,
        }

        struct QueuedNative {
            apply_started: mpsc::Sender<()>,
            events: mpsc::Sender<Event>,
        }

        impl RuntimeNative for QueuedNative {
            fn apply(&self, _config: PetNativeConfig) -> bool {
                self.apply_started.send(()).unwrap();
                true
            }

            fn show(&self) {}
            fn hide(&self) {}
            fn reset(&self) {}
            fn signal(&self, _signal: u32) {}
            fn renderer_state(&self) -> NativeRendererState {
                NativeRendererState::Ready
            }
            fn finish_drop(&self, _generation: u64, _result: NativeDropResult) {}

            fn shutdown(self: Box<Self>) -> NativeShutdownState {
                self.events.send(Event::NativeShutdown).unwrap();
                NativeShutdownState::Complete
            }
        }

        let settings = PetSettings {
            mode: PetMode::Lite,
            visual_style: PetVisualStyle::Gargantua,
            size: 220,
            fps: PetFps::Auto,
            visible: true,
            x: None,
            y: None,
            display_id: None,
        };
        let (apply_started_tx, apply_started_rx) = mpsc::channel();
        let (events_tx, events_rx) = mpsc::channel();
        let native = QueuedNative {
            apply_started: apply_started_tx,
            events: events_tx.clone(),
        };
        let state = Arc::new(Mutex::new(RuntimeState {
            pet: NativeOwner::new(Box::new(native)),
            settings,
            pending_count: 0,
            capture_state: NativeCaptureState::Unavailable,
            renderer_state: NativeRendererState::Ready,
            presentation_unavailable: false,
        }));
        let worker_state = Arc::clone(&state);
        let worker_gate = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_gate_for_thread = Arc::clone(&worker_gate);
        let worker = thread::spawn(move || {
            worker_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .apply(false);
            let (released, wake) = &*worker_gate_for_thread;
            let mut released = released
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while !*released {
                released = wake
                    .wait(released)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            events_tx.send(Event::WorkerExited).unwrap();
        });
        apply_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        let runtime = PetRuntime {
            app: None,
            state,
            mutation: Arc::new(RuntimeMutation::default()),
            callback: CallbackRegistration {
                active: AtomicBool::new(false),
            },
            worker: Mutex::new(Some(worker)),
        };
        let (shutdown_started_tx, shutdown_started_rx) = mpsc::channel();
        let shutdown = thread::spawn(move || {
            shutdown_started_tx.send(()).unwrap();
            runtime.shutdown()
        });
        shutdown_started_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap();

        assert_eq!(
            events_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        );
        let (released, wake) = &*worker_gate;
        *released
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = true;
        wake.notify_one();

        assert_eq!(
            events_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            Event::WorkerExited
        );
        assert_eq!(
            events_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            Event::NativeShutdown
        );
        assert_eq!(shutdown.join().unwrap(), NativeShutdownState::Complete);
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
    fn every_unavailable_capture_state_uses_lite_without_changing_requested_mode() {
        for (event, permission, reason) in [
            (
                CaptureEvent::NotDetermined,
                CapturePermission::NotDetermined,
                "permission_not_determined",
            ),
            (
                CaptureEvent::Denied,
                CapturePermission::Denied,
                "permission_denied",
            ),
            (
                CaptureEvent::RestartRequired,
                CapturePermission::RestartRequired,
                "permission_restart_required",
            ),
            (
                CaptureEvent::Unavailable,
                CapturePermission::Unavailable,
                "capture_unavailable",
            ),
        ] {
            let status = CaptureState::Requested.reduce(event);
            assert_eq!(status.effective_mode, PetMode::Lite);
            assert_eq!(status.permission, permission);
            assert_eq!(status.fallback_reason.as_deref(), Some(reason));
            assert!(status.pet_visible);
        }
    }

    #[test]
    fn ready_capture_restores_real_and_saved_large_fusion_settings_reload() {
        let db = AppDatabase::open_in_memory().unwrap();
        let saved = PetStore::apply(
            &db,
            PetSettingsPatch {
                mode: Some(PetMode::Real),
                size: Some(900),
                visual_style: Some(PetVisualStyle::Fusion),
                ..Default::default()
            },
        )
        .unwrap();
        let status = capture_status(
            saved.mode,
            NativeCaptureState::Ready,
            NativeRendererState::Ready,
            false,
        );
        assert_eq!(status.effective_mode, PetMode::Real);
        assert_eq!(status.permission, CapturePermission::Granted);
        assert_eq!(status.fallback_reason, None);
        let reloaded = PetStore::load(&db).unwrap();
        assert_eq!(reloaded.mode, PetMode::Real);
        assert_eq!(reloaded.size, 900);
        assert_eq!(reloaded.visual_style, PetVisualStyle::Fusion);
    }

    #[test]
    fn ready_capture_without_metal_keeps_the_visible_lite_fallback() {
        let status = capture_status(
            PetMode::Real,
            NativeCaptureState::Ready,
            NativeRendererState::Unavailable,
            true,
        );

        assert_eq!(status.effective_mode, PetMode::Lite);
        assert_eq!(status.permission, crate::pet::CapturePermission::Granted);
        assert_eq!(status.fallback_reason.as_deref(), Some("metal_unavailable"));
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

    #[test]
    fn render_failure_does_not_block_import_or_settlement() {
        let mut core = RuntimeCore::for_test_with_mapped_fixture();

        core.reduce(NativeEvent::RendererUnavailable);
        let imported = core.import_fixture().unwrap();
        core.settle_success(imported.job_id).unwrap();

        assert_eq!(core.pending_summary().unwrap().count, 0);
        assert_eq!(core.status().effective_mode, PetMode::Lite);
        assert_eq!(
            core.status().fallback_reason.as_deref(),
            Some("metal_unavailable")
        );
        assert_eq!(core.last_native_config().effective_mode, 1);
    }

    #[test]
    fn renderer_transitions_are_reduced_once_and_ready_recovers_real_mode() {
        let mut core = RuntimeCore::for_test_with_mapped_fixture();
        let initial_applies = core
            .configs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();

        core.reduce(NativeEvent::RendererUnavailable);
        core.reduce(NativeEvent::RendererUnavailable);
        assert_eq!(
            core.configs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            initial_applies + 1
        );
        assert_eq!(core.status().effective_mode, PetMode::Lite);

        core.reduce(NativeEvent::RendererReady);
        core.reduce(NativeEvent::RendererReady);
        assert_eq!(
            core.configs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            initial_applies + 2
        );
        assert_eq!(core.status().effective_mode, PetMode::Real);
        assert_eq!(core.status().fallback_reason, None);
    }

    #[test]
    fn presentation_transitions_are_reduced_once_without_changing_renderer_state() {
        let mut core = RuntimeCore::for_test_with_mapped_fixture();
        let initial_applies = core
            .configs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();

        core.reduce(NativeEvent::PresentationUnavailable);
        core.reduce(NativeEvent::PresentationUnavailable);
        assert_eq!(
            core.configs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            initial_applies
        );
        assert_eq!(core.status().effective_mode, PetMode::Lite);
        assert_eq!(
            core.status().fallback_reason.as_deref(),
            Some("presentation_unavailable")
        );

        core.reduce(NativeEvent::PresentationReady);
        core.reduce(NativeEvent::PresentationReady);
        assert_eq!(
            core.configs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            initial_applies
        );
        assert_eq!(core.status().effective_mode, PetMode::Real);
        assert_eq!(core.status().fallback_reason, None);
    }

    #[test]
    fn capture_failure_keeps_the_same_import_and_settlement_core_alive() {
        let mut core = RuntimeCore::for_test_with_mapped_fixture();
        let service_identity = core.service_identity();

        core.reduce(NativeEvent::CaptureFailed);
        let imported = core.import_fixture().unwrap();
        core.settle_success(imported.job_id).unwrap();

        assert_eq!(core.service_identity(), service_identity);
        assert_eq!(core.pending_summary().unwrap().count, 0);
        assert_eq!(core.status().effective_mode, PetMode::Lite);
        assert_eq!(
            core.status().fallback_reason.as_deref(),
            Some("capture_failed")
        );
        assert_eq!(core.last_native_config().effective_mode, 1);
    }

    #[test]
    fn live_settings_reuse_the_business_service_and_native_owner() {
        let mut core = RuntimeCore::for_test_with_mapped_fixture();
        let service_identity = core.service_identity();
        let native_identity = core.native_identity();

        core.apply_settings(PetSettings {
            mode: PetMode::Real,
            visual_style: PetVisualStyle::Gargantua,
            size: 300,
            fps: PetFps::Fps60,
            visible: false,
            x: None,
            y: None,
            display_id: None,
        });

        assert_eq!(core.service_identity(), service_identity);
        assert_eq!(core.native_identity(), native_identity);
        assert_eq!(core.last_native_config().size, 300.0);
        assert_eq!(core.last_native_config().fps, 60);
        assert_eq!(core.last_native_config().visible, 0);
        assert!(core.import_fixture().is_ok());
    }

    #[test]
    fn disabled_mode_cannot_show_even_with_a_stale_visible_flag() {
        let mut core = RuntimeCore::for_test_with_mapped_fixture();
        core.apply_settings(PetSettings {
            mode: PetMode::Lite,
            visual_style: PetVisualStyle::Gargantua,
            size: 220,
            fps: PetFps::Auto,
            visible: true,
            x: None,
            y: None,
            display_id: None,
        });

        assert_eq!(core.last_native_config().visible, 0);
    }

    #[test]
    fn stale_settings_snapshot_cannot_overwrite_a_newer_mouse_up_position() {
        let core = RuntimeCore::for_test_with_mapped_fixture();
        {
            let mut state = core
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.settings.x = Some(100.0);
            state.settings.y = Some(80.0);
            state.settings.display_id = Some(1);
            state.apply(false);
        }
        PetStore::apply(
            &core.service.database,
            crate::pet::PetSettingsPatch {
                x: Some(100.0),
                y: Some(80.0),
                display_id: Some(1),
                ..Default::default()
            },
        )
        .unwrap();

        let service = Arc::new(Mutex::new(core.service));
        let state = Arc::clone(&core.state);
        let mutation = Arc::new(RuntimeMutation::default());
        let settings_saved = Arc::new(Barrier::new(2));
        let move_completed = Arc::new(Barrier::new(2));

        let settings_thread = {
            let service = Arc::clone(&service);
            let state = Arc::clone(&state);
            let mutation = Arc::clone(&mutation);
            let settings_saved = Arc::clone(&settings_saved);
            let move_completed = Arc::clone(&move_completed);
            thread::spawn(move || {
                let stale_snapshot = {
                    let service = service
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    PetStore::apply(
                        &service.database,
                        crate::pet::PetSettingsPatch {
                            size: Some(300),
                            ..Default::default()
                        },
                    )
                    .unwrap()
                };
                settings_saved.wait();
                move_completed.wait();
                mutation.apply_settings(
                    &state,
                    stale_snapshot,
                    false,
                    PositionMerge::PreserveRuntime,
                );
            })
        };
        let move_thread = {
            let service = Arc::clone(&service);
            let state = Arc::clone(&state);
            let mutation = Arc::clone(&mutation);
            let settings_saved = Arc::clone(&settings_saved);
            let move_completed = Arc::clone(&move_completed);
            thread::spawn(move || {
                settings_saved.wait();
                mutation
                    .persist_position(
                        &mut service
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                        &state,
                        1700.0,
                        120.0,
                        2,
                    )
                    .unwrap();
                move_completed.wait();
            })
        };

        settings_thread.join().unwrap();
        move_thread.join().unwrap();

        let saved = PetStore::load(
            &service
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .database,
        )
        .unwrap();
        let runtime_settings = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .settings
            .clone();
        let last_config = *core
            .configs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .last()
            .unwrap();

        assert_eq!(saved.size, 300);
        assert_eq!(
            (saved.x, saved.y, saved.display_id),
            (Some(1700.0), Some(120.0), Some(2))
        );
        assert_eq!(runtime_settings.size, 300);
        assert_eq!(
            (
                runtime_settings.x,
                runtime_settings.y,
                runtime_settings.display_id,
            ),
            (Some(1700.0), Some(120.0), Some(2))
        );
        assert_eq!(last_config.size, 300.0);
        assert_eq!(
            (last_config.x, last_config.y, last_config.display_id),
            (1700.0, 120.0, 2)
        );
    }
}
