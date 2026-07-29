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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PetVisualStyle {
    Gargantua,
    Fusion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetSettings {
    pub mode: PetMode,
    pub visual_style: PetVisualStyle,
    pub size: u16,
    pub fps: PetFps,
    pub visible: bool,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub display_id: Option<u64>,
}

impl PetSettings {
    pub fn enabled(&self) -> bool {
        self.mode == PetMode::Real
    }

    pub fn effective_visibility(&self) -> bool {
        self.enabled() && self.visible
    }
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
    pub visual_style: Option<PetVisualStyle>,
    pub size: Option<u16>,
    pub fps: Option<PetFps>,
    pub visible: Option<bool>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub display_id: Option<u64>,
    pub reset_position: Option<bool>,
}

fn pet_view(settings: PetSettings, status: PetStatus) -> PetView {
    PetView { settings, status }
}

fn should_request_permission(patch: &PetSettingsPatch) -> bool {
    patch.mode == Some(PetMode::Real)
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
    Ok(pet_view(settings, runtime.status()))
}

#[tauri::command]
pub fn set_pet_settings(
    patch: PetSettingsPatch,
    state: tauri::State<'_, PrintState>,
    runtime: tauri::State<'_, PetRuntime>,
) -> Result<PetView> {
    let request_permission = should_request_permission(&patch);
    let reset_position = patch.reset_position == Some(true);
    let replace_position =
        reset_position || patch.x.is_some() || patch.y.is_some() || patch.display_id.is_some();
    let service = state
        .lock()
        .map_err(|_| AppError::Database("print lock poisoned".to_owned()))?;
    let settings = PetStore::apply(&service.database, patch)?;
    drop(service);
    if reset_position {
        runtime.reset();
    }
    if replace_position && !reset_position {
        runtime.apply_replacing_position(settings.clone(), request_permission);
    } else {
        runtime.apply_with_permission_request(settings.clone(), request_permission);
    }
    Ok(pet_view(settings, runtime.status()))
}

#[cfg(test)]
mod tests {
    use super::{
        native::PetNativeConfig, should_request_permission, PetFps, PetMode, PetSettings,
        PetSettingsPatch, PetVisualStyle,
    };

    #[test]
    fn lite_requested_mode_is_never_effectively_visible() {
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

        assert!(!settings.enabled());
        assert!(!settings.effective_visibility());
        assert_eq!(PetNativeConfig::from_settings(&settings, false).visible, 0);
    }

    #[test]
    fn real_requested_mode_preserves_the_users_hidden_choice() {
        let settings = PetSettings {
            mode: PetMode::Real,
            visual_style: PetVisualStyle::Gargantua,
            size: 220,
            fps: PetFps::Auto,
            visible: false,
            x: None,
            y: None,
            display_id: None,
        };

        assert!(settings.enabled());
        assert!(!settings.effective_visibility());
    }

    #[test]
    fn only_an_explicit_turn_on_patch_requests_permission() {
        assert!(should_request_permission(&PetSettingsPatch {
            mode: Some(PetMode::Real),
            visible: Some(true),
            ..Default::default()
        }));
        assert!(!should_request_permission(&PetSettingsPatch {
            mode: Some(PetMode::Lite),
            visible: Some(false),
            ..Default::default()
        }));
        assert!(!should_request_permission(&PetSettingsPatch {
            visible: Some(true),
            ..Default::default()
        }));
    }
}
