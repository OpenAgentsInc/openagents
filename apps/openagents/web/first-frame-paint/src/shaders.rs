//! GPUI's shader math from zui's `gpui_wgpu/src/shaders.wgsl` (Apache-2.0),
//! ported to GLSL ES 3.0: the quad SDF with rounded corners and borders,
//! the blurred and inset shadow, path rasterization with its curve
//! antialiasing, and glyph coverage with GPUI's contrast and gamma
//! correction. Each function keeps its WGSL name and steps, so the two can
//! be read side by side.
//!
//! GPUI's positions have their origin at the top left; GL's fragment
//! coordinates start at the bottom left, so every fragment shader flips
//! `gl_FragCoord.y` against the viewport first.

/// Shared by every vertex shader: device-pixel position to clip space.
const VS_HEAD: &str = "#version 300 es
precision highp float;
uniform vec2 u_vp;
vec2 unit_vertex() { return vec2(float(gl_VertexID & 1), 0.5 * float(gl_VertexID & 2)); }
vec4 to_device(vec2 p) { return vec4(p / u_vp * vec2(2.0, -2.0) + vec2(-1.0, 1.0), 0.0, 1.0); }
";

/// Shared by every fragment shader.
const FS_HEAD: &str = "#version 300 es
precision highp float;
uniform vec2 u_vp;
out vec4 o;
vec2 frag() { return vec2(gl_FragCoord.x, u_vp.y - gl_FragCoord.y); }
bool clipped(vec2 p, vec4 m) {
    vec4 d = vec4(p.x - m.x, m.x + m.z - p.x, p.y - m.y, m.y + m.w - p.y);
    return any(lessThan(d, vec4(0.0)));
}
// radii: top left, top right, bottom right, bottom left.
float pick_corner_radius(vec2 c, vec4 r) {
    if (c.x < 0.0) { return c.y < 0.0 ? r.x : r.w; }
    return c.y < 0.0 ? r.y : r.z;
}
float quad_sdf_impl(vec2 c, float r) {
    if (r == 0.0) { return max(c.x, c.y); }
    return length(max(vec2(0.0), c)) + min(0.0, max(c.x, c.y)) - r;
}
float quad_sdf(vec2 p, vec4 b, vec4 radii) {
    vec2 half_size = b.zw / 2.0;
    vec2 center_to_point = p - (b.xy + half_size);
    float r = pick_corner_radius(center_to_point, radii);
    return quad_sdf_impl(abs(center_to_point) - half_size + r, r);
}
";

pub fn vs(body: &str) -> String {
    let mut s = String::from(VS_HEAD);
    s.push_str(body);
    s
}

pub fn fs(body: &str) -> String {
    let mut s = String::from(FS_HEAD);
    s.push_str(body);
    s
}

/// Quads: bounds, mask, background, border color, radii, border widths.
pub const QUAD_VS: &str = "
in vec4 a0; in vec4 a1; in vec4 a2; in vec4 a3; in vec4 a4; in vec4 a5;
flat out vec4 v_bounds; flat out vec4 v_mask; flat out vec4 v_bg; flat out vec4 v_border;
flat out vec4 v_radii; flat out vec4 v_widths;
void main() {
    gl_Position = to_device(a0.xy + unit_vertex() * a0.zw);
    v_bounds = a0; v_mask = a1; v_bg = a2; v_border = a3; v_radii = a4; v_widths = a5;
}
";

/// `fs_quad` for solid backgrounds and solid borders.
pub const QUAD_FS: &str = "
flat in vec4 v_bounds; flat in vec4 v_mask; flat in vec4 v_bg; flat in vec4 v_border;
flat in vec4 v_radii; flat in vec4 v_widths;
float quarter_ellipse_sdf(vec2 p, vec2 radii) {
    return (length(p / radii) - 1.0) * (radii.x + radii.y) * -0.5;
}
vec4 over(vec4 below, vec4 above) {
    float alpha = above.a + below.a * (1.0 - above.a);
    return vec4((above.rgb * above.a + below.rgb * below.a * (1.0 - above.a)) / alpha, alpha);
}
void main() {
    vec2 p = frag();
    if (clipped(p, v_mask)) { discard; }
    vec4 bg = v_bg;
    bool unrounded = all(equal(v_radii, vec4(0.0)));
    if (all(equal(v_widths, vec4(0.0))) && unrounded) { o = bg; return; }
    vec2 half_size = v_bounds.zw / 2.0;
    vec2 center_to_point = p - v_bounds.xy - half_size;
    const float aa = 0.5;
    float corner_radius = pick_corner_radius(center_to_point, v_radii);
    // widths: top, right, bottom, left.
    vec2 border = vec2(center_to_point.x < 0.0 ? v_widths.w : v_widths.y,
                       center_to_point.y < 0.0 ? v_widths.x : v_widths.z);
    vec2 reduced = vec2(border.x == 0.0 ? -aa : border.x, border.y == 0.0 ? -aa : border.y);
    vec2 corner_to_point = abs(center_to_point) - half_size;
    vec2 corner_center_to_point = corner_to_point + corner_radius;
    bool near_rounded = corner_center_to_point.x >= 0.0 && corner_center_to_point.y >= 0.0;
    vec2 inner_corner = corner_to_point + reduced;
    bool beyond = inner_corner.x > 0.0 || inner_corner.y > 0.0;
    bool within = inner_corner.x < -aa && inner_corner.y < -aa;
    if (within && !near_rounded) { o = bg; return; }
    float outer_sdf = quad_sdf_impl(corner_center_to_point, corner_radius);
    float inner_sdf;
    if (corner_center_to_point.x <= 0.0 || corner_center_to_point.y <= 0.0) {
        inner_sdf = -max(inner_corner.x, inner_corner.y);
    } else if (beyond) {
        inner_sdf = -1.0;
    } else if (reduced.x == reduced.y) {
        inner_sdf = -(outer_sdf + reduced.x);
    } else {
        inner_sdf = quarter_ellipse_sdf(corner_center_to_point, max(vec2(0.0), corner_radius - reduced));
    }
    vec4 color = bg;
    if (max(inner_sdf, outer_sdf) < aa) {
        color = mix(bg, over(bg, v_border), clamp(aa - inner_sdf, 0.0, 1.0));
    }
    o = vec4(color.rgb, color.a * clamp(aa - outer_sdf, 0.0, 1.0));
}
";

