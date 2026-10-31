// ---- generated: apps/openagents-web/gpu/builder.coax (the module) by coax wgsl; do not edit
struct coax_t9 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_9 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t11 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 34>;
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
  var l_d: vec4f;
  var l_o: coax_t11;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t11();
  l_ndc = coax_p_v2((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_o.x = coax_p_v4(l_ndc.x, l_ndc.y, 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_p: vec2f;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  return coax_p_wg_make_f4_f3f(coax_p_builder_panels(l_p, coax_p_wg_mul_f3f(coax_p_builder_field(l_p), coax_at_u(i32(3i)))), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t11) -> vec4f {
  var l_v: coax_t11 = p_v;
  var l_r: f32;
  var l_d: f32;
  var l_inside: f32;
  var l_t: f32;
  var l_lit: vec3f;
  var l_halo: f32;
  var l_lod: f32;
  var l_a: f32;
  var l_along: f32;
  var l_d_13: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo_16: f32;
  if ((l_v.w.w > 0x1.200000p2f))
  {
    l_r = l_v.z.y;
    l_d = coax_p_wg_length2(l_v.y);
    l_inside = (0x1.000000p0f - coax_p_smoothstep((l_r - 0x1.800000p-1f), (l_r + 0x1.800000p-1f), l_d));
    l_t = coax_p_clampf(((coax_p_wg_length2(coax_p_wg_add_f2f2(l_v.y, coax_p_wg_make_f2_f((0x1.666666p-2f * l_r)))) - (0x1.99999ap-4f * l_r)) / (0x1.ccccccp-1f * l_r)), 0.0f, 0x1.000000p0f);
    l_lit = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.353f7cp-1f, 0x1.2f1aa0p-1f, 0x1.472b02p-1f), coax_p_v3(0x1.e147aep-3f, 0x1.d0e560p-3f, 0x1.19999ap-2f), l_t);
    l_halo = ((0x1.eb851ep-6f * (0x1.000000p0f - coax_p_smoothstep((0x1.99999ap-1f * l_r), (0x1.000000p1f * l_r), l_d))) * (0x1.000000p0f - l_inside));
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_lit, l_inside), coax_p_wg_mul_f3f(coax_p_v3(0x1.dc28f6p-1f, 0x1.dc28f6p-1f, 0x1.dc28f6p-1f), l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  if ((l_v.w.w > 0x1.c00000p1f))
  {
    l_lod = coax_p_clampf(coax_p_wg_log2((0x1.000000p6f / coax_p_fmax(l_v.z.x, 0x1.000000p0f))), 0.0f, 0x1.000000p2f);
    l_a = coax_pow_bg_vs((textureSampleLevel(sans_atlas, sans_sampler, l_v.y, l_lod)).w, 0x1.b33334p-1f);
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_a), coax_p_fmin((l_v.z.z * 0x1.266666p0f), 0x1.000000p0f)), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d_13 = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d_13));
  l_g = (l_d_13 / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo_16 = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.1eb852p-2f);
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo_16)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
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
fn coax_p_builder_field(p_pos: vec2f) -> vec3f {
  var l_pos: vec2f = p_pos;
  var l_uv: vec2f;
  var l_t: f32;
  var l_w: vec2f;
  var l_n: f32;
  var l_glow: f32;
  var l_tint: vec3f;
  var l_c: vec3f;
  var l_px: vec2f;
  var l_star: f32;
  l_uv = coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(l_pos, coax_p_wg_mul_ff2(0x1.000000p-1f, coax_p_wg_at2(0i))), coax_at_u(i32(1i)));
  l_t = ((0x1.400000p4f + select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f))) * 0x1.1eb852p-5f);
  l_w = coax_p_v2(coax_p_builder_fbm(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_uv, 0x1.99999ap0f), coax_p_v2(l_t, (-l_t)))), coax_p_builder_fbm(coax_p_wg_add_f2f(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_uv, 0x1.99999ap0f), coax_p_v2((-l_t), (l_t * 0x1.666666p-1f))), 0x1.000000p2f)));
  l_n = coax_p_builder_fbm(coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_uv, 0x1.19999ap1f), coax_p_wg_mul_ff2(0x1.333334p1f, l_w)), coax_p_v2(0.0f, l_t)));
  l_glow = exp(((-0x1.19999ap1f) * coax_p_wg_dot2(l_uv, l_uv)));
  l_tint = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.99999ap-5f, 0x1.c28f5cp-5f, 0x1.333334p-4f), coax_p_v3(0x1.99999ap-3f, 0x1.ae147ap-3f, 0x1.147ae2p-2f), (l_n * l_n));
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_tint, l_glow), l_glow), (0x1.99999ap-3f + (0x1.99999ap-1f * l_n)));
  l_px = coax_p_wg_div_f2f(coax_p_wg_mul_f2f(l_pos, coax_p_fmin(0x1.000000p1f, coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f))), coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f));
  l_star = (coax_p_stepf(0x1.feb852p-1f, coax_p_builder_hash(coax_p_wg_floor_f2(coax_p_wg_mul_f2f(l_px, 0x1.000000p-1f)))) * (0x1.000000p-1f + (0x1.000000p-1f * sin((((0x1.400000p4f + coax_at_u(i32(2i))) * 0x1.4cccccp0f) + (coax_p_builder_hash(coax_p_wg_floor_f2(l_px)) * 0x1.400000p5f))))));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_make_f3_f(((l_star * 0x1.000000p-1f) * (0x1.000000p0f - (l_glow * 0x1.333334p-1f)))));
  return l_c;
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_builder_panels(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_k: i32;
  var l_s: vec4f;
  l_c = l_under;
  {
    l_k = 0i;
    while ((l_k < 16i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_s = coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 8i));
      if ((l_s.z >= 0x1.c00000p1f))
      {
        l_c = coax_p_builder_hud(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 8i)), l_s);
      }
      else
      {
        l_c = coax_p_builder_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 8i)), l_s);
      }
      l_k = (l_k + 1i);
    }
  }
  return l_c;
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(coax_p_dot2(l_a, l_a));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_make_f2_f(p_p0: f32) -> vec2f {
  var l_p0: f32 = p_p0;
  return coax_p_v2(l_p0, l_p0);
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_clampf(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return coax_p_fmin(coax_p_fmax(l_x, l_lo), l_hi);
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
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
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
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_b < l_a))
  {
    return l_b;
  }
  return l_a;
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
fn coax_p_builder_fbm(p_q: vec2f) -> f32 {
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
      l_v = (l_v + (l_a * coax_p_builder_noise(l_p)));
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
  return coax_p_dot2(l_a, l_b);
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_builder_hash(p_p: vec2f) -> f32 {
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
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_builder_hud(p_pos: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_pos: vec2f = p_pos;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_k: f32;
  var l_ext: vec2f;
  var l_q: vec2f;
  var l_press: f32;
  var l_hover: f32;
  var l_p: vec2f;
  var l_n: vec2f;
  var l_glass: vec3f;
  var l_edge: vec3f;
  var l_d_box: f32;
  var l_d: f32;
  var l_inside: f32;
  var l_rgb: vec3f;
  var l_col: vec3f;
  var l_alpha: f32;
  var l_t: f32;
  var l_phase: f32;
  var l_w: f32;
  var l_sweep: f32;
  var l_on_cut: f32;
  var l_e: f32;
  var l_off: f32;
  var l_arm: f32;
  var l_th: f32;
  var l_br: f32;
  var l_c: i32;
  var l_m: vec2f;
  var l_b: vec2f;
  var l_glow: f32;
  var l_opacity: f32;
  l_k = l_s.x;
  l_ext = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_r.z, l_r.w), coax_p_v2(l_r.x, l_r.y)), 0x1.000000p-1f);
  l_q = coax_p_wg_sub_f2f2(l_pos, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_ext));
  if (coax_p_wg_any_gt_f2f2(coax_p_wg_abs_f2(l_q), coax_p_wg_add_f2f2(l_ext, coax_p_wg_make_f2_f((0x1.400000p5f * l_k)))))
  {
    return l_under;
  }
  l_press = select(0.0f, 0x1.000000p0f, (l_s.z >= 0x1.600000p2f));
  l_hover = select(0.0f, 0x1.000000p0f, (l_s.z >= 0x1.200000p2f));
  l_p = coax_p_wg_div_f2f(l_q, (0x1.000000p0f - (0x1.eb851ep-6f * l_press)));
  l_n = coax_p_wg_div_f2f2(l_p, l_ext);
  l_glass = coax_p_wg_add_f3f(coax_p_v3(0x1.1eb852p-5f, 0x1.47ae14p-5f, 0x1.99999ap-5f), (0x1.99999ap-5f * (0x1.000000p-1f - (0x1.000000p-1f * l_n.y))));
  l_edge = coax_p_v3(0x1.d70a3ep-1f, 0x1.e66666p-1f, 0x1.000000p0f);
  l_d_box = coax_p_builder_sd_box(l_p, l_ext);
  l_d = coax_p_builder_sd_cut(l_p, l_ext, (0x1.c00000p3f * l_k));
  l_inside = coax_p_builder_cover(l_d);
  l_rgb = coax_p_wg_add_f3f(coax_p_wg_add_f3f(coax_p_wg_add_f3f(l_glass, (0x1.89374cp-7f * coax_p_stepf((0x1.000000p1f * l_k), (l_pos.y - ((0x1.800000p1f * l_k) * floor((l_pos.y / (0x1.800000p1f * l_k)))))))), (0x1.99999ap-4f * l_press)), (0x1.47ae14p-5f * l_hover));
  l_col = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_rgb, 0x1.99999ap-1f), l_inside);
  l_alpha = (0x1.99999ap-1f * l_inside);
  l_t = select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
  l_phase = coax_p_fract((l_t / 0x1.800000p2f));
  l_w = ((l_p.x + (l_p.y * 0x1.333334p-1f)) / l_ext.x);
  l_sweep = (exp((-(((l_w - coax_p_mix((-0x1.99999ap0f), 0x1.99999ap0f, (l_phase / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f) * ((l_w - coax_p_mix((-0x1.99999ap0f), 0x1.99999ap0f, (l_phase / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f)))) * coax_p_builder_band(l_phase, 0.0f, 0x1.c28f5cp-3f));
  l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-4f, 0x1.99999ap-4f), l_sweep), l_inside));
  l_on_cut = coax_p_stepf(l_d_box, (l_d - 0x1.000000p-2f));
  l_e = ((coax_p_builder_rim(l_d, (0x1.333334p0f * l_k)) * (0x1.ae147ap-2f + (0x1.99999ap-3f * l_hover))) + ((coax_p_builder_rim(l_d, (0x1.4cccccp1f * l_k)) * l_on_cut) * 0x1.e66666p-1f));
  l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(l_edge, l_e));
  l_alpha = (l_alpha + l_e);
  l_off = (0x1.800000p2f * l_k);
  l_arm = (0x1.000000p4f * l_k);
  l_th = (0x1.666666p0f * l_k);
  l_br = 0.0f;
  {
    l_c = 0i;
    while ((l_c < 2i))
    {
      l_m = select(coax_p_v2((-l_p.x), l_p.y), coax_p_v2(l_p.x, (-l_p.y)), (l_c == 0i));
      l_b = coax_p_wg_sub_f2f2(l_m, coax_p_wg_add_f2f2(l_ext, coax_p_v2(l_off, l_off)));
      l_br = coax_p_fmax(l_br, coax_p_fmax((coax_p_builder_band(l_b.x, (-l_arm), (l_th * 0x1.000000p-1f)) * coax_p_stepf(abs(l_b.y), (l_th * 0x1.000000p-1f))), (coax_p_builder_band(l_b.y, (-l_arm), (l_th * 0x1.000000p-1f)) * coax_p_stepf(abs(l_b.x), (l_th * 0x1.000000p-1f)))));
      l_c = (l_c + 1i);
    }
  }
  l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.b33334p-1f, 0x1.b33334p-1f, 0x1.b33334p-1f), l_br), 0x1.19999ap-1f));
  l_alpha = (l_alpha + (l_br * 0x1.19999ap-1f));
  l_alpha = coax_p_fmin(l_alpha, 0x1.000000p0f);
  l_col = coax_p_wg_min_f3f3(l_col, coax_p_v3(l_alpha, l_alpha, l_alpha));
  l_glow = ((exp(((-coax_p_fmax(l_d, 0.0f)) / (0x1.200000p3f * l_k))) * (0x1.000000p0f - l_inside)) * 0x1.99999ap-4f);
  l_opacity = (0x1.000000p0f - (0x1.ccccccp-2f * l_press));
  return coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(l_under, (0x1.000000p0f - l_alpha))), l_opacity), coax_p_wg_mul_f3f(l_under, (0x1.000000p0f - l_opacity))), coax_p_v3(l_glow, l_glow, l_glow));
}
fn coax_p_builder_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_half: vec2f;
  var l_corner: f32;
  var l_q: vec2f;
  var l_d: f32;
  var l_over: bool;
  var l_lit: bool;
  var l_fill: vec3f;
  var l_line: vec3f;
  var l_w: f32;
  var l_inside: f32;
  var l_edge: f32;
  var l_a: f32;
  if ((l_s.y <= 0.0f))
  {
    return l_under;
  }
  l_half = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_r.z, l_r.w), coax_p_v2(l_r.x, l_r.y)), 0x1.000000p-1f);
  l_corner = (0x1.000000p2f * l_s.x);
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_d = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  if ((l_d > 0x1.000000p0f))
  {
    return l_under;
  }
  l_over = (l_s.z >= 0x1.000000p1f);
  l_lit = (l_over || ((l_s.z >= 0x1.000000p0f) && (l_s.z < 0x1.000000p1f)));
  l_fill = select(coax_p_v3(0x1.51eb86p-3f, 0x1.d2f1aap-4f, 0x1.3f7ceep-4f), coax_p_v3(0x1.e147aep-3f, 0x1.581062p-3f, 0x1.d70a3ep-4f), l_over);
  l_line = select(coax_p_v3(0x1.6147aep-1f, 0x1.14fdf4p-1f, 0x1.28f5c2p-2f), coax_p_v3(0x1.b851ecp-1f, 0x1.666666p-1f, 0x1.ae147ap-2f), l_lit);
  l_w = (0x1.400000p0f * l_s.x);
  l_inside = coax_p_clampf((0x1.000000p-1f - l_d), 0.0f, 0x1.000000p0f);
  l_edge = coax_p_clampf((((l_w * 0x1.000000p-1f) + 0x1.000000p-1f) - abs((l_d + (l_w * 0x1.000000p-1f)))), 0.0f, 0x1.000000p0f);
  l_a = coax_p_clampf(l_s.y, 0.0f, 0x1.000000p0f);
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_under, l_fill, (l_a * l_inside)), l_line, (l_edge * coax_p_fmin(0x1.000000p0f, (l_a + 0x1.99999ap-4f))));
}
fn coax_p_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x * l_b.x) + (l_a.y * l_b.y));
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
fn coax_p_builder_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_builder_hash(l_i), coax_p_builder_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_builder_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_builder_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_wg_abs_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(abs(l_p0.x), abs(l_p0.y));
}
fn coax_p_wg_any_gt_f2f2(p_a: vec2f, p_b: vec2f) -> bool {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x > l_b.x) || (l_a.y > l_b.y));
}
fn coax_p_wg_div_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x / l_b.x), (l_a.y / l_b.y));
}
fn coax_p_wg_add_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x + l_b), (l_a.y + l_b), (l_a.z + l_b));
}
fn coax_p_builder_sd_box(p_p: vec2f, p_b: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_b: vec2f = p_b;
  var l_q: vec2f;
  l_q = coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(l_p), l_b);
  return (coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f));
}
fn coax_p_builder_sd_cut(p_p: vec2f, p_b: vec2f, p_cut: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_b: vec2f = p_b;
  var l_cut: f32 = p_cut;
  return coax_p_fmax(coax_p_builder_sd_box(l_p, l_b), ((abs((l_p.x + l_p.y)) - ((l_b.x + l_b.y) - l_cut)) * 0x1.6a09e6p-1f));
}
fn coax_p_builder_cover(p_d: f32) -> f32 {
  var l_d: f32 = p_d;
  return (0x1.000000p0f - coax_p_smoothstep((-0x1.000000p-1f), 0x1.000000p-1f, l_d));
}
fn coax_p_builder_band(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return (coax_p_stepf(l_lo, l_x) * coax_p_stepf(l_x, l_hi));
}
fn coax_p_builder_rim(p_d: f32, p_w: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_w: f32 = p_w;
  return (0x1.000000p0f - coax_p_smoothstep(((l_w * 0x1.000000p-1f) - 0x1.000000p-1f), ((l_w * 0x1.000000p-1f) + 0x1.000000p-1f), abs((l_d + (l_w * 0x1.000000p-1f)))));
}
fn coax_p_wg_min_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmin(l_p0.x, l_p1.x), coax_p_fmin(l_p0.y, l_p1.y), coax_p_fmin(l_p0.z, l_p1.z));
}
fn coax_p_wg_max_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v2(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y));
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
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_9) -> coax_io_11 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t9(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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