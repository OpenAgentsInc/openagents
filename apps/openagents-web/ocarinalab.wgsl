// ---- generated: apps/openagents-web/gpu/ocarinalab.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec3f,
  y: vec3f,
  z: vec2f,
  w: vec2f,
  f4: vec4f,
  f5: vec4f,
  f6: f32,
  f7: f32,
}
struct coax_t12 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec2f,
  f4: vec2f,
  f5: vec4f,
  f6: vec4f,
  f7: f32,
  f8: f32,
}
struct coax_io_12 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec2f,
  @location(3) f4: vec2f,
  @location(4) f5: vec4f,
  @location(5) f6: vec4f,
  @location(6) f7: f32,
  @location(7) f8: f32,
}
struct coax_t13 {
  x: vec4f,
  y: vec3f,
  z: vec2f,
  w: vec2f,
  f4: f32,
  f5: f32,
}
struct coax_io_13 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec2f,
  @location(2) w: vec2f,
  @location(3) f4: f32,
  @location(4) f5: f32,
}
struct coax_t14 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_in_14 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
}
struct coax_t15 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t16 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_16 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t21 {
  x: vec2f,
  y: vec2f,
}
struct coax_t22 {
  x: vec3f,
  y: vec3f,
  z: f32,
  w: f32,
  f4: f32,
  f5: u32,
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 16>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> vertices: array<u32>;
@group(1) @binding(1) var<storage, read> bones: array<f32>;
@group(1) @binding(2) var sheets: texture_2d_array<f32>;
@group(1) @binding(3) var sheet: sampler;
@group(2) @binding(0) var shadow_map: texture_depth_2d;
@group(2) @binding(1) var shadow_cmp: sampler_comparison;
fn coax_p_bg_vs(p_id: u32, p_instance: u32) -> vec4f {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  return coax_p_splash_bg_vs(l_id, l_instance);
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t12 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  var l_v: coax_t11;
  var l_r: vec3f;
  var l_z: f32;
  l_v = coax_p_og_vertex(l_id);
  l_r = coax_p_sub3(l_v.x, coax_p_og_at(8i));
  l_z = dot(l_r, coax_p_og_at(20i));
  return coax_t12(coax_p_v4((dot(l_r, coax_p_og_at(12i)) * coax_at_u(i32(15i))), (dot(l_r, coax_p_og_at(16i)) * coax_at_u(i32(19i))), (0x1.0004aep0f * (l_z - 0x1.99999ap-5f)), l_z), l_v.x, l_v.y, l_v.z, l_v.w, l_v.f4, l_v.f5, l_v.f6, l_v.f7);
}
fn coax_p_shadow_vs(p_id: u32, p_instance: u32) -> coax_t13 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  var l_v: coax_t11;
  var l_q: vec3f;
  l_v = coax_p_og_vertex(l_id);
  l_q = coax_p_og_shadow_point(l_v.x);
  return coax_t13(coax_p_v4(l_q.x, l_q.y, l_q.z, 0x1.000000p0f), l_v.x, l_v.z, l_v.w, l_v.f6, l_v.f7);
}
fn coax_p_fx_vs(p_id: u32, p_instance: u32, p_a: coax_t14) -> coax_t16 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  let l_a: coax_t14 = p_a;
  return coax_p_splash_fx_vs(l_id, l_instance, coax_t15(l_a.x, l_a.y, l_a.z, coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f)));
}
fn coax_p_bg_fs(p_p: vec4f) -> vec4f {
  let l_p: vec4f = p_p;
  var l_px: f32;
  var l_py: f32;
  var l_ray: vec3f;
  var l_sky: vec3f;
  l_px = l_p.x;
  l_py = l_p.y;
  l_ray = coax_p_normalize3(coax_p_add3(coax_p_og_at(20i), coax_p_add3(coax_p_scale3(coax_p_og_at(12i), ((((l_px / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f) / coax_at_u(i32(15i)))), coax_p_scale3(coax_p_og_at(16i), ((0x1.000000p0f - ((l_py / coax_at_u(i32(1i))) * 0x1.000000p1f)) / coax_at_u(i32(19i)))))));
  l_sky = coax_p_oa_sky(l_ray, coax_p_og_at(28i));
  return coax_p_v4(coax_p_og_tone(l_sky.x), coax_p_og_tone(l_sky.y), coax_p_og_tone(l_sky.z), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t12) -> vec4f {
  let l_s: coax_t12 = p_s;
  var l_g0: coax_t21;
  var l_g1: coax_t21;
  var l_ground_uv: vec2f;
  var l_ground_g: coax_t21;
  var l_soil_uv: vec2f;
  var l_soil_g: coax_t21;
  var l_n: vec3f;
  var l_view: vec3f;
  var l_sun: vec3f;
  var l_colour: vec3f;
  var l_rough: f32;
  var l_ao: f32;
  var l_alpha: f32;
  var l_kind: u32;
  var l_m: i32;
  var l_uv: vec2f;
  var l_g: coax_t21;
  var l_a: vec4f;
  var l_soil: vec4f;
  var l_patch: f32;
  var l_blend: f32;
  var l_eye_scale: f32;
  var l_euv: vec2f;
  var l_iris: vec4f;
  var l_white: vec4f;
  var l_mask: vec4f;
  var l_f: f32;
  var l_normal_uv: vec2f;
  var l_normal_g: coax_t21;
  var l_layer: u32;
  var l_aN: vec4f;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_t0: vec3f;
  var l_fallback: vec3f;
  var l_t: vec3f;
  var l_b: vec3f;
  var l_material: coax_t22;
  var l_q: vec3f;
  var l_shadow: f32;
  var l_y_48: i32;
  var l_x_49: i32;
  var l_ndl: f32;
  var l_ambient: f32;
  var l_lighting: f32;
  var l_h: vec3f;
  var l_spec: f32;
  var l_highlight: f32;
  var l_vertical: bool;
  var l_reflected: vec3f;
  var l_sky: vec3f;
  var l_fresnel: f32;
  var l_glint: f32;
  var l_flow: f32;
  var l_m_62: i32;
  var l_a_63: vec4f;
  var l_m_64: i32;
  var l_em: vec4f;
  var l_distance: f32;
  var l_height_density: f32;
  var l_fog: f32;
  var l_fog_sky: vec3f;
  var l_opacity: f32;
  l_g0 = coax_p_og_grad(l_s.w);
  l_g1 = coax_p_og_grad(l_s.f4);
  l_ground_uv = coax_p_v2((l_s.y.x * 0x1.000000p-2f), (l_s.y.z * 0x1.000000p-2f));
  l_ground_g = coax_p_og_grad(l_ground_uv);
  l_soil_uv = coax_p_v2(((l_s.y.x * 0x1.851eb8p-2f) + (l_s.y.z * 0x1.eb851ep-4f)), ((l_s.y.z * 0x1.851eb8p-2f) - (l_s.y.x * 0x1.eb851ep-4f)));
  l_soil_g = coax_p_og_grad(l_soil_uv);
  if (((l_s.f8 < 0x1.000000p-1f) && (coax_at_u(i32(49i)) < 0x1.000000p-1f)))
  {
    discard;
  }
  l_n = coax_p_og_unit(l_s.z, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f));
  l_view = coax_p_normalize3(coax_p_sub3(coax_p_og_at(8i), l_s.y));
  l_sun = coax_p_og_at(28i);
  l_colour = coax_p_v3(l_s.f6.x, l_s.f6.y, l_s.f6.z);
  l_rough = 0x1.b33334p-1f;
  l_ao = 0x1.000000p0f;
  l_alpha = 0x1.000000p0f;
  l_kind = 0u;
  if ((l_s.f8 < 0x1.000000p-1f))
  {
    {
      l_m = (i32(vertices[5i]) + (i32(floor((l_s.f7 + 0x1.000000p-1f))) * 64i));
      l_kind = vertices[(l_m + 16i)];
      if (((l_kind == 4u) || (l_kind == 6u)))
      {
        discard;
      }
      l_uv = l_s.w;
      l_g = l_g0;
      if ((l_kind == 9u))
      {
        l_uv = l_ground_uv;
        l_g = l_ground_g;
      }
      if ((l_kind == 7u))
      {
        l_g = coax_p_og_foot_scale(l_g1, coax_p_og_word((l_m + 33i)));
        l_uv = coax_p_v2((l_s.f4.x * coax_p_og_word((l_m + 33i))), (l_s.f4.y * coax_p_og_word((l_m + 34i))));
      }
      l_a = coax_p_og_sample(vertices[l_m], l_uv, l_g);
      l_colour = coax_p_v3(l_a.x, l_a.y, l_a.z);
      if ((l_kind == 14u))
      {
        l_colour = coax_p_v3((l_colour.x * 0x1.a3d70ap-1f), (l_colour.y * 0x1.b851ecp-1f), (l_colour.z * 0x1.d70a3ep-1f));
      }
      if ((l_kind == 9u))
      {
        l_soil = coax_p_og_sample(vertices[(l_m + 7i)], l_soil_uv, l_soil_g);
        l_patch = ((coax_p_oa_noise(coax_p_v2((l_s.y.x * 0x1.1eb852p-3f), (l_s.y.z * 0x1.1eb852p-3f))) * 0x1.666666p-1f) + (coax_p_oa_noise(coax_p_v2(((l_s.y.x * 0x1.7ae148p-2f) + 0x1.200000p3f), (l_s.y.z * 0x1.7ae148p-2f))) * 0x1.333334p-2f));
        l_blend = coax_p_oa_soil(l_patch, l_n.y);
        l_colour = coax_p_oa_mix(l_colour, coax_p_v3(l_soil.x, l_soil.y, l_soil.z), l_blend);
        l_colour = coax_p_v3((l_colour.x * 0x1.c28f5cp-1f), (l_colour.y * 0x1.99999ap-1f), (l_colour.z * 0x1.c28f5cp-1f));
      }
      l_alpha = coax_p_og_coverage(l_s.w, l_s.f4, l_s.f7, l_g0, l_g1);
      if ((l_alpha < coax_p_og_word((l_m + 18i))))
      {
        discard;
      }
      if ((l_kind == 5u))
      {
        l_colour = coax_p_og_point((l_m + 24i));
      }
      if ((l_kind == 3u))
      {
        l_eye_scale = coax_p_og_word((l_m + 47i));
        l_euv = coax_p_v2((((l_s.w.x - 0x1.000000p-1f) * l_eye_scale) + 0x1.000000p-1f), (((l_s.w.y - 0x1.000000p-1f) * l_eye_scale) + 0x1.000000p-1f));
        l_iris = coax_p_og_sample(vertices[l_m], l_euv, coax_p_og_foot_scale(l_g0, l_eye_scale));
        l_white = coax_p_og_sample(vertices[(l_m + 7i)], l_s.w, l_g0);
        l_mask = coax_p_og_sample(vertices[(l_m + 5i)], l_euv, coax_p_og_foot_scale(l_g0, l_eye_scale));
        l_f = coax_p_og_raw(l_mask.x);
        if (((((l_euv.x < 0.0f) || (l_euv.x > 0x1.000000p0f)) || (l_euv.y < 0.0f)) || (l_euv.y > 0x1.000000p0f)))
        {
          l_f = 0.0f;
        }
        l_colour = coax_p_v3(coax_p_mix(l_white.x, l_iris.x, l_f), coax_p_mix(l_white.y, l_iris.y, l_f), coax_p_mix(l_white.z, l_iris.z, l_f));
        l_rough = 0x1.70a3d8p-3f;
      }
      if ((l_kind != 3u))
      {
        if ((vertices[(l_m + 1i)] > 0u))
        {
          l_normal_uv = l_uv;
          l_normal_g = l_g;
          l_layer = vertices[(l_m + 1i)];
          if (((l_kind == 7u) && (vertices[(l_m + 4i)] > 0u)))
          {
            l_layer = vertices[(l_m + 4i)];
            l_normal_uv = l_s.w;
            l_normal_g = l_g0;
          }
          l_aN = coax_p_og_sample(l_layer, l_normal_uv, l_normal_g);
          l_x = ((coax_p_og_raw(l_aN.x) * 0x1.010204p1f) - 0x1.020408p0f);
          l_y = ((coax_p_og_raw(l_aN.y) * 0x1.010204p1f) - 0x1.020408p0f);
          l_z = sqrt(coax_p_fmax(0.0f, ((0x1.000000p0f - (l_x * l_x)) - (l_y * l_y))));
          l_t0 = coax_p_v3(l_s.f5.x, l_s.f5.y, l_s.f5.z);
          l_fallback = coax_p_og_unit(coax_p_cross3(l_n, coax_p_v3(0.0f, 0.0f, 0x1.000000p0f)), coax_p_v3(0x1.000000p0f, 0.0f, 0.0f));
          l_t = coax_p_og_unit(coax_p_sub3(l_t0, coax_p_scale3(l_n, dot(l_n, l_t0))), l_fallback);
          l_b = coax_p_scale3(coax_p_cross3(l_n, l_t), l_s.f5.w);
          l_n = coax_p_normalize3(coax_p_add3(coax_p_add3(coax_p_scale3(l_t, l_x), coax_p_scale3(l_b, l_y)), coax_p_scale3(l_n, l_z)));
        }
        l_rough = coax_p_og_word((l_m + 19i));
        if ((vertices[(l_m + 2i)] > 0u))
        {
          l_rough = coax_p_og_raw((coax_p_og_sample(vertices[(l_m + 2i)], l_uv, l_g)).x);
        }
        if ((vertices[(l_m + 3i)] > 0u))
        {
          l_ao = coax_p_og_raw((coax_p_og_sample(vertices[(l_m + 3i)], l_s.w, l_g0)).x);
        }
      }
    }
    if ((l_kind == 12u))
    {
      l_colour = coax_p_v3(0x1.89374cp-7f, 0x1.0624dep-6f, 0x1.89374cp-7f);
      l_rough = 0x1.000000p0f;
    }
  }
  else
  {
    l_material = coax_p_og_material_surface(l_s, (i32(vertices[5i]) + (i32(floor((l_s.f7 + 0x1.000000p-1f))) * 64i)), l_g0, l_g1);
    l_colour = l_material.x;
    l_n = l_material.y;
    l_rough = l_material.z;
    l_ao = l_material.w;
    l_alpha = l_material.f4;
    l_kind = l_material.f5;
  }
  l_q = coax_p_og_shadow_point(l_s.y);
  l_shadow = 0x1.000000p0f;
  if (((((abs(l_q.x) < 0x1.000000p0f) && (abs(l_q.y) < 0x1.000000p0f)) && (l_q.z > 0.0f)) && (l_q.z < 0x1.000000p0f)))
  {
    l_shadow = 0.0f;
    {
      let coax_from: i32 = 0i;
      let coax_to: i32 = 3i;
      for (l_y_48 = coax_from; l_y_48 < coax_to; l_y_48 = l_y_48 + 1i)
      {
        {
          let coax_from: i32 = 0i;
          let coax_to: i32 = 3i;
          for (l_x_49 = coax_from; l_x_49 < coax_to; l_x_49 = l_x_49 + 1i)
          {
            l_shadow = (l_shadow + (textureSampleCompareLevel(shadow_map, shadow_cmp, coax_p_v2((((l_q.x * 0x1.000000p-1f) + 0x1.000000p-1f) + (f32((l_x_49 - 1i)) * 0x1.6f0068p-11f)), ((0x1.000000p-1f - (l_q.y * 0x1.000000p-1f)) + (f32((l_y_48 - 1i)) * 0x1.6f0068p-11f))), (l_q.z - select(0x1.3a92a4p-11f, 0x1.d7dbf4p-10f, (coax_at_u(i32(49i)) < 0x1.000000p-1f)))) / 0x1.200000p3f));
          }
        }
      }
    }
  }
  l_ndl = coax_p_fmax(0.0f, dot(l_n, l_sun));
  if ((l_kind == 2u))
  {
    l_ndl = coax_p_fmax(l_ndl, ((dot(l_n, l_sun) + 0x1.99999ap-2f) / 0x1.666666p0f));
  }
  if ((l_kind == 14u))
  {
    l_ndl = (0x1.000000p-2f + (abs(dot(l_n, l_sun)) * 0x1.19999ap-1f));
  }
  l_ambient = (0x1.c28f5cp-3f + (0x1.47ae14p-3f * ((l_n.y * 0x1.000000p-1f) + 0x1.000000p-1f)));
  l_lighting = ((l_ambient * (0x1.666666p-2f + (0x1.4cccccp-1f * l_ao))) + ((l_ndl * l_shadow) * 0x1.8cccccp0f));
  l_colour = coax_p_v3(((l_colour.x * l_lighting) * 0x1.11eb86p0f), ((l_colour.y * l_lighting) * 0x1.051eb8p0f), ((l_colour.z * l_lighting) * 0x1.c28f5cp-1f));
  l_h = coax_p_normalize3(coax_p_add3(l_view, l_sun));
  l_spec = ((coax_p_oa_specular(l_ndl, coax_p_fmax(0.0f, dot(l_n, l_view)), coax_p_fmax(0.0f, dot(l_n, l_h)), coax_p_fmax(0.0f, dot(l_view, l_h)), l_rough) * l_shadow) * 0x1.8cccccp0f);
  l_colour = coax_p_add3(l_colour, coax_p_v3(l_spec, l_spec, l_spec));
  if ((l_kind == 3u))
  {
    l_highlight = (coax_pow_bg_vs(coax_p_fmax(0.0f, dot(l_n, l_h)), 0x1.900000p6f) * 0x1.99999ap-1f);
    l_colour = coax_p_add3(l_colour, coax_p_v3(l_highlight, l_highlight, l_highlight));
  }
  if ((l_kind == 11u))
  {
    l_vertical = (abs(l_s.z.y) < 0x1.000000p-1f);
    l_n = select(coax_p_oa_water_normal(l_s.y, coax_at_u(i32(48i))), coax_p_og_unit(l_s.z, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f)), l_vertical);
    l_reflected = coax_p_sub3(coax_p_scale3(l_n, (0x1.000000p1f * dot(l_n, l_view))), l_view);
    l_sky = coax_p_oa_sky(l_reflected, l_sun);
    l_fresnel = coax_p_oa_fresnel(dot(l_n, l_view));
    l_colour = coax_p_oa_mix(coax_p_v3(0x1.cac084p-6f, 0x1.70a3d8p-4f, 0x1.0a3d70p-4f), l_sky, l_fresnel);
    l_glint = (coax_pow_bg_vs(coax_p_fmax(0.0f, dot(l_n, l_h)), 0x1.cc0000p7f) * 0x1.99999ap0f);
    l_colour = coax_p_add3(l_colour, coax_p_v3(l_glint, (l_glint * 0x1.d1eb86p-1f), (l_glint * 0x1.75c290p-1f)));
    if (l_vertical)
    {
      l_flow = (0x1.000000p-1f + (0x1.000000p-1f * sin((((l_s.y.y * 0x1.100000p4f) + (coax_at_u(i32(48i)) * 0x1.000000p2f)) + (l_s.y.x * 0x1.800000p1f)))));
      l_colour = coax_p_oa_mix(coax_p_v3(0x1.47ae14p-3f, 0x1.28f5c2p-2f, 0x1.0a3d70p-2f), coax_p_v3(0x1.47ae14p-1f, 0x1.75c290p-1f, 0x1.570a3ep-1f), (l_flow * 0x1.19999ap-1f));
    }
  }
  if (((coax_at_u(i32(4i)) == 0x1.000000p0f) && (l_s.f8 > 0x1.000000p-1f)))
  {
    l_m_62 = (i32(vertices[5i]) + (i32(floor((l_s.f7 + 0x1.000000p-1f))) * 64i));
    l_a_63 = coax_p_og_sample(vertices[l_m_62], l_s.w, l_g0);
    l_colour = coax_p_v3(l_a_63.x, l_a_63.y, l_a_63.z);
  }
  if ((coax_at_u(i32(4i)) == 0x1.000000p1f))
  {
    l_colour = coax_p_add3(coax_p_scale3(l_n, 0x1.000000p-1f), coax_p_v3(0x1.000000p-1f, 0x1.000000p-1f, 0x1.000000p-1f));
  }
  if ((coax_at_u(i32(4i)) == 0x1.800000p1f))
  {
    l_colour = coax_p_v3(l_rough, l_rough, l_rough);
  }
  if ((l_kind >= 8u))
  {
    l_m_64 = (i32(vertices[5i]) + (i32(floor((l_s.f7 + 0x1.000000p-1f))) * 64i));
    if ((vertices[(l_m_64 + 61i)] > 0u))
    {
      l_em = coax_p_og_sample(vertices[(l_m_64 + 61i)], l_s.w, l_g0);
      l_colour = coax_p_add3(l_colour, coax_p_scale3(coax_p_v3(l_em.x, l_em.y, l_em.z), 0x1.800000p0f));
    }
  }
  l_distance = coax_p_wg_length3(coax_p_sub3(l_s.y, coax_p_og_at(8i)));
  l_height_density = (0x1.ccccccp-2f + (0x1.19999ap-1f * exp(((-coax_p_fmax((l_s.y.y - 0x1.b40000p6f), 0.0f)) / 0x1.200000p4f))));
  l_fog = (0x1.000000p0f - exp((((-l_distance) * 0x1.26e978p-8f) * l_height_density)));
  l_fog_sky = coax_p_oa_sky(coax_p_normalize3(coax_p_sub3(l_s.y, coax_p_og_at(8i))), l_sun);
  l_colour = coax_p_oa_mix(l_colour, l_fog_sky, l_fog);
  l_opacity = select(0x1.000000p0f, (0x1.ccccccp-2f + (coax_p_oa_fresnel(dot(l_n, l_view)) * 0x1.000000p-1f)), (l_kind == 11u));
  return coax_p_v4(coax_p_og_tone(l_colour.x), coax_p_og_tone(l_colour.y), coax_p_og_tone(l_colour.z), l_opacity);
}
fn coax_p_shadow_fs(p_s: coax_t13) {
  let l_s: coax_t13 = p_s;
  var l_g0: coax_t21;
  var l_g1: coax_t21;
  var l_m: i32;
  var l_kind: u32;
  l_g0 = coax_p_og_grad(l_s.z);
  l_g1 = coax_p_og_grad(l_s.w);
  if (((l_s.f5 < 0x1.000000p-1f) && (coax_at_u(i32(49i)) < 0x1.000000p-1f)))
  {
    discard;
  }
  l_m = (i32(vertices[5i]) + (i32(floor((l_s.f4 + 0x1.000000p-1f))) * 64i));
  l_kind = vertices[(l_m + 16i)];
  if (((l_kind == 14u) && (coax_p_oa_hash(coax_p_v2(floor((l_s.y.x * 0x1.000000p5f)), floor((l_s.y.z * 0x1.000000p5f)))) >= coax_p_oa_leaf_shadow(coax_p_wg_length3(coax_p_sub3(l_s.y, coax_p_og_at(24i)))))))
  {
    discard;
  }
  if ((((l_kind == 4u) || (l_kind == 6u)) || (l_kind == 11u)))
  {
    discard;
  }
  if ((coax_p_og_coverage(l_s.z, l_s.w, l_s.f4, l_g0, l_g1) < coax_p_og_word((l_m + 18i))))
  {
    discard;
  }
}
fn coax_p_fx_fs(p_v: coax_t16) -> vec4f {
  let l_v: coax_t16 = p_v;
  return coax_p_splash_fx_fs(l_v);
}
fn coax_p_splash_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  let l_i: u32 = p_i;
  let l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_og_vertex(p_id: u32) -> coax_t11 {
  let l_id: u32 = p_id;
  var l_at: i32;
  var l_n: vec3f;
  var l_t: vec3f;
  var l_w: f32;
  var l_p: vec3f;
  var l_mat: i32;
  var l_m: i32;
  var l_at_11: i32;
  var l_p_12: vec3f;
  var l_normal: vec3f;
  var l_t_14: vec3f;
  var l_posed: vec3f;
  var l_n_16: vec3f;
  var l_tangent: vec3f;
  var l_yaw: f32;
  if ((l_id >= vertices[2i]))
  {
    l_at = (i32(vertices[9i]) + (i32((l_id - vertices[2i])) * 16i));
    l_n = coax_p_og_snorm(vertices[(l_at + 3i)]);
    l_t = coax_p_og_snorm(vertices[(l_at + 9i)]);
    l_w = select(0x1.000000p0f, (-0x1.000000p0f), ((vertices[(l_at + 9i)] >> u32(30u)) == 3u));
    l_p = coax_p_og_point(l_at);
    l_mat = i32(vertices[(l_at + 8i)]);
    l_m = (i32(vertices[5i]) + (l_mat * 64i));
    return coax_t11(l_p, l_n, coax_p_v2(coax_p_og_word((l_at + 4i)), coax_p_og_word((l_at + 5i))), coax_p_v2(coax_p_og_word((l_at + 6i)), coax_p_og_word((l_at + 7i))), coax_p_v4(l_t.x, l_t.y, l_t.z, l_w), coax_p_wg_unpack4x8unorm(vertices[(l_at + 10i)]), f32(l_mat), 0.0f);
  }
  l_at_11 = (i32(vertices[4i]) + (i32(l_id) * 32i));
  l_p_12 = coax_p_og_point(l_at_11);
  l_normal = coax_p_og_point((l_at_11 + 3i));
  l_t_14 = coax_p_og_point((l_at_11 + 15i));
  l_posed = coax_p_og_skin(coax_p_v4(l_p_12.x, l_p_12.y, l_p_12.z, 0x1.000000p0f), l_at_11);
  l_n_16 = coax_p_og_skin(coax_p_v4(l_normal.x, l_normal.y, l_normal.z, 0.0f), l_at_11);
  l_tangent = coax_p_og_skin(coax_p_v4(l_t_14.x, l_t_14.y, l_t_14.z, 0.0f), l_at_11);
  l_yaw = coax_at_u(i32(27i));
  return coax_t11(coax_p_add3(coax_p_og_yaw(l_posed, l_yaw), coax_p_og_at(24i)), coax_p_og_yaw(l_n_16, l_yaw), coax_p_v2(coax_p_og_word((l_at_11 + 6i)), coax_p_og_word((l_at_11 + 7i))), coax_p_v2(coax_p_og_word((l_at_11 + 20i)), coax_p_og_word((l_at_11 + 21i))), coax_p_v4((coax_p_og_yaw(l_tangent, l_yaw)).x, (coax_p_og_yaw(l_tangent, l_yaw)).y, (coax_p_og_yaw(l_tangent, l_yaw)).z, coax_p_og_word((l_at_11 + 18i))), coax_p_wg_unpack4x8unorm(vertices[(l_at_11 + 19i)]), f32(vertices[(l_at_11 + 8i)]), 0x1.000000p0f);
}
fn coax_p_og_at(p_i: i32) -> vec3f {
  let l_i: i32 = p_i;
  return coax_p_v3(coax_at_u(i32(l_i)), coax_at_u(i32((l_i + 1i))), coax_at_u(i32((l_i + 2i))));
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  let l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_og_shadow_point(p_p: vec3f) -> vec3f {
  let l_p: vec3f = p_p;
  var l_r: vec3f;
  l_r = coax_p_sub3(l_p, coax_p_og_at(44i));
  return coax_p_v3((dot(l_r, coax_p_og_at(32i)) / coax_at_u(i32(47i))), (dot(l_r, coax_p_og_at(36i)) / coax_at_u(i32(47i))), (0x1.000000p-1f - (dot(l_r, coax_p_og_at(40i)) / (coax_at_u(i32(47i)) * 0x1.000000p1f))));
}
fn coax_p_splash_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t15) -> coax_t16 {
  let l__vid2: u32 = p__vid2;
  let l__iid3: u32 = p__iid3;
  let l_in_: coax_t15 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t16;
  var l_ndc: vec2f;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t16();
  l_ndc = coax_p_v2((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)));
  l_o.x = coax_p_v4(l_ndc.x, l_ndc.y, 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_oa_sky(p_ray: vec3f, p_sun: vec3f) -> vec3f {
  let l_ray: vec3f = p_ray;
  let l_sun: vec3f = p_sun;
  var l_elevation: f32;
  var l_sky: vec3f;
  var l_p: vec2f;
  var l_cloud: f32;
  var l_veil: f32;
  var l_alignment: f32;
  var l_glow: f32;
  l_elevation = coax_p_clampf(l_ray.y, 0.0f, 0x1.000000p0f);
  l_sky = coax_p_oa_mix(coax_p_v3(0x1.28f5c2p-1f, 0x1.51eb86p-1f, 0x1.570a3ep-1f), coax_p_v3(0x1.333334p-3f, 0x1.47ae14p-2f, 0x1.23d70ap-1f), coax_pow_bg_vs(l_elevation, 0x1.19999ap-1f));
  l_p = coax_p_v2(((l_ray.x / (abs(l_ray.y) + 0x1.47ae14p-2f)) * 0x1.333334p1f), ((l_ray.z / (abs(l_ray.y) + 0x1.47ae14p-2f)) * 0x1.333334p1f));
  l_cloud = ((coax_p_oa_noise(l_p) * 0x1.4cccccp-1f) + (coax_p_oa_noise(coax_p_v2(((l_p.x * 0x1.0cccccp1f) + 0x1.100000p4f), ((l_p.y * 0x1.0cccccp1f) + 0x1.400000p2f))) * 0x1.666666p-2f));
  l_veil = ((coax_p_smoothstep(0x1.0f5c28p-1f, 0x1.8f5c28p-1f, l_cloud) * coax_p_smoothstep(0x1.eb851ep-6f, 0x1.333334p-2f, l_elevation)) * 0x1.28f5c2p-1f);
  l_sky = coax_p_oa_mix(l_sky, coax_p_v3(0x1.a3d70ap-1f, 0x1.ae147ap-1f, 0x1.9eb852p-1f), l_veil);
  l_alignment = coax_p_fmax(0.0f, dot(l_ray, l_sun));
  l_glow = ((coax_pow_bg_vs(l_alignment, 0x1.200000p4f) * 0x1.1eb852p-3f) + (coax_pow_bg_vs(l_alignment, 0x1.450000p9f) * 0x1.19999ap1f));
  return coax_p_add3(l_sky, coax_p_v3(l_glow, (l_glow * 0x1.b33334p-1f), (l_glow * 0x1.333334p-1f)));
}
fn coax_p_og_tone(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  var l_a: f32;
  l_a = coax_p_fmax(l_x, 0.0f);
  return coax_p_og_raw(coax_p_clampf(((l_a * ((0x1.4147aep1f * l_a) + 0x1.eb851ep-6f)) / ((l_a * ((0x1.370a3ep1f * l_a) + 0x1.2e147ap-1f)) + 0x1.1eb852p-3f)), 0.0f, 0x1.000000p0f));
}
fn coax_p_og_grad(p_uv: vec2f) -> coax_t21 {
  let l_uv: vec2f = p_uv;
  return coax_t21(coax_p_v2(dpdxFine(l_uv.x), dpdxFine(l_uv.y)), coax_p_v2(dpdyFine(l_uv.x), dpdyFine(l_uv.y)));
}
fn coax_p_v2(p_x: f32, p_y: f32) -> vec2f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return vec2f(l_x, l_y);
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_og_unit(p_p: vec3f, p_fallback: vec3f) -> vec3f {
  let l_p: vec3f = p_p;
  let l_fallback: vec3f = p_fallback;
  var l_l: f32;
  l_l = dot(l_p, l_p);
  if ((l_l < 0x1.5798eep-27f))
  {
    return l_fallback;
  }
  return coax_p_scale3(l_p, (0x1.000000p0f / sqrt(l_l)));
}
fn coax_p_og_word(p_i: i32) -> f32 {
  let l_i: i32 = p_i;
  return bitcast<f32>(vertices[l_i]);
}
fn coax_p_og_foot_scale(p_g: coax_t21, p_f: f32) -> coax_t21 {
  let l_g: coax_t21 = p_g;
  let l_f: f32 = p_f;
  return coax_t21(coax_p_v2((l_g.x.x * l_f), (l_g.x.y * l_f)), coax_p_v2((l_g.y.x * l_f), (l_g.y.y * l_f)));
}
fn coax_p_og_sample(p_layer: u32, p_uv: vec2f, p_g: coax_t21) -> vec4f {
  let l_layer: u32 = p_layer;
  let l_uv: vec2f = p_uv;
  let l_g: coax_t21 = p_g;
  var l_grid: i32;
  var l_tile: i32;
  var l_cell: i32;
  var l_scale: f32;
  var l_origin: vec2f;
  var l_at: vec2f;
  if ((l_layer == 0u))
  {
    return coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
  }
  l_grid = i32((l_layer >> u32(24u)));
  if ((l_grid > 0i))
  {
    l_tile = (i32(((l_layer >> u32(16u)) & 255u)) - 1i);
    l_cell = (1536i / l_grid);
    l_scale = (f32((l_cell - 16i)) / 0x1.800000p10f);
    l_origin = coax_p_v2((f32((((l_tile % l_grid) * l_cell) + 8i)) / 0x1.800000p10f), (f32((((l_tile / l_grid) * l_cell) + 8i)) / 0x1.800000p10f));
    l_at = coax_p_v2((l_origin.x + (coax_p_fract(l_uv.x) * l_scale)), (l_origin.y + (coax_p_fract(l_uv.y) * l_scale)));
    return coax_p_render_sample_layer8_grad(l_at, i32(((l_layer & 65535u) - 1u)), coax_p_v2((l_g.x.x * l_scale), (l_g.x.y * l_scale)), coax_p_v2((l_g.y.x * l_scale), (l_g.y.y * l_scale)), 0.0f);
  }
  return coax_p_render_sample_layer8_grad(l_uv, i32((l_layer - 1u)), l_g.x, l_g.y, 0.0f);
}
fn coax_p_oa_noise(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  var l_x: f32;
  var l_y: f32;
  var l_fx: f32;
  var l_fy: f32;
  var l_u: f32;
  var l_v: f32;
  l_x = floor(l_p.x);
  l_y = floor(l_p.y);
  l_fx = coax_p_fract(l_p.x);
  l_fy = coax_p_fract(l_p.y);
  l_u = ((l_fx * l_fx) * (0x1.800000p1f - (0x1.000000p1f * l_fx)));
  l_v = ((l_fy * l_fy) * (0x1.800000p1f - (0x1.000000p1f * l_fy)));
  return coax_p_mix(coax_p_mix(coax_p_oa_hash(coax_p_v2(l_x, l_y)), coax_p_oa_hash(coax_p_v2((l_x + 0x1.000000p0f), l_y)), l_u), coax_p_mix(coax_p_oa_hash(coax_p_v2(l_x, (l_y + 0x1.000000p0f))), coax_p_oa_hash(coax_p_v2((l_x + 0x1.000000p0f), (l_y + 0x1.000000p0f))), l_u), l_v);
}
fn coax_p_oa_soil(p_patch: f32, p_up: f32) -> f32 {
  let l_patch: f32 = p_patch;
  let l_up: f32 = p_up;
  return coax_p_clampf(((0x1.70a3d8p-3f + (coax_p_smoothstep(0x1.1eb852p-2f, 0x1.70a3d8p-1f, l_patch) * 0x1.0a3d70p-1f)) + (coax_p_smoothstep(0x1.333334p-3f, 0x1.4cccccp-1f, (0x1.000000p0f - l_up)) * 0x1.4cccccp-1f)), 0.0f, 0x1.000000p0f);
}
fn coax_p_oa_mix(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_t: f32 = p_t;
  return coax_p_add3(coax_p_scale3(l_a, (0x1.000000p0f - l_t)), coax_p_scale3(l_b, l_t));
}
fn coax_p_og_coverage(p_uv: vec2f, p_uv1: vec2f, p_material: f32, p_g0: coax_t21, p_g1: coax_t21) -> f32 {
  let l_uv: vec2f = p_uv;
  let l_uv1: vec2f = p_uv1;
  let l_material: f32 = p_material;
  let l_g0: coax_t21 = p_g0;
  let l_g1: coax_t21 = p_g1;
  return coax_p_og_material_coverage(l_uv, l_uv1, (i32(vertices[5i]) + (i32(floor((l_material + 0x1.000000p-1f))) * 64i)), l_g0, l_g1);
}
fn coax_p_og_point(p_i: i32) -> vec3f {
  let l_i: i32 = p_i;
  return coax_p_v3(coax_p_og_word(l_i), coax_p_og_word((l_i + 1i)), coax_p_og_word((l_i + 2i)));
}
fn coax_p_og_raw(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return select(((0x1.0e147ap0f * coax_pow_bg_vs(l_x, (0x1.000000p0f / 0x1.333334p1f))) - 0x1.c28f5cp-5f), (l_x * 0x1.9d70a4p3f), (l_x <= 0x1.9a5c38p-9f));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  if ((l_b > l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_og_material_surface(p_s: coax_t12, p_word: i32, p_g0: coax_t21, p_g1: coax_t21) -> coax_t22 {
  let l_s: coax_t12 = p_s;
  let l_word: i32 = p_word;
  let l_g0: coax_t21 = p_g0;
  let l_g1: coax_t21 = p_g1;
  var l_n: vec3f;
  var l_colour: vec3f;
  var l_rough: f32;
  var l_ao: f32;
  var l_alpha: f32;
  var l_kind: u32;
  var l_m: i32;
  var l_uv: vec2f;
  var l_g: coax_t21;
  var l_a: vec4f;
  var l_eye_scale: f32;
  var l_euv: vec2f;
  var l_iris: vec4f;
  var l_white: vec4f;
  var l_mask: vec4f;
  var l_f: f32;
  var l_normal_uv: vec2f;
  var l_normal_g: coax_t21;
  var l_layer: u32;
  var l_aN: vec4f;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_t0: vec3f;
  var l_fallback: vec3f;
  var l_t: vec3f;
  var l_b: vec3f;
  l_n = coax_p_og_unit(l_s.z, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f));
  l_colour = coax_p_v3(l_s.f6.x, l_s.f6.y, l_s.f6.z);
  l_rough = 0x1.b33334p-1f;
  l_ao = 0x1.000000p0f;
  l_alpha = 0x1.000000p0f;
  l_kind = 0u;
  {
    l_m = l_word;
    l_kind = vertices[(l_m + 16i)];
    if (((l_kind == 4u) || (l_kind == 6u)))
    {
      discard;
    }
    l_uv = l_s.w;
    l_g = l_g0;
    if ((l_kind == 7u))
    {
      l_g = coax_p_og_foot_scale(l_g1, coax_p_og_word((l_m + 33i)));
      l_uv = coax_p_v2((l_s.f4.x * coax_p_og_word((l_m + 33i))), (l_s.f4.y * coax_p_og_word((l_m + 34i))));
    }
    l_a = coax_p_og_sample(vertices[l_m], l_uv, l_g);
    l_colour = coax_p_v3(l_a.x, l_a.y, l_a.z);
    if ((l_kind == 14u))
    {
      l_colour = coax_p_v3((l_colour.x * 0x1.a3d70ap-1f), (l_colour.y * 0x1.b851ecp-1f), (l_colour.z * 0x1.d70a3ep-1f));
    }
    l_alpha = coax_p_og_material_coverage(l_s.w, l_s.f4, l_word, l_g0, l_g1);
    if ((l_alpha < coax_p_og_word((l_m + 18i))))
    {
      discard;
    }
    if ((l_kind == 5u))
    {
      l_colour = coax_p_og_point((l_m + 24i));
    }
    if ((l_kind == 3u))
    {
      l_eye_scale = coax_p_og_word((l_m + 47i));
      l_euv = coax_p_v2((((l_s.w.x - 0x1.000000p-1f) * l_eye_scale) + 0x1.000000p-1f), (((l_s.w.y - 0x1.000000p-1f) * l_eye_scale) + 0x1.000000p-1f));
      l_iris = coax_p_og_sample(vertices[l_m], l_euv, coax_p_og_foot_scale(l_g0, l_eye_scale));
      l_white = coax_p_og_sample(vertices[(l_m + 7i)], l_s.w, l_g0);
      l_mask = coax_p_og_sample(vertices[(l_m + 5i)], l_euv, coax_p_og_foot_scale(l_g0, l_eye_scale));
      l_f = coax_p_og_raw(l_mask.x);
      if (((((l_euv.x < 0.0f) || (l_euv.x > 0x1.000000p0f)) || (l_euv.y < 0.0f)) || (l_euv.y > 0x1.000000p0f)))
      {
        l_f = 0.0f;
      }
      l_colour = coax_p_v3(coax_p_mix(l_white.x, l_iris.x, l_f), coax_p_mix(l_white.y, l_iris.y, l_f), coax_p_mix(l_white.z, l_iris.z, l_f));
      l_rough = 0x1.70a3d8p-3f;
    }
    if ((l_kind != 3u))
    {
      if ((vertices[(l_m + 1i)] > 0u))
      {
        l_normal_uv = l_uv;
        l_normal_g = l_g;
        l_layer = vertices[(l_m + 1i)];
        if (((l_kind == 7u) && (vertices[(l_m + 4i)] > 0u)))
        {
          l_layer = vertices[(l_m + 4i)];
          l_normal_uv = l_s.w;
          l_normal_g = l_g0;
        }
        l_aN = coax_p_og_sample(l_layer, l_normal_uv, l_normal_g);
        l_x = ((coax_p_og_raw(l_aN.x) * 0x1.010204p1f) - 0x1.020408p0f);
        l_y = ((coax_p_og_raw(l_aN.y) * 0x1.010204p1f) - 0x1.020408p0f);
        l_z = sqrt(coax_p_fmax(0.0f, ((0x1.000000p0f - (l_x * l_x)) - (l_y * l_y))));
        l_t0 = coax_p_v3(l_s.f5.x, l_s.f5.y, l_s.f5.z);
        l_fallback = coax_p_og_unit(coax_p_cross3(l_n, coax_p_v3(0.0f, 0.0f, 0x1.000000p0f)), coax_p_v3(0x1.000000p0f, 0.0f, 0.0f));
        l_t = coax_p_og_unit(coax_p_sub3(l_t0, coax_p_scale3(l_n, dot(l_n, l_t0))), l_fallback);
        l_b = coax_p_scale3(coax_p_cross3(l_n, l_t), l_s.f5.w);
        l_n = coax_p_normalize3(coax_p_add3(coax_p_add3(coax_p_scale3(l_t, l_x), coax_p_scale3(l_b, l_y)), coax_p_scale3(l_n, l_z)));
      }
      l_rough = coax_p_og_word((l_m + 19i));
      if ((vertices[(l_m + 2i)] > 0u))
      {
        l_rough = coax_p_og_raw((coax_p_og_sample(vertices[(l_m + 2i)], l_uv, l_g)).x);
      }
      if ((vertices[(l_m + 3i)] > 0u))
      {
        l_ao = coax_p_og_raw((coax_p_og_sample(vertices[(l_m + 3i)], l_s.w, l_g0)).x);
      }
    }
  }
  if ((l_kind == 12u))
  {
    l_colour = coax_p_v3(0x1.89374cp-7f, 0x1.0624dep-6f, 0x1.89374cp-7f);
    l_rough = 0x1.000000p0f;
  }
  return coax_t22(l_colour, l_n, l_rough, l_ao, l_alpha, l_kind);
}
fn coax_p_oa_specular(p_nl: f32, p_nv: f32, p_nh: f32, p_vh: f32, p_rough: f32) -> f32 {
  let l_nl: f32 = p_nl;
  let l_nv: f32 = p_nv;
  let l_nh: f32 = p_nh;
  let l_vh: f32 = p_vh;
  let l_rough: f32 = p_rough;
  return coax_p_og_material_specular(l_nl, l_nv, l_nh, l_vh, l_rough);
}
fn coax_p_oa_water_normal(p_p: vec3f, p_time: f32) -> vec3f {
  let l_p: vec3f = p_p;
  let l_time: f32 = p_time;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  l_a = (((l_p.x * 0x1.8cccccp1f) + (l_p.z * 0x1.b33334p0f)) - (l_time * 0x1.99999ap-1f));
  l_b = (((l_p.x * 0x1.ccccccp2f) - (l_p.z * 0x1.133334p2f)) + (l_time * 0x1.19999ap0f));
  l_c = (((l_p.z * 0x1.766666p3f) + (l_p.x * 0x1.400000p1f)) - (l_time * 0x1.99999ap0f));
  return coax_p_normalize3(coax_p_v3(((cos(l_a) * 0x1.26e978p-6f) + (cos(l_b) * 0x1.47ae14p-7f)), 0x1.000000p0f, ((sin(l_a) * 0x1.eb851ep-7f) + (cos(l_c) * 0x1.0624dep-7f))));
}
fn coax_p_oa_fresnel(p_cosine: f32) -> f32 {
  let l_cosine: f32 = p_cosine;
  return (0x1.99999ap-6f + (0x1.f33334p-1f * coax_pow_bg_vs((0x1.000000p0f - coax_p_clampf(l_cosine, 0.0f, 0x1.000000p0f)), 0x1.400000p2f)));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_oa_hash(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  return coax_p_fract((sin(((l_p.x * 0x1.fc6666p6f) + (l_p.y * 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_oa_leaf_shadow(p_distance: f32) -> f32 {
  let l_distance: f32 = p_distance;
  return (0x1.000000p0f - coax_p_smoothstep(0x1.000000p3f, 0x1.400000p3f, l_distance));
}
fn coax_p_splash_fx_fs(p_v: coax_t16) -> vec4f {
  let l_v: coax_t16 = p_v;
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
fn coax_p_og_snorm(p_word: u32) -> vec3f {
  let l_word: u32 = p_word;
  return coax_p_v3(coax_p_og_signed10(l_word), coax_p_og_signed10((l_word >> u32(10u))), coax_p_og_signed10((l_word >> u32(20u))));
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  let l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_og_skin(p_value: vec4f, p_at: i32) -> vec3f {
  let l_value: vec4f = p_value;
  let l_at: i32 = p_at;
  var l_sum: f32;
  var l_out: vec3f;
  var l_k: i32;
  var l_joint: i32;
  var l_weight: f32;
  var l_b: i32;
  l_sum = 0.0f;
  l_out = vec3f();
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 8i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_joint = i32(((vertices[((l_at + 9i) + (l_k / 2i))] >> u32(u32(((l_k % 2i) * 16i)))) & 65535u));
      l_weight = f32(((vertices[((l_at + 13i) + (l_k / 4i))] >> u32(u32(((l_k % 4i) * 8i)))) & 255u));
      if ((l_weight > 0.0f))
      {
        l_b = (l_joint * 12i);
        l_out = coax_p_add3(l_out, coax_p_scale3(coax_p_v3(dot(coax_p_wg_at4(l_b), l_value), dot(coax_p_wg_at4((l_b + 4i)), l_value), dot(coax_p_wg_at4((l_b + 8i)), l_value)), l_weight));
        l_sum = (l_sum + l_weight);
      }
    }
  }
  return coax_p_scale3(l_out, (0x1.000000p0f / coax_p_fmax(l_sum, 0x1.000000p0f)));
}
fn coax_p_og_yaw(p_v: vec3f, p_yaw: f32) -> vec3f {
  let l_v: vec3f = p_v;
  let l_yaw: f32 = p_yaw;
  var l_c: f32;
  var l_s: f32;
  l_c = cos(l_yaw);
  l_s = sin(l_yaw);
  return coax_p_v3(((l_c * l_v.x) + (l_s * l_v.z)), l_v.y, (((-l_s) * l_v.x) + (l_c * l_v.z)));
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_low: f32 = p_low;
  let l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_fract(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_render_sample_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> vec4f {
  let l_uv: vec2f = p_uv;
  let l_layer: i32 = p_layer;
  let l_dx: vec2f = p_dx;
  let l_dy: vec2f = p_dy;
  let l_bias: f32 = p_bias;
  var l_rs_result: coax_t24;
  l_rs_result = coax_p_render_filter_layer8_grad(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
}
fn coax_p_og_material_coverage(p_uv: vec2f, p_uv1: vec2f, p_word: i32, p_g0: coax_t21, p_g1: coax_t21) -> f32 {
  let l_uv: vec2f = p_uv;
  let l_uv1: vec2f = p_uv1;
  let l_word: i32 = p_word;
  let l_g0: coax_t21 = p_g0;
  let l_g1: coax_t21 = p_g1;
  var l_g: coax_t21;
  var l_m: i32;
  var l_kind: u32;
  var l_a: vec4f;
  var l_mask: vec4f;
  l_g = l_g0;
  l_m = l_word;
  l_kind = vertices[(l_m + 16i)];
  l_a = coax_p_og_sample(vertices[l_m], l_uv, l_g);
  if ((l_kind == 1u))
  {
    l_mask = coax_p_og_sample(vertices[(l_m + 5i)], l_uv1, l_g1);
    return (l_a.w * coax_p_og_raw(l_mask.x));
  }
  if ((l_kind == 5u))
  {
    return coax_p_og_raw(l_a.x);
  }
  if ((((l_kind == 9u) || (l_kind == 11u)) || (l_kind == 12u)))
  {
    return 0x1.000000p0f;
  }
  return l_a.w;
}
fn coax_p_og_material_specular(p_nl: f32, p_nv: f32, p_nh: f32, p_vh: f32, p_rough: f32) -> f32 {
  let l_nl: f32 = p_nl;
  let l_nv: f32 = p_nv;
  let l_nh: f32 = p_nh;
  let l_vh: f32 = p_vh;
  let l_rough: f32 = p_rough;
  var l_r: f32;
  var l_a: f32;
  var l_a2: f32;
  var l_d: f32;
  var l_distribution: f32;
  var l_k: f32;
  var l_geometry: f32;
  var l_f: f32;
  if (((l_nl <= 0.0f) || (l_nv <= 0.0f)))
  {
    return 0.0f;
  }
  l_r = coax_p_clampf(l_rough, 0x1.47ae14p-4f, 0x1.000000p0f);
  l_a = (l_r * l_r);
  l_a2 = (l_a * l_a);
  l_d = (((l_nh * l_nh) * (l_a2 - 0x1.000000p0f)) + 0x1.000000p0f);
  l_distribution = (l_a2 / ((0x1.921fb6p1f * l_d) * l_d));
  l_k = (((l_r + 0x1.000000p0f) * (l_r + 0x1.000000p0f)) / 0x1.000000p3f);
  l_geometry = (((l_nl / ((l_nl * (0x1.000000p0f - l_k)) + l_k)) * l_nv) / ((l_nv * (0x1.000000p0f - l_k)) + l_k));
  l_f = (0x1.47ae14p-5f + (0x1.eb851ep-1f * coax_pow_bg_vs((0x1.000000p0f - coax_p_clampf(l_vh, 0.0f, 0x1.000000p0f)), 0x1.400000p2f)));
  return (((l_distribution * l_geometry) * l_f) / (0x1.000000p2f * coax_p_fmax(l_nv, 0x1.0624dep-10f)));
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
fn coax_p_og_signed10(p_word: u32) -> f32 {
  let l_word: u32 = p_word;
  var l_n: i32;
  l_n = i32((l_word & 1023u));
  return coax_p_clampf((f32(select(l_n, (l_n - 1024i), (l_n >= 512i))) / 0x1.ff0000p8f), (-0x1.000000p0f), 0x1.000000p0f);
}
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  let l_x: u32 = p_x;
  let l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  let l_at: i32 = p_at;
  return coax_p_v4(bones[l_at], bones[(l_at + 1i)], bones[(l_at + 2i)], bones[(l_at + 3i)]);
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  if ((l_b < l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_saturate(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_render_filter_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t24 {
  let l_uv: vec2f = p_uv;
  let l_layer: i32 = p_layer;
  let l_dx: vec2f = p_dx;
  let l_dy: vec2f = p_dy;
  let l_bias: f32 = p_bias;
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
fn coax_p_render_footprint(p_width: i32, p_height: i32, p_du_dx: f32, p_dv_dx: f32, p_du_dy: f32, p_dv_dy: f32, p_bias: f32) -> coax_t25 {
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
fn coax_p_render_footprint_finite(p_x: f32) -> bool {
  let l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_footprint_max(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return (((l_a + l_b) + abs((l_a - l_b))) * 0x1.000000p-1f);
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_12 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_12(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@vertex
fn shadow_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_13 {
  let coax_r = coax_p_shadow_vs(coax_vid, coax_iid);
  return coax_io_13(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_14) -> coax_io_16 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t14(coax_in.x, coax_in.y, coax_in.z));
  return coax_io_16(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn scene_fs(coax_in: coax_io_12) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t12(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8));
  return coax_r;
}
@fragment
fn shadow_fs(coax_in: coax_io_13) {
  coax_p_shadow_fs(coax_t13(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
}
@fragment
fn fx_fs(coax_in: coax_io_16) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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