// ---- generated: everglade/client/gpu/everglade2.coax (the module) by coax wgsl; do not edit
struct coax_t9 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
  f5: vec4f,
}
struct coax_t13 {
  x: vec4f,
  y: vec2f,
  z: u32,
  w: vec2f,
  f4: f32,
}
struct coax_io_13 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) @interpolate(flat) z: u32,
  @location(2) w: vec2f,
  @location(3) f4: f32,
}
struct coax_t15 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: u32,
  f5: vec3f,
  f6: vec4f,
  f7: vec3f,
  f8: u32,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) @interpolate(flat) f4: u32,
  @location(4) f5: vec3f,
  @location(5) @interpolate(flat) f6: vec4f,
  @location(6) f7: vec3f,
  @location(7) @interpolate(flat) f8: u32,
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
  y: vec4f,
  z: vec4f,
  w: vec4f,
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
  z: f32,
}
struct coax_out_19 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @builtin(frag_depth) z: f32,
}
struct coax_t24 {
  x: vec3f,
  y: vec3f,
  z: vec2f,
  w: vec2f,
}
struct coax_t25 {
  x: vec4f,
  y: vec4f,
}
struct coax_out_25 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
struct coax_t28 {
  x: vec3f,
  y: vec3f,
}
struct coax_t29 {
  x: u32,
  y: u32,
}
struct coax_t30 {
  x: i32,
  y: i32,
}
struct coax_t32 {
  x: i32,
  y: f32,
  z: f32,
  w: f32,
  f4: array<f32, 3>,
}
struct coax_t35 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_t36 {
  x: bool,
  y: vec4f,
}
struct coax_t37 {
  x: f32,
  y: f32,
  z: f32,
  w: f32,
  f4: f32,
  f5: f32,
  f6: f32,
  f7: f32,
  f8: f32,
  f9: f32,
}
struct coax_t38 {
  x: f32,
  y: f32,
  z: f32,
  w: f32,
  f4: f32,
  f5: f32,
  f6: f32,
}
struct coax_t40 {
  x: bool,
  y: i32,
  z: f32,
  w: f32,
  f4: f32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 79>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> objects: array<coax_t9>;
@group(1) @binding(2) var<storage, read> bake: array<vec4f>;
@group(1) @binding(3) var<storage, read> inst: array<vec4f>;
@group(0) @binding(1) var<storage, read> frame: array<f32>;
@group(1) @binding(6) var sheets: texture_2d_array<f32>;
@group(1) @binding(7) var sheet: sampler;
@group(1) @binding(4) var<storage, read> mats: array<vec4f>;
@group(1) @binding(5) var<storage, read> ground: array<vec4f>;
@group(1) @binding(8) var data: texture_2d_array<f32>;
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
fn coax_p_bg_vs(p_id: u32, p_instance: u32) -> vec4f {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  return coax_p_fivelab_bg_vs(l_id, l_instance);
}
fn coax_p_shadow_vs(p_id: u32, p_instance: u32) -> coax_t13 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_o: coax_t13;
  var l_k: i32;
  var l_s: coax_t15;
  var l_q: vec3f;
  l_o = coax_t13();
  l_o.x = coax_p_v4(0.0f, 0.0f, (-0x1.000000p1f), 0x1.000000p0f);
  l_k = i32(coax_at_u(i32(105i)));
  if (((((coax_at_u(i32(10i)) < 0x1.800000p0f) || (coax_at_u(i32(104i)) < 0x1.000000p-1f)) || (l_k < 0i)) || (l_k > 3i)))
  {
    return l_o;
  }
  l_s = coax_p_scene_vs(l_id, l_instance);
  if (((l_s.f6.w > 0x1.700000p3f) && (l_s.f7.x < 0x1.000000p-1f)))
  {
    return l_o;
  }
  l_q = coax_p_e2_casc_place(l_s.y, l_k);
  l_o.x = coax_p_e2_casc_clip(l_q, l_k);
  l_o.w = coax_p_v2(l_q.x, l_q.y);
  l_o.y = l_s.w;
  l_o.z = l_s.f4;
  l_o.f4 = coax_p_e2_body_fade(l_s);
  return l_o;
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_mp: coax_t9;
  var l_o: coax_t15;
  var l_m0: coax_t9;
  var l_fi: f32;
  var l_m: coax_t9;
  var l_k: f32;
  var l_w: vec3f;
  if ((l_instance >= 4194304u))
  {
    return coax_p_e2_field_vs(l_id, l_instance);
  }
  if ((l_instance >= 1048576u))
  {
    return coax_p_e2_kit_vs(l_id, l_instance);
  }
  if ((l_instance < 65536u))
  {
    l_mp = objects[i32(l_instance)];
    if (((l_mp.w.w > 0x1.480000p4f) && (l_mp.w.w < 0x1.580000p4f)))
    {
      return coax_p_e2_prop_vs(l_id, l_instance, l_mp);
    }
  }
  l_o = coax_p_fivelab_scene_vs(l_id, l_instance);
  if ((l_instance < 65536u))
  {
    l_m0 = objects[i32(l_instance)];
    if (((l_m0.w.w > 0x1.180000p4f) && (l_m0.w.w < 0x1.380000p4f)))
    {
      l_o.f6.x = l_m0.f5.w;
    }
  }
  l_fi = f32(l_instance);
  if ((((abs(coax_at_u(i32(99i))) > 0x1.0624dep-11f) && (l_fi >= coax_at_u(i32(101i)))) && (l_fi < coax_at_u(i32(102i)))))
  {
    l_m = objects[i32(l_instance)];
    l_k = (0x1.000000p0f - coax_at_u(i32(99i)));
    l_w = coax_p_v3((l_m.x.x + ((l_o.y.x - l_m.x.x) * l_k)), (l_m.x.y + ((l_o.y.y - l_m.x.y) * l_k)), (l_m.x.z + ((l_o.y.z - l_m.x.z) * l_k)));
    l_o.y = l_w;
    l_o.x = coax_p_fivelab_project(l_w);
  }
  return l_o;
}
fn coax_p_fx_vs(p_id: u32, p_instance: u32, p_in_: coax_t16) -> coax_t18 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_in_: coax_t16 = p_in_;
  return coax_p_fivelab_fx_vs(l_id, l_instance, coax_t17(l_in_.x, l_in_.y, l_in_.z, l_in_.w));
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t19 {
  var l_pos: vec4f = p_pos;
  var l_rd: vec3f;
  var l_sky: vec3f;
  l_rd = coax_p_normalize3(coax_p_add3(coax_p_wg_at3(20i), coax_p_add3(coax_p_scale3(coax_p_wg_at3(24i), ((l_pos.x - coax_at_u(i32(23i))) / coax_at_u(i32(19i)))), coax_p_scale3(coax_p_wg_at3(28i), ((coax_at_u(i32(27i)) - l_pos.y) / coax_at_u(i32(19i)))))));
  l_sky = coax_p_cg_decode(coax_p_cg_sky(l_rd, coax_p_wg_at3(32i), coax_at_u(i32(2i))));
  return coax_t19(coax_p_v4(l_sky.x, l_sky.y, l_sky.z, 0.0f), vec4f(), 0x1.000000p0f);
}
fn coax_p_shadow_fs(p_s: coax_t13) {
  var l_s: coax_t13 = p_s;
  var l_t: vec4f;
  if (((abs(l_s.w.x) > 0x1.000000p0f) || (abs(l_s.w.y) > 0x1.000000p0f)))
  {
    discard;
  }
  if ((!coax_p_e2_dither_keep(coax_p_v2(l_s.x.x, l_s.x.y), l_s.f4)))
  {
    discard;
  }
  l_t = coax_p_render_sample_layer8_bias(l_s.y, i32(l_s.z), 0.0f);
  if ((l_t.w < 0x1.666666p-2f))
  {
    discard;
  }
}
fn coax_p_scene_fs(p_s: coax_t15, p_front: bool) -> coax_t25 {
  var l_s: coax_t15 = p_s;
  var l_front: bool = p_front;
  var l_deriv: coax_t24;
  var l_uv: vec2f;
  var l_ray: vec3f;
  var l_plate: vec2f;
  var l_tex: vec4f;
  var l_rd: vec3f;
  var l_sky: vec3f;
  var l_c: vec4f;
  var l_n: vec3f;
  var l_z: f32;
  var l_sky_21: bool;
  var l_foliage: bool;
  var l_indirect: f32;
  var l_packed: vec2f;
  l_deriv = coax_t24(coax_p_v3(dpdxFine(l_s.y.x), dpdxFine(l_s.y.y), dpdxFine(l_s.y.z)), coax_p_v3(dpdyFine(l_s.y.x), dpdyFine(l_s.y.y), dpdyFine(l_s.y.z)), coax_p_v2(dpdxFine(l_s.w.x), dpdxFine(l_s.w.y)), coax_p_v2(dpdyFine(l_s.w.x), dpdyFine(l_s.w.y)));
  l_uv = l_s.w;
  l_ray = coax_p_add3(coax_p_wg_at3(20i), coax_p_add3(coax_p_scale3(coax_p_wg_at3(24i), ((l_s.x.x - coax_at_u(i32(23i))) / coax_at_u(i32(19i)))), coax_p_scale3(coax_p_wg_at3(28i), ((coax_at_u(i32(27i)) - l_s.x.y) / coax_at_u(i32(19i))))));
  l_plate = coax_p_cg_sky_uv(l_ray);
  if (((l_s.f6.w > 0x1.700000p3f) && (l_s.f7.x < 0x1.000000p-1f)))
  {
    l_uv = coax_p_v2(coax_p_clampf(l_plate.x, 0x1.0624dep-10f, 0x1.ff7ceep-1f), coax_p_clampf(l_plate.y, 0x1.0624dep-10f, 0x1.ff7ceep-1f));
  }
  l_tex = coax_p_render_sample_layer8_bias(l_uv, i32(l_s.f4), 0x1.000000p0f);
  if (((l_s.f6.w > 0x1.700000p3f) && (l_s.f7.x < 0x1.000000p-1f)))
  {
    l_rd = coax_p_normalize3(l_ray);
    l_sky = coax_p_mix3(coax_p_cg_decode(coax_p_cg_sky(l_rd, coax_p_wg_at3(32i), coax_at_u(i32(2i)))), coax_p_mul3(coax_p_v3(l_tex.x, l_tex.y, l_tex.z), coax_p_cg_sky_tint(l_rd)), coax_p_cg_sky_plate_weight(l_plate));
    l_tex = coax_p_v4(l_sky.x, l_sky.y, l_sky.z, 0x1.000000p0f);
  }
  if ((l_tex.w < 0x1.666666p-2f))
  {
    discard;
  }
  if (((l_s.f6.w > 0x1.700000p3f) && (!coax_p_e2_dither_keep(coax_p_v2(l_s.x.x, l_s.x.y), l_s.f6.x))))
  {
    discard;
  }
  if (((l_s.f6.w < 0x1.700000p3f) && (!coax_p_e2_dither_keep(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_e2_body_fade(l_s)))))
  {
    discard;
  }
  l_c = vec4f();
  if ((l_s.f6.w > 0x1.700000p3f))
  {
    l_c = coax_p_e2_environment_fs(l_s, l_tex, l_front, coax_p_e2_shadow_visibility(l_s.y, l_s.z));
  }
  else
  {
    l_c = coax_p_cc_player_fs(l_s, l_tex, l_front, coax_p_e2_shadow_visibility(l_s.y, l_s.z), l_deriv);
  }
  l_n = coax_p_normalize3(l_s.z);
  l_z = dot(coax_p_sub3(l_s.y, coax_p_wg_at3(16i)), coax_p_wg_at3(20i));
  l_sky_21 = ((l_s.f6.w > 0x1.700000p3f) && (l_s.f7.x < 0x1.000000p-1f));
  l_foliage = (((l_s.f6.w > 0x1.700000p3f) && (l_s.f7.x > 0x1.000000p-1f)) && (l_s.f7.x < 0x1.400000p1f));
  l_indirect = l_c.w;
  l_packed = coax_p_cg_normal_pack(l_n);
  l_c.w = select(select(0x1.eb851ep-4f, 0x1.99999ap-2f, l_foliage), 0.0f, l_sky_21);
  return coax_t25(l_c, coax_p_v4(l_packed.x, l_packed.y, l_indirect, select(select(l_z, (-l_z), l_foliage), 0.0f, l_sky_21)));
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
        l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(l_t.x, l_t.y, l_t.z), (coax_p_verse_style_bloom_share(coax_p_fmax(l_t.x, coax_p_fmax(l_t.y, l_t.z)), 0x1.3d70a4p-1f, 0x1.333334p-2f) * l_t.w)));
        l_k = (l_k + 1i);
      }
    }
    return coax_p_v4((l_c.x * 0x1.000000p-2f), (l_c.y * 0x1.000000p-2f), (l_c.z * 0x1.000000p-2f), 0x1.000000p0f);
  }
  l_d = coax_p_v2((coax_at_bloom_step(i32(0i)) * l_texel.x), (coax_at_bloom_step(i32(1i)) * l_texel.y));
  l_c_11 = coax_p_scale3(coax_p_cg_rgb(textureSampleLevel(bloom_src, bloom_lin, l_uv, 0.0f)), 0x1.d0e560p-3f);
  l_c_11 = coax_p_add3(l_c_11, coax_p_scale3(coax_p_add3(coax_p_cg_rgb(textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x + (l_d.x * 0x1.628f5cp0f)), (l_uv.y + (l_d.y * 0x1.628f5cp0f))), 0.0f)), coax_p_cg_rgb(textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x - (l_d.x * 0x1.628f5cp0f)), (l_uv.y - (l_d.y * 0x1.628f5cp0f))), 0.0f))), 0x1.439582p-2f));
  l_c_11 = coax_p_add3(l_c_11, coax_p_scale3(coax_p_add3(coax_p_cg_rgb(textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x + (l_d.x * 0x1.9d9168p1f)), (l_uv.y + (l_d.y * 0x1.9d9168p1f))), 0.0f)), coax_p_cg_rgb(textureSampleLevel(bloom_src, bloom_lin, coax_p_v2((l_uv.x - (l_d.x * 0x1.9d9168p1f)), (l_uv.y - (l_d.y * 0x1.9d9168p1f))), 0.0f))), 0x1.1eb852p-4f));
  return coax_p_v4(l_c_11.x, l_c_11.y, l_c_11.z, 0x1.000000p0f);
}
fn coax_p_post_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_pixel: vec2f;
  var l_uv: vec2f;
  var l_raw: vec4f;
  var l_encoded: vec3f;
  var l_a: vec4f;
  var l_low: bool;
  var l_m: vec3f;
  var l_res: vec3f;
  var l_glass: f32;
  var l_fade: f32;
  var l_behind: vec3f;
  var l_panel: vec3f;
  var l_c: vec3f;
  var l_glow: vec3f;
  var l_soft: vec3f;
  var l_i: i32;
  var l_angle: f32;
  var l_at: vec2f;
  var l_result: vec3f;
  var l_glass_27: f32;
  var l_fade_28: f32;
  var l_panel_29: vec3f;
  l_pixel = coax_p_v2(l_pos.x, l_pos.y);
  l_uv = coax_p_e2_arrival_uv(coax_p_v2((l_pos.x / coax_at_u(i32(0i))), (l_pos.y / coax_at_u(i32(1i)))));
  l_raw = textureSampleLevel(post_light, post_lin, l_uv, 0.0f);
  if ((coax_at_u(i32(10i)) < 0x1.800000p0f))
  {
    l_encoded = coax_p_e2_arrival(coax_p_cg_encode(coax_p_v3(l_raw.x, l_raw.y, l_raw.z)), l_pixel);
    return coax_p_v4(l_encoded.x, l_encoded.y, l_encoded.z, 0x1.000000p0f);
  }
  l_a = coax_p_wg_load_2d(coax_p_cg_post_pixel(l_pixel, coax_p_wg_dims_2d_1()), 0i);
  if ((coax_at_u(i32(37i)) < 0x1.fae148p-1f))
  {
    l_low = (coax_at_u(i32(37i)) < 0x1.ccccccp-2f);
    l_m = coax_p_v3(l_raw.x, l_raw.y, l_raw.z);
    if ((!l_low))
    {
      l_m = coax_p_cg_fxaa(l_uv, coax_p_v2((0x1.000000p0f / coax_at_u(i32(0i))), (0x1.000000p0f / coax_at_u(i32(1i)))));
      l_m = coax_p_scale3(l_m, (0x1.000000p0f - ((0x1.000000p0f - coax_p_cg_contact_ao(l_pixel)) * coax_p_saturate(l_a.z))));
      l_m = coax_p_add3(l_m, coax_p_scale3(coax_p_cg_post_sample(l_uv), 0x1.99999ap-2f));
    }
    l_res = select(select(coax_p_cg_output(coax_p_add3(l_m, coax_p_cg_sun_veil(l_uv))), coax_p_cg_encode(coax_p_add3(l_m, coax_p_cg_sun_veil(l_uv))), (coax_at_u(i32(51i)) > 0x1.400000p1f)), coax_p_cg_encode(l_m), (abs(l_a.w) < 0x1.0624dep-10f));
    if ((coax_at_u(i32(11i)) < 0x1.000000p-1f))
    {
      l_glass = coax_p_fmax(coax_p_cg_panel_opacity(l_pixel, coax_at_u(i32(0i)), coax_at_u(i32(1i)), coax_at_u(i32(4i))), coax_p_e2_pane_cover(l_pixel));
      l_fade = (0x1.000000p0f - coax_at_u(i32(40i)));
      if (((l_glass * l_fade) > 0.0f))
      {
        l_behind = select(coax_p_cg_output(coax_p_cg_glass(l_uv)), coax_p_cg_output(l_m), l_low);
        l_panel = coax_p_mix3(l_behind, coax_p_v3(0x1.70a3d8p-4f, 0x1.eb851ep-4f, 0x1.c28f5cp-4f), l_glass);
        l_res = coax_p_mix3(l_res, l_panel, l_fade);
        l_res = coax_p_mix3(l_res, coax_p_v3(0x1.dc28f6p-1f, 0x1.b851ecp-1f, 0x1.51eb86p-1f), ((coax_p_fmax(coax_p_cg_panel_edge(l_pixel, coax_at_u(i32(0i)), coax_at_u(i32(1i)), coax_at_u(i32(4i))), coax_p_e2_pane_edge(l_pixel)) * 0x1.19999ap-1f) * l_fade));
      }
    }
    l_res = coax_p_cg_walk_veil(l_res);
    l_res = coax_p_e2_arrival(l_res, l_pixel);
    return coax_p_v4(l_res.x, l_res.y, l_res.z, 0x1.000000p0f);
  }
  l_c = coax_p_cg_fxaa(l_uv, coax_p_v2((0x1.000000p0f / coax_at_u(i32(0i))), (0x1.000000p0f / coax_at_u(i32(1i)))));
  l_c = coax_p_scale3(l_c, (0x1.000000p0f - ((0x1.000000p0f - coax_p_cg_contact_ao(l_pixel)) * coax_p_saturate(l_a.z))));
  l_glow = coax_p_add3(coax_p_scale3(coax_p_cg_post_sample(l_uv), 0x1.99999ap-3f), coax_p_scale3(coax_p_cg_post_sample_1(l_uv), 0x1.99999ap-3f));
  if ((l_a.w < 0.0f))
  {
    l_soft = coax_p_v3(0.0f, 0.0f, 0.0f);
    {
      let coax_from: i32 = 0i;
      let coax_to: i32 = 4i;
      for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
      {
        l_angle = (f32(l_i) * 0x1.921fb4p0f);
        l_at = coax_p_v2((l_uv.x + ((cos(l_angle) * 0x1.000000p1f) / coax_at_u(i32(0i)))), (l_uv.y + ((sin(l_angle) * 0x1.000000p1f) / coax_at_u(i32(1i)))));
        l_soft = coax_p_add3(l_soft, coax_p_cg_post_sample_2(l_at));
      }
    }
    l_c = coax_p_mix3(l_c, coax_p_scale3(l_soft, 0x1.000000p-2f), 0x1.19999ap-1f);
  }
  l_c = coax_p_add3(l_c, l_glow);
  l_result = select(select(coax_p_cg_output(coax_p_add3(l_c, coax_p_cg_sun_veil(l_uv))), coax_p_cg_encode(coax_p_add3(l_c, coax_p_cg_sun_veil(l_uv))), (coax_at_u(i32(51i)) > 0x1.400000p1f)), coax_p_cg_encode(l_c), (abs(l_a.w) < 0x1.0624dep-10f));
  if ((coax_at_u(i32(11i)) < 0x1.000000p-1f))
  {
    l_glass_27 = coax_p_fmax(coax_p_cg_panel_opacity(l_pixel, coax_at_u(i32(0i)), coax_at_u(i32(1i)), coax_at_u(i32(4i))), coax_p_e2_pane_cover(l_pixel));
    l_fade_28 = (0x1.000000p0f - coax_at_u(i32(40i)));
    if (((l_glass_27 * l_fade_28) > 0.0f))
    {
      l_panel_29 = coax_p_mix3(coax_p_cg_output(coax_p_cg_glass(l_uv)), coax_p_v3(0x1.70a3d8p-4f, 0x1.eb851ep-4f, 0x1.c28f5cp-4f), l_glass_27);
      l_result = coax_p_mix3(l_result, l_panel_29, l_fade_28);
      l_result = coax_p_mix3(l_result, coax_p_v3(0x1.dc28f6p-1f, 0x1.b851ecp-1f, 0x1.51eb86p-1f), ((coax_p_fmax(coax_p_cg_panel_edge(l_pixel, coax_at_u(i32(0i)), coax_at_u(i32(1i)), coax_at_u(i32(4i))), coax_p_e2_pane_edge(l_pixel)) * 0x1.19999ap-1f) * l_fade_28));
    }
  }
  l_result = coax_p_cg_walk_veil(l_result);
  l_result = coax_p_e2_arrival(l_result, l_pixel);
  return coax_p_v4(l_result.x, l_result.y, l_result.z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_s: coax_t18) -> vec4f {
  var l_s: coax_t18 = p_s;
  return coax_p_fivelab_fx_fs(l_s);
}
fn coax_p_fivelab_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_e2_casc_place(p_w: vec3f, p_k: i32) -> vec3f {
  var l_w: vec3f = p_w;
  var l_k: i32 = p_k;
  var l_p: vec4f;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  return coax_p_v3(coax_p_wg_dot4(coax_p_e2_casc(l_k, 1i), l_p), coax_p_wg_dot4(coax_p_e2_casc(l_k, 2i), l_p), coax_p_wg_dot4(coax_p_e2_casc(l_k, 3i), l_p));
}
fn coax_p_e2_casc_clip(p_q: vec3f, p_k: i32) -> vec4f {
  var l_q: vec3f = p_q;
  var l_k: i32 = p_k;
  var l_r: vec4f;
  l_r = coax_p_e2_casc(l_k, 0i);
  return coax_p_v4(((((l_q.x * l_r.z) + (0x1.000000p1f * l_r.x)) + l_r.z) - 0x1.000000p0f), ((((l_q.y * l_r.w) + 0x1.000000p0f) - (0x1.000000p1f * l_r.y)) - l_r.w), l_q.z, 0x1.000000p0f);
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_e2_body_fade(p_s: coax_t15) -> f32 {
  var l_s: coax_t15 = p_s;
  var l_w: f32;
  if ((l_s.f6.w > 0x1.700000p3f))
  {
    return 0.0f;
  }
  l_w = objects[i32(l_s.f8)].f5.w;
  return select(0.0f, (l_w - 0x1.000000p1f), ((l_w > 0x1.001062p1f) && (l_w < 0x1.800000p1f)));
}
fn coax_p_e2_field_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_o: coax_t15;
  var l_local: u32;
  var l_cell: u32;
  var l_rest: u32;
  var l_blk: u32;
  var l_row: u32;
  var l_tier: u32;
  var l_bb: u32;
  var l_bz: u32;
  var l_bx: u32;
  var l_gx: u32;
  var l_gz: u32;
  var l_t: i32;
  var l_fx: u32;
  var l_fz: u32;
  var l_x: f32;
  var l_z: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_d: f32;
  var l_whole: bool;
  var l_rel: f32;
  var l_shown: f32;
  var l_woods: f32;
  var l_edge: f32;
  var l_hgt: f32;
  var l_sv: f32;
  var l_sh: f32;
  var l_au: f32;
  var l_dark: f32;
  var l_tr: f32;
  var l_tg: f32;
  var l_tb: f32;
  var l_c: vec4f;
  l_o = coax_t15();
  l_o.x = coax_p_v4(0.0f, 0.0f, (-0x1.000000p1f), 0x1.000000p0f);
  l_local = (l_instance - 4194304u);
  l_cell = (l_local & 255u);
  l_rest = (l_local >> u32(8u));
  l_blk = (l_rest >> u32(2u));
  l_row = 128u;
  l_tier = (l_blk / (l_row * l_row));
  l_bb = (l_blk - ((l_tier * l_row) * l_row));
  l_bz = (l_bb / l_row);
  l_bx = (l_bb - (l_bz * l_row));
  l_gx = ((l_bx * 16u) + (l_cell & 15u));
  l_gz = ((l_bz * 16u) + (l_cell >> u32(4u)));
  l_t = i32(l_tier);
  l_fx = coax_p_e2_field_fine_x(l_t, l_gx, l_gz);
  l_fz = coax_p_e2_field_fine_z(l_t, l_gx, l_gz);
  l_x = ((-0x1.900000p7f) + ((f32(l_fx) + coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 1u))) * 0x1.000000p-1f));
  l_z = (0x1.400000p6f - ((f32(l_fz) + coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 2u))) * 0x1.000000p-1f));
  l_dx = (l_x - coax_at_u(i32(16i)));
  l_dz = (l_z - coax_at_u(i32(18i)));
  l_d = sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
  l_whole = ((coax_at_u(i32(35i)) >= 0x1.fff2e4p-1f) && (coax_at_u(i32(36i)) >= 0x1.fff2e4p-1f));
  if (l_whole)
  {
    if (((l_d < coax_p_e2_field_start(l_t)) || (l_d >= coax_p_e2_field_out(l_t))))
    {
      return l_o;
    }
  }
  else
  {
    if (((l_d < coax_p_e2_field_start_r(l_t, coax_at_u(i32(35i)), coax_at_u(i32(36i)))) || (l_d >= coax_p_e2_field_out_r(l_t, coax_at_u(i32(35i)), coax_at_u(i32(36i))))))
    {
      return l_o;
    }
  }
  if ((coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 6u)) >= coax_p_e2_field_cover(l_x, l_z)))
  {
    return l_o;
  }
  l_rel = select(coax_p_e2_field_rel_r(l_t, l_d, coax_at_u(i32(35i)), coax_at_u(i32(36i))), coax_p_e2_field_rel(l_t, l_d), l_whole);
  l_shown = coax_p_e2_field_shown(l_rel, coax_p_e2_field_rank(l_t, l_gx, l_gz, l_fx, l_fz));
  if ((l_shown <= 0.0f))
  {
    return l_o;
  }
  l_woods = coax_p_e2_forest(l_x, l_z);
  l_edge = coax_p_e2_smooth(0x1.eb851ep-6f, 0x1.333334p-2f, coax_p_e2_path(l_x, l_z));
  l_hgt = (((0x1.c28f5cp-3f + (0x1.0a3d70p-2f * coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 4u)))) * (0x1.000000p0f - (0x1.99999ap-2f * l_edge))) * (0x1.000000p0f - (0x1.000000p-2f * l_woods)));
  l_sv = (l_hgt / 0x1.4fcce2p0f);
  l_sh = ((0x1.19999ap-1f + (0x1.666666p-2f * coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 7u)))) * 0x1.b33334p-1f);
  l_au = coax_p_e2_gold(l_x, l_z);
  l_dark = (0x1.000000p0f - (0x1.851eb8p-2f * l_woods));
  l_tr = ((0x1.333334p-2f + (0x1.0a3d70p-2f * l_au)) * l_dark);
  l_tg = (0x1.eb851ep-2f * l_dark);
  l_tb = (0x1.c28f5cp-5f * l_dark);
  l_c = coax_p_v4(l_sv, l_sh, ((floor(((l_tr * 0x1.fe0000p7f) + 0x1.000000p-1f)) * 0x1.000000p8f) + floor(((l_tg * 0x1.fe0000p7f) + 0x1.000000p-1f))), (l_tb + 0x1.000000p1f));
  l_o = coax_p_e2_kit_place(l_id, coax_p_v4(l_x, coax_p_e2_height(l_x, l_z), l_z, (coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 3u)) * 0x1.921fb6p2f)), l_c);
  l_o.f6.x = (0x1.000000p0f - l_shown);
  return l_o;
}
fn coax_p_e2_kit_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_k: i32;
  l_k = i32((l_instance - 1048576u));
  return coax_p_e2_kit_place(l_id, inst[(l_k * 2i)], inst[((l_k * 2i) + 1i)]);
}
fn coax_p_e2_prop_vs(p_id: u32, p_instance: u32, p_m: coax_t9) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m: coax_t9 = p_m;
  var l_b: u32;
  var l_p: vec3f;
  var l_n: vec3f;
  var l_w8: u32;
  var l_t: vec4f;
  var l_cs: f32;
  var l_sn: f32;
  var l_w: vec3f;
  var l_o: coax_t15;
  l_b = (coax_p_wg_u32_f(l_m.y.w) + (l_id * 12u));
  l_p = coax_p_v3(bitcast<f32>(vertices[i32(l_b)]), bitcast<f32>(vertices[i32((l_b + 1u))]), bitcast<f32>(vertices[i32((l_b + 2u))]));
  l_n = coax_p_v3(bitcast<f32>(vertices[i32((l_b + 3u))]), bitcast<f32>(vertices[i32((l_b + 4u))]), bitcast<f32>(vertices[i32((l_b + 5u))]));
  l_w8 = vertices[i32((l_b + 8u))];
  l_t = coax_p_wg_unpack4x8unorm(l_w8);
  l_cs = l_m.z.y;
  l_sn = l_m.z.x;
  l_w = coax_p_add3(coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z), coax_p_v3(((l_p.x * l_cs) + (l_p.z * l_sn)), l_p.y, (((-l_p.x) * l_sn) + (l_p.z * l_cs))));
  l_o = coax_t15();
  l_o.x = coax_p_fivelab_project(l_w);
  l_o.y = l_w;
  l_o.z = coax_p_v3(((l_n.x * l_cs) + (l_n.z * l_sn)), l_n.y, (((-l_n.x) * l_sn) + (l_n.z * l_cs)));
  l_o.w = coax_p_v2(bitcast<f32>(vertices[i32((l_b + 6u))]), bitcast<f32>(vertices[i32((l_b + 7u))]));
  l_o.f4 = ((l_w8 >> u32(24u)) & 255u);
  l_o.f5 = coax_p_v3(l_t.x, l_t.y, l_t.z);
  l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(0x1.400000p3f, 0.0f, 0.0f);
  l_o.f8 = l_instance;
  return l_o;
}
fn coax_p_fivelab_scene_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m0: coax_t9;
  var l_m: coax_t9;
  var l_b: u32;
  var l_xy: vec2f;
  var l_zw: u32;
  var l_local: vec3f;
  var l_flags: f32;
  var l_bones: bool;
  var l_at: vec3f;
  var l_at4: vec4f;
  var l_nw4: vec4f;
  var l_nw: vec3f;
  var l_joints: u32;
  var l_weights: u32;
  var l_arm: bool;
  var l_sk: coax_t28;
  var l_sk2: coax_t28;
  var l_c: f32;
  var l_sn: f32;
  var l_w: vec3f;
  var l_o: coax_t15;
  var l_kind: u32;
  if (((coax_at_u(i32(10i)) > 0x1.800000p0f) && (l_instance >= 65536u)))
  {
    l_m0 = objects[i32(((l_instance >> u32(16u)) - 1u))];
    if ((l_m0.w.w > 0x1.380000p4f))
    {
      return coax_p_cg_meadow_vs(l_id, l_instance, l_m0);
    }
    return coax_p_cg_instanced_vs(l_id, l_instance, l_m0);
  }
  l_m = objects[i32(l_instance)];
  if ((l_m.w.w > 0x1.f00000p3f))
  {
    return coax_p_cg_environment_vs(l_id, l_instance, l_m);
  }
  if ((coax_p_wg_fmod(l_m.w.w, 0x1.000000p3f) > 0x1.c00000p1f))
  {
    return coax_p_fivelab_block_vs(l_id, l_instance, l_m);
  }
  l_b = (coax_p_wg_u32_f(l_m.y.w) + (l_id * 6u));
  l_xy = coax_p_wg_unpack2x16unorm(vertices[i32(l_b)]);
  l_zw = vertices[i32((l_b + 1u))];
  l_local = coax_p_add3(coax_p_scale3(coax_p_v3(l_xy.x, l_xy.y, (f32((l_zw & 65535u)) / 0x1.fffe00p15f)), 0x1.400000p1f), coax_p_v3((-0x1.400000p0f), (-0x1.000000p-2f), (-0x1.400000p0f)));
  l_flags = l_m.w.w;
  l_bones = (coax_p_wg_fmod(l_flags, 0x1.000000p1f) > 0x1.000000p-1f);
  l_at = l_local;
  if (l_bones)
  {
    l_at = coax_p_fivelab_gaunt(l_local);
  }
  l_at4 = coax_p_v4(l_at.x, l_at.y, l_at.z, 0x1.000000p0f);
  l_nw4 = coax_p_wg_unpack4x8snorm(vertices[i32((l_b + 2u))]);
  l_nw = coax_p_v3(l_nw4.x, l_nw4.y, l_nw4.z);
  l_joints = vertices[i32((l_b + 4u))];
  l_weights = vertices[i32((l_b + 5u))];
  l_arm = (coax_p_wg_fmod(l_flags, 0x1.000000p4f) > 0x1.e00000p2f);
  l_sk = coax_t28();
  if ((l_m.y.z > 0x1.800000p0f))
  {
    l_sk = coax_p_fivelab_skin_mix(coax_p_wg_u32_f(l_m.y.x), coax_p_wg_u32_f(l_m.y.y), l_m.y.z, l_joints, l_weights, l_at4, l_nw, l_arm);
  }
  else
  {
    l_sk = coax_p_fivelab_skin(coax_p_wg_u32_f(l_m.y.x), l_joints, l_weights, l_at4, l_nw, l_arm);
  }
  if (((l_m.y.z > 0x1.0624dep-10f) && (l_m.y.z < 0x1.800000p0f)))
  {
    l_sk2 = coax_p_fivelab_skin(coax_p_wg_u32_f(l_m.y.y), l_joints, l_weights, l_at4, l_nw, l_arm);
    l_sk.x = coax_p_mix3(l_sk.x, l_sk2.x, l_m.y.z);
    l_sk.y = coax_p_mix3(l_sk.y, l_sk2.y, l_m.y.z);
  }
  l_c = cos(l_m.x.w);
  l_sn = sin(l_m.x.w);
  l_w = coax_p_v3((((l_sk.x.x * l_c) + (l_sk.x.z * l_sn)) + l_m.x.x), (l_sk.x.y + l_m.x.y), ((((-l_sk.x.x) * l_sn) + (l_sk.x.z * l_c)) + l_m.x.z));
  l_o = coax_t15();
  l_o.x = coax_p_fivelab_project(l_w);
  l_o.y = l_w;
  l_o.z = coax_p_v3(((l_sk.y.x * l_c) + (l_sk.y.z * l_sn)), l_sk.y.y, (((-l_sk.y.x) * l_sn) + (l_sk.y.z * l_c)));
  l_o.w = coax_p_wg_unpack2x16unorm(vertices[i32((l_b + 3u))]);
  l_o.f7 = l_local;
  l_o.f8 = l_instance;
  l_kind = ((l_zw >> u32(24u)) & 255u);
  l_o.f4 = ((l_zw >> u32(16u)) & 255u);
  l_o.f5 = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  if ((l_kind == 1u))
  {
    l_o.f5 = coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z);
  }
  if ((l_kind == 2u))
  {
    l_o.f5 = coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z);
  }
  if (((l_kind == 3u) || (l_kind == 4u)))
  {
    l_o.f4 = (l_o.f4 + coax_p_wg_u32_f(l_m.z.w));
    l_o.f6 = coax_p_v4(l_m.f4.x, l_m.f4.y, l_m.f4.z, ((f32((l_kind - 2u)) * 0x1.000000p1f) + l_m.f4.w));
  }
  if (l_bones)
  {
    l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, select(0x1.000000p3f, 0x1.200000p3f, (l_kind >= 3u)));
  }
  return l_o;
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_fivelab_project(p_w: vec3f) -> vec4f {
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
  l_cz = ((0x1.c20000p9f * (l_z - 0x1.99999ap-4f)) / (0x1.c20000p9f - 0x1.99999ap-4f));
  return coax_p_v4(l_cx, l_cy, l_cz, l_z);
}
fn coax_p_fivelab_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t17) -> coax_t18 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_in_: coax_t17 = p_in_;
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
fn coax_p_wg_at3(p_at: i32) -> vec3f {
  var l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
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
fn coax_p_cg_sky(p_rd: vec3f, p_sun: vec3f, p_time: f32) -> vec3f {
  var l_rd: vec3f = p_rd;
  var l_sun: vec3f = p_sun;
  var l_time: f32 = p_time;
  return coax_p_paint_env_sky(l_rd, l_sun, l_time);
}
fn coax_p_cg_decode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_transfer_decode(l_c);
}
fn coax_p_e2_dither_keep(p_pixel: vec2f, p_fade: f32) -> bool {
  var l_pixel: vec2f = p_pixel;
  var l_fade: f32 = p_fade;
  var l_n: f32;
  l_n = coax_p_fract((0x1.a7dd04p5f * coax_p_fract(((0x1.12e286p-4f * floor(l_pixel.x)) + (0x1.7e8b20p-8f * floor(l_pixel.y))))));
  return select(select(true, (l_n < (-l_fade)), (l_fade < 0.0f)), (l_n >= l_fade), (l_fade > 0.0f));
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
fn coax_p_cg_sky_uv(p_ray: vec3f) -> vec2f {
  var l_ray: vec3f = p_ray;
  var l_fy: f32;
  var l_fz: f32;
  var l_depth: f32;
  l_fy = (0x1.99999ap-2f / sqrt(0x1.38a3d8p3f));
  l_fz = ((-0x1.8cccccp1f) / sqrt(0x1.38a3d8p3f));
  l_depth = coax_p_fmax(((l_ray.y * l_fy) + (l_ray.z * l_fz)), 0x1.0624dep-10f);
  return coax_p_v2((0x1.000000p-1f + (((l_ray.x / l_depth) * (0x1.0e0000p10f * 0x1.c7ae14p0f)) / (0x1.000000p1f * 0x1.e00000p10f))), (0x1.eb851ep-2f - ((0x1.c7ae14p-1f * ((l_ray.y * (-l_fz)) + (l_ray.z * l_fy))) / l_depth)));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_cg_sky_tint(p_rd: vec3f) -> vec3f {
  var l_rd: vec3f = p_rd;
  return coax_p_paint_env_sky_tint(l_rd);
}
fn coax_p_mul3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_cg_sky_plate_weight(p_uv: vec2f) -> f32 {
  var l_uv: vec2f = p_uv;
  return (((coax_p_smoothstep(0.0f, 0x1.47ae14p-3f, l_uv.x) * coax_p_smoothstep(0.0f, 0x1.47ae14p-3f, (0x1.000000p0f - l_uv.x))) * coax_p_smoothstep(0.0f, 0x1.47ae14p-3f, l_uv.y)) * coax_p_smoothstep(0.0f, 0x1.47ae14p-3f, (0x1.000000p0f - l_uv.y)));
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  var l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_e2_shadow_visibility(p_p: vec3f, p_n: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  if ((coax_at_u(i32(104i)) < 0x1.000000p-1f))
  {
    return 0x1.000000p0f;
  }
  l_a = coax_p_e2_casc_try(l_p, l_n, 0i);
  if ((l_a >= 0.0f))
  {
    return l_a;
  }
  l_b = coax_p_e2_casc_try(l_p, l_n, 1i);
  if ((l_b >= 0.0f))
  {
    return l_b;
  }
  if ((coax_at_u(i32(104i)) < 0x1.400000p1f))
  {
    return 0x1.000000p0f;
  }
  l_c = coax_p_e2_casc_try(l_p, l_n, 2i);
  if ((l_c >= 0.0f))
  {
    return l_c;
  }
  l_d = coax_p_e2_casc_try(l_p, l_n, 3i);
  if ((l_d >= 0.0f))
  {
    return l_d;
  }
  return 0x1.000000p0f;
}
fn coax_p_e2_environment_fs(p_s: coax_t15, p_tex: vec4f, p_front: bool, p_shadow: f32) -> vec4f {
  var l_s: coax_t15 = p_s;
  var l_tex: vec4f = p_tex;
  var l_front: bool = p_front;
  var l_shadow: f32 = p_shadow;
  var l_kind: f32;
  var l_n: vec3f;
  var l_foliage: bool;
  var l_grass: bool;
  var l_leaves: bool;
  var l_ground: bool;
  var l_sun: vec3f;
  var l_eye: vec3f;
  var l_view: vec3f;
  var l_albedo: vec3f;
  var l_trail: f32;
  var l_steep: f32;
  var l_cut: f32;
  var l_woods: f32;
  var l_au: f32;
  var l_dk: f32;
  var l_lawn: vec3f;
  var l_away: f32;
  var l_age: f32;
  var l_town: f32;
  var l_base: vec3f;
  var l_grey: f32;
  var l_moss: f32;
  var l_visibility: f32;
  var l_ndl: f32;
  var l_direct: f32;
  var l_ao: f32;
  var l_green: f32;
  var l_fill: f32;
  var l_indirect: vec3f;
  var l_colour: vec3f;
  var l_lv: f32;
  var l_lt: vec3f;
  var l_back: f32;
  var l_scatter: f32;
  var l_mottle: f32;
  var l_meadow: f32;
  var l_grain: f32;
  var l_surface: f32;
  var l_share: f32;
  if ((l_s.f7.x < 0x1.000000p-1f))
  {
    return coax_p_v4(coax_pow_bg_vs(l_tex.x, 0x1.3d70a4p-1f), coax_pow_bg_vs(l_tex.y, 0x1.800000p-1f), (l_tex.z * 0x1.ee147ap-1f), 0x1.000000p0f);
  }
  l_kind = l_s.f7.x;
  l_n = coax_p_normalize3(coax_p_add3(l_s.z, coax_p_v3(0.0f, 0x1.a36e2ep-14f, 0.0f)));
  l_foliage = ((l_kind > 0x1.000000p-1f) && (l_kind < 0x1.400000p1f));
  l_grass = ((l_kind > 0x1.800000p0f) && (l_kind < 0x1.400000p1f));
  l_leaves = ((l_kind > 0x1.000000p-1f) && (l_kind < 0x1.800000p0f));
  l_ground = ((l_kind > 0x1.200000p2f) && (l_kind < 0x1.600000p2f));
  if (((!l_front) && (!l_foliage)))
  {
    l_n = coax_p_scale3(l_n, (-0x1.000000p0f));
  }
  l_sun = coax_p_cg_sun_dir();
  l_eye = coax_p_wg_at3(16i);
  l_view = coax_p_normalize3(coax_p_sub3(l_eye, l_s.y));
  l_albedo = coax_p_mul3(coax_p_v3(l_tex.x, l_tex.y, l_tex.z), coax_p_cg_decode(l_s.f5));
  if ((l_foliage || ((l_kind > 0x1.e00000p2f) && (l_kind < 0x1.300000p3f))))
  {
    l_albedo = coax_p_cg_decode(l_s.f5);
  }
  if (l_ground)
  {
    l_albedo = coax_p_e2_ground_albedo(l_s.y.x, l_s.y.z);
  }
  if (l_ground)
  {
    l_trail = coax_p_e2_path(l_s.y.x, l_s.y.z);
    l_steep = (0x1.19999ap-1f + (0x1.ccccccp-2f * coax_p_smoothstep(0x1.99999ap-1f, 0x1.e66666p-1f, l_n.y)));
    l_cut = (((0x1.000000p0f - coax_p_cg_ramp(0x1.800000p1f, 0x1.400000p3f, abs((l_s.y.x - coax_p_e2_road_x(l_s.y.z))))) * coax_p_cg_ramp((-0x1.100000p4f), (-0x1.600000p4f), l_s.y.z)) * (0x1.000000p0f - coax_p_cg_ramp((-0x1.400000p5f), (-0x1.e00000p5f), l_s.y.z)));
    l_steep = (l_steep * (0x1.000000p0f - (0x1.4cccccp-1f * l_cut)));
    l_woods = coax_p_e2_forest(l_s.y.x, l_s.y.z);
    l_albedo = coax_p_mix3(l_albedo, coax_p_cg_decode(coax_p_v3(0x1.a3d70ap-1f, 0x1.28f5c2p-1f, 0x1.47ae14p-3f)), ((((coax_p_e2_gold(l_s.y.x, l_s.y.z) * 0x1.99999ap-1f) * l_steep) * (0x1.000000p0f - l_trail)) * (0x1.000000p0f - (0x1.800000p-1f * l_woods))));
    l_albedo = coax_p_mix3(l_albedo, coax_p_mul3(l_albedo, coax_p_v3(0x1.3d70a4p-1f, 0x1.666666p-1f, 0x1.19999ap-1f)), ((l_woods * 0x1.19999ap-1f) * (0x1.000000p0f - l_trail)));
    l_albedo = coax_p_scale3(l_albedo, (0x1.ae147ap-1f + (0x1.47ae14p-2f * coax_p_cg_noise(coax_p_v2(((l_s.y.x * 0x1.666666p-2f) + 0x1.000000p2f), (l_s.y.z * 0x1.666666p-2f))))));
    l_albedo = coax_p_mix3(l_albedo, coax_p_cg_decode(coax_p_v3(0x1.000000p-1f, 0x1.d70a3ep-2f, 0x1.eb851ep-3f)), (l_trail * 0x1.99999ap-1f));
    l_au = coax_p_e2_gold(l_s.y.x, l_s.y.z);
    l_dk = (0x1.000000p0f - (0x1.851eb8p-2f * l_woods));
    l_lawn = coax_p_mul3(coax_p_cg_decode(coax_p_scale3(coax_p_v3((0x1.e147aep-2f + (0x1.147ae2p-2f * l_au)), (0x1.51eb86p-1f - (0x1.99999ap-4f * l_au)), 0x1.eb851ep-5f), l_dk)), coax_p_mix3(coax_p_v3(0x1.c28f5cp-1f, 0x1.99999ap-1f, 0x1.47ae14p-4f), coax_p_v3(0x1.333334p-1f, 0x1.b851ecp-1f, 0x1.99999ap-4f), 0x1.333334p-1f));
    l_away = coax_p_smoothstep(0x1.800000p4f, 0x1.e00000p6f, coax_p_wg_length3(coax_p_sub3(l_s.y, coax_p_wg_at3(16i))));
    l_albedo = coax_p_mix3(l_albedo, l_lawn, ((coax_p_e2_field_cover(l_s.y.x, l_s.y.z) * l_away) * 0x1.800000p-1f));
  }
  if (l_leaves)
  {
    if ((l_s.f5.x > (l_s.f5.y * 0x1.333334p0f)))
    {
      l_age = coax_p_cg_noise(coax_p_v2(((l_s.y.x * 0x1.666666p0f) + (l_s.y.y * 0x1.666666p-1f)), (l_s.y.z * 0x1.666666p0f)));
      l_albedo = coax_p_cg_decode(coax_p_cg_leaf_palette(l_s.f5, l_age));
    }
    l_albedo = coax_p_scale3(l_albedo, (0x1.666666p-1f + (0x1.ccccccp-2f * sqrt(coax_p_saturate(coax_p_verse_style_luma(l_tex.x, l_tex.y, l_tex.z))))));
  }
  if ((((l_kind > 0x1.400000p1f) && (l_kind < 0x1.c00000p1f)) && (l_s.f5.y < 0x1.fae148p-1f)))
  {
    l_albedo = coax_p_scale3(coax_p_cg_decode(coax_p_v3(0x1.000000p0f, 0x1.ae147ap-2f, 0x1.47ae14p-2f)), (0x1.666666p-1f + (0x1.333334p-2f * sqrt(coax_p_saturate(coax_p_verse_style_luma(l_tex.x, l_tex.y, l_tex.z))))));
  }
  if (((l_kind > 0x1.a00000p2f) && (l_kind < 0x1.100000p3f)))
  {
    l_albedo = coax_p_scale3(coax_p_cg_decode(select(coax_p_v3(0x1.b851ecp-1f, 0x1.8f5c28p-1f, 0x1.333334p-1f), coax_p_v3(0x1.000000p0f, 0x1.333334p-1f, 0x1.eb851ep-4f), (l_n.y > 0x1.333334p-3f))), (0x1.70a3d8p-1f + (0x1.1eb852p-2f * sqrt(coax_p_verse_style_luma(l_tex.x, l_tex.y, l_tex.z)))));
  }
  l_town = (-0x1.000000p0f);
  if ((((l_kind > 0x1.c00000p1f) && (l_kind < 0x1.200000p2f)) && (l_town < 0.0f)))
  {
    l_base = coax_p_cg_decode(l_s.f5);
    l_grey = coax_p_verse_style_luma(l_base.x, l_base.y, l_base.z);
    l_base = coax_p_mix3(l_base, coax_p_v3(l_grey, (l_grey * 0x1.051eb8p0f), (l_grey * 0x1.eb851ep-1f)), 0x1.666666p-2f);
    l_moss = (coax_p_smoothstep(0x1.333334p-2f, 0x1.b33334p-1f, (l_n.y + (0x1.000000p-2f * (coax_p_cg_noise(coax_p_v2(((l_s.y.x * 0x1.ccccccp-1f) + l_s.y.y), (l_s.y.z * 0x1.ccccccp-1f))) - 0x1.000000p-1f)))) * (0x1.19999ap-1f + (0x1.ccccccp-2f * coax_p_cg_noise(coax_p_v2((l_s.y.x * 0x1.b33334p0f), (l_s.y.z * 0x1.b33334p0f))))));
    l_base = coax_p_mix3(l_base, coax_p_scale3(coax_p_cg_decode(coax_p_v3(0x1.0a3d70p-1f, 0x1.47ae14p-1f, 0x1.0a3d70p-2f)), coax_p_fmax((l_grey / 0x1.c28f5cp-3f), 0x1.666666p-1f)), (l_moss * 0x1.b33334p-1f));
    l_albedo = coax_p_scale3(l_base, (0x1.333334p-1f + (0x1.000000p-1f * sqrt(coax_p_verse_style_luma(l_tex.x, l_tex.y, l_tex.z)))));
  }
  l_visibility = (l_shadow * coax_p_cg_cloud(l_s.y.x, l_s.y.z));
  l_ndl = dot(l_n, l_sun);
  l_direct = coax_p_saturate(l_ndl);
  l_ao = 0x1.000000p0f;
  if (l_leaves)
  {
    l_direct = coax_p_saturate(((l_ndl + 0x1.4cccccp-1f) / 0x1.a66666p0f));
    l_ao = coax_p_clampf(l_s.f7.y, 0x1.000000p-1f, 0x1.000000p0f);
  }
  if (l_grass)
  {
    l_green = coax_p_saturate(((l_s.f5.y - l_s.f5.x) * 0x1.000000p2f));
    l_albedo = coax_p_mul3(l_albedo, coax_p_mix3(coax_p_v3(0x1.c28f5cp-1f, 0x1.99999ap-1f, 0x1.47ae14p-4f), coax_p_v3(0x1.333334p-1f, 0x1.b851ecp-1f, 0x1.99999ap-4f), l_green));
    l_direct = ((0x1.c28f5cp-3f + (0x1.1eb852p-1f * l_s.f7.y)) * (0x1.b33334p-1f + (0x1.333334p-3f * abs(l_ndl))));
    l_ao = (0x1.70a3d8p-2f + (0x1.47ae14p-1f * l_s.f7.y));
  }
  if (((l_kind > 0x1.400000p1f) && (l_kind < 0x1.c00000p1f)))
  {
    l_direct = (0x1.666666p-2f + (0x1.ccccccp-2f * abs(l_ndl)));
  }
  if (l_ground)
  {
    l_ao = (0x1.000000p0f + ((coax_p_e2_ground_ao(l_s.y.x, l_s.y.z) - 0x1.000000p0f) * ground[1i].w));
  }
  l_fill = select(select(select(0x1.000000p0f, 0x1.8f5c28p-1f, (((l_kind > 0x1.c00000p1f) && (l_kind < 0x1.200000p2f)) && (l_town < 0.0f))), 0x1.800000p0f, ((l_kind > 0x1.600000p2f) && (l_kind < 0x1.a00000p2f))), select(0x1.266666p0f, 0x1.0cccccp1f, (l_s.f5.x > (l_s.f5.y * 0x1.333334p0f))), l_leaves);
  l_indirect = coax_p_mul3(l_albedo, coax_p_scale3(coax_p_cg_hemisphere(l_n), (l_ao * l_fill)));
  if ((l_leaves && (l_s.f5.x > (l_s.f5.y * 0x1.333334p0f))))
  {
    l_indirect = coax_p_mul3(l_indirect, coax_p_v3(0x1.1eb852p0f, 0x1.ccccccp-1f, 0x1.800000p-1f));
  }
  l_colour = coax_p_add3(l_indirect, coax_p_mul3(l_albedo, coax_p_cg_irradiance(l_n, l_direct, 0.0f, l_visibility)));
  l_lv = coax_at_u(i32(51i));
  if (((l_lv > 0x1.000000p-1f) && (l_lv < 0x1.400000p1f)))
  {
    l_lt = select(coax_p_scale3(coax_p_cg_sun_colour(), (l_direct * l_visibility)), coax_p_scale3(coax_p_cg_hemisphere(l_n), (l_ao * l_fill)), (l_lv < 0x1.800000p0f));
    return coax_p_v4(l_lt.x, l_lt.y, l_lt.z, 0.0f);
  }
  if (l_foliage)
  {
    l_back = (coax_p_saturate(dot(coax_p_scale3(l_sun, (-0x1.000000p0f)), l_view)) * coax_p_saturate(dot(coax_p_scale3(l_sun, (-0x1.000000p0f)), l_view)));
    l_scatter = (((0x1.333334p-3f + (0x1.333334p-1f * l_back)) * (0x1.000000p0f - (abs(l_ndl) * 0x1.4cccccp-1f))) * l_visibility);
    l_colour = coax_p_add3(l_colour, coax_p_mul3(l_albedo, coax_p_scale3(coax_p_v3(0x1.59999ap0f, 0x1.28f5c2p0f, 0x1.3d70a4p-1f), (l_scatter * select(0x1.000000p0f, l_s.f7.y, l_grass)))));
  }
  if (l_ground)
  {
    l_mottle = coax_p_cg_noise(coax_p_v2((l_s.y.x * 0x1.000000p1f), (l_s.y.z * 0x1.000000p1f)));
    l_meadow = (0x1.000000p0f - coax_p_e2_path(l_s.y.x, l_s.y.z));
    l_grain = coax_p_cg_noise(coax_p_v2((l_s.y.x * 0x1.c00000p3f), (l_s.y.z * 0x1.c00000p3f)));
    l_surface = (((0x1.ccccccp-1f + (0x1.47ae14p-3f * l_mottle)) * (0x1.000000p0f + ((l_meadow * (l_grain - 0x1.000000p-1f)) * 0x1.70a3d8p-3f))) * coax_p_mix(coax_p_cg_soil(coax_p_v2(l_s.y.x, l_s.y.z)), 0x1.000000p0f, l_meadow));
    l_colour = coax_p_scale3(l_colour, l_surface);
    l_indirect = coax_p_scale3(l_indirect, l_surface);
  }
  l_colour = coax_p_cg_atmosphere(l_colour, l_eye, l_s.y);
  l_share = coax_p_saturate(((coax_p_verse_style_luma(l_indirect.x, l_indirect.y, l_indirect.z) * exp((-coax_p_cg_air_depth(l_eye, l_s.y)))) / coax_p_fmax(coax_p_verse_style_luma(l_colour.x, l_colour.y, l_colour.z), 0x1.4f8b58p-17f)));
  return coax_p_v4(l_colour.x, l_colour.y, l_colour.z, l_share);
}
fn coax_p_cc_player_fs(p_s: coax_t15, p_tex: vec4f, p_front: bool, p_visibility: f32, p_d: coax_t24) -> vec4f {
  var l_s: coax_t15 = p_s;
  var l_tex: vec4f = p_tex;
  var l_front: bool = p_front;
  var l_visibility: f32 = p_visibility;
  var l_d: coax_t24 = p_d;
  var l_m: coax_t9;
  var l_at: i32;
  var l_ma: vec4f;
  var l_mb: vec4f;
  var l_mc: vec4f;
  var l_kind: i32;
  var l_albedo: vec3f;
  var l_iris: f32;
  var l_shade: f32;
  var l_g: vec3f;
  var l_maps: bool;
  var l_rough: f32;
  var l_metal: f32;
  var l_ao: f32;
  var l_n: vec3f;
  var l_flags: i32;
  var l_nt: vec4f;
  var l_o: vec4f;
  var l_eye: vec3f;
  var l_view: vec3f;
  var l_sun: vec3f;
  var l_sunc: vec3f;
  var l_r: f32;
  var l_f0: vec3f;
  var l_diffuse: vec3f;
  var l_direct: f32;
  var l_fill: f32;
  var l_indirect: vec3f;
  var l_film: bool;
  var l_key: vec3f;
  var l_colour: vec3f;
  var l_lv: f32;
  var l_lt: vec3f;
  var l_around: vec3f;
  var l_t: vec3f;
  var l_share: f32;
  l_m = objects[i32(l_s.f8)];
  l_at = (i32(l_s.f4) * 3i);
  l_ma = mats[l_at];
  l_mb = mats[(l_at + 1i)];
  l_mc = mats[(l_at + 2i)];
  l_kind = i32((l_ma.w + 0x1.000000p-1f));
  l_albedo = coax_p_cc_albedo(coax_p_v3(l_tex.x, l_tex.y, l_tex.z), l_s.f6, l_s.f5, coax_p_v3(l_ma.x, l_ma.y, l_ma.z), (l_mc.z > 0x1.000000p-1f));
  if (((l_m.f5.w > 0x1.800000p0f) && (l_s.f4 == 32u)))
  {
    l_iris = coax_p_saturate(((l_tex.x - coax_p_fmax(l_tex.y, l_tex.z)) * 0x1.400000p2f));
    l_shade = coax_p_clampf((coax_p_verse_style_luma(l_tex.x, l_tex.y, l_tex.z) * 0x1.800000p1f), 0x1.333334p-3f, 0x1.4cccccp0f);
    l_albedo = coax_p_mix3(l_albedo, coax_p_scale3(coax_p_cg_decode(coax_p_v3(l_m.f5.x, l_m.f5.y, l_m.f5.z)), l_shade), l_iris);
  }
  l_g = coax_p_normalize3(coax_p_add3(l_s.z, coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  if ((!l_front))
  {
    l_g = coax_p_scale3(l_g, (-0x1.000000p0f));
  }
  l_maps = (coax_at_u(i32(61i)) < 0x1.000000p-1f);
  l_rough = l_mb.x;
  l_metal = l_mb.y;
  l_ao = coax_p_clampf(coax_at_u(i32(62i)), 0.0f, 0x1.000000p0f);
  l_n = l_g;
  l_flags = i32((l_mc.w + 0x1.000000p-1f));
  if ((l_maps && (l_mb.z > (-0x1.000000p-1f))))
  {
    l_nt = coax_p_render_sample_layer8_grad_1(l_s.w, i32((l_mb.z + 0x1.000000p-1f)), l_d.z, l_d.w, 0.0f);
    l_n = coax_p_cm_normal(l_nt, l_mc.x, l_g, l_d.x, l_d.y, l_d.z, l_d.w);
  }
  if ((l_maps && (l_mb.w > (-0x1.000000p-1f))))
  {
    l_o = coax_p_render_sample_layer8_grad_1(l_s.w, i32((l_mb.w + 0x1.000000p-1f)), l_d.z, l_d.w, 0.0f);
    l_rough = l_o.y;
    if (((l_flags & 2i) != 0i))
    {
      l_metal = l_o.z;
    }
    if (((l_flags & 1i) != 0i))
    {
      l_ao = (l_ao * l_o.x);
    }
  }
  l_eye = coax_p_wg_at3(16i);
  l_view = coax_p_normalize3(coax_p_sub3(l_eye, l_s.y));
  l_sun = coax_p_cg_sun_dir();
  l_sunc = coax_p_cg_sun_colour();
  l_r = coax_p_cm_rough(l_rough, l_metal);
  l_f0 = coax_p_cm_f0(l_albedo, l_metal);
  l_diffuse = coax_p_scale3(l_albedo, (0x1.000000p0f - l_metal));
  l_direct = (coax_p_saturate(dot(l_n, l_sun)) * l_visibility);
  l_fill = coax_p_saturate(dot(l_n, coax_p_normalize3(coax_p_add3(l_view, coax_p_v3(0.0f, 0x1.666666p-2f, 0.0f)))));
  l_indirect = coax_p_mul3(l_diffuse, coax_p_scale3(coax_p_add3(coax_p_cg_hemisphere(l_n), coax_p_scale3(coax_p_v3(0x1.b33334p-1f, 0x1.70a3d8p-1f, 0x1.0a3d70p-1f), l_fill)), l_ao));
  l_film = (coax_at_u(i32(60i)) > 0x1.000000p-1f);
  l_key = coax_p_scale3(coax_p_mul3(l_diffuse, l_sunc), l_direct);
  if ((l_film && (l_kind == 0i)))
  {
    l_key = coax_p_scale3(coax_p_mul3(coax_p_cm_skin(l_diffuse, l_f0, l_r, l_n, l_g, l_sun, l_view), l_sunc), l_visibility);
  }
  else
  {
    l_key = coax_p_add3(l_key, coax_p_scale3(coax_p_mul3(coax_p_cm_ggx(l_f0, l_r, l_n, l_sun, l_view), l_sunc), l_visibility));
  }
  l_colour = coax_p_add3(l_key, l_indirect);
  l_lv = coax_at_u(i32(51i));
  if (((l_lv > 0x1.000000p-1f) && (l_lv < 0x1.400000p1f)))
  {
    l_lt = select(coax_p_scale3(l_sunc, l_direct), coax_p_scale3(coax_p_add3(coax_p_cg_hemisphere(l_n), coax_p_scale3(coax_p_v3(0x1.b33334p-1f, 0x1.70a3d8p-1f, 0x1.0a3d70p-1f), l_fill)), l_ao), (l_lv < 0x1.800000p0f));
    return coax_p_v4(l_lt.x, l_lt.y, l_lt.z, 0.0f);
  }
  if ((l_metal > 0.0f))
  {
    l_around = coax_p_add3(coax_p_scale3(coax_p_add3(coax_p_cg_hemisphere(l_n), coax_p_scale3(coax_p_v3(0x1.b33334p-1f, 0x1.70a3d8p-1f, 0x1.0a3d70p-1f), l_fill)), l_ao), coax_p_scale3(l_sunc, l_direct));
    l_colour = coax_p_add3(l_colour, coax_p_cm_metal_sky(coax_p_cm_metal_env(coax_p_cg_hemisphere(coax_p_reflect3(coax_p_scale3(l_view, (-0x1.000000p0f)), l_n)), l_around, l_r), l_f0, l_r, coax_p_saturate(dot(l_n, l_view)), l_metal));
  }
  if ((l_film && (l_kind == 1i)))
  {
    l_colour = coax_p_add3(l_colour, coax_p_scale3(coax_p_mul3(coax_p_cm_sheen(coax_p_scale3(l_albedo, l_mc.y), l_r, l_n, l_sun, l_view), l_sunc), l_visibility));
  }
  if ((l_film && (l_kind == 2i)))
  {
    l_t = coax_p_cm_strand(l_n, l_d.x, l_d.y, l_d.z, l_d.w);
    l_colour = coax_p_add3(l_colour, coax_p_scale3(coax_p_mul3(coax_p_cm_hair(l_albedo, l_r, l_t, l_n, l_sun, l_view), l_sunc), l_visibility));
  }
  l_share = coax_p_saturate((coax_p_verse_style_luma(l_indirect.x, l_indirect.y, l_indirect.z) / coax_p_fmax(coax_p_verse_style_luma(l_colour.x, l_colour.y, l_colour.z), 0x1.4f8b58p-17f)));
  l_colour = coax_p_cg_atmosphere(l_colour, l_eye, l_s.y);
  return coax_p_v4(l_colour.x, l_colour.y, l_colour.z, l_share);
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_cg_normal_pack(p_n: vec3f) -> vec2f {
  var l_n: vec3f = p_n;
  return coax_p_shade_normal_pack(l_n);
}
fn coax_p_wg_dims_2d() -> coax_t29 {
  return coax_p_u2(textureDimensions(bloom_src).x, textureDimensions(bloom_src).y);
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
fn coax_p_cg_rgb(p_c: vec4f) -> vec3f {
  var l_c: vec4f = p_c;
  return coax_p_v3(l_c.x, l_c.y, l_c.z);
}
fn coax_p_e2_arrival_uv(p_uv: vec2f) -> vec2f {
  var l_uv: vec2f = p_uv;
  var l_x: f32;
  var l_y: f32;
  var l_half: f32;
  var l_l: f32;
  var l_r: f32;
  var l_w: f32;
  if ((coax_at_u(i32(98i)) <= 0x1.0624dep-11f))
  {
    return l_uv;
  }
  l_x = ((l_uv.x - 0x1.000000p-1f) * coax_at_u(i32(0i)));
  l_y = ((l_uv.y - 0x1.000000p-1f) * coax_at_u(i32(1i)));
  l_half = (0x1.000000p-1f * sqrt(((coax_at_u(i32(0i)) * coax_at_u(i32(0i))) + (coax_at_u(i32(1i)) * coax_at_u(i32(1i))))));
  l_l = coax_p_fmax(sqrt(((l_x * l_x) + (l_y * l_y))), 0x1.000000p0f);
  l_r = (l_l / l_half);
  l_w = ((((sin(((l_r * 0x1.a00000p4f) - (coax_at_u(i32(97i)) * 0x1.c00000p2f))) * coax_at_u(i32(98i))) * 0x1.6872b0p-7f) * l_half) * (0x1.000000p0f - (0x1.000000p-1f * coax_p_saturate(l_r))));
  return coax_p_v2((l_uv.x + (((l_x / l_l) * l_w) / coax_at_u(i32(0i)))), (l_uv.y + (((l_y / l_l) * l_w) / coax_at_u(i32(1i)))));
}
fn coax_p_cg_encode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_transfer_encode(l_c);
}
fn coax_p_e2_arrival(p_c: vec3f, p_pixel: vec2f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_pixel: vec2f = p_pixel;
  var l_half: f32;
  var l_x: f32;
  var l_y: f32;
  var l_big: f32;
  var l_p: vec2f;
  var l_light: vec3f;
  var l_rn: f32;
  var l_edge: f32;
  var l_hold: f32;
  var l_rim: f32;
  var l_o: vec3f;
  if ((coax_at_u(i32(96i)) <= 0x1.0624dep-11f))
  {
    return l_c;
  }
  l_half = (0x1.000000p-1f * sqrt(((coax_at_u(i32(0i)) * coax_at_u(i32(0i))) + (coax_at_u(i32(1i)) * coax_at_u(i32(1i))))));
  l_x = (l_pixel.x - (0x1.000000p-1f * coax_at_u(i32(0i))));
  l_y = (l_pixel.y - (0x1.000000p-1f * coax_at_u(i32(1i))));
  l_big = (0x1.800000p1f * l_half);
  l_p = coax_p_v2((l_x / l_big), (l_y / l_big));
  l_light = coax_p_portal_field(l_p, coax_at_u(i32(97i)), coax_at_u(i32(98i)));
  l_rn = (sqrt(((l_x * l_x) + (l_y * l_y))) / l_half);
  l_edge = ((((coax_at_u(i32(100i)) * 0x1.800000p0f) - 0x1.99999ap-4f) - l_rn) + ((coax_p_portal_noise(coax_p_v2(((l_p.x * 0x1.e00000p4f) + (coax_at_u(i32(97i)) * 0x1.333334p-1f)), ((l_p.y * 0x1.e00000p4f) - (coax_at_u(i32(97i)) * 0x1.99999ap-2f)))) - 0x1.000000p-1f) * 0x1.47ae14p-3f));
  l_hold = coax_p_saturate((coax_at_u(i32(96i)) * (0x1.000000p0f - coax_p_smoothstep((-0x1.47ae14p-6f), 0x1.c28f5cp-3f, l_edge))));
  l_rim = (((exp(((-(l_edge * l_edge)) / 0x1.0624dep-8f)) * coax_at_u(i32(96i))) * (0x1.000000p0f - (coax_at_u(i32(96i)) * 0x1.000000p-1f))) * 0x1.19999ap-1f);
  l_o = coax_p_mix3(l_c, l_light, l_hold);
  return coax_p_v3((l_o.x + (0x1.000000p-1f * l_rim)), (l_o.y + (0x1.d70a3ep-1f * l_rim)), (l_o.z + (0x1.ae147ap-1f * l_rim)));
}
fn coax_p_wg_dims_2d_1() -> coax_t29 {
  return coax_p_u2(textureDimensions(post_aux).x, textureDimensions(post_aux).y);
}
fn coax_p_cg_post_pixel(p_p: vec2f, p_size: coax_t29) -> coax_t30 {
  var l_p: vec2f = p_p;
  var l_size: coax_t29 = p_size;
  return coax_p_i2(coax_p_wg_clamp_i(i32(l_p.x), 0i, (i32(l_size.x) - 1i)), coax_p_wg_clamp_i(i32(l_p.y), 0i, (i32(l_size.y) - 1i)));
}
fn coax_p_wg_load_2d(p_c: coax_t30, p_level: i32) -> vec4f {
  var l_c: coax_t30 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_aux, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_cg_fxaa(p_uv: vec2f, p_step: vec2f) -> vec3f {
  var l_uv: vec2f = p_uv;
  var l_step: vec2f = p_step;
  var l_center: vec3f;
  var l_nw: f32;
  var l_ne: f32;
  var l_sw: f32;
  var l_se: f32;
  var l_mid: f32;
  var l_low: f32;
  var l_high: f32;
  var l_dir: vec2f;
  var l_reduce: f32;
  var l_rcp: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_lum: f32;
  l_center = coax_p_cg_post_sample_2(l_uv);
  l_nw = coax_p_cg_luma(coax_p_cg_post_sample_2(coax_p_v2((l_uv.x - l_step.x), (l_uv.y - l_step.y))));
  l_ne = coax_p_cg_luma(coax_p_cg_post_sample_2(coax_p_v2((l_uv.x + l_step.x), (l_uv.y - l_step.y))));
  l_sw = coax_p_cg_luma(coax_p_cg_post_sample_2(coax_p_v2((l_uv.x - l_step.x), (l_uv.y + l_step.y))));
  l_se = coax_p_cg_luma(coax_p_cg_post_sample_2(coax_p_v2((l_uv.x + l_step.x), (l_uv.y + l_step.y))));
  l_mid = coax_p_cg_luma(l_center);
  l_low = coax_p_fmin(l_mid, coax_p_fmin(coax_p_fmin(l_nw, l_ne), coax_p_fmin(l_sw, l_se)));
  l_high = coax_p_fmax(l_mid, coax_p_fmax(coax_p_fmax(l_nw, l_ne), coax_p_fmax(l_sw, l_se)));
  if (((l_high - l_low) < coax_p_fmax(0x1.ff2e48p-6f, (l_high * 0x1.000000p-3f))))
  {
    return l_center;
  }
  l_dir = coax_p_v2((-(((l_nw + l_ne) - l_sw) - l_se)), (((l_nw + l_sw) - l_ne) - l_se));
  l_reduce = coax_p_fmax(((((l_nw + l_ne) + l_sw) + l_se) * 0x1.000000p-5f), 0x1.000000p-7f);
  l_rcp = (0x1.000000p0f / (coax_p_fmin(abs(l_dir.x), abs(l_dir.y)) + l_reduce));
  l_dir = coax_p_v2((coax_p_clampf((l_dir.x * l_rcp), (-0x1.000000p3f), 0x1.000000p3f) * l_step.x), (coax_p_clampf((l_dir.y * l_rcp), (-0x1.000000p3f), 0x1.000000p3f) * l_step.y));
  l_a = coax_p_scale3(coax_p_add3(coax_p_cg_post_sample_2(coax_p_v2((l_uv.x - (l_dir.x / 0x1.800000p2f)), (l_uv.y - (l_dir.y / 0x1.800000p2f)))), coax_p_cg_post_sample_2(coax_p_v2((l_uv.x + (l_dir.x / 0x1.800000p2f)), (l_uv.y + (l_dir.y / 0x1.800000p2f))))), 0x1.000000p-1f);
  l_b = coax_p_add3(coax_p_scale3(l_a, 0x1.000000p-1f), coax_p_scale3(coax_p_add3(coax_p_cg_post_sample_2(coax_p_v2((l_uv.x - (l_dir.x * 0x1.000000p-1f)), (l_uv.y - (l_dir.y * 0x1.000000p-1f)))), coax_p_cg_post_sample_2(coax_p_v2((l_uv.x + (l_dir.x * 0x1.000000p-1f)), (l_uv.y + (l_dir.y * 0x1.000000p-1f))))), 0x1.000000p-2f));
  l_lum = coax_p_cg_luma(l_b);
  return select(l_b, l_a, ((l_lum < l_low) || (l_lum > l_high)));
}
fn coax_p_cg_contact_ao(p_pixel: vec2f) -> f32 {
  var l_pixel: vec2f = p_pixel;
  var l_size: coax_t29;
  var l_a: vec4f;
  var l_n: vec3f;
  var l_center: vec3f;
  var l_sum: f32;
  var l_radius: f32;
  var l_i: i32;
  var l_angle: f32;
  var l_r: f32;
  var l_at: vec2f;
  var l_b: vec4f;
  l_size = coax_p_wg_dims_2d_1();
  l_a = coax_p_wg_load_2d(coax_p_cg_post_pixel(l_pixel, l_size), 0i);
  if ((l_a.w <= 0.0f))
  {
    return 0x1.000000p0f;
  }
  l_n = coax_p_cg_normal_unpack(coax_p_v2(l_a.x, l_a.y));
  l_center = coax_p_cg_viewpoint(l_pixel, l_a.w);
  l_sum = 0.0f;
  l_radius = coax_p_shade_contact_radius(coax_at_u(i32(1i)));
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 8i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      l_angle = (f32(l_i) * 0x1.921fb0p-1f);
      l_r = (l_radius * select(0x1.000000p0f, 0x1.000000p-1f, ((l_i % 2i) == 0i)));
      l_at = coax_p_v2((l_pixel.x + (cos(l_angle) * l_r)), (l_pixel.y + (sin(l_angle) * l_r)));
      l_b = coax_p_wg_load_2d(coax_p_cg_post_pixel(l_at, l_size), 0i);
      if ((abs(l_b.w) > 0x1.0624dep-10f))
      {
        l_sum = (l_sum + coax_p_shade_contact_tap(l_n, l_center, coax_p_cg_viewpoint(l_at, abs(l_b.w))));
      }
    }
  }
  return coax_p_shade_contact_finish(l_sum);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_cg_post_sample(p_uv: vec2f) -> vec3f {
  var l_uv: vec2f = p_uv;
  return coax_p_cg_rgb(textureSampleLevel(post_quarter, post_lin, l_uv, 0.0f));
}
fn coax_p_cg_sun_veil(p_uv: vec2f) -> vec3f {
  var l_uv: vec2f = p_uv;
  var l_side: f32;
  var l_k: f32;
  l_side = (l_uv.x - 0x1.3d70a4p-1f);
  l_k = (coax_p_smoothstep(0x1.8f5c28p-1f, 0x1.47ae14p-4f, l_uv.y) * (0x1.99999ap-3f + (0x1.99999ap-1f * exp((((-l_side) * l_side) / 0x1.47ae14p-4f)))));
  return coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.a3d70ap-1f, 0x1.0a3d70p-1f), (0x1.333334p-4f * l_k));
}
fn coax_p_cg_output(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_paint_env_output(l_c);
}
fn coax_p_cg_panel_opacity(p_p: vec2f, p_w: f32, p_h: f32, p_k: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_w: f32 = p_w;
  var l_h: f32 = p_h;
  var l_k: f32 = p_k;
  return select(0.0f, 0x1.28f5c2p-1f, coax_p_cg_ui_cover(l_p, l_w, l_h, l_k));
}
fn coax_p_e2_pane_cover(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return select(0.0f, 0x1.28f5c2p-1f, (coax_p_e2_in_box(l_p, coax_at_u(i32(52i)), coax_at_u(i32(53i)), coax_at_u(i32(54i)), coax_at_u(i32(55i))) || coax_p_e2_in_box(l_p, coax_at_u(i32(56i)), coax_at_u(i32(57i)), coax_at_u(i32(58i)), coax_at_u(i32(59i)))));
}
fn coax_p_cg_glass(p_uv: vec2f) -> vec3f {
  var l_uv: vec2f = p_uv;
  var l_sum: vec3f;
  var l_r: f32;
  var l_i: i32;
  var l_a: f32;
  var l_d: f32;
  l_sum = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_r = (0x1.c00000p3f * coax_at_u(i32(4i)));
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 16i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      l_a = (f32(l_i) * 0x1.3331fcp1f);
      l_d = (sqrt(((f32(l_i) + 0x1.000000p-1f) / 0x1.000000p4f)) * l_r);
      l_sum = coax_p_add3(l_sum, coax_p_cg_post_sample_2(coax_p_v2((l_uv.x + ((cos(l_a) * l_d) / coax_at_u(i32(0i)))), (l_uv.y + ((sin(l_a) * l_d) / coax_at_u(i32(1i)))))));
    }
  }
  return coax_p_scale3(l_sum, (0x1.000000p0f / 0x1.000000p4f));
}
fn coax_p_cg_panel_edge(p_p: vec2f, p_w: f32, p_h: f32, p_k: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_w: f32 = p_w;
  var l_h: f32 = p_h;
  var l_k: f32 = p_k;
  var l_d: f32;
  if ((((l_w < (0x1.a40000p9f * l_k)) || (l_h < (0x1.7c0000p9f * l_k))) || (!coax_p_cg_ui_cover(l_p, l_w, l_h, l_k))))
  {
    return 0.0f;
  }
  l_d = coax_p_fmin(coax_p_fmin((l_p.x - (0x1.000000p4f * l_k)), ((0x1.5e0000p8f * l_k) - l_p.x)), coax_p_fmin((l_p.y - (0x1.400000p4f * l_k)), ((0x1.6d0000p9f * l_k) - l_p.y)));
  return (0x1.000000p0f - coax_p_smoothstep(0.0f, (0x1.400000p0f * l_k), l_d));
}
fn coax_p_e2_pane_edge(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fmax(coax_p_e2_box_edge(l_p, coax_at_u(i32(52i)), coax_at_u(i32(53i)), coax_at_u(i32(54i)), coax_at_u(i32(55i)), coax_at_u(i32(4i))), coax_p_e2_box_edge(l_p, coax_at_u(i32(56i)), coax_at_u(i32(57i)), coax_at_u(i32(58i)), coax_at_u(i32(59i)), coax_at_u(i32(4i))));
}
fn coax_p_cg_walk_veil(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_scale3(l_c, (0x1.000000p0f - coax_p_saturate(coax_at_u(i32(41i)))));
}
fn coax_p_cg_post_sample_1(p_uv: vec2f) -> vec3f {
  var l_uv: vec2f = p_uv;
  return coax_p_cg_rgb(textureSampleLevel(post_eighth, post_lin, l_uv, 0.0f));
}
fn coax_p_cg_post_sample_2(p_uv: vec2f) -> vec3f {
  var l_uv: vec2f = p_uv;
  return coax_p_cg_rgb(textureSampleLevel(post_light, post_lin, l_uv, 0.0f));
}
fn coax_p_fivelab_fx_fs(p_v: coax_t18) -> vec4f {
  var l_v: coax_t18 = p_v;
  var l_lod: f32;
  var l_alpha: f32;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  if ((l_v.f4.w > 0x1.000000p-1f))
  {
    return coax_p_fivelab_hud_fs(l_v);
  }
  if ((l_v.w.w > 0x1.c00000p1f))
  {
    l_lod = coax_p_clampf(coax_p_wg_log2((0x1.000000p6f / coax_p_fmax(l_v.z.x, 0x1.000000p0f))), 0.0f, 0x1.000000p2f);
    l_alpha = coax_pow_bg_vs((textureSampleLevel(sans_atlas, sans_sampler, l_v.y, l_lod)).w, 0x1.b33334p-1f);
    return coax_p_wg_make_f4_f3f(coax_p_scale3(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), ((l_alpha * coax_p_fmin((l_v.z.z * 0x1.266666p0f), 0x1.000000p0f)) * coax_at_u(i32(3i)))), 0x1.000000p0f);
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.1eb852p-2f);
  return coax_p_wg_make_f4_f3f(coax_p_scale3(coax_p_scale3(coax_p_scale3(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
}
fn coax_p_e2_casc(p_k: i32, p_row: i32) -> vec4f {
  var l_k: i32 = p_k;
  var l_row: i32 = p_row;
  return coax_p_wg_at4(((108i + (l_k * 24i)) + (l_row * 4i)));
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_e2_field_fine_x(p_t: i32, p_x: u32, p_z: u32) -> u32 {
  var l_t: i32 = p_t;
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_fx: u32;
  var l_fz: u32;
  var l_p: u32;
  var l_p_6: u32;
  var l_p_7: u32;
  l_fx = l_x;
  l_fz = l_z;
  if ((l_t >= 3i))
  {
    l_p = coax_p_e2_field_pick(l_fx, l_fz, 3u);
    l_fx = ((l_fx * 2u) + (l_p & 1u));
    l_fz = ((l_fz * 2u) + (l_p >> u32(1u)));
  }
  if ((l_t >= 2i))
  {
    l_p_6 = coax_p_e2_field_pick(l_fx, l_fz, 2u);
    l_fx = ((l_fx * 2u) + (l_p_6 & 1u));
    l_fz = ((l_fz * 2u) + (l_p_6 >> u32(1u)));
  }
  if ((l_t >= 1i))
  {
    l_p_7 = coax_p_e2_field_pick(l_fx, l_fz, 1u);
    l_fx = ((l_fx * 2u) + (l_p_7 & 1u));
    l_fz = ((l_fz * 2u) + (l_p_7 >> u32(1u)));
  }
  return l_fx;
}
fn coax_p_e2_field_fine_z(p_t: i32, p_x: u32, p_z: u32) -> u32 {
  var l_t: i32 = p_t;
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_fx: u32;
  var l_fz: u32;
  var l_p: u32;
  var l_p_6: u32;
  var l_p_7: u32;
  l_fx = l_x;
  l_fz = l_z;
  if ((l_t >= 3i))
  {
    l_p = coax_p_e2_field_pick(l_fx, l_fz, 3u);
    l_fx = ((l_fx * 2u) + (l_p & 1u));
    l_fz = ((l_fz * 2u) + (l_p >> u32(1u)));
  }
  if ((l_t >= 2i))
  {
    l_p_6 = coax_p_e2_field_pick(l_fx, l_fz, 2u);
    l_fx = ((l_fx * 2u) + (l_p_6 & 1u));
    l_fz = ((l_fz * 2u) + (l_p_6 >> u32(1u)));
  }
  if ((l_t >= 1i))
  {
    l_p_7 = coax_p_e2_field_pick(l_fx, l_fz, 1u);
    l_fx = ((l_fx * 2u) + (l_p_7 & 1u));
    l_fz = ((l_fz * 2u) + (l_p_7 >> u32(1u)));
  }
  return l_fz;
}
fn coax_p_e2_field_hash(p_x: u32, p_z: u32, p_k: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_k: u32 = p_k;
  return coax_p_e2_ihash((((l_x * 73856093u) ^ (l_z * 19349663u)) ^ ((l_k + 1u) * 83492791u)));
}
fn coax_p_e2_unit(p_h: u32) -> f32 {
  var l_h: u32 = p_h;
  return (f32((l_h >> u32(8u))) / 0x1.000000p24f);
}
fn coax_p_e2_field_start(p_t: i32) -> f32 {
  var l_t: i32 = p_t;
  return select((0x1.000000p4f + (f32(l_t) * 0x1.800000p3f)), 0.0f, (l_t == 0i));
}
fn coax_p_e2_field_out(p_t: i32) -> f32 {
  var l_t: i32 = p_t;
  return select((0x1.000000p4f + (f32((l_t + 1i)) * 0x1.800000p3f)), 0x1.180000p7f, (l_t == 3i));
}
fn coax_p_e2_field_start_r(p_t: i32, p_g: f32, p_c: f32) -> f32 {
  var l_t: i32 = p_t;
  var l_g: f32 = p_g;
  var l_c: f32 = p_c;
  if (((l_t == 0i) || ((l_t == 1i) && coax_p_e2_field_fine_off(l_c))))
  {
    return 0.0f;
  }
  return ((0x1.000000p4f + (f32(l_t) * 0x1.800000p3f)) * l_g);
}
fn coax_p_e2_field_out_r(p_t: i32, p_g: f32, p_c: f32) -> f32 {
  var l_t: i32 = p_t;
  var l_g: f32 = p_g;
  var l_c: f32 = p_c;
  if ((l_t == 3i))
  {
    return (0x1.180000p7f * l_g);
  }
  if (((l_t == 0i) && coax_p_e2_field_fine_off(l_c)))
  {
    return 0.0f;
  }
  return ((0x1.000000p4f + (f32((l_t + 1i)) * 0x1.800000p3f)) * l_g);
}
fn coax_p_e2_field_cover(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_bare: f32;
  if (((((l_x < (-0x1.900000p7f)) || (l_x > 0x1.b80000p7f)) || (l_z < (-0x1.400000p9f))) || (l_z > 0x1.400000p6f)))
  {
    return 0.0f;
  }
  l_bare = coax_p_e2_smooth(0x1.99999ap-4f, 0x1.ae147ap-2f, coax_p_e2_path(l_x, l_z));
  return ((0x1.000000p0f - l_bare) * (0x1.000000p0f - (0x1.19999ap-1f * coax_p_e2_forest(l_x, l_z))));
}
fn coax_p_e2_field_rel(p_t: i32, p_d: f32) -> f32 {
  var l_t: i32 = p_t;
  var l_d: f32 = p_d;
  var l_v: f32;
  var l_v_3: f32;
  if ((l_t == 3i))
  {
    l_v = (0x1.000000p0f - ((l_d - coax_p_e2_field_start(3i)) / (0x1.180000p7f - coax_p_e2_field_start(3i))));
    return select(select(l_v, 0x1.000000p0f, (l_v > 0x1.000000p0f)), 0.0f, (l_v < 0.0f));
  }
  l_v_3 = coax_pow_bg_vs(0x1.000000p-2f, (((l_d - 0x1.000000p4f) - (f32(l_t) * 0x1.800000p3f)) / 0x1.800000p3f));
  return select(l_v_3, 0x1.000000p0f, (l_v_3 > 0x1.000000p0f));
}
fn coax_p_e2_field_rel_r(p_t: i32, p_d: f32, p_g: f32, p_c: f32) -> f32 {
  var l_t: i32 = p_t;
  var l_d: f32 = p_d;
  var l_g: f32 = p_g;
  var l_c: f32 = p_c;
  var l_k: f32;
  var l_s: f32;
  var l_v: f32;
  var l_v_7: f32;
  l_k = select(0x1.99999ap-5f, l_g, (l_g > 0x1.99999ap-5f));
  if ((l_t == 3i))
  {
    l_s = ((0x1.000000p4f + (0x1.800000p1f * 0x1.800000p3f)) * l_k);
    l_v = (0x1.000000p0f - ((l_d - l_s) / ((0x1.180000p7f * l_k) - l_s)));
    return select(select(l_v, 0x1.000000p0f, (l_v > 0x1.000000p0f)), 0.0f, (l_v < 0.0f));
  }
  l_v_7 = coax_pow_bg_vs(0x1.000000p-2f, ((l_d - ((0x1.000000p4f + (f32(l_t) * 0x1.800000p3f)) * l_k)) / (0x1.800000p3f * l_k)));
  l_v_7 = select(l_v_7, 0x1.000000p0f, (l_v_7 > 0x1.000000p0f));
  if (((l_t == 0i) && (l_v_7 > l_c)))
  {
    l_v_7 = l_c;
  }
  return l_v_7;
}
fn coax_p_e2_field_rank(p_t: i32, p_x: u32, p_z: u32, p_fx: u32, p_fz: u32) -> f32 {
  var l_t: i32 = p_t;
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_fx: u32 = p_fx;
  var l_fz: u32 = p_fz;
  var l_u: f32;
  var l_p: u32;
  l_u = coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 5u));
  if ((l_t == 3i))
  {
    return (0x1.0624dep-10f + (0x1.ff7ceep-1f * l_u));
  }
  l_p = coax_p_e2_field_pick((l_x >> u32(1u)), (l_z >> u32(1u)), u32((l_t + 1i)));
  if ((((l_x & 1u) == (l_p & 1u)) && ((l_z & 1u) == (l_p >> u32(1u)))))
  {
    return 0.0f;
  }
  return (0x1.000000p-2f + (0x1.800000p-1f * l_u));
}
fn coax_p_e2_field_shown(p_rel: f32, p_rank: f32) -> f32 {
  var l_rel: f32 = p_rel;
  var l_rank: f32 = p_rank;
  var l_v: f32;
  l_v = ((l_rel - l_rank) / 0x1.47ae14p-4f);
  return select(select(l_v, 0x1.000000p0f, (l_v > 0x1.000000p0f)), 0.0f, (l_v < 0.0f));
}
fn coax_p_e2_forest(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_rd: f32;
  var l_clump: f32;
  var l_road: f32;
  l_rd = coax_p_e2_road_dist(l_x, l_z);
  l_clump = (0x1.a3d70ap-1f + (0x1.70a3d8p-3f * coax_p_e2_smooth(0x1.000000p-2f, 0x1.333334p-1f, coax_p_e2_noise(((l_x / 0x1.e00000p4f) - 0x1.0cccccp2f), ((l_z / 0x1.e00000p4f) + 0x1.a66666p2f)))));
  l_road = coax_p_cg_ramp(0x1.400000p1f, 0x1.000000p2f, l_rd);
  l_road = (l_road + ((0x1.000000p0f - l_road) * coax_p_cg_ramp(0x1.800000p2f, 0x1.800000p3f, l_z)));
  return ((((l_clump * l_road) * coax_p_e2_wooded(l_x, l_z)) * (0x1.000000p0f - coax_p_e2_clearing(l_x, l_z))) * select(0x1.000000p0f, coax_p_stream_keep(coax_p_e2_brook(), l_x, l_z), coax_p_e2_near_brook(l_x, l_z)));
}
fn coax_p_e2_path(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_d: f32;
  var l_spread: f32;
  var l_main: f32;
  var l_branch_d: f32;
  var l_branch_fade: f32;
  var l_branch: f32;
  var l_p: f32;
  var l_w: f32;
  l_d = coax_p_e2_road_dist(l_x, l_z);
  l_spread = (0x1.000000p-1f + (abs(l_z) * 0x1.47ae14p-4f));
  if ((l_spread > 0x1.e66666p0f))
  {
    l_spread = 0x1.e66666p0f;
  }
  l_main = ((exp((((-l_d) * l_d) / l_spread)) * (0x1.000000p0f - coax_p_cg_ramp(0x1.800000p2f, 0x1.800000p3f, l_z))) * (0x1.000000p0f - coax_p_cg_ramp((-0x1.c34cccp8f), (-0x1.d2999ap8f), l_z)));
  l_branch_d = ((l_z + 0x1.400000p3f) + (0x1.666666p0f * (l_x + 0x1.000000p1f)));
  l_branch_fade = (((-l_x) - 0x1.000000p-1f) / 0x1.800000p0f);
  l_branch = (exp((((-l_branch_d) * l_branch_d) / 0x1.000000p2f)) * select(select(l_branch_fade, 0x1.000000p0f, (l_branch_fade > 0x1.000000p0f)), 0.0f, (l_branch_fade < 0.0f)));
  l_p = select(l_branch, l_main, (l_main > l_branch));
  l_w = coax_p_e2_wet(l_x, l_z);
  return select(l_w, l_p, (l_p > l_w));
}
fn coax_p_e2_smooth(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = ((l_x - l_a) / (l_b - l_a));
  l_t = select(select(l_t, 0x1.000000p0f, (l_t > 0x1.000000p0f)), 0.0f, (l_t < 0.0f));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_e2_gold(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_o: f32;
  var l_patch: f32;
  l_o = coax_p_e2_woods_share(l_x, l_z);
  l_patch = coax_p_e2_smooth(0x1.99999ap-2f, 0x1.3d70a4p-1f, coax_p_e2_noise(((l_x / 0x1.e00000p5f) + 0x1.8cccccp1f), ((l_z / 0x1.e00000p5f) - 0x1.b33334p0f)));
  return ((coax_p_cg_autumn(l_x, l_z) * (0x1.000000p0f - l_o)) + ((l_patch * l_o) * 0x1.333334p-2f));
}
fn coax_p_e2_height(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_away: f32;
  var l_h: f32;
  var l_fade: f32;
  var l_o: f32;
  var l_g: f32;
  var l_corridor: f32;
  var l_land: f32;
  l_away = sqrt(((l_x * l_x) + (l_z * l_z)));
  if ((l_away < 0x1.000000p1f))
  {
    return 0.0f;
  }
  l_h = ((0x1.000000p2f * exp((-((((l_x + 0x1.600000p3f) * (l_x + 0x1.600000p3f)) / 0x1.b80000p6f) + (((l_z + 0x1.600000p4f) * (l_z + 0x1.600000p4f)) / 0x1.7c0000p6f))))) + (0x1.733334p2f * exp((-((((l_x - 0x1.100000p4f) * (l_x - 0x1.100000p4f)) / 0x1.400000p7f) + (((l_z + 0x1.080000p5f) * (l_z + 0x1.080000p5f)) / 0x1.680000p7f))))));
  l_h = (l_h + (0x1.666666p3f * exp((-((((l_x + 0x1.000000p3f) * (l_x + 0x1.000000p3f)) / 0x1.e00000p6f) + (((l_z + 0x1.100000p5f) * (l_z + 0x1.100000p5f)) / 0x1.180000p6f))))));
  l_fade = select(0x1.000000p0f, ((l_away - 0x1.000000p1f) / 0x1.800000p1f), (l_away < 0x1.400000p2f));
  l_o = coax_p_e2_outside(l_x, l_z);
  l_g = (((l_h * l_fade) * (0x1.000000p0f - l_o)) + (coax_p_e2_wild(l_x, l_z) * l_o));
  l_corridor = (((0x1.000000p0f - coax_p_cg_ramp(0x1.000000p1f, 0x1.400000p2f, coax_p_e2_road_dist(l_x, l_z))) * coax_p_cg_ramp((-0x1.c00000p3f), (-0x1.200000p4f), l_z)) * (0x1.000000p0f - coax_p_cg_ramp((-0x1.9f999ap8f), (-0x1.c1999ap8f), l_z)));
  l_land = (l_g + ((coax_p_e2_level(l_z) - l_g) * l_corridor));
  if (coax_p_e2_near_city(l_x, l_z, 0x1.600000p4f))
  {
    l_land = coax_p_clearing_floor(coax_p_e2_city(), l_x, l_z, l_land, coax_p_e2_city_level());
  }
  if ((!coax_p_e2_near_brook(l_x, l_z)))
  {
    return l_land;
  }
  return coax_p_stream_bed(coax_p_e2_brook(), l_x, l_z, l_land, coax_p_e2_brook_level(l_z));
}
fn coax_p_e2_kit_place(p_id: u32, p_a: vec4f, p_c: vec4f) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_a: vec4f = p_a;
  var l_c: vec4f = p_c;
  var l_b: u32;
  var l_source: vec3f;
  var l_normal: u32;
  var l_kind: u32;
  var l_p: vec3f;
  var l_sway: f32;
  var l_nw: vec4f;
  var l_cs: f32;
  var l_sn: f32;
  var l_n: vec3f;
  var l_uv: vec2f;
  var l_vt: vec4f;
  var l_t: vec3f;
  var l_o: coax_t15;
  var l_height: f32;
  var l_autumn: f32;
  var l_base: vec3f;
  l_b = (coax_p_wg_u32_f(coax_at_u(i32(48i))) + (l_id * 8u));
  l_source = coax_p_v3(bitcast<f32>(vertices[i32(l_b)]), bitcast<f32>(vertices[i32((l_b + 1u))]), bitcast<f32>(vertices[i32((l_b + 2u))]));
  l_normal = vertices[i32((l_b + 3u))];
  l_kind = ((l_normal >> u32(24u)) & 7u);
  l_p = coax_p_cg_instance_point(l_source, coax_p_v3(l_a.x, l_a.y, l_a.z), l_c.x, l_c.y, l_a.w);
  if ((((l_kind == 1u) || (l_kind == 2u)) || (l_kind == 3u)))
  {
    l_sway = (sin((((l_a.x * 0x1.7ae148p-2f) + (l_a.z * 0x1.ae147ap-3f)) + (coax_at_u(i32(2i)) * 0x1.19999ap0f))) + (sin(((l_a.z * 0x1.0f5c28p-1f) + (coax_at_u(i32(2i)) * 0x1.666666p-1f))) * 0x1.666666p-2f));
    l_p.x = (l_p.x + (((l_sway * 0x1.99999ap-5f) * coax_p_saturate((l_source.y * 0x1.70a3d8p-3f))) * l_c.x));
  }
  l_nw = coax_p_wg_unpack4x8snorm(l_normal);
  l_cs = cos(l_a.w);
  l_sn = sin(l_a.w);
  l_n = coax_p_normalize3(coax_p_v3((((l_nw.x * l_cs) + (l_nw.z * l_sn)) / l_c.y), (l_nw.y / l_c.x), ((((-l_nw.x) * l_sn) + (l_nw.z * l_cs)) / l_c.y)));
  l_uv = coax_p_wg_unpack2x16unorm(vertices[i32((l_b + 4u))]);
  if ((l_kind == 6u))
  {
    l_uv = coax_p_v2(((l_uv.x * 0x1.000000p6f) - 0x1.000000p5f), ((l_uv.y * 0x1.000000p6f) - 0x1.000000p5f));
  }
  l_vt = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 5u))]);
  l_t = select(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_e2_unpack_tint(l_c), ((l_kind == 1u) || (l_kind == 4u)));
  l_o = coax_t15();
  l_o.x = coax_p_fivelab_project(l_p);
  l_o.y = l_p;
  l_o.z = l_n;
  l_o.w = l_uv;
  l_o.f4 = vertices[i32((l_b + 6u))];
  l_o.f5 = select(coax_p_mul3(coax_p_v3(l_vt.x, l_vt.y, l_vt.z), l_t), l_t, (l_kind == 1u));
  l_o.f6 = coax_p_v4(coax_p_e2_inst_fade(l_c), 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(f32(l_kind), bitcast<f32>(vertices[i32((l_b + 7u))]), 0.0f);
  l_o.f8 = 0u;
  if ((l_kind == 2u))
  {
    l_height = coax_p_saturate(l_source.y);
    l_autumn = coax_p_e2_gold(l_a.x, l_a.z);
    l_base = coax_p_e2_unpack_tint(l_c);
    l_o.f5 = coax_p_mix3(l_base, coax_p_v3((0x1.4cccccp-1f + (0x1.1eb852p-2f * l_autumn)), (0x1.c28f5cp-1f - (0x1.99999ap-3f * l_autumn)), 0x1.1eb852p-4f), l_height);
    l_o.f7 = coax_p_v3(0x1.000000p1f, l_height, 0x1.000000p0f);
  }
  return l_o;
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
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_cg_meadow_vs(p_id: u32, p_instance: u32, p_m: coax_t9) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m: coax_t9 = p_m;
  var l_o: coax_t15;
  var l_i: u32;
  var l_kit: i32;
  var l_side: u32;
  var l_iz: u32;
  var l_ix: u32;
  var l_half: u32;
  var l_origin: f32;
  var l_ex: f32;
  var l_ez: f32;
  var l_reach: f32;
  var l_fine: f32;
  var l_t: i32;
  var l_c: f32;
  var l_gx: u32;
  var l_gz: u32;
  var l_fx: u32;
  var l_fz: u32;
  var l_x: f32;
  var l_z: f32;
  var l_dx: f32;
  var l_dz: f32;
  var l_d: f32;
  var l_shown: f32;
  var l_au: f32;
  var l_c_30: f32;
  var l_gx_31: u32;
  var l_gz_32: u32;
  var l_x_33: f32;
  var l_z_34: f32;
  var l_dx_35: f32;
  var l_dz_36: f32;
  var l_k: f32;
  var l_h: f32;
  var l_au_39: f32;
  var l_p: coax_t32;
  l_o = coax_t15();
  l_o.x = coax_p_v4(0.0f, 0.0f, (-0x1.000000p1f), 0x1.000000p0f);
  l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f8 = ((l_instance >> u32(16u)) - 1u);
  l_i = (l_instance & 65535u);
  l_kit = i32(l_m.x.x);
  l_side = u32(l_m.x.z);
  l_iz = (l_i / l_side);
  l_ix = (l_i - (l_iz * l_side));
  l_half = (l_side / 2u);
  l_origin = (-0x1.000000p15f);
  l_ex = coax_at_u(i32(16i));
  l_ez = coax_at_u(i32(18i));
  l_reach = select(0x1.000000p0f, l_m.y.y, (l_m.y.y > 0.0f));
  l_fine = select(0x1.000000p0f, l_m.y.z, (l_m.y.z > 0.0f));
  if ((l_kit == 33i))
  {
    l_t = i32(l_m.x.y);
    l_c = coax_p_e2_field_cell(l_t);
    l_gx = ((u32(floor(((l_ex - l_origin) / l_c))) + l_ix) - l_half);
    l_gz = ((u32(floor(((l_ez - l_origin) / l_c))) + l_iz) - l_half);
    l_fx = coax_p_e2_field_fine_x(l_t, l_gx, l_gz);
    l_fz = coax_p_e2_field_fine_z(l_t, l_gx, l_gz);
    l_x = (l_origin + ((f32(l_fx) + coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 1u))) * 0x1.000000p-1f));
    l_z = (l_origin + ((f32(l_fz) + coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 2u))) * 0x1.000000p-1f));
    l_dx = (l_x - l_ex);
    l_dz = (l_z - l_ez);
    l_d = sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
    if (((l_d < coax_p_e2_field_start_r(l_t, l_reach, l_fine)) || (l_d >= coax_p_e2_field_out_r(l_t, l_reach, l_fine))))
    {
      return l_o;
    }
    l_shown = coax_p_e2_field_shown(coax_p_e2_field_rel_r(l_t, l_d, l_reach, l_fine), coax_p_e2_field_rank(l_t, l_gx, l_gz, l_fx, l_fz));
    if ((l_shown <= 0.0f))
    {
      return l_o;
    }
    l_au = coax_p_e2_meadow_gold(l_x, l_z);
    return coax_p_cg_meadow_place(l_id, coax_p_v4(l_x, coax_p_exp_height_gpu(l_x, l_z), l_z, (coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 3u)) * 0x1.921fb6p2f)), (coax_p_e2_meadow_tuft_sv(l_fx, l_fz) * l_shown), (coax_p_e2_meadow_tuft_sh(l_fx, l_fz) * l_shown), coax_p_v3((0x1.333334p-2f + (0x1.0a3d70p-2f * l_au)), 0x1.eb851ep-2f, 0x1.c28f5cp-5f), l_au, l_m, l_o.f8);
  }
  l_c_30 = coax_p_e2_meadow_cell(l_kit);
  l_gx_31 = ((u32(floor(((l_ex - l_origin) / l_c_30))) + l_ix) - l_half);
  l_gz_32 = ((u32(floor(((l_ez - l_origin) / l_c_30))) + l_iz) - l_half);
  if ((!coax_p_e2_meadow_held(l_gx_31, l_gz_32, l_kit)))
  {
    return l_o;
  }
  l_x_33 = coax_p_e2_meadow_at_x(l_gx_31, l_gz_32, l_kit);
  l_z_34 = coax_p_e2_meadow_at_z(l_gx_31, l_gz_32, l_kit);
  l_dx_35 = (l_x_33 - l_ex);
  l_dz_36 = (l_z_34 - l_ez);
  if ((l_m.z.z > l_m.z.x))
  {
    if (((((l_x_33 > l_m.z.x) && (l_x_33 < l_m.z.z)) && (l_z_34 > l_m.z.y)) && (l_z_34 < l_m.z.w)))
    {
      return l_o;
    }
    if (((l_m.x.w > 0.0f) && (((((l_x_33 < (l_m.z.x - l_m.x.w)) || (l_x_33 > (l_m.z.z + l_m.x.w))) || (l_z_34 < (l_m.z.y - l_m.x.w))) || (l_z_34 > (l_m.z.w + l_m.x.w))) || (coax_p_cg_hash(coax_p_e2_meadow_seed(l_gx_31, l_gz_32, l_kit), 0x1.200000p3f) > l_m.y.x))))
    {
      return l_o;
    }
  }
  l_k = coax_p_e2_meadow_fade(l_kit, (sqrt(((l_dx_35 * l_dx_35) + (l_dz_36 * l_dz_36))) / l_reach));
  if ((l_k <= 0.0f))
  {
    return l_o;
  }
  l_h = coax_p_e2_meadow_seed(l_gx_31, l_gz_32, l_kit);
  l_au_39 = coax_p_e2_meadow_gold(l_x_33, l_z_34);
  l_p = coax_p_e2_meadow_piece(l_kit, l_h, l_au_39);
  return coax_p_cg_meadow_place(l_id, coax_p_v4(l_x_33, (coax_p_exp_height_gpu(l_x_33, l_z_34) - l_p.w), l_z_34, (coax_p_cg_hash(l_h, 0x1.000000p4f) * 0x1.921cacp2f)), (l_p.y * l_k), (l_p.z * l_k), coax_p_v3(l_p.f4[0i], l_p.f4[1i], l_p.f4[2i]), l_au_39, l_m, l_o.f8);
}
fn coax_p_cg_instanced_vs(p_id: u32, p_instance: u32, p_m: coax_t9) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m: coax_t9 = p_m;
  var l_b: u32;
  var l_seed: u32;
  var l_source: vec3f;
  var l_center: vec3f;
  var l_sizes: vec2f;
  var l_angle: f32;
  var l_p: vec3f;
  var l_height: f32;
  var l_packed: u32;
  var l_nw: vec4f;
  var l_n: vec3f;
  var l_base: vec4f;
  var l_autumn: f32;
  var l_o: coax_t15;
  var l_rust: f32;
  var l_bottom: vec3f;
  var l_top: vec3f;
  l_b = (coax_p_wg_u32_f(l_m.y.w) + (l_id * 8u));
  l_seed = (coax_p_wg_u32_f(l_m.x.x) + ((l_instance & 65535u) * 8u));
  l_source = coax_p_v3(bitcast<f32>(vertices[i32(l_b)]), bitcast<f32>(vertices[i32((l_b + 1u))]), bitcast<f32>(vertices[i32((l_b + 2u))]));
  l_center = coax_p_v3(bitcast<f32>(vertices[i32(l_seed)]), bitcast<f32>(vertices[i32((l_seed + 1u))]), bitcast<f32>(vertices[i32((l_seed + 2u))]));
  l_sizes = coax_p_wg_unpack2x16unorm(vertices[i32((l_seed + 4u))]);
  l_angle = bitcast<f32>(vertices[i32((l_seed + 7u))]);
  l_p = coax_p_cg_instance_point(l_source, l_center, l_sizes.x, l_sizes.y, l_angle);
  l_height = coax_p_saturate(l_source.y);
  l_p.x = (l_p.x + ((((sin((((l_center.x * 0x1.ccccccp0f) + (l_center.z * 0x1.666666p-1f)) + (coax_at_u(i32(2i)) * 0x1.99999ap0f))) + (sin(((l_center.z * 0x1.59999ap1f) + (coax_at_u(i32(2i)) * 0x1.ccccccp-1f))) * 0x1.666666p-2f)) * 0x1.70a3d8p-5f) * l_height) * l_height));
  l_packed = vertices[i32((l_b + 3u))];
  l_nw = coax_p_wg_unpack4x8snorm(l_packed);
  l_n = coax_p_normalize3(coax_p_v3((((l_nw.x * cos(l_angle)) + (l_nw.z * sin(l_angle))) / l_sizes.y), (l_nw.y / l_sizes.x), ((((-l_nw.x) * sin(l_angle)) + (l_nw.z * cos(l_angle))) / l_sizes.y)));
  l_base = coax_p_wg_unpack4x8unorm(vertices[i32((l_seed + 5u))]);
  l_autumn = coax_p_cg_autumn(l_center.x, l_center.z);
  l_o = coax_t15();
  l_o.x = coax_p_fivelab_project(l_p);
  l_o.y = l_p;
  l_o.z = l_n;
  l_o.w = coax_p_wg_unpack2x16unorm(vertices[i32((l_b + 4u))]);
  l_o.f4 = vertices[i32((l_b + 6u))];
  l_rust = ((coax_p_saturate(((l_center.x - 0x1.000000p-1f) / 0x1.800000p1f)) * coax_p_smoothstep(0x1.000000p-1f, 0x1.70a3d8p-1f, coax_p_cg_noise(coax_p_v2((l_center.x * 0x1.99999ap-1f), (l_center.z * 0x1.99999ap-1f))))) * (0x1.000000p0f - l_autumn));
  l_bottom = coax_p_mix3(coax_p_v3(l_base.x, l_base.y, l_base.z), coax_p_v3(0x1.1eb852p-1f, 0x1.5c28f6p-3f, 0x1.99999ap-6f), l_rust);
  l_top = coax_p_mix3(coax_p_v3((0x1.4cccccp-1f + (0x1.1eb852p-2f * l_autumn)), (0x1.c28f5cp-1f - (0x1.99999ap-3f * l_autumn)), 0x1.1eb852p-4f), coax_p_v3(0x1.e66666p-1f, 0x1.a3d70ap-2f, 0x1.1eb852p-5f), l_rust);
  l_o.f5 = coax_p_mix3(l_bottom, l_top, l_height);
  l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(0x1.000000p1f, l_height, 0x1.000000p0f);
  l_o.f8 = ((l_instance >> u32(16u)) - 1u);
  return l_o;
}
fn coax_p_cg_environment_vs(p_id: u32, p_instance: u32, p_m: coax_t9) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m: coax_t9 = p_m;
  var l_b: u32;
  var l_p: vec3f;
  var l_sky_clip: vec4f;
  var l_seed_uv: vec2f;
  var l_uv: vec2f;
  var l_normal: u32;
  var l_kind: u32;
  var l_tint: vec4f;
  var l_wind: f32;
  var l_nw: vec4f;
  var l_normal3: vec3f;
  var l_local: vec3f;
  var l_cs: f32;
  var l_sn: f32;
  var l_o: coax_t15;
  l_b = (coax_p_wg_u32_f(l_m.y.w) + (l_id * 8u));
  l_p = coax_p_v3(bitcast<f32>(vertices[i32(l_b)]), bitcast<f32>(vertices[i32((l_b + 1u))]), bitcast<f32>(vertices[i32((l_b + 2u))]));
  l_sky_clip = coax_p_v4(l_p.x, l_p.y, 0x1.ffffdep-1f, 0x1.000000p0f);
  l_seed_uv = coax_p_wg_unpack2x16unorm(vertices[i32((l_b + 4u))]);
  l_uv = l_seed_uv;
  l_normal = vertices[i32((l_b + 3u))];
  l_kind = ((l_normal >> u32(24u)) & 15u);
  if ((l_kind == 0u))
  {
    l_p = coax_p_add3(coax_p_wg_at3(16i), coax_p_scale3(coax_p_cg_sky_ray(l_p.x, l_p.y), 0x1.680000p7f));
  }
  if ((l_kind == 6u))
  {
    l_uv = coax_p_v2(((l_seed_uv.x * 0x1.000000p6f) - 0x1.000000p5f), ((l_seed_uv.y * 0x1.000000p6f) - 0x1.000000p5f));
  }
  if (((l_kind == 4u) && (bitcast<f32>(vertices[i32((l_b + 7u))]) >= 0x1.900000p6f)))
  {
    l_uv = coax_p_v2(((l_seed_uv.x * 0x1.000000p3f) - 0x1.000000p2f), ((l_seed_uv.y * 0x1.000000p3f) - 0x1.000000p2f));
  }
  l_tint = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 5u))]);
  l_wind = (sin((((l_p.x * 0x1.ccccccp0f) + (l_p.z * 0x1.666666p-1f)) + (coax_at_u(i32(2i)) * 0x1.99999ap0f))) + (sin(((l_p.z * 0x1.59999ap1f) + (coax_at_u(i32(2i)) * 0x1.ccccccp-1f))) * 0x1.666666p-2f));
  if (((l_kind == 1u) || (l_kind == 2u)))
  {
    l_p.x = (l_p.x + ((l_wind * select(0x1.1eb852p-5f, 0x1.99999ap-6f, (l_kind == 2u))) * select(coax_p_saturate((l_p.y * 0x1.666666p-1f)), (coax_p_wg_unpack2x16unorm(vertices[i32((l_b + 4u))])).y, (l_kind == 2u))));
  }
  l_nw = coax_p_wg_unpack4x8snorm(l_normal);
  l_normal3 = coax_p_v3(l_nw.x, l_nw.y, l_nw.z);
  if ((l_m.w.w > 0x1.280000p4f))
  {
    l_p = coax_p_add3(coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z), coax_p_add3(coax_p_add3(coax_p_scale3(coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z), l_p.x), coax_p_scale3(coax_p_v3(l_m.f4.x, l_m.f4.y, l_m.f4.z), l_p.y)), coax_p_scale3(coax_p_v3(l_m.f5.x, l_m.f5.y, l_m.f5.z), l_p.z)));
    l_normal3 = coax_p_normalize3(coax_p_add3(coax_p_add3(coax_p_scale3(coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z), l_normal3.x), coax_p_scale3(coax_p_v3(l_m.f4.x, l_m.f4.y, l_m.f4.z), l_normal3.y)), coax_p_scale3(coax_p_v3(l_m.f5.x, l_m.f5.y, l_m.f5.z), l_normal3.z)));
  }
  else
  {
    if ((l_m.w.w > 0x1.180000p4f))
    {
      l_local = coax_p_sub3(l_p, coax_p_v3(l_m.y.x, l_m.y.y, l_m.y.z));
      if ((l_m.z.z > 0.0f))
      {
        l_local = coax_p_scale3(l_local, l_m.z.z);
      }
      l_cs = l_m.z.y;
      l_sn = l_m.z.x;
      l_p = coax_p_add3(coax_p_v3(l_m.x.x, l_m.x.y, l_m.x.z), coax_p_v3(((l_local.x * l_cs) + (l_local.z * l_sn)), l_local.y, (((-l_local.x) * l_sn) + (l_local.z * l_cs))));
      l_normal3 = coax_p_v3(((l_normal3.x * l_cs) + (l_normal3.z * l_sn)), l_normal3.y, (((-l_normal3.x) * l_sn) + (l_normal3.z * l_cs)));
    }
  }
  l_o = coax_t15();
  l_o.x = select(coax_p_fivelab_project(l_p), l_sky_clip, (l_kind == 0u));
  l_o.y = l_p;
  l_o.z = l_normal3;
  l_o.w = l_uv;
  l_o.f4 = vertices[i32((l_b + 6u))];
  l_o.f5 = coax_p_v3(l_tint.x, l_tint.y, l_tint.z);
  l_o.f6 = coax_p_v4(select(0.0f, 0x1.000000p0f, ((l_m.w.w < 0x1.080000p4f) && (l_m.z.x > 0x1.000000p-1f))), 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(f32(l_kind), bitcast<f32>(vertices[i32((l_b + 7u))]), 0.0f);
  l_o.f8 = l_instance;
  if (((l_m.w.w > 0x1.280000p4f) && (l_m.x.w > 0.0f)))
  {
    l_o.f7.z = (l_tint.w - (0x1.000000p0f - l_m.x.w));
    l_o.f6.w = 0x1.a00000p3f;
  }
  return l_o;
}
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
}
fn coax_p_fivelab_block_vs(p_id: u32, p_instance: u32, p_m: coax_t9) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_m: coax_t9 = p_m;
  var l_boxes: array<f32, 104>;
  var l_corner_a: array<f32, 6>;
  var l_corner_b: array<f32, 6>;
  var l_v: i32;
  var l_q: i32;
  var l_k: i32;
  var l_face: i32;
  var l_axis: i32;
  var l_side: f32;
  var l_ca: f32;
  var l_cb: f32;
  var l_bones: bool;
  var l_thin: f32;
  var l_h: vec3f;
  var l_c: vec3f;
  var l_nl: vec3f;
  var l_local: vec3f;
  var l_at4: vec4f;
  var l_joint: u32;
  var l_r: u32;
  var l_ra: vec4f;
  var l_rc: vec4f;
  var l_re: vec4f;
  var l_p: vec3f;
  var l_n: vec3f;
  var l_bs: f32;
  var l_r2: u32;
  var l_sa: vec4f;
  var l_sc: vec4f;
  var l_se: vec4f;
  var l_cs: f32;
  var l_sn: f32;
  var l_w: vec3f;
  var l_o: coax_t15;
  var l_what: f32;
  var l_cloth: vec3f;
  var l_t: vec3f;
  l_boxes = array<f32, 104>(0x1.000000p0f, 0.0f, 0x1.e66666p-1f, (-0x1.47ae14p-5f), 0x1.47ae14p-3f, 0x1.70a3d8p-4f, 0x1.c28f5cp-4f, 0x1.000000p0f, 0x1.000000p2f, 0.0f, 0x1.4cccccp0f, (-0x1.eb851ep-6f), 0x1.5c28f6p-3f, 0x1.99999ap-3f, 0x1.c28f5cp-4f, 0x1.000000p0f, 0x1.800000p2f, 0.0f, 0x1.a8f5c2p0f, (-0x1.47ae14p-7f), 0x1.851eb8p-4f, 0x1.eb851ep-4f, 0x1.c28f5cp-4f, 0.0f, 0x1.000000p3f, 0x1.449ba6p-2f, 0x1.74bc6ap0f, (-0x1.1a9fbep-4f), 0x1.020c4ap-3f, 0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.000000p0f, 0x1.200000p3f, 0x1.20418ap-1f, 0x1.74bc6ap0f, (-0x1.1eb852p-4f), 0x1.eb851ep-4f, 0x1.47ae14p-5f, 0x1.47ae14p-5f, 0x1.000000p1f, 0x1.000000p5f, (-0x1.449ba6p-2f), 0x1.74bc6ap0f, (-0x1.1a9fbep-4f), 0x1.020c4ap-3f, 0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.000000p0f, 0x1.080000p5f, (-0x1.20418ap-1f), 0x1.74bc6ap0f, (-0x1.1eb852p-4f), 0x1.eb851ep-4f, 0x1.47ae14p-5f, 0x1.47ae14p-5f, 0x1.000000p1f, 0x1.b80000p5f, 0x1.74bc6ap-4f, 0x1.83126ep-1f, (-0x1.26e978p-5f), 0x1.333334p-4f, 0x1.b851ecp-3f, 0x1.333334p-4f, 0x1.000000p0f, 0x1.c00000p5f, 0x1.74bc6ap-4f, 0x1.418938p-2f, (-0x1.99999ap-5f), 0x1.eb851ep-5f, 0x1.d2f1aap-3f, 0x1.eb851ep-5f, 0x1.000000p1f, 0x1.c80000p5f, 0x1.74bc6ap-4f, 0x1.99999ap-5f, 0.0f, 0x1.c28f5cp-5f, 0x1.99999ap-5f, 0x1.eb851ep-4f, 0x1.800000p1f, 0x1.e00000p5f, (-0x1.74bc6ap-4f), 0x1.83126ep-1f, (-0x1.26e978p-5f), 0x1.333334p-4f, 0x1.b851ecp-3f, 0x1.333334p-4f, 0x1.000000p0f, 0x1.e80000p5f, (-0x1.74bc6ap-4f), 0x1.418938p-2f, (-0x1.99999ap-5f), 0x1.eb851ep-5f, 0x1.d2f1aap-3f, 0x1.eb851ep-5f, 0x1.000000p1f, 0x1.f00000p5f, (-0x1.74bc6ap-4f), 0x1.99999ap-5f, 0.0f, 0x1.c28f5cp-5f, 0x1.99999ap-5f, 0x1.eb851ep-4f, 0x1.800000p1f);
  l_corner_a = array<f32, 6>((-0x1.000000p0f), 0x1.000000p0f, 0x1.000000p0f, (-0x1.000000p0f), 0x1.000000p0f, (-0x1.000000p0f));
  l_corner_b = array<f32, 6>((-0x1.000000p0f), (-0x1.000000p0f), 0x1.000000p0f, (-0x1.000000p0f), 0x1.000000p0f, 0x1.000000p0f);
  l_v = i32(l_id);
  l_q = ((l_v / 36i) * 8i);
  l_k = (l_v % 36i);
  l_face = (l_k / 6i);
  l_axis = (l_face / 2i);
  l_side = select(0x1.000000p0f, (-0x1.000000p0f), ((l_face % 2i) == 0i));
  l_ca = l_corner_a[(l_k % 6i)];
  l_cb = l_corner_b[(l_k % 6i)];
  l_bones = (coax_p_wg_fmod(l_m.w.w, 0x1.000000p1f) > 0x1.000000p-1f);
  l_thin = 0x1.000000p0f;
  if (l_bones)
  {
    l_thin = select(0x1.19999ap-1f, 0x1.b33334p-1f, (l_q == 16i));
  }
  l_h = coax_p_v3((l_boxes[(l_q + 4i)] * l_thin), l_boxes[(l_q + 5i)], (l_boxes[(l_q + 6i)] * l_thin));
  l_c = coax_p_v3(l_ca, l_cb, l_side);
  l_nl = coax_p_v3(0.0f, 0.0f, l_side);
  if ((l_axis == 0i))
  {
    l_c = coax_p_v3(l_side, l_ca, l_cb);
    l_nl = coax_p_v3(l_side, 0.0f, 0.0f);
  }
  if ((l_axis == 1i))
  {
    l_c = coax_p_v3(l_cb, l_side, l_ca);
    l_nl = coax_p_v3(0.0f, l_side, 0.0f);
  }
  l_local = coax_p_add3(coax_p_v3(l_boxes[(l_q + 1i)], l_boxes[(l_q + 2i)], l_boxes[(l_q + 3i)]), coax_p_mul3(l_c, l_h));
  l_at4 = coax_p_v4(l_local.x, l_local.y, l_local.z, 0x1.000000p0f);
  l_joint = u32(i32(l_boxes[l_q]));
  l_r = (coax_p_wg_u32_f(l_m.y.x) + (l_joint * 3u));
  l_ra = bake[i32(l_r)];
  l_rc = bake[i32((l_r + 1u))];
  l_re = bake[i32((l_r + 2u))];
  l_p = coax_p_v3(coax_p_wg_dot4(l_ra, l_at4), coax_p_wg_dot4(l_rc, l_at4), coax_p_wg_dot4(l_re, l_at4));
  l_n = coax_p_v3(dot(coax_p_v3(l_ra.x, l_ra.y, l_ra.z), l_nl), dot(coax_p_v3(l_rc.x, l_rc.y, l_rc.z), l_nl), dot(coax_p_v3(l_re.x, l_re.y, l_re.z), l_nl));
  l_bs = coax_p_fivelab_gait_share(l_m.y.z, l_joint);
  if ((l_bs > 0x1.0624dep-10f))
  {
    l_r2 = (coax_p_wg_u32_f(l_m.y.y) + (l_joint * 3u));
    l_sa = bake[i32(l_r2)];
    l_sc = bake[i32((l_r2 + 1u))];
    l_se = bake[i32((l_r2 + 2u))];
    l_p = coax_p_mix3(l_p, coax_p_v3(coax_p_wg_dot4(l_sa, l_at4), coax_p_wg_dot4(l_sc, l_at4), coax_p_wg_dot4(l_se, l_at4)), l_bs);
    l_n = coax_p_mix3(l_n, coax_p_v3(dot(coax_p_v3(l_sa.x, l_sa.y, l_sa.z), l_nl), dot(coax_p_v3(l_sc.x, l_sc.y, l_sc.z), l_nl), dot(coax_p_v3(l_se.x, l_se.y, l_se.z), l_nl)), l_bs);
  }
  l_cs = cos(l_m.x.w);
  l_sn = sin(l_m.x.w);
  l_w = coax_p_v3((((l_p.x * l_cs) + (l_p.z * l_sn)) + l_m.x.x), (l_p.y + l_m.x.y), ((((-l_p.x) * l_sn) + (l_p.z * l_cs)) + l_m.x.z));
  l_o = coax_t15();
  l_o.x = coax_p_fivelab_project(l_w);
  l_o.y = l_w;
  l_o.z = coax_p_v3(((l_n.x * l_cs) + (l_n.z * l_sn)), l_n.y, (((-l_n.x) * l_sn) + (l_n.z * l_cs)));
  l_o.w = coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f);
  l_o.f7 = l_local;
  l_o.f8 = l_instance;
  l_o.f4 = 0u;
  l_what = l_boxes[(l_q + 7i)];
  l_cloth = coax_p_mix3(coax_p_v3(0x1.ae147ap-2f, 0x1.5c28f6p-2f, 0x1.eb851ep-3f), coax_p_v3(l_m.f4.x, l_m.f4.y, l_m.f4.z), coax_p_clampf(l_m.f4.w, 0.0f, 0x1.000000p0f));
  l_t = l_cloth;
  if ((l_what < 0x1.000000p-1f))
  {
    l_t = coax_p_v3(l_m.z.x, l_m.z.y, l_m.z.z);
  }
  if (((l_what > 0x1.800000p0f) && (l_what < 0x1.400000p1f)))
  {
    l_t = coax_p_scale3(l_cloth, 0x1.800000p-1f);
  }
  if ((l_what > 0x1.400000p1f))
  {
    l_t = coax_p_v3(0x1.99999ap-3f, 0x1.1eb852p-3f, 0x1.70a3d8p-4f);
  }
  if (l_bones)
  {
    l_t = coax_p_v3(0x1.3d70a4p-1f, 0x1.28f5c2p-1f, 0x1.e147aep-2f);
    if (((l_q == 0i) || (l_q == 8i)))
    {
      l_t = coax_p_v3(0x1.47ae14p-4f, 0x1.1eb852p-4f, 0x1.eb851ep-5f);
    }
  }
  l_o.f5 = l_t;
  l_o.f6 = coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.400000p3f);
  return l_o;
}
fn coax_p_wg_unpack2x16unorm(p_x: u32) -> vec2f {
  var l_x: u32 = p_x;
  return coax_p_v2((f32((l_x & 65535u)) / 0x1.fffe00p15f), (f32((l_x >> u32(16u))) / 0x1.fffe00p15f));
}
fn coax_p_fivelab_gaunt(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_q: vec3f;
  var l_side: f32;
  l_q = l_p;
  if ((abs(l_p.x) < 0x1.eb851ep-3f))
  {
    if (((l_p.y > 0x1.ccccccp-1f) && (l_p.y < 0x1.800000p0f)))
    {
      l_q = coax_p_v3((l_p.x * 0x1.7ae148p-1f), l_p.y, (l_p.z * 0x1.51eb86p-1f));
    }
    if ((l_p.y <= 0x1.ccccccp-1f))
    {
      l_side = select(0x1.99999ap-4f, (-0x1.99999ap-4f), (l_p.x < 0.0f));
      l_q = coax_p_v3((l_side + ((l_p.x - l_side) * 0x1.19999ap-1f)), l_p.y, (l_p.z * 0x1.19999ap-1f));
    }
  }
  else
  {
    if ((l_p.y > 0x1.266666p0f))
    {
      l_q = coax_p_v3(l_p.x, (0x1.666666p0f + ((l_p.y - 0x1.666666p0f) * 0x1.19999ap-1f)), (l_p.z * 0x1.19999ap-1f));
    }
  }
  return l_q;
}
fn coax_p_wg_unpack4x8snorm(p_x: u32) -> vec4f {
  var l_x: u32 = p_x;
  return coax_p_v4(coax_p_wg_snorm8(coax_p_wg_byte(l_x, 0u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 8u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 16u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 24u)));
}
fn coax_p_fivelab_skin_mix(p_ra: u32, p_rb: u32, p_code: f32, p_joints: u32, p_weights: u32, p_at: vec4f, p_nw: vec3f, p_arm: bool) -> coax_t28 {
  var l_ra: u32 = p_ra;
  var l_rb: u32 = p_rb;
  var l_code: f32 = p_code;
  var l_joints: u32 = p_joints;
  var l_weights: u32 = p_weights;
  var l_at: vec4f = p_at;
  var l_nw: vec3f = p_nw;
  var l_arm: bool = p_arm;
  var l_o: coax_t28;
  var l_k: u32;
  var l_w: f32;
  var l_j: u32;
  var l_s: f32;
  var l_m: coax_t35;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_e: vec4f;
  var l_m2: coax_t35;
  l_o = coax_t28();
  l_o.x = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.y = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      l_w = (f32(coax_p_wg_byte(l_weights, (l_k * 8u))) / 0x1.fe0000p7f);
      if ((l_w > 0.0f))
      {
        l_j = coax_p_wg_byte(l_joints, (l_k * 8u));
        l_s = coax_p_fivelab_gait_share(l_code, l_j);
        l_m = coax_p_fivelab_rows(l_ra, l_j, l_arm);
        l_a = l_m.x;
        l_c = l_m.y;
        l_e = l_m.z;
        if ((l_s > 0x1.0624dep-10f))
        {
          l_m2 = coax_p_fivelab_rows(l_rb, l_j, l_arm);
          l_a = coax_p_fivelab_mix4(l_a, l_m2.x, l_s);
          l_c = coax_p_fivelab_mix4(l_c, l_m2.y, l_s);
          l_e = coax_p_fivelab_mix4(l_e, l_m2.z, l_s);
        }
        l_o.x = coax_p_add3(l_o.x, coax_p_scale3(coax_p_v3(coax_p_wg_dot4(l_a, l_at), coax_p_wg_dot4(l_c, l_at), coax_p_wg_dot4(l_e, l_at)), l_w));
        l_o.y = coax_p_add3(l_o.y, coax_p_scale3(coax_p_v3(dot(coax_p_v3(l_a.x, l_a.y, l_a.z), l_nw), dot(coax_p_v3(l_c.x, l_c.y, l_c.z), l_nw), dot(coax_p_v3(l_e.x, l_e.y, l_e.z), l_nw)), l_w));
      }
      l_k = (l_k + 1u);
    }
  }
  return l_o;
}
fn coax_p_fivelab_skin(p_row: u32, p_joints: u32, p_weights: u32, p_at: vec4f, p_nw: vec3f, p_arm: bool) -> coax_t28 {
  var l_row: u32 = p_row;
  var l_joints: u32 = p_joints;
  var l_weights: u32 = p_weights;
  var l_at: vec4f = p_at;
  var l_nw: vec3f = p_nw;
  var l_arm: bool = p_arm;
  var l_o: coax_t28;
  var l_k: u32;
  var l_w: f32;
  var l_m: coax_t35;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_e: vec4f;
  l_o = coax_t28();
  l_o.x = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_o.y = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      l_w = (f32(coax_p_wg_byte(l_weights, (l_k * 8u))) / 0x1.fe0000p7f);
      if ((l_w > 0.0f))
      {
        l_m = coax_p_fivelab_rows(l_row, coax_p_wg_byte(l_joints, (l_k * 8u)), l_arm);
        l_a = l_m.x;
        l_c = l_m.y;
        l_e = l_m.z;
        l_o.x = coax_p_add3(l_o.x, coax_p_scale3(coax_p_v3(coax_p_wg_dot4(l_a, l_at), coax_p_wg_dot4(l_c, l_at), coax_p_wg_dot4(l_e, l_at)), l_w));
        l_o.y = coax_p_add3(l_o.y, coax_p_scale3(coax_p_v3(dot(coax_p_v3(l_a.x, l_a.y, l_a.z), l_nw), dot(coax_p_v3(l_c.x, l_c.y, l_c.z), l_nw), dot(coax_p_v3(l_e.x, l_e.y, l_e.z), l_nw)), l_w));
      }
      l_k = (l_k + 1u);
    }
  }
  return l_o;
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_paint_env_sky(p_rd: vec3f, p_sun: vec3f, p_time: f32) -> vec3f {
  var l_rd: vec3f = p_rd;
  var l_sun: vec3f = p_sun;
  var l_time: f32 = p_time;
  var l_up: f32;
  var l_col: vec3f;
  var l_q: vec2f;
  var l_p: vec2f;
  var l_warp: vec2f;
  var l_density: f32;
  var l_threshold: f32;
  var l_cloud: f32;
  var l_thickness: f32;
  var l_cloud_colour: vec3f;
  var l_across: f32;
  var l_along: f32;
  var l_fine: f32;
  var l_wisps: f32;
  var l_halo: f32;
  l_up = coax_p_saturate(l_rd.y);
  l_col = coax_p_mix3(coax_p_paint_env_c3(16i), coax_p_paint_env_c3(12i), sqrt(coax_p_saturate(((l_up * 0x1.800000p0f) + 0x1.47ae14p-4f))));
  l_q = coax_p_v2((l_rd.x / coax_p_fmax((l_rd.y + 0x1.333334p-3f), 0x1.47ae14p-4f)), (l_rd.z / coax_p_fmax((l_rd.y + 0x1.333334p-3f), 0x1.47ae14p-4f)));
  l_p = coax_p_v2(((l_q.x * 0x1.400000p3f) + (l_time * 0x1.0624dep-10f)), (l_q.y * 0x1.c00000p3f));
  l_warp = coax_p_v2(coax_p_paint_noise(coax_p_v2((l_p.x * 0x1.ccccccp-2f), (l_p.y * 0x1.ccccccp-2f))), coax_p_paint_noise(coax_p_v2(((l_p.x * 0x1.ccccccp-2f) + 0x1.c00000p2f), ((l_p.y * 0x1.ccccccp-2f) - 0x1.800000p1f))));
  l_density = coax_p_paint_cloud_fbm(coax_p_v2((l_p.x + (l_warp.x * 0x1.99999ap-1f)), (l_p.y + (l_warp.y * 0x1.99999ap-1f))));
  l_threshold = (0x1.3d70a4p-1f - (0x1.99999ap-4f * coax_p_smoothstep((-0x1.000000p-1f), 0x1.99999ap-1f, l_rd.x)));
  l_cloud = ((coax_p_smoothstep(l_threshold, (l_threshold + 0x1.70a3d8p-4f), l_density) * coax_p_smoothstep(0.0f, 0x1.eb851ep-4f, l_rd.y)) * (0x1.333334p-3f + (0x1.b33334p-1f * coax_p_smoothstep((-0x1.333334p-3f), 0x1.666666p-1f, l_rd.x))));
  l_thickness = coax_p_saturate(((l_density - l_threshold) * 0x1.800000p2f));
  l_cloud_colour = coax_p_mix3(coax_p_paint_env_c3(20i), coax_p_paint_env_c3(24i), coax_p_saturate(((l_thickness * 0x1.666666p-1f) + 0x1.333334p-3f)));
  l_col = coax_p_mix3(l_col, l_cloud_colour, (l_cloud * coax_p_paint_env_c(15i)));
  l_across = ((l_q.x * 0x1.8f5c28p-1f) + (l_q.y * 0x1.428f5cp-1f));
  l_along = (((-l_q.x) * 0x1.428f5cp-1f) + (l_q.y * 0x1.8f5c28p-1f));
  l_fine = coax_p_paint_cloud_fbm(coax_p_v2(((l_along * 0x1.800000p-1f) + (coax_p_paint_noise(coax_p_v2((l_across * 0x1.333334p-2f), (l_along * 0x1.333334p-2f))) * 0x1.99999ap-2f)), (l_across * 0x1.e00000p4f)));
  l_wisps = (((coax_pow_bg_vs(coax_p_saturate(((l_fine - 0x1.666666p-2f) * 0x1.99999ap0f)), 0x1.800000p1f) * coax_p_smoothstep(0x1.99999ap-4f, 0x1.19999ap-1f, l_rd.y)) * coax_p_paint_env_c(27i)) * coax_p_smoothstep(0x1.666666p-2f, 0x1.666666p-1f, coax_p_paint_noise(coax_p_v2((l_q.x * 0x1.666666p-1f), (l_q.y * 0x1.666666p-1f)))));
  l_col = coax_p_mix3(l_col, coax_p_paint_env_c3(32i), l_wisps);
  l_halo = coax_pow_bg_vs(coax_p_saturate(dot(coax_p_normalize3(l_rd), coax_p_normalize3(l_sun))), coax_p_paint_env_c(23i));
  return coax_p_mix3(l_col, coax_p_paint_env_c3(28i), (l_halo * coax_p_paint_env_c(19i)));
}
fn coax_p_transfer_decode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_v3(coax_p_transfer_linear(l_c.x), coax_p_transfer_linear(l_c.y), coax_p_transfer_linear(l_c.z));
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_render_sample_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_result: coax_t36;
  l_rs_result = coax_p_render_filter_layer8_grad(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
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
fn coax_p_paint_env_sky_tint(p_rd: vec3f) -> vec3f {
  var l_rd: vec3f = p_rd;
  var l_w: f32;
  var l_now: vec3f;
  var l_glade: vec3f;
  l_w = sqrt(coax_p_saturate(((coax_p_saturate(l_rd.y) * 0x1.800000p0f) + 0x1.47ae14p-4f)));
  l_now = coax_p_transfer_decode(coax_p_mix3(coax_p_paint_env_c3(16i), coax_p_paint_env_c3(12i), l_w));
  l_glade = coax_p_transfer_decode(coax_p_mix3(coax_p_v3(0x1.ae147ap-1f, 0x1.e66666p-1f, 0x1.eb851ep-1f), coax_p_v3(0x1.0a3d70p-1f, 0x1.b851ecp-1f, 0x1.e147aep-1f), l_w));
  return coax_p_v3(select(0x1.000000p0f, (l_now.x / l_glade.x), (l_glade.x > 0.0f)), select(0x1.000000p0f, (l_now.y / l_glade.y), (l_glade.y > 0.0f)), select(0x1.000000p0f, (l_now.z / l_glade.z), (l_glade.z > 0.0f)));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_e2_casc_try(p_p: vec3f, p_n: vec3f, p_k: i32) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_k: i32 = p_k;
  var l_x: vec4f;
  var l_f: vec4f;
  var l_q: vec3f;
  var l_edge: f32;
  var l_r: vec4f;
  var l_uv: vec2f;
  var l_lit: f32;
  var l_i: i32;
  var l_a: f32;
  var l_d: f32;
  var l_i_16: i32;
  var l_a_17: f32;
  var l_d_18: f32;
  l_x = coax_p_e2_casc(l_k, 5i);
  if ((l_x.y < 0x1.000000p-1f))
  {
    return (-0x1.000000p0f);
  }
  l_f = coax_p_e2_casc(l_k, 4i);
  l_q = coax_p_e2_casc_place(coax_p_add3(l_p, coax_p_scale3(coax_p_normalize3(l_n), l_f.w)), l_k);
  l_edge = (0x1.000000p0f - l_x.x);
  if ((!((((abs(l_q.x) < l_edge) && (abs(l_q.y) < l_edge)) && (l_q.z > 0.0f)) && (l_q.z < 0x1.000000p0f))))
  {
    return (-0x1.000000p0f);
  }
  l_r = coax_p_e2_casc(l_k, 0i);
  l_uv = coax_p_v2((l_r.x + ((0x1.000000p-1f + (0x1.000000p-1f * l_q.x)) * l_r.z)), (l_r.y + ((0x1.000000p-1f - (0x1.000000p-1f * l_q.y)) * l_r.w)));
  l_lit = 0.0f;
  if ((l_x.w > 0x1.000000p-1f))
  {
    {
      let coax_from: i32 = 0i;
      let coax_to: i32 = 6i;
      for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
      {
        l_a = (f32(l_i) * 0x1.3331fcp1f);
        l_d = sqrt(((f32(l_i) + 0x1.000000p-1f) / 0x1.800000p2f));
        l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((l_uv.x + ((cos(l_a) * l_d) * l_f.x)), (l_uv.y + ((sin(l_a) * l_d) * l_f.y))), (l_q.z - l_f.z)));
      }
    }
    return (l_lit / 0x1.800000p2f);
  }
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 12i;
    for (l_i_16 = coax_from; l_i_16 < coax_to; l_i_16 = l_i_16 + 1i)
    {
      l_a_17 = (f32(l_i_16) * 0x1.3331fcp1f);
      l_d_18 = sqrt(((f32(l_i_16) + 0x1.000000p-1f) / 0x1.800000p3f));
      l_lit = (l_lit + textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((l_uv.x + ((cos(l_a_17) * l_d_18) * l_f.x)), (l_uv.y + ((sin(l_a_17) * l_d_18) * l_f.y))), (l_q.z - l_f.z)));
    }
  }
  return (l_lit / 0x1.800000p3f);
}
fn coax_p_cg_sun_dir() -> vec3f {
  return coax_p_paint_env_sun_dir();
}
fn coax_p_e2_ground_albedo(p_x: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_w: vec4f;
  var l_out: vec3f;
  l_w = coax_p_e2_ground_weights(l_x, l_z);
  l_out = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((l_w.x > 0.0f))
  {
    l_out = coax_p_add3(l_out, coax_p_scale3(coax_p_e2_ground_material(l_x, l_z, 0i, l_w), l_w.x));
  }
  if ((l_w.y > 0.0f))
  {
    l_out = coax_p_add3(l_out, coax_p_scale3(coax_p_e2_ground_material(l_x, l_z, 1i, l_w), l_w.y));
  }
  if ((l_w.z > 0.0f))
  {
    l_out = coax_p_add3(l_out, coax_p_scale3(coax_p_e2_ground_material(l_x, l_z, 2i, l_w), l_w.z));
  }
  if ((l_w.w > 0.0f))
  {
    l_out = coax_p_add3(l_out, coax_p_scale3(coax_p_e2_ground_material(l_x, l_z, 3i, l_w), l_w.w));
  }
  return l_out;
}
fn coax_p_e2_road_x(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  var l_b: f32;
  var l_t: f32;
  var l_wind: f32;
  if ((l_z > (-0x1.e00000p4f)))
  {
    return coax_p_cg_road_x(l_z);
  }
  l_b = coax_p_cg_ramp((-0x1.e00000p4f), (-0x1.2c0000p6f), l_z);
  l_t = (l_z + 0x1.e00000p4f);
  l_wind = ((((0x1.e00000p3f * sin((l_t * 0x1.6872b0p-6f))) + (0x1.800000p2f * sin(((l_t * 0x1.a1cac0p-5f) + 0x1.4cccccp0f)))) - (0x1.800000p2f * sin(0x1.4cccccp0f))) + (0x1.000000p1f * sin((l_t * 0x1.eb851ep-4f))));
  return ((coax_p_cg_road_x(l_z) * (0x1.000000p0f - l_b)) + ((0x1.a66666p1f + l_wind) * l_b));
}
fn coax_p_cg_ramp(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = ((l_x - l_a) / (l_b - l_a));
  l_t = select(select(l_t, 0x1.000000p0f, (l_t > 0x1.000000p0f)), 0.0f, (l_t < 0.0f));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_cg_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_paint_noise(l_p);
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_cg_leaf_palette(p_tint: vec3f, p_age: f32) -> vec3f {
  var l_tint: vec3f = p_tint;
  var l_age: f32 = p_age;
  var l_red: f32;
  var l_deep: vec3f;
  l_red = coax_p_saturate(((0x1.000000p-1f - l_tint.y) * 0x1.800000p1f));
  l_deep = coax_p_mix3(l_tint, coax_p_v3(0x1.51eb86p-1f, 0x1.99999ap-5f, 0x1.47ae14p-6f), (((0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-3f, 0x1.19999ap-1f, l_age)) * 0x1.b33334p-1f) * l_red));
  return coax_p_mix3(l_deep, coax_p_v3(0x1.000000p0f, 0x1.28f5c2p-1f, 0x1.99999ap-4f), (coax_p_smoothstep(0x1.99999ap-2f, 0x1.b33334p-1f, l_age) * 0x1.19999ap-1f));
}
fn coax_p_verse_style_luma(p_r: f32, p_g: f32, p_b: f32) -> f32 {
  var l_r: f32 = p_r;
  var l_g: f32 = p_g;
  var l_b: f32 = p_b;
  return (((0x1.b367a0p-3f * l_r) + (0x1.6e2eb2p-1f * l_g)) + (0x1.27bb30p-4f * l_b));
}
fn coax_p_cg_cloud(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  return coax_p_paint_env_cloud(l_x, l_z);
}
fn coax_p_e2_ground_ao(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_grid: vec4f;
  var l_head: vec4f;
  var l_n: f32;
  var l_span: f32;
  var l_gx: f32;
  var l_gz: f32;
  var l_ix: f32;
  var l_iz: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_side: i32;
  var l_q: i32;
  var l_ao: f32;
  var l_edge: f32;
  var l_t: f32;
  l_grid = ground[0i];
  l_head = ground[1i];
  if ((l_head.z < 0x1.000000p-1f))
  {
    return 0x1.000000p0f;
  }
  l_n = l_grid.x;
  l_span = (l_n * l_grid.y);
  if (((coax_p_aethyr_bake_inside(l_x, l_grid.z, l_span) * coax_p_aethyr_bake_inside(l_z, l_grid.w, l_span)) < 0x1.000000p-1f))
  {
    return 0x1.000000p0f;
  }
  l_gx = ((l_x - l_grid.z) / l_grid.y);
  l_gz = ((l_z - l_grid.w) / l_grid.y);
  l_ix = coax_p_aethyr_bake_index(l_gx, l_n);
  l_iz = coax_p_aethyr_bake_index(l_gz, l_n);
  l_fx = coax_p_aethyr_bake_fraction(l_gx, l_ix);
  l_fz = coax_p_aethyr_bake_fraction(l_gz, l_iz);
  l_side = (i32(l_n) + 1i);
  l_q = ((i32(l_iz) * l_side) + i32(l_ix));
  l_ao = coax_p_aethyr_bake_ao(coax_p_aethyr_bake_bilerp(coax_p_e2_ground_at(l_q), coax_p_e2_ground_at((l_q + 1i)), coax_p_e2_ground_at((l_q + l_side)), coax_p_e2_ground_at(((l_q + l_side) + 1i)), l_fx, l_fz), coax_p_aethyr_bake_rise(0.0f, l_head.x));
  l_edge = coax_p_fmin(coax_p_fmin((l_x - l_grid.z), ((l_grid.z + l_span) - l_x)), coax_p_fmin((l_z - l_grid.w), ((l_grid.w + l_span) - l_z)));
  l_t = coax_p_saturate((l_edge / l_head.y));
  return (0x1.000000p0f + ((((l_ao - 0x1.000000p0f) * l_t) * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t))));
}
fn coax_p_cg_hemisphere(p_n: vec3f) -> vec3f {
  var l_n: vec3f = p_n;
  return coax_p_paint_env_hemisphere(l_n);
}
fn coax_p_cg_irradiance(p_n: vec3f, p_direct: f32, p_occlusion: f32, p_visibility: f32) -> vec3f {
  var l_n: vec3f = p_n;
  var l_direct: f32 = p_direct;
  var l_occlusion: f32 = p_occlusion;
  var l_visibility: f32 = p_visibility;
  return coax_p_paint_env_irradiance(l_n, l_direct, l_occlusion, l_visibility);
}
fn coax_p_cg_sun_colour() -> vec3f {
  return coax_p_paint_env_sun_colour();
}
fn coax_p_cg_soil(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_paint_soil(l_p);
}
fn coax_p_cg_atmosphere(p_colour: vec3f, p_eye: vec3f, p_p: vec3f) -> vec3f {
  var l_colour: vec3f = p_colour;
  var l_eye: vec3f = p_eye;
  var l_p: vec3f = p_p;
  return coax_p_paint_env_atmosphere(l_colour, l_eye, l_p);
}
fn coax_p_cg_air_depth(p_eye: vec3f, p_p: vec3f) -> f32 {
  var l_eye: vec3f = p_eye;
  var l_p: vec3f = p_p;
  return coax_p_paint_env_air_depth(l_eye, l_p);
}
fn coax_p_cc_albedo(p_tex: vec3f, p_dye: vec4f, p_tint: vec3f, p_base: vec3f, p_dyed: bool) -> vec3f {
  var l_tex: vec3f = p_tex;
  var l_dye: vec4f = p_dye;
  var l_tint: vec3f = p_tint;
  var l_base: vec3f = p_base;
  var l_dyed: bool = p_dyed;
  var l_display: vec3f;
  var l_d: vec3f;
  l_display = coax_p_cg_encode(l_tex);
  l_d = select(l_display, coax_p_fivelab_dye(l_display, l_dye), l_dyed);
  return coax_p_mul3(coax_p_mul3(coax_p_cg_decode(l_d), coax_p_cg_decode(l_tint)), l_base);
}
fn coax_p_render_sample_layer8_grad_1(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> vec4f {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_result: coax_t36;
  l_rs_result = coax_p_render_filter_layer8_grad_1(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
}
fn coax_p_cm_normal(p_texel: vec4f, p_strength: f32, p_n: vec3f, p_px: vec3f, p_py: vec3f, p_ux: vec2f, p_uy: vec2f) -> vec3f {
  var l_texel: vec4f = p_texel;
  var l_strength: f32 = p_strength;
  var l_n: vec3f = p_n;
  var l_px: vec3f = p_px;
  var l_py: vec3f = p_py;
  var l_ux: vec2f = p_ux;
  var l_uy: vec2f = p_uy;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_up: vec3f;
  var l_p2: vec3f;
  var l_p1: vec3f;
  var l_t: vec3f;
  var l_b: vec3f;
  var l_m: f32;
  var l_k: f32;
  var l_out: vec3f;
  var l_l: f32;
  l_x = (((l_texel.x * 0x1.000000p1f) - 0x1.000000p0f) * l_strength);
  l_y = (((l_texel.y * 0x1.000000p1f) - 0x1.000000p0f) * l_strength);
  l_z = ((l_texel.z * 0x1.000000p1f) - 0x1.000000p0f);
  l_up = coax_p_scale3(l_py, (-0x1.000000p0f));
  l_p2 = coax_p_cross3(l_up, l_n);
  l_p1 = coax_p_cross3(l_n, l_px);
  l_t = coax_p_add3(coax_p_scale3(l_p2, l_ux.x), coax_p_scale3(l_p1, (-l_uy.x)));
  l_b = coax_p_add3(coax_p_scale3(l_p2, l_ux.y), coax_p_scale3(l_p1, (-l_uy.y)));
  l_m = coax_p_fmax(dot(l_t, l_t), dot(l_b, l_b));
  if ((!(l_m > 0.0f)))
  {
    return l_n;
  }
  l_k = (0x1.000000p0f / sqrt(l_m));
  l_out = coax_p_add3(coax_p_add3(coax_p_scale3(l_t, (l_x * l_k)), coax_p_scale3(l_b, ((-l_y) * l_k))), coax_p_scale3(l_n, coax_p_fmax(l_z, 0.0f)));
  l_l = dot(l_out, l_out);
  if ((!(l_l > 0.0f)))
  {
    return l_n;
  }
  return coax_p_scale3(l_out, (0x1.000000p0f / sqrt(l_l)));
}
fn coax_p_cm_rough(p_rough: f32, p_metal: f32) -> f32 {
  var l_rough: f32 = p_rough;
  var l_metal: f32 = p_metal;
  return coax_p_clampf(l_rough, coax_p_mix(0x1.19999ap-1f, 0x1.666666p-1f, coax_p_saturate(l_metal)), 0x1.000000p0f);
}
fn coax_p_cm_f0(p_albedo: vec3f, p_metal: f32) -> vec3f {
  var l_albedo: vec3f = p_albedo;
  var l_metal: f32 = p_metal;
  return coax_p_mix3(coax_p_v3(0x1.47ae14p-5f, 0x1.47ae14p-5f, 0x1.47ae14p-5f), l_albedo, coax_p_saturate(l_metal));
}
fn coax_p_cm_skin(p_albedo: vec3f, p_f0: vec3f, p_rough: f32, p_n: vec3f, p_g: vec3f, p_l: vec3f, p_v: vec3f) -> vec3f {
  var l_albedo: vec3f = p_albedo;
  var l_f0: vec3f = p_f0;
  var l_rough: f32 = p_rough;
  var l_n: vec3f = p_n;
  var l_g: vec3f = p_g;
  var l_l: vec3f = p_l;
  var l_v: vec3f = p_v;
  var l_ns: vec3f;
  var l_sr: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_nr: vec3f;
  var l_ng: vec3f;
  var l_dr: f32;
  var l_dg: f32;
  var l_db: f32;
  l_ns = coax_p_normalize3(coax_p_add3(coax_p_scale3(l_n, 0x1.000000p-1f), coax_p_scale3(l_g, 0x1.000000p-1f)));
  l_sr = coax_p_fmax(l_rough, 0x1.19999ap-1f);
  l_a = coax_p_cm_ggx(l_f0, coax_p_clampf((l_sr * 0x1.800000p-1f), 0x1.70a3d8p-5f, 0x1.000000p0f), l_ns, l_l, l_v);
  l_b = coax_p_cm_ggx(l_f0, coax_p_clampf((l_sr * 0x1.4cccccp0f), 0x1.70a3d8p-5f, 0x1.000000p0f), l_ns, l_l, l_v);
  l_nr = coax_p_normalize3(coax_p_add3(coax_p_scale3(l_n, 0x1.99999ap-2f), coax_p_scale3(l_g, 0x1.333334p-1f)));
  l_ng = coax_p_normalize3(coax_p_add3(coax_p_scale3(l_n, 0x1.800000p-1f), coax_p_scale3(l_g, 0x1.000000p-2f)));
  l_dr = coax_p_fmax(0.0f, ((dot(l_nr, l_l) + 0x1.666666p-2f) / 0x1.59999ap0f));
  l_dg = coax_p_fmax(0.0f, ((dot(l_ng, l_l) + 0x1.eb851ep-4f) / 0x1.1eb852p0f));
  l_db = coax_p_fmax(0.0f, dot(l_n, l_l));
  return coax_p_add3(coax_p_add3(coax_p_scale3(l_a, 0x1.333334p-3f), coax_p_scale3(l_b, 0x1.b33334p-1f)), coax_p_v3((l_albedo.x * l_dr), (l_albedo.y * l_dg), (l_albedo.z * l_db)));
}
fn coax_p_cm_ggx(p_f0: vec3f, p_rough: f32, p_n: vec3f, p_l: vec3f, p_v: vec3f) -> vec3f {
  var l_f0: vec3f = p_f0;
  var l_rough: f32 = p_rough;
  var l_n: vec3f = p_n;
  var l_l: vec3f = p_l;
  var l_v: vec3f = p_v;
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
fn coax_p_reflect3(p_i: vec3f, p_n: vec3f) -> vec3f {
  var l_i: vec3f = p_i;
  var l_n: vec3f = p_n;
  return coax_p_sub3(l_i, coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_i))));
}
fn coax_p_cm_metal_env(p_along: vec3f, p_around: vec3f, p_rough: f32) -> vec3f {
  var l_along: vec3f = p_along;
  var l_around: vec3f = p_around;
  var l_rough: f32 = p_rough;
  return coax_p_mix3(l_along, l_around, coax_p_saturate(l_rough));
}
fn coax_p_cm_metal_sky(p_sky: vec3f, p_f0: vec3f, p_rough: f32, p_nv: f32, p_ao: f32) -> vec3f {
  var l_sky: vec3f = p_sky;
  var l_f0: vec3f = p_f0;
  var l_rough: f32 = p_rough;
  var l_nv: f32 = p_nv;
  var l_ao: f32 = p_ao;
  var l_f: f32;
  var l_k: f32;
  l_f = coax_pow_bg_vs((0x1.000000p0f - coax_p_saturate(l_nv)), 0x1.400000p2f);
  l_k = l_ao;
  return coax_p_v3(((l_sky.x * (l_f0.x + ((0x1.000000p0f - l_f0.x) * l_f))) * l_k), ((l_sky.y * (l_f0.y + ((0x1.000000p0f - l_f0.y) * l_f))) * l_k), ((l_sky.z * (l_f0.z + ((0x1.000000p0f - l_f0.z) * l_f))) * l_k));
}
fn coax_p_cm_sheen(p_colour: vec3f, p_rough: f32, p_n: vec3f, p_l: vec3f, p_v: vec3f) -> vec3f {
  var l_colour: vec3f = p_colour;
  var l_rough: f32 = p_rough;
  var l_n: vec3f = p_n;
  var l_l: vec3f = p_l;
  var l_v: vec3f = p_v;
  var l_nl: f32;
  var l_h: vec3f;
  var l_nv: f32;
  var l_nh: f32;
  var l_r: f32;
  var l_inv: f32;
  var l_sin2: f32;
  var l_d: f32;
  var l_vis: f32;
  l_nl = dot(l_n, l_l);
  if ((!(l_nl > 0.0f)))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_h = coax_p_normalize3(coax_p_add3(l_l, l_v));
  l_nv = coax_p_fmax(0x1.a36e2ep-14f, dot(l_n, l_v));
  l_nh = coax_p_clampf(dot(l_n, l_h), 0.0f, 0x1.000000p0f);
  l_r = coax_p_clampf(l_rough, 0x1.1eb852p-4f, 0x1.000000p0f);
  l_inv = (0x1.000000p0f / l_r);
  l_sin2 = coax_p_fmax((0x1.000000p0f - (l_nh * l_nh)), 0x1.000000p-7f);
  l_d = (((0x1.000000p1f + l_inv) * coax_pow_bg_vs(l_sin2, (l_inv * 0x1.000000p-1f))) / (0x1.000000p1f * 0x1.921fb6p1f));
  l_vis = (0x1.000000p0f / (0x1.000000p2f * ((l_nl + l_nv) - (l_nl * l_nv))));
  return coax_p_scale3(l_colour, coax_p_fmin((((l_d * l_vis) * 0x1.921fb6p1f) * l_nl), 0x1.000000p0f));
}
fn coax_p_cm_strand(p_n: vec3f, p_px: vec3f, p_py: vec3f, p_ux: vec2f, p_uy: vec2f) -> vec3f {
  var l_n: vec3f = p_n;
  var l_px: vec3f = p_px;
  var l_py: vec3f = p_py;
  var l_ux: vec2f = p_ux;
  var l_uy: vec2f = p_uy;
  var l_up: vec3f;
  var l_p2: vec3f;
  var l_p1: vec3f;
  var l_b: vec3f;
  var l_l: f32;
  var l_w: vec3f;
  var l_lw: f32;
  l_up = coax_p_scale3(l_py, (-0x1.000000p0f));
  l_p2 = coax_p_cross3(l_up, l_n);
  l_p1 = coax_p_cross3(l_n, l_px);
  l_b = coax_p_add3(coax_p_scale3(l_p2, l_ux.y), coax_p_scale3(l_p1, (-l_uy.y)));
  l_l = dot(l_b, l_b);
  if ((!(l_l > 0.0f)))
  {
    l_w = coax_p_sub3(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), coax_p_scale3(l_n, l_n.y));
    l_lw = dot(l_w, l_w);
    return select(coax_p_v3(0x1.000000p0f, 0.0f, 0.0f), coax_p_scale3(l_w, (0x1.000000p0f / sqrt(l_lw))), (l_lw > 0.0f));
  }
  return coax_p_scale3(l_b, (0x1.000000p0f / sqrt(l_l)));
}
fn coax_p_cm_hair(p_albedo: vec3f, p_rough: f32, p_t: vec3f, p_n: vec3f, p_l: vec3f, p_v: vec3f) -> vec3f {
  var l_albedo: vec3f = p_albedo;
  var l_rough: f32 = p_rough;
  var l_t: vec3f = p_t;
  var l_n: vec3f = p_n;
  var l_l: vec3f = p_l;
  var l_v: vec3f = p_v;
  var l_wrap: f32;
  var l_h: vec3f;
  var l_r: f32;
  var l_e1: f32;
  var l_e2: f32;
  var l_p: f32;
  var l_q: f32;
  l_wrap = coax_p_saturate(((dot(l_n, l_l) + 0x1.000000p-1f) / 0x1.800000p0f));
  if ((!(l_wrap > 0.0f)))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_h = coax_p_normalize3(coax_p_add3(l_l, l_v));
  l_r = coax_p_clampf(l_rough, 0x1.19999ap-1f, 0x1.000000p0f);
  l_e1 = coax_p_mix(0x1.400000p6f, 0x1.000000p4f, l_r);
  l_e2 = (l_e1 * 0x1.000000p-1f);
  l_p = (coax_p_cm_kk(l_t, l_h, (-0x1.47ae14p-4f), l_n, l_e1) * 0x1.99999ap-5f);
  l_q = (coax_p_cm_kk(l_t, l_h, 0x1.eb851ep-4f, l_n, l_e2) * 0x1.47ae14p-5f);
  return coax_p_scale3(coax_p_add3(coax_p_v3(l_p, l_p, l_p), coax_p_scale3(l_albedo, l_q)), l_wrap);
}
fn coax_p_shade_normal_pack(p_n: vec3f) -> vec2f {
  var l_n: vec3f = p_n;
  var l_v: vec3f;
  var l_p: vec2f;
  l_v = coax_p_scale3(l_n, (0x1.000000p0f / ((abs(l_n.x) + abs(l_n.y)) + abs(l_n.z))));
  l_p = coax_p_v2(l_v.x, l_v.y);
  if ((l_v.z < 0.0f))
  {
    l_p = coax_p_v2(((0x1.000000p0f - abs(l_v.y)) * coax_p_shade_sign(l_v.x)), ((0x1.000000p0f - abs(l_v.x)) * coax_p_shade_sign(l_v.y)));
  }
  return l_p;
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t29 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t29(l_x, l_y);
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
fn coax_p_st_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_transfer_encode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_v3(coax_p_transfer_srgb(l_c.x), coax_p_transfer_srgb(l_c.y), coax_p_transfer_srgb(l_c.z));
}
fn coax_p_portal_field(p_p: vec2f, p_t: f32, p_ripple: f32) -> vec3f {
  var l_p: vec2f = p_p;
  var l_t: f32 = p_t;
  var l_ripple: f32 = p_ripple;
  var l_r: f32;
  var l_a: f32;
  var l_ca: f32;
  var l_sa: f32;
  var l_q: vec2f;
  var l_n: f32;
  var l_m: f32;
  var l_ring: f32;
  var l_r2: f32;
  var l_r4: f32;
  var l_glint: f32;
  var l_swell: f32;
  var l_s2: f32;
  var l_s4: f32;
  var l_core: f32;
  var l_deep: vec3f;
  var l_aqua: vec3f;
  var l_gold: vec3f;
  var l_hi: vec3f;
  var l_c: vec3f;
  var l_cell: vec2f;
  var l_h: f32;
  var l_tw: f32;
  l_r = sqrt(((l_p.x * l_p.x) + (l_p.y * l_p.y)));
  l_a = ((0x1.333334p1f * exp(((-l_r) * 0x1.4cccccp0f))) + (l_t * 0x1.d70a3ep-3f));
  l_ca = cos(l_a);
  l_sa = sin(l_a);
  l_q = coax_p_v2(((l_p.x * l_ca) - (l_p.y * l_sa)), ((l_p.x * l_sa) + (l_p.y * l_ca)));
  l_n = coax_p_portal_fbm(coax_p_v2(((l_q.x * 0x1.19999ap1f) + (l_t * 0x1.c28f5cp-4f)), ((l_q.y * 0x1.19999ap1f) - (l_t * 0x1.1eb852p-4f))));
  l_m = coax_p_portal_fbm(coax_p_v2((((l_q.x * 0x1.533334p2f) - (l_t * 0x1.851eb8p-3f)) + 0x1.8cccccp1f), (((l_q.y * 0x1.533334p2f) + (l_t * 0x1.0a3d70p-3f)) - 0x1.b33334p0f)));
  l_ring = (0x1.000000p-1f + (0x1.000000p-1f * sin((((l_r * 0x1.500000p4f) - (l_t * 0x1.8cccccp1f)) + (l_n * 0x1.400000p2f)))));
  l_r2 = (l_ring * l_ring);
  l_r4 = (l_r2 * l_r2);
  l_glint = ((l_r4 * l_r4) * (0x1.666666p-2f + (0x1.4cccccp-1f * l_m)));
  l_swell = (0x1.000000p-1f + (0x1.000000p-1f * sin((((l_r * 0x1.000000p3f) - (l_t * 0x1.a00000p2f)) + (l_m * 0x1.000000p1f)))));
  l_s2 = (l_swell * l_swell);
  l_s4 = (l_s2 * l_s2);
  l_core = exp((((-l_r) * l_r) * 0x1.4cccccp1f));
  l_deep = coax_p_v3(0x1.0624dep-6f, 0x1.d70a3ep-4f, 0x1.000000p-3f);
  l_aqua = coax_p_v3(0x1.5c28f6p-4f, 0x1.000000p-1f, 0x1.e147aep-2f);
  l_gold = coax_p_v3(0x1.d70a3ep-1f, 0x1.7ae148p-1f, 0x1.ae147ap-2f);
  l_hi = coax_p_v3(0x1.000000p-1f, 0x1.d70a3ep-1f, 0x1.ae147ap-1f);
  l_c = coax_p_mix3(l_deep, l_aqua, coax_p_saturate((0x1.99999ap-3f + ((0x1.99999ap-1f * l_n) * (0x1.333334p0f - (0x1.ccccccp-2f * coax_p_saturate(l_r)))))));
  l_c = coax_p_add3(l_c, coax_p_scale3(l_gold, ((l_core * 0x1.000000p-1f) * (0x1.666666p-1f + (0x1.333334p-1f * l_m)))));
  l_c = coax_p_add3(l_c, coax_p_scale3(l_hi, ((l_glint * (0x1.99999ap-3f + (0x1.000000p-2f * l_ripple))) * (0x1.000000p0f - (0x1.ccccccp-2f * l_core)))));
  l_c = coax_p_add3(l_c, coax_p_scale3(l_hi, (((l_s4 * l_s4) * 0x1.c28f5cp-3f) * l_ripple)));
  l_cell = coax_p_v2(floor((l_q.x * 0x1.100000p5f)), floor((l_q.y * 0x1.100000p5f)));
  l_h = coax_p_render_noise2(l_cell.x, l_cell.y);
  l_tw = (0x1.000000p-1f + (0x1.000000p-1f * sin(((l_t * 0x1.59999ap1f) + (l_h * 0x1.400000p5f)))));
  l_c = coax_p_add3(l_c, coax_p_scale3(l_hi, ((coax_p_stepf(0x1.f851ecp-1f, l_h) * l_tw) * 0x1.666666p-2f)));
  return coax_p_v3(coax_p_fmin(l_c.x, 0x1.a3d70ap-1f), coax_p_fmin(l_c.y, 0x1.a3d70ap-1f), coax_p_fmin(l_c.z, 0x1.a3d70ap-1f));
}
fn coax_p_portal_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
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
  l_ix = floor(l_p.x);
  l_iy = floor(l_p.y);
  l_fx = (l_p.x - l_ix);
  l_fy = (l_p.y - l_iy);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sy = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  l_a = coax_p_render_noise2(l_ix, l_iy);
  l_b = coax_p_render_noise2((l_ix + 0x1.000000p0f), l_iy);
  l_c = coax_p_render_noise2(l_ix, (l_iy + 0x1.000000p0f));
  l_d = coax_p_render_noise2((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f));
  return coax_p_mix(coax_p_mix(l_a, l_b, l_sx), coax_p_mix(l_c, l_d, l_sx), l_sy);
}
fn coax_p_wg_clamp_i(p_x: i32, p_low: i32, p_high: i32) -> i32 {
  var l_x: i32 = p_x;
  var l_low: i32 = p_low;
  var l_high: i32 = p_high;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_low), l_high);
}
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t30 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t30(l_x, l_y);
}
fn coax_p_cg_luma(p_c: vec3f) -> f32 {
  var l_c: vec3f = p_c;
  return coax_p_verse_style_luma(l_c.x, l_c.y, l_c.z);
}
fn coax_p_cg_normal_unpack(p_p: vec2f) -> vec3f {
  var l_p: vec2f = p_p;
  return coax_p_shade_normal_unpack(l_p);
}
fn coax_p_cg_viewpoint(p_pixel: vec2f, p_z: f32) -> vec3f {
  var l_pixel: vec2f = p_pixel;
  var l_z: f32 = p_z;
  return coax_p_shade_view_point(l_pixel, l_z, coax_p_wg_at3(16i), coax_p_wg_at3(20i), coax_p_wg_at3(24i), coax_p_wg_at3(28i), coax_at_u(i32(19i)), coax_p_v2(coax_at_u(i32(23i)), coax_at_u(i32(27i))));
}
fn coax_p_shade_contact_radius(p_height: f32) -> f32 {
  var l_height: f32 = p_height;
  return coax_p_clampf((l_height * 0x1.89374cp-7f), 0x1.000000p2f, 0x1.200000p4f);
}
fn coax_p_shade_contact_tap(p_n: vec3f, p_center: vec3f, p_at: vec3f) -> f32 {
  var l_n: vec3f = p_n;
  var l_center: vec3f = p_center;
  var l_at: vec3f = p_at;
  var l_delta: vec3f;
  var l_distance: f32;
  l_delta = coax_p_sub3(l_at, l_center);
  l_distance = coax_p_wg_length3(l_delta);
  return (coax_p_saturate(((dot(l_n, l_delta) - 0x1.eb851ep-7f) / coax_p_fmax(l_distance, 0x1.a36e2ep-14f))) * (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-4f, 0x1.333334p-1f, l_distance)));
}
fn coax_p_shade_contact_finish(p_sum: f32) -> f32 {
  var l_sum: f32 = p_sum;
  return (0x1.000000p0f - ((l_sum * 0x1.4cccccp-1f) / 0x1.000000p3f));
}
fn coax_p_paint_env_output(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_f: vec3f;
  var l_y: f32;
  var l_k: f32;
  l_f = coax_p_paint_env_film(l_c);
  l_y = (((l_f.x * 0x1.b367a0p-3f) + (l_f.y * 0x1.6e2eb2p-1f)) + (l_f.z * 0x1.27bb30p-4f));
  l_k = coax_p_paint_env_c(69i);
  return coax_p_transfer_encode(coax_p_v3(coax_p_verse_style_saturation(l_f.x, l_y, l_k), coax_p_verse_style_saturation(l_f.y, l_y, l_k), coax_p_verse_style_saturation(l_f.z, l_y, l_k)));
}
fn coax_p_cg_ui_cover(p_p: vec2f, p_w: f32, p_h: f32, p_k: f32) -> bool {
  var l_p: vec2f = p_p;
  var l_w: f32 = p_w;
  var l_h: f32 = p_h;
  var l_k: f32 = p_k;
  if (((l_w < (0x1.a40000p9f * l_k)) || (l_h < (0x1.7c0000p9f * l_k))))
  {
    return ((l_p.y > (l_h - (0x1.f80000p8f * l_k))) || ((((l_p.y > (0x1.a00000p4f * l_k)) && (l_p.y < (0x1.f00000p6f * l_k))) && (l_p.x > (0x1.800000p3f * l_k))) && (l_p.x < (l_w - (0x1.800000p3f * l_k)))));
  }
  return ((((l_p.x >= (0x1.000000p4f * l_k)) && (l_p.x <= (0x1.5e0000p8f * l_k))) && (l_p.y >= (0x1.400000p4f * l_k))) && (l_p.y <= (0x1.6d0000p9f * l_k)));
}
fn coax_p_e2_in_box(p_p: vec2f, p_x0: f32, p_y0: f32, p_x1: f32, p_y1: f32) -> bool {
  var l_p: vec2f = p_p;
  var l_x0: f32 = p_x0;
  var l_y0: f32 = p_y0;
  var l_x1: f32 = p_x1;
  var l_y1: f32 = p_y1;
  return (((((l_x1 > l_x0) && (l_p.x >= l_x0)) && (l_p.x <= l_x1)) && (l_p.y >= l_y0)) && (l_p.y <= l_y1));
}
fn coax_p_e2_box_edge(p_p: vec2f, p_x0: f32, p_y0: f32, p_x1: f32, p_y1: f32, p_k: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_x0: f32 = p_x0;
  var l_y0: f32 = p_y0;
  var l_x1: f32 = p_x1;
  var l_y1: f32 = p_y1;
  var l_k: f32 = p_k;
  var l_d: f32;
  if ((!coax_p_e2_in_box(l_p, l_x0, l_y0, l_x1, l_y1)))
  {
    return 0.0f;
  }
  l_d = coax_p_fmin(coax_p_fmin((l_p.x - l_x0), (l_x1 - l_p.x)), coax_p_fmin((l_p.y - l_y0), (l_y1 - l_p.y)));
  return (0x1.000000p0f - coax_p_smoothstep(0.0f, (0x1.400000p0f * l_k), l_d));
}
fn coax_p_fivelab_hud_fs(p_v: coax_t18) -> vec4f {
  var l_v: coax_t18 = p_v;
  var l_m: f32;
  var l_kind: f32;
  var l_a: f32;
  var l_hw: f32;
  var l_hh: f32;
  var l_r: f32;
  var l_wd: f32;
  var l_qx: f32;
  var l_qy: f32;
  var l_d: f32;
  var l_t: f32;
  var l_c: vec3f;
  var l_cov: f32;
  var l_e: f32;
  var l_turn: f32;
  var l_lod: f32;
  var l_g: vec4f;
  var l_face: f32;
  var l_along: f32;
  var l_dd: f32;
  var l_core: f32;
  var l_gg: f32;
  var l_halo: f32;
  l_m = l_v.w.w;
  if ((l_m > 0x1.300000p3f))
  {
    l_kind = floor((l_m / 0x1.400000p3f));
    l_a = (l_m - (l_kind * 0x1.400000p3f));
    l_hw = l_v.z.x;
    l_hh = l_v.z.y;
    l_r = l_v.z.z;
    l_wd = l_v.z.w;
    l_qx = ((abs(l_v.y.x) - l_hw) + l_r);
    l_qy = ((abs(l_v.y.y) - l_hh) + l_r);
    l_d = ((coax_p_wg_length2(coax_p_v2(coax_p_fmax(l_qx, 0.0f), coax_p_fmax(l_qy, 0.0f))) + coax_p_fmin(coax_p_fmax(l_qx, l_qy), 0.0f)) - l_r);
    l_t = coax_p_clampf((((l_v.y.y / coax_p_fmax(l_hh, 0x1.0624dep-10f)) + 0x1.000000p0f) * 0x1.000000p-1f), 0.0f, 0x1.000000p0f);
    l_c = coax_p_mix3(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), coax_p_v3(l_v.f4.x, l_v.f4.y, l_v.f4.z), l_t);
    l_cov = (0x1.000000p0f - coax_p_smoothstep((-l_wd), l_wd, l_d));
    if ((l_kind > 0x1.800000p0f))
    {
      l_cov = ((0x1.000000p0f - coax_p_smoothstep((-0x1.333334p-1f), 0x1.333334p-1f, l_d)) * coax_p_smoothstep(((-l_wd) - 0x1.333334p-1f), ((-l_wd) + 0x1.333334p-1f), l_d));
    }
    if ((l_kind > 0x1.400000p1f))
    {
      l_e = (0x1.000000p0f - coax_p_smoothstep((-l_wd), 0.0f, l_d));
      l_cov = (l_e * l_e);
    }
    if ((l_kind > 0x1.c00000p1f))
    {
      l_turn = coax_p_wg_atan2(l_v.y.x, (-l_v.y.y));
      if ((l_turn < 0.0f))
      {
        l_turn = (l_turn + 0x1.921fb6p2f);
      }
      l_cov = ((0x1.000000p0f - coax_p_smoothstep((-0x1.333334p-1f), 0x1.333334p-1f, l_d)) * (0x1.000000p0f - coax_p_smoothstep(((l_wd * 0x1.921fb6p2f) - 0x1.47ae14p-6f), (l_wd * 0x1.921fb6p2f), l_turn)));
    }
    return coax_p_v4(l_c.x, l_c.y, l_c.z, coax_p_saturate(((l_cov * l_a) * coax_at_u(i32(3i)))));
  }
  if ((l_m > 0x1.c00000p1f))
  {
    l_lod = coax_p_clampf(coax_p_wg_log2((0x1.000000p6f / coax_p_fmax(l_v.z.x, 0x1.000000p0f))), 0.0f, 0x1.000000p2f);
    l_g = textureSampleLevel(sans_atlas, sans_sampler, l_v.y, l_lod);
    l_face = l_g.x;
    if ((l_m > 0x1.200000p2f))
    {
      l_face = l_g.y;
    }
    return coax_p_v4(l_v.w.x, l_v.w.y, l_v.w.z, coax_p_saturate(((coax_pow_bg_vs(l_face, 0x1.b33334p-1f) * coax_p_fmin((l_v.z.z * 0x1.266666p0f), 0x1.000000p0f)) * coax_at_u(i32(3i)))));
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_dd = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_dd));
  l_gg = (l_dd / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_gg) * l_gg)) * 0x1.1eb852p-2f);
  return coax_p_v4(l_v.w.x, l_v.w.y, l_v.w.z, coax_p_saturate((((l_core + l_halo) * l_v.z.z) * coax_at_u(i32(3i)))));
}
fn coax_p_wg_log2(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (log(l_x) / 0x1.62e430p-1f);
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
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_e2_field_pick(p_x: u32, p_z: u32, p_k: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_z: u32 = p_z;
  var l_k: u32 = p_k;
  return (coax_p_e2_field_hash(l_x, l_z, (l_k + 10u)) & 3u);
}
fn coax_p_e2_ihash(p_v: u32) -> u32 {
  var l_v: u32 = p_v;
  var l_x: u32;
  l_x = (l_v ^ (l_v >> u32(16u)));
  l_x = (l_x * 2146121005u);
  l_x = (l_x ^ (l_x >> u32(15u)));
  l_x = (l_x * 1663597195u);
  return (l_x ^ (l_x >> u32(16u)));
}
fn coax_p_e2_field_fine_off(p_c: f32) -> bool {
  var l_c: f32 = p_c;
  return (l_c <= 0x1.001a36p-2f);
}
fn coax_p_e2_road_dist(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_slope: f32;
  l_slope = (coax_p_e2_road_x((l_z + 0x1.000000p-1f)) - coax_p_e2_road_x((l_z - 0x1.000000p-1f)));
  return (abs((l_x - coax_p_e2_road_x(l_z))) / sqrt((0x1.000000p0f + (l_slope * l_slope))));
}
fn coax_p_e2_noise(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_i: f32;
  var l_j: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_i = floor(l_x);
  l_j = floor(l_z);
  l_fx = (l_x - l_i);
  l_fz = (l_z - l_j);
  l_fx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_fz = ((l_fz * l_fz) * (0x1.800000p1f - (0x1.000000p1f * l_fz)));
  l_a = coax_p_cg_hash(l_i, l_j);
  l_b = coax_p_cg_hash((l_i + 0x1.000000p0f), l_j);
  l_c = coax_p_cg_hash(l_i, (l_j + 0x1.000000p0f));
  l_d = coax_p_cg_hash((l_i + 0x1.000000p0f), (l_j + 0x1.000000p0f));
  return ((((l_a * (0x1.000000p0f - l_fx)) + (l_b * l_fx)) * (0x1.000000p0f - l_fz)) + (((l_c * (0x1.000000p0f - l_fx)) + (l_d * l_fx)) * l_fz));
}
fn coax_p_e2_wooded(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  return coax_p_cg_ramp(0x1.100000p4f, (0x1.100000p4f + 0x1.c00000p2f), sqrt(((l_x * l_x) + (l_z * l_z))));
}
fn coax_p_e2_clearing(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_dx: f32;
  var l_dz: f32;
  var l_r: f32;
  var l_k: f32;
  var l_edge: f32;
  var l_a: f32;
  var l_b: f32;
  l_dx = (l_x - coax_p_e2_clear_x());
  l_dz = (l_z - (-0x1.d60000p8f));
  l_r = sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
  l_k = (0x1.000000p0f / (l_r + 0x1.0624dep-10f));
  l_edge = (0x1.100000p5f * (0x1.b33334p-1f + (0x1.333334p-2f * coax_p_e2_noise((((l_dx * l_k) * 0x1.400000p1f) + 0x1.200000p3f), (((l_dz * l_k) * 0x1.400000p1f) + 0x1.800000p1f)))));
  l_a = (0x1.000000p0f - coax_p_cg_ramp((l_edge - 0x1.800000p2f), (l_edge + 0x1.000000p1f), l_r));
  if ((!coax_p_e2_near_city(l_x, l_z, 0.0f)))
  {
    return l_a;
  }
  l_b = coax_p_clearing_open(coax_p_e2_city(), l_x, l_z);
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_e2_near_brook(p_x: f32, p_z: f32) -> bool {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  return (((l_z > (-0x1.a00000p7f)) && (l_z < (-0x1.000000p5f))) && (abs((l_x - 0x1.500000p6f)) < 0x1.c00000p4f));
}
fn coax_p_e2_brook() -> coax_t37 {
  return coax_t37(0x1.500000p6f, (-0x1.b80000p5f), (-0x1.720000p7f), 0x1.400000p2f, 0x1.eb851ep-6f, 0x1.333334p-1f, 0x1.19999ap1f, 0x1.c00000p1f, 0x1.000000p4f, 0x1.19999ap-1f);
}
fn coax_p_stream_keep(p_s: coax_t37, p_x: f32, p_z: f32) -> f32 {
  var l_s: coax_t37 = p_s;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  return coax_p_clr_ramp((l_s.f6 + l_s.f7), ((l_s.f6 + l_s.f7) + 0x1.800000p1f), coax_p_stream_dist(l_s, l_x, l_z));
}
fn coax_p_e2_wet(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_b: coax_t37;
  if ((!coax_p_e2_near_brook(l_x, l_z)))
  {
    return 0.0f;
  }
  l_b = coax_p_e2_brook();
  return (0x1.000000p0f - coax_p_cg_ramp(l_b.f6, (l_b.f6 + 0x1.000000p0f), coax_p_stream_dist(l_b, l_x, l_z)));
}
fn coax_p_e2_woods_share(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_a: f32;
  var l_b: f32;
  l_a = coax_p_e2_outside(l_x, l_z);
  l_b = coax_p_e2_wooded(l_x, l_z);
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_cg_autumn(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_side: f32;
  var l_rear: f32;
  var l_wood: f32;
  var l_a: f32;
  var l_b: f32;
  var l_v: f32;
  l_side = (((l_x + (l_z * 0x1.99999ap-4f)) - 0x1.000000p0f) / 0x1.000000p4f);
  l_rear = (((-l_z) - 0x1.000000p4f) / 0x1.a00000p3f);
  l_wood = ((l_x + 0x1.800000p3f) / 0x1.400000p2f);
  l_a = select(0.0f, l_side, (l_side > 0.0f));
  l_b = (select(select(l_rear, 0x1.000000p0f, (l_rear > 0x1.000000p0f)), 0.0f, (l_rear < 0.0f)) * select(select(l_wood, 0x1.000000p0f, (l_wood > 0x1.000000p0f)), 0.0f, (l_wood < 0.0f)));
  l_v = select(l_b, l_a, (l_a > l_b));
  return select(l_v, 0x1.000000p0f, (l_v > 0x1.000000p0f));
}
fn coax_p_e2_outside(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_dx: f32;
  var l_dz: f32;
  l_dx = (l_x - 0x1.c00000p4f);
  if ((((-0x1.e00000p4f) - l_x) > l_dx))
  {
    l_dx = ((-0x1.e00000p4f) - l_x);
  }
  if ((l_dx < 0.0f))
  {
    l_dx = 0.0f;
  }
  l_dz = (l_z - 0x1.000000p3f);
  if ((((-0x1.500000p5f) - l_z) > l_dz))
  {
    l_dz = ((-0x1.500000p5f) - l_z);
  }
  if ((l_dz < 0.0f))
  {
    l_dz = 0.0f;
  }
  return coax_p_cg_ramp(0.0f, 0x1.c00000p4f, sqrt(((l_dx * l_dx) + (l_dz * l_dz))));
}
fn coax_p_e2_wild(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_rd: f32;
  var l_valley: f32;
  var l_hills: f32;
  var l_ends: f32;
  var l_ring: f32;
  var l_bowl: f32;
  var l_wild: f32;
  var l_near: f32;
  var l_floor_y: f32;
  l_rd = coax_p_e2_road_dist(l_x, l_z);
  l_valley = (0x1.e00000p3f * (0x1.000000p0f - exp((((-l_rd) * l_rd) / 0x1.b58000p12f))));
  l_hills = (((coax_p_e2_noise(((l_x / 0x1.800000p5f) + 0x1.533334p2f), ((l_z / 0x1.800000p5f) - 0x1.0cccccp1f)) - 0x1.000000p-1f) * 0x1.400000p3f) + ((coax_p_e2_noise(((l_x / 0x1.300000p4f) + 0x1.ecccccp2f), ((l_z / 0x1.300000p4f) - 0x1.a66666p1f)) - 0x1.000000p-1f) * 0x1.800000p1f));
  l_ends = ((coax_p_cg_ramp((-0x1.040000p9f), (-0x1.310000p9f), l_z) * 0x1.600000p4f) + (coax_p_cg_ramp(0x1.e00000p4f, 0x1.2c0000p6f, l_z) * 0x1.000000p4f));
  l_ring = sqrt((((l_x - coax_p_e2_clear_x()) * (l_x - coax_p_e2_clear_x())) + ((l_z - (-0x1.d60000p8f)) * (l_z - (-0x1.d60000p8f)))));
  l_bowl = (coax_p_cg_ramp(0x1.b33334p4f, 0x1.2b3334p6f, l_ring) * 0x1.200000p3f);
  l_wild = (((coax_p_e2_level(l_z) + l_valley) + (l_hills * coax_p_cg_ramp(0x1.400000p2f, 0x1.e00000p4f, l_rd))) + l_ends);
  l_near = (0x1.000000p0f - coax_p_cg_ramp(0x1.466666p5f, 0x1.61999ap6f, l_ring));
  l_floor_y = (coax_p_e2_level((-0x1.d60000p8f)) + (0x1.333334p-1f * (coax_p_e2_noise((l_x / 0x1.c00000p3f), (l_z / 0x1.c00000p3f)) - 0x1.000000p-1f)));
  return ((l_wild * (0x1.000000p0f - l_near)) + (((l_floor_y + l_bowl) + ((l_hills * 0x1.333334p-3f) * coax_p_cg_ramp(0x1.100000p5f, 0x1.100000p6f, l_ring))) * l_near));
}
fn coax_p_e2_level(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  var l_t: f32;
  l_t = (l_z + 0x1.400000p5f);
  return ((0x1.a8f5c2p-1f + (0x1.f0a3d8p-1f * coax_p_cg_ramp((-0x1.000000p4f), (-0x1.080000p5f), l_z))) + (coax_p_cg_ramp((-0x1.400000p5f), (-0x1.680000p6f), l_z) * ((0x1.200000p2f * sin((l_t * 0x1.a9fbe8p-7f))) + (0x1.000000p1f * sin((l_t * 0x1.16872cp-5f))))));
}
fn coax_p_e2_near_city(p_x: f32, p_z: f32, p_more: f32) -> bool {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_more: f32 = p_more;
  var l_m: f32;
  l_m = (0x1.00147ap6f + l_more);
  return ((abs((l_x - 0x1.100000p5f)) < l_m) && (abs((l_z - (-0x1.c80000p6f))) < l_m));
}
fn coax_p_e2_city() -> coax_t38 {
  return coax_t38(0x1.100000p5f, (-0x1.c80000p6f), 0x1.b00000p5f, 0x1.400000p3f, 0x1.000000p-2f, 0x1.c00000p4f, 0x1.800000p3f);
}
fn coax_p_e2_city_level() -> f32 {
  return coax_p_e2_level((-0x1.c80000p6f));
}
fn coax_p_clearing_floor(p_c: coax_t38, p_x: f32, p_z: f32, p_ground: f32, p_level: f32) -> f32 {
  var l_c: coax_t38 = p_c;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_ground: f32 = p_ground;
  var l_level: f32 = p_level;
  var l_inner: f32;
  var l_w: f32;
  l_inner = ((l_c.z * (0x1.000000p0f - (l_c.f4 * 0x1.000000p-1f))) - (l_c.w * 0x1.800000p-1f));
  l_w = coax_p_clr_ramp(l_inner, (l_c.z + l_c.f5), coax_p_clearing_r(l_c, l_x, l_z));
  return (l_level + ((l_ground - l_level) * l_w));
}
fn coax_p_e2_brook_level(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  return ((coax_p_e2_city_level() - 0x1.333334p-2f) + (0x1.89374cp-9f * (l_z - (-0x1.c80000p6f))));
}
fn coax_p_stream_bed(p_s: coax_t37, p_x: f32, p_z: f32, p_ground: f32, p_level: f32) -> f32 {
  var l_s: coax_t37 = p_s;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_ground: f32 = p_ground;
  var l_level: f32 = p_level;
  var l_d: f32;
  var l_bank: f32;
  var l_t: f32;
  var l_near: f32;
  l_d = coax_p_stream_dist(l_s, l_x, l_z);
  if ((l_d >= ((l_s.f6 + l_s.f7) + l_s.f8)))
  {
    return l_ground;
  }
  l_bank = (l_level + 0x1.666666p-2f);
  if ((l_d < l_s.f6))
  {
    l_t = (l_d / l_s.f6);
    return (l_level - (l_s.f9 * (0x1.000000p0f - (l_t * l_t))));
  }
  l_near = (l_level + ((l_bank - l_level) * coax_p_clr_ramp(l_s.f6, (l_s.f6 + (l_s.f7 * 0x1.000000p-1f)), l_d)));
  return (l_near + ((l_ground - l_near) * coax_p_clr_ramp((l_s.f6 + l_s.f7), ((l_s.f6 + l_s.f7) + l_s.f8), l_d)));
}
fn coax_p_cg_instance_point(p_p: vec3f, p_center: vec3f, p_vertical: f32, p_horizontal: f32, p_angle: f32) -> vec3f {
  var l_p: vec3f = p_p;
  var l_center: vec3f = p_center;
  var l_vertical: f32 = p_vertical;
  var l_horizontal: f32 = p_horizontal;
  var l_angle: f32 = p_angle;
  var l_cs: f32;
  var l_sn: f32;
  l_cs = cos(l_angle);
  l_sn = sin(l_angle);
  return coax_p_add3(l_center, coax_p_v3((((l_p.x * l_cs) + (l_p.z * l_sn)) * l_horizontal), (l_p.y * l_vertical), ((((-l_p.x) * l_sn) + (l_p.z * l_cs)) * l_horizontal)));
}
fn coax_p_e2_unpack_tint(p_c: vec4f) -> vec3f {
  var l_c: vec4f = p_c;
  var l_hi: f32;
  var l_b: f32;
  l_hi = floor((l_c.z / 0x1.000000p8f));
  l_b = (l_c.w - (0x1.000000p2f * floor((l_c.w / 0x1.000000p2f))));
  return coax_p_v3((l_hi / 0x1.fe0000p7f), ((l_c.z - (l_hi * 0x1.000000p8f)) / 0x1.fe0000p7f), select(l_b, (l_b - 0x1.000000p1f), (l_b > 0x1.800000p0f)));
}
fn coax_p_e2_inst_fade(p_c: vec4f) -> f32 {
  var l_c: vec4f = p_c;
  var l_k: f32;
  l_k = floor((l_c.w / 0x1.000000p2f));
  return select((l_k / 0x1.000000p6f), ((0x1.000000p6f - l_k) / 0x1.000000p6f), (l_k > 0x1.020000p6f));
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  var l_x: u32 = p_x;
  var l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_e2_field_cell(p_t: i32) -> f32 {
  var l_t: i32 = p_t;
  return select(select(select(0x1.000000p2f, 0x1.000000p1f, (l_t == 2i)), 0x1.000000p0f, (l_t == 1i)), 0x1.000000p-1f, (l_t == 0i));
}
fn coax_p_e2_meadow_gold(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  return (coax_p_e2_smooth(0x1.99999ap-2f, 0x1.3d70a4p-1f, coax_p_e2_noise(((l_x / 0x1.e00000p5f) + 0x1.8cccccp1f), ((l_z / 0x1.e00000p5f) - 0x1.b33334p0f))) * 0x1.333334p-2f);
}
fn coax_p_exp_height_gpu(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_t: array<f32, 192>;
  var l_h: f32;
  var l_i: i32;
  var l_o: i32;
  var l_d: f32;
  var l_u: f32;
  l_t = coax_p_exp_ground_sites();
  l_h = coax_p_cg_height(l_x, l_z);
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 32i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      l_o = (l_i * 6i);
      l_h = coax_p_exp_ground_level(l_h, l_t[l_o], l_t[(l_o + 1i)], l_t[(l_o + 2i)], l_t[(l_o + 3i)], l_t[(l_o + 4i)], l_t[(l_o + 5i)], l_x, l_z);
    }
  }
  l_d = coax_p_exp_ground_castle_distance(l_x, l_z);
  if ((l_d < 0x1.000000p2f))
  {
    l_u = select((l_d / 0x1.000000p2f), 0.0f, (l_d <= 0.0f));
    l_u = ((l_u * l_u) * (0x1.800000p1f - (0x1.000000p1f * l_u)));
    l_h = ((coax_p_cg_height(0x1.200000p4f, (-0x1.400000p4f)) * (0x1.000000p0f - l_u)) + (l_h * l_u));
  }
  return l_h;
}
fn coax_p_e2_meadow_tuft_sv(p_fx: u32, p_fz: u32) -> f32 {
  var l_fx: u32 = p_fx;
  var l_fz: u32 = p_fz;
  return ((0x1.c28f5cp-3f + (0x1.0a3d70p-2f * coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 4u)))) / 0x1.4fcce2p0f);
}
fn coax_p_e2_meadow_tuft_sh(p_fx: u32, p_fz: u32) -> f32 {
  var l_fx: u32 = p_fx;
  var l_fz: u32 = p_fz;
  return ((0x1.19999ap-1f + (0x1.666666p-2f * coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 7u)))) * 0x1.b33334p-1f);
}
fn coax_p_cg_meadow_place(p_id: u32, p_a: vec4f, p_sv: f32, p_sh: f32, p_tint: vec3f, p_autumn: f32, p_m: coax_t9, p_who: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_a: vec4f = p_a;
  var l_sv: f32 = p_sv;
  var l_sh: f32 = p_sh;
  var l_tint: vec3f = p_tint;
  var l_autumn: f32 = p_autumn;
  var l_m: coax_t9 = p_m;
  var l_who: u32 = p_who;
  var l_b: u32;
  var l_source: vec3f;
  var l_normal: u32;
  var l_kind: u32;
  var l_p: vec3f;
  var l_sway: f32;
  var l_nw: vec4f;
  var l_cs: f32;
  var l_sn: f32;
  var l_o: coax_t15;
  var l_vt: vec4f;
  var l_height: f32;
  l_b = (coax_p_wg_u32_f(l_m.y.w) + (l_id * 8u));
  l_source = coax_p_v3(bitcast<f32>(vertices[i32(l_b)]), bitcast<f32>(vertices[i32((l_b + 1u))]), bitcast<f32>(vertices[i32((l_b + 2u))]));
  l_normal = vertices[i32((l_b + 3u))];
  l_kind = ((l_normal >> u32(24u)) & 7u);
  l_p = coax_p_cg_instance_point(l_source, coax_p_v3(l_a.x, l_a.y, l_a.z), l_sv, l_sh, l_a.w);
  if ((((l_kind == 1u) || (l_kind == 2u)) || (l_kind == 3u)))
  {
    l_sway = (sin((((l_a.x * 0x1.7ae148p-2f) + (l_a.z * 0x1.ae147ap-3f)) + (coax_at_u(i32(2i)) * 0x1.19999ap0f))) + (sin(((l_a.z * 0x1.0f5c28p-1f) + (coax_at_u(i32(2i)) * 0x1.666666p-1f))) * 0x1.666666p-2f));
    l_p.x = (l_p.x + (((l_sway * 0x1.99999ap-5f) * coax_p_saturate((l_source.y * 0x1.70a3d8p-3f))) * l_sv));
  }
  l_nw = coax_p_wg_unpack4x8snorm(l_normal);
  l_cs = cos(l_a.w);
  l_sn = sin(l_a.w);
  l_o = coax_t15();
  l_o.x = coax_p_fivelab_project(l_p);
  l_o.y = l_p;
  l_o.z = coax_p_normalize3(coax_p_v3((((l_nw.x * l_cs) + (l_nw.z * l_sn)) / l_sh), (l_nw.y / l_sv), ((((-l_nw.x) * l_sn) + (l_nw.z * l_cs)) / l_sh)));
  l_o.w = coax_p_wg_unpack2x16unorm(vertices[i32((l_b + 4u))]);
  if ((l_kind == 6u))
  {
    l_o.w = coax_p_v2(((l_o.w.x * 0x1.000000p6f) - 0x1.000000p5f), ((l_o.w.y * 0x1.000000p6f) - 0x1.000000p5f));
  }
  l_vt = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 5u))]);
  l_o.f4 = vertices[i32((l_b + 6u))];
  l_o.f5 = select(select(coax_p_v3(l_vt.x, l_vt.y, l_vt.z), coax_p_mul3(coax_p_v3(l_vt.x, l_vt.y, l_vt.z), l_tint), (l_kind == 4u)), l_tint, (l_kind == 1u));
  l_o.f6 = coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(f32(l_kind), bitcast<f32>(vertices[i32((l_b + 7u))]), 0.0f);
  l_o.f8 = l_who;
  if ((l_kind == 2u))
  {
    l_height = coax_p_saturate(l_source.y);
    l_o.f5 = coax_p_mix3(l_tint, coax_p_v3((0x1.4cccccp-1f + (0x1.1eb852p-2f * l_autumn)), (0x1.c28f5cp-1f - (0x1.99999ap-3f * l_autumn)), 0x1.1eb852p-4f), l_height);
    l_o.f7 = coax_p_v3(0x1.000000p1f, l_height, 0x1.000000p0f);
  }
  return l_o;
}
fn coax_p_e2_meadow_cell(p_kit: i32) -> f32 {
  var l_kit: i32 = p_kit;
  var l_s: f32;
  l_s = coax_p_e2_meadow_share(l_kit);
  return select(sqrt((0x1.666666p-1f / (0x1.b00000p-3f * l_s))), 0.0f, (l_s <= 0.0f));
}
fn coax_p_e2_meadow_held(p_gx: u32, p_gz: u32, p_kit: i32) -> bool {
  var l_gx: u32 = p_gx;
  var l_gz: u32 = p_gz;
  var l_kit: i32 = p_kit;
  return (coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((100i + l_kit)))) < 0x1.666666p-1f);
}
fn coax_p_e2_meadow_at_x(p_gx: u32, p_gz: u32, p_kit: i32) -> f32 {
  var l_gx: u32 = p_gx;
  var l_gz: u32 = p_gz;
  var l_kit: i32 = p_kit;
  return ((-0x1.000000p15f) + (((f32(l_gx) + 0x1.99999ap-4f) + (0x1.99999ap-1f * coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((60i + l_kit)))))) * coax_p_e2_meadow_cell(l_kit)));
}
fn coax_p_e2_meadow_at_z(p_gx: u32, p_gz: u32, p_kit: i32) -> f32 {
  var l_gx: u32 = p_gx;
  var l_gz: u32 = p_gz;
  var l_kit: i32 = p_kit;
  return ((-0x1.000000p15f) + (((f32(l_gz) + 0x1.99999ap-4f) + (0x1.99999ap-1f * coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((80i + l_kit)))))) * coax_p_e2_meadow_cell(l_kit)));
}
fn coax_p_e2_meadow_seed(p_gx: u32, p_gz: u32, p_kit: i32) -> f32 {
  var l_gx: u32 = p_gx;
  var l_gz: u32 = p_gz;
  var l_kit: i32 = p_kit;
  return ((coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((40i + l_kit)))) * 0x1.840000p6f) + 0x1.000000p-1f);
}
fn coax_p_cg_hash(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_v: f32;
  l_v = (sin(((l_a * 0x1.fc6666p6f) + (l_b * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
}
fn coax_p_e2_meadow_fade(p_kit: i32, p_d: f32) -> f32 {
  var l_kit: i32 = p_kit;
  var l_d: f32 = p_d;
  var l_r: f32;
  l_r = coax_p_e2_meadow_reach(l_kit);
  return (0x1.000000p0f - coax_p_cg_ramp((0x1.800000p-1f * l_r), l_r, l_d));
}
fn coax_p_e2_meadow_piece(p_kit: i32, p_h: f32, p_autumn: f32) -> coax_t32 {
  var l_kit: i32 = p_kit;
  var l_h: f32 = p_h;
  var l_autumn: f32 = p_autumn;
  var l_sz: f32;
  var l_p: coax_t32;
  var l_s: f32;
  var l_s_6: f32;
  var l_s_7: f32;
  var l_g: f32;
  var l_s_9: f32;
  var l_s_10: f32;
  l_sz = coax_p_cg_hash(l_h, 0x1.e00000p3f);
  l_p = coax_t32();
  l_p.x = l_kit;
  if (((l_kit >= 25i) && (l_kit <= 27i)))
  {
    l_s = (0x1.eb851ep-4f + (l_sz * 0x1.0a3d70p-3f));
    l_p.y = l_s;
    l_p.z = l_s;
    l_p.f4 = array<f32, 3>(0x1.99999ap-2f, 0x1.800000p-1f, 0x1.99999ap-5f);
  }
  else
  {
    if ((l_kit == 31i))
    {
      l_s_6 = (0x1.333334p-2f + (l_sz * 0x1.333334p-2f));
      l_p.y = l_s_6;
      l_p.z = l_s_6;
      l_p.f4 = array<f32, 3>(0x1.70a3d8p-2f, 0x1.99999ap-1f, 0x1.47ae14p-4f);
    }
    else
    {
      if (((l_kit >= 20i) && (l_kit <= 22i)))
      {
        l_s_7 = (0x1.000000p-2f + (l_sz * 0x1.99999ap-2f));
        l_p.y = l_s_7;
        l_p.z = l_s_7;
        l_p.w = (0x1.99999ap-4f * l_s_7);
        l_g = coax_p_cg_hash(l_h, 0x1.200000p4f);
        l_p.f4 = array<f32, 3>((0x1.333334p-1f + (0x1.47ae14p-4f * l_g)), (0x1.3d70a4p-1f + (0x1.eb851ep-5f * l_g)), 0x1.000000p-1f);
      }
      else
      {
        if (((l_kit == 29i) || (l_kit == 30i)))
        {
          l_s_9 = (0x1.333334p-2f + (l_sz * 0x1.000000p-2f));
          l_p.y = l_s_9;
          l_p.z = l_s_9;
          l_p.f4 = array<f32, 3>(0x1.70a3d8p-2f, 0x1.99999ap-1f, 0x1.47ae14p-4f);
          if ((l_autumn > 0x1.99999ap-2f))
          {
            l_p.f4 = array<f32, 3>(0x1.000000p0f, 0x1.333334p-2f, 0x1.eb851ep-6f);
            if ((coax_p_cg_hash(l_h, 0x1.100000p4f) >= 0x1.000000p-1f))
            {
              l_p.f4[1i] = 0x1.c28f5cp-2f;
              l_p.f4[2i] = 0x1.99999ap-5f;
            }
          }
        }
        else
        {
          l_s_10 = (0x1.000000p-1f + (l_sz * 0x1.333334p-2f));
          l_p.y = (l_s_10 * 0x1.800000p-1f);
          l_p.z = l_s_10;
          l_p.f4 = array<f32, 3>(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
        }
      }
    }
  }
  return l_p;
}
fn coax_p_cg_sky_ray(p_x: f32, p_y: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_fy: f32;
  var l_fz: f32;
  var l_qx: f32;
  var l_qy: f32;
  l_fy = (0x1.99999ap-2f / sqrt(0x1.38a3d8p3f));
  l_fz = ((-0x1.8cccccp1f) / sqrt(0x1.38a3d8p3f));
  l_qx = ((l_x * 0x1.e00000p10f) / (0x1.0e0000p10f * 0x1.c7ae14p0f));
  l_qy = (((l_y * 0x1.000000p-1f) - 0x1.47ae14p-6f) / 0x1.c7ae14p-1f);
  return coax_p_v3(l_qx, (l_fy - (l_qy * l_fz)), (l_fz + (l_qy * l_fy)));
}
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
}
fn coax_p_fivelab_gait_share(p_code: f32, p_j: u32) -> f32 {
  var l_code: f32 = p_code;
  var l_j: u32 = p_j;
  var l_c: f32;
  var l_low: f32;
  var l_up: f32;
  var l_m: f32;
  if ((l_code < 0x1.800000p0f))
  {
    return l_code;
  }
  l_c = (l_code - 0x1.000000p1f);
  l_low = floor((l_c / 0x1.000000p8f));
  l_up = (l_c - (l_low * 0x1.000000p8f));
  l_m = coax_p_fivelab_upper(l_j);
  return (((l_up / 0x1.fe0000p7f) * l_m) + ((l_low / 0x1.fe0000p7f) * (0x1.000000p0f - l_m)));
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
fn coax_p_fivelab_rows(p_row: u32, p_j: u32, p_arm: bool) -> coax_t35 {
  var l_row: u32 = p_row;
  var l_j: u32 = p_j;
  var l_arm: bool = p_arm;
  var l_r: u32;
  var l_m: coax_t35;
  l_r = (l_row + (l_j * 3u));
  l_m = coax_t35();
  l_m.x = bake[i32(l_r)];
  l_m.y = bake[i32((l_r + 1u))];
  l_m.z = bake[i32((l_r + 2u))];
  if (l_arm)
  {
    l_m = coax_p_fivelab_arm(l_row, l_j, l_m);
  }
  return l_m;
}
fn coax_p_fivelab_mix4(p_a: vec4f, p_b: vec4f, p_t: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  var l_t: f32 = p_t;
  return coax_p_v4((l_a.x + ((l_b.x - l_a.x) * l_t)), (l_a.y + ((l_b.y - l_a.y) * l_t)), (l_a.z + ((l_b.z - l_a.z) * l_t)), (l_a.w + ((l_b.w - l_a.w) * l_t)));
}
fn coax_p_paint_env_c3(p_c: i32) -> vec3f {
  var l_c: i32 = p_c;
  return coax_p_wg_at3((228i + l_c));
}
fn coax_p_paint_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_i = coax_p_v2(floor(l_p.x), floor(l_p.y));
  l_f = coax_p_v2(coax_p_fract(l_p.x), coax_p_fract(l_p.y));
  l_f = coax_p_v2(((l_f.x * l_f.x) * (0x1.800000p1f - (0x1.000000p1f * l_f.x))), ((l_f.y * l_f.y) * (0x1.800000p1f - (0x1.000000p1f * l_f.y))));
  l_a = coax_p_verse_style_hash(l_i.x, l_i.y);
  l_b = coax_p_verse_style_hash((l_i.x + 0x1.000000p0f), l_i.y);
  l_c = coax_p_verse_style_hash(l_i.x, (l_i.y + 0x1.000000p0f));
  l_d = coax_p_verse_style_hash((l_i.x + 0x1.000000p0f), (l_i.y + 0x1.000000p0f));
  return coax_p_mix(coax_p_mix(l_a, l_b, l_f.x), coax_p_mix(l_c, l_d, l_f.x), l_f.y);
}
fn coax_p_paint_cloud_fbm(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return ((((coax_p_paint_noise(l_p) * 0x1.110cb2p-1f) + (coax_p_paint_noise(coax_p_v2(((l_p.x * 0x1.03d70ap1f) + 0x1.c00000p2f), ((l_p.y * 0x1.03d70ap1f) - 0x1.800000p1f))) * 0x1.1119cep-2f)) + (coax_p_paint_noise(coax_p_v2(((l_p.x * 0x1.070a3ep2f) - 0x1.400000p2f), ((l_p.y * 0x1.070a3ep2f) + 0x1.600000p3f))) * 0x1.10ff98p-3f)) + (coax_p_paint_noise(coax_p_v2(((l_p.x * 0x1.09eb86p3f) + 0x1.a00000p3f), (l_p.y * 0x1.09eb86p3f))) * 0x1.113404p-4f));
}
fn coax_p_paint_env_c(p_c: i32) -> f32 {
  var l_c: i32 = p_c;
  return coax_at_u(i32((228i + l_c)));
}
fn coax_p_transfer_linear(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return select(coax_pow_bg_vs(((l_x + 0x1.c28f5cp-5f) / 0x1.0e147ap0f), 0x1.333334p1f), (coax_p_fmax(l_x, 0.0f) / 0x1.9d70a4p3f), (l_x <= 0x1.4b5dccp-5f));
}
fn coax_p_render_filter_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t36 {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_f: coax_t40;
  var l_rs_sum: vec4f;
  var l_rs_i: i32;
  var l_rs_offset: f32;
  var l_rs_at: vec2f;
  l_rs_f = coax_p_render_footprint(i32(textureDimensions(sheets).x), i32(textureDimensions(sheets).y), l_dx.x, l_dx.y, l_dy.x, l_dy.y, l_bias);
  if ((!l_rs_f.x))
  {
    return coax_t36(false, vec4f());
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
  return coax_t36(true, coax_p_scale4(l_rs_sum, (0x1.000000p0f / f32(l_rs_f.y))));
}
fn coax_p_paint_env_sun_dir() -> vec3f {
  return coax_p_wg_at3(216i);
}
fn coax_p_e2_ground_weights(p_x: f32, p_z: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_w: vec4f;
  var l_c: f32;
  l_w = coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0.0f);
  l_c = coax_p_e2_ground_layer(l_x, l_z, 0i);
  l_w = coax_p_v4(coax_p_aethyr_terrain_over(l_w.x, l_c, 0.0f), coax_p_aethyr_terrain_over(l_w.y, l_c, 0x1.000000p0f), coax_p_aethyr_terrain_over(l_w.z, l_c, 0.0f), coax_p_aethyr_terrain_over(l_w.w, l_c, 0.0f));
  l_c = coax_p_e2_ground_layer(l_x, l_z, 1i);
  l_w = coax_p_v4(coax_p_aethyr_terrain_over(l_w.x, l_c, 0.0f), coax_p_aethyr_terrain_over(l_w.y, l_c, 0.0f), coax_p_aethyr_terrain_over(l_w.z, l_c, 0x1.000000p0f), coax_p_aethyr_terrain_over(l_w.w, l_c, 0.0f));
  l_c = coax_p_e2_ground_layer(l_x, l_z, 2i);
  l_w = coax_p_v4(coax_p_aethyr_terrain_over(l_w.x, l_c, 0.0f), coax_p_aethyr_terrain_over(l_w.y, l_c, 0.0f), coax_p_aethyr_terrain_over(l_w.z, l_c, 0.0f), coax_p_aethyr_terrain_over(l_w.w, l_c, 0x1.000000p0f));
  return l_w;
}
fn coax_p_e2_ground_material(p_x: f32, p_z: f32, p_m: i32, p_w: vec4f) -> vec3f {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_m: i32 = p_m;
  var l_w: vec4f = p_w;
  var l_b: i32;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_f: vec4f;
  var l_g: vec4f;
  var l_h: vec4f;
  var l_t: f32;
  var l_gain: f32;
  l_b = (2i + (l_m * 5i));
  l_a = ground[l_b];
  l_c = ground[(l_b + 1i)];
  l_f = ground[(l_b + 2i)];
  l_g = ground[(l_b + 3i)];
  l_h = ground[(l_b + 4i)];
  l_t = coax_p_aethyr_terrain_blend_t(l_x, l_z, l_f.x, l_f.y, l_f.z, l_f.w, l_g.x, l_g.y);
  if ((l_a.w > 0.0f))
  {
    l_t = (l_t + (l_a.w * coax_p_aethyr_terrain_share(coax_p_e2_ground_pick(l_w, l_c.w), coax_p_e2_ground_pick(l_w, l_h.z))));
  }
  l_gain = coax_p_aethyr_terrain_gain(l_x, l_z, l_g.z, l_g.w, l_h.x, l_h.y);
  return coax_p_v3(coax_p_aethyr_terrain_decode(l_a.x, l_c.x, l_t, l_gain), coax_p_aethyr_terrain_decode(l_a.y, l_c.y, l_t, l_gain), coax_p_aethyr_terrain_decode(l_a.z, l_c.z, l_t, l_gain));
}
fn coax_p_cg_road_x(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  var l_b: f32;
  l_b = coax_p_cg_ramp((-0x1.a00000p3f), (-0x1.300000p4f), l_z);
  return ((coax_p_cg_path_x(l_z) * (0x1.000000p0f - l_b)) + ((0x1.a66666p1f + (0x1.333334p-2f * sin(((l_z + 0x1.500000p4f) * 0x1.000000p-2f)))) * l_b));
}
fn coax_p_paint_env_cloud(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_flags: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  l_flags = coax_p_paint_env_w(6i);
  if (((l_flags - (0x1.000000p1f * floor((l_flags * 0x1.000000p-1f)))) < 0x1.000000p-1f))
  {
    return 0x1.000000p0f;
  }
  l_a = coax_p_paint_env_c(53i);
  l_b = coax_p_paint_env_c(54i);
  l_c = ((coax_p_paint_noise(coax_p_v2(((l_x * l_a) + 0x1.9cccccp3f), ((l_z * l_a) - 0x1.b33334p0f))) * 0x1.666666p-1f) + (coax_p_paint_noise(coax_p_v2((l_x * l_b), (l_z * l_b))) * 0x1.333334p-2f));
  return (0x1.000000p0f - ((0x1.000000p0f - coax_p_paint_env_c(52i)) * coax_p_smoothstep(coax_p_paint_env_c(56i), coax_p_paint_env_c(57i), l_c)));
}
fn coax_p_aethyr_bake_inside(p_x: f32, p_edge: f32, p_span: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_edge: f32 = p_edge;
  var l_span: f32 = p_span;
  if (((l_x < l_edge) || (l_x > (l_edge + l_span))))
  {
    return 0.0f;
  }
  return 0x1.000000p0f;
}
fn coax_p_aethyr_bake_index(p_g: f32, p_n: f32) -> f32 {
  var l_g: f32 = p_g;
  var l_n: f32 = p_n;
  return coax_p_clampf(floor(l_g), 0.0f, (l_n - 0x1.000000p0f));
}
fn coax_p_aethyr_bake_fraction(p_g: f32, p_i: f32) -> f32 {
  var l_g: f32 = p_g;
  var l_i: f32 = p_i;
  return coax_p_clampf((l_g - l_i), 0.0f, 0x1.000000p0f);
}
fn coax_p_e2_ground_at(p_k: i32) -> f32 {
  var l_k: i32 = p_k;
  var l_v: vec4f;
  var l_r: i32;
  l_v = ground[(22i + (l_k / 4i))];
  l_r = (l_k % 4i);
  return select(select(select(l_v.w, l_v.z, (l_r == 2i)), l_v.y, (l_r == 1i)), l_v.x, (l_r == 0i));
}
fn coax_p_aethyr_bake_bilerp(p_a: f32, p_b: f32, p_c: f32, p_d: f32, p_fx: f32, p_fz: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_c: f32 = p_c;
  var l_d: f32 = p_d;
  var l_fx: f32 = p_fx;
  var l_fz: f32 = p_fz;
  return coax_p_aethyr_terrain_mix(coax_p_aethyr_terrain_mix(l_a, l_b, l_fx), coax_p_aethyr_terrain_mix(l_c, l_d, l_fx), l_fz);
}
fn coax_p_aethyr_bake_rise(p_h: f32, p_rise: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_rise: f32 = p_rise;
  return coax_p_smoothstep(0.0f, l_rise, l_h);
}
fn coax_p_aethyr_bake_ao(p_ao: f32, p_s: f32) -> f32 {
  var l_ao: f32 = p_ao;
  var l_s: f32 = p_s;
  return coax_p_aethyr_terrain_mix(l_ao, 0x1.000000p0f, l_s);
}
fn coax_p_paint_env_hemisphere(p_n: vec3f) -> vec3f {
  var l_n: vec3f = p_n;
  var l_t: f32;
  var l_h: vec3f;
  l_t = coax_p_saturate(((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f));
  l_h = coax_p_paint_env_c3(4i);
  return select(coax_p_mix3(l_h, coax_p_paint_env_c3(0i), ((l_t * 0x1.000000p1f) - 0x1.000000p0f)), coax_p_mix3(coax_p_paint_env_c3(8i), l_h, (l_t * 0x1.000000p1f)), (l_t < 0x1.000000p-1f));
}
fn coax_p_paint_env_irradiance(p_n: vec3f, p_direct: f32, p_occlusion: f32, p_visibility: f32) -> vec3f {
  var l_n: vec3f = p_n;
  var l_direct: f32 = p_direct;
  var l_occlusion: f32 = p_occlusion;
  var l_visibility: f32 = p_visibility;
  return coax_p_add3(coax_p_scale3(coax_p_paint_env_hemisphere(l_n), l_occlusion), coax_p_scale3(coax_p_paint_env_sun_colour(), (l_direct * l_visibility)));
}
fn coax_p_paint_env_sun_colour() -> vec3f {
  return coax_p_wg_at3(220i);
}
fn coax_p_paint_soil(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_cell: vec2f;
  var l_local: vec2f;
  var l_first: f32;
  var l_second: f32;
  var l_j: i32;
  var l_i: i32;
  var l_x: f32;
  var l_y: f32;
  var l_dx: f32;
  var l_dy: f32;
  var l_d: f32;
  l_cell = coax_p_v2(floor((l_p.x * 0x1.c00000p1f)), floor((l_p.y * 0x1.c00000p1f)));
  l_local = coax_p_v2(coax_p_fract((l_p.x * 0x1.c00000p1f)), coax_p_fract((l_p.y * 0x1.c00000p1f)));
  l_first = 0x1.400000p3f;
  l_second = 0x1.400000p3f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 3i;
    for (l_j = coax_from; l_j < coax_to; l_j = l_j + 1i)
    {
      {
        let coax_from: i32 = 0i;
        let coax_to: i32 = 3i;
        for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
        {
          l_x = (f32(l_i) - 0x1.000000p0f);
          l_y = (f32(l_j) - 0x1.000000p0f);
          l_dx = (((l_x + 0x1.333334p-3f) + (0x1.666666p-1f * coax_p_verse_style_hash((l_cell.x + l_x), (l_cell.y + l_y)))) - l_local.x);
          l_dy = (((l_y + 0x1.333334p-3f) + (0x1.666666p-1f * coax_p_verse_style_hash(((l_cell.x + l_x) + 0x1.100000p4f), ((l_cell.y + l_y) + 0x1.800000p1f)))) - l_local.y);
          l_d = ((l_dx * l_dx) + (l_dy * l_dy));
          if ((l_d < l_first))
          {
            l_second = l_first;
            l_first = l_d;
          }
          else
          {
            if ((l_d < l_second))
            {
              l_second = l_d;
            }
          }
        }
      }
    }
  }
  return ((0x1.b851ecp-1f + (0x1.1eb852p-3f * coax_p_smoothstep(0.0f, 0x1.99999ap-5f, (l_second - l_first)))) * (0x1.e147aep-1f + (0x1.eb851ep-5f * coax_p_paint_noise(coax_p_v2((l_p.x * 0x1.000000p1f), (l_p.y * 0x1.000000p1f))))));
}
fn coax_p_paint_env_atmosphere(p_colour: vec3f, p_eye: vec3f, p_p: vec3f) -> vec3f {
  var l_colour: vec3f = p_colour;
  var l_eye: vec3f = p_eye;
  var l_p: vec3f = p_p;
  var l_d: vec3f;
  var l_ray: vec3f;
  var l_sun: vec3f;
  var l_mie: f32;
  var l_air: vec3f;
  var l_trans: f32;
  l_d = coax_p_sub3(l_p, l_eye);
  l_ray = coax_p_normalize3(coax_p_add3(l_d, coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  l_sun = coax_p_paint_env_sun_dir();
  l_mie = coax_p_fmin(coax_p_paint_phase(dot(l_ray, l_sun), coax_p_paint_env_c(43i)), coax_p_paint_env_c(47i));
  l_air = coax_p_add3(coax_p_paint_env_c3(36i), coax_p_scale3(coax_p_paint_env_c3(40i), (l_mie * coax_p_paint_env_c(39i))));
  l_trans = exp(((-coax_p_paint_env_air_depth(l_eye, l_p)) * coax_p_paint_env_haze_window(coax_p_wg_length3(l_d))));
  return coax_p_add3(coax_p_scale3(l_colour, l_trans), coax_p_scale3(l_air, (0x1.000000p0f - l_trans)));
}
fn coax_p_paint_env_air_depth(p_eye: vec3f, p_p: vec3f) -> f32 {
  var l_eye: vec3f = p_eye;
  var l_p: vec3f = p_p;
  var l_ray: vec3f;
  var l_distance: f32;
  var l_density: f32;
  var l_falloff: f32;
  var l_i: i32;
  var l_height: f32;
  l_ray = coax_p_sub3(l_p, l_eye);
  l_distance = coax_p_wg_length3(l_ray);
  l_density = 0.0f;
  l_falloff = coax_p_paint_env_c(45i);
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 4i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      l_height = (l_eye.y + ((l_ray.y * (f32(l_i) + 0x1.000000p-1f)) * 0x1.000000p-2f));
      l_density = (l_density + exp(((-coax_p_fmax(l_height, 0.0f)) / l_falloff)));
    }
  }
  return (((coax_p_fmax((l_distance - coax_p_paint_env_c(46i)), 0.0f) * coax_p_paint_env_c(44i)) * l_density) * 0x1.000000p-2f);
}
fn coax_p_fivelab_dye(p_t: vec3f, p_dye: vec4f) -> vec3f {
  var l_t: vec3f = p_t;
  var l_dye: vec4f = p_dye;
  var l_green: bool;
  var l_share: f32;
  var l_hi: f32;
  var l_lo: f32;
  var l_sat: f32;
  var l_light: f32;
  var l_mask: f32;
  var l_usual: f32;
  var l_dyed: vec3f;
  if (((l_dye.w < 0x1.000000p1f) || (l_dye.w >= 0x1.800000p2f)))
  {
    return l_t;
  }
  l_green = (l_dye.w >= 0x1.000000p2f);
  l_share = coax_p_clampf((l_dye.w - select(0x1.000000p1f, 0x1.000000p2f, l_green)), 0.0f, 0x1.000000p0f);
  l_hi = coax_p_fmax(l_t.x, coax_p_fmax(l_t.y, l_t.z));
  l_lo = coax_p_fmin(l_t.x, coax_p_fmin(l_t.y, l_t.z));
  l_sat = ((l_hi - l_lo) / coax_p_fmax(l_hi, 0x1.a36e2ep-14f));
  l_light = coax_p_verse_style_luma(l_t.x, l_t.y, l_t.z);
  l_mask = (coax_p_smoothstep(0x1.eb851ep-4f, 0x1.333334p-2f, l_hi) * (0x1.000000p0f - coax_p_smoothstep(0x1.c28f5cp-3f, 0x1.ae147ap-2f, l_sat)));
  l_usual = 0x1.ae147ap-2f;
  if (l_green)
  {
    l_mask = coax_p_smoothstep(0x1.47ae14p-6f, 0x1.eb851ep-5f, (l_t.y - coax_p_fmax(l_t.x, l_t.z)));
    l_usual = 0x1.70a3d8p-4f;
  }
  l_dyed = coax_p_scale3(coax_p_v3(l_dye.x, l_dye.y, l_dye.z), coax_p_clampf((l_light / l_usual), 0.0f, 0x1.99999ap0f));
  return coax_p_mix3(l_t, l_dyed, (l_mask * l_share));
}
fn coax_p_render_filter_layer8_grad_1(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t36 {
  var l_uv: vec2f = p_uv;
  var l_layer: i32 = p_layer;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_bias: f32 = p_bias;
  var l_rs_f: coax_t40;
  var l_rs_sum: vec4f;
  var l_rs_i: i32;
  var l_rs_offset: f32;
  var l_rs_at: vec2f;
  l_rs_f = coax_p_render_footprint(i32(textureDimensions(data).x), i32(textureDimensions(data).y), l_dx.x, l_dx.y, l_dy.x, l_dy.y, l_bias);
  if ((!l_rs_f.x))
  {
    return coax_t36(false, vec4f());
  }
  l_rs_sum = vec4f();
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = l_rs_f.y;
    for (l_rs_i = coax_from; l_rs_i < coax_to; l_rs_i = l_rs_i + 1i)
    {
      l_rs_offset = ((f32(l_rs_i) + 0x1.000000p-1f) - (0x1.000000p-1f * f32(l_rs_f.y)));
      l_rs_at = coax_p_add2(l_uv, coax_p_scale2(coax_p_v2(l_rs_f.z, l_rs_f.w), l_rs_offset));
      l_rs_sum = coax_p_add4(l_rs_sum, textureSampleLevel(data, sheet, l_rs_at, l_layer, l_rs_f.f4));
    }
  }
  return coax_t36(true, coax_p_scale4(l_rs_sum, (0x1.000000p0f / f32(l_rs_f.y))));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_cm_kk(p_t: vec3f, p_h: vec3f, p_shift: f32, p_n: vec3f, p_exponent: f32) -> f32 {
  var l_t: vec3f = p_t;
  var l_h: vec3f = p_h;
  var l_shift: f32 = p_shift;
  var l_n: vec3f = p_n;
  var l_exponent: f32 = p_exponent;
  var l_ts: vec3f;
  var l_th: f32;
  var l_s: f32;
  l_ts = coax_p_normalize3(coax_p_add3(l_t, coax_p_scale3(l_n, l_shift)));
  l_th = dot(l_ts, l_h);
  l_s = sqrt(coax_p_fmax(0.0f, (0x1.000000p0f - (l_th * l_th))));
  return (coax_pow_bg_vs(l_s, l_exponent) * coax_p_smoothstep((-0x1.000000p0f), 0.0f, l_th));
}
fn coax_p_shade_sign(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return select(0x1.000000p0f, (-0x1.000000p0f), (l_x < 0.0f));
}
fn coax_p_transfer_srgb(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return select(((0x1.0e147ap0f * coax_pow_bg_vs(l_x, (0x1.000000p0f / 0x1.333334p1f))) - 0x1.c28f5cp-5f), (coax_p_fmax(l_x, 0.0f) * 0x1.9d70a4p3f), (l_x <= 0x1.9a5c38p-9f));
}
fn coax_p_portal_fbm(p_q: vec2f) -> f32 {
  var l_q: vec2f = p_q;
  var l_p: vec2f;
  var l_v: f32;
  var l_a: f32;
  var l_k: i32;
  l_p = l_q;
  l_v = 0.0f;
  l_a = 0x1.000000p-1f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 4i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_v = (l_v + (l_a * coax_p_portal_noise(l_p)));
      l_p = coax_p_v2(((l_p.x * 0x1.03d70ap1f) + 0x1.b33334p0f), ((l_p.y * 0x1.03d70ap1f) + 0x1.266666p3f));
      l_a = (l_a * 0x1.000000p-1f);
    }
  }
  return (l_v / 0x1.e00000p-1f);
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
fn coax_p_stepf(p_edge: f32, p_x: f32) -> f32 {
  var l_edge: f32 = p_edge;
  var l_x: f32 = p_x;
  if ((l_x < l_edge))
  {
    return 0.0f;
  }
  return 0x1.000000p0f;
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
fn coax_p_shade_normal_unpack(p_p: vec2f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_n: vec3f;
  var l_t: f32;
  l_n = coax_p_v3(l_p.x, l_p.y, ((0x1.000000p0f - abs(l_p.x)) - abs(l_p.y)));
  l_t = coax_p_saturate((-l_n.z));
  l_n.x = (l_n.x - (coax_p_shade_sign(l_n.x) * l_t));
  l_n.y = (l_n.y - (coax_p_shade_sign(l_n.y) * l_t));
  return coax_p_normalize3(l_n);
}
fn coax_p_shade_view_point(p_pixel: vec2f, p_z: f32, p_eye: vec3f, p_forward: vec3f, p_right: vec3f, p_up: vec3f, p_focal: f32, p_centre: vec2f) -> vec3f {
  var l_pixel: vec2f = p_pixel;
  var l_z: f32 = p_z;
  var l_eye: vec3f = p_eye;
  var l_forward: vec3f = p_forward;
  var l_right: vec3f = p_right;
  var l_up: vec3f = p_up;
  var l_focal: f32 = p_focal;
  var l_centre: vec2f = p_centre;
  return coax_p_add3(l_eye, coax_p_scale3(coax_p_add3(l_forward, coax_p_add3(coax_p_scale3(l_right, ((l_pixel.x - l_centre.x) / l_focal)), coax_p_scale3(l_up, ((l_centre.y - l_pixel.y) / l_focal)))), l_z));
}
fn coax_p_paint_env_film(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_f: vec3f;
  l_f = coax_p_film_exposed(coax_p_film_rgb(l_c.x, l_c.y, l_c.z), coax_p_paint_env_c(68i));
  return coax_p_v3(l_f.x, l_f.y, l_f.z);
}
fn coax_p_verse_style_saturation(p_c: f32, p_lum: f32, p_amount: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_lum: f32 = p_lum;
  var l_amount: f32 = p_amount;
  return coax_p_st_max(0.0f, (l_lum + ((l_c - l_lum) * l_amount)));
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
fn coax_p_e2_clear_x() -> f32 {
  return coax_p_e2_road_x((-0x1.d60000p8f));
}
fn coax_p_clearing_open(p_c: coax_t38, p_x: f32, p_z: f32) -> f32 {
  var l_c: coax_t38 = p_c;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_e: f32;
  l_e = coax_p_clearing_edge(l_c, l_x, l_z);
  return (0x1.000000p0f - coax_p_clr_ramp((l_e - (l_c.w * 0x1.800000p-1f)), (l_e + (l_c.w * 0x1.000000p-2f)), coax_p_clearing_r(l_c, l_x, l_z)));
}
fn coax_p_stream_dist(p_s: coax_t37, p_x: f32, p_z: f32) -> f32 {
  var l_s: coax_t37 = p_s;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_zc: f32;
  var l_slope: f32;
  var l_across: f32;
  var l_along: f32;
  l_zc = select(select(l_z, l_s.z, (l_z < l_s.z)), l_s.y, (l_z > l_s.y));
  l_slope = ((l_s.w * l_s.f4) * cos(((l_zc * l_s.f4) + l_s.f5)));
  l_across = (abs((l_x - coax_p_stream_x(l_s, l_zc))) / sqrt((0x1.000000p0f + (l_slope * l_slope))));
  l_along = (l_z - l_zc);
  return sqrt(((l_across * l_across) + (l_along * l_along)));
}
fn coax_p_clr_ramp(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = ((l_x - l_a) / (l_b - l_a));
  l_t = select(select(l_t, 0x1.000000p0f, (l_t > 0x1.000000p0f)), 0.0f, (l_t < 0.0f));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_clearing_r(p_c: coax_t38, p_x: f32, p_z: f32) -> f32 {
  var l_c: coax_t38 = p_c;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_dx: f32;
  var l_dz: f32;
  l_dx = (l_x - l_c.x);
  l_dz = (l_z - l_c.y);
  return sqrt(((l_dx * l_dx) + (l_dz * l_dz)));
}
fn coax_p_exp_ground_sites() -> array<f32, 192> {
  return array<f32, 192>((-0x1.200000p4f), (-0x1.700000p4f), 0x1.400000p3f, 0x1.000000p2f, 0x1.59999ap3f, 0x1.066666p2f, 0x1.200000p4f, (-0x1.400000p4f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, (-0x1.900000p5f), 0x1.900000p4f, 0x1.733334p1f, 0x1.600000p4f, 0x1.c00000p2f, 0x1.8cccccp2f, 0x1.180000p5f, 0x1.e00000p4f, 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, (-0x1.400000p4f), 0x1.900000p4f, 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, (-0x1.680000p5f), 0x1.600000p5f, 0x1.400000p3f, 0x1.000000p2f, 0x1.e00000p2f, 0x1.0cccccp2f, (-0x1.300000p4f), 0x1.680000p5f, 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, (-0x1.400000p2f), (-0x1.700000p4f), 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, 0x1.e00000p3f, (-0x1.400000p2f), 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, 0x1.800000p5f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.d00000p5f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.100000p6f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.380000p6f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.800000p5f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.d00000p5f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.100000p6f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.380000p6f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.800000p5f, 0x1.f80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, (-0x1.300000p5f), (-0x1.b00000p5f), 0x1.51999ap3f, 0x1.6c6666p5f, 0x1.923334p6f, 0x1.51999ap3f, 0x1.900000p5f, (-0x1.400000p4f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.200000p4f, (-0x1.a00000p5f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.900000p5f, (-0x1.a00000p5f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.280000p6f, (-0x1.400000p4f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.280000p6f, (-0x1.a00000p5f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.200000p4f, (-0x1.280000p6f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.900000p5f, (-0x1.280000p6f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.280000p6f, (-0x1.280000p6f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.200000p5f, (-0x1.100000p5f), 0x1.400000p3f, 0x1.000000p2f, 0x1.59999ap3f, 0x1.066666p2f, (-0x1.400000p4f), (-0x1.400000p2f), 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.a451ecp4f, 0x1.700000p5f, 0x1.e570a4p3f, 0x1.e570a4p3f, 0x1.a66666p2f, 0x1.a66666p2f, 0x1.5947aep3f, 0x1.20999ap6f, 0x1.eb0a3ep4f, 0x1.eb0a3ep4f, 0x1.333334p3f, 0x1.333334p3f, 0x1.df999ap5f, 0x1.3a0000p6f, 0x1.c9999ap3f, 0x1.c9999ap3f, 0x1.d9999ap1f, 0x1.a00000p1f);
}
fn coax_p_cg_height(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_away: f32;
  var l_h: f32;
  var l_fade: f32;
  l_away = sqrt(((l_x * l_x) + (l_z * l_z)));
  if ((l_away < 0x1.000000p1f))
  {
    return 0.0f;
  }
  l_h = ((0x1.000000p2f * exp((-((((l_x + 0x1.600000p3f) * (l_x + 0x1.600000p3f)) / 0x1.b80000p6f) + (((l_z + 0x1.600000p4f) * (l_z + 0x1.600000p4f)) / 0x1.7c0000p6f))))) + (0x1.733334p2f * exp((-((((l_x - 0x1.100000p4f) * (l_x - 0x1.100000p4f)) / 0x1.400000p7f) + (((l_z + 0x1.080000p5f) * (l_z + 0x1.080000p5f)) / 0x1.680000p7f))))));
  l_h = (l_h + (0x1.666666p3f * exp((-((((l_x + 0x1.000000p3f) * (l_x + 0x1.000000p3f)) / 0x1.e00000p6f) + (((l_z + 0x1.100000p5f) * (l_z + 0x1.100000p5f)) / 0x1.180000p6f))))));
  l_fade = select(0x1.000000p0f, ((l_away - 0x1.000000p1f) / 0x1.800000p1f), (l_away < 0x1.400000p2f));
  return coax_p_cg_road_ground(l_x, l_z, (l_h * l_fade));
}
fn coax_p_exp_ground_level(p_h: f32, p_sx: f32, p_sz: f32, p_left: f32, p_right: f32, p_back: f32, p_front: f32, p_x: f32, p_z: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_sx: f32 = p_sx;
  var l_sz: f32 = p_sz;
  var l_left: f32 = p_left;
  var l_right: f32 = p_right;
  var l_back: f32 = p_back;
  var l_front: f32 = p_front;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_d: f32;
  var l_t: f32;
  l_d = coax_p_exp_ground_distance(l_sx, l_sz, l_left, l_right, l_back, l_front, l_x, l_z);
  if ((l_d >= 0x1.000000p2f))
  {
    return l_h;
  }
  l_t = select((l_d / 0x1.000000p2f), 0.0f, (l_d <= 0.0f));
  l_t = ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
  return ((coax_p_cg_height(l_sx, l_sz) * (0x1.000000p0f - l_t)) + (l_h * l_t));
}
fn coax_p_exp_ground_castle_distance(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  return coax_p_exp_ground_distance(0x1.700000p5f, (-0x1.780000p5f), 0x1.040000p5f, 0x1.040000p5f, 0x1.f80000p4f, 0x1.f00000p4f, l_x, l_z);
}
fn coax_p_e2_meadow_share(p_kit: i32) -> f32 {
  var l_kit: i32 = p_kit;
  if (((l_kit >= 25i) && (l_kit <= 27i)))
  {
    return 0x1.777778p-3f;
  }
  if ((l_kit == 31i))
  {
    return 0x1.5c28f6p-3f;
  }
  if (((l_kit >= 20i) && (l_kit <= 22i)))
  {
    return 0x1.111112p-5f;
  }
  if (((l_kit == 29i) || (l_kit == 30i)))
  {
    return 0x1.99999ap-5f;
  }
  if ((l_kit == 28i))
  {
    return 0x1.47ae14p-4f;
  }
  return 0.0f;
}
fn coax_p_e2_meadow_reach(p_kit: i32) -> f32 {
  var l_kit: i32 = p_kit;
  return select(select(select(0x1.400000p6f, 0x1.900000p6f, ((l_kit >= 25i) && (l_kit <= 27i))), 0x1.180000p7f, ((l_kit == 29i) || (l_kit == 30i))), 0x1.400000p8f, ((l_kit >= 20i) && (l_kit <= 22i)));
}
fn coax_p_fivelab_upper(p_j: u32) -> f32 {
  var l_j: u32 = p_j;
  if (((l_j <= 1u) || (l_j >= 55u)))
  {
    return 0.0f;
  }
  if ((l_j == 2u))
  {
    return 0x1.99999ap-2f;
  }
  if ((l_j == 3u))
  {
    return 0x1.800000p-1f;
  }
  return 0x1.000000p0f;
}
fn coax_p_fivelab_arm(p_row: u32, p_j: u32, p_m: coax_t35) -> coax_t35 {
  var l_row: u32 = p_row;
  var l_j: u32 = p_j;
  var l_m: coax_t35 = p_m;
  var l_q: vec4f;
  var l_qe: vec4f;
  var l_qw: vec4f;
  var l_p: vec4f;
  var l_still: bool;
  var l_ao: f32;
  var l_af: f32;
  var l_ak: f32;
  var l_ca: f32;
  var l_sa: f32;
  var l_cf: f32;
  var l_sf: f32;
  var l_r1: coax_t35;
  var l_t1: coax_t35;
  var l_e: vec4f;
  var l_w: vec4f;
  if ((((l_j < 8u) || (l_j > 30u)) || (!coax_p_fivelab_arm_frame(l_row))))
  {
    return l_m;
  }
  l_q = coax_p_v4(0x1.8902dep-3f, 0x1.749ba6p0f, (-0x1.0be0dep-4f), 0x1.000000p0f);
  l_qe = coax_p_v4(0x1.c58794p-2f, 0x1.749ba6p0f, (-0x1.2b020cp-4f), 0x1.000000p0f);
  l_qw = coax_p_v4(0x1.5dcc64p-1f, 0x1.749ba6p0f, (-0x1.0be0dep-4f), 0x1.000000p0f);
  if (((l_row / 195u) >= 4137u))
  {
    l_q = coax_p_v4(0x1.367a10p-3f, 0x1.6b089ap0f, (-0x1.bcd35ap-5f), 0x1.000000p0f);
    l_qe = coax_p_v4(0x1.914e3cp-2f, 0x1.6b089ap0f, (-0x1.e1b08ap-5f), 0x1.000000p0f);
    l_qw = coax_p_v4(0x1.483e42p-1f, 0x1.6b089ap0f, (-0x1.a6b50cp-5f), 0x1.000000p0f);
  }
  l_p = coax_p_fivelab_at(coax_p_fivelab_joint(l_row, 8u), l_q);
  l_still = (((l_row / 195u) % 4137u) < 75u);
  l_ao = 0x1.666666p-2f;
  l_af = 0x1.666666p-2f;
  l_ak = 0.0f;
  if (l_still)
  {
    l_ao = 0.0f;
    l_af = 0x1.333334p-1f;
    l_ak = 0x1.99999ap-1f;
  }
  l_ca = cos(l_ao);
  l_sa = sin(l_ao);
  l_cf = cos(l_af);
  l_sf = sin(l_af);
  l_r1 = coax_t35();
  l_r1.x = coax_p_v4(l_ca, (-l_sa), 0.0f, 0.0f);
  l_r1.y = coax_p_v4((l_cf * l_sa), (l_cf * l_ca), l_sf, 0.0f);
  l_r1.z = coax_p_v4(((-l_sf) * l_sa), ((-l_sf) * l_ca), l_cf, 0.0f);
  l_t1 = coax_p_fivelab_turn_at(l_r1, l_p);
  if ((l_j == 8u))
  {
    return coax_p_fivelab_aff(l_t1, l_m);
  }
  l_e = coax_p_fivelab_at(coax_p_fivelab_aff(l_t1, coax_p_fivelab_joint(l_row, 9u)), l_qe);
  l_w = coax_p_fivelab_at(coax_p_fivelab_aff(l_t1, coax_p_fivelab_joint(l_row, 10u)), l_qw);
  return coax_p_fivelab_aff(coax_p_fivelab_turn_at(coax_p_fivelab_elbow_open(l_p, l_e, l_w, l_ak), l_e), coax_p_fivelab_aff(l_t1, l_m));
}
fn coax_p_verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return coax_p_render_noise2(l_x, l_y);
}
fn coax_p_render_footprint(p_width: i32, p_height: i32, p_du_dx: f32, p_dv_dx: f32, p_du_dy: f32, p_dv_dy: f32, p_bias: f32) -> coax_t40 {
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
    return coax_t40(false, 0i, 0.0f, 0.0f, 0.0f);
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
  return coax_t40(true, l_taps, (l_du / f32(l_taps)), (l_dv / f32(l_taps)), ((log(l_span) / log(0x1.000000p1f)) + l_bias));
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
fn coax_p_e2_ground_layer(p_x: f32, p_z: f32, p_l: i32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_l: i32 = p_l;
  if ((l_l == 0i))
  {
    return coax_p_e2_gold(l_x, l_z);
  }
  if ((l_l == 1i))
  {
    return (coax_p_e2_forest(l_x, l_z) * 0x1.333334p-1f);
  }
  return coax_p_e2_path(l_x, l_z);
}
fn coax_p_aethyr_terrain_over(p_w: f32, p_c: f32, p_is: f32) -> f32 {
  var l_w: f32 = p_w;
  var l_c: f32 = p_c;
  var l_is: f32 = p_is;
  return ((l_w * (0x1.000000p0f - l_c)) + (l_c * l_is));
}
fn coax_p_aethyr_terrain_blend_t(p_x: f32, p_z: f32, p_f1: f32, p_w1: f32, p_o1: f32, p_f2: f32, p_w2: f32, p_o2: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_f1: f32 = p_f1;
  var l_w1: f32 = p_w1;
  var l_o1: f32 = p_o1;
  var l_f2: f32 = p_f2;
  var l_w2: f32 = p_w2;
  var l_o2: f32 = p_o2;
  var l_t: f32;
  l_t = (coax_p_aethyr_terrain_noise(((l_x * l_f1) + l_o1), ((l_z * l_f1) + l_o1)) * l_w1);
  if ((l_w2 > 0.0f))
  {
    l_t = (l_t + (coax_p_aethyr_terrain_noise(((l_x * l_f2) + l_o2), ((l_z * l_f2) + l_o2)) * l_w2));
  }
  return l_t;
}
fn coax_p_e2_ground_pick(p_w: vec4f, p_k: f32) -> f32 {
  var l_w: vec4f = p_w;
  var l_k: f32 = p_k;
  return select(select(select(l_w.w, l_w.z, (l_k < 0x1.400000p1f)), l_w.y, (l_k < 0x1.800000p0f)), l_w.x, (l_k < 0x1.000000p-1f));
}
fn coax_p_aethyr_terrain_share(p_w_of: f32, p_w_against: f32) -> f32 {
  var l_w_of: f32 = p_w_of;
  var l_w_against: f32 = p_w_against;
  var l_total: f32;
  l_total = (l_w_of + l_w_against);
  if ((l_total > 0.0f))
  {
    return (l_w_of / l_total);
  }
  return 0.0f;
}
fn coax_p_aethyr_terrain_gain(p_x: f32, p_z: f32, p_g0: f32, p_g1: f32, p_f3: f32, p_o3: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_g0: f32 = p_g0;
  var l_g1: f32 = p_g1;
  var l_f3: f32 = p_f3;
  var l_o3: f32 = p_o3;
  if ((l_g1 > 0.0f))
  {
    return (l_g0 + (l_g1 * coax_p_aethyr_terrain_noise(((l_x * l_f3) + l_o3), ((l_z * l_f3) + l_o3))));
  }
  return l_g0;
}
fn coax_p_aethyr_terrain_decode(p_a: f32, p_b: f32, p_t: f32, p_scale: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  var l_scale: f32 = p_scale;
  return coax_pow_bg_vs(coax_p_fmax((coax_p_aethyr_terrain_mix(l_a, l_b, l_t) * l_scale), 0.0f), 0x1.19999ap1f);
}
fn coax_p_cg_path_x(p_z: f32) -> f32 {
  var l_z: f32 = p_z;
  return (((0x1.0cccccp1f * sin((l_z * 0x1.147ae2p-2f))) + 0x1.ccccccp-1f) - select(0.0f, (l_z * 0x1.19999ap-1f), (l_z > 0.0f)));
}
fn coax_p_paint_env_w(p_k: i32) -> f32 {
  var l_k: i32 = p_k;
  return coax_at_u(i32((208i + l_k)));
}
fn coax_p_aethyr_terrain_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_paint_phase(p_cos_t: f32, p_g: f32) -> f32 {
  var l_cos_t: f32 = p_cos_t;
  var l_g: f32 = p_g;
  return coax_p_verse_style_haze_phase(l_cos_t, l_g);
}
fn coax_p_paint_env_haze_window(p_distance: f32) -> f32 {
  var l_distance: f32 = p_distance;
  return coax_p_verse_style_haze_window(l_distance, coax_p_paint_env_c(48i), coax_p_paint_env_c(49i), coax_p_paint_env_c(50i));
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
fn coax_p_film_rgb(p_r: f32, p_g: f32, p_b: f32) -> vec3f {
  var l_r: f32 = p_r;
  var l_g: f32 = p_g;
  var l_b: f32 = p_b;
  return vec3f(l_r, l_g, l_b);
}
fn coax_p_film_exposed(p_linear: vec3f, p_exposure: f32) -> vec3f {
  var l_linear: vec3f = p_linear;
  var l_exposure: f32 = p_exposure;
  return coax_p_film_rec709(coax_p_film_scale(l_linear, l_exposure));
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
fn coax_p_clearing_edge(p_c: coax_t38, p_x: f32, p_z: f32) -> f32 {
  var l_c: coax_t38 = p_c;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_dx: f32;
  var l_dz: f32;
  var l_k: f32;
  l_dx = (l_x - l_c.x);
  l_dz = (l_z - l_c.y);
  l_k = (0x1.000000p0f / (sqrt(((l_dx * l_dx) + (l_dz * l_dz))) + 0x1.0624dep-10f));
  return (l_c.z * ((0x1.000000p0f - (l_c.f4 * 0x1.000000p-1f)) + (l_c.f4 * coax_p_clr_noise((((l_dx * l_k) * 0x1.400000p1f) + 0x1.200000p3f), (((l_dz * l_k) * 0x1.400000p1f) + 0x1.800000p1f)))));
}
fn coax_p_stream_x(p_s: coax_t37, p_z: f32) -> f32 {
  var l_s: coax_t37 = p_s;
  var l_z: f32 = p_z;
  return (l_s.x + (l_s.w * sin(((l_z * l_s.f4) + l_s.f5))));
}
fn coax_p_cg_road_ground(p_x: f32, p_z: f32, p_h: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_h: f32 = p_h;
  var l_corridor: f32;
  var l_g: f32;
  var l_ex: f32;
  var l_ez: f32;
  l_corridor = (((0x1.000000p0f - coax_p_cg_ramp(0x1.000000p1f, 0x1.400000p2f, abs((l_x - coax_p_cg_road_x(l_z))))) * coax_p_cg_ramp((-0x1.c00000p3f), (-0x1.200000p4f), l_z)) * (0x1.000000p0f - coax_p_cg_ramp((-0x1.c00000p5f), (-0x1.f00000p5f), l_z)));
  l_g = (l_h + (((0x1.a8f5c2p-1f + (0x1.f0a3d8p-1f * coax_p_cg_ramp((-0x1.000000p4f), (-0x1.080000p5f), l_z))) - l_h) * l_corridor));
  l_ex = ((l_x - 0x1.200000p3f) / 0x1.a00000p3f);
  l_ez = ((l_z + 0x1.740000p5f) / 0x1.900000p3f);
  l_g = (l_g + ((0x1.ccccccp0f - l_g) * (0x1.000000p0f - coax_p_cg_ramp(0x1.b33334p-1f, 0x1.333334p0f, sqrt(((l_ex * l_ex) + (l_ez * l_ez)))))));
  return (l_g + (coax_p_cg_ramp((-0x1.f00000p5f), (-0x1.880000p6f), l_z) * (0x1.a00000p2f + (0x1.800000p1f * sin(((l_x * 0x1.70a3d8p-4f) + 0x1.4cccccp0f))))));
}
fn coax_p_exp_ground_distance(p_sx: f32, p_sz: f32, p_left: f32, p_right: f32, p_back: f32, p_front: f32, p_x: f32, p_z: f32) -> f32 {
  var l_sx: f32 = p_sx;
  var l_sz: f32 = p_sz;
  var l_left: f32 = p_left;
  var l_right: f32 = p_right;
  var l_back: f32 = p_back;
  var l_front: f32 = p_front;
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_dx: f32;
  var l_dz: f32;
  l_dx = coax_p_exp_ground_max(((l_sx - l_left) - l_x), ((l_x - l_sx) - l_right));
  l_dz = coax_p_exp_ground_max(((l_sz - l_back) - l_z), ((l_z - l_sz) - l_front));
  return coax_p_exp_ground_max(l_dx, l_dz);
}
fn coax_p_fivelab_arm_frame(p_row: u32) -> bool {
  var l_row: u32 = p_row;
  var l_f: u32;
  l_f = ((l_row / 195u) % 4137u);
  return ((l_f < 143u) || ((l_f >= 389u) && (l_f < 464u)));
}
fn coax_p_fivelab_joint(p_row: u32, p_j: u32) -> coax_t35 {
  var l_row: u32 = p_row;
  var l_j: u32 = p_j;
  var l_r: u32;
  var l_o: coax_t35;
  l_r = (l_row + (l_j * 3u));
  l_o = coax_t35();
  l_o.x = bake[i32(l_r)];
  l_o.y = bake[i32((l_r + 1u))];
  l_o.z = bake[i32((l_r + 2u))];
  return l_o;
}
fn coax_p_fivelab_at(p_m: coax_t35, p_q: vec4f) -> vec4f {
  var l_m: coax_t35 = p_m;
  var l_q: vec4f = p_q;
  return coax_p_v4(coax_p_wg_dot4(l_m.x, l_q), coax_p_wg_dot4(l_m.y, l_q), coax_p_wg_dot4(l_m.z, l_q), 0x1.000000p0f);
}
fn coax_p_fivelab_turn_at(p_r: coax_t35, p_p: vec4f) -> coax_t35 {
  var l_r: coax_t35 = p_r;
  var l_p: vec4f = p_p;
  var l_o: coax_t35;
  l_o = coax_t35();
  l_o.x = coax_p_v4(l_r.x.x, l_r.x.y, l_r.x.z, (l_p.x - (((l_r.x.x * l_p.x) + (l_r.x.y * l_p.y)) + (l_r.x.z * l_p.z))));
  l_o.y = coax_p_v4(l_r.y.x, l_r.y.y, l_r.y.z, (l_p.y - (((l_r.y.x * l_p.x) + (l_r.y.y * l_p.y)) + (l_r.y.z * l_p.z))));
  l_o.z = coax_p_v4(l_r.z.x, l_r.z.y, l_r.z.z, (l_p.z - (((l_r.z.x * l_p.x) + (l_r.z.y * l_p.y)) + (l_r.z.z * l_p.z))));
  return l_o;
}
fn coax_p_fivelab_aff(p_t: coax_t35, p_m: coax_t35) -> coax_t35 {
  var l_t: coax_t35 = p_t;
  var l_m: coax_t35 = p_m;
  var l_o: coax_t35;
  l_o = coax_t35();
  l_o.x = coax_p_fivelab_lin(l_t.x.x, l_t.x.y, l_t.x.z, l_t.x.w, l_m);
  l_o.y = coax_p_fivelab_lin(l_t.y.x, l_t.y.y, l_t.y.z, l_t.y.w, l_m);
  l_o.z = coax_p_fivelab_lin(l_t.z.x, l_t.z.y, l_t.z.z, l_t.z.w, l_m);
  return l_o;
}
fn coax_p_fivelab_elbow_open(p_p: vec4f, p_e: vec4f, p_w: vec4f, p_k: f32) -> coax_t35 {
  var l_p: vec4f = p_p;
  var l_e: vec4f = p_e;
  var l_w: vec4f = p_w;
  var l_k: f32 = p_k;
  var l_ux: f32;
  var l_uy: f32;
  var l_uz: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_fz: f32;
  var l_ax: f32;
  var l_ay: f32;
  var l_az: f32;
  var l_la: f32;
  var l_d: f32;
  var l_o: coax_t35;
  var l_th: f32;
  var l_c: f32;
  var l_sn: f32;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_v: f32;
  l_ux = (l_e.x - l_p.x);
  l_uy = (l_e.y - l_p.y);
  l_uz = (l_e.z - l_p.z);
  l_fx = (l_w.x - l_e.x);
  l_fy = (l_w.y - l_e.y);
  l_fz = (l_w.z - l_e.z);
  l_ax = ((l_fy * l_uz) - (l_fz * l_uy));
  l_ay = ((l_fz * l_ux) - (l_fx * l_uz));
  l_az = ((l_fx * l_uy) - (l_fy * l_ux));
  l_la = sqrt((((l_ax * l_ax) + (l_ay * l_ay)) + (l_az * l_az)));
  l_d = (((l_fx * l_ux) + (l_fy * l_uy)) + (l_fz * l_uz));
  l_o = coax_t35();
  l_o.x = coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0.0f);
  l_o.y = coax_p_v4(0.0f, 0x1.000000p0f, 0.0f, 0.0f);
  l_o.z = coax_p_v4(0.0f, 0.0f, 0x1.000000p0f, 0.0f);
  if ((l_la < 0x1.0c6f7ap-20f))
  {
    return l_o;
  }
  l_th = (coax_p_wg_atan2(l_la, l_d) * l_k);
  l_c = cos(l_th);
  l_sn = sin(l_th);
  l_x = (l_ax / l_la);
  l_y = (l_ay / l_la);
  l_z = (l_az / l_la);
  l_v = (0x1.000000p0f - l_c);
  l_o.x = coax_p_v4((l_c + ((l_x * l_x) * l_v)), (((l_x * l_y) * l_v) - (l_z * l_sn)), (((l_x * l_z) * l_v) + (l_y * l_sn)), 0.0f);
  l_o.y = coax_p_v4((((l_y * l_x) * l_v) + (l_z * l_sn)), (l_c + ((l_y * l_y) * l_v)), (((l_y * l_z) * l_v) - (l_x * l_sn)), 0.0f);
  l_o.z = coax_p_v4((((l_z * l_x) * l_v) - (l_y * l_sn)), (((l_z * l_y) * l_v) + (l_x * l_sn)), (l_c + ((l_z * l_z) * l_v)), 0.0f);
  return l_o;
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
fn coax_p_aethyr_terrain_noise(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_ix: f32;
  var l_iz: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_sx: f32;
  var l_sz: f32;
  l_ix = floor(l_x);
  l_iz = floor(l_z);
  l_fx = (l_x - l_ix);
  l_fz = (l_z - l_iz);
  l_sx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_sz = ((l_fz * l_fz) * (0x1.800000p1f - (0x1.000000p1f * l_fz)));
  return coax_p_aethyr_terrain_mix(coax_p_aethyr_terrain_mix(coax_p_render_noise2(l_ix, l_iz), coax_p_render_noise2((l_ix + 0x1.000000p0f), l_iz), l_sx), coax_p_aethyr_terrain_mix(coax_p_render_noise2(l_ix, (l_iz + 0x1.000000p0f)), coax_p_render_noise2((l_ix + 0x1.000000p0f), (l_iz + 0x1.000000p0f)), l_sx), l_sz);
}
fn coax_p_verse_style_haze_phase(p_vdl: f32, p_g: f32) -> f32 {
  var l_vdl: f32 = p_vdl;
  var l_g: f32 = p_g;
  var l_g2: f32;
  var l_d: f32;
  l_g2 = (l_g * l_g);
  l_d = coax_p_st_max(((0x1.000000p0f + l_g2) - ((0x1.000000p1f * l_g) * coax_p_st_max((-0x1.000000p0f), coax_p_st_min(0x1.000000p0f, l_vdl)))), 0x1.a36e2ep-14f);
  return ((0x1.000000p0f - l_g2) / (l_d * sqrt(l_d)));
}
fn coax_p_verse_style_haze_window(p_d: f32, p_near: f32, p_far: f32, p_gain: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_near: f32 = p_near;
  var l_far: f32 = p_far;
  var l_gain: f32 = p_gain;
  return (0x1.000000p0f + (l_gain * coax_p_verse_style_soft_step(((l_near + l_far) * 0x1.000000p-1f), (coax_p_st_max((l_far - l_near), 0x1.a36e2ep-14f) * 0x1.000000p-1f), l_d)));
}
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  var l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
}
fn coax_p_film_scale(p_c: vec3f, p_s: f32) -> vec3f {
  var l_c: vec3f = p_c;
  var l_s: f32 = p_s;
  return coax_p_film_rgb((l_c.x * l_s), (l_c.y * l_s), (l_c.z * l_s));
}
fn coax_p_film_rec709(p_linear: vec3f) -> vec3f {
  var l_linear: vec3f = p_linear;
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
fn coax_p_clr_noise(p_x: f32, p_z: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_z: f32 = p_z;
  var l_i: f32;
  var l_j: f32;
  var l_fx: f32;
  var l_fz: f32;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_i = floor(l_x);
  l_j = floor(l_z);
  l_fx = (l_x - l_i);
  l_fz = (l_z - l_j);
  l_fx = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_fz = ((l_fz * l_fz) * (0x1.800000p1f - (0x1.000000p1f * l_fz)));
  l_a = coax_p_clr_hash(l_i, l_j);
  l_b = coax_p_clr_hash((l_i + 0x1.000000p0f), l_j);
  l_c = coax_p_clr_hash(l_i, (l_j + 0x1.000000p0f));
  l_d = coax_p_clr_hash((l_i + 0x1.000000p0f), (l_j + 0x1.000000p0f));
  return ((((l_a * (0x1.000000p0f - l_fx)) + (l_b * l_fx)) * (0x1.000000p0f - l_fz)) + (((l_c * (0x1.000000p0f - l_fx)) + (l_d * l_fx)) * l_fz));
}
fn coax_p_exp_ground_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_fivelab_lin(p_x: f32, p_y: f32, p_z: f32, p_w: f32, p_m: coax_t35) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  var l_m: coax_t35 = p_m;
  return coax_p_v4((((l_x * l_m.x.x) + (l_y * l_m.y.x)) + (l_z * l_m.z.x)), (((l_x * l_m.x.y) + (l_y * l_m.y.y)) + (l_z * l_m.z.y)), (((l_x * l_m.x.z) + (l_y * l_m.y.z)) + (l_z * l_m.z.z)), ((((l_x * l_m.x.w) + (l_y * l_m.y.w)) + (l_z * l_m.z.w)) + l_w));
}
fn coax_p_verse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_film_in(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_film_rgb((((l_c.x * 0x1.39e7e8p-1f) + (l_c.y * 0x1.5babf4p-2f)) + (l_c.z * 0x1.8421eap-5f)), (((l_c.x * 0x1.1f8374p-4f) + (l_c.y * 0x1.d52c56p-1f)) + (l_c.z * 0x1.b8cee6p-7f)), (((l_c.x * 0x1.51c410p-6f) + (l_c.y * 0x1.c0cc3cp-4f)) + (l_c.z * 0x1.bd5858p-1f)));
}
fn coax_p_film_ap0(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_film_rgb((((l_c.x * 0x1.641250p-1f) + (l_c.y * 0x1.201c28p-3f)) + (l_c.z * 0x1.4f9a96p-3f)), (((l_c.x * 0x1.6ef502p-5f) + (l_c.y * 0x1.b826d0p-1f)) + (l_c.z * 0x1.874efep-4f)), ((((-l_c.x) * 0x1.6a24eep-8f) + (l_c.y * 0x1.07cbd2p-8f)) + (l_c.z * 0x1.00625ap0f)));
}
fn coax_p_film_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return select(l_b, l_a, (l_a < l_b));
}
fn coax_p_film_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_film_atan2(p_y: f32, p_x: f32) -> f32 {
  var l_y: f32 = p_y;
  var l_x: f32 = p_x;
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
fn coax_p_film_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_film_min(0x1.000000p0f, coax_p_film_max(0.0f, l_x));
}
fn coax_p_film_ap1(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_film_rgb((((l_c.x * 0x1.739186p0f) - (l_c.y * 0x1.e45fc0p-3f)) - (l_c.z * 0x1.b82c78p-3f)), ((((-l_c.x) * 0x1.399074p-4f) + (l_c.y * 0x1.2d1d64p0f)) - (l_c.z * 0x1.9845c8p-4f)), (((l_c.x * 0x1.1080e8p-7f) - (l_c.y * 0x1.8b57b6p-8f)) + (l_c.z * 0x1.fed4acp-1f)));
}
fn coax_p_film_mix_grey(p_c: vec3f, p_keep: f32) -> vec3f {
  var l_c: vec3f = p_c;
  var l_keep: f32 = p_keep;
  var l_y: f32;
  l_y = (((l_c.x * 0x1.16c320p-2f) + (l_c.y * 0x1.59213ep-1f)) + (l_c.z * 0x1.b7d314p-5f));
  return coax_p_film_rgb((l_y + ((l_c.x - l_y) * l_keep)), (l_y + ((l_c.y - l_y) * l_keep)), (l_y + ((l_c.z - l_y) * l_keep)));
}
fn coax_p_film_curve(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
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
  var l_c: vec3f = p_c;
  return coax_p_film_rgb((((l_c.x * 0x1.b47e38p0f) - (l_c.y * 0x1.3e5b8ap-1f)) - (l_c.z * 0x1.550742p-4f)), ((((-l_c.x) * 0x1.0ac3e0p-3f) + (l_c.y * 0x1.240bc8p0f)) - (l_c.z * 0x1.59a5b6p-7f)), ((((-l_c.x) * 0x1.894560p-6f) - (l_c.y * 0x1.0820e2p-3f)) + (l_c.z * 0x1.272932p0f)));
}
fn coax_p_clr_hash(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_v: f32;
  l_v = (sin(((l_a * 0x1.fc6666p6f) + (l_b * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn shadow_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_13 {
  let coax_r = coax_p_shadow_vs(coax_vid, coax_iid);
  return coax_io_13(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_16) -> coax_io_18 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_18(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_19 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_19(coax_r.x, coax_r.y, coax_r.z);
}
@fragment
fn shadow_fs(coax_in: coax_io_13) {
  coax_p_shadow_fs(coax_t13(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
}
@fragment
fn scene_fs(coax_in: coax_io_15, @builtin(front_facing) coax_front: bool) -> coax_out_25 {
  let coax_r = coax_p_scene_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8), coax_front);
  return coax_out_25(coax_r.x, coax_r.y);
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