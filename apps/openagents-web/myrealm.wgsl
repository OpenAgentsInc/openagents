// ---- generated: apps/openagents-web/gpu/myrealm.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t14 {
  x: vec3f,
  y: vec3f,
}
struct coax_t15 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec3f,
  f5: u32,
  f6: f32,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec3f,
  @location(4) @interpolate(flat) f5: u32,
  @location(5) @interpolate(flat) f6: f32,
}
struct coax_t16 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_16 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t17 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_17 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 113>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> objects: array<coax_t11>;
@group(1) @binding(2) var sheets: texture_2d_array<f32>;
@group(1) @binding(3) var sheet: sampler;
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
  var l_m: coax_t11;
  var l_v: coax_t14;
  var l_o: coax_t15;
  var l_b: u32;
  var l_p: vec4f;
  var l_n: vec3f;
  var l_m_11: coax_t11;
  var l_w: vec3f;
  var l_wp: vec4f;
  var l_o_14: coax_t15;
  if ((l_id >= 16777216u))
  {
    l_m = objects[i32(l_instance)];
    l_v = coax_p_myrealm_round_vertex(l_m, (l_id - 16777216u));
    l_o = coax_t15();
    l_o.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f)), coax_p_wg_dot4(coax_p_wg_at4(12i), coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f)), coax_p_wg_dot4(coax_p_wg_at4(16i), coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f)), coax_p_wg_dot4(coax_p_wg_at4(20i), coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f)));
    l_o.y = l_v.x;
    l_o.z = l_v.y;
    l_o.w = coax_p_v2(0.0f, 0.0f);
    l_o.f4 = coax_p_wg_mul_f3f(coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z), 0x1.99999ap-1f);
    l_o.f5 = 4096u;
    l_o.f6 = l_m.w.w;
    return l_o;
  }
  l_b = (l_id * 12u);
  l_p = coax_p_v4(coax_p_myrealm_word(l_b), coax_p_myrealm_word((l_b + 1u)), coax_p_myrealm_word((l_b + 2u)), 0x1.000000p0f);
  l_n = coax_p_v3(coax_p_myrealm_word((l_b + 3u)), coax_p_myrealm_word((l_b + 4u)), coax_p_myrealm_word((l_b + 5u)));
  l_m_11 = objects[i32(l_instance)];
  l_w = coax_p_v3(coax_p_wg_dot4(l_m_11.x, l_p), coax_p_wg_dot4(l_m_11.y, l_p), coax_p_wg_dot4(l_m_11.z, l_p));
  l_wp = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  l_o_14 = coax_t15();
  l_o_14.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(12i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(16i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(20i), l_wp));
  l_o_14.y = l_w;
  l_o_14.z = coax_p_v3(coax_p_wg_dot3(coax_p_v3(l_m_11.x.x, l_m_11.x.y, l_m_11.x.z), l_n), coax_p_wg_dot3(coax_p_v3(l_m_11.y.x, l_m_11.y.y, l_m_11.y.z), l_n), coax_p_wg_dot3(coax_p_v3(l_m_11.z.x, l_m_11.z.y, l_m_11.z.z), l_n));
  l_o_14.w = coax_p_v2(coax_p_myrealm_word((l_b + 6u)), coax_p_myrealm_word((l_b + 7u)));
  l_o_14.f4 = coax_p_wg_mul_f3f3(coax_p_wg_sw_rgb_f4(coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 8u))])), coax_p_v3(l_m_11.w.x, l_m_11.w.y, l_m_11.w.z));
  l_o_14.f5 = (vertices[i32((l_b + 8u))] >> u32(24u));
  l_o_14.f6 = l_m_11.w.w;
  return l_o_14;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t16) -> coax_t17 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t16 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t17;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t17();
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
  var l_ndc: vec2f;
  var l_off: vec3f;
  var l_ortho: bool;
  var l_ray: vec3f;
  var l_org: vec3f;
  var l_down: bool;
  var l_t: f32;
  var l_hit: vec3f;
  var l_r: f32;
  var l_sky_s: vec3f;
  var l_edge_s: f32;
  var l_far: vec3f;
  var l_cs: vec3f;
  var l_sky: vec3f;
  var l_big: f32;
  var l_fine: f32;
  var l_brush: f32;
  var l_vary: f32;
  var l_patches: f32;
  var l_hue: vec3f;
  var l_ground: vec3f;
  var l_wi: f32;
  var l_rip: f32;
  var l_mirror: vec3f;
  var l_deep: f32;
  var l_water: vec3f;
  var l_rim: f32;
  var l_rip_30: f32;
  var l_sea: vec3f;
  var l_haze: f32;
  var l_edge: f32;
  var l_c: vec3f;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  l_ndc = coax_p_v2((((l_p.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_p.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_off = coax_p_wg_add_f3f3(coax_p_wg_mul_ff3(l_ndc.x, coax_p_v3(coax_at_u(i32(36i)), coax_at_u(i32(37i)), coax_at_u(i32(38i)))), coax_p_wg_mul_ff3(l_ndc.y, coax_p_v3(coax_at_u(i32(40i)), coax_at_u(i32(41i)), coax_at_u(i32(42i)))));
  l_ortho = (coax_at_u(i32(35i)) > 0x1.000000p-1f);
  l_ray = select(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(32i)), coax_at_u(i32(33i)), coax_at_u(i32(34i))), l_off), coax_p_v3(coax_at_u(i32(32i)), coax_at_u(i32(33i)), coax_at_u(i32(34i))), l_ortho);
  l_org = select(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), l_off), l_ortho);
  l_down = (l_ray.y < (-0x1.a36e2ep-14f));
  l_t = select(0x1.f40000p9f, ((-l_org.y) / coax_p_fmin(l_ray.y, (-0x1.a36e2ep-14f))), l_down);
  l_hit = coax_p_wg_add_f3f3(l_org, coax_p_wg_mul_f3f(l_ray, coax_p_fmin(l_t, 0x1.f40000p9f)));
  l_r = coax_p_wg_length2(coax_p_v2(l_hit.x, l_hit.z));
  if (coax_p_myrealm_styled())
  {
    l_sky_s = coax_p_myrealm_sky_styled(coax_p_wg_normalize3(l_ray), l_p);
    l_edge_s = select(0.0f, (0x1.000000p0f - coax_p_smoothstep((coax_at_u(i32(31i)) - 0x1.800000p1f), (coax_at_u(i32(31i)) + 0x1.800000p2f), l_r)), l_down);
    l_far = coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(13u));
    l_cs = l_sky_s;
    if (l_down)
    {
      l_cs = coax_p_wg_mix_f3f3f(l_far, coax_p_myrealm_styled_ground(l_hit, l_ray, (l_t * coax_p_wg_length3(l_ray)), l_p), select(l_edge_s, 0x1.000000p0f, (coax_p_myrealm_land() > 0x1.800000p0f)));
    }
    if (((((coax_p_myrealm_land() > 0x1.000000p-1f) && (coax_p_myrealm_land() < 0x1.800000p0f)) && l_down) && (l_r > coax_at_u(i32(31i)))))
    {
      l_cs = l_sky_s;
    }
    return coax_p_wg_make_f4_f3f(coax_p_myrealm_panels(l_p, coax_p_wg_mul_f3f(coax_p_myrealm_encoded(l_cs), coax_at_u(i32(3i)))), 0x1.000000p0f);
  }
  l_sky = coax_p_myrealm_sky_of(l_ray, l_p);
  l_big = coax_p_myrealm_noise(coax_p_wg_mul_f2f(coax_p_v2(l_hit.x, l_hit.z), 0x1.70a3d8p-3f));
  l_fine = coax_p_myrealm_noise(coax_p_wg_mul_f2f(coax_p_v2(l_hit.x, l_hit.z), 0x1.b33334p0f));
  l_brush = coax_p_myrealm_noise(coax_p_v2(((l_hit.x * 0x1.ccccccp-1f) + (l_hit.z * 0x1.99999ap-2f)), (l_hit.z * 0x1.99999ap1f)));
  l_vary = select(((0x1.a3d70ap-1f + (0x1.c28f5cp-3f * l_big)) + (0x1.47ae14p-4f * l_fine)), ((0x1.8f5c28p-1f + (0x1.99999ap-3f * l_big)) + (0x1.47ae14p-3f * l_brush)), (coax_at_u(i32(47i)) > 0x1.000000p-1f));
  l_patches = coax_p_myrealm_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_hit.x, l_hit.z), 0x1.eb851ep-5f), coax_p_v2(0x1.8cccccp1f, 0x1.ecccccp2f)));
  l_hue = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.1eb852p0f, 0x1.0f5c28p0f, 0x1.8f5c28p-1f), coax_p_v3(0x1.ae147ap-1f, 0x1.000000p0f, 0x1.f5c290p-1f), coax_p_smoothstep(0x1.000000p-2f, 0x1.800000p-1f, l_patches));
  l_ground = coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(52i)), coax_at_u(i32(53i)), coax_at_u(i32(54i))), l_vary), l_hue), coax_p_myrealm_daylight(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)));
  l_wi = coax_p_myrealm_water_in(coax_p_v2(l_hit.x, l_hit.z));
  if (((coax_at_u(i32(63i)) > 0x1.000000p-1f) && (l_wi > 0.0f)))
  {
    l_rip = (coax_p_myrealm_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_hit.x, l_hit.z), 0x1.333334p1f), coax_p_v2((coax_at_u(i32(2i)) * 0x1.333334p-2f), (coax_at_u(i32(2i)) * 0x1.99999ap-3f)))) - 0x1.000000p-1f);
    l_mirror = coax_p_myrealm_sky_of(coax_p_v3((l_ray.x + (l_rip * 0x1.99999ap-5f)), (-l_ray.y), (l_ray.z + (l_rip * 0x1.99999ap-5f))), l_p);
    l_deep = coax_p_smoothstep(0.0f, (coax_at_u(i32(62i)) * 0x1.333334p-1f), l_wi);
    l_water = coax_p_wg_mix_f3f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_mirror, 0x1.800000p-1f), coax_p_v3(0x1.47ae14p-6f, 0x1.99999ap-5f, 0x1.47ae14p-4f)), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f3(coax_p_v3(0x1.eb851ep-6f, 0x1.70a3d8p-4f, 0x1.1eb852p-3f), coax_p_myrealm_daylight(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f))), coax_p_wg_mul_f3f(l_mirror, 0x1.99999ap-2f)), (l_deep * 0x1.333334p-1f));
    l_rim = (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.000000p-2f, l_wi));
    l_ground = coax_p_wg_mix_f3f3f(l_water, coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(52i)), coax_at_u(i32(53i)), coax_at_u(i32(54i))), 0x1.333334p0f), coax_p_myrealm_daylight(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f))), (l_rim * 0x1.000000p-1f));
    if (((coax_at_u(i32(63i)) > 0x1.400000p1f) && (coax_at_u(i32(63i)) < 0x1.c00000p1f)))
    {
      l_ground = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f3(coax_p_myrealm_ice_at(coax_p_v2(l_hit.x, l_hit.z)), coax_p_myrealm_daylight(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f))), coax_p_wg_mul_f3f(l_mirror, 0x1.eb851ep-4f));
    }
    if ((coax_at_u(i32(63i)) > 0x1.c00000p1f))
    {
      l_ground = coax_p_myrealm_lava_at(coax_p_v2(l_hit.x, l_hit.z));
    }
  }
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.3d70a4p-1f, 0x1.1eb852p-2f), coax_p_myrealm_lamp_near(l_hit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f))), 0x1.19999ap-1f), coax_at_u(i32(55i))));
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_myrealm_lanterns_near(l_hit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)), 0x1.ccccccp-1f));
  if ((coax_p_myrealm_land() > 0x1.800000p0f))
  {
    l_rip_30 = (coax_p_myrealm_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_hit.x, l_hit.z), 0x1.333334p0f), coax_p_v2((coax_at_u(i32(2i)) * 0x1.99999ap-3f), (coax_at_u(i32(2i)) * 0x1.99999ap-4f)))) - 0x1.000000p-1f);
    l_sea = coax_p_myrealm_sea_at(coax_p_v2(l_hit.x, l_hit.z), coax_p_myrealm_sky_of(coax_p_v3((l_ray.x + (l_rip_30 * 0x1.99999ap-5f)), (-l_ray.y), (l_ray.z + (l_rip_30 * 0x1.99999ap-5f))), l_p));
    l_ground = coax_p_wg_mix_f3f3f(l_ground, l_sea, coax_p_smoothstep((coax_at_u(i32(31i)) - 0x1.800000p1f), (coax_at_u(i32(31i)) - 0x1.19999ap1f), l_r));
  }
  if (((coax_p_myrealm_land() > 0x1.000000p-1f) && (coax_p_myrealm_land() < 0x1.800000p0f)))
  {
    l_ground = coax_p_wg_mix_f3f3f(l_ground, coax_p_wg_mul_f3f3(coax_p_v3(0x1.47ae14p-2f, 0x1.c28f5cp-3f, 0x1.333334p-3f), coax_p_myrealm_daylight(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f))), coax_p_smoothstep((coax_at_u(i32(31i)) - 0x1.ccccccp-1f), (coax_at_u(i32(31i)) - 0x1.99999ap-2f), l_r));
  }
  l_haze = coax_p_clampf((0x1.000000p0f - exp(((-coax_p_fmax((l_t - coax_at_u(i32(39i))), 0.0f)) * 0x1.26e978p-6f))), 0.0f, 0x1.000000p0f);
  l_ground = coax_p_wg_mix_f3f3f(l_ground, coax_p_v3(coax_at_u(i32(48i)), coax_at_u(i32(49i)), coax_at_u(i32(50i))), (l_haze * 0x1.b33334p-1f));
  l_edge = select(0.0f, (0x1.000000p0f - coax_p_smoothstep((coax_at_u(i32(31i)) - 0x1.800000p1f), (coax_at_u(i32(31i)) + 0x1.800000p2f), l_r)), l_down);
  if (((coax_p_myrealm_land() > 0x1.800000p0f) && l_down))
  {
    l_edge = 0x1.000000p0f;
  }
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_sky, coax_p_wg_mix_f3f3f(coax_p_v3(coax_at_u(i32(48i)), coax_at_u(i32(49i)), coax_at_u(i32(50i))), l_ground, l_edge), select(0.0f, 0x1.000000p0f, l_down)), coax_at_u(i32(3i)));
  if (((((coax_p_myrealm_land() > 0x1.000000p-1f) && (coax_p_myrealm_land() < 0x1.800000p0f)) && l_down) && (l_r > coax_at_u(i32(31i)))))
  {
    l_c = coax_p_wg_mul_f3f(l_sky, coax_at_u(i32(3i)));
  }
  return coax_p_wg_make_f4_f3f(coax_p_myrealm_panels(l_p, l_c), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t15) -> vec4f {
  var l_s: coax_t15 = p_s;
  var l_rounded: bool;
  var l_layer: u32;
  var l_sampled: vec4f;
  var l_bare: bool;
  var l_tex: vec4f;
  var l_cut: f32;
  var l_n: vec3f;
  var l_d: vec3f;
  var l_dist: f32;
  var l_soft: vec3f;
  var l_a: vec3f;
  var l_c: vec3f;
  var l_lit: vec3f;
  var l_far: f32;
  var l_haze: f32;
  var l_thick: f32;
  var l_c_20: vec3f;
  l_rounded = (l_s.f5 >= 4096u);
  l_layer = select(l_s.f5, 0u, l_rounded);
  l_sampled = textureSample(sheets, sheet, l_s.w, i32(l_layer));
  l_bare = (l_rounded || (l_s.f6 < (-0x1.400000p1f)));
  l_tex = select(coax_p_wg_make_f4_f3f(coax_p_myrealm_surfaced(coax_p_v3(l_sampled.x, l_sampled.y, l_sampled.z), l_s.f6), l_sampled.w), coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_bare);
  if (((l_s.f6 < (-0x1.800000p0f)) && (!l_bare)))
  {
    l_tex = coax_p_wg_make_f4_f3f(coax_p_wg_make_f3_f(coax_p_myrealm_masonry(l_s.y)), 0x1.000000p0f);
  }
  l_cut = select(select(l_tex.w, (textureSampleLevel(sheets, sheet, l_s.w, i32(l_layer), 0x1.000000p1f)).w, ((coax_at_u(i32(449i)) > 0x1.000000p-1f) && (coax_at_u(i32(449i)) < 0x1.800000p0f))), 0x1.000000p0f, l_bare);
  if ((l_cut < 0x1.666666p-2f))
  {
    discard;
  }
  if (((l_s.y.y < (-0x1.47ae14p-5f)) && (((coax_p_wg_length2(coax_p_v2(l_s.y.x, l_s.y.z)) < coax_at_u(i32(31i))) || (coax_p_myrealm_land() < 0x1.000000p-1f)) || (coax_p_myrealm_land() > 0x1.800000p0f))))
  {
    discard;
  }
  l_n = coax_p_wg_normalize3(l_s.z);
  if (coax_p_myrealm_styled())
  {
    l_d = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i)))));
    l_dist = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i)))));
    l_soft = select(coax_p_myrealm_surfaced(coax_p_wg_sw_rgb_f4(textureSampleLevel(sheets, sheet, l_s.w, i32(l_s.f5), select(0x1.400000p1f, 0x1.400000p2f, coax_p_myrealm_flat_style()))), l_s.f6), coax_p_v3(l_tex.x, l_tex.y, l_tex.z), ((l_s.f6 < (-0x1.800000p0f)) || l_rounded));
    l_a = coax_p_wg_mul_f3f3(coax_p_myrealm_paletted(coax_p_wg_mix_f3f3f(coax_p_v3(l_tex.x, l_tex.y, l_tex.z), l_soft, select(0x1.19999ap-1f, 0x1.99999ap-1f, coax_p_myrealm_flat_style()))), l_s.f4);
    l_c = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(select(coax_p_myrealm_painted(l_a, l_n, l_d), coax_p_myrealm_flat_lit(l_a, l_n), coax_p_myrealm_flat_style()), coax_p_myrealm_styled_lights(l_a, l_s.y, l_n)), coax_p_wg_mul_f3f(l_a, coax_p_fmax(l_s.f6, 0.0f)));
    l_c = coax_p_myrealm_styled_far(l_c, l_s.y, l_dist, l_d);
    return coax_p_wg_make_f4_f3f(coax_p_myrealm_panels(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_wg_mul_f3f(coax_p_myrealm_encoded(l_c), coax_at_u(i32(3i)))), 0x1.000000p0f);
  }
  l_lit = coax_p_myrealm_daylight(l_n);
  l_lit = coax_p_wg_add_f3f3(l_lit, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.3d70a4p-1f, 0x1.1eb852p-2f), coax_p_myrealm_lamp_near(l_s.y, l_n)), 0x1.99999ap0f), coax_at_u(i32(55i))));
  l_lit = coax_p_wg_add_f3f3(l_lit, coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_myrealm_lanterns_near(l_s.y, l_n), 0x1.666666p0f), coax_p_wg_make_f3_f(coax_p_fmax(l_s.f6, 0.0f))));
  l_far = coax_p_fmax((coax_p_wg_length3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))))) - coax_at_u(i32(39i))), 0.0f);
  l_haze = coax_p_clampf((0x1.000000p0f - exp(((-l_far) * 0x1.26e978p-6f))), 0.0f, 0x1.000000p0f);
  l_thick = select(0x1.b33334p-1f, 0x1.99999ap-2f, ((coax_p_myrealm_land() > 0x1.000000p-1f) && (coax_p_myrealm_land() < 0x1.800000p0f)));
  l_c_20 = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f3(coax_p_myrealm_paletted(coax_p_v3(l_tex.x, l_tex.y, l_tex.z)), l_s.f4), l_lit), coax_p_v3(coax_at_u(i32(48i)), coax_at_u(i32(49i)), coax_at_u(i32(50i))), (l_haze * l_thick)), coax_at_u(i32(3i)));
  return coax_p_wg_make_f4_f3f(coax_p_myrealm_panels(coax_p_v2(l_s.x.x, l_s.x.y), l_c_20), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t17) -> vec4f {
  var l_v: coax_t17 = p_v;
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
fn coax_p_myrealm_round_vertex(p_m: coax_t11, p_id: u32) -> coax_t14 {
  var l_m: coax_t11 = p_m;
  var l_id: u32 = p_id;
  var l_q: u32;
  var l_c: vec2f;
  var l_row: u32;
  var l_k: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_axis: vec3f;
  var l_side: vec3f;
  var l_other: vec3f;
  var l_out: vec3f;
  var l_top: bool;
  var l_step: f32;
  var l_phi: f32;
  var l_n: vec3f;
  var l_r: f32;
  l_q = (l_id / 6u);
  l_c = coax_p_myrealm_quad((l_id % 6u));
  l_row = ((l_q / 12u) + coax_p_wg_u32_f(l_c.y));
  l_k = ((f32(((l_q % 12u) + coax_p_wg_u32_f(l_c.x))) / f32(12u)) * 0x1.921fb6p2f);
  l_a = coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z);
  l_b = coax_p_v3(l_m.y.x, l_m.y.y, l_m.y.z);
  l_axis = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_sub_f3f3(l_b, l_a), coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  l_side = coax_p_wg_normalize3(coax_p_wg_cross3(l_axis, select(coax_p_v3(0x1.000000p0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (abs(l_axis.y) < 0x1.ccccccp-1f))));
  l_other = coax_p_wg_cross3(l_axis, l_side);
  l_out = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_side, cos(l_k)), coax_p_wg_mul_f3f(l_other, sin(l_k)));
  l_top = (l_row > 3u);
  l_step = ((f32(select(l_row, ((l_row - 3u) - 1u), l_top)) / f32(3u)) * 0x1.921fb4p0f);
  l_phi = select((l_step - 0x1.921fb4p0f), l_step, l_top);
  l_n = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_out, cos(l_phi)), coax_p_wg_mul_f3f(l_axis, sin(l_phi)));
  l_r = select(l_m.x.w, l_m.y.w, l_top);
  return coax_t14(coax_p_wg_add_f3f3(select(l_a, l_b, l_top), coax_p_wg_mul_f3f(l_n, l_r)), l_n);
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return dot(l_a, l_b);
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
fn coax_p_myrealm_word(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(vertices[i32(l_i)]);
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
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
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
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
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_myrealm_styled() -> bool {
  return (coax_at_u(i32(449i)) > 0x1.000000p-1f);
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_myrealm_sky_styled(p_d: vec3f, p_p: vec2f) -> vec3f {
  var l_d: vec3f = p_d;
  var l_p: vec2f = p_p;
  var l_up: f32;
  var l_l: vec3f;
  var l_ct: f32;
  var l_moon: bool;
  var l_c: vec3f;
  var l_c_8: vec3f;
  var l_disc: f32;
  var l_halo: f32;
  var l_cl: vec4f;
  var l_c2: vec4f;
  var l_t: f32;
  var l_q: vec2f;
  var l_n: f32;
  var l_cover: f32;
  var l_toward: vec2f;
  var l_lit: f32;
  var l_night: f32;
  var l_cloud: vec3f;
  l_up = l_d.y;
  l_l = coax_p_myrealm_key_l();
  l_ct = coax_p_wg_dot3(l_d, l_l);
  l_moon = ((coax_p_myrealm_st(1u)).w > 0x1.000000p-1f);
  if (coax_p_myrealm_flat_style())
  {
    l_c = coax_p_wg_mix_f3f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(12u)), coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(13u)), coax_p_verse_style_flat_sky(l_up));
    if ((l_ct > cos((coax_p_myrealm_st(14u)).x)))
    {
      l_c = coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(17u)), select(0x1.19999ap0f, 0x1.b33334p-1f, l_moon));
    }
    return coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(17u)), coax_p_myrealm_stars(l_p)), (coax_p_myrealm_st(14u)).w), coax_p_smoothstep(0.0f, 0x1.99999ap-3f, l_up)));
  }
  l_c_8 = coax_p_wg_mix_f3f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(13u)), coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(12u)), coax_p_verse_style_sky_t(l_up));
  l_c_8 = coax_p_wg_mix_f3f3f(l_c_8, coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(10u)), coax_p_verse_style_sky_band(l_up, (coax_p_myrealm_st(12u)).w));
  l_disc = coax_p_smoothstep(cos(((coax_p_myrealm_st(14u)).x * 0x1.266666p0f)), cos(((coax_p_myrealm_st(14u)).x * 0x1.b33334p-1f)), l_ct);
  l_halo = ((coax_p_verse_style_halo(l_ct, (coax_p_myrealm_st(14u)).y) * 0x1.000000p-1f) + (coax_p_verse_style_halo(l_ct, (coax_p_myrealm_st(14u)).z) * 0x1.000000p-2f));
  if (l_moon)
  {
    l_c_8 = coax_p_wg_add_f3f3(l_c_8, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(0u)), l_halo), 0x1.000000p-1f));
  }
  else
  {
    l_c_8 = coax_p_wg_mix_f3f3f(l_c_8, coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(11u)), 0x1.800000p0f), coax_p_clampf((l_halo * 0x1.19999ap0f), 0.0f, 0x1.ccccccp-1f));
  }
  l_c_8 = coax_p_wg_mix_f3f3f(l_c_8, select(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.c28f5cp-1f), 0x1.000000p2f), coax_p_wg_mul_f3f(coax_p_v3(0x1.b33334p-1f, 0x1.c28f5cp-1f, 0x1.000000p0f), 0x1.99999ap0f), l_moon), l_disc);
  l_cl = coax_p_myrealm_st(15u);
  l_c2 = coax_p_myrealm_st(16u);
  if (((l_up > 0x1.47ae14p-7f) && (l_cl.x > 0.0f)))
  {
    l_t = select(coax_at_u(i32(2i)), 0.0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
    l_q = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), l_up), l_cl.y), coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_sw_xy_f4(coax_p_myrealm_st(25u)), l_cl.z), l_t));
    l_n = coax_p_verse_style_cloud_field(l_q.x, l_q.y, l_c2.x, l_c2.y, l_cl.w);
    l_cover = coax_p_verse_style_cloud_cover(l_n, l_cl.x, l_c2.z, l_c2.w);
    l_toward = coax_p_wg_mul_f2f(coax_p_wg_normalize2(coax_p_wg_add_f2f2(coax_p_v2(l_l.x, l_l.z), coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f))), 0x1.040000p8f);
    l_lit = coax_p_verse_style_cloud_lit(l_n, coax_p_verse_style_cloud_field((l_q.x + l_toward.x), (l_q.y + l_toward.y), l_c2.x, l_c2.y, l_cl.w));
    l_night = (coax_p_myrealm_st(14u)).w;
    l_cloud = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(4u)), (coax_p_myrealm_st(17u)).w), 0x1.ccccccp-1f), coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(17u)), l_lit), coax_p_fmax((coax_p_myrealm_st(0u)).w, 0x1.333334p-2f)), (0x1.000000p0f - (0x1.800000p-1f * l_night)));
    l_c_8 = coax_p_wg_mix_f3f3f(l_c_8, l_cloud, ((l_cover * coax_p_verse_style_cloud_fade(l_up)) * (0x1.000000p0f - (0x1.000000p-1f * l_night))));
  }
  return coax_p_wg_add_f3f3(l_c_8, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_make_f3_f(coax_p_myrealm_stars(l_p)), (coax_p_myrealm_st(14u)).w), coax_p_smoothstep(0.0f, 0x1.000000p-2f, l_up)));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_myrealm_st(p_i: u32) -> vec4f {
  var l_i: u32 = p_i;
  return coax_p_wg_at4(((i32(l_i) * 4i) + 324i));
}
fn coax_p_wg_sw_xyz_f4(p_p: vec4f) -> vec3f {
  var l_p: vec4f = p_p;
  return coax_p_v3(l_p.x, l_p.y, l_p.z);
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_myrealm_styled_ground(p_hit: vec3f, p_ray: vec3f, p_t: f32, p_p: vec2f) -> vec3f {
  var l_hit: vec3f = p_hit;
  var l_ray: vec3f = p_ray;
  var l_t: f32 = p_t;
  var l_p: vec2f = p_p;
  var l_up: vec3f;
  var l_d: vec3f;
  var l_a: vec3f;
  var l_big: f32;
  var l_brush: f32;
  var l_c: vec3f;
  var l_wi: f32;
  var l_depth: f32;
  var l_sh: vec4f;
  var l_md: vec4f;
  var l_dp: vec4f;
  var l_soft: f32;
  var l_w: vec3f;
  var l_rip: f32;
  var l_f: f32;
  var l_sea: vec3f;
  l_up = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_d = coax_p_wg_normalize3(l_ray);
  l_a = coax_p_wg_pow_f3f3(coax_p_v3(coax_at_u(i32(52i)), coax_at_u(i32(53i)), coax_at_u(i32(54i))), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f));
  if (coax_p_myrealm_flat_style())
  {
    l_a = coax_p_wg_mul_f3f(l_a, select(0x1.000000p0f, 0x1.c28f5cp-1f, (coax_p_myrealm_noise(coax_p_wg_mul_f2f(coax_p_v2(l_hit.x, l_hit.z), 0x1.1eb852p-4f)) > 0x1.28f5c2p-1f)));
  }
  else
  {
    l_big = coax_p_myrealm_noise(coax_p_wg_mul_f2f(coax_p_v2(l_hit.x, l_hit.z), 0x1.70a3d8p-3f));
    l_brush = coax_p_myrealm_noise(coax_p_v2(((l_hit.x * 0x1.ccccccp-1f) + (l_hit.z * 0x1.99999ap-2f)), (l_hit.z * 0x1.99999ap1f)));
    l_a = coax_p_wg_mul_f3f(l_a, ((0x1.99999ap-1f + (0x1.70a3d8p-3f * l_big)) + (0x1.eb851ep-4f * l_brush)));
  }
  l_c = select(coax_p_myrealm_painted(l_a, l_up, l_d), coax_p_myrealm_flat_lit(l_a, l_up), coax_p_myrealm_flat_style());
  l_wi = coax_p_myrealm_water_in(coax_p_v2(l_hit.x, l_hit.z));
  if (((coax_at_u(i32(63i)) > 0x1.000000p-1f) && (l_wi > 0.0f)))
  {
    l_depth = (l_wi * 0x1.000000p-1f);
    l_sh = coax_p_myrealm_st(27u);
    l_md = coax_p_myrealm_st(28u);
    l_dp = coax_p_myrealm_st(29u);
    l_soft = coax_p_fmax(l_dp.w, 0x1.47ae14p-7f);
    l_w = coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(coax_p_v3(l_sh.x, l_sh.y, l_sh.z), coax_p_v3(l_md.x, l_md.y, l_md.z), coax_p_smoothstep((l_sh.w - l_soft), (l_sh.w + l_soft), l_depth)), coax_p_v3(l_dp.x, l_dp.y, l_dp.z), coax_p_smoothstep((l_md.w - l_soft), (l_md.w + l_soft), l_depth));
    if (coax_p_myrealm_flat_style())
    {
      l_w = coax_p_myrealm_flat_lit(select(coax_p_v3(l_md.x, l_md.y, l_md.z), coax_p_v3(l_sh.x, l_sh.y, l_sh.z), (l_depth < l_sh.w)), l_up);
      l_w = coax_p_wg_mix_f3f3f(l_w, coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(17u)), (0x1.000000p0f - coax_p_stepf((coax_p_myrealm_st(30u)).w, l_wi)));
    }
    else
    {
      l_rip = (coax_p_myrealm_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_hit.x, l_hit.z), 0x1.333334p1f), coax_p_v2((coax_at_u(i32(2i)) * 0x1.333334p-2f), (coax_at_u(i32(2i)) * 0x1.99999ap-3f)))) - 0x1.000000p-1f);
      l_w = coax_p_myrealm_painted(l_w, l_up, l_d);
      l_f = (0x1.47ae14p-6f + (0x1.f5c290p-1f * coax_pow_bg_vs((0x1.000000p0f - coax_p_fmax((-l_d.y), 0.0f)), 0x1.400000p2f)));
      l_w = coax_p_wg_mix_f3f3f(l_w, coax_p_myrealm_sky_styled(coax_p_v3((l_d.x + (l_rip * 0x1.47ae14p-5f)), (-l_d.y), (l_d.z + (l_rip * 0x1.47ae14p-5f))), l_p), coax_p_clampf((l_f + 0x1.333334p-3f), 0.0f, 0x1.000000p0f));
    }
    if (((coax_at_u(i32(63i)) > 0x1.400000p1f) && (coax_at_u(i32(63i)) < 0x1.c00000p1f)))
    {
      l_w = select(coax_p_myrealm_painted(coax_p_wg_pow_f3f3(coax_p_myrealm_ice_at(coax_p_v2(l_hit.x, l_hit.z)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f)), l_up, l_d), coax_p_myrealm_flat_lit(coax_p_wg_pow_f3f3(coax_p_myrealm_ice_at(coax_p_v2(l_hit.x, l_hit.z)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f)), l_up), coax_p_myrealm_flat_style());
    }
    if ((coax_at_u(i32(63i)) > 0x1.c00000p1f))
    {
      l_w = coax_p_wg_mul_f3f(coax_p_wg_pow_f3f3(coax_p_myrealm_lava_at(coax_p_v2(l_hit.x, l_hit.z)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f)), 0x1.99999ap0f);
    }
    l_c = l_w;
  }
  if (((coax_p_myrealm_land() > 0x1.800000p0f) && (coax_p_wg_length2(coax_p_v2(l_hit.x, l_hit.z)) > (coax_at_u(i32(31i)) - 0x1.800000p1f))))
  {
    l_sea = select(coax_p_myrealm_painted(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(28u)), l_up, l_d), coax_p_myrealm_flat_lit(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(28u)), l_up), coax_p_myrealm_flat_style());
    l_c = coax_p_wg_mix_f3f3f(l_c, l_sea, coax_p_smoothstep((coax_at_u(i32(31i)) - 0x1.800000p1f), (coax_at_u(i32(31i)) - 0x1.000000p1f), coax_p_wg_length2(coax_p_v2(l_hit.x, l_hit.z))));
  }
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_myrealm_styled_lights(l_a, l_hit, l_up));
  return coax_p_myrealm_styled_far(l_c, l_hit, l_t, l_d);
}
fn coax_p_myrealm_land() -> f32 {
  return floor(((coax_at_u(i32(7i)) / 0x1.400000p3f) + 0x1.0624dep-10f));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_myrealm_encoded(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_wg_pow_f3f3(coax_p_wg_clamp_f3f3f3(coax_p_myrealm_tone(l_c), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), coax_p_wg_make_f3_f((0x1.000000p0f / 0x1.19999ap1f)));
}
fn coax_p_myrealm_panels(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_k: i32;
  l_c = l_under;
  {
    l_k = 0i;
    while ((l_k < 24i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_c = coax_p_myrealm_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 64i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 64i)));
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
fn coax_p_myrealm_sky_of(p_ray: vec3f, p_p: vec2f) -> vec3f {
  var l_ray: vec3f = p_ray;
  var l_p: vec2f = p_p;
  var l_r: vec3f;
  var l_c: vec3f;
  var l_toward: f32;
  var l_q: vec2f;
  var l_n: f32;
  var l_cover: f32;
  l_r = coax_p_wg_normalize3(l_ray);
  l_c = coax_p_wg_mix_f3f3f(coax_p_v3(coax_at_u(i32(48i)), coax_at_u(i32(49i)), coax_at_u(i32(50i))), coax_p_v3(coax_at_u(i32(44i)), coax_at_u(i32(45i)), coax_at_u(i32(46i))), coax_p_smoothstep((-0x1.47ae14p-6f), 0x1.ccccccp-2f, l_r.y));
  l_toward = coax_p_fmax(coax_p_wg_dot3(l_r, coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(56i)), coax_at_u(i32(57i)), coax_at_u(i32(58i))))), 0.0f);
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.b33334p-1f, 0x1.333334p-1f), coax_pow_bg_vs(l_toward, 0x1.400000p5f)), coax_at_u(i32(59i))), 0x1.333334p-1f), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(48i)), coax_at_u(i32(49i)), coax_at_u(i32(50i))), coax_pow_bg_vs(l_toward, 0x1.800000p2f)), 0x1.000000p-2f)));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_make_f3_f(coax_p_myrealm_stars(l_p)), coax_at_u(i32(51i))), coax_p_smoothstep(0.0f, 0x1.000000p-2f, l_r.y)));
  if ((l_r.y > 0.0f))
  {
    l_q = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2(l_r.x, l_r.z), (l_r.y + 0x1.eb851ep-4f)), 0x1.666666p0f), coax_p_v2((coax_at_u(i32(2i)) * 0x1.eb851ep-7f), 0.0f));
    l_n = (((coax_p_myrealm_noise(l_q) * 0x1.333334p-1f) + (coax_p_myrealm_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.266666p1f), coax_p_v2(0x1.c00000p2f, 0x1.c00000p2f))) * 0x1.333334p-2f)) + (coax_p_myrealm_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.466666p2f), coax_p_v2(0x1.800000p1f, 0x1.800000p1f))) * 0x1.99999ap-4f));
    l_cover = ((coax_p_smoothstep(0x1.000000p-1f, 0x1.8f5c28p-1f, l_n) * coax_p_smoothstep(0x1.47ae14p-6f, 0x1.333334p-2f, l_r.y)) * (0x1.000000p0f - coax_at_u(i32(51i))));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mix_f3f3f(coax_p_v3(coax_at_u(i32(48i)), coax_at_u(i32(49i)), coax_at_u(i32(50i))), coax_p_wg_mul_f3f(coax_p_myrealm_sun_tint(), 0x1.0cccccp0f), 0x1.333334p-1f), (l_cover * 0x1.b33334p-1f));
  }
  return l_c;
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_myrealm_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_w: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_w = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_myrealm_hash(l_i), coax_p_myrealm_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_w.x), coax_p_mix(coax_p_myrealm_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_myrealm_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_w.x), l_w.y);
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_myrealm_daylight(p_n: vec3f) -> vec3f {
  var l_n: vec3f = p_n;
  var l_key: f32;
  var l_soft: f32;
  var l_amb: vec3f;
  l_key = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(56i)), coax_at_u(i32(57i)), coax_at_u(i32(58i))))));
  l_soft = select(l_key, ((l_key * 0x1.333334p-1f) + 0x1.000000p-2f), (coax_at_u(i32(47i)) > 0x1.000000p-1f));
  l_amb = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(coax_at_u(i32(48i)), coax_at_u(i32(49i)), coax_at_u(i32(50i))), coax_p_v3(coax_at_u(i32(44i)), coax_at_u(i32(45i)), coax_at_u(i32(46i))), (0x1.000000p-1f + (0x1.000000p-1f * l_n.y))), 0x1.19999ap-1f), coax_p_v3(0x1.47ae14p-5f, 0x1.47ae14p-5f, 0x1.47ae14p-5f));
  return coax_p_wg_add_f3f3(l_amb, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_myrealm_sun_tint(), l_soft), coax_at_u(i32(59i))));
}
fn coax_p_myrealm_water_in(p_q: vec2f) -> f32 {
  var l_q: vec2f = p_q;
  if (((coax_at_u(i32(63i)) > 0x1.800000p0f) && (coax_at_u(i32(63i)) < 0x1.400000p1f)))
  {
    return (coax_at_u(i32(62i)) - abs(((l_q.y - coax_at_u(i32(61i))) - (0x1.400000p1f * sin((l_q.x * 0x1.333334p-3f))))));
  }
  return (coax_at_u(i32(62i)) - coax_p_wg_length2(coax_p_wg_sub_f2f2(l_q, coax_p_v2(coax_at_u(i32(60i)), coax_at_u(i32(61i))))));
}
fn coax_p_myrealm_ice_at(p_q: vec2f) -> vec3f {
  var l_q: vec2f = p_q;
  var l_crack: f32;
  l_crack = (coax_p_smoothstep(0x1.e147aep-2f, 0x1.000000p-1f, coax_p_myrealm_noise(coax_p_wg_mul_f2f(l_q, 0x1.4cccccp0f))) * (0x1.000000p0f - coax_p_smoothstep(0x1.000000p-1f, 0x1.0f5c28p-1f, coax_p_myrealm_noise(coax_p_wg_mul_f2f(l_q, 0x1.4cccccp0f)))));
  return coax_p_wg_mix_f3f3f(coax_p_v3(0x1.99999ap-1f, 0x1.c28f5cp-1f, 0x1.e147aep-1f), coax_p_v3(0x1.19999ap-1f, 0x1.51eb86p-1f, 0x1.800000p-1f), ((l_crack * 0x1.99999ap-1f) + (0x1.333334p-3f * coax_p_myrealm_noise(coax_p_wg_mul_f2f(l_q, 0x1.99999ap-2f)))));
}
fn coax_p_myrealm_lava_at(p_q: vec2f) -> vec3f {
  var l_q: vec2f = p_q;
  var l_flow: f32;
  var l_crust: f32;
  l_flow = coax_p_myrealm_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.ccccccp-1f), coax_p_v2((coax_at_u(i32(2i)) * 0x1.333334p-3f), (coax_at_u(i32(2i)) * 0x1.1eb852p-4f))));
  l_crust = coax_p_smoothstep(0x1.19999ap-1f, 0x1.666666p-1f, coax_p_myrealm_noise(coax_p_wg_sub_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.b33334p0f), coax_p_v2((coax_at_u(i32(2i)) * 0x1.99999ap-5f), 0.0f))));
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(coax_p_v3(0x1.000000p0f, 0x1.333334p-2f, 0x1.47ae14p-5f), coax_p_v3(0x1.000000p0f, 0x1.8f5c28p-1f, 0x1.000000p-2f), l_flow), coax_p_v3(0x1.eb851ep-4f, 0x1.99999ap-5f, 0x1.47ae14p-5f), (l_crust * 0x1.b33334p-1f));
}
fn coax_p_myrealm_lamp_near(p_p: vec3f, p_n: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_to: vec3f;
  var l_d: f32;
  l_to = coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_p);
  l_d = coax_p_wg_length3(l_to);
  return ((coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_wg_div_f3f(l_to, coax_p_fmax(l_d, 0x1.0624dep-10f)))) * coax_at_u(i32(27i))) / (0x1.000000p0f + ((0x1.70a3d8p-3f * l_d) * l_d)));
}
fn coax_p_myrealm_lanterns_near(p_p: vec3f, p_n: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_sum: f32;
  var l_i: i32;
  var l_l: vec4f;
  var l_to: vec3f;
  var l_d: f32;
  if ((coax_at_u(i32(323i)) <= 0.0f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_sum = 0.0f;
  {
    l_i = 0i;
    while ((l_i < 16i))
    {
      l_l = coax_p_wg_at4(((i32(l_i) * 4i) + 256i));
      if ((l_l.w < 0x1.000000p-1f))
      {
        break;
      }
      l_to = coax_p_wg_sub_f3f3(coax_p_v3(l_l.x, l_l.y, l_l.z), l_p);
      l_d = coax_p_wg_length3(l_to);
      l_sum = (l_sum + ((0x1.000000p-2f + (0x1.800000p-1f * coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_wg_div_f3f(l_to, coax_p_fmax(l_d, 0x1.0624dep-10f)))))) / (0x1.000000p0f + ((0x1.19999ap-1f * l_d) * l_d))));
      l_i = (l_i + 1i);
    }
  }
  return coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(320i)), coax_at_u(i32(321i)), coax_at_u(i32(322i))), l_sum), coax_at_u(i32(323i)));
}
fn coax_p_myrealm_sea_at(p_q: vec2f, p_mirror: vec3f) -> vec3f {
  var l_q: vec2f = p_q;
  var l_mirror: vec3f = p_mirror;
  return coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_mirror, 0x1.ccccccp-2f), coax_p_wg_mul_f3f(coax_p_v3(0x1.eb851ep-6f, 0x1.47ae14p-3f, 0x1.0a3d70p-2f), (0x1.b33334p-1f + (0x1.333334p-2f * coax_p_myrealm_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.000000p-1f), coax_p_v2((coax_at_u(i32(2i)) * 0x1.99999ap-4f), 0.0f)))))));
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
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_myrealm_surfaced(p_t: vec3f, p_w: f32) -> vec3f {
  var l_t: vec3f = p_t;
  var l_w: f32 = p_w;
  return coax_p_wg_mix_f3f3f(l_t, coax_p_wg_make_f3_f((0x1.ae147ap-2f + (0x1.99999ap-1f * coax_p_wg_dot3(l_t, coax_p_v3(0x1.333334p-2f, 0x1.2e147ap-1f, 0x1.c28f5cp-4f))))), coax_p_clampf((-l_w), 0.0f, 0x1.000000p0f));
}
fn coax_p_myrealm_masonry(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_row: f32;
  var l_along: f32;
  var l_fu: f32;
  var l_fv: f32;
  var l_joint: f32;
  var l_block: f32;
  l_row = floor((l_p.y / 0x1.000000p-1f));
  l_along = (((l_p.x + (l_p.z * 0x1.a8f5c2p-1f)) / 0x1.0cccccp0f) + (0x1.000000p-1f * (l_row - (0x1.000000p1f * floor((l_row * 0x1.000000p-1f))))));
  l_fu = coax_p_fract(l_along);
  l_fv = coax_p_fract((l_p.y / 0x1.000000p-1f));
  l_joint = coax_p_fmin((coax_p_fmin(l_fu, (0x1.000000p0f - l_fu)) * 0x1.0cccccp0f), (coax_p_fmin(l_fv, (0x1.000000p0f - l_fv)) * 0x1.000000p-1f));
  l_block = ((0x1.8f5c28p-1f + (0x1.99999ap-3f * coax_p_myrealm_hash(coax_p_v2(floor(l_along), l_row)))) + (0x1.eb851ep-5f * coax_p_myrealm_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_p.x, l_p.z), 0x1.800000p1f), coax_p_wg_mul_f2f(coax_p_v2(l_p.y, l_p.y), 0x1.000000p1f)))));
  return coax_p_mix(0x1.ae147ap-2f, l_block, coax_p_smoothstep(0x1.89374cp-7f, 0x1.1eb852p-5f, l_joint));
}
fn coax_p_wg_make_f3_f(p_p0: f32) -> vec3f {
  var l_p0: f32 = p_p0;
  return coax_p_v3(l_p0, l_p0, l_p0);
}
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_myrealm_flat_style() -> bool {
  return (coax_at_u(i32(449i)) > 0x1.400000p1f);
}
fn coax_p_myrealm_paletted(p_t: vec3f) -> vec3f {
  var l_t: vec3f = p_t;
  return select(l_t, coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_t, 0x1.666666p-2f), coax_p_v3(0x1.3d70a4p-1f, 0x1.3d70a4p-1f, 0x1.3d70a4p-1f)), ((coax_p_myrealm_pal() > 0x1.000000p-1f) && (coax_p_myrealm_pal() < 0x1.800000p0f)));
}
fn coax_p_myrealm_flat_lit(p_a: vec3f, p_n: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_n: vec3f = p_n;
  var l_band: f32;
  l_band = coax_p_verse_style_flat_band(coax_p_verse_style_flat_light(coax_p_wg_dot3(l_n, coax_p_myrealm_key_l()), 0x1.000000p0f));
  return coax_p_wg_mul_f3f3(l_a, coax_p_wg_mix_f3f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(4u)), coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(0u)), (coax_p_myrealm_st(0u)).w), l_band));
}
fn coax_p_myrealm_painted(p_a: vec3f, p_n: vec3f, p_d: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_n: vec3f = p_n;
  var l_d: vec3f = p_d;
  var l_l: vec3f;
  var l_ndl: f32;
  var l_band: f32;
  var l_amb: vec3f;
  var l_s: vec3f;
  var l_t: vec3f;
  var l_tl: f32;
  var l_k: f32;
  var l_m: vec3f;
  var l_y: f32;
  var l_ym: f32;
  var l_cool: vec3f;
  var l_lit: vec3f;
  var l_c: vec3f;
  l_l = coax_p_myrealm_key_l();
  l_ndl = coax_p_wg_dot3(l_n, l_l);
  l_band = coax_p_verse_style_bands(coax_p_verse_style_wrap(l_ndl), (coax_p_myrealm_st(7u)).x, (coax_p_myrealm_st(7u)).y, (coax_p_myrealm_st(7u)).z, (coax_p_myrealm_st(7u)).w);
  l_amb = coax_p_v3(coax_p_verse_style_ambient(l_n.y, (coax_p_myrealm_st(5u)).x, (coax_p_myrealm_st(6u)).x, (coax_p_myrealm_st(5u)).w), coax_p_verse_style_ambient(l_n.y, (coax_p_myrealm_st(5u)).y, (coax_p_myrealm_st(6u)).y, (coax_p_myrealm_st(5u)).w), coax_p_verse_style_ambient(l_n.y, (coax_p_myrealm_st(5u)).z, (coax_p_myrealm_st(6u)).z, (coax_p_myrealm_st(5u)).w));
  l_s = coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f3(l_a, coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(4u))), l_amb);
  l_t = coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(4u));
  l_tl = coax_p_verse_style_luma(l_t.x, l_t.y, l_t.z);
  l_k = (coax_p_myrealm_st(21u)).w;
  l_m = coax_p_v3(coax_p_verse_style_cool(l_s.x, l_t.x, l_tl, l_k), coax_p_verse_style_cool(l_s.y, l_t.y, l_tl, l_k), coax_p_verse_style_cool(l_s.z, l_t.z, l_tl, l_k));
  l_y = coax_p_verse_style_luma(l_s.x, l_s.y, l_s.z);
  l_ym = coax_p_verse_style_luma(l_m.x, l_m.y, l_m.z);
  l_cool = coax_p_v3(coax_p_verse_style_keep_value(l_m.x, l_y, l_ym), coax_p_verse_style_keep_value(l_m.y, l_y, l_ym), coax_p_verse_style_keep_value(l_m.z, l_y, l_ym));
  l_lit = coax_p_wg_mul_f3f3(l_a, coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(0u)), (coax_p_myrealm_st(0u)).w), l_amb));
  l_c = coax_p_v3(coax_p_verse_style_banded(l_cool.x, l_lit.x, l_band), coax_p_verse_style_banded(l_cool.y, l_lit.y, l_band), coax_p_verse_style_banded(l_cool.z, l_lit.z, l_band));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(2u)), coax_p_verse_style_rim(coax_p_wg_dot3(l_n, coax_p_wg_neg_f3(l_d)), l_ndl, (coax_p_myrealm_st(3u)).y, (coax_p_myrealm_st(3u)).z, (coax_p_myrealm_st(2u)).w)));
  return l_c;
}
fn coax_p_myrealm_styled_lights(p_a: vec3f, p_p: vec3f, p_n: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_lamp: vec3f;
  var l_pools: vec3f;
  var l_w: vec3f;
  l_lamp = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.3d70a4p-1f, 0x1.1eb852p-2f), coax_p_myrealm_lamp_near(l_p, l_n)), 0x1.99999ap0f), coax_at_u(i32(55i)));
  l_pools = coax_p_wg_mul_f3f(coax_p_myrealm_lanterns_near(l_p, l_n), 0x1.666666p0f);
  if (coax_p_myrealm_flat_style())
  {
    l_w = coax_p_wg_add_f3f3(l_lamp, l_pools);
    return coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_a, 0x1.19999ap1f), coax_p_v3(0x1.47ae14p-5f, 0x1.47ae14p-5f, 0x1.47ae14p-5f)), coax_p_wg_floor_f3(coax_p_wg_clamp_f3f3f3(coax_p_wg_mul_f3f(l_w, 0x1.000000p1f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p1f, 0x1.000000p1f, 0x1.000000p1f)))), 0x1.000000p-1f);
  }
  return coax_p_wg_mul_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_a, 0x1.19999ap1f), coax_p_v3(0x1.47ae14p-5f, 0x1.47ae14p-5f, 0x1.47ae14p-5f)), coax_p_wg_add_f3f3(l_lamp, l_pools));
}
fn coax_p_myrealm_styled_far(p_c: vec3f, p_p: vec3f, p_dist: f32, p_d: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_p: vec3f = p_p;
  var l_dist: f32 = p_dist;
  var l_d: vec3f = p_d;
  var l_hz: vec4f;
  var l_f: f32;
  var l_hz3: vec3f;
  var l_hue: vec3f;
  var l_hc: vec3f;
  l_hz = coax_p_myrealm_st(9u);
  if (coax_p_myrealm_flat_style())
  {
    return coax_p_wg_mix_f3f3f(l_c, coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(13u)), coax_p_verse_style_flat_fade(coax_p_fmax((l_dist - coax_at_u(i32(39i))), 0.0f), l_hz.x, (0x1.000000p0f / coax_p_fmax(l_hz.y, 0x1.a36e2ep-14f)), l_hz.z));
  }
  l_f = coax_p_verse_style_haze(coax_p_fmax((l_dist - coax_at_u(i32(39i))), 0.0f), l_p.y, 0.0f, l_hz.x, l_hz.y, l_hz.z, l_hz.w);
  l_hz3 = coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(10u));
  l_hue = coax_p_v3(coax_p_verse_style_haze_hue(l_hz3.x, 0.0f, l_f, l_hz.z), coax_p_verse_style_haze_hue(l_hz3.y, 0x1.000000p0f, l_f, l_hz.z), coax_p_verse_style_haze_hue(l_hz3.z, 0x1.000000p1f, l_f, l_hz.z));
  l_hc = coax_p_wg_mix_f3f3f(l_hue, coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(11u)), coax_p_verse_style_haze_glow(coax_p_wg_dot3(l_d, coax_p_myrealm_key_l())));
  return coax_p_v3(coax_p_verse_style_fog(l_c.x, l_hc.x, l_f), coax_p_verse_style_fog(l_c.y, l_hc.y, l_f), coax_p_verse_style_fog(l_c.z, l_hc.z, l_f));
}
fn coax_p_myrealm_quad(p_v: u32) -> vec2f {
  var l_v: u32 = p_v;
  var l_q: array<vec2f, 6>;
  l_q = array<vec2f, 6>(coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0x1.000000p0f));
  return l_q[i32(l_v)];
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
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_myrealm_key_l() -> vec3f {
  return coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_sw_xyz_f4(coax_p_myrealm_st(1u)), coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
}
fn coax_p_verse_style_flat_sky(p_fl_up: f32) -> f32 {
  var l_fl_up: f32 = p_fl_up;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.eb851ep-4f, 0x1.eb851ep-4f, l_fl_up));
}
fn coax_p_myrealm_stars(p_p: vec2f) -> f32 {
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
  l_h = coax_p_myrealm_hash(l_g);
  if ((l_h < 0x1.dc28f6p-1f))
  {
    return 0.0f;
  }
  l_at = coax_p_wg_mul_f2f(coax_p_wg_add_f2f(coax_p_wg_add_f2f2(l_g, coax_p_wg_mul_f2f(coax_p_v2(coax_p_myrealm_hash(coax_p_wg_add_f2f(l_g, 0x1.b33334p0f)), coax_p_myrealm_hash(coax_p_wg_add_f2f(l_g, 0x1.233334p3f))), 0x1.99999ap-1f)), 0x1.99999ap-4f), l_cell);
  l_d = (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, l_at)) / coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f));
  l_size = select(0x1.19999ap-1f, 0x1.e66666p-1f, (l_h > 0x1.fbe76cp-1f));
  l_twinkle = select((0x1.800000p-1f + (0x1.000000p-2f * sin(((coax_at_u(i32(2i)) * (0x1.333334p-1f + (l_h * 0x1.000000p1f))) + (l_h * 0x1.400000p5f))))), 0x1.000000p0f, (coax_at_u(i32(4i)) > 0x1.000000p-1f));
  l_peak = (select(0x1.47ae14p-2f, 0x1.800000p-1f, (l_h > 0x1.fbe76cp-1f)) * l_twinkle);
  return (l_peak * (0x1.000000p0f - coax_p_smoothstep((l_size * 0x1.99999ap-2f), (l_size + 0x1.333334p-1f), l_d)));
}
fn coax_p_verse_style_sky_t(p_up: f32) -> f32 {
  var l_up: f32 = p_up;
  return coax_pow_bg_vs(coax_p_st_max(l_up, 0x1.0c6f7ap-20f), 0x1.19999ap-1f);
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
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
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
fn coax_p_wg_normalize2(p_a: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  return coax_p_scale2(l_a, (0x1.000000p0f / coax_p_wg_length2(l_a)));
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
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_wg_pow_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_pow_bg_vs(l_p0.x, l_p1.x), coax_pow_bg_vs(l_p0.y, l_p1.y), coax_pow_bg_vs(l_p0.z, l_p1.z));
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
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_myrealm_tone(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_e: f32;
  var l_t: vec3f;
  var l_y: f32;
  if (((coax_p_myrealm_st(18u)).w < 0x1.000000p-1f))
  {
    return l_c;
  }
  l_e = (coax_p_myrealm_st(18u)).x;
  l_t = coax_p_v3(coax_p_verse_style_tone(l_c.x, l_e), coax_p_verse_style_tone(l_c.y, l_e), coax_p_verse_style_tone(l_c.z, l_e));
  l_y = coax_p_verse_style_luma(l_t.x, l_t.y, l_t.z);
  l_t = coax_p_v3(coax_p_verse_style_saturation(l_t.x, l_y, (coax_p_myrealm_st(18u)).y), coax_p_verse_style_saturation(l_t.y, l_y, (coax_p_myrealm_st(18u)).y), coax_p_verse_style_saturation(l_t.z, l_y, (coax_p_myrealm_st(18u)).y));
  return coax_p_v3(coax_p_verse_style_lift(l_t.x, (coax_p_myrealm_st(18u)).z), coax_p_verse_style_lift(l_t.y, (coax_p_myrealm_st(18u)).z), coax_p_verse_style_lift(l_t.z, (coax_p_myrealm_st(18u)).z));
}
fn coax_p_wg_clamp_f3f3f3(p_p0: vec3f, p_p1: vec3f, p_p2: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: vec3f = p_p2;
  return coax_p_v3(coax_p_clampf(l_p0.x, l_p1.x, l_p2.x), coax_p_clampf(l_p0.y, l_p1.y, l_p2.y), coax_p_clampf(l_p0.z, l_p1.z, l_p2.z));
}
fn coax_p_myrealm_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
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
fn coax_p_myrealm_sun_tint() -> vec3f {
  var l_k: f32;
  l_k = coax_p_myrealm_pal();
  if (((l_k > 0x1.000000p-1f) && (l_k < 0x1.800000p0f)))
  {
    return coax_p_v3(0x1.000000p0f, 0x1.c28f5cp-1f, 0x1.d70a3ep-1f);
  }
  if (((l_k > 0x1.800000p0f) && (l_k < 0x1.400000p1f)))
  {
    return coax_p_v3(0x1.19999ap0f, 0x1.e66666p-1f, 0x1.70a3d8p-1f);
  }
  if ((l_k > 0x1.400000p1f))
  {
    return coax_p_v3(0x1.ae147ap-1f, 0x1.b851ecp-1f, 0x1.ccccccp-1f);
  }
  return coax_p_v3(0x1.000000p0f, 0x1.e147aep-1f, 0x1.a3d70ap-1f);
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
fn coax_p_myrealm_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_myrealm_pal() -> f32 {
  return (coax_at_u(i32(7i)) - (0x1.400000p3f * coax_p_myrealm_land()));
}
fn coax_p_verse_style_flat_light(p_fl_ndl: f32, p_fl_lit: f32) -> f32 {
  var l_fl_ndl: f32 = p_fl_ndl;
  var l_fl_lit: f32 = p_fl_lit;
  return (coax_p_st_sat(((l_fl_ndl * 0x1.000000p-1f) + 0x1.000000p-1f)) * coax_p_st_sat(l_fl_lit));
}
fn coax_p_verse_style_flat_band(p_fl_w: f32) -> f32 {
  var l_fl_w: f32 = p_fl_w;
  return coax_p_verse_style_bands(l_fl_w, 0x1.851eb8p-2f, 0x1.3d70a4p-1f, 0x1.47ae14p-7f, 0x1.000000p-1f);
}
fn coax_p_verse_style_wrap(p_ndl: f32) -> f32 {
  var l_ndl: f32 = p_ndl;
  return coax_p_st_sat(((l_ndl + 0x1.000000p-2f) / 0x1.400000p0f));
}
fn coax_p_verse_style_bands(p_w: f32, p_e0: f32, p_e1: f32, p_s: f32, p_mid: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_e0: f32 = p_e0;
  var l_e1: f32 = p_e1;
  var l_s: f32 = p_s;
  var l_mid: f32 = p_mid;
  return ((l_mid * coax_p_verse_style_soft_step(l_e0, l_s, l_w)) + ((0x1.000000p0f - l_mid) * coax_p_verse_style_soft_step(l_e1, l_s, l_w)));
}
fn coax_p_verse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_sky: f32 = p_sky;
  var l_ground: f32 = p_ground;
  var l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
}
fn coax_p_verse_style_luma(p_r: f32, p_g: f32, p_b: f32) -> f32 {
  var l_r: f32 = p_r;
  var l_g: f32 = p_g;
  var l_b: f32 = p_b;
  return (((0x1.b367a0p-3f * l_r) + (0x1.6e2eb2p-1f * l_g)) + (0x1.27bb30p-4f * l_b));
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
fn coax_p_wg_floor_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(floor(l_p0.x), floor(l_p0.y), floor(l_p0.z));
}
fn coax_p_verse_style_flat_fade(p_fl_d: f32, p_fl_start: f32, p_fl_reach: f32, p_fl_cap: f32) -> f32 {
  var l_fl_d: f32 = p_fl_d;
  var l_fl_start: f32 = p_fl_start;
  var l_fl_reach: f32 = p_fl_reach;
  var l_fl_cap: f32 = p_fl_cap;
  return (l_fl_cap * coax_p_verse_style_soft_step((l_fl_start + (l_fl_reach * 0x1.000000p-1f)), coax_p_st_max((l_fl_reach * 0x1.000000p-1f), 0x1.0624dep-10f), l_fl_d));
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
fn coax_p_verse_style_fog(p_c: f32, p_haze_c: f32, p_f: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_haze_c: f32 = p_haze_c;
  var l_f: f32 = p_f;
  return (l_c + ((l_haze_c - l_c) * coax_p_st_sat(l_f)));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
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
fn coax_p_verse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
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
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
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
fn coax_p_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
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
fn coax_p_wg_abs_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(abs(l_p0.x), abs(l_p0.y));
}
fn coax_p_wg_max_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v2(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
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
fn coax_p_verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_v: f32;
  l_v = (sin(((l_x * 0x1.fc6666p6f) + (l_y * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_16) -> coax_io_17 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_17(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_17) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t17(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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