// ---- generated: apps/openagents-web/gpu/gridcrowd.coax (the module) by coax wgsl; do not edit
struct coax_t12 {
  x: vec4f,
  y: vec4f,
}
struct coax_t15 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec3f,
  f5: f32,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec3f,
  @location(4) @interpolate(flat) f5: f32,
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 9>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> lines: array<u32>;
@group(1) @binding(2) var<storage, read> bones: array<vec4f>;
@group(1) @binding(3) var<storage, read> body: array<u32>;
@group(1) @binding(4) var<storage, read> robots: array<coax_t12>;
@group(1) @binding(5) var<storage, read> palette: array<vec4f>;
@group(1) @binding(6) var sheets: texture_2d_array<f32>;
@group(1) @binding(7) var sheet: sampler;
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
  var l_o: coax_t15;
  var l_r: coax_t12;
  var l_w: vec3f;
  var l_k: u32;
  var l_b: u32;
  var l_c: u32;
  var l_a: vec3f;
  var l_e: vec3f;
  var l_corner: u32;
  var l_alongs: array<f32, 6>;
  var l_along: f32;
  var l_sides: array<f32, 6>;
  var l_side: f32;
  var l_pa: vec4f;
  var l_pe: vec4f;
  var l_near: f32;
  var l_t: f32;
  var l_t_26: f32;
  var l_sa: vec2f;
  var l_se: vec2f;
  var l_dir: vec2f;
  var l_n: vec2f;
  var l_p: vec4f;
  l_o = coax_t15();
  if ((f32(l_instance) >= coax_at_u(i32(35i))))
  {
    return coax_p_gridcrowd_person(l_id, (l_instance - coax_p_wg_u32_f(coax_at_u(i32(35i)))));
  }
  l_o.f5 = (-0x1.000000p0f);
  l_r = robots[i32(l_instance)];
  if (((l_id & 2147483648u) == 0u))
  {
    l_w = coax_p_v3(coax_p_gridcrowd_f((l_id * 6u)), coax_p_gridcrowd_f(((l_id * 6u) + 1u)), coax_p_gridcrowd_f(((l_id * 6u) + 2u)));
    if ((f32(l_id) >= coax_at_u(i32(7i))))
    {
      l_w = coax_p_gridcrowd_skinned(l_id, l_r);
    }
    l_o.y = l_w;
    l_o.x = coax_p_gridcrowd_clip(l_w);
    l_o.z = coax_p_wg_sw_rgb_f4(coax_p_wg_unpack4x8unorm(vertices[i32(((l_id * 6u) + 3u))]));
    return l_o;
  }
  l_k = (l_id & 2147483647u);
  l_b = ((l_k / 6u) * 7u);
  l_c = lines[i32((l_b + 6u))];
  l_a = coax_p_v3(coax_p_gridcrowd_lf(l_b), coax_p_gridcrowd_lf((l_b + 1u)), coax_p_gridcrowd_lf((l_b + 2u)));
  l_e = coax_p_v3(coax_p_gridcrowd_lf((l_b + 3u)), coax_p_gridcrowd_lf((l_b + 4u)), coax_p_gridcrowd_lf((l_b + 5u)));
  if (((l_c >> u32(24u)) == 0u))
  {
    l_a = coax_p_gridcrowd_skinned(lines[i32(l_b)], l_r);
    l_e = coax_p_gridcrowd_skinned(lines[i32((l_b + 1u))], l_r);
  }
  l_corner = (l_k % 6u);
  l_alongs = array<f32, 6>(0.0f, 0x1.000000p0f, 0x1.000000p0f, 0.0f, 0x1.000000p0f, 0.0f);
  l_along = l_alongs[i32(l_corner)];
  l_sides = array<f32, 6>((-0x1.000000p0f), (-0x1.000000p0f), 0x1.000000p0f, (-0x1.000000p0f), 0x1.000000p0f, 0x1.000000p0f);
  l_side = l_sides[i32(l_corner)];
  l_pa = coax_p_gridcrowd_clip(l_a);
  l_pe = coax_p_gridcrowd_clip(l_e);
  l_near = 0x1.99999ap-5f;
  if (((l_pa.w < l_near) && (l_pe.w < l_near)))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    return l_o;
  }
  if ((l_pa.w < l_near))
  {
    l_t = ((l_near - l_pa.w) / (l_pe.w - l_pa.w));
    l_pa = coax_p_wg_mix_f4f4f(l_pa, l_pe, l_t);
    l_a = coax_p_wg_mix_f3f3f(l_a, l_e, l_t);
  }
  if ((l_pe.w < l_near))
  {
    l_t_26 = ((l_near - l_pe.w) / (l_pa.w - l_pe.w));
    l_pe = coax_p_wg_mix_f4f4f(l_pe, l_pa, l_t_26);
    l_e = coax_p_wg_mix_f3f3f(l_e, l_a, l_t_26);
  }
  l_sa = coax_p_wg_mul_f2f(coax_p_wg_mul_f2f2(coax_p_wg_div_f2f(coax_p_v2(l_pa.x, l_pa.y), l_pa.w), coax_p_wg_at2(0i)), 0x1.000000p-1f);
  l_se = coax_p_wg_mul_f2f(coax_p_wg_mul_f2f2(coax_p_wg_div_f2f(coax_p_v2(l_pe.x, l_pe.y), l_pe.w), coax_p_wg_at2(0i)), 0x1.000000p-1f);
  l_dir = coax_p_wg_sub_f2f2(l_se, l_sa);
  if ((coax_p_wg_length2(l_dir) < 0x1.a36e2ep-14f))
  {
    l_dir = coax_p_v2(0x1.000000p0f, 0.0f);
  }
  l_n = coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_normalize2(coax_p_v2((-l_dir.y), l_dir.x)), coax_at_u(i32(27i))), 0x1.000000p-1f), l_side);
  l_p = coax_p_wg_mix_f4f4f(l_pa, l_pe, l_along);
  l_o.x = coax_p_wg_make_f4_f2ff(coax_p_wg_add_f2f2(coax_p_v2(l_p.x, l_p.y), coax_p_wg_mul_f2f(coax_p_wg_div_f2f2(l_n, coax_p_wg_mul_f2f(coax_p_wg_at2(0i), 0x1.000000p-1f)), l_p.w)), l_p.z, l_p.w);
  l_o.y = coax_p_wg_mix_f3f3f(l_a, l_e, l_along);
  l_o.z = coax_p_wg_sw_rgb_f4(coax_p_wg_unpack4x8unorm(l_c));
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t17) -> coax_t18 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
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
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  return coax_p_wg_make_f4_f3f(coax_p_GRIDCROWD_FIELD(), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t15) -> vec4f {
  var l_s: coax_t15 = p_s;
  var l_d: f32;
  var l_t: f32;
  var l_albedo: vec3f;
  var l_up: vec3f;
  var l_light: f32;
  var l_colour: vec3f;
  var l_c: vec3f;
  l_d = coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(coax_at_u(i32(24i)), coax_at_u(i32(26i))), coax_p_v2(l_s.y.x, l_s.y.z)));
  l_t = coax_p_clampf(((l_d - coax_at_u(i32(28i))) / coax_p_fmax((coax_at_u(i32(29i)) - coax_at_u(i32(28i))), 0x1.000000p0f)), 0.0f, 0x1.000000p0f);
  l_albedo = coax_p_wg_sw_rgb_f4(textureSample(sheets, sheet, l_s.w, coax_p_wg_i32_f(coax_p_clampf(l_s.f5, 0.0f, 0x1.400000p2f))));
  l_up = coax_p_wg_normalize3(coax_p_v3(0x1.666666p-2f, 0x1.b33334p-1f, 0x1.99999ap-2f));
  l_light = (0x1.70a3d8p-1f + (0x1.3d70a4p-1f * coax_p_fmax(coax_p_wg_dot3(coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_s.f4, coax_p_v3(0.0f, 0x1.a36e2ep-14f, 0.0f))), l_up), 0.0f)));
  l_colour = l_s.z;
  if ((l_s.f5 >= 0.0f))
  {
    l_colour = select(coax_p_gridcrowd_encode(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(l_albedo, coax_p_gridcrowd_decode(l_s.z)), l_light)), coax_p_gridcrowd_encode(coax_p_wg_mul_f3f(coax_p_gridcrowd_decode(l_s.z), l_light)), (l_s.f5 > 0x1.f40000p7f));
  }
  l_c = coax_p_wg_mix_f3f3f(l_colour, coax_p_GRIDCROWD_FIELD(), ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t))));
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
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
fn coax_p_gridcrowd_person(p_id: u32, p_h: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_h: u32 = p_h;
  var l_o: coax_t15;
  var l_v: u32;
  var l_b: u32;
  var l_p: vec4f;
  var l_n: vec3f;
  var l_w: vec4f;
  var l_j: vec4f;
  var l_at: vec3f;
  var l_nn: vec3f;
  var l_k: u32;
  var l_r: u32;
  var l_r0: vec4f;
  var l_r1: vec4f;
  var l_r2: vec4f;
  var l_tint: vec4f;
  l_o = coax_t15();
  l_v = body[i32((coax_p_wg_u32_f(coax_at_u(i32(33i))) + l_id))];
  l_b = (coax_p_wg_u32_f(coax_at_u(i32(32i))) + (l_v * 16u));
  l_p = coax_p_v4(coax_p_gridcrowd_cf(l_b), coax_p_gridcrowd_cf((l_b + 1u)), coax_p_gridcrowd_cf((l_b + 2u)), 0x1.000000p0f);
  l_n = coax_p_v3(coax_p_gridcrowd_cf((l_b + 3u)), coax_p_gridcrowd_cf((l_b + 4u)), coax_p_gridcrowd_cf((l_b + 5u)));
  l_w = coax_p_v4(coax_p_gridcrowd_cf((l_b + 8u)), coax_p_gridcrowd_cf((l_b + 9u)), coax_p_gridcrowd_cf((l_b + 10u)), coax_p_gridcrowd_cf((l_b + 11u)));
  l_j = coax_p_wg_mul_f4f(coax_p_wg_unpack4x8unorm(body[i32((l_b + 12u))]), 0x1.fe0000p7f);
  l_at = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_nn = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      if ((coax_p_wg_get_f4(l_w, i32(l_k)) <= 0.0f))
      {
        {
          l_k = (l_k + 1u);
          continue;
        }
      }
      l_r = (((l_h * coax_p_wg_u32_f(coax_at_u(i32(34i)))) + coax_p_wg_u32_f((coax_p_wg_get_f4(l_j, i32(l_k)) + 0x1.000000p-1f))) * 3u);
      l_r0 = palette[i32(l_r)];
      l_r1 = palette[i32((l_r + 1u))];
      l_r2 = palette[i32((l_r + 2u))];
      l_at = coax_p_wg_add_f3f3(l_at, coax_p_wg_mul_ff3(coax_p_wg_get_f4(l_w, i32(l_k)), coax_p_v3(coax_p_wg_dot4(l_r0, l_p), coax_p_wg_dot4(l_r1, l_p), coax_p_wg_dot4(l_r2, l_p))));
      l_nn = coax_p_wg_add_f3f3(l_nn, coax_p_wg_mul_ff3(coax_p_wg_get_f4(l_w, i32(l_k)), coax_p_v3(coax_p_wg_dot3(coax_p_v3(l_r0.x, l_r0.y, l_r0.z), l_n), coax_p_wg_dot3(coax_p_v3(l_r1.x, l_r1.y, l_r1.z), l_n), coax_p_wg_dot3(coax_p_v3(l_r2.x, l_r2.y, l_r2.z), l_n))));
      l_k = (l_k + 1u);
    }
  }
  l_tint = coax_p_wg_unpack4x8unorm(body[i32((l_b + 13u))]);
  l_o.y = l_at;
  l_o.x = coax_p_gridcrowd_clip(l_at);
  l_o.z = coax_p_v3(l_tint.x, l_tint.y, l_tint.z);
  l_o.w = coax_p_v2(coax_p_gridcrowd_cf((l_b + 6u)), coax_p_gridcrowd_cf((l_b + 7u)));
  l_o.f4 = l_nn;
  l_o.f5 = (l_tint.w * 0x1.fe0000p7f);
  return l_o;
}
fn coax_p_gridcrowd_f(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(vertices[i32(l_i)]);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_gridcrowd_skinned(p_v: u32, p_r: coax_t12) -> vec3f {
  var l_v: u32 = p_v;
  var l_r: coax_t12 = p_r;
  var l_b: u32;
  var l_p: vec4f;
  var l_joints: vec4f;
  var l_weights: vec4f;
  var l_m: vec3f;
  var l_k: u32;
  var l_w: f32;
  var l_j: u32;
  var l_q: vec3f;
  var l_row: u32;
  var l_a: f32;
  var l_c: f32;
  var l_s: vec3f;
  var l_cy: f32;
  var l_sy: f32;
  l_b = (l_v * 6u);
  l_p = coax_p_v4(coax_p_gridcrowd_f(l_b), coax_p_gridcrowd_f((l_b + 1u)), coax_p_gridcrowd_f((l_b + 2u)), 0x1.000000p0f);
  l_joints = coax_p_wg_mul_f4f(coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 4u))]), 0x1.fe0000p7f);
  l_weights = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 5u))]);
  l_m = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      l_w = coax_p_wg_get_f4(l_weights, i32(l_k));
      if ((l_w <= 0.0f))
      {
        {
          l_k = (l_k + 1u);
          continue;
        }
      }
      l_j = coax_p_wg_u32_f((coax_p_wg_get_f4(l_joints, i32(l_k)) + 0x1.000000p-1f));
      l_q = coax_p_v3(0.0f, 0.0f, 0.0f);
      {
        l_row = 0u;
        while ((l_row < 3u))
        {
          l_a = coax_p_wg_dot4(coax_p_gridcrowd_bone(l_r.y.y, l_j, l_row), l_p);
          l_c = coax_p_wg_dot4(coax_p_gridcrowd_bone(l_r.y.z, l_j, l_row), l_p);
          l_q = coax_p_wg_set_f3(l_q, i32(l_row), coax_p_mix(l_a, l_c, l_r.y.w));
          l_row = (l_row + 1u);
        }
      }
      l_m = coax_p_wg_add_f3f3(l_m, coax_p_wg_mul_f3f(l_q, l_w));
      l_k = (l_k + 1u);
    }
  }
  l_s = coax_p_wg_mul_ff3(l_r.y.x, l_m);
  l_cy = cos(l_r.x.w);
  l_sy = sin(l_r.x.w);
  return coax_p_v3(((l_r.x.x + (l_s.x * l_cy)) + (l_s.z * l_sy)), (l_r.x.y + l_s.y), ((l_r.x.z - (l_s.x * l_sy)) + (l_s.z * l_cy)));
}
fn coax_p_gridcrowd_clip(p_w: vec3f) -> vec4f {
  var l_w: vec3f = p_w;
  var l_p: vec4f;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  return coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_p), coax_p_wg_dot4(coax_p_wg_at4(12i), l_p), coax_p_wg_dot4(coax_p_wg_at4(16i), l_p), coax_p_wg_dot4(coax_p_wg_at4(20i), l_p));
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_wg_sw_rgb_f4(p_p: vec4f) -> vec3f {
  var l_p: vec4f = p_p;
  return coax_p_v3(l_p.x, l_p.y, l_p.z);
}
fn coax_p_gridcrowd_lf(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(lines[i32(l_i)]);
}
fn coax_p_wg_mix_f4f4f(p_p0: vec4f, p_p1: vec4f, p_p2: f32) -> vec4f {
  var l_p0: vec4f = p_p0;
  var l_p1: vec4f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v4(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2), coax_p_mix(l_p0.w, l_p1.w, l_p2));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
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
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
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
fn coax_p_wg_normalize2(p_a: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  return coax_p_scale2(l_a, (0x1.000000p0f / coax_p_wg_length2(l_a)));
}
fn coax_p_wg_div_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x / l_b.x), (l_a.y / l_b.y));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_make_f4_f2ff(p_p0: vec2f, p_p1: f32, p_p2: f32) -> vec4f {
  var l_p0: vec2f = p_p0;
  var l_p1: f32 = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v4(l_p0.x, l_p0.y, l_p1, l_p2);
}
fn coax_p_GRIDCROWD_FIELD() -> vec3f {
  return coax_p_v3(0x1.41205cp-5f, 0x1.41205cp-5f, 0x1.41205cp-5f);
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
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
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_gridcrowd_decode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_v3(coax_p_gridcrowd_decode1(l_c.x), coax_p_gridcrowd_decode1(l_c.y), coax_p_gridcrowd_decode1(l_c.z));
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_gridcrowd_encode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_v3(coax_p_gridcrowd_encode1(l_c.x), coax_p_gridcrowd_encode1(l_c.y), coax_p_gridcrowd_encode1(l_c.z));
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
fn coax_p_gridcrowd_cf(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(body[i32(l_i)]);
}
fn coax_p_wg_mul_f4f(p_a: vec4f, p_b: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v4((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b), (l_a.w * l_b));
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
  return dot(l_a, l_b);
}
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_gridcrowd_bone(p_frame: f32, p_j: u32, p_row: u32) -> vec4f {
  var l_frame: f32 = p_frame;
  var l_j: u32 = p_j;
  var l_row: u32 = p_row;
  return bones[i32(((((coax_p_wg_u32_f(l_frame) * coax_p_wg_u32_f(coax_at_u(i32(30i)))) + l_j) * 3u) + l_row))];
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
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
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
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
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_gridcrowd_decode1(p_c: f32) -> f32 {
  var l_c: f32 = p_c;
  if ((l_c <= 0x1.4b5dccp-5f))
  {
    return (l_c / 0x1.9d70a4p3f);
  }
  return coax_pow_bg_vs(((l_c + 0x1.c28f5cp-5f) / 0x1.0e147ap0f), 0x1.333334p1f);
}
fn coax_p_gridcrowd_encode1(p_c: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_x: f32;
  l_x = coax_p_clampf(l_c, 0.0f, 0x1.000000p0f);
  if ((l_x <= 0x1.9a5c38p-9f))
  {
    return (l_x * 0x1.9d70a4p3f);
  }
  return ((0x1.0e147ap0f * coax_pow_bg_vs(l_x, (0x1.000000p0f / 0x1.333334p1f))) - 0x1.c28f5cp-5f);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
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
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_17) -> coax_io_18 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t17(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_18(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_18) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t18(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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