// GPUI's quad, shadow and monochrome sprite shaders (zeronsh/zui's
// gpui_macos/src/shaders.metal, Apache-2.0, the renderer Zeron draws with),
// cut to what zeron/gpui.coax paints: one vertex stream, the kind in each
// vertex. Based on Zeron by Wing, MIT. Colors are straight sRGB with alpha,
// blended source-over into a BGRA8Unorm target, as GPUI does.
//
// A vertex is 24 floats (gpui.coax GP_VERTEX_FLOATS):
//   a: device x, y, kind (0 quad, 1 shadow, 2 sprite, 3 color sprite), blur (a shadow's sigma)
//   b: the primitive's bounds in device pixels: x, y, w, h
//   c: corner radius, border width, u, v (a sprite's atlas coordinate)
//   d: fill (a sprite's color)
//   e: border color
//   f: the content mask, device x0, y0, x1, y1
// Color sprites (emoji) sample the color atlas: straight-alpha RGBA, as
// gpui_macos stores polychrome glyphs.
#include <metal_stdlib>
using namespace metal;

struct ZIn { float4 a [[attribute(0)]]; float4 b [[attribute(1)]]; float4 c [[attribute(2)]]; float4 d [[attribute(3)]]; float4 e [[attribute(4)]]; float4 f [[attribute(5)]]; };
struct ZV {
  float4 pos [[position]];
  float kind [[flat]]; float blur [[flat]];
  float4 bounds [[flat]];
  float2 shape [[flat]];
  float2 uv;
  float4 fill [[flat]]; float4 border [[flat]];
  float4 clip [[flat]];
};

vertex ZV zr_vs(ZIn v [[stage_in]], constant float2 &viewport [[buffer(1)]]) {
  ZV o;
  o.pos = float4(v.a.x / viewport.x * 2.0 - 1.0, 1.0 - v.a.y / viewport.y * 2.0, 0.0, 1.0);
  o.kind = v.a.z; o.blur = v.a.w; o.bounds = v.b; o.shape = v.c.xy; o.uv = v.c.zw;
  o.fill = v.d; o.border = v.e; o.clip = v.f;
  return o;
}

static float quad_sdf_impl(float2 corner_center_to_point, float corner_radius) {
  if (corner_radius == 0.0) { return max(corner_center_to_point.x, corner_center_to_point.y); }
  float signed_distance_to_inset_quad = length(max(float2(0.0), corner_center_to_point)) +
                                        min(0.0, max(corner_center_to_point.x, corner_center_to_point.y));
  return signed_distance_to_inset_quad - corner_radius;
}
static float4 over(float4 below, float4 above) {
  float alpha = above.a + below.a * (1.0 - above.a);
  return float4((above.rgb * above.a + below.rgb * below.a * (1.0 - above.a)) / alpha, alpha);
}
static float gaussian(float x, float sigma) { return exp(-(x * x) / (2. * sigma * sigma)) / (sqrt(2. * M_PI_F) * sigma); }
static float2 erf2(float2 x) {
  float2 s = sign(x); float2 a = abs(x);
  float2 r1 = 1. + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
  float2 r2 = r1 * r1;
  return s - s / (r2 * r2);
}
static float blur_along_x(float x, float y, float sigma, float corner, float2 half_size) {
  float delta = min(half_size.y - corner - abs(y), 0.);
  float curved = half_size.x - corner + sqrt(max(0., corner * corner - delta * delta));
  float2 integral = 0.5 + 0.5 * erf2((x + float2(-curved, curved)) * (sqrt(0.5) / sigma));
  return integral.y - integral.x;
}

