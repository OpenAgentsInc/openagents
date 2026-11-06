// ---- generated: apps/openagents-web/gpu/character.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec4f,
  f5: f32,
  f6: vec4f,
  f7: vec4f,
  f8: vec4f,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec4f,
  @location(4) f5: f32,
  @location(5) f6: vec4f,
  @location(6) f7: vec4f,
  @location(7) f8: vec4f,
}
struct coax_t12 {
  x: vec4f,
  y: vec2f,
  z: f32,
  w: f32,
  f4: f32,
}
struct coax_io_12 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: f32,
  @location(2) w: f32,
  @location(3) f4: f32,
}
struct coax_t13 {
  x: vec3f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: f32,
  f5: f32,
  f6: f32,
}
struct coax_t14 {
  x: vec3f,
  y: vec3f,
}
struct coax_t19 {
  x: vec3f,
  y: vec3f,
  z: f32,
  w: f32,
  f4: f32,
  f5: f32,
}
struct coax_t21 {
  x: vec4f,
  y: vec4f,
}
struct coax_out_21 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
struct coax_t22 {
  x: vec3f,
  y: vec3f,
}
struct coax_t23 {
  x: vec3f,
  y: vec3f,
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 68>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> bones: array<f32>;
@group(1) @binding(2) var sheets: texture_2d_array<f32>;
@group(1) @binding(3) var sheet: sampler;
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
fn coax_p_character_vs(p_id: u32, p_instance: u32) -> coax_t11 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  return coax_p_ch_place(l_id);
}
fn coax_p_caster_vs(p_id: u32, p_instance: u32) -> coax_t12 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_world: vec3f;
  var l_uv: vec2f;
  var l_material: f32;
  var l_coverage: f32;
  var l_edge: f32;
  var l_g: coax_t13;
  var l_at: i32;
  var l_m: coax_t14;
  var l_q: vec3f;
  if (((coax_at_u(i32(164i)) < 0x1.000000p-1f) || ((l_id >= 1000000u) && (!coax_p_cv_groom_kept(l_id, true)))))
  {
    return coax_t12(coax_p_v4(0.0f, 0.0f, (-0x1.000000p1f), 0x1.000000p0f), coax_p_v2(0.0f, 0.0f), 0x1.a00000p4f, 0.0f, 0.0f);
  }
  l_world = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_uv = coax_p_v2(0.0f, 0.0f);
  l_material = 0.0f;
  l_coverage = 0x1.000000p0f;
  l_edge = 0.0f;
  if ((l_id >= 1000000u))
  {
    l_g = coax_p_cv_groom(l_id, true);
    l_world = l_g.x;
    l_uv = l_g.w;
    l_material = l_g.f4;
    l_coverage = l_g.f5;
    l_edge = l_g.f6;
  }
  else
  {
    l_at = coax_p_cv_vertex_at(l_id);
    l_m = coax_p_cv_morph(l_id);
    l_world = coax_p_cv_add(coax_p_cv_at(104i), coax_p_cv_echo_direction(coax_p_cv_skin(l_id, coax_p_v4(l_m.x.x, l_m.x.y, l_m.x.z, 0x1.000000p0f))));
    l_uv = coax_p_v2(coax_p_cv_word((l_at + 6i)), coax_p_cv_word((l_at + 7i)));
    l_material = f32(vertices[(l_at + 8i)]);
  }
  l_q = coax_p_ch_shadow_place(l_world);
  return coax_t12(coax_p_v4(l_q.x, l_q.y, l_q.z, 0x1.000000p0f), l_uv, l_material, l_coverage, l_edge);
}
fn coax_p_character_fs(p_s: coax_t11) -> coax_t21 {
  var l_s: coax_t11 = p_s;
  var l_dx: vec2f;
  var l_dy: vec2f;
  var l_e: coax_t19;
  var l_geo: vec3f;
  var l_v: vec3f;
  var l_wet: f32;
  var l_hair: bool;
  var l_colour: vec3f;
  var l_skin: bool;
  var l_albedo: vec3f;
  var l_rough: f32;
  var l_metal: f32;
  var l_n: vec3f;
  var l_nv: f32;
  var l_f0: vec3f;
  var l_eye: bool;
  var l_inner: f32;
  var l_contact: f32;
  var l_soft: f32;
  var l_l: vec3f;
  var l_key: vec3f;
  var l_shadow: f32;
  var l_colour_29: vec3f;
  var l_teeth: bool;
  var l_bleed: f32;
  var l_k: i32;
  var l_at: i32;
  var l_delta: vec3f;
  var l_d: f32;
  var l_share: f32;
  var l_env: vec2f;
  var l_sky: vec3f;
  var l_reflect: vec3f;
  l_dx = coax_p_v2(dpdxFine(l_s.w.x), dpdxFine(l_s.w.y));
  l_dy = coax_p_v2(dpdyFine(l_s.w.x), dpdyFine(l_s.w.y));
  l_e = coax_p_cv_echo_material(l_s.z, l_s.f4, l_s.y, l_s.w, l_s.f5, l_dx, l_dy);
  l_geo = coax_p_wg_normalize3(l_s.z);
  l_v = coax_p_wg_normalize3(coax_p_cv_sub(coax_p_cv_at(8i), l_s.y));
  l_wet = coax_p_clampf(coax_at_u(i32(192i)), 0.0f, 0x1.000000p0f);
  l_hair = ((l_s.f5 == 0x1.a00000p4f) || (l_s.f5 == 0x1.b00000p4f));
  if (l_hair)
  {
    l_colour = coax_p_cv_add(coax_p_cv_scale(coax_p_v3(l_s.f7.x, l_s.f7.y, l_s.f7.z), coax_p_ch_shadow(l_s.y, l_geo)), coax_p_v3(l_s.f8.x, l_s.f8.y, l_s.f8.z));
    l_colour = coax_p_cv_scale(l_colour, (0x1.000000p0f - (0x1.333334p-2f * l_wet)));
    return coax_p_ch_out(coax_p_ch_haze(l_colour, l_s.y, coax_p_cv_scale(l_v, (-0x1.000000p0f))), coax_p_cv_groom_opacity(l_s.f6.x, l_s.f6.y), l_s.y);
  }
  l_skin = ((l_s.f5 == 0x1.600000p3f) || (l_s.f5 == 0x1.800000p2f));
  l_albedo = coax_p_cv_scale(l_e.y, (0x1.000000p0f - (select(0x1.333334p-2f, 0x1.eb851ep-4f, l_skin) * l_wet)));
  l_rough = coax_p_clampf((l_e.z * (0x1.000000p0f - (0x1.666666p-2f * l_wet))), 0x1.70a3d8p-5f, 0x1.000000p0f);
  l_metal = l_e.w;
  l_n = l_e.x;
  l_nv = coax_p_fmax(0x1.a36e2ep-14f, coax_p_wg_dot3(l_n, l_v));
  l_f0 = coax_p_v3(coax_p_mix(l_e.f4, l_albedo.x, l_metal), coax_p_mix(l_e.f4, l_albedo.y, l_metal), coax_p_mix(l_e.f4, l_albedo.z, l_metal));
  l_eye = ((l_s.f5 == 0x1.200000p3f) || (l_s.f5 == 0x1.100000p4f));
  l_inner = select(select(0x1.000000p0f, 0x1.666666p-2f, l_eye), 0x1.eb851ep-5f, (l_s.f5 == 0x1.400000p3f));
  l_contact = select(0x1.000000p0f, coax_p_fmin(0x1.000000p0f, (l_e.f5 * 0x1.19999ap0f)), l_skin);
  l_soft = (l_inner * l_contact);
  l_l = coax_p_cv_at(24i);
  l_key = coax_p_cv_at(168i);
  l_shadow = coax_p_ch_shadow(l_s.y, l_geo);
  l_colour_29 = coax_p_cv_mul(coax_p_cv_scale(l_albedo, (((0x1.000000p0f - l_metal) * l_e.f5) * l_inner)), coax_p_ch_ambient(l_n.y));
  l_colour_29 = coax_p_cv_add(l_colour_29, coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_slay_light(l_skin, l_albedo, l_f0, l_metal, l_rough, l_n, l_geo, l_l, l_v, 0x1.000000p0f, 0.0f), ((0x1.921fb6p1f * l_shadow) * l_soft)), l_key));
  l_teeth = (l_s.f5 == 0x1.400000p3f);
  if ((l_skin || l_teeth))
  {
    l_bleed = (((coax_p_fmax(0.0f, ((coax_p_wg_dot3(l_geo, l_l) + 0x1.000000p-1f) / 0x1.800000p0f)) * (0x1.000000p0f - l_shadow)) * l_contact) * select(0x1.000000p0f, 0x1.000000p-1f, l_teeth));
    l_colour_29 = coax_p_cv_add(l_colour_29, coax_p_v3((((l_albedo.x * l_key.x) * l_bleed) * 0x1.666666p-2f), (((l_albedo.y * l_key.y) * l_bleed) * 0x1.1eb852p-4f), 0.0f));
  }
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 8i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      if ((f32(l_k) >= coax_at_u(i32(196i))))
      {
        break;
      }
      l_at = (200i + (l_k * 8i));
      l_delta = coax_p_cv_sub(coax_p_cv_at(l_at), l_s.y);
      l_d = coax_p_wg_length3(l_delta);
      l_share = coax_p_ch_pool(l_d, coax_at_u(i32((l_at + 3i))));
      if ((l_share > 0.0f))
      {
        l_colour_29 = coax_p_cv_add(l_colour_29, coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_slay_light(l_skin, l_albedo, l_f0, l_metal, l_rough, l_n, l_geo, coax_p_cv_scale(l_delta, (0x1.000000p0f / coax_p_fmax(l_d, 0x1.0624dep-10f))), l_v, 0x1.000000p0f, 0.0f), (((0x1.921fb6p1f * l_share) * coax_at_u(i32((l_at + 7i)))) * l_soft)), coax_p_cv_at((l_at + 4i))));
      }
    }
  }
  l_env = coax_p_pbr_environment(l_rough, l_nv);
  l_sky = coax_p_cv_at(172i);
  l_reflect = coax_p_cv_scale(coax_p_cv_add(coax_p_cv_scale(l_f0, l_env.x), coax_p_v3(l_env.y, l_env.y, l_env.y)), ((coax_p_cv_specular_occlusion(l_nv, l_e.f5, l_rough) * l_inner) * (0x1.000000p0f + l_wet)));
  l_colour_29 = coax_p_cv_add(l_colour_29, coax_p_cv_mul(l_reflect, l_sky));
  return coax_p_ch_out(coax_p_ch_haze(l_colour_29, l_s.y, coax_p_cv_scale(l_v, (-0x1.000000p0f))), 0.0f, l_s.y);
}
fn coax_p_caster_fs(p_s: coax_t12) {
  var l_s: coax_t12 = p_s;
  var l_noise: f32;
  var l_dx: vec2f;
  var l_dy: vec2f;
  if (((l_s.z == 0x1.a00000p4f) || (l_s.z == 0x1.b00000p4f)))
  {
    l_noise = (sin((((floor(l_s.x.x) * 0x1.9fac72p3f) + (floor(l_s.x.y) * 0x1.38ee98p6f)) + (l_s.y.y * 0x1.b50000p8f))) * 0x1.55dd18p15f);
    if ((coax_p_cv_groom_opacity(l_s.w, l_s.f4) < (l_noise - floor(l_noise))))
    {
      discard;
    }
  }
  l_dx = coax_p_v2(dpdxFine(l_s.y.x), dpdxFine(l_s.y.y));
  l_dy = coax_p_v2(dpdyFine(l_s.y.x), dpdyFine(l_s.y.y));
  coax_p_cv_echo_mask(l_s.y, l_s.z, l_dx, l_dy);
}
fn coax_p_ch_place(p_id: u32) -> coax_t11 {
  var l_id: u32 = p_id;
  var l_g: coax_t13;
  var l_rough: f32;
  var l_lit: coax_t22;
  var l_at: i32;
  var l_m: coax_t14;
  var l_world: vec3f;
  var l_normal: vec3f;
  var l_t: vec3f;
  if ((l_id >= 1000000u))
  {
    if ((!coax_p_cv_groom_kept(l_id, false)))
    {
      return coax_p_ch_hidden();
    }
    l_g = coax_p_cv_groom(l_id, false);
    l_rough = (coax_p_mix(0x1.47ae14p-2f, 0x1.ccccccp-2f, l_g.w.y) * (0x1.000000p0f + (0x1.99999ap-4f * l_g.w.x)));
    l_lit = coax_p_ch_hair_light(coax_p_cv_hair_colour(l_g.w.x, l_g.w.y), l_rough, l_g.w.y, coax_p_wg_normalize3(l_g.z), l_g.x);
    return coax_t11(coax_p_ch_clip(l_g.x), l_g.x, l_g.y, l_g.w, coax_p_v4(l_g.z.x, l_g.z.y, l_g.z.z, 0x1.000000p0f), l_g.f4, coax_p_v4(l_g.f5, l_g.f6, 0x1.000000p0f, 0x1.000000p0f), coax_p_v4(l_lit.x.x, l_lit.x.y, l_lit.x.z, 0x1.000000p0f), coax_p_v4(l_lit.y.x, l_lit.y.y, l_lit.y.z, 0.0f));
  }
  l_at = coax_p_cv_vertex_at(l_id);
  l_m = coax_p_cv_morph(l_id);
  l_world = coax_p_cv_add(coax_p_cv_at(104i), coax_p_cv_echo_direction(coax_p_cv_skin(l_id, coax_p_v4(l_m.x.x, l_m.x.y, l_m.x.z, 0x1.000000p0f))));
  l_normal = coax_p_cv_echo_direction(coax_p_cv_skin(l_id, coax_p_v4(l_m.y.x, l_m.y.y, l_m.y.z, 0.0f)));
  l_t = coax_p_cv_echo_direction(coax_p_cv_skin(l_id, coax_p_v4(coax_p_cv_word((l_at + 15i)), coax_p_cv_word((l_at + 16i)), coax_p_cv_word((l_at + 17i)), 0.0f)));
  return coax_t11(coax_p_ch_clip(l_world), l_world, l_normal, coax_p_v2(coax_p_cv_word((l_at + 6i)), coax_p_cv_word((l_at + 7i))), coax_p_v4(l_t.x, l_t.y, l_t.z, coax_p_cv_word((l_at + 18i))), f32(vertices[(l_at + 8i)]), coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_cv_groom_kept(p_id: u32, p_shadow: bool) -> bool {
  var l_id: u32 = p_id;
  var l_shadow: bool = p_shadow;
  var l_share: f32;
  l_share = coax_p_cv_groom_share(l_shadow);
  if ((l_share >= 0x1.000000p0f))
  {
    return true;
  }
  return (coax_p_cv_groom_hash(coax_p_cv_word(((13573828i + (((i32(l_id) - 1000000i) / 6i) * 12i)) + 10i))) < l_share);
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_cv_groom(p_id: u32, p_shadow: bool) -> coax_t13 {
  var l_id: u32 = p_id;
  var l_shadow: bool = p_shadow;
  var l_vertex: i32;
  var l_at: i32;
  var l_endpoint: i32;
  var l_p0: vec3f;
  var l_p1: vec3f;
  var l_p: vec3f;
  var l_t: vec3f;
  var l_view: vec3f;
  var l_side: vec3f;
  var l_radius: f32;
  var l_z: f32;
  var l_expanded: f32;
  var l_edge: f32;
  var l_world: vec3f;
  l_vertex = (i32(l_id) - 1000000i);
  l_at = (13573828i + ((l_vertex / 6i) * 12i));
  l_endpoint = coax_p_cv_groom_endpoint((l_vertex % 6i));
  l_p0 = coax_p_cv_head(coax_p_v3(coax_p_cv_word(l_at), coax_p_cv_word((l_at + 1i)), coax_p_cv_word((l_at + 2i))));
  l_p1 = coax_p_cv_head(coax_p_v3(coax_p_cv_word((l_at + 4i)), coax_p_cv_word((l_at + 5i)), coax_p_cv_word((l_at + 6i))));
  l_p = select(l_p1, l_p0, (l_endpoint == 0i));
  l_t = coax_p_wg_normalize3(coax_p_cv_sub(l_p1, l_p0));
  l_view = coax_p_wg_normalize3(coax_p_cv_sub(coax_p_cv_at(8i), l_p));
  l_side = coax_p_wg_normalize3(coax_p_wg_cross3(l_t, l_view));
  l_radius = (coax_p_cv_word(((l_at + (l_endpoint * 4i)) + 3i)) / coax_p_cv_groom_share(l_shadow));
  l_z = coax_p_fmax(0x1.99999ap-5f, coax_p_wg_dot3(coax_p_cv_sub(l_p, coax_p_cv_at(8i)), coax_p_cv_at(20i)));
  l_expanded = coax_p_fmax(l_radius, (l_z / (coax_at_u(i32(19i)) * coax_at_u(i32(1i)))));
  l_edge = coax_p_cv_groom_side((l_vertex % 6i));
  l_world = coax_p_cv_add(l_p, coax_p_cv_scale(l_side, (l_expanded * l_edge)));
  return coax_t13(l_world, coax_p_wg_normalize3(coax_p_wg_cross3(l_side, l_t)), l_t, coax_p_v2(coax_p_cv_word(((l_at + 8i) + l_endpoint)), coax_p_cv_word((l_at + 10i))), f32(vertices[(l_at + 11i)]), (l_radius / l_expanded), l_edge);
}
fn coax_p_cv_vertex_at(p_id: u32) -> i32 {
  var l_id: u32 = p_id;
  return select(select((6052916i + ((i32(l_id) - 504367i) * 24i)), (i32(vertices[6052900i]) + ((i32(l_id) - 900000i) * 24i)), (l_id >= 900000u)), (i32(vertices[6052901i]) + ((i32(l_id) - 980000i) * 24i)), (l_id >= 980000u));
}
fn coax_p_cv_morph(p_id: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_at: i32;
  var l_p: vec3f;
  var l_n: vec3f;
  l_at = coax_p_cv_vertex_at(l_id);
  l_p = coax_p_v3(coax_p_cv_word(l_at), coax_p_cv_word((l_at + 1i)), coax_p_cv_word((l_at + 2i)));
  l_n = coax_p_v3(coax_p_cv_word((l_at + 3i)), coax_p_cv_word((l_at + 4i)), coax_p_cv_word((l_at + 5i)));
  if ((coax_at_u(i32(111i)) < 0x1.000000p-1f))
  {
    return coax_t14(l_p, l_n);
  }
  return coax_p_morph_gpu(l_p, l_n, i32(vertices[6052884i]), i32(vertices[(l_at + 20i)]), i32(vertices[(l_at + 21i)]), 4008i, i32(vertices[6052885i]));
}
fn coax_p_cv_at(p_at: i32) -> vec3f {
  var l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
}
fn coax_p_cv_skin(p_id: u32, p_p: vec4f) -> vec3f {
  var l_id: u32 = p_id;
  var l_p: vec4f = p_p;
  var l_at: i32;
  var l_out: vec3f;
  var l_sum: f32;
  var l_k: i32;
  var l_joint: i32;
  var l_weight: f32;
  var l_b: i32;
  var l_q: vec3f;
  l_at = coax_p_cv_vertex_at(l_id);
  l_out = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_sum = 0.0f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 8i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_joint = i32(((vertices[((l_at + 9i) + (l_k / 2i))] >> u32(u32(((l_k % 2i) * 16i)))) & 65535u));
      l_weight = (f32(((vertices[((l_at + 13i) + (l_k / 4i))] >> u32(u32(((l_k % 4i) * 8i)))) & 255u)) / 0x1.fe0000p7f);
      if (((l_weight > 0.0f) && (l_joint < 334i)))
      {
        l_b = (l_joint * 12i);
        l_q = coax_p_v3(coax_p_wg_dot4(coax_p_wg_at4(l_b), l_p), coax_p_wg_dot4(coax_p_wg_at4((l_b + 4i)), l_p), coax_p_wg_dot4(coax_p_wg_at4((l_b + 8i)), l_p));
        l_out = coax_p_cv_add(l_out, coax_p_cv_scale(l_q, l_weight));
        l_sum = (l_sum + l_weight);
      }
    }
  }
  return coax_p_cv_scale(l_out, (0x1.000000p0f / coax_p_fmax(l_sum, 0x1.4f8b58p-17f)));
}
fn coax_p_cv_echo_direction(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  return coax_p_v3(((coax_at_u(i32(108i)) * l_p.x) + (coax_at_u(i32(109i)) * l_p.z)), l_p.y, (((-coax_at_u(i32(109i))) * l_p.x) + (coax_at_u(i32(108i)) * l_p.z)));
}
fn coax_p_cv_add(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_cv_word(p_at: i32) -> f32 {
  var l_at: i32 = p_at;
  return bitcast<f32>(vertices[l_at]);
}
fn coax_p_ch_shadow_place(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_q: vec4f;
  var l_at: i32;
  l_q = coax_p_v4(l_p.x, l_p.y, l_p.z, 0x1.000000p0f);
  l_at = 152i;
  return coax_p_v3(coax_p_wg_dot4(coax_p_wg_at4_1(l_at), l_q), coax_p_wg_dot4(coax_p_wg_at4_1((l_at + 4i)), l_q), coax_p_wg_dot4(coax_p_wg_at4_1((l_at + 8i)), l_q));
}
fn coax_p_cv_echo_material(p_normal: vec3f, p_tangent: vec4f, p_world: vec3f, p_uv: vec2f, p_material: f32, p_dx: vec2f, p_dy: vec2f) -> coax_t19 {
  var l_normal: vec3f = p_normal;
  var l_tangent: vec4f = p_tangent;
  var l_world: vec3f = p_world;
  var l_uv: vec2f = p_uv;
  var l_material: f32 = p_material;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_n: vec3f;
  var l_albedo: vec3f;
  var l_rough: f32;
  var l_metal: f32;
  var l_skin_f0: f32;
  var l_occlusion: f32;
  var l_m: i32;
  var l_colour: i32;
  var l_normal_map: i32;
  var l_orm: i32;
  var l_a: vec4f;
  var l_a_22: vec4f;
  var l_macro: vec4f;
  var l_additional: vec4f;
  var l_highpass: vec4f;
  var l_mask: vec4f;
  var l_x: f32;
  var l_y: f32;
  var l_r: f32;
  var l_iris_uv: vec2f;
  var l_iris: vec4f;
  var l_mask_32: f32;
  var l_limbus: f32;
  var l_ic: vec3f;
  var l_sclera: vec3f;
  var l_corner: f32;
  var l_a_37: vec4f;
  var l_strength: f32;
  var l_map: vec3f;
  var l_micro: vec3f;
  var l_view: vec3f;
  var l_fresnel: f32;
  var l_amount: f32;
  var l_detail: vec3f;
  var l_brow: vec3f;
  var l_mask_46: f32;
  var l_t: vec3f;
  var l_b: vec3f;
  var l_view_49: vec3f;
  var l_nv: f32;
  var l_fuzzy: f32;
  var l_mult: f32;
  var l_gamma: f32;
  var l_rolloff: f32;
  var l_desat: f32;
  l_n = coax_p_wg_normalize3(l_normal);
  l_albedo = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_rough = 0x1.4cccccp-1f;
  l_metal = 0.0f;
  l_skin_f0 = 0x1.47ae14p-5f;
  l_occlusion = 0x1.000000p0f;
  coax_p_cv_echo_mask(l_uv, l_material, l_dx, l_dy);
  l_m = (6052404i + (i32(l_material) * 16i));
  l_albedo = coax_p_v3(coax_p_cv_word((l_m + 4i)), coax_p_cv_word((l_m + 5i)), coax_p_cv_word((l_m + 6i)));
  l_rough = coax_p_cv_word((l_m + 8i));
  l_metal = coax_p_cv_word((l_m + 7i));
  l_colour = (i32(vertices[l_m]) - 1i);
  l_normal_map = (i32(vertices[(l_m + 1i)]) - 1i);
  l_orm = (i32(vertices[(l_m + 2i)]) - 1i);
  if ((l_colour >= 0i))
  {
    l_a = coax_p_render_sample_layer8_grad(l_uv, i32(l_colour), l_dx, l_dy, 0.0f);
    l_albedo = coax_p_v3((l_albedo.x * l_a.x), (l_albedo.y * l_a.y), (l_albedo.z * l_a.z));
  }
  if ((l_orm >= 0i))
  {
    l_a_22 = coax_p_render_sample_layer8_grad(l_uv, i32(l_orm), l_dx, l_dy, 0.0f);
    l_rough = coax_p_cv_srgb(l_a_22.y);
    l_metal = coax_p_cv_srgb(l_a_22.z);
  }
  if (((l_material == 0x1.a00000p4f) || (l_material == 0x1.b00000p4f)))
  {
    l_albedo = coax_p_cv_hair_colour(l_uv.x, l_uv.y);
    l_metal = 0.0f;
    l_rough = (coax_p_mix(0x1.47ae14p-2f, 0x1.ccccccp-2f, l_uv.y) * (0x1.000000p0f + (0x1.99999ap-4f * l_uv.x)));
  }
  if ((l_material == 0x1.200000p4f))
  {
    l_albedo = coax_p_cv_scale(coax_p_v3(coax_p_mix(0x1.47ae14p-4f, 0x1.47ae14p-3f, l_uv.y), coax_p_mix(0x1.b56da0p-5f, 0x1.740e5ap-4f, l_uv.y), coax_p_mix(0x1.47ae14p-5f, 0x1.cac084p-5f, l_uv.y)), 0x1.000000p-1f);
    l_rough = 0x1.333334p-1f;
    l_metal = 0.0f;
  }
  if ((l_material == 0x1.600000p3f))
  {
    l_albedo = coax_p_cv_profile_head_colour(l_albedo, (coax_at_u(i32(114i)) < 0.0f));
    l_metal = 0.0f;
    l_macro = coax_p_render_sample_layer8_grad(l_uv, 26i, l_dx, l_dy, 0.0f);
    l_additional = coax_p_render_sample_layer8_grad(l_uv, 27i, l_dx, l_dy, 0.0f);
    l_rough = ((l_macro.x + (l_rough * 0x1.99999ap-3f)) + l_additional.x);
    l_highpass = coax_p_render_sample_layer8_grad(l_uv, 31i, l_dx, l_dy, 0.0f);
    l_mask = coax_p_render_sample_layer8_grad(l_uv, 17i, l_dx, l_dy, 0.0f);
    if ((coax_at_u(i32(114i)) >= 0.0f))
    {
      l_skin_f0 = coax_p_cv_echo_head_f0(l_highpass.x, l_highpass.y, coax_p_cv_srgb(l_mask.x));
    }
    l_occlusion = coax_p_cv_echo_head_ao(coax_p_cv_srgb(l_mask.x));
  }
  if ((l_material == 0x1.800000p2f))
  {
    l_albedo = coax_p_cv_echo_body_colour(l_albedo);
    l_rough = ((l_rough * 0x1.99999ap-3f) + 0x1.70a3d8p-2f);
    l_metal = 0.0f;
  }
  if (((l_material == 0x1.200000p3f) || (l_material == 0x1.100000p4f)))
  {
    l_x = (l_uv.x - 0x1.000000p-1f);
    l_y = (l_uv.y - 0x1.000000p-1f);
    l_r = sqrt(((l_x * l_x) + (l_y * l_y)));
    l_iris_uv = coax_p_cv_echo_pupil(coax_p_v2(((l_x / 0x1.333334p-2f) + 0x1.000000p-1f), ((l_y / 0x1.333334p-2f) + 0x1.000000p-1f)));
    l_iris = coax_p_render_sample_layer8_grad(l_iris_uv, 9i, coax_p_cv_scale2(l_dx, 0x1.aaaaaap1f), coax_p_cv_scale2(l_dy, 0x1.aaaaaap1f), 0.0f);
    l_mask_32 = (0x1.000000p0f - coax_p_smoothstep(0x1.d70a3ep-4f, 0x1.333334p-3f, l_r));
    l_limbus = (0x1.000000p0f - coax_pow_character_vs(coax_p_fmin(0x1.000000p0f, (sqrt((((l_iris_uv.x - 0x1.000000p-1f) * (l_iris_uv.x - 0x1.000000p-1f)) + ((l_iris_uv.y - 0x1.000000p-1f) * (l_iris_uv.y - 0x1.000000p-1f)))) * 0x1.133334p1f)), 0x1.000000p3f));
    l_ic = coax_p_v3(coax_pow_character_vs((((l_iris.x * 0x1.723444p-1f) * 0x1.666666p0f) * 0x1.000000p1f), 0x1.19999ap0f), coax_pow_character_vs((((l_iris.y * 0x1.49f948p-1f) * 0x1.666666p0f) * 0x1.000000p1f), 0x1.19999ap0f), coax_pow_character_vs((((l_iris.z * 0x1.3f77b0p-1f) * 0x1.666666p0f) * 0x1.000000p1f), 0x1.19999ap0f));
    l_sclera = coax_p_v3((l_albedo.x * 0x1.155560p-1f), (l_albedo.y * 0x1.fa19cap-2f), (l_albedo.z * 0x1.d7f0eep-2f));
    l_albedo = coax_p_cv_add(coax_p_cv_scale(l_sclera, (0x1.000000p0f - l_mask_32)), coax_p_cv_scale(l_ic, (l_mask_32 * l_limbus)));
    l_corner = coax_p_cv_echo_eye_shadow(l_r);
    l_albedo = coax_p_v3(((l_albedo.x * coax_p_mix(0x1.d70a3ep-3f, 0x1.000000p0f, l_corner)) * 0x1.666666p0f), ((l_albedo.y * coax_p_mix(0x1.5423dap-4f, 0x1.000000p0f, l_corner)) * 0x1.666666p0f), ((l_albedo.z * coax_p_mix(0x1.055fbcp-5f, 0x1.000000p0f, l_corner)) * 0x1.666666p0f));
  }
  if ((l_normal_map >= 0i))
  {
    l_a_37 = coax_p_render_sample_layer8_grad(l_uv, i32(l_normal_map), l_dx, l_dy, 0.0f);
    l_strength = select(select(select(select(0x1.000000p0f, 0x1.4cccccp-1f, coax_p_cv_echo_core_material(l_material)), 0x1.666666p-1f, ((l_material == 0x1.400000p2f) || (l_material == 0x1.a00000p3f))), 0x1.000000p0f, (l_material == 0x1.800000p2f)), 0x1.4cccccp-1f, (l_material == 0x1.600000p3f));
    l_map = coax_p_cv_echo_normal(l_a_37, l_strength);
    if ((l_material == 0x1.600000p3f))
    {
      l_micro = coax_p_cv_echo_normal(coax_p_render_sample_layer8_grad(l_uv, 28i, l_dx, l_dy, 0.0f), 0x1.000000p0f);
      l_view = coax_p_wg_normalize3(coax_p_cv_sub(coax_p_cv_at(8i), l_world));
      l_fresnel = coax_pow_character_vs((0x1.000000p0f - coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, l_view))), 0x1.800000p1f);
      l_amount = (0x1.000000p-1f + l_fresnel);
      l_detail = coax_p_cv_add(coax_p_cv_scale(coax_p_v3(0.0f, 0.0f, 0x1.000000p0f), (0x1.000000p0f - l_amount)), coax_p_cv_scale(l_micro, l_amount));
      l_detail = coax_p_v3((l_detail.x * 0x1.800000p-1f), (l_detail.y * 0x1.800000p-1f), l_detail.z);
      l_map = coax_p_cv_echo_normal_blend(l_map, l_detail);
      l_brow = coax_p_cv_echo_normal(coax_p_render_sample_layer8_grad(l_uv, 29i, l_dx, l_dy, 0.0f), 0x1.99999ap-4f);
      l_mask_46 = (coax_p_render_sample_layer8_grad(l_uv, 30i, l_dx, l_dy, 0.0f)).x;
      l_map = coax_p_cv_add(coax_p_cv_scale(l_map, (0x1.000000p0f - l_mask_46)), coax_p_cv_scale(l_brow, l_mask_46));
    }
    l_t = coax_p_wg_normalize3(coax_p_v3(l_tangent.x, l_tangent.y, l_tangent.z));
    l_b = coax_p_cv_scale(coax_p_wg_cross3(l_n, l_t), l_tangent.w);
    l_n = coax_p_wg_normalize3(coax_p_cv_add(coax_p_cv_scale(l_t, l_map.x), coax_p_cv_add(coax_p_cv_scale(l_b, l_map.y), coax_p_cv_scale(l_n, l_map.z))));
  }
  if (coax_p_cv_echo_core_material(l_material))
  {
    l_view_49 = coax_p_wg_normalize3(coax_p_cv_sub(coax_p_cv_at(8i), l_world));
    l_nv = coax_p_clampf(coax_p_wg_dot3(l_n, l_view_49), 0.0f, 0x1.000000p0f);
    l_fuzzy = 0x1.000000p0f;
    if (((l_material == 0x1.000000p1f) || (l_material == 0x1.700000p4f)))
    {
      l_fuzzy = (0x1.000000p0f - (0x1.99999ap-4f * l_nv));
    }
    if (((l_material == 0x1.400000p2f) || (l_material == 0x1.a00000p3f)))
    {
      l_fuzzy = coax_p_mix(0x1.000000p0f, ((0x1.000000p0f - (0x1.333334p-3f * l_nv)) + coax_pow_character_vs((0x1.000000p0f - l_nv), 0x1.ccccccp0f)), coax_p_smoothstep(0x1.f0a3d8p-1f, 0x1.f5c290p-1f, l_nv));
    }
    l_mult = select(0x1.000000p0f, 0x1.333334p-1f, ((l_material == 0x1.000000p0f) || (l_material == 0x1.e00000p3f)));
    l_gamma = select(select(0x1.000000p0f, 0x1.0cccccp0f, ((l_material == 0x1.400000p2f) || (l_material == 0x1.a00000p3f))), 0x1.19999ap0f, (l_material == 0x1.000000p3f));
    l_albedo = coax_p_v3(coax_pow_character_vs(coax_p_fmax(0.0f, ((l_albedo.x * l_fuzzy) * l_mult)), (0x1.000000p0f / l_gamma)), coax_pow_character_vs(coax_p_fmax(0.0f, ((l_albedo.y * l_fuzzy) * l_mult)), (0x1.000000p0f / l_gamma)), coax_pow_character_vs(coax_p_fmax(0.0f, ((l_albedo.z * l_fuzzy) * l_mult)), (0x1.000000p0f / l_gamma)));
    l_rolloff = select(0x1.666666p-1f, 0x1.333334p-1f, (((l_material == 0x1.400000p2f) || (l_material == 0x1.a00000p3f)) || (l_material == 0x1.000000p3f)));
    l_desat = select(select(0x1.99999ap-5f, 0x1.99999ap-4f, (((l_material == 0x1.000000p1f) || (l_material == 0x1.700000p4f)) || (l_material == 0x1.000000p3f))), 0x1.1eb852p-4f, (l_material == 0x1.800000p2f));
    l_albedo = coax_p_cv_echo_overrides(l_albedo, l_rolloff, l_desat);
    if ((l_material == 0.0f))
    {
      l_rough = ((l_rough * 0x1.333334p-1f) + 0x1.861848p-4f);
    }
    if (((l_material == 0x1.000000p0f) || (l_material == 0x1.e00000p3f)))
    {
      l_rough = (l_rough * 0x1.715f24p-1f);
    }
    if ((l_material == 0x1.000000p3f))
    {
      l_rough = (l_rough * 0x1.c4443ap-1f);
    }
    if (((l_material == 0x1.400000p2f) || (l_material == 0x1.a00000p3f)))
    {
      l_rough = (l_rough * 0x1.b703d2p-1f);
    }
  }
  return coax_t19(l_n, l_albedo, l_rough, l_metal, l_skin_f0, l_occlusion);
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_cv_sub(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_ch_shadow(p_p: vec3f, p_n: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  var l_n: vec3f = p_n;
  var l_q: vec3f;
  var l_lit: f32;
  var l_r: f32;
  var l_y: i32;
  var l_x: i32;
  if ((coax_at_u(i32(164i)) < 0x1.000000p-1f))
  {
    return 0x1.000000p0f;
  }
  l_q = coax_p_ch_shadow_place(coax_p_cv_add(l_p, coax_p_cv_scale(l_n, coax_at_u(i32(167i)))));
  if ((!((((abs(l_q.x) < 0x1.000000p0f) && (abs(l_q.y) < 0x1.000000p0f)) && (l_q.z > 0.0f)) && (l_q.z < 0x1.000000p0f))))
  {
    return 0x1.000000p0f;
  }
  l_lit = 0.0f;
  l_r = coax_at_u(i32(165i));
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 3i;
    for (l_y = coax_from; l_y < coax_to; l_y = l_y + 1i)
    {
      {
        let coax_from: i32 = 0i;
        let coax_to: i32 = 3i;
        for (l_x = coax_from; l_x < coax_to; l_x = l_x + 1i)
        {
          l_lit = (l_lit + (textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((((l_q.x * 0x1.000000p-1f) + 0x1.000000p-1f) + (f32((l_x - 1i)) * l_r)), ((0x1.000000p-1f - (l_q.y * 0x1.000000p-1f)) + (f32((l_y - 1i)) * l_r))), (l_q.z - coax_at_u(i32(166i)))) / 0x1.200000p3f));
        }
      }
    }
  }
  return coax_p_smoothstep(0x1.99999ap-4f, 0x1.800000p-1f, l_lit);
}
fn coax_p_cv_scale(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return coax_p_v3((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_ch_haze(p_c: vec3f, p_world: vec3f, p_v: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_world: vec3f = p_world;
  var l_v: vec3f = p_v;
  var l_d: f32;
  var l_f: f32;
  var l_cap: f32;
  var l_h: vec3f;
  var l_glow: f32;
  l_d = coax_p_wg_length3(coax_p_cv_sub(coax_p_cv_at(8i), l_world));
  l_f = coax_p_verse_style_haze(l_d, l_world.y, coax_at_u(i32(184i)), coax_at_u(i32(185i)), coax_at_u(i32(186i)), coax_at_u(i32(183i)), coax_at_u(i32(187i)));
  if ((l_f <= 0.0f))
  {
    return l_c;
  }
  l_cap = coax_at_u(i32(183i));
  l_h = coax_p_v3(coax_p_verse_style_haze_hue(coax_at_u(i32(180i)), 0.0f, l_f, l_cap), coax_p_verse_style_haze_hue(coax_at_u(i32(181i)), 0x1.000000p0f, l_f, l_cap), coax_p_verse_style_haze_hue(coax_at_u(i32(182i)), 0x1.000000p1f, l_f, l_cap));
  l_glow = coax_p_verse_style_haze_glow(coax_p_wg_dot3(l_v, coax_p_cv_at(24i)));
  l_h = select(coax_p_cv_add(coax_p_cv_scale(l_h, (0x1.000000p0f - l_glow)), coax_p_cv_scale(coax_p_cv_at(188i), l_glow)), coax_p_cv_add(l_h, coax_p_cv_scale(coax_p_cv_at(188i), l_glow)), (coax_at_u(i32(191i)) > 0x1.000000p-1f));
  return coax_p_cv_add(coax_p_cv_scale(l_c, (0x1.000000p0f - l_f)), coax_p_cv_scale(l_h, l_f));
}
fn coax_p_cv_groom_opacity(p_coverage: f32, p_edge: f32) -> f32 {
  var l_coverage: f32 = p_coverage;
  var l_edge: f32 = p_edge;
  return coax_p_clampf((l_coverage * (0x1.000000p0f - (abs(l_edge) * 0x1.000000p-2f))), 0.0f, 0x1.000000p0f);
}
fn coax_p_ch_out(p_c: vec3f, p_alpha: f32, p_world: vec3f) -> coax_t21 {
  var l_c: vec3f = p_c;
  var l_alpha: f32 = p_alpha;
  var l_world: vec3f = p_world;
  var l_d: f32;
  l_d = coax_p_wg_length3(coax_p_cv_sub(coax_p_cv_at(8i), l_world));
  return coax_t21(coax_p_v4(l_c.x, l_c.y, l_c.z, l_alpha), coax_p_v4(0.0f, (l_d / coax_p_fmax(coax_at_u(i32(197i)), 0x1.0624dep-10f)), 0.0f, l_alpha));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
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
fn coax_p_ch_ambient(p_ny: f32) -> vec3f {
  var l_ny: f32 = p_ny;
  return coax_p_v3(coax_p_verse_style_ambient(l_ny, coax_at_u(i32(172i)), coax_at_u(i32(176i)), 0x1.000000p0f), coax_p_verse_style_ambient(l_ny, coax_at_u(i32(173i)), coax_at_u(i32(177i)), 0x1.000000p0f), coax_p_verse_style_ambient(l_ny, coax_at_u(i32(174i)), coax_at_u(i32(178i)), 0x1.000000p0f));
}
fn coax_p_cv_mul(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_cv_slay_light(p_skin: bool, p_base: vec3f, p_f0: vec3f, p_metal: f32, p_rough: f32, p_n: vec3f, p_g: vec3f, p_l: vec3f, p_v: vec3f, p_spec_scale: f32, p_size: f32) -> vec3f {
  var l_skin: bool = p_skin;
  var l_base: vec3f = p_base;
  var l_f0: vec3f = p_f0;
  var l_metal: f32 = p_metal;
  var l_rough: f32 = p_rough;
  var l_n: vec3f = p_n;
  var l_g: vec3f = p_g;
  var l_l: vec3f = p_l;
  var l_v: vec3f = p_v;
  var l_spec_scale: f32 = p_spec_scale;
  var l_size: f32 = p_size;
  if (l_skin)
  {
    return coax_p_cv_skin_light(l_base, l_f0, l_rough, l_n, l_g, l_l, l_v, l_spec_scale, l_size);
  }
  return coax_p_cv_area_light(l_base, l_f0, l_metal, l_rough, l_n, l_l, l_v, l_spec_scale, l_size);
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_ch_pool(p_d: f32, p_reach: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_reach: f32 = p_reach;
  var l_k: f32;
  l_k = coax_p_clampf((0x1.000000p0f - ((l_d * l_d) / coax_p_fmax((l_reach * l_reach), 0x1.a36e2ep-14f))), 0.0f, 0x1.000000p0f);
  return (l_k * l_k);
}
fn coax_p_pbr_environment(p_rough: f32, p_nv: f32) -> vec2f {
  var l_rough: f32 = p_rough;
  var l_nv: f32 = p_nv;
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
  l_b = coax_pow_character_vs(0x1.000000p1f, ((-0x1.28f5c2p3f) * l_nv));
  l_term = ((select(l_b, l_a, (l_a < l_b)) * l_x) + l_y);
  return vec2f((((-0x1.0a3d70p0f) * l_term) + l_z), ((0x1.0a3d70p0f * l_term) + l_w));
}
fn coax_p_cv_specular_occlusion(p_nv: f32, p_ao: f32, p_rough: f32) -> f32 {
  var l_nv: f32 = p_nv;
  var l_ao: f32 = p_ao;
  var l_rough: f32 = p_rough;
  return coax_p_clampf(((coax_pow_character_vs((coax_p_fmax(l_nv, 0.0f) + l_ao), coax_pow_character_vs(0x1.000000p1f, (((-0x1.000000p4f) * l_rough) - 0x1.000000p0f))) - 0x1.000000p0f) + l_ao), 0.0f, 0x1.000000p0f);
}
fn coax_p_cv_echo_mask(p_uv: vec2f, p_material: f32, p_dx: vec2f, p_dy: vec2f) {
  var l_uv: vec2f = p_uv;
  var l_material: f32 = p_material;
  var l_dx: vec2f = p_dx;
  var l_dy: vec2f = p_dy;
  var l_m: i32;
  var l_alpha: i32;
  var l_a: vec4f;
  if ((l_material == 0x1.200000p4f))
  {
    if ((coax_p_cv_echo_lash_mask(l_uv) < 0x1.554c98p-2f))
    {
      discard;
    }
    return;
  }
  l_m = (6052404i + (i32(l_material) * 16i));
  if ((vertices[(l_m + 9i)] != 0u))
  {
    discard;
  }
  l_alpha = (i32(vertices[(l_m + 3i)]) - 1i);
  if ((l_alpha >= 0i))
  {
    l_a = coax_p_render_sample_layer8_grad(l_uv, i32(l_alpha), l_dx, l_dy, 0.0f);
    if ((coax_p_cv_srgb(l_a.x) < 0x1.55553ep-2f))
    {
      discard;
    }
  }
}
fn coax_p_ch_hidden() -> coax_t11 {
  return coax_t11(coax_p_v4(0.0f, 0.0f, (-0x1.000000p1f), 0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), coax_p_v2(0.0f, 0.0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f), 0x1.a00000p4f, coax_p_v4(0.0f, 0.0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_cv_hair_colour(p_along: f32, p_seed: f32) -> vec3f {
  var l_along: f32 = p_along;
  var l_seed: f32 = p_seed;
  return select(coax_p_cv_echo_groom_colour(l_along, l_seed), coax_p_cv_blonde_groom_colour(l_along, l_seed), (coax_at_u(i32(2i)) < 0.0f));
}
fn coax_p_ch_hair_light(p_albedo: vec3f, p_rough: f32, p_seed: f32, p_t: vec3f, p_world: vec3f) -> coax_t22 {
  var l_albedo: vec3f = p_albedo;
  var l_rough: f32 = p_rough;
  var l_seed: f32 = p_seed;
  var l_t: vec3f = p_t;
  var l_world: vec3f = p_world;
  var l_v: vec3f;
  var l_avg: coax_t23;
  var l_key: vec3f;
  var l_rest: vec3f;
  var l_k: i32;
  var l_at: i32;
  var l_delta: vec3f;
  var l_d: f32;
  var l_share: f32;
  l_v = coax_p_wg_normalize3(coax_p_cv_sub(coax_p_cv_at(8i), l_world));
  l_avg = coax_p_cv_hair_averages(l_albedo, (-coax_p_wg_dot3(l_t, l_v)));
  l_key = coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_fiber_light(l_albedo, l_t, coax_p_cv_at(24i), l_v, l_rough, l_seed, l_avg), 0x1.921fb6p1f), coax_p_cv_at(168i));
  l_rest = coax_p_cv_mul(l_albedo, coax_p_cv_scale(coax_p_cv_add(coax_p_cv_at(172i), coax_p_cv_at(176i)), 0x1.000000p-1f));
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 8i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      if ((f32(l_k) >= coax_at_u(i32(196i))))
      {
        break;
      }
      l_at = (200i + (l_k * 8i));
      l_delta = coax_p_cv_sub(coax_p_cv_at(l_at), l_world);
      l_d = coax_p_wg_length3(l_delta);
      l_share = coax_p_ch_pool(l_d, coax_at_u(i32((l_at + 3i))));
      if ((l_share > 0.0f))
      {
        l_rest = coax_p_cv_add(l_rest, coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_fiber_light(l_albedo, l_t, coax_p_cv_scale(l_delta, (0x1.000000p0f / coax_p_fmax(l_d, 0x1.0624dep-10f))), l_v, l_rough, l_seed, l_avg), ((0x1.921fb6p1f * l_share) * coax_at_u(i32((l_at + 7i))))), coax_p_cv_at((l_at + 4i))));
      }
    }
  }
  return coax_t22(l_key, l_rest);
}
fn coax_p_ch_clip(p_p: vec3f) -> vec4f {
  var l_p: vec3f = p_p;
  return coax_p_ch_rows(136i, l_p);
}
fn coax_p_cv_groom_share(p_shadow: bool) -> f32 {
  var l_shadow: bool = p_shadow;
  var l_f: f32;
  l_f = abs(coax_at_u(i32(2i)));
  l_f = select(0x1.000000p0f, l_f, (l_f > 0.0f));
  return select(l_f, coax_p_fmin(l_f, 0x1.000000p-2f), l_shadow);
}
fn coax_p_cv_groom_hash(p_seed: f32) -> f32 {
  var l_seed: f32 = p_seed;
  var l_h: f32;
  l_h = (sin(((l_seed * 0x1.f60000p7f) * 0x1.9fac72p3f)) * 0x1.55dd18p15f);
  return (l_h - floor(l_h));
}
fn coax_p_cv_groom_endpoint(p_vertex: i32) -> i32 {
  var l_vertex: i32 = p_vertex;
  return select(0i, 1i, (((l_vertex == 2i) || (l_vertex == 3i)) || (l_vertex == 5i)));
}
fn coax_p_cv_head(p_p: vec3f) -> vec3f {
  var l_p: vec3f = p_p;
  var l_b: i32;
  var l_q: vec4f;
  var l_local: vec3f;
  l_b = 984i;
  l_q = coax_p_v4(l_p.x, l_p.y, l_p.z, 0x1.000000p0f);
  l_local = coax_p_v3(coax_p_wg_dot4(coax_p_wg_at4(l_b), l_q), coax_p_wg_dot4(coax_p_wg_at4((l_b + 4i)), l_q), coax_p_wg_dot4(coax_p_wg_at4((l_b + 8i)), l_q));
  return coax_p_cv_add(coax_p_cv_at(104i), coax_p_cv_echo_direction(l_local));
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_cv_groom_side(p_vertex: i32) -> f32 {
  var l_vertex: i32 = p_vertex;
  return select(0x1.000000p0f, (-0x1.000000p0f), (((l_vertex == 0i) || (l_vertex == 2i)) || (l_vertex == 3i)));
}
fn coax_p_morph_gpu(p_position: vec3f, p_normal: vec3f, p_base: i32, p_first: i32, p_count: i32, p_weight_at: i32, p_targets: i32) -> coax_t14 {
  var l_position: vec3f = p_position;
  var l_normal: vec3f = p_normal;
  var l_base: i32 = p_base;
  var l_first: i32 = p_first;
  var l_count: i32 = p_count;
  var l_weight_at: i32 = p_weight_at;
  var l_targets: i32 = p_targets;
  var l_out: coax_t14;
  var l_k: i32;
  var l_at: i32;
  var l_target: i32;
  var l_weight: f32;
  l_out = coax_t14(l_position, l_normal);
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = l_count;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_at = (l_base + ((l_first + l_k) * 7i));
      l_target = i32(vertices[l_at]);
      if ((l_target >= l_targets))
      {
        continue;
      }
      l_weight = bones[(l_weight_at + l_target)];
      if ((l_weight == 0.0f))
      {
        continue;
      }
      l_out.x = coax_p_v3(coax_p_morph_axis(l_out.x.x, bitcast<f32>(vertices[(l_at + 1i)]), l_weight), coax_p_morph_axis(l_out.x.y, bitcast<f32>(vertices[(l_at + 2i)]), l_weight), coax_p_morph_axis(l_out.x.z, bitcast<f32>(vertices[(l_at + 3i)]), l_weight));
      l_out.y = coax_p_v3(coax_p_morph_axis(l_out.y.x, bitcast<f32>(vertices[(l_at + 4i)]), l_weight), coax_p_morph_axis(l_out.y.y, bitcast<f32>(vertices[(l_at + 5i)]), l_weight), coax_p_morph_axis(l_out.y.z, bitcast<f32>(vertices[(l_at + 6i)]), l_weight));
    }
  }
  return l_out;
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(bones[l_at], bones[(l_at + 1i)], bones[(l_at + 2i)], bones[(l_at + 3i)]);
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_at4_1(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
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
fn coax_p_cv_srgb(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return select(((0x1.0e147ap0f * coax_pow_character_vs(l_x, (0x1.000000p0f / 0x1.333334p1f))) - 0x1.c28f5cp-5f), (0x1.9d70a4p3f * l_x), (l_x <= 0x1.9a5c38p-9f));
}
fn coax_p_cv_profile_head_colour(p_a: vec3f, p_valley: bool) -> vec3f {
  var l_a: vec3f = p_a;
  var l_valley: bool = p_valley;
  return select(coax_p_cv_echo_slay_head_colour(l_a), coax_p_cv_echo_head_colour(l_a), l_valley);
}
fn coax_p_cv_echo_head_f0(p_r: f32, p_g: f32, p_mask: f32) -> f32 {
  var l_r: f32 = p_r;
  var l_g: f32 = p_g;
  var l_mask: f32 = p_mask;
  return (((0x1.47ae14p-4f * coax_p_mix(0x1.8fd8f0p-3f, 0x1.d5d570p-1f, l_r)) * coax_p_mix(0x1.3127f6p-2f, 0x1.000000p0f, l_g)) * l_mask);
}
fn coax_p_cv_echo_head_ao(p_r: f32) -> f32 {
  var l_r: f32 = p_r;
  return (0x1.000000p-2f + (0x1.800000p-1f * coax_pow_character_vs(coax_p_fmax(l_r, 0.0f), 0x1.99999ap0f)));
}
fn coax_p_cv_echo_body_colour(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_v3((l_a.x * 0x1.ccccccp-1f), (l_a.y * 0x1.2a09fep0f), (l_a.z * 0x1.400000p0f));
}
fn coax_p_cv_echo_pupil(p_uv: vec2f) -> vec2f {
  var l_uv: vec2f = p_uv;
  var l_x: f32;
  var l_y: f32;
  var l_r: f32;
  var l_k: f32;
  l_x = (l_uv.x - 0x1.000000p-1f);
  l_y = (l_uv.y - 0x1.000000p-1f);
  l_r = sqrt(((l_x * l_x) + (l_y * l_y)));
  if ((l_r < 0x1.0c6f7ap-20f))
  {
    return coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f);
  }
  l_k = ((0x1.000000p-1f / l_r) * (0x1.000000p0f - coax_p_clampf(((0x1.000000p0f - (l_r * 0x1.000000p1f)) * 0x1.800000p-1f), 0.0f, 0x1.000000p0f)));
  return coax_p_v2(((l_x * l_k) + 0x1.000000p-1f), ((l_y * l_k) + 0x1.000000p-1f));
}
fn coax_p_cv_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return coax_p_v2((l_a.x * l_k), (l_a.y * l_k));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_cv_echo_eye_shadow(p_radius: f32) -> f32 {
  var l_radius: f32 = p_radius;
  return coax_p_clampf(((0x1.000000p0f - (l_radius / 0x1.6147aep-2f)) / 0x1.db890ep-2f), 0.0f, 0x1.000000p0f);
}
fn coax_p_cv_echo_core_material(p_m: f32) -> bool {
  var l_m: f32 = p_m;
  return (((((((((((l_m == 0.0f) || (l_m == 0x1.000000p0f)) || (l_m == 0x1.000000p1f)) || (l_m == 0x1.000000p2f)) || (l_m == 0x1.400000p2f)) || (l_m == 0x1.800000p2f)) || (l_m == 0x1.000000p3f)) || (l_m == 0x1.a00000p3f)) || (l_m == 0x1.e00000p3f)) || (l_m == 0x1.700000p4f)) || (l_m == 0x1.900000p4f));
}
fn coax_p_cv_echo_normal(p_a: vec4f, p_strength: f32) -> vec3f {
  var l_a: vec4f = p_a;
  var l_strength: f32 = p_strength;
  return coax_p_v3((((coax_p_cv_srgb(l_a.x) * 0x1.000000p1f) - 0x1.000000p0f) * l_strength), ((0x1.000000p0f - (coax_p_cv_srgb(l_a.y) * 0x1.000000p1f)) * l_strength), ((coax_p_cv_srgb(l_a.z) * 0x1.000000p1f) - 0x1.000000p0f));
}
fn coax_p_cv_echo_normal_blend(p_base: vec3f, p_detail: vec3f) -> vec3f {
  var l_base: vec3f = p_base;
  var l_detail: vec3f = p_detail;
  var l_t: vec3f;
  var l_d: vec3f;
  l_t = coax_p_v3(l_base.x, l_base.y, (l_base.z + 0x1.000000p0f));
  l_d = coax_p_v3((-l_detail.x), (-l_detail.y), l_detail.z);
  return coax_p_cv_sub(coax_p_cv_scale(l_t, (coax_p_wg_dot3(l_t, l_d) / l_t.z)), l_d);
}
fn coax_p_cv_echo_overrides(p_base: vec3f, p_rolloff: f32, p_desat: f32) -> vec3f {
  var l_base: vec3f = p_base;
  var l_rolloff: f32 = p_rolloff;
  var l_desat: f32 = p_desat;
  var l_c: vec3f;
  var l_l: f32;
  l_c = coax_p_v3((coax_pow_character_vs(coax_p_fmax(0.0f, (l_base.x * 0x1.000000p2f)), (0x1.000000p0f / 0x1.333334p0f)) * l_rolloff), (coax_pow_character_vs(coax_p_fmax(0.0f, (l_base.y * 0x1.000000p2f)), (0x1.000000p0f / 0x1.333334p0f)) * l_rolloff), (coax_pow_character_vs(coax_p_fmax(0.0f, (l_base.z * 0x1.000000p2f)), (0x1.000000p0f / 0x1.333334p0f)) * l_rolloff));
  l_l = coax_p_wg_dot3(l_c, coax_p_v3(0x1.333334p-2f, 0x1.2e147ap-1f, 0x1.c28f5cp-4f));
  return coax_p_cv_add(coax_p_cv_scale(l_c, (0x1.000000p0f - l_desat)), coax_p_v3((l_l * l_desat), (l_l * l_desat), (l_l * l_desat)));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_verse_style_haze(p_d: f32, p_y: f32, p_base_y: f32, p_start: f32, p_density: f32, p_cap: f32, p_falloff: f32) -> f32 {
  var l_d: f32 = p_d;
  var l_y: f32 = p_y;
  var l_base_y: f32 = p_base_y;
  var l_start: f32 = p_start;
  var l_density: f32 = p_density;
  var l_cap: f32 = p_cap;
  var l_falloff: f32 = p_falloff;
  var l_dist: f32;
  var l_height: f32;
  l_dist = (0x1.000000p0f - exp(((-l_density) * coax_p_st_max((l_d - l_start), 0.0f))));
  l_height = exp(((-coax_p_st_max((l_y - l_base_y), 0.0f)) / coax_p_st_max(l_falloff, 0x1.0624dep-10f)));
  return coax_p_st_min((l_dist * (0x1.000000p0f + ((l_height - 0x1.000000p0f) * 0x1.333334p-1f))), l_cap);
}
fn coax_p_verse_style_haze_hue(p_h: f32, p_ch: f32, p_f: f32, p_cap: f32) -> f32 {
  var l_h: f32 = p_h;
  var l_ch: f32 = p_ch;
  var l_f: f32 = p_f;
  var l_cap: f32 = p_cap;
  var l_t: f32;
  var l_far: f32;
  l_t = coax_p_verse_style_haze_teal(l_ch);
  l_far = coax_p_verse_style_soft_step(((0x1.333334p-3f + l_cap) * 0x1.000000p-1f), ((l_cap - 0x1.333334p-3f) * 0x1.000000p-1f), l_f);
  return (l_h * (l_t + ((0x1.000000p0f - l_t) * l_far)));
}
fn coax_p_verse_style_haze_glow(p_vdl: f32) -> f32 {
  var l_vdl: f32 = p_vdl;
  var l_a: f32;
  var l_a2: f32;
  var l_a4: f32;
  l_a = coax_p_st_sat(l_vdl);
  l_a2 = (l_a * l_a);
  l_a4 = (l_a2 * l_a2);
  return ((l_a4 * l_a4) * 0x1.ccccccp-2f);
}
fn coax_p_verse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  var l_ny: f32 = p_ny;
  var l_sky: f32 = p_sky;
  var l_ground: f32 = p_ground;
  var l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
}
fn coax_p_cv_skin_light(p_base: vec3f, p_f0: vec3f, p_rough: f32, p_n: vec3f, p_g: vec3f, p_l: vec3f, p_v: vec3f, p_spec_scale: f32, p_size: f32) -> vec3f {
  var l_base: vec3f = p_base;
  var l_f0: vec3f = p_f0;
  var l_rough: f32 = p_rough;
  var l_n: vec3f = p_n;
  var l_g: vec3f = p_g;
  var l_l: vec3f = p_l;
  var l_v: vec3f = p_v;
  var l_spec_scale: f32 = p_spec_scale;
  var l_size: f32 = p_size;
  var l_black: vec3f;
  var l_ns: vec3f;
  var l_sr: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_nr: vec3f;
  var l_ng: vec3f;
  var l_dr: f32;
  var l_dg: f32;
  var l_db: f32;
  l_black = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_ns = coax_p_wg_normalize3(coax_p_cv_add(coax_p_cv_scale(l_n, 0x1.000000p-1f), coax_p_cv_scale(l_g, 0x1.000000p-1f)));
  l_sr = coax_p_fmax(l_rough, 0x1.99999ap-2f);
  l_a = coax_p_cv_area_light(l_black, l_f0, 0.0f, coax_p_clampf((l_sr * 0x1.800000p-1f), 0x1.70a3d8p-5f, 0x1.000000p0f), l_ns, l_l, l_v, l_spec_scale, l_size);
  l_b = coax_p_cv_area_light(l_black, l_f0, 0.0f, coax_p_clampf((l_sr * 0x1.4cccccp0f), 0x1.70a3d8p-5f, 0x1.000000p0f), l_ns, l_l, l_v, l_spec_scale, l_size);
  l_nr = coax_p_wg_normalize3(coax_p_cv_add(coax_p_cv_scale(l_n, 0x1.99999ap-2f), coax_p_cv_scale(l_g, 0x1.333334p-1f)));
  l_ng = coax_p_wg_normalize3(coax_p_cv_add(coax_p_cv_scale(l_n, 0x1.800000p-1f), coax_p_cv_scale(l_g, 0x1.000000p-2f)));
  l_dr = coax_p_fmax(0.0f, ((coax_p_wg_dot3(l_nr, l_l) + 0x1.666666p-2f) / 0x1.59999ap0f));
  l_dg = coax_p_fmax(0.0f, ((coax_p_wg_dot3(l_ng, l_l) + 0x1.eb851ep-4f) / 0x1.1eb852p0f));
  l_db = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, l_l));
  return coax_p_cv_add(coax_p_cv_add(coax_p_cv_scale(l_a, 0x1.333334p-3f), coax_p_cv_scale(l_b, 0x1.b33334p-1f)), coax_p_v3(((l_base.x * l_dr) / 0x1.921fb6p1f), ((l_base.y * l_dg) / 0x1.921fb6p1f), ((l_base.z * l_db) / 0x1.921fb6p1f)));
}
fn coax_p_cv_area_light(p_base: vec3f, p_f0: vec3f, p_metal: f32, p_rough: f32, p_n: vec3f, p_l: vec3f, p_v: vec3f, p_spec_scale: f32, p_size: f32) -> vec3f {
  var l_base: vec3f = p_base;
  var l_f0: vec3f = p_f0;
  var l_metal: f32 = p_metal;
  var l_rough: f32 = p_rough;
  var l_n: vec3f = p_n;
  var l_l: vec3f = p_l;
  var l_v: vec3f = p_v;
  var l_spec_scale: f32 = p_spec_scale;
  var l_size: f32 = p_size;
  var l_nl: f32;
  var l_h: vec3f;
  var l_nv: f32;
  var l_nh: f32;
  var l_vh: f32;
  var l_a: f32;
  var l_aa: f32;
  var l_den: f32;
  var l_k: f32;
  var l_scale: f32;
  var l_f: f32;
  l_nl = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, l_l));
  if ((l_nl == 0.0f))
  {
    return coax_p_v3(0.0f, 0.0f, 0.0f);
  }
  l_h = coax_p_wg_normalize3(coax_p_cv_add(l_l, l_v));
  l_nv = coax_p_fmax(0x1.a36e2ep-14f, coax_p_wg_dot3(l_n, l_v));
  l_nh = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, l_h));
  l_vh = coax_p_fmax(0.0f, coax_p_wg_dot3(l_v, l_h));
  l_a = (l_rough * l_rough);
  l_aa = coax_p_cv_area_a2(l_a, l_size, l_vh);
  l_den = (((l_nh * l_nh) * (l_aa - 0x1.000000p0f)) + 0x1.000000p0f);
  l_k = (((l_rough + 0x1.000000p0f) * (l_rough + 0x1.000000p0f)) / 0x1.000000p3f);
  l_scale = (((((((l_aa / ((0x1.921fb6p1f * l_den) * l_den)) * l_nv) / ((l_nv * (0x1.000000p0f - l_k)) + l_k)) * l_nl) / ((l_nl * (0x1.000000p0f - l_k)) + l_k)) / (0x1.000000p2f * l_nv)) * l_spec_scale);
  l_f = coax_pow_character_vs((0x1.000000p0f - l_vh), 0x1.400000p2f);
  return coax_p_cv_add(coax_p_cv_scale(l_base, ((l_nl * (0x1.000000p0f - l_metal)) / 0x1.921fb6p1f)), coax_p_cv_scale(coax_p_v3((l_f0.x + ((0x1.000000p0f - l_f0.x) * l_f)), (l_f0.y + ((0x1.000000p0f - l_f0.y) * l_f)), (l_f0.z + ((0x1.000000p0f - l_f0.z) * l_f))), l_scale));
}
fn coax_p_cv_echo_lash_mask(p_uv: vec2f) -> f32 {
  var l_uv: vec2f = p_uv;
  return (coax_pow_character_vs(coax_p_fmax(0.0f, (0x1.000000p0f - abs((l_uv.x - 0x1.000000p-1f)))), 0x1.800000p0f) * coax_pow_character_vs(coax_p_fmax(0.0f, l_uv.y), 0x1.ccccccp-2f));
}
fn coax_p_cv_blonde_groom_colour(p_u: f32, p_seed: f32) -> vec3f {
  var l_u: f32 = p_u;
  var l_seed: f32 = p_seed;
  var l_root: vec3f;
  var l_mid: vec3f;
  var l_tip: vec3f;
  var l_c: vec3f;
  if ((l_seed < 0x1.333334p-3f))
  {
    return coax_p_v3(0x1.baaac2p-2f, 0x1.539086p-2f, 0x1.0238dap-2f);
  }
  l_root = coax_p_v3(0x1.99999ap-2f, 0x1.1182dap-2f, 0x1.90ccd6p-3f);
  l_mid = coax_p_v3(0x1.4cccccp-1f, 0x1.b056c6p-2f, 0x1.ee043ap-3f);
  l_tip = coax_p_v3(0x1.99999ap-1f, 0x1.69d042p-1f, 0x1.0a3d70p-1f);
  l_c = select(coax_p_cv_add(l_mid, coax_p_cv_scale(coax_p_cv_sub(l_tip, l_mid), ((l_u - 0x1.19999ap-1f) / 0x1.ccccccp-2f))), coax_p_cv_add(l_root, coax_p_cv_scale(coax_p_cv_sub(l_mid, l_root), (l_u / 0x1.19999ap-1f))), (l_u < 0x1.19999ap-1f));
  return coax_p_cv_scale(l_c, coax_p_mix(0x1.99999ap-1f, 0x1.19999ap0f, ((l_seed - 0x1.333334p-3f) / 0x1.b33334p-1f)));
}
fn coax_p_cv_echo_groom_colour(p_u: f32, p_seed: f32) -> vec3f {
  var l_u: f32 = p_u;
  var l_seed: f32 = p_seed;
  var l_brightness: f32;
  if ((l_seed < 0x1.333334p-3f))
  {
    return coax_p_v3(0x1.9cc964p-3f, 0x1.00baa6p-3f, 0x1.20f3ccp-4f);
  }
  l_brightness = select(select(coax_p_mix(0x1.000000p-1f, 0.0f, ((l_seed * 0x1.800000p1f) - 0x1.000000p1f)), coax_p_mix(0x1.333334p-3f, 0x1.000000p-1f, ((l_seed * 0x1.800000p1f) - 0x1.000000p0f)), (l_seed < (0x1.000000p1f / 0x1.800000p1f))), coax_p_mix(0x1.99999ap-2f, 0x1.333334p-3f, (l_seed * 0x1.800000p1f)), (l_seed < (0x1.000000p0f / 0x1.800000p1f)));
  return coax_p_cv_scale(coax_p_v3(coax_p_mix(0x1.000000p-2f, 0x1.e00000p-3f, l_u), coax_p_mix(0x1.3d334cp-3f, 0x1.50bbf6p-3f, l_u), coax_p_mix(0x1.666666p-4f, 0x1.b00044p-4f, l_u)), l_brightness);
}
fn coax_p_cv_hair_averages(p_base: vec3f, p_sinv: f32) -> coax_t23 {
  var l_base: vec3f = p_base;
  var l_sinv: f32 = p_sinv;
  var l_x: vec3f;
  l_x = coax_p_v3(sqrt(coax_p_fmax(l_base.x, 0.0f)), sqrt(coax_p_fmax(l_base.y, 0.0f)), sqrt(coax_p_fmax(l_base.z, 0.0f)));
  return coax_t23(coax_p_v3(coax_p_cv_hair_average(l_x.x, l_sinv, false), coax_p_cv_hair_average(l_x.y, l_sinv, false), coax_p_cv_hair_average(l_x.z, l_sinv, false)), coax_p_v3(coax_p_cv_hair_average(l_x.x, l_sinv, true), coax_p_cv_hair_average(l_x.y, l_sinv, true), coax_p_cv_hair_average(l_x.z, l_sinv, true)));
}
fn coax_p_cv_fiber_light(p_base: vec3f, p_t: vec3f, p_l: vec3f, p_v: vec3f, p_rough: f32, p_seed: f32, p_avg: coax_t23) -> vec3f {
  var l_base: vec3f = p_base;
  var l_t: vec3f = p_t;
  var l_l: vec3f = p_l;
  var l_v: vec3f = p_v;
  var l_rough: f32 = p_rough;
  var l_seed: f32 = p_seed;
  var l_avg: coax_t23 = p_avg;
  var l_root: vec3f;
  var l_sl: f32;
  var l_sv: f32;
  var l_lp: vec3f;
  var l_vp: vec3f;
  var l_phi: f32;
  var l_vl: f32;
  var l_spec: f32;
  var l_back: f32;
  l_root = coax_p_cv_scale(l_t, (-0x1.000000p0f));
  l_sl = coax_p_wg_dot3(l_root, l_l);
  l_sv = coax_p_wg_dot3(l_root, l_v);
  l_lp = coax_p_cv_sub(l_l, coax_p_cv_scale(l_root, l_sl));
  l_vp = coax_p_cv_sub(l_v, coax_p_cv_scale(l_root, l_sv));
  l_phi = (coax_p_wg_dot3(l_lp, l_vp) / sqrt(((coax_p_wg_dot3(l_lp, l_lp) * coax_p_wg_dot3(l_vp, l_vp)) + 0x1.a36e2ep-14f)));
  l_vl = coax_p_wg_dot3(l_v, l_l);
  l_spec = coax_p_mix(0x1.99999ap-5f, 0x1.99999ap-4f, l_seed);
  l_back = coax_p_mix(0x1.333334p-3f, 0x1.000000p-1f, l_seed);
  return coax_p_cv_hair_dual(l_avg, l_rough, l_sl, l_sv, 0x1.333334p-2f, coax_p_v3(coax_p_hair_fiber(l_base.x, l_rough, l_spec, l_back, l_sl, l_sv, l_phi, l_vl), coax_p_hair_fiber(l_base.y, l_rough, l_spec, l_back, l_sl, l_sv, l_phi, l_vl), coax_p_hair_fiber(l_base.z, l_rough, l_spec, l_back, l_sl, l_sv, l_phi, l_vl)));
}
fn coax_p_ch_rows(p_at: i32, p_p: vec3f) -> vec4f {
  var l_at: i32 = p_at;
  var l_p: vec3f = p_p;
  var l_q: vec4f;
  l_q = coax_p_v4(l_p.x, l_p.y, l_p.z, 0x1.000000p0f);
  return coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4_1(l_at), l_q), coax_p_wg_dot4(coax_p_wg_at4_1((l_at + 4i)), l_q), coax_p_wg_dot4(coax_p_wg_at4_1((l_at + 8i)), l_q), coax_p_wg_dot4(coax_p_wg_at4_1((l_at + 12i)), l_q));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_morph_axis(p_base: f32, p_delta: f32, p_weight: f32) -> f32 {
  var l_base: f32 = p_base;
  var l_delta: f32 = p_delta;
  var l_weight: f32 = p_weight;
  if ((l_weight == 0.0f))
  {
    return l_base;
  }
  return (l_base + (l_delta * l_weight));
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
fn coax_p_cv_echo_head_colour(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_c: vec3f;
  var l_l: f32;
  l_c = coax_p_cv_scale(l_a, 0x1.400000p1f);
  l_l = (((l_c.x * 0x1.333334p-2f) + (l_c.y * 0x1.2e147ap-1f)) + (l_c.z * 0x1.c28f5cp-4f));
  return coax_p_v3((coax_p_clampf(((((l_c.x * 0x1.99999ap-1f) + (l_l * 0x1.99999ap-3f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f) * 0x1.ccccccp-1f), coax_p_clampf(((((l_c.y * 0x1.99999ap-1f) + (l_l * 0x1.99999ap-3f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f), (coax_p_clampf(((((l_c.z * 0x1.99999ap-1f) + (l_l * 0x1.99999ap-3f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f) * 0x1.e5e5f4p-1f));
}
fn coax_p_cv_echo_slay_head_colour(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_c: vec3f;
  var l_l: f32;
  l_c = coax_p_cv_scale(l_a, 0x1.400000p1f);
  l_l = (((l_c.x * 0x1.333334p-2f) + (l_c.y * 0x1.2e147ap-1f)) + (l_c.z * 0x1.c28f5cp-4f));
  return coax_p_v3((coax_p_clampf(((((l_c.x * 0x1.666666p-1f) + (l_l * 0x1.333334p-2f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f) * 0x1.d7d998p-1f), coax_p_clampf(((((l_c.y * 0x1.666666p-1f) + (l_l * 0x1.333334p-2f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f), (coax_p_clampf(((((l_c.z * 0x1.666666p-1f) + (l_l * 0x1.333334p-2f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f) * 0x1.be23d0p-1f));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
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
fn coax_p_verse_style_haze_teal(p_ch: f32) -> f32 {
  var l_ch: f32 = p_ch;
  if ((l_ch < 0x1.000000p-1f))
  {
    return 0x1.666666p-1f;
  }
  if ((l_ch < 0x1.800000p0f))
  {
    return 0x1.000000p0f;
  }
  return 0x1.ae147ap-1f;
}
fn coax_p_verse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_cv_area_a2(p_a: f32, p_size: f32, p_vh: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_size: f32 = p_size;
  var l_vh: f32 = p_vh;
  return coax_p_fmin(0x1.000000p0f, ((l_a * l_a) + (((0x1.000000p-2f * l_size) * ((0x1.800000p1f * l_a) + l_size)) / (l_vh + 0x1.0624dep-10f))));
}
fn coax_p_cv_hair_average(p_xin: f32, p_sinl: f32, p_back: bool) -> f32 {
  var l_xin: f32 = p_xin;
  var l_sinl: f32 = p_sinl;
  var l_back: bool = p_back;
  var l_s: f32;
  var l_x: f32;
  var l_v: f32;
  var l_low: f32;
  l_s = coax_p_clampf(abs(l_sinl), 0x1.000000p-3f, 0x1.c00000p-1f);
  l_x = coax_p_clampf(l_xin, 0x1.800000p-4f, 0x1.000000p0f);
  l_v = select((((-0x1.6707f6p-6f) + (l_s * (0x1.cd8f4ap-6f + (l_s * ((-0x1.545fe4p-4f) + (l_s * 0x1.dbbe02p-4f)))))) + (l_x * ((0x1.129caep2f + (l_s * ((-0x1.9d3eaap-1f) + (l_s * (0x1.a22ff0p1f + (l_s * (-0x1.c4a7cep1f))))))) + (l_x * (((-0x1.27e924p3f) + (l_s * (0x1.d1f9bep0f + (l_s * ((-0x1.cff4fep2f) + (l_s * 0x1.0483d6p3f)))))) + (l_x * ((0x1.724b34p3f + (l_s * ((-0x1.2e521ep1f) + (l_s * (0x1.292faep3f + (l_s * (-0x1.58c558p3f))))))) + (l_x * (((-0x1.fa632cp2f) + (l_s * (0x1.a74d8cp0f + (l_s * ((-0x1.9cab3ep2f) + (l_s * 0x1.ea9664p2f)))))) + (l_x * (0x1.1ebafap1f + (l_s * ((-0x1.e61984p-2f) + (l_s * (0x1.d78f36p0f + (l_s * (-0x1.1d7920p1f))))))))))))))))), ((0x1.002cc0p-5f + (l_s * (0x1.aee69ep-6f + (l_s * ((-0x1.511beap-4f) + (l_s * 0x1.522236p-4f)))))) + (l_x * ((0x1.4bf618p-6f + (l_s * (0x1.3a2fdcp-5f + (l_s * ((-0x1.d150a6p-4f) + (l_s * 0x1.33c43ep-4f)))))) + (l_x * ((0x1.7399e6p-5f + (l_s * ((-0x1.3f99c4p-3f) + (l_s * (0x1.0e0d32p-1f + (l_s * (-0x1.e4a1e4p-2f))))))) + (l_x * ((-0x1.1f7b0cp-7f) + (l_s * (0x1.4e9b44p-4f + (l_s * ((-0x1.12224ep-2f) + (l_s * 0x1.3ca3fcp-2f)))))))))))), l_back);
  l_low = select(coax_p_mix(0x1.bc9eecp-4f, 0x1.139970p-3f, ((l_s - 0x1.000000p-3f) / 0x1.800000p-1f)), coax_p_mix(0x1.160956p-5f, 0x1.7f38c6p-5f, ((l_s - 0x1.000000p-3f) / 0x1.800000p-1f)), l_back);
  return coax_p_fmin(0x1.fae148p-1f, select(l_v, coax_p_mix(l_low, l_v, coax_p_clampf(((l_xin - 0x1.000000p-5f) / 0x1.000000p-4f), 0.0f, 0x1.000000p0f)), (l_xin < 0x1.800000p-4f)));
}
fn coax_p_hair_fiber(p_base: f32, p_rough: f32, p_spec: f32, p_back: f32, p_sinL: f32, p_sinV: f32, p_phi: f32, p_voL: f32) -> f32 {
  var l_base: f32 = p_base;
  var l_rough: f32 = p_rough;
  var l_spec: f32 = p_spec;
  var l_back: f32 = p_back;
  var l_sinL: f32 = p_sinL;
  var l_sinV: f32 = p_sinV;
  var l_phi: f32 = p_phi;
  var l_voL: f32 = p_voL;
  var l_cL: f32;
  var l_cV: f32;
  var l_d: f32;
  var l_halfPhi: f32;
  var l_width: f32;
  var l_shift: f32;
  var l_reflect: f32;
  var l_eta: f32;
  var l_h: f32;
  var l_f: f32;
  var l_transmit: f32;
  var l_internal: f32;
  l_cL = sqrt(coax_p_hf_max(0.0f, (0x1.000000p0f - (l_sinL * l_sinL))));
  l_cV = sqrt(coax_p_hf_max(0.0f, (0x1.000000p0f - (l_sinV * l_sinV))));
  l_d = coax_p_hf_max(0x1.47ae14p-7f, sqrt(coax_p_hf_max(0.0f, (0x1.000000p-1f + (0x1.000000p-1f * ((l_cL * l_cV) + (l_sinL * l_sinV)))))));
  l_halfPhi = sqrt(coax_p_hf_clamp((0x1.000000p-1f + (0x1.000000p-1f * l_phi)), 0.0f, 0x1.000000p0f));
  l_width = coax_p_hf_max(0x1.010102p-8f, l_rough);
  l_width = (l_width * l_width);
  l_shift = ((0x1.000000p1f * sin((-0x1.1eb852p-4f))) * (((cos((-0x1.1eb852p-4f)) * l_halfPhi) * l_cV) + (sin((-0x1.1eb852p-4f)) * l_sinV)));
  l_reflect = ((((((coax_p_hair_gaussian(l_width, ((l_sinL + l_sinV) - l_shift)) * 0x1.000000p-2f) * l_halfPhi) * coax_p_hair_fresnel(sqrt(coax_p_hf_clamp((0x1.000000p-1f + (0x1.000000p-1f * l_voL)), 0.0f, 0x1.000000p0f)))) * l_spec) * 0x1.000000p1f) * coax_p_hf_mix(0x1.000000p0f, l_back, coax_p_hf_clamp((-l_voL), 0.0f, 0x1.000000p0f)));
  l_eta = (0x1.000000p0f / ((0x1.30a3d8p0f / l_d) + (0x1.70a3d8p-2f * l_d)));
  l_h = (l_halfPhi * (0x1.000000p0f + (l_eta * (0x1.333334p-1f - (0x1.99999ap-1f * l_phi)))));
  l_f = coax_p_hair_fresnel((l_d * sqrt(coax_p_hf_max(0.0f, (0x1.000000p0f - (l_h * l_h))))));
  l_transmit = (((((coax_p_hair_gaussian((l_width * 0x1.000000p-1f), ((l_sinL + l_sinV) - 0x1.1eb852p-5f)) * exp((((-0x1.d33334p1f) * l_phi) - 0x1.fd70a4p1f))) * (0x1.000000p0f - l_f)) * (0x1.000000p0f - l_f)) * coax_pow_character_vs(abs(l_base), ((0x1.000000p-1f * sqrt(coax_p_hf_max(0.0f, (0x1.000000p0f - (((l_h * l_h) * l_eta) * l_eta))))) / l_d))) * l_back);
  l_f = coax_p_hair_fresnel((l_d * 0x1.000000p-1f));
  l_internal = (((((coax_p_hair_gaussian((l_width * 0x1.000000p1f), ((l_sinL + l_sinV) - 0x1.1eb852p-3f)) * exp(((0x1.100000p4f * l_phi) - 0x1.0c7ae2p4f))) * (0x1.000000p0f - l_f)) * (0x1.000000p0f - l_f)) * l_f) * coax_pow_character_vs(abs(l_base), (0x1.99999ap-1f / l_d)));
  return ((l_reflect + l_transmit) + l_internal);
}
fn coax_p_cv_hair_dual(p_avg: coax_t23, p_rough: f32, p_sl: f32, p_sv: f32, p_count: f32, p_fs: vec3f) -> vec3f {
  var l_avg: coax_t23 = p_avg;
  var l_rough: f32 = p_rough;
  var l_sl: f32 = p_sl;
  var l_sv: f32 = p_sv;
  var l_count: f32 = p_count;
  var l_fs: vec3f = p_fs;
  var l_af: vec3f;
  var l_ab: vec3f;
  var l_r: f32;
  var l_br: f32;
  var l_btt: f32;
  var l_btrt: f32;
  var l_fa: f32;
  var l_ba: f32;
  var l_bf: f32;
  var l_bb: f32;
  var l_shift: f32;
  var l_theta: f32;
  l_af = l_avg.x;
  l_ab = l_avg.y;
  l_r = coax_p_clampf(l_rough, 0x1.70a3d8p-3f, 0x1.333334p-1f);
  l_br = (l_r * l_r);
  l_btt = ((l_r * l_r) * 0x1.000000p-2f);
  l_btrt = ((l_r * l_r) * 0x1.000000p2f);
  l_fa = ((l_af.x + l_af.y) + l_af.z);
  l_ba = ((l_ab.x + l_ab.y) + l_ab.z);
  l_bf = ((((l_br * l_af.x) + (l_btt * l_af.y)) + (l_btrt * l_af.z)) / l_fa);
  l_bb = ((((l_br * l_ab.x) + (l_btt * l_ab.y)) + (l_btrt * l_ab.z)) / l_ba);
  l_shift = (((((-0x1.1eb852p-4f) * l_af.x) + (0x1.1eb852p-5f * l_af.y)) + (0x1.1eb852p-3f * l_af.z)) / l_fa);
  l_theta = ((coax_p_cv_hair_asin(l_sl) + coax_p_cv_hair_asin(l_sv)) * 0x1.000000p-1f);
  return coax_p_v3(coax_p_cv_dual_channel(l_af.x, l_ab.x, l_bf, l_bb, l_shift, l_theta, l_count, l_fs.x), coax_p_cv_dual_channel(l_af.y, l_ab.y, l_bf, l_bb, l_shift, l_theta, l_count, l_fs.y), coax_p_cv_dual_channel(l_af.z, l_ab.z, l_bf, l_bb, l_shift, l_theta, l_count, l_fs.z));
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
fn coax_p_hf_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_hf_clamp(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return select(select(l_x, l_hi, (l_x > l_hi)), l_lo, (l_x < l_lo));
}
fn coax_p_hair_gaussian(p_width: f32, p_angle: f32) -> f32 {
  var l_width: f32 = p_width;
  var l_angle: f32 = p_angle;
  return (exp(((((-0x1.000000p-1f) * l_angle) * l_angle) / (l_width * l_width))) / (0x1.40d932p1f * coax_p_hf_max(l_width, 0x1.47ae14p-7f)));
}
fn coax_p_hair_fresnel(p_c: f32) -> f32 {
  var l_c: f32 = p_c;
  var l_f: f32;
  l_f = 0x1.7d18b4p-5f;
  return (l_f + ((0x1.000000p0f - l_f) * coax_pow_character_vs((0x1.000000p0f - l_c), 0x1.400000p2f)));
}
fn coax_p_hf_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_cv_hair_asin(p_s: f32) -> f32 {
  var l_s: f32 = p_s;
  return coax_p_wg_atan2(l_s, sqrt(coax_p_fmax(0.0f, (0x1.000000p0f - (l_s * l_s)))));
}
fn coax_p_cv_dual_channel(p_af: f32, p_ab: f32, p_bf: f32, p_bb: f32, p_shift: f32, p_theta: f32, p_count: f32, p_fs: f32) -> f32 {
  var l_af: f32 = p_af;
  var l_ab: f32 = p_ab;
  var l_bf: f32 = p_bf;
  var l_bb: f32 = p_bb;
  var l_shift: f32 = p_shift;
  var l_theta: f32 = p_theta;
  var l_count: f32 = p_count;
  var l_fs: f32 = p_fs;
  var l_af2: f32;
  var l_ab2: f32;
  var l_one: f32;
  var l_big: f32;
  var l_delta: f32;
  var l_q: f32;
  var l_sigma: f32;
  var l_sf2: f32;
  var l_local: f32;
  var l_global: f32;
  l_af2 = (l_af * l_af);
  l_ab2 = (l_ab * l_ab);
  l_one = (0x1.000000p0f - l_af2);
  l_big = (((l_ab * l_af2) / l_one) + (((l_ab * l_ab2) * l_af2) / ((l_one * l_one) * l_one)));
  l_delta = ((((l_shift * (0x1.000000p0f - ((0x1.000000p1f * l_ab2) / (l_one * l_one)))) * l_shift) * (((0x1.000000p1f * l_one) * l_one) + ((0x1.000000p2f * l_af2) * l_ab2))) / ((l_one * l_one) * l_one));
  l_q = sqrt((((0x1.000000p1f * l_bf) * l_bf) + (l_bb * l_bb)));
  l_sigma = (((0x1.000000p0f + (0x1.666666p-1f * l_af2)) * ((l_ab * l_q) + ((l_ab * l_ab2) * l_q))) / (l_ab + ((l_ab * l_ab2) * ((0x1.000000p1f * l_bf) + (0x1.800000p1f * l_bb)))));
  l_sf2 = ((l_bf * l_bf) * coax_p_fmax(0x1.000000p0f, l_count));
  l_local = ((((0x1.000000p1f * l_big) * exp(((((-0x1.000000p-1f) * (l_theta - l_delta)) * (l_theta - l_delta)) / (l_sf2 + (l_sigma * l_sigma))))) / 0x1.921fb6p1f) * 0x1.666666p-1f);
  l_global = coax_p_mix(0x1.000000p0f, (((coax_pow_character_vs(l_af, l_count) * exp(((((-0x1.000000p-1f) * l_theta) * l_theta) / l_sf2))) / 0x1.921fb6p1f) * 0x1.666666p-1f), coax_p_clampf(l_count, 0.0f, 0x1.000000p0f));
  return (l_global * (l_fs + l_local));
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
@vertex
fn character_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_11 {
  let coax_r = coax_p_character_vs(coax_vid, coax_iid);
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@vertex
fn caster_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_12 {
  let coax_r = coax_p_caster_vs(coax_vid, coax_iid);
  return coax_io_12(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn character_fs(coax_in: coax_io_11) -> coax_out_21 {
  let coax_r = coax_p_character_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8));
  return coax_out_21(coax_r.x, coax_r.y);
}
@fragment
fn caster_fs(coax_in: coax_io_12) {
  coax_p_caster_fs(coax_t12(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
}
fn coax_bits_character_vs(b: u32) -> f32 {
  return bitcast<f32>(b);
}
fn coax_pow_character_vs(x: f32, y: f32) -> f32 {
  if (y == 0.0f) {
    return 1.0f;
  }
  if (x == 0.0f) {
    return select(coax_bits_character_vs(0x7f800000u), 0.0f, y > 0.0f);
  }
  let r = pow(abs(x), y);
  if (!(x < 0.0f)) {
    return r;
  }
  if (floor(y) != y) {
    return coax_bits_character_vs(0x7fc00000u);
  }
  if (fract(y * 0.5f) != 0.0f) {
    return -r;
  }
  return r;
}