use super::{PetFps, PetMode, PetSettings, PetVisualStyle};
use std::{error::Error, ffi::c_char, fmt};

#[cfg(target_os = "macos")]
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

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeRendererState {
    Unavailable = 0,
    Ready = 1,
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

#[cfg(test)]
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestVisualStyle {
    Gargantua = 0,
    Fusion = 1,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TestRenderOptions {
    pub mode: TestRenderMode,
    pub visual_style: TestVisualStyle,
    pub center_uv: [f32; 2],
    pub time_seconds: f32,
    pub drop_origin_uv: [f32; 2],
    pub drop_progress: f32,
    pub absorption_progress: f32,
    pub success_progress: f32,
    pub error_progress: f32,
    pub pending_count: u32,
    pub reduce_motion: bool,
    pub drop_phase: u32,
    pub file_kind: u32,
    pub impact_level: f32,
    pub feed_strength: f32,
    pub capture_origin_uv: [f32; 2],
    pub capture_extent_uv: [f32; 2],
}

#[cfg(test)]
impl Default for TestRenderOptions {
    fn default() -> Self {
        Self {
            mode: TestRenderMode::Lite,
            visual_style: TestVisualStyle::Gargantua,
            center_uv: [0.5, 0.5],
            time_seconds: 0.0,
            drop_origin_uv: [0.5, 0.5],
            drop_progress: 0.0,
            absorption_progress: 0.0,
            success_progress: 0.0,
            error_progress: 0.0,
            pending_count: 0,
            reduce_motion: false,
            drop_phase: 0,
            file_kind: 0,
            impact_level: 0.0,
            feed_strength: 0.0,
            capture_origin_uv: [0.0, 0.0],
            capture_extent_uv: [1.0, 1.0],
        }
    }
}

#[cfg(test)]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct TestNativeRenderUniforms {
    viewport_px: [f32; 2],
    capture_origin_uv: [f32; 2],
    capture_extent_uv: [f32; 2],
    center_uv: [f32; 2],
    time_seconds: f32,
    hole_radius_uv: f32,
    temperature: f32,
    inclination: f32,
    roll: f32,
    disk_inner: f32,
    disk_outer: f32,
    disk_opacity: f32,
    doppler: f32,
    beaming: f32,
    gain: f32,
    contrast: f32,
    wind: f32,
    speed: f32,
    exposure: f32,
    stars: f32,
    spin: f32,
    spin_phase: f32,
    drop_origin_uv: [f32; 2],
    drop_progress: f32,
    absorption_progress: f32,
    success_progress: f32,
    error_progress: f32,
    pending_count: u32,
    mode: u32,
    reduce_motion: u32,
    drop_phase: u32,
    file_kind: u32,
    visual_style: u32,
    impact_level: f32,
    feed_strength: f32,
}

#[cfg(test)]
impl From<TestRenderOptions> for TestNativeRenderUniforms {
    fn from(options: TestRenderOptions) -> Self {
        let mut uniforms = Self {
            capture_origin_uv: options.capture_origin_uv,
            capture_extent_uv: options.capture_extent_uv,
            center_uv: options.center_uv,
            time_seconds: options.time_seconds,
            hole_radius_uv: 0.075,
            spin_phase: 0.0,
            drop_origin_uv: options.drop_origin_uv,
            drop_progress: options.drop_progress,
            absorption_progress: options.absorption_progress,
            success_progress: options.success_progress,
            error_progress: options.error_progress,
            pending_count: options.pending_count,
            mode: options.mode as u32,
            reduce_motion: u32::from(options.reduce_motion),
            drop_phase: options.drop_phase,
            file_kind: options.file_kind,
            visual_style: options.visual_style as u32,
            impact_level: options.impact_level,
            feed_strength: options.feed_strength,
            ..Self::default()
        };
        match options.visual_style {
            TestVisualStyle::Gargantua => {
                uniforms.temperature = 8500.0;
                uniforms.inclination = 1.45;
                uniforms.roll = 0.15;
                uniforms.disk_inner = 3.0;
                uniforms.disk_outer = 9.0;
                uniforms.disk_opacity = 0.65;
                uniforms.doppler = 1.0;
                uniforms.beaming = 3.0;
                uniforms.gain = 1.0;
                uniforms.contrast = 0.9;
                uniforms.wind = 5.0;
                uniforms.speed = 3.6;
                uniforms.exposure = 1.0;
                uniforms.stars = 0.0;
                uniforms.spin = 0.0;
            }
            TestVisualStyle::Fusion => {
                uniforms.temperature = 5200.0;
                uniforms.inclination = 1.535;
                uniforms.roll = 0.04;
                uniforms.disk_inner = 1.9;
                uniforms.disk_outer = 8.0;
                uniforms.disk_opacity = 0.88;
                uniforms.doppler = 0.45;
                uniforms.beaming = 2.2;
                uniforms.gain = 2.0;
                uniforms.contrast = 0.65;
                uniforms.wind = 7.0;
                uniforms.speed = 4.0;
                uniforms.exposure = 1.35;
                uniforms.stars = 0.0;
                uniforms.spin = 0.0;
            }
        }
        uniforms
    }
}

#[cfg(test)]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TestRenderStats {
    pub base_draw_calls: u32,
    pub pending_draw_calls: u32,
    pub pending_instances: u32,
    pub fragment_pending_iterations: u32,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRenderResult {
    pub pixels: Vec<u8>,
    pub stats: TestRenderStats,
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
            mode: match settings.mode {
                PetMode::Real => MODE_REAL,
                PetMode::Lite => MODE_LITE,
            },
            effective_mode: match settings.mode {
                PetMode::Real => MODE_REAL,
                PetMode::Lite => MODE_LITE,
            },
            has_position: u8::from(position.is_some()),
            size: f64::from(settings.size),
            x: position.map_or(0.0, |value| value.0),
            y: position.map_or(0.0, |value| value.1),
            display_id: position.map_or(0, |value| value.2),
            fps: fps_value(settings.fps),
            visible: u8::from(settings.visible),
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
        self.effective_mode = match mode {
            PetMode::Real => MODE_REAL,
            PetMode::Lite => MODE_LITE,
        };
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
        fn pet_finish_drop(handle: *mut c_void, generation: u64, result: u32);
        fn pet_capture_state(handle: *mut c_void) -> u32;
        fn pet_renderer_state(handle: *mut c_void) -> u32;
        fn pet_abi_version() -> u32;
        #[cfg(test)]
        fn pet_test_render_rgba(
            input: *const u8,
            width: u32,
            height: u32,
            uniforms: super::TestNativeRenderUniforms,
            output: *mut u8,
            output_capacity: u64,
            stats: *mut super::TestRenderStats,
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

    #[cfg(test)]
    pub fn test_render_with_options(
        input: &[u8],
        width: u32,
        height: u32,
        options: super::TestRenderOptions,
    ) -> Option<super::TestRenderResult> {
        let length = usize::try_from(width)
            .ok()?
            .checked_mul(usize::try_from(height).ok()?)?
            .checked_mul(4)?;
        if options.mode == super::TestRenderMode::Real && input.len() != length {
            return None;
        }
        let mut output = vec![0_u8; length];
        let mut stats = super::TestRenderStats::default();
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
                options.into(),
                output.as_mut_ptr(),
                u64::try_from(output.len()).ok()?,
                &mut stats,
                source.as_ptr(),
            )
        };
        (checksum != 0).then_some(super::TestRenderResult {
            pixels: output,
            stats,
        })
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

#[cfg(all(test, target_os = "macos"))]
pub fn test_render_rgba(
    input: &[u8],
    width: u32,
    height: u32,
    mode: TestRenderMode,
) -> Result<Vec<u8>, NativePetError> {
    test_render_with_options(
        input,
        width,
        height,
        TestRenderOptions {
            mode,
            ..TestRenderOptions::default()
        },
    )
    .map(|rendered| rendered.pixels)
}

#[cfg(all(test, target_os = "macos"))]
pub fn test_render_with_options(
    input: &[u8],
    width: u32,
    height: u32,
    options: TestRenderOptions,
) -> Result<TestRenderResult, NativePetError> {
    platform::test_render_with_options(input, width, height, options).ok_or(NativePetError)
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
    use super::{
        NativePet, PetActivity, PetNativeConfig, TestRenderMode, TestRenderOptions, TestVisualStyle,
    };
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

    #[cfg(target_os = "macos")]
    fn rgba_at(pixels: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
        let offset = (y * width + x) * 4;
        pixels[offset..offset + 4].try_into().unwrap()
    }

    #[cfg(target_os = "macos")]
    fn opaque_horizon_centroid_x(pixels: &[u8], width: usize) -> f64 {
        let mut weighted_x = 0.0;
        let mut count = 0_u64;
        for (index, pixel) in pixels.chunks_exact(4).enumerate() {
            if pixel[3] > 240 && pixel[0] < 8 && pixel[1] < 8 && pixel[2] < 8 {
                weighted_x += (index % width) as f64 + 0.5;
                count += 1;
            }
        }
        assert!(count > 0, "renderer produced no opaque horizon pixels");
        weighted_x / count as f64
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn cpu_center_uniform_moves_the_horizon_without_moving_the_capture() {
        let input = vec![255_u8; 256 * 256 * 4];
        let left = super::test_render_with_options(
            &input,
            256,
            256,
            TestRenderOptions {
                mode: TestRenderMode::Real,
                center_uv: [0.30, 0.50],
                ..Default::default()
            },
        )
        .unwrap()
        .pixels;
        let right = super::test_render_with_options(
            &input,
            256,
            256,
            TestRenderOptions {
                mode: TestRenderMode::Real,
                center_uv: [0.70, 0.50],
                ..Default::default()
            },
        )
        .unwrap()
        .pixels;
        assert!(opaque_horizon_centroid_x(&left, 256) < 110.0);
        assert!(opaque_horizon_centroid_x(&right, 256) > 146.0);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn approved_renderer_uses_the_upstream_effect_boundary() {
        let input = vec![255_u8; 256 * 256 * 4];
        let pixels = super::test_render_with_options(
            &input,
            256,
            256,
            TestRenderOptions {
                mode: TestRenderMode::Real,
                ..Default::default()
            },
        )
        .unwrap()
        .pixels;

        assert_eq!(rgba_at(&pixels, 256, 230, 128)[3], 0);
    }

    #[cfg(target_os = "macos")]
    fn warm(pixel: [u8; 4]) -> bool {
        pixel[0] > pixel[2] && pixel[0] > 80 && pixel[1] > 45 && pixel[3] > 20
    }

    #[cfg(target_os = "macos")]
    fn checker_boundary_moved(
        output: &[u8],
        inverted_output: &[u8],
        width: usize,
        height: usize,
        vertical: bool,
    ) -> bool {
        let capture_signal = |x: usize, y: usize| {
            i16::from(rgba_at(output, width, x, y)[0])
                - i16::from(rgba_at(inverted_output, width, x, y)[0])
        };
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let normalized_x = ((x as f64 + 0.5) / width as f64) * 2.0 - 1.0;
                let normalized_y = ((y as f64 + 0.5) / height as f64) * 2.0 - 1.0;
                let radius = (normalized_x * normalized_x + normalized_y * normalized_y).sqrt();
                if !(0.42..=0.84).contains(&radius) {
                    continue;
                }
                let (next_x, next_y) = if vertical { (x + 1, y) } else { (x, y + 1) };
                let input_stays_in_same_tile = if vertical {
                    x / 8 == next_x / 8
                } else {
                    y / 8 == next_y / 8
                };
                if !input_stays_in_same_tile {
                    continue;
                }
                let current = capture_signal(x, y);
                let next = capture_signal(next_x, next_y);
                if current.abs() > 12 && next.abs() > 12 && current.signum() != next.signum() {
                    return true;
                }
            }
        }
        false
    }

    fn checksum_annulus(
        pixels: &[u8],
        width: usize,
        height: usize,
        inner_radius: f64,
        outer_radius: f64,
    ) -> u64 {
        let mut checksum = 0_u64;
        for y in 0..height {
            for x in 0..width {
                let normalized_x = ((x as f64 + 0.5) / width as f64) * 2.0 - 1.0;
                let normalized_y = ((y as f64 + 0.5) / height as f64) * 2.0 - 1.0;
                let radius = (normalized_x * normalized_x + normalized_y * normalized_y).sqrt();
                if radius < inner_radius || radius > outer_radius {
                    continue;
                }
                let offset = (y * width + x) * 4;
                for byte in &pixels[offset..offset + 4] {
                    checksum = checksum.wrapping_mul(16_777_619) ^ u64::from(*byte);
                }
            }
        }
        checksum
    }

    #[cfg(target_os = "macos")]
    fn render_style(
        input: &[u8],
        width: u32,
        height: u32,
        mode: TestRenderMode,
        visual_style: TestVisualStyle,
    ) -> Vec<u8> {
        super::test_render_with_options(
            input,
            width,
            height,
            TestRenderOptions {
                mode,
                visual_style,
                ..TestRenderOptions::default()
            },
        )
        .unwrap()
        .pixels
    }

    #[cfg(target_os = "macos")]
    fn different_pixel_count(lhs: &[u8], rhs: &[u8]) -> usize {
        lhs.chunks_exact(4)
            .zip(rhs.chunks_exact(4))
            .filter(|(left, right)| *left != *right)
            .count()
    }

    #[cfg(target_os = "macos")]
    fn render_drop_cell(
        input: &[u8],
        width: u32,
        height: u32,
        options: TestRenderOptions,
    ) -> Vec<u8> {
        super::test_render_with_options(input, width, height, options)
            .unwrap()
            .pixels
    }

    #[cfg(target_os = "macos")]
    fn opaque_black_horizon_area(pixels: &[u8], width: usize, height: usize) -> usize {
        let mut area = 0;
        for y in 0..height {
            for x in 0..width {
                let normalized_x = ((x as f64 + 0.5) / width as f64) * 2.0 - 1.0;
                let normalized_y = ((y as f64 + 0.5) / height as f64) * 2.0 - 1.0;
                // hole_radius_uv is 0.075 in panel UV, or 0.15 in these
                // full-panel coordinates. Exclude black capture pixels beyond it.
                if normalized_x.hypot(normalized_y) > 0.15 {
                    continue;
                }
                let pixel = rgba_at(pixels, width, x, y);
                area += usize::from(pixel[3] > 240 && pixel[0] < 8 && pixel[1] < 8 && pixel[2] < 8);
            }
        }
        area
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn switching_styles_preserves_gargantua_and_the_shared_event_horizon() {
        let input = checkerboard_rgba(256, 256);
        let gargantua_before = render_style(
            &input,
            256,
            256,
            TestRenderMode::Real,
            TestVisualStyle::Gargantua,
        );
        let fusion = render_style(
            &input,
            256,
            256,
            TestRenderMode::Real,
            TestVisualStyle::Fusion,
        );
        let gargantua_after = render_style(
            &input,
            256,
            256,
            TestRenderMode::Real,
            TestVisualStyle::Gargantua,
        );

        assert_eq!(gargantua_before, gargantua_after);
        assert_eq!(&rgba_at(&gargantua_before, 256, 128, 128)[..3], &[0, 0, 0]);
        assert_eq!(&rgba_at(&fusion, 256, 128, 128)[..3], &[0, 0, 0]);
        let gargantua_horizon = opaque_black_horizon_area(&gargantua_before, 256, 256);
        let fusion_horizon = opaque_black_horizon_area(&fusion, 256, 256);
        let area_delta = gargantua_horizon.abs_diff(fusion_horizon) as f64
            / gargantua_horizon.max(fusion_horizon) as f64;
        assert!(
            area_delta <= 0.01,
            "event-horizon area changed by {area_delta:.3}: \
             Gargantua={gargantua_horizon}, Fusion={fusion_horizon}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn both_styles_keep_checkerboard_capture_active_in_the_annulus() {
        let input = checkerboard_rgba(128, 128);
        let inverted = input
            .chunks_exact(4)
            .flat_map(|pixel| [255 - pixel[0], 255 - pixel[1], 255 - pixel[2], 255])
            .collect::<Vec<_>>();

        for visual_style in [TestVisualStyle::Gargantua, TestVisualStyle::Fusion] {
            let output = render_style(&input, 128, 128, TestRenderMode::Real, visual_style);
            let inverted_output =
                render_style(&inverted, 128, 128, TestRenderMode::Real, visual_style);
            assert_ne!(
                checksum_annulus(&output, 128, 128, 0.20, 0.96),
                checksum_annulus(&inverted_output, 128, 128, 0.20, 0.96),
                "{visual_style:?} stopped responding to capture input"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn approved_renderer_animates_inside_its_transparent_boundary() {
        let input = checkerboard_rgba(256, 256);
        let render_at = |time_seconds| {
            super::test_render_with_options(
                &input,
                256,
                256,
                TestRenderOptions {
                    mode: TestRenderMode::Real,
                    time_seconds,
                    ..Default::default()
                },
            )
            .unwrap()
            .pixels
        };
        let first = render_at(0.0);
        let later = render_at(1.5);

        assert!(different_pixel_count(&first, &later) > 400);
        assert_preview_corners_are_transparent(&first, 256, "approved renderer");
        assert_preview_corners_are_transparent(&later, 256, "approved renderer later");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn approved_renderer_keeps_a_soft_premultiplied_feather() {
        let input = [180_u8, 200, 220, 255]
            .into_iter()
            .cycle()
            .take(256 * 256 * 4)
            .collect::<Vec<_>>();
        let output = super::test_render_with_options(
            &input,
            256,
            256,
            TestRenderOptions {
                mode: TestRenderMode::Real,
                ..Default::default()
            },
        )
        .unwrap()
        .pixels;

        let mut soft_feather_pixels = 0;
        for pixel in output.chunks_exact(4) {
            let alpha = pixel[3];
            if (8..=160).contains(&alpha) {
                soft_feather_pixels += 1;
            }
            if alpha > 0 && alpha < 255 {
                assert!(
                    pixel[..3]
                        .iter()
                        .all(|&channel| channel <= alpha.saturating_add(2)),
                    "renderer emitted non-premultiplied RGBA {pixel:?}"
                );
            }
        }

        assert!(
            soft_feather_pixels > 100,
            "mask edge became an opaque lens: only {soft_feather_pixels} soft-feather pixels"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn synthetic_capture_drives_distortion_in_the_annulus_outside_the_horizon() {
        let input = checkerboard_rgba(128, 128);
        let inverted = input
            .chunks_exact(4)
            .flat_map(|pixel| [255 - pixel[0], 255 - pixel[1], 255 - pixel[2], 255])
            .collect::<Vec<_>>();
        let output = super::test_render_rgba(&input, 128, 128, TestRenderMode::Real).unwrap();
        let inverted_output =
            super::test_render_rgba(&inverted, 128, 128, TestRenderMode::Real).unwrap();

        assert_ne!(
            checksum_annulus(&output, 128, 128, 0.42, 0.84),
            checksum_annulus(&input, 128, 128, 0.42, 0.84)
        );
        assert_ne!(
            checksum_annulus(&output, 128, 128, 0.42, 0.84),
            checksum_annulus(&inverted_output, 128, 128, 0.42, 0.84)
        );
        assert!(
            checker_boundary_moved(&output, &inverted_output, 128, 128, true),
            "no vertical checkerboard boundary moved"
        );
        assert!(
            checker_boundary_moved(&output, &inverted_output, 128, 128, false),
            "no horizontal checkerboard boundary moved"
        );
        for (x, y) in [(0, 0), (127, 0), (0, 127), (127, 127)] {
            assert_eq!(rgba_at(&output, 128, x, y), [0, 0, 0, 0]);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn gargantua_has_a_black_shadow_warm_disk_and_two_lensed_arcs() {
        let input = checkerboard_rgba(256, 256);
        let output = super::test_render_with_options(
            &input,
            256,
            256,
            TestRenderOptions {
                mode: TestRenderMode::Real,
                ..Default::default()
            },
        )
        .unwrap()
        .pixels;
        let center = rgba_at(&output, 256, 128, 128);
        assert!(center[0] < 16 && center[1] < 16 && center[2] < 16);
        assert!(center[3] > 240);
        let outside_approved_shadow = rgba_at(&output, 256, 160, 128);
        assert!(
            outside_approved_shadow[..3]
                .iter()
                .any(|&channel| channel > 16),
            "legacy oversized radial shadow remains"
        );
        let upper = (70..124)
            .flat_map(|y| (72..184).map(move |x| (x, y)))
            .filter(|&(x, y)| warm(rgba_at(&output, 256, x, y)))
            .count();
        let lower = (132..186)
            .flat_map(|y| (72..184).map(move |x| (x, y)))
            .filter(|&(x, y)| warm(rgba_at(&output, 256, x, y)))
            .count();
        assert!(upper > 24, "missing upper lensed disk arc");
        assert!(lower > 24, "missing lower lensed disk arc");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn gargantua_has_no_legacy_spectral_ring_function() {
        let source = include_str!("../../native/mac/shader.metal");
        assert!(!source.contains("spectral_ring"));
        assert!(source.contains("kGeodesicSteps = 48"));
        assert!(source.contains("shade_crossing"));
        assert!(source.contains("weak_deflection_background"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn synthetic_render_keeps_panel_corners_transparent() {
        let input = checkerboard_rgba(128, 128);
        let output = super::test_render_rgba(&input, 128, 128, TestRenderMode::Real).unwrap();

        assert_eq!(&output[0..4], &[0, 0, 0, 0]);
        let last = output.len() - 4;
        assert_eq!(&output[last..], &[0, 0, 0, 0]);
        let outside_lens = (64 * 4)..(65 * 4);
        assert_eq!(&output[outside_lens], &[0, 0, 0, 0]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn lite_mode_never_requires_a_capture_texture() {
        let output = super::test_render_rgba(&[], 128, 128, TestRenderMode::Lite).unwrap();

        assert!(output.chunks_exact(4).any(|pixel| pixel[3] > 0));
        assert_eq!(&output[0..4], &[0, 0, 0, 0]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn pending_points_use_one_instanced_pass_without_fragment_count_loops() {
        let source = include_str!("../../native/mac/shader.metal");
        let base_fragment = source
            .split_once("fragment float4 pet_fragment")
            .and_then(|(_, after_start)| after_start.split_once("vertex PetPendingVertexOutput"))
            .map(|(fragment, _)| fragment)
            .expect("base and pending shader stages remain present");
        assert!(!base_fragment.contains("pending_count"));

        for pending_count in [37, 4_096] {
            let rendered = super::test_render_with_options(
                &[],
                128,
                128,
                TestRenderOptions {
                    mode: TestRenderMode::Lite,
                    pending_count,
                    ..TestRenderOptions::default()
                },
            )
            .unwrap();

            assert_eq!(rendered.stats.base_draw_calls, 1);
            assert_eq!(rendered.stats.pending_draw_calls, 1);
            assert_eq!(rendered.stats.pending_instances, pending_count);
            assert_eq!(rendered.stats.fragment_pending_iterations, 0);
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reduced_motion_freezes_continuous_phase_and_pending_orbits() {
        let render_at = |time_seconds| {
            super::test_render_with_options(
                &[],
                128,
                128,
                TestRenderOptions {
                    mode: TestRenderMode::Lite,
                    time_seconds,
                    pending_count: 37,
                    reduce_motion: true,
                    ..TestRenderOptions::default()
                },
            )
            .unwrap()
            .pixels
        };

        assert_eq!(render_at(0.0), render_at(73.0));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reduced_motion_success_and_error_keep_fixed_geometry() {
        for signal in ["success", "error"] {
            let render_at = |progress| {
                let (success_progress, error_progress) = match signal {
                    "success" => (progress, 0.0),
                    "error" => (0.0, progress),
                    _ => unreachable!(),
                };
                super::test_render_with_options(
                    &[],
                    128,
                    128,
                    TestRenderOptions {
                        mode: TestRenderMode::Lite,
                        success_progress,
                        error_progress,
                        reduce_motion: true,
                        ..TestRenderOptions::default()
                    },
                )
                .unwrap()
                .pixels
            };

            assert_eq!(render_at(0.25), render_at(0.75), "{signal}");
        }
    }

    #[cfg(target_os = "macos")]
    fn preview_side(size: u32) -> u32 {
        size.min(360)
    }

    #[cfg(target_os = "macos")]
    fn preview_input(mode: TestRenderMode, side: u32) -> Vec<u8> {
        match mode {
            TestRenderMode::Real => checkerboard_rgba(side as usize, side as usize),
            TestRenderMode::Lite => Vec::new(),
        }
    }

    #[cfg(target_os = "macos")]
    fn preview_options(visual_style: TestVisualStyle, mode: TestRenderMode) -> TestRenderOptions {
        TestRenderOptions {
            mode,
            visual_style,
            ..TestRenderOptions::default()
        }
    }

    #[cfg(target_os = "macos")]
    fn assert_preview_corners_are_transparent(pixels: &[u8], side: u32, label: &str) {
        let last = side as usize - 1;
        for (x, y) in [(0, 0), (last, 0), (0, last), (last, last)] {
            assert_eq!(
                rgba_at(pixels, side as usize, x, y)[3],
                0,
                "{label} clipped visible content into corner ({x}, {y})"
            );
        }
    }

    #[cfg(target_os = "macos")]
    fn verify_idle_distortion_or_transparency(
        visual_style: TestVisualStyle,
        mode: TestRenderMode,
        size: u32,
    ) {
        let side = preview_side(size);
        let input = checkerboard_rgba(side as usize, side as usize);
        let inverted = input
            .chunks_exact(4)
            .flat_map(|pixel| [255 - pixel[0], 255 - pixel[1], 255 - pixel[2], 255])
            .collect::<Vec<_>>();
        let options = preview_options(visual_style, mode);
        let output = render_drop_cell(&input, side, side, options);
        let inverted_output = render_drop_cell(&inverted, side, side, options);
        let label = format!("{visual_style:?}/{mode:?}/{size}/idle");

        match mode {
            TestRenderMode::Real => assert_ne!(
                checksum_annulus(&output, side as usize, side as usize, 0.20, 0.96,),
                checksum_annulus(&inverted_output, side as usize, side as usize, 0.20, 0.96,),
                "{label} stopped responding to inverted checkerboard capture"
            ),
            TestRenderMode::Lite => assert_eq!(
                different_pixel_count(&output, &inverted_output),
                0,
                "{label} unexpectedly depended on capture input"
            ),
        }
        assert_preview_corners_are_transparent(&output, side, &label);
    }

    #[cfg(target_os = "macos")]
    fn verify_standard_success(
        visual_style: TestVisualStyle,
        mode: TestRenderMode,
        size: u32,
        faller_seconds: f32,
        jet_seconds: f32,
    ) {
        let side = preview_side(size);
        let input = preview_input(mode, side);
        let base = preview_options(visual_style, mode);
        let baseline = render_drop_cell(&input, side, side, base);
        let render_at = |elapsed: f32, absorption_progress: f32| {
            render_drop_cell(
                &input,
                side,
                side,
                TestRenderOptions {
                    drop_origin_uv: [0.72, 0.44],
                    drop_progress: (elapsed / faller_seconds).clamp(0.0, 1.0),
                    absorption_progress,
                    drop_phase: 3,
                    file_kind: 1,
                    ..base
                },
            )
        };
        let label = format!("{visual_style:?}/{mode:?}/{size}/success");

        let phase_samples = [
            render_at(0.0, 0.0),
            render_at(1.15, 0.0),
            render_at(2.07, 0.0),
            render_at(3.312, 0.0),
            render_at(3.772, 0.0),
            render_at(4.048, 0.0),
        ];
        for (index, pair) in phase_samples.windows(2).enumerate() {
            assert!(
                different_pixel_count(&pair[0], &pair[1]) > 24,
                "{label} did not visibly cross phase boundary {index}"
            );
        }
        assert_eq!(
            different_pixel_count(&render_at(faller_seconds, 0.0), &baseline),
            0,
            "{label} retained the delivered card after the 4.6-second faller"
        );

        let jet_mid_elapsed = 3.772 + jet_seconds * 0.5;
        assert!(
            different_pixel_count(
                &render_at(jet_mid_elapsed, 0.0),
                &render_at(jet_mid_elapsed, 0.5),
            ) > 24,
            "{label} did not draw the 0.90-second jet midpoint"
        );
    }

    #[cfg(target_os = "macos")]
    fn verify_rejection_has_no_delivery_or_jet(
        visual_style: TestVisualStyle,
        mode: TestRenderMode,
        size: u32,
        recoil_seconds: f32,
    ) {
        let side = preview_side(size);
        let input = preview_input(mode, side);
        let base = preview_options(visual_style, mode);
        let baseline = render_drop_cell(&input, side, side, base);
        let rejected = TestRenderOptions {
            drop_origin_uv: [0.72, 0.44],
            drop_progress: 0.60,
            absorption_progress: 0.5,
            error_progress: 0.21 / recoil_seconds,
            drop_phase: 4,
            file_kind: 2,
            ..base
        };
        let rendered = render_drop_cell(&input, side, side, rejected);
        let label = format!("{visual_style:?}/{mode:?}/{size}/reject");

        assert!(
            different_pixel_count(&baseline, &rendered) > 24,
            "{label} did not draw the 0.42-second recoil"
        );
        assert_eq!(
            different_pixel_count(
                &rendered,
                &render_drop_cell(
                    &input,
                    side,
                    side,
                    TestRenderOptions {
                        drop_progress: 0.0,
                        absorption_progress: 0.0,
                        ..rejected
                    },
                ),
            ),
            0,
            "{label} rendered delivery fragments or a stale jet"
        );
        assert_eq!(
            different_pixel_count(
                &render_drop_cell(
                    &input,
                    side,
                    side,
                    TestRenderOptions {
                        error_progress: 0.42 / recoil_seconds,
                        absorption_progress: 0.0,
                        ..rejected
                    },
                ),
                &baseline,
            ),
            0,
            "{label} remained visible after the 0.42-second recoil"
        );
    }

    #[cfg(target_os = "macos")]
    fn verify_cancellation_is_idle_and_empty(
        visual_style: TestVisualStyle,
        mode: TestRenderMode,
        size: u32,
    ) {
        let side = preview_side(size);
        let input = preview_input(mode, side);
        let base = preview_options(visual_style, mode);
        let baseline = render_drop_cell(&input, side, side, base);
        let cancelled = render_drop_cell(
            &input,
            side,
            side,
            TestRenderOptions {
                drop_progress: 0.72,
                absorption_progress: 0.5,
                drop_phase: 0,
                file_kind: 0,
                ..base
            },
        );

        assert_eq!(
            different_pixel_count(&cancelled, &baseline),
            0,
            "{visual_style:?}/{mode:?}/{size}/cancel retained a card, fragment, impact, or jet"
        );
    }

    #[cfg(target_os = "macos")]
    fn verify_reduced_motion_has_no_fragments_or_jet(
        visual_style: TestVisualStyle,
        mode: TestRenderMode,
        size: u32,
        reduced_seconds: f32,
    ) {
        let side = preview_side(size);
        let input = preview_input(mode, side);
        let base = preview_options(visual_style, mode);
        let baseline = render_drop_cell(&input, side, side, base);
        let midpoint = TestRenderOptions {
            drop_origin_uv: [0.72, 0.44],
            drop_progress: 0.075 / reduced_seconds,
            absorption_progress: 0.5,
            reduce_motion: true,
            drop_phase: 3,
            file_kind: 1,
            ..base
        };
        let rendered = render_drop_cell(&input, side, side, midpoint);
        let label = format!("{visual_style:?}/{mode:?}/{size}/reduced");

        assert!(
            different_pixel_count(&baseline, &rendered) > 24,
            "{label} did not draw the 0.15-second fade"
        );
        assert_eq!(
            different_pixel_count(
                &rendered,
                &render_drop_cell(
                    &input,
                    side,
                    side,
                    TestRenderOptions {
                        absorption_progress: 0.0,
                        ..midpoint
                    },
                ),
            ),
            0,
            "{label} drew a jet during Reduce Motion"
        );
        assert_eq!(
            different_pixel_count(
                &render_drop_cell(
                    &input,
                    side,
                    side,
                    TestRenderOptions {
                        drop_progress: 0.15 / reduced_seconds,
                        absorption_progress: 0.0,
                        ..midpoint
                    },
                ),
                &baseline,
            ),
            0,
            "{label} retained the card or fragments after 0.15 seconds"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn complete_synthetic_preview_matrix_covers_every_style_mode_size_and_state() {
        for visual_style in [TestVisualStyle::Gargantua, TestVisualStyle::Fusion] {
            for mode in [TestRenderMode::Real, TestRenderMode::Lite] {
                for size in [300_u32, 600, 900] {
                    verify_idle_distortion_or_transparency(visual_style, mode, size);
                    verify_standard_success(visual_style, mode, size, 4.6, 0.90);
                    verify_rejection_has_no_delivery_or_jet(visual_style, mode, size, 0.42);
                    verify_cancellation_is_idle_and_empty(visual_style, mode, size);
                    verify_reduced_motion_has_no_fragments_or_jet(visual_style, mode, size, 0.15);
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn procedural_file_cards_distinguish_3mf_from_gcode_without_textures() {
        let render_kind = |file_kind| {
            render_drop_cell(
                &[],
                128,
                128,
                TestRenderOptions {
                    mode: TestRenderMode::Lite,
                    drop_origin_uv: [0.72, 0.44],
                    drop_phase: 2,
                    file_kind,
                    ..TestRenderOptions::default()
                },
            )
        };
        let three_mf = render_kind(1);
        let gcode = render_kind(2);

        assert!(
            different_pixel_count(&three_mf, &gcode) > 24,
            "procedural 3MF and GCODE cards were indistinguishable"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn faller_starts_at_the_acknowledged_origin_without_a_phase_jump() {
        let options = TestRenderOptions {
            mode: TestRenderMode::Lite,
            drop_origin_uv: [0.72, 0.44],
            file_kind: 1,
            drop_phase: 2,
            ..TestRenderOptions::default()
        };
        let pending = render_drop_cell(&[], 128, 128, options);
        let swallowing = render_drop_cell(
            &[],
            128,
            128,
            TestRenderOptions {
                drop_phase: 3,
                drop_progress: 0.0,
                ..options
            },
        );

        assert_eq!(
            pending, swallowing,
            "the accepted card moved or rotated before faller time advanced"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn impact_and_afterglow_remain_visible_after_the_card_is_gone() {
        let baseline = render_drop_cell(&[], 128, 128, TestRenderOptions::default());
        let impact = render_drop_cell(
            &[],
            128,
            128,
            TestRenderOptions {
                drop_origin_uv: [0.72, 0.44],
                file_kind: 1,
                impact_level: 0.72,
                ..TestRenderOptions::default()
            },
        );
        let afterglow = render_drop_cell(
            &[],
            128,
            128,
            TestRenderOptions {
                drop_origin_uv: [0.72, 0.44],
                file_kind: 1,
                feed_strength: 0.08,
                ..TestRenderOptions::default()
            },
        );

        assert!(
            different_pixel_count(&baseline, &impact) > 24,
            "the disk did not react at the impact"
        );
        assert!(
            different_pixel_count(&baseline, &afterglow) > 24,
            "the fed-disk afterglow disappeared before its 14-second lifetime"
        );
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
        assert_eq!(size_of::<PetNativeConfig>(), 64);
        assert_eq!(align_of::<PetNativeConfig>(), 8);
        assert_eq!(offset_of!(PetNativeConfig, abi_version), 0);
        assert_eq!(offset_of!(PetNativeConfig, mode), 4);
        assert_eq!(offset_of!(PetNativeConfig, effective_mode), 8);
        assert_eq!(offset_of!(PetNativeConfig, size), 16);
        assert_eq!(offset_of!(PetNativeConfig, has_position), 12);
        assert_eq!(offset_of!(PetNativeConfig, x), 24);
        assert_eq!(offset_of!(PetNativeConfig, y), 32);
        assert_eq!(offset_of!(PetNativeConfig, display_id), 40);
        assert_eq!(offset_of!(PetNativeConfig, fps), 48);
        assert_eq!(offset_of!(PetNativeConfig, visible), 52);
        assert_eq!(offset_of!(PetNativeConfig, pending_count), 56);
        assert_eq!(offset_of!(PetNativeConfig, reduce_motion), 60);
        assert_eq!(offset_of!(PetNativeConfig, request_permission), 61);
        assert_eq!(offset_of!(PetNativeConfig, visual_style), 62);
        assert_eq!(size_of::<super::TestNativeRenderUniforms>(), 160);
        assert_eq!(
            offset_of!(super::TestNativeRenderUniforms, capture_origin_uv),
            8
        );
        assert_eq!(offset_of!(super::TestNativeRenderUniforms, center_uv), 24);
        assert_eq!(offset_of!(super::TestNativeRenderUniforms, temperature), 40);
        assert_eq!(
            offset_of!(super::TestNativeRenderUniforms, drop_origin_uv),
            104
        );
        assert_eq!(
            offset_of!(super::TestNativeRenderUniforms, pending_count),
            128
        );
        assert_eq!(
            offset_of!(super::TestNativeRenderUniforms, visual_style),
            148
        );
        assert_eq!(
            offset_of!(super::TestNativeRenderUniforms, impact_level),
            152
        );
        assert_eq!(
            offset_of!(super::TestNativeRenderUniforms, feed_strength),
            156
        );

        let pet = NativePet::new(test_callback).unwrap();
        assert!(pet.apply(PetNativeConfig::lite(220.0, PetFps::Auto, true)));
        drop(pet);
    }

    #[test]
    fn approved_default_uses_the_pinned_tiyda_material() {
        let uniforms = super::TestNativeRenderUniforms::from(TestRenderOptions::default());

        assert_eq!(uniforms.visual_style, 0);
        assert_eq!(
            [
                uniforms.temperature,
                uniforms.inclination,
                uniforms.roll,
                uniforms.disk_inner,
                uniforms.disk_outer,
                uniforms.disk_opacity,
                uniforms.doppler,
                uniforms.beaming,
                uniforms.gain,
                uniforms.contrast,
                uniforms.wind,
                uniforms.speed,
                uniforms.exposure,
                uniforms.stars,
                uniforms.spin,
            ],
            [8500.0, 1.45, 0.15, 3.0, 9.0, 0.65, 1.0, 3.0, 1.0, 0.9, 5.0, 3.6, 1.0, 0.0, 0.0,]
        );
    }

    #[test]
    fn fusion_resolves_the_exact_cpu_material() {
        let uniforms = super::TestNativeRenderUniforms::from(TestRenderOptions {
            visual_style: TestVisualStyle::Fusion,
            ..TestRenderOptions::default()
        });

        assert_eq!(uniforms.visual_style, 1);
        assert_eq!(
            [
                uniforms.temperature,
                uniforms.inclination,
                uniforms.roll,
                uniforms.disk_inner,
                uniforms.disk_outer,
                uniforms.disk_opacity,
                uniforms.doppler,
                uniforms.beaming,
                uniforms.gain,
                uniforms.contrast,
                uniforms.wind,
                uniforms.speed,
                uniforms.exposure,
                uniforms.stars,
                uniforms.spin,
            ],
            [
                5200.0, 1.535, 0.04, 1.9, 8.0, 0.88, 0.45, 2.2, 2.0, 0.65, 7.0, 4.0, 1.35, 0.0,
                0.0,
            ]
        );
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
        use crate::pet::{PetFps, PetMode, PetSettings, PetVisualStyle};

        let settings = PetSettings {
            mode: PetMode::Real,
            visual_style: PetVisualStyle::Gargantua,
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

    #[test]
    fn requested_real_mode_can_render_the_lite_fallback_without_losing_intent() {
        use crate::pet::{PetFps, PetMode, PetSettings, PetVisualStyle};

        let settings = PetSettings {
            mode: PetMode::Real,
            visual_style: PetVisualStyle::Fusion,
            size: 220,
            fps: PetFps::Fps60,
            visible: true,
            x: None,
            y: None,
            display_id: None,
        };
        let mut config = PetNativeConfig::from_settings(&settings, false);

        config.set_effective_mode(PetMode::Lite);

        assert_eq!(config.mode, super::MODE_REAL);
        assert_eq!(config.effective_mode, super::MODE_LITE);
        assert_eq!(config.fps, 60);
        assert_eq!(config.visual_style, 1);
    }

    #[test]
    fn persisted_position_is_carried_atomically_in_one_native_config() {
        use crate::pet::{PetFps, PetMode, PetSettings, PetVisualStyle};

        let settings = PetSettings {
            mode: PetMode::Lite,
            visual_style: PetVisualStyle::Gargantua,
            size: 220,
            fps: PetFps::Auto,
            visible: true,
            x: Some(-940.5),
            y: Some(88.25),
            display_id: Some(42),
        };

        let config = PetNativeConfig::from_settings(&settings, false);

        assert_eq!(config.has_position, 1);
        assert_eq!((config.x, config.y, config.display_id), (-940.5, 88.25, 42));
    }
}
