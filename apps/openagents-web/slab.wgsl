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
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_14 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t15 {
  x: f32,
  y: vec3f,
  z: f32,
  w: f32,
  f4: f32,
  f5: vec3f,
  f6: vec3f,
}
struct coax_t16 {
  x: vec4f,
  y: f32,
}
struct coax_out_16 {
  @location(0) x: vec4f,
  @builtin(frag_depth) y: f32,
}
struct coax_t19 {
  x: i32,
  y: i32,
}
struct coax_t20 {
  x: u32,
  y: u32,
}
struct coax_t21 {
  x: vec3f,
  y: vec3f,
}
struct coax_t22 {
  x: u32,
  y: u32,
  z: u32,
  w: u32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 3968>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(0) @binding(1) var sans_atlas: texture_2d<f32>;
@group(0) @binding(2) var sans_sampler: sampler;
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
  var l_o: coax_t14;
  l_a = l_in_.x;
  l_o = coax_t14();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_in_.y;
  l_o.w = l_in_.z;
  l_o.f4 = l_in_.w;
  return l_o;
}
fn coax_p_bg_fs(p_frag: vec4f) -> coax_t16 {
  let l_frag: vec4f = p_frag;
  var l_ndc: vec2f;
  var l_aspect: f32;
  var l_d: vec3f;
  var l_o: vec3f;
  var l_ho: vec3f;
  var l_hd: vec3f;
  var l_hit: coax_t15;
  var l_ch: coax_t15;
  var l_wet: bool;
  var l_ln: coax_t15;
  var l_c: vec3f;
  var l_under: f32;
  var l_wy: f32;
  var l_sm: vec4f;
  var l_slot: bool;
  var l_c0: vec3f;
  var l_beyond: f32;
  var l_c_19: vec3f;
  var l_c_20: vec3f;
  var l_sm_21: vec4f;
  l_ndc = coax_p_v2((((l_frag.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_frag.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_aspect = (coax_at_u(i32(0i)) / coax_at_u(i32(1i)));
  l_d = coax_p_wg_normalize3(coax_p_sb_add(coax_p_sb_add(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))), coax_p_sb_mul(coax_p_sb_mul(coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i))), l_ndc.x), (coax_at_u(i32(11i)) * l_aspect))), coax_p_sb_mul(coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i))), (l_ndc.y * coax_at_u(i32(11i))))));
  l_o = coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i)));
  l_ho = coax_p_sb_to_hull(l_o);
  l_hd = coax_p_sb_dir_to_hull(l_d);
  l_hit = coax_p_slab_scene(l_ho, l_hd, false, 0x1.dcd650p29f);
  l_ch = coax_p_slab_walker(l_ho, l_hd, l_hit.x);
  if ((l_ch.x < l_hit.x))
  {
    l_hit = l_ch;
  }
  l_wet = ((coax_at_u(i32(1496i)) > 0x1.000000p-1f) || (coax_at_u(i32(1492i)) != 0.0f));
  if ((coax_at_u(i32(1496i)) > 0x1.000000p-1f))
  {
    l_ln = coax_p_slab_lines(l_ho, l_hd, l_hit.x);
    if ((l_ln.x < l_hit.x))
    {
      l_hit = l_ln;
    }
  }
  if ((l_hit.x < 0x1.dcd650p29f))
  {
    l_c = coax_p_slab_lit(l_hit, l_ho, l_hd);
    l_under = select(0.0f, coax_p_slab_under_len(l_ho, l_hd, l_hit.x), l_wet);
    l_wy = (l_ho.y + (l_hd.y * l_hit.x));
    if ((((coax_at_u(i32(1492i)) != 0.0f) && (abs(l_hit.y.z) > 0x1.ccccccp-1f)) && (abs(((l_ho.z + (l_hd.z * l_hit.x)) - coax_at_u(i32(1493i)))) < 0x1.333334p-1f)))
    {
      l_under = (l_under * 0x1.eb851ep-4f);
      l_wy = 0.0f;
    }
    if ((l_under > 0.0f))
    {
      l_c = coax_p_slab_through_water(l_c, l_under, l_wy, coax_p_fmin(l_ho.y, 0.0f), l_hd.y);
      if ((l_ho.y > 0.0f))
      {
        l_c = coax_p_slab_haze(l_c, l_d, (l_hit.x - l_under));
      }
    }
    else
    {
      l_c = coax_p_slab_haze(l_c, l_d, l_hit.x);
    }
    l_sm = coax_p_slab_smoke(l_ho, l_hd, l_hit.x);
    l_c = coax_p_sb_mix(l_c, coax_p_v3(l_sm.x, l_sm.y, l_sm.z), l_sm.w);
    if ((l_ho.y > 0.0f))
    {
      l_c = coax_p_slab_weather_air(l_c, coax_p_v2(l_frag.x, l_frag.y));
    }
    l_c = coax_p_slab_grade(l_c);
    return coax_t16(coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f), coax_p_slab_depth_of(l_d, l_hit.x));
  }
  l_slot = false;
  if ((((coax_at_u(i32(1492i)) != 0.0f) && (l_hd.y < 0.0f)) && (l_ho.y > 0.0f)))
  {
    l_c0 = coax_p_sb_add(l_ho, coax_p_sb_mul(l_hd, (l_ho.y / (-l_hd.y))));
    l_beyond = (coax_at_u(i32(1492i)) * (l_c0.z - coax_at_u(i32(1493i))));
    l_slot = (((abs(l_c0.x) < (coax_at_u(i32(36i)) + 0x1.400000p4f)) && (l_beyond > 0.0f)) && (l_beyond < 0x1.770000p10f));
  }
  if ((l_slot || ((coax_at_u(i32(1496i)) > 0x1.000000p-1f) && ((l_hd.y < 0.0f) || (l_ho.y < 0.0f)))))
  {
    l_c_19 = coax_p_slab_grade(select(coax_p_slab_sea_open(l_o, l_d), coax_p_slab_deep_colour(coax_p_fmin(l_ho.y, 0.0f), l_hd.y), l_slot));
    return coax_t16(coax_p_v4(l_c_19.x, l_c_19.y, l_c_19.z, 0x1.000000p0f), 0x1.000000p0f);
  }
  l_c_20 = coax_p_slab_haze(coax_p_slab_weather_sky(coax_p_slab_sky(l_d), l_d), l_d, (0x1.388000p15f * (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.99999ap-3f, l_d.y))));
  l_sm_21 = coax_p_slab_smoke(l_ho, l_hd, 0x1.dcd650p29f);
  l_c_20 = coax_p_sb_mix(l_c_20, coax_p_v3(l_sm_21.x, l_sm_21.y, l_sm_21.z), l_sm_21.w);
  l_c_20 = coax_p_slab_grade(coax_p_slab_weather_air(l_c_20, coax_p_v2(l_frag.x, l_frag.y)));
  return coax_t16(coax_p_v4(l_c_20.x, l_c_20.y, l_c_20.z, 0x1.000000p0f), 0x1.000000p0f);
}
fn coax_p_surface_fs(p_v: coax_t12) -> vec4f {
  let l_v: coax_t12 = p_v;
  var l_rel: vec3f;
  var l_dist: f32;
  var l_d: vec3f;
  var l_sec: f32;
  var l_hp: vec3f;
  var l_beyond: f32;
  var l_c: vec3f;
  var l_c_9: vec3f;
  var l_sm: vec4f;
  l_rel = coax_p_wg_sub_f3f3(l_v.z, coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_dist = coax_p_wg_length3(l_rel);
  l_d = coax_p_wg_div_f3f(l_rel, coax_p_fmax(l_dist, 0x1.a36e2ep-14f));
  if ((((coax_at_u(i32(1501i)) > 0x1.000000p-1f) && (abs((l_v.z.x - coax_at_u(i32(1502i)))) < 0x1.8d999ap5f)) && (abs((l_v.z.z - coax_at_u(i32(1503i)))) < 0x1.8b3334p4f)))
  {
    discard;
  }
  l_sec = coax_at_u(i32(1492i));
  if ((l_sec != 0.0f))
  {
    l_hp = coax_p_sb_to_hull(l_v.z);
    l_beyond = (l_sec * (l_hp.z - coax_at_u(i32(1493i))));
    if ((((abs(l_hp.x) < (coax_at_u(i32(36i)) + 0x1.400000p4f)) && (l_beyond > 0.0f)) && (l_beyond < 0x1.770000p10f)))
    {
      discard;
    }
  }
  if ((coax_at_u(i32(1496i)) > 0x1.000000p-1f))
  {
    l_c = coax_p_slab_grade(coax_p_slab_sea_below(l_v.y, l_v.z, l_d, l_dist, coax_p_slab_coarse(l_v.y)));
    return coax_p_wg_make_f4_f3f(l_c, 0x1.000000p0f);
  }
  l_c_9 = coax_p_slab_haze(coax_p_slab_water(l_v.y, l_v.z, l_d, l_dist, coax_at_u(i32(2i))), l_d, (l_dist * 0x1.99999ap-2f));
  l_sm = coax_p_slab_smoke(coax_p_sb_to_hull(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i)))), coax_p_sb_dir_to_hull(l_d), l_dist);
  l_c_9 = coax_p_slab_grade(coax_p_slab_weather_air(coax_p_sb_mix(l_c_9, coax_p_v3(l_sm.x, l_sm.y, l_sm.z), l_sm.w), coax_p_slab_weather_px(l_d)));
  return coax_p_wg_make_f4_f3f(l_c_9, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t14) -> vec4f {
  let l_v: coax_t14 = p_v;
  var l_lod: f32;
  var l_sa: f32;
  var l_q: vec2f;
  var l_out: f32;
  var l_cover: f32;
  var l_rim: f32;
  var l_c: vec3f;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  var l_a: f32;
  if (((l_v.w.w > 0x1.c00000p1f) && (l_v.w.w < 0x1.200000p2f)))
  {
    l_lod = coax_p_clampf(coax_p_wg_log2((0x1.000000p6f / coax_p_fmax(l_v.z.x, 0x1.000000p0f))), 0.0f, 0x1.000000p2f);
    l_sa = coax_pow_bg_vs((textureSampleLevel(sans_atlas, sans_sampler, l_v.y, l_lod)).w, 0x1.b33334p-1f);
    return coax_p_v4(l_v.w.x, l_v.w.y, l_v.w.z, (coax_p_clampf((l_sa * l_v.z.z), 0.0f, 0x1.000000p0f) * coax_at_u(i32(3i))));
  }
  if ((l_v.w.w > 0x1.200000p2f))
  {
    l_q = coax_p_v2((abs(l_v.y.x) - (l_v.z.x - l_v.z.z)), (abs(l_v.y.y) - (l_v.z.y - l_v.z.z)));
    l_out = ((coax_p_wg_length2(coax_p_v2(coax_p_fmax(l_q.x, 0.0f), coax_p_fmax(l_q.y, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_v.z.z);
    l_cover = (0x1.000000p0f - coax_p_smoothstep((-0x1.800000p-1f), 0x1.800000p-1f, l_out));
    l_rim = ((0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.333334p0f, abs((l_out + 0x1.000000p0f)))) * l_v.f4.x);
    l_c = coax_p_sb_mix(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.b33334p-1f), (l_rim * 0x1.333334p-1f));
    return coax_p_v4(l_c.x, l_c.y, l_c.z, (coax_p_clampf((coax_p_fmax((l_cover * l_v.z.w), (l_rim * 0x1.99999ap-1f)) * l_cover), 0.0f, 0x1.000000p0f) * coax_at_u(i32(3i))));
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.70a3d8p-3f);
  l_a = coax_p_clampf(((l_core + l_halo) * l_v.z.z), 0.0f, 0x1.000000p0f);
  return coax_p_v4(l_v.w.x, l_v.w.y, l_v.w.z, (l_a * coax_at_u(i32(3i))));
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
  var l_i: coax_t19;
  var l_acc: vec2f;
  var l_k: i32;
  var l_o: coax_t19;
  var l_c: coax_t20;
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
fn coax_p_sb_mul(p_a: vec3f, p_b: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_sb_add(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_sb_to_hull(p_w: vec3f) -> vec3f {
  let l_w: vec3f = p_w;
  var l_q: vec3f;
  l_q = coax_p_sb_sub(l_w, coax_p_v3(coax_at_u(i32(1459i)), coax_at_u(i32(1463i)), coax_at_u(i32(1467i))));
  return coax_p_sb_dir_to_hull(l_q);
}
fn coax_p_sb_dir_to_hull(p_q: vec3f) -> vec3f {
  let l_q: vec3f = p_q;
  var l_r0: vec3f;
  var l_r1: vec3f;
  var l_r2: vec3f;
  l_r0 = coax_p_sb_p(16i);
  l_r1 = coax_p_sb_p(20i);
  l_r2 = coax_p_sb_p(24i);
  return coax_p_v3((((l_r0.x * l_q.x) + (l_r1.x * l_q.y)) + (l_r2.x * l_q.z)), (((l_r0.y * l_q.x) + (l_r1.y * l_q.y)) + (l_r2.y * l_q.z)), (((l_r0.z * l_q.x) + (l_r1.z * l_q.y)) + (l_r2.z * l_q.z)));
}
fn coax_p_slab_scene(p_o: vec3f, p_d: vec3f, p_masses: bool, p_limit: f32) -> coax_t15 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_masses: bool = p_masses;
  let l_limit: f32 = p_limit;
  var l_hit: coax_t15;
  var l_groups: i32;
  var l_g: i32;
  var l_sec: f32;
  var l_cut: f32;
  var l_ga: i32;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_sp: vec2f;
  var l_first: i32;
  var l_count: i32;
  var l_i: i32;
  var l_at: i32;
  var l_p: vec4f;
  var l_q: vec4f;
  var l_lo: vec3f;
  var l_hi: vec3f;
  var l_sh: i32;
  var l_keep: bool;
  var l_h: vec4f;
  l_hit = coax_t15(l_limit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (-0x1.000000p0f), 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
  l_groups = coax_p_wg_i32_f(coax_at_u(i32(1440i)));
  l_g = select(0i, 1i, (l_masses && (coax_at_u(i32(1500i)) < 0x1.000000p-1f)));
  l_sec = coax_at_u(i32(1492i));
  l_cut = coax_at_u(i32(1493i));
  while ((l_g < l_groups))
  {
    l_ga = (1312i + (i32(l_g) * 8i));
    l_a = coax_p_wg_at4(l_ga);
    l_b = coax_p_wg_at4((l_ga + 4i));
    l_sp = coax_p_slab_span(l_o, l_d, coax_p_v3(l_a.x, l_a.y, l_a.z), coax_p_v3(l_b.x, l_b.y, l_b.z));
    if ((((l_sp.x <= l_sp.y) && (l_sp.y >= 0.0f)) && (l_sp.x < l_hit.x)))
    {
      l_first = coax_p_wg_i32_f(l_a.w);
      l_count = coax_p_wg_i32_f(l_b.w);
      l_i = l_first;
      while ((l_i < (l_first + l_count)))
      {
        l_at = (96i + (i32(l_i) * 8i));
        l_p = coax_p_wg_at4(l_at);
        l_q = coax_p_wg_at4((l_at + 4i));
        l_lo = coax_p_v3(l_p.x, l_p.y, l_p.z);
        l_hi = coax_p_v3(l_q.x, l_q.y, l_q.z);
        l_sh = coax_p_wg_i32_f((l_q.w + 0x1.000000p-1f));
        l_keep = true;
        if ((l_sec != 0.0f))
        {
          if ((l_sh == 0i))
          {
            if ((l_sec > 0.0f))
            {
              l_hi.z = coax_p_fmin(l_hi.z, l_cut);
            }
            else
            {
              l_lo.z = coax_p_fmax(l_lo.z, l_cut);
            }
            l_keep = (l_lo.z < l_hi.z);
          }
          else
          {
            if ((coax_p_wg_i32_f((l_p.w + 0x1.000000p-1f)) != 0i))
            {
              l_keep = ((l_sec * (((l_lo.z + l_hi.z) * 0x1.000000p-1f) - l_cut)) <= 0.0f);
            }
          }
        }
        if ((((coax_p_wg_i32_f((l_p.w + 0x1.000000p-1f)) == 21i) && (coax_at_u(i32(1497i)) > 0x1.000000p-1f)) && (l_lo.z < (-0x1.900000p7f))))
        {
          l_lo.y = (l_hi.y - 0x1.2c0000p8f);
        }
        if (l_keep)
        {
          l_h = coax_p_slab_record_hit(l_o, l_d, l_lo, l_hi, l_sh, l_hit.x);
          if (((((l_sec != 0.0f) && (l_sh != 0i)) && (coax_p_wg_i32_f((l_p.w + 0x1.000000p-1f)) == 0i)) && ((l_sec * ((l_o.z + (l_d.z * l_h.x)) - l_cut)) > 0.0f)))
          {
            l_h.x = 0x1.dcd650p29f;
          }
          if ((l_h.x < l_hit.x))
          {
            l_hit = coax_t15(l_h.x, coax_p_v3(l_h.y, l_h.z, l_h.w), (l_p.w + select(0.0f, 0x1.000000p-2f, (l_sh >= 11i))), l_q.y, f32(l_i), l_lo, l_hi);
          }
        }
        l_i = (l_i + 1i);
      }
    }
    l_g = (l_g + 1i);
  }
  return l_hit;
}
fn coax_p_slab_walker(p_o: vec3f, p_d: vec3f, p_limit: f32) -> coax_t15 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_limit: f32 = p_limit;
  var l_hit: coax_t15;
  var l_feet: vec3f;
  var l_cen: vec3f;
  var l_oc: vec3f;
  var l_bb: f32;
  var l_cc: f32;
  var l_yaw: f32;
  var l_f: vec3f;
  var l_rt: vec3f;
  var l_phase: f32;
  var l_speed: f32;
  var l_air: f32;
  var l_sw: f32;
  var l_k: i32;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_r: f32;
  var l_part: f32;
  var l_sd: f32;
  var l_s2: f32;
  var l_sd_24: f32;
  var l_s2_25: f32;
  var l_wa: vec3f;
  var l_wb: vec3f;
  var l_h: vec4f;
  l_hit = coax_t15(l_limit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (-0x1.000000p0f), 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
  if ((coax_at_u(i32(1474i)) < 0x1.000000p-1f))
  {
    return l_hit;
  }
  l_feet = coax_p_sb_p(28i);
  l_cen = coax_p_sb_add(l_feet, coax_p_v3(0.0f, 0x1.ccccccp-1f, 0.0f));
  l_oc = coax_p_sb_sub(l_o, l_cen);
  l_bb = coax_p_wg_dot3(l_oc, l_d);
  l_cc = (coax_p_wg_dot3(l_oc, l_oc) - 0x1.70a3d8p0f);
  if ((((l_bb * l_bb) - l_cc) < 0.0f))
  {
    return l_hit;
  }
  l_yaw = coax_at_u(i32(1471i));
  l_f = coax_p_v3(sin(l_yaw), 0.0f, cos(l_yaw));
  l_rt = coax_p_v3((-cos(l_yaw)), 0.0f, sin(l_yaw));
  l_phase = coax_at_u(i32(1472i));
  l_speed = coax_p_clampf((coax_at_u(i32(1473i)) / 0x1.800000p2f), 0.0f, 0x1.000000p0f);
  l_air = coax_at_u(i32(1475i));
  l_sw = ((sin(l_phase) * (0x1.333334p-3f + (0x1.000000p-1f * l_speed))) * (0x1.000000p0f - (l_air * 0x1.333334p-1f)));
  l_k = 0i;
  while ((l_k < 7i))
  {
    l_a = coax_p_v3(0.0f, 0.0f, 0.0f);
    l_b = l_a;
    l_r = 0x1.99999ap-4f;
    l_part = 0x1.000000p0f;
    if ((l_k == 0i))
    {
      l_a = coax_p_v3(0.0f, 0x1.e66666p-1f, 0.0f);
      l_b = coax_p_v3(0.0f, 0x1.6147aep0f, 0.0f);
      l_r = 0x1.851eb8p-3f;
      l_part = 0x1.000000p1f;
    }
    if ((l_k == 1i))
    {
      l_a = coax_p_v3(0.0f, 0x1.9eb852p0f, 0x1.47ae14p-6f);
      l_b = coax_p_v3(0.0f, 0x1.a8f5c2p0f, 0x1.47ae14p-6f);
      l_r = 0x1.0a3d70p-3f;
      l_part = 0x1.000000p0f;
    }
    if (((l_k == 2i) || (l_k == 3i)))
    {
      l_sd = select((-0x1.000000p0f), 0x1.000000p0f, (l_k == 2i));
      l_s2 = (l_sw * l_sd);
      l_a = coax_p_v3((0x1.99999ap-4f * l_sd), 0x1.ccccccp-1f, 0.0f);
      l_b = coax_p_v3((0x1.99999ap-4f * l_sd), (0x1.ccccccp-1f - (cos(l_s2) * 0x1.a3d70ap-1f)), (sin(l_s2) * 0x1.a3d70ap-1f));
      l_r = 0x1.5c28f6p-4f;
      l_part = 0x1.800000p1f;
    }
    if (((l_k == 4i) || (l_k == 5i)))
    {
      l_sd_24 = select((-0x1.000000p0f), 0x1.000000p0f, (l_k == 4i));
      l_s2_25 = (((-l_sw) * l_sd_24) * 0x1.99999ap-1f);
      l_a = coax_p_v3((0x1.000000p-2f * l_sd_24), 0x1.5c28f6p0f, 0.0f);
      l_b = coax_p_v3((0x1.1eb852p-2f * l_sd_24), (0x1.5c28f6p0f - (cos(l_s2_25) * 0x1.333334p-1f)), (sin(l_s2_25) * 0x1.333334p-1f));
      l_r = 0x1.eb851ep-5f;
      l_part = 0x1.000000p1f;
    }
    if ((l_k == 6i))
    {
      l_a = coax_p_v3(0.0f, 0x1.b851ecp0f, (-0x1.47ae14p-6f));
      l_b = coax_p_v3(0.0f, 0x1.bd70a4p0f, (-0x1.eb851ep-6f));
      l_r = 0x1.d70a3ep-4f;
      l_part = 0x1.000000p2f;
    }
    l_wa = coax_p_sb_add(l_feet, coax_p_sb_add(coax_p_sb_add(coax_p_sb_mul(l_rt, l_a.x), coax_p_v3(0.0f, l_a.y, 0.0f)), coax_p_sb_mul(l_f, l_a.z)));
    l_wb = coax_p_sb_add(l_feet, coax_p_sb_add(coax_p_sb_add(coax_p_sb_mul(l_rt, l_b.x), coax_p_v3(0.0f, l_b.y, 0.0f)), coax_p_sb_mul(l_f, l_b.z)));
    l_h = coax_p_slab_capsule(l_o, l_d, l_wa, l_wb, l_r);
    if ((l_h.x < l_hit.x))
    {
      l_hit = coax_t15(l_h.x, coax_p_v3(l_h.y, l_h.z, l_h.w), (0x1.400000p5f + l_part), 0.0f, (-0x1.000000p0f), l_feet, l_feet);
    }
    l_k = (l_k + 1i);
  }
  return l_hit;
}
fn coax_p_slab_lines(p_o: vec3f, p_d: vec3f, p_limit: f32) -> coax_t15 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_limit: f32 = p_limit;
  var l_hit: coax_t15;
  var l_hl: f32;
  var l_hb: f32;
  var l_keel: f32;
  var l_i: i32;
  var l_corner: i32;
  var l_k: i32;
  var l_sx: f32;
  var l_sz: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_dl: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_r: f32;
  var l_ax: f32;
  var l_az: f32;
  var l_h: vec4f;
  l_hit = coax_t15(l_limit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (-0x1.000000p0f), 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
  l_hl = coax_at_u(i32(36i));
  l_hb = coax_at_u(i32(37i));
  l_keel = coax_at_u(i32(39i));
  l_i = 0i;
  while ((l_i < 16i))
  {
    l_corner = (l_i / 4i);
    l_k = (l_i % 4i);
    l_sx = select(0x1.000000p0f, (-0x1.000000p0f), ((l_corner & 1i) == 0i));
    l_sz = select(0x1.000000p0f, (-0x1.000000p0f), ((l_corner & 2i) == 0i));
    l_fx = (l_sx * (l_hl - 0x1.800000p3f));
    l_fz = (l_sz * (l_hb - 0x1.800000p3f));
    l_dl = sqrt(((l_hl * l_hl) + (l_hb * l_hb)));
    l_dx = ((l_sx * l_hl) / l_dl);
    l_dz = ((l_sz * l_hb) / l_dl);
    l_r = ((f32(l_k) - 0x1.800000p0f) * 0x1.0c1524p-1f);
    l_ax = ((l_dx * cos(l_r)) - (l_dz * sin(l_r)));
    l_az = ((l_dz * cos(l_r)) + (l_dx * sin(l_r)));
    l_h = coax_p_slab_capsule(l_o, l_d, coax_p_v3(l_fx, l_keel, l_fz), coax_p_v3((l_fx + (l_ax * 0x1.f40000p10f)), (-0x1.770000p10f), (l_fz + (l_az * 0x1.f40000p10f))), 0x1.333334p-1f);
    if ((l_h.x < l_hit.x))
    {
      l_hit = coax_t15(l_h.x, coax_p_v3(l_h.y, l_h.z, l_h.w), 0x1.680000p5f, 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
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
  var l_scol: vec3f;
  var l_amb: f32;
  var l_pt: coax_t21;
  var l_base: vec3f;
  var l_ndl: f32;
  var l_shade: f32;
  var l_s: coax_t15;
  var l_ch: coax_t15;
  var l_skyc: vec3f;
  var l_occ: f32;
  var l_light: vec3f;
  var l_c: vec3f;
  var l_pulse: f32;
  l_w = coax_p_sb_add(l_o, coax_p_sb_mul(l_d, l_hit.x));
  l_n = l_hit.y;
  l_sun = coax_p_sb_p(40i);
  l_scol = coax_p_sb_p(4i);
  l_amb = coax_at_u(i32(1447i));
  l_pt = coax_p_slab_paint(l_hit, l_w, l_d);
  l_base = l_pt.x;
  l_ndl = coax_p_fmax(coax_p_wg_dot3(l_n, l_sun), 0.0f);
  l_shade = 0x1.000000p0f;
  if ((((l_ndl > 0.0f) && (coax_at_u(i32(5i)) < 0x1.000000p-1f)) && (l_hit.z < 0x1.380000p5f)))
  {
    l_s = coax_p_slab_scene(coax_p_sb_add(l_w, coax_p_sb_mul(l_n, 0x1.47ae14p-4f)), l_sun, true, 0x1.c20000p9f);
    if ((l_s.x < 0x1.c20000p9f))
    {
      l_shade = 0.0f;
    }
    if ((l_shade > 0.0f))
    {
      l_ch = coax_p_slab_walker(coax_p_sb_add(l_w, coax_p_sb_mul(l_n, 0x1.99999ap-5f)), l_sun, 0x1.e00000p5f);
      if ((l_ch.x < 0x1.e00000p5f))
      {
        l_shade = 0.0f;
      }
    }
  }
  l_skyc = coax_p_sb_mix(coax_p_sb_p(12i), coax_p_sb_p(8i), (0x1.000000p-1f + (0x1.000000p-1f * l_n.y)));
  l_occ = select(0x1.000000p0f, (0x1.70a3d8p-1f + (0x1.1eb852p-2f * coax_p_smoothstep(0.0f, 0x1.800000p2f, (l_w.y - coax_at_u(i32(38i)))))), (abs(l_n.y) < 0x1.000000p-1f));
  if ((l_hit.z < 0x1.000000p-1f))
  {
    l_occ = 0x1.000000p0f;
  }
  l_light = coax_p_sb_add(coax_p_sb_mul(l_skyc, ((((0x1.70a3d8p-2f + (0x1.99999ap-3f * l_n.y)) * (0x1.000000p0f + (0x1.99999ap-4f * coax_at_u(i32(1483i))))) * l_occ) * (0x1.666666p-2f + (0x1.4cccccp-1f * l_amb)))), coax_p_sb_mul(l_scol, ((l_ndl * l_shade) * (0x1.0cccccp0f + (0x1.ccccccp-1f * coax_at_u(i32(1483i)))))));
  l_c = coax_p_sb_mulv(l_base, l_light);
  if ((l_n.y < 0x1.333334p-2f))
  {
    l_c = coax_p_sb_add(l_c, coax_p_sb_mul(coax_p_sb_mulv(l_base, coax_p_v3(0x1.99999ap-5f, 0x1.70a3d8p-4f, 0x1.99999ap-4f)), (l_amb * coax_p_smoothstep(0x1.e00000p4f, 0.0f, l_w.y))));
  }
  l_c = coax_p_sb_add(l_c, l_pt.y);
  if ((l_hit.f4 > (-0x1.000000p-1f)))
  {
    if ((abs((l_hit.f4 - coax_at_u(i32(1476i)))) < 0x1.000000p-1f))
    {
      l_pulse = (0x1.000000p-1f + (0x1.000000p-1f * sin((coax_at_u(i32(1478i)) * 0x1.400000p2f))));
      l_c = coax_p_sb_add(coax_p_sb_mul(l_c, 0x1.19999ap0f), coax_p_sb_mul(coax_p_v3(0x1.000000p0f, 0x1.3d70a4p-1f, 0x1.99999ap-3f), (0x1.eb851ep-4f + (0x1.eb851ep-4f * l_pulse))));
    }
    else
    {
      if ((abs((l_hit.f4 - coax_at_u(i32(1477i)))) < 0x1.000000p-1f))
      {
        l_c = coax_p_sb_add(coax_p_sb_mul(l_c, 0x1.0f5c28p0f), coax_p_v3(0x1.eb851ep-5f, 0x1.99999ap-5f, 0x1.eb851ep-6f));
      }
    }
  }
  return l_c;
}
fn coax_p_slab_under_len(p_o: vec3f, p_d: vec3f, p_t: f32) -> f32 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_t: f32 = p_t;
  if ((l_o.y < 0.0f))
  {
    if ((l_d.y > 0x1.4f8b58p-17f))
    {
      return coax_p_fmin(l_t, ((-l_o.y) / l_d.y));
    }
    return l_t;
  }
  if ((l_d.y < (-0x1.4f8b58p-17f)))
  {
    return coax_p_fmax((l_t - (l_o.y / (-l_d.y))), 0.0f);
  }
  return 0.0f;
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b < l_a));
}
fn coax_p_slab_through_water(p_c: vec3f, p_path: f32, p_y: f32, p_eye: f32, p_up: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_path: f32 = p_path;
  let l_y: f32 = p_y;
  let l_eye: f32 = p_eye;
  let l_up: f32 = p_up;
  return coax_p_slab_sea_through(l_c, l_path, l_y, l_eye, l_up);
}
fn coax_p_slab_haze(p_c: vec3f, p_d: vec3f, p_dist: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  var l_a: f32;
  var l_sun: vec3f;
  var l_tint: vec3f;
  l_a = (0x1.000000p0f - exp(((-l_dist) / coax_at_u(i32(1455i)))));
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_tint = coax_p_sb_mix(coax_p_sb_p(12i), coax_p_sb_add(coax_p_sb_p(12i), coax_p_sb_mul(coax_p_sb_p(4i), 0x1.000000p-2f)), coax_p_sb_pow(coax_p_fmax(coax_p_wg_dot3(l_d, l_sun), 0.0f), 0x1.400000p2f));
  return coax_p_sb_mix(l_c, l_tint, (l_a * 0x1.d70a3ep-1f));
}
fn coax_p_slab_smoke(p_o: vec3f, p_d: vec3f, p_limit: f32) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_limit: f32 = p_limit;
  var l_amount: f32;
  var l_first: i32;
  var l_count: i32;
  var l_acc: vec3f;
  var l_alpha: f32;
  var l_sun: vec3f;
  var l_scol: vec3f;
  var l_wind: f32;
  var l_rise: f32;
  var l_v: vec3f;
  var l_L: f32;
  var l_i: i32;
  var l_at: i32;
  var l_p: vec4f;
  var l_q: vec4f;
  var l_a: vec3f;
  var l_w0: vec3f;
  var l_b: f32;
  var l_dd: f32;
  var l_e: f32;
  var l_den: f32;
  var l_s: f32;
  var l_pa: vec3f;
  var l_t: f32;
  var l_dist: f32;
  var l_r: f32;
  var l_billow: f32;
  var l_dens: f32;
  var l_a1: f32;
  var l_fwd: f32;
  var l_sc: vec3f;
  l_amount = coax_at_u(i32(1443i));
  l_first = coax_p_wg_i32_f(coax_at_u(i32(1441i)));
  l_count = coax_p_wg_i32_f(coax_at_u(i32(1442i)));
  l_acc = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_alpha = 0.0f;
  if (((l_amount < 0x1.47ae14p-7f) || (l_first < 0i)))
  {
    return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  }
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_scol = coax_p_sb_p(4i);
  l_wind = coax_at_u(i32(34i));
  l_rise = (0x1.ccccccp0f / (0x1.000000p0f + (l_wind * 0x1.eb851ep-4f)));
  l_v = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(32i)), l_rise, coax_at_u(i32(33i))));
  l_L = (0x1.7c0000p7f + (l_wind * 0x1.c00000p2f));
  l_i = l_first;
  while ((l_i < (l_first + l_count)))
  {
    l_at = (96i + (i32(l_i) * 8i));
    l_p = coax_p_wg_at4(l_at);
    l_q = coax_p_wg_at4((l_at + 4i));
    l_a = coax_p_v3(((l_p.x + l_q.x) * 0x1.000000p-1f), l_q.y, ((l_p.z + l_q.z) * 0x1.000000p-1f));
    l_w0 = coax_p_sb_sub(l_o, l_a);
    l_b = coax_p_wg_dot3(l_d, l_v);
    l_dd = coax_p_wg_dot3(l_d, l_w0);
    l_e = coax_p_wg_dot3(l_v, l_w0);
    l_den = coax_p_fmax((0x1.000000p0f - (l_b * l_b)), 0x1.4f8b58p-17f);
    l_s = coax_p_clampf(((l_e - (l_b * l_dd)) / l_den), 0.0f, l_L);
    l_pa = coax_p_sb_add(l_a, coax_p_sb_mul(l_v, l_s));
    l_t = coax_p_wg_dot3(coax_p_sb_sub(l_pa, l_o), l_d);
    if ((((l_t > 0.0f) && (l_t < l_limit)) && ((l_q.y - coax_at_u(i32(38i))) > 0x1.680000p6f)))
    {
      l_dist = coax_p_wg_length3(coax_p_sb_sub(coax_p_sb_add(l_o, coax_p_sb_mul(l_d, l_t)), l_pa));
      l_r = (0x1.800000p1f + (l_s * 0x1.eb851ep-3f));
      l_billow = (0x1.19999ap-1f + (0x1.ccccccp-2f * coax_p_slab_noise(coax_p_v2(((l_s * 0x1.eb851ep-5f) - (coax_at_u(i32(2i)) * 0x1.000000p-1f)), ((f32(l_i) * 0x1.d9999ap1f) + (l_dist * 0x1.47ae14p-4f))))));
      l_dens = (((exp(((-((l_dist / l_r) * (l_dist / l_r))) * 0x1.b33334p0f)) * coax_p_sb_pow((0x1.000000p0f - (l_s / l_L)), 0x1.666666p0f)) * l_billow) * l_amount);
      l_a1 = ((0x1.000000p0f - exp(((-l_dens) * 0x1.c00000p1f))) * 0x1.e66666p-1f);
      l_fwd = coax_p_sb_pow(coax_p_fmax(coax_p_wg_dot3(l_d, l_sun), 0.0f), 0x1.000000p2f);
      l_sc = coax_p_sb_add(coax_p_sb_mul(coax_p_v3(0x1.dc28f6p-1f, 0x1.d70a3ep-1f, 0x1.ccccccp-1f), (0x1.333334p-2f + (0x1.333334p-1f * coax_at_u(i32(1447i))))), coax_p_sb_mul(l_scol, (0x1.333334p-2f + (0x1.ccccccp-2f * l_fwd))));
      l_sc = coax_p_sb_mul(l_sc, (0x1.ccccccp-1f + (0x1.99999ap-4f * l_billow)));
      l_acc = coax_p_sb_add(coax_p_sb_mul(l_acc, (0x1.000000p0f - l_a1)), coax_p_sb_mul(l_sc, l_a1));
      l_alpha = (l_alpha + (l_a1 * (0x1.000000p0f - l_alpha)));
    }
    l_i = (l_i + 1i);
  }
  return coax_p_v4(l_acc.x, l_acc.y, l_acc.z, l_alpha);
}
fn coax_p_sb_mix(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_t: f32 = p_t;
  return coax_p_v3(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_slab_weather_air(p_c: vec3f, p_frag: vec2f) -> vec3f {
  let l_c: vec3f = p_c;
  let l_frag: vec2f = p_frag;
  var l_rain: f32;
  var l_px: vec2f;
  var l_lean: f32;
  var l_x: f32;
  var l_s1: f32;
  var l_streak: f32;
  var l_s2: f32;
  var l_grey: vec3f;
  l_rain = coax_at_u(i32(15857i));
  if ((l_rain <= 0x1.47ae14p-7f))
  {
    return l_c;
  }
  l_px = coax_p_wg_div_f2f(l_frag, coax_at_u(i32(7i)));
  l_lean = coax_p_clampf(((((coax_at_u(i32(32i)) * coax_at_u(i32(16i))) + (coax_at_u(i32(33i)) * coax_at_u(i32(18i)))) * coax_at_u(i32(34i))) * 0x1.eb851ep-6f), (-0x1.ccccccp-1f), 0x1.ccccccp-1f);
  l_x = (l_px.x + (l_px.y * l_lean));
  l_s1 = coax_p_slab_noise(coax_p_v2((l_x * 0x1.000000p-1f), ((l_px.y * 0x1.70a3d8p-5f) - (coax_at_u(i32(2i)) * 0x1.c00000p3f))));
  l_streak = (coax_p_smoothstep(0x1.b851ecp-1f, 0x1.f5c290p-1f, l_s1) * 0x1.000000p-1f);
  if ((coax_at_u(i32(5i)) < 0x1.000000p-1f))
  {
    l_s2 = coax_p_slab_noise(coax_p_v2(((l_x * 0x1.ae147ap-3f) + 0x1.f00000p4f), ((l_px.y * 0x1.6872b0p-6f) - (coax_at_u(i32(2i)) * 0x1.200000p3f))));
    l_streak = (l_streak + (coax_p_smoothstep(0x1.bd70a4p-1f, 0x1.f5c290p-1f, l_s2) * 0x1.666666p-2f));
  }
  l_grey = coax_p_sb_mul(coax_p_sb_add(coax_p_sb_p(8i), coax_p_sb_p(12i)), 0x1.3d70a4p-1f);
  return coax_p_sb_mix(l_c, coax_p_sb_add(l_grey, coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-4f, 0x1.c28f5cp-4f)), coax_p_clampf(((l_streak * l_rain) * 0x1.851eb8p-2f), 0.0f, 0x1.000000p-1f));
}
fn coax_p_slab_grade(p_c0: vec3f) -> vec3f {
  let l_c0: vec3f = p_c0;
  var l_g: f32;
  var l_lift: f32;
  var l_c: vec3f;
  var l_l: f32;
  var l_sep: vec3f;
  l_g = coax_at_u(i32(1483i));
  l_lift = (0x1.000000p0f + (0x1.800000p-1f * l_g));
  l_c = coax_p_v3(coax_p_slab_shoulder((l_c0.x * l_lift)), coax_p_slab_shoulder((l_c0.y * l_lift)), coax_p_slab_shoulder((l_c0.z * l_lift)));
  l_l = (((l_c.x * 0x1.333334p-2f) + (l_c.y * 0x1.2e147ap-1f)) + (l_c.z * 0x1.c28f5cp-4f));
  l_sep = coax_p_v3((l_l * 0x1.147ae2p0f), (l_l * 0x1.e66666p-1f), (l_l * 0x1.8f5c28p-1f));
  return coax_p_sb_mix(l_c, coax_p_sb_add(coax_p_sb_mul(l_c, 0x1.000000p-1f), coax_p_sb_mul(l_sep, 0x1.19999ap-1f)), (l_g * 0x1.800000p-1f));
}
fn coax_p_slab_depth_of(p_d: vec3f, p_along: f32) -> f32 {
  let l_d: vec3f = p_d;
  let l_along: f32 = p_along;
  return coax_p_clampf((0x1.000000p0f - (0x1.000000p-1f / coax_p_fmax((l_along * coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))))), 0x1.000000p-1f))), 0.0f, 0x1.000000p0f);
}
fn coax_p_slab_deep_colour(p_y: f32, p_up: f32) -> vec3f {
  let l_y: f32 = p_y;
  let l_up: f32 = p_up;
  return coax_p_slab_sea_out(coax_p_slab_sea_fog(l_y, l_up));
}
fn coax_p_slab_sea_open(p_o: vec3f, p_d: vec3f) -> vec3f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  var l_eye: f32;
  var l_fog: vec3f;
  var l_bed: f32;
  var l_reach: f32;
  var l_hit: vec3f;
  var l_grain: f32;
  var l_net: f32;
  var l_sand: vec3f;
  var l_day: vec3f;
  var l_tr: vec3f;
  var l_shafts: f32;
  l_eye = coax_p_fmin(l_o.y, 0.0f);
  l_fog = coax_p_slab_sea_fog(l_eye, l_d.y);
  l_bed = coax_at_u(i32(15863i));
  if (((l_bed > 0.0f) && (l_d.y < (-0x1.a36e2ep-14f))))
  {
    l_reach = ((l_o.y + l_bed) / (-l_d.y));
    l_hit = coax_p_sb_add(l_o, coax_p_sb_mul(l_d, l_reach));
    l_grain = ((coax_p_slab_noise(coax_p_v2((l_hit.x * 0x1.99999ap-5f), (l_hit.z * 0x1.99999ap-5f))) * 0x1.333334p-1f) + (coax_p_slab_noise(coax_p_v2((l_hit.x * 0x1.3d70a4p-2f), (l_hit.z * 0x1.3d70a4p-2f))) * 0x1.99999ap-2f));
    l_net = coax_p_wo_caustic(coax_p_v2((l_hit.x * 0x1.70a3d8p-4f), (l_hit.z * 0x1.70a3d8p-4f)), (coax_at_u(i32(2i)) * 0x1.333334p-1f));
    l_sand = coax_p_sb_mul(coax_p_v3(0x1.d70a3ep-2f, 0x1.a3d70ap-2f, 0x1.3d70a4p-2f), ((0x1.800000p-1f + (0x1.99999ap-2f * l_grain)) * (0x1.99999ap-1f + (0x1.666666p-1f * l_net))));
    l_day = coax_p_sb_add(coax_p_slab_sea_daylight(l_bed), coax_p_sb_mul(coax_p_slab_sea_daylight(0.0f), coax_at_u(i32(15864i))));
    l_tr = coax_p_slab_sea_beam(l_reach);
    l_fog = coax_p_sb_add(coax_p_sb_mulv(coax_p_sb_mulv(l_sand, l_day), l_tr), coax_p_sb_mulv(l_fog, coax_p_v3((0x1.000000p0f - l_tr.x), (0x1.000000p0f - l_tr.y), (0x1.000000p0f - l_tr.z))));
  }
  l_shafts = coax_p_slab_sea_shafts(l_o, l_d, 0x1.2c0000p9f);
  return coax_p_slab_sea_out(coax_p_sb_mul(l_fog, (0x1.000000p0f + (0x1.99999ap0f * l_shafts))));
}
fn coax_p_slab_sky(p_d: vec3f) -> vec3f {
  let l_d: vec3f = p_d;
  var l_up: f32;
  var l_zenith: vec3f;
  var l_horizon: vec3f;
  var l_c: vec3f;
  var l_sun: vec3f;
  var l_scol: vec3f;
  var l_s: f32;
  var l_gold: f32;
  var l_q: vec2f;
  var l_q2: vec2f;
  var l_q3: vec2f;
  var l_fbm: f32;
  var l_cover: f32;
  var l_cl: f32;
  var l_lit: vec3f;
  var l_night: f32;
  var l_g: vec2f;
  var l_st: f32;
  l_up = coax_p_clampf(l_d.y, 0.0f, 0x1.000000p0f);
  l_zenith = coax_p_sb_p(8i);
  l_horizon = coax_p_sb_p(12i);
  l_c = coax_p_sb_mix(l_horizon, l_zenith, coax_p_sb_pow(l_up, 0x1.ccccccp-2f));
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_scol = coax_p_sb_p(4i);
  l_s = coax_p_fmax(coax_p_wg_dot3(l_d, l_sun), 0.0f);
  l_gold = coax_at_u(i32(1483i));
  l_c = coax_p_sb_add(l_c, coax_p_sb_mul(l_scol, (((coax_p_sb_pow(l_s, 0x1.800000p2f) * (0x1.70a3d8p-3f + (0x1.000000p-1f * l_gold))) + (coax_p_sb_pow(l_s, 0x1.e00000p5f) * 0x1.666666p-2f)) + (coax_p_smoothstep(0x1.ffb15cp-1f, 0x1.ffdf3cp-1f, l_s) * 0x1.800000p2f))));
  if ((l_d.y < 0.0f))
  {
    l_c = coax_p_sb_mix(l_c, coax_p_sb_mul(l_horizon, 0x1.99999ap-1f), coax_p_clampf(((-l_d.y) * 0x1.800000p2f), 0.0f, 0x1.000000p0f));
  }
  if ((l_d.y > 0.0f))
  {
    l_q = coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), (l_d.y + 0x1.70a3d8p-3f)), 0x1.333334p0f);
    l_q2 = coax_p_v2((((l_q.x * 0x1.b5c290p0f) - (l_q.y * 0x1.19999ap0f)) + 0x1.8cccccp1f), (((l_q.x * 0x1.19999ap0f) + (l_q.y * 0x1.b5c290p0f)) - 0x1.b33334p0f));
    l_q3 = coax_p_v2((((l_q.x * 0x1.333334p0f) - (l_q.y * 0x1.ecccccp1f)) - 0x1.533334p2f), (((l_q.x * 0x1.ecccccp1f) + (l_q.y * 0x1.333334p0f)) + 0x1.733334p1f));
    l_fbm = (((coax_p_slab_noise(l_q) * 0x1.000000p-1f) + (coax_p_slab_noise(l_q2) * 0x1.333334p-2f)) + (coax_p_slab_noise(l_q3) * 0x1.99999ap-3f));
    l_cover = coax_at_u(i32(1451i));
    l_cl = coax_p_smoothstep((0x1.3d70a4p-1f - (l_cover * 0x1.000000p-2f)), (0x1.b851ecp-1f - (l_cover * 0x1.99999ap-3f)), l_fbm);
    l_lit = coax_p_sb_add(coax_p_sb_mul(l_horizon, 0x1.ccccccp-1f), coax_p_sb_mul(l_scol, (0x1.666666p-2f + (0x1.333334p0f * coax_p_sb_pow(l_s, 0x1.800000p1f)))));
    l_c = coax_p_sb_mix(l_c, l_lit, ((l_cl * 0x1.333334p-1f) * coax_p_smoothstep(0x1.47ae14p-6f, 0x1.000000p-2f, l_d.y)));
  }
  l_night = coax_at_u(i32(1479i));
  if (((l_night > 0x1.333334p-2f) && (l_d.y > 0x1.99999ap-5f)))
  {
    l_g = coax_p_wg_floor_f2(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), (l_d.y + 0x1.333334p-2f)), 0x1.680000p7f));
    l_st = coax_p_stepf(0x1.fdf3b6p-1f, coax_p_slab_hash2(l_g));
    l_c = coax_p_sb_add(l_c, coax_p_sb_mul(coax_p_v3(0x1.99999ap-1f, 0x1.b33334p-1f, 0x1.000000p0f), (l_st * (l_night - 0x1.333334p-2f))));
  }
  return l_c;
}
fn coax_p_slab_weather_sky(p_c: vec3f, p_d: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  var l_cover: f32;
  var l_q: vec2f;
  var l_drift: vec2f;
  var l_a: f32;
  var l_b: f32;
  var l_deck: f32;
  var l_flash: f32;
  var l_shade: f32;
  l_cover = coax_at_u(i32(15856i));
  if (((l_cover <= 0x1.47ae14p-6f) || (l_d.y <= 0.0f)))
  {
    return l_c;
  }
  l_q = coax_p_wg_div_f2f(coax_p_v2(l_d.x, l_d.z), (l_d.y + 0x1.eb851ep-4f));
  l_drift = coax_p_wg_mul_f2f(coax_p_v2(coax_at_u(i32(32i)), coax_at_u(i32(33i))), (coax_at_u(i32(2i)) * 0x1.47ae14p-6f));
  l_a = coax_p_slab_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_q, 0x1.666666p-1f), l_drift));
  l_b = coax_p_slab_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(((l_q.x * 0x1.99999ap0f) - (l_q.y * 0x1.19999ap0f)), ((l_q.x * 0x1.19999ap0f) + (l_q.y * 0x1.99999ap0f))), 0x1.4cccccp0f), coax_p_wg_mul_f2f(l_drift, 0x1.b33334p0f)));
  l_deck = ((l_a * 0x1.3d70a4p-1f) + (l_b * 0x1.851eb8p-2f));
  l_flash = (coax_at_u(i32(15859i)) * coax_p_smoothstep(0x1.666666p-2f, 0x1.99999ap-1f, l_a));
  l_shade = coax_p_mix(0x1.000000p0f, (0x1.3d70a4p-1f + (0x1.851eb8p-1f * l_deck)), (l_cover * (0x1.19999ap-1f + (0x1.ccccccp-2f * coax_at_u(i32(15857i))))));
  return coax_p_sb_add(coax_p_sb_mul(l_c, l_shade), coax_p_sb_mul(coax_p_v3(0x1.800000p-1f, 0x1.99999ap-1f, 0x1.000000p0f), (l_flash * 0x1.ccccccp-1f)));
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
fn coax_p_slab_coarse(p_p: vec2f) -> vec4f {
  let l_p: vec2f = p_p;
  var l_q: vec2f;
  var l_f: vec2f;
  var l_i: coax_t19;
  var l_acc: vec4f;
  var l_k: i32;
  var l_o: coax_t19;
  var l_c: coax_t20;
  var l_t: u32;
  var l_aw: u32;
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
      l_aw = coax_p_slab_word((0u + (2u * l_t)));
      l_a = coax_p_wg_unpack2x16float(l_aw);
      l_b = coax_p_wg_unpack2x16float(coax_p_slab_word(((0u + (2u * l_t)) + 1u)));
      l_w = (select((0x1.000000p0f - l_f.x), l_f.x, (l_o.x == 1i)) * select((0x1.000000p0f - l_f.y), l_f.y, (l_o.y == 1i)));
      l_acc = coax_p_wg_add_f4f4(l_acc, coax_p_wg_mul_ff4(l_w, coax_p_v4((l_a.x * coax_at_u(i32(40i))), (f32(((l_aw >> u32(16u)) & 255u)) / 0x1.fe0000p7f), (l_b.x * coax_at_u(i32(41i))), (l_b.y * coax_at_u(i32(41i))))));
      l_k = (l_k + 1i);
    }
  }
  return l_acc;
}
fn coax_p_slab_sea_below(p_p: vec2f, p_world: vec3f, p_d: vec3f, p_dist: f32, p_c: vec4f) -> vec3f {
  let l_p: vec2f = p_p;
  let l_world: vec3f = p_world;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  let l_c: vec4f = p_c;
  var l_slope: vec2f;
  var l_n: vec3f;
  var l_ci: f32;
  var l_back: f32;
  var l_st2: f32;
  var l_co: f32;
  var l_out: vec3f;
  var l_sk: vec3f;
  var l_sun: vec3f;
  var l_sky: vec3f;
  var l_glare: f32;
  var l_mirror: vec3f;
  var l_col: vec3f;
  var l_tr: vec3f;
  var l_fog: vec3f;
  var l_shafts: f32;
  l_slope = coax_p_v2(l_c.z, l_c.w);
  if ((coax_at_u(i32(5i)) < 0x1.000000p-1f))
  {
    l_slope = coax_p_wg_add_f2f2(l_slope, coax_p_slab_sea_fine(l_p, coax_at_u(i32(30i)), 0u, coax_at_u(i32(42i))));
  }
  l_n = coax_p_wg_normalize3(coax_p_v3((-l_slope.x), 0x1.000000p0f, (-l_slope.y)));
  l_ci = coax_p_fmax(coax_p_wg_dot3(l_d, l_n), 0.0f);
  l_back = coax_p_wo_below(l_ci);
  l_st2 = ((0x1.547ae2p0f * 0x1.547ae2p0f) * (0x1.000000p0f - (l_ci * l_ci)));
  l_co = sqrt(coax_p_fmax((0x1.000000p0f - l_st2), 0.0f));
  l_out = coax_p_wg_normalize3(coax_p_sb_add(coax_p_sb_mul(coax_p_sb_sub(l_d, coax_p_sb_mul(l_n, l_ci)), 0x1.547ae2p0f), coax_p_sb_mul(l_n, coax_p_fmax(l_co, 0x1.eb851ep-6f))));
  l_sk = coax_p_slab_sky(l_out);
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_sky = coax_p_sb_mul(coax_p_slab_sea_lin(coax_p_v3(coax_p_fmin(l_sk.x, 0x1.666666p0f), coax_p_fmin(l_sk.y, 0x1.666666p0f), coax_p_fmin(l_sk.z, 0x1.666666p0f))), (0x1.000000p0f - l_back));
  l_glare = (coax_p_sb_pow(coax_p_fmax(coax_p_wg_dot3(l_out, l_sun), 0.0f), 0x1.680000p7f) * (0x1.000000p0f - l_back));
  l_mirror = coax_p_sb_mul(coax_p_slab_sea_fog(coax_p_fmin(coax_at_u(i32(9i)), 0.0f), (-0x1.99999ap-2f)), l_back);
  l_col = coax_p_sb_add(coax_p_sb_add(l_sky, l_mirror), coax_p_sb_mul(coax_p_slab_sea_sun(), (l_glare * 0x1.800000p2f)));
  l_tr = coax_p_slab_sea_beam(l_dist);
  l_fog = coax_p_slab_sea_fog(coax_p_fmin(coax_at_u(i32(9i)), 0.0f), l_d.y);
  l_shafts = coax_p_slab_sea_shafts(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))), l_d, l_dist);
  return coax_p_slab_sea_out(coax_p_sb_add(coax_p_sb_mulv(l_col, l_tr), coax_p_sb_mul(coax_p_sb_mulv(l_fog, coax_p_v3((0x1.000000p0f - l_tr.x), (0x1.000000p0f - l_tr.y), (0x1.000000p0f - l_tr.z))), (0x1.000000p0f + (0x1.99999ap0f * l_shafts)))));
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  let l_p0: vec3f = p_p0;
  let l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_slab_water(p_p: vec2f, p_world: vec3f, p_d: vec3f, p_dist: f32, p_t: f32) -> vec3f {
  let l_p: vec2f = p_p;
  let l_world: vec3f = p_world;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  let l_t: f32 = p_t;
  var l_c: vec4f;
  var l_footprint: f32;
  var l_sea: vec4f;
  var l_col: vec3f;
  var l_froth: f32;
  var l_hp: vec3f;
  var l_edge: f32;
  var l_nd: i32;
  var l_k: i32;
  var l_b: i32;
  var l_wa: f32;
  var l_wc: f32;
  var l_st1: f32;
  var l_st2: f32;
  var l_patch: f32;
  var l_thin: f32;
  var l_veins: f32;
  var l_nearhull: f32;
  var l_crest: f32;
  var l_lace: f32;
  var l_wash: f32;
  var l_white: f32;
  l_c = coax_p_slab_coarse(l_p);
  l_footprint = ((((l_dist * 0x1.000000p1f) * coax_at_u(i32(11i))) / coax_at_u(i32(1i))) / coax_p_fmax(abs(l_d.y), 0x1.47ae14p-4f));
  l_sea = coax_p_slab_sea(l_p, l_world, l_d, l_dist, l_t, l_c);
  l_col = coax_p_v3(l_sea.x, l_sea.y, l_sea.z);
  l_froth = ((coax_p_slab_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.666666p-2f), coax_p_v2((l_t * 0x1.99999ap-3f), 0.0f))) * 0x1.333334p-1f) + (coax_p_slab_noise(coax_p_wg_mul_f2f(l_p, 0x1.4cccccp0f)) * 0x1.99999ap-2f));
  l_hp = coax_p_sb_to_hull(l_world);
  l_edge = coax_p_slab_hull_plan(coax_p_v2(l_hp.x, l_hp.z));
  if ((coax_at_u(i32(1500i)) > 0x1.000000p-1f))
  {
    l_edge = 0x1.86a000p16f;
  }
  if ((coax_at_u(i32(1500i)) > 0x1.800000p0f))
  {
    l_nd = select(1i, 7i, (coax_at_u(i32(1500i)) > 0x1.400000p1f));
    l_k = 0i;
    while ((l_k < l_nd))
    {
      l_b = (96i + (l_k * 8i));
      l_edge = coax_p_fmin(l_edge, (coax_p_wg_length2(coax_p_v2((l_hp.x - ((coax_at_u(i32(l_b)) + coax_at_u(i32((l_b + 4i)))) * 0x1.000000p-1f)), (l_hp.z - ((coax_at_u(i32((l_b + 2i))) + coax_at_u(i32((l_b + 6i)))) * 0x1.000000p-1f)))) - 0x1.e7ae14p5f));
      l_k = (l_k + 1i);
    }
  }
  if ((coax_at_u(i32(1500i)) > 0x1.800000p0f))
  {
    l_edge = (l_edge * 0x1.000000p2f);
  }
  l_wa = ((l_p.x * coax_at_u(i32(32i))) + (l_p.y * coax_at_u(i32(33i))));
  l_wc = ((l_p.y * coax_at_u(i32(32i))) - (l_p.x * coax_at_u(i32(33i))));
  l_st1 = coax_p_slab_noise(coax_p_v2(((l_wa * 0x1.1eb852p-5f) - (l_t * 0x1.47ae14p-5f)), (l_wc * 0x1.47ae14p-3f)));
  l_st2 = coax_p_slab_noise(coax_p_v2(((l_wa * 0x1.70a3d8p-4f) - (l_t * 0x1.1eb852p-4f)), ((l_wc * 0x1.99999ap-2f) + 0x1.400000p2f)));
  l_patch = coax_p_smoothstep(0x1.333334p-2f, 0x1.666666p-1f, coax_p_slab_noise(coax_p_wg_mul_f2f(l_p, 0x1.eb851ep-7f)));
  l_thin = (0x1.1eb852p-5f + (l_footprint * 0x1.0624dep-9f));
  l_veins = ((((0x1.000000p0f - coax_p_smoothstep(0.0f, l_thin, abs((l_st1 - 0x1.000000p-1f)))) * 0x1.800000p-1f) + ((0x1.000000p0f - coax_p_smoothstep(0.0f, (l_thin * 0x1.99999ap-1f), abs((l_st2 - 0x1.000000p-1f)))) * 0x1.000000p-1f)) * (0x1.333334p-2f + (0x1.666666p-1f * l_patch)));
  l_nearhull = (0x1.000000p0f - coax_p_smoothstep(0x1.e00000p3f, (0x1.180000p7f + (coax_at_u(i32(1484i)) * 0x1.800000p2f)), l_edge));
  l_crest = (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_smoothstep((-0x1.99999ap-2f), 0x1.000000p-1f, (l_c.x / coax_p_fmax(coax_at_u(i32(35i)), 0x1.99999ap-3f)))));
  l_lace = (((l_veins * l_crest) * (l_nearhull * 0x1.ccccccp-1f)) * (0x1.000000p0f - coax_p_smoothstep(0x1.2c0000p8f, 0x1.f40000p10f, l_dist)));
  l_wash = ((0x1.000000p0f - coax_p_smoothstep(0.0f, ((0x1.c00000p3f + (0x1.e00000p4f * l_froth)) + (coax_at_u(i32(1484i)) * 0x1.99999ap-1f)), l_edge)) * coax_p_smoothstep(0x1.99999ap-3f, 0x1.000000p-1f, (l_froth + (0x1.99999ap-3f * sin((((l_p.x + l_p.y) * 0x1.47ae14p-4f) + (l_t * 0x1.4cccccp0f)))))));
  l_white = coax_p_clampf(coax_p_fmax((l_wash * 0x1.e66666p-1f), (l_lace * 0x1.99999ap-2f)), 0.0f, 0x1.000000p0f);
  l_col = coax_p_sb_mix(l_col, coax_p_slab_sea_white(), l_white);
  return l_col;
}
fn coax_p_slab_weather_px(p_d: vec3f) -> vec2f {
  let l_d: vec3f = p_d;
  var l_vz: f32;
  var l_sx: f32;
  var l_sy: f32;
  l_vz = coax_p_fmax(coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i)))), 0x1.a36e2ep-14f);
  l_sx = (coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)))) / ((l_vz * coax_at_u(i32(11i))) * (coax_at_u(i32(0i)) / coax_at_u(i32(1i)))));
  l_sy = (coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i)))) / (l_vz * coax_at_u(i32(11i))));
  return coax_p_v2((((l_sx + 0x1.000000p0f) * 0x1.000000p-1f) * coax_at_u(i32(0i))), (((0x1.000000p0f - l_sy) * 0x1.000000p-1f) * coax_at_u(i32(1i))));
}
fn coax_p_wg_log2(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return (log(l_x) / 0x1.62e430p-1f);
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
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t19 {
  let l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
}
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t19 {
  let l_x: i32 = p_x;
  let l_y: i32 = p_y;
  return coax_t19(l_x, l_y);
}
fn coax_p_wg_add_i2i2(p_a: coax_t19, p_b: coax_t19) -> coax_t19 {
  let l_a: coax_t19 = p_a;
  let l_b: coax_t19 = p_b;
  return coax_p_i2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_make_i2_i(p_p0: i32) -> coax_t19 {
  let l_p0: i32 = p_p0;
  return coax_p_i2(l_p0, l_p0);
}
fn coax_p_wg_band_i2i2(p_a: coax_t19, p_b: coax_t19) -> coax_t19 {
  let l_a: coax_t19 = p_a;
  let l_b: coax_t19 = p_b;
  return coax_p_i2((l_a.x & l_b.x), (l_a.y & l_b.y));
}
fn coax_p_wg_cvt_u2i2(p_p: coax_t19) -> coax_t20 {
  let l_p: coax_t19 = p_p;
  return coax_p_u2(u32(l_p.x), u32(l_p.y));
}
fn coax_p_slab_word(p_i: u32) -> u32 {
  let l_i: u32 = p_i;
  return coax_p_wg_get_u4(coax_p_wg_at4u(((i32((l_i >> u32(2u))) * 4i) + 1504i)), i32((l_i & 3u)));
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
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_sb_sub(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_sb_p(p_i: i32) -> vec3f {
  let l_i: i32 = p_i;
  return coax_p_v3(coax_at_u(i32((1440i + l_i))), coax_at_u(i32(((1440i + l_i) + 1i))), coax_at_u(i32(((1440i + l_i) + 2i))));
}
fn coax_p_slab_span(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f) -> vec2f {
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
  l_tn = coax_p_fmax(coax_p_fmax(coax_p_fmin(l_ax, l_bx), coax_p_fmin(l_ay, l_by)), coax_p_fmin(l_az, l_bz));
  l_tf = coax_p_fmin(coax_p_fmin(coax_p_fmax(l_ax, l_bx), coax_p_fmax(l_ay, l_by)), coax_p_fmax(l_az, l_bz));
  return coax_p_v2(l_tn, l_tf);
}
fn coax_p_slab_record_hit(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f, p_sh: i32, p_limit: f32) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_lo: vec3f = p_lo;
  let l_hi: vec3f = p_hi;
  let l_sh: i32 = p_sh;
  let l_limit: f32 = p_limit;
  var l_ax: f32;
  var l_az: f32;
  var l_bx: f32;
  var l_bz: f32;
  var l_ex: f32;
  var l_ez: f32;
  if ((l_sh == 1i))
  {
    return coax_p_slab_drum_hit(l_o, l_d, l_lo, l_hi);
  }
  if ((l_sh == 2i))
  {
    return coax_p_slab_gable_hit(l_o, l_d, l_lo, l_hi, true);
  }
  if ((l_sh == 3i))
  {
    return coax_p_slab_gable_hit(l_o, l_d, l_lo, l_hi, false);
  }
  if (((l_sh >= 7i) && (l_sh <= 10i)))
  {
    l_ax = select(l_lo.x, l_hi.x, (((l_sh - 7i) & 1i) == 1i));
    l_az = select(l_lo.z, l_hi.z, (((l_sh - 7i) & 2i) == 2i));
    l_bx = ((l_lo.x + l_hi.x) - l_ax);
    l_bz = ((l_lo.z + l_hi.z) - l_az);
    return coax_p_slab_capsule(l_o, l_d, coax_p_v3(l_ax, l_hi.y, l_az), coax_p_v3(l_bx, l_lo.y, l_bz), 0x1.eb851ep-2f);
  }
  if ((l_sh == 4i))
  {
    return coax_p_slab_grove_hit(l_o, l_d, l_lo, l_hi, false, l_limit);
  }
  if ((l_sh == 5i))
  {
    return coax_p_slab_grove_hit(l_o, l_d, l_lo, l_hi, true, l_limit);
  }
  if ((l_sh == 6i))
  {
    return coax_p_slab_dome_hit(l_o, l_d, l_lo, l_hi);
  }
  if ((l_sh == 11i))
  {
    return coax_p_slab_greek_hit(l_o, l_d, l_lo, l_hi, false, l_limit);
  }
  if ((l_sh == 13i))
  {
    return coax_p_slab_statues_hit(l_o, l_d, l_lo, l_hi, l_limit);
  }
  if ((l_sh == 14i))
  {
    return coax_p_slab_silone_hit(l_o, l_d, l_lo, l_hi);
  }
  if (((l_sh >= 15i) && (l_sh <= 18i)))
  {
    l_ex = select((l_lo.x + 0x1.200000p2f), (l_hi.x - 0x1.200000p2f), (((l_sh - 15i) & 1i) == 1i));
    l_ez = select((l_lo.z + 0x1.200000p2f), (l_hi.z - 0x1.200000p2f), (((l_sh - 15i) & 2i) == 2i));
    return coax_p_slab_obox_hit(l_o, l_d, coax_p_v3(l_ex, (l_hi.y - 0x1.333334p0f), l_ez), coax_p_v3(((l_lo.x + l_hi.x) - l_ex), (l_lo.y + 0x1.333334p0f), ((l_lo.z + l_hi.z) - l_ez)), 0x1.000000p2f, 0x1.333334p-1f);
  }
  if ((l_sh == 12i))
  {
    return coax_p_slab_greek_hit(l_o, l_d, l_lo, l_hi, true, l_limit);
  }
  return coax_p_slab_box_hit(l_o, l_d, l_lo, l_hi);
}
fn coax_p_slab_capsule(p_o: vec3f, p_d: vec3f, p_a: vec3f, p_b: vec3f, p_r: f32) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_r: f32 = p_r;
  var l_ba: vec3f;
  var l_oa: vec3f;
  var l_baba: f32;
  var l_bard: f32;
  var l_baoa: f32;
  var l_rdoa: f32;
  var l_oaoa: f32;
  var l_qa: f32;
  var l_qb: f32;
  var l_qc: f32;
  var l_h: f32;
  var l_t: f32;
  var l_y: f32;
  var l_pp: vec3f;
  var l_nn: vec3f;
  var l_oc: vec3f;
  var l_bb: f32;
  var l_cc: f32;
  var l_hh: f32;
  var l_t2: f32;
  var l_pp_25: vec3f;
  var l_nn_26: vec3f;
  l_ba = coax_p_sb_sub(l_b, l_a);
  l_oa = coax_p_sb_sub(l_o, l_a);
  l_baba = coax_p_wg_dot3(l_ba, l_ba);
  l_bard = coax_p_wg_dot3(l_ba, l_d);
  l_baoa = coax_p_wg_dot3(l_ba, l_oa);
  l_rdoa = coax_p_wg_dot3(l_d, l_oa);
  l_oaoa = coax_p_wg_dot3(l_oa, l_oa);
  l_qa = (l_baba - (l_bard * l_bard));
  l_qb = ((l_baba * l_rdoa) - (l_baoa * l_bard));
  l_qc = (((l_baba * l_oaoa) - (l_baoa * l_baoa)) - ((l_r * l_r) * l_baba));
  l_h = ((l_qb * l_qb) - (l_qa * l_qc));
  if (((l_h >= 0.0f) && (l_qa > 0x1.5798eep-27f)))
  {
    l_t = (((-l_qb) - sqrt(l_h)) / l_qa);
    l_y = (l_baoa + (l_t * l_bard));
    if ((((l_y > 0.0f) && (l_y < l_baba)) && (l_t > 0.0f)))
    {
      l_pp = coax_p_sb_add(l_oa, coax_p_sb_mul(l_d, l_t));
      l_nn = coax_p_wg_normalize3(coax_p_sb_sub(l_pp, coax_p_sb_mul(l_ba, (l_y / l_baba))));
      return coax_p_v4(l_t, l_nn.x, l_nn.y, l_nn.z);
    }
    l_oc = select(coax_p_sb_sub(l_o, l_b), l_oa, (l_y <= 0.0f));
    l_bb = coax_p_wg_dot3(l_d, l_oc);
    l_cc = (coax_p_wg_dot3(l_oc, l_oc) - (l_r * l_r));
    l_hh = ((l_bb * l_bb) - l_cc);
    if ((l_hh > 0.0f))
    {
      l_t2 = ((-l_bb) - sqrt(l_hh));
      if ((l_t2 > 0.0f))
      {
        l_pp_25 = coax_p_sb_add(l_oc, coax_p_sb_mul(l_d, l_t2));
        l_nn_26 = coax_p_wg_normalize3(l_pp_25);
        return coax_p_v4(l_t2, l_nn_26.x, l_nn_26.y, l_nn_26.z);
      }
    }
  }
  return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
}
fn coax_p_slab_paint(p_hit: coax_t15, p_w: vec3f, p_d: vec3f) -> coax_t21 {
  let l_hit: coax_t15 = p_hit;
  let l_w: vec3f = p_w;
  let l_d: vec3f = p_d;
  var l_kind: f32;
  var l_n: vec3f;
  var l_top: f32;
  var l_k: i32;
  var l_roof: bool;
  var l_night: f32;
  var l_grain: f32;
  var l_c: vec3f;
  var l_emit: vec3f;
  var l_deck: f32;
  var l_oncut: bool;
  var l_j: f32;
  var l_jx: f32;
  var l_jz: f32;
  var l_gx: f32;
  var l_gz: f32;
  var l_jx_20: f32;
  var l_jz_21: f32;
  var l_edge: f32;
  var l_along: f32;
  var l_pu: f32;
  var l_pv: f32;
  var l_tone: f32;
  var l_fine: f32;
  var l_lu: f32;
  var l_lv: f32;
  var l_line: f32;
  var l_streak: f32;
  var l_face: f32;
  var l_sprayw: f32;
  var l_thru: f32;
  var l_win: f32;
  var l_pipes: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_strip: f32;
  var l_storey: f32;
  var l_along_41: f32;
  var l_mull: f32;
  var l_r: vec3f;
  var l_sky: vec3f;
  var l_pane: f32;
  var l_detail: f32;
  var l_fres: f32;
  var l_frame: f32;
  var l_tower: f32;
  var l_glass: vec3f;
  var l_ce: f32;
  var l_lit: f32;
  var l_slot: f32;
  var l_row: f32;
  var l_h: f32;
  var l_index: f32;
  var l_on: f32;
  var l_led: f32;
  var l_inset: f32;
  var l_tall: f32;
  var l_along_61: f32;
  var l_h_62: f32;
  var l_cornice: f32;
  var l_col: f32;
  var l_shade: f32;
  var l_win_66: f32;
  var l_fx_67: f32;
  var l_fz_68: f32;
  var l_sky_69: vec3f;
  var l_e: f32;
  var l_px: f32;
  var l_pz: f32;
  var l_gcx: f32;
  var l_gcz: f32;
  var l_nz: f32;
  var l_rr: f32;
  var l_dcx: f32;
  var l_dcz: f32;
  var l_rr_79: f32;
  var l_yc: f32;
  var l_big: f32;
  var l_tier: f32;
  var l_edge_83: f32;
  var l_ang: f32;
  var l_mull_85: f32;
  l_kind = l_hit.z;
  l_n = l_hit.y;
  l_top = l_hit.w;
  l_k = coax_p_wg_i32_f((l_kind + 0x1.000000p-1f));
  l_roof = (l_n.y > 0x1.000000p-1f);
  l_night = coax_at_u(i32(1479i));
  l_grain = (0x1.e66666p-1f + (0x1.1eb852p-4f * coax_p_slab_noise(coax_p_v2(((l_w.x * 0x1.0a3d70p-3f) + (l_w.y * 0x1.1eb852p-4f)), ((l_w.z * 0x1.0a3d70p-3f) - (l_w.y * 0x1.99999ap-5f))))));
  l_c = coax_p_v3(0x1.99999ap-1f, 0x1.8f5c28p-1f, 0x1.7ae148p-1f);
  l_emit = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_deck = coax_at_u(i32(38i));
  l_oncut = (((coax_at_u(i32(1492i)) != 0.0f) && (abs(l_n.z) > 0x1.ccccccp-1f)) && (abs((l_w.z - coax_at_u(i32(1493i)))) < 0x1.333334p-1f));
  if (((l_k >= 22i) && (l_k <= 26i)))
  {
    if ((l_k == 22i))
    {
      l_c = coax_p_v3(0x1.70a3d8p-1f, 0x1.70a3d8p-1f, 0x1.51eb86p-1f);
      if (l_roof)
      {
        l_j = coax_p_fmax(coax_p_slab_band(l_w.x, 0x1.900000p6f, 0x1.89374cp-7f), coax_p_slab_band(l_w.z, 0x1.900000p5f, 0x1.6872b0p-6f));
        l_c = coax_p_sb_mul(l_c, (0x1.000000p0f - (0x1.19999ap-1f * l_j)));
      }
      else
      {
        l_c = coax_p_sb_mul(l_c, (0x1.dc28f6p-1f - (0x1.0a3d70p-3f * coax_p_slab_band(l_w.y, 0x1.000000p2f, 0x1.333334p-3f))));
        l_c = coax_p_sb_mix(coax_p_v3(0x1.0a3d70p-2f, 0x1.5c28f6p-2f, 0x1.3d70a4p-2f), l_c, coax_p_smoothstep(0.0f, 0x1.000000p1f, l_w.y));
      }
    }
    else
    {
      if ((l_k == 23i))
      {
        l_c = coax_p_v3(0x1.f5c290p-1f, 0x1.2e147ap-1f, 0x1.0a3d70p-3f);
      }
      else
      {
        if ((l_k == 24i))
        {
          l_c = select(coax_p_v3(0x1.eb851ep-4f, 0x1.147ae2p-2f, 0x1.47ae14p-2f), coax_p_v3(0x1.051eb8p-1f, 0x1.47ae14p-3f, 0x1.47ae14p-4f), (l_w.y < 0x1.800000p0f));
        }
        else
        {
          if ((l_k == 25i))
          {
            l_c = select(coax_p_v3(0x1.eb851ep-1f, 0x1.d1eb86p-1f, 0x1.947ae2p-1f), coax_p_v3(0x1.ae147ap-3f, 0x1.851eb8p-2f, 0x1.b851ecp-2f), (l_w.y > 0x1.933334p2f));
          }
          else
          {
            l_c = coax_p_v3(0x1.9eb852p-1f, 0x1.99999ap-2f, 0x1.99999ap-3f);
          }
        }
      }
    }
    return coax_t21(coax_p_sb_mul(l_c, l_grain), l_emit);
  }
  if (l_oncut)
  {
    return coax_p_slab_section(l_k, l_w);
  }
  if ((l_k == 21i))
  {
    l_c = coax_p_sb_mul(coax_p_v3(0x1.47ae14p-3f, 0x1.5c28f6p-3f, 0x1.70a3d8p-3f), (0x1.ccccccp-1f + (0x1.99999ap-3f * coax_p_slab_band(l_w.y, 0x1.800000p3f, 0x1.47ae14p-4f))));
    return coax_t21(l_c, l_emit);
  }
  if (((l_k == 0i) && (l_n.y < (-0x1.000000p-1f))))
  {
    l_jx = abs((coax_p_fract((l_w.x / 0x1.400000p3f)) - 0x1.000000p-1f));
    l_jz = abs((coax_p_fract((l_w.z / 0x1.400000p3f)) - 0x1.000000p-1f));
    l_c = coax_p_sb_mul(coax_p_v3(0x1.ae147ap-2f, 0x1.ccccccp-2f, 0x1.99999ap-2f), ((0x1.b33334p-1f + (0x1.333334p-3f * coax_p_slab_noise(coax_p_v2((l_w.x * 0x1.99999ap-5f), (l_w.z * 0x1.99999ap-5f))))) * (0x1.000000p0f - (0x1.eb851ep-4f * coax_p_stepf(0x1.eb851ep-2f, coax_p_fmax(l_jx, l_jz))))));
    return coax_t21(l_c, l_emit);
  }
  if ((l_k == 0i))
  {
    if (l_roof)
    {
      l_c = coax_p_v3(0x1.a3d70ap-1f, 0x1.947ae2p-1f, 0x1.70a3d8p-1f);
      l_gx = (abs(((l_w.x / 0x1.900000p6f) - coax_p_wg_round((l_w.x / 0x1.900000p6f)))) * 0x1.900000p6f);
      l_gz = (abs(((l_w.z / 0x1.900000p6f) - coax_p_wg_round((l_w.z / 0x1.900000p6f)))) * 0x1.900000p6f);
      if ((coax_p_fmin(l_gx, l_gz) < 0x1.400000p3f))
      {
        l_c = coax_p_v3(0x1.666666p-1f, 0x1.6147aep-1f, 0x1.51eb86p-1f);
        if ((abs((coax_p_fmin(l_gx, l_gz) - 0x1.400000p2f)) < 0x1.000000p-2f))
        {
          l_c = coax_p_v3(0x1.ccccccp-1f, 0x1.c28f5cp-1f, 0x1.99999ap-1f);
        }
      }
      l_jx_20 = abs((coax_p_fract((l_w.x / 0x1.400000p2f)) - 0x1.000000p-1f));
      l_jz_21 = abs((coax_p_fract((l_w.z / 0x1.400000p2f)) - 0x1.000000p-1f));
      l_c = coax_p_sb_mul(l_c, (0x1.000000p0f - (0x1.99999ap-5f * coax_p_stepf(0x1.e147aep-2f, coax_p_fmax(l_jx_20, l_jz_21)))));
      l_edge = coax_p_slab_hull_plan(coax_p_v2(l_w.x, l_w.z));
      if ((l_edge > (-0x1.400000p4f)))
      {
        l_c = coax_p_v3(0x1.ccccccp-1f, 0x1.bd70a4p-1f, 0x1.99999ap-1f);
      }
      if ((l_edge > (-0x1.99999ap0f)))
      {
        l_c = coax_p_v3(0x1.e66666p-1f, 0x1.dc28f6p-1f, 0x1.c28f5cp-1f);
      }
    }
    else
    {
      l_along = select(l_w.x, l_w.z, (abs(l_n.x) > abs(l_n.z)));
      l_pu = floor((l_along / 0x1.400000p2f));
      l_pv = floor((l_w.y / 0x1.200000p2f));
      l_tone = (0x1.e147aep-1f + (0x1.47ae14p-4f * coax_p_slab_hash2(coax_p_v2(l_pu, l_pv))));
      l_c = coax_p_sb_mul(coax_p_v3(0x1.b851ecp-1f, 0x1.a3d70ap-1f, 0x1.7ae148p-1f), l_tone);
      l_fine = (0x1.000000p0f - coax_p_smoothstep(0x1.f40000p8f, 0x1.900000p10f, l_hit.x));
      l_lu = abs((coax_p_fract((l_along / 0x1.400000p2f)) - 0x1.000000p-1f));
      l_lv = abs((coax_p_fract((l_w.y / 0x1.200000p2f)) - 0x1.000000p-1f));
      l_line = coax_p_fmax(coax_p_stepf(0x1.e147aep-2f, l_lu), coax_p_stepf(0x1.dc28f6p-2f, l_lv));
      l_c = coax_p_sb_mul(l_c, (0x1.000000p0f - (((0x1.99999ap-3f * l_line) * l_fine) + (0x1.99999ap-5f * (0x1.000000p0f - l_fine)))));
      l_streak = coax_p_slab_noise(coax_p_v2((l_along * 0x1.333334p-1f), (l_w.y * 0x1.47ae14p-6f)));
      l_c = coax_p_sb_mul(l_c, (0x1.dc28f6p-1f + (0x1.1eb852p-4f * l_streak)));
      l_c = coax_p_sb_mix(l_c, coax_p_v3(0x1.333334p-2f, 0x1.5c28f6p-2f, 0x1.333334p-2f), ((0x1.000000p0f - coax_p_smoothstep((-0x1.000000p0f), 0x1.400000p1f, l_w.y)) * 0x1.99999ap-1f));
      l_face = coax_p_clampf((0x1.000000p-2f + (((-(l_n.x * coax_at_u(i32(32i)))) - (l_n.z * coax_at_u(i32(33i)))) * 0x1.ccccccp-1f)), 0x1.333334p-3f, 0x1.000000p0f);
      l_sprayw = coax_p_slab_spray((l_along * select((-0x1.000000p0f), 0x1.000000p0f, ((l_n.x + l_n.z) > 0.0f))), l_w.y, l_face, coax_at_u(i32(2i)));
      l_c = coax_p_sb_mix(l_c, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.f5c290p-1f), l_sprayw);
      l_thru = coax_p_sb_pow(coax_p_fmax(coax_p_wg_dot3(l_d, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f), 0x1.800000p1f);
      l_emit = coax_p_sb_mul(coax_p_sb_add(coax_p_sb_mul(coax_p_sb_p(12i), 0x1.666666p-2f), coax_p_sb_mul(coax_p_sb_p(4i), (0x1.000000p-2f + (0x1.666666p-1f * l_thru)))), l_sprayw);
    }
  }
  else
  {
    if (((l_k == 2i) || (l_k == 20i)))
    {
      l_c = coax_p_v3(0x1.c28f5cp-1f, 0x1.c7ae14p-1f, 0x1.ccccccp-1f);
      if (l_roof)
      {
        l_c = coax_p_sb_mul(coax_p_v3(0x1.666666p-1f, 0x1.75c290p-1f, 0x1.851eb8p-1f), (0x1.d70a3ep-1f + (0x1.47ae14p-4f * coax_p_slab_band((l_w.x + l_w.z), 0x1.800000p2f, 0x1.000000p-1f))));
      }
      else
      {
        l_c = coax_p_sb_mul(l_c, (0x1.c28f5cp-1f + (0x1.eb851ep-4f * coax_p_slab_band(l_w.y, 0x1.000000p2f, 0x1.99999ap-1f))));
        l_win = (coax_p_slab_band((l_w.x + l_w.z), 0x1.000000p3f, 0x1.333334p-2f) * coax_p_slab_band((l_w.y - l_deck), 0x1.800000p2f, 0x1.000000p-2f));
        l_c = coax_p_sb_mix(l_c, coax_p_v3(0x1.000000p-2f, 0x1.47ae14p-2f, 0x1.851eb8p-2f), (l_win * 0x1.333334p-1f));
        l_emit = coax_p_sb_mul(coax_p_v3(0x1.000000p0f, 0x1.99999ap-1f, 0x1.000000p-1f), ((l_win * l_night) * 0x1.000000p-1f));
      }
    }
    else
    {
      if ((l_k == 3i))
      {
        l_c = coax_p_v3(0x1.7ae148p-1f, 0x1.666666p-1f, 0x1.47ae14p-1f);
        if ((!l_roof))
        {
          l_c = coax_p_sb_mix(l_c, coax_p_v3(0x1.ccccccp-2f, 0x1.b851ecp-2f, 0x1.ae147ap-2f), coax_p_slab_band(l_w.y, 0x1.c00000p2f, 0x1.70a3d8p-3f));
          l_pipes = (coax_p_slab_band((l_w.x + l_w.z), 0x1.800000p1f, 0x1.99999ap-3f) * coax_p_smoothstep(0x1.000000p2f, 0x1.800000p2f, (l_w.y - l_deck)));
          l_c = coax_p_sb_mix(l_c, coax_p_v3(0x1.333334p-1f, 0x1.333334p-1f, 0x1.28f5c2p-1f), (l_pipes * 0x1.000000p-1f));
        }
        else
        {
          l_c = coax_p_v3(0x1.3d70a4p-1f, 0x1.333334p-1f, 0x1.23d70ap-1f);
        }
        l_emit = coax_p_sb_mul(coax_p_v3(0x1.000000p0f, 0x1.333334p-1f, 0x1.333334p-2f), (((l_night * 0x1.333334p-3f) * coax_at_u(i32(1488i))) * coax_p_slab_band((l_w.x + l_w.y), 0x1.600000p3f, 0x1.47ae14p-4f)));
      }
      else
      {
        if ((l_k == 4i))
        {
          l_c = coax_p_v3(0x1.3d70a4p-1f, 0x1.47ae14p-1f, 0x1.51eb86p-1f);
          if (l_roof)
          {
            l_fx = (coax_p_fract((l_w.x / 0x1.200000p3f)) - 0x1.000000p-1f);
            l_fz = (coax_p_fract((l_w.z / 0x1.200000p3f)) - 0x1.000000p-1f);
            l_c = select(coax_p_v3(0x1.70a3d8p-1f, 0x1.75c290p-1f, 0x1.800000p-1f), coax_p_v3(0x1.c28f5cp-3f, 0x1.eb851ep-3f, 0x1.147ae2p-2f), (((l_fx * l_fx) + (l_fz * l_fz)) < 0x1.70a3d8p-4f));
          }
          else
          {
            l_c = coax_p_sb_mul(l_c, (0x1.b33334p-1f + (0x1.333334p-3f * coax_p_slab_band(l_w.y, 0x1.333334p0f, 0x1.000000p-1f))));
            l_strip = coax_p_slab_band((l_w.y - l_deck), 0x1.800000p2f, 0x1.eb851ep-4f);
            l_emit = coax_p_sb_mul(coax_p_v3(0x1.99999ap-2f, 0x1.666666p-1f, 0x1.000000p0f), ((l_strip * coax_at_u(i32(1487i))) * (0x1.333334p-3f + (0x1.333334p-1f * l_night))));
          }
        }
        else
        {
          if ((l_k == 5i))
          {
            if (l_roof)
            {
              l_c = coax_p_v3(0x1.7ae148p-1f, 0x1.75c290p-1f, 0x1.666666p-1f);
            }
            else
            {
              l_storey = coax_p_fract(((l_w.y - l_deck) / 0x1.ccccccp1f));
              l_along_41 = select(l_w.x, l_w.z, (abs(l_n.x) > abs(l_n.z)));
              l_mull = coax_p_slab_band(l_along_41, 0x1.ccccccp0f, 0x1.eb851ep-4f);
              l_r = coax_p_wg_reflect3(l_d, l_n);
              l_sky = coax_p_sb_add(coax_p_sb_mix(coax_p_sb_p(8i), coax_p_sb_p(12i), 0x1.333334p-2f), coax_p_sb_mul(coax_p_sb_p(4i), (0x1.333334p-1f * coax_p_sb_pow(coax_p_fmax(coax_p_wg_dot3(l_r, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f), 0x1.800000p3f))));
              l_pane = coax_p_slab_hash2(coax_p_v2(floor((l_along_41 / 0x1.ccccccp0f)), (floor(((l_w.y - l_deck) / 0x1.ccccccp1f)) + (l_hit.f4 * 0x1.c00000p2f))));
              l_detail = (0x1.000000p0f - coax_p_smoothstep(0x1.b80000p7f, 0x1.450000p9f, l_hit.x));
              l_fres = (0x1.000000p-2f + (0x1.800000p-1f * coax_p_sb_pow((0x1.000000p0f - abs(coax_p_wg_dot3(l_d, l_n))), 0x1.800000p1f)));
              l_frame = select(0x1.000000p0f, ((l_mull * 0x1.000000p-1f) * l_detail), (l_storey > 0x1.99999ap-3f));
              l_tower = coax_p_slab_hash2(coax_p_v2(l_hit.f4, 0x1.800000p1f));
              l_frame = coax_p_mix((0x1.333334p-2f + (0x1.000000p-2f * l_tower)), l_frame, l_detail);
              l_glass = coax_p_sb_mix(coax_p_v3(0x1.47ae14p-4f, 0x1.eb851ep-4f, 0x1.5c28f6p-3f), coax_p_v3(0x1.99999ap-3f, 0x1.eb851ep-3f, 0x1.1eb852p-2f), l_tower);
              l_c = coax_p_sb_mix(l_glass, coax_p_v3(0x1.a3d70ap-1f, 0x1.a3d70ap-1f, 0x1.99999ap-1f), l_frame);
              l_emit = coax_p_sb_mul(coax_p_sb_mulv(l_sky, coax_p_v3(0x1.333334p-1f, 0x1.99999ap-1f, 0x1.0cccccp0f)), ((((0x1.000000p0f - l_frame) * l_fres) * 0x1.000000p-1f) * (0x1.b33334p-1f + ((0x1.333334p-2f * l_pane) * l_detail))));
              l_ce = select(coax_p_fmin(abs((l_w.x - l_hit.f5.x)), abs((l_hit.f6.x - l_w.x))), coax_p_fmin(abs((l_w.z - l_hit.f5.z)), abs((l_hit.f6.z - l_w.z))), (abs(l_n.x) > abs(l_n.z)));
              l_c = coax_p_sb_mix(l_c, coax_p_v3(0x1.c28f5cp-1f, 0x1.c28f5cp-1f, 0x1.b851ecp-1f), (0x1.000000p0f - coax_p_smoothstep(0x1.333334p-1f, 0x1.666666p0f, l_ce)));
              l_lit = coax_p_stepf(0x1.ccccccp-2f, coax_p_slab_hash2(coax_p_v2(floor((l_along_41 / 0x1.ccccccp1f)), (floor(((l_w.y - l_deck) / 0x1.ccccccp1f)) + (l_hit.f4 * 0x1.a00000p3f)))));
              l_emit = coax_p_sb_add(l_emit, coax_p_sb_mul(coax_p_v3(0x1.000000p0f, 0x1.8f5c28p-1f, 0x1.eb851ep-2f), (((coax_p_stepf(0x1.eb851ep-3f, l_storey) * l_lit) * l_night) * 0x1.666666p-1f)));
            }
          }
          else
          {
            if ((l_k == 6i))
            {
              l_c = coax_p_v3(0x1.c28f5cp-1f, 0x1.b851ecp-1f, 0x1.a3d70ap-1f);
              if (((l_w.y > (l_top - 0x1.c00000p2f)) || ((l_w.y > (l_top - 0x1.000000p4f)) && (l_w.y < (l_top - 0x1.800000p3f)))))
              {
                l_c = coax_p_v3(0x1.7ae148p-1f, 0x1.c28f5cp-3f, 0x1.47ae14p-3f);
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
                l_c = coax_p_sb_mix(coax_p_v3(0x1.333334p-2f, 0x1.ae147ap-2f, 0x1.c28f5cp-3f), coax_p_v3(0x1.99999ap-2f, 0x1.000000p-1f, 0x1.1eb852p-2f), coax_p_slab_noise(coax_p_v2((l_w.x * 0x1.99999ap-3f), (l_w.z * 0x1.99999ap-3f))));
              }
              else
              {
                if ((l_k == 8i))
                {
                  l_slot = ((l_w.x - 0x1.400000p7f) / 0x1.b33334p3f);
                  l_row = floor(((l_w.z + 0x1.180000p8f) / 0x1.200000p3f));
                  l_h = coax_p_slab_hash2(coax_p_v2(floor(l_slot), l_row));
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
                    l_c = coax_p_v3(0x1.3d70a4p-1f, 0x1.3d70a4p-1f, 0x1.333334p-1f);
                  }
                  if ((!l_roof))
                  {
                    l_c = coax_p_sb_mul(l_c, (0x1.c28f5cp-1f + (0x1.eb851ep-4f * coax_p_slab_band(l_w.x, 0x1.333334p-1f, 0x1.000000p-1f))));
                  }
                  l_index = ((l_row * 0x1.400000p3f) + floor(l_slot));
                  l_on = coax_p_stepf((l_index + 0x1.000000p-1f), coax_at_u(i32(1486i)));
                  l_led = ((((0x1.000000p0f - coax_p_stepf(0x1.b33334p-1f, coax_p_fract(l_slot))) * coax_p_stepf(0x1.666666p-1f, coax_p_fract(l_slot))) * coax_p_stepf(l_w.y, (l_deck + 0x1.333334p1f))) * coax_p_stepf((l_deck + 0x1.0cccccp1f), l_w.y));
                  l_emit = coax_p_sb_mul(coax_p_v3(0x1.333334p-2f, 0x1.000000p0f, 0x1.000000p-1f), ((l_led * l_on) * (0x1.99999ap-2f + l_night)));
                }
                else
                {
                  if ((((l_k == 16i) || (l_k == 18i)) && (coax_p_fract(l_kind) > 0x1.99999ap-4f)))
                  {
                    l_c = coax_p_v3(0x1.e66666p-1f, 0x1.d70a3ep-1f, 0x1.b33334p-1f);
                    if (((l_n.y > 0x1.99999ap-3f) && (l_n.y < 0x1.fae148p-1f)))
                    {
                      l_c = coax_p_v3(0x1.70a3d8p-1f, 0x1.666666p-1f, 0x1.4cccccp-1f);
                    }
                    l_inset = coax_p_fmin(coax_p_fmin((l_w.x - l_hit.f5.x), (l_hit.f6.x - l_w.x)), coax_p_fmin((l_w.z - l_hit.f5.z), (l_hit.f6.z - l_w.z)));
                    l_tall = (l_hit.w - l_hit.f5.y);
                    if ((((abs(l_n.y) < 0x1.333334p-2f) && (l_inset > (coax_p_fmin((l_hit.f6.x - l_hit.f5.x), (l_hit.f6.z - l_hit.f5.z)) * 0x1.eb851ep-4f))) && ((l_w.y - l_hit.f5.y) < (l_tall * 0x1.3d70a4p-1f))))
                    {
                      l_c = coax_p_sb_mul(l_c, 0x1.3d70a4p-1f);
                    }
                    l_c = coax_p_sb_mul(l_c, (0x1.f0a3d8p-1f + (0x1.eb851ep-6f * sin(((l_w.x + l_w.z) * 0x1.200000p3f)))));
                    l_emit = coax_p_sb_mul(coax_p_v3(0x1.000000p0f, 0x1.99999ap-1f, 0x1.19999ap-1f), (((l_night * 0x1.99999ap-3f) * coax_p_stepf(0x1.333334p-2f, l_inset)) * (0x1.000000p0f - abs(l_n.y))));
                  }
                  else
                  {
                    if ((((l_k == 9i) || (l_k == 16i)) || (l_k == 18i)))
                    {
                      l_c = coax_p_v3(0x1.e147aep-1f, 0x1.d1eb86p-1f, 0x1.ae147ap-1f);
                      if ((l_n.y > 0x1.99999ap-3f))
                      {
                        l_c = select(coax_p_v3(0x1.51eb86p-1f, 0x1.47ae14p-1f, 0x1.333334p-1f), coax_p_v3(0x1.a3d70ap-1f, 0x1.99999ap-1f, 0x1.7ae148p-1f), (l_k == 9i));
                        if ((l_n.y > 0x1.f5c290p-1f))
                        {
                          l_c = coax_p_v3(0x1.b851ecp-1f, 0x1.ae147ap-1f, 0x1.8f5c28p-1f);
                        }
                      }
                      else
                      {
                        l_along_61 = select(l_w.x, l_w.z, (abs(l_n.x) > abs(l_n.z)));
                        l_h_62 = (l_w.y - l_hit.f5.y);
                        l_cornice = ((l_hit.w - l_hit.f5.y) * 0x1.3d70a4p-1f);
                        if (((l_h_62 > 0x1.333334p0f) && (l_h_62 < l_cornice)))
                        {
                          l_col = coax_p_slab_columns(l_along_61, select(0x1.99999ap1f, 0x1.000000p2f, (l_k == 9i)));
                          l_shade = select(0x1.ae147ap-2f, 0x1.19999ap-1f, (l_k == 18i));
                          l_c = coax_p_sb_mul(l_c, coax_p_mix(l_shade, 0x1.000000p0f, l_col));
                          l_c = coax_p_sb_mul(l_c, (0x1.eb851ep-1f + (0x1.47ae14p-5f * sin((l_along_61 * 0x1.800000p3f)))));
                        }
                        else
                        {
                          if ((l_h_62 <= 0x1.333334p0f))
                          {
                            l_c = coax_p_sb_mul(l_c, (0x1.ccccccp-1f + (0x1.99999ap-4f * coax_p_slab_band(l_h_62, 0x1.99999ap-2f, 0x1.000000p-1f))));
                          }
                          else
                          {
                            l_c = coax_p_sb_mul(l_c, (0x1.f0a3d8p-1f - (0x1.eb851ep-5f * coax_p_slab_band((l_h_62 - l_cornice), 0x1.ccccccp-1f, 0x1.333334p-2f))));
                          }
                        }
                        l_emit = coax_p_sb_mul(coax_p_v3(0x1.000000p0f, 0x1.99999ap-1f, 0x1.19999ap-1f), ((((l_night * 0x1.000000p-2f) * (0x1.000000p0f - coax_p_slab_columns(l_along_61, 0x1.99999ap1f))) * coax_p_stepf(0x1.333334p0f, l_h_62)) * coax_p_stepf(l_h_62, l_cornice)));
                      }
                      if (((l_k == 9i) && (l_hit.f5.y > (l_deck + 0x1.400000p2f))))
                      {
                        l_c = coax_p_sb_mix(coax_p_v3(0x1.70a3d8p-1f, 0x1.8f5c28p-1f, 0x1.70a3d8p-1f), coax_p_v3(0x1.ccccccp-1f, 0x1.ccccccp-1f, 0x1.b851ecp-1f), coax_p_slab_band((coax_p_wg_atan2((l_w.z - ((l_hit.f5.z + l_hit.f6.z) * 0x1.000000p-1f)), (l_w.x - ((l_hit.f5.x + l_hit.f6.x) * 0x1.000000p-1f))) * 0x1.800000p2f), 0x1.000000p0f, 0x1.333334p-3f));
                      }
                    }
                    else
                    {
                      if ((l_k == 10i))
                      {
                        l_c = coax_p_v3(0x1.d70a3ep-1f, 0x1.d70a3ep-1f, 0x1.c7ae14p-1f);
                        if ((!l_roof))
                        {
                          l_c = coax_p_sb_mul(l_c, (0x1.d70a3ep-1f + (0x1.47ae14p-4f * coax_p_slab_band(l_w.y, 0x1.800000p1f, 0x1.99999ap-4f))));
                        }
                      }
                      else
                      {
                        if ((l_k == 11i))
                        {
                          l_c = coax_p_v3(0x1.ccccccp-1f, 0x1.d70a3ep-1f, 0x1.d70a3ep-1f);
                          if ((!l_roof))
                          {
                            l_win_66 = (coax_p_stepf(0x1.ccccccp-2f, coax_p_fract(((l_w.y - l_deck) / 0x1.ccccccp1f))) * coax_p_slab_band(select(l_w.x, l_w.z, (abs(l_n.x) > abs(l_n.z))), 0x1.000000p2f, 0x1.666666p-1f));
                            l_c = select(l_c, coax_p_v3(0x1.99999ap-2f, 0x1.147ae2p-1f, 0x1.333334p-1f), (l_win_66 > 0x1.000000p-1f));
                            l_emit = coax_p_sb_mul(coax_p_v3(0x1.ccccccp-1f, 0x1.e66666p-1f, 0x1.000000p0f), ((l_win_66 * l_night) * 0x1.ccccccp-2f));
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
                              l_c = coax_p_v3(0x1.3d70a4p-1f, 0x1.47ae14p-1f, 0x1.51eb86p-1f);
                              if (l_roof)
                              {
                                l_fx_67 = (coax_p_fract((l_w.x / 0x1.400000p3f)) - 0x1.000000p-1f);
                                l_fz_68 = (coax_p_fract((l_w.z / 0x1.400000p3f)) - 0x1.000000p-1f);
                                l_c = select(l_c, coax_p_v3(0x1.99999ap-3f, 0x1.c28f5cp-3f, 0x1.eb851ep-3f), (((l_fx_67 * l_fx_67) + (l_fz_68 * l_fz_68)) < 0x1.eb851ep-4f));
                              }
                            }
                            else
                            {
                              if ((l_k == 14i))
                              {
                                l_sky_69 = coax_p_slab_sky(coax_p_wg_reflect3(l_d, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)));
                                l_c = coax_p_sb_mix(coax_p_v3(0x1.eb851ep-5f, 0x1.0a3d70p-2f, 0x1.47ae14p-2f), l_sky_69, 0x1.ccccccp-2f);
                                if ((!l_roof))
                                {
                                  l_c = coax_p_v3(0x1.a3d70ap-1f, 0x1.99999ap-1f, 0x1.7ae148p-1f);
                                }
                              }
                              else
                              {
                                if ((l_k == 15i))
                                {
                                  if ((l_w.y < (l_hit.f5.y + 0x1.ccccccp-2f)))
                                  {
                                    l_c = coax_p_sb_mix(coax_p_v3(0x1.ae147ap-2f, 0x1.d70a3ep-2f, 0x1.0a3d70p-2f), coax_p_v3(0x1.147ae2p-1f, 0x1.1eb852p-1f, 0x1.5c28f6p-2f), coax_p_slab_noise(coax_p_v2((l_w.x * 0x1.333334p-2f), (l_w.z * 0x1.333334p-2f))));
                                    l_e = coax_p_fmin(coax_p_fmin((l_w.x - l_hit.f5.x), (l_hit.f6.x - l_w.x)), coax_p_fmin((l_w.z - l_hit.f5.z), (l_hit.f6.z - l_w.z)));
                                    if ((l_e < 0x1.000000p1f))
                                    {
                                      l_c = coax_p_v3(0x1.c28f5cp-3f, 0x1.47ae14p-2f, 0x1.47ae14p-3f);
                                    }
                                    l_px = (abs((coax_p_fract((l_w.x / 0x1.800000p4f)) - 0x1.000000p-1f)) * 0x1.800000p4f);
                                    l_pz = (abs((coax_p_fract((l_w.z / 0x1.800000p4f)) - 0x1.000000p-1f)) * 0x1.800000p4f);
                                    if ((((l_px < 0x1.99999ap0f) || (l_pz < 0x1.99999ap0f)) || ((l_e > 0x1.000000p1f) && (l_e < 0x1.200000p2f))))
                                    {
                                      l_c = coax_p_v3(0x1.ae147ap-1f, 0x1.99999ap-1f, 0x1.666666p-1f);
                                    }
                                    l_gcx = ((l_hit.f5.x + l_hit.f6.x) * 0x1.000000p-1f);
                                    l_gcz = ((l_hit.f5.z + l_hit.f6.z) * 0x1.000000p-1f);
                                    if (((abs((l_w.x - l_gcx)) < 0x1.000000p1f) || (abs((l_w.z - l_gcz)) < 0x1.000000p1f)))
                                    {
                                      l_c = coax_p_v3(0x1.ae147ap-1f, 0x1.99999ap-1f, 0x1.666666p-1f);
                                    }
                                  }
                                  else
                                  {
                                    l_nz = coax_p_slab_noise(coax_p_v2(((l_w.x + l_w.z) * 0x1.b33334p0f), (l_w.y * 0x1.266666p1f)));
                                    l_c = coax_p_sb_mix(coax_p_v3(0x1.1eb852p-3f, 0x1.99999ap-3f, 0x1.99999ap-4f), coax_p_v3(0x1.333334p-2f, 0x1.70a3d8p-2f, 0x1.70a3d8p-3f), l_nz);
                                  }
                                }
                                else
                                {
                                  if ((l_k == 17i))
                                  {
                                    l_rr = coax_p_wg_length2(coax_p_v2((l_w.x - ((l_hit.f5.x + l_hit.f6.x) * 0x1.000000p-1f)), (l_w.z - ((l_hit.f5.z + l_hit.f6.z) * 0x1.000000p-1f))));
                                    l_c = coax_p_sb_mix(coax_p_v3(0x1.c28f5cp-1f, 0x1.b33334p-1f, 0x1.8f5c28p-1f), coax_p_v3(0x1.70a3d8p-1f, 0x1.666666p-1f, 0x1.51eb86p-1f), coax_p_slab_band(l_rr, 0x1.800000p2f, 0x1.eb851ep-4f));
                                  }
                                  else
                                  {
                                    if ((l_k == 28i))
                                    {
                                      l_dcx = ((l_hit.f5.x + l_hit.f6.x) * 0x1.000000p-1f);
                                      l_dcz = ((l_hit.f5.z + l_hit.f6.z) * 0x1.000000p-1f);
                                      l_rr_79 = sqrt((((l_w.x - l_dcx) * (l_w.x - l_dcx)) + ((l_w.z - l_dcz) * (l_w.z - l_dcz))));
                                      l_yc = (l_hit.f5.y + 0x1.000000p4f);
                                      l_big = ((l_hit.f6.x - l_hit.f5.x) * 0x1.000000p-1f);
                                      if ((l_w.y < (l_yc - 0x1.99999ap-5f)))
                                      {
                                        l_c = coax_p_sb_mul(coax_p_v3(0x1.a3d70ap-1f, 0x1.99999ap-1f, 0x1.800000p-1f), (0x1.e147aep-1f + (0x1.eb851ep-5f * coax_p_slab_band(l_w.y, 0x1.000000p2f, 0x1.47ae14p-4f))));
                                        l_c = coax_p_sb_mix(coax_p_v3(0x1.99999ap-3f, 0x1.147ae2p-2f, 0x1.c28f5cp-3f), l_c, coax_p_smoothstep((l_hit.f5.y + 0x1.600000p3f), (l_hit.f5.y + 0x1.b00000p3f), l_w.y));
                                      }
                                      else
                                      {
                                        if ((l_n.y > 0x1.000000p-1f))
                                        {
                                          l_tier = coax_p_wg_round(((l_w.y - l_yc) / 0x1.800000p2f));
                                          l_edge_83 = (l_big - (0x1.800000p2f * l_tier));
                                          l_c = coax_p_v3(0x1.b851ecp-1f, 0x1.ae147ap-1f, 0x1.8f5c28p-1f);
                                          if ((l_rr_79 > (l_edge_83 - 0x1.4cccccp1f)))
                                          {
                                            l_c = coax_p_sb_mix(coax_p_v3(0x1.0a3d70p-2f, 0x1.70a3d8p-2f, 0x1.47ae14p-3f), coax_p_v3(0x1.851eb8p-2f, 0x1.d70a3ep-2f, 0x1.c28f5cp-3f), coax_p_slab_noise(coax_p_v2((l_w.x * 0x1.000000p-1f), (l_w.z * 0x1.000000p-1f))));
                                          }
                                          if ((l_w.y > (l_yc + 0x1.d00000p4f)))
                                          {
                                            l_c = select(coax_p_v3(0x1.99999ap-1f, 0x1.a3d70ap-1f, 0x1.99999ap-1f), coax_p_v3(0x1.ccccccp-1f, 0x1.ccccccp-1f, 0x1.c28f5cp-1f), (l_rr_79 < 0x1.200000p2f));
                                          }
                                        }
                                        else
                                        {
                                          l_ang = (coax_p_wg_atan2((l_w.z - l_dcz), (l_w.x - l_dcx)) * l_rr_79);
                                          l_mull_85 = coax_p_fmax(coax_p_slab_band(l_ang, 0x1.800000p1f, 0x1.99999ap-4f), coax_p_slab_band((l_w.y - l_yc), 0x1.800000p1f, 0x1.47ae14p-4f));
                                          l_c = coax_p_sb_mix(coax_p_v3(0x1.333334p-2f, 0x1.851eb8p-2f, 0x1.ae147ap-2f), coax_p_v3(0x1.d70a3ep-1f, 0x1.d70a3ep-1f, 0x1.ccccccp-1f), l_mull_85);
                                          if ((l_w.y > (l_yc + 0x1.e00000p4f)))
                                          {
                                            l_c = coax_p_sb_mix(coax_p_v3(0x1.3d70a4p-1f, 0x1.7ae148p-1f, 0x1.8f5c28p-1f), coax_p_v3(0x1.d70a3ep-1f, 0x1.d70a3ep-1f, 0x1.ccccccp-1f), l_mull_85);
                                          }
                                          l_emit = coax_p_sb_mul(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.3d70a4p-1f), (((0x1.000000p0f - l_mull_85) * coax_at_u(i32(1479i))) * 0x1.000000p-1f));
                                        }
                                      }
                                    }
                                    else
                                    {
                                      if ((l_k == 30i))
                                      {
                                        l_c = select(select(coax_p_v3(0x1.ccccccp-1f, 0x1.ccccccp-1f, 0x1.c28f5cp-1f), coax_p_v3(0x1.eb851ep-3f, 0x1.0a3d70p-2f, 0x1.28f5c2p-2f), (l_w.y < 0x1.01999ap3f)), coax_p_v3(0x1.ccccccp-2f, 0x1.eb851ep-4f, 0x1.70a3d8p-4f), (l_w.y < 0x1.000000p-1f));
                                        if ((((l_w.y > 0x1.01999ap3f) && (l_n.y < 0x1.000000p-1f)) && (l_w.y > (l_hit.f5.y + 0x1.400000p3f))))
                                        {
                                          l_c = coax_p_sb_mix(l_c, coax_p_v3(0x1.99999ap-3f, 0x1.0a3d70p-2f, 0x1.47ae14p-2f), coax_p_slab_band(l_w.y, 0x1.800000p1f, 0x1.666666p-2f));
                                        }
                                        if ((l_hit.f6.y > 0x1.e00000p4f))
                                        {
                                          l_c = select(coax_p_v3(0x1.3d70a4p-1f, 0x1.428f5cp-1f, 0x1.47ae14p-1f), coax_p_v3(0x1.333334p-3f, 0x1.333334p-3f, 0x1.47ae14p-3f), (l_w.y > 0x1.f00000p4f));
                                        }
                                      }
                                      else
                                      {
                                        if ((l_k == 31i))
                                        {
                                          l_c = coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-4f, 0x1.99999ap-4f);
                                        }
                                        else
                                        {
                                          if ((l_k == 29i))
                                          {
                                            l_c = select(coax_p_v3(0x1.28f5c2p-1f, 0x1.23d70ap-1f, 0x1.19999ap-1f), coax_p_v3(0x1.666666p-1f, 0x1.6147aep-1f, 0x1.51eb86p-1f), (l_n.y > 0x1.000000p-1f));
                                          }
                                          else
                                          {
                                            if ((l_k == 27i))
                                            {
                                              l_c = select(coax_p_v3(0x1.e66666p-1f, 0x1.dc28f6p-1f, 0x1.c28f5cp-1f), coax_p_v3(0x1.b851ecp-1f, 0x1.ae147ap-1f, 0x1.947ae2p-1f), (l_w.y < (l_hit.f5.y + 0x1.ccccccp0f)));
                                            }
                                            else
                                            {
                                              if ((l_k == 19i))
                                              {
                                                l_c = coax_p_v3(0x1.e66666p-1f, 0x1.f0a3d8p-1f, 0x1.000000p0f);
                                              }
                                              else
                                              {
                                                if ((l_k == 41i))
                                                {
                                                  l_c = coax_p_v3(0x1.b851ecp-1f, 0x1.47ae14p-1f, 0x1.000000p-1f);
                                                }
                                                else
                                                {
                                                  if ((l_k == 42i))
                                                  {
                                                    l_c = coax_p_v3(0x1.e66666p-1f, 0x1.ccccccp-2f, 0x1.70a3d8p-3f);
                                                  }
                                                  else
                                                  {
                                                    if ((l_k == 43i))
                                                    {
                                                      l_c = coax_p_v3(0x1.47ae14p-3f, 0x1.99999ap-3f, 0x1.333334p-2f);
                                                    }
                                                    else
                                                    {
                                                      if ((l_k == 44i))
                                                      {
                                                        l_c = coax_p_v3(0x1.eb851ep-4f, 0x1.70a3d8p-4f, 0x1.1eb852p-4f);
                                                      }
                                                      else
                                                      {
                                                        if ((l_k == 45i))
                                                        {
                                                          l_c = coax_p_v3(0x1.99999ap-3f, 0x1.851eb8p-3f, 0x1.47ae14p-3f);
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
  return coax_t21(coax_p_sb_mul(l_c, l_grain), l_emit);
}
fn coax_p_sb_mulv(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_slab_sea_through(p_c: vec3f, p_path: f32, p_y: f32, p_eye: f32, p_up: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_path: f32 = p_path;
  let l_y: f32 = p_y;
  let l_eye: f32 = p_eye;
  let l_up: f32 = p_up;
  var l_tr: vec3f;
  var l_day: vec3f;
  var l_top: vec3f;
  var l_fill: f32;
  var l_lit: vec3f;
  var l_fog: vec3f;
  l_tr = coax_p_slab_sea_beam(l_path);
  l_day = coax_p_slab_sea_daylight((-coax_p_fmin(l_y, 0.0f)));
  l_top = coax_p_slab_sea_daylight(0.0f);
  l_fill = coax_at_u(i32(15864i));
  l_lit = coax_p_v3(((l_day.x / coax_p_fmax(l_top.x, 0x1.a36e2ep-14f)) + l_fill), ((l_day.y / coax_p_fmax(l_top.y, 0x1.a36e2ep-14f)) + l_fill), ((l_day.z / coax_p_fmax(l_top.z, 0x1.a36e2ep-14f)) + l_fill));
  l_fog = coax_p_slab_sea_fog(l_eye, l_up);
  return coax_p_slab_sea_out(coax_p_sb_add(coax_p_sb_mulv(coax_p_sb_mulv(coax_p_slab_sea_lin(l_c), l_lit), l_tr), coax_p_sb_mulv(l_fog, coax_p_v3((0x1.000000p0f - l_tr.x), (0x1.000000p0f - l_tr.y), (0x1.000000p0f - l_tr.z)))));
}
fn coax_p_sb_pow(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return coax_pow_bg_vs(coax_p_fmax(l_x, 0x1.0c6f7ap-20f), l_y);
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
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_slab_shoulder(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  if ((l_x < 0x1.666666p-1f))
  {
    return l_x;
  }
  return (0x1.666666p-1f + (0x1.333334p-2f * (0x1.000000p0f - exp(((-(l_x - 0x1.666666p-1f)) / 0x1.333334p-2f)))));
}
fn coax_p_slab_sea_fog(p_y: f32, p_up: f32) -> vec3f {
  let l_y: f32 = p_y;
  let l_up: f32 = p_up;
  var l_deep: vec3f;
  l_deep = coax_p_v3(coax_at_u(i32(15844i)), coax_at_u(i32(15845i)), coax_at_u(i32(15846i)));
  return coax_p_sb_mul(coax_p_sb_mulv(l_deep, coax_p_slab_sea_daylight((-coax_p_fmin(l_y, 0.0f)))), (0x1.800000p1f * (0x1.000000p-1f + (0x1.000000p-1f * coax_p_smoothstep((-0x1.000000p0f), 0x1.333334p-1f, l_up)))));
}
fn coax_p_slab_sea_out(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_v3(sqrt(coax_p_fmax(l_c.x, 0.0f)), sqrt(coax_p_fmax(l_c.y, 0.0f)), sqrt(coax_p_fmax(l_c.z, 0.0f)));
}
fn coax_p_wo_caustic(p_p: vec2f, p_t: f32) -> f32 {
  let l_p: vec2f = p_p;
  let l_t: f32 = p_t;
  var l_a: f32;
  var l_b: f32;
  var l_r: f32;
  l_a = coax_p_wo_noise(coax_p_v2(((l_p.x * 0x1.ccccccp-1f) + (l_t * 0x1.3d70a4p-2f)), ((l_p.y * 0x1.ccccccp-1f) - (l_t * 0x1.d70a3ep-3f))));
  l_b = coax_p_wo_noise(coax_p_v2((((l_p.x * 0x1.b33334p0f) - (l_t * 0x1.851eb8p-3f)) + 0x1.d33334p2f), (((l_p.y * 0x1.b33334p0f) + (l_t * 0x1.147ae2p-2f)) + 0x1.0cccccp1f)));
  l_r = ((0x1.000000p0f - abs(((0x1.000000p1f * l_a) - 0x1.000000p0f))) * (0x1.000000p0f - abs(((0x1.000000p1f * l_b) - 0x1.000000p0f))));
  return (l_r * l_r);
}
fn coax_p_slab_sea_daylight(p_down: f32) -> vec3f {
  let l_down: f32 = p_down;
  var l_top: vec3f;
  l_top = coax_p_sb_add(coax_p_sb_mul(coax_p_slab_sea_sun(), (coax_p_fmax(coax_at_u(i32(25i)), 0.0f) * (0x1.000000p0f - coax_p_wo_fresnel(coax_p_fmax(coax_at_u(i32(25i)), 0x1.47ae14p-6f))))), coax_p_sb_mul(coax_p_slab_sea_skylight(), 0x1.e147aep-1f));
  return coax_p_sb_mulv(l_top, coax_p_wo_through(coax_p_slab_sea_kd(), (coax_p_fmax(l_down, 0.0f) / coax_p_slab_sea_clear())));
}
fn coax_p_slab_sea_beam(p_path: f32) -> vec3f {
  let l_path: f32 = p_path;
  var l_k: vec3f;
  var l_b: f32;
  l_k = coax_p_slab_sea_kd();
  l_b = coax_at_u(i32(15847i));
  return coax_p_wo_through(coax_p_v3((l_k.x + l_b), (l_k.y + l_b), (l_k.z + l_b)), (l_path / coax_p_slab_sea_clear()));
}
fn coax_p_slab_sea_shafts(p_o: vec3f, p_d: vec3f, p_limit: f32) -> f32 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_limit: f32 = p_limit;
  var l_sun: vec3f;
  var l_sy: f32;
  var l_sh: f32;
  var l_ray: vec3f;
  var l_steps: i32;
  var l_acc: f32;
  var l_at: f32;
  var l_i: i32;
  var l_q: vec3f;
  var l_rise: f32;
  var l_top: vec2f;
  var l_toward: f32;
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  if ((l_sun.y < 0x1.47ae14p-4f))
  {
    return 0.0f;
  }
  l_sy = coax_p_wo_cos_refracted(l_sun.y);
  l_sh = (sqrt(coax_p_fmax((0x1.000000p0f - (l_sy * l_sy)), 0.0f)) / coax_p_fmax(sqrt(((l_sun.x * l_sun.x) + (l_sun.z * l_sun.z))), 0x1.a36e2ep-14f));
  l_ray = coax_p_v3((l_sun.x * l_sh), l_sy, (l_sun.z * l_sh));
  l_steps = select(5i, 3i, (coax_at_u(i32(5i)) > 0x1.000000p-1f));
  l_acc = 0.0f;
  l_at = 0x1.800000p1f;
  l_i = 0i;
  while ((l_i < l_steps))
  {
    if ((l_at < l_limit))
    {
      l_q = coax_p_sb_add(l_o, coax_p_sb_mul(l_d, l_at));
      if ((l_q.y < 0.0f))
      {
        l_rise = ((-l_q.y) / l_ray.y);
        l_top = coax_p_v2((l_q.x + (l_ray.x * l_rise)), (l_q.z + (l_ray.z * l_rise)));
        l_acc = (l_acc + ((coax_p_wo_caustic(coax_p_wg_mul_f2f(l_top, 0x1.c28f5cp-4f), (coax_at_u(i32(2i)) * 0x1.333334p-1f)) * exp((l_q.y / (0x1.680000p5f * sqrt(coax_p_slab_sea_clear()))))) * exp(((-l_at) / (0x1.180000p6f * sqrt(coax_p_slab_sea_clear()))))));
      }
    }
    l_at = (l_at * 0x1.266666p1f);
    l_i = (l_i + 1i);
  }
  l_toward = (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_fmax(coax_p_wg_dot3(l_d, l_ray), 0.0f)));
  return ((l_acc * l_toward) * (coax_p_clampf((l_sun.y * 0x1.800000p1f), 0.0f, 0x1.000000p0f) * coax_at_u(i32(15860i))));
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
fn coax_p_slab_sea_fine(p_p: vec2f, p_patch: f32, p_hi: u32, p_scale: f32) -> vec2f {
  let l_p: vec2f = p_p;
  let l_patch: f32 = p_patch;
  let l_hi: u32 = p_hi;
  let l_scale: f32 = p_scale;
  var l_q: vec2f;
  var l_f: vec2f;
  var l_i: coax_t19;
  var l_acc: vec2f;
  var l_k: i32;
  var l_o: coax_t19;
  var l_c: coax_t20;
  var l_w: f32;
  var l_half: u32;
  l_q = coax_p_wg_sub_f2f(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(l_p, l_patch), f32(64u)), 0x1.000000p-1f);
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
      l_half = ((coax_p_slab_word(((10240u + (l_c.y * 64u)) + l_c.x)) >> u32((16u * l_hi))) & 65535u);
      l_acc = coax_p_wg_add_f2f2(l_acc, coax_p_wg_mul_ff2(l_w, coax_p_wg_sw_xy_f4(coax_p_wg_unpack4x8snorm(l_half))));
      l_k = (l_k + 1i);
    }
  }
  return coax_p_wg_mul_f2f(l_acc, l_scale);
}
fn coax_p_wo_below(p_ci: f32) -> f32 {
  let l_ci: f32 = p_ci;
  var l_st2: f32;
  l_st2 = ((0x1.547ae2p0f * 0x1.547ae2p0f) * (0x1.000000p0f - (l_ci * l_ci)));
  return select(0x1.000000p0f, coax_p_wo_fresnel(sqrt((0x1.000000p0f - l_st2))), (l_st2 < 0x1.000000p0f));
}
fn coax_p_slab_sea_lin(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_v3((l_c.x * l_c.x), (l_c.y * l_c.y), (l_c.z * l_c.z));
}
fn coax_p_slab_sea_sun() -> vec3f {
  return coax_p_slab_sea_lin(coax_p_sb_p(4i));
}
fn coax_p_slab_sea(p_p: vec2f, p_world: vec3f, p_d: vec3f, p_dist: f32, p_t: f32, p_c: vec4f) -> vec4f {
  let l_p: vec2f = p_p;
  let l_world: vec3f = p_world;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  let l_t: f32 = p_t;
  let l_c: vec4f = p_c;
  var l_phone: bool;
  var l_hs: f32;
  var l_wind: vec2f;
  var l_footprint: f32;
  var l_keep0: f32;
  var l_keep1: f32;
  var l_patch2: f32;
  var l_keep2: f32;
  var l_slope: vec2f;
  var l_at: vec2f;
  var l_sigma2: f32;
  var l_n: vec3f;
  var l_v: vec3f;
  var l_sun: vec3f;
  var l_sunl: vec3f;
  var l_skyl: vec3f;
  var l_cv: f32;
  var l_fres: f32;
  var l_r: vec3f;
  var l_sk: vec3f;
  var l_col: vec3f;
  var l_ndl: f32;
  var l_into: f32;
  var l_deep: vec3f;
  var l_light: vec3f;
  var l_high: f32;
  var l_crest: f32;
  var l_flat: f32;
  var l_gate: f32;
  var l_near: f32;
  var l_wa: f32;
  var l_wc: f32;
  var l_f1: f32;
  var l_foam: f32;
  var l_white: f32;
  var l_grain: f32;
  var l_along: f32;
  var l_f0: f32;
  var l_body: f32;
  var l_lace: f32;
  var l_line: f32;
  var l_line2: f32;
  var l_smoke: f32;
  var l_veil: f32;
  var l_o: vec3f;
  l_phone = (coax_at_u(i32(5i)) > 0x1.000000p-1f);
  l_hs = coax_p_fmax(coax_at_u(i32(35i)), 0x1.99999ap-5f);
  l_wind = coax_p_v2(coax_at_u(i32(32i)), coax_at_u(i32(33i)));
  l_footprint = ((((l_dist * 0x1.000000p1f) * coax_at_u(i32(11i))) / coax_at_u(i32(1i))) / coax_p_fmax(abs(l_d.y), 0x1.47ae14p-4f));
  l_keep0 = (0x1.333334p-2f + (0x1.666666p-1f * coax_p_clampf((((coax_at_u(i32(29i)) / 0x1.000000p4f) / coax_p_fmax(l_footprint, 0x1.a36e2ep-14f)) - 0x1.000000p-1f), 0.0f, 0x1.000000p0f)));
  l_keep1 = coax_p_clampf((((coax_at_u(i32(30i)) / 0x1.000000p6f) / coax_p_fmax(l_footprint, 0x1.a36e2ep-14f)) - 0x1.000000p-1f), 0.0f, 0x1.000000p0f);
  l_patch2 = coax_at_u(i32(15855i));
  l_keep2 = select(0.0f, coax_p_clampf((((l_patch2 / 0x1.000000p6f) / coax_p_fmax(l_footprint, 0x1.a36e2ep-14f)) - 0x1.000000p-1f), 0.0f, 0x1.000000p0f), (l_patch2 > 0.0f));
  l_slope = coax_p_wg_mul_f2f(coax_p_v2(l_c.z, l_c.w), l_keep0);
  if ((l_keep1 > 0.0f))
  {
    l_at = select(l_p, coax_p_wg_add_f2f2(l_p, coax_p_wg_mul_f2f(l_wind, (l_t * 0x1.333334p-2f))), l_phone);
    l_slope = coax_p_wg_add_f2f2(l_slope, coax_p_wg_mul_f2f(coax_p_slab_sea_fine(l_at, coax_at_u(i32(30i)), 0u, coax_at_u(i32(42i))), l_keep1));
  }
  if ((l_keep2 > 0.0f))
  {
    l_slope = coax_p_wg_add_f2f2(l_slope, coax_p_wg_mul_f2f(coax_p_slab_sea_fine(l_p, l_patch2, 1u, coax_at_u(i32(15862i))), l_keep2));
  }
  l_sigma2 = (((coax_at_u(i32(15851i)) + ((0x1.000000p0f - (l_keep0 * l_keep0)) * coax_at_u(i32(15848i)))) + ((0x1.000000p0f - (l_keep1 * l_keep1)) * coax_at_u(i32(15849i)))) + ((0x1.000000p0f - (l_keep2 * l_keep2)) * coax_at_u(i32(15850i))));
  l_n = coax_p_wg_normalize3(coax_p_v3((-l_slope.x), 0x1.000000p0f, (-l_slope.y)));
  l_v = coax_p_sb_mul(l_d, (-0x1.000000p0f));
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_sunl = coax_p_slab_sea_sun();
  l_skyl = coax_p_slab_sea_skylight();
  l_cv = coax_p_fmax(coax_p_wg_dot3(l_n, l_v), 0x1.47ae14p-6f);
  l_fres = coax_p_wo_mean_fresnel(l_cv, l_sigma2);
  l_r = coax_p_wg_reflect3(l_d, l_n);
  l_r = coax_p_wg_normalize3(coax_p_v3(l_r.x, (abs(l_r.y) + (0x1.333334p0f * sqrt(l_sigma2))), l_r.z));
  l_sk = coax_p_slab_sky(l_r);
  l_col = coax_p_sb_mul(coax_p_slab_sea_lin(coax_p_v3(coax_p_fmin(l_sk.x, 0x1.400000p0f), coax_p_fmin(l_sk.y, 0x1.400000p0f), coax_p_fmin(l_sk.z, 0x1.400000p0f))), l_fres);
  l_ndl = coax_p_fmax(coax_p_wg_dot3(l_n, l_sun), 0.0f);
  l_into = (0x1.000000p0f - coax_p_wo_fresnel(coax_p_fmax(l_sun.y, 0x1.47ae14p-6f)));
  l_deep = coax_p_v3(coax_at_u(i32(15844i)), coax_at_u(i32(15845i)), coax_at_u(i32(15846i)));
  l_light = coax_p_sb_add(coax_p_sb_mul(l_sunl, (l_ndl * l_into)), coax_p_sb_mul(l_skyl, 0x1.e147aep-1f));
  l_col = coax_p_sb_add(l_col, coax_p_sb_mul(coax_p_sb_mulv(l_deep, l_light), (0x1.147ae2p-1f * (0x1.000000p0f - l_fres))));
  l_col = coax_p_sb_add(l_col, coax_p_sb_mul(l_sunl, (0x1.921fb6p1f * coax_p_wo_glint(l_n, l_v, l_sun, coax_p_wo_alpha(l_sigma2), 0x1.30be0ep-8f, 0.0f))));
  l_high = (l_c.x / l_hs);
  l_crest = coax_p_smoothstep(0x1.99999ap-5f, 0x1.ccccccp-2f, l_high);
  l_flat = coax_p_fmax((((l_d.x * l_sun.x) + (l_d.z * l_sun.z)) / coax_p_fmax(sqrt((((l_d.x * l_d.x) + (l_d.z * l_d.z)) * ((l_sun.x * l_sun.x) + (l_sun.z * l_sun.z)))), 0x1.a36e2ep-14f)), 0.0f);
  l_col = coax_p_sb_add(l_col, coax_p_sb_mul(coax_p_sb_mulv(l_sunl, coax_p_wo_through(coax_p_slab_sea_kd(), 0x1.800000p2f)), ((0x1.c28f5cp-4f * l_crest) * ((l_flat * l_flat) * (l_into * (0x1.000000p0f - l_fres))))));
  l_gate = coax_p_smoothstep((-0x1.99999ap-5f), 0x1.99999ap-3f, l_high);
  l_near = (0x1.000000p0f - coax_p_smoothstep(0x1.2c0000p9f, 0x1.b58000p11f, l_dist));
  l_wa = ((l_p.x * l_wind.x) + (l_p.y * l_wind.y));
  l_wc = ((l_p.y * l_wind.x) - (l_p.x * l_wind.y));
  l_f1 = 0.0f;
  if (((coax_at_u(i32(15852i)) > 0x1.000000p-1f) && (l_keep1 > 0.0f)))
  {
    l_f1 = ((coax_p_slab_sea_foam1(l_p) * l_gate) * l_keep1);
  }
  l_foam = coax_p_fmax(l_c.y, l_f1);
  l_white = 0.0f;
  if (((l_foam > 0x1.47ae14p-6f) || (coax_at_u(i32(15853i)) > 0.0f)))
  {
    l_grain = (((coax_p_slab_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.ae147ap-3f), coax_p_v2((l_t * 0x1.eb851ep-4f), 0.0f))) * 0x1.000000p-1f) + (coax_p_slab_noise(coax_p_wg_mul_f2f(l_p, 0x1.428f5cp-1f)) * 0x1.333334p-2f)) + (coax_p_slab_noise(coax_p_wg_mul_f2f(l_p, 0x1.e66666p0f)) * 0x1.99999ap-3f));
    l_along = coax_p_slab_noise(coax_p_v2(((l_wa * 0x1.1eb852p-4f) - (l_t * 0x1.99999ap-5f)), (l_wc * 0x1.0624dep-6f)));
    l_f0 = ((l_c.y * l_gate) * ((0x1.99999ap-3f + (0x1.99999ap-1f * l_grain)) + (0x1.800000p-1f * l_along)));
    l_body = coax_p_fmax(coax_p_fmin(l_f0, 0x1.000000p0f), ((l_f1 * (0x1.19999ap-1f + (0x1.ccccccp-2f * l_along))) * (0x1.333334p-1f + (0x1.333334p-1f * l_grain))));
    l_lace = (((coax_p_slab_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_p, 0x1.19999ap-1f), coax_p_v2((l_t * 0x1.47ae14p-4f), (l_t * 0x1.99999ap-5f)))) * 0x1.000000p-1f) + (coax_p_slab_noise(coax_p_wg_mul_f2f(l_p, 0x1.e66666p0f)) * 0x1.333334p-2f)) + (coax_p_slab_noise(coax_p_wg_mul_f2f(l_p, 0x1.533334p2f)) * 0x1.99999ap-3f));
    l_white = (coax_p_smoothstep(((l_lace * 0x1.400000p0f) - 0x1.333334p-2f), ((l_lace * 0x1.400000p0f) + 0x1.000000p-2f), l_body) * (0x1.666666p-1f + (0x1.333334p-2f * coax_p_slab_noise(coax_p_wg_mul_f2f(l_p, 0x1.266666p1f)))));
    l_line = coax_p_slab_sea_ridge(coax_p_slab_noise(coax_p_v2(((l_wa * 0x1.6872b0p-7f) - (l_t * 0x1.eb851ep-6f)), (l_wc * 0x1.333334p-2f))), (0x1.1eb852p-4f + (l_footprint * 0x1.0624dep-8f)));
    l_line2 = coax_p_slab_sea_ridge(coax_p_slab_noise(coax_p_v2((((l_wa * 0x1.ba5e36p-6f) - (l_t * 0x1.99999ap-5f)) + 0x1.000000p2f), ((l_wc * 0x1.666666p-1f) + 0x1.200000p3f))), (0x1.eb851ep-5f + (l_footprint * 0x1.0624dep-8f)));
    l_white = coax_p_fmax(l_white, ((coax_at_u(i32(15853i)) * (0x1.333334p-2f + (0x1.666666p-1f * coax_p_smoothstep(0x1.333334p-2f, 0x1.666666p-1f, coax_p_slab_noise(coax_p_wg_mul_f2f(l_p, 0x1.89374cp-7f)))))) * (((l_line * 0x1.000000p-1f) + (l_line2 * 0x1.666666p-2f)) * (0x1.000000p-1f + (0x1.000000p-1f * l_grain)))));
  }
  l_white = coax_p_mix((coax_at_u(i32(15865i)) * (0x1.99999ap-2f + (0x1.333334p-1f * l_gate))), l_white, l_near);
  l_smoke = ((coax_at_u(i32(15854i)) * l_gate) * coax_p_smoothstep(0x1.666666p-2f, 0x1.99999ap-1f, coax_p_slab_noise(coax_p_v2(((l_wa * 0x1.eb851ep-6f) - (l_t * 0x1.19999ap0f)), (l_wc * 0x1.eb851ep-4f)))));
  l_white = coax_p_clampf(coax_p_fmax(l_white, (l_smoke * 0x1.666666p-1f)), 0.0f, 0x1.000000p0f);
  l_col = coax_p_sb_add(l_col, coax_p_sb_mul(coax_p_sb_mulv(coax_p_wo_through(coax_p_slab_sea_kd(), 0x1.800000p1f), l_light), ((0x1.99999ap-5f * coax_p_smoothstep(0x1.99999ap-5f, 0x1.000000p-1f, l_foam)) * (0x1.000000p0f - l_fres))));
  l_col = coax_p_sb_mix(l_col, coax_p_slab_sea_foam_lit(l_ndl), l_white);
  l_veil = (coax_at_u(i32(15854i)) * (0x1.000000p0f - exp(((-l_dist) / 0x1.5e0000p9f))));
  l_col = coax_p_sb_mix(l_col, coax_p_sb_mul(coax_p_slab_sea_lin(coax_p_sb_p(12i)), 0x1.ccccccp-1f), (l_veil * 0x1.99999ap-1f));
  l_o = coax_p_slab_sea_out(l_col);
  return coax_p_v4(l_o.x, l_o.y, l_o.z, l_white);
}
fn coax_p_slab_hull_plan(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  var l_qx: f32;
  var l_qz: f32;
  var l_out: f32;
  l_qx = ((abs(l_p.x) - coax_at_u(i32(36i))) + 0x1.800000p0f);
  l_qz = ((abs(l_p.y) - coax_at_u(i32(37i))) + 0x1.800000p0f);
  l_out = sqrt(((coax_p_fmax(l_qx, 0.0f) * coax_p_fmax(l_qx, 0.0f)) + (coax_p_fmax(l_qz, 0.0f) * coax_p_fmax(l_qz, 0.0f))));
  return ((l_out + coax_p_fmin(coax_p_fmax(l_qx, l_qz), 0.0f)) - 0x1.800000p0f);
}
fn coax_p_slab_sea_white() -> vec3f {
  return coax_p_slab_sea_out(coax_p_slab_sea_foam_lit(coax_p_fmax(coax_at_u(i32(25i)), 0.0f)));
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
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t20 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  return coax_t20(l_x, l_y);
}
fn coax_p_wg_at4u(p_at: i32) -> coax_t22 {
  let l_at: i32 = p_at;
  return coax_p_u4(coax_p_wg_atu(l_at), coax_p_wg_atu((l_at + 1i)), coax_p_wg_atu((l_at + 2i)), coax_p_wg_atu((l_at + 3i)));
}
fn coax_p_wg_get_u4(p_p: coax_t22, p_i: i32) -> u32 {
  let l_p: coax_t22 = p_p;
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
fn coax_p_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
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
fn coax_p_slab_gable_hit(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f, p_along_x: bool) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_lo: vec3f = p_lo;
  let l_hi: vec3f = p_hi;
  let l_along_x: bool = p_along_x;
  var l_h: vec4f;
  var l_tn: f32;
  var l_nrm: vec3f;
  var l_sp: vec2f;
  var l_tf: f32;
  var l_across: f32;
  var l_mid: f32;
  var l_rh: f32;
  var l_oc: f32;
  var l_dc: f32;
  var l_k: i32;
  var l_sg: f32;
  var l_ny: f32;
  var l_nc: f32;
  var l_dd: f32;
  var l_den: f32;
  var l_dist: f32;
  var l_t: f32;
  var l_nn: vec3f;
  l_h = coax_p_slab_box_hit(l_o, l_d, l_lo, l_hi);
  l_tn = l_h.x;
  l_nrm = coax_p_v3(l_h.y, l_h.z, l_h.w);
  if ((l_tn >= 0x1.dcd650p29f))
  {
    return l_h;
  }
  l_sp = coax_p_slab_span(l_o, l_d, l_lo, l_hi);
  l_tf = l_sp.y;
  l_across = select(((l_hi.x - l_lo.x) * 0x1.000000p-1f), ((l_hi.z - l_lo.z) * 0x1.000000p-1f), l_along_x);
  l_mid = select(((l_hi.x + l_lo.x) * 0x1.000000p-1f), ((l_hi.z + l_lo.z) * 0x1.000000p-1f), l_along_x);
  l_rh = coax_p_fmin((l_across * 0x1.70a3d8p-2f), ((l_hi.y - l_lo.y) * 0x1.ccccccp-2f));
  l_oc = select(l_o.x, l_o.z, l_along_x);
  l_dc = select(l_d.x, l_d.z, l_along_x);
  l_k = 0i;
  while ((l_k < 2i))
  {
    l_sg = select(0x1.000000p0f, (-0x1.000000p0f), (l_k == 0i));
    l_ny = l_across;
    l_nc = (l_sg * l_rh);
    l_dd = ((l_ny * l_hi.y) + (l_nc * l_mid));
    l_den = ((l_ny * l_d.y) + (l_nc * l_dc));
    l_dist = (l_dd - ((l_ny * l_o.y) + (l_nc * l_oc)));
    if ((abs(l_den) < 0x1.ad7f2ap-24f))
    {
      if ((l_dist < 0.0f))
      {
        return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
      }
    }
    else
    {
      l_t = (l_dist / l_den);
      if ((l_den < 0.0f))
      {
        if ((l_t > l_tn))
        {
          l_tn = l_t;
          l_nn = select(coax_p_v3(l_nc, l_ny, 0.0f), coax_p_v3(0.0f, l_ny, l_nc), l_along_x);
          l_nrm = coax_p_wg_normalize3(l_nn);
        }
      }
      else
      {
        l_tf = coax_p_fmin(l_tf, l_t);
      }
    }
    l_k = (l_k + 1i);
  }
  if (((l_tf < l_tn) || (l_tn < 0.0f)))
  {
    return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  }
  return coax_p_v4(l_tn, l_nrm.x, l_nrm.y, l_nrm.z);
}
fn coax_p_slab_grove_hit(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f, p_glade: bool, p_limit: f32) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_lo: vec3f = p_lo;
  let l_hi: vec3f = p_hi;
  let l_glade: bool = p_glade;
  let l_limit: f32 = p_limit;
  var l_sp: vec2f;
  var l_ground: f32;
  var l_t: f32;
  var l_tend: f32;
  var l_tg: f32;
  var l_p: vec3f;
  var l_cx: f32;
  var l_cz: f32;
  var l_stx: f32;
  var l_stz: f32;
  var l_tdx: f32;
  var l_tdz: f32;
  var l_tmx: f32;
  var l_tmz: f32;
  var l_gcx: f32;
  var l_gcz: f32;
  var l_ghx: f32;
  var l_ghz: f32;
  var l_i: i32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_tx: f32;
  var l_tz: f32;
  var l_r: f32;
  var l_h: f32;
  var l_inside: bool;
  var l_clear: bool;
  var l_cy: vec4f;
  l_sp = coax_p_slab_span(l_o, l_d, l_lo, l_hi);
  if ((((l_sp.y < l_sp.x) || (l_sp.y < 0.0f)) || (l_sp.x >= l_limit)))
  {
    return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  }
  l_ground = (l_lo.y + 0x1.333334p-2f);
  l_t = coax_p_fmax(l_sp.x, 0.0f);
  l_tend = coax_p_fmin(l_sp.y, l_limit);
  l_tg = 0x1.dcd650p29f;
  if ((l_d.y < 0.0f))
  {
    l_tg = ((l_ground - l_o.y) / l_d.y);
    if ((l_tg < l_t))
    {
      l_tg = 0x1.dcd650p29f;
    }
  }
  l_tend = coax_p_fmin(l_tend, l_tg);
  l_p = coax_p_sb_add(l_o, coax_p_sb_mul(l_d, (l_t + 0x1.0624dep-10f)));
  l_cx = floor((l_p.x / 0x1.c00000p2f));
  l_cz = floor((l_p.z / 0x1.c00000p2f));
  l_stx = select((-0x1.000000p0f), 0x1.000000p0f, (l_d.x > 0.0f));
  l_stz = select((-0x1.000000p0f), 0x1.000000p0f, (l_d.z > 0.0f));
  l_tdx = select(0x1.dcd650p29f, (0x1.c00000p2f / abs(l_d.x)), (abs(l_d.x) > 0x1.0c6f7ap-20f));
  l_tdz = select(0x1.dcd650p29f, (0x1.c00000p2f / abs(l_d.z)), (abs(l_d.z) > 0x1.0c6f7ap-20f));
  l_tmx = select(0x1.dcd650p29f, ((((l_cx + select(0.0f, 0x1.000000p0f, (l_d.x > 0.0f))) * 0x1.c00000p2f) - l_o.x) / l_d.x), (abs(l_d.x) > 0x1.0c6f7ap-20f));
  l_tmz = select(0x1.dcd650p29f, ((((l_cz + select(0.0f, 0x1.000000p0f, (l_d.z > 0.0f))) * 0x1.c00000p2f) - l_o.z) / l_d.z), (abs(l_d.z) > 0x1.0c6f7ap-20f));
  l_gcx = ((l_lo.x + l_hi.x) * 0x1.000000p-1f);
  l_gcz = ((l_lo.z + l_hi.z) * 0x1.000000p-1f);
  l_ghx = ((l_hi.x - l_lo.x) * 0x1.70a3d8p-3f);
  l_ghz = ((l_hi.z - l_lo.z) * 0x1.70a3d8p-3f);
  l_i = 0i;
  while (((l_i < 56i) && (l_t < l_tend)))
  {
    l_a = coax_p_render_noise2(l_cx, l_cz);
    l_b = coax_p_render_noise2((l_cx + 0x1.100000p4f), (l_cz - 0x1.200000p3f));
    l_c = coax_p_render_noise2((l_cx - 0x1.f00000p4f), (l_cz + 0x1.400000p2f));
    l_tx = (((l_cx + 0x1.000000p-1f) * 0x1.c00000p2f) + ((l_b - 0x1.000000p-1f) * 0x1.19999ap1f));
    l_tz = (((l_cz + 0x1.000000p-1f) * 0x1.c00000p2f) + ((l_c - 0x1.000000p-1f) * 0x1.19999ap1f));
    l_r = (0x1.800000p0f + (0x1.99999ap-1f * l_b));
    l_h = (0x1.c00000p2f + (0x1.800000p2f * l_c));
    l_inside = ((((l_tx > (l_lo.x + 0x1.000000p0f)) && (l_tx < (l_hi.x - 0x1.000000p0f))) && (l_tz > (l_lo.z + 0x1.000000p0f))) && (l_tz < (l_hi.z - 0x1.000000p0f)));
    l_clear = ((l_glade && (abs((l_tx - l_gcx)) < l_ghx)) && (abs((l_tz - l_gcz)) < l_ghz));
    if ((((l_a > 0x1.99999ap-2f) && l_inside) && (!l_clear)))
    {
      l_cy = coax_p_slab_cypress(l_o, l_d, coax_p_v3(l_tx, l_ground, l_tz), l_r, l_h);
      if (((l_cy.x >= coax_p_fmax((l_sp.x - 0x1.000000p-1f), 0.0f)) && (l_cy.x < l_tend)))
      {
        return l_cy;
      }
    }
    if ((l_tmx < l_tmz))
    {
      l_t = l_tmx;
      l_tmx = (l_tmx + l_tdx);
      l_cx = (l_cx + l_stx);
    }
    else
    {
      l_t = l_tmz;
      l_tmz = (l_tmz + l_tdz);
      l_cz = (l_cz + l_stz);
    }
    l_i = (l_i + 1i);
  }
  if ((((l_tg < 0x1.dcd650p29f) && (l_tg <= l_sp.y)) && (l_tg < l_limit)))
  {
    return coax_p_v4(l_tg, 0.0f, 0x1.000000p0f, 0.0f);
  }
  return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
}
fn coax_p_slab_dome_hit(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_lo: vec3f = p_lo;
  let l_hi: vec3f = p_hi;
  var l_r: f32;
  var l_c: vec3f;
  var l_oc: vec3f;
  var l_b: f32;
  var l_q: f32;
  var l_disc: f32;
  var l_t: f32;
  var l_p: vec3f;
  var l_nn: vec3f;
  l_r = (coax_p_fmin((l_hi.x - l_lo.x), (l_hi.z - l_lo.z)) * 0x1.000000p-1f);
  l_c = coax_p_v3(((l_lo.x + l_hi.x) * 0x1.000000p-1f), l_lo.y, ((l_lo.z + l_hi.z) * 0x1.000000p-1f));
  l_oc = coax_p_sb_sub(l_o, l_c);
  l_b = coax_p_wg_dot3(l_oc, l_d);
  l_q = (coax_p_wg_dot3(l_oc, l_oc) - (l_r * l_r));
  l_disc = ((l_b * l_b) - l_q);
  if ((l_disc < 0.0f))
  {
    return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  }
  l_t = ((-l_b) - sqrt(l_disc));
  if ((l_t < 0.0f))
  {
    return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  }
  l_p = coax_p_sb_add(l_oc, coax_p_sb_mul(l_d, l_t));
  if ((l_p.y < 0.0f))
  {
    return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  }
  l_nn = coax_p_sb_mul(l_p, (0x1.000000p0f / l_r));
  return coax_p_v4(l_t, l_nn.x, l_nn.y, l_nn.z);
}
fn coax_p_slab_greek_hit(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f, p_round: bool, p_limit: f32) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_lo: vec3f = p_lo;
  let l_hi: vec3f = p_hi;
  let l_round: bool = p_round;
  let l_limit: f32 = p_limit;
  var l_sp: vec2f;
  var l_c: vec3f;
  var l_along: bool;
  var l_s: vec3f;
  var l_t: f32;
  var l_tend: f32;
  var l_i: i32;
  var l_w: vec3f;
  var l_q: vec3f;
  var l_dist: f32;
  var l_e: f32;
  var l_gx: f32;
  var l_gy: f32;
  var l_gz: f32;
  var l_nn: vec3f;
  l_sp = coax_p_slab_span(l_o, l_d, l_lo, l_hi);
  if ((((l_sp.y < l_sp.x) || (l_sp.y < 0.0f)) || (l_sp.x >= l_limit)))
  {
    return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  }
  l_c = coax_p_v3(((l_lo.x + l_hi.x) * 0x1.000000p-1f), l_lo.y, ((l_lo.z + l_hi.z) * 0x1.000000p-1f));
  l_along = ((l_hi.x - l_lo.x) >= (l_hi.z - l_lo.z));
  l_s = select(coax_p_v3(((l_hi.z - l_lo.z) * 0x1.000000p-1f), (l_hi.y - l_lo.y), ((l_hi.x - l_lo.x) * 0x1.000000p-1f)), coax_p_v3(((l_hi.x - l_lo.x) * 0x1.000000p-1f), (l_hi.y - l_lo.y), ((l_hi.z - l_lo.z) * 0x1.000000p-1f)), l_along);
  l_t = coax_p_fmax(l_sp.x, 0.0f);
  l_tend = coax_p_fmin(l_sp.y, l_limit);
  l_i = 0i;
  while (((l_i < 72i) && (l_t < l_tend)))
  {
    l_w = coax_p_sb_sub(coax_p_sb_add(l_o, coax_p_sb_mul(l_d, l_t)), l_c);
    l_q = select(coax_p_v3(l_w.z, l_w.y, l_w.x), l_w, l_along);
    l_dist = coax_p_slab_greek_sd(l_q, l_s, l_round);
    if ((l_dist < (0x1.47ae14p-6f + (l_t * 0x1.a36e2ep-12f))))
    {
      l_e = (0x1.eb851ep-6f + (l_t * 0x1.a36e2ep-13f));
      l_gx = (coax_p_slab_greek_sd(coax_p_sb_add(l_q, coax_p_v3(l_e, 0.0f, 0.0f)), l_s, l_round) - coax_p_slab_greek_sd(coax_p_sb_sub(l_q, coax_p_v3(l_e, 0.0f, 0.0f)), l_s, l_round));
      l_gy = (coax_p_slab_greek_sd(coax_p_sb_add(l_q, coax_p_v3(0.0f, l_e, 0.0f)), l_s, l_round) - coax_p_slab_greek_sd(coax_p_sb_sub(l_q, coax_p_v3(0.0f, l_e, 0.0f)), l_s, l_round));
      l_gz = (coax_p_slab_greek_sd(coax_p_sb_add(l_q, coax_p_v3(0.0f, 0.0f, l_e)), l_s, l_round) - coax_p_slab_greek_sd(coax_p_sb_sub(l_q, coax_p_v3(0.0f, 0.0f, l_e)), l_s, l_round));
      l_nn = coax_p_wg_normalize3(select(coax_p_v3(l_gz, l_gy, l_gx), coax_p_v3(l_gx, l_gy, l_gz), l_along));
      return coax_p_v4(l_t, l_nn.x, l_nn.y, l_nn.z);
    }
    l_t = (l_t + coax_p_fmax((l_dist * 0x1.ccccccp-1f), 0x1.47ae14p-6f));
    l_i = (l_i + 1i);
  }
  return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
}
fn coax_p_slab_statues_hit(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f, p_limit: f32) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_lo: vec3f = p_lo;
  let l_hi: vec3f = p_hi;
  let l_limit: f32 = p_limit;
  var l_best: vec4f;
  var l_sp: vec2f;
  var l_t0: f32;
  var l_t1: f32;
  var l_alongx: bool;
  var l_la: f32;
  var l_ha: f32;
  var l_a0: f32;
  var l_a1: f32;
  var l_count: f32;
  var l_i0: f32;
  var l_i1: f32;
  var l_s0: f32;
  var l_s1: f32;
  var l_base: f32;
  var l_i: f32;
  var l_j: i32;
  var l_c: f32;
  var l_side: i32;
  var l_w: f32;
  var l_px: f32;
  var l_pz: f32;
  var l_ped: vec4f;
  var l_fig: vec4f;
  l_best = coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  l_sp = coax_p_slab_span(l_o, l_d, l_lo, l_hi);
  if ((((l_sp.y < l_sp.x) || (l_sp.y < 0.0f)) || (l_sp.x >= l_limit)))
  {
    return l_best;
  }
  l_t0 = coax_p_fmax(l_sp.x, 0.0f);
  l_t1 = coax_p_fmin(l_sp.y, l_limit);
  l_alongx = ((l_hi.x - l_lo.x) >= (l_hi.z - l_lo.z));
  l_la = select(l_lo.z, l_lo.x, l_alongx);
  l_ha = select(l_hi.z, l_hi.x, l_alongx);
  l_a0 = select((l_o.z + (l_d.z * l_t0)), (l_o.x + (l_d.x * l_t0)), l_alongx);
  l_a1 = select((l_o.z + (l_d.z * l_t1)), (l_o.x + (l_d.x * l_t1)), l_alongx);
  l_count = floor(((l_ha - l_la) / 0x1.800000p3f));
  l_i0 = coax_p_fmax((floor(((coax_p_fmin(l_a0, l_a1) - l_la) / 0x1.800000p3f)) - 0x1.000000p0f), 0.0f);
  l_i1 = coax_p_fmin((floor(((coax_p_fmax(l_a0, l_a1) - l_la) / 0x1.800000p3f)) + 0x1.000000p0f), (l_count - 0x1.000000p0f));
  l_s0 = (select(l_lo.x, l_lo.z, l_alongx) + 0x1.800000p0f);
  l_s1 = (select(l_hi.x, l_hi.z, l_alongx) - 0x1.800000p0f);
  l_base = l_lo.y;
  l_i = l_i0;
  l_j = 0i;
  while (((l_i <= l_i1) && (l_j < 64i)))
  {
    l_c = (l_la + (0x1.800000p3f * (l_i + 0x1.000000p-1f)));
    l_side = 0i;
    while ((l_side < 2i))
    {
      l_w = select(l_s1, l_s0, (l_side == 0i));
      l_px = select(l_w, l_c, l_alongx);
      l_pz = select(l_c, l_w, l_alongx);
      l_ped = coax_p_slab_box_hit(l_o, l_d, coax_p_v3((l_px - 0x1.99999ap-1f), l_base, (l_pz - 0x1.99999ap-1f)), coax_p_v3((l_px + 0x1.99999ap-1f), (l_base + 0x1.ccccccp0f), (l_pz + 0x1.99999ap-1f)));
      if (((l_ped.x > 0.0f) && (l_ped.x < l_best.x)))
      {
        l_best = l_ped;
      }
      l_fig = coax_p_slab_drum_hit(l_o, l_d, coax_p_v3((l_px - 0x1.851eb8p-2f), (l_base + 0x1.ccccccp0f), (l_pz - 0x1.851eb8p-2f)), coax_p_v3((l_px + 0x1.851eb8p-2f), (l_base + 0x1.0cccccp2f), (l_pz + 0x1.851eb8p-2f)));
      if (((l_fig.x > 0.0f) && (l_fig.x < l_best.x)))
      {
        l_best = l_fig;
      }
      l_side = (l_side + 1i);
    }
    l_i = (l_i + 0x1.000000p0f);
    l_j = (l_j + 1i);
  }
  if ((l_best.x >= l_limit))
  {
    return coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  }
  return l_best;
}
fn coax_p_slab_silone_hit(p_o: vec3f, p_d: vec3f, p_lo: vec3f, p_hi: vec3f) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_lo: vec3f = p_lo;
  let l_hi: vec3f = p_hi;
  var l_cx: f32;
  var l_cz: f32;
  var l_big: f32;
  var l_yc: f32;
  var l_best: vec4f;
  var l_k: i32;
  var l_r: f32;
  var l_y0: f32;
  var l_y1: f32;
  var l_h: vec4f;
  l_cx = ((l_lo.x + l_hi.x) * 0x1.000000p-1f);
  l_cz = ((l_lo.z + l_hi.z) * 0x1.000000p-1f);
  l_big = ((l_hi.x - l_lo.x) * 0x1.000000p-1f);
  l_yc = (l_lo.y + 0x1.000000p4f);
  l_best = coax_p_slab_drum_hit(l_o, l_d, l_lo, coax_p_v3(l_hi.x, l_yc, l_hi.z));
  l_k = 0i;
  while ((l_k < 6i))
  {
    l_r = select(0x1.000000p2f, (l_big - (0x1.800000p2f * (f32(l_k) + 0x1.000000p0f))), (l_k < 5i));
    l_y0 = (l_yc + (0x1.800000p2f * f32(l_k)));
    l_y1 = select((l_y0 + 0x1.400000p3f), (l_y0 + 0x1.800000p2f), (l_k < 5i));
    l_h = coax_p_slab_drum_hit(l_o, l_d, coax_p_v3((l_cx - l_r), l_y0, (l_cz - l_r)), coax_p_v3((l_cx + l_r), l_y1, (l_cz + l_r)));
    if (((l_h.x > 0.0f) && (l_h.x < l_best.x)))
    {
      l_best = l_h;
    }
    l_k = (l_k + 1i);
  }
  return l_best;
}
fn coax_p_slab_obox_hit(p_o: vec3f, p_d: vec3f, p_a: vec3f, p_b: vec3f, p_hw: f32, p_hh: f32) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_hw: f32 = p_hw;
  let l_hh: f32 = p_hh;
  var l_miss: vec4f;
  var l_ax: vec3f;
  var l_hl: f32;
  var l_e1: vec3f;
  var l_wl: f32;
  var l_e3: vec3f;
  var l_e2: vec3f;
  var l_m: vec3f;
  var l_p: vec3f;
  var l_lo: vec3f;
  var l_ld: vec3f;
  var l_h: vec4f;
  var l_nw: vec3f;
  l_miss = coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  l_ax = coax_p_sb_add(l_b, coax_p_sb_mul(l_a, (-0x1.000000p0f)));
  l_hl = (coax_p_wg_length3(l_ax) * 0x1.000000p-1f);
  if ((l_hl < 0x1.0624dep-10f))
  {
    return l_miss;
  }
  l_e1 = coax_p_sb_mul(l_ax, (0x1.000000p-1f / l_hl));
  l_wl = coax_p_fmax(sqrt(((l_e1.x * l_e1.x) + (l_e1.z * l_e1.z))), 0x1.a36e2ep-14f);
  l_e3 = coax_p_v3(((-l_e1.z) / l_wl), 0.0f, (l_e1.x / l_wl));
  l_e2 = coax_p_v3(((l_e3.y * l_e1.z) - (l_e3.z * l_e1.y)), ((l_e3.z * l_e1.x) - (l_e3.x * l_e1.z)), ((l_e3.x * l_e1.y) - (l_e3.y * l_e1.x)));
  l_m = coax_p_sb_mul(coax_p_sb_add(l_a, l_b), 0x1.000000p-1f);
  l_p = coax_p_sb_add(l_o, coax_p_sb_mul(l_m, (-0x1.000000p0f)));
  l_lo = coax_p_v3(coax_p_wg_dot3(l_p, l_e1), coax_p_wg_dot3(l_p, l_e2), coax_p_wg_dot3(l_p, l_e3));
  l_ld = coax_p_v3(coax_p_wg_dot3(l_d, l_e1), coax_p_wg_dot3(l_d, l_e2), coax_p_wg_dot3(l_d, l_e3));
  l_h = coax_p_slab_box_hit(l_lo, l_ld, coax_p_v3((-l_hl), (-l_hh), (-l_hw)), coax_p_v3(l_hl, l_hh, l_hw));
  if ((l_h.x >= 0x1.dcd650p29f))
  {
    return l_miss;
  }
  l_nw = coax_p_sb_add(coax_p_sb_add(coax_p_sb_mul(l_e1, l_h.y), coax_p_sb_mul(l_e2, l_h.z)), coax_p_sb_mul(l_e3, l_h.w));
  return coax_p_v4(l_h.x, l_nw.x, l_nw.y, l_nw.z);
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
fn coax_p_slab_band(p_v: f32, p_period: f32, p_share: f32) -> f32 {
  let l_v: f32 = p_v;
  let l_period: f32 = p_period;
  let l_share: f32 = p_share;
  var l_f: f32;
  l_f = coax_p_fract((l_v / l_period));
  return coax_p_stepf(l_f, l_share);
}
fn coax_p_slab_section(p_k: i32, p_w: vec3f) -> coax_t21 {
  let l_k: i32 = p_k;
  let l_w: vec3f = p_w;
  var l_c: vec3f;
  var l_hl: f32;
  var l_hb: f32;
  var l_deck: f32;
  var l_keel: f32;
  var l_top: f32;
  var l_bx: f32;
  var l_seam: f32;
  var l_upper: bool;
  var l_concrete: bool;
  var l_hatch: f32;
  var l_a1: bool;
  l_c = coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-4f, 0x1.c28f5cp-4f);
  if ((l_k == 0i))
  {
    l_hl = coax_at_u(i32(36i));
    l_hb = coax_at_u(i32(37i));
    l_deck = coax_at_u(i32(38i));
    l_keel = coax_at_u(i32(39i));
    l_top = (l_keel + 0x1.900000p5f);
    l_bx = (abs((coax_p_fract((((l_w.x + l_hl) / 0x1.900000p5f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * 0x1.900000p5f);
    l_seam = (abs((coax_p_fract((((l_w.x + l_hl) / 0x1.900000p6f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * 0x1.900000p6f);
    l_upper = (l_w.y > l_top);
    l_concrete = (((((abs(l_w.x) > (l_hl - 0x1.800000p0f)) || (l_w.y < (l_keel + 0x1.800000p0f))) || (l_w.y > (l_top - 0x1.000000p0f))) || (l_bx < 0x1.333334p-2f)) || (l_seam < 0x1.333334p-1f));
    if (l_upper)
    {
      l_concrete = (((abs(l_w.x) > (l_hl - 0x1.99999ap-1f)) || (l_w.y > (l_deck - 0x1.333334p-1f))) || ((l_bx < 0x1.000000p0f) && (l_w.y > (l_deck - 0x1.ccccccp1f))));
    }
    if (l_concrete)
    {
      l_hatch = coax_p_slab_band((l_w.x + l_w.y), 0x1.000000p1f, 0x1.000000p-2f);
      l_c = coax_p_sb_mul(coax_p_v3(0x1.51eb86p-1f, 0x1.47ae14p-1f, 0x1.333334p-1f), (0x1.ccccccp-1f + (0x1.99999ap-4f * l_hatch)));
    }
    else
    {
      if (l_upper)
      {
        l_c = select(coax_p_v3(0x1.0a3d70p-3f, 0x1.1eb852p-3f, 0x1.47ae14p-3f), coax_p_v3(0x1.51eb86p-2f, 0x1.47ae14p-2f, 0x1.3d70a4p-2f), (l_bx < 0x1.000000p2f));
      }
      else
      {
        l_c = coax_p_v3(0x1.5c28f6p-3f, 0x1.5c28f6p-3f, 0x1.70a3d8p-3f);
        l_c = coax_p_sb_add(l_c, coax_p_sb_mul(coax_p_v3(0x1.99999ap-3f, 0x1.70a3d8p-3f, 0x1.1eb852p-3f), coax_p_slab_band((l_w.y - (l_top - 0x1.800000p2f)), 0x1.900000p5f, 0x1.47ae14p-5f)));
        l_a1 = ((l_w.x < ((-l_hl) + 0x1.900000p5f)) && (l_w.z < ((-l_hb) + 0x1.900000p5f)));
        if (((l_w.y < coax_at_u(i32(1494i))) && (!l_a1)))
        {
          l_c = coax_p_v3(0x1.eb851ep-5f, 0x1.0a3d70p-2f, 0x1.ae147ap-2f);
        }
        if ((l_a1 && (l_w.y < coax_at_u(i32(1495i)))))
        {
          l_c = coax_p_v3(0x1.666666p-1f, 0x1.70a3d8p-3f, 0x1.99999ap-4f);
        }
      }
    }
    if (((abs(l_w.y) < 0x1.666666p-2f) && (coax_p_slab_band(l_w.x, 0x1.000000p3f, 0x1.333334p-1f) > 0x1.000000p-1f)))
    {
      l_c = coax_p_v3(0x1.ccccccp-1f, 0x1.e66666p-1f, 0x1.000000p0f);
    }
  }
  else
  {
    if ((l_k != 21i))
    {
      l_c = coax_p_sb_add(coax_p_v3(0x1.47ae14p-3f, 0x1.333334p-3f, 0x1.1eb852p-3f), coax_p_sb_mul(coax_p_v3(0x1.000000p-1f, 0x1.eb851ep-2f, 0x1.c28f5cp-2f), coax_p_slab_band((l_w.y - coax_at_u(i32(38i))), 0x1.ccccccp1f, 0x1.99999ap-4f)));
    }
  }
  return coax_t21(coax_p_sb_mul(l_c, 0x1.000000p-2f), coax_p_sb_mul(l_c, 0x1.99999ap-1f));
}
fn coax_p_slab_spray(p_s: f32, p_y: f32, p_face: f32, p_t: f32) -> f32 {
  let l_s: f32 = p_s;
  let l_y: f32 = p_y;
  let l_face: f32 = p_face;
  let l_t: f32 = p_t;
  var l_top: f32;
  var l_spacing: f32;
  var l_ph: f32;
  var l_cell: f32;
  var l_h: f32;
  var l_local: f32;
  var l_beat: f32;
  var l_pulse: f32;
  var l_height: f32;
  var l_h2: f32;
  var l_width: f32;
  var l_streak: f32;
  var l_lx: f32;
  var l_shape: f32;
  var l_plume: f32;
  var l_foot: f32;
  l_top = (coax_at_u(i32(1484i)) * l_face);
  l_spacing = coax_at_u(i32(1485i));
  l_ph = ((l_s / l_spacing) - (l_t * (coax_at_u(i32(1489i)) / l_spacing)));
  l_cell = floor(l_ph);
  l_h = coax_p_slab_hash2(coax_p_v2(l_cell, 0x1.c00000p2f));
  l_local = ((coax_p_fract(l_ph) - 0x1.000000p-1f) * l_spacing);
  l_beat = coax_p_fract(((l_t / 0x1.200000p3f) + l_h));
  l_pulse = (coax_p_smoothstep(0.0f, 0x1.333334p-3f, l_beat) * (0x1.000000p0f - coax_p_smoothstep(0x1.19999ap-1f, 0x1.000000p0f, l_beat)));
  l_height = ((l_top * (0x1.800000p-1f + (0x1.000000p-2f * l_h))) * (0x1.000000p-1f + (0x1.000000p-1f * l_pulse)));
  l_h2 = coax_p_slab_hash2(coax_p_v2(l_cell, 0x1.300000p4f));
  l_width = ((l_spacing * (0x1.333334p-2f + (0x1.000000p-2f * l_h2))) * (0x1.666666p-1f + (0x1.333334p-2f * l_pulse)));
  l_streak = ((coax_p_slab_noise(coax_p_v2((l_s * 0x1.666666p-2f), ((l_y * 0x1.eb851ep-4f) - (l_t * 0x1.800000p0f)))) * 0x1.333334p-1f) + (coax_p_slab_noise(coax_p_v2((l_s * 0x1.19999ap0f), ((l_y * 0x1.99999ap-2f) - (l_t * 0x1.800000p1f)))) * 0x1.99999ap-2f));
  l_lx = coax_p_clampf((abs((l_local + ((l_y * 0x1.eb851ep-4f) * (l_h2 - 0x1.000000p-1f)))) / l_width), 0.0f, 0x1.000000p0f);
  l_shape = ((l_height * sqrt(coax_p_fmax((0x1.000000p0f - (l_lx * l_lx)), 0.0f))) * (0x1.99999ap-1f + (0x1.99999ap-2f * l_streak)));
  l_plume = (0x1.000000p0f - coax_p_smoothstep((l_shape * (0x1.b33334p-1f + (0x1.99999ap-3f * l_streak))), ((l_shape * (0x1.19999ap0f + (0x1.333334p-2f * l_streak))) + 0x1.000000p0f), l_y));
  l_plume = coax_p_fmax(l_plume, ((((0x1.000000p0f - coax_p_smoothstep(l_shape, ((l_shape * 0x1.99999ap0f) + 0x1.000000p1f), l_y)) * 0x1.99999ap-2f) * l_pulse) * l_streak));
  l_foot = (0x1.000000p0f - coax_p_smoothstep((0x1.333334p-1f + ((0x1.ccccccp0f * l_face) * l_streak)), (0x1.ccccccp0f + (0x1.800000p1f * l_face)), l_y));
  return coax_p_clampf(coax_p_fmax((l_plume * (0x1.666666p-2f + (0x1.4cccccp-1f * l_streak))), ((l_foot * l_face) * 0x1.ccccccp-1f)), 0.0f, 0x1.000000p0f);
}
fn coax_p_wg_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  let l_i: vec3f = p_i;
  let l_n: vec3f = p_n;
  return coax_p_reflect3(l_i, l_n);
}
fn coax_p_slab_columns(p_s: f32, p_period: f32) -> f32 {
  let l_s: f32 = p_s;
  let l_period: f32 = p_period;
  var l_f: f32;
  l_f = (abs((coax_p_fract((l_s / l_period)) - 0x1.000000p-1f)) * 0x1.000000p1f);
  return coax_p_smoothstep(0x1.ae147ap-2f, 0x1.19999ap-1f, l_f);
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
fn coax_p_wo_noise(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  var l_ix: f32;
  var l_iy: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_sx: f32;
  var l_sy: f32;
  l_ix = floor(l_p.x);
  l_iy = floor(l_p.y);
  l_fx = (l_p.x - l_ix);
  l_fy = (l_p.y - l_iy);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sy = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  return coax_p_mix(coax_p_mix(coax_p_render_noise2(l_ix, l_iy), coax_p_render_noise2((l_ix + 0x1.000000p0f), l_iy), l_sx), coax_p_mix(coax_p_render_noise2(l_ix, (l_iy + 0x1.000000p0f)), coax_p_render_noise2((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f)), l_sx), l_sy);
}
fn coax_p_wo_fresnel(p_c: f32) -> f32 {
  let l_c: f32 = p_c;
  var l_m: f32;
  var l_m2: f32;
  l_m = coax_p_clampf((0x1.000000p0f - l_c), 0.0f, 0x1.000000p0f);
  l_m2 = (l_m * l_m);
  return (0x1.48a6cep-6f + ((((0x1.000000p0f - 0x1.48a6cep-6f) * l_m2) * l_m2) * l_m));
}
fn coax_p_slab_sea_skylight() -> vec3f {
  return coax_p_slab_sea_lin(coax_p_sb_mul(coax_p_sb_add(coax_p_sb_p(8i), coax_p_sb_p(12i)), 0x1.000000p-1f));
}
fn coax_p_slab_sea_kd() -> vec3f {
  return coax_p_v3(coax_at_u(i32(15840i)), coax_at_u(i32(15841i)), coax_at_u(i32(15842i)));
}
fn coax_p_slab_sea_clear() -> f32 {
  return coax_p_fmax(coax_at_u(i32(15843i)), 0x1.000000p0f);
}
fn coax_p_wo_through(p_absorb: vec3f, p_path: f32) -> vec3f {
  let l_absorb: vec3f = p_absorb;
  let l_path: f32 = p_path;
  return coax_p_wo_exp3(coax_p_scale3(l_absorb, (-coax_p_fmax(l_path, 0.0f))));
}
fn coax_p_wo_cos_refracted(p_ci: f32) -> f32 {
  let l_ci: f32 = p_ci;
  return sqrt(coax_p_fmax((0x1.000000p0f - ((0x1.000000p0f - (l_ci * l_ci)) / (0x1.547ae2p0f * 0x1.547ae2p0f))), 0.0f));
}
fn coax_p_render_noise2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  var l_ok: bool;
  l_ok = (coax_p_render_noise_coordinate_ok(l_x) && coax_p_render_noise_coordinate_ok(l_y));
  return select(0.0f, coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y))), l_ok);
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
fn coax_p_wo_mean_fresnel(p_c: f32, p_sigma2: f32) -> f32 {
  let l_c: f32 = p_c;
  let l_sigma2: f32 = p_sigma2;
  var l_s: f32;
  l_s = sqrt(coax_p_fmax(l_sigma2, 0.0f));
  return (0x1.48a6cep-6f + (((0x1.000000p0f - 0x1.48a6cep-6f) * coax_pow_bg_vs(coax_p_clampf((0x1.000000p0f - l_c), 0.0f, 0x1.000000p0f), (0x1.400000p2f * exp(((-0x1.5851ecp1f) * l_s))))) / (0x1.000000p0f + (0x1.6b3334p4f * coax_pow_bg_vs(l_s, 0x1.800000p0f)))));
}
fn coax_p_wo_alpha(p_mss: f32) -> f32 {
  let l_mss: f32 = p_mss;
  return sqrt(sqrt(coax_p_clampf(l_mss, 0x1.4f8b58p-17f, 0x1.000000p0f)));
}
fn coax_p_wo_glint(p_n: vec3f, p_v: vec3f, p_l: vec3f, p_roughness: f32, p_disc: f32, p_far: f32) -> f32 {
  let l_n: vec3f = p_n;
  let l_v: vec3f = p_v;
  let l_l: vec3f = p_l;
  let l_roughness: f32 = p_roughness;
  let l_disc: f32 = p_disc;
  let l_far: f32 = p_far;
  var l_nol: f32;
  var l_nov: f32;
  var l_h: vec3f;
  var l_a: f32;
  var l_a2: f32;
  var l_spec: f32;
  l_nol = dot(l_n, l_l);
  l_nov = coax_p_fmax(dot(l_n, l_v), 0x1.0624dep-10f);
  if ((l_nol <= 0.0f))
  {
    return 0.0f;
  }
  l_h = coax_p_normalize3(coax_p_add3(l_l, l_v));
  l_a = coax_p_fmin((((l_roughness * l_roughness) + (l_disc * 0x1.000000p-1f)) + (l_far * 0x1.47ae14p-4f)), 0x1.000000p0f);
  l_a2 = (l_a * l_a);
  l_spec = ((coax_p_wo_ggx(coax_p_fmax(dot(l_n, l_h), 0.0f), l_a2) * coax_p_wo_smith(l_nov, l_nol, l_a2)) * coax_p_wo_fresnel(coax_p_fmax(dot(l_v, l_h), 0.0f)));
  return ((l_spec * l_nol) * (0x1.000000p0f - (0x1.800000p-1f * l_far)));
}
fn coax_p_slab_sea_foam1(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  var l_q: vec2f;
  var l_f: vec2f;
  var l_i: coax_t19;
  var l_acc: f32;
  var l_k: i32;
  var l_o: coax_t19;
  var l_c: coax_t20;
  var l_w: f32;
  var l_word: u32;
  l_q = coax_p_wg_sub_f2f(coax_p_wg_mul_f2f(coax_p_wg_div_f2f(l_p, coax_at_u(i32(30i))), f32(64u)), 0x1.000000p-1f);
  l_f = coax_p_wg_fract_f2(l_q);
  l_i = coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(l_q));
  l_acc = 0.0f;
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_o = coax_p_i2((l_k & 1i), (l_k >> u32(1i)));
      l_c = coax_p_wg_cvt_u2i2(coax_p_wg_band_i2i2(coax_p_wg_add_i2i2(l_i, l_o), coax_p_wg_make_i2_i((i32(64u) - 1i))));
      l_w = (select((0x1.000000p0f - l_f.x), l_f.x, (l_o.x == 1i)) * select((0x1.000000p0f - l_f.y), l_f.y, (l_o.y == 1i)));
      l_word = coax_p_slab_word((0u + (2u * ((l_c.y * 64u) + l_c.x))));
      l_acc = (l_acc + (l_w * (f32(((l_word >> u32(24u)) & 255u)) / 0x1.f80000p6f)));
      l_k = (l_k + 1i);
    }
  }
  return l_acc;
}
fn coax_p_slab_sea_ridge(p_v: f32, p_width: f32) -> f32 {
  let l_v: f32 = p_v;
  let l_width: f32 = p_width;
  return (0x1.000000p0f - coax_p_smoothstep(0.0f, l_width, abs((l_v - 0x1.000000p-1f))));
}
fn coax_p_slab_sea_foam_lit(p_ndl: f32) -> vec3f {
  let l_ndl: f32 = p_ndl;
  return coax_p_sb_mul(coax_p_sb_add(coax_p_sb_mul(coax_p_slab_sea_sun(), (0x1.333334p-2f + (0x1.666666p-1f * l_ndl))), coax_p_slab_sea_skylight()), 0x1.8f5c28p-1f);
}
fn coax_p_wg_atu(p_at: i32) -> u32 {
  let l_at: i32 = p_at;
  return bitcast<u32>(coax_at_u(i32(l_at)));
}
fn coax_p_u4(p_x: u32, p_y: u32, p_z: u32, p_w: u32) -> coax_t22 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  let l_z: u32 = p_z;
  let l_w: u32 = p_w;
  return coax_t22(l_x, l_y, l_z, l_w);
}
fn coax_p_slab_cypress(p_o: vec3f, p_d: vec3f, p_b: vec3f, p_r: f32, p_h: f32) -> vec4f {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_b: vec3f = p_b;
  let l_r: f32 = p_r;
  let l_h: f32 = p_h;
  var l_best: vec4f;
  var l_y0: f32;
  var l_y1: f32;
  var l_top: f32;
  var l_ox: f32;
  var l_oz: f32;
  var l_qa: f32;
  var l_qb: f32;
  var l_qc: f32;
  var l_disc: f32;
  var l_t0: f32;
  var l_y: f32;
  var l_k: f32;
  var l_oy: f32;
  var l_ca: f32;
  var l_cb: f32;
  var l_cc: f32;
  var l_cd: f32;
  var l_sq: f32;
  var l_ta: f32;
  var l_tb: f32;
  var l_t1: f32;
  var l_t2: f32;
  var l_j: i32;
  var l_tt: f32;
  var l_y_30: f32;
  var l_px: f32;
  var l_pz: f32;
  var l_rr: f32;
  var l_nn: vec3f;
  l_best = coax_p_v4(0x1.dcd650p29f, 0.0f, 0x1.000000p0f, 0.0f);
  l_y0 = (l_b.y + 0x1.99999ap-2f);
  l_y1 = (l_b.y + (l_h * 0x1.51eb86p-2f));
  l_top = (l_b.y + l_h);
  l_ox = (l_o.x - l_b.x);
  l_oz = (l_o.z - l_b.z);
  l_qa = ((l_d.x * l_d.x) + (l_d.z * l_d.z));
  l_qb = ((l_ox * l_d.x) + (l_oz * l_d.z));
  l_qc = (((l_ox * l_ox) + (l_oz * l_oz)) - (l_r * l_r));
  l_disc = ((l_qb * l_qb) - (l_qa * l_qc));
  if (((l_qa > 0x1.5798eep-27f) && (l_disc > 0.0f)))
  {
    l_t0 = (((-l_qb) - sqrt(l_disc)) / l_qa);
    l_y = (l_o.y + (l_d.y * l_t0));
    if ((((l_t0 > 0.0f) && (l_y >= l_y0)) && (l_y <= l_y1)))
    {
      l_best = coax_p_v4(l_t0, ((l_ox + (l_d.x * l_t0)) / l_r), 0.0f, ((l_oz + (l_d.z * l_t0)) / l_r));
    }
  }
  l_k = (l_r / (l_top - l_y1));
  l_oy = (l_top - l_o.y);
  l_ca = (l_qa - (((l_k * l_k) * l_d.y) * l_d.y));
  l_cb = ((0x1.000000p1f * l_qb) + ((((0x1.000000p1f * l_k) * l_k) * l_oy) * l_d.y));
  l_cc = (((l_ox * l_ox) + (l_oz * l_oz)) - (((l_k * l_k) * l_oy) * l_oy));
  l_cd = ((l_cb * l_cb) - ((0x1.000000p2f * l_ca) * l_cc));
  if (((abs(l_ca) > 0x1.5798eep-27f) && (l_cd > 0.0f)))
  {
    l_sq = sqrt(l_cd);
    l_ta = (((-l_cb) - l_sq) / (0x1.000000p1f * l_ca));
    l_tb = (((-l_cb) + l_sq) / (0x1.000000p1f * l_ca));
    l_t1 = coax_p_fmin(l_ta, l_tb);
    l_t2 = coax_p_fmax(l_ta, l_tb);
    l_j = 0i;
    while ((l_j < 2i))
    {
      l_tt = select(l_t2, l_t1, (l_j == 0i));
      l_y_30 = (l_o.y + (l_d.y * l_tt));
      if (((((l_tt > 0.0f) && (l_tt < l_best.x)) && (l_y_30 >= l_y1)) && (l_y_30 <= l_top)))
      {
        l_px = (l_ox + (l_d.x * l_tt));
        l_pz = (l_oz + (l_d.z * l_tt));
        l_rr = coax_p_fmax(sqrt(((l_px * l_px) + (l_pz * l_pz))), 0x1.a36e2ep-14f);
        l_nn = coax_p_wg_normalize3(coax_p_v3((l_px / l_rr), l_k, (l_pz / l_rr)));
        l_best = coax_p_v4(l_tt, l_nn.x, l_nn.y, l_nn.z);
        l_j = 2i;
      }
      l_j = (l_j + 1i);
    }
  }
  return l_best;
}
fn coax_p_slab_greek_sd(p_q: vec3f, p_s: vec3f, p_round: bool) -> f32 {
  let l_q: vec3f = p_q;
  let l_s: vec3f = p_s;
  let l_round: bool = p_round;
  if (l_round)
  {
    return coax_p_slab_tholos_sd(l_q, l_s);
  }
  return coax_p_slab_temple_sd(l_q, l_s);
}
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  let l_i: vec3f = p_i;
  let l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_i))));
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
fn coax_p_wo_exp3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_v3(exp(l_a.x), exp(l_a.y), exp(l_a.z));
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
fn coax_p_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wo_ggx(p_noh: f32, p_a2: f32) -> f32 {
  let l_noh: f32 = p_noh;
  let l_a2: f32 = p_a2;
  var l_d: f32;
  l_d = (((l_noh * l_noh) * (l_a2 - 0x1.000000p0f)) + 0x1.000000p0f);
  return (l_a2 / ((0x1.921fb6p1f * l_d) * l_d));
}
fn coax_p_wo_smith(p_nov: f32, p_nol: f32, p_a2: f32) -> f32 {
  let l_nov: f32 = p_nov;
  let l_nol: f32 = p_nol;
  let l_a2: f32 = p_a2;
  var l_gv: f32;
  var l_gl: f32;
  l_gv = (l_nol * sqrt((((l_nov * l_nov) * (0x1.000000p0f - l_a2)) + l_a2)));
  l_gl = (l_nov * sqrt((((l_nol * l_nol) * (0x1.000000p0f - l_a2)) + l_a2)));
  return (0x1.000000p-1f / coax_p_fmax((l_gv + l_gl), 0x1.0c6f7ap-20f));
}
fn coax_p_slab_tholos_sd(p_q: vec3f, p_s: vec3f) -> f32 {
  let l_q: vec3f = p_q;
  let l_s: vec3f = p_s;
  var l_hh: f32;
  var l_r: f32;
  var l_st: f32;
  var l_step: f32;
  var l_d: f32;
  var l_k: i32;
  var l_kf: f32;
  var l_r2: f32;
  var l_hc: f32;
  var l_rc: f32;
  var l_rr: f32;
  var l_n: f32;
  var l_sector: f32;
  var l_a: f32;
  var l_pr: f32;
  var l_px: f32;
  var l_pz: f32;
  var l_col: f32;
  var l_ye: f32;
  var l_he: f32;
  var l_yd: f32;
  var l_rd: f32;
  var l_dy: f32;
  l_hh = l_s.y;
  l_r = l_s.x;
  l_st = (l_hh * 0x1.47ae14p-4f);
  l_step = (l_r * 0x1.99999ap-5f);
  l_d = 0x1.dcd650p29f;
  l_k = 0i;
  while ((l_k < 3i))
  {
    l_kf = f32(l_k);
    l_d = coax_p_fmin(l_d, coax_p_slab_sd_ring(l_q, ((l_st * l_kf) / 0x1.800000p1f), ((l_st * (l_kf + 0x1.000000p0f)) / 0x1.800000p1f), (l_r - (l_kf * l_step))));
    l_k = (l_k + 1i);
  }
  l_r2 = (l_r - (0x1.800000p1f * l_step));
  l_hc = (l_hh * 0x1.ccccccp-2f);
  l_rc = coax_p_fmax((l_r2 * 0x1.1eb852p-4f), 0x1.666666p-2f);
  l_rr = (l_r2 - (l_rc * 0x1.99999ap0f));
  l_n = coax_p_fmax(floor(((0x1.921fb6p2f * l_rr) / (l_rc * 0x1.400000p2f))), 0x1.800000p2f);
  l_sector = (0x1.921fb6p2f / l_n);
  l_a = coax_p_wg_atan2(l_q.z, l_q.x);
  l_a = (l_a - (l_sector * coax_p_wg_round((l_a / l_sector))));
  l_pr = sqrt(((l_q.x * l_q.x) + (l_q.z * l_q.z)));
  l_px = ((l_pr * cos(l_a)) - l_rr);
  l_pz = (l_pr * sin(l_a));
  l_col = coax_p_fmax((sqrt(((l_px * l_px) + (l_pz * l_pz))) - l_rc), coax_p_fmax((l_st - l_q.y), (l_q.y - (l_st + l_hc))));
  l_d = coax_p_fmin(l_d, l_col);
  l_d = coax_p_fmin(l_d, coax_p_slab_sd_ring(l_q, l_st, (l_st + l_hc), (l_rr - (l_rc * 0x1.800000p1f))));
  l_ye = (l_st + l_hc);
  l_he = (l_hh * 0x1.eb851ep-4f);
  l_d = coax_p_fmin(l_d, coax_p_slab_sd_ring(l_q, l_ye, (l_ye + l_he), l_r2));
  l_yd = (l_ye + l_he);
  l_rd = coax_p_fmin((l_r2 * 0x1.d70a3ep-1f), (l_hh - l_yd));
  l_dy = (l_q.y - l_yd);
  l_d = coax_p_fmin(l_d, coax_p_fmax((sqrt((((l_q.x * l_q.x) + (l_dy * l_dy)) + (l_q.z * l_q.z))) - l_rd), (-l_dy)));
  return l_d;
}
fn coax_p_slab_temple_sd(p_q: vec3f, p_s: vec3f) -> f32 {
  let l_q: vec3f = p_q;
  let l_s: vec3f = p_s;
  var l_hh: f32;
  var l_st: f32;
  var l_step: f32;
  var l_d: f32;
  var l_k: i32;
  var l_kf: f32;
  var l_hl: f32;
  var l_hw: f32;
  var l_hc: f32;
  var l_rc: f32;
  var l_cl: f32;
  var l_cw: f32;
  var l_nl: f32;
  var l_sl: f32;
  var l_nw: f32;
  var l_sw: f32;
  var l_i: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_col: f32;
  var l_j: f32;
  var l_dz2: f32;
  var l_dx2: f32;
  var l_ye: f32;
  var l_he: f32;
  var l_yr: f32;
  var l_hr: f32;
  var l_b: f32;
  var l_ln: f32;
  var l_qy: f32;
  var l_tri: f32;
  l_hh = l_s.y;
  l_st = (l_hh * 0x1.70a3d8p-4f);
  l_step = (l_s.z * 0x1.eb851ep-6f);
  l_d = 0x1.dcd650p29f;
  l_k = 0i;
  while ((l_k < 3i))
  {
    l_kf = f32(l_k);
    l_d = coax_p_fmin(l_d, coax_p_slab_sd_box(l_q, coax_p_v3(0.0f, ((l_st * (l_kf + 0x1.000000p-1f)) / 0x1.800000p1f), 0.0f), coax_p_v3((l_s.x - (l_kf * l_step)), (l_st / 0x1.800000p2f), (l_s.z - (l_kf * l_step)))));
    l_k = (l_k + 1i);
  }
  l_hl = (l_s.x - (0x1.800000p1f * l_step));
  l_hw = (l_s.z - (0x1.800000p1f * l_step));
  l_hc = (l_hh * 0x1.1eb852p-1f);
  l_rc = coax_p_fmax((l_hw * 0x1.c28f5cp-5f), 0x1.ccccccp-2f);
  l_cl = (l_hl - (l_rc * 0x1.99999ap0f));
  l_cw = (l_hw - (l_rc * 0x1.99999ap0f));
  l_nl = coax_p_fmax(floor(((l_cl * 0x1.000000p1f) / (l_rc * 0x1.400000p2f))), 0x1.000000p1f);
  l_sl = ((l_cl * 0x1.000000p1f) / l_nl);
  l_nw = coax_p_fmax(floor(((l_cw * 0x1.000000p1f) / (l_rc * 0x1.400000p2f))), 0x1.000000p1f);
  l_sw = ((l_cw * 0x1.000000p1f) / l_nw);
  l_i = coax_p_clampf(coax_p_wg_round(((l_q.x + l_cl) / l_sl)), 0.0f, l_nl);
  l_dx = (l_q.x - ((-l_cl) + (l_i * l_sl)));
  l_dz = (abs(l_q.z) - l_cw);
  l_col = (sqrt(((l_dx * l_dx) + (l_dz * l_dz))) - l_rc);
  l_j = coax_p_clampf(coax_p_wg_round(((l_q.z + l_cw) / l_sw)), 0.0f, l_nw);
  l_dz2 = (l_q.z - ((-l_cw) + (l_j * l_sw)));
  l_dx2 = (abs(l_q.x) - l_cl);
  l_col = coax_p_fmin(l_col, (sqrt(((l_dx2 * l_dx2) + (l_dz2 * l_dz2))) - l_rc));
  l_col = coax_p_fmax(l_col, coax_p_fmax((l_st - l_q.y), (l_q.y - (l_st + l_hc))));
  l_d = coax_p_fmin(l_d, l_col);
  l_d = coax_p_fmin(l_d, coax_p_slab_sd_box(l_q, coax_p_v3(0.0f, (l_st + (l_hc * 0x1.000000p-1f)), 0.0f), coax_p_v3((l_cl - (l_rc * 0x1.c00000p1f)), (l_hc * 0x1.000000p-1f), (l_cw - (l_rc * 0x1.c00000p1f)))));
  l_ye = (l_st + l_hc);
  l_he = (l_hh * 0x1.0a3d70p-3f);
  l_d = coax_p_fmin(l_d, coax_p_slab_sd_box(l_q, coax_p_v3(0.0f, (l_ye + (l_he * 0x1.000000p-1f)), 0.0f), coax_p_v3(l_hl, (l_he * 0x1.000000p-1f), l_hw)));
  l_yr = (l_ye + l_he);
  l_hr = coax_p_fmax((l_hh - l_yr), 0x1.000000p-1f);
  l_b = (l_hw + 0x1.99999ap-2f);
  l_ln = sqrt(((l_hr * l_hr) + (l_b * l_b)));
  l_qy = (l_q.y - l_yr);
  l_tri = coax_p_fmax(((((abs(l_q.z) * l_hr) + (l_qy * l_b)) - (l_b * l_hr)) / l_ln), (-l_qy));
  l_d = coax_p_fmin(l_d, coax_p_fmax(l_tri, (abs(l_q.x) - (l_hl + 0x1.99999ap-2f))));
  return l_d;
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  let l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
}
fn coax_p_slab_sd_ring(p_p: vec3f, p_y0: f32, p_y1: f32, p_r: f32) -> f32 {
  let l_p: vec3f = p_p;
  let l_y0: f32 = p_y0;
  let l_y1: f32 = p_y1;
  let l_r: f32 = p_r;
  return coax_p_fmax((sqrt(((l_p.x * l_p.x) + (l_p.z * l_p.z))) - l_r), coax_p_fmax((l_y0 - l_p.y), (l_p.y - l_y1)));
}
fn coax_p_slab_sd_box(p_p: vec3f, p_c: vec3f, p_h: vec3f) -> f32 {
  let l_p: vec3f = p_p;
  let l_c: vec3f = p_c;
  let l_h: vec3f = p_h;
  var l_qx: f32;
  var l_qy: f32;
  var l_qz: f32;
  var l_ox: f32;
  var l_oy: f32;
  var l_oz: f32;
  l_qx = (abs((l_p.x - l_c.x)) - l_h.x);
  l_qy = (abs((l_p.y - l_c.y)) - l_h.y);
  l_qz = (abs((l_p.z - l_c.z)) - l_h.z);
  l_ox = coax_p_fmax(l_qx, 0.0f);
  l_oy = coax_p_fmax(l_qy, 0.0f);
  l_oz = coax_p_fmax(l_qz, 0.0f);
  return (sqrt((((l_ox * l_ox) + (l_oy * l_oy)) + (l_oz * l_oz))) + coax_p_fmin(coax_p_fmax(l_qx, coax_p_fmax(l_qy, l_qz)), 0.0f));
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
  return coax_io_14(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
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
  let coax_r = coax_p_fx_fs(coax_t14(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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