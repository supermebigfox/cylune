use std::{
    fs::{self, File, OpenOptions},
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};

use image::{ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::error::{AppError, Result};

const MAX_COMPRESSED_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_EXTRACTED_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_IMAGE_WIDTH: u32 = 8_192;
const MAX_IMAGE_HEIGHT: u32 = 8_192;
const MAX_DECODED_IMAGE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAsset {
    pub asset_id: String,
    pub relative_path: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub byte_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValidatedImage {
    pub extension: &'static str,
    pub mime_type: &'static str,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct MediaStore {
    app_data_root: PathBuf,
}

impl MediaStore {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        let media_root = root.join("media");
        if media_root.exists() && fs::symlink_metadata(&media_root)?.file_type().is_symlink() {
            return Err(AppError::InvalidFile);
        }
        fs::create_dir_all(media_root)?;
        Ok(Self {
            app_data_root: root,
        })
    }

    pub fn extract_image(&self, archive_path: &Path, entry: &str) -> Result<Option<MediaAsset>> {
        validate_entry_name(entry)?;
        let archive_file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(archive_file).map_err(|_| AppError::InvalidFile)?;
        let mut archive_entry = match archive.by_name(entry) {
            Ok(entry) => entry,
            Err(zip::result::ZipError::FileNotFound) => return Ok(None),
            Err(_) => return Err(AppError::InvalidFile),
        };

        if archive_entry.is_dir()
            || archive_entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            || archive_entry.compressed_size() > MAX_COMPRESSED_ENTRY_BYTES
            || archive_entry.name() != entry
        {
            return Err(AppError::InvalidFile);
        }

        let declared_size = archive_entry.size();
        if declared_size > MAX_COMPRESSED_ENTRY_BYTES {
            return Err(AppError::InvalidFile);
        }
        let bytes = read_entry_bytes(&mut archive_entry, declared_size)?;
        let validated = validate_image_bytes(&bytes)?;

        let asset_id = format!("{:x}", Sha256::digest(&bytes));
        let relative_path = format!(
            "media/{}/{}.{}",
            &asset_id[..2],
            asset_id,
            validated.extension
        );
        let destination = self.app_data_root.join(&relative_path);
        self.persist(&destination, &bytes)?;

        Ok(Some(MediaAsset {
            asset_id,
            relative_path,
            mime_type: validated.mime_type.to_owned(),
            width: validated.width,
            height: validated.height,
            byte_size: bytes.len() as u64,
        }))
    }

    pub(crate) fn persist_verified(&self, relative_path: &str, bytes: &[u8]) -> Result<bool> {
        validate_entry_name(relative_path)?;
        let relative = Path::new(relative_path);
        let mut components = relative.components();
        if components.next() != Some(Component::Normal("media".as_ref()))
            || components.clone().count() != 2
        {
            return Err(AppError::InvalidFile);
        }
        let destination = self.app_data_root.join(relative);
        self.persist(&destination, bytes)
    }

    fn persist(&self, destination: &Path, bytes: &[u8]) -> Result<bool> {
        let media_root = self.app_data_root.join("media");
        ensure_real_directory(&media_root)?;
        let parent = destination.parent().ok_or(AppError::InvalidFile)?;
        ensure_real_directory(parent)?;
        match fs::symlink_metadata(destination) {
            Ok(_) => {
                validate_existing(destination, bytes)?;
                return Ok(false);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let temporary = parent.join(format!(
            ".{}.{}.tmp",
            destination
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(AppError::InvalidFile)?,
            uuid::Uuid::new_v4()
        ));
        let write_result = (|| -> Result<bool> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            match fs::hard_link(&temporary, destination) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    validate_existing(destination, bytes)?;
                    fs::remove_file(&temporary)?;
                    return Ok(false);
                }
                Err(error) => return Err(error.into()),
            }
            fs::remove_file(&temporary)?;
            Ok(true)
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        write_result
    }
}

pub(crate) fn validate_image_bytes(bytes: &[u8]) -> Result<ValidatedImage> {
    if bytes.len() as u64 > MAX_EXTRACTED_IMAGE_BYTES {
        return Err(AppError::InvalidFile);
    }
    let (format, extension, mime_type) = image_type(bytes).ok_or(AppError::InvalidFile)?;
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(image_limits());
    let image = reader.decode().map_err(|_| AppError::InvalidFile)?;
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 {
        return Err(AppError::InvalidFile);
    }
    Ok(ValidatedImage {
        extension,
        mime_type,
        width,
        height,
    })
}

fn read_entry_bytes(reader: &mut impl Read, declared_size: u64) -> Result<Vec<u8>> {
    let capacity = usize::try_from(declared_size).map_err(|_| AppError::InvalidFile)?;
    let mut bytes = Vec::with_capacity(capacity);
    reader
        .take(MAX_EXTRACTED_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::InvalidFile)?;
    if bytes.len() > MAX_EXTRACTED_IMAGE_BYTES as usize {
        return Err(AppError::InvalidFile);
    }
    Ok(bytes)
}

fn image_limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_WIDTH);
    limits.max_image_height = Some(MAX_IMAGE_HEIGHT);
    limits.max_alloc = Some(MAX_DECODED_IMAGE_BYTES);
    limits
}

fn validate_entry_name(entry: &str) -> Result<()> {
    if entry.is_empty() || entry.contains('\\') || entry.contains('\0') {
        return Err(AppError::InvalidFile);
    }
    let path = Path::new(entry);
    if !path.is_relative()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppError::InvalidFile);
    }
    Ok(())
}

fn image_type(bytes: &[u8]) -> Option<(ImageFormat, &'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((ImageFormat::Png, "png", "image/png"));
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some((ImageFormat::Jpeg, "jpg", "image/jpeg"));
    }
    None
}

