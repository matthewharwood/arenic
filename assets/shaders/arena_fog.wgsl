// Per-arena ATMOSPHERE: a cloud / fog / vignette shader rendered on two
// camera-attached planes for dynamic depth layering (see foreground.rs):
//   - a SKYBOX plane far behind the (semi-transparent) arena floor, and
//   - a FOREGROUND plane between the camera and the floor.
// `mode.x` selects which: skybox = full coverage; foreground = edge-framed so
// the boss stays clear. Clouds always carry the theme ACCENT so they read on a
// dark scene. Animated from `globals.time`; shaped per arena by `flags`.

#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

struct CloudParams {
    tint: vec4<f32>,     // rgb = cloud mass (dark), a = coverage
    accent: vec4<f32>,   // rgb = cloud glow (theme accent), a = animation speed
    vignette: vec4<f32>, // a = corner-vignette strength (rgb unused)
    flags: vec4<f32>,    // x = style, y = noise scale, z = drift.x, w = drift.y
    mode: vec4<f32>,     // x: 0 = skybox (full), 1 = foreground (edge-framed); y: alpha scale
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: CloudParams;

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2(i);
    let b = hash2(i + vec2<f32>(1.0, 0.0));
    let c = hash2(i + vec2<f32>(0.0, 1.0));
    let d = hash2(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 5; i = i + 1) {
        v = v + amp * vnoise(q);
        q = q * 2.0;
        amp = amp * 0.5;
    }
    return v;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = globals.time;
    let style = params.flags.x;
    let scale = params.flags.y;
    let drift = vec2<f32>(params.flags.z, params.flags.w);
    let coverage = params.tint.a;
    let speed = params.accent.a;
    let cdist = distance(uv, vec2<f32>(0.5, 0.5));

    let p = uv * scale + drift * (t * speed * 6.0);
    var field = fbm(p);

    if (style < 0.5) {
        // 0 — banded stratus: soft + wide (spread out, blurred, faint)
        let band = 0.5 + 0.5 * sin((uv.x + uv.y) * scale * 0.8 + t * speed);
        field = mix(fbm(p), band, 0.35);
    } else if (style < 1.5) {
        // 1 — rolling billows (two opposing layers)
        field = mix(fbm(p), fbm(p * 1.7 - drift * (t * speed * 4.0)), 0.45);
    } else if (style < 2.5) {
        // 2 — vertical (lid / updraft); density biased by drift.y below
        field = fbm(p);
    } else if (style < 3.5) {
        // 3 — lazy spiral (cigar smoke)
        let c = uv - vec2<f32>(0.5, 0.5);
        let r = length(c);
        let ang = atan2(c.y, c.x) + r * 6.0 - t * speed * 0.5;
        let sp = vec2<f32>(cos(ang), sin(ang)) * r * scale + vec2<f32>(t * speed * 0.3, 0.0);
        field = fbm(sp);
    } else if (style < 4.5) {
        // 4 — rising embers (soft glowing motes drifting up)
        let suv = uv * scale + vec2<f32>(0.0, -t * speed * 0.2);
        let g = floor(suv);
        let f = fract(suv);
        let h = hash2(g);
        let d = length(f - vec2<f32>(0.5, 0.5));
        let tw = 0.55 + 0.45 * sin(t * speed * 2.0 + h * 40.0);
        // Large, soft, sparse, faint motes melted into haze (spread + blurred).
        let mote = (1.0 - smoothstep(0.0, 0.7, d)) * step(0.62, h) * tw * 0.7;
        field = max(mote, fbm(p) * 0.4);
    } else if (style < 5.5) {
        // 5 — searchlight sweep: a wide soft band sweeping over a base haze
        let pos = fract(t * speed * 0.08);
        let dx = abs(uv.x - pos);
        let wrapped = min(dx, 1.0 - dx);
        // Wide, soft, faint band over a base haze (spread out, blurred).
        let sweep = 1.0 - smoothstep(0.0, 0.42, wrapped);
        field = max(fbm(p) * 0.55, sweep * 0.7);
    } else if (style < 6.5) {
        // 6 — rhythmic pulse: the whole haze brightens on a beat (festival)
        let beat = 0.45 + 0.55 * pow(0.5 + 0.5 * sin(t * speed * 2.4), 2.0);
        field = fbm(p) * (0.35 + 0.9 * beat);
    } else if (style < 7.5) {
        // 7 — hearth: a gently breathing warm haze (home)
        let breathe = 0.7 + 0.3 * sin(t * speed * 1.4);
        field = fbm(p) * (0.55 + 0.5 * breathe);
    } else if (style < 8.5) {
        // 8 — fine drifting grain / dust (high register — a FOREGROUND voice)
        let q = uv * scale + drift * (t * speed * 8.0);
        field = pow(fbm(q), 1.5);
    } else if (style < 9.5) {
        // 9 — elongated streaks ALONG the drift axis (a FOREGROUND voice)
        let dir = normalize(drift + vec2<f32>(0.0001, 0.0));
        let perp = vec2<f32>(-dir.y, dir.x);
        let c = uv - vec2<f32>(0.5, 0.5);
        let q = vec2<f32>(dot(c, perp) * scale, dot(c, dir) * scale * 0.22) - vec2<f32>(0.0, t * speed * 0.6);
        field = fbm(q);
    } else {
        // 10 — concentric ripples expanding from centre (a FOREGROUND voice)
        let r = length(uv - vec2<f32>(0.5, 0.5));
        let ring = 0.5 + 0.5 * sin(r * scale * 4.0 - t * speed * 2.0);
        field = ring * (1.0 - smoothstep(0.1, 0.75, r)) + fbm(uv * scale + drift * t * speed) * 0.2;
    }

    var density = smoothstep(1.0 - coverage, 1.0, field);

    // Vertical density bias for style 2: lid (drift down) vs updraft (drift up).
    if (style > 1.5 && style < 2.5) {
        if (drift.y < 0.0) {
            density = density * mix(0.55, 1.15, uv.y); // lid: denser at top
        } else {
            density = density * mix(1.15, 0.55, uv.y); // updraft: denser at bottom
        }
    }

    // Layer framing: the skybox covers the whole backdrop (seen through the
    // translucent floor); the foreground clears the centre so the boss reads.
    var frame = mix(0.55, 1.0, smoothstep(0.0, 0.6, cdist)); // skybox default
    if (params.mode.x > 0.5) {
        frame = smoothstep(0.13, 0.45, cdist); // foreground: clear centre
    }
    density = clamp(density * frame, 0.0, 1.0);

    // Cloud colour ALWAYS carries the accent so it reads on a dark scene.
    let cloud_rgb = mix(params.tint.rgb, params.accent.rgb, 0.35 + density * 0.55);
    let cloud_a = density * 0.85;

    // A subtle true-black corner vignette (a real frame, visible even on dark).
    let corner = smoothstep(0.62, 0.92, cdist) * params.vignette.a;

    let out_rgb = mix(cloud_rgb, vec3<f32>(0.0, 0.0, 0.0), corner);
    let out_a = clamp(cloud_a + corner, 0.0, 0.9) * params.mode.y;
    return vec4<f32>(out_rgb, out_a);
}