/// Shadows: bounds, radii, mask, color, element bounds, element radii,
/// and (blur, inset).
pub const SHADOW_VS: &str = "
in vec4 a0; in vec4 a1; in vec4 a2; in vec4 a3; in vec4 a4; in vec4 a5; in vec4 a6;
flat out vec4 v_bounds; flat out vec4 v_radii; flat out vec4 v_mask; flat out vec4 v_color;
flat out vec4 v_elem; flat out vec4 v_eradii; flat out vec4 v_misc;
void main() {
    vec4 g = a4;
    if (a6.y == 0.0) { float m = 3.0 * a6.x; g = vec4(a0.xy - m, a0.zw + 2.0 * m); }
    gl_Position = to_device(g.xy + unit_vertex() * g.zw);
    v_bounds = a0; v_radii = a1; v_mask = a2; v_color = a3; v_elem = a4; v_eradii = a5; v_misc = a6;
}
";

/// `fs_shadow`.
pub const SHADOW_FS: &str = "
flat in vec4 v_bounds; flat in vec4 v_radii; flat in vec4 v_mask; flat in vec4 v_color;
flat in vec4 v_elem; flat in vec4 v_eradii; flat in vec4 v_misc;
float gaussian(float x, float sigma) {
    return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * 3.1415926) * sigma);
}
vec2 erf(vec2 v) {
    vec2 s = sign(v); vec2 a = abs(v);
    vec2 r1 = 1.0 + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
    vec2 r2 = r1 * r1;
    return s - s / (r2 * r2);
}
float blur_along_x(float x, float y, float sigma, float corner, vec2 half_size) {
    float delta = min(half_size.y - corner - abs(y), 0.0);
    float curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
    vec2 integral = 0.5 + 0.5 * erf((x + vec2(-curved, curved)) * (sqrt(0.5) / sigma));
    return integral.y - integral.x;
}
void main() {
    vec2 p = frag();
    if (clipped(p, v_mask)) { discard; }
    float blur = v_misc.x;
    vec2 half_size = v_bounds.zw / 2.0;
    vec2 center_to_point = p - (v_bounds.xy + half_size);
    float corner_radius = pick_corner_radius(center_to_point, v_radii);
    float alpha;
    if (blur == 0.0) {
        alpha = clamp(0.5 - quad_sdf(p, v_bounds, v_radii), 0.0, 1.0);
    } else {
        float low = center_to_point.y - half_size.y;
        float high = center_to_point.y + half_size.y;
        float start = clamp(-3.0 * blur, low, high);
        float end = clamp(3.0 * blur, low, high);
        float step = (end - start) / 4.0;
        float y = start + step * 0.5;
        alpha = 0.0;
        for (int i = 0; i < 4; i++) {
            alpha += blur_along_x(center_to_point.x, center_to_point.y - y, blur, corner_radius, half_size)
                * gaussian(y, blur) * step;
            y += step;
        }
    }
    if (v_misc.y != 0.0) {
        alpha = (1.0 - alpha) * clamp(0.5 - quad_sdf(p, v_elem, v_eradii), 0.0, 1.0);
    }
    o = vec4(v_color.rgb, v_color.a * alpha);
}
";

/// Glyphs: bounds (the tile's size), mask, color, tile in atlas pixels.
pub const SPRITE_VS: &str = "
uniform vec2 u_atlas;
in vec4 a0; in vec4 a1; in vec4 a2; in vec4 a3;
out vec2 v_uv; flat out vec4 v_mask; flat out vec4 v_color;
void main() {
    vec2 u = unit_vertex();
    gl_Position = to_device(a0.xy + u * a0.zw);
    v_uv = (a3.xy + u * a3.zw) / u_atlas;
    v_mask = a1; v_color = a2;
}
";

