// Quad shader with SDF for rounded corners and borders

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
    @location(5) corner_radii: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_width: f32,
    @location(5) corner_radii: vec4<f32>,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Generate quad vertices (triangle strip: 0, 1, 2, 3)
    let vertex_positions = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),  // top-left
        vec2<f32>(1.0, 0.0),  // top-right
        vec2<f32>(0.0, 1.0),  // bottom-left
        vec2<f32>(1.0, 1.0),  // bottom-right
    );

    let local = vertex_positions[vertex.vertex_idx];
    let world_pos = instance.origin + local * instance.size;

    // Convert to NDC (normalized device coordinates)
    // Flip Y axis for WebGPU coordinate system
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
    out.corner_radii = instance.corner_radii;

    return out;
}

// Signed distance function for a rounded rectangle
fn sdf_rounded_rect(p: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    // Select corner radius based on quadrant
    var r: f32;
    if p.x < size.x * 0.5 {
        if p.y < size.y * 0.5 {
            r = radii.x; // top-left
        } else {
            r = radii.w; // bottom-left
        }
    } else {
        if p.y < size.y * 0.5 {
            r = radii.y; // top-right
        } else {
            r = radii.z; // bottom-right
        }
    }

    // Clamp radius to half of smaller dimension
    r = min(r, min(size.x, size.y) * 0.5);

    // Calculate distance from center of quad
    let half_size = size * 0.5;
    let center = half_size;
    let q = abs(p - center) - half_size + vec2<f32>(r);

    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = sdf_rounded_rect(in.local_pos, in.size, in.corner_radii);

    // Anti-aliasing
    let aa_width = 1.0;
    let outer_alpha = 1.0 - smoothstep(-aa_width, 0.0, d);

    // Border
    let inner_d = d + in.border_width;
    let border_alpha = smoothstep(-aa_width, 0.0, inner_d) * outer_alpha;
    let fill_alpha = (1.0 - smoothstep(-aa_width, 0.0, inner_d)) * outer_alpha;

    // Premultiplied alpha blending - must premultiply RGB by color's alpha AND coverage
    let fill_color = vec4<f32>(
        in.background.rgb * in.background.a * fill_alpha,
        in.background.a * fill_alpha
    );
    let border_color = vec4<f32>(
        in.border_color.rgb * in.border_color.a * border_alpha,
        in.border_color.a * border_alpha
    );

    var color = fill_color + border_color * (1.0 - fill_color.a);

    if color.a < 0.001 {
        discard;
    }

    return color;
}
