use std::fs;
use std::path::{Path, PathBuf};

/// Content-addressable cache for full-size original images.
/// Structure: {cache_dir}/originals/{ab}/{cd}/{hex_gid}_{page_index}.{ext}
pub struct OriginalsCache {
    base_dir: PathBuf,
}

impl OriginalsCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            base_dir: cache_dir.join("originals"),
        }
    }

    /// Compute the sharded path for a gallery image.
    /// Uses hex gid for sharding, with page index in the filename.
    pub fn path_for_image(&self, gid: i64, page_index: i32, ext: &str) -> PathBuf {
        let hex = format!("{:016x}", gid);
        let dir1 = &hex[0..2];
        let dir2 = &hex[2..4];
        self.base_dir
            .join(dir1)
            .join(dir2)
            .join(format!("{}_{:04}.{}", hex, page_index, ext))
    }

    /// Save image bytes. Returns the path string.
    /// Rejects empty or obviously non-image data.
    pub fn save(&self, gid: i64, page_index: i32, data: &[u8], ext: &str) -> Result<String, String> {
        if data.is_empty() {
            return Err("Image data is empty".into());
        }
        if data.starts_with(b"<") || data.starts_with(b"<!") {
            return Err("Image data appears to be HTML, not an image".into());
        }
        let path = self.path_for_image(gid, page_index, ext);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, data).map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Check if image is already cached. Returns path if it exists.
    /// Checks common extensions since we may not know the ext ahead of time.
    pub fn find_cached(&self, gid: i64, page_index: i32) -> Option<String> {
        for ext in &["jpg", "png", "webp", "gif"] {
            let path = self.path_for_image(gid, page_index, ext);
            if path.exists() {
                return Some(path.to_string_lossy().to_string());
            }
        }
        None
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

/// Cache for page thumbnails (gallery detail page previews).
/// Structure: {cache_dir}/page-thumbs/{gid}/{page_number}.jpg
pub struct PageThumbCache {
    base_dir: PathBuf,
}

impl PageThumbCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            base_dir: cache_dir.join("page-thumbs"),
        }
    }

    /// Compute the path for a page thumbnail.
    pub fn path_for_page(&self, gid: i64, page_index: i32) -> PathBuf {
        self.base_dir
            .join(gid.to_string())
            .join(format!("{}.jpg", page_index))
    }

    /// Save page thumbnail bytes. Returns the path string.
    pub fn save(&self, gid: i64, page_index: i32, data: &[u8]) -> Result<String, String> {
        let path = self.path_for_page(gid, page_index);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, data).map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Check if a page thumbnail is already cached. Returns path if it exists.
    pub fn find_cached(&self, gid: i64, page_index: i32) -> Option<String> {
        let path = self.path_for_page(gid, page_index);
        if path.exists() {
            Some(path.to_string_lossy().to_string())
        } else {
            None
        }
    }

    /// Remove all cached page thumbnails for a gallery.
    pub fn clear_gallery(&self, gid: i64) -> Result<(), String> {
        let dir = self.base_dir.join(gid.to_string());
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}

/// Content-addressable cache for thumbnails.
/// Structure: {cache_dir}/thumbs/{ab}/{cd}/{abcdef...}.jpg
#[derive(Clone)]
pub struct ThumbCache {
    base_dir: PathBuf,
}

impl ThumbCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            base_dir: cache_dir.join("thumbs"),
        }
    }

    /// Compute the sharded path for a given gallery ID.
    /// Uses hex-encoded gid to shard into directories.
    pub fn path_for_gid(&self, gid: i64) -> PathBuf {
        let hex = format!("{:016x}", gid);
        let dir1 = &hex[0..2];
        let dir2 = &hex[2..4];
        self.base_dir.join(dir1).join(dir2).join(format!("{}.jpg", hex))
    }

    /// Save thumbnail bytes to the cache. Returns the path.
    /// Rejects empty or obviously non-image data (HTML error pages).
    pub fn save(&self, gid: i64, data: &[u8]) -> Result<String, String> {
        if data.is_empty() {
            return Err("Thumbnail data is empty".into());
        }
        // Reject HTML error pages saved as images.
        if data.starts_with(b"<") || data.starts_with(b"<!") {
            return Err("Thumbnail data appears to be HTML, not an image".into());
        }
        let path = self.path_for_gid(gid);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&path, data).map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().to_string())
    }

    /// Check if a thumbnail already exists for this gid.
    pub fn exists(&self, gid: i64) -> bool {
        let path = self.path_for_gid(gid);
        path.exists() && fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false)
    }

    /// Check if a thumbnail exists and is valid (nonzero size, not HTML).
    /// Used by download_thumbs_sequential to skip already-cached thumbnails.
    pub fn exists_valid(&self, gid: i64) -> bool {
        let path = self.path_for_gid(gid);
        if !path.exists() {
            return false;
        }
        match fs::metadata(&path) {
            Ok(meta) if meta.len() > 0 => true,
            _ => {
                // Remove corrupted/empty file so it can be re-downloaded.
                let _ = fs::remove_file(&path);
                false
            }
        }
    }

    /// Get the path string if the thumbnail exists.
    pub fn get_path(&self, gid: i64) -> Option<String> {
        let path = self.path_for_gid(gid);
        if path.exists() {
            Some(path.to_string_lossy().to_string())
        } else {
            None
        }
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }
}
