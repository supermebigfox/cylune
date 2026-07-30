use crate::error::{AppError, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
        if let Some(app) = &self.explicit_app {
            match discover_selected_app(app) {
                Ok(installation) => return Ok(installation),
                Err(AppError::BambuStudioMissing) => {}
                Err(error) => return Err(error),
            }
        }

        discover_from_executable(&self.system_executable)
    }
}

fn discover_selected_app(app: &Path) -> Result<BambuInstallation> {
    if !is_directory(app) {
        return Err(AppError::BambuStudioMissing);
    }

    discover_from_executable(&app.join("Contents/MacOS/BambuStudio"))
}

fn discover_from_executable(executable: &Path) -> Result<BambuInstallation> {
    if !is_executable(executable) {
        return Err(AppError::BambuStudioMissing);
    }

    let Some(contents) = executable.parent().and_then(Path::parent) else {
        return Err(AppError::BambuStudioMissing);
    };
    let profiles_root = contents.join("Resources/profiles");
    if !is_directory(&profiles_root) {
        return Err(AppError::SlicerProfilesMissing);
    }

    Ok(BambuInstallation {
        executable: fs::canonicalize(executable).map_err(|_| AppError::BambuStudioMissing)?,
        profiles_root: fs::canonicalize(profiles_root)
            .map_err(|_| AppError::SlicerProfilesMissing)?,
    })
}

fn is_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_dir() && !metadata.file_type().is_symlink())
}

fn is_executable(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_file()
            && !metadata.file_type().is_symlink()
            && has_execute_permission(&metadata)
    })
}

#[cfg(unix)]
fn has_execute_permission(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn has_execute_permission(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{InstallationDiscovery, SYSTEM_EXECUTABLE};
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
}
