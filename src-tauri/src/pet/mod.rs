pub mod geom;
pub mod input;
pub mod native;
mod store;

use crate::{
    error::{AppError, Result},
    imports::PrintState,
};
use serde::{Deserialize, Serialize};
use std::{ffi::c_char, sync::Mutex};

use native::{NativePet, NativePetError, PetNativeConfig};
pub use store::PetStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetMode {
    Real,
    Lite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetFps {
    Auto,
    Fps30,
    Fps60,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetSettings {
    pub mode: PetMode,
    pub size: u16,
    pub fps: PetFps,
    pub visible: bool,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub display_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePermission {
    Unavailable,
    NotDetermined,
    Denied,
    RestartRequired,
    Granted,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PetStatus {
    pub effective_mode: PetMode,
    pub permission: CapturePermission,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PetView {
    #[serde(flatten)]
    pub settings: PetSettings,
    #[serde(flatten)]
    pub status: PetStatus,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PetSettingsPatch {
    pub mode: Option<PetMode>,
    pub size: Option<u16>,
    pub fps: Option<PetFps>,
    pub visible: Option<bool>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub display_id: Option<u64>,
    pub reset_position: Option<bool>,
}

fn native_status(settings: &PetSettings) -> PetStatus {
    #[cfg(target_os = "macos")]
    let fallback_reason = match settings.mode {
        PetMode::Real => Some("native_not_started".to_owned()),
        PetMode::Lite => None,
    };
    #[cfg(not(target_os = "macos"))]
    let fallback_reason = Some("platform_unsupported".to_owned());

    PetStatus {
        effective_mode: PetMode::Lite,
        permission: CapturePermission::Unavailable,
        fallback_reason,
    }
}

fn pet_view(settings: PetSettings) -> PetView {
    let status = native_status(&settings);
    PetView { settings, status }
}

extern "C" fn native_callback(
    _kind: u32,
    _payload: *const c_char,
    _x: f64,
    _y: f64,
    _display_id: u64,
) {
}

pub struct PetNativeState {
    pet: Mutex<NativePet>,
}

impl PetNativeState {
    pub fn new(settings: &PetSettings) -> std::result::Result<Self, NativePetError> {
        let state = Self {
            pet: Mutex::new(NativePet::new(native_callback)?),
        };
        state.apply(settings);
        Ok(state)
    }

    fn with_pet<T>(&self, operation: impl FnOnce(&NativePet) -> T) -> T {
        let pet = self
            .pet
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        operation(&pet)
    }

    pub fn apply(&self, settings: &PetSettings) -> bool {
        self.with_pet(|pet| pet.apply(PetNativeConfig::from_settings(settings)))
    }

    pub fn reset(&self) {
        self.with_pet(NativePet::reset);
    }
}

#[tauri::command]
pub fn get_pet_settings(
    state: tauri::State<'_, PrintState>,
    native: tauri::State<'_, PetNativeState>,
) -> Result<PetView> {
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))?;
    let settings = PetStore::load(&service.database)?;
    drop(service);
    native.apply(&settings);
    Ok(pet_view(settings))
}

#[tauri::command]
pub fn set_pet_settings(
    patch: PetSettingsPatch,
    state: tauri::State<'_, PrintState>,
    native: tauri::State<'_, PetNativeState>,
) -> Result<PetView> {
    let reset_position = patch.reset_position == Some(true);
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))?;
    let settings = PetStore::apply(&service.database, patch)?;
    drop(service);
    if reset_position {
        native.reset();
    }
    native.apply(&settings);
    Ok(pet_view(settings))
}
