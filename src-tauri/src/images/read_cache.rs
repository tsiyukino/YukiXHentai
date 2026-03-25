/// Helpers for tracking originals cache entries in the DB read_cache_index table.
/// Called by command handlers after saving/finding cached images.

use crate::db::DbState;

/// Compute a cache key from gid and page_index.
pub fn cache_key_for(gid: i64, page_index: i32) -> String {
    format!("{}_{}", gid, page_index)
}

/// Record a newly-saved cache entry and evict LRU entries if needed.
/// max_bytes: upper limit for the originals cache (from config.storage.read_cache_max_mb * 1024 * 1024).
pub fn track_save(db: &DbState, cache_key: &str, file_path: &str, max_bytes: i64) -> Result<(), String> {
    let size_bytes = std::fs::metadata(file_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    db.read_cache_upsert(cache_key, file_path, size_bytes, now)?;

    // Evict LRU entries if over budget.
    if max_bytes > 0 {
        let evicted = db.read_cache_evict_lru(max_bytes)?;
        for (_, evict_path) in evicted {
            let _ = std::fs::remove_file(&evict_path);
        }
    }

    Ok(())
}

/// Bump the last_access timestamp for a cache hit.
pub fn track_access(db: &DbState, cache_key: &str) -> Result<(), String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    db.read_cache_touch(cache_key, now)
}
