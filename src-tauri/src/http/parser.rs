//! HTML parser for ExHentai gallery listing pages.
//!
//! All site-specific HTML assumptions are isolated here.
//! If ExHentai changes its HTML structure, only this file needs updating.

use scraper::{Html, Selector};

use crate::models::{Gallery, Tag};

/// Parsed result from a gallery listing page.
pub struct ListingPage {
    pub galleries: Vec<Gallery>,
    /// Full URL for the next page of results (from #unext href), if available.
    pub next_url: Option<String>,
    /// Full URL for the previous page of results (from #uprev href), if available.
    pub prev_url: Option<String>,
}

/// Parse one page of ExHentai gallery listing HTML (extended mode).
///
/// Expected structure per gallery row in extended mode:
/// ```text
/// <table class="itg glte">
///   <tr>
///     <td class="gl1e">  — thumbnail
///       <a href="/g/{gid}/{token}/"><img src="..." /></a>
///     </td>
///     <td class="gl2e">  — metadata
///       <div><a href="..."><div class="glink">Title</div></a></div>
///       <div class="gl3e">
///         <div> category </div>
///         <div id="posted_{gid}"> date </div>
///         <div class="ir" style="background-position:..."> rating </div>
///         <div> uploader </div>
///         <div> pages </div>
///       </div>
///       <div class="gl4e glte">
///         <div>
///           <div class="gt" title="namespace:tag">...</div>
///           ...
///         </div>
///       </div>
///     </td>
///   </tr>
/// </table>
/// ```
pub fn parse_gallery_listing(html: &str) -> Result<ListingPage, String> {
    let document = Html::parse_document(html);

    // Detect sadpanda / error.
    if html.len() < 500 && !html.contains("<html") {
        return Err("Received sadpanda page — not authenticated.".into());
    }

    let mut galleries = Vec::new();

    // Try extended mode table first, fall back to other layouts.
    let row_sel = sel("table.itg.glte tr, table.itg.gltm tr, div.itg > div.gl1t");

    for row in document.select(&row_sel) {
        if let Some(gallery) = parse_gallery_row(&row) {
            galleries.push(gallery);
        }
    }

    // If extended/minimal mode found nothing, try compact mode.
    if galleries.is_empty() {
        let compact_sel = sel("table.itg.gltc tr");
        for row in document.select(&compact_sel) {
            if let Some(gallery) = parse_gallery_row_compact(&row) {
                galleries.push(gallery);
            }
        }
    }

    // Parse pagination: extract full URL from next/prev page links.
    let next_url = parse_pagination_url(&document, "unext");
    let prev_url = parse_pagination_url(&document, "uprev");

    Ok(ListingPage {
        galleries,
        next_url,
        prev_url,
    })
}

/// Parse a single gallery row from extended or minimal mode.
fn parse_gallery_row(row: &scraper::ElementRef) -> Option<Gallery> {
    // Extract gallery URL → gid + token.
    let link_sel = sel("a[href]");
    let (gid, token) = row
        .select(&link_sel)
        .filter_map(|a| parse_gallery_url(a.value().attr("href")?))
        .next()?;

    // Title from .glink
    let title_sel = sel(".glink");
    let title = row
        .select(&title_sel)
        .next()
        .map(|el| el.text().collect::<String>().trim().to_string())
        .unwrap_or_default();

    if title.is_empty() {
        return None;
    }

    // Thumbnail URL
    let img_sel = sel("img[data-src], img[src]");
    let thumb_url = row
        .select(&img_sel)
        .next()
        .and_then(|img| {
            img.value()
                .attr("data-src")
                .or_else(|| img.value().attr("src"))
        })
        .unwrap_or_default()
        .to_string();

    // Category from .cn or .cs or first child of .gl3e
    let category = extract_category(row);

    // Rating from .ir element's background-position style.
    let rating = extract_rating(row);

    // Uploader: in extended mode, look inside .gl3e children.
    let uploader = extract_uploader(row);

    // Page count: look for "N pages" text.
    let file_count = extract_page_count(row);

    // Posted date: from element with id="posted_*" or date text.
    let posted = extract_posted(row, gid);

    // Tags from .gt or .gtl elements.
    let tags = extract_tags(row);

    Some(Gallery {
        gid,
        token,
        title,
        title_jpn: None,
        category,
        thumb_url,
        thumb_path: None,
        uploader,
        posted,
        rating,
        file_count,
        file_size: None,
        tags,
    })
}

