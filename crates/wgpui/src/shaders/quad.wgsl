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
    @location(5) corner_radius: f32,
    @location(6) clip_origin: vec2<f32>,
    @location(7) clip_size: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_width: f32,
    @location(5) corner_radius: f32,
    @location(6) world_pos: vec2<f32>,
    @location(7) clip_origin: vec2<f32>,
    @location(8) clip_size: vec2<f32>,
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
    out.corner_radius = instance.corner_radius;
    out.world_pos = world_pos;
    out.clip_origin = instance.clip_origin;
    out.clip_size = instance.clip_size;

    return out;
}

// Signed distance function for a rounded rectangle
fn rounded_box_sdf(p: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    // p is relative to center, size is half-extents
    let q = abs(p) - size + radius;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
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

    // Convert local_pos to centered coordinates (center of quad = 0,0)
    let half_size = in.size * 0.5;
    let centered_pos = in.local_pos - half_size;

    // Clamp corner radius to reasonable value
    let max_radius = min(half_size.x, half_size.y);
    let radius = min(in.corner_radius, max_radius);

    // Calculate SDF for the rounded rectangle
    let sdf = rounded_box_sdf(centered_pos, half_size, radius);

    // Anti-aliasing: smooth transition at edge
    let aa_width = 1.0; // 1 pixel anti-aliasing
    let alpha = 1.0 - smoothstep(-aa_width, aa_width, sdf);

    if alpha < 0.001 {
        discard;
    }

    // Determine if we're in the border region
    var color: vec4<f32>;
    if in.border_width > 0.0 {
        // Calculate inner rounded rect for border
        let inner_half_size = half_size - in.border_width;
        let inner_radius = max(0.0, radius - in.border_width);
        let inner_sdf = rounded_box_sdf(centered_pos, inner_half_size, inner_radius);

        // Smooth border transition
        let border_alpha = smoothstep(-aa_width, aa_width, inner_sdf);

        // Mix border and background colors
        let bg_color = vec4<f32>(in.background.rgb * in.background.a, in.background.a);
        let bd_color = vec4<f32>(in.border_color.rgb * in.border_color.a, in.border_color.a);
        color = mix(bg_color, bd_color, border_alpha);
    } else {
        color = vec4<f32>(in.background.rgb * in.background.a, in.background.a);
    }

    // Apply outer edge anti-aliasing
    color = color * alpha;

    if color.a < 0.001 {
        discard;
    }

    return color;
}
