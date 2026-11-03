// ---- generated: apps/openagents-web/gpu/waterlab.coax (the module) by coax wgsl; do not edit
struct coax_t12 {
  x: vec4f,
  y: vec2f,
  z: vec3f,
}
struct coax_io_12 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec3f,
}
struct coax_t13 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_13 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t14 {
  x: vec4f,
  y: vec4f,
}
struct coax_io_14 {
  @builtin(position) x: vec4f,
  @location(0) y: vec4f,
}
struct coax_t15 {
  x: vec4f,
  y: f32,
}
struct coax_out_15 {
  @location(0) x: vec4f,
  @builtin(frag_depth) y: f32,
}
struct coax_t16 {
  x: f32,
  y: bool,
}
struct coax_t17 {
  x: i32,
  y: i32,
}
struct coax_t18 {
  x: u32,
  y: u32,
}
struct coax_t20 {
  x: u32,
  y: u32,
  z: u32,
  w: u32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 3937>;
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
fn coax_p_surface_vs(p_vid: u32, p__iid2: u32) -> coax_t12 {
  var l_vid: u32 = p_vid;
  var l__iid2: u32 = p__iid2;
  var l_n: u32;
  var l_count: u32;
  var l_half: u32;
  var l_q: u32;
  var l_corner: u32;
  var l_full: u32;
  var l_ring: u32;
  var l_l: u32;
  var l_lv: vec4f;
  var l_qx: u32;
  var l_qz: u32;
  var l_hole: u32;
  var l_hx: u32;
  var l_hz: u32;
  var l_top: u32;
  var l_band: u32;
  var l_col: u32;
  var l_rest: u32;
  var l_cx: array<u32, 6>;
  var l_cz: array<u32, 6>;
  var l_ix: i32;
  var l_iz: i32;
  var l_gx: i32;
  var l_gz: i32;
  var l_rx: i32;
  var l_rz: i32;
  var l_edge: i32;
  var l_a: f32;
  var l_p: vec2f;
  var l_t: f32;
  var l_reach: f32;
  var l_keep: f32;
  var l_taper: f32;
  var l_moved: vec2f;
  var l_w: vec3f;
  var l_o: coax_t12;
  var l_rel: vec3f;
  var l_vz: f32;
  var l_aspect: f32;
  l_n = coax_p_wg_u32_f(coax_at_u(i32(44i)));
  l_count = coax_p_wg_u32_f(coax_at_u(i32(45i)));
  l_half = (l_n / 2u);
  l_q = (l_vid / 6u);
  l_corner = (l_vid % 6u);
  l_full = (l_n * l_n);
  l_ring = (l_full - (l_half * l_half));
  l_l = 0u;
  if ((l_q >= l_full))
  {
    l_q = (l_q - l_full);
    l_l = coax_p_wg_min_u((1u + (l_q / l_ring)), (l_count - 1u));
    l_q = (l_q - ((l_l - 1u) * l_ring));
  }
  l_lv = coax_p_wg_at4(((i32(l_l) * 4i) + 48i));
  l_qx = (l_q % l_n);
  l_qz = (l_q / l_n);
  if ((l_l > 0u))
  {
    l_hole = coax_p_wg_u32_f(l_lv.w);
    l_hx = ((l_n / 4u) + (l_hole & 1u));
    l_hz = ((l_n / 4u) + (l_hole >> u32(1u)));
    l_top = (l_hz * l_n);
    if ((l_q >= l_top))
    {
      l_band = (l_q - l_top);
      if ((l_band < (l_half * l_half)))
      {
        l_col = (l_band % l_half);
        l_qx = select((l_col + l_half), l_col, (l_col < l_hx));
        l_qz = (l_hz + (l_band / l_half));
      }
      else
      {
        l_rest = (l_band - (l_half * l_half));
        l_qx = (l_rest % l_n);
        l_qz = ((l_hz + l_half) + (l_rest / l_n));
      }
    }
  }
  l_cx = array<u32, 6>(0u, 1u, 0u, 0u, 1u, 1u);
  l_cz = array<u32, 6>(0u, 0u, 1u, 1u, 0u, 1u);
  l_ix = i32((l_qx + l_cx[i32(l_corner)]));
  l_iz = i32((l_qz + l_cz[i32(l_corner)]));
  l_gx = ((coax_p_wg_i32_f(coax_p_wg_round((l_lv.x / l_lv.z))) - i32(l_half)) + l_ix);
  l_gz = ((coax_p_wg_i32_f(coax_p_wg_round((l_lv.y / l_lv.z))) - i32(l_half)) + l_iz);
  l_rx = coax_p_wg_abs_i((l_ix - i32(l_half)));
  l_rz = coax_p_wg_abs_i((l_iz - i32(l_half)));
  l_edge = coax_p_wg_max_i(l_rx, l_rz);
  l_a = 0.0f;
  if (((l_l + 1u) < l_count))
  {
    l_a = coax_p_clampf((f32((l_edge - (i32(l_half) - coax_p_wg_i32_f(coax_at_u(i32(46i)))))) / coax_at_u(i32(46i))), 0.0f, 0x1.000000p0f);
  }
  l_p = coax_p_v2(((f32(l_gx) - (l_a * f32((l_gx & 1i)))) * l_lv.z), ((f32(l_gz) - (l_a * f32((l_gz & 1i)))) * l_lv.z));
  if ((((l_l + 1u) == l_count) && (l_edge == i32(l_half))))
  {
    l_p = coax_p_wg_add_f2f2(coax_p_v2(l_lv.x, l_lv.y), coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_lv.x, l_lv.y)), (coax_at_u(i32(47i)) / (f32(l_half) * l_lv.z))));
  }
  l_t = coax_at_u(i32(2i));
  l_reach = coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(coax_at_u(i32(8i)), coax_at_u(i32(10i)))));
  l_keep = (0x1.000000p0f - coax_p_smoothstep((coax_at_u(i32(29i)) * 0x1.400000p1f), (coax_at_u(i32(29i)) * 0x1.000000p3f), l_reach));
  l_taper = coax_p_smoothstep(0.0f, 0x1.333334p-1f, coax_p_waterlab_depth_at(l_p));
  l_moved = coax_p_wg_mul_f2f(coax_p_waterlab_chop(l_p), (l_keep * l_taper));
  l_w = coax_p_v3((l_p.x + l_moved.x), (coax_at_u(i32(28i)) + ((coax_p_waterlab_sea_height(l_p, l_t) - coax_at_u(i32(28i))) * l_keep)), (l_p.y + l_moved.y));
  l_o = coax_t12();
  l_rel = coax_p_wg_sub_f3f3(l_w, coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_vz = coax_p_wg_dot3(l_rel, coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))));
  l_aspect = (coax_at_u(i32(0i)) / coax_at_u(i32(1i)));
  l_o.x = coax_p_v4((coax_p_wg_dot3(l_rel, coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)))) / (coax_at_u(i32(11i)) * l_aspect)), (coax_p_wg_dot3(l_rel, coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i)))) / coax_at_u(i32(11i))), (l_vz - 0x1.99999ap-4f), l_vz);
  l_o.y = l_p;
  l_o.z = l_w;
  return l_o;
}
fn coax_p_fx_vs(p__vid3: u32, p__iid4: u32, p_in_: coax_t13) -> coax_t14 {
  var l__vid3: u32 = p__vid3;
  var l__iid4: u32 = p__iid4;
  var l_in_: coax_t13 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_e: vec4f;
  var l_o: coax_t14;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_e = l_in_.w;
  l_o = coax_t14();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = l_c;
  return l_o;
}
fn coax_p_bg_fs(p_frag: vec4f) -> coax_t15 {
  var l_frag: vec4f = p_frag;
  var l_ndc: vec2f;
  var l_aspect: f32;
  var l_d: vec3f;
  var l_o: vec3f;
  var l_t: f32;
  var l_near: vec3f;
  var l_line: f32;
  var l_waver: vec3f;
  var l_c: vec3f;
  var l_hit: coax_t16;
  var l_c_12: vec3f;
  var l_w: vec3f;
  var l_e: f32;
  var l_n: vec3f;
  var l_wet: f32;
  l_ndc = coax_p_v2((((l_frag.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_frag.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_aspect = (coax_at_u(i32(0i)) / coax_at_u(i32(1i)));
  l_d = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))), l_ndc.x), coax_at_u(i32(11i))), l_aspect)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i))), l_ndc.y), coax_at_u(i32(11i)))));
  l_o = coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i)));
  l_t = coax_at_u(i32(2i));
  l_near = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, 0x1.99999ap-4f));
  l_line = (l_near.y - coax_p_waterlab_sea_height(coax_p_v2(l_near.x, l_near.z), l_t));
  if ((l_line < 0.0f))
  {
    l_waver = select(coax_p_wg_mul_f3f(coax_p_v3(sin(((l_frag.y * 0x1.47ae14p-6f) + (l_t * 0x1.b33334p0f))), cos(((l_frag.x * 0x1.16872cp-6f) + (l_t * 0x1.4cccccp0f))), 0.0f), 0x1.0624dep-8f), coax_p_v3(0.0f, 0.0f, 0.0f), (coax_at_u(i32(5i)) > 0x1.000000p-1f));
    l_c = coax_p_waterlab_under_view(l_o, coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(l_d, coax_p_wg_mul_ff3(l_waver.x, coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))))), coax_p_wg_mul_ff3(l_waver.y, coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i)))))), l_t);
    if ((coax_at_u(i32(5i)) < 0x1.000000p-1f))
    {
      l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.b33334p-1f, 0x1.e66666p-1f, 0x1.e66666p-1f), ((0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.89374cp-8f, (-l_line))) * 0x1.666666p-1f));
    }
    return coax_t15(coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f), 0.0f);
  }
  l_hit = coax_p_waterlab_trace(l_o, l_d, l_t);
  l_c_12 = vec3f();
  if ((l_hit.x > 0x1.7d7840p26f))
  {
    l_c_12 = coax_p_waterlab_haze(coax_p_waterlab_sky(l_d), l_d, (0x1.770000p11f * (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.99999ap-3f, l_d.y))));
  }
  else
  {
    l_w = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_hit.x));
    if (l_hit.y)
    {
      l_c_12 = coax_p_waterlab_water(coax_p_v2(l_w.x, l_w.z), l_w, l_d, l_hit.x, l_t);
    }
    else
    {
      l_e = 0x1.99999ap-5f;
      l_n = coax_p_wg_normalize3(coax_p_v3((coax_p_waterlab_ground(coax_p_wg_sub_f2f2(coax_p_v2(l_w.x, l_w.z), coax_p_v2(l_e, 0.0f))) - coax_p_waterlab_ground(coax_p_wg_add_f2f2(coax_p_v2(l_w.x, l_w.z), coax_p_v2(l_e, 0.0f)))), (0x1.000000p1f * l_e), (coax_p_waterlab_ground(coax_p_wg_sub_f2f2(coax_p_v2(l_w.x, l_w.z), coax_p_v2(0.0f, l_e))) - coax_p_waterlab_ground(coax_p_wg_add_f2f2(coax_p_v2(l_w.x, l_w.z), coax_p_v2(0.0f, l_e))))));
      l_wet = (0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-4f, 0x1.333334p-2f, (l_w.y - coax_at_u(i32(28i)))));
      l_c_12 = coax_p_waterlab_sand(coax_p_v2(l_w.x, l_w.z), l_n, l_wet);
    }
    l_c_12 = coax_p_waterlab_haze(l_c_12, l_d, l_hit.x);
  }
  return coax_t15(coax_p_v4(l_c_12.x, l_c_12.y, l_c_12.z, 0x1.000000p0f), select(coax_p_waterlab_depth_of(l_d, l_hit.x), 0x1.000000p0f, (l_hit.x > 0x1.7d7840p26f)));
}
fn coax_p_surface_fs(p_v: coax_t12) -> vec4f {
  var l_v: coax_t12 = p_v;
  var l_rel: vec3f;
  var l_dist: f32;
  var l_d: vec3f;
  var l_c: vec3f;
  l_rel = coax_p_wg_sub_f3f3(l_v.z, coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_dist = coax_p_wg_length3(l_rel);
  l_d = coax_p_wg_div_f3f(l_rel, coax_p_fmax(l_dist, 0x1.a36e2ep-14f));
  l_c = coax_p_waterlab_water(l_v.y, l_v.z, l_d, l_dist, coax_at_u(i32(2i)));
  return coax_p_wg_make_f4_f3f(coax_p_waterlab_haze(l_c, l_d, l_dist), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t14) -> vec4f {
  var l_v: coax_t14 = p_v;
  return coax_p_wg_make_f4_f3f(coax_p_v3(l_v.y.x, l_v.y.y, l_v.y.z), 0x1.000000p0f);
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
fn coax_p_wg_min_u(p_a: u32, p_b: u32) -> u32 {
  var l_a: u32 = p_a;
  var l_b: u32 = p_b;
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
fn coax_p_wg_round(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_r: f32;
  l_r = floor((l_x + 0x1.000000p-1f));
  if ((((l_r - l_x) == 0x1.000000p-1f) && (coax_p_wg_fmod(l_r, 0x1.000000p1f) != 0.0f)))
  {
    return (l_r - 0x1.000000p0f);
  }
  return l_r;
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
fn coax_p_wg_abs_i(p_x: i32) -> i32 {
  var l_x: i32 = p_x;
  if ((l_x < 0i))
  {
    return (-l_x);
  }
  return l_x;
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
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
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
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_waterlab_depth_at(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return (coax_p_waterlab_shore_at(l_p)).x;
}
fn coax_p_waterlab_chop(p_p: vec2f) -> vec2f {
  var l_p: vec2f = p_p;
  var l_q: vec2f;
  var l_f: vec2f;
  var l_i: coax_t17;
  var l_acc: vec2f;
  var l_k: i32;
  var l_o: coax_t17;
  var l_c: coax_t18;
  var l_t: u32;
  var l_w: f32;
  var l_half: u32;
  l_q = coax_p_wg_sub_f2f(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(l_p, coax_at_u(i32(29i))), f32(64u)), 0x1.000000p-1f);
  l_f = coax_p_wg_fract_f2(l_q);
  l_i = coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(l_q));
  l_acc = coax_p_v2(0.0f, 0.0f);
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_o = coax_p_i2((l_k & 1i), (l_k >> u32(1i)));
      l_c = coax_p_wg_cvt_u2i2(coax_p_wg_band_i2i2(coax_p_wg_add_i2i2(l_i, l_o), coax_p_wg_make_i2_i((i32(64u) - 1i))));
      l_t = ((l_c.y * 64u) + l_c.x);
      l_w = (select((0x1.000000p0f - l_f.x), l_f.x, (l_o.x == 1i)) * select((0x1.000000p0f - l_f.y), l_f.y, (l_o.y == 1i)));
      l_half = ((coax_p_waterlab_word((9456u + (l_t >> u32(1u)))) >> u32((16u * (l_t & 1u)))) & 65535u);
      l_acc = coax_p_wg_add_f2f2(l_acc, coax_p_wg_mul_ff2(l_w, coax_p_wg_sw_xy_f4(coax_p_wg_unpack4x8snorm(l_half))));
      l_k = (l_k + 1i);
    }
  }
  return coax_p_wg_mul_f2f(l_acc, coax_at_u(i32(43i)));
}
fn coax_p_waterlab_sea_height(p_p: vec2f, p_t: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_t: f32 = p_t;
  var l_s: vec4f;
  var l_h: f32;
  var l_taper: f32;
  var l_wave: f32;
  l_s = coax_p_waterlab_surf(l_p, l_t);
  l_h = l_s.w;
  l_taper = coax_p_smoothstep(0.0f, 0x1.333334p-1f, l_h);
  l_wave = coax_p_fmax((((coax_p_waterlab_coarse(l_p)).x * coax_p_fmin(l_s.z, 0x1.99999ap0f)) * l_taper), ((-0x1.000000p-1f) * l_h));
  return ((coax_at_u(i32(28i)) + l_wave) + l_s.x);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
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
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_waterlab_under_view(p_o: vec3f, p_d: vec3f, p_t: f32) -> vec3f {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_t: f32 = p_t;
  var l_tg: f32;
  var l_s: f32;
  var l_prev: f32;
  var l_i: i32;
  var l_step: f32;
  var l_q: vec3f;
  var l_gap: f32;
  var l_ts: f32;
  var l_amp: f32;
  var l_top: f32;
  var l_a: f32;
  var l_fa: f32;
  var l_i_16: i32;
  var l_b: f32;
  var l_q_18: vec3f;
  var l_fb: f32;
  var l_depth_eye: f32;
  var l_light: vec3f;
  var l_col: vec3f;
  var l_path: f32;
  var l_w: vec3f;
  var l_n: vec3f;
  var l_up: f32;
  var l_r: vec3f;
  var l_mirrored: vec3f;
  var l_f0: f32;
  var l_tr: f32;
  var l_w_31: vec3f;
  var l_e: f32;
  var l_n_33: vec3f;
  var l_below: f32;
  var l_c: vec3f;
  var l_tr_36: vec3f;
  l_tg = 0x1.dcd650p29f;
  l_s = 0.0f;
  l_prev = (l_o.y - coax_p_waterlab_ground(coax_p_v2(l_o.x, l_o.z)));
  {
    l_i = 0i;
    while ((l_i < 48i))
    {
      l_step = (0x1.333334p-2f + (l_s * 0x1.47ae14p-4f));
      l_q = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, (l_s + l_step)));
      l_gap = (l_q.y - coax_p_waterlab_ground(coax_p_v2(l_q.x, l_q.z)));
      if ((l_gap < 0.0f))
      {
        l_tg = (l_s + ((l_step * l_prev) / coax_p_fmax((l_prev - l_gap), 0x1.a36e2ep-14f)));
        break;
      }
      l_prev = l_gap;
      l_s = (l_s + l_step);
      if ((l_s > 0x1.e00000p6f))
      {
        break;
      }
      l_i = (l_i + 1i);
    }
  }
  l_ts = 0x1.dcd650p29f;
  if ((l_d.y > 0.0f))
  {
    l_amp = ((coax_at_u(i32(35i)) * 0x1.4cccccp0f) + 0x1.99999ap-2f);
    l_top = (((coax_at_u(i32(28i)) + l_amp) - l_o.y) / l_d.y);
    l_a = 0.0f;
    l_fa = (coax_p_waterlab_sea_height(coax_p_v2(l_o.x, l_o.z), l_t) - l_o.y);
    {
      l_i_16 = 1i;
      while ((l_i_16 <= 16i))
      {
        l_b = ((l_top * f32(l_i_16)) / 0x1.000000p4f);
        l_q_18 = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_b));
        l_fb = (coax_p_waterlab_sea_height(coax_p_v2(l_q_18.x, l_q_18.z), l_t) - l_q_18.y);
        if ((l_fb < 0.0f))
        {
          l_ts = (l_a + (((l_b - l_a) * l_fa) / coax_p_fmax((l_fa - l_fb), 0x1.4f8b58p-17f)));
          break;
        }
        l_a = l_b;
        l_fa = l_fb;
        l_i_16 = (l_i_16 + 1i);
      }
    }
  }
  l_depth_eye = coax_p_fmax((coax_at_u(i32(28i)) - l_o.y), 0.0f);
  l_light = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_WATERLAB_UNDER_SCATTER(), (0x1.19999ap-1f + (0x1.ccccccp-2f * exp(((-l_depth_eye) * 0x1.47ae14p-4f))))), (0x1.333334p-1f + (0x1.99999ap-2f * coax_at_u(i32(27i)))));
  l_col = l_light;
  l_path = 0x1.400000p7f;
  if (((l_ts < l_tg) && (l_ts < 0x1.7d7840p26f)))
  {
    l_path = l_ts;
    l_w = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_ts));
    l_n = coax_p_waterlab_surface_normal(coax_p_v2(l_w.x, l_w.z), l_t);
    l_up = coax_p_wg_dot3(l_d, l_n);
    l_r = coax_p_wg_refract3(l_d, coax_p_wg_neg_f3(l_n), 0x1.547ae2p0f);
    l_mirrored = coax_p_wg_mul_f3f(l_light, 0x1.99999ap-1f);
    if ((coax_p_wg_dot3(l_r, l_r) < 0x1.0c6f7ap-20f))
    {
      l_col = l_mirrored;
    }
    else
    {
      l_f0 = 0x1.48a6cep-6f;
      l_tr = (0x1.000000p0f - (l_f0 + ((0x1.000000p0f - l_f0) * coax_pow_bg_vs((0x1.000000p0f - coax_p_clampf(l_up, 0.0f, 0x1.000000p0f)), 0x1.400000p2f))));
      l_col = coax_p_wg_mix_f3f3f(l_mirrored, coax_p_wg_mul_f3f(coax_p_waterlab_sky(l_r), 0x1.0cccccp0f), (l_tr * coax_p_smoothstep(0.0f, 0x1.47ae14p-4f, l_up)));
      l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.99999ap-1f), coax_pow_bg_vs(coax_p_fmax(coax_p_wg_dot3(l_r, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f), 0x1.2c0000p8f)), 0x1.000000p1f), l_tr));
    }
    l_col = coax_p_wg_mix_f3f3f(l_col, coax_p_v3(0x1.ccccccp-1f, 0x1.e66666p-1f, 0x1.e147aep-1f), ((coax_p_waterlab_coarse(coax_p_v2(l_w.x, l_w.z))).y * 0x1.333334p-1f));
  }
  else
  {
    if ((l_tg < 0x1.7d7840p26f))
    {
      l_path = l_tg;
      l_w_31 = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tg));
      l_e = 0x1.99999ap-5f;
      l_n_33 = coax_p_wg_normalize3(coax_p_v3((coax_p_waterlab_ground(coax_p_wg_sub_f2f2(coax_p_v2(l_w_31.x, l_w_31.z), coax_p_v2(l_e, 0.0f))) - coax_p_waterlab_ground(coax_p_wg_add_f2f2(coax_p_v2(l_w_31.x, l_w_31.z), coax_p_v2(l_e, 0.0f)))), (0x1.000000p1f * l_e), (coax_p_waterlab_ground(coax_p_wg_sub_f2f2(coax_p_v2(l_w_31.x, l_w_31.z), coax_p_v2(0.0f, l_e))) - coax_p_waterlab_ground(coax_p_wg_add_f2f2(coax_p_v2(l_w_31.x, l_w_31.z), coax_p_v2(0.0f, l_e))))));
      l_below = coax_p_fmax((coax_at_u(i32(28i)) - l_w_31.y), 0.0f);
      l_c = coax_p_wg_mul_f3f3(coax_p_waterlab_sand(coax_p_v2(l_w_31.x, l_w_31.z), l_n_33, 0x1.000000p0f), coax_p_wg_exp_f3(coax_p_wg_mul_f3f(coax_p_wg_neg_f3(coax_p_WATERLAB_UNDER_ABSORB()), l_below)));
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.eb851ep-1f, 0x1.a3d70ap-1f), coax_p_waterlab_caustic_pattern(coax_p_wg_mul_f2f(coax_p_v2(l_w_31.x, l_w_31.z), 0x1.ccccccp-1f), l_t)), exp(((-l_below) * 0x1.333334p-3f))), coax_p_smoothstep(0.0f, 0x1.333334p-2f, l_below)), 0x1.ccccccp-1f), coax_p_fmax(l_n_33.y, 0.0f)));
      l_col = l_c;
    }
  }
  l_tr_36 = coax_p_wg_exp_f3(coax_p_wg_mul_f3f(coax_p_wg_neg_f3(coax_p_WATERLAB_UNDER_ABSORB()), l_path));
  l_col = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f3(l_col, l_tr_36), coax_p_wg_mul_f3f3(l_light, coax_p_wg_sub_f3f3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_tr_36)));
  l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_v3(0x1.e66666p-1f, 0x1.f0a3d8p-1f, 0x1.b33334p-1f), ((coax_p_waterlab_motes(l_o, l_d, coax_p_fmin(l_path, 0x1.c00000p2f), l_t) * 0x1.666666p-1f) + coax_p_waterlab_shafts(l_o, l_d, l_path, l_t))));
  return l_col;
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_waterlab_trace(p_o: vec3f, p_d: vec3f, p_t0: f32) -> coax_t16 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_t0: f32 = p_t0;
  var l_hit: coax_t16;
  var l_amp: f32;
  var l_t: f32;
  var l_prev: f32;
  var l_i: i32;
  var l_step: f32;
  var l_nt: f32;
  var l_q: vec3f;
  var l_gap: f32;
  var l_top: f32;
  var l_bottom: f32;
  var l_a: f32;
  var l_b: f32;
  var l_steps: i32;
  var l_fa: f32;
  var l_found: bool;
  var l_i_20: i32;
  var l_tb: f32;
  var l_q_22: vec3f;
  var l_fb: f32;
  var l_fb_24: f32;
  var l_i_25: i32;
  var l_tm: f32;
  var l_fm: f32;
  var l_tw: f32;
  l_hit = coax_t16(0x1.dcd650p29f, false);
  l_amp = ((coax_at_u(i32(35i)) * 0x1.4cccccp0f) + 0x1.99999ap-2f);
  l_t = 0.0f;
  l_prev = (l_o.y - coax_p_waterlab_ground(coax_p_v2(l_o.x, l_o.z)));
  {
    l_i = 0i;
    while ((l_i < 64i))
    {
      l_step = (0x1.99999ap-2f + (l_t * 0x1.eb851ep-5f));
      l_nt = (l_t + l_step);
      l_q = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_nt));
      l_gap = (l_q.y - coax_p_waterlab_ground(coax_p_v2(l_q.x, l_q.z)));
      if ((l_gap < 0.0f))
      {
        l_hit.x = (l_t + ((l_step * l_prev) / coax_p_fmax((l_prev - l_gap), 0x1.a36e2ep-14f)));
        break;
      }
      l_prev = l_gap;
      l_t = l_nt;
      if ((l_t > 0x1.c20000p9f))
      {
        break;
      }
      l_i = (l_i + 1i);
    }
  }
  if (((coax_at_u(i32(6i)) < 0x1.000000p-1f) && ((l_d.y < 0.0f) || (l_o.y < (coax_at_u(i32(28i)) + l_amp)))))
  {
    l_top = ((l_o.y - (coax_at_u(i32(28i)) + l_amp)) / coax_p_fmax((-l_d.y), 0x1.a36e2ep-14f));
    l_bottom = ((l_o.y - (coax_at_u(i32(28i)) - l_amp)) / coax_p_fmax((-l_d.y), 0x1.a36e2ep-14f));
    l_a = coax_p_fmax(l_top, 0.0f);
    l_b = select(coax_p_fmin(l_bottom, 0x1.f40000p10f), 0x1.f40000p10f, (l_d.y >= 0.0f));
    l_steps = 14i;
    l_fa = ((coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_a))).y - coax_p_waterlab_sea_height(coax_p_wg_sw_xz_f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_a))), l_t0));
    l_found = false;
    {
      l_i_20 = 1i;
      while ((l_i_20 <= l_steps))
      {
        l_tb = (l_a + (((l_b - l_a) * f32(l_i_20)) / f32(l_steps)));
        l_q_22 = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tb));
        l_fb = (l_q_22.y - coax_p_waterlab_sea_height(coax_p_v2(l_q_22.x, l_q_22.z), l_t0));
        if ((l_fb < 0.0f))
        {
          l_b = l_tb;
          l_found = true;
          break;
        }
        l_a = l_tb;
        l_fa = l_fb;
        l_i_20 = (l_i_20 + 1i);
      }
    }
    if (l_found)
    {
      l_fb_24 = ((coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_b))).y - coax_p_waterlab_sea_height(coax_p_wg_sw_xz_f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_b))), l_t0));
      {
        l_i_25 = 0i;
        while ((l_i_25 < 4i))
        {
          l_tm = (l_a + (((l_b - l_a) * l_fa) / coax_p_fmax((l_fa - l_fb_24), 0x1.4f8b58p-17f)));
          l_fm = ((coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tm))).y - coax_p_waterlab_sea_height(coax_p_wg_sw_xz_f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_tm))), l_t0));
          if ((l_fm > 0.0f))
          {
            l_a = l_tm;
            l_fa = l_fm;
          }
          else
          {
            l_b = l_tm;
            l_fb_24 = l_fm;
          }
          l_i_25 = (l_i_25 + 1i);
        }
      }
      l_tw = (l_a + (((l_b - l_a) * l_fa) / coax_p_fmax((l_fa - l_fb_24), 0x1.4f8b58p-17f)));
      if ((l_tw < l_hit.x))
      {
        l_hit = coax_t16(l_tw, true);
      }
    }
    else
    {
      if ((((l_d.y < 0.0f) && (l_bottom > 0x1.f40000p10f)) && (0x1.f40000p10f < l_hit.x)))
      {
        l_hit = coax_t16(((l_o.y - coax_at_u(i32(28i))) / (-l_d.y)), true);
      }
    }
  }
  return l_hit;
}
fn coax_p_waterlab_sky(p_d: vec3f) -> vec3f {
  var l_d: vec3f = p_d;
  var l_up: f32;
  var l_zenith: vec3f;
  var l_horizon: vec3f;
  var l_c: vec3f;
  var l_s: f32;
  var l_q: vec2f;
  var l_cl: f32;
  l_up = coax_p_clampf(l_d.y, 0.0f, 0x1.000000p0f);
  l_zenith = coax_p_v3(0x1.70a3d8p-2f, 0x1.333334p-1f, 0x1.99999ap-1f);
  l_horizon = coax_p_v3(0x1.99999ap-1f, 0x1.c28f5cp-1f, 0x1.b851ecp-1f);
  l_c = coax_p_wg_mix_f3f3f(l_horizon, l_zenith, coax_pow_bg_vs(l_up, 0x1.19999ap-1f));
  l_s = coax_p_fmax(coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f);
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.3d70a4p-1f), ((coax_pow_bg_vs(l_s, 0x1.000000p3f) * 0x1.333334p-2f) + (coax_pow_bg_vs(l_s, 0x1.c20000p9f) * 0x1.333334p0f))));
  if ((l_d.y > 0.0f))
  {
    l_q = coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), (l_d.y + 0x1.333334p-3f)), 0x1.99999ap0f);
    l_cl = coax_p_smoothstep(0x1.19999ap-1f, 0x1.b33334p-1f, ((coax_p_waterlab_noise(l_q) * 0x1.4cccccp-1f) + (coax_p_waterlab_noise(coax_p_wg_add_f2f(coax_p_wg_mul_f2f(l_q, 0x1.266666p1f), 0x1.000000p2f)) * 0x1.666666p-2f)));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.e66666p-1f, 0x1.eb851ep-1f, 0x1.dc28f6p-1f), ((l_cl * 0x1.19999ap-1f) * coax_p_smoothstep(0.0f, 0x1.000000p-2f, l_d.y)));
  }
  return l_c;
}
fn coax_p_waterlab_haze(p_c: vec3f, p_d: vec3f, p_dist: f32) -> vec3f {
  var l_c: vec3f = p_c;
  var l_d: vec3f = p_d;
  var l_dist: f32 = p_dist;
  var l_a: f32;
  var l_tint: vec3f;
  l_a = (0x1.000000p0f - exp(((-l_dist) / 0x1.a40000p8f)));
  l_tint = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.7ae148p-1f, 0x1.b851ecp-1f, 0x1.ae147ap-1f), coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.7ae148p-1f), (coax_pow_bg_vs(coax_p_fmax(coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f), 0x1.800000p2f) * 0x1.000000p-1f));
  return coax_p_wg_mix_f3f3f(l_c, l_tint, (l_a * 0x1.b33334p-1f));
}
fn coax_p_waterlab_water(p_p: vec2f, p_world: vec3f, p_d: vec3f, p_dist: f32, p_t: f32) -> vec3f {
  var l_p: vec2f = p_p;
  var l_world: vec3f = p_world;
  var l_d: vec3f = p_d;
  var l_dist: f32 = p_dist;
  var l_t: f32 = p_t;
  var l_c: vec4f;
  var l_s: vec4f;
  var l_h: f32;
  var l_footprint: f32;
  var l_keep: f32;
  var l_farfade: f32;
  var l_slope: vec2f;
  var l_n: vec3f;
  var l_calm: vec3f;
  var l_shallow: vec3f;
  var l_mid: vec3f;
  var l_deep: vec3f;
  var l_open: vec3f;
  var l_col: vec3f;
  var l_v: vec3f;
  var l_r: vec3f;
  var l_fres: f32;
  var l_rs: vec3f;
  var l_cell: f32;
  var l_g: vec2f;
  var l_tw: f32;
  var l_near: f32;
  var l_froth: f32;
  var l_caps: f32;
  var l_thin: f32;
  var l_line: f32;
  var l_white: f32;
  l_c = coax_p_waterlab_coarse(l_p);
  l_s = coax_p_waterlab_surf(l_p, l_t);
  l_h = l_s.w;
  l_footprint = ((((l_dist * 0x1.000000p1f) * coax_at_u(i32(11i))) / coax_at_u(i32(1i))) / coax_p_fmax(abs(l_d.y), 0x1.47ae14p-4f));
  l_keep = coax_p_clampf((((coax_at_u(i32(30i)) / 0x1.000000p6f) / coax_p_fmax(l_footprint, 0x1.a36e2ep-14f)) - 0x1.000000p-1f), 0.0f, 0x1.000000p0f);
  l_farfade = (0x1.000000p0f - coax_p_smoothstep(0x1.2c0000p7f, 0x1.c20000p9f, l_dist));
  l_slope = coax_p_wg_mul_f2f(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_c.z, l_c.w), coax_p_fmin(l_s.z, 0x1.99999ap0f)), coax_p_wg_mul_f2f(coax_p_waterlab_fine(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.000000p0f), coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_v2(coax_at_u(i32(32i)), coax_at_u(i32(33i))), l_t), 0x1.333334p-2f))), l_keep)), l_farfade);
  l_n = coax_p_wg_normalize3(coax_p_v3((-l_slope.x), 0x1.000000p0f, (-l_slope.y)));
  l_calm = coax_p_wg_normalize3(coax_p_v3(((-l_slope.x) / 0x1.800000p1f), 0x1.000000p0f, ((-l_slope.y) / 0x1.800000p1f)));
  l_shallow = coax_p_v3(0x1.333334p-2f, 0x1.70a3d8p-1f, 0x1.7ae148p-1f);
  l_mid = coax_p_v3(0x1.99999ap-4f, 0x1.ccccccp-2f, 0x1.19999ap-1f);
  l_deep = coax_p_v3(0x1.99999ap-5f, 0x1.c28f5cp-3f, 0x1.70a3d8p-2f);
  l_open = coax_p_v3(0x1.1eb852p-5f, 0x1.5c28f6p-3f, 0x1.3d70a4p-2f);
  l_col = coax_p_wg_mix_f3f3f(l_shallow, l_mid, coax_p_smoothstep(0x1.000000p-2f, 0x1.19999ap-1f, l_h));
  l_col = coax_p_wg_mix_f3f3f(l_col, l_deep, coax_p_smoothstep(0x1.59999ap0f, 0x1.a66666p0f, l_h));
  l_col = coax_p_wg_mix_f3f3f(l_col, l_open, coax_p_smoothstep(0x1.800000p2f, 0x1.c00000p3f, l_h));
  l_col = coax_p_wg_mul_f3f(l_col, (0x1.a3d70ap-1f + (0x1.333334p-2f * coax_p_fmax(coax_p_wg_dot3(l_n, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f))));
  l_v = coax_p_wg_neg_f3(l_d);
  l_r = coax_p_wg_reflect3(l_d, l_calm);
  l_fres = (0x1.47ae14p-6f + (0x1.f5c290p-1f * coax_pow_bg_vs((0x1.000000p0f - coax_p_fmax(coax_p_wg_dot3(l_calm, l_v), 0.0f)), 0x1.400000p2f)));
  l_col = coax_p_wg_mix_f3f3f(l_col, coax_p_waterlab_sky(coax_p_v3(l_r.x, abs(l_r.y), l_r.z)), coax_p_fmin(l_fres, 0x1.000000p-1f));
  l_rs = coax_p_wg_reflect3(coax_p_wg_neg_f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), l_n);
  l_cell = (select(0x1.333334p-3f, 0x1.000000p-2f, (coax_at_u(i32(5i)) > 0x1.000000p-1f)) * coax_p_fmax(0x1.000000p0f, (l_footprint / 0x1.99999ap-5f)));
  l_g = coax_p_wg_floor_f2(coax_p_wg_div_f2f(coax_p_v2(l_world.x, l_world.z), l_cell));
  l_tw = coax_p_waterlab_hash2(coax_p_wg_add_f2f(l_g, floor((l_t * 0x1.800000p1f))));
  l_near = coax_p_smoothstep(cos((0x1.1eb852p-4f + (l_footprint * 0x1.47ae14p-6f))), 0x1.000000p0f, coax_p_wg_dot3(l_rs, l_v));
  l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.b33334p-1f), coax_p_stepf(0x1.a3d70ap-1f, l_tw)), l_near), 0x1.666666p0f), l_farfade));
  l_froth = ((coax_p_waterlab_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.666666p0f), coax_p_v2((l_t * 0x1.99999ap-3f), 0.0f))) * 0x1.333334p-1f) + (coax_p_waterlab_noise(coax_p_wg_mul_f2f(l_p, 0x1.066666p2f)) * 0x1.99999ap-2f));
  l_caps = coax_p_smoothstep(0x1.ccccccp-2f, 0x1.800000p-1f, (l_c.y * (0x1.333334p-1f + l_froth)));
  l_thin = (l_world.y - coax_p_waterlab_ground(l_p));
  l_line = ((0x1.000000p0f - coax_p_smoothstep(0x1.89374cp-8f, 0x1.47ae14p-6f, (l_thin + (0x1.0624dep-8f * sin(((l_p.x * 0x1.8cccccp1f) + (l_t * 0x1.800000p0f))))))) * coax_p_smoothstep(0x1.333334p-2f, 0x1.333334p-1f, (l_froth + 0x1.000000p-2f)));
  l_white = coax_p_fmax(coax_p_fmax((l_caps * 0x1.e66666p-1f), (l_s.y * coax_p_smoothstep(0x1.333334p-2f, 0x1.333334p-1f, (l_froth + 0x1.333334p-2f)))), l_line);
  l_col = coax_p_wg_mix_f3f3f(l_col, coax_p_wg_mul_f3f(coax_p_v3(0x1.f0a3d8p-1f, 0x1.f5c290p-1f, 0x1.eb851ep-1f), (0x1.b33334p-1f + (0x1.333334p-3f * coax_p_fmax(coax_p_wg_dot3(l_n, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f)))), coax_p_clampf(l_white, 0.0f, 0x1.000000p0f));
  return l_col;
}
fn coax_p_waterlab_ground(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_wobble: f32;
  var l_z: f32;
  var l_land: f32;
  l_wobble = ((0x1.666666p-2f * sin((l_p.x * 0x1.70a3d8p-5f))) + (0x1.70a3d8p-3f * sin(((l_p.x * 0x1.c28f5cp-4f) + 0x1.4cccccp0f))));
  l_z = ((l_p.y - coax_at_u(i32(36i))) - ((l_wobble / coax_at_u(i32(37i))) * 0x1.99999ap-5f));
  l_land = (coax_p_fmax(l_z, 0.0f) * 0x1.70a3d8p-4f);
  return coax_p_fmax((((l_z * coax_at_u(i32(37i))) + l_land) + (0x1.eb851ep-6f * coax_p_waterlab_noise(coax_p_wg_mul_f2f(l_p, 0x1.333334p-2f)))), (-0x1.400000p5f));
}
fn coax_p_waterlab_sand(p_p: vec2f, p_n: vec3f, p_wet: f32) -> vec3f {
  var l_p: vec2f = p_p;
  var l_n: vec3f = p_n;
  var l_wet: f32 = p_wet;
  var l_dry: vec3f;
  var l_base: vec3f;
  var l_light: f32;
  l_dry = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.ae147ap-1f, 0x1.8a3d70p-1f, 0x1.333334p-1f), coax_p_v3(0x1.8f5c28p-1f, 0x1.6b851ep-1f, 0x1.19999ap-1f), coax_p_waterlab_noise(coax_p_wg_mul_f2f(l_p, 0x1.b33334p0f)));
  l_base = coax_p_wg_mix_f3f3f(l_dry, coax_p_v3(0x1.47ae14p-1f, 0x1.28f5c2p-1f, 0x1.ccccccp-2f), l_wet);
  l_light = (0x1.19999ap-1f + (0x1.333334p-1f * coax_p_fmax(coax_p_wg_dot3(l_n, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f)));
  return coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f(l_base, l_light), coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.d70a3ep-1f));
}
fn coax_p_waterlab_depth_of(p_d: vec3f, p_along: f32) -> f32 {
  var l_d: vec3f = p_d;
  var l_along: f32 = p_along;
  return coax_p_clampf((0x1.000000p0f - (0x1.99999ap-4f / coax_p_fmax((l_along * coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))))), 0x1.99999ap-4f))), 0.0f, 0x1.000000p0f);
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
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
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
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
fn coax_p_waterlab_shore_at(p_p: vec2f) -> vec2f {
  var l_p: vec2f = p_p;
  var l_c: coax_t17;
  var l_side: i32;
  var l_k: u32;
  var l_slot: u32;
  var l_s: u32;
  var l_q: vec2f;
  var l_i: coax_t18;
  var l_f: vec2f;
  var l_at: u32;
  var l_a: vec2f;
  var l_b: vec2f;
  var l_d: vec2f;
  var l_e: vec2f;
  var l_v: vec2f;
  l_c = coax_p_wg_sub_i2i2(coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(coax_p_wg_div_f2f(l_p, 0x1.000000p3f))), coax_p_wg_cvt_i2f2(coax_p_v2(coax_at_u(i32(96i)), coax_at_u(i32(97i)))));
  l_side = coax_p_wg_i32_f(coax_at_u(i32(98i)));
  if ((coax_p_wg_all_ge_i2i2(l_c, coax_p_i2(0i, 0i)) && coax_p_wg_all_lt_i2i2(l_c, coax_p_i2(l_side, l_side))))
  {
    l_k = u32(((l_c.y * l_side) + l_c.x));
    l_slot = ((coax_p_waterlab_word((0u + (l_k >> u32(2u)))) >> u32((8u * (l_k & 3u)))) & 255u);
    if ((l_slot > 0u))
    {
      l_s = (l_slot - 1u);
      l_q = coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(l_p, coax_p_wg_mul_f2f(coax_p_wg_floor_f2(coax_p_wg_div_f2f(l_p, 0x1.000000p3f)), 0x1.000000p3f)), coax_at_u(i32(99i)));
      l_i = coax_p_wg_min_u2u2(coax_p_wg_cvt_u2f2(l_q), coax_p_wg_make_u2_u((5u - 2u)));
      l_f = coax_p_wg_sub_f2f2(l_q, coax_p_wg_cvt_f2u2(l_i));
      l_at = (((64u + ((l_s * 5u) * 5u)) + (l_i.y * 5u)) + l_i.x);
      l_a = coax_p_wg_unpack2x16float(coax_p_waterlab_word(l_at));
      l_b = coax_p_wg_unpack2x16float(coax_p_waterlab_word((l_at + 1u)));
      l_d = coax_p_wg_unpack2x16float(coax_p_waterlab_word((l_at + 5u)));
      l_e = coax_p_wg_unpack2x16float(coax_p_waterlab_word(((l_at + 5u) + 1u)));
      l_v = coax_p_wg_mix_f2f2f(coax_p_wg_mix_f2f2f(l_a, l_b, l_f.x), coax_p_wg_mix_f2f2f(l_d, l_e, l_f.x), l_f.y);
      return coax_p_v2(l_v.x, (l_v.y + coax_p_wg_get_f4(coax_p_wg_at4(((i32((l_s >> u32(2u))) * 4i) + 100i)), i32((l_s & 3u)))));
    }
  }
  return coax_p_v2(coax_p_waterlab_depth_here(l_p), coax_p_waterlab_surf_phase_here(l_p));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_wg_fract_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t17 {
  var l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
}
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t17 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t17(l_x, l_y);
}
fn coax_p_wg_add_i2i2(p_a: coax_t17, p_b: coax_t17) -> coax_t17 {
  var l_a: coax_t17 = p_a;
  var l_b: coax_t17 = p_b;
  return coax_p_i2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_make_i2_i(p_p0: i32) -> coax_t17 {
  var l_p0: i32 = p_p0;
  return coax_p_i2(l_p0, l_p0);
}
fn coax_p_wg_band_i2i2(p_a: coax_t17, p_b: coax_t17) -> coax_t17 {
  var l_a: coax_t17 = p_a;
  var l_b: coax_t17 = p_b;
  return coax_p_i2((l_a.x & l_b.x), (l_a.y & l_b.y));
}
fn coax_p_wg_cvt_u2i2(p_p: coax_t17) -> coax_t18 {
  var l_p: coax_t17 = p_p;
  return coax_p_u2(u32(l_p.x), u32(l_p.y));
}
fn coax_p_waterlab_word(p_i: u32) -> u32 {
  var l_i: u32 = p_i;
  return coax_p_wg_get_u4(coax_p_wg_at4u(((i32((l_i >> u32(2u))) * 4i) + 148i)), i32((l_i & 3u)));
}
fn coax_p_wg_unpack4x8snorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4(coax_p_wg_snorm8(coax_p_wg_byte(l_x, 0u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 8u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 16u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 24u)));
}
fn coax_p_wg_sw_xy_f4(p_p: vec4f) -> vec2f {
  var l_p: vec4f = p_p;
  return coax_p_v2(l_p.x, l_p.y);
}
fn coax_p_wg_mul_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a * l_b.x), (l_a * l_b.y));
}
fn coax_p_waterlab_surf(p_p: vec2f, p_t: f32) -> vec4f {
  var l_p: vec2f = p_p;
  var l_t: f32 = p_t;
  var l_o: vec4f;
  var l_sh: vec2f;
  var l_h: f32;
  var l_k0: f32;
  var l_height: f32;
  var l_breaking: f32;
  var l_phase: f32;
  var l_crest: f32;
  var l_line: f32;
  var l_broken: f32;
  var l_trail: f32;
  l_o = vec4f();
  l_sh = coax_p_waterlab_shore_at(l_p);
  l_h = l_sh.x;
  l_o.w = l_h;
  l_k0 = coax_at_u(i32(38i));
  l_o.z = select(coax_p_fmin(coax_p_waterlab_shoal_gain(l_k0, l_h), 0x1.19999ap1f), 0x1.000000p0f, (l_h > (0x1.800000p1f / l_k0)));
  l_height = ((coax_at_u(i32(35i)) * 0x1.000000p-1f) * l_o.z);
  l_breaking = coax_p_clampf(((l_height / (0x1.8f5c28p-1f * coax_p_fmax(l_h, 0x1.47ae14p-6f))) - 0x1.333334p-1f), 0.0f, 0x1.000000p0f);
  l_phase = ((l_sh.y + (coax_at_u(i32(39i)) * l_t)) + ((0x1.333334p-1f * coax_p_waterlab_noise(coax_p_v2((l_p.x * 0x1.eb851ep-6f), 0.0f))) * 0x1.91eb86p2f));
  l_crest = coax_pow_bg_vs((0x1.000000p-1f + (0x1.000000p-1f * cos(l_phase))), 0x1.800000p2f);
  l_line = coax_pow_bg_vs((0x1.000000p-1f + (0x1.000000p-1f * cos((l_phase - 0x1.000000p-2f)))), 0x1.400000p5f);
  l_o.x = ((((l_crest * l_height) * 0x1.333334p-1f) * coax_p_smoothstep(0.0f, 0x1.99999ap-2f, l_h)) * (0x1.000000p0f - (0x1.333334p-1f * l_breaking)));
  l_broken = ((coax_p_waterlab_noise(coax_p_v2((l_p.x * 0x1.666666p-1f), (l_phase * 0x1.000000p1f))) * 0x1.333334p-1f) + 0x1.99999ap-2f);
  l_trail = (coax_pow_bg_vs((0x1.000000p-1f + (0x1.000000p-1f * cos((l_phase + 0x1.ccccccp-1f)))), 0x1.400000p3f) * 0x1.333334p-1f);
  l_o.y = ((l_breaking * coax_p_smoothstep(0x1.ccccccp-2f, 0x1.99999ap-1f, ((coax_p_fmax(l_line, l_trail) * l_broken) + ((0x1.333334p-3f * l_breaking) * l_breaking)))) * coax_p_smoothstep(0x1.eb851ep-6f, 0x1.99999ap-3f, l_h));
  return l_o;
}
fn coax_p_waterlab_coarse(p_p: vec2f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_q: vec2f;
  var l_f: vec2f;
  var l_i: coax_t17;
  var l_acc: vec4f;
  var l_k: i32;
  var l_o: coax_t17;
  var l_c: coax_t18;
  var l_t: u32;
  var l_a: vec2f;
  var l_b: vec2f;
  var l_w: f32;
  l_q = coax_p_wg_sub_f2f(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(l_p, coax_at_u(i32(29i))), f32(64u)), 0x1.000000p-1f);
  l_f = coax_p_wg_fract_f2(l_q);
  l_i = coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(l_q));
  l_acc = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_o = coax_p_i2((l_k & 1i), (l_k >> u32(1i)));
      l_c = coax_p_wg_cvt_u2i2(coax_p_wg_band_i2i2(coax_p_wg_add_i2i2(l_i, l_o), coax_p_wg_make_i2_i((i32(64u) - 1i))));
      l_t = ((l_c.y * 64u) + l_c.x);
      l_a = coax_p_wg_unpack2x16float(coax_p_waterlab_word((1264u + (2u * l_t))));
      l_b = coax_p_wg_unpack2x16float(coax_p_waterlab_word(((1264u + (2u * l_t)) + 1u)));
      l_w = (select((0x1.000000p0f - l_f.x), l_f.x, (l_o.x == 1i)) * select((0x1.000000p0f - l_f.y), l_f.y, (l_o.y == 1i)));
      l_acc = coax_p_wg_add_f4f4(l_acc, coax_p_wg_mul_ff4(l_w, coax_p_v4((l_a.x * coax_at_u(i32(40i))), l_a.y, (l_b.x * coax_at_u(i32(41i))), (l_b.y * coax_at_u(i32(41i))))));
      l_k = (l_k + 1i);
    }
  }
  return l_acc;
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_WATERLAB_UNDER_SCATTER() -> vec3f {
  return coax_p_v3(0x1.99999ap-5f, 0x1.51eb86p-2f, 0x1.ae147ap-2f);
}
fn coax_p_waterlab_surface_normal(p_p: vec2f, p_t: f32) -> vec3f {
  var l_p: vec2f = p_p;
  var l_t: f32 = p_t;
  var l_c: vec4f;
  var l_sl: vec2f;
  l_c = coax_p_waterlab_coarse(l_p);
  l_sl = coax_p_wg_add_f2f2(coax_p_v2(l_c.z, l_c.w), coax_p_waterlab_fine(coax_p_wg_add_f2f2(l_p, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_v2(coax_at_u(i32(32i)), coax_at_u(i32(33i))), l_t), 0x1.333334p-2f))));
  return coax_p_wg_normalize3(coax_p_v3((-l_sl.x), 0x1.000000p0f, (-l_sl.y)));
}
fn coax_p_wg_neg_f3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_v3((-l_a.x), (-l_a.y), (-l_a.z));
}
fn coax_p_wg_refract3(p_e: vec3f, p_n: vec3f, p_eta: f32) -> vec3f {
  var l_e: vec3f = p_e;
  var l_n: vec3f = p_n;
  var l_eta: f32 = p_eta;
  var l_d: f32;
  var l_k: f32;
  var l_s: f32;
  l_d = dot(l_n, l_e);
  l_k = (0x1.000000p0f - ((l_eta * l_eta) * (0x1.000000p0f - (l_d * l_d))));
  if ((l_k < 0.0f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_s = ((l_eta * l_d) + sqrt(l_k));
  return coax_p_v3(((l_eta * l_e.x) - (l_s * l_n.x)), ((l_eta * l_e.y) - (l_s * l_n.y)), ((l_eta * l_e.z) - (l_s * l_n.z)));
}
fn coax_p_WATERLAB_UNDER_ABSORB() -> vec3f {
  return coax_p_v3(0x1.333334p-2f, 0x1.333334p-4f, 0x1.eb851ep-5f);
}
fn coax_p_wg_exp_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(exp(l_p0.x), exp(l_p0.y), exp(l_p0.z));
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_waterlab_caustic_pattern(p_p: vec2f, p_t: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_t: f32 = p_t;
  var l_a: f32;
  var l_b: f32;
  var l_ra: f32;
  var l_rb: f32;
  l_a = coax_p_waterlab_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.19999ap0f), coax_p_v2((l_t * 0x1.000000p-2f), (l_t * 0x1.99999ap-4f))));
  l_b = coax_p_waterlab_noise(coax_p_wg_sub_f2f2(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.e66666p0f), coax_p_v2(0x1.8cccccp1f, 0x1.ecccccp2f)), coax_p_v2((l_t * 0x1.70a3d8p-3f), (l_t * 0x1.333334p-2f))));
  l_ra = (0x1.000000p0f - abs(((0x1.000000p1f * l_a) - 0x1.000000p0f)));
  l_rb = (0x1.000000p0f - abs(((0x1.000000p1f * l_b) - 0x1.000000p0f)));
  return ((coax_pow_bg_vs(l_ra, 0x1.c00000p2f) * 0x1.99999ap-1f) + (coax_pow_bg_vs(l_rb, 0x1.c00000p2f) * 0x1.19999ap-1f));
}
fn coax_p_waterlab_motes(p_o: vec3f, p_d: vec3f, p_reach: f32, p_t: f32) -> f32 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_reach: f32 = p_reach;
  var l_t: f32 = p_t;
  var l_a: f32;
  var l_n: i32;
  var l_i: i32;
  var l_c: vec3f;
  var l_m: vec3f;
  var l_along: f32;
  var l_size: f32;
  l_a = 0.0f;
  l_n = select(8i, 4i, (coax_at_u(i32(5i)) > 0x1.000000p-1f));
  {
    l_i = 0i;
    while ((l_i < l_n))
    {
      l_c = coax_p_wg_floor_f3(coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, (((f32(l_i) + 0x1.000000p-1f) * l_reach) / f32(l_n)))));
      if ((coax_p_waterlab_hash3(coax_p_wg_add_f3f(l_c, 0x1.3d70a4p-2f)) > 0x1.000000p-2f))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_m = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(l_c, coax_p_v3(coax_p_waterlab_hash3(l_c), coax_p_waterlab_hash3(coax_p_wg_add_f3f(l_c, 0x1.c00000p2f)), coax_p_waterlab_hash3(coax_p_wg_add_f3f(l_c, 0x1.a00000p3f)))), coax_p_wg_mul_f3f(coax_p_v3(sin(((l_t * 0x1.333334p-2f) + (coax_p_waterlab_hash3(l_c) * 0x1.800000p2f))), cos((l_t * 0x1.ae147ap-3f)), 0.0f), 0x1.47ae14p-4f));
      l_along = coax_p_fmax(coax_p_wg_dot3(coax_p_wg_sub_f3f3(l_m, l_o), l_d), 0x1.99999ap-5f);
      if ((l_along > l_reach))
      {
        {
          l_i = (l_i + 1i);
          continue;
        }
      }
      l_size = (((((0x1.47ae14p-9f * l_along) * coax_at_u(i32(1i))) / 0x1.c20000p9f) * 0x1.000000p2f) + 0x1.0624dep-8f);
      l_a = (l_a + (((0x1.000000p0f - coax_p_smoothstep((l_size * 0x1.99999ap-2f), l_size, coax_p_wg_length3(coax_p_wg_sub_f3f3(l_m, coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_along)))))) * 0x1.000000p-1f) * exp(((-l_along) * 0x1.99999ap-3f))));
      l_i = (l_i + 1i);
    }
  }
  return l_a;
}
fn coax_p_waterlab_shafts(p_o: vec3f, p_d: vec3f, p_path: f32, p_t: f32) -> f32 {
  var l_o: vec3f = p_o;
  var l_d: vec3f = p_d;
  var l_path: f32 = p_path;
  var l_t: f32 = p_t;
  var l_sum: f32;
  var l_reach: f32;
  var l_i: i32;
  var l_q: vec3f;
  var l_below: f32;
  if ((coax_at_u(i32(5i)) > 0x1.000000p-1f))
  {
    return 0.0f;
  }
  l_sum = 0.0f;
  l_reach = coax_p_fmin(l_path, 0x1.c00000p3f);
  {
    l_i = 0i;
    while ((l_i < 16i))
    {
      l_q = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, (((f32(l_i) + 0x1.000000p-1f) / 0x1.000000p4f) * l_reach)));
      l_below = coax_p_fmax((coax_at_u(i32(28i)) - l_q.y), 0.0f);
      l_sum = (l_sum + (coax_p_waterlab_caustic_pattern(coax_p_wg_mul_f2f(coax_p_wg_add_f2f2(coax_p_v2(l_q.x, l_q.z), coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_v2(coax_at_u(i32(24i)), coax_at_u(i32(26i))), l_below), 0x1.800000p-1f)), 0x1.000000p-1f), l_t) * exp((((-l_below) * 0x1.eb851ep-4f) - (coax_p_wg_length3(coax_p_wg_sub_f3f3(l_q, l_o)) * 0x1.47ae14p-4f)))));
      l_i = (l_i + 1i);
    }
  }
  return (((l_sum / 0x1.000000p4f) * l_reach) * 0x1.eb851ep-4f);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_sw_xz_f3(p_p: vec3f) -> vec2f {
  var l_p: vec3f = p_p;
  return coax_p_v2(l_p.x, l_p.z);
}
fn coax_p_waterlab_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_waterlab_hash2(l_i), coax_p_waterlab_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_waterlab_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_waterlab_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_waterlab_fine(p_p: vec2f) -> vec2f {
  var l_p: vec2f = p_p;
  var l_q: vec2f;
  var l_f: vec2f;
  var l_i: coax_t17;
  var l_acc: vec2f;
  var l_k: i32;
  var l_o: coax_t17;
  var l_c: coax_t18;
  var l_w: f32;
  l_q = coax_p_wg_sub_f2f(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(l_p, coax_at_u(i32(30i))), f32(64u)), 0x1.000000p-1f);
  l_f = coax_p_wg_fract_f2(l_q);
  l_i = coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(l_q));
  l_acc = coax_p_v2(0.0f, 0.0f);
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_o = coax_p_i2((l_k & 1i), (l_k >> u32(1i)));
      l_c = coax_p_wg_cvt_u2i2(coax_p_wg_band_i2i2(coax_p_wg_add_i2i2(l_i, l_o), coax_p_wg_make_i2_i((i32(64u) - 1i))));
      l_w = (select((0x1.000000p0f - l_f.x), l_f.x, (l_o.x == 1i)) * select((0x1.000000p0f - l_f.y), l_f.y, (l_o.y == 1i)));
      l_acc = coax_p_wg_add_f2f2(l_acc, coax_p_wg_mul_f2f(coax_p_wg_mul_ff2(l_w, coax_p_wg_unpack2x16float(coax_p_waterlab_word(((11504u + (l_c.y * 64u)) + l_c.x)))), coax_at_u(i32(42i))));
      l_k = (l_k + 1i);
    }
  }
  return l_acc;
}
fn coax_p_wg_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_reflect3(l_i, l_n);
}
fn coax_p_waterlab_hash2(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_render_noise2(l_p.x, l_p.y);
}
fn coax_p_stepf(p_edge: f32, p_x: f32) -> f32 {
  var l_edge: f32 = p_edge;
  var l_x: f32 = p_x;
  if ((l_x < l_edge))
  {
    return 0.0f;
  }
  return 0x1.000000p0f;
}
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
}
fn coax_p_wg_sub_i2i2(p_a: coax_t17, p_b: coax_t17) -> coax_t17 {
  var l_a: coax_t17 = p_a;
  var l_b: coax_t17 = p_b;
  return coax_p_i2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_all_ge_i2i2(p_a: coax_t17, p_b: coax_t17) -> bool {
  var l_a: coax_t17 = p_a;
  var l_b: coax_t17 = p_b;
  return ((l_a.x >= l_b.x) && (l_a.y >= l_b.y));
}
fn coax_p_wg_all_lt_i2i2(p_a: coax_t17, p_b: coax_t17) -> bool {
  var l_a: coax_t17 = p_a;
  var l_b: coax_t17 = p_b;
  return ((l_a.x < l_b.x) && (l_a.y < l_b.y));
}
fn coax_p_wg_cvt_u2f2(p_p: vec2f) -> coax_t18 {
  var l_p: vec2f = p_p;
  return coax_p_u2(coax_p_wg_u32_f(l_p.x), coax_p_wg_u32_f(l_p.y));
}
fn coax_p_wg_make_u2_u(p_p0: u32) -> coax_t18 {
  var l_p0: u32 = p_p0;
  return coax_p_u2(l_p0, l_p0);
}
fn coax_p_wg_min_u2u2(p_p0: coax_t18, p_p1: coax_t18) -> coax_t18 {
  var l_p0: coax_t18 = p_p0;
  var l_p1: coax_t18 = p_p1;
  return coax_p_u2(coax_p_wg_min_u(l_p0.x, l_p1.x), coax_p_wg_min_u(l_p0.y, l_p1.y));
}
fn coax_p_wg_cvt_f2u2(p_p: coax_t18) -> vec2f {
  var l_p: coax_t18 = p_p;
  return coax_p_v2(f32(l_p.x), f32(l_p.y));
}
fn coax_p_wg_unpack2x16float(p_x: u32) -> vec2f {
  var l_x: u32 = p_x;
  return coax_p_v2(coax_p_wg_half((l_x & 65535u)), coax_p_wg_half((l_x >> u32(16u))));
}
fn coax_p_wg_mix_f2f2f(p_p0: vec2f, p_p1: vec2f, p_p2: f32) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v2(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2));
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
fn coax_p_waterlab_depth_here(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fmax((coax_at_u(i32(28i)) - coax_p_waterlab_ground(l_p)), 0.0f);
}
fn coax_p_waterlab_surf_phase_here(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_k0: f32;
  var l_h: f32;
  var l_reach: f32;
  var l_sum: f32;
  var l_i: i32;
  var l_hh: f32;
  l_k0 = coax_at_u(i32(38i));
  l_h = coax_p_waterlab_depth_here(l_p);
  l_reach = coax_p_fmin(l_h, (0x1.800000p1f / l_k0));
  l_sum = 0.0f;
  {
    l_i = 0i;
    while ((l_i < 8i))
    {
      l_hh = (((f32(l_i) + 0x1.000000p-1f) / 0x1.000000p3f) * l_reach);
      l_sum = (l_sum + coax_p_waterlab_shallow_k(l_k0, l_hh));
      l_i = (l_i + 1i);
    }
  }
  return ((((l_sum / 0x1.000000p3f) * l_reach) + ((l_h - l_reach) * l_k0)) / coax_at_u(i32(37i)));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t18 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t18(l_x, l_y);
}
fn coax_p_wg_at4u(p_at: i32) -> coax_t20 {
  var l_at: i32 = p_at;
  return coax_p_u4(coax_p_wg_atu(l_at), coax_p_wg_atu((l_at + 1i)), coax_p_wg_atu((l_at + 2i)), coax_p_wg_atu((l_at + 3i)));
}
fn coax_p_wg_get_u4(p_p: coax_t20, p_i: i32) -> u32 {
  var l_p: coax_t20 = p_p;
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
fn coax_p_waterlab_shoal_gain(p_k0: f32, p_h: f32) -> f32 {
  var l_k0: f32 = p_k0;
  var l_h: f32 = p_h;
  var l_k: f32;
  var l_kh: f32;
  var l_n: f32;
  var l_c: f32;
  var l_cg0: f32;
  l_k = coax_p_waterlab_shallow_k(l_k0, l_h);
  l_kh = (l_k * coax_p_fmax(l_h, 0x1.47ae14p-7f));
  l_n = (0x1.000000p-1f * (0x1.000000p0f + ((0x1.000000p1f * l_kh) / coax_p_wg_sinh(coax_p_fmin((0x1.000000p1f * l_kh), 0x1.400000p4f)))));
  l_c = sqrt(((0x1.39eb86p3f / l_k) * coax_p_wg_tanh(l_kh)));
  l_cg0 = (0x1.000000p-1f * sqrt((0x1.39eb86p3f / l_k0)));
  return sqrt((l_cg0 / coax_p_fmax((l_n * l_c), 0x1.0624dep-10f)));
}
fn coax_p_wg_mul_ff4(p_a: f32, p_b: vec4f) -> vec4f {
  var l_a: f32 = p_a;
  var l_b: vec4f = p_b;
  return coax_p_v4((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z), (l_a * l_b.w));
}
fn coax_p_wg_add_f4f4(p_a: vec4f, p_b: vec4f) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return coax_p_v4((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
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
fn coax_p_wg_floor_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(floor(l_p0.x), floor(l_p0.y), floor(l_p0.z));
}
fn coax_p_wg_add_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x + l_b), (l_a.y + l_b), (l_a.z + l_b));
}
fn coax_p_waterlab_hash3(p_c: vec3f) -> f32 {
  var l_c: vec3f = p_c;
  return coax_p_render_noise3(l_c.x, l_c.y, l_c.z);
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_wg_sub_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a - l_b.x), (l_a - l_b.y));
}
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_i))));
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
fn coax_p_waterlab_shallow_k(p_k0: f32, p_h: f32) -> f32 {
  var l_k0: f32 = p_k0;
  var l_h: f32 = p_h;
  return (l_k0 / sqrt(coax_p_fmax(coax_p_wg_tanh((l_k0 * coax_p_fmax(l_h, 0x1.47ae14p-7f))), 0x1.a36e2ep-14f)));
}
fn coax_p_wg_atu(p_at: i32) -> u32 {
  var l_at: i32 = p_at;
  return bitcast<u32>(coax_at_u(i32(l_at)));
}
fn coax_p_u4(p_x: u32, p_y: u32, p_z: u32, p_w: u32) -> coax_t20 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  var l_z: u32 = p_z;
  var l_w: u32 = p_w;
  return coax_t20(l_x, l_y, l_z, l_w);
}
fn coax_p_wg_sinh(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return ((exp(l_x) - exp((-l_x))) * 0x1.000000p-1f);
}
fn coax_p_wg_tanh(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_e: f32;
  if ((l_x > 0x1.400000p3f))
  {
    return 0x1.000000p0f;
  }
  if ((l_x < (-0x1.400000p3f)))
  {
    return (-0x1.000000p0f);
  }
  l_e = exp((0x1.000000p1f * l_x));
  return ((l_e - 0x1.000000p0f) / (l_e + 0x1.000000p0f));
}
fn coax_p_render_noise3(p_x: f32, p_y: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  if ((((!coax_p_render_noise_coordinate_ok(l_x)) || (!coax_p_render_noise_coordinate_ok(l_y))) || (!coax_p_render_noise_coordinate_ok(l_z))))
  {
    return 0.0f;
  }
  return coax_p_render_noise_unit(coax_p_render_noise_mix3(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y), coax_p_render_noise_cell(l_z)));
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
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
fn coax_p_render_noise_mix3(p_x: u32, p_y: u32, p_z: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  var l_z: u32 = p_z;
  return coax_p_render_noise_permute((((l_x * 1597334677u) ^ (l_y * 3812015801u)) ^ (l_z * 2654435761u)));
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
fn surface_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_12 {
  let coax_r = coax_p_surface_vs(coax_vid, coax_iid);
  return coax_io_12(coax_r.x, coax_r.y, coax_r.z);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_13) -> coax_io_14 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t13(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_14(coax_r.x, coax_r.y);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_15 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_15(coax_r.x, coax_r.y);
}
@fragment
fn surface_fs(coax_in: coax_io_12) -> @location(0) vec4f {
  let coax_r = coax_p_surface_fs(coax_t12(coax_in.x, coax_in.y, coax_in.z));
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_14) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t14(coax_in.x, coax_in.y));
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