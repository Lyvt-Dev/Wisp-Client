# Native resource-pack skies

Wisp 0.10 supports the static Nuit schema-1 subset used by Girl Sky: square-textured
3-by-2 cube textures, monocolor backgrounds, the normal Overworld sky and vanilla
sun/moon/star visibility controls. Definitions are read from `assets/nuit/sky/`.
Layers are ordered by `properties.layer` and file name. Transparent textures use
normal alpha blending, independent from celestial additive lighting.

The first implementation supports unrestricted and Overworld skybox/dimension
conditions, normal/alpha blending, and stationary textured layers. Advanced
conditions, animated/rotating textures, fades, clocks, custom shader programs and
custom celestial textures are not supported. Unsupported definitions are logged.
Native shader packs that replace the sky take priority over these resource skies.
Individual faces are limited to 1024 pixels to bound atlas memory.

A valid `pack.mcmeta` only describes a pack. An archive containing just that file
and `pack.png` has no texture data. Keep the original ZIP intact instead of copying
only its metadata or icon. The sky JSON files and referenced PNG must be included.

Loading retains the prepared vanilla archive through an indexed overlay. Pack
overrides and merged files read from the overlay; unchanged bytes use the original
archive. Filtered resources are removed from the index, and rebuilding a selection
replaces the in-process index. This also applies when reusing an on-disk cache.

For a read-only I/O probe (no game/player files are modified):

```sh
cargo run -p wisp-core --example prepared_overlay_probe -- PREPARED_ROOT SCRATCH_OVERLAY
```

Format references: [Nuit schema](https://github.com/FlashyReese/nuit/blob/26.3/dev/docs/schema.md)
and face layout in `SquareTexturedSkybox.java` / `Utils.java` at
`df9ff0dd57508afc0889938b90770858ad86a696` (MIT). The native implementation
includes no Java mod. See THIRD_PARTY_NOTICES.md for attribution and license.
