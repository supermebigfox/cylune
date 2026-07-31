mod catalog;
mod command;
mod discovery;
mod inspect;
mod runtime;

use crate::{
    error::{AppError, Result},
    printers::{PrinterState, SavedPrinter},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub use catalog::{SliceFilamentPreset, SlicePresetCatalog, SlicePresetOption, SliceProcessPreset};
pub use command::{build_bambu_args, PlateSelection, SliceRequest};
pub use discovery::{BambuInstallation, InstallationDiscovery};
pub use inspect::{
    inspect_3mf_content, EmbeddedMachine, EmbeddedPlate, EmbeddedProcess, EmbeddedTool,
    ThreeMfInspection, ThreeMfKind,
};
pub use runtime::{
    SliceComplete, SliceErrorEvent, SlicePhase, SliceProgress, SliceTask, SliceTaskState,
    SlicerService,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FastSliceRequest {
    pub input_path: PathBuf,
    pub printer_id: String,
    #[serde(default)]
    pub confirm_printer_mismatch: bool,
}

fn resolve_fast_request(
    profiles_root: &Path,
    printer: SavedPrinter,
    request: FastSliceRequest,
) -> Result<SliceRequest> {
    let inspection = inspect_3mf_content(&request.input_path)?;
    let input_path = fs::canonicalize(&request.input_path).map_err(|_| AppError::InvalidFile)?;
    if inspection.kind != ThreeMfKind::Unsliced {
        return Err(AppError::SlicerIncompatible);
    }
    let model_mismatch = inspection
        .embedded_model_key
        .as_deref()
        .is_some_and(|model| model != printer.model_key);
    let nozzle_mismatch = inspection
        .embedded_nozzle_diameter
        .is_some_and(|diameter| (diameter - printer.nozzle_diameter).abs() > 0.001);
    if (model_mismatch || nozzle_mismatch) && !request.confirm_printer_mismatch {
        return Err(AppError::SlicerIncompatible);
    }

    let machine_settings = catalog::resolve_machine_path(profiles_root, &printer)?;

    Ok(SliceRequest {
        printer,
        input: input_path,
        plate_selection: PlateSelection::All,
        estimate_mode: model_mismatch || nozzle_mismatch,
        machine_settings,
    })
}

#[tauri::command]
pub fn start_slice(
    request: FastSliceRequest,
    service: tauri::State<'_, SlicerService>,
    printers: tauri::State<'_, PrinterState>,
) -> Result<SliceTask> {
    let printer = find_saved_printer(&request.printer_id, &printers)?;
    service.start_fast(request, printer)
}

fn find_saved_printer(printer_id: &str, printers: &PrinterState) -> Result<SavedPrinter> {
    let parsed = Uuid::parse_str(printer_id).map_err(|_| AppError::InvalidFile)?;
    printers
        .lock()
        .map_err(|_| AppError::Database("printer lock poisoned".into()))?
        .list_saved(&[])?
        .into_iter()
        .find(|printer| printer.printer_id == parsed.to_string())
        .ok_or(AppError::InvalidFile)
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

#[tauri::command]
pub fn inspect_3mf(path: String) -> Result<ThreeMfInspection> {
    inspect_3mf_content(Path::new(&path))
}

#[tauri::command]
pub fn list_slice_presets(
    printer_id: String,
    service: tauri::State<'_, SlicerService>,
    printers: tauri::State<'_, PrinterState>,
) -> Result<SlicePresetCatalog> {
    let printer = find_saved_printer(&printer_id, &printers)?;
    service.list_presets(&printer)
}

#[tauri::command]
pub fn open_in_bambu_studio(path: String, service: tauri::State<'_, SlicerService>) -> Result<()> {
    service.open_in_bambu_studio(Path::new(&path))
}

#[cfg(test)]
mod task6_tests {
    use super::{resolve_fast_request, FastSliceRequest};
    use crate::printers::SavedPrinter;
    use std::{fs, io::Write, path::PathBuf};
    use uuid::Uuid;
    use zip::write::FileOptions;

    struct Fixture {
        root: PathBuf,
        input: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("cylune-fast-request-{}", Uuid::new_v4()));
            for category in ["machine", "process", "filament"] {
                fs::create_dir_all(root.join("profiles/BBL").join(category)).unwrap();
            }
            let write_profile = |category: &str, name: &str, contents: &str| {
                fs::write(
                    root.join("profiles/BBL")
                        .join(category)
                        .join(format!("{name}.json")),
                    contents,
                )
                .unwrap();
            };
            write_profile(
                "machine",
                "Bambu Lab P2S",
                r#"{"type":"machine_model","name":"Bambu Lab P2S","default_bed_type":"Supertack Plate"}"#,
            );
            write_profile(
                "machine",
                "Bambu Lab P2S 0.4 nozzle",
                r#"{"type":"machine","name":"Bambu Lab P2S 0.4 nozzle","instantiation":"true","printer_model":"Bambu Lab P2S","nozzle_diameter":["0.4"],"default_print_profile":"0.20mm Standard @BBL P2S","default_filament_profile":["Bambu PLA Basic @BBL P2S"]}"#,
            );
            write_profile(
                "process",
                "0.20mm Standard @BBL P2S",
                r#"{"type":"process","name":"0.20mm Standard @BBL P2S","instantiation":"true","layer_height":"0.2","compatible_printers":["Bambu Lab P2S 0.4 nozzle"]}"#,
            );
            write_profile(
                "filament",
                "Bambu PLA Basic @BBL P2S",
                r##"{"type":"filament","name":"Bambu PLA Basic @BBL P2S","instantiation":"true","filament_type":["PLA"],"filament_colour":["#FFFFFF"],"compatible_printers":["Bambu Lab P2S 0.4 nozzle"]}"##,
            );
            write_profile(
                "filament",
                "fdm_filament_common",
                r#"{"supertack_plate_temp":["45"]}"#,
            );

            let input = root.join("ordinary.3mf");
            write_project(&input, "Bambu Lab P2S");
            Self { root, input }
        }

        fn request(&self) -> FastSliceRequest {
            FastSliceRequest {
                input_path: self.input.clone(),
                printer_id: self.printer().printer_id,
                confirm_printer_mismatch: false,
            }
        }

        fn printer(&self) -> SavedPrinter {
            SavedPrinter {
                printer_id: Uuid::new_v4().to_string(),
                display_name: "P2S".to_owned(),
                model_key: "Bambu Lab P2S".to_owned(),
                nozzle_diameter: 0.4,
                default_plate: "Supertack Plate".to_owned(),
                ams_kind: "ams".to_owned(),
                is_default: true,
                is_available: true,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn write_project(path: &std::path::Path, model: &str) {
        let mut archive = zip::ZipWriter::new(fs::File::create(path).unwrap());
        let options = FileOptions::default();
        let settings = format!(
            r##"{{"printer_model":"{model}","printer_settings_id":"{model} 0.4 nozzle","nozzle_diameter":["0.4"],"filament_settings_id":["Bambu PLA Basic @BBL P2S","Bambu PLA Basic @BBL P2S"],"filament_type":["PLA","PLA"],"filament_colour":["#FFFFFF","#000000"]}}"##
        );
        for (name, contents) in [
            ("[Content_Types].xml", b"<Types/>".as_slice()),
            ("3D/3dmodel.model", b"<model/>".as_slice()),
            ("Metadata/project_settings.config", settings.as_bytes()),
            (
                "Metadata/model_settings.config",
                br#"<config><plate><metadata key="plater_id" value="1"/></plate></config>"#
                    .as_slice(),
            ),
        ] {
            archive.start_file(name, options).unwrap();
            archive.write_all(contents).unwrap();
        }
        archive.finish().unwrap();
    }

    #[test]
    fn resolves_only_the_internal_machine_path_and_canonical_input() {
        let fixture = Fixture::new();
        let printer = fixture.printer();
        let request = fixture.request();

        let resolved =
            resolve_fast_request(&fixture.root.join("profiles"), printer, request).unwrap();

        let profiles = fs::canonicalize(fixture.root.join("profiles/BBL")).unwrap();
        assert!(resolved
            .machine_settings
            .starts_with(profiles.join("machine")));
        assert_eq!(resolved.input, fs::canonicalize(&fixture.input).unwrap());
        assert!(!resolved.estimate_mode);
    }

    #[test]
    fn serializes_private_metadata_request_without_output_fields() {
        let fixture = Fixture::new();

        let value = serde_json::to_value(fixture.request()).unwrap();
        let object = value.as_object().unwrap();

        assert_eq!(object.len(), 3);
        assert!(object.contains_key("input_path"));
        assert!(object.contains_key("printer_id"));
        assert!(object.contains_key("confirm_printer_mismatch"));
        assert!(!object.contains_key("output_path"));
        assert!(!object.contains_key("destination"));
        assert!(!object.contains_key("allow_overwrite"));
    }

    #[test]
    fn matching_embedded_machine_does_not_enable_estimation() {
        let fixture = Fixture::new();
        let printer = fixture.printer();
        let request = fixture.request();

        let resolved =
            resolve_fast_request(&fixture.root.join("profiles"), printer, request).unwrap();

        assert!(!resolved.estimate_mode);
    }

    #[test]
    fn requires_explicit_confirmation_for_an_embedded_printer_mismatch() {
        let fixture = Fixture::new();
        let mismatch = fixture.root.join("a1-project.3mf");
        write_project(&mismatch, "Bambu Lab A1");
        let mut request = fixture.request();
        request.input_path = mismatch;
        assert!(resolve_fast_request(
            &fixture.root.join("profiles"),
            fixture.printer(),
            request.clone(),
        )
        .is_err());

        request.confirm_printer_mismatch = true;
        let resolved =
            resolve_fast_request(&fixture.root.join("profiles"), fixture.printer(), request)
                .unwrap();
        assert!(resolved.estimate_mode);
    }
}
