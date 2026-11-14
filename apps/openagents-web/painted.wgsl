// ---- generated: apps/openagents-web/gpu/painted.coax (the module) by coax wgsl; do not edit
struct coax_t10 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t13 {
  x: vec4f,
}
struct coax_io_13 {
  @builtin(position) x: vec4f,
}
struct coax_t14 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: f32,
}
struct coax_io_14 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: f32,
}
struct coax_t15 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec3f,
  f5: vec3f,
  f6: vec4f,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec3f,
  @location(4) f5: vec3f,
  @location(5) @interpolate(flat) f6: vec4f,
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
struct coax_t20 {
  x: vec3f,
  y: vec3f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 55>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> mesh: array<f32>;
@group(1) @binding(1) var<storage, read> objects: array<coax_t10>;
@group(1) @binding(2) var<storage, read> heights: array<f32>;
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
fn coax_p_bg_vs(p_i: u32, p__iid: u32) -> vec4f {
  let l_i: u32 = p_i;
  let l__iid: u32 = p__iid;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0x1.000000p0f, 0x1.000000p0f);
}
fn coax_p_shadow_vs(p_id: u32, p_instance: u32) -> coax_t13 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  var l_m: coax_t10;
  var l_o: i32;
  var l_local: vec3f;
  var l_c: f32;
  var l_sn: f32;
  var l_w: vec3f;
  var l_s: coax_t13;
  l_m = objects[i32(l_instance)];
  l_o = (i32(l_id) * 12i);
  l_local = coax_p_scale3(coax_p_v3(mesh[l_o], mesh[(l_o + 1i)], mesh[(l_o + 2i)]), l_m.y.x);
  l_c = cos(l_m.x.w);
  l_sn = sin(l_m.x.w);
  l_w = coax_p_v3((((l_local.x * l_c) + (l_local.z * l_sn)) + l_m.x.x), (l_local.y + l_m.x.y), ((((0.0f - l_local.x) * l_sn) + (l_local.z * l_c)) + l_m.x.z));
  l_s = coax_t13();
  l_s.x = coax_p_sty_clip(l_w, 24i);
  if (coax_p_sty_is(mesh[(l_o + 6i)], 1i))
  {
    l_s.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
  }
  return l_s;
}
fn coax_p_grass_vs(p_v: u32, p_i: u32) -> coax_t14 {
  let l_v: u32 = p_v;
  let l_i: u32 = p_i;
  var l_o: coax_t14;
  var l_cols: u32;
  var l_cx: f32;
  var l_cz: f32;
  var l_cell: f32;
  var l_gx: f32;
  var l_gz: f32;
  var l_x: f32;
  var l_z: f32;
  var l_eye: vec3f;
  var l_ex: f32;
  var l_ez: f32;
  var l_dist: f32;
  var l_reach: f32;
  var l_grown: f32;
  var l_cover: f32;
  var l_clear: f32;
  var l_tuft: f32;
  var l_rnd: f32;
  var l_thin: f32;
  var l_tall: f32;
  var l_y: f32;
  var l_il: f32;
  var l_wide: f32;
  var l_ax: f32;
  var l_az: f32;
  var l_ang: f32;
  var l_sway: f32;
  var l_lean: f32;
  var l_w: vec3f;
  l_o = coax_t14();
  l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
  l_o.y = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.z = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.w = 0.0f;
  l_cols = coax_p_wg_max_u(coax_p_wg_u32_f(coax_at_u(i32(87i))), 1u);
  l_cx = f32((l_i % l_cols));
  l_cz = f32((l_i / l_cols));
  l_cell = coax_at_u(i32(86i));
  l_gx = (floor((coax_at_u(i32(84i)) / l_cell)) + l_cx);
  l_gz = (floor((coax_at_u(i32(85i)) / l_cell)) + l_cz);
  l_x = ((l_gx + coax_p_sty_hash2(l_gx, l_gz)) * l_cell);
  l_z = ((l_gz + coax_p_sty_hash2((l_gx + 0x1.c80000p5f), (l_gz + 0x1.a00000p3f))) * l_cell);
  l_eye = coax_p_wg_at3(4i);
  l_ex = (l_x - l_eye.x);
  l_ez = (l_z - l_eye.z);
  l_dist = sqrt(((l_ex * l_ex) + (l_ez * l_ez)));
  l_reach = coax_at_u(i32(91i));
  l_grown = coax_p_sty_table(l_x, l_z, 1i);
  l_cover = (coax_p_fmin(l_grown, 0x1.000000p0f) * (0x1.000000p0f - coax_p_sty_table(l_x, l_z, 2i)));
  l_clear = coax_p_smoothstep((0.0f - 0x1.99999ap-4f), 0x1.000000p-1f, coax_p_sty_path(l_x, l_z));
  l_tuft = (0x1.666666p-1f + (0x1.000000p-1f * coax_p_sty_noise2((l_x * 0x1.666666p0f), (l_z * 0x1.666666p0f))));
  l_rnd = coax_p_sty_hash2((l_gx + 0x1.c00000p2f), (l_gz + 0x1.6c0000p6f));
  l_thin = (0x1.000000p0f - coax_p_smoothstep((l_reach * 0x1.ccccccp-2f), l_reach, l_dist));
  l_tall = ((((((coax_at_u(i32(89i)) * coax_p_fmax(l_grown, 0x1.000000p0f)) * (0x1.19999ap-1f + (0x1.333334p-1f * l_rnd))) * l_tuft) * l_clear) * coax_p_smoothstep(0x1.666666p-2f, 0x1.666666p-1f, l_cover)) * coax_p_smoothstep(0.0f, 0x1.000000p-2f, ((l_thin - (l_rnd * 0x1.19999ap-1f)) + 0x1.99999ap-3f)));
  if ((l_tall < 0x1.eb851ep-6f))
  {
    return l_o;
  }
  l_y = coax_p_sty_table(l_x, l_z, 0i);
  l_il = (0x1.000000p0f / coax_p_fmax(l_dist, 0x1.0624dep-10f));
  l_wide = ((coax_at_u(i32(90i)) * (0x1.666666p-1f + (0x1.333334p-1f * coax_p_sty_hash2((l_gx + 0x1.800000p1f), (l_gz + 0x1.480000p5f))))) * (0x1.000000p0f + (l_dist * 0x1.99999ap-5f)));
  l_ax = ((((0.0f - l_ez) * l_il) * l_wide) * 0x1.000000p-1f);
  l_az = (((l_ex * l_il) * l_wide) * 0x1.000000p-1f);
  l_ang = (coax_p_sty_hash2((l_gx + 0x1.300000p4f), (l_gz + 0x1.400000p2f)) * 0x1.921fb6p2f);
  l_sway = (sin((((coax_at_u(i32(2i)) * 0x1.4cccccp0f) + (l_x * 0x1.666666p-1f)) + (l_z * 0x1.000000p-1f))) * 0x1.1eb852p-5f);
  l_lean = (l_tall * (0x1.eb851ep-4f + (0x1.333334p-2f * coax_p_sty_hash2((l_gx + 0x1.700000p4f), (l_gz + 0x1.1c0000p6f)))));
  l_w = coax_p_v3(l_x, l_y, l_z);
  if ((l_v == 0u))
  {
    l_w = coax_p_v3((l_x - l_ax), l_y, (l_z - l_az));
  }
  else
  {
    if ((l_v == 1u))
    {
      l_w = coax_p_v3((l_x + l_ax), l_y, (l_z + l_az));
    }
    else
    {
      l_w = coax_p_v3(((l_x + (cos(l_ang) * l_lean)) + l_sway), (l_y + l_tall), (l_z + (sin(l_ang) * l_lean)));
      l_o.w = 0x1.000000p0f;
    }
  }
  l_o.y = coax_p_v3(l_x, l_y, l_z);
  l_o.z = coax_p_scale3(coax_p_sty_grass_colour(l_x, l_z, l_o.w), (0x1.c28f5cp-1f + (0x1.eb851ep-3f * l_rnd)));
  l_o.x = coax_p_sty_clip(l_w, 8i);
  return l_o;
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  return coax_p_sty_stand(l_id, l_instance);
}
fn coax_p_water_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  return coax_p_sty_stand(l_id, l_instance);
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t16) -> coax_t17 {
  let l__vid2: u32 = p__vid2;
  let l__iid3: u32 = p__iid3;
  let l_in_: coax_t16 = p_in_;
  var l_o: coax_t17;
  l_o = coax_t17();
  l_o.x = coax_p_v4((((l_in_.x.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_in_.x.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_in_.x.z, l_in_.x.w);
  l_o.z = l_in_.y;
  l_o.w = l_in_.z;
  l_o.f4 = l_in_.w;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  let l_pos: vec4f = p_pos;
  var l_c: vec3f;
  l_c = coax_p_sty_encode(coax_p_sty_decode(coax_p_sty_sky(coax_p_sty_ray(l_pos))));
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_shadow_fs(p_s: coax_t13) {
  let l_s: coax_t13 = p_s;
}
fn coax_p_grass_fs(p_g: coax_t14) -> vec4f {
  let l_g: coax_t14 = p_g;
  var l_up: vec3f;
  var l_shadow: f32;
  var l_root: f32;
  var l_c: vec3f;
  l_up = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_shadow = coax_p_sty_shadow(l_g.y, l_up);
  l_root = (0x1.8f5c28p-1f + (0x1.c28f5cp-3f * l_g.w));
  l_c = coax_p_scale3(coax_p_sty_lit(coax_p_sty_decode(l_g.z), l_up, (coax_p_wg_at3(40i)).y, l_shadow), l_root);
  l_c = coax_p_sty_encode(coax_p_sty_haze(l_c, l_g.y));
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t15) -> vec4f {
  let l_s: coax_t15 = p_s;
  var l_n: vec3f;
  var l_sun: vec3f;
  var l_m: coax_t20;
  var l_albedo: vec3f;
  var l_ndl: f32;
  var l_shadow: f32;
  var l_c: vec3f;
  l_n = coax_p_normalize3(l_s.z);
  l_sun = coax_p_wg_at3(40i);
  l_m = coax_p_sty_surface(l_s);
  l_albedo = coax_p_sty_decode(coax_p_mul3(l_m.x, l_s.f5));
  l_ndl = dot(l_n, l_sun);
  l_shadow = coax_p_sty_shadow(l_s.y, l_n);
  if ((coax_p_sty_is(l_s.f6.x, 11i) || coax_p_sty_is(l_s.f6.x, 25i)))
  {
    l_ndl = 0x1.000000p0f;
    l_shadow = coax_p_mix(0x1.000000p-1f, 0x1.000000p0f, l_shadow);
  }
  l_c = coax_p_add3(coax_p_sty_lit(l_albedo, l_n, l_ndl, l_shadow), coax_p_sty_decode(l_m.y));
  l_c = coax_p_sty_encode(coax_p_sty_haze(l_c, l_s.y));
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_water_transmit_fs(p_s: coax_t15) -> vec4f {
  let l_s: coax_t15 = p_s;
  var l_k: f32;
  l_k = (0x1.000000p0f - (coax_p_sty_wet(l_s)).w);
  return coax_p_v4(l_k, l_k, l_k, 0x1.000000p0f);
}
fn coax_p_water_emit_fs(p_s: coax_t15) -> vec4f {
  let l_s: coax_t15 = p_s;
  var l_w: vec4f;
  var l_up: vec3f;
  var l_c: vec3f;
  var l_shadow: f32;
  l_w = coax_p_sty_wet(l_s);
  l_up = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_c = coax_p_sty_decode(coax_p_mul3(coax_p_v3(l_w.x, l_w.y, l_w.z), l_s.f5));
  if ((coax_p_sty_is(l_s.f6.x, 26i) || coax_p_sty_is(l_s.f6.x, 27i)))
  {
    l_shadow = coax_p_mix(0x1.ccccccp-2f, 0x1.000000p0f, coax_p_sty_shadow(l_s.y, l_up));
    l_c = coax_p_sty_lit(l_c, l_up, 0x1.000000p0f, l_shadow);
  }
  l_c = coax_p_scale3(coax_p_sty_encode(coax_p_sty_haze(l_c, l_s.y)), l_w.w);
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t17) -> vec4f {
  let l_v: coax_t17 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  var l_k: f32;
  l_along = coax_p_fmax(coax_p_fmax((0.0f - l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((0.0f - 0x1.400000p2f) * l_g) * l_g)) * 0x1.1eb852p-2f);
  l_k = (((l_core + l_halo) * l_v.z.z) * coax_at_u(i32(3i)));
  return coax_p_v4((l_v.w.x * l_k), (l_v.w.y * l_k), (l_v.w.z * l_k), 0x1.000000p0f);
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  let l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_sty_clip(p_w: vec3f, p_rows: i32) -> vec4f {
  let l_w: vec3f = p_w;
  let l_rows: i32 = p_rows;
  var l_p: vec4f;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  return coax_p_v4(dot(coax_p_wg_at4(l_rows), l_p), dot(coax_p_wg_at4((l_rows + 4i)), l_p), dot(coax_p_wg_at4((l_rows + 8i)), l_p), dot(coax_p_wg_at4((l_rows + 12i)), l_p));
}
fn coax_p_sty_is(p_material: f32, p_which: i32) -> bool {
  let l_material: f32 = p_material;
  let l_which: i32 = p_which;
  return (abs((l_material - f32(l_which))) < 0x1.000000p-1f);
}
fn coax_p_wg_u32_f(p_x: f32) -> u32 {
  let l_x: f32 = p_x;
  return select(0u, u32(coax_p_fmin(l_x, 0x1.fffffep31f)), (l_x > 0.0f));
}
fn coax_p_wg_max_u(p_a: u32, p_b: u32) -> u32 {
  let l_a: u32 = p_a;
  let l_b: u32 = p_b;
  return select(l_a, l_b, (l_b > l_a));
}
fn coax_p_sty_hash2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_sty_cell(l_x), coax_p_sty_cell(l_y)));
}
fn coax_p_wg_at3(p_at: i32) -> vec3f {
  let l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
}
fn coax_p_sty_table(p_x: f32, p_z: f32, p_field: i32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  let l_field: i32 = p_field;
  var l_n: i32;
  var l_fx: f32;
  var l_fz: f32;
  var l_i: i32;
  var l_j: i32;
  var l_tx: f32;
  var l_tz: f32;
  var l_row: i32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_n = i32(coax_at_u(i32(95i)));
  if ((l_n < 1i))
  {
    return select(0.0f, 0x1.000000p0f, (l_field == 1i));
  }
  l_fx = ((l_x - coax_at_u(i32(92i))) / coax_at_u(i32(94i)));
  l_fz = ((l_z - coax_at_u(i32(93i))) / coax_at_u(i32(94i)));
  l_i = coax_p_wg_clamp_i(i32(floor(l_fx)), 0i, (l_n - 1i));
  l_j = coax_p_wg_clamp_i(i32(floor(l_fz)), 0i, (l_n - 1i));
  l_tx = coax_p_saturate((l_fx - f32(l_i)));
  l_tz = coax_p_saturate((l_fz - f32(l_j)));
  l_row = (l_n + 1i);
  l_a = heights[(i32((((l_j * l_row) + l_i) * 4i)) + l_field)];
  l_b = heights[(i32(((((l_j * l_row) + l_i) + 1i) * 4i)) + l_field)];
  l_c = heights[(i32(((((l_j + 1i) * l_row) + l_i) * 4i)) + l_field)];
  l_d = heights[(i32((((((l_j + 1i) * l_row) + l_i) + 1i) * 4i)) + l_field)];
  return coax_p_mix(coax_p_mix(l_a, l_b, l_tx), coax_p_mix(l_c, l_d, l_tx), l_tz);
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b < l_a));
}
fn coax_p_sty_path(p_x: f32, p_z: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  var l_d: f32;
  var l_count: i32;
  var l_i: i32;
  var l_o: i32;
  var l_ax: f32;
  var l_az: f32;
  var l_bx: f32;
  var l_bz: f32;
  var l_t: f32;
  var l_ex: f32;
  var l_ez: f32;
  l_d = 0x1.f40000p9f;
  l_count = i32(coax_at_u(i32(96i)));
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 12i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      if ((i32(l_i) < l_count))
      {
        l_o = (100i + (l_i * 8i));
        l_ax = coax_at_u(i32(l_o));
        l_az = coax_at_u(i32((l_o + 1i)));
        l_bx = (coax_at_u(i32((l_o + 2i))) - l_ax);
        l_bz = (coax_at_u(i32((l_o + 3i))) - l_az);
        l_t = coax_p_saturate(((((l_x - l_ax) * l_bx) + ((l_z - l_az) * l_bz)) / coax_p_fmax(((l_bx * l_bx) + (l_bz * l_bz)), 0x1.a36e2ep-14f)));
        l_ex = ((l_x - l_ax) - (l_bx * l_t));
        l_ez = ((l_z - l_az) - (l_bz * l_t));
        l_d = coax_p_fmin(l_d, (sqrt(((l_ex * l_ex) + (l_ez * l_ez))) - coax_at_u(i32((l_o + 4i)))));
      }
    }
  }
  return ((l_d + ((coax_p_sty_noise2((l_x * 0x1.99999ap-2f), (l_z * 0x1.99999ap-2f)) - 0x1.000000p-1f) * 0x1.000000p-1f)) + ((coax_p_sty_noise2(((l_x * 0x1.0cccccp1f) + 0x1.400000p2f), (l_z * 0x1.0cccccp1f)) - 0x1.000000p-1f) * 0x1.c28f5cp-3f));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_sty_noise2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  var l_ix: f32;
  var l_iy: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_sx: f32;
  var l_sy: f32;
  l_ix = floor(l_x);
  l_iy = floor(l_y);
  l_fx = (l_x - l_ix);
  l_fy = (l_y - l_iy);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sy = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  return coax_p_mix(coax_p_mix(coax_p_sty_hash2(l_ix, l_iy), coax_p_sty_hash2((l_ix + 0x1.000000p0f), l_iy), l_sx), coax_p_mix(coax_p_sty_hash2(l_ix, (l_iy + 0x1.000000p0f)), coax_p_sty_hash2((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f)), l_sx), l_sy);
}
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b > l_a));
}
fn coax_p_sty_grass_colour(p_x: f32, p_z: f32, p_along: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  let l_along: f32 = p_along;
  var l_k: f32;
  var l_root: vec3f;
  var l_tip: vec3f;
  l_k = coax_p_sty_meadow(l_x, l_z);
  l_root = coax_p_mix3(coax_p_v3(0x1.851eb8p-3f, 0x1.28f5c2p-2f, 0x1.5c28f6p-3f), coax_p_v3(0x1.333334p-2f, 0x1.851eb8p-2f, 0x1.ae147ap-3f), l_k);
  l_tip = coax_p_mix3(coax_p_v3(0x1.99999ap-3f, 0x1.70a3d8p-2f, 0x1.851eb8p-3f), coax_p_v3(0x1.51eb86p-1f, 0x1.5c28f6p-1f, 0x1.8f5c28p-2f), coax_p_smoothstep(0x1.5c28f6p-2f, 0x1.666666p-1f, l_k));
  return coax_p_mix3(l_root, l_tip, l_along);
}
fn coax_p_sty_stand(p_id: u32, p_instance: u32) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  var l_m: coax_t10;
  var l_o: i32;
  var l_local: vec3f;
  var l_n: vec3f;
  var l_c: f32;
  var l_sn: f32;
  var l_w: vec3f;
  var l_s: coax_t15;
  l_m = objects[i32(l_instance)];
  l_o = (i32(l_id) * 12i);
  l_local = coax_p_scale3(coax_p_v3(mesh[l_o], mesh[(l_o + 1i)], mesh[(l_o + 2i)]), l_m.y.x);
  l_n = coax_p_v3(mesh[(l_o + 3i)], mesh[(l_o + 4i)], mesh[(l_o + 5i)]);
  l_c = cos(l_m.x.w);
  l_sn = sin(l_m.x.w);
  l_w = coax_p_v3((((l_local.x * l_c) + (l_local.z * l_sn)) + l_m.x.x), (l_local.y + l_m.x.y), ((((0.0f - l_local.x) * l_sn) + (l_local.z * l_c)) + l_m.x.z));
  l_s = coax_t15();
  l_s.y = l_w;
  l_s.z = coax_p_v3(((l_n.x * l_c) + (l_n.z * l_sn)), l_n.y, (((0.0f - l_n.x) * l_sn) + (l_n.z * l_c)));
  l_s.w = coax_p_v2(mesh[(l_o + 7i)], mesh[(l_o + 8i)]);
  l_s.f4 = coax_p_v3(mesh[(l_o + 9i)], mesh[(l_o + 10i)], mesh[(l_o + 11i)]);
  l_s.f5 = coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z);
  l_s.f6 = coax_p_v4(mesh[(l_o + 6i)], l_m.z.w, l_m.y.z, 0.0f);
  l_s.x = coax_p_sty_clip(l_w, 8i);
  return l_s;
}
fn coax_p_sty_ray(p_pos: vec4f) -> vec3f {
  let l_pos: vec4f = p_pos;
  return coax_p_normalize3(coax_p_add3(coax_p_scale3(coax_p_wg_at3(80i), coax_at_u(i32(7i))), coax_p_add3(coax_p_scale3(coax_p_wg_at3(72i), (l_pos.x - (coax_at_u(i32(0i)) * 0x1.000000p-1f))), coax_p_scale3(coax_p_wg_at3(76i), ((coax_at_u(i32(1i)) * 0x1.000000p-1f) - l_pos.y)))));
}
fn coax_p_sty_sky(p_rd: vec3f) -> vec3f {
  let l_rd: vec3f = p_rd;
  var l_zenith: vec3f;
  var l_horizon: vec3f;
  var l_up: f32;
  var l_c: vec3f;
  var l_size: f32;
  var l_az: f32;
  var l_ax: f32;
  var l_ay: f32;
  var l_cover: f32;
  var l_d: f32;
  var l_band: f32;
  var l_wide: f32;
  var l_edge: f32;
  var l_sun: vec3f;
  var l_saz: f32;
  var l_turn: f32;
  var l_toward: f32;
  var l_lit: f32;
  var l_bright: f32;
  var l_shade: vec3f;
  var l_white: vec3f;
  var l_cloud: vec3f;
  var l_el: f32;
  var l_i: i32;
  var l_o: i32;
  var l_tall: f32;
  var l_turn_28: f32;
  var l_px: f32;
  var l_py: f32;
  var l_half: f32;
  var l_seed: f32;
  var l_in_: f32;
  var l_w: f32;
  var l_over: f32;
  var l_rim: f32;
  var l_body: f32;
  var l_heap: vec3f;
  l_zenith = coax_p_wg_at3(64i);
  l_horizon = coax_p_wg_at3(68i);
  l_up = coax_p_saturate(l_rd.y);
  l_c = coax_p_mix3(l_horizon, l_zenith, coax_pow_bg_vs(l_up, 0x1.3d70a4p-1f));
  l_size = coax_p_fmax(coax_at_u(i32(71i)), 0x1.99999ap-5f);
  l_az = coax_p_wg_atan2(l_rd.x, (0.0f - l_rd.z));
  l_ax = ((((l_az + coax_at_u(i32(62i))) * 0x1.19999ap1f) / l_size) + (coax_at_u(i32(2i)) * 0x1.89374cp-9f));
  l_ay = ((l_up * 0x1.b33334p1f) / l_size);
  l_cover = coax_at_u(i32(67i));
  l_d = coax_p_sty_cloud_field(l_ax, l_ay, l_cover);
  l_band = (coax_p_smoothstep(0x1.47ae14p-5f, 0x1.c28f5cp-3f, l_up) * (0x1.000000p0f - coax_p_smoothstep(0x1.666666p-1f, 0x1.f5c290p-1f, l_up)));
  l_wide = coax_p_fmax(fwidth(l_d), 0x1.0624dep-11f);
  l_edge = ((coax_p_smoothstep((0x1.000000p-1f - l_wide), (0x1.000000p-1f + l_wide), l_d) * l_band) * coax_p_smoothstep(0.0f, 0x1.47ae14p-5f, l_cover));
  if ((l_edge > 0.0f))
  {
    l_sun = coax_p_wg_at3(40i);
    l_saz = coax_p_wg_atan2(l_sun.x, (0.0f - l_sun.z));
    l_turn = (l_saz - l_az);
    l_turn = (l_turn - (0x1.921fb6p2f * floor(((l_turn / 0x1.921fb6p2f) + 0x1.000000p-1f))));
    l_toward = (0x1.70a3d8p-4f * coax_p_clampf((l_turn * 0x1.400000p1f), (0.0f - 0x1.000000p0f), 0x1.000000p0f));
    l_lit = (l_d - coax_p_sty_cloud_field((l_ax + l_toward), (l_ay + 0x1.47ae14p-3f), l_cover));
    l_bright = coax_p_smoothstep((0.0f - 0x1.47ae14p-7f), 0x1.70a3d8p-5f, l_lit);
    l_shade = coax_p_v3(0x1.7ae148p-1f, 0x1.99999ap-1f, 0x1.ccccccp-1f);
    l_white = coax_p_v3(0x1.000000p0f, 0x1.fae148p-1f, 0x1.eb851ep-1f);
    l_cloud = coax_p_mix3(l_shade, l_white, coax_p_saturate(((l_bright * 0x1.b33334p-1f) + 0x1.eb851ep-4f)));
    l_cloud = coax_p_mix3(l_horizon, l_cloud, coax_p_smoothstep(0x1.47ae14p-6f, 0x1.333334p-2f, l_up));
    l_c = coax_p_mix3(l_c, l_cloud, l_edge);
  }
  l_el = coax_p_wg_atan2(l_rd.y, sqrt(((l_rd.x * l_rd.x) + (l_rd.z * l_rd.z))));
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 6i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      l_o = (196i + (l_i * 4i));
      l_tall = coax_at_u(i32((l_o + 3i)));
      if ((l_tall > 0x1.0624dep-10f))
      {
        l_turn_28 = (l_az - coax_at_u(i32(l_o)));
        l_turn_28 = (l_turn_28 - (0x1.921fb6p2f * floor(((l_turn_28 / 0x1.921fb6p2f) + 0x1.000000p-1f))));
        l_px = ((l_turn_28 * cos(coax_at_u(i32((l_o + 1i))))) / l_tall);
        l_py = ((l_el - coax_at_u(i32((l_o + 1i)))) / l_tall);
        l_half = (coax_at_u(i32((l_o + 2i))) / l_tall);
        if ((((abs(l_px) < (l_half + 0x1.ccccccp-1f)) && (l_py > (0.0f - 0x1.99999ap-3f))) && (l_py < 0x1.b33334p0f)))
        {
          l_seed = (f32(l_i) + 0x1.000000p0f);
          l_in_ = coax_p_sty_cumulus(l_px, l_py, l_half, l_seed);
          l_w = (0x1.800000p0f / (coax_p_fmax(coax_at_u(i32(7i)), 0x1.000000p0f) * l_tall));
          l_over = coax_p_smoothstep((0.0f - l_w), l_w, l_in_);
          l_rim = coax_p_smoothstep((0.0f - 0x1.47ae14p-6f), 0x1.99999ap-4f, (l_in_ - coax_p_sty_cumulus((l_px + 0x1.eb851ep-6f), (l_py + 0x1.1eb852p-3f), l_half, l_seed)));
          l_body = coax_p_smoothstep(0x1.eb851ep-5f, 0x1.3d70a4p-1f, (l_py + ((coax_p_sty_noise2(((l_px * 0x1.19999ap1f) + l_seed), (l_py * 0x1.19999ap1f)) - 0x1.000000p-1f) * 0x1.99999ap-2f)));
          l_heap = coax_p_mix3(coax_p_v3(0x1.8f5c28p-1f, 0x1.b33334p-1f, 0x1.e147aep-1f), coax_p_v3(0x1.000000p0f, 0x1.fd70a4p-1f, 0x1.f0a3d8p-1f), coax_p_saturate(((l_body * 0x1.ccccccp-1f) + (l_rim * 0x1.ccccccp-2f))));
          l_c = coax_p_mix3(l_c, l_heap, l_over);
        }
      }
    }
  }
  if ((l_rd.y < 0.0f))
  {
    return coax_p_mix3(l_horizon, coax_p_wg_at3(56i), coax_p_saturate((0.0f - (l_rd.y * 0x1.000000p3f))));
  }
  return l_c;
}
fn coax_p_sty_decode(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_v3(coax_pow_bg_vs(coax_p_fmax(l_c.x, 0.0f), 0x1.19999ap1f), coax_pow_bg_vs(coax_p_fmax(l_c.y, 0.0f), 0x1.19999ap1f), coax_pow_bg_vs(coax_p_fmax(l_c.z, 0.0f), 0x1.19999ap1f));
}
fn coax_p_sty_encode(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  var l_l: f32;
  var l_s: vec3f;
  l_l = (((l_c.x * 0x1.b367a0p-3f) + (l_c.y * 0x1.6e2eb2p-1f)) + (l_c.z * 0x1.27bb30p-4f));
  l_s = coax_p_v3(coax_p_fmax((l_l + ((l_c.x - l_l) * 0x1.0a3d70p0f)), 0.0f), coax_p_fmax((l_l + ((l_c.y - l_l) * 0x1.0a3d70p0f)), 0.0f), coax_p_fmax((l_l + ((l_c.z - l_l) * 0x1.0a3d70p0f)), 0.0f));
  return coax_p_v3(coax_pow_bg_vs(coax_p_saturate(l_s.x), (0x1.000000p0f / 0x1.19999ap1f)), coax_pow_bg_vs(coax_p_saturate(l_s.y), (0x1.000000p0f / 0x1.19999ap1f)), coax_pow_bg_vs(coax_p_saturate(l_s.z), (0x1.000000p0f / 0x1.19999ap1f)));
}
fn coax_p_sty_shadow(p_p: vec3f, p_n: vec3f) -> f32 {
  let l_p: vec3f = p_p;
  let l_n: vec3f = p_n;
  var l_q: vec4f;
  var l_uv: vec2f;
  var l_texel: f32;
  var l_lit: f32;
  var l_i: i32;
  var l_a: f32;
  var l_d: f32;
  l_q = coax_p_sty_clip(coax_p_add3(l_p, coax_p_scale3(l_n, 0x1.1eb852p-4f)), 24i);
  if (((((abs(l_q.x) > 0x1.fd70a4p-1f) || (abs(l_q.y) > 0x1.fd70a4p-1f)) || (l_q.z < 0.0f)) || (l_q.z > 0x1.000000p0f)))
  {
    return 0x1.000000p0f;
  }
  l_uv = coax_p_v2(((l_q.x * 0x1.000000p-1f) + 0x1.000000p-1f), (0x1.000000p-1f - (l_q.y * 0x1.000000p-1f)));
  l_texel = (0x1.000000p0f / f32(textureDimensions(shadow_map).x));
  l_lit = 0.0f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 12i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      l_a = (f32(l_i) * 0x1.3331fcp1f);
      l_d = ((sqrt(((f32(l_i) + 0x1.000000p-1f) / 0x1.800000p3f)) * 0x1.99999ap1f) * l_texel);
      l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((l_uv.x + (cos(l_a) * l_d)), (l_uv.y + (sin(l_a) * l_d))), (l_q.z - 0x1.3a92a4p-10f)));
    }
  }
  return (l_lit / 0x1.800000p3f);
}
fn coax_p_sty_lit(p_albedo: vec3f, p_n: vec3f, p_ndl: f32, p_shadow: f32) -> vec3f {
  let l_albedo: vec3f = p_albedo;
  let l_n: vec3f = p_n;
  let l_ndl: f32 = p_ndl;
  let l_shadow: f32 = p_shadow;
  var l_sun: f32;
  var l_ambient: vec3f;
  l_sun = (coax_p_sty_bands(l_ndl, coax_at_u(i32(47i))) * coax_p_mix(0x1.000000p0f, l_shadow, coax_at_u(i32(43i))));
  l_ambient = coax_p_mix3(coax_p_wg_at3(52i), coax_p_wg_at3(48i), ((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f));
  return coax_p_mul3(l_albedo, coax_p_add3(l_ambient, coax_p_scale3(coax_p_wg_at3(44i), l_sun)));
}
fn coax_p_sty_haze(p_c: vec3f, p_p: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  let l_p: vec3f = p_p;
  var l_d: f32;
  var l_f: f32;
  l_d = coax_p_length3(coax_p_sub3(coax_p_wg_at3(4i), l_p));
  l_f = (0x1.000000p0f - exp((0.0f - (coax_p_fmax((l_d - coax_at_u(i32(60i))), 0.0f) * coax_at_u(i32(59i))))));
  return coax_p_mix3(l_c, coax_p_sty_decode(coax_p_wg_at3(56i)), coax_p_fmin(l_f, coax_at_u(i32(61i))));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_sty_surface(p_s: coax_t15) -> coax_t20 {
  let l_s: coax_t15 = p_s;
  var l_o: coax_t20;
  var l_m: f32;
  var l_a: f32;
  var l_b: f32;
  var l_p: vec3f;
  var l_base: vec3f;
  var l_base_8: vec3f;
  var l_base_9: vec3f;
  var l_r: f32;
  var l_row: f32;
  var l_fr: f32;
  var l_c: f32;
  var l_fc: f32;
  var l_lip: f32;
  var l_under: bool;
  var l_tile: f32;
  var l_tone: f32;
  var l_warm: vec3f;
  var l_moss: f32;
  var l_base_21: vec3f;
  var l_r_22: f32;
  var l_row_23: f32;
  var l_fr_24: f32;
  var l_c_25: f32;
  var l_fc_26: f32;
  var l_joint: f32;
  var l_block: f32;
  var l_stone: vec3f;
  var l_base_30: vec3f;
  var l_form: f32;
  var l_base_32: vec3f;
  var l_c_33: f32;
  var l_fc_34: f32;
  var l_gap: f32;
  var l_base_36: vec3f;
  var l_c_37: f32;
  var l_fc_38: f32;
  var l_gap_39: f32;
  var l_strap: f32;
  var l_light: vec3f;
  var l_da: f32;
  var l_db: f32;
  var l_lead: f32;
  var l_pane: f32;
  var l_base_46: vec3f;
  var l_base_47: vec3f;
  var l_facing: f32;
  var l_patch: f32;
  var l_light_50: f32;
  var l_shade: vec3f;
  var l_c_52: vec3f;
  var l_steep: f32;
  var l_wander: f32;
  var l_bare: f32;
  var l_grass: vec3f;
  var l_stone_57: vec3f;
  var l_rock: vec3f;
  var l_base_59: vec3f;
  var l_base_60: vec3f;
  var l_slat: f32;
  var l_gold: vec3f;
  var l_blue: vec3f;
  var l_da_64: f32;
  var l_db_65: f32;
  var l_lead_66: f32;
  var l_c_67: vec3f;
  var l_head: f32;
  var l_loz: f32;
  var l_field: vec3f;
  var l_gold_71: vec3f;
  var l_c_72: vec3f;
  var l_bx: f32;
  var l_ring: f32;
  var l_on: f32;
  var l_base_76: vec3f;
  var l_r_77: f32;
  var l_row_78: f32;
  var l_fr_79: f32;
  var l_c_80: f32;
  var l_fc_81: f32;
  var l_joint_82: f32;
  var l_base_83: vec3f;
  var l_r_84: f32;
  var l_row_85: f32;
  var l_fr_86: f32;
  var l_c_87: f32;
  var l_fc_88: f32;
  var l_joint_89: f32;
  var l_glass: vec3f;
  var l_gold_91: vec3f;
  var l_r_92: f32;
  var l_k: f32;
  var l_fk: f32;
  var l_c_95: vec3f;
  var l_reach: f32;
  var l_base_97: vec3f;
  var l_t: f32;
  var l_glint: f32;
  var l_base_100: vec3f;
  var l_bed: f32;
  var l_fr_102: f32;
  var l_crease: f32;
  var l_tone_104: f32;
  var l_rock_105: vec3f;
  var l_moss_106: f32;
  l_o = coax_t20();
  l_o.y = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.x = coax_p_v3(0x1.333334p-1f, 0x1.333334p-1f, 0x1.333334p-1f);
  l_m = l_s.f6.x;
  l_a = l_s.w.x;
  l_b = l_s.w.y;
  l_p = l_s.y;
  if (coax_p_sty_is(l_m, 1i))
  {
    l_o.x = coax_p_sty_ground(l_p, l_a, l_b);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 2i))
  {
    l_base = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.dc28f6p-1f, 0x1.c28f5cp-1f, 0x1.7ae148p-1f));
    l_o.x = coax_p_scale3(l_base, (0x1.ccccccp-1f + (0x1.0a3d70p-3f * coax_p_sty_fbm2(((l_a * 0x1.19999ap0f) + (l_p.x * 0x1.333334p-2f)), ((l_b * 0x1.19999ap0f) + (l_p.z * 0x1.333334p-2f))))));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 3i))
  {
    l_base_8 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.147ae2p-2f, 0x1.5c28f6p-3f, 0x1.c28f5cp-4f));
    l_o.x = coax_p_scale3(l_base_8, (0x1.a3d70ap-1f + (0x1.70a3d8p-2f * coax_p_sty_noise2(((l_a * 0x1.000000p1f) + (l_b * 0x1.200000p3f)), ((l_b * 0x1.800000p1f) - (l_a * 0x1.a00000p4f))))));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 4i))
  {
    l_base_9 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.0a3d70p-1f, 0x1.99999ap-3f, 0x1.c28f5cp-4f));
    l_r = (l_b / 0x1.147ae2p-2f);
    l_row = floor(l_r);
    l_fr = (l_r - l_row);
    l_c = ((l_a / 0x1.000000p-2f) + (0x1.000000p-1f * (l_row - (0x1.000000p1f * floor((l_row * 0x1.000000p-1f))))));
    l_fc = (coax_p_fract(l_c) - 0x1.000000p-1f);
    l_lip = ((0x1.266666p0f * l_fc) * l_fc);
    l_under = (l_fr < l_lip);
    if (l_under)
    {
      l_row = (l_row - 0x1.000000p0f);
      l_c = ((l_a / 0x1.000000p-2f) + (0x1.000000p-1f * (l_row - (0x1.000000p1f * floor((l_row * 0x1.000000p-1f))))));
    }
    l_tile = coax_p_sty_hash2(floor(l_c), l_row);
    l_tone = select((0x1.b851ecp-1f + (0x1.47ae14p-3f * coax_p_smoothstep(l_lip, 0x1.000000p0f, l_fr))), 0x1.51eb86p-1f, l_under);
    l_tone = (l_tone * (0x1.ae147ap-1f + (0x1.333334p-2f * l_tile)));
    l_warm = coax_p_mix3(l_base_9, coax_p_mul3(l_base_9, coax_p_v3(0x1.1eb852p0f, 0x1.4cccccp0f, 0x1.266666p0f)), (coax_p_sty_hash2((floor(l_c) + 0x1.f00000p4f), (l_row + 0x1.c00000p2f)) * 0x1.333334p-1f));
    l_moss = (coax_p_smoothstep(0x1.333334p-1f, 0x1.7ae148p-1f, coax_p_sty_fbm2(((l_a * 0x1.ae147ap-2f) + (l_p.x * 0x1.99999ap-5f)), ((l_b * 0x1.000000p-1f) + 0x1.000000p2f))) * 0x1.99999ap-1f);
    l_o.x = coax_p_mix3(coax_p_scale3(l_warm, l_tone), coax_p_v3(0x1.c28f5cp-3f, 0x1.333334p-2f, 0x1.c28f5cp-4f), l_moss);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 5i))
  {
    l_base_21 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.428f5cp-1f, 0x1.428f5cp-1f, 0x1.3851ecp-1f));
    l_r_22 = (l_b / 0x1.333334p-2f);
    l_row_23 = floor(l_r_22);
    l_fr_24 = (l_r_22 - l_row_23);
    l_c_25 = ((l_a / 0x1.1eb852p-1f) + (0x1.000000p-1f * (l_row_23 - (0x1.000000p1f * floor((l_row_23 * 0x1.000000p-1f))))));
    l_fc_26 = coax_p_fract(l_c_25);
    l_joint = (0x1.000000p0f - coax_p_smoothstep(0x1.89374cp-7f, 0x1.eb851ep-6f, coax_p_fmin((coax_p_fmin(l_fr_24, (0x1.000000p0f - l_fr_24)) * 0x1.333334p-2f), (coax_p_fmin(l_fc_26, (0x1.000000p0f - l_fc_26)) * 0x1.1eb852p-1f))));
    l_block = coax_p_sty_hash2(floor(l_c_25), l_row_23);
    l_stone = coax_p_scale3(l_base_21, ((0x1.b851ecp-1f + (0x1.99999ap-3f * l_block)) * (0x1.dc28f6p-1f + (0x1.1eb852p-4f * coax_p_smoothstep(0.0f, 0x1.000000p-1f, l_fr_24)))));
    l_o.x = coax_p_mix3(l_stone, coax_p_scale3(l_base_21, 0x1.333334p0f), (l_joint * 0x1.b33334p-1f));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 6i))
  {
    l_base_30 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.333334p-1f, 0x1.2e147ap-1f, 0x1.147ae2p-1f));
    l_form = (0x1.99999ap-1f + (0x1.eb851ep-3f * coax_p_smoothstep((0.0f - 0x1.333334p-2f), 0x1.ccccccp-1f, (coax_p_normalize3(l_s.z)).y)));
    l_o.x = coax_p_scale3(l_base_30, (l_form * (0x1.e147aep-1f + (0x1.47ae14p-4f * coax_p_sty_noise2(((l_p.x * 0x1.19999ap1f) + (l_p.y * 0x1.800000p0f)), ((l_p.z * 0x1.19999ap1f) - (l_p.y * 0x1.800000p0f)))))));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 7i))
  {
    l_base_32 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.8f5c28p-1f, 0x1.1eb852p-1f, 0x1.333334p-2f));
    l_c_33 = (l_a / 0x1.c28f5cp-3f);
    l_fc_34 = coax_p_fract(l_c_33);
    l_gap = coax_p_smoothstep(0.0f, 0x1.1eb852p-4f, coax_p_fmin(l_fc_34, (0x1.000000p0f - l_fc_34)));
    l_o.x = coax_p_scale3(l_base_32, (((0x1.ae147ap-1f + (0x1.eb851ep-3f * coax_p_sty_hash2(floor(l_c_33), 0x1.800000p1f))) * (0x1.19999ap-1f + (0x1.ccccccp-2f * l_gap))) * (0x1.e147aep-1f + (0x1.99999ap-4f * coax_p_sty_noise2((l_a * 0x1.e00000p4f), (l_b * 0x1.000000p1f))))));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 8i))
  {
    l_base_36 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.851eb8p-2f, 0x1.d70a3ep-3f, 0x1.0a3d70p-3f));
    l_c_37 = (l_a / 0x1.99999ap-3f);
    l_fc_38 = coax_p_fract(l_c_37);
    l_gap_39 = coax_p_smoothstep(0.0f, 0x1.47ae14p-4f, coax_p_fmin(l_fc_38, (0x1.000000p0f - l_fc_38)));
    l_strap = coax_p_fmax((0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-5f, 0x1.eb851ep-5f, abs((l_b - 0x1.ccccccp-2f)))), (0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-5f, 0x1.eb851ep-5f, abs((l_b - 0x1.8cccccp0f)))));
    l_o.x = coax_p_mix3(coax_p_scale3(l_base_36, ((0x1.b33334p-1f + (0x1.000000p-2f * coax_p_sty_hash2(floor(l_c_37), 0x1.200000p3f))) * (0x1.000000p-1f + (0x1.000000p-1f * l_gap_39)))), coax_p_v3(0x1.70a3d8p-4f, 0x1.47ae14p-4f, 0x1.47ae14p-4f), l_strap);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 9i))
  {
    l_light = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.000000p0f, 0x1.70a3d8p-1f, 0x1.333334p-2f));
    l_da = coax_p_fract(((l_a + l_b) / 0x1.5c28f6p-3f));
    l_db = coax_p_fract(((l_a - l_b) / 0x1.5c28f6p-3f));
    l_lead = (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-5f, 0x1.99999ap-4f, coax_p_fmin(coax_p_fmin(l_da, (0x1.000000p0f - l_da)), coax_p_fmin(l_db, (0x1.000000p0f - l_db)))));
    l_pane = (0x1.a3d70ap-1f + (0x1.333334p-2f * coax_p_sty_hash2(floor(((l_a + l_b) / 0x1.5c28f6p-3f)), floor(((l_a - l_b) / 0x1.5c28f6p-3f)))));
    l_o.x = coax_p_v3(0x1.99999ap-5f, 0x1.47ae14p-5f, 0x1.eb851ep-6f);
    l_o.y = coax_p_mix3(coax_p_scale3(l_light, (l_pane * 0x1.0cccccp0f)), coax_p_v3(0x1.eb851ep-4f, 0x1.47ae14p-4f, 0x1.99999ap-5f), l_lead);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 10i))
  {
    l_base_46 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.000000p-2f, 0x1.5c28f6p-3f, 0x1.eb851ep-4f));
    l_o.x = coax_p_scale3(l_base_46, (0x1.8f5c28p-1f + (0x1.99999ap-2f * coax_p_sty_noise2((l_a * 0x1.c00000p3f), (l_p.y * 0x1.4cccccp0f)))));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 11i))
  {
    l_base_47 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.0a3d70p-2f, 0x1.000000p-1f, 0x1.5c28f6p-3f));
    l_facing = ((dot(coax_p_normalize3(l_s.z), coax_p_wg_at3(40i)) * 0x1.000000p-1f) + 0x1.000000p-1f);
    l_patch = (coax_p_sty_noise2(((l_p.x * 0x1.ae147ap-2f) + (l_p.y * 0x1.333334p-2f)), ((l_p.z * 0x1.ae147ap-2f) - (l_p.y * 0x1.333334p-2f))) - 0x1.000000p-1f);
    l_light_50 = (((l_facing * 0x1.666666p-1f) + (l_a * 0x1.0a3d70p-2f)) + (l_patch * 0x1.0a3d70p-3f));
    l_shade = coax_p_mul3(l_base_47, coax_p_v3(0x1.28f5c2p-1f, 0x1.5c28f6p-1f, 0x1.51eb86p-1f));
    if ((l_b < (0.0f - 0x1.000000p-1f)))
    {
      l_o.x = coax_p_mix3(l_shade, coax_p_scale3(l_base_47, 0x1.ccccccp-1f), coax_p_smoothstep(0x1.d70a3ep-2f, 0x1.0a3d70p-1f, l_light_50));
      return l_o;
    }
    l_light_50 = (l_light_50 + ((l_b - 0x1.000000p-1f) * 0x1.99999ap-5f));
    l_c_52 = coax_p_mix3(l_shade, l_base_47, coax_p_smoothstep(0x1.851eb8p-2f, 0x1.b851ecp-2f, l_light_50));
    l_c_52 = coax_p_mix3(l_c_52, coax_p_mul3(l_base_47, coax_p_v3(0x1.333334p0f, 0x1.266666p0f, 0x1.b851ecp-1f)), coax_p_smoothstep(0x1.333334p-1f, 0x1.4cccccp-1f, l_light_50));
    l_c_52 = coax_p_mix3(l_c_52, coax_p_mul3(l_base_47, coax_p_v3(0x1.75c290p0f, 0x1.5c28f6p0f, 0x1.ccccccp-1f)), coax_p_smoothstep(0x1.b33334p-1f, 0x1.ccccccp-1f, l_light_50));
    l_o.x = l_c_52;
    return l_o;
  }
  if (coax_p_sty_is(l_m, 12i))
  {
    l_o.x = coax_p_v3(0x1.333334p-4f, 0x1.333334p-4f, 0x1.5c28f6p-4f);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 13i))
  {
    l_o.x = coax_p_v3(0x1.99999ap-4f, 0x1.47ae14p-4f, 0x1.99999ap-5f);
    l_o.y = coax_p_scale3(coax_p_sty_base(l_s.f4, coax_p_v3(0x1.000000p0f, 0x1.99999ap-1f, 0x1.ae147ap-2f)), 0x1.400000p0f);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 14i))
  {
    l_o.x = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.800000p-1f, 0x1.99999ap-3f, 0x1.47ae14p-3f));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 15i))
  {
    l_steep = (0x1.000000p0f - (coax_p_normalize3(l_s.z)).y);
    l_wander = (coax_p_sty_noise2((l_p.x * 0x1.47ae14p-6f), (l_p.z * 0x1.47ae14p-6f)) - 0x1.000000p-1f);
    l_bare = coax_p_smoothstep(0x1.eb851ep-3f, 0x1.99999ap-2f, (l_steep + (l_wander * 0x1.70a3d8p-3f)));
    l_grass = coax_p_mix3(coax_p_v3(0x1.99999ap-2f, 0x1.28f5c2p-1f, 0x1.1eb852p-2f), coax_p_v3(0x1.28f5c2p-1f, 0x1.51eb86p-1f, 0x1.5c28f6p-2f), coax_p_sty_noise2(((l_p.x * 0x1.26e978p-7f) + 0x1.c00000p2f), (l_p.z * 0x1.26e978p-7f)));
    l_stone_57 = coax_p_scale3(coax_p_v3(0x1.333334p-1f, 0x1.333334p-1f, 0x1.3851ecp-1f), (0x1.d70a3ep-1f + (0x1.eb851ep-4f * coax_p_sty_noise2(((l_p.y * 0x1.c28f5cp-4f) + (l_wander * 0x1.800000p1f)), 0x1.000000p1f))));
    l_rock = coax_p_mix3(l_grass, l_stone_57, l_bare);
    if ((((l_s.f4.x + l_s.f4.y) + l_s.f4.z) > 0x1.0624dep-10f))
    {
      l_rock = coax_p_mul3(l_rock, l_s.f4);
    }
    l_o.x = l_rock;
    return l_o;
  }
  if (coax_p_sty_is(l_m, 16i))
  {
    l_base_59 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.99999ap-1f, 0x1.947ae2p-1f, 0x1.800000p-1f));
    l_o.x = coax_p_scale3(l_base_59, (0x1.dc28f6p-1f + (0x1.99999ap-4f * coax_p_sty_noise2(((l_p.x * 0x1.b33334p0f) + l_p.y), ((l_p.z * 0x1.b33334p0f) - l_p.y)))));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 17i))
  {
    l_base_60 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.d70a3ep-2f, 0x1.333334p-2f, 0x1.5c28f6p-3f));
    l_slat = coax_p_fract((l_b / 0x1.5c28f6p-4f));
    l_o.x = coax_p_scale3(l_base_60, (0x1.19999ap-1f + (0x1.000000p-1f * coax_p_smoothstep(0.0f, 0x1.000000p-1f, l_slat))));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 18i))
  {
    l_gold = coax_p_v3(0x1.e66666p-1f, 0x1.7ae148p-1f, 0x1.c28f5cp-3f);
    l_blue = coax_p_v3(0x1.1eb852p-3f, 0x1.99999ap-3f, 0x1.8f5c28p-1f);
    l_da_64 = coax_p_fract(((l_a + l_b) * 0x1.800000p1f));
    l_db_65 = coax_p_fract(((l_a - l_b) * 0x1.800000p1f));
    l_lead_66 = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.1eb852p-4f, coax_p_fmin(coax_p_fmin(l_da_64, (0x1.000000p0f - l_da_64)), coax_p_fmin(l_db_65, (0x1.000000p0f - l_db_65)))));
    l_c_67 = coax_p_scale3(l_blue, (0x1.99999ap-1f + (0x1.666666p-2f * coax_p_sty_hash2(floor(((l_a + l_b) * 0x1.800000p1f)), floor(((l_a - l_b) * 0x1.800000p1f))))));
    l_c_67 = coax_p_mix3(l_c_67, coax_p_v3(0x1.99999ap-5f, 0x1.eb851ep-5f, 0x1.99999ap-3f), (l_lead_66 * 0x1.666666p-1f));
    if ((l_s.f4.x > 0x1.000000p-1f))
    {
      l_head = sqrt((((l_a - 0x1.000000p-1f) * (l_a - 0x1.000000p-1f)) + ((l_b + 0x1.e66666p-1f) * (l_b + 0x1.e66666p-1f))));
      if ((((l_b < (0.0f - 0x1.1eb852p0f)) && (l_b > (0.0f - 0x1.800000p1f))) && (abs((l_a - 0x1.000000p-1f)) < (0x1.0a3d70p-3f + (((0.0f - l_b) - 0x1.1eb852p0f) * 0x1.70a3d8p-4f)))))
      {
        l_c_67 = select(coax_p_v3(0x1.dc28f6p-1f, 0x1.51eb86p-1f, 0x1.47ae14p-3f), coax_p_v3(0x1.a3d70ap-1f, 0x1.1eb852p-3f, 0x1.99999ap-4f), (coax_p_fract(((l_a * 0x1.000000p0f) + 0x1.000000p-2f)) < 0x1.000000p-1f));
      }
      if ((l_head < 0x1.1eb852p-3f))
      {
        l_c_67 = coax_p_v3(0x1.eb851ep-1f, 0x1.a3d70ap-1f, 0x1.333334p-1f);
      }
      else
      {
        if ((l_head < 0x1.99999ap-3f))
        {
          l_c_67 = l_gold;
        }
      }
    }
    else
    {
      if ((coax_p_sty_bell(((l_a - 0x1.000000p-1f) * 0x1.800000p-1f), ((l_b + 0x1.000000p0f) * 0x1.800000p-1f)) > 0x1.000000p-1f))
      {
        l_c_67 = l_gold;
      }
      l_loz = ((abs((l_a - 0x1.000000p-1f)) / 0x1.c28f5cp-3f) + (abs((l_b + 0x1.0cccccp1f)) / 0x1.70a3d8p-2f));
      if ((l_loz < 0x1.000000p0f))
      {
        l_c_67 = select(l_gold, coax_p_v3(0x1.b851ecp-1f, 0x1.99999ap-3f, 0x1.99999ap-4f), (l_loz < 0x1.70a3d8p-1f));
      }
    }
    if ((abs((l_a - 0x1.000000p-1f)) > 0x1.7ae148p-2f))
    {
      l_c_67 = l_gold;
    }
    l_o.x = coax_p_scale3(l_c_67, 0x1.666666p-2f);
    l_o.y = coax_p_scale3(l_c_67, 0x1.3d70a4p-1f);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 19i))
  {
    l_field = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.eb851ep-4f, 0x1.47ae14p-3f, 0x1.333334p-1f));
    l_gold_71 = coax_p_v3(0x1.e66666p-1f, 0x1.7ae148p-1f, 0x1.99999ap-3f);
    l_c_72 = coax_p_scale3(l_field, (0x1.ccccccp-1f + (0x1.1eb852p-3f * sin(((l_a * 0x1.200000p3f) + (l_b * 0x1.b33334p0f))))));
    l_bx = abs((l_a - 0x1.000000p-1f));
    l_ring = sqrt((((l_a - 0x1.000000p-1f) * (l_a - 0x1.000000p-1f)) + ((l_b + 0x1.7ae148p-1f) * (l_b + 0x1.7ae148p-1f))));
    l_on = ((((coax_p_sty_bell((l_a - 0x1.000000p-1f), (l_b + 0x1.7ae148p-1f)) + coax_p_sty_star((l_a - 0x1.000000p-1f), (l_b + 0x1.570a3ep0f), 0x1.1eb852p-4f)) + coax_p_sty_star((l_a - 0x1.147ae2p-2f), (l_b + 0x1.99999ap0f), 0x1.99999ap-5f)) + coax_p_sty_star((l_a - 0x1.75c290p-1f), (l_b + 0x1.99999ap0f), 0x1.99999ap-5f)) + coax_p_sty_star((l_a - 0x1.000000p-1f), (l_b + 0x1.e147aep0f), 0x1.eb851ep-5f));
    if (((((l_on > 0x1.000000p-1f) || (abs((l_ring - 0x1.147ae2p-2f)) < 0x1.6872b0p-6f)) || ((l_bx > 0x1.8f5c28p-2f) && (l_bx < 0x1.c28f5cp-2f))) || ((l_b > (0.0f - 0x1.99999ap-4f)) && (l_b < (0.0f - 0x1.99999ap-5f)))))
    {
      l_c_72 = l_gold_71;
    }
    l_o.x = l_c_72;
    return l_o;
  }
  if (coax_p_sty_is(l_m, 20i))
  {
    l_base_76 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.947ae2p-1f, 0x1.8f5c28p-1f, 0x1.75c290p-1f));
    l_r_77 = (l_b / 0x1.99999ap-1f);
    l_row_78 = floor(l_r_77);
    l_fr_79 = (l_r_77 - l_row_78);
    l_c_80 = ((l_a / 0x1.b33334p0f) + (0x1.000000p-1f * (l_row_78 - (0x1.000000p1f * floor((l_row_78 * 0x1.000000p-1f))))));
    l_fc_81 = coax_p_fract(l_c_80);
    l_joint_82 = (0x1.000000p0f - coax_p_smoothstep(0x1.0624dep-7f, 0x1.47ae14p-6f, coax_p_fmin((coax_p_fmin(l_fr_79, (0x1.000000p0f - l_fr_79)) * 0x1.99999ap-1f), (coax_p_fmin(l_fc_81, (0x1.000000p0f - l_fc_81)) * 0x1.b33334p0f))));
    l_o.x = coax_p_mix3(coax_p_scale3(l_base_76, (0x1.dc28f6p-1f + (0x1.99999ap-4f * coax_p_sty_hash2(floor(l_c_80), l_row_78)))), coax_p_scale3(l_base_76, 0x1.3d70a4p-1f), l_joint_82);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 21i))
  {
    l_base_83 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.333334p-1f, 0x1.2e147ap-1f, 0x1.1eb852p-1f));
    l_r_84 = (l_b / 0x1.eb851ep-3f);
    l_row_85 = floor(l_r_84);
    l_fr_86 = (l_r_84 - l_row_85);
    l_c_87 = ((l_a / 0x1.70a3d8p-2f) + (0x1.000000p-1f * (l_row_85 - (0x1.000000p1f * floor((l_row_85 * 0x1.000000p-1f))))));
    l_fc_88 = coax_p_fract(l_c_87);
    l_joint_89 = (0x1.000000p0f - coax_p_smoothstep(0x1.89374cp-7f, 0x1.eb851ep-6f, coax_p_fmin((coax_p_fmin(l_fr_86, (0x1.000000p0f - l_fr_86)) * 0x1.eb851ep-3f), (coax_p_fmin(l_fc_88, (0x1.000000p0f - l_fc_88)) * 0x1.70a3d8p-2f))));
    l_o.x = coax_p_mix3(coax_p_scale3(l_base_83, (0x1.b851ecp-1f + (0x1.c28f5cp-3f * coax_p_sty_hash2(floor(l_c_87), l_row_85)))), coax_p_scale3(l_base_83, 0x1.47ae14p-1f), l_joint_89);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 22i))
  {
    l_glass = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.000000p-1f, 0x1.851eb8p-2f, 0x1.e66666p-1f));
    l_gold_91 = coax_p_v3(0x1.dc28f6p-1f, 0x1.7ae148p-1f, 0x1.333334p-2f);
    l_r_92 = sqrt(((l_a * l_a) + (l_b * l_b)));
    l_k = (((coax_p_wg_atan2(l_b, l_a) / 0x1.921fb6p2f) + 0x1.000000p-1f) * 0x1.000000p3f);
    l_fk = (coax_p_fract(l_k) - 0x1.000000p-1f);
    l_c_95 = coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-4f, 0x1.333334p-2f);
    l_reach = ((l_r_92 - 0x1.28f5c2p-1f) / 0x1.333334p-2f);
    if ((((l_r_92 > 0x1.0a3d70p-2f) && (l_r_92 < 0x1.ccccccp-1f)) && (abs(l_fk) < (0x1.99999ap-2f * sqrt(coax_p_fmax((0x1.000000p0f - (l_reach * l_reach)), 0.0f))))))
    {
      l_c_95 = coax_p_scale3(l_glass, (0x1.800000p-1f + (0x1.99999ap-2f * coax_p_sty_hash2(floor(l_k), 0x1.000000p1f))));
      if (((abs(l_fk) < 0x1.0a3d70p-3f) && (abs((l_r_92 - 0x1.333334p-1f)) < 0x1.70a3d8p-4f)))
      {
        l_c_95 = coax_p_v3(0x1.eb851ep-1f, 0x1.1eb852p-1f, 0x1.47ae14p-3f);
      }
    }
    if (((l_r_92 < 0x1.99999ap-3f) || (l_r_92 > 0x1.d70a3ep-1f)))
    {
      l_c_95 = l_gold_91;
    }
    else
    {
      if ((l_r_92 < 0x1.0a3d70p-2f))
      {
        l_c_95 = coax_p_scale3(l_glass, 0x1.333334p-1f);
      }
    }
    l_o.x = coax_p_scale3(l_c_95, 0x1.666666p-2f);
    l_o.y = coax_p_scale3(l_c_95, 0x1.3d70a4p-1f);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 23i))
  {
    l_base_97 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.333334p-1f, 0x1.8f5c28p-1f, 0x1.c28f5cp-1f));
    l_t = coax_at_u(i32(2i));
    l_glint = coax_p_smoothstep(0x1.3d70a4p-1f, 0x1.7ae148p-1f, coax_p_sty_noise2(((l_p.x * 0x1.8cccccp1f) + (l_t * 0x1.000000p-1f)), (((l_p.z * 0x1.8cccccp1f) + (l_p.y * 0x1.400000p2f)) + (l_t * 0x1.99999ap0f))));
    l_o.x = coax_p_mix3(l_base_97, coax_p_v3(0x1.eb851ep-1f, 0x1.f5c290p-1f, 0x1.000000p0f), (l_glint * 0x1.666666p-1f));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 24i))
  {
    l_base_100 = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.99999ap-2f, 0x1.c28f5cp-2f, 0x1.000000p-1f));
    l_bed = ((l_p.y / 0x1.333334p1f) + ((coax_p_sty_noise2((l_p.x * 0x1.70a3d8p-4f), (l_p.z * 0x1.70a3d8p-4f)) - 0x1.000000p-1f) * 0x1.19999ap0f));
    l_fr_102 = coax_p_fract(l_bed);
    l_crease = (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.1eb852p-4f, coax_p_fmin(l_fr_102, (0x1.000000p0f - l_fr_102))));
    l_tone_104 = ((0x1.c28f5cp-1f + (0x1.99999ap-3f * coax_p_sty_hash2(floor(l_bed), 0x1.800000p1f))) + (0x1.99999ap-3f * (coax_p_sty_fbm2(((l_a * 0x1.c28f5cp-3f) + (l_p.x * 0x1.1eb852p-4f)), ((l_p.y * 0x1.333334p-2f) + (l_p.z * 0x1.1eb852p-4f))) - 0x1.000000p-1f)));
    l_rock_105 = coax_p_mix3(coax_p_scale3(l_base_100, l_tone_104), coax_p_scale3(l_base_100, 0x1.51eb86p-1f), (l_crease * 0x1.333334p-1f));
    l_moss_106 = coax_p_smoothstep(0x1.3d70a4p-1f, 0x1.a3d70ap-1f, ((coax_p_normalize3(l_s.z)).y + ((coax_p_sty_noise2((l_p.x * 0x1.666666p-1f), (l_p.z * 0x1.666666p-1f)) - 0x1.000000p-1f) * 0x1.333334p-2f)));
    l_o.x = coax_p_mix3(l_rock_105, coax_p_v3(0x1.28f5c2p-2f, 0x1.ae147ap-2f, 0x1.99999ap-3f), l_moss_106);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 25i))
  {
    l_o.x = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.000000p-1f, 0x1.ccccccp-2f, 0x1.ccccccp-1f));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 26i))
  {
    l_o.x = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-2f, 0x1.a3d70ap-1f));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 27i))
  {
    l_o.x = coax_p_sty_base(l_s.f4, coax_p_v3(0x1.99999ap-1f, 0x1.ccccccp-1f, 0x1.f5c290p-1f));
    return l_o;
  }
  if (coax_p_sty_is(l_m, 28i))
  {
    l_o.x = coax_p_v3(0x1.e66666p-1f, 0x1.f0a3d8p-1f, 0x1.000000p0f);
    return l_o;
  }
  if (coax_p_sty_is(l_m, 29i))
  {
    l_o.x = coax_p_v3(0x1.ccccccp-1f, 0x1.ccccccp-1f, 0x1.99999ap-1f);
    return l_o;
  }
  return l_o;
}
fn coax_p_mul3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_sty_wet(p_s: coax_t15) -> vec4f {
  let l_s: coax_t15 = p_s;
  var l_m: f32;
  var l_p: vec3f;
  var l_t: f32;
  var l_toward: vec3f;
  var l_k: f32;
  var l_c: vec3f;
  var l_o: coax_t20;
  l_m = l_s.f6.x;
  l_p = l_s.y;
  l_t = coax_at_u(i32(2i));
  if (coax_p_sty_is(l_m, 26i))
  {
    return coax_p_sty_river(coax_p_sty_base(l_s.f4, coax_p_v3(0x1.47ae14p-4f, 0x1.70a3d8p-2f, 0x1.99999ap-1f)), l_s.w.x, l_s.w.y, l_p.x, l_p.z, (l_p.y - coax_p_sty_table(l_p.x, l_p.z, 0i)), coax_p_sty_table(l_p.x, l_p.z, 3i), l_t);
  }
  if (coax_p_sty_is(l_m, 27i))
  {
    return coax_p_sty_falls(coax_p_sty_base(l_s.f4, coax_p_v3(0x1.99999ap-2f, 0x1.47ae14p-1f, 0x1.d70a3ep-1f)), l_s.w.x, l_s.w.y, l_t);
  }
  if (coax_p_sty_is(l_m, 28i))
  {
    l_toward = coax_p_normalize3(coax_p_sub3(coax_p_wg_at3(4i), l_p));
    l_k = coax_p_saturate(abs(dot(coax_p_normalize3(l_s.z), l_toward)));
    return coax_p_v4(0x1.eb851ep-1f, 0x1.f5c290p-1f, 0x1.000000p0f, (((l_k * l_k) * l_k) * (0x1.99999ap-4f + (0x1.70a3d8p-4f * coax_p_sty_noise2(((l_p.x * 0x1.333334p-1f) + (l_t * 0x1.99999ap-2f)), ((l_p.y * 0x1.333334p-1f) - (l_t * 0x1.666666p-1f)))))));
  }
  if (coax_p_sty_is(l_m, 29i))
  {
    l_c = coax_p_sty_spectrum(l_s.w.x);
    return coax_p_v4(l_c.x, l_c.y, l_c.z, ((coax_pow_bg_vs(sin((coax_p_saturate(l_s.w.x) * 0x1.921fa0p1f)), 0x1.666666p-1f) * sin((coax_p_saturate(l_s.w.y) * 0x1.921fa0p1f))) * 0x1.333334p-1f));
  }
  l_o = coax_p_sty_surface(l_s);
  return coax_p_v4(l_o.x.x, l_o.x.y, l_o.x.z, 0x1.000000p0f);
}
fn coax_p_length2(p_a: vec2f) -> f32 {
  let l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  let l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_sty_cell(p_x: f32) -> u32 {
  let l_x: f32 = p_x;
  return u32(i32(floor(coax_p_fmin(coax_p_fmax(l_x, (0.0f - 0x1.000000p25f)), 0x1.fffffep24f))));
}
fn coax_p_render_noise_mix2(p_x: u32, p_y: u32) -> u32 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  return coax_p_render_noise_permute(((l_x * 1597334677u) ^ (l_y * 3812015801u)));
}
fn coax_p_render_noise_unit(p_word: u32) -> f32 {
  let l_word: u32 = p_word;
  return (f32((l_word >> u32(8u))) * 0x1.000000p-24f);
}
fn coax_p_wg_clamp_i(p_x: i32, p_low: i32, p_high: i32) -> i32 {
  let l_x: i32 = p_x;
  let l_low: i32 = p_low;
  let l_high: i32 = p_high;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_low), l_high);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_sty_meadow(p_x: f32, p_z: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  var l_sx: f32;
  var l_patch: f32;
  var l_fine: f32;
  l_sx = (l_x + coax_at_u(i32(99i)));
  l_patch = coax_p_sty_noise2(((l_sx * 0x1.eb851ep-5f) + 0x1.800000p1f), (l_z * 0x1.eb851ep-5f));
  l_fine = coax_p_sty_noise2((l_sx * 0x1.333334p-1f), ((l_z * 0x1.333334p-1f) + 0x1.000000p3f));
  return coax_p_saturate(((((l_patch * 0x1.99999ap-1f) + (l_fine * 0x1.333334p-2f)) + coax_at_u(i32(98i))) - 0x1.19999ap-1f));
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_wg_atan2(p_y: f32, p_x: f32) -> f32 {
  let l_y: f32 = p_y;
  let l_x: f32 = p_x;
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
fn coax_p_sty_cloud_field(p_ax: f32, p_ay: f32, p_cover: f32) -> f32 {
  let l_ax: f32 = p_ax;
  let l_ay: f32 = p_ay;
  let l_cover: f32 = p_cover;
  var l_mass: f32;
  var l_billow: f32;
  var l_puff: f32;
  l_mass = coax_p_sty_noise2(((l_ax * 0x1.3d70a4p-1f) + 0x1.000000p2f), ((l_ay * 0x1.3d70a4p-1f) + 0x1.800000p0f));
  l_billow = coax_p_sty_fbm2((l_ax * 0x1.0cccccp1f), (l_ay * 0x1.0cccccp1f));
  l_puff = coax_p_sty_noise2(((l_ax * 0x1.c00000p2f) + 0x1.600000p3f), ((l_ay * 0x1.c00000p2f) + 0x1.800000p1f));
  return ((((l_mass * 0x1.3d70a4p-1f) + (l_billow * 0x1.5c28f6p-2f)) + ((l_puff - 0x1.000000p-1f) * 0x1.1eb852p-4f)) + ((l_cover - 0x1.000000p-1f) * 0x1.70a3d8p-2f));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_low: f32 = p_low;
  let l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_sty_cumulus(p_px: f32, p_py: f32, p_wide: f32, p_seed: f32) -> f32 {
  let l_px: f32 = p_px;
  let l_py: f32 = p_py;
  let l_wide: f32 = p_wide;
  let l_seed: f32 = p_seed;
  var l_d: f32;
  var l_k: i32;
  var l_fk: f32;
  var l_h1: f32;
  var l_h2: f32;
  var l_cx: f32;
  var l_cy: f32;
  var l_r: f32;
  var l_edge: f32;
  var l_dx: f32;
  var l_dy: f32;
  l_d = (0.0f - 0x1.400000p3f);
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 9i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_fk = f32(l_k);
      l_h1 = coax_p_sty_hash2((l_fk + (l_seed * 0x1.c00000p2f)), (l_seed + 0x1.800000p1f));
      l_h2 = coax_p_sty_hash2((l_fk + 0x1.600000p3f), ((l_seed * 0x1.400000p2f) + 0x1.000000p0f));
      l_cx = 0.0f;
      l_cy = 0.0f;
      l_r = 0.0f;
      if ((l_k < 6i))
      {
        l_cx = (l_wide * (((((l_fk + 0x1.000000p-1f) / 0x1.800000p2f) * 0x1.b33334p0f) - 0x1.b33334p-1f) + ((l_h1 - 0x1.000000p-1f) * 0x1.eb851ep-4f)));
        l_edge = (l_cx / l_wide);
        l_r = (0x1.eb851ep-3f + ((0x1.333334p-2f * (0x1.000000p0f - (l_edge * l_edge))) * (0x1.800000p-1f + (0x1.000000p-1f * l_h2))));
        l_cy = (l_r * 0x1.ae147ap-2f);
      }
      else
      {
        l_cx = (l_wide * (((l_fk - 0x1.c00000p2f) * 0x1.70a3d8p-2f) + ((l_h1 - 0x1.000000p-1f) * 0x1.99999ap-3f)));
        l_r = (0x1.51eb86p-2f + (0x1.333334p-3f * l_h2));
        l_cy = (0x1.000000p-1f + (0x1.1eb852p-3f * l_h1));
      }
      l_dx = (l_px - l_cx);
      l_dy = (l_py - l_cy);
      l_d = coax_p_fmax(l_d, (l_r - sqrt(((l_dx * l_dx) + (l_dy * l_dy)))));
    }
  }
  l_d = ((l_d + ((coax_p_sty_noise2(((l_px * 0x1.400000p2f) + (l_seed * 0x1.a00000p3f)), (l_py * 0x1.400000p2f)) - 0x1.000000p-1f) * 0x1.70a3d8p-4f)) + ((coax_p_sty_noise2((l_px * 0x1.a00000p3f), ((l_py * 0x1.a00000p3f) + l_seed)) - 0x1.000000p-1f) * 0x1.1eb852p-5f));
  return coax_p_fmin(l_d, ((l_py * 0x1.99999ap0f) + 0x1.47ae14p-6f));
}
fn coax_p_sty_bands(p_ndl: f32, p_soft: f32) -> f32 {
  let l_ndl: f32 = p_ndl;
  let l_soft: f32 = p_soft;
  var l_s: f32;
  l_s = coax_p_fmax(l_soft, 0x1.47ae14p-7f);
  return ((coax_p_smoothstep((0.0f - (l_s * 0x1.333334p-2f)), (l_s * 0x1.666666p-1f), l_ndl) * 0x1.666666p-1f) + (coax_p_smoothstep(0x1.ae147ap-2f, (0x1.ae147ap-2f + l_s), l_ndl) * 0x1.333334p-2f));
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_sty_ground(p_p: vec3f, p_cover: f32, p_paved: f32) -> vec3f {
  let l_p: vec3f = p_p;
  let l_cover: f32 = p_cover;
  let l_paved: f32 = p_paved;
  var l_x: f32;
  var l_z: f32;
  var l_soil: f32;
  var l_grit: f32;
  var l_dirt: vec3f;
  var l_grass: vec3f;
  var l_path: f32;
  var l_c: vec3f;
  var l_f: vec3f;
  var l_joint: f32;
  var l_stone: vec3f;
  var l_share: f32;
  l_x = l_p.x;
  l_z = l_p.z;
  l_soil = coax_p_sty_fbm2((l_x * 0x1.51eb86p-2f), (l_z * 0x1.51eb86p-2f));
  l_grit = coax_p_sty_noise2((l_x * 0x1.8cccccp1f), (l_z * 0x1.8cccccp1f));
  l_dirt = coax_p_scale3(coax_p_mix3(coax_p_v3(0x1.23d70ap-1f, 0x1.c28f5cp-2f, 0x1.28f5c2p-2f), coax_p_v3(0x1.570a3ep-1f, 0x1.0a3d70p-1f, 0x1.666666p-2f), l_soil), (0x1.eb851ep-1f + (0x1.1eb852p-4f * l_grit)));
  l_grass = coax_p_mix3(coax_p_v3(0x1.28f5c2p-2f, 0x1.99999ap-2f, 0x1.c28f5cp-3f), coax_p_v3(0x1.ae147ap-2f, 0x1.eb851ep-2f, 0x1.147ae2p-2f), coax_p_sty_meadow(l_x, l_z));
  l_path = coax_p_sty_path_share(l_x, l_z);
  l_c = coax_p_mix3(l_dirt, l_grass, (coax_p_saturate(l_cover) * (0x1.000000p0f - l_path)));
  if ((l_paved > 0x1.47ae14p-7f))
  {
    l_f = coax_p_sty_flags((l_x * 0x1.333334p0f), (l_z * 0x1.333334p0f));
    l_joint = coax_p_smoothstep(0x1.47ae14p-6f, 0x1.c28f5cp-5f, (l_f.y - l_f.x));
    l_stone = coax_p_scale3(coax_p_mix3(coax_p_v3(0x1.19999ap-1f, 0x1.0f5c28p-1f, 0x1.f5c290p-2f), coax_p_v3(0x1.570a3ep-1f, 0x1.47ae14p-1f, 0x1.28f5c2p-1f), l_f.z), (0x1.eb851ep-1f + (0x1.eb851ep-5f * l_grit)));
    l_share = coax_p_smoothstep(0x1.851eb8p-2f, 0x1.3d70a4p-1f, (l_paved + ((coax_p_sty_noise2((l_x * 0x1.666666p-1f), (l_z * 0x1.666666p-1f)) - 0x1.000000p-1f) * 0x1.333334p-2f)));
    l_c = coax_p_mix3(l_c, coax_p_mix3(coax_p_v3(0x1.99999ap-2f, 0x1.8f5c28p-2f, 0x1.7ae148p-2f), l_stone, l_joint), l_share);
  }
  return l_c;
}
fn coax_p_sty_base(p_tint: vec3f, p_own: vec3f) -> vec3f {
  let l_tint: vec3f = p_tint;
  let l_own: vec3f = p_own;
  if ((((l_tint.x + l_tint.y) + l_tint.z) > 0x1.0624dep-10f))
  {
    return l_tint;
  }
  return l_own;
}
fn coax_p_sty_fbm2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  var l_v: f32;
  var l_a: f32;
  var l_px: f32;
  var l_py: f32;
  var l_k: i32;
  l_v = 0.0f;
  l_a = 0x1.000000p-1f;
  l_px = l_x;
  l_py = l_y;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 4i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_v = (l_v + (l_a * coax_p_sty_noise2(l_px, l_py)));
      l_px = ((l_px * 0x1.03d70ap1f) + 0x1.b33334p0f);
      l_py = ((l_py * 0x1.03d70ap1f) + 0x1.266666p3f);
      l_a = (l_a * 0x1.000000p-1f);
    }
  }
  return (l_v / 0x1.e00000p-1f);
}
fn coax_p_fract(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_sty_bell(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  var l_ax: f32;
  var l_dome: bool;
  var l_body: bool;
  var l_lip: bool;
  var l_clapper: bool;
  var l_knob: bool;
  l_ax = abs(l_x);
  l_dome = ((l_y >= 0x1.47ae14p-6f) && (sqrt(((l_x * l_x) + ((l_y - 0x1.47ae14p-6f) * (l_y - 0x1.47ae14p-6f)))) < 0x1.5c28f6p-4f));
  l_body = (((l_y < 0x1.47ae14p-6f) && (l_y > (0.0f - 0x1.70a3d8p-4f))) && (l_ax < (0x1.5c28f6p-4f + ((0x1.47ae14p-6f - l_y) * 0x1.000000p-1f))));
  l_lip = (((l_y <= (0.0f - 0x1.70a3d8p-4f)) && (l_y > (0.0f - 0x1.d70a3ep-4f))) && (l_ax < 0x1.47ae14p-3f));
  l_clapper = (sqrt(((l_x * l_x) + ((l_y + 0x1.28f5c2p-3f) * (l_y + 0x1.28f5c2p-3f)))) < 0x1.a9fbe8p-6f);
  l_knob = (sqrt(((l_x * l_x) + ((l_y - 0x1.000000p-3f) * (l_y - 0x1.000000p-3f)))) < 0x1.89374cp-6f);
  if (((((l_dome || l_body) || l_lip) || l_clapper) || l_knob))
  {
    return 0x1.000000p0f;
  }
  return 0.0f;
}
fn coax_p_sty_star(p_x: f32, p_y: f32, p_size: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_size: f32 = p_size;
  if (((sqrt(abs(l_x)) + sqrt(abs(l_y))) < sqrt(l_size)))
  {
    return 0x1.000000p0f;
  }
  return 0.0f;
}
fn coax_p_sty_river(p_base: vec3f, p_a: f32, p_b: f32, p_x: f32, p_z: f32, p_deep: f32, p_churn: f32, p_t: f32) -> vec4f {
  let l_base: vec3f = p_base;
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  let l_deep: f32 = p_deep;
  let l_churn: f32 = p_churn;
  let l_t: f32 = p_t;
  var l_c: vec3f;
  var l_wa: f32;
  var l_wb: f32;
  var l_r1: f32;
  var l_r2: f32;
  var l_net: f32;
  var l_wobble: f32;
  var l_shore: f32;
  var l_white: f32;
  var l_foam: f32;
  l_c = coax_p_mix3(coax_p_v3(0x1.99999ap-2f, 0x1.99999ap-1f, 0x1.b851ecp-1f), l_base, coax_p_smoothstep(0x1.47ae14p-5f, 0x1.b33334p-1f, l_deep));
  l_wa = (l_a + ((coax_p_sty_noise2(((l_a * 0x1.ccccccp-2f) - (l_t * 0x1.666666p-2f)), (l_b * 0x1.ccccccp-2f)) - 0x1.000000p-1f) * 0x1.19999ap1f));
  l_wb = (l_b + ((coax_p_sty_noise2(((l_a * 0x1.ccccccp-2f) + 0x1.200000p3f), ((l_b * 0x1.ccccccp-2f) + (l_t * 0x1.000000p-2f))) - 0x1.000000p-1f) * 0x1.19999ap1f));
  l_r1 = (0x1.000000p0f - abs(((coax_p_sty_noise2(((l_wa - (l_t * 0x1.4cccccp0f)) * 0x1.000000p-1f), (l_wb * 0x1.333334p0f)) * 0x1.000000p1f) - 0x1.000000p0f)));
  l_r2 = (0x1.000000p0f - abs(((coax_p_sty_noise2((((l_wa - (l_t * 0x1.b33334p-1f)) * 0x1.266666p0f) + 0x1.d33334p2f), ((l_wb * 0x1.400000p1f) + 0x1.8cccccp1f)) * 0x1.000000p1f) - 0x1.000000p0f)));
  l_net = coax_p_fmax((coax_p_smoothstep(0x1.b851ecp-1f, 0x1.e66666p-1f, l_r1) * 0x1.ccccccp-1f), (coax_p_smoothstep(0x1.ccccccp-1f, 0x1.f0a3d8p-1f, l_r2) * 0x1.000000p-1f));
  l_c = coax_p_scale3(l_c, (0x1.a3d70ap-1f + (0x1.333334p-2f * coax_p_sty_noise2(((l_wa * 0x1.70a3d8p-2f) - (l_t * 0x1.000000p-1f)), (l_wb * 0x1.19999ap-1f)))));
  l_c = coax_p_mix3(l_c, coax_p_v3(0x1.ae147ap-1f, 0x1.eb851ep-1f, 0x1.000000p0f), (coax_p_saturate(l_net) * coax_p_smoothstep(0x1.eb851ep-6f, 0x1.333334p-2f, l_deep)));
  l_wobble = coax_p_sty_noise2(((l_x * 0x1.266666p1f) + (l_t * 0x1.666666p-2f)), ((l_z * 0x1.266666p1f) - (l_t * 0x1.99999ap-3f)));
  l_shore = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-7f, (0x1.47ae14p-5f + (0x1.70a3d8p-4f * l_wobble)), l_deep));
  l_white = coax_p_smoothstep(0x1.333334p-2f, 0x1.19999ap-1f, (l_churn + (((coax_p_sty_noise2(((l_x * 0x1.e66666p0f) - (l_t * 0x1.19999ap0f)), ((l_z * 0x1.e66666p0f) + (l_t * 0x1.333334p-1f))) - 0x1.000000p-1f) * 0x1.333334p-1f) * coax_p_smoothstep(0x1.47ae14p-6f, 0x1.99999ap-3f, l_churn))));
  l_foam = coax_p_fmax((l_shore * coax_p_smoothstep((0.0f - 0x1.eb851ep-6f), 0x1.47ae14p-6f, l_deep)), l_white);
  l_c = coax_p_mix3(l_c, coax_p_v3(0x1.f0a3d8p-1f, 0x1.fae148p-1f, 0x1.000000p0f), l_foam);
  return coax_p_v4(l_c.x, l_c.y, l_c.z, coax_p_fmin(coax_p_fmax((coax_p_smoothstep((0.0f - 0x1.47ae14p-6f), 0x1.333334p-2f, l_deep) * 0x1.e147aep-1f), l_foam), 0x1.f0a3d8p-1f));
}
fn coax_p_sty_falls(p_base: vec3f, p_a: f32, p_b: f32, p_t: f32) -> vec4f {
  let l_base: vec3f = p_base;
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  var l_w: f32;
  var l_c: vec3f;
  var l_side: f32;
  l_w = ((coax_p_sty_noise2((l_a * 0x1.600000p3f), ((l_b * 0x1.99999ap-3f) - (l_t * 0x1.b33334p0f))) * 0x1.333334p-1f) + (coax_p_sty_noise2(((l_a * 0x1.a00000p4f) + 0x1.800000p1f), ((l_b * 0x1.000000p-1f) - (l_t * 0x1.733334p1f))) * 0x1.99999ap-2f));
  l_c = coax_p_mix3(l_base, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_smoothstep(0x1.147ae2p-2f, 0x1.000000p-1f, l_w));
  l_side = ((coax_p_fmin(l_a, (0x1.000000p0f - l_a)) + ((coax_p_sty_noise2(((l_b * 0x1.666666p-1f) - (l_t * 0x1.19999ap1f)), (l_a * 0x1.200000p3f)) - 0x1.000000p-1f) * 0x1.c28f5cp-3f)) + ((coax_p_sty_noise2((l_a * 0x1.400000p5f), ((l_b * 0x1.666666p-2f) - (l_t * 0x1.4cccccp1f))) - 0x1.000000p-1f) * 0x1.99999ap-4f));
  return coax_p_v4(l_c.x, l_c.y, l_c.z, coax_p_saturate(((coax_p_smoothstep(0x1.47ae14p-6f, 0x1.99999ap-4f, l_side) * (0x1.ae147ap-1f + (0x1.47ae14p-3f * l_w))) * coax_p_smoothstep((0.0f - 0x1.99999ap-3f), 0x1.000000p-1f, l_b))));
}
fn coax_p_sty_spectrum(p_k: f32) -> vec3f {
  let l_k: f32 = p_k;
  var l_h: f32;
  l_h = ((0x1.000000p0f - coax_p_saturate(l_k)) * 0x1.851eb8p-1f);
  return coax_p_v3(coax_p_saturate((abs(((coax_p_fract(l_h) * 0x1.800000p2f) - 0x1.800000p1f)) - 0x1.000000p0f)), coax_p_saturate((abs(((coax_p_fract((l_h + 0x1.555556p-1f)) * 0x1.800000p2f) - 0x1.800000p1f)) - 0x1.000000p0f)), coax_p_saturate((abs(((coax_p_fract((l_h + 0x1.555554p-2f)) * 0x1.800000p2f) - 0x1.800000p1f)) - 0x1.000000p0f)));
}
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  let l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
}
fn coax_p_wg_max_i(p_a: i32, p_b: i32) -> i32 {
  let l_a: i32 = p_a;
  let l_b: i32 = p_b;
  return select(l_a, l_b, (l_b > l_a));
}
fn coax_p_wg_min_i(p_a: i32, p_b: i32) -> i32 {
  let l_a: i32 = p_a;
  let l_b: i32 = p_b;
  return select(l_a, l_b, (l_b < l_a));
}
fn coax_p_wg_atan01(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
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
fn coax_p_sty_path_share(p_x: f32, p_z: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  var l_soft: f32;
  l_soft = coax_p_fmax(coax_at_u(i32(97i)), 0x1.47ae14p-6f);
  return (0x1.000000p0f - coax_p_smoothstep((0.0f - l_soft), l_soft, coax_p_sty_path(l_x, l_z)));
}
fn coax_p_sty_flags(p_x: f32, p_z: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  var l_ix: f32;
  var l_iz: f32;
  var l_near: f32;
  var l_next: f32;
  var l_id: f32;
  var l_j: i32;
  var l_i: i32;
  var l_cx: f32;
  var l_cz: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_d: f32;
  l_ix = floor(l_x);
  l_iz = floor(l_z);
  l_near = 0x1.400000p3f;
  l_next = 0x1.400000p3f;
  l_id = 0.0f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 3i;
    for (l_j = coax_from; l_j < coax_to; l_j = l_j + 1i)
    {
      {
        let coax_from: i32 = 0i;
        let coax_to: i32 = 3i;
        for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
        {
          l_cx = ((l_ix + f32(l_i)) - 0x1.000000p0f);
          l_cz = ((l_iz + f32(l_j)) - 0x1.000000p0f);
          l_dx = (((l_cx + 0x1.000000p-1f) + ((coax_p_sty_hash2(l_cx, l_cz) - 0x1.000000p-1f) * 0x1.70a3d8p-1f)) - l_x);
          l_dz = (((l_cz + 0x1.000000p-1f) + ((coax_p_sty_hash2((l_cx + 0x1.100000p4f), (l_cz + 0x1.d00000p4f)) - 0x1.000000p-1f) * 0x1.70a3d8p-1f)) - l_z);
          l_d = sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
          if ((l_d < l_near))
          {
            l_next = l_near;
            l_near = l_d;
            l_id = coax_p_sty_hash2((l_cx + 0x1.400000p2f), (l_cz + 0x1.800000p1f));
          }
          else
          {
            if ((l_d < l_next))
            {
              l_next = l_d;
            }
          }
        }
      }
    }
  }
  return coax_p_v3(l_near, l_next, l_id);
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn shadow_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_13 {
  let coax_r = coax_p_shadow_vs(coax_vid, coax_iid);
  return coax_io_13(coax_r.x);
}
@vertex
fn grass_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_14 {
  let coax_r = coax_p_grass_vs(coax_vid, coax_iid);
  return coax_io_14(coax_r.x, coax_r.y, coax_r.z, coax_r.w);
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6);
}
@vertex
fn water_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_water_vs(coax_vid, coax_iid);
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
fn shadow_fs(coax_in: coax_io_13) {
  coax_p_shadow_fs(coax_t13(coax_in.x));
}
@fragment
fn grass_fs(coax_in: coax_io_14) -> @location(0) vec4f {
  let coax_r = coax_p_grass_fs(coax_t14(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_r;
}
@fragment
fn water_transmit_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_water_transmit_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_r;
}
@fragment
fn water_emit_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_water_emit_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
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