use super::discovery::BambuInstallation;
use crate::error::{AppError, Result};
use std::{fs, path::Path};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPlatform {
    MacOs,
    Windows,
}

pub fn resolve_selected_install(
    selected: &Path,
    platform: InstallPlatform,
) -> Result<BambuInstallation> {
    let (root, executable, profiles_root) = match platform {
        InstallPlatform::MacOs => {
            if !is_real_directory(selected) {
                return Err(AppError::BambuStudioMissing);
            }
            (
                selected.to_path_buf(),
                selected.join("Contents/MacOS/BambuStudio"),
                selected.join("Contents/Resources/profiles"),
            )
        }
        InstallPlatform::Windows => {
            let executable = if is_real_directory(selected) {
                selected.join("BambuStudio.exe")
            } else if selected
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("BambuStudio.exe"))
            {
                selected.to_path_buf()
            } else {
                return Err(AppError::BambuStudioMissing);
            };
            let Some(root) = executable.parent().map(Path::to_path_buf) else {
                return Err(AppError::BambuStudioMissing);
            };
            let profiles_root = root.join("resources/profiles");
            (root, executable, profiles_root)
        }
    };

    if !is_real_directory(&root) {
        return Err(AppError::BambuStudioMissing);
    }
    if !is_real_file(&executable)
        || (platform == InstallPlatform::MacOs && !has_execute_permission(&executable))
    {
        return Err(AppError::BambuStudioMissing);
    }
    if !is_real_directory(&profiles_root) {
        return Err(AppError::SlicerProfilesMissing);
    }

    let canonical_root = fs::canonicalize(&root).map_err(|_| AppError::BambuStudioMissing)?;
    let canonical_executable =
        fs::canonicalize(&executable).map_err(|_| AppError::BambuStudioMissing)?;
    let canonical_profiles =
        fs::canonicalize(&profiles_root).map_err(|_| AppError::SlicerProfilesMissing)?;
    let (expected_executable, expected_profiles) = match platform {
        InstallPlatform::MacOs => (
            canonical_root.join("Contents/MacOS/BambuStudio"),
            canonical_root.join("Contents/Resources/profiles"),
        ),
        InstallPlatform::Windows => {
            let resources = root.join("resources");
            if !is_real_directory(&resources)
                || fs::canonicalize(&resources).ok() != Some(canonical_root.join("resources"))
            {
                return Err(AppError::SlicerProfilesMissing);
            }
            (
                canonical_root.join("BambuStudio.exe"),
                canonical_root.join("resources/profiles"),
            )
        }
    };
    if canonical_executable != expected_executable {
        return Err(AppError::BambuStudioMissing);
    }
    if canonical_profiles != expected_profiles {
        return Err(AppError::SlicerProfilesMissing);
    }

    Ok(BambuInstallation {
        executable: canonical_executable,
        profiles_root: canonical_profiles,
    })
}

fn is_real_directory(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_dir()
            && !metadata.file_type().is_symlink()
            && !is_reparse_point(&metadata)
    })
}