/// Parse a gallery row from compact mode (table.itg.gltc).
fn parse_gallery_row_compact(row: &scraper::ElementRef) -> Option<Gallery> {
    // Compact mode uses a similar structure — delegate to the same logic.
    parse_gallery_row(row)
}

/// Extract gid and token from a gallery URL like "/g/12345/abcdef1234/".
fn parse_gallery_url(href: &str) -> Option<(i64, String)> {
    // Match /g/{gid}/{token}/ pattern.
    let parts: Vec<&str> = href.split('/').filter(|s| !s.is_empty()).collect();
    // Find "g" segment, then take next two.
    let g_pos = parts.iter().position(|&s| s == "g")?;
    let gid: i64 = parts.get(g_pos + 1)?.parse().ok()?;
    let token = parts.get(g_pos + 2)?.to_string();
    Some((gid, token))
}

fn extract_category(row: &scraper::ElementRef) -> String {
    // .cn = full category name, .cs = short category name.
    let cn_sel = sel(".cn");
    if let Some(el) = row.select(&cn_sel).next() {
        return el.text().collect::<String>().trim().to_string();
    }
    let cs_sel = sel(".cs");
    if let Some(el) = row.select(&cs_sel).next() {
        return el.text().collect::<String>().trim().to_string();
    }
    // Extended mode: first div inside .gl3e is often the category.
    let gl3e_sel = sel(".gl3e div:first-child");
    if let Some(el) = row.select(&gl3e_sel).next() {
        let text = el.text().collect::<String>().trim().to_string();
        if !text.is_empty() && text.len() < 30 {
            return text;
        }
    }
    "Unknown".to_string()
}

fn extract_rating(row: &scraper::ElementRef) -> f64 {
    // The .ir element has a style like "background-position:-Xpx -Ypx"
    // X: 0=5★, -16=4.5★, -32=4★, -48=3.5★, -64=3★, -80=2.5★, -96=2★, -112=1.5★, -128=1★, -144=0.5★
    // Y: 0=full star row, -21=half-star variant
    let ir_sel = sel(".ir");
    if let Some(el) = row.select(&ir_sel).next() {
        if let Some(style) = el.value().attr("style") {
            return parse_rating_style(style);
        }
    }
    0.0
}

fn parse_rating_style(style: &str) -> f64 {
    // Parse "background-position:-Xpx -Ypx" or similar.
    let mut x: i32 = 0;
    let mut y: i32 = 0;

    for part in style.split(';') {
        let part = part.trim();
        if part.starts_with("background-position:") {
            let val = part.trim_start_matches("background-position:").trim();
            let nums: Vec<i32> = val
                .split_whitespace()
                .filter_map(|s| s.trim_end_matches("px").parse().ok())
                .collect();
            if let Some(&nx) = nums.first() {
                x = nx;
            }
            if let Some(&ny) = nums.get(1) {
                y = ny;
            }
        }
    }

    // x goes from 0 (5 stars) to -160 (0 stars) in steps of -16.
    let base = 5.0 - (x.unsigned_abs() as f64 / 16.0);
    // If y == -21, subtract 0.5 for the "half star down" row. Wait — actually
    // y=-1 means the second row which represents -0.5.  Different sources say
    // y=-21 means subtract nothing (it's the normal colored star row).
    // Safe approach: if y is significantly negative (< -1), it's the "half-down" row.
    let rating = if y < -1 { base - 0.5 } else { base };
    rating.clamp(0.0, 5.0)
}

fn extract_uploader(row: &scraper::ElementRef) -> Option<String> {
    // Extended mode: uploader is in an <a> inside .gl3e that links to /uploader/...
    let a_sel = sel("a[href*=\"/uploader/\"]");
    if let Some(el) = row.select(&a_sel).next() {
        let text = el.text().collect::<String>().trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }
    // Also check for "(Disowned)" text.
    let gl3e_sel = sel(".gl3e");
    if let Some(el) = row.select(&gl3e_sel).next() {
        let text = el.text().collect::<String>();
        if text.contains("(Disowned)") {
            return Some("(Disowned)".to_string());
        }
    }
    None
}

fn extract_page_count(row: &scraper::ElementRef) -> i32 {
    // Look for text matching "N pages" anywhere in the row.
    let text = row.text().collect::<String>();
    for word in text.split_whitespace() {
        // The pattern is "<number> pages".
        if let Ok(n) = word.parse::<i32>() {
            // Check if next word-ish contains "page".
            if text.contains(&format!("{} page", n)) {
                return n;
            }
        }
    }
    0
}

