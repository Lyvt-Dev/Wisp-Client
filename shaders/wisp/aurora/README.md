# Wisp Aurora 1.0.0

Original MIT-licensed WGSL shader for Wisp's native Rust renderer (shader API 0.1).
The visual target is a cinematic, vanilla-friendly world: block-shaped clouds,
warm sunlight, soft shadows, reflective animated water, waving foliage, bloom,
light shafts, coloured block light and subtle edge smoothing.

Install and select Aurora in the launcher's **Shaders** page, then start Minecraft.
It is off until selected. Selection and options belong to the selected instance.
The launcher offers Low, Balanced and High presets. In-game shader options expose
individual shadow, cloud, reflection, wind, bloom, exposure and saturation controls.
Use Low on integrated GPUs; High adds shadow/cloud/shaft samples and costs more GPU time.

This is an independent native implementation inspired by the general visual goals
of Complementary Reimagined, not a port, a copy or a promise of identical rendering.
No Complementary source, textures or assets are included. Java/OptiFine/Iris shader
ZIPs do not run on the native WGSL API. Reflection uses the engine's planar water
pass; foliage motion currently targets configured leaves, grass and flowers.
Clouds and atmospheric shafts apply to the Overworld. Vanilla skies remain in
other dimensions. This first release has no temporal antialiasing or path tracing.

Build and validate with the repository's shader SDK:

```text
cargo run -p wisp-shader-api --bin wisp-shader -- check shaders/wisp/aurora
cargo run -p wisp-shader-api --bin wisp-shader -- pack shaders/wisp/aurora --out Wisp-Aurora-1.0.0.hshader
```

All WGSL and package metadata are original Wisp contributions. See LICENSE.
