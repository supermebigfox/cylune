use crate::{
    error::{AppError, Result},
    printers::SavedPrinter,
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const THREE_MF_SUFFIX: &str = ".3mf";
const GCODE_THREE_MF_SUFFIX: &str = ".gcode.3mf";
const MAX_PROFILE_BYTES: u64 = 4 * 1024 * 1024;

pub const BAMBU_PLATE_TYPES: [&str; 5] = [
    "Cool Plate",
    "Engineering Plate",
    "Smooth PEI Plate / High Temp Plate",
    "Supertack Plate",
    "Textured PEI Plate",
];

/// Bambu Studio's all-plates selection is represented by CLI value `0`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlateSelection {
    #[default]
    All,
}

impl PlateSelection {
    const fn bambu_value(self) -> &'static str {
        match self {
            Self::All => "0",
        }
    }
}

/// The small, explicit subset of Bambu Studio settings that can be overridden
/// without changing a process preset. Layer height intentionally remains in
/// the process preset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct FastOverrides {
    pub infill_density: Option<f64>,
    pub support_enabled: Option<bool>,
    pub plate_type: Option<String>,
}

impl FastOverrides {
    pub(crate) fn is_empty(&self) -> bool {
        self.infill_density.is_none() && self.support_enabled.is_none() && self.plate_type.is_none()
    }
}

/// All inputs required to prepare one local Bambu Studio slicing invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceRequest {
    /// The saved printer selected by the user. Profile resolution is performed
    /// by the caller, while the request preserves the selected printer.
    pub printer: SavedPrinter,
    pub expected_filament_count: usize,
    pub input: PathBuf,
    pub plate_selection: PlateSelection,
    #[serde(default)]
    pub estimate_mode: bool,
    #[serde(default)]
    pub preserve_project_settings: bool,
    pub preserve_filament_settings: Vec<bool>,
    pub machine_settings: PathBuf,
    pub process_settings: PathBuf,
    pub filament_settings: Vec<PathBuf>,
    pub fast_overrides: FastOverrides,
}

/// Builds individual process arguments for Bambu Studio. This function never
/// invokes a shell and therefore never adds shell quoting.
pub fn build_bambu_args(request: &SliceRequest, temporary_output: &Path) -> Result<Vec<OsString>> {
    validate_request(request, temporary_output)?;

    let mut args = vec![
        OsString::from("--slice"),
        OsString::from(request.plate_selection.bambu_value()),
        OsString::from("--debug"),
        OsString::from("2"),
    ];
    if request.estimate_mode {
        args.push(OsString::from("--estimate-mode"));
    }
    args.extend([
        OsString::from("--load-settings"),
        join_profile_paths([
            request.machine_settings.as_path(),
            request.process_settings.as_path(),
        ])?,
    ]);
    args.extend([
        OsString::from("--load-filaments"),
        join_profile_paths(request.filament_settings.iter().map(PathBuf::as_path))?,
    ]);
    args.extend([
        OsString::from("--export-3mf"),
        temporary_output.as_os_str().to_os_string(),
        request.input.as_os_str().to_os_string(),
    ]);
    Ok(args)
}

fn validate_request(request: &SliceRequest, temporary_output: &Path) -> Result<()> {
    if !is_regular_file(&request.input) || !has_suffix(&request.input, THREE_MF_SUFFIX) {
        return Err(AppError::InvalidFile);
    }
    if !is_regular_file(&request.machine_settings)
        || !is_regular_file(&request.process_settings)
        || request.filament_settings.is_empty()
        || request
            .filament_settings
            .iter()
            .any(|path| !is_regular_file(path))
    {
        return Err(AppError::InvalidFile);
    }
    if request.filament_settings.len() != request.expected_filament_count
        || request.preserve_filament_settings.len() != request.expected_filament_count
    {
        return Err(AppError::SlicerIncompatible);
    }
    if profile_path_has_delimiter(&request.machine_settings)
        || profile_path_has_delimiter(&request.process_settings)
        || request
            .filament_settings
            .iter()
            .any(|path| profile_path_has_delimiter(path))
    {
        return Err(AppError::InvalidFile);
    }

    validate_private_output_path(temporary_output)?;
    if fs::symlink_metadata(temporary_output).is_ok() {
        return Err(AppError::OutputExists);
    }

    validate_fast_overrides(&request.fast_overrides)
}

