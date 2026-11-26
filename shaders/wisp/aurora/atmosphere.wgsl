fn post(input: PostInput) -> vec4<f32> {
    var color = input.color.rgb;
    if frame.dimension != 0 || frame.eye_in != 0u { return input.color; }
    let point = view_position(input.uv, max(input.depth, 0.000001));
    let distance = select(min(length(point), 96.0), 96.0, input.depth == 0.0);
    let ray = normalize(point + vec3<f32>(0.000001));
    if shadows && frame.has_shadow == 1u && sun_shafts > 0.0 {
        let count = 4u + quality * 4u;
        var light = 0.0;
        for (var i = 0u; i < count; i += 1u) {
            let t = (f32(i) + 0.5) / f32(count) * distance;
            let p = shadow_position(ray * t);
            if all(p.xy > vec2<f32>(0.001)) && all(p.xy < vec2<f32>(0.999)) && p.z > 0.0 && p.z < 1.0 {
                light += textureSampleCompareLevel(shadow_map, shadow_sampler, p.xy, p.z - 0.0002);
            }
        }
        let phase = 0.18 + 0.82 * pow(max(dot(ray, frame.sun_direction), 0.0), 8.0);
        let visibility = aurora_day() * (1.0 - frame.rain * 0.75) * clamp(distance / 80.0, 0.0, 1.0);
        color += vec3<f32>(1.0, 0.79, 0.50) * light / f32(count) * phase * sun_shafts * visibility;
    }
    return vec4<f32>(color, input.color.a);
}
