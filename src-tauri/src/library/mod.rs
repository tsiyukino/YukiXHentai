/// Local gallery library management.
/// Provides functions for determining library/gallery folder paths,
/// sanitizing titles, and reading/writing metadata.json sidecar files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::AppConfig;

/// Metadata for a locally-imported gallery, stored as metadata.json in the gallery folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalGalleryMeta {
    pub gid: Option<i64>,
    pub token: Option<String>,
    pub title: String,
    pub title_jpn: Option<String>,
    pub category: String,
    pub uploader: Option<String>,
    pub description: Option<String>,
    /// Origin site identifier (e.g. "exhentai"). None for manually imported galleries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// Remote gallery ID on the origin site.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_gid: Option<i64>,
    /// Tags in "namespace:value" or "value" format.
    pub tags: Vec<String>,
    pub pages: Vec<LocalPageMeta>,
}

/// Metadata for a single page in a locally-imported gallery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPageMeta {
    pub index: usize,
    pub filename: String,
    pub source_url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Determine the root library directory.
/// Falls back to data_local_dir/library if config has no override.
pub fn library_dir(config: &AppConfig, data_local_dir: &Path) -> PathBuf {
    if let Some(ref dir) = config.storage.library_dir {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    data_local_dir.join("library")
}

/// Determine the folder path for a specific gallery.
/// Format: {library_dir}/{gid}/
pub fn gallery_folder(config: &AppConfig, data_local_dir: &Path, gid: i64) -> PathBuf {
    let lib = library_dir(config, data_local_dir);
    lib.join(format!("{}", gid))
}

/// Strip filesystem-unsafe characters from a title and truncate to 80 chars.
pub fn sanitize_title(title: &str) -> String {
    let sanitized: String = title
        .chars()
        .map(|c| match c {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' | '/' | '\\' => '_',
            c => c,
        })
        .collect();
    // Trim trailing dots and spaces (Windows doesn't like those at end of dir names).
    let trimmed = sanitized.trim_end_matches(|c: char| c == '.' || c == ' ');
    if trimmed.len() > 80 {
        trimmed[..80].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Write metadata.json to a gallery folder.
pub fn write_metadata_json(folder: &Path, meta: &LocalGalleryMeta) -> Result<(), String> {
    let path = folder.join("metadata.json");
    let json = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Read metadata.json from a gallery folder.
/// Returns None if no metadata.json file exists.
pub fn read_metadata_json(folder: &Path) -> Result<Option<LocalGalleryMeta>, String> {
    let path = folder.join("metadata.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let meta: LocalGalleryMeta = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(Some(meta))
}

/// Natural-sort comparator for filenames.
/// Splits filenames into digit and non-digit runs and compares digit runs numerically.
pub fn natural_sort_key(s: &str) -> Vec<NaturalSortPart> {
    let mut parts = Vec::new();
    let mut chars = s.chars().peekable();
    while chars.peek().is_some() {
        if chars.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            let num: String = std::iter::from_fn(|| {
                chars.next_if(|c| c.is_ascii_digit())
            })
            .collect();
            parts.push(NaturalSortPart::Num(num.parse::<u64>().unwrap_or(u64::MAX)));
        } else {
            let text: String = std::iter::from_fn(|| {
                chars.next_if(|c| !c.is_ascii_digit())
            })
            .collect();
            parts.push(NaturalSortPart::Text(text.to_lowercase()));
        }
    }
    parts
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
pub enum NaturalSortPart {
    Text(String),
    Num(u64),
}

/// Image extensions considered valid for import.
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif", "bmp"];

/// Check if a filename has a recognized image extension.
pub fn is_image_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    IMAGE_EXTENSIONS.iter().any(|ext| lower.ends_with(&format!(".{}", ext)))
}

/// Scan a folder for image files, sorted naturally by filename.
/// Returns (filename, full_path) pairs.
pub fn scan_image_files(folder: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let entries = std::fs::read_dir(folder).map_err(|e| e.to_string())?;
    let mut images: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if is_image_file(name) {
                        return Some((name.to_string(), path));
                    }
                }
            }
            None
        })
        .collect();

    images.sort_by(|(a, _), (b, _)| natural_sort_key(a).cmp(&natural_sort_key(b)));
    Ok(images)
}