fn validate_fast_overrides(overrides: &FastOverrides) -> Result<()> {
    if overrides
        .infill_density
        .is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value))
    {
        return Err(AppError::InvalidFile);
    }
    if overrides
        .plate_type
        .as_deref()
        .is_some_and(|plate| !BAMBU_PLATE_TYPES.contains(&plate) || plate.trim() != plate)
    {
        return Err(AppError::InvalidFile);
    }
    Ok(())
}

pub(crate) fn materialize_process_settings(
    source: &Path,
    destination: &Path,
    overrides: &FastOverrides,
    preserve_project_settings: bool,
) -> Result<()> {
    validate_fast_overrides(overrides)?;
    let metadata = fs::symlink_metadata(source).map_err(|_| AppError::InvalidFile)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_PROFILE_BYTES
    {
        return Err(AppError::InvalidFile);
    }
    let source_profile = serde_json::from_slice::<serde_json::Value>(&fs::read(source)?)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or(AppError::InvalidFile)?;
    if source_profile
        .get("type")
        .and_then(serde_json::Value::as_str)
        != Some("process")
    {
        return Err(AppError::InvalidFile);
    }
    let mut profile = if preserve_project_settings {
        let mut minimal = serde_json::Map::new();
        // Bambu Studio still requires the identity metadata from an official
        // process preset even when we deliberately omit its inherited print
        // values. Keeping only these fields lets the 3MF retain its own
        // adjusted wall/infill/layer speeds without the CLI rejecting the
        // lightweight preset as unsupported.
        for key in ["type", "name", "from", "setting_id", "instantiation"] {
            if let Some(value) = source_profile.get(key).cloned() {
                minimal.insert(key.to_owned(), value);
            }
        }
        minimal
    } else {
        source_profile
    };
    if let Some(infill_density) = overrides.infill_density {
        profile.insert(
            "sparse_infill_density".to_owned(),
            serde_json::Value::String(format!("{infill_density}%")),
        );
    }
    if let Some(support_enabled) = overrides.support_enabled {
        profile.insert(
            "enable_support".to_owned(),
            serde_json::Value::String(if support_enabled { "1" } else { "0" }.to_owned()),
        );
    }
    if let Some(plate_type) = &overrides.plate_type {
        profile.insert(
            "curr_bed_type".to_owned(),
            serde_json::Value::String(plate_type.clone()),
        );
    }

    let parent = destination.parent().ok_or(AppError::InvalidFile)?;
    if !is_regular_directory(parent) {
        return Err(AppError::InvalidFile);
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| AppError::InvalidFile)?;
    serde_json::to_writer(&mut output, &profile).map_err(|_| AppError::InvalidFile)?;
    output.flush().map_err(|_| AppError::InvalidFile)?;
    output.sync_all().map_err(|_| AppError::InvalidFile)
}

