// High-tier screen-space detail (`verse_engine::quality`, `pbr::screen`):
// ambient occlusion and the sun's contact shadow, traced at half resolution
// through the single-sample depth prepass, then blurred back to full
// resolution with depth-aware weights. The scene's lit and textured shaders
// read the result: red scales ambient light only, green the sun's direct
// light only.
//
// The occlusion is a two-direction GTAO after Jimenez, Wu, Pesce, and Jarabo,
// "Practical Real-Time Strategies for Accurate Indirect Occlusion" (2016).
// The contact shadow is a short ray march toward the light through the depth
// buffer, after Bend Studio's "Screen Space Shadows" (2023).
//
// `pbr::screen`'s tests hold a CPU form of `fs_trace`; keep the two in step.

struct Screen {
    // Reversed-depth world to clip space, and its inverse.
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    // xyz toward the light that casts contact shadows; w 1 when one does.
    light: vec4<f32>,
    // Full-resolution width and height, and their reciprocals.
    size: vec4<f32>,
    // x radius, m; y largest screen radius, full-resolution pixels; z the
    // fraction of the radius over which an occluder fades out.
    ao: vec4<f32>,
    // x ray length, m; y occluder thickness, m; z start offset along the
    // normal, m.
    contact: vec4<f32>,
};

@group(0) @binding(0) var<uniform> screen: Screen;
@group(0) @binding(1) var scene_depth: texture_depth_2d;
// The half-resolution trace, for the resolve pass.
@group(0) @binding(2) var traced: texture_2d<f32>;

const HALF_PI: f32 = 1.5707963;
const AO_STEPS: i32 = 6;
const CONTACT_STEPS: i32 = 12;

@vertex
fn vs_fullscreen(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

// Interleaved gradient noise (Jimenez 2014).
fn ign(p: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(p, vec2<f32>(0.06711056, 0.00583715))));
}

fn depth_size() -> vec2<i32> {
    return vec2<i32>(textureDimensions(scene_depth));
}

fn load_depth(p: vec2<i32>) -> f32 {
    return textureLoad(scene_depth, clamp(p, vec2<i32>(0), depth_size() - 1), 0);
}

// The world point at the center of full-resolution pixel `p` (fractional
// pixels allowed) and reversed depth `d`.
fn world_at(p: vec2<f32>, d: f32) -> vec3<f32> {
    let uv = (p + 0.5) * screen.size.zw;
    let w = screen.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    return w.xyz / w.w;
}

// A world point's full-resolution pixel coordinates, whose integer parts
// name its pixel, and its reversed depth. Points behind the eye land off the
// screen.
fn project(world: vec3<f32>) -> vec3<f32> {
    let c = screen.view_proj * vec4<f32>(world, 1.0);
    if c.w <= 1e-6 {
        return vec3<f32>(-1.0, -1.0, 0.0);
    }
    let ndc = c.xyz / c.w;
    return vec3<f32>((ndc.x * 0.5 + 0.5) * screen.size.x, (0.5 - ndc.y * 0.5) * screen.size.y, ndc.z);
}

// From `center` to the surface at pixel `p + offset`; far away when that
// pixel is off the screen or shows the sky, so the other side is chosen.
fn toward(p: vec2<i32>, offset: vec2<i32>, center: vec3<f32>) -> vec3<f32> {
    let q = p + offset;
    if any(q < vec2<i32>(0)) || any(q >= depth_size()) {
        return vec3<f32>(1e9);
    }
    let d = textureLoad(scene_depth, q, 0);
    if d <= 0.0 {
        return vec3<f32>(1e9);
    }
    return world_at(vec2<f32>(q), d) - center;
}

// The surface normal from depth, facing the eye: on each axis the nearer
// neighbor wins, so silhouettes do not bend it.
fn normal_at(p: vec2<i32>, center: vec3<f32>) -> vec3<f32> {
    let right = toward(p, vec2<i32>(1, 0), center);
    let left = -toward(p, vec2<i32>(-1, 0), center);
    let down = toward(p, vec2<i32>(0, 1), center);
    let up = -toward(p, vec2<i32>(0, -1), center);
    let dx = select(left, right, dot(right, right) <= dot(left, left));
    let dy = select(up, down, dot(down, down) <= dot(up, up));
    let to_eye = screen.eye.xyz - center;
    let n = cross(dy, dx);
    if dot(n, n) < 1e-20 {
        return normalize(to_eye);
    }
    let unit = normalize(n);
    return select(-unit, unit, dot(unit, to_eye) >= 0.0);
}

