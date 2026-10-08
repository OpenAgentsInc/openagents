struct Camera {
    current: mat4x4<f32>, inverse: mat4x4<f32>, previous: mat4x4<f32>,
    size: vec4<f32>, settings: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var history: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32((index << 1u) & 2u) * 2.0 - 1.0, f32(index & 2u) * 2.0 - 1.0, 0.0, 1.0);
}
fn load(p: vec2<i32>) -> vec3<f32> {
    return textureLoad(history, clamp(p, vec2<i32>(0), vec2<i32>(camera.size.xy) - vec2<i32>(1)), 0).rgb;
}
@fragment fn fs(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let p = vec2<i32>(pixel.xy);
    let center = load(p);
    let left = load(p + vec2<i32>(-1, 0));
    let right = load(p + vec2<i32>(1, 0));
    let up = load(p + vec2<i32>(0, -1));
    let down = load(p + vec2<i32>(0, 1));
    let lower = min(center, min(min(left, right), min(up, down)));
    let upper = max(center, max(max(left, right), max(up, down)));
    let sharpened = center + (center - (left + right + up + down) * 0.25) * camera.settings.z;
    return vec4<f32>(max(clamp(sharpened, lower, upper), vec3<f32>(0.0)), 1.0);
}
