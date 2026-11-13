// Original Wisp Aurora shader. MIT; see LICENSE. No third-party shader source.
override quality: u32 = 1u;
override shadows: bool = true;
override shadow_resolution: u32 = 1u;
override water_reflections: bool = true;
override clouds: bool = true;
override cloud_density: f32 = 0.55;
override foliage_wind: bool = true;
override bloom_strength: f32 = 0.18;
override sun_shafts: f32 = 0.28;
override exposure: f32 = 1.0;
override saturation: f32 = 1.08;

fn aurora_day() -> f32 {
    return smoothstep(-0.14, 0.18, frame.sun_direction.y);
}
fn aurora_hash(p: vec2<f32>) -> f32 {
    var h = bitcast<u32>(i32(p.x)) * 2654435761u + bitcast<u32>(i32(p.y)) * 2246822519u;
    h = (h ^ (h >> 16u)) * 2146121005u;
    h = (h ^ (h >> 15u)) * 2221713035u;
    h ^= h >> 16u;
    return f32(h >> 8u) / 16777216.0;
}
fn aurora_noise(p: vec2<f32>) -> f32 {
    let cell = floor(p);
    let f = fract(p);
    let w = f * f * (vec2<f32>(3.0) - 2.0 * f);
    return mix(mix(aurora_hash(cell), aurora_hash(cell + vec2<f32>(1.0, 0.0)), w.x),
        mix(aurora_hash(cell + vec2<f32>(0.0, 1.0)), aurora_hash(cell + vec2<f32>(1.0)), w.x), w.y);
}
fn aurora_cloud(p: vec3<f32>) -> f32 {
    let block = floor(p / 3.0) * 3.0;
    let drift = vec2<f32>(frame.time * 0.9, frame.time * 0.22);
    let n = aurora_noise((block.xz + drift) * 0.012)
        + 0.35 * aurora_noise((block.xz + drift) * 0.035);
    let layer = smoothstep(190.0, 196.0, p.y) * (1.0 - smoothstep(210.0, 220.0, p.y));
    return max(n - (1.08 - cloud_density * 0.65), 0.0) * layer * 2.7;
}
fn aurora_sky_color(ray: vec3<f32>) -> vec3<f32> {
    let day = aurora_day();
    let elevation = pow(clamp(ray.y * 0.7 + 0.3, 0.0, 1.0), 0.55);
    let daylight = mix(vec3<f32>(0.66, 0.78, 0.92), vec3<f32>(0.16, 0.39, 0.75), elevation);
    let night = mix(vec3<f32>(0.065, 0.09, 0.16), vec3<f32>(0.009, 0.018, 0.05), elevation);
    let dusk = (1.0 - smoothstep(0.0, 0.32, abs(frame.sun_direction.y)))
        * pow(max(dot(ray, frame.sun_direction), 0.0), 3.0);
    var color = mix(night, daylight, day) + vec3<f32>(0.8, 0.24, 0.09) * dusk * 0.5;
    color = mix(color, vec3<f32>(0.26, 0.30, 0.38) * mix(0.3, 1.0, day), frame.rain * 0.7);
    return color;
}
fn aurora_luma(color: vec3<f32>) -> f32 {
    return dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
}
