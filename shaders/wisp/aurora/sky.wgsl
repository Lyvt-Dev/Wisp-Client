fn sky(input: SkyInput) -> vec4<f32> {
    if frame.dimension != 0 { return input.color; }
    let ray = normalize(input.direction);
    let day = aurora_day();
    var color = aurora_sky_color(ray);
    let star_grid = floor(ray.xz / max(abs(ray.y), 0.15) * 360.0);
    let star = pow(aurora_hash(star_grid), 160.0) * smoothstep(0.04, 0.5, ray.y);
    color += vec3<f32>(0.72, 0.80, 1.0) * star * (1.0 - day) * (1.0 - frame.rain);
    let sun = normalize(frame.sun_direction);
    let axis = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(0.0, 0.0, 1.0), abs(sun.y) > 0.98);
    let right = normalize(cross(axis, sun));
    let up = cross(sun, right);
    let facing = dot(ray, sun);
    let sun_uv = vec2<f32>(dot(ray, right), dot(ray, up));
    let sun_square = select(0.0, 1.0, facing > 0.0 && max(abs(sun_uv.x), abs(sun_uv.y)) < 0.028);
    color += vec3<f32>(1.0, 0.78, 0.46) * (sun_square * 1.5 + pow(max(facing, 0.0), 100.0) * 0.14) * (1.0 - frame.rain);
    if clouds && abs(ray.y) > 0.002 && frame.eye_in == 0u {
        let near_layer = (190.0 - frame.camera_position.y) / ray.y;
        let far_layer = (220.0 - frame.camera_position.y) / ray.y;
        let start = max(min(near_layer, far_layer), 0.0);
        let end = min(max(near_layer, far_layer), 1600.0);
        if end > start {
            let count = 8u + quality * 4u;
            let step_size = (end - start) / f32(count);
            var transmittance = 1.0;
            var scatter = vec3<f32>(0.0);
            for (var i = 0u; i < count; i += 1u) {
                let t = start + (f32(i) + 0.5) * step_size;
                let p = frame.camera_position + ray * t;
                let density = aurora_cloud(p);
                let alpha = 1.0 - exp(-density * step_size * 0.12);
                let top_light = 1.0 - aurora_cloud(p + frame.light_direction * 16.0) * 0.45;
                let cloud_light = mix(vec3<f32>(0.10, 0.14, 0.24), vec3<f32>(1.0, 0.93, 0.84), day);
                let tint = cloud_light * (0.55 + 0.45 * clamp(top_light, 0.0, 1.0));
                scatter += tint * alpha * transmittance;
                transmittance *= 1.0 - alpha;
            }
            let haze = smoothstep(700.0, 1600.0, start);
            color = mix(color * transmittance + scatter, color, haze);
        }
    }
    return vec4<f32>(color, 1.0);
}
