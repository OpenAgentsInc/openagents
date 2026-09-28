// Verse physical post-processing: bloom, exposure adaptation, and the output
// transform.
//
// Bloom is the energy-conserving mip chain of Jimenez, "Next Generation Post
// Processing in Call of Duty: Advanced Warfare" (SIGGRAPH 2014): a 13-tap
// downsample with a Karis average on the first level, then tent upsampling.
// The output transform is the Khronos PBR Neutral tone mapper (Khronos Group,
// Apache-2.0 reference), after white balance, local exposure, lens ghosts,
// lateral chromatic aberration, and vignetting; sensor grain follows it.

struct Post {
    // x texel width, y texel height of the source; z Karis flag; w unused.
    source: vec4<f32>,
    // x bloom strength; y local exposure; z grain; w vignette.
    look: vec4<f32>,
    // x fringe (pixels); y ghosts; z time; w auto exposure (0 or 1).
    lens: vec4<f32>,
    // rgb white-balance gains; w adaptation blend this frame.
    balance: vec4<f32>,
    // x target signal for auto exposure; y min gain; z max gain; w 1 for the
    // hue-preserving output curve.
    adapt: vec4<f32>,
    // x the output ceiling: 1.0 on a standard display, the headroom over
    // reference white on an extended-range (HDR) surface.
    output: vec4<f32>,
};

