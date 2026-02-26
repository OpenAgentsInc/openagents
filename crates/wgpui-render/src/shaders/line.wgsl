// Line rendering shader with anti-aliasing
// Each line instance is defined by start/end points, rendered as a quad with SDF edges

struct Uniforms {
    viewport: vec2<f32>,
    scale: f32,
    _padding: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct InstanceInput {
    @location(0) start: vec2<f32>,
    @location(1) end: vec2<f32>,
    @location(2) width: f32,
    @location(3) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,    // Position within line quad
    @location(1) line_length: f32,
    @location(2) line_width: f32,
    @location(3) color: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_idx: u32,
    instance: InstanceInput
) -> VertexOutput {
    var out: VertexOutput;

    // Calculate line direction and perpendicular
    let dir = instance.end - instance.start;
    let line_len = length(dir);
    let unit_dir = dir / max(line_len, 0.001);
    let perp = vec2<f32>(-unit_dir.y, unit_dir.x);

    // Expand line by half width on each side, plus a pixel for anti-aliasing
    let half_width = instance.width * 0.5 + 1.0;

    // 4 vertices forming a quad around the line (triangle strip order)
    // 0: start - perp, 1: start + perp, 2: end - perp, 3: end + perp
    let vertex_positions = array<vec2<f32>, 4>(
        instance.start - perp * half_width,
        instance.start + perp * half_width,
        instance.end - perp * half_width,
        instance.end + perp * half_width,
    );

    // Local coordinates for SDF calculation
    let local_coords = array<vec2<f32>, 4>(
        vec2<f32>(0.0, -half_width),
        vec2<f32>(0.0, half_width),
        vec2<f32>(line_len, -half_width),
        vec2<f32>(line_len, half_width),
    );

    let world_pos = vertex_positions[vertex_idx];

    let ndc = vec2<f32>(
        (world_pos.x / uniforms.viewport.x) * 2.0 - 1.0,
        1.0 - (world_pos.y / uniforms.viewport.y) * 2.0
    );

    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.local_pos = local_coords[vertex_idx];
    out.line_length = line_len;
    out.line_width = instance.width;
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // SDF for a line segment (capsule shape)
    let half_width = in.line_width * 0.5;

    // Distance from the line center
    let dist_from_center = abs(in.local_pos.y);

    // Distance to the line endpoints (for rounded caps)
    let dist_to_start = length(in.local_pos);
    let dist_to_end = length(vec2<f32>(in.local_pos.x - in.line_length, in.local_pos.y));

    // SDF: negative inside, positive outside
    var sdf: f32;
    if in.local_pos.x < 0.0 {
        sdf = dist_to_start - half_width;
    } else if in.local_pos.x > in.line_length {
        sdf = dist_to_end - half_width;
    } else {
        sdf = dist_from_center - half_width;
    }

    // Anti-aliasing: smooth transition at edge
    let alpha = 1.0 - smoothstep(-1.0, 1.0, sdf);

    if alpha < 0.001 {
        discard;
    }

    // Use the input color with SDF-based alpha
    let final_alpha = in.color.a * alpha;
    return vec4<f32>(in.color.rgb * final_alpha, final_alpha);
}
