mod commands;
mod config;
mod db;
mod download;
mod http;
mod images;
mod models;

use std::sync::Arc;

use commands::{DefaultCacheDir, DownloadCancellation, PageThumbCancellation, SyncCursor};
use config::ConfigState;
use db::DbState;
use download::ImageDownloadQueue;
use http::{GdataRateLimiter, RateLimiter};
use images::{OriginalsCache, PageThumbCache, ThumbCache};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing for structured logging.
    tracing_subscriber::fmt::init();

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("yukixhentai");

    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("yukixhentai");

    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| data_dir.clone())
        .join("yukixhentai");

    let config_state = ConfigState::load(config_dir.clone());

    // Build an HTTP client for the download queue using stored cookies.
    let queue_client = {
        let config = config_state.config.lock().unwrap();
        if let Some(cookies) = config.auth.to_cookies() {
            http::build_client(&cookies).ok()
        } else {
            None
        }
    };

    let db_state = DbState::open(data_dir).expect("Failed to open database");
    let rate_limiter = RateLimiter::new(1000); // 1s minimum between requests
    let gdata_rate_limiter = GdataRateLimiter::new(4, 5); // 4 requests burst, then 5s cooldown
    let thumb_cache = ThumbCache::new(cache_dir.clone());
    let page_thumb_cache = PageThumbCache::new(cache_dir.clone());
    let originals_cache = OriginalsCache::new(cache_dir.clone());
    let sync_cursor = SyncCursor {
        next_url: std::sync::Mutex::new(None),
    };
    let page_thumb_cancellation = PageThumbCancellation {
        active_gid: std::sync::Mutex::new(None),
    };
    let download_cancellation = DownloadCancellation::new();
    let default_cache_dir_state = DefaultCacheDir {
        path: cache_dir.clone(),
    };

    // Wrap shared state in Arc for the download queue.
    let rate_limiter_arc = Arc::new(rate_limiter);
    let gdata_rate_limiter_arc = Arc::new(gdata_rate_limiter);
    let db_state_arc = Arc::new(db_state);
    let originals_cache_arc = Arc::new(originals_cache);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(config_state)
        .manage(db_state_arc.clone() as Arc<DbState>)
        .manage(rate_limiter_arc.clone() as Arc<RateLimiter>)
        .manage(gdata_rate_limiter_arc.clone() as Arc<GdataRateLimiter>)
        .manage(thumb_cache)
        .manage(page_thumb_cache)
        .manage(originals_cache_arc.clone() as Arc<OriginalsCache>)
        .manage(sync_cursor)
        .manage(page_thumb_cancellation)
        .manage(download_cancellation)
        .manage(default_cache_dir_state)
        .setup(move |app| {
            use tauri::Manager;
            // Create the download queue now that we have an AppHandle.
            let client = queue_client.unwrap_or_else(|| {
                // Fallback: build a client without cookies (will fail on download, but
                // won't crash). User must login first.
                reqwest::Client::new()
            });
            let queue = ImageDownloadQueue::new(
                app.handle().clone(),
                client,
                rate_limiter_arc.clone(),
                db_state_arc.clone(),
                originals_cache_arc.clone(),
            );
            app.manage(queue);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::login,
            commands::logout,
            commands::get_auth_status,
            commands::sync_gallery_page,
            commands::sync_galleries,
            commands::sync_next_page,
            commands::download_thumbnails_for_gids,
            commands::reset_sync_cursor,
            commands::get_galleries,
            commands::get_galleries_by_gids,
            commands::search_galleries,
            commands::save_preset,
            commands::get_presets,
            commands::delete_preset,
            commands::fetch_gallery_metadata,
            commands::get_page_thumbnail,
            commands::set_active_detail_gallery,
            commands::get_gallery_pages,
            commands::get_gallery_pages_batch,
            commands::get_gallery_image,
            commands::cancel_image_downloads,
            commands::register_download_session,
            commands::update_read_progress,
            commands::get_read_progress,
            commands::get_read_progress_batch,
            commands::start_reading_session,
            commands::end_reading_session,
            commands::get_reading_history,
            commands::resolve_thumb_path,
            commands::get_detail_preview_size,
            commands::set_detail_preview_size,
            commands::get_theme,
            commands::set_theme,
            commands::get_cache_dir,
            commands::set_cache_dir,
            commands::clear_image_cache,
            commands::start_enrichment,
            commands::search_exhentai,
            commands::get_search_history,
            commands::clear_search_history,
            commands::search_tags_autocomplete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