@group(0) @binding(0) var<uniform> p: Post;
@group(0) @binding(1) var source: texture_2d<f32>;
@group(0) @binding(2) var clamp_linear: sampler;
@group(0) @binding(3) var bloom: texture_2d<f32>;
@group(0) @binding(4) var adapted: texture_2d<f32>;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Out {
    let q = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    var o: Out;
    o.clip = vec4<f32>(q * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(q.x, 1.0 - q.y);
    return o;
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn tap(uv: vec2<f32>, dx: f32, dy: f32) -> vec3<f32> {
    return textureSampleLevel(source, clamp_linear, uv + vec2<f32>(dx, dy) * p.source.xy, 0.0).rgb;
}

fn karis(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>, d: vec3<f32>) -> vec3<f32> {
    let wa = 1.0 / (1.0 + luma(a));
    let wb = 1.0 / (1.0 + luma(b));
    let wc = 1.0 / (1.0 + luma(c));
    let wd = 1.0 / (1.0 + luma(d));
    return (a * wa + b * wb + c * wc + d * wd) / (wa + wb + wc + wd);
}

@fragment
fn fs_down(i: Out) -> @location(0) vec4<f32> {
    let uv = i.uv;
    let a = tap(uv, -2.0, -2.0);
    let b = tap(uv, 0.0, -2.0);
    let c = tap(uv, 2.0, -2.0);
    let d = tap(uv, -2.0, 0.0);
    let e = tap(uv, 0.0, 0.0);
    let f = tap(uv, 2.0, 0.0);
    let g = tap(uv, -2.0, 2.0);
    let h = tap(uv, 0.0, 2.0);
    let k = tap(uv, 2.0, 2.0);
    let j = tap(uv, -1.0, -1.0);
    let l = tap(uv, 1.0, -1.0);
    let m = tap(uv, -1.0, 1.0);
    let n = tap(uv, 1.0, 1.0);
    if p.source.z > 0.5 {
        // Karis average of the five 2×2 blocks keeps fireflies from blooming.
        let center = karis(j, l, m, n) * 0.5;
        let tl = karis(a, b, d, e) * 0.125;
        let tr = karis(b, c, e, f) * 0.125;
        let bl = karis(d, e, g, h) * 0.125;
        let br = karis(e, f, h, k) * 0.125;
        return vec4<f32>(center + tl + tr + bl + br, 1.0);
    }
    var c0 = e * 0.125;
    c0 += (a + c + g + k) * 0.03125;
    c0 += (b + d + f + h) * 0.0625;
    c0 += (j + l + m + n) * 0.125;
    return vec4<f32>(c0, 1.0);
}

@fragment
fn fs_up(i: Out) -> @location(0) vec4<f32> {
    let uv = i.uv;
    var c = tap(uv, 0.0, 0.0) * 4.0;
    c += (tap(uv, -1.0, 0.0) + tap(uv, 1.0, 0.0) + tap(uv, 0.0, -1.0) + tap(uv, 0.0, 1.0)) * 2.0;
    c += tap(uv, -1.0, -1.0) + tap(uv, 1.0, -1.0) + tap(uv, -1.0, 1.0) + tap(uv, 1.0, 1.0);
    return vec4<f32>(c / 16.0, 1.0);
}

// Adaptation: blend the previous adapted luminance toward the scene average,
// which is the smallest bloom level.
@fragment
fn fs_adapt(i: Out) -> @location(0) vec4<f32> {
    // Center-weighted log average over lit pixels, excluding empty space, so a
    // sunlit suit against black sky is metered as the suit. A high power mean
    // of the same samples guards highlights, as matrix metering does.
    var sum = 0.0;
    var high = 0.0;
    var weight = 0.0;
    for (var y = 0; y < 10; y++) {
        for (var x = 0; x < 16; x++) {
            let uv = (vec2<f32>(f32(x), f32(y)) + 0.5) / vec2<f32>(16.0, 10.0);
            let l = luma(textureSampleLevel(source, clamp_linear, uv, 0.0).rgb);
            let d = uv - 0.5;
            let w = select(0.0, 1.0 - dot(d, d) * 1.6, l > 1e-4);
            sum += log2(max(l, 1e-4)) * w;
            // Clamp so a few pixels of Sun do not decide the exposure.
            let c = min(l, 2.0);
            high += c * c * c * c * w;
            weight += w;
        }
    }
    let previous = textureLoad(adapted, vec2<i32>(0, 0), 0).r;
    if weight < 0.5 {
        // Nothing lit in view: hold the previous exposure.
        return vec4<f32>(max(previous, 0.18), 0.0, 0.0, 1.0);
    }
    let average = exp2(sum / weight);
    let bright = sqrt(sqrt(high / weight));
    // The equivalent average that keeps the high mean near 0.9.
    let scene = max(average, bright * 0.18 / 0.9);
    var value = mix(previous, scene, p.balance.w);
    if previous <= 0.0 {
        value = scene;
    }
    return vec4<f32>(value, 0.0, 0.0, 1.0);
}

// Khronos PBR Neutral, with its shoulder generalized to approach `ceiling`
// instead of 1.0. Below the shoulder's start the curve is unchanged, so
// standard-range content looks the same on an HDR display; highlights above
// it keep rising into the display's headroom.
fn neutral(color: vec3<f32>, ceiling: f32) -> vec3<f32> {
    let start = 0.8 - 0.04;
    let desaturation = 0.15;
    let x = min(color.r, min(color.g, color.b));
    let offset = select(0.04, x - 6.25 * x * x, x < 0.08);
    var c = color - offset;
    let peak = max(c.r, max(c.g, c.b));
    if peak < start {
        return c;
    }
    let d = ceiling - start;
    let new_peak = ceiling - d * d / (peak + d - start);
    c *= new_peak / peak;
    let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return mix(c, vec3<f32>(new_peak), g);
}

// Hue-preserving shoulder for the neon stage: a bright amber core compresses
// along its own hue instead of washing toward white.
fn hue_shoulder(color: vec3<f32>, ceiling: f32) -> vec3<f32> {
    let peak = max(color.r, max(color.g, color.b));
    let start = 0.76;
    if peak < start {
        return color;
    }
    let d = ceiling - start;
    return color * ((ceiling - d * d / (peak + d - start)) / peak);
}

fn hash(q: vec2<f32>) -> f32 {
    var r = fract(q * vec2<f32>(123.34, 456.21));
    r += dot(r, r + 45.32);
    return fract(r.x * r.y);
}

@fragment
fn fs_output(i: Out) -> @location(0) vec4<f32> {
    let uv = i.uv;
    let size = vec2<f32>(textureDimensions(source));
    // Lateral chromatic aberration grows toward the corners.
    let from_center = uv - 0.5;
    let shift = from_center * p.lens.x * 2.0 / size;
    var c = vec3<f32>(
        textureSampleLevel(source, clamp_linear, uv - shift, 0.0).r,
        textureSampleLevel(source, clamp_linear, uv, 0.0).g,
        textureSampleLevel(source, clamp_linear, uv + shift, 0.0).b
    );
    let levels = textureNumLevels(bloom);
    let glow = textureSampleLevel(bloom, clamp_linear, uv, 0.0).rgb * p.source.w;
    c = mix(c, glow, p.look.x);
    // Lens ghosts: the bloom mirrored through the center at a few scales.
    if p.lens.y > 0.0 {
        let ghost_level = f32(min(3u, levels - 1u));
        let tints = array<vec3<f32>, 3>(vec3<f32>(0.2, 0.5, 1.0), vec3<f32>(1.0, 0.6, 0.2), vec3<f32>(0.4, 1.0, 0.6));
        let scales = array<f32, 3>(-0.6, -1.2, 0.4);
        for (var k = 0; k < 3; k++) {
            let g = 0.5 + from_center * scales[k];
            // Fade before the texture edge so clamped samples never repeat.
            let edge = min(min(g.x, 1.0 - g.x), min(g.y, 1.0 - g.y));
            let fade = smoothstep(0.0, 0.15, edge) * (1.0 - smoothstep(0.3, 0.5, length(g - 0.5)));
            c += textureSampleLevel(bloom, clamp_linear, g, ghost_level).rgb * tints[k] * fade * p.lens.y * 2.0e-5;
        }
    }
    // Exposure adaptation, clamped to the camera's range.
    if p.lens.w > 0.5 {
        let avg = max(textureLoad(adapted, vec2<i32>(0, 0), 0).r, 1e-5);
        c *= clamp(p.adapt.x / avg, p.adapt.y, p.adapt.z);
    }
    // Local exposure: lift regions whose blurred surround is dark, like a
    // phone camera's multi-frame HDR, but never darken.
    if p.look.y > 0.0 {
        let local_level = f32(min(4u, levels - 1u));
        let surround = max(luma(textureSampleLevel(bloom, clamp_linear, uv, local_level).rgb), 1e-4);
        let lift = clamp(pow(0.18 / surround, p.look.y * 0.5), 1.0, 1.0 + 7.0 * p.look.y);
        let here = luma(c);
        // Keep highlights intact: fade the lift out as the pixel brightens.
        c *= mix(lift, 1.0, smoothstep(0.05, 0.6, here));
    }
    c *= p.balance.rgb;
    // Natural vignetting, cos⁴ of the field angle.
    let r2 = dot(from_center, from_center) * 2.0;
    c *= mix(1.0, pow(1.0 / (1.0 + r2), 2.0), p.look.w);
    var o: vec3<f32>;
    if p.adapt.w > 0.5 {
        o = hue_shoulder(max(c, vec3<f32>(0.0)), p.output.x);
    } else {
        o = neutral(max(c, vec3<f32>(0.0)), p.output.x);
    }
    // Sensor grain, stronger in shadows, applied in display space.
    if p.look.z > 0.0 {
        let n = hash(i.clip.xy + fract(p.lens.z) * 97.0) - 0.5;
        let shadow = 1.0 - smoothstep(0.0, 0.5, luma(o));
        o += vec3<f32>(n) * 0.03 * p.look.z * (0.3 + shadow);
    }
    return vec4<f32>(clamp(o, vec3<f32>(0.0), vec3<f32>(p.output.x)), 1.0);
}
