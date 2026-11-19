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
struct coax_t15 {
  x: vec4f,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
}
struct coax_t16 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec3f,
  f4: vec3f,
  f5: vec3f,
  f6: vec4f,
}
struct coax_io_16 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
  @location(3) f4: vec3f,
  @location(4) f5: vec3f,
  @location(5) @interpolate(flat) f6: vec4f,
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
struct coax_t19 {
  x: f32,
  y: vec3f,
  z: f32,
  w: f32,
  f4: f32,
  f5: vec3f,
  f6: vec3f,
}
struct coax_t20 {
  x: vec4f,
  y: vec4f,
  z: f32,
}
struct coax_out_20 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @builtin(frag_depth) z: f32,
}
struct coax_t21 {
  x: vec4f,
  y: vec4f,
}
struct coax_out_21 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
struct coax_t24 {
  x: vec3f,
  y: vec3f,
}
struct coax_t26 {
  x: vec3f,
  y: f32,
  z: f32,
  w: vec3f,
  f4: vec3f,
  f5: f32,
}
struct coax_t29 {
  x: i32,
  y: i32,
}
struct coax_t30 {
  x: u32,
  y: u32,
}
struct coax_t33 {
  x: u32,
  y: u32,
  z: u32,
  w: u32,
}
struct coax_t34 {
  x: vec3f,
  y: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 4063>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> parts: array<f32>;
@group(1) @binding(1) var<storage, read> objects: array<coax_t13>;
@group(0) @binding(1) var<storage, read> frame: array<f32>;
@group(1) @binding(2) var<storage, read> materials: array<f32>;
@group(1) @binding(3) var<storage, read> field: array<f32>;
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
@group(1) @binding(0) var bloom_src: texture_2d<f32>;
@group(1) @binding(1) var bloom_lin: sampler;
@group(1) @binding(2) var<uniform> bloom_step: array<vec4f, 1>;
fn coax_at_bloom_step(i: i32) -> f32 {
  return bloom_step[i / 4][i % 4];
}
@group(1) @binding(0) var post_light: texture_2d<f32>;
@group(1) @binding(1) var post_quarter: texture_2d<f32>;
@group(1) @binding(2) var post_eighth: texture_2d<f32>;
@group(1) @binding(3) var post_aux: texture_2d<f32>;
@group(1) @binding(4) var post_depth: texture_depth_2d;
@group(1) @binding(5) var post_lin: sampler;
@group(0) @binding(2) var sans_atlas: texture_2d<f32>;
@group(0) @binding(3) var sans_sampler: sampler;
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
fn coax_p_shadow_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  return coax_p_pr_shadow_stand(l_id, l_instance, 15872i);
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t16 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  return coax_p_pr_stand(l_id, l_instance, 15872i);
}
fn coax_p_fx_vs(p__vid3: u32, p__iid4: u32, p_in_: coax_t17) -> coax_t18 {
  let l__vid3: u32 = p__vid3;
  let l__iid4: u32 = p__iid4;
  let l_in_: coax_t17 = p_in_;
  var l_a: vec4f;
  var l_o: coax_t18;
  l_a = l_in_.x;
  l_o = coax_t18();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_in_.y;
  l_o.w = l_in_.z;
  l_o.f4 = l_in_.w;
  return l_o;
}
fn coax_p_bg_fs(p_frag: vec4f) -> coax_t20 {
  let l_frag: vec4f = p_frag;
  var l_ndc: vec2f;
  var l_aspect: f32;
  var l_d: vec3f;
  var l_o: vec3f;
  var l_ho: vec3f;
  var l_hd: vec3f;
  var l_hit: coax_t19;
  var l_ch: coax_t19;
  var l_wet: bool;
  var l_ln: coax_t19;
  var l_under: f32;
  var l_c: vec3f;
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
  l_hit = coax_p_slab_photo_first(l_ho, l_hd);
  l_ch = coax_p_slab_walker(l_ho, l_hd, l_hit.x);
  if ((l_ch.x < l_hit.x))
  {
    l_hit = l_ch;
  }
  l_wet = ((coax_at_u(i32(1496i)) > 0x1.000000p-1f) || (coax_at_u(i32(1492i)) != 0.0f));
  if ((coax_at_u(i32(1496i)) > 0x1.000000p-1f))
  {
    l_ln = coax_p_slab_lines(l_o, l_d, l_hit.x);
    if ((l_ln.x < l_hit.x))
    {
      l_hit = l_ln;
    }
  }
  if ((l_hit.x < 0x1.dcd650p29f))
  {
    l_under = select(0.0f, coax_p_slab_under_len(l_ho, l_hd, l_hit.x), l_wet);
    l_c = select(coax_p_slab_lit(l_hit, l_ho, l_hd), coax_p_slab_photo_light(l_hit, l_ho, l_hd), (coax_p_slab_photo() && (l_under > 0.0f)));
    l_wy = (l_ho.y + (l_hd.y * l_hit.x));
    if ((((coax_at_u(i32(1492i)) != 0.0f) && (abs(l_hit.y.z) > 0x1.ccccccp-1f)) && (abs(((l_ho.z + (l_hd.z * l_hit.x)) - coax_at_u(i32(1493i)))) < 0x1.333334p-1f)))
    {
      l_under = (l_under * 0x1.eb851ep-4f);
      l_wy = 0.0f;
    }
    if ((l_under > 0.0f))
    {
      l_c = coax_p_slab_through_water(l_c, l_under, l_wy, coax_p_fmin(l_ho.y, 0.0f), l_hd.y);
      if ((l_ho.y < 0.0f))
      {
        l_c = coax_p_sw_under(l_c, l_ho, l_hd, l_hit.x);
      }
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
    return coax_t20(coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f), coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, (-0x1.000000p0f)), coax_p_slab_depth_of(l_d, l_hit.x));
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
    l_c_19 = select(coax_p_slab_sea_open(l_o, l_d), coax_p_slab_deep_colour(coax_p_fmin(l_ho.y, 0.0f), l_hd.y), l_slot);
    if ((l_ho.y < 0.0f))
    {
      l_c_19 = coax_p_sw_under(l_c_19, l_ho, l_hd, 0x1.f40000p11f);
    }
    l_c_19 = coax_p_slab_grade(l_c_19);
    return coax_t20(coax_p_v4(l_c_19.x, l_c_19.y, l_c_19.z, 0x1.000000p0f), coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, (-0x1.000000p0f)), 0x1.000000p0f);
  }
  l_c_20 = coax_p_slab_land(coax_p_slab_haze(coax_p_slab_weather_sky(coax_p_slab_sky(l_d), l_d), l_d, (0x1.388000p15f * (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.99999ap-3f, l_d.y)))), l_d, l_o);
  l_sm_21 = coax_p_slab_smoke(l_ho, l_hd, 0x1.dcd650p29f);
  l_c_20 = coax_p_sb_mix(l_c_20, coax_p_v3(l_sm_21.x, l_sm_21.y, l_sm_21.z), l_sm_21.w);
  l_c_20 = coax_p_slab_grade(coax_p_slab_weather_air(l_c_20, coax_p_v2(l_frag.x, l_frag.y)));
  return coax_t20(coax_p_v4(l_c_20.x, l_c_20.y, l_c_20.z, 0x1.000000p0f), coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, (-0x1.000000p0f)), 0x1.000000p0f);
}
fn coax_p_surface_fs(p_v: coax_t12) -> coax_t21 {
  let l_v: coax_t12 = p_v;
  var l_rel: vec3f;
  var l_dist: f32;
  var l_d: vec3f;
  var l_sec: f32;
  var l_hp: vec3f;
  var l_beyond: f32;
  var l_c: vec3f;
  var l_c_10: vec3f;
  var l_sm: vec4f;
  l_rel = coax_p_wg_sub_f3f3(l_v.z, coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_dist = coax_p_wg_length3(l_rel);
  l_d = coax_p_wg_div_f3f(l_rel, coax_p_fmax(l_dist, 0x1.a36e2ep-14f));
  if (coax_p_slab_dry_cell(l_v.z))
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
    l_c = coax_p_slab_sea_below(l_v.y, l_v.z, l_d, l_dist, coax_p_sw_sea(l_v.y));
    l_c = coax_p_slab_grade(coax_p_sw_under(l_c, coax_p_sb_to_hull(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i)))), coax_p_sb_dir_to_hull(l_d), l_dist));
    return coax_p_pr_lit_plain(coax_p_wg_make_f4_f3f(l_c, 0x1.000000p0f));
  }
  l_c_10 = coax_p_slab_shore(coax_p_slab_haze(coax_p_slab_water(l_v.y, l_v.z, l_d, l_dist, coax_at_u(i32(2i))), l_d, (l_dist * coax_p_slab_photo_sea_haze())), l_v.z, l_d);
  l_sm = coax_p_slab_smoke(coax_p_sb_to_hull(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i)))), coax_p_sb_dir_to_hull(l_d), l_dist);
  l_c_10 = coax_p_slab_grade(coax_p_slab_weather_air(coax_p_sb_mix(l_c_10, coax_p_v3(l_sm.x, l_sm.y, l_sm.z), l_sm.w), coax_p_slab_weather_px(l_d)));
  return coax_p_pr_lit_plain(coax_p_wg_make_f4_f3f(l_c_10, 0x1.000000p0f));
}
fn coax_p_shadow_fs(p_s: coax_t15) {
  let l_s: coax_t15 = p_s;
}
fn coax_p_scene_fs(p_v: coax_t16) -> coax_t21 {
  let l_v: coax_t16 = p_v;
  var l_b: i32;
  var l_full: bool;
  var l_eye: vec3f;
  var l_to: vec3f;
  var l_dist: f32;
  var l_rd: vec3f;
  var l_toward: vec3f;
  var l_free: bool;
  var l_wh: vec3f;
  var l_gh: vec3f;
  var l_dh: vec3f;
  var l_aa: f32;
  var l_record: i32;
  var l_exposure: f32;
  var l_kind: i32;
  var l_row: f32;
  var l_painted: bool;
  var l_tinted: bool;
  var l_pt: coax_t24;
  var l_wall: bool;
  var l_deck: bool;
  var l_street: vec4f;
  var l_vv: coax_t16;
  var l_at: i32;
  var l_p: vec4f;
  var l_q: vec4f;
  var l_hit: coax_t19;
  var l_s: coax_t26;
  var l_nh: vec3f;
  var l_along: f32;
  var l_cell: vec2f;
  var l_on: f32;
  var l_pool: vec3f;
  var l_rain: f32;
  var l_low: f32;
  var l_gutter: f32;
  var l_hs: f32;
  var l_over: f32;
  var l_along_44: f32;
  var l_run: f32;
  var l_wet: f32;
  var l_damp: f32;
  var l_wash: vec2f;
  var l_n: vec3f;
  var l_sun: vec3f;
  var l_reach: f32;
  var l_outside: f32;
  var l_far: coax_t19;
  var l_fe: vec3f;
  var l_me: coax_t19;
  var l_open: f32;
  var l_mirrored: vec3f;
  var l_r: vec3f;
  var l_lamp: vec3f;
  var l_c: vec3f;
  var l_sm: vec4f;
  var l_pulse: f32;
  l_b = 15872i;
  l_full = (coax_at_u(i32(((l_b + 24i) + 3i))) < 0x1.000000p-1f);
  l_eye = coax_p_pr_at3((l_b + 0i));
  l_to = coax_p_sub3(l_v.y, l_eye);
  l_dist = coax_p_length3(l_to);
  l_rd = coax_p_scale3(l_to, (0x1.000000p0f / coax_p_fmax(l_dist, 0x1.a36e2ep-14f)));
  l_toward = coax_p_scale3(l_rd, (0.0f - 0x1.000000p0f));
  l_free = (l_v.f6.w > 0x1.000000p-1f);
  l_wh = l_v.y;
  l_gh = coax_p_normalize3(l_v.w);
  l_dh = l_rd;
  if ((!l_free))
  {
    l_wh = coax_p_sb_to_hull(l_v.y);
    l_gh = coax_p_sb_dir_to_hull(l_gh);
    l_dh = coax_p_sb_dir_to_hull(l_rd);
  }
  l_aa = coax_p_fmax(fwidth(l_wh.x), fwidth(l_wh.z));
  l_record = (i32((l_v.f6.y + 0x1.000000p-1f)) - 1i);
  l_exposure = coax_p_fmax(coax_at_u(i32(((l_b + 20i) + 3i))), 0x1.0624dep-10f);
  l_kind = (-1i);
  l_row = l_v.f6.x;
  l_painted = (l_row > (0x1.f80000p5f - 0x1.000000p-1f));
  l_tinted = ((l_row > (0x1.c00000p3f - 0x1.000000p-1f)) && (l_row < (0x1.700000p4f + 0x1.000000p-1f)));
  l_pt = coax_t24(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f));
  l_wall = false;
  l_deck = false;
  l_street = vec4f(l_row, 0.0f, 0.0f, 0x1.900000p6f);
  l_vv = l_v;
  if ((l_record >= 0i))
  {
    l_at = (96i + (l_record * 8i));
    l_p = coax_p_wg_at4(l_at);
    l_q = coax_p_wg_at4((l_at + 4i));
    l_kind = coax_p_wg_i32_f((l_p.w + 0x1.000000p-1f));
    if ((l_painted || l_tinted))
    {
      l_hit = coax_t19(l_dist, l_gh, (l_p.w + select(0.0f, 0x1.000000p-2f, (l_q.w > 0x1.500000p3f))), l_q.y, f32(l_record), coax_p_v3(l_p.x, l_p.y, l_p.z), coax_p_v3(l_q.x, l_q.y, l_q.z));
      l_pt = coax_p_slab_paint(l_hit, l_wh, l_dh);
    }
    if (((l_kind == 0i) && (abs((l_row - 0x1.c00000p2f)) < 0x1.000000p-1f)))
    {
      if ((l_gh.y > 0x1.000000p-1f))
      {
        l_deck = true;
        l_street = coax_p_slab_deck_at(coax_p_v2(l_wh.x, l_wh.z));
        l_vv.f6.x = l_street.x;
      }
      else
      {
        if ((abs(l_gh.y) < 0x1.000000p-1f))
        {
          l_wall = true;
        }
      }
    }
  }
  if (l_tinted)
  {
    l_vv.f5 = coax_p_mul3(l_vv.f5, coax_p_pr_decode(l_pt.x));
  }
  l_s = coax_p_pr_surface(l_vv, l_b);
  l_nh = l_s.w;
  if ((!l_free))
  {
    l_nh = coax_p_sb_dir_to_hull(l_s.w);
  }
  if (l_tinted)
  {
    l_s.f4 = coax_p_add3(l_s.f4, coax_p_scale3(coax_p_pr_decode(l_pt.y), (0x1.000000p0f / l_exposure)));
  }
  if ((l_painted && (l_record >= 0i)))
  {
    l_s.x = coax_p_mul3(coax_p_pr_decode(l_pt.x), l_v.f5);
    l_s.f4 = coax_p_scale3(coax_p_pr_decode(l_pt.y), (0x1.000000p0f / l_exposure));
    if ((((((l_kind == 2i) || (l_kind == 3i)) || (l_kind == 11i)) || (l_kind == 20i)) && (abs(l_nh.y) < 0x1.000000p-1f)))
    {
      l_along = select(l_wh.x, l_wh.z, (abs(l_nh.x) > abs(l_nh.z)));
      l_cell = coax_p_v2(floor((l_along / 0x1.000000p2f)), (floor(((l_wh.y - coax_at_u(i32(38i))) / 0x1.ccccccp1f)) + (f32(l_record) * 0x1.c00000p2f)));
      l_on = coax_p_stepf(coax_p_slab_hash2(l_cell), coax_at_u(i32((l_b + 79i))));
      l_s.f4 = coax_p_scale3(l_s.f4, ((l_on * coax_at_u(i32((l_b + 112i)))) * 0x1.19999ap-1f));
    }
    l_s.y = coax_p_slab_photo_rough(l_kind, (l_nh.y > 0x1.000000p-1f));
    l_s.z = 0.0f;
    if (((l_kind == 14i) && (l_nh.y > 0x1.000000p-1f)))
    {
      l_s.x = coax_p_v3(0x1.3d70a4p-1f, 0x1.333334p-1f, 0x1.19999ap-1f);
      l_s.y = 0x1.000000p-1f;
      l_s.f4 = coax_p_v3(0.0f, 0.0f, 0.0f);
      if ((abs((l_wh.x + 0x1.850000p8f)) < 0x1.a00000p2f))
      {
        l_pool = coax_p_sw_pool(coax_p_v3(0.0f, 0.0f, 0.0f), l_wh, l_dh);
        l_s.x = coax_p_v3(0.0f, 0.0f, 0.0f);
        l_s.y = 0x1.000000p0f;
        l_s.f4 = coax_p_mul3(coax_p_pr_decode(l_pool), coax_p_pr_sky_light(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_b));
      }
    }
  }
  l_rain = coax_p_saturate((coax_at_u(i32(15857i)) * 0x1.99999ap0f));
  if (l_deck)
  {
    l_s = coax_p_slab_deck_street(l_s, l_street, l_aa);
    if ((l_rain > 0.0f))
    {
      l_low = coax_p_smoothstep(0x1.333334p-1f, 0x1.666666p-1f, coax_p_pr_cloud2((l_wh.x * 0x1.0a3d70p-3f), (l_wh.z * 0x1.0a3d70p-3f)));
      l_gutter = select(0.0f, coax_p_pr_band(abs(l_street.y), 0x1.300000p2f, 0x1.000000p-1f, l_aa, 0.0f), (l_street.x < (0x1.200000p3f + 0x1.000000p-1f)));
      l_s = coax_p_pr_wet(l_s, l_rain, coax_p_fmax(l_low, (l_gutter * 0x1.99999ap-1f)), coax_p_normalize3(l_v.w));
    }
  }
  if ((l_wall && (l_v.y.y < (coax_at_u(i32(28i)) + 0x1.000000p4f))))
  {
    l_hs = coax_at_u(i32(35i));
    l_over = (l_v.y.y - coax_p_slab_sea_height(coax_p_v2(l_v.y.x, l_v.y.z)));
    l_along_44 = select(l_wh.x, l_wh.z, (abs(l_gh.x) > abs(l_gh.z)));
    l_run = (0x1.99999ap-2f + (l_hs * (0x1.000000p-1f + (0x1.99999ap-1f * coax_p_pr_noise2((l_along_44 * 0x1.99999ap-5f), (coax_at_u(i32(((l_b + 12i) + 3i))) * 0x1.333334p-3f))))));
    l_wet = (0x1.000000p0f - coax_p_smoothstep(0.0f, l_run, l_over));
    l_damp = (0x1.000000p-1f * (0x1.000000p0f - coax_p_smoothstep((l_hs * 0x1.333334p-1f), ((l_hs * 0x1.666666p0f) + 0x1.000000p0f), (l_v.y.y - coax_at_u(i32(28i))))));
    l_s = coax_p_pr_wet(l_s, coax_p_fmax(l_wet, l_damp), 0.0f, l_s.w);
  }
  if (l_wall)
  {
    l_wash = coax_p_sw_wash(l_v.y, l_s.w, coax_at_u(i32(28i)));
    l_s = coax_p_pr_wet(l_s, l_wash.y, 0.0f, l_s.w);
    l_s.x = coax_p_mix3(l_s.x, coax_p_v3(0x1.99999ap-1f, 0x1.a3d70ap-1f, 0x1.ae147ap-1f), l_wash.x);
  }
  l_n = l_s.w;
  l_sun = coax_p_pr_at3((l_b + 16i));
  l_reach = 0.0f;
  if ((((dot(l_n, l_sun) > 0.0f) || (l_s.f5 > 0.0f)) && (l_sun.y > (-0x1.47ae14p-6f))))
  {
    l_outside = 0x1.000000p0f;
    if (((l_full && (!l_free)) && (coax_p_pr_shadow_edge(l_v.y, l_b) > 0x1.ccccccp-1f)))
    {
      l_far = coax_p_slab_scene(coax_p_sb_add(l_wh, coax_p_sb_mul(l_nh, 0x1.99999ap-4f)), coax_p_sb_dir_to_hull(l_sun), true, 0x1.c20000p9f);
      if ((l_far.x < 0x1.c20000p9f))
      {
        l_outside = 0.0f;
      }
    }
    l_reach = coax_p_pr_shadow(l_v.y, l_n, l_outside, l_b);
    if (((l_reach > 0.0f) && (coax_at_u(i32(1474i)) > 0x1.000000p-1f)))
    {
      l_fe = coax_p_sb_sub(l_wh, coax_p_sb_p(28i));
      if ((((l_fe.x * l_fe.x) + (l_fe.z * l_fe.z)) < 0x1.c20000p9f))
      {
        l_me = coax_p_slab_walker(coax_p_sb_add(l_wh, coax_p_sb_mul(l_nh, 0x1.99999ap-5f)), coax_p_sb_dir_to_hull(l_sun), 0x1.400000p5f);
        if ((l_me.x < 0x1.400000p5f))
        {
          l_reach = 0.0f;
        }
      }
    }
  }
  l_open = select(coax_p_pr_field(l_wh, l_nh, l_b), 0x1.000000p0f, l_free);
  l_mirrored = coax_p_pr_environment(l_n, l_toward, l_s.y, l_b);
  if (((l_full && (!l_free)) && (l_s.y < 0x1.47ae14p-3f)))
  {
    l_r = coax_p_reflect3(l_rd, l_n);
    l_mirrored = coax_p_slab_photo_mirror(coax_p_sb_add(l_wh, coax_p_sb_mul(l_nh, 0x1.333334p-3f)), coax_p_sb_dir_to_hull(l_r), l_mirrored);
  }
  l_lamp = select(coax_p_pr_lamps(l_wh, l_nh, coax_p_sb_dir_to_hull(l_toward), l_s, l_b), coax_p_v3(0.0f, 0.0f, 0.0f), l_free);
  l_c = coax_p_pr_out(coax_p_pr_air(coax_p_add3(coax_p_pr_lit(l_s, l_toward, l_reach, l_open, l_mirrored, l_b), l_lamp), l_v.y, l_b), l_b);
  l_sm = coax_p_slab_smoke(coax_p_sb_to_hull(l_eye), coax_p_sb_dir_to_hull(l_rd), l_dist);
  l_c = coax_p_mix3(l_c, coax_p_v3(l_sm.x, l_sm.y, l_sm.z), l_sm.w);
  l_c = coax_p_slab_weather_air(l_c, coax_p_v2(l_v.x.x, l_v.x.y));
  if ((l_record >= 0i))
  {
    if ((abs((f32(l_record) - coax_at_u(i32(1476i)))) < 0x1.000000p-1f))
    {
      l_pulse = (0x1.000000p-1f + (0x1.000000p-1f * sin((coax_at_u(i32(1478i)) * 0x1.400000p2f))));
      l_c = coax_p_add3(coax_p_scale3(l_c, 0x1.19999ap0f), coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.3d70a4p-1f, 0x1.99999ap-3f), (0x1.eb851ep-4f + (0x1.eb851ep-4f * l_pulse))));
    }
    else
    {
      if ((abs((f32(l_record) - coax_at_u(i32(1477i)))) < 0x1.000000p-1f))
      {
        l_c = coax_p_add3(coax_p_scale3(l_c, 0x1.0f5c28p0f), coax_p_v3(0x1.eb851ep-5f, 0x1.99999ap-5f, 0x1.eb851ep-6f));
      }
    }
  }
  return coax_p_pr_lit_out(l_c, l_n, coax_p_pr_sun_share(l_n, l_reach, l_open, l_b));
}
fn coax_p_bloom_fs(p_pos: vec4f) -> vec4f {
  let l_pos: vec4f = p_pos;
  return textureLoad(bloom_src, vec2i(i32(l_pos.x), i32(l_pos.y)), 0i);
}
fn coax_p_post_fs(p_pos: vec4f) -> vec4f {
  let l_pos: vec4f = p_pos;
  var l_x: i32;
  var l_y: i32;
  var l_light: vec4f;
  var l_c: vec3f;
  l_x = i32(l_pos.x);
  l_y = i32(l_pos.y);
  l_light = textureLoad(post_light, vec2i(l_x, l_y), 0i);
  if ((coax_at_u(i32(15990i)) < 0x1.000000p-1f))
  {
    return coax_p_v4(l_light.x, l_light.y, l_light.z, 0x1.000000p0f);
  }
  l_c = coax_p_pr_post(l_light, textureLoad(post_aux, vec2i(l_x, l_y), 0i), l_x, l_y, 15872i);
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t18) -> vec4f {
  let l_v: coax_t18 = p_v;
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
  var l_i: coax_t29;
  var l_acc: vec2f;
  var l_k: i32;
  var l_o: coax_t29;
  var l_c: coax_t30;
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
  return coax_p_wg_mul_f2f(l_acc, (coax_at_u(i32(43i)) * coax_p_sw_calm_at(l_p)));
}
fn coax_p_slab_sea_height(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  return (coax_at_u(i32(28i)) + (coax_p_sw_sea(l_p)).x);
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
fn coax_p_pr_shadow_stand(p_id: u32, p_instance: u32, p_b: i32) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  let l_b: i32 = p_b;
  var l_m: coax_t13;
  var l_o: i32;
  var l_w: vec3f;
  var l_s: coax_t15;
  l_m = objects[i32(l_instance)];
  l_o = (i32(l_id) * 12i);
  l_w = coax_p_pr_placed_in(coax_p_v3(parts[l_o], parts[(l_o + 1i)], parts[(l_o + 2i)]), l_m, coax_at_u(i32(((l_b + 12i) + 3i))), coax_p_wg_at4((l_b + 104i)));
  if ((l_m.y.w < 0x1.000000p-1f))
  {
    l_w = coax_p_pr_posed(l_w, l_b);
  }
  l_w = coax_p_pr_kept(l_w, l_m, l_b);
  l_s = coax_t15();
  l_s.x = coax_p_pr_light_clip(l_w, l_b);
  if (((l_m.y.y > 0x1.200000p2f) || (parts[(l_o + 7i)] <= (0.0f - 0x1.000000p3f))))
  {
    l_s.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
  }
  return l_s;
}
fn coax_p_pr_stand(p_id: u32, p_instance: u32, p_b: i32) -> coax_t16 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  let l_b: i32 = p_b;
  var l_m: coax_t13;
  var l_o: i32;
  var l_p: vec3f;
  var l_s: vec3f;
  var l_n: vec3f;
  var l_c: f32;
  var l_sn: f32;
  var l_time: f32;
  var l_wind: vec4f;
  var l_eye: vec3f;
  var l_rides: bool;
  var l_gone: bool;
  var l_dim: f32;
  var l_level: f32;
  var l_at: vec3f;
  var l_grow: f32;
  var l_w: vec3f;
  var l_ln: vec3f;
  var l_wn: vec3f;
  var l_from: f32;
  var l_seen: vec3f;
  var l_fold: f32;
  var l_own: f32;
  var l_flutter: f32;
  var l_v: coax_t16;
  var l_tint: vec3f;
  l_m = objects[i32(l_instance)];
  l_o = (i32(l_id) * 12i);
  l_p = coax_p_v3(parts[l_o], parts[(l_o + 1i)], parts[(l_o + 2i)]);
  l_s = coax_p_pr_sizes(l_m);
  l_n = coax_p_normalize3(coax_p_v3((parts[(l_o + 3i)] / coax_p_fmax(l_s.x, 0x1.0c6f7ap-20f)), (parts[(l_o + 4i)] / coax_p_fmax(l_s.y, 0x1.0c6f7ap-20f)), (parts[(l_o + 5i)] / coax_p_fmax(l_s.z, 0x1.0c6f7ap-20f))));
  l_c = cos(l_m.x.w);
  l_sn = sin(l_m.x.w);
  l_time = coax_at_u(i32(((l_b + 12i) + 3i)));
  l_wind = coax_p_wg_at4((l_b + 104i));
  l_eye = coax_p_pr_at3((l_b + 0i));
  l_rides = (l_m.y.w < 0x1.000000p-1f);
  l_gone = false;
  l_dim = 0x1.000000p0f;
  if ((l_m.y.y > 0x1.200000p2f))
  {
    l_level = coax_at_u(i32((l_b + 112i)));
    l_at = coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z);
    if (l_rides)
    {
      l_at = coax_p_pr_posed(l_at, l_b);
    }
    l_grow = coax_p_pr_lamp_grow(coax_p_length3(coax_p_sub3(l_at, l_eye)), (0x1.000000p-1f * l_s.x), coax_at_u(i32(((l_b + 0i) + 3i))));
    l_p = coax_p_scale3(l_p, l_grow);
    l_dim = (l_level * coax_p_fmax((0x1.000000p0f / (l_grow * l_grow)), 0x1.0624dep-8f));
    l_gone = ((l_level < 0x1.47ae14p-6f) || ((l_m.y.y > 0x1.600000p2f) && (sin(((l_time * 0x1.0cccccp1f) + l_m.y.z)) < (0.0f - 0x1.333334p-2f))));
  }
  l_w = coax_p_pr_placed_in(l_p, l_m, l_time, l_wind);
  l_ln = coax_p_pr_leant(l_n, l_m);
  l_wn = coax_p_v3(((l_ln.x * l_c) + (l_ln.z * l_sn)), l_ln.y, (((0.0f - l_ln.x) * l_sn) + (l_ln.z * l_c)));
  l_from = (0.0f - parts[(l_o + 7i)]);
  if ((l_from >= 0x1.000000p3f))
  {
    l_seen = l_w;
    if (l_rides)
    {
      l_seen = coax_p_pr_posed(l_w, l_b);
    }
    l_fold = coax_p_pr_fold(coax_p_length3(coax_p_sub3(l_seen, l_eye)), l_from, coax_at_u(i32(((l_b + 0i) + 3i))));
    if ((coax_at_u(i32(((l_b + 24i) + 3i))) > 0x1.000000p-1f))
    {
      l_fold = 0x1.000000p0f;
    }
    l_gone = (l_fold >= 0x1.000000p0f);
    l_own = coax_p_fract((sin((((l_p.x * 0x1.9fac72p3f) + (l_p.y * 0x1.38ee98p6f)) + (l_p.z * 0x1.2dc084p5f))) * 0x1.55dd18p15f));
    l_flutter = ((coax_p_fmin((0x1.89374cp-7f + (0x1.89374cp-8f * l_wind.z)), 0x1.70a3d8p-4f) * sin(((l_time * (0x1.000000p2f + (0x1.800000p1f * l_own))) + (l_own * 0x1.921fb6p2f)))) * (0x1.000000p0f - l_fold));
    l_w = coax_p_add3(l_w, coax_p_scale3(l_wn, (l_flutter - ((parts[(l_o + 8i)] * l_fold) * l_s.x))));
  }
  if (l_rides)
  {
    l_w = coax_p_pr_posed(l_w, l_b);
    l_wn = coax_p_pr_posed_dir(l_wn, l_b);
  }
  l_w = coax_p_pr_kept(l_w, l_m, l_b);
  l_v = coax_t16();
  l_v.y = l_w;
  l_v.z = coax_p_v3((l_p.x * l_s.x), (l_p.y * l_s.y), (l_p.z * l_s.z));
  l_v.w = l_wn;
  l_v.f4 = l_n;
  l_tint = coax_p_v3(parts[(l_o + 9i)], parts[(l_o + 10i)], parts[(l_o + 11i)]);
  if ((((l_tint.x == 0.0f) && (l_tint.y == 0.0f)) && (l_tint.z == 0.0f)))
  {
    l_tint = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  }
  l_v.f5 = coax_p_scale3(coax_p_mul3(l_tint, coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z)), l_dim);
  l_v.f6 = coax_p_v4(select(parts[(l_o + 6i)], l_m.w.w, (l_m.w.w > 0x1.000000p-1f)), l_m.z.w, select(l_m.y.z, 0.0f, ((l_m.y.y > 0x1.c00000p1f) && (l_m.y.y < 0x1.200000p2f))), l_m.y.w);
  l_v.x = coax_p_pr_clip(l_w, l_b);
  if (((l_m.y.y > 0x1.400000p1f) && (l_m.y.y < 0x1.200000p2f)))
  {
    l_v.x = coax_p_v4(l_v.x.x, l_v.x.y, (l_v.x.z - (0x1.421f60p-21f * l_v.x.w)), l_v.x.w);
  }
  if (l_gone)
  {
    l_v.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
  }
  return l_v;
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
fn coax_p_slab_photo_first(p_o: vec3f, p_d: vec3f) -> coax_t19 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  if (coax_p_slab_photo_meshes())
  {
    return coax_t19(0x1.dcd650p29f, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (-0x1.000000p0f), 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
  }
  return coax_p_slab_scene(l_o, l_d, false, 0x1.dcd650p29f);
}
fn coax_p_slab_walker(p_o: vec3f, p_d: vec3f, p_limit: f32) -> coax_t19 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_limit: f32 = p_limit;
  var l_hit: coax_t19;
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
  l_hit = coax_t19(l_limit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (-0x1.000000p0f), 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
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
      l_hit = coax_t19(l_h.x, coax_p_v3(l_h.y, l_h.z, l_h.w), (0x1.400000p5f + l_part), 0.0f, (-0x1.000000p0f), l_feet, l_feet);
    }
    l_k = (l_k + 1i);
  }
  return l_hit;
}
fn coax_p_slab_lines(p_o: vec3f, p_d: vec3f, p_limit: f32) -> coax_t19 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_limit: f32 = p_limit;
  var l_hit: coax_t19;
  var l_bed: f32;
  var l_fy: f32;
  var l_i: i32;
  var l_b: i32;
  var l_code: f32;
  var l_q: f32;
  var l_r: f32;
  var l_lost: bool;
  var l_half: bool;
  var l_sag: f32;
  var l_share: f32;
  var l_a: vec3f;
  var l_f: vec3f;
  var l_p1: vec3f;
  var l_p2: vec3f;
  var l_p3: vec3f;
  var l_rad: f32;
  var l_j: i32;
  var l_ca: vec3f;
  var l_cb: vec3f;
  var l_h: vec4f;
  l_hit = coax_t19(l_limit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (-0x1.000000p0f), 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
  l_bed = coax_at_u(i32(16212i));
  l_fy = coax_at_u(i32(16213i));
  l_i = 0i;
  while ((l_i < 4i))
  {
    l_b = (16216i + (l_i * 5i));
    l_code = coax_at_u(i32((l_b + 4i)));
    if ((l_code >= 0.0f))
    {
      l_q = floor((l_code / 0x1.000000p11f));
      l_r = (l_code - (l_q * 0x1.000000p11f));
      l_lost = (l_r > 0x1.ffa000p10f);
      l_half = ((l_r > 0x1.ffc000p9f) && (!l_lost));
      l_sag = select(((-select(l_r, (l_r - 0x1.000000p10f), l_half)) * 0x1.c71c72p-1f), 0.0f, l_lost);
      l_share = select((l_q / 0x1.f40000p9f), 0x1.000000p-1f, l_lost);
      l_a = coax_p_v3(coax_at_u(i32(l_b)), l_bed, coax_at_u(i32((l_b + 1i))));
      l_f = coax_p_v3(coax_at_u(i32((l_b + 2i))), select(l_fy, l_bed, l_lost), coax_at_u(i32((l_b + 3i))));
      l_p1 = coax_p_v3(coax_p_mix(l_a.x, l_f.x, l_share), l_bed, coax_p_mix(l_a.z, l_f.z, l_share));
      l_p2 = coax_p_sb_add(coax_p_sb_mix(l_p1, l_f, 0x1.555556p-2f), coax_p_v3(0.0f, l_sag, 0.0f));
      l_p3 = coax_p_sb_add(coax_p_sb_mix(l_p1, l_f, 0x1.555556p-1f), coax_p_v3(0.0f, l_sag, 0.0f));
      l_rad = select(0x1.333334p-1f, 0x1.333334p-2f, l_half);
      l_j = 0i;
      while ((l_j < 4i))
      {
        l_ca = select(select(select(l_p3, l_p2, (l_j == 2i)), l_p1, (l_j == 1i)), l_a, (l_j == 0i));
        l_cb = select(select(select(l_f, l_p3, (l_j == 2i)), l_p2, (l_j == 1i)), l_p1, (l_j == 0i));
        l_h = coax_p_slab_capsule(l_o, l_d, l_ca, l_cb, l_rad);
        if ((l_h.x < l_hit.x))
        {
          l_hit = coax_t19(l_h.x, coax_p_v3(l_h.y, l_h.z, l_h.w), 0x1.680000p5f, 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
        }
        l_j = (l_j + 1i);
      }
    }
    l_i = (l_i + 1i);
  }
  if ((l_hit.z > 0x1.600000p5f))
  {
    l_hit = coax_t19(l_hit.x, coax_p_sb_dir_to_hull(l_hit.y), 0x1.680000p5f, 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
  }
  return l_hit;
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
fn coax_p_slab_photo() -> bool {
  return (coax_at_u(i32(15988i)) > 0x1.000000p-1f);
}
fn coax_p_slab_photo_light(p_hit: coax_t19, p_o: vec3f, p_d: vec3f) -> vec3f {
  let l_hit: coax_t19 = p_hit;
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  var l_b: i32;
  var l_w: vec3f;
  var l_pt: coax_t24;
  var l_nw: vec3f;
  var l_sun: vec3f;
  var l_ndl: f32;
  var l_s: coax_t19;
  var l_occ: f32;
  var l_light: vec3f;
  l_b = 15872i;
  l_w = coax_p_sb_add(l_o, coax_p_sb_mul(l_d, l_hit.x));
  l_pt = coax_p_slab_paint(l_hit, l_w, l_d);
  l_nw = coax_p_sb_dir_to_world(l_hit.y);
  l_sun = coax_p_pr_at3((l_b + 16i));
  l_ndl = coax_p_fmax(dot(l_nw, l_sun), 0.0f);
  if (((((l_ndl > 0.0f) && (coax_at_u(i32(5i)) < 0x1.000000p-1f)) && (l_hit.z < 0x1.380000p5f)) && (l_sun.y > (-0x1.47ae14p-6f))))
  {
    l_s = coax_p_slab_scene(coax_p_sb_add(l_w, coax_p_sb_mul(l_hit.y, 0x1.47ae14p-4f)), coax_p_sb_dir_to_hull(l_sun), true, 0x1.c20000p9f);
    if ((l_s.x < 0x1.c20000p9f))
    {
      l_ndl = 0.0f;
    }
  }
  l_occ = select(0x1.000000p0f, (0x1.70a3d8p-1f + (0x1.1eb852p-2f * coax_p_smoothstep(0.0f, 0x1.800000p2f, (l_w.y - coax_at_u(i32(38i)))))), ((abs(l_hit.y.y) < 0x1.000000p-1f) && (l_hit.z >= 0x1.000000p-1f)));
  l_light = coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_b + 20i)), l_ndl), coax_p_scale3(coax_p_pr_sky_light(l_nw, l_b), l_occ));
  return coax_p_add3(coax_p_mul3(coax_p_pr_decode(l_pt.x), l_light), coax_p_scale3(coax_p_pr_decode(l_pt.y), (0x1.000000p0f / coax_p_fmax(coax_at_u(i32(((l_b + 20i) + 3i))), 0x1.0624dep-10f))));
}
fn coax_p_slab_lit(p_hit: coax_t19, p_o: vec3f, p_d: vec3f) -> vec3f {
  let l_hit: coax_t19 = p_hit;
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  var l_w: vec3f;
  var l_n: vec3f;
  var l_sun: vec3f;
  var l_scol: vec3f;
  var l_amb: f32;
  var l_pt: coax_t24;
  var l_base: vec3f;
  var l_ndl: f32;
  var l_shade: f32;
  var l_s: coax_t19;
  var l_ch: coax_t19;
  var l_skyc: vec3f;
  var l_occ: f32;
  var l_light: vec3f;
  var l_c: vec3f;
  var l_pulse: f32;
  if (coax_p_slab_photo())
  {
    return coax_p_slab_photo_lit(l_hit, l_o, l_d);
  }
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
fn coax_p_sw_under(p_c: vec3f, p_o: vec3f, p_d: vec3f, p_limit: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_limit: f32 = p_limit;
  var l_out: vec3f;
  var l_f: vec2f;
  var l_light: f32;
  var l_k: i32;
  var l_oz: f32;
  var l_carry: f32;
  var l_seg: i32;
  var l_a: vec3f;
  var l_run: f32;
  var l_dir: vec3f;
  var l_w0: vec3f;
  var l_b: f32;
  var l_dd: f32;
  var l_e: f32;
  var l_den: f32;
  var l_s: f32;
  var l_pa: vec3f;
  var l_tt: f32;
  var l_gone: f32;
  var l_r: f32;
  var l_dist: f32;
  var l_billow: f32;
  var l_dens: f32;
  var l_a1: f32;
  var l_t0: f32;
  var l_t1: f32;
  var l_lo: f32;
  var l_hi: f32;
  var l_mid: vec3f;
  var l_wk: vec2f;
  var l_a2: f32;
  if (((coax_at_u(i32(15992i)) < 0x1.000000p-1f) || (coax_at_u(i32(15992i)) > 0x1.800000p0f)))
  {
    return l_c;
  }
  l_out = l_c;
  l_f = coax_p_sw_turn(coax_p_v2(coax_at_u(i32(16016i)), coax_at_u(i32(16017i))));
  l_light = (0x1.666666p-2f + (0x1.4cccccp-1f * coax_at_u(i32(1447i))));
  if ((coax_at_u(i32(16020i)) > 0x1.0624dep-10f))
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_oz = ((-0x1.c20000p7f) + (0x1.2c0000p7f * f32(l_k)));
      l_carry = coax_p_fmax(coax_at_u(i32(16023i)), 0x1.000000p0f);
      l_seg = 0i;
      while ((l_seg < 2i))
      {
        l_a = coax_p_v3((-0x1.c20000p8f), (-coax_at_u(i32(16021i))), l_oz);
        l_run = l_carry;
        l_dir = coax_p_wg_normalize3(coax_p_v3((l_f.x * l_carry), (-coax_at_u(i32(16022i))), (l_f.y * l_carry)));
        if ((l_seg == 1i))
        {
          l_a = coax_p_v3(((-0x1.c20000p8f) + (l_f.x * l_carry)), (-(coax_at_u(i32(16021i)) + coax_at_u(i32(16022i)))), (l_oz + (l_f.y * l_carry)));
          l_dir = coax_p_v3(l_f.x, 0.0f, l_f.y);
          l_run = 0x1.c20000p9f;
        }
        else
        {
          l_run = sqrt(((l_carry * l_carry) + (coax_at_u(i32(16022i)) * coax_at_u(i32(16022i)))));
        }
        l_w0 = coax_p_sb_sub(l_o, l_a);
        l_b = coax_p_wg_dot3(l_d, l_dir);
        l_dd = coax_p_wg_dot3(l_d, l_w0);
        l_e = coax_p_wg_dot3(l_dir, l_w0);
        l_den = coax_p_fmax((0x1.000000p0f - (l_b * l_b)), 0x1.4f8b58p-17f);
        l_s = coax_p_clampf(((l_e - (l_b * l_dd)) / l_den), 0.0f, l_run);
        l_pa = coax_p_sb_add(l_a, coax_p_sb_mul(l_dir, l_s));
        l_tt = coax_p_wg_dot3(coax_p_sb_sub(l_pa, l_o), l_d);
        if (((l_tt > 0.0f) && (l_tt < l_limit)))
        {
          l_gone = (l_s + select(0.0f, l_carry, (l_seg == 1i)));
          l_r = (0x1.000000p3f + (0x1.99999ap-4f * l_gone));
          l_dist = coax_p_wg_length3(coax_p_sb_sub(coax_p_sb_add(l_o, coax_p_sb_mul(l_d, l_tt)), l_pa));
          l_billow = (0x1.333334p-1f + (0x1.99999ap-2f * coax_p_slab_noise(coax_p_v2(((l_gone * 0x1.99999ap-5f) - (coax_at_u(i32(2i)) * 0x1.333334p-3f)), ((f32(l_k) * 0x1.466666p2f) + (l_dist * 0x1.1eb852p-4f))))));
          l_dens = ((((exp(((-((l_dist / l_r) * (l_dist / l_r))) * 0x1.99999ap0f)) * (0x1.e00000p3f / (0x1.e00000p3f + (0x1.99999ap-4f * l_gone)))) * exp(((-l_gone) / 0x1.5e0000p9f))) * l_billow) * coax_at_u(i32(16020i)));
          l_a1 = (((0x1.000000p0f - exp(((-l_dens) * 0x1.19999ap1f))) * 0x1.99999ap-1f) * exp(((-l_tt) / 0x1.040000p8f)));
          l_out = coax_p_sb_mix(l_out, coax_p_sb_mul(coax_p_v3(0x1.ae147ap-2f, 0x1.7ae148p-1f, 0x1.8f5c28p-1f), (l_light * exp((coax_p_fmin(l_pa.y, 0.0f) / 0x1.400000p7f)))), l_a1);
        }
        l_seg = (l_seg + 1i);
      }
      l_k = (l_k + 1i);
    }
  }
  if ((abs(l_d.y) > 0x1.47ae14p-7f))
  {
    l_t0 = ((-l_o.y) / l_d.y);
    l_t1 = (((-0x1.000000p3f) - l_o.y) / l_d.y);
    l_lo = coax_p_fmax(coax_p_fmin(l_t0, l_t1), 0.0f);
    l_hi = coax_p_fmin(coax_p_fmax(l_t0, l_t1), l_limit);
    if ((l_hi > l_lo))
    {
      l_mid = coax_p_sb_add(l_o, coax_p_sb_mul(l_d, (0x1.000000p-1f * (l_lo + l_hi))));
      l_wk = coax_p_sw_wake(coax_p_v2(l_mid.x, l_mid.z));
      l_a2 = (((0x1.000000p0f - exp((((-(l_hi - l_lo)) * 0x1.47ae14p-6f) * (l_wk.x + (0x1.800000p0f * l_wk.y))))) * 0x1.000000p-1f) * exp(((-l_lo) / 0x1.900000p7f)));
      l_out = coax_p_sb_mix(l_out, coax_p_sb_mul(coax_p_v3(0x1.19999ap-1f, 0x1.8f5c28p-1f, 0x1.99999ap-1f), l_light), l_a2);
    }
  }
  return l_out;
}
fn coax_p_slab_haze(p_c: vec3f, p_d: vec3f, p_dist: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  var l_a: f32;
  var l_sun: vec3f;
  var l_tint: vec3f;
  if (coax_p_slab_photo())
  {
    return coax_p_slab_photo_haze(l_c, l_d, l_dist);
  }
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
  if (coax_p_slab_photo())
  {
    return l_c0;
  }
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
  return coax_p_slab_sea_under_out(coax_p_slab_sea_fog(l_y, l_up), l_y);
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
  var l_sunlit: f32;
  var l_shadow: coax_t19;
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
    l_sunlit = 0x1.000000p0f;
    if ((coax_at_u(i32(25i)) > 0x1.47ae14p-4f))
    {
      l_shadow = coax_p_slab_scene(coax_p_sb_to_hull(l_hit), coax_p_sb_dir_to_hull(coax_p_slab_sea_sunray()), false, 0x1.dcd650p29f);
      if ((l_shadow.x < 0x1.dcd650p29f))
      {
        l_sunlit = 0x1.333334p-2f;
      }
    }
    else
    {
      l_sunlit = 0x1.333334p-2f;
    }
    l_net = (coax_p_wo_caustic(coax_p_v2((l_hit.x * 0x1.70a3d8p-4f), (l_hit.z * 0x1.70a3d8p-4f)), (coax_at_u(i32(2i)) * 0x1.333334p-1f)) * select(0.0f, (0x1.666666p0f / (0x1.000000p0f + (l_bed / 0x1.900000p4f))), (l_sunlit > 0x1.000000p-1f)));
    l_sand = coax_p_sb_mul(coax_p_v3(0x1.d70a3ep-2f, 0x1.a3d70ap-2f, 0x1.3d70a4p-2f), ((0x1.800000p-1f + (0x1.99999ap-2f * l_grain)) * (0x1.000000p0f + l_net)));
    l_day = coax_p_sb_add(coax_p_sb_mul(coax_p_slab_sea_daylight(l_bed), l_sunlit), coax_p_sb_mul(coax_p_slab_sea_daylight(0.0f), coax_at_u(i32(15864i))));
    l_tr = coax_p_slab_sea_beam(l_reach);
    l_fog = coax_p_sb_add(coax_p_sb_mulv(coax_p_sb_mulv(l_sand, l_day), l_tr), coax_p_sb_mulv(l_fog, coax_p_v3((0x1.000000p0f - l_tr.x), (0x1.000000p0f - l_tr.y), (0x1.000000p0f - l_tr.z))));
  }
  l_shafts = coax_p_slab_sea_shafts(l_o, l_d, 0x1.2c0000p9f);
  return coax_p_slab_sea_under_out(coax_p_sb_mul(l_fog, (0x1.000000p0f + (0x1.99999ap0f * l_shafts))), l_eye);
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
  if (coax_p_slab_photo())
  {
    return coax_p_slab_photo_sky(l_d);
  }
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
fn coax_p_slab_land(p_c: vec3f, p_d: vec3f, p_o: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  let l_o: vec3f = p_o;
  var l_peak: f32;
  var l_far: f32;
  var l_flat: f32;
  var l_toward: f32;
  var l_within: f32;
  var l_reach: f32;
  var l_foot: f32;
  var l_ax: f32;
  var l_az: f32;
  var l_rise: f32;
  var l_ridge: f32;
  var l_el: f32;
  var l_m: f32;
  var l_land: vec3f;
  l_peak = coax_at_u(i32(1498i));
  l_far = coax_p_wg_length2(coax_p_v2(coax_at_u(i32(1490i)), coax_at_u(i32(1491i))));
  l_flat = coax_p_wg_length2(coax_p_v2(l_d.x, l_d.z));
  if (((((l_peak <= 0.0f) || (l_far < 0x1.000000p0f)) || (l_flat < 0x1.0624dep-10f)) || (l_d.y > 0x1.333334p-1f)))
  {
    return l_c;
  }
  l_toward = (((l_d.x * coax_at_u(i32(1490i))) + (l_d.z * coax_at_u(i32(1491i)))) / (l_flat * l_far));
  l_within = coax_p_slab_land_within(l_toward);
  l_reach = (coax_p_fmax((l_far - (((l_o.x * coax_at_u(i32(1490i))) + (l_o.z * coax_at_u(i32(1491i)))) / l_far)), 0x1.000000p0f) / coax_p_fmax(l_toward, 0x1.a36e2ep-14f));
  l_foot = select((-0x1.0624dep-10f), (-(coax_p_fmax(l_o.y, 0.0f) / l_reach)), (l_toward > 0x1.47ae14p-6f));
  l_ax = (l_d.x / l_flat);
  l_az = (l_d.z / l_flat);
  l_rise = ((((0x1.eb851ep-4f + (0x1.851eb8p-2f * coax_p_slab_noise(coax_p_v2((l_ax * 0x1.000000p2f), (l_az * 0x1.000000p2f))))) + (0x1.47ae14p-2f * coax_p_slab_noise(coax_p_v2(((l_ax * 0x1.100000p4f) + 0x1.800000p1f), (l_az * 0x1.100000p4f))))) + (0x1.1eb852p-3f * coax_p_slab_noise(coax_p_v2(((l_ax * 0x1.e80000p5f) + 0x1.c00000p2f), (l_az * 0x1.e80000p5f))))) + (0x1.47ae14p-5f * coax_p_slab_noise(coax_p_v2(((l_ax * 0x1.e00000p7f) + 0x1.600000p3f), (l_az * 0x1.e00000p7f)))));
  l_rise = ((l_rise * l_rise) * 0x1.99999ap0f);
  l_ridge = ((l_peak * l_within) * l_rise);
  l_el = (l_d.y / l_flat);
  l_m = ((0x1.000000p0f - coax_p_smoothstep((l_ridge - (0x1.3a92a4p-10f + (l_ridge * 0x1.47ae14p-5f))), l_ridge, l_el)) * coax_p_smoothstep((l_foot - 0x1.3a92a4p-11f), l_foot, l_el));
  l_m = (l_m * coax_p_smoothstep(0.0f, 0x1.0624dep-9f, l_ridge));
  l_land = coax_p_slab_land_far(coax_p_slab_haze(coax_p_slab_land_colour(), l_d, (l_far * 0x1.333334p-1f)), l_c, l_far, (0x1.000000p0f - coax_p_fmin(coax_p_fmax(((l_el - l_foot) / coax_p_fmax((l_ridge - l_foot), 0x1.a36e2ep-14f)), 0.0f), 0x1.000000p0f)));
  return coax_p_sb_mix(l_c, l_land, l_m);
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
fn coax_p_slab_dry_cell(p_w: vec3f) -> bool {
  let l_w: vec3f = p_w;
  var l_dm: f32;
  var l_turn: f32;
  var l_rx: f32;
  var l_rz: f32;
  var l_along: f32;
  var l_across: f32;
  var l_kf: f32;
  var l_bits: f32;
  l_dm = coax_at_u(i32(1501i));
  if ((l_dm < 0x1.000000p-1f))
  {
    return false;
  }
  l_turn = ((coax_p_fract(l_dm) / 0x1.ff7ceep-1f) * 0x1.921fb6p2f);
  l_rx = (l_w.x - coax_at_u(i32(1502i)));
  l_rz = (l_w.z - coax_at_u(i32(1503i)));
  l_along = ((l_rx * cos(l_turn)) + (l_rz * sin(l_turn)));
  l_across = ((l_rz * cos(l_turn)) - (l_rx * sin(l_turn)));
  l_kf = floor(((l_along / 0x1.2c0000p7f) + 0x1.000000p-1f));
  if (((((l_kf < 0.0f) || (l_kf > 0x1.e00000p2f)) || (abs((l_along - (l_kf * 0x1.2c0000p7f))) > 0x1.a00000p5f)) || (abs(l_across) > 0x1.b00000p4f)))
  {
    return false;
  }
  l_bits = floor((floor(l_dm) / coax_pow_bg_vs(0x1.000000p1f, l_kf)));
  return (coax_p_fract((l_bits * 0x1.000000p-1f)) > 0x1.000000p-2f);
}
fn coax_p_sw_sea(p_p: vec2f) -> vec4f {
  let l_p: vec2f = p_p;
  var l_c: vec4f;
  var l_q: vec2f;
  var l_g: f32;
  var l_o: vec4f;
  var l_hx: f32;
  var l_hz: f32;
  var l_k: i32;
  var l_amp: f32;
  var l_n: vec2f;
  var l_half: f32;
  var l_span: f32;
  var l_out: f32;
  var l_along: f32;
  var l_spread: f32;
  var l_from: f32;
  var l_edge: f32;
  var l_beam: f32;
  var l_r: vec4f;
  var l_s: vec2f;
  var l_along_n: f32;
  l_c = coax_p_slab_coarse(l_p);
  if ((coax_at_u(i32(15992i)) < 0x1.000000p-1f))
  {
    return l_c;
  }
  l_q = coax_p_sw_frame(l_p);
  l_g = (coax_p_sw_calm(l_q)).x;
  l_o = coax_p_v4((l_c.x * l_g), (l_c.y * coax_p_smoothstep(0x1.000000p-1f, 0x1.ccccccp-1f, l_g)), (l_c.z * l_g), (l_c.w * l_g));
  if ((coax_at_u(i32(15992i)) > 0x1.800000p0f))
  {
    return l_o;
  }
  l_hx = coax_at_u(i32(16028i));
  l_hz = coax_at_u(i32(16029i));
  if (((abs(l_q.x) < l_hx) && (abs(l_q.y) < l_hz)))
  {
    return l_o;
  }
  l_k = 0i;
  while ((l_k < 4i))
  {
    l_amp = coax_at_u(i32((16000i + l_k)));
    l_n = coax_p_sw_normal(l_k);
    l_half = select(l_hz, l_hx, ((l_k == 0i) || (l_k == 2i)));
    l_span = select((0x1.000000p1f * l_hx), (0x1.000000p1f * l_hz), ((l_k == 0i) || (l_k == 2i)));
    l_out = (((l_q.x * l_n.x) + (l_q.y * l_n.y)) - l_half);
    if (((l_amp > 0x1.47ae14p-7f) && (l_out >= 0.0f)))
    {
      l_along = ((l_q.y * l_n.x) - (l_q.x * l_n.y));
      l_spread = coax_at_u(i32((16008i + l_k)));
      l_from = (l_along - (l_out * coax_at_u(i32((16004i + l_k)))));
      l_edge = ((0x1.47ae14p-5f * l_span) + (l_out * l_spread));
      l_beam = ((coax_p_clampf((0x1.000000p-1f + (((0x1.000000p-1f * l_span) - abs(l_from)) / l_edge)), 0.0f, 0x1.000000p0f) * sqrt((l_span / (l_span + ((0x1.000000p1f * l_out) * l_spread))))) * l_amp);
      if ((l_beam > 0x1.89374cp-9f))
      {
        l_r = coax_p_slab_coarse(coax_p_sw_world(coax_p_v2((l_q.x - ((0x1.000000p1f * l_out) * l_n.x)), (l_q.y - ((0x1.000000p1f * l_out) * l_n.y)))));
        l_s = coax_p_sw_turn(coax_p_v2(l_r.z, l_r.w));
        l_along_n = ((l_s.x * l_n.x) + (l_s.y * l_n.y));
        l_s = coax_p_v2((l_s.x - ((0x1.000000p1f * l_along_n) * l_n.x)), (l_s.y - ((0x1.000000p1f * l_along_n) * l_n.y)));
        l_o = coax_p_v4((l_o.x + (l_beam * l_r.x)), coax_p_fmin((l_o.y + ((l_beam * l_r.y) * coax_p_smoothstep(0x1.000000p-1f, 0x1.ccccccp-1f, l_g))), 0x1.000000p0f), (l_o.z + (l_beam * ((coax_at_u(i32(15998i)) * l_s.x) - (coax_at_u(i32(15999i)) * l_s.y)))), (l_o.w + (l_beam * ((coax_at_u(i32(15999i)) * l_s.x) + (coax_at_u(i32(15998i)) * l_s.y)))));
      }
    }
    l_k = (l_k + 1i);
  }
  return l_o;
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
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_sky = coax_p_sb_mul(coax_p_slab_sea_top_sky(l_out, 0x1.666666p0f), (0x1.000000p0f - l_back));
  l_glare = (coax_p_sb_pow(coax_p_fmax(coax_p_wg_dot3(l_out, l_sun), 0.0f), 0x1.680000p7f) * (0x1.000000p0f - l_back));
  l_mirror = coax_p_sb_mul(coax_p_slab_sea_fog(coax_p_fmin(coax_at_u(i32(9i)), 0.0f), (-0x1.99999ap-2f)), l_back);
  l_col = coax_p_sb_add(coax_p_sb_add(l_sky, l_mirror), coax_p_sb_mul(coax_p_slab_sea_top_sun(), (l_glare * 0x1.800000p2f)));
  l_tr = coax_p_slab_sea_beam(l_dist);
  l_fog = coax_p_slab_sea_fog(coax_p_fmin(coax_at_u(i32(9i)), 0.0f), l_d.y);
  l_shafts = coax_p_slab_sea_shafts(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))), l_d, l_dist);
  return coax_p_slab_sea_under_out(coax_p_sb_add(coax_p_sb_mulv(l_col, l_tr), coax_p_sb_mul(coax_p_sb_mulv(l_fog, coax_p_v3((0x1.000000p0f - l_tr.x), (0x1.000000p0f - l_tr.y), (0x1.000000p0f - l_tr.z))), (0x1.000000p0f + (0x1.99999ap0f * l_shafts)))), coax_at_u(i32(9i)));
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  let l_p0: vec3f = p_p0;
  let l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_pr_lit_plain(p_c: vec4f) -> coax_t21 {
  let l_c: vec4f = p_c;
  return coax_t21(l_c, coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, (-0x1.000000p0f)));
}
fn coax_p_slab_water(p_p: vec2f, p_world: vec3f, p_d: vec3f, p_dist: f32, p_t: f32) -> vec3f {
  let l_p: vec2f = p_p;
  let l_world: vec3f = p_world;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  let l_t: f32 = p_t;
  var l_sea: vec4f;
  var l_white: vec3f;
  var l_fp: f32;
  l_sea = coax_p_slab_sea(l_p, l_world, l_d, l_dist, l_t, coax_p_sw_sea(l_p));
  l_white = coax_p_slab_sea_white();
  l_fp = ((((l_dist * 0x1.000000p1f) * coax_at_u(i32(11i))) / coax_at_u(i32(1i))) / coax_p_fmax(abs(l_d.y), 0x1.47ae14p-4f));
  return coax_p_sw_tint(coax_p_sb_mix(coax_p_sb_mix(coax_p_v3(l_sea.x, l_sea.y, l_sea.z), l_white, coax_p_sw_white_px(l_p, l_t, l_fp)), l_white, coax_p_slab_ship_wake(l_p, l_t)), l_p, l_white, l_t);
}
fn coax_p_slab_photo_sea_haze() -> f32 {
  return select(0x1.99999ap-2f, 0.0f, coax_p_slab_photo());
}
fn coax_p_slab_shore(p_c: vec3f, p_w: vec3f, p_d: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  let l_w: vec3f = p_w;
  let l_d: vec3f = p_d;
  var l_far: f32;
  var l_r: f32;
  var l_along: f32;
  var l_m: f32;
  var l_strand: f32;
  var l_land: vec3f;
  l_far = coax_p_wg_length2(coax_p_v2(coax_at_u(i32(1490i)), coax_at_u(i32(1491i))));
  l_r = coax_p_wg_length2(coax_p_v2(l_w.x, l_w.z));
  if (((l_far < 0x1.000000p0f) || (l_r < (l_far * 0x1.ccccccp-1f))))
  {
    return l_c;
  }
  l_along = (((l_w.x * coax_at_u(i32(1490i))) + (l_w.z * coax_at_u(i32(1491i)))) / l_far);
  l_m = (coax_p_smoothstep(l_far, ((l_far * 0x1.051eb8p0f) + 0x1.e00000p3f), l_along) * coax_p_slab_land_within((l_along / l_r)));
  l_strand = (0x1.000000p0f - coax_p_smoothstep((l_far * 0x1.051eb8p0f), ((l_far * 0x1.0cccccp0f) + 0x1.400000p5f), l_along));
  l_land = coax_p_sb_mix(coax_p_slab_land_colour(), coax_p_sb_mul(coax_p_sb_p(12i), 0x1.a3d70ap-1f), (l_strand * 0x1.333334p-1f));
  return coax_p_sb_mix(l_c, coax_p_slab_land_far(coax_p_slab_haze(l_land, l_d, (coax_p_fmin(l_r, (l_far * 0x1.800000p0f)) * 0x1.99999ap-2f)), coax_p_slab_haze(l_land, l_d, 0x1.e84800p19f), l_far, 0x1.000000p0f), l_m);
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
fn coax_p_pr_at3(p_at: i32) -> vec3f {
  let l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
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
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_slab_paint(p_hit: coax_t19, p_w: vec3f, p_d: vec3f) -> coax_t24 {
  let l_hit: coax_t19 = p_hit;
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
  var l_win: f32;
  var l_pipes: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_strip: f32;
  var l_storey: f32;
  var l_along_38: f32;
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
  var l_along_58: f32;
  var l_h_59: f32;
  var l_cornice: f32;
  var l_col: f32;
  var l_shade: f32;
  var l_win_63: f32;
  var l_fx_64: f32;
  var l_fz_65: f32;
  var l_sky_66: vec3f;
  var l_e: f32;
  var l_px: f32;
  var l_pz: f32;
  var l_gcx: f32;
  var l_gcz: f32;
  var l_nz: f32;
  var l_rr: f32;
  var l_dcx: f32;
  var l_dcz: f32;
  var l_rr_76: f32;
  var l_yc: f32;
  var l_big: f32;
  var l_tier: f32;
  var l_edge_80: f32;
  var l_ang: f32;
  var l_mull_82: f32;
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
    return coax_t24(coax_p_sb_mul(l_c, l_grain), l_emit);
  }
  if (l_oncut)
  {
    return coax_p_slab_section(l_k, l_w);
  }
  if ((l_k == 21i))
  {
    l_c = coax_p_sb_mul(coax_p_v3(0x1.47ae14p-3f, 0x1.5c28f6p-3f, 0x1.70a3d8p-3f), (0x1.ccccccp-1f + (0x1.99999ap-3f * coax_p_slab_band(l_w.y, 0x1.800000p3f, 0x1.47ae14p-4f))));
    return coax_t24(l_c, l_emit);
  }
  if (((l_k == 0i) && (l_n.y < (-0x1.000000p-1f))))
  {
    l_jx = abs((coax_p_fract((l_w.x / 0x1.400000p3f)) - 0x1.000000p-1f));
    l_jz = abs((coax_p_fract((l_w.z / 0x1.400000p3f)) - 0x1.000000p-1f));
    l_c = coax_p_sb_mul(coax_p_v3(0x1.ae147ap-2f, 0x1.ccccccp-2f, 0x1.99999ap-2f), ((0x1.b33334p-1f + (0x1.333334p-3f * coax_p_slab_noise(coax_p_v2((l_w.x * 0x1.99999ap-5f), (l_w.z * 0x1.99999ap-5f))))) * (0x1.000000p0f - (0x1.eb851ep-4f * coax_p_stepf(0x1.eb851ep-2f, coax_p_fmax(l_jx, l_jz))))));
    return coax_t24(l_c, l_emit);
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
              l_along_38 = select(l_w.x, l_w.z, (abs(l_n.x) > abs(l_n.z)));
              l_mull = coax_p_slab_band(l_along_38, 0x1.ccccccp0f, 0x1.eb851ep-4f);
              l_r = coax_p_wg_reflect3(l_d, l_n);
              l_sky = coax_p_sb_add(coax_p_sb_mix(coax_p_sb_p(8i), coax_p_sb_p(12i), 0x1.333334p-2f), coax_p_sb_mul(coax_p_sb_p(4i), (0x1.333334p-1f * coax_p_sb_pow(coax_p_fmax(coax_p_wg_dot3(l_r, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), 0.0f), 0x1.800000p3f))));
              l_pane = coax_p_slab_hash2(coax_p_v2(floor((l_along_38 / 0x1.ccccccp0f)), (floor(((l_w.y - l_deck) / 0x1.ccccccp1f)) + (l_hit.f4 * 0x1.c00000p2f))));
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
              l_lit = coax_p_stepf(0x1.ccccccp-2f, coax_p_slab_hash2(coax_p_v2(floor((l_along_38 / 0x1.ccccccp1f)), (floor(((l_w.y - l_deck) / 0x1.ccccccp1f)) + (l_hit.f4 * 0x1.a00000p3f)))));
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
                        l_along_58 = select(l_w.x, l_w.z, (abs(l_n.x) > abs(l_n.z)));
                        l_h_59 = (l_w.y - l_hit.f5.y);
                        l_cornice = ((l_hit.w - l_hit.f5.y) * 0x1.3d70a4p-1f);
                        if (((l_h_59 > 0x1.333334p0f) && (l_h_59 < l_cornice)))
                        {
                          l_col = coax_p_slab_columns(l_along_58, select(0x1.99999ap1f, 0x1.000000p2f, (l_k == 9i)));
                          l_shade = select(0x1.ae147ap-2f, 0x1.19999ap-1f, (l_k == 18i));
                          l_c = coax_p_sb_mul(l_c, coax_p_mix(l_shade, 0x1.000000p0f, l_col));
                          l_c = coax_p_sb_mul(l_c, (0x1.eb851ep-1f + (0x1.47ae14p-5f * sin((l_along_58 * 0x1.800000p3f)))));
                        }
                        else
                        {
                          if ((l_h_59 <= 0x1.333334p0f))
                          {
                            l_c = coax_p_sb_mul(l_c, (0x1.ccccccp-1f + (0x1.99999ap-4f * coax_p_slab_band(l_h_59, 0x1.99999ap-2f, 0x1.000000p-1f))));
                          }
                          else
                          {
                            l_c = coax_p_sb_mul(l_c, (0x1.f0a3d8p-1f - (0x1.eb851ep-5f * coax_p_slab_band((l_h_59 - l_cornice), 0x1.ccccccp-1f, 0x1.333334p-2f))));
                          }
                        }
                        l_emit = coax_p_sb_mul(coax_p_v3(0x1.000000p0f, 0x1.99999ap-1f, 0x1.19999ap-1f), ((((l_night * 0x1.000000p-2f) * (0x1.000000p0f - coax_p_slab_columns(l_along_58, 0x1.99999ap1f))) * coax_p_stepf(0x1.333334p0f, l_h_59)) * coax_p_stepf(l_h_59, l_cornice)));
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
                            l_win_63 = (coax_p_stepf(0x1.ccccccp-2f, coax_p_fract(((l_w.y - l_deck) / 0x1.ccccccp1f))) * coax_p_slab_band(select(l_w.x, l_w.z, (abs(l_n.x) > abs(l_n.z))), 0x1.000000p2f, 0x1.666666p-1f));
                            l_c = select(l_c, coax_p_v3(0x1.99999ap-2f, 0x1.147ae2p-1f, 0x1.333334p-1f), (l_win_63 > 0x1.000000p-1f));
                            l_emit = coax_p_sb_mul(coax_p_v3(0x1.ccccccp-1f, 0x1.e66666p-1f, 0x1.000000p0f), ((l_win_63 * l_night) * 0x1.ccccccp-2f));
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
                                l_fx_64 = (coax_p_fract((l_w.x / 0x1.400000p3f)) - 0x1.000000p-1f);
                                l_fz_65 = (coax_p_fract((l_w.z / 0x1.400000p3f)) - 0x1.000000p-1f);
                                l_c = select(l_c, coax_p_v3(0x1.99999ap-3f, 0x1.c28f5cp-3f, 0x1.eb851ep-3f), (((l_fx_64 * l_fx_64) + (l_fz_65 * l_fz_65)) < 0x1.eb851ep-4f));
                              }
                            }
                            else
                            {
                              if ((l_k == 14i))
                              {
                                l_sky_66 = coax_p_slab_sky(coax_p_wg_reflect3(l_d, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)));
                                l_c = coax_p_sb_mix(coax_p_v3(0x1.eb851ep-5f, 0x1.0a3d70p-2f, 0x1.47ae14p-2f), l_sky_66, 0x1.ccccccp-2f);
                                if ((!l_roof))
                                {
                                  l_c = coax_p_v3(0x1.a3d70ap-1f, 0x1.99999ap-1f, 0x1.7ae148p-1f);
                                }
                                else
                                {
                                  l_c = coax_p_sw_pool(l_c, l_w, l_d);
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
                                      l_rr_76 = sqrt((((l_w.x - l_dcx) * (l_w.x - l_dcx)) + ((l_w.z - l_dcz) * (l_w.z - l_dcz))));
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
                                          l_edge_80 = (l_big - (0x1.800000p2f * l_tier));
                                          l_c = coax_p_v3(0x1.b851ecp-1f, 0x1.ae147ap-1f, 0x1.8f5c28p-1f);
                                          if ((l_rr_76 > (l_edge_80 - 0x1.4cccccp1f)))
                                          {
                                            l_c = coax_p_sb_mix(coax_p_v3(0x1.0a3d70p-2f, 0x1.70a3d8p-2f, 0x1.47ae14p-3f), coax_p_v3(0x1.851eb8p-2f, 0x1.d70a3ep-2f, 0x1.c28f5cp-3f), coax_p_slab_noise(coax_p_v2((l_w.x * 0x1.000000p-1f), (l_w.z * 0x1.000000p-1f))));
                                          }
                                          if ((l_w.y > (l_yc + 0x1.d00000p4f)))
                                          {
                                            l_c = select(coax_p_v3(0x1.99999ap-1f, 0x1.a3d70ap-1f, 0x1.99999ap-1f), coax_p_v3(0x1.ccccccp-1f, 0x1.ccccccp-1f, 0x1.c28f5cp-1f), (l_rr_76 < 0x1.200000p2f));
                                          }
                                        }
                                        else
                                        {
                                          l_ang = (coax_p_wg_atan2((l_w.z - l_dcz), (l_w.x - l_dcx)) * l_rr_76);
                                          l_mull_82 = coax_p_fmax(coax_p_slab_band(l_ang, 0x1.800000p1f, 0x1.99999ap-4f), coax_p_slab_band((l_w.y - l_yc), 0x1.800000p1f, 0x1.47ae14p-4f));
                                          l_c = coax_p_sb_mix(coax_p_v3(0x1.333334p-2f, 0x1.851eb8p-2f, 0x1.ae147ap-2f), coax_p_v3(0x1.d70a3ep-1f, 0x1.d70a3ep-1f, 0x1.ccccccp-1f), l_mull_82);
                                          if ((l_w.y > (l_yc + 0x1.e00000p4f)))
                                          {
                                            l_c = coax_p_sb_mix(coax_p_v3(0x1.3d70a4p-1f, 0x1.7ae148p-1f, 0x1.8f5c28p-1f), coax_p_v3(0x1.d70a3ep-1f, 0x1.d70a3ep-1f, 0x1.ccccccp-1f), l_mull_82);
                                          }
                                          l_emit = coax_p_sb_mul(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.3d70a4p-1f), (((0x1.000000p0f - l_mull_82) * coax_at_u(i32(1479i))) * 0x1.000000p-1f));
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
  return coax_t24(coax_p_sb_mul(l_c, l_grain), l_emit);
}
fn coax_p_slab_deck_at(p_p: vec2f) -> vec4f {
  let l_p: vec2f = p_p;
  var l_sx: f32;
  var l_sz: f32;
  var l_gx: f32;
  var l_gz: f32;
  var l_d: vec4f;
  var l_g: f32;
  var l_edge: f32;
  l_sx = (l_p.x - (coax_p_wg_round((l_p.x / 0x1.900000p6f)) * 0x1.900000p6f));
  l_sz = (l_p.y - (coax_p_wg_round((l_p.y / 0x1.900000p6f)) * 0x1.900000p6f));
  l_gx = abs(l_sx);
  l_gz = abs(l_sz);
  l_d = vec4f(0x1.600000p3f, l_sx, l_p.y, l_gz);
  if ((l_gz < l_gx))
  {
    l_d.y = l_sz;
    l_d.z = l_p.x;
    l_d.w = l_gx;
  }
  l_g = abs(l_d.y);
  if ((l_g < 0x1.400000p3f))
  {
    l_d.x = select(select(0x1.400000p3f, 0x1.a00000p3f, (l_g < 0x1.533334p2f)), 0x1.200000p3f, (l_g < 0x1.400000p2f));
    if ((l_d.w < 0x1.400000p2f))
    {
      l_d.x = 0x1.200000p3f;
    }
  }
  l_edge = coax_p_slab_hull_plan(l_p);
  if ((l_edge > (-0x1.400000p4f)))
  {
    l_d.x = 0x1.800000p3f;
  }
  return l_d;
}
fn coax_p_pr_decode(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_v3(coax_p_pr_decode1(l_c.x), coax_p_pr_decode1(l_c.y), coax_p_pr_decode1(l_c.z));
}
fn coax_p_mul3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_pr_surface(p_v: coax_t16, p_b: i32) -> coax_t26 {
  let l_v: coax_t16 = p_v;
  let l_b: i32 = p_b;
  var l_row: i32;
  var l_s: coax_t26;
  var l_pattern: i32;
  var l_scale: f32;
  var l_full: bool;
  var l_fl: vec3f;
  var l_f: vec2f;
  var l_ax: f32;
  var l_ay: f32;
  var l_da: f32;
  var l_seed: f32;
  var l_cloud: f32;
  var l_grain: f32;
  var l_side: f32;
  var l_cx: f32;
  var l_cz: f32;
  var l_tone: f32;
  var l_joint: f32;
  var l_wear: f32;
  var l_grit: f32;
  var l_worn: f32;
  var l_tuft: f32;
  var l_fine: f32;
  var l_t: f32;
  var l_wind: vec4f;
  var l_depth: f32;
  var l_amp: f32;
  var l_run: f32;
  var l_qx: f32;
  var l_qz: f32;
  var l_rx: f32;
  var l_rz: f32;
  var l_toward: vec3f;
  var l_ci: f32;
  var l_sun_in: f32;
  var l_body: vec3f;
  var l_net: f32;
  var l_grit_41: f32;
  var l_stone: f32;
  var l_rake: f32;
  var l_row_c: vec3f;
  var l_tuft_45: f32;
  var l_clump: f32;
  var l_cellx: f32;
  var l_cellz: f32;
  var l_flower: f32;
  var l_rib: f32;
  var l_phase: f32;
  var l_slope: f32;
  var l_sheet: f32;
  var l_mow: f32;
  var l_blade: f32;
  l_row = (i32(coax_p_wg_clamp_i(i32((l_v.f6.x + 0x1.000000p-1f)), 0i, 63i)) * 8i);
  l_s = coax_t26();
  l_s.x = coax_p_mul3(coax_p_pr_decode(coax_p_v3(materials[l_row], materials[(l_row + 1i)], materials[(l_row + 2i)])), l_v.f5);
  l_s.y = materials[(l_row + 3i)];
  l_s.z = materials[(l_row + 4i)];
  l_s.w = coax_p_normalize3(l_v.w);
  l_s.f4 = coax_p_scale3(l_s.x, materials[(l_row + 7i)]);
  l_s.f5 = 0.0f;
  l_pattern = i32((materials[(l_row + 5i)] + 0x1.000000p-1f));
  l_scale = materials[(l_row + 6i)];
  l_full = (coax_at_u(i32(((l_b + 24i) + 3i))) < 0x1.000000p-1f);
  l_fl = l_v.f4;
  l_f = coax_p_pr_face(l_v.z, l_fl);
  l_ax = fwidth(l_f.x);
  l_ay = fwidth(l_f.y);
  l_da = (fwidth(l_fl.x) + fwidth(l_fl.z));
  l_seed = (l_v.f6.z * 0x1.d3d70ap2f);
  if ((l_pattern == 1i))
  {
    l_cloud = coax_p_pr_cloud2(((l_f.x * 0x1.70a3d8p-5f) + l_seed), ((l_f.y * 0x1.70a3d8p-5f) - l_seed));
    l_grain = coax_p_pr_noise2((l_f.x * 0x1.200000p3f), ((l_f.y * 0x1.200000p3f) + l_seed));
    l_s.x = coax_p_scale3(l_s.x, ((0x1.b851ecp-1f + (0x1.99999ap-3f * l_cloud)) + (0x1.eb851ep-5f * (l_grain - 0x1.000000p-1f))));
    l_s.y = coax_p_saturate((l_s.y + (0x1.eb851ep-4f * (l_cloud - 0x1.000000p-1f))));
  }
  else
  {
    if ((l_pattern == 2i))
    {
      l_s = coax_p_pr_glazing(l_s, l_f, l_ax, l_ay, l_fl, l_seed, l_scale, 0i, l_full, l_b);
    }
    else
    {
      if ((l_pattern == 13i))
      {
        l_s = coax_p_pr_glazing(l_s, l_f, l_ax, l_ay, l_fl, l_seed, l_scale, 1i, l_full, l_b);
      }
      else
      {
        if ((l_pattern == 14i))
        {
          l_s = coax_p_pr_glazing(l_s, l_f, l_ax, l_ay, l_fl, l_seed, l_scale, 2i, l_full, l_b);
        }
        else
        {
          if ((l_pattern == 12i))
          {
            l_s = coax_p_pr_poured(l_s, l_f, l_ax, l_ay, l_v.y, l_fl, l_seed, l_scale, l_b, (l_row + 8i));
          }
          else
          {
            if ((l_pattern == 15i))
            {
              l_s = coax_p_pr_painted(l_s, l_f, l_ax, l_ay, l_v.z, l_fl, l_seed, l_scale);
            }
            else
            {
              if ((l_pattern == 3i))
              {
                l_side = select(0x1.333334p0f, l_scale, (l_scale > 0.0f));
                l_cx = floor((l_f.x / l_side));
                l_cz = floor((l_f.y / l_side));
                l_tone = coax_p_render_noise2(l_cx, l_cz);
                l_joint = coax_p_fmax(coax_p_pr_line(l_f.x, l_side, 0x1.47ae14p-6f, l_ax), coax_p_pr_line(l_f.y, l_side, 0x1.47ae14p-6f, l_ay));
                l_wear = coax_p_pr_cloud2((l_f.x * 0x1.47ae14p-4f), (l_f.y * 0x1.47ae14p-4f));
                l_s.x = coax_p_scale3(l_s.x, (((0x1.ccccccp-1f + (0x1.47ae14p-3f * l_tone)) * (0x1.c28f5cp-1f + (0x1.99999ap-3f * l_wear))) * (0x1.000000p0f - (0x1.666666p-2f * l_joint))));
                l_s.y = coax_p_saturate(((l_s.y + (0x1.99999ap-4f * (l_tone - 0x1.000000p-1f))) + (0x1.99999ap-3f * l_joint)));
              }
              else
              {
                if ((l_pattern == 4i))
                {
                  l_grit = coax_p_pr_noise2((l_f.x * 0x1.c00000p3f), (l_f.y * 0x1.c00000p3f));
                  l_worn = coax_p_pr_cloud2((l_f.x * 0x1.eb851ep-5f), (l_f.y * 0x1.eb851ep-5f));
                  l_s.x = coax_p_scale3(l_s.x, ((0x1.99999ap-1f + (0x1.000000p-2f * l_worn)) + (0x1.eb851ep-4f * (l_grit - 0x1.000000p-1f))));
                }
                else
                {
                  if ((l_pattern == 5i))
                  {
                    l_tuft = coax_p_pr_noise2(((l_v.z.x * 0x1.4cccccp0f) + l_seed), ((l_v.z.z * 0x1.4cccccp0f) - (l_v.z.y * 0x1.ccccccp-1f)));
                    l_fine = select(0x1.000000p-1f, coax_p_pr_noise2((((l_v.z.x * 0x1.c00000p2f) - (l_v.z.y * 0x1.800000p1f)) + l_seed), ((l_v.z.z * 0x1.c00000p2f) + (l_v.z.y * 0x1.400000p2f))), l_full);
                    l_s.x = coax_p_scale3(l_s.x, ((0x1.3d70a4p-1f + (0x1.666666p-1f * l_tuft)) * (0x1.99999ap-1f + (0x1.99999ap-2f * l_fine))));
                    l_s.f5 = 0x1.666666p-2f;
                  }
                  else
                  {
                    if ((l_pattern == 6i))
                    {
                      if ((l_fl.y > 0x1.666666p-1f))
                      {
                        l_t = coax_at_u(i32(((l_b + 12i) + 3i)));
                        l_wind = coax_p_wg_at4((l_b + 104i));
                        l_depth = select(0x1.333334p0f, l_scale, (l_scale > 0.0f));
                        l_amp = coax_p_fmin((0x1.47ae14p-6f + (0x1.6872b0p-7f * l_wind.z)), 0x1.47ae14p-3f);
                        l_run = (l_t * (0x1.eb851ep-4f + (0x1.eb851ep-6f * l_wind.z)));
                        l_qx = (l_f.x - (l_wind.x * l_run));
                        l_qz = (l_f.y - (l_wind.y * l_run));
                        l_rx = ((coax_p_pr_noise2(((l_qx * 0x1.b33334p0f) + l_seed), (l_qz * 0x1.b33334p0f)) - 0x1.000000p-1f) + (0x1.000000p-1f * (coax_p_pr_noise2(((l_qx * 0x1.466666p2f) - (l_t * 0x1.99999ap-2f)), ((l_qz * 0x1.466666p2f) + l_seed)) - 0x1.000000p-1f)));
                        l_rz = ((coax_p_pr_noise2(((l_qx * 0x1.b33334p0f) + 0x1.300000p4f), ((l_qz * 0x1.b33334p0f) - l_seed)) - 0x1.000000p-1f) + (0x1.000000p-1f * (coax_p_pr_noise2(((l_qx * 0x1.466666p2f) + 0x1.c00000p2f), ((l_qz * 0x1.466666p2f) + (l_t * 0x1.7ae148p-2f))) - 0x1.000000p-1f)));
                        l_s.w = coax_p_normalize3(coax_p_add3(l_s.w, coax_p_v3((l_rx * l_amp), 0.0f, (l_rz * l_amp))));
                        l_toward = coax_p_normalize3(coax_p_sub3(coax_p_pr_at3((l_b + 0i)), l_v.y));
                        l_ci = coax_p_fmax(dot(l_s.w, l_toward), 0x1.eb851ep-6f);
                        l_sun_in = coax_p_wo_cos_refracted(coax_p_fmax(coax_at_u(i32(((l_b + 16i) + 1i))), 0x1.99999ap-5f));
                        l_body = coax_p_wo_through(coax_p_v3(0x1.ae147ap-2f, 0x1.333334p-4f, 0x1.1eb852p-5f), ((l_depth / coax_p_wo_cos_refracted(l_ci)) + (l_depth / l_sun_in)));
                        l_net = select(0x1.000000p0f, (0x1.000000p0f + ((0x1.000000p-1f * (coax_p_wo_caustic(coax_p_v2((l_f.x * 0x1.666666p0f), (l_f.y * 0x1.666666p0f)), l_t) - 0x1.70a3d8p-3f)) * coax_at_u(i32(((l_b + 16i) + 3i))))), l_full);
                        l_s.x = coax_p_scale3(coax_p_mul3(l_s.x, l_body), ((0x1.147ae2p-1f * 0x1.99999ap0f) * l_net));
                        l_s.y = 0x1.47ae14p-5f;
                        l_s.z = 0.0f;
                      }
                    }
                    else
                    {
                      if ((l_pattern == 10i))
                      {
                        l_grit_41 = coax_p_pr_noise2(((l_f.x * 0x1.700000p4f) + l_seed), (l_f.y * 0x1.700000p4f));
                        l_stone = coax_p_pr_noise2((l_f.x * 0x1.e80000p5f), ((l_f.y * 0x1.e80000p5f) - l_seed));
                        l_rake = coax_p_pr_cloud2((l_f.x * 0x1.666666p-2f), (l_f.y * 0x1.666666p-2f));
                        l_s.x = coax_p_scale3(l_s.x, (((0x1.8f5c28p-1f + (0x1.0a3d70p-2f * l_grit_41)) + (0x1.99999ap-3f * (l_stone - 0x1.000000p-1f))) * (0x1.ccccccp-1f + (0x1.99999ap-3f * l_rake))));
                        if (l_full)
                        {
                          l_s.w = coax_p_normalize3(coax_p_add3(l_s.w, coax_p_v3(((l_grit_41 - 0x1.000000p-1f) * 0x1.333334p-2f), 0.0f, ((l_stone - 0x1.000000p-1f) * 0x1.333334p-2f))));
                        }
                      }
                      else
                      {
                        if ((l_pattern == 11i))
                        {
                          l_row_c = coax_p_pr_decode(coax_p_v3(materials[l_row], materials[(l_row + 1i)], materials[(l_row + 2i)]));
                          l_tuft_45 = coax_p_pr_noise2(((l_v.z.x * 0x1.0cccccp1f) + l_seed), ((l_v.z.z * 0x1.0cccccp1f) - l_v.z.y));
                          l_clump = coax_p_pr_cloud2(((l_v.z.x * 0x1.666666p-2f) - l_seed), ((l_v.z.z * 0x1.666666p-2f) + l_seed));
                          l_cellx = floor(((l_v.z.x * 0x1.800000p2f) + (l_v.z.y * 0x1.000000p1f)));
                          l_cellz = floor(((l_v.z.z * 0x1.800000p2f) - (l_v.z.y * 0x1.000000p1f)));
                          l_flower = coax_p_stepf((0x1.000000p0f - (0x1.000000p-1f * coax_p_smoothstep(0x1.666666p-2f, 0x1.666666p-1f, l_clump))), coax_p_render_noise2((l_cellx + (l_seed * 0x1.f00000p4f)), l_cellz));
                          l_s.x = coax_p_mix3(coax_p_scale3(l_row_c, (0x1.333334p-1f + (0x1.666666p-1f * l_tuft_45))), coax_p_scale3(l_v.f5, (0x1.800000p-1f + (0x1.99999ap-2f * coax_p_render_noise2(l_cellz, l_cellx)))), l_flower);
                          l_s.y = 0x1.ccccccp-1f;
                          l_s.f5 = 0x1.000000p-2f;
                        }
                        else
                        {
                          if ((l_pattern == 7i))
                          {
                            l_s = coax_p_pr_stone(l_s, l_f, l_ax, l_ay, l_da, l_v.z, l_fl, l_seed, l_scale, l_full, (l_row + 8i));
                          }
                          else
                          {
                            if ((l_pattern == 8i))
                            {
                              l_rib = select(0x1.333334p-2f, l_scale, (l_scale > 0.0f));
                              l_phase = coax_p_fract((l_f.x / l_rib));
                              l_slope = select(select(0.0f, (0.0f - 0x1.000000p-1f), ((l_phase > 0x1.000000p-1f) && (l_phase < 0x1.666666p-1f))), 0x1.000000p-1f, (l_phase < 0x1.99999ap-3f));
                              if (l_full)
                              {
                                l_s.w = coax_p_pr_tilt(l_s.w, l_fl, (l_slope * coax_p_saturate(((l_rib / (coax_p_fmax(l_ax, 0x1.a36e2ep-14f) * 0x1.000000p2f)) - 0x1.000000p-1f))), 0.0f);
                              }
                              l_sheet = coax_p_render_noise2((floor((l_f.x / (l_rib * 0x1.400000p3f))) + l_seed), floor((l_f.y / 0x1.800000p2f)));
                              l_s.x = coax_p_scale3(l_s.x, ((0x1.ccccccp-1f + (0x1.1eb852p-3f * l_sheet)) + (0x1.47ae14p-4f * coax_p_pr_cloud2((l_f.x * 0x1.99999ap-3f), (l_f.y * 0x1.99999ap-5f)))));
                            }
                            else
                            {
                              if ((l_pattern == 9i))
                              {
                                l_mow = coax_p_pr_cloud2((l_v.y.x * 0x1.c28f5cp-4f), (l_v.y.z * 0x1.c28f5cp-4f));
                                l_blade = coax_p_pr_noise2((l_v.y.x * 0x1.800000p2f), (l_v.y.z * 0x1.800000p2f));
                                l_s.x = coax_p_scale3(l_s.x, ((0x1.800000p-1f + (0x1.99999ap-2f * l_mow)) + (0x1.99999ap-3f * (l_blade - 0x1.000000p-1f))));
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
  return l_s;
}
fn coax_p_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
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
fn coax_p_slab_photo_rough(p_k: i32, p_roof: bool) -> f32 {
  let l_k: i32 = p_k;
  let l_roof: bool = p_roof;
  if ((l_k == 0i))
  {
    return select(0x1.99999ap-1f, 0x1.3d70a4p-1f, l_roof);
  }
  if (((((((l_k == 4i) || (l_k == 8i)) || (l_k == 10i)) || (l_k == 12i)) || (l_k == 13i)) || (l_k == 20i)))
  {
    return 0x1.ccccccp-2f;
  }
  if ((((((l_k == 9i) || (l_k == 16i)) || (l_k == 18i)) || (l_k == 27i)) || (l_k == 19i)))
  {
    return 0x1.19999ap-1f;
  }
  if (((l_k == 15i) || (l_k == 7i)))
  {
    return 0x1.ccccccp-1f;
  }
  return 0x1.800000p-1f;
}
fn coax_p_sw_pool(p_c: vec3f, p_w: vec3f, p_d: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  let l_w: vec3f = p_w;
  let l_d: vec3f = p_d;
  var l_k: f32;
  var l_cz: f32;
  var l_level: f32;
  var l_t: f32;
  var l_hit: vec2f;
  var l_r: f32;
  var l_light: f32;
  var l_ring: f32;
  var l_hx: f32;
  var l_hz: f32;
  var l_qa: f32;
  var l_qb: f32;
  var l_qc: f32;
  var l_tw: f32;
  var l_down: f32;
  var l_wet: f32;
  if ((((coax_at_u(i32(15992i)) < 0x1.000000p-1f) || (coax_at_u(i32(15992i)) > 0x1.800000p0f)) || (abs((l_w.x + 0x1.850000p8f)) > 0x1.a00000p2f)))
  {
    return l_c;
  }
  l_k = floor((((l_w.z + 0x1.c20000p7f) / 0x1.2c0000p7f) + 0x1.000000p-1f));
  if (((l_k < 0.0f) || (l_k > 0x1.800000p1f)))
  {
    return l_c;
  }
  l_cz = ((-0x1.c20000p7f) + (0x1.2c0000p7f * l_k));
  if ((coax_p_wg_length2(coax_p_v2((l_w.x + 0x1.850000p8f), (l_w.z - l_cz))) > 0x1.933334p2f))
  {
    return l_c;
  }
  l_level = coax_at_u(i32((16024i + i32(l_k))));
  if ((l_d.y > (-0x1.47ae14p-7f)))
  {
    return coax_p_v3(0x1.99999ap-5f, 0x1.eb851ep-5f, 0x1.eb851ep-5f);
  }
  l_t = ((l_w.y - l_level) / (-l_d.y));
  l_hit = coax_p_v2(((l_w.x + 0x1.850000p8f) + (l_d.x * l_t)), ((l_w.z - l_cz) + (l_d.z * l_t)));
  l_r = coax_p_wg_length2(l_hit);
  l_light = (0x1.333334p-2f + (0x1.666666p-1f * coax_at_u(i32(1447i))));
  if ((l_r < 0x1.800000p2f))
  {
    l_ring = (coax_p_smoothstep(0x1.266666p2f, 0x1.79999ap2f, l_r) * coax_p_clampf((abs(l_level) * 0x1.000000p-1f), 0.0f, 0x1.000000p0f));
    return coax_p_sb_mul(coax_p_sb_mix(coax_p_v3(0x1.eb851ep-6f, 0x1.0a3d70p-3f, 0x1.47ae14p-3f), coax_p_v3(0x1.666666p-1f, 0x1.8f5c28p-1f, 0x1.8f5c28p-1f), (l_ring * 0x1.666666p-1f)), l_light);
  }
  l_hx = (l_w.x + 0x1.850000p8f);
  l_hz = (l_w.z - l_cz);
  l_qa = ((l_d.x * l_d.x) + (l_d.z * l_d.z));
  l_qb = ((l_hx * l_d.x) + (l_hz * l_d.z));
  l_qc = (((l_hx * l_hx) + (l_hz * l_hz)) - 0x1.200000p5f);
  l_tw = (((-l_qb) + sqrt(coax_p_fmax(((l_qb * l_qb) - (l_qa * l_qc)), 0.0f))) / coax_p_fmax(l_qa, 0x1.4f8b58p-17f));
  l_down = ((-l_d.y) * l_tw);
  l_wet = (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.800000p2f, ((l_w.y - l_down) - l_level)));
  return coax_p_sb_mul(coax_p_sb_mix(coax_p_v3(0x1.99999ap-2f, 0x1.8f5c28p-2f, 0x1.70a3d8p-2f), coax_p_v3(0x1.eb851ep-4f, 0x1.333334p-3f, 0x1.333334p-3f), l_wet), (l_light * exp(((-l_down) / 0x1.680000p5f))));
}
fn coax_p_pr_sky_light(p_n: vec3f, p_b: i32) -> vec3f {
  let l_n: vec3f = p_n;
  let l_b: i32 = p_b;
  var l_o: i32;
  var l_c: vec3f;
  l_o = (l_b + 68i);
  l_c = coax_p_pr_at3(l_o);
  l_c = coax_p_add3(l_c, coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 4i)), l_n.y), coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 8i)), l_n.z), coax_p_scale3(coax_p_pr_at3((l_o + 12i)), l_n.x))));
  l_c = coax_p_add3(l_c, coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 16i)), (l_n.x * l_n.y)), coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 20i)), (l_n.y * l_n.z)), coax_p_scale3(coax_p_pr_at3((l_o + 24i)), (((0x1.800000p1f * l_n.z) * l_n.z) - 0x1.000000p0f)))));
  l_c = coax_p_add3(l_c, coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 28i)), (l_n.x * l_n.z)), coax_p_scale3(coax_p_pr_at3((l_o + 32i)), ((l_n.x * l_n.x) - (l_n.y * l_n.y)))));
  return coax_p_v3(coax_p_fmax(l_c.x, 0.0f), coax_p_fmax(l_c.y, 0.0f), coax_p_fmax(l_c.z, 0.0f));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_slab_deck_street(p_s0: coax_t26, p_d: vec4f, p_aa: f32) -> coax_t26 {
  let l_s0: coax_t26 = p_s0;
  let l_d: vec4f = p_d;
  let l_aa: f32 = p_aa;
  var l_s: coax_t26;
  var l_g: f32;
  var l_white: f32;
  var l_junction: bool;
  var l_track: f32;
  var l_oil: f32;
  l_s = l_s0;
  l_g = abs(l_d.y);
  l_white = 0.0f;
  if ((l_d.x < (0x1.200000p3f + 0x1.000000p-1f)))
  {
    l_junction = (l_d.w < 0x1.400000p3f);
    if ((!l_junction))
    {
      l_white = coax_p_fmax(l_white, coax_p_pr_band(l_g, 0x1.0cccccp2f, 0x1.eb851ep-4f, l_aa, 0x1.400000p3f));
      l_track = coax_p_fmax(coax_p_pr_band(l_g, 0x1.333334p0f, 0x1.666666p-1f, l_aa, 0.0f), coax_p_pr_band(l_g, 0x1.b33334p1f, 0x1.666666p-1f, l_aa, 0.0f));
      l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - (0x1.1eb852p-4f * l_track)));
      l_s.y = (l_s.y - (0x1.99999ap-4f * l_track));
      l_oil = (coax_p_pr_band(l_g, 0x1.266666p1f, 0x1.333334p-1f, l_aa, 0.0f) * coax_p_smoothstep(0x1.19999ap-1f, 0x1.99999ap-1f, coax_p_pr_noise2((l_d.z * 0x1.666666p-1f), (l_d.y * 0x1.000000p1f))));
      l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - (0x1.000000p-2f * l_oil)));
    }
    l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - ((0x1.851eb8p-2f * coax_p_pr_band(l_g, 0x1.300000p2f, 0x1.000000p-1f, l_aa, 0.0f)) * select(0x1.000000p0f, 0.0f, l_junction))));
  }
  else
  {
    if ((l_d.x < (0x1.a00000p3f + 0x1.000000p-1f)))
    {
      l_s.x = coax_p_scale3(l_s.x, coax_p_mix(0x1.19999ap-1f, 0x1.1eb852p0f, coax_p_smoothstep(0x1.41eb86p2f, 0x1.466666p2f, l_g)));
    }
    else
    {
      if ((l_d.x < (0x1.400000p3f + 0x1.000000p-1f)))
      {
        l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - (0x1.eb851ep-4f * coax_p_pr_band(l_g, 0x1.59999ap2f, 0x1.99999ap-3f, l_aa, 0.0f))));
      }
    }
  }
  l_s.x = coax_p_mix3(l_s.x, coax_p_v3(0x1.3d70a4p-1f, 0x1.3d70a4p-1f, 0x1.28f5c2p-1f), l_white);
  l_s.y = coax_p_mix(l_s.y, 0x1.333334p-1f, l_white);
  return l_s;
}
fn coax_p_pr_cloud2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return (((coax_p_pr_noise2(l_x, l_y) * 0x1.23d70ap-1f) + (coax_p_pr_noise2(((l_x * 0x1.03d70ap1f) + 0x1.600000p3f), ((l_y * 0x1.03d70ap1f) - 0x1.c00000p2f)) * 0x1.28f5c2p-2f)) + (coax_p_pr_noise2(((l_x * 0x1.00a3d8p2f) - 0x1.800000p1f), ((l_y * 0x1.00a3d8p2f) + 0x1.300000p4f)) * 0x1.1eb852p-3f));
}
fn coax_p_pr_band(p_x: f32, p_c: f32, p_wide: f32, p_aa0: f32, p_every: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_c: f32 = p_c;
  let l_wide: f32 = p_wide;
  let l_aa0: f32 = p_aa0;
  let l_every: f32 = p_every;
  var l_h: f32;
  l_h = (coax_p_fmax(l_aa0, 0x1.a36e2ep-14f) * 0x1.000000p-1f);
  return coax_p_saturate(((coax_p_fmin((l_x + l_h), (l_c + (l_wide * 0x1.000000p-1f))) - coax_p_fmax((l_x - l_h), (l_c - (l_wide * 0x1.000000p-1f)))) / (l_h * 0x1.000000p1f)));
}
fn coax_p_pr_wet(p_s0: coax_t26, p_wet: f32, p_puddle: f32, p_flat: vec3f) -> coax_t26 {
  let l_s0: coax_t26 = p_s0;
  let l_wet: f32 = p_wet;
  let l_puddle: f32 = p_puddle;
  let l_flat: vec3f = p_flat;
  var l_s: coax_t26;
  var l_w: f32;
  var l_p: f32;
  l_s = l_s0;
  l_w = coax_p_saturate(l_wet);
  l_p = (coax_p_saturate(l_puddle) * l_w);
  l_s.x = coax_p_scale3(l_s.x, ((0x1.000000p0f - (0x1.ae147ap-2f * l_w)) * (0x1.000000p0f - (0x1.333334p-2f * l_p))));
  l_s.y = coax_p_mix(coax_p_mix(l_s.y, coax_p_fmin(l_s.y, 0x1.c28f5cp-3f), l_w), 0x1.eb851ep-6f, l_p);
  l_s.w = coax_p_normalize3(coax_p_mix3(l_s.w, l_flat, l_p));
  return l_s;
}
fn coax_p_pr_noise2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  var l_ix: f32;
  var l_iy: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_ux: f32;
  var l_uy: f32;
  l_ix = floor(l_x);
  l_iy = floor(l_y);
  l_fx = (l_x - l_ix);
  l_fy = (l_y - l_iy);
  l_ux = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_uy = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  return coax_p_mix(coax_p_mix(coax_p_render_noise2(l_ix, l_iy), coax_p_render_noise2((l_ix + 0x1.000000p0f), l_iy), l_ux), coax_p_mix(coax_p_render_noise2(l_ix, (l_iy + 0x1.000000p0f)), coax_p_render_noise2((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f)), l_ux), l_uy);
}
fn coax_p_sw_wash(p_w: vec3f, p_nw: vec3f, p_level: f32) -> vec2f {
  let l_w: vec3f = p_w;
  let l_nw: vec3f = p_nw;
  let l_level: f32 = p_level;
  var l_up: f32;
  var l_y: f32;
  var l_q: vec2f;
  var l_nf: vec2f;
  var l_k: i32;
  var l_n: vec2f;
  var l_tg: vec2f;
  var l_hx: f32;
  var l_hz: f32;
  var l_span: f32;
  var l_foot: f32;
  var l_line: f32;
  var l_top: f32;
  var l_above: f32;
  var l_rag: f32;
  var l_white: f32;
  var l_wet: f32;
  if (((coax_at_u(i32(15992i)) < 0x1.000000p-1f) || (coax_at_u(i32(15992i)) > 0x1.800000p0f)))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_up = coax_at_u(i32(16033i));
  l_y = (l_w.y - l_level);
  if (((((l_up < 0x1.99999ap-4f) || (l_y > (l_up + 0x1.000000p1f))) || (l_y < (-0x1.000000p3f))) || (abs(l_nw.y) > 0x1.000000p-1f)))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_q = coax_p_sw_frame(coax_p_v2(l_w.x, l_w.z));
  l_nf = coax_p_sw_turn(coax_p_v2(l_nw.x, l_nw.z));
  l_k = select(select(3i, 1i, (l_nf.y > 0.0f)), select(2i, 0i, (l_nf.x > 0.0f)), (abs(l_nf.x) > abs(l_nf.y)));
  l_n = coax_p_sw_normal(l_k);
  l_tg = coax_p_v2((-l_n.y), l_n.x);
  l_hx = coax_at_u(i32(16028i));
  l_hz = coax_at_u(i32(16029i));
  l_span = select((0x1.000000p1f * l_hx), (0x1.000000p1f * l_hz), ((l_k == 0i) || (l_k == 2i)));
  l_foot = coax_p_sw_foot(l_k, ((l_q.x * l_tg.x) + (l_q.y * l_tg.y)), l_span);
  if ((l_foot < 0x1.47ae14p-7f))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_line = (coax_p_sw_sea(coax_p_sw_world(coax_p_v2((l_q.x + (0x1.800000p0f * l_n.x)), (l_q.y + (0x1.800000p0f * l_n.y)))))).x;
  l_top = (coax_p_fmax(l_line, 0.0f) + (l_up * coax_p_smoothstep(0.0f, 0x1.333334p-2f, l_foot)));
  l_above = (l_y - l_line);
  l_rag = coax_p_slab_noise(coax_p_v2((((l_q.x * l_tg.x) + (l_q.y * l_tg.y)) * 0x1.666666p-2f), (l_y * 0x1.333334p-1f)));
  l_white = ((sqrt(l_foot) * (0x1.000000p0f - coax_p_smoothstep((0x1.99999ap-2f * l_top), l_top, l_y))) * coax_p_smoothstep((-0x1.000000p-1f), 0x1.333334p-2f, l_above));
  l_wet = (coax_p_smoothstep(0.0f, 0x1.333334p-3f, l_foot) * (0x1.000000p0f - coax_p_smoothstep(l_top, (l_top + 0x1.800000p0f), l_y)));
  return coax_p_v2(coax_p_clampf((l_white * (0x1.333334p-1f + (0x1.99999ap-1f * l_rag))), 0.0f, 0x1.000000p0f), l_wet);
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_pr_shadow_edge(p_p: vec3f, p_b: i32) -> f32 {
  let l_p: vec3f = p_p;
  let l_b: i32 = p_b;
  var l_q: vec4f;
  l_q = coax_p_pr_light_clip(l_p, l_b);
  if (((l_q.z < 0.0f) || (l_q.z > 0x1.000000p0f)))
  {
    return 0x1.000000p1f;
  }
  return coax_p_fmax(abs(l_q.x), abs(l_q.y));
}
fn coax_p_slab_scene(p_o: vec3f, p_d: vec3f, p_masses: bool, p_limit: f32) -> coax_t19 {
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  let l_masses: bool = p_masses;
  let l_limit: f32 = p_limit;
  var l_hit: coax_t19;
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
  l_hit = coax_t19(l_limit, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (-0x1.000000p0f), 0.0f, (-0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0.0f, 0.0f));
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
            l_hit = coax_t19(l_h.x, coax_p_v3(l_h.y, l_h.z, l_h.w), (l_p.w + select(0.0f, 0x1.000000p-2f, (l_sh >= 11i))), l_q.y, f32(l_i), l_lo, l_hi);
          }
        }
        l_i = (l_i + 1i);
      }
    }
    l_g = (l_g + 1i);
  }
  return l_hit;
}
fn coax_p_pr_shadow(p_p: vec3f, p_n: vec3f, p_outside: f32, p_b: i32) -> f32 {
  let l_p: vec3f = p_p;
  let l_n: vec3f = p_n;
  let l_outside: f32 = p_outside;
  let l_b: i32 = p_b;
  var l_size: f32;
  var l_o: i32;
  var l_reach: f32;
  var l_texel: f32;
  var l_q: vec4f;
  var l_edge: f32;
  var l_uv: vec2f;
  var l_taps: i32;
  var l_lit: f32;
  var l_i: i32;
  var l_a: f32;
  var l_d: f32;
  l_size = f32(textureDimensions(shadow_map).x);
  l_o = (l_b + 52i);
  l_reach = (0x1.000000p0f / coax_p_fmax(sqrt((((coax_at_u(i32(l_o)) * coax_at_u(i32(l_o))) + (coax_at_u(i32((l_o + 1i))) * coax_at_u(i32((l_o + 1i))))) + (coax_at_u(i32((l_o + 2i))) * coax_at_u(i32((l_o + 2i)))))), 0x1.0c6f7ap-20f));
  l_texel = ((0x1.000000p1f * l_reach) / l_size);
  l_q = coax_p_pr_light_clip(coax_p_add3(l_p, coax_p_scale3(l_n, ((l_texel * 0x1.99999ap0f) + 0x1.47ae14p-6f))), l_b);
  l_edge = coax_p_fmax(abs(l_q.x), abs(l_q.y));
  if ((((l_edge > 0x1.fd70a4p-1f) || (l_q.z < 0.0f)) || (l_q.z > 0x1.000000p0f)))
  {
    return l_outside;
  }
  l_uv = coax_p_v2(((l_q.x * 0x1.000000p-1f) + 0x1.000000p-1f), (0x1.000000p-1f - (l_q.y * 0x1.000000p-1f)));
  l_taps = select(9i, 4i, (coax_at_u(i32(((l_b + 24i) + 3i))) > 0x1.000000p-1f));
  l_lit = 0.0f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 9i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      if ((l_i < l_taps))
      {
        l_a = (((f32(l_i) * 0x1.3331fcp1f) + (l_p.x * 0x1.8cccccp1f)) + (l_p.z * 0x1.b33334p0f));
        l_d = ((sqrt(((f32(l_i) + 0x1.000000p-1f) / f32(l_taps))) * 0x1.99999ap0f) / l_size);
        l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((l_uv.x + (cos(l_a) * l_d)), (l_uv.y + (sin(l_a) * l_d))), (l_q.z - 0x1.4f8b58p-15f)));
      }
    }
  }
  l_lit = (l_lit / f32(l_taps));
  return coax_p_mix(l_lit, l_outside, coax_p_smoothstep(0x1.ccccccp-1f, 0x1.fd70a4p-1f, l_edge));
}
fn coax_p_sb_p(p_i: i32) -> vec3f {
  let l_i: i32 = p_i;
  return coax_p_v3(coax_at_u(i32((1440i + l_i))), coax_at_u(i32(((1440i + l_i) + 1i))), coax_at_u(i32(((1440i + l_i) + 2i))));
}
fn coax_p_sb_sub(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_pr_field(p_q: vec3f, p_n: vec3f, p_b: i32) -> f32 {
  let l_q: vec3f = p_q;
  let l_n: vec3f = p_n;
  let l_b: i32 = p_b;
  var l_nx: i32;
  var l_nz: i32;
  var l_cell: f32;
  var l_x: f32;
  var l_z: f32;
  var l_i: i32;
  var l_j: i32;
  var l_tx: f32;
  var l_tz: f32;
  var l_a: vec2f;
  var l_c: vec2f;
  var l_d: vec2f;
  var l_e: vec2f;
  var l_open: f32;
  var l_top: f32;
  var l_over: f32;
  l_nx = i32(coax_at_u(i32((l_b + 48i))));
  l_nz = i32(coax_at_u(i32(((l_b + 48i) + 1i))));
  if (((l_nx < 2i) || (l_nz < 2i)))
  {
    return 0x1.000000p0f;
  }
  l_cell = coax_at_u(i32(((l_b + 44i) + 2i)));
  l_x = ((((l_q.x + ((l_n.x * l_cell) * 0x1.800000p-1f)) - coax_at_u(i32((l_b + 44i)))) / l_cell) - 0x1.000000p-1f);
  l_z = ((((l_q.z + ((l_n.z * l_cell) * 0x1.800000p-1f)) - coax_at_u(i32(((l_b + 44i) + 1i)))) / l_cell) - 0x1.000000p-1f);
  l_i = i32(coax_p_wg_clamp_i(i32(floor(l_x)), 0i, i32((l_nx - 2i))));
  l_j = i32(coax_p_wg_clamp_i(i32(floor(l_z)), 0i, i32((l_nz - 2i))));
  l_tx = coax_p_saturate((l_x - f32(l_i)));
  l_tz = coax_p_saturate((l_z - f32(l_j)));
  l_a = coax_p_v2(field[(((l_j * l_nx) + l_i) * 2i)], field[((((l_j * l_nx) + l_i) * 2i) + 1i)]);
  l_c = coax_p_v2(field[((((l_j * l_nx) + l_i) + 1i) * 2i)], field[(((((l_j * l_nx) + l_i) + 1i) * 2i) + 1i)]);
  l_d = coax_p_v2(field[((((l_j + 1i) * l_nx) + l_i) * 2i)], field[(((((l_j + 1i) * l_nx) + l_i) * 2i) + 1i)]);
  l_e = coax_p_v2(field[(((((l_j + 1i) * l_nx) + l_i) + 1i) * 2i)], field[((((((l_j + 1i) * l_nx) + l_i) + 1i) * 2i) + 1i)]);
  l_open = coax_p_mix(coax_p_mix(l_a.x, l_c.x, l_tx), coax_p_mix(l_d.x, l_e.x, l_tx), l_tz);
  l_top = coax_p_mix(coax_p_mix(l_a.y, l_c.y, l_tx), coax_p_mix(l_d.y, l_e.y, l_tx), l_tz);
  l_over = coax_p_fmax((l_q.y - (coax_at_u(i32(((l_b + 44i) + 3i))) + l_top)), 0.0f);
  return coax_p_mix(l_open, 0x1.000000p0f, coax_p_saturate((l_over / coax_p_fmax(coax_at_u(i32(((l_b + 48i) + 2i))), 0x1.000000p0f))));
}
fn coax_p_pr_environment(p_n: vec3f, p_toward: vec3f, p_rough: f32, p_b: i32) -> vec3f {
  let l_n: vec3f = p_n;
  let l_toward: vec3f = p_toward;
  let l_rough: f32 = p_rough;
  let l_b: i32 = p_b;
  var l_r: vec3f;
  var l_sharp: vec3f;
  l_r = coax_p_reflect3(coax_p_scale3(l_toward, (0.0f - 0x1.000000p0f)), l_n);
  l_r = coax_p_normalize3(coax_p_mix3(l_r, l_n, (l_rough * l_rough)));
  l_sharp = coax_p_pr_sky(l_r, l_b);
  return coax_p_mix3(l_sharp, coax_p_pr_sky_light(l_r, l_b), coax_p_saturate((l_rough * 0x1.400000p0f)));
}
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  let l_i: vec3f = p_i;
  let l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_i))));
}
fn coax_p_slab_photo_mirror(p_ph: vec3f, p_rh: vec3f, p_sky: vec3f) -> vec3f {
  let l_ph: vec3f = p_ph;
  let l_rh: vec3f = p_rh;
  let l_sky: vec3f = p_sky;
  var l_b: i32;
  var l_hit: coax_t19;
  var l_w: vec3f;
  var l_pt: coax_t24;
  var l_nw: vec3f;
  var l_light: vec3f;
  var l_emit: vec3f;
  var l_c: vec3f;
  var l_s: coax_t26;
  var l_t: f32;
  l_b = 15872i;
  l_hit = coax_p_slab_scene(l_ph, l_rh, true, 0x1.770000p10f);
  if ((l_hit.x >= 0x1.770000p10f))
  {
    return l_sky;
  }
  l_w = coax_p_sb_add(l_ph, coax_p_sb_mul(l_rh, l_hit.x));
  l_pt = coax_p_slab_paint(l_hit, l_w, l_rh);
  l_nw = coax_p_sb_dir_to_world(l_hit.y);
  l_light = coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_b + 20i)), coax_p_fmax(dot(l_nw, coax_p_pr_at3((l_b + 16i))), 0.0f)), coax_p_pr_sky_light(l_nw, l_b));
  l_emit = select(coax_p_pr_decode(l_pt.y), coax_p_v3(0.0f, 0.0f, 0.0f), (coax_p_wg_i32_f((l_hit.z + 0x1.000000p-1f)) == 5i));
  l_c = coax_p_add3(coax_p_mul3(coax_p_pr_decode(l_pt.x), l_light), coax_p_scale3(l_emit, (0x1.000000p0f / coax_p_fmax(coax_at_u(i32(((l_b + 20i) + 3i))), 0x1.0624dep-10f))));
  if ((coax_at_u(i32((l_b + 112i))) > 0x1.0624dep-10f))
  {
    l_s = coax_t26();
    l_s.x = coax_p_pr_decode(l_pt.x);
    l_s.w = l_hit.y;
    l_s.y = 0x1.000000p0f;
    l_s.z = 0.0f;
    l_c = coax_p_add3(l_c, coax_p_pr_lamps(l_w, l_hit.y, coax_p_sb_mul(l_rh, (-0x1.000000p0f)), l_s, l_b));
  }
  l_t = coax_p_pr_air_through(l_hit.x, coax_at_u(i32(((l_b + 0i) + 1i))), l_rh.y, l_b);
  return coax_p_mix3(l_sky, l_c, l_t);
}
fn coax_p_pr_lamps(p_q: vec3f, p_n: vec3f, p_toward: vec3f, p_s: coax_t26, p_b: i32) -> vec3f {
  let l_q: vec3f = p_q;
  let l_n: vec3f = p_n;
  let l_toward: vec3f = p_toward;
  let l_s: coax_t26 = p_s;
  let l_b: i32 = p_b;
  var l_level: f32;
  var l_out: vec3f;
  var l_nx: i32;
  var l_nz: i32;
  var l_cell: f32;
  var l_base: i32;
  var l_x: f32;
  var l_z: f32;
  var l_i: i32;
  var l_j: i32;
  var l_tx: f32;
  var l_tz: f32;
  var l_e: vec4f;
  var l_over: f32;
  var l_under: f32;
  var l_inside: f32;
  var l_share: f32;
  var l_count: i32;
  var l_f0: vec3f;
  var l_rough: f32;
  var l_k: i32;
  var l_row: i32;
  var l_to: vec3f;
  var l_d2: f32;
  var l_fade: f32;
  l_level = coax_at_u(i32((l_b + 112i)));
  if ((l_level <= 0x1.0624dep-10f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_out = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_nx = i32(coax_at_u(i32((l_b + 48i))));
  l_nz = i32(coax_at_u(i32(((l_b + 48i) + 1i))));
  if ((((coax_at_u(i32(((l_b + 112i) + 3i))) > 0x1.000000p-1f) && (l_nx >= 2i)) && (l_nz >= 2i)))
  {
    l_cell = coax_at_u(i32(((l_b + 44i) + 2i)));
    l_base = ((l_nx * l_nz) * 2i);
    l_x = ((((l_q.x + ((l_n.x * l_cell) * 0x1.333334p-1f)) - coax_at_u(i32((l_b + 44i)))) / l_cell) - 0x1.000000p-1f);
    l_z = ((((l_q.z + ((l_n.z * l_cell) * 0x1.333334p-1f)) - coax_at_u(i32(((l_b + 44i) + 1i)))) / l_cell) - 0x1.000000p-1f);
    l_i = i32(coax_p_wg_clamp_i(i32(floor(l_x)), 0i, i32((l_nx - 2i))));
    l_j = i32(coax_p_wg_clamp_i(i32(floor(l_z)), 0i, i32((l_nz - 2i))));
    l_tx = coax_p_saturate((l_x - f32(l_i)));
    l_tz = coax_p_saturate((l_z - f32(l_j)));
    l_e = coax_p_mix4(coax_p_mix4(coax_p_wg_at4_1((l_base + (((l_j * l_nx) + l_i) * 4i))), coax_p_wg_at4_1((l_base + ((((l_j * l_nx) + l_i) + 1i) * 4i))), l_tx), coax_p_mix4(coax_p_wg_at4_1((l_base + ((((l_j + 1i) * l_nx) + l_i) * 4i))), coax_p_wg_at4_1((l_base + (((((l_j + 1i) * l_nx) + l_i) + 1i) * 4i))), l_tx), l_tz);
    l_over = (coax_p_fmax((l_q.y - coax_at_u(i32(((l_b + 44i) + 3i)))), 0.0f) / coax_p_fmax(l_e.w, 0x1.000000p0f));
    l_under = coax_p_saturate(((coax_at_u(i32(((l_b + 44i) + 3i))) - l_q.y) * 0x1.000000p-1f));
    l_inside = (((coax_p_stepf((-0x1.000000p-1f), l_x) * coax_p_stepf(l_x, (f32(l_nx) - 0x1.000000p-1f))) * coax_p_stepf((-0x1.000000p-1f), l_z)) * coax_p_stepf(l_z, (f32(l_nz) - 0x1.000000p-1f)));
    l_share = ((((coax_p_mix(0x1.000000p-1f, 0x1.000000p0f, coax_p_saturate(l_n.y)) * coax_p_saturate((0x1.000000p0f + (l_n.y * 0x1.000000p1f)))) * (0x1.000000p0f - l_under)) * l_inside) / (0x1.000000p0f + (l_over * l_over)));
    l_out = coax_p_scale3(coax_p_mul3(coax_p_scale3(l_s.x, (0x1.000000p0f - l_s.z)), coax_p_v3(l_e.x, l_e.y, l_e.z)), l_share);
  }
  l_count = i32((coax_at_u(i32(((l_b + 112i) + 2i))) + 0x1.000000p-1f));
  if (((l_count > 0i) && (l_s.y < 0x1.000000p-1f)))
  {
    l_f0 = coax_p_cm_f0(l_s.x, l_s.z);
    l_rough = coax_p_fmax(l_s.y, 0x1.47ae14p-3f);
    {
      let coax_from: i32 = 0i;
      let coax_to: i32 = 12i;
      for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
      {
        if ((l_k < l_count))
        {
          l_row = ((64i + l_k) * 8i);
          l_to = coax_p_sub3(coax_p_v3(materials[l_row], materials[(l_row + 1i)], materials[(l_row + 2i)]), l_q);
          l_d2 = coax_p_fmax(dot(l_to, l_to), 0x1.47ae14p-7f);
          l_fade = coax_p_saturate((0x1.000000p0f - (l_d2 / coax_p_fmax((materials[(l_row + 3i)] * materials[(l_row + 3i)]), 0x1.000000p0f))));
          l_out = coax_p_add3(l_out, coax_p_scale3(coax_p_mul3(coax_p_cm_ggx(l_f0, l_rough, l_n, coax_p_scale3(l_to, (0x1.000000p0f / sqrt(l_d2))), l_toward), coax_p_v3(materials[(l_row + 4i)], materials[(l_row + 5i)], materials[(l_row + 6i)])), ((l_fade * l_fade) / (l_d2 + 0x1.000000p0f))));
        }
      }
    }
  }
  return coax_p_scale3(l_out, l_level);
}
fn coax_p_pr_lit(p_s: coax_t26, p_toward: vec3f, p_sun_reach: f32, p_open: f32, p_mirrored: vec3f, p_b: i32) -> vec3f {
  let l_s: coax_t26 = p_s;
  let l_toward: vec3f = p_toward;
  let l_sun_reach: f32 = p_sun_reach;
  let l_open: f32 = p_open;
  let l_mirrored: vec3f = p_mirrored;
  let l_b: i32 = p_b;
  var l_n: vec3f;
  var l_sun: vec3f;
  var l_light: vec3f;
  var l_f0: vec3f;
  var l_rough: f32;
  var l_nv: f32;
  var l_ndl: f32;
  var l_wrap: f32;
  var l_env: vec2f;
  var l_spec_share: vec3f;
  var l_kept: vec3f;
  var l_diffuse: vec3f;
  var l_glint: vec3f;
  var l_seen: f32;
  l_n = l_s.w;
  l_sun = coax_p_pr_at3((l_b + 16i));
  l_light = coax_p_scale3(coax_p_pr_at3((l_b + 20i)), l_sun_reach);
  l_f0 = coax_p_cm_f0(l_s.x, l_s.z);
  l_rough = coax_p_clampf(l_s.y, 0x1.eb851ep-6f, 0x1.000000p0f);
  l_nv = coax_p_fmax(dot(l_n, l_toward), 0x1.a36e2ep-14f);
  l_ndl = dot(l_n, l_sun);
  l_wrap = ((coax_p_fmax(l_ndl, 0.0f) + ((l_s.f5 * coax_p_fmax((0.0f - l_ndl), 0.0f)) * 0x1.333334p-1f)) + (l_s.f5 * 0x1.333334p-3f));
  l_env = coax_p_pbr_environment(l_rough, l_nv);
  l_spec_share = coax_p_v3(coax_p_saturate(((l_f0.x * l_env.x) + l_env.y)), coax_p_saturate(((l_f0.y * l_env.x) + l_env.y)), coax_p_saturate(((l_f0.z * l_env.x) + l_env.y)));
  l_kept = coax_p_scale3(coax_p_v3((0x1.000000p0f - l_spec_share.x), (0x1.000000p0f - l_spec_share.y), (0x1.000000p0f - l_spec_share.z)), (0x1.000000p0f - l_s.z));
  l_diffuse = coax_p_mul3(coax_p_mul3(l_s.x, l_kept), coax_p_add3(coax_p_scale3(l_light, l_wrap), coax_p_scale3(coax_p_pr_sky_light(l_n, l_b), l_open)));
  l_glint = coax_p_mul3(coax_p_cm_ggx(l_f0, l_rough, l_n, l_sun, l_toward), l_light);
  l_seen = coax_p_mix(0x1.000000p0f, l_open, coax_p_saturate((0x1.333334p-2f + l_rough)));
  return coax_p_add3(coax_p_add3(l_diffuse, l_glint), coax_p_add3(coax_p_scale3(coax_p_mul3(l_mirrored, l_spec_share), l_seen), l_s.f4));
}
fn coax_p_pr_air(p_c: vec3f, p_w: vec3f, p_b: i32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_w: vec3f = p_w;
  let l_b: i32 = p_b;
  var l_to: vec3f;
  var l_d: f32;
  var l_rd: vec3f;
  var l_t: f32;
  l_to = coax_p_sub3(l_w, coax_p_pr_at3((l_b + 0i)));
  l_d = coax_p_length3(l_to);
  l_rd = coax_p_scale3(l_to, (0x1.000000p0f / coax_p_fmax(l_d, 0x1.a36e2ep-14f)));
  l_t = coax_p_pr_air_through(l_d, coax_at_u(i32(((l_b + 0i) + 1i))), l_rd.y, l_b);
  return coax_p_add3(coax_p_scale3(l_c, l_t), coax_p_scale3(coax_p_add3(coax_p_pr_under_cloud(coax_p_pr_air_far(l_rd, coax_p_pr_at3((l_b + 16i)), coax_at_u(i32((l_b + 51i)))), l_b), coax_p_pr_night(coax_at_u(i32(((l_b + 16i) + 3i))))), (0x1.000000p0f - l_t)));
}
fn coax_p_pr_out(p_c: vec3f, p_b: i32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_b: i32 = p_b;
  var l_e: f32;
  var l_w: f32;
  var l_k: f32;
  var l_t: vec3f;
  l_e = coax_at_u(i32(((l_b + 20i) + 3i)));
  l_w = coax_at_u(i32((l_b + 71i)));
  l_k = (l_e / (0x1.000000p0f + (0x1.1f8a0ap-3f * l_w)));
  l_t = coax_p_film_photo(coax_p_film_rgb(((coax_p_fmax(l_c.x, 0.0f) * l_k) * (0x1.000000p0f + l_w)), (coax_p_fmax(l_c.y, 0.0f) * l_k), ((coax_p_fmax(l_c.z, 0.0f) * l_k) * (0x1.000000p0f - l_w))));
  return coax_p_pr_encode(coax_p_v3(coax_p_saturate(l_t.x), coax_p_saturate(l_t.y), coax_p_saturate(l_t.z)));
}
fn coax_p_pr_sun_share(p_n: vec3f, p_sun_reach: f32, p_open: f32, p_b: i32) -> f32 {
  let l_n: vec3f = p_n;
  let l_sun_reach: f32 = p_sun_reach;
  let l_open: f32 = p_open;
  let l_b: i32 = p_b;
  var l_sl: vec3f;
  var l_direct: f32;
  var l_sky: f32;
  l_sl = coax_p_pr_at3((l_b + 20i));
  l_direct = ((coax_p_pr_lum(l_sl) * l_sun_reach) * coax_p_fmax(dot(l_n, coax_p_pr_at3((l_b + 16i))), 0.0f));
  l_sky = (coax_p_pr_lum(coax_p_pr_sky_light(l_n, l_b)) * l_open);
  return coax_p_saturate((l_direct / coax_p_fmax((l_direct + l_sky), 0x1.0c6f7ap-20f)));
}
fn coax_p_pr_lit_out(p_c: vec3f, p_n: vec3f, p_share: f32) -> coax_t21 {
  let l_c: vec3f = p_c;
  let l_n: vec3f = p_n;
  let l_share: f32 = p_share;
  return coax_t21(coax_p_v4(l_c.x, l_c.y, l_c.z, 0.0f), coax_p_v4(l_n.x, l_n.y, l_n.z, l_share));
}
fn coax_p_pr_post(p_light: vec4f, p_aux: vec4f, p_x: i32, p_y: i32, p_b: i32) -> vec3f {
  let l_light: vec4f = p_light;
  let l_aux: vec4f = p_aux;
  let l_x: i32 = p_x;
  let l_y: i32 = p_y;
  let l_b: i32 = p_b;
  var l_c: vec3f;
  var l_k: f32;
  l_c = coax_p_v3(l_light.x, l_light.y, l_light.z);
  if ((l_aux.w < 0.0f))
  {
    return l_c;
  }
  l_k = coax_p_pr_contact(l_x, l_y, coax_p_v3(l_aux.x, l_aux.y, l_aux.z), l_aux.w, l_b);
  return coax_p_pr_encode(coax_p_scale3(coax_p_pr_decode(l_c), l_k));
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
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t29 {
  let l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
}
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t29 {
  let l_x: i32 = p_x;
  let l_y: i32 = p_y;
  return coax_t29(l_x, l_y);
}
fn coax_p_wg_add_i2i2(p_a: coax_t29, p_b: coax_t29) -> coax_t29 {
  let l_a: coax_t29 = p_a;
  let l_b: coax_t29 = p_b;
  return coax_p_i2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_make_i2_i(p_p0: i32) -> coax_t29 {
  let l_p0: i32 = p_p0;
  return coax_p_i2(l_p0, l_p0);
}
fn coax_p_wg_band_i2i2(p_a: coax_t29, p_b: coax_t29) -> coax_t29 {
  let l_a: coax_t29 = p_a;
  let l_b: coax_t29 = p_b;
  return coax_p_i2((l_a.x & l_b.x), (l_a.y & l_b.y));
}
fn coax_p_wg_cvt_u2i2(p_p: coax_t29) -> coax_t30 {
  let l_p: coax_t29 = p_p;
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
fn coax_p_sw_calm_at(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  return (coax_p_sw_calm(coax_p_sw_frame(l_p))).x;
}
fn coax_p_pr_placed_in(p_p: vec3f, p_m: coax_t13, p_time: f32, p_wind: vec4f) -> vec3f {
  let l_p: vec3f = p_p;
  let l_m: coax_t13 = p_m;
  let l_time: f32 = p_time;
  let l_wind: vec4f = p_wind;
  var l_s: vec3f;
  var l_l: vec3f;
  var l_c: f32;
  var l_sn: f32;
  var l_w: vec3f;
  var l_h2: f32;
  var l_lean: f32;
  var l_own: f32;
  var l_gust: f32;
  var l_along: f32;
  var l_across: f32;
  l_s = coax_p_pr_sizes(l_m);
  l_l = coax_p_v3((l_p.x * l_s.x), (l_p.y * l_s.y), (l_p.z * l_s.z));
  if (((l_m.y.y > 0x1.000000p-1f) && (l_m.y.y < 0x1.800000p0f)))
  {
    l_l = coax_p_v3(l_l.x, (l_l.y + (0x1.000000p-2f * sin(((l_time * 0x1.99999ap-1f) + l_m.y.z)))), l_l.z);
  }
  l_l = coax_p_pr_leant(l_l, l_m);
  l_c = cos(l_m.x.w);
  l_sn = sin(l_m.x.w);
  l_w = coax_p_v3((((l_l.x * l_c) + (l_l.z * l_sn)) + l_m.x.x), (l_l.y + l_m.x.y), ((((0.0f - l_l.x) * l_sn) + (l_l.z * l_c)) + l_m.x.z));
  if (((l_m.y.y > 0x1.800000p0f) && (l_m.y.y < 0x1.400000p1f)))
  {
    l_h2 = (l_l.y * l_l.y);
    l_lean = coax_p_fmin((((l_h2 * 0x1.4f8b58p-15f) * l_wind.z) * l_wind.z), (l_l.y * 0x1.333334p-2f));
    l_own = (sin(((l_time * 0x1.e66666p0f) + l_m.y.z)) + (0x1.666666p-2f * sin(((l_time * 0x1.8cccccp1f) + (l_m.y.z * 0x1.b33334p0f)))));
    l_gust = sin((((((l_m.x.x * l_wind.x) + (l_m.x.z * l_wind.y)) - (l_time * coax_p_fmax(l_wind.z, 0x1.000000p1f))) * 0x1.1eb852p-3f) + (0x1.333334p-1f * sin(((l_time * 0x1.d70a3ep-3f) + l_m.y.z)))));
    l_along = ((l_lean * (0x1.000000p0f + (l_wind.w * ((0x1.666666p-1f * l_gust) + (0x1.333334p-2f * l_own))))) + ((l_h2 * 0x1.3a92a4p-12f) * l_own));
    l_across = (((l_h2 * 0x1.a36e2ep-13f) + ((l_lean * 0x1.000000p-2f) * l_wind.w)) * sin(((l_time * 0x1.4cccccp0f) + (l_m.y.z * 0x1.266666p1f))));
    l_w = coax_p_v3(((l_w.x + (l_wind.x * l_along)) - (l_wind.y * l_across)), l_w.y, ((l_w.z + (l_wind.y * l_along)) + (l_wind.x * l_across)));
  }
  return l_w;
}
fn coax_p_pr_posed(p_p: vec3f, p_b: i32) -> vec3f {
  let l_p: vec3f = p_p;
  let l_b: i32 = p_b;
  var l_o: i32;
  l_o = (l_b + 32i);
  return coax_p_v3(((((coax_at_u(i32(l_o)) * l_p.x) + (coax_at_u(i32((l_o + 1i))) * l_p.y)) + (coax_at_u(i32((l_o + 2i))) * l_p.z)) + coax_at_u(i32((l_o + 3i)))), ((((coax_at_u(i32((l_o + 4i))) * l_p.x) + (coax_at_u(i32((l_o + 5i))) * l_p.y)) + (coax_at_u(i32((l_o + 6i))) * l_p.z)) + coax_at_u(i32((l_o + 7i)))), ((((coax_at_u(i32((l_o + 8i))) * l_p.x) + (coax_at_u(i32((l_o + 9i))) * l_p.y)) + (coax_at_u(i32((l_o + 10i))) * l_p.z)) + coax_at_u(i32((l_o + 11i)))));
}
fn coax_p_pr_kept(p_w: vec3f, p_m: coax_t13, p_b: i32) -> vec3f {
  let l_w: vec3f = p_w;
  let l_m: coax_t13 = p_m;
  let l_b: i32 = p_b;
  var l_near: f32;
  var l_place: vec3f;
  var l_k: f32;
  l_near = coax_p_pr_near_of(l_m);
  if ((l_near <= 0.0f))
  {
    return l_w;
  }
  l_place = coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z);
  if ((l_m.y.w < 0x1.000000p-1f))
  {
    l_place = coax_p_pr_posed(l_place, l_b);
  }
  l_k = coax_p_pr_near_keep(l_near, coax_p_length3(coax_p_sub3(l_place, coax_p_pr_at3((l_b + 0i)))), (coax_at_u(i32(((l_b + 24i) + 3i))) > 0x1.000000p-1f));
  return coax_p_add3(l_place, coax_p_scale3(coax_p_sub3(l_w, l_place), l_k));
}
fn coax_p_pr_light_clip(p_w: vec3f, p_b: i32) -> vec4f {
  let l_w: vec3f = p_w;
  let l_b: i32 = p_b;
  var l_p: vec4f;
  var l_o: i32;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  l_o = (l_b + 52i);
  return coax_p_v4(dot(coax_p_wg_at4(l_o), l_p), dot(coax_p_wg_at4((l_o + 4i)), l_p), dot(coax_p_wg_at4((l_o + 8i)), l_p), dot(coax_p_wg_at4((l_o + 12i)), l_p));
}
fn coax_p_pr_sizes(p_m: coax_t13) -> vec3f {
  let l_m: coax_t13 = p_m;
  if ((((l_m.w.x == 0.0f) && (l_m.w.y == 0.0f)) && (l_m.w.z == 0.0f)))
  {
    if (((l_m.y.y > 0x1.c00000p1f) && (l_m.y.y < 0x1.200000p2f)))
    {
      return coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
    }
    return coax_p_v3(l_m.y.x, l_m.y.x, l_m.y.x);
  }
  return coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z);
}
fn coax_p_pr_lamp_grow(p_dist: f32, p_radius: f32, p_tangent: f32) -> f32 {
  let l_dist: f32 = p_dist;
  let l_radius: f32 = p_radius;
  let l_tangent: f32 = p_tangent;
  return coax_p_fmax(0x1.000000p0f, (((l_dist * l_tangent) * 0x1.61e4f8p-9f) / coax_p_fmax(l_radius, 0x1.0624dep-10f)));
}
fn coax_p_pr_leant(p_l: vec3f, p_m: coax_t13) -> vec3f {
  let l_l: vec3f = p_l;
  let l_m: coax_t13 = p_m;
  var l_cr: f32;
  var l_sr: f32;
  var l_cp: f32;
  var l_sp: f32;
  var l_y: f32;
  var l_z: f32;
  if (((l_m.y.y < 0x1.c00000p1f) || (l_m.y.y > 0x1.200000p2f)))
  {
    return l_l;
  }
  l_cr = cos(l_m.y.x);
  l_sr = sin(l_m.y.x);
  l_cp = cos(l_m.y.z);
  l_sp = sin(l_m.y.z);
  l_y = ((l_l.y * l_cr) - (l_l.z * l_sr));
  l_z = ((l_l.y * l_sr) + (l_l.z * l_cr));
  return coax_p_v3(((l_l.x * l_cp) - (l_y * l_sp)), ((l_l.x * l_sp) + (l_y * l_cp)), l_z);
}
fn coax_p_pr_fold(p_dist: f32, p_from: f32, p_tangent: f32) -> f32 {
  let l_dist: f32 = p_dist;
  let l_from: f32 = p_from;
  let l_tangent: f32 = p_tangent;
  return coax_p_saturate(((((l_dist * l_tangent) * 0x1.333334p1f) - l_from) / (l_from * 0x1.333334p-1f)));
}
fn coax_p_fract(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_pr_posed_dir(p_d: vec3f, p_b: i32) -> vec3f {
  let l_d: vec3f = p_d;
  let l_b: i32 = p_b;
  var l_o: i32;
  l_o = (l_b + 32i);
  return coax_p_v3((((coax_at_u(i32(l_o)) * l_d.x) + (coax_at_u(i32((l_o + 1i))) * l_d.y)) + (coax_at_u(i32((l_o + 2i))) * l_d.z)), (((coax_at_u(i32((l_o + 4i))) * l_d.x) + (coax_at_u(i32((l_o + 5i))) * l_d.y)) + (coax_at_u(i32((l_o + 6i))) * l_d.z)), (((coax_at_u(i32((l_o + 8i))) * l_d.x) + (coax_at_u(i32((l_o + 9i))) * l_d.y)) + (coax_at_u(i32((l_o + 10i))) * l_d.z)));
}
fn coax_p_pr_clip(p_w: vec3f, p_b: i32) -> vec4f {
  let l_w: vec3f = p_w;
  let l_b: i32 = p_b;
  var l_rel: vec3f;
  var l_z: f32;
  var l_t: f32;
  l_rel = coax_p_sub3(l_w, coax_p_pr_at3((l_b + 0i)));
  l_z = dot(l_rel, coax_p_pr_at3((l_b + 4i)));
  l_t = coax_at_u(i32(((l_b + 0i) + 3i)));
  return coax_p_v4((dot(l_rel, coax_p_pr_at3((l_b + 8i))) / (l_t * coax_at_u(i32(((l_b + 8i) + 3i))))), (dot(l_rel, coax_p_pr_at3((l_b + 12i))) / l_t), (l_z - coax_at_u(i32(((l_b + 4i) + 3i)))), l_z);
}
fn coax_p_slab_photo_meshes() -> bool {
  return (coax_at_u(i32(15989i)) > 0x1.000000p-1f);
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
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_sb_dir_to_world(p_q: vec3f) -> vec3f {
  let l_q: vec3f = p_q;
  return coax_p_v3(coax_p_wg_dot3(coax_p_sb_p(16i), l_q), coax_p_wg_dot3(coax_p_sb_p(20i), l_q), coax_p_wg_dot3(coax_p_sb_p(24i), l_q));
}
fn coax_p_slab_photo_lit(p_hit: coax_t19, p_o: vec3f, p_d: vec3f) -> vec3f {
  let l_hit: coax_t19 = p_hit;
  let l_o: vec3f = p_o;
  let l_d: vec3f = p_d;
  return coax_p_slab_photo_pulse(coax_p_pr_out(coax_p_slab_photo_light(l_hit, l_o, l_d), 15872i), l_hit.f4);
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
  return coax_p_slab_sea_under_out(coax_p_sb_add(coax_p_sb_mulv(coax_p_sb_mulv(coax_p_slab_sea_in(l_c), l_lit), l_tr), coax_p_sb_mulv(l_fog, coax_p_v3((0x1.000000p0f - l_tr.x), (0x1.000000p0f - l_tr.y), (0x1.000000p0f - l_tr.z)))), l_eye);
}
fn coax_p_sw_turn(p_d: vec2f) -> vec2f {
  let l_d: vec2f = p_d;
  return coax_p_v2(((coax_at_u(i32(15998i)) * l_d.x) + (coax_at_u(i32(15999i)) * l_d.y)), ((coax_at_u(i32(15998i)) * l_d.y) - (coax_at_u(i32(15999i)) * l_d.x)));
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
fn coax_p_sw_wake(p_q: vec2f) -> vec2f {
  let l_q: vec2f = p_q;
  var l_speed: f32;
  var l_f: vec2f;
  var l_across: f32;
  var l_reach: f32;
  var l_x: f32;
  var l_y: f32;
  var l_w: f32;
  var l_lost: f32;
  var l_swing: f32;
  var l_sway: f32;
  var l_off: f32;
  var l_inside: f32;
  var l_lw: f32;
  var l_wander: f32;
  var l_e: f32;
  l_speed = coax_at_u(i32(16018i));
  if (((coax_at_u(i32(15992i)) < 0x1.000000p-1f) || (l_speed < 0x1.47ae14p-6f)))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_f = coax_p_sw_turn(coax_p_v2(coax_at_u(i32(16016i)), coax_at_u(i32(16017i))));
  l_across = coax_p_fmax(coax_at_u(i32(16019i)), 0x1.000000p0f);
  l_reach = ((coax_at_u(i32(16028i)) * abs(l_f.x)) + (coax_at_u(i32(16029i)) * abs(l_f.y)));
  l_x = (((l_q.x * l_f.x) + (l_q.y * l_f.y)) - l_reach);
  if ((l_x <= 0.0f))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_y = ((l_q.y * l_f.x) - (l_q.x * l_f.y));
  l_w = (l_across * sqrt((0x1.000000p0f + (l_x / l_across))));
  l_lost = ((coax_p_fmin(0x1.000000p0f, sqrt(((0x1.000000p1f * l_across) / l_x))) * exp(((-l_x) / 0x1.770000p12f))) * coax_p_smoothstep(0x1.47ae14p-6f, 0x1.333334p-2f, l_speed));
  l_swing = ((0x1.b33334p-1f * l_across) / 0x1.333334p-3f);
  l_sway = (((0x1.47ae14p-4f * l_w) * coax_p_fmin((l_x / l_across), 0x1.000000p0f)) * sin((0x1.921fb6p2f * ((l_x / l_swing) - (((coax_at_u(i32(15994i)) * 0x1.333334p-3f) * l_speed) / l_across)))));
  l_off = abs((l_y - l_sway));
  l_inside = ((0x1.000000p0f - coax_p_smoothstep((0x1.ae147ap-2f * l_w), (0x1.000000p-1f * l_w), l_off)) * l_lost);
  l_lw = (0x1.200000p3f + (0x1.eb851ep-6f * l_x));
  l_wander = ((coax_p_slab_noise(coax_p_v2((l_x * 0x1.89374cp-8f), (select(0x1.600000p3f, 0x1.800000p1f, (l_y > l_sway)) + (coax_at_u(i32(15994i)) * 0x1.0624dep-8f)))) - 0x1.000000p-1f) * (0x1.eb851ep-4f * l_w));
  l_e = (((l_off - (0x1.000000p-1f * l_w)) - l_wander) / l_lw);
  return coax_p_v2(l_inside, ((exp((-(l_e * l_e))) * l_lost) * coax_p_smoothstep(0.0f, 0x1.e00000p5f, l_x)));
}
fn coax_p_slab_photo_haze(p_c: vec3f, p_d: vec3f, p_dist: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  var l_b: i32;
  var l_t: f32;
  var l_far: vec3f;
  l_b = 15872i;
  l_t = coax_p_pr_air_through(l_dist, coax_at_u(i32(((l_b + 0i) + 1i))), l_d.y, l_b);
  l_far = coax_p_pr_out(coax_p_add3(coax_p_pr_under_cloud(coax_p_pr_air_far(l_d, coax_p_pr_at3((l_b + 16i)), coax_at_u(i32((l_b + 51i)))), l_b), coax_p_pr_night(coax_at_u(i32(((l_b + 16i) + 3i))))), l_b);
  return coax_p_mix3(l_c, l_far, (0x1.000000p0f - l_t));
}
fn coax_p_sb_pow(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return coax_pow_bg_vs(coax_p_fmax(l_x, 0x1.0c6f7ap-20f), l_y);
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
fn coax_p_slab_sea_under_out(p_c: vec3f, p_eye: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_eye: f32 = p_eye;
  var l_top: vec3f;
  var l_round: vec3f;
  var l_k: f32;
  if ((!coax_p_slab_photo()))
  {
    return coax_p_slab_sea_out(l_c);
  }
  l_top = coax_p_slab_sea_daylight(0.0f);
  l_round = coax_p_slab_sea_fog(l_eye, 0.0f);
  l_k = coax_p_clampf(coax_pow_bg_vs(((0x1.000000p-2f * coax_p_pr_lum(l_top)) / coax_p_fmax(coax_p_pr_lum(l_round), 0x1.0c6f7ap-20f)), 0x1.666666p-1f), 0x1.000000p0f, 0x1.400000p5f);
  l_k = coax_p_mix(0x1.000000p0f, l_k, coax_p_smoothstep(0.0f, (-0x1.000000p1f), l_eye));
  return coax_p_slab_photo_show(coax_p_sb_mul(l_c, l_k));
}
fn coax_p_slab_sea_sunray() -> vec3f {
  var l_sun: vec3f;
  var l_sy: f32;
  var l_sh: f32;
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_sy = coax_p_wo_cos_refracted(coax_p_fmax(l_sun.y, 0.0f));
  l_sh = (sqrt(coax_p_fmax((0x1.000000p0f - (l_sy * l_sy)), 0.0f)) / coax_p_fmax(sqrt(((l_sun.x * l_sun.x) + (l_sun.z * l_sun.z))), 0x1.a36e2ep-14f));
  return coax_p_v3((l_sun.x * l_sh), l_sy, (l_sun.z * l_sh));
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
  l_top = coax_p_sb_add(coax_p_sb_mul(coax_p_slab_sea_top_sun(), (coax_p_fmax(coax_at_u(i32(25i)), 0.0f) * (0x1.000000p0f - coax_p_wo_fresnel(coax_p_fmax(coax_at_u(i32(25i)), 0x1.47ae14p-6f))))), coax_p_sb_mul(coax_p_slab_sea_top_skylight(), 0x1.e147aep-1f));
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
  l_ray = coax_p_slab_sea_sunray();
  l_steps = select(5i, 3i, (coax_at_u(i32(5i)) > 0x1.000000p-1f));
  l_acc = 0.0f;
  l_at = 0x1.800000p1f;
  l_i = 0i;
  while ((l_i < l_steps))
  {
    if ((l_at < l_limit))
    {
      l_q = coax_p_sb_add(l_o, coax_p_sb_mul(l_d, l_at));
      if (((l_q.y < 0.0f) && (!coax_p_slab_sea_hull_shades(l_q, l_ray))))
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
fn coax_p_slab_photo_sky(p_d: vec3f) -> vec3f {
  let l_d: vec3f = p_d;
  var l_b: i32;
  var l_soft: f32;
  var l_c: vec3f;
  l_b = 15872i;
  l_soft = ((coax_at_u(i32(((l_b + 0i) + 3i))) * 0x1.000000p1f) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f));
  l_c = coax_p_pr_dome(l_d, l_soft, l_b);
  return coax_p_pr_out(l_c, l_b);
}
fn coax_p_slab_land_within(p_toward: f32) -> f32 {
  let l_toward: f32 = p_toward;
  var l_edge: f32;
  l_edge = coax_at_u(i32(1499i));
  if ((l_edge <= (-0x1.ff7ceep-1f)))
  {
    return 0x1.000000p0f;
  }
  return coax_p_smoothstep(l_edge, ((l_edge + ((0x1.000000p0f - l_edge) * 0x1.333334p-2f)) + 0x1.a36e2ep-14f), l_toward);
}
fn coax_p_slab_land_colour() -> vec3f {
  return coax_p_sb_mulv(coax_p_sb_p(12i), coax_p_v3(0x1.5c28f6p-2f, 0x1.ae147ap-2f, 0x1.51eb86p-2f));
}
fn coax_p_slab_land_far(p_land: vec3f, p_sky: vec3f, p_far: f32, p_low: f32) -> vec3f {
  let l_land: vec3f = p_land;
  let l_sky: vec3f = p_sky;
  let l_far: f32 = p_far;
  let l_low: f32 = p_low;
  var l_grey: f32;
  var l_l: vec3f;
  var l_k: f32;
  l_grey = (((l_land.x * 0x1.333334p-2f) + (l_land.y * 0x1.2e147ap-1f)) + (l_land.z * 0x1.c28f5cp-4f));
  l_l = coax_p_sb_mix(l_land, coax_p_v3(l_grey, l_grey, l_grey), 0x1.666666p-2f);
  l_k = coax_p_fmin(((0x1.c28f5cp-3f + (0x1.ccccccp-2f * (0x1.000000p0f - exp(((-l_far) / 0x1.194000p13f))))) + (0x1.333334p-3f * l_low)), 0x1.b33334p-1f);
  return coax_p_sb_mix(l_l, l_sky, l_k);
}
fn coax_p_slab_coarse(p_p: vec2f) -> vec4f {
  let l_p: vec2f = p_p;
  var l_q: vec2f;
  var l_f: vec2f;
  var l_i: coax_t29;
  var l_acc: vec4f;
  var l_k: i32;
  var l_o: coax_t29;
  var l_c: coax_t30;
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
fn coax_p_sw_frame(p_p: vec2f) -> vec2f {
  let l_p: vec2f = p_p;
  var l_dx: f32;
  var l_dz: f32;
  l_dx = (l_p.x - coax_at_u(i32(15996i)));
  l_dz = (l_p.y - coax_at_u(i32(15997i)));
  return coax_p_v2(((coax_at_u(i32(15998i)) * l_dx) + (coax_at_u(i32(15999i)) * l_dz)), ((coax_at_u(i32(15998i)) * l_dz) - (coax_at_u(i32(15999i)) * l_dx)));
}
fn coax_p_sw_calm(p_q: vec2f) -> vec2f {
  let l_q: vec2f = p_q;
  var l_a: f32;
  var l_b: f32;
  var l_i: f32;
  var l_j: f32;
  var l_fa: f32;
  var l_fb: f32;
  var l_n: u32;
  var l_row: u32;
  var l_g00: vec2f;
  var l_g10: vec2f;
  var l_g01: vec2f;
  var l_g11: vec2f;
  if ((coax_at_u(i32(15992i)) < 0x1.000000p-1f))
  {
    return coax_p_v2(0x1.000000p0f, 0x1.000000p0f);
  }
  l_a = coax_p_sw_node(coax_at_u(i32(16028i)), l_q.x);
  l_b = coax_p_sw_node(coax_at_u(i32(16029i)), l_q.y);
  l_i = coax_p_fmin(floor(l_a), (0x1.800000p4f - 0x1.000000p1f));
  l_j = coax_p_fmin(floor(l_b), (0x1.800000p4f - 0x1.000000p1f));
  l_fa = (l_a - l_i);
  l_fb = (l_b - l_j);
  l_n = coax_p_wg_u32_f(((l_j * 0x1.800000p4f) + l_i));
  l_row = coax_p_wg_u32_f(0x1.800000p4f);
  l_g00 = coax_p_sw_gains(l_n);
  l_g10 = coax_p_sw_gains((l_n + 1u));
  l_g01 = coax_p_sw_gains((l_n + l_row));
  l_g11 = coax_p_sw_gains(((l_n + l_row) + 1u));
  return coax_p_v2(coax_p_mix(coax_p_mix(l_g00.x, l_g10.x, l_fa), coax_p_mix(l_g01.x, l_g11.x, l_fa), l_fb), coax_p_mix(coax_p_mix(l_g00.y, l_g10.y, l_fa), coax_p_mix(l_g01.y, l_g11.y, l_fa), l_fb));
}
fn coax_p_sw_normal(p_k: i32) -> vec2f {
  let l_k: i32 = p_k;
  if ((l_k == 0i))
  {
    return coax_p_v2(0x1.000000p0f, 0.0f);
  }
  if ((l_k == 1i))
  {
    return coax_p_v2(0.0f, 0x1.000000p0f);
  }
  if ((l_k == 2i))
  {
    return coax_p_v2((-0x1.000000p0f), 0.0f);
  }
  return coax_p_v2(0.0f, (-0x1.000000p0f));
}
fn coax_p_sw_world(p_q: vec2f) -> vec2f {
  let l_q: vec2f = p_q;
  return coax_p_v2(((coax_at_u(i32(15996i)) + (coax_at_u(i32(15998i)) * l_q.x)) - (coax_at_u(i32(15999i)) * l_q.y)), ((coax_at_u(i32(15997i)) + (coax_at_u(i32(15999i)) * l_q.x)) + (coax_at_u(i32(15998i)) * l_q.y)));
}
fn coax_p_slab_sea_fine(p_p: vec2f, p_patch: f32, p_hi: u32, p_scale: f32) -> vec2f {
  let l_p: vec2f = p_p;
  let l_patch: f32 = p_patch;
  let l_hi: u32 = p_hi;
  let l_scale: f32 = p_scale;
  var l_q: vec2f;
  var l_f: vec2f;
  var l_i: coax_t29;
  var l_acc: vec2f;
  var l_k: i32;
  var l_o: coax_t29;
  var l_c: coax_t30;
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
fn coax_p_slab_sea_top_sky(p_r: vec3f, p_cap: f32) -> vec3f {
  let l_r: vec3f = p_r;
  let l_cap: f32 = p_cap;
  var l_sk: vec3f;
  if (coax_p_slab_photo())
  {
    return coax_p_slab_photo_mirror_sky(l_r);
  }
  l_sk = coax_p_slab_sky(l_r);
  return coax_p_slab_sea_lin(coax_p_v3(coax_p_fmin(l_sk.x, l_cap), coax_p_fmin(l_sk.y, l_cap), coax_p_fmin(l_sk.z, l_cap)));
}
fn coax_p_slab_sea_top_sun() -> vec3f {
  if (coax_p_slab_photo())
  {
    return coax_p_slab_photo_sun();
  }
  return coax_p_slab_sea_sun();
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
  var l_lee: vec2f;
  var l_rain: f32;
  var l_n: vec3f;
  var l_v: vec3f;
  var l_sun: vec3f;
  var l_sunl: vec3f;
  var l_skyl: vec3f;
  var l_cv: f32;
  var l_fres: f32;
  var l_r: vec3f;
  var l_col: vec3f;
  var l_ndl: f32;
  var l_into: f32;
  var l_deep: vec3f;
  var l_light: vec3f;
  var l_high: f32;
  var l_crest: f32;
  var l_flat: f32;
  var l_gate: f32;
  var l_fw: vec2f;
  var l_white: f32;
  var l_veil: f32;
  var l_hang: vec3f;
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
  l_lee = coax_p_sw_short(l_p);
  l_slope = coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_c.z, l_c.w), l_keep0), coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(l_slope, coax_p_wg_mul_f2f(coax_p_v2(l_c.z, l_c.w), l_keep0)), l_lee.x));
  l_sigma2 = (l_sigma2 * (l_lee.x * l_lee.x));
  l_rain = coax_at_u(i32(15857i));
  l_slope = coax_p_wg_add_f2f2(l_slope, coax_p_slab_rain_rings(l_p, l_t, l_rain, l_footprint));
  l_sigma2 = (l_sigma2 + coax_p_slab_rain_mss(l_rain));
  l_n = coax_p_wg_normalize3(coax_p_v3((-l_slope.x), 0x1.000000p0f, (-l_slope.y)));
  l_v = coax_p_sb_mul(l_d, (-0x1.000000p0f));
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_sunl = coax_p_slab_sea_top_sun();
  l_skyl = coax_p_slab_sea_top_skylight();
  l_cv = coax_p_fmax(coax_p_wg_dot3(l_n, l_v), 0x1.47ae14p-6f);
  l_fres = coax_p_wo_mean_fresnel(l_cv, l_sigma2);
  l_r = coax_p_wg_reflect3(l_d, l_n);
  l_r = coax_p_wg_normalize3(coax_p_v3(l_r.x, (abs(l_r.y) + (0x1.666666p-2f * sqrt(l_sigma2))), l_r.z));
  l_col = coax_p_sb_mul(coax_p_slab_sea_top_sky(l_r, 0x1.400000p0f), l_fres);
  l_col = coax_p_sb_add(l_col, coax_p_sb_mul(coax_p_v3(0x1.800000p-1f, 0x1.99999ap-1f, 0x1.000000p0f), ((l_fres * coax_at_u(i32(15859i))) * 0x1.666666p-2f)));
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
  l_fw = coax_p_slab_foam_white(l_p, l_t, l_footprint, l_dist, l_wind, coax_at_u(i32(15854i)), l_gate, coax_at_u(i32(15865i)));
  l_white = l_fw.x;
  l_white = (l_white * l_lee.y);
  l_col = coax_p_sb_add(l_col, coax_p_sb_mul(coax_p_sb_mulv(coax_p_wo_through(coax_p_slab_sea_kd(), 0x1.800000p1f), l_light), ((0x1.99999ap-5f * coax_p_smoothstep(0x1.99999ap-4f, 0x1.666666p-1f, (l_fw.y * l_lee.y))) * (0x1.000000p0f - l_fres))));
  l_col = coax_p_sb_mix(l_col, coax_p_slab_sea_foam_lit(l_ndl), l_white);
  l_veil = (coax_at_u(i32(15854i)) * (0x1.000000p0f - exp(((-l_dist) / 0x1.5e0000p9f))));
  l_hang = l_skyl;
  if ((!coax_p_slab_photo()))
  {
    l_hang = coax_p_slab_sea_lin(coax_p_sb_p(12i));
  }
  l_col = coax_p_sb_mix(l_col, coax_p_sb_mul(l_hang, 0x1.ccccccp-1f), (l_veil * 0x1.333334p-1f));
  l_o = coax_p_slab_sea_top_out(coax_p_slab_sea_top_air(l_col, l_d, l_dist));
  return coax_p_v4(l_o.x, l_o.y, l_o.z, l_white);
}
fn coax_p_slab_sea_white() -> vec3f {
  return coax_p_slab_sea_top_out(coax_p_slab_sea_foam_lit(coax_p_fmax(coax_at_u(i32(25i)), 0.0f)));
}
fn coax_p_sw_white_px(p_p: vec2f, p_t: f32, p_fp: f32) -> f32 {
  let l_p: vec2f = p_p;
  let l_t: f32 = p_t;
  let l_fp: f32 = p_fp;
  var l_mode: f32;
  l_mode = coax_at_u(i32(15992i));
  if (((l_mode < 0x1.000000p-1f) || (l_mode > 0x1.800000p0f)))
  {
    return coax_p_sw_white(l_p, l_t);
  }
  return coax_p_sw_foot_white(l_p, l_t, l_fp);
}
fn coax_p_slab_ship_wake(p_p: vec2f, p_t: f32) -> f32 {
  let l_p: vec2f = p_p;
  let l_t: f32 = p_t;
  var l_foam: f32;
  var l_k: i32;
  var l_b: i32;
  var l_speed: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_ln: f32;
  var l_half: f32;
  var l_a: f32;
  var l_y: f32;
  var l_back: f32;
  var l_strong: f32;
  var l_fade: f32;
  var l_arm: f32;
  var l_aw: f32;
  var l_kel: f32;
  var l_side: f32;
  var l_bw: f32;
  var l_bow: f32;
  var l_aft: f32;
  var l_ww: f32;
  var l_wash: f32;
  var l_n: f32;
  var l_f: f32;
  l_foam = 0.0f;
  l_k = 0i;
  while ((l_k < 2i))
  {
    l_b = (16236i + (l_k * 8i));
    l_speed = coax_at_u(i32((l_b + 4i)));
    if ((l_speed > 0x1.333334p-2f))
    {
      l_dx = (l_p.x - coax_at_u(i32(l_b)));
      l_dz = (l_p.y - coax_at_u(i32((l_b + 1i))));
      l_fx = coax_at_u(i32((l_b + 2i)));
      l_fz = coax_at_u(i32((l_b + 3i)));
      l_ln = coax_at_u(i32((l_b + 5i)));
      l_half = (coax_at_u(i32((l_b + 6i))) * 0x1.000000p-1f);
      l_a = ((l_dx * l_fx) + (l_dz * l_fz));
      l_y = abs(((l_dz * l_fx) - (l_dx * l_fz)));
      l_back = ((l_ln * 0x1.000000p-1f) - l_a);
      if ((((l_back > (-0x1.000000p2f)) && (l_back < (l_ln * 0x1.c00000p3f))) && (l_y < (((l_back * 0x1.000000p-1f) + l_half) + 0x1.000000p3f))))
      {
        l_strong = coax_p_smoothstep(0x1.333334p-2f, 0x1.000000p2f, l_speed);
        l_fade = exp(((-coax_p_fmax(l_back, 0.0f)) / (l_ln * 0x1.000000p2f)));
        l_arm = ((l_y - (l_half * 0x1.333334p-1f)) - (l_back * 0x1.6a161ep-2f));
        l_aw = (0x1.800000p0f + (0x1.99999ap-6f * l_back));
        l_kel = ((exp((-((l_arm * l_arm) / (l_aw * l_aw)))) * l_fade) * coax_p_smoothstep(0.0f, (l_ln * 0x1.333334p-2f), l_back));
        l_side = (l_half * coax_p_clampf(((l_back + 0x1.000000p1f) / (l_ln * 0x1.333334p-2f)), 0.0f, 0x1.000000p0f));
        l_bw = ((l_y - l_side) / (0x1.333334p0f + (0x1.99999ap-5f * l_back)));
        l_bow = (exp((-(l_bw * l_bw))) * (0x1.000000p0f - coax_p_smoothstep((l_ln * 0x1.333334p-3f), (l_ln * 0x1.666666p-1f), l_back)));
        l_aft = (l_back - l_ln);
        l_ww = (l_half * (0x1.99999ap-1f + (0x1.eb851ep-6f * coax_p_fmax(l_aft, 0.0f))));
        l_wash = select(0.0f, ((exp((-((l_y * l_y) / (l_ww * l_ww)))) * exp(((-l_aft) / (l_ln * 0x1.800000p1f)))) * coax_p_smoothstep((-0x1.000000p1f), 0x1.800000p2f, l_aft)), (l_aft > 0.0f));
        l_n = ((coax_p_slab_noise(coax_p_v2(((l_p.x * 0x1.70a3d8p-4f) + (l_t * 0x1.99999ap-5f)), (l_p.y * 0x1.70a3d8p-4f))) * 0x1.333334p-1f) + (coax_p_slab_noise(coax_p_v2((l_p.x * 0x1.3d70a4p-2f), ((l_p.y * 0x1.3d70a4p-2f) - (l_t * 0x1.47ae14p-4f)))) * 0x1.99999ap-2f));
        l_f = ((coax_p_fmax(coax_p_fmax((l_kel * 0x1.99999ap-1f), l_bow), (l_wash * 0x1.ccccccp-1f)) * l_strong) * coax_p_smoothstep(0x1.000000p-2f, 0x1.666666p-1f, (l_n + (0x1.000000p-2f * l_bow))));
        l_foam = coax_p_fmax(l_foam, l_f);
      }
    }
    l_k = (l_k + 1i);
  }
  return coax_p_clampf(l_foam, 0.0f, 0x1.ccccccp-1f);
}
fn coax_p_sw_tint(p_col: vec3f, p_p: vec2f, p_foam: vec3f, p_t: f32) -> vec3f {
  let l_col: vec3f = p_col;
  let l_p: vec2f = p_p;
  let l_foam: vec3f = p_foam;
  let l_t: f32 = p_t;
  var l_q: vec2f;
  var l_c: vec3f;
  var l_seen: f32;
  var l_k: i32;
  var l_pl: vec2f;
  var l_wk: vec2f;
  var l_line: f32;
  if ((coax_at_u(i32(15992i)) < 0x1.000000p-1f))
  {
    return l_col;
  }
  l_q = coax_p_sw_frame(l_p);
  l_c = l_col;
  if ((coax_at_u(i32(16020i)) > 0x1.0624dep-10f))
  {
    l_seen = 0.0f;
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_pl = coax_p_sw_plume_at(l_q, l_k);
      l_seen = coax_p_fmax(l_seen, ((l_pl.x * 0x1.800000p-1f) * coax_at_u(i32(16032i))));
      l_k = (l_k + 1i);
    }
    l_c = coax_p_sb_mix(l_c, coax_p_sb_add(coax_p_sb_mul(l_col, 0x1.19999ap-1f), coax_p_v3(0x1.99999ap-4f, 0x1.70a3d8p-2f, 0x1.ae147ap-2f)), coax_p_clampf((l_seen * 0x1.99999ap0f), 0.0f, 0x1.99999ap-1f));
  }
  l_wk = coax_p_sw_wake(l_q);
  l_line = (l_wk.y * coax_p_smoothstep(0x1.666666p-2f, 0x1.800000p-1f, ((coax_p_slab_noise(coax_p_v2((l_q.x * 0x1.581062p-6f), ((l_q.y * 0x1.99999ap-5f) + (l_t * 0x1.47ae14p-6f)))) * 0x1.333334p-1f) + (coax_p_slab_noise(coax_p_v2((l_q.x * 0x1.c28f5cp-4f), (l_q.y * 0x1.5c28f6p-3f))) * 0x1.99999ap-2f))));
  return coax_p_sb_mix(l_c, l_foam, coax_p_clampf((l_line * 0x1.333334p-2f), 0.0f, 0x1.333334p-2f));
}
fn coax_p_slab_band(p_v: f32, p_period: f32, p_share: f32) -> f32 {
  let l_v: f32 = p_v;
  let l_period: f32 = p_period;
  let l_share: f32 = p_share;
  var l_f: f32;
  l_f = coax_p_fract((l_v / l_period));
  return coax_p_stepf(l_f, l_share);
}
fn coax_p_slab_section(p_k: i32, p_w: vec3f) -> coax_t24 {
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
  return coax_t24(coax_p_sb_mul(l_c, 0x1.000000p-2f), coax_p_sb_mul(l_c, 0x1.99999ap-1f));
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
fn coax_p_pr_decode1(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  var l_c: f32;
  l_c = coax_p_fmax(l_x, 0.0f);
  return select(coax_pow_bg_vs(((l_c + 0x1.c28f5cp-5f) / 0x1.0e147ap0f), 0x1.333334p1f), (l_c / 0x1.9d70a4p3f), (l_c <= 0x1.4b5dccp-5f));
}
fn coax_p_wg_clamp_i(p_x: i32, p_low: i32, p_high: i32) -> i32 {
  let l_x: i32 = p_x;
  let l_low: i32 = p_low;
  let l_high: i32 = p_high;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_low), l_high);
}
fn coax_p_pr_face(p_local: vec3f, p_facing: vec3f) -> vec2f {
  let l_local: vec3f = p_local;
  let l_facing: vec3f = p_facing;
  if ((abs(l_facing.y) > 0x1.666666p-1f))
  {
    return coax_p_v2(l_local.x, l_local.z);
  }
  if ((abs(l_facing.x) > abs(l_facing.z)))
  {
    return coax_p_v2(l_local.z, l_local.y);
  }
  return coax_p_v2(l_local.x, l_local.y);
}
fn coax_p_pr_glazing(p_s0: coax_t26, p_f: vec2f, p_ax: f32, p_ay: f32, p_fl: vec3f, p_seed: f32, p_floor0: f32, p_facade: i32, p_full: bool, p_b: i32) -> coax_t26 {
  let l_s0: coax_t26 = p_s0;
  let l_f: vec2f = p_f;
  let l_ax: f32 = p_ax;
  let l_ay: f32 = p_ay;
  let l_fl: vec3f = p_fl;
  let l_seed: f32 = p_seed;
  let l_floor0: f32 = p_floor0;
  let l_facade: i32 = p_facade;
  let l_full: bool = p_full;
  let l_b: i32 = p_b;
  var l_s: coax_t26;
  var l_floor_h: f32;
  var l_bay: f32;
  var l_px: f32;
  var l_py: f32;
  var l_tone: f32;
  var l_k: f32;
  var l_v: f32;
  var l_sp: f32;
  var l_sp_20: f32;
  var l_mullion: f32;
  var l_fin: f32;
  l_s = l_s0;
  if ((abs(l_fl.y) >= 0x1.666666p-1f))
  {
    l_s.x = coax_p_scale3(coax_p_v3(0x1.851eb8p-3f, 0x1.851eb8p-3f, 0x1.7ae148p-3f), (0x1.99999ap-1f + (0x1.99999ap-2f * coax_p_pr_noise2((l_f.x * 0x1.666666p-1f), (l_f.y * 0x1.666666p-1f)))));
    l_s.y = 0x1.ccccccp-1f;
    l_s.z = 0.0f;
    return l_s;
  }
  l_floor_h = select(0x1.f33334p1f, l_floor0, (l_floor0 > 0.0f));
  l_bay = select(select(0x1.800000p0f, 0x1.ccccccp0f, (l_facade == 1i)), 0x1.333334p0f, (l_facade == 2i));
  l_px = floor(((l_f.x / l_bay) + 0x1.000000p-1f));
  l_py = floor((l_f.y / l_floor_h));
  l_tone = coax_p_render_noise2((l_px + (l_seed * 0x1.a00000p3f)), (l_py - (l_seed * 0x1.400000p2f)));
  if (l_full)
  {
    l_k = (coax_p_pr_resolve(l_bay, l_ax) * 0x1.89374cp-7f);
    l_s.w = coax_p_pr_tilt(l_s.w, l_fl, ((coax_p_render_noise2((l_px + 0x1.f00000p4f), (l_py + l_seed)) - 0x1.000000p-1f) * l_k), ((l_tone - 0x1.000000p-1f) * l_k));
  }
  l_s.x = coax_p_scale3(l_s.x, (0x1.e147aep-1f + (0x1.eb851ep-4f * l_tone)));
  l_v = coax_p_fract((l_f.y / l_floor_h));
  if ((l_facade == 1i))
  {
    l_sp = coax_p_pr_band((l_v * l_floor_h), (l_floor_h * 0x1.ae147ap-3f), (l_floor_h * 0x1.ae147ap-2f), l_ay, l_floor_h);
    l_s.x = coax_p_mix3(l_s.x, coax_p_v3(0x1.666666p-1f, 0x1.666666p-1f, 0x1.5c28f6p-1f), l_sp);
    l_s.y = coax_p_mix(l_s.y, 0x1.47ae14p-2f, l_sp);
    l_s.z = coax_p_mix(l_s.z, 0x1.666666p-1f, l_sp);
  }
  else
  {
    l_sp_20 = coax_p_pr_band((l_v * l_floor_h), (l_floor_h * 0x1.eb851ep-4f), 0x1.ccccccp-1f, l_ay, l_floor_h);
    l_s.x = coax_p_mix3(l_s.x, coax_p_scale3(l_s.x, 0x1.666666p-1f), l_sp_20);
    l_s.y = coax_p_mix(l_s.y, 0x1.eb851ep-4f, l_sp_20);
  }
  l_mullion = coax_p_fmax(coax_p_pr_line(l_f.x, l_bay, 0x1.1eb852p-4f, l_ax), coax_p_pr_line(l_f.y, l_floor_h, 0x1.1eb852p-4f, l_ay));
  l_s.x = coax_p_mix3(l_s.x, coax_p_v3(0x1.47ae14p-2f, 0x1.51eb86p-2f, 0x1.5c28f6p-2f), l_mullion);
  l_s.y = coax_p_mix(l_s.y, 0x1.666666p-2f, l_mullion);
  l_s.z = coax_p_mix(l_s.z, 0x1.000000p0f, (l_mullion * 0x1.99999ap-1f));
  if ((l_facade == 2i))
  {
    l_fin = coax_p_pr_line(l_f.x, l_bay, 0x1.1eb852p-2f, l_ax);
    l_s.x = coax_p_mix3(l_s.x, coax_p_v3(0x1.8f5c28p-1f, 0x1.851eb8p-1f, 0x1.70a3d8p-1f), l_fin);
    l_s.y = coax_p_mix(l_s.y, 0x1.19999ap-1f, l_fin);
    l_s.z = coax_p_mix(l_s.z, 0.0f, l_fin);
  }
  l_s.f4 = coax_p_add3(l_s.f4, coax_p_pr_room(l_f, (l_bay * 0x1.000000p1f), l_floor_h, l_seed, l_ax, l_ay, l_b));
  return l_s;
}
fn coax_p_pr_poured(p_s0: coax_t26, p_f: vec2f, p_ax: f32, p_ay: f32, p_world: vec3f, p_fl: vec3f, p_seed: f32, p_lift0: f32, p_b: i32, p_x: i32) -> coax_t26 {
  let l_s0: coax_t26 = p_s0;
  let l_f: vec2f = p_f;
  let l_ax: f32 = p_ax;
  let l_ay: f32 = p_ay;
  let l_world: vec3f = p_world;
  let l_fl: vec3f = p_fl;
  let l_seed: f32 = p_seed;
  let l_lift0: f32 = p_lift0;
  let l_b: i32 = p_b;
  let l_x: i32 = p_x;
  var l_s: coax_t26;
  var l_cloud: f32;
  var l_lift: f32;
  var l_panel: f32;
  var l_bay: f32;
  var l_seam: f32;
  var l_every: f32;
  var l_o1: f32;
  var l_o2: f32;
  var l_vary: f32;
  var l_li: f32;
  var l_bi: f32;
  var l_t1: f32;
  var l_t2: f32;
  var l_k: f32;
  var l_tone: f32;
  var l_cloud_28: f32;
  var l_above: f32;
  var l_streak: f32;
  var l_dark: f32;
  var l_tx: f32;
  var l_ty: f32;
  var l_hole: f32;
  var l_d: f32;
  var l_pil: f32;
  var l_id: f32;
  var l_ox: f32;
  var l_r: i32;
  var l_oy: f32;
  var l_h: f32;
  var l_reach: f32;
  var l_dy: f32;
  var l_w: f32;
  var l_run: f32;
  var l_over: f32;
  var l_blot: f32;
  var l_top: f32;
  var l_grow: f32;
  var l_salt: f32;
  l_s = l_s0;
  if ((abs(l_fl.y) > 0x1.666666p-1f))
  {
    l_cloud = coax_p_pr_cloud2(((l_f.x * 0x1.99999ap-5f) + l_seed), ((l_f.y * 0x1.99999ap-5f) - l_seed));
    l_s.x = coax_p_scale3(l_s.x, (0x1.c28f5cp-1f + (0x1.70a3d8p-3f * l_cloud)));
    return l_s;
  }
  l_lift = select(0x1.800000p1f, l_lift0, (l_lift0 > 0.0f));
  l_panel = select(0x1.400000p1f, materials[l_x], (materials[l_x] > 0.0f));
  l_bay = select(0x1.900000p5f, materials[(l_x + 1i)], (materials[(l_x + 1i)] > 0.0f));
  l_seam = materials[(l_x + 2i)];
  l_every = materials[(l_x + 3i)];
  l_o1 = materials[(l_x + 4i)];
  l_o2 = materials[(l_x + 5i)];
  l_vary = select(0x1.99999ap-5f, materials[(l_x + 6i)], (materials[(l_x + 6i)] > 0.0f));
  l_li = floor((l_f.y / l_lift));
  l_bi = floor((l_f.x / l_bay));
  l_t1 = (coax_p_render_noise2(((l_li * 0x1.75c290p-1f) + l_seed), 0x1.8cccccp1f) - 0x1.000000p-1f);
  l_t2 = (coax_p_render_noise2(((l_bi * 0x1.4f5c28p0f) + (l_seed * 0x1.000000p1f)), ((l_li * 0x1.570a3ep-1f) + 0x1.400000p2f)) - 0x1.000000p-1f);
  l_k = coax_p_fract((l_f.y / l_lift));
  l_tone = (((0x1.000000p0f + ((0x1.333334p-1f * l_vary) * l_t1)) + (l_vary * l_t2)) * (0x1.f851ecp-1f + (0x1.eb851ep-6f * l_k)));
  l_cloud_28 = coax_p_pr_cloud2(((l_f.x * 0x1.c28f5cp-4f) + l_seed), ((l_f.y * 0x1.47ae14p-3f) - l_seed));
  l_tone = (l_tone * (0x1.eb851ep-1f + (0x1.47ae14p-4f * l_cloud_28)));
  l_above = 0.0f;
  if ((l_seam > 0.0f))
  {
    l_above = coax_p_smoothstep((l_seam - l_ay), (l_seam + l_ay), l_f.y);
    l_tone = (l_tone * coax_p_mix(0x1.e66666p-1f, (0x1.000000p0f + materials[(l_x + 7i)]), l_above));
  }
  l_s.x = coax_p_scale3(l_s.x, l_tone);
  l_s.x = coax_p_mix3(l_s.x, coax_p_scale3(coax_p_v3(0x1.3d70a4p-1f, 0x1.428f5cp-1f, 0x1.47ae14p-1f), (coax_p_pr_lum(l_s.x) / 0x1.428f5cp-1f)), ((0x1.000000p0f - l_above) * 0x1.000000p-2f));
  l_streak = coax_p_smoothstep(0x1.ccccccp-2f, 0x1.ccccccp-1f, coax_p_pr_noise2(((l_f.x * 0x1.4cccccp-1f) + l_seed), (l_f.y * 0x1.89374cp-7f)));
  l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - (0x1.1eb852p-4f * coax_p_mix(0x1.c28f5cp-3f, l_streak, coax_p_pr_resolve(0x1.800000p0f, l_ax)))));
  l_dark = 0.0f;
  l_dark = coax_p_fmax(l_dark, (0x1.1eb852p-2f * coax_p_pr_line(l_f.y, l_lift, 0x1.eb851ep-6f, l_ay)));
  l_dark = coax_p_fmax(l_dark, (0x1.1eb852p-3f * coax_p_fmax(coax_p_pr_line(l_f.x, l_panel, 0x1.89374cp-7f, l_ax), coax_p_pr_line(l_f.y, (l_lift * 0x1.000000p-1f), 0x1.0624dep-7f, l_ay))));
  l_dark = coax_p_fmax(l_dark, (0x1.99999ap-2f * coax_p_pr_line(l_f.x, l_bay, 0x1.eb851ep-5f, l_ax)));
  l_tx = ((coax_p_fract((l_f.x / l_panel)) - 0x1.000000p-1f) * l_panel);
  l_ty = (((coax_p_fract((l_f.y / (l_lift * 0x1.000000p-1f))) - 0x1.000000p-1f) * l_lift) * 0x1.000000p-1f);
  l_hole = ((0x1.000000p0f - coax_p_smoothstep(0x1.a9fbe8p-7f, 0x1.47ae14p-6f, sqrt(((l_tx * l_tx) + (l_ty * l_ty))))) * coax_p_pr_resolve(0x1.eb851ep-6f, coax_p_fmax(l_ax, l_ay)));
  l_dark = coax_p_fmax(l_dark, (0x1.19999ap-1f * l_hole));
  if ((l_seam > 0.0f))
  {
    l_dark = coax_p_fmax(l_dark, (0x1.ccccccp-2f * coax_p_pr_band(l_f.y, (l_seam - 0x1.666666p-2f), 0x1.666666p-1f, l_ay, 0.0f)));
    l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f + (0x1.eb851ep-4f * coax_p_pr_band(l_f.y, (l_seam + 0x1.eb851ep-4f), 0x1.eb851ep-3f, l_ay, 0.0f))));
    l_d = (abs((coax_p_fract(((l_f.x / l_bay) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_bay);
    l_pil = (coax_p_pr_band(l_d, 0.0f, 0x1.000000p3f, l_ax, 0.0f) * l_above);
    l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f + (0x1.1eb852p-5f * l_pil)));
    l_dark = coax_p_fmax(l_dark, ((0x1.c28f5cp-3f * coax_p_pr_band(l_d, 0x1.000000p2f, 0x1.47ae14p-4f, l_ax, 0.0f)) * l_above));
  }
  if ((l_every > 0.0f))
  {
    l_id = floor((l_f.x / l_every));
    l_ox = ((coax_p_fract((l_f.x / l_every)) - 0x1.000000p-1f) * l_every);
    {
      let coax_from: i32 = 0i;
      let coax_to: i32 = 2i;
      for (l_r = coax_from; l_r < coax_to; l_r = l_r + 1i)
      {
        l_oy = select(l_o2, l_o1, (l_r == 0i));
        if ((l_oy > 0.0f))
        {
          l_dark = coax_p_fmax(l_dark, ((0x1.b33334p-1f * coax_p_pr_band(l_ox, 0.0f, 0x1.333334p0f, l_ax, 0.0f)) * coax_p_pr_band(l_f.y, l_oy, 0x1.333334p-1f, l_ay, 0.0f)));
          l_h = coax_p_render_noise2((((l_id * 0x1.b33334p0f) + (f32(l_r) * 0x1.400000p2f)) + l_seed), l_oy);
          l_reach = (0x1.400000p3f + (0x1.400000p4f * l_h));
          l_dy = ((l_oy - 0x1.333334p-2f) - l_f.y);
          if (((l_dy > 0.0f) && (l_dy < l_reach)))
          {
            l_w = ((0x1.000000p-1f + (0x1.666666p-2f * (0x1.000000p0f - (l_dy / l_reach)))) + (0x1.99999ap-2f * (coax_p_pr_noise2((l_f.x * 0x1.000000p1f), (l_f.y * 0x1.333334p-2f)) - 0x1.000000p-1f)));
            l_run = ((0x1.000000p0f - coax_p_smoothstep((l_w * 0x1.000000p-1f), (((l_w * 0x1.000000p-1f) + 0x1.99999ap-2f) + l_ax), abs(l_ox))) * coax_pow_bg_vs((0x1.000000p0f - (l_dy / l_reach)), 0x1.800000p0f));
            if ((l_h > 0x1.19999ap-1f))
            {
              l_s.x = coax_p_mix3(l_s.x, coax_p_mul3(l_s.x, coax_p_v3(0x1.3d70a4p-1f, 0x1.ae147ap-2f, 0x1.147ae2p-2f)), (l_run * 0x1.800000p-1f));
            }
            else
            {
              l_dark = coax_p_fmax(l_dark, (0x1.333334p-2f * l_run));
            }
          }
        }
      }
    }
  }
  l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - l_dark));
  l_over = (l_world.y - coax_at_u(i32(((l_b + 28i) + 2i))));
  if ((l_over < 0x1.800000p2f))
  {
    l_blot = coax_p_pr_cloud2(((l_f.x * 0x1.99999ap-2f) + l_seed), (l_f.y * 0x1.ccccccp-1f));
    l_top = (0x1.800000p0f + (0x1.99999ap-1f * l_blot));
    l_grow = (0x1.000000p0f - coax_p_smoothstep((l_top - 0x1.99999ap-2f), (l_top + 0x1.333334p-2f), l_over));
    l_salt = ((0x1.000000p0f - coax_p_smoothstep((0x1.800000p1f + l_blot), (0x1.200000p2f + l_blot), l_over)) * (0x1.000000p0f - l_grow));
    l_s.x = coax_p_mix3(l_s.x, coax_p_scale3(l_s.x, 0x1.147ae2p0f), (l_salt * 0x1.333334p-1f));
    l_s.x = coax_p_mix3(l_s.x, coax_p_mix3(coax_p_v3(0x1.1eb852p-4f, 0x1.47ae14p-4f, 0x1.99999ap-5f), coax_p_v3(0x1.47ae14p-3f, 0x1.5c28f6p-3f, 0x1.c28f5cp-4f), l_blot), l_grow);
    l_s.y = coax_p_mix(l_s.y, 0x1.333334p-1f, l_grow);
  }
  l_s.y = coax_p_saturate(((l_s.y + (0x1.99999ap-4f * (l_cloud_28 - 0x1.000000p-1f))) + (0x1.99999ap-3f * l_hole)));
  return l_s;
}
fn coax_p_pr_painted(p_s0: coax_t26, p_f: vec2f, p_ax: f32, p_ay: f32, p_local: vec3f, p_fl: vec3f, p_seed: f32, p_course0: f32) -> coax_t26 {
  let l_s0: coax_t26 = p_s0;
  let l_f: vec2f = p_f;
  let l_ax: f32 = p_ax;
  let l_ay: f32 = p_ay;
  let l_local: vec3f = p_local;
  let l_fl: vec3f = p_fl;
  let l_seed: f32 = p_seed;
  let l_course0: f32 = p_course0;
  var l_s: coax_t26;
  var l_course: f32;
  var l_long: f32;
  var l_ci: f32;
  var l_along: f32;
  var l_chalk: f32;
  var l_seam: f32;
  var l_k: f32;
  var l_runs: f32;
  l_s = l_s0;
  l_course = select(0x1.333334p1f, l_course0, (l_course0 > 0.0f));
  l_long = (l_course * 0x1.000000p2f);
  l_ci = floor((l_f.y / l_course));
  l_along = (l_f.x + select(0.0f, (l_long * 0x1.000000p-1f), (coax_p_fract((l_ci * 0x1.000000p-1f)) > 0x1.000000p-2f)));
  l_chalk = coax_p_pr_cloud2(((l_f.x * 0x1.eb851ep-4f) + l_seed), (l_f.y * 0x1.eb851ep-4f));
  l_s.x = coax_p_scale3(l_s.x, ((0x1.d70a3ep-1f + (0x1.1eb852p-3f * l_chalk)) + (0x1.99999ap-5f * (coax_p_render_noise2((floor((l_along / l_long)) + l_seed), l_ci) - 0x1.000000p-1f))));
  l_seam = coax_p_fmax(coax_p_pr_line(l_f.y, l_course, 0x1.47ae14p-6f, l_ay), coax_p_pr_line(l_along, l_long, 0x1.47ae14p-6f, l_ax));
  l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - (0x1.70a3d8p-3f * l_seam)));
  l_k = coax_p_fract((l_f.y / l_course));
  l_runs = (coax_p_smoothstep(0x1.3d70a4p-1f, 0x1.b33334p-1f, coax_p_pr_noise2(((l_f.x * 0x1.99999ap0f) + l_seed), (l_f.y * 0x1.47ae14p-4f))) * coax_pow_bg_vs(l_k, 0x1.800000p2f));
  l_s.x = coax_p_mix3(l_s.x, coax_p_v3(0x1.333334p-2f, 0x1.47ae14p-3f, 0x1.47ae14p-4f), ((l_runs * 0x1.333334p-1f) * coax_p_pr_resolve(0x1.333334p-3f, l_ax)));
  l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - ((0x1.c28f5cp-3f * (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.99999ap0f, l_local.y))) * abs((0x1.000000p0f - abs(l_fl.y))))));
  l_s.y = coax_p_saturate(((l_s.y + (0x1.333334p-3f * (l_chalk - 0x1.000000p-1f))) + (0x1.333334p-2f * l_runs)));
  return l_s;
}
fn coax_p_render_noise2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  var l_ok: bool;
  l_ok = (coax_p_render_noise_coordinate_ok(l_x) && coax_p_render_noise_coordinate_ok(l_y));
  return select(0.0f, coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y))), l_ok);
}
fn coax_p_pr_line(p_x: f32, p_every: f32, p_wide: f32, p_aa0: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_every: f32 = p_every;
  let l_wide: f32 = p_wide;
  let l_aa0: f32 = p_aa0;
  var l_d: f32;
  var l_aa: f32;
  var l_sharp: f32;
  l_d = (abs((coax_p_fract(((l_x / l_every) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_every);
  l_aa = coax_p_fmax(l_aa0, 0x1.a36e2ep-14f);
  l_sharp = (0x1.000000p0f - coax_p_smoothstep(((l_wide * 0x1.000000p-1f) - l_aa), ((l_wide * 0x1.000000p-1f) + l_aa), l_d));
  return coax_p_mix(coax_p_saturate((l_wide / l_every)), l_sharp, coax_p_saturate(((l_every / (l_aa * 0x1.400000p1f)) - 0x1.000000p-1f)));
}
fn coax_p_wo_cos_refracted(p_ci: f32) -> f32 {
  let l_ci: f32 = p_ci;
  return sqrt(coax_p_fmax((0x1.000000p0f - ((0x1.000000p0f - (l_ci * l_ci)) / (0x1.547ae2p0f * 0x1.547ae2p0f))), 0.0f));
}
fn coax_p_wo_through(p_absorb: vec3f, p_path: f32) -> vec3f {
  let l_absorb: vec3f = p_absorb;
  let l_path: f32 = p_path;
  return coax_p_wo_exp3(coax_p_scale3(l_absorb, (-coax_p_fmax(l_path, 0.0f))));
}
fn coax_p_pr_stone(p_s0: coax_t26, p_f: vec2f, p_ax: f32, p_ay: f32, p_da: f32, p_local: vec3f, p_fl: vec3f, p_seed: f32, p_course0: f32, p_full: bool, p_x: i32) -> coax_t26 {
  let l_s0: coax_t26 = p_s0;
  let l_f: vec2f = p_f;
  let l_ax: f32 = p_ax;
  let l_ay: f32 = p_ay;
  let l_da: f32 = p_da;
  let l_local: vec3f = p_local;
  let l_fl: vec3f = p_fl;
  let l_seed: f32 = p_seed;
  let l_course0: f32 = p_course0;
  let l_full: bool = p_full;
  let l_x: i32 = p_x;
  var l_s: coax_t26;
  var l_course: f32;
  var l_flutes: f32;
  var l_weather: f32;
  var l_row_i: f32;
  var l_vein: f32;
  var l_fine: f32;
  var l_joint: f32;
  var l_tone: f32;
  var l_a: f32;
  var l_ph: f32;
  var l_keep: f32;
  var l_t: vec3f;
  var l_hollow: f32;
  var l_along: f32;
  var l_hang: f32;
  var l_foot: f32;
  l_s = l_s0;
  l_course = select(0x1.333334p-1f, l_course0, (l_course0 > 0.0f));
  l_flutes = materials[l_x];
  l_weather = materials[(l_x + 1i)];
  l_row_i = floor((l_f.y / l_course));
  l_vein = coax_p_pr_cloud2(((l_f.x * 0x1.000000p-1f) + l_seed), (l_f.y * 0x1.000000p-1f));
  l_fine = coax_p_pr_noise2(((l_f.x * 0x1.800000p1f) + (l_f.y * 0x1.4cccccp0f)), ((l_f.y * 0x1.800000p1f) - l_seed));
  l_joint = coax_p_pr_line(l_f.y, l_course, 0x1.0624dep-7f, l_ay);
  l_tone = 0.0f;
  if (((l_flutes > 0.0f) && (abs(l_fl.y) < 0x1.666666p-1f)))
  {
    l_a = coax_p_wg_atan2(l_fl.z, l_fl.x);
    l_ph = coax_p_fract(((l_a * l_flutes) / 0x1.921fb6p2f));
    l_keep = coax_p_saturate((((0x1.921fb6p2f / l_flutes) / (coax_p_fmax(l_da, 0x1.a36e2ep-14f) * 0x1.800000p1f)) - 0x1.000000p-1f));
    if (l_full)
    {
      l_t = coax_p_normalize3(coax_p_v3((0.0f - l_s.w.z), 0.0f, l_s.w.x));
      l_s.w = coax_p_normalize3(coax_p_add3(l_s.w, coax_p_scale3(l_t, ((sin((l_ph * 0x1.921fb6p2f)) * 0x1.666666p-2f) * l_keep))));
    }
    l_hollow = (0x1.000000p-1f - (0x1.000000p-1f * cos((l_ph * 0x1.921fb6p2f))));
    l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - (((0x1.99999ap-4f + (0x1.eb851ep-4f * l_weather)) * (0x1.000000p0f - l_hollow)) * l_keep)));
    l_tone = coax_p_render_noise2((l_row_i + (l_seed * 0x1.800000p1f)), l_seed);
  }
  else
  {
    if ((abs(l_fl.y) < 0x1.666666p-1f))
    {
      l_along = (l_f.x + select(0.0f, l_course, (coax_p_fract((l_row_i * 0x1.000000p-1f)) > 0x1.000000p-2f)));
      l_tone = coax_p_render_noise2((floor((l_along / (l_course * 0x1.000000p1f))) + (l_seed * 0x1.800000p1f)), l_row_i);
      l_joint = coax_p_fmax(l_joint, coax_p_pr_line(l_along, (l_course * 0x1.000000p1f), 0x1.0624dep-7f, l_ax));
    }
    else
    {
      l_tone = coax_p_render_noise2((floor((l_f.x / (l_course * 0x1.000000p1f))) + l_seed), floor((l_f.y / l_course)));
      l_joint = coax_p_fmax(coax_p_pr_line(l_f.x, (l_course * 0x1.000000p1f), 0x1.0624dep-7f, l_ax), coax_p_pr_line(l_f.y, l_course, 0x1.0624dep-7f, l_ay));
    }
  }
  l_s.x = coax_p_scale3(l_s.x, ((((0x1.dc28f6p-1f + (0x1.99999ap-4f * l_tone)) * (0x1.d70a3ep-1f + (0x1.eb851ep-4f * l_vein))) * (0x1.f0a3d8p-1f + (0x1.eb851ep-5f * l_fine))) * (0x1.000000p0f - (0x1.47ae14p-2f * l_joint))));
  l_s.x = coax_p_mix3(l_s.x, coax_p_mul3(l_s.x, coax_p_v3(0x1.051eb8p0f, 0x1.e66666p-1f, 0x1.ae147ap-1f)), (l_weather * coax_p_smoothstep(0x1.666666p-2f, 0x1.800000p-1f, l_vein)));
  if ((abs(l_fl.y) < 0x1.666666p-1f))
  {
    l_hang = coax_p_mix(0x1.333334p-3f, coax_p_smoothstep(0x1.19999ap-1f, 0x1.ccccccp-1f, coax_p_pr_noise2(((l_f.x * 0x1.19999ap0f) + l_seed), (l_f.y * 0x1.99999ap-5f))), coax_p_pr_resolve(0x1.ccccccp-1f, l_ax));
    l_s.x = coax_p_scale3(l_s.x, (0x1.000000p0f - ((0x1.c28f5cp-3f * l_weather) * l_hang)));
    l_foot = ((0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.ccccccp-1f, l_local.y)) * (0x1.333334p-1f + (0x1.99999ap-2f * l_fine)));
    l_s.x = coax_p_mix3(l_s.x, coax_p_mul3(l_s.x, coax_p_v3(0x1.3d70a4p-1f, 0x1.333334p-1f, 0x1.19999ap-1f)), (l_foot * l_weather));
  }
  l_s.y = coax_p_saturate(((l_s.y + (0x1.eb851ep-4f * (l_vein - 0x1.000000p-1f))) + (0x1.333334p-3f * l_joint)));
  return l_s;
}
fn coax_p_pr_tilt(p_n: vec3f, p_facing: vec3f, p_dx: f32, p_dy: f32) -> vec3f {
  let l_n: vec3f = p_n;
  let l_facing: vec3f = p_facing;
  let l_dx: f32 = p_dx;
  let l_dy: f32 = p_dy;
  var l_t: vec3f;
  var l_s: vec3f;
  l_t = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
  l_s = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
  if ((abs(l_facing.y) <= 0x1.666666p-1f))
  {
    l_s = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
    if ((abs(l_facing.x) > abs(l_facing.z)))
    {
      l_t = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
    }
  }
  return coax_p_normalize3(coax_p_sub3(l_n, coax_p_add3(coax_p_scale3(l_t, l_dx), coax_p_scale3(l_s, l_dy))));
}
fn coax_p_sw_foot(p_k: i32, p_along: f32, p_span: f32) -> f32 {
  let l_k: i32 = p_k;
  let l_along: f32 = p_along;
  let l_span: f32 = p_span;
  var l_a: f32;
  var l_i: f32;
  var l_j: f32;
  var l_base: u32;
  l_a = (coax_p_clampf((((l_along / l_span) + 0x1.000000p-1f) * 0x1.000000p5f), 0.0f, 0x1.fffbe8p4f) - 0x1.000000p-1f);
  l_i = coax_p_clampf(floor(l_a), 0.0f, 0x1.f00000p4f);
  l_j = coax_p_fmin((l_i + 0x1.000000p0f), 0x1.f00000p4f);
  l_base = u32((l_k * 32i));
  return coax_p_mix(coax_p_sw_byte(16180i, (l_base + coax_p_wg_u32_f(l_i))), coax_p_sw_byte(16180i, (l_base + coax_p_wg_u32_f(l_j))), coax_p_clampf((l_a - l_i), 0.0f, 0x1.000000p0f));
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
fn coax_p_pr_sky(p_rd: vec3f, p_b: i32) -> vec3f {
  let l_rd: vec3f = p_rd;
  let l_b: i32 = p_b;
  var l_sun: vec3f;
  var l_above: vec3f;
  l_sun = coax_p_pr_at3((l_b + 16i));
  l_above = coax_p_add3(coax_p_pr_under_cloud(coax_p_pr_air_light(coax_p_v3(l_rd.x, coax_p_fmax(l_rd.y, 0.0f), l_rd.z), l_sun, coax_at_u(i32((l_b + 51i)))), l_b), coax_p_pr_night(coax_at_u(i32(((l_b + 16i) + 3i)))));
  if ((l_rd.y >= 0.0f))
  {
    return l_above;
  }
  return coax_p_mix3(l_above, coax_p_pr_at3((l_b + 24i)), coax_p_saturate(((0.0f - l_rd.y) * 0x1.200000p3f)));
}
fn coax_p_pr_air_through(p_dist: f32, p_eye_y: f32, p_rd_y: f32, p_b: i32) -> f32 {
  let l_dist: f32 = p_dist;
  let l_eye_y: f32 = p_eye_y;
  let l_rd_y: f32 = p_rd_y;
  let l_b: i32 = p_b;
  var l_h: f32;
  var l_y0: f32;
  var l_k: f32;
  var l_along: f32;
  l_h = coax_p_fmax(coax_at_u(i32(((l_b + 28i) + 1i))), 0x1.000000p0f);
  l_y0 = coax_p_fmax((l_eye_y - coax_at_u(i32(((l_b + 28i) + 2i)))), 0.0f);
  l_k = ((l_rd_y * l_dist) / l_h);
  l_along = select(((l_dist * (0x1.000000p0f - exp((0.0f - l_k)))) / l_k), l_dist, (abs(l_k) < 0x1.0624dep-10f));
  return exp((0.0f - ((coax_at_u(i32((l_b + 28i))) * exp((0.0f - (l_y0 / l_h)))) * l_along)));
}
fn coax_p_wg_at4_1(p_at: i32) -> vec4f {
  let l_at: i32 = p_at;
  return coax_p_v4(field[l_at], field[(l_at + 1i)], field[(l_at + 2i)], field[(l_at + 3i)]);
}
fn coax_p_mix4(p_a: vec4f, p_b: vec4f, p_t: f32) -> vec4f {
  let l_a: vec4f = p_a;
  let l_b: vec4f = p_b;
  let l_t: f32 = p_t;
  return vec4f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t), coax_p_mix(l_a.w, l_b.w, l_t));
}
fn coax_p_cm_f0(p_albedo: vec3f, p_metal: f32) -> vec3f {
  let l_albedo: vec3f = p_albedo;
  let l_metal: f32 = p_metal;
  return coax_p_mix3(coax_p_v3(0x1.47ae14p-5f, 0x1.47ae14p-5f, 0x1.47ae14p-5f), l_albedo, coax_p_saturate(l_metal));
}
fn coax_p_cm_ggx(p_f0: vec3f, p_rough: f32, p_n: vec3f, p_l: vec3f, p_v: vec3f) -> vec3f {
  let l_f0: vec3f = p_f0;
  let l_rough: f32 = p_rough;
  let l_n: vec3f = p_n;
  let l_l: vec3f = p_l;
  let l_v: vec3f = p_v;
  var l_nl: f32;
  var l_h: vec3f;
  var l_nv: f32;
  var l_nh: f32;
  var l_vh: f32;
  var l_a: f32;
  var l_aa: f32;
  var l_den: f32;
  var l_k: f32;
  var l_s: f32;
  var l_f: f32;
  l_nl = dot(l_n, l_l);
  if ((!(l_nl > 0.0f)))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_h = coax_p_normalize3(coax_p_add3(l_l, l_v));
  l_nv = coax_p_fmax(0x1.a36e2ep-14f, dot(l_n, l_v));
  l_nh = coax_p_fmax(0.0f, dot(l_n, l_h));
  l_vh = coax_p_fmax(0.0f, dot(l_v, l_h));
  l_a = (l_rough * l_rough);
  l_aa = (l_a * l_a);
  l_den = (((l_nh * l_nh) * (l_aa - 0x1.000000p0f)) + 0x1.000000p0f);
  l_k = (((l_rough + 0x1.000000p0f) * (l_rough + 0x1.000000p0f)) / 0x1.000000p3f);
  l_s = (((((((l_aa / ((0x1.921fb6p1f * l_den) * l_den)) * l_nv) / ((l_nv * (0x1.000000p0f - l_k)) + l_k)) * l_nl) / ((l_nl * (0x1.000000p0f - l_k)) + l_k)) / (0x1.000000p2f * l_nv)) * 0x1.921fb6p1f);
  l_f = coax_pow_bg_vs((0x1.000000p0f - l_vh), 0x1.400000p2f);
  return coax_p_v3(((l_f0.x + ((0x1.000000p0f - l_f0.x) * l_f)) * l_s), ((l_f0.y + ((0x1.000000p0f - l_f0.y) * l_f)) * l_s), ((l_f0.z + ((0x1.000000p0f - l_f0.z) * l_f)) * l_s));
}
fn coax_p_pbr_environment(p_rough: f32, p_nv: f32) -> vec2f {
  let l_rough: f32 = p_rough;
  let l_nv: f32 = p_nv;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_w: f32;
  var l_a: f32;
  var l_b: f32;
  var l_term: f32;
  l_x = (0x1.000000p0f - l_rough);
  l_y = (0x1.5c28f6p-5f - (0x1.c28f5cp-6f * l_rough));
  l_z = (0x1.0a3d70p0f - (0x1.24dd30p-1f * l_rough));
  l_w = ((-0x1.47ae14p-5f) + (0x1.6872b0p-6f * l_rough));
  l_a = (l_x * l_x);
  l_b = coax_pow_bg_vs(0x1.000000p1f, ((-0x1.28f5c2p3f) * l_nv));
  l_term = ((select(l_b, l_a, (l_a < l_b)) * l_x) + l_y);
  return vec2f((((-0x1.0a3d70p0f) * l_term) + l_z), ((0x1.0a3d70p0f * l_term) + l_w));
}
fn coax_p_pr_air_far(p_rd: vec3f, p_sun: vec3f, p_hz: f32) -> vec3f {
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  let l_hz: f32 = p_hz;
  var l_l: f32;
  l_l = sqrt(((l_rd.x * l_rd.x) + (l_rd.z * l_rd.z)));
  if ((l_l < 0x1.a36e2ep-14f))
  {
    return coax_p_pr_air_light(coax_p_v3(0.0f, 0.0f, (0.0f - 0x1.000000p0f)), l_sun, l_hz);
  }
  return coax_p_pr_air_light(coax_p_v3((l_rd.x / l_l), 0.0f, (l_rd.z / l_l)), l_sun, l_hz);
}
fn coax_p_pr_under_cloud(p_c: vec3f, p_b: i32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_b: i32 = p_b;
  var l_l: f32;
  l_l = (((l_c.x * 0x1.b367a0p-3f) + (l_c.y * 0x1.6e2eb2p-1f)) + (l_c.z * 0x1.27bb30p-4f));
  return coax_p_scale3(coax_p_mix3(l_c, coax_p_v3(l_l, l_l, l_l), coax_p_pr_cloud_shade(l_b)), coax_at_u(i32((l_b + 75i))));
}
fn coax_p_pr_night(p_day: f32) -> vec3f {
  let l_day: f32 = p_day;
  return coax_p_scale3(coax_p_v3(0x1.cac084p-9f, 0x1.54c986p-8f, 0x1.581062p-7f), (0x1.000000p0f - l_day));
}
fn coax_p_film_rgb(p_r: f32, p_g: f32, p_b: f32) -> vec3f {
  let l_r: f32 = p_r;
  let l_g: f32 = p_g;
  let l_b: f32 = p_b;
  return vec3f(l_r, l_g, l_b);
}
fn coax_p_film_photo(p_linear: vec3f) -> vec3f {
  let l_linear: vec3f = p_linear;
  var l_t: vec3f;
  var l_yi: f32;
  var l_yo: f32;
  var l_soft: f32;
  var l_w: f32;
  l_t = coax_p_film_rec709(l_linear);
  l_yi = coax_p_film_max(coax_p_film_luma(l_linear), 0.0f);
  l_yo = coax_p_film_luma(l_t);
  l_soft = (0x1.70a3d8p-3f * coax_pow_bg_vs((l_yi / 0x1.70a3d8p-3f), 0x1.400000p0f));
  l_w = coax_p_film_sat((l_yi / 0x1.70a3d8p-3f));
  l_w = ((l_w * l_w) * (0x1.800000p1f - (0x1.000000p1f * l_w)));
  return coax_p_film_scale(l_t, ((l_soft + ((l_yo - l_soft) * l_w)) / coax_p_film_max(l_yo, 0x1.0c6f7ap-20f)));
}
fn coax_p_pr_encode(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_v3(coax_p_pr_encode1(l_c.x), coax_p_pr_encode1(l_c.y), coax_p_pr_encode1(l_c.z));
}
fn coax_p_pr_lum(p_c: vec3f) -> f32 {
  let l_c: vec3f = p_c;
  return (((l_c.x * 0x1.b367a0p-3f) + (l_c.y * 0x1.6e2eb2p-1f)) + (l_c.z * 0x1.27bb30p-4f));
}
fn coax_p_pr_contact(p_x: i32, p_y: i32, p_n: vec3f, p_share: f32, p_b: i32) -> f32 {
  let l_x: i32 = p_x;
  let l_y: i32 = p_y;
  let l_n: vec3f = p_n;
  let l_share: f32 = p_share;
  let l_b: i32 = p_b;
  var l_wi: i32;
  var l_hi: i32;
  var l_w: f32;
  var l_h: f32;
  var l_d0: f32;
  var l_z: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_p: vec3f;
  var l_reduced: bool;
  var l_t: f32;
  var l_focal: f32;
  var l_noise: f32;
  var l_reach: f32;
  var l_rpx: f32;
  var l_taps: i32;
  var l_occ: f32;
  var l_i: i32;
  var l_a: f32;
  var l_r: f32;
  var l_qx: i32;
  var l_qy: i32;
  var l_dq: f32;
  var l_s: vec3f;
  var l_v: vec3f;
  var l_l: f32;
  var l_ao: f32;
  var l_shade: f32;
  var l_sun: vec3f;
  var l_march: f32;
  var l_from: vec3f;
  var l_eye: vec3f;
  var l_fw: vec3f;
  var l_i_40: i32;
  var l_q: vec3f;
  var l_rel: vec3f;
  var l_zq: f32;
  var l_sx: f32;
  var l_sy: f32;
  var l_zs: f32;
  var l_gap: f32;
  l_wi = i32(textureDimensions(post_depth).x);
  l_hi = i32(textureDimensions(post_depth).y);
  l_w = f32(l_wi);
  l_h = f32(l_hi);
  l_d0 = textureLoad(post_depth, vec2i(l_x, l_y), 0i);
  if ((l_d0 >= 0x1.fffeb0p-1f))
  {
    return 0x1.000000p0f;
  }
  l_z = coax_p_pr_post_z(l_d0, l_b);
  l_fx = (f32(l_x) + 0x1.000000p-1f);
  l_fy = (f32(l_y) + 0x1.000000p-1f);
  l_p = coax_p_pr_post_at(l_fx, l_fy, l_z, l_w, l_h, l_b);
  l_reduced = (coax_at_u(i32(((l_b + 24i) + 3i))) > 0x1.000000p-1f);
  l_t = coax_at_u(i32(((l_b + 0i) + 3i)));
  l_focal = (l_h / (0x1.000000p1f * l_t));
  l_noise = coax_p_fract((0x1.a7dd04p5f * coax_p_fract(((0x1.12e286p-4f * l_fx) + (0x1.7e8b20p-8f * l_fy)))));
  l_reach = coax_p_clampf((l_z * 0x1.89374cp-7f), 0x1.99999ap-1f, 0x1.800000p4f);
  l_rpx = coax_p_fmin(((l_reach * l_focal) / l_z), 0x1.680000p6f);
  l_taps = select(12i, 6i, l_reduced);
  l_occ = 0.0f;
  if ((l_rpx > 0x1.800000p0f))
  {
    {
      let coax_from: i32 = 0i;
      let coax_to: i32 = 12i;
      for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
      {
        if ((l_i < l_taps))
        {
          l_a = ((f32(l_i) + l_noise) * 0x1.3331fcp1f);
          l_r = ((sqrt(((f32(l_i) + l_noise) / f32(l_taps))) * l_rpx) + 0x1.000000p0f);
          l_qx = i32(coax_p_clampf((l_fx + (cos(l_a) * l_r)), 0.0f, (l_w - 0x1.000000p0f)));
          l_qy = i32(coax_p_clampf((l_fy + (sin(l_a) * l_r)), 0.0f, (l_h - 0x1.000000p0f)));
          l_dq = textureLoad(post_depth, vec2i(l_qx, l_qy), 0i);
          if ((l_dq < 0x1.fffeb0p-1f))
          {
            l_s = coax_p_pr_post_at((f32(l_qx) + 0x1.000000p-1f), (f32(l_qy) + 0x1.000000p-1f), coax_p_pr_post_z(l_dq, l_b), l_w, l_h, l_b);
            l_v = coax_p_sub3(l_s, l_p);
            l_l = coax_p_length3(l_v);
            l_occ = (l_occ + (coax_p_fmax(((dot(l_n, l_v) / coax_p_fmax(l_l, 0x1.a36e2ep-14f)) - 0x1.eb851ep-4f), 0.0f) * (0x1.000000p0f - coax_p_smoothstep((l_reach * 0x1.666666p-1f), (l_reach * 0x1.ccccccp0f), l_l))));
          }
        }
      }
    }
  }
  l_ao = (0x1.000000p0f - coax_p_clampf(((l_occ / f32(l_taps)) * 0x1.800000p0f), 0.0f, 0x1.99999ap-1f));
  l_shade = 0.0f;
  l_sun = coax_p_pr_at3((l_b + 16i));
  if ((((!l_reduced) && (l_share > 0x1.47ae14p-6f)) && (l_sun.y > 0.0f)))
  {
    l_march = coax_p_clampf((l_z * 0x1.0624dep-7f), 0x1.000000p-1f, 0x1.400000p3f);
    l_from = coax_p_add3(l_p, coax_p_scale3(l_n, ((l_march * 0x1.eb851ep-6f) + (l_z * 0x1.a36e2ep-12f))));
    l_eye = coax_p_pr_at3((l_b + 0i));
    l_fw = coax_p_pr_at3((l_b + 4i));
    {
      let coax_from: i32 = 1i;
      let coax_to: i32 = 9i;
      for (l_i_40 = coax_from; l_i_40 < coax_to; l_i_40 = l_i_40 + 1i)
      {
        if ((l_shade < 0x1.000000p-1f))
        {
          l_q = coax_p_add3(l_from, coax_p_scale3(l_sun, ((l_march * (f32(l_i_40) - l_noise)) / 0x1.000000p3f)));
          l_rel = coax_p_sub3(l_q, l_eye);
          l_zq = dot(l_rel, l_fw);
          l_sx = ((((dot(l_rel, coax_p_pr_at3((l_b + 8i))) / ((l_zq * l_t) * coax_at_u(i32(((l_b + 8i) + 3i))))) * 0x1.000000p-1f) + 0x1.000000p-1f) * l_w);
          l_sy = ((0x1.000000p-1f - ((dot(l_rel, coax_p_pr_at3((l_b + 12i))) / (l_zq * l_t)) * 0x1.000000p-1f)) * l_h);
          if ((((((l_zq > coax_at_u(i32(((l_b + 4i) + 3i)))) && (l_sx >= 0.0f)) && (l_sy >= 0.0f)) && (l_sx < l_w)) && (l_sy < l_h)))
          {
            l_zs = coax_p_pr_post_z(textureLoad(post_depth, vec2i(i32(l_sx), i32(l_sy)), 0i), l_b);
            l_gap = (l_zq - l_zs);
            if (((l_gap > ((l_zq * 0x1.3a92a4p-10f) + 0x1.eb851ep-6f)) && (l_gap < ((l_march * 0x1.800000p0f) + (l_zq * 0x1.89374cp-9f)))))
            {
              l_shade = 0x1.000000p0f;
            }
          }
        }
      }
    }
  }
  return (((0x1.000000p0f - l_share) * l_ao) + ((l_share * (0x1.000000p0f - (0x1.b33334p-1f * l_shade))) * coax_p_mix(0x1.000000p0f, l_ao, 0x1.000000p-2f)));
}
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t30 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  return coax_t30(l_x, l_y);
}
fn coax_p_wg_at4u(p_at: i32) -> coax_t33 {
  let l_at: i32 = p_at;
  return coax_p_u4(coax_p_wg_atu(l_at), coax_p_wg_atu((l_at + 1i)), coax_p_wg_atu((l_at + 2i)), coax_p_wg_atu((l_at + 3i)));
}
fn coax_p_wg_get_u4(p_p: coax_t33, p_i: i32) -> u32 {
  let l_p: coax_t33 = p_p;
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
fn coax_p_pr_near_of(p_m: coax_t13) -> f32 {
  let l_m: coax_t13 = p_m;
  if ((l_m.y.w < 0.0f))
  {
    return (0.0f - l_m.y.w);
  }
  return coax_p_fmax((l_m.y.w - 0x1.000000p0f), 0.0f);
}
fn coax_p_pr_near_keep(p_near: f32, p_dist: f32, p_reduced: bool) -> f32 {
  let l_near: f32 = p_near;
  let l_dist: f32 = p_dist;
  let l_reduced: bool = p_reduced;
  var l_n: f32;
  if ((l_near <= 0.0f))
  {
    return 0x1.000000p0f;
  }
  l_n = select(l_near, (l_near * 0x1.333334p-1f), l_reduced);
  return coax_p_saturate(((l_n - l_dist) / (l_n * 0x1.99999ap-3f)));
}
fn coax_p_slab_photo_pulse(p_c0: vec3f, p_id: f32) -> vec3f {
  let l_c0: vec3f = p_c0;
  let l_id: f32 = p_id;
  var l_c: vec3f;
  var l_pulse: f32;
  l_c = l_c0;
  if ((l_id > (-0x1.000000p-1f)))
  {
    if ((abs((l_id - coax_at_u(i32(1476i)))) < 0x1.000000p-1f))
    {
      l_pulse = (0x1.000000p-1f + (0x1.000000p-1f * sin((coax_at_u(i32(1478i)) * 0x1.400000p2f))));
      l_c = coax_p_add3(coax_p_scale3(l_c, 0x1.19999ap0f), coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.3d70a4p-1f, 0x1.99999ap-3f), (0x1.eb851ep-4f + (0x1.eb851ep-4f * l_pulse))));
    }
    else
    {
      if ((abs((l_id - coax_at_u(i32(1477i)))) < 0x1.000000p-1f))
      {
        l_c = coax_p_add3(coax_p_scale3(l_c, 0x1.0f5c28p0f), coax_p_v3(0x1.eb851ep-5f, 0x1.99999ap-5f, 0x1.eb851ep-6f));
      }
    }
  }
  return l_c;
}
fn coax_p_slab_sea_in(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  if (coax_p_slab_photo())
  {
    return l_c;
  }
  return coax_p_slab_sea_lin(l_c);
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
fn coax_p_slab_sea_out(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_v3(sqrt(coax_p_fmax(l_c.x, 0.0f)), sqrt(coax_p_fmax(l_c.y, 0.0f)), sqrt(coax_p_fmax(l_c.z, 0.0f)));
}
fn coax_p_slab_photo_show(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_pr_out(l_c, 15872i);
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
fn coax_p_slab_sea_top_skylight() -> vec3f {
  if (coax_p_slab_photo())
  {
    return coax_p_slab_photo_skylight();
  }
  return coax_p_slab_sea_skylight();
}
fn coax_p_slab_sea_kd() -> vec3f {
  return coax_p_v3(coax_at_u(i32(15840i)), coax_at_u(i32(15841i)), coax_at_u(i32(15842i)));
}
fn coax_p_slab_sea_clear() -> f32 {
  return coax_p_fmax(coax_at_u(i32(15843i)), 0x1.000000p0f);
}
fn coax_p_slab_sea_hull_shades(p_q: vec3f, p_ray: vec3f) -> bool {
  let l_q: vec3f = p_q;
  let l_ray: vec3f = p_ray;
  var l_o: vec3f;
  var l_d: vec3f;
  var l_span: vec2f;
  if (((coax_at_u(i32(15992i)) < 0x1.000000p-1f) || (coax_at_u(i32(15992i)) > 0x1.800000p0f)))
  {
    return false;
  }
  l_o = coax_p_sb_to_hull(l_q);
  l_d = coax_p_sb_dir_to_hull(l_ray);
  l_span = coax_p_slab_span(l_o, l_d, coax_p_v3((-coax_at_u(i32(36i))), coax_at_u(i32(39i)), (-coax_at_u(i32(37i)))), coax_p_v3(coax_at_u(i32(36i)), coax_at_u(i32(38i)), coax_at_u(i32(37i))));
  return (l_span.y > coax_p_fmax(l_span.x, 0.0f));
}
fn coax_p_pr_dome(p_rd: vec3f, p_soft: f32, p_b: i32) -> vec3f {
  let l_rd: vec3f = p_rd;
  let l_soft: f32 = p_soft;
  let l_b: i32 = p_b;
  var l_sun: vec3f;
  var l_night: vec3f;
  var l_hz: f32;
  var l_sky: vec3f;
  var l_dim: vec3f;
  var l_disc: vec3f;
  var l_c: coax_t34;
  l_sun = coax_p_pr_at3((l_b + 16i));
  l_night = coax_p_pr_night(coax_at_u(i32(((l_b + 16i) + 3i))));
  l_hz = coax_at_u(i32((l_b + 51i)));
  if ((l_rd.y < 0.0f))
  {
    return coax_p_add3(coax_p_pr_under_cloud(coax_p_pr_air_light(coax_p_v3(l_rd.x, 0.0f, l_rd.z), l_sun, l_hz), l_b), l_night);
  }
  l_sky = coax_p_pr_under_cloud(coax_p_pr_air_light(l_rd, l_sun, l_hz), l_b);
  l_dim = coax_p_scale3(coax_p_pr_haze_through(l_sun.y, l_hz), (0x1.000000p0f - (0x1.b33334p-1f * coax_p_pr_cloud_shade(l_b))));
  l_disc = coax_p_mul3(coax_p_atmo_sun(l_rd, l_sun, 0x1.3a92a4p-8f, l_soft), l_dim);
  if ((coax_at_u(i32(((l_b + 24i) + 3i))) > 0x1.000000p-1f))
  {
    return coax_p_pr_night_sky(coax_p_add3(coax_p_add3(l_sky, l_disc), l_night), l_rd, l_soft, 0.0f, l_b);
  }
  l_c = coax_p_atmo_clouds(l_rd, l_sun, l_sky, coax_at_u(i32(((l_b + 28i) + 3i))), 0x1.000000p0f, (coax_at_u(i32(((l_b + 12i) + 3i))) * 0x1.0624dep-8f));
  return coax_p_add3(coax_p_add3(coax_p_scale3(coax_p_pr_night_sky(coax_p_add3(l_sky, l_disc), l_rd, l_soft, l_c.y, l_b), (0x1.000000p0f - l_c.y)), coax_p_pr_under_cloud(coax_p_mul3(l_c.x, coax_p_mix3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_dim, 0x1.800000p-1f)), l_b)), l_night);
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
fn coax_p_sw_node(p_h: f32, p_x: f32) -> f32 {
  let l_h: f32 = p_h;
  let l_x: f32 = p_x;
  var l_j: f32;
  if ((abs(l_x) <= l_h))
  {
    return (0x1.000000p3f + (((l_x + l_h) / (0x1.000000p1f * l_h)) * 0x1.c00000p2f));
  }
  l_j = coax_p_fmin((log((0x1.000000p0f + (((abs(l_x) - l_h) * (0x1.99999ap0f - 0x1.000000p0f)) / 0x1.900000p4f))) / log(0x1.99999ap0f)), 0x1.000000p3f);
  if ((l_x < 0.0f))
  {
    return (0x1.000000p3f - l_j);
  }
  return ((0x1.000000p3f + 0x1.c00000p2f) + l_j);
}
fn coax_p_sw_gains(p_n: u32) -> vec2f {
  let l_n: u32 = p_n;
  var l_w: u32;
  var l_b: u32;
  l_w = coax_p_wg_get_u4(coax_p_wg_at4u(((i32((l_n >> u32(4u))) * 4i) + 16036i)), i32(((l_n >> u32(2u)) & 3u)));
  l_b = ((l_w >> u32((8u * (l_n & 3u)))) & 255u);
  return coax_p_v2((f32((l_b >> u32(4u))) / 0x1.e00000p3f), (f32((l_b & 15u)) / 0x1.e00000p3f));
}
fn coax_p_slab_photo_mirror_sky(p_r: vec3f) -> vec3f {
  let l_r: vec3f = p_r;
  return coax_p_pr_sky(l_r, 15872i);
}
fn coax_p_slab_sea_lin(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_v3((l_c.x * l_c.x), (l_c.y * l_c.y), (l_c.z * l_c.z));
}
fn coax_p_slab_photo_sun() -> vec3f {
  return coax_p_pr_at3(15892i);
}
fn coax_p_slab_sea_sun() -> vec3f {
  return coax_p_slab_sea_lin(coax_p_sb_p(4i));
}
fn coax_p_sw_short(p_p: vec2f) -> vec2f {
  let l_p: vec2f = p_p;
  var l_q: vec2f;
  var l_g: vec2f;
  if ((coax_at_u(i32(15992i)) < 0x1.000000p-1f))
  {
    return coax_p_v2(0x1.000000p0f, 0x1.000000p0f);
  }
  l_q = coax_p_sw_frame(l_p);
  l_g = coax_p_sw_calm(l_q);
  return coax_p_v2(((0x1.000000p-2f + (0x1.800000p-1f * l_g.y)) * (0x1.000000p0f - (0x1.666666p-2f * (coax_p_sw_wake(l_q)).x))), coax_p_smoothstep(0x1.ccccccp-2f, 0x1.99999ap-1f, l_g.x));
}
fn coax_p_slab_rain_rings(p_p: vec2f, p_t: f32, p_rain: f32, p_fp: f32) -> vec2f {
  let l_p: vec2f = p_p;
  let l_t: f32 = p_t;
  let l_rain: f32 = p_rain;
  let l_fp: f32 = p_fp;
  var l_near: f32;
  var l_rate: f32;
  var l_s: vec2f;
  l_near = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.eb851ep-4f, l_fp));
  if (((l_rain <= 0x1.47ae14p-7f) || (l_near <= 0.0f)))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_rate = (0x1.400000p1f * l_rain);
  l_s = coax_p_wg_add_f2f2(coax_p_slab_rain_layer(l_p, l_t, l_rate, 0.0f), coax_p_slab_rain_layer(coax_p_wg_add_f2f2(l_p, coax_p_v2(0x1.ae147ap-3f, 0x1.0a3d70p-3f)), l_t, l_rate, 0x1.100000p4f));
  return coax_p_wg_mul_f2f(l_s, l_near);
}
fn coax_p_slab_rain_mss(p_rain: f32) -> f32 {
  let l_rain: f32 = p_rain;
  return (0x1.89374cp-8f * coax_p_clampf(l_rain, 0.0f, 0x1.000000p0f));
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
fn coax_p_slab_foam_white(p_p: vec2f, p_t: f32, p_fp: f32, p_dist: f32, p_wind: vec2f, p_spin: f32, p_gate: f32, p_cover: f32) -> vec2f {
  let l_p: vec2f = p_p;
  let l_t: f32 = p_t;
  let l_fp: f32 = p_fp;
  let l_dist: f32 = p_dist;
  let l_wind: vec2f = p_wind;
  let l_spin: f32 = p_spin;
  let l_gate: f32 = p_gate;
  let l_cover: f32 = p_cover;
  var l_tex: f32;
  var l_o: vec2f;
  var l_wa: f32;
  var l_wc: f32;
  var l_fa: vec2f;
  var l_wq: vec2f;
  var l_wp: vec2f;
  var l_mean: vec2f;
  var l_far: f32;
  var l_zoom: f32;
  var l_rank: f32;
  var l_bend: f32;
  var l_tri: f32;
  var l_gap: f32;
  var l_soft: f32;
  var l_cap: f32;
  var l_lane: f32;
  var l_m: f32;
  var l_nn: f32;
  var l_scales: f32;
  var l_phase: f32;
  var l_l8: f32;
  var l_l64: f32;
  var l_streak: f32;
  var l_lr: f32;
  var l_white: f32;
  var l_up: vec2f;
  var l_torn: f32;
  var l_smoke: f32;
  l_tex = coax_p_slab_foam_texel_m();
  if ((l_tex <= 0.0f))
  {
    return coax_p_v2(l_cover, 0.0f);
  }
  l_o = coax_p_v2((l_p.x - frame[2i]), (l_p.y - frame[3i]));
  l_wa = ((l_o.x * l_wind.x) + (l_o.y * l_wind.y));
  l_wc = ((l_o.y * l_wind.x) - (l_o.x * l_wind.y));
  l_fa = coax_p_v2(0.0f, 0.0f);
  if ((frame[0i] < 0x1.900000p7f))
  {
    l_wq = coax_p_wg_div_f2f(l_o, (0x1.800000p1f * l_tex));
    l_wp = coax_p_v2((l_p.x + ((coax_p_slab_noise(l_wq) - 0x1.000000p-1f) * (0x1.800000p1f * l_tex))), (l_p.y + ((coax_p_slab_noise(coax_p_v2((l_wq.x + 0x1.266666p3f), (l_wq.y + 0x1.066666p2f))) - 0x1.000000p-1f) * (0x1.800000p1f * l_tex))));
    l_fa = coax_p_slab_foam_at(l_wp);
  }
  else
  {
    l_fa = coax_p_slab_foam_at(l_p);
  }
  l_mean = coax_p_slab_foam_mean();
  l_far = coax_p_smoothstep((0x1.333334p-1f * l_tex), (0x1.800000p1f * l_tex), l_fp);
  l_fa = coax_p_slab_foam_mix2(l_fa, l_mean, l_far);
  l_zoom = coax_p_clampf((0x1.000000p1f / coax_p_fmax(l_fp, 0x1.0624dep-10f)), coax_p_fmin(0x1.000000p0f, ((0x1.000000p-1f * l_tex) / 0x1.800000p3f)), 0x1.000000p0f);
  l_rank = coax_p_slab_foam_rank((l_wa * l_zoom), (l_wc * l_zoom));
  if ((frame[0i] < 0x1.900000p7f))
  {
    l_bend = coax_p_slab_noise(coax_p_v2((l_wa * 0x1.eb851ep-6f), (l_wc * 0x1.47ae14p-5f)));
    l_tri = (abs((coax_p_fract(((l_wa / 0x1.c00000p3f) + (0x1.800000p0f * l_bend))) - 0x1.000000p-1f)) * 0x1.000000p1f);
    l_gap = coax_p_slab_noise(coax_p_v2(((l_wa * 0x1.99999ap-4f) + 0x1.400000p2f), (l_wc * 0x1.1eb852p-3f)));
    l_rank = ((0x1.333334p-1f * l_tri) + (0x1.99999ap-2f * l_gap));
  }
  l_soft = (0x1.99999ap-5f + (0x1.99999ap-4f * (0x1.000000p0f - l_fa.x)));
  l_cap = (coax_p_slab_foam_show(l_fa.x, l_rank, l_soft, l_fp, (0x1.800000p0f / l_zoom)) * (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_smoothstep(0x1.333334p-3f, 0x1.666666p-1f, l_fa.x))));
  l_lane = 0.0f;
  if ((l_fa.y > 0x1.0624dep-8f))
  {
    l_m = frame[4i];
    l_nn = frame[5i];
    l_scales = frame[6i];
    l_phase = (((l_m * (l_p.x - frame[2i])) + (l_nn * (l_p.y - frame[3i]))) / frame[1i]);
    l_l8 = (abs((coax_p_fract((l_phase * l_scales)) - 0x1.000000p-1f)) * 0x1.000000p1f);
    l_l64 = (abs((coax_p_fract(((l_phase * l_scales) * l_scales)) - 0x1.000000p-1f)) * 0x1.000000p1f);
    l_streak = coax_p_slab_noise(coax_p_v2((l_wa * 0x1.1eb852p-5f), (l_wc * 0x1.333334p-1f)));
    l_lr = coax_p_clampf(((((0x1.000000p0f - (l_l8 * l_l8)) * 0x1.ccccccp-2f) + ((0x1.000000p0f - l_l64) * 0x1.000000p-2f)) + (l_streak * 0x1.333334p-2f)), 0.0f, 0x1.000000p0f);
    l_lane = (coax_p_slab_foam_show(l_fa.y, (0x1.000000p0f - l_lr), 0x1.eb851ep-4f, l_fp, 0x1.333334p0f) * 0x1.b33334p-1f);
  }
  l_white = coax_p_fmax(l_cap, l_lane);
  if ((l_spin > 0.0f))
  {
    l_up = coax_p_slab_foam_at(coax_p_wg_sub_f2f2(l_p, coax_p_wg_mul_f2f(l_wind, 0x1.c00000p2f)));
    l_torn = coax_p_smoothstep(0x1.666666p-2f, 0x1.ccccccp-1f, coax_p_fmax(l_fa.x, l_up.x));
    l_smoke = coax_p_slab_noise(coax_p_v2(((l_wa * 0x1.99999ap-5f) - (l_t * 0x1.19999ap0f)), (l_wc * 0x1.666666p-2f)));
    l_white = coax_p_fmax(l_white, ((l_spin * l_torn) * (coax_p_smoothstep(0x1.333334p-2f, 0x1.800000p-1f, l_smoke) * (0x1.99999ap-2f + (0x1.99999ap-2f * l_gate)))));
  }
  return coax_p_v2(coax_p_clampf(l_white, 0.0f, 0x1.000000p0f), l_fa.x);
}
fn coax_p_slab_sea_foam_lit(p_ndl: f32) -> vec3f {
  let l_ndl: f32 = p_ndl;
  return coax_p_sb_mul(coax_p_sb_add(coax_p_sb_mul(coax_p_slab_sea_top_sun(), (0x1.333334p-2f + (0x1.666666p-1f * l_ndl))), coax_p_slab_sea_top_skylight()), 0x1.8f5c28p-1f);
}
fn coax_p_slab_sea_top_air(p_c: vec3f, p_d: vec3f, p_dist: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  if (coax_p_slab_photo())
  {
    return coax_p_slab_photo_air(l_c, l_d, l_dist);
  }
  return l_c;
}
fn coax_p_slab_sea_top_out(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  if (coax_p_slab_photo())
  {
    return coax_p_slab_photo_show(l_c);
  }
  return coax_p_slab_sea_out(l_c);
}
fn coax_p_sw_white(p_p: vec2f, p_t: f32) -> f32 {
  let l_p: vec2f = p_p;
  let l_t: f32 = p_t;
  var l_mode: f32;
  var l_q: vec2f;
  var l_hs: f32;
  var l_dd: vec2f;
  var l_da: f32;
  var l_dc: f32;
  var l_rag: f32;
  var l_white: f32;
  var l_nd: i32;
  var l_k: i32;
  var l_b: i32;
  var l_c: vec2f;
  var l_rel: vec2f;
  var l_out: f32;
  var l_ang: f32;
  var l_cell: f32;
  var l_foam: f32;
  l_mode = coax_at_u(i32(15992i));
  if ((l_mode < 0x1.000000p-1f))
  {
    return 0.0f;
  }
  l_q = coax_p_sw_frame(l_p);
  l_hs = coax_at_u(i32(16030i));
  l_dd = coax_p_sw_turn(coax_p_v2(coax_at_u(i32(16012i)), coax_at_u(i32(16013i))));
  l_da = ((l_q.x * l_dd.x) + (l_q.y * l_dd.y));
  l_dc = ((l_q.y * l_dd.x) - (l_q.x * l_dd.y));
  l_rag = ((coax_p_slab_noise(coax_p_v2(((l_da * 0x1.99999ap-5f) - (l_t * 0x1.99999ap-5f)), (l_dc * 0x1.c28f5cp-3f))) * 0x1.333334p-1f) + (coax_p_slab_noise(coax_p_v2(((l_da * 0x1.ae147ap-3f) - (l_t * 0x1.99999ap-3f)), ((l_dc * 0x1.333334p-1f) + 0x1.800000p1f))) * 0x1.99999ap-2f));
  l_white = 0.0f;
  if ((l_mode > 0x1.800000p0f))
  {
    l_nd = select(1i, 7i, (coax_at_u(i32(1500i)) > 0x1.400000p1f));
    l_k = 0i;
    while ((l_k < l_nd))
    {
      l_b = (96i + (l_k * 8i));
      l_c = coax_p_sw_frame(coax_p_v2(((coax_at_u(i32(l_b)) + coax_at_u(i32((l_b + 4i)))) * 0x1.000000p-1f), ((coax_at_u(i32((l_b + 2i))) + coax_at_u(i32((l_b + 6i)))) * 0x1.000000p-1f)));
      if ((l_nd == 1i))
      {
        l_c = coax_p_v2(0.0f, 0.0f);
      }
      l_rel = coax_p_v2((l_q.x - l_c.x), (l_q.y - l_c.y));
      l_out = (coax_p_wg_length2(l_rel) - (0x1.000000p-1f * (coax_at_u(i32((l_b + 4i))) - coax_at_u(i32(l_b)))));
      if (((l_out > (-0x1.000000p0f)) && (l_out < 0x1.e00000p4f)))
      {
        l_ang = coax_p_wg_atan2(l_rel.y, l_rel.x);
        l_cell = floor((coax_p_fract(((l_ang / 0x1.921fb6p2f) + 0x1.000000p0f)) * 0x1.000000p4f));
        l_foam = coax_p_sw_byte(16180i, (u32((l_k * 16i)) + coax_p_wg_u32_f(coax_p_fmin(l_cell, 0x1.e00000p3f))));
        l_white = coax_p_fmax(l_white, (l_foam * (0x1.000000p0f - coax_p_smoothstep((0x1.000000p0f + ((0x1.99999ap-1f * l_hs) * l_rag)), (0x1.800000p1f + (0x1.99999ap0f * l_hs)), l_out))));
      }
      l_k = (l_k + 1i);
    }
    return coax_p_clampf((l_white * (0x1.19999ap-1f + (0x1.ccccccp-1f * l_rag))), 0.0f, 0x1.000000p0f);
  }
  return 0.0f;
}
fn coax_p_sw_foot_white(p_p: vec2f, p_t: f32, p_fp: f32) -> f32 {
  let l_p: vec2f = p_p;
  let l_t: f32 = p_t;
  let l_fp: f32 = p_fp;
  var l_q: vec2f;
  var l_hs: f32;
  var l_dd: vec2f;
  var l_da: f32;
  var l_dc: f32;
  var l_rag: f32;
  var l_hx: f32;
  var l_hz: f32;
  var l_width: f32;
  var l_reach: f32;
  var l_cover: f32;
  var l_k: i32;
  var l_n: vec2f;
  var l_tg: vec2f;
  var l_half: f32;
  var l_span: f32;
  var l_out: f32;
  var l_along: f32;
  var l_slide: f32;
  var l_e: f32;
  var l_corner: vec2f;
  var l_s: f32;
  var l_y: f32;
  var l_w: f32;
  var l_hold: f32;
  var l_g: f32;
  l_q = coax_p_sw_frame(l_p);
  l_hs = coax_at_u(i32(16030i));
  l_dd = coax_p_sw_turn(coax_p_v2(coax_at_u(i32(16012i)), coax_at_u(i32(16013i))));
  l_da = ((l_q.x * l_dd.x) + (l_q.y * l_dd.y));
  l_dc = ((l_q.y * l_dd.x) - (l_q.x * l_dd.y));
  l_rag = coax_p_slab_foam_rank(l_da, l_dc);
  l_hx = coax_at_u(i32(16028i));
  l_hz = coax_at_u(i32(16029i));
  l_width = (0x1.000000p2f + (0x1.000000p1f * l_hs));
  l_reach = coax_p_fmax(coax_at_u(i32(16015i)), 0x1.400000p4f);
  l_cover = 0.0f;
  l_k = 0i;
  while ((l_k < 4i))
  {
    l_n = coax_p_sw_normal(l_k);
    l_tg = coax_p_v2((-l_n.y), l_n.x);
    l_half = select(l_hz, l_hx, ((l_k == 0i) || (l_k == 2i)));
    l_span = select((0x1.000000p1f * l_hx), (0x1.000000p1f * l_hz), ((l_k == 0i) || (l_k == 2i)));
    l_out = (((l_q.x * l_n.x) + (l_q.y * l_n.y)) - l_half);
    l_along = ((l_q.x * l_tg.x) + (l_q.y * l_tg.y));
    if ((((l_out > (-0x1.800000p0f)) && (l_out < l_width)) && (abs(l_along) < ((0x1.000000p-1f * l_span) + l_width))))
    {
      l_cover = coax_p_fmax(l_cover, (coax_p_sw_foot(l_k, l_along, l_span) * (0x1.000000p0f - coax_p_smoothstep((0x1.333334p-2f * l_width), l_width, l_out))));
    }
    l_slide = ((l_dd.x * l_tg.x) + (l_dd.y * l_tg.y));
    if ((abs(l_slide) > 0x1.99999ap-5f))
    {
      l_e = select((-0x1.000000p0f), 0x1.000000p0f, (l_slide > 0.0f));
      l_corner = coax_p_v2(((l_n.x * l_half) + (((l_tg.x * l_e) * 0x1.000000p-1f) * l_span)), ((l_n.y * l_half) + (((l_tg.y * l_e) * 0x1.000000p-1f) * l_span)));
      l_s = (((l_q.x - l_corner.x) * l_dd.x) + ((l_q.y - l_corner.y) * l_dd.y));
      if (((l_s > 0.0f) && (l_s < (0x1.000000p2f * l_reach))))
      {
        l_y = (((l_q.y - l_corner.y) * l_dd.x) - ((l_q.x - l_corner.x) * l_dd.y));
        l_w = (0x1.400000p2f + (0x1.99999ap-4f * l_s));
        l_hold = (((coax_p_sw_foot(l_k, ((l_e * 0x1.e147aep-2f) * l_span), l_span) + coax_p_sw_foot(l_k, ((l_e * 0x1.99999ap-2f) * l_span), l_span)) + coax_p_sw_foot(l_k, ((l_e * 0x1.333334p-2f) * l_span), l_span)) / 0x1.800000p1f);
        l_g = (l_y / l_w);
        l_cover = coax_p_fmax(l_cover, ((((0x1.333334p-1f * l_hold) * exp(((-l_s) / l_reach))) * exp((-(l_g * l_g)))) * abs(l_slide)));
      }
    }
    l_k = (l_k + 1i);
  }
  return coax_p_sw_cover(l_cover, l_rag, l_fp);
}
fn coax_p_sw_plume_at(p_q: vec2f, p_k: i32) -> vec2f {
  let l_q: vec2f = p_q;
  let l_k: i32 = p_k;
  var l_f: vec2f;
  var l_ox: f32;
  var l_oz: f32;
  var l_x: f32;
  var l_y: f32;
  var l_w: f32;
  var l_tt: f32;
  var l_depth: f32;
  var l_g: f32;
  l_f = coax_p_sw_turn(coax_p_v2(coax_at_u(i32(16016i)), coax_at_u(i32(16017i))));
  l_ox = (-0x1.c20000p8f);
  l_oz = ((-0x1.c20000p7f) + (0x1.2c0000p7f * f32(l_k)));
  l_x = (((l_q.x - l_ox) * l_f.x) + ((l_q.y - l_oz) * l_f.y));
  l_y = (((l_q.y - l_oz) * l_f.x) - ((l_q.x - l_ox) * l_f.y));
  if ((l_x < (-0x1.400000p4f)))
  {
    return coax_p_v2(0.0f, coax_at_u(i32(16021i)));
  }
  l_w = (0x1.e00000p4f + (0x1.99999ap-3f * coax_p_fmax(l_x, 0.0f)));
  l_tt = coax_p_clampf((l_x / coax_p_fmax(coax_at_u(i32(16023i)), 0x1.000000p0f)), 0.0f, 0x1.000000p0f);
  l_depth = (coax_at_u(i32(16021i)) + (((coax_at_u(i32(16022i)) * l_tt) * l_tt) * (0x1.800000p1f - (0x1.000000p1f * l_tt))));
  l_g = (l_y / (0x1.000000p-1f * l_w));
  return coax_p_v2((((((0x1.e00000p4f / l_w) * exp((-(l_g * l_g)))) * coax_p_smoothstep((-0x1.400000p4f), 0x1.400000p3f, l_x)) * exp(((-coax_p_fmax(l_x, 0.0f)) / 0x1.c20000p9f))) * coax_at_u(i32(16020i))), l_depth);
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
fn coax_p_wg_min_i(p_a: i32, p_b: i32) -> i32 {
  let l_a: i32 = p_a;
  let l_b: i32 = p_b;
  return select(l_a, l_b, (l_b < l_a));
}
fn coax_p_pr_resolve(p_size: f32, p_aa: f32) -> f32 {
  let l_size: f32 = p_size;
  let l_aa: f32 = p_aa;
  return coax_p_saturate(((l_size / (coax_p_fmax(l_aa, 0x1.a36e2ep-14f) * 0x1.800000p1f)) - 0x1.000000p-1f));
}
fn coax_p_pr_room(p_f: vec2f, p_bay: f32, p_floor_h: f32, p_seed: f32, p_ax: f32, p_ay: f32, p_b: i32) -> vec3f {
  let l_f: vec2f = p_f;
  let l_bay: f32 = p_bay;
  let l_floor_h: f32 = p_floor_h;
  let l_seed: f32 = p_seed;
  let l_ax: f32 = p_ax;
  let l_ay: f32 = p_ay;
  let l_b: i32 = p_b;
  var l_share: f32;
  var l_py: f32;
  var l_px: f32;
  var l_fl: f32;
  var l_busy: f32;
  var l_h: f32;
  var l_on: f32;
  var l_hue: f32;
  var l_c: vec3f;
  var l_v: f32;
  var l_inside: f32;
  var l_dim: f32;
  var l_near: vec3f;
  var l_far: vec3f;
  l_share = coax_at_u(i32((l_b + 79i)));
  if ((l_share <= 0.0f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_py = floor((l_f.y / l_floor_h));
  l_px = floor((l_f.x / l_bay));
  l_fl = coax_p_render_noise2(((l_py * 0x1.b33334p0f) + (l_seed * 0x1.200000p3f)), ((l_seed * 0x1.800000p1f) + 0x1.600000p3f));
  l_busy = (0x1.666666p-2f + (0x1.4cccccp0f * l_fl));
  l_h = coax_p_render_noise2(((l_px * 0x1.4f5c28p0f) + (l_seed * 0x1.a00000p3f)), ((l_py * 0x1.8a3d70p-1f) - (l_seed * 0x1.400000p2f)));
  l_on = coax_p_stepf(l_h, (l_share * l_busy));
  l_hue = coax_p_render_noise2(((l_px * 0x1.7ae148p-2f) + l_seed), (l_py * 0x1.0cccccp1f));
  l_c = select(select(coax_p_v3(0x1.a3d70ap-1f, 0x1.ccccccp-1f, 0x1.000000p0f), coax_p_v3(0x1.000000p0f, 0x1.a3d70ap-1f, 0x1.3d70a4p-1f), (l_hue < 0x1.99999ap-1f)), coax_p_v3(0x1.000000p0f, 0x1.51eb86p-1f, 0x1.851eb8p-2f), (l_hue < 0x1.ccccccp-2f));
  l_v = coax_p_fract((l_f.y / l_floor_h));
  l_inside = ((coax_p_smoothstep(0x1.99999ap-4f, 0x1.99999ap-3f, l_v) * (0x1.000000p0f - coax_p_smoothstep(0x1.b851ecp-1f, 0x1.dc28f6p-1f, l_v))) * (0x1.333334p-1f + (0x1.000000p-1f * l_v)));
  l_dim = coax_p_fract((l_h * 0x1.d33334p2f));
  l_near = coax_p_scale3(l_c, ((l_on * l_inside) * (0x1.99999ap-3f + ((0x1.19999ap0f * l_dim) * l_dim))));
  l_far = coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.8f5c28p-1f, 0x1.1eb852p-1f), (l_share * 0x1.333334p-2f));
  return coax_p_scale3(coax_p_mix3(l_far, l_near, coax_p_saturate(((coax_p_fmin(l_bay, l_floor_h) / (coax_p_fmax(coax_p_fmax(l_ax, l_ay), 0x1.a36e2ep-14f) * 0x1.333334p0f)) - 0x1.99999ap-2f))), 0x1.99999ap-6f);
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
fn coax_p_wo_exp3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_v3(exp(l_a.x), exp(l_a.y), exp(l_a.z));
}
fn coax_p_sw_byte(p_base: i32, p_i: u32) -> f32 {
  let l_base: i32 = p_base;
  let l_i: u32 = p_i;
  var l_w: u32;
  l_w = coax_p_wg_get_u4(coax_p_wg_at4u(((i32((l_i >> u32(4u))) * 4i) + l_base)), i32(((l_i >> u32(2u)) & 3u)));
  return (f32(((l_w >> u32((8u * (l_i & 3u)))) & 255u)) / 0x1.fe0000p7f);
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
fn coax_p_pr_air_light(p_rd: vec3f, p_sun: vec3f, p_hz: f32) -> vec3f {
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  let l_hz: f32 = p_hz;
  var l_up: f32;
  var l_cos_t: f32;
  var l_day: f32;
  var l_extra: vec3f;
  var l_clear: vec3f;
  var l_all: vec3f;
  var l_reach: vec3f;
  var l_mass: f32;
  var l_took: vec3f;
  var l_air: vec3f;
  var l_forward: f32;
  var l_glow: f32;
  var l_hazy: vec3f;
  var l_first: f32;
  var l_once: vec3f;
  var l_low: f32;
  var l_again: vec3f;
  l_up = coax_p_saturate(l_rd.y);
  l_cos_t = dot(l_rd, l_sun);
  l_day = coax_p_atmo_day(l_sun.y);
  l_extra = coax_p_pr_haze_loss(l_hz);
  l_clear = coax_p_v3((0x1.916872p-6f + 0x1.89374cp-7f), (0x1.d3c362p-5f + 0x1.89374cp-7f), (0x1.1eb852p-3f + 0x1.89374cp-7f));
  l_all = coax_p_add3(l_clear, l_extra);
  l_reach = coax_p_atmo_exp3(coax_p_scale3(l_all, (0.0f - (coax_p_atmo_sun_mass(l_sun.y) * coax_p_mix(0x1.000000p0f, 0x1.333334p-2f, sqrt(l_up))))));
  l_mass = coax_p_atmo_mass(l_up);
  l_took = coax_p_v3((0x1.000000p0f - exp((0.0f - (l_all.x * l_mass)))), (0x1.000000p0f - exp((0.0f - (l_all.y * l_mass)))), (0x1.000000p0f - exp((0.0f - (l_all.z * l_mass)))));
  l_air = coax_p_scale3(coax_p_v3((0x1.916872p-6f / l_all.x), (0x1.d3c362p-5f / l_all.y), (0x1.1eb852p-3f / l_all.z)), coax_p_atmo_even(l_cos_t));
  l_forward = coax_p_atmo_forward(l_cos_t);
  l_glow = (0x1.5c28f6p-3f * (l_forward + ((0x1.666666p1f * exp((0.0f - (coax_p_fmax((0x1.000000p0f - l_cos_t), 0.0f) * 0x1.19999ap1f)))) * exp((0.0f - (l_up * 0x1.c00000p2f))))));
  l_hazy = coax_p_scale3(coax_p_v3((l_extra.x / l_all.x), (l_extra.y / l_all.y), (l_extra.z / l_all.z)), (0x1.ccccccp-1f * (0x1.99999ap-2f + (0x1.333334p-1f * l_forward))));
  l_first = coax_p_mix(coax_p_smoothstep((0.0f - 0x1.1eb852p-3f), 0x1.47ae14p-6f, l_sun.y), 0x1.000000p0f, exp((0.0f - (l_up * 0x1.800000p2f))));
  l_once = coax_p_scale3(coax_p_mul3(coax_p_mul3(l_reach, l_took), coax_p_add3(coax_p_add3(l_air, coax_p_v3(l_glow, l_glow, l_glow)), l_hazy)), l_first);
  l_low = (0x1.000000p0f - coax_p_saturate(((l_sun.y * 0x1.400000p2f) + 0x1.333334p-3f)));
  l_again = coax_p_scale3(coax_p_mul3(coax_p_mul3(coax_p_v3(0x1.eb851ep-6f, 0x1.1eb852p-4f, 0x1.ae147ap-3f), l_took), coax_p_v3((l_clear.x / l_all.x), (l_clear.y / l_all.y), (l_clear.z / l_all.z))), coax_p_mix(0x1.000000p0f, (0x1.4cccccp1f * coax_p_mix(0x1.eb851ep-4f, 0x1.000000p0f, coax_p_smoothstep(0.0f, 0x1.ccccccp-2f, l_up))), l_low));
  return coax_p_scale3(coax_p_add3(l_once, l_again), (0x1.8cccccp0f * l_day));
}
fn coax_p_pr_cloud_shade(p_b: i32) -> f32 {
  let l_b: i32 = p_b;
  return coax_p_smoothstep(0x1.ccccccp-2f, 0x1.ccccccp-1f, coax_at_u(i32(((l_b + 28i) + 3i))));
}
fn coax_p_film_rec709(p_linear: vec3f) -> vec3f {
  let l_linear: vec3f = p_linear;
  var l_c: vec3f;
  var l_lo: f32;
  var l_hi: f32;
  var l_saturation: f32;
  var l_chroma: f32;
  var l_brightness: f32;
  var l_shape: f32;
  var l_triangle: f32;
  var l_sign: f32;
  var l_gain: f32;
  var l_glow: f32;
  var l_hue: f32;
  var l_red: f32;
  l_c = coax_p_film_ap0(coax_p_film_in(l_linear));
  l_lo = coax_p_film_min(l_c.x, coax_p_film_min(l_c.y, l_c.z));
  l_hi = coax_p_film_max(l_c.x, coax_p_film_max(l_c.y, l_c.z));
  l_saturation = ((coax_p_film_max(l_hi, 0x1.b7cdfep-34f) - coax_p_film_max(l_lo, 0x1.b7cdfep-34f)) / coax_p_film_max(l_hi, 0x1.47ae14p-7f));
  l_chroma = sqrt(coax_p_film_max(0.0f, (((l_c.z * (l_c.z - l_c.y)) + (l_c.y * (l_c.y - l_c.x))) + (l_c.x * (l_c.x - l_c.z)))));
  l_brightness = ((((l_c.x + l_c.y) + l_c.z) + (0x1.c00000p0f * l_chroma)) / 0x1.800000p1f);
  l_shape = ((l_saturation - 0x1.99999ap-2f) / 0x1.99999ap-3f);
  l_triangle = coax_p_film_max((0x1.000000p0f - abs((l_shape * 0x1.000000p-1f))), 0.0f);
  l_sign = select(select(0.0f, (-0x1.000000p0f), (l_shape < 0.0f)), 0x1.000000p0f, (l_shape > 0.0f));
  l_gain = (0x1.99999ap-6f * (0x1.000000p0f + (l_sign * (0x1.000000p0f - (l_triangle * l_triangle)))));
  l_glow = select(select((l_gain * ((0x1.47ae14p-4f / l_brightness) - 0x1.000000p-1f)), 0.0f, (l_brightness >= 0x1.47ae14p-3f)), l_gain, (l_brightness <= 0x1.b4e81cp-5f));
  l_c = coax_p_film_scale(l_c, (0x1.000000p0f + l_glow));
  l_hue = ((coax_p_film_atan2((sqrt(0x1.800000p1f) * (l_c.y - l_c.z)), (((0x1.000000p1f * l_c.x) - l_c.y) - l_c.z)) * 0x1.680000p7f) / 0x1.921fb6p1f);
  l_red = coax_p_film_sat((0x1.000000p0f - abs(((0x1.000000p1f * l_hue) / 0x1.0e0000p7f))));
  l_red = ((l_red * l_red) * (0x1.800000p1f - (0x1.000000p1f * l_red)));
  l_c.x = (l_c.x + ((((l_red * l_red) * l_saturation) * (0x1.eb851ep-6f - l_c.x)) * 0x1.70a3d8p-3f));
  l_c = coax_p_film_ap1(l_c);
  l_c = coax_p_film_rgb(coax_p_film_max(l_c.x, 0.0f), coax_p_film_max(l_c.y, 0.0f), coax_p_film_max(l_c.z, 0.0f));
  l_c = coax_p_film_mix_grey(l_c, 0x1.eb851ep-1f);
  l_c = coax_p_film_rgb(coax_p_film_curve(l_c.x), coax_p_film_curve(l_c.y), coax_p_film_curve(l_c.z));
  l_c = coax_p_film_mix_grey(l_c, 0x1.dc28f6p-1f);
  l_c = coax_p_film_out(l_c);
  return coax_p_film_rgb(coax_p_film_max(l_c.x, 0.0f), coax_p_film_max(l_c.y, 0.0f), coax_p_film_max(l_c.z, 0.0f));
}
fn coax_p_film_luma(p_c: vec3f) -> f32 {
  let l_c: vec3f = p_c;
  return (((l_c.x * 0x1.b367a0p-3f) + (l_c.y * 0x1.6e2eb2p-1f)) + (l_c.z * 0x1.27bb30p-4f));
}
fn coax_p_film_max(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_film_sat(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_film_min(0x1.000000p0f, coax_p_film_max(0.0f, l_x));
}
fn coax_p_film_scale(p_c: vec3f, p_s: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_s: f32 = p_s;
  return coax_p_film_rgb((l_c.x * l_s), (l_c.y * l_s), (l_c.z * l_s));
}
fn coax_p_pr_encode1(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  var l_c: f32;
  l_c = coax_p_fmax(l_x, 0.0f);
  return select(((0x1.0e147ap0f * coax_pow_bg_vs(l_c, (0x1.000000p0f / 0x1.333334p1f))) - 0x1.c28f5cp-5f), (l_c * 0x1.9d70a4p3f), (l_c <= 0x1.9a5c38p-9f));
}
fn coax_p_pr_post_z(p_depth: f32, p_b: i32) -> f32 {
  let l_depth: f32 = p_depth;
  let l_b: i32 = p_b;
  return (coax_at_u(i32(((l_b + 4i) + 3i))) / coax_p_fmax((0x1.000000p0f - l_depth), 0x1.ad7f2ap-24f));
}
fn coax_p_pr_post_at(p_x: f32, p_y: f32, p_z: f32, p_w: f32, p_h: f32, p_b: i32) -> vec3f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  let l_w: f32 = p_w;
  let l_h: f32 = p_h;
  let l_b: i32 = p_b;
  var l_t: f32;
  var l_nx: f32;
  var l_ny: f32;
  var l_ray: vec3f;
  l_t = coax_at_u(i32(((l_b + 0i) + 3i)));
  l_nx = (((l_x / l_w) * 0x1.000000p1f) - 0x1.000000p0f);
  l_ny = (0x1.000000p0f - ((l_y / l_h) * 0x1.000000p1f));
  l_ray = coax_p_add3(coax_p_pr_at3((l_b + 4i)), coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_b + 8i)), ((l_nx * l_t) * coax_at_u(i32(((l_b + 8i) + 3i))))), coax_p_scale3(coax_p_pr_at3((l_b + 12i)), (l_ny * l_t))));
  return coax_p_add3(coax_p_pr_at3((l_b + 0i)), coax_p_scale3(l_ray, l_z));
}
fn coax_p_wg_atu(p_at: i32) -> u32 {
  let l_at: i32 = p_at;
  return bitcast<u32>(coax_at_u(i32(l_at)));
}
fn coax_p_u4(p_x: u32, p_y: u32, p_z: u32, p_w: u32) -> coax_t33 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  let l_z: u32 = p_z;
  let l_w: u32 = p_w;
  return coax_t33(l_x, l_y, l_z, l_w);
}
fn coax_p_slab_photo_skylight() -> vec3f {
  return coax_p_pr_sky_light(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), 15872i);
}
fn coax_p_slab_sea_skylight() -> vec3f {
  return coax_p_slab_sea_lin(coax_p_sb_mul(coax_p_sb_add(coax_p_sb_p(8i), coax_p_sb_p(12i)), 0x1.000000p-1f));
}
fn coax_p_pr_haze_through(p_sun_up: f32, p_hz: f32) -> vec3f {
  let l_sun_up: f32 = p_sun_up;
  let l_hz: f32 = p_hz;
  return coax_p_atmo_exp3(coax_p_scale3(coax_p_pr_haze_loss(l_hz), (0.0f - coax_p_atmo_sun_mass(l_sun_up))));
}
fn coax_p_atmo_sun(p_rd: vec3f, p_sun: vec3f, p_radius: f32, p_soft: f32) -> vec3f {
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  let l_radius: f32 = p_radius;
  let l_soft: f32 = p_soft;
  var l_light: vec3f;
  var l_d: vec3f;
  var l_flat: f32;
  var l_dv: f32;
  var l_a: f32;
  var l_r: f32;
  var l_w: f32;
  var l_disc: f32;
  var l_near: f32;
  var l_limb: f32;
  var l_glare: f32;
  if ((l_radius <= 0.0f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_light = coax_p_scale3(coax_p_atmo_sunlight(l_sun.y), coax_p_atmo_day(l_sun.y));
  l_d = coax_p_sub3(l_rd, l_sun);
  l_flat = (0x1.000000p0f / (0x1.000000p0f - (0x1.99999ap-3f * exp((0.0f - (coax_p_fmax(l_sun.y, 0.0f) * 0x1.c00000p3f))))));
  l_dv = (l_d.y * l_flat);
  l_a = sqrt(coax_p_fmax(((dot(l_d, l_d) - (l_d.y * l_d.y)) + (l_dv * l_dv)), 0.0f));
  l_r = (l_radius * (0x1.000000p0f + (0x1.000000p-1f * exp((0.0f - (coax_p_fmax(l_sun.y, 0.0f) * 0x1.200000p3f))))));
  l_w = coax_p_fmax(l_soft, (l_r * 0x1.99999ap-3f));
  l_disc = (0x1.000000p0f - coax_p_smoothstep((l_r - l_w), (l_r + l_w), l_a));
  l_near = (l_a / l_r);
  l_limb = (0x1.000000p0f - ((0x1.ccccccp-2f * coax_p_saturate(l_near)) * coax_p_saturate(l_near)));
  l_glare = ((0x1.000000p1f / (0x1.000000p0f + (((l_near * l_near) * l_near) * 0x1.ccccccp-1f))) + (0x1.000000p-1f * exp((0.0f - (l_near * 0x1.333334p-2f)))));
  return coax_p_scale3(l_light, (((0x1.800000p5f * l_disc) * l_limb) + l_glare));
}
fn coax_p_pr_night_sky(p_sky: vec3f, p_rd: vec3f, p_soft: f32, p_hide: f32, p_b: i32) -> vec3f {
  let l_sky: vec3f = p_sky;
  let l_rd: vec3f = p_rd;
  let l_soft: f32 = p_soft;
  let l_hide: f32 = p_hide;
  let l_b: i32 = p_b;
  var l_night: f32;
  var l_show: f32;
  var l_stars: vec3f;
  var l_c: vec3f;
  var l_face: vec2f;
  l_night = (0x1.000000p0f - coax_at_u(i32(((l_b + 16i) + 3i))));
  l_show = (0x1.000000p0f - l_hide);
  l_stars = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((l_night > 0x1.000000p-2f))
  {
    l_stars = coax_p_scale3(coax_p_pr_stars(l_rd), (((l_night - 0x1.000000p-2f) * 0x1.555326p0f) * l_show));
  }
  l_c = l_sky;
  if ((coax_at_u(i32(((l_b + 108i) + 3i))) > 0x1.000000p-1f))
  {
    l_face = coax_p_pr_moon_face(l_rd, coax_p_pr_at3((l_b + 108i)), coax_p_pr_at3((l_b + 16i)), l_soft);
    l_stars = coax_p_scale3(l_stars, (0x1.000000p0f - l_face.x));
    l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.99999ap-4f, 0x1.8d4fe0p-4f, 0x1.70a3d8p-4f), ((l_face.x * l_show) * (l_face.y + 0x1.0624dep-8f))));
  }
  return coax_p_add3(l_c, l_stars);
}
fn coax_p_atmo_clouds(p_rd: vec3f, p_sun: vec3f, p_sky: vec3f, p_cover: f32, p_scale: f32, p_drift: f32) -> coax_t34 {
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  let l_sky: vec3f = p_sky;
  let l_cover: f32 = p_cover;
  let l_scale: f32 = p_scale;
  let l_drift: f32 = p_drift;
  var l_o: coax_t34;
  var l_day: f32;
  var l_sunlight: vec3f;
  var l_fill: vec3f;
  var l_cos_t: f32;
  var l_size: f32;
  var l_k: f32;
  var l_x: f32;
  var l_y: f32;
  var l_thick: f32;
  var l_sl: f32;
  var l_toward: f32;
  var l_open: f32;
  var l_edge: f32;
  var l_through: f32;
  var l_heap: vec3f;
  var l_far: f32;
  var l_hide: f32;
  var l_hk: f32;
  var l_hx: f32;
  var l_hy: f32;
  var l_wisp: f32;
  var l_high: vec3f;
  l_o = coax_t34();
  if (((l_rd.y <= 0x1.0624dep-8f) || (l_cover <= 0.0f)))
  {
    return l_o;
  }
  l_day = coax_p_atmo_day(l_sun.y);
  l_sunlight = coax_p_scale3(coax_p_atmo_sunlight(l_sun.y), ((l_day * 0x1.800000p0f) * coax_p_smoothstep((0.0f - 0x1.1eb852p-4f), 0x1.47ae14p-6f, l_sun.y)));
  l_fill = coax_p_scale3(coax_p_add3(coax_p_atmo_sky(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_sun), coax_p_scale3(l_sky, 0x1.666666p-2f)), 0x1.ccccccp-1f);
  l_cos_t = dot(l_rd, l_sun);
  l_size = coax_p_fmax(l_scale, 0x1.99999ap-5f);
  l_k = (0x1.b33334p0f / (l_size * (l_rd.y + 0x1.47ae14p-3f)));
  l_x = ((l_rd.x * l_k) + (l_drift * 0x1.89374cp-8f));
  l_y = ((l_rd.z * l_k) + (l_drift * 0x1.47ae14p-9f));
  l_thick = coax_p_atmo_heap(l_x, l_y, l_cover);
  l_sl = sqrt(coax_p_fmax(((l_sun.x * l_sun.x) + (l_sun.z * l_sun.z)), 0x1.a36e2ep-14f));
  l_toward = coax_p_atmo_heap((l_x + ((l_sun.x / l_sl) * 0x1.c28f5cp-3f)), (l_y + ((l_sun.z / l_sl) * 0x1.c28f5cp-3f)), l_cover);
  l_open = (0x1.000000p0f - ((0x1.800000p-1f * l_toward) * coax_p_smoothstep(0.0f, 0x1.333334p-1f, l_thick)));
  l_edge = ((l_thick * (0x1.000000p0f - l_thick)) * 0x1.000000p2f);
  l_through = (l_edge * (0x1.000000p-2f + (0x1.99999ap0f * coax_pow_bg_vs(coax_p_saturate(l_cos_t), 0x1.800000p2f))));
  l_heap = coax_p_add3(coax_p_scale3(l_sunlight, ((0x1.99999ap-3f + (0x1.e66666p-1f * l_open)) + l_through)), coax_p_scale3(l_fill, (0x1.000000p-1f + (0x1.000000p-2f * (0x1.000000p0f - l_thick)))));
  l_far = coax_p_smoothstep(0x1.47ae14p-7f, 0x1.c28f5cp-4f, l_rd.y);
  l_heap = coax_p_mix3(l_sky, l_heap, (0x1.19999ap-1f + (0x1.ccccccp-2f * coax_p_smoothstep(0x1.47ae14p-7f, 0x1.47ae14p-3f, l_rd.y))));
  l_hide = (coax_p_smoothstep(0.0f, 0x1.19999ap-1f, l_thick) * l_far);
  l_hk = (0x1.800000p-1f / (l_size * (l_rd.y + 0x1.333334p-2f)));
  l_hx = ((((l_rd.x * 0x1.ae147ap-2f) + (l_rd.z * 0x1.333334p-2f)) * l_hk) + (l_drift * 0x1.0624dep-9f));
  l_hy = (((l_rd.z * 0x1.000000p0f) - (l_rd.x * 0x1.333334p-1f)) * l_hk);
  l_wisp = (((coax_p_smoothstep(0x1.0a3d70p-1f, 0x1.b851ecp-1f, coax_p_atmo_fbm2(l_hx, l_hy)) * 0x1.ae147ap-2f) * coax_p_smoothstep(0.0f, 0x1.99999ap-5f, l_cover)) * coax_p_smoothstep(0x1.eb851ep-6f, 0x1.99999ap-3f, l_rd.y));
  l_high = coax_p_add3(coax_p_scale3(l_sunlight, (0x1.800000p-1f + (0x1.ccccccp-1f * coax_pow_bg_vs(coax_p_saturate(l_cos_t), 0x1.000000p2f)))), coax_p_scale3(l_fill, 0x1.333334p-2f));
  l_o.y = (l_hide + ((0x1.000000p0f - l_hide) * l_wisp));
  l_o.x = coax_p_add3(coax_p_scale3(l_heap, l_hide), coax_p_scale3(l_high, ((0x1.000000p0f - l_hide) * l_wisp)));
  return l_o;
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
fn coax_p_slab_rain_layer(p_p: vec2f, p_t: f32, p_rate: f32, p_seed: f32) -> vec2f {
  let l_p: vec2f = p_p;
  let l_t: f32 = p_t;
  let l_rate: f32 = p_rate;
  let l_seed: f32 = p_seed;
  var l_q: vec2f;
  var l_cell: vec2f;
  var l_h: f32;
  var l_at: vec2f;
  var l_age: f32;
  var l_life: f32;
  var l_rel: vec2f;
  var l_r: f32;
  var l_front: f32;
  var l_x: f32;
  var l_wave: f32;
  l_q = coax_p_wg_div_f2f(l_p, 0x1.ccccccp-2f);
  l_cell = coax_p_wg_floor_f2(l_q);
  l_h = coax_p_slab_hash2(coax_p_wg_add_f2f2(l_cell, coax_p_v2(l_seed, (l_seed * 0x1.b33334p0f))));
  l_at = coax_p_v2((0x1.000000p-2f + (0x1.000000p-1f * coax_p_slab_hash2(coax_p_wg_add_f2f2(l_cell, coax_p_v2((l_seed + 0x1.8cccccp1f), 0x1.533334p2f))))), (0x1.000000p-2f + (0x1.000000p-1f * coax_p_slab_hash2(coax_p_wg_add_f2f2(l_cell, coax_p_v2(0x1.ecccccp2f, (l_seed + 0x1.e66666p0f)))))));
  l_age = (coax_p_fract(((l_t * l_rate) + l_h)) / l_rate);
  l_life = 0x1.333334p-1f;
  if ((l_age > l_life))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_rel = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_q, l_cell), l_at), 0x1.ccccccp-2f);
  l_r = coax_p_wg_length2(l_rel);
  l_front = (0x1.666666p-2f * l_age);
  l_x = ((l_r - l_front) / 0x1.eb851ep-6f);
  l_wave = ((cos((l_x * 0x1.921fa0p1f)) * exp(((-(l_x * l_x)) * 0x1.000000p-1f))) * (0x1.000000p0f - (l_age / l_life)));
  return coax_p_wg_mul_f2f(coax_p_wg_div_f2f(l_rel, coax_p_fmax(l_r, 0x1.0624dep-10f)), (l_wave * 0x1.666666p-2f));
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
fn coax_p_slab_foam_texel_m() -> f32 {
  return select(0.0f, (frame[1i] / frame[0i]), (frame[0i] > 0x1.000000p-1f));
}
fn coax_p_slab_foam_at(p_p: vec2f) -> vec2f {
  let l_p: vec2f = p_p;
  var l_nf: f32;
  var l_n: i32;
  var l_patch: f32;
  var l_q: vec2f;
  var l_fl: vec2f;
  var l_f: vec2f;
  var l_ix: i32;
  var l_iz: i32;
  var l_a: vec2f;
  var l_b: vec2f;
  var l_c: vec2f;
  var l_d: vec2f;
  l_nf = frame[0i];
  if ((l_nf < 0x1.000000p0f))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_n = coax_p_wg_i32_f((l_nf + 0x1.000000p-1f));
  l_patch = frame[1i];
  l_q = coax_p_v2(((((l_p.x - frame[2i]) / l_patch) * l_nf) - 0x1.000000p-1f), ((((l_p.y - frame[3i]) / l_patch) * l_nf) - 0x1.000000p-1f));
  l_fl = coax_p_wg_floor_f2(l_q);
  l_f = coax_p_wg_sub_f2f2(l_q, l_fl);
  l_ix = coax_p_wg_i32_f((l_fl.x - (floor((l_fl.x / l_nf)) * l_nf)));
  l_iz = coax_p_wg_i32_f((l_fl.y - (floor((l_fl.y / l_nf)) * l_nf)));
  l_a = coax_p_slab_foam_texel(l_ix, l_iz, l_n);
  l_b = coax_p_slab_foam_texel((l_ix + 1i), l_iz, l_n);
  l_c = coax_p_slab_foam_texel(l_ix, (l_iz + 1i), l_n);
  l_d = coax_p_slab_foam_texel((l_ix + 1i), (l_iz + 1i), l_n);
  return coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_a, (0x1.000000p0f - l_f.x)), coax_p_wg_mul_f2f(l_b, l_f.x)), (0x1.000000p0f - l_f.y)), coax_p_wg_mul_f2f(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_c, (0x1.000000p0f - l_f.x)), coax_p_wg_mul_f2f(l_d, l_f.x)), l_f.y));
}
fn coax_p_slab_foam_mean() -> vec2f {
  return coax_p_v2(frame[8i], frame[9i]);
}
fn coax_p_slab_foam_mix2(p_a: vec2f, p_b: vec2f, p_k: f32) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: vec2f = p_b;
  let l_k: f32 = p_k;
  return coax_p_v2(coax_p_mix(l_a.x, l_b.x, l_k), coax_p_mix(l_a.y, l_b.y, l_k));
}
fn coax_p_slab_foam_rank(p_wa: f32, p_wc: f32) -> f32 {
  let l_wa: f32 = p_wa;
  let l_wc: f32 = p_wc;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  l_a = coax_p_slab_noise(coax_p_v2((l_wa * 0x1.333334p-2f), (l_wc * 0x1.333334p-4f)));
  l_b = coax_p_slab_noise(coax_p_v2(((l_wa * 0x1.b33334p-1f) + 0x1.c00000p2f), (l_wc * 0x1.ae147ap-2f)));
  l_c = coax_p_slab_noise(coax_p_v2(((l_wa * 0x1.4cccccp1f) + 0x1.800000p1f), ((l_wc * 0x1.0cccccp1f) + 0x1.600000p3f)));
  return coax_p_smoothstep(0x1.70a3d8p-3f, 0x1.a3d70ap-1f, (((l_a * 0x1.19999ap-1f) + (l_b * 0x1.333334p-2f)) + (l_c * 0x1.333334p-3f)));
}
fn coax_p_slab_foam_show(p_cover: f32, p_rank: f32, p_soft: f32, p_fp: f32, p_grain: f32) -> f32 {
  let l_cover: f32 = p_cover;
  let l_rank: f32 = p_rank;
  let l_soft: f32 = p_soft;
  let l_fp: f32 = p_fp;
  let l_grain: f32 = p_grain;
  var l_c: f32;
  var l_at: f32;
  var l_fine: f32;
  l_c = coax_p_clampf(l_cover, 0.0f, 0x1.000000p0f);
  l_at = (l_rank * (0x1.000000p0f - (0x1.000000p1f * l_soft)));
  l_fine = coax_p_smoothstep(l_at, (l_at + (0x1.000000p1f * l_soft)), l_c);
  return coax_p_mix(l_fine, l_c, coax_p_smoothstep((0x1.99999ap-2f * l_grain), (0x1.000000p1f * l_grain), l_fp));
}
fn coax_p_slab_photo_air(p_c: vec3f, p_d: vec3f, p_dist: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  var l_b: i32;
  var l_t: f32;
  l_b = 15872i;
  l_t = coax_p_pr_air_through(l_dist, coax_at_u(i32(((l_b + 0i) + 1i))), l_d.y, l_b);
  return coax_p_add3(coax_p_scale3(l_c, l_t), coax_p_scale3(coax_p_add3(coax_p_pr_under_cloud(coax_p_pr_air_far(l_d, coax_p_pr_at3((l_b + 16i)), coax_at_u(i32((l_b + 51i)))), l_b), coax_p_pr_night(coax_at_u(i32(((l_b + 16i) + 3i))))), (0x1.000000p0f - l_t)));
}
fn coax_p_sw_cover(p_cover: f32, p_rag: f32, p_fp: f32) -> f32 {
  let l_cover: f32 = p_cover;
  let l_rag: f32 = p_rag;
  let l_fp: f32 = p_fp;
  var l_fine: f32;
  l_fine = coax_p_smoothstep((l_rag - 0x1.eb851ep-4f), (l_rag + 0x1.eb851ep-4f), (l_cover * 0x1.266666p0f));
  return coax_p_mix(l_fine, coax_p_clampf(l_cover, 0.0f, 0x1.000000p0f), coax_p_smoothstep(0x1.99999ap-2f, 0x1.400000p1f, l_fp));
}
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  let l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
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
fn coax_p_atmo_day(p_sun_up: f32) -> f32 {
  let l_sun_up: f32 = p_sun_up;
  var l_t: f32;
  l_t = coax_p_saturate(((l_sun_up - (-0x1.ef74c0p-3f)) / (0.0f - (-0x1.ef74c0p-3f))));
  return ((l_t * l_t) * l_t);
}
fn coax_p_pr_haze_loss(p_hz: f32) -> vec3f {
  let l_hz: f32 = p_hz;
  return coax_p_scale3(coax_p_v3(0x1.bd70a4p-1f, 0x1.000000p0f, 0x1.4cccccp0f), coax_p_fmax(l_hz, 0.0f));
}
fn coax_p_atmo_sun_mass(p_sun_up: f32) -> f32 {
  let l_sun_up: f32 = p_sun_up;
  if ((l_sun_up < 0.0f))
  {
    return (0x1.200000p5f - (l_sun_up * 0x1.2c0000p7f));
  }
  return (0x1.000000p0f / (l_sun_up + 0x1.c71c8ap-6f));
}
fn coax_p_atmo_exp3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_v3(exp(l_a.x), exp(l_a.y), exp(l_a.z));
}
fn coax_p_atmo_mass(p_up: f32) -> f32 {
  let l_up: f32 = p_up;
  return (0x1.000000p0f / (coax_p_fmax(l_up, 0.0f) + 0x1.47ae14p-4f));
}
fn coax_p_atmo_even(p_cos_t: f32) -> f32 {
  let l_cos_t: f32 = p_cos_t;
  return ((0x1.000000p-1f * (0x1.000000p0f + (l_cos_t * l_cos_t))) + 0x1.000000p-1f);
}
fn coax_p_atmo_forward(p_cos_t: f32) -> f32 {
  let l_cos_t: f32 = p_cos_t;
  var l_g: f32;
  var l_d: f32;
  l_g = 0x1.851eb8p-1f;
  l_d = coax_p_fmax(((0x1.000000p0f + (l_g * l_g)) - ((0x1.000000p1f * l_g) * l_cos_t)), 0x1.a36e2ep-14f);
  return (((0x1.000000p0f - (l_g * l_g)) / (l_d * sqrt(l_d))) * 0x1.28f5c2p-2f);
}
fn coax_p_film_in(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_film_rgb((((l_c.x * 0x1.39e7e8p-1f) + (l_c.y * 0x1.5babf4p-2f)) + (l_c.z * 0x1.8421eap-5f)), (((l_c.x * 0x1.1f8374p-4f) + (l_c.y * 0x1.d52c56p-1f)) + (l_c.z * 0x1.b8cee6p-7f)), (((l_c.x * 0x1.51c410p-6f) + (l_c.y * 0x1.c0cc3cp-4f)) + (l_c.z * 0x1.bd5858p-1f)));
}
fn coax_p_film_ap0(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_film_rgb((((l_c.x * 0x1.641250p-1f) + (l_c.y * 0x1.201c28p-3f)) + (l_c.z * 0x1.4f9a96p-3f)), (((l_c.x * 0x1.6ef502p-5f) + (l_c.y * 0x1.b826d0p-1f)) + (l_c.z * 0x1.874efep-4f)), ((((-l_c.x) * 0x1.6a24eep-8f) + (l_c.y * 0x1.07cbd2p-8f)) + (l_c.z * 0x1.00625ap0f)));
}
fn coax_p_film_min(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_b, l_a, (l_a < l_b));
}
fn coax_p_film_atan2(p_y: f32, p_x: f32) -> f32 {
  let l_y: f32 = p_y;
  let l_x: f32 = p_x;
  var l_ax: f32;
  var l_ay: f32;
  var l_high: f32;
  var l_t: f32;
  var l_z: f32;
  var l_p: f32;
  var l_r: f32;
  l_ax = abs(l_x);
  l_ay = abs(l_y);
  l_high = coax_p_film_max(l_ax, l_ay);
  if ((l_high == 0.0f))
  {
    return 0.0f;
  }
  l_t = (coax_p_film_min(l_ax, l_ay) / l_high);
  l_z = (l_t * l_t);
  l_p = (-0x1.09afcep-8f);
  l_p = ((l_p * l_z) + 0x1.662ca4p-6f);
  l_p = ((l_p * l_z) - 0x1.ca0388p-5f);
  l_p = ((l_p * l_z) + 0x1.8aefbep-4f);
  l_p = ((l_p * l_z) - 0x1.1cd8c6p-3f);
  l_p = ((l_p * l_z) + 0x1.98814cp-3f);
  l_p = ((l_p * l_z) - 0x1.554c38p-2f);
  l_p = ((l_p * l_z) + 0x1.ffffeap-1f);
  l_r = (l_t * l_p);
  if ((l_ay > l_ax))
  {
    l_r = (0x1.921fb6p0f - l_r);
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
fn coax_p_film_ap1(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_film_rgb((((l_c.x * 0x1.739186p0f) - (l_c.y * 0x1.e45fc0p-3f)) - (l_c.z * 0x1.b82c78p-3f)), ((((-l_c.x) * 0x1.399074p-4f) + (l_c.y * 0x1.2d1d64p0f)) - (l_c.z * 0x1.9845c8p-4f)), (((l_c.x * 0x1.1080e8p-7f) - (l_c.y * 0x1.8b57b6p-8f)) + (l_c.z * 0x1.fed4acp-1f)));
}
fn coax_p_film_mix_grey(p_c: vec3f, p_keep: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_keep: f32 = p_keep;
  var l_y: f32;
  l_y = (((l_c.x * 0x1.16c320p-2f) + (l_c.y * 0x1.59213ep-1f)) + (l_c.z * 0x1.b7d314p-5f));
  return coax_p_film_rgb((l_y + ((l_c.x - l_y) * l_keep)), (l_y + ((l_c.y - l_y) * l_keep)), (l_y + ((l_c.z - l_y) * l_keep)));
}
fn coax_p_film_curve(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  var l_slope: f32;
  var l_toe: f32;
  var l_shoulder: f32;
  var l_balance: f32;
  var l_low: f32;
  var l_straight: f32;
  var l_high: f32;
  var l_value: f32;
  var l_line: f32;
  var l_dark: f32;
  var l_bright: f32;
  var l_blend: f32;
  l_slope = 0x1.c28f5cp-1f;
  l_toe = 0x1.ccccccp-2f;
  l_shoulder = 0x1.8f5c28p-1f;
  l_balance = ((0x1.70a3d8p-3f / l_toe) - 0x1.000000p0f);
  l_low = ((log(0x1.70a3d8p-3f) / log(0x1.400000p3f)) - (((0x1.000000p-1f * log(((0x1.000000p0f + l_balance) / (0x1.000000p0f - l_balance)))) * l_toe) / l_slope));
  l_straight = ((0x1.ccccccp-2f / l_slope) - l_low);
  l_high = ((0x1.0a3d70p-2f / l_slope) - l_straight);
  l_value = (log(coax_p_film_max(l_x, 0x1.b7cdfep-34f)) / log(0x1.400000p3f));
  l_line = (l_slope * (l_value + l_straight));
  l_dark = select(l_line, ((0x1.000000p1f * l_toe) / (0x1.000000p0f + exp(((((-0x1.000000p1f) * l_slope) / l_toe) * (l_value - l_low))))), (l_value < l_low));
  l_bright = select(l_line, (0x1.0a3d70p0f - ((0x1.000000p1f * l_shoulder) / (0x1.000000p0f + exp((((0x1.000000p1f * l_slope) / l_shoulder) * (l_value - l_high)))))), (l_value > l_high));
  l_blend = coax_p_film_sat(((l_value - l_low) / (l_high - l_low)));
  if ((l_high < l_low))
  {
    l_blend = (0x1.000000p0f - l_blend);
  }
  l_blend = ((l_blend * l_blend) * (0x1.800000p1f - (0x1.000000p1f * l_blend)));
  return (l_dark + ((l_bright - l_dark) * l_blend));
}
fn coax_p_film_out(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_film_rgb((((l_c.x * 0x1.b47e38p0f) - (l_c.y * 0x1.3e5b8ap-1f)) - (l_c.z * 0x1.550742p-4f)), ((((-l_c.x) * 0x1.0ac3e0p-3f) + (l_c.y * 0x1.240bc8p0f)) - (l_c.z * 0x1.59a5b6p-7f)), ((((-l_c.x) * 0x1.894560p-6f) - (l_c.y * 0x1.0820e2p-3f)) + (l_c.z * 0x1.272932p0f)));
}
fn coax_p_atmo_sunlight(p_sun_up: f32) -> vec3f {
  let l_sun_up: f32 = p_sun_up;
  return coax_p_atmo_through(coax_p_atmo_sun_mass(l_sun_up));
}
fn coax_p_pr_stars(p_rd: vec3f) -> vec3f {
  let l_rd: vec3f = p_rd;
  var l_k: f32;
  var l_gx: f32;
  var l_gz: f32;
  var l_cx: f32;
  var l_cz: f32;
  var l_h: f32;
  var l_ox: f32;
  var l_oz: f32;
  var l_d2: f32;
  var l_bright: f32;
  var l_point: f32;
  var l_warm: f32;
  var l_col: vec3f;
  if ((l_rd.y <= 0x1.47ae14p-6f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_k = (0x1.040000p8f / (l_rd.y + 0x1.ae147ap-2f));
  l_gx = (l_rd.x * l_k);
  l_gz = (l_rd.z * l_k);
  l_cx = floor(l_gx);
  l_cz = floor(l_gz);
  l_h = coax_p_render_noise2(l_cx, l_cz);
  if ((l_h < 0x1.e8f5c2p-1f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_ox = (0x1.000000p-2f + (0x1.000000p-1f * coax_p_render_noise2((l_cx + 0x1.100000p4f), l_cz)));
  l_oz = (0x1.000000p-2f + (0x1.000000p-1f * coax_p_render_noise2(l_cx, (l_cz + 0x1.f00000p4f))));
  l_d2 = ((((l_gx - l_cx) - l_ox) * ((l_gx - l_cx) - l_ox)) + (((l_gz - l_cz) - l_oz) * ((l_gz - l_cz) - l_oz)));
  l_bright = ((l_h - 0x1.e8f5c2p-1f) / 0x1.70a3d8p-5f);
  l_point = coax_p_saturate((0x1.000000p0f - (l_d2 / 0x1.99999ap-5f)));
  l_warm = coax_p_render_noise2((l_cx - 0x1.200000p3f), (l_cz + 0x1.400000p2f));
  l_col = coax_p_mix3(coax_p_v3(0x1.666666p-1f, 0x1.99999ap-1f, 0x1.000000p0f), coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.666666p-1f), l_warm);
  return coax_p_scale3(l_col, (((l_point * l_point) * (0x1.89374cp-8f + (((0x1.70a3d8p-4f * l_bright) * l_bright) * l_bright))) * coax_p_saturate((l_rd.y * 0x1.400000p2f))));
}
fn coax_p_pr_moon_face(p_rd: vec3f, p_moon: vec3f, p_sun: vec3f, p_soft: f32) -> vec2f {
  let l_rd: vec3f = p_rd;
  let l_moon: vec3f = p_moon;
  let l_sun: vec3f = p_sun;
  let l_soft: f32 = p_soft;
  var l_ax: vec3f;
  var l_ay: vec3f;
  var l_px: f32;
  var l_py: f32;
  var l_r2: f32;
  var l_edge: f32;
  var l_cover: f32;
  var l_nz: f32;
  var l_nb: vec3f;
  var l_lit: f32;
  var l_seas: f32;
  if ((dot(l_rd, l_moon) < 0x1.fae148p-1f))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_ax = coax_p_cross3(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_moon);
  if ((dot(l_ax, l_ax) < 0x1.0c6f7ap-20f))
  {
    l_ax = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
  }
  l_ax = coax_p_normalize3(l_ax);
  l_ay = coax_p_cross3(l_moon, l_ax);
  l_px = (dot(l_rd, l_ax) / 0x1.2d7732p-8f);
  l_py = (dot(l_rd, l_ay) / 0x1.2d7732p-8f);
  l_r2 = ((l_px * l_px) + (l_py * l_py));
  l_edge = (coax_p_fmax(l_soft, 0x1.4f8b58p-16f) / 0x1.2d7732p-8f);
  l_cover = (0x1.000000p0f - coax_p_smoothstep((0x1.000000p0f - l_edge), (0x1.000000p0f + l_edge), sqrt(l_r2)));
  if ((l_cover <= 0.0f))
  {
    return coax_p_v2(0.0f, 0.0f);
  }
  l_nz = sqrt(coax_p_fmax((0x1.000000p0f - l_r2), 0.0f));
  l_nb = coax_p_sub3(coax_p_add3(coax_p_scale3(l_ax, l_px), coax_p_scale3(l_ay, l_py)), coax_p_scale3(l_moon, l_nz));
  l_lit = coax_p_saturate(((dot(l_nb, l_sun) * 0x1.000000p2f) + 0x1.47ae14p-4f));
  l_seas = (0x1.70a3d8p-1f + (0x1.1eb852p-2f * coax_p_pr_noise2(((l_px * 0x1.266666p1f) + 0x1.400000p2f), ((l_py * 0x1.266666p1f) - 0x1.800000p1f))));
  return coax_p_v2(l_cover, (l_lit * l_seas));
}
fn coax_p_atmo_sky(p_rd: vec3f, p_sun: vec3f) -> vec3f {
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  var l_up: f32;
  var l_cos_t: f32;
  var l_day: f32;
  var l_reach: vec3f;
  var l_mass: f32;
  var l_took: vec3f;
  var l_air: vec3f;
  var l_haze: f32;
  var l_first: f32;
  var l_once: vec3f;
  var l_low: f32;
  var l_again: vec3f;
  l_up = coax_p_saturate(l_rd.y);
  l_cos_t = dot(l_rd, l_sun);
  l_day = coax_p_atmo_day(l_sun.y);
  l_reach = coax_p_atmo_through((coax_p_atmo_sun_mass(l_sun.y) * coax_p_mix(0x1.000000p0f, 0x1.333334p-2f, sqrt(l_up))));
  l_mass = coax_p_atmo_mass(l_up);
  l_took = coax_p_v3((0x1.000000p0f - exp((0.0f - ((0x1.916872p-6f + 0x1.89374cp-7f) * l_mass)))), (0x1.000000p0f - exp((0.0f - ((0x1.d3c362p-5f + 0x1.89374cp-7f) * l_mass)))), (0x1.000000p0f - exp((0.0f - ((0x1.1eb852p-3f + 0x1.89374cp-7f) * l_mass)))));
  l_air = coax_p_scale3(coax_p_v3((0x1.916872p-6f / (0x1.916872p-6f + 0x1.89374cp-7f)), (0x1.d3c362p-5f / (0x1.d3c362p-5f + 0x1.89374cp-7f)), (0x1.1eb852p-3f / (0x1.1eb852p-3f + 0x1.89374cp-7f))), coax_p_atmo_even(l_cos_t));
  l_haze = (0x1.5c28f6p-3f * (coax_p_atmo_forward(l_cos_t) + ((0x1.666666p1f * exp((0.0f - (coax_p_fmax((0x1.000000p0f - l_cos_t), 0.0f) * 0x1.19999ap1f)))) * exp((0.0f - (l_up * 0x1.c00000p2f))))));
  l_first = coax_p_mix(coax_p_smoothstep((0.0f - 0x1.1eb852p-3f), 0x1.47ae14p-6f, l_sun.y), 0x1.000000p0f, exp((0.0f - (l_up * 0x1.800000p2f))));
  l_once = coax_p_scale3(coax_p_mul3(coax_p_mul3(l_reach, l_took), coax_p_add3(l_air, coax_p_v3(l_haze, l_haze, l_haze))), l_first);
  l_low = (0x1.000000p0f - coax_p_saturate(((l_sun.y * 0x1.400000p2f) + 0x1.333334p-3f)));
  l_again = coax_p_scale3(coax_p_mul3(coax_p_v3(0x1.eb851ep-6f, 0x1.1eb852p-4f, 0x1.ae147ap-3f), l_took), coax_p_mix(0x1.000000p0f, (0x1.4cccccp1f * coax_p_mix(0x1.eb851ep-4f, 0x1.000000p0f, coax_p_smoothstep(0.0f, 0x1.ccccccp-2f, l_up))), l_low));
  return coax_p_scale3(coax_p_add3(l_once, l_again), (0x1.8cccccp0f * l_day));
}
fn coax_p_atmo_heap(p_x: f32, p_y: f32, p_cover: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_cover: f32 = p_cover;
  var l_n: f32;
  var l_lo: f32;
  l_n = coax_p_atmo_fbm2(l_x, l_y);
  l_lo = (0x1.7ae148p-1f - (0x1.000000p-1f * coax_p_saturate(l_cover)));
  return (coax_p_smoothstep(l_lo, (l_lo + 0x1.99999ap-3f), l_n) * coax_p_smoothstep(0.0f, 0x1.99999ap-5f, l_cover));
}
fn coax_p_atmo_fbm2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  var l_v: f32;
  var l_a: f32;
  var l_px: f32;
  var l_py: f32;
  var l_k: i32;
  var l_qx: f32;
  l_v = 0.0f;
  l_a = 0x1.000000p-1f;
  l_px = l_x;
  l_py = l_y;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 5i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_v = (l_v + (l_a * coax_p_atmo_noise2(l_px, l_py)));
      l_qx = (((l_px * 0x1.9eb852p0f) + (l_py * 0x1.2e147ap0f)) + 0x1.d9999ap1f);
      l_py = (((l_py * 0x1.9eb852p0f) - (l_px * 0x1.2e147ap0f)) + 0x1.266666p3f);
      l_px = l_qx;
      l_a = (l_a * 0x1.000000p-1f);
    }
  }
  return (l_v / 0x1.f00000p-1f);
}
fn coax_p_slab_foam_texel(p_ix: i32, p_iz: i32, p_n: i32) -> vec2f {
  let l_ix: i32 = p_ix;
  let l_iz: i32 = p_iz;
  let l_n: i32 = p_n;
  var l_m: i32;
  var l_k: i32;
  var l_w: u32;
  var l_b: u32;
  l_m = (l_n - 1i);
  l_k = (((l_iz & l_m) * l_n) + (l_ix & l_m));
  l_w = coax_p_wg_atu_1((16i + i32((l_k >> u32(1i)))));
  l_b = (l_w >> u32(u32(((l_k & 1i) * 16i))));
  return coax_p_v2((f32((l_b & 255u)) / 0x1.fe0000p7f), (f32(((l_b >> u32(8u)) & 255u)) / 0x1.f80000p6f));
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
fn coax_p_atmo_through(p_mass: f32) -> vec3f {
  let l_mass: f32 = p_mass;
  return coax_p_atmo_exp3(coax_p_scale3(coax_p_v3((0x1.916872p-6f + 0x1.89374cp-7f), (0x1.d3c362p-5f + 0x1.89374cp-7f), (0x1.1eb852p-3f + 0x1.89374cp-7f)), (0.0f - l_mass)));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_atmo_noise2(p_x: f32, p_y: f32) -> f32 {
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
  return coax_p_mix(coax_p_mix(coax_p_atmo_hash2(l_ix, l_iy), coax_p_atmo_hash2((l_ix + 0x1.000000p0f), l_iy), l_sx), coax_p_mix(coax_p_atmo_hash2(l_ix, (l_iy + 0x1.000000p0f)), coax_p_atmo_hash2((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f)), l_sx), l_sy);
}
fn coax_p_wg_atu_1(p_at: i32) -> u32 {
  let l_at: i32 = p_at;
  return bitcast<u32>(frame[l_at]);
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
fn coax_p_atmo_hash2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_atmo_cell(l_x), coax_p_atmo_cell(l_y)));
}
fn coax_p_atmo_cell(p_x: f32) -> u32 {
  let l_x: f32 = p_x;
  return u32(i32(floor(coax_p_fmin(coax_p_fmax(l_x, (0.0f - 0x1.000000p25f)), 0x1.fffffep24f))));
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
fn shadow_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_shadow_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x);
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_16 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_16(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_17) -> coax_io_18 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t17(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_18(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_20 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_20(coax_r.x, coax_r.y, coax_r.z);
}
@fragment
fn surface_fs(coax_in: coax_io_12) -> coax_out_21 {
  let coax_r = coax_p_surface_fs(coax_t12(coax_in.x, coax_in.y, coax_in.z));
  return coax_out_21(coax_r.x, coax_r.y);
}
@fragment
fn shadow_fs(coax_in: coax_io_15) {
  coax_p_shadow_fs(coax_t15(coax_in.x));
}
@fragment
fn scene_fs(coax_in: coax_io_16) -> coax_out_21 {
  let coax_r = coax_p_scene_fs(coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_out_21(coax_r.x, coax_r.y);
}
@fragment
fn bloom_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bloom_fs(coax_pos);
  return coax_r;
}
@fragment
fn post_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_post_fs(coax_pos);
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