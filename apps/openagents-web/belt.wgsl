// ---- generated: apps/openagents-web/gpu/belt.coax (the module) by coax wgsl; do not edit
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 44>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
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
  var l_stars: f32;
  var l_c: vec3f;
  var l_d: f32;
  var l_disc: f32;
  var l_corona: f32;
  var l_worlds: i32;
  var l_k: i32;
  var l_n: i32;
  var l_k_10: i32;
  l_stars = coax_p_verse_starfield(l_pos.x, l_pos.y, coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(2i)), coax_at_u(i32(7i)));
  l_c = coax_p_wg_add_f3f3(coax_p_v3(0x1.89374cp-8f, 0x1.0624dep-7f, 0x1.cac084p-7f), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.b33334p-1f, 0x1.ccccccp-1f, 0x1.000000p0f), l_stars), 0x1.99999ap-1f));
  l_d = coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_pos.x, l_pos.y), coax_p_v2(coax_at_u(i32(4i)), coax_at_u(i32(5i)))));
  l_disc = coax_p_verse_sun_disc(l_d, coax_at_u(i32(6i)));
  l_corona = coax_p_verse_sun_corona(l_d, coax_at_u(i32(6i)), coax_at_u(i32(7i)));
  l_c = coax_p_wg_mix_f3f3f(coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_BELT_SUN_GLOW(), l_corona), 0x1.ccccccp-1f)), coax_p_BELT_SUN_CORE(), l_disc);
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_BELT_DUST(), coax_p_belt_belt_dust(coax_p_v2(l_pos.x, l_pos.y))));
  l_worlds = coax_p_wg_i32_f(coax_at_u(i32(11i)));
  {
    l_k = 0i;
    while ((l_k < 12i))
    {
      if ((l_k >= l_worlds))
      {
        break;
      }
      l_c = coax_p_belt_world(coax_p_v2(l_pos.x, l_pos.y), l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 80i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 80i)));
      l_k = (l_k + 1i);
    }
  }
  l_c = coax_p_wg_mul_f3f(l_c, coax_at_u(i32(3i)));
  l_n = coax_p_wg_i32_f(coax_at_u(i32(10i)));
  {
    l_k_10 = 0i;
    while ((l_k_10 < 8i))
    {
      if ((l_k_10 >= l_n))
      {
        break;
      }
      l_c = coax_p_belt_panel(coax_p_v2(l_pos.x, l_pos.y), l_c, coax_p_wg_at4(((i32((l_k_10 * 2i)) * 4i) + 12i)), coax_p_wg_at4(((i32(((l_k_10 * 2i) + 1i)) * 4i) + 12i)));
      l_k_10 = (l_k_10 + 1i);
    }
  }
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t11) -> vec4f {
  var l_v: coax_t11 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  var l_dim: f32;
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.1eb852p-2f);
  l_dim = select(0x1.000000p0f, coax_p_belt_under_hud(coax_p_v2(l_v.x.x, l_v.x.y)), (l_v.w.w > 0x1.000000p-1f));
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), l_dim), 0x1.000000p0f);
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
fn coax_p_verse_starfield(p_x: f32, p_y: f32, p_drift_x: f32, p_drift_y: f32, p_t: f32, p_dpr: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_drift_x: f32 = p_drift_x;
  var l_drift_y: f32 = p_drift_y;
  var l_t: f32 = p_t;
  var l_dpr: f32 = p_dpr;
  var l_k: f32;
  var l_far: f32;
  var l_mid: f32;
  var l_near: f32;
  l_k = coax_p_stars_max(l_dpr, 0x1.000000p-1f);
  l_far = (coax_p_verse_star_layer((l_x + (l_drift_x * 0x1.47ae14p-6f)), (l_y + (l_drift_y * 0x1.47ae14p-6f)), (0x1.200000p3f * l_k), 0x1.eb851ep-4f, (0x1.19999ap-1f * l_k), l_t, 0x1.000000p0f) * 0x1.19999ap-1f);
  l_mid = (coax_p_verse_star_layer((l_x + (l_drift_x * 0x1.99999ap-5f)), (l_y + (l_drift_y * 0x1.99999ap-5f)), (0x1.700000p4f * l_k), 0x1.99999ap-4f, (0x1.99999ap-1f * l_k), l_t, 0x1.c00000p2f) * 0x1.b33334p-1f);
  l_near = (coax_p_verse_star_layer((l_x + (l_drift_x * 0x1.99999ap-4f)), (l_y + (l_drift_y * 0x1.99999ap-4f)), (0x1.e80000p5f * l_k), 0x1.47ae14p-4f, (0x1.19999ap0f * l_k), l_t, 0x1.a00000p3f) * 0x1.333334p0f);
  return ((l_far + l_mid) + l_near);
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
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(coax_p_dot2(l_a, l_a));
}
fn coax_p_verse_sun_disc(p_d: f32, p_r: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_r: f32 = p_r;
  var l_rr: f32;
  var l_x: f32;
  l_rr = coax_p_stars_max(l_r, 0x1.800000p-1f);
  l_x = ((l_d - l_rr) / coax_p_stars_max((l_rr * 0x1.47ae14p-4f), 0x1.800000p-1f));
  if ((l_x <= (-0x1.000000p0f)))
  {
    return 0x1.000000p0f;
  }
  if ((l_x >= 0x1.000000p0f))
  {
    return 0.0f;
  }
  return (0x1.000000p-1f - (0x1.000000p-1f * l_x));
}
fn coax_p_verse_sun_corona(p_d: f32, p_r: f32, p_dpr: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_r: f32 = p_r;
  var l_dpr: f32 = p_dpr;
  var l_k: f32;
  var l_rr: f32;
  var l_out: f32;
  var l_corona: f32;
  var l_halo: f32;
  var l_glare: f32;
  l_k = coax_p_stars_max(l_dpr, 0x1.000000p-1f);
  l_rr = coax_p_stars_max(l_r, 0x1.800000p-1f);
  l_out = coax_p_stars_max((l_d - l_rr), 0.0f);
  l_corona = (exp(((-l_out) / ((l_rr * 0x1.333334p-1f) + (0x1.800000p1f * l_k)))) * 0x1.ccccccp-1f);
  l_halo = (exp(((-l_out) / ((l_rr * 0x1.800000p1f) + (0x1.400000p5f * l_k)))) * 0x1.c28f5cp-3f);
  l_glare = (0x1.eb851ep-5f / (0x1.000000p0f + (l_out / (0x1.e00000p6f * l_k))));
  return coax_p_stars_min(((l_corona + l_halo) + l_glare), 0x1.800000p0f);
}
fn coax_p_BELT_SUN_GLOW() -> vec3f {
  return coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.51eb86p-1f);
}
fn coax_p_BELT_SUN_CORE() -> vec3f {
  return coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.ccccccp-1f);
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_BELT_DUST() -> vec3f {
  return coax_p_v3(0x1.333334p-1f, 0x1.333334p-1f, 0x1.3d70a4p-1f);
}
fn coax_p_belt_belt_dust(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_au: f32;
  var l_rel: vec2f;
  var l_r: f32;
  var l_prof: f32;
  var l_g1: f32;
  var l_g2: f32;
  var l_g3: f32;
  var l_gaps: f32;
  var l_c: f32;
  var l_s: f32;
  var l_w: vec2f;
  var l_n: f32;
  var l_fade: f32;
  l_au = coax_at_u(i32(78i));
  if ((l_au <= 0.0f))
  {
    return 0.0f;
  }
  l_rel = coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(l_p, coax_p_v2(coax_at_u(i32(76i)), coax_at_u(i32(77i)))), l_au);
  l_r = coax_p_wg_length2(l_rel);
  if (((l_r < 0x1.000000p1f) || (l_r > 0x1.c00000p1f)))
  {
    return 0.0f;
  }
  l_prof = (coax_p_smoothstep(0x1.07ae14p1f, 0x1.333334p1f, l_r) * (0x1.000000p0f - coax_p_smoothstep(0x1.79999ap1f, 0x1.b33334p1f, l_r)));
  l_g1 = ((l_r - 0x1.400000p1f) / 0x1.6872b0p-6f);
  l_g2 = ((l_r - 0x1.68f5c2p1f) / 0x1.26e978p-6f);
  l_g3 = ((l_r - 0x1.79999ap1f) / 0x1.0624dep-6f);
  l_gaps = (((0x1.000000p0f - (0x1.333334p-1f * exp(((-l_g1) * l_g1)))) - (0x1.000000p-1f * exp(((-l_g2) * l_g2)))) - (0x1.99999ap-2f * exp(((-l_g3) * l_g3))));
  l_c = cos(coax_at_u(i32(79i)));
  l_s = sin(coax_at_u(i32(79i)));
  l_w = coax_p_v2(((l_c * l_rel.x) - (l_s * l_rel.y)), ((l_s * l_rel.x) + (l_c * l_rel.y)));
  l_n = ((coax_p_belt_fbm(coax_p_wg_mul_f2f(l_w, 0x1.c00000p2f)) * 0x1.4cccccp-1f) + (coax_p_belt_fbm(coax_p_wg_mul_f2f(l_w, 0x1.f00000p4f)) * 0x1.666666p-2f));
  l_fade = (0x1.000000p0f - coax_p_smoothstep(0x1.388000p14f, 0x1.d4c000p15f, l_au));
  return ((((l_prof * l_gaps) * (0x1.eb851ep-4f + (((0x1.19999ap0f * l_n) * l_n) * l_n))) * 0x1.47ae14p-3f) * l_fade);
}
fn coax_p_wg_i32_f(p_x: f32) -> i32 {
  var l_x: f32 = p_x;
  if ((l_x != l_x))
  {
    return 0i;
  }
  if ((l_x <= (-0x1.000000p31f)))
  {
    return (-2147483647i - 1i);
  }
  if ((l_x >= 0x1.fffffep30f))
  {
    return 2147483520i;
  }
  return i32(l_x);
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_belt_world(p_p: vec2f, p_under: vec3f, p_a: vec4f, p_b: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  var l_rad: f32;
  var l_rel: vec2f;
  var l_d: f32;
  var l_kind: i32;
  var l_q: vec2f;
  var l_l2: vec2f;
  var l_l: vec3f;
  var l_aa: f32;
  var l_c: vec3f;
  var l_ring: vec4f;
  var l_ring_front: bool;
  var l_ca: f32;
  var l_sa: f32;
  var l_edge: f32;
  var l_dir: vec2f;
  var l_lump: f32;
  var l_qe: f32;
  var l_atm: vec4f;
  var l_lit: f32;
  var l_glow: f32;
  var l_cover: f32;
  var l_qq: vec2f;
  var l_z: f32;
  var l_n: vec3f;
  var l_ln: vec3f;
  var l_lam: f32;
  var l_col: vec3f;
  var l_light: f32;
  var l_lit_c: vec3f;
  l_rad = coax_p_fmax(l_a.z, 0x1.000000p-1f);
  l_rel = coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_a.x, l_a.y));
  l_d = coax_p_wg_length2(l_rel);
  if ((l_d > ((l_rad * 0x1.333334p1f) + 0x1.800000p2f)))
  {
    return l_under;
  }
  l_kind = coax_p_wg_i32_f((l_a.w + 0x1.000000p-1f));
  l_q = coax_p_wg_div_f2f(l_rel, l_rad);
  l_l2 = coax_p_wg_normalize2(coax_p_wg_add_f2f2(coax_p_v2(l_b.x, l_b.y), coax_p_v2(0x1.a36e2ep-14f, 0.0f)));
  l_l = coax_p_wg_normalize3(coax_p_v3(l_l2.x, l_l2.y, 0x1.666666p-2f));
  l_aa = (0x1.333334p0f / l_rad);
  l_c = l_under;
  l_ring = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  l_ring_front = false;
  if ((l_kind == 7i))
  {
    l_ring = coax_p_belt_rings(l_q, l_l2);
    l_ca = 0x1.e147aep-1f;
    l_sa = (-0x1.5e3540p-2f);
    l_ring_front = (((l_sa * l_q.x) + (l_ca * l_q.y)) > 0.0f);
    if ((!l_ring_front))
    {
      l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(l_ring.x, l_ring.y, l_ring.z), (l_ring.w * 0x1.b33334p-1f));
    }
  }
  l_edge = 0x1.000000p0f;
  if ((l_kind >= 8i))
  {
    l_dir = coax_p_wg_div_f2f(l_q, coax_p_fmax(coax_p_wg_length2(l_q), 0x1.a36e2ep-14f));
    l_lump = select(0x1.eb851ep-5f, 0x1.333334p-2f, (l_kind == 10i));
    l_edge = (0x1.000000p0f + ((l_lump * (coax_p_belt_fbm(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_dir, 0x1.b33334p0f), coax_p_v2((l_b.z * 0x1.c00000p2f), (l_b.z * 0x1.800000p1f)))) - 0x1.000000p-1f)) * 0x1.000000p1f));
  }
  l_qe = ((l_d / l_rad) / l_edge);
  l_atm = coax_p_belt_air(l_kind);
  if (((l_atm.w > 0.0f) && (l_qe > 0x1.000000p0f)))
  {
    l_lit = (0x1.000000p-2f + (0x1.800000p-1f * coax_p_fmax(coax_p_wg_dot2(coax_p_wg_div_f2f(l_q, coax_p_fmax(coax_p_wg_length2(l_q), 0x1.a36e2ep-14f)), l_l2), 0.0f)));
    l_glow = ((exp((((-(l_qe - 0x1.000000p0f)) * l_rad) / ((0x1.eb851ep-5f * l_rad) + 0x1.800000p0f))) * l_atm.w) * l_lit);
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_atm.x, l_atm.y, l_atm.z), l_glow), 0x1.99999ap-1f));
  }
  l_cover = (0x1.000000p0f - coax_p_smoothstep((0x1.000000p0f - l_aa), (0x1.000000p0f + l_aa), l_qe));
  if ((l_cover > 0.0f))
  {
    l_qq = coax_p_wg_div_f2f(l_q, l_edge);
    l_z = sqrt(coax_p_fmax((0x1.000000p0f - coax_p_wg_dot2(l_qq, l_qq)), 0.0f));
    l_n = coax_p_v3(l_qq.x, (-l_qq.y), l_z);
    l_ln = coax_p_v3(l_l.x, (-l_l.y), l_l.z);
    l_lam = coax_p_fmax(coax_p_wg_dot3(l_n, l_ln), 0.0f);
    l_col = coax_p_belt_surface(l_kind, l_n, l_b.z, l_b.w);
    l_light = (0x1.1eb852p-5f + (0x1.266666p0f * coax_pow_bg_vs(l_lam, 0x1.b33334p-1f)));
    if (((l_kind == 6i) || (l_kind == 7i)))
    {
      l_light = (l_light * (0x1.4cccccp-1f + (0x1.666666p-2f * l_z)));
    }
    l_lit_c = coax_p_wg_mul_f3f(l_col, l_light);
    if ((l_atm.w > 0.0f))
    {
      l_lit_c = coax_p_wg_add_f3f3(l_lit_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_atm.x, l_atm.y, l_atm.z), coax_pow_bg_vs((0x1.000000p0f - l_z), 0x1.800000p1f)), l_atm.w), 0x1.333334p-1f), coax_p_smoothstep((-0x1.99999ap-3f), 0x1.99999ap-2f, coax_p_wg_dot3(l_n, l_ln))));
    }
    l_c = coax_p_wg_mix_f3f3f(l_c, l_lit_c, l_cover);
  }
  if (((l_kind == 7i) && l_ring_front))
  {
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(l_ring.x, l_ring.y, l_ring.z), (l_ring.w * 0x1.b33334p-1f));
  }
  return l_c;
}
fn coax_p_belt_panel(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
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
  var l_warm: f32;
  var l_tint: vec3f;
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
  l_warm = select(0.0f, 0x1.000000p0f, (l_s.z > 0x1.000000p-1f));
  l_tint = coax_p_wg_mix_f3f3f(coax_p_BELT_GREY_LIGHT(), coax_p_BELT_ACCENT(), l_warm);
  l_shade = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.6872b0p-6f, 0x1.6872b0p-6f, 0x1.6872b0p-6f), coax_p_v3(0x1.a9fbe8p-7f, 0x1.a9fbe8p-7f, 0x1.a9fbe8p-7f), coax_p_clampf((l_y / coax_p_fmax(l_h, 0x1.000000p0f)), 0.0f, 0x1.000000p0f));
  l_line = select(0.0f, 0x1.26e978p-6f, (coax_p_fract((l_p.y / 0x1.800000p1f)) < 0x1.5c28f6p-2f));
  l_edge = ((exp(((-l_inside) / 0x1.600000p4f)) * (0x1.1eb852p-4f + (0x1.99999ap-4f * l_warm))) + (0x1.eb851ep-6f * l_warm));
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
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_belt_under_hud(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_n: i32;
  var l_k: i32;
  var l_r: vec4f;
  l_n = coax_p_wg_i32_f(coax_at_u(i32(10i)));
  {
    l_k = 0i;
    while ((l_k < 8i))
    {
      if ((l_k >= l_n))
      {
        break;
      }
      l_r = coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 12i));
      if (((((l_p.x >= l_r.x) && (l_p.x <= l_r.z)) && (l_p.y >= l_r.y)) && (l_p.y <= l_r.w)))
      {
        return 0x1.eb851ep-4f;
      }
      l_k = (l_k + 1i);
    }
  }
  return 0x1.000000p0f;
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_stars_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_verse_star_layer(p_x: f32, p_y: f32, p_cell: f32, p_density: f32, p_size: f32, p_t: f32, p_seed: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_cell: f32 = p_cell;
  var l_density: f32 = p_density;
  var l_size: f32 = p_size;
  var l_t: f32 = p_t;
  var l_seed: f32 = p_seed;
  var l_cx: f32;
  var l_cy: f32;
  var l_pick: f32;
  var l_ox: f32;
  var l_oy: f32;
  var l_dx: f32;
  var l_dy: f32;
  var l_d2: f32;
  var l_bright: f32;
  var l_twinkle: f32;
  l_cx = floor((l_x / l_cell));
  l_cy = floor((l_y / l_cell));
  l_pick = coax_p_stars_hash((l_cx + l_seed), (l_cy - (l_seed * 0x1.7ae148p-2f)));
  if ((l_pick < (0x1.000000p0f - l_density)))
  {
    return 0.0f;
  }
  l_ox = ((0x1.99999ap-3f + (0x1.333334p-1f * coax_p_stars_hash(((l_cx * 0x1.b33334p0f) + l_seed), (l_cy + 0x1.8cccccp1f)))) * l_cell);
  l_oy = ((0x1.99999ap-3f + (0x1.333334p-1f * coax_p_stars_hash((l_cx - 0x1.266666p1f), ((l_cy * 0x1.4cccccp0f) + l_seed)))) * l_cell);
  l_dx = (l_x - ((l_cx * l_cell) + l_ox));
  l_dy = (l_y - ((l_cy * l_cell) + l_oy));
  l_d2 = (((l_dx * l_dx) + (l_dy * l_dy)) / (l_size * l_size));
  l_bright = (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_stars_hash((l_cx + 0x1.600000p3f), (l_cy + (l_seed * 0x1.000000p1f)))));
  l_twinkle = (0x1.99999ap-1f + (0x1.99999ap-3f * sin(((l_t * (0x1.666666p-1f + (0x1.000000p1f * l_pick))) + (l_pick * 0x1.400000p5f)))));
  return ((exp((-l_d2)) * l_bright) * l_twinkle);
}
fn coax_p_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x * l_b.x) + (l_a.y * l_b.y));
}
fn coax_p_stars_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
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
fn coax_p_belt_fbm(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_v: f32;
  var l_a: f32;
  var l_q: vec2f;
  var l_i: i32;
  l_v = 0.0f;
  l_a = 0x1.000000p-1f;
  l_q = l_p;
  {
    l_i = 0i;
    while ((l_i < 3i))
    {
      l_v = (l_v + (l_a * coax_p_belt_vnoise(l_q)));
      l_q = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.08f5c2p1f), coax_p_v2(0x1.b33334p0f, 0x1.266666p3f));
      l_a = (l_a * 0x1.000000p-1f);
      l_i = (l_i + 1i);
    }
  }
  return (l_v / 0x1.c00000p-1f);
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_normalize2(p_a: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  return coax_p_scale2(l_a, (0x1.000000p0f / coax_p_wg_length2(l_a)));
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_belt_rings(p_q: vec2f, p_l: vec2f) -> vec4f {
  var l_q: vec2f = p_q;
  var l_l: vec2f = p_l;
  var l_ca: f32;
  var l_sa: f32;
  var l_r2: vec2f;
  var l_e: vec2f;
  var l_rr: f32;
  var l_dens: f32;
  var l_lr: vec2f;
  var l_lp: vec2f;
  var l_along: f32;
  var l_side: f32;
  var l_shade: f32;
  l_ca = 0x1.e147aep-1f;
  l_sa = (-0x1.5e3540p-2f);
  l_r2 = coax_p_v2(((l_ca * l_q.x) - (l_sa * l_q.y)), ((l_sa * l_q.x) + (l_ca * l_q.y)));
  l_e = coax_p_v2(l_r2.x, (l_r2.y / 0x1.ae147ap-2f));
  l_rr = coax_p_wg_length2(l_e);
  if (((l_rr < 0x1.3d70a4p0f) || (l_rr > 0x1.228f5cp1f)))
  {
    return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  }
  l_dens = ((0x1.19999ap-1f + (0x1.000000p-2f * sin((l_rr * 0x1.700000p5f)))) + (0x1.99999ap-3f * sin((l_rr * 0x1.060000p7f))));
  l_dens = (l_dens * (0x1.000000p0f - (0x1.d70a3ep-1f * (coax_p_smoothstep(0x1.e66666p0f, 0x1.ee147ap0f, l_rr) - coax_p_smoothstep(0x1.fae148p0f, 0x1.0147aep1f, l_rr)))));
  l_dens = (l_dens * (coax_p_smoothstep(0x1.3d70a4p0f, 0x1.51eb86p0f, l_rr) * (0x1.000000p0f - coax_p_smoothstep(0x1.170a3ep1f, 0x1.228f5cp1f, l_rr))));
  l_dens = (l_dens * select(0x1.19999ap-1f, 0x1.000000p0f, (l_rr > 0x1.8cccccp0f)));
  l_lr = coax_p_v2(((l_ca * l_l.x) - (l_sa * l_l.y)), ((l_sa * l_l.x) + (l_ca * l_l.y)));
  l_lp = coax_p_wg_normalize2(coax_p_v2(l_lr.x, (l_lr.y / 0x1.ae147ap-2f)));
  l_along = coax_p_wg_dot2(l_e, l_lp);
  l_side = coax_p_wg_length2(coax_p_wg_sub_f2f2(l_e, coax_p_wg_mul_ff2(l_along, l_lp)));
  l_shade = (0x1.000000p0f - ((0x1.c28f5cp-1f * (0x1.000000p0f - coax_p_smoothstep(0x1.e66666p-1f, 0x1.0cccccp0f, l_side))) * coax_p_smoothstep(0.0f, (-0x1.99999ap-4f), l_along)));
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.b851ecp-1f, 0x1.99999ap-1f, 0x1.51eb86p-1f), l_shade), coax_p_clampf(l_dens, 0.0f, 0x1.000000p0f));
}
fn coax_p_belt_air(p_kind: i32) -> vec4f {
  var l_kind: i32 = p_kind;
  if ((l_kind == 2i))
  {
    return coax_p_v4(0x1.000000p0f, 0x1.e66666p-1f, 0x1.a3d70ap-1f, 0x1.19999ap-1f);
  }
  if ((l_kind == 3i))
  {
    return coax_p_v4(0x1.666666p-2f, 0x1.3d70a4p-1f, 0x1.000000p0f, 0x1.b33334p-1f);
  }
  if ((l_kind == 5i))
  {
    return coax_p_v4(0x1.e66666p-1f, 0x1.3d70a4p-1f, 0x1.19999ap-1f, 0x1.333334p-2f);
  }
  if ((l_kind == 6i))
  {
    return coax_p_v4(0x1.ccccccp-1f, 0x1.b851ecp-1f, 0x1.8f5c28p-1f, 0x1.99999ap-3f);
  }
  if ((l_kind == 7i))
  {
    return coax_p_v4(0x1.e66666p-1f, 0x1.ccccccp-1f, 0x1.800000p-1f, 0x1.70a3d8p-3f);
  }
  return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_dot2(l_a, l_b);
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_dot3(l_a, l_b);
}
fn coax_p_belt_surface(p_kind: i32, p_n: vec3f, p_seed: f32, p_spin: f32) -> vec3f {
  var l_kind: i32 = p_kind;
  var l_n: vec3f = p_n;
  var l_seed: f32 = p_seed;
  var l_spin: f32 = p_spin;
  var l_lon: f32;
  var l_uv: vec2f;
  var l_land: f32;
  var l_col: vec3f;
  var l_cap: f32;
  var l_cloud: f32;
  var l_dark: f32;
  var l_col_12: vec3f;
  var l_wob: f32;
  var l_band: f32;
  var l_crater: f32;
  l_lon = (coax_p_wg_atan2(l_n.x, l_n.z) + l_spin);
  l_uv = coax_p_wg_add_f2f2(coax_p_v2(l_lon, (l_n.y * 0x1.99999ap0f)), coax_p_v2((l_seed * 0x1.8cccccp1f), l_seed));
  if ((l_kind == 1i))
  {
    return coax_p_wg_mul_f3f(coax_p_v3(0x1.1eb852p-1f, 0x1.0a3d70p-1f, 0x1.f5c290p-2f), (0x1.666666p-1f + (0x1.19999ap-1f * coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.000000p2f)))));
  }
  if ((l_kind == 2i))
  {
    return coax_p_wg_mix_f3f3f(coax_p_v3(0x1.dc28f6p-1f, 0x1.bd70a4p-1f, 0x1.666666p-1f), coax_p_v3(0x1.f5c290p-1f, 0x1.e66666p-1f, 0x1.b851ecp-1f), coax_p_belt_fbm(coax_p_wg_mul_f2f2(l_uv, coax_p_v2(0x1.800000p0f, 0x1.000000p2f))));
  }
  if ((l_kind == 3i))
  {
    l_land = coax_p_smoothstep(0x1.0a3d70p-1f, 0x1.28f5c2p-1f, coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.19999ap1f)));
    l_col = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.99999ap-5f, 0x1.47ae14p-3f, 0x1.ae147ap-2f), coax_p_wg_mix_f3f3f(coax_p_v3(0x1.99999ap-3f, 0x1.851eb8p-2f, 0x1.5c28f6p-3f), coax_p_v3(0x1.19999ap-1f, 0x1.eb851ep-2f, 0x1.5c28f6p-2f), coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.400000p2f))), l_land);
    l_cap = coax_p_smoothstep(0x1.a3d70ap-1f, 0x1.ccccccp-1f, abs(l_n.y));
    l_cloud = coax_p_smoothstep(0x1.000000p-1f, 0x1.70a3d8p-1f, coax_p_belt_fbm(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f2(l_uv, coax_p_v2(0x1.800000p1f, 0x1.800000p2f)), coax_p_v2((coax_at_u(i32(2i)) * 0x1.47ae14p-7f), 0.0f))));
    return coax_p_wg_mix_f3f3f(l_col, coax_p_v3(0x1.e66666p-1f, 0x1.e66666p-1f, 0x1.e66666p-1f), coax_p_fmax(l_cap, (l_cloud * 0x1.b33334p-1f)));
  }
  if ((l_kind == 4i))
  {
    return coax_p_wg_mul_f3f(coax_p_v3(0x1.3d70a4p-1f, 0x1.3d70a4p-1f, 0x1.333334p-1f), (0x1.4cccccp-1f + (0x1.000000p-1f * coax_p_smoothstep(0x1.666666p-2f, 0x1.333334p-1f, coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.800000p1f))))));
  }
  if ((l_kind == 5i))
  {
    l_dark = coax_p_smoothstep(0x1.ccccccp-2f, 0x1.4cccccp-1f, coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.800000p1f)));
    l_col_12 = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.70a3d8p-1f, 0x1.5c28f6p-2f, 0x1.000000p-2f), coax_p_v3(0x1.ae147ap-2f, 0x1.c28f5cp-3f, 0x1.5c28f6p-3f), l_dark);
    return coax_p_wg_mix_f3f3f(l_col_12, coax_p_v3(0x1.dc28f6p-1f, 0x1.d70a3ep-1f, 0x1.ccccccp-1f), coax_p_smoothstep(0x1.b851ecp-1f, 0x1.dc28f6p-1f, abs(l_n.y)));
  }
  if (((l_kind == 6i) || (l_kind == 7i)))
  {
    l_wob = (coax_p_belt_fbm(coax_p_v2((l_lon * 0x1.000000p1f), (l_n.y * 0x1.c00000p3f))) * 0x1.19999ap1f);
    l_band = (0x1.000000p-1f + (0x1.000000p-1f * sin(((l_n.y * select(0x1.100000p4f, 0x1.800000p3f, (l_kind == 7i))) + l_wob))));
    if ((l_kind == 6i))
    {
      return coax_p_wg_mix_f3f3f(coax_p_v3(0x1.3d70a4p-1f, 0x1.eb851ep-2f, 0x1.851eb8p-2f), coax_p_v3(0x1.ccccccp-1f, 0x1.b33334p-1f, 0x1.800000p-1f), l_band);
    }
    return coax_p_wg_mix_f3f3f(coax_p_v3(0x1.851eb8p-1f, 0x1.5c28f6p-1f, 0x1.000000p-1f), coax_p_v3(0x1.d70a3ep-1f, 0x1.b851ecp-1f, 0x1.5c28f6p-1f), l_band);
  }
  if ((l_kind == 8i))
  {
    return coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-2f, 0x1.99999ap-2f, 0x1.8f5c28p-2f), (0x1.800000p-1f + (0x1.ccccccp-2f * coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.400000p2f))))), coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p-1f, 0x1.000000p-1f, 0x1.000000p-1f), coax_p_smoothstep(0x1.ccccccp-1f, 0x1.e66666p-1f, coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.200000p3f)))));
  }
  if ((l_kind == 9i))
  {
    return coax_p_wg_mul_f3f(coax_p_v3(0x1.19999ap-1f, 0x1.0f5c28p-1f, 0x1.000000p-1f), (0x1.666666p-1f + (0x1.000000p-1f * coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.000000p2f)))));
  }
  l_crater = coax_p_smoothstep(0x1.19999ap-1f, 0x1.666666p-1f, coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.400000p2f)));
  return coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(0x1.51eb86p-1f, 0x1.70a3d8p-1f, 0x1.8f5c28p-1f), coax_p_v3(0x1.70a3d8p-2f, 0x1.851eb8p-2f, 0x1.ae147ap-2f), l_crater), (0x1.b33334p-1f + (0x1.333334p-2f * coax_p_belt_fbm(coax_p_wg_mul_f2f(l_uv, 0x1.a00000p3f)))));
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
fn coax_p_BELT_GREY_LIGHT() -> vec3f {
  return coax_p_v3(0x1.8f5c28p-1f, 0x1.8f5c28p-1f, 0x1.8f5c28p-1f);
}
fn coax_p_BELT_ACCENT() -> vec3f {
  return coax_p_v3(0x1.ba5e36p-3f, 0x1.b3b646p-1f, 0x1.b3b646p-1f);
}
fn coax_p_clampf(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return coax_p_fmin(coax_p_fmax(l_x, l_lo), l_hi);
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_stars_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return coax_p_stars_fract((sin(((l_x * 0x1.fc6666p6f) + (l_y * 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_belt_vnoise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_belt_hash2(l_i), coax_p_belt_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_belt_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_belt_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
}
fn coax_p_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_wg_mul_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a * l_b.x), (l_a * l_b.y));
}
fn coax_p_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return (((l_a.x * l_b.x) + (l_a.y * l_b.y)) + (l_a.z * l_b.z));
}
fn coax_p_wg_atan2(p_y: f32, p_x: f32) -> f32 {
  var l_y: f32 = p_y;
  var l_x: f32 = p_x;
  var l_ax: f32;
  var l_ay: f32;
  var l_hi: f32;
  var l_r: f32;
  l_ax = abs(l_x);
  l_ay = abs(l_y);
  l_hi = coax_p_fmax(l_ax, l_ay);
  if ((l_hi == 0.0f))
  {
    return 0.0f;
  }
  l_r = coax_p_wg_atan01((coax_p_fmin(l_ax, l_ay) / l_hi));
  if ((l_ay > l_ax))
  {
    l_r = ((0x1.921fb6p1f * 0x1.000000p-1f) - l_r);
  }
  if ((l_x < 0.0f))
  {
    l_r = (0x1.921fb6p1f - l_r);
  }
  if ((l_y < 0.0f))
  {
    l_r = (-l_r);
  }
  return l_r;
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_stars_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_fract_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y));
}
fn coax_p_wg_sub_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a - l_b.x), (l_a - l_b.y));
}
fn coax_p_belt_hash2(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(coax_p_dot3(l_a, l_a));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_wg_atan01(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32;
  var l_p: f32;
  l_z = (l_x * l_x);
  l_p = (-0x1.09afcep-8f);
  l_p = ((l_p * l_z) + 0x1.662ca4p-6f);
  l_p = ((l_p * l_z) - 0x1.ca0388p-5f);
  l_p = ((l_p * l_z) + 0x1.8aefbep-4f);
  l_p = ((l_p * l_z) - 0x1.1cd8c6p-3f);
  l_p = ((l_p * l_z) + 0x1.98814cp-3f);
  l_p = ((l_p * l_z) - 0x1.554c38p-2f);
  l_p = ((l_p * l_z) + 0x1.ffffeap-1f);
  return (l_x * l_p);
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