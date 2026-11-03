// ---- generated: apps/openagents-web/gpu/metahumanlab.coax (the module) by coax wgsl; do not edit
struct coax_t13 {
  x: vec3f,
  y: vec3f,
  z: vec3f,
  w: f32,
  f4: vec2f,
  f5: u32,
  f6: u32,
}
struct coax_t14 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec4f,
  f4: vec2f,
  f5: u32,
  f6: u32,
}
struct coax_io_14 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec4f,
  @location(3) f4: vec2f,
  @location(4) @interpolate(flat) f5: u32,
  @location(5) @interpolate(flat) f6: u32,
}
struct coax_t15 {
  x: vec4f,
  y: vec2f,
  z: u32,
  w: u32,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) @interpolate(flat) z: u32,
  @location(2) @interpolate(flat) w: u32,
}
struct coax_t16 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_16 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t17 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_17 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t20 {
  x: f32,
  y: vec3f,
  z: f32,
  w: f32,
}
struct coax_t21 {
  x: vec4f,
  y: f32,
}
struct coax_out_21 {
  @location(0) x: vec4f,
  @builtin(frag_depth) y: f32,
}
struct coax_t24 {
  x: bool,
  y: vec4f,
}
struct coax_t25 {
  x: bool,
  y: i32,
  z: f32,
  w: f32,
  f4: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 97>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> pose: array<vec4f>;
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
@group(1) @binding(2) var sheets: texture_2d_array<f32>;
@group(1) @binding(3) var sheet: sampler;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_caster_vs(p_v: u32, p_i: u32) -> vec4f {
  var l_v: u32 = p_v;
  var l_i: u32 = p_i;
  return coax_p_v4(0x1.000000p1f, 0x1.000000p1f, 0x1.000000p1f, 0x1.000000p0f);
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_v: coax_t13;
  var l_o: coax_t14;
  var l_t: vec3f;
  l_v = coax_p_mhlab_skin(l_id);
  l_o = coax_t14();
  l_o.y = coax_p_mhlab_place(l_v.x);
  l_o.z = coax_p_mhlab_turn(l_v.y);
  l_t = coax_p_mhlab_turn(l_v.z);
  l_o.w = coax_p_v4(l_t.x, l_t.y, l_t.z, l_v.w);
  l_o.f4 = l_v.f4;
  l_o.f5 = l_v.f5;
  l_o.f6 = l_v.f6;
  l_o.x = coax_p_mhlab_project(l_o.y);
  return l_o;
}
fn coax_p_shadow_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_v: coax_t13;
  var l_q: vec3f;
  var l_o: coax_t15;
  l_v = coax_p_mhlab_skin(l_id);
  l_q = coax_p_mhlab_light(coax_p_mhlab_place(l_v.x));
  l_o = coax_t15();
  l_o.x = coax_p_v4(l_q.x, l_q.y, l_q.z, 0x1.000000p0f);
  l_o.y = l_v.f4;
  l_o.z = l_v.f5;
  l_o.w = l_v.f6;
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t16) -> coax_t17 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t16 = p_in_;
  var l_a: vec4f;
  var l_o: coax_t17;
  l_a = l_in_.x;
  l_o = coax_t17();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_in_.y;
  l_o.w = l_in_.z;
  l_o.f4 = l_in_.w;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t21 {
  var l_pos: vec4f = p_pos;
  var l_p: vec2f;
  var l_eye: vec3f;
  var l_f: f32;
  var l_fwd: vec3f;
  var l_rd: vec3f;
  var l_mesh: bool;
  var l_h: coax_t20;
  var l_at: vec3f;
  var l_mlit: f32;
  var l_lit_in: f32;
  var l_c: vec3f;
  var l_probe: f32;
  var l_depth: f32;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  l_eye = coax_p_wg_at3(16i);
  l_f = coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f);
  l_fwd = coax_p_wg_at3(20i);
  l_rd = coax_p_normalize3(coax_p_add3(l_fwd, coax_p_add3(coax_p_scale3(coax_p_wg_at3(24i), ((l_p.x - coax_at_u(i32(23i))) / l_f)), coax_p_scale3(coax_p_wg_at3(28i), ((coax_at_u(i32(27i)) - l_p.y) / l_f)))));
  l_mesh = (coax_at_u(i32(5i)) > 0x1.000000p-1f);
  l_h = coax_p_mhlab_trace(l_eye, l_rd, false, (!l_mesh));
  l_at = coax_p_add3(l_eye, coax_p_scale3(l_rd, coax_p_fmax(l_h.x, 0.0f)));
  l_mlit = coax_p_mhlab_lit(l_at, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f));
  l_lit_in = (-0x1.000000p0f);
  if (((l_mesh && (l_h.z >= 0.0f)) && (l_h.z < 0x1.000000p-1f)))
  {
    l_lit_in = l_mlit;
  }
  l_c = coax_p_mhlab_scene_hit(l_eye, l_rd, l_h, (!l_mesh), l_lit_in);
  l_probe = coax_at_u(i32(386i));
  if (l_mesh)
  {
    l_c = coax_p_scale3(coax_p_v3(coax_pow_bg_vs(coax_p_fmax(l_c.x, 0.0f), 0x1.19999ap1f), coax_pow_bg_vs(coax_p_fmax(l_c.y, 0.0f), 0x1.19999ap1f), coax_pow_bg_vs(coax_p_fmax(l_c.z, 0.0f), 0x1.19999ap1f)), 0x1.e66666p-1f);
  }
  l_depth = 0x1.000000p0f;
  if ((l_h.z >= 0.0f))
  {
    l_depth = coax_p_mhlab_depth((l_h.x * dot(l_rd, l_fwd)));
  }
  l_c = coax_p_mhlab_strips(l_p, coax_p_mhlab_out(l_c));
  if ((l_probe > 0x1.000000p-1f))
  {
    l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
    if ((((l_probe > 0x1.400000p1f) && (l_probe < 0x1.c00000p1f)) && (l_h.z > 0x1.000000p-1f)))
    {
      l_c = coax_p_v3(0x1.000000p0f, 0.0f, 0x1.000000p0f);
    }
  }
  return coax_t21(coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f), l_depth);
}
fn coax_p_scene_fs(p_s: coax_t14, p_front: bool) -> vec4f {
  var l_s: coax_t14 = p_s;
  var l_front: bool = p_front;
  var l_dx: vec2f;
  var l_dy: vec2f;
  var l_m: u32;
  var l_mat: vec4f;
  var l_base: vec4f;
  var l_ao: f32;
  var l_albedo: vec3f;
  var l_ng: vec3f;
  var l_n: vec3f;
  var l_tg: vec3f;
  var l_strand: vec3f;
  var l_radial: vec3f;
  var l_nm: vec4f;
  var l_tn: vec3f;
  var l_t: vec3f;
  var l_bt: vec3f;
  var l_rough: f32;
  var l_v: vec3f;
  var l_key: vec3f;
  var l_sh: f32;
  var l_c: vec3f;
  var l_sky: f32;
  var l_amb: vec3f;
  var l_rr: vec3f;
  var l_nv0: f32;
  var l_f0s: f32;
  var l_fs: f32;
  var l_env: vec3f;
  var l_sheen: f32;
  var l_r: vec3f;
  var l_fr: f32;
  var l_hk: vec3f;
  var l_glint: f32;
  l_dx = coax_p_v2(dpdxFine(l_s.f4.x), dpdxFine(l_s.f4.y));
  l_dy = coax_p_v2(dpdyFine(l_s.f4.x), dpdyFine(l_s.f4.y));
  if (((coax_at_u(i32(4i)) > 0x1.000000p-1f) && (((i32(l_s.x.x) + i32(l_s.x.y)) % 2i) == 0i)))
  {
    discard;
  }
  l_m = l_s.f6;
  l_mat = coax_p_wg_at4((280i + ((i32(coax_p_clamp_u(l_m, 1u, 24u)) - 1i) * 4i)));
  l_base = coax_p_mhlab_layer(l_s.f4, l_dx, l_dy, (l_s.f5 & 255u), coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f));
  if (((l_m == 3u) || (l_m == 4u)))
  {
    if ((l_base.w < (0x1.000000p-2f + (0x1.000000p-1f * coax_p_mhlab_bayer(i32(l_s.x.x), i32(l_s.x.y))))))
    {
      discard;
    }
  }
  else
  {
    if ((l_m != 1u))
    {
      if ((l_base.w < 0x1.666666p-2f))
      {
        discard;
      }
    }
  }
  l_ao = 0x1.000000p0f;
  if ((l_m == 1u))
  {
    l_ao = l_base.w;
  }
  l_albedo = coax_p_v3((l_base.x * l_mat.x), (l_base.y * l_mat.y), (l_base.z * l_mat.z));
  l_ng = coax_p_normalize3(l_s.z);
  if ((!l_front))
  {
    l_ng = coax_p_scale3(l_ng, (-0x1.000000p0f));
  }
  l_n = l_ng;
  l_tg = coax_p_normalize3(coax_p_sub3(coax_p_v3(l_s.w.x, l_s.w.y, l_s.w.z), coax_p_scale3(l_ng, dot(l_ng, coax_p_v3(l_s.w.x, l_s.w.y, l_s.w.z)))));
  l_strand = coax_p_scale3(coax_p_cross3(l_ng, l_tg), l_s.w.w);
  if ((l_m == 3u))
  {
    l_radial = coax_p_normalize3(coax_p_sub3(l_s.y, coax_p_wg_at3(40i)));
    l_ng = coax_p_normalize3(coax_p_add3(coax_p_scale3(l_ng, 0x1.666666p-2f), coax_p_scale3(l_radial, 0x1.4cccccp-1f)));
  }
  l_nm = coax_p_mhlab_layer(l_s.f4, l_dx, l_dy, ((l_s.f5 >> u32(8u)) & 255u), coax_p_v4(0x1.000000p-1f, 0x1.000000p-1f, 0x1.000000p0f, 0x1.000000p0f));
  if ((((l_s.f5 >> u32(8u)) & 255u) != 255u))
  {
    l_tn = coax_p_v3(((coax_p_mhlab_srgb(l_nm.x) * 0x1.000000p1f) - 0x1.000000p0f), ((coax_p_mhlab_srgb(l_nm.y) * 0x1.000000p1f) - 0x1.000000p0f), ((coax_p_mhlab_srgb(l_nm.z) * 0x1.000000p1f) - 0x1.000000p0f));
    l_t = coax_p_normalize3(coax_p_sub3(coax_p_v3(l_s.w.x, l_s.w.y, l_s.w.z), coax_p_scale3(l_ng, dot(l_ng, coax_p_v3(l_s.w.x, l_s.w.y, l_s.w.z)))));
    l_bt = coax_p_scale3(coax_p_cross3(l_ng, l_t), l_s.w.w);
    l_n = coax_p_normalize3(coax_p_add3(coax_p_add3(coax_p_scale3(l_t, l_tn.x), coax_p_scale3(l_bt, l_tn.y)), coax_p_scale3(l_ng, coax_p_fmax(l_tn.z, 0x1.99999ap-5f))));
  }
  l_rough = 0x1.000000p0f;
  if ((((l_s.f5 >> u32(16u)) & 255u) != 255u))
  {
    l_rough = coax_p_mhlab_srgb((coax_p_mhlab_layer(l_s.f4, l_dx, l_dy, ((l_s.f5 >> u32(16u)) & 255u), coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f))).y);
  }
  l_rough = coax_p_clampf((l_rough * l_mat.w), 0x1.47ae14p-5f, 0x1.000000p0f);
  l_v = coax_p_normalize3(coax_p_sub3(coax_p_wg_at3(16i), l_s.y));
  l_key = coax_p_normalize3(coax_p_wg_at3(276i));
  l_sh = coax_p_mhlab_lit(l_s.y, l_ng);
  l_c = coax_p_mhlab_light_one(l_n, l_v, l_key, l_albedo, l_rough, l_m, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.c28f5cp-1f), (coax_at_u(i32(384i)) * l_sh)), l_strand);
  l_c = coax_p_add3(l_c, coax_p_mhlab_light_one(l_n, l_v, coax_p_normalize3(coax_p_wg_at3(376i)), l_albedo, l_rough, l_m, coax_p_scale3(coax_p_v3(0x1.800000p-1f, 0x1.a3d70ap-1f, 0x1.000000p0f), coax_at_u(i32(379i))), l_strand));
  l_c = coax_p_add3(l_c, coax_p_mhlab_light_one(l_n, l_v, coax_p_normalize3(coax_p_wg_at3(380i)), l_albedo, l_rough, l_m, coax_p_scale3(coax_p_v3(0x1.ccccccp-1f, 0x1.e66666p-1f, 0x1.000000p0f), coax_at_u(i32(383i))), l_strand));
  l_sky = (0x1.000000p-1f + (0x1.000000p-1f * l_n.y));
  l_amb = coax_p_add3(coax_p_scale3(coax_p_v3(0x1.70a3d8p-2f, 0x1.99999ap-2f, 0x1.eb851ep-2f), (l_sky * 0x1.19999ap-1f)), coax_p_scale3(coax_p_v3(0x1.c28f5cp-3f, 0x1.99999ap-3f, 0x1.70a3d8p-3f), ((0x1.000000p0f - l_sky) * 0x1.666666p-2f)));
  l_amb = coax_p_add3(l_amb, coax_p_scale3(coax_p_v3(0x1.99999ap-5f, 0x1.70a3d8p-5f, 0x1.47ae14p-5f), coax_p_saturate(dot(l_n, coax_p_normalize3(coax_p_wg_at3(276i))))));
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3((l_albedo.x * l_amb.x), (l_albedo.y * l_amb.y), (l_albedo.z * l_amb.z)), l_ao));
  if (((l_m != 3u) && (l_m != 4u)))
  {
    l_rr = coax_p_reflect3(coax_p_scale3(l_v, (-0x1.000000p0f)), l_n);
    l_nv0 = coax_p_saturate(dot(l_n, l_v));
    l_f0s = 0x1.47ae14p-5f;
    if ((l_m == 1u))
    {
      l_f0s = 0x1.cac084p-6f;
    }
    l_fs = (l_f0s + (((0x1.000000p0f - l_f0s) * coax_pow_bg_vs((0x1.000000p0f - l_nv0), 0x1.400000p2f)) * (0x1.000000p0f - l_rough)));
    l_env = coax_p_mix3(coax_p_v3(0x1.99999ap-3f, 0x1.851eb8p-3f, 0x1.70a3d8p-3f), coax_p_v3(0x1.99999ap-2f, 0x1.c28f5cp-2f, 0x1.0a3d70p-1f), coax_p_saturate(((l_rr.y * 0x1.000000p-1f) + 0x1.000000p-1f)));
    l_c = coax_p_add3(l_c, coax_p_scale3(l_env, (((l_fs * (0x1.000000p0f - (l_rough * l_rough))) * l_ao) * 0x1.333334p-1f)));
  }
  if ((l_m >= 8u))
  {
    l_sheen = (coax_pow_bg_vs((0x1.000000p0f - coax_p_saturate(dot(l_n, l_v))), 0x1.800000p1f) * 0x1.c28f5cp-3f);
    l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3((0x1.99999ap-2f + l_albedo.x), (0x1.99999ap-2f + l_albedo.y), (0x1.99999ap-2f + l_albedo.z)), (l_sheen * (0x1.666666p-2f + (0x1.4cccccp-1f * l_sh)))));
  }
  if ((l_m == 2u))
  {
    l_r = coax_p_reflect3(coax_p_scale3(l_v, (-0x1.000000p0f)), l_ng);
    l_fr = (0x1.47ae14p-6f + (0x1.f5c290p-1f * coax_pow_bg_vs((0x1.000000p0f - coax_p_saturate(dot(l_ng, l_v))), 0x1.400000p2f)));
    l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.333334p-1f, 0x1.5c28f6p-1f), (l_fr * (0x1.99999ap-2f + (0x1.333334p-1f * coax_p_saturate(l_r.y))))));
    l_hk = coax_p_normalize3(coax_p_add3(l_key, l_v));
    l_glint = ((coax_p_mhlab_ggx(coax_p_saturate(dot(l_ng, l_hk)), 0x1.47ae14p-6f) * 0x1.47ae14p-9f) * l_sh);
    l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.eb851ep-1f, 0x1.ccccccp-1f), (l_glint * coax_at_u(i32(384i)))));
  }
  l_c = coax_p_mhlab_out(l_c);
  l_c = coax_p_mhlab_strips(coax_p_v2(l_s.x.x, l_s.x.y), l_c);
  if (((coax_at_u(i32(386i)) > 0x1.000000p-1f) && (coax_at_u(i32(386i)) < 0x1.800000p0f)))
  {
    l_c = coax_p_v3(coax_p_mhlab_srgb(l_albedo.x), coax_p_mhlab_srgb(l_albedo.y), coax_p_mhlab_srgb(l_albedo.z));
  }
  if (((coax_at_u(i32(386i)) > 0x1.800000p0f) && (coax_at_u(i32(386i)) < 0x1.400000p1f)))
  {
    l_c = coax_p_v3(coax_p_fract(l_s.f4.x), coax_p_fract(l_s.f4.y), (f32((l_s.f5 & 255u)) / 0x1.fe0000p7f));
  }
  if (((coax_at_u(i32(386i)) > 0x1.400000p1f) && (coax_at_u(i32(386i)) < 0x1.c00000p1f)))
  {
    l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  if ((coax_at_u(i32(386i)) > 0x1.c00000p1f))
  {
    l_c = coax_p_v3((f32(l_m) / 0x1.fe0000p7f), 0.0f, 0.0f);
  }
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_shadow_fs(p_v: coax_t15) {
  var l_v: coax_t15 = p_v;
  if ((l_v.w == 7u))
  {
    discard;
  }
}
fn coax_p_fx_fs(p_v: coax_t17) -> vec4f {
  var l_v: coax_t17 = p_v;
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
  return coax_p_mhlab_make_f4_f3f(coax_p_scale3(coax_p_scale3(coax_p_scale3(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
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
fn coax_p_mhlab_skin(p_id: u32) -> coax_t13 {
  var l_id: u32 = p_id;
  var l_b: u32;
  var l_xy: vec2f;
  var l_w1: u32;
  var l_at: vec4f;
  var l_nw: vec3f;
  var l_tw: vec3f;
  var l_j: vec4f;
  var l_wt: vec4f;
  var l_o: coax_t13;
  var l_k: i32;
  var l_jk: f32;
  var l_wk: f32;
  var l_r: i32;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_e: vec4f;
  var l_w7: u32;
  l_b = (l_id * 8u);
  l_xy = coax_p_wg_unpack2x16unorm(vertices[i32(l_b)]);
  l_w1 = vertices[i32((l_b + 1u))];
  l_at = coax_p_v4(((l_xy.x * 0x1.400000p1f) - (0x1.400000p1f * 0x1.000000p-1f)), ((l_xy.y * 0x1.400000p1f) + (-0x1.000000p-2f)), (((f32((l_w1 & 65535u)) / 0x1.fffe00p15f) * 0x1.400000p1f) - (0x1.400000p1f * 0x1.000000p-1f)), 0x1.000000p0f);
  l_nw = coax_p_mhlab_octa(vertices[i32((l_b + 2u))]);
  l_tw = coax_p_mhlab_octa(vertices[i32((l_b + 3u))]);
  l_j = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 5u))]);
  l_wt = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 6u))]);
  l_o = coax_t13();
  l_o.x = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.y = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.z = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_k = 0i;
  while ((l_k < 4i))
  {
    l_jk = l_j.x;
    l_wk = l_wt.x;
    if ((l_k == 1i))
    {
      l_jk = l_j.y;
      l_wk = l_wt.y;
    }
    if ((l_k == 2i))
    {
      l_jk = l_j.z;
      l_wk = l_wt.z;
    }
    if ((l_k == 3i))
    {
      l_jk = l_j.w;
      l_wk = l_wt.w;
    }
    if ((l_wk > 0.0f))
    {
      l_r = (i32(((l_jk * 0x1.fe0000p7f) + 0x1.000000p-1f)) * 3i);
      l_a = pose[l_r];
      l_c = pose[(l_r + 1i)];
      l_e = pose[(l_r + 2i)];
      l_o.x = coax_p_add3(l_o.x, coax_p_scale3(coax_p_v3(dot(l_a, l_at), dot(l_c, l_at), dot(l_e, l_at)), l_wk));
      l_o.y = coax_p_add3(l_o.y, coax_p_scale3(coax_p_v3(dot(coax_p_v3(l_a.x, l_a.y, l_a.z), l_nw), dot(coax_p_v3(l_c.x, l_c.y, l_c.z), l_nw), dot(coax_p_v3(l_e.x, l_e.y, l_e.z), l_nw)), l_wk));
      l_o.z = coax_p_add3(l_o.z, coax_p_scale3(coax_p_v3(dot(coax_p_v3(l_a.x, l_a.y, l_a.z), l_tw), dot(coax_p_v3(l_c.x, l_c.y, l_c.z), l_tw), dot(coax_p_v3(l_e.x, l_e.y, l_e.z), l_tw)), l_wk));
    }
    l_k = (l_k + 1i);
  }
  l_o.w = 0x1.000000p0f;
  if (((l_w1 >> u32(31u)) == 1u))
  {
    l_o.w = (-0x1.000000p0f);
  }
  l_o.f4 = coax_p_wg_unpack2x16unorm(vertices[i32((l_b + 4u))]);
  l_w7 = vertices[i32((l_b + 7u))];
  l_o.f5 = (l_w7 & 16777215u);
  l_o.f6 = (l_w7 >> u32(24u));
  return l_o;
}
fn coax_p_mhlab_place(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_c: f32;
  var l_sn: f32;
  l_c = coax_at_u(i32(260i));
  l_sn = coax_at_u(i32(261i));
  return coax_p_v3((((l_p.x * l_c) + (l_p.z * l_sn)) + coax_at_u(i32(262i))), l_p.y, ((((-l_p.x) * l_sn) + (l_p.z * l_c)) + coax_at_u(i32(263i))));
}
fn coax_p_mhlab_turn(p_n: vec3f) -> vec3f {
  var l_n: vec3f = p_n;
  var l_c: f32;
  var l_sn: f32;
  l_c = coax_at_u(i32(260i));
  l_sn = coax_at_u(i32(261i));
  return coax_p_v3(((l_n.x * l_c) + (l_n.z * l_sn)), l_n.y, (((-l_n.x) * l_sn) + (l_n.z * l_c)));
}
fn coax_p_mhlab_project(p_w: vec3f) -> vec4f {
  var l_w: vec3f = p_w;
  var l_rel: vec3f;
  var l_z: f32;
  var l_x: f32;
  var l_y: f32;
  var l_f: f32;
  var l_cx: f32;
  var l_cy: f32;
  var l_cz: f32;
  l_rel = coax_p_sub3(l_w, coax_p_wg_at3(16i));
  l_z = dot(l_rel, coax_p_wg_at3(20i));
  l_x = dot(l_rel, coax_p_wg_at3(24i));
  l_y = dot(l_rel, coax_p_wg_at3(28i));
  l_f = coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f);
  l_cx = (((((0x1.000000p1f * coax_at_u(i32(23i))) / coax_at_u(i32(0i))) - 0x1.000000p0f) * l_z) + (((0x1.000000p1f * l_f) * l_x) / coax_at_u(i32(0i))));
  l_cy = (((0x1.000000p0f - ((0x1.000000p1f * coax_at_u(i32(27i))) / coax_at_u(i32(1i)))) * l_z) + (((0x1.000000p1f * l_f) * l_y) / coax_at_u(i32(1i))));
  l_cz = ((0x1.900000p8f * (l_z - 0x1.99999ap-5f)) / (0x1.900000p8f - 0x1.99999ap-5f));
  return coax_p_v4(l_cx, l_cy, l_cz, l_z);
}
fn coax_p_mhlab_light(p_w: vec3f) -> vec3f {
  var l_w: vec3f = p_w;
  var l_rel: vec3f;
  var l_k: f32;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  l_rel = coax_p_sub3(l_w, coax_p_wg_at3(264i));
  l_k = coax_at_u(i32(267i));
  l_x = (dot(l_rel, coax_p_wg_at3(268i)) * l_k);
  l_y = (dot(l_rel, coax_p_wg_at3(272i)) * l_k);
  l_z = (0x1.000000p-1f - ((dot(l_rel, coax_p_wg_at3(276i)) * l_k) * 0x1.000000p-2f));
  return coax_p_v3(l_x, l_y, l_z);
}
fn coax_p_wg_at3(p_at: i32) -> vec3f {
  var l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
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
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_mhlab_trace(p_ro: vec3f, p_rd: vec3f, p_shadow: bool, p_figure: bool) -> coax_t20 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_shadow: bool = p_shadow;
  var l_figure: bool = p_figure;
  var l_h: coax_t20;
  var l_bound: vec4f;
  var l_i: i32;
  var l_ca: vec4f;
  var l_cb: vec4f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_tc: f32;
  l_h = coax_t20();
  l_h.x = 0x1.dcd650p29f;
  l_h.y = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_h.z = (-0x1.000000p0f);
  l_h.w = (-0x1.000000p0f);
  if (((l_rd.y < (-0x1.a36e2ep-14f)) && (!l_shadow)))
  {
    l_h.x = ((-l_ro.y) / l_rd.y);
    l_h.z = 0.0f;
  }
  l_bound = coax_p_wg_at4(36i);
  if ((l_figure && coax_p_mhlab_near(l_ro, l_rd, coax_p_v3(l_bound.x, l_bound.y, l_bound.z), l_bound.w)))
  {
    l_i = 0i;
    while ((l_i < 26i))
    {
      l_ca = coax_p_wg_at4((52i + (l_i * 8i)));
      l_cb = coax_p_wg_at4((56i + (l_i * 8i)));
      if ((l_ca.w > 0.0f))
      {
        l_a = coax_p_v3(l_ca.x, l_ca.y, l_ca.z);
        l_b = coax_p_v3(l_cb.x, l_cb.y, l_cb.z);
        l_tc = coax_p_mhlab_capsule(l_ro, l_rd, l_a, l_b, l_ca.w);
        if (((l_tc > 0.0f) && (l_tc < l_h.x)))
        {
          l_h.x = l_tc;
          l_h.y = coax_p_mhlab_capsule_n(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_tc)), l_a, l_b);
          l_h.z = l_cb.w;
          l_h.w = f32(l_i);
        }
      }
      l_i = (l_i + 1i);
    }
  }
  return l_h;
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_mhlab_lit(p_w: vec3f, p_n: vec3f) -> f32 {
  var l_w: vec3f = p_w;
  var l_n: vec3f = p_n;
  var l_q: vec3f;
  var l_uv: vec2f;
  var l_texel: f32;
  var l_s: f32;
  l_q = coax_p_mhlab_light(coax_p_add3(l_w, coax_p_scale3(l_n, 0x1.89374cp-8f)));
  l_uv = coax_p_v2(((l_q.x * 0x1.000000p-1f) + 0x1.000000p-1f), (0x1.000000p-1f - (l_q.y * 0x1.000000p-1f)));
  if (((((((l_uv.x < 0.0f) || (l_uv.x > 0x1.000000p0f)) || (l_uv.y < 0.0f)) || (l_uv.y > 0x1.000000p0f)) || (l_q.z >= 0x1.ff7ceep-1f)) || (l_q.z <= 0.0f)))
  {
    return 0x1.000000p0f;
  }
  l_texel = (0x1.000000p0f / f32(textureDimensions(shadow_map).x));
  l_s = 0.0f;
  l_s = (l_s + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_add2(l_uv, coax_p_v2((-l_texel), (-l_texel))), (l_q.z - coax_at_u(i32(279i)))));
  l_s = (l_s + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_add2(l_uv, coax_p_v2(l_texel, (-l_texel))), (l_q.z - coax_at_u(i32(279i)))));
  l_s = (l_s + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_add2(l_uv, coax_p_v2((-l_texel), l_texel)), (l_q.z - coax_at_u(i32(279i)))));
  l_s = (l_s + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_add2(l_uv, coax_p_v2(l_texel, l_texel)), (l_q.z - coax_at_u(i32(279i)))));
  return (l_s * 0x1.000000p-2f);
}
fn coax_p_mhlab_scene_hit(p_eye: vec3f, p_rd: vec3f, p_h: coax_t20, p_figure: bool, p_lit_in: f32) -> vec3f {
  var l_eye: vec3f = p_eye;
  var l_rd: vec3f = p_rd;
  var l_h: coax_t20 = p_h;
  var l_figure: bool = p_figure;
  var l_lit_in: f32 = p_lit_in;
  var l_c: vec3f;
  var l_back: coax_t20;
  var l_behind: vec3f;
  l_c = coax_p_mhlab_shade(l_eye, l_rd, l_h, l_figure, l_lit_in);
  if (((coax_at_u(i32(4i)) > 0x1.000000p-1f) && (l_h.z > 0x1.000000p-1f)))
  {
    l_back = coax_p_mhlab_trace(l_eye, l_rd, false, false);
    l_behind = coax_p_mhlab_shade(l_eye, l_rd, l_back, false, (-0x1.000000p0f));
    l_c = coax_p_mix3(l_behind, coax_p_scale3(l_c, 0x1.99999ap-1f), 0x1.47ae14p-2f);
  }
  return l_c;
}
fn coax_p_mhlab_depth(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  if ((l_z <= 0x1.99999ap-5f))
  {
    return 0.0f;
  }
  return coax_p_fmin(((0x1.900000p8f * (l_z - 0x1.99999ap-5f)) / ((0x1.900000p8f - 0x1.99999ap-5f) * l_z)), 0x1.000000p0f);
}
fn coax_p_mhlab_out(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  if ((coax_at_u(i32(5i)) > 0x1.000000p-1f))
  {
    return coax_p_v3(coax_p_mhlab_srgb(coax_p_mhlab_aces((l_c.x * coax_at_u(i32(385i))))), coax_p_mhlab_srgb(coax_p_mhlab_aces((l_c.y * coax_at_u(i32(385i))))), coax_p_mhlab_srgb(coax_p_mhlab_aces((l_c.z * coax_at_u(i32(385i))))));
  }
  return coax_p_v3(coax_p_verse_style_tone(l_c.x, 0x1.333334p0f), coax_p_verse_style_tone(l_c.y, 0x1.333334p0f), coax_p_verse_style_tone(l_c.z, 0x1.333334p0f));
}
fn coax_p_mhlab_strips(p_p: vec2f, p_c: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_c: vec3f = p_c;
  if (((l_p.y < coax_at_u(i32(6i))) || (l_p.y > (coax_at_u(i32(1i)) - coax_at_u(i32(7i))))))
  {
    return coax_p_add3(coax_p_scale3(l_c, 0x1.c28f5cp-3f), coax_p_v3(0x1.47ae14p-7f, 0x1.89374cp-7f, 0x1.0624dep-6f));
  }
  return l_c;
}
fn coax_p_clamp_u(p_x: u32, p_a: u32, p_b: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_a: u32 = p_a;
  var l_b: u32 = p_b;
  if ((l_x < l_a))
  {
    return l_a;
  }
  if ((l_x > l_b))
  {
    return l_b;
  }
  return l_x;
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_mhlab_layer(p_uv: vec2f, p_dx: vec2f, p_dy: vec2f, p_layer: u32, p_none: vec4f) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_layer: u32 = p_layer;
  var l_none: vec4f = p_none;
  if ((l_layer == 255u))
  {
    return l_none;
  }
  return coax_p_render_sample_layer8_grad(l_uv, i32(l_layer), l_dx, l_dy, 0.0f);
}
fn coax_p_mhlab_bayer(p_x: i32, p_y: i32) -> f32 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  var l_x0: i32;
  var l_x1: i32;
  var l_y0: i32;
  var l_y1: i32;
  var l_k: i32;
  l_x0 = (l_x % 2i);
  l_x1 = ((l_x / 2i) % 2i);
  l_y0 = (l_y % 2i);
  l_y1 = ((l_y / 2i) % 2i);
  l_k = ((((((l_x0 + l_y0) % 2i) * 8i) + (l_y0 * 4i)) + (((l_x1 + l_y1) % 2i) * 2i)) + l_y1);
  return ((f32(l_k) + 0x1.000000p-1f) / 0x1.000000p4f);
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_mhlab_srgb(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_c: f32;
  l_c = coax_p_saturate(l_x);
  if ((l_c <= 0x1.9a5c38p-9f))
  {
    return (l_c * 0x1.9d70a4p3f);
  }
  return ((0x1.0e147ap0f * coax_pow_bg_vs(l_c, (0x1.000000p0f / 0x1.333334p1f))) - 0x1.c28f5cp-5f);
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_mhlab_light_one(p_n: vec3f, p_v: vec3f, p_l: vec3f, p_albedo: vec3f, p_rough: f32, p_material: u32, p_colour: vec3f, p_strand: vec3f) -> vec3f {
  var l_n: vec3f = p_n;
  var l_v: vec3f = p_v;
  var l_l: vec3f = p_l;
  var l_albedo: vec3f = p_albedo;
  var l_rough: f32 = p_rough;
  var l_material: u32 = p_material;
  var l_colour: vec3f = p_colour;
  var l_strand: vec3f = p_strand;
  var l_nl: f32;
  var l_nv: f32;
  var l_diff: vec3f;
  var l_w: vec3f;
  var l_spec: f32;
  var l_h: vec3f;
  var l_nh: f32;
  var l_vh: f32;
  var l_f0: f32;
  var l_fr: f32;
  var l_a: f32;
  var l_lobe: f32;
  var l_a2: f32;
  var l_a1: f32;
  var l_wrap: f32;
  var l_r1: f32;
  var l_r2: f32;
  l_nl = dot(l_n, l_l);
  l_nv = coax_p_fmax(dot(l_n, l_v), 0x1.0624dep-10f);
  l_diff = coax_p_scale3(l_albedo, (coax_p_saturate(l_nl) / 0x1.921fb6p1f));
  if ((l_material == 1u))
  {
    l_w = coax_p_v3(coax_p_saturate(((l_nl + 0x1.ae147ap-2f) / 0x1.6b851ep0f)), coax_p_saturate(((l_nl + 0x1.70a3d8p-3f) / 0x1.2e147ap0f)), coax_p_saturate(((l_nl + 0x1.99999ap-4f) / 0x1.19999ap0f)));
    l_w = coax_p_v3((l_w.x * l_w.x), (l_w.y * l_w.y), (l_w.z * l_w.z));
    l_diff = coax_p_v3((l_albedo.x * l_w.x), (l_albedo.y * l_w.y), (l_albedo.z * l_w.z));
    l_diff = coax_p_scale3(l_diff, (0x1.000000p0f / 0x1.921fb6p1f));
  }
  l_spec = 0.0f;
  if ((l_nl > 0.0f))
  {
    l_h = coax_p_normalize3(coax_p_add3(l_l, l_v));
    l_nh = coax_p_saturate(dot(l_n, l_h));
    l_vh = coax_p_saturate(dot(l_v, l_h));
    l_f0 = 0x1.47ae14p-5f;
    if ((l_material == 1u))
    {
      l_f0 = 0x1.cac084p-6f;
    }
    l_fr = (l_f0 + ((0x1.000000p0f - l_f0) * coax_pow_bg_vs((0x1.000000p0f - l_vh), 0x1.400000p2f)));
    l_a = coax_p_fmax((l_rough * l_rough), 0x1.0624dep-9f);
    l_lobe = (coax_p_mhlab_ggx(l_nh, l_a) * coax_p_mhlab_vis(l_nl, l_nv, l_a));
    if ((l_material == 1u))
    {
      l_a2 = coax_p_fmax(((l_rough * 0x1.666666p0f) * (l_rough * 0x1.666666p0f)), 0x1.0624dep-9f);
      l_a1 = coax_p_fmax(((l_rough * 0x1.800000p-1f) * (l_rough * 0x1.800000p-1f)), 0x1.0624dep-9f);
      l_lobe = (((0x1.b33334p-1f * coax_p_mhlab_ggx(l_nh, l_a1)) * coax_p_mhlab_vis(l_nl, l_nv, l_a1)) + ((0x1.333334p-3f * coax_p_mhlab_ggx(l_nh, l_a2)) * coax_p_mhlab_vis(l_nl, l_nv, l_a2)));
    }
    l_spec = ((l_fr * l_lobe) * l_nl);
    if ((l_material == 3u))
    {
      l_wrap = coax_p_saturate(((l_nl * 0x1.333334p-1f) + 0x1.99999ap-2f));
      l_r1 = (coax_p_mhlab_strand(l_strand, l_n, l_h, (-0x1.47ae14p-4f), 0x1.680000p6f) * 0x1.99999ap-5f);
      l_r2 = (coax_p_mhlab_strand(l_strand, l_n, l_h, 0x1.eb851ep-4f, 0x1.800000p4f) * 0x1.eb851ep-4f);
      return coax_p_v3((l_colour.x * (l_diff.x + ((l_r1 + (l_r2 * l_albedo.x)) * l_wrap))), (l_colour.y * (l_diff.y + ((l_r1 + (l_r2 * l_albedo.y)) * l_wrap))), (l_colour.z * (l_diff.z + ((l_r1 + (l_r2 * l_albedo.z)) * l_wrap))));
    }
  }
  return coax_p_v3((l_colour.x * (l_diff.x + l_spec)), (l_colour.y * (l_diff.y + l_spec)), (l_colour.z * (l_diff.z + l_spec)));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_i))));
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_mhlab_ggx(p_nh: f32, p_a: f32) -> f32 {
  var l_nh: f32 = p_nh;
  var l_a: f32 = p_a;
  var l_a2: f32;
  var l_d: f32;
  l_a2 = (l_a * l_a);
  l_d = (((l_nh * l_nh) * (l_a2 - 0x1.000000p0f)) + 0x1.000000p0f);
  return (l_a2 / ((0x1.921fb6p1f * l_d) * l_d));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
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
fn coax_p_mhlab_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_wg_unpack2x16unorm(p_x: u32) -> vec2f {
  var l_x: u32 = p_x;
  return coax_p_v2((f32((l_x & 65535u)) / 0x1.fffe00p15f), (f32((l_x >> u32(16u))) / 0x1.fffe00p15f));
}
fn coax_p_mhlab_octa(p_w: u32) -> vec3f {
  var l_w: u32 = p_w;
  var l_e: vec2f;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_ox: f32;
  var l_sx: f32;
  var l_sy: f32;
  l_e = coax_p_wg_unpack2x16unorm(l_w);
  l_x = ((l_e.x * 0x1.000000p1f) - 0x1.000000p0f);
  l_y = ((l_e.y * 0x1.000000p1f) - 0x1.000000p0f);
  l_z = ((0x1.000000p0f - abs(l_x)) - abs(l_y));
  if ((l_z < 0.0f))
  {
    l_ox = l_x;
    l_sx = 0x1.000000p0f;
    if ((l_ox < 0.0f))
    {
      l_sx = (-0x1.000000p0f);
    }
    l_sy = 0x1.000000p0f;
    if ((l_y < 0.0f))
    {
      l_sy = (-0x1.000000p0f);
    }
    l_x = ((0x1.000000p0f - abs(l_y)) * l_sx);
    l_y = ((0x1.000000p0f - abs(l_ox)) * l_sy);
  }
  return coax_p_normalize3(coax_p_v3(l_x, l_y, l_z));
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_mhlab_near(p_ro: vec3f, p_rd: vec3f, p_c: vec3f, p_r: f32) -> bool {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_c: vec3f = p_c;
  var l_r: f32 = p_r;
  var l_oc: vec3f;
  var l_b: f32;
  var l_k: f32;
  l_oc = coax_p_sub3(l_ro, l_c);
  l_b = dot(l_oc, l_rd);
  l_k = (dot(l_oc, l_oc) - (l_r * l_r));
  if ((l_k < 0.0f))
  {
    return true;
  }
  if ((l_b > 0.0f))
  {
    return false;
  }
  return (((l_b * l_b) - l_k) > 0.0f);
}
fn coax_p_mhlab_capsule(p_ro: vec3f, p_rd: vec3f, p_a: vec3f, p_b: vec3f, p_r: f32) -> f32 {
  var l_ro: vec3f = p_ro;
  var l_rd: vec3f = p_rd;
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_r: f32 = p_r;
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
  var l_oc: vec3f;
  var l_b2: f32;
  var l_c2: f32;
  var l_h2: f32;
  var l_t2: f32;
  l_ba = coax_p_sub3(l_b, l_a);
  l_oa = coax_p_sub3(l_ro, l_a);
  l_baba = coax_p_fmax(dot(l_ba, l_ba), 0x1.ad7f2ap-24f);
  l_bard = dot(l_ba, l_rd);
  l_baoa = dot(l_ba, l_oa);
  l_rdoa = dot(l_rd, l_oa);
  l_oaoa = dot(l_oa, l_oa);
  l_qa = (l_baba - (l_bard * l_bard));
  l_qb = ((l_baba * l_rdoa) - (l_baoa * l_bard));
  l_qc = (((l_baba * l_oaoa) - (l_baoa * l_baoa)) - ((l_r * l_r) * l_baba));
  l_h = ((l_qb * l_qb) - (l_qa * l_qc));
  if ((l_h < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  l_t = (((-l_qb) - sqrt(l_h)) / coax_p_fmax(l_qa, 0x1.0c6f7ap-20f));
  l_y = (l_baoa + (l_t * l_bard));
  if (((l_y > 0.0f) && (l_y < l_baba)))
  {
    if ((l_t < 0.0f))
    {
      return (-0x1.000000p0f);
    }
    return l_t;
  }
  l_oc = l_oa;
  if ((l_y >= l_baba))
  {
    l_oc = coax_p_sub3(l_ro, l_b);
  }
  l_b2 = dot(l_rd, l_oc);
  l_c2 = (dot(l_oc, l_oc) - (l_r * l_r));
  l_h2 = ((l_b2 * l_b2) - l_c2);
  if ((l_h2 < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  l_t2 = ((-l_b2) - sqrt(l_h2));
  if ((l_t2 < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return l_t2;
}
fn coax_p_mhlab_capsule_n(p_p: vec3f, p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_ba: vec3f;
  var l_pa: vec3f;
  var l_h: f32;
  l_ba = coax_p_sub3(l_b, l_a);
  l_pa = coax_p_sub3(l_p, l_a);
  l_h = coax_p_saturate((dot(l_pa, l_ba) / coax_p_fmax(dot(l_ba, l_ba), 0x1.0c6f7ap-20f)));
  return coax_p_normalize3(coax_p_sub3(l_pa, coax_p_scale3(l_ba, l_h)));
}
fn coax_p_add2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return vec2f((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_mhlab_shade(p_eye: vec3f, p_rd: vec3f, p_h: coax_t20, p_figure: bool, p_lit_in: f32) -> vec3f {
  var l_eye: vec3f = p_eye;
  var l_rd: vec3f = p_rd;
  var l_h: coax_t20 = p_h;
  var l_figure: bool = p_figure;
  var l_lit_in: f32 = p_lit_in;
  var l_sun: vec3f;
  var l_pos: vec3f;
  var l_n: vec3f;
  var l_albedo: vec3f;
  var l_spec: f32;
  var l_bound: vec4f;
  var l_d: f32;
  var l_hc: vec3f;
  var l_hf: vec3f;
  var l_hu: vec3f;
  var l_hs: vec3f;
  var l_d_17: vec3f;
  var l_fw: f32;
  var l_upw: f32;
  var l_side: f32;
  var l_eye_d: f32;
  var l_ndl: f32;
  var l_lit: f32;
  var l_sh: coax_t20;
  var l_w: f32;
  var l_amb: f32;
  var l_sky: vec3f;
  var l_key: vec3f;
  var l_c: vec3f;
  var l_v: vec3f;
  var l_hv: vec3f;
  var l_rim: f32;
  var l_fog: f32;
  l_sun = coax_p_normalize3(coax_p_wg_at3(32i));
  if ((l_h.z < 0.0f))
  {
    return coax_p_mhlab_sky(l_rd, l_sun);
  }
  l_pos = coax_p_add3(l_eye, coax_p_scale3(l_rd, l_h.x));
  l_n = l_h.y;
  l_albedo = coax_p_v3(0x1.000000p-1f, 0x1.000000p-1f, 0x1.000000p-1f);
  l_spec = 0.0f;
  if ((l_h.z < 0x1.000000p-1f))
  {
    l_albedo = coax_p_mhlab_floor(l_pos.x, l_pos.z);
    l_bound = coax_p_wg_at4(36i);
    l_d = coax_p_wg_length2(coax_p_v2((l_pos.x - l_bound.x), (l_pos.z - l_bound.z)));
    if (l_figure)
    {
      l_albedo = coax_p_scale3(l_albedo, (0x1.3d70a4p-1f + (0x1.851eb8p-2f * coax_p_smoothstep(0x1.99999ap-5f, 0x1.800000p-1f, l_d))));
    }
  }
  else
  {
    if ((l_h.z < 0x1.800000p0f))
    {
      l_albedo = coax_p_v3(0x1.0a3d70p-3f, 0x1.333334p-3f, 0x1.851eb8p-3f);
      l_spec = 0x1.eb851ep-4f;
    }
    else
    {
      if ((l_h.z < 0x1.400000p1f))
      {
        l_albedo = coax_p_v3(0x1.70a3d8p-1f, 0x1.000000p-1f, 0x1.99999ap-2f);
        l_spec = 0x1.99999ap-3f;
      }
      else
      {
        if ((l_h.z < 0x1.c00000p1f))
        {
          l_albedo = coax_p_v3(0x1.eb851ep-5f, 0x1.eb851ep-5f, 0x1.1eb852p-4f);
          l_spec = 0x1.333334p-2f;
        }
        else
        {
          l_albedo = coax_p_v3(0x1.70a3d8p-1f, 0x1.000000p-1f, 0x1.99999ap-2f);
          l_spec = 0x1.99999ap-3f;
          l_hc = coax_p_wg_at3(40i);
          l_hf = coax_p_normalize3(coax_p_wg_at3(44i));
          l_hu = coax_p_normalize3(coax_p_wg_at3(48i));
          l_hs = coax_p_wg_cross3(l_hu, l_hf);
          l_d_17 = coax_p_normalize3(coax_p_sub3(l_pos, l_hc));
          l_fw = dot(l_d_17, l_hf);
          l_upw = dot(l_d_17, l_hu);
          l_side = dot(l_d_17, l_hs);
          if (((((l_upw > 0x1.ae147ap-2f) && (l_fw < 0x1.99999ap-1f)) || ((l_fw < (-0x1.333334p-3f)) && (l_upw > (-0x1.19999ap-1f)))) || ((l_upw > 0x1.333334p-3f) && (abs(l_side) > 0x1.a3d70ap-1f))))
          {
            l_albedo = coax_p_v3(0x1.99999ap-4f, 0x1.1eb852p-4f, 0x1.99999ap-5f);
            l_spec = 0x1.666666p-2f;
          }
          l_eye_d = coax_p_wg_length2(coax_p_v2((abs(l_side) - 0x1.5c28f6p-2f), (l_upw - 0x1.eb851ep-5f)));
          if (((l_fw > 0x1.666666p-1f) && (l_eye_d < 0x1.333334p-4f)))
          {
            l_albedo = coax_p_v3(0x1.ccccccp-1f, 0x1.c28f5cp-1f, 0x1.b33334p-1f);
            if ((l_eye_d < 0x1.70a3d8p-5f))
            {
              l_albedo = coax_p_v3(0x1.70a3d8p-3f, 0x1.0a3d70p-2f, 0x1.47ae14p-2f);
            }
            if ((l_eye_d < 0x1.6872b0p-6f))
            {
              l_albedo = coax_p_v3(0x1.47ae14p-6f, 0x1.47ae14p-6f, 0x1.eb851ep-6f);
            }
            l_spec = 0x1.99999ap-1f;
          }
          if ((((l_fw > 0x1.666666p-1f) && (abs((l_upw - 0x1.99999ap-3f)) < 0x1.99999ap-6f)) && (abs((abs(l_side) - 0x1.5c28f6p-2f)) < 0x1.c28f5cp-4f)))
          {
            l_albedo = coax_p_v3(0x1.eb851ep-4f, 0x1.47ae14p-4f, 0x1.eb851ep-5f);
          }
        }
      }
    }
  }
  l_ndl = dot(l_n, l_sun);
  l_lit = 0x1.000000p0f;
  if ((l_lit_in >= 0.0f))
  {
    l_lit = l_lit_in;
  }
  else
  {
    if ((l_ndl > 0.0f))
    {
      l_sh = coax_p_mhlab_trace(coax_p_add3(l_pos, coax_p_scale3(l_n, 0x1.47ae14p-7f)), l_sun, true, l_figure);
      if ((l_sh.z > 0.0f))
      {
        l_lit = 0.0f;
      }
    }
  }
  l_w = (coax_p_verse_style_wrap(l_ndl) * l_lit);
  l_amb = coax_p_verse_style_ambient(l_n.y, 0x1.000000p0f, 0x1.000000p-1f, 0x1.000000p0f);
  l_sky = coax_p_scale3(coax_p_v3(0x1.ae147ap-2f, 0x1.eb851ep-2f, 0x1.333334p-1f), (0x1.000000p-1f * l_amb));
  l_key = coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.dc28f6p-1f, 0x1.a3d70ap-1f), (0x1.59999ap0f * l_w));
  l_c = coax_p_v3((l_albedo.x * (l_sky.x + l_key.x)), (l_albedo.y * (l_sky.y + l_key.y)), (l_albedo.z * (l_sky.z + l_key.z)));
  l_v = coax_p_scale3(l_rd, (-0x1.000000p0f));
  if ((l_h.z > 0x1.000000p-1f))
  {
    l_hv = coax_p_normalize3(coax_p_add3(l_sun, l_v));
    l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.c28f5cp-1f), ((l_spec * coax_pow_bg_vs(coax_p_saturate(dot(l_n, l_hv)), 0x1.800000p4f)) * l_lit)));
    l_rim = coax_pow_bg_vs((0x1.000000p0f - coax_p_saturate(dot(l_n, l_v))), 0x1.800000p1f);
    l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.4cccccp-1f, 0x1.b33334p-1f), (l_rim * 0x1.000000p-2f)));
  }
  l_fog = (0x1.000000p0f - exp(((-l_h.x) * 0x1.1eb852p-5f)));
  return coax_p_mix3(l_c, coax_p_mhlab_sky(l_rd, l_sun), l_fog);
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
fn coax_p_mhlab_aces(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_saturate(((l_x * ((0x1.4147aep1f * l_x) + 0x1.eb851ep-6f)) / ((l_x * ((0x1.370a3ep1f * l_x) + 0x1.2e147ap-1f)) + 0x1.1eb852p-3f)));
}
fn coax_p_verse_style_tone(p_c: f32, p_exposure: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_exposure: f32 = p_exposure;
  return (0x1.000000p0f - exp(((-coax_p_st_max(l_c, 0.0f)) * l_exposure)));
}
fn coax_p_render_sample_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_result: coax_t24;
  l_rs_result = coax_p_render_filter_layer8_grad(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
}
fn coax_p_mhlab_vis(p_nl: f32, p_nv: f32, p_a: f32) -> f32 {
  var l_nl: f32 = p_nl;
  var l_nv: f32 = p_nv;
  var l_a: f32 = p_a;
  return (0x1.000000p-1f / coax_p_fmax(((l_nl * ((l_nv * (0x1.000000p0f - l_a)) + l_a)) + (l_nv * ((l_nl * (0x1.000000p0f - l_a)) + l_a))), 0x1.a36e2ep-14f));
}
fn coax_p_mhlab_strand(p_t: vec3f, p_n: vec3f, p_h: vec3f, p_shift: f32, p_power: f32) -> f32 {
  var l_t: vec3f = p_t;
  var l_n: vec3f = p_n;
  var l_h: vec3f = p_h;
  var l_shift: f32 = p_shift;
  var l_power: f32 = p_power;
  var l_ts: vec3f;
  var l_th: f32;
  l_ts = coax_p_normalize3(coax_p_add3(l_t, coax_p_scale3(l_n, l_shift)));
  l_th = dot(l_ts, l_h);
  return coax_pow_bg_vs(sqrt(coax_p_fmax((0x1.000000p0f - (l_th * l_th)), 0.0f)), l_power);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_mhlab_sky(p_rd: vec3f, p_sun: vec3f) -> vec3f {
  var l_rd: vec3f = p_rd;
  var l_sun: vec3f = p_sun;
  var l_up: f32;
  var l_c: vec3f;
  l_up = coax_p_saturate(l_rd.y);
  l_c = coax_p_mix3(coax_p_v3(0x1.000000p-1f, 0x1.0f5c28p-1f, 0x1.23d70ap-1f), coax_p_v3(0x1.99999ap-3f, 0x1.d70a3ep-3f, 0x1.28f5c2p-2f), coax_pow_bg_vs(l_up, 0x1.333334p-1f));
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.800000p-1f), (coax_p_verse_style_halo(dot(l_rd, l_sun), 0x1.800000p2f) * 0x1.eb851ep-4f)));
  return l_c;
}
fn coax_p_mhlab_floor(p_x: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_c: vec3f;
  var l_lx: f32;
  var l_lz: f32;
  var l_line: f32;
  var l_sx: f32;
  var l_sz: f32;
  var l_fine: f32;
  l_c = coax_p_v3(0x1.d70a3ep-2f, 0x1.e147aep-2f, 0x1.f5c290p-2f);
  l_c = coax_p_scale3(l_c, (0x1.e147aep-1f + (0x1.eb851ep-5f * coax_p_verse_style_noise((l_x * 0x1.000000p1f), (l_z * 0x1.000000p1f)))));
  l_lx = abs((coax_p_fract((l_x + 0x1.000000p-1f)) - 0x1.000000p-1f));
  l_lz = abs((coax_p_fract((l_z + 0x1.000000p-1f)) - 0x1.000000p-1f));
  l_line = (0x1.000000p0f - coax_p_smoothstep(0x1.0624dep-8f, 0x1.cac084p-7f, coax_p_fmin(l_lx, l_lz)));
  l_sx = (abs((coax_p_fract(((l_x * 0x1.000000p2f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * 0x1.000000p-2f);
  l_sz = (abs((coax_p_fract(((l_z * 0x1.000000p2f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * 0x1.000000p-2f);
  l_fine = ((0x1.000000p0f - coax_p_smoothstep(0x1.0624dep-9f, 0x1.0624dep-7f, coax_p_fmin(l_sx, l_sz))) * (0x1.000000p0f - coax_p_smoothstep(0x1.000000p1f, 0x1.000000p2f, coax_p_wg_length2(coax_p_v2(l_x, l_z)))));
  l_c = coax_p_mix3(l_c, coax_p_v3(0x1.333334p-2f, 0x1.3d70a4p-2f, 0x1.5c28f6p-2f), ((l_line * 0x1.19999ap-1f) + (l_fine * 0x1.c28f5cp-3f)));
  return l_c;
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_verse_style_wrap(p_ndl: f32) -> f32 {
  var l_ndl: f32 = p_ndl;
  return coax_p_st_sat(((l_ndl + 0x1.000000p-2f) / 0x1.400000p0f));
}
fn coax_p_verse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_sky: f32 = p_sky;
  var l_ground: f32 = p_ground;
  var l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
}
fn coax_p_st_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_render_filter_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t24 {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_f: coax_t25;
  var l_rs_sum: vec4f;
  var l_rs_i: i32;
  var l_rs_offset: f32;
  var l_rs_at: vec2f;
  l_rs_f = coax_p_render_footprint(i32(textureDimensions(sheets).x), i32(textureDimensions(sheets).y), l_dx.x, l_dx.y, l_dy.x, l_dy.y, l_bias);
  if ((!l_rs_f.x))
  {
    return coax_t24(false, vec4f());
  }
  l_rs_sum = vec4f();
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = l_rs_f.y;
    for (l_rs_i = coax_from; l_rs_i < coax_to; l_rs_i = l_rs_i + 1i)
    {
      l_rs_offset = ((f32(l_rs_i) + 0x1.000000p-1f) - (0x1.000000p-1f * f32(l_rs_f.y)));
      l_rs_at = coax_p_add2(l_uv, coax_p_scale2(coax_p_v2(l_rs_f.z, l_rs_f.w), l_rs_offset));
      l_rs_sum = coax_p_add4(l_rs_sum, textureSampleLevel(sheets, sheet, l_rs_at, l_layer, l_rs_f.f4));
    }
  }
  return coax_t24(true, coax_p_scale4(l_rs_sum, (0x1.000000p0f / f32(l_rs_f.y))));
}
fn coax_p_verse_style_halo(p_cos_t: f32, p_k: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  var l_k: f32 = p_k;
  return exp(((-l_k) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn coax_p_verse_style_noise(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_ix: f32;
  var l_iy: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_sx: f32;
  var l_sy: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_ix = floor(l_x);
  l_iy = floor(l_y);
  l_fx = (l_x - l_ix);
  l_fy = (l_y - l_iy);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sy = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  l_a = coax_p_verse_style_hash(l_ix, l_iy);
  l_b = coax_p_verse_style_hash((l_ix + 0x1.000000p0f), l_iy);
  l_c = coax_p_verse_style_hash(l_ix, (l_iy + 0x1.000000p0f));
  l_d = coax_p_verse_style_hash((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f));
  return ((l_a + ((l_b - l_a) * l_sx)) + (((l_c + ((l_d - l_c) * l_sx)) - (l_a + ((l_b - l_a) * l_sx))) * l_sy));
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_render_footprint(p_width: i32, p_height: i32, p_du_dx: f32, p_dv_dx: f32, p_du_dy: f32, p_dv_dy: f32, p_bias: f32) -> coax_t25 {
  var l_width: i32 = p_width;
  var l_height: i32 = p_height;
  var l_du_dx: f32 = p_du_dx;
  var l_dv_dx: f32 = p_dv_dx;
  var l_du_dy: f32 = p_du_dy;
  var l_dv_dy: f32 = p_dv_dy;
  var l_bias: f32 = p_bias;
  var l_x: f32;
  var l_y: f32;
  var l_u: f32;
  var l_v: f32;
  var l_a: f32;
  var l_b: f32;
  var l_major: f32;
  var l_minor: f32;
  var l_du: f32;
  var l_dv: f32;
  var l_taps: i32;
  var l_excess: i32;
  var l_span: f32;
  if (((((((((((((((l_width < 1i) || (l_width > 16384i)) || (l_height < 1i)) || (l_height > 16384i)) || (!coax_p_render_footprint_finite(l_du_dx))) || (!coax_p_render_footprint_finite(l_dv_dx))) || (!coax_p_render_footprint_finite(l_du_dy))) || (!coax_p_render_footprint_finite(l_dv_dy))) || (!coax_p_render_footprint_finite(l_bias))) || (abs(l_du_dx) > 0x1.000000p16f)) || (abs(l_dv_dx) > 0x1.000000p16f)) || (abs(l_du_dy) > 0x1.000000p16f)) || (abs(l_dv_dy) > 0x1.000000p16f)) || (abs(l_bias) > 0x1.000000p5f)))
  {
    return coax_t25(false, 0i, 0.0f, 0.0f, 0.0f);
  }
  l_x = (l_du_dx * f32(l_width));
  l_y = (l_dv_dx * f32(l_height));
  l_u = (l_du_dy * f32(l_width));
  l_v = (l_dv_dy * f32(l_height));
  l_a = ((l_x * l_x) + (l_y * l_y));
  l_b = ((l_u * l_u) + (l_v * l_v));
  l_major = 0.0f;
  l_minor = 0.0f;
  l_du = 0.0f;
  l_dv = 0.0f;
  if ((l_a >= l_b))
  {
    l_major = sqrt(l_a);
    l_minor = sqrt(l_b);
    l_du = l_du_dx;
    l_dv = l_dv_dx;
  }
  else
  {
    l_major = sqrt(l_b);
    l_minor = sqrt(l_a);
    l_du = l_du_dy;
    l_dv = l_dv_dy;
  }
  l_minor = coax_p_render_footprint_max(l_minor, 0x1.000000p0f);
  l_taps = i32((-floor(((-l_major) / l_minor))));
  if ((l_taps == 0i))
  {
    l_taps = 1i;
  }
  l_excess = i32((l_taps - 8i));
  l_taps = (l_taps - i32((l_excess & ((l_excess >> u32(31i)) ^ (-1i)))));
  l_span = coax_p_render_footprint_max(l_minor, (l_major / f32(l_taps)));
  return coax_t25(true, l_taps, (l_du / f32(l_taps)), (l_dv / f32(l_taps)), ((log(l_span) / log(0x1.000000p1f)) + l_bias));
}
fn coax_p_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
}
fn coax_p_add4(p_a: vec4f, p_b: vec4f) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return vec4f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
}
fn coax_p_scale4(p_a: vec4f, p_k: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_k: f32 = p_k;
  return vec4f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k), (l_a.w * l_k));
}
fn coax_p_verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return coax_p_render_noise2(l_x, l_y);
}
fn coax_p_st_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_render_footprint_finite(p_x: f32) -> bool {
  var l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_footprint_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (((l_a + l_b) + abs((l_a - l_b))) * 0x1.000000p-1f);
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
fn caster_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_caster_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_14 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_14(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6);
}
@vertex
fn shadow_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_shadow_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_16) -> coax_io_17 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_17(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_21 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_21(coax_r.x, coax_r.y);
}
@fragment
fn scene_fs(coax_in: coax_io_14, @builtin(front_facing) coax_front: bool) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t14(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6), coax_front);
  return coax_r;
}
@fragment
fn shadow_fs(coax_in: coax_io_15) {
  coax_p_shadow_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
}
@fragment
fn fx_fs(coax_in: coax_io_17) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t17(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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