# Wisp `.amod` client modules

Wisp 0.7 adds installable client modules for its native Rust Minecraft engine.
The first module is **Zoom**. Installing it does not require Rust or rebuilding the game.
This is the first API version: only the `zoom` module with the `camera-zoom` capability
is implemented. It is not a loader for arbitrary Rust code, Java/Fabric/NeoForge JARs or WebAssembly.

## Install Zoom

1. Select an instance in the launcher, then open **Mods**.
2. Click **Install Zoom**, or **Import .amod** and select a Zoom package.
3. Start Minecraft. Hold **Z** and scroll up to zoom further in, or down to zoom out.
   Release Z to return to the normal view. Zoom scrolling does not change the hotbar slot.
4. Change the key in **Options > Mods > Zoom > Settings**. Starting magnification,
   smoothing and proportional mouse sensitivity are also configurable in the launcher.

Changes made in the launcher apply at the next game start. Mods belong to the selected
instance, in `<game folder>/wisp-mods/`. Enable/disable preferences remain in
`wisp-mods.txt`; per-module options are in
`wisp-mods/.wisp-mod-data/<id>/zoom.json`.
Fresh instances have no installed modules and retain the normal camera.

Zoom has no old 10× gameplay cap. Wheel input adjusts magnification continuously, with
a finite 10^12 precision guard and a positive FOV floor to keep the renderer stable.
It cannot reveal chunks the server has not sent or increase texture resolution.
The spyglass retains its own camera behavior. Leaving gameplay or losing window focus
resets the module's held key state.

## Max Render

`module = "max_render"`, `capabilities = ["render-distance"]`: lifts the render distance cap
from 128 to 2,147,483,647 chunks (`options_file::set_unlimited_render_distance`). The slider's
right half then climbs logarithmically; exact values go in `options.txt`, `--render-distance`
or `wisp-ctl render-distance`. The engine never streams past the world's 3,750,000 chunks
(`options_file::WORLD_RENDER_DISTANCE`), and memory, not the number, decides how much loads.
Source: `mods/wisp-mods/max_render/`.

## Package format, API 1

A client `.amod` is a ZIP containing exactly one UTF-8 `wisp-client-mod.toml`:

```toml
format = "wisp-client-mod"
api = 1
id = "zoom"
name = "Zoom"
version = "1.0.0"
description = "Hold Z to zoom. Scroll while holding it to zoom further in or out."
module = "zoom"
capabilities = ["camera-zoom"]

[zoom]
key = "key.keyboard.z"
initial_factor = 4.0
scroll_step = 1.25
smooth = true
scale_sensitivity = true
```

IDs contain 1–64 lowercase ASCII letters, digits or underscores. Unknown fields,
unsupported APIs/modules/capabilities, extra archive entries and invalid numeric options
are rejected. Packages are limited to 1 MiB and the manifest to 64 KiB. No archive
files are extracted and packages cannot execute additional code.

## Build and check

Build the SDK from this repository with `cargo build -p cargo-wisp-mod`.
From a directory containing the manifest:

```sh
cargo wisp-mod build
cargo wisp-mod check target/wisp-mod/zoom.amod --json
```

The checked-in source is `mods/wisp-mods/zoom/wisp-client-mod.toml`.
`cargo wisp-mod new my_zoom --template client --name "My Zoom"` creates a
manifest for a configurable instance of the supported Zoom module. Adding a new
kind of module requires implementing and shipping its Rust engine handler first.

Client modules currently use local import; registry submission/signature checking
for this API is not implemented. `check --key/--signature` explicitly rejects client modules.

Content mods also use the `.amod` extension for new SDK builds, but contain the
existing `wisp-mod.toml`, assets and data format. Old `.hmod` content packages
remain readable, including pre-rebrand archive manifest names. Archive bytes are not
rewritten, preserving existing signatures. The first launcher client-module page imports API 1 client modules;
content packages still use the existing content-mod installation flow described in
[mods.md](mods.md).

## Featured server cleanup

The game no longer inserts a featured SMP server. On first normal startup, the
known old featured-server addresses are removed from an existing `servers.dat`.
The original bytes are saved to `servers.dat.before-wisp-cleanup`; other entries,
hidden entries and unknown NBT fields are preserved. A marker prevents repeat
cleanup, so a server manually added after migration stays. A corrupt list is left untouched.
