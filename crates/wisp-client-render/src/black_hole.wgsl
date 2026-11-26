// The Black Hole emote (`gfx/world_post.rs`, `Lens`): the world's post pass draws a
// Schwarzschild black hole into the scene. Each pixel's ray is bent around the hole
// (lensing the world behind it), crosses a turbulent accretion disk with Doppler
// beaming, falls into the horizon or escapes; what stands in front (the depth buffer)
// stays in front.
@group(0) @binding(0) var world: texture_2d<f32>;
@group(0) @binding(1) var world_sampler: sampler;

struct Lens {
    // Clip space to camera-relative world space, and back (unjittered).
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    // xyz: the hole's centre, relative to the camera; w: its horizon radius in blocks.
    centre: vec4<f32>,
    // xyz: the disk's normal; w: seconds since the hole appeared.
    disk: vec4<f32>,
    // x: strength (0..1); y: white flash (0..1); z: shock wave radius in horizon
    // radii (0: none); w: 1 draws the Supernova Crown in place of the hole (x is then how
    // far the star dims the world).
    params: vec4<f32>,
    // The Supernova Crown. x: the star's radius in blocks; y: seconds since it burst
    // (negative before); z: how much is left of the remnant (1..0); w: the star's power.
    nova: vec4<f32>,
    // xyz: the crown's centre, relative to the camera; w: how much of it shows (0..1).
    // The Purple Hollow (params.w 2): the blue orb's centre and its radius in blocks; `nova`
    // is then x: seconds since the sphere was fired (negative before), y: its power, z: how
    // much is left of its wake (1..0); `centre` is the purple sphere's centre and radius.
    crown: vec4<f32>,
    // The Purple Hollow's red orb: its centre, relative to the camera, and its radius.
    red: vec4<f32>,
    // Where the Purple Hollow's sphere was fired from, relative to the camera.
    // False Dreams (params.w 3): `centre` is the floating player's middle and the eyes' size;
    // `nova` x: how many of the eyes show (0..1), y: how blurred this view is (0..1), z: how
    // far out they float, w: how many there are.
    origin: vec4<f32>,
};
@group(1) @binding(0) var scene_depth: texture_depth_2d;
@group(1) @binding(1) var<uniform> lens: Lens;

struct PostOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex fn vs_post(@builtin(vertex_index) i: u32) -> PostOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var out: PostOut;
    out.clip = vec4(p * 2.0 - 1.0, 0.0, 1.0);
    out.uv = vec2(p.x, 1.0 - p.y);
    return out;
}

// How far out (in horizon radii) light is bent; beyond it rays go straight.
const REACH: f32 = 14.0;
const DISK_INNER: f32 = 2.6;
const DISK_OUTER: f32 = 9.0;

fn hash(p: vec2<f32>) -> f32 {
    let q = fract(p * vec2(123.34, 456.21));
    let r = q + dot(q, q + 45.32);
    return fract(r.x * r.y);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
        mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x),
        u.y,
    );
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i++) {
        v += a * noise(q);
        q = mat2x2<f32>(1.6, 1.2, -1.2, 1.6) * q;
        a *= 0.5;
    }
    return v;
}

fn uv_of(clip: vec4<f32>) -> vec2<f32> {
    return vec2(0.5, -0.5) * clip.xy / clip.w + 0.5;
}

// Deep space where a bent ray leaves the screen: dark violet with a few stars.
fn space(dir: vec3<f32>) -> vec3<f32> {
    let s = vec2(atan2(dir.z, dir.x), asin(clamp(dir.y, -1.0, 1.0))) * 60.0;
    let star = pow(hash(floor(s)), 40.0) * 3.0;
    return vec3(0.012, 0.006, 0.025) + vec3(star * 0.9, star * 0.85, star);
}

// The world in direction `dir` (from the camera), as if it were far away.
fn background(dir: vec3<f32>) -> vec3<f32> {
    let clip = lens.view_proj * vec4(dir * 512.0, 1.0);
    if clip.w <= 0.001 {
        return space(dir);
    }
    let uv = uv_of(clip);
    let outside = length(max(abs(uv - 0.5) - 0.5, vec2(0.0)));
    let seen = textureSampleLevel(world, world_sampler, clamp(uv, vec2(0.001), vec2(0.999)), 0.0).rgb;
    return mix(seen, space(dir), clamp(outside * 4.0, 0.0, 1.0));
}

// Black body-ish ramp: deep red -> orange -> white-hot, with an Wisp violet rim.
fn heat(t: f32) -> vec3<f32> {
    let ember = vec3(0.55, 0.12, 0.45);
    let orange = vec3(1.0, 0.45, 0.12);
    let white = vec3(1.0, 0.92, 0.82);
    return select(mix(orange, white, (t - 0.5) * 2.0), mix(ember, orange, t * 2.0), t < 0.5);
}

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * vec3(0.1031, 0.1030, 0.0973));
    q = q + dot(q, q.yxz + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let x00 = mix(hash3(i), hash3(i + vec3(1.0, 0.0, 0.0)), u.x);
    let x10 = mix(hash3(i + vec3(0.0, 1.0, 0.0)), hash3(i + vec3(1.0, 1.0, 0.0)), u.x);
    let x01 = mix(hash3(i + vec3(0.0, 0.0, 1.0)), hash3(i + vec3(1.0, 0.0, 1.0)), u.x);
    let x11 = mix(hash3(i + vec3(0.0, 1.0, 1.0)), hash3(i + vec3(1.0, 1.0, 1.0)), u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

fn fbm3(p: vec3<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 3; i++) {
        v += a * noise3(q);
        q = q * 2.03 + vec3(7.3, 1.9, 4.1);
        a *= 0.5;
    }
    return v / 0.875;
}

// How far the scene is from the camera at `uv`, in blocks (1e9: the sky).
fn scene_distance(uv: vec2<f32>) -> f32 {
    let size = vec2<f32>(textureDimensions(scene_depth));
    let texel = clamp(vec2<i32>(uv * size), vec2(0), vec2<i32>(size) - 1);
    let z = textureLoad(scene_depth, texel, 0);
    if z <= 0.0 {
        return 1e9;
    }
    let hit = lens.inv_view_proj * vec4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, z, 1.0);
    if abs(hit.w) < 1e-7 {
        return 1e9;
    }
    return length(hit.xyz / hit.w);
}

// A thin line of light from `a` to `b` (relative to the camera) as the ray `rd` sees it:
// x its bright core, y the glow round it; nothing where the scene is in front.
fn light_line(rd: vec3<f32>, a: vec3<f32>, b: vec3<f32>, scene: f32) -> vec2<f32> {
    let ab = b - a;
    let d1 = dot(rd, ab);
    let ra = dot(rd, a);
    let s = clamp((d1 * ra - dot(ab, a)) / max(dot(ab, ab) - d1 * d1, 1e-6), 0.0, 1.0);
    let q = a + ab * s;
    let t = max(dot(rd, q), 0.0);
    if t > scene {
        return vec2(0.0);
    }
    let d = length(rd * t - q);
    return vec2(exp(-d * d / 0.00016), 0.35 / (1.0 + d * d / 0.0016)) * (1.0 - 0.45 * s);
}

