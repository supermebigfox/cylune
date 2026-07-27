use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use std::fmt;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    InvalidFile,
    UnslicedProject,
    UnknownGcode,
    StandaloneGcodeProfilesRequired,
    SlotConflict,
    InvalidSlot,
    ArchivedSpool,
    DuplicateJob,
    FileNotStable,
    InvalidJob,
    InvalidMapping,
    InsufficientFilament,
    Database(String),
    Io(String),
}

impl AppError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidFile => "invalid_file",
            Self::UnslicedProject => "unsliced_project",
            Self::UnknownGcode => "unknown_gcode",
            Self::StandaloneGcodeProfilesRequired => "standalone_gcode_profiles_required",
            Self::SlotConflict => "slot_conflict",
            Self::InvalidSlot => "invalid_slot",
            Self::ArchivedSpool => "archived_spool",
            Self::DuplicateJob => "duplicate_job",
            Self::FileNotStable => "file_not_stable",
            Self::InvalidJob => "invalid_job",
            Self::InvalidMapping => "invalid_mapping",
            Self::InsufficientFilament => "insufficient_filament",
            Self::Database(_) => "database",
            Self::Io(_) => "io",
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 1)?;
        state.serialize_field("code", self.code())?;
        state.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::AppError;

    #[test]
    fn serialized_database_error_exposes_only_its_stable_code() {
        let error = AppError::Database(
            "unable to open /Users/robin/Library/Application Support/Bambu Spools/data.db"
                .to_owned(),
        );

        assert_eq!(
            serde_json::to_value(error).unwrap(),
            serde_json::json!({ "code": "database" })
        );
    }
}