pub(crate) fn materialize_filament_settings(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source).map_err(|_| AppError::InvalidFile)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_PROFILE_BYTES
    {
        return Err(AppError::InvalidFile);
    }
    let source_profile = serde_json::from_slice::<serde_json::Value>(&fs::read(source)?)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or(AppError::InvalidFile)?;
    if source_profile
        .get("type")
        .and_then(serde_json::Value::as_str)
        != Some("filament")
    {
        return Err(AppError::InvalidFile);
    }
    let mut profile = serde_json::Map::new();
    for key in ["type", "name", "from", "setting_id", "instantiation"] {
        if let Some(value) = source_profile.get(key).cloned() {
            profile.insert(key.to_owned(), value);
        }
    }

    let parent = destination.parent().ok_or(AppError::InvalidFile)?;
    if !is_regular_directory(parent) {
        return Err(AppError::InvalidFile);
    }
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| AppError::InvalidFile)?;
    serde_json::to_writer(&mut output, &profile).map_err(|_| AppError::InvalidFile)?;
    output.flush().map_err(|_| AppError::InvalidFile)?;
    output.sync_all().map_err(|_| AppError::InvalidFile)
}

fn join_profile_paths<'a>(paths: impl IntoIterator<Item = &'a Path>) -> Result<OsString> {
    let mut joined = OsString::new();
    for (index, path) in paths.into_iter().enumerate() {
        if profile_path_has_delimiter(path) {
            return Err(AppError::InvalidFile);
        }
        if index > 0 {
            joined.push(OsStr::new(";"));
        }
        joined.push(path.as_os_str());
    }
    Ok(joined)
}

fn validate_private_output_path(path: &Path) -> Result<()> {
    if !has_suffix(path, GCODE_THREE_MF_SUFFIX) {
        return Err(AppError::InvalidFile);
    }
    let Some(parent) = path.parent() else {
        return Err(AppError::InvalidFile);
    };
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    if !is_regular_directory(parent) {
        return Err(AppError::InvalidFile);
    }
    Ok(())
}

fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_file() && !metadata.file_type().is_symlink())
}

fn is_regular_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_dir() && !metadata.file_type().is_symlink())
}

fn has_suffix(path: &Path, suffix: &str) -> bool {
    path.file_name().is_some_and(|file_name| {
        file_name
            .to_string_lossy()
            .to_ascii_lowercase()
            .ends_with(suffix)
    })
}

fn profile_path_has_delimiter(path: &Path) -> bool {
    path.as_os_str().to_string_lossy().contains(';')
}

#[cfg(test)]
mod tests {
    use super::{
        build_bambu_args, materialize_filament_settings, materialize_process_settings,
        FastOverrides, PlateSelection, SliceRequest,
    };
    use crate::{error::AppError, printers::SavedPrinter};
    use std::{
        ffi::OsString,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        input: PathBuf,
        machine: PathBuf,
        process: PathBuf,
        filaments: Vec<PathBuf>,
        temporary_output: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "bambu-pools-command-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            let profiles = root.join("profiles");
            let models = root.join("models");
            fs::create_dir_all(&profiles).unwrap();
            fs::create_dir_all(&models).unwrap();

            let input = models.join("a model;not-a-command.3mf");
            let machine = profiles.join("P2S machine.json");
            let process = profiles.join("0.20 Standard.json");
            let filament = profiles.join("PLA Basic.json");
            fs::write(&input, b"project").unwrap();
            fs::write(&machine, b"machine").unwrap();
            fs::write(&process, b"process").unwrap();
            fs::write(&filament, b"filament").unwrap();

            Self {
                temporary_output: root.join("temporary.gcode.3mf"),
                root,
                input,
                machine,
                process,
                filaments: vec![filament],
            }
        }

