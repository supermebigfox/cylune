use crate::error::{AppError, Result};
use std::path::{Path, PathBuf};

use super::install_layout::{resolve_selected_install, InstallPlatform};

const SYSTEM_EXECUTABLE: &str = "/Applications/BambuStudio.app/Contents/MacOS/BambuStudio";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BambuInstallation {
    pub executable: PathBuf,
    pub profiles_root: PathBuf,
}

pub struct InstallationDiscovery {
    explicit_app: Option<PathBuf>,
    system_executable: PathBuf,
}

impl InstallationDiscovery {
    pub fn new(explicit_app: Option<PathBuf>) -> Self {
        Self {
            explicit_app,
            system_executable: PathBuf::from(SYSTEM_EXECUTABLE),
        }
    }

    pub fn discover(&self) -> Result<BambuInstallation> {
        #[cfg(target_os = "windows")]
        {
            return self.discover_windows();
        }
        #[cfg(not(target_os = "windows"))]
        {
            self.discover_macos()
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn discover_macos(&self) -> Result<BambuInstallation> {
        if let Some(app) = &self.explicit_app {
            match resolve_selected_install(app, InstallPlatform::MacOs) {
                Ok(installation) => return Ok(installation),
                Err(AppError::BambuStudioMissing) => {}
                Err(error) => return Err(error),
            }
        }

        discover_from_executable(&self.system_executable)
    }

    #[cfg(target_os = "windows")]
    fn discover_windows(&self) -> Result<BambuInstallation> {
        let candidates = ordered_candidates(
            self.explicit_app.clone(),
            windows_registry_candidates(),
            windows_standard_candidates(),
        );
        let mut profiles_missing = false;
        for candidate in candidates {
            match resolve_selected_install(&candidate, InstallPlatform::Windows) {
                Ok(installation) => return Ok(installation),
                Err(AppError::SlicerProfilesMissing) => profiles_missing = true,
                Err(AppError::BambuStudioMissing) => {}
                Err(error) => return Err(error),
            }
        }
        if profiles_missing {
            Err(AppError::SlicerProfilesMissing)
        } else {
            Err(AppError::BambuStudioMissing)
        }
    }
}

fn discover_from_executable(executable: &Path) -> Result<BambuInstallation> {
    let Some(app) = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
    else {
        return Err(AppError::BambuStudioMissing);
    };
    resolve_selected_install(app, InstallPlatform::MacOs)
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn ordered_candidates(
    explicit: Option<PathBuf>,
    registry: Vec<PathBuf>,
    standard: Vec<PathBuf>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for candidate in explicit.into_iter().chain(registry).chain(standard) {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    candidates
}

#[cfg(target_os = "windows")]
pub fn windows_registry_candidates() -> Vec<PathBuf> {
    use winreg::{
        enums::{
            HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
        },
        RegKey,
    };

    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    let mut candidates = Vec::new();
    for hive in [
        RegKey::predef(HKEY_CURRENT_USER),
        RegKey::predef(HKEY_LOCAL_MACHINE),
    ] {
        for access in [KEY_READ | KEY_WOW64_64KEY, KEY_READ | KEY_WOW64_32KEY] {
            let Ok(uninstall) = hive.open_subkey_with_flags(UNINSTALL, access) else {
                continue;
            };
            for key_name in uninstall.enum_keys().filter_map(std::result::Result::ok) {
                let Ok(entry) = uninstall.open_subkey_with_flags(&key_name, access) else {
                    continue;
                };
                let Ok(display_name) = entry.get_value::<String, _>("DisplayName") else {
                    continue;
                };
                if !display_name.to_ascii_lowercase().contains("bambu studio") {
                    continue;
                }
                if let Ok(location) = entry.get_value::<String, _>("InstallLocation") {
                    let location = location.trim().trim_matches('"');
                    if !location.is_empty() {
                        candidates.push(PathBuf::from(location).join("BambuStudio.exe"));
                    }
                }
            }
        }
    }
    ordered_candidates(None, candidates, Vec::new())
}

#[cfg(not(target_os = "windows"))]
pub fn windows_registry_candidates() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(target_os = "windows")]
fn windows_standard_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(
            PathBuf::from(program_files)
                .join("Bambu Studio")
                .join("BambuStudio.exe"),
        );
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local_app_data = PathBuf::from(local_app_data);
        candidates.push(
            local_app_data
                .join("Programs/Bambu Studio")
                .join("BambuStudio.exe"),
        );
        candidates.push(local_app_data.join("BambuStudio/BambuStudio.exe"));
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::{ordered_candidates, InstallationDiscovery, SYSTEM_EXECUTABLE};
    use crate::error::AppError;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEMPORARY_BUNDLE: AtomicU64 = AtomicU64::new(0);

    struct TemporaryBundle {
        root: PathBuf,
        app: PathBuf,
        executable: PathBuf,
        profiles_root: PathBuf,
    }

    impl TemporaryBundle {
        fn new(with_profiles: bool) -> Self {
            let root = std::env::temp_dir().join(format!(
                "bambu-pools-discovery-{}-{}",
                std::process::id(),
                NEXT_TEMPORARY_BUNDLE.fetch_add(1, Ordering::Relaxed)
            ));
            let app = root.join("BambuStudio.app");
            let executable = app.join("Contents/MacOS/BambuStudio");
            let profiles_root = app.join("Contents/Resources/profiles");

            fs::create_dir_all(executable.parent().unwrap()).unwrap();
            fs::write(&executable, b"not launched by discovery").unwrap();
            make_executable(&executable);
            if with_profiles {
                fs::create_dir_all(&profiles_root).unwrap();
            }

            Self {
                root,
                app,
                executable,
                profiles_root,
            }
        }
    }

    impl Drop for TemporaryBundle {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[cfg(not(unix))]
    fn make_executable(_path: &Path) {}

    fn discovery_for(app: &Path) -> InstallationDiscovery {
        InstallationDiscovery {
            explicit_app: Some(app.to_path_buf()),
            system_executable: std::env::temp_dir().join("missing-system-bambu-studio"),
        }
    }

    #[test]
    fn discovers_canonical_executable_and_official_profiles_from_selected_app() {
        let bundle = TemporaryBundle::new(true);

        let installation = discovery_for(&bundle.app).discover().unwrap();

        assert_eq!(
            installation.executable,
            fs::canonicalize(&bundle.executable).unwrap()
        );
        assert_eq!(
            installation.profiles_root,
            fs::canonicalize(&bundle.profiles_root).unwrap()
        );
    }

    #[test]
    fn reports_missing_profiles_for_an_otherwise_valid_selected_app() {
        let bundle = TemporaryBundle::new(false);

        let error = discovery_for(&bundle.app).discover().unwrap_err();

        assert!(matches!(error, AppError::SlicerProfilesMissing));
    }

    #[test]
    fn uses_the_fixed_macos_system_executable_by_default() {
        assert_eq!(
            InstallationDiscovery::new(None).system_executable,
            PathBuf::from(SYSTEM_EXECUTABLE)
        );
    }

    #[test]
    fn falls_back_to_the_system_executable_when_selected_app_is_missing() {
        let system_bundle = TemporaryBundle::new(true);
        let discovery = InstallationDiscovery {
            explicit_app: Some(system_bundle.root.join("missing.app")),
            system_executable: system_bundle.executable.clone(),
        };

        let installation = discovery.discover().unwrap();

        assert_eq!(
            installation.executable,
            fs::canonicalize(&system_bundle.executable).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_executables_and_profile_roots() {
        use std::os::unix::fs::symlink;

        let executable_link = TemporaryBundle::new(true);
        let executable_target = executable_link.root.join("outside-executable");
        fs::write(&executable_target, b"outside").unwrap();
        make_executable(&executable_target);
        fs::remove_file(&executable_link.executable).unwrap();
        symlink(&executable_target, &executable_link.executable).unwrap();
        assert!(matches!(
            discovery_for(&executable_link.app).discover(),
            Err(AppError::BambuStudioMissing)
        ));

        let profiles_link = TemporaryBundle::new(true);
        let profiles_target = profiles_link.root.join("outside-profiles");
        fs::create_dir(&profiles_target).unwrap();
        fs::remove_dir(&profiles_link.profiles_root).unwrap();
        symlink(&profiles_target, &profiles_link.profiles_root).unwrap();
        assert!(matches!(
            discovery_for(&profiles_link.app).discover(),
            Err(AppError::SlicerProfilesMissing)
        ));
    }

    #[test]
    fn windows_candidates_keep_manual_registry_and_standard_priority() {
        let manual = PathBuf::from(r"C:\Selected\BambuStudio.exe");
        let registry = vec![
            PathBuf::from(r"C:\Registry\First\BambuStudio.exe"),
            PathBuf::from(r"C:\Registry\Second\BambuStudio.exe"),
        ];
        let standard = vec![
            PathBuf::from(r"C:\Program Files\Bambu Studio\BambuStudio.exe"),
            PathBuf::from(r"C:\Users\Robin\AppData\Local\BambuStudio\BambuStudio.exe"),
        ];

        assert_eq!(
            ordered_candidates(Some(manual.clone()), registry.clone(), standard.clone()),
            vec![
                manual,
                registry[0].clone(),
                registry[1].clone(),
                standard[0].clone(),
                standard[1].clone(),
            ]
        );
    }
}