fn extract_posted(row: &scraper::ElementRef, gid: i64) -> i64 {
    // Look for element with id="posted_{gid}" which contains a date string.
    let id = format!("posted_{}", gid);
    let sel_str = format!("[id=\"{}\"]", id);
    if let Ok(posted_sel) = Selector::parse(&sel_str) {
        if let Some(el) = row.select(&posted_sel).next() {
            let text = el.text().collect::<String>().trim().to_string();
            if let Some(ts) = parse_exh_date(&text) {
                return ts;
            }
        }
    }
    // Fallback: look for date-like text "YYYY-MM-DD HH:MM" in the row.
    let all_text = row.text().collect::<String>();
    for line in all_text.lines() {
        let trimmed = line.trim();
        if trimmed.len() >= 16 && trimmed.len() <= 20 {
            if let Some(ts) = parse_exh_date(trimmed) {
                return ts;
            }
        }
    }
    0
}

/// Parse "YYYY-MM-DD HH:MM" into a Unix timestamp (UTC).
fn parse_exh_date(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 16 {
        return None;
    }
    let year: i32 = s.get(0..4)?.parse().ok()?;
    let month: u32 = s.get(5..7)?.parse().ok()?;
    let day: u32 = s.get(8..10)?.parse().ok()?;
    let hour: u32 = s.get(11..13)?.parse().ok()?;
    let min: u32 = s.get(14..16)?.parse().ok()?;

    // Simple conversion without pulling in chrono:
    // days from epoch to year, then add month/day, then hours/minutes.
    let days = days_from_epoch(year, month, day)?;
    Some(days as i64 * 86400 + hour as i64 * 3600 + min as i64 * 60)
}

fn days_from_epoch(year: i32, month: u32, day: u32) -> Option<i64> {
    if month < 1 || month > 12 || day < 1 || day > 31 {
        return None;
    }
    // Adjusted from Howard Hinnant's algorithm.
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let m = if month <= 2 { month + 9 } else { month - 3 } as i64;
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * m + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe - 719468)
}

fn extract_tags(row: &scraper::ElementRef) -> Vec<Tag> {
    // Tags are in .gt or .gtl elements, with title="namespace:tag".
    let tag_sel = sel(".gt, .gtl, .gtw");
    row.select(&tag_sel)
        .filter_map(|el| {
            let title = el.value().attr("title")?;
            Some(Tag::parse(title))
        })
        .collect()
}

/// Parsed result from a gallery detail page — the list of image page URLs.
pub struct GalleryDetailPage {
    /// Image page URLs extracted from the thumbnail grid (e.g. "/s/hash/gid-N").
    pub page_urls: Vec<String>,
    /// Thumbnail URLs corresponding to each page (from the #gdt thumbnail grid).
    pub thumb_urls: Vec<String>,
    /// Image keys (imgkeys) extracted from page URLs — one per page.
    /// Format: the hash portion from "/s/{imgkey}/{gid}-{page}".
    pub imgkeys: Vec<String>,
    /// Total number of pages (from the page navigation, may exceed urls on one page).
    pub total_pages: i32,
    /// Whether there are more gallery detail pages (multi-page galleries show pages of thumbs).
    pub next_detail_page: Option<String>,
    /// The showkey extracted from the page's JavaScript (var showkey = "...").
    /// Only present on the first detail page (p=0). Used with the showpage API.
    pub showkey: Option<String>,
}

