// ---- generated: apps/openagents-web/gpu/garden.coax (the module) by coax wgsl; do not edit
struct coax_t10 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t13 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec3f,
  f4: u32,
}
struct coax_io_13 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
  @location(3) @interpolate(flat) f4: u32,
}
struct coax_t14 {
  x: vec3f,
  y: vec3f,
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 82>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> heights: array<f32>;
@group(1) @binding(1) var<storage, read> objects: array<coax_t10>;
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
  var l_n: u32;
  var l_c: vec2f;
  var l_gx: i32;
  var l_gz: i32;
  var l_w: vec3f;
  var l_normal: vec3f;
  var l_m: coax_t10;
  var l_kind: u32;
  var l_v: coax_t14;
  var l__sw3: u32;
  var l_p: vec4f;
  var l_w_17: vec3f;
  var l_c0: vec3f;
  var l_c1: vec3f;
  var l_c2: vec3f;
  var l_normal_21: vec3f;
  l_o = coax_t13();
  l_n = coax_p_wg_u32_f(coax_at_u(i32(32i)));
  if ((l_instance < (l_n * l_n)))
  {
    l_c = coax_p_garden_quad((l_id % 6u));
    l_gx = (i32((l_instance % l_n)) + coax_p_wg_i32_f(l_c.x));
    l_gz = (i32((l_instance / l_n)) + coax_p_wg_i32_f(l_c.y));
    l_w = coax_p_v3((coax_at_u(i32(34i)) + (f32(l_gx) * coax_at_u(i32(33i)))), coax_p_garden_height(l_gx, l_gz), (coax_at_u(i32(35i)) + (f32(l_gz) * coax_at_u(i32(33i)))));
    l_normal = coax_p_wg_normalize3(coax_p_v3((coax_p_garden_height((l_gx - 1i), l_gz) - coax_p_garden_height((l_gx + 1i), l_gz)), (0x1.000000p1f * coax_at_u(i32(33i))), (coax_p_garden_height(l_gx, (l_gz - 1i)) - coax_p_garden_height(l_gx, (l_gz + 1i)))));
    l_o.x = coax_p_garden_clip(l_w);
    l_o.y = l_w;
    l_o.z = l_normal;
    l_o.w = coax_p_v3(0x1.333334p-2f, 0x1.000000p-1f, 0x1.99999ap-3f);
    l_o.f4 = 99u;
    return l_o;
  }
  l_m = objects[i32((l_instance - (l_n * l_n)))];
  l_kind = (coax_p_wg_u32_f(l_m.w.w) % 8u);
  l_v = coax_t14();
  {
    l__sw3 = l_kind;
    if ((l__sw3 == 1u))
    {
      l_v = coax_p_garden_box_vertex(l_id);
    }
    else
    {
      if ((l__sw3 == 2u))
      {
        l_v = coax_p_garden_sphere_vertex(l_id);
      }
      else
      {
        if ((l__sw3 == 3u))
        {
          l_v = coax_p_garden_cylinder_vertex(l_id);
        }
        else
        {
          l_v = coax_p_garden_cone_vertex(l_id);
        }
      }
    }
  }
  l_p = coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f);
  l_w_17 = coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p));
  l_c0 = coax_p_v3(l_m.x.x, l_m.y.x, l_m.z.x);
  l_c1 = coax_p_v3(l_m.x.y, l_m.y.y, l_m.z.y);
  l_c2 = coax_p_v3(l_m.x.z, l_m.y.z, l_m.z.z);
  l_normal_21 = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c1, l_c2), l_v.y.x), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c2, l_c0), l_v.y.y)), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c0, l_c1), l_v.y.z));
  l_o.x = coax_p_garden_clip(l_w_17);
  l_o.y = l_w_17;
  l_o.z = coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_normal_21, coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  l_o.w = coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z);
  l_o.f4 = (coax_p_wg_u32_f(l_m.w.w) / 8u);
  return l_o;
}
fn coax_p_fx_vs(p__vid4: u32, p__iid5: u32, p_in_: coax_t15) -> coax_t16 {
  var l__vid4: u32 = p__vid4;
  var l__iid5: u32 = p__iid5;
  var l_in_: coax_t15 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t16;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t16();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_up: f32;
  var l_sky: vec3f;
  l_up = coax_p_clampf((((coax_at_u(i32(7i)) - l_pos.y) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f)) * 0x1.99999ap0f), 0.0f, 0x1.000000p0f);
  l_sky = coax_p_wg_mix_f3f3f(coax_p_GARDEN_HORIZON(), coax_p_GARDEN_ZENITH(), coax_pow_bg_vs(l_up, 0x1.666666p-1f));
  return coax_p_wg_make_f4_f3f(coax_p_garden_plates(coax_p_v2(l_pos.x, l_pos.y), coax_p_garden_encode(l_sky)), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t13) -> vec4f {
  var l_s: coax_t13 = p_s;
  var l_n: vec3f;
  var l_base: vec3f;
  var l_xz: vec2f;
  var l_shadow: f32;
  var l_broad: f32;
  var l_fine: f32;
  var l_d: f32;
  var l_half: f32;
  var l_edge: f32;
  var l_grit: f32;
  var l_dirt: vec3f;
  var l_sun: vec3f;
  var l_lambert: f32;
  var l_sky: vec3f;
  var l_c: vec3f;
  var l_d_17: f32;
  var l_fog: f32;
  l_n = coax_p_wg_normalize3(l_s.z);
  l_base = l_s.w;
  l_xz = coax_p_v2(l_s.y.x, l_s.y.z);
  l_shadow = 0x1.000000p0f;
  if ((l_s.f4 == 99u))
  {
    l_broad = coax_p_garden_noise(coax_p_wg_mul_f2f(l_xz, 0x1.0a3d70p-3f));
    l_fine = coax_p_garden_noise(coax_p_wg_mul_f2f(l_xz, 0x1.59999ap1f));
    l_base = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(0x1.eb851ep-3f, 0x1.b851ecp-2f, 0x1.333334p-3f), coax_p_v3(0x1.851eb8p-2f, 0x1.28f5c2p-1f, 0x1.c28f5cp-3f), l_broad), (0x1.c28f5cp-1f + (0x1.99999ap-3f * l_fine)));
    l_d = coax_p_garden_path_distance(l_xz);
    l_half = coax_at_u(i32(229i));
    l_edge = (0x1.000000p0f - coax_p_smoothstep((l_half - 0x1.eb851ep-4f), (l_half + 0x1.eb851ep-4f), l_d));
    if ((l_edge > 0.0f))
    {
      l_grit = ((coax_p_garden_noise(coax_p_wg_mul_f2f(l_xz, 0x1.400000p2f)) * 0x1.000000p-1f) + (coax_p_garden_noise(coax_p_wg_mul_f2f(l_xz, 0x1.a00000p3f)) * 0x1.000000p-1f));
      l_dirt = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.19999ap-1f, 0x1.d70a3ep-2f, 0x1.51eb86p-2f), coax_p_v3(0x1.5c28f6p-1f, 0x1.333334p-1f, 0x1.d70a3ep-2f), l_grit);
      l_base = coax_p_wg_mix_f3f3f(l_base, l_dirt, l_edge);
    }
    l_shadow = coax_p_garden_shade_shadows(l_xz);
  }
  else
  {
    if ((l_s.f4 == 1u))
    {
      l_base = coax_p_wg_mul_f3f(l_base, (0x1.99999ap-1f + (0x1.666666p-2f * coax_p_garden_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.x, l_s.y.y), 0x1.8cccccp1f), coax_p_wg_mul_f2f(coax_p_v2(l_s.y.z, l_s.y.x), 0x1.266666p1f))))));
    }
    else
    {
      if ((l_s.f4 == 2u))
      {
        l_base = coax_p_wg_mul_f3f(l_base, (0x1.ccccccp-1f + (0x1.333334p-3f * coax_p_garden_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.x, l_s.y.z), 0x1.200000p3f), (l_s.y.y * 0x1.c00000p2f))))));
      }
      else
      {
        if ((l_s.f4 == 4u))
        {
          l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_v3(0x1.b33334p-1f, 0x1.d70a3ep-1f, 0x1.e66666p-1f), (0x1.000000p-2f * coax_p_garden_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_xz, 0x1.800000p0f), coax_p_v2((coax_at_u(i32(2i)) * 0x1.99999ap-3f), 0.0f)))));
        }
      }
    }
  }
  l_sun = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))));
  l_lambert = coax_p_fmax(coax_p_wg_dot3(l_n, l_sun), 0.0f);
  l_sky = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.5c28f6p-2f, 0x1.3d70a4p-2f, 0x1.eb851ep-3f), coax_p_v3(0x1.1eb852p-1f, 0x1.51eb86p-1f, 0x1.99999ap-1f), ((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f));
  l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(l_base, coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_sky, 0x1.3d70a4p-1f), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.e147aep-1f, 0x1.a3d70ap-1f), l_lambert), 0x1.ccccccp-1f), l_shadow))), coax_p_mix(0x1.800000p-1f, 0x1.000000p0f, l_shadow));
  l_d_17 = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y));
  l_fog = (0x1.000000p0f - exp(((-coax_p_fmax((l_d_17 - 0x1.600000p4f), 0.0f)) * 0x1.cac084p-7f)));
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_GARDEN_HORIZON(), coax_p_clampf(l_fog, 0.0f, 0x1.ccccccp-1f));
  return coax_p_wg_make_f4_f3f(coax_p_garden_plates(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_garden_encode(l_c)), 0x1.000000p0f);
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
fn coax_p_garden_quad(p_v: u32) -> vec2f {
  var l_v: u32 = p_v;
  var l_q: array<vec2f, 6>;
  l_q = array<vec2f, 6>(coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0x1.000000p0f));
  return l_q[i32(l_v)];
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
fn coax_p_garden_height(p_x: i32, p_z: i32) -> f32 {
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
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_garden_clip(p_w: vec3f) -> vec4f {
  var l_w: vec3f = p_w;
  var l_p: vec4f;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  return coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_p), coax_p_wg_dot4(coax_p_wg_at4(12i), l_p), coax_p_wg_dot4(coax_p_wg_at4(16i), l_p), coax_p_wg_dot4(coax_p_wg_at4(20i), l_p));
}
fn coax_p_garden_box_vertex(p_id: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_f: u32;
  var l_c: vec2f;
  var l_n: vec3f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l__sw2: u32;
  l_f = (l_id / 6u);
  l_c = coax_p_wg_sub_f2f2(coax_p_garden_quad((l_id % 6u)), coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f));
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
  return coax_t14(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_n, 0x1.000000p-1f), coax_p_wg_mul_f3f(l_a, l_c.x)), coax_p_wg_mul_f3f(l_b, l_c.y)), l_n);
}
fn coax_p_garden_sphere_vertex(p_id: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_q: u32;
  var l_c: vec2f;
  var l_lon: f32;
  var l_lat: f32;
  var l_n: vec3f;
  l_q = (l_id / 6u);
  l_c = coax_p_garden_quad((l_id % 6u));
  l_lon = (((f32((l_q % 16u)) + l_c.x) / 0x1.000000p4f) * 0x1.921fb6p2f);
  l_lat = (((f32((l_q / 16u)) + l_c.y) / 0x1.400000p3f) * 0x1.921fb6p1f);
  l_n = coax_p_v3((sin(l_lat) * cos(l_lon)), cos(l_lat), (sin(l_lat) * sin(l_lon)));
  return coax_t14(coax_p_wg_mul_f3f(l_n, 0x1.000000p-1f), l_n);
}
fn coax_p_garden_cylinder_vertex(p_id: u32) -> coax_t14 {
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
    l_c = coax_p_garden_quad((l_id % 6u));
    l_a = (((f32((l_id / 6u)) + l_c.x) / 0x1.000000p4f) * 0x1.921fb6p2f);
    l_n = coax_p_v3(cos(l_a), 0.0f, sin(l_a));
    return coax_t14(coax_p_v3((l_n.x * 0x1.000000p-1f), (l_c.y - 0x1.000000p-1f), (l_n.z * 0x1.000000p-1f)), l_n);
  }
  l_t = ((l_id - 96u) / 3u);
  l_k = ((l_id - 96u) % 3u);
  l_top = (l_t < 16u);
  l_y = select((-0x1.000000p-1f), 0x1.000000p-1f, l_top);
  l_n_8 = coax_p_v3(0.0f, select((-0x1.000000p0f), 0x1.000000p0f, l_top), 0.0f);
  if ((l_k == 0u))
  {
    return coax_t14(coax_p_v3(0.0f, l_y, 0.0f), l_n_8);
  }
  l_a_9 = (((f32((l_t % 16u)) + f32((l_k - 1u))) / 0x1.000000p4f) * 0x1.921fb6p2f);
  return coax_t14(coax_p_v3((cos(l_a_9) * 0x1.000000p-1f), l_y, (sin(l_a_9) * 0x1.000000p-1f)), l_n_8);
}
fn coax_p_garden_cone_vertex(p_id: u32) -> coax_t14 {
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
      return coax_t14(coax_p_v3(0.0f, 0x1.000000p-1f, 0.0f), coax_p_wg_normalize3(coax_p_v3(cos(l_mid), 0x1.000000p-1f, sin(l_mid))));
    }
    l_a = (((f32(l_t) + f32((l_k - 1u))) / 0x1.000000p4f) * 0x1.921fb6p2f);
    return coax_t14(coax_p_v3((cos(l_a) * 0x1.000000p-1f), (-0x1.000000p-1f), (sin(l_a) * 0x1.000000p-1f)), coax_p_wg_normalize3(coax_p_v3(cos(l_a), 0x1.000000p-1f, sin(l_a))));
  }
  l_n = coax_p_v3(0.0f, (-0x1.000000p0f), 0.0f);
  if ((l_k == 0u))
  {
    return coax_t14(coax_p_v3(0.0f, (-0x1.000000p-1f), 0.0f), l_n);
  }
  l_a_6 = (((f32(l_t) + f32((l_k - 1u))) / 0x1.000000p4f) * 0x1.921fb6p2f);
  return coax_t14(coax_p_v3((cos(l_a_6) * 0x1.000000p-1f), (-0x1.000000p-1f), (sin(l_a_6) * 0x1.000000p-1f)), l_n);
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return coax_p_dot4(l_a, l_b);
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
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
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_b > l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_clampf(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return coax_p_fmin(coax_p_fmax(l_x, l_lo), l_hi);
}
fn coax_p_GARDEN_HORIZON() -> vec3f {
  return coax_p_v3(0x1.99999ap-1f, 0x1.b851ecp-1f, 0x1.ccccccp-1f);
}
fn coax_p_GARDEN_ZENITH() -> vec3f {
  return coax_p_v3(0x1.70a3d8p-2f, 0x1.1eb852p-1f, 0x1.a3d70ap-1f);
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_garden_encode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_wg_pow_f3f3(coax_p_wg_clamp_f3f3f3(l_c, coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), coax_p_wg_make_f3_f((0x1.000000p0f / 0x1.19999ap1f)));
}
fn coax_p_garden_plates(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_k: i32;
  l_c = l_under;
  {
    l_k = 0i;
    while ((l_k < 12i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_c = coax_p_garden_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 232i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 232i)));
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
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_garden_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_garden_hash(l_i), coax_p_garden_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_garden_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_garden_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
}
fn coax_p_garden_path_distance(p_p: vec2f) -> f32 {
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
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_garden_shade_shadows(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_s: f32;
  var l_i: i32;
  var l_b: vec4f;
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
      l_d = (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_b.x, l_b.y))) / l_b.z);
      l_s = (l_s * (0x1.000000p0f - (l_b.w * (0x1.000000p0f - coax_p_smoothstep(0x1.666666p-2f, 0x1.000000p0f, l_d)))));
      l_i = (l_i + 1i);
    }
  }
  return l_s;
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_dot3(l_a, l_b);
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
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
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(coax_p_dot2(l_a, l_a));
}
fn coax_p_wg_clamp_i(p_x: i32, p_lo: i32, p_hi: i32) -> i32 {
  var l_x: i32 = p_x;
  var l_lo: i32 = p_lo;
  var l_hi: i32 = p_hi;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_lo), l_hi);
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return ((((l_a.x * l_b.x) + (l_a.y * l_b.y)) + (l_a.z * l_b.z)) + (l_a.w * l_b.w));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
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
fn coax_p_wg_clamp_f3f3f3(p_p0: vec3f, p_p1: vec3f, p_p2: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: vec3f = p_p2;
  return coax_p_v3(coax_p_clampf(l_p0.x, l_p1.x, l_p2.x), coax_p_clampf(l_p0.y, l_p1.y, l_p2.y), coax_p_clampf(l_p0.z, l_p1.z, l_p2.z));
}
fn coax_p_wg_make_f3_f(p_p0: f32) -> vec3f {
  var l_p0: f32 = p_p0;
  return coax_p_v3(l_p0, l_p0, l_p0);
}
fn coax_p_wg_pow_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_pow_bg_vs(l_p0.x, l_p1.x), coax_pow_bg_vs(l_p0.y, l_p1.y), coax_pow_bg_vs(l_p0.z, l_p1.z));
}
fn coax_p_garden_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
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
fn coax_p_garden_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_dot2(l_a, l_b);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return (((l_a.x * l_b.x) + (l_a.y * l_b.y)) + (l_a.z * l_b.z));
}
fn coax_p_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return ((l_a.x * l_b.x) + (l_a.y * l_b.y));
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
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(coax_p_dot3(l_a, l_a));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
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
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_13 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_13(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
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
fn scene_fs(coax_in: coax_io_13) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t13(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_16) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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