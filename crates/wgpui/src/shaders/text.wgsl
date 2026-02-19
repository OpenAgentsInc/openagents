struct Uniforms {
    viewport: vec2<f32>,
    scale: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var glyph_atlas: texture_2d<f32>;

@group(1) @binding(1)
var atlas_sampler: sampler;

struct VertexInput {
    @builtin(vertex_index) vertex_idx: u32,
}

struct InstanceInput {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) uv: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) clip_origin: vec2<f32>,
    @location(5) clip_size: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world_pos: vec2<f32>,
    @location(3) clip_origin: vec2<f32>,
    @location(4) clip_size: vec2<f32>,
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

    let uv_min = instance.uv.xy;
    let uv_max = instance.uv.zw;
    out.uv = mix(uv_min, uv_max, local);

    out.color = instance.color;
    out.world_pos = world_pos;
    out.clip_origin = instance.clip_origin;
    out.clip_size = instance.clip_size;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if in.clip_size.x >= 0.0 && in.clip_size.y >= 0.0 {
        if in.world_pos.x < in.clip_origin.x
            || in.world_pos.y < in.clip_origin.y
            || in.world_pos.x > in.clip_origin.x + in.clip_size.x
            || in.world_pos.y > in.clip_origin.y + in.clip_size.y
        {
            discard;
        }
    }

    let alpha = textureSample(glyph_atlas, atlas_sampler, in.uv).r;

    if alpha < 0.01 {
        discard;
    }

    return vec4<f32>(in.color.rgb * alpha, alpha * in.color.a);
}