// GPUI quad_fragment: one radius and one border width on every side, solid fills.
static float4 quad_color(float2 p, float4 bounds, float corner_radius, float bw, float4 background_color, float4 border_color) {
  if (bw == 0.0 && corner_radius == 0.0) { return background_color; }
  const float antialias_threshold = 0.5;
  float2 half_size = bounds.zw / 2.0;
  float2 center_to_point = p - bounds.xy - half_size;
  float reduced_border = bw == 0.0 ? -antialias_threshold : bw;
  float2 corner_to_point = fabs(center_to_point) - half_size;
  float2 corner_center_to_point = corner_to_point + corner_radius;
  bool is_near_rounded_corner = corner_center_to_point.x >= 0.0 && corner_center_to_point.y >= 0.0;
  float2 straight_border_inner_corner_to_point = corner_to_point + reduced_border;
  bool is_beyond_inner_straight_border = straight_border_inner_corner_to_point.x > 0.0 || straight_border_inner_corner_to_point.y > 0.0;
  bool is_within_inner_straight_border = straight_border_inner_corner_to_point.x < -antialias_threshold &&
                                         straight_border_inner_corner_to_point.y < -antialias_threshold;
  if (is_within_inner_straight_border && !is_near_rounded_corner) { return background_color; }
  float outer_sdf = quad_sdf_impl(corner_center_to_point, corner_radius);
  float inner_sdf = 0.0;
  if (corner_center_to_point.x <= 0.0 || corner_center_to_point.y <= 0.0) {
    inner_sdf = -max(straight_border_inner_corner_to_point.x, straight_border_inner_corner_to_point.y);
  } else if (is_beyond_inner_straight_border) {
    inner_sdf = -1.0;
  } else {
    inner_sdf = -(outer_sdf + reduced_border);
  }
  float border_sdf = max(inner_sdf, outer_sdf);
  float4 color = background_color;
  if (border_sdf < antialias_threshold) {
    float4 blended_border = over(background_color, border_color);
    color = mix(background_color, blended_border, saturate(antialias_threshold - inner_sdf));
  }
  return color * float4(1.0, 1.0, 1.0, saturate(antialias_threshold - outer_sdf));
}

// GPUI shadow_fragment for a drop shadow.
static float4 shadow_color(float2 p, float4 bounds, float corner_radius, float blur, float4 color) {
  float2 half_size = bounds.zw / 2.;
  float2 point = p - (bounds.xy + half_size);
  float alpha;
  if (blur == 0.) {
    float2 corner_center_to_point = fabs(point) - half_size + corner_radius;
    alpha = saturate(0.5 - quad_sdf_impl(corner_center_to_point, corner_radius));
  } else {
    float low = point.y - half_size.y;
    float high = point.y + half_size.y;
    float start = clamp(-3. * blur, low, high);
    float end = clamp(3. * blur, low, high);
    float step = (end - start) / 4.;
    float y = start + step * 0.5;
    alpha = 0.;
    for (int i = 0; i < 4; i++) {
      alpha += blur_along_x(point.x, point.y - y, blur, corner_radius, half_size) * gaussian(y, blur) * step;
      y += step;
    }
  }
  return color * float4(1., 1., 1., alpha);
}

fragment float4 zr_fs(ZV v [[stage_in]], texture2d<float> atlas [[texture(0)]], texture2d<float> color_atlas [[texture(1)]]) {
  if (v.pos.x < v.clip.x || v.pos.y < v.clip.y || v.pos.x >= v.clip.z || v.pos.y >= v.clip.w) { discard_fragment(); }
  if (v.kind > 2.5) {
    constexpr sampler s(mag_filter::linear, min_filter::linear);
    float4 c = color_atlas.sample(s, v.uv);
    c.a *= v.fill.a;
    return c;
  }
  if (v.kind > 1.5) {
    constexpr sampler s(mag_filter::linear, min_filter::linear);
    float4 c = v.fill;
    c.a *= atlas.sample(s, v.uv).r;
    return c;
  }
  if (v.kind > 0.5) { return shadow_color(v.pos.xy, v.bounds, v.shape.x, v.blur, v.fill); }
  return quad_color(v.pos.xy, v.bounds, v.shape.x, v.shape.y, v.fill, v.border);
}
