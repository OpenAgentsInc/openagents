// ---- generated: apps/openagents-web/gpu/icecream.coax (the module) by coax wgsl; do not edit
struct coax_t13 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec3f,
  f4: vec4f,
  f5: f32,
}
struct coax_io_13 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
  @location(3) @interpolate(flat) f4: vec4f,
  @location(4) @interpolate(flat) f5: f32,
}
struct coax_t14 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_14 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 191>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> objects: array<vec4f>;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t13 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_o: coax_t13;
  var l_object: bool;
  var l_outline: bool;
  var l_at: u32;
  var l_light: vec3f;
  var l_tint: vec3f;
  var l_mode: f32;
  var l_shine: f32;
  var l_r0: vec4f;
  var l_r1: vec4f;
  var l_r2: vec4f;
  var l_k: u32;
  var l_i: u32;
  var l_b: u32;
  var l_p: vec3f;
  var l_n: vec4f;
  var l_c: vec4f;
  var l_slot: u32;
  var l_hull: vec4f;
  var l_w: vec3f;
  var l_normal: vec3f;
  var l_clip: vec4f;
  var l_hw: vec3f;
  var l_push: vec4f;
  var l_dir: vec2f;
  var l_len_2: f32;
  var l_near: f32;
  l_o = coax_t13();
  l_object = (l_instance >= 8388608u);
  l_outline = false;
  l_at = ((l_instance * 1020u) + l_id);
  l_light = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_tint = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_mode = 0.0f;
  l_shine = 0.0f;
  l_r0 = coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0.0f);
  l_r1 = coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, 0.0f);
  l_r2 = coax_p_v4(0.0f, 0.0f, 0x1.000000p0f, 0.0f);
  if (l_object)
  {
    l_k = (l_instance - 8388608u);
    l_outline = ((l_k & 4194304u) != 0u);
    l_i = (((l_k & (4194304u - 1u)) >> u32(10u)) * 6u);
    l_r0 = objects[i32(l_i)];
    l_r1 = objects[i32((l_i + 1u))];
    l_r2 = objects[i32((l_i + 2u))];
    l_tint = coax_p_v3(objects[i32((l_i + 3u))].x, objects[i32((l_i + 3u))].y, objects[i32((l_i + 3u))].z);
    l_shine = objects[i32((l_i + 3u))].w;
    l_light = coax_p_v3(objects[i32((l_i + 4u))].x, objects[i32((l_i + 4u))].y, objects[i32((l_i + 4u))].z);
    if ((objects[i32((l_i + 4u))].w > 0x1.000000p-1f))
    {
      l_mode = 0x1.000000p0f;
      l_light = l_tint;
    }
    l_at = (((coax_p_wg_u32_f(objects[i32((l_i + 5u))].x) + (l_k & 1023u)) * 1020u) + l_id);
  }
  l_b = (l_at * 6u);
  l_p = coax_p_v3(bitcast<f32>(vertices[i32(l_b)]), bitcast<f32>(vertices[i32((l_b + 1u))]), bitcast<f32>(vertices[i32((l_b + 2u))]));
  l_n = coax_p_wg_unpack4x8snorm(vertices[i32((l_b + 3u))]);
  l_c = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 4u))]);
  l_slot = (vertices[i32((l_b + 4u))] >> u32(24u));
  l_hull = coax_p_wg_unpack4x8snorm(vertices[i32((l_b + 5u))]);
  l_w = coax_p_v3((coax_p_wg_dot3(coax_p_v3(l_r0.x, l_r0.y, l_r0.z), l_p) + l_r0.w), (coax_p_wg_dot3(coax_p_v3(l_r1.x, l_r1.y, l_r1.z), l_p) + l_r1.w), (coax_p_wg_dot3(coax_p_v3(l_r2.x, l_r2.y, l_r2.z), l_p) + l_r2.w));
  l_normal = coax_p_v3(coax_p_wg_dot3(coax_p_v3(l_r0.x, l_r0.y, l_r0.z), coax_p_v3(l_n.x, l_n.y, l_n.z)), coax_p_wg_dot3(coax_p_v3(l_r1.x, l_r1.y, l_r1.z), coax_p_v3(l_n.x, l_n.y, l_n.z)), coax_p_wg_dot3(coax_p_v3(l_r2.x, l_r2.y, l_r2.z), coax_p_v3(l_n.x, l_n.y, l_n.z)));
  if ((!l_object))
  {
    if ((l_slot >= 128u))
    {
      l_light = coax_p_wg_min_f3f3(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(((i32(coax_p_wg_min_u((l_slot - 128u), 99u)) * 4i) + 44i))), coax_at_u(i32(((i32(coax_p_wg_min_u((l_slot - 128u), 99u)) * 4i) + 45i))), coax_at_u(i32(((i32(coax_p_wg_min_u((l_slot - 128u), 99u)) * 4i) + 46i)))), 0x1.0cccccp0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f));
      l_mode = 0x1.000000p0f;
    }
    else
    {
      l_light = coax_p_v3(coax_at_u(i32(((i32(coax_p_wg_min_u(l_slot, 99u)) * 4i) + 44i))), coax_at_u(i32(((i32(coax_p_wg_min_u(l_slot, 99u)) * 4i) + 45i))), coax_at_u(i32(((i32(coax_p_wg_min_u(l_slot, 99u)) * 4i) + 46i))));
    }
    l_shine = coax_p_fmax(l_n.w, 0.0f);
  }
  l_clip = coax_p_icecream_clip_of(l_w);
  if (l_outline)
  {
    l_mode = 0x1.000000p1f;
    if (((l_hull.w <= 0.0f) || (((l_n.x == 0.0f) && (l_n.y == 0.0f)) && (l_n.z == 0.0f))))
    {
      l_clip = coax_p_v4(0x1.000000p1f, 0x1.000000p1f, 0x1.000000p1f, 0x1.000000p0f);
    }
    else
    {
      l_hw = coax_p_v3(coax_p_wg_dot3(coax_p_v3(l_r0.x, l_r0.y, l_r0.z), coax_p_v3(l_hull.x, l_hull.y, l_hull.z)), coax_p_wg_dot3(coax_p_v3(l_r1.x, l_r1.y, l_r1.z), coax_p_v3(l_hull.x, l_hull.y, l_hull.z)), coax_p_wg_dot3(coax_p_v3(l_r2.x, l_r2.y, l_r2.z), coax_p_v3(l_hull.x, l_hull.y, l_hull.z)));
      l_push = coax_p_v4(coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))), l_hw), coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))), l_hw), coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))), l_hw), coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i))), l_hw));
      l_dir = coax_p_v2(l_push.x, l_push.y);
      l_len_2 = coax_p_wg_length2(l_dir);
      if ((l_len_2 > 0x1.0c6f7ap-20f))
      {
        l_dir = coax_p_wg_div_f2f(l_dir, l_len_2);
      }
      l_near = coax_p_clampf((0x1.800000p2f / l_clip.w), 0x1.666666p-2f, 0x1.000000p0f);
      l_clip = coax_p_wg_make_f4_f2f2(coax_p_wg_add_f2f2(coax_p_v2(l_clip.x, l_clip.y), coax_p_wg_mul_f2f(coax_p_wg_div_f2f2(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_dir, coax_at_u(i32(7i))), l_hull.w), l_near), 0x1.000000p1f), coax_p_wg_at2(0i)), l_clip.w)), coax_p_v2(l_clip.z, l_clip.w));
    }
  }
  l_o.x = l_clip;
  l_o.y = l_w;
  l_o.z = l_normal;
  l_o.w = coax_p_wg_mul_f3f3(coax_p_v3(l_c.z, l_c.y, l_c.x), l_tint);
  l_o.f4 = coax_p_v4(l_light.x, l_light.y, l_light.z, l_mode);
  l_o.f5 = l_shine;
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t14) -> coax_t15 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t14 = p_in_;
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
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  return coax_p_wg_make_f4_f3f(coax_p_icecream_panels(coax_p_v2(l_pos.x, l_pos.y), coax_p_icecream_overlay(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), coax_at_u(i32(3i))))), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t13, p_front: bool) -> vec4f {
  var l_s: coax_t13 = p_s;
  var l_front: bool = p_front;
  var l_mode: f32;
  var l_shine: f32;
  var l_fog: f32;
  var l_c: vec3f;
  var l_n: vec3f;
  var l_l: vec3f;
  var l_d: f32;
  var l_band: f32;
  var l_v: vec3f;
  var l_h: vec3f;
  var l_sp: f32;
  l_mode = l_s.f4.w;
  l_shine = l_s.f5;
  l_fog = (coax_p_smoothstep(coax_at_u(i32(32i)), coax_at_u(i32(33i)), coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y))) * coax_at_u(i32(34i)));
  l_c = vec3f();
  if ((l_mode > 0x1.800000p0f))
  {
    if (l_front)
    {
      discard;
    }
    l_c = coax_p_wg_mix_f3f3f(coax_p_ICECREAM_INK(), coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), (l_fog * 0x1.333334p-1f));
  }
  else
  {
    if ((!l_front))
    {
      discard;
    }
    if ((l_mode > 0x1.000000p-1f))
    {
      l_c = coax_p_wg_mix_f3f3f(coax_p_v3(l_s.f4.x, l_s.f4.y, l_s.f4.z), coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), (l_fog * 0x1.333334p-1f));
    }
    else
    {
      l_n = coax_p_wg_normalize3(l_s.z);
      l_l = coax_p_wg_normalize3(coax_p_v3(0x1.666666p-2f, 0x1.000000p0f, 0x1.000000p-2f));
      l_d = coax_p_wg_dot3(l_n, l_l);
      l_band = select(select(0x1.7ae148p-1f, 0x1.b851ecp-1f, (l_d > (-0x1.666666p-2f))), 0x1.000000p0f, (l_d > 0x1.000000p-2f));
      l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(l_s.w, coax_p_v3(l_s.f4.x, l_s.f4.y, l_s.f4.z)), l_band);
      if ((l_shine > 0.0f))
      {
        l_v = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y));
        l_h = coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_l, l_v));
        l_sp = coax_pow_bg_vs(coax_p_fmax(coax_p_wg_dot3(l_n, l_h), 0.0f), 0x1.c00000p4f);
        l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_smoothstep(0x1.99999ap-2f, 0x1.000000p-1f, l_sp)), 0x1.333334p-1f), l_shine), coax_p_wg_mul_f3f(coax_p_v3(0x1.eb851ep-5f, 0x1.eb851ep-5f, 0x1.eb851ep-5f), l_shine)));
      }
      l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))), l_fog);
    }
  }
  return coax_p_wg_make_f4_f3f(coax_p_icecream_panels(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_icecream_overlay(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_wg_mul_f3f(l_c, coax_at_u(i32(3i))))), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t15) -> vec4f {
  var l_v: coax_t15 = p_v;
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
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
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
fn coax_p_wg_unpack4x8snorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4(coax_p_wg_snorm8(coax_p_wg_byte(l_x, 0u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 8u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 16u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 24u)));
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_dot3(l_a, l_b);
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
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_wg_min_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmin(l_p0.x, l_p1.x), coax_p_fmin(l_p0.y, l_p1.y), coax_p_fmin(l_p0.z, l_p1.z));
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
fn coax_p_icecream_clip_of(p_w: vec3f) -> vec4f {
  var l_w: vec3f = p_w;
  var l_p: vec4f;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  return coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_p), coax_p_wg_dot4(coax_p_wg_at4(12i), l_p), coax_p_wg_dot4(coax_p_wg_at4(16i), l_p), coax_p_wg_dot4(coax_p_wg_at4(20i), l_p));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(coax_p_dot2(l_a, l_a));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_clampf(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return coax_p_fmin(coax_p_fmax(l_x, l_lo), l_hi);
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
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
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_make_f4_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec4f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p1.x, l_p1.y);
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_icecream_overlay(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_hurt: f32;
  var l_e: f32;
  var l_band: f32;
  var l_day: f32;
  var l_q: vec2f;
  var l_r: f32;
  var l_sky: vec3f;
  l_c = l_under;
  l_hurt = coax_p_clampf(coax_at_u(i32(36i)), 0.0f, 0x1.000000p0f);
  if ((l_hurt > 0.0f))
  {
    l_e = (coax_p_fmin(coax_p_fmin(l_p.x, (coax_at_u(i32(0i)) - l_p.x)), coax_p_fmin(l_p.y, (coax_at_u(i32(1i)) - l_p.y))) / (0x1.e00000p6f * coax_at_u(i32(6i))));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.000000p0f, 0x1.eb851ep-3f, 0x1.b851ecp-2f), ((l_hurt * 0x1.800000p-1f) * coax_p_clampf((0x1.000000p0f - l_e), 0.0f, 0x1.000000p0f)));
  }
  if ((coax_at_u(i32(38i)) > 0x1.000000p-1f))
  {
    l_band = coax_p_wg_fmod((l_p.y / coax_at_u(i32(6i))), 0x1.700000p5f);
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.418938p-3f, 0x1.810624p-4f, 0x1.c28f5cp-5f), select(0x1.666666p-2f, 0x1.e147aep-1f, (l_band < 0x1.100000p5f)));
  }
  l_day = coax_p_clampf(coax_at_u(i32(37i)), 0.0f, 0x1.000000p0f);
  if ((l_day > 0.0f))
  {
    l_q = coax_p_wg_sub_f2f2(coax_p_wg_div_f2f2(l_p, coax_p_wg_at2(0i)), coax_p_v2(0x1.666666p-1f, 0x1.000000p-2f));
    l_r = coax_p_wg_length2(coax_p_wg_mul_f2f2(l_q, coax_p_v2((coax_at_u(i32(0i)) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f)), 0x1.000000p0f)));
    l_sky = select(select(coax_p_v3(0x1.7f7ceep-1f, 0x1.d1eb86p-1f, 0x1.000000p0f), coax_p_v3(0x1.000000p0f, 0x1.cbc6a8p-1f, 0x1.14fdf4p-1f), (l_r < 0x1.1eb852p-3f)), coax_p_v3(0x1.000000p0f, 0x1.f7cedap-1f, 0x1.c18938p-1f), (l_r < 0x1.47ae14p-4f));
    l_c = coax_p_wg_mix_f3f3f(l_c, l_sky, l_day);
  }
  return l_c;
}
fn coax_p_icecream_panels(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_k: i32;
  l_c = l_under;
  {
    l_k = 0i;
    while ((l_k < 40i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_c = coax_p_icecream_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 444i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 444i)));
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
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(coax_p_dot3(l_a, l_a));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_ICECREAM_INK() -> vec3f {
  return coax_p_v3(0x1.5c28f6p-3f, 0x1.c28f5cp-4f, 0x1.47ae14p-4f);
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
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
fn coax_p_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return (((l_a.x * l_b.x) + (l_a.y * l_b.y)) + (l_a.z * l_b.z));
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
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return coax_p_dot4(l_a, l_b);
}
fn coax_p_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x * l_b.x) + (l_a.y * l_b.y));
}
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_icecream_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_half: vec2f;
  var l_corner: f32;
  var l_q: vec2f;
  var l_d: f32;
  var l_look: i32;
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
  if (((((l_p.x < (l_r.x - 0x1.000000p1f)) || (l_p.y < (l_r.y - 0x1.000000p1f))) || (l_p.x > (l_r.z + 0x1.000000p1f))) || (l_p.y > (l_r.w + 0x1.000000p1f))))
  {
    return l_under;
  }
  l_half = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_r.z, l_r.w), coax_p_v2(l_r.x, l_r.y)), 0x1.000000p-1f);
  l_corner = coax_p_fmin(select((0x1.000000p2f * l_s.x), l_s.w, (l_s.w > 0.0f)), coax_p_fmin(l_half.x, l_half.y));
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_d = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  if ((l_d > 0x1.000000p0f))
  {
    return l_under;
  }
  l_look = coax_p_wg_i32_f((l_s.z + 0x1.000000p-1f));
  l_fill = coax_p_v3(0x1.51eb86p-3f, 0x1.d2f1aap-4f, 0x1.3f7ceep-4f);
  l_line = coax_p_v3(0x1.6147aep-1f, 0x1.14fdf4p-1f, 0x1.28f5c2p-2f);
  if ((l_look == 1i))
  {
    l_line = coax_p_v3(0x1.b851ecp-1f, 0x1.666666p-1f, 0x1.ae147ap-2f);
  }
  if ((l_look == 2i))
  {
    l_fill = coax_p_v3(0x1.e147aep-3f, 0x1.581062p-3f, 0x1.d70a3ep-4f);
    l_line = coax_p_v3(0x1.e66666p-1f, 0x1.99999ap-1f, 0x1.0a3d70p-1f);
  }
  if ((l_look == 3i))
  {
    l_fill = coax_p_v3(0x1.28f5c2p-2f, 0x1.0a3d70p-3f, 0x1.5c28f6p-3f);
    l_line = coax_p_v3(0x1.000000p0f, 0x1.1eb852p-1f, 0x1.6147aep-1f);
  }
  if ((l_look == 4i))
  {
    l_fill = coax_p_v3(0x1.c28f5cp-3f, 0x1.47ae14p-3f, 0x1.1eb852p-4f);
    l_line = coax_p_v3(0x1.f0a3d8p-1f, 0x1.8a3d70p-1f, 0x1.5c28f6p-3f);
  }
  if ((l_look == 5i))
  {
    l_fill = coax_p_v3(0x1.333334p-2f, 0x1.99999ap-3f, 0x1.1eb852p-4f);
    l_line = coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.99999ap-2f);
  }
  l_w = ((0x1.400000p0f * l_s.x) * select(0x1.000000p0f, 0x1.99999ap0f, (l_look >= 3i)));
  l_inside = coax_p_clampf((0x1.000000p-1f - l_d), 0.0f, 0x1.000000p0f);
  l_edge = coax_p_clampf((((l_w * 0x1.000000p-1f) + 0x1.000000p-1f) - abs((l_d + (l_w * 0x1.000000p-1f)))), 0.0f, 0x1.000000p0f);
  l_a = coax_p_clampf(l_s.y, 0.0f, 0x1.000000p0f);
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_under, l_fill, (l_a * l_inside)), l_line, (l_edge * coax_p_fmin(0x1.000000p0f, (l_a + 0x1.99999ap-4f))));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return ((((l_a.x * l_b.x) + (l_a.y * l_b.y)) + (l_a.z * l_b.z)) + (l_a.w * l_b.w));
}
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
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
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(coax_p_dot3(l_a, l_a));
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
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_13 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_13(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_14) -> coax_io_15 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t14(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_13, @builtin(front_facing) coax_front: bool) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t13(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5), coax_front);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_15) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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