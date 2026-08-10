use super::{PetFps, PetMode, PetSettings, PetVisualStyle};
use std::{error::Error, ffi::c_char, fmt};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::{
    ffi::{c_void, CString},
    ptr::NonNull,
};

pub type PetCallback =
    extern "C" fn(kind: u32, payload: *const c_char, x: f64, y: f64, event_value: u64);

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetCallbackKind {
    Clicked = 1,
    Moved = 2,
    DropEntered = 3,
    DropExited = 4,
    FileDropped = 5,
    DisplayChanged = 6,
    PermissionChanged = 7,
    CaptureFailed = 8,
    Sleep = 9,
    Wake = 10,
}

impl TryFrom<u32> for PetCallbackKind {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Clicked),
            2 => Ok(Self::Moved),
            3 => Ok(Self::DropEntered),
            4 => Ok(Self::DropExited),
            5 => Ok(Self::FileDropped),
            6 => Ok(Self::DisplayChanged),
            7 => Ok(Self::PermissionChanged),
            8 => Ok(Self::CaptureFailed),
            9 => Ok(Self::Sleep),
            10 => Ok(Self::Wake),
            _ => Err(()),
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCaptureState {
    Unavailable = 0,
    NotDetermined = 1,
    Denied = 2,
    RestartRequired = 3,
    Ready = 4,
    Failed = 5,
}

impl TryFrom<u32> for NativeCaptureState {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unavailable),
            1 => Ok(Self::NotDetermined),
            2 => Ok(Self::Denied),
            3 => Ok(Self::RestartRequired),
            4 => Ok(Self::Ready),
            5 => Ok(Self::Failed),
            _ => Err(()),
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRendererState {
    Unavailable = 0,
    Ready = 1,
}

impl TryFrom<u32> for NativeRendererState {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unavailable),
            1 => Ok(Self::Ready),
            _ => Err(()),
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeDropResult {
    Accepted = 1,
    Rejected = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeShutdownState {
    Complete,
    StopFailed,
    StopTimedOut,
}

impl NativeShutdownState {
    pub fn code(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::StopFailed => "capture_stop_failed",
            Self::StopTimedOut => "capture_stop_timed_out",
        }
    }
}

impl TryFrom<u32> for NativeShutdownState {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Complete),
            1 => Ok(Self::StopFailed),
            2 => Ok(Self::StopTimedOut),
            _ => Err(()),
        }
    }
}

const ABI_VERSION: u32 = 1;
const MODE_REAL: u32 = 0;
const MODE_LITE: u32 = 1;
const FPS_AUTO: u32 = 0;
const FPS_30: u32 = 30;
const FPS_60: u32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetActivity {
    Idle,
    DropHover,
    Signal,
    Hidden,
}

