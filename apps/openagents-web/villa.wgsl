// ---- generated: apps/openagents-web/gpu/villa.coax (the module) by coax wgsl; do not edit
struct coax_t10 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t13 {
  x: vec4f,
  y: vec3f,
  z: vec3f,
  w: vec3f,
  f4: u32,
  f5: vec3f,
  f6: vec2f,
}
struct coax_io_13 {
  @builtin(position) x: vec4f,
  @location(0) y: vec3f,
  @location(1) z: vec3f,
  @location(2) w: vec3f,
  @location(3) @interpolate(flat) f4: u32,
  @location(4) f5: vec3f,
  @location(5) @interpolate(flat) f6: vec2f,
}
struct coax_t14 {
  x: vec3f,
  y: vec3f,
}
struct coax_t15 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_15 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
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
struct coax_t17 {
  x: vec4f,
  y: vec4f,
}
struct coax_out_17 {
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
@group(0) @binding(0) var<uniform> u: array<vec4f, 124>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> heights: array<f32>;
@group(1) @binding(1) var<storage, read> objects: array<coax_t10>;
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
@group(1) @binding(5) var post_lin: sampler;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t13 {
  var l_id: u32 = p_id;
  var l_instance: u32 = p_instance;
  var l_o: coax_t13;
  var l_n: u32;
  var l_c: vec2f;
  var l_gx: i32;
  var l_gz: i32;
  var l_w: vec3f;
  var l_m: coax_t10;
  var l_kind: u32;
  var l_shape_look: u32;
  var l_nudge: f32;
  var l_seed: f32;
  var l_v: coax_t14;
  var l__sw3: u32;
  var l_p: vec4f;
  var l_w_19: vec3f;
  var l_c0: vec3f;
  var l_c1: vec3f;
  var l_c2: vec3f;
  var l_normal: vec3f;
  l_o = coax_t13();
  l_n = coax_p_wg_u32_f(coax_at_u(i32(168i)));
  if ((l_instance < (l_n * l_n)))
  {
    l_c = coax_p_villa_quad((l_id % 6u));
    l_gx = (i32((l_instance % l_n)) + coax_p_wg_i32_f(l_c.x));
    l_gz = (i32((l_instance / l_n)) + coax_p_wg_i32_f(l_c.y));
    l_w = coax_p_v3((coax_at_u(i32(170i)) + (f32(l_gx) * coax_at_u(i32(169i)))), coax_p_villa_height(l_gx, l_gz), (coax_at_u(i32(171i)) + (f32(l_gz) * coax_at_u(i32(169i)))));
    l_o.x = coax_p_villa_clip(l_w);
    l_o.y = l_w;
    l_o.z = coax_p_wg_normalize3(coax_p_v3((coax_p_villa_height((l_gx - 1i), l_gz) - coax_p_villa_height((l_gx + 1i), l_gz)), (0x1.000000p1f * coax_at_u(i32(169i))), (coax_p_villa_height(l_gx, (l_gz - 1i)) - coax_p_villa_height(l_gx, (l_gz + 1i)))));
    l_o.w = coax_p_v3(0x1.99999ap-4f, 0x1.47ae14p-3f, 0x1.eb851ep-4f);
    l_o.f4 = 99u;
    l_o.f5 = coax_p_v3(0.0f, 0.0f, 0.0f);
    l_o.f6 = coax_p_v2(0.0f, 0.0f);
    return l_o;
  }
  l_m = objects[i32((l_instance - (l_n * l_n)))];
  l_kind = (coax_p_wg_u32_f(l_m.w.w) % 8u);
  l_shape_look = (coax_p_wg_u32_f(l_m.w.w) / 8u);
  l_nudge = select(0.0f, 0x1.47ae14p-3f, ((((l_shape_look == 1u) || (l_shape_look == 2u)) || (l_shape_look == 9u)) || (l_shape_look == 12u)));
  l_seed = coax_p_fract((((l_m.x.w * 0x1.189374p-3f) + (l_m.z.w * 0x1.22d0e6p-4f)) + (l_m.y.w * 0x1.28f5c2p-2f)));
  l_v = coax_t14();
  {
    l__sw3 = l_kind;
    if ((l__sw3 == 1u))
    {
      l_v = coax_p_villa_box_vertex(l_id);
    }
    else
    {
      if ((l__sw3 == 2u))
      {
        l_v = coax_p_villa_globe_vertex(l_id, 8u, 5u, l_nudge, l_seed);
      }
      else
      {
        if ((l__sw3 == 3u))
        {
          l_v = coax_p_villa_cylinder_vertex(l_id);
        }
        else
        {
          if ((l__sw3 == 5u))
          {
            l_v = coax_p_villa_globe_vertex(l_id, 8u, 5u, l_nudge, l_seed);
          }
          else
          {
            l_v = coax_p_villa_cone_vertex(l_id);
          }
        }
      }
    }
  }
  l_p = coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f);
  l_w_19 = coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p));
  l_c0 = coax_p_v3(l_m.x.x, l_m.y.x, l_m.z.x);
  l_c1 = coax_p_v3(l_m.x.y, l_m.y.y, l_m.z.y);
  l_c2 = coax_p_v3(l_m.x.z, l_m.y.z, l_m.z.z);
  l_normal = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c1, l_c2), l_v.y.x), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c2, l_c0), l_v.y.y)), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c0, l_c1), l_v.y.z));
  l_o.x = coax_p_villa_clip(l_w_19);
  l_o.y = l_w_19;
  l_o.z = coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_normal, coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  l_o.w = coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z);
  l_o.f4 = (coax_p_wg_u32_f(l_m.w.w) / 8u);
  l_o.f5 = l_v.x;
  l_o.f6 = coax_p_v2(coax_p_wg_length3(l_c1), coax_p_smoothstep(0x1.99999ap0f, 0x1.99999ap1f, coax_p_wg_length3(l_c1)));
  return l_o;
}
fn coax_p_fx_vs(p__vid4: u32, p__iid5: u32, p_in_: coax_t15) -> coax_t16 {
  var l__vid4: u32 = p__vid4;
  var l__iid5: u32 = p__iid5;
  var l_in_: coax_t15 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t16;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t16();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t17 {
  var l_pos: vec4f = p_pos;
  var l_d: vec3f;
  var l_space: f32;
  var l_sky: vec3f;
  var l_deep: vec3f;
  var l_mo: vec4f;
  var l_c: vec3f;
  l_d = coax_p_villa_ray(coax_p_v2(l_pos.x, l_pos.y));
  l_space = coax_at_u(i32(27i));
  l_sky = coax_p_wg_mix_f3f3f(coax_p_villa_pal(0.0f), coax_p_villa_pal(0x1.000000p0f), coax_p_verse_style_flat_sky(l_d.y));
  l_deep = coax_p_wg_add_f3f3(coax_p_v3(0x1.0624dep-8f, 0x1.89374cp-9f, 0x1.89374cp-7f), coax_p_wg_mul_f3f(coax_p_v3(0x1.47ae14p-6f, 0x1.89374cp-7f, 0x1.47ae14p-5f), coax_pow_bg_vs((0x1.000000p0f - abs(coax_p_wg_dot3(l_d, coax_p_wg_normalize3(coax_p_v3(0x1.333334p-2f, 0x1.99999ap-3f, 0x1.000000p0f))))), 0x1.800000p2f)));
  l_sky = coax_p_wg_mix_f3f3f(l_sky, coax_p_wg_add_f3f3(l_deep, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.b33334p-1f, 0x1.ccccccp-1f, 0x1.000000p0f), coax_p_villa_stars(l_d, l_space)), 0x1.333334p0f)), l_space);
  l_mo = coax_p_villa_moon(l_d, l_sky);
  l_c = coax_p_wg_add_f3f3(coax_p_v3(l_mo.x, l_mo.y, l_mo.z), coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(28i)), coax_at_u(i32(28i))));
  return coax_t17(coax_p_v4(l_c.x, l_c.y, l_c.z, (select(0.0f, 0x1.333334p-2f, (l_mo.w > 0x1.99999ap-3f)) + coax_at_u(i32(28i)))), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_scene_fs(p_s: coax_t13) -> coax_t17 {
  var l_s: coax_t13 = p_s;
  var l_dx: vec3f;
  var l_dy: vec3f;
  var l_ground_aa: f32;
  var l_local_aa: f32;
  var l_to_eye: vec3f;
  var l_facet: vec3f;
  var l_smooth_n: vec3f;
  var l_d: f32;
  var l_fade: f32;
  var l_facing: f32;
  var l_drift: f32;
  var l_body: f32;
  var l_cell: vec2f;
  var l_core: vec3f;
  var l_c: vec3f;
  var l_n: vec3f;
  var l_lit: f32;
  var l_land: f32;
  var l_base: vec3f;
  var l_c_21: vec3f;
  var l_n_22: vec3f;
  var l_xz: vec2f;
  var l_base_24: vec3f;
  var l_cover: f32;
  var l_edge: f32;
  var l_pd: f32;
  var l_half: f32;
  var l_aa: f32;
  var l_from_top: f32;
  var l_band: f32;
  var l_c_32: vec3f;
  var l_reads: bool;
  var l_share: f32;
  var l_rim: f32;
  l_dx = coax_p_wg_dpdx_f3(l_s.y);
  l_dy = coax_p_wg_dpdy_f3(l_s.y);
  l_ground_aa = coax_p_fmax(coax_p_wg_length2(coax_p_wg_fwidth_f2(coax_p_v2(l_s.y.x, l_s.y.z))), 0x1.0624dep-9f);
  l_local_aa = fwidth(l_s.f5.y);
  l_to_eye = coax_p_wg_normalize3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y));
  l_facet = coax_p_wg_cross3(l_dy, l_dx);
  l_facet = coax_p_wg_normalize3(select(l_facet, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (coax_p_wg_dot3(l_facet, l_facet) < 0x1.6849b8p-47f)));
  if ((coax_p_wg_dot3(l_facet, l_to_eye) < 0.0f))
  {
    l_facet = coax_p_wg_neg_f3(l_facet);
  }
  l_smooth_n = coax_p_wg_normalize3(l_s.z);
  l_d = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_s.y));
  l_fade = coax_p_verse_style_flat_fade(l_d, 0x1.400000p5f, 0x1.400000p6f, 0x1.000000p-1f);
  if ((l_s.f4 == 6u))
  {
    l_facing = abs(coax_p_wg_dot3(l_smooth_n, l_to_eye));
    l_drift = (floor((coax_at_u(i32(2i)) * 0x1.800000p3f)) * (0x1.000000p0f - coax_at_u(i32(4i))));
    l_body = coax_p_smoothstep(0.0f, 0x1.99999ap-1f, l_facing);
    l_cell = coax_p_wg_add_f2f2(coax_p_wg_floor_f2(coax_p_wg_div_f2f(coax_p_v2(l_s.x.x, l_s.x.y), coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f))), coax_p_v2(l_drift, (l_drift * 0x1.000000p1f)));
    if ((coax_p_verse_style_bayer(l_cell.x, l_cell.y) > (l_body * 0x1.5c28f6p-1f)))
    {
      discard;
    }
    l_core = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.000000p-1f, 0x1.19999ap-1f, 0x1.d70a3ep-1f), coax_p_v3(0x1.99999ap-1f, 0x1.a3d70ap-1f, 0x1.000000p0f), l_body);
    return coax_t17(coax_p_wg_make_f4_f3f(coax_p_wg_mix_f3f3f(l_core, coax_p_villa_pal(0x1.000000p0f), (l_fade * 0x1.000000p-1f)), 0x1.c28f5cp-3f), coax_p_v4(l_smooth_n.x, l_smooth_n.y, l_smooth_n.z, (-l_d)));
  }
  if (((l_s.f4 == 5u) || (l_s.f4 == 7u)))
  {
    l_c = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_villa_pal(0x1.400000p3f), l_s.w, select(0x1.99999ap-1f, 0x1.000000p-2f, (l_s.f4 == 7u))), select(0x1.266666p0f, 0x1.99999ap0f, (l_s.f4 == 7u)));
    return coax_t17(coax_p_wg_make_f4_f3f(coax_p_wg_mix_f3f3f(l_c, coax_p_villa_pal(0x1.000000p0f), (l_fade * 0x1.666666p-2f)), select(0x1.666666p-2f, 0x1.ccccccp-1f, (l_s.f4 == 7u))), coax_p_v4(l_smooth_n.x, l_smooth_n.y, l_smooth_n.z, (-l_d)));
  }
  if ((l_s.f4 == 8u))
  {
    l_n = l_smooth_n;
    l_lit = coax_p_smoothstep((-0x1.47ae14p-6f), 0x1.47ae14p-6f, coax_p_wg_dot3(l_n, coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(160i)), coax_at_u(i32(161i)), coax_at_u(i32(162i))))));
    l_land = coax_p_stepf(0x1.0a3d70p-1f, coax_p_villa_noise(coax_p_v2((coax_p_wg_atan2(l_n.z, l_n.x) * 0x1.800000p1f), (l_n.y * 0x1.000000p2f))));
    l_base = coax_p_wg_mix_f3f3f(coax_p_villa_pal(0x1.200000p4f), coax_p_villa_pal(0x1.800000p1f), l_land);
    l_c_21 = coax_p_wg_mul_f3f(l_base, coax_p_mix(0x1.000000p-2f, 0x1.000000p0f, l_lit));
    return coax_t17(coax_p_v4(l_c_21.x, l_c_21.y, l_c_21.z, 0.0f), coax_p_v4(l_n.x, l_n.y, l_n.z, (-l_d)));
  }
  l_n_22 = l_facet;
  l_xz = coax_p_v2(l_s.y.x, l_s.y.z);
  l_base_24 = coax_p_villa_albedo(l_s.f4, l_s.w);
  l_cover = 0.0f;
  l_edge = 0x1.000000p0f;
  if ((l_s.f4 == 99u))
  {
    l_base_24 = coax_p_wg_mix_f3f3f(coax_p_villa_pal(0x1.000000p1f), coax_p_villa_pal(0x1.800000p1f), coax_p_stepf(0x1.333334p-1f, coax_p_villa_noise(coax_p_wg_mul_f2f(l_xz, 0x1.70a3d8p-5f))));
    l_pd = coax_p_villa_path_distance(l_xz);
    l_half = coax_at_u(i32(493i));
    l_aa = l_ground_aa;
    l_base_24 = coax_p_wg_mix_f3f3f(l_base_24, coax_p_villa_pal(0x1.000000p2f), (0x1.000000p0f - coax_p_smoothstep((l_half - l_aa), (l_half + l_aa), l_pd)));
    l_base_24 = coax_p_wg_mix_f3f3f(l_base_24, coax_p_villa_pal(0x1.400000p3f), (coax_p_verse_style_flat_hem(abs((l_pd - l_half)), 0x1.1eb852p-5f, l_aa) * 0x1.666666p-1f));
    l_cover = coax_p_villa_shade_shadows(l_xz);
    l_edge = 0.0f;
  }
  else
  {
    if ((l_s.f4 == 4u))
    {
      l_edge = 0.0f;
    }
    else
    {
      if ((l_s.f4 == 10u))
      {
        l_n_22 = l_smooth_n;
      }
    }
  }
  if ((l_s.f4 == 3u))
  {
    l_from_top = ((0x1.000000p-1f - l_s.f5.y) * l_s.f6.x);
    l_base_24 = coax_p_wg_mix_f3f3f(l_base_24, coax_p_villa_pal(0x1.000000p3f), coax_p_verse_style_flat_hem(l_from_top, 0x1.eb851ep-6f, coax_p_fmax((l_local_aa * l_s.f6.x), 0x1.0624dep-10f)));
  }
  l_band = coax_p_verse_style_facet_tone(coax_p_wg_dot3(l_n_22, coax_p_villa_key_dir()), 0x1.000000p0f);
  l_c_32 = coax_p_wg_mul_f3f3(l_base_24, coax_p_wg_mix_f3f3f(coax_p_villa_pal(0x1.800000p2f), coax_p_villa_pal(0x1.400000p2f), l_band));
  l_c_32 = coax_p_wg_mul_f3f3(l_c_32, coax_p_wg_mix_f3f3f(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), coax_p_villa_pal(0x1.c00000p2f), l_cover));
  l_c_32 = coax_p_wg_mix_f3f3f(l_c_32, coax_p_villa_pal(0x1.000000p0f), l_fade);
  l_reads = (((coax_p_fmax(l_s.f6.x, 0.0f) / coax_p_fmax(l_d, 0x1.99999ap-4f)) > 0x1.1eb852p-5f) || (l_s.f4 == 99u));
  l_share = 0.0f;
  if ((l_s.f4 == 13u))
  {
    l_edge = 0.0f;
    l_rim = coax_pow_bg_vs((0x1.000000p0f - abs(coax_p_wg_dot3(l_smooth_n, l_to_eye))), 0x1.400000p1f);
    l_c_32 = coax_p_wg_mix_f3f3f(l_c_32, coax_p_v3(0x1.d70a3ep-1f, 0x1.e147aep-1f, 0x1.000000p0f), (l_rim * 0x1.19999ap-1f));
    l_share = (0x1.47ae14p-4f + (0x1.333334p-2f * l_rim));
  }
  return coax_t17(coax_p_v4(l_c_32.x, l_c_32.y, l_c_32.z, l_share), coax_p_v4(l_n_22.x, l_n_22.y, l_n_22.z, select((-l_d), l_d, ((l_edge > 0x1.000000p-1f) && l_reads))));
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
  var l_p: coax_t20;
  var l_uv: vec2f;
  var l_light: vec3f;
  var l_near: vec3f;
  var l_far: vec3f;
  var l_c: vec3f;
  var l_own: f32;
  var l_hi: coax_t20;
  var l_e: f32;
  l_p = coax_p_wg_cvt_i2f2(coax_p_v2(l_pos.x, l_pos.y));
  l_uv = coax_p_wg_div_f2f2(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_at2(0i));
  l_light = coax_p_wg_sw_rgb_f4(coax_p_wg_load_2d(l_p, 0i));
  l_near = coax_p_wg_sw_rgb_f4(textureSampleLevel(post_quarter, post_lin, l_uv, 0.0f));
  l_far = coax_p_wg_sw_rgb_f4(textureSampleLevel(post_eighth, post_lin, l_uv, 0.0f));
  l_c = coax_p_wg_add_f3f3(l_light, coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_near, 0x1.19999ap-1f), coax_p_wg_mul_f3f(l_far, 0x1.ccccccp-2f)), 0x1.333334p-1f));
  l_own = (coax_p_wg_load_2d_1(l_p, 0i)).w;
  if ((((l_own > 0.0f) && (l_own < 0x1.180000p7f)) && (coax_at_u(i32(27i)) < 0x1.000000p-1f)))
  {
    l_hi = coax_p_wg_sub_i2i2(coax_p_wg_cvt_i2f2(coax_p_wg_at2(0i)), coax_p_i2(1i, 1i));
    l_e = coax_p_verse_style_facet_edge(l_own, (coax_p_wg_load_2d_1(coax_p_wg_clamp_i2i2i2(coax_p_wg_add_i2i2(l_p, coax_p_i2(1i, 0i)), coax_p_i2(0i, 0i), l_hi), 0i)).w);
    l_e = coax_p_fmax(l_e, coax_p_verse_style_facet_edge(l_own, (coax_p_wg_load_2d_1(coax_p_wg_clamp_i2i2i2(coax_p_wg_sub_i2i2(l_p, coax_p_i2(1i, 0i)), coax_p_i2(0i, 0i), l_hi), 0i)).w));
    l_e = coax_p_fmax(l_e, coax_p_verse_style_facet_edge(l_own, (coax_p_wg_load_2d_1(coax_p_wg_clamp_i2i2i2(coax_p_wg_add_i2i2(l_p, coax_p_i2(0i, 1i)), coax_p_i2(0i, 0i), l_hi), 0i)).w));
    l_e = coax_p_fmax(l_e, coax_p_verse_style_facet_edge(l_own, (coax_p_wg_load_2d_1(coax_p_wg_clamp_i2i2i2(coax_p_wg_sub_i2i2(l_p, coax_p_i2(0i, 1i)), coax_p_i2(0i, 0i), l_hi), 0i)).w));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_villa_pal(0x1.000000p3f), ((l_e * 0x1.99999ap-1f) * (0x1.000000p0f - coax_p_smoothstep(0x1.e00000p5f, 0x1.180000p7f, l_own))));
  }
  return coax_p_wg_make_f4_f3f(coax_p_villa_plates(coax_p_v2(l_pos.x, l_pos.y), coax_p_villa_encode(l_c)), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t16) -> vec4f {
  var l_v: coax_t16 = p_v;
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
fn coax_p_villa_quad(p_v: u32) -> vec2f {
  var l_v: u32 = p_v;
  var l_q: array<vec2f, 6>;
  l_q = array<vec2f, 6>(coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0.0f), coax_p_v2(0x1.000000p0f, 0x1.000000p0f), coax_p_v2(0.0f, 0x1.000000p0f));
  return l_q[i32(l_v)];
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
fn coax_p_villa_height(p_x: i32, p_z: i32) -> f32 {
  var l_x: i32 = p_x;
  var l_z: i32 = p_z;
  var l_n: i32;
  var l_cx: i32;
  var l_cz: i32;
  l_n = coax_p_wg_i32_f(coax_at_u(i32(168i)));
  l_cx = coax_p_wg_clamp_i(l_x, 0i, l_n);
  l_cz = coax_p_wg_clamp_i(l_z, 0i, l_n);
  return heights[i32(u32(((l_cz * (l_n + 1i)) + l_cx)))];
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_villa_clip(p_w: vec3f) -> vec4f {
  var l_w: vec3f = p_w;
  var l_p: vec4f;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  return coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_p), coax_p_wg_dot4(coax_p_wg_at4(12i), l_p), coax_p_wg_dot4(coax_p_wg_at4(16i), l_p), coax_p_wg_dot4(coax_p_wg_at4(20i), l_p));
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_villa_box_vertex(p_id: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_f: u32;
  var l_c: vec2f;
  var l_n: vec3f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l__sw2: u32;
  l_f = (l_id / 6u);
  l_c = coax_p_wg_sub_f2f2(coax_p_villa_quad((l_id % 6u)), coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f));
  l_n = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_a = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_b = coax_p_v3(0.0f, 0.0f, 0.0f);
  {
    l__sw2 = l_f;
    if ((l__sw2 == 0u))
    {
      l_n = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
      l_a = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
      l_b = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
    }
    else
    {
      if ((l__sw2 == 1u))
      {
        l_n = coax_p_v3((-0x1.000000p0f), 0.0f, 0.0f);
        l_a = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
        l_b = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
      }
      else
      {
        if ((l__sw2 == 2u))
        {
          l_n = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
          l_a = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
          l_b = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
        }
        else
        {
          if ((l__sw2 == 3u))
          {
            l_n = coax_p_v3(0.0f, (-0x1.000000p0f), 0.0f);
            l_a = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
            l_b = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
          }
          else
          {
            if ((l__sw2 == 4u))
            {
              l_n = coax_p_v3(0.0f, 0.0f, 0x1.000000p0f);
              l_a = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
              l_b = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
            }
            else
            {
              l_n = coax_p_v3(0.0f, 0.0f, (-0x1.000000p0f));
              l_a = coax_p_v3(0x1.000000p0f, 0.0f, 0.0f);
              l_b = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
            }
          }
        }
      }
    }
  }
  return coax_t14(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_n, 0x1.000000p-1f), coax_p_wg_mul_f3f(l_a, l_c.x)), coax_p_wg_mul_f3f(l_b, l_c.y)), l_n);
}
fn coax_p_villa_globe_vertex(p_id: u32, p_lons: u32, p_lats: u32, p_nudge: f32, p_seed: f32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_lons: u32 = p_lons;
  var l_lats: u32 = p_lats;
  var l_nudge: f32 = p_nudge;
  var l_seed: f32 = p_seed;
  var l_q: u32;
  var l_c: vec2f;
  var l_i: f32;
  var l_j: f32;
  var l_lon: f32;
  var l_lat: f32;
  var l_n: vec3f;
  l_q = (l_id / 6u);
  l_c = coax_p_villa_quad((l_id % 6u));
  if ((((l_q % l_lons) >= 8u) || ((l_q / l_lons) >= l_lats)))
  {
    return coax_p_villa_nothing();
  }
  l_i = (f32((l_q % l_lons)) + l_c.x);
  l_j = (f32((l_q / l_lons)) + l_c.y);
  l_lon = ((l_i / 0x1.000000p3f) * 0x1.921fb6p2f);
  l_lat = ((l_j / 0x1.400000p2f) * 0x1.921fb6p1f);
  l_n = coax_p_v3((sin(l_lat) * cos(l_lon)), cos(l_lat), (sin(l_lat) * sin(l_lon)));
  return coax_t14(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_n, 0x1.000000p-1f), (0x1.000000p0f + (l_nudge * coax_p_verse_style_facet_jitter(l_i, l_j, 0x1.000000p3f, 0x1.400000p2f, l_seed)))), l_n);
}
fn coax_p_villa_cylinder_vertex(p_id: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_c: vec2f;
  var l_a: f32;
  var l_n: vec3f;
  var l_t: u32;
  var l_k: u32;
  var l_top: bool;
  var l_y: f32;
  var l_n_8: vec3f;
  var l_a_9: f32;
  if ((l_id < 48u))
  {
    l_c = coax_p_villa_quad((l_id % 6u));
    l_a = (((f32((l_id / 6u)) + l_c.x) / 0x1.000000p3f) * 0x1.921fb6p2f);
    l_n = coax_p_v3(cos(l_a), 0.0f, sin(l_a));
    return coax_t14(coax_p_v3((l_n.x * 0x1.000000p-1f), (l_c.y - 0x1.000000p-1f), (l_n.z * 0x1.000000p-1f)), l_n);
  }
  l_t = ((l_id - 48u) / 3u);
  l_k = ((l_id - 48u) % 3u);
  l_top = (l_t < 8u);
  l_y = select((-0x1.000000p-1f), 0x1.000000p-1f, l_top);
  l_n_8 = coax_p_v3(0.0f, select((-0x1.000000p0f), 0x1.000000p0f, l_top), 0.0f);
  if ((l_k == 0u))
  {
    return coax_t14(coax_p_v3(0.0f, l_y, 0.0f), l_n_8);
  }
  l_a_9 = (((f32((l_t % 8u)) + f32((l_k - 1u))) / 0x1.000000p3f) * 0x1.921fb6p2f);
  return coax_t14(coax_p_v3((cos(l_a_9) * 0x1.000000p-1f), l_y, (sin(l_a_9) * 0x1.000000p-1f)), l_n_8);
}
fn coax_p_villa_cone_vertex(p_id: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_t: u32;
  var l_k: u32;
  var l_mid: f32;
  var l_a: f32;
  var l_n: vec3f;
  var l_a_6: f32;
  l_t = ((l_id % 24u) / 3u);
  l_k = (l_id % 3u);
  if ((l_id < 24u))
  {
    l_mid = (((f32(l_t) + 0x1.000000p-1f) / 0x1.000000p3f) * 0x1.921fb6p2f);
    if ((l_k == 0u))
    {
      return coax_t14(coax_p_v3(0.0f, 0x1.000000p-1f, 0.0f), coax_p_wg_normalize3(coax_p_v3(cos(l_mid), 0x1.000000p-1f, sin(l_mid))));
    }
    l_a = (((f32(l_t) + f32((l_k - 1u))) / 0x1.000000p3f) * 0x1.921fb6p2f);
    return coax_t14(coax_p_v3((cos(l_a) * 0x1.000000p-1f), (-0x1.000000p-1f), (sin(l_a) * 0x1.000000p-1f)), coax_p_wg_normalize3(coax_p_v3(cos(l_a), 0x1.000000p-1f, sin(l_a))));
  }
  l_n = coax_p_v3(0.0f, (-0x1.000000p0f), 0.0f);
  if ((l_k == 0u))
  {
    return coax_t14(coax_p_v3(0.0f, (-0x1.000000p-1f), 0.0f), l_n);
  }
  l_a_6 = (((f32(l_t) + f32((l_k - 1u))) / 0x1.000000p3f) * 0x1.921fb6p2f);
  return coax_t14(coax_p_v3((cos(l_a_6) * 0x1.000000p-1f), (-0x1.000000p-1f), (sin(l_a_6) * 0x1.000000p-1f)), l_n);
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_cross3(l_a, l_b);
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
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
fn coax_p_villa_ray(p_pos: vec2f) -> vec3f {
  var l_pos: vec2f = p_pos;
  var l_nx: f32;
  var l_ny: f32;
  var l_fx2: f32;
  var l_fy2: f32;
  l_nx = (((l_pos.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f);
  l_ny = (0x1.000000p0f - ((l_pos.y / coax_at_u(i32(1i))) * 0x1.000000p1f));
  l_fx2 = coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))), coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_fy2 = coax_p_wg_dot3(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))), coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))));
  return coax_p_wg_normalize3(coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i))), coax_p_wg_div_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))), l_nx), l_fx2)), coax_p_wg_div_f3f(coax_p_wg_mul_f3f(coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))), l_ny), l_fy2)));
}
fn coax_p_villa_pal(p_slot: f32) -> vec3f {
  var l_slot: f32 = p_slot;
  return coax_p_v3(coax_p_verse_style_flat_colour(0.0f, l_slot, 0.0f), coax_p_verse_style_flat_colour(0.0f, l_slot, 0x1.000000p0f), coax_p_verse_style_flat_colour(0.0f, l_slot, 0x1.000000p1f));
}
fn coax_p_verse_style_flat_sky(p_fl_up: f32) -> f32 {
  var l_fl_up: f32 = p_fl_up;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.eb851ep-4f, 0x1.eb851ep-4f, l_fl_up));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_villa_stars(p_d: vec3f, p_many: f32) -> f32 {
  var l_d: vec3f = p_d;
  var l_many: f32 = p_many;
  var l_c: f32;
  var l_layer: i32;
  var l_scale: f32;
  var l_q: vec3f;
  var l_cell: vec3f;
  var l_h: f32;
  var l_keep: f32;
  var l_at: vec3f;
  var l_r: f32;
  var l_twinkle: f32;
  l_c = 0.0f;
  {
    l_layer = 0i;
    while ((l_layer < 2i))
    {
      l_scale = select(0x1.680000p6f, 0x1.b80000p7f, (l_layer == 1i));
      l_q = coax_p_wg_mul_f3f(l_d, l_scale);
      l_cell = coax_p_wg_floor_f3(l_q);
      l_h = coax_p_villa_hash3(l_cell);
      l_keep = select(0x1.f851ecp-1f, (0x1.f0a3d8p-1f - (0x1.99999ap-5f * l_many)), (l_layer == 1i));
      if ((l_h > l_keep))
      {
        l_at = coax_p_wg_add_f3f3(l_cell, coax_p_v3(coax_p_villa_hash3(coax_p_wg_add_f3f(l_cell, 0x1.4cccccp0f)), coax_p_villa_hash3(coax_p_wg_add_f3f(l_cell, 0x1.59999ap1f)), coax_p_villa_hash3(coax_p_wg_add_f3f(l_cell, 0x1.066666p2f))));
        l_r = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_q, l_at));
        l_twinkle = (0x1.800000p-1f + ((0x1.000000p-2f * sin(((coax_at_u(i32(2i)) * (0x1.000000p0f + (0x1.800000p1f * l_h))) + (l_h * 0x1.400000p5f)))) * (0x1.000000p0f - coax_at_u(i32(4i)))));
        l_c = (l_c + (((((0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.666666p-2f, l_r)) * (l_h - l_keep)) / (0x1.000000p0f - l_keep)) * l_twinkle) * select(0x1.000000p0f, 0x1.19999ap-1f, (l_layer == 1i))));
      }
      l_layer = (l_layer + 1i);
    }
  }
  return l_c;
}
fn coax_p_villa_moon(p_d: vec3f, p_sky: vec3f) -> vec4f {
  var l_d: vec3f = p_d;
  var l_sky: vec3f = p_sky;
  var l_m: vec3f;
  var l_size: f32;
  var l_c: f32;
  var l_side: vec3f;
  var l_up: vec3f;
  var l_x: f32;
  var l_y: f32;
  var l_r: f32;
  var l_z: f32;
  var l_a: f32;
  var l_toward_sun: vec3f;
  var l_lit: f32;
  var l_disc: vec3f;
  var l_edge: f32;
  l_m = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(160i)), coax_at_u(i32(161i)), coax_at_u(i32(162i))));
  l_size = 0x1.70a3d8p-5f;
  l_c = coax_p_wg_dot3(l_d, l_m);
  l_side = coax_p_wg_normalize3(coax_p_wg_cross3(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), l_m));
  l_up = coax_p_wg_cross3(l_m, l_side);
  l_x = (coax_p_wg_dot3(l_d, l_side) / l_size);
  l_y = (coax_p_wg_dot3(l_d, l_up) / l_size);
  l_r = coax_p_wg_length2(coax_p_v2(l_x, l_y));
  if (((l_r >= 0x1.000000p0f) || (l_c < 0.0f)))
  {
    return coax_p_v4(l_sky.x, l_sky.y, l_sky.z, 0.0f);
  }
  l_z = sqrt(coax_p_fmax(0.0f, (0x1.000000p0f - (l_r * l_r))));
  l_a = (coax_at_u(i32(163i)) * 0x1.921fb6p2f);
  l_toward_sun = coax_p_v3((-sin(l_a)), 0.0f, (-cos(l_a)));
  l_lit = coax_p_smoothstep((-0x1.47ae14p-6f), 0x1.47ae14p-6f, coax_p_wg_dot3(coax_p_v3(l_x, l_y, l_z), l_toward_sun));
  l_disc = coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_sky, coax_p_villa_pal(0x1.200000p4f), 0x1.000000p-1f), coax_p_villa_pal(0x1.400000p3f), l_lit);
  l_edge = (0x1.000000p0f - coax_p_smoothstep(0x1.f0a3d8p-1f, 0x1.000000p0f, l_r));
  return coax_p_wg_make_f4_f3f(coax_p_wg_mix_f3f3f(l_sky, l_disc, l_edge), (l_edge * l_lit));
}
fn coax_p_wg_dpdx_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(dpdx(l_p0.x), dpdx(l_p0.y), dpdx(l_p0.z));
}
fn coax_p_wg_dpdy_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(dpdy(l_p0.x), dpdy(l_p0.y), dpdy(l_p0.z));
}
fn coax_p_wg_fwidth_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(fwidth(l_p0.x), fwidth(l_p0.y));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
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
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_neg_f3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_v3((-l_a.x), (-l_a.y), (-l_a.z));
}
fn coax_p_verse_style_flat_fade(p_fl_d: f32, p_fl_start: f32, p_fl_reach: f32, p_fl_cap: f32) -> f32 {
  var l_fl_d: f32 = p_fl_d;
  var l_fl_start: f32 = p_fl_start;
  var l_fl_reach: f32 = p_fl_reach;
  var l_fl_cap: f32 = p_fl_cap;
  return (l_fl_cap * coax_p_verse_style_soft_step((l_fl_start + (l_fl_reach * 0x1.000000p-1f)), coax_p_st_max((l_fl_reach * 0x1.000000p-1f), 0x1.0624dep-10f), l_fl_d));
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_verse_style_bayer(p_fl_px: f32, p_fl_py: f32) -> f32 {
  var l_fl_px: f32 = p_fl_px;
  var l_fl_py: f32 = p_fl_py;
  var l_fl_x: f32;
  var l_fl_y: f32;
  var l_fl_x0: f32;
  var l_fl_y0: f32;
  var l_fl_x1: f32;
  var l_fl_y1: f32;
  l_fl_x = (l_fl_px - (0x1.000000p2f * floor((l_fl_px * 0x1.000000p-2f))));
  l_fl_y = (l_fl_py - (0x1.000000p2f * floor((l_fl_py * 0x1.000000p-2f))));
  l_fl_x0 = (l_fl_x - (0x1.000000p1f * floor((l_fl_x * 0x1.000000p-1f))));
  l_fl_y0 = (l_fl_y - (0x1.000000p1f * floor((l_fl_y * 0x1.000000p-1f))));
  l_fl_x1 = floor((l_fl_x * 0x1.000000p-1f));
  l_fl_y1 = floor((l_fl_y * 0x1.000000p-1f));
  return ((((0x1.000000p2f * coax_p_st_bayer2(l_fl_x0, l_fl_y0)) + coax_p_st_bayer2(l_fl_x1, l_fl_y1)) + 0x1.000000p-1f) / 0x1.000000p4f);
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
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
fn coax_p_villa_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_villa_hash(l_i), coax_p_villa_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_villa_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_villa_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
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
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_villa_albedo(p_look: u32, p_tint: vec3f) -> vec3f {
  var l_look: u32 = p_look;
  var l_tint: vec3f = p_tint;
  var l_hi: f32;
  l_hi = coax_p_fmax(l_tint.x, coax_p_fmax(l_tint.y, l_tint.z));
  if ((l_look == 1u))
  {
    return coax_p_wg_mix_f3f3f(coax_p_villa_pal(0x1.600000p3f), coax_p_villa_pal(0x1.800000p3f), coax_p_smoothstep(0x1.5c28f6p-3f, 0x1.c28f5cp-3f, l_hi));
  }
  if ((l_look == 2u))
  {
    if ((l_hi < 0x1.333334p-2f))
    {
      return coax_p_villa_pal(0x1.200000p4f);
    }
    return coax_p_wg_mix_f3f3f(coax_p_villa_pal(0x1.100000p4f), coax_p_villa_pal(0x1.400000p3f), coax_p_smoothstep(0x1.851eb8p-2f, 0x1.000000p-1f, l_hi));
  }
  if ((l_look == 3u))
  {
    return coax_p_villa_pal(0x1.200000p3f);
  }
  if ((l_look == 4u))
  {
    return coax_p_villa_pal(0x1.a00000p3f);
  }
  if ((l_look == 9u))
  {
    return coax_p_villa_pal(0x1.800000p3f);
  }
  if ((l_look == 10u))
  {
    return coax_p_villa_pal(0x1.200000p3f);
  }
  if ((((l_look == 11u) || (l_look == 12u)) || (l_look == 13u)))
  {
    return coax_p_wg_pow_f3f3(coax_p_wg_clamp_f3f3f3(l_tint, coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), coax_p_v3(0x1.19999ap1f, 0x1.19999ap1f, 0x1.19999ap1f));
  }
  return coax_p_villa_pal(0x1.c00000p3f);
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_villa_path_distance(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_d: f32;
  var l_count: i32;
  var l_i: i32;
  var l_seg: vec4f;
  var l_a: vec2f;
  var l_ab: vec2f;
  var l_t: f32;
  l_d = 0x1.f40000p9f;
  l_count = coax_p_wg_i32_f(coax_at_u(i32(492i)));
  {
    l_i = 0i;
    while ((l_i < 48i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_seg = coax_p_wg_at4(((i32(l_i) * 4i) + 300i));
      l_a = coax_p_v2(l_seg.x, l_seg.y);
      l_ab = coax_p_wg_sub_f2f2(coax_p_v2(l_seg.z, l_seg.w), l_a);
      l_t = coax_p_clampf((coax_p_wg_dot2(coax_p_wg_sub_f2f2(l_p, l_a), l_ab) / coax_p_fmax(coax_p_wg_dot2(l_ab, l_ab), 0x1.a36e2ep-14f)), 0.0f, 0x1.000000p0f);
      l_d = coax_p_fmin(l_d, coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, l_a), coax_p_wg_mul_f2f(l_ab, l_t))));
      l_i = (l_i + 1i);
    }
  }
  return l_d;
}
fn coax_p_verse_style_flat_hem(p_fl_y: f32, p_fl_width: f32, p_fl_aa: f32) -> f32 {
  var l_fl_y: f32 = p_fl_y;
  var l_fl_width: f32 = p_fl_width;
  var l_fl_aa: f32 = p_fl_aa;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(l_fl_width, coax_p_st_max(l_fl_aa, 0x1.0c6f7ap-20f), l_fl_y));
}
fn coax_p_villa_shade_shadows(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_s: f32;
  var l_k: vec3f;
  var l_away: vec2f;
  var l_i: i32;
  var l_b: vec4f;
  var l_reach: f32;
  var l_strength: f32;
  var l_t: f32;
  var l_q: vec2f;
  var l_r: f32;
  l_s = 0.0f;
  l_k = coax_p_villa_key_dir();
  l_away = coax_p_wg_neg_f2(coax_p_wg_normalize2(coax_p_wg_add_f2f2(coax_p_v2(l_k.x, l_k.z), coax_p_v2(0x1.a36e2ep-14f, 0.0f))));
  {
    l_i = 0i;
    while ((l_i < 32i))
    {
      l_b = coax_p_wg_at4(((i32(l_i) * 4i) + 172i));
      if ((l_b.z <= 0.0f))
      {
        break;
      }
      l_reach = (floor(l_b.w) * 0x1.99999ap-4f);
      l_strength = coax_p_fract(l_b.w);
      l_t = coax_p_clampf(coax_p_wg_dot2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_b.x, l_b.y)), l_away), 0.0f, l_reach);
      l_q = coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_b.x, l_b.y)), coax_p_wg_mul_f2f(l_away, l_t));
      l_r = (l_b.z * (0x1.000000p0f - ((0x1.666666p-2f * l_t) / coax_p_fmax(l_reach, 0x1.0624dep-10f))));
      l_s = coax_p_fmax(l_s, (coax_p_verse_style_flat_shadow((coax_p_wg_length2(l_q) / l_r)) * coax_p_fmin((l_strength * 0x1.666666p0f), 0x1.000000p0f)));
      l_i = (l_i + 1i);
    }
  }
  return l_s;
}
fn coax_p_villa_key_dir() -> vec3f {
  var l_m: vec3f;
  l_m = coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(160i)), coax_at_u(i32(161i)), coax_at_u(i32(162i))));
  return coax_p_wg_normalize3(coax_p_v3(l_m.x, coax_p_fmax(l_m.y, 0x1.ccccccp-2f), l_m.z));
}
fn coax_p_verse_style_facet_tone(p_fl_ndl: f32, p_fl_lit: f32) -> f32 {
  var l_fl_ndl: f32 = p_fl_ndl;
  var l_fl_lit: f32 = p_fl_lit;
  var l_fl_t: f32;
  l_fl_t = coax_p_st_sat((((((l_fl_ndl * 0x1.000000p-1f) + 0x1.000000p-1f) * coax_p_st_sat(l_fl_lit)) - 0x1.70a3d8p-3f) / 0x1.666666p-1f));
  return ((l_fl_t * l_fl_t) * (0x1.800000p1f - (0x1.000000p1f * l_fl_t)));
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_wg_dims_2d() -> coax_t22 {
  return coax_p_u2(textureDimensions(bloom_src).x, textureDimensions(bloom_src).y);
}
fn coax_p_wg_cvt_f2u2(p_p: coax_t22) -> vec2f {
  var l_p: coax_t22 = p_p;
  return coax_p_v2(f32(l_p.x), f32(l_p.y));
}
fn coax_p_wg_div_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a / l_b.x), (l_a / l_b.y));
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
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_b < l_a))
  {
    return l_b;
  }
  return l_a;
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
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t20 {
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
fn coax_p_wg_load_2d(p_c: coax_t20, p_level: i32) -> vec4f {
  var l_c: coax_t20 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_light, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_load_2d_1(p_c: coax_t20, p_level: i32) -> vec4f {
  var l_c: coax_t20 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_aux, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t20 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t20(l_x, l_y);
}
fn coax_p_wg_sub_i2i2(p_a: coax_t20, p_b: coax_t20) -> coax_t20 {
  var l_a: coax_t20 = p_a;
  var l_b: coax_t20 = p_b;
  return coax_p_i2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_add_i2i2(p_a: coax_t20, p_b: coax_t20) -> coax_t20 {
  var l_a: coax_t20 = p_a;
  var l_b: coax_t20 = p_b;
  return coax_p_i2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_clamp_i2i2i2(p_p0: coax_t20, p_p1: coax_t20, p_p2: coax_t20) -> coax_t20 {
  var l_p0: coax_t20 = p_p0;
  var l_p1: coax_t20 = p_p1;
  var l_p2: coax_t20 = p_p2;
  return coax_p_i2(coax_p_wg_clamp_i(l_p0.x, l_p1.x, l_p2.x), coax_p_wg_clamp_i(l_p0.y, l_p1.y, l_p2.y));
}
fn coax_p_verse_style_facet_edge(p_fl_own: f32, p_fl_other: f32) -> f32 {
  var l_fl_own: f32 = p_fl_own;
  var l_fl_other: f32 = p_fl_other;
  var l_fl_gap: f32;
  var l_fl_at: f32;
  if ((l_fl_own <= 0.0f))
  {
    return 0.0f;
  }
  if ((l_fl_other <= 0.0f))
  {
    return 0x1.000000p0f;
  }
  l_fl_gap = (coax_p_st_max(l_fl_other, (-l_fl_other)) - l_fl_own);
  l_fl_at = ((l_fl_own * 0x1.47ae14p-5f) + 0x1.000000p-2f);
  return coax_p_verse_style_soft_step((l_fl_at * 0x1.800000p0f), (l_fl_at * 0x1.000000p-1f), l_fl_gap);
}
fn coax_p_villa_encode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_wg_pow_f3f3(coax_p_wg_clamp_f3f3f3(l_c, coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), coax_p_wg_make_f3_f((0x1.000000p0f / 0x1.19999ap1f)));
}
fn coax_p_villa_plates(p_p: vec2f, p_under: vec3f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_c: vec3f;
  var l_k: i32;
  l_c = l_under;
  {
    l_k = 0i;
    while ((l_k < 16i))
    {
      if ((f32(l_k) >= coax_at_u(i32(5i))))
      {
        break;
      }
      l_c = coax_p_villa_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 32i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 32i)));
      l_k = (l_k + 1i);
    }
  }
  return l_c;
}
fn coax_p_wg_clamp_i(p_x: i32, p_low: i32, p_high: i32) -> i32 {
  var l_x: i32 = p_x;
  var l_low: i32 = p_low;
  var l_high: i32 = p_high;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_low), l_high);
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_villa_nothing() -> coax_t14 {
  return coax_t14(coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0.0f, 0x1.000000p0f, 0.0f));
}
fn coax_p_verse_style_facet_jitter(p_fl_i: f32, p_fl_j: f32, p_fl_round: f32, p_fl_down: f32, p_fl_seed: f32) -> f32 {
  var l_fl_i: f32 = p_fl_i;
  var l_fl_j: f32 = p_fl_j;
  var l_fl_round: f32 = p_fl_round;
  var l_fl_down: f32 = p_fl_down;
  var l_fl_seed: f32 = p_fl_seed;
  var l_fl_a: f32;
  l_fl_a = (l_fl_i - (l_fl_round * floor((l_fl_i / coax_p_st_max(l_fl_round, 0x1.000000p0f)))));
  if (((l_fl_j < 0x1.000000p-1f) || (l_fl_j > (l_fl_down - 0x1.000000p-1f))))
  {
    l_fl_a = 0.0f;
  }
  return ((coax_p_verse_style_hash((l_fl_a + (l_fl_seed * 0x1.a00000p3f)), (l_fl_j + (l_fl_seed * 0x1.c00000p2f))) * 0x1.000000p1f) - 0x1.000000p0f);
}
fn coax_p_cross3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return vec3f(((l_a.y * l_b.z) - (l_a.z * l_b.y)), ((l_a.z * l_b.x) - (l_a.x * l_b.z)), ((l_a.x * l_b.y) - (l_a.y * l_b.x)));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_verse_style_flat_colour(p_fl_realm: f32, p_fl_slot: f32, p_fl_ch: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_slot: f32 = p_fl_slot;
  var l_fl_ch: f32 = p_fl_ch;
  return coax_pow_bg_vs(coax_p_verse_style_flat_srgb(l_fl_realm, l_fl_slot, l_fl_ch), 0x1.19999ap1f);
}
fn coax_p_verse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  var l_e: f32 = p_e;
  var l_s: f32 = p_s;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_floor_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(floor(l_p0.x), floor(l_p0.y), floor(l_p0.z));
}
fn coax_p_villa_hash3(p_p: vec3f) -> f32 {
  var l_p: vec3f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot3(l_p, coax_p_v3(0x1.fc6666p6f, 0x1.37b334p8f, 0x1.2accccp6f))) * 0x1.55dd18p15f));
}
fn coax_p_wg_add_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x + l_b), (l_a.y + l_b), (l_a.z + l_b));
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
fn coax_p_st_bayer2(p_fl_a: f32, p_fl_b: f32) -> f32 {
  var l_fl_a: f32 = p_fl_a;
  var l_fl_b: f32 = p_fl_b;
  return (((0x1.000000p1f * l_fl_a) + (0x1.800000p1f * l_fl_b)) - ((0x1.000000p2f * l_fl_a) * l_fl_b));
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
fn coax_p_wg_fract_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y));
}
fn coax_p_wg_mul_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a * l_b.x), (l_a * l_b.y));
}
fn coax_p_wg_sub_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a - l_b.x), (l_a - l_b.y));
}
fn coax_p_villa_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_wg_clamp_f3f3f3(p_p0: vec3f, p_p1: vec3f, p_p2: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: vec3f = p_p2;
  return coax_p_v3(coax_p_clampf(l_p0.x, l_p1.x, l_p2.x), coax_p_clampf(l_p0.y, l_p1.y, l_p2.y), coax_p_clampf(l_p0.z, l_p1.z, l_p2.z));
}
fn coax_p_wg_pow_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_pow_bg_vs(l_p0.x, l_p1.x), coax_pow_bg_vs(l_p0.y, l_p1.y), coax_pow_bg_vs(l_p0.z, l_p1.z));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_wg_normalize2(p_a: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  return coax_p_scale2(l_a, (0x1.000000p0f / coax_p_wg_length2(l_a)));
}
fn coax_p_wg_neg_f2(p_a: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  return coax_p_v2((-l_a.x), (-l_a.y));
}
fn coax_p_verse_style_flat_shadow(p_fl_d: f32) -> f32 {
  var l_fl_d: f32 = p_fl_d;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.b33334p-1f, 0x1.333334p-3f, l_fl_d));
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t22 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t22(l_x, l_y);
}
fn coax_p_wg_make_f3_f(p_p0: f32) -> vec3f {
  var l_p0: f32 = p_p0;
  return coax_p_v3(l_p0, l_p0, l_p0);
}
fn coax_p_villa_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_half: vec2f;
  var l_corner: f32;
  var l_q: vec2f;
  var l_dd: f32;
  var l_over: bool;
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
  l_corner = (0x1.800000p2f * l_s.x);
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_dd = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  if ((l_dd > 0x1.000000p0f))
  {
    return l_under;
  }
  l_over = (l_s.z >= 0x1.000000p1f);
  l_fill = select(coax_p_v3(0x1.47ae14p-6f, 0x1.99999ap-6f, 0x1.70a3d8p-5f), coax_p_v3(0x1.70a3d8p-5f, 0x1.c28f5cp-5f, 0x1.70a3d8p-4f), l_over);
  l_line = select(coax_p_v3(0x1.70a3d8p-2f, 0x1.ae147ap-2f, 0x1.28f5c2p-1f), coax_p_v3(0x1.3d70a4p-1f, 0x1.666666p-1f, 0x1.c28f5cp-1f), (l_over || (l_s.z >= 0x1.000000p0f)));
  l_w = (0x1.400000p0f * l_s.x);
  l_inside = coax_p_clampf((0x1.000000p-1f - l_dd), 0.0f, 0x1.000000p0f);
  l_edge = coax_p_clampf((((l_w * 0x1.000000p-1f) + 0x1.000000p-1f) - abs((l_dd + (l_w * 0x1.000000p-1f)))), 0.0f, 0x1.000000p0f);
  l_a = coax_p_clampf(l_s.y, 0.0f, 0x1.000000p0f);
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_under, l_fill, (l_a * l_inside)), l_line, ((l_edge * coax_p_fmin(0x1.000000p0f, (l_a + 0x1.99999ap-4f))) * 0x1.99999ap-1f));
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
fn coax_p_length3(p_a: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_verse_style_hash(p_x: f32, p_y: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_v: f32;
  l_v = (sin(((l_x * 0x1.fc6666p6f) + (l_y * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
}
fn coax_p_verse_style_flat_srgb(p_fl_realm: f32, p_fl_slot: f32, p_fl_ch: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_slot: f32 = p_fl_slot;
  var l_fl_ch: f32 = p_fl_ch;
  if ((l_fl_realm < 0x1.000000p-1f))
  {
    if ((l_fl_slot < 0x1.000000p-1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.99999ap-2f, 0x1.da1cacp-2f, 0x1.6147aep-1f);
    }
    if ((l_fl_slot < 0x1.800000p0f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.7d70a4p-1f, 0x1.6978d4p-1f, 0x1.b1a9fcp-1f);
    }
    if ((l_fl_slot < 0x1.400000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.c5a1cap-2f, 0x1.2b020cp-1f, 0x1.12f1aap-1f);
    }
    if ((l_fl_slot < 0x1.c00000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.f9db22p-2f, 0x1.43126ep-1f, 0x1.2147aep-1f);
    }
    if ((l_fl_slot < 0x1.200000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.c00000p-1f, 0x1.a3d70ap-1f, 0x1.6b851ep-1f);
    }
    if ((l_fl_slot < 0x1.600000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.fae148p-1f, 0x1.f0a3d8p-1f);
    }
    if ((l_fl_slot < 0x1.a00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.51eb86p-1f, 0x1.51eb86p-1f, 0x1.b851ecp-1f);
    }
    if ((l_fl_slot < 0x1.e00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.333334p-1f, 0x1.333334p-1f, 0x1.a3d70ap-1f);
    }
    if ((l_fl_slot < 0x1.100000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.f5c290p-1f, 0x1.eb851ep-1f, 0x1.ccccccp-1f);
    }
    if ((l_fl_slot < 0x1.300000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.a1cac0p-3f, 0x1.b22d0ep-3f, 0x1.5d2f1ap-2f);
    }
    if ((l_fl_slot < 0x1.500000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.ee147ap-1f, 0x1.dfbe76p-1f, 0x1.bdf3b6p-1f);
    }
    if ((l_fl_slot < 0x1.700000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.158106p-2f, 0x1.c9ba5ep-2f, 0x1.ae147ap-2f);
    }
    if ((l_fl_slot < 0x1.900000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.d1eb86p-2f, 0x1.3d70a4p-1f, 0x1.f5c290p-2f);
    }
    if ((l_fl_slot < 0x1.b00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.28f5c2p-1f, 0x1.a3d70ap-1f, 0x1.99999ap-1f);
    }
    if ((l_fl_slot < 0x1.d00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.6147aep-1f, 0x1.170a3ep-1f, 0x1.ae147ap-2f);
    }
    if ((l_fl_slot < 0x1.f00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.f5c290p-1f, 0x1.c00000p-1f, 0x1.28f5c2p-1f);
    }
    if ((l_fl_slot < 0x1.080000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.8f5c28p-1f, 0x1.ccccccp-1f, 0x1.b33334p-1f);
    }
    if ((l_fl_slot < 0x1.180000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.8d4fe0p-1f, 0x1.87ae14p-1f, 0x1.bbe76cp-1f);
    }
    if ((l_fl_slot < 0x1.280000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.10e560p-1f, 0x1.10e560p-1f, 0x1.71a9fcp-1f);
    }
    return coax_p_st_pick(l_fl_ch, 0x1.333334p-1f, 0x1.666666p-1f, 0x1.3d70a4p-1f);
  }
  if ((l_fl_realm > 0x1.800000p0f))
  {
    if ((l_fl_slot < 0x1.000000p-1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.410624p-1f, 0x1.6353f8p-1f, 0x1.a76c8cp-1f);
    }
    if ((l_fl_slot < 0x1.800000p0f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.ab851ep-1f, 0x1.b3b646p-1f, 0x1.d81062p-1f);
    }
    if ((l_fl_slot < 0x1.400000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.410624p-1f, 0x1.839582p-1f, 0x1.f9db22p-2f);
    }
    if ((l_fl_slot < 0x1.c00000p1f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.170a3ep-1f, 0x1.6147aep-1f, 0x1.d60418p-2f);
    }
    if ((l_fl_slot < 0x1.200000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.c7ae14p-1f, 0x1.a5e354p-1f, 0x1.795810p-1f);
    }
    if ((l_fl_slot < 0x1.600000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.f7cedap-1f, 0x1.e7ef9ep-1f);
    }
    if ((l_fl_slot < 0x1.a00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.28f5c2p-1f, 0x1.2d0e56p-1f, 0x1.99999ap-1f);
    }
    if ((l_fl_slot < 0x1.e00000p2f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.333334p-1f, 0x1.333334p-1f, 0x1.a3d70ap-1f);
    }
    if ((l_fl_slot < 0x1.100000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.fbe76cp-1f, 0x1.f7cedap-1f, 0x1.ee147ap-1f);
    }
    if ((l_fl_slot < 0x1.300000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.51eb86p-3f, 0x1.51eb86p-3f, 0x1.f9db22p-3f);
    }
    if ((l_fl_slot < 0x1.500000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.ec0832p-1f, 0x1.e5e354p-1f, 0x1.d1eb86p-1f);
    }
    if ((l_fl_slot < 0x1.700000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.7df3b6p-2f, 0x1.49374cp-1f, 0x1.03126ep-1f);
    }
    if ((l_fl_slot < 0x1.900000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.3126eap-1f, 0x1.9fbe76p-1f, 0x1.f1a9fcp-2f);
    }
    if ((l_fl_slot < 0x1.b00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.010624p-1f, 0x1.916872p-1f, 0x1.a5e354p-1f);
    }
    if ((l_fl_slot < 0x1.d00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.5d2f1ap-1f, 0x1.0cccccp-1f, 0x1.d1eb86p-2f);
    }
    if ((l_fl_slot < 0x1.f00000p3f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.e9fbe8p-1f, 0x1.bdf3b6p-1f, 0x1.3126eap-1f);
    }
    if ((l_fl_slot < 0x1.080000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.916872p-1f, 0x1.d3f7cep-1f, 0x1.b7cedap-1f);
    }
    if ((l_fl_slot < 0x1.180000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.656042p-1f, 0x1.558106p-1f, 0x1.ab851ep-1f);
    }
    if ((l_fl_slot < 0x1.280000p4f))
    {
      return coax_p_st_pick(l_fl_ch, 0x1.a5e354p-1f, 0x1.5d2f1ap-1f, 0x1.c9ba5ep-1f);
    }
    return coax_p_st_pick(l_fl_ch, 0x1.d60418p-1f, 0x1.472b02p-1f, 0x1.753f7cp-1f);
  }
  if ((l_fl_slot < 0x1.000000p-1f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.5374bcp-1f, 0x1.a76c8cp-1f, 0x1.a1cac0p-1f);
  }
  if ((l_fl_slot < 0x1.800000p0f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.d1eb86p-1f, 0x1.dfbe76p-1f, 0x1.a1cac0p-1f);
  }
  if ((l_fl_slot < 0x1.400000p1f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.3f7ceep-1f, 0x1.6b851ep-1f, 0x1.28f5c2p-2f);
  }
  if ((l_fl_slot < 0x1.c00000p1f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.010624p-1f, 0x1.333334p-1f, 0x1.f1a9fcp-3f);
  }
  if ((l_fl_slot < 0x1.200000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.b3b646p-1f, 0x1.818938p-1f, 0x1.14fdf4p-1f);
  }
  if ((l_fl_slot < 0x1.600000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.f5c290p-1f, 0x1.d70a3ep-1f);
  }
  if ((l_fl_slot < 0x1.a00000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.666666p-1f, 0x1.666666p-1f, 0x1.ae147ap-1f);
  }
  if ((l_fl_slot < 0x1.e00000p2f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.851eb8p-1f, 0x1.800000p-1f, 0x1.c28f5cp-1f);
  }
  if ((l_fl_slot < 0x1.100000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.ee147ap-1f, 0x1.c18938p-1f);
  }
  if ((l_fl_slot < 0x1.300000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.d2f1aap-4f, 0x1.f3b646p-4f, 0x1.51eb86p-3f);
  }
  if ((l_fl_slot < 0x1.500000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.e5e354p-1f, 0x1.d60418p-1f, 0x1.a5e354p-1f);
  }
  if ((l_fl_slot < 0x1.700000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.bd70a4p-2f, 0x1.14fdf4p-1f, 0x1.d0e560p-3f);
  }
  if ((l_fl_slot < 0x1.900000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.6b851ep-1f, 0x1.9fbe76p-1f, 0x1.6978d4p-2f);
  }
  if ((l_fl_slot < 0x1.b00000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.1f3b64p-1f, 0x1.9fbe76p-1f, 0x1.818938p-1f);
  }
  if ((l_fl_slot < 0x1.d00000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.516872p-1f, 0x1.0ed916p-1f, 0x1.6978d4p-2f);
  }
  if ((l_fl_slot < 0x1.f00000p3f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.c18938p-1f, 0x1.09374cp-1f, 0x1.d0e560p-3f);
  }
  if ((l_fl_slot < 0x1.080000p4f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.89ba5ep-1f, 0x1.d9999ap-1f, 0x1.ad9168p-1f);
  }
  if ((l_fl_slot < 0x1.180000p4f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.5374bcp-1f, 0x1.472b02p-1f, 0x1.89ba5ep-1f);
  }
  if ((l_fl_slot < 0x1.280000p4f))
  {
    return coax_p_st_pick(l_fl_ch, 0x1.9374bcp-1f, 0x1.676c8cp-1f, 0x1.c18938p-1f);
  }
  return coax_p_st_pick(l_fl_ch, 0x1.4f5c28p-1f, 0x1.6b851ep-1f, 0x1.353f7cp-1f);
}
fn coax_p_scale2(p_a: vec2f, p_k: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_k: f32 = p_k;
  return vec2f((l_a.x * l_k), (l_a.y * l_k));
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
fn coax_p_wg_abs_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(abs(l_p0.x), abs(l_p0.y));
}
fn coax_p_wg_max_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v2(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y));
}
fn coax_p_st_pick(p_fl_ch: f32, p_fl_r: f32, p_fl_g: f32, p_fl_b: f32) -> f32 {
  var l_fl_ch: f32 = p_fl_ch;
  var l_fl_r: f32 = p_fl_r;
  var l_fl_g: f32 = p_fl_g;
  var l_fl_b: f32 = p_fl_b;
  if ((l_fl_ch < 0x1.000000p-1f))
  {
    return l_fl_r;
  }
  if ((l_fl_ch < 0x1.800000p0f))
  {
    return l_fl_g;
  }
  return l_fl_b;
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_13 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_13(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_15) -> coax_io_16 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_16(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_17 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_17(coax_r.x, coax_r.y);
}
@fragment
fn scene_fs(coax_in: coax_io_13) -> coax_out_17 {
  let coax_r = coax_p_scene_fs(coax_t13(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_out_17(coax_r.x, coax_r.y);
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