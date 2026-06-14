// Title-screen FROSTED GLASS post-process.
//
// Reads the 3D layer (a white paper + dark "character" circles) and returns it
// blurred + FBM-warped, so the circles read as soft, ethereal, drifting
// drop-shadows seen through a sheet of glass. Runs in the Core3d graph between
// Tonemapping and EndMainPassPostProcessing; the UI text/buttons composite on
// top AFTERWARDS (the UI pass runs later), so they stay razor-sharp.
//
// FBM noise mirrors `arena_fog.wgsl` (value-noise, 5 octaves).

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

struct GlassSettings {
    params: vec4<f32>, // x = blur radius (texels), y = distort, z = noise scale, w = grain
    anim: vec4<f32>,   // x = time seconds (y,z,w spare)
    tint: vec4<f32>,   // rgb = ethereal tint (theme accent), a = tint strength
};
@group(0) @binding(2) var<uniform> settings: GlassSettings;

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
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(screen_texture));
    let blur = settings.params.x;
    let distort = settings.params.y;
    let nscale = settings.params.z;
    let grain = settings.params.w;
    let t = settings.anim.x;

    // 1) Glassy refraction — warp the lookup by a slow, drifting FBM field.
    let nx = fbm(in.uv * nscale + vec2<f32>(t * 0.03, 0.0));
    let ny = fbm(in.uv * nscale + vec2<f32>(5.2, 1.3) + vec2<f32>(0.0, t * 0.03));
    let warp = (vec2<f32>(nx, ny) - 0.5) * distort;
    let uv = in.uv + warp;

    // 2) Frosted blur — a 16-tap golden-angle disc, centre-weighted. Taps near
    //    the edges read outside [0,1]; that's fine because the sampler clamps to
    //    edge (default) and the background is a full-bleed solid paper colour.
    var acc = vec3<f32>(0.0);
    var wsum = 0.0;
    for (var i = 0; i < 16; i = i + 1) {
        let fi = f32(i);
        let ang = fi * 2.39996323;                  // golden angle (rad)
        let rad = sqrt((fi + 0.5) / 16.0) * blur;   // even disc coverage
        let off = vec2<f32>(cos(ang), sin(ang)) * rad * texel;
        let w = 1.0 - rad / (blur + 1.0);           // soft centre weight
        acc = acc + textureSample(screen_texture, screen_sampler, uv + off).rgb * w;
        wsum = wsum + w;
    }
    var col = acc / max(wsum, 0.0001);

    // 3) Ethereal tint (a whisper of the active theme accent) + fine grain so
    //    the sheet itself reads as "blur noise".
    col = mix(col, settings.tint.rgb, settings.tint.a);
    // Grain time is wrapped (fract) so the hash stays precise — an unbounded `t`
    // would lose f32 precision after minutes idle and freeze the noise.
    let gt = fract(t * 2.0);
    let g = (hash2(in.uv * vec2<f32>(textureDimensions(screen_texture)) + vec2<f32>(gt, -gt)) - 0.5) * grain;
    col = col + vec3<f32>(g);

    return vec4<f32>(col, 1.0);
}
