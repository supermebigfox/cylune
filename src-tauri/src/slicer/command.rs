use crate::{
    error::{AppError, Result},
    printers::SavedPrinter,
};
use std::{
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

const THREE_MF_SUFFIX: &str = ".3mf";
const GCODE_THREE_MF_SUFFIX: &str = ".gcode.3mf";

/// Bambu Studio's all-plates selection is represented by CLI value `0`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FastOverrides {
    pub infill_density: Option<f64>,
}

/// All inputs required to prepare one local Bambu Studio slicing invocation.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceRequest {
    /// The saved printer selected by the user. Profile resolution is performed
    /// by the caller, while the request preserves the selected printer.
    pub printer: SavedPrinter,
    pub expected_filament_count: usize,
    pub allow_overwrite: bool,
    pub input: PathBuf,
    pub destination: PathBuf,
    pub plate_selection: PlateSelection,
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
        OsString::from("--load-settings"),
        join_profile_paths([
            request.machine_settings.as_path(),
            request.process_settings.as_path(),
        ])?,
        OsString::from("--load-filaments"),
        join_profile_paths(request.filament_settings.iter().map(PathBuf::as_path))?,
    ];
    append_fast_overrides(&mut args, &request.fast_overrides)?;
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
    if request.filament_settings.len() != request.expected_filament_count {
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

    validate_output_path(&request.destination)?;
    validate_output_path(temporary_output)?;
    if same_existing_path(&request.input, &request.destination)
        || same_existing_path(&request.input, temporary_output)
        || same_existing_path(&request.destination, temporary_output)
    {
        return Err(AppError::InvalidFile);
    }
    if fs::symlink_metadata(&request.destination).is_ok() && !request.allow_overwrite {
        return Err(AppError::OutputExists);
    }
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
    Ok(())
}

fn append_fast_overrides(args: &mut Vec<OsString>, overrides: &FastOverrides) -> Result<()> {
    validate_fast_overrides(overrides)?;
    if let Some(infill_density) = overrides.infill_density {
        args.push(OsString::from(format!("--infill_density={infill_density}")));
    }
    Ok(())
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

fn validate_output_path(path: &Path) -> Result<()> {
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
    if fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_symlink() || !metadata.file_type().is_file())
    {
        return Err(AppError::InvalidFile);
    }
    Ok(())
}

fn same_existing_path(first: &Path, second: &Path) -> bool {
    match (fs::canonicalize(first), fs::canonicalize(second)) {
        (Ok(first), Ok(second)) => first == second,
        _ => false,
    }
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
    use super::{build_bambu_args, FastOverrides, PlateSelection, SliceRequest};
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
        destination: PathBuf,
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
                destination: root.join("out.gcode.3mf"),
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
                allow_overwrite: false,
                input: self.input.clone(),
                destination: self.destination.clone(),
                plate_selection: PlateSelection::All,
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
    fn rejects_mismatched_filament_count_nan_and_unauthorized_destination_overwrite() {
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

        fs::write(&fixture.destination, b"existing output").unwrap();
        let request = fixture.request();
        assert!(matches!(
            build_bambu_args(&request, &fixture.temporary_output),
            Err(AppError::OutputExists)
        ));
    }

    #[test]
    fn rejects_a_destination_equal_to_the_input() {
        let fixture = Fixture::new();
        let sliced_name = fixture.root.join("same-file.gcode.3mf");
        fs::write(&sliced_name, b"project").unwrap();
        let mut request = fixture.request();
        request.input = sliced_name.clone();
        request.destination = sliced_name;

        assert!(matches!(
            build_bambu_args(&request, &fixture.temporary_output),
            Err(AppError::InvalidFile)
        ));
    }

    #[test]
    fn accepts_bare_relative_output_names_in_the_current_directory() {
        let fixture = Fixture::new();
        let unique = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let mut request = fixture.request();
        request.destination = PathBuf::from(format!("relative-{unique}.gcode.3mf"));
        let temporary_output = PathBuf::from(format!("temporary-{unique}.gcode.3mf"));

        assert!(build_bambu_args(&request, &temporary_output).is_ok());
    }

    #[test]
    fn translates_only_typed_allowlisted_fast_overrides() {
        let fixture = Fixture::new();
        let mut request = fixture.request();
        request.fast_overrides.infill_density = Some(15.5);

        let args = build_bambu_args(&request, &fixture.temporary_output).unwrap();

        assert!(args.contains(&OsString::from("--infill_density=15.5")));
        assert!(!args.iter().any(|argument| argument == "--layer_height=0.2"));
    }

    #[test]
    fn joins_multiple_filament_profiles_in_one_bambu_argument() {
        let fixture = Fixture::new();
        let second_filament = fixture.root.join("profiles/PETG Basic.json");
        fs::write(&second_filament, b"filament").unwrap();
        let mut request = fixture.request();
        request.expected_filament_count = 2;
        request.filament_settings.push(second_filament.clone());

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
}
