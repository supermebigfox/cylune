use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::Path,
};
use zip::ZipArchive;

const MAX_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;
const MAX_TOOLS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreeMfKind {
    Unsliced,
    Sliced,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedMachine {
    pub model_key: Option<String>,
    pub preset_name: Option<String>,
    pub nozzle_diameter: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedProcess {
    pub preset_name: Option<String>,
    pub plate_type: Option<String>,
    pub infill_density: Option<f64>,
    pub support_enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddedTool {
    pub tool: u32,
    pub label: String,
    pub material: Option<String>,
    pub color_hex: Option<String>,
    pub embedded_filament_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddedPlate {
    pub plate_index: u32,
    pub tool_indices: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreeMfInspection {
    pub kind: ThreeMfKind,
    pub file_name: String,
    pub plate_count: u32,
    pub embedded_model_key: Option<String>,
    pub embedded_nozzle_diameter: Option<f64>,
    pub embedded_process_key: Option<String>,
    pub embedded_plate_key: Option<String>,
    pub embedded_infill_density: Option<f64>,
    pub embedded_support_enabled: Option<bool>,
    pub tools: Vec<EmbeddedTool>,
    pub plates: Vec<EmbeddedPlate>,
}

pub fn inspect_3mf_content(path: &Path) -> Result<ThreeMfInspection> {
    validate_input(path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .ok_or(AppError::InvalidFile)?
        .to_owned();
    let mut archive = ZipArchive::new(File::open(path)?).map_err(|_| AppError::InvalidFile)?;
    if archive.len() == 0 || archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(AppError::InvalidFile);
    }

    let mut names = BTreeSet::new();
    let mut has_content_types = false;
    let mut has_model = false;
    let mut project_settings = None;
    let mut filament_settings = None;
    let mut model_settings = None;
    let mut slice_info = None;
    let mut gcode_indices = BTreeSet::new();

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| AppError::InvalidFile)?;
        let name = entry.name().to_owned();
        if entry.enclosed_name().is_none() || !names.insert(name.clone()) {
            return Err(AppError::InvalidFile);
        }
        match name.as_str() {
            "[Content_Types].xml" => has_content_types = true,
            "3D/3dmodel.model" => has_model = true,
            "Metadata/project_settings.config" => {
                project_settings = Some(read_limited(&mut entry)?);
            }
            "Metadata/filament_settings.config" => {
                filament_settings = Some(read_limited(&mut entry)?);
            }
            "Metadata/model_settings.config" => {
                model_settings = Some(read_limited(&mut entry)?);
            }
            "Metadata/slice_info.config" => {
                slice_info = Some(read_limited(&mut entry)?);
            }
            _ => {
                if let Some(plate_index) = plate_gcode_index(&name) {
                    if entry.size() == 0 || !gcode_indices.insert(plate_index) {
                        return Err(AppError::InvalidFile);
                    }
                }
            }
        }
    }
    if !has_content_types || !has_model {
        return Err(AppError::InvalidFile);
    }

    let settings = project_settings
        .as_deref()
        .or(filament_settings.as_deref())
        .map(parse_json_object)
        .transpose()?;
    let tools = embedded_tools(settings.as_ref(), model_settings.as_deref())?;
    let kind = if gcode_indices.is_empty() {
        ThreeMfKind::Unsliced
    } else {
        ThreeMfKind::Sliced
    };
    let plates = embedded_plates(
        kind,
        &gcode_indices,
        model_settings.as_deref(),
        slice_info.as_deref(),
        &tools,
    );
    if plates.is_empty() {
        return Err(AppError::InvalidFile);
    }

    let machine = settings.as_ref().and_then(embedded_machine);
    let process = settings.as_ref().and_then(embedded_process);
    Ok(ThreeMfInspection {
        kind,
        file_name,
        plate_count: u32::try_from(plates.len()).map_err(|_| AppError::InvalidFile)?,
        embedded_model_key: machine.as_ref().and_then(|value| value.model_key.clone()),
        embedded_nozzle_diameter: machine.as_ref().and_then(|value| value.nozzle_diameter),
        embedded_process_key: process.as_ref().and_then(|value| value.preset_name.clone()),
        embedded_plate_key: process.as_ref().and_then(|value| value.plate_type.clone()),
        embedded_infill_density: process.as_ref().and_then(|value| value.infill_density),
        embedded_support_enabled: process.as_ref().and_then(|value| value.support_enabled),
        tools,
        plates,
    })
}

fn validate_input(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| AppError::InvalidFile)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || !path.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".3mf")
        })
    {
        return Err(AppError::InvalidFile);
    }
    Ok(())
}

