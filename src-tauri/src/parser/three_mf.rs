use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use serde_json::Value;
use zip::ZipArchive;

use super::gcode::parse_gcode;
use super::{FilamentProfile, ParsedPrintFile};
use crate::error::{AppError, Result};

pub fn parse_3mf(path: &Path) -> Result<ParsedPrintFile> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|_| AppError::InvalidFile)?;
    let mut settings = Vec::new();
    let mut gcode_name = None;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| AppError::InvalidFile)?;
        let name = entry.name().to_owned();

        if is_filament_config(&name) {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            let config = parse_config(&contents)?;
            if contains_filament_settings(&config) {
                settings.push(config);
            }
        } else if is_gcode_entry(&name) && gcode_name.is_none() {
            gcode_name = Some(name);
        }
    }

    let Some(gcode_name) = gcode_name else {
        return Err(AppError::UnslicedProject);
    };
    let gcode_entry = archive
        .by_name(&gcode_name)
        .map_err(|_| AppError::InvalidFile)?;
    let gcode = parse_gcode(BufReader::new(gcode_entry))?;

    let mut filaments = Vec::new();
    for config in &settings {
        filaments.extend(profiles_from_config(config)?);
    }
    if filaments.is_empty() {
        return Err(AppError::InvalidFile);
    }

    Ok(ParsedPrintFile { filaments, gcode })
}

fn is_filament_config(name: &str) -> bool {
    let Some(file_name) = name.strip_prefix("Metadata/") else {
        return false;
    };

    file_name == "project_settings.config"
        || file_name == "filament_settings.config"
        || (file_name.starts_with("filament_settings_") && file_name.ends_with(".config"))
}

fn contains_filament_settings(config: &Value) -> bool {
    config
        .as_object()
        .is_some_and(|object| object.contains_key("filament_settings_id"))
}

fn is_gcode_entry(name: &str) -> bool {
    name.ends_with(".gcode")
}

fn parse_config(contents: &str) -> Result<Value> {
    serde_json::from_str(contents).map_err(|_| AppError::InvalidFile)
}

fn profiles_from_config(config: &Value) -> Result<Vec<FilamentProfile>> {
    let object = config.as_object().ok_or(AppError::InvalidFile)?;
    let preset_ids = required_values(object, "filament_settings_id")?;

    (0..preset_ids.len())
        .map(|index| {
            let preset_id = required_string(object, "filament_settings_id", index)?;
            let material = required_string(object, "filament_type", index)?;
            let (brand, series) = normalize_preset(&preset_id, &material);

            Ok(FilamentProfile {
                tool: u8::try_from(index).map_err(|_| AppError::InvalidFile)?,
                preset_id,
                brand,
                material,
                series,
                color_hex: required_string(object, "filament_colour", index)?,
                diameter_mm: required_number(object, "filament_diameter", index)?,
                density_g_cm3: required_number(object, "filament_density", index)?,
                unknown_fields: unknown_fields(object, index),
            })
        })
        .collect()
}

fn required_values<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a [Value]> {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .filter(|values| !values.is_empty())
        .ok_or(AppError::InvalidFile)
}

fn required_string(
    object: &serde_json::Map<String, Value>,
    key: &str,
    index: usize,
) -> Result<String> {
    required_values(object, key)?
        .get(index)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
        .ok_or(AppError::InvalidFile)
}

fn required_number(
    object: &serde_json::Map<String, Value>,
    key: &str,
    index: usize,
) -> Result<f64> {
    let value = required_string(object, key, index)?
        .parse::<f64>()
        .map_err(|_| AppError::InvalidFile)?;
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(AppError::InvalidFile)
    }
}

fn unknown_fields(
    object: &serde_json::Map<String, Value>,
    index: usize,
) -> BTreeMap<String, Value> {
    object
        .iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "filament_settings_id"
                    | "filament_type"
                    | "filament_colour"
                    | "filament_diameter"
                    | "filament_density"
            )
        })
        .map(|(key, value)| {
            let value = value
                .as_array()
                .and_then(|items| items.get(index))
                .cloned()
                .unwrap_or_else(|| value.clone());
            (key.clone(), value)
        })
        .collect()
}

fn normalize_preset(preset_id: &str, material: &str) -> (String, String) {
    let name = preset_id
        .strip_prefix("Bambu Lab ")
        .or_else(|| preset_id.strip_prefix("Bambu "));
    let Some(name) = name else {
        return (String::new(), preset_id.to_owned());
    };

    let preset_name = name.split(" @").next().unwrap_or(name);
    let series = preset_name
        .strip_prefix(material)
        .unwrap_or(preset_name)
        .trim()
        .to_owned();
    ("Bambu Lab".to_owned(), series)
}

