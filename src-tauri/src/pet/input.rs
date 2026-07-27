use std::{
    io,
    path::{Path, PathBuf},
};

const DRAG_THRESHOLD: f64 = 4.0;

#[derive(Debug, Clone, PartialEq)]
pub enum PetInput {
    PointerDown { x: f64, y: f64 },
    PointerMove { x: f64, y: f64 },
    PointerUp,
    FilesDropped(Vec<DropFile>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PetAction {
    Click,
    MoveWindow { dx: f64, dy: f64 },
    Import(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DropFile {
    path: PathBuf,
    is_regular_file: bool,
}

impl DropFile {
    pub fn new(path: impl Into<PathBuf>, is_regular_file: bool) -> Self {
        Self {
            path: path.into(),
            is_regular_file,
        }
    }
}

#[derive(Debug, Default)]
pub struct InputState {
    pointer_origin: Option<(f64, f64)>,
    dragging: bool,
}

impl InputState {
    pub fn reduce(&mut self, input: PetInput) -> Vec<PetAction> {
        match input {
            PetInput::PointerDown { x, y } if x.is_finite() && y.is_finite() => {
                self.pointer_origin = Some((x, y));
                self.dragging = false;
                Vec::new()
            }
            PetInput::PointerDown { .. } => {
                self.pointer_origin = None;
                self.dragging = false;
                Vec::new()
            }
            PetInput::PointerMove { x, y } => {
                let Some((origin_x, origin_y)) = self.pointer_origin else {
                    return Vec::new();
                };
                if !x.is_finite() || !y.is_finite() {
                    return Vec::new();
                }
                let dx = x - origin_x;
                let dy = y - origin_y;
                if !self.dragging && dx.hypot(dy) < DRAG_THRESHOLD {
                    return Vec::new();
                }
                self.dragging = true;
                vec![PetAction::MoveWindow { dx, dy }]
            }
            PetInput::PointerUp => {
                let clicked = self.pointer_origin.take().is_some() && !self.dragging;
                self.dragging = false;
                if clicked {
                    vec![PetAction::Click]
                } else {
                    Vec::new()
                }
            }
            PetInput::FilesDropped(files) => canonicalize_supported_drop(&files)
                .ok()
                .flatten()
                .map(|path| vec![PetAction::Import(path)])
                .unwrap_or_default(),
        }
    }
}

pub fn is_supported_print_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".gcode.3mf") || lower.ends_with(".3mf") || lower.ends_with(".gcode")
}

pub fn first_supported_drop(files: &[DropFile]) -> Option<&Path> {
    files
        .iter()
        .find(|file| {
            file.is_regular_file && file.path.is_absolute() && is_supported_print_path(&file.path)
        })
        .map(|file| file.path.as_path())
}

pub fn canonicalize_supported_drop(files: &[DropFile]) -> io::Result<Option<PathBuf>> {
    first_supported_drop(files)
        .map(Path::canonicalize)
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::{
        canonicalize_supported_drop, first_supported_drop, DropFile, InputState, PetAction,
        PetInput,
    };
    use std::{fs, path::PathBuf};

    fn temp_drop_dir() -> PathBuf {
        std::env::temp_dir().join(format!("pet-drop-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn moving_the_pet_never_emits_an_import() {
        let mut state = InputState::default();
        assert_eq!(
            state.reduce(PetInput::PointerDown { x: 20.0, y: 20.0 }),
            vec![]
        );
        assert_eq!(
            state.reduce(PetInput::PointerMove { x: 240.0, y: 80.0 }),
            vec![PetAction::MoveWindow {
                dx: 220.0,
                dy: 60.0
            }]
        );
        assert!(state
            .reduce(PetInput::PointerUp)
            .iter()
            .all(|action| !matches!(action, PetAction::Import(_))));
    }

    #[test]
    fn a_press_shorter_than_four_logical_pixels_emits_a_click() {
        let mut state = InputState::default();
        assert!(state
            .reduce(PetInput::PointerDown { x: 10.0, y: 10.0 })
            .is_empty());
        assert!(state
            .reduce(PetInput::PointerMove { x: 13.0, y: 10.0 })
            .is_empty());
        assert_eq!(state.reduce(PetInput::PointerUp), vec![PetAction::Click]);
    }

    #[test]
    fn drop_selection_is_pure_and_uses_only_the_first_supported_regular_file() {
        let files = vec![
            DropFile::new("/tmp/readme.txt", true),
            DropFile::new("/tmp/folder.3mf", false),
            DropFile::new("relative.gcode", true),
            DropFile::new("/tmp/plate.gcode.3mf", true),
            DropFile::new("/tmp/second.3mf", true),
        ];

        assert_eq!(
            first_supported_drop(&files),
            Some(PathBuf::from("/tmp/plate.gcode.3mf").as_path())
        );
    }

    #[test]
    fn drop_canonicalizes_only_the_selected_supported_file() {
        let directory = temp_drop_dir();
        let nested = directory.join("nested");
        fs::create_dir_all(&nested).unwrap();
        let supported = directory.join("plate.gcode.3mf");
        let second = directory.join("second.3mf");
        fs::write(&supported, b"first").unwrap();
        fs::write(&second, b"second").unwrap();
        let files = vec![
            DropFile::new(directory.join("missing.txt"), true),
            DropFile::new(nested.join("..").join("plate.gcode.3mf"), true),
            DropFile::new(second, true),
        ];

        assert_eq!(
            canonicalize_supported_drop(&files).unwrap(),
            Some(supported.canonicalize().unwrap())
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_broken_first_supported_file_does_not_fall_through_to_a_later_file() {
        let directory = temp_drop_dir();
        fs::create_dir_all(&directory).unwrap();
        let second = directory.join("second.3mf");
        fs::write(&second, b"second").unwrap();
        let files = vec![
            DropFile::new(directory.join("missing.gcode"), true),
            DropFile::new(second, true),
        ];

        assert!(canonicalize_supported_drop(&files).is_err());

        fs::remove_dir_all(directory).unwrap();
    }
}