// The crown of light on the player's head: a band and seven points rising from it,
// turning slowly, the part behind the head hidden by it.
fn crown_light(rd: vec3<f32>, scene: f32, time: f32) -> vec3<f32> {
    let k = lens.crown.w;
    let c = lens.crown.xyz;
    let tc = dot(c, rd);
    if k <= 0.002 || tc <= 0.0 || length(rd * tc - c) > 1.3 {
        return vec3(0.0);
    }
    let radius = 0.4 * smoothstep(0.0, 0.5, k);
    var light = vec2(0.0);
    // The band: the nearest point of the ring on the near side and on the far side.
    for (var side = 0; side < 2; side++) {
        var t = tc + select(-radius, radius, side == 1);
        var q = c;
        for (var j = 0; j < 4; j++) {
            let o = rd * t - c;
            let on = normalize(vec2(o.x, o.z) + vec2(1e-5, 0.0)) * radius;
            q = c + vec3(on.x, 0.0, on.y);
            t = max(dot(q, rd), 0.0);
        }
        if t <= scene {
            let d = length(rd * t - q);
            light = max(light, vec2(exp(-d * d / 0.0003), 0.4 / (1.0 + d * d / 0.002)));
        }
    }
    var tips = 0.0;
    let turn = time * 0.55;
    for (var i = 0; i < 7; i++) {
        let a0 = turn + f32(i) * 0.8975979;
        let a1 = a0 + 0.44879895;
        let a2 = a0 + 0.8975979;
        let rise = (0.3 + 0.035 * sin(time * 2.3 + f32(i) * 1.7)) * smoothstep(0.2, 1.0, k);
        let v0 = c + vec3(cos(a0), 0.0, sin(a0)) * radius;
        let v1 = c + vec3(cos(a2), 0.0, sin(a2)) * radius;
        let tip = c + vec3(cos(a1) * radius * 1.14, rise, sin(a1) * radius * 1.14);
        light += light_line(rd, v0, tip, scene);
        light += light_line(rd, v1, tip, scene);
        // A star on each point.
        let tt = dot(rd, tip);
        if tt > 0.0 && tt <= scene {
            let d = length(rd * tt - tip);
            tips += (0.6 + 0.4 * sin(time * 5.0 + f32(i) * 2.4)) * 0.0005 / (d * d + 0.0004);
        }
    }
    return (vec3(1.0, 0.78, 0.32) * light.x * 1.8 + vec3(1.0, 0.55, 0.08) * light.y
        + vec3(1.0, 0.95, 0.85) * tips) * k;
}

// The Supernova Crown: a star with a boiling surface and a corona; then, once it has
// burst, the shell it threw off (filaments of glowing gas, marched through), the pulsar
// left at its centre, and the crown. `base` is the world at this pixel.
fn supernova(base: vec3<f32>, uv: vec2<f32>, rd: vec3<f32>, pixel: vec2<f32>) -> vec3<f32> {
    let time = lens.disk.w;
    let c = lens.centre.xyz;
    let star = lens.nova.x;
    let age = lens.nova.y;
    let life = lens.nova.z;
    let power = lens.nova.w;
    let scene = scene_distance(uv);
    let dist = length(c);
    let tc = dot(c, rd);
    // From the star to the ray's nearest point to it.
    let m = rd * tc - c;
    let miss = length(m);
    var e = vec3(0.0);

    // Whether the star itself is in view, or behind the world.
    var seen = 0.0;
    var streak = vec2(9.0);
    let clip = lens.view_proj * vec4(c, 1.0);
    if clip.w > 0.001 {
        let cuv = uv_of(clip);
        seen = 1.0;
        if all(cuv > vec2(0.0)) && all(cuv < vec2(1.0)) {
            seen = step(dist - 0.7, scene_distance(cuv));
        }
        let dims = vec2<f32>(textureDimensions(world));
        streak = (uv - cuv) * vec2(dims.x / dims.y, 1.0);
    }

    if tc > 0.0 && power > 0.0 && seen > 0.0 {
        // The corona: streamers round the star, a wide soft bloom, and the lens's streak.
        let size = max(star, 0.08);
        let x = miss / size;
        let axis = c / max(dist, 1e-4);
        let side = normalize(cross(axis, vec3(0.0, 1.0, 0.0)) + vec3(1e-5, 0.0, 0.0));
        let up = cross(side, axis);
        let ang = vec2(dot(m, side), dot(m, up)) / max(miss, 1e-4);
        let broad = noise3(vec3(ang * 2.5, time * 0.35));
        let fine = noise3(vec3(ang * 9.0, time * 0.7 + 3.0));
        let streamers = 0.3 + 1.4 * broad * broad + 0.7 * fine * fine * fine;
        let fall = pow(max(x, 1.0), -2.4);
        let reach = 1.0 - smoothstep(5.0, 12.0, x);
        e += power * fall * streamers * reach * 1.7
            * mix(vec3(1.0, 0.5, 0.16), vec3(1.0, 0.9, 0.72), fall);
        e += power * vec3(1.0, 0.58, 0.26) * 0.3 * exp(-x * 0.5);
        e += power * vec3(0.7, 0.82, 1.0) * 0.45 * exp(-abs(streak.y) * 240.0)
            / (1.0 + streak.x * streak.x * 30.0);
    }

    if age < 0.0 && star > 0.01 && tc > 0.0 {
        // The star's surface: boiling cells, hotter spots, darker towards its limb.
        let disc = star * star - miss * miss;
        let th = tc - sqrt(max(disc, 0.0));
        if disc > 0.0 && th > 0.0 && th < scene {
            let n = (rd * th - c) / star;
            let mu = clamp(dot(n, -rd), 0.0, 1.0);
            let spin = time * 0.25;
            let sn = vec3(n.x * cos(spin) - n.z * sin(spin), n.y, n.x * sin(spin) + n.z * cos(spin));
            let cells = fbm3(sn * 4.5 + vec3(0.0, time * 0.18, 0.0));
            let grain = noise3(sn * 17.0 + vec3(time * 0.9));
            let hot = clamp(cells * 1.5 + grain * 0.4 - 0.45, 0.0, 1.0);
            var col = mix(vec3(1.0, 0.3, 0.03), vec3(1.0, 0.76, 0.28), hot);
            col = mix(col, vec3(1.0, 0.97, 0.9), smoothstep(0.62, 1.0, hot));
            e += col * (0.28 + 0.72 * pow(mu, 0.55)) * (1.3 + 2.4 * power);
        }
    }

    if age >= 0.0 && life > 0.0 {
        // The shell: fast at first, slowing as it sweeps up the air.
        let rs = 0.3 + 2.1 * (1.0 - exp(-age * 1.25)) + 0.03 * age;
        let width = 0.07 + 0.06 * rs;
        let outer = rs + width * 2.2;
        let d2 = outer * outer - miss * miss;
        if d2 > 0.0 {
            let half = sqrt(d2);
            let t0 = max(tc - half, 0.0);
            let t1 = min(tc + half, scene);
            if t1 > t0 {
                let dt = (t1 - t0) / 32.0;
                let dither = fract(52.9829189 * fract(dot(pixel, vec2(0.06711056, 0.00583715))));
                var t = t0 + dt * dither;
                let fresh = exp(-age * 2.0);
                let gain = (0.42 + 3.2 * fresh) * life;
                for (var i = 0; i < 32; i++) {
                    let p = rd * t - c;
                    let r = length(p);
                    let off = (r - rs) / width;
                    let shell = exp(-off * off);
                    if shell > 0.02 {
                        let dir = p / max(r, 1e-3);
                        let f = fbm3(dir * 3.2 + vec3(4.1, 0.0, 1.7));
                        // Filaments where the gas folds, knotted along their length.
                        let ridge = pow(1.0 - abs(2.0 * f - 1.0), 7.0);
                        let knots = noise3(dir * 10.0 + vec3(r * 1.2));
                        let dens = shell * (0.035 + 3.0 * ridge * (0.3 + knots));
                        // Hydrogen red and oxygen blue-green, gold where it is densest.
                        let hue = noise3(dir * 2.0 + vec3(9.0, 2.0, 5.0));
                        var col = mix(vec3(1.0, 0.22, 0.42), vec3(0.25, 0.8, 1.0), smoothstep(0.35, 0.65, hue));
                        col = mix(col, vec3(1.0, 0.78, 0.4), ridge * 0.7);
                        col = mix(col, vec3(1.0, 0.95, 0.85), fresh);
                        e += dens * col * gain * dt;
                    }
                    // Thin violet gas left inside the bubble.
                    e += vec3(0.4, 0.26, 1.0) * 0.012 * (1.0 - smoothstep(0.3 * rs, rs, r)) * gain * dt;
                    t += dt;
                }
            }
        }
        // The pulsar left behind.
        if tc > 0.0 && tc < scene {
            let beat = 0.75 + 0.25 * sin(time * 31.0);
            e += vec3(0.7, 0.85, 1.0) * beat * life * smoothstep(0.15, 1.0, age)
                * 0.0022 / (miss * miss + 0.0011);
        }
    }

    e += crown_light(rd, scene, time);
    // The star outshines the world: the eye adapts, and everything else goes dark.
    let dimmed = base * (1.0 - 0.55 * lens.params.x);
    return dimmed + max(vec3(1.0) - dimmed, vec3(0.0)) * (vec3(1.0) - exp(-e));
}

