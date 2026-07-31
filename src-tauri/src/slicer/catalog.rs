use crate::{
    error::{AppError, Result},
    printers::SavedPrinter,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

const MAX_PROFILE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlicePresetOption {
    pub key: String,
    pub label: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceProcessPreset {
    pub key: String,
    pub label: String,
    pub layer_height_mm: f64,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceFilamentPreset {
    pub key: String,
    pub label: String,
    pub material: String,
    pub color_hex: Option<String>,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlicePresetCatalog {
    pub processes: Vec<SliceProcessPreset>,
    pub filaments: Vec<SliceFilamentPreset>,
    pub plates: Vec<SlicePresetOption>,
}

pub(crate) fn load_slice_preset_catalog(
    profiles_root: &Path,
    printer: &SavedPrinter,
) -> Result<SlicePresetCatalog> {
    Ok(load_catalog_data(profiles_root, printer)?.catalog)
}

pub(crate) fn resolve_machine_path(
    profiles_root: &Path,
    printer: &SavedPrinter,
) -> Result<PathBuf> {
    Ok(load_catalog_data(profiles_root, printer)?.machine_path)
}

struct CatalogData {
    catalog: SlicePresetCatalog,
    machine_path: PathBuf,
}

struct CatalogPath {
    supported: bool,
    layer_height_mm: Option<f64>,
    material: Option<String>,
    color_hex: Option<String>,
}

struct ProfileDocument {
    path: PathBuf,
    raw: Map<String, Value>,
}

struct ProfileCategory {
    documents: BTreeMap<String, ProfileDocument>,
    directory: PathBuf,
}

fn load_catalog_data(profiles_root: &Path, printer: &SavedPrinter) -> Result<CatalogData> {
    if !printer.nozzle_diameter.is_finite() || printer.nozzle_diameter <= 0.0 {
        return Err(AppError::SlicerIncompatible);
    }
    let machines = load_category(profiles_root, "machine")?;
    let processes = load_category(profiles_root, "process")?;
    let filaments = load_category(profiles_root, "filament")?;
    let machine_resolved = resolve_documents(&machines.documents)?;

    let matching_machines = machines
        .documents
        .iter()
        .filter(|(_, document)| {
            document.raw.get("type").and_then(Value::as_str) == Some("machine")
                && json_true(document.raw.get("instantiation"))
        })
        .filter_map(|(name, document)| {
            let resolved = machine_resolved.get(name)?;
            let model_matches = resolved.get("printer_model").and_then(Value::as_str)
                == Some(printer.model_key.as_str());
            let nozzle_matches = json_strings(resolved.get("nozzle_diameter"))
                .into_iter()
                .filter_map(|value| value.parse::<f64>().ok())
                .any(|value| (value - printer.nozzle_diameter).abs() < 0.000_001);
            (model_matches && nozzle_matches).then_some((name, document, resolved))
        })
        .collect::<Vec<_>>();
    let [(machine_name, machine_document, machine_profile)] = matching_machines.as_slice() else {
        return Err(AppError::SlicerIncompatible);
    };

    let process_entries = catalog_entries(&processes, "process", machine_name)?;
    let filament_entries = catalog_entries(&filaments, "filament", machine_name)?;
    let default_process_key = machine_profile
        .get("default_print_profile")
        .and_then(Value::as_str)
        .filter(|key| process_entries.contains_key(*key))
        .map(str::to_owned);
    let default_filament_key = json_strings(machine_profile.get("default_filament_profile"))
        .into_iter()
        .find(|key| filament_entries.contains_key(key));

    let process_options = process_options(&process_entries, default_process_key.as_deref())?;
    let filament_options = filament_options(&filament_entries, default_filament_key.as_deref());
    let plates = plate_options(&machines, &filaments, printer)?;
    Ok(CatalogData {
        catalog: SlicePresetCatalog {
            processes: process_options,
            filaments: filament_options,
            plates,
        },
        machine_path: machine_document.path.clone(),
    })
}

fn load_category(profiles_root: &Path, category: &str) -> Result<ProfileCategory> {
    let root = safe_directory(profiles_root, AppError::SlicerProfilesMissing)?;
    let bbl = safe_child_directory(&root, "BBL")?;
    let directory = safe_child_directory(&bbl, category)?;
    let mut paths = fs::read_dir(&directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let mut documents = BTreeMap::new();
    for path in paths {
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| AppError::SlicerIncompatible)?;
        if metadata.file_type().is_symlink()
            || !metadata.file_type().is_file()
            || metadata.len() > MAX_PROFILE_BYTES
        {
            return Err(AppError::SlicerIncompatible);
        }
        let canonical = fs::canonicalize(&path).map_err(|_| AppError::SlicerIncompatible)?;
        if !canonical.starts_with(&directory) {
            return Err(AppError::SlicerIncompatible);
        }
        let raw = serde_json::from_slice::<Value>(&fs::read(&canonical)?)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .ok_or(AppError::SlicerIncompatible)?;
        let Some(name) = raw.get("name").and_then(Value::as_str) else {
            if raw.get("type").is_some() || raw.get("instantiation").is_some() {
                return Err(AppError::SlicerIncompatible);
            }
            continue;
        };
        if name.trim().is_empty()
            || documents
                .insert(
                    name.to_owned(),
                    ProfileDocument {
                        path: canonical,
                        raw,
                    },
                )
                .is_some()
        {
            return Err(AppError::SlicerIncompatible);
        }
    }
    Ok(ProfileCategory {
        documents,
        directory,
    })
}

fn safe_directory(path: &Path, error: AppError) -> Result<PathBuf> {
    let metadata = fs::symlink_metadata(path).map_err(|_| error)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(AppError::SlicerProfilesMissing);
    }
    fs::canonicalize(path).map_err(|_| AppError::SlicerProfilesMissing)
}

fn safe_child_directory(parent: &Path, child: &str) -> Result<PathBuf> {
    let path = parent.join(child);
    let metadata = fs::symlink_metadata(&path).map_err(|_| AppError::SlicerProfilesMissing)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(AppError::SlicerProfilesMissing);
    }
    let canonical = fs::canonicalize(path).map_err(|_| AppError::SlicerProfilesMissing)?;
    if !canonical.starts_with(parent) {
        return Err(AppError::SlicerProfilesMissing);
    }
    Ok(canonical)
}

fn resolve_documents(
    documents: &BTreeMap<String, ProfileDocument>,
) -> Result<HashMap<String, Map<String, Value>>> {
    let mut resolved = HashMap::new();
    for name in documents.keys() {
        resolve_document(name, documents, &mut resolved, &mut HashSet::new())?;
    }
    Ok(resolved)
}

fn resolve_document(
    name: &str,
    documents: &BTreeMap<String, ProfileDocument>,
    cache: &mut HashMap<String, Map<String, Value>>,
    visiting: &mut HashSet<String>,
) -> Result<Map<String, Value>> {
    if let Some(cached) = cache.get(name) {
        return Ok(cached.clone());
    }
    if !visiting.insert(name.to_owned()) {
        return Err(AppError::SlicerIncompatible);
    }
    let document = documents.get(name).ok_or(AppError::SlicerIncompatible)?;
    let mut merged = if let Some(parent) = document.raw.get("inherits").and_then(Value::as_str) {
        resolve_document(parent, documents, cache, visiting)?
    } else {
        Map::new()
    };
    for (key, value) in &document.raw {
        merged.insert(key.clone(), value.clone());
    }
    visiting.remove(name);
    cache.insert(name.to_owned(), merged.clone());
    Ok(merged)
}

fn catalog_entries(
    category: &ProfileCategory,
    expected_type: &str,
    machine_name: &str,
) -> Result<BTreeMap<String, CatalogPath>> {
    let resolved = resolve_documents(&category.documents)?;
    Ok(category
        .documents
        .iter()
        .filter(|(_, document)| {
            document.raw.get("type").and_then(Value::as_str) == Some(expected_type)
                && json_true(document.raw.get("instantiation"))
        })
        .map(|(name, _document)| {
            let supported = resolved.get(name).is_some_and(|profile| {
                json_strings(profile.get("compatible_printers"))
                    .iter()
                    .any(|machine| machine == machine_name)
            });
            (
                name.clone(),
                CatalogPath {
                    supported,
                    layer_height_mm: resolved.get(name).and_then(|profile| {
                        json_strings(profile.get("layer_height"))
                            .into_iter()
                            .next()
                            .and_then(|value| value.parse::<f64>().ok())
                            .filter(|value| value.is_finite() && *value > 0.0)
                    }),
                    material: resolved.get(name).and_then(|profile| {
                        json_strings(profile.get("filament_type"))
                            .into_iter()
                            .next()
                    }),
                    color_hex: resolved.get(name).and_then(|profile| {
                        json_strings(profile.get("filament_colour"))
                            .into_iter()
                            .next()
                    }),
                },
            )
        })
        .collect())
}

fn process_options(
    entries: &BTreeMap<String, CatalogPath>,
    default_key: Option<&str>,
) -> Result<Vec<SliceProcessPreset>> {
    entries
        .iter()
        .filter(|(_, entry)| entry.supported)
        .map(|(name, entry)| {
            Ok(SliceProcessPreset {
                key: name.clone(),
                label: name.clone(),
                layer_height_mm: entry.layer_height_mm.ok_or(AppError::SlicerIncompatible)?,
                is_default: default_key == Some(name.as_str()),
            })
        })
        .collect()
}

fn filament_options(
    entries: &BTreeMap<String, CatalogPath>,
    default_key: Option<&str>,
) -> Vec<SliceFilamentPreset> {
    let mut options = entries
        .iter()
        .filter(|(_, entry)| entry.supported)
        .map(|(name, entry)| SliceFilamentPreset {
            key: name.clone(),
            label: name.clone(),
            material: entry.material.clone().unwrap_or_default(),
            color_hex: entry.color_hex.clone(),
            is_default: default_key == Some(name.as_str()),
        })
        .collect::<Vec<_>>();
    options.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.label.cmp(&right.label))
    });
    options
}