#[cfg(test)]
mod tests {
    use super::super::{parse_3mf, FilamentProfile};
    use std::collections::BTreeMap;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use zip::write::FileOptions;

    #[test]
    fn reads_bambu_profiles_and_multicolor_gcode_from_a_sliced_3mf() {
        let parsed = parse_3mf(&fixture("bambu_multicolor.3mf")).unwrap();

        assert_eq!(parsed.filaments[0].preset_id, "Bambu PLA Basic @BBL A1");
        assert_eq!(parsed.filaments[0].brand, "Bambu Lab");
        assert_eq!(parsed.filaments[0].material, "PLA");
        assert_eq!(parsed.filaments[0].series, "Basic");
        assert_eq!(parsed.filaments[1].series, "Matte");
        assert_eq!(
            parsed.filaments[0].unknown_fields["filament_start_gcode"],
            "; tool 0"
        );
        assert_eq!(parsed.gcode.totals_mm.len(), 2);
    }

    #[test]
    fn identifies_a_3mf_without_sliced_gcode_as_an_unsliced_project() {
        let error = parse_3mf(&fixture("project_only.3mf")).unwrap_err();

        assert_eq!(error.code(), "unsliced_project");
    }

    #[test]
    fn converts_filament_length_to_grams_using_its_diameter_and_density() {
        let profile = FilamentProfile {
            tool: 0,
            preset_id: "Bambu PLA Basic".to_owned(),
            brand: "Bambu Lab".to_owned(),
            material: "PLA".to_owned(),
            series: "Basic".to_owned(),
            color_hex: "#FFFFFF".to_owned(),
            diameter_mm: 1.75,
            density_g_cm3: 1.24,
            unknown_fields: BTreeMap::new(),
        };

        assert!((profile.grams_for_length_mm(1000.0) - 2.98).abs() < 0.01);
    }

    #[test]
    fn rejects_profiles_with_missing_or_malformed_required_fields() {
        for config in [
            r##"{"filament_type":["PLA"],"filament_colour":["#FFFFFF"],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##,
            r##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##,
            r##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_colour":[17],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##,
            r##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"],"filament_diameter":["NaN"],"filament_density":["1.24"]}"##,
            r##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"],"filament_diameter":["1.75"]}"##,
        ] {
            assert_invalid_profile(config);
        }
    }

    #[test]
    fn reads_project_settings_json_and_ignores_xml_metadata() {
        let path = temporary_archive_path();
        write_realistic_archive(&path);

        let parsed = parse_3mf(&path).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(parsed.filaments[0].preset_id, "Bambu PLA Basic");
        assert_eq!(parsed.gcode.totals_mm[&0], 1.0);
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn assert_invalid_profile(config: &str) {
        let path = temporary_archive_path();
        write_test_archive(&path, config);

        let error = parse_3mf(&path).unwrap_err();

        fs::remove_file(path).unwrap();
        assert_eq!(error.code(), "invalid_file");
    }

    fn write_test_archive(path: &PathBuf, config: &str) {
        let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
        let options = FileOptions::default();
        archive
            .start_file("Metadata/filament_settings.config", options)
            .unwrap();
        archive.write_all(config.as_bytes()).unwrap();
        archive
            .start_file("Metadata/plate_1.gcode", options)
            .unwrap();
        archive.write_all(b"M83\nG1 E1\n").unwrap();
        archive.finish().unwrap();
    }

    fn write_realistic_archive(path: &PathBuf) {
        let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
        let options = FileOptions::default();
        archive
            .start_file("Metadata/model_settings.config", options)
            .unwrap();
        archive
            .write_all(b"<?xml version=\"1.0\"?><config />")
            .unwrap();
        archive
            .start_file("Metadata/slice_info.config", options)
            .unwrap();
        archive
            .write_all(b"<?xml version=\"1.0\"?><config />")
            .unwrap();
        archive
            .start_file("Metadata/project_settings.config", options)
            .unwrap();
        archive
            .write_all(
                br##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##,
            )
            .unwrap();
        archive
            .start_file("Metadata/plate_1.gcode", options)
            .unwrap();
        archive.write_all(b"M83\nG1 E1\n").unwrap();
        archive.finish().unwrap();
    }

    fn temporary_archive_path() -> PathBuf {
        static NEXT_PATH: AtomicUsize = AtomicUsize::new(0);

        std::env::temp_dir().join(format!(
            "bambu-pools-invalid-profile-{}-{}.3mf",
            std::process::id(),
            NEXT_PATH.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