// How far the world's picture is pulled towards a ball of energy at `c` (`amount` below 0:
// pushed away from it), as an offset to `uv`; nothing where the ball is behind the world.
fn pull(uv: vec2<f32>, c: vec3<f32>, radius: f32, amount: f32) -> vec2<f32> {
    let clip = lens.view_proj * vec4(c, 1.0);
    if radius <= 0.004 || clip.w <= 0.05 {
        return vec2(0.0);
    }
    let cuv = uv_of(clip);
    if all(cuv > vec2(0.0)) && all(cuv < vec2(1.0)) && scene_distance(cuv) < length(c) - radius - 0.3 {
        return vec2(0.0);
    }
    let dims = vec2<f32>(textureDimensions(world));
    let aspect = vec2(dims.x / dims.y, 1.0);
    let d = (uv - cuv) * aspect;
    let sr = radius / clip.w * 0.7;
    let l2 = dot(d, d);
    return normalize(d + vec2(1e-5, 0.0)) * amount * sr * sr * sr / (l2 + sr * sr) / aspect;
}

// The light round a ball of energy at `c`: a tight halo and a wide soft bloom, torn into
// streamers by `rough`; hidden where the world stands in front of the ball.
fn ball_glow(rd: vec3<f32>, c: vec3<f32>, radius: f32, scene: f32, time: f32, colour: vec3<f32>, rough: f32) -> vec3<f32> {
    let tc = dot(c, rd);
    if radius <= 0.004 || tc <= 0.0 || tc > scene + radius {
        return vec3(0.0);
    }
    let m = rd * tc - c;
    let miss = length(m);
    let x = miss / radius;
    let axis = c / max(length(c), 1e-4);
    let side = normalize(cross(axis, vec3(0.0, 1.0, 0.0)) + vec3(1e-5, 0.0, 0.0));
    let up = cross(side, axis);
    let ang = vec2(dot(m, side), dot(m, up)) / max(miss, 1e-4);
    let broad = noise3(vec3(ang * 2.6, time * 0.9));
    let fine = noise3(vec3(ang * 8.0, time * 2.1 + 3.0));
    let streamers = 1.0 + rough * (1.6 * broad * broad + 1.2 * fine * fine * fine - 0.5);
    let halo = 1.2 * pow(max(x, 1.0), -3.0) * (1.0 - smoothstep(4.0, 10.0, x));
    return colour * (halo * streamers + 0.22 * exp(-x * 0.65));
}

// The surface of a ball of energy at `c`, as the ray `rd` sees it: rgb its colour, a how
// much of the world behind it it covers. Its gas turns `spin` times as fast as the clock
// (below 0: the other way), `core` in its middle, `rim` at its limb, `veins` where it folds.
fn ball(rd: vec3<f32>, c: vec3<f32>, radius: f32, scene: f32, time: f32, core: vec3<f32>, rim: vec3<f32>, veins: vec3<f32>, spin: f32) -> vec4<f32> {
    let tc = dot(c, rd);
    let miss = length(rd * tc - c);
    let disc = radius * radius - miss * miss;
    let th = tc - sqrt(max(disc, 0.0));
    if radius <= 0.004 || disc <= 0.0 || th <= 0.0 || th >= scene {
        return vec4(0.0);
    }
    let n = (rd * th - c) / radius;
    let mu = clamp(dot(n, -rd), 0.0, 1.0);
    let a = time * spin;
    let sn = vec3(n.x * cos(a) - n.z * sin(a), n.y, n.x * sin(a) + n.z * cos(a));
    let f = fbm3(sn * 3.4 + vec3(0.0, time * 0.45 * spin, 0.0));
    let ridge = pow(1.0 - abs(2.0 * f - 1.0), 6.0);
    let grain = noise3(sn * 13.0 + vec3(time * 1.3));
    var col = mix(core, rim, pow(1.0 - mu, 2.0));
    col += veins * ridge * (0.35 + 0.65 * mu) * (0.6 + 0.8 * grain);
    col += rim * 1.5 * pow(1.0 - mu, 5.0);
    return vec4(col, smoothstep(0.0, 0.16, mu));
}

