mod command;
mod discovery;
mod runtime;

use crate::error::{AppError, Result};
use uuid::Uuid;

pub use command::{build_bambu_args, FastOverrides, PlateSelection, SliceRequest};
pub use discovery::{BambuInstallation, InstallationDiscovery};
pub use runtime::{
    SliceComplete, SliceErrorEvent, SlicePhase, SliceProgress, SliceTask, SliceTaskState,
    SlicerService,
};

#[tauri::command]
pub fn start_slice(
    request: SliceRequest,
    service: tauri::State<'_, SlicerService>,
) -> Result<SliceTask> {
    service.start(request)
}

#[tauri::command]
pub fn cancel_slice(task_id: Uuid, service: tauri::State<'_, SlicerService>) -> Result<()> {
    service.cancel(task_id)
}

#[tauri::command]
pub fn get_slice_task(
    task_id: Uuid,
    service: tauri::State<'_, SlicerService>,
) -> Result<SliceTask> {
    service.get(task_id).ok_or(AppError::InvalidJob)
}
