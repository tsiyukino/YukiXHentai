/// Local gallery download queue.
/// Downloads full galleries from ExHentai and saves them as local galleries.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::Client;
use tauri::{AppHandle, Emitter};

use crate::config::AppConfig;
use crate::db::library::LibraryDbState;
use crate::http::{self, RateLimiter};
use crate::models::{DownloadQueueStatus, Gallery, Tag};

/// A pending download job for a full gallery.
struct LocalDownloadJob {
    gid: i64,
    token: String,
    title: Option<String>,
}

/// Internal mutable state of the queue.
struct QueueState {
    jobs: std::collections::VecDeque<LocalDownloadJob>,
    completed: i64,
    failed: i64,
    current_gid: Option<i64>,
    current_title: Option<String>,
    current_page: Option<i32>,
    total_pages: Option<i32>,
    downloading: i64,
}

/// Local gallery download queue. Runs gallery downloads sequentially.
pub struct LocalDownloadQueue {
    state: Arc<Mutex<QueueState>>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}

impl LocalDownloadQueue {
    pub fn new(
        app: AppHandle,
        client: Client,
        rate_limiter: Arc<RateLimiter>,
        lib_db: Arc<LibraryDbState>,
        data_local_dir: PathBuf,
        config: AppConfig,
    ) -> Self {
        let state = Arc::new(Mutex::new(QueueState {
            jobs: std::collections::VecDeque::new(),
            completed: 0,
            failed: 0,
            current_gid: None,
            current_title: None,
            current_page: None,
            total_pages: None,
            downloading: 0,
        }));
        let paused = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));

        let worker_state = state.clone();
        let worker_paused = paused.clone();
        let worker_cancelled = cancelled.clone();

        tauri::async_runtime::spawn(queue_worker(
            app,
            client,
            rate_limiter,
            lib_db,
            data_local_dir,
            config,
            worker_state,
            worker_paused,
            worker_cancelled,
        ));

        Self { state, paused, cancelled }
    }

    /// Enqueue a gallery for download.
    pub fn enqueue(&self, gid: i64, token: String, title: Option<String>) {
        let mut state = self.state.lock().unwrap();
        // Reset cancelled flag when new jobs are added.
        self.cancelled.store(false, Ordering::Relaxed);
        state.jobs.push_back(LocalDownloadJob { gid, token, title });
    }

    /// Pause the queue (current download completes, next won't start).
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Relaxed);
    }

    /// Resume a paused queue.
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Relaxed);
    }

    /// Cancel pending downloads. If gid is None, cancel everything.
    pub fn cancel(&self, gid: Option<i64>) {
        let mut state = self.state.lock().unwrap();
        match gid {
            Some(g) => {
                state.jobs.retain(|j| j.gid != g);
            }
            None => {
                state.jobs.clear();
                self.cancelled.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Get the current queue status.
    pub fn status(&self) -> DownloadQueueStatus {
        let state = self.state.lock().unwrap();
        DownloadQueueStatus {
            queued: state.jobs.len() as i64,
            downloading: state.downloading,
            completed: state.completed,
            failed: state.failed,
            current_gid: state.current_gid,
            current_title: state.current_title.clone(),
            current_page: state.current_page,
            total_pages: state.total_pages,
        }
    }
}

/// Progress event emitted to the frontend during local gallery download.
#[derive(Debug, Clone, serde::Serialize)]
struct LocalDownloadProgressEvent {
    gid: i64,
    title: Option<String>,
    current_page: i32,
    total_pages: i32,
    status: String, // "downloading" | "done" | "error" | "cancelled"
    error: Option<String>,
}

/// Background worker: polls the queue and processes one gallery at a time.
async fn queue_worker(
    app: AppHandle,
    client: Client,
    rate_limiter: Arc<RateLimiter>,
    lib_db: Arc<LibraryDbState>,
    data_local_dir: PathBuf,
    config: AppConfig,
    state: Arc<Mutex<QueueState>>,
    paused: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
) {
    loop {
        // Wait a bit before polling to avoid busy-loop.
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Check if cancelled.
        if cancelled.load(Ordering::Relaxed) {
            cancelled.store(false, Ordering::Relaxed);
        }

        // Check if paused.
        if paused.load(Ordering::Relaxed) {
            continue;
        }

        // Dequeue next job.
        let job = {
            let mut s = state.lock().unwrap();
            if s.downloading > 0 {
                // Already processing something (shouldn't happen, but guard it).
                continue;
            }
            s.jobs.pop_front()
        };

        let job = match job {
            Some(j) => j,
            None => continue,
        };

        // Mark as downloading.
        {
            let mut s = state.lock().unwrap();
            s.downloading = 1;
            s.current_gid = Some(job.gid);
            s.current_title = job.title.clone();
            s.current_page = Some(0);
            s.total_pages = None;
        }

        let success = download_gallery(
            &app,
            &client,
            &rate_limiter,
            &lib_db,
            &data_local_dir,
            &config,
            &state,
            &paused,
            &cancelled,
            job.gid,
            &job.token,
        )
        .await;

        // Mark done.
        {
            let mut s = state.lock().unwrap();
            s.downloading = 0;
            s.current_gid = None;
            s.current_title = None;
            s.current_page = None;
            s.total_pages = None;
            if success {
                s.completed += 1;
            } else {
                s.failed += 1;
            }
        }

        let _ = app.emit(
            "local-download-progress",
            LocalDownloadProgressEvent {
                gid: job.gid,
                title: job.title.clone(),
                current_page: 0,
                total_pages: 0,
                status: if success { "done" } else { "error" }.to_string(),
                error: None,
            },
        );
    }
}

/// Download a single gallery: fetch metadata + all pages, save to library as local gallery.
/// Returns true on success, false on failure.
async fn download_gallery(
    app: &AppHandle,
    client: &Client,
    rate_limiter: &Arc<RateLimiter>,
    lib_db: &Arc<LibraryDbState>,
    data_local_dir: &PathBuf,
    config: &AppConfig,
    queue_state: &Arc<Mutex<QueueState>>,
    paused: &Arc<AtomicBool>,
    cancelled: &Arc<AtomicBool>,
    gid: i64,
    token: &str,
) -> bool {
    tracing::info!("[local_queue] Starting download for gid={}", gid);

    // Step 1: Fetch gallery metadata via gdata API.
    let meta = match http::api::api_gallery_metadata(client, &[(gid, token.to_string())]).await {
        Ok(mut list) if !list.is_empty() => list.remove(0),
        Ok(_) => {
            tracing::warn!("[local_queue] gdata returned empty for gid={}", gid);
            return false;
        }
        Err(e) => {
            tracing::warn!("[local_queue] gdata failed for gid={}: {}", gid, e);
            return false;
        }
    };

    let title = meta.title.clone();
    let category = meta.category.clone();
    let total_pages = meta.file_count;

    // Update queue state with title + total.
    {
        let mut s = queue_state.lock().unwrap();
        s.current_title = Some(title.clone());
        s.total_pages = Some(total_pages);
    }

    tracing::info!(
        "[local_queue] gid={} title={:?} pages={}",
        gid, title, total_pages
    );

    // Step 2: Fetch all gallery detail pages to get page URLs + imgkeys.
    let mut all_page_urls: Vec<String> = Vec::new();
    let mut all_imgkeys: Vec<String> = Vec::new();
    let mut showkey: Option<String> = None;
    let mut detail_page = 0u32;

    loop {
        if cancelled.load(Ordering::Relaxed) {
            tracing::info!("[local_queue] Cancelled during page fetch for gid={}", gid);
            return false;
        }
        while paused.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        let detail = match http::fetch_gallery_detail(client, rate_limiter, gid, token, Some(detail_page)).await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("[local_queue] detail page {} fetch failed for gid={}: {}", detail_page, gid, e);
                return false;
            }
        };

        if detail_page == 0 {
            showkey = detail.showkey.clone();
        }

        all_page_urls.extend(detail.page_urls);
        all_imgkeys.extend(detail.imgkeys);

        if detail.next_detail_page.is_none() {
            break;
        }
        detail_page += 1;
    }

    let actual_total = all_page_urls.len() as i32;
    {
        let mut s = queue_state.lock().unwrap();
        s.total_pages = Some(actual_total);
    }

    // Step 3: Create the gallery folder.
    let dest_folder = crate::library::gallery_folder(config, data_local_dir, gid);
    if let Err(e) = std::fs::create_dir_all(&dest_folder) {
        tracing::warn!("[local_queue] Failed to create folder {:?}: {}", dest_folder, e);
        return false;
    }

    // Step 4: Download each page image.
    let mut page_data: Vec<(String, Option<String>, Option<i32>, Option<i32>)> = Vec::new();
    let mut page_metas: Vec<crate::library::LocalPageMeta> = Vec::new();

    for (page_index, page_url) in all_page_urls.iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            tracing::info!("[local_queue] Cancelled during image download for gid={}", gid);
            let _ = app.emit(
                "local-download-progress",
                LocalDownloadProgressEvent {
                    gid,
                    title: Some(title.clone()),
                    current_page: page_index as i32,
                    total_pages: actual_total,
                    status: "cancelled".to_string(),
                    error: None,
                },
            );
            return false;
        }
        while paused.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        // Update progress.
        {
            let mut s = queue_state.lock().unwrap();
            s.current_page = Some(page_index as i32 + 1);
        }
        let _ = app.emit(
            "local-download-progress",
            LocalDownloadProgressEvent {
                gid,
                title: Some(title.clone()),
                current_page: page_index as i32 + 1,
                total_pages: actual_total,
                status: "downloading".to_string(),
                error: None,
            },
        );

        // Resolve image URL — use showpage API if we have imgkey + showkey.
        let imgkey = all_imgkeys.get(page_index).map(|k| k.as_str());
        let page_result = if let (Some(ik), Some(ref sk)) = (imgkey, &showkey) {
            match http::api::api_show_page(client, gid, page_index as i32 + 1, ik, sk).await {
                Ok(r) => r,
                Err(_) => match http::fetch_image_url(client, rate_limiter, page_url).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!("[local_queue] Failed to resolve image URL for gid={} page={}: {}", gid, page_index, e);
                        continue;
                    }
                },
            }
        } else {
            match http::fetch_image_url(client, rate_limiter, page_url).await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!("[local_queue] Failed to resolve image URL for gid={} page={}: {}", gid, page_index, e);
                    continue;
                }
            }
        };

        // Download image bytes.
        let bytes = match http::download_image(client, rate_limiter, &page_result.image_url).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!("[local_queue] Failed to download image for gid={} page={}: {}", gid, page_index, e);
                continue;
            }
        };

        // Determine file extension and write to disk.
        let ext = extract_extension(&page_result.image_url);
        let filename = format!("{:04}.{}", page_index, ext);
        let dest_path = dest_folder.join(&filename);

        if let Err(e) = std::fs::write(&dest_path, &bytes) {
            tracing::warn!("[local_queue] Failed to write image {:?}: {}", dest_path, e);
            continue;
        }

        let dest_str = dest_path.to_string_lossy().to_string();
        let (width, height) = read_image_dimensions(&dest_path);
        page_data.push((dest_str.clone(), Some(page_url.clone()), width, height));
        page_metas.push(crate::library::LocalPageMeta {
            index: page_index,
            filename: filename.clone(),
            source_url: Some(page_url.clone()),
            width: width.map(|w| w as u32),
            height: height.map(|h| h as u32),
        });
    }

    if page_data.is_empty() {
        tracing::warn!("[local_queue] No pages downloaded for gid={}", gid);
        return false;
    }

    // Step 5: Download the gallery cover thumbnail into the gallery folder.
    // Uses the thumb URL from gdata metadata. Falls back to the first page image if unavailable.
    let thumb_path = if !meta.thumb_url.is_empty() {
        match http::download_thumbnail(client, &meta.thumb_url).await {
            Ok(bytes) if !bytes.is_empty() => {
                let ext = meta.thumb_url.rsplit('.').next()
                    .and_then(|e| {
                        let e = e.split('?').next().unwrap_or(e).to_lowercase();
                        match e.as_str() {
                            "jpg" | "jpeg" | "png" | "webp" | "gif" => Some(e),
                            _ => None,
                        }
                    })
                    .unwrap_or_else(|| "jpg".to_string());
                let thumb_file = dest_folder.join(format!("cover_thumb.{}", ext));
                match std::fs::write(&thumb_file, &bytes) {
                    Ok(()) => Some(thumb_file.to_string_lossy().to_string()),
                    Err(_) => page_data.first().map(|(fp, _, _, _)| fp.clone()),
                }
            }
            _ => page_data.first().map(|(fp, _, _, _)| fp.clone()),
        }
    } else {
        page_data.first().map(|(file_path, _, _, _)| file_path.clone())
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let tags: Vec<Tag> = meta.tags.clone();

    let gallery = Gallery {
        gid,
        token: token.to_string(),
        title: title.clone(),
        title_jpn: meta.title_jpn.clone(),
        category: category.clone(),
        thumb_url: String::new(),
        thumb_path,
        uploader: meta.uploader.clone(),
        posted: now,
        rating: meta.rating,
        file_count: page_data.len() as i32,
        file_size: meta.file_size,
        tags: tags.clone(),
        is_local: Some(1),
        description: meta.description.clone(),
        origin: Some("exhentai".to_string()),
        remote_gid: Some(gid),
    };

    let folder_str = dest_folder.to_string_lossy().to_string();

    // Step 6: Upsert gallery and pages in DB.
    if let Err(e) = lib_db.upsert_local_gallery(&gallery, &folder_str, meta.description.as_deref()) {
        tracing::warn!("[local_queue] DB upsert failed for gid={}: {}", gid, e);
        return false;
    }

    // Clear existing local pages and insert new ones.
    {
        let conn = match lib_db.conn.lock() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("[local_queue] DB lock failed: {}", e);
                return false;
            }
        };
        let _ = conn.execute(
            "DELETE FROM local_gallery_pages WHERE gid = ?1",
            rusqlite::params![gid],
        );
    }

    if let Err(e) = lib_db.insert_local_pages(gid, &page_data, -1) {
        tracing::warn!("[local_queue] insert_local_pages failed for gid={}: {}", gid, e);
        return false;
    }

    // Step 7: Write metadata.json.
    let meta_file = crate::library::LocalGalleryMeta {
        gid: Some(gid),
        token: Some(token.to_string()),
        title: title.clone(),
        title_jpn: meta.title_jpn.clone(),
        category,
        uploader: meta.uploader.clone(),
        description: meta.description.clone(),
        origin: Some("exhentai".to_string()),
        remote_gid: Some(gid),
        tags: tags.iter().map(|t| t.full_name()).collect(),
        pages: page_metas,
    };
    let _ = crate::library::write_metadata_json(&dest_folder, &meta_file);

    tracing::info!("[local_queue] Download complete for gid={} pages={}", gid, page_data.len());
    true
}

/// Extract file extension from an image URL (jpg/jpeg/png/webp/gif).
fn extract_extension(url: &str) -> String {
    url.rsplit('.')
        .next()
        .and_then(|e| {
            let e = e.split('?').next().unwrap_or(e).to_lowercase();
            match e.as_str() {
                "jpg" | "jpeg" | "png" | "webp" | "gif" => Some(e),
                _ => None,
            }
        })
        .unwrap_or_else(|| "jpg".to_string())
}

/// Read image dimensions using the image crate. Returns (width, height) in i32 or None.
fn read_image_dimensions(path: &std::path::Path) -> (Option<i32>, Option<i32>) {
    match image::image_dimensions(path) {
        Ok((w, h)) => (Some(w as i32), Some(h as i32)),
        Err(_) => (None, None),
    }
}
