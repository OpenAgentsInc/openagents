// ---- generated: bitcoin/explorer/gpu.coax (the module) by coax wgsl; do not edit
struct coax_t7 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_7 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t10 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_10 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 4>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(0) @binding(1) var sans_atlas: texture_2d<f32>;
@group(0) @binding(2) var sans_sampler: sampler;
fn coax_p_bg_vs(p_i: u32, p_iid: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l_iid: u32 = p_iid;
  return coax_p_nip_bg_vs(l_i, l_iid);
}
fn coax_p_fx_vs(p_vid: u32, p_iid: u32, p_in_: coax_t7) -> coax_t10 {
  var l_vid: u32 = p_vid;
  var l_iid: u32 = p_iid;
  var l_in_: coax_t7 = p_in_;
  return coax_p_nip_fx_vs(l_vid, l_iid, l_in_);
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  return coax_p_v4(0x1.cac084p-7f, 0x1.26e978p-6f, 0x1.a9fbe8p-6f, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t10) -> vec4f {
  var l_v: coax_t10 = p_v;
  return coax_p_nip_fx_fs(l_v);
}
fn coax_p_nip_bg_vs(p_i: u32, p_iid: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l_iid: u32 = p_iid;
  return coax_p_splash_bg_vs(l_i, l_iid);
}
fn coax_p_nip_fx_vs(p_vid: u32, p_iid: u32, p_in_: coax_t7) -> coax_t10 {
  var l_vid: u32 = p_vid;
  var l_iid: u32 = p_iid;
  var l_in_: coax_t7 = p_in_;
  return coax_p_splash_fx_vs(l_vid, l_iid, l_in_);
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_nip_fx_fs(p_v: coax_t10) -> vec4f {
  var l_v: coax_t10 = p_v;
  var l_lod: f32;
  var l_a: f32;
  if (((l_v.w.w > 0x1.c00000p1f) && (l_v.z.y == 0.0f)))
  {
    l_lod = coax_p_clampf(coax_p_wg_log2((0x1.000000p6f / coax_p_fmax(l_v.z.x, 0x1.000000p0f))), 0.0f, 0x1.000000p2f);
    l_a = coax_pow_bg_vs((textureSampleLevel(sans_atlas, sans_sampler, l_v.y, l_lod)).w, 0x1.b33334p-1f);
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_a), coax_p_fmin((l_v.z.z * 0x1.266666p0f), 0x1.000000p0f)), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  return coax_p_splash_fx_fs(l_v);
}
fn coax_p_splash_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_splash_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t7) -> coax_t10 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t7 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t10;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t10();
  l_ndc = coax_p_v2((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_o.x = coax_p_v4(l_ndc.x, l_ndc.y, 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
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
fn coax_p_wg_log2(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (log(l_x) / 0x1.62e430p-1f);
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
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
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_splash_fx_fs(p_v: coax_t10) -> vec4f {
  var l_v: coax_t10 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_mode: f32;
  var l_p: vec2f;
  var l_c: f32;
  var l_r: f32;
  var l_tail: f32;
  var l_g: f32;
  var l_hot: f32;
  var l_c_11: vec3f;
  var l_core: f32;
  var l_g_13: f32;
  var l_halo: f32;
  var l_ends: f32;
  var l_c_16: vec3f;
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_mode = l_v.w.w;
  if ((l_mode > 0x1.c00000p1f))
  {
    l_p = coax_p_wg_div_f2f(l_v.y, (0x1.000000p0f - (0x1.eb851ep-6f * l_v.f4.x)));
    l_c = coax_p_verse_bar((-l_p.x), l_p.y, l_v.z.x, l_v.z.y, l_v.z.z, coax_at_u(i32(2i)), l_v.f4.x, l_v.f4.y);
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_c), l_v.z.w), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  if (((l_mode > 0x1.800000p0f) && (l_mode < 0x1.400000p1f)))
  {
    l_r = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
    if ((l_r >= 0x1.000000p0f))
    {
      return coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f);
    }
    l_tail = select(0x1.000000p0f, coax_p_mix(0x1.47ae14p-4f, 0x1.000000p0f, coax_p_clampf((l_v.y.x / l_v.z.x), 0.0f, 0x1.000000p0f)), (l_v.z.x > 0x1.000000p0f));
    l_g = (exp((((-l_r) * l_r) * 0x1.400000p2f)) * (0x1.000000p0f - l_r));
    l_hot = (exp((((-l_r) * l_r) * 0x1.200000p5f)) * l_v.z.y);
    l_c_11 = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_g), coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.b851ecp-1f), l_hot)), l_tail), l_v.z.z), coax_at_u(i32(3i)));
    return coax_p_v4(l_c_11.x, l_c_11.y, l_c_11.z, 0x1.000000p0f);
  }
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g_13 = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g_13) * l_g_13)) * 0x1.1eb852p-2f);
  if (((l_mode > 0x1.000000p-1f) && (l_mode < 0x1.800000p0f)))
  {
    l_ends = (coax_p_smoothstep(0.0f, 0x1.000000p3f, l_v.y.x) * coax_p_smoothstep(0.0f, 0x1.000000p3f, (l_v.z.x - l_v.y.x)));
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), ((l_core * 0x1.99999ap-1f) + l_halo)), l_ends), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  if (((l_mode > 0x1.400000p1f) && (l_mode < 0x1.c00000p1f)))
  {
    l_c_16 = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.d70a3ep-1f), 0x1.800000p-1f), l_core), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_halo), 0x1.ccccccp0f)), l_v.z.z), coax_at_u(i32(3i)));
    return coax_p_v4(l_c_16.x, l_c_16.y, l_c_16.z, 0x1.000000p0f);
  }
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_verse_bar(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32, p_t: f32, p_press: f32, p_flash: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_w: f32 = p_w;
  var l_h: f32 = p_h;
  var l_k: f32 = p_k;
  var l_t: f32 = p_t;
  var l_press: f32 = p_press;
  var l_flash: f32 = p_flash;
  var l_d: f32;
  var l_inside: f32;
  var l_edge: f32;
  var l_phase: f32;
  var l_sweep: f32;
  var l_glass: f32;
  var l_glow: f32;
  l_d = coax_p_cut_distance(l_x, l_y, l_w, l_h, l_k);
  l_inside = (0x1.000000p0f - coax_p_smooth((-0x1.000000p-1f), 0x1.000000p-1f, l_d));
  l_edge = (0x1.000000p0f - coax_p_smooth((0x1.000000p-1f * l_k), (0x1.99999ap0f * l_k), abs(l_d)));
  l_phase = ((l_t / 0x1.800000p2f) - floor((l_t / 0x1.800000p2f)));
  l_sweep = exp((-(((((l_x + (l_y * 0x1.333334p-1f)) / l_w) - ((-0x1.99999ap0f) + ((0x1.99999ap1f * l_phase) / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f) * ((((l_x + (l_y * 0x1.333334p-1f)) / l_w) - ((-0x1.99999ap0f) + ((0x1.99999ap1f * l_phase) / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f))));
  if ((l_phase > 0x1.c28f5cp-3f))
  {
    l_sweep = 0.0f;
  }
  l_glass = ((((0x1.1eb852p-5f + (0x1.99999ap-6f * (0x1.000000p0f - (l_y / l_h)))) + (0x1.99999ap-4f * l_press)) + (0x1.99999ap-4f * l_sweep)) * l_inside);
  l_glow = ((exp(((-coax_p_hi(l_d, 0.0f)) / (0x1.200000p3f * l_k))) * (0x1.000000p0f - l_inside)) * (0x1.99999ap-4f + (0x1.333334p-2f * l_flash)));
  return ((l_glass + (l_edge * (0x1.4cccccp-1f + (0x1.000000p-1f * l_flash)))) + l_glow);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_cut_distance(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_w: f32 = p_w;
  var l_h: f32 = p_h;
  var l_k: f32 = p_k;
  var l_dx: f32;
  var l_dy: f32;
  var l_box: f32;
  l_dx = (abs(l_x) - l_w);
  l_dy = (abs(l_y) - l_h);
  l_box = (sqrt(((coax_p_hi(l_dx, 0.0f) * coax_p_hi(l_dx, 0.0f)) + (coax_p_hi(l_dy, 0.0f) * coax_p_hi(l_dy, 0.0f)))) + coax_p_lo(coax_p_hi(l_dx, l_dy), 0.0f));
  return coax_p_hi(l_box, ((abs((l_x + l_y)) - ((l_w + l_h) - (0x1.c00000p3f * l_k))) * 0x1.6a09e6p-1f));
}
fn coax_p_smooth(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_sat(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_hi(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_lo(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_sat(p_a: f32) -> f32 {
  var l_a: f32 = p_a;
  return coax_p_lo(0x1.000000p0f, coax_p_hi(0.0f, l_a));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_7) -> coax_io_10 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t7(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_10(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_10) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t10(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_r;
}
fn coax_bits_bg_vs(b: u32) -> f32 {
  return bitcast<f32>(b);
}
fn coax_pow_bg_vs(x: f32, y: f32) -> f32 {
  if (y == 0.0f) {
    return 1.0f;
  }
  if (x == 0.0f) {
    return select(coax_bits_bg_vs(0x7f800000u), 0.0f, y > 0.0f);
  }
  let r = pow(abs(x), y);
  if (!(x < 0.0f)) {
    return r;
  }
  if (floor(y) != y) {
    return coax_bits_bg_vs(0x7fc00000u);
  }
  if (fract(y * 0.5f) != 0.0f) {
    return -r;
  }
  return r;
}