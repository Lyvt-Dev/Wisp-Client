fn aurora_shadow(position: vec3<f32>, normal: vec3<f32>) -> f32 {
    if !shadows || frame.has_shadow == 0u { return 1.0; }
    let bias = max(frame.shadow_center_texel * 0.22, 0.025);
    let p = shadow_position(position + normal * bias);
    if any(p.xy < vec2<f32>(0.002)) || any(p.xy > vec2<f32>(0.998)) || p.z < 0.0 || p.z > 1.0 { return 1.0; }
    let texel = 1.0 / max(frame.shadow_resolution, 1.0);
    var lit = 0.0;
    let radius = select(1, 2, quality == 2u);
    for (var x = -radius; x <= radius; x += 1) {
        for (var y = -radius; y <= radius; y += 1) {
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, p.xy + vec2<f32>(f32(x), f32(y)) * texel * 1.1, p.z - 0.0002);
        }
    }
    let width = f32(radius * 2 + 1);
    return lit / (width * width);
}
fn aurora_behind(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let clip = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), max(depth, 0.000001), 1.0);
    let view = frame.projection_inverse * clip;
    return (frame.view_inverse * vec4<f32>(view.xyz / max(abs(view.w), 0.000001), 1.0)).xyz;
}
fn material(input: MaterialInput) -> Surface {
    var normal = input.normal;
    let world = input.position + frame.camera_position;
    let day = aurora_day();
    let shade = aurora_shadow(input.position, normal);
    let sky_exposure = pow(clamp(input.light.y, 0.0, 1.0), 2.0);
    let facing = max(dot(normal, frame.light_direction), 0.0);
    var light = input.lightmap * mix(vec3<f32>(0.68, 0.76, 0.94), vec3<f32>(1.10, 1.03, 0.90), shade * mix(0.4, 1.0, facing) * day);
    light *= mix(1.0, 0.58 + shade * 0.42, sky_exposure);
    let block = block_light_color(input.position);
    light += block.rgb * block.a * input.light.x * 0.18 + input.colored_light * 0.25;
    light += vec3<f32>(input.emission * 0.08);
    // Keep moonlit outdoor terrain readable without brightening unlit caves.
    light = max(light, vec3<f32>(0.10, 0.13, 0.20) * sky_exposure * (1.0 - day) * (1.0 - frame.rain * 0.4));
    var color = input.albedo * input.tint * vec4<f32>(max(light, vec3<f32>(0.015)), 1.0);
    if input.fluid == 1u && input.translucent {
        let phase = world.x * 0.65 + world.z * 0.38 + frame.time * 1.15;
        let phase2 = world.x * -0.31 + world.z * 0.78 - frame.time * 0.92;
        if input.face == 1u {
            normal = normalize(vec3<f32>(-cos(phase) * 0.09 + cos(phase2) * 0.04, 1.0, -cos(phase) * 0.05 - cos(phase2) * 0.08));
        }
        let uv = clamp(input.screen + normal.xz * 0.007, vec2<f32>(0.001), vec2<f32>(0.999));
        let size = vec2<f32>(textureDimensions(opaque_color));
        let pixel = vec2<i32>(uv * (size - 1.0));
        let behind_depth = textureLoad(opaque_depth, pixel, 0);
        let thickness = clamp(length(aurora_behind(uv, behind_depth)) - length(input.position), 0.0, 24.0);
        let transmission = exp(-vec3<f32>(0.18, 0.07, 0.035) * thickness);
        let refracted = textureLoad(opaque_color, pixel, 0).rgb;
        let water = refracted * transmission + vec3<f32>(0.025, 0.20, 0.27) * (1.0 - transmission);
        let view = normalize(-input.position + vec3<f32>(0.00001));
        let fresnel = 0.035 + 0.965 * pow(1.0 - max(dot(view, normal), 0.0), 5.0);
        var mirror = aurora_sky_color(reflect(-view, normal));
        if water_reflections && frame.has_reflection == 1u {
            mirror = textureSampleLevel(reflection, reflection_sampler, clamp(input.screen + normal.xz * 0.015, vec2<f32>(0.002), vec2<f32>(0.998)), 0.0).rgb;
        }
        let glint = pow(max(dot(reflect(-frame.sun_direction, normal), view), 0.0), 96.0) * day * shade;
        color = vec4<f32>(mix(water, mirror, fresnel * 0.78) + vec3<f32>(1.0, 0.84, 0.58) * glint * 0.65, max(input.albedo.a, 0.78));
    }
    set_surface(normal, select(0.08, 0.85, input.fluid == 1u), input.emission);
    return Surface(color);
}
