# ExHentai Favorites — Facts
> Source: JHenTai (jiangtian616/JHenTai) and EhViewer-Overhauled. Reverse-engineered facts only — do not modify without user permission.

## 1. Favorite Folders
- 10 cloud folders, indexed 0–9. Default names: "Favorite 0" through "Favorite 9".
- Users can rename on the EH website.

### Folder colors (index → CSS hex → display color)
| Index | CSS hex | Display color name |
|---|---|---|
| 0 | `#000` | Grey |
| 1 | `#f00` | Red |
| 2 | `#fa0` | Orange |
| 3 | `#dd0` | Yellow |
| 4 | `#080` | Dark Green |
| 5 | `#9f4` | Light Green |
| 6 | `#4bf` | Cyan |
| 7 | `#00f` | Blue |
| 8 | `#508` | Purple |
| 9 | `#e8e` | Pink |

## 2. Endpoints

| Purpose | Method | URL |
|---|---|---|
| Browse favorites / get folder info | GET | `https://exhentai.org/favorites.php` |
| Add / remove / move a gallery | POST | `https://exhentai.org/gallerypopups.php?gid={gid}&t={token}&act=addfav` |
| Get current note (pre-edit fetch) | GET | `https://exhentai.org/gallerypopups.php?gid={gid}&t={token}&act=addfav` |
| Bulk move/delete (from fav page) | POST | `https://exhentai.org/favorites.php` |

## 3. Add to Favorites (POST /gallerypopups.php)

```
POST /gallerypopups.php?gid={gid}&t={token}&act=addfav
Content-Type: application/x-www-form-urlencoded

favcat={0-9}&favnote={text}&apply=Apply+Changes&update=1
```

- `favcat`: integer 0–9 (folder index)
- `favnote`: personal note, max 200 chars, may be empty
- `apply`: `"Apply Changes"` (works for both add and move)
- `update`: must be `1`

## 4. Remove from Favorites (POST /gallerypopups.php)

```
POST /gallerypopups.php?gid={gid}&t={token}&act=addfav
Content-Type: application/x-www-form-urlencoded

favcat=favdel&favnote=&apply=Apply+Changes&update=1
```

- `favcat`: literal string `"favdel"` (not an integer)
- `favnote`: empty string
- `apply`: `"Apply Changes"`

## 5. Get Favorite Note (GET /gallerypopups.php)

```
GET /gallerypopups.php?gid={gid}&t={token}&act=addfav
```

HTML parsing:
- Note text: `#galpop > div > div:nth-child(3) > textarea` → `.text`
- Note slot count: `#galpop > div > div:nth-child(3) > div:nth-child(6)` → format `"1 / 1000 favorite note slots used."`
- Limit: 1000 total favorite note slots per account.

Needed only when gallery is already favorited and user is editing/moving (not removing).

## 6. Favorites Page (GET /favorites.php)

Query parameters:
| Parameter | Description |
|---|---|
| `favcat` | `0`–`9` for specific folder; `"all"` for all; omit for default |
| `f_search` | Keyword search within favorites |
| `prev` | Prev-page cursor (gid string) |
| `next` | Next-page cursor (gid string) |
| `seek` | Jump to date `yyyy-MM-dd` |
| `inline_set` | Sort: `fs_p` = by publish time; `fs_f` = by favorited time |

Returns up to 25 galleries per page. Same HTML structure as listing pages.

### Folder names and counts (parsed from favorites.php)
```
CSS: .nosel > .fp   (10 elements, last is not a folder)
  count: div:first-child → text → int
  name:  div:last-child  → text → string
```

## 7. Detecting Favorite Status

**Gallery list pages:**
- Small colored `<div>` with `border-color:#{hex}` → folder index lookup.
- Absent → not favorited.

**Gallery detail page:**
- `#fav > .i` with `style` attribute.
- `style` null → not favorited.
- Index from sprite: `background-position:0px -{offset}px` → `(offset - 2) / 19`.
- Folder name: `#favoritelink` → `.text`.

## 8. Required Auth
- All cloud operations require valid cookies: `ipb_member_id`, `ipb_pass_hash`, `igneous`.
- Not logged in → server returns a login-required page.

## 9. Note Constraints
- Max note length: 200 characters (client-enforced).
- Max note slots per account: 1000 (server-reported).
- Note preserved when moving between folders (fetch first, re-submit).
- Note cleared on remove (`favnote` sent as empty string).