// The Purple Hollow: a blue orb that pulls the world in and a red one that pushes it away,
// the purple sphere they merge into, lightning crawling round it, and the wake it tears
// through the air once fired. `uv0` is this pixel, `rd` its ray.
fn hollow(uv0: vec2<f32>, rd: vec3<f32>) -> vec3<f32> {
    let time = lens.disk.w;
    let age = lens.nova.x;
    let power = lens.nova.y;
    let wake = lens.nova.z;
    let pc = lens.centre.xyz;
    let pr = lens.centre.w;
    let bc = lens.crown.xyz;
    let br = lens.crown.w;
    let rc = lens.red.xyz;
    let rr = lens.red.w;
    let bent = uv0 + pull(uv0, bc, br, 0.9) + pull(uv0, rc, rr, -0.9) + pull(uv0, pc, pr, 1.2);
    let uv = clamp(bent, vec2(0.001), vec2(0.999));
    let scene = scene_distance(uv0);
    // The sphere outshines the world round it.
    var col = textureSampleLevel(world, world_sampler, uv, 0.0).rgb * (1.0 - 0.6 * lens.params.x);
    var e = vec3(0.0);

    if age > 0.0 && wake > 0.0 {
        // The wake: a tunnel of violet light from where it was fired to where it is, brightest
        // just behind it.
        let a = lens.origin.xyz;
        let ab = pc - a;
        let d1 = dot(rd, ab);
        let s = clamp((d1 * dot(rd, a) - dot(ab, a)) / max(dot(ab, ab) - d1 * d1, 1e-6), 0.0, 1.0);
        let q = a + ab * s;
        let t = max(dot(rd, q), 0.0);
        if t <= scene {
            let d = length(rd * t - q);
            let w = max(pr, 0.3) * (0.3 + 0.7 * s);
            let streak = 0.55 + 0.9 * noise3(vec3(s * length(ab) * 0.7 - time * 5.0, d * 3.0 / w, time));
            let near = 0.2 + 0.8 * s * s;
            e += vec3(0.6, 0.28, 1.0) * exp(-d * d / (w * w)) * wake * near * streak * 1.5;
            e += vec3(1.0, 0.9, 1.0) * exp(-d * d / (w * w * 0.06)) * wake * s * s * 1.2;
        }
    }

    e += ball_glow(rd, bc, br, scene, time, vec3(0.2, 0.5, 1.0), 0.25);
    e += ball_glow(rd, rc, rr, scene, time, vec3(1.0, 0.16, 0.1), 1.0);
    e += ball_glow(rd, pc, pr, scene, time, vec3(0.62, 0.28, 1.0), 0.5) * power;
    // Lightning crawling round the purple sphere.
    let tc = dot(pc, rd);
    if pr > 0.004 && tc > 0.0 && tc < scene + pr {
        let m = rd * tc - pc;
        let miss = length(m);
        let x = miss / pr;
        let axis = pc / max(length(pc), 1e-4);
        let side = normalize(cross(axis, vec3(0.0, 1.0, 0.0)) + vec3(1e-5, 0.0, 0.0));
        let ang = vec2(dot(m, side), dot(m, cross(side, axis))) / max(miss, 1e-4);
        let bolt = noise3(vec3(ang * 3.2 + vec2(x * 1.3, 0.0), floor(time * 14.0) * 1.7));
        let arcs = pow(1.0 - abs(2.0 * bolt - 1.0), 16.0) * smoothstep(0.95, 1.1, x) * (1.0 - smoothstep(1.5, 3.0, x));
        e += vec3(0.9, 0.8, 1.0) * arcs * 2.5 * power;
    }
    col = col + max(vec3(1.0) - col, vec3(0.0)) * (vec3(1.0) - exp(-e));

    // The blue orb: nearly black inside, its light caught at the rim; the red one burning.
    let blue = ball(rd, bc, br, scene, time, vec3(0.0, 0.02, 0.14), vec3(0.3, 0.68, 1.0), vec3(0.25, 0.6, 1.0), -1.8);
    col = mix(col, blue.rgb, blue.a);
    let red = ball(rd, rc, rr, scene, time, vec3(0.95, 0.07, 0.05), vec3(1.0, 0.5, 0.32), vec3(1.0, 0.75, 0.45), 1.4);
    col = mix(col, red.rgb, red.a);
    let purple = ball(rd, pc, pr, scene, time, vec3(0.42, 0.1, 0.9), vec3(0.95, 0.78, 1.0), vec3(1.0, 0.92, 1.0), 2.2);
    col = mix(col, purple.rgb * (0.75 + 0.4 * power), purple.a);
    return col;
}

// False Dreams' eye `i`: its centre relative to the camera and its radius (0: not there).
// They drift round the player at their own heights and speeds, coming one after another.
fn dream_eye(i: i32) -> vec4<f32> {
    let time = lens.disk.w;
    let count = lens.nova.w;
    let fi = f32(i);
    let h1 = hash(vec2(fi, 1.7));
    let h2 = hash(vec2(fi, 9.1));
    let h3 = hash(vec2(fi, 4.3));
    let appear = clamp(lens.nova.x * count - fi * 0.85, 0.0, 1.0);
    let way = select(-1.0, 1.0, i % 2 == 0);
    let a = fi * 6.2832 / count + time * (0.16 + 0.1 * h1) * way;
    let reach = lens.nova.z * (0.75 + 0.5 * h2);
    let up = -0.7 + 1.9 * fract(fi * 0.618 + 0.3 * h3) + 0.18 * sin(time * (0.6 + 0.4 * h1) + fi * 2.0);
    let c = lens.centre.xyz + vec3(cos(a) * reach, up, sin(a) * reach);
    let r = lens.centre.w * (0.7 + 0.6 * h2) * smoothstep(0.0, 1.0, appear);
    return vec4(c, r);
}

