use crate::{
    db::AppDatabase,
    error::{AppError, Result},
    slicer::InstallationDiscovery,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrinterProfile {
    pub model_key: String,
    pub display_name: String,
    pub nozzle_diameters: Vec<f64>,
    pub plate_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedPrinter {
    pub printer_id: String,
    pub display_name: String,
    pub model_key: String,
    pub nozzle_diameter: f64,
    pub default_plate: String,
    pub ams_kind: String,
    pub is_default: bool,
    pub is_available: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavePrinter {
    #[serde(default)]
    pub printer_id: Option<String>,
    pub display_name: String,
    pub model_key: String,
    pub nozzle_diameter: f64,
    pub default_plate: String,
    pub ams_kind: String,
    #[serde(default)]
    pub is_default: bool,
}

pub struct PrinterService {
    database: AppDatabase,
}

pub type PrinterState = Mutex<PrinterService>;

impl PrinterService {
    pub fn new(database: AppDatabase) -> Self {
        Self { database }
    }

    pub fn save(&mut self, printer: SavePrinter) -> Result<SavedPrinter> {
        validate_saved_input(&printer)?;
        let printer_id = match &printer.printer_id {
            Some(value) => Uuid::parse_str(value).map_err(|_| AppError::InvalidFile)?,
            None => Uuid::new_v4(),
        };
        let transaction = self.database.connection.transaction()?;
        if printer.printer_id.is_some() {
            let exists = transaction
                .query_row(
                    "SELECT 1 FROM printers WHERE printer_id = ?1",
                    [printer_id.to_string()],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !exists {
                return Err(AppError::InvalidFile);
            }
        }
        if printer.is_default {
            transaction.execute(
                "UPDATE printers SET is_default = 0,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE is_default = 1",
                [],
            )?;
        }
        if printer.printer_id.is_some() {
            transaction.execute(
                "UPDATE printers SET
                    display_name = ?2, model_key = ?3, nozzle_diameter = ?4,
                    default_plate = ?5, ams_kind = ?6, is_default = ?7,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE printer_id = ?1",
                params![
                    printer_id.to_string(),
                    printer.display_name.trim(),
                    printer.model_key.trim(),
                    printer.nozzle_diameter,
                    printer.default_plate.trim(),
                    printer.ams_kind.trim(),
                    printer.is_default,
                ],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO printers (
                    printer_id, display_name, model_key, nozzle_diameter,
                    default_plate, ams_kind, is_default
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    printer_id.to_string(),
                    printer.display_name.trim(),
                    printer.model_key.trim(),
                    printer.nozzle_diameter,
                    printer.default_plate.trim(),
                    printer.ams_kind.trim(),
                    printer.is_default,
                ],
            )?;
        }
        transaction.commit()?;
        self.get(printer_id, &[])
    }

    pub fn list_saved(&self, catalog: &[PrinterProfile]) -> Result<Vec<SavedPrinter>> {
        let mut statement = self.database.connection.prepare(
            "SELECT printer_id, display_name, model_key, nozzle_diameter,
                    default_plate, ams_kind, is_default
             FROM printers
             ORDER BY is_default DESC, created_at, printer_id",
        )?;
        let printers = statement
            .query_map([], |row| {
                Ok(SavedPrinter {
                    printer_id: row.get(0)?,
                    display_name: row.get(1)?,
                    model_key: row.get(2)?,
                    nozzle_diameter: row.get(3)?,
                    default_plate: row.get(4)?,
                    ams_kind: row.get(5)?,
                    is_default: row.get::<_, i64>(6)? != 0,
                    is_available: false,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(printers
            .into_iter()
            .map(|mut printer| {
                printer.is_available = printer_available(&printer, catalog);
                printer
            })
            .collect())
    }

    pub fn delete(&mut self, printer_id: Uuid) -> Result<()> {
        let changed = self.database.connection.execute(
            "DELETE FROM printers WHERE printer_id = ?1",
            [printer_id.to_string()],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidFile);
        }
        Ok(())
    }

    pub fn set_default(&mut self, printer_id: Uuid) -> Result<()> {
        let transaction = self.database.connection.transaction()?;
        let exists = transaction
            .query_row(
                "SELECT 1 FROM printers WHERE printer_id = ?1",
                [printer_id.to_string()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if !exists {
            return Err(AppError::InvalidFile);
        }
        transaction.execute(
            "UPDATE printers SET is_default = 0,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE is_default = 1",
            [],
        )?;
        let changed = transaction.execute(
            "UPDATE printers SET is_default = 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE printer_id = ?1",
            [printer_id.to_string()],
        )?;
        if changed != 1 {
            return Err(AppError::InvalidFile);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn into_database(self) -> AppDatabase {
        self.database
    }

    fn get(&self, printer_id: Uuid, catalog: &[PrinterProfile]) -> Result<SavedPrinter> {
        let mut printer = self.database.connection.query_row(
            "SELECT printer_id, display_name, model_key, nozzle_diameter,
                    default_plate, ams_kind, is_default
             FROM printers WHERE printer_id = ?1",
            [printer_id.to_string()],
            |row| {
                Ok(SavedPrinter {
                    printer_id: row.get(0)?,
                    display_name: row.get(1)?,
                    model_key: row.get(2)?,
                    nozzle_diameter: row.get(3)?,
                    default_plate: row.get(4)?,
                    ams_kind: row.get(5)?,
                    is_default: row.get::<_, i64>(6)? != 0,
                    is_available: false,
                })
            },
        )?;
        printer.is_available = printer_available(&printer, catalog);
        Ok(printer)
    }
}

fn validate_saved_input(printer: &SavePrinter) -> Result<()> {
    let valid_text = |value: &str, maximum: usize| {
        let trimmed = value.trim();
        !trimmed.is_empty()
            && trimmed.chars().count() <= maximum
            && !trimmed.chars().any(char::is_control)
    };
    if !valid_text(&printer.display_name, 80)
        || !valid_text(&printer.model_key, 160)
        || !valid_text(&printer.default_plate, 120)
        || !valid_text(&printer.ams_kind, 80)
        || !printer.nozzle_diameter.is_finite()
        || !(0.0..=2.0).contains(&printer.nozzle_diameter)
        || printer.nozzle_diameter == 0.0
    {
        return Err(AppError::InvalidFile);
    }
    Ok(())
}

fn printer_available(printer: &SavedPrinter, catalog: &[PrinterProfile]) -> bool {
    catalog.iter().any(|profile| {
        profile.model_key == printer.model_key
            && profile
                .nozzle_diameters
                .iter()
                .any(|diameter| (*diameter - printer.nozzle_diameter).abs() < f64::EPSILON)
            && profile.plate_keys.contains(&printer.default_plate)
    })
}

pub fn load_printer_profiles(profiles_root: &Path) -> Result<Vec<PrinterProfile>> {
    let machine_root = profiles_root.join("BBL/machine");
    let documents = read_machine_documents(&machine_root)?;
    let official_plates = read_official_plate_keys(profiles_root)?;
    let mut resolved = HashMap::new();
    let mut catalog: BTreeMap<String, (BTreeSet<String>, Vec<f64>)> = BTreeMap::new();

    for (name, raw) in &documents {
        if raw.get("type").and_then(Value::as_str) != Some("machine")
            || !json_true(raw.get("instantiation"))
        {
            continue;
        }
        let profile = resolve_profile(name, &documents, &mut resolved, &mut HashSet::new())?;
        let Some(model_key) = profile
            .get("printer_model")
            .and_then(Value::as_str)
            .filter(|model| model.starts_with("Bambu Lab "))
        else {
            continue;
        };
        let nozzle_diameters = json_strings(profile.get("nozzle_diameter"))
            .into_iter()
            .map(|diameter| {
                diameter
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .ok_or(AppError::SlicerIncompatible)
            })
            .collect::<Result<Vec<_>>>()?;
        if nozzle_diameters.is_empty() {
            return Err(AppError::SlicerIncompatible);
        }

        let mut plate_keys = explicit_plate_keys(&profile);
        if plate_keys.is_empty() {
            plate_keys.extend(official_plates.iter().cloned());
            if let Some(model) = documents.get(model_key) {
                plate_keys.extend(explicit_plate_keys(model));
                if let Some(default_plate) = model.get("default_bed_type").and_then(Value::as_str) {
                    plate_keys.insert(default_plate.to_owned());
                }
                for unsupported in json_strings(model.get("not_support_bed_type")) {
                    plate_keys.remove(&unsupported);
                }
            }
        }
        if plate_keys.is_empty() {
            return Err(AppError::SlicerIncompatible);
        }

        let entry = catalog.entry(model_key.to_owned()).or_default();
        entry.0.extend(plate_keys);
        entry.1.extend(nozzle_diameters);
    }

    Ok(catalog
        .into_iter()
        .map(|(model_key, (plate_keys, mut nozzle_diameters))| {
            nozzle_diameters.sort_by(f64::total_cmp);
            nozzle_diameters.dedup_by(|left, right| (*left - *right).abs() < f64::EPSILON);
            PrinterProfile {
                display_name: model_key.clone(),
                model_key,
                nozzle_diameters,
                plate_keys: plate_keys.into_iter().collect(),
            }
        })
        .collect())
}

fn read_machine_documents(root: &Path) -> Result<HashMap<String, Map<String, Value>>> {
    if !regular_directory(root) {
        return Err(AppError::SlicerProfilesMissing);
    }
    let mut paths = fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let mut documents = HashMap::new();
    for path in paths {
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        if !regular_file(&path) {
            return Err(AppError::SlicerIncompatible);
        }
        let document = read_object(&path)?;
        let name = document
            .get("name")
            .and_then(Value::as_str)
            .ok_or(AppError::SlicerIncompatible)?
            .to_owned();
        if documents.insert(name, document).is_some() {
            return Err(AppError::SlicerIncompatible);
        }
    }
    Ok(documents)
}

fn read_object(path: &Path) -> Result<Map<String, Value>> {
    let bytes = fs::read(path)?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or(AppError::SlicerIncompatible)
}

fn resolve_profile(
    name: &str,
    documents: &HashMap<String, Map<String, Value>>,
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
    let mut merged = if let Some(parent) = document.get("inherits").and_then(Value::as_str) {
        resolve_profile(parent, documents, cache, visiting)?
    } else {
        Map::new()
    };
    for (key, value) in document {
        merged.insert(key.clone(), value.clone());
    }
    visiting.remove(name);
    cache.insert(name.to_owned(), merged.clone());
    Ok(merged)
}

fn read_official_plate_keys(profiles_root: &Path) -> Result<BTreeSet<String>> {
    let path = profiles_root.join("BBL/filament/fdm_filament_common.json");
    if !path.exists() {
        return Ok(BTreeSet::new());
    }
    if !regular_file(&path) {
        return Err(AppError::SlicerIncompatible);
    }
    let document = read_object(&path)?;
    let mappings = [
        ("cool_plate_temp", "Cool Plate"),
        ("eng_plate_temp", "Engineering Plate"),
        ("hot_plate_temp", "Smooth PEI Plate / High Temp Plate"),
        ("supertack_plate_temp", "Supertack Plate"),
        ("textured_plate_temp", "Textured PEI Plate"),
    ];
    Ok(mappings
        .into_iter()
        .filter(|(setting, _)| document.contains_key(*setting))
        .map(|(_, plate)| plate.to_owned())
        .collect())
}

fn explicit_plate_keys(document: &Map<String, Value>) -> BTreeSet<String> {
    ["plate_keys", "supported_bed_types", "bed_type"]
        .into_iter()
        .flat_map(|key| json_strings(document.get(key)))
        .collect()
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

fn regular_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_dir() && !metadata.file_type().is_symlink())
}

fn regular_file(path: &PathBuf) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_file() && !metadata.file_type().is_symlink())
}

fn discover_catalog(app_path: Option<String>) -> Result<Vec<PrinterProfile>> {
    let explicit = app_path.map(PathBuf::from);
    let installation = InstallationDiscovery::new(explicit).discover()?;
    load_printer_profiles(&installation.profiles_root)
}

#[tauri::command]
pub fn list_available_printers(app_path: Option<String>) -> Result<Vec<PrinterProfile>> {
    discover_catalog(app_path)
}

#[tauri::command]
pub fn list_saved_printers(
    app_path: Option<String>,
    state: tauri::State<'_, PrinterState>,
) -> Result<Vec<SavedPrinter>> {
    let catalog = discover_catalog(app_path).unwrap_or_default();
    state
        .lock()
        .map_err(|_| AppError::Database("printer lock poisoned".into()))?
        .list_saved(&catalog)
}

#[tauri::command]
pub fn save_printer(
    printer: SavePrinter,
    state: tauri::State<'_, PrinterState>,
) -> Result<SavedPrinter> {
    state
        .lock()
        .map_err(|_| AppError::Database("printer lock poisoned".into()))?
        .save(printer)
}

#[tauri::command]
pub fn delete_printer(printer_id: String, state: tauri::State<'_, PrinterState>) -> Result<()> {
    let printer_id = Uuid::parse_str(&printer_id).map_err(|_| AppError::InvalidFile)?;
    state
        .lock()
        .map_err(|_| AppError::Database("printer lock poisoned".into()))?
        .delete(printer_id)
}

#[tauri::command]
pub fn set_default_printer(
    printer_id: String,
    state: tauri::State<'_, PrinterState>,
) -> Result<()> {
    let printer_id = Uuid::parse_str(&printer_id).map_err(|_| AppError::InvalidFile)?;
    state
        .lock()
        .map_err(|_| AppError::Database("printer lock poisoned".into()))?
        .set_default(printer_id)
}

#[cfg(test)]
mod tests {
    use super::{load_printer_profiles, PrinterProfile, PrinterService, SavePrinter};
    use crate::db::AppDatabase;
    use std::{fs, path::PathBuf};
    use uuid::Uuid;

    struct ProfileTree {
        root: PathBuf,
    }

    impl ProfileTree {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("cylune-printer-profiles-{}", Uuid::new_v4()));
            fs::create_dir_all(root.join("BBL/machine")).unwrap();
            fs::create_dir_all(root.join("BBL/filament")).unwrap();
            Self { root }
        }

        fn machine(&self, name: &str, json: &str) {
            fs::write(
                self.root.join("BBL/machine").join(format!("{name}.json")),
                json,
            )
            .unwrap();
        }

        fn filament(&self, name: &str, json: &str) {
            fs::write(
                self.root.join("BBL/filament").join(format!("{name}.json")),
                json,
            )
            .unwrap();
        }
    }

    impl Drop for ProfileTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn resolves_inherited_p2s_nozzle_and_supported_plates() {
        let profiles = ProfileTree::new();
        profiles.machine(
            "base",
            r#"{
                "type":"machine",
                "name":"base",
                "instantiation":"false",
                "plate_keys":["Textured PEI Plate","Supertack Plate"]
            }"#,
        );
        profiles.machine(
            "Bambu Lab P2S 0.4 nozzle",
            r#"{
                "type":"machine",
                "name":"Bambu Lab P2S 0.4 nozzle",
                "inherits":"base",
                "instantiation":"true",
                "printer_model":"Bambu Lab P2S",
                "nozzle_diameter":["0.4"]
            }"#,
        );

        let catalog = load_printer_profiles(&profiles.root).unwrap();

        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].model_key, "Bambu Lab P2S");
        assert_eq!(catalog[0].display_name, "Bambu Lab P2S");
        assert_eq!(catalog[0].nozzle_diameters, vec![0.4]);
        assert_eq!(
            catalog[0].plate_keys,
            vec!["Supertack Plate", "Textured PEI Plate"]
        );
    }

    #[test]
    fn derives_official_plate_keys_and_removes_model_exclusions() {
        let profiles = ProfileTree::new();
        profiles.machine(
            "Bambu Lab P2S",
            r#"{
                "type":"machine_model",
                "name":"Bambu Lab P2S",
                "default_bed_type":"Textured PEI Plate",
                "not_support_bed_type":"Cool Plate"
            }"#,
        );
        profiles.machine(
            "base",
            r#"{"type":"machine","name":"base","instantiation":"false"}"#,
        );
        profiles.machine(
            "Bambu Lab P2S 0.4 nozzle",
            r#"{
                "type":"machine",
                "name":"Bambu Lab P2S 0.4 nozzle",
                "inherits":"base",
                "instantiation":"true",
                "printer_model":"Bambu Lab P2S",
                "nozzle_diameter":["0.4"]
            }"#,
        );
        profiles.filament(
            "fdm_filament_common",
            r#"{
                "cool_plate_temp":["40"],
                "eng_plate_temp":["50"],
                "hot_plate_temp":["55"],
                "supertack_plate_temp":["45"],
                "textured_plate_temp":["60"]
            }"#,
        );

        let catalog = load_printer_profiles(&profiles.root).unwrap();

        assert_eq!(
            catalog[0].plate_keys,
            vec![
                "Engineering Plate",
                "Smooth PEI Plate / High Temp Plate",
                "Supertack Plate",
                "Textured PEI Plate",
            ]
        );
    }

    #[test]
    fn rejects_inheritance_cycles_in_instantiable_bambu_profiles() {
        let profiles = ProfileTree::new();
        profiles.machine(
            "first",
            r#"{
                "type":"machine","name":"first","inherits":"second",
                "instantiation":"true","printer_model":"Bambu Lab Cycle",
                "nozzle_diameter":["0.4"],"plate_keys":["Textured PEI Plate"]
            }"#,
        );
        profiles.machine(
            "second",
            r#"{"type":"machine","name":"second","inherits":"first"}"#,
        );

        assert!(load_printer_profiles(&profiles.root).is_err());
    }

    #[test]
    fn never_reads_machine_profiles_from_third_party_vendor_directories() {
        let profiles = ProfileTree::new();
        let third_party = profiles.root.join("Other/machine");
        fs::create_dir_all(&third_party).unwrap();
        fs::write(
            third_party.join("Not Bambu.json"),
            r#"{
                "type":"machine","name":"Not Bambu","instantiation":"true",
                "printer_model":"Not Bambu","nozzle_diameter":["0.4"],
                "plate_keys":["Textured PEI Plate"]
            }"#,
        )
        .unwrap();

        assert!(load_printer_profiles(&profiles.root).unwrap().is_empty());
    }

    fn saved_input(name: &str, model: &str, is_default: bool) -> SavePrinter {
        SavePrinter {
            printer_id: None,
            display_name: name.to_owned(),
            model_key: model.to_owned(),
            nozzle_diameter: 0.4,
            default_plate: "Supertack Plate".to_owned(),
            ams_kind: "ams".to_owned(),
            is_default,
        }
    }

    #[test]
    fn setting_and_deleting_the_default_never_selects_a_replacement() {
        let mut service = PrinterService::new(AppDatabase::open_in_memory().unwrap());
        let first = service
            .save(saved_input("First", "Bambu Lab P2S", true))
            .unwrap();
        let second = service
            .save(saved_input("Second", "Bambu Lab A1", false))
            .unwrap();

        service
            .set_default(Uuid::parse_str(&second.printer_id).unwrap())
            .unwrap();
        let selected = service.list_saved(&[]).unwrap();
        assert!(
            !selected
                .iter()
                .find(|item| item.printer_id == first.printer_id)
                .unwrap()
                .is_default
        );
        assert!(
            selected
                .iter()
                .find(|item| item.printer_id == second.printer_id)
                .unwrap()
                .is_default
        );

        service
            .delete(Uuid::parse_str(&second.printer_id).unwrap())
            .unwrap();
        let remaining = service.list_saved(&[]).unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(!remaining[0].is_default);
    }

    #[test]
    fn saved_printer_availability_is_derived_from_the_current_catalog() {
        let mut service = PrinterService::new(AppDatabase::open_in_memory().unwrap());
        service
            .save(saved_input("Available", "Bambu Lab P2S", true))
            .unwrap();
        service
            .save(saved_input("Missing", "Bambu Lab Retired", false))
            .unwrap();
        let catalog = vec![PrinterProfile {
            model_key: "Bambu Lab P2S".to_owned(),
            display_name: "Bambu Lab P2S".to_owned(),
            nozzle_diameters: vec![0.4],
            plate_keys: vec!["Supertack Plate".to_owned()],
        }];

        let saved = service.list_saved(&catalog).unwrap();

        assert!(
            saved
                .iter()
                .find(|item| item.display_name == "Available")
                .unwrap()
                .is_available
        );
        assert!(
            !saved
                .iter()
                .find(|item| item.display_name == "Missing")
                .unwrap()
                .is_available
        );
    }

    #[test]
    fn chinese_display_names_use_character_not_byte_limits() {
        let mut service = PrinterService::new(AppDatabase::open_in_memory().unwrap());
        let name = "我的打印机".repeat(10);

        let saved = service
            .save(saved_input(&name, "Bambu Lab P2S", false))
            .unwrap();

        assert_eq!(saved.display_name, name);
    }

    #[test]
    #[ignore = "requires BAMBU_PROFILES_ROOT to point to installed Bambu Studio profiles"]
    fn smoke_installed_profiles_include_p2s_supertack_and_point_four_nozzle() {
        let root = std::env::var_os("BAMBU_PROFILES_ROOT")
            .map(PathBuf::from)
            .expect("BAMBU_PROFILES_ROOT is required");

        let catalog = load_printer_profiles(&root).unwrap();
        let p2s = catalog
            .iter()
            .find(|profile| profile.model_key == "Bambu Lab P2S")
            .expect("installed profiles must include P2S");

        assert!(p2s.nozzle_diameters.contains(&0.4));
        assert!(p2s.plate_keys.contains(&"Supertack Plate".to_owned()));
    }
}
