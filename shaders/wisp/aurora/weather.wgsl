fn weather(input: WeatherInput) -> vec4<f32> {
    let tint = select(vec3<f32>(0.67, 0.76, 0.93), vec3<f32>(0.96, 0.97, 1.0), input.snow);
    return vec4<f32>(input.color.rgb * tint, input.color.a * 0.85);
}