fn read_limited(entry: &mut zip::read::ZipFile<'_>) -> Result<String> {
    if entry.size() > MAX_METADATA_BYTES {
        return Err(AppError::InvalidFile);
    }
    let mut contents = String::with_capacity(entry.size() as usize);
    entry
        .take(MAX_METADATA_BYTES + 1)
        .read_to_string(&mut contents)
        .map_err(|_| AppError::InvalidFile)?;
    if contents.len() as u64 > MAX_METADATA_BYTES {
        return Err(AppError::InvalidFile);
    }
    Ok(contents)
}

fn parse_json_object(contents: &str) -> Result<Map<String, Value>> {
    serde_json::from_str::<Value>(contents)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or(AppError::InvalidFile)
}

fn embedded_machine(settings: &Map<String, Value>) -> Option<EmbeddedMachine> {
    let machine = EmbeddedMachine {
        model_key: json_string(settings.get("printer_model")),
        preset_name: json_string(settings.get("printer_settings_id")),
        nozzle_diameter: json_string(settings.get("nozzle_diameter"))
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite() && *value > 0.0),
    };
    (machine.model_key.is_some()
        || machine.preset_name.is_some()
        || machine.nozzle_diameter.is_some())
    .then_some(machine)
}

fn embedded_process(settings: &Map<String, Value>) -> Option<EmbeddedProcess> {
    let process = EmbeddedProcess {
        preset_name: json_string(settings.get("default_print_profile")),
        plate_type: json_string(settings.get("curr_bed_type")),
        infill_density: json_string(settings.get("sparse_infill_density"))
            .and_then(|value| value.trim_end_matches('%').parse::<f64>().ok())
            .filter(|value| value.is_finite() && (0.0..=100.0).contains(value)),
        support_enabled: json_string(settings.get("enable_support")).and_then(|value| match value
            .as_str()
        {
            "1" | "true" => Some(true),
            "0" | "false" => Some(false),
            _ => None,
        }),
    };
    (process.preset_name.is_some()
        || process.plate_type.is_some()
        || process.infill_density.is_some()
        || process.support_enabled.is_some())
    .then_some(process)
}

fn embedded_tools(
    settings: Option<&Map<String, Value>>,
    model_settings: Option<&str>,
) -> Result<Vec<EmbeddedTool>> {
    let preset_names = json_strings(settings.and_then(|value| value.get("filament_settings_id")));
    let materials = json_strings(settings.and_then(|value| value.get("filament_type")));
    let colors = json_strings(settings.and_then(|value| value.get("filament_colour")));
    let model_tool_count = model_settings
        .map(metadata_extruder_indices)
        .and_then(|indices| indices.into_iter().max())
        .unwrap_or(0) as usize;
    let count = preset_names
        .len()
        .max(materials.len())
        .max(colors.len())
        .max(model_tool_count)
        .max(1);
    if count > MAX_TOOLS {
        return Err(AppError::InvalidFile);
    }
    Ok((0..count)
        .map(|index| EmbeddedTool {
            tool: index as u32,
            label: preset_names
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("Tool {}", index + 1)),
            material: materials.get(index).cloned(),
            color_hex: colors.get(index).cloned(),
            embedded_filament_key: preset_names
                .get(index)
                .map(|value| normalize_embedded_filament_key(value)),
        })
        .collect())
}