// The cosine-weighted visible arc from the view direction to horizon angle
// `h` in a slice whose projected normal sits at angle `n`.
fn arc(h: f32, n: f32) -> f32 {
    return 0.25 * (cos(n) + 2.0 * h * sin(n) - cos(2.0 * h - n));
}

// GTAO over two screen directions at a rotation `noise`: in each slice, the
// highest horizon on both sides within the radius, integrated against the
// cosine. The result is the visible over the unoccluded integral, so an open
// plane is exactly 1.
fn ambient_occlusion(p: vec2<i32>, d: f32, center: vec3<f32>, n: vec3<f32>, noise: f32) -> f32 {
    let v = normalize(screen.eye.xyz - center);
    let side_axis = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(v.y) > 0.9);
    let across = normalize(cross(v, side_axis));
    let radius_px = min(distance(project(center + across * screen.ao.x).xy, project(center).xy), screen.ao.y);
    if radius_px < 1.0 {
        return 1.0;
    }
    let stride = radius_px / f32(AO_STEPS);
    var seen = 0.0;
    var open = 0.0;
    for (var slice_index = 0; slice_index < 2; slice_index++) {
        let phi = (f32(slice_index) + noise) * HALF_PI;
        let dir = vec2<f32>(cos(phi), sin(phi));
        var along = world_at(vec2<f32>(p) + dir * 4.0, d) - center;
        along = normalize(along - v * dot(along, v));
        let axis = normalize(cross(along, v));
        let projected = n - axis * dot(n, axis);
        let len = length(projected);
        if len < 1e-4 {
            continue;
        }
        let cos_n = clamp(dot(projected, v) / len, -1.0, 1.0);
        let angle = select(-1.0, 1.0, dot(along, projected) >= 0.0) * acos(cos_n);
        var ahead = 0.0;
        var behind = 0.0;
        for (var side = 0; side < 2; side++) {
            let heading = 1.0 - 2.0 * f32(side);
            let low = cos(angle + heading * HALF_PI);
            var horizon = low;
            for (var k = 0; k < AO_STEPS; k++) {
                let t = 1.0 + (f32(k) + noise) * stride;
                let q = floor(vec2<f32>(p) + 0.5 + dir * heading * t);
                if any(q < vec2<f32>(0.0)) || any(q >= screen.size.xy) {
                    break;
                }
                let qd = load_depth(vec2<i32>(q));
                if qd <= 0.0 {
                    continue;
                }
                let delta = world_at(q, qd) - center;
                let dist = length(delta);
                if dist < 1e-4 {
                    continue;
                }
                let weight = clamp((screen.ao.x - dist) / (screen.ao.x * screen.ao.z), 0.0, 1.0);
                horizon = max(horizon, mix(low, dot(delta, v) / dist, weight));
            }
            if side == 0 {
                ahead = horizon;
            } else {
                behind = horizon;
            }
        }
        let h1 = angle + min(acos(clamp(ahead, -1.0, 1.0)) - angle, HALF_PI);
        let h0 = angle + max(-acos(clamp(behind, -1.0, 1.0)) - angle, -HALF_PI);
        seen += len * (arc(h0, angle) + arc(h1, angle));
        open += len * (arc(angle - HALF_PI, angle) + arc(angle + HALF_PI, angle));
    }
    if open <= 1e-6 {
        return 1.0;
    }
    return clamp(seen / open, 0.0, 1.0);
}

// The eye's distance to the surface at pixel `q`, or `fallback` off the
// screen or on the sky.
fn range_at(q: vec2<i32>, fallback: f32) -> f32 {
    if any(q < vec2<i32>(0)) || any(q >= depth_size()) {
        return fallback;
    }
    let d = textureLoad(scene_depth, q, 0);
    if d <= 0.0 {
        return fallback;
    }
    return distance(world_at(vec2<f32>(q), d), screen.eye.xyz);
}