// False Dreams: the player's own view swims, blurred towards its edges, drained and darkened;
// eyes float round them, each turned to whoever watches (the camera), glancing about and
// blinking now and then, a red glow round each. `uv0` is this pixel, `rd` its ray.
fn dreams(uv0: vec2<f32>, rd: vec3<f32>) -> vec3<f32> {
    let time = lens.disk.w;
    let blur = lens.nova.y;
    let count = i32(lens.nova.w);
    let dims = vec2<f32>(textureDimensions(world));
    var col = textureSampleLevel(world, world_sampler, uv0, 0.0).rgb;
    if blur > 0.0 {
        let edge = length((uv0 - 0.5) * vec2(dims.x / dims.y, 1.0));
        let radius = blur * (2.0 + 8.0 * smoothstep(0.1, 0.8, edge)) * (1.0 + 0.3 * sin(time * 1.3));
        var sum = col;
        for (var i = 0; i < 16; i++) {
            let fi = f32(i);
            let a = fi * 2.39996 + time * 0.5;
            let o = vec2(cos(a), sin(a)) * sqrt((fi + 0.5) / 16.0) * radius / dims;
            sum += textureSampleLevel(world, world_sampler, clamp(uv0 + o, vec2(0.001), vec2(0.999)), 0.0).rgb;
        }
        col = sum / 17.0;
        // A ghost of the world drifting beside it.
        let drift = vec2(sin(time * 0.7), cos(time * 0.53)) * blur * 7.0 / dims;
        let ghost = textureSampleLevel(world, world_sampler, clamp(uv0 + drift, vec2(0.001), vec2(0.999)), 0.0).rgb;
        col = mix(col, ghost, 0.3 * blur);
        let grey = dot(col, vec3(0.299, 0.587, 0.114));
        col = mix(col, vec3(grey) * vec3(0.95, 0.84, 0.92), 0.45 * blur);
        col *= 1.0 - 0.7 * blur * smoothstep(0.3, 0.95, edge);
        col += vec3(0.16, 0.0, 0.03) * blur * smoothstep(0.45, 1.0, edge);
    }

    let scene = scene_distance(uv0);
    // The red glow round each eye, behind them.
    var e = vec3(0.0);
    for (var i = 0; i < count; i++) {
        let eye = dream_eye(i);
        let c = eye.xyz;
        let r = eye.w;
        let tc = dot(c, rd);
        if r <= 0.002 || tc <= 0.0 || tc > scene + r {
            continue;
        }
        let x = length(rd * tc - c) / r;
        e += vec3(0.85, 0.08, 0.06) * (0.5 * pow(max(x, 1.0), -2.5) + 0.08 * exp(-x * 0.5));
    }
    col = col + max(vec3(1.0) - col, vec3(0.0)) * (vec3(1.0) - exp(-e));

    for (var i = 0; i < count; i++) {
        let eye = dream_eye(i);
        let c = eye.xyz;
        let r = eye.w;
        let tc = dot(c, rd);
        let miss2 = dot(c, c) - tc * tc;
        let disc = r * r - miss2;
        if r <= 0.002 || disc <= 0.0 {
            continue;
        }
        let th = tc - sqrt(disc);
        if th <= 0.0 || th >= scene {
            continue;
        }
        let fi = f32(i);
        let n = (rd * th - c) / r;
        // It looks at the camera (the origin), now and then glancing a little aside.
        let glance = floor(time * 0.8 + hash(vec2(fi, 2.9)) * 7.0);
        let aside = vec3(
            hash(vec2(glance, fi + 0.3)) - 0.5,
            hash(vec2(glance, fi + 5.1)) - 0.5,
            hash(vec2(glance, fi + 8.7)) - 0.5,
        ) * 0.25 * length(c) * step(0.6, hash(vec2(glance, fi + 3.3)));
        let g = normalize(-c + aside);
        let side = normalize(cross(vec3(0.0, 1.0, 0.0), g) + vec3(1e-5, 0.0, 0.0));
        let up = cross(g, side);
        let mu = dot(n, g);
        let u = dot(n, side);
        let v = dot(n, up);
        // Bare eyeballs; lids sweep over them only for a blink, a few times a minute.
        let b = fract(time * (0.12 + 0.05 * hash(vec2(fi, 4.3))) + hash(vec2(fi, 1.7)));
        let closed = smoothstep(0.0, 0.02, b) * (1.0 - smoothstep(0.03, 0.06, b));
        let opening = 1.6 * (1.0 - closed) * sqrt(max(1.0 - u * u / 0.9, 0.0));
        let light = 0.55 + 0.45 * clamp(dot(n, normalize(vec3(0.2, 1.0, 0.3))) * 0.5 + 0.5, 0.0, 1.0);
        var shade: vec3<f32>;
        if abs(v) > opening {
            let crease = 1.0 - smoothstep(0.0, 0.1, abs(v) - opening);
            shade = vec3(0.3, 0.22, 0.26) * light * (1.0 - 0.75 * crease);
        } else {
            // The white, bloodshot towards its edges.
            let f = fbm3(n * 5.0 + vec3(fi * 3.7));
            let vein = pow(1.0 - abs(2.0 * f - 1.0), 9.0) * (1.0 - smoothstep(0.2, 0.75, mu));
            var white = mix(vec3(0.72, 0.5, 0.46), vec3(0.95, 0.9, 0.84), smoothstep(0.0, 0.6, mu));
            white = mix(white, vec3(0.75, 0.04, 0.04), vein * 0.85);
            // The iris, a burning red-amber with fibres, a dark ring round it; the pupil.
            let iris = 0.8;
            let fibres = 0.55 + 0.45 * noise(vec2(atan2(v, u) * 7.0, (mu - iris) * 30.0 + fi * 11.0));
            var colour = mix(vec3(0.45, 0.02, 0.02), vec3(1.0, 0.5, 0.12), smoothstep(iris, 0.96, mu)) * fibres;
            colour *= 0.35 + 0.65 * smoothstep(iris, iris + 0.04, mu);
            shade = mix(white, colour, smoothstep(iris - 0.012, iris + 0.012, mu));
            let pupil = 0.955 - 0.012 * sin(time * 1.7 + fi);
            shade = mix(shade, vec3(0.01, 0.0, 0.0), smoothstep(pupil - 0.006, pupil + 0.006, mu));
            shade *= light * (1.0 - 0.55 * smoothstep(opening - 0.2, opening, v));
            // A wet highlight.
            let shine = normalize(g + up * 0.45 + side * 0.3);
            shade += vec3(1.0) * pow(max(dot(n, shine), 0.0), 160.0) * 1.4;
        }
        col = mix(col, shade, smoothstep(0.0, 0.12, dot(n, -rd)));
    }
    return col;
}

// The Rook (params.w 4): `centre` is the rook's base and its height in blocks; `disk.xyz` the
// way it blasts (level); `params.x` how much of it is left (it crumbles below 1); `nova` x:
// seconds since the blast (negative before), y: how hard the blast burns, z: how far the rook
// leans back (radians); `crown` the Brilliant badge's centre and size; `origin` where the blast
// starts.

fn sd_cyl(p: vec3<f32>, r: f32, y0: f32, y1: f32, rnd: f32) -> f32 {
    let d = vec2(length(p.xz) - r + rnd, abs(p.y - (y0 + y1) * 0.5) - (y1 - y0) * 0.5 + rnd);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2(0.0))) - rnd;
}

// A cone cut flat at `y0` (radius `r0`) and `y1` (radius `r1`).
fn sd_cone(p: vec3<f32>, y0: f32, y1: f32, r0: f32, r1: f32) -> f32 {
    let h = (y1 - y0) * 0.5;
    let q = vec2(length(p.xz), p.y - (y0 + y1) * 0.5);
    let k1 = vec2(r1, h);
    let k2 = vec2(r1 - r0, 2.0 * h);
    let ca = vec2(q.x - min(q.x, select(r1, r0, q.y < 0.0)), abs(q.y) - h);
    let cb = q - k1 + k2 * clamp(dot(k1 - q, k2) / dot(k2, k2), 0.0, 1.0);
    let s = select(1.0, -1.0, cb.x < 0.0 && ca.y < 0.0);
    return s * sqrt(min(dot(ca, ca), dot(cb, cb)));
}

fn sd_ring(p: vec3<f32>, y: f32, big: f32, small: f32) -> f32 {
    return length(vec2(length(p.xz) - big, p.y - y)) - small;
}

fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// A Staunton rook one unit tall, its base at the origin: a moulded foot, a tapering tower, a
// collar and a hollow head with six crenels.
fn rook_sdf(p: vec3<f32>) -> f32 {
    var d = sd_cyl(p, 0.38, 0.0, 0.075, 0.025);
    d = smin(d, sd_ring(p, 0.1, 0.325, 0.034), 0.02);
    d = smin(d, sd_cone(p, 0.11, 0.25, 0.32, 0.232), 0.03);
    d = smin(d, sd_cone(p, 0.22, 0.735, 0.232, 0.2), 0.04);
    d = smin(d, sd_ring(p, 0.738, 0.222, 0.028), 0.02);
    d = smin(d, sd_cyl(p, 0.272, 0.75, 0.79, 0.012), 0.015);
    d = smin(d, sd_cone(p, 0.78, 1.0, 0.272, 0.3) - 0.004, 0.02);
    // The hollow top, and the crenels round it.
    d = max(d, -sd_cyl(p, 0.2, 0.9, 1.3, 0.0));
    let sector = 6.28318 / 6.0;
    let a = atan2(p.z, p.x);
    let local = (fract(a / sector) - 0.5) * sector;
    let rr = length(p.xz);
    let along = rr * cos(local);
    let across = rr * sin(local);
    let notch = max(max(abs(across) - 0.056, 0.905 - p.y), 0.12 - along);
    return max(d, -notch);
}

// The rook's level frame: x to its right, z the way it blasts.
fn rook_frame() -> mat3x3<f32> {
    let f = normalize(vec3(lens.disk.x, 0.0, lens.disk.z) + vec3(1e-5, 0.0, 0.0));
    let right = normalize(cross(vec3(0.0, 1.0, 0.0), f));
    return mat3x3<f32>(right, vec3(0.0, 1.0, 0.0), f);
}

// A point relative to the camera into the rook's own space (one unit tall, leaning back).
fn rook_space(w: vec3<f32>, frame: mat3x3<f32>) -> vec3<f32> {
    let p = (w - lens.centre.xyz) / lens.centre.w;
    let l = vec3(dot(p, frame[0]), p.y, dot(p, frame[2]));
    let c = cos(lens.nova.z);
    let s = sin(lens.nova.z);
    return vec3(l.x, l.y * c - l.z * s, l.y * s + l.z * c);
}

// A direction in the rook's space back into the world.
fn rook_world(n: vec3<f32>, frame: mat3x3<f32>) -> vec3<f32> {
    let c = cos(lens.nova.z);
    let s = sin(lens.nova.z);
    let l = vec3(n.x, n.y * c + n.z * s, -n.y * s + n.z * c);
    return frame[0] * l.x + vec3(0.0, l.y, 0.0) + frame[2] * l.z;
}

// Fire: deep red, orange, yellow, white-hot.
fn fire(t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0);
    let a = mix(vec3(0.35, 0.03, 0.01), vec3(1.0, 0.32, 0.04), smoothstep(0.0, 0.45, x));
    let b = mix(a, vec3(1.0, 0.78, 0.25), smoothstep(0.4, 0.75, x));
    return mix(b, vec3(1.0, 0.97, 0.88), smoothstep(0.75, 1.0, x));
}

// The blast's density (x) and heat (y) at `w` (relative to the camera).
fn blast_at(w: vec3<f32>, age: f32) -> vec2<f32> {
    let f = rook_frame()[2];
    var p = w - lens.origin.xyz;
    // The smoke rises as it cools.
    p.y -= age * age * 0.3;
    let grow = 1.0 - exp(-age * 3.2);
    let reach = 1.0 + 8.5 * grow;
    let s = dot(p, f);
    let side = length(p - f * s);
    let radius = (0.55 + 0.4 * max(s, 0.0)) * (0.55 + 0.45 * grow) + 0.5 * age;
    let n = fbm3(p * 1.25 - f * age * 2.5 + vec3(0.0, -age * 0.7, 0.0));
    let ends = smoothstep(reach + 0.3, reach - 1.6, s) * smoothstep(-0.9, 0.3, s);
    let density = clamp((1.0 - side / radius) * 1.7 + (n - 0.5) * 1.6, 0.0, 1.0) * ends;
    let hot = clamp(1.25 - age * 0.5 - side / radius * 0.7 + (n - 0.5) * 0.6 - max(s - reach * 0.6, 0.0) * 0.12, 0.0, 1.0);
    return vec2(density, hot);
}

// The blast ahead of the rook, marched up to `limit` blocks: rgb is what it adds, a how much of
// what lies behind it hides.
fn blast(rd: vec3<f32>, limit: f32) -> vec4<f32> {
    let age = lens.nova.x;
    if age <= 0.0 || age > 4.6 {
        return vec4(0.0);
    }
    let f = rook_frame()[2];
    let grow = 1.0 - exp(-age * 3.2);
    let reach = 1.0 + 8.5 * grow;
    let c = lens.origin.xyz + f * reach * 0.5;
    let r = reach * 0.5 + 4.5 + age;
    let tc = dot(c, rd);
    let disc = r * r - (dot(c, c) - tc * tc);
    if disc <= 0.0 {
        return vec4(0.0);
    }
    let t0 = max(tc - sqrt(disc), 0.0);
    let t1 = min(tc + sqrt(disc), limit);
    if t1 <= t0 {
        return vec4(0.0);
    }
    let steps = 30;
    let dt = (t1 - t0) / f32(steps);
    let fade = 1.0 - smoothstep(3.0, 4.6, age);
    let power = lens.nova.y;
    var col = vec3(0.0);
    var through = 1.0;
    // Start each pixel's march a little off, so the steps don't show as bands.
    var t = t0 + dt * hash(rd.xy * 913.7 + rd.z * 37.1);
    for (var i = 0; i < steps; i++) {
        let s = blast_at(rd * t, age);
        if s.x > 0.001 {
            // Fire is dense; the smoke it leaves is thin, grey and soon gone.
            let smoky = 1.0 - smoothstep(0.15, 0.5, s.y);
            let thin = mix(2.4, 0.7 * (1.0 - smoothstep(1.2, 3.2, age)), smoky);
            let a = 1.0 - exp(-s.x * thin * dt * fade);
            let burn = fire(s.y) * s.y * s.y * 5.0 * power;
            let smoke = vec3(0.42, 0.4, 0.39) * (0.7 + 0.3 * s.x) * smoky;
            col += through * a * (burn + smoke);
            through *= 1.0 - a;
            if through < 0.02 {
                break;
            }
        }
        t += dt;
    }
    return vec4(col, 1.0 - through);
}

