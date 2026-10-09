// Temporal anti-aliasing for the physical path (`pbr::taa`).
//
// Each frame renders with a sub-pixel jitter from the Halton (2, 3)
// sequence. `fs_resolve` reprojects every pixel into the last frame's
// history through the depth prepass and the last frame's camera, clamps
// the history to the current frame's 3 by 3 neighborhood in YCoCg (a box
// tightened toward the neighborhood's mean and spread), and blends it with
// the current frame, weighting both by inverse luminance so a bright spark
// does not dominate the average. `fs_sharpen` then restores the contrast
// the blending and the jitter soften, bounded by the neighborhood so it
// never rings. The ideas follow Karis, "High Quality Temporal
// Supersampling" (SIGGRAPH 2014), and Salvi's variance clipping (GDC 2016);
// the code is our own.

struct Taa {
    // This frame's unjittered, reversed-depth clip to world transform.
    inv_view_proj: mat4x4<f32>,
    // The last frame's unjittered, reversed-depth world to clip transform.
    prev_view_proj: mat4x4<f32>,
    // This frame's unjittered, reversed-depth world to clip transform.
    view_proj: mat4x4<f32>,
    // Width, height, and their reciprocals.
    size: vec4<f32>,
    // x the current frame's weight; y the weight once motion is fast; z the
    // speed in pixels a frame at which it is; w 1 to drop the history.
    blend: vec4<f32>,
    // x sharpening strength; y the variance clip's width in standard
    // deviations once the pixel moves a pixel a frame, z while it holds
    // still; w how far, as a fraction of depth, the history's surface may
    // lie from this pixel's before it counts as another surface.
    sharpen: vec4<f32>,
    // xy this frame's jitter, pixels: where the scene drew moved by it.
    jitter: vec4<f32>,
};

@group(0) @binding(0) var<uniform> taa: Taa;
@group(0) @binding(1) var scene_depth: texture_depth_2d;
// The resolve reads this frame's scene and the last history; the sharpen
// pass reads the new history from the same slot.
@group(0) @binding(2) var current: texture_2d<f32>;
@group(0) @binding(3) var history: texture_2d<f32>;
@group(0) @binding(4) var linear_clamp: sampler;
// The last frame's depth history: each pixel's clip-space w.
@group(0) @binding(5) var depth_history: texture_2d<f32>;

struct Resolved {
    @location(0) color: vec4<f32>,
    @location(1) depth: f32,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn rgb_to_ycocg(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.25 * c.r + 0.5 * c.g + 0.25 * c.b,
        0.5 * c.r - 0.5 * c.b,
        -0.25 * c.r + 0.5 * c.g - 0.25 * c.b,
    );
}

fn ycocg_to_rgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(c.x + c.y - c.z, c.x + c.z, c.x - c.y - c.z);
}

fn load(t: texture_2d<f32>, p: vec2<i32>) -> vec3<f32> {
    let size = vec2<i32>(textureDimensions(t));
    return max(textureLoad(t, clamp(p, vec2<i32>(0), size - 1), 0).rgb, vec3<f32>(0.0));
}

// Where pixel `p`'s surface is: its clip-space w this frame, and where it
// was on the last frame's screen, in pixels, with its w there.
struct Surface {
    w: f32,
    was: vec2<f32>,
    was_w: f32,
};

fn reproject(p: vec2<i32>) -> Surface {
    let size = vec2<i32>(textureDimensions(scene_depth));
    // The nearest surface in the 3 by 3 neighborhood (reversed depth: the
    // largest), so a pixel on a silhouette moves with what stands in front
    // whichever side of the edge the jitter put its center: otherwise it
    // reprojects with the roof one frame and the sky the next, and crawls.
    var d = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let q = clamp(p + vec2<i32>(x, y), vec2<i32>(0), size - 1);
            d = max(d, textureLoad(scene_depth, q, 0));
        }
    }
    // The sky has no depth: reproject it from very far away, by direction.
    d = max(d, 1e-7);
    let uv = (vec2<f32>(p) + 0.5) * taa.size.zw;
    let world = taa.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let at = vec4<f32>(world.xyz / world.w, 1.0);
    let now = taa.view_proj * at;
    let c = taa.prev_view_proj * at;
    if c.w <= 1e-6 {
        return Surface(now.w, vec2<f32>(-1e4), c.w);
    }
    let ndc = c.xy / c.w;
    let was = vec2<f32>((ndc.x * 0.5 + 0.5) * taa.size.x, (0.5 - ndc.y * 0.5) * taa.size.y);
    return Surface(now.w, was, c.w);
}

// How far, as a fraction of `w`, the nearest of the depth history's four
// pixels round `p` lies from depth `w`: zero where the history holds the
// same surface, large where it holds another (a chunk that moved, or what
// one uncovered).
fn depth_mismatch(p: vec2<f32>, w: f32) -> f32 {
    let size = vec2<i32>(textureDimensions(depth_history));
    let base = vec2<i32>(floor(p - 0.5));
    var best = 1e9;
    for (var y = 0; y <= 1; y++) {
        for (var x = 0; x <= 1; x++) {
            let q = clamp(base + vec2<i32>(x, y), vec2<i32>(0), size - 1);
            let h = textureLoad(depth_history, q, 0).r;
            best = min(best, abs(h - w));
        }
    }
    return best / max(w, 1e-4);
}

