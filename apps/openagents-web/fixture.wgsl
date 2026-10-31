// ---- generated: apps/openagents-web/gpu/fixture.coax (the module) by coax wgsl; do not edit
struct coax_t10 {
  x: u32,
  y: u32,
}
struct coax_t13 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec3f,
}
struct coax_io_13 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
}
struct coax_t14 {
  x: vec3f,
  y: vec3f,
  z: vec3f,
}
struct coax_t16 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t18 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec3f,
  f5: u32,
}
struct coax_io_18 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec3f,
  @location(4) @interpolate(flat) f5: u32,
}
struct coax_t19 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_19 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t20 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_20 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t24 {
  x: vec4f,
  y: vec4f,
  z: f32,
}
struct coax_out_24 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @builtin(frag_depth) z: f32,
}
struct coax_t25 {
  x: f32,
  y: vec3f,
  z: vec3f,
  w: f32,
  f4: f32,
}
struct coax_t26 {
  x: vec4f,
  y: vec4f,
}
struct coax_out_26 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
struct coax_t32 {
  x: vec3f,
  y: vec3f,
  z: vec3f,
  w: f32,
  f4: f32,
  f5: f32,
}
struct coax_t33 {
  x: i32,
  y: i32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 71>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> objects: array<coax_t16>;
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
@group(1) @binding(2) var sheets: texture_2d_array<f32>;
@group(1) @binding(3) var sheet: sampler;
@group(1) @binding(0) var bloom_src: texture_2d<f32>;
@group(1) @binding(1) var bloom_lin: sampler;
@group(1) @binding(2) var<uniform> bloom_step: array<vec4f, 1>;
fn coax_at_bloom_step(i: i32) -> f32 {
  return bloom_step[i / 4][i % 4];
}
@group(1) @binding(0) var post_light: texture_2d<f32>;
@group(1) @binding(1) var post_quarter: texture_2d<f32>;
@group(1) @binding(2) var post_eighth: texture_2d<f32>;
@group(1) @binding(3) var post_aux: texture_2d<f32>;
@group(1) @binding(5) var post_lin: sampler;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_caster_vs(p_v: u32, p_i: u32) -> vec4f {
  var l_v: u32 = p_v;
  var l_i: u32 = p_i;
  var l_corner: array<coax_t10, 6>;
  var l_s: vec4f;
  var l_w: vec3f;
  var l_f: u32;
  var l_a: u32;
  var l_k: coax_t10;
  var l_q: vec3f;
  var l_quad: u32;
  var l_k_11: coax_t10;
  var l_ring: u32;
  var l_th: f32;
  var l_ph: f32;
  var l_q_15: vec3f;
  l_corner = array<coax_t10, 6>(coax_p_u2(0u, 0u), coax_p_u2(1u, 0u), coax_p_u2(1u, 1u), coax_p_u2(0u, 0u), coax_p_u2(1u, 1u), coax_p_u2(0u, 1u));
  l_s = coax_p_fixture_caster(l_i);
  l_w = coax_p_v3(l_s.x, l_s.y, l_s.z);
  if ((l_i == 1u))
  {
    if ((l_v < 36u))
    {
      l_f = (l_v / 6u);
      l_a = (l_f / 2u);
      l_k = l_corner[i32((l_v % 6u))];
      l_q = coax_p_v3(0.0f, 0.0f, 0.0f);
      l_q = coax_p_wg_set_f3(l_q, i32(l_a), select((-0x1.000000p0f), 0x1.000000p0f, ((l_f & 1u) == 1u)));
      l_q = coax_p_wg_set_f3(l_q, i32(((l_a + 1u) % 3u)), ((f32(l_k.x) * 0x1.000000p1f) - 0x1.000000p0f));
      l_q = coax_p_wg_set_f3(l_q, i32(((l_a + 2u) % 3u)), ((f32(l_k.y) * 0x1.000000p1f) - 0x1.000000p0f));
      l_w = coax_p_wg_add_f3f3(coax_p_v3(l_s.x, l_s.y, l_s.z), coax_p_fixture_turn(coax_p_wg_mul_f3f(l_q, l_s.w), 0x1.000000p-1f));
    }
  }
  else
  {
    l_quad = (l_v / 6u);
    l_k_11 = l_corner[i32((l_v % 6u))];
    l_ring = coax_p_wg_min_u(((l_quad / 24u) + l_k_11.y), 13u);
    l_th = ((0x1.921fb6p1f * f32(l_ring)) / f32(13u));
    l_ph = ((0x1.921fb6p2f * f32(((l_quad % 24u) + l_k_11.x))) / f32(24u));
    l_w = coax_p_wg_add_f3f3(coax_p_v3(l_s.x, l_s.y, l_s.z), coax_p_wg_mul_f3f(coax_p_v3((sin(l_th) * cos(l_ph)), cos(l_th), (sin(l_th) * sin(l_ph))), l_s.w));
    l_w.y = (l_w.y + (select((-0x1.000000p0f), 0x1.000000p0f, ((l_ring * 2u) < 13u)) * coax_p_fixture_caster_stretch(l_i)));
  }
  l_q_15 = coax_p_fixture_shadow_place(l_w, coax_p_fixture_light_space());
  return coax_p_wg_make_f4_f2ff(coax_p_v2(l_q_15.x, l_q_15.y), l_q_15.z, 0x1.000000p0f);
}
fn coax_p_grass_vs(p_v: u32, p_i: u32) -> coax_t13 {
  var l_v: u32 = p_v;
  var l_i: u32 = p_i;
  var l_o: coax_t13;
  var l_g: vec4f;
  var l_g2: vec4f;
  var l_w: vec4f;
  var l_w2: vec4f;
  var l_per: u32;
  var l_r: f32;
  var l_side: u32;
  var l_cell: u32;
  var l_b: f32;
  var l_c: vec2f;
  var l_k: vec2f;
  var l_q: vec2f;
  var l_dist: f32;
  var l_tall: f32;
  var l_t: f32;
  var l_wd: vec2f;
  var l_theta: f32;
  var l_away: vec2f;
  var l_part: f32;
  var l_lean: vec2f;
  var l_ang: f32;
  var l_across: vec3f;
  var l_world: vec3f;
  var l_base: vec3f;
  var l_hj: f32;
  var l_a: vec3f;
  var l_l: vec3f;
  var l_lx: vec2f;
  var l_tilt: f32;
  var l_e: coax_t14;
  var l_rel: vec3f;
  var l_aspect: f32;
  var l_tv: f32;
  var l_z: f32;
  l_o = coax_t13();
  l_o.x = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  l_o.y = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.z = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_o.w = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_g = coax_p_fixture_st(22u);
  l_g2 = coax_p_fixture_st(23u);
  l_w = coax_p_fixture_st(24u);
  l_w2 = coax_p_fixture_st(25u);
  l_per = coax_p_wg_max_u(coax_p_wg_u32_f(l_g.x), 1u);
  l_r = l_g.y;
  l_side = coax_p_wg_u32_f((0x1.000000p1f * l_r));
  l_cell = (l_i / l_per);
  l_b = f32((l_i % l_per));
  l_c = coax_p_v2(((f32((l_cell % l_side)) - l_r) + (coax_p_FIXTURE_EYE()).x), (((coax_p_FIXTURE_EYE()).z - 0x1.000000p0f) - f32((l_cell / l_side))));
  l_k = coax_p_wg_add_f2f2(l_c, coax_p_v2((l_b * 0x1.c851ecp2f), (l_b * (-0x1.dae148p1f))));
  l_q = coax_p_wg_add_f2f2(l_c, coax_p_v2(coax_p_fixture_hash(l_k), coax_p_fixture_hash(coax_p_wg_add_f2f2(l_k, coax_p_v2(0x1.31999ap4f, 0x1.ecccccp2f)))));
  l_dist = coax_p_wg_length2(coax_p_wg_sub_f2f2(l_q, coax_p_wg_sw_xz_f3(coax_p_FIXTURE_EYE())));
  l_tall = ((coax_p_verse_style_blade_height(coax_p_fixture_hash(coax_p_wg_add_f2f2(l_k, coax_p_v2(0x1.a66666p1f, 0x1.4f3334p5f))), l_g.w, l_g2.x) * coax_p_verse_style_grass_fade(l_dist, l_r, l_g.z)) * coax_p_fixture_grass_mask(l_q));
  if ((l_tall < 0x1.47ae14p-6f))
  {
    return l_o;
  }
  l_t = select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
  l_wd = coax_p_v2(l_w2.x, l_w2.y);
  l_theta = coax_p_verse_style_wind(coax_p_wg_dot2(l_q, l_wd), coax_p_verse_style_gust(l_q.x, l_q.y, l_t, l_wd.x, l_wd.y, l_w.z, l_w.w), l_t, l_w.x, l_w.y, l_w.z);
  l_away = coax_p_wg_sub_f2f2(l_q, coax_p_wg_sw_xz_f4(coax_p_FIXTURE_BODY()));
  l_part = coax_p_verse_style_part(coax_p_wg_length2(l_away), l_w2.w);
  l_lean = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_wd, coax_p_verse_style_bend(l_theta, l_tall, 0x1.000000p0f)), coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_normalize2(coax_p_wg_add_f2f2(l_away, coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f))), l_tall), 0x1.b33334p-1f), l_part));
  l_ang = (coax_p_fixture_hash(coax_p_wg_add_f2f2(l_k, coax_p_v2(0x1.79999ap2f, 0x1.a9999ap3f))) * 0x1.921fb6p2f);
  l_across = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(cos(l_ang), 0.0f, sin(l_ang)), l_g2.y), 0x1.000000p-1f);
  l_world = coax_p_v3(l_q.x, 0.0f, l_q.y);
  l_base = coax_p_fixture_meadow(l_q);
  l_hj = coax_p_fixture_hash(coax_p_wg_add_f2f2(l_k, coax_p_v2(0x1.fb3334p4f, 0x1.733334p1f)));
  l_a = coax_p_v3(coax_p_verse_style_jitter(l_base.x, 0.0f, l_hj, l_g2.w), coax_p_verse_style_jitter(l_base.y, 0x1.000000p0f, l_hj, l_g2.w), coax_p_verse_style_jitter(l_base.z, 0x1.000000p1f, l_hj, l_g2.w));
  if ((l_v == 0u))
  {
    l_world = coax_p_wg_sub_f3f3(l_world, l_across);
    l_o.w = coax_p_wg_mul_f3f(l_a, 0x1.8f5c28p-1f);
  }
  else
  {
    if ((l_v == 1u))
    {
      l_world = coax_p_wg_add_f3f3(l_world, l_across);
      l_o.w = coax_p_wg_mul_f3f(l_a, 0x1.8f5c28p-1f);
    }
    else
    {
      l_world = coax_p_wg_add_f3f3(l_world, coax_p_v3(l_lean.x, sqrt(coax_p_fmax(((l_tall * l_tall) - coax_p_wg_dot2(l_lean, l_lean)), ((l_tall * l_tall) * 0x1.333334p-3f))), l_lean.y));
      l_o.w = coax_p_v3(coax_p_verse_style_tip(l_a.x, 0.0f, l_g2.z), coax_p_verse_style_tip(l_a.y, 0x1.000000p0f, l_g2.z), coax_p_verse_style_tip(l_a.z, 0x1.000000p1f, l_g2.z));
    }
  }
  l_l = coax_p_wg_sw_xyz_f4(coax_p_fixture_st(1u));
  l_lx = coax_p_wg_normalize2(coax_p_wg_add_f2f2(coax_p_v2(l_l.x, l_l.z), coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f)));
  l_tilt = coax_p_verse_style_grass_tilt(l_theta, l_w.x, l_w2.z);
  l_o.z = coax_p_wg_normalize3(coax_p_v3((l_lx.x * sin(l_tilt)), cos(l_tilt), (l_lx.y * sin(l_tilt))));
  l_e = coax_p_fixture_eye_basis();
  l_rel = coax_p_wg_sub_f3f3(l_world, coax_p_FIXTURE_EYE());
  l_aspect = (coax_at_u(i32(0i)) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f));
  l_tv = coax_p_fmax(0x1.70a3d8p-2f, (0x1.000000p-1f / l_aspect));
  l_z = coax_p_wg_dot3(l_rel, l_e.z);
  l_o.x = coax_p_v4((coax_p_wg_dot3(l_rel, l_e.x) / (l_tv * l_aspect)), (coax_p_wg_dot3(l_rel, l_e.y) / l_tv), ((0x1.f40000p10f / (0x1.f40000p10f - 0x1.99999ap-4f)) * (l_z - 0x1.99999ap-4f)), l_z);
  l_o.y = l_world;
  return l_o;
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t18 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_b: u32;
  var l_p: vec4f;
  var l_n: vec3f;
  var l_m: coax_t16;
  var l_w: vec3f;
  var l_wp: vec4f;
  var l_o: coax_t18;
  l_b = (l_id * 12u);
  l_p = coax_p_v4(coax_p_fixture_word(l_b), coax_p_fixture_word((l_b + 1u)), coax_p_fixture_word((l_b + 2u)), 0x1.000000p0f);
  l_n = coax_p_v3(coax_p_fixture_word((l_b + 3u)), coax_p_fixture_word((l_b + 4u)), coax_p_fixture_word((l_b + 5u)));
  l_m = objects[i32(l_instance)];
  l_w = coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p));
  l_wp = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  l_o = coax_t18();
  l_o.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(12i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(16i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(20i), l_wp));
  l_o.y = l_w;
  l_o.z = coax_p_v3(coax_p_wg_dot3(coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z), l_n), coax_p_wg_dot3(coax_p_v3(l_m.y.x, l_m.y.y, l_m.y.z), l_n), coax_p_wg_dot3(coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z), l_n));
  l_o.w = coax_p_v2(coax_p_fixture_word((l_b + 6u)), coax_p_fixture_word((l_b + 7u)));
  l_o.f4 = coax_p_wg_mul_f3f3(coax_p_wg_sw_rgb_f4(coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 8u))])), coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z));
  l_o.f5 = (vertices[i32((l_b + 8u))] >> u32(24u));
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t19) -> coax_t20 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t19 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t20;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t20();
  l_ndc = coax_p_v2((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_o.x = coax_p_v4(l_ndc.x, l_ndc.y, 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t24 {
  var l_pos: vec4f = p_pos;
  var l_shaft_glow_store: array<f32, 1>;
  var l_paint_depth_store: array<f32, 1>;
  var l_p: vec2f;
  var l_c: vec4f;
  var l_c_10: vec2f;
  var l_r: f32;
  var l_sky: vec3f;
  l_shaft_glow_store = array<f32, 1>();
  l_shaft_glow_store[0i] = 0.0f;
  l_paint_depth_store = array<f32, 1>();
  l_paint_depth_store[0i] = 0x1.000000p0f;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  if ((coax_at_u(i32(7i)) > 0x1.000000p-1f))
  {
    l_c = coax_p_fixture_paint(l_p, &l_shaft_glow_store, &l_paint_depth_store);
    return coax_t24(l_c, coax_p_v4(l_shaft_glow_store[0i], 0.0f, 0.0f, 0.0f), l_paint_depth_store[0i]);
  }
  l_c_10 = coax_p_wg_mul_f2f2(coax_p_v2(0x1.000000p-1f, 0x1.28f5c2p-1f), coax_p_wg_at2(0i));
  l_r = coax_p_wg_length2(coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(l_p, l_c_10), coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f)));
  l_sky = coax_p_wg_add_f3f3(coax_p_v3(0x1.89374cp-7f, 0x1.a9fbe8p-7f, 0x1.47ae14p-6f), coax_p_wg_mul_f3f(coax_p_v3(0x1.1eb852p-5f, 0x1.26e978p-5f, 0x1.70a3d8p-5f), exp((((-l_r) * l_r) * 0x1.800000p2f))));
  l_sky = coax_p_wg_add_f3f3(l_sky, coax_p_wg_mul_f3f(coax_p_wg_make_f3_f(coax_p_fixture_stars(l_p)), coax_at_u(i32(3i))));
  return coax_t24(coax_p_wg_make_f4_f3f(coax_p_fixture_panels(l_p, l_sky), 0.0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f), 0x1.000000p0f);
}
fn coax_p_grass_fs(p_g: coax_t13) -> coax_t26 {
  var l_g: coax_t13 = p_g;
  var l_d: vec3f;
  var l_h: coax_t25;
  var l_c: vec3f;
  var l_b: vec4f;
  l_d = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_g.y, coax_p_FIXTURE_EYE()));
  l_h = coax_t25(coax_p_wg_length3(coax_p_wg_sub_f3f3(l_g.y, coax_p_FIXTURE_EYE())), coax_p_wg_normalize3(l_g.z), l_g.w, 0x1.000000p0f, 0x1.800000p3f);
  l_c = coax_p_fixture_shade(l_h, coax_p_FIXTURE_EYE(), l_d);
  l_b = coax_p_fixture_st(19u);
  return coax_t26(coax_p_v4(l_c.x, l_c.y, l_c.z, coax_p_verse_style_bloom_share(coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z), l_b.x, l_b.y)), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_scene_fs(p_s: coax_t18) -> coax_t26 {
  var l_s: coax_t18 = p_s;
  var l_tex: vec4f;
  var l_n: vec3f;
  var l_key: f32;
  var l_lit: vec3f;
  var l_to: vec3f;
  var l_d: f32;
  var l_near: f32;
  var l_c: vec3f;
  l_tex = textureSample(sheets, sheet, l_s.w, i32(l_s.f5));
  if ((l_tex.w < 0x1.666666p-2f))
  {
    discard;
  }
  l_n = coax_p_wg_normalize3(l_s.z);
  l_key = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_wg_normalize3(coax_p_v3((-0x1.99999ap-2f), 0x1.99999ap-1f, 0x1.ccccccp-2f))));
  l_lit = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-4f, 0x1.c28f5cp-4f, 0x1.47ae14p-3f), (0x1.333334p-1f + (0x1.99999ap-2f * l_n.y))), coax_p_wg_mul_f3f(coax_p_v3(0x1.ae147ap-2f, 0x1.c28f5cp-2f, 0x1.19999ap-1f), l_key));
  l_to = coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y);
  l_d = coax_p_wg_length3(l_to);
  l_near = ((coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_wg_div_f3f(l_to, coax_p_fmax(l_d, 0x1.0624dep-10f)))) * coax_at_u(i32(27i))) / (0x1.000000p0f + ((0x1.70a3d8p-3f * l_d) * l_d)));
  l_lit = coax_p_wg_add_f3f3(l_lit, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.3d70a4p-1f, 0x1.1eb852p-2f), l_near), 0x1.99999ap0f));
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f3(coax_p_v3(l_tex.x, l_tex.y, l_tex.z), l_s.f4), l_lit), coax_at_u(i32(3i)));
  return coax_t26(coax_p_wg_make_f4_f3f(coax_p_fixture_panels(coax_p_v2(l_s.x.x, l_s.x.y), l_c), 0.0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_bloom_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_texel_2: vec2f;
  var l_uv: vec2f;
  var l_c: vec3f;
  var l_k: i32;
  var l_t: vec4f;
  var l_d: vec2f;
  var l_c_10: vec3f;
  l_texel_2 = coax_p_wg_div_ff2(0x1.000000p0f, coax_p_wg_cvt_f2u2(coax_p_wg_dims_2d()));
  l_uv = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_pos.x, l_pos.y), coax_at_bloom_step(i32(3i))), l_texel_2);
  if ((coax_at_bloom_step(i32(2i)) < 0x1.000000p-1f))
  {
    l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
    {
      l_k = 0i;
      while ((l_k < 4i))
      {
        l_t = textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f2(coax_p_wg_sub_f2f(coax_p_wg_mul_f2f(coax_p_v2(f32((l_k & 1i)), f32((l_k >> u32(1i)))), 0x1.000000p1f), 0x1.000000p0f), l_texel_2)), 0.0f);
        l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(l_t.x, l_t.y, l_t.z), coax_p_clampf(l_t.w, 0.0f, 0x1.000000p0f)));
        l_k = (l_k + 1i);
      }
    }
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(l_c, 0x1.000000p-2f), 0x1.000000p0f);
  }
  l_d = coax_p_wg_mul_f2f2(coax_p_v2(coax_at_bloom_step(i32(0i)), coax_at_bloom_step(i32(1i))), l_texel_2);
  l_c_10 = coax_p_wg_mul_f3f(coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, l_uv, 0.0f)), 0x1.d0e560p-3f);
  l_c_10 = coax_p_wg_add_f3f3(l_c_10, coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f(l_d, 0x1.628f5cp0f)), 0.0f)), coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_sub_f2f2(l_uv, coax_p_wg_mul_f2f(l_d, 0x1.628f5cp0f)), 0.0f))), 0x1.439582p-2f));
  l_c_10 = coax_p_wg_add_f3f3(l_c_10, coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f(l_d, 0x1.9d9168p1f)), 0.0f)), coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_sub_f2f2(l_uv, coax_p_wg_mul_f2f(l_d, 0x1.9d9168p1f)), 0.0f))), 0x1.1eb852p-4f));
  return coax_p_v4(l_c_10.x, l_c_10.y, l_c_10.z, 0x1.000000p0f);
}
fn coax_p_post_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_light: vec3f;
  var l_uv: vec2f;
  var l_b: vec3f;
  var l_m: f32;
  var l_c: vec3f;
  l_light = coax_p_wg_sw_rgb_f4(coax_p_wg_load_2d(coax_p_wg_cvt_i2f2(coax_p_v2(l_pos.x, l_pos.y)), 0i));
  if (((coax_at_u(i32(7i)) < 0x1.000000p-1f) || (coax_at_u(i32(7i)) > 0x1.800000p0f)))
  {
    return coax_p_v4(l_light.x, l_light.y, l_light.z, 0x1.000000p0f);
  }
  l_uv = coax_p_wg_div_f2f2(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_at2(0i));
  l_b = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_sw_rgb_f4(textureSampleLevel(post_quarter, post_lin, l_uv, 0.0f)), 0x1.19999ap-1f), coax_p_wg_mul_f3f(coax_p_wg_sw_rgb_f4(textureSampleLevel(post_eighth, post_lin, l_uv, 0.0f)), 0x1.ccccccp-2f));
  l_m = (coax_p_fixture_st(19u)).z;
  l_c = coax_p_v3(coax_p_verse_style_bloom_mix(l_light.x, l_b.x, l_m), coax_p_verse_style_bloom_mix(l_light.y, l_b.y, l_m), coax_p_verse_style_bloom_mix(l_light.z, l_b.z, l_m));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_ff3(coax_p_fixture_shafts(coax_p_v2(l_pos.x, l_pos.y), l_uv), coax_p_wg_sw_xyz_f4(coax_p_fixture_st(11u))), (coax_p_fixture_st(0u)).w));
  return coax_p_wg_make_f4_f3f(coax_p_fixture_panels(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_pow_f3f3(coax_p_wg_clamp_f3f3f3(coax_p_fixture_tone(l_c), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), coax_p_wg_make_f3_f((0x1.000000p0f / 0x1.19999ap1f)))), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t20) -> vec4f {
  var l_v: coax_t20 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.1eb852p-2f);
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
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t10 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t10(l_x, l_y);
}
fn coax_p_fixture_caster(p_i: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l_c: array<vec4f, 8>;
  l_c = array<vec4f, 8>(coax_p_FIXTURE_SPHERE(), coax_p_FIXTURE_CUBE(), coax_p_fixture_canopy(0i), coax_p_fixture_canopy(1i), coax_p_fixture_canopy(2i), coax_p_v4((coax_p_FIXTURE_TRUNK()).x, 0x1.e66666p-1f, (coax_p_FIXTURE_TRUNK()).z, (coax_p_FIXTURE_TRUNK()).w), coax_p_v4((coax_p_FIXTURE_BODY()).x, 0x1.8a3d70p-1f, (coax_p_FIXTURE_BODY()).z, (coax_p_FIXTURE_BODY()).w), coax_p_FIXTURE_HEAD());
  return l_c[i32(l_i)];
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_set_f3(p_p: vec3f, p_i: i32, p_x: f32) -> vec3f {
  var l_p: vec3f = p_p;
  var l_i: i32 = p_i;
  var l_x: f32 = p_x;
  var l_r: vec3f;
  l_r = l_p;
  if ((l_i == 0i))
  {
    l_r.x = l_x;
  }
  if ((l_i == 1i))
  {
    l_r.y = l_x;
  }
  if ((l_i == 2i))
  {
    l_r.z = l_x;
  }
  return l_r;
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_fixture_turn(p_v: vec3f, p_a: f32) -> vec3f {
  var l_v: vec3f = p_v;
  var l_a: f32 = p_a;
  var l_c: f32;
  var l_s: f32;
  l_c = cos(l_a);
  l_s = sin(l_a);
  return coax_p_v3(((l_c * l_v.x) - (l_s * l_v.z)), l_v.y, ((l_s * l_v.x) + (l_c * l_v.z)));
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_min_u(p_a: u32, p_b: u32) -> u32 {
  var l_a: u32 = p_a;
  var l_b: u32 = p_b;
  if ((l_b < l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_fixture_caster_stretch(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  var l_h: array<f32, 8>;
  l_h = array<f32, 8>(0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0x1.8a3d70p-1f, 0x1.0f5c28p-1f, 0.0f);
  return l_h[i32(l_i)];
}
fn coax_p_fixture_light_space() -> coax_t32 {
  var l_l: vec3f;
  var l_ax: vec3f;
  var l_ay: vec3f;
  var l_reach: f32;
  var l_size: f32;
  var l_c: vec3f;
  l_l = coax_p_wg_sw_xyz_f4(coax_p_fixture_st(1u));
  l_ax = select(coax_p_v3(0x1.000000p0f, 0.0f, 0.0f), coax_p_wg_normalize3(coax_p_wg_cross3(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_l)), (abs(l_l.y) < 0x1.ff7ceep-1f));
  l_ay = coax_p_wg_cross3(l_l, l_ax);
  l_reach = coax_p_verse_style_shadow_reach((coax_p_fixture_st(31u)).z);
  l_size = coax_p_fmax((coax_p_fixture_st(21u)).x, 0x1.000000p0f);
  l_c = coax_p_v3((coax_p_FIXTURE_EYE()).x, 0.0f, (coax_p_FIXTURE_EYE()).z);
  return coax_t32(l_ax, l_ay, l_l, coax_p_verse_style_shadow_snap(coax_p_wg_dot3(l_c, l_ax), l_reach, l_size), coax_p_verse_style_shadow_snap(coax_p_wg_dot3(l_c, l_ay), l_reach, l_size), l_reach);
}
fn coax_p_fixture_shadow_place(p_w: vec3f, p_k: coax_t32) -> vec3f {
  var l_w: vec3f = p_w;
  var l_k: coax_t32 = p_k;
  return coax_p_v3(coax_p_verse_style_shadow_across((coax_p_wg_dot3(l_w, l_k.x) - l_k.w), l_k.f5), coax_p_verse_style_shadow_across((coax_p_wg_dot3(l_w, l_k.y) - l_k.f4), l_k.f5), coax_p_verse_style_shadow_depth(coax_p_wg_dot3(coax_p_wg_sub_f3f3(l_w, coax_p_wg_add_f3f3(coax_p_wg_mul_ff3(l_k.w, l_k.x), coax_p_wg_mul_ff3(l_k.f4, l_k.y))), l_k.z), l_k.f5));
}
fn coax_p_wg_make_f4_f2ff(p_p0: vec2f, p_p1: f32, p_p2: f32) -> vec4f {
  var l_p0: vec2f = p_p0;
  var l_p1: f32 = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v4(l_p0.x, l_p0.y, l_p1, l_p2);
}
fn coax_p_fixture_st(p_i: u32) -> vec4f {
  var l_i: u32 = p_i;
  return coax_p_wg_at4(((i32(l_i) * 4i) + 156i));
}
fn coax_p_wg_u32_f(p_x: f32) -> u32 {
  var l_x: f32 = p_x;
  if ((!(l_x > 0.0f)))
  {
    return 0u;
  }
  if ((l_x >= 0x1.fffffep31f))
  {
    return 4294967040u;
  }
  return u32(l_x);
}
fn coax_p_wg_max_u(p_a: u32, p_b: u32) -> u32 {
  var l_a: u32 = p_a;
  var l_b: u32 = p_b;
  if ((l_b > l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_FIXTURE_EYE() -> vec3f {
  return coax_p_v3(0.0f, 0x1.c00000p0f, 0x1.000000p3f);
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_fixture_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_wg_sw_xz_f3(p_p: vec3f) -> vec2f {
  var l_p: vec3f = p_p;
  return coax_p_v2(l_p.x, l_p.z);
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
fn coax_p_verse_style_blade_height(p_h: f32, p_low: f32, p_high: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return (l_low + ((l_high - l_low) * coax_p_st_sat(l_h)));
}
fn coax_p_verse_style_grass_fade(p_d: f32, p_radius: f32, p_fade: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_radius: f32 = p_radius;
  var l_fade: f32 = p_fade;
  return (0x1.000000p0f - coax_p_verse_style_soft_step((l_radius - (l_fade * 0x1.000000p-1f)), (l_fade * 0x1.000000p-1f), l_d));
}
fn coax_p_fixture_grass_mask(p_q: vec2f) -> f32 {
  var l_q: vec2f = p_q;
  var l_path: f32;
  var l_pond: f32;
  l_path = (coax_p_fmin(coax_p_verse_style_segment(l_q.x, l_q.y, (coax_p_FIXTURE_PATH0()).x, (coax_p_FIXTURE_PATH0()).y, (coax_p_FIXTURE_PATH0()).z, (coax_p_FIXTURE_PATH0()).w), coax_p_verse_style_segment(l_q.x, l_q.y, (coax_p_FIXTURE_PATH1()).x, (coax_p_FIXTURE_PATH1()).y, (coax_p_FIXTURE_PATH1()).z, (coax_p_FIXTURE_PATH1()).w)) - 0x1.333334p-1f);
  l_pond = (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_q, coax_p_wg_sw_xz_f4(coax_p_FIXTURE_POND()))) - (coax_p_FIXTURE_POND()).w);
  return (coax_p_verse_style_grass_clear(l_path) * coax_p_verse_style_grass_clear(l_pond));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_verse_style_gust(p_x: f32, p_z: f32, p_t: f32, p_wx: f32, p_wz: f32, p_speed: f32, p_scale: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_t: f32 = p_t;
  var l_wx: f32 = p_wx;
  var l_wz: f32 = p_wz;
  var l_speed: f32 = p_speed;
  var l_scale: f32 = p_scale;
  return ((0x1.000000p1f * coax_p_verse_style_noise(((l_x - ((l_wx * l_speed) * l_t)) * l_scale), ((l_z - ((l_wz * l_speed) * l_t)) * l_scale))) - 0x1.000000p0f);
}
fn coax_p_verse_style_wind(p_along: f32, p_gust: f32, p_t: f32, p_amp: f32, p_wavelength: f32, p_speed: f32) -> f32 {
  var l_along: f32 = p_along;
  var l_gust: f32 = p_gust;
  var l_t: f32 = p_t;
  var l_amp: f32 = p_amp;
  var l_wavelength: f32 = p_wavelength;
  var l_speed: f32 = p_speed;
  var l_k: f32;
  l_k = (0x1.921fb6p2f / coax_p_st_max(l_wavelength, 0x1.0624dep-10f));
  return (l_amp * ((0x1.333334p-1f * sin(((l_k * l_along) - ((l_k * l_speed) * l_t)))) + (0x1.99999ap-2f * l_gust)));
}
fn coax_p_FIXTURE_BODY() -> vec4f {
  return coax_p_v4((-0x1.333334p-2f), 0.0f, (-0x1.ccccccp0f), 0x1.eb851ep-3f);
}
fn coax_p_wg_sw_xz_f4(p_p: vec4f) -> vec2f {
  var l_p: vec4f = p_p;
  return coax_p_v2(l_p.x, l_p.z);
}
fn coax_p_verse_style_part(p_d: f32, p_radius: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_radius: f32 = p_radius;
  return coax_p_st_sat((0x1.000000p0f - (l_d / coax_p_st_max(l_radius, 0x1.0624dep-10f))));
}
fn coax_p_verse_style_bend(p_theta: f32, p_height: f32, p_share: f32) -> f32 {
  var l_theta: f32 = p_theta;
  var l_height: f32 = p_height;
  var l_share: f32 = p_share;
  return (((sin(l_theta) * l_height) * l_share) * l_share);
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_wg_normalize2(p_a: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  return coax_p_scale2(l_a, (0x1.000000p0f / coax_p_wg_length2(l_a)));
}
fn coax_p_fixture_meadow(p_q: vec2f) -> vec3f {
  var l_q: vec2f = p_q;
  var l_lush: f32;
  var l_t: f32;
  l_lush = (0x1.000000p0f - coax_p_smoothstep(0x1.333334p-1f, 0x1.000000p0f, (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_q, coax_p_wg_sw_xz_f4(coax_p_FIXTURE_GRASS()))) / (coax_p_FIXTURE_GRASS()).w)));
  l_t = (0x1.000000p-1f + (0x1.000000p-1f * sin(((l_q.x * 0x1.ae147ap-3f) + (l_q.y * 0x1.0a3d70p-3f)))));
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(coax_p_v3(0x1.c28f5cp-3f, 0x1.851eb8p-2f, 0x1.47ae14p-5f), coax_p_v3(0x1.99999ap-4f, 0x1.000000p-2f, 0x1.47ae14p-5f), (l_t * 0x1.666666p-1f)), coax_p_v3(0x1.70a3d8p-4f, 0x1.0a3d70p-2f, 0x1.eb851ep-6f), (l_lush * 0x1.666666p-1f));
}
fn coax_p_verse_style_jitter(p_c: f32, p_ch: f32, p_h: f32, p_jitter: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_ch: f32 = p_ch;
  var l_h: f32 = p_h;
  var l_jitter: f32 = p_jitter;
  var l_j: f32;
  l_j = (((l_h * 0x1.000000p1f) - 0x1.000000p0f) * l_jitter);
  if ((l_ch < 0x1.000000p-1f))
  {
    return (l_c * (0x1.000000p0f + l_j));
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return l_c;
  }
  return (l_c * (0x1.000000p0f - l_j));
}
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
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
fn coax_p_verse_style_tip(p_c: f32, p_ch: f32, p_lift: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_ch: f32 = p_ch;
  var l_lift: f32 = p_lift;
  if ((l_ch < 0x1.000000p-1f))
  {
    return (l_c * (0x1.000000p0f + (l_lift * 0x1.4cccccp0f)));
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return (l_c * (0x1.000000p0f + l_lift));
  }
  return l_c;
}
fn coax_p_wg_sw_xyz_f4(p_p: vec4f) -> vec3f {
  var l_p: vec4f = p_p;
  return coax_p_v3(l_p.x, l_p.y, l_p.z);
}
fn coax_p_verse_style_grass_tilt(p_theta: f32, p_amp: f32, p_tilt_most: f32) -> f32 {
  var l_theta: f32 = p_theta;
  var l_amp: f32 = p_amp;
  var l_tilt_most: f32 = p_tilt_most;
  return (l_tilt_most * coax_p_st_max((-0x1.000000p0f), coax_p_st_min(0x1.000000p0f, (l_theta / coax_p_st_max(l_amp, 0x1.a36e2ep-14f)))));
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_fixture_eye_basis() -> coax_t14 {
  var l_a: f32;
  var l_fwd: vec3f;
  var l_right: vec3f;
  l_a = ((-0x1.e924f2p-4f) + coax_at_u(i32(8i)));
  l_fwd = coax_p_v3(0.0f, sin(l_a), (-cos(l_a)));
  l_right = coax_p_wg_normalize3(coax_p_wg_cross3(l_fwd, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)));
  return coax_t14(l_right, coax_p_wg_cross3(l_right, l_fwd), l_fwd);
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_fixture_word(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(vertices[i32(l_i)]);
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_wg_sw_rgb_f4(p_p: vec4f) -> vec3f {
  var l_p: vec4f = p_p;
  return coax_p_v3(l_p.x, l_p.y, l_p.z);
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_fixture_paint(p_p: vec2f, p_shaft_glow: ptr<function, array<f32, 1>>, p_paint_depth: ptr<function, array<f32, 1>>) -> vec4f {
  var l_p: vec2f = p_p;
  var l_d: vec3f;
  var l_h: coax_t25;
  var l_wp: vec3f;
  var l_hz: vec4f;
  var l_c: vec3f;
  var l_w: f32;
  var l_hx: coax_t25;
  var l_hy: coax_t25;
  var l_edge: f32;
  var l_wp_15: vec3f;
  var l_hz_16: vec4f;
  var l_f: f32;
  var l_style: f32;
  var l_cap: f32;
  var l_b: vec4f;
  l_d = coax_p_fixture_ray(l_p);
  l_h = coax_p_fixture_trace(coax_p_FIXTURE_EYE(), l_d);
  if ((l_h.w > 0x1.000000p-1f))
  {
    (*p_paint_depth)[0i] = coax_p_fixture_depth_at((l_h.x * coax_p_wg_dot3(l_d, (coax_p_fixture_eye_basis()).z)));
  }
  if ((coax_at_u(i32(7i)) > 0x1.800000p0f))
  {
    l_wp = coax_p_wg_add_f3f3(coax_p_FIXTURE_EYE(), coax_p_wg_mul_f3f(l_d, l_h.x));
    l_hz = coax_p_fixture_st(9u);
    if ((((l_h.w < 0x1.000000p-1f) || (l_h.w > 0x1.c00000p1f)) || (coax_p_verse_style_haze(l_h.x, l_wp.y, 0.0f, l_hz.x, l_hz.y, l_hz.z, l_hz.w) >= 0x1.666666p-2f)))
    {
      return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
    }
    return coax_p_v4(select(0.0f, 0x1.000000p0f, (coax_p_fixture_surface_band(l_h, coax_p_fixture_shadow(l_wp, l_h.y)) < 0x1.000000p-1f)), (l_h.f4 / 0x1.000000p4f), 0x1.000000p0f, 0.0f);
  }
  l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((l_h.w == 0.0f))
  {
    l_c = coax_p_fixture_sky(l_d, coax_p_wg_sw_xyz_f4(coax_p_fixture_st(1u)));
    (*p_shaft_glow)[0i] = coax_p_verse_style_shaft_source(coax_p_wg_dot3(l_d, coax_p_wg_sw_xyz_f4(coax_p_fixture_st(1u))));
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_make_f3_f(coax_p_fixture_stars(l_p)), (coax_p_fixture_st(14u)).w), 0x1.333334p-1f));
  }
  else
  {
    l_c = coax_p_fixture_shade(l_h, coax_p_FIXTURE_EYE(), l_d);
    l_w = coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f);
    l_hx = coax_p_fixture_trace(coax_p_FIXTURE_EYE(), coax_p_fixture_ray(coax_p_wg_add_f2f2(l_p, coax_p_v2(l_w, 0.0f))));
    l_hy = coax_p_fixture_trace(coax_p_FIXTURE_EYE(), coax_p_fixture_ray(coax_p_wg_add_f2f2(l_p, coax_p_v2(0.0f, l_w))));
    l_edge = select(0.0f, 0x1.000000p0f, ((l_hx.f4 != l_h.f4) || (l_hy.f4 != l_h.f4)));
    l_edge = coax_p_fmax(l_edge, coax_p_fmax(coax_p_verse_outline_crease(coax_p_wg_dot3(l_h.y, l_hx.y)), coax_p_verse_outline_crease(coax_p_wg_dot3(l_h.y, l_hy.y))));
    l_wp_15 = coax_p_wg_add_f3f3(coax_p_FIXTURE_EYE(), coax_p_wg_mul_f3f(l_d, l_h.x));
    l_hz_16 = coax_p_fixture_st(9u);
    l_f = coax_p_verse_style_haze(l_h.x, l_wp_15.y, 0.0f, l_hz_16.x, l_hz_16.y, l_hz_16.z, l_hz_16.w);
    (*p_shaft_glow)[0i] = (coax_p_verse_style_shaft_source(coax_p_wg_dot3(l_d, coax_p_wg_sw_xyz_f4(coax_p_fixture_st(1u)))) * l_f);
    l_style = (coax_p_fixture_st(31u)).y;
    l_cap = (coax_p_fixture_st(31u)).x;
    l_c = coax_p_v3(coax_p_verse_outline_styled(l_style, l_c.x, l_edge, l_cap, l_f), coax_p_verse_outline_styled(l_style, l_c.y, l_edge, l_cap, l_f), coax_p_verse_outline_styled(l_style, l_c.z, l_edge, l_cap, l_f));
  }
  l_b = coax_p_fixture_st(19u);
  return coax_p_v4(l_c.x, l_c.y, l_c.z, select(coax_p_verse_style_bloom_share(coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z), l_b.x, l_b.y), 0x1.000000p0f, (l_h.w > 0x1.c00000p1f)));
}
fn coax_p_wg_at2(p_at: i32) -> vec2f {
  var l_at: i32 = p_at;
  return coax_p_v2(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))));
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_fixture_stars(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_cell: f32;
  var l_g: vec2f;
  var l_h: f32;
  var l_at: vec2f;
  var l_d: f32;
  var l_size: f32;
  var l_twinkle: f32;
  var l_peak: f32;
  l_cell = ((0x1.800000p1f * coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f)) * 0x1.800000p2f);
  l_g = coax_p_wg_floor_f2(coax_p_wg_div_f2f(l_p, l_cell));
  l_h = coax_p_fixture_hash(l_g);
  if ((l_h < 0x1.dc28f6p-1f))
  {
    return 0.0f;
  }
  l_at = coax_p_wg_mul_f2f(coax_p_wg_add_f2f(coax_p_wg_add_f2f2(l_g, coax_p_wg_mul_f2f(coax_p_v2(coax_p_fixture_hash(coax_p_wg_add_f2f(l_g, 0x1.b33334p0f)), coax_p_fixture_hash(coax_p_wg_add_f2f(l_g, 0x1.233334p3f))), 0x1.99999ap-1f)), 0x1.99999ap-4f), l_cell);
  l_d = (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, l_at)) / coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f));
  l_size = select(0x1.19999ap-1f, 0x1.e66666p-1f, (l_h > 0x1.fbe76cp-1f));
  l_twinkle = select((0x1.800000p-1f + (0x1.000000p-2f * sin(((coax_at_u(i32(2i)) * (0x1.333334p-1f + (l_h * 0x1.000000p1f))) + (l_h * 0x1.400000p5f))))), 0x1.000000p0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
  l_peak = (select(0x1.47ae14p-2f, 0x1.800000p-1f, (l_h > 0x1.fbe76cp-1f)) * l_twinkle);
  return (l_peak * (0x1.000000p0f - coax_p_smoothstep((l_size * 0x1.99999ap-2f), (l_size + 0x1.333334p-1f), l_d)));
}
fn coax_p_wg_make_f3_f(p_p0: f32) -> vec3f {
  var l_p0: f32 = p_p0;
  return coax_p_v3(l_p0, l_p0, l_p0);
}
fn coax_p_fixture_panels(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_k: i32;
  l_c = l_under;
  {
    l_k = 0i;
    while ((l_k < 16i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_c = coax_p_fixture_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 28i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 28i)));
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
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_fixture_shade(p_h: coax_t25, p_o: vec3f, p_d: vec3f) -> vec3f {
  var l_h: coax_t25 = p_h;
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_p: vec3f;
  var l_l: vec3f;
  var l_n: vec3f;
  var l_ndl: f32;
  var l_character: bool;
  var l_lit: f32;
  var l_band: f32;
  var l_key: vec3f;
  var l_amb: vec3f;
  var l_dark: vec3f;
  var l_bright: vec3f;
  var l_a: vec3f;
  var l_c: vec3f;
  var l_depth: f32;
  var l_sh: vec4f;
  var l_md: vec4f;
  var l_dp: vec4f;
  var l_s1: f32;
  var l_s2: f32;
  var l_r: vec3f;
  var l_f: f32;
  var l_rs: f32;
  var l_hz: vec4f;
  var l_f_29: f32;
  var l_hz3: vec3f;
  var l_hue: vec3f;
  var l_hc: vec3f;
  if ((l_h.w > 0x1.c00000p1f))
  {
    return l_h.z;
  }
  l_p = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_h.x));
  l_l = coax_p_wg_sw_xyz_f4(coax_p_fixture_st(1u));
  l_n = l_h.y;
  l_ndl = coax_p_wg_dot3(l_n, l_l);
  l_character = (l_h.w > 0x1.400000p1f);
  l_lit = coax_p_fixture_shadow(l_p, l_n);
  l_band = coax_p_fixture_surface_band(l_h, l_lit);
  l_key = coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_fixture_st(0u)), (coax_p_fixture_st(0u)).w);
  l_amb = coax_p_v3(coax_p_verse_style_ambient(l_n.y, (coax_p_fixture_st(5u)).x, (coax_p_fixture_st(6u)).x, (coax_p_fixture_st(5u)).w), coax_p_verse_style_ambient(l_n.y, (coax_p_fixture_st(5u)).y, (coax_p_fixture_st(6u)).y, (coax_p_fixture_st(5u)).w), coax_p_verse_style_ambient(l_n.y, (coax_p_fixture_st(5u)).z, (coax_p_fixture_st(6u)).z, (coax_p_fixture_st(5u)).w));
  l_dark = coax_p_wg_mul_f3f3(coax_p_wg_sw_xyz_f4(coax_p_fixture_st(4u)), l_amb);
  l_bright = coax_p_wg_add_f3f3(l_key, l_amb);
  l_a = l_h.z;
  l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
  if (((l_h.w > 0x1.800000p0f) && (l_h.w < 0x1.400000p1f)))
  {
    l_depth = l_h.z.x;
    l_sh = coax_p_fixture_st(27u);
    l_md = coax_p_fixture_st(28u);
    l_dp = coax_p_fixture_st(29u);
    l_s1 = coax_p_smoothstep((l_sh.w - l_dp.w), (l_sh.w + l_dp.w), l_depth);
    l_s2 = coax_p_smoothstep((l_md.w - l_dp.w), (l_md.w + l_dp.w), l_depth);
    l_a = coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(coax_p_v3(l_sh.x, l_sh.y, l_sh.z), coax_p_v3(l_md.x, l_md.y, l_md.z), l_s1), coax_p_v3(l_dp.x, l_dp.y, l_dp.z), l_s2);
    l_c = coax_p_fixture_banded(l_a, l_dark, l_bright, l_band);
    l_r = coax_p_wg_reflect3(l_d, l_n);
    l_f = (0x1.47ae14p-6f + (0x1.f5c290p-1f * coax_pow_bg_vs((0x1.000000p0f - coax_p_fmax(coax_p_wg_dot3(coax_p_wg_neg_f3(l_d), l_n), 0.0f)), 0x1.400000p2f)));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_fixture_sky(l_r, l_l), l_f);
  }
  else
  {
    l_c = coax_p_fixture_banded(l_a, l_dark, l_bright, l_band);
    l_rs = select((coax_p_fixture_st(2u)).w, (coax_p_fixture_st(3u)).x, l_character);
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_fixture_st(2u)), coax_p_verse_style_rim(coax_p_wg_dot3(l_n, coax_p_wg_neg_f3(l_d)), l_ndl, (coax_p_fixture_st(3u)).y, (coax_p_fixture_st(3u)).z, l_rs)), l_lit), select(0x1.000000p0f, 0.0f, (l_h.f4 == 0x1.800000p3f))));
  }
  l_hz = coax_p_fixture_st(9u);
  l_f_29 = coax_p_verse_style_haze(l_h.x, l_p.y, 0.0f, l_hz.x, l_hz.y, l_hz.z, l_hz.w);
  l_hz3 = coax_p_wg_sw_xyz_f4(coax_p_fixture_st(10u));
  l_hue = coax_p_v3(coax_p_verse_style_haze_hue(l_hz3.x, 0.0f, l_f_29, l_hz.z), coax_p_verse_style_haze_hue(l_hz3.y, 0x1.000000p0f, l_f_29, l_hz.z), coax_p_verse_style_haze_hue(l_hz3.z, 0x1.000000p1f, l_f_29, l_hz.z));
  l_hc = coax_p_wg_mix_f3f3f(l_hue, coax_p_wg_sw_xyz_f4(coax_p_fixture_st(11u)), coax_p_verse_style_haze_glow(coax_p_wg_dot3(l_d, l_l)));
  return coax_p_v3(coax_p_verse_style_fog(l_c.x, l_hc.x, l_f_29), coax_p_verse_style_fog(l_c.y, l_hc.y, l_f_29), coax_p_verse_style_fog(l_c.z, l_hc.z, l_f_29));
}
fn coax_p_verse_style_luma(p_r: f32, p_g: f32, p_b: f32) -> f32 {
  var l_r: f32 = p_r;
  var l_g: f32 = p_g;
  var l_b: f32 = p_b;
  return (((0x1.b367a0p-3f * l_r) + (0x1.6e2eb2p-1f * l_g)) + (0x1.27bb30p-4f * l_b));
}
fn coax_p_verse_style_bloom_share(p_lum: f32, p_threshold: f32, p_knee: f32) -> f32 {
  var l_lum: f32 = p_lum;
  var l_threshold: f32 = p_threshold;
  var l_knee: f32 = p_knee;
  var l_k: f32;
  var l_q: f32;
  if ((l_lum <= 0.0f))
  {
    return 0.0f;
  }
  l_k = coax_p_st_max(l_knee, 0x1.4f8b58p-17f);
  l_q = coax_p_st_min(coax_p_st_max(((l_lum - l_threshold) + l_k), 0.0f), (0x1.000000p1f * l_k));
  return coax_p_st_min((coax_p_st_max(((l_q * l_q) / (0x1.000000p2f * l_k)), (l_lum - l_threshold)) / l_lum), 0x1.000000p0f);
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_wg_dims_2d() -> coax_t10 {
  return coax_p_u2(textureDimensions(bloom_src).x, textureDimensions(bloom_src).y);
}
fn coax_p_wg_cvt_f2u2(p_p: coax_t10) -> vec2f {
  var l_p: coax_t10 = p_p;
  return coax_p_v2(f32(l_p.x), f32(l_p.y));
}
fn coax_p_wg_div_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a / l_b.x), (l_a / l_b.y));
}
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t33 {
  var l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
}
fn coax_p_wg_load_2d(p_c: coax_t33, p_level: i32) -> vec4f {
  var l_c: coax_t33 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_light, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_div_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x / l_b.x), (l_a.y / l_b.y));
}
fn coax_p_verse_style_bloom_mix(p_c: f32, p_bloom: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_bloom: f32 = p_bloom;
  var l_amount: f32 = p_amount;
  return (l_c + (l_bloom * l_amount));
}
fn coax_p_fixture_shafts(p_p: vec2f, p_uv: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_uv: vec2f = p_uv;
  var l_sh: vec4f;
  var l_l: vec3f;
  var l_fade: f32;
  var l_n: f32;
  var l_step: vec2f;
  var l_jitter: f32;
  var l_sum: f32;
  var l_i: f32;
  var l_q: vec2f;
  l_sh = coax_p_fixture_st(20u);
  l_l = coax_p_fixture_screen_at(coax_p_wg_sw_xyz_f4(coax_p_fixture_st(1u)));
  l_fade = (coax_p_verse_style_shaft_fade(l_l.x, l_l.y, l_sh.w) * l_l.z);
  if (((l_fade <= 0.0f) || (l_sh.z <= 0.0f)))
  {
    return 0.0f;
  }
  l_n = l_sh.x;
  l_step = coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_l.x, l_l.y), l_uv), l_n);
  l_jitter = coax_p_fract((0x1.a7dd04p5f * coax_p_fract(coax_p_wg_dot2(l_p, coax_p_v2(0x1.12e286p-4f, 0x1.7e8b20p-8f)))));
  l_sum = 0.0f;
  {
    l_i = 0.0f;
    while ((l_i < l_n))
    {
      l_q = coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f(l_step, (l_i + l_jitter)));
      if ((coax_p_wg_any_lt_f2f2(l_q, coax_p_v2(0.0f, 0.0f)) || coax_p_wg_any_gt_f2f2(l_q, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))))
      {
        break;
      }
      l_sum = (l_sum + ((textureSampleLevel(post_aux, post_lin, l_q, 0.0f)).x * coax_p_verse_style_shaft_tap(l_i, l_sh.y)));
      l_i = (l_i + 0x1.000000p0f);
    }
  }
  return coax_p_verse_style_shaft(l_sum, coax_p_verse_style_shaft_total(l_n, l_sh.y), l_sh.z, l_fade);
}
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_fixture_tone(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_e: f32;
  var l_t: vec3f;
  var l_y: f32;
  if (((coax_p_fixture_st(18u)).w < 0x1.000000p-1f))
  {
    return l_c;
  }
  l_e = (coax_p_fixture_st(18u)).x;
  l_t = coax_p_v3(coax_p_verse_style_tone(l_c.x, l_e), coax_p_verse_style_tone(l_c.y, l_e), coax_p_verse_style_tone(l_c.z, l_e));
  l_y = coax_p_verse_style_luma(l_t.x, l_t.y, l_t.z);
  l_t = coax_p_v3(coax_p_verse_style_saturation(l_t.x, l_y, (coax_p_fixture_st(18u)).y), coax_p_verse_style_saturation(l_t.y, l_y, (coax_p_fixture_st(18u)).y), coax_p_verse_style_saturation(l_t.z, l_y, (coax_p_fixture_st(18u)).y));
  return coax_p_v3(coax_p_verse_style_lift(l_t.x, (coax_p_fixture_st(18u)).z), coax_p_verse_style_lift(l_t.y, (coax_p_fixture_st(18u)).z), coax_p_verse_style_lift(l_t.z, (coax_p_fixture_st(18u)).z));
}
fn coax_p_wg_clamp_f3f3f3(p_p0: vec3f, p_p1: vec3f, p_p2: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: vec3f = p_p2;
  return coax_p_v3(coax_p_clampf(l_p0.x, l_p1.x, l_p2.x), coax_p_clampf(l_p0.y, l_p1.y, l_p2.y), coax_p_clampf(l_p0.z, l_p1.z, l_p2.z));
}
fn coax_p_wg_pow_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_pow_bg_vs(l_p0.x, l_p1.x), coax_pow_bg_vs(l_p0.y, l_p1.y), coax_pow_bg_vs(l_p0.z, l_p1.z));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_FIXTURE_SPHERE() -> vec4f {
  return coax_p_v4((-0x1.b33334p0f), 0x1.b33334p-1f, 0.0f, 0x1.b33334p-1f);
}
fn coax_p_FIXTURE_CUBE() -> vec4f {
  return coax_p_v4(0x1.800000p0f, 0x1.333334p-1f, 0x1.333334p-2f, 0x1.333334p-1f);
}
fn coax_p_fixture_canopy(p_i: i32) -> vec4f {
  var l_i: i32 = p_i;
  var l_c: array<vec4f, 3>;
  var l_w: vec4f;
  var l_w2: vec4f;
  var l_t: f32;
  var l_wd: vec2f;
  var l_theta: f32;
  var l_d: f32;
  l_c = array<vec4f, 3>(coax_p_v4(0x1.f33334p1f, 0x1.59999ap1f, (-0x1.4cccccp1f), 0x1.266666p0f), coax_p_v4(0x1.a00000p1f, 0x1.266666p1f, (-0x1.266666p1f), 0x1.99999ap-1f), coax_p_v4(0x1.200000p2f, 0x1.2cccccp1f, (-0x1.733334p1f), 0x1.b33334p-1f));
  l_w = coax_p_fixture_st(24u);
  l_w2 = coax_p_fixture_st(25u);
  l_t = select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
  l_wd = coax_p_v2(l_w2.x, l_w2.y);
  l_theta = coax_p_verse_style_wind(coax_p_wg_dot2(coax_p_wg_sw_xz_f4(coax_p_FIXTURE_TRUNK()), l_wd), coax_p_verse_style_gust((coax_p_FIXTURE_TRUNK()).x, (coax_p_FIXTURE_TRUNK()).z, l_t, l_wd.x, l_wd.y, l_w.z, l_w.w), l_t, l_w.x, l_w.y, l_w.z);
  l_d = coax_p_verse_style_canopy_sway(l_theta, l_w.x, (coax_p_fixture_st(26u)).y, (l_c[i32(l_i)].y / 0x1.ecccccp1f));
  return coax_p_wg_add_f4f4(l_c[i32(l_i)], coax_p_v4((l_wd.x * l_d), 0.0f, (l_wd.y * l_d), 0.0f));
}
fn coax_p_FIXTURE_TRUNK() -> vec4f {
  return coax_p_v4(0x1.f33334p1f, 0.0f, (-0x1.4cccccp1f), 0x1.70a3d8p-3f);
}
fn coax_p_FIXTURE_HEAD() -> vec4f {
  return coax_p_v4((-0x1.333334p-2f), 0x1.9eb852p0f, (-0x1.ccccccp0f), 0x1.5c28f6p-3f);
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_verse_style_shadow_reach(p_on_phone: f32) -> f32 {
  var l_on_phone: f32 = p_on_phone;
  if ((l_on_phone > 0x1.000000p-1f))
  {
    return 0x1.400000p5f;
  }
  return 0x1.e00000p5f;
}
fn coax_p_verse_style_shadow_snap(p_x: f32, p_reach: f32, p_size: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_reach: f32 = p_reach;
  var l_size: f32 = p_size;
  var l_texel: f32;
  l_texel = ((0x1.000000p1f * l_reach) / coax_p_st_max(l_size, 0x1.000000p0f));
  return (floor(((l_x / l_texel) + 0x1.000000p-1f)) * l_texel);
}
fn coax_p_verse_style_shadow_across(p_x: f32, p_reach: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_reach: f32 = p_reach;
  return (l_x / coax_p_st_max(l_reach, 0x1.0624dep-10f));
}
fn coax_p_verse_style_shadow_depth(p_along: f32, p_reach: f32) -> f32 {
  var l_along: f32 = p_along;
  var l_reach: f32 = p_reach;
  return (0x1.000000p-1f - (l_along / (0x1.000000p2f * coax_p_st_max(l_reach, 0x1.0624dep-10f))));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_verse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_FIXTURE_PATH0() -> vec4f {
  return coax_p_v4((-0x1.000000p4f), (-0x1.ccccccp1f), 0x1.000000p1f, (-0x1.4cccccp2f));
}
fn coax_p_verse_style_segment(p_x: f32, p_z: f32, p_ax: f32, p_az: f32, p_bx: f32, p_bz: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_ax: f32 = p_ax;
  var l_az: f32 = p_az;
  var l_bx: f32 = p_bx;
  var l_bz: f32 = p_bz;
  var l_ux: f32;
  var l_uz: f32;
  var l_px: f32;
  var l_pz: f32;
  var l_h: f32;
  var l_dx: f32;
  var l_dz: f32;
  l_ux = (l_bx - l_ax);
  l_uz = (l_bz - l_az);
  l_px = (l_x - l_ax);
  l_pz = (l_z - l_az);
  l_h = coax_p_st_sat((((l_px * l_ux) + (l_pz * l_uz)) / coax_p_st_max(((l_ux * l_ux) + (l_uz * l_uz)), 0x1.0c6f7ap-20f)));
  l_dx = (l_px - (l_ux * l_h));
  l_dz = (l_pz - (l_uz * l_h));
  return sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
}
fn coax_p_FIXTURE_PATH1() -> vec4f {
  return coax_p_v4(0x1.000000p1f, (-0x1.4cccccp2f), 0x1.000000p4f, (-0x1.0cccccp3f));
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
fn coax_p_FIXTURE_POND() -> vec4f {
  return coax_p_v4(0x1.666666p-1f, 0.0f, 0x1.59999ap1f, 0x1.800000p0f);
}
fn coax_p_verse_style_grass_clear(p_past: f32) -> f32 {
  var l_past: f32 = p_past;
  return coax_p_verse_style_soft_step(0x1.000000p-3f, 0x1.000000p-3f, l_past);
}
fn coax_p_verse_style_noise(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_ix: f32;
  var l_iy: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_sx: f32;
  var l_sy: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_ix = floor(l_x);
  l_iy = floor(l_y);
  l_fx = (l_x - l_ix);
  l_fy = (l_y - l_iy);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sy = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  l_a = coax_p_verse_style_hash(l_ix, l_iy);
  l_b = coax_p_verse_style_hash((l_ix + 0x1.000000p0f), l_iy);
  l_c = coax_p_verse_style_hash(l_ix, (l_iy + 0x1.000000p0f));
  l_d = coax_p_verse_style_hash((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f));
  return ((l_a + ((l_b - l_a) * l_sx)) + (((l_c + ((l_d - l_c) * l_sx)) - (l_a + ((l_b - l_a) * l_sx))) * l_sy));
}
fn coax_p_st_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
}
fn coax_p_FIXTURE_GRASS() -> vec4f {
  return coax_p_v4((-0x1.b33334p1f), 0.0f, 0x1.19999ap1f, 0x1.99999ap0f);
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_st_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_fixture_ray(p_p: vec2f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_aspect: f32;
  var l_tv: f32;
  var l_q: vec2f;
  var l_a: f32;
  var l_fwd: vec3f;
  var l_right: vec3f;
  var l_up: vec3f;
  l_aspect = (coax_at_u(i32(0i)) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f));
  l_tv = coax_p_fmax(0x1.70a3d8p-2f, (0x1.000000p-1f / l_aspect));
  l_q = coax_p_v2((((((l_p.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f) * l_tv) * l_aspect), ((0x1.000000p0f - ((l_p.y / coax_at_u(i32(1i))) * 0x1.000000p1f)) * l_tv));
  l_a = ((-0x1.e924f2p-4f) + coax_at_u(i32(8i)));
  l_fwd = coax_p_v3(0.0f, sin(l_a), (-cos(l_a)));
  l_right = coax_p_wg_normalize3(coax_p_wg_cross3(l_fwd, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)));
  l_up = coax_p_wg_cross3(l_right, l_fwd);
  return coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(l_fwd, coax_p_wg_mul_f3f(l_right, l_q.x)), coax_p_wg_mul_f3f(l_up, l_q.y)));
}
fn coax_p_fixture_trace(p_o: vec3f, p_d: vec3f) -> coax_t25 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_h: coax_t25;
  var l_t: f32;
  var l_c: vec4f;
  var l_p: vec3f;
  var l_i: i32;
  var l_c_2: vec4f;
  var l_tc: f32;
  var l_oc: vec3f;
  var l_b: f32;
  var l_thick: f32;
  var l_p_13: vec3f;
  var l_k: f32;
  var l_nc: vec3f;
  var l_nb: vec3f;
  var l_i_17: i32;
  var l_zs: vec3f;
  var l_tz: f32;
  var l_p_20: vec3f;
  var l_tg: f32;
  var l_p_22: vec3f;
  var l_pond: f32;
  var l_grass: f32;
  var l_bare: f32;
  l_h = coax_t25(0x1.dcd650p29f, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f), 0.0f, 0.0f);
  l_t = coax_p_fixture_sphere_t(l_o, l_d, coax_p_FIXTURE_SPHERE());
  if (((l_t > 0.0f) && (l_t < l_h.x)))
  {
    l_h = coax_t25(l_t, coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t)), coax_p_wg_sw_xyz_f4(coax_p_FIXTURE_SPHERE()))), coax_p_v3(0x1.d70a3ep-2f, 0x1.0a3d70p-3f, 0x1.47ae14p-4f), 0x1.000000p0f, 0x1.000000p0f);
  }
  l_c = coax_p_fixture_cube_hit(l_o, l_d);
  if (((l_c.x > 0.0f) && (l_c.x < l_h.x)))
  {
    l_h = coax_t25(l_c.x, coax_p_v3(l_c.y, l_c.z, l_c.w), coax_p_v3(0x1.c28f5cp-4f, 0x1.851eb8p-3f, 0x1.d70a3ep-2f), 0x1.000000p0f, 0x1.000000p1f);
  }
  l_t = coax_p_fixture_cylinder_t(l_o, l_d, coax_p_FIXTURE_TRUNK(), 0x1.e66666p0f);
  if (((l_t > 0.0f) && (l_t < l_h.x)))
  {
    l_p = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t));
    l_h = coax_t25(l_t, coax_p_wg_normalize3(coax_p_v3((l_p.x - (coax_p_FIXTURE_TRUNK()).x), 0.0f, (l_p.z - (coax_p_FIXTURE_TRUNK()).z))), coax_p_v3(0x1.5c28f6p-3f, 0x1.99999ap-4f, 0x1.99999ap-5f), 0x1.000000p0f, 0x1.800000p1f);
  }
  {
    l_i = 0i;
    while ((l_i < 3i))
    {
      l_c_2 = coax_p_fixture_canopy(l_i);
      l_tc = coax_p_fixture_sphere_t(l_o, l_d, l_c_2);
      if ((l_tc > 0.0f))
      {
        l_oc = coax_p_wg_sub_f3f3(l_o, coax_p_v3(l_c_2.x, l_c_2.y, l_c_2.z));
        l_b = coax_p_wg_dot3(l_oc, l_d);
        l_thick = (sqrt(coax_p_fmax((((l_b * l_b) - coax_p_wg_dot3(l_oc, l_oc)) + (l_c_2.w * l_c_2.w)), 0.0f)) / l_c_2.w);
        if (coax_p_fixture_canopy_open(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tc)), l_thick))
        {
          l_tc = (-0x1.000000p0f);
        }
      }
      if (((l_tc > 0x1.0624dep-10f) && (l_tc < l_h.x)))
      {
        l_p_13 = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tc));
        l_k = (coax_p_fixture_st(26u)).x;
        l_nc = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_p_13, coax_p_v3(l_c_2.x, l_c_2.y, l_c_2.z)));
        l_nb = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_p_13, coax_p_FIXTURE_CANOPY_C()));
        l_h = coax_t25(l_tc, coax_p_wg_normalize3(coax_p_v3(coax_p_verse_style_canopy_normal(l_nc.x, l_nb.x, l_k), coax_p_verse_style_canopy_normal(l_nc.y, l_nb.y, l_k), coax_p_verse_style_canopy_normal(l_nc.z, l_nb.z, l_k))), coax_p_v3(0x1.c28f5cp-4f, 0x1.147ae2p-2f, 0x1.99999ap-5f), 0x1.000000p0f, 0x1.000000p2f);
      }
      l_i = (l_i + 1i);
    }
  }
  l_t = coax_p_fixture_capsule_t(l_o, l_d, coax_p_wg_add_f3f3(coax_p_wg_sw_xyz_f4(coax_p_FIXTURE_BODY()), coax_p_v3(0.0f, 0x1.eb851ep-3f, 0.0f)), coax_p_wg_add_f3f3(coax_p_wg_sw_xyz_f4(coax_p_FIXTURE_BODY()), coax_p_v3(0.0f, 0x1.4cccccp0f, 0.0f)), (coax_p_FIXTURE_BODY()).w);
  if (((l_t > 0.0f) && (l_t < l_h.x)))
  {
    l_h = coax_t25(l_t, coax_p_fixture_capsule_n(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t)), coax_p_wg_add_f3f3(coax_p_wg_sw_xyz_f4(coax_p_FIXTURE_BODY()), coax_p_v3(0.0f, 0x1.eb851ep-3f, 0.0f)), coax_p_wg_add_f3f3(coax_p_wg_sw_xyz_f4(coax_p_FIXTURE_BODY()), coax_p_v3(0.0f, 0x1.4cccccp0f, 0.0f))), coax_p_v3(0x1.70a3d8p-2f, 0x1.eb851ep-3f, 0x1.c28f5cp-4f), 0x1.800000p1f, 0x1.400000p2f);
  }
  l_t = coax_p_fixture_sphere_t(l_o, l_d, coax_p_FIXTURE_HEAD());
  if (((l_t > 0.0f) && (l_t < l_h.x)))
  {
    l_h = coax_t25(l_t, coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t)), coax_p_wg_sw_xyz_f4(coax_p_FIXTURE_HEAD()))), coax_p_v3(0x1.3d70a4p-1f, 0x1.851eb8p-2f, 0x1.0a3d70p-2f), 0x1.800000p1f, 0x1.800000p2f);
  }
  l_t = coax_p_fixture_sphere_t(l_o, l_d, coax_p_FIXTURE_LANTERN());
  if (((l_t > 0.0f) && (l_t < l_h.x)))
  {
    l_h = coax_t25(l_t, coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t)), coax_p_wg_sw_xyz_f4(coax_p_FIXTURE_LANTERN()))), coax_p_v3(0x1.99999ap1f, 0x1.e66666p0f, 0x1.99999ap-1f), 0x1.000000p2f, 0x1.600000p3f);
  }
  {
    l_i_17 = 0i;
    while ((l_i_17 < 3i))
    {
      l_zs = coax_p_FIXTURE_RIDGE_Z();
      l_tz = (((-coax_p_wg_get_f3(l_zs, i32(l_i_17))) - l_o.z) / l_d.z);
      if ((((l_d.z < 0.0f) && (l_tz > 0.0f)) && (l_tz < l_h.x)))
      {
        l_p_20 = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tz));
        if ((l_p_20.y < coax_p_fixture_ridge_h(l_p_20.x, l_i_17)))
        {
          l_h = coax_t25(l_tz, coax_p_wg_normalize3(coax_p_v3((-coax_p_fixture_ridge_slope(l_p_20.x, l_i_17)), (0x1.666666p-1f + (0x1.333334p-2f * coax_p_clampf((l_p_20.y / coax_p_fixture_ridge_h(l_p_20.x, l_i_17)), 0.0f, 0x1.000000p0f))), 0x1.000000p-1f)), coax_p_fixture_ridge_albedo(l_i_17), 0x1.000000p0f, (0x1.c00000p2f + f32(l_i_17)));
        }
      }
      l_i_17 = (l_i_17 + 1i);
    }
  }
  if ((l_d.y < 0.0f))
  {
    l_tg = ((-l_o.y) / l_d.y);
    if (((l_tg > 0.0f) && (l_tg < l_h.x)))
    {
      l_p_22 = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tg));
      l_pond = (coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p_22.x, l_p_22.z), coax_p_wg_sw_xz_f4(coax_p_FIXTURE_POND()))) / (coax_p_FIXTURE_POND()).w);
      l_grass = (coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p_22.x, l_p_22.z), coax_p_wg_sw_xz_f4(coax_p_FIXTURE_GRASS()))) / (coax_p_FIXTURE_GRASS()).w);
      if ((l_pond < 0x1.000000p0f))
      {
        l_h = coax_t25(l_tg, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), coax_p_wg_make_f3_f((0x1.e66666p0f * (0x1.000000p0f - (l_pond * l_pond)))), 0x1.000000p1f, 0x1.400000p3f);
      }
      else
      {
        l_bare = (0x1.000000p0f - coax_p_fixture_grass_mask(coax_p_v2(l_p_22.x, l_p_22.z)));
        l_h = coax_t25(l_tg, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(coax_p_fixture_meadow(coax_p_v2(l_p_22.x, l_p_22.z)), 0x1.8f5c28p-1f), coax_p_fixture_path_albedo(coax_p_v2(l_p_22.x, l_p_22.z)), l_bare), 0x1.000000p0f, 0x1.400000p3f);
      }
    }
  }
  return l_h;
}
fn coax_p_fixture_depth_at(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  return ((0x1.f40000p10f / (0x1.f40000p10f - 0x1.99999ap-4f)) * (0x1.000000p0f - (0x1.99999ap-4f / coax_p_fmax(l_z, 0x1.99999ap-4f))));
}
fn coax_p_verse_style_haze(p_d: f32, p_y: f32, p_base_y: f32, p_start: f32, p_density: f32, p_cap: f32, p_falloff: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_y: f32 = p_y;
  var l_base_y: f32 = p_base_y;
  var l_start: f32 = p_start;
  var l_density: f32 = p_density;
  var l_cap: f32 = p_cap;
  var l_falloff: f32 = p_falloff;
  var l_dist: f32;
  var l_height: f32;
  l_dist = (0x1.000000p0f - exp(((-l_density) * coax_p_st_max((l_d - l_start), 0.0f))));
  l_height = exp(((-coax_p_st_max((l_y - l_base_y), 0.0f)) / coax_p_st_max(l_falloff, 0x1.0624dep-10f)));
  return coax_p_st_min((l_dist * (0x1.000000p0f + ((l_height - 0x1.000000p0f) * 0x1.333334p-1f))), l_cap);
}
fn coax_p_fixture_shadow(p_p: vec3f, p_n: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_blob: f32;
  l_blob = (0x1.000000p0f - (coax_p_smoothstep(0x1.333334p-3f, 0x1.19999ap-1f, coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_wg_sw_xz_f4(coax_p_FIXTURE_BODY())))) * coax_p_stepf(l_p.y, 0x1.47ae14p-6f)));
  return coax_p_verse_style_contact(coax_p_fixture_map_lit(l_p, l_n), l_blob, (coax_p_fixture_st(4u)).w);
}
fn coax_p_fixture_surface_band(p_h: coax_t25, p_lit: f32) -> f32 {
  var l_h: coax_t25 = p_h;
  var l_lit: f32 = p_lit;
  var l_w: f32;
  l_w = (coax_p_verse_style_wrap(coax_p_wg_dot3(l_h.y, coax_p_wg_sw_xyz_f4(coax_p_fixture_st(1u)))) * l_lit);
  return select(coax_p_verse_style_bands(l_w, (coax_p_fixture_st(7u)).x, (coax_p_fixture_st(7u)).y, (coax_p_fixture_st(7u)).z, (coax_p_fixture_st(7u)).w), coax_p_verse_style_bands2(l_w, (coax_p_fixture_st(8u)).x, (coax_p_fixture_st(8u)).y), (l_h.w > 0x1.400000p1f));
}
fn coax_p_fixture_sky(p_d: vec3f, p_l: vec3f) -> vec3f {
  var l_d: vec3f = p_d;
  var l_l: vec3f = p_l;
  var l_up: f32;
  var l_h: vec3f;
  var l_z: vec3f;
  var l_t: f32;
  var l_c: vec3f;
  var l_gain: f32;
  var l_cy: f32;
  var l_ct: f32;
  var l_moon: bool;
  var l_disc: f32;
  var l_halo: f32;
  var l_cl: vec4f;
  var l_c2: vec4f;
  var l_t_2: f32;
  var l_p: vec2f;
  var l_n: f32;
  var l_cover: f32;
  var l_toward: vec2f;
  var l_lit: f32;
  var l_night: f32;
  var l_cloud: vec3f;
  l_up = l_d.y;
  l_h = coax_p_wg_sw_xyz_f4(coax_p_fixture_st(13u));
  l_z = coax_p_wg_sw_xyz_f4(coax_p_fixture_st(12u));
  l_t = coax_p_verse_style_sky_t(l_up);
  l_c = coax_p_wg_mix_f3f3f(l_h, l_z, l_t);
  l_gain = coax_p_verse_style_chroma_gain(coax_p_fixture_sky_sat(l_c), coax_p_mix(coax_p_fixture_sky_sat(l_h), coax_p_fixture_sky_sat(l_z), l_t), 0x1.000000p0f);
  l_cy = coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z);
  l_c = coax_p_wg_max_f3f3(coax_p_wg_add_f3f3(coax_p_v3(l_cy, l_cy, l_cy), coax_p_wg_mul_f3f(coax_p_wg_sub_f3f3(l_c, coax_p_v3(l_cy, l_cy, l_cy)), l_gain)), coax_p_v3(0.0f, 0.0f, 0.0f));
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_sw_xyz_f4(coax_p_fixture_st(10u)), coax_p_verse_style_sky_band(l_up, (coax_p_fixture_st(12u)).w));
  l_ct = coax_p_wg_dot3(l_d, l_l);
  l_moon = ((coax_p_fixture_st(1u)).w > 0x1.000000p-1f);
  l_disc = coax_p_smoothstep(cos(((coax_p_fixture_st(14u)).x * 0x1.266666p0f)), cos(((coax_p_fixture_st(14u)).x * 0x1.b33334p-1f)), l_ct);
  l_halo = ((coax_p_verse_style_halo(l_ct, (coax_p_fixture_st(14u)).y) * 0x1.000000p-1f) + (coax_p_verse_style_halo(l_ct, (coax_p_fixture_st(14u)).z) * 0x1.000000p-2f));
  if (l_moon)
  {
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_fixture_st(0u)), l_halo), 0x1.000000p-1f));
  }
  else
  {
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_fixture_st(11u)), 0x1.800000p0f), coax_p_clampf((l_halo * 0x1.19999ap0f), 0.0f, 0x1.ccccccp-1f));
  }
  l_c = coax_p_wg_mix_f3f3f(l_c, select(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.c28f5cp-1f), 0x1.000000p2f), coax_p_wg_mul_f3f(coax_p_v3(0x1.b33334p-1f, 0x1.c28f5cp-1f, 0x1.000000p0f), 0x1.99999ap0f), l_moon), l_disc);
  l_cl = coax_p_fixture_st(15u);
  l_c2 = coax_p_fixture_st(16u);
  if (((l_up > 0x1.47ae14p-7f) && (l_cl.x > 0.0f)))
  {
    l_t_2 = select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
    l_p = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), l_up), l_cl.y), coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_sw_xy_f4(coax_p_fixture_st(25u)), l_cl.z), l_t_2));
    l_n = coax_p_verse_style_cloud_field(l_p.x, l_p.y, l_c2.x, l_c2.y, l_cl.w);
    l_cover = coax_p_verse_style_cloud_cover(l_n, l_cl.x, l_c2.z, l_c2.w);
    l_toward = coax_p_wg_mul_f2f(coax_p_wg_normalize2(coax_p_wg_add_f2f2(coax_p_v2(l_l.x, l_l.z), coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f))), 0x1.040000p8f);
    l_lit = coax_p_verse_style_cloud_lit(l_n, coax_p_verse_style_cloud_field((l_p.x + l_toward.x), (l_p.y + l_toward.y), l_c2.x, l_c2.y, l_cl.w));
    l_night = (coax_p_fixture_st(14u)).w;
    l_cloud = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_fixture_st(4u)), (coax_p_fixture_st(17u)).w), 0x1.ccccccp-1f), coax_p_wg_sw_xyz_f4(coax_p_fixture_st(17u)), l_lit), coax_p_fmax((coax_p_fixture_st(0u)).w, 0x1.333334p-2f)), (0x1.000000p0f - (0x1.800000p-1f * l_night)));
    l_c = coax_p_wg_mix_f3f3f(l_c, l_cloud, ((l_cover * coax_p_verse_style_cloud_fade(l_up)) * (0x1.000000p0f - (0x1.000000p-1f * l_night))));
  }
  return l_c;
}
fn coax_p_verse_style_shaft_source(p_cos_t: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  return exp(((-0x1.800000p4f) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn coax_p_verse_outline_crease(p_facing: f32) -> f32 {
  var l_facing: f32 = p_facing;
  return coax_p_lk_ramp(0x1.800000p-1f, 0x1.19999ap-1f, l_facing);
}
fn coax_p_verse_outline_styled(p_style: f32, p_light: f32, p_edge: f32, p_cap: f32, p_haze: f32) -> f32 {
  var l_style: f32 = p_style;
  var l_light: f32 = p_light;
  var l_edge: f32 = p_edge;
  var l_cap: f32 = p_cap;
  var l_haze: f32 = p_haze;
  if ((l_style < 0x1.000000p-1f))
  {
    return coax_p_verse_outline_ink(l_light, l_edge);
  }
  if ((l_style < 0x1.800000p0f))
  {
    return coax_p_verse_outline_pale(l_light, l_edge, l_cap, l_haze);
  }
  return l_light;
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_fixture_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
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
fn coax_p_verse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_sky: f32 = p_sky;
  var l_ground: f32 = p_ground;
  var l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
}
fn coax_p_fixture_banded(p_a: vec3f, p_dark: vec3f, p_bright: vec3f, p_band: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_dark: vec3f = p_dark;
  var l_bright: vec3f = p_bright;
  var l_band: f32 = p_band;
  var l_s: vec3f;
  var l_t: vec3f;
  var l_tl: f32;
  var l_k: f32;
  var l_m: vec3f;
  var l_y: f32;
  var l_ym: f32;
  var l_cool: vec3f;
  var l_l: vec3f;
  l_s = coax_p_wg_mul_f3f3(l_a, l_dark);
  l_t = coax_p_wg_sw_xyz_f4(coax_p_fixture_st(4u));
  l_tl = coax_p_verse_style_luma(l_t.x, l_t.y, l_t.z);
  l_k = (coax_p_fixture_st(21u)).w;
  l_m = coax_p_v3(coax_p_verse_style_cool(l_s.x, l_t.x, l_tl, l_k), coax_p_verse_style_cool(l_s.y, l_t.y, l_tl, l_k), coax_p_verse_style_cool(l_s.z, l_t.z, l_tl, l_k));
  l_y = coax_p_verse_style_luma(l_s.x, l_s.y, l_s.z);
  l_ym = coax_p_verse_style_luma(l_m.x, l_m.y, l_m.z);
  l_cool = coax_p_v3(coax_p_verse_style_keep_value(l_m.x, l_y, l_ym), coax_p_verse_style_keep_value(l_m.y, l_y, l_ym), coax_p_verse_style_keep_value(l_m.z, l_y, l_ym));
  l_l = coax_p_wg_mul_f3f3(l_a, l_bright);
  return coax_p_v3(coax_p_verse_style_banded(l_cool.x, l_l.x, l_band), coax_p_verse_style_banded(l_cool.y, l_l.y, l_band), coax_p_verse_style_banded(l_cool.z, l_l.z, l_band));
}
fn coax_p_wg_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_reflect3(l_i, l_n);
}
fn coax_p_wg_neg_f3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_v3((-l_a.x), (-l_a.y), (-l_a.z));
}
fn coax_p_verse_style_rim(p_ndv: f32, p_ndl: f32, p_start: f32, p_full: f32, p_strength: f32) -> f32 {
  var l_ndv: f32 = p_ndv;
  var l_ndl: f32 = p_ndl;
  var l_start: f32 = p_start;
  var l_full: f32 = p_full;
  var l_strength: f32 = p_strength;
  var l_t: f32;
  var l_edge: f32;
  l_t = coax_p_st_sat(((l_ndv - l_start) / (l_full - l_start)));
  l_edge = ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
  return ((l_edge * (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_st_sat((l_ndl + 0x1.333334p-2f))))) * l_strength);
}
fn coax_p_verse_style_haze_hue(p_h: f32, p_ch: f32, p_f: f32, p_cap: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_ch: f32 = p_ch;
  var l_f: f32 = p_f;
  var l_cap: f32 = p_cap;
  var l_t: f32;
  var l_far: f32;
  l_t = coax_p_verse_style_haze_teal(l_ch);
  l_far = coax_p_verse_style_soft_step(((0x1.333334p-3f + l_cap) * 0x1.000000p-1f), ((l_cap - 0x1.333334p-3f) * 0x1.000000p-1f), l_f);
  return (l_h * (l_t + ((0x1.000000p0f - l_t) * l_far)));
}
fn coax_p_verse_style_haze_glow(p_vdl: f32) -> f32 {
  var l_vdl: f32 = p_vdl;
  var l_a: f32;
  var l_a2: f32;
  var l_a4: f32;
  l_a = coax_p_st_sat(l_vdl);
  l_a2 = (l_a * l_a);
  l_a4 = (l_a2 * l_a2);
  return ((l_a4 * l_a4) * 0x1.ccccccp-2f);
}
fn coax_p_verse_style_fog(p_c: f32, p_haze_c: f32, p_f: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_haze_c: f32 = p_haze_c;
  var l_f: f32 = p_f;
  return (l_c + ((l_haze_c - l_c) * coax_p_st_sat(l_f)));
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
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t33 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t33(l_x, l_y);
}
fn coax_p_fixture_screen_at(p_d: vec3f) -> vec3f {
  var l_d: vec3f = p_d;
  var l_aspect: f32;
  var l_tv: f32;
  var l_a: f32;
  var l_fwd: vec3f;
  var l_right: vec3f;
  var l_up: vec3f;
  var l_z: f32;
  var l_q: vec2f;
  l_aspect = (coax_at_u(i32(0i)) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f));
  l_tv = coax_p_fmax(0x1.70a3d8p-2f, (0x1.000000p-1f / l_aspect));
  l_a = ((-0x1.e924f2p-4f) + coax_at_u(i32(8i)));
  l_fwd = coax_p_v3(0.0f, sin(l_a), (-cos(l_a)));
  l_right = coax_p_wg_normalize3(coax_p_wg_cross3(l_fwd, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)));
  l_up = coax_p_wg_cross3(l_right, l_fwd);
  l_z = coax_p_wg_dot3(l_d, l_fwd);
  l_q = coax_p_wg_div_f2f(coax_p_v2(coax_p_wg_dot3(l_d, l_right), coax_p_wg_dot3(l_d, l_up)), coax_p_fmax(l_z, 0x1.a36e2ep-14f));
  return coax_p_v3((((l_q.x / (l_tv * l_aspect)) + 0x1.000000p0f) * 0x1.000000p-1f), ((0x1.000000p0f - (l_q.y / l_tv)) * 0x1.000000p-1f), select(0.0f, 0x1.000000p0f, (l_z > 0x1.47ae14p-7f)));
}
fn coax_p_verse_style_shaft_fade(p_x: f32, p_y: f32, p_margin: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_margin: f32 = p_margin;
  var l_out: f32;
  l_out = coax_p_st_max(coax_p_st_max(coax_p_st_max((-l_x), (l_x - 0x1.000000p0f)), coax_p_st_max((-l_y), (l_y - 0x1.000000p0f))), 0.0f);
  return (0x1.000000p0f - coax_p_verse_style_soft_step((l_margin * 0x1.000000p-1f), (l_margin * 0x1.000000p-1f), l_out));
}
fn coax_p_wg_any_lt_f2f2(p_a: vec2f, p_b: vec2f) -> bool {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x < l_b.x) || (l_a.y < l_b.y));
}
fn coax_p_wg_any_gt_f2f2(p_a: vec2f, p_b: vec2f) -> bool {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x > l_b.x) || (l_a.y > l_b.y));
}
fn coax_p_verse_style_shaft_tap(p_i: f32, p_decay: f32) -> f32 {
  var l_i: f32 = p_i;
  var l_decay: f32 = p_decay;
  return coax_pow_bg_vs(coax_p_st_max(l_decay, 0x1.0c6f7ap-20f), l_i);
}
fn coax_p_verse_style_shaft_total(p_n: f32, p_decay: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_decay: f32 = p_decay;
  if ((abs((0x1.000000p0f - l_decay)) < 0x1.0c6f7ap-20f))
  {
    return l_n;
  }
  return ((0x1.000000p0f - coax_pow_bg_vs(coax_p_st_max(l_decay, 0x1.0c6f7ap-20f), l_n)) / (0x1.000000p0f - l_decay));
}
fn coax_p_verse_style_shaft(p_sum: f32, p_total: f32, p_weight: f32, p_fade: f32) -> f32 {
  var l_sum: f32 = p_sum;
  var l_total: f32 = p_total;
  var l_weight: f32 = p_weight;
  var l_fade: f32 = p_fade;
  return (((l_sum / coax_p_st_max(l_total, 0x1.0c6f7ap-20f)) * l_weight) * coax_p_st_sat(l_fade));
}
fn coax_p_verse_style_tone(p_c: f32, p_exposure: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_exposure: f32 = p_exposure;
  return (0x1.000000p0f - exp(((-coax_p_st_max(l_c, 0.0f)) * l_exposure)));
}
fn coax_p_verse_style_saturation(p_c: f32, p_lum: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lum: f32 = p_lum;
  var l_amount: f32 = p_amount;
  return coax_p_st_max(0.0f, (l_lum + ((l_c - l_lum) * l_amount)));
}
fn coax_p_verse_style_lift(p_c: f32, p_lift: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lift: f32 = p_lift;
  return (l_lift + ((0x1.000000p0f - l_lift) * l_c));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_verse_style_canopy_sway(p_theta: f32, p_amp: f32, p_reach: f32, p_share: f32) -> f32 {
  var l_theta: f32 = p_theta;
  var l_amp: f32 = p_amp;
  var l_reach: f32 = p_reach;
  var l_share: f32 = p_share;
  return ((l_reach * coax_p_st_sat(l_share)) * coax_p_st_max((-0x1.000000p0f), coax_p_st_min(0x1.000000p0f, (l_theta / coax_p_st_max(l_amp, 0x1.a36e2ep-14f)))));
}
fn coax_p_wg_add_f4f4(p_a: vec4f, p_b: vec4f) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return coax_p_v4((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_v: f32;
  l_v = (sin(((l_x * 0x1.fc6666p6f) + (l_y * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
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
fn coax_p_fixture_sphere_t(p_o: vec3f, p_d: vec3f, p_s: vec4f) -> f32 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_s: vec4f = p_s;
  var l_oc: vec3f;
  var l_b: f32;
  var l_c: f32;
  var l_h: f32;
  var l_t: f32;
  l_oc = coax_p_wg_sub_f3f3(l_o, coax_p_v3(l_s.x, l_s.y, l_s.z));
  l_b = coax_p_wg_dot3(l_oc, l_d);
  l_c = (coax_p_wg_dot3(l_oc, l_oc) - (l_s.w * l_s.w));
  l_h = ((l_b * l_b) - l_c);
  if ((l_h < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  l_t = ((-l_b) - sqrt(l_h));
  return select((-0x1.000000p0f), l_t, (l_t > 0x1.0624dep-10f));
}
fn coax_p_fixture_cube_hit(p_o: vec3f, p_d: vec3f) -> vec4f {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_lo: vec3f;
  var l_ld: vec3f;
  var l_inv: vec3f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_t0: vec3f;
  var l_t1: vec3f;
  var l_tn: f32;
  var l_tf: f32;
  var l_n: vec3f;
  l_lo = coax_p_fixture_turn(coax_p_wg_sub_f3f3(l_o, coax_p_wg_sw_xyz_f4(coax_p_FIXTURE_CUBE())), (-0x1.000000p-1f));
  l_ld = coax_p_fixture_turn(l_d, (-0x1.000000p-1f));
  l_inv = coax_p_wg_div_ff3(0x1.000000p0f, l_ld);
  l_a = coax_p_wg_mul_f3f3(coax_p_wg_sub_f3f3(coax_p_wg_neg_f3(coax_p_wg_make_f3_f((coax_p_FIXTURE_CUBE()).w)), l_lo), l_inv);
  l_b = coax_p_wg_mul_f3f3(coax_p_wg_sub_f3f3(coax_p_wg_make_f3_f((coax_p_FIXTURE_CUBE()).w), l_lo), l_inv);
  l_t0 = coax_p_wg_min_f3f3(l_a, l_b);
  l_t1 = coax_p_wg_max_f3f3(l_a, l_b);
  l_tn = coax_p_fmax(coax_p_fmax(l_t0.x, l_t0.y), l_t0.z);
  l_tf = coax_p_fmin(coax_p_fmin(l_t1.x, l_t1.y), l_t1.z);
  if (((l_tn > l_tf) || (l_tn < 0x1.0624dep-10f)))
  {
    return coax_p_v4((-0x1.000000p0f), (-0x1.000000p0f), (-0x1.000000p0f), (-0x1.000000p0f));
  }
  l_n = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((l_tn == l_t0.x))
  {
    l_n = coax_p_v3((-coax_p_wg_sign(l_ld.x)), 0.0f, 0.0f);
  }
  else
  {
    if ((l_tn == l_t0.y))
    {
      l_n = coax_p_v3(0.0f, (-coax_p_wg_sign(l_ld.y)), 0.0f);
    }
    else
    {
      l_n = coax_p_v3(0.0f, 0.0f, (-coax_p_wg_sign(l_ld.z)));
    }
  }
  return coax_p_wg_make_f4_ff3(l_tn, coax_p_fixture_turn(l_n, 0x1.000000p-1f));
}
fn coax_p_fixture_cylinder_t(p_o: vec3f, p_d: vec3f, p_c: vec4f, p_top: f32) -> f32 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_c: vec4f = p_c;
  var l_top: f32 = p_top;
  var l_oc: vec2f;
  var l_a: f32;
  var l_b: f32;
  var l_k: f32;
  var l_h: f32;
  var l_t: f32;
  var l_y: f32;
  l_oc = coax_p_wg_sub_f2f2(coax_p_v2(l_o.x, l_o.z), coax_p_v2(l_c.x, l_c.z));
  l_a = coax_p_wg_dot2(coax_p_v2(l_d.x, l_d.z), coax_p_v2(l_d.x, l_d.z));
  l_b = coax_p_wg_dot2(l_oc, coax_p_v2(l_d.x, l_d.z));
  l_k = (coax_p_wg_dot2(l_oc, l_oc) - (l_c.w * l_c.w));
  l_h = ((l_b * l_b) - (l_a * l_k));
  if (((l_h < 0.0f) || (l_a < 0x1.0c6f7ap-20f)))
  {
    return (-0x1.000000p0f);
  }
  l_t = (((-l_b) - sqrt(l_h)) / l_a);
  l_y = (l_o.y + (l_d.y * l_t));
  return select((-0x1.000000p0f), l_t, (((l_t > 0x1.0624dep-10f) && (l_y >= l_c.y)) && (l_y <= l_top)));
}
fn coax_p_fixture_canopy_open(p_p: vec3f, p_thick: f32) -> bool {
  var l_p: vec3f = p_p;
  var l_thick: f32 = p_thick;
  var l_q: vec3f;
  var l_a: f32;
  var l_n: f32;
  l_q = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_p, coax_p_FIXTURE_CANOPY_C()));
  l_a = coax_p_wg_atan2(l_q.z, l_q.x);
  l_n = ((coax_p_verse_style_noise(((l_a * 0x1.066666p2f) + 0x1.600000p3f), ((l_q.y * 0x1.800000p2f) + 0x1.000000p1f)) * 0x1.333334p-1f) + (coax_p_verse_style_noise(((l_a * 0x1.29999ap3f) + 0x1.800000p1f), ((l_q.y * 0x1.900000p3f) + 0x1.400000p2f)) * 0x1.99999ap-2f));
  return (coax_p_verse_style_canopy_open((l_n + ((0x1.c28f5cp-3f * l_thick) * l_thick)), (coax_p_fixture_st(26u)).z) > 0x1.000000p-1f);
}
fn coax_p_FIXTURE_CANOPY_C() -> vec3f {
  return coax_p_v3(0x1.f33334p1f, 0x1.400000p1f, (-0x1.4cccccp1f));
}
fn coax_p_verse_style_canopy_normal(p_mesh_n: f32, p_blob_n: f32, p_k: f32) -> f32 {
  var l_mesh_n: f32 = p_mesh_n;
  var l_blob_n: f32 = p_blob_n;
  var l_k: f32 = p_k;
  return (l_mesh_n + ((l_blob_n - l_mesh_n) * l_k));
}
fn coax_p_fixture_capsule_t(p_o: vec3f, p_d: vec3f, p_a: vec3f, p_b: vec3f, p_r: f32) -> f32 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_r: f32 = p_r;
  var l_ba: vec3f;
  var l_oa: vec3f;
  var l_baba: f32;
  var l_bard: f32;
  var l_baoa: f32;
  var l_rdoa: f32;
  var l_oaoa: f32;
  var l_qa: f32;
  var l_qb: f32;
  var l_qc: f32;
  var l_h: f32;
  var l_t: f32;
  var l_y: f32;
  var l_oc: vec3f;
  var l_bb: f32;
  var l_cc: f32;
  var l_hh: f32;
  var l_t2: f32;
  l_ba = coax_p_wg_sub_f3f3(l_b, l_a);
  l_oa = coax_p_wg_sub_f3f3(l_o, l_a);
  l_baba = coax_p_wg_dot3(l_ba, l_ba);
  l_bard = coax_p_wg_dot3(l_ba, l_d);
  l_baoa = coax_p_wg_dot3(l_ba, l_oa);
  l_rdoa = coax_p_wg_dot3(l_d, l_oa);
  l_oaoa = coax_p_wg_dot3(l_oa, l_oa);
  l_qa = (l_baba - (l_bard * l_bard));
  l_qb = ((l_baba * l_rdoa) - (l_baoa * l_bard));
  l_qc = (((l_baba * l_oaoa) - (l_baoa * l_baoa)) - ((l_r * l_r) * l_baba));
  l_h = ((l_qb * l_qb) - (l_qa * l_qc));
  if ((l_h >= 0.0f))
  {
    l_t = (((-l_qb) - sqrt(l_h)) / l_qa);
    l_y = (l_baoa + (l_t * l_bard));
    if ((((l_y > 0.0f) && (l_y < l_baba)) && (l_t > 0x1.0624dep-10f)))
    {
      return l_t;
    }
    l_oc = select(coax_p_wg_sub_f3f3(l_o, l_b), l_oa, (l_y <= 0.0f));
    l_bb = coax_p_wg_dot3(l_d, l_oc);
    l_cc = (coax_p_wg_dot3(l_oc, l_oc) - (l_r * l_r));
    l_hh = ((l_bb * l_bb) - l_cc);
    if ((l_hh > 0.0f))
    {
      l_t2 = ((-l_bb) - sqrt(l_hh));
      if ((l_t2 > 0x1.0624dep-10f))
      {
        return l_t2;
      }
    }
  }
  return (-0x1.000000p0f);
}
fn coax_p_fixture_capsule_n(p_p: vec3f, p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_ba: vec3f;
  var l_h: f32;
  l_ba = coax_p_wg_sub_f3f3(l_b, l_a);
  l_h = coax_p_clampf((coax_p_wg_dot3(coax_p_wg_sub_f3f3(l_p, l_a), l_ba) / coax_p_wg_dot3(l_ba, l_ba)), 0.0f, 0x1.000000p0f);
  return coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_p, coax_p_wg_add_f3f3(l_a, coax_p_wg_mul_f3f(l_ba, l_h))));
}
fn coax_p_FIXTURE_LANTERN() -> vec4f {
  return coax_p_v4((-0x1.266666p1f), 0x1.47ae14p-3f, 0x1.e66666p0f, 0x1.47ae14p-3f);
}
fn coax_p_FIXTURE_RIDGE_Z() -> vec3f {
  return coax_p_v3(0x1.e00000p5f, 0x1.cc0000p6f, 0x1.040000p8f);
}
fn coax_p_wg_get_f3(p_p: vec3f, p_i: i32) -> f32 {
  var l_p: vec3f = p_p;
  var l_i: i32 = p_i;
  if ((l_i == 0i))
  {
    return l_p.x;
  }
  if ((l_i == 1i))
  {
    return l_p.y;
  }
  return l_p.z;
}
fn coax_p_fixture_ridge_h(p_x: f32, p_i: i32) -> f32 {
  var l_x: f32 = p_x;
  var l_i: i32 = p_i;
  var l_f: f32;
  var l_bases: array<f32, 3>;
  var l_base: f32;
  var l_s: f32;
  var l_gaps: array<vec2f, 3>;
  var l_g: vec2f;
  var l_open: f32;
  l_f = f32(l_i);
  l_bases = array<f32, 3>(0x1.200000p3f, 0x1.500000p4f, 0x1.600000p5f);
  l_base = l_bases[i32(l_i)];
  l_s = (0x1.800000p3f * (0x1.000000p0f + (l_f * l_f)));
  l_gaps = array<vec2f, 3>(coax_p_v2(0x1.c00000p3f, 0x1.e00000p4f), coax_p_v2(0x1.800000p4f, 0x1.800000p5f), coax_p_v2(0.0f, 0.0f));
  l_g = l_gaps[i32(l_i)];
  l_open = select(0x1.000000p0f, coax_p_smoothstep(l_g.x, l_g.y, abs(l_x)), (l_g.y > 0.0f));
  return ((l_open * l_base) * ((0x1.70a3d8p-1f + (0x1.70a3d8p-3f * sin(((l_x / l_s) + (l_f * 0x1.b33334p0f))))) + (0x1.47ae14p-4f * sin((((l_x / (0x1.7ae148p-2f * l_s)) + 0x1.333334p-1f) + l_f)))));
}
fn coax_p_fixture_ridge_slope(p_x: f32, p_i: i32) -> f32 {
  var l_x: f32 = p_x;
  var l_i: i32 = p_i;
  return (coax_p_fixture_ridge_h((l_x + 0x1.000000p-1f), l_i) - coax_p_fixture_ridge_h((l_x - 0x1.000000p-1f), l_i));
}
fn coax_p_fixture_ridge_albedo(p_i: i32) -> vec3f {
  var l_i: i32 = p_i;
  var l_a: array<vec3f, 3>;
  l_a = array<vec3f, 3>(coax_p_v3(0x1.70a3d8p-4f, 0x1.99999ap-3f, 0x1.eb851ep-5f), coax_p_v3(0x1.47ae14p-4f, 0x1.5c28f6p-3f, 0x1.eb851ep-4f), coax_p_v3(0x1.5c28f6p-2f, 0x1.ae147ap-2f, 0x1.000000p-1f));
  return l_a[i32(l_i)];
}
fn coax_p_fixture_path_albedo(p_q: vec2f) -> vec3f {
  var l_q: vec2f = p_q;
  return coax_p_wg_mix_f3f3f(coax_p_v3(0x1.333334p-2f, 0x1.ae147ap-3f, 0x1.99999ap-4f), coax_p_v3(0x1.0a3d70p-2f, 0x1.99999ap-3f, 0x1.c28f5cp-4f), (0x1.000000p-1f + (0x1.000000p-1f * sin((l_q.x * 0x1.5c28f6p-3f)))));
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
fn coax_p_fixture_map_lit(p_p: vec3f, p_n: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_q: vec3f;
  var l_uv: vec2f;
  var l_texel_2: f32;
  var l_phone: f32;
  var l_taps: f32;
  var l_r: f32;
  var l_lit: f32;
  var l_k: f32;
  var l_o: vec2f;
  if (((coax_p_fixture_st(21u)).x < 0x1.000000p0f))
  {
    return 0x1.000000p0f;
  }
  l_q = coax_p_fixture_shadow_place(coax_p_wg_add_f3f3(l_p, coax_p_wg_mul_f3f(l_n, 0x1.99999ap-5f)), coax_p_fixture_light_space());
  l_uv = coax_p_v2((0x1.000000p-1f + (0x1.000000p-1f * l_q.x)), (0x1.000000p-1f - (0x1.000000p-1f * l_q.y)));
  if ((coax_p_wg_any_lt_f2f2(l_uv, coax_p_v2(0.0f, 0.0f)) || coax_p_wg_any_gt_f2f2(l_uv, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))))
  {
    return 0x1.000000p0f;
  }
  l_texel_2 = (0x1.000000p0f / f32((coax_p_wg_dims_depth()).x));
  l_phone = (coax_p_fixture_st(31u)).z;
  l_taps = (coax_p_fixture_st(21u)).y;
  l_r = (coax_p_fixture_st(21u)).z;
  l_lit = 0.0f;
  {
    l_k = 0.0f;
    while ((l_k < l_taps))
    {
      l_o = coax_p_v2(coax_p_verse_style_shadow_tap(l_k, l_phone, 0.0f), coax_p_verse_style_shadow_tap(l_k, l_phone, 0x1.000000p0f));
      l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_o, l_r), l_texel_2)), (l_q.z - 0x1.a36e2ep-12f)));
      l_k = (l_k + 0x1.000000p0f);
    }
  }
  return (l_lit / coax_p_fmax(l_taps, 0x1.000000p0f));
}
fn coax_p_verse_style_contact(p_cast_lit: f32, p_blob: f32, p_strength: f32) -> f32 {
  var l_cast_lit: f32 = p_cast_lit;
  var l_blob: f32 = p_blob;
  var l_strength: f32 = p_strength;
  return coax_p_st_min(l_cast_lit, (0x1.000000p0f - (l_strength * coax_p_st_sat(l_blob))));
}
fn coax_p_verse_style_wrap(p_ndl: f32) -> f32 {
  var l_ndl: f32 = p_ndl;
  return coax_p_st_sat(((l_ndl + 0x1.000000p-2f) / 0x1.400000p0f));
}
fn coax_p_verse_style_bands2(p_w: f32, p_e: f32, p_s: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  return coax_p_verse_style_soft_step(l_e, l_s, l_w);
}
fn coax_p_verse_style_bands(p_w: f32, p_e0: f32, p_e1: f32, p_s: f32, p_mid: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_e0: f32 = p_e0;
  var l_e1: f32 = p_e1;
  var l_s: f32 = p_s;
  var l_mid: f32 = p_mid;
  return ((l_mid * coax_p_verse_style_soft_step(l_e0, l_s, l_w)) + ((0x1.000000p0f - l_mid) * coax_p_verse_style_soft_step(l_e1, l_s, l_w)));
}
fn coax_p_verse_style_sky_t(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return coax_pow_bg_vs(coax_p_st_max(l_up, 0x1.0c6f7ap-20f), 0x1.19999ap-1f);
}
fn coax_p_fixture_sky_sat(p_c: vec3f) -> f32 {
  var l_c: vec3f = p_c;
  var l_hi: f32;
  l_hi = coax_p_fmax(l_c.x, coax_p_fmax(l_c.y, l_c.z));
  return ((l_hi - coax_p_fmin(l_c.x, coax_p_fmin(l_c.y, l_c.z))) / coax_p_fmax(l_hi, 0x1.a36e2ep-14f));
}
fn coax_p_verse_style_chroma_gain(p_have: f32, p_albedo: f32, p_keep: f32) -> f32 {
  var l_have: f32 = p_have;
  var l_albedo: f32 = p_albedo;
  var l_keep: f32 = p_keep;
  if ((l_have < 0x1.a36e2ep-14f))
  {
    return 0x1.000000p0f;
  }
  return coax_p_st_min(0x1.400000p1f, coax_p_st_max(0x1.000000p0f, ((l_albedo * l_keep) / l_have)));
}
fn coax_p_wg_max_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y), coax_p_fmax(l_p0.z, l_p1.z));
}
fn coax_p_verse_style_sky_band(p_up: f32, p_lift: f32) -> f32 {
  var l_up: f32 = p_up;
  var l_lift: f32 = p_lift;
  return (l_lift * (0x1.000000p0f - coax_p_st_sat((abs(l_up) / 0x1.ac0832p-4f))));
}
fn coax_p_verse_style_halo(p_cos_t: f32, p_k: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  var l_k: f32 = p_k;
  return exp(((-l_k) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn coax_p_wg_sw_xy_f4(p_p: vec4f) -> vec2f {
  var l_p: vec4f = p_p;
  return coax_p_v2(l_p.x, l_p.y);
}
fn coax_p_verse_style_cloud_field(p_x: f32, p_z: f32, p_s1: f32, p_s2: f32, p_octaves: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_s1: f32 = p_s1;
  var l_s2: f32 = p_s2;
  var l_octaves: f32 = p_octaves;
  var l_n: f32;
  l_n = coax_p_verse_style_noise((l_x * l_s1), (l_z * l_s1));
  if ((l_octaves < 0x1.800000p0f))
  {
    return l_n;
  }
  return ((l_n * 0x1.4cccccp-1f) + (coax_p_verse_style_noise(((l_x * l_s2) + 0x1.8cccccp1f), ((l_z * l_s2) + 0x1.ecccccp2f)) * 0x1.666666p-2f));
}
fn coax_p_verse_style_cloud_cover(p_n: f32, p_cover: f32, p_cut_lo: f32, p_cut_hi: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_cover: f32 = p_cover;
  var l_cut_lo: f32 = p_cut_lo;
  var l_cut_hi: f32 = p_cut_hi;
  return (coax_p_verse_style_soft_step(((l_cut_lo + l_cut_hi) * 0x1.000000p-1f), ((l_cut_hi - l_cut_lo) * 0x1.000000p-1f), (l_n + ((l_cover - 0x1.666666p-2f) * 0x1.99999ap-2f))) * coax_p_verse_style_soft_step(0x1.99999ap-6f, 0x1.99999ap-6f, l_cover));
}
fn coax_p_verse_style_cloud_lit(p_n: f32, p_toward: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_toward: f32 = p_toward;
  return coax_p_st_sat((((l_n - l_toward) * 0x1.400000p2f) + 0x1.19999ap-1f));
}
fn coax_p_verse_style_cloud_fade(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return (coax_p_verse_style_soft_step(0x1.5c28f6p-4f, 0x1.333334p-4f, l_up) * 0x1.d70a3ep-1f);
}
fn coax_p_lk_ramp(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_lk_min(0x1.000000p0f, coax_p_lk_max(0.0f, ((l_x - l_a) / (l_b - l_a))));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_verse_outline_ink(p_light: f32, p_edge: f32) -> f32 {
  var l_light: f32 = p_light;
  var l_edge: f32 = p_edge;
  return (l_light * (0x1.000000p0f - (0x1.a3d70ap-1f * coax_p_lk_min(0x1.000000p0f, coax_p_lk_max(0.0f, l_edge)))));
}
fn coax_p_verse_outline_pale(p_light: f32, p_edge: f32, p_cap: f32, p_haze: f32) -> f32 {
  var l_light: f32 = p_light;
  var l_edge: f32 = p_edge;
  var l_cap: f32 = p_cap;
  var l_haze: f32 = p_haze;
  var l_w: f32;
  l_w = ((coax_p_lk_min(coax_p_lk_min(0x1.000000p0f, coax_p_lk_max(0.0f, l_edge)), 0x1.000000p0f) * coax_p_lk_min(coax_p_lk_max(l_cap, 0.0f), 0x1.666666p-2f)) * (0x1.000000p0f - coax_p_lk_min(0x1.000000p0f, coax_p_lk_max(0.0f, l_haze))));
  return (l_light + (l_w * (coax_p_lk_max(l_light, 0.0f) + 0x1.000000p-2f)));
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
fn coax_p_verse_style_cool(p_c: f32, p_tint: f32, p_tint_lum: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_tint: f32 = p_tint;
  var l_tint_lum: f32 = p_tint_lum;
  var l_amount: f32 = p_amount;
  return (l_c * coax_pow_bg_vs((coax_p_st_max(l_tint, 0x1.0c6f7ap-20f) / coax_p_st_max(l_tint_lum, 0x1.0c6f7ap-20f)), coax_p_st_max(l_amount, 0.0f)));
}
fn coax_p_verse_style_keep_value(p_c: f32, p_before: f32, p_after: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_before: f32 = p_before;
  var l_after: f32 = p_after;
  return ((l_c * l_before) / coax_p_st_max(l_after, 0x1.0c6f7ap-20f));
}
fn coax_p_verse_style_banded(p_shadow_c: f32, p_lit_c: f32, p_band: f32) -> f32 {
  var l_shadow_c: f32 = p_shadow_c;
  var l_lit_c: f32 = p_lit_c;
  var l_band: f32 = p_band;
  return (l_shadow_c + ((l_lit_c - l_shadow_c) * coax_p_st_sat(l_band)));
}
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_i))));
}
fn coax_p_verse_style_haze_teal(p_ch: f32) -> f32 {
  var l_ch: f32 = p_ch;
  if ((l_ch < 0x1.000000p-1f))
  {
    return 0x1.666666p-1f;
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return 0x1.000000p0f;
  }
  return 0x1.ae147ap-1f;
}
fn coax_p_wg_div_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a / l_b.x), (l_a / l_b.y), (l_a / l_b.z));
}
fn coax_p_wg_min_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmin(l_p0.x, l_p1.x), coax_p_fmin(l_p0.y, l_p1.y), coax_p_fmin(l_p0.z, l_p1.z));
}
fn coax_p_wg_sign(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x > 0.0f))
  {
    return 0x1.000000p0f;
  }
  if ((l_x < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return 0.0f;
}
fn coax_p_wg_make_f4_ff3(p_p0: f32, p_p1: vec3f) -> vec4f {
  var l_p0: f32 = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v4(l_p0, l_p1.x, l_p1.y, l_p1.z);
}
fn coax_p_wg_atan2(p_y: f32, p_x: f32) -> f32 {
  var l_y: f32 = p_y;
  var l_x: f32 = p_x;
  var l_ax: f32;
  var l_ay: f32;
  var l_high: f32;
  var l_r: f32;
  l_ax = abs(l_x);
  l_ay = abs(l_y);
  l_high = coax_p_fmax(l_ax, l_ay);
  if ((l_high == 0.0f))
  {
    return 0.0f;
  }
  l_r = coax_p_wg_atan01((coax_p_fmin(l_ax, l_ay) / l_high));
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
fn coax_p_verse_style_canopy_open(p_n: f32, p_gap: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_gap: f32 = p_gap;
  return select(0.0f, 0x1.000000p0f, (l_n < l_gap));
}
fn coax_p_wg_dims_depth() -> coax_t10 {
  return coax_p_u2(textureDimensions(shadow_map).x, textureDimensions(shadow_map).y);
}
fn coax_p_verse_style_shadow_tap(p_k: f32, p_on_phone: f32, p_which: f32) -> f32 {
  var l_k: f32 = p_k;
  var l_on_phone: f32 = p_on_phone;
  var l_which: f32 = p_which;
  var l_x: f32;
  var l_y: f32;
  l_x = 0.0f;
  l_y = 0.0f;
  if ((l_on_phone > 0x1.000000p-1f))
  {
    if ((l_k < 0x1.000000p-1f))
    {
      l_x = (-0x1.333334p-1f);
      l_y = (-0x1.000000p-2f);
    }
    else
    {
      if ((l_k < 0x1.800000p0f))
      {
        l_x = 0x1.000000p-2f;
        l_y = (-0x1.333334p-1f);
      }
      else
      {
        if ((l_k < 0x1.400000p1f))
        {
          l_x = 0x1.333334p-1f;
          l_y = 0x1.000000p-2f;
        }
        else
        {
          l_x = (-0x1.000000p-2f);
          l_y = 0x1.333334p-1f;
        }
      }
    }
  }
  else
  {
    if ((l_k < 0x1.000000p-1f))
    {
      l_x = 0.0f;
      l_y = 0.0f;
    }
    else
    {
      if ((l_k < 0x1.800000p0f))
      {
        l_x = (-0x1.4dd2f2p-2f);
        l_y = (-0x1.9fbe76p-2f);
      }
      else
      {
        if ((l_k < 0x1.400000p1f))
        {
          l_x = (-0x1.ae147ap-1f);
          l_y = (-0x1.2f1aa0p-4f);
        }
        else
        {
          if ((l_k < 0x1.c00000p1f))
          {
            l_x = (-0x1.645a1cp-1f);
            l_y = 0x1.d3f7cep-2f;
          }
          else
          {
            if ((l_k < 0x1.200000p2f))
            {
              l_x = (-0x1.9fbe76p-3f);
              l_y = 0x1.3df3b6p-1f;
            }
            else
            {
              if ((l_k < 0x1.600000p2f))
              {
                l_x = 0x1.ec8b44p-1f;
                l_y = (-0x1.8f5c28p-3f);
              }
              else
              {
                if ((l_k < 0x1.a00000p2f))
                {
                  l_x = 0x1.e45a1cp-2f;
                  l_y = (-0x1.eb851ep-2f);
                }
                else
                {
                  if ((l_k < 0x1.e00000p2f))
                  {
                    l_x = 0x1.09ba5ep-1f;
                    l_y = 0x1.88b43ap-1f;
                  }
                  else
                  {
                    l_x = 0x1.7ae148p-3f;
                    l_y = (-0x1.c9374cp-1f);
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if ((l_which > 0x1.000000p-1f))
  {
    return l_y;
  }
  return l_x;
}
fn coax_p_lk_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lk_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
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
fn caster_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_caster_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn grass_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_13 {
  let coax_r = coax_p_grass_vs(coax_vid, coax_iid);
  return coax_io_13(coax_r.x, coax_r.y, coax_r.z, coax_r.w);
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_18 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_18(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_19) -> coax_io_20 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t19(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_20(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_24 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_24(coax_r.x, coax_r.y, coax_r.z);
}
@fragment
fn grass_fs(coax_in: coax_io_13) -> coax_out_26 {
  let coax_r = coax_p_grass_fs(coax_t13(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_out_26(coax_r.x, coax_r.y);
}
@fragment
fn scene_fs(coax_in: coax_io_18) -> coax_out_26 {
  let coax_r = coax_p_scene_fs(coax_t18(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
  return coax_out_26(coax_r.x, coax_r.y);
}
@fragment
fn bloom_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bloom_fs(coax_pos);
  return coax_r;
}
@fragment
fn post_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_post_fs(coax_pos);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_20) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t20(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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