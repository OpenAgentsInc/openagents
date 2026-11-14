// ---- generated: apps/openagents-web/gpu/valleycanyon.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec4f,
  f5: vec4f,
  f6: f32,
  f7: f32,
  f8: vec4f,
  f9: vec4f,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec4f,
  @location(4) f5: vec4f,
  @location(5) f6: f32,
  @location(6) f7: f32,
  @location(7) f8: vec4f,
  @location(8) f9: vec4f,
}
struct coax_t12 {
  x: vec3f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: f32,
  f5: f32,
  f6: f32,
}
struct coax_t13 {
  x: vec3f,
  y: vec3f,
}
struct coax_t14 {
  x: vec3f,
  y: vec3f,
}
struct coax_t15 {
  x: vec4f,
  y: vec2f,
  z: f32,
  w: f32,
  f4: f32,
  f5: f32,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: f32,
  @location(2) w: f32,
  @location(3) f4: f32,
  @location(4) f5: f32,
}
struct coax_t16 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_in_16 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
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
}
struct coax_out_19 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
}
struct coax_t25 {
  x: i32,
  y: i32,
}
struct coax_t26 {
  x: u32,
  y: u32,
}
struct coax_t28 {
  x: vec3f,
  y: vec3f,
}
struct coax_t29 {
  x: vec3f,
  y: vec3f,
  z: f32,
  w: f32,
  f4: f32,
  f5: f32,
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 34>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> bones: array<f32>;
@group(1) @binding(2) var sheets: texture_2d_array<f32>;
@group(1) @binding(3) var sheet: sampler;
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
fn coax_p_bg_vs(p_i: u32, p_j: u32) -> vec4f {
  let l_i: u32 = p_i;
  let l_j: u32 = p_j;
  return coax_p_splash_bg_vs(l_i, l_j);
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t11 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  var l_world: vec3f;
  var l_at: i32;
  var l_normal: vec3f;
  var l_uv: vec2f;
  var l_paint: vec4f;
  var l_tangent: vec4f;
  var l_material: f32;
  var l_character: f32;
  var l_extra: vec4f;
  var l_extra2: vec4f;
  var l_groom: coax_t12;
  var l_lit: coax_t13;
  var l_t: vec3f;
  var l_m: coax_t14;
  var l_t_19: vec3f;
  var l_rect: vec3f;
  var l_n: vec3f;
  var l_m_22: i32;
  var l_r: vec3f;
  var l_z: f32;
  l_world = coax_p_cv_world(l_id, l_instance);
  l_at = (i32(l_id) * 12i);
  l_normal = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_uv = coax_p_v2(0.0f, 0.0f);
  l_paint = coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_tangent = coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f);
  l_material = 0.0f;
  l_character = 0.0f;
  l_extra = coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_extra2 = coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
  if (((l_id >= 1000000u) && (!coax_p_cv_groom_kept(l_id, false))))
  {
    return coax_t11(coax_p_v4(0.0f, 0.0f, (-0x1.000000p1f), 0x1.000000p0f), coax_p_v3(0.0f, 0.0f, 0.0f), l_normal, l_uv, l_paint, l_tangent, 0x1.000000p0f, 0x1.a00000p4f, l_extra, l_extra2);
  }
  if ((l_id >= 1000000u))
  {
    l_groom = coax_p_cv_groom(l_id, false);
    l_normal = l_groom.y;
    l_tangent = coax_p_v4(l_groom.z.x, l_groom.z.y, l_groom.z.z, 0x1.000000p0f);
    l_uv = l_groom.w;
    l_paint = coax_p_v4(l_groom.f5, l_groom.f6, 0x1.000000p0f, 0x1.000000p0f);
    l_character = 0x1.000000p0f;
    l_material = l_groom.f4;
    l_lit = coax_p_cv_hair_vertex_light(coax_p_cv_hair_colour(l_groom.w.x, l_groom.w.y), (coax_p_mix(0x1.47ae14p-2f, 0x1.ccccccp-2f, l_groom.w.y) * (0x1.000000p0f + (0x1.99999ap-4f * l_groom.w.x))), l_groom.w.y, coax_p_wg_normalize3(l_groom.z), l_world);
    l_extra = coax_p_v4(l_lit.x.x, l_lit.x.y, l_lit.x.z, 0x1.000000p0f);
    l_extra2 = coax_p_v4(l_lit.y.x, l_lit.y.y, l_lit.y.z, 0.0f);
  }
  else
  {
    if ((l_id >= 980000u))
    {
      l_at = coax_p_cv_vertex_at(l_id);
      l_normal = coax_p_cv_orb_rotate(coax_p_v3(coax_p_cv_word((l_at + 3i)), coax_p_cv_word((l_at + 4i)), coax_p_cv_word((l_at + 5i))));
      l_t = coax_p_cv_orb_rotate(coax_p_v3(coax_p_cv_word((l_at + 15i)), coax_p_cv_word((l_at + 16i)), coax_p_cv_word((l_at + 17i))));
      l_tangent = coax_p_v4(l_t.x, l_t.y, l_t.z, coax_p_cv_word((l_at + 18i)));
      l_character = 0x1.000000p0f;
      l_material = 0x1.d00000p4f;
    }
    else
    {
      if ((l_instance == 2u))
      {
        l_at = coax_p_cv_vertex_at(l_id);
        l_m = coax_p_cv_morph(l_id);
        l_normal = coax_p_cv_echo_direction(coax_p_cv_skin(l_id, coax_p_v4(l_m.y.x, l_m.y.y, l_m.y.z, 0.0f)));
        l_t_19 = coax_p_cv_echo_direction(coax_p_cv_skin(l_id, coax_p_v4(coax_p_cv_word((l_at + 15i)), coax_p_cv_word((l_at + 16i)), coax_p_cv_word((l_at + 17i)), 0.0f)));
        l_tangent = coax_p_v4(l_t_19.x, l_t_19.y, l_t_19.z, coax_p_cv_word((l_at + 18i)));
        l_character = 0x1.000000p0f;
        l_material = f32(vertices[(l_at + 8i)]);
        if ((coax_at_u(i32(112i)) > 0x1.000000p-1f))
        {
          l_rect = coax_p_cv_rect_delta(l_world);
          if ((coax_p_cv_rect_power(l_rect) > 0.0f))
          {
            l_extra.x = coax_p_cv_hand_visibility(l_world, coax_p_cv_add(l_world, l_rect));
          }
        }
      }
      else
      {
        l_n = coax_p_v3(coax_p_cv_word((l_at + 3i)), coax_p_cv_word((l_at + 4i)), coax_p_cv_word((l_at + 5i)));
        l_m_22 = (44i + (i32(l_instance) * 24i));
        l_normal = coax_p_v3(coax_p_wg_dot3(coax_p_cv_at(l_m_22), l_n), coax_p_wg_dot3(coax_p_cv_at((l_m_22 + 4i)), l_n), coax_p_wg_dot3(coax_p_cv_at((l_m_22 + 8i)), l_n));
        l_paint = coax_p_wg_unpack4x8unorm(vertices[(l_at + 8i)]);
      }
    }
  }
  if ((l_id < 1000000u))
  {
    l_uv = coax_p_v2(coax_p_cv_word((l_at + 6i)), coax_p_cv_word((l_at + 7i)));
  }
  l_r = coax_p_cv_sub(l_world, coax_p_cv_at(8i));
  l_z = coax_p_wg_dot3(l_r, coax_p_cv_at(20i));
  return coax_t11(coax_p_v4((coax_p_wg_dot3(l_r, coax_p_cv_at(12i)) * coax_at_u(i32(15i))), (coax_p_wg_dot3(l_r, coax_p_cv_at(16i)) * coax_at_u(i32(19i))), (0x1.000aecp0f * (l_z - 0x1.99999ap-5f)), l_z), l_world, l_normal, l_uv, l_paint, l_tangent, l_character, l_material, l_extra, l_extra2);
}
fn coax_p_shadow_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  var l_groom: coax_t12;
  var l_g: vec3f;
  var l_q: vec3f;
  var l_uv: vec2f;
  var l_material: f32;
  var l_coverage: f32;
  var l_edge: f32;
  var l_groom_12: coax_t12;
  var l_at: i32;
  if (((l_id >= 1000000u) && (!coax_p_cv_groom_kept(l_id, true))))
  {
    return coax_t15(coax_p_v4(0.0f, 0.0f, (-0x1.000000p1f), 0x1.000000p0f), coax_p_v2(0.0f, 0.0f), 0x1.000000p0f, 0x1.a00000p4f, 0.0f, 0.0f);
  }
  if ((l_id >= 1000000u))
  {
    l_groom = coax_p_cv_groom(l_id, true);
    l_g = coax_p_cv_shadow(l_groom.x);
    return coax_t15(coax_p_v4(l_g.x, l_g.y, l_g.z, 0x1.000000p0f), l_groom.w, select(0.0f, 0x1.000000p0f, (l_instance == 2u)), l_groom.f4, l_groom.f5, l_groom.f6);
  }
  l_q = coax_p_cv_shadow(coax_p_cv_world(l_id, l_instance));
  l_uv = coax_p_v2(0.0f, 0.0f);
  l_material = 0.0f;
  l_coverage = 0x1.000000p0f;
  l_edge = 0.0f;
  if ((l_id >= 1000000u))
  {
    l_groom_12 = coax_p_cv_groom(l_id, true);
    l_uv = l_groom_12.w;
    l_material = l_groom_12.f4;
    l_coverage = l_groom_12.f5;
    l_edge = l_groom_12.f6;
  }
  else
  {
    if ((l_instance == 2u))
    {
      l_at = coax_p_cv_vertex_at(l_id);
      l_uv = coax_p_v2(coax_p_cv_word((l_at + 6i)), coax_p_cv_word((l_at + 7i)));
      l_material = f32(vertices[(l_at + 8i)]);
    }
  }
  return coax_t15(coax_p_v4(l_q.x, l_q.y, l_q.z, 0x1.000000p0f), l_uv, select(0.0f, 0x1.000000p0f, (l_instance == 2u)), l_material, l_coverage, l_edge);
}
fn coax_p_fx_vs(p_i: u32, p_j: u32, p_a: coax_t16) -> coax_t18 {
  let l_i: u32 = p_i;
  let l_j: u32 = p_j;
  let l_a: coax_t16 = p_a;
  return coax_p_splash_fx_vs(l_i, l_j, coax_t17(l_a.x, l_a.y, l_a.z, coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f)));
}
fn coax_p_bg_fs(p_p: vec4f) -> coax_t19 {
  let l_p: vec4f = p_p;
  var l_c: vec4f;
  l_c = coax_p_cv_bg_colour(l_p);
  return coax_t19(l_c, coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f));
}
fn coax_p_scene_fs(p_s: coax_t11) -> coax_t19 {
  let l_s: coax_t11 = p_s;
  var l_dx: vec2f;
  var l_dy: vec2f;
  var l_c: vec4f;
  var l_skin: f32;
  if (((coax_at_u(i32(112i)) > 0x1.000000p-1f) && (abs((0x1.000000p0f - ((l_s.x.y / coax_at_u(i32(1i))) * 0x1.000000p1f))) > ((coax_at_u(i32(0i)) / coax_at_u(i32(1i))) / coax_at_u(i32(113i))))))
  {
    discard;
  }
  l_dx = coax_p_v2(dpdxFine(l_s.w.x), dpdxFine(l_s.w.y));
  l_dy = coax_p_v2(dpdyFine(l_s.w.x), dpdyFine(l_s.w.y));
  l_c = coax_p_cv_scene_shade(l_s, l_dx, l_dy);
  l_skin = select(0.0f, 0x1.000000p0f, ((l_s.f6 > 0x1.000000p-1f) && ((l_s.f7 == 0x1.600000p3f) || (l_s.f7 == 0x1.800000p2f))));
  return coax_t19(l_c, coax_p_v4(l_skin, coax_p_cv_view_z(l_s.x.z), 0x1.000000p0f, l_c.w));
}
fn coax_p_shadow_fs(p_s: coax_t15) {
  let l_s: coax_t15 = p_s;
  var l_noise: f32;
  var l_dx: vec2f;
  var l_dy: vec2f;
  if ((l_s.w == 0x1.d00000p4f))
  {
    discard;
  }
  if (((l_s.w == 0x1.a00000p4f) || (l_s.w == 0x1.b00000p4f)))
  {
    l_noise = (sin((((floor(l_s.x.x) * 0x1.9fac72p3f) + (floor(l_s.x.y) * 0x1.38ee98p6f)) + (l_s.y.y * 0x1.b50000p8f))) * 0x1.55dd18p15f);
    if ((coax_p_cv_groom_opacity(l_s.f4, l_s.f5) < (l_noise - floor(l_noise))))
    {
      discard;
    }
  }
  l_dx = coax_p_v2(dpdxFine(l_s.y.x), dpdxFine(l_s.y.y));
  l_dy = coax_p_v2(dpdyFine(l_s.y.x), dpdyFine(l_s.y.y));
  if ((l_s.z > 0x1.000000p-1f))
  {
    coax_p_cv_echo_mask(l_s.y, l_s.w, l_dx, l_dy);
  }
}
fn coax_p_bloom_fs(p_pos: vec4f) -> vec4f {
  let l_pos: vec4f = p_pos;
  return coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f);
}
fn coax_p_post_fs(p_pos: vec4f) -> vec4f {
  let l_pos: vec4f = p_pos;
  var l_px: coax_t25;
  var l_size: coax_t26;
  var l_width: f32;
  var l_raw: vec4f;
  var l_zc: f32;
  var l_rc: f32;
  var l_centre: vec3f;
  var l_skin: f32;
  var l_per_mm: f32;
  var l_reach: f32;
  var l_sum: vec3f;
  var l_wr: f32;
  var l_wgt: f32;
  var l_wb: f32;
  var l_k: i32;
  var l_r: f32;
  var l_a: f32;
  var l_q: coax_t25;
  var l_c: vec3f;
  var l_mm: f32;
  var l_kr: f32;
  var l_kg: f32;
  var l_kb: f32;
  var l_blurred: vec3f;
  var l_reach_32: f32;
  var l_k_33: i32;
  var l_a_34: f32;
  var l_q_35: coax_t25;
  var l_z: f32;
  var l_colour: vec3f;
  var l_total: f32;
  var l_k_39: i32;
  var l_r_40: f32;
  var l_a_41: f32;
  var l_q_42: coax_t25;
  var l_z_43: f32;
  var l_sz: f32;
  var l_m: f32;
  var l_c_46: vec3f;
  var l_mean: vec3f;
  l_px = coax_p_i2(coax_p_wg_i32_f(l_pos.x), coax_p_wg_i32_f(l_pos.y));
  l_size = coax_p_wg_dims_2d();
  l_width = f32(l_size.x);
  l_raw = coax_p_wg_load_2d(l_px, 0i);
  if (((coax_at_u(i32(112i)) < 0x1.000000p-1f) || (coax_at_u(i32(129i)) <= 0.0f)))
  {
    return coax_p_v4(l_raw.x, l_raw.y, l_raw.z, 0x1.000000p0f);
  }
  l_zc = coax_p_cv_post_z(l_px);
  l_rc = coax_p_cv_coc(l_zc, l_width);
  l_centre = coax_p_cv_light_at(l_px, l_size);
  l_skin = (coax_p_wg_load_2d_1(l_px, 0i)).x;
  if ((l_skin > 0x1.47ae14p-7f))
  {
    l_per_mm = ((((0x1.0624dep-10f * coax_at_u(i32(130i))) / 0x1.97cc3ap-6f) * l_width) / l_zc);
    l_reach = coax_p_fmin(((0x1.800000p1f * 0x1.99999ap0f) * l_per_mm), 0x1.800000p3f);
    l_sum = l_centre;
    l_wr = 0x1.000000p0f;
    l_wgt = 0x1.000000p0f;
    l_wb = 0x1.000000p0f;
    {
      let coax_from: i32 = 0i;
      let coax_to: i32 = 24i;
      for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
      {
        l_r = (l_reach * sqrt(((f32(l_k) + 0x1.000000p-1f) / 0x1.800000p4f)));
        l_a = (f32(l_k) * 0x1.3331fep1f);
        l_q = coax_p_i2((l_px.x + coax_p_wg_i32_f(((l_r * cos(l_a)) + 0x1.000000p-1f))), (l_px.y + coax_p_wg_i32_f(((l_r * sin(l_a)) + 0x1.000000p-1f))));
        l_q = coax_p_i2(coax_p_clamp_s(l_q.x, 0i, (i32(l_size.x) - 1i)), coax_p_clamp_s(l_q.y, 0i, (i32(l_size.y) - 1i)));
        if ((((coax_p_wg_load_2d_1(l_q, 0i)).x < 0x1.000000p-1f) || (abs((coax_p_cv_view_z(coax_p_wg_load_depth(l_q, 0i)) - l_zc)) > 0x1.89374cp-7f)))
        {
          continue;
        }
        l_c = coax_p_cv_light_at(l_q, l_size);
        l_mm = (l_r / l_per_mm);
        l_kr = exp((((-l_mm) * l_mm) / ((0x1.000000p1f * 0x1.99999ap0f) * 0x1.99999ap0f)));
        l_kg = exp((((-l_mm) * l_mm) / ((0x1.000000p1f * 0x1.800000p-1f) * 0x1.800000p-1f)));
        l_kb = exp((((-l_mm) * l_mm) / ((0x1.000000p1f * 0x1.99999ap-2f) * 0x1.99999ap-2f)));
        l_sum = coax_p_v3((l_sum.x + (l_c.x * l_kr)), (l_sum.y + (l_c.y * l_kg)), (l_sum.z + (l_c.z * l_kb)));
        l_wr = (l_wr + l_kr);
        l_wgt = (l_wgt + l_kg);
        l_wb = (l_wb + l_kb);
      }
    }
    l_blurred = coax_p_v3((l_sum.x / l_wr), (l_sum.y / l_wgt), (l_sum.z / l_wb));
    l_centre = coax_p_cv_add(coax_p_cv_scale(l_centre, (0x1.000000p0f - l_skin)), coax_p_cv_scale(l_blurred, l_skin));
  }
  l_reach_32 = l_rc;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 8i;
    for (l_k_33 = coax_from; l_k_33 < coax_to; l_k_33 = l_k_33 + 1i)
    {
      l_a_34 = (f32(l_k_33) * 0x1.921fb0p-1f);
      l_q_35 = coax_p_i2((l_px.x + coax_p_wg_i32_f((cos(l_a_34) * 0x1.c00000p3f))), (l_px.y + coax_p_wg_i32_f((sin(l_a_34) * 0x1.c00000p3f))));
      l_q_35 = coax_p_i2(coax_p_clamp_s(l_q_35.x, 0i, (i32(l_size.x) - 1i)), coax_p_clamp_s(l_q_35.y, 0i, (i32(l_size.y) - 1i)));
      l_z = coax_p_cv_post_z(l_q_35);
      if ((l_z < l_zc))
      {
        l_reach_32 = coax_p_fmax(l_reach_32, coax_p_cv_coc(l_z, l_width));
      }
    }
  }
  l_colour = l_centre;
  if ((l_reach_32 > 0x1.000000p-1f))
  {
    l_total = 0x1.000000p0f;
    {
      let coax_from: i32 = 0i;
      let coax_to: i32 = 48i;
      for (l_k_39 = coax_from; l_k_39 < coax_to; l_k_39 = l_k_39 + 1i)
      {
        l_r_40 = (l_reach_32 * sqrt(((f32(l_k_39) + 0x1.000000p-1f) / 0x1.800000p5f)));
        l_a_41 = (f32(l_k_39) * 0x1.3331fep1f);
        l_q_42 = coax_p_i2(coax_p_clamp_s((l_px.x + coax_p_wg_i32_f(((l_r_40 * cos(l_a_41)) + 0x1.000000p-1f))), 0i, (i32(l_size.x) - 1i)), coax_p_clamp_s((l_px.y + coax_p_wg_i32_f(((l_r_40 * sin(l_a_41)) + 0x1.000000p-1f))), 0i, (i32(l_size.y) - 1i)));
        l_z_43 = coax_p_cv_post_z(l_q_42);
        l_sz = coax_p_cv_coc(l_z_43, l_width);
        if ((l_z_43 > l_zc))
        {
          l_sz = coax_p_fmin(l_sz, (l_rc * 0x1.000000p1f));
        }
        l_m = coax_p_smoothstep((l_r_40 - 0x1.000000p-1f), (l_r_40 + 0x1.000000p-1f), l_sz);
        l_c_46 = coax_p_cv_light_at(l_q_42, l_size);
        l_mean = coax_p_cv_scale(l_colour, (0x1.000000p0f / l_total));
        l_colour = coax_p_cv_add(l_colour, coax_p_cv_add(coax_p_cv_scale(l_mean, (0x1.000000p0f - l_m)), coax_p_cv_scale(l_c_46, l_m)));
        l_total = (l_total + 0x1.000000p0f);
      }
    }
    l_colour = coax_p_cv_scale(l_colour, (0x1.000000p0f / l_total));
  }
  return coax_p_v4(coax_p_cv_srgb(coax_p_clampf(l_colour.x, 0.0f, 0x1.000000p0f)), coax_p_cv_srgb(coax_p_clampf(l_colour.y, 0.0f, 0x1.000000p0f)), coax_p_cv_srgb(coax_p_clampf(l_colour.z, 0.0f, 0x1.000000p0f)), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t18) -> vec4f {
  let l_v: coax_t18 = p_v;
  return coax_p_splash_fx_fs(l_v);
}
fn coax_p_splash_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  let l_i: u32 = p_i;
  let l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_cv_world(p_id: u32, p_instance: u32) -> vec3f {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  var l_at: i32;
  var l_m: coax_t14;
  var l_p: vec4f;
  var l_at_8: i32;
  var l_p_9: vec4f;
  var l_m_10: i32;
  if ((l_id >= 1000000u))
  {
    return (coax_p_cv_groom(l_id, false)).x;
  }
  if ((l_id >= 980000u))
  {
    l_at = coax_p_cv_vertex_at(l_id);
    return coax_p_cv_add(coax_p_cv_at(116i), coax_p_cv_orb_rotate(coax_p_cv_scale(coax_p_v3(coax_p_cv_word(l_at), coax_p_cv_word((l_at + 1i)), coax_p_cv_word((l_at + 2i))), (0x1.333334p-5f * 0x1.000000p1f))));
  }
  if ((l_instance == 2u))
  {
    l_m = coax_p_cv_morph(l_id);
    l_p = coax_p_v4(l_m.x.x, l_m.x.y, l_m.x.z, 0x1.000000p0f);
    return coax_p_cv_add(coax_p_cv_at(104i), coax_p_cv_echo_direction(coax_p_cv_skin(l_id, l_p)));
  }
  l_at_8 = (i32(l_id) * 12i);
  l_p_9 = coax_p_v4(coax_p_cv_word(l_at_8), coax_p_cv_word((l_at_8 + 1i)), coax_p_cv_word((l_at_8 + 2i)), 0x1.000000p0f);
  l_m_10 = (32i + (i32(l_instance) * 24i));
  return coax_p_v3(coax_p_wg_dot4(coax_p_wg_at4(l_m_10), l_p_9), coax_p_wg_dot4(coax_p_wg_at4((l_m_10 + 4i)), l_p_9), coax_p_wg_dot4(coax_p_wg_at4((l_m_10 + 8i)), l_p_9));
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
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
fn coax_p_cv_groom_kept(p_id: u32, p_shadow: bool) -> bool {
  let l_id: u32 = p_id;
  let l_shadow: bool = p_shadow;
  var l_share: f32;
  l_share = coax_p_cv_groom_share(l_shadow);
  if ((l_share >= 0x1.000000p0f))
  {
    return true;
  }
  return (coax_p_cv_groom_hash(coax_p_cv_word(((13573828i + (((i32(l_id) - 1000000i) / 6i) * 12i)) + 10i))) < l_share);
}
fn coax_p_cv_groom(p_id: u32, p_shadow: bool) -> coax_t12 {
  let l_id: u32 = p_id;
  let l_shadow: bool = p_shadow;
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
  return coax_t12(l_world, coax_p_wg_normalize3(coax_p_wg_cross3(l_side, l_t)), l_t, coax_p_v2(coax_p_cv_word(((l_at + 8i) + l_endpoint)), coax_p_cv_word((l_at + 10i))), f32(vertices[(l_at + 11i)]), (l_radius / l_expanded), l_edge);
}
fn coax_p_cv_hair_colour(p_along: f32, p_seed: f32) -> vec3f {
  let l_along: f32 = p_along;
  let l_seed: f32 = p_seed;
  return select(coax_p_cv_echo_groom_colour(l_along, l_seed), coax_p_cv_blonde_groom_colour(l_along, l_seed), (coax_at_u(i32(2i)) < 0.0f));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_cv_hair_vertex_light(p_albedo: vec3f, p_rough: f32, p_seed: f32, p_t: vec3f, p_world: vec3f) -> coax_t13 {
  let l_albedo: vec3f = p_albedo;
  let l_rough: f32 = p_rough;
  let l_seed: f32 = p_seed;
  let l_t: vec3f = p_t;
  let l_world: vec3f = p_world;
  var l_v: vec3f;
  var l_avg: coax_t28;
  var l_slay: bool;
  var l_key: vec3f;
  var l_rest: vec3f;
  var l_rect: vec3f;
  var l_lights: i32;
  var l_light: i32;
  var l_dir: vec3f;
  var l_power: f32;
  var l_tint: vec3f;
  var l_c: vec3f;
  l_v = coax_p_wg_normalize3(coax_p_cv_sub(coax_p_cv_at(8i), l_world));
  l_avg = coax_p_cv_hair_averages(l_albedo, (-coax_p_wg_dot3(l_t, l_v)));
  l_slay = (coax_at_u(i32(112i)) > 0x1.000000p-1f);
  l_key = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_rest = coax_p_cv_scale(l_albedo, select(0x1.8c0000p-1f, 0x1.47ae14p-4f, l_slay));
  l_rect = coax_p_cv_rect_delta(l_world);
  l_lights = select(1i, 5i, l_slay);
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = l_lights;
    for (l_light = coax_from; l_light < coax_to; l_light = l_light + 1i)
    {
      l_dir = select(select(select(select(coax_p_wg_normalize3(l_rect), coax_p_cv_kick_dir(), (l_light == 3i)), coax_p_cv_at(28i), (l_light == 2i)), coax_p_cv_at(97i), (l_light == 1i)), coax_p_cv_at(24i), (l_light == 0i));
      l_power = select(select(select(select(coax_p_cv_rect_power(l_rect), 0x1.941ed0p0f, (l_light == 3i)), (coax_at_u(i32(31i)) * 0x1.0ace90p-2f), (l_light == 2i)), (coax_at_u(i32(115i)) * 0x1.941ed0p-1f), (l_light == 1i)), select(0x1.c00000p1f, coax_at_u(i32(123i)), l_slay), (l_light == 0i));
      l_tint = select(select(select(select(coax_p_v3(coax_at_u(i32(23i)), coax_at_u(i32(27i)), coax_at_u(i32(95i))), coax_p_v3(0x1.000000p0f, 0x1.eded28p-1f, 0x1.e978d4p-1f), (l_light == 3i)), coax_p_cv_at(124i), (l_light == 2i)), coax_p_v3(0x1.000000p0f, 0x1.9ba010p-2f, 0x1.9cc986p-3f), (l_light == 1i)), select(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_cv_at(120i), l_slay), (l_light == 0i));
      l_c = coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_fiber_light(l_albedo, l_t, l_dir, l_v, l_rough, l_seed, l_avg), l_power), l_tint);
      if ((l_light == 0i))
      {
        l_key = l_c;
      }
      else
      {
        l_rest = coax_p_cv_add(l_rest, l_c);
      }
    }
  }
  return coax_t13(l_key, l_rest);
}
fn coax_p_cv_vertex_at(p_id: u32) -> i32 {
  let l_id: u32 = p_id;
  return select(select((6052916i + ((i32(l_id) - 504367i) * 24i)), (i32(vertices[6052900i]) + ((i32(l_id) - 900000i) * 24i)), (l_id >= 900000u)), (i32(vertices[6052901i]) + ((i32(l_id) - 980000i) * 24i)), (l_id >= 980000u));
}
fn coax_p_cv_word(p_at: i32) -> f32 {
  let l_at: i32 = p_at;
  return bitcast<f32>(vertices[l_at]);
}
fn coax_p_cv_orb_rotate(p_p: vec3f) -> vec3f {
  let l_p: vec3f = p_p;
  var l_q: vec3f;
  l_q = coax_p_cv_at(100i);
  return coax_p_cv_add(l_p, coax_p_cv_scale(coax_p_wg_cross3(l_q, coax_p_cv_add(coax_p_wg_cross3(l_q, l_p), coax_p_cv_scale(l_p, coax_at_u(i32(103i))))), 0x1.000000p1f));
}
fn coax_p_cv_morph(p_id: u32) -> coax_t14 {
  let l_id: u32 = p_id;
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
fn coax_p_cv_skin(p_id: u32, p_p: vec4f) -> vec3f {
  let l_id: u32 = p_id;
  let l_p: vec4f = p_p;
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
        l_q = coax_p_v3(coax_p_wg_dot4(coax_p_wg_at4_1(l_b), l_p), coax_p_wg_dot4(coax_p_wg_at4_1((l_b + 4i)), l_p), coax_p_wg_dot4(coax_p_wg_at4_1((l_b + 8i)), l_p));
        l_out = coax_p_cv_add(l_out, coax_p_cv_scale(l_q, l_weight));
        l_sum = (l_sum + l_weight);
      }
    }
  }
  return coax_p_cv_scale(l_out, (0x1.000000p0f / coax_p_fmax(l_sum, 0x1.4f8b58p-17f)));
}
fn coax_p_cv_echo_direction(p_p: vec3f) -> vec3f {
  let l_p: vec3f = p_p;
  return coax_p_v3(((coax_at_u(i32(108i)) * l_p.x) + (coax_at_u(i32(109i)) * l_p.z)), l_p.y, (((-coax_at_u(i32(109i))) * l_p.x) + (coax_at_u(i32(108i)) * l_p.z)));
}
fn coax_p_cv_rect_delta(p_world: vec3f) -> vec3f {
  let l_world: vec3f = p_world;
  return coax_p_v3((coax_at_u(i32(83i)) - l_world.x), (coax_at_u(i32(87i)) - l_world.y), (coax_at_u(i32(91i)) - l_world.z));
}
fn coax_p_cv_rect_power(p_rect: vec3f) -> f32 {
  let l_rect: vec3f = p_rect;
  var l_d2: f32;
  var l_aim: vec3f;
  l_d2 = coax_p_wg_dot3(l_rect, l_rect);
  l_aim = coax_p_cv_oct_decode(coax_at_u(i32(11i)), coax_at_u(i32(127i)));
  return (coax_p_slay_candela_scale(l_d2, coax_at_u(i32(107i))) * coax_p_slay_rect_emission(((-coax_p_wg_dot3(l_aim, l_rect)) / sqrt(coax_p_fmax(l_d2, 0x1.0c6f7ap-20f)))));
}
fn coax_p_cv_add(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_cv_hand_visibility(p_p: vec3f, p_light: vec3f) -> f32 {
  let l_p: vec3f = p_p;
  let l_light: vec3f = p_light;
  var l_a0: vec3f;
  var l_b0: vec3f;
  var l_q: vec3f;
  var l_l: vec3f;
  var l_hand: i32;
  var l_centre: vec3f;
  var l_visible: f32;
  var l_k: i32;
  var l_at: i32;
  var l_r: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  l_a0 = coax_p_cv_sub(l_p, coax_p_cv_at(104i));
  l_b0 = coax_p_cv_sub(l_light, coax_p_cv_at(104i));
  l_q = coax_p_v3(((coax_at_u(i32(108i)) * l_a0.x) - (coax_at_u(i32(109i)) * l_a0.z)), l_a0.y, ((coax_at_u(i32(109i)) * l_a0.x) + (coax_at_u(i32(108i)) * l_a0.z)));
  l_l = coax_p_v3(((coax_at_u(i32(108i)) * l_b0.x) - (coax_at_u(i32(109i)) * l_b0.z)), l_b0.y, ((coax_at_u(i32(109i)) * l_b0.x) + (coax_at_u(i32(108i)) * l_b0.z)));
  l_hand = 4272i;
  if ((bones[(l_hand + 3i)] <= 0.0f))
  {
    return 0x1.000000p0f;
  }
  l_centre = coax_p_v3(bones[l_hand], bones[(l_hand + 1i)], bones[(l_hand + 2i)]);
  if ((coax_p_cv_segment_gap(l_q, l_l, l_centre, l_centre) > 0x1.333334p-2f))
  {
    return 0x1.000000p0f;
  }
  l_visible = 0x1.000000p0f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 20i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_at = (4264i + (l_k * 8i));
      l_r = bones[(l_at + 3i)];
      if ((l_r > 0.0f))
      {
        l_a = coax_p_v3(bones[l_at], bones[(l_at + 1i)], bones[(l_at + 2i)]);
        l_b = coax_p_v3(bones[(l_at + 4i)], bones[(l_at + 5i)], bones[(l_at + 6i)]);
        if ((coax_p_cv_segment_gap(l_q, l_q, l_a, l_b) > (l_r * 0x1.99999ap0f)))
        {
          l_visible = (l_visible * coax_p_clampf(((coax_p_cv_segment_gap(l_q, l_l, l_a, l_b) - (l_r * 0x1.99999ap-1f)) / (l_r * 0x1.000000p-1f)), 0.0f, 0x1.000000p0f));
        }
      }
    }
  }
  return l_visible;
}
fn coax_p_cv_at(p_at: i32) -> vec3f {
  let l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  let l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_cv_sub(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_cv_shadow(p_world: vec3f) -> vec3f {
  let l_world: vec3f = p_world;
  var l_p: vec3f;
  l_p = coax_p_cv_sub(l_world, coax_p_cv_at(92i));
  return coax_p_v3((coax_p_wg_dot3(l_p, coax_p_cv_at(80i)) / coax_at_u(i32(96i))), (coax_p_wg_dot3(l_p, coax_p_cv_at(84i)) / coax_at_u(i32(96i))), (0x1.000000p-1f - (coax_p_wg_dot3(l_p, coax_p_cv_at(88i)) / (0x1.000000p2f * coax_at_u(i32(96i))))));
}
fn coax_p_splash_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t17) -> coax_t18 {
  let l__vid2: u32 = p__vid2;
  let l__iid3: u32 = p__iid3;
  let l_in_: coax_t17 = p_in_;
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
fn coax_p_cv_bg_colour(p_p: vec4f) -> vec4f {
  let l_p: vec4f = p_p;
  var l_sy: f32;
  var l_ray: vec3f;
  var l_delta: vec3f;
  var l_along: f32;
  var l_distance: f32;
  var l_r: f32;
  var l_core: f32;
  var l_halo: f32;
  var l_flare: f32;
  var l_sky: vec3f;
  var l_ray_12: vec3f;
  var l_t: f32;
  var l_at: vec3f;
  var l_fx: f32;
  var l_fz: f32;
  var l_line: f32;
  var l_fade: f32;
  var l_c: vec3f;
  if ((coax_at_u(i32(112i)) > 0x1.000000p-1f))
  {
    l_sy = (0x1.000000p0f - ((l_p.y / coax_at_u(i32(1i))) * 0x1.000000p1f));
    if ((abs(l_sy) > ((coax_at_u(i32(0i)) / coax_at_u(i32(1i))) / coax_at_u(i32(113i)))))
    {
      return coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f);
    }
    l_ray = coax_p_wg_normalize3(coax_p_cv_add(coax_p_cv_at(20i), coax_p_cv_add(coax_p_cv_scale(coax_p_cv_at(12i), ((((l_p.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f) / coax_at_u(i32(15i)))), coax_p_cv_scale(coax_p_cv_at(16i), (l_sy / coax_at_u(i32(19i)))))));
    l_delta = coax_p_cv_sub(coax_p_cv_at(116i), coax_p_cv_at(8i));
    l_along = coax_p_wg_dot3(l_delta, l_ray);
    l_distance = sqrt(coax_p_fmax(0.0f, (coax_p_wg_dot3(l_delta, l_delta) - (l_along * l_along))));
    l_r = 0x1.333334p-5f;
    l_core = (0x1.000000p0f - coax_p_smoothstep((l_r * 0x1.99999ap-1f), (l_r * 0x1.0cccccp0f), l_distance));
    l_halo = (exp((((-l_distance) * l_distance) / ((l_r * l_r) * 0x1.400000p2f))) * 0x1.000000p-1f);
    l_flare = (exp((((-l_distance) * l_distance) / ((l_r * l_r) * 0x1.180000p5f))) * 0x1.eb851ep-5f);
    return coax_p_v4(coax_p_clampf(((l_halo * 0x1.99999ap-3f) + (l_flare * 0x1.99999ap-4f)), 0.0f, 0x1.000000p0f), coax_p_clampf(((l_halo * 0x1.4cccccp-1f) + (l_flare * 0x1.000000p-1f)), 0.0f, 0x1.000000p0f), coax_p_clampf((l_halo + l_flare), 0.0f, 0x1.000000p0f), 0x1.000000p0f);
  }
  if ((coax_at_u(i32(110i)) > 0x1.000000p-1f))
  {
    return coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f);
  }
  l_sky = coax_p_v3(0x1.0a3d70p-4f, 0x1.333334p-4f, 0x1.70a3d8p-4f);
  if ((coax_at_u(i32(7i)) < 0x1.000000p-1f))
  {
    return coax_p_v4(l_sky.x, l_sky.y, l_sky.z, 0x1.000000p0f);
  }
  l_ray_12 = coax_p_cv_add(coax_p_cv_at(20i), coax_p_cv_add(coax_p_cv_scale(coax_p_cv_at(12i), ((((l_p.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f) / coax_at_u(i32(15i)))), coax_p_cv_scale(coax_p_cv_at(16i), ((0x1.000000p0f - ((l_p.y / coax_at_u(i32(1i))) * 0x1.000000p1f)) / coax_at_u(i32(19i))))));
  if ((l_ray_12.y >= (-0x1.a36e2ep-14f)))
  {
    return coax_p_v4(l_sky.x, l_sky.y, l_sky.z, 0x1.000000p0f);
  }
  l_t = ((coax_at_u(i32(100i)) - coax_at_u(i32(9i))) / l_ray_12.y);
  if ((l_t <= 0.0f))
  {
    return coax_p_v4(l_sky.x, l_sky.y, l_sky.z, 0x1.000000p0f);
  }
  l_at = coax_p_cv_add(coax_p_cv_at(8i), coax_p_cv_scale(l_ray_12, l_t));
  l_fx = abs((l_at.x - floor((l_at.x + 0x1.000000p-1f))));
  l_fz = abs((l_at.z - floor((l_at.z + 0x1.000000p-1f))));
  l_line = select(0.0f, 0x1.47ae14p-4f, ((l_fx < 0x1.99999ap-6f) || (l_fz < 0x1.99999ap-6f)));
  l_fade = (0x1.000000p0f / (0x1.000000p0f + (l_t * 0x1.99999ap-6f)));
  l_c = coax_p_cv_add(coax_p_cv_scale(coax_p_v3((0x1.47ae14p-3f + l_line), (0x1.333334p-3f + l_line), (0x1.0a3d70p-3f + l_line)), l_fade), coax_p_cv_scale(l_sky, (0x1.000000p0f - l_fade)));
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_cv_scene_shade(p_s: coax_t11, p_dx: vec2f, p_dy: vec2f) -> vec4f {
  let l_s: coax_t11 = p_s;
  let l_dx: vec2f = p_dx;
  let l_dy: vec2f = p_dy;
  var l_n: vec3f;
  var l_albedo: vec3f;
  var l_rough: f32;
  var l_metal: f32;
  var l_skin_f0: f32;
  var l_occlusion: f32;
  var l_e: coax_t29;
  var l_a: vec4f;
  var l_colour: vec3f;
  var l_mapped: vec3f;
  var l_l: vec3f;
  var l_v: vec3f;
  var l_halfway: vec3f;
  var l_nl: f32;
  var l_nv: f32;
  var l_nh: f32;
  var l_vh: f32;
  var l_alpha: f32;
  var l_aa: f32;
  var l_den: f32;
  var l_distribution: f32;
  var l_k: f32;
  var l_g: f32;
  var l_dielectric: f32;
  var l_f0: vec3f;
  var l_fresnel: f32;
  var l_spec: vec3f;
  var l_shadow: f32;
  var l_ambient: f32;
  var l_direct: f32;
  var l_colour_39: vec3f;
  var l_skin: bool;
  var l_geo: vec3f;
  var l_rect: vec3f;
  var l_inner: f32;
  var l_contact: f32;
  var l_soft: f32;
  var l_bleed: f32;
  var l_key_rgb: vec3f;
  var l_env: vec2f;
  var l_environment: vec2f;
  var l_f90: f32;
  var l_mapped_51: vec3f;
  if ((l_s.f7 == 0x1.d00000p4f))
  {
    return coax_p_cv_orb_shade(l_s);
  }
  l_n = coax_p_wg_normalize3(l_s.z);
  l_albedo = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  l_rough = 0x1.4cccccp-1f;
  l_metal = 0.0f;
  l_skin_f0 = 0x1.47ae14p-5f;
  l_occlusion = 0x1.000000p0f;
  if ((l_s.f6 > 0x1.000000p-1f))
  {
    l_e = coax_p_cv_echo_material(l_s.z, l_s.f5, l_s.y, l_s.w, l_s.f7, l_dx, l_dy);
    l_n = l_e.x;
    l_albedo = l_e.y;
    l_rough = l_e.z;
    l_metal = l_e.w;
    l_skin_f0 = l_e.f4;
    l_occlusion = l_e.f5;
  }
  else
  {
    l_a = coax_p_render_sample_layer8_grad(l_s.w, 0i, l_dx, l_dy, 0.0f);
    l_albedo = coax_p_cv_scale(coax_p_v3(l_a.x, l_a.y, l_a.z), coax_at_u(i32(6i)));
    l_rough = coax_p_cv_srgb((coax_p_render_sample_layer8_grad(l_s.w, 1i, l_dx, l_dy, 0.0f)).x);
  }
  l_rough = coax_p_clampf(l_rough, 0x1.70a3d8p-5f, 0x1.000000p0f);
  if (((coax_at_u(i32(4i)) > 0x1.000000p-1f) && (coax_at_u(i32(4i)) < 0x1.800000p0f)))
  {
    return coax_p_v4(coax_p_cv_srgb(l_albedo.x), coax_p_cv_srgb(l_albedo.y), coax_p_cv_srgb(l_albedo.z), 0x1.000000p0f);
  }
  if (((coax_at_u(i32(4i)) > 0x1.800000p0f) && (coax_at_u(i32(4i)) < 0x1.400000p1f)))
  {
    return coax_p_v4(((l_n.x * 0x1.000000p-1f) + 0x1.000000p-1f), ((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f), ((l_n.z * 0x1.000000p-1f) + 0x1.000000p-1f), 0x1.000000p0f);
  }
  if (((coax_at_u(i32(4i)) > 0x1.400000p1f) && (coax_at_u(i32(4i)) < 0x1.c00000p1f)))
  {
    return coax_p_v4(l_rough, l_rough, l_rough, 0x1.000000p0f);
  }
  if ((coax_at_u(i32(4i)) > 0x1.c00000p1f))
  {
    return coax_p_v4(l_s.f4.x, l_s.f4.y, l_s.f4.z, 0x1.000000p0f);
  }
  if ((((l_s.f6 > 0x1.000000p-1f) && ((l_s.f7 == 0x1.a00000p4f) || (l_s.f7 == 0x1.b00000p4f))) && (coax_at_u(i32(114i)) >= 0.0f)))
  {
    l_colour = coax_p_cv_add(coax_p_cv_scale(coax_p_v3(l_s.f8.x, l_s.f8.y, l_s.f8.z), coax_p_cv_shadow_pcf(l_s.y)), coax_p_v3(l_s.f9.x, l_s.f9.y, l_s.f9.z));
    l_mapped = coax_p_film_rec709(coax_p_film_rgb(l_colour.x, l_colour.y, l_colour.z));
    return coax_p_v4(coax_p_cv_srgb(coax_p_clampf(l_mapped.x, 0.0f, 0x1.000000p0f)), coax_p_cv_srgb(coax_p_clampf(l_mapped.y, 0.0f, 0x1.000000p0f)), coax_p_cv_srgb(coax_p_clampf(l_mapped.z, 0.0f, 0x1.000000p0f)), coax_p_cv_groom_opacity(l_s.f4.x, l_s.f4.y));
  }
  l_l = coax_p_cv_at(24i);
  l_v = coax_p_wg_normalize3(coax_p_cv_sub(coax_p_cv_at(8i), l_s.y));
  l_halfway = coax_p_wg_normalize3(coax_p_cv_add(l_l, l_v));
  l_nl = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, l_l));
  l_nv = coax_p_fmax(0x1.a36e2ep-14f, coax_p_wg_dot3(l_n, l_v));
  l_nh = coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, l_halfway));
  l_vh = coax_p_fmax(0.0f, coax_p_wg_dot3(l_v, l_halfway));
  l_alpha = (l_rough * l_rough);
  l_aa = (l_alpha * l_alpha);
  l_den = (((l_nh * l_nh) * (l_aa - 0x1.000000p0f)) + 0x1.000000p0f);
  l_distribution = (l_aa / ((0x1.921fb6p1f * l_den) * l_den));
  l_k = (((l_rough + 0x1.000000p0f) * (l_rough + 0x1.000000p0f)) / 0x1.000000p3f);
  l_g = (((l_nv / ((l_nv * (0x1.000000p0f - l_k)) + l_k)) * l_nl) / ((l_nl * (0x1.000000p0f - l_k)) + l_k));
  l_dielectric = select(l_skin_f0, coax_p_mix(0x1.0624dep-8f, 0x1.0624dep-7f, l_s.w.y), ((l_s.f7 == 0x1.a00000p4f) || (l_s.f7 == 0x1.b00000p4f)));
  l_f0 = coax_p_v3(coax_p_mix(l_dielectric, l_albedo.x, l_metal), coax_p_mix(l_dielectric, l_albedo.y, l_metal), coax_p_mix(l_dielectric, l_albedo.z, l_metal));
  l_fresnel = coax_pow_bg_vs((0x1.000000p0f - l_vh), 0x1.400000p2f);
  l_spec = coax_p_cv_scale(coax_p_v3((l_f0.x + ((0x1.000000p0f - l_f0.x) * l_fresnel)), (l_f0.y + ((0x1.000000p0f - l_f0.y) * l_fresnel)), (l_f0.z + ((0x1.000000p0f - l_f0.z) * l_fresnel))), ((l_distribution * l_g) / ((0x1.000000p2f * l_nv) * coax_p_fmax(l_nl, 0x1.a36e2ep-14f))));
  l_shadow = coax_p_cv_shadow_pcf(l_s.y);
  l_ambient = select((0x1.47ae14p-3f + (0x1.eb851ep-4f * ((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f))), 0x1.8c0000p-1f, (l_s.f6 > 0x1.000000p-1f));
  l_direct = ((l_nl * l_shadow) * 0x1.c00000p1f);
  l_colour_39 = coax_p_cv_add(coax_p_cv_scale(l_albedo, ((l_ambient * (0x1.000000p0f - l_metal)) + ((l_direct * (0x1.000000p0f - l_metal)) / 0x1.921fb6p1f))), coax_p_cv_scale(l_spec, l_direct));
  if ((coax_at_u(i32(112i)) > 0x1.000000p-1f))
  {
    l_skin = ((l_s.f6 > 0x1.000000p-1f) && ((l_s.f7 == 0x1.600000p3f) || (l_s.f7 == 0x1.800000p2f)));
    l_geo = coax_p_wg_normalize3(l_s.z);
    l_rect = coax_p_cv_rect_delta(l_s.y);
    l_inner = select(0x1.000000p0f, 0x1.eb851ep-5f, ((l_s.f6 > 0x1.000000p-1f) && (l_s.f7 == 0x1.400000p3f)));
    l_contact = select(0x1.000000p0f, coax_p_fmin(0x1.000000p0f, (l_occlusion * 0x1.19999ap0f)), l_skin);
    l_soft = (l_inner * l_contact);
    l_colour_39 = coax_p_cv_add(coax_p_cv_scale(l_albedo, ((0x1.47ae14p-4f * l_occlusion) * l_inner)), coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_slay_light(l_skin, l_albedo, l_f0, l_metal, l_rough, l_n, l_geo, l_l, l_v, 0x1.000000p0f, 0x1.40f7fcp-4f), ((l_shadow * coax_at_u(i32(123i))) * l_inner)), coax_p_cv_at(120i)));
    l_colour_39 = coax_p_cv_scale(l_colour_39, l_contact);
    if ((l_skin || (l_inner < 0x1.000000p0f)))
    {
      l_bleed = (((((coax_p_fmax(0.0f, ((coax_p_wg_dot3(l_geo, l_l) + 0x1.000000p-1f) / 0x1.800000p0f)) * (0x1.000000p0f - l_shadow)) * coax_at_u(i32(123i))) * l_contact) * select(0x1.000000p0f, 0x1.000000p-1f, (l_inner < 0x1.000000p0f))) / 0x1.921fb6p1f);
      l_key_rgb = coax_p_cv_at(120i);
      l_colour_39 = coax_p_cv_add(l_colour_39, coax_p_v3((((l_albedo.x * l_key_rgb.x) * l_bleed) * 0x1.666666p-2f), (((l_albedo.y * l_key_rgb.y) * l_bleed) * 0x1.1eb852p-4f), 0.0f));
    }
    l_colour_39 = coax_p_cv_add(l_colour_39, coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_slay_light(l_skin, l_albedo, l_f0, l_metal, l_rough, l_n, l_geo, coax_p_cv_at(97i), l_v, 0x1.000000p0f, 0x1.ac1d6cp-2f), ((coax_at_u(i32(115i)) * 0x1.941ed0p-1f) * l_soft)), coax_p_v3(0x1.000000p0f, 0x1.9ba010p-2f, 0x1.9cc986p-3f)));
    l_colour_39 = coax_p_cv_add(l_colour_39, coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_slay_light(l_skin, l_albedo, l_f0, l_metal, l_rough, l_n, l_geo, coax_p_cv_at(28i), l_v, 0x1.000000p-1f, 0x1.ac1d6cp-2f), ((coax_at_u(i32(31i)) * 0x1.0ace90p-2f) * l_soft)), coax_p_cv_at(124i)));
    l_colour_39 = coax_p_cv_add(l_colour_39, coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_slay_light(l_skin, l_albedo, l_f0, l_metal, l_rough, l_n, l_geo, coax_p_cv_kick_dir(), l_v, 0x1.000000p-1f, 0x1.ac1d6cp-2f), (0x1.941ed0p0f * l_soft)), coax_p_v3(0x1.000000p0f, 0x1.eded28p-1f, 0x1.e978d4p-1f)));
    l_colour_39 = coax_p_cv_add(l_colour_39, coax_p_cv_mul(coax_p_cv_scale(coax_p_cv_slay_light(l_skin, l_albedo, l_f0, l_metal, l_rough, l_n, l_geo, coax_p_wg_normalize3(l_rect), l_v, 0x1.000000p0f, 0.0f), ((coax_p_cv_rect_power(l_rect) * l_s.f8.x) * l_soft)), coax_p_v3(coax_at_u(i32(23i)), coax_at_u(i32(27i)), coax_at_u(i32(95i)))));
    if (l_skin)
    {
      l_env = coax_p_pbr_environment(l_rough, l_nv);
      l_colour_39 = coax_p_cv_add(l_colour_39, coax_p_cv_scale(coax_p_v3(0x1.000000p0f, 0x1.e147aep-1f, 0x1.800000p-1f), (((((l_f0.x * l_env.x) + l_env.y) * 0x1.666666p-1f) * coax_p_cv_specular_occlusion(l_nv, l_occlusion, l_rough)) * coax_p_smoothstep(0.0f, 0x1.666666p-2f, l_nv))));
    }
  }
  if ((((l_s.f6 > 0x1.000000p-1f) && (l_s.f7 < 0x1.a00000p4f)) && (coax_at_u(i32(112i)) < 0x1.000000p-1f)))
  {
    l_environment = coax_p_pbr_environment(l_rough, l_nv);
    l_f90 = coax_p_clampf((0x1.900000p5f * l_f0.y), 0.0f, 0x1.000000p0f);
    l_colour_39 = coax_p_cv_add(l_colour_39, coax_p_cv_scale(coax_p_cv_add(coax_p_cv_scale(l_f0, l_environment.x), coax_p_v3((l_f90 * l_environment.y), (l_f90 * l_environment.y), (l_f90 * l_environment.y))), 0x1.8c0000p-1f));
  }
  if ((l_s.f6 > 0x1.000000p-1f))
  {
    l_mapped_51 = coax_p_film_rec709(coax_p_film_rgb(l_colour_39.x, l_colour_39.y, l_colour_39.z));
    return coax_p_v4(coax_p_cv_srgb(coax_p_clampf(l_mapped_51.x, 0.0f, 0x1.000000p0f)), coax_p_cv_srgb(coax_p_clampf(l_mapped_51.y, 0.0f, 0x1.000000p0f)), coax_p_cv_srgb(coax_p_clampf(l_mapped_51.z, 0.0f, 0x1.000000p0f)), select(0x1.000000p0f, coax_p_cv_groom_opacity(l_s.f4.x, l_s.f4.y), ((l_s.f7 == 0x1.a00000p4f) || (l_s.f7 == 0x1.b00000p4f))));
  }
  return coax_p_v4(coax_p_cv_tone(l_colour_39.x), coax_p_cv_tone(l_colour_39.y), coax_p_cv_tone(l_colour_39.z), 0x1.000000p0f);
}
fn coax_p_cv_view_z(p_d: f32) -> f32 {
  let l_d: f32 = p_d;
  return (0x1.99999ap-5f / coax_p_fmax((0x1.000000p0f - ((l_d * 0x1.2bf334p8f) / 0x1.2c0000p8f)), 0x1.0c6f7ap-20f));
}
fn coax_p_cv_groom_opacity(p_coverage: f32, p_edge: f32) -> f32 {
  let l_coverage: f32 = p_coverage;
  let l_edge: f32 = p_edge;
  return coax_p_clampf((l_coverage * (0x1.000000p0f - (abs(l_edge) * 0x1.000000p-2f))), 0.0f, 0x1.000000p0f);
}
fn coax_p_cv_echo_mask(p_uv: vec2f, p_material: f32, p_dx: vec2f, p_dy: vec2f) {
  let l_uv: vec2f = p_uv;
  let l_material: f32 = p_material;
  let l_dx: vec2f = p_dx;
  let l_dy: vec2f = p_dy;
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
fn coax_p_wg_i32_f(p_x: f32) -> i32 {
  let l_x: f32 = p_x;
  return select(i32(coax_p_fmax(coax_p_fmin(l_x, 0x1.fffffep30f), (-0x1.000000p31f))), 0i, (l_x != l_x));
}
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t25 {
  let l_x: i32 = p_x;
  let l_y: i32 = p_y;
  return coax_t25(l_x, l_y);
}
fn coax_p_wg_dims_2d() -> coax_t26 {
  return coax_p_u2(textureDimensions(post_light).x, textureDimensions(post_light).y);
}
fn coax_p_wg_load_2d(p_c: coax_t25, p_level: i32) -> vec4f {
  let l_c: coax_t25 = p_c;
  let l_level: i32 = p_level;
  return textureLoad(post_light, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_cv_post_z(p_q: coax_t25) -> f32 {
  let l_q: coax_t25 = p_q;
  var l_zd: f32;
  var l_a: vec4f;
  l_zd = coax_p_cv_view_z(coax_p_wg_load_depth(l_q, 0i));
  l_a = coax_p_wg_load_2d_1(l_q, 0i);
  if ((l_a.z > 0x1.47ae14p-6f))
  {
    return coax_p_fmin(l_zd, (l_a.y / l_a.z));
  }
  return l_zd;
}
fn coax_p_cv_coc(p_z: f32, p_width: f32) -> f32 {
  let l_z: f32 = p_z;
  let l_width: f32 = p_width;
  var l_f: f32;
  var l_zf: f32;
  var l_c: f32;
  l_f = coax_at_u(i32(130i));
  l_zf = coax_at_u(i32(128i));
  if ((((coax_at_u(i32(129i)) <= 0.0f) || (l_f <= 0.0f)) || (l_zf <= l_f)))
  {
    return 0.0f;
  }
  l_c = ((((l_f * l_f) / coax_at_u(i32(129i))) * abs((l_z - l_zf))) / (l_z * (l_zf - l_f)));
  return coax_p_fmin((((0x1.000000p-1f * l_c) / 0x1.97cc3ap-6f) * l_width), ((0x1.c00000p3f * l_width) / 0x1.400000p10f));
}
fn coax_p_cv_light_at(p_p: coax_t25, p_size: coax_t26) -> vec3f {
  let l_p: coax_t25 = p_p;
  let l_size: coax_t26 = p_size;
  var l_q: coax_t25;
  var l_c: vec4f;
  l_q = coax_p_i2(coax_p_clamp_s(l_p.x, 0i, (i32(l_size.x) - 1i)), coax_p_clamp_s(l_p.y, 0i, (i32(l_size.y) - 1i)));
  l_c = coax_p_wg_load_2d(l_q, 0i);
  return coax_p_v3(coax_p_cv_srgb_linear(l_c.x), coax_p_cv_srgb_linear(l_c.y), coax_p_cv_srgb_linear(l_c.z));
}
fn coax_p_wg_load_2d_1(p_c: coax_t25, p_level: i32) -> vec4f {
  let l_c: coax_t25 = p_c;
  let l_level: i32 = p_level;
  return textureLoad(post_aux, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b < l_a));
}
fn coax_p_clamp_s(p_x: i32, p_a: i32, p_b: i32) -> i32 {
  let l_x: i32 = p_x;
  let l_a: i32 = p_a;
  let l_b: i32 = p_b;
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
fn coax_p_wg_load_depth(p_c: coax_t25, p_level: i32) -> f32 {
  let l_c: coax_t25 = p_c;
  let l_level: i32 = p_level;
  return textureLoad(post_depth, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_cv_scale(p_a: vec3f, p_k: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_k: f32 = p_k;
  return coax_p_v3((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b > l_a));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_low: f32 = p_low;
  let l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_cv_srgb(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return select(((0x1.0e147ap0f * coax_pow_bg_vs(l_x, (0x1.000000p0f / 0x1.333334p1f))) - 0x1.c28f5cp-5f), (0x1.9d70a4p3f * l_x), (l_x <= 0x1.9a5c38p-9f));
}
fn coax_p_splash_fx_fs(p_v: coax_t18) -> vec4f {
  let l_v: coax_t18 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_mode: f32;
  var l_p: vec2f;
  var l_c: f32;
  var l_r: f32;
  var l_tail: f32;
  var l_g: f32;
  var l_hot: f32;
  var l_c_11: vec3f;
  var l_core: f32;
  var l_g_13: f32;
  var l_halo: f32;
  var l_ends: f32;
  var l_c_16: vec3f;
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_mode = l_v.w.w;
  if ((l_mode > 0x1.c00000p1f))
  {
    l_p = coax_p_wg_div_f2f(l_v.y, (0x1.000000p0f - (0x1.eb851ep-6f * l_v.f4.x)));
    l_c = coax_p_metaverse_bar((-l_p.x), l_p.y, l_v.z.x, l_v.z.y, l_v.z.z, coax_at_u(i32(2i)), l_v.f4.x, l_v.f4.y);
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_c), l_v.z.w), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  if (((l_mode > 0x1.800000p0f) && (l_mode < 0x1.400000p1f)))
  {
    l_r = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
    if ((l_r >= 0x1.000000p0f))
    {
      return coax_p_v4(0.0f, 0.0f, 0.0f, 0x1.000000p0f);
    }
    l_tail = select(0x1.000000p0f, coax_p_mix(0x1.47ae14p-4f, 0x1.000000p0f, coax_p_clampf((l_v.y.x / l_v.z.x), 0.0f, 0x1.000000p0f)), (l_v.z.x > 0x1.000000p0f));
    l_g = (exp((((-l_r) * l_r) * 0x1.400000p2f)) * (0x1.000000p0f - l_r));
    l_hot = (exp((((-l_r) * l_r) * 0x1.200000p5f)) * l_v.z.y);
    l_c_11 = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_g), coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.b851ecp-1f), l_hot)), l_tail), l_v.z.z), coax_at_u(i32(3i)));
    return coax_p_v4(l_c_11.x, l_c_11.y, l_c_11.z, 0x1.000000p0f);
  }
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g_13 = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g_13) * l_g_13)) * 0x1.1eb852p-2f);
  if (((l_mode > 0x1.000000p-1f) && (l_mode < 0x1.800000p0f)))
  {
    l_ends = (coax_p_smoothstep(0.0f, 0x1.000000p3f, l_v.y.x) * coax_p_smoothstep(0.0f, 0x1.000000p3f, (l_v.z.x - l_v.y.x)));
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), ((l_core * 0x1.99999ap-1f) + l_halo)), l_ends), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  if (((l_mode > 0x1.400000p1f) && (l_mode < 0x1.c00000p1f)))
  {
    l_c_16 = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.d70a3ep-1f), 0x1.800000p-1f), l_core), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_halo), 0x1.ccccccp0f)), l_v.z.z), coax_at_u(i32(3i)));
    return coax_p_v4(l_c_16.x, l_c_16.y, l_c_16.z, 0x1.000000p0f);
  }
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  let l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  let l_a: vec4f = p_a;
  let l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_cv_groom_share(p_shadow: bool) -> f32 {
  let l_shadow: bool = p_shadow;
  var l_f: f32;
  l_f = abs(coax_at_u(i32(2i)));
  l_f = select(0x1.000000p0f, l_f, (l_f > 0.0f));
  return select(l_f, coax_p_fmin(l_f, 0x1.000000p-2f), l_shadow);
}
fn coax_p_cv_groom_hash(p_seed: f32) -> f32 {
  let l_seed: f32 = p_seed;
  var l_h: f32;
  l_h = (sin(((l_seed * 0x1.f60000p7f) * 0x1.9fac72p3f)) * 0x1.55dd18p15f);
  return (l_h - floor(l_h));
}
fn coax_p_cv_groom_endpoint(p_vertex: i32) -> i32 {
  let l_vertex: i32 = p_vertex;
  return select(0i, 1i, (((l_vertex == 2i) || (l_vertex == 3i)) || (l_vertex == 5i)));
}
fn coax_p_cv_head(p_p: vec3f) -> vec3f {
  let l_p: vec3f = p_p;
  var l_b: i32;
  var l_q: vec4f;
  var l_local: vec3f;
  l_b = 984i;
  l_q = coax_p_v4(l_p.x, l_p.y, l_p.z, 0x1.000000p0f);
  l_local = coax_p_v3(coax_p_wg_dot4(coax_p_wg_at4_1(l_b), l_q), coax_p_wg_dot4(coax_p_wg_at4_1((l_b + 4i)), l_q), coax_p_wg_dot4(coax_p_wg_at4_1((l_b + 8i)), l_q));
  return coax_p_cv_add(coax_p_cv_at(104i), coax_p_cv_echo_direction(l_local));
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_cv_groom_side(p_vertex: i32) -> f32 {
  let l_vertex: i32 = p_vertex;
  return select(0x1.000000p0f, (-0x1.000000p0f), (((l_vertex == 0i) || (l_vertex == 2i)) || (l_vertex == 3i)));
}
fn coax_p_cv_blonde_groom_colour(p_u: f32, p_seed: f32) -> vec3f {
  let l_u: f32 = p_u;
  let l_seed: f32 = p_seed;
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
  let l_u: f32 = p_u;
  let l_seed: f32 = p_seed;
  var l_brightness: f32;
  if ((l_seed < 0x1.333334p-3f))
  {
    return coax_p_v3(0x1.9cc964p-3f, 0x1.00baa6p-3f, 0x1.20f3ccp-4f);
  }
  l_brightness = select(select(coax_p_mix(0x1.000000p-1f, 0.0f, ((l_seed * 0x1.800000p1f) - 0x1.000000p1f)), coax_p_mix(0x1.333334p-3f, 0x1.000000p-1f, ((l_seed * 0x1.800000p1f) - 0x1.000000p0f)), (l_seed < (0x1.000000p1f / 0x1.800000p1f))), coax_p_mix(0x1.99999ap-2f, 0x1.333334p-3f, (l_seed * 0x1.800000p1f)), (l_seed < (0x1.000000p0f / 0x1.800000p1f)));
  return coax_p_cv_scale(coax_p_v3(coax_p_mix(0x1.000000p-2f, 0x1.e00000p-3f, l_u), coax_p_mix(0x1.3d334cp-3f, 0x1.50bbf6p-3f, l_u), coax_p_mix(0x1.666666p-4f, 0x1.b00044p-4f, l_u)), l_brightness);
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_cv_hair_averages(p_base: vec3f, p_sinv: f32) -> coax_t28 {
  let l_base: vec3f = p_base;
  let l_sinv: f32 = p_sinv;
  var l_x: vec3f;
  l_x = coax_p_v3(sqrt(coax_p_fmax(l_base.x, 0.0f)), sqrt(coax_p_fmax(l_base.y, 0.0f)), sqrt(coax_p_fmax(l_base.z, 0.0f)));
  return coax_t28(coax_p_v3(coax_p_cv_hair_average(l_x.x, l_sinv, false), coax_p_cv_hair_average(l_x.y, l_sinv, false), coax_p_cv_hair_average(l_x.z, l_sinv, false)), coax_p_v3(coax_p_cv_hair_average(l_x.x, l_sinv, true), coax_p_cv_hair_average(l_x.y, l_sinv, true), coax_p_cv_hair_average(l_x.z, l_sinv, true)));
}
fn coax_p_cv_kick_dir() -> vec3f {
  var l_c: f32;
  var l_s: f32;
  l_c = cos(coax_at_u(i32(119i)));
  l_s = sin(coax_at_u(i32(119i)));
  return coax_p_v3(((l_c * (-0x1.285722p-1f)) + (l_s * (-0x1.031462p-1f))), 0x1.476ba2p-1f, (((-l_s) * (-0x1.285722p-1f)) + (l_c * (-0x1.031462p-1f))));
}
fn coax_p_cv_fiber_light(p_base: vec3f, p_t: vec3f, p_l: vec3f, p_v: vec3f, p_rough: f32, p_seed: f32, p_avg: coax_t28) -> vec3f {
  let l_base: vec3f = p_base;
  let l_t: vec3f = p_t;
  let l_l: vec3f = p_l;
  let l_v: vec3f = p_v;
  let l_rough: f32 = p_rough;
  let l_seed: f32 = p_seed;
  let l_avg: coax_t28 = p_avg;
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
fn coax_p_cv_mul(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_morph_gpu(p_position: vec3f, p_normal: vec3f, p_base: i32, p_first: i32, p_count: i32, p_weight_at: i32, p_targets: i32) -> coax_t14 {
  let l_position: vec3f = p_position;
  let l_normal: vec3f = p_normal;
  let l_base: i32 = p_base;
  let l_first: i32 = p_first;
  let l_count: i32 = p_count;
  let l_weight_at: i32 = p_weight_at;
  let l_targets: i32 = p_targets;
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
fn coax_p_wg_at4_1(p_at: i32) -> vec4f {
  let l_at: i32 = p_at;
  return coax_p_v4(bones[l_at], bones[(l_at + 1i)], bones[(l_at + 2i)], bones[(l_at + 3i)]);
}
fn coax_p_cv_oct_decode(p_x: f32, p_y: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  var l_n: vec3f;
  var l_t: f32;
  l_n = coax_p_v3(l_x, l_y, ((0x1.000000p0f - abs(l_x)) - abs(l_y)));
  l_t = coax_p_fmax((-l_n.z), 0.0f);
  return coax_p_wg_normalize3(coax_p_v3((l_n.x + select(l_t, (-l_t), (l_n.x >= 0.0f))), (l_n.y + select(l_t, (-l_t), (l_n.y >= 0.0f))), l_n.z));
}
fn coax_p_slay_candela_scale(p_d2: f32, p_radius: f32) -> f32 {
  let l_d2: f32 = p_d2;
  let l_radius: f32 = p_radius;
  return ((0x1.f9a6ecp3f * coax_p_slay_window(l_d2, l_radius)) / (l_d2 + 0x1.a36e2ep-14f));
}
fn coax_p_slay_rect_emission(p_cos_out: f32) -> f32 {
  let l_cos_out: f32 = p_cos_out;
  var l_edge: f32;
  if ((l_cos_out <= 0x1.2ef67cp-1f))
  {
    return 0.0f;
  }
  l_edge = ((l_cos_out - 0x1.2ef67cp-1f) / 0x1.47ae14p-5f);
  l_edge = select(l_edge, 0x1.000000p0f, (l_edge > 0x1.000000p0f));
  return (((l_cos_out * l_edge) * l_edge) * (0x1.800000p1f - (0x1.000000p1f * l_edge)));
}
fn coax_p_cv_segment_gap(p_p0: vec3f, p_p1: vec3f, p_q0: vec3f, p_q1: vec3f) -> f32 {
  let l_p0: vec3f = p_p0;
  let l_p1: vec3f = p_p1;
  let l_q0: vec3f = p_q0;
  let l_q1: vec3f = p_q1;
  var l_d1: vec3f;
  var l_d2: vec3f;
  var l_r: vec3f;
  var l_a: f32;
  var l_e: f32;
  var l_f: f32;
  var l_c: f32;
  var l_b: f32;
  var l_denom: f32;
  var l_sa: f32;
  var l_t: f32;
  l_d1 = coax_p_cv_sub(l_p1, l_p0);
  l_d2 = coax_p_cv_sub(l_q1, l_q0);
  l_r = coax_p_cv_sub(l_p0, l_q0);
  l_a = coax_p_fmax(coax_p_wg_dot3(l_d1, l_d1), 0x1.5798eep-27f);
  l_e = coax_p_fmax(coax_p_wg_dot3(l_d2, l_d2), 0x1.5798eep-27f);
  l_f = coax_p_wg_dot3(l_d2, l_r);
  l_c = coax_p_wg_dot3(l_d1, l_r);
  l_b = coax_p_wg_dot3(l_d1, l_d2);
  l_denom = ((l_a * l_e) - (l_b * l_b));
  l_sa = select(0.0f, coax_p_clampf((((l_b * l_f) - (l_c * l_e)) / l_denom), 0.0f, 0x1.000000p0f), (l_denom > 0x1.b7cdfep-34f));
  l_t = (((l_b * l_sa) + l_f) / l_e);
  if ((l_t < 0.0f))
  {
    l_t = 0.0f;
    l_sa = coax_p_clampf(((-l_c) / l_a), 0.0f, 0x1.000000p0f);
  }
  else
  {
    if ((l_t > 0x1.000000p0f))
    {
      l_t = 0x1.000000p0f;
      l_sa = coax_p_clampf(((l_b - l_c) / l_a), 0.0f, 0x1.000000p0f);
    }
  }
  return coax_p_wg_length3(coax_p_cv_sub(coax_p_cv_add(l_p0, coax_p_cv_scale(l_d1, l_sa)), coax_p_cv_add(l_q0, coax_p_cv_scale(l_d2, l_t))));
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  let l_x: u32 = p_x;
  let l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_cv_orb_shade(p_s: coax_t11) -> vec4f {
  let l_s: coax_t11 = p_s;
  var l_dist: i32;
  var l_energy: i32;
  var l_n: vec3f;
  var l_t: vec3f;
  var l_b: vec3f;
  var l_v: vec3f;
  var l_nv: f32;
  var l_bump: vec3f;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_normal: vec3f;
  var l_e: vec4f;
  var l_mapped: vec3f;
  l_dist = i32(vertices[6052902i]);
  l_energy = i32(vertices[6052903i]);
  l_n = coax_p_wg_normalize3(l_s.z);
  l_t = coax_p_wg_normalize3(coax_p_v3(l_s.f5.x, l_s.f5.y, l_s.f5.z));
  l_b = coax_p_cv_scale(coax_p_wg_cross3(l_n, l_t), l_s.f5.w);
  l_v = coax_p_wg_normalize3(coax_p_cv_sub(coax_p_cv_at(8i), l_s.y));
  l_nv = coax_p_wg_dot3(l_n, l_v);
  if ((l_nv <= 0.0f))
  {
    discard;
  }
  l_bump = coax_p_raw_rgb_sample(l_dist, 256i, coax_p_v2((coax_at_u(i32(5i)) * 0x1.99999ap-4f), (coax_at_u(i32(5i)) * 0x1.99999ap-4f)));
  l_x = ((l_bump.x * 0x1.000000p1f) - 0x1.000000p0f);
  l_y = (0x1.000000p0f - (l_bump.y * 0x1.000000p1f));
  l_z = sqrt(coax_p_fmax(0.0f, ((0x1.000000p0f - (l_x * l_x)) - (l_y * l_y))));
  l_normal = coax_p_cv_add(coax_p_cv_scale(l_t, (l_x * 0x1.000000p-1f)), coax_p_cv_add(coax_p_cv_scale(l_b, (l_y * 0x1.000000p-1f)), coax_p_cv_scale(l_n, ((l_z * 0x1.000000p-1f) + 0x1.000000p-1f))));
  l_e = coax_p_slay_orb_emission(coax_p_wg_dot3(l_normal, l_v), l_nv, coax_p_slay_orb_energy(l_dist, l_energy, l_s.w, coax_at_u(i32(5i))));
  l_mapped = coax_p_film_rec709(coax_p_film_rgb(l_e.x, l_e.y, l_e.z));
  return coax_p_v4(coax_p_cv_srgb(coax_p_clampf(l_mapped.x, 0.0f, 0x1.000000p0f)), coax_p_cv_srgb(coax_p_clampf(l_mapped.y, 0.0f, 0x1.000000p0f)), coax_p_cv_srgb(coax_p_clampf(l_mapped.z, 0.0f, 0x1.000000p0f)), l_e.w);
}
fn coax_p_cv_echo_material(p_normal: vec3f, p_tangent: vec4f, p_world: vec3f, p_uv: vec2f, p_material: f32, p_dx: vec2f, p_dy: vec2f) -> coax_t29 {
  let l_normal: vec3f = p_normal;
  let l_tangent: vec4f = p_tangent;
  let l_world: vec3f = p_world;
  let l_uv: vec2f = p_uv;
  let l_material: f32 = p_material;
  let l_dx: vec2f = p_dx;
  let l_dy: vec2f = p_dy;
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
    l_limbus = (0x1.000000p0f - coax_pow_bg_vs(coax_p_fmin(0x1.000000p0f, (sqrt((((l_iris_uv.x - 0x1.000000p-1f) * (l_iris_uv.x - 0x1.000000p-1f)) + ((l_iris_uv.y - 0x1.000000p-1f) * (l_iris_uv.y - 0x1.000000p-1f)))) * 0x1.133334p1f)), 0x1.000000p3f));
    l_ic = coax_p_v3(coax_pow_bg_vs((((l_iris.x * 0x1.723444p-1f) * 0x1.666666p0f) * 0x1.000000p1f), 0x1.19999ap0f), coax_pow_bg_vs((((l_iris.y * 0x1.49f948p-1f) * 0x1.666666p0f) * 0x1.000000p1f), 0x1.19999ap0f), coax_pow_bg_vs((((l_iris.z * 0x1.3f77b0p-1f) * 0x1.666666p0f) * 0x1.000000p1f), 0x1.19999ap0f));
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
      l_fresnel = coax_pow_bg_vs((0x1.000000p0f - coax_p_fmax(0.0f, coax_p_wg_dot3(l_n, l_view))), 0x1.800000p1f);
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
      l_fuzzy = coax_p_mix(0x1.000000p0f, ((0x1.000000p0f - (0x1.333334p-3f * l_nv)) + coax_pow_bg_vs((0x1.000000p0f - l_nv), 0x1.ccccccp0f)), coax_p_smoothstep(0x1.f0a3d8p-1f, 0x1.f5c290p-1f, l_nv));
    }
    l_mult = select(0x1.000000p0f, 0x1.333334p-1f, ((l_material == 0x1.000000p0f) || (l_material == 0x1.e00000p3f)));
    l_gamma = select(select(0x1.000000p0f, 0x1.0cccccp0f, ((l_material == 0x1.400000p2f) || (l_material == 0x1.a00000p3f))), 0x1.19999ap0f, (l_material == 0x1.000000p3f));
    l_albedo = coax_p_v3(coax_pow_bg_vs(coax_p_fmax(0.0f, ((l_albedo.x * l_fuzzy) * l_mult)), (0x1.000000p0f / l_gamma)), coax_pow_bg_vs(coax_p_fmax(0.0f, ((l_albedo.y * l_fuzzy) * l_mult)), (0x1.000000p0f / l_gamma)), coax_pow_bg_vs(coax_p_fmax(0.0f, ((l_albedo.z * l_fuzzy) * l_mult)), (0x1.000000p0f / l_gamma)));
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
  return coax_t29(l_n, l_albedo, l_rough, l_metal, l_skin_f0, l_occlusion);
}
fn coax_p_render_sample_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> vec4f {
  let l_uv: vec2f = p_uv;
  let l_layer: i32 = p_layer;
  let l_dx: vec2f = p_dx;
  let l_dy: vec2f = p_dy;
  let l_bias: f32 = p_bias;
  var l_rs_result: coax_t32;
  l_rs_result = coax_p_render_filter_layer8_grad(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
}
fn coax_p_cv_shadow_pcf(p_world: vec3f) -> f32 {
  let l_world: vec3f = p_world;
  var l_q: vec3f;
  var l_shadow: f32;
  var l_y: i32;
  var l_x: i32;
  var l_offset: f32;
  l_q = coax_p_cv_shadow(l_world);
  l_shadow = 0x1.000000p0f;
  if (((((abs(l_q.x) < 0x1.000000p0f) && (abs(l_q.y) < 0x1.000000p0f)) && (l_q.z > 0.0f)) && (l_q.z < 0x1.000000p0f)))
  {
    l_shadow = 0.0f;
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
            l_offset = select(0.0f, 0x1.0624dep-8f, (coax_at_u(i32(112i)) > 0x1.000000p-1f));
            l_shadow = (l_shadow + (textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((((l_q.x * 0x1.000000p-1f) + 0x1.000000p-1f) + (f32((l_x - 1i)) * l_offset)), ((0x1.000000p-1f - (l_q.y * 0x1.000000p-1f)) + (f32((l_y - 1i)) * l_offset))), (l_q.z - 0x1.3a92a4p-13f)) / 0x1.200000p3f));
          }
        }
      }
    }
  }
  return l_shadow;
}
fn coax_p_film_rgb(p_r: f32, p_g: f32, p_b: f32) -> vec3f {
  let l_r: f32 = p_r;
  let l_g: f32 = p_g;
  let l_b: f32 = p_b;
  return vec3f(l_r, l_g, l_b);
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
fn coax_p_cv_slay_light(p_skin: bool, p_base: vec3f, p_f0: vec3f, p_metal: f32, p_rough: f32, p_n: vec3f, p_g: vec3f, p_l: vec3f, p_v: vec3f, p_spec_scale: f32, p_size: f32) -> vec3f {
  let l_skin: bool = p_skin;
  let l_base: vec3f = p_base;
  let l_f0: vec3f = p_f0;
  let l_metal: f32 = p_metal;
  let l_rough: f32 = p_rough;
  let l_n: vec3f = p_n;
  let l_g: vec3f = p_g;
  let l_l: vec3f = p_l;
  let l_v: vec3f = p_v;
  let l_spec_scale: f32 = p_spec_scale;
  let l_size: f32 = p_size;
  if (l_skin)
  {
    return coax_p_cv_skin_light(l_base, l_f0, l_rough, l_n, l_g, l_l, l_v, l_spec_scale, l_size);
  }
  return coax_p_cv_area_light(l_base, l_f0, l_metal, l_rough, l_n, l_l, l_v, l_spec_scale, l_size);
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
fn coax_p_cv_specular_occlusion(p_nv: f32, p_ao: f32, p_rough: f32) -> f32 {
  let l_nv: f32 = p_nv;
  let l_ao: f32 = p_ao;
  let l_rough: f32 = p_rough;
  return coax_p_clampf(((coax_pow_bg_vs((coax_p_fmax(l_nv, 0.0f) + l_ao), coax_pow_bg_vs(0x1.000000p1f, (((-0x1.000000p4f) * l_rough) - 0x1.000000p0f))) - 0x1.000000p0f) + l_ao), 0.0f, 0x1.000000p0f);
}
fn coax_p_cv_tone(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_cv_srgb(coax_p_fmin(0x1.000000p0f, ((l_x * ((0x1.4147aep1f * l_x) + 0x1.eb851ep-6f)) / ((l_x * ((0x1.370a3ep1f * l_x) + 0x1.2e147ap-1f)) + 0x1.1eb852p-3f))));
}
fn coax_p_cv_echo_lash_mask(p_uv: vec2f) -> f32 {
  let l_uv: vec2f = p_uv;
  return (coax_pow_bg_vs(coax_p_fmax(0.0f, (0x1.000000p0f - abs((l_uv.x - 0x1.000000p-1f)))), 0x1.800000p0f) * coax_pow_bg_vs(coax_p_fmax(0.0f, l_uv.y), 0x1.ccccccp-2f));
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t26 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  return coax_t26(l_x, l_y);
}
fn coax_p_cv_srgb_linear(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return select(coax_pow_bg_vs(((l_x + 0x1.c28f5cp-5f) / 0x1.0e147ap0f), 0x1.333334p1f), (l_x / 0x1.9d70a4p3f), (l_x <= 0x1.4b5dccp-5f));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  let l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_metaverse_bar(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32, p_t: f32, p_press: f32, p_flash: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_w: f32 = p_w;
  let l_h: f32 = p_h;
  let l_k: f32 = p_k;
  let l_t: f32 = p_t;
  let l_press: f32 = p_press;
  let l_flash: f32 = p_flash;
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
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  let l_p0: vec3f = p_p0;
  let l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  let l_p0: vec3f = p_p0;
  let l_p1: vec3f = p_p1;
  let l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
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
fn coax_p_cv_hair_average(p_xin: f32, p_sinl: f32, p_back: bool) -> f32 {
  let l_xin: f32 = p_xin;
  let l_sinl: f32 = p_sinl;
  let l_back: bool = p_back;
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
  let l_base: f32 = p_base;
  let l_rough: f32 = p_rough;
  let l_spec: f32 = p_spec;
  let l_back: f32 = p_back;
  let l_sinL: f32 = p_sinL;
  let l_sinV: f32 = p_sinV;
  let l_phi: f32 = p_phi;
  let l_voL: f32 = p_voL;
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
  l_transmit = (((((coax_p_hair_gaussian((l_width * 0x1.000000p-1f), ((l_sinL + l_sinV) - 0x1.1eb852p-5f)) * exp((((-0x1.d33334p1f) * l_phi) - 0x1.fd70a4p1f))) * (0x1.000000p0f - l_f)) * (0x1.000000p0f - l_f)) * coax_pow_bg_vs(abs(l_base), ((0x1.000000p-1f * sqrt(coax_p_hf_max(0.0f, (0x1.000000p0f - (((l_h * l_h) * l_eta) * l_eta))))) / l_d))) * l_back);
  l_f = coax_p_hair_fresnel((l_d * 0x1.000000p-1f));
  l_internal = (((((coax_p_hair_gaussian((l_width * 0x1.000000p1f), ((l_sinL + l_sinV) - 0x1.1eb852p-3f)) * exp(((0x1.100000p4f * l_phi) - 0x1.0c7ae2p4f))) * (0x1.000000p0f - l_f)) * (0x1.000000p0f - l_f)) * l_f) * coax_pow_bg_vs(abs(l_base), (0x1.99999ap-1f / l_d)));
  return ((l_reflect + l_transmit) + l_internal);
}
fn coax_p_cv_hair_dual(p_avg: coax_t28, p_rough: f32, p_sl: f32, p_sv: f32, p_count: f32, p_fs: vec3f) -> vec3f {
  let l_avg: coax_t28 = p_avg;
  let l_rough: f32 = p_rough;
  let l_sl: f32 = p_sl;
  let l_sv: f32 = p_sv;
  let l_count: f32 = p_count;
  let l_fs: vec3f = p_fs;
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
fn coax_p_morph_axis(p_base: f32, p_delta: f32, p_weight: f32) -> f32 {
  let l_base: f32 = p_base;
  let l_delta: f32 = p_delta;
  let l_weight: f32 = p_weight;
  if ((l_weight == 0.0f))
  {
    return l_base;
  }
  return (l_base + (l_delta * l_weight));
}
fn coax_p_slay_window(p_d2: f32, p_radius: f32) -> f32 {
  let l_d2: f32 = p_d2;
  let l_radius: f32 = p_radius;
  var l_x: f32;
  var l_w: f32;
  if ((l_radius <= 0.0f))
  {
    return 0.0f;
  }
  l_x = (l_d2 / (l_radius * l_radius));
  l_w = (0x1.000000p0f - (l_x * l_x));
  l_w = select(l_w, 0.0f, (l_w < 0.0f));
  return (l_w * l_w);
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_raw_rgb_sample(p_base: i32, p_edge: i32, p_uv: vec2f) -> vec3f {
  let l_base: i32 = p_base;
  let l_edge: i32 = p_edge;
  let l_uv: vec2f = p_uv;
  return coax_p_raw_rgb_filter(l_base, l_edge, l_uv, false);
}
fn coax_p_slay_orb_energy(p_distortion: i32, p_energy: i32, p_uv: vec2f, p_time: f32) -> f32 {
  let l_distortion: i32 = p_distortion;
  let l_energy: i32 = p_energy;
  let l_uv: vec2f = p_uv;
  let l_time: f32 = p_time;
  var l_a: vec3f;
  var l_b: vec3f;
  var l_first: vec3f;
  var l_second: vec3f;
  l_a = coax_p_raw_rgb_sample(l_distortion, 256i, coax_p_v2(((l_uv.x * 0x1.000000p1f) + (l_time * 0x1.99999ap-4f)), ((l_uv.y * 0x1.000000p1f) + (l_time * 0x1.99999ap-4f))));
  l_b = coax_p_raw_rgb_sample(l_distortion, 256i, coax_p_v2((l_uv.x + (l_time * 0x1.47ae14p-5f)), (l_uv.y - (l_time * 0x1.333334p-3f))));
  l_first = coax_p_raw_rgb_srgb_sample(l_energy, 1024i, coax_p_v2(((l_uv.x + (((l_a.x * 0x1.000000p1f) - 0x1.000000p0f) * 0x1.333334p-3f)) - (l_time * 0x1.47ae14p-7f)), ((l_uv.y + (((l_a.y * 0x1.000000p1f) - 0x1.000000p0f) * 0x1.333334p-3f)) - (l_time * 0x1.47ae14p-7f))));
  l_second = coax_p_raw_rgb_srgb_sample(l_energy, 1024i, coax_p_v2((((l_uv.x * 0x1.000000p1f) + (((l_b.x * 0x1.000000p1f) - 0x1.000000p0f) * 0x1.000000p-3f)) + (l_time * 0x1.47ae14p-7f)), (((l_uv.y * 0x1.000000p1f) + (((l_b.y * 0x1.000000p1f) - 0x1.000000p0f) * 0x1.000000p-3f)) + (l_time * 0x1.000000p-2f))));
  return (l_first.x * l_second.x);
}
fn coax_p_slay_orb_emission(p_bump_dot: f32, p_nv: f32, p_energy: f32) -> vec4f {
  let l_bump_dot: f32 = p_bump_dot;
  let l_nv: f32 = p_nv;
  let l_energy: f32 = p_energy;
  var l_facing: f32;
  var l_base: f32;
  var l_e: f32;
  var l_hdr: f32;
  var l_r: f32;
  var l_g: f32;
  var l_b: f32;
  var l_l: f32;
  l_facing = coax_p_clampf(l_nv, 0.0f, 0x1.000000p0f);
  l_base = (coax_pow_bg_vs(coax_pow_bg_vs(coax_p_clampf(l_bump_dot, 0.0f, 0x1.000000p0f), 0x1.000000p3f), 0x1.333334p0f) * 0x1.400000p4f);
  l_e = (coax_p_clampf(l_energy, 0.0f, 0x1.000000p0f) * coax_pow_bg_vs(l_facing, 0x1.000000p2f));
  l_hdr = (((0x1.900000p7f * l_e) * l_e) / (0x1.940000p6f - (0x1.900000p6f * l_e)));
  l_r = (l_base * 0x1.9a0f0ap-7f);
  l_g = ((l_base * 0x1.541c82p-2f) + (((l_hdr * 0x1.33e942p-1f) * 0x1.22cb20p-3f) * 0x1.900000p4f));
  l_b = (l_base + (l_hdr * 0x1.900000p4f));
  l_l = (((l_r * 0x1.333334p-2f) + (l_g * 0x1.2e147ap-1f)) + (l_b * 0x1.c28f5cp-4f));
  return coax_p_v4(((l_r * 0x1.800000p-1f) + (l_l * 0x1.000000p-2f)), ((l_g * 0x1.800000p-1f) + (l_l * 0x1.000000p-2f)), ((l_b * 0x1.800000p-1f) + (l_l * 0x1.000000p-2f)), coax_pow_bg_vs(l_facing, 0x1.cf08ccp0f));
}
fn coax_p_cv_profile_head_colour(p_a: vec3f, p_valley: bool) -> vec3f {
  let l_a: vec3f = p_a;
  let l_valley: bool = p_valley;
  return select(coax_p_cv_echo_slay_head_colour(l_a), coax_p_cv_echo_head_colour(l_a), l_valley);
}
fn coax_p_cv_echo_head_f0(p_r: f32, p_g: f32, p_mask: f32) -> f32 {
  let l_r: f32 = p_r;
  let l_g: f32 = p_g;
  let l_mask: f32 = p_mask;
  return (((0x1.47ae14p-4f * coax_p_mix(0x1.8fd8f0p-3f, 0x1.d5d570p-1f, l_r)) * coax_p_mix(0x1.3127f6p-2f, 0x1.000000p0f, l_g)) * l_mask);
}
fn coax_p_cv_echo_head_ao(p_r: f32) -> f32 {
  let l_r: f32 = p_r;
  return (0x1.000000p-2f + (0x1.800000p-1f * coax_pow_bg_vs(coax_p_fmax(l_r, 0.0f), 0x1.99999ap0f)));
}
fn coax_p_cv_echo_body_colour(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_v3((l_a.x * 0x1.ccccccp-1f), (l_a.y * 0x1.2a09fep0f), (l_a.z * 0x1.400000p0f));
}
fn coax_p_cv_echo_pupil(p_uv: vec2f) -> vec2f {
  let l_uv: vec2f = p_uv;
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
  let l_a: vec2f = p_a;
  let l_k: f32 = p_k;
  return coax_p_v2((l_a.x * l_k), (l_a.y * l_k));
}
fn coax_p_cv_echo_eye_shadow(p_radius: f32) -> f32 {
  let l_radius: f32 = p_radius;
  return coax_p_clampf(((0x1.000000p0f - (l_radius / 0x1.6147aep-2f)) / 0x1.db890ep-2f), 0.0f, 0x1.000000p0f);
}
fn coax_p_cv_echo_core_material(p_m: f32) -> bool {
  let l_m: f32 = p_m;
  return (((((((((((l_m == 0.0f) || (l_m == 0x1.000000p0f)) || (l_m == 0x1.000000p1f)) || (l_m == 0x1.000000p2f)) || (l_m == 0x1.400000p2f)) || (l_m == 0x1.800000p2f)) || (l_m == 0x1.000000p3f)) || (l_m == 0x1.a00000p3f)) || (l_m == 0x1.e00000p3f)) || (l_m == 0x1.700000p4f)) || (l_m == 0x1.900000p4f));
}
fn coax_p_cv_echo_normal(p_a: vec4f, p_strength: f32) -> vec3f {
  let l_a: vec4f = p_a;
  let l_strength: f32 = p_strength;
  return coax_p_v3((((coax_p_cv_srgb(l_a.x) * 0x1.000000p1f) - 0x1.000000p0f) * l_strength), ((0x1.000000p0f - (coax_p_cv_srgb(l_a.y) * 0x1.000000p1f)) * l_strength), ((coax_p_cv_srgb(l_a.z) * 0x1.000000p1f) - 0x1.000000p0f));
}
fn coax_p_cv_echo_normal_blend(p_base: vec3f, p_detail: vec3f) -> vec3f {
  let l_base: vec3f = p_base;
  let l_detail: vec3f = p_detail;
  var l_t: vec3f;
  var l_d: vec3f;
  l_t = coax_p_v3(l_base.x, l_base.y, (l_base.z + 0x1.000000p0f));
  l_d = coax_p_v3((-l_detail.x), (-l_detail.y), l_detail.z);
  return coax_p_cv_sub(coax_p_cv_scale(l_t, (coax_p_wg_dot3(l_t, l_d) / l_t.z)), l_d);
}
fn coax_p_cv_echo_overrides(p_base: vec3f, p_rolloff: f32, p_desat: f32) -> vec3f {
  let l_base: vec3f = p_base;
  let l_rolloff: f32 = p_rolloff;
  let l_desat: f32 = p_desat;
  var l_c: vec3f;
  var l_l: f32;
  l_c = coax_p_v3((coax_pow_bg_vs(coax_p_fmax(0.0f, (l_base.x * 0x1.000000p2f)), (0x1.000000p0f / 0x1.333334p0f)) * l_rolloff), (coax_pow_bg_vs(coax_p_fmax(0.0f, (l_base.y * 0x1.000000p2f)), (0x1.000000p0f / 0x1.333334p0f)) * l_rolloff), (coax_pow_bg_vs(coax_p_fmax(0.0f, (l_base.z * 0x1.000000p2f)), (0x1.000000p0f / 0x1.333334p0f)) * l_rolloff));
  l_l = coax_p_wg_dot3(l_c, coax_p_v3(0x1.333334p-2f, 0x1.2e147ap-1f, 0x1.c28f5cp-4f));
  return coax_p_cv_add(coax_p_cv_scale(l_c, (0x1.000000p0f - l_desat)), coax_p_v3((l_l * l_desat), (l_l * l_desat), (l_l * l_desat)));
}
fn coax_p_render_filter_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t32 {
  let l_uv: vec2f = p_uv;
  let l_layer: i32 = p_layer;
  let l_dx: vec2f = p_dx;
  let l_dy: vec2f = p_dy;
  let l_bias: f32 = p_bias;
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
fn coax_p_film_max(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_film_scale(p_c: vec3f, p_s: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_s: f32 = p_s;
  return coax_p_film_rgb((l_c.x * l_s), (l_c.y * l_s), (l_c.z * l_s));
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
fn coax_p_film_sat(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_film_min(0x1.000000p0f, coax_p_film_max(0.0f, l_x));
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
fn coax_p_cv_skin_light(p_base: vec3f, p_f0: vec3f, p_rough: f32, p_n: vec3f, p_g: vec3f, p_l: vec3f, p_v: vec3f, p_spec_scale: f32, p_size: f32) -> vec3f {
  let l_base: vec3f = p_base;
  let l_f0: vec3f = p_f0;
  let l_rough: f32 = p_rough;
  let l_n: vec3f = p_n;
  let l_g: vec3f = p_g;
  let l_l: vec3f = p_l;
  let l_v: vec3f = p_v;
  let l_spec_scale: f32 = p_spec_scale;
  let l_size: f32 = p_size;
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
  let l_base: vec3f = p_base;
  let l_f0: vec3f = p_f0;
  let l_metal: f32 = p_metal;
  let l_rough: f32 = p_rough;
  let l_n: vec3f = p_n;
  let l_l: vec3f = p_l;
  let l_v: vec3f = p_v;
  let l_spec_scale: f32 = p_spec_scale;
  let l_size: f32 = p_size;
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
  l_f = coax_pow_bg_vs((0x1.000000p0f - l_vh), 0x1.400000p2f);
  return coax_p_cv_add(coax_p_cv_scale(l_base, ((l_nl * (0x1.000000p0f - l_metal)) / 0x1.921fb6p1f)), coax_p_cv_scale(coax_p_v3((l_f0.x + ((0x1.000000p0f - l_f0.x) * l_f)), (l_f0.y + ((0x1.000000p0f - l_f0.y) * l_f)), (l_f0.z + ((0x1.000000p0f - l_f0.z) * l_f))), l_scale));
}
fn coax_p_cut_distance(p_x: f32, p_y: f32, p_w: f32, p_h: f32, p_k: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_w: f32 = p_w;
  let l_h: f32 = p_h;
  let l_k: f32 = p_k;
  var l_dx: f32;
  var l_dy: f32;
  var l_box: f32;
  l_dx = (abs(l_x) - l_w);
  l_dy = (abs(l_y) - l_h);
  l_box = (sqrt(((coax_p_hi(l_dx, 0.0f) * coax_p_hi(l_dx, 0.0f)) + (coax_p_hi(l_dy, 0.0f) * coax_p_hi(l_dy, 0.0f)))) + coax_p_lo(coax_p_hi(l_dx, l_dy), 0.0f));
  return coax_p_hi(l_box, ((abs((l_x + l_y)) - ((l_w + l_h) - (0x1.c00000p3f * l_k))) * 0x1.6a09e6p-1f));
}
fn coax_p_smooth(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_sat(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_hi(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_hf_max(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_hf_clamp(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_lo: f32 = p_lo;
  let l_hi: f32 = p_hi;
  return select(select(l_x, l_hi, (l_x > l_hi)), l_lo, (l_x < l_lo));
}
fn coax_p_hair_gaussian(p_width: f32, p_angle: f32) -> f32 {
  let l_width: f32 = p_width;
  let l_angle: f32 = p_angle;
  return (exp(((((-0x1.000000p-1f) * l_angle) * l_angle) / (l_width * l_width))) / (0x1.40d932p1f * coax_p_hf_max(l_width, 0x1.47ae14p-7f)));
}
fn coax_p_hair_fresnel(p_c: f32) -> f32 {
  let l_c: f32 = p_c;
  var l_f: f32;
  l_f = 0x1.7d18b4p-5f;
  return (l_f + ((0x1.000000p0f - l_f) * coax_pow_bg_vs((0x1.000000p0f - l_c), 0x1.400000p2f)));
}
fn coax_p_hf_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_cv_hair_asin(p_s: f32) -> f32 {
  let l_s: f32 = p_s;
  return coax_p_wg_atan2(l_s, sqrt(coax_p_fmax(0.0f, (0x1.000000p0f - (l_s * l_s)))));
}
fn coax_p_cv_dual_channel(p_af: f32, p_ab: f32, p_bf: f32, p_bb: f32, p_shift: f32, p_theta: f32, p_count: f32, p_fs: f32) -> f32 {
  let l_af: f32 = p_af;
  let l_ab: f32 = p_ab;
  let l_bf: f32 = p_bf;
  let l_bb: f32 = p_bb;
  let l_shift: f32 = p_shift;
  let l_theta: f32 = p_theta;
  let l_count: f32 = p_count;
  let l_fs: f32 = p_fs;
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
  l_global = coax_p_mix(0x1.000000p0f, (((coax_pow_bg_vs(l_af, l_count) * exp(((((-0x1.000000p-1f) * l_theta) * l_theta) / l_sf2))) / 0x1.921fb6p1f) * 0x1.666666p-1f), coax_p_clampf(l_count, 0.0f, 0x1.000000p0f));
  return (l_global * (l_fs + l_local));
}
fn coax_p_raw_rgb_filter(p_base: i32, p_edge: i32, p_uv: vec2f, p_decode: bool) -> vec3f {
  let l_base: i32 = p_base;
  let l_edge: i32 = p_edge;
  let l_uv: vec2f = p_uv;
  let l_decode: bool = p_decode;
  var l_x: f32;
  var l_y: f32;
  var l_ix: i32;
  var l_iy: i32;
  var l_fx: f32;
  var l_fy: f32;
  var l_a: vec3f;
  var l_b: vec3f;
  l_x = (((l_uv.x - floor(l_uv.x)) * f32(l_edge)) - 0x1.000000p-1f);
  l_y = (((l_uv.y - floor(l_uv.y)) * f32(l_edge)) - 0x1.000000p-1f);
  l_ix = i32(floor(l_x));
  l_iy = i32(floor(l_y));
  l_fx = (l_x - floor(l_x));
  l_fy = (l_y - floor(l_y));
  l_a = coax_p_raw_rgb_lerp(coax_p_raw_rgb_decode(coax_p_raw_rgb_texel(l_base, l_edge, l_ix, l_iy), l_decode), coax_p_raw_rgb_decode(coax_p_raw_rgb_texel(l_base, l_edge, (l_ix + 1i), l_iy), l_decode), l_fx);
  l_b = coax_p_raw_rgb_lerp(coax_p_raw_rgb_decode(coax_p_raw_rgb_texel(l_base, l_edge, l_ix, (l_iy + 1i)), l_decode), coax_p_raw_rgb_decode(coax_p_raw_rgb_texel(l_base, l_edge, (l_ix + 1i), (l_iy + 1i)), l_decode), l_fx);
  return coax_p_raw_rgb_lerp(l_a, l_b, l_fy);
}
fn coax_p_raw_rgb_srgb_sample(p_base: i32, p_edge: i32, p_uv: vec2f) -> vec3f {
  let l_base: i32 = p_base;
  let l_edge: i32 = p_edge;
  let l_uv: vec2f = p_uv;
  return coax_p_raw_rgb_filter(l_base, l_edge, l_uv, true);
}
fn coax_p_cv_echo_head_colour(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  var l_c: vec3f;
  var l_l: f32;
  l_c = coax_p_cv_scale(l_a, 0x1.400000p1f);
  l_l = (((l_c.x * 0x1.333334p-2f) + (l_c.y * 0x1.2e147ap-1f)) + (l_c.z * 0x1.c28f5cp-4f));
  return coax_p_v3((coax_p_clampf(((((l_c.x * 0x1.99999ap-1f) + (l_l * 0x1.99999ap-3f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f) * 0x1.ccccccp-1f), coax_p_clampf(((((l_c.y * 0x1.99999ap-1f) + (l_l * 0x1.99999ap-3f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f), (coax_p_clampf(((((l_c.z * 0x1.99999ap-1f) + (l_l * 0x1.99999ap-3f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f) * 0x1.e5e5f4p-1f));
}
fn coax_p_cv_echo_slay_head_colour(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  var l_c: vec3f;
  var l_l: f32;
  l_c = coax_p_cv_scale(l_a, 0x1.400000p1f);
  l_l = (((l_c.x * 0x1.333334p-2f) + (l_c.y * 0x1.2e147ap-1f)) + (l_c.z * 0x1.c28f5cp-4f));
  return coax_p_v3((coax_p_clampf(((((l_c.x * 0x1.666666p-1f) + (l_l * 0x1.333334p-2f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f) * 0x1.d7d998p-1f), coax_p_clampf(((((l_c.y * 0x1.666666p-1f) + (l_l * 0x1.333334p-2f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f), (coax_p_clampf(((((l_c.z * 0x1.666666p-1f) + (l_l * 0x1.333334p-2f)) * 0x1.eb851ep-1f) + 0x1.47ae14p-6f), 0.0f, 0x1.000000p0f) * 0x1.be23d0p-1f));
}
fn coax_p_render_footprint(p_width: i32, p_height: i32, p_du_dx: f32, p_dv_dx: f32, p_du_dy: f32, p_dv_dy: f32, p_bias: f32) -> coax_t33 {
  let l_width: i32 = p_width;
  let l_height: i32 = p_height;
  let l_du_dx: f32 = p_du_dx;
  let l_dv_dx: f32 = p_dv_dx;
  let l_du_dy: f32 = p_du_dy;
  let l_dv_dy: f32 = p_dv_dy;
  let l_bias: f32 = p_bias;
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
  let l_a: vec2f = p_a;
  let l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
}
fn coax_p_add2(p_a: vec2f, p_b: vec2f) -> vec2f {
  let l_a: vec2f = p_a;
  let l_b: vec2f = p_b;
  return vec2f((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_add4(p_a: vec4f, p_b: vec4f) -> vec4f {
  let l_a: vec4f = p_a;
  let l_b: vec4f = p_b;
  return vec4f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
}
fn coax_p_scale4(p_a: vec4f, p_k: f32) -> vec4f {
  let l_a: vec4f = p_a;
  let l_k: f32 = p_k;
  return vec4f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k), (l_a.w * l_k));
}
fn coax_p_cv_area_a2(p_a: f32, p_size: f32, p_vh: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_size: f32 = p_size;
  let l_vh: f32 = p_vh;
  return coax_p_fmin(0x1.000000p0f, ((l_a * l_a) + (((0x1.000000p-2f * l_size) * ((0x1.800000p1f * l_a) + l_size)) / (l_vh + 0x1.0624dep-10f))));
}
fn coax_p_lo(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_sat(p_a: f32) -> f32 {
  let l_a: f32 = p_a;
  return coax_p_lo(0x1.000000p0f, coax_p_hi(0.0f, l_a));
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
fn coax_p_raw_rgb_texel(p_base: i32, p_edge: i32, p_x: i32, p_y: i32) -> vec3f {
  let l_base: i32 = p_base;
  let l_edge: i32 = p_edge;
  let l_x: i32 = p_x;
  let l_y: i32 = p_y;
  var l_px: i32;
  var l_py: i32;
  var l_word: u32;
  l_px = (((l_x % l_edge) + l_edge) % l_edge);
  l_py = (((l_y % l_edge) + l_edge) % l_edge);
  l_word = vertices[((l_base + (l_py * l_edge)) + l_px)];
  return coax_p_v3((f32((l_word & 255u)) / 0x1.fe0000p7f), (f32(((l_word >> u32(8u)) & 255u)) / 0x1.fe0000p7f), (f32(((l_word >> u32(16u)) & 255u)) / 0x1.fe0000p7f));
}
fn coax_p_raw_rgb_decode(p_a: vec3f, p_decode: bool) -> vec3f {
  let l_a: vec3f = p_a;
  let l_decode: bool = p_decode;
  if ((!l_decode))
  {
    return l_a;
  }
  return coax_p_v3(coax_p_raw_srgb_channel(l_a.x), coax_p_raw_srgb_channel(l_a.y), coax_p_raw_srgb_channel(l_a.z));
}
fn coax_p_raw_rgb_lerp(p_a: vec3f, p_b: vec3f, p_k: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_k: f32 = p_k;
  return coax_p_v3((l_a.x + ((l_b.x - l_a.x) * l_k)), (l_a.y + ((l_b.y - l_a.y) * l_k)), (l_a.z + ((l_b.z - l_a.z) * l_k)));
}
fn coax_p_render_footprint_finite(p_x: f32) -> bool {
  let l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_footprint_max(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return (((l_a + l_b) + abs((l_a - l_b))) * 0x1.000000p-1f);
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
fn coax_p_raw_srgb_channel(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return select(coax_pow_bg_vs(((l_x + 0x1.c28f5cp-5f) / 0x1.0e147ap0f), 0x1.333334p1f), (l_x / 0x1.9d70a4p3f), (l_x <= 0x1.4b5dccp-5f));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_11 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8, coax_r.f9);
}
@vertex
fn shadow_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_shadow_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_16) -> coax_io_18 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t16(coax_in.x, coax_in.y, coax_in.z));
  return coax_io_18(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_19 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_19(coax_r.x, coax_r.y);
}
@fragment
fn scene_fs(coax_in: coax_io_11) -> coax_out_19 {
  let coax_r = coax_p_scene_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8, coax_in.f9));
  return coax_out_19(coax_r.x, coax_r.y);
}
@fragment
fn shadow_fs(coax_in: coax_io_15) {
  coax_p_shadow_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
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