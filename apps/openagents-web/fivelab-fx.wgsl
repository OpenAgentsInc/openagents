// ---- generated: apps/openagents-web/gpu/fivelab-fx.coax (the module) by coax wgsl; do not edit
struct coax_t8 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_t11 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec2f,
  f5: f32,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) @interpolate(flat) w: vec4f,
  @location(3) f4: vec2f,
  @location(4) @interpolate(flat) f5: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 8>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> puffs: array<coax_t8>;
fn coax_p_particle_vs(p_v: u32, p_n: u32) -> coax_t11 {
  var l_v: u32 = p_v;
  var l_n: u32 = p_n;
  if ((floor(puffs[i32(l_n)].z.w) == 0x1.000000p0f))
  {
    return coax_p_egfx_gone();
  }
  return coax_p_fxg_place(l_v, l_n);
}
fn coax_p_particle_over_vs(p_v: u32, p_n: u32) -> coax_t11 {
  var l_v: u32 = p_v;
  var l_n: u32 = p_n;
  var l_kind: f32;
  l_kind = floor(puffs[i32(l_n)].z.w);
  if (((l_kind != 0x1.000000p0f) && (l_kind != 0.0f)))
  {
    return coax_p_egfx_gone();
  }
  return coax_p_fxg_place(l_v, l_n);
}
fn coax_p_particle_fs(p_p: coax_t11) -> vec4f {
  var l_p: coax_t11 = p_p;
  var l_kind: f32;
  var l_a: f32;
  var l_c: vec3f;
  var l_r: f32;
  var l_band: f32;
  var l_q: vec2f;
  var l_r2: f32;
  var l_m: f32;
  var l_k: f32;
  l_kind = l_p.w.x;
  l_a = 0.0f;
  l_c = coax_p_v3(l_p.z.x, l_p.z.y, l_p.z.z);
  if ((l_kind == 0x1.800000p1f))
  {
    l_r = coax_p_wg_length2(l_p.y);
    l_band = (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.47ae14p-3f, abs((l_r - 0x1.b851ecp-1f))));
    l_a = (l_band * (0x1.666666p-1f + (0x1.333334p-1f * coax_p_egfx_noise2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p.y, 0x1.800000p2f), coax_p_v2((l_p.w.y * 0x1.200000p3f), coax_at_u(i32(2i))))))));
  }
  else
  {
    if ((l_kind == 0x1.000000p1f))
    {
      l_q = l_p.y;
      if ((l_p.w.z > 0.0f))
      {
        l_q.x = coax_p_fmax(((abs(l_q.x) * (l_p.w.z + 0x1.000000p0f)) - l_p.w.z), 0.0f);
      }
      l_r2 = coax_p_wg_dot2(l_q, l_q);
      if ((l_r2 >= 0x1.000000p0f))
      {
        discard;
      }
      l_a = (exp(((-l_r2) * 0x1.c00000p1f)) * (0x1.000000p0f - l_r2));
      l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_make_f3_f(coax_p_fmax(l_c.x, coax_p_fmax(l_c.y, l_c.z))), (l_p.z.w * exp(((-l_r2) * 0x1.200000p4f))));
    }
    else
    {
      l_m = coax_p_egfx_billow(l_p.y, l_p.w.y, 0x1.333334p-1f, 0x1.47ae14p-4f, l_p.w.z);
      if ((l_m <= 0x1.0624dep-9f))
      {
        discard;
      }
      l_c = coax_p_wg_mul_f3f(coax_p_egfx_fire(l_p, l_m), (0x1.47ae14p-4f + (0x1.99999ap-2f * l_p.z.w)));
      l_a = l_m;
    }
  }
  l_k = ((l_a * l_p.f5) * select(coax_p_fxg_soft(l_p), 0x1.000000p0f, (l_kind == 0x1.800000p1f)));
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(l_c, l_k), coax_p_clampf((l_k * 0x1.333334p-1f), 0.0f, 0x1.000000p0f));
}
fn coax_p_particle_over_fs(p_p: coax_t11) -> vec4f {
  var l_p: coax_t11 = p_p;
  var l_m: f32;
  var l_a: f32;
  var l_m_4: f32;
  var l_a_5: f32;
  var l_n: f32;
  var l_r: f32;
  var l_bulge: f32;
  var l_up: f32;
  var l_lit: f32;
  if ((l_p.w.x == 0.0f))
  {
    l_m = coax_p_egfx_billow(l_p.y, l_p.w.y, 0x1.333334p-1f, 0x1.47ae14p-4f, l_p.w.z);
    if ((l_m <= 0x1.0624dep-9f))
    {
      discard;
    }
    l_a = ((coax_p_clampf(l_m, 0.0f, 0x1.000000p0f) * l_p.f5) * coax_p_fxg_soft(l_p));
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_egfx_fire(l_p, l_m), l_a), l_a);
  }
  l_m_4 = coax_p_egfx_billow(l_p.y, l_p.w.y, 0x1.666666p-1f, 0x1.eb851ep-4f, l_p.z.w);
  if ((l_m_4 <= 0x1.0624dep-9f))
  {
    discard;
  }
  l_a_5 = ((coax_p_clampf((l_m_4 * coax_p_fmin((l_p.z.w * 0x1.99999ap0f), 0x1.d70a3ep-1f)), 0.0f, 0x1.000000p0f) * l_p.f5) * coax_p_fxg_soft(l_p));
  l_n = coax_p_egfx_churn(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p.y, 0x1.19999ap0f), coax_p_v2(0.0f, 0x1.99999ap-2f)), (l_p.w.y + 0x1.800000p1f));
  l_r = coax_p_wg_length2(l_p.y);
  l_bulge = sqrt(coax_p_fmax((0x1.000000p0f - (l_r * l_r)), 0.0f));
  l_up = coax_p_clampf((0x1.000000p-1f + (0x1.000000p-1f * ((l_p.y.y + (0x1.333334p-1f * l_bulge)) - 0x1.333334p-2f))), 0.0f, 0x1.000000p0f);
  l_lit = coax_p_mix(0x1.000000p-1f, 0x1.4cccccp0f, coax_p_smoothstep(0x1.ae147ap-2f, 0x1.000000p-1f, (l_up * (0x1.19999ap-1f + (0x1.99999ap-1f * l_n)))));
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_p.z.x, l_p.z.y, l_p.z.z), l_lit), l_a_5), l_a_5);
}
fn coax_p_egfx_gone() -> coax_t11 {
  var l_o: coax_t11;
  l_o = coax_t11();
  l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
  return l_o;
}
fn coax_p_fxg_place(p_v: u32, p_n: u32) -> coax_t11 {
  var l_v: u32 = p_v;
  var l_n: u32 = p_n;
  var l_o: coax_t11;
  var l_e: coax_t8;
  var l_quad: array<vec2f, 6>;
  var l_c: vec2f;
  var l_kind: f32;
  var l_seed: f32;
  var l_size: f32;
  var l_to_eye: vec3f;
  var l_d: f32;
  var l_view: vec3f;
  var l_w: vec3f;
  var l_streak: f32;
  var l_along: f32;
  var l_a: vec3f;
  var l_side: vec3f;
  var l_right: vec3f;
  var l_up: vec3f;
  l_o = coax_t11();
  l_e = puffs[i32(l_n)];
  l_quad = array<vec2f, 6>(coax_p_v2((-0x1.000000p0f), (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, 0x1.000000p0f));
  l_c = l_quad[i32(l_v)];
  l_kind = floor(l_e.z.w);
  l_seed = (coax_p_fract(l_e.z.w) * 0x1.000000p1f);
  l_size = l_e.x.w;
  l_to_eye = coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))), coax_p_v3(l_e.x.x, l_e.x.y, l_e.x.z));
  l_d = coax_p_fmax(coax_p_wg_length3(l_to_eye), 0x1.0624dep-10f);
  l_view = coax_p_wg_div_f3f(l_to_eye, l_d);
  l_w = coax_p_v3(l_e.x.x, l_e.x.y, l_e.x.z);
  l_streak = select(0.0f, coax_p_wg_length3(coax_p_v3(l_e.z.x, l_e.z.y, l_e.z.z)), (l_kind == 0x1.000000p1f));
  l_along = 0.0f;
  if ((l_kind == 0x1.800000p1f))
  {
    l_w = coax_p_wg_add_f3f3(l_w, coax_p_wg_mul_f3f(coax_p_v3(l_c.x, 0.0f, l_c.y), l_size));
  }
  else
  {
    if ((l_streak > 0x1.0624dep-10f))
    {
      l_a = coax_p_wg_div_f3f(coax_p_v3(l_e.z.x, l_e.z.y, l_e.z.z), l_streak);
      l_side = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_cross3(l_a, l_view), coax_p_v3(0x1.4f8b58p-17f, 0.0f, 0.0f)));
      l_w = coax_p_wg_add_f3f3(l_w, coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_a, l_c.x), (l_streak + l_size)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_side, l_c.y), l_size)));
      l_along = (l_streak / coax_p_fmax(l_size, 0x1.0624dep-10f));
    }
    else
    {
      l_right = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_cross3(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_view), coax_p_v3(0x1.4f8b58p-17f, 0.0f, 0.0f)));
      l_up = coax_p_wg_cross3(l_view, l_right);
      l_w = coax_p_wg_add_f3f3(l_w, coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_right, l_c.x), coax_p_wg_mul_f3f(l_up, l_c.y)), l_size));
    }
  }
  l_o.x = coax_p_fxg_clip(l_w);
  l_o.f4 = coax_p_v2(l_o.x.w, coax_p_fmax((l_size * 0x1.333334p-1f), 0x1.eb851ep-4f));
  l_o.y = l_c;
  l_o.z = l_e.y;
  l_o.w = coax_p_v4(l_kind, l_seed, select(l_along, l_e.z.x, (l_kind == 0.0f)), l_d);
  l_o.f5 = select(coax_p_smoothstep(((l_size * 0x1.ccccccp-1f) + 0x1.333334p-2f), ((l_size * 0x1.19999ap1f) + 0x1.000000p0f), l_d), 0x1.000000p0f, (l_kind == 0x1.800000p1f));
  return l_o;
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_egfx_noise2(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_egfx_hash2(l_i), coax_p_egfx_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_egfx_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_egfx_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
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
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_make_f3_f(p_p0: f32) -> vec3f {
  var l_p0: f32 = p_p0;
  return coax_p_v3(l_p0, l_p0, l_p0);
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_egfx_billow(p_c: vec2f, p_seed: f32, p_rag: f32, p_soft: f32, p_cover: f32) -> f32 {
  var l_c: vec2f = p_c;
  var l_seed: f32 = p_seed;
  var l_rag: f32 = p_rag;
  var l_soft: f32 = p_soft;
  var l_cover: f32 = p_cover;
  var l_n: f32;
  var l_edge: f32;
  l_n = coax_p_egfx_churn(l_c, l_seed);
  l_edge = (((0x1.000000p0f - coax_p_wg_length2(l_c)) + ((l_n - 0x1.000000p-1f) * l_rag)) - ((0x1.000000p0f - coax_p_clampf(l_cover, 0.0f, 0x1.000000p0f)) * (0x1.000000p0f + (0x1.000000p-1f * l_rag))));
  return coax_p_smoothstep(0.0f, l_soft, l_edge);
}
fn coax_p_egfx_fire(p_p: coax_t11, p_m: f32) -> vec3f {
  var l_p: coax_t11 = p_p;
  var l_m: f32 = p_m;
  var l_n: f32;
  var l_r: f32;
  var l_bulge: f32;
  var l_v: f32;
  var l_band: f32;
  var l_heart: f32;
  var l_c: vec3f;
  l_n = ((0x1.333334p-1f * coax_p_egfx_churn(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p.y, 0x1.99999ap0f), coax_p_v2(0.0f, ((-coax_at_u(i32(2i))) * 0x1.666666p-1f))), (l_p.w.y + 0x1.400000p2f))) + (0x1.99999ap-2f * coax_p_egfx_noise2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p.y, 0x1.600000p2f), coax_p_v2((l_p.w.y * 0x1.f00000p4f), ((-coax_at_u(i32(2i))) * 0x1.19999ap1f))))));
  l_r = coax_p_wg_length2(l_p.y);
  l_bulge = sqrt(coax_p_fmax((0x1.000000p0f - (l_r * l_r)), 0.0f));
  l_v = (l_bulge * (0x1.000000p-1f + (0x1.99999ap-1f * l_n)));
  l_band = ((coax_p_smoothstep(0x1.70a3d8p-3f, 0x1.0a3d70p-2f, l_v) * 0x1.ccccccp-2f) + (coax_p_smoothstep(0x1.000000p-1f, 0x1.28f5c2p-1f, l_v) * 0x1.19999ap-1f));
  l_heart = (l_p.z.w * coax_p_smoothstep(0x1.70a3d8p-1f, 0x1.99999ap-1f, l_v));
  l_c = coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f3(coax_p_v3(l_p.z.x, l_p.z.y, l_p.z.z), coax_p_v3(0x1.ae147ap-2f, 0x1.99999ap-3f, 0x1.0a3d70p-3f)), coax_p_wg_mul_f3f(coax_p_v3(l_p.z.x, l_p.z.y, l_p.z.z), 0x1.266666p0f), l_band);
  return coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.c28f5cp-1f, 0x1.3d70a4p-1f), coax_p_fmax(coax_p_fmax(l_p.z.x, 0x1.666666p-1f), 0x1.000000p0f)), 0x1.400000p0f), l_heart);
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_fxg_soft(p_p: coax_t11) -> f32 {
  var l_p: coax_t11 = p_p;
  return coax_p_clampf(((coax_p_fxg_ground(coax_p_v2(l_p.x.x, l_p.x.y)) - l_p.f4.x) / l_p.f4.y), 0.0f, 0x1.000000p0f);
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
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
fn coax_p_egfx_churn(p_c: vec2f, p_seed: f32) -> f32 {
  var l_c: vec2f = p_c;
  var l_seed: f32 = p_seed;
  var l_t: f32;
  l_t = coax_at_u(i32(2i));
  return ((0x1.3d70a4p-1f * coax_p_egfx_noise2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_c, 0x1.b33334p0f), coax_p_v2(((l_seed * 0x1.300000p4f) + (l_t * 0x1.666666p-2f)), ((l_seed * 0x1.c00000p2f) - (l_t * 0x1.19999ap-1f)))))) + (0x1.851eb8p-2f * coax_p_egfx_noise2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_c, 0x1.f33334p1f), coax_p_v2(((-l_t) * 0x1.ccccccp-1f), ((l_seed * 0x1.a00000p3f) + (l_t * 0x1.99999ap-2f)))))));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_fxg_clip(p_w: vec3f) -> vec4f {
  var l_w: vec3f = p_w;
  var l_rel: vec3f;
  var l_z: f32;
  var l_r: f32;
  var l_q: f32;
  var l_wd: f32;
  var l_ht: f32;
  var l_f: f32;
  var l_x: f32;
  var l_y: f32;
  l_rel = coax_p_v3((l_w.x - coax_at_u(i32(16i))), (l_w.y - coax_at_u(i32(17i))), (l_w.z - coax_at_u(i32(18i))));
  l_z = (((l_rel.x * coax_at_u(i32(20i))) + (l_rel.y * coax_at_u(i32(21i)))) + (l_rel.z * coax_at_u(i32(22i))));
  l_r = (((l_rel.x * coax_at_u(i32(24i))) + (l_rel.y * coax_at_u(i32(25i)))) + (l_rel.z * coax_at_u(i32(26i))));
  l_q = (((l_rel.x * coax_at_u(i32(28i))) + (l_rel.y * coax_at_u(i32(29i)))) + (l_rel.z * coax_at_u(i32(30i))));
  l_wd = coax_p_fmax(coax_at_u(i32(0i)), 0x1.000000p0f);
  l_ht = coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f);
  l_f = coax_at_u(i32(19i));
  l_x = (((((0x1.000000p1f * coax_at_u(i32(23i))) / l_wd) - 0x1.000000p0f) * l_z) + (((0x1.000000p1f * l_f) / l_wd) * l_r));
  l_y = (((0x1.000000p0f - ((0x1.000000p1f * coax_at_u(i32(27i))) / l_ht)) * l_z) + (((0x1.000000p1f * l_f) / l_ht) * l_q));
  return coax_p_v4(l_x, l_y, (0x1.000000p-1f * l_z), l_z);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_fract_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y));
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_wg_mul_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a * l_b.x), (l_a * l_b.y));
}
fn coax_p_wg_sub_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a - l_b.x), (l_a - l_b.y));
}
fn coax_p_egfx_hash2(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_render_noise2(l_p.x, l_p.y);
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_fxg_ground(p_px: vec2f) -> f32 {
  var l_px: vec2f = p_px;
  var l_f: f32;
  var l_dy: f32;
  l_f = coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f);
  l_dy = ((coax_at_u(i32(21i)) + ((coax_at_u(i32(25i)) * (l_px.x - coax_at_u(i32(23i)))) / l_f)) + ((coax_at_u(i32(29i)) * (coax_at_u(i32(27i)) - l_px.y)) / l_f));
  if ((l_dy > (-0x1.a36e2ep-14f)))
  {
    return 0x1.e84800p19f;
  }
  return ((-coax_at_u(i32(17i))) / l_dy);
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_render_noise2(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  if (((!coax_p_render_noise_coordinate_ok(l_x)) || (!coax_p_render_noise_coordinate_ok(l_y))))
  {
    return 0.0f;
  }
  return coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y)));
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_render_noise_coordinate_ok(p_x: f32) -> bool {
  var l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_noise_cell(p_x: f32) -> u32 {
  var l_x: f32 = p_x;
  var l_bounded: f32;
  l_bounded = coax_p_fmin(coax_p_fmax(l_x, (-0x1.000000p25f)), 0x1.fffffep24f);
  return u32(i32(floor((l_bounded * 0x1.000000p6f))));
}
fn coax_p_render_noise_mix2(p_x: u32, p_y: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_p_render_noise_permute(((l_x * 1597334677u) ^ (l_y * 3812015801u)));
}
fn coax_p_render_noise_unit(p_word: u32) -> f32 {
  var l_word: u32 = p_word;
  return (f32((l_word >> u32(8u))) * 0x1.000000p-24f);
}
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  var l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
}
@vertex
fn particle_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_11 {
  let coax_r = coax_p_particle_vs(coax_vid, coax_iid);
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@vertex
fn particle_over_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_11 {
  let coax_r = coax_p_particle_over_vs(coax_vid, coax_iid);
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@fragment
fn particle_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_particle_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
  return coax_r;
}
@fragment
fn particle_over_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_particle_over_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
  return coax_r;
}