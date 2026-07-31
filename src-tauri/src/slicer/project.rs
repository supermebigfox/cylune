use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use serde_json::{Map, Value};
use zip::{write::FileOptions, ZipArchive, ZipWriter};

use crate::error::{AppError, Result};

const PROJECT_SETTINGS: &str = "Metadata/project_settings.config";
const MAX_PROJECT_SETTINGS_BYTES: u64 = 8 * 1024 * 1024;

pub(crate) fn remap_project_for_machine(
    source: &Path,
    destination: &Path,
    machine_settings: &Path,
) -> Result<()> {
    let machine = serde_json::from_slice::<Value>(&fs::read(machine_settings)?)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or(AppError::SlicerIncompatible)?;
    let target_machine = required_string(&machine, "name")?;
    let target_model = required_string(&machine, "printer_model")?;
    let target_process = required_string(&machine, "default_print_profile")?;
    let target_filament = machine
        .get("default_filament_profile")
        .and_then(Value::as_array)
        .and_then(|profiles| profiles.first())
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .ok_or(AppError::SlicerIncompatible)?
        .to_owned();

    let input = File::open(source)?;
    let mut archive = ZipArchive::new(input).map_err(|_| AppError::InvalidFile)?;
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)?;
    let mut writer = ZipWriter::new(output);
    let mut remapped_settings = false;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| AppError::InvalidFile)?;
        let name = entry.name().to_owned();
        let mut options = FileOptions::default()
            .compression_method(entry.compression())
            .last_modified_time(entry.last_modified())
            .large_file(entry.size() > u32::MAX as u64);
        if let Some(mode) = entry.unix_mode() {
            options = options.unix_permissions(mode);
        }

        if entry.is_dir() {
            writer
                .add_directory(name, options)
                .map_err(|_| AppError::InvalidFile)?;
            continue;
        }
        writer
            .start_file(&name, options)
            .map_err(|_| AppError::InvalidFile)?;
        if name == PROJECT_SETTINGS {
            if remapped_settings || entry.size() > MAX_PROJECT_SETTINGS_BYTES {
                return Err(AppError::InvalidFile);
            }
            let mut bytes = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut bytes)?;
            let mut settings = serde_json::from_slice::<Value>(&bytes)
                .ok()
                .and_then(|value| value.as_object().cloned())
                .ok_or(AppError::InvalidFile)?;
            remap_identity(
                &mut settings,
                &target_model,
                &target_machine,
                &target_process,
                &target_filament,
            );
            writer.write_all(&serde_json::to_vec(&settings).map_err(|_| AppError::InvalidFile)?)?;
            remapped_settings = true;
        } else {
            io::copy(&mut entry, &mut writer)?;
        }
    }
    if !remapped_settings {
        return Err(AppError::InvalidFile);
    }
    writer.finish().map_err(|_| AppError::InvalidFile)?;
    Ok(())
}

fn required_string(object: &Map<String, Value>, key: &str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or(AppError::SlicerIncompatible)
}

