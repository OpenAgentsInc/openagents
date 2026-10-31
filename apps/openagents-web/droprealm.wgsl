// ---- generated: apps/openagents-web/gpu/droprealm.coax (the module) by coax wgsl; do not edit
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
  var l_p: vec4f;
  var l_n: vec3f;
  var l_m: coax_t11;
  var l_w: vec3f;
  var l_wp: vec4f;
  var l_o: coax_t14;
  l_b = (l_id * 12u);
  l_p = coax_p_v4(coax_p_droprealm_word(l_b), coax_p_droprealm_word((l_b + 1u)), coax_p_droprealm_word((l_b + 2u)), 0x1.000000p0f);
  l_n = coax_p_v3(coax_p_droprealm_word((l_b + 3u)), coax_p_droprealm_word((l_b + 4u)), coax_p_droprealm_word((l_b + 5u)));
  l_m = objects[i32(l_instance)];
  l_w = coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p));
  l_wp = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  l_o = coax_t14();
  l_o.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(12i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(16i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(20i), l_wp));
  l_o.y = l_w;
  l_o.z = coax_p_v3(coax_p_wg_dot3(coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z), l_n), coax_p_wg_dot3(coax_p_v3(l_m.y.x, l_m.y.y, l_m.y.z), l_n), coax_p_wg_dot3(coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z), l_n));
  l_o.w = coax_p_v2(coax_p_droprealm_word((l_b + 6u)), coax_p_droprealm_word((l_b + 7u)));
  l_o.f4 = coax_p_wg_mul_f3f3(coax_p_wg_sw_rgb_f4(coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 8u))])), coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z));
  l_o.f5 = (vertices[i32((l_b + 8u))] >> u32(24u));
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t15) -> coax_t16 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t15 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t16;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t16();
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
  var l_ray: vec3f;
  var l_sky: vec3f;
  var l_down: bool;
  var l_t: f32;
  var l_hit: vec3f;
  var l_r: f32;
  var l_w2: vec2f;
  var l_wr: f32;
  var l_g: vec2f;
  var l_grid: f32;
  var l_rings: f32;
  var l_edge: f32;
  var l_fade: f32;
  var l_ground: vec3f;
  var l_off: f32;
  var l_c: vec3f;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  l_ndc = coax_p_v2((((l_p.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_p.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_ray = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(32i)), coax_at_u(i32(33i)), coax_at_u(i32(34i))), coax_p_wg_mul_ff3(l_ndc.x, coax_p_v3(coax_at_u(i32(36i)), coax_at_u(i32(37i)), coax_at_u(i32(38i))))), coax_p_wg_mul_ff3(l_ndc.y, coax_p_v3(coax_at_u(i32(40i)), coax_at_u(i32(41i)), coax_at_u(i32(42i)))));
  l_sky = coax_p_wg_add_f3f3(coax_p_v3(0x1.89374cp-7f, 0x1.a9fbe8p-7f, 0x1.47ae14p-6f), coax_p_wg_mul_f3f(coax_p_v3(0x1.eb851ep-6f, 0x1.0624dep-5f, 0x1.70a3d8p-5f), coax_p_clampf((0x1.000000p0f - (l_ray.y * 0x1.800000p1f)), 0.0f, 0x1.000000p0f)));
  l_sky = coax_p_wg_add_f3f3(l_sky, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_make_f3_f(coax_p_droprealm_stars(l_p)), coax_at_u(i32(3i))), coax_p_smoothstep(0.0f, 0x1.000000p-2f, l_ray.y)));
  l_down = (l_ray.y < (-0x1.a36e2ep-14f));
  l_t = select(0x1.f40000p9f, ((-coax_at_u(i32(29i))) / coax_p_fmin(l_ray.y, (-0x1.a36e2ep-14f))), l_down);
  l_hit = coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), coax_p_wg_mul_f3f(l_ray, coax_p_fmin(l_t, 0x1.f40000p9f)));
  l_r = coax_p_wg_length2(coax_p_v2(l_hit.x, l_hit.z));
  l_w2 = coax_p_wg_max_f2f2(coax_p_wg_fwidth_f2(coax_p_v2(l_hit.x, l_hit.z)), coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f));
  l_wr = coax_p_fmax((fwidth((l_r / 0x1.000000p1f)) * 0x1.000000p1f), 0x1.a36e2ep-14f);
  l_g = coax_p_wg_div_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f(coax_p_wg_fract_f2(coax_p_wg_add_f2f(coax_p_v2(l_hit.x, l_hit.z), 0x1.000000p-1f)), 0x1.000000p-1f)), l_w2);
  l_grid = (0x1.000000p0f - coax_p_fmin(coax_p_fmin(l_g.x, l_g.y), 0x1.000000p0f));
  l_rings = (0x1.000000p0f - coax_p_fmin(((abs((coax_p_fract(((l_r / 0x1.000000p1f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * 0x1.000000p1f) / l_wr), 0x1.000000p0f));
  l_edge = select(0.0f, (0x1.000000p0f - coax_p_smoothstep((coax_at_u(i32(31i)) - 0x1.333334p-1f), coax_at_u(i32(31i)), l_r)), l_down);
  l_fade = exp(((-l_t) * 0x1.47ae14p-6f));
  l_ground = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(0x1.70a3d8p-5f, 0x1.60418ap-5f, 0x1.99999ap-5f), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.1eb852p-5f, 0x1.eb851ep-6f, 0x1.99999ap-6f), l_grid), l_fade)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-5f, 0x1.fbe76cp-5f, 0x1.3f7ceep-4f), l_rings), 0x1.333334p-1f), l_fade));
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.19999ap-1f, 0x1.8f5c28p-1f, 0x1.000000p0f), coax_p_droprealm_lamp_near(l_hit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f))), 0x1.19999ap-1f));
  l_off = ((l_r - coax_at_u(i32(31i))) * 0x1.400000p1f);
  l_ground = coax_p_wg_add_f3f3(l_ground, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.ccccccp-2f, 0x1.333334p-1f, 0x1.8f5c28p-1f), exp(((-l_off) * l_off))), 0x1.000000p-2f));
  l_c = coax_p_wg_mix_f3f3f(l_sky, l_ground, (l_edge * coax_at_u(i32(3i))));
  return coax_p_wg_make_f4_f3f(coax_p_droprealm_panels(l_p, l_c), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t14) -> vec4f {
  var l_s: coax_t14 = p_s;
  var l_tex: vec4f;
  var l_n: vec3f;
  var l_key: f32;
  var l_lit: vec3f;
  var l_c: vec3f;
  l_tex = textureSample(sheets, sheet, l_s.w, i32(l_s.f5));
  if ((l_tex.w < 0x1.666666p-2f))
  {
    discard;
  }
  l_n = coax_p_wg_normalize3(l_s.z);
  l_key = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_wg_normalize3(coax_p_v3((-0x1.99999ap-2f), 0x1.99999ap-1f, 0x1.ccccccp-2f))));
  l_lit = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(0x1.47ae14p-3f, 0x1.5c28f6p-3f, 0x1.c28f5cp-3f), (0x1.333334p-1f + (0x1.99999ap-2f * l_n.y))), coax_p_wg_mul_f3f(coax_p_v3(0x1.19999ap-1f, 0x1.1eb852p-1f, 0x1.4cccccp-1f), l_key));
  l_lit = coax_p_wg_add_f3f3(l_lit, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.19999ap-1f, 0x1.8f5c28p-1f, 0x1.000000p0f), coax_p_droprealm_lamp_near(l_s.y, l_n)), 0x1.99999ap0f));
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f3(coax_p_v3(l_tex.x, l_tex.y, l_tex.z), l_s.f4), l_lit), coax_at_u(i32(3i)));
  return coax_p_wg_make_f4_f3f(coax_p_droprealm_panels(coax_p_v2(l_s.x.x, l_s.x.y), l_c), 0x1.000000p0f);
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
fn coax_p_droprealm_word(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(vertices[i32(l_i)]);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
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
fn coax_p_clampf(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return coax_p_fmin(coax_p_fmax(l_x, l_lo), l_hi);
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_droprealm_stars(p_p: vec2f) -> f32 {
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
  l_h = coax_p_droprealm_hash(l_g);
  if ((l_h < 0x1.dc28f6p-1f))
  {
    return 0.0f;
  }
  l_at = coax_p_wg_mul_f2f(coax_p_wg_add_f2f(coax_p_wg_add_f2f2(l_g, coax_p_wg_mul_f2f(coax_p_v2(coax_p_droprealm_hash(coax_p_wg_add_f2f(l_g, 0x1.b33334p0f)), coax_p_droprealm_hash(coax_p_wg_add_f2f(l_g, 0x1.233334p3f))), 0x1.99999ap-1f)), 0x1.99999ap-4f), l_cell);
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
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
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
fn coax_p_wg_fwidth_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(fwidth(l_p0.x), fwidth(l_p0.y));
}
fn coax_p_wg_max_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v2(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y));
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
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_droprealm_lamp_near(p_p: vec3f, p_n: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_to: vec3f;
  var l_d: f32;
  l_to = coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_p);
  l_d = coax_p_wg_length3(l_to);
  return ((coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_wg_div_f3f(l_to, coax_p_fmax(l_d, 0x1.0624dep-10f)))) * coax_at_u(i32(27i))) / (0x1.000000p0f + ((0x1.70a3d8p-3f * l_d) * l_d)));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_droprealm_panels(p_p: vec2f, p_under: vec3f) -> vec3f {
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
      l_c = coax_p_droprealm_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 44i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 44i)));
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
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_droprealm_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
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
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
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
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_droprealm_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
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
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
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