// The history at `p` pixels through a Catmull-Rom filter, from five
// bilinear taps: a bilinear read alone softens the image a little every
// frame the camera moves, and the softening accumulates.
fn history_at(p: vec2<f32>) -> vec3<f32> {
    let center = floor(p - 0.5) + 0.5;
    let f = p - center;
    let f2 = f * f;
    let f3 = f2 * f;
    let w0 = f2 - 0.5 * (f3 + f);
    let w1 = 1.5 * f3 - 2.5 * f2 + 1.0;
    let w3 = 0.5 * (f3 - f2);
    let w2 = 1.0 - w0 - w1 - w3;
    let w12 = w1 + w2;
    let t0 = (center - 1.0) * taa.size.zw;
    let t12 = (center + w2 / w12) * taa.size.zw;
    let t3 = (center + 2.0) * taa.size.zw;
    var sum = vec3<f32>(0.0);
    var weight = 0.0;
    let taps = array<vec3<f32>, 5>(
        vec3<f32>(t12.x, t0.y, w12.x * w0.y),
        vec3<f32>(t0.x, t12.y, w0.x * w12.y),
        vec3<f32>(t12.x, t12.y, w12.x * w12.y),
        vec3<f32>(t3.x, t12.y, w3.x * w12.y),
        vec3<f32>(t12.x, t3.y, w12.x * w3.y),
    );
    for (var i = 0; i < 5; i++) {
        let tap = taps[i];
        sum += textureSampleLevel(history, linear_clamp, tap.xy, 0.0).rgb * tap.z;
        weight += tap.z;
    }
    return sum / max(weight, 1e-6);
}

fn luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_resolve(@builtin(position) frag: vec4<f32>) -> Resolved {
    let p = vec2<i32>(frag.xy);
    // The neighborhood's box, mean, and spread in YCoCg, and the current
    // frame at this pixel's center: each neighbor's sample was drawn a
    // jitter away from its own center, so it is weighted by its distance
    // from this one (a Gaussian close to Blackman-Harris), and the jitter
    // never reaches the output.
    var lo = vec3<f32>(1e9);
    var hi = vec3<f32>(-1e9);
    var sum = vec3<f32>(0.0);
    var sum2 = vec3<f32>(0.0);
    var here = vec3<f32>(0.0);
    var weights = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let rgb = load(current, p + vec2<i32>(x, y));
            let c = rgb_to_ycocg(rgb);
            lo = min(lo, c);
            hi = max(hi, c);
            sum += c;
            sum2 += c * c;
            let d = vec2<f32>(f32(x), f32(y)) - taa.jitter.xy;
            let w = exp(-2.29 * dot(d, d));
            here += rgb * w;
            weights += w;
        }
    }
    here /= max(weights, 1e-6);
    let surface = reproject(p);
    let was = surface.was;
    let moved = length(was - (vec2<f32>(p) + 0.5));
    // A still pixel's box reaches wider, so the jitter's shifting
    // neighborhood does not pull its history back and forth.
    let gamma = mix(taa.sharpen.z, taa.sharpen.y, clamp(moved, 0.0, 1.0));
    let mean = sum / 9.0;
    let spread = sqrt(max(sum2 / 9.0 - mean * mean, vec3<f32>(0.0)));
    lo = max(lo, mean - spread * gamma);
    hi = min(hi, mean + spread * gamma);
    let off = any(was < vec2<f32>(0.0)) || any(was >= taa.size.xy);
    if taa.blend.w > 0.5 || off {
        return Resolved(vec4<f32>(here, 1.0), surface.w);
    }
    let old = max(history_at(was), vec3<f32>(0.0));
    var clipped = ycocg_to_rgb(clamp(rgb_to_ycocg(old), lo, hi));
    // Fast motion trusts the current frame more, so fast debris never
    // trails more than about a frame.
    var alpha = mix(taa.blend.x, taa.blend.y, clamp(moved / taa.blend.z, 0.0, 1.0));
    // Where the history holds another surface, a chunk that moved on its
    // own or what it uncovered, it is not this pixel's past: trust the
    // current frame, held to its tightest box.
    if depth_mismatch(was, surface.was_w) > taa.sharpen.w {
        let tight = ycocg_to_rgb(clamp(rgb_to_ycocg(old), max(lo, mean - spread * 0.75), min(hi, mean + spread * 0.75)));
        clipped = tight;
        alpha = max(alpha, taa.blend.y);
    }
    let w_here = alpha / (1.0 + luma(here));
    let w_old = (1.0 - alpha) / (1.0 + luma(clipped));
    let out = (here * w_here + clipped * w_old) / max(w_here + w_old, 1e-6);
    return Resolved(vec4<f32>(out, 1.0), surface.w);
}

@fragment
fn fs_sharpen(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(frag.xy);
    let c = load(history, p);
    let n = load(history, p + vec2<i32>(0, -1));
    let s = load(history, p + vec2<i32>(0, 1));
    let e = load(history, p + vec2<i32>(1, 0));
    let w = load(history, p + vec2<i32>(-1, 0));
    let lo = min(c, min(min(n, s), min(e, w)));
    let hi = max(c, max(max(n, s), max(e, w)));
    let k = taa.sharpen.x;
    let sharp = c * (1.0 + 4.0 * k) - (n + s + e + w) * k;
    return vec4<f32>(clamp(sharp, lo, hi), 1.0);
}