        fn request(&self) -> SliceRequest {
            SliceRequest {
                printer: saved_printer(),
                expected_filament_count: 1,
                input: self.input.clone(),
                plate_selection: PlateSelection::All,
                estimate_mode: false,
                preserve_project_settings: false,
                preserve_filament_settings: vec![false],
                machine_settings: self.machine.clone(),
                process_settings: self.process.clone(),
                filament_settings: self.filaments.clone(),
                fast_overrides: FastOverrides::default(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn saved_printer() -> SavedPrinter {
        SavedPrinter {
            printer_id: "saved-printer".to_owned(),
            display_name: "P2S".to_owned(),
            model_key: "P2S".to_owned(),
            nozzle_diameter: 0.4,
            default_plate: "smooth".to_owned(),
            ams_kind: "ams".to_owned(),
            is_default: true,
            is_available: true,
        }
    }

    #[test]
    fn builds_literal_shell_free_arguments_for_paths_with_spaces_and_metacharacters() {
        let fixture = Fixture::new();
        let request = fixture.request();

        let args = build_bambu_args(&request, &fixture.temporary_output).unwrap();

        assert_eq!(
            args,
            vec![
                OsString::from("--slice"),
                OsString::from("0"),
                OsString::from("--debug"),
                OsString::from("2"),
                OsString::from("--load-settings"),
                OsString::from(format!(
                    "{};{}",
                    fixture.machine.display(),
                    fixture.process.display()
                )),
                OsString::from("--load-filaments"),
                OsString::from(fixture.filaments[0].as_os_str()),
                OsString::from("--export-3mf"),
                OsString::from(fixture.temporary_output.as_os_str()),
                OsString::from(fixture.input.as_os_str()),
            ]
        );
        assert!(args
            .iter()
            .all(|argument| !argument.to_string_lossy().contains('"')));
    }

    #[test]
    fn preserving_project_settings_still_loads_the_effective_process_profile() {
        let fixture = Fixture::new();
        let mut request = fixture.request();
        request.preserve_project_settings = true;
        request.fast_overrides = FastOverrides {
            infill_density: Some(20.0),
            support_enabled: Some(true),
            plate_type: Some("Textured PEI Plate".to_owned()),
        };

        let args = build_bambu_args(&request, &fixture.temporary_output).unwrap();

        let settings_index = args
            .iter()
            .position(|argument| argument == "--load-settings")
            .unwrap();
        assert!(args[settings_index + 1]
            .to_string_lossy()
            .contains(fixture.process.to_string_lossy().as_ref()));
        assert!(!args.iter().any(|argument| {
            let argument = argument.to_string_lossy();
            argument.contains("sparse-infill-density")
                || argument.contains("enable-support")
                || argument.contains("curr-bed-type")
        }));
    }

    #[test]
    fn rejects_missing_machine_or_process_settings() {
        let fixture = Fixture::new();
        let mut request = fixture.request();
        request.machine_settings = fixture.root.join("missing-machine.json");
        assert!(matches!(
            build_bambu_args(&request, &fixture.temporary_output),
            Err(AppError::InvalidFile)
        ));

        let mut request = fixture.request();
        request.process_settings = fixture.root.join("missing-process.json");
        assert!(matches!(
            build_bambu_args(&request, &fixture.temporary_output),
            Err(AppError::InvalidFile)
        ));
    }

    #[test]
    fn rejects_mismatched_filament_count_and_nan_overrides() {
        let fixture = Fixture::new();

        let mut request = fixture.request();
        request.expected_filament_count = 2;
        assert!(matches!(
            build_bambu_args(&request, &fixture.temporary_output),
            Err(AppError::SlicerIncompatible)
        ));

        let mut request = fixture.request();
        request.fast_overrides.infill_density = Some(f64::NAN);
        assert!(matches!(
            build_bambu_args(&request, &fixture.temporary_output),
            Err(AppError::InvalidFile)
        ));
    }

    #[test]
    fn rejects_existing_or_malformed_private_output() {
        let fixture = Fixture::new();
        fs::write(&fixture.temporary_output, b"existing private output").unwrap();

        assert!(matches!(
            build_bambu_args(&fixture.request(), &fixture.temporary_output),
            Err(AppError::OutputExists)
        ));
        fs::remove_file(&fixture.temporary_output).unwrap();
        let malformed = fixture.root.join("temporary.3mf");

        assert!(matches!(
            build_bambu_args(&fixture.request(), &malformed),
            Err(AppError::InvalidFile)
        ));
    }

    #[test]
    fn fast_overrides_are_not_forwarded_as_unsupported_cli_options() {
        let fixture = Fixture::new();
        let mut request = fixture.request();
        request.fast_overrides.infill_density = Some(15.5);

        let args = build_bambu_args(&request, &fixture.temporary_output).unwrap();

        assert!(!args
            .iter()
            .any(|argument| argument.to_string_lossy().contains("infill_density")));
        assert!(!args.iter().any(|argument| argument == "--layer_height=0.2"));
    }

    #[test]
    fn enables_bambu_estimation_only_for_an_explicit_machine_conversion() {
        let fixture = Fixture::new();
        let mut request = fixture.request();
        request.estimate_mode = true;

        let args = build_bambu_args(&request, &fixture.temporary_output).unwrap();

        assert_eq!(
            args.iter()
                .filter(|argument| *argument == "--estimate-mode")
                .count(),
            1
        );
    }

    #[test]
    fn joins_multiple_filament_profiles_in_one_bambu_argument() {
        let fixture = Fixture::new();
        let second_filament = fixture.root.join("profiles/PETG Basic.json");
        fs::write(&second_filament, b"filament").unwrap();
        let mut request = fixture.request();
        request.expected_filament_count = 2;
        request.filament_settings.push(second_filament.clone());
        request.preserve_filament_settings.push(false);

        let args = build_bambu_args(&request, &fixture.temporary_output).unwrap();

        let load_filaments = args
            .iter()
            .position(|argument| argument == "--load-filaments")
            .unwrap();
        assert_eq!(
            args[load_filaments + 1],
            OsString::from(format!(
                "{};{}",
                fixture.filaments[0].display(),
                second_filament.display()
            ))
        );
    }

    #[test]
    fn accepts_a_nonexistent_temporary_output_with_a_safe_suffix() {
        let fixture = Fixture::new();
        let request = fixture.request();

        assert!(build_bambu_args(&request, &fixture.temporary_output).is_ok());
    }

    #[test]
    fn slice_request_round_trips_through_the_tauri_json_boundary() {
        let fixture = Fixture::new();
        let request = fixture.request();

        let json = serde_json::to_value(&request).unwrap();
        let decoded: SliceRequest = serde_json::from_value(json).unwrap();

        assert_eq!(decoded, request);
    }

    #[test]
    fn materializes_real_bambu_fast_override_keys_in_a_process_profile() {
        let fixture = Fixture::new();
        fs::write(
            &fixture.process,
            br#"{"type":"process","name":"0.20 Standard","sparse_infill_density":"15%"}"#,
        )
        .unwrap();
        let output = fixture.root.join("effective-process.json");
        let overrides = FastOverrides {
            infill_density: Some(22.5),
            support_enabled: Some(true),
            plate_type: Some("Supertack Plate".to_owned()),
        };

        materialize_process_settings(&fixture.process, &output, &overrides, false).unwrap();

        let effective: serde_json::Value =
            serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(effective["sparse_infill_density"], "22.5%");
        assert_eq!(effective["enable_support"], "1");
        assert_eq!(effective["curr_bed_type"], "Supertack Plate");
    }

    #[test]
    fn materializes_only_explicit_overrides_when_preserving_project_settings() {
        let fixture = Fixture::new();
        fs::write(
            &fixture.process,
            br#"{"type":"process","name":"0.20 Standard","from":"system","setting_id":"GP155","instantiation":"true","inherits":"fdm_process_single_0.20","outer_wall_speed":"999","layer_height":"0.2"}"#,
        )
        .unwrap();
        let output = fixture.root.join("preserved-process.json");

        materialize_process_settings(
            &fixture.process,
            &output,
            &FastOverrides {
                infill_density: Some(15.0),
                support_enabled: Some(false),
                plate_type: Some("Textured PEI Plate".to_owned()),
            },
            true,
        )
        .unwrap();

        let effective: serde_json::Value =
            serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(effective["name"], "0.20 Standard");
        assert_eq!(effective["from"], "system");
        assert_eq!(effective["setting_id"], "GP155");
        assert_eq!(effective["instantiation"], "true");
        assert_eq!(effective["sparse_infill_density"], "15%");
        assert_eq!(effective["enable_support"], "0");
        assert_eq!(effective["curr_bed_type"], "Textured PEI Plate");
        assert!(effective.get("inherits").is_none());
        assert!(effective.get("outer_wall_speed").is_none());
        assert!(effective.get("layer_height").is_none());
    }

    #[test]
    fn materializes_only_identity_metadata_when_preserving_filament_settings() {
        let fixture = Fixture::new();
        fs::write(
            &fixture.filaments[0],
            br#"{"type":"filament","name":"Bambu PLA Basic @BBL P2S","from":"system","setting_id":"GFSA00_11","instantiation":"true","inherits":"Bambu PLA Basic @base","filament_density":["1.26"],"filament_flow_ratio":["0.98"]}"#,
        )
        .unwrap();
        let output = fixture.root.join("preserved-filament.json");

        materialize_filament_settings(&fixture.filaments[0], &output).unwrap();

        let effective: serde_json::Value =
            serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
        assert_eq!(effective["type"], "filament");
        assert_eq!(effective["name"], "Bambu PLA Basic @BBL P2S");
        assert_eq!(effective["from"], "system");
        assert_eq!(effective["setting_id"], "GFSA00_11");
        assert_eq!(effective["instantiation"], "true");
        assert!(effective.get("inherits").is_none());
        assert!(effective.get("filament_density").is_none());
        assert!(effective.get("filament_flow_ratio").is_none());
    }

    #[test]
    #[ignore = "requires an installed Bambu Studio and real project fixture"]
    fn smoke_installed_cli_accepts_the_materialized_fast_process() {
        use std::process::Command;

        let executable = PathBuf::from(std::env::var_os("BAMBU_STUDIO_EXECUTABLE").unwrap());
        let profiles = PathBuf::from(std::env::var_os("BAMBU_PROFILES_ROOT").unwrap());
        let project = PathBuf::from(std::env::var_os("BAMBU_PROJECT_3MF").unwrap());
        let fixture = Fixture::new();
        let machine = profiles.join("BBL/machine/Bambu Lab P2S 0.4 nozzle.json");
        let process = profiles.join("BBL/process/0.20mm Standard @BBL P2S.json");
        let filament = profiles.join("BBL/filament/Bambu PLA Basic @BBL P2S.json");
        let effective_process = fixture.root.join("effective-process.json");
        let exported = fixture.root.join("exported-settings.json");
        materialize_process_settings(
            &process,
            &effective_process,
            &FastOverrides {
                infill_density: Some(17.5),
                support_enabled: Some(true),
                plate_type: Some("Supertack Plate".to_owned()),
            },
            false,
        )
        .unwrap();
        let settings = OsString::from(format!(
            "{};{}",
            machine.display(),
            effective_process.display()
        ));
        let filaments = OsString::from(
            std::iter::repeat_n(filament.display().to_string(), 4)
                .collect::<Vec<_>>()
                .join(";"),
        );

        let status = Command::new(executable)
            .current_dir(&fixture.root)
            .arg("--debug")
            .arg("2")
            .arg("--load-settings")
            .arg(settings)
            .arg("--load-filaments")
            .arg(filaments)
            .arg("--export-settings")
            .arg(&exported)
            .arg(project)
            .status()
            .unwrap();

        assert!(status.success());
        let exported: serde_json::Value =
            serde_json::from_slice(&fs::read(exported).unwrap()).unwrap();
        assert_eq!(exported["sparse_infill_density"], "17.5%");
        assert_eq!(exported["enable_support"], "1");
        assert_eq!(exported["curr_bed_type"], "Supertack Plate");
    }
}
