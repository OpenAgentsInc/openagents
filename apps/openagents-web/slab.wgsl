// ---- generated: apps/openagents-web/gpu/slab.coax (the module) by coax wgsl; do not edit
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
  x: f32,
  y: vec3f,
  z: f32,
  w: f32,
}
struct coax_t16 {
  x: vec4f,
  y: f32,
}
struct coax_out_16 {
  @location(0) x: vec4f,
  @builtin(frag_depth) y: f32,
}
struct coax_t17 {
  x: i32,
  y: i32,
}
struct coax_t18 {
  x: u32,
  y: u32,
}
struct coax_t19 {
  x: u32,
  y: u32,
  z: u32,
  w: u32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 3848>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  let l_i: u32 = p_i;
  let l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_surface_vs(p_vid: u32, p__iid2: u32) -> coax_t12 {
  let l_vid: u32 = p_vid;
  let l__iid2: u32 = p__iid2;
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
  var l_reach: f32;
  var l_keep: f32;
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
  l_reach = coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(coax_at_u(i32(8i)), coax_at_u(i32(10i)))));
  l_keep = (0x1.000000p0f - coax_p_smoothstep((coax_at_u(i32(29i)) * 0x1.400000p1f), (coax_at_u(i32(29i)) * 0x1.000000p3f), l_reach));
  l_moved = coax_p_wg_mul_f2f(coax_p_slab_chop(l_p), l_keep);
  l_w = coax_p_v3((l_p.x + l_moved.x), (coax_at_u(i32(28i)) + ((coax_p_slab_sea_height(l_p) - coax_at_u(i32(28i))) * l_keep)), (l_p.y + l_moved.y));
  l_o = coax_t12();
  l_rel = coax_p_wg_sub_f3f3(l_w, coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_vz = coax_p_wg_dot3(l_rel, coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))));
  l_aspect = (coax_at_u(i32(0i)) / coax_at_u(i32(1i)));
  l_o.x = coax_p_v4((coax_p_wg_dot3(l_rel, coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)))) / (coax_at_u(i32(11i)) * l_aspect)), (coax_p_wg_dot3(l_rel, coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i)))) / coax_at_u(i32(11i))), (l_vz - 0x1.000000p-1f), l_vz);
  l_o.y = l_p;
  l_o.z = l_w;
  return l_o;
}
fn coax_p_fx_vs(p__vid3: u32, p__iid4: u32, p_in_: coax_t13) -> coax_t14 {
  let l__vid3: u32 = p__vid3;
  let l__iid4: u32 = p__iid4;
  let l_in_: coax_t13 = p_in_;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_o: coax_t14;
  l_a = l_in_.x;
  l_c = l_in_.z;
  l_o = coax_t14();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = l_c;
  return l_o;
}
fn coax_p_bg_fs(p_frag: vec4f) -> coax_t16 {
  let l_frag: vec4f = p_frag;
  var l_ndc: vec2f;
  var l_aspect: f32;
  var l_d: vec3f;
  var l_o: vec3f;
  var l_hit: coax_t15;
  var l_c: vec3f;
  var l_c_8: vec3f;
  l_ndc = coax_p_v2((((l_frag.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_frag.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_aspect = (coax_at_u(i32(0i)) / coax_at_u(i32(1i)));
  l_d = coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))), l_ndc.x), coax_at_u(i32(11i))), l_aspect)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i))), l_ndc.y), coax_at_u(i32(11i)))));
  l_o = coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i)));
  l_hit = coax_p_slab_scene(l_o, l_d, 0i, 0x1.dcd650p29f);
  if ((l_hit.x < 0x1.dcd650p29f))
  {
    l_c = coax_p_slab_haze(coax_p_slab_lit(l_hit, l_o, l_d), l_d, l_hit.x);
    return coax_t16(coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f), coax_p_slab_depth_of(l_d, l_hit.x));
  }
  l_c_8 = coax_p_slab_haze(coax_p_slab_sky(l_d), l_d, (0x1.388000p15f * (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.99999ap-3f, l_d.y))));
  return coax_t16(coax_p_v4(l_c_8.x, l_c_8.y, l_c_8.z, 0x1.000000p0f), 0x1.000000p0f);
}
fn coax_p_surface_fs(p_v: coax_t12) -> vec4f {
  let l_v: coax_t12 = p_v;
  var l_rel: vec3f;
  var l_dist: f32;
  var l_d: vec3f;
  var l_c: vec3f;
  l_rel = coax_p_wg_sub_f3f3(l_v.z, coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_dist = coax_p_wg_length3(l_rel);
  l_d = coax_p_wg_div_f3f(l_rel, coax_p_fmax(l_dist, 0x1.a36e2ep-14f));
  l_c = coax_p_slab_water(l_v.y, l_v.z, l_d, l_dist, coax_at_u(i32(2i)));
  return coax_p_wg_make_f4_f3f(coax_p_slab_haze(l_c, l_d, l_dist), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t14) -> vec4f {
  let l_v: coax_t14 = p_v;
  return coax_p_wg_make_f4_f3f(coax_p_v3(l_v.y.x, l_v.y.y, l_v.y.z), 0x1.000000p0f);
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
fn coax_p_wg_u32_f(p_x: f32) -> u32 {
  let l_x: f32 = p_x;
  return select(0u, u32(coax_p_fmin(l_x, 0x1.fffffep31f)), (l_x > 0.0f));
}
fn coax_p_wg_min_u(p_a: u32, p_b: u32) -> u32 {
  let l_a: u32 = p_a;
  let l_b: u32 = p_b;
  return select(l_a, l_b, (l_b < l_a));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  let l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_round(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  var l_r: f32;
  l_r = floor((l_x + 0x1.000000p-1f));
  if ((((l_r - l_x) == 0x1.000000p-1f) && (coax_p_wg_fmod(l_r, 0x1.000000p1f) != 0.0f)))
  {
    return (l_r - 0x1.000000p0f);
  }
  return l_r;
}
fn coax_p_wg_i32_f(p_x: f32) -> i32 {
  let l_x: f32 = p_x;
  return select(i32(coax_p_fmax(coax_p_fmin(l_x, 0x1.fffffep30f), (-0x1.000000p31f))), 0i, (l_x != l_x));
}
fn coax_p_wg_abs_i(p_x: i32) -> i32 {
  let l_x: i32 = p_x;
  return select(l_x, (-l_x), (l_x < 0i));
}
fn coax_p_wg_max_i(p_a: i32, p_b: i32) -> i32 {
  let l_a: i32 = p_a;
  let l_b: i32 = p_b;
  return select(l_a, l_b, (l_b > l_a));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_low: f32 = p_low;
  let l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  let l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_slab_chop(p_p: vec2f) -> vec2f {
  let l_p: vec2f = p_p;
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
      l_half = ((coax_p_slab_word((8192u + (l_t >> u32(1u)))) >> u32((16u * (l_t & 1u)))) & 65535u);
      l_acc = coax_p_wg_add_f2f2(l_acc, coax_p_wg_mul_ff2(l_w, coax_p_wg_sw_xy_f4(coax_p_wg_unpack4x8snorm(l_half))));
      l_k = (l_k + 1i);
    }
  }
  return coax_p_wg_mul_f2f(l_acc, coax_at_u(i32(43i)));
}
fn coax_p_slab_sea_height(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  return (coax_at_u(i32(28i)) + (coax_p_slab_coarse(l_p)).x);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return dot(l_a, l_b);
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
fn coax_p_slab_scene(p_o: vec3f, p_d: vec3f, p_first: i32, p_limit: f32) -> coax_t15 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_first: i32 = p_first;
  let l_limit: f32 = p_limit;
  var l_hit: coax_t15;
  var l_lo: vec3f;
  var l_hi: vec3f;
  var l_inside: bool;
  var l_count: i32;
  var l_i: i32;
  var l_at: i32;
  var l_p: vec4f;
  var l_q: vec4f;
  var l_blo: vec3f;
  var l_bhi: vec3f;
  var l_h: vec4f;
  l_hit = coax_t15(l_limit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (-0x1.000000p0f), 0.0f);
  l_lo = coax_p_v3(((-coax_at_u(i32(36i))) - 0x1.800000p3f), (coax_at_u(i32(39i)) - 0x1.000000p0f), ((-coax_at_u(i32(37i))) - 0x1.800000p3f));
  l_hi = coax_p_v3((coax_at_u(i32(36i)) + 0x1.800000p3f), (coax_at_u(i32(38i)) + 0x1.7c0000p7f), (coax_at_u(i32(37i)) + 0x1.800000p3f));
  l_inside = ((((((l_o.x > l_lo.x) && (l_o.x < l_hi.x)) && (l_o.y > l_lo.y)) && (l_o.y < l_hi.y)) && (l_o.z > l_lo.z)) && (l_o.z < l_hi.z));
  if ((!l_inside))
  {
    if (((coax_p_slab_box_hit(l_o, l_d, l_lo, l_hi)).x >= l_limit))
    {
      return l_hit;
    }
  }
  l_count = coax_p_wg_i32_f(coax_at_u(i32(6i)));
  l_i = l_first;
  while ((l_i < l_count))
  {
    l_at = (96i + (i32(l_i) * 8i));
    l_p = coax_p_wg_at4(l_at);
    l_q = coax_p_wg_at4((l_at + 4i));
    l_blo = coax_p_v3(l_p.x, l_p.y, l_p.z);
    l_bhi = coax_p_v3(l_q.x, l_q.y, l_q.z);
    l_h = coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
    if ((l_q.w > 0x1.000000p-1f))
    {
      l_h = coax_p_slab_drum_hit(l_o, l_d, l_blo, l_bhi);
    }
    else
    {
      l_h = coax_p_slab_box_hit(l_o, l_d, l_blo, l_bhi);
    }
    if ((l_h.x < l_hit.x))
    {
      l_hit = coax_t15(l_h.x, coax_p_v3(l_h.y, l_h.z, l_h.w), l_p.w, l_q.y);
    }
    l_i = (l_i + 1i);
  }
  return l_hit;
}
fn coax_p_slab_lit(p_hit: coax_t15, p_o: vec3f, p_d: vec3f) -> vec3f {
  let l_hit: coax_t15 = p_hit;
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  var l_w: vec3f;
  var l_n: vec3f;
  var l_sun: vec3f;
  var l_base: vec3f;
  var l_ndl: f32;
  var l_shade: f32;
  var l_s: coax_t15;
  var l_sky: f32;
  var l_light: f32;
  var l_c: vec3f;
  l_w = coax_p_wg_add_f3f3(l_o, coax_p_wg_mul_f3f(l_d, l_hit.x));
  l_n = l_hit.y;
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_base = coax_p_slab_paint(l_hit.z, l_w, l_n, l_hit.w, l_d);
  l_ndl = coax_p_fmax(coax_p_wg_dot3(l_n, l_sun), 0.0f);
  l_shade = 0x1.000000p0f;
  if (((l_ndl > 0.0f) && (coax_at_u(i32(5i)) < 0x1.000000p-1f)))
  {
    l_s = coax_p_slab_scene(coax_p_wg_add_f3f3(l_w, coax_p_wg_mul_f3f(l_n, 0x1.99999ap-5f)), l_sun, 6i, 0x1.f40000p11f);
    if ((l_s.x < 0x1.f40000p11f))
    {
      l_shade = 0.0f;
    }
  }
  l_sky = (0x1.d70a3ep-2f + (0x1.47ae14p-3f * l_n.y));
  l_light = (l_sky + ((0x1.70a3d8p-1f * l_ndl) * l_shade));
  l_light = coax_p_mix(l_light, (floor(((l_light * 0x1.800000p2f) + 0x1.000000p-1f)) / 0x1.800000p2f), 0x1.666666p-2f);
  l_c = coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f(l_base, l_light), coax_p_v3(0x1.051eb8p0f, 0x1.000000p0f, 0x1.eb851ep-1f));
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.47ae14p-6f, 0x1.47ae14p-5f, 0x1.47ae14p-4f), (0x1.000000p0f - (l_ndl * l_shade))));
  return l_c;
}
fn coax_p_slab_haze(p_c: vec3f, p_d: vec3f, p_dist: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  var l_a: f32;
  var l_tint: vec3f;
  l_a = (0x1.000000p0f - exp(((-l_dist) / 0x1.b58000p12f)));
  l_tint = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.851eb8p-1f, 0x1.b851ecp-1f, 0x1.ccccccp-1f), coax_p_v3(0x1.000000p0f, 0x1.d70a3ep-1f, 0x1.8f5c28p-1f), (coax_pow_bg_vs(coax_p_fmax(coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f), 0x1.800000p2f) * 0x1.000000p-1f));
  return coax_p_wg_mix_f3f3f(l_c, l_tint, (l_a * 0x1.ccccccp-1f));
}
fn coax_p_slab_depth_of(p_d: vec3f, p_along: f32) -> f32 {
  let l_d: vec3f = p_d;
  let l_along: f32 = p_along;
  return coax_p_clampf((0x1.000000p0f - (0x1.000000p-1f / coax_p_fmax((l_along * coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))))), 0x1.000000p-1f))), 0.0f, 0x1.000000p0f);
}
fn coax_p_slab_sky(p_d: vec3f) -> vec3f {
  let l_d: vec3f = p_d;
  var l_up: f32;
  var l_zenith: vec3f;
  var l_horizon: vec3f;
  var l_c: vec3f;
  var l_s: f32;
  var l_q: vec2f;
  var l_q2: vec2f;
  var l_q3: vec2f;
  var l_fbm: f32;
  var l_cl: f32;
  l_up = coax_p_clampf(l_d.y, 0.0f, 0x1.000000p0f);
  l_zenith = coax_p_v3(0x1.333334p-2f, 0x1.19999ap-1f, 0x1.a3d70ap-1f);
  l_horizon = coax_p_v3(0x1.99999ap-1f, 0x1.c28f5cp-1f, 0x1.ccccccp-1f);
  l_c = coax_p_wg_mix_f3f3f(l_horizon, l_zenith, coax_pow_bg_vs(l_up, 0x1.000000p-1f));
  l_s = coax_p_fmax(coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f);
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.c28f5cp-1f, 0x1.51eb86p-1f), ((coax_pow_bg_vs(l_s, 0x1.000000p3f) * 0x1.000000p-2f) + (coax_pow_bg_vs(l_s, 0x1.c20000p9f) * 0x1.333334p0f))));
  if ((l_d.y > 0.0f))
  {
    l_q = coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), (l_d.y + 0x1.70a3d8p-3f)), 0x1.333334p0f);
    l_q2 = coax_p_v2((((l_q.x * 0x1.b5c290p0f) - (l_q.y * 0x1.19999ap0f)) + 0x1.8cccccp1f), (((l_q.x * 0x1.19999ap0f) + (l_q.y * 0x1.b5c290p0f)) - 0x1.b33334p0f));
    l_q3 = coax_p_v2((((l_q.x * 0x1.333334p0f) - (l_q.y * 0x1.ecccccp1f)) - 0x1.533334p2f), (((l_q.x * 0x1.ecccccp1f) + (l_q.y * 0x1.333334p0f)) + 0x1.733334p1f));
    l_fbm = (((coax_p_slab_noise(l_q) * 0x1.000000p-1f) + (coax_p_slab_noise(l_q2) * 0x1.333334p-2f)) + (coax_p_slab_noise(l_q3) * 0x1.99999ap-3f));
    l_cl = coax_p_smoothstep(0x1.000000p-1f, 0x1.8f5c28p-1f, l_fbm);
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.f0a3d8p-1f, 0x1.f0a3d8p-1f, 0x1.e66666p-1f), ((l_cl * 0x1.19999ap-1f) * coax_p_smoothstep(0x1.47ae14p-5f, 0x1.333334p-2f, l_d.y)));
  }
  return l_c;
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b > l_a));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_slab_water(p_p: vec2f, p_world: vec3f, p_d: vec3f, p_dist: f32, p_t: f32) -> vec3f {
  let l_p: vec2f = p_p;
  let l_world: vec3f = p_world;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  let l_t: f32 = p_t;
  var l_c: vec4f;
  var l_footprint: f32;
  var l_keep: f32;
  var l_farfade: f32;
  var l_slope: vec2f;
  var l_n: vec3f;
  var l_calm: vec3f;
  var l_sun: vec3f;
  var l_col: vec3f;
  var l_v: vec3f;
  var l_r: vec3f;
  var l_fres: f32;
  var l_rs: vec3f;
  var l_cell: f32;
  var l_g: vec2f;
  var l_tw: f32;
  var l_near: f32;
  var l_glint: f32;
  var l_froth: f32;
  var l_caps: f32;
  var l_edge: f32;
  var l_wash: f32;
  var l_white: f32;
  l_c = coax_p_slab_coarse(l_p);
  l_footprint = ((((l_dist * 0x1.000000p1f) * coax_at_u(i32(11i))) / coax_at_u(i32(1i))) / coax_p_fmax(abs(l_d.y), 0x1.47ae14p-4f));
  l_keep = coax_p_clampf((((coax_at_u(i32(30i)) / 0x1.000000p6f) / coax_p_fmax(l_footprint, 0x1.a36e2ep-14f)) - 0x1.000000p-1f), 0.0f, 0x1.000000p0f);
  l_farfade = (0x1.000000p0f - coax_p_smoothstep(0x1.900000p8f, 0x1.770000p12f, l_dist));
  l_slope = coax_p_wg_mul_f2f(coax_p_wg_add_f2f2(coax_p_v2(l_c.z, l_c.w), coax_p_wg_mul_f2f(coax_p_slab_fine(coax_p_wg_add_f2f2(l_p, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_v2(coax_at_u(i32(32i)), coax_at_u(i32(33i))), l_t), 0x1.333334p-2f))), l_keep)), (0x1.666666p-2f + (0x1.4cccccp-1f * l_farfade)));
  l_n = coax_p_wg_normalize3(coax_p_v3((-l_slope.x), 0x1.000000p0f, (-l_slope.y)));
  l_calm = coax_p_wg_normalize3(coax_p_v3(((-l_slope.x) / 0x1.800000p1f), 0x1.000000p0f, ((-l_slope.y) / 0x1.800000p1f)));
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_col = coax_p_v3(0x1.eb851ep-6f, 0x1.47ae14p-3f, 0x1.51eb86p-2f);
  l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_v3(0x1.47ae14p-6f, 0x1.eb851ep-4f, 0x1.eb851ep-4f), coax_p_clampf((l_c.x / coax_p_fmax(coax_at_u(i32(35i)), 0x1.99999ap-3f)), 0.0f, 0x1.000000p0f)));
  l_col = coax_p_wg_mul_f3f(l_col, (0x1.b33334p-1f + (0x1.333334p-2f * coax_p_fmax(coax_p_wg_dot3(l_n, l_sun), 0.0f))));
  l_v = coax_p_wg_neg_f3(l_d);
  l_r = coax_p_wg_reflect3(l_d, l_calm);
  l_fres = (0x1.47ae14p-6f + (0x1.f5c290p-1f * coax_pow_bg_vs((0x1.000000p0f - coax_p_fmax(coax_p_wg_dot3(l_calm, l_v), 0.0f)), 0x1.400000p2f)));
  l_col = coax_p_wg_mix_f3f3f(l_col, coax_p_slab_sky(coax_p_v3(l_r.x, abs(l_r.y), l_r.z)), coax_p_fmin(l_fres, 0x1.19999ap-1f));
  l_rs = coax_p_wg_reflect3(coax_p_wg_neg_f3(l_sun), l_n);
  l_cell = (select(0x1.333334p-3f, 0x1.000000p-2f, (coax_at_u(i32(5i)) > 0x1.000000p-1f)) * coax_p_fmax(0x1.000000p0f, (l_footprint / 0x1.99999ap-5f)));
  l_g = coax_p_wg_floor_f2(coax_p_wg_div_f2f(coax_p_v2(l_world.x, l_world.z), l_cell));
  l_tw = coax_p_slab_hash2(coax_p_wg_add_f2f(l_g, floor((l_t * 0x1.800000p1f))));
  l_near = coax_p_smoothstep(cos((0x1.1eb852p-4f + (l_footprint * 0x1.47ae14p-6f))), 0x1.000000p0f, coax_p_wg_dot3(l_rs, l_v));
  l_glint = (0x1.000000p0f - coax_p_smoothstep(0x1.900000p7f, 0x1.2c0000p10f, l_dist));
  l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.b33334p-1f), coax_p_stepf(0x1.a3d70ap-1f, l_tw)), l_near), (0x1.666666p0f * l_glint)));
  l_froth = ((coax_p_slab_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.666666p-2f), coax_p_v2((l_t * 0x1.99999ap-3f), 0.0f))) * 0x1.333334p-1f) + (coax_p_slab_noise(coax_p_wg_mul_f2f(l_p, 0x1.4cccccp0f)) * 0x1.99999ap-2f));
  l_caps = (coax_p_smoothstep(0x1.ccccccp-2f, 0x1.800000p-1f, (l_c.y * (0x1.333334p-1f + l_froth))) * (0x1.000000p0f - coax_p_smoothstep(0x1.f40000p8f, 0x1.388000p11f, l_dist)));
  l_edge = coax_p_slab_hull_plan(l_p);
  l_wash = ((0x1.000000p0f - coax_p_smoothstep(0.0f, (0x1.800000p2f + (0x1.400000p3f * l_froth)), l_edge)) * coax_p_smoothstep(0x1.000000p-2f, 0x1.19999ap-1f, (l_froth + (0x1.99999ap-3f * sin((((l_p.x + l_p.y) * 0x1.47ae14p-4f) + (l_t * 0x1.4cccccp0f)))))));
  l_white = coax_p_clampf(coax_p_fmax((l_caps * 0x1.e66666p-1f), (l_wash * 0x1.ccccccp-1f)), 0.0f, 0x1.000000p0f);
  l_col = coax_p_wg_mix_f3f3f(l_col, coax_p_wg_mul_f3f(coax_p_v3(0x1.f0a3d8p-1f, 0x1.f5c290p-1f, 0x1.eb851ep-1f), (0x1.b33334p-1f + (0x1.333334p-3f * coax_p_fmax(coax_p_wg_dot3(l_n, l_sun), 0.0f)))), l_white);
  return l_col;
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  let l_p0: vec3f = p_p0;
  let l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b < l_a));
}
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_wg_fract_f2(p_p0: vec2f) -> vec2f {
  let l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  let l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t17 {
  let l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
}
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t17 {
  let l_x: i32 = p_x;
  let l_y: i32 = p_y;
  return coax_t17(l_x, l_y);
}
fn coax_p_wg_add_i2i2(p_a: coax_t17, p_b: coax_t17) -> coax_t17 {
  let l_a: coax_t17 = p_a;
  let l_b: coax_t17 = p_b;
  return coax_p_i2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_make_i2_i(p_p0: i32) -> coax_t17 {
  let l_p0: i32 = p_p0;
  return coax_p_i2(l_p0, l_p0);
}
fn coax_p_wg_band_i2i2(p_a: coax_t17, p_b: coax_t17) -> coax_t17 {
  let l_a: coax_t17 = p_a;
  let l_b: coax_t17 = p_b;
  return coax_p_i2((l_a.x & l_b.x), (l_a.y & l_b.y));
}
fn coax_p_wg_cvt_u2i2(p_p: coax_t17) -> coax_t18 {
  let l_p: coax_t17 = p_p;
  return coax_p_u2(u32(l_p.x), u32(l_p.y));
}
fn coax_p_slab_word(p_i: u32) -> u32 {
  let l_i: u32 = p_i;
  return coax_p_wg_get_u4(coax_p_wg_at4u(((i32((l_i >> u32(2u))) * 4i) + 1056i)), i32((l_i & 3u)));
}
fn coax_p_wg_unpack4x8snorm(p_x: u32) -> vec4f {
  let l_x: u32 = p_x;
  return coax_p_v4(coax_p_wg_snorm8(coax_p_wg_byte(l_x, 0u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 8u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 16u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 24u)));
}
fn coax_p_wg_sw_xy_f4(p_p: vec4f) -> vec2f {
  let l_p: vec4f = p_p;
  return coax_p_v2(l_p.x, l_p.y);
}
fn coax_p_wg_mul_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  let l_a: f32 = p_a;
  let l_b: vec2f = p_b;
  return coax_p_v2((l_a * l_b.x), (l_a * l_b.y));
}
fn coax_p_slab_coarse(p_p: vec2f) -> vec4f {
  let l_p: vec2f = p_p;
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
      l_a = coax_p_wg_unpack2x16float(coax_p_slab_word((0u + (2u * l_t))));
      l_b = coax_p_wg_unpack2x16float(coax_p_slab_word(((0u + (2u * l_t)) + 1u)));
      l_w = (select((0x1.000000p0f - l_f.x), l_f.x, (l_o.x == 1i)) * select((0x1.000000p0f - l_f.y), l_f.y, (l_o.y == 1i)));
      l_acc = coax_p_wg_add_f4f4(l_acc, coax_p_wg_mul_ff4(l_w, coax_p_v4((l_a.x * coax_at_u(i32(40i))), l_a.y, (l_b.x * coax_at_u(i32(41i))), (l_b.y * coax_at_u(i32(41i))))));
      l_k = (l_k + 1i);
    }
  }
  return l_acc;
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_slab_box_hit(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_lo: vec3f = p_lo;
  let l_hi: vec3f = p_hi;
  var l_ix: f32;
  var l_iy: f32;
  var l_iz: f32;
  var l_ax: f32;
  var l_bx: f32;
  var l_ay: f32;
  var l_by: f32;
  var l_az: f32;
  var l_bz: f32;
  var l_nx: f32;
  var l_ny: f32;
  var l_nz: f32;
  var l_tn: f32;
  var l_tf: f32;
  l_ix = select(0x1.e84800p19f, (0x1.000000p0f / l_d.x), (abs(l_d.x) > 0x1.0c6f7ap-20f));
  l_iy = select(0x1.e84800p19f, (0x1.000000p0f / l_d.y), (abs(l_d.y) > 0x1.0c6f7ap-20f));
  l_iz = select(0x1.e84800p19f, (0x1.000000p0f / l_d.z), (abs(l_d.z) > 0x1.0c6f7ap-20f));
  l_ax = ((l_lo.x - l_o.x) * l_ix);
  l_bx = ((l_hi.x - l_o.x) * l_ix);
  l_ay = ((l_lo.y - l_o.y) * l_iy);
  l_by = ((l_hi.y - l_o.y) * l_iy);
  l_az = ((l_lo.z - l_o.z) * l_iz);
  l_bz = ((l_hi.z - l_o.z) * l_iz);
  l_nx = coax_p_fmin(l_ax, l_bx);
  l_ny = coax_p_fmin(l_ay, l_by);
  l_nz = coax_p_fmin(l_az, l_bz);
  l_tn = coax_p_fmax(coax_p_fmax(l_nx, l_ny), l_nz);
  l_tf = coax_p_fmin(coax_p_fmin(coax_p_fmax(l_ax, l_bx), coax_p_fmax(l_ay, l_by)), coax_p_fmax(l_az, l_bz));
  if (((l_tf < l_tn) || (l_tn < 0.0f)))
  {
    return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  }
  if ((l_tn == l_nx))
  {
    return coax_p_v4(l_tn, select(0x1.000000p0f, (-0x1.000000p0f), (l_d.x > 0.0f)), 0.0f, 0.0f);
  }
  if ((l_tn == l_ny))
  {
    return coax_p_v4(l_tn, 0.0f, select(0x1.000000p0f, (-0x1.000000p0f), (l_d.y > 0.0f)), 0.0f);
  }
  return coax_p_v4(l_tn, 0.0f, 0.0f, select(0x1.000000p0f, (-0x1.000000p0f), (l_d.z > 0.0f)));
}
fn coax_p_slab_drum_hit(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_lo: vec3f = p_lo;
  let l_hi: vec3f = p_hi;
  var l_cx: f32;
  var l_cz: f32;
  var l_r: f32;
  var l_best: vec4f;
  var l_ox: f32;
  var l_oz: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_disc: f32;
  var l_t: f32;
  var l_y: f32;
  var l_t_16: f32;
  var l_px: f32;
  var l_pz: f32;
  l_cx = ((l_lo.x + l_hi.x) * 0x1.000000p-1f);
  l_cz = ((l_lo.z + l_hi.z) * 0x1.000000p-1f);
  l_r = (coax_p_fmin((l_hi.x - l_lo.x), (l_hi.z - l_lo.z)) * 0x1.000000p-1f);
  l_best = coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  l_ox = (l_o.x - l_cx);
  l_oz = (l_o.z - l_cz);
  l_a = ((l_d.x * l_d.x) + (l_d.z * l_d.z));
  if ((l_a > 0x1.5798eep-27f))
  {
    l_b = ((l_ox * l_d.x) + (l_oz * l_d.z));
    l_c = (((l_ox * l_ox) + (l_oz * l_oz)) - (l_r * l_r));
    l_disc = ((l_b * l_b) - (l_a * l_c));
    if ((l_disc >= 0.0f))
    {
      l_t = (((-l_b) - sqrt(l_disc)) / l_a);
      l_y = (l_o.y + (l_t * l_d.y));
      if ((((l_t > 0.0f) && (l_y >= l_lo.y)) && (l_y <= l_hi.y)))
      {
        l_best = coax_p_v4(l_t, ((l_ox + (l_t * l_d.x)) / l_r), 0.0f, ((l_oz + (l_t * l_d.z)) / l_r));
      }
    }
  }
  if ((l_d.y < 0.0f))
  {
    l_t_16 = ((l_hi.y - l_o.y) / l_d.y);
    l_px = (l_ox + (l_t_16 * l_d.x));
    l_pz = (l_oz + (l_t_16 * l_d.z));
    if ((((l_t_16 > 0.0f) && (l_t_16 < l_best.x)) && (((l_px * l_px) + (l_pz * l_pz)) <= (l_r * l_r))))
    {
      l_best = coax_p_v4(l_t_16, 0.0f, 0x1.000000p0f, 0.0f);
    }
  }
  return l_best;
}
fn coax_p_slab_paint(p_kind: f32, p_w: vec3f, p_n: vec3f, p_top: f32, p_d: vec3f) -> vec3f {
  let l_kind: f32 = p_kind;
  let l_w: vec3f = p_w;
  let l_n: vec3f = p_n;
  let l_top: f32 = p_top;
  let l_d: vec3f = p_d;
  var l_k: i32;
  var l_roof: bool;
  var l_grain: f32;
  var l_c: vec3f;
  var l_gx: f32;
  var l_gz: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_storey: f32;
  var l_glass: vec3f;
  var l_slot: f32;
  var l_h: f32;
  var l_fx_18: f32;
  var l_fz_19: f32;
  l_k = coax_p_wg_i32_f((l_kind + 0x1.000000p-1f));
  l_roof = (l_n.y > 0x1.000000p-1f);
  l_grain = (0x1.e147aep-1f + (0x1.47ae14p-4f * coax_p_slab_noise(coax_p_v2(((l_w.x * 0x1.0a3d70p-3f) + (l_w.y * 0x1.1eb852p-4f)), ((l_w.z * 0x1.0a3d70p-3f) - (l_w.y * 0x1.99999ap-5f))))));
  l_c = coax_p_v3(0x1.666666p-1f, 0x1.666666p-1f, 0x1.5c28f6p-1f);
  if ((l_k == 0i))
  {
    if (l_roof)
    {
      l_c = coax_p_v3(0x1.666666p-1f, 0x1.6147aep-1f, 0x1.4cccccp-1f);
      l_gx = (abs(((l_w.x / 0x1.900000p6f) - coax_p_wg_round((l_w.x / 0x1.900000p6f)))) * 0x1.900000p6f);
      l_gz = (abs(((l_w.z / 0x1.900000p6f) - coax_p_wg_round((l_w.z / 0x1.900000p6f)))) * 0x1.900000p6f);
      if ((coax_p_fmin(l_gx, l_gz) < 0x1.400000p3f))
      {
        l_c = coax_p_v3(0x1.ccccccp-2f, 0x1.d70a3ep-2f, 0x1.eb851ep-2f);
        if ((coax_p_fmin(l_gx, l_gz) < 0x1.99999ap-2f))
        {
          l_c = coax_p_v3(0x1.d70a3ep-1f, 0x1.ccccccp-1f, 0x1.a3d70ap-1f);
        }
      }
      if ((coax_p_slab_hull_plan(coax_p_v2(l_w.x, l_w.z)) > (-0x1.400000p4f)))
      {
        l_c = coax_p_v3(0x1.99999ap-1f, 0x1.7ae148p-1f, 0x1.47ae14p-1f);
      }
    }
    else
    {
      l_c = coax_p_wg_mul_f3f(coax_p_v3(0x1.3d70a4p-1f, 0x1.3d70a4p-1f, 0x1.333334p-1f), (0x1.e147aep-1f + (0x1.eb851ep-5f * coax_p_slab_band(l_w.y, 0x1.400000p2f, 0x1.47ae14p-4f))));
      l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.c28f5cp-3f, 0x1.1eb852p-2f, 0x1.147ae2p-2f), (0x1.000000p0f - coax_p_smoothstep(0x1.000000p-1f, 0x1.800000p1f, l_w.y)));
    }
  }
  else
  {
    if ((l_k == 2i))
    {
      l_c = coax_p_v3(0x1.b851ecp-1f, 0x1.c28f5cp-1f, 0x1.ccccccp-1f);
      if (l_roof)
      {
        l_c = coax_p_wg_mul_f3f(coax_p_v3(0x1.7ae148p-1f, 0x1.8a3d70p-1f, 0x1.99999ap-1f), (0x1.d70a3ep-1f + (0x1.47ae14p-4f * coax_p_slab_band(l_w.x, 0x1.800000p2f, 0x1.000000p-1f))));
      }
      else
      {
        l_c = coax_p_wg_mul_f3f(l_c, (0x1.ccccccp-1f + (0x1.99999ap-4f * coax_p_slab_band(l_w.y, 0x1.000000p2f, 0x1.99999ap-1f))));
      }
    }
    else
    {
      if ((l_k == 3i))
      {
        l_c = coax_p_v3(0x1.333334p-1f, 0x1.c28f5cp-2f, 0x1.51eb86p-2f);
        if ((!l_roof))
        {
          l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.70a3d8p-2f, 0x1.5c28f6p-2f, 0x1.51eb86p-2f), coax_p_slab_band(l_w.y, 0x1.c00000p2f, 0x1.70a3d8p-3f));
        }
      }
      else
      {
        if ((l_k == 4i))
        {
          l_c = coax_p_v3(0x1.333334p-2f, 0x1.5c28f6p-2f, 0x1.99999ap-2f);
          if (l_roof)
          {
            l_fx = (coax_p_fract((l_w.x / 0x1.200000p3f)) - 0x1.000000p-1f);
            l_fz = (coax_p_fract((l_w.z / 0x1.200000p3f)) - 0x1.000000p-1f);
            l_c = select(coax_p_v3(0x1.d70a3ep-2f, 0x1.eb851ep-2f, 0x1.0a3d70p-1f), coax_p_v3(0x1.47ae14p-3f, 0x1.70a3d8p-3f, 0x1.ae147ap-3f), (((l_fx * l_fx) + (l_fz * l_fz)) < 0x1.70a3d8p-4f));
          }
          else
          {
            l_c = coax_p_wg_mul_f3f(l_c, (0x1.b33334p-1f + (0x1.333334p-3f * coax_p_slab_band(l_w.y, 0x1.333334p0f, 0x1.000000p-1f))));
          }
        }
        else
        {
          if ((l_k == 5i))
          {
            if (l_roof)
            {
              l_c = coax_p_v3(0x1.70a3d8p-1f, 0x1.6b851ep-1f, 0x1.5c28f6p-1f);
            }
            else
            {
              l_storey = coax_p_fract(((l_w.y - coax_at_u(i32(38i))) / 0x1.ccccccp1f));
              l_glass = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.333334p-2f, 0x1.c28f5cp-2f, 0x1.1eb852p-1f), coax_p_slab_sky(coax_p_wg_reflect3(l_d, l_n)), 0x1.666666p-2f);
              l_c = select(coax_p_v3(0x1.ccccccp-1f, 0x1.c28f5cp-1f, 0x1.a3d70ap-1f), l_glass, (l_storey > 0x1.1eb852p-2f));
            }
          }
          else
          {
            if ((l_k == 6i))
            {
              l_c = coax_p_v3(0x1.b851ecp-1f, 0x1.ae147ap-1f, 0x1.99999ap-1f);
              if (((l_w.y > (l_top - 0x1.200000p3f)) || ((l_w.y > (l_top - 0x1.300000p4f)) && (l_w.y < (l_top - 0x1.c00000p3f)))))
              {
                l_c = coax_p_v3(0x1.7ae148p-1f, 0x1.99999ap-3f, 0x1.333334p-3f);
              }
              if (l_roof)
              {
                l_c = coax_p_v3(0x1.eb851ep-4f, 0x1.c28f5cp-4f, 0x1.99999ap-4f);
              }
            }
            else
            {
              if ((l_k == 7i))
              {
                l_c = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.333334p-2f, 0x1.000000p-1f, 0x1.0a3d70p-2f), coax_p_v3(0x1.ae147ap-2f, 0x1.333334p-1f, 0x1.333334p-2f), coax_p_slab_noise(coax_p_v2((l_w.x * 0x1.99999ap-3f), (l_w.z * 0x1.99999ap-3f))));
                if ((l_top > (coax_at_u(i32(38i)) + 0x1.000000p1f)))
                {
                  l_c = coax_p_wg_mul_f3f(l_c, 0x1.800000p-1f);
                }
              }
              else
              {
                if ((l_k == 8i))
                {
                  l_slot = ((l_w.x - 0x1.400000p7f) / 0x1.b33334p3f);
                  l_h = coax_p_slab_hash2(coax_p_v2(floor(l_slot), floor((l_w.z / 0x1.200000p3f))));
                  l_c = coax_p_v3(0x1.99999ap-3f, 0x1.70a3d8p-2f, 0x1.28f5c2p-1f);
                  if ((l_h > 0x1.800000p-1f))
                  {
                    l_c = coax_p_v3(0x1.a3d70ap-1f, 0x1.ae147ap-2f, 0x1.47ae14p-3f);
                  }
                  else
                  {
                    if ((l_h > 0x1.000000p-1f))
                    {
                      l_c = coax_p_v3(0x1.3d70a4p-1f, 0x1.70a3d8p-3f, 0x1.1eb852p-3f);
                    }
                    else
                    {
                      if ((l_h > 0x1.000000p-2f))
                      {
                        l_c = coax_p_v3(0x1.19999ap-1f, 0x1.23d70ap-1f, 0x1.28f5c2p-1f);
                      }
                    }
                  }
                  if ((coax_p_fract(l_slot) > 0x1.ccccccp-1f))
                  {
                    l_c = coax_p_v3(0x1.ccccccp-2f, 0x1.d70a3ep-2f, 0x1.eb851ep-2f);
                  }
                  if ((!l_roof))
                  {
                    l_c = coax_p_wg_mul_f3f(l_c, (0x1.c28f5cp-1f + (0x1.eb851ep-4f * coax_p_slab_band(l_w.x, 0x1.333334p-1f, 0x1.000000p-1f))));
                  }
                }
                else
                {
                  if ((l_k == 9i))
                  {
                    l_c = coax_p_v3(0x1.dc28f6p-1f, 0x1.c7ae14p-1f, 0x1.947ae2p-1f);
                    if ((!l_roof))
                    {
                      l_c = coax_p_wg_mul_f3f(l_c, (0x1.b851ecp-1f + (0x1.1eb852p-3f * coax_p_slab_band((l_w.x + l_w.z), 0x1.000000p2f, 0x1.4cccccp-1f))));
                    }
                  }
                  else
                  {
                    if ((l_k == 10i))
                    {
                      l_c = coax_p_v3(0x1.ccccccp-1f, 0x1.ccccccp-1f, 0x1.bd70a4p-1f);
                    }
                    else
                    {
                      if ((l_k == 11i))
                      {
                        l_c = coax_p_v3(0x1.b851ecp-1f, 0x1.ccccccp-1f, 0x1.d1eb86p-1f);
                        if ((!l_roof))
                        {
                          l_c = select(l_c, coax_p_v3(0x1.ae147ap-2f, 0x1.1eb852p-1f, 0x1.3d70a4p-1f), (coax_p_fract(((l_w.y - coax_at_u(i32(38i))) / 0x1.ccccccp1f)) > 0x1.ccccccp-2f));
                        }
                      }
                      else
                      {
                        if ((l_k == 12i))
                        {
                          l_c = coax_p_v3(0x1.ccccccp-1f, 0x1.70a3d8p-1f, 0x1.70a3d8p-3f);
                        }
                        else
                        {
                          if ((l_k == 13i))
                          {
                            l_c = coax_p_v3(0x1.19999ap-1f, 0x1.23d70ap-1f, 0x1.333334p-1f);
                            if (l_roof)
                            {
                              l_fx_18 = (coax_p_fract((l_w.x / 0x1.400000p3f)) - 0x1.000000p-1f);
                              l_fz_19 = (coax_p_fract((l_w.z / 0x1.400000p3f)) - 0x1.000000p-1f);
                              l_c = select(l_c, coax_p_v3(0x1.99999ap-3f, 0x1.c28f5cp-3f, 0x1.eb851ep-3f), (((l_fx_18 * l_fx_18) + (l_fz_19 * l_fz_19)) < 0x1.eb851ep-4f));
                            }
                          }
                          else
                          {
                            if ((l_k == 14i))
                            {
                              l_c = coax_p_v3(0x1.99999ap-5f, 0x1.d70a3ep-3f, 0x1.51eb86p-2f);
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  return coax_p_wg_mul_f3f(l_c, l_grain);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  let l_p0: vec3f = p_p0;
  let l_p1: vec3f = p_p1;
  let l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_slab_noise(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_slab_hash2(l_i), coax_p_slab_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_slab_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_slab_hash2(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
}
fn coax_p_slab_fine(p_p: vec2f) -> vec2f {
  let l_p: vec2f = p_p;
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
      l_acc = coax_p_wg_add_f2f2(l_acc, coax_p_wg_mul_f2f(coax_p_wg_mul_ff2(l_w, coax_p_wg_unpack2x16float(coax_p_slab_word(((10240u + (l_c.y * 64u)) + l_c.x)))), coax_at_u(i32(42i))));
      l_k = (l_k + 1i);
    }
  }
  return l_acc;
}
fn coax_p_wg_neg_f3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_v3((-l_a.x), (-l_a.y), (-l_a.z));
}
fn coax_p_wg_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  let l_i: vec3f = p_i;
  let l_n: vec3f = p_n;
  return coax_p_reflect3(l_i, l_n);
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_slab_hash2(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  return coax_p_render_noise2(l_p.x, l_p.y);
}
fn coax_p_stepf(p_edge: f32, p_x: f32) -> f32 {
  let l_edge: f32 = p_edge;
  let l_x: f32 = p_x;
  return select(0x1.000000p0f, 0.0f, (l_x < l_edge));
}
fn coax_p_slab_hull_plan(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  var l_qx: f32;
  var l_qz: f32;
  var l_out: f32;
  l_qx = ((abs(l_p.x) - coax_at_u(i32(36i))) + 0x1.400000p5f);
  l_qz = ((abs(l_p.y) - coax_at_u(i32(37i))) + 0x1.400000p5f);
  l_out = sqrt(((coax_p_fmax(l_qx, 0.0f) * coax_p_fmax(l_qx, 0.0f)) + (coax_p_fmax(l_qz, 0.0f) * coax_p_fmax(l_qz, 0.0f))));
  return ((l_out + coax_p_fmin(coax_p_fmax(l_qx, l_qz), 0.0f)) - 0x1.400000p5f);
}
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
}
fn coax_p_fract(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t18 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  return coax_t18(l_x, l_y);
}
fn coax_p_wg_at4u(p_at: i32) -> coax_t19 {
  let l_at: i32 = p_at;
  return coax_p_u4(coax_p_wg_atu(l_at), coax_p_wg_atu((l_at + 1i)), coax_p_wg_atu((l_at + 2i)), coax_p_wg_atu((l_at + 3i)));
}
fn coax_p_wg_get_u4(p_p: coax_t19, p_i: i32) -> u32 {
  let l_p: coax_t19 = p_p;
  let l_i: i32 = p_i;
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
  let l_x: u32 = p_x;
  let l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_wg_snorm8(p_b: u32) -> f32 {
  let l_b: u32 = p_b;
  var l_s: f32;
  l_s = f32(l_b);
  if ((l_b >= 128u))
  {
    l_s = (l_s - 0x1.000000p8f);
  }
  return coax_p_fmax((l_s / 0x1.fc0000p6f), (-0x1.000000p0f));
}
fn coax_p_wg_unpack2x16float(p_x: u32) -> vec2f {
  let l_x: u32 = p_x;
  return coax_p_v2(coax_p_wg_half((l_x & 65535u)), coax_p_wg_half((l_x >> u32(16u))));
}
fn coax_p_wg_mul_ff4(p_a: f32, p_b: vec4f) -> vec4f {
  let l_a: f32 = p_a;
  let l_b: vec4f = p_b;
  return coax_p_v4((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z), (l_a * l_b.w));
}
fn coax_p_wg_add_f4f4(p_a: vec4f, p_b: vec4f) -> vec4f {
  let l_a: vec4f = p_a;
  let l_b: vec4f = p_b;
  return coax_p_v4((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
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
fn coax_p_slab_band(p_v: f32, p_period: f32, p_share: f32) -> f32 {
  let l_v: f32 = p_v;
  let l_period: f32 = p_period;
  let l_share: f32 = p_share;
  var l_f: f32;
  l_f = coax_p_fract((l_v / l_period));
  return coax_p_stepf(l_f, l_share);
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_wg_sub_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  let l_a: f32 = p_a;
  let l_b: vec2f = p_b;
  return coax_p_v2((l_a - l_b.x), (l_a - l_b.y));
}
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  let l_i: vec3f = p_i;
  let l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_i))));
}
fn coax_p_render_noise2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  var l_ok: bool;
  l_ok = (coax_p_render_noise_coordinate_ok(l_x) && coax_p_render_noise_coordinate_ok(l_y));
  return select(0.0f, coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y))), l_ok);
}
fn coax_p_wg_atu(p_at: i32) -> u32 {
  let l_at: i32 = p_at;
  return bitcast<u32>(coax_at_u(i32(l_at)));
}
fn coax_p_u4(p_x: u32, p_y: u32, p_z: u32, p_w: u32) -> coax_t19 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  let l_z: u32 = p_z;
  let l_w: u32 = p_w;
  return coax_t19(l_x, l_y, l_z, l_w);
}
fn coax_p_wg_half(p_h: u32) -> f32 {
  let l_h: u32 = p_h;
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
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_render_noise_coordinate_ok(p_x: f32) -> bool {
  let l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_noise_cell(p_x: f32) -> u32 {
  let l_x: f32 = p_x;
  var l_bounded: f32;
  l_bounded = coax_p_fmin(coax_p_fmax(l_x, (-0x1.000000p25f)), 0x1.fffffep24f);
  return u32(i32(floor((l_bounded * 0x1.000000p6f))));
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
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  let l_word: u32 = p_word;
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
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_16 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_16(coax_r.x, coax_r.y);
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