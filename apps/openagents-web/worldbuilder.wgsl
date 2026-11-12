// ---- generated: apps/openagents-web/gpu/worldbuilder.coax (the module) by coax wgsl; do not edit
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
  w: vec2f,
  f4: vec3f,
  f5: u32,
  f6: vec3f,
  f7: vec3f,
  f8: u32,
}
struct coax_io_15 {
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
struct coax_t16 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
  f5: vec4f,
  f6: vec4f,
}
struct coax_in_16 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
  @location(4) f4: vec4f,
  @location(5) f5: vec4f,
  @location(6) f6: vec4f,
}
struct coax_t17 {
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
struct coax_io_17 {
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
struct coax_t22 {
  x: i32,
  y: i32,
}
struct coax_t23 {
  x: u32,
  y: u32,
}
struct coax_t24 {
  x: bool,
  y: vec4f,
}
struct coax_t25 {
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
@group(1) @binding(1) var<storage, read> objects: array<coax_t11>;
@group(1) @binding(2) var<storage, read> ground: array<vec4f>;
@group(1) @binding(3) var sheets: texture_2d_array<f32>;
@group(1) @binding(4) var sheet: sampler;
@group(0) @binding(1) var gpui_atlas: texture_2d<f32>;
@group(0) @binding(2) var gpui_color: texture_2d<f32>;
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
  var l_o: coax_t15;
  var l_c0: vec3f;
  var l_c1: vec3f;
  var l_c2: vec3f;
  var l_rgb: vec4f;
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
  l_o = coax_t15();
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
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_v: coax_t16) -> coax_t17 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_v: coax_t16 = p_v;
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
  l_ground = coax_p_worldbuilder_floor(l_xz, l_w1, l_fog, 0x1.000000p0f);
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_worldbuilder_marks(l_xz, l_w1));
  l_ground = coax_p_wg_mix_f3f3f(l_sky, l_ground, ((l_fog * 0x1.b33334p-1f) + 0x1.333334p-3f));
  l_edge = select(0.0f, coax_p_smoothstep(0.0f, 0x1.47ae14p-6f, (-l_up)), l_down);
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_sky, l_ground, l_edge), coax_at_u(i32(3i)));
  return coax_p_wg_make_f4_f3f(coax_p_worldbuilder_out(l_c), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t15) -> vec4f {
  var l_s: coax_t15 = p_s;
  var l_w1: vec2f;
  var l_textured: bool;
  var l_layer: i32;
  var l_tex: vec4f;
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
fn coax_p_fx_fs(p_v: coax_t17) -> vec4f {
  var l_v: coax_t17 = p_v;
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
fn coax_p_worldbuilder_ground_vs(p_id: u32) -> coax_t15 {
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
  var l_o: coax_t15;
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
  l_o = coax_t15();
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
fn coax_p_chat_fx_vs(p__vid2: u32, p__iid3: u32, p_v: coax_t16) -> coax_t17 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_v: coax_t16 = p_v;
  var l_o: coax_t17;
  l_o = coax_t17();
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
fn coax_p_worldbuilder_ground_fs(p_s: coax_t15, p_w1: vec2f) -> vec3f {
  var l_s: coax_t15 = p_s;
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
fn coax_p_chat_fx_fs(p_v: coax_t17) -> vec4f {
  var l_v: coax_t17 = p_v;
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
  l_t = coax_p_clampf(coax_p_aethyr_terrain_blend_t(l_xz.x, l_xz.y, coax_at_u(i32((l_at + 3i))), coax_at_u(i32((l_at + 8i))), 0.0f, coax_at_u(i32((l_at + 7i))), coax_at_u(i32((l_at + 9i))), 0.0f), 0.0f, 0x1.000000p0f);
  l_gain = coax_p_aethyr_terrain_gain(l_xz.x, l_xz.y, coax_at_u(i32((l_at + 10i))), coax_at_u(i32((l_at + 11i))), 0x1.7ae148p-2f, 0.0f);
  return coax_p_v3(coax_p_aethyr_terrain_decode(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 4i))), l_t, l_gain), coax_p_aethyr_terrain_decode(coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 5i))), l_t, l_gain), coax_p_aethyr_terrain_decode(coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 6i))), l_t, l_gain));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_wg_dims_2d() -> coax_t23 {
  return coax_p_u2(textureDimensions(gpui_atlas).x, textureDimensions(gpui_atlas).y);
}
fn coax_p_wg_cvt_f2u2(p_p: coax_t23) -> vec2f {
  var l_p: coax_t23 = p_p;
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
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t22 {
  var l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
}
fn coax_p_wg_load_2d(p_c: coax_t22, p_level: i32) -> vec4f {
  var l_c: coax_t22 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(gpui_atlas, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_dims_2d_1() -> coax_t23 {
  return coax_p_u2(textureDimensions(gpui_color).x, textureDimensions(gpui_color).y);
}
fn coax_p_wg_load_2d_1(p_c: coax_t22, p_level: i32) -> vec4f {
  var l_c: coax_t22 = p_c;
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
  var l_rs_result: coax_t24;
  l_rs_result = coax_p_render_filter_layer8_grad(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
}
fn coax_p_aethyr_terrain_blend_t(p_x: f32, p_z: f32, p_f1: f32, p_w1: f32, p_o1: f32, p_f2: f32, p_w2: f32, p_o2: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_f1: f32 = p_f1;
  var l_w1: f32 = p_w1;
  var l_o1: f32 = p_o1;
  var l_f2: f32 = p_f2;
  var l_w2: f32 = p_w2;
  var l_o2: f32 = p_o2;
  var l_t: f32;
  l_t = (coax_p_aethyr_terrain_noise(((l_x * l_f1) + l_o1), ((l_z * l_f1) + l_o1)) * l_w1);
  if ((l_w2 > 0.0f))
  {
    l_t = (l_t + (coax_p_aethyr_terrain_noise(((l_x * l_f2) + l_o2), ((l_z * l_f2) + l_o2)) * l_w2));
  }
  return l_t;
}
fn coax_p_aethyr_terrain_gain(p_x: f32, p_z: f32, p_g0: f32, p_g1: f32, p_f3: f32, p_o3: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_g0: f32 = p_g0;
  var l_g1: f32 = p_g1;
  var l_f3: f32 = p_f3;
  var l_o3: f32 = p_o3;
  if ((l_g1 > 0.0f))
  {
    return (l_g0 + (l_g1 * coax_p_aethyr_terrain_noise(((l_x * l_f3) + l_o3), ((l_z * l_f3) + l_o3))));
  }
  return l_g0;
}
fn coax_p_aethyr_terrain_decode(p_a: f32, p_b: f32, p_t: f32, p_scale: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  var l_scale: f32 = p_scale;
  return coax_pow_bg_vs(coax_p_fmax((coax_p_aethyr_terrain_mix(l_a, l_b, l_t) * l_scale), 0.0f), 0x1.19999ap1f);
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
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t23 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t23(l_x, l_y);
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
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t22 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t22(l_x, l_y);
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
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_render_filter_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t24 {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_f: coax_t25;
  var l_rs_sum: vec4f;
  var l_rs_i: i32;
  var l_rs_offset: f32;
  var l_rs_at: vec2f;
  l_rs_f = coax_p_render_footprint(i32(textureDimensions(sheets).x), i32(textureDimensions(sheets).y), l_dx.x, l_dx.y, l_dy.x, l_dy.y, l_bias);
  if ((!l_rs_f.x))
  {
    return coax_t24(false, vec4f());
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
  return coax_t24(true, coax_p_scale4(l_rs_sum, (0x1.000000p0f / f32(l_rs_f.y))));
}
fn coax_p_aethyr_terrain_noise(p_x: f32, p_z: f32) -> f32 {
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
  return coax_p_aethyr_terrain_mix(coax_p_aethyr_terrain_mix(coax_p_render_noise2(l_ix, l_iz), coax_p_render_noise2((l_ix + 0x1.000000p0f), l_iz), l_sx), coax_p_aethyr_terrain_mix(coax_p_render_noise2(l_ix, (l_iz + 0x1.000000p0f)), coax_p_render_noise2((l_ix + 0x1.000000p0f), (l_iz + 0x1.000000p0f)), l_sx), l_sz);
}
fn coax_p_aethyr_terrain_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
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
fn coax_p_render_footprint(p_width: i32, p_height: i32, p_du_dx: f32, p_dv_dx: f32, p_du_dy: f32, p_dv_dy: f32, p_bias: f32) -> coax_t25 {
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
    return coax_t25(false, 0i, 0.0f, 0.0f, 0.0f);
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
  return coax_t25(true, l_taps, (l_du / f32(l_taps)), (l_dv / f32(l_taps)), ((log(l_span) / log(0x1.000000p1f)) + l_bias));
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
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_16) -> coax_io_17 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_io_17(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8));
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_17) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t17(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8));
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