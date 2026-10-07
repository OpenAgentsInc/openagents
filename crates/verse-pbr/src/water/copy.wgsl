// The water's depth copy (`verse_pbr::water::screen`): a full-screen
// triangle that writes each pixel's view depth (its clip w, m) from the
// scene's reversed depth buffer, read at its first sample, or 60000
// (`screen::FAR`) where nothing was drawn. The physical renderer's frame
// uniform starts with the two matrices it reads.

// VERSE_DEPTH_TEXTURE

struct CopyFrame {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> c: CopyFrame;
@group(0) @binding(1) var scene_depth: DepthTexture;

@vertex
fn vs_copy(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u)) * 2.0 - 1.0;
    return vec4<f32>(p, 0.0, 1.0);
}

@fragment
fn fs_copy(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(scene_depth));
    let d = textureLoad(scene_depth, vec2<i32>(pixel.xy), 0);
    if d <= 0.0 {
        return vec4<f32>(60000.0, 0.0, 0.0, 0.0);
    }
    let uv = pixel.xy / size;
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let h = c.inv_view_proj * vec4<f32>(ndc, d, 1.0);
    let world = h.xyz / h.w;
    let m = c.view_proj;
    let w = dot(vec4<f32>(m[0].w, m[1].w, m[2].w, m[3].w), vec4<f32>(world, 1.0));
    return vec4<f32>(min(w, 59000.0), 0.0, 0.0, 0.0);
}