fn validate_existing(path: &Path, expected: &[u8]) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() != expected.len() as u64
        || fs::read(path)? != expected
    {
        return Err(AppError::InvalidFile);
    }
    Ok(())
}

fn ensure_real_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() => {
            Ok(())
        }
        Ok(_) => Err(AppError::InvalidFile),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)?;
            ensure_real_directory(path)
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        io::{Cursor, Write},
        path::{Path, PathBuf},
    };

    use image::{codecs::png::PngEncoder, ColorType, ImageEncoder};
    use zip::write::FileOptions;

    use super::MediaStore;
    use crate::error::AppError;

    #[test]
    fn deduplicates_a_valid_thumbnail_by_content_hash() {
        let root = temporary_root();
        let thumbnail = valid_png();
        let archive_path = write_archive(&root, "thumb.png", &thumbnail);
        let store = MediaStore::new(root.clone()).unwrap();

        let first = store
            .extract_image(&archive_path, "thumb.png")
            .unwrap()
            .unwrap();
        let second = store
            .extract_image(&archive_path, "thumb.png")
            .unwrap()
            .unwrap();

        assert_eq!(first.asset_id, second.asset_id);
        assert_eq!(first.relative_path, second.relative_path);
        assert_eq!(first.mime_type, "image/png");
        assert_eq!((first.width, first.height), (1, 1));
        assert_eq!(first.byte_size, thumbnail.len() as u64);
        assert_eq!(media_file_count(&root), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_archive_entry_paths_that_escape_the_media_root() {
        let root = temporary_root();
        let archive_path = write_archive(&root, "../escape.png", &valid_png());
        let store = MediaStore::new(root.clone()).unwrap();

        let error = store
            .extract_image(&archive_path, "../escape.png")
            .unwrap_err();

        assert!(matches!(error, AppError::InvalidFile));
        assert!(!root.parent().unwrap().join("escape.png").exists());
        assert_eq!(media_file_count(&root), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn returns_none_when_the_requested_thumbnail_is_missing() {
        let root = temporary_root();
        let archive_path = write_archive(&root, "thumb.png", &valid_png());
        let store = MediaStore::new(root.clone()).unwrap();

        assert!(store
            .extract_image(&archive_path, "missing.png")
            .unwrap()
            .is_none());
        assert_eq!(media_file_count(&root), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_corrupt_images_without_writing_a_media_file() {
        let root = temporary_root();
        let archive_path =
            write_archive(&root, "thumb.png", b"\x89PNG\r\n\x1a\ncorrupt image data");
        let store = MediaStore::new(root.clone()).unwrap();

        let error = store.extract_image(&archive_path, "thumb.png").unwrap_err();

        assert!(matches!(error, AppError::InvalidFile));
        assert_eq!(media_file_count(&root), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_entries_larger_than_the_compressed_image_limit() {
        let root = temporary_root();
        let bytes = vec![0; 16 * 1024 * 1024 + 1];
        let archive_path = write_stored_archive(&root, "too-large.png", &bytes);
        let store = MediaStore::new(root.clone()).unwrap();

        let error = store
            .extract_image(&archive_path, "too-large.png")
            .unwrap_err();

        assert!(matches!(error, AppError::InvalidFile));
        assert_eq!(media_file_count(&root), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_reader_output_past_the_image_byte_limit_despite_a_small_declared_size() {
        let mut reader = Cursor::new(vec![0; 16 * 1024 * 1024 + 1]);

        let error = super::read_entry_bytes(&mut reader, 1).unwrap_err();

        assert!(matches!(error, AppError::InvalidFile));
    }

    #[test]
    fn rejects_images_wider_than_the_thumbnail_dimension_limit_without_writing() {
        let root = temporary_root();
        let oversized_png = png(8_193, 1);
        let archive_path = write_archive(&root, "too-wide.png", &oversized_png);
        let store = MediaStore::new(root.clone()).unwrap();

        let error = store
            .extract_image(&archive_path, "too-wide.png")
            .unwrap_err();

        assert!(matches!(error, AppError::InvalidFile));
        assert_eq!(media_file_count(&root), 0);
        fs::remove_dir_all(root).unwrap();
    }

    fn temporary_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("bambu-pools-media-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn valid_png() -> Vec<u8> {
        png(1, 1)
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        let pixels = vec![0; width as usize * height as usize * 4];
        PngEncoder::new(&mut bytes)
            .write_image(&pixels, width, height, ColorType::Rgba8.into())
            .unwrap();
        bytes
    }

    fn write_archive(root: &Path, entry: &str, contents: &[u8]) -> PathBuf {
        write_archive_with_options(root, entry, contents, FileOptions::default())
    }

    fn write_stored_archive(root: &Path, entry: &str, contents: &[u8]) -> PathBuf {
        write_archive_with_options(
            root,
            entry,
            contents,
            FileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
    }

    fn write_archive_with_options(
        root: &Path,
        entry: &str,
        contents: &[u8],
        options: FileOptions,
    ) -> PathBuf {
        let archive_path = root.join("project.3mf");
        let mut archive = zip::ZipWriter::new(File::create(&archive_path).unwrap());
        archive.start_file(entry, options).unwrap();
        archive.write_all(contents).unwrap();
        archive.finish().unwrap();
        archive_path
    }

    fn media_file_count(root: &Path) -> usize {
        let media = root.join("media");
        if !media.exists() {
            return 0;
        }
        fs::read_dir(media)
            .unwrap()
            .flat_map(|entry| fs::read_dir(entry.unwrap().path()).unwrap())
            .count()
    }
}
