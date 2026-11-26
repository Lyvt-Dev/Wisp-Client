fn vertex(input: VertexInput) -> vec3<f32> {
    var position = input.position;
    if !foliage_wind || frame.dimension != 0 { return position; }
    let world = input.position + frame.camera_position;
    let wave = sin(world.x * 0.55 + world.z * 0.38 + frame.time * 1.7)
        + 0.4 * sin(world.x * -0.91 + world.z * 0.62 + frame.time * 2.5);
    if input.material == 1u {
        position.x += wave * 0.035;
        position.z += wave * 0.022;
    } else if input.material == 2u {
        let tip = clamp(1.0 - input.uv.y, 0.0, 1.0);
        position.x += wave * 0.10 * tip;
        position.z += wave * 0.06 * tip;
    }
    return position;
}
