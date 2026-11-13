fn post(input: PostInput) -> vec4<f32> {
    let pixel = 1.0 / max(frame.resolution, vec2<f32>(1.0));
    let north = textureSampleLevel(previous, linear_sampler, input.uv - vec2<f32>(0.0, pixel.y), 0.0).rgb;
    let south = textureSampleLevel(previous, linear_sampler, input.uv + vec2<f32>(0.0, pixel.y), 0.0).rgb;
    let west = textureSampleLevel(previous, linear_sampler, input.uv - vec2<f32>(pixel.x, 0.0), 0.0).rgb;
    let east = textureSampleLevel(previous, linear_sampler, input.uv + vec2<f32>(pixel.x, 0.0), 0.0).rgb;
    let center_luma = aurora_luma(input.color.rgb);
    let neighbor_luma = vec4<f32>(aurora_luma(north), aurora_luma(south), aurora_luma(west), aurora_luma(east));
    let low = min(center_luma, min(min(neighbor_luma.x, neighbor_luma.y), min(neighbor_luma.z, neighbor_luma.w)));
    let high = max(center_luma, max(max(neighbor_luma.x, neighbor_luma.y), max(neighbor_luma.z, neighbor_luma.w)));
    var color = mix(input.color.rgb, (north + south + west + east) * 0.25, select(0.0, 0.22, high - low > max(0.055, high * 0.18)));
    var bloom = textureSampleLevel(aurora_bloom, linear_sampler, input.uv, 0.0).rgb;
    bloom += textureSampleLevel(aurora_bloom, linear_sampler, input.uv + vec2<f32>(pixel.x * 5.0, 0.0), 0.0).rgb;
    bloom += textureSampleLevel(aurora_bloom, linear_sampler, input.uv - vec2<f32>(pixel.x * 5.0, 0.0), 0.0).rgb;
    bloom += textureSampleLevel(aurora_bloom, linear_sampler, input.uv + vec2<f32>(0.0, pixel.y * 5.0), 0.0).rgb;
    bloom += textureSampleLevel(aurora_bloom, linear_sampler, input.uv - vec2<f32>(0.0, pixel.y * 5.0), 0.0).rgb;
    color += bloom * bloom_strength * 0.2;
    color *= exposure;
    // A gentle highlight shoulder retains bright block colours rather than flattening them.
    color = color / (vec3<f32>(1.0) + max(color - 0.72, vec3<f32>(0.0)) * 0.4);
    let luma = aurora_luma(color);
    color = mix(vec3<f32>(luma), color, saturation);
    let edge = input.uv * (vec2<f32>(1.0) - input.uv);
    color *= 0.92 + 0.08 * pow(clamp(edge.x * edge.y * 16.0, 0.0, 1.0), 0.3);
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), input.color.a);
}