fn remap_identity(
    settings: &mut Map<String, Value>,
    target_model: &str,
    target_machine: &str,
    target_process: &str,
    target_filament: &str,
) {
    settings.insert(
        "printer_model".to_owned(),
        Value::String(target_model.to_owned()),
    );
    settings.insert(
        "printer_settings_id".to_owned(),
        Value::String(target_machine.to_owned()),
    );
    settings.insert(
        "print_compatible_printers".to_owned(),
        Value::Array(vec![Value::String(target_machine.to_owned())]),
    );
    if settings.contains_key("compatible_printers") {
        settings.insert(
            "compatible_printers".to_owned(),
            Value::Array(vec![Value::String(target_machine.to_owned())]),
        );
    }

    let process_name = settings
        .get("print_settings_id")
        .and_then(Value::as_str)
        .map(|source| remap_profile_name(source, target_process))
        .unwrap_or_else(|| target_process.to_owned());
    settings.insert("print_settings_id".to_owned(), Value::String(process_name));
    settings.insert(
        "default_print_profile".to_owned(),
        Value::String(target_process.to_owned()),
    );

    for key in ["filament_settings_id", "default_filament_profile"] {
        let remapped = settings
            .get(key)
            .and_then(Value::as_array)
            .map(|profiles| {
                profiles
                    .iter()
                    .map(|profile| {
                        Value::String(
                            profile
                                .as_str()
                                .map(|source| remap_profile_name(source, target_filament))
                                .unwrap_or_else(|| target_filament.to_owned()),
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![Value::String(target_filament.to_owned())]);
        settings.insert(key.to_owned(), Value::Array(remapped));
    }
}

fn remap_profile_name(source: &str, target_template: &str) -> String {
    let Some((source_base, _)) = source.split_once(" @") else {
        return target_template.to_owned();
    };
    let Some((_, target_suffix)) = target_template.split_once(" @") else {
        return target_template.to_owned();
    };
    format!("{} @{}", source_base.trim(), target_suffix.trim())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::{Read, Write};

    use serde_json::{json, Value};
    use uuid::Uuid;
    use zip::{write::FileOptions, ZipArchive, ZipWriter};

    use super::remap_project_for_machine;

    #[test]
    fn remaps_only_compatibility_identity_and_preserves_project_settings() {
        let root = std::env::temp_dir().join(format!("cylune-project-remap-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.3mf");
        let destination = root.join("compatible.3mf");
        let machine = root.join("machine.json");
        let settings = json!({
            "printer_model": "Bambu Lab X2D",
            "printer_settings_id": "Bambu Lab X2D 0.4 nozzle",
            "print_settings_id": "0.16mm Optimal @BBL X2D",
            "default_print_profile": "0.20mm Standard @BBL X2D",
            "print_compatible_printers": ["Bambu Lab X2D 0.4 nozzle"],
            "filament_settings_id": [
                "Bambu PLA Basic @BBL X2D 0.4 nozzle",
                "Bambu PETG HF @BBL X2D 0.4 nozzle-custom(project.3mf)"
            ],
            "default_filament_profile": [
                "Bambu PLA Basic @BBL X2D 0.4 nozzle",
                "Bambu PETG HF @BBL X2D 0.4 nozzle"
            ],
            "layer_height": "0.12",
            "wall_loops": "5",
            "sparse_infill_density": "37%",
            "enable_support": "1",
            "ironing_type": "top",
            "nozzle_temperature": ["213", "227"]
        });
        write_project(&source, &settings);
        fs::write(
            &machine,
            serde_json::to_vec(&json!({
                "name": "Bambu Lab P2S 0.4 nozzle",
                "printer_model": "Bambu Lab P2S",
                "default_print_profile": "0.20mm Standard @BBL P2S",
                "default_filament_profile": ["Bambu PLA Basic @BBL P2S"]
            }))
            .unwrap(),
        )
        .unwrap();
        let source_before = fs::read(&source).unwrap();

        remap_project_for_machine(&source, &destination, &machine).unwrap();

        assert_eq!(fs::read(&source).unwrap(), source_before);
        let (remapped, model) = read_project(&destination);
        assert_eq!(model, b"unchanged-model");
        assert_eq!(remapped["printer_model"], "Bambu Lab P2S");
        assert_eq!(remapped["printer_settings_id"], "Bambu Lab P2S 0.4 nozzle");
        assert_eq!(remapped["print_settings_id"], "0.16mm Optimal @BBL P2S");
        assert_eq!(
            remapped["default_print_profile"],
            "0.20mm Standard @BBL P2S"
        );
        assert_eq!(
            remapped["print_compatible_printers"],
            json!(["Bambu Lab P2S 0.4 nozzle"])
        );
        assert_eq!(
            remapped["filament_settings_id"],
            json!(["Bambu PLA Basic @BBL P2S", "Bambu PETG HF @BBL P2S"])
        );
        assert_eq!(remapped["layer_height"], settings["layer_height"]);
        assert_eq!(remapped["wall_loops"], settings["wall_loops"]);
        assert_eq!(
            remapped["sparse_infill_density"],
            settings["sparse_infill_density"]
        );
        assert_eq!(remapped["enable_support"], settings["enable_support"]);
        assert_eq!(remapped["ironing_type"], settings["ironing_type"]);
        assert_eq!(
            remapped["nozzle_temperature"],
            settings["nozzle_temperature"]
        );

        fs::remove_dir_all(root).unwrap();
    }

    fn write_project(path: &std::path::Path, settings: &Value) {
        let mut archive = ZipWriter::new(File::create(path).unwrap());
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        archive
            .start_file("Metadata/project_settings.config", options)
            .unwrap();
        archive
            .write_all(&serde_json::to_vec(settings).unwrap())
            .unwrap();
        archive.start_file("3D/3dmodel.model", options).unwrap();
        archive.write_all(b"unchanged-model").unwrap();
        archive.finish().unwrap();
    }

    fn read_project(path: &std::path::Path) -> (Value, Vec<u8>) {
        let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
        let mut settings = String::new();
        archive
            .by_name("Metadata/project_settings.config")
            .unwrap()
            .read_to_string(&mut settings)
            .unwrap();
        let mut model = Vec::new();
        archive
            .by_name("3D/3dmodel.model")
            .unwrap()
            .read_to_end(&mut model)
            .unwrap();
        (serde_json::from_str(&settings).unwrap(), model)
    }
}
