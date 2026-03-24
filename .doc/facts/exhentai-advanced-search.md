# ExHentai Advanced Search — Full Parameters
> External facts — do NOT modify without user permission

## Enabling advanced search
- Add `advsearch=1` to enable advanced mode. Without this, all `f_s*` params are ignored.

## Search scope toggles (checkboxes)
All are `=on` when enabled, omitted when disabled.

| Param | Label | Notes |
|-------|-------|-------|
| `f_sname` | Search Gallery Name | Default on |
| `f_stags` | Search Gallery Tags | Default on |
| `f_sdesc` | Search Gallery Description | |
| `f_storr` | Search Torrent Filenames | |
| `f_sto` | Only Show Galleries With Torrents | |
| `f_sdt1` | Search Low-Power Tags | |
| `f_sdt2` | Search Downvoted Tags | |
| `f_sh` | Show Expunged Galleries | |

## Minimum rating filter
- `f_sr=on` — enable minimum rating filter
- `f_srdd=N` — minimum star rating, values: `2`, `3`, `4`, `5`
- Both must be present. `f_sr=on` alone does nothing without `f_srdd`.

## Page count range filter
- `f_sp=on` — enable page count filter
- `f_spf=N` — minimum pages (integer, empty string = no minimum)
- `f_spt=N` — maximum pages (integer, empty string = no maximum)
- All three must be present when filtering. `f_spf=` and `f_spt=` can be empty strings.

## File search (reverse image / similar)
- `f_sfl=on` — Search by File: URL match
- `f_sfu=on` — Search by File: SHA hash match
- `f_sft=on` — Search by File: similarity scan
- `f_shash=HASH` — SHA-1 hash of image file (40 hex chars)
- `fs_similar=1` — find similar galleries
- `fs_covers=1` — only match cover images
- `fs_exp=1` — include expunged in file search

## Search query syntax (in f_search)
The `f_search` param is a single string that can combine free text and structured tag expressions.

### Free text
- Plain words search gallery titles (when `f_sname=on`): `ff14 comic`

### Tag syntax
- Namespaced tags: `female:catgirl`, `language:english`, `artist:oda_non`
- Quotes required for multi-word values: `parody:"series name"`
- Dollar sign for exact match: `artist:"name$"`
- Uploader search: `uploader:"username"`

### Tag exclusion
- Minus prefix excludes: `-female:netorare`, `-language:chinese`
- Works with namespaced and quoted tags: `-parody:"series name"`

### Combining multiple tags
- Space-separated tags are AND: `female:catgirl language:english` (both required)
- Tilde for OR: `artist:"name1" ~ artist:"name2"` (either matches)

### Combining free text with tags
- Free text and tags can coexist in the same f_search string
- Example: `ff14 language:english -female:netorare` = title contains "ff14" AND tag language:english AND NOT tag female:netorare
- The search scope toggles (f_sname, f_stags, etc.) control which fields the entire query is matched against

## Category bitmask (f_cats)
Same as basic search — see exhentai-search.md.

## Pagination
Same as basic search — `page=N` (0-indexed), next URL from `#unext` element.

## Example full URL
```
https://exhentai.org/?f_search=ff14+language%3Aenglish+-female%3Anetorare&f_cats=0&advsearch=1&f_sname=on&f_stags=on&f_sr=on&f_srdd=3&f_sp=on&f_spf=20&f_spt=200&f_sh=on&page=0
```
