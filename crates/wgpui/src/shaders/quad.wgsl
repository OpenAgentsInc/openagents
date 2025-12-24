struct Uniforms {
    viewport: vec2<f32>,
    scale: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @builtin(vertex_index) vertex_idx: u32,
    @builtin(instance_index) instance_idx: u32,
}

struct InstanceInput {
    @location(0) origin: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_width: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_width: f32,
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
    let world_pos = instance.origin + local * instance.size;

    let ndc = vec2<f32>(
        (world_pos.x / uniforms.viewport.x) * 2.0 - 1.0,
        1.0 - (world_pos.y / uniforms.viewport.y) * 2.0
    );

    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.local_pos = local * instance.size;
    out.size = instance.size;
    out.background = instance.background;
    out.border_color = instance.border_color;
    out.border_width = instance.border_width;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.local_pos;
    let half_border = in.border_width * 0.5;

    let in_border_x = p.x < in.border_width || p.x > in.size.x - in.border_width;
    let in_border_y = p.y < in.border_width || p.y > in.size.y - in.border_width;
    let in_border = in_border_x || in_border_y;

    var color: vec4<f32>;
    if in_border && in.border_width > 0.0 {
        color = vec4<f32>(
            in.border_color.rgb * in.border_color.a,
            in.border_color.a
        );
    } else {
        color = vec4<f32>(
            in.background.rgb * in.background.a,
            in.background.a
        );
    }

    if color.a < 0.001 {
        discard;
    }

    return color;
}
