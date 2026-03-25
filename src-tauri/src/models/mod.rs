use serde::{Deserialize, Serialize};

/// ExHentai authentication cookies provided by the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExhCookies {
    pub ipb_member_id: String,
    pub ipb_pass_hash: String,
    pub igneous: String,
}

/// Result of a login validation attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResult {
    pub success: bool,
    pub message: String,
}

/// A gallery entry parsed from ExHentai.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gallery {
    pub gid: i64,
    pub token: String,
    pub title: String,
    pub title_jpn: Option<String>,
    pub category: String,
    pub thumb_url: String,
    /// Local path to cached thumbnail, if downloaded.
    pub thumb_path: Option<String>,
    pub uploader: Option<String>,
    /// Unix timestamp.
    pub posted: i64,
    pub rating: f64,
    pub file_count: i32,
    pub file_size: Option<i64>,
    pub tags: Vec<Tag>,
    /// Whether this is a locally-imported gallery (1 = local).
    #[serde(default)]
    pub is_local: Option<i32>,
    /// Gallery description (from API or local import).
    #[serde(default)]
    pub description: Option<String>,
    /// Origin site identifier (e.g. "exhentai"). Set on locally-downloaded galleries.
    #[serde(default)]
    pub origin: Option<String>,
    /// Remote gallery ID on the origin site. For ExHentai downloads this equals gid.
    #[serde(default)]
    pub remote_gid: Option<i64>,
}

/// A namespaced tag (e.g. "female:glasses").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub namespace: String,
    pub name: String,
}

impl Tag {
    /// Parse "namespace:name" format. If no colon, namespace is "misc".
    pub fn parse(raw: &str) -> Self {
        match raw.split_once(':') {
            Some((ns, name)) => Self {
                namespace: ns.to_string(),
                name: name.to_string(),
            },
            None => Self {
                namespace: "misc".to_string(),
                name: raw.to_string(),
            },
        }
    }

    pub fn full_name(&self) -> String {
        format!("{}:{}", self.namespace, self.name)
    }
}

/// Result of a sync operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub galleries_synced: usize,
    pub has_next_page: bool,
    pub message: String,
}

/// Progress event emitted during multi-page sync.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    /// Current listing page being fetched (1-based).
    pub current_page: u32,
    /// Total listing pages to fetch.
    pub total_pages: u32,
    /// Number of thumbnails downloaded so far.
    pub thumbs_downloaded: usize,
    /// Total thumbnails to download.
    pub thumbs_total: usize,
    /// Total galleries synced so far.
    pub galleries_synced: usize,
    /// Status message.
    pub message: String,
    /// Whether the sync is complete.
    pub done: bool,
}

/// A page of gallery results from the local DB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryPage {
    pub galleries: Vec<Gallery>,
    pub total_count: i64,
}

// ── Search & Filter types ─────────────────────────────────────────────────

/// A tag filter entry: namespace + name, used for include/exclude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagFilter {
    pub namespace: String,
    pub name: String,
}

/// Filter parameters for searching galleries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterParams {
    /// Free text search (matched against FTS5: titles + tags).
    pub query: Option<String>,
    /// Tags that must be present.
    pub tags_include: Vec<TagFilter>,
    /// Tags that must NOT be present.
    pub tags_exclude: Vec<TagFilter>,
    /// Only these categories (empty = all).
    pub categories: Vec<String>,
    /// Minimum rating (inclusive).
    pub rating_min: Option<f64>,
    /// Maximum rating (inclusive).
    pub rating_max: Option<f64>,
    /// Minimum page count (inclusive).
    pub pages_min: Option<i32>,
    /// Maximum page count (inclusive).
    pub pages_max: Option<i32>,
    /// Earliest posted date (Unix timestamp, inclusive).
    pub date_min: Option<i64>,
    /// Latest posted date (Unix timestamp, inclusive).
    pub date_max: Option<i64>,
    /// Filter by language tag (e.g. "english", "japanese").
    pub language: Option<String>,
    /// Filter by uploader name (exact match).
    pub uploader: Option<String>,
}

/// Sort direction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl Default for SortDirection {
    fn default() -> Self {
        Self::Desc
    }
}

