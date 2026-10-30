// /chat's passes: GPUI's quad, shadow and sprite shaders, the WebGPU port of
// apps/openagents-desktop/Zeron.metal (itself zeronsh/zui's
// gpui_macos/src/shaders.metal, Apache-2.0, the renderer Zeron draws with),
// so the page draws the shared Zeron components (apps/shared/zeron/,
// gpui.coax writes the vertices) exactly as the Mac app does. Based on Zeron
// by Wing, MIT. Colors are straight sRGB with alpha, blended source-over into
// a non-sRGB target, as GPUI does; the host (coax/web/gpui.js) gives this the
// glyph and icon atlas (gpui_atlas, r8) and the color-glyph atlas
// (gpui_color, rgba8, straight alpha).
//
// Hand-written WGSL, as Zeron.metal is hand-written Metal: GPUI's fragment
// shaders are not Coax, so coax/web/wgsl.py (which translates Coax scalar
// functions) has nothing to translate. Keep the two files in step.
//
// A vertex is 28 floats (gpui.coax GP_VERTEX_FLOATS):
//   a: device x, y, kind (0 quad, 1 shadow, 2 sprite, 3 color sprite, 4 elbow, 5 gradient, 6 inset), blur
//   b: the primitive's bounds in device pixels: x, y, w, h
//   c: corner radius, border width, u, v
//   d: fill (a sprite's color)
//   e: border color
//   f: the content mask, device x0, y0, x1, y1
//   g: GPUI's EdgeFade, device top y, top band, bottom y, bottom band

struct U { size: vec2f, pad0: f32, pad1: f32 }
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var gpui_atlas: texture_2d<f32>;
@group(0) @binding(2) var gpui_color: texture_2d<f32>;
@group(0) @binding(3) var gpui_sampler: sampler;

// The canvas: Zeron Dark's shell (the frame paints it too).
@vertex fn bg_vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let p = array(vec2f(-1.0, -3.0), vec2f(-1.0, 1.0), vec2f(3.0, 1.0));
  return vec4f(p[i], 0.0, 1.0);
}
@fragment fn bg_fs(@builtin(position) pos: vec4f) -> @location(0) vec4f {
  return vec4f(13.0 / 255.0, 13.0 / 255.0, 13.0 / 255.0, 1.0);
}

struct ZIn {
  @location(0) a: vec4f, @location(1) b: vec4f, @location(2) c: vec4f, @location(3) d: vec4f,
  @location(4) e: vec4f, @location(5) f: vec4f, @location(6) g: vec4f,
}
struct ZV {
  @builtin(position) pos: vec4f,
  @location(0) @interpolate(flat) kind_blur: vec2f,
  @location(1) @interpolate(flat) bounds: vec4f,
  @location(2) @interpolate(flat) shape: vec2f,
  @location(3) uv: vec2f,
  @location(4) @interpolate(flat) fill: vec4f,
  @location(5) @interpolate(flat) border: vec4f,
  @location(6) @interpolate(flat) clip: vec4f,
  @location(7) @interpolate(flat) fade: vec4f,
}

@vertex fn fx_vs(v: ZIn) -> ZV {
  var o: ZV;
  o.pos = vec4f(v.a.x / u.size.x * 2.0 - 1.0, 1.0 - v.a.y / u.size.y * 2.0, 0.0, 1.0);
  o.kind_blur = v.a.zw; o.bounds = v.b; o.shape = v.c.xy; o.uv = v.c.zw;
  o.fill = v.d; o.border = v.e; o.clip = v.f; o.fade = v.g;
  return o;
}

