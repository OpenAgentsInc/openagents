// ---- generated: apps/openagents-web/gpu/worldbuilder.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t14 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
  f5: vec4f,
}
struct coax_t17 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec3f,
  f5: u32,
  f6: vec3f,
  f7: vec3f,
  f8: u32,
}
struct coax_io_17 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec3f,
  @location(4) @interpolate(flat) f5: u32,
  @location(5) f6: vec3f,
  @location(6) f7: vec3f,
  @location(7) @interpolate(flat) f8: u32,
}
struct coax_t18 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
  f5: vec4f,
  f6: vec4f,
}
struct coax_in_18 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
  @location(4) f4: vec4f,
  @location(5) f5: vec4f,
  @location(6) f6: vec4f,
}
struct coax_t19 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec2f,
  f4: vec2f,
  f5: vec4f,
  f6: vec4f,
  f7: vec4f,
  f8: vec4f,
}
struct coax_io_19 {
  @builtin(position) x: vec4f,
  @location(0) @interpolate(flat) y: vec2f,
  @location(1) @interpolate(flat) z: vec4f,
  @location(2) @interpolate(flat) w: vec2f,
  @location(3) f4: vec2f,
  @location(4) @interpolate(flat) f5: vec4f,
  @location(5) @interpolate(flat) f6: vec4f,
  @location(6) @interpolate(flat) f7: vec4f,
  @location(7) @interpolate(flat) f8: vec4f,
}
struct coax_t23 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: u32,
  f5: vec3f,
  f6: vec4f,
  f7: vec3f,
  f8: u32,
}
struct coax_t25 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
  f5: vec4f,
  f6: vec4f,
  f7: vec4f,
  f8: vec4f,
  f9: vec4f,
  f10: vec4f,
  f11: vec4f,
}
struct coax_t26 {
  x: vec3f,
  y: vec3f,
}
struct coax_t27 {
  x: i32,
  y: i32,
}
struct coax_t28 {
  x: u32,
  y: u32,
}
struct coax_t30 {
  x: i32,
  y: f32,
  z: f32,
  w: f32,
  f4: array<f32, 3>,
}
struct coax_t33 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_t34 {
  x: bool,
  y: vec4f,
}
struct coax_t36 {
  x: bool,
  y: i32,
  z: f32,
  w: f32,
  f4: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 33>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> people: array<u32>;
@group(1) @binding(2) var<storage, read> objects: array<coax_t11>;
@group(1) @binding(3) var<storage, read> ground: array<vec4f>;
@group(1) @binding(4) var<storage, read> figures: array<coax_t14>;
@group(1) @binding(5) var<storage, read> bake: array<vec4f>;
@group(1) @binding(6) var sheets: texture_2d_array<f32>;
@group(1) @binding(7) var sheet: sampler;
@group(0) @binding(1) var gpui_atlas: texture_2d<f32>;
@group(0) @binding(2) var gpui_color: texture_2d<f32>;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t17 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_b: u32;
  var l_w0: u32;
  var l_w1: u32;
  var l_w2: u32;
  var l_w3: u32;
  var l_xy: vec2f;
  var l_p: vec4f;
  var l_ob: vec4f;
  var l_n: vec3f;
  var l_m: coax_t11;
  var l_w: vec3f;
  var l_wp: vec4f;
  var l_o: coax_t17;
  var l_c0: vec3f;
  var l_c1: vec3f;
  var l_c2: vec3f;
  var l_rgb: vec4f;
  if ((l_instance >= 1048576u))
  {
    return coax_p_worldbuilder_figure_vs(l_id, (l_instance - 1048576u));
  }
  if ((objects[i32(l_instance)].w.w > 0x1.000000p-1f))
  {
    return coax_p_worldbuilder_ground_vs(l_id);
  }
  l_b = (l_id * 4u);
  l_w0 = vertices[i32(l_b)];
  l_w1 = vertices[i32((l_b + 1u))];
  l_w2 = vertices[i32((l_b + 2u))];
  l_w3 = vertices[i32((l_b + 3u))];
  l_xy = coax_p_wg_unpack2x16unorm(l_w0);
  l_p = coax_p_v4(l_xy.x, l_xy.y, (f32((l_w1 & 65535u)) / 0x1.fffe00p15f), 0x1.000000p0f);
  l_ob = coax_p_wg_unpack4x8unorm(l_w1);
  l_n = coax_p_worldbuilder_octa(coax_p_v2(l_ob.z, l_ob.w));
  l_m = objects[i32(l_instance)];
  l_w = coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p));
  l_wp = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  l_o = coax_t17();
  l_o.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(12i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(16i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(20i), l_wp));
  l_o.y = l_w;
  l_c0 = coax_p_v3(l_m.x.x, l_m.y.x, l_m.z.x);
  l_c1 = coax_p_v3(l_m.x.y, l_m.y.y, l_m.z.y);
  l_c2 = coax_p_v3(l_m.x.z, l_m.y.z, l_m.z.z);
  l_o.z = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_c0, (l_n.x / coax_p_fmax(coax_p_wg_dot3(l_c0, l_c0), 0x1.ad7f2ap-24f))), coax_p_wg_mul_f3f(l_c1, (l_n.y / coax_p_fmax(coax_p_wg_dot3(l_c1, l_c1), 0x1.ad7f2ap-24f)))), coax_p_wg_mul_f3f(l_c2, (l_n.z / coax_p_fmax(coax_p_wg_dot3(l_c2, l_c2), 0x1.ad7f2ap-24f))));
  l_o.w = coax_p_wg_unpack2x16float(l_w2);
  l_rgb = coax_p_wg_unpack4x8unorm(l_w3);
  l_o.f4 = coax_p_wg_mul_f3f3(coax_p_v3((l_rgb.x * l_rgb.x), (l_rgb.y * l_rgb.y), (l_rgb.z * l_rgb.z)), coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z));
  l_o.f5 = (l_w3 >> u32(24u));
  l_o.f6 = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.f7 = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.f8 = 0u;
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_v: coax_t18) -> coax_t19 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_v: coax_t18 = p_v;
  return coax_p_chat_fx_vs(l__vid2, l__iid3, l_v);
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_p: vec2f;
  var l_ndc: vec2f;
  var l_ray: vec3f;
  var l_rl: f32;
  var l_up: f32;
  var l_sky: vec3f;
  var l_down: bool;
  var l_t: f32;
  var l_hit: vec3f;
  var l_xz: vec2f;
  var l_w1: vec2f;
  var l_dist: f32;
  var l_fog: f32;
  var l_ground: vec3f;
  var l_edge: f32;
  var l_c: vec3f;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  l_ndc = coax_p_v2((((l_p.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_p.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_ray = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(32i)), coax_at_u(i32(33i)), coax_at_u(i32(34i))), coax_p_wg_mul_ff3(l_ndc.x, coax_p_v3(coax_at_u(i32(36i)), coax_at_u(i32(37i)), coax_at_u(i32(38i))))), coax_p_wg_mul_ff3(l_ndc.y, coax_p_v3(coax_at_u(i32(40i)), coax_at_u(i32(41i)), coax_at_u(i32(42i)))));
  l_rl = coax_p_wg_length3(l_ray);
  l_up = (l_ray.y / coax_p_fmax(l_rl, 0x1.a36e2ep-14f));
  l_sky = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.1eb852p-5f, 0x1.eb851ep-5f, 0x1.70a3d8p-4f), coax_p_v3(0x1.0624dep-8f, 0x1.0624dep-7f, 0x1.47ae14p-6f), coax_p_clampf((l_up * 0x1.99999ap0f), 0.0f, 0x1.000000p0f));
  l_down = (l_ray.y < (-0x1.a36e2ep-14f));
  l_t = select(0x1.f40000p11f, ((-coax_at_u(i32(29i))) / coax_p_fmin(l_ray.y, (-0x1.a36e2ep-14f))), l_down);
  l_hit = coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), coax_p_wg_mul_f3f(l_ray, coax_p_fmin(l_t, 0x1.f40000p11f)));
  l_xz = coax_p_v2(l_hit.x, l_hit.z);
  l_w1 = coax_p_wg_max_f2f2(coax_p_wg_fwidth_f2(l_xz), coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f));
  l_dist = (l_t * l_rl);
  l_fog = exp(((-l_dist) * 0x1.89374cp-7f));
  l_ground = coax_p_worldbuilder_floor(l_xz, l_w1, l_fog, (0x1.000000p0f - (0x1.800000p-1f * coax_p_worldbuilder_beyond(l_xz))));
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_worldbuilder_marks(l_xz, l_w1));
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_v3(0x1.19999ap-1f, 0x1.e66666p-1f, 0x1.000000p0f), (coax_p_worldbuilder_rim(l_xz, l_w1) * ((l_fog * 0x1.99999ap-1f) + 0x1.99999ap-3f))));
  l_ground = coax_p_wg_mix_f3f3f(l_sky, l_ground, ((l_fog * 0x1.b33334p-1f) + 0x1.333334p-3f));
  l_edge = select(0.0f, coax_p_smoothstep(0.0f, 0x1.47ae14p-6f, (-l_up)), l_down);
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_sky, l_ground, l_edge), coax_at_u(i32(3i)));
  return coax_p_wg_make_f4_f3f(coax_p_worldbuilder_out(l_c), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t17) -> vec4f {
  var l_s: coax_t17 = p_s;
  var l_w1: vec2f;
  var l_textured: bool;
  var l_layer: i32;
  var l_tex: vec4f;
  var l_gn: vec3f;
  var l_gv: vec3f;
  var l_ndv: f32;
  var l_rim: f32;
  var l_glow: vec3f;
  var l_base: vec3f;
  var l_n: vec3f;
  var l_sun: f32;
  var l_sky: vec3f;
  var l_lit: vec3f;
  var l_c: vec3f;
  var l_d: f32;
  l_w1 = coax_p_wg_max_f2f2(coax_p_wg_fwidth_f2(coax_p_v2(l_s.y.x, l_s.y.z)), coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f));
  l_textured = (l_s.f5 < 255u);
  l_layer = select(0i, i32(l_s.f5), l_textured);
  l_tex = coax_p_render_sample_layer8(l_s.w, l_layer);
  if ((l_s.f8 == 1u))
  {
    return coax_p_wg_make_f4_f3f(coax_p_worldbuilder_out(coax_p_wg_mul_f3f(coax_p_worldbuilder_ground_fs(l_s, l_w1), coax_at_u(i32(3i)))), 0x1.000000p0f);
  }
  if ((l_s.f8 == 2u))
  {
    l_gn = coax_p_wg_normalize3(l_s.z);
    l_gv = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), l_s.y));
    l_ndv = coax_p_wg_dot3(l_gn, l_gv);
    if ((l_ndv < 0.0f))
    {
      discard;
    }
    l_rim = coax_p_engine_ghost_rim(l_ndv);
    l_glow = coax_p_v3(coax_p_engine_ghost_channel(l_rim, 0i), coax_p_engine_ghost_channel(l_rim, 1i), coax_p_engine_ghost_channel(l_rim, 2i));
    return coax_p_wg_make_f4_f3f(coax_p_worldbuilder_out(coax_p_wg_mul_f3f(l_glow, coax_at_u(i32(3i)))), coax_p_engine_ghost_alpha(l_rim));
  }
  if ((l_textured && (l_tex.w < 0x1.99999ap-2f)))
  {
    discard;
  }
  l_base = select(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_v3(l_tex.x, l_tex.y, l_tex.z), l_textured);
  l_n = coax_p_wg_normalize3(l_s.z);
  l_sun = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_wg_normalize3(coax_p_v3((-0x1.ccccccp-2f), 0x1.99999ap-1f, 0x1.99999ap-2f))));
  l_sky = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.47ae14p-3f, 0x1.333334p-3f, 0x1.1eb852p-3f), coax_p_v3(0x1.5c28f6p-2f, 0x1.99999ap-2f, 0x1.eb851ep-2f), ((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f));
  l_lit = coax_p_wg_add_f3f3(l_sky, coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.e147aep-1f, 0x1.ae147ap-1f), (l_sun * 0x1.e66666p-1f)));
  l_c = coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f3(l_base, l_s.f4), l_lit);
  l_d = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i)))));
  l_c = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.0624dep-8f, 0x1.0624dep-7f, 0x1.47ae14p-6f), l_c, exp(((-l_d) * 0x1.0624dep-7f)));
  return coax_p_wg_make_f4_f3f(coax_p_worldbuilder_out(coax_p_wg_mul_f3f(l_c, coax_at_u(i32(3i)))), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t19) -> vec4f {
  var l_v: coax_t19 = p_v;
  return coax_p_chat_fx_fs(l_v);
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
fn coax_p_worldbuilder_figure_vs(p_id: u32, p_k: u32) -> coax_t17 {
  var l_id: u32 = p_id;
  var l_k: u32 = p_k;
  var l_f: coax_t23;
  var l_m: coax_t14;
  var l_w: vec3f;
  var l_wp: vec4f;
  var l_o: coax_t17;
  l_f = coax_p_fivelab_scene_place(l_id, l_k, coax_p_glade_frame());
  l_m = figures[i32(l_k)];
  l_w = coax_p_v3(l_f.y.x, (l_f.y.y + coax_p_worldbuilder_height(l_m.x.x, l_m.x.z)), l_f.y.z);
  l_wp = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  l_o = coax_t17();
  l_o.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(12i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(16i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(20i), l_wp));
  l_o.y = l_w;
  l_o.z = l_f.z;
  l_o.w = coax_p_v2(0.0f, 0.0f);
  l_o.f4 = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_o.f5 = 255u;
  l_o.f6 = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.f7 = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.f8 = 2u;
  return l_o;
}
fn coax_p_worldbuilder_ground_vs(p_id: u32) -> coax_t17 {
  var l_id: u32 = p_id;
  var l_ox: array<i32, 6>;
  var l_oz: array<i32, 6>;
  var l_v: i32;
  var l_cell: i32;
  var l_k: i32;
  var l_i: i32;
  var l_j: i32;
  var l_g: vec4f;
  var l_w: vec3f;
  var l_wp: vec4f;
  var l_o: coax_t17;
  l_ox = array<i32, 6>(0i, 1i, 1i, 0i, 0i, 1i);
  l_oz = array<i32, 6>(0i, 1i, 0i, 0i, 1i, 1i);
  l_v = i32(l_id);
  l_cell = (l_v / 6i);
  l_k = (l_v % 6i);
  l_i = ((l_cell % 256i) + l_ox[l_k]);
  l_j = ((l_cell / 256i) + l_oz[l_k]);
  l_g = coax_p_worldbuilder_sample(l_i, l_j);
  l_w = coax_p_v3((f32(l_i) - 0x1.000000p7f), l_g.x, (f32(l_j) - 0x1.000000p7f));
  l_wp = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  l_o = coax_t17();
  l_o.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(12i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(16i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(20i), l_wp));
  l_o.y = l_w;
  l_o.z = coax_p_wg_normalize3(coax_p_v3(((coax_p_worldbuilder_sample((l_i - 1i), l_j)).x - (coax_p_worldbuilder_sample((l_i + 1i), l_j)).x), 0x1.000000p1f, ((coax_p_worldbuilder_sample(l_i, (l_j - 1i))).x - (coax_p_worldbuilder_sample(l_i, (l_j + 1i))).x)));
  l_o.w = coax_p_v2(l_w.x, l_w.z);
  l_o.f4 = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_o.f5 = 255u;
  l_o.f8 = 1u;
  l_o.f6 = coax_p_worldbuilder_shares(l_g.y);
  l_o.f7 = coax_p_worldbuilder_shares(l_g.z);
  return l_o;
}
fn coax_p_wg_unpack2x16unorm(p_x: u32) -> vec2f {
  var l_x: u32 = p_x;
  return coax_p_v2((f32((l_x & 65535u)) / 0x1.fffe00p15f), (f32((l_x >> u32(16u))) / 0x1.fffe00p15f));
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_worldbuilder_octa(p_o: vec2f) -> vec3f {
  var l_o: vec2f = p_o;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_ox: f32;
  var l_oy: f32;
  l_x = ((l_o.x * 0x1.000000p1f) - 0x1.000000p0f);
  l_y = ((l_o.y * 0x1.000000p1f) - 0x1.000000p0f);
  l_z = ((0x1.000000p0f - abs(l_x)) - abs(l_y));
  if ((l_z < 0.0f))
  {
    l_ox = ((0x1.000000p0f - abs(l_y)) * select((-0x1.000000p0f), 0x1.000000p0f, (l_x >= 0.0f)));
    l_oy = ((0x1.000000p0f - abs(l_x)) * select((-0x1.000000p0f), 0x1.000000p0f, (l_y >= 0.0f)));
    l_x = l_ox;
    l_y = l_oy;
  }
  return coax_p_wg_normalize3(coax_p_v3(l_x, l_y, l_z));
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
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
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
fn coax_p_wg_unpack2x16float(p_x: u32) -> vec2f {
  var l_x: u32 = p_x;
  return coax_p_v2(coax_p_wg_half((l_x & 65535u)), coax_p_wg_half((l_x >> u32(16u))));
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_chat_fx_vs(p__vid2: u32, p__iid3: u32, p_v: coax_t18) -> coax_t19 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_v: coax_t18 = p_v;
  var l_o: coax_t19;
  l_o = coax_t19();
  l_o.x = coax_p_v4((((l_v.x.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_v.x.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_v.x.z, l_v.x.w);
  l_o.z = l_v.y;
  l_o.w = coax_p_v2(l_v.z.x, l_v.z.y);
  l_o.f4 = coax_p_v2(l_v.z.z, l_v.z.w);
  l_o.f5 = l_v.w;
  l_o.f6 = l_v.f4;
  l_o.f7 = l_v.f5;
  l_o.f8 = l_v.f6;
  return l_o;
}
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
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
fn coax_p_wg_fwidth_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(fwidthFine(l_p0.x), fwidthFine(l_p0.y));
}
fn coax_p_wg_max_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v2(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y));
}
fn coax_p_worldbuilder_beyond(p_xz: vec2f) -> f32 {
  var l_xz: vec2f = p_xz;
  return select(0.0f, 0x1.000000p0f, (coax_p_fmax(abs(l_xz.x), abs(l_xz.y)) > 0x1.000000p7f));
}
fn coax_p_worldbuilder_floor(p_xz: vec2f, p_w1: vec2f, p_fog: f32, p_lines: f32) -> vec3f {
  var l_xz: vec2f = p_xz;
  var l_w1: vec2f = p_w1;
  var l_fog: f32 = p_fog;
  var l_lines: f32 = p_lines;
  var l_fine: f32;
  var l_coarse: f32;
  var l_fineFade: f32;
  var l_axes: f32;
  var l_ground: vec3f;
  l_fine = coax_p_worldbuilder_line(l_xz, l_w1);
  l_coarse = coax_p_worldbuilder_line(coax_p_wg_div_f2f(l_xz, 0x1.400000p3f), coax_p_wg_div_f2f(l_w1, 0x1.400000p3f));
  l_fineFade = (0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-4f, 0x1.ccccccp-2f, coax_p_fmax(l_w1.x, l_w1.y)));
  l_axes = (0x1.000000p0f - coax_p_fmin((coax_p_fmin((abs(l_xz.x) / l_w1.x), (abs(l_xz.y) / l_w1.y)) * 0x1.333334p-1f), 0x1.000000p0f));
  l_ground = coax_p_v3(0x1.89374cp-7f, 0x1.47ae14p-6f, 0x1.eb851ep-6f);
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-5f, 0x1.47ae14p-3f, 0x1.c28f5cp-3f), ((((l_fine * l_fineFade) * 0x1.19999ap-1f) * l_fog) * l_lines)));
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-4f, 0x1.47ae14p-2f, 0x1.ae147ap-2f), ((l_coarse * l_fog) * l_lines)));
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-3f, 0x1.ae147ap-2f, 0x1.000000p-1f), (((l_axes * 0x1.000000p-1f) * l_fog) * l_lines)));
  return l_ground;
}
fn coax_p_worldbuilder_marks(p_xz: vec2f, p_w1: vec2f) -> vec3f {
  var l_xz: vec2f = p_xz;
  var l_w1: vec2f = p_w1;
  var l_px: f32;
  var l_hd: f32;
  var l_ring: f32;
  var l_disc: f32;
  var l_c: vec3f;
  var l_bd: f32;
  var l_rim: f32;
  var l_core: f32;
  var l_wash: f32;
  l_px = coax_p_fmax((coax_p_wg_length2(l_w1) * 0x1.333334p0f), 0x1.47ae14p-6f);
  l_hd = coax_p_wg_length2(coax_p_v2((l_xz.x - coax_at_u(i32(24i))), (l_xz.y - coax_at_u(i32(26i)))));
  l_ring = (0x1.000000p0f - coax_p_smoothstep(0.0f, l_px, abs((l_hd - 0x1.333334p-1f))));
  l_disc = (exp((((-l_hd) * l_hd) * 0x1.800000p1f)) * 0x1.666666p-2f);
  l_c = coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.ccccccp-2f), ((l_ring + l_disc) * coax_at_u(i32(27i))));
  l_bd = coax_p_wg_length2(coax_p_v2((l_xz.x - coax_at_u(i32(44i))), (l_xz.y - coax_at_u(i32(46i)))));
  l_rim = (0x1.000000p0f - coax_p_smoothstep(0.0f, (l_px * 0x1.800000p0f), abs((l_bd - coax_at_u(i32(45i))))));
  l_core = (0x1.000000p0f - coax_p_smoothstep(0.0f, (l_px * 0x1.800000p0f), abs((l_bd - (coax_at_u(i32(45i)) * 0x1.666666p-2f)))));
  l_wash = ((0x1.000000p0f - coax_p_smoothstep((coax_at_u(i32(45i)) * 0x1.666666p-2f), coax_at_u(i32(45i)), l_bd)) * 0x1.eb851ep-5f);
  return coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.666666p-2f, 0x1.000000p0f, 0x1.e66666p-1f), (((l_rim + (l_core * 0x1.666666p-2f)) + l_wash) * coax_at_u(i32(47i)))));
}
fn coax_p_worldbuilder_rim(p_xz: vec2f, p_w1: vec2f) -> f32 {
  var l_xz: vec2f = p_xz;
  var l_w1: vec2f = p_w1;
  var l_d: f32;
  var l_px: f32;
  var l_line: f32;
  var l_glow: f32;
  l_d = (0x1.000000p7f - coax_p_fmax(abs(l_xz.x), abs(l_xz.y)));
  l_px = coax_p_fmax((coax_p_fmax(l_w1.x, l_w1.y) * 0x1.800000p0f), 0x1.47ae14p-5f);
  l_line = (0x1.000000p0f - coax_p_smoothstep(0.0f, l_px, abs(l_d)));
  l_glow = select(0.0f, (exp(((-l_d) * 0x1.000000p1f)) * 0x1.eb851ep-4f), (l_d > 0.0f));
  return coax_p_fmin((l_line + l_glow), 0x1.000000p0f);
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_worldbuilder_out(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_v3(sqrt(coax_p_fmax(l_c.x, 0.0f)), sqrt(coax_p_fmax(l_c.y, 0.0f)), sqrt(coax_p_fmax(l_c.z, 0.0f)));
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_render_sample_layer8(p_uv: vec2f, p_layer: i32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_rs_no_bias: f32;
  l_rs_no_bias = 0.0f;
  return coax_p_render_sample_layer8_bias(l_uv, l_layer, l_rs_no_bias);
}
fn coax_p_worldbuilder_ground_fs(p_s: coax_t17, p_w1: vec2f) -> vec3f {
  var l_s: coax_t17 = p_s;
  var l_w1: vec2f = p_w1;
  var l_xz: vec2f;
  var l_d: f32;
  var l_fog: f32;
  var l_painted: f32;
  var l_albedo: vec3f;
  var l_n: vec3f;
  var l_sun: f32;
  var l_sky: vec3f;
  var l_lit: vec3f;
  var l_view: vec3f;
  var l_graze: f32;
  var l_c: vec3f;
  var l_slope: f32;
  var l_bare: vec3f;
  var l_skyc: vec3f;
  l_xz = coax_p_v2(l_s.y.x, l_s.y.z);
  l_d = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i)))));
  l_fog = exp(((-l_d) * 0x1.89374cp-7f));
  l_painted = coax_p_clampf((((((l_s.f6.x + l_s.f6.y) + l_s.f6.z) + l_s.f7.x) + l_s.f7.y) + l_s.f7.z), 0.0f, 0x1.000000p0f);
  l_albedo = coax_p_wg_mul_f3f(coax_p_worldbuilder_layer(1i, l_xz), l_s.f6.x);
  l_albedo = coax_p_wg_add_f3f3(l_albedo, coax_p_wg_mul_f3f(coax_p_worldbuilder_layer(2i, l_xz), l_s.f6.y));
  l_albedo = coax_p_wg_add_f3f3(l_albedo, coax_p_wg_mul_f3f(coax_p_worldbuilder_layer(3i, l_xz), l_s.f6.z));
  l_albedo = coax_p_wg_add_f3f3(l_albedo, coax_p_wg_mul_f3f(coax_p_worldbuilder_layer(4i, l_xz), l_s.f7.x));
  l_albedo = coax_p_wg_add_f3f3(l_albedo, coax_p_wg_mul_f3f(coax_p_worldbuilder_layer(5i, l_xz), l_s.f7.y));
  l_albedo = coax_p_wg_add_f3f3(l_albedo, coax_p_wg_mul_f3f(coax_p_worldbuilder_layer(6i, l_xz), l_s.f7.z));
  l_n = coax_p_wg_normalize3(l_s.z);
  l_sun = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_wg_normalize3(coax_p_v3((-0x1.ccccccp-2f), 0x1.99999ap-1f, 0x1.99999ap-2f))));
  l_sky = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.47ae14p-3f, 0x1.333334p-3f, 0x1.1eb852p-3f), coax_p_v3(0x1.5c28f6p-2f, 0x1.99999ap-2f, 0x1.eb851ep-2f), ((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f));
  l_lit = coax_p_wg_add_f3f3(l_sky, coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.e147aep-1f, 0x1.ae147ap-1f), (l_sun * 0x1.e66666p-1f)));
  l_view = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), l_s.y));
  l_graze = coax_pow_bg_vs((0x1.000000p0f - coax_p_clampf(coax_p_wg_dot3(l_n, l_view), 0.0f, 0x1.000000p0f)), 0x1.800000p1f);
  l_c = coax_p_wg_mul_f3f3(l_albedo, l_lit);
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-3f, 0x1.5c28f6p-2f, 0x1.d70a3ep-2f), (l_graze * l_s.f7.z)));
  l_c = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.0624dep-8f, 0x1.0624dep-7f, 0x1.47ae14p-6f), l_c, exp(((-l_d) * 0x1.0624dep-7f)));
  l_slope = (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_clampf(((l_sun * 0x1.400000p0f) + 0x1.99999ap-4f), 0.0f, 0x1.000000p0f)));
  l_bare = coax_p_wg_mul_f3f(coax_p_worldbuilder_floor(l_xz, l_w1, l_fog, 0x1.000000p0f), select((l_slope * 0x1.99999ap0f), 0x1.000000p0f, (l_n.y > 0x1.ffbe76p-1f)));
  l_skyc = coax_p_v3(0x1.1eb852p-5f, 0x1.eb851ep-5f, 0x1.70a3d8p-4f);
  l_bare = coax_p_wg_mix_f3f3f(l_skyc, l_bare, ((l_fog * 0x1.b33334p-1f) + 0x1.333334p-3f));
  l_c = coax_p_wg_mix_f3f3f(l_bare, coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_worldbuilder_floor(l_xz, l_w1, l_fog, 0x1.000000p0f), (0x1.000000p-1f * l_painted))), l_painted);
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.19999ap-1f, 0x1.e66666p-1f, 0x1.000000p0f), (coax_p_worldbuilder_rim(l_xz, l_w1) * ((l_fog * 0x1.99999ap-1f) + 0x1.99999ap-3f))));
  return coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_worldbuilder_marks(l_xz, l_w1), ((l_fog * 0x1.b33334p-1f) + 0x1.333334p-3f)));
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_engine_ghost_rim(p_ndv: f32) -> f32 {
  var l_ndv: f32 = p_ndv;
  var l_t: f32;
  l_t = (0x1.000000p0f - coax_p_clampf(abs(l_ndv), 0.0f, 0x1.000000p0f));
  return (l_t * l_t);
}
fn coax_p_engine_ghost_channel(p_rim: f32, p_k: i32) -> f32 {
  var l_rim: f32 = p_rim;
  var l_k: i32 = p_k;
  var l_body: f32;
  var l_edge: f32;
  l_body = select(select(0x1.70a3d8p-1f, 0x1.3d70a4p-1f, (l_k == 1i)), 0x1.333334p-2f, (l_k == 0i));
  l_edge = select(0x1.000000p0f, 0x1.b33334p-1f, (l_k == 0i));
  return ((l_body + ((l_edge - l_body) * coax_p_clampf(l_rim, 0.0f, 0x1.000000p0f))) * coax_p_engine_ghost_light(l_rim));
}
fn coax_p_engine_ghost_alpha(p_rim: f32) -> f32 {
  var l_rim: f32 = p_rim;
  return (0x1.47ae14p-3f + (0x1.7ae148p-1f * coax_p_smoothstep(0.0f, 0x1.000000p0f, l_rim)));
}
fn coax_p_chat_fx_fs(p_v: coax_t19) -> vec4f {
  var l_v: coax_t19 = p_v;
  var l_p: vec2f;
  var l_mono: f32;
  var l_poly: vec4f;
  var l_kind: f32;
  var l_c: vec4f;
  l_p = coax_p_v2(l_v.x.x, l_v.x.y);
  if (((((l_p.x < l_v.f7.x) || (l_p.y < l_v.f7.y)) || (l_p.x >= l_v.f7.z)) || (l_p.y >= l_v.f7.w)))
  {
    discard;
  }
  l_mono = (coax_p_wg_load_2d(coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(coax_p_wg_mul_f2f2(l_v.f4, coax_p_wg_cvt_f2u2(coax_p_wg_dims_2d())))), 0i)).x;
  l_poly = coax_p_wg_load_2d_1(coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(coax_p_wg_mul_f2f2(l_v.f4, coax_p_wg_cvt_f2u2(coax_p_wg_dims_2d_1())))), 0i);
  l_kind = l_v.y.x;
  l_c = vec4f();
  if ((l_kind > 0x1.a00000p2f))
  {
    l_c = coax_p_chat_segment_color(l_p, l_v.z, l_v.w.x, l_v.f5);
  }
  else
  {
    if ((l_kind > 0x1.600000p2f))
    {
      l_c = coax_p_chat_inset_color(l_p, l_v.z, l_v.w.x, l_v.y.y, l_v.f5);
    }
    else
    {
      if ((l_kind > 0x1.200000p2f))
      {
        l_c = coax_p_chat_gradient_color(l_p, l_v.z, l_v.w.x, l_v.f5, l_v.f6);
      }
      else
      {
        if ((l_kind > 0x1.c00000p1f))
        {
          l_c = coax_p_chat_elbow_color(l_p, l_v.z, l_v.w, l_v.f5);
        }
        else
        {
          if ((l_kind > 0x1.400000p1f))
          {
            l_c = coax_p_wg_make_f4_f3f(coax_p_v3(l_poly.x, l_poly.y, l_poly.z), (l_poly.w * l_v.f5.w));
          }
          else
          {
            if ((l_kind > 0x1.800000p0f))
            {
              l_c = coax_p_wg_make_f4_f3f(coax_p_v3(l_v.f5.x, l_v.f5.y, l_v.f5.z), (l_v.f5.w * l_mono));
            }
            else
            {
              if ((l_kind > 0x1.000000p-1f))
              {
                l_c = coax_p_chat_shadow_color(l_p, l_v.z, l_v.w.x, l_v.y.y, l_v.f5);
              }
              else
              {
                l_c = coax_p_chat_quad_color(l_p, l_v.z, l_v.w.x, l_v.w.y, l_v.f5, l_v.f6);
              }
            }
          }
        }
      }
    }
  }
  l_c.w = (l_c.w * coax_p_chat_edge_fade_alpha(l_p, l_v.f8));
  return l_c;
}
fn coax_p_glade_frame() -> coax_t25 {
  return coax_t25(coax_p_v4_at(0i), coax_p_v4_at(4i), coax_p_v4_at(8i), coax_p_v4_at(12i), coax_p_v4_at(16i), coax_p_v4_at(20i), coax_p_v4_at(24i), coax_p_v4_at(28i), coax_p_v4_at(32i), coax_p_v4_at(36i), coax_p_v4_at(40i), coax_p_v4_at(44i));
}
fn coax_p_fivelab_scene_place(p_id: u32, p_instance: u32, p_gf: coax_t25) -> coax_t23 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_gf: coax_t25 = p_gf;
  var l_m0: coax_t14;
  var l_m: coax_t14;
  var l_b: u32;
  var l_xy: vec2f;
  var l_zw: u32;
  var l_local: vec3f;
  var l_flags: f32;
  var l_bones: bool;
  var l_at: vec3f;
  var l_at4: vec4f;
  var l_nw4: vec4f;
  var l_nw: vec3f;
  var l_joints: u32;
  var l_weights: u32;
  var l_arm: bool;
  var l_sk: coax_t26;
  var l_sk2: coax_t26;
  var l_c: f32;
  var l_sn: f32;
  var l_w: vec3f;
  var l_o: coax_t23;
  var l_kind: u32;
  if (((l_gf.z.z > 0x1.800000p0f) && (l_instance >= 65536u)))
  {
    l_m0 = figures[i32(((l_instance >> u32(16u)) - 1u))];
    if ((l_m0.w.w > 0x1.380000p4f))
    {
      return coax_p_cg_meadow_vs(l_id, l_instance, l_m0, l_gf);
    }
    return coax_p_cg_instanced_vs(l_id, l_instance, l_m0, l_gf);
  }
  l_m = figures[i32(l_instance)];
  if ((l_m.w.w > 0x1.f00000p3f))
  {
    return coax_p_cg_environment_vs(l_id, l_instance, l_m, l_gf);
  }
  if ((coax_p_wg_fmod(l_m.w.w, 0x1.000000p3f) > 0x1.c00000p1f))
  {
    return coax_p_fivelab_block_vs(l_id, l_instance, l_m, l_gf);
  }
  l_b = (coax_p_wg_u32_f(l_m.y.w) + (l_id * 6u));
  l_xy = coax_p_wg_unpack2x16unorm(people[i32(l_b)]);
  l_zw = people[i32((l_b + 1u))];
  l_local = coax_p_add3(coax_p_scale3(coax_p_v3(l_xy.x, l_xy.y, (f32((l_zw & 65535u)) / 0x1.fffe00p15f)), 0x1.400000p1f), coax_p_v3((-0x1.400000p0f), (-0x1.000000p-2f), (-0x1.400000p0f)));
  l_flags = l_m.w.w;
  l_bones = (coax_p_wg_fmod(l_flags, 0x1.000000p1f) > 0x1.000000p-1f);
  l_at = l_local;
  if (l_bones)
  {
    l_at = coax_p_fivelab_gaunt(l_local);
  }
  l_at4 = coax_p_v4(l_at.x, l_at.y, l_at.z, 0x1.000000p0f);
  l_nw4 = coax_p_wg_unpack4x8snorm(people[i32((l_b + 2u))]);
  l_nw = coax_p_v3(l_nw4.x, l_nw4.y, l_nw4.z);
  l_joints = people[i32((l_b + 4u))];
  l_weights = people[i32((l_b + 5u))];
  l_arm = (coax_p_wg_fmod(l_flags, 0x1.000000p4f) > 0x1.e00000p2f);
  l_sk = coax_t26();
  if ((l_m.y.z > 0x1.800000p0f))
  {
    l_sk = coax_p_fivelab_skin_mix(coax_p_wg_u32_f(l_m.y.x), coax_p_wg_u32_f(l_m.y.y), l_m.y.z, l_joints, l_weights, l_at4, l_nw, l_arm);
  }
  else
  {
    l_sk = coax_p_fivelab_skin(coax_p_wg_u32_f(l_m.y.x), l_joints, l_weights, l_at4, l_nw, l_arm);
  }
  if (((l_m.y.z > 0x1.0624dep-10f) && (l_m.y.z < 0x1.800000p0f)))
  {
    l_sk2 = coax_p_fivelab_skin(coax_p_wg_u32_f(l_m.y.y), l_joints, l_weights, l_at4, l_nw, l_arm);
    l_sk.x = coax_p_mix3(l_sk.x, l_sk2.x, l_m.y.z);
    l_sk.y = coax_p_mix3(l_sk.y, l_sk2.y, l_m.y.z);
  }
  l_c = cos(l_m.x.w);
  l_sn = sin(l_m.x.w);
  l_w = coax_p_v3((((l_sk.x.x * l_c) + (l_sk.x.z * l_sn)) + l_m.x.x), (l_sk.x.y + l_m.x.y), ((((-l_sk.x.x) * l_sn) + (l_sk.x.z * l_c)) + l_m.x.z));
  l_o = coax_t23();
  l_o.x = coax_p_fivelab_project(l_w, l_gf);
  l_o.y = l_w;
  l_o.z = coax_p_v3(((l_sk.y.x * l_c) + (l_sk.y.z * l_sn)), l_sk.y.y, (((-l_sk.y.x) * l_sn) + (l_sk.y.z * l_c)));
  l_o.w = coax_p_wg_unpack2x16unorm(people[i32((l_b + 3u))]);
  l_o.f7 = l_local;
  l_o.f8 = l_instance;
  l_kind = ((l_zw >> u32(24u)) & 255u);
  l_o.f4 = ((l_zw >> u32(16u)) & 255u);
  l_o.f5 = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  if ((l_kind == 1u))
  {
    l_o.f5 = coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z);
  }
  if ((l_kind == 2u))
  {
    l_o.f5 = coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z);
  }
  if (((l_kind == 3u) || (l_kind == 4u)))
  {
    l_o.f4 = (l_o.f4 + coax_p_wg_u32_f(l_m.z.w));
    l_o.f6 = coax_p_v4(l_m.f4.x, l_m.f4.y, l_m.f4.z, ((f32((l_kind - 2u)) * 0x1.000000p1f) + l_m.f4.w));
  }
  if (l_bones)
  {
    l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, select(0x1.000000p3f, 0x1.200000p3f, (l_kind >= 3u)));
  }
  return l_o;
}
fn coax_p_worldbuilder_height(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_fx: f32;
  var l_fz: f32;
  var l_i: i32;
  var l_j: i32;
  var l_ax: f32;
  var l_az: f32;
  var l_a: f32;
  var l_b: f32;
  l_fx = coax_p_clampf((l_x + 0x1.000000p7f), 0.0f, 0x1.ffff7cp7f);
  l_fz = coax_p_clampf((l_z + 0x1.000000p7f), 0.0f, 0x1.ffff7cp7f);
  l_i = i32(floor(l_fx));
  l_j = i32(floor(l_fz));
  l_ax = (l_fx - floor(l_fx));
  l_az = (l_fz - floor(l_fz));
  l_a = coax_p_mix((coax_p_worldbuilder_sample(l_i, l_j)).x, (coax_p_worldbuilder_sample((l_i + 1i), l_j)).x, l_ax);
  l_b = coax_p_mix((coax_p_worldbuilder_sample(l_i, (l_j + 1i))).x, (coax_p_worldbuilder_sample((l_i + 1i), (l_j + 1i))).x, l_ax);
  return coax_p_mix(l_a, l_b, l_az);
}
fn coax_p_worldbuilder_sample(p_i: i32, p_j: i32) -> vec4f {
  var l_i: i32 = p_i;
  var l_j: i32 = p_j;
  var l_ci: i32;
  var l_cj: i32;
  l_ci = select(select(l_i, 256i, (l_i > 256i)), 0i, (l_i < 0i));
  l_cj = select(select(l_j, 256i, (l_j > 256i)), 0i, (l_j < 0i));
  return ground[((l_cj * 257i) + l_ci)];
}
fn coax_p_worldbuilder_shares(p_packed: f32) -> vec3f {
  var l_packed: f32 = p_packed;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  l_a = floor((l_packed / 0x1.000000p16f));
  l_b = floor(((l_packed - (l_a * 0x1.000000p16f)) / 0x1.000000p8f));
  l_c = ((l_packed - (l_a * 0x1.000000p16f)) - (l_b * 0x1.000000p8f));
  return coax_p_v3((l_c / 0x1.fe0000p7f), (l_b / 0x1.fe0000p7f), (l_a / 0x1.fe0000p7f));
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_wg_half(p_h: u32) -> f32 {
  var l_h: u32 = p_h;
  var l_sign: u32;
  var l_e: u32;
  var l_m: u32;
  var l_v: f32;
  l_sign = ((l_h >> u32(15u)) & 1u);
  l_e = ((l_h >> u32(10u)) & 31u);
  l_m = (l_h & 1023u);
  if ((l_e == 0u))
  {
    l_v = (f32(l_m) * 0x1.000000p-24f);
    if ((l_sign == 1u))
    {
      return (-l_v);
    }
    return l_v;
  }
  if ((l_e == 31u))
  {
    return bitcast<f32>((((l_sign << u32(31u)) | 2139095040u) | (l_m << u32(13u))));
  }
  return bitcast<f32>((((l_sign << u32(31u)) | ((l_e + 112u) << u32(23u))) | (l_m << u32(13u))));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_worldbuilder_line(p_v: vec2f, p_w: vec2f) -> f32 {
  var l_v: vec2f = p_v;
  var l_w: vec2f = p_w;
  var l_g: vec2f;
  l_g = coax_p_wg_div_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f(coax_p_wg_fract_f2(coax_p_wg_add_f2f(l_v, 0x1.000000p-1f)), 0x1.000000p-1f)), l_w);
  return (0x1.000000p0f - coax_p_fmin(coax_p_fmin(l_g.x, l_g.y), 0x1.000000p0f));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_render_sample_layer8_bias(p_uv: vec2f, p_layer: i32, p_bias: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_bias: f32 = p_bias;
  var l_rs_dx: vec2f;
  var l_rs_dy: vec2f;
  l_rs_dx = coax_p_v2(dpdxFine(l_uv.x), dpdxFine(l_uv.y));
  l_rs_dy = coax_p_v2(dpdyFine(l_uv.x), dpdyFine(l_uv.y));
  return coax_p_render_sample_layer8_grad(l_uv, l_layer, l_rs_dx, l_rs_dy, l_bias);
}
fn coax_p_worldbuilder_layer(p_k: i32, p_xz: vec2f) -> vec3f {
  var l_k: i32 = p_k;
  var l_xz: vec2f = p_xz;
  var l_at: i32;
  var l_t: f32;
  var l_gain: f32;
  l_at = (48i + (l_k * 12i));
  l_t = coax_p_clampf(coax_p_engine_terrain_blend_t(l_xz.x, l_xz.y, coax_at_u(i32((l_at + 3i))), coax_at_u(i32((l_at + 8i))), 0.0f, coax_at_u(i32((l_at + 7i))), coax_at_u(i32((l_at + 9i))), 0.0f), 0.0f, 0x1.000000p0f);
  l_gain = coax_p_engine_terrain_gain(l_xz.x, l_xz.y, coax_at_u(i32((l_at + 10i))), coax_at_u(i32((l_at + 11i))), 0x1.7ae148p-2f, 0.0f);
  return coax_p_v3(coax_p_engine_terrain_decode(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 4i))), l_t, l_gain), coax_p_engine_terrain_decode(coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 5i))), l_t, l_gain), coax_p_engine_terrain_decode(coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 6i))), l_t, l_gain));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_engine_ghost_light(p_rim: f32) -> f32 {
  var l_rim: f32 = p_rim;
  return (0x1.000000p-1f + (0x1.666666p-1f * l_rim));
}
fn coax_p_wg_dims_2d() -> coax_t28 {
  return coax_p_u2(textureDimensions(gpui_atlas).x, textureDimensions(gpui_atlas).y);
}
fn coax_p_wg_cvt_f2u2(p_p: coax_t28) -> vec2f {
  var l_p: coax_t28 = p_p;
  return coax_p_v2(f32(l_p.x), f32(l_p.y));
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t27 {
  var l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
}
fn coax_p_wg_load_2d(p_c: coax_t27, p_level: i32) -> vec4f {
  var l_c: coax_t27 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(gpui_atlas, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_dims_2d_1() -> coax_t28 {
  return coax_p_u2(textureDimensions(gpui_color).x, textureDimensions(gpui_color).y);
}
fn coax_p_wg_load_2d_1(p_c: coax_t27, p_level: i32) -> vec4f {
  var l_c: coax_t27 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(gpui_color, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_chat_segment_color(p_p: vec2f, p_b: vec4f, p_width: f32, p_color: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_b: vec4f = p_b;
  var l_width: f32 = p_width;
  var l_color: vec4f = p_color;
  var l_dist: f32;
  var l_cover: f32;
  l_dist = coax_p_chat_segment_distance(l_p, coax_p_v2(l_b.x, l_b.y), coax_p_v2(l_b.z, l_b.w));
  l_cover = coax_p_saturate((((0x1.000000p-1f * l_width) + 0x1.000000p-1f) - l_dist));
  return coax_p_wg_mul_f4f4(l_color, coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, l_cover));
}
fn coax_p_chat_inset_color(p_p: vec2f, p_bounds: vec4f, p_corner_radius: f32, p_dy: f32, p_color: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_bounds: vec4f = p_bounds;
  var l_corner_radius: f32 = p_corner_radius;
  var l_dy: f32 = p_dy;
  var l_color: vec4f = p_color;
  var l_half_size: vec2f;
  var l_a: vec2f;
  var l_b: vec2f;
  var l_inside: f32;
  var l_covered: f32;
  l_half_size = coax_p_wg_div_f2f(coax_p_v2(l_bounds.z, l_bounds.w), 0x1.000000p1f);
  l_a = coax_p_wg_add_f2f(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_bounds.x, l_bounds.y)), l_half_size)), l_half_size), l_corner_radius);
  l_b = coax_p_wg_add_f2f(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(0.0f, l_dy)), coax_p_v2(l_bounds.x, l_bounds.y)), l_half_size)), l_half_size), l_corner_radius);
  l_inside = coax_p_saturate((0x1.000000p-1f - coax_p_chat_quad_sdf_impl(l_a, l_corner_radius)));
  l_covered = coax_p_saturate((0x1.000000p-1f - coax_p_chat_quad_sdf_impl(l_b, l_corner_radius)));
  return coax_p_wg_mul_f4f4(l_color, coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, (l_inside * (0x1.000000p0f - l_covered))));
}
fn coax_p_chat_gradient_color(p_p: vec2f, p_bounds: vec4f, p_corner_radius: f32, p_top: vec4f, p_bottom: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_bounds: vec4f = p_bounds;
  var l_corner_radius: f32 = p_corner_radius;
  var l_top: vec4f = p_top;
  var l_bottom: vec4f = p_bottom;
  var l_t: f32;
  l_t = coax_p_saturate(((l_p.y - l_bounds.y) / coax_p_fmax(l_bounds.w, 0x1.000000p0f)));
  return coax_p_chat_quad_color(l_p, l_bounds, l_corner_radius, 0.0f, coax_p_wg_mix_f4f4f(l_top, l_bottom, l_t), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_chat_elbow_color(p_p: vec2f, p_b: vec4f, p_c: vec2f, p_color: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_b: vec4f = p_b;
  var l_c: vec2f = p_c;
  var l_color: vec4f = p_color;
  var l_s: vec2f;
  var l_r: f32;
  var l_end_x: f32;
  var l_half_w: f32;
  var l_dist: f32;
  var l_prev: vec2f;
  var l_k: i32;
  var l_t: f32;
  var l_q: vec2f;
  var l_cover: f32;
  l_s = coax_p_v2(l_b.x, l_b.y);
  l_r = l_b.z;
  l_end_x = l_b.w;
  l_half_w = (0x1.000000p-1f * l_c.x);
  l_dist = 0x1.dcd650p29f;
  l_prev = l_s;
  {
    l_k = 1i;
    while ((l_k <= 24i))
    {
      l_t = (f32(l_k) / 0x1.800000p4f);
      l_q = coax_p_wg_add_f2f2(l_s, coax_p_wg_mul_ff2(l_r, coax_p_v2((l_t * l_t), ((0x1.000000p1f * l_t) - (l_t * l_t)))));
      l_dist = coax_p_fmin(l_dist, coax_p_chat_segment_distance(l_p, l_prev, l_q));
      l_prev = l_q;
      l_k = (l_k + 1i);
    }
  }
  l_dist = coax_p_fmin(l_dist, coax_p_chat_segment_distance(l_p, l_prev, coax_p_v2(l_end_x, (l_s.y + l_r))));
  l_cover = coax_p_saturate(((l_half_w + 0x1.000000p-1f) - l_dist));
  if ((l_c.y > 0x1.000000p-1f))
  {
    l_cover = coax_p_fmax(0.0f, (l_cover - coax_p_saturate(((l_half_w + 0x1.000000p-1f) - abs((l_p.x - l_s.x))))));
  }
  return coax_p_wg_mul_f4f4(l_color, coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, l_cover));
}
fn coax_p_chat_shadow_color(p_p: vec2f, p_bounds: vec4f, p_corner_radius: f32, p_blur: f32, p_color: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_bounds: vec4f = p_bounds;
  var l_corner_radius: f32 = p_corner_radius;
  var l_blur: f32 = p_blur;
  var l_color: vec4f = p_color;
  var l_half_size: vec2f;
  var l_pt: vec2f;
  var l_alpha: f32;
  var l_corner_center_to_point: vec2f;
  var l_low: f32;
  var l_high: f32;
  var l_start: f32;
  var l_end: f32;
  var l_step: f32;
  var l_y: f32;
  var l_i: i32;
  l_half_size = coax_p_wg_div_f2f(coax_p_v2(l_bounds.z, l_bounds.w), 0x1.000000p1f);
  l_pt = coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_bounds.x, l_bounds.y), l_half_size));
  l_alpha = 0.0f;
  if ((l_blur == 0.0f))
  {
    l_corner_center_to_point = coax_p_wg_add_f2f(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(l_pt), l_half_size), l_corner_radius);
    l_alpha = coax_p_saturate((0x1.000000p-1f - coax_p_chat_quad_sdf_impl(l_corner_center_to_point, l_corner_radius)));
  }
  else
  {
    l_low = (l_pt.y - l_half_size.y);
    l_high = (l_pt.y + l_half_size.y);
    l_start = coax_p_clampf(((-0x1.800000p1f) * l_blur), l_low, l_high);
    l_end = coax_p_clampf((0x1.800000p1f * l_blur), l_low, l_high);
    l_step = ((l_end - l_start) / 0x1.000000p2f);
    l_y = (l_start + (l_step * 0x1.000000p-1f));
    {
      l_i = 0i;
      while ((l_i < 4i))
      {
        l_alpha = (l_alpha + ((coax_p_chat_blur_along_x(l_pt.x, (l_pt.y - l_y), l_blur, l_corner_radius, l_half_size) * coax_p_chat_gaussian(l_y, l_blur)) * l_step));
        l_y = (l_y + l_step);
        l_i = (l_i + 1i);
      }
    }
  }
  return coax_p_wg_mul_f4f4(l_color, coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, l_alpha));
}
fn coax_p_chat_quad_color(p_p: vec2f, p_bounds: vec4f, p_corner_radius: f32, p_bw: f32, p_background_color: vec4f, p_border_color: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_bounds: vec4f = p_bounds;
  var l_corner_radius: f32 = p_corner_radius;
  var l_bw: f32 = p_bw;
  var l_background_color: vec4f = p_background_color;
  var l_border_color: vec4f = p_border_color;
  var l_antialias_threshold: f32;
  var l_half_size: vec2f;
  var l_center_to_point: vec2f;
  var l_reduced_border: f32;
  var l_corner_to_point: vec2f;
  var l_corner_center_to_point: vec2f;
  var l_is_near_rounded_corner: bool;
  var l_straight_border_inner_corner_to_point: vec2f;
  var l_is_beyond_inner_straight_border: bool;
  var l_is_within_inner_straight_border: bool;
  var l_outer_sdf: f32;
  var l_inner_sdf: f32;
  var l_border_sdf: f32;
  var l_color: vec4f;
  var l_blended_border: vec4f;
  if (((l_bw == 0.0f) && (l_corner_radius == 0.0f)))
  {
    return l_background_color;
  }
  l_antialias_threshold = 0x1.000000p-1f;
  l_half_size = coax_p_wg_div_f2f(coax_p_v2(l_bounds.z, l_bounds.w), 0x1.000000p1f);
  l_center_to_point = coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_bounds.x, l_bounds.y)), l_half_size);
  l_reduced_border = select(l_bw, (-l_antialias_threshold), (l_bw == 0.0f));
  l_corner_to_point = coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(l_center_to_point), l_half_size);
  l_corner_center_to_point = coax_p_wg_add_f2f(l_corner_to_point, l_corner_radius);
  l_is_near_rounded_corner = ((l_corner_center_to_point.x >= 0.0f) && (l_corner_center_to_point.y >= 0.0f));
  l_straight_border_inner_corner_to_point = coax_p_wg_add_f2f(l_corner_to_point, l_reduced_border);
  l_is_beyond_inner_straight_border = ((l_straight_border_inner_corner_to_point.x > 0.0f) || (l_straight_border_inner_corner_to_point.y > 0.0f));
  l_is_within_inner_straight_border = ((l_straight_border_inner_corner_to_point.x < (-l_antialias_threshold)) && (l_straight_border_inner_corner_to_point.y < (-l_antialias_threshold)));
  if ((l_is_within_inner_straight_border && (!l_is_near_rounded_corner)))
  {
    return l_background_color;
  }
  l_outer_sdf = coax_p_chat_quad_sdf_impl(l_corner_center_to_point, l_corner_radius);
  l_inner_sdf = 0.0f;
  if (((l_corner_center_to_point.x <= 0.0f) || (l_corner_center_to_point.y <= 0.0f)))
  {
    l_inner_sdf = (-coax_p_fmax(l_straight_border_inner_corner_to_point.x, l_straight_border_inner_corner_to_point.y));
  }
  else
  {
    if (l_is_beyond_inner_straight_border)
    {
      l_inner_sdf = (-0x1.000000p0f);
    }
    else
    {
      l_inner_sdf = (-(l_outer_sdf + l_reduced_border));
    }
  }
  l_border_sdf = coax_p_fmax(l_inner_sdf, l_outer_sdf);
  l_color = l_background_color;
  if ((l_border_sdf < l_antialias_threshold))
  {
    l_blended_border = coax_p_chat_over(l_background_color, l_border_color);
    l_color = coax_p_wg_mix_f4f4f(l_background_color, l_blended_border, coax_p_saturate((l_antialias_threshold - l_inner_sdf)));
  }
  return coax_p_wg_mul_f4f4(l_color, coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, coax_p_saturate((l_antialias_threshold - l_outer_sdf))));
}
fn coax_p_chat_edge_fade_alpha(p_p: vec2f, p_fade: vec4f) -> f32 {
  var l_p: vec2f = p_p;
  var l_fade: vec4f = p_fade;
  var l_ramp: f32;
  l_ramp = 0x1.000000p0f;
  if ((l_fade.y > 0.0f))
  {
    l_ramp = coax_p_fmin(l_ramp, coax_p_clampf(((l_p.y - l_fade.x) / l_fade.y), 0.0f, 0x1.000000p0f));
  }
  if ((l_fade.w > 0.0f))
  {
    l_ramp = coax_p_fmin(l_ramp, coax_p_clampf(((l_fade.z - l_p.y) / l_fade.w), 0.0f, 0x1.000000p0f));
  }
  return (l_ramp * l_ramp);
}
fn coax_p_v4_at(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return vec4f(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_cg_meadow_vs(p_id: u32, p_instance: u32, p_m: coax_t14, p_gf: coax_t25) -> coax_t23 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m: coax_t14 = p_m;
  var l_gf: coax_t25 = p_gf;
  var l_o: coax_t23;
  var l_i: u32;
  var l_kit: i32;
  var l_side: u32;
  var l_iz: u32;
  var l_ix: u32;
  var l_half: u32;
  var l_origin: f32;
  var l_ex: f32;
  var l_ez: f32;
  var l_reach: f32;
  var l_fine: f32;
  var l_t: i32;
  var l_c: f32;
  var l_gx: u32;
  var l_gz: u32;
  var l_fx: u32;
  var l_fz: u32;
  var l_x: f32;
  var l_z: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_d: f32;
  var l_shown: f32;
  var l_au: f32;
  var l_c_30: f32;
  var l_gx_31: u32;
  var l_gz_32: u32;
  var l_x_33: f32;
  var l_z_34: f32;
  var l_dx_35: f32;
  var l_dz_36: f32;
  var l_k: f32;
  var l_h: f32;
  var l_au_39: f32;
  var l_p: coax_t30;
  l_o = coax_t23();
  l_o.x = coax_p_v4(0.0f, 0.0f, (-0x1.000000p1f), 0x1.000000p0f);
  l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f8 = ((l_instance >> u32(16u)) - 1u);
  l_i = (l_instance & 65535u);
  l_kit = i32(l_m.x.x);
  l_side = u32(l_m.x.z);
  l_iz = (l_i / l_side);
  l_ix = (l_i - (l_iz * l_side));
  l_half = (l_side / 2u);
  l_origin = (-0x1.000000p15f);
  l_ex = l_gf.f4.x;
  l_ez = l_gf.f4.z;
  l_reach = select(0x1.000000p0f, l_m.y.y, (l_m.y.y > 0.0f));
  l_fine = select(0x1.000000p0f, l_m.y.z, (l_m.y.z > 0.0f));
  if ((l_kit == 33i))
  {
    l_t = i32(l_m.x.y);
    l_c = coax_p_e2_field_cell(l_t);
    l_gx = ((u32(floor(((l_ex - l_origin) / l_c))) + l_ix) - l_half);
    l_gz = ((u32(floor(((l_ez - l_origin) / l_c))) + l_iz) - l_half);
    l_fx = coax_p_e2_field_fine_x(l_t, l_gx, l_gz);
    l_fz = coax_p_e2_field_fine_z(l_t, l_gx, l_gz);
    l_x = (l_origin + ((f32(l_fx) + coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 1u))) * 0x1.000000p-1f));
    l_z = (l_origin + ((f32(l_fz) + coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 2u))) * 0x1.000000p-1f));
    l_dx = (l_x - l_ex);
    l_dz = (l_z - l_ez);
    l_d = sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
    if (((l_d < coax_p_e2_field_start_r(l_t, l_reach, l_fine)) || (l_d >= coax_p_e2_field_out_r(l_t, l_reach, l_fine))))
    {
      return l_o;
    }
    l_shown = coax_p_e2_field_shown(coax_p_e2_field_rel_r(l_t, l_d, l_reach, l_fine), coax_p_e2_field_rank(l_t, l_gx, l_gz, l_fx, l_fz));
    if ((l_shown <= 0.0f))
    {
      return l_o;
    }
    l_au = coax_p_e2_meadow_gold(l_x, l_z);
    return coax_p_cg_meadow_place(l_id, coax_p_v4(l_x, coax_p_exp_height_gpu(l_x, l_z), l_z, (coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 3u)) * 0x1.921fb6p2f)), (coax_p_e2_meadow_tuft_sv(l_fx, l_fz) * l_shown), (coax_p_e2_meadow_tuft_sh(l_fx, l_fz) * l_shown), coax_p_v3((0x1.333334p-2f + (0x1.0a3d70p-2f * l_au)), 0x1.eb851ep-2f, 0x1.c28f5cp-5f), l_au, l_m, l_gf, l_o.f8);
  }
  l_c_30 = coax_p_e2_meadow_cell(l_kit);
  l_gx_31 = ((u32(floor(((l_ex - l_origin) / l_c_30))) + l_ix) - l_half);
  l_gz_32 = ((u32(floor(((l_ez - l_origin) / l_c_30))) + l_iz) - l_half);
  if ((!coax_p_e2_meadow_held(l_gx_31, l_gz_32, l_kit)))
  {
    return l_o;
  }
  l_x_33 = coax_p_e2_meadow_at_x(l_gx_31, l_gz_32, l_kit);
  l_z_34 = coax_p_e2_meadow_at_z(l_gx_31, l_gz_32, l_kit);
  l_dx_35 = (l_x_33 - l_ex);
  l_dz_36 = (l_z_34 - l_ez);
  if ((l_m.z.z > l_m.z.x))
  {
    if (((((l_x_33 > l_m.z.x) && (l_x_33 < l_m.z.z)) && (l_z_34 > l_m.z.y)) && (l_z_34 < l_m.z.w)))
    {
      return l_o;
    }
    if (((l_m.x.w > 0.0f) && (((((l_x_33 < (l_m.z.x - l_m.x.w)) || (l_x_33 > (l_m.z.z + l_m.x.w))) || (l_z_34 < (l_m.z.y - l_m.x.w))) || (l_z_34 > (l_m.z.w + l_m.x.w))) || (coax_p_cg_hash(coax_p_e2_meadow_seed(l_gx_31, l_gz_32, l_kit), 0x1.200000p3f) > l_m.y.x))))
    {
      return l_o;
    }
  }
  l_k = coax_p_e2_meadow_fade(l_kit, (sqrt(((l_dx_35 * l_dx_35) + (l_dz_36 * l_dz_36))) / l_reach));
  if ((l_k <= 0.0f))
  {
    return l_o;
  }
  l_h = coax_p_e2_meadow_seed(l_gx_31, l_gz_32, l_kit);
  l_au_39 = coax_p_e2_meadow_gold(l_x_33, l_z_34);
  l_p = coax_p_e2_meadow_piece(l_kit, l_h, l_au_39);
  return coax_p_cg_meadow_place(l_id, coax_p_v4(l_x_33, (coax_p_exp_height_gpu(l_x_33, l_z_34) - l_p.w), l_z_34, (coax_p_cg_hash(l_h, 0x1.000000p4f) * 0x1.921cacp2f)), (l_p.y * l_k), (l_p.z * l_k), coax_p_v3(l_p.f4[0i], l_p.f4[1i], l_p.f4[2i]), l_au_39, l_m, l_gf, l_o.f8);
}
fn coax_p_cg_instanced_vs(p_id: u32, p_instance: u32, p_m: coax_t14, p_gf: coax_t25) -> coax_t23 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m: coax_t14 = p_m;
  var l_gf: coax_t25 = p_gf;
  var l_b: u32;
  var l_seed: u32;
  var l_source: vec3f;
  var l_center: vec3f;
  var l_sizes: vec2f;
  var l_angle: f32;
  var l_p: vec3f;
  var l_height: f32;
  var l_packed: u32;
  var l_nw: vec4f;
  var l_n: vec3f;
  var l_base: vec4f;
  var l_autumn: f32;
  var l_o: coax_t23;
  var l_rust: f32;
  var l_bottom: vec3f;
  var l_top: vec3f;
  l_b = (coax_p_wg_u32_f(l_m.y.w) + (l_id * 8u));
  l_seed = (coax_p_wg_u32_f(l_m.x.x) + ((l_instance & 65535u) * 8u));
  l_source = coax_p_v3(bitcast<f32>(people[i32(l_b)]), bitcast<f32>(people[i32((l_b + 1u))]), bitcast<f32>(people[i32((l_b + 2u))]));
  l_center = coax_p_v3(bitcast<f32>(people[i32(l_seed)]), bitcast<f32>(people[i32((l_seed + 1u))]), bitcast<f32>(people[i32((l_seed + 2u))]));
  l_sizes = coax_p_wg_unpack2x16unorm(people[i32((l_seed + 4u))]);
  l_angle = bitcast<f32>(people[i32((l_seed + 7u))]);
  l_p = coax_p_cg_instance_point(l_source, l_center, l_sizes.x, l_sizes.y, l_angle);
  l_height = coax_p_saturate(l_source.y);
  l_p.x = (l_p.x + ((((sin((((l_center.x * 0x1.ccccccp0f) + (l_center.z * 0x1.666666p-1f)) + (l_gf.x.z * 0x1.99999ap0f))) + (sin(((l_center.z * 0x1.59999ap1f) + (l_gf.x.z * 0x1.ccccccp-1f))) * 0x1.666666p-2f)) * 0x1.70a3d8p-5f) * l_height) * l_height));
  l_packed = people[i32((l_b + 3u))];
  l_nw = coax_p_wg_unpack4x8snorm(l_packed);
  l_n = coax_p_normalize3(coax_p_v3((((l_nw.x * cos(l_angle)) + (l_nw.z * sin(l_angle))) / l_sizes.y), (l_nw.y / l_sizes.x), ((((-l_nw.x) * sin(l_angle)) + (l_nw.z * cos(l_angle))) / l_sizes.y)));
  l_base = coax_p_wg_unpack4x8unorm(people[i32((l_seed + 5u))]);
  l_autumn = coax_p_cg_autumn(l_center.x, l_center.z);
  l_o = coax_t23();
  l_o.x = coax_p_fivelab_project(l_p, l_gf);
  l_o.y = l_p;
  l_o.z = l_n;
  l_o.w = coax_p_wg_unpack2x16unorm(people[i32((l_b + 4u))]);
  l_o.f4 = people[i32((l_b + 6u))];
  l_rust = ((coax_p_saturate(((l_center.x - 0x1.000000p-1f) / 0x1.800000p1f)) * coax_p_smoothstep(0x1.000000p-1f, 0x1.70a3d8p-1f, coax_p_cg_noise(coax_p_v2((l_center.x * 0x1.99999ap-1f), (l_center.z * 0x1.99999ap-1f))))) * (0x1.000000p0f - l_autumn));
  l_bottom = coax_p_mix3(coax_p_v3(l_base.x, l_base.y, l_base.z), coax_p_v3(0x1.1eb852p-1f, 0x1.5c28f6p-3f, 0x1.99999ap-6f), l_rust);
  l_top = coax_p_mix3(coax_p_v3((0x1.4cccccp-1f + (0x1.1eb852p-2f * l_autumn)), (0x1.c28f5cp-1f - (0x1.99999ap-3f * l_autumn)), 0x1.1eb852p-4f), coax_p_v3(0x1.e66666p-1f, 0x1.a3d70ap-2f, 0x1.1eb852p-5f), l_rust);
  l_o.f5 = coax_p_mix3(l_bottom, l_top, l_height);
  l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(0x1.000000p1f, l_height, 0x1.000000p0f);
  l_o.f8 = ((l_instance >> u32(16u)) - 1u);
  return l_o;
}
fn coax_p_cg_environment_vs(p_id: u32, p_instance: u32, p_m: coax_t14, p_gf: coax_t25) -> coax_t23 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m: coax_t14 = p_m;
  var l_gf: coax_t25 = p_gf;
  var l_b: u32;
  var l_p: vec3f;
  var l_sky_clip: vec4f;
  var l_seed_uv: vec2f;
  var l_uv: vec2f;
  var l_normal: u32;
  var l_kind: u32;
  var l_tint: vec4f;
  var l_wind: f32;
  var l_nw: vec4f;
  var l_normal3: vec3f;
  var l_local: vec3f;
  var l_cs: f32;
  var l_sn: f32;
  var l_o: coax_t23;
  l_b = (coax_p_wg_u32_f(l_m.y.w) + (l_id * 8u));
  l_p = coax_p_v3(bitcast<f32>(people[i32(l_b)]), bitcast<f32>(people[i32((l_b + 1u))]), bitcast<f32>(people[i32((l_b + 2u))]));
  l_sky_clip = coax_p_v4(l_p.x, l_p.y, 0x1.ffffdep-1f, 0x1.000000p0f);
  l_seed_uv = coax_p_wg_unpack2x16unorm(people[i32((l_b + 4u))]);
  l_uv = l_seed_uv;
  l_normal = people[i32((l_b + 3u))];
  l_kind = ((l_normal >> u32(24u)) & 15u);
  if ((l_kind == 0u))
  {
    l_p = coax_p_add3(coax_p_v3_of(l_gf.f4), coax_p_scale3(coax_p_cg_sky_ray(l_p.x, l_p.y), 0x1.680000p7f));
  }
  if ((l_kind == 6u))
  {
    l_uv = coax_p_v2(((l_seed_uv.x * 0x1.000000p6f) - 0x1.000000p5f), ((l_seed_uv.y * 0x1.000000p6f) - 0x1.000000p5f));
  }
  if (((l_kind == 4u) && (bitcast<f32>(people[i32((l_b + 7u))]) >= 0x1.900000p6f)))
  {
    l_uv = coax_p_v2(((l_seed_uv.x * 0x1.000000p3f) - 0x1.000000p2f), ((l_seed_uv.y * 0x1.000000p3f) - 0x1.000000p2f));
  }
  l_tint = coax_p_wg_unpack4x8unorm(people[i32((l_b + 5u))]);
  l_wind = (sin((((l_p.x * 0x1.ccccccp0f) + (l_p.z * 0x1.666666p-1f)) + (l_gf.x.z * 0x1.99999ap0f))) + (sin(((l_p.z * 0x1.59999ap1f) + (l_gf.x.z * 0x1.ccccccp-1f))) * 0x1.666666p-2f));
  if (((l_kind == 1u) || (l_kind == 2u)))
  {
    l_p.x = (l_p.x + ((l_wind * select(0x1.1eb852p-5f, 0x1.99999ap-6f, (l_kind == 2u))) * select(coax_p_saturate((l_p.y * 0x1.666666p-1f)), (coax_p_wg_unpack2x16unorm(people[i32((l_b + 4u))])).y, (l_kind == 2u))));
  }
  l_nw = coax_p_wg_unpack4x8snorm(l_normal);
  l_normal3 = coax_p_v3(l_nw.x, l_nw.y, l_nw.z);
  if ((l_m.w.w > 0x1.280000p4f))
  {
    l_p = coax_p_add3(coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z), coax_p_add3(coax_p_add3(coax_p_scale3(coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z), l_p.x), coax_p_scale3(coax_p_v3(l_m.f4.x, l_m.f4.y, l_m.f4.z), l_p.y)), coax_p_scale3(coax_p_v3(l_m.f5.x, l_m.f5.y, l_m.f5.z), l_p.z)));
    l_normal3 = coax_p_normalize3(coax_p_add3(coax_p_add3(coax_p_scale3(coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z), l_normal3.x), coax_p_scale3(coax_p_v3(l_m.f4.x, l_m.f4.y, l_m.f4.z), l_normal3.y)), coax_p_scale3(coax_p_v3(l_m.f5.x, l_m.f5.y, l_m.f5.z), l_normal3.z)));
  }
  else
  {
    if ((l_m.w.w > 0x1.180000p4f))
    {
      l_local = coax_p_sub3(l_p, coax_p_v3(l_m.y.x, l_m.y.y, l_m.y.z));
      if ((l_m.z.z > 0.0f))
      {
        l_local = coax_p_scale3(l_local, l_m.z.z);
      }
      l_cs = l_m.z.y;
      l_sn = l_m.z.x;
      l_p = coax_p_add3(coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z), coax_p_v3(((l_local.x * l_cs) + (l_local.z * l_sn)), l_local.y, (((-l_local.x) * l_sn) + (l_local.z * l_cs))));
      l_normal3 = coax_p_v3(((l_normal3.x * l_cs) + (l_normal3.z * l_sn)), l_normal3.y, (((-l_normal3.x) * l_sn) + (l_normal3.z * l_cs)));
    }
  }
  l_o = coax_t23();
  l_o.x = select(coax_p_fivelab_project(l_p, l_gf), l_sky_clip, (l_kind == 0u));
  l_o.y = l_p;
  l_o.z = l_normal3;
  l_o.w = l_uv;
  l_o.f4 = people[i32((l_b + 6u))];
  l_o.f5 = coax_p_v3(l_tint.x, l_tint.y, l_tint.z);
  l_o.f6 = coax_p_v4(select(0.0f, 0x1.000000p0f, ((l_m.w.w < 0x1.080000p4f) && (l_m.z.x > 0x1.000000p-1f))), 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(f32(l_kind), bitcast<f32>(people[i32((l_b + 7u))]), 0.0f);
  l_o.f8 = l_instance;
  if (((l_m.w.w > 0x1.280000p4f) && (l_m.x.w > 0.0f)))
  {
    l_o.f7.z = (l_tint.w - (0x1.000000p0f - l_m.x.w));
    l_o.f6.w = 0x1.a00000p3f;
  }
  return l_o;
}
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
}
fn coax_p_fivelab_block_vs(p_id: u32, p_instance: u32, p_m: coax_t14, p_gf: coax_t25) -> coax_t23 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m: coax_t14 = p_m;
  var l_gf: coax_t25 = p_gf;
  var l_boxes: array<f32, 104>;
  var l_corner_a: array<f32, 6>;
  var l_corner_b: array<f32, 6>;
  var l_v: i32;
  var l_q: i32;
  var l_k: i32;
  var l_face: i32;
  var l_axis: i32;
  var l_side: f32;
  var l_ca: f32;
  var l_cb: f32;
  var l_bones: bool;
  var l_thin: f32;
  var l_h: vec3f;
  var l_c: vec3f;
  var l_nl: vec3f;
  var l_local: vec3f;
  var l_at4: vec4f;
  var l_joint: u32;
  var l_r: u32;
  var l_ra: vec4f;
  var l_rc: vec4f;
  var l_re: vec4f;
  var l_p: vec3f;
  var l_n: vec3f;
  var l_bs: f32;
  var l_r2: u32;
  var l_sa: vec4f;
  var l_sc: vec4f;
  var l_se: vec4f;
  var l_cs: f32;
  var l_sn: f32;
  var l_w: vec3f;
  var l_o: coax_t23;
  var l_what: f32;
  var l_cloth: vec3f;
  var l_t: vec3f;
  l_boxes = array<f32, 104>(0x1.000000p0f, 0.0f, 0x1.e66666p-1f, (-0x1.47ae14p-5f), 0x1.47ae14p-3f, 0x1.70a3d8p-4f, 0x1.c28f5cp-4f, 0x1.000000p0f, 0x1.000000p2f, 0.0f, 0x1.4cccccp0f, (-0x1.eb851ep-6f), 0x1.5c28f6p-3f, 0x1.99999ap-3f, 0x1.c28f5cp-4f, 0x1.000000p0f, 0x1.800000p2f, 0.0f, 0x1.a8f5c2p0f, (-0x1.47ae14p-7f), 0x1.851eb8p-4f, 0x1.eb851ep-4f, 0x1.c28f5cp-4f, 0.0f, 0x1.000000p3f, 0x1.449ba6p-2f, 0x1.74bc6ap0f, (-0x1.1a9fbep-4f), 0x1.020c4ap-3f, 0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.000000p0f, 0x1.200000p3f, 0x1.20418ap-1f, 0x1.74bc6ap0f, (-0x1.1eb852p-4f), 0x1.eb851ep-4f, 0x1.47ae14p-5f, 0x1.47ae14p-5f, 0x1.000000p1f, 0x1.000000p5f, (-0x1.449ba6p-2f), 0x1.74bc6ap0f, (-0x1.1a9fbep-4f), 0x1.020c4ap-3f, 0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.000000p0f, 0x1.080000p5f, (-0x1.20418ap-1f), 0x1.74bc6ap0f, (-0x1.1eb852p-4f), 0x1.eb851ep-4f, 0x1.47ae14p-5f, 0x1.47ae14p-5f, 0x1.000000p1f, 0x1.b80000p5f, 0x1.74bc6ap-4f, 0x1.83126ep-1f, (-0x1.26e978p-5f), 0x1.333334p-4f, 0x1.b851ecp-3f, 0x1.333334p-4f, 0x1.000000p0f, 0x1.c00000p5f, 0x1.74bc6ap-4f, 0x1.418938p-2f, (-0x1.99999ap-5f), 0x1.eb851ep-5f, 0x1.d2f1aap-3f, 0x1.eb851ep-5f, 0x1.000000p1f, 0x1.c80000p5f, 0x1.74bc6ap-4f, 0x1.99999ap-5f, 0.0f, 0x1.c28f5cp-5f, 0x1.99999ap-5f, 0x1.eb851ep-4f, 0x1.800000p1f, 0x1.e00000p5f, (-0x1.74bc6ap-4f), 0x1.83126ep-1f, (-0x1.26e978p-5f), 0x1.333334p-4f, 0x1.b851ecp-3f, 0x1.333334p-4f, 0x1.000000p0f, 0x1.e80000p5f, (-0x1.74bc6ap-4f), 0x1.418938p-2f, (-0x1.99999ap-5f), 0x1.eb851ep-5f, 0x1.d2f1aap-3f, 0x1.eb851ep-5f, 0x1.000000p1f, 0x1.f00000p5f, (-0x1.74bc6ap-4f), 0x1.99999ap-5f, 0.0f, 0x1.c28f5cp-5f, 0x1.99999ap-5f, 0x1.eb851ep-4f, 0x1.800000p1f);
  l_corner_a = array<f32, 6>((-0x1.000000p0f), 0x1.000000p0f, 0x1.000000p0f, (-0x1.000000p0f), 0x1.000000p0f, (-0x1.000000p0f));
  l_corner_b = array<f32, 6>((-0x1.000000p0f), (-0x1.000000p0f), 0x1.000000p0f, (-0x1.000000p0f), 0x1.000000p0f, 0x1.000000p0f);
  l_v = i32(l_id);
  l_q = ((l_v / 36i) * 8i);
  l_k = (l_v % 36i);
  l_face = (l_k / 6i);
  l_axis = (l_face / 2i);
  l_side = select(0x1.000000p0f, (-0x1.000000p0f), ((l_face % 2i) == 0i));
  l_ca = l_corner_a[(l_k % 6i)];
  l_cb = l_corner_b[(l_k % 6i)];
  l_bones = (coax_p_wg_fmod(l_m.w.w, 0x1.000000p1f) > 0x1.000000p-1f);
  l_thin = 0x1.000000p0f;
  if (l_bones)
  {
    l_thin = select(0x1.19999ap-1f, 0x1.b33334p-1f, (l_q == 16i));
  }
  l_h = coax_p_v3((l_boxes[(l_q + 4i)] * l_thin), l_boxes[(l_q + 5i)], (l_boxes[(l_q + 6i)] * l_thin));
  l_c = coax_p_v3(l_ca, l_cb, l_side);
  l_nl = coax_p_v3(0.0f, 0.0f, l_side);
  if ((l_axis == 0i))
  {
    l_c = coax_p_v3(l_side, l_ca, l_cb);
    l_nl = coax_p_v3(l_side, 0.0f, 0.0f);
  }
  if ((l_axis == 1i))
  {
    l_c = coax_p_v3(l_cb, l_side, l_ca);
    l_nl = coax_p_v3(0.0f, l_side, 0.0f);
  }
  l_local = coax_p_add3(coax_p_v3(l_boxes[(l_q + 1i)], l_boxes[(l_q + 2i)], l_boxes[(l_q + 3i)]), coax_p_mul3(l_c, l_h));
  l_at4 = coax_p_v4(l_local.x, l_local.y, l_local.z, 0x1.000000p0f);
  l_joint = u32(i32(l_boxes[l_q]));
  l_r = (coax_p_wg_u32_f(l_m.y.x) + (l_joint * 3u));
  l_ra = bake[i32(l_r)];
  l_rc = bake[i32((l_r + 1u))];
  l_re = bake[i32((l_r + 2u))];
  l_p = coax_p_v3(coax_p_wg_dot4(l_ra, l_at4), coax_p_wg_dot4(l_rc, l_at4), coax_p_wg_dot4(l_re, l_at4));
  l_n = coax_p_v3(dot(coax_p_v3(l_ra.x, l_ra.y, l_ra.z), l_nl), dot(coax_p_v3(l_rc.x, l_rc.y, l_rc.z), l_nl), dot(coax_p_v3(l_re.x, l_re.y, l_re.z), l_nl));
  l_bs = coax_p_fivelab_gait_share(l_m.y.z, l_joint);
  if ((l_bs > 0x1.0624dep-10f))
  {
    l_r2 = (coax_p_wg_u32_f(l_m.y.y) + (l_joint * 3u));
    l_sa = bake[i32(l_r2)];
    l_sc = bake[i32((l_r2 + 1u))];
    l_se = bake[i32((l_r2 + 2u))];
    l_p = coax_p_mix3(l_p, coax_p_v3(coax_p_wg_dot4(l_sa, l_at4), coax_p_wg_dot4(l_sc, l_at4), coax_p_wg_dot4(l_se, l_at4)), l_bs);
    l_n = coax_p_mix3(l_n, coax_p_v3(dot(coax_p_v3(l_sa.x, l_sa.y, l_sa.z), l_nl), dot(coax_p_v3(l_sc.x, l_sc.y, l_sc.z), l_nl), dot(coax_p_v3(l_se.x, l_se.y, l_se.z), l_nl)), l_bs);
  }
  l_cs = cos(l_m.x.w);
  l_sn = sin(l_m.x.w);
  l_w = coax_p_v3((((l_p.x * l_cs) + (l_p.z * l_sn)) + l_m.x.x), (l_p.y + l_m.x.y), ((((-l_p.x) * l_sn) + (l_p.z * l_cs)) + l_m.x.z));
  l_o = coax_t23();
  l_o.x = coax_p_fivelab_project(l_w, l_gf);
  l_o.y = l_w;
  l_o.z = coax_p_v3(((l_n.x * l_cs) + (l_n.z * l_sn)), l_n.y, (((-l_n.x) * l_sn) + (l_n.z * l_cs)));
  l_o.w = coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f);
  l_o.f7 = l_local;
  l_o.f8 = l_instance;
  l_o.f4 = 0u;
  l_what = l_boxes[(l_q + 7i)];
  l_cloth = coax_p_mix3(coax_p_v3(0x1.ae147ap-2f, 0x1.5c28f6p-2f, 0x1.eb851ep-3f), coax_p_v3(l_m.f4.x, l_m.f4.y, l_m.f4.z), coax_p_clampf(l_m.f4.w, 0.0f, 0x1.000000p0f));
  l_t = l_cloth;
  if ((l_what < 0x1.000000p-1f))
  {
    l_t = coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z);
  }
  if (((l_what > 0x1.800000p0f) && (l_what < 0x1.400000p1f)))
  {
    l_t = coax_p_scale3(l_cloth, 0x1.800000p-1f);
  }
  if ((l_what > 0x1.400000p1f))
  {
    l_t = coax_p_v3(0x1.99999ap-3f, 0x1.1eb852p-3f, 0x1.70a3d8p-4f);
  }
  if (l_bones)
  {
    l_t = coax_p_v3(0x1.3d70a4p-1f, 0x1.28f5c2p-1f, 0x1.e147aep-2f);
    if (((l_q == 0i) || (l_q == 8i)))
    {
      l_t = coax_p_v3(0x1.47ae14p-4f, 0x1.1eb852p-4f, 0x1.eb851ep-5f);
    }
  }
  l_o.f5 = l_t;
  l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.400000p3f);
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
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_fivelab_gaunt(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_q: vec3f;
  var l_side: f32;
  l_q = l_p;
  if ((abs(l_p.x) < 0x1.eb851ep-3f))
  {
    if (((l_p.y > 0x1.ccccccp-1f) && (l_p.y < 0x1.800000p0f)))
    {
      l_q = coax_p_v3((l_p.x * 0x1.7ae148p-1f), l_p.y, (l_p.z * 0x1.51eb86p-1f));
    }
    if ((l_p.y <= 0x1.ccccccp-1f))
    {
      l_side = select(0x1.99999ap-4f, (-0x1.99999ap-4f), (l_p.x < 0.0f));
      l_q = coax_p_v3((l_side + ((l_p.x - l_side) * 0x1.19999ap-1f)), l_p.y, (l_p.z * 0x1.19999ap-1f));
    }
  }
  else
  {
    if ((l_p.y > 0x1.266666p0f))
    {
      l_q = coax_p_v3(l_p.x, (0x1.666666p0f + ((l_p.y - 0x1.666666p0f) * 0x1.19999ap-1f)), (l_p.z * 0x1.19999ap-1f));
    }
  }
  return l_q;
}
fn coax_p_wg_unpack4x8snorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4(coax_p_wg_snorm8(coax_p_wg_byte(l_x, 0u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 8u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 16u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 24u)));
}
fn coax_p_fivelab_skin_mix(p_ra: u32, p_rb: u32, p_code: f32, p_joints: u32, p_weights: u32, p_at: vec4f, p_nw: vec3f, p_arm: bool) -> coax_t26 {
  var l_ra: u32 = p_ra;
  var l_rb: u32 = p_rb;
  var l_code: f32 = p_code;
  var l_joints: u32 = p_joints;
  var l_weights: u32 = p_weights;
  var l_at: vec4f = p_at;
  var l_nw: vec3f = p_nw;
  var l_arm: bool = p_arm;
  var l_o: coax_t26;
  var l_k: u32;
  var l_w: f32;
  var l_j: u32;
  var l_s: f32;
  var l_m: coax_t33;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_e: vec4f;
  var l_m2: coax_t33;
  l_o = coax_t26();
  l_o.x = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.y = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      l_w = (f32(coax_p_wg_byte(l_weights, (l_k * 8u))) / 0x1.fe0000p7f);
      if ((l_w > 0.0f))
      {
        l_j = coax_p_wg_byte(l_joints, (l_k * 8u));
        l_s = coax_p_fivelab_gait_share(l_code, l_j);
        l_m = coax_p_fivelab_rows(l_ra, l_j, l_arm);
        l_a = l_m.x;
        l_c = l_m.y;
        l_e = l_m.z;
        if ((l_s > 0x1.0624dep-10f))
        {
          l_m2 = coax_p_fivelab_rows(l_rb, l_j, l_arm);
          l_a = coax_p_fivelab_mix4(l_a, l_m2.x, l_s);
          l_c = coax_p_fivelab_mix4(l_c, l_m2.y, l_s);
          l_e = coax_p_fivelab_mix4(l_e, l_m2.z, l_s);
        }
        l_o.x = coax_p_add3(l_o.x, coax_p_scale3(coax_p_v3(coax_p_wg_dot4(l_a, l_at), coax_p_wg_dot4(l_c, l_at), coax_p_wg_dot4(l_e, l_at)), l_w));
        l_o.y = coax_p_add3(l_o.y, coax_p_scale3(coax_p_v3(dot(coax_p_v3(l_a.x, l_a.y, l_a.z), l_nw), dot(coax_p_v3(l_c.x, l_c.y, l_c.z), l_nw), dot(coax_p_v3(l_e.x, l_e.y, l_e.z), l_nw)), l_w));
      }
      l_k = (l_k + 1u);
    }
  }
  return l_o;
}
fn coax_p_fivelab_skin(p_row: u32, p_joints: u32, p_weights: u32, p_at: vec4f, p_nw: vec3f, p_arm: bool) -> coax_t26 {
  var l_row: u32 = p_row;
  var l_joints: u32 = p_joints;
  var l_weights: u32 = p_weights;
  var l_at: vec4f = p_at;
  var l_nw: vec3f = p_nw;
  var l_arm: bool = p_arm;
  var l_o: coax_t26;
  var l_k: u32;
  var l_w: f32;
  var l_m: coax_t33;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_e: vec4f;
  l_o = coax_t26();
  l_o.x = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.y = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      l_w = (f32(coax_p_wg_byte(l_weights, (l_k * 8u))) / 0x1.fe0000p7f);
      if ((l_w > 0.0f))
      {
        l_m = coax_p_fivelab_rows(l_row, coax_p_wg_byte(l_joints, (l_k * 8u)), l_arm);
        l_a = l_m.x;
        l_c = l_m.y;
        l_e = l_m.z;
        l_o.x = coax_p_add3(l_o.x, coax_p_scale3(coax_p_v3(coax_p_wg_dot4(l_a, l_at), coax_p_wg_dot4(l_c, l_at), coax_p_wg_dot4(l_e, l_at)), l_w));
        l_o.y = coax_p_add3(l_o.y, coax_p_scale3(coax_p_v3(dot(coax_p_v3(l_a.x, l_a.y, l_a.z), l_nw), dot(coax_p_v3(l_c.x, l_c.y, l_c.z), l_nw), dot(coax_p_v3(l_e.x, l_e.y, l_e.z), l_nw)), l_w));
      }
      l_k = (l_k + 1u);
    }
  }
  return l_o;
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_fivelab_project(p_w: vec3f, p_gf: coax_t25) -> vec4f {
  var l_w: vec3f = p_w;
  var l_gf: coax_t25 = p_gf;
  var l_rel: vec3f;
  var l_z: f32;
  var l_x: f32;
  var l_y: f32;
  var l_f: f32;
  var l_cx: f32;
  var l_cy: f32;
  var l_cz: f32;
  l_rel = coax_p_sub3(l_w, coax_p_v3_of(l_gf.f4));
  l_z = dot(l_rel, coax_p_v3_of(l_gf.f5));
  l_x = dot(l_rel, coax_p_v3_of(l_gf.f6));
  l_y = dot(l_rel, coax_p_v3_of(l_gf.f7));
  l_f = coax_p_fmax(l_gf.f4.w, 0x1.000000p0f);
  l_cx = (((((0x1.000000p1f * l_gf.f5.w) / l_gf.x.x) - 0x1.000000p0f) * l_z) + (((0x1.000000p1f * l_f) * l_x) / l_gf.x.x));
  l_cy = (((0x1.000000p0f - ((0x1.000000p1f * l_gf.f6.w) / l_gf.x.y)) * l_z) + (((0x1.000000p1f * l_f) * l_y) / l_gf.x.y));
  l_cz = ((0x1.c20000p9f * (l_z - 0x1.99999ap-4f)) / (0x1.c20000p9f - 0x1.99999ap-4f));
  return coax_p_v4(l_cx, l_cy, l_cz, l_z);
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_wg_fract_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y));
}
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_wg_abs_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(abs(l_p0.x), abs(l_p0.y));
}
fn coax_p_wg_div_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x / l_b.x), (l_a.y / l_b.y));
}
fn coax_p_render_sample_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_result: coax_t34;
  l_rs_result = coax_p_render_filter_layer8_grad(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
}
fn coax_p_engine_terrain_blend_t(p_x: f32, p_z: f32, p_f1: f32, p_w1: f32, p_o1: f32, p_f2: f32, p_w2: f32, p_o2: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_f1: f32 = p_f1;
  var l_w1: f32 = p_w1;
  var l_o1: f32 = p_o1;
  var l_f2: f32 = p_f2;
  var l_w2: f32 = p_w2;
  var l_o2: f32 = p_o2;
  var l_t: f32;
  l_t = (coax_p_engine_terrain_noise(((l_x * l_f1) + l_o1), ((l_z * l_f1) + l_o1)) * l_w1);
  if ((l_w2 > 0.0f))
  {
    l_t = (l_t + (coax_p_engine_terrain_noise(((l_x * l_f2) + l_o2), ((l_z * l_f2) + l_o2)) * l_w2));
  }
  return l_t;
}
fn coax_p_engine_terrain_gain(p_x: f32, p_z: f32, p_g0: f32, p_g1: f32, p_f3: f32, p_o3: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_g0: f32 = p_g0;
  var l_g1: f32 = p_g1;
  var l_f3: f32 = p_f3;
  var l_o3: f32 = p_o3;
  if ((l_g1 > 0.0f))
  {
    return (l_g0 + (l_g1 * coax_p_engine_terrain_noise(((l_x * l_f3) + l_o3), ((l_z * l_f3) + l_o3))));
  }
  return l_g0;
}
fn coax_p_engine_terrain_decode(p_a: f32, p_b: f32, p_t: f32, p_scale: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  var l_scale: f32 = p_scale;
  return coax_pow_bg_vs(coax_p_fmax((coax_p_engine_terrain_mix(l_a, l_b, l_t) * l_scale), 0.0f), 0x1.19999ap1f);
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t28 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t28(l_x, l_y);
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
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t27 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t27(l_x, l_y);
}
fn coax_p_chat_segment_distance(p_p: vec2f, p_a: vec2f, p_b: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  var l_pa: vec2f;
  var l_ba: vec2f;
  var l_h: f32;
  l_pa = coax_p_wg_sub_f2f2(l_p, l_a);
  l_ba = coax_p_wg_sub_f2f2(l_b, l_a);
  l_h = coax_p_clampf((coax_p_wg_dot2(l_pa, l_ba) / coax_p_fmax(coax_p_wg_dot2(l_ba, l_ba), 0x1.0c6f7ap-20f)), 0.0f, 0x1.000000p0f);
  return coax_p_wg_length2(coax_p_wg_sub_f2f2(l_pa, coax_p_wg_mul_f2f(l_ba, l_h)));
}
fn coax_p_wg_mul_f4f4(p_a: vec4f, p_b: vec4f) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return coax_p_v4((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z), (l_a.w * l_b.w));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_chat_quad_sdf_impl(p_corner_center_to_point: vec2f, p_corner_radius: f32) -> f32 {
  var l_corner_center_to_point: vec2f = p_corner_center_to_point;
  var l_corner_radius: f32 = p_corner_radius;
  var l_signed_distance_to_inset_quad: f32;
  if ((l_corner_radius == 0.0f))
  {
    return coax_p_fmax(l_corner_center_to_point.x, l_corner_center_to_point.y);
  }
  l_signed_distance_to_inset_quad = (coax_p_wg_length2(coax_p_wg_max_f2f2(coax_p_v2(0.0f, 0.0f), l_corner_center_to_point)) + coax_p_fmin(0.0f, coax_p_fmax(l_corner_center_to_point.x, l_corner_center_to_point.y)));
  return (l_signed_distance_to_inset_quad - l_corner_radius);
}
fn coax_p_wg_mix_f4f4f(p_p0: vec4f, p_p1: vec4f, p_p2: f32) -> vec4f {
  var l_p0: vec4f = p_p0;
  var l_p1: vec4f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v4(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2), coax_p_mix(l_p0.w, l_p1.w, l_p2));
}
fn coax_p_wg_mul_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a * l_b.x), (l_a * l_b.y));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_chat_blur_along_x(p_x: f32, p_y: f32, p_sigma: f32, p_corner: f32, p_half_size: vec2f) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_sigma: f32 = p_sigma;
  var l_corner: f32 = p_corner;
  var l_half_size: vec2f = p_half_size;
  var l_delta: f32;
  var l_curved: f32;
  var l_integral: vec2f;
  l_delta = coax_p_fmin(((l_half_size.y - l_corner) - abs(l_y)), 0.0f);
  l_curved = ((l_half_size.x - l_corner) + sqrt(coax_p_fmax(0.0f, ((l_corner * l_corner) - (l_delta * l_delta)))));
  l_integral = coax_p_wg_add_ff2(0x1.000000p-1f, coax_p_wg_mul_ff2(0x1.000000p-1f, coax_p_chat_erf2(coax_p_wg_mul_f2f(coax_p_wg_add_ff2(l_x, coax_p_v2((-l_curved), l_curved)), (sqrt(0x1.000000p-1f) / l_sigma)))));
  return (l_integral.y - l_integral.x);
}
fn coax_p_chat_gaussian(p_x: f32, p_sigma: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_sigma: f32 = p_sigma;
  return (exp(((-(l_x * l_x)) / ((0x1.000000p1f * l_sigma) * l_sigma))) / (sqrt((0x1.000000p1f * 0x1.921fb6p1f)) * l_sigma));
}
fn coax_p_chat_over(p_below: vec4f, p_above: vec4f) -> vec4f {
  var l_below: vec4f = p_below;
  var l_above: vec4f = p_above;
  var l_alpha: f32;
  l_alpha = (l_above.w + (l_below.w * (0x1.000000p0f - l_above.w)));
  return coax_p_wg_make_f4_f3f(coax_p_wg_div_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(l_above.x, l_above.y, l_above.z), l_above.w), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_below.x, l_below.y, l_below.z), l_below.w), (0x1.000000p0f - l_above.w))), l_alpha), l_alpha);
}
fn coax_p_e2_field_cell(p_t: i32) -> f32 {
  var l_t: i32 = p_t;
  return select(select(select(0x1.000000p2f, 0x1.000000p1f, (l_t == 2i)), 0x1.000000p0f, (l_t == 1i)), 0x1.000000p-1f, (l_t == 0i));
}
fn coax_p_e2_field_fine_x(p_t: i32, p_x: u32, p_z: u32) -> u32 {
  var l_t: i32 = p_t;
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_fx: u32;
  var l_fz: u32;
  var l_p: u32;
  var l_p_6: u32;
  var l_p_7: u32;
  l_fx = l_x;
  l_fz = l_z;
  if ((l_t >= 3i))
  {
    l_p = coax_p_e2_field_pick(l_fx, l_fz, 3u);
    l_fx = ((l_fx * 2u) + (l_p & 1u));
    l_fz = ((l_fz * 2u) + (l_p >> u32(1u)));
  }
  if ((l_t >= 2i))
  {
    l_p_6 = coax_p_e2_field_pick(l_fx, l_fz, 2u);
    l_fx = ((l_fx * 2u) + (l_p_6 & 1u));
    l_fz = ((l_fz * 2u) + (l_p_6 >> u32(1u)));
  }
  if ((l_t >= 1i))
  {
    l_p_7 = coax_p_e2_field_pick(l_fx, l_fz, 1u);
    l_fx = ((l_fx * 2u) + (l_p_7 & 1u));
    l_fz = ((l_fz * 2u) + (l_p_7 >> u32(1u)));
  }
  return l_fx;
}
fn coax_p_e2_field_fine_z(p_t: i32, p_x: u32, p_z: u32) -> u32 {
  var l_t: i32 = p_t;
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_fx: u32;
  var l_fz: u32;
  var l_p: u32;
  var l_p_6: u32;
  var l_p_7: u32;
  l_fx = l_x;
  l_fz = l_z;
  if ((l_t >= 3i))
  {
    l_p = coax_p_e2_field_pick(l_fx, l_fz, 3u);
    l_fx = ((l_fx * 2u) + (l_p & 1u));
    l_fz = ((l_fz * 2u) + (l_p >> u32(1u)));
  }
  if ((l_t >= 2i))
  {
    l_p_6 = coax_p_e2_field_pick(l_fx, l_fz, 2u);
    l_fx = ((l_fx * 2u) + (l_p_6 & 1u));
    l_fz = ((l_fz * 2u) + (l_p_6 >> u32(1u)));
  }
  if ((l_t >= 1i))
  {
    l_p_7 = coax_p_e2_field_pick(l_fx, l_fz, 1u);
    l_fx = ((l_fx * 2u) + (l_p_7 & 1u));
    l_fz = ((l_fz * 2u) + (l_p_7 >> u32(1u)));
  }
  return l_fz;
}
fn coax_p_e2_field_hash(p_x: u32, p_z: u32, p_k: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_k: u32 = p_k;
  return coax_p_e2_ihash((((l_x * 73856093u) ^ (l_z * 19349663u)) ^ ((l_k + 1u) * 83492791u)));
}
fn coax_p_e2_unit(p_h: u32) -> f32 {
  var l_h: u32 = p_h;
  return (f32((l_h >> u32(8u))) / 0x1.000000p24f);
}
fn coax_p_e2_field_start_r(p_t: i32, p_g: f32, p_c: f32) -> f32 {
  var l_t: i32 = p_t;
  var l_g: f32 = p_g;
  var l_c: f32 = p_c;
  if (((l_t == 0i) || ((l_t == 1i) && coax_p_e2_field_fine_off(l_c))))
  {
    return 0.0f;
  }
  return ((0x1.000000p4f + (f32(l_t) * 0x1.800000p3f)) * l_g);
}
fn coax_p_e2_field_out_r(p_t: i32, p_g: f32, p_c: f32) -> f32 {
  var l_t: i32 = p_t;
  var l_g: f32 = p_g;
  var l_c: f32 = p_c;
  if ((l_t == 3i))
  {
    return (0x1.180000p7f * l_g);
  }
  if (((l_t == 0i) && coax_p_e2_field_fine_off(l_c)))
  {
    return 0.0f;
  }
  return ((0x1.000000p4f + (f32((l_t + 1i)) * 0x1.800000p3f)) * l_g);
}
fn coax_p_e2_field_rel_r(p_t: i32, p_d: f32, p_g: f32, p_c: f32) -> f32 {
  var l_t: i32 = p_t;
  var l_d: f32 = p_d;
  var l_g: f32 = p_g;
  var l_c: f32 = p_c;
  var l_k: f32;
  var l_s: f32;
  var l_v: f32;
  var l_v_7: f32;
  l_k = select(0x1.99999ap-5f, l_g, (l_g > 0x1.99999ap-5f));
  if ((l_t == 3i))
  {
    l_s = ((0x1.000000p4f + (0x1.800000p1f * 0x1.800000p3f)) * l_k);
    l_v = (0x1.000000p0f - ((l_d - l_s) / ((0x1.180000p7f * l_k) - l_s)));
    return select(select(l_v, 0x1.000000p0f, (l_v > 0x1.000000p0f)), 0.0f, (l_v < 0.0f));
  }
  l_v_7 = coax_pow_bg_vs(0x1.000000p-2f, ((l_d - ((0x1.000000p4f + (f32(l_t) * 0x1.800000p3f)) * l_k)) / (0x1.800000p3f * l_k)));
  l_v_7 = select(l_v_7, 0x1.000000p0f, (l_v_7 > 0x1.000000p0f));
  if (((l_t == 0i) && (l_v_7 > l_c)))
  {
    l_v_7 = l_c;
  }
  return l_v_7;
}
fn coax_p_e2_field_rank(p_t: i32, p_x: u32, p_z: u32, p_fx: u32, p_fz: u32) -> f32 {
  var l_t: i32 = p_t;
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_fx: u32 = p_fx;
  var l_fz: u32 = p_fz;
  var l_u: f32;
  var l_p: u32;
  l_u = coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 5u));
  if ((l_t == 3i))
  {
    return (0x1.0624dep-10f + (0x1.ff7ceep-1f * l_u));
  }
  l_p = coax_p_e2_field_pick((l_x >> u32(1u)), (l_z >> u32(1u)), u32((l_t + 1i)));
  if ((((l_x & 1u) == (l_p & 1u)) && ((l_z & 1u) == (l_p >> u32(1u)))))
  {
    return 0.0f;
  }
  return (0x1.000000p-2f + (0x1.800000p-1f * l_u));
}
fn coax_p_e2_field_shown(p_rel: f32, p_rank: f32) -> f32 {
  var l_rel: f32 = p_rel;
  var l_rank: f32 = p_rank;
  var l_v: f32;
  l_v = ((l_rel - l_rank) / 0x1.47ae14p-4f);
  return select(select(l_v, 0x1.000000p0f, (l_v > 0x1.000000p0f)), 0.0f, (l_v < 0.0f));
}
fn coax_p_e2_meadow_gold(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  return (coax_p_e2_smooth(0x1.99999ap-2f, 0x1.3d70a4p-1f, coax_p_e2_noise(((l_x / 0x1.e00000p5f) + 0x1.8cccccp1f), ((l_z / 0x1.e00000p5f) - 0x1.b33334p0f))) * 0x1.333334p-2f);
}
fn coax_p_exp_height_gpu(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_t: array<f32, 192>;
  var l_h: f32;
  var l_i: i32;
  var l_o: i32;
  var l_d: f32;
  var l_u: f32;
  l_t = coax_p_exp_ground_sites();
  l_h = coax_p_cg_height(l_x, l_z);
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 32i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      l_o = (l_i * 6i);
      l_h = coax_p_exp_ground_level(l_h, l_t[l_o], l_t[(l_o + 1i)], l_t[(l_o + 2i)], l_t[(l_o + 3i)], l_t[(l_o + 4i)], l_t[(l_o + 5i)], l_x, l_z);
    }
  }
  l_d = coax_p_exp_ground_castle_distance(l_x, l_z);
  if ((l_d < 0x1.000000p2f))
  {
    l_u = select((l_d / 0x1.000000p2f), 0.0f, (l_d <= 0.0f));
    l_u = ((l_u * l_u) * (0x1.800000p1f - (0x1.000000p1f * l_u)));
    l_h = ((coax_p_cg_height(0x1.200000p4f, (-0x1.400000p4f)) * (0x1.000000p0f - l_u)) + (l_h * l_u));
  }
  return l_h;
}
fn coax_p_e2_meadow_tuft_sv(p_fx: u32, p_fz: u32) -> f32 {
  var l_fx: u32 = p_fx;
  var l_fz: u32 = p_fz;
  return ((0x1.c28f5cp-3f + (0x1.0a3d70p-2f * coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 4u)))) / 0x1.4fcce2p0f);
}
fn coax_p_e2_meadow_tuft_sh(p_fx: u32, p_fz: u32) -> f32 {
  var l_fx: u32 = p_fx;
  var l_fz: u32 = p_fz;
  return ((0x1.19999ap-1f + (0x1.666666p-2f * coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 7u)))) * 0x1.b33334p-1f);
}
fn coax_p_cg_meadow_place(p_id: u32, p_a: vec4f, p_sv: f32, p_sh: f32, p_tint: vec3f, p_autumn: f32, p_m: coax_t14, p_gf: coax_t25, p_who: u32) -> coax_t23 {
  var l_id: u32 = p_id;
  var l_a: vec4f = p_a;
  var l_sv: f32 = p_sv;
  var l_sh: f32 = p_sh;
  var l_tint: vec3f = p_tint;
  var l_autumn: f32 = p_autumn;
  var l_m: coax_t14 = p_m;
  var l_gf: coax_t25 = p_gf;
  var l_who: u32 = p_who;
  var l_b: u32;
  var l_source: vec3f;
  var l_normal: u32;
  var l_kind: u32;
  var l_p: vec3f;
  var l_sway: f32;
  var l_nw: vec4f;
  var l_cs: f32;
  var l_sn: f32;
  var l_o: coax_t23;
  var l_vt: vec4f;
  var l_height: f32;
  l_b = (coax_p_wg_u32_f(l_m.y.w) + (l_id * 8u));
  l_source = coax_p_v3(bitcast<f32>(people[i32(l_b)]), bitcast<f32>(people[i32((l_b + 1u))]), bitcast<f32>(people[i32((l_b + 2u))]));
  l_normal = people[i32((l_b + 3u))];
  l_kind = ((l_normal >> u32(24u)) & 7u);
  l_p = coax_p_cg_instance_point(l_source, coax_p_v3(l_a.x, l_a.y, l_a.z), l_sv, l_sh, l_a.w);
  if ((((l_kind == 1u) || (l_kind == 2u)) || (l_kind == 3u)))
  {
    l_sway = (sin((((l_a.x * 0x1.7ae148p-2f) + (l_a.z * 0x1.ae147ap-3f)) + (l_gf.x.z * 0x1.19999ap0f))) + (sin(((l_a.z * 0x1.0f5c28p-1f) + (l_gf.x.z * 0x1.666666p-1f))) * 0x1.666666p-2f));
    l_p.x = (l_p.x + (((l_sway * 0x1.99999ap-5f) * coax_p_saturate((l_source.y * 0x1.70a3d8p-3f))) * l_sv));
  }
  l_nw = coax_p_wg_unpack4x8snorm(l_normal);
  l_cs = cos(l_a.w);
  l_sn = sin(l_a.w);
  l_o = coax_t23();
  l_o.x = coax_p_fivelab_project(l_p, l_gf);
  l_o.y = l_p;
  l_o.z = coax_p_normalize3(coax_p_v3((((l_nw.x * l_cs) + (l_nw.z * l_sn)) / l_sh), (l_nw.y / l_sv), ((((-l_nw.x) * l_sn) + (l_nw.z * l_cs)) / l_sh)));
  l_o.w = coax_p_wg_unpack2x16unorm(people[i32((l_b + 4u))]);
  if ((l_kind == 6u))
  {
    l_o.w = coax_p_v2(((l_o.w.x * 0x1.000000p6f) - 0x1.000000p5f), ((l_o.w.y * 0x1.000000p6f) - 0x1.000000p5f));
  }
  l_vt = coax_p_wg_unpack4x8unorm(people[i32((l_b + 5u))]);
  l_o.f4 = people[i32((l_b + 6u))];
  l_o.f5 = select(select(coax_p_v3(l_vt.x, l_vt.y, l_vt.z), coax_p_mul3(coax_p_v3(l_vt.x, l_vt.y, l_vt.z), l_tint), (l_kind == 4u)), l_tint, (l_kind == 1u));
  l_o.f6 = coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(f32(l_kind), bitcast<f32>(people[i32((l_b + 7u))]), 0.0f);
  l_o.f8 = l_who;
  if ((l_kind == 2u))
  {
    l_height = coax_p_saturate(l_source.y);
    l_o.f5 = coax_p_mix3(l_tint, coax_p_v3((0x1.4cccccp-1f + (0x1.1eb852p-2f * l_autumn)), (0x1.c28f5cp-1f - (0x1.99999ap-3f * l_autumn)), 0x1.1eb852p-4f), l_height);
    l_o.f7 = coax_p_v3(0x1.000000p1f, l_height, 0x1.000000p0f);
  }
  return l_o;
}
fn coax_p_e2_meadow_cell(p_kit: i32) -> f32 {
  var l_kit: i32 = p_kit;
  var l_s: f32;
  l_s = coax_p_e2_meadow_share(l_kit);
  return select(sqrt((0x1.666666p-1f / (0x1.b00000p-3f * l_s))), 0.0f, (l_s <= 0.0f));
}
fn coax_p_e2_meadow_held(p_gx: u32, p_gz: u32, p_kit: i32) -> bool {
  var l_gx: u32 = p_gx;
  var l_gz: u32 = p_gz;
  var l_kit: i32 = p_kit;
  return (coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((100i + l_kit)))) < 0x1.666666p-1f);
}
fn coax_p_e2_meadow_at_x(p_gx: u32, p_gz: u32, p_kit: i32) -> f32 {
  var l_gx: u32 = p_gx;
  var l_gz: u32 = p_gz;
  var l_kit: i32 = p_kit;
  return ((-0x1.000000p15f) + (((f32(l_gx) + 0x1.99999ap-4f) + (0x1.99999ap-1f * coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((60i + l_kit)))))) * coax_p_e2_meadow_cell(l_kit)));
}
fn coax_p_e2_meadow_at_z(p_gx: u32, p_gz: u32, p_kit: i32) -> f32 {
  var l_gx: u32 = p_gx;
  var l_gz: u32 = p_gz;
  var l_kit: i32 = p_kit;
  return ((-0x1.000000p15f) + (((f32(l_gz) + 0x1.99999ap-4f) + (0x1.99999ap-1f * coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((80i + l_kit)))))) * coax_p_e2_meadow_cell(l_kit)));
}
fn coax_p_e2_meadow_seed(p_gx: u32, p_gz: u32, p_kit: i32) -> f32 {
  var l_gx: u32 = p_gx;
  var l_gz: u32 = p_gz;
  var l_kit: i32 = p_kit;
  return ((coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((40i + l_kit)))) * 0x1.840000p6f) + 0x1.000000p-1f);
}
fn coax_p_cg_hash(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_v: f32;
  l_v = (sin(((l_a * 0x1.fc6666p6f) + (l_b * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
}
fn coax_p_e2_meadow_fade(p_kit: i32, p_d: f32) -> f32 {
  var l_kit: i32 = p_kit;
  var l_d: f32 = p_d;
  var l_r: f32;
  l_r = coax_p_e2_meadow_reach(l_kit);
  return (0x1.000000p0f - coax_p_cg_ramp((0x1.800000p-1f * l_r), l_r, l_d));
}
fn coax_p_e2_meadow_piece(p_kit: i32, p_h: f32, p_autumn: f32) -> coax_t30 {
  var l_kit: i32 = p_kit;
  var l_h: f32 = p_h;
  var l_autumn: f32 = p_autumn;
  var l_sz: f32;
  var l_p: coax_t30;
  var l_s: f32;
  var l_s_6: f32;
  var l_s_7: f32;
  var l_g: f32;
  var l_s_9: f32;
  var l_s_10: f32;
  l_sz = coax_p_cg_hash(l_h, 0x1.e00000p3f);
  l_p = coax_t30();
  l_p.x = l_kit;
  if (((l_kit >= 25i) && (l_kit <= 27i)))
  {
    l_s = (0x1.eb851ep-4f + (l_sz * 0x1.0a3d70p-3f));
    l_p.y = l_s;
    l_p.z = l_s;
    l_p.f4 = array<f32, 3>(0x1.99999ap-2f, 0x1.800000p-1f, 0x1.99999ap-5f);
  }
  else
  {
    if ((l_kit == 31i))
    {
      l_s_6 = (0x1.333334p-2f + (l_sz * 0x1.333334p-2f));
      l_p.y = l_s_6;
      l_p.z = l_s_6;
      l_p.f4 = array<f32, 3>(0x1.70a3d8p-2f, 0x1.99999ap-1f, 0x1.47ae14p-4f);
    }
    else
    {
      if (((l_kit >= 20i) && (l_kit <= 22i)))
      {
        l_s_7 = (0x1.000000p-2f + (l_sz * 0x1.99999ap-2f));
        l_p.y = l_s_7;
        l_p.z = l_s_7;
        l_p.w = (0x1.99999ap-4f * l_s_7);
        l_g = coax_p_cg_hash(l_h, 0x1.200000p4f);
        l_p.f4 = array<f32, 3>((0x1.333334p-1f + (0x1.47ae14p-4f * l_g)), (0x1.3d70a4p-1f + (0x1.eb851ep-5f * l_g)), 0x1.000000p-1f);
      }
      else
      {
        if (((l_kit == 29i) || (l_kit == 30i)))
        {
          l_s_9 = (0x1.333334p-2f + (l_sz * 0x1.000000p-2f));
          l_p.y = l_s_9;
          l_p.z = l_s_9;
          l_p.f4 = array<f32, 3>(0x1.70a3d8p-2f, 0x1.99999ap-1f, 0x1.47ae14p-4f);
          if ((l_autumn > 0x1.99999ap-2f))
          {
            l_p.f4 = array<f32, 3>(0x1.000000p0f, 0x1.333334p-2f, 0x1.eb851ep-6f);
            if ((coax_p_cg_hash(l_h, 0x1.100000p4f) >= 0x1.000000p-1f))
            {
              l_p.f4[1i] = 0x1.c28f5cp-2f;
              l_p.f4[2i] = 0x1.99999ap-5f;
            }
          }
        }
        else
        {
          l_s_10 = (0x1.000000p-1f + (l_sz * 0x1.333334p-2f));
          l_p.y = (l_s_10 * 0x1.800000p-1f);
          l_p.z = l_s_10;
          l_p.f4 = array<f32, 3>(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
        }
      }
    }
  }
  return l_p;
}
fn coax_p_cg_instance_point(p_p: vec3f, p_center: vec3f, p_vertical: f32, p_horizontal: f32, p_angle: f32) -> vec3f {
  var l_p: vec3f = p_p;
  var l_center: vec3f = p_center;
  var l_vertical: f32 = p_vertical;
  var l_horizontal: f32 = p_horizontal;
  var l_angle: f32 = p_angle;
  var l_cs: f32;
  var l_sn: f32;
  l_cs = cos(l_angle);
  l_sn = sin(l_angle);
  return coax_p_add3(l_center, coax_p_v3((((l_p.x * l_cs) + (l_p.z * l_sn)) * l_horizontal), (l_p.y * l_vertical), ((((-l_p.x) * l_sn) + (l_p.z * l_cs)) * l_horizontal)));
}
fn coax_p_cg_autumn(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_side: f32;
  var l_rear: f32;
  var l_wood: f32;
  var l_a: f32;
  var l_b: f32;
  var l_v: f32;
  l_side = (((l_x + (l_z * 0x1.99999ap-4f)) - 0x1.000000p0f) / 0x1.000000p4f);
  l_rear = (((-l_z) - 0x1.000000p4f) / 0x1.a00000p3f);
  l_wood = ((l_x + 0x1.800000p3f) / 0x1.400000p2f);
  l_a = select(0.0f, l_side, (l_side > 0.0f));
  l_b = (select(select(l_rear, 0x1.000000p0f, (l_rear > 0x1.000000p0f)), 0.0f, (l_rear < 0.0f)) * select(select(l_wood, 0x1.000000p0f, (l_wood > 0x1.000000p0f)), 0.0f, (l_wood < 0.0f)));
  l_v = select(l_b, l_a, (l_a > l_b));
  return select(l_v, 0x1.000000p0f, (l_v > 0x1.000000p0f));
}
fn coax_p_cg_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_paint_noise(l_p);
}
fn coax_p_v3_of(p_v: vec4f) -> vec3f {
  var l_v: vec4f = p_v;
  return vec3f(l_v.x, l_v.y, l_v.z);
}
fn coax_p_cg_sky_ray(p_x: f32, p_y: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_fy: f32;
  var l_fz: f32;
  var l_qx: f32;
  var l_qy: f32;
  l_fy = (0x1.99999ap-2f / sqrt(0x1.38a3d8p3f));
  l_fz = ((-0x1.8cccccp1f) / sqrt(0x1.38a3d8p3f));
  l_qx = ((l_x * 0x1.e00000p10f) / (0x1.0e0000p10f * 0x1.c7ae14p0f));
  l_qy = (((l_y * 0x1.000000p-1f) - 0x1.47ae14p-6f) / 0x1.c7ae14p-1f);
  return coax_p_v3(l_qx, (l_fy - (l_qy * l_fz)), (l_fz + (l_qy * l_fy)));
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
}
fn coax_p_mul3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_fivelab_gait_share(p_code: f32, p_j: u32) -> f32 {
  var l_code: f32 = p_code;
  var l_j: u32 = p_j;
  var l_c: f32;
  var l_low: f32;
  var l_up: f32;
  var l_m: f32;
  if ((l_code < 0x1.800000p0f))
  {
    return l_code;
  }
  l_c = (l_code - 0x1.000000p1f);
  l_low = floor((l_c / 0x1.000000p8f));
  l_up = (l_c - (l_low * 0x1.000000p8f));
  l_m = coax_p_fivelab_upper(l_j);
  return (((l_up / 0x1.fe0000p7f) * l_m) + ((l_low / 0x1.fe0000p7f) * (0x1.000000p0f - l_m)));
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
fn coax_p_fivelab_rows(p_row: u32, p_j: u32, p_arm: bool) -> coax_t33 {
  var l_row: u32 = p_row;
  var l_j: u32 = p_j;
  var l_arm: bool = p_arm;
  var l_r: u32;
  var l_m: coax_t33;
  l_r = (l_row + (l_j * 3u));
  l_m = coax_t33();
  l_m.x = bake[i32(l_r)];
  l_m.y = bake[i32((l_r + 1u))];
  l_m.z = bake[i32((l_r + 2u))];
  if (l_arm)
  {
    l_m = coax_p_fivelab_arm(l_row, l_j, l_m);
  }
  return l_m;
}
fn coax_p_fivelab_mix4(p_a: vec4f, p_b: vec4f, p_t: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  var l_t: f32 = p_t;
  return coax_p_v4((l_a.x + ((l_b.x - l_a.x) * l_t)), (l_a.y + ((l_b.y - l_a.y) * l_t)), (l_a.z + ((l_b.z - l_a.z) * l_t)), (l_a.w + ((l_b.w - l_a.w) * l_t)));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_render_filter_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t34 {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_f: coax_t36;
  var l_rs_sum: vec4f;
  var l_rs_i: i32;
  var l_rs_offset: f32;
  var l_rs_at: vec2f;
  l_rs_f = coax_p_render_footprint(i32(textureDimensions(sheets).x), i32(textureDimensions(sheets).y), l_dx.x, l_dx.y, l_dy.x, l_dy.y, l_bias);
  if ((!l_rs_f.x))
  {
    return coax_t34(false, vec4f());
  }
  l_rs_sum = vec4f();
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = l_rs_f.y;
    for (l_rs_i = coax_from; l_rs_i < coax_to; l_rs_i = l_rs_i + 1i)
    {
      l_rs_offset = ((f32(l_rs_i) + 0x1.000000p-1f) - (0x1.000000p-1f * f32(l_rs_f.y)));
      l_rs_at = coax_p_add2(l_uv, coax_p_scale2(coax_p_v2(l_rs_f.z, l_rs_f.w), l_rs_offset));
      l_rs_sum = coax_p_add4(l_rs_sum, textureSampleLevel(sheets, sheet, l_rs_at, l_layer, l_rs_f.f4));
    }
  }
  return coax_t34(true, coax_p_scale4(l_rs_sum, (0x1.000000p0f / f32(l_rs_f.y))));
}
fn coax_p_engine_terrain_noise(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_ix: f32;
  var l_iz: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_sx: f32;
  var l_sz: f32;
  l_ix = floor(l_x);
  l_iz = floor(l_z);
  l_fx = (l_x - l_ix);
  l_fz = (l_z - l_iz);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sz = ((l_fz * l_fz) * (0x1.800000p1f - (0x1.000000p1f * l_fz)));
  return coax_p_engine_terrain_mix(coax_p_engine_terrain_mix(coax_p_render_noise2(l_ix, l_iz), coax_p_render_noise2((l_ix + 0x1.000000p0f), l_iz), l_sx), coax_p_engine_terrain_mix(coax_p_render_noise2(l_ix, (l_iz + 0x1.000000p0f)), coax_p_render_noise2((l_ix + 0x1.000000p0f), (l_iz + 0x1.000000p0f)), l_sx), l_sz);
}
fn coax_p_engine_terrain_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
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
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_wg_add_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a + l_b.x), (l_a + l_b.y));
}
fn coax_p_chat_erf2(p_x: vec2f) -> vec2f {
  var l_x: vec2f = p_x;
  var l_s: vec2f;
  var l_a: vec2f;
  var l_r1: vec2f;
  var l_r2: vec2f;
  l_s = coax_p_wg_sign_f2(l_x);
  l_a = coax_p_wg_abs_f2(l_x);
  l_r1 = coax_p_wg_add_ff2(0x1.000000p0f, coax_p_wg_mul_f2f2(coax_p_wg_add_ff2(0x1.1d130ep-2f, coax_p_wg_mul_f2f2(coax_p_wg_add_ff2(0x1.d7d630p-3f, coax_p_wg_mul_f2f2(coax_p_wg_add_ff2(0x1.fd9ba2p-11f, coax_p_wg_mul_ff2(0x1.3fee2cp-4f, l_a)), l_a)), l_a)), l_a));
  l_r2 = coax_p_wg_mul_f2f2(l_r1, l_r1);
  return coax_p_wg_sub_f2f2(l_s, coax_p_wg_div_f2f2(l_s, coax_p_wg_mul_f2f2(l_r2, l_r2)));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_e2_field_pick(p_x: u32, p_z: u32, p_k: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_k: u32 = p_k;
  return (coax_p_e2_field_hash(l_x, l_z, (l_k + 10u)) & 3u);
}
fn coax_p_e2_ihash(p_v: u32) -> u32 {
  var l_v: u32 = p_v;
  var l_x: u32;
  l_x = (l_v ^ (l_v >> u32(16u)));
  l_x = (l_x * 2146121005u);
  l_x = (l_x ^ (l_x >> u32(15u)));
  l_x = (l_x * 1663597195u);
  return (l_x ^ (l_x >> u32(16u)));
}
fn coax_p_e2_field_fine_off(p_c: f32) -> bool {
  var l_c: f32 = p_c;
  return (l_c <= 0x1.001a36p-2f);
}
fn coax_p_e2_noise(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_i: f32;
  var l_j: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_i = floor(l_x);
  l_j = floor(l_z);
  l_fx = (l_x - l_i);
  l_fz = (l_z - l_j);
  l_fx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_fz = ((l_fz * l_fz) * (0x1.800000p1f - (0x1.000000p1f * l_fz)));
  l_a = coax_p_cg_hash(l_i, l_j);
  l_b = coax_p_cg_hash((l_i + 0x1.000000p0f), l_j);
  l_c = coax_p_cg_hash(l_i, (l_j + 0x1.000000p0f));
  l_d = coax_p_cg_hash((l_i + 0x1.000000p0f), (l_j + 0x1.000000p0f));
  return ((((l_a * (0x1.000000p0f - l_fx)) + (l_b * l_fx)) * (0x1.000000p0f - l_fz)) + (((l_c * (0x1.000000p0f - l_fx)) + (l_d * l_fx)) * l_fz));
}
fn coax_p_e2_smooth(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = ((l_x - l_a) / (l_b - l_a));
  l_t = select(select(l_t, 0x1.000000p0f, (l_t > 0x1.000000p0f)), 0.0f, (l_t < 0.0f));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_exp_ground_sites() -> array<f32, 192> {
  return array<f32, 192>((-0x1.200000p4f), (-0x1.700000p4f), 0x1.400000p3f, 0x1.000000p2f, 0x1.59999ap3f, 0x1.066666p2f, 0x1.200000p4f, (-0x1.400000p4f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, (-0x1.900000p5f), 0x1.900000p4f, 0x1.733334p1f, 0x1.600000p4f, 0x1.c00000p2f, 0x1.8cccccp2f, 0x1.180000p5f, 0x1.e00000p4f, 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, (-0x1.400000p4f), 0x1.900000p4f, 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, (-0x1.680000p5f), 0x1.600000p5f, 0x1.400000p3f, 0x1.000000p2f, 0x1.e00000p2f, 0x1.0cccccp2f, (-0x1.300000p4f), 0x1.680000p5f, 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, (-0x1.400000p2f), (-0x1.700000p4f), 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, 0x1.e00000p3f, (-0x1.400000p2f), 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, 0x1.800000p5f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.d00000p5f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.100000p6f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.380000p6f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.800000p5f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.d00000p5f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.100000p6f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.380000p6f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.800000p5f, 0x1.f80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, (-0x1.300000p5f), (-0x1.b00000p5f), 0x1.51999ap3f, 0x1.6c6666p5f, 0x1.923334p6f, 0x1.51999ap3f, 0x1.900000p5f, (-0x1.400000p4f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.200000p4f, (-0x1.a00000p5f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.900000p5f, (-0x1.a00000p5f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.280000p6f, (-0x1.400000p4f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.280000p6f, (-0x1.a00000p5f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.200000p4f, (-0x1.280000p6f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.900000p5f, (-0x1.280000p6f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.280000p6f, (-0x1.280000p6f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.200000p5f, (-0x1.100000p5f), 0x1.400000p3f, 0x1.000000p2f, 0x1.59999ap3f, 0x1.066666p2f, (-0x1.400000p4f), (-0x1.400000p2f), 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.a451ecp4f, 0x1.700000p5f, 0x1.e570a4p3f, 0x1.e570a4p3f, 0x1.a66666p2f, 0x1.a66666p2f, 0x1.5947aep3f, 0x1.20999ap6f, 0x1.eb0a3ep4f, 0x1.eb0a3ep4f, 0x1.333334p3f, 0x1.333334p3f, 0x1.df999ap5f, 0x1.3a0000p6f, 0x1.c9999ap3f, 0x1.c9999ap3f, 0x1.d9999ap1f, 0x1.a00000p1f);
}
fn coax_p_cg_height(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_away: f32;
  var l_h: f32;
  var l_fade: f32;
  l_away = sqrt(((l_x * l_x) + (l_z * l_z)));
  if ((l_away < 0x1.000000p1f))
  {
    return 0.0f;
  }
  l_h = ((0x1.000000p2f * exp((-((((l_x + 0x1.600000p3f) * (l_x + 0x1.600000p3f)) / 0x1.b80000p6f) + (((l_z + 0x1.600000p4f) * (l_z + 0x1.600000p4f)) / 0x1.7c0000p6f))))) + (0x1.733334p2f * exp((-((((l_x - 0x1.100000p4f) * (l_x - 0x1.100000p4f)) / 0x1.400000p7f) + (((l_z + 0x1.080000p5f) * (l_z + 0x1.080000p5f)) / 0x1.680000p7f))))));
  l_h = (l_h + (0x1.666666p3f * exp((-((((l_x + 0x1.000000p3f) * (l_x + 0x1.000000p3f)) / 0x1.e00000p6f) + (((l_z + 0x1.100000p5f) * (l_z + 0x1.100000p5f)) / 0x1.180000p6f))))));
  l_fade = select(0x1.000000p0f, ((l_away - 0x1.000000p1f) / 0x1.800000p1f), (l_away < 0x1.400000p2f));
  return coax_p_cg_road_ground(l_x, l_z, (l_h * l_fade));
}
fn coax_p_exp_ground_level(p_h: f32, p_sx: f32, p_sz: f32, p_left: f32, p_right: f32, p_back: f32, p_front: f32, p_x: f32, p_z: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_sx: f32 = p_sx;
  var l_sz: f32 = p_sz;
  var l_left: f32 = p_left;
  var l_right: f32 = p_right;
  var l_back: f32 = p_back;
  var l_front: f32 = p_front;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_d: f32;
  var l_t: f32;
  l_d = coax_p_exp_ground_distance(l_sx, l_sz, l_left, l_right, l_back, l_front, l_x, l_z);
  if ((l_d >= 0x1.000000p2f))
  {
    return l_h;
  }
  l_t = select((l_d / 0x1.000000p2f), 0.0f, (l_d <= 0.0f));
  l_t = ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
  return ((coax_p_cg_height(l_sx, l_sz) * (0x1.000000p0f - l_t)) + (l_h * l_t));
}
fn coax_p_exp_ground_castle_distance(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  return coax_p_exp_ground_distance(0x1.700000p5f, (-0x1.780000p5f), 0x1.040000p5f, 0x1.040000p5f, 0x1.f80000p4f, 0x1.f00000p4f, l_x, l_z);
}
fn coax_p_e2_meadow_share(p_kit: i32) -> f32 {
  var l_kit: i32 = p_kit;
  if (((l_kit >= 25i) && (l_kit <= 27i)))
  {
    return 0x1.777778p-3f;
  }
  if ((l_kit == 31i))
  {
    return 0x1.5c28f6p-3f;
  }
  if (((l_kit >= 20i) && (l_kit <= 22i)))
  {
    return 0x1.111112p-5f;
  }
  if (((l_kit == 29i) || (l_kit == 30i)))
  {
    return 0x1.99999ap-5f;
  }
  if ((l_kit == 28i))
  {
    return 0x1.47ae14p-4f;
  }
  return 0.0f;
}
fn coax_p_e2_meadow_reach(p_kit: i32) -> f32 {
  var l_kit: i32 = p_kit;
  return select(select(select(0x1.400000p6f, 0x1.900000p6f, ((l_kit >= 25i) && (l_kit <= 27i))), 0x1.180000p7f, ((l_kit == 29i) || (l_kit == 30i))), 0x1.400000p8f, ((l_kit >= 20i) && (l_kit <= 22i)));
}
fn coax_p_cg_ramp(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = ((l_x - l_a) / (l_b - l_a));
  l_t = select(select(l_t, 0x1.000000p0f, (l_t > 0x1.000000p0f)), 0.0f, (l_t < 0.0f));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_paint_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_i = coax_p_v2(floor(l_p.x), floor(l_p.y));
  l_f = coax_p_v2(coax_p_fract(l_p.x), coax_p_fract(l_p.y));
  l_f = coax_p_v2(((l_f.x * l_f.x) * (0x1.800000p1f - (0x1.000000p1f * l_f.x))), ((l_f.y * l_f.y) * (0x1.800000p1f - (0x1.000000p1f * l_f.y))));
  l_a = coax_p_metaverse_style_hash(l_i.x, l_i.y);
  l_b = coax_p_metaverse_style_hash((l_i.x + 0x1.000000p0f), l_i.y);
  l_c = coax_p_metaverse_style_hash(l_i.x, (l_i.y + 0x1.000000p0f));
  l_d = coax_p_metaverse_style_hash((l_i.x + 0x1.000000p0f), (l_i.y + 0x1.000000p0f));
  return coax_p_mix(coax_p_mix(l_a, l_b, l_f.x), coax_p_mix(l_c, l_d, l_f.x), l_f.y);
}
fn coax_p_fivelab_upper(p_j: u32) -> f32 {
  var l_j: u32 = p_j;
  if (((l_j <= 1u) || (l_j >= 55u)))
  {
    return 0.0f;
  }
  if ((l_j == 2u))
  {
    return 0x1.99999ap-2f;
  }
  if ((l_j == 3u))
  {
    return 0x1.800000p-1f;
  }
  return 0x1.000000p0f;
}
fn coax_p_fivelab_arm(p_row: u32, p_j: u32, p_m: coax_t33) -> coax_t33 {
  var l_row: u32 = p_row;
  var l_j: u32 = p_j;
  var l_m: coax_t33 = p_m;
  var l_q: vec4f;
  var l_qe: vec4f;
  var l_qw: vec4f;
  var l_p: vec4f;
  var l_still: bool;
  var l_ao: f32;
  var l_af: f32;
  var l_ak: f32;
  var l_ca: f32;
  var l_sa: f32;
  var l_cf: f32;
  var l_sf: f32;
  var l_r1: coax_t33;
  var l_t1: coax_t33;
  var l_e: vec4f;
  var l_w: vec4f;
  if ((((l_j < 8u) || (l_j > 30u)) || (!coax_p_fivelab_arm_frame(l_row))))
  {
    return l_m;
  }
  l_q = coax_p_v4(0x1.8902dep-3f, 0x1.749ba6p0f, (-0x1.0be0dep-4f), 0x1.000000p0f);
  l_qe = coax_p_v4(0x1.c58794p-2f, 0x1.749ba6p0f, (-0x1.2b020cp-4f), 0x1.000000p0f);
  l_qw = coax_p_v4(0x1.5dcc64p-1f, 0x1.749ba6p0f, (-0x1.0be0dep-4f), 0x1.000000p0f);
  if (((l_row / 195u) >= 4137u))
  {
    l_q = coax_p_v4(0x1.367a10p-3f, 0x1.6b089ap0f, (-0x1.bcd35ap-5f), 0x1.000000p0f);
    l_qe = coax_p_v4(0x1.914e3cp-2f, 0x1.6b089ap0f, (-0x1.e1b08ap-5f), 0x1.000000p0f);
    l_qw = coax_p_v4(0x1.483e42p-1f, 0x1.6b089ap0f, (-0x1.a6b50cp-5f), 0x1.000000p0f);
  }
  l_p = coax_p_fivelab_at(coax_p_fivelab_joint(l_row, 8u), l_q);
  l_still = (((l_row / 195u) % 4137u) < 75u);
  l_ao = 0x1.666666p-2f;
  l_af = 0x1.666666p-2f;
  l_ak = 0.0f;
  if (l_still)
  {
    l_ao = 0.0f;
    l_af = 0x1.333334p-1f;
    l_ak = 0x1.99999ap-1f;
  }
  l_ca = cos(l_ao);
  l_sa = sin(l_ao);
  l_cf = cos(l_af);
  l_sf = sin(l_af);
  l_r1 = coax_t33();
  l_r1.x = coax_p_v4(l_ca, (-l_sa), 0.0f, 0.0f);
  l_r1.y = coax_p_v4((l_cf * l_sa), (l_cf * l_ca), l_sf, 0.0f);
  l_r1.z = coax_p_v4(((-l_sf) * l_sa), ((-l_sf) * l_ca), l_cf, 0.0f);
  l_t1 = coax_p_fivelab_turn_at(l_r1, l_p);
  if ((l_j == 8u))
  {
    return coax_p_fivelab_aff(l_t1, l_m);
  }
  l_e = coax_p_fivelab_at(coax_p_fivelab_aff(l_t1, coax_p_fivelab_joint(l_row, 9u)), l_qe);
  l_w = coax_p_fivelab_at(coax_p_fivelab_aff(l_t1, coax_p_fivelab_joint(l_row, 10u)), l_qw);
  return coax_p_fivelab_aff(coax_p_fivelab_turn_at(coax_p_fivelab_elbow_open(l_p, l_e, l_w, l_ak), l_e), coax_p_fivelab_aff(l_t1, l_m));
}
fn coax_p_render_footprint(p_width: i32, p_height: i32, p_du_dx: f32, p_dv_dx: f32, p_du_dy: f32, p_dv_dy: f32, p_bias: f32) -> coax_t36 {
  var l_width: i32 = p_width;
  var l_height: i32 = p_height;
  var l_du_dx: f32 = p_du_dx;
  var l_dv_dx: f32 = p_dv_dx;
  var l_du_dy: f32 = p_du_dy;
  var l_dv_dy: f32 = p_dv_dy;
  var l_bias: f32 = p_bias;
  var l_x: f32;
  var l_y: f32;
  var l_u: f32;
  var l_v: f32;
  var l_a: f32;
  var l_b: f32;
  var l_major: f32;
  var l_minor: f32;
  var l_du: f32;
  var l_dv: f32;
  var l_taps: i32;
  var l_excess: i32;
  var l_span: f32;
  if (((((((((((((((l_width < 1i) || (l_width > 16384i)) || (l_height < 1i)) || (l_height > 16384i)) || (!coax_p_render_footprint_finite(l_du_dx))) || (!coax_p_render_footprint_finite(l_dv_dx))) || (!coax_p_render_footprint_finite(l_du_dy))) || (!coax_p_render_footprint_finite(l_dv_dy))) || (!coax_p_render_footprint_finite(l_bias))) || (abs(l_du_dx) > 0x1.000000p16f)) || (abs(l_dv_dx) > 0x1.000000p16f)) || (abs(l_du_dy) > 0x1.000000p16f)) || (abs(l_dv_dy) > 0x1.000000p16f)) || (abs(l_bias) > 0x1.000000p5f)))
  {
    return coax_t36(false, 0i, 0.0f, 0.0f, 0.0f);
  }
  l_x = (l_du_dx * f32(l_width));
  l_y = (l_dv_dx * f32(l_height));
  l_u = (l_du_dy * f32(l_width));
  l_v = (l_dv_dy * f32(l_height));
  l_a = ((l_x * l_x) + (l_y * l_y));
  l_b = ((l_u * l_u) + (l_v * l_v));
  l_major = 0.0f;
  l_minor = 0.0f;
  l_du = 0.0f;
  l_dv = 0.0f;
  if ((l_a >= l_b))
  {
    l_major = sqrt(l_a);
    l_minor = sqrt(l_b);
    l_du = l_du_dx;
    l_dv = l_dv_dx;
  }
  else
  {
    l_major = sqrt(l_b);
    l_minor = sqrt(l_a);
    l_du = l_du_dy;
    l_dv = l_dv_dy;
  }
  l_minor = coax_p_render_footprint_max(l_minor, 0x1.000000p0f);
  l_taps = i32((-floor(((-l_major) / l_minor))));
  if ((l_taps == 0i))
  {
    l_taps = 1i;
  }
  l_excess = i32((l_taps - 8i));
  l_taps = (l_taps - i32((l_excess & ((l_excess >> u32(31i)) ^ (-1i)))));
  l_span = coax_p_render_footprint_max(l_minor, (l_major / f32(l_taps)));
  return coax_t36(true, l_taps, (l_du / f32(l_taps)), (l_dv / f32(l_taps)), ((log(l_span) / log(0x1.000000p1f)) + l_bias));
}
fn coax_p_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
}
fn coax_p_add2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return vec2f((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_add4(p_a: vec4f, p_b: vec4f) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return vec4f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
}
fn coax_p_scale4(p_a: vec4f, p_k: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_k: f32 = p_k;
  return vec4f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k), (l_a.w * l_k));
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
fn coax_p_wg_sign_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_wg_sign(l_p0.x), coax_p_wg_sign(l_p0.y));
}
fn coax_p_cg_road_ground(p_x: f32, p_z: f32, p_h: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_h: f32 = p_h;
  var l_corridor: f32;
  var l_g: f32;
  var l_ex: f32;
  var l_ez: f32;
  l_corridor = (((0x1.000000p0f - coax_p_cg_ramp(0x1.000000p1f, 0x1.400000p2f, abs((l_x - coax_p_cg_road_x(l_z))))) * coax_p_cg_ramp((-0x1.c00000p3f), (-0x1.200000p4f), l_z)) * (0x1.000000p0f - coax_p_cg_ramp((-0x1.c00000p5f), (-0x1.f00000p5f), l_z)));
  l_g = (l_h + (((0x1.a8f5c2p-1f + (0x1.f0a3d8p-1f * coax_p_cg_ramp((-0x1.000000p4f), (-0x1.080000p5f), l_z))) - l_h) * l_corridor));
  l_ex = ((l_x - 0x1.200000p3f) / 0x1.a00000p3f);
  l_ez = ((l_z + 0x1.740000p5f) / 0x1.900000p3f);
  l_g = (l_g + ((0x1.ccccccp0f - l_g) * (0x1.000000p0f - coax_p_cg_ramp(0x1.b33334p-1f, 0x1.333334p0f, sqrt(((l_ex * l_ex) + (l_ez * l_ez)))))));
  return (l_g + (coax_p_cg_ramp((-0x1.f00000p5f), (-0x1.880000p6f), l_z) * (0x1.a00000p2f + (0x1.800000p1f * sin(((l_x * 0x1.70a3d8p-4f) + 0x1.4cccccp0f))))));
}
fn coax_p_exp_ground_distance(p_sx: f32, p_sz: f32, p_left: f32, p_right: f32, p_back: f32, p_front: f32, p_x: f32, p_z: f32) -> f32 {
  var l_sx: f32 = p_sx;
  var l_sz: f32 = p_sz;
  var l_left: f32 = p_left;
  var l_right: f32 = p_right;
  var l_back: f32 = p_back;
  var l_front: f32 = p_front;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_dx: f32;
  var l_dz: f32;
  l_dx = coax_p_exp_ground_max(((l_sx - l_left) - l_x), ((l_x - l_sx) - l_right));
  l_dz = coax_p_exp_ground_max(((l_sz - l_back) - l_z), ((l_z - l_sz) - l_front));
  return coax_p_exp_ground_max(l_dx, l_dz);
}
fn coax_p_metaverse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return coax_p_render_noise2(l_x, l_y);
}
fn coax_p_fivelab_arm_frame(p_row: u32) -> bool {
  var l_row: u32 = p_row;
  var l_f: u32;
  l_f = ((l_row / 195u) % 4137u);
  return ((l_f < 143u) || ((l_f >= 389u) && (l_f < 464u)));
}
fn coax_p_fivelab_joint(p_row: u32, p_j: u32) -> coax_t33 {
  var l_row: u32 = p_row;
  var l_j: u32 = p_j;
  var l_r: u32;
  var l_o: coax_t33;
  l_r = (l_row + (l_j * 3u));
  l_o = coax_t33();
  l_o.x = bake[i32(l_r)];
  l_o.y = bake[i32((l_r + 1u))];
  l_o.z = bake[i32((l_r + 2u))];
  return l_o;
}
fn coax_p_fivelab_at(p_m: coax_t33, p_q: vec4f) -> vec4f {
  var l_m: coax_t33 = p_m;
  var l_q: vec4f = p_q;
  return coax_p_v4(coax_p_wg_dot4(l_m.x, l_q), coax_p_wg_dot4(l_m.y, l_q), coax_p_wg_dot4(l_m.z, l_q), 0x1.000000p0f);
}
fn coax_p_fivelab_turn_at(p_r: coax_t33, p_p: vec4f) -> coax_t33 {
  var l_r: coax_t33 = p_r;
  var l_p: vec4f = p_p;
  var l_o: coax_t33;
  l_o = coax_t33();
  l_o.x = coax_p_v4(l_r.x.x, l_r.x.y, l_r.x.z, (l_p.x - (((l_r.x.x * l_p.x) + (l_r.x.y * l_p.y)) + (l_r.x.z * l_p.z))));
  l_o.y = coax_p_v4(l_r.y.x, l_r.y.y, l_r.y.z, (l_p.y - (((l_r.y.x * l_p.x) + (l_r.y.y * l_p.y)) + (l_r.y.z * l_p.z))));
  l_o.z = coax_p_v4(l_r.z.x, l_r.z.y, l_r.z.z, (l_p.z - (((l_r.z.x * l_p.x) + (l_r.z.y * l_p.y)) + (l_r.z.z * l_p.z))));
  return l_o;
}
fn coax_p_fivelab_aff(p_t: coax_t33, p_m: coax_t33) -> coax_t33 {
  var l_t: coax_t33 = p_t;
  var l_m: coax_t33 = p_m;
  var l_o: coax_t33;
  l_o = coax_t33();
  l_o.x = coax_p_fivelab_lin(l_t.x.x, l_t.x.y, l_t.x.z, l_t.x.w, l_m);
  l_o.y = coax_p_fivelab_lin(l_t.y.x, l_t.y.y, l_t.y.z, l_t.y.w, l_m);
  l_o.z = coax_p_fivelab_lin(l_t.z.x, l_t.z.y, l_t.z.z, l_t.z.w, l_m);
  return l_o;
}
fn coax_p_fivelab_elbow_open(p_p: vec4f, p_e: vec4f, p_w: vec4f, p_k: f32) -> coax_t33 {
  var l_p: vec4f = p_p;
  var l_e: vec4f = p_e;
  var l_w: vec4f = p_w;
  var l_k: f32 = p_k;
  var l_ux: f32;
  var l_uy: f32;
  var l_uz: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_fz: f32;
  var l_ax: f32;
  var l_ay: f32;
  var l_az: f32;
  var l_la: f32;
  var l_d: f32;
  var l_o: coax_t33;
  var l_th: f32;
  var l_c: f32;
  var l_sn: f32;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_v: f32;
  l_ux = (l_e.x - l_p.x);
  l_uy = (l_e.y - l_p.y);
  l_uz = (l_e.z - l_p.z);
  l_fx = (l_w.x - l_e.x);
  l_fy = (l_w.y - l_e.y);
  l_fz = (l_w.z - l_e.z);
  l_ax = ((l_fy * l_uz) - (l_fz * l_uy));
  l_ay = ((l_fz * l_ux) - (l_fx * l_uz));
  l_az = ((l_fx * l_uy) - (l_fy * l_ux));
  l_la = sqrt((((l_ax * l_ax) + (l_ay * l_ay)) + (l_az * l_az)));
  l_d = (((l_fx * l_ux) + (l_fy * l_uy)) + (l_fz * l_uz));
  l_o = coax_t33();
  l_o.x = coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0.0f);
  l_o.y = coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, 0.0f);
  l_o.z = coax_p_v4(0.0f, 0.0f, 0x1.000000p0f, 0.0f);
  if ((l_la < 0x1.0c6f7ap-20f))
  {
    return l_o;
  }
  l_th = (coax_p_wg_atan2(l_la, l_d) * l_k);
  l_c = cos(l_th);
  l_sn = sin(l_th);
  l_x = (l_ax / l_la);
  l_y = (l_ay / l_la);
  l_z = (l_az / l_la);
  l_v = (0x1.000000p0f - l_c);
  l_o.x = coax_p_v4((l_c + ((l_x * l_x) * l_v)), (((l_x * l_y) * l_v) - (l_z * l_sn)), (((l_x * l_z) * l_v) + (l_y * l_sn)), 0.0f);
  l_o.y = coax_p_v4((((l_y * l_x) * l_v) + (l_z * l_sn)), (l_c + ((l_y * l_y) * l_v)), (((l_y * l_z) * l_v) - (l_x * l_sn)), 0.0f);
  l_o.z = coax_p_v4((((l_z * l_x) * l_v) - (l_y * l_sn)), (((l_z * l_y) * l_v) + (l_x * l_sn)), (l_c + ((l_z * l_z) * l_v)), 0.0f);
  return l_o;
}
fn coax_p_render_footprint_finite(p_x: f32) -> bool {
  var l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_footprint_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (((l_a + l_b) + abs((l_a - l_b))) * 0x1.000000p-1f);
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
fn coax_p_cg_road_x(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  var l_b: f32;
  l_b = coax_p_cg_ramp((-0x1.a00000p3f), (-0x1.300000p4f), l_z);
  return ((coax_p_cg_path_x(l_z) * (0x1.000000p0f - l_b)) + ((0x1.a66666p1f + (0x1.333334p-2f * sin(((l_z + 0x1.500000p4f) * 0x1.000000p-2f)))) * l_b));
}
fn coax_p_exp_ground_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_fivelab_lin(p_x: f32, p_y: f32, p_z: f32, p_w: f32, p_m: coax_t33) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  var l_m: coax_t33 = p_m;
  return coax_p_v4((((l_x * l_m.x.x) + (l_y * l_m.y.x)) + (l_z * l_m.z.x)), (((l_x * l_m.x.y) + (l_y * l_m.y.y)) + (l_z * l_m.z.y)), (((l_x * l_m.x.z) + (l_y * l_m.y.z)) + (l_z * l_m.z.z)), ((((l_x * l_m.x.w) + (l_y * l_m.y.w)) + (l_z * l_m.z.w)) + l_w));
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
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  var l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
}
fn coax_p_cg_path_x(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  return (((0x1.0cccccp1f * sin((l_z * 0x1.147ae2p-2f))) + 0x1.ccccccp-1f) - select(0.0f, (l_z * 0x1.19999ap-1f), (l_z > 0.0f)));
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
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_17 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_17(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_18) -> coax_io_19 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t18(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_io_19(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_17) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t17(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8));
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_19) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t19(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8));
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