fn normalize_embedded_filament_key(value: &str) -> String {
    let had_project_suffix = value.contains('(');
    let without_project = value.split('(').next().unwrap_or(value).trim();
    let copy = without_project.rsplit_once('-').filter(|(_, suffix)| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    });
    let without_copy = copy.map_or(without_project, |(base, _)| base);
    let normalized = if had_project_suffix || copy.is_some() {
        without_copy
            .strip_suffix(" 0.4 nozzle")
            .unwrap_or(without_copy)
    } else {
        without_copy
    };
    normalized.trim().to_owned()
}

fn embedded_plates(
    kind: ThreeMfKind,
    gcode_indices: &BTreeSet<u32>,
    model_settings: Option<&str>,
    slice_info: Option<&str>,
    tools: &[EmbeddedTool],
) -> Vec<EmbeddedPlate> {
    let all_tools = tools.iter().map(|tool| tool.tool).collect::<Vec<_>>();
    let mut per_plate = slice_info.map(slice_plate_tools).unwrap_or_default();
    for indices in per_plate.values_mut() {
        indices.retain(|tool| (*tool as usize) < tools.len());
    }
    let indices = match kind {
        ThreeMfKind::Sliced => gcode_indices.clone(),
        ThreeMfKind::Unsliced => {
            let mut indices = model_settings
                .map(project_plate_indices)
                .unwrap_or_default();
            if indices.is_empty() {
                indices.insert(1);
            }
            indices
        }
    };
    indices
        .into_iter()
        .map(|plate_index| EmbeddedPlate {
            plate_index,
            tool_indices: per_plate
                .get(&plate_index)
                .cloned()
                .filter(|indices| !indices.is_empty())
                .unwrap_or_else(|| all_tools.clone()),
        })
        .collect()
}

fn project_plate_indices(contents: &str) -> BTreeSet<u32> {
    metadata_values(contents, "plater_id")
        .into_iter()
        .filter_map(|value| value.parse().ok())
        .filter(|index| *index > 0)
        .collect()
}

fn metadata_extruder_indices(contents: &str) -> Vec<u32> {
    metadata_values(contents, "extruder")
        .into_iter()
        .filter_map(|value| value.parse().ok())
        .filter(|index| *index > 0)
        .collect()
}

fn metadata_values<'a>(contents: &'a str, wanted: &str) -> Vec<&'a str> {
    contents
        .split("<metadata")
        .skip(1)
        .filter_map(|element| element.split('>').next())
        .filter(|tag| xml_attribute(tag, "key") == Some(wanted))
        .filter_map(|tag| xml_attribute(tag, "value"))
        .collect()
}

fn slice_plate_tools(contents: &str) -> BTreeMap<u32, Vec<u32>> {
    contents
        .split("<plate")
        .skip(1)
        .filter_map(|element| {
            let body = element.split("</plate>").next().unwrap_or(element);
            let index = metadata_values(body, "index")
                .into_iter()
                .next()?
                .parse::<u32>()
                .ok()?;
            let mut tools = body
                .split("<filament")
                .skip(1)
                .filter_map(|element| element.split('>').next())
                .filter_map(|tag| xml_attribute(tag, "id"))
                .filter_map(|value| value.parse::<u32>().ok())
                .filter(|tool| *tool > 0)
                .map(|tool| tool - 1)
                .collect::<Vec<_>>();
            tools.sort_unstable();
            tools.dedup();
            Some((index, tools))
        })
        .collect()
}

fn json_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(value)) => Some(value.clone()),
        Some(Value::Array(values)) => values.iter().find_map(Value::as_str).map(str::to_owned),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.trim().is_empty())
}

fn json_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(value)) if !value.trim().is_empty() => vec![value.clone()],
        _ => Vec::new(),
    }
}

fn plate_gcode_index(name: &str) -> Option<u32> {
    let value = name
        .strip_prefix("Metadata/plate_")?
        .strip_suffix(".gcode")?;
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

fn xml_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    tag.split_ascii_whitespace().find_map(|attribute| {
        let value = attribute.strip_prefix(name)?.strip_prefix('=')?;
        value
            .trim_end_matches('/')
            .strip_prefix('"')?
            .strip_suffix('"')
    })
}

