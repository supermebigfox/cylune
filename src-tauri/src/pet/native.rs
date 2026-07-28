use super::{PetFps, PetMode, PetSettings};
use std::{error::Error, ffi::c_char, fmt};

#[cfg(target_os = "macos")]
use std::{
    ffi::{c_void, CString},
    ptr::NonNull,
};

pub type PetCallback =
    extern "C" fn(kind: u32, payload: *const c_char, x: f64, y: f64, display_id: u64);

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

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRendererState {
    Unavailable = 0,
    Ready = 1,
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

#[cfg(test)]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestRenderMode {
    Real = MODE_REAL,
    Lite = MODE_LITE,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetNativeConfig {
    pub abi_version: u32,
    pub mode: u32,
    pub size: f64,
    pub fps: u32,
    pub visible: u8,
    pub pending_count: u32,
    pub reduce_motion: u8,
    pub request_permission: u8,
}

impl PetNativeConfig {
    pub fn lite(size: f64, fps: PetFps, visible: bool) -> Self {
        Self {
            abi_version: ABI_VERSION,
            mode: MODE_LITE,
            size,
            fps: fps_value(fps),
            visible: u8::from(visible),
            pending_count: 0,
            reduce_motion: 0,
            request_permission: 0,
        }
    }

    pub fn from_settings(settings: &PetSettings, request_permission: bool) -> Self {
        Self {
            abi_version: ABI_VERSION,
            mode: match settings.mode {
                PetMode::Real => MODE_REAL,
                PetMode::Lite => MODE_LITE,
            },
            size: f64::from(settings.size),
            fps: fps_value(settings.fps),
            visible: u8::from(settings.visible),
            pending_count: 0,
            reduce_motion: 0,
            request_permission: u8::from(request_permission),
        }
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

    extern "C" {
        fn pet_create(callback: PetCallback, metal_source: *const c_char) -> *mut c_void;
        fn pet_destroy(handle: *mut c_void) -> u32;
        fn pet_apply(handle: *mut c_void, config: PetNativeConfig) -> bool;
        fn pet_show(handle: *mut c_void);
        fn pet_hide(handle: *mut c_void);
        fn pet_reset(handle: *mut c_void);
        fn pet_signal(handle: *mut c_void, signal: u32);
        fn pet_capture_state(handle: *mut c_void) -> u32;
        fn pet_renderer_state(handle: *mut c_void) -> u32;
        fn pet_abi_version() -> u32;
        #[cfg(test)]
        fn pet_test_render_rgba(
            input: *const u8,
            width: u32,
            height: u32,
            mode: u32,
            output: *mut u8,
            output_capacity: u64,
            metal_source: *const c_char,
        ) -> u64;
    }

    pub fn abi_version() -> u32 {
        unsafe { pet_abi_version() }
    }

    pub struct Handle(Option<NonNull<c_void>>);

    impl Handle {
        pub fn new(callback: PetCallback) -> Option<Self> {
            let source = CString::new(include_str!("../../native/mac/shader.metal"))
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

    #[cfg(test)]
    pub fn test_render_rgba(
        input: &[u8],
        width: u32,
        height: u32,
        mode: super::TestRenderMode,
    ) -> Option<Vec<u8>> {
        let length = usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            .checked_mul(4)?;
        if mode == super::TestRenderMode::Real && input.len() != length {
            return None;
        }
        let mut output = vec![0_u8; length];
        let source = CString::new(include_str!("../../native/mac/shader.metal"))
            .expect("embedded Metal source contains no NUL bytes");
        let input_pointer = if input.is_empty() {
            std::ptr::null()
        } else {
            input.as_ptr()
        };
        let checksum = unsafe {
            pet_test_render_rgba(
                input_pointer,
                width,
                height,
                mode as u32,
                output.as_mut_ptr(),
                u64::try_from(output.len()).ok()?,
                source.as_ptr(),
            )
        };
        (checksum != 0).then_some(output)
    }
}

#[cfg(not(target_os = "macos"))]
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

#[cfg(all(test, target_os = "macos"))]
pub fn test_render_rgba(
    input: &[u8],
    width: u32,
    height: u32,
    mode: TestRenderMode,
) -> Result<Vec<u8>, NativePetError> {
    platform::test_render_rgba(input, width, height, mode).ok_or(NativePetError)
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
    use super::{NativePet, PetActivity, PetNativeConfig, TestRenderMode};
    use crate::pet::PetFps;
    use std::{
        ffi::c_char,
        mem::{align_of, offset_of, size_of},
    };

    extern "C" fn test_callback(
        _kind: u32,
        _payload: *const c_char,
        _x: f64,
        _y: f64,
        _display_id: u64,
    ) {
    }

    fn checkerboard_rgba(width: usize, height: usize) -> Vec<u8> {
        (0..height)
            .flat_map(|y| {
                (0..width).flat_map(move |x| {
                    let value = if ((x / 8) + (y / 8)) % 2 == 0 {
                        32
                    } else {
                        224
                    };
                    [value, value, value, 255]
                })
            })
            .collect()
    }

    fn checksum_region(
        pixels: &[u8],
        width: usize,
        x: usize,
        y: usize,
        region_width: usize,
        region_height: usize,
    ) -> u64 {
        (y..y + region_height)
            .flat_map(|row| {
                (x..x + region_width).flat_map(move |column| {
                    let offset = (row * width + column) * 4;
                    pixels[offset..offset + 4].iter().copied()
                })
            })
            .fold(0_u64, |sum, byte| {
                sum.wrapping_mul(16_777_619) ^ u64::from(byte)
            })
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn synthetic_checkerboard_is_distorted_inside_the_lens() {
        let input = checkerboard_rgba(128, 128);
        let output = super::test_render_rgba(&input, 128, 128, TestRenderMode::Real).unwrap();

        assert_ne!(
            checksum_region(&output, 128, 48, 48, 32, 32),
            checksum_region(&input, 128, 48, 48, 32, 32)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn synthetic_render_keeps_panel_corners_transparent() {
        let input = checkerboard_rgba(128, 128);
        let output = super::test_render_rgba(&input, 128, 128, TestRenderMode::Real).unwrap();

        assert_eq!(&output[0..4], &[0, 0, 0, 0]);
        let last = output.len() - 4;
        assert_eq!(&output[last..], &[0, 0, 0, 0]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn lite_mode_never_requires_a_capture_texture() {
        let output = super::test_render_rgba(&[], 128, 128, TestRenderMode::Lite).unwrap();

        assert!(output.chunks_exact(4).any(|pixel| pixel[3] > 0));
        assert_eq!(&output[0..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn automatic_fps_tracks_interaction_and_visibility() {
        assert_eq!(super::target_fps(PetFps::Auto, PetActivity::Idle), 30);
        assert_eq!(super::target_fps(PetFps::Auto, PetActivity::DropHover), 60);
        assert_eq!(super::target_fps(PetFps::Auto, PetActivity::Signal), 60);
        assert_eq!(super::target_fps(PetFps::Auto, PetActivity::Hidden), 0);
    }

    #[test]
    fn fixed_fps_is_respected_until_the_pet_is_hidden() {
        assert_eq!(super::target_fps(PetFps::Fps30, PetActivity::DropHover), 30);
        assert_eq!(super::target_fps(PetFps::Fps60, PetActivity::Idle), 60);
        assert_eq!(super::target_fps(PetFps::Fps60, PetActivity::Hidden), 0);
    }

    #[test]
    fn native_abi_and_raii_handle_are_stable() {
        assert_eq!(super::abi_version(), 1);
        assert_eq!(size_of::<PetNativeConfig>(), 32);
        assert_eq!(align_of::<PetNativeConfig>(), 8);
        assert_eq!(offset_of!(PetNativeConfig, abi_version), 0);
        assert_eq!(offset_of!(PetNativeConfig, mode), 4);
        assert_eq!(offset_of!(PetNativeConfig, size), 8);
        assert_eq!(offset_of!(PetNativeConfig, fps), 16);
        assert_eq!(offset_of!(PetNativeConfig, visible), 20);
        assert_eq!(offset_of!(PetNativeConfig, pending_count), 24);
        assert_eq!(offset_of!(PetNativeConfig, reduce_motion), 28);
        assert_eq!(offset_of!(PetNativeConfig, request_permission), 29);

        let pet = NativePet::new(test_callback).unwrap();
        assert!(pet.apply(PetNativeConfig::lite(220.0, PetFps::Auto, true)));
        drop(pet);
    }

    #[test]
    fn native_handle_can_be_sent_to_the_runtime_owner() {
        fn assert_send<T: Send>() {}
        assert_send::<NativePet>();
    }

    #[test]
    fn callback_kinds_decode_only_the_stable_native_events() {
        use super::PetCallbackKind;

        assert_eq!(PetCallbackKind::try_from(1), Ok(PetCallbackKind::Clicked));
        assert_eq!(PetCallbackKind::try_from(2), Ok(PetCallbackKind::Moved));
        assert_eq!(
            PetCallbackKind::try_from(3),
            Ok(PetCallbackKind::DropEntered)
        );
        assert_eq!(
            PetCallbackKind::try_from(4),
            Ok(PetCallbackKind::DropExited)
        );
        assert_eq!(
            PetCallbackKind::try_from(5),
            Ok(PetCallbackKind::FileDropped)
        );
        assert_eq!(
            PetCallbackKind::try_from(6),
            Ok(PetCallbackKind::DisplayChanged)
        );
        assert_eq!(
            PetCallbackKind::try_from(7),
            Ok(PetCallbackKind::PermissionChanged)
        );
        assert_eq!(
            PetCallbackKind::try_from(8),
            Ok(PetCallbackKind::CaptureFailed)
        );
        assert_eq!(PetCallbackKind::try_from(9), Ok(PetCallbackKind::Sleep));
        assert_eq!(PetCallbackKind::try_from(10), Ok(PetCallbackKind::Wake));
        assert!(PetCallbackKind::try_from(0).is_err());
        assert!(PetCallbackKind::try_from(11).is_err());
    }

    #[test]
    fn permission_request_bit_is_set_only_for_an_explicit_real_mode_action() {
        use crate::pet::{PetFps, PetMode, PetSettings};

        let settings = PetSettings {
            mode: PetMode::Real,
            size: 220,
            fps: PetFps::Auto,
            visible: true,
            x: None,
            y: None,
            display_id: None,
        };

        assert_eq!(
            PetNativeConfig::from_settings(&settings, false).request_permission,
            0
        );
        assert_eq!(
            PetNativeConfig::from_settings(&settings, true).request_permission,
            1
        );
    }
}
