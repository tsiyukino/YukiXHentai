# ExHentai Authentication
> External facts — do NOT modify without user permission

## Required cookies
- `ipb_member_id` — session/member ID
- `ipb_pass_hash` — session password hash
- `igneous` — required for ExHentai access (not needed for E-Hentai)

## Cookie behavior
- All three cookies must be set on requests to `exhentai.org`
- Without valid cookies, ExHentai returns "sadpanda" (see html-structure.md)
- E-Hentai API (`api.e-hentai.org`) works without `igneous`

## Thumbnail CDN
- Thumbnails served from `ehgt.org` — no authentication required
- No rate limiting on CDN thumbnail downloads
- Parallel downloads OK for thumbnails
