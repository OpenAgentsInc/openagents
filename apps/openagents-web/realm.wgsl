// ---- generated: apps/openagents-web/gpu/realm.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t14 {
  x: vec4f,
  y: vec2f,
  z: u32,
}
struct coax_io_14 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) @interpolate(flat) z: u32,
}
struct coax_t15 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec3f,
  f5: f32,
  f6: u32,
  f7: f32,
  f8: u32,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec3f,
  @location(4) f5: f32,
  @location(5) @interpolate(flat) f6: u32,
  @location(6) f7: f32,
  @location(7) @interpolate(flat) f8: u32,
}
struct coax_t16 {
  x: u32,
  y: u32,
  z: u32,
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
  x: vec4f,
  y: vec4f,
}
struct coax_out_19 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
struct coax_t22 {
  x: vec4f,
  y: vec4f,
}
struct coax_t27 {
  x: i32,
  y: i32,
}
struct coax_t29 {
  x: u32,
  y: u32,
}
struct coax_t32 {
  x: bool,
  y: vec4f,
}
struct coax_t33 {
  x: bool,
  y: i32,
  z: f32,
  w: f32,
  f4: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 64>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> objects: array<coax_t11>;
@group(1) @binding(3) var sheets: texture_2d_array<f32>;
@group(1) @binding(4) var sheet: sampler;
@group(1) @binding(2) var<storage, read> lights: array<coax_t22>;
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
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_shadow_vs(p_id: u32, p_instance: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_b: u32;
  var l_p: vec4f;
  var l_m: coax_t11;
  var l_q: vec3f;
  var l_o: coax_t14;
  l_b = (l_id * 12u);
  l_p = coax_p_v4(coax_p_realm_word(l_b), coax_p_realm_word((l_b + 1u)), coax_p_realm_word((l_b + 2u)), 0x1.000000p0f);
  l_m = objects[i32(l_instance)];
  l_q = coax_p_realm_shadow_place(coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p)));
  l_o = coax_t14();
  l_o.x = coax_p_wg_make_f4_f2ff(coax_p_v2(l_q.x, l_q.y), l_q.z, 0x1.000000p0f);
  if ((l_m.w.w > 0x1.000000p-1f))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, (-0x1.000000p0f), 0x1.000000p0f);
  }
  l_o.y = coax_p_v2(coax_p_realm_word((l_b + 6u)), coax_p_realm_word((l_b + 7u)));
  l_o.z = (vertices[i32((l_b + 8u))] >> u32(24u));
  return l_o;
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_b: u32;
  var l_p: vec4f;
  var l_n: vec3f;
  var l_colour: vec4f;
  var l_m: coax_t11;
  var l_w: vec3f;
  var l_wp: vec4f;
  var l_o: coax_t15;
  var l_tint: coax_t16;
  l_b = (l_id * 12u);
  l_p = coax_p_v4(coax_p_realm_word(l_b), coax_p_realm_word((l_b + 1u)), coax_p_realm_word((l_b + 2u)), 0x1.000000p0f);
  l_n = coax_p_v3(coax_p_realm_word((l_b + 3u)), coax_p_realm_word((l_b + 4u)), coax_p_realm_word((l_b + 5u)));
  l_colour = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 8u))]);
  l_m = objects[i32(l_instance)];
  l_w = coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p));
  l_wp = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  l_o = coax_t15();
  l_o.x = coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(12i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(16i), l_wp), coax_p_wg_dot4(coax_p_wg_at4(20i), l_wp));
  l_o.y = l_w;
  l_o.z = coax_p_wg_normalize3(coax_p_v3(coax_p_wg_dot3(coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z), l_n), coax_p_wg_dot3(coax_p_v3(l_m.y.x, l_m.y.y, l_m.y.z), l_n), coax_p_wg_dot3(coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z), l_n)));
  l_o.w = coax_p_v2(coax_p_realm_word((l_b + 6u)), coax_p_realm_word((l_b + 7u)));
  l_o.f4 = coax_p_wg_mul_f3f3(coax_p_v3(l_colour.x, l_colour.y, l_colour.z), coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z));
  l_o.f5 = l_m.w.w;
  l_o.f6 = (vertices[i32((l_b + 8u))] >> u32(24u));
  l_o.f7 = coax_p_realm_word((l_b + 9u));
  l_tint = coax_p_wg_cvt_u3f3(coax_p_wg_mul_f3f(coax_p_wg_clamp_f3f3f3(coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), 0x1.fe0000p7f));
  l_o.f8 = (((((l_o.f6 * 37u) + (l_tint.x * 3u)) + (l_tint.y * 5u)) + (l_tint.z * 11u)) & 127u);
  return l_o;
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t17) -> coax_t18 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t17 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t18;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t18();
  l_ndc = coax_p_v2((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_o.x = coax_p_v4(l_ndc.x, l_ndc.y, 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t19 {
  var l_pos: vec4f = p_pos;
  var l_v: f32;
  var l_sky: vec3f;
  l_v = coax_p_clampf((l_pos.y / coax_p_fmax(coax_at_u(i32(1i)), 0x1.000000p0f)), 0.0f, 0x1.000000p0f);
  l_sky = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.99999ap-6f, 0x1.eb851ep-6f, 0x1.0a3d70p-4f), coax_p_v3(0x1.1eb852p-5f, 0x1.47ae14p-5f, 0x1.1eb852p-4f), l_v);
  return coax_t19(coax_p_v4(l_sky.x, l_sky.y, l_sky.z, 0.0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_shadow_fs(p_s: coax_t14) {
  var l_s: coax_t14 = p_s;
  if (((textureSampleLevel(sheets, sheet, l_s.y, i32(l_s.z), 0.0f)).w < 0x1.666666p-2f))
  {
    discard;
  }
}
fn coax_p_scene_fs(p_s: coax_t15) -> coax_t19 {
  var l_s: coax_t15 = p_s;
  var l_tex: vec4f;
  var l_n: vec3f;
  var l_facing: f32;
  var l_moon: f32;
  var l_lit: vec3f;
  var l_count: u32;
  var l_i: u32;
  var l_l: coax_t22;
  var l_delta: vec3f;
  var l_d: f32;
  var l_lambert: f32;
  var l_colour: vec3f;
  var l_d_19: f32;
  var l_f: f32;
  var l_c: vec3f;
  var l_share: f32;
  var l_ink: u32;
  l_tex = coax_p_render_sample_layer8(l_s.w, i32(l_s.f6));
  if ((l_tex.w < 0x1.666666p-2f))
  {
    discard;
  }
  if (coax_p_realm_cut_away(l_s.y, coax_p_wg_normalize3(l_s.z)))
  {
    discard;
  }
  l_n = coax_p_wg_normalize3(l_s.z);
  l_facing = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, coax_p_realm_moon_dir()));
  l_moon = select(0.0f, (l_facing * coax_p_realm_moonlit(l_s.y, l_n)), (l_facing > 0.0f));
  l_lit = coax_p_wg_add_f3f3(coax_p_v3(coax_p_verse_haunt_ambient(l_n.y, 0.0f), coax_p_verse_haunt_ambient(l_n.y, 0x1.000000p0f), coax_p_verse_haunt_ambient(l_n.y, 0x1.000000p1f)), coax_p_wg_mul_ff3(l_moon, coax_p_v3(coax_p_verse_haunt_moonlight(0.0f), coax_p_verse_haunt_moonlight(0x1.000000p0f), coax_p_verse_haunt_moonlight(0x1.000000p1f))));
  l_count = coax_p_wg_u32_f(coax_at_u(i32(27i)));
  {
    l_i = 0u;
    while ((l_i < l_count))
    {
      l_l = lights[i32(l_i)];
      l_delta = coax_p_wg_sub_f3f3(coax_p_v3(l_l.x.x, l_l.x.y, l_l.x.z), l_s.y);
      l_d = coax_p_wg_length3(l_delta);
      l_lambert = coax_p_fmax(0x1.47ae14p-4f, coax_p_wg_dot3(l_n, coax_p_wg_div_f3f(l_delta, coax_p_fmax(l_d, 0x1.0624dep-10f))));
      l_lit = coax_p_wg_add_f3f3(l_lit, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_l.y.x, l_l.y.y, l_l.y.z), coax_p_verse_haunt_attenuate(l_d, l_l.x.w)), l_lambert), 0x1.19999ap1f));
      l_i = (l_i + 1u);
    }
  }
  l_colour = coax_p_wg_mul_f3f3(coax_p_v3(l_tex.x, l_tex.y, l_tex.z), l_s.f4);
  l_d_19 = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y));
  l_f = coax_at_u(i32(28i));
  l_c = coax_p_v3(coax_p_verse_haunt_shade(l_colour.x, l_lit.x, l_s.f5, l_d_19, 0.0f, l_f), coax_p_verse_haunt_shade(l_colour.y, l_lit.y, l_s.f5, l_d_19, 0x1.000000p0f, l_f), coax_p_verse_haunt_shade(l_colour.z, l_lit.z, l_s.f5, l_d_19, 0x1.000000p1f, l_f));
  l_share = coax_p_verse_haunt_bloom_share(coax_p_fmax(l_c.x, coax_p_fmax(l_c.y, l_c.z)), l_s.f5);
  l_ink = coax_p_wg_u32_f(coax_p_wg_round((coax_p_verse_outline_strength(l_s.f7, l_s.f5) * 0x1.e00000p3f)));
  return coax_t19(coax_p_v4(l_c.x, l_c.y, l_c.z, l_share), coax_p_v4(l_n.x, l_n.y, l_n.z, f32(((l_s.f8 * 16u) + l_ink))));
}
fn coax_p_bloom_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_texel_2: vec2f;
  var l_uv: vec2f;
  var l_c: vec3f;
  var l_k: i32;
  var l_t: vec4f;
  var l_d: vec2f;
  var l_c_10: vec3f;
  l_texel_2 = coax_p_wg_div_ff2(0x1.000000p0f, coax_p_wg_cvt_f2u2(coax_p_wg_dims_2d()));
  l_uv = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f(coax_p_v2(l_pos.x, l_pos.y), coax_at_bloom_step(i32(3i))), l_texel_2);
  if ((coax_at_bloom_step(i32(2i)) < 0x1.000000p-1f))
  {
    l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
    {
      l_k = 0i;
      while ((l_k < 4i))
      {
        l_t = textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f2(coax_p_wg_sub_f2f(coax_p_wg_mul_f2f(coax_p_v2(f32((l_k & 1i)), f32((l_k >> u32(1i)))), 0x1.000000p1f), 0x1.000000p0f), l_texel_2)), 0.0f);
        l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_v3(l_t.x, l_t.y, l_t.z), coax_p_fmin(l_t.w, 0x1.000000p0f)));
        l_k = (l_k + 1i);
      }
    }
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(l_c, 0x1.000000p-2f), 0x1.000000p0f);
  }
  l_d = coax_p_wg_mul_f2f2(coax_p_v2(coax_at_bloom_step(i32(0i)), coax_at_bloom_step(i32(1i))), l_texel_2);
  l_c_10 = coax_p_wg_mul_f3f(coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, l_uv, 0.0f)), 0x1.d0e560p-3f);
  l_c_10 = coax_p_wg_add_f3f3(l_c_10, coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f(l_d, 0x1.628f5cp0f)), 0.0f)), coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_sub_f2f2(l_uv, coax_p_wg_mul_f2f(l_d, 0x1.628f5cp0f)), 0.0f))), 0x1.439582p-2f));
  l_c_10 = coax_p_wg_add_f3f3(l_c_10, coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f(l_d, 0x1.9d9168p1f)), 0.0f)), coax_p_wg_sw_rgb_f4(textureSampleLevel(bloom_src, bloom_lin, coax_p_wg_sub_f2f2(l_uv, coax_p_wg_mul_f2f(l_d, 0x1.9d9168p1f)), 0.0f))), 0x1.1eb852p-4f));
  return coax_p_v4(l_c_10.x, l_c_10.y, l_c_10.z, 0x1.000000p0f);
}
fn coax_p_post_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_p: coax_t27;
  var l_uv: vec2f;
  var l_light: vec3f;
  var l_near: vec3f;
  var l_far: vec3f;
  var l_c: vec3f;
  l_p = coax_p_wg_cvt_i2f2(coax_p_v2(l_pos.x, l_pos.y));
  l_uv = coax_p_wg_div_f2f2(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_at2(0i));
  l_light = coax_p_realm_outlined(l_p, coax_p_wg_sw_rgb_f4(coax_p_wg_load_2d(l_p, 0i)));
  l_near = coax_p_wg_sw_rgb_f4(textureSampleLevel(post_quarter, post_lin, l_uv, 0.0f));
  l_far = coax_p_wg_sw_rgb_f4(textureSampleLevel(post_eighth, post_lin, l_uv, 0.0f));
  l_c = coax_p_v3(coax_p_verse_haunt_glow(l_light.x, l_near.x, l_far.x), coax_p_verse_haunt_glow(l_light.y, l_near.y, l_far.y), coax_p_verse_haunt_glow(l_light.z, l_near.z, l_far.z));
  return coax_p_wg_make_f4_f3f(coax_p_realm_panels(coax_p_v2(l_pos.x, l_pos.y), coax_p_realm_tone(l_c)), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t18) -> vec4f {
  var l_v: coax_t18 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_p: vec2f;
  var l_c: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  if ((l_v.w.w > 0x1.c00000p1f))
  {
    l_p = coax_p_wg_div_f2f(l_v.y, (0x1.000000p0f - (0x1.eb851ep-6f * l_v.f4.x)));
    l_c = coax_p_verse_bar((-l_p.x), l_p.y, l_v.z.x, l_v.z.y, l_v.z.z, coax_at_u(i32(2i)), l_v.f4.x, l_v.f4.y);
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_c), l_v.z.w), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
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
fn coax_p_realm_word(p_i: u32) -> f32 {
  var l_i: u32 = p_i;
  return bitcast<f32>(vertices[i32(l_i)]);
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_realm_shadow_place(p_w: vec3f) -> vec3f {
  var l_w: vec3f = p_w;
  var l_l: vec3f;
  var l_r: vec3f;
  var l_up: vec3f;
  var l_reach: f32;
  var l_deep: f32;
  var l_snap: f32;
  var l_c: vec3f;
  l_l = coax_p_realm_moon_dir();
  l_r = coax_p_wg_normalize3(coax_p_wg_cross3(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_l));
  l_up = coax_p_wg_cross3(l_l, l_r);
  l_reach = coax_p_verse_haunt_shadow_reach(0.0f);
  l_deep = coax_p_verse_haunt_shadow_reach(0x1.000000p0f);
  l_snap = ((l_reach * 0x1.000000p1f) / 0x1.000000p10f);
  l_c = coax_p_v3((floor((coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_r) / l_snap)) * l_snap), (floor((coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_up) / l_snap)) * l_snap), coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_l));
  return coax_p_v3(((coax_p_wg_dot3(l_w, l_r) - l_c.x) / l_reach), ((coax_p_wg_dot3(l_w, l_up) - l_c.y) / l_reach), (0x1.000000p-1f - ((coax_p_wg_dot3(l_w, l_l) - l_c.z) / (0x1.000000p1f * l_deep))));
}
fn coax_p_wg_make_f4_f2ff(p_p0: vec2f, p_p1: f32, p_p2: f32) -> vec4f {
  var l_p0: vec2f = p_p0;
  var l_p1: f32 = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v4(l_p0.x, l_p0.y, l_p1, l_p2);
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_wg_clamp_f3f3f3(p_p0: vec3f, p_p1: vec3f, p_p2: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: vec3f = p_p2;
  return coax_p_v3(coax_p_clampf(l_p0.x, l_p1.x, l_p2.x), coax_p_clampf(l_p0.y, l_p1.y, l_p2.y), coax_p_clampf(l_p0.z, l_p1.z, l_p2.z));
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_wg_cvt_u3f3(p_p: vec3f) -> coax_t16 {
  var l_p: vec3f = p_p;
  return coax_p_u3(coax_p_wg_u32_f(l_p.x), coax_p_wg_u32_f(l_p.y), coax_p_wg_u32_f(l_p.z));
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
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_render_sample_layer8(p_uv: vec2f, p_layer: i32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_rs_no_bias: f32;
  l_rs_no_bias = 0.0f;
  return coax_p_render_sample_layer8_bias(l_uv, l_layer, l_rs_no_bias);
}
fn coax_p_realm_cut_away(p_w: vec3f, p_n: vec3f) -> bool {
  var l_w: vec3f = p_w;
  var l_n: vec3f = p_n;
  var l_chest: vec3f;
  var l_line: vec3f;
  var l_span: f32;
  var l_along: f32;
  var l_off: f32;
  l_chest = coax_p_v3(coax_at_u(i32(29i)), coax_at_u(i32(30i)), coax_at_u(i32(31i)));
  if ((l_chest.y < (-0x1.f40000p9f)))
  {
    return false;
  }
  if (((l_n.y > 0x1.666666p-1f) && (l_w.y < l_chest.y)))
  {
    return false;
  }
  if (((coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(l_w.x, l_w.z), coax_p_v2(l_chest.x, l_chest.z))) < 0x1.99999ap-2f) && (l_w.y < (l_chest.y + 0x1.000000p0f))))
  {
    return false;
  }
  l_line = coax_p_wg_sub_f3f3(l_chest, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))));
  l_span = coax_p_wg_length3(l_line);
  if ((l_span < 0x1.333334p-2f))
  {
    return false;
  }
  l_along = (coax_p_wg_dot3(coax_p_wg_sub_f3f3(l_w, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))), l_line) / l_span);
  if (((l_along <= 0.0f) || (l_along > (l_span - 0x1.99999ap-3f))))
  {
    return false;
  }
  if ((l_w.y < (l_chest.y - 0x1.19999ap0f)))
  {
    return false;
  }
  l_off = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_w, coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), coax_p_wg_mul_f3f(l_line, (l_along / l_span)))));
  return (l_off < (0x1.0cccccp0f + (0x1.666666p-2f * (0x1.000000p0f - (l_along / l_span)))));
}
fn coax_p_realm_moon_dir() -> vec3f {
  return coax_p_v3(coax_p_verse_haunt_moon_dir(0.0f), coax_p_verse_haunt_moon_dir(0x1.000000p0f), coax_p_verse_haunt_moon_dir(0x1.000000p1f));
}
fn coax_p_realm_moonlit(p_w: vec3f, p_n: vec3f) -> f32 {
  var l_w: vec3f = p_w;
  var l_n: vec3f = p_n;
  var l_q: vec3f;
  var l_uv: vec2f;
  var l_texel_2: f32;
  var l_taps: array<vec2f, 12>;
  var l_lit: f32;
  var l_k: i32;
  l_q = coax_p_realm_shadow_place(coax_p_wg_add_f3f3(l_w, coax_p_wg_mul_f3f(l_n, 0x1.eb851ep-5f)));
  l_uv = coax_p_v2(((l_q.x * 0x1.000000p-1f) + 0x1.000000p-1f), (0x1.000000p-1f - (l_q.y * 0x1.000000p-1f)));
  l_texel_2 = (0x1.000000p0f / f32((coax_p_wg_dims_depth()).x));
  l_taps = array<vec2f, 12>(coax_p_v2((-0x1.e147aep-1f), (-0x1.99999ap-2f)), coax_p_v2(0x1.e66666p-1f, (-0x1.8a3d70p-1f)), coax_p_v2((-0x1.70a3d8p-4f), (-0x1.dc28f6p-1f)), coax_p_v2(0x1.5c28f6p-2f, 0x1.28f5c2p-2f), coax_p_v2((-0x1.d70a3ep-1f), 0x1.d70a3ep-2f), coax_p_wg_mul_f2f(coax_p_v2((-0x1.a3d70ap-1f), (-0x1.f5c290p-1f)), 0x1.000000p-1f), coax_p_v2((-0x1.851eb8p-2f), 0x1.1eb852p-2f), coax_p_v2(0x1.f0a3d8p-1f, 0x1.851eb8p-1f), coax_p_v2(0x1.c28f5cp-2f, (-0x1.7ae148p-2f)), coax_p_wg_mul_f2f(coax_p_v2(0x1.147ae2p-1f, (-0x1.bd70a4p-1f)), 0x1.000000p-1f), coax_p_v2((-0x1.0a3d70p-2f), 0x1.f0a3d8p-1f), coax_p_v2(0x1.947ae2p-1f, 0x1.851eb8p-3f));
  l_lit = 0.0f;
  {
    l_k = 0i;
    while ((l_k < 12i))
    {
      l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_wg_add_f2f2(l_uv, coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_taps[i32(l_k)], l_texel_2), 0x1.19999ap1f)), l_q.z));
      l_k = (l_k + 1i);
    }
  }
  return coax_p_verse_haunt_shadow_edge(l_uv.x, l_uv.y, (l_lit / 0x1.800000p3f));
}
fn coax_p_verse_haunt_ambient(p_ny: f32, p_channel: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_channel: f32 = p_channel;
  var l_sky: f32;
  var l_ground: f32;
  l_sky = select(select(0x1.333334p-3f, 0x1.70a3d8p-4f, (l_channel < 0x1.800000p0f)), 0x1.47ae14p-4f, (l_channel < 0x1.000000p-1f));
  l_ground = select(select(0x1.47ae14p-6f, 0x1.99999ap-6f, (l_channel < 0x1.800000p0f)), 0x1.eb851ep-6f, (l_channel < 0x1.000000p-1f));
  return (l_ground + ((l_sky - l_ground) * ((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)));
}
fn coax_p_verse_haunt_moonlight(p_channel: f32) -> f32 {
  var l_channel: f32 = p_channel;
  return select(select(0x1.051eb8p-1f, 0x1.800000p-2f, (l_channel < 0x1.800000p0f)), 0x1.51eb86p-2f, (l_channel < 0x1.000000p-1f));
}
fn coax_p_wg_mul_ff3(p_a: f32, p_b: vec3f) -> vec3f {
  var l_a: f32 = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a * l_b.x), (l_a * l_b.y), (l_a * l_b.z));
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
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
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_verse_haunt_attenuate(p_d: f32, p_r: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_r: f32 = p_r;
  var l_q: f32;
  l_q = coax_p_hl_max(0.0f, (0x1.000000p0f - (l_d / coax_p_hl_max(l_r, 0x1.47ae14p-7f))));
  return (l_q * l_q);
}
fn coax_p_verse_haunt_shade(p_albedo: f32, p_light: f32, p_glow: f32, p_distance: f32, p_channel: f32, p_flash: f32) -> f32 {
  var l_albedo: f32 = p_albedo;
  var l_light: f32 = p_light;
  var l_glow: f32 = p_glow;
  var l_distance: f32 = p_distance;
  var l_channel: f32 = p_channel;
  var l_flash: f32 = p_flash;
  var l_fog: f32;
  var l_fogcolor: f32;
  var l_lit: f32;
  l_fog = (0x1.000000p0f - exp((((-l_distance) * l_distance) * 0x1.fb82c2p-14f)));
  l_fogcolor = select(select(0x1.1eb852p-4f, 0x1.47ae14p-5f, (l_channel < 0x1.800000p0f)), 0x1.1eb852p-5f, (l_channel < 0x1.000000p-1f));
  l_lit = ((l_albedo * ((l_light + (l_glow * 0x1.000000p1f)) + (l_flash * 0x1.000000p1f))) * 0x1.4cccccp0f);
  return (l_lit + ((l_fogcolor - l_lit) * l_fog));
}
fn coax_p_verse_haunt_bloom_share(p_peak: f32, p_glow: f32) -> f32 {
  var l_peak: f32 = p_peak;
  var l_glow: f32 = p_glow;
  var l_own: f32;
  var l_past: f32;
  l_own = coax_p_hl_min(0x1.ccccccp-1f, (coax_p_hl_max(0.0f, l_glow) * 0x1.99999ap-1f));
  l_past = coax_p_hl_min(0x1.000000p0f, (coax_p_hl_max(0.0f, (l_peak - 0x1.000000p0f)) / (l_peak + 0x1.000000p-1f)));
  return (l_own + (((0x1.000000p0f - l_own) * l_past) * 0x1.333334p-1f));
}
fn coax_p_verse_outline_strength(p_cut: f32, p_glow: f32) -> f32 {
  var l_cut: f32 = p_cut;
  var l_glow: f32 = p_glow;
  return (coax_p_lk_max(0.0f, (0x1.000000p0f - coax_p_lk_max(l_cut, 0.0f))) * (0x1.000000p0f - coax_p_lk_ramp(0x1.000000p-2f, 0x1.333334p-1f, l_glow)));
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
fn coax_p_wg_dims_2d() -> coax_t29 {
  return coax_p_u2(textureDimensions(bloom_src).x, textureDimensions(bloom_src).y);
}
fn coax_p_wg_cvt_f2u2(p_p: coax_t29) -> vec2f {
  var l_p: coax_t29 = p_p;
  return coax_p_v2(f32(l_p.x), f32(l_p.y));
}
fn coax_p_wg_div_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a / l_b.x), (l_a / l_b.y));
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
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
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_wg_sw_rgb_f4(p_p: vec4f) -> vec3f {
  var l_p: vec4f = p_p;
  return coax_p_v3(l_p.x, l_p.y, l_p.z);
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t27 {
  var l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
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
fn coax_p_wg_load_2d(p_c: coax_t27, p_level: i32) -> vec4f {
  var l_c: coax_t27 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_light, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_realm_outlined(p_p: coax_t27, p_light: vec3f) -> vec3f {
  var l_p: coax_t27 = p_p;
  var l_light: vec3f = p_light;
  var l_a: vec4f;
  var l_tag: u32;
  var l_strength: f32;
  var l_o: i32;
  var l_top: coax_t27;
  var l_at: array<coax_t27, 4>;
  var l_dc: f32;
  var l_d: array<f32, 4>;
  var l_edge: f32;
  var l_k: i32;
  var l_q: coax_t27;
  var l_b: vec4f;
  var l_other: u32;
  var l_lap: f32;
  l_a = coax_p_wg_load_2d_1(l_p, 0i);
  l_tag = coax_p_wg_u32_f(coax_p_wg_round(l_a.w));
  l_strength = (f32((l_tag & 15u)) / 0x1.e00000p3f);
  if ((l_strength <= 0.0f))
  {
    return l_light;
  }
  l_o = coax_p_wg_i32_f(coax_p_fmax(0x1.000000p0f, floor((coax_at_u(i32(1i)) / 0x1.900000p9f))));
  l_top = coax_p_wg_sub_i2i(coax_p_wg_cvt_i2u2(coax_p_wg_dims_depth_1()), 1i);
  l_at = array<coax_t27, 4>(coax_p_i2((-l_o), 0i), coax_p_i2(l_o, 0i), coax_p_i2(0i, (-l_o)), coax_p_i2(0i, l_o));
  l_dc = coax_p_wg_load_depth(l_p, 0i);
  l_d = array<f32, 4>();
  l_edge = 0.0f;
  {
    l_k = 0i;
    while ((l_k < 4i))
    {
      l_q = coax_p_wg_clamp_i2i2i2(coax_p_wg_add_i2i2(l_p, l_at[i32(l_k)]), coax_p_i2(0i, 0i), l_top);
      l_d[i32(l_k)] = coax_p_wg_load_depth(l_q, 0i);
      l_b = coax_p_wg_load_2d_1(l_q, 0i);
      if ((l_d[i32(l_k)] < 0x1.000000p0f))
      {
        l_edge = coax_p_fmax(l_edge, coax_p_verse_outline_crease(coax_p_wg_dot3(coax_p_v3(l_a.x, l_a.y, l_a.z), coax_p_v3(l_b.x, l_b.y, l_b.z))));
        l_other = (coax_p_wg_u32_f(coax_p_wg_round(l_b.w)) >> u32(4u));
        if ((((l_other != (l_tag >> u32(4u))) && (l_dc <= (l_d[i32(l_k)] + 0x1.4f8b58p-17f))) && ((l_tag >> u32(4u)) > l_other)))
        {
          l_edge = coax_p_fmax(l_edge, 0x1.666666p-1f);
        }
      }
      l_k = (l_k + 1i);
    }
  }
  l_lap = coax_p_fmax(abs(((l_d[0i] + l_d[1i]) - (0x1.000000p1f * l_dc))), abs(((l_d[2i] + l_d[3i]) - (0x1.000000p1f * l_dc))));
  l_edge = coax_p_fmax(l_edge, coax_p_verse_outline_depth(l_lap, l_dc));
  return coax_p_v3(coax_p_verse_outline_ink(l_light.x, (l_edge * l_strength)), coax_p_verse_outline_ink(l_light.y, (l_edge * l_strength)), coax_p_verse_outline_ink(l_light.z, (l_edge * l_strength)));
}
fn coax_p_verse_haunt_glow(p_light: f32, p_near: f32, p_far: f32) -> f32 {
  var l_light: f32 = p_light;
  var l_near: f32 = p_near;
  var l_far: f32 = p_far;
  return ((l_light + (l_near * 0x1.333334p0f)) + (l_far * 0x1.19999ap0f));
}
fn coax_p_realm_tone(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_v3(coax_p_verse_haunt_night(l_c.x), coax_p_verse_haunt_night(l_c.y), coax_p_verse_haunt_night(l_c.z));
}
fn coax_p_realm_panels(p_p: vec2f, p_under: vec3f) -> vec3f {
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
      l_c = coax_p_realm_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 32i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 32i)));
      l_k = (l_k + 1i);
    }
  }
  return l_c;
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_verse_bar(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32, p_t: f32, p_press: f32, p_flash: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_w: f32 = p_w;
  var l_h: f32 = p_h;
  var l_k: f32 = p_k;
  var l_t: f32 = p_t;
  var l_press: f32 = p_press;
  var l_flash: f32 = p_flash;
  var l_d: f32;
  var l_inside: f32;
  var l_edge: f32;
  var l_phase: f32;
  var l_sweep: f32;
  var l_glass: f32;
  var l_glow: f32;
  l_d = coax_p_cut_distance(l_x, l_y, l_w, l_h, l_k);
  l_inside = (0x1.000000p0f - coax_p_smooth((-0x1.000000p-1f), 0x1.000000p-1f, l_d));
  l_edge = (0x1.000000p0f - coax_p_smooth((0x1.000000p-1f * l_k), (0x1.99999ap0f * l_k), abs(l_d)));
  l_phase = ((l_t / 0x1.800000p2f) - floor((l_t / 0x1.800000p2f)));
  l_sweep = exp((-(((((l_x + (l_y * 0x1.333334p-1f)) / l_w) - ((-0x1.99999ap0f) + ((0x1.99999ap1f * l_phase) / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f) * ((((l_x + (l_y * 0x1.333334p-1f)) / l_w) - ((-0x1.99999ap0f) + ((0x1.99999ap1f * l_phase) / 0x1.c28f5cp-3f))) / 0x1.99999ap-4f))));
  if ((l_phase > 0x1.c28f5cp-3f))
  {
    l_sweep = 0.0f;
  }
  l_glass = ((((0x1.1eb852p-5f + (0x1.99999ap-6f * (0x1.000000p0f - (l_y / l_h)))) + (0x1.99999ap-4f * l_press)) + (0x1.99999ap-4f * l_sweep)) * l_inside);
  l_glow = ((exp(((-coax_p_hi(l_d, 0.0f)) / (0x1.200000p3f * l_k))) * (0x1.000000p0f - l_inside)) * (0x1.99999ap-4f + (0x1.333334p-2f * l_flash)));
  return ((l_glass + (l_edge * (0x1.4cccccp-1f + (0x1.000000p-1f * l_flash)))) + l_glow);
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_verse_haunt_shadow_reach(p_axis: f32) -> f32 {
  var l_axis: f32 = p_axis;
  return select(0x1.180000p6f, 0x1.a00000p4f, (l_axis < 0x1.000000p-1f));
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_u3(p_x: u32, p_y: u32, p_z: u32) -> coax_t16 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  var l_z: u32 = p_z;
  return coax_t16(l_x, l_y, l_z);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_render_sample_layer8_bias(p_uv: vec2f, p_layer: i32, p_bias: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_bias: f32 = p_bias;
  var l_rs_dx: vec2f;
  var l_rs_dy: vec2f;
  l_rs_dx = coax_p_v2(dpdxFine(l_uv.x), dpdxFine(l_uv.y));
  l_rs_dy = coax_p_v2(dpdyFine(l_uv.x), dpdyFine(l_uv.y));
  return coax_p_render_sample_layer8_grad(l_uv, l_layer, l_rs_dx, l_rs_dy, l_bias);
}
fn coax_p_verse_haunt_moon_dir(p_axis: f32) -> f32 {
  var l_axis: f32 = p_axis;
  return select(select((-0x1.3f227ep-1f), 0x1.1b1b7ap-1f, (l_axis < 0x1.800000p0f)), (-0x1.1b1b7ap-1f), (l_axis < 0x1.000000p-1f));
}
fn coax_p_wg_dims_depth() -> coax_t29 {
  return coax_p_u2(textureDimensions(shadow_map).x, textureDimensions(shadow_map).y);
}
fn coax_p_verse_haunt_shadow_edge(p_u: f32, p_v: f32, p_lit: f32) -> f32 {
  var l_u: f32 = p_u;
  var l_v: f32 = p_v;
  var l_lit: f32 = p_lit;
  var l_e: f32;
  var l_fade: f32;
  l_e = coax_p_hl_max(coax_p_hl_max(((l_u * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - (l_u * 0x1.000000p1f))), coax_p_hl_max(((l_v * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - (l_v * 0x1.000000p1f))));
  l_fade = coax_p_hl_min(0x1.000000p0f, coax_p_hl_max(0.0f, ((l_e - 0x1.ccccccp-1f) * 0x1.400000p3f)));
  return (l_lit + ((0x1.000000p0f - l_lit) * l_fade));
}
fn coax_p_hl_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_hl_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lk_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_lk_ramp(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_lk_min(0x1.000000p0f, coax_p_lk_max(0.0f, ((l_x - l_a) / (l_b - l_a))));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t29 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t29(l_x, l_y);
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
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t27 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t27(l_x, l_y);
}
fn coax_p_wg_load_2d_1(p_c: coax_t27, p_level: i32) -> vec4f {
  var l_c: coax_t27 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_aux, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_dims_depth_1() -> coax_t29 {
  return coax_p_u2(textureDimensions(post_depth).x, textureDimensions(post_depth).y);
}
fn coax_p_wg_cvt_i2u2(p_p: coax_t29) -> coax_t27 {
  var l_p: coax_t29 = p_p;
  return coax_p_i2(i32(l_p.x), i32(l_p.y));
}
fn coax_p_wg_sub_i2i(p_a: coax_t27, p_b: i32) -> coax_t27 {
  var l_a: coax_t27 = p_a;
  var l_b: i32 = p_b;
  return coax_p_i2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_wg_load_depth(p_c: coax_t27, p_level: i32) -> f32 {
  var l_c: coax_t27 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_depth, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_add_i2i2(p_a: coax_t27, p_b: coax_t27) -> coax_t27 {
  var l_a: coax_t27 = p_a;
  var l_b: coax_t27 = p_b;
  return coax_p_i2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_clamp_i2i2i2(p_p0: coax_t27, p_p1: coax_t27, p_p2: coax_t27) -> coax_t27 {
  var l_p0: coax_t27 = p_p0;
  var l_p1: coax_t27 = p_p1;
  var l_p2: coax_t27 = p_p2;
  return coax_p_i2(coax_p_wg_clamp_i(l_p0.x, l_p1.x, l_p2.x), coax_p_wg_clamp_i(l_p0.y, l_p1.y, l_p2.y));
}
fn coax_p_verse_outline_crease(p_facing: f32) -> f32 {
  var l_facing: f32 = p_facing;
  return coax_p_lk_ramp(0x1.800000p-1f, 0x1.19999ap-1f, l_facing);
}
fn coax_p_verse_outline_depth(p_lap: f32, p_depth: f32) -> f32 {
  var l_lap: f32 = p_lap;
  var l_depth: f32 = p_depth;
  var l_jump: f32;
  l_jump = (abs(l_lap) / coax_p_lk_max((0x1.000000p0f - l_depth), 0x1.0c6f7ap-20f));
  return coax_p_lk_ramp(0x1.47ae14p-5f, 0x1.eb851ep-4f, l_jump);
}
fn coax_p_verse_outline_ink(p_light: f32, p_edge: f32) -> f32 {
  var l_light: f32 = p_light;
  var l_edge: f32 = p_edge;
  return (l_light * (0x1.000000p0f - (0x1.a3d70ap-1f * coax_p_lk_min(0x1.000000p0f, coax_p_lk_max(0.0f, l_edge)))));
}
fn coax_p_verse_haunt_night(p_light: f32) -> f32 {
  var l_light: f32 = p_light;
  var l_t: f32;
  l_t = (0x1.000000p0f - exp(((-l_light) * 0x1.59999ap0f)));
  return ((l_t * 0x1.99999ap-1f) + (sqrt(coax_p_hl_max(l_t, 0.0f)) * 0x1.99999ap-3f));
}
fn coax_p_realm_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
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
fn coax_p_cut_distance(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_w: f32 = p_w;
  var l_h: f32 = p_h;
  var l_k: f32 = p_k;
  var l_dx: f32;
  var l_dy: f32;
  var l_box: f32;
  l_dx = (abs(l_x) - l_w);
  l_dy = (abs(l_y) - l_h);
  l_box = (sqrt(((coax_p_hi(l_dx, 0.0f) * coax_p_hi(l_dx, 0.0f)) + (coax_p_hi(l_dy, 0.0f) * coax_p_hi(l_dy, 0.0f)))) + coax_p_lo(coax_p_hi(l_dx, l_dy), 0.0f));
  return coax_p_hi(l_box, ((abs((l_x + l_y)) - ((l_w + l_h) - (0x1.c00000p3f * l_k))) * 0x1.6a09e6p-1f));
}
fn coax_p_smooth(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_sat(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_hi(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
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
fn coax_p_render_sample_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_result: coax_t32;
  l_rs_result = coax_p_render_filter_layer8_grad(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
}
fn coax_p_lk_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
}
fn coax_p_wg_clamp_i(p_x: i32, p_low: i32, p_high: i32) -> i32 {
  var l_x: i32 = p_x;
  var l_low: i32 = p_low;
  var l_high: i32 = p_high;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_low), l_high);
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
fn coax_p_lo(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_sat(p_a: f32) -> f32 {
  var l_a: f32 = p_a;
  return coax_p_lo(0x1.000000p0f, coax_p_hi(0.0f, l_a));
}
fn coax_p_render_filter_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t32 {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_f: coax_t33;
  var l_rs_sum: vec4f;
  var l_rs_i: i32;
  var l_rs_offset: f32;
  var l_rs_at: vec2f;
  l_rs_f = coax_p_render_footprint(i32(textureDimensions(sheets).x), i32(textureDimensions(sheets).y), l_dx.x, l_dx.y, l_dy.x, l_dy.y, l_bias);
  if ((!l_rs_f.x))
  {
    return coax_t32(false, vec4f());
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
  return coax_t32(true, coax_p_scale4(l_rs_sum, (0x1.000000p0f / f32(l_rs_f.y))));
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
fn coax_p_render_footprint(p_width: i32, p_height: i32, p_du_dx: f32, p_dv_dx: f32, p_du_dy: f32, p_dv_dy: f32, p_bias: f32) -> coax_t33 {
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
    return coax_t33(false, 0i, 0.0f, 0.0f, 0.0f);
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
  return coax_t33(true, l_taps, (l_du / f32(l_taps)), (l_dv / f32(l_taps)), ((log(l_span) / log(0x1.000000p1f)) + l_bias));
}
fn coax_p_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
}
fn coax_p_add2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return vec2f((l_a.x + l_b.x), (l_a.y + l_b.y));
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
fn coax_p_render_footprint_finite(p_x: f32) -> bool {
  var l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_footprint_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (((l_a + l_b) + abs((l_a - l_b))) * 0x1.000000p-1f);
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn shadow_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_14 {
  let coax_r = coax_p_shadow_vs(coax_vid, coax_iid);
  return coax_io_14(coax_r.x, coax_r.y, coax_r.z);
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_17) -> coax_io_18 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t17(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_18(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_19 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_19(coax_r.x, coax_r.y);
}
@fragment
fn shadow_fs(coax_in: coax_io_14) {
  coax_p_shadow_fs(coax_t14(coax_in.x, coax_in.y, coax_in.z));
}
@fragment
fn scene_fs(coax_in: coax_io_15) -> coax_out_19 {
  let coax_r = coax_p_scene_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8));
  return coax_out_19(coax_r.x, coax_r.y);
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