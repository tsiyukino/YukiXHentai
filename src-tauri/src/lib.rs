mod commands;
mod config;
mod db;
mod download;
mod http;
mod images;
mod library;
mod models;

use std::sync::Arc;

use commands::{DataLocalDir, DefaultCacheDir, DownloadCancellation, PageThumbCancellation, SyncCursor, ThumbClient};
use config::ConfigState;
use db::DbState;
use db::library::LibraryDbState;
use download::ImageDownloadQueue;
use download::local_queue::LocalDownloadQueue;
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

    // On Windows, dirs::cache_dir() == dirs::data_local_dir(), so appending "yukixhentai"
    // alone would put cache files in the same directory as permanent data (library/).
    // We add a "cache" subdirectory so that {data_dir}/cache/ and {data_dir}/library/
    // are always distinct and clearing the cache can never touch library files.
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| data_dir.clone())
        .join("yukixhentai")
        .join("cache");

    // iOS: all paths must exist inside the app sandbox before use.
    // The dirs crate applies macOS conventions on iOS (Library/Application Support, Library/Caches)
    // but does not create the directories — we must do so explicitly.
    #[cfg(target_os = "ios")]
    {
        let _ = std::fs::create_dir_all(&config_dir);
        let _ = std::fs::create_dir_all(&data_dir);
        let _ = std::fs::create_dir_all(&cache_dir);
    }

    let config_state = ConfigState::load(config_dir.clone());

    // Build HTTP clients using stored cookies (if already logged in from a previous session).
    let (queue_client, local_queue_client, thumb_client_init) = {
        let config = config_state.config.lock().unwrap();
        if let Some(cookies) = config.auth.to_cookies() {
            let c1 = http::build_client(&cookies).ok();
            let c2 = http::build_client(&cookies).ok();
            let c3 = http::build_client(&cookies).ok();
            (c1, c2, c3)
        } else {
            (None, None, None)
        }
    };

    // Shared thumbnail client — persists across scroll batches so the connection
    // pool to s.exhentai.org / ehgt.org stays alive between requests.
    let thumb_client = ThumbClient::new();
    if let Some(c) = thumb_client_init {
        // Synchronously pre-populate using block_in_place since we're not in async yet.
        // Use a one-shot runtime just to drive the async set().
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(thumb_client.set(c));
    }

    let db_state = DbState::open(data_dir.clone()).expect("Failed to open database");
    // library.db lives inside the library folder, not alongside yukixhentai.db.
    let library_dir_path = {
        let config = config_state.config.lock().unwrap();
        crate::library::library_dir(&config, &data_dir)
    };
    let library_db_state = LibraryDbState::open(library_dir_path).expect("Failed to open library database");
    let library_db_state_arc = Arc::new(library_db_state);
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
    let data_local_dir_state = DataLocalDir {
        path: data_dir.clone(),
    };

    // Wrap shared state in Arc for the download queue.
    let rate_limiter_arc = Arc::new(rate_limiter);
    let gdata_rate_limiter_arc = Arc::new(gdata_rate_limiter);
    let db_state_arc = Arc::new(db_state);
    let db_state_for_exit = db_state_arc.clone(); // kept for on-exit cleanup
    let history_retention_days = {
        let config = config_state.config.lock().unwrap();
        config.history.retention_days
    };
    let originals_cache_arc = Arc::new(originals_cache);

    tauri::Builder::default()
        .plugin(tauri_plugin_edge_to_edge::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(config_state)
        .manage(db_state_arc.clone() as Arc<DbState>)
        .manage(library_db_state_arc.clone() as Arc<LibraryDbState>)
        .manage(rate_limiter_arc.clone() as Arc<RateLimiter>)
        .manage(gdata_rate_limiter_arc.clone() as Arc<GdataRateLimiter>)
        .manage(thumb_cache)
        .manage(page_thumb_cache)
        .manage(originals_cache_arc.clone() as Arc<OriginalsCache>)
        .manage(sync_cursor)
        .manage(page_thumb_cancellation)
        .manage(download_cancellation)
        .manage(default_cache_dir_state)
        .manage(data_local_dir_state)
        .manage(thumb_client)
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
            // Create the local download queue with its own HTTP client.
            let local_client = local_queue_client.unwrap_or_else(|| reqwest::Client::new());
            let config_snapshot = {
                use tauri::Manager;
                app.state::<ConfigState>().config.lock().unwrap().clone()
            };
            let local_queue = LocalDownloadQueue::new(
                app.handle().clone(),
                local_client,
                rate_limiter_arc.clone(),
                library_db_state_arc.clone(),
                data_dir.clone(),
                config_snapshot,
            );
            app.manage(local_queue);

            Ok(())
        })
        // Mobile login: intercept E-Hentai login page loads to extract cookies
        // via a sentinel redirect, then emit them to the frontend.
        // Builder::on_page_load is the correct Tauri 2.x API — receives &Webview<R>.
        // The body is gated by cfg(not(desktop)) at runtime via early return on desktop.
        .on_page_load(|webview, payload| {
            // No-op on desktop — only active on iOS/Android.
            #[cfg(desktop)]
            {
                let _ = (webview, payload);
                return;
            }
            #[cfg(not(desktop))]
            {
                use tauri::{webview::PageLoadEvent, Emitter, Manager};
                if payload.event() != PageLoadEvent::Finished {
                    return;
                }
                let url = payload.url().to_string();
                const COOKIE_PARAM: &str = "__exh_cookies__";

                // Inject JS after login pages to extract cookies via sentinel redirect.
                if url.starts_with("https://forums.e-hentai.org/")
                    && !url.contains(COOKIE_PARAM)
                {
                    let js = r#"
                        (function() {
                            var c = encodeURIComponent(document.cookie);
                            if (c.indexOf('ipb_member_id') !== -1 && c.indexOf('ipb_pass_hash') !== -1) {
                                window.location.href = 'https://forums.e-hentai.org/?__exh_cookies__=' + c;
                            }
                        })();
                    "#;
                    let _ = webview.eval(js);
                    return;
                }

                // Intercept the sentinel URL and emit cookies to the frontend.
                if url.contains(COOKIE_PARAM) {
                    if let Ok(parsed) = url::Url::parse(&url) {
                        let cookie_str = parsed
                            .query_pairs()
                            .find(|(k, _)| k == COOKIE_PARAM)
                            .map(|(_, v)| v.into_owned())
                            .unwrap_or_default();

                        let mut ipb_member_id = String::new();
                        let mut ipb_pass_hash = String::new();
                        let mut igneous = String::new();

                        for part in cookie_str.split(';') {
                            let part = part.trim();
                            if let Some(v) = part.strip_prefix("ipb_member_id=") {
                                ipb_member_id = v.to_string();
                            } else if let Some(v) = part.strip_prefix("ipb_pass_hash=") {
                                ipb_pass_hash = v.to_string();
                            } else if let Some(v) = part.strip_prefix("igneous=") {
                                if v != "mystery" && v != "deleted" {
                                    igneous = v.to_string();
                                }
                            }
                        }

                        let _ = webview.app_handle().emit("webview-login-cookies", serde_json::json!({
                            "ipb_member_id": ipb_member_id,
                            "ipb_pass_hash": ipb_pass_hash,
                            "igneous": igneous,
                        }));

                        // Navigate back to the app.
                        let _ = webview.navigate("tauri://localhost".parse().unwrap());
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::login,
            commands::open_login_window,
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
            commands::get_library_dir,
            commands::set_library_dir,
            commands::clear_image_cache,
            commands::start_enrichment,
            commands::search_exhentai,
            commands::get_search_history,
            commands::clear_search_history,
            commands::search_tags_autocomplete,
            commands::get_favorite_status,
            commands::add_favorite,
            commands::remove_favorite,
            commands::fetch_favorites,
            commands::get_favorite_folders,
            commands::get_read_cache_stats,
            commands::set_read_cache_max_mb,
            commands::clear_read_cache,
            commands::get_history_retention_days,
            commands::set_history_retention_days,
            commands::get_local_galleries,
            commands::get_local_gallery_pages,
            commands::update_gallery_metadata,
            commands::reorder_local_pages,
            commands::insert_local_pages,
            commands::remove_local_page,
            commands::set_local_gallery_cover,
            commands::import_local_folder,
            commands::confirm_import_local_folder,
            commands::parse_download_queue_json,
            commands::resolve_gallery_token,
            commands::submit_download_queue,
            commands::get_download_queue_status,
            commands::pause_download_queue,
            commands::resume_download_queue,
            commands::cancel_download_queue,
            commands::delete_local_gallery,
            commands::sync_local_gallery,
            commands::cancel_mobile_login,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app, event| {
            if let tauri::RunEvent::Exit = event {
                // Auto-clean old reading history before clearing ephemeral data.
                if let Err(e) = db_state_for_exit.clean_old_reading_history(history_retention_days) {
                    tracing::warn!("Failed to clean reading history on exit: {}", e);
                }
                // Clear ephemeral browse DB on exit (keeps search_history, reading_sessions, reading_progress).
                if let Err(e) = db_state_for_exit.clear_all() {
                    tracing::warn!("Failed to clear yukixhentai.db on exit: {}", e);
                }
                // Clear cache directory contents (but not the directory itself).
                // On iOS, cache eviction is managed by the OS — we skip aggressive
                // cleanup on exit to avoid conflicts with backgrounding lifecycle.
                #[cfg(not(target_os = "ios"))]
                if cache_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                        for entry in entries.flatten() {
                            let p = entry.path();
                            if p.is_dir() {
                                let _ = std::fs::remove_dir_all(&p);
                            } else {
                                let _ = std::fs::remove_file(&p);
                            }
                        }
                    }
                }
            }
        });
}
