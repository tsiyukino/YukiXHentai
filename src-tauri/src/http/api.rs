//! ExHentai JSON API client.
//!
//! Uses https://api.e-hentai.org/api.php for metadata retrieval
//! without consuming page views. Works for both e-hentai and exhentai.

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::http::parser;
use crate::models::{Gallery, Tag};

const API_URL: &str = "https://api.e-hentai.org/api.php";
const EXH_API_URL: &str = "https://exhentai.org/api.php";

/// Maximum galleries per gdata request (API limit).
const GDATA_MAX_BATCH: usize = 25;

// ── Request types ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GdataRequest {
    method: &'static str,
    gidlist: Vec<(i64, String)>,
    namespace: i32,
}

// ── Response types ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GdataResponse {
    gmetadata: Vec<GalleryMetadata>,
}

#[derive(Deserialize)]
struct GalleryMetadata {
    gid: i64,
    token: String,
    title: String,
    title_jpn: Option<String>,
    category: String,
    thumb: String,
    uploader: Option<String>,
    posted: String,
    filecount: String,
    filesize: i64,
    rating: String,
    tags: Vec<String>,
    #[serde(default)]
    expunged: bool,
}

// ── Public API ───────────────────────────────────────────────────────────

/// Fetch gallery metadata via the ExHentai JSON API (method: "gdata").
///
/// This does NOT consume page views and is not subject to the same
/// rate limits as HTML page fetches. Max 25 galleries per request.
pub async fn api_gallery_metadata(
    client: &Client,
    gids_tokens: &[(i64, String)],
) -> Result<Vec<Gallery>, String> {
    let mut all_galleries = Vec::with_capacity(gids_tokens.len());

    // Process in batches of 25 (API limit).
    for chunk in gids_tokens.chunks(GDATA_MAX_BATCH) {
        let gidlist: Vec<(i64, String)> = chunk
            .iter()
            .map(|(gid, token)| (*gid, token.clone()))
            .collect();

        let request = GdataRequest {
            method: "gdata",
            gidlist,
            namespace: 1,
        };

        let response: reqwest::Response = client
            .post(API_URL)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("API request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("API returned status {}", response.status()));
        }

        let data = response
            .json::<GdataResponse>()
            .await
            .map_err(|e| format!("Failed to parse API response: {}", e))?;

        for meta in data.gmetadata {
            if meta.expunged {
                continue;
            }
            all_galleries.push(metadata_to_gallery(meta));
        }
    }

    Ok(all_galleries)
}

// ── showpage API ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ShowPageRequest {
    method: &'static str,
    gid: i64,
    page: i32,
    imgkey: String,
    showkey: String,
}

#[derive(Deserialize)]
struct ShowPageResponse {
    /// HTML fragment containing the image tag and navigation.
    #[serde(default)]
    i3: String,
    /// HTML fragment with original image link (optional).
    #[serde(default)]
    i6: String,
}

/// Resolve an image URL via the showpage API (method: "showpage").
///
/// This is much faster than loading the full image HTML page because
/// it returns a small JSON response with the image URL embedded in
/// an HTML fragment. One request per page (no batch support).
///
/// Parameters:
/// - `gid`: Gallery ID
/// - `page`: 1-indexed page number
/// - `imgkey`: Image key extracted from gallery detail page URLs
/// - `showkey`: Session key extracted from gallery detail page JavaScript
///
/// Uses exhentai.org/api.php (not e-hentai) because we need cookies.
pub async fn api_show_page(
    client: &Client,
    gid: i64,
    page: i32,
    imgkey: &str,
    showkey: &str,
) -> Result<parser::ImagePageResult, String> {
    let request = ShowPageRequest {
        method: "showpage",
        gid,
        page,
        imgkey: imgkey.to_string(),
        showkey: showkey.to_string(),
    };

    let response = client
        .post(EXH_API_URL)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("showpage API request failed: {}", e))?;

    let status = response.status();
    if status.as_u16() == 509 {
        return Err("509_RATE_LIMITED".into());
    }
    if !status.is_success() {
        return Err(format!("showpage API returned status {}", status));
    }

    let data: ShowPageResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse showpage response: {}", e))?;

    if data.i3.is_empty() {
        return Err("showpage API returned empty i3 field".into());
    }

    parser::parse_showpage_response(&data.i3)
}

/// Convert API metadata to our Gallery type.
fn metadata_to_gallery(meta: GalleryMetadata) -> Gallery {
    let tags: Vec<Tag> = meta
        .tags
        .iter()
        .map(|t| Tag::parse(t))
        .collect();

    let posted: i64 = meta.posted.parse().unwrap_or(0);
    let file_count: i32 = meta.filecount.parse().unwrap_or(0);
    let rating: f64 = meta.rating.parse().unwrap_or(0.0);

    Gallery {
        gid: meta.gid,
        token: meta.token,
        title: meta.title,
        title_jpn: meta.title_jpn,
        category: meta.category,
        thumb_url: meta.thumb,
        thumb_path: None,
        uploader: meta.uploader,
        posted,
        rating,
        file_count,
        file_size: Some(meta.filesize),
        tags,
        is_local: None,
        description: None,
        origin: None,
        remote_gid: None,
    }
}
