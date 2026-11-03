// ---- generated: apps/openagents-web/gpu/newland.coax (the module) by coax wgsl; do not edit
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
struct coax_t15 {
  x: bool,
  y: i32,
  z: i32,
  w: f32,
}
struct coax_t16 {
  x: bool,
  y: i32,
  z: i32,
  w: i32,
  f4: i32,
  f5: f32,
  f6: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 28>;
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
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_o = coax_t11();
  l_ndc = coax_p_v2((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_o.x = coax_p_v4(l_ndc.x, l_ndc.y, 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_uv: vec2f;
  var l_t: f32;
  var l_w: vec2f;
  var l_n: f32;
  var l_glow: f32;
  var l_tint: vec3f;
  var l_c: vec3f;
  var l_star: f32;
  var l_q: vec2f;
  var l_r2: f32;
  var l_k: i32;
  l_uv = coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_mul_ff2(0x1.000000p-1f, coax_p_wg_at2(0i))), coax_at_u(i32(1i)));
  l_t = (coax_at_u(i32(2i)) * 0x1.0624dep-5f);
  l_w = coax_p_v2(coax_p_newland_fbm(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_uv, 0x1.800000p0f), coax_p_v2(l_t, (-l_t)))), coax_p_newland_fbm(coax_p_wg_add_f2f(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_uv, 0x1.800000p0f), coax_p_v2((-l_t), (l_t * 0x1.666666p-1f))), 0x1.000000p2f)));
  l_n = coax_p_newland_fbm(coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_uv, 0x1.0cccccp1f), coax_p_wg_mul_ff2(0x1.333334p1f, l_w)), coax_p_v2(0.0f, l_t)));
  l_glow = exp(((-0x1.e66666p0f) * coax_p_wg_dot2(l_uv, l_uv)));
  l_tint = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.70a3d8p-5f, 0x1.70a3d8p-5f, 0x1.70a3d8p-5f), coax_p_v3(0x1.7ae148p-3f, 0x1.7ae148p-3f, 0x1.7ae148p-3f), (l_n * l_n));
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_tint, l_glow), (0x1.99999ap-3f + (0x1.99999ap-1f * l_n))), 0x1.b33334p-1f);
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_NEWLAND_GREY_LIGHT(), 0x1.c28f5cp-5f), exp((-(((l_uv.y + 0x1.99999ap-5f) * 0x1.400000p2f) * ((l_uv.y + 0x1.99999ap-5f) * 0x1.400000p2f))))), l_glow));
  l_star = (coax_p_stepf(0x1.fef9dcp-1f, coax_p_newland_hash(coax_p_wg_floor_f2(coax_p_wg_mul_f2f(coax_p_v2(l_pos.x, l_pos.y), 0x1.000000p-1f)))) * (0x1.000000p-1f + (0x1.000000p-1f * sin(((coax_at_u(i32(2i)) * 0x1.4cccccp0f) + (coax_p_newland_hash(coax_p_wg_floor_f2(coax_p_v2(l_pos.x, l_pos.y))) * 0x1.400000p5f))))));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_make_f3_f(((l_star * 0x1.666666p-2f) * (0x1.000000p0f - (l_glow * 0x1.333334p-1f)))));
  l_q = coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_mul_ff2(0x1.000000p-1f, coax_p_wg_at2(0i))), (0x1.000000p-1f * coax_p_wg_length2(coax_p_wg_at2(0i))));
  l_r2 = ((l_q.x * l_q.x) + ((l_q.y * l_q.y) * 0x1.4cccccp0f));
  l_c = coax_p_wg_mul_f3f(l_c, coax_p_mix(0x1.000000p0f, coax_p_clampf((0x1.2e147ap0f - (0x1.400000p0f * l_r2)), 0.0f, 0x1.000000p0f), coax_at_u(i32(6i))));
  l_c = coax_p_wg_mul_f3f(l_c, coax_at_u(i32(3i)));
  {
    l_k = 0i;
    while ((l_k < 13i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      if ((l_k == 4i))
      {
        {
          l_k = (l_k + 1i);
          continue;
        }
      }
      l_c = coax_p_newland_panel(coax_p_v2(l_pos.x, l_pos.y), l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 8i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 8i)));
      l_k = (l_k + 1i);
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
  var l_mode: f32;
  var l_r: f32;
  var l_tail: f32;
  var l_g: f32;
  var l_hot: f32;
  var l_c: vec3f;
  var l_core: f32;
  var l_g_15: f32;
  var l_halo: f32;
  var l_ends: f32;
  var l_c_18: vec3f;
  if ((l_v.w.w > 0x1.c00000p1f))
  {
    l_lod = coax_p_clampf(coax_p_wg_log2((0x1.000000p6f / coax_p_fmax(l_v.z.x, 0x1.000000p0f))), 0.0f, 0x1.000000p2f);
    l_a = coax_pow_bg_vs((coax_p_render_sans_sample(l_v.y, l_lod)).w, 0x1.b33334p-1f);
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_a), coax_p_fmin((l_v.z.z * 0x1.266666p0f), 0x1.000000p0f)), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_mode = l_v.w.w;
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
    l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_g), coax_p_wg_mul_f3f(coax_p_NEWLAND_GREY_WHITE(), l_hot)), l_tail), l_v.z.z), coax_at_u(i32(3i)));
    return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
  }
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g_15 = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g_15) * l_g_15)) * 0x1.1eb852p-2f);
  if (((l_mode > 0x1.000000p-1f) && (l_mode < 0x1.800000p0f)))
  {
    l_ends = (coax_p_smoothstep(0.0f, 0x1.000000p3f, l_v.y.x) * coax_p_smoothstep(0.0f, 0x1.000000p3f, (l_v.z.x - l_v.y.x)));
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), ((l_core * 0x1.99999ap-1f) + l_halo)), l_ends), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  if ((l_mode > 0x1.400000p1f))
  {
    l_c_18 = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), coax_p_NEWLAND_GREY_WHITE(), 0x1.800000p-1f), l_core), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_halo), 0x1.ccccccp0f)), l_v.z.z), coax_at_u(i32(3i)));
    return coax_p_v4(l_c_18.x, l_c_18.y, l_c_18.z, 0x1.000000p0f);
  }
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
fn coax_p_wg_at2(p_at: i32) -> vec2f {
  var l_at: i32 = p_at;
  return coax_p_v2(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))));
}
fn coax_p_wg_mul_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a * l_b.x), (l_a * l_b.y));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
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
fn coax_p_newland_fbm(p_q: vec2f) -> f32 {
  var l_q: vec2f = p_q;
  var l_p: vec2f;
  var l_v: f32;
  var l_a: f32;
  var l_k: i32;
  l_p = l_q;
  l_v = 0.0f;
  l_a = 0x1.000000p-1f;
  {
    l_k = 0i;
    while ((l_k < 5i))
    {
      l_v = (l_v + (l_a * coax_p_newland_noise(l_p)));
      l_p = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.03d70ap1f), coax_p_v2(0x1.b33334p0f, 0x1.266666p3f));
      l_a = (l_a * 0x1.000000p-1f);
      l_k = (l_k + 1i);
    }
  }
  return l_v;
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_NEWLAND_GREY_LIGHT() -> vec3f {
  return coax_p_v3(0x1.8f5c28p-1f, 0x1.8f5c28p-1f, 0x1.8f5c28p-1f);
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_newland_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_render_noise2(l_p.x, l_p.y);
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
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_newland_panel(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_alpha: f32;
  var l_x: f32;
  var l_y: f32;
  var l_w: f32;
  var l_h: f32;
  var l_side: f32;
  var l_corner: f32;
  var l_inside: f32;
  var l_tint: vec3f;
  var l_warm: f32;
  var l_shade: vec3f;
  var l_line: f32;
  var l_edge: f32;
  var l_glass: vec3f;
  l_alpha = (l_s.y * coax_at_u(i32(3i)));
  if ((l_alpha <= 0.0f))
  {
    return l_under;
  }
  l_x = (l_p.x - l_r.x);
  l_y = (l_p.y - l_r.y);
  l_w = (l_r.z - l_r.x);
  l_h = (l_r.w - l_r.y);
  l_side = coax_p_fmin(coax_p_fmin(l_x, (l_w - l_x)), coax_p_fmin(l_y, (l_h - l_y)));
  l_corner = (coax_p_fmin(((l_w - l_x) + l_y), (l_x + (l_h - l_y))) - l_s.x);
  l_inside = coax_p_fmin(l_side, (l_corner * 0x1.6a0902p-1f));
  if ((l_inside <= 0.0f))
  {
    return l_under;
  }
  l_tint = coax_p_NEWLAND_GREY_LIGHT();
  l_warm = select(0.0f, 0x1.000000p0f, (l_s.z > 0x1.000000p-1f));
  l_shade = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.6872b0p-6f, 0x1.6872b0p-6f, 0x1.6872b0p-6f), coax_p_v3(0x1.a9fbe8p-7f, 0x1.a9fbe8p-7f, 0x1.a9fbe8p-7f), coax_p_clampf((l_y / coax_p_fmax(l_h, 0x1.000000p0f)), 0.0f, 0x1.000000p0f));
  l_line = select(0.0f, 0x1.26e978p-6f, (coax_p_fract((l_p.y / 0x1.800000p1f)) < 0x1.5c28f6p-2f));
  l_edge = ((exp(((-l_inside) / 0x1.600000p4f)) * (0x1.1eb852p-4f + (0x1.99999ap-4f * l_warm))) + (0x1.1eb852p-5f * l_warm));
  l_glass = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(l_under, 0x1.000000p-2f), l_shade, 0x1.b33334p-1f), coax_p_v3(l_line, l_line, l_line)), coax_p_wg_mul_f3f(l_tint, l_edge));
  return coax_p_wg_mix_f3f3f(l_under, l_glass, (coax_p_clampf(l_alpha, 0.0f, 0x1.000000p0f) * coax_p_smoothstep(0.0f, 0x1.800000p0f, l_inside)));
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
fn coax_p_render_sans_sample(p_uv: vec2f, p_lod: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_lod: f32 = p_lod;
  var l_plan: coax_t15;
  var l_low: coax_t16;
  var l_high: coax_t16;
  l_plan = coax_p_render_sans_filter_lod(i32(textureDimensions(sans_atlas).x), i32(textureDimensions(sans_atlas).y), l_lod);
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
fn coax_p_NEWLAND_GREY_WHITE() -> vec3f {
  return coax_p_v3(0x1.e147aep-1f, 0x1.e147aep-1f, 0x1.e147aep-1f);
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_newland_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_newland_hash(l_i), coax_p_newland_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_newland_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_newland_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
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
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_render_sans_filter_lod(p_width: i32, p_height: i32, p_lod: f32) -> coax_t15 {
  var l_width: i32 = p_width;
  var l_height: i32 = p_height;
  var l_lod: f32 = p_lod;
  var l_last: f32;
  var l_bounded: f32;
  var l_low: i32;
  var l_high: i32;
  if ((((l_width != coax_p_render_sans_filter_width(0i)) || (l_height != coax_p_render_sans_filter_height(0i))) || ((l_lod - l_lod) != 0.0f)))
  {
    return coax_t15(false, 0i, 0i, 0.0f);
  }
  l_last = 0x1.000000p2f;
  l_bounded = coax_p_clampf(l_lod, 0.0f, l_last);
  l_low = i32(floor(l_bounded));
  l_high = i32(coax_p_fmin(f32((l_low + 1i)), l_last));
  return coax_t15(true, l_low, l_high, (l_bounded - f32(l_low)));
}
fn coax_p_render_sans_filter_width(p_level: i32) -> i32 {
  var l_level: i32 = p_level;
  return (1536i >> u32(l_level));
}
fn coax_p_render_sans_filter_height(p_level: i32) -> i32 {
  var l_level: i32 = p_level;
  return (672i >> u32(l_level));
}
fn coax_p_render_sans_filter_cell(p_width: i32, p_height: i32, p_u: f32, p_v: f32) -> coax_t16 {
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
    return coax_t16(false, 0i, 0i, 0i, 0i, 0.0f, 0.0f);
  }
  l_fx = ((coax_p_clampf(l_u, 0.0f, 0x1.000000p0f) * f32(l_width)) - 0x1.000000p-1f);
  l_fy = ((coax_p_clampf(l_v, 0.0f, 0x1.000000p0f) * f32(l_height)) - 0x1.000000p-1f);
  l_x = i32(floor(l_fx));
  l_y = i32(floor(l_fy));
  return coax_t16(true, i32(coax_p_clamp_texel(l_x, i32(l_width))), i32(coax_p_clamp_texel(l_y, i32(l_height))), i32(coax_p_clamp_texel((l_x + 1i), i32(l_width))), i32(coax_p_clamp_texel((l_y + 1i), i32(l_height))), (l_fx - f32(l_x)), (l_fy - f32(l_y)));
}
fn coax_p_render_sans_filter_level(p_cell: coax_t16, p_level: i32) -> vec4f {
  var l_cell: coax_t16 = p_cell;
  var l_level: i32 = p_level;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  l_a = textureLoad(sans_atlas, vec2i(l_cell.y, l_cell.z), l_level);
  l_b = textureLoad(sans_atlas, vec2i(l_cell.w, l_cell.z), l_level);
  l_c = textureLoad(sans_atlas, vec2i(l_cell.y, l_cell.f4), l_level);
  l_d = textureLoad(sans_atlas, vec2i(l_cell.w, l_cell.f4), l_level);
  return coax_p_render_sans_filter_bilinear(l_a, l_b, l_c, l_d, l_cell.f5, l_cell.f6);
}
fn coax_p_render_sans_filter_interpolate(p_low: vec4f, p_high: vec4f, p_amount: f32) -> vec4f {
  var l_low: vec4f = p_low;
  var l_high: vec4f = p_high;
  var l_amount: f32 = p_amount;
  return coax_p_mix4(l_low, l_high, l_amount);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
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
fn coax_p_wg_sub_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a - l_b.x), (l_a - l_b.y));
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
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  var l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
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