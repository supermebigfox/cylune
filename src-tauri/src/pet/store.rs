use crate::{
    db::AppDatabase,
    error::{AppError, Result},
    pet::{PetFps, PetMode, PetSettings, PetSettingsPatch},
};
use rusqlite::{OptionalExtension, Transaction};

const DEFAULT_SIZE: u16 = 220;
const MIN_SIZE: u16 = 120;
const MAX_SIZE: u16 = 360;

pub struct PetStore;

impl PetStore {
    pub fn load(database: &AppDatabase) -> Result<PetSettings> {
        let settings = PetSettings {
            mode: parse_mode(setting(database, "pet_mode")?)?,
            size: parse_size(setting(database, "pet_size")?)?,
            fps: parse_fps(setting(database, "pet_fps")?)?,
            visible: parse_visible(setting(database, "pet_visible")?)?,
            x: parse_coordinate(setting(database, "pet_x")?)?,
            y: parse_coordinate(setting(database, "pet_y")?)?,
            display_id: parse_display_id(setting(database, "pet_display_id")?)?,
        };
        validate(&settings)?;
        Ok(settings)
    }

    pub fn apply(database: &AppDatabase, patch: PetSettingsPatch) -> Result<PetSettings> {
        let current = Self::load(database)?;
        let mut next = current.clone();

        if let Some(mode) = patch.mode {
            next.mode = mode;
        }
        if let Some(size) = patch.size {
            next.size = size;
        }
        if let Some(fps) = patch.fps {
            next.fps = fps;
        }
        if let Some(visible) = patch.visible {
            next.visible = visible;
        }
        if patch.reset_position == Some(true) {
            next.x = None;
            next.y = None;
            next.display_id = None;
        } else {
            if let Some(x) = patch.x {
                next.x = Some(x);
            }
            if let Some(y) = patch.y {
                next.y = Some(y);
            }
            if let Some(display_id) = patch.display_id {
                next.display_id = Some(display_id);
            }
        }

        validate(&next)?;

        let transaction = database.connection.unchecked_transaction()?;
        write_changes(
            &transaction,
            &current,
            &next,
            patch.reset_position == Some(true),
        )?;
        transaction.commit()?;
        Ok(next)
    }
}

fn setting(database: &AppDatabase, key: &str) -> Result<Option<String>> {
    Ok(database
        .connection
        .query_row(
            "SELECT setting_value FROM app_settings WHERE setting_key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?)
}

fn parse_mode(value: Option<String>) -> Result<PetMode> {
    match value.as_deref().unwrap_or("lite") {
        "real" => Ok(PetMode::Real),
        "lite" => Ok(PetMode::Lite),
        _ => Err(AppError::InvalidPetSettings),
    }
}

fn parse_size(value: Option<String>) -> Result<u16> {
    let size = match value {
        Some(value) => value.parse().map_err(|_| AppError::InvalidPetSettings)?,
        None => DEFAULT_SIZE,
    };
    if !(MIN_SIZE..=MAX_SIZE).contains(&size) {
        return Err(AppError::InvalidPetSettings);
    }
    Ok(size)
}

fn parse_fps(value: Option<String>) -> Result<PetFps> {
    match value.as_deref().unwrap_or("auto") {
        "auto" => Ok(PetFps::Auto),
        "fps30" => Ok(PetFps::Fps30),
        "fps60" => Ok(PetFps::Fps60),
        _ => Err(AppError::InvalidPetSettings),
    }
}

fn parse_visible(value: Option<String>) -> Result<bool> {
    match value.as_deref().unwrap_or("true") {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(AppError::InvalidPetSettings),
    }
}

fn parse_coordinate(value: Option<String>) -> Result<Option<f64>> {
    value
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| AppError::InvalidPetSettings)
        })
        .transpose()
}

fn parse_display_id(value: Option<String>) -> Result<Option<u64>> {
    value
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| AppError::InvalidPetSettings)
        })
        .transpose()
}

fn validate(settings: &PetSettings) -> Result<()> {
    if !(MIN_SIZE..=MAX_SIZE).contains(&settings.size)
        || settings.x.is_some_and(|x| !x.is_finite())
        || settings.y.is_some_and(|y| !y.is_finite())
        || !matches!(
            (settings.x, settings.y, settings.display_id),
            (None, None, None) | (Some(_), Some(_), Some(_))
        )
    {
        return Err(AppError::InvalidPetSettings);
    }
    Ok(())
}