/// Parse a gallery detail page to extract image page URLs.
///
/// Gallery detail URL: `https://exhentai.org/g/{gid}/{token}/`
/// Optionally with `?p=N` for multi-page thumbnail grids.
///
/// Structure (gdtl — large thumbnails, one image per page):
/// ```text
/// <div id="gdt">
///   <div class="gdtl">
///     <a href="https://exhentai.org/s/{hash}/{gid}-{page}">
///       <img src="https://...individual-thumb.jpg" />
///     </a>
///   </div>
/// </div>
/// ```
///
/// Structure (gdtm — normal thumbnails, sprite sheets):
/// ```text
/// <div id="gdt">
///   <div class="gdtm" style="height:NNpx">
///     <div style="width:Wpx;height:Hpx;background:transparent url(SPRITE_URL) -Xpx 0 no-repeat">
///       <a href="https://exhentai.org/s/{hash}/{gid}-{page}"></a>
///     </div>
///   </div>
/// </div>
/// ```
///
/// For gdtm sprites, `thumb_urls` encodes crop info: `sprite:{url}:{offsetX}:{offsetY}:{width}:{height}`
///
/// Pagination: `<table class="ptt"> ... <td> <a href="...?p=N"> </td> ...`
pub fn parse_gallery_detail(html: &str) -> Result<GalleryDetailPage, String> {
    let document = Html::parse_document(html);

    if html.len() < 500 && !html.contains("<html") {
        return Err("Received sadpanda page — not authenticated.".into());
    }

    // Extract image page URLs, thumbnail URLs, and imgkeys from the thumbnail grid.
    //
    // Three thumbnail formats exist:
    // 1. Old gdtl (large): #gdt > .gdtl > a > img  — individual thumbnail per page
    // 2. Old gdtm (sprite): #gdt > .gdtm > div[style] > a  — CSS sprite sheet
    // 3. New format (post Oct 2024): #gdt (with classes) > a > div[style]  — may be sprite or large
    //    In the new format, #gdt has CSS classes and there are NO .gdtm/.gdtl wrappers.
    //    Sprite thumbnails have a negative background-position offset; large ones don't.
    let mut page_urls: Vec<String> = Vec::new();
    let mut thumb_urls: Vec<String> = Vec::new();
    let mut imgkeys: Vec<String> = Vec::new();

    let gdtl_sel = sel("#gdt .gdtl a[href]");
    let gdtm_sel = sel("#gdt .gdtm");

    let gdtl_items: Vec<_> = document.select(&gdtl_sel).collect();
    let gdtm_items: Vec<_> = document.select(&gdtm_sel).collect();

    if !gdtl_items.is_empty() {
        // Old large thumbnail mode — each <a> has a child <img src="...">.
        for link in gdtl_items {
            if let Some(href) = link.value().attr("href") {
                if href.contains("/s/") {
                    page_urls.push(href.to_string());
                    let imgkey = extract_imgkey_from_page_url(href).unwrap_or_default();
                    imgkeys.push(imgkey);
                    let thumb_url = extract_thumb_url_from_gdtl(&link).unwrap_or_default();
                    thumb_urls.push(thumb_url);
                }
            }
        }
    } else if !gdtm_items.is_empty() {
        // Old sprite thumbnail mode — styled div contains the <a> and CSS sprite info.
        let a_sel = sel("a[href]");
        let div_sel = sel("div[style]");
        for gdtm in gdtm_items {
            let styled_div = gdtm.select(&div_sel).next();
            if let Some(link) = gdtm.select(&a_sel).next() {
                if let Some(href) = link.value().attr("href") {
                    if href.contains("/s/") {
                        page_urls.push(href.to_string());
                        let imgkey = extract_imgkey_from_page_url(href).unwrap_or_default();
                        imgkeys.push(imgkey);
                        let thumb_url = styled_div
                            .and_then(|d| d.value().attr("style"))
                            .and_then(extract_sprite_thumb_info)
                            .unwrap_or_default();
                        thumb_urls.push(thumb_url);
                    }
                }
            }
        }
    } else {
        // New format or unknown: #gdt > a[href] with child div[style] for thumbnails.
        // Each <a> contains a <div style="...background:url(SPRITE) -Xpx 0 ...">.
        // If the style has a negative background-position offset → sprite; otherwise → large.
        let fallback_sel = sel("#gdt a[href]");
        let div_sel = sel("div[style]");
        for el in document.select(&fallback_sel) {
            if let Some(href) = el.value().attr("href") {
                if href.contains("/s/") {
                    page_urls.push(href.to_string());
                    let imgkey = extract_imgkey_from_page_url(href).unwrap_or_default();
                    imgkeys.push(imgkey);
                    let thumb_url = extract_thumb_from_new_format(&el, &div_sel);
                    thumb_urls.push(thumb_url);
                }
            }
        }
    }

    // Extract showkey from JavaScript: var showkey="...";
    let showkey = extract_showkey(html);

    // Extract total page count from the page info text or pagination.
    // Look for "Showing X - Y of Z" or count from the last page link.
    let total_pages = extract_total_pages(&document, page_urls.len() as i32);

    // Check for next detail page (?p=N).
    let next_detail_page = extract_next_detail_page(&document);

    Ok(GalleryDetailPage {
        page_urls,
        thumb_urls,
        imgkeys,
        total_pages,
        next_detail_page,
        showkey,
    })
}

