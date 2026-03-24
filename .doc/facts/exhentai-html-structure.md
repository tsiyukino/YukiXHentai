# ExHentai HTML Structure
> External facts — do NOT modify without user permission

## Gallery listing page
- Gallery link titles: `.glink`
- Category icons: `.ir`
- Tags: `.gt` and `.gtl`
- Next page link: `#unext` anchor (href = absolute URL, cursor-based)
- Previous page link: `#uprev` anchor
- If `#unext` is missing, there are no more pages

## Gallery detail page
- **showkey:** Extracted from inline JS: `var showkey="..."` (simple string search, no regex)
- **Image page URLs:** Links in the thumbnail grid, pattern `/s/{imgkey}/{gid}-{page}`

### Thumbnail grid formats (three variants)
1. **Old large (gdtl):** `#gdt > .gdtl > a > img` — individual thumbnail URL per page
2. **Old sprite (gdtm):** `#gdt > .gdtm > div[style] > a` — CSS sprite sheet, each page has unique background-position offset
3. **New format (post Oct 2024):** `#gdt (with classes) > a > div[style]` — no `.gdtm`/`.gdtl` wrappers. May be sprite or large; sprite has negative `background-position` offset, large has `0px 0px`

### Sprite thumbnails
- ~20 thumbnails per sprite image
- Differentiated by `background-position` CSS property (negative values for offset)
- Sprite encoding convention (our format): `sprite:{url}:{offsetX}:{offsetY}:{width}:{height}` — offset values stored as absolute (positive), converted from negative CSS values

## Image viewer page
- Full-size image: `#img` src attribute (primary selector)
- Fallback: `#i3 img[src]`
- Reload key: `#loadfail` element's onclick attribute, pattern `nl('KEY')`
- 509 detection: image URL contains `509.gif`

## Sadpanda detection (cookie validation)
- ExHentai returns "sadpanda" (blank page) when cookies are invalid
- Detection: response body < 200 bytes AND no `<html` tag

## 509 rate limiting
- HTTP 509 status code = bandwidth exceeded
- Response body is a small GIF (`509.gif`)
- This is ExHentai's rate limit signal for image downloads