fn quad_sdf_impl(corner_center_to_point: vec2f, corner_radius: f32) -> f32 {
  if (corner_radius == 0.0) { return max(corner_center_to_point.x, corner_center_to_point.y); }
  let signed_distance_to_inset_quad = length(max(vec2f(0.0), corner_center_to_point)) +
                                      min(0.0, max(corner_center_to_point.x, corner_center_to_point.y));
  return signed_distance_to_inset_quad - corner_radius;
}
fn over(below: vec4f, above: vec4f) -> vec4f {
  let alpha = above.a + below.a * (1.0 - above.a);
  return vec4f((above.rgb * above.a + below.rgb * below.a * (1.0 - above.a)) / alpha, alpha);
}
fn gaussian(x: f32, sigma: f32) -> f32 { return exp(-(x * x) / (2.0 * sigma * sigma)) / (sqrt(2.0 * 3.14159265) * sigma); }
fn erf2(x: vec2f) -> vec2f {
  let s = sign(x); let a = abs(x);
  let r1 = 1.0 + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
  let r2 = r1 * r1;
  return s - s / (r2 * r2);
}
fn blur_along_x(x: f32, y: f32, sigma: f32, corner: f32, half_size: vec2f) -> f32 {
  let delta = min(half_size.y - corner - abs(y), 0.0);
  let curved = half_size.x - corner + sqrt(max(0.0, corner * corner - delta * delta));
  let integral = 0.5 + 0.5 * erf2((x + vec2f(-curved, curved)) * (sqrt(0.5) / sigma));
  return integral.y - integral.x;
}

// GPUI quad_fragment: one radius and one border width on every side, solid fills.
fn quad_color(p: vec2f, bounds: vec4f, corner_radius: f32, bw: f32, background_color: vec4f, border_color: vec4f) -> vec4f {
  if (bw == 0.0 && corner_radius == 0.0) { return background_color; }
  let antialias_threshold = 0.5;
  let half_size = bounds.zw / 2.0;
  let center_to_point = p - bounds.xy - half_size;
  let reduced_border = select(bw, -antialias_threshold, bw == 0.0);
  let corner_to_point = abs(center_to_point) - half_size;
  let corner_center_to_point = corner_to_point + corner_radius;
  let is_near_rounded_corner = corner_center_to_point.x >= 0.0 && corner_center_to_point.y >= 0.0;
  let straight_border_inner_corner_to_point = corner_to_point + reduced_border;
  let is_beyond_inner_straight_border = straight_border_inner_corner_to_point.x > 0.0 || straight_border_inner_corner_to_point.y > 0.0;
  let is_within_inner_straight_border = straight_border_inner_corner_to_point.x < -antialias_threshold &&
                                        straight_border_inner_corner_to_point.y < -antialias_threshold;
  if (is_within_inner_straight_border && !is_near_rounded_corner) { return background_color; }
  let outer_sdf = quad_sdf_impl(corner_center_to_point, corner_radius);
  var inner_sdf = 0.0;
  if (corner_center_to_point.x <= 0.0 || corner_center_to_point.y <= 0.0) {
    inner_sdf = -max(straight_border_inner_corner_to_point.x, straight_border_inner_corner_to_point.y);
  } else if (is_beyond_inner_straight_border) {
    inner_sdf = -1.0;
  } else {
    inner_sdf = -(outer_sdf + reduced_border);
  }
  let border_sdf = max(inner_sdf, outer_sdf);
  var color = background_color;
  if (border_sdf < antialias_threshold) {
    let blended_border = over(background_color, border_color);
    color = mix(background_color, blended_border, saturate(antialias_threshold - inner_sdf));
  }
  return color * vec4f(1.0, 1.0, 1.0, saturate(antialias_threshold - outer_sdf));
}

// GPUI shadow_fragment for a drop shadow.
fn shadow_color(p: vec2f, bounds: vec4f, corner_radius: f32, blur: f32, color: vec4f) -> vec4f {
  let half_size = bounds.zw / 2.0;
  let pt = p - (bounds.xy + half_size);
  var alpha = 0.0;
  if (blur == 0.0) {
    let corner_center_to_point = abs(pt) - half_size + corner_radius;
    alpha = saturate(0.5 - quad_sdf_impl(corner_center_to_point, corner_radius));
  } else {
    let low = pt.y - half_size.y;
    let high = pt.y + half_size.y;
    let start = clamp(-3.0 * blur, low, high);
    let end = clamp(3.0 * blur, low, high);
    let step = (end - start) / 4.0;
    var y = start + step * 0.5;
    for (var i = 0; i < 4; i++) {
      alpha += blur_along_x(pt.x, pt.y - y, blur, corner_radius, half_size) * gaussian(y, blur) * step;
      y += step;
    }
  }
  return color * vec4f(1.0, 1.0, 1.0, alpha);
}