/// Extract thumbnail URL from a gdtl (large thumbnail) grid item.
/// The `<a>` element has a child `<img src="...">` with the individual thumbnail URL.
fn extract_thumb_url_from_gdtl(link: &scraper::ElementRef) -> Option<String> {
    let img_sel = sel("img[src]");
    if let Some(img) = link.select(&img_sel).next() {
        if let Some(src) = img.value().attr("src") {
            if src.starts_with("http") {
                return Some(src.to_string());
            }
        }
    }

    // Fallback: check for child div with background style.
    let div_sel = sel("div[style]");
    for div in link.select(&div_sel) {
        if let Some(style) = div.value().attr("style") {
            if let Some(url) = extract_url_from_css_background(style) {
                return Some(url);
            }
        }
    }

    None
}

/// Extract thumbnail info from the new format (post Oct 2024).
///
/// In the new format, each `<a>` in `#gdt` contains a `<div style="...">` with CSS background.
/// Sprite thumbnails have a negative x-offset (e.g., `-100px`); large thumbnails have `0px 0px`.
/// If the style has a non-zero offset → encode as `sprite:{url}:{x}:{y}:{w}:{h}`.
/// If the style has zero offset and appears to be a large thumbnail → return the plain URL.
fn extract_thumb_from_new_format(link: &scraper::ElementRef, div_sel: &Selector) -> String {
    // Look for a child div with inline style containing the CSS background.
    if let Some(div) = link.select(div_sel).next() {
        if let Some(style) = div.value().attr("style") {
            // Try to parse as sprite info first (handles both sprite and large).
            if let Some(sprite) = extract_sprite_thumb_info(style) {
                return sprite;
            }
            // If sprite extraction failed but there's a URL, return it as-is (large thumb).
            if let Some(url) = extract_url_from_css_background(style) {
                return url;
            }
        }
    }
    // Final fallback: check for <img src> (shouldn't happen in new format but safe).
    extract_thumb_url_from_gdtl(link).unwrap_or_default()
}

/// Extract sprite thumbnail info from a gdtm inner div's style attribute.
///
/// CSS format: `width:100px;height:145px;background:transparent url(https://...) -300px 0 no-repeat`
///
/// Returns encoded string: `sprite:{url}:{offsetX}:{offsetY}:{width}:{height}`
/// The offset values are absolute (positive), converted from the negative CSS values.
fn extract_sprite_thumb_info(style: &str) -> Option<String> {
    let url = extract_url_from_css_background(style)?;

    // Parse width from "width:Npx"
    let width = parse_px_value(style, "width")?;
    // Parse height from "height:Npx"
    let height = parse_px_value(style, "height")?;

    // Parse background-position offset from the CSS after the URL.
    // Format: `url(...) -Xpx Ypx` — X is typically negative (rightward offset into sprite).
    let url_end = style.find(&url)? + url.len();
    let after_url = &style[url_end..];
    // Skip past the closing paren.
    let after_paren = after_url.find(')')? + 1;
    let position_str = &after_url[after_paren..];

    // Extract X offset — typically "-300px" or "0px".
    let offset_x = parse_offset_value(position_str, 0);
    let offset_y = parse_offset_value(position_str, 1);

    Some(format!("sprite:{}:{}:{}:{}:{}", url, offset_x, offset_y, width, height))
}

/// Parse a `property:Npx` value from a CSS style string.
fn parse_px_value(style: &str, property: &str) -> Option<u32> {
    // Match property name followed by optional whitespace, colon, optional whitespace, number, "px"
    let prop_start = style.find(property)?;
    let after_prop = &style[prop_start + property.len()..];
    let colon = after_prop.find(':')?;
    let after_colon = after_prop[colon + 1..].trim_start();
    let num_end = after_colon.find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(after_colon.len());
    after_colon[..num_end].parse::<u32>().ok()
}

/// Parse the Nth positional offset value from a CSS background-position string.
/// Returns absolute (positive) pixel offset.
fn parse_offset_value(position_str: &str, index: usize) -> u32 {
    let mut count = 0;
    for part in position_str.split_whitespace() {
        let cleaned = part.trim_end_matches("px").trim_end_matches(';');
        if let Ok(val) = cleaned.parse::<i32>() {
            if count == index {
                return val.unsigned_abs();
            }
            count += 1;
        }
    }
    0
}

/// Extract URL from a CSS background property like `background:transparent url(...) ...`.
fn extract_url_from_css_background(style: &str) -> Option<String> {
    let start = style.find("url(")?;
    let rest = &style[start + 4..];
    let end = rest.find(')')?;
    let url = rest[..end].trim_matches('"').trim_matches('\'').trim();
    if url.starts_with("http") {
        Some(url.to_string())
    } else {
        None
    }
}

