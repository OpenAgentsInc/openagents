// Match the opaque lit vertex projection and visible depth samples exactly.
struct Camera {
    current: mat4x4<f32>, inverse: mat4x4<f32>, previous: mat4x4<f32>,
    size: vec4<f32>, settings: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: Camera;
@vertex fn vs(@location(0) position: vec3<f32>) -> @builtin(position) @invariant vec4<f32> {
    return camera.current * vec4<f32>(position, 1.0);
}
@fragment fn fs() -> @location(0) vec2<f32> { return vec2<f32>(1.0, 0.0); }