/// A single sort criterion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortField {
    /// Column name: "posted", "rating", "file_count", "title", "category".
    pub field: String,
    pub direction: SortDirection,
}

/// Sort parameters supporting multi-level sort.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SortParams {
    pub fields: Vec<SortField>,
}

/// A saved filter preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterPreset {
    pub id: i64,
    pub name: String,
    pub filter: FilterParams,
    pub sort: SortParams,
}

// ── Reader types ──────────────────────────────────────────────────────────

/// A single page entry in a gallery (links to an image viewer page).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryPageEntry {
    /// 0-based page index.
    pub page_index: i32,
    /// URL to the image viewer page (e.g. "/s/hash/gid-page").
    pub page_url: String,
    /// Local cached path to the full-size image, if downloaded.
    pub image_path: Option<String>,
    /// Thumbnail URL from the gallery detail page.
    pub thumb_url: Option<String>,
    /// Image key for the showpage API (extracted from page URL).
    pub imgkey: Option<String>,
}

/// Full page list for a gallery, returned by get_gallery_pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryPages {
    pub gid: i64,
    pub token: String,
    pub title: String,
    pub pages: Vec<GalleryPageEntry>,
    pub total_pages: i32,
    /// Showkey for the showpage API (extracted from gallery detail JS).
    pub showkey: Option<String>,
}

/// Read progress for a gallery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadProgress {
    pub gid: i64,
    /// Last 0-based page index the user was on.
    pub last_page_read: i32,
    /// Total pages in this gallery.
    pub total_pages: i32,
    /// Unix timestamp of last read.
    pub last_read_at: i64,
    /// Whether the user has read all pages.
    pub is_completed: bool,
}

/// A reading session log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingSession {
    pub id: i64,
    pub gid: i64,
    pub opened_at: i64,
    pub closed_at: Option<i64>,
    pub pages_read: i32,
}

/// Result of a single-page sync (sync_next_page).
/// Returns the full list of galleries fetched so the frontend can append directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPageResult {
    pub galleries: Vec<Gallery>,
    pub has_more: bool,
}

/// Event payload when a thumbnail finishes downloading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailReadyEvent {
    pub gid: i64,
    pub path: String,
}

/// Result of get_gallery_pages_batch — a single detail page worth of entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryPagesBatchResult {
    pub gid: i64,
    /// The page entries in this batch (up to ~20).
    pub pages: Vec<GalleryPageEntry>,
    /// Showkey extracted from this detail page (present on first page).
    pub showkey: Option<String>,
    /// Total number of images in the gallery.
    pub total_pages: i32,
    /// Whether there is a next detail page to fetch.
    pub has_next_page: bool,
    /// The detail page number that was fetched (0-based).
    pub detail_page: u32,
}

/// Event payload for a batch of gallery page entries (emitted during get_gallery_pages).
/// Allows the frontend to start loading thumbnails before all detail pages are fetched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryPagesBatchEvent {
    pub gid: i64,
    /// The page entries in this batch.
    pub pages: Vec<GalleryPageEntry>,
    /// Showkey extracted from the first detail page (only present on first batch).
    pub showkey: Option<String>,
    /// Total pages in the gallery (may increase as more detail pages are fetched).
    pub total_pages: i32,
}

/// Event payload for image download progress (from the download queue).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageDownloadProgressEvent {
    pub gid: i64,
    pub page_index: i32,
    /// Status: "queued", "downloading", "done", "error", "rate_limited".
    pub status: String,
    /// Local file path when status is "done".
    pub path: Option<String>,
    /// Error message when status is "error" or "rate_limited".
    pub error: Option<String>,
}

// ── ExHentai server-side search types ─────────────────────────────────

