// ---- generated: apps/openagents-web/gpu/deadgears.coax (the module) by coax wgsl; do not edit
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
  f4: f32,
}
struct coax_io_13 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
  @location(3) f4: f32,
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 171>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> objects: array<coax_t10>;
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
  var l_m: coax_t10;
  var l_v: coax_t14;
  var l_f: u32;
  var l_c: vec2f;
  var l_n: vec3f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l__sw2: u32;
  var l_p: vec4f;
  var l_w: vec3f;
  var l_c0: vec3f;
  var l_c1: vec3f;
  var l_c2: vec3f;
  var l_normal: vec3f;
  var l_wp: vec4f;
  l_o = coax_t13();
  l_m = objects[i32(l_instance)];
  if ((l_instance < 96u))
  {
    l_v = coax_p_deadgears_round_vertex(l_m, l_id);
    l_o.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f)), coax_p_wg_dot4(coax_p_wg_at4(12i), coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f)), coax_p_wg_dot4(coax_p_wg_at4(16i), coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f)), coax_p_wg_dot4(coax_p_wg_at4(20i), coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f)));
    l_o.y = l_v.x;
    l_o.z = l_v.y;
    l_o.w = coax_p_wg_pow_f3f3(coax_p_wg_max_f3f3(coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z), coax_p_v3(0.0f, 0.0f, 0.0f)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f));
    l_o.f4 = l_m.w.w;
    return l_o;
  }
  l_f = (l_id / 6u);
  l_c = coax_p_wg_sub_f2f2(coax_p_deadgears_quad((l_id % 6u)), coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f));
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
        l_a = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
        l_b = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
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
            l_a = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
            l_b = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
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
              l_a = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
              l_b = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
            }
          }
        }
      }
    }
  }
  l_p = coax_p_wg_make_f4_f3f(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_n, 0x1.000000p-1f), coax_p_wg_mul_f3f(l_a, l_c.x)), coax_p_wg_mul_f3f(l_b, l_c.y)), 0x1.000000p0f);
  l_w = coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p));
  l_c0 = coax_p_v3(l_m.x.x, l_m.y.x, l_m.z.x);
  l_c1 = coax_p_v3(l_m.x.y, l_m.y.y, l_m.z.y);
  l_c2 = coax_p_v3(l_m.x.z, l_m.y.z, l_m.z.z);
  l_normal = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c1, l_c2), l_n.x), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c2, l_c0), l_n.y)), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c0, l_c1), l_n.z));
  l_wp = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  l_o.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(12i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(16i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(20i), l_wp));
  l_o.y = l_w;
  l_o.z = coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_normal, coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  l_o.w = coax_p_wg_pow_f3f3(coax_p_wg_max_f3f3(coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z), coax_p_v3(0.0f, 0.0f, 0.0f)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f));
  l_o.f4 = l_m.w.w;
  return l_o;
}
fn coax_p_fx_vs(p__vid3: u32, p__iid4: u32, p_in_: coax_t15) -> coax_t16 {
  var l__vid3: u32 = p__vid3;
  var l__iid4: u32 = p__iid4;
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
  var l_cloud: f32;
  l_up = coax_p_clampf((((coax_at_u(i32(7i)) - l_pos.y) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f)) * 0x1.ccccccp0f), 0.0f, 0x1.000000p0f);
  l_sky = coax_p_wg_mix_f3f3f(coax_p_v3(coax_at_u(i32(48i)), coax_at_u(i32(49i)), coax_at_u(i32(50i))), coax_p_v3(coax_at_u(i32(44i)), coax_at_u(i32(45i)), coax_at_u(i32(46i))), coax_pow_bg_vs(l_up, 0x1.333334p-1f));
  l_sky = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(l_sky, l_sky), 0x1.666666p0f);
  l_cloud = (coax_p_deadgears_noise(coax_p_v2((((l_pos.x / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f)) * 0x1.800000p1f) + (coax_at_u(i32(2i)) * 0x1.47ae14p-7f)), (l_up * 0x1.c00000p2f))) * coax_p_deadgears_noise(coax_p_v2(((l_pos.x / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f)) * 0x1.200000p3f), (l_up * 0x1.300000p4f))));
  l_sky = coax_p_wg_mix_f3f3f(l_sky, coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(48i)), coax_at_u(i32(49i)), coax_at_u(i32(50i))), 0x1.19999ap0f), ((l_cloud * 0x1.666666p-2f) * (0x1.000000p0f - (l_up * 0x1.000000p-1f))));
  return coax_p_wg_make_f4_f3f(coax_p_deadgears_plates(coax_p_v2(l_pos.x, l_pos.y), coax_p_deadgears_encode(coax_p_deadgears_tone(coax_p_wg_mul_f3f(l_sky, 0x1.ccccccp-1f)))), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t13) -> vec4f {
  var l_s: coax_t13 = p_s;
  var l_n: vec3f;
  var l_base: vec3f;
  var l_c: vec3f;
  var l_sun: vec3f;
  var l_lit: f32;
  var l_ambient: vec3f;
  var l_occlusion: f32;
  var l_light: vec3f;
  var l_count: i32;
  var l_i: i32;
  var l_l: vec4f;
  var l_to: vec3f;
  var l_d: f32;
  var l_fall: f32;
  var l_wrap: f32;
  var l_c_17: vec3f;
  var l_d_18: f32;
  var l_fog: f32;
  l_n = coax_p_wg_normalize3(l_s.z);
  l_base = l_s.w;
  if ((l_s.f4 < 0.0f))
  {
    l_c = coax_p_wg_mul_f3f(l_base, 0x1.99999ap0f);
    return coax_p_wg_make_f4_f3f(coax_p_deadgears_plates(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_deadgears_hurt(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_deadgears_encode(coax_p_deadgears_tone(l_c)))), 0x1.000000p0f);
  }
  if (((l_s.y.y < 0x1.47ae14p-6f) && (l_n.y > 0x1.000000p-1f)))
  {
    l_base = coax_p_wg_mul_f3f(l_base, ((0x1.b33334p-1f + (0x1.000000p-2f * coax_p_deadgears_noise(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.x, l_s.y.z), 0x1.4cccccp0f)))) + (0x1.47ae14p-4f * coax_p_deadgears_noise(coax_p_wg_mul_f2f(coax_p_v2(l_s.y.x, l_s.y.z), 0x1.600000p3f)))));
  }
  l_sun = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))));
  l_lit = ((coax_p_deadgears_sunlight(l_s.y, l_n) * coax_p_fmax(coax_p_wg_dot3(l_n, l_sun), 0.0f)) * coax_at_u(i32(31i)));
  l_ambient = coax_p_wg_mix_f3f3f(coax_p_v3(coax_at_u(i32(40i)), coax_at_u(i32(41i)), coax_at_u(i32(42i))), coax_p_v3(coax_at_u(i32(36i)), coax_at_u(i32(37i)), coax_at_u(i32(38i))), ((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f));
  l_occlusion = coax_p_deadgears_blobs(l_s.y);
  l_light = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_ambient, l_occlusion), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(32i)), coax_at_u(i32(33i)), coax_at_u(i32(34i))), l_lit), coax_p_mix(0x1.666666p-2f, 0x1.000000p0f, l_occlusion)));
  l_count = coax_p_wg_i32_f(coax_at_u(i32(56i)));
  {
    l_i = 0i;
    while ((l_i < 40i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_l = coax_p_wg_at4(((i32((l_i * 2i)) * 4i) + 60i));
      l_to = coax_p_wg_sub_f3f3(coax_p_v3(l_l.x, l_l.y, l_l.z), l_s.y);
      l_d = coax_p_wg_length3(l_to);
      if ((l_d >= l_l.w))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_fall = (0x1.000000p0f - (l_d / l_l.w));
      l_wrap = ((coax_p_fmax(coax_p_wg_dot3(l_n, coax_p_wg_div_f3f(l_to, coax_p_fmax(l_d, 0x1.0624dep-10f))), 0.0f) * 0x1.b33334p-1f) + 0x1.333334p-3f);
      l_light = coax_p_wg_add_f3f3(l_light, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(((i32(((l_i * 2i) + 1i)) * 4i) + 60i))), coax_at_u(i32(((i32(((l_i * 2i) + 1i)) * 4i) + 61i))), coax_at_u(i32(((i32(((l_i * 2i) + 1i)) * 4i) + 62i)))), l_fall), l_fall), l_wrap), 0x1.666666p0f));
      l_i = (l_i + 1i);
    }
  }
  l_c_17 = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f3(l_base, l_light), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_base, l_s.f4), 0x1.4cccccp0f));
  l_d_18 = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y));
  l_fog = (0x1.000000p0f - exp(((-coax_p_fmax((l_d_18 - 0x1.800000p2f), 0.0f)) * coax_at_u(i32(27i)))));
  l_c_17 = coax_p_wg_mix_f3f3f(l_c_17, coax_p_v3(coax_at_u(i32(52i)), coax_at_u(i32(53i)), coax_at_u(i32(54i))), coax_p_clampf(l_fog, 0.0f, 0x1.b33334p-1f));
  return coax_p_wg_make_f4_f3f(coax_p_deadgears_plates(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_deadgears_hurt(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_deadgears_encode(coax_p_deadgears_tone(coax_p_wg_mul_f3f(l_c_17, 0x1.266666p0f))))), 0x1.000000p0f);
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
fn coax_p_deadgears_round_vertex(p_m: coax_t10, p_id: u32) -> coax_t14 {
  var l_m: coax_t10 = p_m;
  var l_id: u32 = p_id;
  var l_q: u32;
  var l_c: vec2f;
  var l_row: u32;
  var l_k: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_axis: vec3f;
  var l_side: vec3f;
  var l_other: vec3f;
  var l_out: vec3f;
  var l_top: bool;
  var l_step: f32;
  var l_phi: f32;
  var l_n: vec3f;
  var l_r: f32;
  l_q = (l_id / 6u);
  l_c = coax_p_deadgears_quad((l_id % 6u));
  l_row = ((l_q / 8u) + coax_p_wg_u32_f(l_c.y));
  l_k = ((f32(((l_q % 8u) + coax_p_wg_u32_f(l_c.x))) / f32(8u)) * 0x1.921fb6p2f);
  l_a = coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z);
  l_b = coax_p_v3(l_m.y.x, l_m.y.y, l_m.y.z);
  l_axis = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_sub_f3f3(l_b, l_a), coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  l_side = coax_p_wg_normalize3(coax_p_wg_cross3(l_axis, select(coax_p_v3(0x1.000000p0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (abs(l_axis.y) < 0x1.ccccccp-1f))));
  l_other = coax_p_wg_cross3(l_axis, l_side);
  l_out = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_side, cos(l_k)), coax_p_wg_mul_f3f(l_other, sin(l_k)));
  l_top = (l_row > 2u);
  l_step = ((f32(select(l_row, ((l_row - 2u) - 1u), l_top)) / f32(2u)) * 0x1.921fb4p0f);
  l_phi = select((l_step - 0x1.921fb4p0f), l_step, l_top);
  l_n = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_out, cos(l_phi)), coax_p_wg_mul_f3f(l_axis, sin(l_phi)));
  l_r = select(l_m.x.w, l_m.y.w, l_top);
  return coax_t14(coax_p_wg_add_f3f3(select(l_a, l_b, l_top), coax_p_wg_mul_f3f(l_n, l_r)), l_n);
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
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_max_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y), coax_p_fmax(l_p0.z, l_p1.z));
}
fn coax_p_wg_pow_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_pow_bg_vs(l_p0.x, l_p1.x), coax_pow_bg_vs(l_p0.y, l_p1.y), coax_pow_bg_vs(l_p0.z, l_p1.z));
}
fn coax_p_deadgears_quad(p_v: u32) -> vec2f {
  var l_v: u32 = p_v;
  var l_q: array<vec2f, 6>;
  l_q = array<vec2f, 6>(coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0x1.000000p0f));
  return l_q[i32(l_v)];
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
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
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
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
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_deadgears_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_deadgears_hash(l_i), coax_p_deadgears_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_deadgears_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_deadgears_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
}
fn coax_p_deadgears_tone(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_x: vec3f;
  l_x = coax_p_wg_max_f3f3(l_c, coax_p_v3(0.0f, 0.0f, 0.0f));
  return coax_p_wg_clamp_f3f3f3(coax_p_wg_div_f3f3(coax_p_wg_mul_f3f3(l_x, coax_p_wg_add_f3f(coax_p_wg_mul_ff3(0x1.4147aep1f, l_x), 0x1.eb851ep-6f)), coax_p_wg_add_f3f(coax_p_wg_mul_f3f3(l_x, coax_p_wg_add_f3f(coax_p_wg_mul_ff3(0x1.370a3ep1f, l_x), 0x1.2e147ap-1f)), 0x1.1eb852p-3f)), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f));
}
fn coax_p_deadgears_encode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_wg_pow_f3f3(coax_p_wg_clamp_f3f3f3(l_c, coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), coax_p_wg_make_f3_f((0x1.000000p0f / 0x1.19999ap1f)));
}
fn coax_p_deadgears_plates(p_p: vec2f, p_under: vec3f) -> vec3f {
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
      l_c = coax_p_deadgears_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 556i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 556i)));
      l_k = (l_k + 1i);
    }
  }
  return l_c;
}
fn coax_p_deadgears_hurt(p_p: vec2f, p_c: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_c: vec3f = p_c;
  var l_q: vec2f;
  var l_edge: f32;
  if ((coax_at_u(i32(59i)) <= 0.0f))
  {
    return l_c;
  }
  l_q = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f(coax_p_wg_div_f2f2(l_p, coax_p_wg_at2(0i)), 0x1.000000p-1f), 0x1.000000p1f);
  l_edge = coax_p_smoothstep(0x1.19999ap-1f, 0x1.59999ap0f, coax_p_wg_length2(l_q));
  return coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.19999ap-1f, 0x1.47ae14p-5f, 0x1.eb851ep-6f), ((l_edge * coax_at_u(i32(59i))) * 0x1.333334p-1f));
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_deadgears_sunlight(p_p: vec3f, p_n: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_s: vec3f;
  var l_o: vec3f;
  var l_v: f32;
  var l_t: f32;
  var l_wall: f32;
  var l_t_8: f32;
  var l_t_9: f32;
  var l_q: vec3f;
  var l_rib: f32;
  var l_count: i32;
  var l_i: i32;
  l_s = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))));
  if ((l_s.y <= 0x1.47ae14p-7f))
  {
    return 0.0f;
  }
  l_o = coax_p_wg_add_f3f3(l_p, coax_p_wg_mul_f3f(l_n, 0x1.eb851ep-6f));
  l_v = 0x1.000000p0f;
  if ((abs(l_s.x) > 0x1.0624dep-10f))
  {
    l_t = (((coax_p_wg_sign(l_s.x) * 0x1.f9999ap3f) - l_o.x) / l_s.x);
    if (((l_t > 0x1.47ae14p-6f) && ((l_o.y + (l_s.y * l_t)) < 0x1.666666p2f)))
    {
      return 0.0f;
    }
  }
  if ((abs(l_s.z) > 0x1.0624dep-10f))
  {
    l_wall = select((-0x1.7cccccp4f), 0x1.b9999ap3f, (l_s.z > 0.0f));
    l_t_8 = ((l_wall - l_o.z) / l_s.z);
    if (((l_t_8 > 0x1.47ae14p-6f) && ((l_o.y + (l_s.y * l_t_8)) < 0x1.800000p2f)))
    {
      return 0.0f;
    }
  }
  if ((l_o.y < 0x1.666666p2f))
  {
    l_t_9 = ((0x1.733334p2f - l_o.y) / l_s.y);
    l_q = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_s, l_t_9));
    l_rib = (abs((coax_p_fract((((l_q.z + 0x1.600000p4f) / 0x1.000000p2f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * 0x1.000000p2f);
    l_v = (l_v * coax_p_smoothstep(0x1.99999ap-4f, 0x1.99999ap-3f, l_rib));
  }
  l_count = coax_p_wg_i32_f(coax_at_u(i32(58i)));
  {
    l_i = 0i;
    while ((l_i < 10i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      if (coax_p_deadgears_slab(l_o, l_s, coax_p_v3(coax_at_u(i32(((i32((l_i * 2i)) * 4i) + 476i))), coax_at_u(i32(((i32((l_i * 2i)) * 4i) + 477i))), coax_at_u(i32(((i32((l_i * 2i)) * 4i) + 478i)))), coax_p_v3(coax_at_u(i32(((i32(((l_i * 2i) + 1i)) * 4i) + 476i))), coax_at_u(i32(((i32(((l_i * 2i) + 1i)) * 4i) + 477i))), coax_at_u(i32(((i32(((l_i * 2i) + 1i)) * 4i) + 478i))))))
      {
        return 0.0f;
      }
      l_i = (l_i + 1i);
    }
  }
  return l_v;
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_dot3(l_a, l_b);
}
fn coax_p_deadgears_blobs(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_s: f32;
  var l_count: i32;
  var l_i: i32;
  var l_b: vec4f;
  var l_d: f32;
  if ((l_p.y > 0x1.eb851ep-4f))
  {
    return 0x1.000000p0f;
  }
  l_s = 0x1.000000p0f;
  l_count = coax_p_wg_i32_f(coax_at_u(i32(57i)));
  {
    l_i = 0i;
    while ((l_i < 24i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_b = coax_p_wg_at4(((i32(l_i) * 4i) + 380i));
      l_d = (coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_p.x, l_p.z), coax_p_v2(l_b.x, l_b.y))) / l_b.z);
      l_s = (l_s * (0x1.000000p0f - (l_b.w * (0x1.000000p0f - coax_p_smoothstep(0x1.333334p-2f, 0x1.000000p0f, l_d)))));
      l_i = (l_i + 1i);
    }
  }
  return l_s;
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
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
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(coax_p_dot3(l_a, l_a));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(coax_p_dot2(l_a, l_a));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
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
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
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
fn coax_p_deadgears_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_wg_add_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x + l_b), (l_a.y + l_b), (l_a.z + l_b));
}
fn coax_p_wg_div_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x / l_b.x), (l_a.y / l_b.y), (l_a.z / l_b.z));
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
fn coax_p_deadgears_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
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
  l_corner = (0x1.000000p2f * l_s.z);
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_d = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  if ((l_d > 0x1.000000p0f))
  {
    return l_under;
  }
  l_over = (l_s.y >= 0x1.000000p1f);
  l_fill = select(coax_p_v3(0x1.99999ap-6f, 0x1.0624dep-5f, 0x1.581062p-5f), coax_p_v3(0x1.1eb852p-4f, 0x1.70a3d8p-4f, 0x1.c28f5cp-4f), l_over);
  l_line = select(coax_p_v3(0x1.70a3d8p-2f, 0x1.ae147ap-2f, 0x1.000000p-1f), coax_p_v3(0x1.a3d70ap-1f, 0x1.c28f5cp-1f, 0x1.eb851ep-1f), (l_over || (l_s.y >= 0x1.000000p0f)));
  l_w = (0x1.000000p0f * l_s.z);
  l_inside = coax_p_clampf((0x1.000000p-1f - l_d), 0.0f, 0x1.000000p0f);
  l_edge = coax_p_clampf((((l_w * 0x1.000000p-1f) + 0x1.000000p-1f) - abs((l_d + (l_w * 0x1.000000p-1f)))), 0.0f, 0x1.000000p0f);
  l_a = coax_p_clampf(l_s.x, 0.0f, 0x1.000000p0f);
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_under, l_fill, (l_a * l_inside)), l_line, ((l_edge * coax_p_fmin(0x1.000000p0f, (l_a + 0x1.99999ap-4f))) * 0x1.b33334p-1f));
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
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
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
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_deadgears_slab(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f) -> bool {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_lo: vec3f = p_lo;
  var l_hi: vec3f = p_hi;
  var l_inv: vec3f;
  var l_t0: vec3f;
  var l_t1: vec3f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_near: f32;
  var l_far: f32;
  l_inv = coax_p_wg_div_ff3(0x1.000000p0f, l_d);
  l_t0 = coax_p_wg_mul_f3f3(coax_p_wg_sub_f3f3(l_lo, l_o), l_inv);
  l_t1 = coax_p_wg_mul_f3f3(coax_p_wg_sub_f3f3(l_hi, l_o), l_inv);
  l_a = coax_p_wg_min_f3f3(l_t0, l_t1);
  l_b = coax_p_wg_max_f3f3(l_t0, l_t1);
  l_near = coax_p_fmax(coax_p_fmax(l_a.x, l_a.y), l_a.z);
  l_far = coax_p_fmin(coax_p_fmin(l_b.x, l_b.y), l_b.z);
  return (l_far > coax_p_fmax(l_near, 0x1.47ae14p-6f));
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
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
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
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_dot2(l_a, l_b);
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
fn coax_p_wg_div_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a / l_b.x), (l_a / l_b.y), (l_a / l_b.z));
}
fn coax_p_wg_min_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmin(l_p0.x, l_p1.x), coax_p_fmin(l_p0.y, l_p1.y), coax_p_fmin(l_p0.z, l_p1.z));
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