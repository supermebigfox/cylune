use crate::error::{AppError, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

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

#[cfg(any(target_os = "windows", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistryStringKind {
    Plain,
    Expand,
    Unsupported,
}

#[cfg(any(target_os = "windows", test))]
pub(crate) fn decode_registry_string<F>(
    kind: RegistryStringKind,
    bytes: &[u8],
    lookup_environment: F,
) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    if kind == RegistryStringKind::Unsupported || bytes.len() % 2 != 0 {
        return None;
    }
    let mut units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    while units.last() == Some(&0) {
        units.pop();
    }
    if units.contains(&0) {
        return None;
    }
    let decoded = String::from_utf16(&units).ok()?;
    if kind == RegistryStringKind::Plain {
        return Some(decoded);
    }

    let mut expanded = String::new();
    let mut rest = decoded.as_str();
    while let Some(start) = rest.find('%') {
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let end = after_start.find('%')?;
        let name = &after_start[..end];
        if name.is_empty() {
            return None;
        }
        expanded.push_str(&lookup_environment(name)?);
        rest = &after_start[end + 1..];
    }
    expanded.push_str(rest);
    Some(expanded)
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
            REG_EXPAND_SZ, REG_SZ,
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
                let Ok(display_name) = entry.get_raw_value("DisplayName") else {
                    continue;
                };
                let display_name_kind = match display_name.vtype {
                    REG_SZ => RegistryStringKind::Plain,
                    REG_EXPAND_SZ => RegistryStringKind::Expand,
                    _ => RegistryStringKind::Unsupported,
                };
                let Some(display_name) =
                    decode_registry_string(display_name_kind, &display_name.bytes, |name| {
                        std::env::var(name).ok()
                    })
                else {
                    continue;
                };
                if !display_name.to_ascii_lowercase().contains("bambu studio") {
                    continue;
                }
                if let Ok(location) = entry.get_raw_value("InstallLocation") {
                    let location_kind = match location.vtype {
                        REG_SZ => RegistryStringKind::Plain,
                        REG_EXPAND_SZ => RegistryStringKind::Expand,
                        _ => RegistryStringKind::Unsupported,
                    };
                    let Some(location) =
                        decode_registry_string(location_kind, &location.bytes, |name| {
                            std::env::var(name).ok()
                        })
                    else {
                        continue;
                    };
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
    use super::{
        decode_registry_string, ordered_candidates, InstallationDiscovery, RegistryStringKind,
        SYSTEM_EXECUTABLE,
    };
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

    #[cfg(not(target_os = "windows"))]
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

    #[cfg(not(target_os = "windows"))]
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

    #[cfg(not(target_os = "windows"))]
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
    fn default_macos_executable_allows_a_symlinked_app_root() {
        use std::os::unix::fs::symlink;

        let system_bundle = TemporaryBundle::new(true);
        let linked_app = system_bundle.root.join("linked-system.app");
        symlink(&system_bundle.app, &linked_app).unwrap();
        let discovery = InstallationDiscovery {
            explicit_app: None,
            system_executable: linked_app.join("Contents/MacOS/BambuStudio"),
        };

        let installation = discovery.discover().unwrap();

        assert_eq!(
            installation.executable,
            fs::canonicalize(&system_bundle.executable).unwrap()
        );
        assert_eq!(
            installation.profiles_root,
            fs::canonicalize(&system_bundle.profiles_root).unwrap()
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

    fn registry_utf16(value: &str) -> Vec<u8> {
        value
            .encode_utf16()
            .chain(std::iter::once(0))
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    #[test]
    fn registry_plain_string_accepts_only_valid_utf16() {
        assert_eq!(
            decode_registry_string(
                RegistryStringKind::Plain,
                &registry_utf16(r"C:\Bambu Studio"),
                |_| None,
            ),
            Some(r"C:\Bambu Studio".to_owned())
        );
        assert_eq!(
            decode_registry_string(RegistryStringKind::Plain, &[0x00, 0xD8, 0x00, 0x00], |_| {
                None
            },),
            None
        );
    }

    #[test]
    fn registry_expand_string_resolves_environment_variables() {
        assert_eq!(
            decode_registry_string(
                RegistryStringKind::Expand,
                &registry_utf16(r"%ProgramFiles%\Bambu Studio"),
                |name| (name == "ProgramFiles").then(|| r"C:\Program Files".to_owned()),
            ),
            Some(r"C:\Program Files\Bambu Studio".to_owned())
        );
    }

    #[test]
    fn registry_decoder_rejects_multi_strings_and_unresolved_variables() {
        assert_eq!(
            decode_registry_string(
                RegistryStringKind::Unsupported,
                &registry_utf16("first\0second"),
                |_| None,
            ),
            None
        );
        assert_eq!(
            decode_registry_string(
                RegistryStringKind::Expand,
                &registry_utf16(r"%MISSING%\Bambu Studio"),
                |_| None,
            ),
            None
        );
    }
}