/// Advanced search options for ExHentai server-side search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdvancedSearchOptions {
    /// Search gallery names (default true).
    pub search_name: Option<bool>,
    /// Search tags (default true).
    pub search_tags: Option<bool>,
    /// Search gallery descriptions.
    pub search_description: Option<bool>,
    /// Show expunged galleries.
    pub show_expunged: Option<bool>,
    /// Search torrent filenames (f_storr).
    pub search_torrent_filenames: Option<bool>,
    /// Only show galleries with torrents (f_sto).
    pub only_with_torrents: Option<bool>,
    /// Search low-power tags (f_sdt1).
    pub search_low_power_tags: Option<bool>,
    /// Search downvoted tags (f_sdt2).
    pub search_downvoted_tags: Option<bool>,
    /// Minimum star rating filter (f_sr + f_srdd). Values: 2–5. None = disabled.
    pub minimum_rating: Option<u8>,
    /// Minimum page count (f_sp + f_spf). None = no minimum.
    pub min_pages: Option<u32>,
    /// Maximum page count (f_sp + f_spt). None = no maximum.
    pub max_pages: Option<u32>,
}

/// A tag suggestion returned by the autocomplete command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagSuggestion {
    pub namespace: String,
    pub name: String,
}

/// Result from an ExHentai server-side search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExhSearchResult {
    pub galleries: Vec<Gallery>,
    pub has_more: bool,
    /// Full URL for the next page of results (from #unext href).
    /// Frontend passes this back for cursor-based pagination.
    pub next_url: Option<String>,
}

/// A search history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHistoryEntry {
    pub id: i64,
    pub query: String,
    pub searched_at: i64,
}

// ── Favorites types ────────────────────────────────────────────────────

/// One of the 10 cloud favorite folders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteFolder {
    /// 0–9 folder index.
    pub index: u8,
    pub name: String,
    pub count: i32,
}

/// Cached cloud favorite entry stored in the local DB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudFavorite {
    pub gid: i64,
    pub token: String,
    /// Folder index 0–9.
    pub favcat: u8,
    /// Personal note (may be empty).
    pub favnote: String,
    /// Unix timestamp when added/updated locally.
    pub added_at: i64,
}

/// Favorite status for a single gallery, returned by get_favorite_status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteStatus {
    pub gid: i64,
    /// None = not favorited; Some(0–9) = folder index.
    pub favcat: Option<u8>,
    /// Current note (empty if not favorited or no note set).
    pub favnote: String,
}

/// Result of a favorites page fetch (browse cloud favorites).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoritesResult {
    pub galleries: Vec<Gallery>,
    pub folders: Vec<FavoriteFolder>,
    pub has_more: bool,
    pub next_url: Option<String>,
}

// ── Local gallery types ────────────────────────────────────────────────────

/// A single page from a locally-imported gallery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPage {
    pub gid: i64,
    pub page_index: i32,
    pub file_path: String,
    pub source_url: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

/// Patch struct for updating gallery metadata fields.
/// Only Some fields are updated; None fields are left unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryMetadataPatch {
    pub title: Option<String>,
    pub title_jpn: Option<String>,
    pub category: Option<String>,
    pub uploader: Option<String>,
    pub description: Option<String>,
    pub tags_add: Option<Vec<Tag>>,
    pub tags_remove: Option<Vec<Tag>>,
}

/// Read cache statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadCacheStats {
    pub used_bytes: i64,
    pub max_bytes: i64,
    pub file_count: i64,
}

/// Preview of a folder being imported as a local gallery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub detected_title: String,
    pub detected_gid: Option<i64>,
    pub detected_token: Option<String>,
    pub metadata_found: bool,
    pub page_count: usize,
    pub sample_filenames: Vec<String>,
}

/// An entry in the download queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub gid: i64,
    pub token: Option<String>,
    pub title: Option<String>,
    pub already_local: bool,
}

/// Result of resolving a gallery token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedGallery {
    pub gid: i64,
    pub token: Option<String>,
    pub title: Option<String>,
    pub error: Option<String>,
}

/// A single entry to submit for batch download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitEntry {
    pub gid: i64,
    pub token: String,
}

/// Result of a batch download queue submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitResult {
    pub queued: i64,
    pub skipped_already_local: i64,
}

/// Status of the local download queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadQueueStatus {
    pub queued: i64,
    pub downloading: i64,
    pub completed: i64,
    pub failed: i64,
    pub current_gid: Option<i64>,
    pub current_title: Option<String>,
    pub current_page: Option<i32>,
    pub total_pages: Option<i32>,
}
