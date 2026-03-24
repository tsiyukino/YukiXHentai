# ExHentai JSON API
> External facts — do NOT modify without user permission

## showpage API
- **Endpoint:** POST `https://exhentai.org/api.php`
- **Request body:** `{ method: "showpage", gid: number, page: number, imgkey: string, showkey: string }`
- **page** is 1-indexed
- **Response:** JSON with `i3` field containing HTML fragment
- **i3 fragment structure:** Contains `<img id="img" src="URL">` for the image URL, and `<a id="loadfail" onclick="return nl('KEY')">` for the nl reload key
- **i3 parsing:** Backslash escapes must be stripped (`i3.replace('\\', "")`) before HTML parsing
- **509 detection:** If image URL in i3 contains `509.gif`, the user is rate limited
- **Notes:** Requires exhentai.org cookies (not e-hentai). Much faster than full HTML page fetch. No page view cost beyond the showkey extraction.

## gdata API (gallery metadata)
- **Endpoint:** POST `https://api.e-hentai.org/api.php`
- **Request body:** `{ method: "gdata", gidlist: [[gid, token], ...], namespace: 1 }`
- **Max per request:** 25 galleries
- **Response:** JSON with `gmetadata` array containing full gallery metadata
- **Fields returned:** title, title_jpn, category, thumb, uploader, posted, filecount, filesize, rating, tags (namespaced), and more
- **Rate limiting:** ~4-5 sequential requests allowed, then must wait ~5 seconds before continuing
- **No page view cost**

## nl retry mechanism
- On image load failure, the image viewer page provides a reload key via `<a id="loadfail" onclick="return nl('KEY')">`
- Appending `?nl=KEY` to the image viewer page URL requests an alternate server
- This gives a different image URL from a different content server