#[cfg(test)]
mod tests {
    use super::{
        embedded_tools, inspect_3mf_content, normalize_embedded_filament_key, ThreeMfKind,
    };
    use crate::{
        imports::sha256, printers::SavedPrinter, slicer::catalog::load_slice_preset_catalog,
    };
    use std::{
        fs::{self, File},
        io::Write,
        path::PathBuf,
    };
    use uuid::Uuid;
    use zip::write::FileOptions;

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("cylune-inspect-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn archive(&self, name: &str, sliced: bool) -> PathBuf {
            let path = self.root.join(name);
            let mut archive = zip::ZipWriter::new(File::create(&path).unwrap());
            let options = FileOptions::default();
            for (entry, contents) in [
                ("[Content_Types].xml", b"<Types/>".as_slice()),
                ("3D/3dmodel.model", b"<model/>".as_slice()),
                (
                    "Metadata/project_settings.config",
                    br##"{
                      "printer_model":"Bambu Lab P2S",
                      "printer_settings_id":"Bambu Lab P2S 0.4 nozzle",
                      "nozzle_diameter":["0.4"],
                      "default_print_profile":"0.20mm Standard @BBL P2S",
                      "curr_bed_type":"Supertack Plate",
                      "sparse_infill_density":"22.5%",
                      "enable_support":"1",
                      "filament_settings_id":["Bambu PLA Basic @BBL P2S","Bambu PLA Matte @BBL P2S"],
                      "filament_type":["PLA","PLA"],
                      "filament_colour":["#FFFFFF","#E56B9F"]
                    }"##
                    .as_slice(),
                ),
                (
                    "Metadata/model_settings.config",
                    br#"<config><plate><metadata key="plater_id" value="1"/></plate></config>"#
                        .as_slice(),
                ),
            ] {
                archive.start_file(entry, options).unwrap();
                archive.write_all(contents).unwrap();
            }
            if sliced {
                archive
                    .start_file("Metadata/plate_1.gcode", options)
                    .unwrap();
                archive.write_all(b"M83\nG1 E1\n").unwrap();
                archive
                    .start_file("Metadata/slice_info.config", options)
                    .unwrap();
                archive.write_all(br##"<config><plate><metadata key="index" value="1"/><filament id="1"/><filament id="2"/></plate></config>"##).unwrap();
            }
            archive.finish().unwrap();
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn archive_contents_not_the_filename_determine_whether_it_is_sliced() {
        let fixture = Fixture::new();
        let project_with_sliced_name = fixture.archive("ordinary.gcode.3mf", false);
        let sliced_with_project_name = fixture.archive("already-sliced.3mf", true);

        assert_eq!(
            inspect_3mf_content(&project_with_sliced_name).unwrap().kind,
            ThreeMfKind::Unsliced
        );
        assert_eq!(
            inspect_3mf_content(&sliced_with_project_name).unwrap().kind,
            ThreeMfKind::Sliced
        );
    }

    #[test]
    fn reads_embedded_machine_process_tools_and_plate_information() {
        let fixture = Fixture::new();
        let path = fixture.archive("project.3mf", false);

        let inspection = inspect_3mf_content(&path).unwrap();

        assert_eq!(
            inspection.embedded_model_key.as_deref(),
            Some("Bambu Lab P2S")
        );
        assert_eq!(
            inspection.embedded_process_key.as_deref(),
            Some("0.20mm Standard @BBL P2S")
        );
        assert_eq!(
            inspection.embedded_plate_key.as_deref(),
            Some("Supertack Plate")
        );
        assert_eq!(inspection.embedded_infill_density, Some(22.5));
        assert_eq!(inspection.embedded_support_enabled, Some(true));
        assert_eq!(inspection.tools.len(), 2);
        assert_eq!(inspection.tools[1].tool, 1);
        assert_eq!(inspection.tools[1].color_hex.as_deref(), Some("#E56B9F"));
        assert_eq!(inspection.plates[0].plate_index, 1);
        assert_eq!(inspection.plates[0].tool_indices, vec![0, 1]);
    }

    #[test]
    fn rejects_non_zip_files_and_symbolic_links() {
        let fixture = Fixture::new();
        let invalid = fixture.root.join("invalid.3mf");
        fs::write(&invalid, b"not a zip").unwrap();
        assert!(inspect_3mf_content(&invalid).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let valid = fixture.archive("valid.3mf", false);
            let link = fixture.root.join("linked.3mf");
            symlink(&valid, &link).unwrap();
            assert!(inspect_3mf_content(&link).is_err());
        }
    }

    #[test]
    fn normalizes_bambu_project_local_filament_copies_to_official_keys() {
        assert_eq!(
            normalize_embedded_filament_key("Bambu PLA Basic @BBL P2S 0.4 nozzle-1(model.3mf)"),
            "Bambu PLA Basic @BBL P2S"
        );
        assert_eq!(
            normalize_embedded_filament_key("Bambu PLA Matte @BBL A1"),
            "Bambu PLA Matte @BBL A1"
        );
        assert_eq!(
            normalize_embedded_filament_key("Bambu PC FR @BBL P2S 0.4 nozzle"),
            "Bambu PC FR @BBL P2S 0.4 nozzle"
        );
    }

    #[test]
    fn rejects_an_unbounded_embedded_tool_array() {
        let settings = serde_json::json!({
            "filament_settings_id": (0..65).map(|index| format!("Preset {index}")).collect::<Vec<_>>()
        })
        .as_object()
        .unwrap()
        .clone();

        assert!(embedded_tools(Some(&settings), None).is_err());
    }

    #[test]
    #[ignore = "requires BAMBU_PROJECT_3MF and BAMBU_SLICED_3MF fixtures"]
    fn smoke_real_bambu_archives_are_classified_by_contents() {
        let project = PathBuf::from(std::env::var_os("BAMBU_PROJECT_3MF").unwrap());
        let sliced = PathBuf::from(std::env::var_os("BAMBU_SLICED_3MF").unwrap());

        let project = inspect_3mf_content(&project).unwrap();
        let sliced = inspect_3mf_content(&sliced).unwrap();

        assert_eq!(project.kind, ThreeMfKind::Unsliced);
        assert_eq!(sliced.kind, ThreeMfKind::Sliced);
        assert_eq!(project.embedded_model_key.as_deref(), Some("Bambu Lab P2S"));
        assert_eq!(project.tools.len(), 4);
        assert!(project.tools.iter().all(|tool| {
            tool.embedded_filament_key.as_deref() == Some("Bambu PLA Basic @BBL P2S")
        }));
        assert_eq!(project.plate_count, 1);
        assert_eq!(sliced.plate_count, 1);
    }

    #[test]
    #[ignore = "requires CYLUNE_MULTI_PLATE_3MF, CYLUNE_POOPY_BUCKET_3MF, and BAMBU_PROFILES_ROOT"]
    fn supplied_projects_are_inspectable_without_mutating_the_sources() {
        let multi_plate_path = PathBuf::from(
            std::env::var_os("CYLUNE_MULTI_PLATE_3MF").expect("CYLUNE_MULTI_PLATE_3MF is required"),
        );
        let poopy_path = PathBuf::from(
            std::env::var_os("CYLUNE_POOPY_BUCKET_3MF")
                .expect("CYLUNE_POOPY_BUCKET_3MF is required"),
        );
        let profiles_root = PathBuf::from(
            std::env::var_os("BAMBU_PROFILES_ROOT").expect("BAMBU_PROFILES_ROOT is required"),
        );
        let multi_plate_hash = sha256(&multi_plate_path).unwrap();
        let poopy_hash = sha256(&poopy_path).unwrap();
        assert_eq!(
            multi_plate_hash,
            "1ad2bde2a8c56455d5c5406fe7933e8a77be8adb2e1fd9dd9bb87f6c645c4e39"
        );
        assert_eq!(
            poopy_hash,
            "b3b1c78cacc6ec2aa71ced4539df19516cf36f2821348b140ca6a88f5aead8c0"
        );

        let multi_plate = inspect_3mf_content(&multi_plate_path).unwrap();
        assert_eq!(multi_plate.kind, ThreeMfKind::Unsliced);
        assert_eq!(multi_plate.plate_count, 4);
        assert_eq!(
            multi_plate.embedded_model_key.as_deref(),
            Some("Bambu Lab A1")
        );
        assert_eq!(multi_plate.embedded_nozzle_diameter, Some(0.4));
        assert_eq!(
            multi_plate.embedded_process_key.as_deref(),
            Some("0.20mm Standard @BBL A1")
        );
        assert_eq!(
            multi_plate.embedded_plate_key.as_deref(),
            Some("Cool Plate")
        );
        assert_eq!(multi_plate.embedded_infill_density, Some(10.0));
        assert_eq!(multi_plate.embedded_support_enabled, Some(true));
        assert_eq!(
            multi_plate
                .tools
                .iter()
                .map(|tool| (
                    tool.tool,
                    tool.label.as_str(),
                    tool.embedded_filament_key.as_deref(),
                    tool.material.as_deref(),
                    tool.color_hex.as_deref(),
                ))
                .collect::<Vec<_>>(),
            vec![(
                0,
                "PLA碳灰0.95",
                Some("PLA碳灰0.95"),
                Some("PLA"),
                Some("#A7A9AA")
            )]
        );
        assert_eq!(
            multi_plate
                .plates
                .iter()
                .map(|plate| plate.plate_index)
                .collect::<Vec<_>>(),
            (1..=4).collect::<Vec<_>>()
        );

        let poopy = inspect_3mf_content(&poopy_path).unwrap();
        assert_eq!(poopy.kind, ThreeMfKind::Unsliced);
        assert_eq!(poopy.plate_count, 1);
        assert_eq!(poopy.embedded_model_key.as_deref(), Some("Bambu Lab X2D"));
        assert_eq!(poopy.embedded_nozzle_diameter, Some(0.4));
        assert_eq!(
            poopy.embedded_process_key.as_deref(),
            Some("0.20mm Standard @BBL X2D")
        );
        assert_eq!(
            poopy.embedded_plate_key.as_deref(),
            Some("Textured PEI Plate")
        );
        assert_eq!(poopy.embedded_infill_density, Some(15.0));
        assert_eq!(poopy.embedded_support_enabled, Some(false));
        assert_eq!(poopy.tools.len(), 1);
        assert_eq!(poopy.tools[0].label, "Bambu PLA Basic @BBL X2D 0.4 nozzle");
        assert_eq!(
            poopy.tools[0].embedded_filament_key.as_deref(),
            Some("Bambu PLA Basic @BBL X2D 0.4 nozzle")
        );
        assert_eq!(poopy.tools[0].material.as_deref(), Some("PLA"));
        assert_eq!(poopy.tools[0].color_hex.as_deref(), Some("#F5547C"));

        for inspection in [&multi_plate, &poopy] {
            let printer = SavedPrinter {
                printer_id: Uuid::new_v4().to_string(),
                display_name: inspection.embedded_model_key.clone().unwrap(),
                model_key: inspection.embedded_model_key.clone().unwrap(),
                nozzle_diameter: inspection.embedded_nozzle_diameter.unwrap(),
                default_plate: inspection.embedded_plate_key.clone().unwrap(),
                ams_kind: "none".to_owned(),
                is_default: false,
                is_available: true,
            };
            let catalog = load_slice_preset_catalog(&profiles_root, &printer).unwrap();
            assert!(catalog.processes.iter().any(|preset| {
                Some(preset.key.as_str()) == inspection.embedded_process_key.as_deref()
            }));
            assert!(catalog.plates.iter().any(|plate| {
                Some(plate.key.as_str()) == inspection.embedded_plate_key.as_deref()
            }));
            assert!(inspection.tools.iter().all(|tool| {
                catalog.filaments.iter().any(|preset| {
                    Some(preset.key.as_str()) == tool.embedded_filament_key.as_deref()
                        || tool.material.as_deref() == Some(preset.material.as_str())
                })
            }));
        }

        assert_eq!(sha256(&multi_plate_path).unwrap(), multi_plate_hash);
        assert_eq!(sha256(&poopy_path).unwrap(), poopy_hash);
    }
}
