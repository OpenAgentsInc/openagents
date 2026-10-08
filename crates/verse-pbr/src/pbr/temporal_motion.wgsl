struct Camera {
    current: mat4x4<f32>, inverse: mat4x4<f32>, previous: mat4x4<f32>,
    size: vec4<f32>, settings: vec4<f32>,
};
// DEPTH_TYPE
// MOTION_FOOTPRINT
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var scene_depth: DepthTexture;
struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) current_x: vec4<f32>, @location(2) current_y: vec4<f32>, @location(3) current_z: vec4<f32>,
    @location(4) previous_x: vec4<f32>, @location(5) previous_y: vec4<f32>, @location(6) previous_z: vec4<f32>,
};
struct Fragment {
    @builtin(position) clip: vec4<f32>,
    @location(0) previous: vec4<f32>,
};
@vertex fn vs(vertex: Vertex) -> Fragment {
    let local = vec4<f32>(vertex.position, 1.0);
    let current = vec4<f32>(dot(vertex.current_x, local), dot(vertex.current_y, local), dot(vertex.current_z, local), 1.0);
    let previous = vec4<f32>(dot(vertex.previous_x, local), dot(vertex.previous_y, local), dot(vertex.previous_z, local), 1.0);
    return Fragment(camera.current * current, camera.previous * previous);
}
fn depth_at(p: vec2<i32>) -> f32 { // DEPTH_LOAD
}
@fragment fn fs(fragment: Fragment) -> @location(0) vec4<f32> {
    let scene = depth_at(vec2<i32>(fragment.clip.xy));
    // Only the visible surface writes motion. This rejects objects behind
    // walls and lets the camera path reproject static geometry.
    // Relative float error stays small in reversed depth. Four-sample depth
    // may come from a position within 3/8 of a pixel from this center; a
    // single-sample target has no such footprint. An absolute depth epsilon
    // would admit substantially hidden surfaces at distance.
    let tolerance = max(abs(scene), abs(fragment.clip.z)) * 8e-7 + 1e-10 + fwidth(fragment.clip.z) * MOTION_FOOTPRINT;
    if abs(scene - fragment.clip.z) > tolerance || fragment.previous.w <= 1e-6 {
        discard;
    }
    let current_uv = fragment.clip.xy * camera.size.zw;
    let old_uv = vec2<f32>(fragment.previous.x * 0.5 / fragment.previous.w + 0.5, 0.5 - fragment.previous.y * 0.5 / fragment.previous.w);
    return vec4<f32>(old_uv - current_uv, min(abs(fragment.previous.w), 60000.0), 1.0);
}
