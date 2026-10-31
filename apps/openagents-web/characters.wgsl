// ---- generated: apps/openagents-web/gpu/characters.coax (the module) by coax wgsl; do not edit
struct coax_t9 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_9 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t11 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t13 {
  x: f32,
  y: vec3f,
  z: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 478>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t9) -> coax_t11 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t9 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t11;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t11();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_ndc: vec2f;
  var l_aspect: f32;
  var l_tv: f32;
  var l_d: vec3f;
  var l_o: vec3f;
  var l_pix: f32;
  var l_c: vec3f;
  var l_far: f32;
  var l_t: f32;
  var l_hits: array<coax_t13, 6>;
  var l_count: i32;
  var l_n: i32;
  var l_f: i32;
  var l_h: coax_t13;
  var l_i: i32;
  var l_j: i32;
  var l_s: coax_t13;
  var l_i_19: i32;
  l_ndc = coax_p_v2((((l_pos.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_pos.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_aspect = (coax_at_u(i32(0i)) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f));
  l_tv = coax_at_u(i32(11i));
  l_d = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i))), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))), l_ndc.x), l_aspect), l_tv)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))), l_ndc.y), l_tv)));
  l_o = coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i)));
  l_pix = ((0x1.000000p1f * l_tv) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f));
  l_c = coax_p_characters_sky(l_d);
  l_far = 0x1.dcd650p29f;
  if ((l_d.y < (-0x1.a36e2ep-14f)))
  {
    l_t = ((-l_o.y) / l_d.y);
    l_far = l_t;
    l_c = coax_p_wg_mix_f3f3f(coax_p_characters_ground(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t)), l_t), l_c, coax_p_smoothstep(0x1.b80000p5f, 0x1.400000p6f, l_t));
  }
  l_hits = array<coax_t13, 6>();
  l_count = 0i;
  l_n = coax_p_wg_i32_f(coax_at_u(i32(4i)));
  {
    l_f = 0i;
    while ((l_f < 6i))
    {
      if ((l_f >= l_n))
      {
        break;
      }
      l_h = coax_p_characters_figure(l_f, l_o, l_d, l_pix);
      if (((l_h.z > 0.0f) && (l_h.x < l_far)))
      {
        l_hits[i32(l_count)] = l_h;
        l_count = (l_count + 1i);
      }
      l_f = (l_f + 1i);
    }
  }
  {
    l_i = 1i;
    while ((l_i < l_count))
    {
      l_j = l_i;
      while (((l_j > 0i) && (l_hits[i32((l_j - 1i))].x < l_hits[i32(l_j)].x)))
      {
        l_s = l_hits[i32(l_j)];
        l_hits[i32(l_j)] = l_hits[i32((l_j - 1i))];
        l_hits[i32((l_j - 1i))] = l_s;
        l_j = (l_j - 1i);
      }
      l_i = (l_i + 1i);
    }
  }
  {
    l_i_19 = 0i;
    while ((l_i_19 < l_count))
    {
      l_c = coax_p_wg_mix_f3f3f(l_c, l_hits[i32(l_i_19)].y, l_hits[i32(l_i_19)].z);
      l_i_19 = (l_i_19 + 1i);
    }
  }
  return coax_p_wg_make_f4_f3f(coax_p_characters_panels(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_mul_f3f(coax_p_characters_tone(l_c), coax_at_u(i32(3i)))), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t11) -> vec4f {
  var l_v: coax_t11 = p_v;
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
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_b > l_a))
  {
    return l_b;
  }
  return l_a;
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
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_characters_sky(p_d: vec3f) -> vec3f {
  var l_d: vec3f = p_d;
  var l_up: f32;
  var l_c: vec3f;
  var l_grid: vec3f;
  var l_star: f32;
  var l_dot: f32;
  var l_moon: vec3f;
  var l_m: f32;
  l_up = coax_p_clampf(l_d.y, 0.0f, 0x1.000000p0f);
  l_c = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.eb851ep-5f, 0x1.1eb852p-4f, 0x1.c28f5cp-4f), coax_p_v3(0x1.47ae14p-7f, 0x1.89374cp-7f, 0x1.1eb852p-5f), sqrt(l_up));
  l_grid = coax_p_wg_mul_f3f(l_d, 0x1.680000p6f);
  l_star = coax_p_characters_hash3(coax_p_wg_floor_f3(l_grid));
  if (((l_star > 0x1.fc6a7ep-1f) && (l_d.y > 0x1.47ae14p-5f)))
  {
    l_dot = coax_p_smoothstep(0x1.47ae14p-3f, 0.0f, coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_wg_fract_f3(l_grid), coax_p_v3(0x1.000000p-1f, 0x1.000000p-1f, 0x1.000000p-1f))));
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.19999ap-1f, 0x1.333334p-1f, 0x1.800000p-1f), l_dot), (l_star - 0x1.fc6a7ep-1f)), 0x1.e00000p6f), (0x1.666666p-1f + (0x1.333334p-2f * sin(((coax_at_u(i32(2i)) * 0x1.4cccccp0f) + (l_star * 0x1.680000p6f)))))));
  }
  l_moon = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))));
  l_m = coax_p_wg_dot3(l_d, l_moon);
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-1f, 0x1.b33334p-1f, 0x1.000000p0f), coax_p_smoothstep(0x1.ffa440p-1f, 0x1.ffcb92p-1f, l_m)), 0x1.99999ap0f), coax_p_wg_mul_f3f(coax_p_v3(0x1.eb851ep-4f, 0x1.1eb852p-3f, 0x1.c28f5cp-3f), coax_pow_bg_vs(coax_p_fmax(l_m, 0.0f), 0x1.e00000p5f))));
  return l_c;
}
fn coax_p_characters_ground(p_p: vec3f, p_dist: f32) -> vec3f {
  var l_p: vec3f = p_p;
  var l_dist: f32 = p_dist;
  var l_r: f32;
  var l_grass: vec3f;
  var l_ring: f32;
  var l_c: vec3f;
  var l_shade: f32;
  var l_n: i32;
  var l_f: i32;
  var l_b: vec4f;
  var l_size: f32;
  var l_e: f32;
  var l_fog: f32;
  l_r = coax_p_wg_length2(coax_p_v2(l_p.x, l_p.z));
  l_grass = coax_p_wg_mul_f3f(coax_p_v3(0x1.70a3d8p-5f, 0x1.1eb852p-4f, 0x1.99999ap-5f), (0x1.800000p-1f + (0x1.000000p-1f * coax_p_characters_noise3(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_p.x, l_p.z), 0x1.800000p1f), 0.0f)))));
  l_ring = (coax_p_smoothstep(0x1.333334p-2f, 0x1.c28f5cp-3f, abs((l_r - 0x1.19999ap1f))) * (0x1.666666p-1f + (0x1.000000p-1f * coax_p_characters_noise3(coax_p_wg_make_f3_f2f(coax_p_wg_mul_f2f(coax_p_v2(l_p.x, l_p.z), 0x1.c00000p3f), 0x1.000000p0f)))));
  l_c = coax_p_wg_mix_f3f3f(l_grass, coax_p_v3(0x1.99999ap-4f, 0x1.89374cp-4f, 0x1.6872b0p-4f), l_ring);
  l_c = coax_p_wg_mul_f3f3(l_c, coax_p_wg_add_ff3(0x1.ccccccp-1f, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.1eb852p-1f, 0x1.47ae14p-1f, 0x1.b851ecp-1f), coax_p_fmax(0.0f, coax_at_u(i32(25i)))), 0x1.333334p0f)));
  l_shade = 0x1.000000p0f;
  l_n = coax_p_wg_i32_f(coax_at_u(i32(4i)));
  {
    l_f = 0i;
    while ((l_f < 6i))
    {
      if ((l_f >= l_n))
      {
        break;
      }
      l_b = coax_p_wg_at4(((i32((l_f * 5i)) * 4i) + 256i));
      l_size = coax_p_fmax(coax_at_u(i32(((i32(((l_f * 5i) + 4i)) * 4i) + 258i))), 0x1.99999ap-4f);
      if ((coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_v2(l_b.x, l_b.z))) > (l_b.w + (0x1.99999ap-2f * l_size))))
      {
        {
          l_f = (l_f + 1i);
          continue;
        }
      }
      l_e = coax_p_characters_body(l_f, coax_p_wg_add_f3f3(l_p, coax_p_v3(0.0f, 0x1.47ae14p-7f, 0.0f)));
      l_shade = coax_p_fmin(l_shade, coax_p_mix(0x1.000000p-2f, 0x1.000000p0f, coax_p_smoothstep(0.0f, (0x1.851eb8p-2f * l_size), l_e)));
      l_f = (l_f + 1i);
    }
  }
  l_c = coax_p_wg_mul_f3f(l_c, l_shade);
  l_fog = (0x1.000000p0f - exp((((-l_dist) * l_dist) * 0x1.d7dbf4p-11f)));
  return coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.eb851ep-5f, 0x1.1eb852p-4f, 0x1.c28f5cp-4f), l_fog);
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
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
fn coax_p_characters_figure(p_f: i32, p_o: vec3f, p_d: vec3f, p_pix: f32) -> coax_t13 {
  var l_f: i32 = p_f;
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_pix: f32 = p_pix;
  var l_h: coax_t13;
  var l_b: vec4f;
  var l_soft: bool;
  var l_size: f32;
  var l_fuzz: f32;
  var l_oc: vec3f;
  var l_r: f32;
  var l_bb: f32;
  var l_disc: f32;
  var l_q: f32;
  var l_t: f32;
  var l_end: f32;
  var l_least: f32;
  var l_at: f32;
  var l_hit: bool;
  var l_i: i32;
  var l_p: vec3f;
  var l_e: f32;
  var l_edge: f32;
  var l_moon: vec3f;
  var l_depth: f32;
  var l_rim: f32;
  var l_reach: f32;
  var l_n: vec3f;
  var l_facing: f32;
  var l_p_30: vec3f;
  var l_wisp: f32;
  var l_cover: f32;
  var l_cover_33: f32;
  var l_p_34: vec3f;
  var l_n_35: vec3f;
  var l_reg: f32;
  var l_albedo: vec3f;
  var l_sky: f32;
  var l_key: f32;
  var l_rim_40: f32;
  var l_glow: f32;
  var l_light: vec3f;
  var l_c: vec3f;
  var l_spec: f32;
  l_h = coax_t13();
  l_h.x = 0x1.dcd650p29f;
  l_h.z = 0.0f;
  l_b = coax_p_wg_at4(((i32((l_f * 5i)) * 4i) + 256i));
  l_soft = (coax_at_u(i32(((i32(((l_f * 5i) + 1i)) * 4i) + 259i))) > 0x1.000000p-1f);
  l_size = coax_p_fmax(coax_at_u(i32(((i32(((l_f * 5i) + 4i)) * 4i) + 258i))), 0x1.99999ap-4f);
  l_fuzz = select(0.0f, (0x1.1eb852p-3f * l_size), l_soft);
  l_oc = coax_p_wg_sub_f3f3(l_o, coax_p_v3(l_b.x, l_b.y, l_b.z));
  l_r = (l_b.w + l_fuzz);
  l_bb = coax_p_wg_dot3(l_oc, l_d);
  l_disc = (((l_bb * l_bb) - coax_p_wg_dot3(l_oc, l_oc)) + (l_r * l_r));
  if ((l_disc <= 0.0f))
  {
    return l_h;
  }
  l_q = sqrt(l_disc);
  l_t = coax_p_fmax(((-l_bb) - l_q), 0.0f);
  l_end = ((-l_bb) + l_q);
  if ((l_end <= 0.0f))
  {
    return l_h;
  }
  l_least = 0x1.86a000p16f;
  l_at = l_t;
  l_hit = false;
  {
    l_i = 0i;
    while ((l_i < 56i))
    {
      l_p = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t));
      l_e = coax_p_characters_body(l_f, l_p);
      l_edge = (l_e - (l_pix * l_t));
      if ((l_edge < l_least))
      {
        l_least = l_edge;
        l_at = l_t;
      }
      if ((l_e < ((0x1.3a92a4p-11f * l_t) + 0x1.a36e2ep-12f)))
      {
        l_hit = true;
        break;
      }
      l_t = (l_t + coax_p_fmax((l_e * 0x1.ccccccp-1f), 0x1.0624dep-8f));
      if ((l_t > l_end))
      {
        break;
      }
      l_i = (l_i + 1i);
    }
  }
  l_moon = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))));
  if (l_soft)
  {
    l_depth = l_least;
    l_rim = 0.0f;
    l_reach = (l_fuzz + (l_pix * l_at));
    if (l_hit)
    {
      l_n = coax_p_characters_normal(l_f, coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t)));
      l_facing = abs(coax_p_wg_dot3(l_n, l_d));
      l_depth = (((-0x1.000000p1f) * l_reach) * l_facing);
      l_rim = (coax_pow_bg_vs((0x1.000000p0f - l_facing), 0x1.400000p1f) * (0x1.333334p-2f + (0x1.666666p-1f * coax_p_fmax(0.0f, (coax_p_wg_dot3(l_n, l_moon) + 0x1.99999ap-2f)))));
    }
    l_p_30 = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, select(l_at, l_t, l_hit)));
    l_wisp = (((coax_p_characters_noise3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_p_30, 0x1.c00000p3f), coax_p_v3(0.0f, (coax_at_u(i32(2i)) * 0x1.333334p-1f), (coax_at_u(i32(2i)) * 0x1.99999ap-3f)))) - 0x1.000000p-1f) * 0x1.ccccccp-1f) * l_fuzz);
    l_cover = (0x1.000000p0f - coax_p_smoothstep((-l_reach), l_reach, (l_depth + l_wisp)));
    if ((l_cover <= 0x1.0624dep-9f))
    {
      return l_h;
    }
    l_rim = coax_p_fmax(l_rim, ((0x1.000000p-1f * (0x1.000000p0f - l_cover)) * l_cover));
    l_h.x = select(l_at, l_t, l_hit);
    l_h.y = coax_p_wg_add_f3f3(coax_p_v3(0x1.89374cp-9f, 0x1.0624dep-8f, 0x1.cac084p-8f), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.ae147ap-2f, 0x1.0a3d70p-1f, 0x1.b33334p-1f), l_rim), 0x1.000000p-1f));
    l_h.z = (l_cover * 0x1.f5c290p-1f);
    return l_h;
  }
  if ((!l_hit))
  {
    l_cover_33 = (0x1.000000p0f - coax_p_smoothstep(0.0f, ((l_pix * l_at) * 0x1.800000p0f), l_least));
    if ((l_cover_33 <= 0x1.0624dep-10f))
    {
      return l_h;
    }
    l_t = l_at;
  }
  l_p_34 = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_t));
  l_n_35 = coax_p_characters_normal(l_f, l_p_34);
  l_reg = coax_p_characters_region(l_f, l_p_34);
  l_albedo = coax_p_characters_coat(l_f, l_p_34, l_reg);
  l_sky = (0x1.eb851ep-5f + (0x1.99999ap-5f * l_n_35.y));
  l_key = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n_35, l_moon));
  l_rim_40 = (coax_pow_bg_vs((0x1.000000p0f - abs(coax_p_wg_dot3(l_n_35, l_d))), 0x1.800000p1f) * 0x1.99999ap-2f);
  l_glow = coax_at_u(i32(((i32(((l_f * 5i) + 4i)) * 4i) + 259i)));
  l_light = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-4f, 0x1.c28f5cp-4f, 0x1.47ae14p-3f), (l_sky * 0x1.000000p3f)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.1eb852p-1f, 0x1.47ae14p-1f, 0x1.b851ecp-1f), l_key), 0x1.800000p0f)), coax_p_wg_mul_f3f(coax_p_v3(0x1.ccccccp-2f, 0x1.19999ap-1f, 0x1.b33334p-1f), l_rim_40)), coax_p_v3(l_glow, l_glow, l_glow));
  l_c = coax_p_wg_mul_f3f3(l_albedo, l_light);
  if (((l_reg > 0x1.a00000p2f) && (l_reg < 0x1.100000p3f)))
  {
    l_spec = coax_pow_bg_vs(coax_p_fmax(0.0f, coax_p_wg_dot3(coax_p_wg_reflect3(l_d, l_n_35), l_moon)), 0x1.800000p4f);
    l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-1f, 0x1.4cccccp-1f, 0x1.99999ap-1f), l_spec));
  }
  l_h.x = l_t;
  l_h.y = l_c;
  l_h.z = select((0x1.000000p0f - coax_p_smoothstep(0.0f, ((l_pix * l_t) * 0x1.800000p0f), l_least)), 0x1.000000p0f, l_hit);
  return l_h;
}
fn coax_p_characters_tone(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_wg_sub_f3f3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_wg_exp_f3(coax_p_wg_mul_f3f(coax_p_wg_neg_f3(l_c), 0x1.59999ap0f)));
}
fn coax_p_characters_panels(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_k: i32;
  l_c = l_under;
  {
    l_k = 0i;
    while ((l_k < 28i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_c = coax_p_characters_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 32i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 32i)));
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
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_wg_floor_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(floor(l_p0.x), floor(l_p0.y), floor(l_p0.z));
}
fn coax_p_characters_hash3(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_q: vec3f;
  var l_r: vec3f;
  l_q = coax_p_wg_fract_f3(coax_p_wg_mul_f3f3(l_p, coax_p_v3(0x1.a64c30p-4f, 0x1.a5e354p-4f, 0x1.8e8a72p-4f)));
  l_r = coax_p_wg_add_f3f(l_q, coax_p_wg_dot3(l_q, coax_p_wg_add_f3f(coax_p_v3(l_q.y, l_q.x, l_q.z), 0x1.0aa3d8p5f)));
  return coax_p_fract(((l_r.x + l_r.y) * l_r.z));
}
fn coax_p_wg_fract_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y), coax_p_fract(l_p0.z));
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
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_wg_make_f3_f2f(p_p0: vec2f, p_p1: f32) -> vec3f {
  var l_p0: vec2f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v3(l_p0.x, l_p0.y, l_p1);
}
fn coax_p_characters_noise3(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_i: vec3f;
  var l_f: vec3f;
  var l_w: vec3f;
  var l_a: f32;
  var l_b: f32;
  l_i = coax_p_wg_floor_f3(l_p);
  l_f = coax_p_wg_fract_f3(l_p);
  l_w = coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f3(l_f, l_f), coax_p_wg_sub_ff3(0x1.800000p1f, coax_p_wg_mul_ff3(0x1.000000p1f, l_f)));
  l_a = coax_p_mix(coax_p_mix(coax_p_characters_hash3(l_i), coax_p_characters_hash3(coax_p_wg_add_f3f3(l_i, coax_p_v3(0x1.000000p0f, 0.0f, 0.0f))), l_w.x), coax_p_mix(coax_p_characters_hash3(coax_p_wg_add_f3f3(l_i, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f))), coax_p_characters_hash3(coax_p_wg_add_f3f3(l_i, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0.0f))), l_w.x), l_w.y);
  l_b = coax_p_mix(coax_p_mix(coax_p_characters_hash3(coax_p_wg_add_f3f3(l_i, coax_p_v3(0.0f, 0.0f, 0x1.000000p0f))), coax_p_characters_hash3(coax_p_wg_add_f3f3(l_i, coax_p_v3(0x1.000000p0f, 0.0f, 0x1.000000p0f))), l_w.x), coax_p_mix(coax_p_characters_hash3(coax_p_wg_add_f3f3(l_i, coax_p_v3(0.0f, 0x1.000000p0f, 0x1.000000p0f))), coax_p_characters_hash3(coax_p_wg_add_f3f3(l_i, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f))), l_w.x), l_w.y);
  return coax_p_mix(l_a, l_b, l_w.z);
}
fn coax_p_wg_add_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a + l_b.x), (l_a + l_b.y), (l_a + l_b.z));
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
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
fn coax_p_characters_body(p_f: i32, p_p: vec3f) -> f32 {
  var l_f: i32 = p_f;
  var l_p: vec3f = p_p;
  var l_info: vec4f;
  var l_n: i32;
  var l_k: f32;
  var l_d: f32;
  var l_i: i32;
  var l_o: i32;
  l_info = coax_p_wg_at4(((i32(((l_f * 5i) + 4i)) * 4i) + 256i));
  l_n = coax_p_wg_i32_f(l_info.x);
  l_k = coax_p_fmax(l_info.y, 0x1.0624dep-10f);
  l_d = 0x1.86a000p16f;
  {
    l_i = 0i;
    while ((l_i < 32i))
    {
      if ((l_i >= l_n))
      {
        break;
      }
      l_o = (((l_f * 32i) + l_i) * 2i);
      l_d = coax_p_characters_smin(l_d, coax_p_characters_cone(l_p, coax_p_wg_at4(((i32(l_o) * 4i) + 376i)), coax_p_wg_at4(((i32((l_o + 1i)) * 4i) + 376i))), l_k);
      l_i = (l_i + 1i);
    }
  }
  return l_d;
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
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
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_characters_normal(p_f: i32, p_p: vec3f) -> vec3f {
  var l_f: i32 = p_f;
  var l_p: vec3f = p_p;
  var l_e: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_c: vec3f;
  var l_d: vec3f;
  l_e = 0x1.0624dep-9f;
  l_a = coax_p_v3(0x1.000000p0f, (-0x1.000000p0f), (-0x1.000000p0f));
  l_b = coax_p_v3((-0x1.000000p0f), (-0x1.000000p0f), 0x1.000000p0f);
  l_c = coax_p_v3((-0x1.000000p0f), 0x1.000000p0f, (-0x1.000000p0f));
  l_d = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  return coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_a, coax_p_characters_body(l_f, coax_p_wg_add_f3f3(l_p, coax_p_wg_mul_f3f(l_a, l_e)))), coax_p_wg_mul_f3f(l_b, coax_p_characters_body(l_f, coax_p_wg_add_f3f3(l_p, coax_p_wg_mul_f3f(l_b, l_e))))), coax_p_wg_mul_f3f(l_c, coax_p_characters_body(l_f, coax_p_wg_add_f3f3(l_p, coax_p_wg_mul_f3f(l_c, l_e))))), coax_p_wg_mul_f3f(l_d, coax_p_characters_body(l_f, coax_p_wg_add_f3f3(l_p, coax_p_wg_mul_f3f(l_d, l_e))))));
}
fn coax_p_characters_region(p_f: i32, p_p: vec3f) -> f32 {
  var l_f: i32 = p_f;
  var l_p: vec3f = p_p;
  var l_n: i32;
  var l_d: f32;
  var l_r: f32;
  var l_i: i32;
  var l_o: i32;
  var l_e: f32;
  l_n = coax_p_wg_i32_f(coax_at_u(i32(((i32(((l_f * 5i) + 4i)) * 4i) + 256i))));
  l_d = 0x1.86a000p16f;
  l_r = 0.0f;
  {
    l_i = 0i;
    while ((l_i < 32i))
    {
      if ((l_i >= l_n))
      {
        break;
      }
      l_o = (((l_f * 32i) + l_i) * 2i);
      l_e = coax_p_characters_cone(l_p, coax_p_wg_at4(((i32(l_o) * 4i) + 376i)), coax_p_wg_at4(((i32((l_o + 1i)) * 4i) + 376i)));
      if ((l_e < l_d))
      {
        l_d = l_e;
        l_r = floor((coax_at_u(i32(((i32(l_o) * 4i) + 379i))) / 0x1.f40000p9f));
      }
      l_i = (l_i + 1i);
    }
  }
  return l_r;
}
fn coax_p_characters_coat(p_f: i32, p_p: vec3f, p_reg: f32) -> vec3f {
  var l_f: i32 = p_f;
  var l_p: vec3f = p_p;
  var l_reg: f32 = p_reg;
  var l_base: vec3f;
  var l_marks: vec3f;
  var l_pattern: f32;
  var l_at: vec4f;
  var l_size: f32;
  var l_c: f32;
  var l_s: f32;
  var l_w: vec3f;
  var l_l: vec3f;
  var l_m: f32;
  l_base = coax_p_v3(coax_at_u(i32(((i32(((l_f * 5i) + 1i)) * 4i) + 256i))), coax_at_u(i32(((i32(((l_f * 5i) + 1i)) * 4i) + 257i))), coax_at_u(i32(((i32(((l_f * 5i) + 1i)) * 4i) + 258i))));
  l_marks = coax_p_v3(coax_at_u(i32(((i32(((l_f * 5i) + 2i)) * 4i) + 256i))), coax_at_u(i32(((i32(((l_f * 5i) + 2i)) * 4i) + 257i))), coax_at_u(i32(((i32(((l_f * 5i) + 2i)) * 4i) + 258i))));
  l_pattern = coax_at_u(i32(((i32(((l_f * 5i) + 2i)) * 4i) + 259i)));
  l_at = coax_p_wg_at4(((i32(((l_f * 5i) + 3i)) * 4i) + 256i));
  l_size = coax_p_fmax(coax_at_u(i32(((i32(((l_f * 5i) + 4i)) * 4i) + 258i))), 0x1.99999ap-4f);
  l_c = cos(l_at.w);
  l_s = sin(l_at.w);
  l_w = coax_p_wg_sub_f3f3(l_p, coax_p_v3(l_at.x, l_at.y, l_at.z));
  l_l = coax_p_wg_div_f3f(coax_p_v3(((l_c * l_w.x) - (l_s * l_w.z)), l_w.y, ((l_s * l_w.x) + (l_c * l_w.z))), l_size);
  if (((l_reg > 0x1.a00000p2f) && (l_reg < 0x1.e00000p2f)))
  {
    return coax_p_v3(0x1.89374cp-7f, 0x1.47ae14p-7f, 0x1.47ae14p-7f);
  }
  if ((l_reg > 0x1.e00000p2f))
  {
    return coax_p_v3(0x1.47ae14p-6f, 0x1.0624dep-6f, 0x1.0624dep-6f);
  }
  l_m = 0.0f;
  if (((l_pattern > 0x1.000000p-1f) && (l_pattern < 0x1.800000p0f)))
  {
    l_m = coax_p_smoothstep(0x1.0a3d70p-1f, 0x1.28f5c2p-1f, coax_p_characters_noise3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_l, 0x1.400000p2f), coax_p_wg_make_f3_f((f32(l_f) * 0x1.c00000p2f)))));
  }
  else
  {
    if (((l_pattern > 0x1.800000p0f) && (l_pattern < 0x1.400000p1f)))
    {
      l_m = (select(0.0f, 0x1.000000p0f, ((l_reg > 0x1.800000p0f) && (l_reg < 0x1.200000p2f))) + select(0.0f, coax_p_smoothstep(0x1.333334p-2f, 0x1.eb851ep-4f, l_l.y), ((l_reg > 0x1.600000p2f) && (l_reg < 0x1.a00000p2f))));
    }
    else
    {
      if (((l_pattern > 0x1.400000p1f) && (l_pattern < 0x1.c00000p1f)))
      {
        l_m = select(0.0f, coax_p_smoothstep(0x1.666666p-2f, 0x1.333334p-1f, sin(((l_l.z * 0x1.000000p5f) + (coax_p_characters_noise3(coax_p_wg_mul_f3f(l_l, 0x1.800000p2f)) * 0x1.000000p2f)))), ((l_reg < 0x1.000000p-1f) || ((l_reg > 0x1.c00000p1f) && (l_reg < 0x1.600000p2f))));
      }
      else
      {
        if ((l_pattern > 0x1.c00000p1f))
        {
          l_m = select(0.0f, coax_p_smoothstep(0x1.5c28f6p-3f, 0x1.eb851ep-4f, l_l.y), ((l_reg > 0x1.600000p2f) && (l_reg < 0x1.a00000p2f)));
        }
      }
    }
  }
  if ((((l_reg > 0x1.800000p0f) && (l_reg < 0x1.400000p1f)) && (l_pattern < 0x1.000000p-1f)))
  {
    l_m = 0x1.666666p-2f;
  }
  return coax_p_wg_mix_f3f3f(l_base, l_marks, coax_p_clampf(l_m, 0.0f, 0x1.000000p0f));
}
fn coax_p_wg_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_reflect3(l_i, l_n);
}
fn coax_p_wg_neg_f3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_v3((-l_a.x), (-l_a.y), (-l_a.z));
}
fn coax_p_wg_exp_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(exp(l_p0.x), exp(l_p0.y), exp(l_p0.z));
}
fn coax_p_characters_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
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
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_wg_add_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x + l_b), (l_a.y + l_b), (l_a.z + l_b));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_wg_sub_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a - l_b.x), (l_a - l_b.y), (l_a - l_b.z));
}
fn coax_p_characters_cone(p_p: vec3f, p_a: vec4f, p_b: vec4f) -> f32 {
  var l_p: vec3f = p_p;
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  var l_ab: vec3f;
  var l_ap: vec3f;
  var l_h: f32;
  var l_ra: f32;
  l_ab = coax_p_wg_sub_f3f3(coax_p_v3(l_b.x, l_b.y, l_b.z), coax_p_v3(l_a.x, l_a.y, l_a.z));
  l_ap = coax_p_wg_sub_f3f3(l_p, coax_p_v3(l_a.x, l_a.y, l_a.z));
  l_h = coax_p_clampf((coax_p_wg_dot3(l_ap, l_ab) / coax_p_fmax(coax_p_wg_dot3(l_ab, l_ab), 0x1.0c6f7ap-20f)), 0.0f, 0x1.000000p0f);
  l_ra = (l_a.w - (floor((l_a.w / 0x1.f40000p9f)) * 0x1.f40000p9f));
  return (coax_p_wg_length3(coax_p_wg_sub_f3f3(l_ap, coax_p_wg_mul_f3f(l_ab, l_h))) - coax_p_mix(l_ra, l_b.w, l_h));
}
fn coax_p_characters_smin(p_a: f32, p_b: f32, p_k: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_k: f32 = p_k;
  var l_h: f32;
  l_h = (coax_p_fmax((l_k - abs((l_a - l_b))), 0.0f) / l_k);
  return (coax_p_fmin(l_a, l_b) - (((l_h * l_h) * l_k) * 0x1.000000p-2f));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_wg_make_f3_f(p_p0: f32) -> vec3f {
  var l_p0: f32 = p_p0;
  return coax_p_v3(l_p0, l_p0, l_p0);
}
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_i))));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
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
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_9) -> coax_io_11 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t9(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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