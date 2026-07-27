mod store;

use crate::{
    error::{AppError, Result},
    imports::PrintState,
};
use serde::{Deserialize, Serialize};

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

fn unavailable_status() -> PetStatus {
    PetStatus {
        effective_mode: PetMode::Lite,
        permission: CapturePermission::Unavailable,
        fallback_reason: Some("native_not_started".to_owned()),
    }
}

fn pet_view(settings: PetSettings) -> PetView {
    PetView {
        settings,
        status: unavailable_status(),
    }
}

#[tauri::command]
pub fn get_pet_settings(state: tauri::State<'_, PrintState>) -> Result<PetView> {
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))?;
    Ok(pet_view(PetStore::load(&service.database)?))
}

#[tauri::command]
pub fn set_pet_settings(
    patch: PetSettingsPatch,
    state: tauri::State<'_, PrintState>,
) -> Result<PetView> {
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))?;
    Ok(pet_view(PetStore::apply(&service.database, patch)?))
}
