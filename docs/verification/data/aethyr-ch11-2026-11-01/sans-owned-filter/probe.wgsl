struct coax_t12 {
  x: bool,
  y: i32,
  z: i32,
  w: f32,
}
struct coax_t13 {
  x: bool,
  y: i32,
  z: i32,
  w: i32,
  f4: i32,
  f5: f32,
  f6: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 1>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(0) @binding(1) var filter_image: texture_2d<f32>;
@group(0) @binding(2) var filter_sampler: sampler;
fn coax_p_render_sans_probe_vs(p_vertex: u32, p_instance: u32) -> vec4f {
  var l_vertex: u32 = p_vertex;
  var l_instance: u32 = p_instance;
  var l_sf_points: array<vec2f, 3>;
  var l_sf_p: vec2f;
  l_sf_points = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f), coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)));
  l_sf_p = l_sf_points[i32(l_vertex)];
  return coax_p_v4(l_sf_p.x, l_sf_p.y, 0x1.000000p-1f, 0x1.000000p0f);
}
fn coax_p_render_sans_probe_fs(p_position: vec4f) -> vec4f {
  var l_position: vec4f = p_position;
  return coax_p_render_sans_sample(coax_p_v2(coax_at_u(i32(0i)), coax_at_u(i32(1i))), coax_at_u(i32(2i)));
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_render_sans_sample(p_uv: vec2f, p_lod: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_lod: f32 = p_lod;
  var l_plan: coax_t12;
  var l_low: coax_t13;
  var l_high: coax_t13;
  l_plan = coax_p_render_sans_filter_lod(i32(textureDimensions(filter_image).x), i32(textureDimensions(filter_image).y), l_lod);
  if ((!l_plan.x))
  {
    discard;
    return vec4f(0.0f, 0.0f, 0.0f, 0.0f);
  }
  l_low = coax_p_render_sans_filter_cell(coax_p_render_sans_filter_width(l_plan.y), coax_p_render_sans_filter_height(l_plan.y), l_uv.x, l_uv.y);
  if ((!l_low.x))
  {
    discard;
    return vec4f(0.0f, 0.0f, 0.0f, 0.0f);
  }
  l_high = coax_p_render_sans_filter_cell(coax_p_render_sans_filter_width(l_plan.z), coax_p_render_sans_filter_height(l_plan.z), l_uv.x, l_uv.y);
  return coax_p_render_sans_filter_interpolate(coax_p_render_sans_filter_level(l_low, l_plan.y), coax_p_render_sans_filter_level(l_high, l_plan.z), l_plan.w);
}
fn coax_p_render_sans_filter_lod(p_width: i32, p_height: i32, p_lod: f32) -> coax_t12 {
  var l_width: i32 = p_width;
  var l_height: i32 = p_height;
  var l_lod: f32 = p_lod;
  var l_last: f32;
  var l_bounded: f32;
  var l_low: i32;
  var l_high: i32;
  if ((((l_width != coax_p_render_sans_filter_width(0i)) || (l_height != coax_p_render_sans_filter_height(0i))) || ((l_lod - l_lod) != 0.0f)))
  {
    return coax_t12(false, 0i, 0i, 0.0f);
  }
  l_last = 0x1.000000p2f;
  l_bounded = coax_p_clampf(l_lod, 0.0f, l_last);
  l_low = i32(floor(l_bounded));
  l_high = i32(coax_p_fmin(f32((l_low + 1i)), l_last));
  return coax_t12(true, l_low, l_high, (l_bounded - f32(l_low)));
}
fn coax_p_render_sans_filter_width(p_level: i32) -> i32 {
  var l_level: i32 = p_level;
  return (1536i >> u32(l_level));
}
fn coax_p_render_sans_filter_height(p_level: i32) -> i32 {
  var l_level: i32 = p_level;
  return (672i >> u32(l_level));
}
fn coax_p_render_sans_filter_cell(p_width: i32, p_height: i32, p_u: f32, p_v: f32) -> coax_t13 {
  var l_width: i32 = p_width;
  var l_height: i32 = p_height;
  var l_u: f32 = p_u;
  var l_v: f32 = p_v;
  var l_fx: f32;
  var l_fy: f32;
  var l_x: i32;
  var l_y: i32;
  if (((((((l_width < 1i) || (l_width > coax_p_render_sans_filter_width(0i))) || (l_height < 1i)) || (l_height > coax_p_render_sans_filter_height(0i))) || ((l_u - l_u) != 0.0f)) || ((l_v - l_v) != 0.0f)))
  {
    return coax_t13(false, 0i, 0i, 0i, 0i, 0.0f, 0.0f);
  }
  l_fx = ((coax_p_clampf(l_u, 0.0f, 0x1.000000p0f) * f32(l_width)) - 0x1.000000p-1f);
  l_fy = ((coax_p_clampf(l_v, 0.0f, 0x1.000000p0f) * f32(l_height)) - 0x1.000000p-1f);
  l_x = i32(floor(l_fx));
  l_y = i32(floor(l_fy));
  return coax_t13(true, i32(coax_p_clamp_texel(l_x, i32(l_width))), i32(coax_p_clamp_texel(l_y, i32(l_height))), i32(coax_p_clamp_texel((l_x + 1i), i32(l_width))), i32(coax_p_clamp_texel((l_y + 1i), i32(l_height))), (l_fx - f32(l_x)), (l_fy - f32(l_y)));
}
fn coax_p_render_sans_filter_level(p_cell: coax_t13, p_level: i32) -> vec4f {
  var l_cell: coax_t13 = p_cell;
  var l_level: i32 = p_level;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  l_a = textureLoad(filter_image, vec2i(l_cell.y, l_cell.z), l_level);
  l_b = textureLoad(filter_image, vec2i(l_cell.w, l_cell.z), l_level);
  l_c = textureLoad(filter_image, vec2i(l_cell.y, l_cell.f4), l_level);
  l_d = textureLoad(filter_image, vec2i(l_cell.w, l_cell.f4), l_level);
  return coax_p_render_sans_filter_bilinear(l_a, l_b, l_c, l_d, l_cell.f5, l_cell.f6);
}
fn coax_p_render_sans_filter_interpolate(p_low: vec4f, p_high: vec4f, p_amount: f32) -> vec4f {
  var l_low: vec4f = p_low;
  var l_high: vec4f = p_high;
  var l_amount: f32 = p_amount;
  return coax_p_mix4(l_low, l_high, l_amount);
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_b < l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_clamp_texel(p_i: i32, p_n: i32) -> i32 {
  var l_i: i32 = p_i;
  var l_n: i32 = p_n;
  if ((l_i < 0i))
  {
    return 0i;
  }
  if ((l_i >= l_n))
  {
    return (l_n - 1i);
  }
  return l_i;
}
fn coax_p_render_sans_filter_bilinear(p_a: vec4f, p_b: vec4f, p_c: vec4f, p_d: vec4f, p_tx: f32, p_ty: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  var l_c: vec4f = p_c;
  var l_d: vec4f = p_d;
  var l_tx: f32 = p_tx;
  var l_ty: f32 = p_ty;
  return coax_p_mix4(coax_p_mix4(l_a, l_b, l_tx), coax_p_mix4(l_c, l_d, l_tx), l_ty);
}
fn coax_p_mix4(p_a: vec4f, p_b: vec4f, p_t: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  var l_t: f32 = p_t;
  return vec4f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t), coax_p_mix(l_a.w, l_b.w, l_t));
}
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_b > l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
@vertex
fn render_sans_probe_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_render_sans_probe_vs(coax_vid, coax_iid);
  return coax_r;
}
@fragment
fn render_sans_probe_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_render_sans_probe_fs(coax_pos);
  return coax_r;
}
