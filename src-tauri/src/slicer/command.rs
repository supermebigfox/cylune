use crate::{
    error::{AppError, Result},
    printers::SavedPrinter,
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

const THREE_MF_SUFFIX: &str = ".3mf";
const GCODE_THREE_MF_SUFFIX: &str = ".gcode.3mf";

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

/// All inputs required to prepare one local Bambu Studio slicing invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SliceRequest {
    /// The saved printer selected by the user. Profile resolution is performed
    /// by the caller, while the request preserves the selected printer.
    pub printer: SavedPrinter,
    pub input: PathBuf,
    pub plate_selection: PlateSelection,
    #[serde(default)]
    pub estimate_mode: bool,
    pub machine_settings: PathBuf,
}

/// Builds individual process arguments for Bambu Studio. This function never
/// invokes a shell and therefore never adds shell quoting.
pub fn build_bambu_args(request: &SliceRequest, temporary_output: &Path) -> Result<Vec<OsString>> {
    validate_request(request, temporary_output)?;

    let mut args = vec![
        OsString::from("--slice"),
        OsString::from(request.plate_selection.bambu_value()),
        OsString::from("--debug"),
        OsString::from("4"),
    ];
    if request.estimate_mode {
        args.push(OsString::from("--estimate-mode"));
    }
    args.extend([
        OsString::from("--load-settings"),
        request.machine_settings.as_os_str().to_os_string(),
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
        || profile_path_has_delimiter(&request.machine_settings)
    {
        return Err(AppError::InvalidFile);
    }

    validate_private_output_path(temporary_output)?;
    if fs::symlink_metadata(temporary_output).is_ok() {
        return Err(AppError::OutputExists);
    }

    Ok(())
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
    use super::{build_bambu_args, PlateSelection, SliceRequest};
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
            fs::write(&input, b"project").unwrap();
            fs::write(&machine, b"machine").unwrap();

            Self {
                temporary_output: root.join("temporary.gcode.3mf"),
                root,
                input,
                machine,
            }
        }

        fn request(&self) -> SliceRequest {
            SliceRequest {
                printer: saved_printer(),
                input: self.input.clone(),
                plate_selection: PlateSelection::All,
                estimate_mode: false,
                machine_settings: self.machine.clone(),
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
    fn loads_only_the_target_machine_and_preserves_native_3mf_settings() {
        let fixture = Fixture::new();
        let request = fixture.request();

        let args = build_bambu_args(&request, &fixture.temporary_output).unwrap();

        assert_eq!(
            args,
            vec![
                OsString::from("--slice"),
                OsString::from("0"),
                OsString::from("--debug"),
                OsString::from("4"),
                OsString::from("--load-settings"),
                OsString::from(fixture.machine.as_os_str()),
                OsString::from("--export-3mf"),
                OsString::from(fixture.temporary_output.as_os_str()),
                OsString::from(fixture.input.as_os_str()),
            ]
        );
        assert!(args
            .iter()
            .all(|argument| !argument.to_string_lossy().contains('"')));
        let joined = args
            .iter()
            .map(|argument| argument.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        for forbidden in [
            "--load-filaments",
            "process",
            "effective-process",
            "effective-filament",
        ] {
            assert!(
                !joined.contains(forbidden),
                "unexpected argument: {forbidden}"
            );
        }
        assert!(!args.iter().any(|argument| argument == "--estimate-mode"));
    }

    #[test]
    fn rejects_missing_machine_settings() {
        let fixture = Fixture::new();
        let mut request = fixture.request();
        request.machine_settings = fixture.root.join("missing-machine.json");
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
        assert!(!args.iter().any(|argument| argument == "--load-filaments"));
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
}
