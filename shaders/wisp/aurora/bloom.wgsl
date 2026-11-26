fn post(input: PostInput) -> vec4<f32> {
    let pixel = 1.0 / max(frame.resolution, vec2<f32>(1.0));
    var bloom = vec3<f32>(0.0);
    for (var x = -1; x <= 1; x += 1) {
        for (var y = -1; y <= 1; y += 1) {
            let color = textureSampleLevel(scene_color, linear_sampler, input.uv + vec2<f32>(f32(x), f32(y)) * pixel * 2.0, 0.0).rgb;
            bloom += color * smoothstep(0.62, 0.95, aurora_luma(color));
        }
    }
    return vec4<f32>(bloom / 9.0, 1.0);
}