/// Extract total page count from gallery detail.
fn extract_total_pages(document: &Html, current_count: i32) -> i32 {
    // Try to find "Showing X - Y of Z" text or "Length: N pages" in the info section.
    // The info section has rows like: <td class="gdt1">Length:</td><td class="gdt2">N pages</td>
    let gdt2_sel = sel("td.gdt2");
    for el in document.select(&gdt2_sel) {
        let text = el.text().collect::<String>();
        if text.contains("page") {
            // Parse "N pages"
            if let Some(n) = text.split_whitespace().next().and_then(|s| s.parse::<i32>().ok()) {
                return n;
            }
        }
    }
    current_count
}

/// Extract next detail page URL from pagination.
fn extract_next_detail_page(document: &Html) -> Option<String> {
    // Pagination table: <table class="ptt">
    // The "current" page has <td class="ptds"> — next page is the <td> after it.
    let ptt_sel = sel("table.ptt td");
    let mut found_current = false;
    for td in document.select(&ptt_sel) {
        let classes = td.value().attr("class").unwrap_or("");
        if classes.contains("ptds") {
            found_current = true;
            continue;
        }
        if found_current {
            // This td is the next page — get its <a> href.
            let a_sel = sel("a[href]");
            if let Some(a) = td.select(&a_sel).next() {
                if let Some(href) = a.value().attr("href") {
                    return Some(href.to_string());
                }
            }
            break;
        }
    }
    None
}

/// Result of parsing an image viewer page.
pub struct ImagePageResult {
    /// The actual full-size image URL.
    pub image_url: String,
    /// The "nl" reload key for alternate server fallback.
    /// Extracted from `<a id="loadfail" onclick="return nl('KEY')">`.
    pub nl_key: Option<String>,
}

/// Parse an image viewer page to extract the actual full-size image URL
/// and the nl (reload) key for alternate server fallback.
///
/// Image viewer URL: `https://exhentai.org/s/{hash}/{gid}-{page}`
///
/// Structure:
/// ```text
/// <div id="i3"><a ...><img id="img" src="https://...actual-image.jpg" ... /></a></div>
/// <a id="loadfail" onclick="return nl('KEY')">Click here if the image fails loading</a>
/// ```
pub fn parse_image_page(html: &str) -> Result<ImagePageResult, String> {
    let document = Html::parse_document(html);

    if html.len() < 500 && !html.contains("<html") {
        return Err("Received sadpanda page — not authenticated.".into());
    }

    let mut image_url: Option<String> = None;

    // Primary: <img id="img" src="...">
    let img_sel = sel("#img");
    if let Some(el) = document.select(&img_sel).next() {
        if let Some(src) = el.value().attr("src") {
            if src.starts_with("http") {
                image_url = Some(src.to_string());
            }
        }
    }

    // Fallback: look for the largest image src in #i3.
    if image_url.is_none() {
        let i3_img_sel = sel("#i3 img[src]");
        if let Some(el) = document.select(&i3_img_sel).next() {
            if let Some(src) = el.value().attr("src") {
                if src.starts_with("http") {
                    image_url = Some(src.to_string());
                }
            }
        }
    }

    let image_url = image_url
        .ok_or_else(|| "Could not find image URL on the image viewer page.".to_string())?;

    // Check for 509 (bandwidth exceeded) image.
    if image_url.contains("509.gif") {
        return Err("509_RATE_LIMITED".into());
    }

    // Extract nl (reload) key from loadfail element.
    // Pattern: onclick="return nl('KEY')"
    let nl_key = extract_nl_key(&document);

    Ok(ImagePageResult { image_url, nl_key })
}

