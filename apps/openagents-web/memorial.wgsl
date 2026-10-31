// ---- generated: apps/openagents-web/gpu/memorial.coax (the module) by coax wgsl; do not edit
struct coax_t9 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_in_9 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
}
struct coax_t11 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 12>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(0) @binding(1) var sans_atlas: texture_2d<f32>;
@group(0) @binding(2) var sans_sampler: sampler;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t9) -> coax_t11 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t9 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_o: coax_t11;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_o = coax_t11();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_h: f32;
  var l_t: f32;
  var l_c: vec3f;
  var l_cell: vec2f;
  var l_tw: f32;
  var l_m: vec4f;
  var l_d: f32;
  var l_k: f32;
  var l_slide: f32;
  var l_far: f32;
  var l_near: f32;
  var l_s: f32;
  var l_i: i32;
  l_h = coax_at_u(i32(1i));
  l_t = coax_p_clampf((l_pos.y / l_h), 0.0f, 0x1.000000p0f);
  l_c = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.e147aep-3f, 0x1.28f5c2p-2f, 0x1.ae147ap-2f), coax_p_v3(0x1.e147aep-2f, 0x1.0f5c28p-1f, 0x1.51eb86p-1f), coax_p_smoothstep(0.0f, 0x1.000000p0f, l_t));
  l_cell = coax_p_wg_floor_f2(coax_p_wg_div_f2f(coax_p_v2(l_pos.x, l_pos.y), 0x1.800000p1f));
  l_tw = select((0x1.000000p-1f + (0x1.000000p-1f * sin(((coax_at_u(i32(2i)) * 0x1.19999ap0f) + (coax_p_memorial_hash(l_cell) * 0x1.400000p5f))))), 0x1.000000p0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_make_f3_f((((coax_p_stepf(0x1.ff3b64p-1f, coax_p_memorial_hash(l_cell)) * 0x1.ccccccp-2f) * l_tw) * (0x1.000000p0f - l_t))));
  l_m = coax_p_wg_at4(40i);
  if ((l_m.z > 0.0f))
  {
    l_d = (coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_pos.x, l_pos.y), coax_p_v2(l_m.x, l_m.y))) / l_m.z);
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.3d70a4p-1f, 0x1.51eb86p-1f, 0x1.99999ap-1f), (exp((-((coax_p_fmax((l_d - 0x1.000000p0f), 0.0f) * 0x1.333334p0f) * (coax_p_fmax((l_d - 0x1.000000p0f), 0.0f) * 0x1.333334p0f)))) * 0x1.666666p-2f));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.eb851ep-1f, 0x1.e147aep-1f, 0x1.bd70a4p-1f), (0x1.000000p0f - coax_p_smoothstep(0x1.f851ecp-1f, 0x1.03d70ap0f, l_d)));
  }
  l_k = (coax_at_u(i32(1i)) / 0x1.900000p9f);
  l_slide = (coax_at_u(i32(43i)) * 0x1.47ae14p-4f);
  l_far = (l_h - ((0x1.b80000p6f + (0x1.180000p6f * coax_p_memorial_ridge((l_pos.x + (l_slide * 0x1.000000p-1f)), (0x1.680000p7f * l_k), 0x1.800000p1f))) * l_k));
  l_near = (l_h - ((0x1.400000p5f + (0x1.400000p6f * coax_p_memorial_ridge((l_pos.x + l_slide), (0x1.400000p7f * l_k), 0x1.c00000p2f))) * l_k));
  if ((l_pos.y > l_far))
  {
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.51eb86p-2f, 0x1.99999ap-2f, 0x1.000000p-1f), 0x1.ccccccp-1f);
  }
  if ((l_pos.y > l_near))
  {
    l_s = (coax_p_memorial_ridge(((l_pos.x + l_slide) + 0x1.000000p0f), (0x1.400000p7f * l_k), 0x1.c00000p2f) - coax_p_memorial_ridge(((l_pos.x + l_slide) - 0x1.000000p0f), (0x1.400000p7f * l_k), 0x1.c00000p2f));
    l_c = select(coax_p_v3(0x1.147ae2p-2f, 0x1.70a3d8p-2f, 0x1.851eb8p-2f), coax_p_v3(0x1.51eb86p-2f, 0x1.b851ecp-2f, 0x1.ae147ap-2f), (l_s < 0.0f));
  }
  l_c = coax_p_wg_mul_f3f(l_c, coax_at_u(i32(3i)));
  {
    l_i = 0i;
    while ((l_i < 4i))
    {
      l_c = coax_p_memorial_panel(coax_p_v2(l_pos.x, l_pos.y), l_c, coax_p_wg_at4(((i32((l_i * 2i)) * 4i) + 8i)), coax_p_wg_at4(((i32(((l_i * 2i) + 1i)) * 4i) + 8i)));
      l_i = (l_i + 1i);
    }
  }
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t11) -> vec4f {
  var l_v: coax_t11 = p_v;
  var l_lod: f32;
  var l_a: f32;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  if ((l_v.w.w > 0x1.c00000p1f))
  {
    l_lod = coax_p_clampf(coax_p_wg_log2((0x1.000000p6f / coax_p_fmax(l_v.z.x, 0x1.000000p0f))), 0.0f, 0x1.000000p2f);
    l_a = coax_pow_bg_vs((textureSampleLevel(sans_atlas, sans_sampler, l_v.y, l_lod)).w, 0x1.b33334p-1f);
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_a), coax_p_fmin((l_v.z.z * 0x1.266666p0f), 0x1.000000p0f)), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.c28f5cp-3f);
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
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
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_memorial_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_stepf(p_edge: f32, p_x: f32) -> f32 {
  var l_edge: f32 = p_edge;
  var l_x: f32 = p_x;
  if ((l_x < l_edge))
  {
    return 0.0f;
  }
  return 0x1.000000p0f;
}
fn coax_p_wg_make_f3_f(p_p0: f32) -> vec3f {
  var l_p0: f32 = p_p0;
  return coax_p_v3(l_p0, l_p0, l_p0);
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
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
fn coax_p_memorial_ridge(p_x: f32, p_step: f32, p_seed: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_step: f32 = p_step;
  var l_seed: f32 = p_seed;
  var l_i: f32;
  var l_f: f32;
  var l_a: f32;
  var l_b: f32;
  l_i = floor((l_x / l_step));
  l_f = coax_p_fract((l_x / l_step));
  l_a = coax_p_memorial_hash(coax_p_v2(l_i, l_seed));
  l_b = coax_p_memorial_hash(coax_p_v2((l_i + 0x1.000000p0f), l_seed));
  return coax_p_mix(l_a, l_b, l_f);
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_memorial_panel(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_alpha: f32;
  var l_half: vec2f;
  var l_corner: f32;
  var l_q: vec2f;
  var l_dd: f32;
  var l_arch: f32;
  var l_e: vec2f;
  var l_fill: vec3f;
  var l_inside: f32;
  var l_c: vec3f;
  var l_edge: f32;
  l_alpha = l_s.y;
  if ((l_alpha <= 0.0f))
  {
    return l_under;
  }
  l_half = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_r.z, l_r.w), coax_p_v2(l_r.x, l_r.y)), 0x1.000000p-1f);
  l_corner = l_s.x;
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_dd = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  l_arch = l_s.w;
  if (((l_arch > 0.0f) && (l_p.y < (l_r.y + l_arch))))
  {
    l_e = coax_p_v2(((l_p.x - (l_r.x + l_half.x)) / l_half.x), ((l_p.y - (l_r.y + l_arch)) / l_arch));
    l_dd = ((coax_p_wg_length2(l_e) - 0x1.000000p0f) * coax_p_fmin(l_half.x, l_arch));
  }
  if ((l_dd > 0x1.000000p0f))
  {
    return l_under;
  }
  l_fill = coax_p_v3(0x1.c28f5cp-4f, 0x1.0a3d70p-3f, 0x1.99999ap-3f);
  if (((l_s.z > 0x1.000000p-1f) && (l_s.z < 0x1.800000p0f)))
  {
    l_fill = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.ae147ap-2f, 0x1.d70a3ep-2f, 0x1.28f5c2p-1f), coax_p_v3(0x1.333334p-2f, 0x1.5c28f6p-2f, 0x1.ccccccp-2f), coax_p_clampf(((l_p.y - l_r.y) / coax_p_fmax((l_r.w - l_r.y), 0x1.000000p0f)), 0.0f, 0x1.000000p0f));
  }
  if ((l_s.z > 0x1.800000p0f))
  {
    l_fill = coax_p_v3(0x1.47ae14p-5f, 0x1.99999ap-5f, 0x1.47ae14p-4f);
  }
  l_inside = coax_p_clampf((0x1.000000p-1f - l_dd), 0.0f, 0x1.000000p0f);
  l_c = coax_p_wg_mix_f3f3f(l_under, l_fill, (l_alpha * l_inside));
  if ((l_s.z < 0x1.800000p0f))
  {
    l_edge = coax_p_clampf((0x1.800000p0f - abs((l_dd + 0x1.800000p-1f))), 0.0f, 0x1.000000p0f);
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.b851ecp-1f, 0x1.c28f5cp-1f, 0x1.eb851ep-1f), (l_edge * 0x1.666666p-2f));
  }
  return l_c;
}
fn coax_p_wg_log2(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (log(l_x) / 0x1.62e430p-1f);
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
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_abs_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(abs(l_p0.x), abs(l_p0.y));
}
fn coax_p_wg_max_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v2(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_9) -> coax_io_11 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t9(coax_in.x, coax_in.y, coax_in.z));
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
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