/// `fs_mono_sprite` without blur or edge fades, and `fs_subpixel_sprite`.
/// GPUI blends subpixel glyphs with a second source color, a coverage per
/// channel; here each channel is its own pass (`u_channel` 0 to 2, with
/// the other channels masked), which blends the same. `u_channel` is -1
/// for one-channel glyphs.
pub const SPRITE_FS: &str = "
uniform sampler2D u_tex; uniform vec4 u_gamma; uniform float u_contrast;
uniform float u_subpixel_contrast; uniform int u_channel;
in vec2 v_uv; flat in vec4 v_mask; flat in vec4 v_color;
float color_brightness(vec3 c) { return dot(c, vec3(0.30, 0.59, 0.11)); }
void main() {
    vec2 p = frag();
    vec3 c = v_color.rgb;
    float b = color_brightness(c);
    float light = clamp(4.0 * (0.75 - b), 0.0, 1.0);
    float a;
    if (u_channel < 0) {
        float s = texture(u_tex, v_uv).r;
        float k = u_contrast * light;
        a = s * (k + 1.0) / (s * k + 1.0);
        float correction = (u_gamma.x * b + u_gamma.y) * a + (u_gamma.z * b + u_gamma.w);
        a = a + a * (1.0 - a) * correction;
    } else {
        vec3 s = texture(u_tex, v_uv).rgb;
        float k = u_subpixel_contrast * light;
        vec3 e = s * (k + 1.0) / (s * k + 1.0);
        vec3 correction = (u_gamma.x * c + u_gamma.y) * e + (u_gamma.z * c + u_gamma.w);
        a = (e + e * (1.0 - e) * correction)[u_channel];
    }
    if (clipped(p, v_mask)) { discard; }
    o = vec4(c, v_color.a * a);
}
";

/// Path triangles into the offscreen layer: position and curve
/// coordinates, color, clip.
pub const PATH_VS: &str = "
in vec4 a0; in vec4 a1; in vec4 a2;
out vec2 v_st; flat out vec4 v_color; flat out vec4 v_clip;
void main() {
    gl_Position = to_device(a0.xy);
    v_st = a0.zw; v_color = a1; v_clip = a2;
}
";

/// `fs_path_rasterization` for solid colors.
pub const PATH_FS: &str = "
in vec2 v_st; flat in vec4 v_color; flat in vec4 v_clip;
void main() {
    vec2 dx = dFdx(v_st);
    vec2 dy = dFdy(v_st);
    if (clipped(frag(), v_clip)) { discard; }
    float alpha = 1.0;
    if (length(vec2(dx.x, dy.x)) >= 0.001) {
        vec2 gradient = 2.0 * v_st.xx * vec2(dx.x, dy.x) - vec2(dx.y, dy.y);
        float f = v_st.x * v_st.x - v_st.y;
        alpha = clamp(0.5 - f / length(gradient), 0.0, 1.0);
    }
    o = vec4(v_color.rgb * v_color.a * alpha, v_color.a * alpha);
}
";

/// The layer copied back through each path sprite.
pub const LAYER_VS: &str = "
in vec4 a0;
out vec2 v_uv;
void main() {
    vec2 p = a0.xy + unit_vertex() * a0.zw;
    gl_Position = to_device(p);
    v_uv = vec2(p.x / u_vp.x, 1.0 - p.y / u_vp.y);
}
";

pub const LAYER_FS: &str = "
uniform sampler2D u_tex;
in vec2 v_uv;
void main() { o = texture(u_tex, v_uv); }
";

/// Images: bounds, mask, tile in atlas pixels, (opacity, grayscale), radii.
pub const IMAGE_VS: &str = "
uniform vec2 u_atlas;
in vec4 a0; in vec4 a1; in vec4 a2; in vec4 a3; in vec4 a4;
out vec2 v_uv; flat out vec4 v_bounds; flat out vec4 v_mask; flat out vec4 v_misc; flat out vec4 v_radii;
void main() {
    vec2 u = unit_vertex();
    gl_Position = to_device(a0.xy + u * a0.zw);
    v_uv = (a2.xy + u * a2.zw) / u_atlas;
    v_bounds = a0; v_mask = a1; v_misc = a3; v_radii = a4;
}
";

/// `fs_poly_sprite` without edge fades or alpha masks.
pub const IMAGE_FS: &str = "
uniform sampler2D u_tex;
in vec2 v_uv; flat in vec4 v_bounds; flat in vec4 v_mask; flat in vec4 v_misc; flat in vec4 v_radii;
void main() {
    vec4 color = texture(u_tex, v_uv);
    vec2 p = frag();
    if (clipped(p, v_mask)) { discard; }
    if (v_misc.y != 0.0) {
        color = vec4(vec3(dot(color.rgb, vec3(0.2126, 0.7152, 0.0722))), color.a);
    }
    float distance = quad_sdf(p, v_bounds, v_radii);
    o = vec4(color.rgb, color.a * v_misc.x * clamp(0.5 - distance, 0.0, 1.0));
}
";
