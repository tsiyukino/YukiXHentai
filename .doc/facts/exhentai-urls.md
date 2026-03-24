# ExHentai URL Patterns
> External facts — do NOT modify without user permission

## Site URLs
- Main site: `https://exhentai.org/`
- E-Hentai (public mirror): `https://e-hentai.org/`
- API endpoint (exhentai): `https://exhentai.org/api.php`
- API endpoint (e-hentai): `https://api.e-hentai.org/api.php`
- Thumbnail CDN: `ehgt.org`

## Gallery detail page
- URL: `https://exhentai.org/g/{gid}/{token}/`
- Paginated: `https://exhentai.org/g/{gid}/{token}/?p={page}`
- `page` is 0-indexed detail page number (each page shows ~20 image entries)

## Image viewer page
- URL pattern: `https://exhentai.org/s/{imgkey}/{gid}-{page}`
- `imgkey` is extracted from the page URL path segment after `/s/`
- `page` is 1-indexed in the URL

## Listing page (main gallery list)
- First page: `https://exhentai.org/`
- Pagination: Cursor-based. `?page=N` does NOT work — ExHentai silently ignores it and returns page 0 content.
- Next page URL comes from `#unext` anchor element in the HTML response.
- Previous page URL comes from `#uprev` anchor element.
- The cursor URL is opaque; always use whatever `#unext` href provides.
