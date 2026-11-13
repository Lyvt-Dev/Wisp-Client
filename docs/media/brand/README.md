# Wisp's mark and icons

**The mark.** A white feather on black, set diagonally, its tip up and to the right: a curved
shaft, splits in the vane and a few downy wisps at its foot. Black and white only.
`packaging/windows/draw-feather.ps1` draws it (`wisp-logo.png` and every SVG below).

## Masters (edit these)

| File | What |
|------|------|
| `icon.svg` | the app icon on the 16 grid: the full square with 22 % corners (Windows, Linux, favicons) |
| `icon-macos.svg` | the same tile on Apple's 1024 grid (824 px tile, 185 px corners, transparent margin) |
| `avatar.svg` | full-bleed square for services that round or crop the image (GitHub, Discord) |
| `mark.svg`, `mark-dark.svg` | the feather alone, for light and for dark backgrounds |
| `wisp-logo.png` | the 1254 px master `packaging/windows/generate-icons.ps1` cuts every icon from |

## Generated (don't edit)

`cargo run -p xtask -- icons` (needs `rsvg-convert` from librsvg) renders the masters into:

| Output | Used by |
|--------|---------|
| `packaging/icons/hicolor/<size>/apps/wisp.png` (16–512) and `scalable/apps/wisp.svg` | Linux: copy the tree into `~/.local/share/icons/hicolor` (or `/usr/share/icons/hicolor`) |
| `packaging/linux/wisp-launcher.desktop`, `wisp.desktop` (hand-written) | Linux: the launcher installs both and the hicolor icons itself, under `$XDG_DATA_HOME` (`~/.local/share`), with its own path in `Exec` (`launcher/src/desktop.rs`; `wisp-launcher desktop-entry --remove` takes them out). Wayland matches the windows' app ids `wisp-launcher` and `wisp` |
| `packaging/icons/wisp.ico` | Windows: embedded in `wisp-client.exe`, its tools and `wisp-launcher.exe` as icon resource 1 (`build.rs` of each, Windows builds only) |
| `packaging/icons/wisp.icns` | macOS: `Wisp.app/Contents/Resources/wisp.icns`, packaged with the .dmg by `packaging/macos/package.sh` |
| `crates/wisp-theme/icons/wisp-64.png` | the game window's icon at run time (X11 and Windows title bars, `window::icon`) |
| `web/favicon.svg`, `web/favicon-32.png`, `web/apple-touch-icon.png` | the website (its `npm run assets` copies them into its `public/`) |
| `icon-1024.png`, `icon-macos-1024.png` | previews, store listings |
| `social/github-avatar-500.png` | GitHub: the organisation or account avatar |

## The README banner and the social images

`docs/media/banner.png` (1600x600), `docs/media/social-preview.jpg` and
`docs/media/brand/social/github-social-1280x640.png` (1280x640) are the feather and "Wisp Client"
on black; `github-avatar-500.png` is the feather alone. `packaging/windows/brand-art.ps1` draws
them from `wisp-logo.png` and the 256 px icon:

```powershell
powershell -ExecutionPolicy Bypass -File packaging/windows/brand-art.ps1 -Repo .
```

The feather itself (`wisp-logo.png` and `icon.svg`) is drawn by `packaging/windows/draw-feather.ps1`.
The launcher screenshots (`docs/media/launcher-{home,shop,settings}.png`) are its development
build on an empty data folder, not signed in (`WISP_LAUNCHER_PAGE` picks the page), captured with
`PrintWindow` at 1280x800.
## Drawn in code

| Where | What |
|-------|------|
| `crates/wisp-client-render/src/title_edition.rs` | the R, U and S of "RUST EDITION" under the classic title screen's logo: Wisp's own glyphs (MIT), rectangles in that file. The banner is rebuilt at load time from the player's own `textures/gui/title/edition.png` (its "EDITION" and "T" carried over, its shades sampled), so no Mojang pixels are in the repository; a resource pack's banner is left as drawn |

## Uploading (the accounts are Ludovic's)

- **GitHub avatar**: github.com/settings/profile (or the organisation's settings), *Edit* the
  picture, upload `social/github-avatar-500.png`.
- **GitHub social preview**: the repository's *Settings › General › Social preview › Edit*,
  upload `social/github-social-1280x640.png`.
- **Discord** (the server icon and banner, Kira's avatar): they live in the private kira repo since 2026-10-15
  (`assets/`, its README's "Discord brand assets"): this repo holds only the game, the launcher and the website.
