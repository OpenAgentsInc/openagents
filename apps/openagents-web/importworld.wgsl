// ---- generated: apps/openagents-web/gpu/importworld.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec4f,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec4f,
}
struct coax_t12 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_in_12 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
}
struct coax_t13 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t15 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 64>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> objects: array<vec4f>;
fn coax_p_bg_vs(p_i: u32, p_j: u32) -> vec4f {
  let l_i: u32 = p_i;
  let l_j: u32 = p_j;
  return coax_p_splash_bg_vs(l_i, l_j);
}
fn coax_p_scene_vs(p_i: u32, p_j: u32) -> coax_t11 {
  let l_i: u32 = p_i;
  let l_j: u32 = p_j;
  var l_at: i32;
  var l_o: i32;
  var l_rest: vec4f;
  var l_rn: vec3f;
  var l_word: u32;
  var l_vc: vec4f;
  var l_p: vec3f;
  var l_n: vec3f;
  var l_m: vec4f;
  var l_r: vec3f;
  var l_z: f32;
  var l_near: f32;
  var l_far: f32;
  var l_clip: vec4f;
  l_at = (i32(l_i) * 12i);
  l_o = (i32(l_j) * 7i);
  l_rest = coax_p_v4(bitcast<f32>(vertices[l_at]), bitcast<f32>(vertices[(l_at + 1i)]), bitcast<f32>(vertices[(l_at + 2i)]), 0x1.000000p0f);
  l_rn = coax_p_v3(bitcast<f32>(vertices[(l_at + 3i)]), bitcast<f32>(vertices[(l_at + 4i)]), bitcast<f32>(vertices[(l_at + 5i)]));
  l_word = vertices[(l_at + 8i)];
  l_vc = coax_p_v4((f32((l_word & 255u)) / 0x1.fe0000p7f), (f32(((l_word >> u32(8u)) & 255u)) / 0x1.fe0000p7f), (f32(((l_word >> u32(16u)) & 255u)) / 0x1.fe0000p7f), (f32(((l_word >> u32(24u)) & 255u)) / 0x1.fe0000p7f));
  l_p = coax_p_v3(coax_p_wg_dot4(objects[l_o], l_rest), coax_p_wg_dot4(objects[(l_o + 1i)], l_rest), coax_p_wg_dot4(objects[(l_o + 2i)], l_rest));
  l_n = coax_p_v3(coax_p_wg_dot3(coax_p_v3(objects[(l_o + 3i)].x, objects[(l_o + 3i)].y, objects[(l_o + 3i)].z), l_rn), coax_p_wg_dot3(coax_p_v3(objects[(l_o + 4i)].x, objects[(l_o + 4i)].y, objects[(l_o + 4i)].z), l_rn), coax_p_wg_dot3(coax_p_v3(objects[(l_o + 5i)].x, objects[(l_o + 5i)].y, objects[(l_o + 5i)].z), l_rn));
  l_m = objects[(l_o + 6i)];
  l_r = coax_p_sub3(l_p, coax_p_iwr_vec(8i));
  l_z = coax_p_wg_dot3(l_r, coax_p_iwr_vec(20i));
  l_near = 0x1.99999ap-5f;
  l_far = 0x1.f40000p11f;
  l_clip = coax_p_v4(((0x1.000000p1f * ((coax_p_wg_dot3(l_r, coax_p_iwr_vec(12i)) * coax_at_u(i32(24i))) + ((coax_at_u(i32(26i)) - (coax_at_u(i32(0i)) * 0x1.000000p-1f)) * l_z))) / coax_at_u(i32(0i))), ((0x1.000000p1f * ((coax_p_wg_dot3(l_r, coax_p_iwr_vec(16i)) * coax_at_u(i32(24i))) + (((coax_at_u(i32(1i)) * 0x1.000000p-1f) - coax_at_u(i32(27i))) * l_z))) / coax_at_u(i32(1i))), ((l_far / (l_far - l_near)) * (l_z - l_near)), l_z);
  return coax_t11(l_clip, l_p, l_n, coax_p_v4((l_vc.x * l_m.x), (l_vc.y * l_m.y), (l_vc.z * l_m.z), l_m.w));
}
fn coax_p_fx_vs(p_i: u32, p_j: u32, p_a: coax_t12) -> coax_t15 {
  let l_i: u32 = p_i;
  let l_j: u32 = p_j;
  let l_a: coax_t12 = p_a;
  return coax_p_splash_fx_vs(l_i, l_j, coax_t13(l_a.x, l_a.y, l_a.z, coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f)));
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  let l_pos: vec4f = p_pos;
  var l_x: f32;
  var l_y: f32;
  var l_ray: vec3f;
  var l_up: f32;
  var l_c: vec3f;
  l_x = ((l_pos.x - coax_at_u(i32(26i))) / coax_at_u(i32(24i)));
  l_y = ((coax_at_u(i32(27i)) - l_pos.y) / coax_at_u(i32(24i)));
  l_ray = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_iwr_vec(20i), coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_iwr_vec(12i), l_x), coax_p_wg_mul_f3f(coax_p_iwr_vec(16i), l_y))));
  l_up = coax_p_clampf((l_ray.y * 0x1.800000p1f), (-0x1.000000p0f), 0x1.000000p0f);
  l_c = select(coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(coax_p_iwr_vec(36i), 0x1.266666p0f), coax_p_iwr_vec(40i), (-l_up)), coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(coax_p_iwr_vec(36i), 0x1.266666p0f), coax_p_iwr_vec(36i), l_up), (l_up > 0.0f));
  l_c = coax_p_wg_mul_f3f(l_c, coax_at_u(i32(3i)));
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_scene_fs(p_v: coax_t11, p_front: bool) -> vec4f {
  let l_v: coax_t11 = p_v;
  let l_front: bool = p_front;
  var l_base: vec3f;
  var l_c: vec3f;
  var l_n: vec3f;
  var l_key: vec3f;
  var l_sky: vec3f;
  var l_light: vec3f;
  var l_k: i32;
  var l_b: i32;
  var l_to: vec3f;
  var l_d: f32;
  var l_fall: f32;
  var l_dist: f32;
  var l_fog: f32;
  l_base = coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z);
  l_c = l_base;
  if ((l_v.w.w >= 0.0f))
  {
    l_n = coax_p_wg_normalize3(l_v.z);
    if ((!l_front))
    {
      l_n = coax_p_wg_mul_f3f(l_n, (-0x1.000000p0f));
    }
    l_key = coax_p_wg_normalize3(coax_p_iwr_vec(28i));
    l_sky = coax_p_wg_mix_f3f3f(coax_p_iwr_vec(40i), coax_p_iwr_vec(36i), ((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f));
    l_light = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_sky, coax_at_u(i32(31i))), coax_p_wg_mul_f3f(coax_p_iwr_vec(32i), coax_p_fmax(coax_p_wg_dot3(l_n, l_key), 0.0f)));
    l_k = 0i;
    while ((l_k < 16i))
    {
      if ((f32(l_k) >= coax_at_u(i32(35i))))
      {
        break;
      }
      l_b = (44i + (l_k * 8i));
      l_to = coax_p_sub3(coax_p_iwr_vec(l_b), l_v.y);
      l_d = coax_p_wg_length3(l_to);
      l_fall = coax_p_clampf((0x1.000000p0f - (l_d / coax_p_fmax(coax_at_u(i32((l_b + 3i))), 0x1.47ae14p-7f))), 0.0f, 0x1.000000p0f);
      l_light = coax_p_wg_add_f3f3(l_light, coax_p_wg_mul_f3f(coax_p_iwr_vec((l_b + 4i)), ((l_fall * l_fall) * coax_p_fmax(coax_p_wg_dot3(l_n, coax_p_wg_mul_f3f(l_to, (0x1.000000p0f / coax_p_fmax(l_d, 0x1.0624dep-10f)))), 0.0f))));
      l_k = (l_k + 1i);
    }
    l_c = coax_p_v3((l_base.x * l_light.x), (l_base.y * l_light.y), (l_base.z * l_light.z));
  }
  l_dist = coax_p_wg_length3(coax_p_sub3(l_v.y, coax_p_iwr_vec(8i)));
  l_fog = coax_p_clampf((l_dist / coax_p_fmax(coax_at_u(i32(39i)), 0x1.000000p0f)), 0.0f, 0x1.000000p0f);
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_iwr_vec(36i), (l_fog * l_fog));
  l_c = coax_p_wg_mul_f3f(l_c, coax_at_u(i32(3i)));
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t15) -> vec4f {
  let l_v: coax_t15 = p_v;
  if ((l_v.w.w > 0x1.200000p2f))
  {
    return coax_p_hud_primitive_fs(l_v.y, l_v.z, l_v.w, coax_at_u(i32(3i)));
  }
  return coax_p_splash_fx_fs(l_v);
}
fn coax_p_splash_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  let l_i: u32 = p_i;
  let l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
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
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  let l_a: vec4f = p_a;
  let l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_iwr_vec(p_at: i32) -> vec3f {
  let l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_splash_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t13) -> coax_t15 {
  let l__vid2: u32 = p__vid2;
  let l__iid3: u32 = p__iid3;
  let l_in_: coax_t13 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t15;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t15();
  l_ndc = coax_p_v2((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_o.x = coax_p_v4(l_ndc.x, l_ndc.y, 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_low: f32 = p_low;
  let l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  let l_p0: vec3f = p_p0;
  let l_p1: vec3f = p_p1;
  let l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b > l_a));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_hud_primitive_fs(p_place: vec2f, p_shape: vec4f, p_tint: vec4f, p_light: f32) -> vec4f {
  let l_place: vec2f = p_place;
  let l_shape: vec4f = p_shape;
  let l_tint: vec4f = p_tint;
  let l_light: f32 = p_light;
  var l_alpha: f32;
  l_alpha = (l_shape.z * l_light);
  if ((l_tint.w < 0x1.600000p2f))
  {
    l_alpha = (l_alpha * coax_p_hud_radial(coax_p_wg_length2(l_place), l_shape.x));
  }
  else
  {
    if ((l_tint.w > 0x1.a00000p2f))
    {
      l_alpha = (l_alpha * (0x1.000000p0f - coax_p_smoothstep((l_shape.x - 0x1.000000p-1f), (l_shape.x + 0x1.000000p-1f), coax_p_wg_length2(l_place))));
    }
  }
  return coax_p_v4((l_tint.x * l_alpha), (l_tint.y * l_alpha), (l_tint.z * l_alpha), 0x1.000000p0f);
}
fn coax_p_splash_fx_fs(p_v: coax_t15) -> vec4f {
  let l_v: coax_t15 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_mode: f32;
  var l_p: vec2f;
  var l_c: f32;
  var l_r: f32;
  var l_tail: f32;
  var l_g: f32;
  var l_hot: f32;
  var l_c_11: vec3f;
  var l_core: f32;
  var l_g_13: f32;
  var l_halo: f32;
  var l_ends: f32;
  var l_c_16: vec3f;
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_mode = l_v.w.w;
  if ((l_mode > 0x1.c00000p1f))
  {
    l_p = coax_p_wg_div_f2f(l_v.y, (0x1.000000p0f - (0x1.eb851ep-6f * l_v.f4.x)));
    l_c = coax_p_metaverse_bar((-l_p.x), l_p.y, l_v.z.x, l_v.z.y, l_v.z.z, coax_at_u(i32(2i)), l_v.f4.x, l_v.f4.y);
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_c), l_v.z.w), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  if (((l_mode > 0x1.800000p0f) && (l_mode < 0x1.400000p1f)))
  {
    l_r = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
    if ((l_r >= 0x1.000000p0f))
    {
      return coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f);
    }
    l_tail = select(0x1.000000p0f, coax_p_mix(0x1.47ae14p-4f, 0x1.000000p0f, coax_p_clampf((l_v.y.x / l_v.z.x), 0.0f, 0x1.000000p0f)), (l_v.z.x > 0x1.000000p0f));
    l_g = (exp((((-l_r) * l_r) * 0x1.400000p2f)) * (0x1.000000p0f - l_r));
    l_hot = (exp((((-l_r) * l_r) * 0x1.200000p5f)) * l_v.z.y);
    l_c_11 = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_g), coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.b851ecp-1f), l_hot)), l_tail), l_v.z.z), coax_at_u(i32(3i)));
    return coax_p_v4(l_c_11.x, l_c_11.y, l_c_11.z, 0x1.000000p0f);
  }
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g_13 = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g_13) * l_g_13)) * 0x1.1eb852p-2f);
  if (((l_mode > 0x1.000000p-1f) && (l_mode < 0x1.800000p0f)))
  {
    l_ends = (coax_p_smoothstep(0.0f, 0x1.000000p3f, l_v.y.x) * coax_p_smoothstep(0.0f, 0x1.000000p3f, (l_v.z.x - l_v.y.x)));
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), ((l_core * 0x1.99999ap-1f) + l_halo)), l_ends), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  if (((l_mode > 0x1.400000p1f) && (l_mode < 0x1.c00000p1f)))
  {
    l_c_16 = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.d70a3ep-1f), 0x1.800000p-1f), l_core), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_halo), 0x1.ccccccp0f)), l_v.z.z), coax_at_u(i32(3i)));
    return coax_p_v4(l_c_16.x, l_c_16.y, l_c_16.z, 0x1.000000p0f);
  }
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b < l_a));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  let l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_hud_radial(p_distance: f32, p_radius: f32) -> f32 {
  let l_distance: f32 = p_distance;
  let l_radius: f32 = p_radius;
  if ((l_radius <= 0.0f))
  {
    return 0.0f;
  }
  return coax_p_clampf((0x1.000000p0f - (l_distance / l_radius)), 0.0f, 0x1.000000p0f);
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_metaverse_bar(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32, p_t: f32, p_press: f32, p_flash: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_w: f32 = p_w;
  let l_h: f32 = p_h;
  let l_k: f32 = p_k;
  let l_t: f32 = p_t;
  let l_press: f32 = p_press;
  let l_flash: f32 = p_flash;
  var l_d: f32;
  var l_inside: f32;
  var l_edge: f32;
  var l_phase: f32;
  var l_sweep: f32;
  var l_glass: f32;
  var l_glow: f32;
  l_d = coax_p_cut_distance(l_x, l_y, l_w, l_h, l_k);
  l_inside = (0x1.000000p0f - coax_p_smooth((-0x1.000000p-1f), 0x1.000000p-1f, l_d));
  l_edge = (0x1.000000p0f - coax_p_smooth((0x1.000000p-1f * l_k), (0x1.99999ap0f * l_k), abs(l_d)));
  l_phase = ((l_t / 0x1.800000p2f) - floor((l_t / 0x1.800000p2f)));
  l_sweep = exp((-(((((l_x + (l_y * 0x1.333334p-1f)) / l_w) - ((-0x1.99999ap0f) + ((0x1.99999ap1f * l_phase) / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f) * ((((l_x + (l_y * 0x1.333334p-1f)) / l_w) - ((-0x1.99999ap0f) + ((0x1.99999ap1f * l_phase) / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f))));
  if ((l_phase > 0x1.c28f5cp-3f))
  {
    l_sweep = 0.0f;
  }
  l_glass = ((((0x1.1eb852p-5f + (0x1.99999ap-6f * (0x1.000000p0f - (l_y / l_h)))) + (0x1.99999ap-4f * l_press)) + (0x1.99999ap-4f * l_sweep)) * l_inside);
  l_glow = ((exp(((-coax_p_hi(l_d, 0.0f)) / (0x1.200000p3f * l_k))) * (0x1.000000p0f - l_inside)) * (0x1.99999ap-4f + (0x1.333334p-2f * l_flash)));
  return ((l_glass + (l_edge * (0x1.4cccccp-1f + (0x1.000000p-1f * l_flash)))) + l_glow);
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  let l_p0: vec3f = p_p0;
  let l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_cut_distance(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_w: f32 = p_w;
  let l_h: f32 = p_h;
  let l_k: f32 = p_k;
  var l_dx: f32;
  var l_dy: f32;
  var l_box: f32;
  l_dx = (abs(l_x) - l_w);
  l_dy = (abs(l_y) - l_h);
  l_box = (sqrt(((coax_p_hi(l_dx, 0.0f) * coax_p_hi(l_dx, 0.0f)) + (coax_p_hi(l_dy, 0.0f) * coax_p_hi(l_dy, 0.0f)))) + coax_p_lo(coax_p_hi(l_dx, l_dy), 0.0f));
  return coax_p_hi(l_box, ((abs((l_x + l_y)) - ((l_w + l_h) - (0x1.c00000p3f * l_k))) * 0x1.6a09e6p-1f));
}
fn coax_p_smooth(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_sat(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_hi(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lo(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_sat(p_a: f32) -> f32 {
  let l_a: f32 = p_a;
  return coax_p_lo(0x1.000000p0f, coax_p_hi(0.0f, l_a));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_11 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_12) -> coax_io_15 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t12(coax_in.x, coax_in.y, coax_in.z));
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_11, @builtin(front_facing) coax_front: bool) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w), coax_front);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_r;
}