/// Extract the nl (reload) key from the loadfail element.
fn extract_nl_key(document: &Html) -> Option<String> {
    let loadfail_sel = sel("#loadfail");
    let el = document.select(&loadfail_sel).next()?;
    let onclick = el.value().attr("onclick")?;
    // Pattern: return nl('KEY')
    let start = onclick.find("nl('")?;
    let rest = &onclick[start + 4..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

/// Extract the full URL from a pagination link element.
/// ExHentai main listing uses cursor-based pagination (e.g. ?next=<id>).
/// Returns None if the element doesn't exist (no more pages).
/// Returns the absolute URL to fetch for the next/prev page.
fn parse_pagination_url(document: &Html, element_id: &str) -> Option<String> {
    let sel_str = format!("[id=\"{}\"]", element_id);
    let pagination_sel = Selector::parse(&sel_str).ok()?;
    let el = document.select(&pagination_sel).next()?;
    let href = el.value().attr("href")?;
    if href.is_empty() {
        return None;
    }
    // Ensure absolute URL.
    if href.starts_with("http") {
        Some(href.to_string())
    } else {
        Some(format!("https://exhentai.org{}", href))
    }
}

/// Extract the showkey from gallery detail page JavaScript.
/// Looks for: var showkey="abcdef1234"; in the raw HTML.
pub fn extract_showkey(html: &str) -> Option<String> {
    // Pattern: var showkey="VALUE";
    let marker = "var showkey=\"";
    let start = html.find(marker)?;
    let rest = &html[start + marker.len()..];
    let end = rest.find('"')?;
    let key = &rest[..end];
    if !key.is_empty() && key.len() < 32 {
        Some(key.to_string())
    } else {
        None
    }
}

/// Extract the imgkey from a page URL like "/s/{imgkey}/{gid}-{page}" or
/// "https://exhentai.org/s/{imgkey}/{gid}-{page}".
fn extract_imgkey_from_page_url(url: &str) -> Option<String> {
    let parts: Vec<&str> = url.split('/').filter(|s| !s.is_empty()).collect();
    let s_pos = parts.iter().position(|&s| s == "s")?;
    let imgkey = parts.get(s_pos + 1)?;
    if !imgkey.is_empty() && imgkey.len() < 20 {
        Some(imgkey.to_string())
    } else {
        None
    }
}

/// Parse the showpage API response to extract the image URL and nl key.
/// The response JSON has `i3` (HTML fragment with the image) and `i6` (original info).
/// The `i3` field contains: `<img id="img" src="IMAGE_URL" ...>`
/// and potentially `nl('KEY')` for alternate server.
pub fn parse_showpage_response(i3: &str) -> Result<ImagePageResult, String> {
    // The response may have backslash-escaped characters — strip them.
    let cleaned = i3.replace('\\', "");

    // Extract image URL from: id="img" src="URL"
    let image_url = {
        let marker = "id=\"img\" src=\"";
        if let Some(start) = cleaned.find(marker) {
            let rest = &cleaned[start + marker.len()..];
            if let Some(end) = rest.find('"') {
                let url = &rest[..end];
                if url.starts_with("http") {
                    Some(url.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            // Fallback: look for src="http...
            let marker2 = "src=\"http";
            if let Some(start) = cleaned.find(marker2) {
                let rest = &cleaned[start + 5..]; // skip src="
                if let Some(end) = rest.find('"') {
                    Some(rest[..end].to_string())
                } else {
                    None
                }
            } else {
                None
            }
        }
    };

    let image_url = image_url
        .ok_or_else(|| "Could not find image URL in showpage response".to_string())?;

    // Check for 509
    if image_url.contains("509.gif") {
        return Err("509_RATE_LIMITED".into());
    }

    // Extract nl key from: nl('KEY')
    let nl_key = {
        let marker = "nl('";
        if let Some(start) = cleaned.find(marker) {
            let rest = &cleaned[start + marker.len()..];
            if let Some(end) = rest.find('\'') {
                Some(rest[..end].to_string())
            } else {
                None
            }
        } else {
            None
        }
    };

    Ok(ImagePageResult { image_url, nl_key })
}

/// Helper to avoid repeating Selector::parse().unwrap().
fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("invalid CSS selector")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gallery_url() {
        assert_eq!(
            parse_gallery_url("/g/12345/abcdef1234/"),
            Some((12345, "abcdef1234".to_string()))
        );
        assert_eq!(
            parse_gallery_url("https://exhentai.org/g/999/abc123/"),
            Some((999, "abc123".to_string()))
        );
        assert_eq!(parse_gallery_url("/not/a/gallery/"), None);
    }

    #[test]
    fn test_parse_rating_style() {
        assert!((parse_rating_style("background-position:0px 0px") - 5.0).abs() < 0.01);
        assert!((parse_rating_style("background-position:-16px 0px") - 4.0).abs() < 0.01);
        assert!((parse_rating_style("background-position:-32px -21px") - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_exh_date() {
        let ts = parse_exh_date("2024-01-15 12:30").unwrap();
        assert!(ts > 0);
    }

    #[test]
    fn test_tag_parse() {
        let tag = Tag::parse("female:glasses");
        assert_eq!(tag.namespace, "female");
        assert_eq!(tag.name, "glasses");

        let tag2 = Tag::parse("someuntagged");
        assert_eq!(tag2.namespace, "misc");
        assert_eq!(tag2.name, "someuntagged");
    }

    #[test]
    fn test_extract_sprite_thumb_info() {
        // Standard gdtm sprite style.
        let style = "width:100px;height:145px;background:transparent url(https://example.com/sprite.jpg) -300px 0 no-repeat";
        let result = extract_sprite_thumb_info(style).unwrap();
        assert_eq!(result, "sprite:https://example.com/sprite.jpg:300:0:100:145");

        // First thumb in sprite (offset 0).
        let style0 = "width:100px;height:145px;background:transparent url(https://example.com/sprite.jpg) 0px 0 no-repeat";
        let result0 = extract_sprite_thumb_info(style0).unwrap();
        assert_eq!(result0, "sprite:https://example.com/sprite.jpg:0:0:100:145");

        // Quoted URL (new format).
        let style_q = "width:100px;height:150px;background:url(\"https://cdn.example.com/s.jpg\") -200px 0px no-repeat transparent";
        let result_q = extract_sprite_thumb_info(style_q).unwrap();
        assert_eq!(result_q, "sprite:https://cdn.example.com/s.jpg:200:0:100:150");
    }

    #[test]
    fn test_parse_gallery_detail_new_format() {
        // Simulate the new format (post Oct 2024): #gdt has classes, no .gdtm/.gdtl wrappers.
        // Each <a> inside #gdt has a child <div style="..."> with CSS sprite background.
        let html = r#"
        <html><body>
        <div id="gdt" class="some-new-class">
            <a href="https://exhentai.org/s/abc001/12345-1">
                <div style="width:100px;height:145px;background:transparent url(https://cdn.example.com/sprite1.jpg) 0px 0 no-repeat"></div>
            </a>
            <a href="https://exhentai.org/s/abc002/12345-2">
                <div style="width:100px;height:145px;background:transparent url(https://cdn.example.com/sprite1.jpg) -100px 0 no-repeat"></div>
            </a>
            <a href="https://exhentai.org/s/abc003/12345-3">
                <div style="width:100px;height:145px;background:transparent url(https://cdn.example.com/sprite1.jpg) -200px 0 no-repeat"></div>
            </a>
        </div>
        <table class="ptt"><tr><td class="ptds"><a href="?p=0">1</a></td></tr></table>
        <td class="gdt2">3 pages</td>
        </body></html>
        "#;

        let result = parse_gallery_detail(html).unwrap();
        assert_eq!(result.page_urls.len(), 3);
        assert_eq!(result.thumb_urls.len(), 3);

        // Each thumb_url should be unique (different offsets).
        assert_eq!(result.thumb_urls[0], "sprite:https://cdn.example.com/sprite1.jpg:0:0:100:145");
        assert_eq!(result.thumb_urls[1], "sprite:https://cdn.example.com/sprite1.jpg:100:0:100:145");
        assert_eq!(result.thumb_urls[2], "sprite:https://cdn.example.com/sprite1.jpg:200:0:100:145");

        // Imgkeys should be correctly extracted.
        assert_eq!(result.imgkeys, vec!["abc001", "abc002", "abc003"]);
    }

    #[test]
    fn test_parse_gallery_detail_old_gdtm_format() {
        // Old gdtm format: #gdt > .gdtm > div[style] > a[href]
        let html = r#"
        <html><body>
        <div id="gdt">
            <div class="gdtm" style="height:170px">
                <div style="width:100px;height:145px;background:transparent url(https://cdn.example.com/sprite.jpg) 0px 0 no-repeat">
                    <a href="https://exhentai.org/s/key1/999-1"></a>
                </div>
            </div>
            <div class="gdtm" style="height:170px">
                <div style="width:100px;height:145px;background:transparent url(https://cdn.example.com/sprite.jpg) -100px 0 no-repeat">
                    <a href="https://exhentai.org/s/key2/999-2"></a>
                </div>
            </div>
        </div>
        <table class="ptt"><tr><td class="ptds"><a href="?p=0">1</a></td></tr></table>
        <td class="gdt2">2 pages</td>
        </body></html>
        "#;

        let result = parse_gallery_detail(html).unwrap();
        assert_eq!(result.page_urls.len(), 2);
        assert_eq!(result.thumb_urls[0], "sprite:https://cdn.example.com/sprite.jpg:0:0:100:145");
        assert_eq!(result.thumb_urls[1], "sprite:https://cdn.example.com/sprite.jpg:100:0:100:145");
    }
}
