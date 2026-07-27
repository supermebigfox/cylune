pub mod geom;
pub mod input;
pub mod native;
pub mod runtime;
mod store;

use crate::{
    error::{AppError, Result},
    imports::PrintState,
};
use serde::{Deserialize, Serialize};

use runtime::PetRuntime;
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

#[tauri::command]
pub fn get_pet_settings(
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, PetRuntime>,
) -> Result<PetView> {
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))?;
    let settings = PetStore::load(&service.database)?;
    drop(service);
    runtime.apply(settings.clone());
    Ok(pet_view(settings))
}

#[tauri::command]
pub fn set_pet_settings(
    patch: PetSettingsPatch,
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, PetRuntime>,
) -> Result<PetView> {
    let reset_position = patch.reset_position == Some(true);
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))?;
    let settings = PetStore::apply(&service.database, patch)?;
    drop(service);
    if reset_position {
        runtime.reset();
    }
    runtime.apply(settings.clone());
    Ok(pet_view(settings))
}