fn write_changes(
    transaction: &Transaction<'_>,
    current: &PetSettings,
    next: &PetSettings,
    reset_position: bool,
) -> Result<()> {
    if current.mode != next.mode {
        upsert(transaction, "pet_mode", mode_name(next.mode))?;
    }
    if current.size != next.size {
        upsert(transaction, "pet_size", &next.size.to_string())?;
    }
    if current.fps != next.fps {
        upsert(transaction, "pet_fps", fps_name(next.fps))?;
    }
    if current.visible != next.visible {
        upsert(transaction, "pet_visible", &next.visible.to_string())?;
    }

    if reset_position {
        transaction.execute(
            "DELETE FROM app_settings WHERE setting_key IN ('pet_x', 'pet_y', 'pet_display_id')",
            [],
        )?;
    } else {
        if current.x != next.x {
            upsert(transaction, "pet_x", &next.x.unwrap().to_string())?;
        }
        if current.y != next.y {
            upsert(transaction, "pet_y", &next.y.unwrap().to_string())?;
        }
        if current.display_id != next.display_id {
            upsert(
                transaction,
                "pet_display_id",
                &next.display_id.unwrap().to_string(),
            )?;
        }
    }
    Ok(())
}

fn upsert(transaction: &Transaction<'_>, key: &str, value: &str) -> Result<()> {
    transaction.execute(
        "INSERT INTO app_settings(setting_key, setting_value) VALUES (?1, ?2)
         ON CONFLICT(setting_key) DO UPDATE SET setting_value = excluded.setting_value,
         updated_at = CURRENT_TIMESTAMP",
        [key, value],
    )?;
    Ok(())
}

fn mode_name(mode: PetMode) -> &'static str {
    match mode {
        PetMode::Real => "real",
        PetMode::Lite => "lite",
    }
}

fn fps_name(fps: PetFps) -> &'static str {
    match fps {
        PetFps::Auto => "auto",
        PetFps::Fps30 => "fps30",
        PetFps::Fps60 => "fps60",
    }
}

#[cfg(test)]
mod tests {
    use super::PetStore;
    use crate::{
        db::AppDatabase,
        pet::{PetFps, PetMode, PetSettings, PetSettingsPatch},
    };

    #[test]
    fn defaults_are_safe_and_valid() {
        let db = AppDatabase::open_in_memory().unwrap();

        assert_eq!(
            PetStore::load(&db).unwrap(),
            PetSettings {
                mode: PetMode::Lite,
                size: 220,
                fps: PetFps::Auto,
                visible: true,
                x: None,
                y: None,
                display_id: None,
            }
        );
    }

    #[test]
    fn rejects_size_and_unknown_enum_values_without_partial_write() {
        let db = AppDatabase::open_in_memory().unwrap();

        assert_eq!(
            PetStore::apply(
                &db,
                PetSettingsPatch {
                    size: Some(119),
                    ..Default::default()
                },
            )
            .unwrap_err()
            .code(),
            "invalid_pet_settings"
        );
        assert_eq!(PetStore::load(&db).unwrap().size, 220);

        db.connection
            .execute(
                "INSERT INTO app_settings(setting_key,setting_value) VALUES ('pet_mode','unknown')",
                [],
            )
            .unwrap();
        assert_eq!(
            PetStore::load(&db).unwrap_err().code(),
            "invalid_pet_settings"
        );
    }

    #[test]
    fn rejects_size_above_the_upper_bound_without_partial_write() {
        let db = AppDatabase::open_in_memory().unwrap();

        assert_eq!(
            PetStore::apply(
                &db,
                PetSettingsPatch {
                    size: Some(361),
                    ..Default::default()
                },
            )
            .unwrap_err()
            .code(),
            "invalid_pet_settings"
        );
        assert_eq!(PetStore::load(&db).unwrap().size, 220);
    }

    #[test]
    fn validates_all_or_nothing_positions_and_finite_coordinates() {
        let db = AppDatabase::open_in_memory().unwrap();

        assert_eq!(
            PetStore::apply(
                &db,
                PetSettingsPatch {
                    x: Some(-40.0),
                    ..Default::default()
                },
            )
            .unwrap_err()
            .code(),
            "invalid_pet_settings"
        );
        assert_eq!(PetStore::load(&db).unwrap().x, None);

        assert_eq!(
            PetStore::apply(
                &db,
                PetSettingsPatch {
                    x: Some(f64::INFINITY),
                    y: Some(220.0),
                    display_id: Some(9),
                    ..Default::default()
                },
            )
            .unwrap_err()
            .code(),
            "invalid_pet_settings"
        );
        assert_eq!(PetStore::load(&db).unwrap().display_id, None);
    }

    #[test]
    fn persists_position_and_reset_clears_all_position_keys() {
        let db = AppDatabase::open_in_memory().unwrap();

        let settings = PetStore::apply(
            &db,
            PetSettingsPatch {
                x: Some(-400.0),
                y: Some(220.0),
                display_id: Some(9),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            (settings.x, settings.y, settings.display_id),
            (Some(-400.0), Some(220.0), Some(9))
        );

        let reset = PetStore::apply(
            &db,
            PetSettingsPatch {
                reset_position: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((reset.x, reset.y, reset.display_id), (None, None, None));
        assert_eq!(PetStore::load(&db).unwrap(), reset);
    }
}