// The light's visibility past small occluders the shadow map misses: march
// toward the light, and stop at the first sample just behind the depth
// buffer. A sample counts only when it is deeper than the surface's own
// depth step across one pixel, so a grazing floor does not shadow itself,
// and shallower than the occluder thickness, so a sample passing far behind
// an object is not shadowed by it. Hits near the ray's end fade out.
fn contact_shadow(center: vec3<f32>, n: vec3<f32>, noise: f32) -> f32 {
    if screen.light.w < 0.5 {
        return 1.0;
    }
    let l = normalize(screen.light.xyz);
    if dot(n, l) <= 0.0 {
        return 1.0;
    }
    let start = center + n * screen.contact.z;
    for (var k = 0; k < CONTACT_STEPS; k++) {
        let t = (f32(k) + noise) / f32(CONTACT_STEPS) * screen.contact.x;
        let marched = start + l * t;
        let q = project(marched);
        if q.x < 0.0 || q.y < 0.0 || q.x >= screen.size.x || q.y >= screen.size.y || q.z <= 0.0 {
            break;
        }
        let qi = vec2<i32>(floor(q.xy));
        let qd = load_depth(qi);
        if qd <= q.z {
            continue;
        }
        let r0 = distance(world_at(vec2<f32>(qi), qd), screen.eye.xyz);
        let gap = distance(marched, screen.eye.xyz) - r0;
        let slope_x = min(abs(range_at(qi + vec2<i32>(1, 0), r0) - r0), abs(range_at(qi - vec2<i32>(1, 0), r0) - r0));
        let slope_y = min(abs(range_at(qi + vec2<i32>(0, 1), r0) - r0), abs(range_at(qi - vec2<i32>(0, 1), r0) - r0));
        if gap > max(max(slope_x, slope_y), 0.001) && gap < screen.contact.y {
            return clamp((t / screen.contact.x - 0.75) * 4.0, 0.0, 1.0);
        }
    }
    return 1.0;
}

// Half resolution: red the ambient occlusion, green the contact shadow, at
// the top-left full-resolution pixel of each 2×2 block.
@fragment
fn fs_trace(@builtin(position) clip: vec4<f32>) -> @location(0) vec4<f32> {
    let cell = vec2<i32>(clip.xy);
    let p = cell * 2;
    let d = load_depth(p);
    if d <= 0.0 {
        return vec4<f32>(1.0);
    }
    let center = world_at(vec2<f32>(p), d);
    let n = normal_at(p, center);
    let noise = ign(vec2<f32>(cell));
    let ao = ambient_occlusion(p, d, center, n, noise);
    let shadow = contact_shadow(center, n, fract(noise + 0.618034));
    return vec4<f32>(ao, shadow, 0.0, 1.0);
}

// Full resolution: a 5×5 blur of the half-resolution trace whose taps weigh
// by their distance from this pixel's surface plane, so occlusion does not
// leak across silhouettes, and which averages the trace's noise away.
@fragment
fn fs_resolve(@builtin(position) clip: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(clip.xy);
    let d = load_depth(p);
    if d <= 0.0 {
        return vec4<f32>(1.0);
    }
    let center = world_at(vec2<f32>(p), d);
    let n = normal_at(p, center);
    let tolerance = 0.02 * distance(center, screen.eye.xyz) + 0.01;
    let low_size = vec2<i32>(textureDimensions(traced));
    let anchor = p / 2;
    var sum = vec2<f32>(0.0);
    var total = 0.0;
    for (var y = -2; y <= 2; y++) {
        for (var x = -2; x <= 2; x++) {
            let cell = clamp(anchor + vec2<i32>(x, y), vec2<i32>(0), low_size - 1);
            let cd = load_depth(cell * 2);
            if cd <= 0.0 {
                continue;
            }
            let plane = abs(dot(world_at(vec2<f32>(cell * 2), cd) - center, n));
            let weight = exp(-f32(x * x + y * y) * 0.25 - plane / tolerance);
            sum += textureLoad(traced, cell, 0).rg * weight;
            total += weight;
        }
    }
    if total < 1e-4 {
        return vec4<f32>(textureLoad(traced, clamp(anchor, vec2<i32>(0), low_size - 1), 0).rg, 0.0, 1.0);
    }
    return vec4<f32>(sum / total, 0.0, 1.0);
}
