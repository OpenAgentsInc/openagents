// ---- generated: apps/openagents-web/gpu/animelab.coax (the module) by coax wgsl; do not edit
struct coax_t9 {
  x: vec4f,
}
struct coax_io_9 {
  @builtin(position) x: vec4f,
}
struct coax_t11 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec3f,
  f4: f32,
  f5: f32,
  f6: f32,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
  @location(3) @interpolate(flat) f4: f32,
  @location(4) f5: f32,
  @location(5) @interpolate(flat) f6: f32,
}
struct coax_t12 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_in_12 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
}
struct coax_t14 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
}
struct coax_io_14 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
}
struct coax_t15 {
  x: vec4f,
  y: vec4f,
}
struct coax_out_15 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
struct coax_t20 {
  x: i32,
  y: i32,
}
struct coax_t22 {
  x: u32,
  y: u32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 88>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> world: array<u32>;
@group(1) @binding(1) var<storage, read> live: array<u32>;
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
@group(0) @binding(1) var sans_atlas: texture_2d<f32>;
@group(0) @binding(2) var sans_sampler: sampler;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  return coax_p_al_bg_vs(l_i, l__iid1);
}
fn coax_p_shadow_vs(p_id: u32, p_instance: u32) -> coax_t9 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_o: coax_t9;
  var l_b: u32;
  var l_c: u32;
  var l_mat: u32;
  l_o = coax_t9();
  l_b = (l_id * 6u);
  l_c = select(live[i32((l_b + 4u))], world[i32((l_b + 4u))], (l_instance == 0u));
  l_mat = (l_c >> u32(24u));
  l_o.x = coax_p_al_rows(36i, coax_p_al_place(l_id, l_instance));
  if ((((((l_mat == 3u) || (l_mat == 4u)) || (l_mat == 7u)) || (l_mat == 8u)) || (l_mat == 9u)))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, (-0x1.000000p1f), 0x1.000000p0f);
  }
  return l_o;
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t11 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_o: coax_t11;
  var l_b: u32;
  var l_nw: u32;
  var l_cw: u32;
  var l_pw: u32;
  var l_w: vec3f;
  var l_n4: vec4f;
  var l_c4: vec4f;
  l_o = coax_t11();
  l_b = (l_id * 6u);
  l_nw = select(live[i32((l_b + 3u))], world[i32((l_b + 3u))], (l_instance == 0u));
  l_cw = select(live[i32((l_b + 4u))], world[i32((l_b + 4u))], (l_instance == 0u));
  l_pw = select(live[i32((l_b + 5u))], world[i32((l_b + 5u))], (l_instance == 0u));
  l_w = coax_p_al_place(l_id, l_instance);
  l_n4 = coax_p_wg_unpack4x8unorm(l_nw);
  l_c4 = coax_p_wg_unpack4x8unorm(l_cw);
  l_o.y = l_w;
  l_o.x = coax_p_al_rows(4i, l_w);
  l_o.z = coax_p_v3(((l_n4.x * 0x1.000000p1f) - 0x1.000000p0f), ((l_n4.y * 0x1.000000p1f) - 0x1.000000p0f), ((l_n4.z * 0x1.000000p1f) - 0x1.000000p0f));
  l_o.w = coax_p_v3(coax_p_al_decode1(l_c4.x), coax_p_al_decode1(l_c4.y), coax_p_al_decode1(l_c4.z));
  l_o.f4 = floor(((l_c4.w * 0x1.fe0000p7f) + 0x1.000000p-1f));
  l_o.f6 = floor(((l_n4.w * 0x1.fe0000p7f) + 0x1.000000p-1f));
  l_o.f5 = bitcast<f32>(l_pw);
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t12) -> coax_t14 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t12 = p_in_;
  var l_a: vec4f;
  var l_o: coax_t14;
  l_a = l_in_.x;
  l_o = coax_t14();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_in_.y;
  l_o.w = l_in_.z;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t15 {
  var l_pos: vec4f = p_pos;
  var l_s: vec4f;
  var l_b: vec4f;
  var l_share: f32;
  l_s = coax_p_al_sky(coax_p_al_ray(l_pos));
  l_b = coax_p_al_st(19i);
  l_share = coax_p_verse_style_anime_glow_share(coax_p_verse_style_luma(l_s.x, l_s.y, l_s.z), l_b.x, l_b.y, l_s.w);
  return coax_t15(coax_p_v4(l_s.x, l_s.y, l_s.z, l_share), coax_p_v4(0.0f, 0.0f, 0.0f, (-0x1.000000p0f)));
}
fn coax_p_shadow_fs(p_s: coax_t9) {
  var l_s: coax_t9 = p_s;
}
fn coax_p_scene_fs(p_s: coax_t11) -> coax_t15 {
  var l_s: coax_t11 = p_s;
  var l_mat: f32;
  var l_eye: vec3f;
  var l_tov: vec3f;
  var l_dist: f32;
  var l_v: vec3f;
  var l_n: vec3f;
  var l_b: vec4f;
  var l_c: vec3f;
  var l_c_12: vec3f;
  var l_c_13: vec3f;
  var l_albedo: vec3f;
  var l_g: vec2f;
  var l_f: vec2f;
  var l_joint: f32;
  var l_stone: f32;
  var l_h: f32;
  var l_flood: f32;
  var l_shading: f32;
  var l_h_22: f32;
  var l_l: vec3f;
  var l_ndl: f32;
  var l_ndv: f32;
  var l_lit: f32;
  var l_w: f32;
  var l_bands: vec4f;
  var l_bands2: vec4f;
  var l_hz: vec4f;
  var l_share: f32;
  var l_aa: f32;
  var l_character: bool;
  var l_tone: f32;
  var l_hi: vec3f;
  var l_lo: vec3f;
  var l_c_37: vec3f;
  var l_k: i32;
  var l_p: vec4f;
  var l_col: vec4f;
  var l_dl: vec3f;
  var l_dd: f32;
  var l_fall: f32;
  var l_a: f32;
  var l_hh: vec3f;
  var l_spot: f32;
  var l_r1: vec4f;
  var l_r2: vec4f;
  var l_strength: f32;
  var l_rim: f32;
  var l_pale: vec4f;
  var l_ink: f32;
  l_mat = l_s.f4;
  l_eye = coax_p_al_xyz(coax_p_wg_at4(20i));
  l_tov = coax_p_al_sub3(l_eye, l_s.y);
  l_dist = coax_p_al_len3(l_tov);
  l_v = coax_p_al_scale3(l_tov, (0x1.000000p0f / coax_p_fmax(l_dist, 0x1.a36e2ep-14f)));
  l_n = coax_p_al_norm3(coax_p_al_add3(l_s.z, coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  if ((coax_p_al_dot3(l_n, l_v) < 0.0f))
  {
    l_n = coax_p_al_scale3(l_n, (-0x1.000000p0f));
  }
  l_b = coax_p_al_st(19i);
  if (((l_mat > 0x1.400000p1f) && (l_mat < 0x1.c00000p1f)))
  {
    l_c = coax_p_al_scale3(l_s.w, l_s.f5);
    return coax_t15(coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f), coax_p_v4(l_n.x, l_n.y, l_n.z, (-0x1.000000p0f)));
  }
  if ((((l_mat > 0x1.c00000p1f) && (l_mat < 0x1.200000p2f)) || ((l_mat > 0x1.a00000p2f) && (l_mat < 0x1.e00000p2f))))
  {
    l_c_12 = l_s.w;
    if ((l_mat < 0x1.200000p2f))
    {
      l_c_12 = coax_p_al_scale3(l_c_12, (0x1.19999ap-1f + (0x1.ccccccp-2f * (coax_p_al_st(0i)).w)));
    }
    return coax_t15(coax_p_v4(l_c_12.x, l_c_12.y, l_c_12.z, 0.0f), coax_p_v4(l_n.x, l_n.y, l_n.z, (-0x1.000000p0f)));
  }
  if (((l_mat > 0x1.100000p3f) && (l_mat < 0x1.300000p3f)))
  {
    l_c_13 = coax_p_al_scale3(l_s.w, l_s.f5);
    return coax_t15(coax_p_v4(l_c_13.x, l_c_13.y, l_c_13.z, 0x1.333334p-1f), coax_p_v4(l_n.x, l_n.y, l_n.z, l_s.f6));
  }
  l_albedo = l_s.w;
  if (((l_mat > 0x1.200000p2f) && (l_mat < 0x1.600000p2f)))
  {
    l_g = coax_p_v2((l_s.y.x * 0x1.3d70a4p-1f), ((l_s.y.z * 0x1.3d70a4p-1f) + (0x1.000000p-1f * floor((l_s.y.x * 0x1.3d70a4p-1f)))));
    l_f = coax_p_v2(coax_p_fract(l_g.x), coax_p_fract(l_g.y));
    l_joint = coax_p_fmin(coax_p_fmin(l_f.x, (0x1.000000p0f - l_f.x)), coax_p_fmin(l_f.y, (0x1.000000p0f - l_f.y)));
    l_stone = coax_p_verse_style_hash(floor(l_g.x), floor(l_g.y));
    l_albedo = coax_p_al_scale3(l_albedo, (0x1.ccccccp-1f + (0x1.47ae14p-3f * l_stone)));
    l_albedo = coax_p_al_scale3(l_albedo, select(0x1.000000p0f, 0x1.3d70a4p-1f, (l_joint < 0x1.1eb852p-5f)));
  }
  if (((l_mat > 0x1.e00000p2f) && (l_mat < 0x1.100000p3f)))
  {
    l_h = coax_p_verse_style_noise(((l_s.y.x * 0x1.8cccccp1f) + (l_s.y.y * 0x1.b33334p0f)), ((l_s.y.z * 0x1.8cccccp1f) - (l_s.y.y * 0x1.266666p1f)));
    if ((l_h > l_s.f5))
    {
      discard;
    }
  }
  l_flood = coax_at_u(i32(56i));
  l_shading = coax_at_u(i32(57i));
  if (((l_mat > 0x1.600000p2f) && (l_mat < 0x1.a00000p2f)))
  {
    l_h_22 = (0x1.70a3d8p-3f * coax_p_verse_style_noise((l_s.y.x * 0x1.000000p2f), ((l_s.y.z * 0x1.000000p2f) + l_s.y.y)));
    if (((l_s.f5 + l_h_22) > (l_flood * 0x1.2e147ap0f)))
    {
      discard;
    }
  }
  l_l = coax_p_al_key();
  l_ndl = coax_p_al_dot3(l_n, l_l);
  l_ndv = coax_p_al_dot3(l_n, l_v);
  l_lit = coax_p_al_lit(l_s.y, l_n);
  l_w = coax_p_verse_style_anime_light(l_ndl, l_lit);
  l_bands = coax_p_al_st(7i);
  l_bands2 = coax_p_al_st(8i);
  l_hz = coax_p_al_st(9i);
  l_share = coax_p_verse_style_anime_aerial(l_dist, l_hz.x, l_hz.y, l_hz.z);
  l_aa = coax_p_verse_style_anime_far_aa(l_bands.z, l_share);
  l_character = (((l_mat > 0x1.000000p-1f) && (l_mat < 0x1.400000p1f)) || ((l_mat > 0x1.300000p3f) && (l_mat < 0x1.500000p3f)));
  l_tone = select(coax_p_verse_style_anime_cel(l_w, l_bands.x, l_aa), coax_p_verse_style_anime_cel_char(l_w, l_ndv, l_bands2.x, l_bands2.y), l_character);
  if (((l_mat > 0x1.600000p2f) && (l_mat < 0x1.a00000p2f)))
  {
    l_tone = coax_p_mix(0x1.000000p0f, l_tone, l_shading);
  }
  l_hi = coax_p_al_lit_tone(l_albedo);
  l_lo = select(coax_p_al_shadow_tone(l_hi), coax_p_al_shadow_tone_char(l_hi), l_character);
  l_c_37 = coax_p_al_mix3(l_lo, l_hi, l_tone);
  {
    l_k = 0i;
    while ((l_k < 8i))
    {
      l_p = coax_p_wg_at4((160i + (i32(l_k) * 8i)));
      l_col = coax_p_wg_at4(((160i + (i32(l_k) * 8i)) + 4i));
      if ((l_col.w > 0.0f))
      {
        l_dl = coax_p_al_sub3(coax_p_v3(l_p.x, l_p.y, l_p.z), l_s.y);
        l_dd = coax_p_al_len3(l_dl);
        l_fall = coax_p_st_sat((0x1.000000p0f - (l_dd / coax_p_fmax(l_p.w, 0x1.47ae14p-7f))));
        l_a = coax_p_verse_style_anime_cel(((coax_p_st_sat(((coax_p_al_dot3(l_n, coax_p_al_scale3(l_dl, (0x1.000000p0f / coax_p_fmax(l_dd, 0x1.a36e2ep-14f)))) * 0x1.000000p-1f) + 0x1.000000p-1f)) * l_fall) * l_fall), 0x1.70a3d8p-3f, 0x1.47ae14p-6f);
        l_c_37 = coax_p_al_add3(l_c_37, coax_p_v3((((l_albedo.x * l_col.x) * l_col.w) * l_a), (((l_albedo.y * l_col.y) * l_col.w) * l_a), (((l_albedo.z * l_col.z) * l_col.w) * l_a)));
      }
      l_k = (l_k + 1i);
    }
  }
  if (((l_mat > 0x1.800000p0f) && (l_mat < 0x1.400000p1f)))
  {
    l_hh = coax_p_al_norm3(coax_p_al_add3(l_l, l_v));
    l_spot = (coax_p_verse_style_anime_spot(abs(coax_p_al_dot3(l_n, l_hh)), (l_bands2.z - 0x1.000000p-2f), 0x1.47ae14p-6f, l_bands2.w) * l_lit);
    l_c_37 = coax_p_al_add3(l_c_37, coax_p_al_scale3(coax_p_v3((l_hi.x + 0x1.99999ap-3f), (l_hi.y + 0x1.99999ap-3f), (l_hi.z + 0x1.000000p-2f)), (l_spot * 0x1.000000p-1f)));
  }
  l_r1 = coax_p_al_st(2i);
  l_r2 = coax_p_al_st(3i);
  l_strength = select((l_r1.w * 0x1.000000p-1f), l_r2.x, l_character);
  l_rim = coax_p_verse_style_anime_rim(abs(l_ndv), l_ndl, l_r2.y, l_r2.z, l_strength);
  l_c_37 = coax_p_al_add3(l_c_37, coax_p_v3((l_r1.x * l_rim), (l_r1.y * l_rim), (l_r1.z * l_rim)));
  l_pale = coax_p_al_st(10i);
  l_c_37 = coax_p_al_mix3(l_c_37, coax_p_v3(l_pale.x, l_pale.y, l_pale.z), l_share);
  l_ink = select(l_s.f6, (-0x1.000000p0f), ((l_mat > 0x1.e00000p2f) && (l_mat < 0x1.100000p3f)));
  return coax_t15(coax_p_v4(l_c_37.x, l_c_37.y, l_c_37.z, coax_p_verse_style_anime_glow_share(coax_p_verse_style_luma(l_c_37.x, l_c_37.y, l_c_37.z), l_b.x, l_b.y, 0.0f)), coax_p_v4(l_n.x, l_n.y, l_n.z, l_ink));
}
fn coax_p_bloom_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_texel: vec2f;
  var l_uv: vec2f;
  var l_c: vec3f;
  var l_k: i32;
  var l_o: vec2f;
  var l_t: vec4f;
  var l_d: vec2f;
  var l_c_11: vec3f;
  l_texel = coax_p_v2((0x1.000000p0f / f32((coax_p_wg_dims_2d()).x)), (0x1.000000p0f / f32((coax_p_wg_dims_2d()).y)));
  l_uv = coax_p_v2(((l_pos.x * coax_at_bloom_step(i32(3i))) * l_texel.x), ((l_pos.y * coax_at_bloom_step(i32(3i))) * l_texel.y));
  if ((coax_at_bloom_step(i32(2i)) < 0x1.000000p-1f))
  {
    l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
    {
      l_k = 0i;
      while ((l_k < 4i))
      {
        l_o = coax_p_v2((((f32((l_k & 1i)) * 0x1.000000p1f) - 0x1.000000p0f) * l_texel.x), (((f32((l_k >> u32(1i))) * 0x1.000000p1f) - 0x1.000000p0f) * l_texel.y));
        l_t = textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x + l_o.x), (l_uv.y + l_o.y)), 0.0f);
        l_c = coax_p_al_add3(l_c, coax_p_al_scale3(coax_p_v3(l_t.x, l_t.y, l_t.z), coax_p_fmin(l_t.w, 0x1.000000p0f)));
        l_k = (l_k + 1i);
      }
    }
    return coax_p_v4((l_c.x * 0x1.000000p-2f), (l_c.y * 0x1.000000p-2f), (l_c.z * 0x1.000000p-2f), 0x1.000000p0f);
  }
  l_d = coax_p_v2((coax_at_bloom_step(i32(0i)) * l_texel.x), (coax_at_bloom_step(i32(1i)) * l_texel.y));
  l_c_11 = coax_p_al_scale3(coax_p_al_xyz(textureSampleLevel(bloom_src, bloom_lin, l_uv, 0.0f)), 0x1.d0e560p-3f);
  l_c_11 = coax_p_al_add3(l_c_11, coax_p_al_scale3(coax_p_al_add3(coax_p_al_xyz(textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x + (l_d.x * 0x1.628f5cp0f)), (l_uv.y + (l_d.y * 0x1.628f5cp0f))), 0.0f)), coax_p_al_xyz(textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x - (l_d.x * 0x1.628f5cp0f)), (l_uv.y - (l_d.y * 0x1.628f5cp0f))), 0.0f))), 0x1.439582p-2f));
  l_c_11 = coax_p_al_add3(l_c_11, coax_p_al_scale3(coax_p_al_add3(coax_p_al_xyz(textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x + (l_d.x * 0x1.9d9168p1f)), (l_uv.y + (l_d.y * 0x1.9d9168p1f))), 0.0f)), coax_p_al_xyz(textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x - (l_d.x * 0x1.9d9168p1f)), (l_uv.y - (l_d.y * 0x1.9d9168p1f))), 0.0f))), 0x1.1eb852p-4f));
  return coax_p_v4(l_c_11.x, l_c_11.y, l_c_11.z, 0x1.000000p0f);
}
fn coax_p_post_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_p: coax_t20;
  var l_uv: vec2f;
  var l_light: vec3f;
  var l_ink: f32;
  var l_dark: f32;
  var l_bloom: vec3f;
  var l_m: f32;
  var l_c: vec3f;
  var l_tn: vec4f;
  var l_lum: f32;
  var l_asp: f32;
  var l_vx: f32;
  var l_vy: f32;
  var l_sp: f32;
  var l_dx: f32;
  var l_dy: f32;
  var l_lines: f32;
  var l_l2: f32;
  var l_g: vec3f;
  l_p = coax_p_i2(coax_p_wg_i32_f(l_pos.x), coax_p_wg_i32_f(l_pos.y));
  l_uv = coax_p_v2((l_pos.x / coax_at_u(i32(0i))), (l_pos.y / coax_at_u(i32(1i))));
  l_light = coax_p_al_xyz(coax_p_wg_load_2d(l_p, 0i));
  l_ink = (coax_p_al_ink(l_p) * (coax_p_al_st(31i)).x);
  l_dark = (coax_p_al_st(6i)).w;
  l_light = coax_p_al_mix3(l_light, coax_p_v3(coax_p_verse_style_anime_ink_colour(l_light.x, l_dark), coax_p_verse_style_anime_ink_colour(l_light.y, l_dark), coax_p_verse_style_anime_ink_colour(l_light.z, l_dark)), l_ink);
  l_bloom = coax_p_al_add3(coax_p_al_scale3(coax_p_al_xyz(textureSampleLevel(post_quarter, post_lin, l_uv, 0.0f)), 0x1.19999ap-1f), coax_p_al_scale3(coax_p_al_xyz(textureSampleLevel(post_eighth, post_lin, l_uv, 0.0f)), 0x1.ccccccp-2f));
  l_m = (coax_p_al_st(19i)).z;
  l_c = coax_p_v3(coax_p_verse_style_bloom_mix(l_light.x, l_bloom.x, l_m), coax_p_verse_style_bloom_mix(l_light.y, l_bloom.y, l_m), coax_p_verse_style_bloom_mix(l_light.z, l_bloom.z, l_m));
  l_tn = coax_p_al_st(18i);
  l_c = coax_p_v3(coax_p_verse_style_tone(l_c.x, l_tn.x), coax_p_verse_style_tone(l_c.y, l_tn.x), coax_p_verse_style_tone(l_c.z, l_tn.x));
  l_lum = coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z);
  l_c = coax_p_v3(coax_p_verse_style_saturation(l_c.x, l_lum, l_tn.y), coax_p_verse_style_saturation(l_c.y, l_lum, l_tn.y), coax_p_verse_style_saturation(l_c.z, l_lum, l_tn.y));
  l_c = coax_p_v3(coax_p_verse_style_lift(l_c.x, l_tn.z), coax_p_verse_style_lift(l_c.y, l_tn.z), coax_p_verse_style_lift(l_c.z, l_tn.z));
  l_asp = (coax_at_u(i32(0i)) / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f));
  l_vx = ((((l_uv.x * 0x1.000000p1f) - 0x1.000000p0f) * coax_p_fmin(l_asp, 0x1.99999ap0f)) / 0x1.99999ap0f);
  l_vy = ((l_uv.y * 0x1.000000p1f) - 0x1.000000p0f);
  l_c = coax_p_al_scale3(l_c, coax_p_verse_style_anime_vignette(l_vx, l_vy, (coax_p_al_st(10i)).w));
  l_sp = coax_at_u(i32(54i));
  if ((l_sp > 0.0f))
  {
    l_dx = ((l_uv.x - coax_at_u(i32(61i))) * l_asp);
    l_dy = (l_uv.y - coax_at_u(i32(62i)));
    l_lines = coax_p_verse_style_anime_speed_lines((coax_p_wg_atan2(l_dy, l_dx) + 0x1.921fb6p1f), (sqrt(((l_dx * l_dx) + (l_dy * l_dy))) * 0x1.000000p1f), coax_at_u(i32(2i)), 0x1.b80000p6f, 0x1.ae147ap-2f);
    l_c = coax_p_al_mix3(l_c, coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.d70a3ep-1f), ((l_lines * l_sp) * 0x1.99999ap-1f));
  }
  if ((coax_at_u(i32(53i)) > 0x1.000000p-1f))
  {
    l_l2 = coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z);
    l_c = select(coax_p_v3(0x1.000000p0f, 0x1.dc28f6p-1f, 0x1.b851ecp-1f), coax_p_v3(0x1.47ae14p-6f, 0.0f, 0x1.eb851ep-6f), (l_l2 > 0x1.ae147ap-2f));
  }
  l_c = coax_p_al_mix3(l_c, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_st_sat(coax_at_u(i32(52i))));
  l_g = coax_p_v3(coax_pow_bg_vs(coax_p_st_sat(l_c.x), (0x1.000000p0f / 0x1.19999ap1f)), coax_pow_bg_vs(coax_p_st_sat(l_c.y), (0x1.000000p0f / 0x1.19999ap1f)), coax_pow_bg_vs(coax_p_st_sat(l_c.z), (0x1.000000p0f / 0x1.19999ap1f)));
  return coax_p_v4((coax_p_al_panels(coax_p_v2(l_pos.x, l_pos.y), l_g)).x, (coax_p_al_panels(coax_p_v2(l_pos.x, l_pos.y), l_g)).y, (coax_p_al_panels(coax_p_v2(l_pos.x, l_pos.y), l_g)).z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t14) -> vec4f {
  var l_v: coax_t14 = p_v;
  var l_tint: vec3f;
  var l_lod: f32;
  var l_sa: f32;
  var l_c: vec3f;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  var l_c_13: vec3f;
  l_tint = coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z);
  if (((l_v.w.w > 0x1.c00000p1f) && (l_v.w.w < 0x1.200000p2f)))
  {
    l_lod = (coax_p_st_sat((coax_p_wg_log2((0x1.000000p6f / coax_p_fmax(l_v.z.x, 0x1.000000p0f))) / 0x1.000000p2f)) * 0x1.000000p2f);
    l_sa = coax_pow_bg_vs((textureSampleLevel(sans_atlas, sans_sampler, l_v.y, l_lod)).w, 0x1.b33334p-1f);
    l_c = coax_p_al_scale3(l_tint, (l_sa * coax_p_fmin((l_v.z.z * 0x1.266666p0f), 0x1.000000p0f)));
    return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = sqrt(((l_along * l_along) + (l_v.y.y * l_v.y.y)));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.c28f5cp-3f);
  l_c_13 = coax_p_al_scale3(l_tint, ((l_core + l_halo) * l_v.z.z));
  return coax_p_v4(l_c_13.x, l_c_13.y, l_c_13.z, 0x1.000000p0f);
}
fn coax_p_al_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_al_place(p_id: u32, p_instance: u32) -> vec3f {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_b: u32;
  l_b = (l_id * 6u);
  if ((l_instance == 0u))
  {
    return coax_p_v3(coax_p_al_word(l_b), coax_p_al_word((l_b + 1u)), coax_p_al_word((l_b + 2u)));
  }
  return coax_p_v3(coax_p_al_word_1(l_b), coax_p_al_word_1((l_b + 1u)), coax_p_al_word_1((l_b + 2u)));
}
fn coax_p_al_rows(p_at: i32, p_w: vec3f) -> vec4f {
  var l_at: i32 = p_at;
  var l_w: vec3f = p_w;
  var l_p: vec4f;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  return coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(l_at), l_p), coax_p_wg_dot4(coax_p_wg_at4((l_at + 4i)), l_p), coax_p_wg_dot4(coax_p_wg_at4((l_at + 8i)), l_p), coax_p_wg_dot4(coax_p_wg_at4((l_at + 12i)), l_p));
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_al_decode1(p_c: f32) -> f32 {
  var l_c: f32 = p_c;
  return coax_pow_bg_vs(l_c, 0x1.19999ap1f);
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_al_ray(p_pos: vec4f) -> vec3f {
  var l_pos: vec4f = p_pos;
  var l_x: f32;
  var l_y: f32;
  var l_t: f32;
  var l_d: vec3f;
  l_x = (((l_pos.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f);
  l_y = (0x1.000000p0f - ((l_pos.y / coax_at_u(i32(1i))) * 0x1.000000p1f));
  l_t = coax_at_u(i32(23i));
  l_d = coax_p_al_add3(coax_p_al_xyz(coax_p_wg_at4(24i)), coax_p_al_add3(coax_p_al_scale3(coax_p_al_xyz(coax_p_wg_at4(28i)), ((l_x * l_t) * coax_at_u(i32(27i)))), coax_p_al_scale3(coax_p_al_xyz(coax_p_wg_at4(32i)), (l_y * l_t))));
  return coax_p_al_norm3(l_d);
}
fn coax_p_al_sky(p_d: vec3f) -> vec4f {
  var l_d: vec3f = p_d;
  var l_up: f32;
  var l_z: vec4f;
  var l_h: vec4f;
  var l_sky: vec3f;
  var l_sun: vec3f;
  var l_flat: f32;
  var l_sflat: f32;
  var l_cs: f32;
  var l_hk: vec4f;
  var l_g: f32;
  var l_az: f32;
  var l_stars: f32;
  var l_emissive: f32;
  var l_m: f32;
  var l_ms: f32;
  var l_disc: f32;
  var l_top: f32;
  var l_aa: f32;
  var l_inside: f32;
  var l_grain: f32;
  var l_lit: f32;
  var l_tone: f32;
  var l_ct: vec4f;
  var l_cloud: vec3f;
  var l_k: vec4f;
  var l_k2: vec4f;
  var l_q: vec2f;
  var l_n: f32;
  var l_cover: f32;
  var l_n2: f32;
  var l_tone_32: f32;
  var l_ct_33: vec4f;
  var l_cloud_34: vec3f;
  var l_ridge: f32;
  var l_side: f32;
  var l_litm: f32;
  var l_base: vec3f;
  var l_fade: f32;
  var l_hz: vec4f;
  var l_r: vec3f;
  var l_band: f32;
  l_up = l_d.y;
  l_z = coax_p_al_st(12i);
  l_h = coax_p_al_st(13i);
  l_sky = coax_p_v3(coax_p_verse_style_anime_sky(l_z.x, l_h.x, l_up, l_h.w), coax_p_verse_style_anime_sky(l_z.y, l_h.y, l_up, l_h.w), coax_p_verse_style_anime_sky(l_z.z, l_h.z, l_up, l_h.w));
  l_sun = coax_p_al_sun();
  l_flat = sqrt(coax_p_fmax(((l_d.x * l_d.x) + (l_d.z * l_d.z)), 0x1.0c6f7ap-20f));
  l_sflat = sqrt(coax_p_fmax(((l_sun.x * l_sun.x) + (l_sun.z * l_sun.z)), 0x1.0c6f7ap-20f));
  l_cs = (((l_d.x * l_sun.x) + (l_d.z * l_sun.z)) / (l_flat * l_sflat));
  l_hk = coax_p_al_st(11i);
  l_g = coax_p_verse_style_anime_glow(coax_p_fmax(l_up, 0.0f), l_cs, l_z.w, l_hk.w);
  l_sky = coax_p_al_mix3(l_sky, coax_p_v3(l_hk.x, l_hk.y, l_hk.z), l_g);
  l_az = coax_p_wg_atan2(l_d.x, l_d.z);
  l_stars = (((coax_p_al_st(14i)).w * coax_p_verse_style_anime_star((l_az * 0x1.e00000p5f), (l_up * 0x1.e00000p5f), 0x1.47ae14p-4f, coax_at_u(i32(2i)))) * coax_p_verse_style_soft_step(0x1.eb851ep-4f, 0x1.eb851ep-5f, l_up));
  l_sky = coax_p_al_add3(l_sky, coax_p_v3((l_stars * 0x1.ccccccp-1f), (l_stars * 0x1.e66666p-1f), l_stars));
  l_emissive = 0.0f;
  l_m = coax_p_al_dot3(l_d, coax_p_al_moon_dir());
  l_ms = coax_at_u(i32(63i));
  l_disc = coax_p_verse_style_soft_step(0x1.fedfa4p-1f, 0x1.0624dep-12f, l_m);
  l_sky = coax_p_al_add3(l_sky, coax_p_al_scale3(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.3d70a4p-1f), (l_ms * (0x1.000000p-2f * exp(((-0x1.e00000p6f) * (0x1.000000p0f - l_m)))))));
  l_sky = coax_p_al_mix3(l_sky, coax_p_v3(0x1.99999ap0f, 0x1.733334p0f, 0x1.266666p0f), (l_disc * l_ms));
  l_emissive = coax_p_fmax(l_emissive, (l_disc * l_ms));
  l_top = coax_p_verse_style_anime_cumulus_top(l_az, 0x1.5c28f6p-2f, 0x1.b33334p0f);
  l_aa = 0x1.0624dep-8f;
  l_inside = (coax_p_verse_style_anime_cumulus(l_up, l_top, l_aa) * coax_p_verse_style_soft_step((-0x1.47ae14p-7f), 0x1.47ae14p-7f, l_up));
  if ((l_inside > 0.0f))
  {
    l_grain = coax_p_verse_style_noise((l_az * 0x1.c00000p2f), (l_up * 0x1.600000p4f));
    l_lit = coax_p_verse_style_anime_cumulus_lit(l_up, l_top, l_cs, l_grain);
    l_tone = coax_p_verse_style_anime_cloud_tone(l_lit, 0x1.89374cp-7f);
    l_ct = coax_p_al_st(17i);
    l_cloud = coax_p_v3(coax_p_verse_style_anime_cloud_colour(l_ct.x, l_z.x, l_ct.w, l_tone), coax_p_verse_style_anime_cloud_colour(l_ct.y, l_z.y, l_ct.w, l_tone), coax_p_verse_style_anime_cloud_colour(l_ct.z, l_z.z, l_ct.w, l_tone));
    l_cloud = coax_p_al_mix3(l_cloud, coax_p_v3(l_hk.x, l_hk.y, l_hk.z), ((l_g * 0x1.19999ap-1f) * (0x1.000000p0f - l_tone)));
    l_sky = coax_p_al_mix3(l_sky, l_cloud, l_inside);
  }
  if ((l_up > 0x1.47ae14p-5f))
  {
    l_k = coax_p_al_st(15i);
    l_k2 = coax_p_al_st(16i);
    l_q = coax_p_v2(((l_d.x / l_up) * l_k.y), ((l_d.z / l_up) * l_k.y));
    l_n = coax_p_verse_style_cloud_field((l_q.x + (coax_at_u(i32(2i)) * l_k.z)), l_q.y, l_k2.x, l_k2.y, l_k.w);
    l_cover = (coax_p_verse_style_cloud_cover(l_n, l_k.x, l_k2.z, l_k2.w) * coax_p_verse_style_soft_step(0x1.eb851ep-4f, 0x1.47ae14p-4f, l_up));
    if ((l_cover > 0.0f))
    {
      l_n2 = coax_p_verse_style_cloud_field(((l_q.x + (coax_at_u(i32(2i)) * l_k.z)) + (l_sun.x * 0x1.2c0000p8f)), (l_q.y + (l_sun.z * 0x1.2c0000p8f)), l_k2.x, l_k2.y, l_k.w);
      l_tone_32 = coax_p_verse_style_anime_cloud_tone(coax_p_st_sat((0x1.000000p-1f + ((l_n - l_n2) * 0x1.800000p2f))), 0x1.47ae14p-6f);
      l_ct_33 = coax_p_al_st(17i);
      l_cloud_34 = coax_p_v3(coax_p_verse_style_anime_cloud_colour(l_ct_33.x, l_z.x, l_ct_33.w, l_tone_32), coax_p_verse_style_anime_cloud_colour(l_ct_33.y, l_z.y, l_ct_33.w, l_tone_32), coax_p_verse_style_anime_cloud_colour(l_ct_33.z, l_z.z, l_ct_33.w, l_tone_32));
      l_sky = coax_p_al_mix3(l_sky, l_cloud_34, l_cover);
    }
  }
  l_ridge = ((0x1.1eb852p-5f + (0x1.333334p-4f * coax_p_verse_style_noise((l_az * 0x1.266666p1f), 0x1.000000p2f))) + (0x1.cac084p-6f * coax_p_verse_style_noise((l_az * 0x1.200000p3f), 0x1.000000p0f)));
  l_ridge = (l_ridge * (0x1.19999ap-1f + (0x1.ccccccp-2f * coax_p_verse_style_soft_step(0.0f, 0x1.333334p-1f, (-l_d.z)))));
  if (((l_up < l_ridge) && (l_up > (-0x1.89374cp-7f))))
  {
    l_side = coax_p_verse_style_noise(((l_az * 0x1.200000p3f) + (l_up * 0x1.400000p5f)), 0x1.000000p1f);
    l_litm = coax_p_verse_style_soft_step(0x1.000000p-1f, 0x1.47ae14p-6f, (l_side + (0x1.000000p-2f * l_cs)));
    l_base = coax_p_al_mix3(coax_p_v3(0x1.99999ap-4f, 0x1.70a3d8p-4f, 0x1.c28f5cp-3f), coax_p_v3(0x1.0a3d70p-2f, 0x1.47ae14p-3f, 0x1.47ae14p-2f), l_litm);
    l_fade = (0x1.19999ap-1f + (0x1.333334p-2f * coax_p_st_sat((l_up / coax_p_fmax(l_ridge, 0x1.0624dep-10f)))));
    l_hz = coax_p_al_st(10i);
    l_sky = coax_p_al_mix3(l_base, coax_p_v3(l_hz.x, l_hz.y, l_hz.z), (l_fade * 0x1.666666p-1f));
  }
  if ((l_up <= (-0x1.89374cp-7f)))
  {
    l_r = coax_p_al_sky_reflect(coax_p_v3(l_d.x, (-l_up), l_d.z));
    l_band = select(0.0f, 0x1.eb851ep-4f, (coax_p_fract((((-0x1.000000p0f) / l_up) * 0x1.ccccccp-1f)) < 0x1.eb851ep-4f));
    l_sky = coax_p_al_add3(coax_p_al_scale3(l_r, 0x1.ae147ap-2f), coax_p_v3((l_band * l_hk.x), (l_band * l_hk.y), (l_band * l_hk.z)));
  }
  return coax_p_v4(l_sky.x, l_sky.y, l_sky.z, l_emissive);
}
fn coax_p_al_st(p_slot: i32) -> vec4f {
  var l_slot: i32 = p_slot;
  return coax_p_wg_at4((224i + (l_slot * 4i)));
}
fn coax_p_verse_style_luma(p_r: f32, p_g: f32, p_b: f32) -> f32 {
  var l_r: f32 = p_r;
  var l_g: f32 = p_g;
  var l_b: f32 = p_b;
  return (((0x1.b367a0p-3f * l_r) + (0x1.6e2eb2p-1f * l_g)) + (0x1.27bb30p-4f * l_b));
}
fn coax_p_verse_style_anime_glow_share(p_an_lum: f32, p_an_threshold: f32, p_an_knee: f32, p_an_emissive: f32) -> f32 {
  var l_an_lum: f32 = p_an_lum;
  var l_an_threshold: f32 = p_an_threshold;
  var l_an_knee: f32 = p_an_knee;
  var l_an_emissive: f32 = p_an_emissive;
  return coax_p_st_max(coax_p_verse_style_bloom_share(l_an_lum, l_an_threshold, l_an_knee), coax_p_st_sat(l_an_emissive));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_al_xyz(p_a: vec4f) -> vec3f {
  var l_a: vec4f = p_a;
  return coax_p_v3(l_a.x, l_a.y, l_a.z);
}
fn coax_p_al_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_al_len3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(coax_p_al_dot3(l_a, l_a));
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
fn coax_p_al_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return coax_p_v3((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_al_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_al_norm3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_al_scale3(l_a, (0x1.000000p0f / coax_p_fmax(coax_p_al_len3(l_a), 0x1.0c6f7ap-20f)));
}
fn coax_p_al_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return (((l_a.x * l_b.x) + (l_a.y * l_b.y)) + (l_a.z * l_b.z));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
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
fn coax_p_verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return coax_p_render_noise2(l_x, l_y);
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
fn coax_p_al_key() -> vec3f {
  return coax_p_al_norm3(coax_p_al_xyz(coax_p_al_st(1i)));
}
fn coax_p_al_lit(p_w: vec3f, p_n: vec3f) -> f32 {
  var l_w: vec3f = p_w;
  var l_n: vec3f = p_n;
  var l_q: vec4f;
  var l_p: vec3f;
  var l_uv: vec2f;
  var l_t: f32;
  var l_lit: f32;
  l_q = coax_p_al_rows(36i, coax_p_al_add3(l_w, coax_p_al_scale3(l_n, 0x1.47ae14p-5f)));
  l_p = coax_p_v3((l_q.x / l_q.w), (l_q.y / l_q.w), (l_q.z / l_q.w));
  l_uv = coax_p_v2(((l_p.x * 0x1.000000p-1f) + 0x1.000000p-1f), (0x1.000000p-1f - (l_p.y * 0x1.000000p-1f)));
  if ((((((l_uv.x < 0.0f) || (l_uv.x > 0x1.000000p0f)) || (l_uv.y < 0.0f)) || (l_uv.y > 0x1.000000p0f)) || (l_p.z > 0x1.000000p0f)))
  {
    return 0x1.000000p0f;
  }
  l_t = (0x1.000000p0f / f32((coax_p_wg_dims_depth()).x));
  l_lit = textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((l_uv.x - (0x1.000000p-1f * l_t)), (l_uv.y - (0x1.000000p-1f * l_t))), (l_p.z - 0x1.89374cp-10f));
  l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((l_uv.x + (0x1.000000p-1f * l_t)), (l_uv.y - (0x1.000000p-1f * l_t))), (l_p.z - 0x1.89374cp-10f)));
  l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((l_uv.x - (0x1.000000p-1f * l_t)), (l_uv.y + (0x1.000000p-1f * l_t))), (l_p.z - 0x1.89374cp-10f)));
  l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((l_uv.x + (0x1.000000p-1f * l_t)), (l_uv.y + (0x1.000000p-1f * l_t))), (l_p.z - 0x1.89374cp-10f)));
  return (l_lit * 0x1.000000p-2f);
}
fn coax_p_verse_style_anime_light(p_an_ndl: f32, p_an_lit: f32) -> f32 {
  var l_an_ndl: f32 = p_an_ndl;
  var l_an_lit: f32 = p_an_lit;
  return (coax_p_st_sat(((l_an_ndl * 0x1.000000p-1f) + 0x1.000000p-1f)) * coax_p_st_sat(l_an_lit));
}
fn coax_p_verse_style_anime_aerial(p_an_d: f32, p_an_start: f32, p_an_density: f32, p_an_cap: f32) -> f32 {
  var l_an_d: f32 = p_an_d;
  var l_an_start: f32 = p_an_start;
  var l_an_density: f32 = p_an_density;
  var l_an_cap: f32 = p_an_cap;
  return (l_an_cap * (0x1.000000p0f - exp(((-l_an_density) * coax_p_st_max((l_an_d - l_an_start), 0.0f)))));
}
fn coax_p_verse_style_anime_far_aa(p_an_aa: f32, p_an_share: f32) -> f32 {
  var l_an_aa: f32 = p_an_aa;
  var l_an_share: f32 = p_an_share;
  return (l_an_aa * (0x1.000000p0f + (0x1.000000p3f * coax_p_st_sat(l_an_share))));
}
fn coax_p_verse_style_anime_cel_char(p_an_w: f32, p_an_ndv: f32, p_an_edge: f32, p_an_aa: f32) -> f32 {
  var l_an_w: f32 = p_an_w;
  var l_an_ndv: f32 = p_an_ndv;
  var l_an_edge: f32 = p_an_edge;
  var l_an_aa: f32 = p_an_aa;
  return coax_p_verse_style_soft_step((l_an_edge - (0x1.eb851ep-4f * coax_p_st_sat(l_an_ndv))), coax_p_st_max(l_an_aa, 0x1.0c6f7ap-20f), l_an_w);
}
fn coax_p_verse_style_anime_cel(p_an_w: f32, p_an_edge: f32, p_an_aa: f32) -> f32 {
  var l_an_w: f32 = p_an_w;
  var l_an_edge: f32 = p_an_edge;
  var l_an_aa: f32 = p_an_aa;
  return coax_p_verse_style_soft_step(l_an_edge, coax_p_st_max(l_an_aa, 0x1.0c6f7ap-20f), l_an_w);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_al_lit_tone(p_albedo: vec3f) -> vec3f {
  var l_albedo: vec3f = p_albedo;
  var l_key: vec4f;
  var l_amb: vec4f;
  var l_l: vec3f;
  l_key = coax_p_al_st(0i);
  l_amb = coax_p_al_st(5i);
  l_l = coax_p_v3(((l_key.x * l_key.w) + (l_amb.x * l_amb.w)), ((l_key.y * l_key.w) + (l_amb.y * l_amb.w)), ((l_key.z * l_key.w) + (l_amb.z * l_amb.w)));
  return coax_p_v3((l_albedo.x * l_l.x), (l_albedo.y * l_l.y), (l_albedo.z * l_l.z));
}
fn coax_p_al_shadow_tone_char(p_lit: vec3f) -> vec3f {
  var l_lit: vec3f = p_lit;
  var l_ti: vec4f;
  var l_bands: vec4f;
  var l_tl: f32;
  var l_s: vec3f;
  var l_before: f32;
  var l_after: f32;
  l_ti = coax_p_v4(0x1.000000p0f, 0x1.99999ap-1f, 0x1.a3d70ap-1f, 0.0f);
  l_bands = coax_p_al_st(7i);
  l_tl = coax_p_verse_style_luma(l_ti.x, l_ti.y, l_ti.z);
  l_s = coax_p_v3(coax_p_verse_style_anime_shadow(l_lit.x, l_ti.x, l_tl, l_bands.w), coax_p_verse_style_anime_shadow(l_lit.y, l_ti.y, l_tl, l_bands.w), coax_p_verse_style_anime_shadow(l_lit.z, l_ti.z, l_tl, l_bands.w));
  l_before = (coax_p_verse_style_luma(l_lit.x, l_lit.y, l_lit.z) * l_bands.w);
  l_after = coax_p_verse_style_luma(l_s.x, l_s.y, l_s.z);
  return coax_p_v3(coax_p_verse_style_keep_value(l_s.x, l_before, l_after), coax_p_verse_style_keep_value(l_s.y, l_before, l_after), coax_p_verse_style_keep_value(l_s.z, l_before, l_after));
}
fn coax_p_al_shadow_tone(p_lit: vec3f) -> vec3f {
  var l_lit: vec3f = p_lit;
  var l_ti: vec4f;
  var l_bands: vec4f;
  var l_tl: f32;
  var l_s: vec3f;
  var l_before: f32;
  var l_after: f32;
  l_ti = coax_p_al_st(4i);
  l_bands = coax_p_al_st(7i);
  l_tl = coax_p_verse_style_luma(l_ti.x, l_ti.y, l_ti.z);
  l_s = coax_p_v3(coax_p_verse_style_anime_shadow(l_lit.x, l_ti.x, l_tl, l_bands.w), coax_p_verse_style_anime_shadow(l_lit.y, l_ti.y, l_tl, l_bands.w), coax_p_verse_style_anime_shadow(l_lit.z, l_ti.z, l_tl, l_bands.w));
  l_before = (coax_p_verse_style_luma(l_lit.x, l_lit.y, l_lit.z) * l_bands.w);
  l_after = coax_p_verse_style_luma(l_s.x, l_s.y, l_s.z);
  return coax_p_v3(coax_p_verse_style_keep_value(l_s.x, l_before, l_after), coax_p_verse_style_keep_value(l_s.y, l_before, l_after), coax_p_verse_style_keep_value(l_s.z, l_before, l_after));
}
fn coax_p_al_mix3(p_a: vec3f, p_b: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_k: f32 = p_k;
  return coax_p_v3(coax_p_mix(l_a.x, l_b.x, l_k), coax_p_mix(l_a.y, l_b.y, l_k), coax_p_mix(l_a.z, l_b.z, l_k));
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_verse_style_anime_spot(p_an_ndh: f32, p_an_cut: f32, p_an_aa: f32, p_an_strength: f32) -> f32 {
  var l_an_ndh: f32 = p_an_ndh;
  var l_an_cut: f32 = p_an_cut;
  var l_an_aa: f32 = p_an_aa;
  var l_an_strength: f32 = p_an_strength;
  return (coax_p_verse_style_soft_step(l_an_cut, coax_p_st_max(l_an_aa, 0x1.0c6f7ap-20f), l_an_ndh) * l_an_strength);
}
fn coax_p_verse_style_anime_rim(p_an_ndv: f32, p_an_ndl: f32, p_an_start: f32, p_an_full: f32, p_an_strength: f32) -> f32 {
  var l_an_ndv: f32 = p_an_ndv;
  var l_an_ndl: f32 = p_an_ndl;
  var l_an_start: f32 = p_an_start;
  var l_an_full: f32 = p_an_full;
  var l_an_strength: f32 = p_an_strength;
  var l_an_edge: f32;
  l_an_edge = (0x1.000000p0f - coax_p_verse_style_soft_step(((l_an_start + l_an_full) * 0x1.000000p-1f), coax_p_st_max(((l_an_start - l_an_full) * 0x1.000000p-1f), 0x1.0c6f7ap-20f), l_an_ndv));
  return ((l_an_edge * (0x1.99999ap-3f + (0x1.99999ap-1f * coax_p_st_sat((l_an_ndl + 0x1.666666p-2f))))) * l_an_strength);
}
fn coax_p_wg_dims_2d() -> coax_t22 {
  return coax_p_u2(textureDimensions(bloom_src).x, textureDimensions(bloom_src).y);
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
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t20 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t20(l_x, l_y);
}
fn coax_p_wg_load_2d(p_c: coax_t20, p_level: i32) -> vec4f {
  var l_c: coax_t20 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_light, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_al_ink(p_p: coax_t20) -> f32 {
  var l_p: coax_t20 = p_p;
  var l_a: vec4f;
  var l_size: coax_t22;
  var l_dc: f32;
  var l_rim2: vec4f;
  var l_bl: vec4f;
  var l_wpx: f32;
  var l_o: f32;
  var l_na: vec3f;
  var l_edge: f32;
  var l_k: i32;
  var l_ang: f32;
  var l_q: coax_t20;
  var l_dq: f32;
  var l_b: vec4f;
  var l_gap: f32;
  var l_ndn: f32;
  var l_e: f32;
  l_a = coax_p_wg_load_2d_1(l_p, 0i);
  if ((l_a.w < 0.0f))
  {
    return 0.0f;
  }
  l_size = coax_p_wg_dims_depth_1();
  l_dc = coax_p_al_linear(coax_p_wg_load_depth(l_p, 0i));
  if ((l_dc > 0x1.86a000p15f))
  {
    return 0.0f;
  }
  l_rim2 = coax_p_al_st(3i);
  l_bl = coax_p_al_st(19i);
  l_wpx = (coax_p_verse_style_anime_ink_width(l_dc, l_rim2.w, l_bl.w) * coax_at_u(i32(55i)));
  l_o = coax_p_fmax((l_wpx * 0x1.000000p-1f), 0x1.800000p-1f);
  l_na = coax_p_v3(l_a.x, l_a.y, l_a.z);
  l_edge = 0.0f;
  {
    l_k = 0i;
    while ((l_k < 8i))
    {
      l_ang = (f32(l_k) * 0x1.921fb6p-1f);
      l_q = coax_p_al_clampi(coax_p_i2((l_p.x + coax_p_wg_i32_f(coax_p_wg_round((cos(l_ang) * l_o)))), (l_p.y + coax_p_wg_i32_f(coax_p_wg_round((sin(l_ang) * l_o))))), l_size);
      l_dq = coax_p_al_linear(coax_p_wg_load_depth(l_q, 0i));
      l_b = coax_p_wg_load_2d_1(l_q, 0i);
      l_gap = (l_dq - l_dc);
      l_ndn = select(coax_p_al_dot3(l_na, coax_p_v3(l_b.x, l_b.y, l_b.z)), 0x1.000000p0f, (l_b.w < 0.0f));
      l_e = coax_p_verse_style_anime_ink_edge(l_gap, l_dc, l_ndn);
      if ((((l_b.w >= 0.0f) && (abs((l_b.w - l_a.w)) > 0x1.000000p-1f)) && (l_gap > (-0x1.47ae14p-6f))))
      {
        l_e = coax_p_fmax(l_e, 0x1.b33334p-1f);
      }
      l_edge = coax_p_fmax(l_edge, l_e);
      l_k = (l_k + 1i);
    }
  }
  return coax_p_verse_style_anime_ink_cover(l_edge, l_wpx);
}
fn coax_p_verse_style_anime_ink_colour(p_an_c: f32, p_an_dark: f32) -> f32 {
  var l_an_c: f32 = p_an_c;
  var l_an_dark: f32 = p_an_dark;
  return (l_an_c * coax_p_st_max(l_an_dark, 0x1.47ae14p-5f));
}
fn coax_p_verse_style_bloom_mix(p_c: f32, p_bloom: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_bloom: f32 = p_bloom;
  var l_amount: f32 = p_amount;
  return (l_c + (l_bloom * l_amount));
}
fn coax_p_verse_style_tone(p_c: f32, p_exposure: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_exposure: f32 = p_exposure;
  return (0x1.000000p0f - exp(((-coax_p_st_max(l_c, 0.0f)) * l_exposure)));
}
fn coax_p_verse_style_saturation(p_c: f32, p_lum: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lum: f32 = p_lum;
  var l_amount: f32 = p_amount;
  return coax_p_st_max(0.0f, (l_lum + ((l_c - l_lum) * l_amount)));
}
fn coax_p_verse_style_lift(p_c: f32, p_lift: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lift: f32 = p_lift;
  return (l_lift + ((0x1.000000p0f - l_lift) * l_c));
}
fn coax_p_verse_style_anime_vignette(p_an_x: f32, p_an_y: f32, p_an_strength: f32) -> f32 {
  var l_an_x: f32 = p_an_x;
  var l_an_y: f32 = p_an_y;
  var l_an_strength: f32 = p_an_strength;
  return (0x1.000000p0f - (l_an_strength * coax_p_verse_style_soft_step(0x1.19999ap0f, 0x1.333334p-1f, ((l_an_x * l_an_x) + (l_an_y * l_an_y)))));
}
fn coax_p_wg_atan2(p_y: f32, p_x: f32) -> f32 {
  var l_y: f32 = p_y;
  var l_x: f32 = p_x;
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
fn coax_p_verse_style_anime_speed_lines(p_an_a: f32, p_an_r: f32, p_an_t: f32, p_an_count: f32, p_an_inner: f32) -> f32 {
  var l_an_a: f32 = p_an_a;
  var l_an_r: f32 = p_an_r;
  var l_an_t: f32 = p_an_t;
  var l_an_count: f32 = p_an_count;
  var l_an_inner: f32 = p_an_inner;
  var l_an_q: f32;
  var l_an_cell: f32;
  var l_an_f: f32;
  var l_an_w: f32;
  var l_an_line: f32;
  l_an_q = ((l_an_a / 0x1.921fb6p2f) * l_an_count);
  l_an_cell = floor(l_an_q);
  l_an_f = floor((l_an_t * 0x1.800000p3f));
  if ((coax_p_verse_style_hash(l_an_cell, l_an_f) < 0x1.000000p-1f))
  {
    return 0.0f;
  }
  l_an_w = (0x1.eb851ep-5f + (0x1.1eb852p-3f * coax_p_verse_style_hash((l_an_cell + 0x1.100000p4f), l_an_f)));
  l_an_line = (0x1.000000p0f - coax_p_verse_style_soft_step(l_an_w, 0x1.eb851ep-6f, abs(((l_an_q - l_an_cell) - 0x1.000000p-1f))));
  return (l_an_line * coax_p_verse_style_soft_step((l_an_inner + 0x1.333334p-3f), 0x1.333334p-3f, l_an_r));
}
fn coax_p_al_panels(p_px: vec2f, p_c: vec3f) -> vec3f {
  var l_px: vec2f = p_px;
  var l_c: vec3f = p_c;
  var l_out: vec3f;
  var l_rad: f32;
  var l_k: i32;
  var l_r: vec4f;
  var l_col: vec4f;
  var l_cx: f32;
  var l_cy: f32;
  var l_hx: f32;
  var l_hy: f32;
  var l_qx: f32;
  var l_qy: f32;
  var l_d: f32;
  var l_inside: f32;
  l_out = l_c;
  l_rad = (0x1.800000p2f * coax_p_fmax(coax_at_u(i32(3i)), 0x1.000000p0f));
  {
    l_k = 0i;
    while ((l_k < 12i))
    {
      l_r = coax_p_wg_at4((64i + (i32(l_k) * 8i)));
      l_col = coax_p_wg_at4(((64i + (i32(l_k) * 8i)) + 4i));
      if ((l_col.w > 0.0f))
      {
        l_cx = ((l_r.x + l_r.z) * 0x1.000000p-1f);
        l_cy = ((l_r.y + l_r.w) * 0x1.000000p-1f);
        l_hx = (((l_r.z - l_r.x) * 0x1.000000p-1f) - l_rad);
        l_hy = (((l_r.w - l_r.y) * 0x1.000000p-1f) - l_rad);
        l_qx = coax_p_fmax((abs((l_px.x - l_cx)) - l_hx), 0.0f);
        l_qy = coax_p_fmax((abs((l_px.y - l_cy)) - l_hy), 0.0f);
        l_d = (sqrt(((l_qx * l_qx) + (l_qy * l_qy))) - l_rad);
        l_inside = (0x1.000000p0f - coax_p_st_sat((l_d + 0x1.000000p-1f)));
        l_out = coax_p_al_mix3(l_out, coax_p_v3(l_col.x, l_col.y, l_col.z), (l_inside * l_col.w));
      }
      l_k = (l_k + 1i);
    }
  }
  return l_out;
}
fn coax_p_wg_log2(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (log(l_x) / 0x1.62e430p-1f);
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_al_word(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(world[i32(l_i)]);
}
fn coax_p_al_word_1(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(live[i32(l_i)]);
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_verse_style_anime_sky(p_an_zenith: f32, p_an_horizon: f32, p_an_up: f32, p_an_power: f32) -> f32 {
  var l_an_zenith: f32 = p_an_zenith;
  var l_an_horizon: f32 = p_an_horizon;
  var l_an_up: f32 = p_an_up;
  var l_an_power: f32 = p_an_power;
  return (l_an_horizon + ((l_an_zenith - l_an_horizon) * coax_p_verse_style_anime_sky_t(l_an_up, l_an_power)));
}
fn coax_p_al_sun() -> vec3f {
  var l_k: vec4f;
  var l_s: vec3f;
  l_k = coax_p_al_st(1i);
  l_s = coax_p_al_norm3(coax_p_v3(l_k.x, l_k.y, l_k.z));
  if ((l_k.w > 0x1.000000p-1f))
  {
    return coax_p_al_scale3(l_s, (-0x1.000000p0f));
  }
  return l_s;
}
fn coax_p_verse_style_anime_glow(p_an_up: f32, p_an_cs: f32, p_an_height: f32, p_an_strength: f32) -> f32 {
  var l_an_up: f32 = p_an_up;
  var l_an_cs: f32 = p_an_cs;
  var l_an_height: f32 = p_an_height;
  var l_an_strength: f32 = p_an_strength;
  var l_an_side: f32;
  var l_an_h: f32;
  l_an_side = coax_p_st_sat(((l_an_cs * 0x1.000000p-1f) + 0x1.000000p-1f));
  l_an_h = (coax_p_st_max(l_an_height, 0x1.0624dep-10f) * (0x1.333334p-1f + (0x1.99999ap-2f * l_an_side)));
  return (((0x1.000000p0f - coax_p_verse_style_soft_step((l_an_h * 0x1.000000p-1f), (l_an_h * 0x1.000000p-1f), l_an_up)) * (0x1.51eb86p-2f + (0x1.570a3ep-1f * l_an_side))) * l_an_strength);
}
fn coax_p_verse_style_anime_star(p_an_x: f32, p_an_y: f32, p_an_density: f32, p_an_t: f32) -> f32 {
  var l_an_x: f32 = p_an_x;
  var l_an_y: f32 = p_an_y;
  var l_an_density: f32 = p_an_density;
  var l_an_t: f32 = p_an_t;
  var l_an_ix: f32;
  var l_an_iy: f32;
  var l_an_h: f32;
  var l_an_fx: f32;
  var l_an_fy: f32;
  var l_an_r: f32;
  l_an_ix = floor(l_an_x);
  l_an_iy = floor(l_an_y);
  l_an_h = coax_p_verse_style_hash(l_an_ix, l_an_iy);
  if ((l_an_h < (0x1.000000p0f - l_an_density)))
  {
    return 0.0f;
  }
  l_an_fx = ((l_an_x - l_an_ix) - 0x1.000000p-1f);
  l_an_fy = ((l_an_y - l_an_iy) - 0x1.000000p-1f);
  l_an_r = sqrt(((l_an_fx * l_an_fx) + (l_an_fy * l_an_fy)));
  return ((0x1.000000p0f - coax_p_verse_style_soft_step(0x1.eb851ep-4f, 0x1.47ae14p-5f, l_an_r)) * (0x1.800000p-1f + (0x1.000000p-2f * sin(((l_an_t * 0x1.800000p1f) + (l_an_h * 0x1.400000p5f))))));
}
fn coax_p_verse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_al_moon_dir() -> vec3f {
  return coax_p_al_norm3(coax_p_v3((-0x1.3d70a4p-1f), 0x1.70a3d8p-2f, (-0x1.666666p-1f)));
}
fn coax_p_verse_style_anime_cumulus_top(p_an_a: f32, p_an_tall: f32, p_an_seed: f32) -> f32 {
  var l_an_a: f32 = p_an_a;
  var l_an_tall: f32 = p_an_tall;
  var l_an_seed: f32 = p_an_seed;
  var l_an_c: f32;
  var l_an_s: f32;
  var l_an_n1: f32;
  var l_an_n2: f32;
  l_an_c = cos(l_an_a);
  l_an_s = sin(l_an_a);
  l_an_n1 = coax_p_verse_style_noise(((l_an_c * 0x1.19999ap1f) + l_an_seed), ((l_an_s * 0x1.19999ap1f) + (l_an_seed * 0x1.666666p-1f)));
  l_an_n2 = coax_p_verse_style_noise((((l_an_c * 0x1.a00000p2f) + l_an_seed) + 0x1.600000p3f), (((l_an_s * 0x1.a00000p2f) + l_an_seed) + 0x1.800000p1f));
  return (l_an_tall * (0x1.eb851ep-4f + (((0x1.c28f5cp-1f * l_an_n1) * l_an_n1) * (0x1.800000p-1f + (0x1.000000p-2f * l_an_n2)))));
}
fn coax_p_verse_style_anime_cumulus(p_an_up: f32, p_an_top: f32, p_an_aa: f32) -> f32 {
  var l_an_up: f32 = p_an_up;
  var l_an_top: f32 = p_an_top;
  var l_an_aa: f32 = p_an_aa;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(l_an_top, coax_p_st_max(l_an_aa, 0x1.0c6f7ap-20f), l_an_up));
}
fn coax_p_verse_style_anime_cumulus_lit(p_an_up: f32, p_an_top: f32, p_an_cs: f32, p_an_grain: f32) -> f32 {
  var l_an_up: f32 = p_an_up;
  var l_an_top: f32 = p_an_top;
  var l_an_cs: f32 = p_an_cs;
  var l_an_grain: f32 = p_an_grain;
  return coax_p_st_sat((((0x1.19999ap-1f * coax_p_st_sat((l_an_up / coax_p_st_max(l_an_top, 0x1.0624dep-10f)))) + (0x1.666666p-2f * coax_p_st_sat(((l_an_cs * 0x1.000000p-1f) + 0x1.000000p-1f)))) + (0x1.000000p-2f * (l_an_grain - 0x1.000000p-1f))));
}
fn coax_p_verse_style_anime_cloud_tone(p_an_lit: f32, p_an_aa: f32) -> f32 {
  var l_an_lit: f32 = p_an_lit;
  var l_an_aa: f32 = p_an_aa;
  var l_an_a: f32;
  l_an_a = coax_p_st_max(l_an_aa, 0x1.0c6f7ap-20f);
  return ((0x1.000000p-1f * coax_p_verse_style_soft_step(0x1.666666p-2f, l_an_a, l_an_lit)) + (0x1.000000p-1f * coax_p_verse_style_soft_step(0x1.666666p-1f, l_an_a, l_an_lit)));
}
fn coax_p_verse_style_anime_cloud_colour(p_an_top: f32, p_an_zenith: f32, p_an_under: f32, p_an_tone: f32) -> f32 {
  var l_an_top: f32 = p_an_top;
  var l_an_zenith: f32 = p_an_zenith;
  var l_an_under: f32 = p_an_under;
  var l_an_tone: f32 = p_an_tone;
  var l_an_shade: f32;
  l_an_shade = (l_an_top + ((l_an_zenith - l_an_top) * coax_p_st_sat(l_an_under)));
  return (l_an_shade + ((l_an_top - l_an_shade) * coax_p_st_sat(l_an_tone)));
}
fn coax_p_verse_style_cloud_field(p_x: f32, p_z: f32, p_s1: f32, p_s2: f32, p_octaves: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_s1: f32 = p_s1;
  var l_s2: f32 = p_s2;
  var l_octaves: f32 = p_octaves;
  var l_n: f32;
  l_n = coax_p_verse_style_noise((l_x * l_s1), (l_z * l_s1));
  if ((l_octaves < 0x1.800000p0f))
  {
    return l_n;
  }
  return ((l_n * 0x1.4cccccp-1f) + (coax_p_verse_style_noise(((l_x * l_s2) + 0x1.8cccccp1f), ((l_z * l_s2) + 0x1.ecccccp2f)) * 0x1.666666p-2f));
}
fn coax_p_verse_style_cloud_cover(p_n: f32, p_cover: f32, p_cut_lo: f32, p_cut_hi: f32) -> f32 {
  var l_n: f32 = p_n;
  var l_cover: f32 = p_cover;
  var l_cut_lo: f32 = p_cut_lo;
  var l_cut_hi: f32 = p_cut_hi;
  return (coax_p_verse_style_soft_step(((l_cut_lo + l_cut_hi) * 0x1.000000p-1f), ((l_cut_hi - l_cut_lo) * 0x1.000000p-1f), (l_n + ((l_cover - 0x1.666666p-2f) * 0x1.99999ap-2f))) * coax_p_verse_style_soft_step(0x1.99999ap-6f, 0x1.99999ap-6f, l_cover));
}
fn coax_p_al_sky_reflect(p_d: vec3f) -> vec3f {
  var l_d: vec3f = p_d;
  var l_z: vec4f;
  var l_h: vec4f;
  var l_hk: vec4f;
  var l_sky: vec3f;
  var l_sun: vec3f;
  var l_cs: f32;
  l_z = coax_p_al_st(12i);
  l_h = coax_p_al_st(13i);
  l_hk = coax_p_al_st(11i);
  l_sky = coax_p_v3(coax_p_verse_style_anime_sky(l_z.x, l_h.x, l_d.y, l_h.w), coax_p_verse_style_anime_sky(l_z.y, l_h.y, l_d.y, l_h.w), coax_p_verse_style_anime_sky(l_z.z, l_h.z, l_d.y, l_h.w));
  l_sun = coax_p_al_sun();
  l_cs = (((l_d.x * l_sun.x) + (l_d.z * l_sun.z)) / coax_p_fmax((sqrt(((l_d.x * l_d.x) + (l_d.z * l_d.z))) * sqrt(((l_sun.x * l_sun.x) + (l_sun.z * l_sun.z)))), 0x1.0c6f7ap-20f));
  return coax_p_al_mix3(l_sky, coax_p_v3(l_hk.x, l_hk.y, l_hk.z), coax_p_verse_style_anime_glow(l_d.y, l_cs, l_z.w, l_hk.w));
}
fn coax_p_verse_style_bloom_share(p_lum: f32, p_threshold: f32, p_knee: f32) -> f32 {
  var l_lum: f32 = p_lum;
  var l_threshold: f32 = p_threshold;
  var l_knee: f32 = p_knee;
  var l_k: f32;
  var l_q: f32;
  if ((l_lum <= 0.0f))
  {
    return 0.0f;
  }
  l_k = coax_p_st_max(l_knee, 0x1.4f8b58p-17f);
  l_q = coax_p_st_min(coax_p_st_max(((l_lum - l_threshold) + l_k), 0.0f), (0x1.000000p1f * l_k));
  return coax_p_st_min((coax_p_st_max(((l_q * l_q) / (0x1.000000p2f * l_k)), (l_lum - l_threshold)) / l_lum), 0x1.000000p0f);
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
fn coax_p_render_noise2(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  if (((!coax_p_render_noise_coordinate_ok(l_x)) || (!coax_p_render_noise_coordinate_ok(l_y))))
  {
    return 0.0f;
  }
  return coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y)));
}
fn coax_p_wg_dims_depth() -> coax_t22 {
  return coax_p_u2(textureDimensions(shadow_map).x, textureDimensions(shadow_map).y);
}
fn coax_p_verse_style_anime_shadow(p_an_c: f32, p_an_tint: f32, p_an_tint_lum: f32, p_an_depth: f32) -> f32 {
  var l_an_c: f32 = p_an_c;
  var l_an_tint: f32 = p_an_tint;
  var l_an_tint_lum: f32 = p_an_tint_lum;
  var l_an_depth: f32 = p_an_depth;
  return (((l_an_c * l_an_depth) * coax_p_st_max(l_an_tint, 0x1.0c6f7ap-20f)) / coax_p_st_max(l_an_tint_lum, 0x1.0c6f7ap-20f));
}
fn coax_p_verse_style_keep_value(p_c: f32, p_before: f32, p_after: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_before: f32 = p_before;
  var l_after: f32 = p_after;
  return ((l_c * l_before) / coax_p_st_max(l_after, 0x1.0c6f7ap-20f));
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
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t22 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t22(l_x, l_y);
}
fn coax_p_wg_load_2d_1(p_c: coax_t20, p_level: i32) -> vec4f {
  var l_c: coax_t20 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_aux, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_dims_depth_1() -> coax_t22 {
  return coax_p_u2(textureDimensions(post_depth).x, textureDimensions(post_depth).y);
}
fn coax_p_wg_load_depth(p_c: coax_t20, p_level: i32) -> f32 {
  var l_c: coax_t20 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_depth, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_al_linear(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  var l_near: f32;
  var l_far: f32;
  l_near = coax_at_u(i32(31i));
  l_far = coax_at_u(i32(35i));
  if ((l_z >= 0x1.000000p0f))
  {
    return 0x1.86a000p16f;
  }
  return ((l_near * l_far) / (l_far - (l_z * (l_far - l_near))));
}
fn coax_p_verse_style_anime_ink_width(p_an_d: f32, p_an_width: f32, p_an_far: f32) -> f32 {
  var l_an_d: f32 = p_an_d;
  var l_an_width: f32 = p_an_width;
  var l_an_far: f32 = p_an_far;
  var l_an_f: f32;
  l_an_f = coax_p_st_max(l_an_far, 0x1.0624dep-10f);
  return (l_an_width * (0x1.000000p0f - (0x1.800000p-1f * coax_p_verse_style_soft_step((l_an_f * 0x1.19999ap-1f), (l_an_f * 0x1.ccccccp-2f), l_an_d))));
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
fn coax_p_al_clampi(p_p: coax_t20, p_size: coax_t22) -> coax_t20 {
  var l_p: coax_t20 = p_p;
  var l_size: coax_t22 = p_size;
  return coax_p_i2(coax_p_wg_clamp_i(l_p.x, 0i, (i32(l_size.x) - 1i)), coax_p_wg_clamp_i(l_p.y, 0i, (i32(l_size.y) - 1i)));
}
fn coax_p_verse_style_anime_ink_edge(p_an_gap: f32, p_an_d: f32, p_an_ndn: f32) -> f32 {
  var l_an_gap: f32 = p_an_gap;
  var l_an_d: f32 = p_an_d;
  var l_an_ndn: f32 = p_an_ndn;
  var l_an_at: f32;
  var l_an_sil: f32;
  var l_an_crease: f32;
  l_an_at = ((coax_p_st_max(l_an_d, 0.0f) * 0x1.47ae14p-5f) + 0x1.333334p-3f);
  l_an_sil = coax_p_verse_style_soft_step((l_an_at * 0x1.200000p0f), (l_an_at * 0x1.000000p-3f), l_an_gap);
  l_an_crease = (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.99999ap-1f, 0x1.47ae14p-5f, l_an_ndn));
  return coax_p_st_max(l_an_sil, l_an_crease);
}
fn coax_p_verse_style_anime_ink_cover(p_an_edge: f32, p_an_width_px: f32) -> f32 {
  var l_an_edge: f32 = p_an_edge;
  var l_an_width_px: f32 = p_an_width_px;
  return (coax_p_st_sat(l_an_edge) * coax_p_st_sat(l_an_width_px));
}
fn coax_p_wg_atan01(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
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
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_verse_style_anime_sky_t(p_an_up: f32, p_an_power: f32) -> f32 {
  var l_an_up: f32 = p_an_up;
  var l_an_power: f32 = p_an_power;
  return coax_pow_bg_vs(coax_p_st_max(coax_p_st_sat(l_an_up), 0x1.0c6f7ap-20f), l_an_power);
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
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
}
fn coax_p_wg_clamp_i(p_x: i32, p_low: i32, p_high: i32) -> i32 {
  var l_x: i32 = p_x;
  var l_low: i32 = p_low;
  var l_high: i32 = p_high;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_low), l_high);
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  var l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
}
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
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
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn shadow_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_9 {
  let coax_r = coax_p_shadow_vs(coax_vid, coax_iid);
  return coax_io_9(coax_r.x);
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_11 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_12) -> coax_io_14 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t12(coax_in.x, coax_in.y, coax_in.z));
  return coax_io_14(coax_r.x, coax_r.y, coax_r.z, coax_r.w);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_15 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_15(coax_r.x, coax_r.y);
}
@fragment
fn shadow_fs(coax_in: coax_io_9) {
  coax_p_shadow_fs(coax_t9(coax_in.x));
}
@fragment
fn scene_fs(coax_in: coax_io_11) -> coax_out_15 {
  let coax_r = coax_p_scene_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_out_15(coax_r.x, coax_r.y);
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
fn fx_fs(coax_in: coax_io_14) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t14(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
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