// A vertical linear gradient (GPUI linear_gradient, 180 degrees, sRGB stops), no border.
fn gradient_color(p: vec2f, bounds: vec4f, corner_radius: f32, top: vec4f, bottom: vec4f) -> vec4f {
  let t = saturate((p.y - bounds.y) / max(bounds.w, 1.0));
  return quad_color(p, bounds, corner_radius, 0.0, mix(top, bottom, t), vec4f(0.0));
}

// An unblurred inset shadow dy down (GPUI paint_inset_shadows).
fn inset_color(p: vec2f, bounds: vec4f, corner_radius: f32, dy: f32, color: vec4f) -> vec4f {
  let half_size = bounds.zw / 2.0;
  let a = abs(p - bounds.xy - half_size) - half_size + corner_radius;
  let b = abs(p - vec2f(0.0, dy) - bounds.xy - half_size) - half_size + corner_radius;
  let inside = saturate(0.5 - quad_sdf_impl(a, corner_radius));
  let covered = saturate(0.5 - quad_sdf_impl(b, corner_radius));
  return color * vec4f(1.0, 1.0, 1.0, inside * (1.0 - covered));
}

// zui shaders.metal edge_fade_alpha, the top and bottom edges.
fn edge_fade_alpha(p: vec2f, fade: vec4f) -> f32 {
  var ramp = 1.0;
  if (fade.y > 0.0) { ramp = min(ramp, clamp((p.y - fade.x) / fade.y, 0.0, 1.0)); }
  if (fade.w > 0.0) { ramp = min(ramp, clamp((fade.z - p.y) / fade.w, 0.0, 1.0)); }
  return ramp * ramp;
}

fn segment_distance(p: vec2f, a: vec2f, b: vec2f) -> f32 {
  let pa = p - a; let ba = b - a;
  let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
  return length(pa - ba * h);
}

// The activity rail's elbow (transcript.rs activity_branch_points).
fn elbow_color(p: vec2f, b: vec4f, c: vec2f, color: vec4f) -> vec4f {
  let s = b.xy; let r = b.z; let end_x = b.w; let half_w = 0.5 * c.x;
  var dist = 1e9;
  var prev = s;
  for (var k = 1; k <= 24; k++) {
    let t = f32(k) / 24.0;
    let q = s + r * vec2f(t * t, 2.0 * t - t * t);
    dist = min(dist, segment_distance(p, prev, q));
    prev = q;
  }
  dist = min(dist, segment_distance(p, prev, vec2f(end_x, s.y + r)));
  var cover = saturate(half_w + 0.5 - dist);
  if (c.y > 0.5) { cover = max(0.0, cover - saturate(half_w + 0.5 - abs(p.x - s.x))); }
  return color * vec4f(1.0, 1.0, 1.0, cover);
}

@fragment fn fx_fs(v: ZV) -> @location(0) vec4f {
  let p = v.pos.xy;
  if (p.x < v.clip.x || p.y < v.clip.y || p.x >= v.clip.z || p.y >= v.clip.w) { discard; }
  // read texel for pixel, as GPUI's sprites land on whole device pixels (no filtering to blur them)
  let mono = textureLoad(gpui_atlas, vec2i(floor(v.uv * vec2f(textureDimensions(gpui_atlas)))), 0).r;
  let poly = textureLoad(gpui_color, vec2i(floor(v.uv * vec2f(textureDimensions(gpui_color)))), 0);
  let kind = v.kind_blur.x;
  var c: vec4f;
  if (kind > 5.5) { c = inset_color(p, v.bounds, v.shape.x, v.kind_blur.y, v.fill); }
  else if (kind > 4.5) { c = gradient_color(p, v.bounds, v.shape.x, v.fill, v.border); }
  else if (kind > 3.5) { c = elbow_color(p, v.bounds, v.shape, v.fill); }
  else if (kind > 2.5) { c = vec4f(poly.rgb, poly.a * v.fill.a); }
  else if (kind > 1.5) { c = vec4f(v.fill.rgb, v.fill.a * mono); }
  else if (kind > 0.5) { c = shadow_color(p, v.bounds, v.shape.x, v.kind_blur.y, v.fill); }
  else { c = quad_color(p, v.bounds, v.shape.x, v.shape.y, v.fill, v.border); }
  c.a *= edge_fade_alpha(p, v.fade);
  return c;
}
