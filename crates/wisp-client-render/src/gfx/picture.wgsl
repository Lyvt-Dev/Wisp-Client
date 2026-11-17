// The title's flat picture: one triangle over the screen, the picture cropped to cover it.
struct Fit { scale: vec2<f32>, offset: vec2<f32> }
@group(0) @binding(0) var<uniform> fit: Fit;
@group(0) @binding(1) var picture: texture_2d<f32>;
@group(0) @binding(2) var picture_sampler: sampler;
struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@vertex fn vs_picture(@builtin(vertex_index) index: u32) -> Out {
    let corner = vec2(f32((index << 1u) & 2u), f32(index & 2u));
    var out: Out;
    out.clip = vec4(corner * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2(corner.x, 1.0 - corner.y) * fit.scale + fit.offset;
    return out;
}
@fragment fn fs_picture(in: Out) -> @location(0) vec4<f32> {
    return textureSample(picture, picture_sampler, in.uv);
}