// The Brilliant badge: a teal disc with a white "!!", turned to the camera, glowing. rgb and a.
fn rook_badge(rd: vec3<f32>, scene: f32) -> vec4<f32> {
    let size = lens.crown.w;
    if size <= 0.001 {
        return vec4(0.0);
    }
    let c = lens.crown.xyz;
    let n = normalize(c);
    let denom = dot(rd, n);
    if denom <= 0.0 {
        return vec4(0.0);
    }
    let t = dot(c, n) / denom;
    if t <= 0.0 || t > scene {
        return vec4(0.0);
    }
    let right = normalize(cross(n, vec3(0.0, 1.0, 0.0)) + vec3(1e-5, 0.0, 0.0));
    let up = cross(right, n);
    let hit = rd * t - c;
    let radius = 0.55 * size;
    let uv = vec2(dot(hit, right), dot(hit, up)) / radius;
    let r = length(uv);
    let aa = 0.03;
    // The glow round it.
    let halo = vec3(0.1, 0.85, 0.72) * exp(-max(r - 1.0, 0.0) * 2.6) * 0.55 * smoothstep(1.0 - aa, 1.0 + aa, r);
    if r > 1.0 + aa {
        return vec4(halo, 0.0);
    }
    var col = mix(vec3(0.09, 0.56, 0.5), vec3(0.22, 0.8, 0.7), uv.y * 0.5 + 0.5);
    col = mix(col, vec3(0.05, 0.38, 0.34), smoothstep(0.86, 0.92, r));
    col += vec3(0.25) * smoothstep(0.55, 0.85, r) * (1.0 - smoothstep(0.85, 0.9, r)) * max(uv.y, 0.0);
    // "!!": two slanted marks, each a tapering bar over a dot, with a soft shadow.
    var glyph = 1e3;
    var shadow = 1e3;
    for (var k = 0; k < 2; k++) {
        let x0 = select(-0.21, 0.21, k == 1);
        for (var layer = 0; layer < 2; layer++) {
            let off = select(vec2(0.0), vec2(0.035, -0.045), layer == 1);
            var q = uv - vec2(x0, 0.0) - off;
            q.x -= q.y * 0.12;
            let w = mix(0.06, 0.105, clamp((q.y + 0.08) / 0.62, 0.0, 1.0));
            let bar = length(max(abs(q - vec2(0.0, 0.23)) - vec2(w - 0.035, 0.31 - 0.035), vec2(0.0))) - 0.035;
            let dot_ = length(q - vec2(0.0, -0.34)) - 0.095;
            let g = min(bar, dot_);
            if layer == 0 {
                glyph = min(glyph, g);
            } else {
                shadow = min(shadow, g);
            }
        }
    }
    col = mix(col, col * 0.55, (1.0 - smoothstep(-aa, aa, shadow)) * 0.6);
    col = mix(col, vec3(1.0), 1.0 - smoothstep(-aa, aa, glyph));
    let edge = 1.0 - smoothstep(1.0 - aa, 1.0 + aa, r);
    return vec4(col * edge + halo * (1.0 - edge), edge);
}

fn rook(uv: vec2<f32>, rd: vec3<f32>) -> vec3<f32> {
    var col = textureSampleLevel(world, world_sampler, uv, 0.0).rgb;
    let scene = scene_distance(uv);
    let shown = lens.params.x;
    let height = lens.centre.w;
    let frame = rook_frame();
    var near = scene;
    if shown > 0.0 {
        // The ray against the rook's bounding sphere, then marched.
        let bc = lens.centre.xyz + vec3(0.0, 0.5 * height, 0.0);
        let br = 0.7 * height;
        let tc = dot(bc, rd);
        let disc = br * br - (dot(bc, bc) - tc * tc);
        if disc > 0.0 {
            var t = max(tc - sqrt(disc), 0.0);
            let t1 = min(tc + sqrt(disc), scene);
            var hit = false;
            var edge = 0.0;
            for (var i = 0; i < 96; i++) {
                if t > t1 {
                    break;
                }
                let p = rook_space(rd * t, frame);
                let d = rook_sdf(p);
                if d < 0.0015 {
                    // Crumbling: a hole wherever the noise is under what's gone.
                    let grain = fbm3(p * 9.0);
                    let gone = 1.0 - shown;
                    if grain >= gone * 1.15 {
                        hit = true;
                        edge = 1.0 - smoothstep(0.0, 0.06, grain - gone * 1.15);
                        break;
                    }
                    t += max(abs(d), 0.004) * height;
                    continue;
                }
                t += d * height * 0.9;
            }
            if hit {
                near = t;
                let p = rook_space(rd * t, frame);
                let e = vec2(1.0, -1.0) * 0.0015;
                let no = normalize(
                    e.xyy * rook_sdf(p + e.xyy) + e.yyx * rook_sdf(p + e.yyx)
                        + e.yxy * rook_sdf(p + e.yxy) + e.xxx * rook_sdf(p + e.xxx),
                );
                let n = normalize(rook_world(no, frame));
                let light = normalize(vec3(0.35, 0.85, 0.4));
                // Polished ivory with faint veins.
                let vein = pow(1.0 - abs(2.0 * fbm3(p * vec3(6.0, 2.5, 6.0)) - 1.0), 6.0);
                let albedo = mix(vec3(0.94, 0.9, 0.81), vec3(0.78, 0.7, 0.6), vein * 0.35);
                var ao = mix(0.55, 1.0, smoothstep(0.0, 0.25, p.y));
                if p.y > 0.88 && length(p.xz) < 0.21 {
                    ao *= 0.45;
                }
                let diffuse = max(dot(n, light), 0.0);
                let ambient = 0.5 + 0.25 * n.y;
                let spec = pow(max(dot(n, normalize(light - rd)), 0.0), 70.0) * 0.9;
                let fresnel = 0.04 + 0.96 * pow(1.0 - max(dot(n, -rd), 0.0), 5.0);
                let env = background(reflect(rd, n));
                var shade = albedo * (ambient + diffuse * 0.75) * ao + vec3(spec) + env * fresnel * 0.55;
                // Teal light from the badge over it, and the blast's fire on its front.
                shade += vec3(0.1, 0.75, 0.65) * pow(1.0 - max(dot(n, -rd), 0.0), 3.0) * 0.4 * min(lens.crown.w, 1.0);
                let age = lens.nova.x;
                if age > 0.0 {
                    let burn = exp(-age * 1.4) * lens.nova.y;
                    shade += vec3(1.0, 0.5, 0.15) * max(dot(n, frame[2]), 0.0) * burn * 0.9;
                }
                shade = mix(shade, vec3(0.3, 1.0, 0.85) * 1.6, edge);
                col = shade;
            }
        }
    }
    let b = blast(rd, near);
    col = col * (1.0 - b.a) + b.rgb;
    let badge = rook_badge(rd, near);
    col = col * (1.0 - badge.a) + badge.rgb;
    return col;
}

