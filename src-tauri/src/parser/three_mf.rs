use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use serde_json::Value;
use zip::ZipArchive;

use super::gcode::parse_gcode;
use super::{preset_base, FilamentProfile, ParsedPlate, ParsedPrintFile, ParsedProjectV2};
use crate::error::{AppError, Result};

pub fn parse_3mf(path: &Path) -> Result<ParsedPrintFile> {
    let project = parse_3mf_project(path)?;
    let plate = project
        .plates
        .into_iter()
        .next()
        .ok_or(AppError::UnslicedProject)?;

    Ok(ParsedPrintFile {
        filaments: plate.filaments,
        gcode: plate.gcode,
    })
}

pub fn parse_3mf_project(path: &Path) -> Result<ParsedProjectV2> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(|_| AppError::InvalidFile)?;
    let mut settings = Vec::new();
    let mut gcode_entries = Vec::new();
    let mut entry_names = BTreeSet::new();
    let mut plate_json = BTreeMap::new();
    let mut slice_info = None;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| AppError::InvalidFile)?;
        let name = entry.name().to_owned();
        entry_names.insert(name.clone());

        if is_filament_config(&name) {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            let config = parse_config(&contents)?;
            if contains_filament_settings(&config) {
                settings.push(config);
            }
        } else if name == "Metadata/slice_info.config" {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            slice_info = Some(contents);
        } else if let Some(plate_index) = plate_json_index(&name) {
            let mut contents = String::new();
            entry.read_to_string(&mut contents)?;
            plate_json.insert(plate_index, contents);
        } else if let Some(plate_index) = plate_gcode_index(&name) {
            gcode_entries.push((plate_index, name));
        }
    }

    if gcode_entries.is_empty() {
        return Err(AppError::UnslicedProject);
    }
    gcode_entries.sort_unstable_by_key(|(index, _)| *index);
    if gcode_entries
        .windows(2)
        .any(|entries| entries[0].0 == entries[1].0)
    {
        return Err(AppError::InvalidFile);
    }

    let mut filaments = Vec::new();
    for config in &settings {
        filaments.extend(profiles_from_config(config)?);
    }
    if filaments.is_empty() {
        return Err(AppError::InvalidFile);
    }

    let predictions = slice_info
        .as_deref()
        .map(slice_predictions)
        .unwrap_or_default();
    let single_plate = gcode_entries.len() == 1;
    let plates = gcode_entries
        .into_iter()
        .map(|(plate_index, gcode_name)| {
            let gcode_entry = archive
                .by_name(&gcode_name)
                .map_err(|_| AppError::InvalidFile)?;
            let gcode = parse_gcode(BufReader::new(gcode_entry))?;
            let display_name = plate_json
                .get(&plate_index)
                .map(String::as_str)
                .map(parse_display_name)
                .transpose()?
                .flatten();

            Ok(ParsedPlate {
                plate_index,
                display_name,
                estimated_seconds: predictions
                    .get(&plate_index)
                    .copied()
                    .or(gcode.declared_estimated_seconds),
                thumbnail_entries: thumbnail_entries(plate_index, &entry_names, single_plate),
                filaments: filaments.clone(),
                gcode,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(ParsedProjectV2 { version: 2, plates })
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

fn plate_gcode_index(name: &str) -> Option<u32> {
    plate_entry_index(name, "plate_", ".gcode")
}

fn plate_json_index(name: &str) -> Option<u32> {
    plate_entry_index(name, "plate_", ".json")
}

fn plate_entry_index(name: &str, prefix: &str, suffix: &str) -> Option<u32> {
    let value = name
        .strip_prefix("Metadata/")?
        .strip_prefix(prefix)?
        .strip_suffix(suffix)?;
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

fn thumbnail_entries(
    plate_index: u32,
    entry_names: &BTreeSet<String>,
    prefer_project_thumbnail: bool,
) -> Vec<String> {
    let project = [
        "Auxiliaries/.thumbnails/thumbnail_middle.png".to_owned(),
        "Auxiliaries/.thumbnails/thumbnail_3mf.png".to_owned(),
    ];
    let plate = [
        format!("Metadata/plate_{plate_index}.png"),
        format!("Metadata/plate_{plate_index}_small.png"),
        format!("Metadata/plate_no_light_{plate_index}.png"),
    ];
    let candidates = if prefer_project_thumbnail {
        project.into_iter().chain(plate).collect::<Vec<_>>()
    } else {
        plate.into_iter().chain(project).collect::<Vec<_>>()
    };
    candidates
        .into_iter()
        .filter(|name| entry_names.contains(name))
        .collect()
}

fn parse_display_name(contents: &str) -> Result<Option<String>> {
    let metadata = parse_config(contents)?;
    Ok(["name", "display_name", "plate_name"]
        .into_iter()
        .find_map(|key| metadata.get(key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned))
}

fn slice_predictions(contents: &str) -> BTreeMap<u32, u32> {
    contents
        .split("<plate")
        .skip(1)
        .filter_map(|element| {
            if !element.starts_with(char::is_whitespace) && !element.starts_with('>') {
                return None;
            }
            let tag = element.split('>').next()?;
            let contents = element.split("</plate>").next().unwrap_or(element);
            let index = xml_attribute(tag, "index")
                .or_else(|| plate_metadata_value(contents, "index"))?
                .parse()
                .ok()?;
            let prediction = xml_attribute(tag, "prediction")
                .or_else(|| plate_metadata_value(contents, "prediction"))?
                .parse()
                .ok()?;
            Some((index, prediction))
        })
        .collect()
}

fn plate_metadata_value<'a>(contents: &'a str, key: &str) -> Option<&'a str> {
    contents.split("<metadata").skip(1).find_map(|element| {
        let tag = element.split('>').next()?;
        (xml_attribute(tag, "key") == Some(key))
            .then(|| xml_attribute(tag, "value"))
            .flatten()
    })
}

fn xml_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    tag.split_ascii_whitespace().find_map(|attribute| {
        let value = attribute.strip_prefix(name)?.strip_prefix('=')?;
        value
            .trim_end_matches('/')
            .strip_prefix('\"')?
            .strip_suffix('\"')
    })
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

    let preset_name = preset_base(name);
    let series = preset_name
        .strip_prefix(material)
        .unwrap_or(preset_name)
        .trim()
        .to_owned();
    ("Bambu Lab".to_owned(), series)
}

#[cfg(test)]
mod tests {
    use super::super::{parse_3mf, parse_3mf_project, FilamentProfile};
    use super::slice_predictions;
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
    fn parses_every_sliced_plate_in_numeric_order() {
        let path = fixture_with_plates(&[(3, 7200, 7), (1, 3600, 3)]);

        let project = parse_3mf_project(&path).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(
            project
                .plates
                .iter()
                .map(|plate| plate.plate_index)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(project.plates[0].estimated_seconds, Some(3600));
        assert_eq!(project.plates[0].gcode.max_layer, 3);
        assert_eq!(project.plates[1].estimated_seconds, Some(7200));
        assert_eq!(project.plates[1].gcode.max_layer, 7);
        assert_eq!(project.plates[1].gcode.totals_mm[&1], 7.0);
    }

    #[test]
    fn keeps_missing_plate_images_empty_and_preserves_single_plate_compatibility() {
        let path = fixture_with_plates(&[(1, 3600, 14)]);

        let project = parse_3mf_project(&path).unwrap();

        assert_eq!(project.plates[0].thumbnail_entries, Vec::<String>::new());
        assert_eq!(parse_3mf(&path).unwrap().gcode.max_layer, 14);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn ignores_non_plate_gcode_entries_when_finding_sliced_plates() {
        let path = temporary_archive_path();
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = FileOptions::default();
        write_filament_config(&mut archive, options);
        archive
            .start_file("Metadata/plate_one.gcode", options)
            .unwrap();
        archive.write_all(b"M83\nG1 E1\n").unwrap();
        archive.finish().unwrap();

        let error = parse_3mf_project(&path).unwrap_err();

        fs::remove_file(path).unwrap();
        assert_eq!(error.code(), "unsliced_project");
    }

    #[test]
    fn associates_plate_json_images_and_gcode_time_when_slice_prediction_is_missing() {
        let path = temporary_archive_path();
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = FileOptions::default();
        write_filament_config(&mut archive, options);
        archive
            .start_file("Metadata/slice_info.config", options)
            .unwrap();
        archive
            .write_all(b"<config><plate index=\"2\" /></config>")
            .unwrap();
        archive
            .start_file("Metadata/plate_2.json", options)
            .unwrap();
        archive.write_all(br#"{"name":"Detailed plate"}"#).unwrap();
        for image in [
            "Auxiliaries/.thumbnails/thumbnail_middle.png",
            "Auxiliaries/.thumbnails/thumbnail_3mf.png",
            "Metadata/plate_2.png",
            "Metadata/plate_2_small.png",
            "Metadata/plate_no_light_2.png",
        ] {
            archive.start_file(image, options).unwrap();
            archive.write_all(b"image").unwrap();
        }
        archive
            .start_file("Metadata/plate_2.gcode", options)
            .unwrap();
        archive
            .write_all(b"; total estimated time: 5h 5m 7s\nM83\n; LAYER:0\nG1 E1\n")
            .unwrap();
        archive.finish().unwrap();

        let plate = parse_3mf_project(&path).unwrap().plates.remove(0);

        fs::remove_file(path).unwrap();
        assert_eq!(plate.display_name.as_deref(), Some("Detailed plate"));
        assert_eq!(plate.estimated_seconds, Some(18_307));
        assert_eq!(
            plate.thumbnail_entries,
            vec![
                "Auxiliaries/.thumbnails/thumbnail_middle.png",
                "Auxiliaries/.thumbnails/thumbnail_3mf.png",
                "Metadata/plate_2.png",
                "Metadata/plate_2_small.png",
                "Metadata/plate_no_light_2.png",
            ]
        );
    }

    #[test]
    fn multi_plate_projects_prioritize_each_plates_own_thumbnail() {
        let path = temporary_archive_path();
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = FileOptions::default();
        write_filament_config(&mut archive, options);
        for image in [
            "Auxiliaries/.thumbnails/thumbnail_middle.png",
            "Auxiliaries/.thumbnails/thumbnail_3mf.png",
            "Metadata/plate_1.png",
            "Metadata/plate_2.png",
        ] {
            archive.start_file(image, options).unwrap();
            archive.write_all(b"image").unwrap();
        }
        for plate_index in [1, 2] {
            archive
                .start_file(format!("Metadata/plate_{plate_index}.gcode"), options)
                .unwrap();
            archive.write_all(b"M83\nG1 E1\n").unwrap();
        }
        archive.finish().unwrap();

        let project = parse_3mf_project(&path).unwrap();

        fs::remove_file(path).unwrap();
        assert_eq!(
            project.plates[0].thumbnail_entries,
            [
                "Metadata/plate_1.png",
                "Auxiliaries/.thumbnails/thumbnail_middle.png",
                "Auxiliaries/.thumbnails/thumbnail_3mf.png",
            ]
        );
        assert_eq!(
            project.plates[1].thumbnail_entries,
            [
                "Metadata/plate_2.png",
                "Auxiliaries/.thumbnails/thumbnail_middle.png",
                "Auxiliaries/.thumbnails/thumbnail_3mf.png",
            ]
        );
    }

    #[test]
    fn reads_bambu_nested_plate_prediction_metadata() {
        let predictions = slice_predictions(
            r#"<config><plate><metadata key="index" value="1"/><metadata key="prediction" value="18307"/></plate></config>"#,
        );

        assert_eq!(predictions.get(&1), Some(&18_307));
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

    fn fixture_with_plates(plates: &[(u32, u32, u32)]) -> PathBuf {
        let path = temporary_archive_path();
        let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = FileOptions::default();
        write_filament_config(&mut archive, options);

        let slice_info = plates
            .iter()
            .map(|(index, prediction, _)| {
                format!(r#"<plate index="{index}" prediction="{prediction}" />"#)
            })
            .collect::<String>();
        archive
            .start_file("Metadata/slice_info.config", options)
            .unwrap();
        archive
            .write_all(format!("<config>{slice_info}</config>").as_bytes())
            .unwrap();

        for (index, _, layers) in plates {
            archive
                .start_file(format!("Metadata/plate_{index}.gcode"), options)
                .unwrap();
            let tool = if *index == 3 { 1 } else { 0 };
            let mut gcode = format!(
                "; total estimated time: 9h 9m 9s\n; total layer number: {layers}\nM83\nT{tool}\n"
            );
            for layer in 0..*layers {
                gcode.push_str(&format!("; LAYER:{layer}\nG1 E1\n"));
            }
            archive.write_all(gcode.as_bytes()).unwrap();
        }
        archive.finish().unwrap();
        path
    }

    fn write_filament_config(archive: &mut zip::ZipWriter<File>, options: FileOptions) {
        archive
            .start_file("Metadata/filament_settings.config", options)
            .unwrap();
        archive
            .write_all(
                br##"{"filament_settings_id":["Bambu PLA Basic"],"filament_type":["PLA"],"filament_colour":["#FFFFFF"],"filament_diameter":["1.75"],"filament_density":["1.24"]}"##,
            )
            .unwrap();
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