fn plate_options(
    machines: &ProfileCategory,
    filaments: &ProfileCategory,
    printer: &SavedPrinter,
) -> Result<Vec<SlicePresetOption>> {
    let common_path = filaments.directory.join("fdm_filament_common.json");
    let metadata =
        fs::symlink_metadata(&common_path).map_err(|_| AppError::SlicerProfilesMissing)?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_file()
        || metadata.len() > MAX_PROFILE_BYTES
    {
        return Err(AppError::SlicerIncompatible);
    }
    let common = serde_json::from_slice::<Value>(&fs::read(&common_path)?)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or(AppError::SlicerIncompatible)?;
    let excluded = machines
        .documents
        .get(&printer.model_key)
        .map(|document| {
            json_strings(document.raw.get("not_support_bed_type"))
                .into_iter()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let mappings = [
        ("cool_plate_temp", "Cool Plate"),
        ("eng_plate_temp", "Engineering Plate"),
        ("hot_plate_temp", "Smooth PEI Plate / High Temp Plate"),
        ("supertack_plate_temp", "Supertack Plate"),
        ("textured_plate_temp", "Textured PEI Plate"),
    ];
    let mut options = mappings
        .into_iter()
        .filter(|(setting, _)| common.contains_key(*setting))
        .map(|(_, plate)| SlicePresetOption {
            key: plate.to_owned(),
            label: plate.to_owned(),
            is_default: plate == printer.default_plate,
        })
        .filter(|option| !excluded.contains(&option.key))
        .collect::<Vec<_>>();
    if !options
        .iter()
        .any(|option| option.key == printer.default_plate)
    {
        return Err(AppError::SlicerIncompatible);
    }
    options.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(options)
}

fn json_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(value)) => value
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .flat_map(|value| value.split(';'))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn json_true(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(true))) || value.and_then(Value::as_str) == Some("true")
}

