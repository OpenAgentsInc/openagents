// Image/SVG rendering shader
// Renders textured quads with RGBA textures

struct Uniforms {
    viewport: vec2<f32>,
    scale: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var image_texture: texture_2d<f32>;

@group(1) @binding(1)
var image_sampler: sampler;

struct VertexInput {
    @builtin(vertex_index) vertex_idx: u32,
}

struct InstanceInput {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) tint: vec4<f32>,  // Optional tint color (use white for no tint)
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    let vertex_positions = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );

    let local = vertex_positions[vertex.vertex_idx];
    let world_pos = instance.position + local * instance.size;

    let ndc = vec2<f32>(
        (world_pos.x / uniforms.viewport.x) * 2.0 - 1.0,
        1.0 - (world_pos.y / uniforms.viewport.y) * 2.0
    );

    out.position = vec4<f32>(ndc, 0.0, 1.0);

    // Map UV coordinates
    let uv_min = instance.uv.xy;
    let uv_max = instance.uv.zw;
    out.uv = mix(uv_min, uv_max, local);

    out.tint = instance.tint;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(image_texture, image_sampler, in.uv);

    // Skip fully transparent pixels
    if texel.a < 0.01 {
        discard;
    }

    // Apply tint: multiply RGB by tint color, preserve alpha
    // For no tint, pass white (1,1,1,1)
    let tinted = vec4<f32>(
        texel.rgb * in.tint.rgb,
        texel.a * in.tint.a
    );

    return tinted;
}
