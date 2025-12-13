//! WGSL shaders for wgpu renderer

/// Quad shader - renders filled rectangles with optional rounded corners
pub const QUAD_SHADER: &str = r#"
struct GlobalParams {
    viewport_size: vec2<f32>,
    _pad: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> globals: GlobalParams;

struct QuadInstance {
    @location(0) origin: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_widths: vec4<f32>,
    @location(5) corner_radii: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) background: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) border_widths: vec4<f32>,
    @location(5) corner_radii: vec4<f32>,
}

// Convert HSLA to RGBA
fn hsla_to_rgba(hsla: vec4<f32>) -> vec4<f32> {
    let h = hsla.x * 6.0;
    let s = hsla.y;
    let l = hsla.z;
    let a = hsla.w;

    let c = (1.0 - abs(2.0 * l - 1.0)) * s;
    let x = c * (1.0 - abs(h % 2.0 - 1.0));
    let m = l - c / 2.0;

    var rgb: vec3<f32>;
    if (h < 1.0) {
        rgb = vec3<f32>(c, x, 0.0);
    } else if (h < 2.0) {
        rgb = vec3<f32>(x, c, 0.0);
    } else if (h < 3.0) {
        rgb = vec3<f32>(0.0, c, x);
    } else if (h < 4.0) {
        rgb = vec3<f32>(0.0, x, c);
    } else if (h < 5.0) {
        rgb = vec3<f32>(x, 0.0, c);
    } else {
        rgb = vec3<f32>(c, 0.0, x);
    }

    return vec4<f32>(rgb + m, a);
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_id: u32,
    instance: QuadInstance,
) -> VertexOutput {
    // Generate quad vertices (triangle strip: 0,1,2,3)
    let x = f32(vertex_id & 1u);
    let y = f32((vertex_id >> 1u) & 1u);
    let unit_pos = vec2<f32>(x, y);

    // Calculate world position
    let world_pos = instance.origin + unit_pos * instance.size;

    // Convert to NDC (normalized device coordinates)
    let ndc = (world_pos / globals.viewport_size) * 2.0 - 1.0;
    // Flip Y for WebGPU coordinate system
    let position = vec4<f32>(ndc.x, -ndc.y, 0.0, 1.0);

    var output: VertexOutput;
    output.position = position;
    output.local_pos = unit_pos * instance.size;
    output.size = instance.size;
    output.background = instance.background;
    output.border_color = instance.border_color;
    output.border_widths = instance.border_widths;
    output.corner_radii = instance.corner_radii;
    return output;
}

// Signed distance to a rounded box
fn rounded_box_sdf(pos: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let half_size = size * 0.5;
    let center_pos = pos - half_size;
    let q = abs(center_pos) - half_size + radius;
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

// Pick corner radius based on position
fn pick_corner_radius(pos: vec2<f32>, size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let half = size * 0.5;
    let center = pos - half;

    if (center.x < 0.0) {
        if (center.y < 0.0) {
            return radii.x; // top_left
        } else {
            return radii.w; // bottom_left
        }
    } else {
        if (center.y < 0.0) {
            return radii.y; // top_right
        } else {
            return radii.z; // bottom_right
        }
    }
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Convert HSLA to RGBA
    let bg_color = hsla_to_rgba(input.background);
    let border_color = hsla_to_rgba(input.border_color);

    // Get corner radius for this position
    let radius = pick_corner_radius(input.local_pos, input.size, input.corner_radii);

    // Calculate SDF for outer edge
    let outer_dist = rounded_box_sdf(input.local_pos, input.size, radius);

    // Calculate SDF for inner edge (for border)
    let border_width = max(max(input.border_widths.x, input.border_widths.y),
                          max(input.border_widths.z, input.border_widths.w));
    let inner_size = input.size - vec2<f32>(border_width * 2.0);
    let inner_pos = input.local_pos - vec2<f32>(border_width);
    let inner_radius = max(0.0, radius - border_width);
    let inner_dist = rounded_box_sdf(inner_pos, inner_size, inner_radius);

    // Anti-aliasing
    let aa = 1.0;

    // Outside the quad entirely
    if (outer_dist > aa) {
        discard;
    }

    // Calculate alpha for anti-aliasing at edges
    let outer_alpha = 1.0 - smoothstep(-aa, aa, outer_dist);

    // Mix border and background
    var color: vec4<f32>;
    if (border_width > 0.0 && inner_dist > -aa) {
        // In border region
        let border_alpha = 1.0 - smoothstep(-aa, aa, inner_dist);
        color = mix(bg_color, border_color, border_alpha);
    } else {
        // In background region
        color = bg_color;
    }

    // Apply outer edge anti-aliasing
    color.a *= outer_alpha;

    // Premultiply alpha
    return vec4<f32>(color.rgb * color.a, color.a);
}
"#;
