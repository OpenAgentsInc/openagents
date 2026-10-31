// ---- generated: verse/everglade/gpu.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t15 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec3f,
  f4: u32,
  f5: vec2f,
  f6: u32,
  f7: vec4f,
  f8: vec3f,
  f9: u32,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
  @location(3) @interpolate(flat) f4: u32,
  @location(4) f5: vec2f,
  @location(5) @interpolate(flat) f6: u32,
  @location(6) @interpolate(flat) f7: vec4f,
  @location(7) f8: vec3f,
  @location(8) @interpolate(flat) f9: u32,
}
struct coax_t16 {
  x: vec4f,
  y: vec2f,
  z: u32,
  w: u32,
  f4: vec3f,
  f5: u32,
  f6: f32,
}
struct coax_io_16 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) @interpolate(flat) z: u32,
  @location(2) @interpolate(flat) w: u32,
  @location(3) f4: vec3f,
  @location(4) @interpolate(flat) f5: u32,
  @location(5) @interpolate(flat) f6: f32,
}
struct coax_t17 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_17 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t18 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_18 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t19 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_19 {
  @builtin(position) x: vec4f,
  @location(0) y: vec4f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t20 {
  x: vec3f,
  y: f32,
  z: f32,
  w: vec2f,
  f4: f32,
}
struct coax_t21 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec3f,
  f4: vec3f,
}
struct coax_io_21 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
  @location(3) f4: vec3f,
}
struct coax_t22 {
  x: vec3f,
  y: f32,
  z: vec3f,
  w: f32,
  f4: f32,
}
struct coax_t23 {
  x: vec4f,
  y: vec4f,
}
struct coax_out_23 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
struct coax_t29 {
  x: u32,
  y: u32,
}
struct coax_t32 {
  x: vec3f,
  y: vec3f,
}
struct coax_t33 {
  x: u32,
  y: u32,
  z: u32,
  w: u32,
}
struct coax_t34 {
  x: vec3f,
  y: vec3f,
  z: vec3f,
  w: f32,
  f4: f32,
  f5: f32,
}
struct coax_t35 {
  x: vec3f,
  y: vec3f,
  z: f32,
}
struct coax_t38 {
  x: i32,
  y: i32,
}
struct coax_t39 {
  x: vec3f,
  y: vec3f,
  z: vec2f,
  w: vec4f,
}
struct coax_t40 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t42 {
  x: u32,
  y: u32,
  z: u32,
}
struct coax_t45 {
  x: i32,
  y: i32,
  z: i32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 575>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> heights: array<f32>;
@group(1) @binding(2) var<storage, read> objects: array<coax_t11>;
@group(1) @binding(3) var<storage, read> pose: array<vec4f>;
@group(1) @binding(4) var<storage, read> water_nodes: array<f32>;
@group(1) @binding(5) var sheets: texture_2d_array<f32>;
@group(1) @binding(6) var sheet: sampler;
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
@group(3) @binding(1) var water_scene_depth: texture_depth_2d;
@group(3) @binding(0) var water_scene: texture_2d<f32>;
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
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  return coax_p_eg_place(l_id, l_instance);
}
fn coax_p_shadow_vs(p_id: u32, p_instance: u32) -> coax_t16 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_o: coax_t16;
  var l_n: u32;
  var l_v: coax_t15;
  var l_q: vec3f;
  l_o = coax_t16();
  l_n = coax_p_wg_u32_f(coax_at_u(i32(32i)));
  if (((l_instance < (l_n * l_n)) || ((coax_p_eg_st(21u)).x < 0x1.000000p0f)))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, (-0x1.000000p0f), 0x1.000000p0f);
    return l_o;
  }
  l_v = coax_p_eg_place(l_id, l_instance);
  if ((((l_v.f4 == 15u) || (l_v.f4 == 16u)) || coax_p_eg_spell_soft(l_v.f4)))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, (-0x1.000000p0f), 0x1.000000p0f);
    return l_o;
  }
  l_q = coax_p_eg_shadow_place(l_v.y, coax_p_eg_light_space());
  l_o.x = coax_p_wg_make_f4_f2ff(coax_p_v2(l_q.x, l_q.y), l_q.z, 0x1.000000p0f);
  l_o.y = l_v.f5;
  l_o.z = l_v.f6;
  l_o.f4 = l_v.f8;
  l_o.f5 = l_v.f9;
  l_o.w = select(0u, 1u, (((((l_v.f4 == 5u) || (l_v.f4 == 7u)) || (l_v.f4 == 8u)) || (l_v.f4 == 10u)) || (l_v.f4 == 14u)));
  l_o.f6 = select(0.0f, l_v.f7.z, (l_v.f4 == 7u));
  return l_o;
}
fn coax_p_fx_vs(p__vid4: u32, p__iid5: u32, p_in_: coax_t17) -> coax_t18 {
  var l__vid4: u32 = p__vid4;
  var l__iid5: u32 = p__iid5;
  var l_in_: coax_t17 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t18;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t18();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_water_vs(p_id: u32, p_piece: u32) -> coax_t19 {
  var l_id: u32 = p_id;
  var l_piece: u32 = p_piece;
  var l_o: coax_t19;
  var l_patches: u32;
  var l_qa: u32;
  var l_ql: u32;
  var l_quad_id: u32;
  var l_c: vec2f;
  var l_qx: u32;
  var l_qz: u32;
  var l_across: f32;
  var l_along: f32;
  var l_base: f32;
  var l_uv: vec2f;
  var l_low: f32;
  var l_k: u32;
  var l_corner: vec2f;
  var l_r: coax_t20;
  var l_wave: vec4f;
  var l_world: vec3f;
  l_o = coax_t19();
  l_patches = coax_p_wg_u32_f(coax_at_u(i32(1077i)));
  if ((l_piece >= (l_patches + 64u)))
  {
    return coax_p_eg_fx_vertex(l_id, ((l_piece - l_patches) - 64u));
  }
  if ((l_piece >= (l_patches + 32u)))
  {
    return coax_p_eg_pond_vertex(l_id, ((l_piece - l_patches) - 32u));
  }
  if ((l_piece >= l_patches))
  {
    return coax_p_eg_fountain_vertex(l_id, (l_piece - l_patches));
  }
  l_qa = coax_p_wg_u32_f(coax_at_u(i32(1074i)));
  l_ql = coax_p_wg_u32_f(coax_at_u(i32(1075i)));
  l_quad_id = (l_id / 6u);
  l_c = coax_p_eg_quad((l_id % 6u));
  l_qx = (l_quad_id % l_qa);
  l_qz = (l_quad_id / l_qa);
  l_across = ((coax_at_u(i32(1072i)) - 0x1.000000p0f) / f32(l_qa));
  l_along = (coax_at_u(i32(1076i)) / f32(l_ql));
  l_base = (f32(l_piece) * coax_at_u(i32(1076i)));
  l_uv = coax_p_v2(((f32(l_qx) + l_c.x) * l_across), (l_base + ((f32(l_qz) + l_c.y) * l_along)));
  l_low = 0x1.f40000p9f;
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      l_corner = coax_p_v2((f32((l_qx + (l_k & 1u))) * l_across), (l_base + (f32((l_qz + (l_k >> u32(1u)))) * l_along)));
      l_low = coax_p_fmin(l_low, (-(coax_p_eg_water_rest(l_corner)).y));
      l_k = (l_k + 1u);
    }
  }
  l_r = coax_p_eg_water_rest(l_uv);
  if ((((l_low - coax_at_u(i32(1480i))) > 0x1.333334p-1f) || (l_quad_id >= (l_qa * l_ql))))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    return l_o;
  }
  l_wave = coax_p_eg_water_gerstner(coax_p_v2(l_r.x.x, l_r.x.z), l_r.y, 0x1.000000p0f);
  l_world = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(l_r.x, coax_p_v3(l_wave.x, l_wave.y, l_wave.z)), coax_p_v3(0.0f, coax_at_u(i32(1480i)), 0.0f));
  l_o.x = coax_p_eg_clip(l_world);
  l_o.y = coax_p_v4(l_world.x, l_world.y, l_world.z, (l_r.y + coax_at_u(i32(1480i))));
  l_o.z = coax_p_wg_make_f4_f2f2(coax_p_v2(l_r.x.x, l_r.x.z), l_r.w);
  l_o.w = coax_p_v4(l_r.f4, 0.0f, l_wave.w, l_r.z);
  l_o.f4 = coax_p_v4(0.0f, (l_world.y - l_r.x.y), 0.0f, 0.0f);
  return l_o;
}
fn coax_p_grass_vs(p_v: u32, p_i: u32) -> coax_t21 {
  var l_v: u32 = p_v;
  var l_i: u32 = p_i;
  var l_o: coax_t21;
  var l_g: vec4f;
  var l_g2: vec4f;
  var l_w: vec4f;
  var l_w2: vec4f;
  var l_per: u32;
  var l_r: f32;
  var l_side: u32;
  var l_reach: f32;
  var l_far_n: u32;
  var l_near: bool;
  var l_nper: u32;
  var l_nside: u32;
  var l_j: u32;
  var l_bper: u32;
  var l_bside: u32;
  var l_half: f32;
  var l_cell: u32;
  var l_b: f32;
  var l_c: vec2f;
  var l_k: vec2f;
  var l_q: vec2f;
  var l_rel: vec2f;
  var l_dist: f32;
  var l_root_clip: vec4f;
  var l_mk: coax_t22;
  var l_tall: f32;
  var l_t: f32;
  var l_wd: vec2f;
  var l_theta: f32;
  var l_lean: vec2f;
  var l_m: i32;
  var l_sh: vec4f;
  var l_away: vec2f;
  var l_ang: f32;
  var l_across: vec3f;
  var l_world: vec3f;
  var l_base: vec3f;
  var l_hj: f32;
  var l_a: vec3f;
  var l_l: vec3f;
  var l_lx: vec2f;
  var l_tilt: f32;
  l_o = coax_t21();
  l_o.x = coax_p_v4(0.0f, 0.0f, (-0x1.000000p0f), 0x1.000000p0f);
  l_o.y = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.z = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_o.w = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.f4 = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_g = coax_p_eg_st(22u);
  l_g2 = coax_p_eg_st(23u);
  l_w = coax_p_eg_st(24u);
  l_w2 = coax_p_eg_st(25u);
  l_per = coax_p_wg_max_u(coax_p_wg_u32_f(l_g.x), 1u);
  l_r = l_g.y;
  l_side = coax_p_wg_u32_f((0x1.000000p1f * l_r));
  l_reach = coax_p_verse_style_near_reach((coax_p_eg_st(31u)).z);
  l_far_n = ((l_per * l_side) * l_side);
  l_near = (l_i >= l_far_n);
  l_nper = coax_p_wg_max_u(coax_p_wg_u32_f((coax_p_eg_st(26u)).w), 1u);
  l_nside = coax_p_wg_u32_f((0x1.000000p1f * l_reach));
  l_j = select(l_i, (l_i - l_far_n), l_near);
  l_bper = select(l_per, l_nper, l_near);
  l_bside = select(l_side, l_nside, l_near);
  l_half = select(l_r, l_reach, l_near);
  l_cell = (l_j / l_bper);
  l_b = (f32((l_j % l_bper)) + select(0.0f, 0x1.940000p6f, l_near));
  l_c = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_floor_f2(coax_p_v2(coax_at_u(i32(24i)), coax_at_u(i32(26i)))), coax_p_v2(l_half, l_half)), coax_p_v2(f32((l_cell % l_bside)), f32((l_cell / l_bside))));
  l_k = coax_p_wg_add_f2f2(l_c, coax_p_v2((l_b * 0x1.c851ecp2f), (l_b * (-0x1.dae148p1f))));
  l_q = coax_p_wg_add_f2f2(l_c, coax_p_v2(coax_p_eg_hash(l_k), coax_p_eg_hash(coax_p_wg_add_f2f2(l_k, coax_p_v2(0x1.31999ap4f, 0x1.ecccccp2f)))));
  l_rel = coax_p_wg_sub_f2f2(l_q, coax_p_v2(coax_at_u(i32(24i)), coax_at_u(i32(26i))));
  l_dist = coax_p_wg_length2(l_rel);
  l_root_clip = coax_p_eg_clip(coax_p_v3(l_q.x, coax_p_eg_ground_y(l_q), l_q.y));
  if (((l_root_clip.w < (-0x1.000000p0f)) || (abs(l_root_clip.x) > ((l_root_clip.w * 0x1.333334p0f) + 0x1.000000p0f))))
  {
    return l_o;
  }
  l_mk = coax_p_eg_ground_mark(l_q, select(0.0f, 0x1.000000p0f, (l_v == 2u)));
  l_tall = (((coax_p_verse_style_blade_height(coax_p_eg_hash(coax_p_wg_add_f2f2(l_k, coax_p_v2(0x1.a66666p1f, 0x1.4f3334p5f))), l_g.w, l_g2.x) * coax_p_verse_style_grass_fade(l_dist, l_r, l_g.z)) * coax_p_eg_grass_mask(l_q)) * (0x1.000000p0f - l_mk.w));
  if ((l_tall < 0x1.47ae14p-6f))
  {
    return l_o;
  }
  l_t = select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
  l_wd = coax_p_v2(l_w2.x, l_w2.y);
  l_theta = coax_p_verse_style_wind(coax_p_wg_dot2(l_q, l_wd), coax_p_verse_style_gust(l_q.x, l_q.y, l_t, l_wd.x, l_wd.y, l_w.z, l_w.w), l_t, l_w.x, l_w.y, l_w.z);
  l_lean = coax_p_wg_mul_f2f(l_wd, coax_p_verse_style_bend(l_theta, l_tall, 0x1.000000p0f));
  {
    l_m = 0i;
    while ((l_m < 8i))
    {
      l_sh = coax_p_wg_at4(((i32(l_m) * 4i) + 36i));
      if ((l_sh.z <= 0.0f))
      {
        break;
      }
      l_away = coax_p_wg_sub_f2f2(l_q, coax_p_v2(l_sh.x, l_sh.y));
      l_lean = coax_p_wg_add_f2f2(l_lean, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_normalize2(coax_p_wg_add_f2f2(l_away, coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f))), l_tall), 0x1.b33334p-1f), coax_p_verse_style_part(coax_p_wg_length2(l_away), l_w2.w)));
      l_m = (l_m + 1i);
    }
  }
  l_ang = (coax_p_eg_hash(coax_p_wg_add_f2f2(l_k, coax_p_v2(0x1.79999ap2f, 0x1.a9999ap3f))) * 0x1.921fb6p2f);
  l_across = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(cos(l_ang), 0.0f, sin(l_ang)), l_g2.y), 0x1.000000p-1f), coax_p_verse_style_near_width(l_dist, l_reach));
  l_world = coax_p_v3(l_q.x, coax_p_eg_ground_y(l_q), l_q.y);
  l_base = coax_p_eg_meadow(l_q);
  l_hj = coax_p_eg_hash(coax_p_wg_add_f2f2(l_k, coax_p_v2(0x1.fb3334p4f, 0x1.733334p1f)));
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
  l_o.w = coax_p_wg_mix_f3f3f(l_o.w, coax_p_wg_pow_f3f3(l_mk.x, coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f)), l_mk.y);
  l_o.f4 = l_mk.z;
  l_l = coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u));
  l_lx = coax_p_wg_normalize2(coax_p_wg_add_f2f2(coax_p_v2(l_l.x, l_l.z), coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f)));
  l_tilt = coax_p_verse_style_grass_tilt(l_theta, l_w.x, l_w2.z);
  l_o.z = coax_p_wg_normalize3(coax_p_v3((l_lx.x * sin(l_tilt)), cos(l_tilt), (l_lx.y * sin(l_tilt))));
  l_o.x = coax_p_eg_clip(l_world);
  l_o.y = l_world;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t23 {
  var l_pos: vec4f = p_pos;
  var l_d: vec3f;
  var l_l: vec3f;
  var l_c: vec3f;
  var l_b: vec4f;
  l_d = coax_p_eg_pixel_ray(coax_p_v2(l_pos.x, l_pos.y));
  l_l = coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u));
  l_c = coax_p_eg_water_under_view(coax_p_eg_weather_streaks(coax_p_eg_weather_sky(coax_p_eg_ridges(l_d, coax_p_eg_sky(l_d, l_l)), l_d), l_d, 0x1.f40000p9f), coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), coax_p_wg_mul_f3f(l_d, 0x1.f40000p9f)), l_d);
  l_b = coax_p_eg_st(19u);
  return coax_t23(coax_p_v4(l_c.x, l_c.y, l_c.z, coax_p_verse_style_bloom_share(coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z), l_b.x, l_b.y)), coax_p_v4(coax_p_verse_style_shaft_source(coax_p_wg_dot3(l_d, l_l)), 0.0f, 0.0f, 0.0f));
}
fn coax_p_shadow_fs(p_v: coax_t16) {
  var l_v: coax_t16 = p_v;
  if (((l_v.w == 1u) && ((textureSampleLevel(sheets, sheet, l_v.y, i32(l_v.z), 0.0f)).w < 0x1.666666p-2f)))
  {
    discard;
  }
  if (((l_v.f5 != 0u) && (coax_p_eg_shard_inside(l_v.f5, l_v.f4) < 0.0f)))
  {
    discard;
  }
  if (coax_p_eg_leaf_gone(l_v.f4, l_v.f6))
  {
    discard;
  }
}
fn coax_p_scene_fs(p_s: coax_t15, p_front: bool) -> coax_t23 {
  var l_s: coax_t15 = p_s;
  var l_front: bool = p_front;
  var l_paint_leaf_far: bool;
  var l_tex: vec4f;
  var l_facing: f32;
  var l_tint: vec3f;
  var l_rim: vec3f;
  var l_c: vec3f;
  var l_n: vec3f;
  var l_base: vec3f;
  var l_shadow: f32;
  var l_mark_ice: f32;
  var l_mark_glow: vec3f;
  var l_keep: f32;
  var l_bayer: array<f32, 16>;
  var l_px: coax_t29;
  var l_inside: f32;
  var l_rough: f32;
  var l_p: vec3f;
  var l_mottle: f32;
  var l_moss: f32;
  var l_xz: vec2f;
  var l_broad: f32;
  var l_q: i32;
  var l_sq: vec4f;
  var l_paved: f32;
  var l_stone: vec3f;
  var l_d: f32;
  var l_half: f32;
  var l_edge: f32;
  var l_dirt: vec3f;
  var l_under: f32;
  var l_mud: vec3f;
  var l_wet: f32;
  var l_mk: coax_t22;
  var l_glow: f32;
  var l_f: vec3f;
  var l_lit: f32;
  var l_res: coax_t23;
  var l_v: vec3f;
  var l_fres: f32;
  var l_key: vec3f;
  var l_figure: bool;
  var l_night: f32;
  var l_near: f32;
  var l_face: f32;
  var l_fill: vec3f;
  var l_spill: f32;
  l_paint_leaf_far = false;
  l_tex = textureSampleBias(sheets, sheet, l_s.f5, i32(l_s.f6), select(0.0f, 0x1.000000p1f, (l_s.f4 == 8u)));
  if ((l_s.f4 == 15u))
  {
    l_facing = abs(coax_p_wg_dot3(coax_p_wg_normalize3(l_s.z), coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y))));
    l_tint = coax_p_wg_pow_f3f3(coax_p_wg_max_f3f3(l_s.w, coax_p_v3(0.0f, 0.0f, 0.0f)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f));
    l_rim = coax_p_wg_mul_f3f3(l_tint, coax_p_v3(0x1.000000p0f, 0x1.666666p-2f, 0x1.99999ap-3f));
    l_c = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(l_rim, 0x1.000000p-1f), coax_p_wg_mix_f3f3f(l_tint, coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.ccccccp-1f), ((l_facing * l_facing) * 0x1.333334p-1f)), coax_p_smoothstep(0.0f, 0x1.666666p-1f, l_facing)), (0x1.99999ap-1f + (0x1.333334p1f * l_facing)));
    return coax_t23(coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
  }
  if ((l_s.f4 == 16u))
  {
    return coax_p_eg_orb(l_s);
  }
  if (coax_p_eg_spell_soft(l_s.f4))
  {
    return coax_p_eg_spell_soft_part(l_s, l_paint_leaf_far);
  }
  l_n = coax_p_wg_normalize3(l_s.z);
  l_base = l_s.w;
  l_shadow = 0x1.000000p0f;
  l_mark_ice = 0.0f;
  l_mark_glow = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((((((l_s.f4 == 5u) || (l_s.f4 == 7u)) || (l_s.f4 == 8u)) || (l_s.f4 == 10u)) || (l_s.f4 == 14u)))
  {
    if ((l_tex.w < 0x1.666666p-2f))
    {
      discard;
    }
    if ((l_s.f4 == 7u))
    {
      l_keep = coax_p_smoothstep(0x1.666666p0f, 0x1.99999ap1f, coax_p_wg_length3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))))));
      l_bayer = array<f32, 16>(0.0f, 0x1.000000p3f, 0x1.000000p1f, 0x1.400000p3f, 0x1.800000p3f, 0x1.000000p2f, 0x1.c00000p3f, 0x1.800000p2f, 0x1.800000p1f, 0x1.600000p3f, 0x1.000000p0f, 0x1.200000p3f, 0x1.e00000p3f, 0x1.c00000p2f, 0x1.a00000p3f, 0x1.400000p2f);
      l_px = coax_p_wg_mod_u2u2(coax_p_wg_cvt_u2f2(coax_p_v2(l_s.x.x, l_s.x.y)), coax_p_u2(4u, 4u));
      if ((((l_bayer[i32(((l_px.y * 4u) + l_px.x))] + 0x1.000000p-1f) / 0x1.000000p4f) >= l_keep))
      {
        discard;
      }
      if (coax_p_eg_leaf_gone(l_s.f8, l_s.f7.z))
      {
        discard;
      }
    }
    l_base = coax_p_wg_mul_f3f3(coax_p_eg_person_dye(coax_p_v3(l_tex.x, l_tex.y, l_tex.z), l_s.f7), l_s.w);
    if (((!l_front) && (l_s.f4 != 7u)))
    {
      l_n = coax_p_wg_neg_f3(l_n);
    }
    if ((l_s.f9 != 0u))
    {
      l_inside = coax_p_eg_shard_inside(l_s.f9, l_s.f8);
      if ((l_inside < 0.0f))
      {
        discard;
      }
      l_rough = coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_s.f8.x, l_s.f8.y), 0x1.200000p3f), coax_p_wg_mul_f2f(coax_p_v2(l_s.f8.z, l_s.f8.x), 0x1.800000p2f)));
      if ((!l_front))
      {
        l_n = coax_p_eg_shard_cap(l_s.f9, l_s.f8);
        l_base = coax_p_wg_mul_f3f(l_base, (0x1.851eb8p-2f + (0x1.eb851ep-3f * l_rough)));
      }
      else
      {
        l_base = coax_p_wg_mul_f3f(l_base, coax_p_mix((0x1.000000p-1f + (0x1.99999ap-3f * l_rough)), 0x1.000000p0f, coax_p_smoothstep(0.0f, 0x1.99999ap-5f, l_inside)));
      }
    }
    if ((l_n.y > 0x1.666666p-1f))
    {
      l_shadow = coax_p_eg_shade_shadows_at(coax_p_v2(l_s.y.x, l_s.y.z), l_s.y.y, true);
    }
  }
  if ((l_s.f4 == 10u))
  {
    l_p = l_s.y;
    l_mottle = ((coax_p_eg_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_p.x, l_p.z), 0x1.ccccccp-1f), (l_p.y * 0x1.666666p-1f))) * 0x1.333334p-1f) + (coax_p_eg_noise(coax_p_wg_sub_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_p.x, l_p.z), 0x1.066666p2f), (l_p.y * 0x1.a66666p1f))) * 0x1.99999ap-2f));
    l_base = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(l_base, coax_p_v3(0x1.3d70a4p-1f, 0x1.333334p-1f, 0x1.19999ap-1f)), (0x1.8f5c28p-1f + (0x1.5c28f6p-2f * l_mottle)));
    l_moss = coax_p_smoothstep(0x1.666666p-2f, 0x1.99999ap-1f, (l_n.y + (0x1.666666p-2f * (coax_p_eg_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_p.x, l_p.z), 0x1.266666p1f), l_p.y)) - 0x1.000000p-1f))));
    l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_wg_mul_f3f(coax_p_v3(0x1.c28f5cp-4f, 0x1.99999ap-3f, 0x1.1eb852p-4f), (0x1.99999ap-1f + (0x1.99999ap-2f * l_mottle))), (l_moss * 0x1.b33334p-1f));
  }
  l_xz = coax_p_v2(l_s.y.x, l_s.y.z);
  if ((l_s.f4 == 99u))
  {
    l_broad = coax_p_eg_noise(coax_p_wg_mul_f2f(l_xz, 0x1.0a3d70p-3f));
    l_base = coax_p_wg_mul_f3f(coax_p_eg_meadow_srgb(l_broad), (0x1.eb851ep-1f + (0x1.47ae14p-4f * coax_p_eg_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(l_xz, 0x1.d70a3ep-3f), 0x1.c00000p2f)))));
    {
      l_q = 0i;
      while ((l_q < 4i))
      {
        l_sq = coax_p_wg_at4(((i32(l_q) * 4i) + 232i));
        if ((l_sq.z <= 0.0f))
        {
          break;
        }
        l_paved = (0x1.000000p0f - coax_p_smoothstep((l_sq.z - 0x1.333334p-2f), (l_sq.z + 0x1.333334p-2f), coax_p_wg_length2(coax_p_wg_sub_f2f2(l_xz, coax_p_v2(l_sq.x, l_sq.y)))));
        l_stone = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.a3d70ap-2f, 0x1.8f5c28p-2f, 0x1.666666p-2f), coax_p_v3(0x1.0f5c28p-1f, 0x1.f5c290p-2f, 0x1.ae147ap-2f), ((coax_p_eg_noise(coax_p_wg_mul_f2f(l_xz, 0x1.b33334p0f)) * 0x1.333334p-1f) + (coax_p_eg_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(l_xz, 0x1.99999ap-3f), 0x1.400000p2f)) * 0x1.99999ap-2f)));
        l_base = coax_p_wg_mix_f3f3f(l_base, l_stone, l_paved);
        l_q = (l_q + 1i);
      }
    }
    l_d = coax_p_eg_path_distance(l_xz);
    l_half = coax_at_u(i32(229i));
    l_edge = (0x1.000000p0f - coax_p_smoothstep((l_half - 0x1.eb851ep-4f), (l_half + 0x1.eb851ep-4f), l_d));
    if ((l_edge > 0.0f))
    {
      l_dirt = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.d70a3ep-2f, 0x1.666666p-2f, 0x1.d70a3ep-3f), coax_p_v3(0x1.28f5c2p-1f, 0x1.d70a3ep-2f, 0x1.333334p-2f), ((coax_p_eg_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(l_xz, 0x1.ae147ap-3f), 0x1.800000p1f)) * 0x1.333334p-1f) + (coax_p_eg_noise(coax_p_wg_mul_f2f(l_xz, 0x1.47ae14p-4f)) * 0x1.99999ap-2f)));
      l_base = coax_p_wg_mix_f3f3f(l_base, l_dirt, l_edge);
    }
    l_under = coax_p_eg_water_over(l_xz);
    l_mud = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.1eb852p-2f, 0x1.d70a3ep-3f, 0x1.3d70a4p-3f), coax_p_v3(0x1.5c28f6p-2f, 0x1.1eb852p-2f, 0x1.851eb8p-3f), coax_p_eg_noise(coax_p_wg_mul_f2f(l_xz, 0x1.eb851ep-3f)));
    l_wet = coax_p_smoothstep((-0x1.47ae14p-6f), 0x1.000000p-2f, l_under);
    l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_wg_mul_f3f(l_mud, (0x1.000000p0f - (0x1.ccccccp-2f * coax_p_smoothstep(0x1.333334p-2f, 0x1.000000p1f, l_under)))), l_wet);
    l_mk = coax_p_eg_ground_mark(l_xz, 0.0f);
    l_base = coax_p_wg_mix_f3f3f(l_base, l_mk.x, l_mk.y);
    l_mark_ice = l_mk.f4;
    l_mark_glow = l_mk.z;
    l_shadow = coax_p_eg_shade_shadows_at(l_xz, l_s.y.y, true);
  }
  else
  {
    if ((l_s.f4 == 1u))
    {
      l_base = coax_p_wg_mul_f3f(l_base, (0x1.99999ap-1f + (0x1.666666p-2f * coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.x, l_s.y.y), 0x1.8cccccp1f), coax_p_wg_mul_f2f(coax_p_v2(l_s.y.z, l_s.y.x), 0x1.266666p1f))))));
    }
    else
    {
      if ((l_s.f4 == 2u))
      {
        l_base = coax_p_wg_mul_f3f(l_base, (0x1.ccccccp-1f + (0x1.333334p-3f * coax_p_eg_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.x, l_s.y.z), 0x1.200000p3f), (l_s.y.y * 0x1.c00000p2f))))));
      }
      else
      {
        if ((l_s.f4 == 9u))
        {
          l_base = coax_p_eg_carved(l_base, l_s.y, l_s.f5);
        }
        else
        {
          if (((l_s.f4 == 11u) || (l_s.f4 == 12u)))
          {
            l_base = coax_p_eg_timber(l_base, l_s.y, l_n, (l_s.f4 == 11u));
          }
          else
          {
            if ((l_s.f4 == 4u))
            {
              l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_v3(0x1.b33334p-1f, 0x1.d70a3ep-1f, 0x1.e66666p-1f), (0x1.000000p-2f * coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_xz, 0x1.800000p0f), coax_p_v2((coax_at_u(i32(2i)) * 0x1.99999ap-3f), 0.0f)))));
            }
          }
        }
      }
    }
  }
  if ((((((l_s.f4 != 5u) && (l_s.f4 != 7u)) && (l_s.f4 != 8u)) && (l_s.f4 != 10u)) && (l_s.f4 != 14u)))
  {
    l_base = coax_p_wg_pow_f3f3(coax_p_wg_max_f3f3(l_base, coax_p_v3(0.0f, 0.0f, 0.0f)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f));
  }
  if ((l_s.f4 == 13u))
  {
    l_glow = coax_at_u(i32(1341i));
    l_f = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_base, (0x1.666666p-2f + (0x1.c00000p2f * l_glow))), (0x1.b33334p-1f + (0x1.333334p-2f * coax_p_eg_noise(coax_p_v2((coax_at_u(i32(2i)) * 0x1.800000p2f), ((l_s.y.y * 0x1.200000p3f) + l_s.y.x))))));
    return coax_t23(coax_p_v4(l_f.x, l_f.y, l_f.z, select(0.0f, 0x1.ccccccp-1f, (l_glow > 0x1.99999ap-5f))), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
  }
  if (((l_s.f4 == 14u) && (l_s.f7.y > 0x1.800000p-1f)))
  {
    return coax_p_eg_glass(l_s, l_front);
  }
  l_paint_leaf_far = (((l_s.f4 == 7u) && ((coax_p_eg_st(31u)).z > 0x1.000000p-1f)) && (coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y)) > 0x1.800000p4f));
  l_lit = coax_p_verse_style_contact(coax_p_eg_map_lit(l_s.y, l_n, l_paint_leaf_far), (0x1.000000p0f - l_shadow), (coax_p_eg_st(4u)).w);
  l_res = coax_p_eg_paint(l_base, l_n, l_s.y, l_lit, ((l_s.f4 == 8u) || (l_s.f4 == 3u)), (l_s.f4 != 99u), l_paint_leaf_far);
  if (((l_mark_ice > 0.0f) || (l_mark_glow.x > 0.0f)))
  {
    l_v = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))));
    l_fres = coax_pow_bg_vs((0x1.000000p0f - coax_p_fmax(coax_p_wg_dot3(coax_p_wg_neg_f3(l_v), l_n), 0.0f)), 0x1.000000p2f);
    l_key = coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), (coax_p_eg_st(0u)).w);
    l_res.x = coax_p_wg_make_f4_f3f(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(l_res.x.x, l_res.x.y, l_res.x.z), coax_p_wg_mul_f3f3(coax_p_wg_mul_ff3((l_mark_ice * (0x1.eb851ep-6f + (0x1.c28f5cp-3f * l_fres))), coax_p_v3(0x1.8f5c28p-1f, 0x1.b851ecp-1f, 0x1.e66666p-1f)), coax_p_wg_add_ff3(0x1.666666p-2f, coax_p_wg_mul_ff3(0x1.4cccccp-1f, coax_p_wg_min_f3f3(l_key, coax_p_v3(0x1.800000p0f, 0x1.800000p0f, 0x1.800000p0f)))))), coax_p_wg_mul_f3f(l_mark_glow, 0x1.70a3d8p-3f)), l_res.x.w);
  }
  l_figure = ((l_s.f4 == 8u) || (l_s.f4 == 3u));
  if ((((l_figure || (l_s.f4 == 5u)) || (l_s.f4 == 11u)) || (l_s.f4 == 12u)))
  {
    l_night = (coax_p_eg_st(14u)).w;
    l_near = (0x1.000000p0f - coax_p_smoothstep(0x1.800000p3f, 0x1.e00000p4f, coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y))));
    l_face = ((0x1.19999ap-1f + (0x1.000000p-2f * l_n.y)) + (0x1.99999ap-3f * coax_p_fmax(coax_p_wg_dot3(l_n, coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u))), 0.0f)));
    l_fill = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-4f, 0x1.0a3d70p-3f, 0x1.0a3d70p-2f), l_night), l_near), l_face), select(0x1.000000p-1f, 0x1.b33334p0f, l_figure));
    l_res.x = coax_p_wg_make_f4_f3f(coax_p_wg_add_f3f3(coax_p_v3(l_res.x.x, l_res.x.y, l_res.x.z), coax_p_wg_mul_f3f3(l_base, l_fill)), l_res.x.w);
  }
  if ((l_s.f4 == 14u))
  {
    l_spill = (l_s.f7.x * coax_at_u(i32(1341i)));
    l_res.x = coax_p_wg_make_f4_f3f(coax_p_wg_add_f3f3(coax_p_v3(l_res.x.x, l_res.x.y, l_res.x.z), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(l_base, coax_p_eg_room_warm(l_s.f7.z)), 0x1.ccccccp-1f), l_spill)), l_res.x.w);
  }
  return l_res;
}
fn coax_p_fx_fs(p_v: coax_t18) -> vec4f {
  var l_v: coax_t18 = p_v;
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
fn coax_p_water_transmit_fs(p_i: coax_t19) -> vec4f {
  var l_i: coax_t19 = p_i;
  var l_paint_leaf_far: bool;
  var l_wake_foam_store: array<f32, 1>;
  l_paint_leaf_far = false;
  l_wake_foam_store = array<f32, 1>();
  if (coax_p_eg_water_surface_part(coax_p_wg_u32_f((l_i.f4.x + 0x1.000000p-1f))))
  {
    return coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f);
  }
  return coax_p_wg_make_f4_f3f(coax_p_wg_clamp_f3f3f3((coax_p_eg_water_halves(l_i, l_paint_leaf_far, &l_wake_foam_store)).y, coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), 0x1.000000p0f);
}
fn coax_p_water_emit_fs(p_i: coax_t19) -> vec4f {
  var l_i: coax_t19 = p_i;
  var l_paint_leaf_far: bool;
  var l_wake_foam_store: array<f32, 1>;
  l_paint_leaf_far = false;
  l_wake_foam_store = array<f32, 1>();
  if (coax_p_eg_water_surface_part(coax_p_wg_u32_f((l_i.f4.x + 0x1.000000p-1f))))
  {
    return coax_p_wg_make_f4_f3f(coax_p_eg_water_whole(l_i, l_paint_leaf_far, &l_wake_foam_store), 0.0f);
  }
  return coax_p_wg_make_f4_f3f((coax_p_eg_water_halves(l_i, l_paint_leaf_far, &l_wake_foam_store)).x, 0.0f);
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
  var l_soft_k: f32;
  var l_far: f32;
  var l_t: vec2f;
  var l_soft: vec3f;
  var l_b: vec3f;
  var l_m: f32;
  var l_c: vec3f;
  l_light = coax_p_eg_water_meniscus(coax_p_wg_sw_rgb_f4(coax_p_wg_load_2d(coax_p_eg_water_waver_at(coax_p_v2(l_pos.x, l_pos.y)), 0i)), coax_p_v2(l_pos.x, l_pos.y));
  l_uv = coax_p_wg_div_f2f2(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_at2(0i));
  l_soft_k = select(0x1.000000p0f, 0x1.666666p-1f, ((coax_p_eg_st(31u)).z > 0x1.000000p-1f));
  l_far = coax_p_smoothstep((0x1.800000p3f * l_soft_k), (0x1.400000p5f * l_soft_k), ((coax_p_wg_load_2d_1(coax_p_wg_cvt_i2f2(coax_p_v2(l_pos.x, l_pos.y)), 0i)).y * 0x1.000000p8f));
  if ((l_far > 0.0f))
  {
    l_t = coax_p_wg_div_ff2(0x1.4cccccp-1f, coax_p_wg_at2(0i));
    l_soft = coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_sw_rgb_f4(textureSampleLevel(post_light, post_lin, coax_p_wg_add_f2f2(l_uv, l_t), 0.0f)), coax_p_wg_sw_rgb_f4(textureSampleLevel(post_light, post_lin, coax_p_wg_sub_f2f2(l_uv, l_t), 0.0f))), coax_p_wg_sw_rgb_f4(textureSampleLevel(post_light, post_lin, coax_p_wg_add_f2f2(l_uv, coax_p_v2(l_t.x, (-l_t.y))), 0.0f))), coax_p_wg_sw_rgb_f4(textureSampleLevel(post_light, post_lin, coax_p_wg_add_f2f2(l_uv, coax_p_v2((-l_t.x), l_t.y)), 0.0f))), 0x1.000000p-2f);
    l_light = coax_p_wg_mix_f3f3f(l_light, l_soft, l_far);
  }
  l_b = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_sw_rgb_f4(textureSampleLevel(post_quarter, post_lin, l_uv, 0.0f)), 0x1.19999ap-1f), coax_p_wg_mul_f3f(coax_p_wg_sw_rgb_f4(textureSampleLevel(post_eighth, post_lin, l_uv, 0.0f)), 0x1.ccccccp-2f));
  l_m = (coax_p_eg_st(19u)).z;
  l_c = coax_p_v3(coax_p_verse_style_bloom_mix(l_light.x, l_b.x, l_m), coax_p_verse_style_bloom_mix(l_light.y, l_b.y, l_m), coax_p_verse_style_bloom_mix(l_light.z, l_b.z, l_m));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_ff3(coax_p_eg_shafts(coax_p_v2(l_pos.x, l_pos.y), l_uv), coax_p_wg_sw_xyz_f4(coax_p_eg_st(11u))), (coax_p_eg_st(0u)).w));
  return coax_p_wg_make_f4_f3f(coax_p_eg_plates(coax_p_v2(l_pos.x, l_pos.y), coax_p_eg_encode(coax_p_wg_mul_f3f(coax_p_eg_tone(l_c), coax_at_u(i32(3i))))), 0x1.000000p0f);
}
fn coax_p_grass_fs(p_g: coax_t21) -> coax_t23 {
  var l_g: coax_t21 = p_g;
  var l_paint_leaf_far: bool;
  var l_res: coax_t23;
  l_paint_leaf_far = false;
  l_res = coax_p_eg_paint(l_g.w, coax_p_wg_normalize3(l_g.z), l_g.y, coax_p_eg_map_lit(l_g.y, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_paint_leaf_far), false, false, l_paint_leaf_far);
  l_res.x = coax_p_wg_make_f4_f3f(coax_p_wg_add_f3f3(coax_p_v3(l_res.x.x, l_res.x.y, l_res.x.z), coax_p_wg_mul_f3f(l_g.f4, 0x1.70a3d8p-3f)), l_res.x.w);
  return l_res;
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
fn coax_p_eg_place(p_id: u32, p_instance: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_o: coax_t15;
  var l_n: u32;
  var l_c: vec2f;
  var l_gx: i32;
  var l_gz: i32;
  var l_w: vec3f;
  var l_normal: vec3f;
  var l_m: coax_t11;
  var l_kind: u32;
  var l_v: coax_t32;
  var l_tint: vec3f;
  var l_canopy: bool;
  var l_cone: bool;
  var l_ranger: bool;
  var l_share: f32;
  var l_glass: bool;
  var l__sw3: u32;
  var l_b: u32;
  var l_blob: u32;
  var l_bare: f32;
  var l_b_27: u32;
  var l_xy: vec2f;
  var l_zw: u32;
  var l_at: vec4f;
  var l_nw: vec3f;
  var l_j: coax_t33;
  var l_wt: vec4f;
  var l_base: u32;
  var l_sp: vec3f;
  var l_sn: vec3f;
  var l_k: u32;
  var l_r: u32;
  var l_a: vec4f;
  var l_c_40: vec4f;
  var l_e: vec4f;
  var l_colours: u32;
  var l_skin: vec4f;
  var l_kind_2: u32;
  var l_d: vec4f;
  var l_b_46: u32;
  var l_at_47: vec4f;
  var l_nw_48: vec3f;
  var l_j_49: coax_t33;
  var l_wt_50: vec4f;
  var l_sp_51: vec3f;
  var l_sn_52: vec3f;
  var l_k_53: u32;
  var l_r_54: u32;
  var l_a_55: vec4f;
  var l_c_56: vec4f;
  var l_e_57: vec4f;
  var l_word: u32;
  var l_a_59: f32;
  var l_p: vec4f;
  var l_w_61: vec3f;
  var l_c0: vec3f;
  var l_c1: vec3f;
  var l_c2: vec3f;
  var l_normal_65: vec3f;
  var l_h: f32;
  var l_at_67: vec3f;
  l_o = coax_t15();
  l_n = coax_p_wg_u32_f(coax_at_u(i32(32i)));
  if ((l_instance < (l_n * l_n)))
  {
    l_c = coax_p_eg_quad((l_id % 6u));
    l_gx = (i32((l_instance % l_n)) + coax_p_wg_i32_f(l_c.x));
    l_gz = (i32((l_instance / l_n)) + coax_p_wg_i32_f(l_c.y));
    l_w = coax_p_v3((coax_at_u(i32(34i)) + (f32(l_gx) * coax_at_u(i32(33i)))), coax_p_eg_height(l_gx, l_gz), (coax_at_u(i32(35i)) + (f32(l_gz) * coax_at_u(i32(33i)))));
    l_normal = coax_p_wg_normalize3(coax_p_v3((coax_p_eg_height((l_gx - 1i), l_gz) - coax_p_eg_height((l_gx + 1i), l_gz)), (0x1.000000p1f * coax_at_u(i32(33i))), (coax_p_eg_height(l_gx, (l_gz - 1i)) - coax_p_eg_height(l_gx, (l_gz + 1i)))));
    l_o.x = coax_p_eg_clip(l_w);
    l_o.y = l_w;
    l_o.z = l_normal;
    l_o.w = coax_p_v3(0x1.333334p-2f, 0x1.000000p-1f, 0x1.99999ap-3f);
    l_o.f4 = 99u;
    l_o.f5 = coax_p_v2(0.0f, 0.0f);
    l_o.f6 = 0u;
    return l_o;
  }
  l_m = objects[i32((l_instance - (l_n * l_n)))];
  l_kind = (coax_p_wg_u32_f(l_m.w.w) % 8u);
  l_v = coax_t32();
  l_tint = coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z);
  l_canopy = false;
  l_cone = false;
  l_ranger = false;
  l_share = 0.0f;
  l_glass = false;
  l_o.f5 = coax_p_v2(0.0f, 0.0f);
  l_o.f6 = 0u;
  l_o.f7 = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  {
    l__sw3 = l_kind;
    if ((l__sw3 == 1u))
    {
      l_v = coax_p_eg_box_vertex(l_id);
    }
    else
    {
      if ((l__sw3 == 2u))
      {
        l_v = coax_p_eg_sphere_vertex(l_id);
      }
      else
      {
        if ((l__sw3 == 3u))
        {
          l_v = coax_p_eg_cylinder_vertex(l_id);
        }
        else
        {
          if ((l__sw3 == 5u))
          {
            l_b = (l_id * 12u);
            l_v = coax_t32(coax_p_v3(coax_p_eg_word(l_b), coax_p_eg_word((l_b + 1u)), coax_p_eg_word((l_b + 2u))), coax_p_v3(coax_p_eg_word((l_b + 3u)), coax_p_eg_word((l_b + 4u)), coax_p_eg_word((l_b + 5u))));
            l_o.f5 = coax_p_v2(coax_p_eg_word((l_b + 6u)), coax_p_eg_word((l_b + 7u)));
            l_o.f6 = (vertices[i32((l_b + 8u))] >> u32(24u));
            l_tint = coax_p_wg_mul_f3f3(coax_p_wg_sw_rgb_f4(coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 8u))])), coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z));
            if ((l_m.w.x < 0.0f))
            {
              l_tint = coax_p_wg_sw_rgb_f4(coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 8u))]));
              l_o.f7 = coax_p_wg_make_f4_f3f(coax_p_wg_neg_f3(coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z)), 0x1.800000p2f);
            }
            l_blob = vertices[i32((l_b + 9u))];
            if ((l_blob != 0u))
            {
              l_v.y = coax_p_eg_canopy_n(l_v.y, coax_p_wg_sw_xyz_f4(coax_p_wg_unpack4x8snorm(l_blob)));
              l_share = (f32((l_blob >> u32(24u))) / 0x1.fe0000p7f);
              l_canopy = true;
              l_bare = (f32((coax_p_wg_u32_f(l_m.w.w) / 256u)) / 0x1.fe0000p7f);
              if ((l_bare >= 0x1.fae148p-1f))
              {
                l_v.x = coax_p_v3(0.0f, 0.0f, 0.0f);
                l_share = 0.0f;
              }
              l_o.f7.z = l_bare;
            }
            l_glass = (vertices[i32((l_b + 10u))] == 1u);
            if ((((coax_p_wg_u32_f(l_m.w.w) / 8u) % 32u) == 24u))
            {
              l_o.f9 = ((l_instance - (l_n * l_n)) + 1u);
            }
          }
          else
          {
            if ((l__sw3 == 6u))
            {
              if ((l_m.w.z > 0x1.800000p0f))
              {
                l_b_27 = (coax_p_wg_u32_f(l_m.w.x) + (l_id * 6u));
                l_xy = coax_p_wg_unpack2x16unorm(vertices[i32(l_b_27)]);
                l_zw = vertices[i32((l_b_27 + 1u))];
                l_at = coax_p_wg_make_f4_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(l_xy.x, l_xy.y, (f32((l_zw & 65535u)) / 0x1.fffe00p15f)), 0x1.400000p1f), coax_p_EG_PERSON_LOW()), 0x1.000000p0f);
                l_nw = coax_p_wg_sw_xyz_f4(coax_p_wg_unpack4x8snorm(vertices[i32((l_b_27 + 2u))]));
                l_j = coax_p_wg_cvt_u4f4(coax_p_wg_add_f4f(coax_p_wg_mul_f4f(coax_p_wg_unpack4x8unorm(vertices[i32((l_b_27 + 4u))]), 0x1.fe0000p7f), 0x1.000000p-1f));
                l_wt = coax_p_wg_unpack4x8unorm(vertices[i32((l_b_27 + 5u))]);
                l_base = coax_p_wg_u32_f(l_m.w.y);
                l_sp = coax_p_v3(0.0f, 0.0f, 0.0f);
                l_sn = coax_p_v3(0.0f, 0.0f, 0.0f);
                {
                  l_k = 0u;
                  while ((l_k < 4u))
                  {
                    l_r = (l_base + (coax_p_wg_get_u4(l_j, i32(l_k)) * 3u));
                    l_a = pose[i32(l_r)];
                    l_c_40 = pose[i32((l_r + 1u))];
                    l_e = pose[i32((l_r + 2u))];
                    l_sp = coax_p_wg_add_f3f3(l_sp, coax_p_wg_mul_ff3(coax_p_wg_get_f4(l_wt, i32(l_k)), coax_p_v3(coax_p_wg_dot4(l_a, l_at), coax_p_wg_dot4(l_c_40, l_at), coax_p_wg_dot4(l_e, l_at))));
                    l_sn = coax_p_wg_add_f3f3(l_sn, coax_p_wg_mul_ff3(coax_p_wg_get_f4(l_wt, i32(l_k)), coax_p_v3(coax_p_wg_dot3(coax_p_v3(l_a.x, l_a.y, l_a.z), l_nw), coax_p_wg_dot3(coax_p_v3(l_c_40.x, l_c_40.y, l_c_40.z), l_nw), coax_p_wg_dot3(coax_p_v3(l_e.x, l_e.y, l_e.z), l_nw))));
                    l_k = (l_k + 1u);
                  }
                }
                l_v = coax_t32(l_sp, l_sn);
                l_o.f5 = coax_p_wg_unpack2x16unorm(vertices[i32((l_b_27 + 3u))]);
                l_colours = (l_base + (65u * 3u));
                l_skin = pose[i32(l_colours)];
                l_kind_2 = ((l_zw >> u32(24u)) & 255u);
                l_o.f6 = ((l_zw >> u32(16u)) & 255u);
                l_tint = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
                if ((l_kind_2 == 1u))
                {
                  l_tint = coax_p_v3(l_skin.x, l_skin.y, l_skin.z);
                }
                if ((l_kind_2 == 2u))
                {
                  l_tint = coax_p_v3(pose[i32((l_colours + 1u))].x, pose[i32((l_colours + 1u))].y, pose[i32((l_colours + 1u))].z);
                }
                if (((l_kind_2 == 3u) || (l_kind_2 == 4u)))
                {
                  l_o.f6 = (l_o.f6 + coax_p_wg_u32_f(l_skin.w));
                  l_d = pose[i32((l_colours + 2u))];
                  l_o.f7 = coax_p_wg_make_f4_f3f(coax_p_v3(l_d.x, l_d.y, l_d.z), ((f32((l_kind_2 - 2u)) * 0x1.000000p1f) + l_d.w));
                }
                l_ranger = true;
              }
              else
              {
                l_b_46 = (coax_p_wg_u32_f(l_m.w.x) + (l_id * 8u));
                l_at_47 = coax_p_v4(coax_p_eg_word(l_b_46), coax_p_eg_word((l_b_46 + 1u)), coax_p_eg_word((l_b_46 + 2u)), 0x1.000000p0f);
                l_nw_48 = coax_p_wg_sw_xyz_f4(coax_p_wg_unpack4x8snorm(vertices[i32((l_b_46 + 3u))]));
                l_j_49 = coax_p_wg_cvt_u4f4(coax_p_wg_add_f4f(coax_p_wg_mul_f4f(coax_p_wg_unpack4x8unorm(vertices[i32((l_b_46 + 6u))]), 0x1.fe0000p7f), 0x1.000000p-1f));
                l_wt_50 = coax_p_wg_unpack4x8unorm(vertices[i32((l_b_46 + 7u))]);
                l_sp_51 = coax_p_v3(0.0f, 0.0f, 0.0f);
                l_sn_52 = coax_p_v3(0.0f, 0.0f, 0.0f);
                {
                  l_k_53 = 0u;
                  while ((l_k_53 < 4u))
                  {
                    l_r_54 = (coax_p_wg_get_u4(l_j_49, i32(l_k_53)) * 3u);
                    l_a_55 = pose[i32(l_r_54)];
                    l_c_56 = pose[i32((l_r_54 + 1u))];
                    l_e_57 = pose[i32((l_r_54 + 2u))];
                    l_sp_51 = coax_p_wg_add_f3f3(l_sp_51, coax_p_wg_mul_ff3(coax_p_wg_get_f4(l_wt_50, i32(l_k_53)), coax_p_v3(coax_p_wg_dot4(l_a_55, l_at_47), coax_p_wg_dot4(l_c_56, l_at_47), coax_p_wg_dot4(l_e_57, l_at_47))));
                    l_sn_52 = coax_p_wg_add_f3f3(l_sn_52, coax_p_wg_mul_ff3(coax_p_wg_get_f4(l_wt_50, i32(l_k_53)), coax_p_v3(coax_p_wg_dot3(coax_p_v3(l_a_55.x, l_a_55.y, l_a_55.z), l_nw_48), coax_p_wg_dot3(coax_p_v3(l_c_56.x, l_c_56.y, l_c_56.z), l_nw_48), coax_p_wg_dot3(coax_p_v3(l_e_57.x, l_e_57.y, l_e_57.z), l_nw_48))));
                    l_k_53 = (l_k_53 + 1u);
                  }
                }
                l_v = coax_t32(l_sp_51, l_sn_52);
                l_o.f5 = coax_p_v2(coax_p_eg_word((l_b_46 + 4u)), coax_p_eg_word((l_b_46 + 5u)));
                l_o.f6 = (vertices[i32((l_b_46 + 3u))] >> u32(24u));
                l_tint = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
                l_ranger = true;
              }
            }
            else
            {
              if ((l__sw3 == 7u))
              {
                l_word = (coax_p_wg_u32_f(l_m.w.w) / 256u);
                l_v = coax_p_eg_lathe_vertex(l_id, l_word);
                l_a_59 = coax_p_wg_atan2(l_v.x.z, l_v.x.x);
                l_o.f5 = coax_p_v2((l_a_59 * coax_p_wg_length2(coax_p_v2(l_v.x.x, l_v.x.z))), l_v.x.y);
              }
              else
              {
                l_v = coax_p_eg_cone_vertex(l_id);
                if (((coax_p_wg_u32_f(l_m.w.w) / 8u) == 1u))
                {
                  l_v.y = coax_p_eg_canopy_n(l_v.y, coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_v.x, coax_p_v3(0.0f, 0x1.a36e2ep-14f, 0.0f))));
                  l_cone = true;
                }
              }
            }
          }
        }
      }
    }
  }
  l_p = coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f);
  l_w_61 = coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p));
  if ((l_share > 0.0f))
  {
    l_w_61 = coax_p_wg_add_f3f3(l_w_61, coax_p_eg_canopy_sway(coax_p_v3(l_m.x.w, l_m.y.w, l_m.z.w), l_share));
  }
  l_c0 = coax_p_v3(l_m.x.x, l_m.y.x, l_m.z.x);
  l_c1 = coax_p_v3(l_m.x.y, l_m.y.y, l_m.z.y);
  l_c2 = coax_p_v3(l_m.x.z, l_m.y.z, l_m.z.z);
  l_normal_65 = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c1, l_c2), l_v.y.x), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c2, l_c0), l_v.y.y)), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c0, l_c1), l_v.y.z));
  l_o.x = coax_p_eg_clip(l_w_61);
  l_o.y = l_w_61;
  l_o.z = coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_normal_65, coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  l_o.w = l_tint;
  l_o.f4 = select(select(select(((coax_p_wg_u32_f(l_m.w.w) / 8u) % 32u), 7u, l_canopy), 6u, l_cone), 8u, l_ranger);
  l_o.f8 = l_v.x;
  if ((l_o.f9 != 0u))
  {
    l_o.f4 = 5u;
  }
  if ((l_o.f4 == 14u))
  {
    l_h = coax_p_fract((sin(coax_p_wg_dot3(coax_p_v3(l_m.x.w, l_m.y.w, l_m.z.w), coax_p_v3(0x1.9fac72p3f, 0x1.090cb2p2f, 0x1.38ee98p6f))) * 0x1.55dd18p15f));
    l_o.f7 = coax_p_v4(select(0.0f, (0x1.333334p-1f + (0x1.99999ap-2f * coax_p_fract((l_h * 0x1.d3d70ap2f)))), (l_h < 0x1.51eb86p-1f)), select(0x1.000000p-1f, 0x1.000000p0f, l_glass), coax_p_fract((l_h * 0x1.b66666p3f)), 0.0f);
  }
  if (coax_p_eg_spell_soft(l_o.f4))
  {
    l_at_67 = coax_p_v3(l_m.x.w, l_m.y.w, l_m.z.w);
    l_o.f7 = coax_p_v4(coax_p_fmin((coax_p_fract(l_m.w.w) / 0x1.f5c290p-1f), 0x1.000000p0f), coax_p_fract((sin(coax_p_wg_dot3(l_at_67, coax_p_v3(0x1.9fac72p3f, 0x1.090cb2p2f, 0x1.38ee98p6f))) * 0x1.55dd18p15f)), 0.0f, 0.0f);
  }
  return l_o;
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
fn coax_p_eg_st(p_i: u32) -> vec4f {
  var l_i: u32 = p_i;
  return coax_p_wg_at4(((i32(l_i) * 4i) + 1084i));
}
fn coax_p_eg_spell_soft(p_look: u32) -> bool {
  var l_look: u32 = p_look;
  return ((l_look >= 17u) && (l_look <= 19u));
}
fn coax_p_eg_light_space() -> coax_t34 {
  var l_l: vec3f;
  var l_ax: vec3f;
  var l_ay: vec3f;
  var l_reach: f32;
  var l_size: f32;
  var l_c: vec3f;
  l_l = coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u));
  l_ax = select(coax_p_v3(0x1.000000p0f, 0.0f, 0.0f), coax_p_wg_normalize3(coax_p_wg_cross3(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_l)), (abs(l_l.y) < 0x1.ff7ceep-1f));
  l_ay = coax_p_wg_cross3(l_l, l_ax);
  l_reach = coax_p_verse_style_shadow_reach((coax_p_eg_st(31u)).z);
  l_size = coax_p_fmax((coax_p_eg_st(21u)).x, 0x1.000000p0f);
  l_c = coax_p_v3(coax_at_u(i32(24i)), 0.0f, coax_at_u(i32(26i)));
  return coax_t34(l_ax, l_ay, l_l, coax_p_verse_style_shadow_snap(coax_p_wg_dot3(l_c, l_ax), l_reach, l_size), coax_p_verse_style_shadow_snap(coax_p_wg_dot3(l_c, l_ay), l_reach, l_size), l_reach);
}
fn coax_p_eg_shadow_place(p_w: vec3f, p_k: coax_t34) -> vec3f {
  var l_w: vec3f = p_w;
  var l_k: coax_t34 = p_k;
  return coax_p_v3(coax_p_verse_style_shadow_across((coax_p_wg_dot3(l_w, l_k.x) - l_k.w), l_k.f5), coax_p_verse_style_shadow_across((coax_p_wg_dot3(l_w, l_k.y) - l_k.f4), l_k.f5), coax_p_verse_style_shadow_depth(coax_p_wg_dot3(coax_p_wg_sub_f3f3(l_w, coax_p_wg_add_f3f3(coax_p_wg_mul_ff3(l_k.w, l_k.x), coax_p_wg_mul_ff3(l_k.f4, l_k.y))), l_k.z), l_k.f5));
}
fn coax_p_wg_make_f4_f2ff(p_p0: vec2f, p_p1: f32, p_p2: f32) -> vec4f {
  var l_p0: vec2f = p_p0;
  var l_p1: f32 = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v4(l_p0.x, l_p0.y, l_p1, l_p2);
}
fn coax_p_eg_fx_vertex(p_id: u32, p_f: u32) -> coax_t19 {
  var l_id: u32 = p_id;
  var l_f: u32 = p_f;
  var l_o: coax_t19;
  var l_per: u32;
  var l_k: u32;
  var l_p: vec4f;
  var l_q: vec4f;
  var l_c: vec2f;
  var l_right: vec3f;
  var l_up: vec3f;
  var l_world: vec3f;
  l_o = coax_t19();
  if ((l_f == 0u))
  {
    return coax_p_eg_weir_vertex(l_id);
  }
  l_per = (coax_p_wg_u32_f(coax_at_u(i32(1074i))) * coax_p_wg_u32_f(coax_at_u(i32(1075i))));
  l_k = (((l_f - 1u) * l_per) + (l_id / 6u));
  if ((((l_id / 6u) >= l_per) || (l_k >= 64u)))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    return l_o;
  }
  l_p = coax_p_wg_at4(((i32((21u + (l_k * 2u))) * 4i) + 1500i));
  l_q = coax_p_wg_at4(((i32(((21u + (l_k * 2u)) + 1u)) * 4i) + 1500i));
  if (((l_p.w <= 0.0f) || (l_q.y <= 0x1.0624dep-9f)))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    return l_o;
  }
  l_c = coax_p_wg_sub_f2f2(coax_p_wg_mul_f2f(coax_p_eg_quad((l_id % 6u)), 0x1.000000p1f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f));
  l_right = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_up = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))));
  l_world = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(l_p.x, l_p.y, l_p.z), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_right, l_c.x), l_p.w)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_up, l_c.y), l_p.w), select(0x1.000000p0f, 0x1.800000p0f, (l_q.x < 0x1.000000p-1f))));
  l_o.x = coax_p_eg_clip(l_world);
  l_o.y = coax_p_v4(l_world.x, l_world.y, l_world.z, l_p.w);
  l_o.z = coax_p_v4(l_c.x, l_c.y, 0.0f, 0.0f);
  l_o.w = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  l_o.f4 = coax_p_v4(f32(8u), l_q.x, l_q.y, l_q.z);
  return l_o;
}
fn coax_p_eg_pond_vertex(p_id: u32, p_f: u32) -> coax_t19 {
  var l_id: u32 = p_id;
  var l_f: u32 = p_f;
  var l_o: coax_t19;
  var l_qa: u32;
  var l_ql: u32;
  var l_quad_id: u32;
  var l_c: vec2f;
  var l_p: u32;
  var l_q: vec4f;
  var l_qx: u32;
  var l_qz: u32;
  var l_rim: f32;
  var l_wedge: u32;
  var l_low: f32;
  var l_k: u32;
  var l_ca: f32;
  var l_cr: f32;
  var l_a: f32;
  var l_r: f32;
  var l_xz: vec2f;
  var l_world: vec3f;
  var l_depth: f32;
  l_o = coax_t19();
  l_qa = coax_p_wg_u32_f(coax_at_u(i32(1074i)));
  l_ql = coax_p_wg_u32_f(coax_at_u(i32(1075i)));
  l_quad_id = (l_id / 6u);
  l_c = coax_p_eg_quad((l_id % 6u));
  l_p = (l_f / 8u);
  if ((((l_f >= 32u) || (!coax_p_eg_pond_on(l_p))) || (l_quad_id >= (l_qa * l_ql))))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    return l_o;
  }
  l_q = coax_p_eg_pond_row(l_p);
  l_qx = (l_quad_id % l_qa);
  l_qz = (l_quad_id / l_qa);
  l_rim = (l_q.z + 0x1.333334p-2f);
  l_wedge = (l_f % 8u);
  l_low = 0x1.f40000p9f;
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      l_ca = ((f32((((l_wedge * l_qa) + l_qx) + (l_k & 1u))) / (0x1.000000p3f * f32(l_qa))) * 0x1.921fb6p2f);
      l_cr = ((l_rim * f32((l_qz + (l_k >> u32(1u))))) / f32(l_ql));
      l_low = coax_p_fmin(l_low, (coax_p_eg_ground_y(coax_p_wg_add_f2f2(coax_p_v2(l_q.x, l_q.y), coax_p_wg_mul_f2f(coax_p_v2(cos(l_ca), sin(l_ca)), l_cr))) - l_q.w));
      l_k = (l_k + 1u);
    }
  }
  if ((l_low > 0x1.333334p-1f))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    return l_o;
  }
  l_a = (((f32(((l_wedge * l_qa) + l_qx)) + l_c.x) / (0x1.000000p3f * f32(l_qa))) * 0x1.921fb6p2f);
  l_r = ((l_rim * (f32(l_qz) + l_c.y)) / f32(l_ql));
  l_xz = coax_p_wg_add_f2f2(coax_p_v2(l_q.x, l_q.y), coax_p_wg_mul_f2f(coax_p_v2(cos(l_a), sin(l_a)), l_r));
  l_world = coax_p_v3(l_xz.x, l_q.w, l_xz.y);
  l_depth = (l_q.w - coax_p_eg_ground_y(l_xz));
  l_o.x = coax_p_eg_clip(l_world);
  l_o.y = coax_p_v4(l_world.x, l_world.y, l_world.z, l_depth);
  l_o.z = coax_p_v4(l_xz.x, l_xz.y, 0.0f, 0.0f);
  l_o.w = coax_p_v4(0.0f, 0.0f, 0.0f, (coax_p_fmax(l_depth, 0.0f) / 0x1.333334p-1f));
  l_o.f4 = coax_p_v4(f32(6u), 0.0f, l_r, 0.0f);
  return l_o;
}
fn coax_p_eg_fountain_vertex(p_id: u32, p_f: u32) -> coax_t19 {
  var l_id: u32 = p_id;
  var l_f: u32 = p_f;
  var l_o: coax_t19;
  var l_qa: u32;
  var l_ql: u32;
  var l_quad_id: u32;
  var l_c: vec2f;
  var l_qx: u32;
  var l_qz: u32;
  var l_base: f32;
  var l_time: f32;
  var l_bowl: bool;
  var l_wedges: f32;
  var l_w: u32;
  var l_a: f32;
  var l_s: f32;
  var l_dir: vec2f;
  var l_r: f32;
  var l_level: f32;
  var l_depth: f32;
  var l_shore: f32;
  var l_land: f32;
  var l_world: vec3f;
  var l_off: f32;
  var l_flow: vec2f;
  var l_white: f32;
  var l_jet: bool;
  var l_k: u32;
  var l_v: f32;
  var l_phi: f32;
  var l_fall: coax_t35;
  var l_side: vec3f;
  var l_up: vec3f;
  var l_wide: f32;
  var l_n: vec3f;
  var l_world_36: vec3f;
  var l_k_37: u32;
  var l_a_38: f32;
  var l_at: vec3f;
  var l_phi_40: f32;
  var l_v_41: f32;
  var l_pulse: f32;
  var l_ring: vec3f;
  var l_world_44: vec3f;
  l_o = coax_t19();
  l_qa = coax_p_wg_u32_f(coax_at_u(i32(1074i)));
  l_ql = coax_p_wg_u32_f(coax_at_u(i32(1075i)));
  l_quad_id = (l_id / 6u);
  l_c = coax_p_eg_quad((l_id % 6u));
  if ((((!coax_p_eg_fountain_on()) || (l_quad_id >= (l_qa * l_ql))) || (l_f >= 32u)))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    return l_o;
  }
  l_qx = (l_quad_id % l_qa);
  l_qz = (l_quad_id / l_qa);
  l_base = coax_p_eg_fountain_base();
  l_time = coax_at_u(i32(1048i));
  if ((l_f < 12u))
  {
    l_bowl = (l_f >= 8u);
    l_wedges = select(0x1.000000p3f, 0x1.000000p2f, l_bowl);
    l_w = select(l_f, (l_f - 8u), l_bowl);
    l_a = (((f32(((l_w * l_qa) + l_qx)) + l_c.x) / (l_wedges * f32(l_qa))) * 0x1.921fb6p2f);
    l_s = ((f32(l_qz) + l_c.y) / f32(l_ql));
    l_dir = coax_p_v2(cos(l_a), sin(l_a));
    l_r = coax_p_mix(0x1.1eb852p-2f, 0x1.51eb86p1f, l_s);
    l_level = 0x1.3d70a4p-1f;
    l_depth = (0x1.3d70a4p-1f - coax_p_eg_fountain_floor(l_r));
    l_shore = coax_p_fmin((0x1.4e147ap1f - l_r), (l_r - 0x1.3d70a4p-2f));
    l_land = 0x1.c51eb8p0f;
    if (l_bowl)
    {
      l_r = coax_p_mix(0x1.eb851ep-4f, 0x1.30a3d8p0f, l_s);
      l_level = 0x1.0b851ep1f;
      l_depth = select((0x1.0b851ep1f - 0x1.e66666p0f), (0x1.0b851ep1f - coax_p_mix(0x1.0cccccp1f, 0x1.eb851ep0f, ((l_r - 0x1.333334p-3f) / 0x1.70a3d8p-4f))), (l_r < 0x1.eb851ep-3f));
      l_shore = coax_p_fmin((0x1.2b851ep0f - l_r), (l_r - 0x1.333334p-3f));
      l_land = 0x1.3d70a4p-2f;
    }
    l_world = coax_p_v3((l_dir.x * l_r), (l_base + l_level), (l_dir.y * l_r));
    l_off = (l_r - l_land);
    l_flow = coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_dir, coax_p_wg_sign(l_off)), 0x1.1eb852p-2f), exp((((-l_off) * l_off) / 0x1.000000p-1f)));
    l_white = (exp((((-l_off) * l_off) / 0x1.eb851ep-6f)) * select((0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_eg_fountain_stream_near(l_a))), 0x1.99999ap-1f, l_bowl));
    l_o.x = coax_p_eg_clip(l_world);
    l_o.y = coax_p_v4(l_world.x, l_world.y, l_world.z, l_depth);
    l_o.z = coax_p_wg_make_f4_f2f2(coax_p_v2(l_world.x, l_world.z), l_flow);
    l_o.w = coax_p_v4(l_white, 0.0f, 0.0f, l_shore);
    l_o.f4 = coax_p_v4(select(0x1.000000p0f, 0x1.000000p1f, l_bowl), 0.0f, l_r, l_land);
    return l_o;
  }
  if ((l_f < 24u))
  {
    l_jet = (l_f >= 20u);
    l_k = select((l_f - 12u), (l_f - 20u), l_jet);
    l_v = ((f32(l_qz) + l_c.y) / f32(l_ql));
    l_phi = (((f32(l_qx) + l_c.x) / f32(l_qa)) * 0x1.921fb6p2f);
    l_fall = coax_p_eg_fountain_fall(l_k, l_jet, l_v);
    l_side = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_cross3(l_fall.y, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)), coax_p_v3(0x1.a36e2ep-14f, 0.0f, 0.0f)));
    l_up = coax_p_wg_normalize3(coax_p_wg_cross3(l_side, l_fall.y));
    l_wide = select(coax_p_mix(0x1.000000p1f, 0x1.000000p0f, coax_p_smoothstep(0.0f, 0x1.000000p-1f, l_v)), 0x1.000000p0f, l_jet);
    l_n = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_side, cos(l_phi)), coax_p_wg_mul_f3f(l_up, sin(l_phi))));
    l_world_36 = coax_p_wg_add_f3f3(l_fall.x, coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_side, cos(l_phi)), l_wide), coax_p_wg_mul_f3f(l_up, sin(l_phi))), l_fall.z));
    l_o.x = coax_p_eg_clip(l_world_36);
    l_o.y = coax_p_v4(l_world_36.x, l_world_36.y, l_world_36.z, l_fall.z);
    l_o.z = coax_p_v4(l_n.x, l_n.y, l_n.z, l_v);
    l_o.w = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
    l_o.f4 = coax_p_v4(select(0x1.800000p1f, 0x1.000000p2f, l_jet), (l_phi / 0x1.921fb6p2f), l_v, f32(l_k));
    return l_o;
  }
  l_k_37 = (l_f - 24u);
  l_a_38 = coax_p_eg_fountain_stream_angle(l_k_37);
  l_at = coax_p_v3((cos(l_a_38) * 0x1.c51eb8p0f), (l_base + 0x1.3d70a4p-1f), (sin(l_a_38) * 0x1.c51eb8p0f));
  l_phi_40 = (((f32(l_qx) + l_c.x) / f32(l_qa)) * 0x1.921fb6p2f);
  l_v_41 = ((f32(l_qz) + l_c.y) / f32(l_ql));
  l_pulse = (0x1.800000p-1f + (0x1.000000p-2f * sin((((l_time * 0x1.600000p3f) + (f32(l_k_37) * 0x1.b33334p0f)) + (l_phi_40 * 0x1.000000p1f)))));
  l_ring = coax_p_v3(cos(l_phi_40), 0.0f, sin(l_phi_40));
  l_world_44 = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(l_at, coax_p_wg_mul_f3f(l_ring, coax_p_mix(0x1.eb851ep-5f, 0x1.99999ap-3f, l_v_41))), coax_p_v3(0.0f, (((l_v_41 * 0x1.47ae14p-3f) * l_pulse) - 0x1.47ae14p-7f), 0.0f));
  l_o.x = coax_p_eg_clip(l_world_44);
  l_o.y = coax_p_v4(l_world_44.x, l_world_44.y, l_world_44.z, 0x1.99999ap-5f);
  l_o.z = coax_p_wg_make_f4_f3f(coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_ring, coax_p_v3(0.0f, 0x1.99999ap-1f, 0.0f))), l_v_41);
  l_o.w = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  l_o.f4 = coax_p_v4(0x1.400000p2f, (l_phi_40 / 0x1.921fb6p2f), l_v_41, f32(l_k_37));
  return l_o;
}
fn coax_p_eg_quad(p_v: u32) -> vec2f {
  var l_v: u32 = p_v;
  var l_q: array<vec2f, 6>;
  l_q = array<vec2f, 6>(coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0x1.000000p0f));
  return l_q[i32(l_v)];
}
fn coax_p_eg_water_rest(p_uv: vec2f) -> coax_t20 {
  var l_uv: vec2f = p_uv;
  var l_cols: u32;
  var l_rows: u32;
  var l_a: vec2f;
  var l_i: u32;
  var l_j: u32;
  var l_f: vec2f;
  var l_k00: u32;
  var l_k10: u32;
  var l_k01: u32;
  var l_k11: u32;
  var l_o: coax_t20;
  var l_v: array<f32, 8>;
  var l_c: u32;
  l_cols = coax_p_wg_u32_f(coax_at_u(i32(1072i)));
  l_rows = coax_p_wg_u32_f(coax_at_u(i32(1073i)));
  l_a = coax_p_wg_clamp_f2f2f2(l_uv, coax_p_v2(0.0f, 0.0f), coax_p_v2(f32((l_cols - 1u)), f32((l_rows - 1u))));
  l_i = coax_p_wg_min_u(coax_p_wg_u32_f(floor(l_a.x)), (l_cols - 2u));
  l_j = coax_p_wg_min_u(coax_p_wg_u32_f(floor(l_a.y)), (l_rows - 2u));
  l_f = coax_p_wg_sub_f2f2(l_a, coax_p_v2(f32(l_i), f32(l_j)));
  l_k00 = ((l_j * l_cols) + l_i);
  l_k10 = (l_k00 + 1u);
  l_k01 = (l_k00 + l_cols);
  l_k11 = (l_k01 + 1u);
  l_o = coax_t20();
  l_v = array<f32, 8>();
  {
    l_c = 0u;
    while ((l_c < 8u))
    {
      l_v[i32(l_c)] = coax_p_mix(coax_p_mix(coax_p_eg_water_node(l_k00, l_c), coax_p_eg_water_node(l_k10, l_c), l_f.x), coax_p_mix(coax_p_eg_water_node(l_k01, l_c), coax_p_eg_water_node(l_k11, l_c), l_f.x), l_f.y);
      l_c = (l_c + 1u);
    }
  }
  l_o.x = coax_p_v3(l_v[0i], l_v[1i], l_v[2i]);
  l_o.y = l_v[3i];
  l_o.z = l_v[4i];
  l_o.w = coax_p_v2(l_v[5i], l_v[6i]);
  l_o.f4 = l_v[7i];
  return l_o;
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
fn coax_p_eg_water_gerstner(p_p0: vec2f, p_depth: f32, p_scale: f32) -> vec4f {
  var l_p0: vec2f = p_p0;
  var l_depth: f32 = p_depth;
  var l_scale: f32 = p_scale;
  var l_d: vec3f;
  var l_squeeze: f32;
  var l_count: i32;
  var l_gain: f32;
  var l_i: i32;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_k: f32;
  var l_amp: f32;
  var l_theta: f32;
  var l_co: f32;
  var l_s: f32;
  l_d = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_squeeze = 0.0f;
  l_count = coax_p_wg_i32_f(coax_at_u(i32(1051i)));
  l_gain = (coax_at_u(i32(1061i)) * l_scale);
  {
    l_i = 0i;
    while ((l_i < 8i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_a = coax_p_wg_at4(((i32((l_i * 2i)) * 4i) + 760i));
      l_c = coax_p_wg_at4(((i32(((l_i * 2i) + 1i)) * 4i) + 760i));
      l_k = l_a.z;
      l_amp = ((l_a.w * l_gain) * coax_p_eg_water_shoal(l_k, l_depth));
      l_theta = ((l_k * coax_p_wg_dot2(coax_p_v2(l_a.x, l_a.y), l_p0)) + l_c.y);
      l_co = cos(l_theta);
      l_s = sin(l_theta);
      l_d.x = (l_d.x + (((l_c.x * l_amp) * l_a.x) * l_co));
      l_d.z = (l_d.z + (((l_c.x * l_amp) * l_a.y) * l_co));
      l_d.y = (l_d.y + (l_amp * l_s));
      l_squeeze = (l_squeeze + (((l_c.x * l_amp) * l_k) * l_s));
      l_i = (l_i + 1i);
    }
  }
  return coax_p_v4(l_d.x, l_d.y, l_d.z, l_squeeze);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_eg_clip(p_w: vec3f) -> vec4f {
  var l_w: vec3f = p_w;
  var l_p: vec4f;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  return coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_p), coax_p_wg_dot4(coax_p_wg_at4(12i), l_p), coax_p_wg_dot4(coax_p_wg_at4(16i), l_p), coax_p_wg_dot4(coax_p_wg_at4(20i), l_p));
}
fn coax_p_wg_make_f4_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec4f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p1.x, l_p1.y);
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
fn coax_p_verse_style_near_reach(p_on_phone: f32) -> f32 {
  var l_on_phone: f32 = p_on_phone;
  return select(0x1.c00000p2f, 0x1.800000p2f, (l_on_phone > 0x1.000000p-1f));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_eg_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(coax_p_dot2(l_a, l_a));
}
fn coax_p_eg_ground_y(p_q: vec2f) -> f32 {
  var l_q: vec2f = p_q;
  var l_g: vec2f;
  var l_i: coax_t38;
  var l_f: vec2f;
  l_g = coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(l_q, coax_p_v2(coax_at_u(i32(34i)), coax_at_u(i32(35i)))), coax_at_u(i32(33i)));
  l_i = coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(l_g));
  l_f = coax_p_wg_fract_f2(l_g);
  return coax_p_mix(coax_p_mix(coax_p_eg_height(l_i.x, l_i.y), coax_p_eg_height((l_i.x + 1i), l_i.y), l_f.x), coax_p_mix(coax_p_eg_height(l_i.x, (l_i.y + 1i)), coax_p_eg_height((l_i.x + 1i), (l_i.y + 1i)), l_f.x), l_f.y);
}
fn coax_p_eg_ground_mark(p_q: vec2f, p_tip: f32) -> coax_t22 {
  var l_q: vec2f = p_q;
  var l_tip: f32 = p_tip;
  var l_m: coax_t22;
  var l_n: i32;
  var l_k: i32;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_rel: vec2f;
  var l_r: f32;
  var l_d: f32;
  var l_broad: f32;
  var l_fine: f32;
  var l_edge: f32;
  var l_inside: f32;
  var l_kind: i32;
  var l_c: vec3f;
  var l_w: f32;
  var l_dust: f32;
  var l_pool: f32;
  var l_ice: f32;
  var l_grain: f32;
  var l_total: f32;
  l_m = coax_t22();
  l_m.x = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_m.y = 0.0f;
  l_m.z = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_m.w = 0.0f;
  l_m.f4 = 0.0f;
  l_n = coax_p_wg_i32_f(coax_at_u(i32(2104i)));
  {
    l_k = 0i;
    while ((l_k < 24i))
    {
      if ((l_k >= l_n))
      {
        break;
      }
      l_a = coax_p_wg_at4(((i32((1i + (2i * l_k))) * 4i) + 2104i));
      l_b = coax_p_wg_at4(((i32((2i + (2i * l_k))) * 4i) + 2104i));
      l_rel = coax_p_wg_sub_f2f2(l_q, coax_p_v2(l_a.x, l_a.y));
      l_r = coax_p_fmax(l_a.z, 0x1.99999ap-4f);
      l_d = select(coax_p_wg_length2(l_rel), coax_p_fmax(abs(l_rel.x), abs(l_rel.y)), (l_b.z > 0x1.000000p-1f));
      if ((l_d > ((l_r * 0x1.59999ap0f) + 0x1.333334p-2f)))
      {
        {
          l_k = (l_k + 1i);
          continue;
        }
      }
      l_broad = coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, (0x1.99999ap0f / coax_p_fmax(l_r, 0x1.99999ap-1f))), coax_p_v2(l_b.y, (l_b.y * 0x1.666666p-1f))));
      l_fine = coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.4cccccp0f), coax_p_v2((l_b.y * 0x1.b33334p0f), 0x1.8cccccp1f)));
      l_edge = (((l_d / l_r) + ((l_broad - 0x1.000000p-1f) * 0x1.000000p-1f)) + ((l_fine - 0x1.000000p-1f) * 0x1.1eb852p-3f));
      l_inside = ((0x1.000000p0f - coax_p_smoothstep(0x1.19999ap-1f, 0x1.051eb8p0f, l_edge)) * l_b.x);
      if ((l_inside <= 0x1.0624dep-9f))
      {
        {
          l_k = (l_k + 1i);
          continue;
        }
      }
      l_kind = coax_p_wg_i32_f((l_a.w + 0x1.000000p-1f));
      l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
      l_w = 0.0f;
      if ((l_kind == 1i))
      {
        l_dust = ((0x1.333334p-1f + (0x1.99999ap-2f * coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.59999ap1f), coax_p_v2(l_b.y, l_b.y))))) * (0x1.70a3d8p-1f + (0x1.1eb852p-2f * coax_p_eg_noise(coax_p_wg_sub_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.09999ap3f), coax_p_v2(l_b.y, l_b.y))))));
        l_w = ((l_inside * l_dust) * coax_p_mix(0x1.99999ap-2f, 0x1.d70a3ep-1f, l_tip));
        l_c = coax_p_v3(0x1.a8f5c2p-1f, 0x1.bd70a4p-1f, 0x1.ccccccp-1f);
        l_pool = ((coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.ccccccp-1f), coax_p_v2((l_b.y * 0x1.8cccccp1f), 0x1.4cccccp0f))) * 0x1.800000p-1f) + (coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.d9999ap1f), coax_p_v2(0x1.b33334p0f, l_b.y))) * 0x1.000000p-2f));
        l_ice = ((coax_p_smoothstep((0x1.000000p0f - l_b.w), (0x1.3851ecp0f - l_b.w), l_pool) * l_inside) * (0x1.000000p0f - l_tip));
        l_grain = (0x1.b33334p-1f + (0x1.333334p-3f * coax_p_eg_noise(coax_p_v2(((l_q.x * 0x1.200000p3f) + (l_q.y * 0x1.800000p1f)), (l_q.y * 0x1.800000p0f)))));
        l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.51eb86p-1f, 0x1.6b851ep-1f, 0x1.7ae148p-1f), l_grain), l_ice);
        l_w = coax_p_fmax(l_w, (l_ice * 0x1.333334p-1f));
        l_m.f4 = coax_p_fmax(l_m.f4, l_ice);
      }
      else
      {
        if ((l_kind == 2i))
        {
          l_c = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.eb851ep-3f, 0x1.5c28f6p-3f, 0x1.99999ap-4f), coax_p_v3(0x1.c28f5cp-5f, 0x1.89374cp-5f, 0x1.581062p-5f), coax_p_smoothstep(0x1.e66666p-1f, 0x1.ccccccp-2f, l_edge));
          l_w = (l_inside * 0x1.d70a3ep-1f);
          l_m.w = coax_p_fmax(l_m.w, (l_inside * 0x1.99999ap-1f));
        }
        else
        {
          if ((l_kind == 3i))
          {
            l_c = coax_p_v3(0x1.19999ap-1f, 0x1.e147aep-2f, 0x1.666666p-2f);
            l_w = ((l_inside * 0x1.333334p-1f) * coax_p_mix(0x1.000000p0f, 0x1.666666p-1f, l_tip));
          }
          else
          {
            if ((l_kind == 4i))
            {
              l_m.z = coax_p_wg_add_f3f3(l_m.z, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.ae147ap-2f, 0x1.000000p-1f, 0x1.666666p-1f), l_inside), (0x1.800000p-1f + (0x1.000000p-2f * l_fine))));
              {
                l_k = (l_k + 1i);
                continue;
              }
            }
            else
            {
              if ((l_kind == 5i))
              {
                l_c = coax_p_v3(0x1.1eb852p-3f, 0x1.c28f5cp-3f, 0x1.70a3d8p-4f);
                l_w = (l_inside * 0x1.19999ap-1f);
              }
              else
              {
                if ((l_kind == 6i))
                {
                  l_c = coax_p_v3(0x1.333334p-4f, 0x1.c28f5cp-5f, 0x1.5c28f6p-4f);
                  l_w = (l_inside * coax_p_mix(0x1.b33334p-1f, 0x1.000000p-1f, l_tip));
                }
                else
                {
                  if ((l_kind == 7i))
                  {
                    l_c = coax_p_v3(0x1.99999ap-2f, 0x1.51eb86p-2f, 0x1.99999ap-3f);
                    l_w = ((l_inside * 0x1.ae147ap-2f) * (0x1.666666p-1f + (0x1.333334p-2f * l_fine)));
                  }
                  else
                  {
                    l_c = coax_p_v3(0x1.99999ap-3f, 0x1.851eb8p-3f, 0x1.47ae14p-3f);
                    l_w = (l_inside * 0x1.ccccccp-2f);
                  }
                }
              }
            }
          }
        }
      }
      l_total = (0x1.000000p0f - ((0x1.000000p0f - l_m.y) * (0x1.000000p0f - l_w)));
      l_m.x = coax_p_wg_div_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_m.x, l_m.y), (0x1.000000p0f - l_w)), coax_p_wg_mul_f3f(l_c, l_w)), coax_p_fmax(l_total, 0x1.a36e2ep-14f));
      l_m.y = l_total;
      l_k = (l_k + 1i);
    }
  }
  return l_m;
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
fn coax_p_eg_grass_mask(p_q: vec2f) -> f32 {
  var l_q: vec2f = p_q;
  var l_m: f32;
  var l_k: i32;
  var l_sq: vec4f;
  var l_k_7: i32;
  var l_b: vec4f;
  var l_d: f32;
  l_m = coax_p_verse_style_grass_clear((coax_p_eg_path_distance(l_q) - coax_at_u(i32(229i))));
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_sq = coax_p_wg_at4(((i32(l_k) * 4i) + 232i));
      if ((l_sq.z <= 0.0f))
      {
        break;
      }
      l_m = (l_m * coax_p_verse_style_grass_clear((coax_p_wg_length2(coax_p_wg_sub_f2f2(l_q, coax_p_v2(l_sq.x, l_sq.y))) - l_sq.z)));
      l_k = (l_k + 1i);
    }
  }
  {
    l_k_7 = 0i;
    while ((l_k_7 < 32i))
    {
      l_b = coax_p_wg_at4(((i32(l_k_7) * 4i) + 1212i));
      if ((l_b.z <= l_b.x))
      {
        {
          l_k_7 = (l_k_7 + 1i);
          continue;
        }
      }
      l_d = coax_p_fmax(coax_p_fmax((l_b.x - l_q.x), (l_q.x - l_b.z)), coax_p_fmax((l_b.y - l_q.y), (l_q.y - l_b.w)));
      l_m = (l_m * coax_p_verse_style_grass_clear((l_d - 0x1.333334p-2f)));
      l_k_7 = (l_k_7 + 1i);
    }
  }
  return (l_m * coax_p_verse_style_grass_clear((0x1.000000p-2f - (coax_p_eg_water_over(l_q) * 0x1.400000p2f))));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_dot2(l_a, l_b);
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
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_normalize2(p_a: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  return coax_p_scale2(l_a, (0x1.000000p0f / coax_p_wg_length2(l_a)));
}
fn coax_p_verse_style_part(p_d: f32, p_radius: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_radius: f32 = p_radius;
  return coax_p_st_sat((0x1.000000p0f - (l_d / coax_p_st_max(l_radius, 0x1.0624dep-10f))));
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_verse_style_near_width(p_d: f32, p_reach: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_reach: f32 = p_reach;
  return (0x1.000000p0f + (0x1.666666p0f * (0x1.000000p0f - coax_p_st_sat((l_d / coax_p_st_max(l_reach, 0x1.0624dep-10f))))));
}
fn coax_p_eg_meadow(p_q: vec2f) -> vec3f {
  var l_q: vec2f = p_q;
  return coax_p_wg_pow_f3f3(coax_p_eg_meadow_srgb(coax_p_eg_noise(coax_p_wg_mul_f2f(l_q, 0x1.0a3d70p-3f))), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f));
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
fn coax_p_wg_pow_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_pow_bg_vs(l_p0.x, l_p1.x), coax_pow_bg_vs(l_p0.y, l_p1.y), coax_pow_bg_vs(l_p0.z, l_p1.z));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
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
fn coax_p_eg_pixel_ray(p_p: vec2f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_ndc: vec4f;
  var l_w: vec4f;
  l_ndc = coax_p_v4((((l_p.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_p.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0x1.ccccccp-1f, 0x1.000000p0f);
  l_w = coax_p_m4_apply(coax_p_eg_unview(), l_ndc);
  return coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_wg_div_f3f(coax_p_v3(l_w.x, l_w.y, l_w.z), l_w.w), coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))));
}
fn coax_p_eg_sky(p_d: vec3f, p_l: vec3f) -> vec3f {
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
  var l_overcast: f32;
  var l_glow_patch: f32;
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
  l_h = coax_p_wg_sw_xyz_f4(coax_p_eg_st(13u));
  l_z = coax_p_wg_sw_xyz_f4(coax_p_eg_st(12u));
  l_t = coax_p_verse_style_sky_t(l_up);
  l_c = coax_p_wg_mix_f3f3f(l_h, l_z, l_t);
  l_gain = coax_p_verse_style_chroma_gain(coax_p_eg_saturation(l_c), coax_p_mix(coax_p_eg_saturation(l_h), coax_p_eg_saturation(l_z), l_t), 0x1.000000p0f);
  l_cy = coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z);
  l_c = coax_p_wg_max_f3f3(coax_p_wg_add_f3f3(coax_p_v3(l_cy, l_cy, l_cy), coax_p_wg_mul_f3f(coax_p_wg_sub_f3f3(l_c, coax_p_v3(l_cy, l_cy, l_cy)), l_gain)), coax_p_v3(0.0f, 0.0f, 0.0f));
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_sw_xyz_f4(coax_p_eg_st(10u)), coax_p_verse_style_sky_band(l_up, (coax_p_eg_st(12u)).w));
  l_ct = coax_p_wg_dot3(l_d, l_l);
  l_moon = ((coax_p_eg_st(1u)).w > 0x1.000000p-1f);
  l_disc = coax_p_smoothstep(cos(((coax_p_eg_st(14u)).x * 0x1.266666p0f)), cos(((coax_p_eg_st(14u)).x * 0x1.b33334p-1f)), l_ct);
  l_halo = ((coax_p_verse_style_halo(l_ct, (coax_p_eg_st(14u)).y) * 0x1.000000p-1f) + (coax_p_verse_style_halo(l_ct, (coax_p_eg_st(14u)).z) * 0x1.000000p-2f));
  if (l_moon)
  {
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), l_halo), 0x1.000000p-1f));
  }
  else
  {
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(11u)), 0x1.800000p0f), coax_p_clampf((l_halo * 0x1.19999ap0f), 0.0f, 0x1.ccccccp-1f));
  }
  l_overcast = coax_p_clampf(((coax_at_u(i32(1478i)) - 0x1.333334p-3f) / 0x1.19999ap-1f), 0.0f, 0x1.000000p0f);
  l_glow_patch = (coax_p_smoothstep(cos(((coax_p_eg_st(14u)).x * 0x1.c00000p2f)), 0x1.000000p0f, l_ct) * 0x1.eb851ep-4f);
  l_c = coax_p_wg_mix_f3f3f(l_c, select(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.c28f5cp-1f), 0x1.000000p2f), coax_p_wg_mul_f3f(coax_p_v3(0x1.b33334p-1f, 0x1.c28f5cp-1f, 0x1.000000p0f), 0x1.99999ap0f), l_moon), coax_p_mix(l_disc, l_glow_patch, l_overcast));
  l_cl = coax_p_eg_st(15u);
  l_c2 = coax_p_eg_st(16u);
  if (((l_up > 0x1.47ae14p-7f) && (l_cl.x > 0.0f)))
  {
    l_t_2 = select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
    l_p = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), l_up), l_cl.y), coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_sw_xy_f4(coax_p_eg_st(25u)), l_cl.z), l_t_2));
    l_n = coax_p_verse_style_cloud_field(l_p.x, l_p.y, l_c2.x, l_c2.y, l_cl.w);
    l_cover = coax_p_verse_style_cloud_cover(l_n, l_cl.x, l_c2.z, l_c2.w);
    l_toward = coax_p_wg_mul_f2f(coax_p_wg_normalize2(coax_p_wg_add_f2f2(coax_p_v2(l_l.x, l_l.z), coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f))), 0x1.040000p8f);
    l_lit = coax_p_verse_style_cloud_lit(l_n, coax_p_verse_style_cloud_field((l_p.x + l_toward.x), (l_p.y + l_toward.y), l_c2.x, l_c2.y, l_cl.w));
    l_night = (coax_p_eg_st(14u)).w;
    l_cloud = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(4u)), (coax_p_eg_st(17u)).w), 0x1.ccccccp-1f), coax_p_wg_sw_xyz_f4(coax_p_eg_st(17u)), l_lit), coax_p_fmax((coax_p_eg_st(0u)).w, 0x1.333334p-2f)), (0x1.000000p0f - (0x1.800000p-1f * l_night)));
    l_c = coax_p_wg_mix_f3f3f(l_c, l_cloud, ((l_cover * coax_p_verse_style_cloud_fade(l_up)) * (0x1.000000p0f - (0x1.000000p-1f * l_night))));
  }
  return coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_make_f3_f(coax_p_eg_stars(l_d)), (coax_p_eg_st(14u)).w), 0x1.333334p-1f));
}
fn coax_p_eg_ridges(p_d: vec3f, p_sky_c: vec3f) -> vec3f {
  var l_d: vec3f = p_d;
  var l_sky_c: vec3f = p_sky_c;
  var l_flat: f32;
  var l_dir: vec2f;
  var l_e: vec2f;
  var l_a: f32;
  var l_i: u32;
  var l_r: vec4f;
  var l_b: f32;
  var l_c: f32;
  var l_t: f32;
  var l_ring: vec2f;
  var l_k: f32;
  var l_shape: f32;
  var l_top: f32;
  var l_cell: f32;
  var l_f: f32;
  var l_h: f32;
  var l_lean: f32;
  var l_face: f32;
  var l_land: vec3f;
  var l_amb: vec3f;
  var l_lit: vec3f;
  var l_fog: f32;
  l_flat = coax_p_wg_length2(coax_p_v2(l_d.x, l_d.z));
  if ((l_flat < 0x1.0624dep-10f))
  {
    return l_sky_c;
  }
  l_dir = coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), l_flat);
  l_e = coax_p_v2(coax_at_u(i32(24i)), coax_at_u(i32(26i)));
  l_a = coax_p_wg_atan2(l_dir.y, l_dir.x);
  {
    l_i = 0u;
    while ((l_i < 3u))
    {
      l_r = coax_p_eg_ridge_ring(l_i);
      l_b = coax_p_wg_dot2(l_e, l_dir);
      l_c = (coax_p_wg_dot2(l_e, l_e) - (l_r.x * l_r.x));
      l_t = ((-l_b) + sqrt(coax_p_fmax(((l_b * l_b) - l_c), 0.0f)));
      l_ring = coax_p_wg_mul_f2f(coax_p_v2(cos(l_a), sin(l_a)), l_r.z);
      l_k = (f32(l_i) * 0x1.100000p4f);
      l_shape = ((0x1.3d70a4p-1f * coax_p_eg_noise(coax_p_wg_add_f2f2(l_ring, coax_p_v2(l_k, 0x1.800000p1f)))) + (0x1.851eb8p-2f * coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_ring, 0x1.59999ap1f), coax_p_v2(0x1.400000p2f, l_k)))));
      l_top = (l_r.y * (0x1.666666p-2f + (0x1.4cccccp-1f * l_shape)));
      if ((l_r.w > 0.0f))
      {
        l_cell = (l_a * 0x1.180000p6f);
        l_f = coax_p_fract(l_cell);
        l_h = coax_p_eg_hash2(coax_p_v2(floor(l_cell), l_k));
        l_top = (l_top + (((l_r.w * (0x1.99999ap-2f + (0x1.333334p-1f * l_h))) * coax_pow_bg_vs(coax_p_fmax((0x1.000000p0f - abs(((0x1.000000p1f * l_f) - 0x1.000000p0f))), 0.0f), 0x1.333334p-1f)) * coax_p_stepf(0x1.99999ap-3f, l_h)));
      }
      if ((((l_d.y * l_t) / l_flat) > (l_top - coax_at_u(i32(25i)))))
      {
        {
          l_i = (l_i + 1u);
          continue;
        }
      }
      l_lean = (coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_ring, 0x1.b33334p0f), coax_p_v2(0x1.200000p3f, l_k))) - 0x1.000000p-1f);
      l_face = (0x1.800000p-1f + (0x1.000000p-1f * coax_p_clampf(((coax_p_wg_dot3(coax_p_v3((-l_dir.x), 0.0f, (-l_dir.y)), coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u))) * 0x1.000000p-1f) + l_lean), 0.0f, 0x1.000000p0f)));
      l_land = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.99999ap-5f, 0x1.333334p-3f, 0x1.99999ap-5f), coax_p_v3(0x1.eb851ep-5f, 0x1.eb851ep-4f, 0x1.5c28f6p-3f), (f32(l_i) * 0x1.000000p-1f));
      l_amb = coax_p_v3(coax_p_verse_style_ambient(0x1.000000p0f, (coax_p_eg_st(5u)).x, (coax_p_eg_st(6u)).x, (coax_p_eg_st(5u)).w), coax_p_verse_style_ambient(0x1.000000p0f, (coax_p_eg_st(5u)).y, (coax_p_eg_st(6u)).y, (coax_p_eg_st(5u)).w), coax_p_verse_style_ambient(0x1.000000p0f, (coax_p_eg_st(5u)).z, (coax_p_eg_st(6u)).z, (coax_p_eg_st(5u)).w));
      l_lit = coax_p_wg_mul_f3f3(l_land, coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), (coax_p_eg_st(0u)).w), l_face), l_amb));
      l_fog = coax_p_fmax(((coax_p_eg_st(9u)).z * (0x1.333334p-3f + (0x1.000000p-2f * f32(l_i)))), coax_p_eg_haze_share(l_t, (l_r.y * 0x1.000000p-1f)));
      return coax_p_wg_mix_f3f3f(l_lit, coax_p_eg_haze_colour(l_fog, l_d), l_fog);
      l_i = (l_i + 1u);
    }
  }
  return l_sky_c;
}
fn coax_p_eg_weather_sky(p_c: vec3f, p_d: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_d: vec3f = p_d;
  var l_cloud: f32;
  var l_grey: vec3f;
  var l_o: vec3f;
  l_cloud = (coax_p_fmax((coax_at_u(i32(1478i)) - 0x1.333334p-3f), 0.0f) / 0x1.b33334p-1f);
  l_grey = coax_p_wg_mul_f3f3(coax_p_wg_make_f3_f(coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z)), coax_p_v3(0x1.b851ecp-1f, 0x1.d70a3ep-1f, 0x1.f5c290p-1f));
  l_o = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(l_grey, (0x1.000000p0f - (0x1.000000p-2f * l_cloud))), (l_cloud * 0x1.666666p-1f));
  l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.800000p-1f, 0x1.99999ap-1f, 0x1.000000p0f), coax_at_u(i32(1475i))), 0x1.99999ap-1f), coax_p_smoothstep((-0x1.99999ap-4f), 0x1.333334p-2f, l_d.y)));
  return l_o;
}
fn coax_p_eg_weather_streaks(p_c: vec3f, p_v: vec3f, p_d: f32) -> vec3f {
  var l_c: vec3f = p_c;
  var l_v: vec3f = p_v;
  var l_d: f32 = p_d;
  var l_rain: f32;
  var l_layers: i32;
  var l_t: f32;
  var l_lean: vec2f;
  var l_w: vec3f;
  var l_flat: f32;
  var l_k: f32;
  var l_around: f32;
  var l_px: f32;
  var l_a: f32;
  var l_i: i32;
  var l_r: f32;
  var l_along: f32;
  var l_fade: f32;
  var l_y: f32;
  var l_cells: f32;
  var l_q: vec2f;
  var l_cell: vec2f;
  var l_h: f32;
  var l_f: vec2f;
  var l_off: f32;
  var l_top: f32;
  var l_width: f32;
  var l_x: f32;
  var l_len_2: f32;
  var l_streak: f32;
  var l_key: vec3f;
  var l_tint: vec3f;
  l_rain = coax_at_u(i32(1472i));
  if ((l_rain <= 0x1.47ae14p-7f))
  {
    return l_c;
  }
  l_layers = select(2i, 4i, coax_p_eg_weather_desktop());
  l_t = select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
  l_lean = coax_p_wg_div_f2f(coax_p_v2(coax_at_u(i32(1473i)), coax_at_u(i32(1474i))), 0x1.c00000p2f);
  l_w = coax_p_v3((l_v.x + (l_lean.x * l_v.y)), l_v.y, (l_v.z + (l_lean.y * l_v.y)));
  l_flat = coax_p_wg_length2(coax_p_v2(l_w.x, l_w.z));
  l_k = (l_flat / coax_p_fmax(coax_p_wg_length3(l_w), 0x1.a36e2ep-14f));
  if ((l_k < 0x1.eb851ep-5f))
  {
    return l_c;
  }
  l_around = ((coax_p_wg_atan2(l_w.x, l_w.z) * 0x1.45f306p-3f) + 0x1.000000p-1f);
  l_px = coax_p_fmax(coax_at_u(i32(1065i)), 0x1.a36e2ep-14f);
  l_a = 0.0f;
  {
    l_i = 0i;
    while ((l_i < 4i))
    {
      if ((l_i >= l_layers))
      {
        break;
      }
      l_r = (0x1.333334p0f * coax_p_wg_exp2(f32(l_i)));
      l_along = (l_r / l_flat);
      if ((l_along > l_d))
      {
        break;
      }
      if (coax_p_eg_weather_covered(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), coax_p_wg_mul_f3f(l_v, l_along))))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_fade = (0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-5f, 0x1.eb851ep-4f, ((l_px * l_along) / l_k)));
      if ((l_fade <= 0.0f))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_y = (l_w.y * l_along);
      l_cells = coax_p_wg_round(((0x1.921fb6p2f * l_r) / 0x1.eb851ep-4f));
      l_q = coax_p_v2((l_around * l_cells), (((l_y + (l_t * 0x1.c00000p2f)) + (f32(l_i) * 0x1.b66666p3f)) / 0x1.b33334p-1f));
      l_cell = coax_p_wg_floor_f2(l_q);
      l_h = coax_p_eg_weather_hash(coax_p_wg_add_f2f2(l_cell, coax_p_v2((f32(l_i) * 0x1.100000p4f), 0x1.800000p1f)));
      if ((l_h > (l_rain * 0x1.19999ap-1f)))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_f = coax_p_wg_fract_f2(l_q);
      l_off = ((coax_p_eg_weather_hash(coax_p_wg_add_f2f(l_cell, 0x1.233334p3f)) * 0x1.666666p-1f) + 0x1.333334p-3f);
      l_top = (coax_p_eg_weather_hash(coax_p_wg_add_f2f(l_cell, 0x1.2cccccp2f)) * 0x1.666666p-1f);
      l_width = coax_p_fmax((((l_px * l_along) * 0x1.19999ap0f) / 0x1.eb851ep-4f), 0x1.47ae14p-6f);
      l_x = abs((l_f.x - l_off));
      l_len_2 = ((0x1.1eb852p-4f * coax_p_wg_exp2((f32(l_i) * 0x1.000000p-1f))) / 0x1.b33334p-1f);
      l_streak = (((0x1.000000p0f - coax_p_smoothstep((l_width * 0x1.000000p-1f), l_width, l_x)) * coax_p_smoothstep(l_top, (l_top + 0x1.47ae14p-6f), l_f.y)) * (0x1.000000p0f - coax_p_smoothstep(((l_top + l_len_2) - 0x1.47ae14p-6f), (l_top + l_len_2), l_f.y)));
      l_a = (l_a + ((l_streak * (0x1.19999ap-1f - (0x1.99999ap-4f * f32(l_i)))) * l_fade));
      l_i = (l_i + 1i);
    }
  }
  l_key = coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), (coax_p_eg_st(0u)).w);
  l_tint = coax_p_wg_mul_f3f(coax_p_v3(0x1.8f5c28p-1f, 0x1.ae147ap-1f, 0x1.ccccccp-1f), (0x1.ccccccp-2f + (0x1.19999ap-1f * coax_p_clampf(coax_p_verse_style_luma(l_key.x, l_key.y, l_key.z), 0.0f, 0x1.000000p0f))));
  return coax_p_wg_mix_f3f3f(l_c, l_tint, (coax_p_clampf(l_a, 0.0f, 0x1.000000p0f) * 0x1.99999ap-2f));
}
fn coax_p_eg_water_under_view(p_c: vec3f, p_world: vec3f, p_d: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_world: vec3f = p_world;
  var l_d: vec3f = p_d;
  var l_o: vec3f;
  var l_level: f32;
  var l_dist: f32;
  var l_path: f32;
  var l_tr: vec3f;
  var l_key: vec3f;
  var l_light: vec3f;
  var l_col: vec3f;
  if ((!coax_p_eg_water_pixel_under(l_d)))
  {
    return l_c;
  }
  l_o = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_level = (l_o.y - coax_p_eg_water_eye_over());
  l_dist = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_world, l_o));
  l_path = l_dist;
  if (((l_world.y > l_level) && (l_d.y > 0.0f)))
  {
    l_path = coax_p_fmin(l_dist, ((l_level - l_o.y) / l_d.y));
  }
  l_tr = coax_p_wg_exp_f3(coax_p_wg_mul_f3f(coax_p_wg_neg_f3(coax_p_EG_WATER_UNDER_ABSORB()), l_path));
  l_key = coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), (coax_p_eg_st(0u)).w);
  l_light = coax_p_wg_mul_f3f(coax_p_EG_WATER_UNDER_SCATTER(), (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_verse_style_luma(l_key.x, l_key.y, l_key.z))));
  l_col = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f3(l_c, coax_p_EG_WATER_UNDER_TINT()), l_tr), coax_p_wg_mul_f3f3(l_light, coax_p_wg_sub_f3f3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_tr)));
  l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.b33334p-1f), l_key), ((coax_p_eg_water_motes(l_o, l_d, coax_p_fmin(l_path, 0x1.800000p2f)) * 0x1.333334p-1f) + coax_p_eg_water_shafts(l_o, l_d, l_path, l_level))));
  return l_col;
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
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_dot3(l_a, l_b);
}
fn coax_p_verse_style_shaft_source(p_cos_t: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  return exp(((-0x1.800000p4f) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn coax_p_eg_shard_inside(p_shard: u32, p_p: vec3f) -> f32 {
  var l_shard: u32 = p_shard;
  var l_p: vec3f = p_p;
  var l_cell: u32;
  var l_d: f32;
  var l_k: u32;
  var l_pl: vec4f;
  l_cell = (coax_p_wg_u32_f(objects[i32((l_shard - 1u))].w.w) / 256u);
  l_d = 0x1.dcd650p29f;
  {
    l_k = 0u;
    while ((l_k < 8u))
    {
      l_pl = coax_p_eg_shard_plane(l_cell, l_k);
      l_d = coax_p_fmin(l_d, (l_pl.w - coax_p_wg_dot3(coax_p_v3(l_pl.x, l_pl.y, l_pl.z), l_p)));
      l_k = (l_k + 1u);
    }
  }
  return l_d;
}
fn coax_p_eg_leaf_gone(p_local: vec3f, p_bare: f32) -> bool {
  var l_local: vec3f = p_local;
  var l_bare: f32 = p_bare;
  if ((l_bare <= 0.0f))
  {
    return false;
  }
  return (coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_local.x, l_local.z), 0x1.b33334p0f), coax_p_v2((l_local.y * 0x1.4cccccp0f), ((-l_local.y) * 0x1.ccccccp-1f)))) < ((l_bare * 0x1.266666p0f) - 0x1.47ae14p-4f));
}
fn coax_p_wg_max_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y), coax_p_fmax(l_p0.z, l_p1.z));
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_eg_orb(p_s: coax_t15) -> coax_t23 {
  var l_s: coax_t15 = p_s;
  var l_n: vec3f;
  var l_v: vec3f;
  var l_facing: f32;
  var l_t: f32;
  var l_swirl: f32;
  var l_bayer: array<f32, 16>;
  var l_px: coax_t29;
  var l_clear: f32;
  var l_l: vec3f;
  var l_deep: vec3f;
  var l_body: vec3f;
  var l_fres: f32;
  var l_rim: vec3f;
  var l_spec: f32;
  var l_c: vec3f;
  l_n = coax_p_wg_normalize3(l_s.z);
  l_v = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y));
  l_facing = abs(coax_p_wg_dot3(l_n, l_v));
  l_t = coax_at_u(i32(2i));
  l_swirl = ((0x1.000000p-1f * coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.x, l_s.y.y), 0x1.0cccccp1f), coax_p_wg_mul_f2f(coax_p_v2(l_n.x, l_n.z), 0x1.800000p1f)), coax_p_v2((l_t * 0x1.99999ap0f), ((-l_t) * 0x1.19999ap0f))))) + (0x1.000000p-1f * coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.z, l_s.y.y), 0x1.59999ap1f), coax_p_wg_mul_f2f(coax_p_v2(l_n.y, l_n.x), 0x1.000000p1f)), coax_p_v2(((-l_t) * 0x1.333334p0f), (l_t * 0x1.ccccccp-1f))))));
  l_bayer = array<f32, 16>(0.0f, 0x1.000000p3f, 0x1.000000p1f, 0x1.400000p3f, 0x1.800000p3f, 0x1.000000p2f, 0x1.c00000p3f, 0x1.800000p2f, 0x1.800000p1f, 0x1.600000p3f, 0x1.000000p0f, 0x1.200000p3f, 0x1.e00000p3f, 0x1.c00000p2f, 0x1.a00000p3f, 0x1.400000p2f);
  l_px = coax_p_wg_mod_u2u2(coax_p_wg_cvt_u2f2(coax_p_v2(l_s.x.x, l_s.x.y)), coax_p_u2(4u, 4u));
  l_clear = (coax_p_smoothstep(0x1.666666p-2f, 0x1.e66666p-1f, l_facing) * (0x1.666666p-2f + (0x1.333334p-2f * l_swirl)));
  if ((((l_bayer[i32(((l_px.y * 4u) + l_px.x))] + 0x1.000000p-1f) / 0x1.000000p4f) < l_clear))
  {
    discard;
  }
  l_l = coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u));
  l_deep = coax_p_wg_pow_f3f3(coax_p_wg_max_f3f3(l_s.w, coax_p_v3(0.0f, 0.0f, 0.0f)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f));
  l_body = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_deep, (0x1.666666p-2f + (0x1.19999ap-1f * coax_p_fmax(coax_p_wg_dot3(l_n, l_l), 0.0f)))), (0x1.800000p-1f + (0x1.000000p-1f * l_swirl)));
  l_fres = coax_pow_bg_vs((0x1.000000p0f - l_facing), 0x1.800000p1f);
  l_rim = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(0x1.19999ap-1f, 0x1.800000p-1f, 0x1.b33334p-1f), coax_p_wg_sw_xyz_f4(coax_p_eg_st(5u)), 0x1.333334p-2f), l_fres), 0x1.4cccccp0f);
  l_spec = ((coax_pow_bg_vs(coax_p_fmax(coax_p_wg_dot3(l_n, coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_l, l_v))), 0.0f), 0x1.680000p6f) * 0x1.400000p1f) + (coax_pow_bg_vs(coax_p_fmax(coax_p_wg_dot3(l_n, coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_v3((-0x1.99999ap-2f), 0x1.99999ap-1f, 0x1.99999ap-3f), l_v))), 0.0f), 0x1.400000p5f) * 0x1.666666p-2f));
  l_c = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(l_body, l_rim), coax_p_v3(l_spec, l_spec, l_spec));
  return coax_t23(coax_p_v4(l_c.x, l_c.y, l_c.z, select(0.0f, 0x1.666666p-1f, (l_spec > 0x1.333334p-1f))), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_eg_spell_soft_part(p_s: coax_t15, p_paint_leaf_far: bool) -> coax_t23 {
  var l_s: coax_t15 = p_s;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_n: vec3f;
  var l_v: vec3f;
  var l_facing: f32;
  var l_t: f32;
  var l_seed: f32;
  var l_churn: f32;
  var l_body: f32;
  var l_cover: f32;
  var l_bayer: array<f32, 16>;
  var l_px: coax_t29;
  var l_deep: vec3f;
  var l_hot: f32;
  var l_heat: f32;
  var l_edge: vec3f;
  var l_body_c: vec3f;
  var l_c: vec3f;
  l_n = coax_p_wg_normalize3(l_s.z);
  l_v = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y));
  l_facing = abs(coax_p_wg_dot3(l_n, l_v));
  l_t = coax_at_u(i32(2i));
  l_seed = (l_s.f7.y * 0x1.480000p5f);
  l_churn = ((0x1.19999ap-1f * coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.x, l_s.y.y), 0x1.99999ap0f), coax_p_wg_mul_f2f(coax_p_v2(l_n.x, l_n.z), 0x1.e66666p0f)), coax_p_v2((l_seed + (l_t * 0x1.0cccccp1f)), ((-l_t) * 0x1.733334p1f))))) + (0x1.ccccccp-2f * coax_p_eg_noise(coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.z, l_s.y.y), 0x1.8cccccp1f), coax_p_wg_mul_f2f(coax_p_v2(l_n.y, l_n.x), 0x1.666666p0f)), coax_p_v2(((-l_t) * 0x1.333334p1f), (l_seed + (l_t * 0x1.b33334p0f)))))));
  l_body = coax_p_smoothstep(0x1.47ae14p-6f, 0x1.99999ap-1f, l_facing);
  if ((l_s.f4 == 19u))
  {
    l_body = coax_p_smoothstep(0x1.99999ap-1f, 0x1.99999ap-3f, abs(l_n.y));
  }
  if ((l_s.f4 == 18u))
  {
    l_body = coax_p_smoothstep(0.0f, 0x1.ccccccp-1f, l_facing);
  }
  l_cover = coax_p_smoothstep(0x1.333334p-2f, 0x1.3d70a4p-1f, ((l_s.f7.x * l_body) * (0x1.ccccccp-2f + (0x1.000000p0f * l_churn))));
  if ((l_s.f4 == 18u))
  {
    l_cover = coax_p_smoothstep(0x1.333334p-3f, 0x1.b33334p-1f, ((l_s.f7.x * l_body) * (0x1.99999ap-3f + (0x1.000000p0f * l_churn))));
  }
  l_bayer = array<f32, 16>(0.0f, 0x1.000000p3f, 0x1.000000p1f, 0x1.400000p3f, 0x1.800000p3f, 0x1.000000p2f, 0x1.c00000p3f, 0x1.800000p2f, 0x1.800000p1f, 0x1.600000p3f, 0x1.000000p0f, 0x1.200000p3f, 0x1.e00000p3f, 0x1.c00000p2f, 0x1.a00000p3f, 0x1.400000p2f);
  l_px = coax_p_wg_mod_u2u2(coax_p_wg_cvt_u2f2(coax_p_v2(l_s.x.x, l_s.x.y)), coax_p_u2(4u, 4u));
  if ((((l_bayer[i32(((l_px.y * 4u) + l_px.x))] + 0x1.000000p-1f) / 0x1.000000p4f) >= l_cover))
  {
    discard;
  }
  l_deep = coax_p_wg_pow_f3f3(coax_p_wg_max_f3f3(l_s.w, coax_p_v3(0.0f, 0.0f, 0.0f)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f));
  if ((l_s.f4 == 18u))
  {
    return coax_p_eg_paint(coax_p_wg_mul_f3f(l_deep, (0x1.99999ap-1f + (0x1.99999ap-2f * l_churn))), l_n, l_s.y, 0x1.000000p0f, false, false, l_paint_leaf_far);
  }
  l_hot = coax_p_clampf(((l_s.w.y * 0x1.99999ap0f) - 0x1.333334p-3f), 0.0f, 0x1.000000p0f);
  l_heat = (coax_p_clampf(((l_facing * l_facing) * (0x1.000000p-2f + (0x1.000000p0f * l_churn))), 0.0f, 0x1.000000p0f) * l_hot);
  l_edge = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-5f, 0x1.1eb852p-5f), coax_p_wg_mul_f3f3(l_deep, coax_p_v3(0x1.ccccccp-1f, 0x1.ae147ap-2f, 0x1.333334p-2f)), (0x1.ccccccp-2f + (0x1.19999ap-1f * l_hot)));
  l_body_c = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_deep, coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.a3d70ap-1f), ((l_heat * l_heat) * 0x1.333334p-1f)), ((0x1.ccccccp-2f + ((0x1.19999ap0f * l_churn) * (0x1.000000p0f - (0x1.000000p-1f * l_hot)))) + (0x1.000000p-1f * l_hot)));
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_edge, l_body_c, coax_p_smoothstep(0x1.99999ap-4f, 0x1.800000p-1f, l_facing)), (0x1.ccccccp-1f + (0x1.ccccccp0f * l_heat)));
  return coax_t23(coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(coax_p_dot3(l_a, l_a));
}
fn coax_p_wg_cvt_u2f2(p_p: vec2f) -> coax_t29 {
  var l_p: vec2f = p_p;
  return coax_p_u2(coax_p_wg_u32_f(l_p.x), coax_p_wg_u32_f(l_p.y));
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t29 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t29(l_x, l_y);
}
fn coax_p_wg_mod_u2u2(p_a: coax_t29, p_b: coax_t29) -> coax_t29 {
  var l_a: coax_t29 = p_a;
  var l_b: coax_t29 = p_b;
  return coax_p_u2((l_a.x % l_b.x), (l_a.y % l_b.y));
}
fn coax_p_eg_person_dye(p_t: vec3f, p_dye: vec4f) -> vec3f {
  var l_t: vec3f = p_t;
  var l_dye: vec4f = p_dye;
  var l_petal: f32;
  var l_shade: f32;
  var l_green: bool;
  var l_share: f32;
  var l_hi: f32;
  var l_lo: f32;
  var l_sat: f32;
  var l_light: f32;
  var l_mask: f32;
  var l_usual: f32;
  var l_dyed: vec3f;
  if ((l_dye.w < 0x1.000000p1f))
  {
    return l_t;
  }
  if ((l_dye.w >= 0x1.800000p2f))
  {
    l_petal = coax_p_smoothstep(0x1.47ae14p-5f, 0x1.47ae14p-3f, (coax_p_fmax(l_t.x, l_t.z) - l_t.y));
    l_shade = coax_p_clampf((coax_p_wg_dot3(l_t, coax_p_v3(0x1.b367a0p-3f, 0x1.6e2eb2p-1f, 0x1.27bb30p-4f)) / 0x1.000000p-1f), 0x1.4cccccp-1f, 0x1.400000p0f);
    return coax_p_wg_max_f3f3(coax_p_wg_mix_f3f3f(l_t, coax_p_wg_mul_f3f(coax_p_v3(l_dye.x, l_dye.y, l_dye.z), l_shade), l_petal), coax_p_v3(0.0f, 0.0f, 0.0f));
  }
  l_green = (l_dye.w >= 0x1.000000p2f);
  l_share = coax_p_clampf((l_dye.w - select(0x1.000000p1f, 0x1.000000p2f, l_green)), 0.0f, 0x1.000000p0f);
  l_hi = coax_p_fmax(l_t.x, coax_p_fmax(l_t.y, l_t.z));
  l_lo = coax_p_fmin(l_t.x, coax_p_fmin(l_t.y, l_t.z));
  l_sat = ((l_hi - l_lo) / coax_p_fmax(l_hi, 0x1.a36e2ep-14f));
  l_light = coax_p_wg_dot3(l_t, coax_p_v3(0x1.b367a0p-3f, 0x1.6e2eb2p-1f, 0x1.27bb30p-4f));
  l_mask = (coax_p_smoothstep(0x1.eb851ep-4f, 0x1.333334p-2f, l_hi) * (0x1.000000p0f - coax_p_smoothstep(0x1.c28f5cp-3f, 0x1.ae147ap-2f, l_sat)));
  l_usual = 0x1.ae147ap-2f;
  if (l_green)
  {
    l_mask = coax_p_smoothstep(0x1.47ae14p-6f, 0x1.eb851ep-5f, (l_t.y - coax_p_fmax(l_t.x, l_t.z)));
    l_usual = 0x1.70a3d8p-4f;
  }
  l_dyed = coax_p_wg_mul_f3f(coax_p_v3(l_dye.x, l_dye.y, l_dye.z), coax_p_clampf((l_light / l_usual), 0.0f, 0x1.99999ap0f));
  return coax_p_wg_max_f3f3(coax_p_wg_mix_f3f3f(l_t, l_dyed, (l_mask * l_share)), coax_p_v3(0.0f, 0.0f, 0.0f));
}
fn coax_p_wg_neg_f3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_v3((-l_a.x), (-l_a.y), (-l_a.z));
}
fn coax_p_eg_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_eg_hash(l_i), coax_p_eg_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_eg_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_eg_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
}
fn coax_p_eg_shard_cap(p_shard: u32, p_p: vec3f) -> vec3f {
  var l_shard: u32 = p_shard;
  var l_p: vec3f = p_p;
  var l_m: coax_t11;
  var l_c0: vec3f;
  var l_c1: vec3f;
  var l_c2: vec3f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_c: vec3f;
  var l_e: vec3f;
  var l_eye: vec3f;
  var l_cell: u32;
  var l_best: f32;
  var l_nl: vec3f;
  var l_k: u32;
  var l_pl: vec4f;
  var l_se: f32;
  var l_t: f32;
  l_m = objects[i32((l_shard - 1u))];
  l_c0 = coax_p_v3(l_m.x.x, l_m.y.x, l_m.z.x);
  l_c1 = coax_p_v3(l_m.x.y, l_m.y.y, l_m.z.y);
  l_c2 = coax_p_v3(l_m.x.z, l_m.y.z, l_m.z.z);
  l_a = coax_p_wg_cross3(l_c1, l_c2);
  l_b = coax_p_wg_cross3(l_c2, l_c0);
  l_c = coax_p_wg_cross3(l_c0, l_c1);
  l_e = coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), coax_p_v3(l_m.x.w, l_m.y.w, l_m.z.w));
  l_eye = coax_p_wg_div_f3f(coax_p_v3(coax_p_wg_dot3(l_a, l_e), coax_p_wg_dot3(l_b, l_e), coax_p_wg_dot3(l_c, l_e)), coax_p_wg_dot3(l_c0, l_a));
  l_cell = (coax_p_wg_u32_f(l_m.w.w) / 256u);
  l_best = (-0x1.000000p0f);
  l_nl = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_k = 0u;
    while ((l_k < 8u))
    {
      l_pl = coax_p_eg_shard_plane(l_cell, l_k);
      l_se = (coax_p_wg_dot3(coax_p_v3(l_pl.x, l_pl.y, l_pl.z), l_eye) - l_pl.w);
      if ((l_se <= 0.0f))
      {
        {
          l_k = (l_k + 1u);
          continue;
        }
      }
      l_t = (l_se / coax_p_fmax((l_se - (coax_p_wg_dot3(coax_p_v3(l_pl.x, l_pl.y, l_pl.z), l_p) - l_pl.w)), 0x1.4f8b58p-17f));
      if ((l_t > l_best))
      {
        l_best = l_t;
        l_nl = coax_p_v3(l_pl.x, l_pl.y, l_pl.z);
      }
      l_k = (l_k + 1u);
    }
  }
  if ((l_best < 0.0f))
  {
    return coax_p_wg_normalize3(l_e);
  }
  return coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_a, l_nl.x), coax_p_wg_mul_f3f(l_b, l_nl.y)), coax_p_wg_mul_f3f(l_c, l_nl.z)), coax_p_v3(0.0f, 0x1.0c6f7ap-20f, 0.0f)));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_eg_shade_shadows_at(p_p: vec2f, p_y: f32, p_level: bool) -> f32 {
  var l_p: vec2f = p_p;
  var l_y: f32 = p_y;
  var l_level: bool = p_level;
  var l_s: f32;
  var l_i: i32;
  var l_b: vec4f;
  var l_cm: f32;
  var l_strength: f32;
  var l_d: f32;
  l_s = 0x1.000000p0f;
  {
    l_i = 0i;
    while ((l_i < 16i))
    {
      l_b = coax_p_wg_at4(((i32(l_i) * 4i) + 36i));
      if ((l_b.z <= 0.0f))
      {
        break;
      }
      l_cm = floor((l_b.w * 0x1.000000p-1f));
      l_strength = (l_b.w - (l_cm * 0x1.000000p1f));
      if ((l_level && (abs((l_y - ((l_cm * 0x1.47ae14p-7f) - 0x1.900000p6f))) > 0x1.666666p-2f)))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_d = (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_b.x, l_b.y))) / l_b.z);
      l_s = (l_s * (0x1.000000p0f - (l_strength * (0x1.000000p0f - coax_p_smoothstep(0x1.666666p-2f, 0x1.000000p0f, l_d)))));
      l_i = (l_i + 1i);
    }
  }
  return l_s;
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_eg_meadow_srgb(p_t: f32) -> vec3f {
  var l_t: f32 = p_t;
  return coax_p_wg_mix_f3f3f(coax_p_EG_MEADOW_COOL(), coax_p_EG_MEADOW_WARM(), l_t);
}
fn coax_p_eg_path_distance(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_d: f32;
  var l_count: i32;
  var l_i: i32;
  var l_seg: vec4f;
  var l_a: vec2f;
  var l_ab: vec2f;
  var l_t: f32;
  l_d = 0x1.f40000p9f;
  l_count = coax_p_wg_i32_f(coax_at_u(i32(228i)));
  {
    l_i = 0i;
    while ((l_i < 32i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_seg = coax_p_wg_at4(((i32(l_i) * 4i) + 100i));
      l_a = coax_p_v2(l_seg.x, l_seg.y);
      l_ab = coax_p_wg_sub_f2f2(coax_p_v2(l_seg.z, l_seg.w), l_a);
      l_t = coax_p_clampf((coax_p_wg_dot2(coax_p_wg_sub_f2f2(l_p, l_a), l_ab) / coax_p_fmax(coax_p_wg_dot2(l_ab, l_ab), 0x1.a36e2ep-14f)), 0.0f, 0x1.000000p0f);
      l_d = coax_p_fmin(l_d, coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, l_a), coax_p_wg_mul_f2f(l_ab, l_t))));
      l_i = (l_i + 1i);
    }
  }
  return l_d;
}
fn coax_p_eg_water_over(p_xz: vec2f) -> f32 {
  var l_xz: vec2f = p_xz;
  var l_q: f32;
  var l_uv: vec2f;
  l_q = coax_p_eg_pond_depth_at(coax_p_v3(l_xz.x, coax_p_eg_ground_y(l_xz), l_xz.y));
  if ((l_q > 0.0f))
  {
    return l_q;
  }
  if (((coax_at_u(i32(1072i)) < 0x1.000000p1f) || (coax_at_u(i32(1083i)) <= 0.0f)))
  {
    return 0.0f;
  }
  l_uv = coax_p_eg_water_uv(l_xz);
  if (((((l_uv.x < 0.0f) || (l_uv.y < 0.0f)) || (l_uv.x > (coax_at_u(i32(1072i)) - 0x1.000000p0f))) || (l_uv.y > (coax_at_u(i32(1073i)) - 0x1.000000p0f))))
  {
    return 0.0f;
  }
  return coax_p_fmax((coax_p_eg_water_rest(l_uv)).y, 0.0f);
}
fn coax_p_eg_carved(p_tint: vec3f, p_p: vec3f, p_uv: vec2f) -> vec3f {
  var l_tint: vec3f = p_tint;
  var l_p: vec3f = p_p;
  var l_uv: vec2f = p_uv;
  var l_course: f32;
  var l_along: f32;
  var l_block: f32;
  var l_fy: f32;
  var l_fx: f32;
  var l_gap: f32;
  var l_joint: f32;
  var l_c: vec3f;
  var l_depth: f32;
  var l_wet: f32;
  var l_moss: f32;
  l_course = floor((l_uv.y / 0x1.0a3d70p-2f));
  l_along = ((l_uv.x / 0x1.333334p-1f) + select(0.0f, 0x1.000000p-1f, (coax_p_fract((l_course * 0x1.000000p-1f)) > 0x1.000000p-2f)));
  l_block = floor(l_along);
  l_fy = coax_p_fract((l_uv.y / 0x1.0a3d70p-2f));
  l_fx = coax_p_fract(l_along);
  l_gap = coax_p_fmin((coax_p_fmin(l_fx, (0x1.000000p0f - l_fx)) * 0x1.333334p-1f), (coax_p_fmin(l_fy, (0x1.000000p0f - l_fy)) * 0x1.0a3d70p-2f));
  l_joint = (0x1.000000p0f - coax_p_smoothstep(0x1.89374cp-8f, 0x1.6872b0p-6f, l_gap));
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_tint, (0x1.b851ecp-1f + (0x1.70a3d8p-3f * coax_p_eg_water_hash(coax_p_v3(l_block, l_course, 0x1.c00000p2f))))), (0x1.ccccccp-1f + (0x1.1eb852p-3f * coax_p_eg_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_p.x, l_p.z), 0x1.800000p2f), (l_p.y * 0x1.000000p2f))))));
  l_c = coax_p_wg_mul_f3f(l_c, (0x1.ccccccp-1f + (0x1.99999ap-4f * coax_p_eg_noise(coax_p_v2((l_uv.x * 0x1.400000p1f), (l_p.y * 0x1.000000p-1f))))));
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(l_c, 0x1.19999ap-1f), l_joint);
  l_depth = coax_p_eg_water_depth_at(l_p);
  l_wet = coax_p_smoothstep((-0x1.47ae14p-4f), 0.0f, l_depth);
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f3(l_c, coax_p_v3(0x1.333334p-1f, 0x1.51eb86p-1f, 0x1.47ae14p-1f)), l_wet);
  l_moss = (exp((((-(l_depth + 0x1.47ae14p-6f)) * (l_depth + 0x1.47ae14p-6f)) / 0x1.0624dep-9f)) * coax_p_smoothstep(0x1.333334p-2f, 0x1.666666p-1f, coax_p_eg_noise(coax_p_v2((l_uv.x * 0x1.800000p1f), (l_p.y * 0x1.200000p3f)))));
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.333334p-2f, 0x1.851eb8p-2f, 0x1.99999ap-3f), (l_moss * 0x1.333334p-1f));
  return l_c;
}
fn coax_p_eg_timber(p_tint: vec3f, p_p: vec3f, p_n: vec3f, p_deck: bool) -> vec3f {
  var l_tint: vec3f = p_tint;
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_deck: bool = p_deck;
  var l_c: vec3f;
  var l_board: f32;
  var l_fx: f32;
  var l_run: f32;
  var l_fz: f32;
  var l_gap: f32;
  var l_grain: f32;
  var l_g: vec2f;
  var l_grain_14: f32;
  var l_damp: f32;
  l_c = l_tint;
  if ((l_deck && (l_n.y > 0x1.000000p-1f)))
  {
    l_board = floor((l_p.x / 0x1.333334p-2f));
    l_fx = coax_p_fract((l_p.x / 0x1.333334p-2f));
    l_run = ((l_p.z / 0x1.333334p1f) + coax_p_eg_water_hash(coax_p_v3(l_board, 0x1.800000p1f, 0x1.000000p0f)));
    l_fz = coax_p_fract(l_run);
    l_gap = (0x1.000000p0f - (coax_p_smoothstep(0x1.eb851ep-6f, 0x1.47ae14p-4f, coax_p_fmin(l_fx, (0x1.000000p0f - l_fx))) * coax_p_smoothstep(0x1.0624dep-8f, 0x1.89374cp-7f, coax_p_fmin(l_fz, (0x1.000000p0f - l_fz)))));
    l_grain = ((coax_p_eg_noise(coax_p_v2((l_p.x * 0x1.e00000p4f), (l_p.z * 0x1.333334p0f))) * 0x1.333334p-1f) + (coax_p_eg_noise(coax_p_v2((l_p.x * 0x1.180000p6f), (l_p.z * 0x1.800000p1f))) * 0x1.99999ap-2f));
    l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_c, (0x1.a3d70ap-1f + (0x1.0a3d70p-2f * coax_p_eg_water_hash(coax_p_v3(l_board, floor(l_run), 0x1.000000p1f))))), (0x1.c28f5cp-1f + (0x1.99999ap-3f * l_grain)));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(l_c, 0x1.666666p-2f), l_gap);
  }
  else
  {
    l_g = select(coax_p_v2(((l_p.x * 0x1.000000p0f) + (l_p.z * 0x1.000000p0f)), (l_p.y * 0x1.900000p4f)), coax_p_v2((l_p.y * 0x1.333334p0f), ((l_p.x + l_p.z) * 0x1.900000p4f)), (abs(l_n.y) > 0x1.000000p-1f));
    l_grain_14 = ((coax_p_eg_noise(coax_p_v2(((l_p.x * 0x1.900000p4f) + (l_p.z * 0x1.900000p4f)), (l_p.y * 0x1.800000p0f))) * 0x1.000000p-1f) + (coax_p_eg_noise(l_g) * 0x1.000000p-1f));
    l_c = coax_p_wg_mul_f3f(l_c, (0x1.b33334p-1f + (0x1.000000p-2f * l_grain_14)));
  }
  l_c = coax_p_wg_mul_f3f(l_c, (0x1.ccccccp-1f + (0x1.eb851ep-4f * coax_p_eg_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_p.x, l_p.z), 0x1.666666p-1f), l_p.y)))));
  l_damp = coax_p_eg_water_depth_at(l_p);
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f3(l_c, coax_p_v3(0x1.19999ap-1f, 0x1.28f5c2p-1f, 0x1.0a3d70p-1f)), coax_p_smoothstep((-0x1.000000p-2f), 0.0f, l_damp));
  return l_c;
}
fn coax_p_eg_glass(p_s: coax_t15, p_front: bool) -> coax_t23 {
  var l_s: coax_t15 = p_s;
  var l_front: bool = p_front;
  var l_v: vec3f;
  var l_up: f32;
  var l_c: vec3f;
  var l_room: f32;
  var l_fog: f32;
  if ((!l_front))
  {
    discard;
  }
  l_v = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))));
  l_up = coax_p_clampf(l_s.f5.y, 0.0f, 0x1.000000p0f);
  l_c = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.1eb852p-5f, 0x1.99999ap-5f, 0x1.0a3d70p-4f), coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(5u)), 0x1.ccccccp-2f), (0x1.666666p-2f + (0x1.99999ap-2f * l_up)));
  l_room = (l_s.f7.x * coax_at_u(i32(1341i)));
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(coax_p_eg_room_warm(l_s.f7.z), (0x1.b33334p0f - (0x1.99999ap-1f * l_up))), coax_p_clampf(l_room, 0.0f, 0x1.000000p0f));
  l_fog = coax_p_eg_haze_share(coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y)), l_s.y.y);
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_eg_haze_colour(l_fog, l_v), (l_fog * (0x1.000000p0f - (0x1.333334p-1f * l_room))));
  return coax_t23(coax_p_v4(l_c.x, l_c.y, l_c.z, select(0.0f, 0x1.b33334p-1f, (l_room > 0x1.99999ap-5f))), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_eg_map_lit(p_p: vec3f, p_n: vec3f, p_paint_leaf_far: bool) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_q: vec3f;
  var l_uv: vec2f;
  var l_texel_2: f32;
  var l_phone: f32;
  var l_taps: f32;
  var l_r: f32;
  var l_lit: f32;
  var l_k: f32;
  var l_o: vec2f;
  var l_inside: bool;
  if (((coax_p_eg_st(21u)).x < 0x1.000000p0f))
  {
    return 0x1.000000p0f;
  }
  l_q = coax_p_eg_shadow_place(coax_p_wg_add_f3f3(l_p, coax_p_wg_mul_f3f(l_n, 0x1.47ae14p-4f)), coax_p_eg_light_space());
  l_uv = coax_p_v2((0x1.000000p-1f + (0x1.000000p-1f * l_q.x)), (0x1.000000p-1f - (0x1.000000p-1f * l_q.y)));
  l_texel_2 = (0x1.000000p0f / f32((coax_p_wg_dims_depth()).x));
  l_phone = (coax_p_eg_st(31u)).z;
  l_taps = select((coax_p_eg_st(21u)).y, 0x1.000000p0f, l_paint_leaf_far);
  l_r = (coax_p_eg_st(21u)).z;
  l_lit = 0.0f;
  {
    l_k = 0.0f;
    while ((l_k < 0x1.200000p3f))
    {
      if ((l_k >= l_taps))
      {
        break;
      }
      l_o = coax_p_v2(coax_p_verse_style_shadow_tap(l_k, l_phone, 0.0f), coax_p_verse_style_shadow_tap(l_k, l_phone, 0x1.000000p0f));
      l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_o, l_r), 0x1.99999ap0f), l_texel_2)), (l_q.z - 0x1.3a92a4p-11f)));
      l_k = (l_k + 0x1.000000p0f);
    }
  }
  l_inside = (coax_p_wg_all_gt_f2f2(l_uv, coax_p_v2(0.0f, 0.0f)) && coax_p_wg_all_lt_f2f2(l_uv, coax_p_v2(0x1.000000p0f, 0x1.000000p0f)));
  return select(0x1.000000p0f, coax_p_smoothstep(0x1.99999ap-4f, 0x1.800000p-1f, (l_lit / coax_p_fmax(l_taps, 0x1.000000p0f))), l_inside);
}
fn coax_p_verse_style_contact(p_cast_lit: f32, p_blob: f32, p_strength: f32) -> f32 {
  var l_cast_lit: f32 = p_cast_lit;
  var l_blob: f32 = p_blob;
  var l_strength: f32 = p_strength;
  return coax_p_st_min(l_cast_lit, (0x1.000000p0f - (l_strength * coax_p_st_sat(l_blob))));
}
fn coax_p_eg_paint(p_dry: vec3f, p_n: vec3f, p_world: vec3f, p_lit: f32, p_character: bool, p_rim: bool, p_paint_leaf_far: bool) -> coax_t23 {
  var l_dry: vec3f = p_dry;
  var l_n: vec3f = p_n;
  var l_world: vec3f = p_world;
  var l_lit: f32 = p_lit;
  var l_character: bool = p_character;
  var l_rim: bool = p_rim;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_l: vec3f;
  var l_v: vec3f;
  var l_base: vec3f;
  var l_w: f32;
  var l_band0: f32;
  var l_band: f32;
  var l_key: vec3f;
  var l_amb: vec3f;
  var l_c: vec3f;
  var l_gain: f32;
  var l_cy: f32;
  var l_rs: f32;
  var l_d: f32;
  var l_fog: f32;
  var l_b: vec4f;
  l_l = coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u));
  l_v = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_world, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))));
  l_base = coax_p_eg_weather_wet_base(l_dry, l_n, l_character);
  l_w = (coax_p_verse_style_wrap(coax_p_wg_dot3(l_n, l_l)) * l_lit);
  l_band0 = select(coax_p_verse_style_bands(l_w, (coax_p_eg_st(7u)).x, (coax_p_eg_st(7u)).y, (coax_p_eg_st(7u)).z, (coax_p_eg_st(7u)).w), coax_p_verse_style_bands2(l_w, (coax_p_eg_st(8u)).x, (coax_p_eg_st(8u)).y), l_character);
  l_band = coax_p_mix(l_band0, 0x1.333334p-1f, (0x1.ccccccp-2f * coax_p_smoothstep(0x1.e00000p3f, 0x1.e00000p5f, coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world)))));
  l_key = coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), (coax_p_eg_st(0u)).w);
  l_amb = coax_p_wg_mul_ff3(coax_p_eg_sky_seen(l_world), coax_p_v3(coax_p_verse_style_ambient(l_n.y, (coax_p_eg_st(5u)).x, (coax_p_eg_st(6u)).x, (coax_p_eg_st(5u)).w), coax_p_verse_style_ambient(l_n.y, (coax_p_eg_st(5u)).y, (coax_p_eg_st(6u)).y, (coax_p_eg_st(5u)).w), coax_p_verse_style_ambient(l_n.y, (coax_p_eg_st(5u)).z, (coax_p_eg_st(6u)).z, (coax_p_eg_st(5u)).w)));
  l_c = coax_p_eg_banded(l_base, coax_p_wg_mul_f3f3(coax_p_wg_sw_xyz_f4(coax_p_eg_st(4u)), l_amb), coax_p_wg_add_f3f3(l_key, l_amb), l_band);
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f3(l_base, coax_p_eg_lamp_light(l_world, l_n)));
  l_gain = coax_p_verse_style_chroma_gain(coax_p_eg_saturation(l_c), coax_p_eg_saturation(l_base), 0x1.666666p-1f);
  l_cy = coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z);
  l_c = coax_p_wg_max_f3f3(coax_p_wg_add_f3f3(coax_p_v3(l_cy, l_cy, l_cy), coax_p_wg_mul_f3f(coax_p_wg_sub_f3f3(l_c, coax_p_v3(l_cy, l_cy, l_cy)), l_gain)), coax_p_v3(0.0f, 0.0f, 0.0f));
  l_rs = select((coax_p_eg_st(2u)).w, (coax_p_eg_st(3u)).x, l_character);
  if ((l_rim && (l_rs > 0.0f)))
  {
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(2u)), coax_p_verse_style_rim(coax_p_wg_dot3(l_n, coax_p_wg_neg_f3(l_v)), coax_p_wg_dot3(l_n, l_l), (coax_p_eg_st(3u)).y, (coax_p_eg_st(3u)).z, l_rs)), l_lit));
  }
  if ((!l_paint_leaf_far))
  {
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(coax_p_wg_mul_ff3(coax_p_eg_water_caustics(l_world, l_n), l_base), l_key), l_lit));
  }
  l_c = coax_p_eg_weather_surface(l_c, l_base, l_n, l_world, l_v, l_lit, l_character);
  l_d = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world));
  l_fog = coax_p_eg_haze_share(l_d, l_world.y);
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_eg_haze_colour(l_fog, l_v), l_fog);
  l_c = coax_p_eg_weather_streaks(l_c, l_v, l_d);
  l_c = coax_p_eg_water_under_view(l_c, l_world, l_v);
  l_b = coax_p_eg_st(19u);
  return coax_t23(coax_p_v4(l_c.x, l_c.y, l_c.z, coax_p_verse_style_bloom_share(coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z), l_b.x, l_b.y)), coax_p_v4((coax_p_verse_style_shaft_source(coax_p_wg_dot3(l_v, l_l)) * l_fog), (l_d / 0x1.000000p8f), 0.0f, 0.0f));
}
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_wg_min_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmin(l_p0.x, l_p1.x), coax_p_fmin(l_p0.y, l_p1.y), coax_p_fmin(l_p0.z, l_p1.z));
}
fn coax_p_wg_add_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a + l_b.x), (l_a + l_b.y), (l_a + l_b.z));
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_eg_room_warm(p_k: f32) -> vec3f {
  var l_k: f32 = p_k;
  return coax_p_wg_mix_f3f3f(coax_p_v3(0x1.000000p0f, 0x1.70a3d8p-2f, 0x1.47ae14p-4f), coax_p_v3(0x1.000000p0f, 0x1.000000p-1f, 0x1.47ae14p-3f), l_k);
}
fn coax_p_eg_water_surface_part(p_part: u32) -> bool {
  var l_part: u32 = p_part;
  return ((l_part < 3u) || (l_part == 6u));
}
fn coax_p_eg_water_halves(p_i: coax_t19, p_paint_leaf_far: bool, p_wake_foam: ptr<function, array<f32, 1>>) -> coax_t39 {
  var l_i: coax_t19 = p_i;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_s: coax_t39;
  var l_d: f32;
  var l_fog: f32;
  var l_emit: vec3f;
  var l_o: coax_t39;
  l_s = coax_p_eg_water_fragment(l_i, l_paint_leaf_far, p_wake_foam);
  l_d = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z)));
  l_fog = coax_p_eg_haze_share(l_d, l_i.y.y);
  l_emit = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_s.x, (0x1.000000p0f - l_fog)), coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f(coax_p_eg_haze_colour(l_fog, coax_p_wg_div_f3f(coax_p_wg_sub_f3f3(coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z), coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), coax_p_fmax(l_d, 0x1.0624dep-10f))), l_fog), coax_p_wg_sub_f3f3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_s.y)));
  l_o = l_s;
  l_o.x = coax_p_wg_max_f3f3(l_emit, coax_p_v3(0.0f, 0.0f, 0.0f));
  return l_o;
}
fn coax_p_wg_clamp_f3f3f3(p_p0: vec3f, p_p1: vec3f, p_p2: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: vec3f = p_p2;
  return coax_p_v3(coax_p_clampf(l_p0.x, l_p1.x, l_p2.x), coax_p_clampf(l_p0.y, l_p1.y, l_p2.y), coax_p_clampf(l_p0.z, l_p1.z, l_p2.z));
}
fn coax_p_eg_water_whole(p_i: coax_t19, p_paint_leaf_far: bool, p_wake_foam: ptr<function, array<f32, 1>>) -> vec3f {
  var l_i: coax_t19 = p_i;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_world: vec3f;
  var l_px: coax_t38;
  var l_s: coax_t39;
  var l_w: f32;
  var l_thick: f32;
  var l_focal: f32;
  var l_small: bool;
  var l_reach: f32;
  var l_off: coax_t38;
  var l_at: coax_t38;
  var l_behind: vec3f;
  var l_out: vec3f;
  var l_v: vec3f;
  var l_calm: vec3f;
  var l_hit: vec4f;
  var l_d: f32;
  var l_part: u32;
  var l_bed: f32;
  var l_near: f32;
  var l_t: f32;
  var l_broken: f32;
  var l_foam: f32;
  var l_foam_rgb: vec3f;
  l_world = coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z);
  l_px = coax_p_wg_cvt_i2f2(coax_p_v2(l_i.x.x, l_i.x.y));
  if (((coax_p_eg_water_hull(coax_p_v2(l_world.x, l_world.z)) > 0x1.000000p-1f) || coax_p_eg_water_dry(coax_p_v2(l_world.x, l_world.z))))
  {
    return coax_p_eg_water_scene_at(l_px);
  }
  l_s = coax_p_eg_water_halves(l_i, l_paint_leaf_far, p_wake_foam);
  l_w = coax_p_fmax(coax_p_wg_dot4(coax_p_wg_at4(20i), coax_p_v4(l_world.x, l_world.y, l_world.z, 0x1.000000p0f)), 0x1.0624dep-10f);
  l_thick = coax_p_eg_water_thickness(l_px, l_world);
  l_focal = ((0x1.000000p-1f * f32((coax_p_wg_dims_2d_1()).y)) * coax_p_wg_length3(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i)))));
  l_small = (coax_at_u(i32(1068i)) < 0x1.000000p3f);
  l_reach = (select(0x1.47ae14p-3f, 0x1.eb851ep-4f, l_small) * coax_p_fmin(l_thick, 0x1.333334p0f));
  l_off = coax_p_wg_cvt_i2f2(coax_p_wg_round_f2(coax_p_wg_div_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_s.z, l_reach), l_focal), l_w)));
  l_at = coax_p_wg_add_i2i2(l_px, l_off);
  if ((coax_p_eg_water_scene_w(l_at) < l_w))
  {
    l_at = l_px;
  }
  l_behind = coax_p_eg_water_scene_at(l_at);
  l_out = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f3(l_behind, l_s.y), l_s.x);
  l_v = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world));
  if ((((!l_small) && (l_s.w.w > 0x1.47ae14p-7f)) && (coax_at_u(i32(25i)) > l_world.y)))
  {
    l_calm = coax_p_wg_normalize3(coax_p_v3((l_s.z.x * 0x1.eb851ep-5f), 0x1.000000p0f, (l_s.z.y * 0x1.eb851ep-5f)));
    l_hit = coax_p_eg_water_ssr(l_world, coax_p_wg_reflect3(coax_p_wg_neg_f3(l_v), l_calm));
    l_d = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world));
    l_out = coax_p_wg_add_f3f3(l_out, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sub_f3f3(coax_p_v3(l_hit.x, l_hit.y, l_hit.z), coax_p_v3(l_s.w.x, l_s.w.y, l_s.w.z)), l_s.w.w), l_hit.w), (0x1.000000p0f - coax_p_eg_haze_share(l_d, l_world.y))));
  }
  l_part = coax_p_wg_u32_f((l_i.f4.x + 0x1.000000p-1f));
  if ((coax_at_u(i32(25i)) > l_world.y))
  {
    l_bed = (l_i.y.w / coax_p_fmax(l_v.y, 0x1.47ae14p-4f));
    l_near = ((0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.99999ap-3f, l_thick)) * (0x1.000000p0f - coax_p_smoothstep(0x1.666666p-2f, 0x1.666666p-1f, (l_thick / coax_p_fmax(l_bed, 0x1.47ae14p-6f)))));
    if (((l_near > 0.0f) && coax_p_eg_water_surface_part(l_part)))
    {
      l_t = coax_at_u(i32(1048i));
      l_broken = coax_p_smoothstep(0x1.000000p-2f, 0x1.333334p-1f, ((coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_i.z.x, l_i.z.y), 0x1.400000p2f), (l_t * 0x1.99999ap-1f))) + (0x1.000000p-1f * l_near)) - 0x1.99999ap-3f));
      l_foam = coax_p_clampf(((l_near * l_broken) * 0x1.666666p0f), 0.0f, 0x1.000000p0f);
      if ((l_foam > 0.0f))
      {
        l_foam_rgb = coax_p_wg_mul_f3f3(coax_p_v3(0x1.e66666p-1f, 0x1.f0a3d8p-1f, 0x1.eb851ep-1f), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_eg_water_host_light(l_world, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_paint_leaf_far), 0x1.ccccccp-1f), coax_p_v3(0x1.eb851ep-5f, 0x1.eb851ep-5f, 0x1.eb851ep-5f)));
        l_out = coax_p_wg_mix_f3f3f(l_out, l_foam_rgb, l_foam);
      }
    }
  }
  l_out = coax_p_wg_mix_f3f3f(coax_p_eg_water_scene_at(l_px), l_out, coax_p_smoothstep(0.0f, 0x1.eb851ep-5f, l_thick));
  return coax_p_wg_max_f3f3(l_out, coax_p_v3(0.0f, 0.0f, 0.0f));
}
fn coax_p_wg_dims_2d() -> coax_t29 {
  return coax_p_u2(textureDimensions(bloom_src).x, textureDimensions(bloom_src).y);
}
fn coax_p_wg_cvt_f2u2(p_p: coax_t29) -> vec2f {
  var l_p: coax_t29 = p_p;
  return coax_p_v2(f32(l_p.x), f32(l_p.y));
}
fn coax_p_wg_div_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a / l_b.x), (l_a / l_b.y));
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_clampf(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return coax_p_fmin(coax_p_fmax(l_x, l_lo), l_hi);
}
fn coax_p_wg_sw_rgb_f4(p_p: vec4f) -> vec3f {
  var l_p: vec4f = p_p;
  return coax_p_v3(l_p.x, l_p.y, l_p.z);
}
fn coax_p_eg_water_waver_at(p_pos: vec2f) -> coax_t38 {
  var l_pos: vec2f = p_pos;
  var l_over: f32;
  var l_t: f32;
  var l_px: f32;
  var l_o: vec2f;
  l_over = coax_p_eg_water_eye_over();
  if ((((!coax_p_eg_weather_desktop()) || (l_over > 0x1.99999ap-3f)) || ((l_over > (-0x1.99999ap-4f)) && (!coax_p_eg_water_pixel_under(coax_p_eg_pixel_ray(l_pos))))))
  {
    return coax_p_wg_cvt_i2f2(l_pos);
  }
  l_t = coax_at_u(i32(1048i));
  l_px = ((0x1.800000p1f * coax_at_u(i32(1i))) / 0x1.c20000p9f);
  l_o = coax_p_wg_mul_f2f(coax_p_v2(sin(((l_pos.y * 0x1.47ae14p-6f) + (l_t * 0x1.b33334p0f))), cos(((l_pos.x * 0x1.16872cp-6f) + (l_t * 0x1.4cccccp0f)))), l_px);
  return coax_p_wg_clamp_i2i2i2(coax_p_wg_cvt_i2f2(coax_p_wg_add_f2f2(l_pos, l_o)), coax_p_i2(0i, 0i), coax_p_wg_sub_i2i(coax_p_wg_cvt_i2f2(coax_p_wg_at2(0i)), 1i));
}
fn coax_p_wg_load_2d(p_c: coax_t38, p_level: i32) -> vec4f {
  var l_c: coax_t38 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_light, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_eg_water_meniscus(p_c: vec3f, p_pos: vec2f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_pos: vec2f = p_pos;
  var l_line: f32;
  if (((!coax_p_eg_weather_desktop()) || (abs(coax_p_eg_water_eye_over()) > 0x1.99999ap-3f)))
  {
    return l_c;
  }
  l_line = (coax_p_eg_water_eye_over() + (0x1.99999ap-4f * (coax_p_eg_pixel_ray(l_pos)).y));
  if ((l_line >= 0.0f))
  {
    return l_c;
  }
  return coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.b33334p-1f, 0x1.e66666p-1f, 0x1.e66666p-1f), ((0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.89374cp-8f, (-l_line))) * 0x1.666666p-1f));
}
fn coax_p_wg_at2(p_at: i32) -> vec2f {
  var l_at: i32 = p_at;
  return coax_p_v2(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))));
}
fn coax_p_wg_div_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x / l_b.x), (l_a.y / l_b.y));
}
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t38 {
  var l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
}
fn coax_p_wg_load_2d_1(p_c: coax_t38, p_level: i32) -> vec4f {
  var l_c: coax_t38 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_aux, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_verse_style_bloom_mix(p_c: f32, p_bloom: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_bloom: f32 = p_bloom;
  var l_amount: f32 = p_amount;
  return (l_c + (l_bloom * l_amount));
}
fn coax_p_eg_shafts(p_p: vec2f, p_uv: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_uv: vec2f = p_uv;
  var l_sh: vec4f;
  var l_k: vec4f;
  var l_at: vec2f;
  var l_fade: f32;
  var l_n: f32;
  var l_step: vec2f;
  var l_jitter: f32;
  var l_sum: f32;
  var l_i: f32;
  var l_q: vec2f;
  l_sh = coax_p_eg_st(20u);
  l_k = coax_p_eg_clip(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u)), 0x1.f40000p9f)));
  if (((l_k.w <= 0x1.47ae14p-7f) || (l_sh.z <= 0.0f)))
  {
    return 0.0f;
  }
  l_at = coax_p_v2((0x1.000000p-1f + ((0x1.000000p-1f * l_k.x) / l_k.w)), (0x1.000000p-1f - ((0x1.000000p-1f * l_k.y) / l_k.w)));
  l_fade = coax_p_verse_style_shaft_fade(l_at.x, l_at.y, l_sh.w);
  if ((l_fade <= 0.0f))
  {
    return 0.0f;
  }
  l_n = l_sh.x;
  l_step = coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(l_at, l_uv), l_n);
  l_jitter = coax_p_fract((0x1.a7dd04p5f * coax_p_fract(coax_p_wg_dot2(l_p, coax_p_v2(0x1.12e286p-4f, 0x1.7e8b20p-8f)))));
  l_sum = 0.0f;
  {
    l_i = 0.0f;
    while ((l_i < 0x1.800000p4f))
    {
      if ((l_i >= l_n))
      {
        break;
      }
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
fn coax_p_eg_tone(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_e: f32;
  var l_t: vec3f;
  var l_y: f32;
  l_e = (coax_p_eg_st(18u)).x;
  l_t = coax_p_v3(coax_p_verse_style_tone(l_c.x, l_e), coax_p_verse_style_tone(l_c.y, l_e), coax_p_verse_style_tone(l_c.z, l_e));
  l_y = coax_p_verse_style_luma(l_t.x, l_t.y, l_t.z);
  l_t = coax_p_v3(coax_p_verse_style_saturation(l_t.x, l_y, (coax_p_eg_st(18u)).y), coax_p_verse_style_saturation(l_t.y, l_y, (coax_p_eg_st(18u)).y), coax_p_verse_style_saturation(l_t.z, l_y, (coax_p_eg_st(18u)).y));
  return coax_p_v3(coax_p_verse_style_lift(l_t.x, (coax_p_eg_st(18u)).z), coax_p_verse_style_lift(l_t.y, (coax_p_eg_st(18u)).z), coax_p_verse_style_lift(l_t.z, (coax_p_eg_st(18u)).z));
}
fn coax_p_eg_encode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_wg_pow_f3f3(coax_p_wg_clamp_f3f3f3(l_c, coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), coax_p_wg_make_f3_f((0x1.000000p0f / 0x1.19999ap1f)));
}
fn coax_p_eg_plates(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_k: i32;
  l_c = l_under;
  {
    l_k = 0i;
    while ((l_k < 64i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_c = coax_p_eg_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 248i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 248i)));
      l_k = (l_k + 1i);
    }
  }
  return l_c;
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
fn coax_p_eg_height(p_x: i32, p_z: i32) -> f32 {
  var l_x: i32 = p_x;
  var l_z: i32 = p_z;
  var l_n: i32;
  var l_cx: i32;
  var l_cz: i32;
  l_n = coax_p_wg_i32_f(coax_at_u(i32(32i)));
  l_cx = coax_p_wg_clamp_i(l_x, 0i, l_n);
  l_cz = coax_p_wg_clamp_i(l_z, 0i, l_n);
  return heights[i32(u32(((l_cz * (l_n + 1i)) + l_cx)))];
}
fn coax_p_eg_box_vertex(p_id: u32) -> coax_t32 {
  var l_id: u32 = p_id;
  var l_f: u32;
  var l_c: vec2f;
  var l_n: vec3f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l__sw2: u32;
  l_f = (l_id / 6u);
  l_c = coax_p_wg_sub_f2f2(coax_p_eg_quad((l_id % 6u)), coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f));
  l_n = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_a = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_b = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l__sw2 = l_f;
    if ((l__sw2 == 0u))
    {
      l_n = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
      l_a = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
      l_b = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
    }
    else
    {
      if ((l__sw2 == 1u))
      {
        l_n = coax_p_v3((-0x1.000000p0f), 0.0f, 0.0f);
        l_a = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
        l_b = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
      }
      else
      {
        if ((l__sw2 == 2u))
        {
          l_n = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
          l_a = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
          l_b = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
        }
        else
        {
          if ((l__sw2 == 3u))
          {
            l_n = coax_p_v3(0.0f, (-0x1.000000p0f), 0.0f);
            l_a = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
            l_b = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
          }
          else
          {
            if ((l__sw2 == 4u))
            {
              l_n = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
              l_a = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
              l_b = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
            }
            else
            {
              l_n = coax_p_v3(0.0f, 0.0f, (-0x1.000000p0f));
              l_a = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
              l_b = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
            }
          }
        }
      }
    }
  }
  return coax_t32(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_n, 0x1.000000p-1f), coax_p_wg_mul_f3f(l_a, l_c.x)), coax_p_wg_mul_f3f(l_b, l_c.y)), l_n);
}
fn coax_p_eg_sphere_vertex(p_id: u32) -> coax_t32 {
  var l_id: u32 = p_id;
  var l_q: u32;
  var l_c: vec2f;
  var l_lon: f32;
  var l_lat: f32;
  var l_n: vec3f;
  l_q = (l_id / 6u);
  l_c = coax_p_eg_quad((l_id % 6u));
  l_lon = (((f32((l_q % 16u)) + l_c.x) / 0x1.000000p4f) * 0x1.921fb6p2f);
  l_lat = (((f32((l_q / 16u)) + l_c.y) / 0x1.400000p3f) * 0x1.921fb6p1f);
  l_n = coax_p_v3((sin(l_lat) * cos(l_lon)), cos(l_lat), (sin(l_lat) * sin(l_lon)));
  return coax_t32(coax_p_wg_mul_f3f(l_n, 0x1.000000p-1f), l_n);
}
fn coax_p_eg_cylinder_vertex(p_id: u32) -> coax_t32 {
  var l_id: u32 = p_id;
  var l_c: vec2f;
  var l_a: f32;
  var l_n: vec3f;
  var l_t: u32;
  var l_k: u32;
  var l_top: bool;
  var l_y: f32;
  var l_n_8: vec3f;
  var l_a_9: f32;
  if ((l_id < 96u))
  {
    l_c = coax_p_eg_quad((l_id % 6u));
    l_a = (((f32((l_id / 6u)) + l_c.x) / 0x1.000000p4f) * 0x1.921fb6p2f);
    l_n = coax_p_v3(cos(l_a), 0.0f, sin(l_a));
    return coax_t32(coax_p_v3((l_n.x * 0x1.000000p-1f), (l_c.y - 0x1.000000p-1f), (l_n.z * 0x1.000000p-1f)), l_n);
  }
  l_t = ((l_id - 96u) / 3u);
  l_k = ((l_id - 96u) % 3u);
  l_top = (l_t < 16u);
  l_y = select((-0x1.000000p-1f), 0x1.000000p-1f, l_top);
  l_n_8 = coax_p_v3(0.0f, select((-0x1.000000p0f), 0x1.000000p0f, l_top), 0.0f);
  if ((l_k == 0u))
  {
    return coax_t32(coax_p_v3(0.0f, l_y, 0.0f), l_n_8);
  }
  l_a_9 = (((f32((l_t % 16u)) + f32((l_k - 1u))) / 0x1.000000p4f) * 0x1.921fb6p2f);
  return coax_t32(coax_p_v3((cos(l_a_9) * 0x1.000000p-1f), l_y, (sin(l_a_9) * 0x1.000000p-1f)), l_n_8);
}
fn coax_p_eg_word(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(vertices[i32(l_i)]);
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_wg_unpack4x8snorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4(coax_p_wg_snorm8(coax_p_wg_byte(l_x, 0u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 8u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 16u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 24u)));
}
fn coax_p_eg_canopy_n(p_n: vec3f, p_blob: vec3f) -> vec3f {
  var l_n: vec3f = p_n;
  var l_blob: vec3f = p_blob;
  return coax_p_wg_normalize3(coax_p_v3(coax_p_verse_style_canopy_normal(l_n.x, l_blob.x, 0x1.b33334p-1f), coax_p_verse_style_canopy_normal(l_n.y, l_blob.y, 0x1.b33334p-1f), coax_p_verse_style_canopy_normal(l_n.z, l_blob.z, 0x1.b33334p-1f)));
}
fn coax_p_wg_unpack2x16unorm(p_x: u32) -> vec2f {
  var l_x: u32 = p_x;
  return coax_p_v2((f32((l_x & 65535u)) / 0x1.fffe00p15f), (f32((l_x >> u32(16u))) / 0x1.fffe00p15f));
}
fn coax_p_EG_PERSON_LOW() -> vec3f {
  return coax_p_v3((-0x1.400000p0f), (-0x1.000000p-2f), (-0x1.400000p0f));
}
fn coax_p_wg_mul_f4f(p_a: vec4f, p_b: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v4((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b), (l_a.w * l_b));
}
fn coax_p_wg_add_f4f(p_a: vec4f, p_b: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v4((l_a.x + l_b), (l_a.y + l_b), (l_a.z + l_b), (l_a.w + l_b));
}
fn coax_p_wg_cvt_u4f4(p_p: vec4f) -> coax_t33 {
  var l_p: vec4f = p_p;
  return coax_p_u4(coax_p_wg_u32_f(l_p.x), coax_p_wg_u32_f(l_p.y), coax_p_wg_u32_f(l_p.z), coax_p_wg_u32_f(l_p.w));
}
fn coax_p_wg_get_u4(p_p: coax_t33, p_i: i32) -> u32 {
  var l_p: coax_t33 = p_p;
  var l_i: i32 = p_i;
  if ((l_i == 0i))
  {
    return l_p.x;
  }
  if ((l_i == 1i))
  {
    return l_p.y;
  }
  if ((l_i == 2i))
  {
    return l_p.z;
  }
  return l_p.w;
}
fn coax_p_wg_get_f4(p_p: vec4f, p_i: i32) -> f32 {
  var l_p: vec4f = p_p;
  var l_i: i32 = p_i;
  if ((l_i == 0i))
  {
    return l_p.x;
  }
  if ((l_i == 1i))
  {
    return l_p.y;
  }
  if ((l_i == 2i))
  {
    return l_p.z;
  }
  return l_p.w;
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return coax_p_dot4(l_a, l_b);
}
fn coax_p_eg_lathe_vertex(p_id: u32, p_word: u32) -> coax_t32 {
  var l_id: u32 = p_id;
  var l_word: u32 = p_word;
  var l_profile: u32;
  var l_wedge: u32;
  var l_q: u32;
  var l_c: vec2f;
  var l_seg: u32;
  var l_sector: u32;
  var l_a: f32;
  var l_j: u32;
  var l_p: vec2f;
  var l_n2: vec2f;
  var l_other: vec2f;
  var l_dir: vec3f;
  l_profile = (l_word / 8u);
  l_wedge = (l_word % 8u);
  l_q = (l_id / 6u);
  l_c = coax_p_eg_quad((l_id % 6u));
  l_seg = (l_q % 28u);
  l_sector = (l_q / 28u);
  l_a = (((f32(((l_wedge * 6u) + l_sector)) + l_c.x) / 0x1.800000p5f) * 0x1.921fb6p2f);
  l_j = (l_seg + coax_p_wg_u32_f(l_c.y));
  l_p = coax_p_eg_lathe_point(l_profile, l_j);
  l_n2 = coax_p_eg_lathe_segment_n(l_profile, l_seg);
  l_other = coax_p_v2(0.0f, 0.0f);
  if ((l_c.y > 0x1.000000p-1f))
  {
    l_other = coax_p_eg_lathe_segment_n(l_profile, (l_seg + 1u));
  }
  else
  {
    if ((l_seg > 0u))
    {
      l_other = coax_p_eg_lathe_segment_n(l_profile, (l_seg - 1u));
    }
  }
  if ((coax_p_wg_dot2(l_n2, l_other) > 0x1.800000p-1f))
  {
    l_n2 = coax_p_wg_normalize2(coax_p_wg_add_f2f2(l_n2, l_other));
  }
  if ((coax_p_wg_dot2(l_n2, l_n2) < 0x1.000000p-1f))
  {
    l_n2 = coax_p_v2(0.0f, 0x1.000000p0f);
  }
  l_dir = coax_p_v3(cos(l_a), 0.0f, sin(l_a));
  return coax_t32(coax_p_v3((l_dir.x * l_p.x), l_p.y, (l_dir.z * l_p.x)), coax_p_wg_normalize3(coax_p_v3((l_dir.x * l_n2.x), l_n2.y, (l_dir.z * l_n2.x))));
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
fn coax_p_eg_cone_vertex(p_id: u32) -> coax_t32 {
  var l_id: u32 = p_id;
  var l_t: u32;
  var l_k: u32;
  var l_mid: f32;
  var l_a: f32;
  var l_n: vec3f;
  var l_a_6: f32;
  l_t = ((l_id % 48u) / 3u);
  l_k = (l_id % 3u);
  if ((l_id < 48u))
  {
    l_mid = (((f32(l_t) + 0x1.000000p-1f) / 0x1.000000p4f) * 0x1.921fb6p2f);
    if ((l_k == 0u))
    {
      return coax_t32(coax_p_v3(0.0f, 0x1.000000p-1f, 0.0f), coax_p_wg_normalize3(coax_p_v3(cos(l_mid), 0x1.000000p-1f, sin(l_mid))));
    }
    l_a = (((f32(l_t) + f32((l_k - 1u))) / 0x1.000000p4f) * 0x1.921fb6p2f);
    return coax_t32(coax_p_v3((cos(l_a) * 0x1.000000p-1f), (-0x1.000000p-1f), (sin(l_a) * 0x1.000000p-1f)), coax_p_wg_normalize3(coax_p_v3(cos(l_a), 0x1.000000p-1f, sin(l_a))));
  }
  l_n = coax_p_v3(0.0f, (-0x1.000000p0f), 0.0f);
  if ((l_k == 0u))
  {
    return coax_t32(coax_p_v3(0.0f, (-0x1.000000p-1f), 0.0f), l_n);
  }
  l_a_6 = (((f32(l_t) + f32((l_k - 1u))) / 0x1.000000p4f) * 0x1.921fb6p2f);
  return coax_t32(coax_p_v3((cos(l_a_6) * 0x1.000000p-1f), (-0x1.000000p-1f), (sin(l_a_6) * 0x1.000000p-1f)), l_n);
}
fn coax_p_eg_canopy_sway(p_at: vec3f, p_share: f32) -> vec3f {
  var l_at: vec3f = p_at;
  var l_share: f32 = p_share;
  var l_t: f32;
  var l_theta: f32;
  var l_d: f32;
  l_t = select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
  l_theta = coax_p_verse_style_wind(coax_p_wg_dot2(coax_p_v2(l_at.x, l_at.z), coax_p_EG_WIND_DIR()), coax_p_verse_style_gust(l_at.x, l_at.z, l_t, (coax_p_EG_WIND_DIR()).x, (coax_p_EG_WIND_DIR()).y, (coax_p_EG_WIND()).z, (coax_p_EG_WIND()).w), l_t, (coax_p_EG_WIND()).x, (coax_p_EG_WIND()).y, (coax_p_EG_WIND()).z);
  l_d = coax_p_verse_style_canopy_sway(l_theta, (coax_p_EG_WIND()).x, 0x1.99999ap-5f, l_share);
  return coax_p_v3(((coax_p_EG_WIND_DIR()).x * l_d), 0.0f, ((coax_p_EG_WIND_DIR()).y * l_d));
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
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
fn coax_p_eg_weir_vertex(p_id: u32) -> coax_t19 {
  var l_id: u32 = p_id;
  var l_o: coax_t19;
  var l_qa: u32;
  var l_ql: u32;
  var l_quad_id: u32;
  var l_c: vec2f;
  var l_w0: vec4f;
  var l_w1: vec4f;
  var l_a: f32;
  var l_v: f32;
  var l_rise: f32;
  var l_top: f32;
  var l_drop: f32;
  var l_fall: f32;
  var l_speed: f32;
  var l_tt: f32;
  var l_x: f32;
  var l_world: vec3f;
  var l_along: vec3f;
  var l_n: vec3f;
  l_o = coax_t19();
  l_qa = coax_p_wg_u32_f(coax_at_u(i32(1074i)));
  l_ql = coax_p_wg_u32_f(coax_at_u(i32(1075i)));
  l_quad_id = (l_id / 6u);
  l_c = coax_p_eg_quad((l_id % 6u));
  l_w0 = coax_p_wg_at4(1500i);
  l_w1 = coax_p_wg_at4(1504i);
  if (((l_w0.w < 0x1.000000p-1f) || (l_quad_id >= (l_qa * l_ql))))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    return l_o;
  }
  l_a = ((f32((l_quad_id % l_qa)) + l_c.x) / f32(l_qa));
  l_v = ((f32((l_quad_id / l_qa)) + l_c.y) / f32(l_ql));
  l_rise = coax_at_u(i32(1480i));
  l_top = ((l_w1.x + l_rise) + 0x1.eb851ep-6f);
  l_drop = (coax_p_fmax((l_w1.x - l_w1.y), 0x1.99999ap-5f) + 0x1.1eb852p-3f);
  l_fall = sqrt(((0x1.000000p1f * l_drop) / 0x1.39eb86p3f));
  l_speed = coax_p_fmax(l_w1.z, 0x1.99999ap-2f);
  l_tt = (l_v * l_fall);
  l_x = coax_p_mix(l_w0.x, l_w0.y, l_a);
  l_world = coax_p_v3(l_x, (l_top - ((0x1.39eb86p2f * l_tt) * l_tt)), (l_w0.z - (l_speed * l_tt)));
  l_along = coax_p_wg_normalize3(coax_p_v3(0.0f, ((-0x1.39eb86p3f) * l_tt), (-l_speed)));
  l_n = coax_p_v3(0.0f, (-l_along.z), l_along.y);
  l_o.x = coax_p_eg_clip(l_world);
  l_o.y = coax_p_v4(l_world.x, l_world.y, l_world.z, 0x1.99999ap-5f);
  l_o.z = coax_p_v4(l_n.x, l_n.y, l_n.z, l_v);
  l_o.w = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  l_o.f4 = coax_p_v4(f32(7u), l_a, l_v, (l_w0.y - l_w0.x));
  return l_o;
}
fn coax_p_eg_pond_on(p_p: u32) -> bool {
  var l_p: u32 = p_p;
  return (coax_at_u(i32(((i32(l_p) * 4i) + 1486i))) > 0x1.000000p-1f);
}
fn coax_p_eg_pond_row(p_p: u32) -> vec4f {
  var l_p: u32 = p_p;
  var l_q: vec4f;
  l_q = coax_p_wg_at4(((i32(l_p) * 4i) + 1484i));
  l_q.w = (l_q.w + coax_at_u(i32(1480i)));
  return l_q;
}
fn coax_p_eg_fountain_on() -> bool {
  return (coax_at_u(i32(1079i)) > 0x1.000000p-1f);
}
fn coax_p_eg_fountain_base() -> f32 {
  return coax_at_u(i32(1078i));
}
fn coax_p_eg_fountain_floor(p_r: f32) -> f32 {
  var l_r: f32 = p_r;
  if ((l_r < 0x1.333334p0f))
  {
    return coax_p_mix(0x1.99999ap-5f, 0x1.99999ap-4f, (l_r / 0x1.333334p0f));
  }
  if ((l_r < 0x1.000000p1f))
  {
    return coax_p_mix(0x1.99999ap-4f, 0x1.000000p-2f, ((l_r - 0x1.333334p0f) / 0x1.99999ap-1f));
  }
  if ((l_r < 0x1.333334p1f))
  {
    return coax_p_mix(0x1.000000p-2f, 0x1.851eb8p-2f, ((l_r - 0x1.000000p1f) / 0x1.99999ap-2f));
  }
  if ((l_r < 0x1.451eb8p1f))
  {
    return coax_p_mix(0x1.851eb8p-2f, 0x1.d70a3ep-2f, ((l_r - 0x1.333334p1f) / 0x1.1eb852p-3f));
  }
  return coax_p_mix(0x1.d70a3ep-2f, 0x1.1eb852p-1f, coax_p_clampf(((l_r - 0x1.451eb8p1f) / 0x1.eb851ep-5f), 0.0f, 0x1.000000p0f));
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
fn coax_p_eg_fountain_stream_near(p_a: f32) -> f32 {
  var l_a: f32 = p_a;
  return coax_pow_bg_vs(coax_p_fmax((-cos((0x1.000000p3f * l_a))), 0.0f), 0x1.800000p2f);
}
fn coax_p_eg_fountain_fall(p_k: u32, p_jet: bool, p_v: f32) -> coax_t35 {
  var l_k: u32 = p_k;
  var l_jet: bool = p_jet;
  var l_v: f32 = p_v;
  var l_f: coax_t35;
  var l_base: f32;
  var l_a: f32;
  var l_dir: vec3f;
  var l_t: f32;
  var l_a_9: f32;
  var l_dir_10: vec3f;
  var l_t_11: f32;
  l_f = coax_t35();
  l_base = coax_p_eg_fountain_base();
  if (l_jet)
  {
    l_a = ((((f32(l_k) + 0x1.000000p-1f) / 0x1.000000p2f) * 0x1.921fb6p2f) + (0x1.921fb6p2f / 0x1.000000p4f));
    l_dir = coax_p_v3(cos(l_a), 0.0f, sin(l_a));
    l_t = (l_v * (coax_p_EG_F_JET()).z);
    l_f.x = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_dir, ((coax_p_EG_F_JET()).x + ((coax_p_EG_F_JET_V()).x * l_t))), coax_p_v3(0.0f, (((l_base + (coax_p_EG_F_JET()).y) + ((coax_p_EG_F_JET_V()).y * l_t)) - (((0x1.000000p-1f * 0x1.39999ap3f) * l_t) * l_t)), 0.0f));
    l_f.y = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_dir, (coax_p_EG_F_JET_V()).x), coax_p_v3(0.0f, ((coax_p_EG_F_JET_V()).y - (0x1.39999ap3f * l_t)), 0.0f)));
    l_f.z = coax_p_mix(0x1.0624dep-5f, 0x1.47ae14p-6f, l_v);
    return l_f;
  }
  l_a_9 = coax_p_eg_fountain_stream_angle(l_k);
  l_dir_10 = coax_p_v3(cos(l_a_9), 0.0f, sin(l_a_9));
  l_t_11 = (l_v * 0x1.1c28f6p-1f);
  l_f.x = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_dir_10, ((coax_p_EG_F_LIP()).x + (0x1.666666p-1f * l_t_11))), coax_p_v3(0.0f, ((l_base + (coax_p_EG_F_LIP()).y) - (((0x1.000000p-1f * 0x1.39999ap3f) * l_t_11) * l_t_11)), 0.0f));
  l_f.y = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_dir_10, 0x1.666666p-1f), coax_p_v3(0.0f, ((-0x1.39999ap3f) * l_t_11), 0.0f)));
  l_f.z = coax_p_mix(0x1.333334p-4f, 0x1.99999ap-5f, l_v);
  return l_f;
}
fn coax_p_eg_fountain_stream_angle(p_k: u32) -> f32 {
  var l_k: u32 = p_k;
  return (((f32(l_k) + 0x1.000000p-1f) / 0x1.000000p3f) * 0x1.921fb6p2f);
}
fn coax_p_wg_clamp_f2f2f2(p_p0: vec2f, p_p1: vec2f, p_p2: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  var l_p2: vec2f = p_p2;
  return coax_p_v2(coax_p_clampf(l_p0.x, l_p1.x, l_p2.x), coax_p_clampf(l_p0.y, l_p1.y, l_p2.y));
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
fn coax_p_eg_water_node(p_k: u32, p_f: u32) -> f32 {
  var l_k: u32 = p_k;
  var l_f: u32 = p_f;
  return water_nodes[i32(((l_k * 8u) + l_f))];
}
fn coax_p_eg_water_shoal(p_k: f32, p_depth: f32) -> f32 {
  var l_k: f32 = p_k;
  var l_depth: f32 = p_depth;
  return sqrt(coax_p_wg_tanh(coax_p_clampf((l_k * l_depth), 0.0f, 0x1.400000p3f)));
}
fn coax_p_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x * l_b.x) + (l_a.y * l_b.y));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_fract_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
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
fn coax_p_eg_unview() -> coax_t40 {
  var l_m: coax_t40;
  var l_a00: f32;
  var l_a01: f32;
  var l_a02: f32;
  var l_a03: f32;
  var l_a10: f32;
  var l_a11: f32;
  var l_a12: f32;
  var l_a13: f32;
  var l_a20: f32;
  var l_a21: f32;
  var l_a22: f32;
  var l_a23: f32;
  var l_a30: f32;
  var l_a31: f32;
  var l_a32: f32;
  var l_a33: f32;
  var l_b00: f32;
  var l_b01: f32;
  var l_b02: f32;
  var l_b03: f32;
  var l_b04: f32;
  var l_b05: f32;
  var l_b06: f32;
  var l_b07: f32;
  var l_b08: f32;
  var l_b09: f32;
  var l_b10: f32;
  var l_b11: f32;
  var l_det: f32;
  var l_k: f32;
  l_m = coax_p_wg_transpose(coax_t40(coax_p_wg_at4(8i), coax_p_wg_at4(12i), coax_p_wg_at4(16i), coax_p_wg_at4(20i)));
  l_a00 = l_m.x.x;
  l_a01 = l_m.x.y;
  l_a02 = l_m.x.z;
  l_a03 = l_m.x.w;
  l_a10 = l_m.y.x;
  l_a11 = l_m.y.y;
  l_a12 = l_m.y.z;
  l_a13 = l_m.y.w;
  l_a20 = l_m.z.x;
  l_a21 = l_m.z.y;
  l_a22 = l_m.z.z;
  l_a23 = l_m.z.w;
  l_a30 = l_m.w.x;
  l_a31 = l_m.w.y;
  l_a32 = l_m.w.z;
  l_a33 = l_m.w.w;
  l_b00 = ((l_a00 * l_a11) - (l_a01 * l_a10));
  l_b01 = ((l_a00 * l_a12) - (l_a02 * l_a10));
  l_b02 = ((l_a00 * l_a13) - (l_a03 * l_a10));
  l_b03 = ((l_a01 * l_a12) - (l_a02 * l_a11));
  l_b04 = ((l_a01 * l_a13) - (l_a03 * l_a11));
  l_b05 = ((l_a02 * l_a13) - (l_a03 * l_a12));
  l_b06 = ((l_a20 * l_a31) - (l_a21 * l_a30));
  l_b07 = ((l_a20 * l_a32) - (l_a22 * l_a30));
  l_b08 = ((l_a20 * l_a33) - (l_a23 * l_a30));
  l_b09 = ((l_a21 * l_a32) - (l_a22 * l_a31));
  l_b10 = ((l_a21 * l_a33) - (l_a23 * l_a31));
  l_b11 = ((l_a22 * l_a33) - (l_a23 * l_a32));
  l_det = ((((((l_b00 * l_b11) - (l_b01 * l_b10)) + (l_b02 * l_b09)) + (l_b03 * l_b08)) - (l_b04 * l_b07)) + (l_b05 * l_b06));
  l_k = (0x1.000000p0f / select(l_det, 0x1.12e0bep-30f, (abs(l_det) < 0x1.197998p-40f)));
  return coax_t40(coax_p_wg_mul_f4f(coax_p_v4((((l_a11 * l_b11) - (l_a12 * l_b10)) + (l_a13 * l_b09)), (((l_a02 * l_b10) - (l_a01 * l_b11)) - (l_a03 * l_b09)), (((l_a31 * l_b05) - (l_a32 * l_b04)) + (l_a33 * l_b03)), (((l_a22 * l_b04) - (l_a21 * l_b05)) - (l_a23 * l_b03))), l_k), coax_p_wg_mul_f4f(coax_p_v4((((l_a12 * l_b08) - (l_a10 * l_b11)) - (l_a13 * l_b07)), (((l_a00 * l_b11) - (l_a02 * l_b08)) + (l_a03 * l_b07)), (((l_a32 * l_b02) - (l_a30 * l_b05)) - (l_a33 * l_b01)), (((l_a20 * l_b05) - (l_a22 * l_b02)) + (l_a23 * l_b01))), l_k), coax_p_wg_mul_f4f(coax_p_v4((((l_a10 * l_b10) - (l_a11 * l_b08)) + (l_a13 * l_b06)), (((l_a01 * l_b08) - (l_a00 * l_b10)) - (l_a03 * l_b06)), (((l_a30 * l_b04) - (l_a31 * l_b02)) + (l_a33 * l_b00)), (((l_a21 * l_b02) - (l_a20 * l_b04)) - (l_a23 * l_b00))), l_k), coax_p_wg_mul_f4f(coax_p_v4((((l_a11 * l_b07) - (l_a10 * l_b09)) - (l_a12 * l_b06)), (((l_a00 * l_b09) - (l_a01 * l_b07)) + (l_a02 * l_b06)), (((l_a31 * l_b01) - (l_a30 * l_b03)) - (l_a32 * l_b00)), (((l_a20 * l_b03) - (l_a21 * l_b01)) + (l_a22 * l_b00))), l_k));
}
fn coax_p_m4_apply(p_m: coax_t40, p_v: vec4f) -> vec4f {
  var l_m: coax_t40 = p_m;
  var l_v: vec4f = p_v;
  return coax_p_add4(coax_p_add4(coax_p_scale4(l_m.x, l_v.x), coax_p_scale4(l_m.y, l_v.y)), coax_p_add4(coax_p_scale4(l_m.z, l_v.z), coax_p_scale4(l_m.w, l_v.w)));
}
fn coax_p_verse_style_sky_t(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return coax_pow_bg_vs(coax_p_st_max(l_up, 0x1.0c6f7ap-20f), 0x1.19999ap-1f);
}
fn coax_p_eg_saturation(p_c: vec3f) -> f32 {
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
fn coax_p_eg_stars(p_d: vec3f) -> f32 {
  var l_d: vec3f = p_d;
  var l_q: vec2f;
  var l_g: vec2f;
  var l_h: f32;
  var l_at: vec2f;
  if ((l_d.y <= 0.0f))
  {
    return 0.0f;
  }
  l_q = coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), (l_d.y + 0x1.666666p-2f)), 0x1.040000p8f);
  l_g = coax_p_wg_floor_f2(l_q);
  l_h = coax_p_eg_hash2(l_g);
  if ((l_h < 0x1.dc28f6p-1f))
  {
    return 0.0f;
  }
  l_at = coax_p_wg_add_f2f(coax_p_wg_add_f2f2(l_g, coax_p_wg_mul_f2f(coax_p_v2(coax_p_eg_hash2(coax_p_wg_add_f2f(l_g, 0x1.b33334p0f)), coax_p_eg_hash2(coax_p_wg_add_f2f(l_g, 0x1.233334p3f))), 0x1.99999ap-1f)), 0x1.99999ap-4f);
  return (select(0x1.47ae14p-2f, 0x1.800000p-1f, (l_h > 0x1.fbe76cp-1f)) * (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-4f, 0x1.ae147ap-2f, coax_p_wg_length2(coax_p_wg_sub_f2f2(l_q, l_at)))));
}
fn coax_p_wg_make_f3_f(p_p0: f32) -> vec3f {
  var l_p0: f32 = p_p0;
  return coax_p_v3(l_p0, l_p0, l_p0);
}
fn coax_p_eg_ridge_ring(p_i: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l_r: array<vec4f, 3>;
  l_r = array<vec4f, 3>(coax_p_v4(0x1.400000p7f, 0x1.a00000p4f, 0x1.800000p1f, 0x1.c00000p2f), coax_p_v4(0x1.040000p8f, 0x1.200000p6f, 0x1.000000p1f, 0.0f), coax_p_v4(0x1.a40000p8f, 0x1.180000p7f, 0x1.4cccccp0f, 0.0f));
  return l_r[i32(l_i)];
}
fn coax_p_eg_hash2(p_p: vec2f) -> f32 {
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
fn coax_p_verse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_sky: f32 = p_sky;
  var l_ground: f32 = p_ground;
  var l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
}
fn coax_p_eg_haze_share(p_d: f32, p_y: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_y: f32 = p_y;
  return coax_p_verse_style_haze(l_d, l_y, (coax_at_u(i32(25i)) - 0x1.400000p1f), coax_at_u(i32(31i)), coax_at_u(i32(27i)), (coax_p_eg_st(9u)).z, (coax_p_eg_st(9u)).w);
}
fn coax_p_eg_haze_colour(p_f: f32, p_v: vec3f) -> vec3f {
  var l_f: f32 = p_f;
  var l_v: vec3f = p_v;
  var l_hz: vec3f;
  var l_cap: f32;
  var l_h: vec3f;
  var l_green: vec3f;
  var l_level: f32;
  l_hz = coax_p_wg_sw_xyz_f4(coax_p_eg_st(10u));
  l_cap = (coax_p_eg_st(9u)).z;
  l_h = coax_p_v3(coax_p_verse_style_haze_hue(l_hz.x, 0.0f, l_f, l_cap), coax_p_verse_style_haze_hue(l_hz.y, 0x1.000000p0f, l_f, l_cap), coax_p_verse_style_haze_hue(l_hz.z, 0x1.000000p1f, l_f, l_cap));
  l_green = coax_p_wg_div_f3f(coax_p_eg_meadow_srgb(0x1.000000p-1f), coax_p_wg_dot3(coax_p_eg_meadow_srgb(0x1.000000p-1f), coax_p_v3(0x1.554c98p-2f, 0x1.554c98p-2f, 0x1.554c98p-2f)));
  l_level = (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-5f, 0x1.666666p-2f, abs(l_v.y)));
  l_h = coax_p_wg_mix_f3f3f(l_h, coax_p_wg_mul_f3f3(l_h, l_green), ((0x1.851eb8p-2f * l_level) * (0x1.000000p0f - (coax_p_eg_st(14u)).w)));
  return coax_p_wg_mix_f3f3f(l_h, coax_p_wg_sw_xyz_f4(coax_p_eg_st(11u)), coax_p_verse_style_haze_glow(coax_p_wg_dot3(l_v, coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u)))));
}
fn coax_p_eg_weather_desktop() -> bool {
  return (coax_at_u(i32(1068i)) >= 0x1.000000p3f);
}
fn coax_p_wg_exp2(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return exp((l_x * 0x1.62e430p-1f));
}
fn coax_p_eg_weather_covered(p_p: vec3f) -> bool {
  var l_p: vec3f = p_p;
  var l_n: u32;
  var l_i: u32;
  var l_b: vec4f;
  if (((coax_p_wg_all_ge_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_wg_sw_xy_f4(coax_p_EG_RAIN_BRIDGE())) && coax_p_wg_all_le_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_wg_sw_zw_f4(coax_p_EG_RAIN_BRIDGE()))) && (l_p.y < coax_at_u(i32(1482i)))))
  {
    return true;
  }
  l_n = coax_p_wg_u32_f(coax_at_u(i32(1483i)));
  {
    l_i = 0u;
    while ((l_i < 32u))
    {
      if ((l_i >= l_n))
      {
        break;
      }
      l_b = coax_p_wg_at4(((i32(l_i) * 4i) + 1212i));
      if ((coax_p_wg_all_ge_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_v2(l_b.x, l_b.y)) && coax_p_wg_all_le_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_v2(l_b.z, l_b.w))))
      {
        return true;
      }
      l_i = (l_i + 1u);
    }
  }
  return false;
}
fn coax_p_wg_round(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_r: f32;
  l_r = floor((l_x + 0x1.000000p-1f));
  if ((((l_r - l_x) == 0x1.000000p-1f) && (coax_p_wg_fmod(l_r, 0x1.000000p1f) != 0.0f)))
  {
    return (l_r - 0x1.000000p0f);
  }
  return l_r;
}
fn coax_p_eg_weather_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_eg_water_pixel_under(p_d: vec3f) -> bool {
  var l_d: vec3f = p_d;
  return ((coax_p_eg_water_eye_over() + (0x1.99999ap-4f * l_d.y)) < 0.0f);
}
fn coax_p_eg_water_eye_over() -> f32 {
  return select(0x1.900000p6f, coax_at_u(i32(1071i)), (coax_at_u(i32(1072i)) >= 0x1.000000p1f));
}
fn coax_p_EG_WATER_UNDER_ABSORB() -> vec3f {
  return coax_p_v3(0x1.ae147ap-2f, 0x1.eb851ep-4f, 0x1.70a3d8p-4f);
}
fn coax_p_wg_exp_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(exp(l_p0.x), exp(l_p0.y), exp(l_p0.z));
}
fn coax_p_EG_WATER_UNDER_SCATTER() -> vec3f {
  return coax_p_v3(0x1.99999ap-5f, 0x1.333334p-2f, 0x1.51eb86p-2f);
}
fn coax_p_EG_WATER_UNDER_TINT() -> vec3f {
  return coax_p_v3(0x1.0a3d70p-1f, 0x1.ae147ap-1f, 0x1.e66666p-1f);
}
fn coax_p_eg_water_motes(p_o: vec3f, p_d: vec3f, p_reach: f32) -> f32 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_reach: f32 = p_reach;
  var l_a: f32;
  var l_n: i32;
  var l_i: i32;
  var l_t: f32;
  var l_c: vec3f;
  var l_h: vec4f;
  var l_drift: vec3f;
  var l_m: vec3f;
  var l_along: f32;
  var l_off: f32;
  var l_size: f32;
  l_a = 0.0f;
  l_n = select(6i, 3i, (coax_at_u(i32(1068i)) < 0x1.000000p3f));
  {
    l_i = 0i;
    while ((l_i < l_n))
    {
      l_t = (((f32(l_i) + 0x1.000000p-1f) * l_reach) / f32(l_n));
      l_c = coax_p_wg_floor_f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t)));
      l_h = coax_p_eg_water_mote_hash(l_c);
      if ((l_h.w > 0x1.000000p-2f))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_drift = coax_p_wg_mul_f3f(coax_p_v3(sin(((coax_at_u(i32(1048i)) * 0x1.333334p-2f) + (l_h.x * 0x1.800000p2f))), cos(((coax_at_u(i32(1048i)) * 0x1.d70a3ep-3f) + (l_h.y * 0x1.800000p2f))), 0.0f), 0x1.47ae14p-4f);
      l_m = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(l_c, coax_p_v3(l_h.x, l_h.y, l_h.z)), l_drift);
      l_along = coax_p_fmax(coax_p_wg_dot3(coax_p_wg_sub_f3f3(l_m, l_o), l_d), 0x1.99999ap-5f);
      if ((l_along > l_reach))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_off = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_m, coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_along))));
      l_size = ((0x1.89374cp-7f * l_along) + 0x1.0624dep-8f);
      l_a = (l_a + (((0x1.000000p0f - coax_p_smoothstep((l_size * 0x1.99999ap-2f), l_size, l_off)) * 0x1.000000p-1f) * exp(((-l_along) * 0x1.000000p-2f))));
      l_i = (l_i + 1i);
    }
  }
  return l_a;
}
fn coax_p_eg_water_shafts(p_o: vec3f, p_d: vec3f, p_path: f32, p_level: f32) -> f32 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_path: f32 = p_path;
  var l_level: f32 = p_level;
  var l_sum: f32;
  var l_reach: f32;
  var l_i: i32;
  var l_t: f32;
  var l_q: vec3f;
  var l_below: f32;
  if ((coax_at_u(i32(1068i)) < 0x1.000000p3f))
  {
    return 0.0f;
  }
  l_sum = 0.0f;
  l_reach = coax_p_fmin(l_path, 0x1.000000p3f);
  {
    l_i = 0i;
    while ((l_i < 16i))
    {
      l_t = (((f32(l_i) + 0x1.000000p-1f) / 0x1.000000p4f) * l_reach);
      l_q = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t));
      l_below = coax_p_fmax((l_level - l_q.y), 0.0f);
      l_sum = (l_sum + (coax_p_eg_water_caustic_pattern(coax_p_wg_add_f2f2(coax_p_v2(l_q.x, l_q.z), coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_sw_xz_f4(coax_p_eg_st(1u)), l_below), 0x1.333334p-1f)), coax_at_u(i32(1048i))) * exp((((-l_below) * 0x1.000000p-1f) - (l_t * 0x1.333334p-2f)))));
      l_i = (l_i + 1i);
    }
  }
  return (((l_sum / 0x1.000000p4f) * l_reach) * 0x1.666666p-2f);
}
fn coax_p_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return (((l_a.x * l_b.x) + (l_a.y * l_b.y)) + (l_a.z * l_b.z));
}
fn coax_p_eg_shard_plane(p_cell: u32, p_k: u32) -> vec4f {
  var l_cell: u32 = p_cell;
  var l_k: u32 = p_k;
  var l_o: coax_t11;
  var l_j: u32;
  l_o = objects[i32((l_cell + (l_k / 4u)))];
  l_j = (l_k % 4u);
  if ((l_j == 0u))
  {
    return l_o.x;
  }
  if ((l_j == 1u))
  {
    return l_o.y;
  }
  if ((l_j == 2u))
  {
    return l_o.z;
  }
  return l_o.w;
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
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
fn coax_p_EG_MEADOW_COOL() -> vec3f {
  return coax_p_v3(0x1.000000p-2f, 0x1.851eb8p-2f, 0x1.c28f5cp-3f);
}
fn coax_p_EG_MEADOW_WARM() -> vec3f {
  return coax_p_v3(0x1.a3d70ap-2f, 0x1.f5c290p-2f, 0x1.000000p-2f);
}
fn coax_p_eg_pond_depth_at(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_k: u32;
  var l_q: vec4f;
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      l_q = coax_p_eg_pond_row(l_k);
      if ((l_q.z <= 0x1.000000p-1f))
      {
        {
          l_k = (l_k + 1u);
          continue;
        }
      }
      if (((coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_v2(l_q.x, l_q.y))) < (l_q.z + 0x1.333334p-2f)) && (l_p.y < (l_q.w + 0x1.47ae14p-4f))))
      {
        return (l_q.w - l_p.y);
      }
      l_k = (l_k + 1u);
    }
  }
  return (-0x1.000000p0f);
}
fn coax_p_eg_water_uv(p_xz: vec2f) -> vec2f {
  var l_xz: vec2f = p_xz;
  var l_t: vec2f;
  var l_side: vec2f;
  var l_d: vec2f;
  l_t = coax_p_v2(sin(coax_at_u(i32(1082i))), cos(coax_at_u(i32(1082i))));
  l_side = coax_p_v2((-l_t.y), l_t.x);
  l_d = coax_p_wg_sub_f2f2(l_xz, coax_p_v2(coax_at_u(i32(1080i)), coax_at_u(i32(1081i))));
  return coax_p_v2((coax_p_wg_dot2(l_d, l_side) / coax_p_fmax(coax_at_u(i32(1083i)), 0x1.0624dep-10f)), (coax_p_wg_dot2(l_d, l_t) / coax_p_fmax(coax_at_u(i32(1067i)), 0x1.0624dep-10f)));
}
fn coax_p_eg_water_hash(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_v: coax_t42;
  l_v = coax_p_wg_add_u3u(coax_p_wg_mul_u3u(coax_p_wg_bits_u3i3(coax_p_wg_cvt_i3f3(l_p)), 1664525u), 1013904223u);
  l_v.x = (l_v.x + (l_v.y * l_v.z));
  l_v.y = (l_v.y + (l_v.z * l_v.x));
  l_v.z = (l_v.z + (l_v.x * l_v.y));
  l_v = coax_p_wg_xor_u3u3(l_v, coax_p_wg_shr_u3u3(l_v, coax_p_u3(16u, 16u, 16u)));
  l_v.x = (l_v.x + (l_v.y * l_v.z));
  return (f32((l_v.x >> u32(8u))) / 0x1.fffffep23f);
}
fn coax_p_eg_water_depth_at(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_f: f32;
  var l_q: f32;
  var l_uv: vec2f;
  var l_r: coax_t20;
  l_f = coax_p_eg_fountain_depth_at(l_p);
  if ((l_f > (-0x1.000000p-1f)))
  {
    return l_f;
  }
  l_q = coax_p_eg_pond_depth_at(l_p);
  if ((l_q > (-0x1.000000p-1f)))
  {
    return l_q;
  }
  if (((coax_at_u(i32(1072i)) < 0x1.000000p1f) || (coax_at_u(i32(1083i)) <= 0.0f)))
  {
    return (-0x1.000000p0f);
  }
  l_uv = coax_p_eg_water_uv(coax_p_v2(l_p.x, l_p.z));
  if (((((l_uv.x < 0.0f) || (l_uv.y < 0.0f)) || (l_uv.x > (coax_at_u(i32(1072i)) - 0x1.000000p0f))) || (l_uv.y > (coax_at_u(i32(1073i)) - 0x1.000000p0f))))
  {
    return (-0x1.000000p0f);
  }
  l_r = coax_p_eg_water_rest(l_uv);
  if ((l_r.y <= 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return (l_r.x.y - l_p.y);
}
fn coax_p_wg_dims_depth() -> coax_t29 {
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
fn coax_p_wg_all_gt_f2f2(p_a: vec2f, p_b: vec2f) -> bool {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x > l_b.x) && (l_a.y > l_b.y));
}
fn coax_p_wg_all_lt_f2f2(p_a: vec2f, p_b: vec2f) -> bool {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x < l_b.x) && (l_a.y < l_b.y));
}
fn coax_p_eg_weather_wet_base(p_base: vec3f, p_n: vec3f, p_character: bool) -> vec3f {
  var l_base: vec3f = p_base;
  var l_n: vec3f = p_n;
  var l_character: bool = p_character;
  var l_wet: f32;
  l_wet = coax_p_eg_weather_wet_share(l_n, l_character);
  if ((l_wet <= 0.0f))
  {
    return l_base;
  }
  return coax_p_wg_mul_f3f(l_base, (0x1.000000p0f - (0x1.99999ap-2f * l_wet)));
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
fn coax_p_eg_sky_seen(p_world: vec3f) -> f32 {
  var l_world: vec3f = p_world;
  var l_n: i32;
  var l_gx: f32;
  var l_gz: f32;
  var l_ix: i32;
  var l_iz: i32;
  var l_fx: f32;
  var l_fz: f32;
  var l_p: i32;
  var l_k: i32;
  var l_sky: f32;
  var l_ground: f32;
  var l_seen: f32;
  l_n = coax_p_wg_i32_f(coax_at_u(i32(32i)));
  l_gx = ((l_world.x - coax_at_u(i32(34i))) / coax_at_u(i32(33i)));
  l_gz = ((l_world.z - coax_at_u(i32(35i))) / coax_at_u(i32(33i)));
  l_ix = coax_p_wg_clamp_i(coax_p_wg_i32_f(floor(l_gx)), 0i, (l_n - 1i));
  l_iz = coax_p_wg_clamp_i(coax_p_wg_i32_f(floor(l_gz)), 0i, (l_n - 1i));
  l_fx = coax_p_clampf((l_gx - f32(l_ix)), 0.0f, 0x1.000000p0f);
  l_fz = coax_p_clampf((l_gz - f32(l_iz)), 0.0f, 0x1.000000p0f);
  l_p = ((l_n + 1i) * (l_n + 1i));
  l_k = ((l_iz * (l_n + 1i)) + l_ix);
  l_sky = coax_p_mix(coax_p_mix(heights[i32(u32((l_p + l_k)))], heights[i32(u32(((l_p + l_k) + 1i)))], l_fx), coax_p_mix(heights[i32(u32((((l_p + l_k) + l_n) + 1i)))], heights[i32(u32((((l_p + l_k) + l_n) + 2i)))], l_fx), l_fz);
  l_ground = coax_p_mix(coax_p_mix(coax_p_eg_height(l_ix, l_iz), coax_p_eg_height((l_ix + 1i), l_iz), l_fx), coax_p_mix(coax_p_eg_height(l_ix, (l_iz + 1i)), coax_p_eg_height((l_ix + 1i), (l_iz + 1i)), l_fx), l_fz);
  l_seen = coax_p_mix(l_sky, 0x1.000000p0f, coax_p_smoothstep(0.0f, 0x1.c00000p2f, (l_world.y - l_ground)));
  return coax_p_mix(0x1.000000p0f, l_seen, 0x1.000000p0f);
}
fn coax_p_eg_banded(p_a: vec3f, p_dark: vec3f, p_bright: vec3f, p_band: f32) -> vec3f {
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
  l_t = coax_p_wg_sw_xyz_f4(coax_p_eg_st(4u));
  l_tl = coax_p_verse_style_luma(l_t.x, l_t.y, l_t.z);
  l_k = (coax_p_eg_st(21u)).w;
  l_m = coax_p_v3(coax_p_verse_style_cool(l_s.x, l_t.x, l_tl, l_k), coax_p_verse_style_cool(l_s.y, l_t.y, l_tl, l_k), coax_p_verse_style_cool(l_s.z, l_t.z, l_tl, l_k));
  l_y = coax_p_verse_style_luma(l_s.x, l_s.y, l_s.z);
  l_ym = coax_p_verse_style_luma(l_m.x, l_m.y, l_m.z);
  l_cool = coax_p_v3(coax_p_verse_style_keep_value(l_m.x, l_y, l_ym), coax_p_verse_style_keep_value(l_m.y, l_y, l_ym), coax_p_verse_style_keep_value(l_m.z, l_y, l_ym));
  l_l = coax_p_wg_mul_f3f3(l_a, l_bright);
  return coax_p_v3(coax_p_verse_style_banded(l_cool.x, l_l.x, l_band), coax_p_verse_style_banded(l_cool.y, l_l.y, l_band), coax_p_verse_style_banded(l_cool.z, l_l.z, l_band));
}
fn coax_p_eg_lamp_light(p_p: vec3f, p_n: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_glow: f32;
  var l_count: u32;
  var l_lit_by_day: u32;
  var l_sum: vec3f;
  var l_i: u32;
  var l_at: vec4f;
  var l_c: vec4f;
  var l_delta: vec3f;
  var l_d: f32;
  var l_k: f32;
  var l_face: f32;
  l_glow = coax_at_u(i32(1341i));
  l_count = coax_p_wg_u32_f(coax_at_u(i32(1340i)));
  l_lit_by_day = (l_count - coax_p_wg_min_u(coax_p_wg_u32_f(coax_at_u(i32(1342i))), l_count));
  if (((l_glow <= 0.0f) && (l_lit_by_day == l_count)))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_sum = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_i = 0u;
    while ((l_i < 16u))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_at = coax_p_wg_at4(((i32((l_i * 2u)) * 4i) + 1344i));
      l_c = coax_p_wg_at4(((i32(((l_i * 2u) + 1u)) * 4i) + 1344i));
      l_delta = coax_p_wg_sub_f3f3(coax_p_v3(l_at.x, l_at.y, l_at.z), l_p);
      l_d = coax_p_wg_length3(l_delta);
      if ((l_d >= l_at.w))
      {
        {
          l_i = (l_i + 1u);
          continue;
        }
      }
      l_k = (0x1.000000p0f - ((l_d * l_d) / (l_at.w * l_at.w)));
      l_face = coax_p_fmax(0x1.333334p-3f, ((coax_p_wg_dot3(l_n, coax_p_wg_div_f3f(l_delta, coax_p_fmax(l_d, 0x1.0624dep-10f))) + 0x1.333334p-2f) / 0x1.4cccccp0f));
      l_sum = coax_p_wg_add_f3f3(l_sum, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_c.x, l_c.y, l_c.z), l_c.w), l_k), l_k), l_face), select(l_glow, 0x1.000000p0f, (l_i >= l_lit_by_day))));
      l_i = (l_i + 1u);
    }
  }
  return l_sum;
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
fn coax_p_eg_water_caustics(p_p: vec3f, p_n: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_depth: f32;
  var l_up: f32;
  l_depth = coax_p_eg_water_depth_at(l_p);
  if ((l_depth <= 0.0f))
  {
    return 0.0f;
  }
  l_up = coax_p_smoothstep((-0x1.99999ap-3f), 0x1.333334p-1f, l_n.y);
  return ((((coax_p_eg_water_caustic_pattern(coax_p_v2(l_p.x, l_p.z), coax_at_u(i32(1048i))) * coax_p_smoothstep(0.0f, 0x1.99999ap-4f, l_depth)) * exp(((-l_depth) * 0x1.666666p-1f))) * l_up) * 0x1.666666p0f);
}
fn coax_p_eg_weather_surface(p_c: vec3f, p_base: vec3f, p_n: vec3f, p_world: vec3f, p_v: vec3f, p_lit: f32, p_character: bool) -> vec3f {
  var l_c: vec3f = p_c;
  var l_base: vec3f = p_base;
  var l_n: vec3f = p_n;
  var l_world: vec3f = p_world;
  var l_v: vec3f = p_v;
  var l_lit: f32 = p_lit;
  var l_character: bool = p_character;
  var l_o: vec3f;
  var l_wet: f32;
  var l_l: vec3f;
  var l_key: vec3f;
  var l_r: vec3f;
  var l_puddles: f32;
  var l_m: f32;
  var l_fill: f32;
  var l_s: vec2f;
  var l_pn: vec3f;
  var l_mirror: vec3f;
  var l_f: f32;
  l_o = l_c;
  l_wet = coax_p_eg_weather_wet_share(l_n, l_character);
  l_l = coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u));
  l_key = coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), (coax_p_eg_st(0u)).w);
  if ((l_wet > 0.0f))
  {
    l_r = coax_p_wg_reflect3(l_v, l_n);
    l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_key, coax_p_smoothstep(0x1.dc28f6p-1f, 0x1.f33334p-1f, coax_p_wg_dot3(l_r, l_l))), l_wet), 0x1.666666p-2f), l_lit));
  }
  l_puddles = coax_at_u(i32(1477i));
  if (((((l_puddles > 0.0f) && (!l_character)) && (l_n.y > 0x1.f0a3d8p-1f)) && (abs((l_world.y - coax_p_eg_ground_y(coax_p_v2(l_world.x, l_world.z)))) < 0x1.99999ap-5f)))
  {
    l_m = ((coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_world.x, l_world.z), 0x1.ccccccp-2f), 0x1.000000p-1f)) * 0x1.666666p-1f) + (coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_world.x, l_world.z), 0x1.b33334p0f), 0x1.400000p1f)) * 0x1.333334p-2f));
    l_fill = (0x1.000000p0f - coax_p_smoothstep((l_puddles * 0x1.333334p-2f), ((l_puddles * 0x1.333334p-2f) + 0x1.eb851ep-6f), l_m));
    if ((l_fill > 0.0f))
    {
      l_s = coax_p_eg_weather_rain_slope(coax_p_v2(l_world.x, l_world.z));
      l_pn = coax_p_wg_normalize3(coax_p_v3((-l_s.x), 0x1.000000p0f, (-l_s.y)));
      l_mirror = coax_p_wg_mul_f3f(coax_p_wg_min_f3f3(coax_p_eg_weather_sky(coax_p_eg_sky(coax_p_wg_reflect3(l_v, l_pn), l_l), coax_p_wg_reflect3(l_v, l_pn)), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), 0x1.800000p-1f);
      l_f = (0x1.000000p-2f + (0x1.333334p-1f * coax_pow_bg_vs((0x1.000000p0f - coax_p_fmax((-l_v.y), 0.0f)), 0x1.800000p1f)));
      l_o = coax_p_wg_mix_f3f3f(l_o, coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(l_c, 0x1.19999ap-1f), l_mirror, l_f), (l_fill * 0x1.b33334p-1f));
    }
  }
  l_o = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(l_base, coax_p_v3(0x1.800000p-1f, 0x1.99999ap-1f, 0x1.000000p0f)), coax_at_u(i32(1475i))), 0x1.333334p0f));
  return l_o;
}
fn coax_p_eg_water_fragment(p_i: coax_t19, p_paint_leaf_far: bool, p_wake_foam: ptr<function, array<f32, 1>>) -> coax_t39 {
  var l_i: coax_t19 = p_i;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_part: u32;
  var l_world: vec3f;
  var l_rest: vec2f;
  var l_flow: vec2f;
  var l_depth: f32;
  var l_v: vec3f;
  var l_eye_distance: f32;
  var l_footprint: f32;
  var l_swell: vec3f;
  var l_detail: vec3f;
  var l_slope: vec2f;
  var l_n: vec3f;
  var l_roughness: f32;
  var l_none: coax_t39;
  var l_o: coax_t39;
  var l_nov: f32;
  var l_fresnel: f32;
  var l_sky: vec3f;
  var l_l: vec3f;
  var l_light: vec3f;
  var l_glint: vec3f;
  var l_crest: vec3f;
  var l_noise: f32;
  var l_foam: f32;
  var l_foam_rgb: vec3f;
  var l_ci: f32;
  var l_through: vec3f;
  var l_body: vec3f;
  var l_clear: vec3f;
  var l_edge: f32;
  var l_cover: f32;
  l_part = coax_p_wg_u32_f((l_i.f4.x + 0x1.000000p-1f));
  if ((l_part == 7u))
  {
    return coax_p_eg_weir_shade(l_i, l_paint_leaf_far);
  }
  if ((l_part == 8u))
  {
    return coax_p_eg_fx_shade(l_i, l_paint_leaf_far);
  }
  if ((!coax_p_eg_water_surface_part(l_part)))
  {
    return coax_p_eg_fountain_fall_shade(l_i, l_part, l_paint_leaf_far);
  }
  l_world = coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z);
  l_rest = coax_p_v2(l_i.z.x, l_i.z.y);
  l_flow = coax_p_v2(l_i.z.z, l_i.z.w);
  l_depth = l_i.y.w;
  l_v = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world));
  l_eye_distance = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world));
  l_footprint = coax_p_eg_water_footprint(l_v, l_eye_distance);
  if ((l_part == 6u))
  {
    return coax_p_eg_pond_shade(l_i, l_v, l_eye_distance, l_footprint, l_paint_leaf_far, p_wake_foam);
  }
  if ((l_part >= 1u))
  {
    return coax_p_eg_fountain_pool_shade(l_i, l_part, l_v, l_footprint, l_paint_leaf_far, p_wake_foam);
  }
  l_swell = coax_p_eg_water_gerstner_slope(l_rest, l_depth, 0x1.000000p0f);
  l_detail = coax_p_eg_water_flow_detail(l_rest, l_flow, l_footprint);
  l_slope = coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(coax_p_v2(l_swell.x, l_swell.y), coax_p_v2(l_detail.x, l_detail.y)), coax_p_eg_water_ripples(l_rest, p_wake_foam)), coax_p_eg_weather_rain_slope(coax_p_v2(l_world.x, l_world.z)));
  l_n = coax_p_wg_normalize3(coax_p_v3((-l_slope.x), (0x1.000000p0f - l_swell.z), (-l_slope.y)));
  l_roughness = coax_p_clampf(sqrt(((coax_at_u(i32(1060i)) * coax_at_u(i32(1060i))) + l_detail.z)), 0x1.47ae14p-6f, 0x1.333334p-1f);
  if ((coax_at_u(i32(25i)) < l_world.y))
  {
    if ((coax_at_u(i32(1062i)) > 0x1.000000p-1f))
    {
      return coax_p_eg_water_below(l_n, l_v, l_eye_distance, coax_p_wg_length2(l_slope));
    }
    l_none = coax_t39();
    l_none.x = coax_p_v3(0.0f, 0.0f, 0.0f);
    l_none.y = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
    return l_none;
  }
  if ((coax_at_u(i32(1055i)) > 0x1.000000p-1f))
  {
    return coax_p_eg_water_painted(l_world, l_rest, l_flow, l_depth, l_i.w.w, l_i.w.x, l_n, l_v, l_footprint, l_paint_leaf_far, p_wake_foam);
  }
  l_o = coax_t39();
  l_nov = coax_p_fmax(coax_p_wg_dot3(l_n, l_v), 0x1.0624dep-10f);
  l_fresnel = coax_p_eg_water_fresnel(l_nov);
  l_sky = coax_p_eg_water_host_sky(coax_p_wg_reflect3(coax_p_wg_neg_f3(l_v), l_n), coax_p_eg_water_sky_roughness(l_roughness, l_footprint));
  l_l = coax_p_wg_sw_xyz_f4(coax_p_eg_water_host_sun());
  l_light = coax_p_wg_mul_f3f(coax_p_eg_water_host_sun_light(), coax_p_eg_shade_shadows(coax_p_v2(l_world.x, l_world.z)));
  l_glint = coax_p_wg_mul_ff3(coax_p_eg_water_glint(l_n, l_v, l_l, l_roughness, coax_at_u(i32(1066i)), l_eye_distance), l_light);
  l_crest = coax_p_wg_mul_f3f3(coax_p_eg_water_crest_scatter(l_v, l_l, l_n, l_i.f4.y, l_i.w.z), l_light);
  l_noise = coax_p_eg_water_foam_noise(l_rest, l_flow);
  l_foam = coax_p_eg_water_foam(l_rest, l_i.w.w, l_i.w.z, l_i.w.x, l_noise, p_wake_foam);
  l_foam_rgb = coax_p_wg_mul_f3f3(coax_p_v3(0x1.ccccccp-1f, 0x1.e147aep-1f, 0x1.e66666p-1f), coax_p_eg_water_host_light(l_world, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_paint_leaf_far));
  l_ci = coax_p_clampf(l_v.y, 0.0f, 0x1.000000p0f);
  l_through = coax_p_eg_water_transmittance(l_depth, l_ci);
  l_body = coax_p_wg_mul_f3f3(coax_p_eg_water_inscatter(), coax_p_wg_sub_f3f3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_through));
  l_clear = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_sky, l_fresnel), l_glint), l_crest), coax_p_wg_mul_f3f(l_body, (0x1.000000p0f - l_fresnel)));
  l_edge = coax_p_smoothstep((-0x1.99999ap-5f), 0x1.eb851ep-4f, l_depth);
  l_cover = (l_foam * coax_p_smoothstep((-0x1.47ae14p-4f), 0.0f, l_depth));
  l_o.x = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_clear, l_edge), (0x1.000000p0f - l_cover)), coax_p_wg_mul_f3f(l_foam_rgb, l_cover));
  l_o.y = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_wg_mul_f3f(l_through, (0x1.000000p0f - l_fresnel)), l_edge), (0x1.000000p0f - l_cover));
  l_o.z = coax_p_v2(l_n.x, l_n.z);
  l_o.w = coax_p_v4(l_sky.x, l_sky.y, l_sky.z, ((l_fresnel * l_edge) * (0x1.000000p0f - l_cover)));
  return l_o;
}
fn coax_p_eg_water_hull(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_k: u32;
  var l_h: vec4f;
  var l_d: vec2f;
  var l_ax: f32;
  var l_az: f32;
  {
    l_k = 0u;
    while ((l_k < 3u))
    {
      l_h = coax_p_wg_at4(((i32((2u + l_k)) * 4i) + 1500i));
      if ((l_h.w < 0x1.000000p-1f))
      {
        {
          l_k = (l_k + 1u);
          continue;
        }
      }
      l_d = coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_h.x, l_h.y));
      l_ax = sin(l_h.z);
      l_az = cos(l_h.z);
      if (((abs(((l_d.x * l_az) - (l_d.y * l_ax))) < 0x1.000000p-1f) && (abs(((l_d.x * l_ax) + (l_d.y * l_az))) < 0x1.8a3d70p0f)))
      {
        return 0x1.000000p0f;
      }
      l_k = (l_k + 1u);
    }
  }
  return 0.0f;
}
fn coax_p_eg_water_dry(p_p: vec2f) -> bool {
  var l_p: vec2f = p_p;
  var l_w: f32;
  var l_a: vec2f;
  var l_e: vec2f;
  var l_t: f32;
  var l_r: f32;
  l_w = coax_at_u(i32(2100i));
  if ((l_w > 0.0f))
  {
    l_a = coax_p_v2(coax_at_u(i32(2096i)), coax_at_u(i32(2097i)));
    l_e = coax_p_wg_sub_f2f2(coax_p_v2(coax_at_u(i32(2098i)), coax_at_u(i32(2099i))), l_a);
    l_t = (coax_p_wg_dot2(coax_p_wg_sub_f2f2(l_p, l_a), l_e) / coax_p_fmax(coax_p_wg_dot2(l_e, l_e), 0x1.a36e2ep-14f));
    if ((((l_t > 0.0f) && (l_t < 0x1.000000p0f)) && (coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, l_a), coax_p_wg_mul_f2f(l_e, l_t))) < l_w)))
    {
      return true;
    }
  }
  l_r = coax_at_u(i32(2103i));
  return ((l_r > 0.0f) && (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(coax_at_u(i32(2101i)), coax_at_u(i32(2102i))))) < l_r));
}
fn coax_p_eg_water_scene_at(p_px: coax_t38) -> vec3f {
  var l_px: coax_t38 = p_px;
  var l_size: coax_t38;
  l_size = coax_p_wg_cvt_i2u2(coax_p_wg_dims_2d_1());
  return coax_p_wg_sw_rgb_f4(coax_p_wg_load_2d_2(coax_p_wg_clamp_i2i2i2(l_px, coax_p_i2(0i, 0i), coax_p_wg_sub_i2i2(l_size, coax_p_i2(1i, 1i))), 0i));
}
fn coax_p_eg_water_thickness(p_px: coax_t38, p_world: vec3f) -> f32 {
  var l_px: coax_t38 = p_px;
  var l_world: vec3f = p_world;
  var l_w: f32;
  l_w = coax_p_wg_dot4(coax_p_wg_at4(20i), coax_p_v4(l_world.x, l_world.y, l_world.z, 0x1.000000p0f));
  return ((coax_p_fmax((coax_p_eg_water_scene_w(l_px) - l_w), 0.0f) * coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world))) / coax_p_fmax(l_w, 0x1.0624dep-10f));
}
fn coax_p_wg_dims_2d_1() -> coax_t29 {
  return coax_p_u2(textureDimensions(water_scene).x, textureDimensions(water_scene).y);
}
fn coax_p_wg_round_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_wg_round(l_p0.x), coax_p_wg_round(l_p0.y));
}
fn coax_p_wg_add_i2i2(p_a: coax_t38, p_b: coax_t38) -> coax_t38 {
  var l_a: coax_t38 = p_a;
  var l_b: coax_t38 = p_b;
  return coax_p_i2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_eg_water_scene_w(p_px: coax_t38) -> f32 {
  var l_px: coax_t38 = p_px;
  var l_size: coax_t38;
  l_size = coax_p_wg_cvt_i2u2(coax_p_wg_dims_depth_1());
  return coax_p_eg_water_view_w(coax_p_wg_load_depth(coax_p_wg_clamp_i2i2i2(l_px, coax_p_i2(0i, 0i), coax_p_wg_sub_i2i2(l_size, coax_p_i2(1i, 1i))), 0i));
}
fn coax_p_wg_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_reflect3(l_i, l_n);
}
fn coax_p_eg_water_ssr(p_p: vec3f, p_dir: vec3f) -> vec4f {
  var l_p: vec3f = p_p;
  var l_dir: vec3f = p_dir;
  var l_size: vec2f;
  var l_t: f32;
  var l_last: f32;
  var l_k: i32;
  var l_q: vec3f;
  var l_c: vec4f;
  var l_ndc: vec2f;
  var l_px: coax_t38;
  var l_sw: f32;
  var l_edge: f32;
  var l_sure: f32;
  if ((l_dir.y < 0x1.eb851ep-6f))
  {
    return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  }
  l_size = coax_p_wg_cvt_f2u2(coax_p_wg_dims_2d_1());
  l_t = 0x1.000000p-2f;
  l_last = 0.0f;
  {
    l_k = 0i;
    while ((l_k < 20i))
    {
      l_q = coax_p_wg_add_f3f3(l_p, coax_p_wg_mul_f3f(l_dir, l_t));
      l_c = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), coax_p_v4(l_q.x, l_q.y, l_q.z, 0x1.000000p0f)), coax_p_wg_dot4(coax_p_wg_at4(12i), coax_p_v4(l_q.x, l_q.y, l_q.z, 0x1.000000p0f)), coax_p_wg_dot4(coax_p_wg_at4(16i), coax_p_v4(l_q.x, l_q.y, l_q.z, 0x1.000000p0f)), coax_p_wg_dot4(coax_p_wg_at4(20i), coax_p_v4(l_q.x, l_q.y, l_q.z, 0x1.000000p0f)));
      if ((l_c.w <= 0x1.99999ap-5f))
      {
        return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
      }
      l_ndc = coax_p_wg_div_f2f(coax_p_v2(l_c.x, l_c.y), l_c.w);
      if (((abs(l_ndc.x) > 0x1.000000p0f) || (abs(l_ndc.y) > 0x1.000000p0f)))
      {
        return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
      }
      l_px = coax_p_wg_cvt_i2f2(coax_p_wg_mul_f2f2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f2(l_ndc, coax_p_v2(0x1.000000p-1f, (-0x1.000000p-1f))), coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f)), l_size));
      l_sw = coax_p_eg_water_scene_w(l_px);
      if (((l_sw < l_c.w) && ((l_c.w - l_sw) < (((l_t - l_last) * 0x1.800000p0f) + 0x1.333334p-2f))))
      {
        l_edge = coax_p_fmin((0x1.000000p0f - abs(l_ndc.x)), (0x1.000000p0f - abs(l_ndc.y)));
        l_sure = (((coax_p_smoothstep(0.0f, 0x1.eb851ep-4f, l_edge) * (0x1.000000p0f - coax_p_smoothstep(0x1.c00000p3f, 0x1.400000p4f, f32(l_k)))) * coax_p_smoothstep(0x1.eb851ep-6f, 0x1.eb851ep-4f, l_dir.y)) * (0x1.000000p0f - coax_p_smoothstep(0x1.800000p3f, 0x1.e00000p4f, l_t)));
        return coax_p_wg_make_f4_f3f(coax_p_eg_water_scene_at(l_px), l_sure);
      }
      l_last = l_t;
      l_t = ((l_t * 0x1.51eb86p0f) + 0x1.99999ap-4f);
      l_k = (l_k + 1i);
    }
  }
  return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
}
fn coax_p_wg_make_f3_f2f(p_p0: vec2f, p_p1: f32) -> vec3f {
  var l_p0: vec2f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v3(l_p0.x, l_p0.y, l_p1);
}
fn coax_p_eg_water_noise(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_i: vec3f;
  var l_f: vec3f;
  var l_s: vec3f;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  var l_e: f32;
  var l_g: f32;
  var l_h: f32;
  var l_k: f32;
  l_i = coax_p_wg_floor_f3(l_p);
  l_f = coax_p_wg_fract_f3(l_p);
  l_s = coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f3(l_f, l_f), coax_p_wg_sub_ff3(0x1.800000p1f, coax_p_wg_mul_ff3(0x1.000000p1f, l_f)));
  l_a = coax_p_eg_water_hash(l_i);
  l_b = coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_i, coax_p_v3(0x1.000000p0f, 0.0f, 0.0f)));
  l_c = coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_i, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)));
  l_d = coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_i, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0.0f)));
  l_e = coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_i, coax_p_v3(0.0f, 0.0f, 0x1.000000p0f)));
  l_g = coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_i, coax_p_v3(0x1.000000p0f, 0.0f, 0x1.000000p0f)));
  l_h = coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_i, coax_p_v3(0.0f, 0x1.000000p0f, 0x1.000000p0f)));
  l_k = coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_i, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)));
  return coax_p_mix(coax_p_mix(coax_p_mix(l_a, l_b, l_s.x), coax_p_mix(l_c, l_d, l_s.x), l_s.y), coax_p_mix(coax_p_mix(l_e, l_g, l_s.x), coax_p_mix(l_h, l_k, l_s.x), l_s.y), l_s.z);
}
fn coax_p_eg_water_host_light(p_world: vec3f, p_n: vec3f, p_paint_leaf_far: bool) -> vec3f {
  var l_world: vec3f = p_world;
  var l_n: vec3f = p_n;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  return coax_p_eg_water_host_light_shaded(l_world, l_n, coax_p_eg_shade_shadows(coax_p_v2(l_world.x, l_world.z)), l_paint_leaf_far);
}
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t38 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t38(l_x, l_y);
}
fn coax_p_wg_sub_i2i(p_a: coax_t38, p_b: i32) -> coax_t38 {
  var l_a: coax_t38 = p_a;
  var l_b: i32 = p_b;
  return coax_p_i2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_wg_clamp_i2i2i2(p_p0: coax_t38, p_p1: coax_t38, p_p2: coax_t38) -> coax_t38 {
  var l_p0: coax_t38 = p_p0;
  var l_p1: coax_t38 = p_p1;
  var l_p2: coax_t38 = p_p2;
  return coax_p_i2(coax_p_wg_clamp_i(l_p0.x, l_p1.x, l_p2.x), coax_p_wg_clamp_i(l_p0.y, l_p1.y, l_p2.y));
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
fn coax_p_eg_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_half: vec2f;
  var l_corner: f32;
  var l_q: vec2f;
  var l_d: f32;
  var l_over: bool;
  var l_fill: vec3f;
  var l_line: vec3f;
  var l_w: f32;
  var l_inside: f32;
  var l_edge: f32;
  var l_a: f32;
  if ((l_s.x <= 0.0f))
  {
    return l_under;
  }
  if ((l_s.y > 0x1.400000p1f))
  {
    return coax_p_eg_hud_rect(l_p, l_under, l_r, l_s);
  }
  l_half = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_r.z, l_r.w), coax_p_v2(l_r.x, l_r.y)), 0x1.000000p-1f);
  l_corner = (0x1.800000p2f * l_s.z);
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_d = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  if ((l_d > 0x1.000000p0f))
  {
    return l_under;
  }
  l_over = (l_s.y >= 0x1.000000p1f);
  l_fill = select(coax_p_v3(0x1.c28f5cp-5f, 0x1.333334p-4f, 0x1.eb851ep-5f), coax_p_v3(0x1.70a3d8p-4f, 0x1.eb851ep-4f, 0x1.70a3d8p-4f), l_over);
  l_line = select(coax_p_v3(0x1.3d70a4p-1f, 0x1.0a3d70p-1f, 0x1.333334p-2f), coax_p_v3(0x1.ccccccp-1f, 0x1.851eb8p-1f, 0x1.ccccccp-2f), (l_over || (l_s.y >= 0x1.000000p0f)));
  l_w = (0x1.400000p0f * l_s.z);
  l_inside = coax_p_clampf((0x1.000000p-1f - l_d), 0.0f, 0x1.000000p0f);
  l_edge = coax_p_clampf((((l_w * 0x1.000000p-1f) + 0x1.000000p-1f) - abs((l_d + (l_w * 0x1.000000p-1f)))), 0.0f, 0x1.000000p0f);
  l_a = coax_p_clampf(l_s.x, 0.0f, 0x1.000000p0f);
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_under, l_fill, (l_a * l_inside)), l_line, ((l_edge * coax_p_fmin(0x1.000000p0f, (l_a + 0x1.99999ap-4f))) * 0x1.99999ap-1f));
}
fn coax_p_wg_clamp_i(p_x: i32, p_lo: i32, p_hi: i32) -> i32 {
  var l_x: i32 = p_x;
  var l_lo: i32 = p_lo;
  var l_hi: i32 = p_hi;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_lo), l_hi);
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_wg_snorm8(p_b: u32) -> f32 {
  var l_b: u32 = p_b;
  var l_s: f32;
  l_s = f32(l_b);
  if ((l_b >= 128u))
  {
    l_s = (l_s - 0x1.000000p8f);
  }
  return coax_p_fmax((l_s / 0x1.fc0000p6f), (-0x1.000000p0f));
}
fn coax_p_verse_style_canopy_normal(p_mesh_n: f32, p_blob_n: f32, p_k: f32) -> f32 {
  var l_mesh_n: f32 = p_mesh_n;
  var l_blob_n: f32 = p_blob_n;
  var l_k: f32 = p_k;
  return (l_mesh_n + ((l_blob_n - l_mesh_n) * l_k));
}
fn coax_p_u4(p_x: u32, p_y: u32, p_z: u32, p_w: u32) -> coax_t33 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  var l_z: u32 = p_z;
  var l_w: u32 = p_w;
  return coax_t33(l_x, l_y, l_z, l_w);
}
fn coax_p_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return ((((l_a.x * l_b.x) + (l_a.y * l_b.y)) + (l_a.z * l_b.z)) + (l_a.w * l_b.w));
}
fn coax_p_eg_lathe_point(p_profile: u32, p_j: u32) -> vec2f {
  var l_profile: u32 = p_profile;
  var l_j: u32 = p_j;
  var l_basin: array<vec2f, 22>;
  var l_column: array<vec2f, 29>;
  l_basin = array<vec2f, 22>(coax_p_v2(0.0f, 0x1.99999ap-5f), coax_p_v2(0x1.333334p0f, 0x1.99999ap-4f), coax_p_v2(0x1.000000p1f, 0x1.000000p-2f), coax_p_v2(0x1.333334p1f, 0x1.851eb8p-2f), coax_p_v2(0x1.451eb8p1f, 0x1.d70a3ep-2f), coax_p_v2(0x1.4cccccp1f, 0x1.1eb852p-1f), coax_p_v2(0x1.4f5c28p1f, 0x1.666666p-1f), coax_p_v2(0x1.547ae2p1f, 0x1.99999ap-1f), coax_p_v2(0x1.5eb852p1f, 0x1.b851ecp-1f), coax_p_v2(0x1.6e147ap1f, 0x1.c7ae14p-1f), coax_p_v2(0x1.7d70a4p1f, 0x1.c28f5cp-1f), coax_p_v2(0x1.87ae14p1f, 0x1.ae147ap-1f), coax_p_v2(0x1.8cccccp1f, 0x1.8a3d70p-1f), coax_p_v2(0x1.8a3d70p1f, 0x1.6147aep-1f), coax_p_v2(0x1.800000p1f, 0x1.4cccccp-1f), coax_p_v2(0x1.7c28f6p1f, 0x1.333334p-1f), coax_p_v2(0x1.7c28f6p1f, 0x1.333334p-2f), coax_p_v2(0x1.828f5cp1f, 0x1.000000p-2f), coax_p_v2(0x1.99999ap1f, 0x1.d70a3ep-3f), coax_p_v2(0x1.a3d70ap1f, 0x1.851eb8p-3f), coax_p_v2(0x1.a7ae14p1f, 0x1.eb851ep-4f), coax_p_v2(0x1.a8f5c2p1f, (-0x1.333334p-2f)));
  l_column = array<vec2f, 29>(coax_p_v2(0.0f, 0x1.4f5c28p1f), coax_p_v2(0x1.70a3d8p-4f, 0x1.4cccccp1f), coax_p_v2(0x1.5c28f6p-3f, 0x1.43d70ap1f), coax_p_v2(0x1.ae147ap-3f, 0x1.3851ecp1f), coax_p_v2(0x1.5c28f6p-3f, 0x1.2e147ap1f), coax_p_v2(0x1.0a3d70p-3f, 0x1.1c28f6p1f), coax_p_v2(0x1.333334p-3f, 0x1.0cccccp1f), coax_p_v2(0x1.eb851ep-3f, 0x1.eb851ep0f), coax_p_v2(0x1.666666p-1f, 0x1.e66666p0f), coax_p_v2(0x1.0cccccp0f, 0x1.ee147ap0f), coax_p_v2(0x1.28f5c2p0f, 0x1.028f5cp1f), coax_p_v2(0x1.333334p0f, 0x1.0f5c28p1f), coax_p_v2(0x1.47ae14p0f, 0x1.147ae2p1f), coax_p_v2(0x1.5c28f6p0f, 0x1.10a3d8p1f), coax_p_v2(0x1.666666p0f, 0x1.07ae14p1f), coax_p_v2(0x1.5c28f6p0f, 0x1.fd70a4p0f), coax_p_v2(0x1.1eb852p0f, 0x1.dc28f6p0f), coax_p_v2(0x1.99999ap-1f, 0x1.bd70a4p0f), coax_p_v2(0x1.000000p-1f, 0x1.9eb852p0f), coax_p_v2(0x1.99999ap-2f, 0x1.8a3d70p0f), coax_p_v2(0x1.ae147ap-2f, 0x1.75c290p0f), coax_p_v2(0x1.51eb86p-2f, 0x1.6147aep0f), coax_p_v2(0x1.28f5c2p-2f, 0x1.4cccccp0f), coax_p_v2(0x1.3d70a4p-2f, 0x1.19999ap-1f), coax_p_v2(0x1.99999ap-2f, 0x1.c28f5cp-2f), coax_p_v2(0x1.d70a3ep-2f, 0x1.333334p-2f), coax_p_v2(0x1.333334p-1f, 0x1.c28f5cp-3f), coax_p_v2(0x1.3d70a4p-1f, 0x1.1eb852p-3f), coax_p_v2(0x1.3d70a4p-1f, (-0x1.99999ap-4f)));
  if ((l_profile == 0u))
  {
    return l_basin[i32(coax_p_wg_min_u(l_j, 21u))];
  }
  return l_column[i32(coax_p_wg_min_u(l_j, 28u))];
}
fn coax_p_eg_lathe_segment_n(p_profile: u32, p_k: u32) -> vec2f {
  var l_profile: u32 = p_profile;
  var l_k: u32 = p_k;
  var l_t: vec2f;
  l_t = coax_p_wg_sub_f2f2(coax_p_eg_lathe_point(l_profile, (l_k + 1u)), coax_p_eg_lathe_point(l_profile, l_k));
  if ((coax_p_wg_dot2(l_t, l_t) < 0x1.5798eep-27f))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  return coax_p_wg_normalize2(coax_p_v2((-l_t.y), l_t.x));
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
fn coax_p_EG_WIND_DIR() -> vec2f {
  return coax_p_v2(0x1.99999ap-1f, 0x1.333334p-1f);
}
fn coax_p_EG_WIND() -> vec4f {
  return coax_p_v4(0x1.333334p-2f, 0x1.800000p3f, 0x1.800000p1f, (0x1.000000p0f / 0x1.900000p4f));
}
fn coax_p_verse_style_canopy_sway(p_theta: f32, p_amp: f32, p_reach: f32, p_share: f32) -> f32 {
  var l_theta: f32 = p_theta;
  var l_amp: f32 = p_amp;
  var l_reach: f32 = p_reach;
  var l_share: f32 = p_share;
  return ((l_reach * coax_p_st_sat(l_share)) * coax_p_st_max((-0x1.000000p0f), coax_p_st_min(0x1.000000p0f, (l_theta / coax_p_st_max(l_amp, 0x1.a36e2ep-14f)))));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_EG_F_JET() -> vec3f {
  return coax_p_v3(0x1.eb851ep-6f, 0x1.4cccccp1f, 0x1.1eb852p-1f);
}
fn coax_p_EG_F_JET_V() -> vec2f {
  return coax_p_v2(0x1.000000p-1f, 0x1.ccccccp0f);
}
fn coax_p_EG_F_LIP() -> vec2f {
  return coax_p_v2(0x1.6147aep0f, 0x1.10a3d8p1f);
}
fn coax_p_wg_tanh(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_e: f32;
  if ((l_x > 0x1.400000p3f))
  {
    return 0x1.000000p0f;
  }
  if ((l_x < (-0x1.400000p3f)))
  {
    return (-0x1.000000p0f);
  }
  l_e = exp((0x1.000000p1f * l_x));
  return ((l_e - 0x1.000000p0f) / (l_e + 0x1.000000p0f));
}
fn coax_p_verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_v: f32;
  l_v = (sin(((l_x * 0x1.fc6666p6f) + (l_y * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
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
fn coax_p_wg_transpose(p_m: coax_t40) -> coax_t40 {
  var l_m: coax_t40 = p_m;
  return coax_t40(coax_p_v4(l_m.x.x, l_m.y.x, l_m.z.x, l_m.w.x), coax_p_v4(l_m.x.y, l_m.y.y, l_m.z.y, l_m.w.y), coax_p_v4(l_m.x.z, l_m.y.z, l_m.z.z, l_m.w.z), coax_p_v4(l_m.x.w, l_m.y.w, l_m.z.w, l_m.w.w));
}
fn coax_p_scale4(p_a: vec4f, p_k: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_k: f32 = p_k;
  return vec4f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k), (l_a.w * l_k));
}
fn coax_p_add4(p_a: vec4f, p_b: vec4f) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return vec4f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
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
fn coax_p_EG_RAIN_BRIDGE() -> vec4f {
  return coax_p_v4(0x1.500000p6f, (-0x1.200000p4f), 0x1.700000p6f, (-0x1.400000p2f));
}
fn coax_p_wg_all_ge_f2f2(p_a: vec2f, p_b: vec2f) -> bool {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x >= l_b.x) && (l_a.y >= l_b.y));
}
fn coax_p_wg_sw_zw_f4(p_p: vec4f) -> vec2f {
  var l_p: vec4f = p_p;
  return coax_p_v2(l_p.z, l_p.w);
}
fn coax_p_wg_all_le_f2f2(p_a: vec2f, p_b: vec2f) -> bool {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x <= l_b.x) && (l_a.y <= l_b.y));
}
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
}
fn coax_p_wg_floor_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(floor(l_p0.x), floor(l_p0.y), floor(l_p0.z));
}
fn coax_p_eg_water_mote_hash(p_c: vec3f) -> vec4f {
  var l_c: vec3f = p_c;
  var l_h: vec4f;
  l_h = coax_p_v4(coax_p_eg_water_hash(l_c), coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_c, coax_p_v3(0x1.100000p4f, 0x1.800000p1f, 0x1.400000p2f))), coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_c, coax_p_v3(0x1.c00000p2f, 0x1.d00000p4f, 0x1.600000p3f))), coax_p_eg_water_hash(coax_p_wg_add_f3f3(l_c, coax_p_v3(0x1.000000p1f, 0x1.a00000p3f, 0x1.f00000p4f))));
  return l_h;
}
fn coax_p_wg_sw_xz_f4(p_p: vec4f) -> vec2f {
  var l_p: vec4f = p_p;
  return coax_p_v2(l_p.x, l_p.z);
}
fn coax_p_eg_water_caustic_pattern(p_p: vec2f, p_t: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_t: f32 = p_t;
  var l_a: f32;
  var l_b: f32;
  var l_ra: f32;
  var l_rb: f32;
  l_a = coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(l_p, 0x1.99999ap0f), (l_t * 0x1.ccccccp-2f)));
  l_b = coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.733334p1f), coax_p_v2(0x1.8cccccp1f, 0x1.ecccccp2f)), ((l_t * 0x1.333334p-1f) + 0x1.000000p2f)));
  l_ra = (0x1.000000p0f - abs(((0x1.000000p1f * l_a) - 0x1.000000p0f)));
  l_rb = (0x1.000000p0f - abs(((0x1.000000p1f * l_b) - 0x1.000000p0f)));
  return ((coax_pow_bg_vs(l_ra, 0x1.c00000p2f) * 0x1.99999ap-1f) + (coax_pow_bg_vs(l_rb, 0x1.c00000p2f) * 0x1.19999ap-1f));
}
fn coax_p_wg_cvt_i3f3(p_p: vec3f) -> coax_t45 {
  var l_p: vec3f = p_p;
  return coax_p_i3(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y), coax_p_wg_i32_f(l_p.z));
}
fn coax_p_wg_bits_u3i3(p_p: coax_t45) -> coax_t42 {
  var l_p: coax_t45 = p_p;
  return coax_p_u3(u32(l_p.x), u32(l_p.y), u32(l_p.z));
}
fn coax_p_wg_mul_u3u(p_a: coax_t42, p_b: u32) -> coax_t42 {
  var l_a: coax_t42 = p_a;
  var l_b: u32 = p_b;
  return coax_p_u3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_wg_add_u3u(p_a: coax_t42, p_b: u32) -> coax_t42 {
  var l_a: coax_t42 = p_a;
  var l_b: u32 = p_b;
  return coax_p_u3((l_a.x + l_b), (l_a.y + l_b), (l_a.z + l_b));
}
fn coax_p_u3(p_x: u32, p_y: u32, p_z: u32) -> coax_t42 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  var l_z: u32 = p_z;
  return coax_t42(l_x, l_y, l_z);
}
fn coax_p_wg_shr_u3u3(p_a: coax_t42, p_b: coax_t42) -> coax_t42 {
  var l_a: coax_t42 = p_a;
  var l_b: coax_t42 = p_b;
  return coax_p_u3((l_a.x >> u32(l_b.x)), (l_a.y >> u32(l_b.y)), (l_a.z >> u32(l_b.z)));
}
fn coax_p_wg_xor_u3u3(p_a: coax_t42, p_b: coax_t42) -> coax_t42 {
  var l_a: coax_t42 = p_a;
  var l_b: coax_t42 = p_b;
  return coax_p_u3((l_a.x ^ l_b.x), (l_a.y ^ l_b.y), (l_a.z ^ l_b.z));
}
fn coax_p_eg_fountain_depth_at(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_r: f32;
  var l_y: f32;
  if ((!coax_p_eg_fountain_on()))
  {
    return (-0x1.000000p0f);
  }
  l_r = coax_p_wg_length2(coax_p_v2(l_p.x, l_p.z));
  l_y = (l_p.y - coax_p_eg_fountain_base());
  if (((l_r < 0x1.51eb86p1f) && (l_y < (0x1.3d70a4p-1f + 0x1.47ae14p-4f))))
  {
    return (0x1.3d70a4p-1f - l_y);
  }
  if ((((l_r < 0x1.30a3d8p0f) && (l_y > 0x1.d9999ap0f)) && (l_y < (0x1.0b851ep1f + 0x1.47ae14p-4f))))
  {
    return (0x1.0b851ep1f - l_y);
  }
  return (-0x1.000000p0f);
}
fn coax_p_eg_weather_wet_share(p_n: vec3f, p_character: bool) -> f32 {
  var l_n: vec3f = p_n;
  var l_character: bool = p_character;
  return (coax_at_u(i32(1476i)) * select(coax_p_smoothstep((-0x1.333334p-2f), 0x1.666666p-1f, l_n.y), 0x1.99999ap-1f, l_character));
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
fn coax_p_eg_weather_rain_slope(p_p: vec2f) -> vec2f {
  var l_p: vec2f = p_p;
  var l_rain: f32;
  var l_cell: f32;
  var l_base: vec2f;
  var l_t: f32;
  var l_g: vec2f;
  var l_j: i32;
  var l_i: i32;
  var l_c: vec2f;
  var l_h: f32;
  var l_clock: f32;
  var l_phase: f32;
  var l_turn: f32;
  var l_at: vec2f;
  var l_to: vec2f;
  var l_dist: f32;
  var l_x: f32;
  var l_env: f32;
  l_rain = coax_at_u(i32(1472i));
  if ((l_rain <= 0x1.47ae14p-7f))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_cell = 0x1.19999ap-1f;
  l_base = coax_p_wg_floor_f2(coax_p_wg_div_f2f(l_p, l_cell));
  l_t = coax_at_u(i32(1048i));
  l_g = coax_p_v2(0.0f, 0.0f);
  {
    l_j = (-1i);
    while ((l_j <= 1i))
    {
      {
        l_i = (-1i);
        while ((l_i <= 1i))
        {
          l_c = coax_p_wg_add_f2f2(l_base, coax_p_v2(f32(l_i), f32(l_j)));
          l_h = coax_p_eg_weather_hash(l_c);
          if ((l_h > (0x1.000000p-2f + (0x1.800000p-1f * l_rain))))
          {
            {
              l_i = (l_i + 1i);
              continue;
            }
          }
          l_clock = ((l_t / 0x1.666666p-1f) + (l_h * 0x1.c851ecp2f));
          l_phase = coax_p_fract(l_clock);
          l_turn = floor(l_clock);
          l_at = coax_p_wg_mul_f2f(coax_p_wg_add_f2f2(l_c, coax_p_v2(coax_p_eg_weather_hash(coax_p_wg_add_f2f(l_c, (l_turn * 0x1.5eb852p0f))), coax_p_eg_weather_hash(coax_p_wg_add_f2f(coax_p_wg_add_f2f(l_c, 0x1.533334p2f), (l_turn * 0x1.0e147ap1f))))), l_cell);
          l_to = coax_p_wg_sub_f2f2(l_p, l_at);
          l_dist = coax_p_wg_length2(l_to);
          l_x = (l_dist - (l_phase * 0x1.333334p-2f));
          l_env = ((exp((((-l_x) * l_x) / 0x1.a36e2ep-10f)) * (0x1.000000p0f - l_phase)) * (0x1.000000p0f - l_phase));
          l_g = coax_p_wg_add_f2f2(l_g, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(l_to, coax_p_fmax(l_dist, 0x1.0624dep-10f)), l_env), cos((l_x * 0x1.b80000p5f))), 0x1.666666p0f));
          l_i = (l_i + 1i);
        }
      }
      l_j = (l_j + 1i);
    }
  }
  return coax_p_wg_mul_f2f(l_g, l_rain);
}
fn coax_p_eg_weir_shade(p_i: coax_t19, p_paint_leaf_far: bool) -> coax_t39 {
  var l_i: coax_t19 = p_i;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_o: coax_t39;
  var l_world: vec3f;
  var l_n: vec3f;
  var l_v_along: f32;
  var l_a: f32;
  var l_width: f32;
  var l_to_eye: vec3f;
  var l_t: f32;
  var l_light: vec3f;
  var l_facing: f32;
  var l_fresnel: f32;
  var l_sky: vec3f;
  var l_x: f32;
  var l_streak: f32;
  var l_sides: f32;
  var l_torn: f32;
  var l_white: f32;
  var l_alpha: f32;
  var l_lip: f32;
  var l_body: vec3f;
  var l_foam: vec3f;
  var l_c: vec3f;
  l_o = coax_t39();
  l_world = coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z);
  l_n = coax_p_wg_normalize3(coax_p_v3(l_i.z.x, l_i.z.y, l_i.z.z));
  l_v_along = l_i.z.w;
  l_a = l_i.f4.y;
  l_width = l_i.f4.w;
  l_to_eye = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world));
  l_t = coax_at_u(i32(1048i));
  l_light = coax_p_eg_water_host_light(l_world, coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_n, coax_p_v3(0.0f, 0x1.000000p-1f, 0.0f))), l_paint_leaf_far);
  l_facing = abs(coax_p_wg_dot3(l_n, l_to_eye));
  l_fresnel = coax_p_eg_water_fresnel(coax_p_fmax(l_facing, 0x1.99999ap-5f));
  l_sky = coax_p_eg_water_host_sky(coax_p_wg_reflect3(coax_p_wg_neg_f3(l_to_eye), l_n), 0.0f);
  l_x = (l_a * l_width);
  l_streak = coax_p_eg_water_noise(coax_p_v3((l_x * 0x1.c00000p2f), ((l_v_along * 0x1.800000p1f) - (l_t * 0x1.200000p2f)), 0x1.000000p0f));
  l_streak = ((l_streak * 0x1.333334p-1f) + (0x1.99999ap-2f * coax_p_eg_water_noise(coax_p_v3((l_x * 0x1.300000p4f), ((l_v_along * 0x1.000000p3f) - (l_t * 0x1.000000p3f)), 0x1.400000p2f))));
  l_sides = (coax_p_smoothstep(0.0f, 0x1.eb851ep-5f, l_a) * coax_p_smoothstep(0.0f, 0x1.eb851ep-5f, (0x1.000000p0f - l_a)));
  l_torn = coax_p_smoothstep(0x1.333334p-3f, 0x1.ccccccp-2f, ((l_streak + (0x1.000000p-1f * l_sides)) - (0x1.333334p-2f * (0x1.000000p0f - l_sides))));
  l_white = coax_p_clampf(((0x1.333334p-2f + (0x1.333334p-1f * coax_p_smoothstep(0x1.666666p-2f, 0x1.e66666p-1f, l_v_along))) + (0x1.ccccccp-2f * coax_p_smoothstep(0x1.19999ap-1f, 0x1.b33334p-1f, l_streak))), 0.0f, 0x1.000000p0f);
  l_alpha = coax_p_clampf(((((0x1.3d70a4p-1f + (0x1.333334p-2f * l_v_along)) + (0x1.333334p-3f * l_streak)) * l_torn) * coax_p_smoothstep(0.0f, 0x1.47ae14p-4f, (l_v_along + 0x1.99999ap-5f))), 0.0f, 0x1.f0a3d8p-1f);
  l_lip = (exp((((-l_v_along) * l_v_along) / 0x1.0624dep-8f)) * 0x1.000000p-1f);
  l_body = coax_p_wg_mul_f3f3(coax_p_wg_pow_f3f3(coax_p_EG_W_SHALLOW(), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f)), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_light, 0x1.ccccccp-1f), coax_p_v3(0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.99999ap-5f)));
  l_foam = coax_p_wg_mul_f3f3(coax_p_v3(0x1.e66666p-1f, 0x1.f0a3d8p-1f, 0x1.eb851ep-1f), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_light, 0x1.e66666p-1f), coax_p_v3(0x1.47ae14p-4f, 0x1.47ae14p-4f, 0x1.47ae14p-4f)));
  l_c = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_body, l_foam, l_white), (0x1.000000p0f - (l_fresnel * 0x1.000000p-1f))), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_sky, l_fresnel), 0x1.000000p-1f)), coax_p_wg_mul_f3f(l_foam, l_lip));
  l_o.x = coax_p_wg_mul_f3f(l_c, l_alpha);
  l_o.y = coax_p_wg_make_f3_f((0x1.000000p0f - l_alpha));
  return l_o;
}
fn coax_p_eg_fx_shade(p_i: coax_t19, p_paint_leaf_far: bool) -> coax_t39 {
  var l_i: coax_t19 = p_i;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_o: coax_t39;
  var l_kind: u32;
  var l_world: vec3f;
  var l_r: f32;
  var l_light: vec3f;
  var l_near: f32;
  var l_alpha: f32;
  var l_c: vec3f;
  var l_puff: f32;
  var l_w: f32;
  l_o = coax_t39();
  l_kind = coax_p_wg_u32_f((l_i.f4.y + 0x1.000000p-1f));
  l_world = coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z);
  l_r = coax_p_wg_length2(coax_p_v2(l_i.z.x, l_i.z.y));
  l_light = coax_p_eg_water_host_light(l_world, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_paint_leaf_far);
  l_near = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world));
  l_alpha = l_i.f4.z;
  l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
  if (((l_kind == 0u) || (l_kind == 2u)))
  {
    l_alpha = (l_alpha * (0x1.000000p0f - coax_p_smoothstep(0x1.19999ap-1f, 0x1.000000p0f, l_r)));
    l_c = coax_p_wg_mul_f3f3(coax_p_wg_mix_f3f3f(coax_p_wg_pow_f3f3(coax_p_EG_W_SHALLOW(), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f)), coax_p_v3(0x1.e66666p-1f, 0x1.f0a3d8p-1f, 0x1.f5c290p-1f), 0x1.666666p-1f), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_light, 0x1.e66666p-1f), coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-4f, 0x1.99999ap-4f)));
  }
  else
  {
    l_puff = coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_i.z.x, l_i.z.y), 0x1.99999ap0f), coax_p_wg_make_f2_f((l_i.f4.w * 0x1.000000p1f))), (l_i.f4.w * 0x1.800000p1f)));
    l_alpha = (l_alpha * (((0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-3f, 0x1.000000p0f, l_r)) * (0x1.333334p-1f + (0x1.333334p-1f * l_puff))) * coax_p_smoothstep(0x1.333334p0f, 0x1.c00000p1f, l_near)));
    l_c = coax_p_wg_mul_f3f3(coax_p_v3(0x1.d70a3ep-1f, 0x1.e147aep-1f, 0x1.e66666p-1f), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_light, 0x1.99999ap-1f), coax_p_v3(0x1.70a3d8p-3f, 0x1.70a3d8p-3f, 0x1.70a3d8p-3f)));
  }
  l_w = coax_p_fmax(coax_p_wg_dot4(coax_p_wg_at4(20i), coax_p_v4(l_world.x, l_world.y, l_world.z, 0x1.000000p0f)), 0x1.0624dep-10f);
  l_alpha = (l_alpha * coax_p_smoothstep(0.0f, coax_p_fmax(l_i.y.w, 0x1.99999ap-5f), (coax_p_eg_water_scene_w(coax_p_wg_cvt_i2f2(coax_p_v2(l_i.x.x, l_i.x.y))) - l_w)));
  l_alpha = coax_p_clampf(l_alpha, 0.0f, 0x1.000000p0f);
  l_o.x = coax_p_wg_mul_f3f(l_c, l_alpha);
  l_o.y = coax_p_wg_make_f3_f((0x1.000000p0f - l_alpha));
  return l_o;
}
fn coax_p_eg_fountain_fall_shade(p_i: coax_t19, p_part: u32, p_paint_leaf_far: bool) -> coax_t39 {
  var l_i: coax_t19 = p_i;
  var l_part: u32 = p_part;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_o: coax_t39;
  var l_world: vec3f;
  var l_n: vec3f;
  var l_v_along: f32;
  var l_around: f32;
  var l_k: f32;
  var l_to_eye: vec3f;
  var l_t: f32;
  var l_light: vec3f;
  var l_facing: f32;
  var l_fresnel: f32;
  var l_sky: vec3f;
  var l_streak: f32;
  var l_alpha: f32;
  var l_white: f32;
  var l_froth: f32;
  var l_thick: f32;
  var l_body: vec3f;
  var l_foam: vec3f;
  var l_c: vec3f;
  l_o = coax_t39();
  l_world = coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z);
  l_n = coax_p_wg_normalize3(coax_p_v3(l_i.z.x, l_i.z.y, l_i.z.z));
  l_v_along = l_i.z.w;
  l_around = l_i.f4.y;
  l_k = l_i.f4.w;
  l_to_eye = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world));
  l_t = coax_at_u(i32(1048i));
  l_light = coax_p_eg_water_host_light(l_world, coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_n, coax_p_v3(0.0f, 0x1.333334p-1f, 0.0f))), l_paint_leaf_far);
  l_facing = abs(coax_p_wg_dot3(l_n, l_to_eye));
  l_fresnel = coax_p_eg_water_fresnel(coax_p_fmax(l_facing, 0x1.99999ap-5f));
  l_sky = coax_p_eg_water_host_sky(coax_p_wg_reflect3(coax_p_wg_neg_f3(l_to_eye), l_n), 0.0f);
  l_streak = coax_p_eg_water_noise(coax_p_v3(((l_around * 0x1.200000p3f) + (l_k * 0x1.8cccccp1f)), ((l_v_along * 0x1.c00000p2f) - (l_t * 0x1.600000p2f)), l_k));
  l_streak = ((l_streak * 0x1.333334p-1f) + (0x1.99999ap-2f * coax_p_eg_water_noise(coax_p_v3(((l_around * 0x1.700000p4f) + l_k), ((l_v_along * 0x1.000000p4f) - (l_t * 0x1.200000p3f)), (l_k + 0x1.000000p2f)))));
  l_alpha = 0.0f;
  l_white = 0.0f;
  if ((l_part == 5u))
  {
    l_froth = coax_p_eg_water_noise(coax_p_v3(((l_around * 0x1.c00000p3f) + (l_k * 0x1.000000p1f)), ((l_v_along * 0x1.800000p1f) - (l_t * 0x1.000000p2f)), ((l_t * 0x1.000000p1f) + l_k)));
    l_alpha = ((coax_p_smoothstep(0x1.666666p-2f, 0x1.666666p-1f, (l_froth + (0x1.666666p-2f * (0x1.000000p0f - l_v_along)))) * (0x1.000000p0f - coax_p_smoothstep(0x1.19999ap-1f, 0x1.000000p0f, l_v_along))) * 0x1.ccccccp-1f);
    l_white = 0x1.b33334p-1f;
  }
  else
  {
    l_thick = coax_p_mix(0x1.19999ap-1f, 0x1.d70a3ep-1f, (0x1.000000p0f - l_facing));
    l_alpha = ((l_thick * (0x1.70a3d8p-1f + (0x1.99999ap-2f * l_streak))) * (0x1.000000p0f - ((0x1.ccccccp-2f * coax_p_smoothstep(0x1.333334p-1f, 0x1.000000p0f, l_v_along)) * (0x1.000000p0f - l_streak))));
    l_white = coax_p_clampf(((0x1.000000p-2f + (0x1.19999ap-1f * coax_p_smoothstep(0x1.ccccccp-2f, 0x1.b33334p-1f, l_streak))) + (0x1.333334p-2f * l_v_along)), 0.0f, 0x1.000000p0f);
    if ((l_part == 4u))
    {
      l_white = (0x1.19999ap-1f + (0x1.666666p-2f * l_streak));
    }
  }
  l_alpha = coax_p_clampf(l_alpha, 0.0f, 0x1.000000p0f);
  l_body = coax_p_wg_mul_f3f3(coax_p_wg_pow_f3f3(coax_p_EG_W_SHALLOW(), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f)), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_light, 0x1.ccccccp-1f), coax_p_v3(0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.99999ap-5f)));
  l_foam = coax_p_wg_mul_f3f3(coax_p_v3(0x1.e66666p-1f, 0x1.f0a3d8p-1f, 0x1.eb851ep-1f), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_light, 0x1.e66666p-1f), coax_p_v3(0x1.47ae14p-4f, 0x1.47ae14p-4f, 0x1.47ae14p-4f)));
  l_c = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_body, l_foam, l_white), (0x1.000000p0f - (l_fresnel * 0x1.000000p-1f))), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_sky, l_fresnel), 0x1.000000p-1f));
  l_o.x = coax_p_wg_mul_f3f(l_c, l_alpha);
  l_o.y = coax_p_wg_make_f3_f((0x1.000000p0f - l_alpha));
  return l_o;
}
fn coax_p_eg_water_footprint(p_v: vec3f, p_distance: f32) -> f32 {
  var l_v: vec3f = p_v;
  var l_distance: f32 = p_distance;
  var l_size: f32;
  l_size = coax_p_fmax((l_distance * coax_at_u(i32(1065i))), 0x1.a36e2ep-14f);
  return (l_size * sqrt((0x1.000000p0f / coax_p_fmax(abs(l_v.y), 0x1.47ae14p-4f))));
}
fn coax_p_eg_pond_shade(p_i: coax_t19, p_v: vec3f, p_eye_distance: f32, p_footprint: f32, p_paint_leaf_far: bool, p_wake_foam: ptr<function, array<f32, 1>>) -> coax_t39 {
  var l_i: coax_t19 = p_i;
  var l_v: vec3f = p_v;
  var l_eye_distance: f32 = p_eye_distance;
  var l_footprint: f32 = p_footprint;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_world: vec3f;
  var l_rest: vec2f;
  var l_detail: vec3f;
  var l_slope: vec2f;
  var l_n: vec3f;
  l_world = coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z);
  l_rest = coax_p_v2(l_i.z.x, l_i.z.y);
  l_detail = coax_p_eg_water_flow_detail(l_rest, coax_p_v2(0.0f, 0.0f), l_footprint);
  l_slope = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_detail.x, l_detail.y), 0x1.000000p-1f), coax_p_eg_water_ripples(l_rest, p_wake_foam));
  l_n = coax_p_wg_normalize3(coax_p_v3((-l_slope.x), 0x1.000000p0f, (-l_slope.y)));
  if ((coax_at_u(i32(25i)) < l_world.y))
  {
    return coax_p_eg_water_below(l_n, l_v, l_eye_distance, coax_p_wg_length2(l_slope));
  }
  return coax_p_eg_water_painted(l_world, l_rest, coax_p_v2(0.0f, 0.0f), l_i.y.w, l_i.w.w, 0.0f, l_n, l_v, l_footprint, l_paint_leaf_far, p_wake_foam);
}
fn coax_p_eg_fountain_pool_shade(p_i: coax_t19, p_part: u32, p_v: vec3f, p_footprint: f32, p_paint_leaf_far: bool, p_wake_foam: ptr<function, array<f32, 1>>) -> coax_t39 {
  var l_i: coax_t19 = p_i;
  var l_part: u32 = p_part;
  var l_v: vec3f = p_v;
  var l_footprint: f32 = p_footprint;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_world: vec3f;
  var l_rest: vec2f;
  var l_flow: vec2f;
  var l_r: f32;
  var l_land: f32;
  var l_t: f32;
  var l_detail: vec3f;
  var l_off: f32;
  var l_d: f32;
  var l_k: f32;
  var l_a: f32;
  var l_near: f32;
  var l_amp: f32;
  var l_ring: vec2f;
  var l_slope: vec2f;
  var l_n: vec3f;
  var l_deep: f32;
  l_world = coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z);
  l_rest = coax_p_v2(l_i.z.x, l_i.z.y);
  l_flow = coax_p_v2(l_i.z.z, l_i.z.w);
  l_r = l_i.f4.z;
  l_land = l_i.f4.w;
  l_t = coax_at_u(i32(1048i));
  l_detail = coax_p_eg_water_flow_detail(coax_p_wg_mul_f2f(l_rest, 0x1.99999ap0f), l_flow, l_footprint);
  l_off = (l_r - l_land);
  l_d = abs(l_off);
  l_k = (0x1.921fb6p2f / select(0x1.99999ap-3f, 0x1.eb851ep-4f, (l_part == 2u)));
  l_a = coax_p_wg_atan2(l_rest.y, l_rest.x);
  l_near = select((0x1.ccccccp-2f + (0x1.19999ap-1f * coax_p_eg_fountain_stream_near(l_a))), 0x1.000000p0f, (l_part == 2u));
  l_amp = ((select(0x1.89374cp-8f, 0x1.89374cp-9f, (l_part == 2u)) * l_near) * exp(((-l_d) * 0x1.666666p0f)));
  l_ring = coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(l_rest, coax_p_fmax(l_r, 0x1.0624dep-10f)), coax_p_wg_sign(l_off)), l_amp), l_k), cos(((l_k * l_d) - (l_t * 0x1.200000p3f))));
  l_slope = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_detail.x, l_detail.y), 0x1.666666p-1f), l_ring);
  l_n = coax_p_wg_normalize3(coax_p_v3((-l_slope.x), 0x1.000000p0f, (-l_slope.y)));
  l_deep = select(0x1.333334p0f, 0x1.666666p0f, (l_part == 2u));
  return coax_p_eg_water_painted_teal(l_world, l_rest, l_flow, (l_i.y.w * l_deep), (l_i.w.w * 0x1.000000p1f), l_i.w.x, l_n, l_v, l_footprint, 0x1.b33334p-1f, l_paint_leaf_far, p_wake_foam);
}
fn coax_p_eg_water_gerstner_slope(p_p0: vec2f, p_depth: f32, p_scale: f32) -> vec3f {
  var l_p0: vec2f = p_p0;
  var l_depth: f32 = p_depth;
  var l_scale: f32 = p_scale;
  var l_g: vec3f;
  var l_count: i32;
  var l_gain: f32;
  var l_i: i32;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_k: f32;
  var l_wa: f32;
  var l_theta: f32;
  var l_co: f32;
  l_g = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_count = coax_p_wg_i32_f(coax_at_u(i32(1051i)));
  l_gain = (coax_at_u(i32(1061i)) * l_scale);
  {
    l_i = 0i;
    while ((l_i < 8i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_a = coax_p_wg_at4(((i32((l_i * 2i)) * 4i) + 760i));
      l_c = coax_p_wg_at4(((i32(((l_i * 2i) + 1i)) * 4i) + 760i));
      l_k = l_a.z;
      l_wa = (((l_k * l_a.w) * l_gain) * coax_p_eg_water_shoal(l_k, l_depth));
      l_theta = ((l_k * coax_p_wg_dot2(coax_p_v2(l_a.x, l_a.y), l_p0)) + l_c.y);
      l_co = cos(l_theta);
      l_g.x = (l_g.x + ((l_a.x * l_wa) * l_co));
      l_g.y = (l_g.y + ((l_a.y * l_wa) * l_co));
      l_g.z = (l_g.z + ((l_c.x * l_wa) * sin(l_theta)));
      l_i = (l_i + 1i);
    }
  }
  return l_g;
}
fn coax_p_eg_water_flow_detail(p_p: vec2f, p_flow: vec2f, p_footprint: f32) -> vec3f {
  var l_p: vec2f = p_p;
  var l_flow: vec2f = p_flow;
  var l_footprint: f32 = p_footprint;
  var l_t: f32;
  var l_phase_a: f32;
  var l_phase_b: f32;
  var l_da: vec3f;
  var l_db: vec3f;
  var l_wa: f32;
  if ((coax_p_wg_length2(l_flow) <= 0x1.47ae14p-6f))
  {
    return coax_p_eg_water_detail_waves(l_p, l_footprint, 0x1.000000p0f);
  }
  l_t = coax_at_u(i32(1048i));
  l_phase_a = coax_p_fract((l_t / 0x1.99999ap0f));
  l_phase_b = coax_p_fract(((l_t / 0x1.99999ap0f) + 0x1.000000p-1f));
  l_da = coax_p_eg_water_detail_waves(coax_p_wg_sub_f2f2(l_p, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_flow, l_phase_a), 0x1.99999ap0f)), l_footprint, 0x1.666666p0f);
  l_db = coax_p_eg_water_detail_waves(coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_flow, l_phase_b), 0x1.99999ap0f)), coax_p_v2(0x1.d9999ap1f, 0x1.4cccccp0f)), l_footprint, 0x1.666666p0f);
  l_wa = (0x1.000000p0f - abs(((0x1.000000p1f * l_phase_a) - 0x1.000000p0f)));
  return coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_da, l_wa), coax_p_wg_mul_f3f(l_db, (0x1.000000p0f - l_wa)));
}
fn coax_p_eg_water_ripples(p_p: vec2f, p_wake_foam: ptr<function, array<f32, 1>>) -> vec2f {
  var l_p: vec2f = p_p;
  var l_g: vec2f;
  var l_t: f32;
  var l_count: i32;
  var l_k: f32;
  var l_i: i32;
  var l_r: vec4f;
  var l_age: f32;
  var l_to: vec2f;
  var l_dist: f32;
  var l_x: f32;
  var l_envelope: f32;
  l_g = coax_p_eg_water_wake(l_p, p_wake_foam);
  l_t = coax_at_u(i32(1048i));
  l_count = coax_p_wg_i32_f(coax_at_u(i32(1050i)));
  l_k = (0x1.921fb6p2f / 0x1.ccccccp-2f);
  {
    l_i = 0i;
    while ((l_i < 24i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_r = coax_p_wg_at4(((i32(l_i) * 4i) + 952i));
      l_age = (l_t - l_r.z);
      l_to = coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_r.x, l_r.y));
      l_dist = coax_p_wg_length2(l_to);
      l_x = (l_dist - (0x1.19999ap0f * l_age));
      l_envelope = ((exp((((-l_x) * l_x) / 0x1.70a3d8p-3f)) * exp(((-l_age) * 0x1.19999ap0f))) / (0x1.000000p0f + (0x1.800000p0f * l_dist)));
      l_g = coax_p_wg_add_f2f2(l_g, coax_p_wg_mul_f2f(coax_p_wg_div_f2f(l_to, coax_p_fmax(l_dist, 0x1.0624dep-10f)), (((l_r.w * l_envelope) * l_k) * cos((l_k * l_x)))));
      l_i = (l_i + 1i);
    }
  }
  return l_g;
}
fn coax_p_eg_water_below(p_n: vec3f, p_v: vec3f, p_path: f32, p_slope: f32) -> coax_t39 {
  var l_n: vec3f = p_n;
  var l_v: vec3f = p_v;
  var l_path: f32 = p_path;
  var l_slope: f32 = p_slope;
  var l_o: coax_t39;
  var l_ci: f32;
  var l_st2: f32;
  var l_reflectance: f32;
  var l_fog: vec3f;
  var l_shimmer: f32;
  var l_trans: vec3f;
  l_o = coax_t39();
  l_ci = coax_p_clampf(coax_p_wg_dot3(l_v, coax_p_wg_neg_f3(l_n)), 0.0f, 0x1.000000p0f);
  l_st2 = ((0x1.547ae2p0f * 0x1.547ae2p0f) * (0x1.000000p0f - (l_ci * l_ci)));
  l_reflectance = 0x1.000000p0f;
  if ((l_st2 < 0x1.000000p0f))
  {
    l_reflectance = coax_p_eg_water_fresnel(sqrt((0x1.000000p0f - l_st2)));
  }
  l_fog = coax_p_wg_mul_f3f(coax_p_eg_water_inscatter(), 0x1.333334p1f);
  l_shimmer = (0x1.19999ap0f + (0x1.99999ap0f * coax_p_clampf(l_slope, 0.0f, 0x1.333334p-1f)));
  l_trans = coax_p_wg_exp_f3(coax_p_wg_mul_f3f(coax_p_wg_neg_f3(coax_p_v3(coax_at_u(i32(1052i)), coax_at_u(i32(1053i)), coax_at_u(i32(1054i)))), l_path));
  l_o.x = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_fog, l_shimmer), l_reflectance), l_trans), coax_p_wg_mul_f3f3(l_fog, coax_p_wg_sub_f3f3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_trans)));
  l_o.y = coax_p_wg_mul_f3f(l_trans, (0x1.000000p0f - l_reflectance));
  return l_o;
}
fn coax_p_eg_water_painted(p_world: vec3f, p_rest: vec2f, p_flow: vec2f, p_depth: f32, p_shore: f32, p_white: f32, p_n: vec3f, p_v: vec3f, p_footprint: f32, p_paint_leaf_far: bool, p_wake_foam: ptr<function, array<f32, 1>>) -> coax_t39 {
  var l_world: vec3f = p_world;
  var l_rest: vec2f = p_rest;
  var l_flow: vec2f = p_flow;
  var l_depth: f32 = p_depth;
  var l_shore: f32 = p_shore;
  var l_white: f32 = p_white;
  var l_n: vec3f = p_n;
  var l_v: vec3f = p_v;
  var l_footprint: f32 = p_footprint;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  return coax_p_eg_water_painted_teal(l_world, l_rest, l_flow, l_depth, l_shore, l_white, l_n, l_v, l_footprint, 0.0f, l_paint_leaf_far, p_wake_foam);
}
fn coax_p_eg_water_fresnel(p_c: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_m: f32;
  var l_m2: f32;
  l_m = coax_p_clampf((0x1.000000p0f - l_c), 0.0f, 0x1.000000p0f);
  l_m2 = (l_m * l_m);
  return (0x1.48a6cep-6f + ((((0x1.000000p0f - 0x1.48a6cep-6f) * l_m2) * l_m2) * l_m));
}
fn coax_p_eg_water_sky_roughness(p_roughness: f32, p_footprint: f32) -> f32 {
  var l_roughness: f32 = p_roughness;
  var l_footprint: f32 = p_footprint;
  return coax_p_clampf((sqrt(l_roughness) + (0x1.333334p-3f * coax_p_wg_log2((0x1.000000p0f + (l_footprint * 0x1.000000p2f))))), 0.0f, 0x1.000000p0f);
}
fn coax_p_eg_water_host_sky(p_dir: vec3f, p_level: f32) -> vec3f {
  var l_dir: vec3f = p_dir;
  var l_level: f32 = p_level;
  var l_l: vec3f;
  var l_d: vec3f;
  var l_sharp: vec3f;
  var l_blurred: vec3f;
  l_l = coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u));
  l_d = coax_p_wg_normalize3(coax_p_v3(l_dir.x, coax_p_fmax(l_dir.y, 0.0f), l_dir.z));
  l_sharp = coax_p_wg_mix_f3f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(10u)), coax_p_wg_sw_xyz_f4(coax_p_eg_st(12u)), coax_p_verse_style_sky_t(l_d.y));
  l_blurred = coax_p_wg_mix_f3f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(10u)), coax_p_wg_sw_xyz_f4(coax_p_eg_st(12u)), 0x1.19999ap-1f);
  return coax_p_wg_add_f3f3(coax_p_wg_mix_f3f3f(l_sharp, l_blurred, (l_level * l_level)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(select(coax_p_wg_sw_xyz_f4(coax_p_eg_st(11u)), coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), ((coax_p_eg_st(1u)).w > 0x1.000000p-1f)), coax_p_verse_style_halo(coax_p_wg_dot3(l_d, l_l), (coax_p_eg_st(14u)).z)), 0x1.000000p-2f));
}
fn coax_p_eg_water_host_sun() -> vec4f {
  return coax_p_wg_make_f4_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u)), select(0x1.000000p0f, 0.0f, ((coax_p_eg_st(0u)).w <= 0.0f)));
}
fn coax_p_eg_water_host_sun_light() -> vec3f {
  return coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), (coax_p_eg_st(0u)).w), 0x1.69e83ep1f);
}
fn coax_p_eg_shade_shadows(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_eg_shade_shadows_at(l_p, 0.0f, false);
}
fn coax_p_eg_water_glint(p_n: vec3f, p_v: vec3f, p_l: vec3f, p_roughness: f32, p_disc: f32, p_distance: f32) -> f32 {
  var l_n: vec3f = p_n;
  var l_v: vec3f = p_v;
  var l_l: vec3f = p_l;
  var l_roughness: f32 = p_roughness;
  var l_disc: f32 = p_disc;
  var l_distance: f32 = p_distance;
  var l_nol: f32;
  var l_nov: f32;
  var l_h: vec3f;
  var l_fade_at: f32;
  var l_far: f32;
  var l_a: f32;
  var l_a2: f32;
  var l_spec: f32;
  l_nol = coax_p_wg_dot3(l_n, l_l);
  l_nov = coax_p_fmax(coax_p_wg_dot3(l_n, l_v), 0x1.0624dep-10f);
  if ((l_nol <= 0.0f))
  {
    return 0.0f;
  }
  l_h = coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_l, l_v));
  l_fade_at = coax_p_fmax(coax_at_u(i32(1069i)), 0x1.000000p0f);
  l_far = coax_p_smoothstep((0x1.99999ap-2f * l_fade_at), l_fade_at, l_distance);
  l_a = coax_p_fmin((((l_roughness * l_roughness) + (l_disc * 0x1.000000p-1f)) + (l_far * 0x1.47ae14p-4f)), 0x1.000000p0f);
  l_a2 = (l_a * l_a);
  l_spec = ((coax_p_eg_water_ggx(coax_p_fmax(coax_p_wg_dot3(l_n, l_h), 0.0f), l_a2) * coax_p_eg_water_smith(l_nov, l_nol, l_a2)) * coax_p_eg_water_fresnel(coax_p_fmax(coax_p_wg_dot3(l_v, l_h), 0.0f)));
  return ((l_spec * l_nol) * (0x1.000000p0f - (0x1.800000p-1f * l_far)));
}
fn coax_p_eg_water_crest_scatter(p_v: vec3f, p_l: vec3f, p_n: vec3f, p_height: f32, p_squeeze: f32) -> vec3f {
  var l_v: vec3f = p_v;
  var l_l: vec3f = p_l;
  var l_n: vec3f = p_n;
  var l_height: f32 = p_height;
  var l_squeeze: f32 = p_squeeze;
  var l_strength: f32;
  var l_g: f32;
  var l_cos_theta: f32;
  var l_hg: f32;
  var l_thin: f32;
  var l_side: f32;
  var l_through: vec3f;
  l_strength = coax_at_u(i32(1063i));
  if ((l_strength <= 0.0f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_g = 0x1.333334p-1f;
  l_cos_theta = coax_p_wg_dot3(coax_p_wg_neg_f3(l_l), l_v);
  l_hg = ((0x1.000000p0f - (l_g * l_g)) / ((0x1.000000p2f * 0x1.921fb6p1f) * coax_pow_bg_vs(((0x1.000000p0f + (l_g * l_g)) - ((0x1.000000p1f * l_g) * l_cos_theta)), 0x1.800000p0f)));
  l_thin = (coax_p_clampf((l_height * 0x1.000000p1f), 0.0f, 0x1.000000p0f) * coax_p_clampf(((l_squeeze * 0x1.800000p0f) + 0x1.99999ap-3f), 0.0f, 0x1.000000p0f));
  l_side = coax_p_clampf(((0x1.000000p0f - (l_n.y * 0x1.333334p-1f)) + (coax_p_wg_dot3(l_n, l_v) * 0x1.99999ap-2f)), 0.0f, 0x1.000000p0f);
  l_through = coax_p_wg_exp_f3(coax_p_wg_mul_f3f(coax_p_wg_neg_f3(coax_p_v3(coax_at_u(i32(1052i)), coax_at_u(i32(1053i)), coax_at_u(i32(1054i)))), (0x1.000000p0f - (0x1.333334p-1f * l_thin))));
  return coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(coax_p_v3(coax_at_u(i32(1056i)), coax_at_u(i32(1057i)), coax_at_u(i32(1058i))), l_through), l_hg), l_thin), l_side), l_strength), 0x1.800000p3f);
}
fn coax_p_eg_water_foam_noise(p_p: vec2f, p_flow: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_flow: vec2f = p_flow;
  var l_t: f32;
  var l_phase_a: f32;
  var l_phase_b: f32;
  var l_wa: f32;
  var l_pa: vec2f;
  var l_pb: vec2f;
  var l_a: f32;
  var l_b: f32;
  l_t = coax_at_u(i32(1048i));
  l_phase_a = coax_p_fract((l_t / 0x1.99999ap0f));
  l_phase_b = coax_p_fract(((l_t / 0x1.99999ap0f) + 0x1.000000p-1f));
  l_wa = (0x1.000000p0f - abs(((0x1.000000p1f * l_phase_a) - 0x1.000000p0f)));
  l_pa = coax_p_wg_sub_f2f2(l_p, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_flow, l_phase_a), 0x1.99999ap0f));
  l_pb = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_flow, l_phase_b), 0x1.99999ap0f)), coax_p_v2(0x1.533334p2f, 0x1.733334p1f));
  l_a = coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(l_pa, 0x1.4cccccp0f), (l_t * 0x1.000000p-2f)));
  l_b = coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(l_pb, 0x1.4cccccp0f), (l_t * 0x1.000000p-2f)));
  if ((coax_at_u(i32(1070i)) > 0x1.800000p0f))
  {
    l_a = ((l_a * 0x1.333334p-1f) + (0x1.99999ap-2f * coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_pa, 0x1.066666p2f), coax_p_v2((l_t * 0x1.1eb852p-4f), 0.0f)), (l_t * 0x1.000000p-1f)))));
    l_b = ((l_b * 0x1.333334p-1f) + (0x1.99999ap-2f * coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_pb, 0x1.066666p2f), coax_p_v2((l_t * 0x1.1eb852p-4f), 0.0f)), (l_t * 0x1.000000p-1f)))));
  }
  return ((l_a * l_wa) + (l_b * (0x1.000000p0f - l_wa)));
}
fn coax_p_eg_water_foam(p_rest: vec2f, p_shore: f32, p_crest: f32, p_extra: f32, p_noise: f32, p_wake_foam: ptr<function, array<f32, 1>>) -> f32 {
  var l_rest: vec2f = p_rest;
  var l_shore: f32 = p_shore;
  var l_crest: f32 = p_crest;
  var l_extra: f32 = p_extra;
  var l_noise: f32 = p_noise;
  var l_band: f32;
  var l_foam: f32;
  l_band = coax_p_fmax(coax_at_u(i32(1064i)), 0x1.99999ap-5f);
  l_foam = ((0x1.000000p0f - coax_p_smoothstep(0.0f, l_band, l_shore)) * coax_p_smoothstep(0x1.333334p-2f, 0x1.3d70a4p-1f, (l_noise + (0x1.333334p-3f * (0x1.000000p0f - (l_shore / l_band))))));
  l_foam = coax_p_fmax(l_foam, (coax_p_smoothstep(0x1.3d70a4p-1f, 0x1.e66666p-1f, l_crest) * coax_p_smoothstep(0x1.666666p-2f, 0x1.666666p-1f, l_noise)));
  l_foam = coax_p_fmax(l_foam, (coax_p_clampf(l_extra, 0.0f, 0x1.000000p0f) * coax_p_smoothstep(0x1.000000p-2f, 0x1.4cccccp-1f, (l_noise + (0x1.000000p-2f * l_extra)))));
  l_foam = coax_p_fmax(l_foam, (coax_p_eg_water_ripple_foam(l_rest, p_wake_foam) * coax_p_smoothstep(0x1.99999ap-3f, 0x1.333334p-1f, l_noise)));
  return coax_p_clampf((l_foam * coax_at_u(i32(1059i))), 0.0f, 0x1.000000p0f);
}
fn coax_p_eg_water_transmittance(p_depth: f32, p_ci: f32) -> vec3f {
  var l_depth: f32 = p_depth;
  var l_ci: f32 = p_ci;
  var l_path: f32;
  l_path = (coax_p_fmax(l_depth, 0.0f) / coax_p_fmax(coax_p_eg_water_cos_refracted(l_ci), 0x1.99999ap-5f));
  return coax_p_wg_exp_f3(coax_p_wg_mul_f3f(coax_p_wg_neg_f3(coax_p_v3(coax_at_u(i32(1052i)), coax_at_u(i32(1053i)), coax_at_u(i32(1054i)))), l_path));
}
fn coax_p_eg_water_inscatter() -> vec3f {
  var l_sun: vec4f;
  var l_sky: vec3f;
  var l_lit: vec3f;
  l_sun = coax_p_eg_water_host_sun();
  l_sky = coax_p_eg_water_host_sky(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), 0x1.000000p0f);
  l_lit = coax_p_wg_mul_f3f(coax_p_eg_water_host_sun_light(), ((l_sun.w * coax_p_fmax(l_sun.y, 0.0f)) / 0x1.921fb6p1f));
  return coax_p_wg_mul_f3f3(coax_p_v3(coax_at_u(i32(1056i)), coax_at_u(i32(1057i)), coax_at_u(i32(1058i))), coax_p_wg_add_f3f3(l_lit, l_sky));
}
fn coax_p_wg_cvt_i2u2(p_p: coax_t29) -> coax_t38 {
  var l_p: coax_t29 = p_p;
  return coax_p_i2(i32(l_p.x), i32(l_p.y));
}
fn coax_p_wg_sub_i2i2(p_a: coax_t38, p_b: coax_t38) -> coax_t38 {
  var l_a: coax_t38 = p_a;
  var l_b: coax_t38 = p_b;
  return coax_p_i2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_load_2d_2(p_c: coax_t38, p_level: i32) -> vec4f {
  var l_c: coax_t38 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(water_scene, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_dims_depth_1() -> coax_t29 {
  return coax_p_u2(textureDimensions(water_scene_depth).x, textureDimensions(water_scene_depth).y);
}
fn coax_p_wg_load_depth(p_c: coax_t38, p_level: i32) -> f32 {
  var l_c: coax_t38 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(water_scene_depth, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_eg_water_view_w(p_d: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_a: f32;
  var l_b: f32;
  var l_w: f32;
  l_a = (coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))), coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i)))) / coax_p_fmax(coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i))), coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i)))), 0x1.0c6f7ap-20f));
  l_b = (coax_at_u(i32(19i)) - (l_a * coax_at_u(i32(23i))));
  l_w = (l_b / coax_p_fmin((l_d - l_a), (-0x1.ad7f2ap-24f)));
  return select(0x1.e84800p19f, l_w, ((l_d < 0x1.ffffdep-1f) && (l_w > 0.0f)));
}
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * coax_p_dot3(l_n, l_i))));
}
fn coax_p_wg_fract_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y), coax_p_fract(l_p0.z));
}
fn coax_p_wg_sub_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a - l_b.x), (l_a - l_b.y), (l_a - l_b.z));
}
fn coax_p_eg_water_host_light_shaded(p_world: vec3f, p_n: vec3f, p_sh: f32, p_paint_leaf_far: bool) -> vec3f {
  var l_world: vec3f = p_world;
  var l_n: vec3f = p_n;
  var l_sh: f32 = p_sh;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_amb: vec3f;
  l_amb = coax_p_v3(coax_p_verse_style_ambient(l_n.y, (coax_p_eg_st(5u)).x, (coax_p_eg_st(6u)).x, (coax_p_eg_st(5u)).w), coax_p_verse_style_ambient(l_n.y, (coax_p_eg_st(5u)).y, (coax_p_eg_st(6u)).y, (coax_p_eg_st(5u)).w), coax_p_verse_style_ambient(l_n.y, (coax_p_eg_st(5u)).z, (coax_p_eg_st(6u)).z, (coax_p_eg_st(5u)).w));
  return coax_p_wg_add_f3f3(l_amb, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_eg_st(0u)), (coax_p_eg_st(0u)).w), coax_p_fmax(coax_p_wg_dot3(l_n, coax_p_wg_sw_xyz_f4(coax_p_eg_st(1u))), 0.0f)), coax_p_eg_map_lit(l_world, l_n, l_paint_leaf_far)), l_sh));
}
fn coax_p_eg_hud_rect(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_e: f32;
  var l_t: f32;
  var l_look: f32;
  var l_h: vec2f;
  var l_d: f32;
  var l_dx: f32;
  var l_dy: f32;
  var l_cut: f32;
  var l_rim: f32;
  if (((((l_p.x < l_r.x) || (l_p.y < l_r.y)) || (l_p.x >= l_r.z)) || (l_p.y >= l_r.w)))
  {
    return l_under;
  }
  l_e = coax_p_fmin(coax_p_fmin((l_p.x - l_r.x), (l_r.z - l_p.x)), coax_p_fmin((l_p.y - l_r.y), (l_r.w - l_p.y)));
  l_t = coax_p_fmax(l_s.w, 0x1.000000p0f);
  l_look = l_s.y;
  if ((l_look > 0x1.e00000p2f))
  {
    l_h = coax_p_wg_max_f2f2(coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_r.z, l_r.w), coax_p_v2(l_r.x, l_r.y)), 0x1.000000p-1f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f));
    l_d = coax_p_wg_length2(coax_p_wg_div_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_h)), l_h));
    return coax_p_wg_mix_f3f3f(l_under, coax_p_v3(0x1.eb851ep-6f, 0.0f, 0.0f), (coax_p_clampf(l_s.x, 0.0f, 0x1.000000p0f) * (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-2f, 0x1.000000p0f, l_d))));
  }
  if ((l_look < 0x1.c00000p1f))
  {
    l_dx = coax_p_fmin((l_p.x - l_r.x), (l_r.z - l_p.x));
    l_dy = coax_p_fmin((l_p.y - l_r.y), (l_r.w - l_p.y));
    l_cut = (0x1.400000p2f * l_t);
    if (((l_dx + l_dy) < l_cut))
    {
      return l_under;
    }
    l_rim = coax_p_fmin(l_e, (((l_dx + l_dy) - l_cut) * 0x1.6a0902p-1f));
    if ((l_rim < l_t))
    {
      return coax_p_wg_mix_f3f3f(l_under, coax_p_v3(0x1.d70a3ep-2f, 0x1.eb851ep-2f, 0x1.000000p-1f), 0x1.ccccccp-1f);
    }
    return coax_p_wg_mix_f3f3f(l_under, coax_p_v3(0x1.70a3d8p-5f, 0x1.99999ap-5f, 0x1.db22d0p-5f), 0x1.a3d70ap-1f);
  }
  if ((l_look < 0x1.600000p2f))
  {
    if ((l_e >= l_t))
    {
      return l_under;
    }
    return select(coax_p_v3(0x1.0a3d70p-2f, 0x1.1eb852p-2f, 0x1.333334p-2f), coax_p_v3(0x1.eb851ep-2f, 0x1.70a3d8p-1f, 0x1.000000p0f), (l_look > 0x1.200000p2f));
  }
  if ((l_look < 0x1.a00000p2f))
  {
    if ((l_e < l_t))
    {
      return coax_p_v3(0x1.6a7efap-1f, 0x1.50e560p-1f, 0x1.0dd2f2p-1f);
    }
    return coax_p_wg_mix_f3f3f(l_under, coax_p_v3(0x1.bf7ceep-2f, 0x1.408312p-2f, 0x1.c49ba6p-3f), 0x1.e147aep-1f);
  }
  return coax_p_v3(0x1.000000p0f, 0x1.7851ecp-1f, 0x1.8624dep-2f);
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
fn coax_p_wg_max_i(p_a: i32, p_b: i32) -> i32 {
  var l_a: i32 = p_a;
  var l_b: i32 = p_b;
  if ((l_b > l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_wg_min_i(p_a: i32, p_b: i32) -> i32 {
  var l_a: i32 = p_a;
  var l_b: i32 = p_b;
  if ((l_b < l_a))
  {
    return l_b;
  }
  return l_a;
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
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
}
fn coax_p_i3(p_x: i32, p_y: i32, p_z: i32) -> coax_t45 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  var l_z: i32 = p_z;
  return coax_t45(l_x, l_y, l_z);
}
fn coax_p_EG_W_SHALLOW() -> vec3f {
  return coax_p_v3(0x1.333334p-2f, 0x1.70a3d8p-1f, 0x1.7ae148p-1f);
}
fn coax_p_wg_make_f2_f(p_p0: f32) -> vec2f {
  var l_p0: f32 = p_p0;
  return coax_p_v2(l_p0, l_p0);
}
fn coax_p_eg_water_painted_teal(p_world: vec3f, p_rest: vec2f, p_flow: vec2f, p_depth: f32, p_shore: f32, p_white: f32, p_n: vec3f, p_v: vec3f, p_footprint: f32, p_teal: f32, p_paint_leaf_far: bool, p_wake_foam: ptr<function, array<f32, 1>>) -> coax_t39 {
  var l_world: vec3f = p_world;
  var l_rest: vec2f = p_rest;
  var l_flow: vec2f = p_flow;
  var l_depth: f32 = p_depth;
  var l_shore: f32 = p_shore;
  var l_white: f32 = p_white;
  var l_n: vec3f = p_n;
  var l_v: vec3f = p_v;
  var l_footprint: f32 = p_footprint;
  var l_teal: f32 = p_teal;
  var l_paint_leaf_far: bool = p_paint_leaf_far;
  var l_o: coax_t39;
  var l_l: vec3f;
  var l_band: vec3f;
  var l_sh: f32;
  var l_light: vec3f;
  var l_body: vec3f;
  var l_calm: vec3f;
  var l_fresnel: f32;
  var l_refl: vec3f;
  var l_grid: f32;
  var l_cell: vec2f;
  var l_jitter: vec2f;
  var l_tilt: vec3f;
  var l_rdl: f32;
  var l_twinkle: f32;
  var l_centre: vec2f;
  var l_dot_: f32;
  var l_sparkle: f32;
  var l_line: f32;
  var l_broken: f32;
  var l_churn: f32;
  var l_noise: f32;
  var l_rings: f32;
  var l_speed: f32;
  var l_streak: f32;
  var l_along: vec2f;
  var l_q: vec2f;
  var l_ridge: f32;
  var l_broken_by: f32;
  var l_foam: f32;
  var l_alpha: f32;
  var l_edge: f32;
  var l_foam_rgb: vec3f;
  var l_emit: vec3f;
  l_o = coax_t39();
  l_l = coax_p_wg_sw_xyz_f4(coax_p_eg_water_host_sun());
  l_band = coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(coax_p_EG_W_SHALLOW(), coax_p_EG_W_MID(), coax_p_eg_water_paint_ramp(0x1.99999ap-2f, l_depth)), coax_p_EG_W_DEEP(), coax_p_eg_water_paint_ramp(0x1.800000p0f, l_depth)), coax_p_EG_W_TEAL(), l_teal);
  l_sh = coax_p_eg_shade_shadows(coax_p_v2(l_world.x, l_world.z));
  l_light = coax_p_eg_water_host_light_shaded(l_world, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_sh, l_paint_leaf_far);
  l_body = coax_p_wg_mul_f3f3(coax_p_wg_pow_f3f3(l_band, coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f)), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_light, 0x1.a3d70ap-1f), coax_p_v3(0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.99999ap-5f)));
  l_calm = coax_p_wg_normalize3(coax_p_wg_mix_f3f3f(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_n, 0x1.666666p-2f));
  l_fresnel = coax_p_fmin(coax_p_eg_water_fresnel(coax_p_fmax(coax_p_wg_dot3(l_calm, l_v), 0x1.0624dep-10f)), (0x1.000000p-1f - (0x1.99999ap-3f * l_teal)));
  l_refl = coax_p_eg_water_host_sky(coax_p_wg_reflect3(coax_p_wg_neg_f3(l_v), l_calm), 0.0f);
  l_grid = select(0x1.333334p-3f, 0x1.000000p-2f, (coax_at_u(i32(1068i)) < 0x1.000000p3f));
  l_cell = coax_p_wg_floor_f2(coax_p_wg_div_f2f(l_rest, l_grid));
  l_jitter = coax_p_wg_sub_f2f2(coax_p_v2(coax_p_eg_water_hash(coax_p_v3(l_cell.x, l_cell.y, 0x1.000000p0f)), coax_p_eg_water_hash(coax_p_v3(l_cell.x, l_cell.y, 0x1.000000p1f))), coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f));
  l_tilt = coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_n, coax_p_wg_mul_f3f(coax_p_v3(l_jitter.x, 0.0f, l_jitter.y), 0x1.666666p-2f)));
  l_rdl = coax_p_wg_dot3(coax_p_wg_reflect3(coax_p_wg_neg_f3(l_v), l_tilt), l_l);
  l_twinkle = (0x1.000000p-1f + (0x1.000000p-1f * sin((((coax_at_u(i32(2i)) * (0x1.000000p0f - coax_at_u(i32(4i)))) * 0x1.500000p4f) + (coax_p_eg_water_hash(coax_p_v3(l_cell.x, l_cell.y, 0x1.800000p1f)) * 0x1.91eb86p2f)))));
  l_centre = coax_p_wg_mul_f2f(coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(l_cell, coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f)), coax_p_wg_mul_f2f(l_jitter, 0x1.000000p-1f)), l_grid);
  l_dot_ = (0x1.000000p0f - coax_p_smoothstep((l_footprint * 0x1.333334p0f), (l_footprint * 0x1.19999ap1f), coax_p_wg_length2(coax_p_wg_sub_f2f2(l_rest, l_centre))));
  l_sparkle = ((((coax_p_eg_water_paint_sparkle(l_rdl) * l_twinkle) * l_dot_) * (coax_p_eg_water_host_sun()).w) * l_sh);
  l_line = 0.0f;
  if (((l_shore < 0x1.5c28f6p-2f) && (l_depth > (-0x1.47ae14p-6f))))
  {
    l_broken = ((coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(l_rest, 0x1.800000p1f), (coax_at_u(i32(2i)) * 0x1.333334p-2f))) - 0x1.000000p-1f) * 0x1.eb851ep-4f);
    l_line = ((0x1.000000p0f - coax_p_smoothstep((0x1.5c28f6p-3f + l_broken), (0x1.147ae2p-2f + l_broken), l_shore)) * coax_p_smoothstep((-0x1.47ae14p-6f), 0x1.47ae14p-5f, l_depth));
  }
  l_churn = 0.0f;
  if ((l_white > 0.0f))
  {
    l_noise = coax_p_eg_water_foam_noise(l_rest, l_flow);
    l_churn = (coax_p_clampf(l_white, 0.0f, 0x1.000000p0f) * coax_p_smoothstep(0x1.333334p-2f, 0x1.333334p-1f, (l_noise + (0x1.333334p-2f * l_white))));
  }
  l_rings = coax_p_clampf((coax_p_eg_water_ripple_foam(l_rest, p_wake_foam) * 0x1.99999ap0f), 0.0f, 0x1.000000p0f);
  l_speed = coax_p_wg_length2(l_flow);
  l_streak = 0.0f;
  if ((((l_speed > 0x1.99999ap-5f) && (l_depth > 0x1.99999ap-4f)) && (l_footprint < 0x1.000000p-2f)))
  {
    l_along = coax_p_wg_div_f2f(l_flow, coax_p_fmax(l_speed, 0x1.0624dep-10f));
    l_q = coax_p_v2((coax_p_wg_dot2(l_rest, coax_p_v2((-l_along.y), l_along.x)) * 0x1.333334p1f), ((coax_p_wg_dot2(l_rest, l_along) * 0x1.333334p-2f) - ((coax_at_u(i32(1048i)) * l_speed) * 0x1.333334p-2f)));
    l_ridge = (0x1.000000p0f - abs(((0x1.000000p1f * coax_p_eg_water_noise(coax_p_v3(l_q.x, l_q.y, (coax_at_u(i32(1048i)) * 0x1.333334p-3f)))) - 0x1.000000p0f)));
    l_broken_by = coax_p_smoothstep(0x1.99999ap-2f, 0x1.666666p-1f, coax_p_eg_water_noise(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(l_rest, 0x1.333334p-1f), (coax_at_u(i32(1048i)) * 0x1.99999ap-4f))));
    l_streak = (((((coax_p_smoothstep(0x1.e66666p-1f, 0x1.fd70a4p-1f, l_ridge) * l_broken_by) * coax_p_smoothstep(0x1.99999ap-5f, 0x1.99999ap-2f, l_speed)) * 0x1.666666p-2f) * coax_p_smoothstep(0x1.99999ap-4f, 0x1.000000p-1f, l_depth)) * (0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-4f, 0x1.000000p-2f, l_footprint)));
  }
  l_foam = coax_p_clampf((coax_p_fmax(coax_p_fmax(l_line, l_churn), l_rings) + l_streak), 0.0f, 0x1.000000p0f);
  l_alpha = (((0x1.000000p-1f + (0x1.99999ap-3f * l_teal)) + (0x1.ae147ap-2f * coax_p_smoothstep(0x1.99999ap-5f, 0x1.333334p0f, l_depth))) * coax_p_smoothstep((-0x1.99999ap-5f), 0x1.99999ap-4f, l_depth));
  l_edge = coax_p_smoothstep((-0x1.99999ap-5f), 0x1.99999ap-4f, l_depth);
  l_foam_rgb = coax_p_wg_mul_f3f3(coax_p_v3(0x1.e66666p-1f, 0x1.f0a3d8p-1f, 0x1.eb851ep-1f), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_light, 0x1.ccccccp-1f), coax_p_v3(0x1.eb851ep-5f, 0x1.eb851ep-5f, 0x1.eb851ep-5f)));
  l_emit = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_body, l_alpha), (0x1.000000p0f - l_fresnel)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_refl, l_fresnel), l_edge)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.c28f5cp-1f), l_sparkle), coax_p_eg_water_host_sun_light()), 0x1.333334p0f));
  l_emit = coax_p_wg_mix_f3f3f(l_emit, l_foam_rgb, l_foam);
  l_o.x = l_emit;
  l_o.y = coax_p_wg_make_f3_f((((0x1.000000p0f - l_alpha) * (0x1.000000p0f - (l_fresnel * l_edge))) * (0x1.000000p0f - l_foam)));
  l_o.z = coax_p_v2(l_n.x, l_n.z);
  l_o.w = coax_p_v4(l_refl.x, l_refl.y, l_refl.z, ((l_fresnel * l_edge) * (0x1.000000p0f - l_foam)));
  return l_o;
}
fn coax_p_eg_water_detail_waves(p_p: vec2f, p_footprint: f32, p_gain: f32) -> vec3f {
  var l_p: vec2f = p_p;
  var l_footprint: f32 = p_footprint;
  var l_gain: f32 = p_gain;
  var l_g: vec2f;
  var l_lost: f32;
  var l_t: f32;
  var l_n: i32;
  var l_i: i32;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_k: f32;
  var l_slope: f32;
  var l_keep: f32;
  var l_theta: f32;
  l_g = coax_p_v2(0.0f, 0.0f);
  l_lost = 0.0f;
  l_t = coax_at_u(i32(1048i));
  l_n = coax_p_wg_min_i(coax_p_wg_i32_f(coax_at_u(i32(1068i))), coax_p_wg_i32_f(coax_at_u(i32(1049i))));
  {
    l_i = 0i;
    while ((l_i < 16i))
    {
      if ((l_i >= l_n))
      {
        break;
      }
      l_a = coax_p_wg_at4(((i32((l_i * 2i)) * 4i) + 824i));
      l_b = coax_p_wg_at4(((i32(((l_i * 2i) + 1i)) * 4i) + 824i));
      l_k = l_a.z;
      l_slope = ((l_a.w * l_k) * l_gain);
      l_keep = coax_p_clampf(((l_b.z / (0x1.800000p2f * coax_p_fmax(l_footprint, 0x1.a36e2ep-14f))) - 0x1.000000p-1f), 0.0f, 0x1.000000p0f);
      l_theta = (((l_k * coax_p_wg_dot2(coax_p_v2(l_a.x, l_a.y), l_p)) - (l_b.x * l_t)) + l_b.y);
      l_g = coax_p_wg_add_f2f2(l_g, coax_p_wg_mul_f2f(coax_p_v2(l_a.x, l_a.y), ((l_slope * l_keep) * cos(l_theta))));
      l_lost = (l_lost + (((0x1.000000p-1f * l_slope) * l_slope) * (0x1.000000p0f - (l_keep * l_keep))));
      l_i = (l_i + 1i);
    }
  }
  return coax_p_v3(l_g.x, l_g.y, l_lost);
}
fn coax_p_eg_water_wake(p_p: vec2f, p_wake_foam: ptr<function, array<f32, 1>>) -> vec2f {
  var l_p: vec2f = p_p;
  var l_g: vec2f;
  var l_t: f32;
  var l_m: u32;
  var l_base: u32;
  var l_head: vec4f;
  var l_behind: f32;
  var l_best: f32;
  var l_at: vec4f;
  var l_dir: vec2f;
  var l_k: u32;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_seg: vec2f;
  var l_len_2: f32;
  var l_d: vec2f;
  var l_s: f32;
  var l_off: vec2f;
  var l_lat: f32;
  var l_v: f32;
  var l_age: f32;
  var l_fade: f32;
  var l_side: f32;
  var l_across: vec2f;
  var l_l: f32;
  var l_wave: f32;
  var l_feather: f32;
  var l_arm: f32;
  var l_w: f32;
  var l_x: f32;
  var l_amp: f32;
  var l_inside: f32;
  var l_trail: f32;
  l_g = coax_p_v2(0.0f, 0.0f);
  l_t = coax_at_u(i32(1048i));
  {
    l_m = 0u;
    while ((l_m < 2u))
    {
      l_base = (5u + (l_m * 8u));
      l_head = coax_p_wg_at4(((i32(l_base) * 4i) + 1500i));
      if (((l_head.x == 0.0f) && (l_head.y == 0.0f)))
      {
        {
          l_m = (l_m + 1u);
          continue;
        }
      }
      if (((l_head.w < 0x1.99999ap-5f) && ((l_t - coax_at_u(i32(((i32((l_base + 1u)) * 4i) + 1502i)))) > 0x1.000000p3f)))
      {
        {
          l_m = (l_m + 1u);
          continue;
        }
      }
      if ((coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_head.x, l_head.y))) > 0x1.c00000p3f))
      {
        {
          l_m = (l_m + 1u);
          continue;
        }
      }
      l_behind = 0.0f;
      l_best = 0x1.dcd650p29f;
      l_at = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
      l_dir = coax_p_v2(0.0f, 0.0f);
      {
        l_k = 0u;
        while ((l_k < 7u))
        {
          l_a = coax_p_wg_at4(((i32((l_base + l_k)) * 4i) + 1500i));
          l_b = coax_p_wg_at4(((i32(((l_base + l_k) + 1u)) * 4i) + 1500i));
          if (((l_b.x == 0.0f) && (l_b.y == 0.0f)))
          {
            break;
          }
          l_seg = coax_p_wg_sub_f2f2(coax_p_v2(l_b.x, l_b.y), coax_p_v2(l_a.x, l_a.y));
          l_len_2 = coax_p_wg_length2(l_seg);
          if ((l_len_2 < 0x1.47ae14p-7f))
          {
            {
              l_k = (l_k + 1u);
              continue;
            }
          }
          l_d = coax_p_wg_div_f2f(l_seg, l_len_2);
          l_s = coax_p_clampf(coax_p_wg_dot2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_a.x, l_a.y)), l_d), 0.0f, l_len_2);
          l_off = coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_a.x, l_a.y), coax_p_wg_mul_f2f(l_d, l_s)));
          l_lat = coax_p_wg_length2(l_off);
          if ((l_lat < l_best))
          {
            l_best = l_lat;
            l_at = coax_p_v4((l_behind + l_s), select((-l_lat), l_lat, (((l_d.x * l_off.y) - (l_d.y * l_off.x)) > 0.0f)), (l_t - coax_p_mix(l_a.z, l_b.z, (l_s / l_len_2))), coax_p_fmax(l_a.w, l_b.w));
            l_dir = l_d;
          }
          l_behind = (l_behind + l_len_2);
          l_k = (l_k + 1u);
        }
      }
      if (((l_best > 0x1.400000p4f) || (l_at.w < 0x1.99999ap-5f)))
      {
        {
          l_m = (l_m + 1u);
          continue;
        }
      }
      l_v = coax_p_fmin(l_at.w, 0x1.400000p1f);
      l_age = coax_p_fmax(l_at.z, 0.0f);
      l_fade = ((exp(((-l_at.x) / 0x1.000000p3f)) * exp(((-l_age) * 0x1.333334p-2f))) * coax_p_smoothstep(0x1.99999ap-5f, 0x1.99999ap-2f, l_v));
      if ((l_fade < 0x1.47ae14p-7f))
      {
        {
          l_m = (l_m + 1u);
          continue;
        }
      }
      l_side = select((-0x1.000000p0f), 0x1.000000p0f, (l_at.y > 0.0f));
      l_across = coax_p_wg_mul_f2f(coax_p_v2((-l_dir.y), l_dir.x), l_side);
      l_l = abs(l_at.y);
      l_wave = coax_p_fmax((((0x1.921fb6p2f * l_v) * l_v) / 0x1.39eb86p3f), 0x1.333334p-2f);
      l_feather = (0x1.4cccccp-1f + (0x1.666666p-2f * cos(((0x1.921fb6p2f * l_at.x) / l_wave))));
      l_arm = (0x1.333334p-2f + (0x1.6a161ep-2f * l_at.x));
      l_w = (0x1.47ae14p-3f + (0x1.99999ap-5f * l_at.x));
      l_x = (l_l - l_arm);
      l_amp = ((0x1.70a3d8p-4f * l_fade) * coax_p_fmin((l_v / 0x1.333334p0f), 0x1.666666p0f));
      l_g = coax_p_wg_add_f2f2(l_g, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_across, (((-0x1.000000p1f) * l_x) / (l_w * l_w))), exp((((-l_x) * l_x) / (l_w * l_w)))), l_amp), l_w), l_feather));
      l_inside = (0x1.000000p0f - coax_p_smoothstep((l_arm * 0x1.666666p-1f), l_arm, l_l));
      l_g = coax_p_wg_add_f2f2(l_g, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_dir, sin(((0x1.921fb6p2f * l_at.x) / l_wave))), l_amp), 0x1.000000p-1f), l_inside));
      l_trail = (0x1.c28f5cp-3f + (0x1.1eb852p-4f * l_at.x));
      (*p_wake_foam)[0i] = coax_p_fmax((*p_wake_foam)[0i], ((((0x1.000000p0f - coax_p_smoothstep((l_trail * 0x1.666666p-2f), l_trail, l_l)) * l_fade) * coax_p_fmin((l_v * 0x1.333334p0f), 0x1.000000p0f)) * exp(((-l_age) * 0x1.000000p-2f))));
      (*p_wake_foam)[0i] = coax_p_fmax((*p_wake_foam)[0i], (((exp((((-l_x) * l_x) / ((l_w * l_w) * 0x1.000000p-2f))) * l_fade) * coax_p_clampf((l_v - 0x1.333334p-1f), 0.0f, 0x1.000000p0f)) * 0x1.333334p-1f));
      l_m = (l_m + 1u);
    }
  }
  return l_g;
}
fn coax_p_wg_log2(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (log(l_x) / 0x1.62e430p-1f);
}
fn coax_p_eg_water_ggx(p_noh: f32, p_a2: f32) -> f32 {
  var l_noh: f32 = p_noh;
  var l_a2: f32 = p_a2;
  var l_d: f32;
  l_d = (((l_noh * l_noh) * (l_a2 - 0x1.000000p0f)) + 0x1.000000p0f);
  return (l_a2 / ((0x1.921fb6p1f * l_d) * l_d));
}
fn coax_p_eg_water_smith(p_nov: f32, p_nol: f32, p_a2: f32) -> f32 {
  var l_nov: f32 = p_nov;
  var l_nol: f32 = p_nol;
  var l_a2: f32 = p_a2;
  var l_gv: f32;
  var l_gl: f32;
  l_gv = (l_nol * sqrt((((l_nov * l_nov) * (0x1.000000p0f - l_a2)) + l_a2)));
  l_gl = (l_nov * sqrt((((l_nol * l_nol) * (0x1.000000p0f - l_a2)) + l_a2)));
  return (0x1.000000p-1f / coax_p_fmax((l_gv + l_gl), 0x1.0c6f7ap-20f));
}
fn coax_p_eg_water_ripple_foam(p_p: vec2f, p_wake_foam: ptr<function, array<f32, 1>>) -> f32 {
  var l_p: vec2f = p_p;
  var l_foam: f32;
  var l_t: f32;
  var l_count: i32;
  var l_i: i32;
  var l_r: vec4f;
  var l_age: f32;
  var l_x: f32;
  l_foam = (*p_wake_foam)[0i];
  l_t = coax_at_u(i32(1048i));
  l_count = coax_p_wg_i32_f(coax_at_u(i32(1050i)));
  {
    l_i = 0i;
    while ((l_i < 24i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_r = coax_p_wg_at4(((i32(l_i) * 4i) + 952i));
      l_age = (l_t - l_r.z);
      l_x = (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_r.x, l_r.y))) - (0x1.19999ap0f * l_age));
      l_foam = coax_p_fmax(l_foam, ((exp((((-l_x) * l_x) / 0x1.47ae14p-4f)) * exp(((-l_age) * 0x1.99999ap0f))) * coax_p_clampf((l_r.w * 0x1.900000p4f), 0.0f, 0x1.000000p0f)));
      l_i = (l_i + 1i);
    }
  }
  return l_foam;
}
fn coax_p_eg_water_cos_refracted(p_ci: f32) -> f32 {
  var l_ci: f32 = p_ci;
  return sqrt(coax_p_fmax((0x1.000000p0f - ((0x1.000000p0f - (l_ci * l_ci)) / (0x1.547ae2p0f * 0x1.547ae2p0f))), 0.0f));
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_EG_W_MID() -> vec3f {
  return coax_p_v3(0x1.99999ap-4f, 0x1.ccccccp-2f, 0x1.19999ap-1f);
}
fn coax_p_eg_water_paint_ramp(p_edge: f32, p_x: f32) -> f32 {
  var l_edge: f32 = p_edge;
  var l_x: f32 = p_x;
  return coax_p_smoothstep((l_edge - 0x1.333334p-4f), (l_edge + 0x1.333334p-4f), l_x);
}
fn coax_p_EG_W_DEEP() -> vec3f {
  return coax_p_v3(0x1.99999ap-5f, 0x1.c28f5cp-3f, 0x1.70a3d8p-2f);
}
fn coax_p_EG_W_TEAL() -> vec3f {
  return coax_p_v3(0x1.eb851ep-4f, 0x1.666666p-1f, 0x1.47ae14p-1f);
}
fn coax_p_eg_water_paint_sparkle(p_rdl: f32) -> f32 {
  var l_rdl: f32 = p_rdl;
  return coax_p_smoothstep(0x1.fec030p-1f, 0x1.ffb00cp-1f, l_rdl);
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8, coax_r.f9);
}
@vertex
fn shadow_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_16 {
  let coax_r = coax_p_shadow_vs(coax_vid, coax_iid);
  return coax_io_16(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_17) -> coax_io_18 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t17(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_18(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@vertex
fn water_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_19 {
  let coax_r = coax_p_water_vs(coax_vid, coax_iid);
  return coax_io_19(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@vertex
fn grass_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_21 {
  let coax_r = coax_p_grass_vs(coax_vid, coax_iid);
  return coax_io_21(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_23 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_23(coax_r.x, coax_r.y);
}
@fragment
fn shadow_fs(coax_in: coax_io_16) {
  coax_p_shadow_fs(coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
}
@fragment
fn scene_fs(coax_in: coax_io_15, @builtin(front_facing) coax_front: bool) -> coax_out_23 {
  let coax_r = coax_p_scene_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8, coax_in.f9), coax_front);
  return coax_out_23(coax_r.x, coax_r.y);
}
@fragment
fn fx_fs(coax_in: coax_io_18) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t18(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_r;
}
@fragment
fn water_transmit_fs(coax_in: coax_io_19) -> @location(0) vec4f {
  let coax_r = coax_p_water_transmit_fs(coax_t19(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_r;
}
@fragment
fn water_emit_fs(coax_in: coax_io_19) -> @location(0) vec4f {
  let coax_r = coax_p_water_emit_fs(coax_t19(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_r;
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
fn grass_fs(coax_in: coax_io_21) -> coax_out_23 {
  let coax_r = coax_p_grass_fs(coax_t21(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_out_23(coax_r.x, coax_r.y);
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