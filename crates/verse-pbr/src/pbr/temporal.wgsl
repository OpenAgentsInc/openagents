// Temporal reprojection and neighborhood conditioning, independently
// implemented from the public temporal supersampling literature.
struct Camera {
    current: mat4x4<f32>,
    inverse: mat4x4<f32>,
    previous: mat4x4<f32>,
    size: vec4<f32>,
    settings: vec4<f32>,
};
// DEPTH_TYPE
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var history: texture_2d<f32>;
@group(0) @binding(3) var scene_depth: DepthTexture;
@group(0) @binding(4) var motion: texture_2d<f32>;
@group(0) @binding(5) var linear_clamp: sampler;

@vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}
fn depth_at(p: vec2<i32>) -> f32 { // DEPTH_LOAD
}
fn bounded(p: vec2<i32>) -> vec2<i32> {
    return clamp(p, vec2<i32>(0), vec2<i32>(camera.size.xy) - vec2<i32>(1));
}
fn ycocg(rgb: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(rgb, vec3<f32>(0.25, 0.5, 0.25)), 0.5 * (rgb.r - rgb.b), 0.5 * rgb.g - 0.25 * (rgb.r + rgb.b));
}
fn rgb(value: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(value.x + value.y - value.z, value.x + value.z, value.x - value.y - value.z);
}

@fragment fn fs(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pixel.xy);
    let uv = pixel.xy * camera.size.zw;
    let current = max(textureLoad(scene, p, 0).rgb, vec3<f32>(0.0));
    let d = depth_at(p);
    let h = camera.inverse * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    let divisor = select(max(h.w, 1e-7), min(h.w, -1e-7), h.w < 0.0);
    let world = h / divisor;
    let previous = camera.previous * vec4<f32>(world.xyz, 1.0);
    let current_clip = camera.current * vec4<f32>(world.xyz, 1.0);
    let view_depth = min(abs(current_clip.w), 60000.0);
    var old_uv = vec2<f32>(previous.x * 0.5 / max(previous.w, 1e-7) + 0.5, 0.5 - previous.y * 0.5 / max(previous.w, 1e-7));
    var old_depth = min(abs(previous.w), 60000.0);
    let object = textureLoad(motion, p, 0);
    if object.w > 0.5 {
        old_uv = uv + object.xy;
        old_depth = object.z;
    }
    // Clear depth represents the sky. Reproject its direction with the
    // camera and keep the history bounded by the current sky neighborhood.
    var weight = camera.settings.x * camera.settings.y;
    if (object.w <= 0.5 && (abs(h.w) < 1e-7 || previous.w <= 1e-6 || !all(abs(previous.xyz) < vec3<f32>(1e20)))) || any(old_uv < vec2<f32>(0.0)) || any(old_uv > vec2<f32>(1.0)) {
        weight = 0.0;
    }
    let sampled = textureSampleLevel(history, linear_clamp, clamp(old_uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0);
    let old_p = bounded(vec2<i32>(old_uv * camera.size.xy));
    let retained_depth = textureLoad(history, old_p, 0).a;
    if !(retained_depth > 0.0) || abs(retained_depth - old_depth) > max(0.04, old_depth * 0.005) {
        weight = 0.0;
    }
    var lower = ycocg(current);
    var upper = lower;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let sample = ycocg(max(textureLoad(scene, bounded(p + vec2<i32>(x, y)), 0).rgb, vec3<f32>(0.0)));
            lower = min(lower, sample);
            upper = max(upper, sample);
        }
    }
    let conditioned = max(rgb(clamp(ycocg(sampled.rgb), lower, upper)), vec3<f32>(0.0));
    let speed = length((old_uv - uv) * camera.size.xy);
    // Fast debris keeps at most one frame's small contribution. Changes in
    // fire, flashes, and smoke also shorten history through luminance change.
    if object.w > 0.5 {
        weight = min(weight, mix(0.9, 0.1, clamp((speed - 0.5) / 3.5, 0.0, 1.0)));
    }
    // An edge's existing contrast is useful history. Only change beyond
    // that neighborhood's range marks a new flash or transparent effect.
    let luma_change = max(abs(ycocg(current).x - ycocg(sampled.rgb).x) - (upper.x - lower.x), 0.0) / max(ycocg(current).x, 0.05);
    weight *= 1.0 - clamp(luma_change * 1.5, 0.0, 0.9);
    return vec4<f32>(mix(current, conditioned, weight), view_depth);
}
