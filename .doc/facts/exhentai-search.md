# ExHentai Search
> External facts — do NOT modify without user permission

## Search URL format
- Base: `https://exhentai.org/?`
- Query params:
  - `f_search` — URL-encoded search query
  - `f_cats` — category exclusion bitmask (0 = show all)
  - `page` — 0-indexed page number (omit for page 0)

## Category bitmask (f_cats)
Each bit **excludes** a category. Add values to exclude multiple.

| Category    | Bit value |
|-------------|-----------|
| Misc        | 1         |
| Doujinshi   | 2         |
| Manga       | 4         |
| Artist CG   | 8         |
| Game CG     | 16        |
| Image Set   | 32        |
| Cosplay     | 64        |
| Asian Porn  | 128       |
| Non-H       | 256       |
| Western     | 512       |

- `f_cats=0` → show all categories
- `f_cats=6` → exclude Doujinshi (2) + Manga (4)

## Advanced search
- Add `advsearch=1` to enable advanced mode
- `f_sname=on` — search gallery names
- `f_stags=on` — search tags
- `f_sdesc=on` — search descriptions
- `f_sh=on` — show expunged galleries

## Tag search syntax
- Namespaced: `artist:"name"`, `female:catgirl`, `language:english`
- Quotes needed for multi-word values: `parody:"series name"`
- Dollar sign for exact match: `artist:"name$"`

## Pagination
- Uses `page=N` starting from 0 (different from homepage cursor-based)
- Next page link: `#unext` element href contains `?page=N`
- Parse page number from `#unext` href

## HTML format
- Search results HTML is identical to normal gallery listing
- Same parser applies (table.itg.glte rows, etc.)