pub fn target_fps(fps: PetFps, activity: PetActivity) -> u32 {
    if activity == PetActivity::Hidden {
        return 0;
    }
    match fps {
        PetFps::Auto => match activity {
            PetActivity::Idle => 30,
            PetActivity::DropHover | PetActivity::Signal => 60,
            PetActivity::Hidden => 0,
        },
        PetFps::Fps30 => 30,
        PetFps::Fps60 => 60,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetNativeConfig {
    pub abi_version: u32,
    pub mode: u32,
    pub effective_mode: u32,
    pub has_position: u8,
    pub size: f64,
    pub x: f64,
    pub y: f64,
    pub display_id: u64,
    pub fps: u32,
    pub visible: u8,
    pub pending_count: u32,
    pub reduce_motion: u8,
    pub request_permission: u8,
    pub visual_style: u8,
    pub _reserved: u8,
}

impl PetNativeConfig {
    pub fn lite(size: f64, fps: PetFps, visible: bool) -> Self {
        Self {
            abi_version: ABI_VERSION,
            mode: MODE_LITE,
            effective_mode: MODE_LITE,
            has_position: 0,
            size,
            x: 0.0,
            y: 0.0,
            display_id: 0,
            fps: fps_value(fps),
            visible: u8::from(visible),
            pending_count: 0,
            reduce_motion: 0,
            request_permission: 0,
            visual_style: 0,
            _reserved: 0,
        }
    }

    pub fn from_settings(settings: &PetSettings, request_permission: bool) -> Self {
        let position = settings
            .x
            .zip(settings.y)
            .zip(settings.display_id)
            .map(|((x, y), display_id)| (x, y, display_id));
        Self {
            abi_version: ABI_VERSION,
            mode: mode_value(settings.mode),
            effective_mode: mode_value(settings.mode),
            has_position: u8::from(position.is_some()),
            size: f64::from(settings.size),
            x: position.map_or(0.0, |value| value.0),
            y: position.map_or(0.0, |value| value.1),
            display_id: position.map_or(0, |value| value.2),
            fps: fps_value(settings.fps),
            visible: u8::from(settings.effective_visibility()),
            pending_count: 0,
            reduce_motion: 0,
            request_permission: u8::from(request_permission),
            visual_style: match settings.visual_style {
                PetVisualStyle::Gargantua => 0,
                PetVisualStyle::Fusion => 1,
            },
            _reserved: 0,
        }
    }

    pub fn set_effective_mode(&mut self, mode: PetMode) {
        self.effective_mode = mode_value(mode);
    }
}

fn mode_value(mode: PetMode) -> u32 {
    match mode {
        PetMode::Real => MODE_REAL,
        PetMode::Lite => MODE_LITE,
    }
}

fn fps_value(fps: PetFps) -> u32 {
    match fps {
        PetFps::Auto => FPS_AUTO,
        PetFps::Fps30 => FPS_30,
        PetFps::Fps60 => FPS_60,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativePetError;

impl fmt::Display for NativePetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("native pet creation failed")
    }
}

impl Error for NativePetError {}

#[cfg(target_os = "macos")]
mod platform {
    use super::{
        c_char, c_void, CString, NativeCaptureState, NativeRendererState, NonNull, PetCallback,
        PetNativeConfig,
    };

    unsafe extern "C" {
        fn pet_create(callback: PetCallback, metal_source: *const c_char) -> *mut c_void;
        fn pet_destroy(handle: *mut c_void) -> u32;
        fn pet_apply(handle: *mut c_void, config: PetNativeConfig) -> bool;
        fn pet_show(handle: *mut c_void);
        fn pet_hide(handle: *mut c_void);
        fn pet_reset(handle: *mut c_void);
        fn pet_signal(handle: *mut c_void, signal: u32);
        fn pet_finish_drop(handle: *mut c_void, generation: u64, result: u32);
        fn pet_capture_state(handle: *mut c_void) -> u32;
        fn pet_renderer_state(handle: *mut c_void) -> u32;
        fn pet_abi_version() -> u32;
    }

    pub fn abi_version() -> u32 {
        unsafe { pet_abi_version() }
    }

    pub struct Handle(Option<NonNull<c_void>>);

    impl Handle {
        pub fn new(callback: PetCallback) -> Option<Self> {
            let source = CString::new(include_str!("../../native/mac/tiyda/BlackHole.metal"))
                .expect("embedded Metal source contains no NUL bytes");
            NonNull::new(unsafe { pet_create(callback, source.as_ptr()) })
                .map(Some)
                .map(Self)
        }

        pub fn apply(&self, config: PetNativeConfig) -> bool {
            self.0
                .is_some_and(|handle| unsafe { pet_apply(handle.as_ptr(), config) })
        }

        pub fn show(&self) {
            if let Some(handle) = self.0 {
                unsafe { pet_show(handle.as_ptr()) }
            }
        }

        pub fn hide(&self) {
            if let Some(handle) = self.0 {
                unsafe { pet_hide(handle.as_ptr()) }
            }
        }

        pub fn reset(&self) {
            if let Some(handle) = self.0 {
                unsafe { pet_reset(handle.as_ptr()) }
            }
        }

        pub fn signal(&self, signal: u32) {
            if let Some(handle) = self.0 {
                unsafe { pet_signal(handle.as_ptr(), signal) }
            }
        }

        pub fn finish_drop(&self, generation: u64, result: u32) {
            if let Some(handle) = self.0 {
                unsafe { pet_finish_drop(handle.as_ptr(), generation, result) }
            }
        }

        pub fn capture_state(&self) -> NativeCaptureState {
            self.0
                .and_then(|handle| {
                    NativeCaptureState::try_from(unsafe { pet_capture_state(handle.as_ptr()) }).ok()
                })
                .unwrap_or(NativeCaptureState::Failed)
        }

        pub fn renderer_state(&self) -> NativeRendererState {
            self.0
                .and_then(|handle| {
                    NativeRendererState::try_from(unsafe { pet_renderer_state(handle.as_ptr()) })
                        .ok()
                })
                .unwrap_or(NativeRendererState::Unavailable)
        }

        pub fn shutdown(mut self) -> super::NativeShutdownState {
            let Some(handle) = self.0.take() else {
                return super::NativeShutdownState::Complete;
            };
            super::NativeShutdownState::try_from(unsafe { pet_destroy(handle.as_ptr()) })
                .unwrap_or(super::NativeShutdownState::StopFailed)
        }
    }

    unsafe impl Send for Handle {}

    impl Drop for Handle {
        fn drop(&mut self) {
            if let Some(handle) = self.0.take() {
                let _ = unsafe { pet_destroy(handle.as_ptr()) };
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{
        c_char, c_void, CString, NativeCaptureState, NativeRendererState, NonNull, PetCallback,
        PetNativeConfig,
    };

    unsafe extern "C" {
        fn pet_create(callback: PetCallback, hlsl_source: *const c_char) -> *mut c_void;
        fn pet_destroy(handle: *mut c_void) -> u32;
        fn pet_apply(handle: *mut c_void, config: PetNativeConfig) -> bool;
        fn pet_show(handle: *mut c_void);
        fn pet_hide(handle: *mut c_void);
        fn pet_reset(handle: *mut c_void);
        fn pet_signal(handle: *mut c_void, signal: u32);
        fn pet_finish_drop(handle: *mut c_void, generation: u64, result: u32);
        fn pet_capture_state(handle: *mut c_void) -> u32;
        fn pet_renderer_state(handle: *mut c_void) -> u32;
        fn pet_abi_version() -> u32;
    }

    pub fn abi_version() -> u32 {
        unsafe { pet_abi_version() }
    }

    pub struct Handle(Option<NonNull<c_void>>);

    impl Handle {
        pub fn new(callback: PetCallback) -> Option<Self> {
            let source = CString::new(include_str!("../../native/windows/BlackHole.hlsl"))
                .expect("embedded HLSL source contains no NUL bytes");
            NonNull::new(unsafe { pet_create(callback, source.as_ptr()) })
                .map(Some)
                .map(Self)
        }

        pub fn apply(&self, config: PetNativeConfig) -> bool {
            self.0
                .is_some_and(|handle| unsafe { pet_apply(handle.as_ptr(), config) })
        }

        pub fn show(&self) {
            if let Some(handle) = self.0 {
                unsafe { pet_show(handle.as_ptr()) }
            }
        }

        pub fn hide(&self) {
            if let Some(handle) = self.0 {
                unsafe { pet_hide(handle.as_ptr()) }
            }
        }

        pub fn reset(&self) {
            if let Some(handle) = self.0 {
                unsafe { pet_reset(handle.as_ptr()) }
            }
        }

        pub fn signal(&self, signal: u32) {
            if let Some(handle) = self.0 {
                unsafe { pet_signal(handle.as_ptr(), signal) }
            }
        }

        pub fn finish_drop(&self, generation: u64, result: u32) {
            if let Some(handle) = self.0 {
                unsafe { pet_finish_drop(handle.as_ptr(), generation, result) }
            }
        }

        pub fn capture_state(&self) -> NativeCaptureState {
            self.0
                .and_then(|handle| {
                    NativeCaptureState::try_from(unsafe { pet_capture_state(handle.as_ptr()) }).ok()
                })
                .unwrap_or(NativeCaptureState::Failed)
        }

        pub fn renderer_state(&self) -> NativeRendererState {
            self.0
                .and_then(|handle| {
                    NativeRendererState::try_from(unsafe { pet_renderer_state(handle.as_ptr()) })
                        .ok()
                })
                .unwrap_or(NativeRendererState::Unavailable)
        }

        pub fn shutdown(mut self) -> super::NativeShutdownState {
            let Some(handle) = self.0.take() else {
                return super::NativeShutdownState::Complete;
            };
            super::NativeShutdownState::try_from(unsafe { pet_destroy(handle.as_ptr()) })
                .unwrap_or(super::NativeShutdownState::StopFailed)
        }
    }

    unsafe impl Send for Handle {}

    impl Drop for Handle {
        fn drop(&mut self) {
            if let Some(handle) = self.0.take() {
                let _ = unsafe { pet_destroy(handle.as_ptr()) };
            }
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use super::{NativeCaptureState, NativeRendererState, PetCallback, PetNativeConfig};

    pub fn abi_version() -> u32 {
        super::ABI_VERSION
    }

    pub struct Handle;

    impl Handle {
        pub fn new(_callback: PetCallback) -> Option<Self> {
            Some(Self)
        }
        pub fn apply(&self, _config: PetNativeConfig) -> bool {
            false
        }
        pub fn show(&self) {}
        pub fn hide(&self) {}
        pub fn reset(&self) {}
        pub fn signal(&self, _signal: u32) {}
        pub fn finish_drop(&self, _generation: u64, _result: u32) {}
        pub fn capture_state(&self) -> NativeCaptureState {
            NativeCaptureState::Unavailable
        }
        pub fn renderer_state(&self) -> NativeRendererState {
            NativeRendererState::Unavailable
        }
        pub fn shutdown(self) -> super::NativeShutdownState {
            super::NativeShutdownState::Complete
        }
    }
}

pub fn abi_version() -> u32 {
    platform::abi_version()
}

pub struct NativePet {
    handle: platform::Handle,
}

impl NativePet {
    pub fn new(callback: PetCallback) -> Result<Self, NativePetError> {
        platform::Handle::new(callback)
            .map(|handle| Self { handle })
            .ok_or(NativePetError)
    }

    pub fn apply(&self, config: PetNativeConfig) -> bool {
        self.handle.apply(config)
    }
    pub fn show(&self) {
        self.handle.show();
    }
    pub fn hide(&self) {
        self.handle.hide();
    }
    pub fn reset(&self) {
        self.handle.reset();
    }
    pub fn signal(&self, signal: u32) {
        self.handle.signal(signal);
    }
    pub fn finish_drop(&self, generation: u64, result: NativeDropResult) {
        self.handle.finish_drop(generation, result as u32);
    }
    pub fn capture_state(&self) -> NativeCaptureState {
        self.handle.capture_state()
    }
    pub fn renderer_state(&self) -> NativeRendererState {
        self.handle.renderer_state()
    }
    pub fn shutdown(self) -> NativeShutdownState {
        self.handle.shutdown()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    extern "C" fn test_callback(
        _kind: u32,
        _payload: *const c_char,
        _x: f64,
        _y: f64,
        _event_value: u64,
    ) {
    }

    #[test]
    fn only_the_tiyda_black_hole_stack_crosses_the_native_boundary() {
        const BUILD_RS: &str = include_str!("../../build.rs");
        const NATIVE_RS: &str = include_str!("native.rs");
        const BRIDGE_H: &str = include_str!("../../native/mac/bridge.h");
        assert!(BUILD_RS.contains("tiyda/MetalBlackHoleView.m"));
        assert!(NATIVE_RS.contains("tiyda/BlackHole.metal"));
        assert!(!BUILD_RS.contains("\"native/mac/capture.mm\""));
        assert!(!BUILD_RS.contains("\"native/mac/render.mm\""));
        let removed_test_renderer = ["pet_test_", "render_rgba"].concat();
        assert!(!NATIVE_RS.contains(&removed_test_renderer));
        assert!(!BRIDGE_H.contains("mac_capture_"));
        assert!(!BRIDGE_H.contains("mac_renderer_"));
    }

    #[test]
    fn windows_and_macos_use_separate_native_sources() {
        const BUILD_RS: &str = include_str!("../../build.rs");
        const WINDOWS_BRIDGE: &str = include_str!("../../native/windows/bridge.h");
        assert!(BUILD_RS.contains("native/windows/pet_bridge.cpp"));
        assert!(WINDOWS_BRIDGE.contains("typedef struct"));
        assert!(WINDOWS_BRIDGE.contains("uint8_t visual_style"));
        assert!(!WINDOWS_BRIDGE.contains("metal_source"));
        assert!(!WINDOWS_BRIDGE.contains("MetalBlackHoleView"));
    }

    #[test]
    fn native_abi_layout_is_stable() {
        assert_eq!(abi_version(), ABI_VERSION);
        assert_eq!(size_of::<PetNativeConfig>(), 64);
        assert_eq!(align_of::<PetNativeConfig>(), 8);
        assert_eq!(offset_of!(PetNativeConfig, abi_version), 0);
        assert_eq!(offset_of!(PetNativeConfig, mode), 4);
        assert_eq!(offset_of!(PetNativeConfig, effective_mode), 8);
        assert_eq!(offset_of!(PetNativeConfig, has_position), 12);
        assert_eq!(offset_of!(PetNativeConfig, size), 16);
        assert_eq!(offset_of!(PetNativeConfig, x), 24);
        assert_eq!(offset_of!(PetNativeConfig, y), 32);
        assert_eq!(offset_of!(PetNativeConfig, display_id), 40);
        assert_eq!(offset_of!(PetNativeConfig, fps), 48);
        assert_eq!(offset_of!(PetNativeConfig, visible), 52);
        assert_eq!(offset_of!(PetNativeConfig, pending_count), 56);
        assert_eq!(offset_of!(PetNativeConfig, reduce_motion), 60);
        assert_eq!(offset_of!(PetNativeConfig, request_permission), 61);
        assert_eq!(offset_of!(PetNativeConfig, visual_style), 62);
    }

    #[test]
    fn callback_kinds_decode_only_stable_events() {
        for value in 1..=10 {
            assert!(PetCallbackKind::try_from(value).is_ok());
        }
        assert!(PetCallbackKind::try_from(0).is_err());
        assert!(PetCallbackKind::try_from(11).is_err());
    }

    #[test]
    fn settings_map_to_the_native_contract() {
        let settings = PetSettings {
            mode: PetMode::Real,
            visual_style: PetVisualStyle::Fusion,
            size: 600,
            fps: PetFps::Fps60,
            visible: true,
            x: Some(10.0),
            y: Some(20.0),
            display_id: Some(42),
        };
        let config = PetNativeConfig::from_settings(&settings, true);
        assert_eq!(config.mode, MODE_REAL);
        assert_eq!(config.visual_style, 1);
        assert_eq!(config.fps, FPS_60);
        assert_eq!(config.has_position, 1);
        assert_eq!(config.display_id, 42);
        assert_eq!(config.request_permission, 1);
    }

    #[test]
    fn automatic_and_fixed_frame_rates_remain_stable() {
        assert_eq!(target_fps(PetFps::Auto, PetActivity::Idle), 30);
        assert_eq!(target_fps(PetFps::Auto, PetActivity::DropHover), 60);
        assert_eq!(target_fps(PetFps::Fps30, PetActivity::Signal), 30);
        assert_eq!(target_fps(PetFps::Fps60, PetActivity::Hidden), 0);
    }

    #[test]
    fn native_handle_is_sendable() {
        fn assert_send<T: Send>() {}
        assert_send::<NativePet>();
        let pet = NativePet::new(test_callback).expect("native handle");
        assert!(pet.apply(PetNativeConfig::lite(300.0, PetFps::Auto, false)));
        assert_eq!(pet.shutdown(), NativeShutdownState::Complete);
    }
}