@fragment fn fs_black_hole(in: PostOut) -> @location(0) vec4<f32> {
    let original = textureSampleLevel(world, world_sampler, in.uv, 0.0);
    let rs = lens.centre.w;
    let strength = lens.params.x;
    if rs <= 0.001 && lens.params.w < 1.5 {
        return vec4(original.rgb + lens.params.y, original.a);
    }
    let time = lens.disk.w;
    let ndc = vec2(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0);
    let far = lens.inv_view_proj * vec4(ndc, 0.5, 1.0);
    let rd = normalize(far.xyz / far.w);

    // The shock wave: a ring rippling outwards on the screen around the hole.
    var uv = in.uv;
    let centre_clip = lens.view_proj * vec4(lens.centre.xyz, 1.0);
    let shock = lens.params.z;
    if shock > 0.0 && centre_clip.w > 0.0 {
        let c = uv_of(centre_clip);
        let aspect = vec2<f32>(textureDimensions(world)).x / vec2<f32>(textureDimensions(world)).y;
        let d = (in.uv - c) * vec2(aspect, 1.0);
        let screen_r = shock * rs / centre_clip.w * 0.6;
        let ring = length(d) - screen_r;
        let wave = exp(-ring * ring * 900.0) * clamp(1.0 - shock / 60.0, 0.0, 1.0);
        uv = in.uv - normalize(d + vec2(1e-5)) * wave * 0.04 / vec2(aspect, 1.0);
    }
    if lens.params.w > 3.5 {
        return vec4(rook(uv, rd) + lens.params.y, original.a);
    }
    if lens.params.w > 2.5 {
        return vec4(dreams(uv, rd) + lens.params.y, original.a);
    }
    if lens.params.w > 1.5 {
        return vec4(hollow(uv, rd) + lens.params.y, original.a);
    }
    var base = textureSampleLevel(world, world_sampler, uv, 0.0).rgb;
    if lens.params.w > 0.5 {
        return vec4(supernova(base, uv, rd, in.clip.xy) + lens.params.y, original.a);
    }
    if strength <= 0.0 {
        // Gone: only the flash and the shock wave are left.
        return vec4(base + lens.params.y, original.a);
    }

    // What the scene has at this pixel, in horizon radii from the camera.
    let size = vec2<f32>(textureDimensions(scene_depth));
    let texel = clamp(vec2<i32>(uv * size), vec2(0), vec2<i32>(size) - 1);
    let z = textureLoad(scene_depth, texel, 0);
    var scene = 1e9;
    if z > 0.0 {
        let hit = lens.inv_view_proj * vec4(ndc, z, 1.0);
        if abs(hit.w) > 1e-7 {
            scene = length(hit.xyz / hit.w) / rs;
        }
    }

    // Everything below is in horizon radii, centred on the hole.
    var p = -lens.centre.xyz / rs;
    let closest_t = -dot(p, rd);
    let miss = length(p + rd * closest_t);
    // A soft violet glow around the hole even where rays barely bend.
    let halo = exp(-max(miss - 1.5, 0.0) * 0.35) * 0.18 * strength * smoothstep(2.2, 3.2, miss);
    if miss > REACH || closest_t < 0.0 {
        let vignette = 1.0 - 0.35 * strength * smoothstep(0.4, 1.2, length(in.uv - 0.5) * 1.6);
        return vec4((base + vec3(0.45, 0.2, 0.8) * halo) * vignette + lens.params.y, original.a);
    }
    let enter = max(closest_t - sqrt(REACH * REACH - miss * miss), 0.0);
    if scene < enter {
        // In front of everything the hole bends.
        return vec4(base + vec3(0.45, 0.2, 0.8) * halo + lens.params.y, original.a);
    }

    let n = normalize(lens.disk.xyz);
    let t1 = normalize(cross(n, vec3(0.0, 0.0, 1.0)));
    let t2 = cross(n, t1);
    p += rd * enter;
    var v = rd;
    var travelled = enter;
    let h = cross(p, v);
    let h2 = dot(h, h);
    var colour = vec3(0.0);
    var alpha = 0.0;
    var glow = 0.0;
    var behind = vec3(0.0);
    var done = false;
    for (var i = 0; i < 160; i++) {
        let r = length(p);
        let dt = clamp(0.07 * r, 0.015, 0.7);
        // Photons around a Schwarzschild hole: a = -3/2 h^2 r / |r|^5 (rs = 1).
        v += -1.5 * h2 * p / pow(r, 5.0) * dt;
        let next = p + v * dt;
        // The disk, where the ray crosses its plane.
        let y0 = dot(p, n);
        let y1 = dot(next, n);
        if y0 * y1 < 0.0 && alpha < 0.99 {
            let q = mix(p, next, y0 / (y0 - y1));
            let planar = vec2(dot(q, t1), dot(q, t2));
            let rq = length(planar);
            if rq > DISK_INNER * 0.8 && rq < DISK_OUTER {
                // Kepler: the inner disk turns fastest.
                let omega = 2.2 * pow(rq, -1.5);
                let a = -time * omega;
                let turned = mat2x2<f32>(cos(a), sin(a), -sin(a), cos(a)) * planar;
                let gas = fbm(turned * 0.9 + vec2(time * 0.05, 0.0));
                let streaks = fbm(vec2(rq * 3.5, atan2(turned.y, turned.x) * 2.0));
                let density = smoothstep(DISK_INNER * 0.8, DISK_INNER * 1.3, rq)
                    * (1.0 - smoothstep(DISK_OUTER * 0.6, DISK_OUTER, rq))
                    * clamp(0.25 + gas * 1.1 * (0.6 + streaks * 0.8), 0.0, 1.0);
                // Relativistic beaming: the side turning towards us is brighter and bluer.
                let orbit = normalize(cross(n, q));
                let beta = clamp(0.62 / sqrt(max(rq - 1.0, 0.3)), 0.0, 0.7);
                let doppler = 1.0 / (1.0 - beta * dot(orbit, -normalize(v)));
                let redshift = sqrt(max(1.0 - 1.0 / rq, 0.0));
                let t = clamp((1.0 - (rq - DISK_INNER) / (DISK_OUTER - DISK_INNER)) * 0.85 * doppler * redshift, 0.0, 1.0);
                let bright = pow(DISK_INNER / rq, 1.4) * 2.6 * pow(doppler, 3.0) * redshift;
                let a_here = clamp(density * 0.92, 0.0, 1.0);
                colour += (1.0 - alpha) * a_here * heat(t) * bright;
                alpha += (1.0 - alpha) * a_here;
            }
        }
        // The photon sphere glows faintly.
        glow += exp(-abs(r - 1.5) * 6.0) * dt * 0.12;
        travelled += dt * length(v);
        p = next;
        if travelled >= scene {
            // Something in the scene (the player orbiting) is reached first.
            behind = base;
            done = true;
            break;
        }
        if length(p) < 1.0 {
            behind = vec3(0.0);
            done = true;
            break;
        }
        if length(p) > REACH && dot(p, v) > 0.0 {
            behind = background(normalize(v));
            done = true;
            break;
        }
    }
    if !done {
        behind = vec3(0.0);
    }
    var bent = colour + (1.0 - alpha) * behind;
    bent += vec3(0.85, 0.55, 1.0) * glow * (0.8 + 0.2 * sin(time * 6.0));
    bent += vec3(0.45, 0.2, 0.8) * halo;
    let out = mix(base, bent, strength);
    let vignette = 1.0 - 0.35 * strength * smoothstep(0.4, 1.2, length(in.uv - 0.5) * 1.6);
    return vec4(out * vignette + lens.params.y, original.a);
}