#[cfg(test)]
mod tests {
    use super::{load_slice_preset_catalog, resolve_machine_path};
    use crate::printers::SavedPrinter;
    use std::{fs, path::PathBuf};
    use uuid::Uuid;

    struct Profiles {
        root: PathBuf,
    }

    impl Profiles {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("cylune-slice-catalog-{}", Uuid::new_v4()));
            for category in ["machine", "process", "filament"] {
                fs::create_dir_all(root.join("BBL").join(category)).unwrap();
            }
            let profiles = Self { root };
            profiles.write(
                "machine",
                "Bambu Lab P2S",
                r#"{"type":"machine_model","name":"Bambu Lab P2S","default_bed_type":"Supertack Plate","not_support_bed_type":"Cool Plate"}"#,
            );
            profiles.write(
                "machine",
                "machine-base",
                r#"{"type":"machine","name":"machine-base","instantiation":"false"}"#,
            );
            profiles.write(
                "machine",
                "Bambu Lab P2S 0.4 nozzle",
                r#"{
                  "type":"machine","name":"Bambu Lab P2S 0.4 nozzle","inherits":"machine-base",
                  "instantiation":"true","printer_model":"Bambu Lab P2S","nozzle_diameter":["0.4"],
                  "default_print_profile":"0.20mm Standard @BBL P2S",
                  "default_filament_profile":["Bambu PLA Basic @BBL P2S"]
                }"#,
            );
            profiles.write(
                "process",
                "process-base",
                r#"{"type":"process","name":"process-base","instantiation":"false","layer_height":"0.2"}"#,
            );
            profiles.write(
                "process",
                "0.20mm Standard @BBL P2S",
                r#"{
                  "type":"process","name":"0.20mm Standard @BBL P2S","inherits":"process-base",
                  "instantiation":"true","compatible_printers":["Bambu Lab P2S 0.4 nozzle"]
                }"#,
            );
            profiles.write(
                "process",
                "A1 only process",
                r#"{"type":"process","name":"A1 only process","instantiation":"true","compatible_printers":["Bambu Lab A1 0.4 nozzle"]}"#,
            );
            profiles.write(
                "filament",
                "Bambu PLA Basic @base",
                r##"{"type":"filament","name":"Bambu PLA Basic @base","instantiation":"false","filament_type":["PLA"],"filament_colour":["#FFFFFF"]}"##,
            );
            profiles.write(
                "filament",
                "Bambu PLA Basic @BBL P2S",
                r#"{
                  "type":"filament","name":"Bambu PLA Basic @BBL P2S","inherits":"Bambu PLA Basic @base",
                  "instantiation":"true","compatible_printers":["Bambu Lab P2S 0.4 nozzle"]
                }"#,
            );
            profiles.write(
                "filament",
                "A1 only PLA",
                r#"{"type":"filament","name":"A1 only PLA","instantiation":"true","compatible_printers":["Bambu Lab A1 0.4 nozzle"]}"#,
            );
            profiles.write(
                "filament",
                "fdm_filament_common",
                r#"{"cool_plate_temp":["35"],"supertack_plate_temp":["45"],"textured_plate_temp":["55"]}"#,
            );
            profiles
        }

        fn write(&self, category: &str, name: &str, json: &str) {
            fs::write(
                self.root
                    .join("BBL")
                    .join(category)
                    .join(format!("{name}.json")),
                json,
            )
            .unwrap();
        }
    }

    impl Drop for Profiles {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn printer() -> SavedPrinter {
        SavedPrinter {
            printer_id: Uuid::new_v4().to_string(),
            display_name: "My P2S".to_owned(),
            model_key: "Bambu Lab P2S".to_owned(),
            nozzle_diameter: 0.4,
            default_plate: "Supertack Plate".to_owned(),
            ams_kind: "ams".to_owned(),
            is_default: true,
            is_available: true,
        }
    }

    #[test]
    fn returns_stable_names_and_support_information_for_official_presets() {
        let profiles = Profiles::new();

        let catalog = load_slice_preset_catalog(&profiles.root, &printer()).unwrap();

        let process = catalog
            .processes
            .iter()
            .find(|item| item.key == "0.20mm Standard @BBL P2S")
            .unwrap();
        assert_eq!(process.label, process.key);
        assert_eq!(process.layer_height_mm, 0.2);
        assert!(process.is_default);
        assert!(!catalog
            .processes
            .iter()
            .any(|item| item.key == "A1 only process"));
        let filament = catalog
            .filaments
            .iter()
            .find(|item| item.key == "Bambu PLA Basic @BBL P2S")
            .unwrap();
        assert_eq!(filament.material, "PLA");
        assert_eq!(filament.color_hex.as_deref(), Some("#FFFFFF"));
        assert!(filament.is_default);
        assert!(catalog.filaments.first().unwrap().is_default);
        assert!(!catalog
            .filaments
            .iter()
            .any(|item| item.key == "A1 only PLA"));
        assert!(!catalog.plates.iter().any(|item| item.key == "Cool Plate"));
        assert!(
            catalog
                .plates
                .iter()
                .find(|item| item.key == "Supertack Plate")
                .unwrap()
                .is_default
        );
    }

    #[test]
    fn resolves_only_the_exact_saved_printer_machine_to_an_internal_canonical_path() {
        let profiles = Profiles::new();

        let resolved = resolve_machine_path(&profiles.root, &printer()).unwrap();

        assert!(resolved.is_absolute());
        assert_eq!(resolved, fs::canonicalize(&resolved).unwrap());
        assert!(resolved.ends_with("BBL/machine/Bambu Lab P2S 0.4 nozzle.json"));

        let mut incompatible = printer();
        incompatible.nozzle_diameter = 0.6;
        assert!(resolve_machine_path(&profiles.root, &incompatible).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_profile_symlinks_even_when_they_point_outside_the_official_root() {
        use std::os::unix::fs::symlink;

        let profiles = Profiles::new();
        let outside = profiles.root.join("outside.json");
        fs::write(
            &outside,
            r#"{"type":"process","name":"escaped","instantiation":"true"}"#,
        )
        .unwrap();
        symlink(&outside, profiles.root.join("BBL/process/escaped.json")).unwrap();

        assert!(load_slice_preset_catalog(&profiles.root, &printer()).is_err());
    }

    #[test]
    #[ignore = "requires BAMBU_PROFILES_ROOT to point to installed Bambu Studio profiles"]
    fn smoke_installed_profiles_expose_p2s_fast_presets() {
        let root = std::env::var_os("BAMBU_PROFILES_ROOT")
            .map(PathBuf::from)
            .expect("BAMBU_PROFILES_ROOT is required");

        let catalog = load_slice_preset_catalog(&root, &printer()).unwrap();

        assert!(catalog
            .processes
            .iter()
            .any(|preset| preset.key == "0.20mm Standard @BBL P2S"));
        assert!(catalog
            .filaments
            .iter()
            .any(|preset| preset.key == "Bambu PLA Basic @BBL P2S"));
        assert!(catalog
            .plates
            .iter()
            .any(|plate| plate.key == "Supertack Plate"));

        let available = crate::printers::load_printer_profiles(&root).unwrap();
        let a1 = available
            .iter()
            .find(|profile| profile.model_key == "Bambu Lab A1")
            .unwrap();
        let a1_printer = SavedPrinter {
            printer_id: Uuid::new_v4().to_string(),
            display_name: "A1".to_owned(),
            model_key: a1.model_key.clone(),
            nozzle_diameter: 0.4,
            default_plate: a1
                .plate_keys
                .iter()
                .find(|plate| plate.as_str() == "Supertack Plate")
                .cloned()
                .unwrap_or_else(|| a1.plate_keys[0].clone()),
            ams_kind: "ams_lite".to_owned(),
            is_default: false,
            is_available: true,
        };
        let a1_catalog = load_slice_preset_catalog(&root, &a1_printer).unwrap();
        assert!(!a1_catalog.processes.is_empty());
        assert!(!a1_catalog.filaments.is_empty());
    }
}
