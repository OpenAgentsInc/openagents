// ---- generated: apps/openagents-web/gpu/worldbuilder.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t14 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec3f,
  f5: u32,
}
struct coax_io_14 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec3f,
  @location(4) @interpolate(flat) f5: u32,
}
struct coax_t15 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_15 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t16 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_16 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t19 {
  x: bool,
  y: vec4f,
}
struct coax_t20 {
  x: bool,
  y: i32,
  z: f32,
  w: f32,
  f4: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 43>;
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
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t14 {
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
  var l_o: coax_t14;
  var l_c0: vec3f;
  var l_c1: vec3f;
  var l_c2: vec3f;
  var l_rgb: vec4f;
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
  l_o = coax_t14();
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
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t15) -> coax_t16 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t15 = p_in_;
  var l_a: vec4f;
  var l_o: coax_t16;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_o = coax_t16();
  l_ndc = coax_p_v2((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_o.x = coax_p_v4(l_ndc.x, l_ndc.y, 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_in_.y;
  l_o.w = l_in_.z;
  l_o.f4 = l_in_.w;
  return l_o;
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
  var l_fine: f32;
  var l_coarse: f32;
  var l_fineFade: f32;
  var l_axes: f32;
  var l_dist: f32;
  var l_fog: f32;
  var l_ground: vec3f;
  var l_hd: f32;
  var l_ring: f32;
  var l_disc: f32;
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
  l_fine = coax_p_worldbuilder_line(l_xz, l_w1);
  l_coarse = coax_p_worldbuilder_line(coax_p_wg_div_f2f(l_xz, 0x1.400000p3f), coax_p_wg_div_f2f(l_w1, 0x1.400000p3f));
  l_fineFade = (0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-4f, 0x1.ccccccp-2f, coax_p_fmax(l_w1.x, l_w1.y)));
  l_axes = (0x1.000000p0f - coax_p_fmin((coax_p_fmin((abs(l_hit.x) / l_w1.x), (abs(l_hit.z) / l_w1.y)) * 0x1.333334p-1f), 0x1.000000p0f));
  l_dist = (l_t * l_rl);
  l_fog = exp(((-l_dist) * 0x1.89374cp-7f));
  l_ground = coax_p_v3(0x1.89374cp-7f, 0x1.47ae14p-6f, 0x1.eb851ep-6f);
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-5f, 0x1.47ae14p-3f, 0x1.c28f5cp-3f), (((l_fine * l_fineFade) * 0x1.19999ap-1f) * l_fog)));
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-4f, 0x1.47ae14p-2f, 0x1.ae147ap-2f), (l_coarse * l_fog)));
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-3f, 0x1.ae147ap-2f, 0x1.000000p-1f), ((l_axes * 0x1.000000p-1f) * l_fog)));
  l_hd = coax_p_wg_length2(coax_p_v2((l_hit.x - coax_at_u(i32(24i))), (l_hit.z - coax_at_u(i32(26i)))));
  l_ring = (0x1.000000p0f - coax_p_smoothstep(0.0f, coax_p_fmax((coax_p_wg_length2(l_w1) * 0x1.333334p0f), 0x1.47ae14p-6f), abs((l_hd - 0x1.333334p-1f))));
  l_disc = (exp((((-l_hd) * l_hd) * 0x1.800000p1f)) * 0x1.666666p-2f);
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.ccccccp-2f), ((l_ring + l_disc) * coax_at_u(i32(27i)))));
  l_ground = coax_p_wg_mix_f3f3f(l_sky, l_ground, ((l_fog * 0x1.b33334p-1f) + 0x1.333334p-3f));
  l_edge = select(0.0f, coax_p_smoothstep(0.0f, 0x1.47ae14p-6f, (-l_up)), l_down);
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_sky, l_ground, l_edge), coax_at_u(i32(3i)));
  return coax_p_wg_make_f4_f3f(coax_p_worldbuilder_panels(l_p, coax_p_worldbuilder_out(l_c)), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t14) -> vec4f {
  var l_s: coax_t14 = p_s;
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
  l_textured = (l_s.f5 < 255u);
  l_layer = select(0i, i32(l_s.f5), l_textured);
  l_tex = coax_p_render_sample_layer8(l_s.w, l_layer);
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
  return coax_p_wg_make_f4_f3f(coax_p_worldbuilder_panels(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_worldbuilder_out(coax_p_wg_mul_f3f(l_c, coax_at_u(i32(3i))))), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t16) -> vec4f {
  var l_v: coax_t16 = p_v;
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
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_worldbuilder_out(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_v3(sqrt(coax_p_fmax(l_c.x, 0.0f)), sqrt(coax_p_fmax(l_c.y, 0.0f)), sqrt(coax_p_fmax(l_c.z, 0.0f)));
}
fn coax_p_worldbuilder_panels(p_p: vec2f, p_under: vec3f) -> vec3f {
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
      l_c = coax_p_worldbuilder_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 44i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 44i)));
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
fn coax_p_render_sample_layer8(p_uv: vec2f, p_layer: i32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_rs_no_bias: f32;
  l_rs_no_bias = 0.0f;
  return coax_p_render_sample_layer8_bias(l_uv, l_layer, l_rs_no_bias);
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
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_worldbuilder_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
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
  l_corner = coax_p_fmin((0x1.400000p3f * l_s.x), coax_p_fmin(l_half.x, l_half.y));
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_d = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  if ((l_d > 0x1.000000p0f))
  {
    return l_under;
  }
  l_over = (l_s.z >= 0x1.000000p1f);
  l_lit = (l_over || ((l_s.z >= 0x1.000000p0f) && (l_s.z < 0x1.000000p1f)));
  l_fill = select(coax_p_v3(0x1.eb851ep-6f, 0x1.70a3d8p-5f, 0x1.fbe76cp-5f), coax_p_v3(0x1.c28f5cp-5f, 0x1.5c28f6p-4f, 0x1.c28f5cp-4f), l_over);
  l_line = select(coax_p_v3(0x1.99999ap-3f, 0x1.851eb8p-2f, 0x1.eb851ep-2f), coax_p_v3(0x1.ccccccp-2f, 0x1.b33334p-1f, 0x1.000000p0f), l_lit);
  l_w = (0x1.400000p0f * l_s.x);
  l_inside = coax_p_clampf((0x1.000000p-1f - l_d), 0.0f, 0x1.000000p0f);
  l_edge = coax_p_clampf((((l_w * 0x1.000000p-1f) + 0x1.000000p-1f) - abs((l_d + (l_w * 0x1.000000p-1f)))), 0.0f, 0x1.000000p0f);
  l_a = coax_p_clampf(l_s.y, 0.0f, 0x1.000000p0f);
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_under, l_fill, (l_a * l_inside)), l_line, (l_edge * coax_p_fmin(0x1.000000p0f, (l_a + 0x1.99999ap-4f))));
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
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
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
fn coax_p_render_sample_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_result: coax_t19;
  l_rs_result = coax_p_render_filter_layer8_grad(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
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
fn coax_p_render_filter_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t19 {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_f: coax_t20;
  var l_rs_sum: vec4f;
  var l_rs_i: i32;
  var l_rs_offset: f32;
  var l_rs_at: vec2f;
  l_rs_f = coax_p_render_footprint(i32(textureDimensions(sheets).x), i32(textureDimensions(sheets).y), l_dx.x, l_dx.y, l_dy.x, l_dy.y, l_bias);
  if ((!l_rs_f.x))
  {
    return coax_t19(false, vec4f());
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
  return coax_t19(true, coax_p_scale4(l_rs_sum, (0x1.000000p0f / f32(l_rs_f.y))));
}
fn coax_p_render_footprint(p_width: i32, p_height: i32, p_du_dx: f32, p_dv_dx: f32, p_du_dy: f32, p_dv_dy: f32, p_bias: f32) -> coax_t20 {
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
    return coax_t20(false, 0i, 0.0f, 0.0f, 0.0f);
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
  return coax_t20(true, l_taps, (l_du / f32(l_taps)), (l_dv / f32(l_taps)), ((log(l_span) / log(0x1.000000p1f)) + l_bias));
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
fn coax_p_render_footprint_finite(p_x: f32) -> bool {
  var l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_footprint_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (((l_a + l_b) + abs((l_a - l_b))) * 0x1.000000p-1f);
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_14 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_14(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_15) -> coax_io_16 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_16(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_14) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t14(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_16) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_r;
}