fn is_real_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_file()
            && !metadata.file_type().is_symlink()
            && !is_reparse_point(&metadata)
    })
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    has_reparse_attribute(metadata.file_attributes())
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(any(windows, test))]
const fn has_reparse_attribute(file_attributes: u32) -> bool {
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(unix)]
fn has_execute_permission(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn has_execute_permission(_path: &Path) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{has_reparse_attribute, resolve_selected_install, InstallPlatform};
    use crate::error::AppError;
    use std::{
        fs,
        path::{Path, PathBuf},
    };
    use uuid::Uuid;

    struct InstallFixture {
        temporary_root: PathBuf,
        root: PathBuf,
        app: PathBuf,
        executable: PathBuf,
        profiles_root: PathBuf,
    }

    impl InstallFixture {
        fn windows(label: &str) -> Self {
            let fixture_root =
                std::env::temp_dir().join(format!("cylune-install-layout-{}", Uuid::new_v4()));
            let root = fixture_root.join(label);
            let executable = root.join("BambuStudio.exe");
            let profiles_root = root.join("resources/profiles");
            fs::create_dir_all(&profiles_root).unwrap();
            fs::write(&executable, b"windows executable fixture").unwrap();
            Self {
                temporary_root: fixture_root,
                root,
                app: PathBuf::new(),
                executable,
                profiles_root,
            }
        }

        fn macos() -> Self {
            let root =
                std::env::temp_dir().join(format!("cylune-install-layout-{}", Uuid::new_v4()));
            let app = root.join("BambuStudio.app");
            let executable = app.join("Contents/MacOS/BambuStudio");
            let profiles_root = app.join("Contents/Resources/profiles");
            fs::create_dir_all(&profiles_root).unwrap();
            fs::create_dir_all(executable.parent().unwrap()).unwrap();
            fs::write(&executable, b"macOS executable fixture").unwrap();
            make_executable(&executable);
            Self {
                temporary_root: root.clone(),
                root,
                app,
                executable,
                profiles_root,
            }
        }
    }

    impl Drop for InstallFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.temporary_root);
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

    #[test]
    fn resolves_a_windows_executable_and_neighbor_resources() {
        let fixture = InstallFixture::windows("C:/Program Files/Bambu Studio");
        let found =
            resolve_selected_install(&fixture.executable, InstallPlatform::Windows).unwrap();
        assert_eq!(found.executable, fixture.executable.canonicalize().unwrap());
        assert_eq!(
            found.profiles_root,
            fixture
                .root
                .join("resources/profiles")
                .canonicalize()
                .unwrap()
        );
    }

    #[test]
    fn resolves_a_windows_install_directory() {
        let fixture = InstallFixture::windows("Bambu Studio");
        let found = resolve_selected_install(&fixture.root, InstallPlatform::Windows).unwrap();

        assert_eq!(found.executable, fixture.executable.canonicalize().unwrap());
        assert_eq!(
            found.profiles_root,
            fixture.profiles_root.canonicalize().unwrap()
        );
    }

    #[test]
    fn macos_bundle_resolution_stays_unchanged() {
        let fixture = InstallFixture::macos();
        let found = resolve_selected_install(&fixture.app, InstallPlatform::MacOs).unwrap();
        assert_eq!(
            found.executable,
            fixture
                .app
                .join("Contents/MacOS/BambuStudio")
                .canonicalize()
                .unwrap()
        );
        assert_eq!(
            found.profiles_root,
            fixture.profiles_root.canonicalize().unwrap()
        );
    }

    #[test]
    fn reports_missing_windows_profiles_beside_a_real_executable() {
        let fixture = InstallFixture::windows("Bambu Studio without profiles");
        fs::remove_dir_all(&fixture.profiles_root).unwrap();

        assert!(matches!(
            resolve_selected_install(&fixture.executable, InstallPlatform::Windows),
            Err(AppError::SlicerProfilesMissing)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_executables_and_profile_directories() {
        use std::os::unix::fs::symlink;

        let executable_link = InstallFixture::windows("linked executable");
        let executable_target = executable_link.root.join("BambuStudio-real.exe");
        fs::write(&executable_target, b"outside executable").unwrap();
        fs::remove_file(&executable_link.executable).unwrap();
        symlink(&executable_target, &executable_link.executable).unwrap();
        assert!(matches!(
            resolve_selected_install(&executable_link.executable, InstallPlatform::Windows),
            Err(AppError::BambuStudioMissing)
        ));

        let profiles_link = InstallFixture::windows("linked profiles");
        let profiles_target = profiles_link.root.join("profiles-real");
        fs::create_dir(&profiles_target).unwrap();
        fs::remove_dir(&profiles_link.profiles_root).unwrap();
        symlink(&profiles_target, &profiles_link.profiles_root).unwrap();
        assert!(matches!(
            resolve_selected_install(&profiles_link.executable, InstallPlatform::Windows),
            Err(AppError::SlicerProfilesMissing)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_intermediate_resources_symlink_that_escapes_the_install_root() {
        use std::os::unix::fs::symlink;

        let fixture = InstallFixture::windows("intermediate resources link");
        let outside_resources = fixture.temporary_root.join("outside-resources");
        fs::create_dir_all(outside_resources.join("profiles")).unwrap();
        fs::remove_dir_all(fixture.root.join("resources")).unwrap();
        symlink(&outside_resources, fixture.root.join("resources")).unwrap();

        assert!(matches!(
            resolve_selected_install(&fixture.executable, InstallPlatform::Windows),
            Err(AppError::SlicerProfilesMissing)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn allows_a_symlinked_ancestor_above_the_selected_install_root() {
        use std::os::unix::fs::symlink;

        let temporary_root =
            std::env::temp_dir().join(format!("cylune-install-ancestor-{}", Uuid::new_v4()));
        let real_parent = temporary_root.join("real-parent");
        let linked_parent = temporary_root.join("linked-parent");
        let selected = linked_parent.join("Bambu Studio");
        fs::create_dir_all(real_parent.join("Bambu Studio/resources/profiles")).unwrap();
        fs::write(
            real_parent.join("Bambu Studio/BambuStudio.exe"),
            b"windows executable fixture",
        )
        .unwrap();
        symlink(&real_parent, &linked_parent).unwrap();

        let found = resolve_selected_install(&selected, InstallPlatform::Windows).unwrap();

        assert_eq!(
            found.executable,
            real_parent
                .join("Bambu Studio/BambuStudio.exe")
                .canonicalize()
                .unwrap()
        );
        fs::remove_dir_all(temporary_root).unwrap();
    }

    #[test]
    fn recognizes_only_the_windows_reparse_attribute_bit() {
        assert!(has_reparse_attribute(0x400));
        assert!(has_reparse_attribute(0x420));
        assert!(!has_reparse_attribute(0x20));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn rejects_a_windows_directory_reparse_point_inside_the_install() {
        use std::io::ErrorKind;
        use std::os::windows::fs::symlink_dir;

        let fixture = InstallFixture::windows("windows reparse resources");
        let outside_resources = fixture.temporary_root.join("outside-reparse-resources");
        fs::create_dir_all(outside_resources.join("profiles")).unwrap();
        fs::remove_dir_all(fixture.root.join("resources")).unwrap();
        if let Err(error) = symlink_dir(&outside_resources, fixture.root.join("resources")) {
            if error.kind() == ErrorKind::PermissionDenied {
                return;
            }
            panic!("failed to create Windows reparse fixture: {error}");
        }

        assert!(matches!(
            resolve_selected_install(&fixture.executable, InstallPlatform::Windows),
            Err(AppError::SlicerProfilesMissing)
        ));
    }
}
