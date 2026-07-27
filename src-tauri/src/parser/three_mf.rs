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

        if is_config_entry(&name) {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            settings.push(parse_config(&contents)?);
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

    Ok(ParsedPrintFile {
        filaments: settings.iter().flat_map(profiles_from_config).collect(),
        gcode,
    })
}

fn is_config_entry(name: &str) -> bool {
    name.starts_with("Metadata/") && name.ends_with(".config")
}

fn is_gcode_entry(name: &str) -> bool {
    name.ends_with(".gcode")
}

fn parse_config(contents: &str) -> Result<Value> {
    serde_json::from_str(contents).map_err(|_| AppError::InvalidFile)
}

fn profiles_from_config(config: &Value) -> Vec<FilamentProfile> {
    let Some(object) = config.as_object() else {
        return Vec::new();
    };
    let count = values(object, "filament_settings_id").len();

    (0..count)
        .map(|index| {
            let preset_id = value_at(object, "filament_settings_id", index);
            let material = value_at(object, "filament_type", index);
            let (brand, series) = normalize_preset(&preset_id, &material);

            FilamentProfile {
                tool: index as u8,
                preset_id,
                brand,
                material,
                series,
                color_hex: value_at(object, "filament_colour", index),
                diameter_mm: parse_number(object, "filament_diameter", index),
                density_g_cm3: parse_number(object, "filament_density", index),
                unknown_fields: unknown_fields(object, index),
            }
        })
        .collect()
}

fn values<'a>(object: &'a serde_json::Map<String, Value>, key: &str) -> &'a [Value] {
    object
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn value_at(object: &serde_json::Map<String, Value>, key: &str, index: usize) -> String {
    values(object, key)
        .get(index)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn parse_number(object: &serde_json::Map<String, Value>, key: &str, index: usize) -> f64 {
    value_at(object, key, index).parse().unwrap_or_default()
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
    use std::path::PathBuf;

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

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }
}
