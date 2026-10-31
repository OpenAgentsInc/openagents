// ---- generated: apps/openagents-web/gpu/fairy.coax (the module) by coax wgsl; do not edit
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
  f4: vec4f,
}
struct coax_io_15 {
  @builtin(position) x: vec4f,
  @location(0) y: vec4f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t16 {
  x: vec3f,
  y: f32,
  z: f32,
  w: vec2f,
  f4: f32,
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
struct coax_t20 {
  x: u32,
  y: u32,
}
struct coax_t22 {
  x: vec3f,
  y: vec3f,
}
struct coax_t25 {
  x: i32,
  y: i32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 199>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> heights: array<f32>;
@group(1) @binding(1) var<storage, read> objects: array<coax_t10>;
@group(1) @binding(2) var<storage, read> water_nodes: array<f32>;
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
  var l_cells: u32;
  var l_c: vec2f;
  var l_gx: i32;
  var l_gz: i32;
  var l_w: vec3f;
  var l_normal: vec3f;
  var l_objects_n: u32;
  var l_m: coax_t10;
  var l_kind: u32;
  var l_shape_look: u32;
  var l_nudge: f32;
  var l_v: coax_t14;
  var l__sw3: u32;
  var l_p: vec4f;
  var l_w_21: vec3f;
  var l_c0: vec3f;
  var l_c1: vec3f;
  var l_c2: vec3f;
  var l_normal_25: vec3f;
  var l_code: u32;
  var l_look: u32;
  var l_own: bool;
  l_o = coax_t13();
  l_n = coax_p_wg_u32_f(coax_at_u(i32(32i)));
  l_cells = (l_n * coax_p_wg_u32_f(coax_at_u(i32(238i))));
  if ((l_instance < l_cells))
  {
    l_c = coax_p_fairy_quad((l_id % 6u));
    l_gx = (i32((l_instance % l_n)) + coax_p_wg_i32_f(l_c.x));
    l_gz = (i32((l_instance / l_n)) + coax_p_wg_i32_f(l_c.y));
    l_w = coax_p_v3((coax_at_u(i32(34i)) + (f32(l_gx) * coax_at_u(i32(33i)))), coax_p_fairy_height(l_gx, l_gz), (coax_at_u(i32(35i)) + (f32(l_gz) * coax_at_u(i32(33i)))));
    l_normal = coax_p_wg_normalize3(coax_p_v3((coax_p_fairy_height((l_gx - 2i), l_gz) - coax_p_fairy_height((l_gx + 2i), l_gz)), (0x1.000000p2f * coax_at_u(i32(33i))), (coax_p_fairy_height(l_gx, (l_gz - 2i)) - coax_p_fairy_height(l_gx, (l_gz + 2i)))));
    l_o.x = coax_p_fairy_clip(l_w);
    l_o.y = l_w;
    l_o.z = l_normal;
    l_o.w = coax_p_wg_make_f3_f2f(coax_p_fairy_path_planes(l_gx, l_gz), 0.0f);
    l_o.f4 = 99u;
    l_o.f5 = coax_p_v3(0.0f, 0.0f, 0.0f);
    l_o.f6 = coax_p_v2(0.0f, 0.0f);
    return l_o;
  }
  l_objects_n = u32(i32(arrayLength(&objects)));
  l_m = objects[i32((l_instance - l_cells))];
  l_kind = (coax_p_wg_u32_f(l_m.w.w) % 8u);
  l_shape_look = ((coax_p_wg_u32_f(l_m.w.w) % 128u) / 8u);
  l_nudge = select(0.0f, 0x1.47ae14p-3f, ((l_shape_look <= 2u) || (l_shape_look == 7u)));
  l_v = coax_t14();
  {
    l__sw3 = l_kind;
    if ((l__sw3 == 1u))
    {
      l_v = coax_p_fairy_box_vertex(l_id);
    }
    else
    {
      if ((l__sw3 == 2u))
      {
        l_v = coax_p_fairy_sphere_vertex(l_id, l_nudge, coax_p_fract(((l_m.x.w * 0x1.189374p-3f) + (l_m.z.w * 0x1.22d0e6p-4f))));
      }
      else
      {
        if ((l__sw3 == 3u))
        {
          l_v = coax_p_fairy_cylinder_vertex(l_id);
        }
        else
        {
          l_v = coax_p_fairy_cone_vertex(l_id);
        }
      }
    }
  }
  l_p = coax_p_v4(l_v.x.x, l_v.x.y, l_v.x.z, 0x1.000000p0f);
  l_w_21 = coax_p_v3(coax_p_wg_dot4(l_m.x, l_p), coax_p_wg_dot4(l_m.y, l_p), coax_p_wg_dot4(l_m.z, l_p));
  l_c0 = coax_p_v3(l_m.x.x, l_m.y.x, l_m.z.x);
  l_c1 = coax_p_v3(l_m.x.y, l_m.y.y, l_m.z.y);
  l_c2 = coax_p_v3(l_m.x.z, l_m.y.z, l_m.z.z);
  l_normal_25 = coax_p_wg_add_f3f3(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c1, l_c2), l_v.y.x), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c2, l_c0), l_v.y.y)), coax_p_wg_mul_f3f(coax_p_wg_cross3(l_c0, l_c1), l_v.y.z));
  l_code = coax_p_wg_u32_f(l_m.w.w);
  l_look = ((l_code % 128u) / 8u);
  if ((((l_look == 1u) && (l_kind == 2u)) && (coax_p_wg_length3(l_c1) > 0x1.000000p0f)))
  {
    l_w_21 = coax_p_v3((l_w_21.x + ((((coax_p_FAIRY_WIND()).x * coax_p_fairy_wind_bend(coax_p_v2(l_w_21.x, l_w_21.z))) * 0x1.000000p-2f) * (l_v.x.y + 0x1.000000p-1f))), l_w_21.y, (l_w_21.z + ((((coax_p_FAIRY_WIND()).y * coax_p_fairy_wind_bend(coax_p_v2(l_w_21.x, l_w_21.z))) * 0x1.000000p-2f) * (l_v.x.y + 0x1.000000p-1f))));
  }
  l_o.x = coax_p_fairy_clip(l_w_21);
  l_o.y = l_w_21;
  l_o.z = coax_p_wg_normalize3(coax_p_wg_add_f3f3(l_normal_25, coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  l_own = (((((l_look == 3u) || (l_look == 4u)) || (l_look == 5u)) || (l_look == 6u)) || (l_look == 9u));
  l_o.w = coax_p_fairy_snap(coax_p_v3(l_m.w.x, l_m.w.y, l_m.w.z), select(0x1.99999ap-1f, 0x1.000000p-1f, l_own));
  l_o.f4 = (l_look + select(0u, 32u, (l_code >= 128u)));
  l_o.f5 = l_v.x;
  l_o.f6 = coax_p_v2(coax_p_wg_length3(l_c1), coax_p_smoothstep(0x1.99999ap0f, 0x1.99999ap1f, coax_p_wg_length3(l_c1)));
  return l_o;
}
fn coax_p_water_vs(p_id: u32, p_piece: u32) -> coax_t15 {
  var l_id: u32 = p_id;
  var l_piece: u32 = p_piece;
  var l_o: coax_t15;
  var l_qa: u32;
  var l_ql: u32;
  var l_quad_id: u32;
  var l_c: vec2f;
  var l_qx: u32;
  var l_qz: u32;
  var l_across: f32;
  var l_along: f32;
  var l_base: f32;
  var l_uv: vec2f;
  var l_low: f32;
  var l_k: u32;
  var l_corner: vec2f;
  var l_r: coax_t16;
  var l_wave: vec4f;
  var l_world: vec3f;
  l_o = coax_t15();
  l_qa = coax_p_wg_u32_f(coax_at_u(i32(782i)));
  l_ql = coax_p_wg_u32_f(coax_at_u(i32(783i)));
  l_quad_id = (l_id / 6u);
  l_c = coax_p_fairy_quad((l_id % 6u));
  l_qx = (l_quad_id % l_qa);
  l_qz = (l_quad_id / l_qa);
  l_across = ((coax_at_u(i32(780i)) - 0x1.000000p0f) / f32(l_qa));
  l_along = (coax_at_u(i32(784i)) / f32(l_ql));
  l_base = (f32(l_piece) * coax_at_u(i32(784i)));
  l_uv = coax_p_v2(((f32(l_qx) + l_c.x) * l_across), (l_base + ((f32(l_qz) + l_c.y) * l_along)));
  l_low = 0x1.f40000p9f;
  {
    l_k = 0u;
    while ((l_k < 4u))
    {
      l_corner = coax_p_v2((f32((l_qx + (l_k & 1u))) * l_across), (l_base + (f32((l_qz + (l_k >> u32(1u)))) * l_along)));
      l_low = coax_p_fmin(l_low, (-(coax_p_fairy_water_rest(l_corner)).y));
      l_k = (l_k + 1u);
    }
  }
  l_r = coax_p_fairy_water_rest(l_uv);
  if (((l_low > 0x1.333334p-1f) || (l_quad_id >= (l_qa * l_ql))))
  {
    l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
    return l_o;
  }
  l_wave = coax_p_fairy_water_gerstner(coax_p_v2(l_r.x.x, l_r.x.z), l_r.y, 0x1.000000p0f);
  l_world = coax_p_wg_add_f3f3(l_r.x, coax_p_v3(l_wave.x, l_wave.y, l_wave.z));
  l_o.x = coax_p_fairy_clip(l_world);
  l_o.y = coax_p_v4(l_world.x, l_world.y, l_world.z, l_r.y);
  l_o.z = coax_p_wg_make_f4_f2f2(coax_p_v2(l_r.x.x, l_r.x.z), l_r.w);
  l_o.w = coax_p_v4(l_r.f4, 0.0f, l_wave.w, l_r.z);
  l_o.f4 = coax_p_v4(0.0f, (l_world.y - l_r.x.y), 0.0f, 0.0f);
  return l_o;
}
fn coax_p_fx_vs(p__vid4: u32, p__iid5: u32, p_in_: coax_t17) -> coax_t18 {
  var l__vid4: u32 = p__vid4;
  var l__iid5: u32 = p__iid5;
  var l_in_: coax_t17 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t18;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t18();
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t19 {
  var l_pos: vec4f = p_pos;
  var l_d: vec3f;
  var l_c: vec3f;
  var l_share: f32;
  l_d = coax_p_fairy_ray(coax_p_v2(l_pos.x, l_pos.y));
  l_c = coax_p_fairy_sky_colour(l_d);
  l_share = select(0.0f, 0x1.333334p-2f, (coax_p_wg_dot3(l_d, coax_p_fairy_sun_dir()) > 0x1.ffd370p-1f));
  return coax_t19(coax_p_v4(l_c.x, l_c.y, l_c.z, l_share), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_scene_fs(p_s: coax_t13) -> coax_t19 {
  var l_s: coax_t13 = p_s;
  var l_ground_aa: f32;
  var l_local_aa: f32;
  var l_face_n: vec3f;
  var l_n: vec3f;
  var l_edge: f32;
  var l_base: vec3f;
  var l_xz: vec2f;
  var l_shadow: f32;
  var l_character: bool;
  var l_glow: f32;
  var l_keep: bool;
  var l_look: u32;
  var l_near: f32;
  var l_clear: f32;
  var l_p: coax_t20;
  var l_bayer: array<f32, 16>;
  var l_q: vec2f;
  var l_along: f32;
  var l_fan: f32;
  var l_f: f32;
  var l_notch: f32;
  var l_half: f32;
  var l_tips: f32;
  var l_pale: vec3f;
  var l_seam: f32;
  var l_rim: f32;
  var l_q_29: vec3f;
  var l_bands: f32;
  var l_streak: f32;
  var l_white: f32;
  var l_cell: coax_t20;
  var l_depth: f32;
  var l_w: vec3f;
  var l_drift: vec2f;
  var l_ph: f32;
  var l_thin: f32;
  var l_ripple: f32;
  var l_from_hem: f32;
  var l_t: f32;
  var l_r: vec2f;
  var l_c: vec3f;
  var l_share: f32;
  var l_dist: f32;
  l_ground_aa = coax_p_fmax(coax_p_wg_length2(coax_p_wg_fwidth_f2(coax_p_v2(l_s.y.x, l_s.y.z))), 0x1.0624dep-9f);
  l_local_aa = fwidth(l_s.f5.y);
  l_face_n = coax_p_wg_cross3(coax_p_wg_dpdx_f3(l_s.y), coax_p_wg_dpdy_f3(l_s.y));
  l_face_n = coax_p_wg_normalize3(select(l_face_n, coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), (coax_p_wg_dot3(l_face_n, l_face_n) < 0x1.6849b8p-47f)));
  if ((coax_p_wg_dot3(l_face_n, l_s.z) < 0.0f))
  {
    l_face_n = coax_p_wg_neg_f3(l_face_n);
  }
  l_n = coax_p_wg_normalize3(l_s.z);
  l_edge = 0x1.000000p0f;
  l_base = l_s.w;
  l_xz = coax_p_v2(l_s.y.x, l_s.y.z);
  l_shadow = 0x1.000000p0f;
  l_character = false;
  l_glow = 0.0f;
  l_keep = false;
  l_look = l_s.f4;
  if (((l_look >= 32u) && (l_look < 99u)))
  {
    l_look = (l_look - 32u);
  }
  else
  {
    if ((l_look < 32u))
    {
      l_near = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))));
      l_clear = select(coax_p_fmax(0x1.99999ap0f, coax_p_wg_length2(coax_p_wg_sub_f2f2(coax_p_v2(coax_at_u(i32(24i)), coax_at_u(i32(26i))), coax_p_v2(coax_at_u(i32(43i)), coax_at_u(i32(7i)))))), 0x1.99999ap0f, (coax_at_u(i32(36i)) > 0.0f));
      if ((l_near < l_clear))
      {
        l_edge = 0.0f;
        l_p = coax_p_wg_mod_u2u(coax_p_wg_cvt_u2f2(coax_p_v2(l_s.x.x, l_s.x.y)), 4u);
        l_bayer = array<f32, 16>(0.0f, 0x1.000000p3f, 0x1.000000p1f, 0x1.400000p3f, 0x1.800000p3f, 0x1.000000p2f, 0x1.c00000p3f, 0x1.800000p2f, 0x1.800000p1f, 0x1.600000p3f, 0x1.000000p0f, 0x1.200000p3f, 0x1.e00000p3f, 0x1.c00000p2f, 0x1.a00000p3f, 0x1.400000p2f);
        if (((coax_p_smoothstep((l_clear * 0x1.99999ap-1f), l_clear, l_near) * 0x1.000000p4f) <= (l_bayer[i32(((l_p.y * 4u) + l_p.x))] + 0x1.000000p-1f)))
        {
          discard;
        }
      }
    }
  }
  if ((l_look == 99u))
  {
    l_base = coax_p_fairy_ground_colour(l_xz, l_ground_aa, coax_p_v2(l_s.w.x, l_s.w.y));
    l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_fairy_pal(0x1.000000p2f), coax_p_stepf(l_s.y.y, (coax_at_u(i32(41i)) + 0x1.99999ap-3f)));
    l_shadow = coax_p_fairy_blob_shadows(l_xz);
    l_n = l_face_n;
    l_edge = 0.0f;
  }
  else
  {
    if (((l_look == 3u) || (l_look == 6u)))
    {
      l_character = true;
      l_keep = true;
      l_n = l_face_n;
    }
    else
    {
      if ((l_look == 4u))
      {
        l_glow = 0x1.000000p0f;
      }
      else
      {
        if ((l_look == 9u))
        {
          l_glow = 0x1.000000p0f;
          l_keep = true;
        }
        else
        {
          if ((l_look == 5u))
          {
            l_q = coax_p_wg_mul_f2f(coax_p_v2(l_s.f5.x, l_s.f5.y), 0x1.000000p1f);
            l_along = coax_p_clampf(((l_q.y + 0x1.000000p0f) * 0x1.000000p-1f), 0.0f, 0x1.000000p0f);
            l_fan = coax_p_wg_atan2(l_q.x, (l_q.y + 0x1.000000p0f));
            l_f = coax_p_fract(((l_fan * 0x1.b33334p1f) + 0x1.000000p-1f));
            l_notch = (abs((l_f - 0x1.000000p-1f)) * 0x1.000000p1f);
            l_half = (coax_p_clampf(((0x1.e66666p0f * coax_pow_bg_vs(l_along, 0x1.333334p-1f)) * (0x1.000000p0f - l_along)), 0.0f, 0x1.000000p0f) * (0x1.000000p0f - ((0x1.99999ap-3f * l_notch) * coax_p_smoothstep(0x1.000000p-2f, 0x1.333334p-1f, l_along))));
            if (((abs(l_q.x) > l_half) || (l_along > (0x1.000000p0f - (0x1.99999ap-4f * l_notch)))))
            {
              discard;
            }
            l_tips = coax_p_stepf((0x1.000000p-1f + (0x1.47ae14p-3f * l_notch)), l_along);
            l_pale = coax_p_wg_mix_f3f3f(l_base, coax_p_fairy_pal(0x1.400000p3f), 0x1.666666p-1f);
            l_base = coax_p_wg_mix_f3f3f(l_pale, l_base, l_tips);
            l_seam = (coax_p_stepf(0x1.dc28f6p-1f, l_notch) * coax_p_stepf(0x1.333334p-2f, l_along));
            l_rim = coax_p_stepf(0x1.b851ecp-1f, (abs(l_q.x) / coax_p_fmax(l_half, 0x1.0624dep-10f)));
            l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_fairy_pal(0x1.000000p3f), coax_p_fmax((l_seam * 0x1.99999ap-1f), l_rim));
            l_n = l_face_n;
            l_glow = 0x1.333334p-2f;
            l_keep = true;
          }
          else
          {
            if ((l_look == 10u))
            {
              l_q_29 = l_s.f5;
              l_bands = coax_p_fairy_noise(coax_p_v2(((l_q_29.x * 0x1.c00000p3f) + (l_q_29.z * 0x1.800000p2f)), ((l_q_29.y * 0x1.000000p2f) + (coax_at_u(i32(2i)) * 0x1.333334p1f))));
              l_streak = coax_p_fairy_noise(coax_p_v2((l_q_29.x * 0x1.f00000p4f), ((l_q_29.y * 0x1.200000p3f) + (coax_at_u(i32(2i)) * 0x1.000000p2f))));
              l_white = coax_p_clampf((((coax_p_smoothstep(0x1.000000p-1f, 0x1.b33334p-1f, l_bands) * 0x1.99999ap-1f) + (coax_p_smoothstep(0x1.333334p-1f, 0x1.ccccccp-1f, l_streak) * 0x1.000000p-1f)) + (coax_p_smoothstep((-0x1.333334p-3f), (-0x1.000000p-1f), l_q_29.y) * 0x1.666666p-1f)), 0.0f, 0x1.000000p0f);
              l_cell = coax_p_wg_mod_u2u(coax_p_wg_cvt_u2f2(coax_p_v2(l_s.x.x, l_s.x.y)), 2u);
              if (((l_white < 0x1.000000p-2f) && (((l_cell.x + l_cell.y) % 2u) == 0u)))
              {
                discard;
              }
              l_base = coax_p_wg_mix_f3f3f(coax_p_fairy_pal(0x1.a00000p3f), coax_p_fairy_pal(0x1.400000p3f), coax_p_stepf(0x1.000000p-1f, l_white));
              l_glow = 0.0f;
            }
            else
            {
              if ((l_look == 11u))
              {
                l_depth = (coax_at_u(i32(41i)) - coax_p_fairy_ground_h(l_xz));
                l_w = coax_p_fairy_pal(0x1.a00000p3f);
                l_base = coax_p_wg_mix_f3f3f(l_w, coax_p_wg_mul_f3f(l_w, 0x1.a3d70ap-1f), coax_p_stepf(0x1.ae147ap-2f, l_depth));
                l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_wg_mix_f3f3f(l_w, coax_p_fairy_pal(0x1.400000p3f), 0x1.1eb852p-2f), coax_p_stepf(l_depth, 0x1.99999ap-3f));
                l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_wg_mix_f3f3f(l_w, coax_p_fairy_pal(0x1.400000p3f), 0x1.19999ap-1f), coax_p_stepf(l_depth, 0x1.1eb852p-4f));
                l_drift = coax_p_v2(0x1.99999ap-1f, 0x1.333334p-1f);
                l_ph = (((coax_p_wg_dot2(l_xz, l_drift) / 0x1.800000p2f) + (coax_p_fairy_noise(coax_p_wg_mul_f2f(l_xz, 0x1.47ae14p-5f)) * 0x1.800000p0f)) - (coax_at_u(i32(2i)) * 0x1.99999ap-6f));
                l_thin = coax_p_fmax(0x1.1eb852p-5f, ((l_ground_aa / 0x1.800000p2f) * 0x1.800000p0f));
                l_ripple = ((coax_p_stepf(abs((coax_p_fract(l_ph) - 0x1.000000p-1f)), l_thin) * coax_p_stepf(0x1.000000p-1f, coax_p_fairy_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f2(l_xz, coax_p_v2(0x1.70a3d8p-4f, 0x1.0a3d70p-3f)), coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_drift, coax_at_u(i32(2i))), 0x1.47ae14p-5f))))) * coax_p_stepf(0x1.1eb852p-3f, l_depth));
                l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_wg_mix_f3f3f(l_w, coax_p_fairy_pal(0x1.400000p3f), 0x1.ccccccp-2f), (l_ripple * (0x1.000000p0f - coax_p_smoothstep(0x1.e00000p4f, 0x1.180000p6f, coax_p_wg_length3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))))))));
                l_n = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
              }
              else
              {
                if ((l_look == 7u))
                {
                  l_keep = true;
                  l_n = l_face_n;
                }
                else
                {
                  if ((l_look != 10u))
                  {
                    l_n = l_face_n;
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if ((l_look == 6u))
  {
    l_from_hem = ((l_s.f5.y + 0x1.000000p-1f) * l_s.f6.x);
    l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_fairy_pal(0x1.000000p3f), coax_p_verse_style_flat_hem(l_from_hem, (0x1.eb851ep-5f * 0x1.000000p-1f), coax_p_fmax((l_local_aa * l_s.f6.x), 0x1.0624dep-10f)));
  }
  if (((l_s.f6.y > 0.0f) && (l_look < 99u)))
  {
    l_t = coax_p_verse_style_flat_dither((0x1.000000p-1f - l_s.f5.y), l_s.x.x, l_s.x.y, 0x1.000000p2f);
    l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(l_base, coax_p_fairy_pal(0x1.800000p2f)), 0x1.99999ap-1f), ((l_t * 0x1.000000p-2f) * l_s.f6.y));
  }
  l_r = coax_p_fairy_restored(l_xz);
  if ((!l_keep))
  {
    l_base = coax_p_fairy_faded(l_base, l_r.x);
  }
  l_c = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_share = 0.0f;
  if ((l_glow > 0.0f))
  {
    l_c = coax_p_wg_mul_f3f(l_base, select(0x1.4cccccp0f, 0x1.000000p0f, (l_glow < 0x1.000000p0f)));
    if ((l_glow < 0x1.000000p-1f))
    {
      l_c = coax_p_wg_mul_f3f(l_base, coax_p_mix(0x1.b851ecp-1f, 0x1.0a3d70p0f, coax_p_verse_style_facet_tone(coax_p_wg_dot3(l_n, coax_p_fairy_sun_dir()), 0x1.000000p0f)));
    }
    if ((!l_keep))
    {
      l_c = coax_p_wg_mul_f3f(l_c, coax_p_mix(0x1.666666p-2f, 0x1.000000p0f, l_r.x));
    }
    l_share = select(select(0x1.ccccccp-2f, 0x1.eb851ep-5f, (l_glow < 0x1.000000p-1f)), 0x1.800000p-1f, (l_glow >= 0x1.000000p0f));
  }
  else
  {
    l_c = coax_p_fairy_painted(l_base, l_n, l_s.y, l_shadow, l_character);
  }
  l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_fairy_pal(0x1.000000p3f), l_r.y), 0x1.333334p-1f));
  l_share = coax_p_fmax(l_share, (l_r.y * 0x1.000000p-1f));
  l_c = coax_p_fairy_hazed(l_c, l_s.y);
  l_dist = coax_p_wg_length3(coax_p_wg_sub_f3f3(l_s.y, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)))));
  return coax_t19(coax_p_v4(l_c.x, l_c.y, l_c.z, l_share), coax_p_v4(l_n.x, l_n.y, l_n.z, select((-l_dist), l_dist, ((l_edge > 0x1.000000p-1f) && (l_look != 11u)))));
}
fn coax_p_water_transmit_fs(p_i: coax_t15) -> coax_t19 {
  var l_i: coax_t15 = p_i;
  var l_s: coax_t22;
  l_s = coax_p_fairy_water_fragment(l_i);
  return coax_t19(coax_p_v4(l_s.y.x, l_s.y.y, l_s.y.z, 0x1.000000p0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_water_emit_fs(p_i: coax_t15) -> coax_t19 {
  var l_i: coax_t15 = p_i;
  var l_s: coax_t22;
  l_s = coax_p_fairy_water_fragment(l_i);
  return coax_t19(coax_p_v4(l_s.x.x, l_s.x.y, l_s.x.z, 0.0f), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
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
  var l_p: coax_t25;
  var l_uv: vec2f;
  var l_light: vec3f;
  var l_near: vec3f;
  var l_far: vec3f;
  var l_c: vec3f;
  var l_own: f32;
  var l_hi: coax_t25;
  var l_e: f32;
  l_p = coax_p_wg_cvt_i2f2(coax_p_v2(l_pos.x, l_pos.y));
  l_uv = coax_p_wg_div_f2f2(coax_p_v2(l_pos.x, l_pos.y), coax_p_wg_at2(0i));
  l_light = coax_p_wg_sw_rgb_f4(coax_p_wg_load_2d(l_p, 0i));
  l_near = coax_p_wg_sw_rgb_f4(textureSampleLevel(post_quarter, post_lin, l_uv, 0.0f));
  l_far = coax_p_wg_sw_rgb_f4(textureSampleLevel(post_eighth, post_lin, l_uv, 0.0f));
  l_c = coax_p_wg_add_f3f3(l_light, coax_p_wg_mul_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_near, 0x1.19999ap-1f), coax_p_wg_mul_f3f(l_far, 0x1.ccccccp-2f)), 0x1.333334p-1f));
  l_own = (coax_p_wg_load_2d_1(l_p, 0i)).w;
  if (((l_own > 0.0f) && (l_own < 0x1.180000p7f)))
  {
    l_hi = coax_p_wg_sub_i2i2(coax_p_wg_cvt_i2f2(coax_p_wg_at2(0i)), coax_p_i2(1i, 1i));
    l_e = coax_p_verse_style_facet_edge(l_own, (coax_p_wg_load_2d_1(coax_p_wg_clamp_i2i2i2(coax_p_wg_add_i2i2(l_p, coax_p_i2(1i, 0i)), coax_p_i2(0i, 0i), l_hi), 0i)).w);
    l_e = coax_p_fmax(l_e, coax_p_verse_style_facet_edge(l_own, (coax_p_wg_load_2d_1(coax_p_wg_clamp_i2i2i2(coax_p_wg_sub_i2i2(l_p, coax_p_i2(1i, 0i)), coax_p_i2(0i, 0i), l_hi), 0i)).w));
    l_e = coax_p_fmax(l_e, coax_p_verse_style_facet_edge(l_own, (coax_p_wg_load_2d_1(coax_p_wg_clamp_i2i2i2(coax_p_wg_add_i2i2(l_p, coax_p_i2(0i, 1i)), coax_p_i2(0i, 0i), l_hi), 0i)).w));
    l_e = coax_p_fmax(l_e, coax_p_verse_style_facet_edge(l_own, (coax_p_wg_load_2d_1(coax_p_wg_clamp_i2i2i2(coax_p_wg_sub_i2i2(l_p, coax_p_i2(0i, 1i)), coax_p_i2(0i, 0i), l_hi), 0i)).w));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_fairy_tp(0x1.000000p3f), (((l_e * 0x1.b33334p-1f) * (0x1.000000p0f - coax_p_smoothstep(0x1.e00000p5f, 0x1.180000p7f, l_own))) * (0x1.000000p0f - (coax_at_u(i32(792i)) * 0x1.333334p-1f))));
  }
  return coax_p_wg_make_f4_f3f(coax_p_fairy_plates(coax_p_v2(l_pos.x, l_pos.y), coax_p_fairy_encode(l_c)), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t18) -> vec4f {
  var l_v: coax_t18 = p_v;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  if ((l_v.w.w < (-0x1.000000p-1f)))
  {
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), l_v.z.z), 0x1.000000p0f);
  }
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
fn coax_p_fairy_quad(p_v: u32) -> vec2f {
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
fn coax_p_fairy_height(p_x: i32, p_z: i32) -> f32 {
  var l_x: i32 = p_x;
  var l_z: i32 = p_z;
  var l_n: i32;
  var l_cx: i32;
  var l_cz: i32;
  l_n = coax_p_wg_i32_f(coax_at_u(i32(32i)));
  l_cx = coax_p_wg_clamp_i(l_x, 0i, l_n);
  l_cz = coax_p_wg_clamp_i(l_z, 0i, coax_p_wg_i32_f(coax_at_u(i32(238i))));
  return heights[i32(u32(((l_cz * (l_n + 1i)) + l_cx)))];
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_normalize3(l_a);
}
fn coax_p_fairy_clip(p_w: vec3f) -> vec4f {
  var l_w: vec3f = p_w;
  var l_p: vec4f;
  l_p = coax_p_v4(l_w.x, l_w.y, l_w.z, 0x1.000000p0f);
  return coax_p_v4(coax_p_wg_dot4(coax_p_wg_at4(8i), l_p), coax_p_wg_dot4(coax_p_wg_at4(12i), l_p), coax_p_wg_dot4(coax_p_wg_at4(16i), l_p), coax_p_wg_dot4(coax_p_wg_at4(20i), l_p));
}
fn coax_p_fairy_path_planes(p_x: i32, p_z: i32) -> vec2f {
  var l_x: i32 = p_x;
  var l_z: i32 = p_z;
  var l_i: u32;
  var l_p: u32;
  l_i = u32(((l_z * (coax_p_wg_i32_f(coax_at_u(i32(32i))) + 1i)) + l_x));
  l_p = coax_p_wg_u32_f(coax_at_u(i32(239i)));
  return coax_p_v2(heights[i32((l_p + l_i))], heights[i32(((2u * l_p) + l_i))]);
}
fn coax_p_wg_make_f3_f2f(p_p0: vec2f, p_p1: f32) -> vec3f {
  var l_p0: vec2f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v3(l_p0.x, l_p0.y, l_p1);
}
fn coax_p_fairy_box_vertex(p_id: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_f: u32;
  var l_c: vec2f;
  var l_n: vec3f;
  var l_a: vec3f;
  var l_b: vec3f;
  var l__sw2: u32;
  l_f = (l_id / 6u);
  l_c = coax_p_wg_sub_f2f2(coax_p_fairy_quad((l_id % 6u)), coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f));
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
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_fairy_sphere_vertex(p_id: u32, p_nudge: f32, p_seed: f32) -> coax_t14 {
  var l_id: u32 = p_id;
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
  if ((((l_q % 16u) >= 8u) || ((l_q / 16u) >= 5u)))
  {
    return coax_p_fairy_nothing();
  }
  l_c = coax_p_fairy_quad((l_id % 6u));
  l_i = (f32((l_q % 16u)) + l_c.x);
  l_j = (f32((l_q / 16u)) + l_c.y);
  l_lon = ((l_i / 0x1.000000p3f) * 0x1.921fb6p2f);
  l_lat = ((l_j / 0x1.400000p2f) * 0x1.921fb6p1f);
  l_n = coax_p_v3((sin(l_lat) * cos(l_lon)), cos(l_lat), (sin(l_lat) * sin(l_lon)));
  return coax_t14(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_n, 0x1.000000p-1f), (0x1.000000p0f + (l_nudge * coax_p_verse_style_facet_jitter(l_i, l_j, 0x1.000000p3f, 0x1.400000p2f, l_seed)))), l_n);
}
fn coax_p_fairy_cylinder_vertex(p_id: u32) -> coax_t14 {
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
  if ((l_id < 96u))
  {
    if (((l_id / 6u) >= 8u))
    {
      return coax_p_fairy_nothing();
    }
    l_c = coax_p_fairy_quad((l_id % 6u));
    l_a = (((f32((l_id / 6u)) + l_c.x) / 0x1.000000p3f) * 0x1.921fb6p2f);
    l_n = coax_p_v3(cos(l_a), 0.0f, sin(l_a));
    return coax_t14(coax_p_v3((l_n.x * 0x1.000000p-1f), (l_c.y - 0x1.000000p-1f), (l_n.z * 0x1.000000p-1f)), l_n);
  }
  l_t = ((l_id - 96u) / 3u);
  l_k = ((l_id - 96u) % 3u);
  if (((l_t % 16u) >= 8u))
  {
    return coax_p_fairy_nothing();
  }
  l_top = (l_t < 16u);
  l_y = select((-0x1.000000p-1f), 0x1.000000p-1f, l_top);
  l_n_8 = coax_p_v3(0.0f, select((-0x1.000000p0f), 0x1.000000p0f, l_top), 0.0f);
  if ((l_k == 0u))
  {
    return coax_t14(coax_p_v3(0.0f, l_y, 0.0f), l_n_8);
  }
  l_a_9 = (((f32((l_t % 16u)) + f32((l_k - 1u))) / 0x1.000000p3f) * 0x1.921fb6p2f);
  return coax_t14(coax_p_v3((cos(l_a_9) * 0x1.000000p-1f), l_y, (sin(l_a_9) * 0x1.000000p-1f)), l_n_8);
}
fn coax_p_fairy_cone_vertex(p_id: u32) -> coax_t14 {
  var l_id: u32 = p_id;
  var l_t: u32;
  var l_k: u32;
  var l_a: f32;
  var l_n: vec3f;
  var l_a_5: f32;
  l_t = ((l_id % 48u) / 3u);
  l_k = (l_id % 3u);
  if ((l_t >= 8u))
  {
    return coax_p_fairy_nothing();
  }
  if ((l_id < 48u))
  {
    if ((l_k == 0u))
    {
      return coax_t14(coax_p_v3(0.0f, 0x1.000000p-1f, 0.0f), coax_p_v3(0.0f, 0x1.000000p0f, 0.0f));
    }
    l_a = (((f32(l_t) + f32((l_k - 1u))) / 0x1.000000p3f) * 0x1.921fb6p2f);
    return coax_t14(coax_p_v3((cos(l_a) * 0x1.000000p-1f), (-0x1.000000p-1f), (sin(l_a) * 0x1.000000p-1f)), coax_p_wg_normalize3(coax_p_v3(cos(l_a), 0x1.000000p-1f, sin(l_a))));
  }
  l_n = coax_p_v3(0.0f, (-0x1.000000p0f), 0.0f);
  if ((l_k == 0u))
  {
    return coax_t14(coax_p_v3(0.0f, (-0x1.000000p-1f), 0.0f), l_n);
  }
  l_a_5 = (((f32(l_t) + f32((l_k - 1u))) / 0x1.000000p3f) * 0x1.921fb6p2f);
  return coax_t14(coax_p_v3((cos(l_a_5) * 0x1.000000p-1f), (-0x1.000000p-1f), (sin(l_a_5) * 0x1.000000p-1f)), l_n);
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
fn coax_p_FAIRY_WIND() -> vec2f {
  return coax_p_v2(0x1.99999ap-1f, 0x1.333334p-1f);
}
fn coax_p_fairy_wind_bend(p_xz: vec2f) -> f32 {
  var l_xz: vec2f = p_xz;
  var l_t: f32;
  var l_k: f32;
  var l_along: f32;
  var l_gust: f32;
  l_t = (coax_at_u(i32(2i)) * (0x1.000000p0f - coax_at_u(i32(4i))));
  l_k = (0x1.921fb6p2f / 0x1.800000p3f);
  l_along = coax_p_wg_dot2(l_xz, coax_p_FAIRY_WIND());
  l_gust = coax_p_fairy_noise(coax_p_wg_sub_f2f2(coax_p_wg_div_f2f(l_xz, 0x1.900000p4f), coax_p_wg_div_f2f(coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(coax_p_FAIRY_WIND(), l_t), 0x1.800000p1f), 0x1.900000p4f)));
  return (0x1.333334p-2f * ((0x1.333334p-1f * sin(((l_k * l_along) - ((l_k * 0x1.800000p1f) * l_t)))) + (0x1.99999ap-2f * ((l_gust * 0x1.000000p1f) - 0x1.000000p0f))));
}
fn coax_p_fairy_snap(p_c: vec3f, p_k: f32) -> vec3f {
  var l_c: vec3f = p_c;
  var l_k: f32 = p_k;
  var l_q: vec3f;
  var l_b: vec4f;
  l_q = coax_p_wg_sqrt_f3(coax_p_wg_max_f3f3(l_c, coax_p_v3(0.0f, 0.0f, 0.0f)));
  l_b = coax_p_v4(0x1.000000p1f, 0x1.dcd650p29f, 0.0f, 0.0f);
  l_b = coax_p_fairy_near(l_q, 0x1.000000p1f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.800000p1f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.000000p2f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.200000p3f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.400000p3f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.600000p3f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.800000p3f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.a00000p3f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.c00000p3f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.e00000p3f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.000000p4f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.100000p4f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.200000p4f, l_b);
  l_b = coax_p_fairy_near(l_q, 0x1.300000p4f, l_b);
  return coax_p_wg_mix_f3f3f(l_c, coax_p_fairy_pal(l_b.x), l_k);
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_fairy_water_rest(p_uv: vec2f) -> coax_t16 {
  var l_uv: vec2f = p_uv;
  var l_cols: u32;
  var l_rows: u32;
  var l_a: vec2f;
  var l_i: u32;
  var l_j: u32;
  var l_f: vec2f;
  var l_k00: u32;
  var l_k10: u32;
  var l_k01: u32;
  var l_k11: u32;
  var l_o: coax_t16;
  var l_v: array<f32, 8>;
  var l_c: u32;
  l_cols = coax_p_wg_u32_f(coax_at_u(i32(780i)));
  l_rows = coax_p_wg_u32_f(coax_at_u(i32(781i)));
  l_a = coax_p_wg_clamp_f2f2f2(l_uv, coax_p_v2(0.0f, 0.0f), coax_p_v2(f32((l_cols - 1u)), f32((l_rows - 1u))));
  l_i = coax_p_wg_min_u(coax_p_wg_u32_f(floor(l_a.x)), (l_cols - 2u));
  l_j = coax_p_wg_min_u(coax_p_wg_u32_f(floor(l_a.y)), (l_rows - 2u));
  l_f = coax_p_wg_sub_f2f2(l_a, coax_p_v2(f32(l_i), f32(l_j)));
  l_k00 = ((l_j * l_cols) + l_i);
  l_k10 = (l_k00 + 1u);
  l_k01 = (l_k00 + l_cols);
  l_k11 = (l_k01 + 1u);
  l_o = coax_t16();
  l_v = array<f32, 8>();
  {
    l_c = 0u;
    while ((l_c < 8u))
    {
      l_v[i32(l_c)] = coax_p_mix(coax_p_mix(coax_p_fairy_water_node(l_k00, l_c), coax_p_fairy_water_node(l_k10, l_c), l_f.x), coax_p_mix(coax_p_fairy_water_node(l_k01, l_c), coax_p_fairy_water_node(l_k11, l_c), l_f.x), l_f.y);
      l_c = (l_c + 1u);
    }
  }
  l_o.x = coax_p_v3(l_v[0i], l_v[1i], l_v[2i]);
  l_o.y = l_v[3i];
  l_o.z = l_v[4i];
  l_o.w = coax_p_v2(l_v[5i], l_v[6i]);
  l_o.f4 = l_v[7i];
  return l_o;
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
fn coax_p_fairy_water_gerstner(p_p0: vec2f, p_depth: f32, p_scale: f32) -> vec4f {
  var l_p0: vec2f = p_p0;
  var l_depth: f32 = p_depth;
  var l_scale: f32 = p_scale;
  var l_d: vec3f;
  var l_squeeze: f32;
  var l_count: i32;
  var l_gain: f32;
  var l_i: i32;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_k: f32;
  var l_amp: f32;
  var l_theta: f32;
  var l_co: f32;
  var l_s: f32;
  l_d = coax_p_v3(0.0f, 0.0f, 0.0f);
  l_squeeze = 0.0f;
  l_count = coax_p_wg_i32_f(coax_at_u(i32(759i)));
  l_gain = (coax_at_u(i32(769i)) * l_scale);
  {
    l_i = 0i;
    while ((l_i < 8i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_a = coax_p_wg_at4(((i32((l_i * 2i)) * 4i) + 468i));
      l_c = coax_p_wg_at4(((i32(((l_i * 2i) + 1i)) * 4i) + 468i));
      l_k = l_a.z;
      l_amp = ((l_a.w * l_gain) * coax_p_fairy_water_shoal(l_k, l_depth));
      l_theta = ((l_k * coax_p_wg_dot2(coax_p_v2(l_a.x, l_a.y), l_p0)) + l_c.y);
      l_co = cos(l_theta);
      l_s = sin(l_theta);
      l_d.x = (l_d.x + (((l_c.x * l_amp) * l_a.x) * l_co));
      l_d.z = (l_d.z + (((l_c.x * l_amp) * l_a.y) * l_co));
      l_d.y = (l_d.y + (l_amp * l_s));
      l_squeeze = (l_squeeze + (((l_c.x * l_amp) * l_k) * l_s));
      l_i = (l_i + 1i);
    }
  }
  return coax_p_v4(l_d.x, l_d.y, l_d.z, l_squeeze);
}
fn coax_p_wg_make_f4_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec4f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p1.x, l_p1.y);
}
fn coax_p_fairy_ray(p_pos: vec2f) -> vec3f {
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
fn coax_p_fairy_sky_colour(p_d: vec3f) -> vec3f {
  var l_d: vec3f = p_d;
  var l_up: f32;
  var l_c: vec3f;
  var l_cs: f32;
  var l_q: vec3f;
  var l_cell: vec3f;
  var l_h: f32;
  var l_point: f32;
  l_up = l_d.y;
  l_c = coax_p_wg_mix_f3f3f(coax_p_fairy_tp(0.0f), coax_p_fairy_tp(0x1.000000p0f), coax_p_verse_style_flat_sky(l_up));
  l_cs = coax_p_wg_dot3(l_d, coax_p_fairy_sun_dir());
  if ((l_cs > 0x1.ffd370p-1f))
  {
    l_c = coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(coax_p_fairy_pal(0x1.400000p3f), 0x1.19999ap0f), coax_p_wg_mul_f3f(coax_p_fairy_pal(0x1.400000p3f), 0x1.99999ap-1f), coax_at_u(i32(792i)));
  }
  if (((coax_at_u(i32(792i)) > 0.0f) && (l_up > 0x1.47ae14p-6f)))
  {
    l_q = coax_p_wg_mul_f3f(coax_p_wg_div_f3f(l_d, coax_p_fmax(abs(l_d.x), coax_p_fmax(abs(l_d.y), abs(l_d.z)))), 0x1.400000p7f);
    l_cell = coax_p_wg_floor_f3(l_q);
    l_h = coax_p_fairy_hash(coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(coax_p_v2(l_cell.x, l_cell.y), coax_p_wg_mul_f2f(coax_p_v2(l_cell.y, l_cell.z), 0x1.c851ecp2f)), coax_p_wg_mul_f2f(coax_p_v2(l_cell.z, l_cell.x), 0x1.dae148p1f)));
    l_point = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-4f, 0x1.99999ap-3f, coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_wg_fract_f3(l_q), coax_p_v3(0x1.000000p-1f, 0x1.000000p-1f, 0x1.000000p-1f)))));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(coax_p_fairy_pal(0x1.400000p3f), 0x1.99999ap-1f), ((l_point * coax_p_stepf(0x1.fae148p-1f, l_h)) * coax_at_u(i32(792i))));
  }
  return l_c;
}
fn coax_p_fairy_sun_dir() -> vec3f {
  return coax_p_wg_normalize3(coax_p_v3(coax_at_u(i32(28i)), coax_at_u(i32(29i)), coax_at_u(i32(30i))));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return dot(l_a, l_b);
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
fn coax_p_wg_dpdx_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(dpdx(l_p0.x), dpdx(l_p0.y), dpdx(l_p0.z));
}
fn coax_p_wg_dpdy_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(dpdy(l_p0.x), dpdy(l_p0.y), dpdy(l_p0.z));
}
fn coax_p_wg_neg_f3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_v3((-l_a.x), (-l_a.y), (-l_a.z));
}
fn coax_p_wg_sub_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_cvt_u2f2(p_p: vec2f) -> coax_t20 {
  var l_p: vec2f = p_p;
  return coax_p_u2(coax_p_wg_u32_f(l_p.x), coax_p_wg_u32_f(l_p.y));
}
fn coax_p_wg_mod_u2u(p_a: coax_t20, p_b: u32) -> coax_t20 {
  var l_a: coax_t20 = p_a;
  var l_b: u32 = p_b;
  return coax_p_u2((l_a.x % l_b), (l_a.y % l_b));
}
fn coax_p_fairy_ground_colour(p_xz: vec2f, p_aa: f32, p_paths: vec2f) -> vec3f {
  var l_xz: vec2f = p_xz;
  var l_aa: f32 = p_aa;
  var l_paths: vec2f = p_paths;
  var l_base: vec3f;
  var l_grey: vec3f;
  l_base = coax_p_wg_mix_f3f3f(coax_p_fairy_pal(0x1.000000p1f), coax_p_fairy_pal(0x1.800000p1f), coax_p_stepf(0x1.28f5c2p-1f, coax_p_fairy_noise(coax_p_wg_mul_f2f(l_xz, 0x1.1eb852p-5f))));
  l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_fairy_pal(0x1.000000p2f), (0x1.000000p0f - coax_p_smoothstep((-l_aa), l_aa, l_paths.x)));
  l_grey = coax_p_wg_mul_f3f(coax_p_wg_make_f3_f(coax_p_wg_dot3(coax_p_fairy_pal(0x1.000000p2f), coax_p_v3(0x1.333334p-2f, 0x1.19999ap-1f, 0x1.333334p-3f))), 0x1.8f5c28p-1f);
  l_base = coax_p_wg_mix_f3f3f(l_base, l_grey, (0x1.000000p0f - coax_p_smoothstep((-l_aa), l_aa, l_paths.y)));
  return l_base;
}
fn coax_p_fairy_pal(p_slot: f32) -> vec3f {
  var l_slot: f32 = p_slot;
  return coax_p_v3(coax_p_verse_style_flat_colour(0x1.000000p1f, l_slot, 0.0f), coax_p_verse_style_flat_colour(0x1.000000p1f, l_slot, 0x1.000000p0f), coax_p_verse_style_flat_colour(0x1.000000p1f, l_slot, 0x1.000000p1f));
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
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_fairy_blob_shadows(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_s: f32;
  var l_sd: vec3f;
  var l_flat_len: f32;
  var l_away: vec2f;
  var l_stretch: f32;
  var l_i: i32;
  var l_b: vec4f;
  var l_q: vec2f;
  var l_a: f32;
  var l_c: f32;
  l_s = 0.0f;
  l_sd = coax_p_fairy_sun_dir();
  l_flat_len = coax_p_wg_length2(coax_p_v2(l_sd.x, l_sd.z));
  l_away = select(coax_p_v2(0.0f, 0.0f), coax_p_wg_div_f2f(coax_p_wg_neg_f2(coax_p_v2(l_sd.x, l_sd.z)), coax_p_fmax(l_flat_len, 0x1.a36e2ep-14f)), (l_flat_len > 0x1.0624dep-10f));
  l_stretch = coax_p_clampf((((l_flat_len / coax_p_fmax(l_sd.y, 0x1.99999ap-5f)) * 0x1.19999ap0f) + 0x1.000000p0f), 0x1.000000p0f, 0x1.19999ap1f);
  {
    l_i = 0i;
    while ((l_i < 16i))
    {
      l_b = coax_p_wg_at4(((i32(l_i) * 4i) + 44i));
      if ((l_b.z <= 0.0f))
      {
        break;
      }
      l_q = coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_b.x, l_b.y));
      if ((coax_p_wg_dot2(l_away, l_away) > 0x1.0c6f7ap-20f))
      {
        l_a = (coax_p_wg_dot2(l_q, l_away) - ((l_b.z * (l_stretch - 0x1.000000p0f)) * 0x1.99999ap-1f));
        l_c = coax_p_wg_dot2(l_q, coax_p_v2((-l_away.y), l_away.x));
        l_q = coax_p_v2((l_a / l_stretch), l_c);
      }
      l_s = coax_p_fmax(l_s, (coax_p_verse_style_flat_shadow((coax_p_wg_length2(l_q) / l_b.z)) * coax_p_fmin((l_b.w * 0x1.99999ap0f), 0x1.000000p0f)));
      l_i = (l_i + 1i);
    }
  }
  return (0x1.000000p0f - l_s);
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_clampf(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return coax_p_fmin(coax_p_fmax(l_x, l_lo), l_hi);
}
fn coax_p_wg_atan2(p_y: f32, p_x: f32) -> f32 {
  var l_y: f32 = p_y;
  var l_x: f32 = p_x;
  var l_ax: f32;
  var l_ay: f32;
  var l_hi: f32;
  var l_r: f32;
  l_ax = abs(l_x);
  l_ay = abs(l_y);
  l_hi = coax_p_fmax(l_ax, l_ay);
  if ((l_hi == 0.0f))
  {
    return 0.0f;
  }
  l_r = coax_p_wg_atan01((coax_p_fmin(l_ax, l_ay) / l_hi));
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
fn coax_p_fairy_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_fairy_hash(l_i), coax_p_fairy_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_fairy_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_fairy_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
}
fn coax_p_fairy_ground_h(p_xz: vec2f) -> f32 {
  var l_xz: vec2f = p_xz;
  var l_n: f32;
  var l_g: vec2f;
  var l_i: i32;
  var l_j: i32;
  var l_f: vec2f;
  var l_h00: f32;
  var l_h10: f32;
  var l_h01: f32;
  var l_h11: f32;
  l_n = coax_at_u(i32(32i));
  l_g = coax_p_wg_clamp_f2f2f2(coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(l_xz, coax_p_v2(coax_at_u(i32(34i)), coax_at_u(i32(35i)))), coax_at_u(i32(33i))), coax_p_v2(0.0f, 0.0f), coax_p_wg_sub_f2f2(coax_p_v2(l_n, coax_at_u(i32(238i))), coax_p_v2(0x1.a36e2ep-14f, 0x1.a36e2ep-14f)));
  l_i = coax_p_wg_i32_f(floor(l_g.x));
  l_j = coax_p_wg_i32_f(floor(l_g.y));
  l_f = coax_p_wg_sub_f2f2(l_g, coax_p_v2(f32(l_i), f32(l_j)));
  l_h00 = coax_p_fairy_height(l_i, l_j);
  l_h10 = coax_p_fairy_height((l_i + 1i), l_j);
  l_h01 = coax_p_fairy_height(l_i, (l_j + 1i));
  l_h11 = coax_p_fairy_height((l_i + 1i), (l_j + 1i));
  if ((l_f.x >= l_f.y))
  {
    return ((l_h00 + ((l_h10 - l_h00) * l_f.x)) + ((l_h11 - l_h10) * l_f.y));
  }
  return ((l_h00 + ((l_h11 - l_h01) * l_f.x)) + ((l_h01 - l_h00) * l_f.y));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_verse_style_flat_hem(p_fl_y: f32, p_fl_width: f32, p_fl_aa: f32) -> f32 {
  var l_fl_y: f32 = p_fl_y;
  var l_fl_width: f32 = p_fl_width;
  var l_fl_aa: f32 = p_fl_aa;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(l_fl_width, coax_p_st_max(l_fl_aa, 0x1.0c6f7ap-20f), l_fl_y));
}
fn coax_p_verse_style_flat_dither(p_fl_t: f32, p_fl_px: f32, p_fl_py: f32, p_fl_steps: f32) -> f32 {
  var l_fl_t: f32 = p_fl_t;
  var l_fl_px: f32 = p_fl_px;
  var l_fl_py: f32 = p_fl_py;
  var l_fl_steps: f32 = p_fl_steps;
  var l_fl_n: f32;
  l_fl_n = coax_p_st_max(l_fl_steps, 0x1.000000p0f);
  return coax_p_st_sat((floor(((coax_p_st_sat(l_fl_t) * l_fl_n) + coax_p_verse_style_bayer(l_fl_px, l_fl_py))) / l_fl_n));
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_fairy_restored(p_xz: vec2f) -> vec2f {
  var l_xz: vec2f = p_xz;
  var l_front: f32;
  var l_count: i32;
  var l_faded: f32;
  var l_i: i32;
  var l_z: vec4f;
  var l_best: f32;
  var l_i_8: i32;
  var l_b: vec4f;
  var l_d: f32;
  var l_edge: f32;
  l_front = 0.0f;
  l_count = coax_p_wg_i32_f(coax_at_u(i32(336i)));
  l_faded = 0.0f;
  {
    l_i = 0i;
    while ((l_i < 8i))
    {
      if ((l_i >= coax_p_wg_i32_f(coax_at_u(i32(337i)))))
      {
        break;
      }
      l_z = coax_p_wg_at4(((i32(l_i) * 4i) + 108i));
      l_faded = coax_p_fmax(l_faded, (0x1.000000p0f - coax_p_clampf(((coax_p_wg_length2(coax_p_wg_sub_f2f2(l_xz, coax_p_v2(l_z.x, l_z.y))) - (l_z.z - l_z.w)) / coax_p_fmax(l_z.w, 0x1.0624dep-10f)), 0.0f, 0x1.000000p0f)));
      l_i = (l_i + 1i);
    }
  }
  l_best = (0x1.000000p0f - l_faded);
  {
    l_i_8 = 0i;
    while ((l_i_8 < 24i))
    {
      if ((l_i_8 >= l_count))
      {
        break;
      }
      l_b = coax_p_wg_at4(((i32(l_i_8) * 4i) + 240i));
      if ((l_b.z <= 0.0f))
      {
        {
          l_i_8 = (l_i_8 + 1i);
          continue;
        }
      }
      l_d = coax_p_wg_length2(coax_p_wg_sub_f2f2(l_xz, coax_p_v2(l_b.x, l_b.y)));
      l_edge = coax_p_fmin(0x1.400000p2f, l_b.z);
      l_best = coax_p_fmax(l_best, (0x1.000000p0f - coax_p_clampf(((l_d - (l_b.z - l_edge)) / coax_p_fmax(l_edge, 0x1.0624dep-10f)), 0.0f, 0x1.000000p0f)));
      l_front = coax_p_fmax(l_front, (l_b.w * exp((-(((l_d - l_b.z) / 0x1.99999ap0f) * ((l_d - l_b.z) / 0x1.99999ap0f))))));
      l_i_8 = (l_i_8 + 1i);
    }
  }
  return coax_p_v2(l_best, l_front);
}
fn coax_p_fairy_faded(p_c: vec3f, p_r: f32) -> vec3f {
  var l_c: vec3f = p_c;
  var l_r: f32 = p_r;
  var l_l: f32;
  var l_grey: vec3f;
  if ((l_r >= 0x1.000000p0f))
  {
    return l_c;
  }
  l_l = coax_p_wg_dot3(l_c, coax_p_v3(0x1.333334p-2f, 0x1.19999ap-1f, 0x1.333334p-3f));
  l_grey = coax_p_wg_mul_f3f3(coax_p_v3(l_l, l_l, l_l), coax_p_v3(0x1.f5c290p-1f, 0x1.f0a3d8p-1f, 0x1.051eb8p0f));
  return coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(l_grey, l_c, 0x1.666666p-1f), 0x1.07ae14p0f), l_c, l_r);
}
fn coax_p_verse_style_facet_tone(p_fl_ndl: f32, p_fl_lit: f32) -> f32 {
  var l_fl_ndl: f32 = p_fl_ndl;
  var l_fl_lit: f32 = p_fl_lit;
  var l_fl_t: f32;
  l_fl_t = coax_p_st_sat((((((l_fl_ndl * 0x1.000000p-1f) + 0x1.000000p-1f) * coax_p_st_sat(l_fl_lit)) - 0x1.70a3d8p-3f) / 0x1.666666p-1f));
  return ((l_fl_t * l_fl_t) * (0x1.800000p1f - (0x1.000000p1f * l_fl_t)));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_fairy_painted(p_c: vec3f, p_n: vec3f, p_world: vec3f, p_shadow: f32, p_character: bool) -> vec3f {
  var l_c: vec3f = p_c;
  var l_n: vec3f = p_n;
  var l_world: vec3f = p_world;
  var l_shadow: f32 = p_shadow;
  var l_character: bool = p_character;
  var l_band: f32;
  var l_light: vec3f;
  var l_to: vec3f;
  var l_d2: f32;
  var l_reach: f32;
  var l_fall: f32;
  l_band = coax_p_verse_style_facet_tone(coax_p_wg_dot3(l_n, coax_p_fairy_sun_dir()), 0x1.000000p0f);
  if (l_character)
  {
    l_band = (0x1.000000p-2f + (0x1.800000p-1f * l_band));
  }
  l_light = coax_p_wg_mul_f3f3(l_c, coax_p_wg_mix_f3f3f(coax_p_fairy_tp(0x1.800000p2f), coax_p_fairy_tp(0x1.400000p2f), l_band));
  l_light = coax_p_wg_mul_f3f3(l_light, coax_p_wg_mix_f3f3f(coax_p_fairy_pal(0x1.c00000p2f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_shadow));
  if ((coax_at_u(i32(794i)) > 0.0f))
  {
    l_to = coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(43i)), coax_at_u(i32(795i)), coax_at_u(i32(7i))), l_world);
    l_d2 = coax_p_wg_dot3(l_to, l_to);
    l_reach = coax_p_fmax(coax_at_u(i32(794i)), 0x1.000000p0f);
    l_fall = (coax_p_fmin(coax_at_u(i32(794i)), 0x1.000000p0f) * ((0x1.000000p-1f * coax_p_stepf(l_d2, ((0x1.200000p5f * l_reach) * l_reach))) + (0x1.000000p-1f * coax_p_stepf(l_d2, ((0x1.200000p3f * l_reach) * l_reach)))));
    l_light = coax_p_wg_add_f3f3(l_light, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(l_c, coax_p_fairy_pal(0x1.e00000p3f)), 0x1.99999ap-1f), l_fall));
  }
  return l_light;
}
fn coax_p_fairy_hazed(p_c: vec3f, p_world: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_world: vec3f = p_world;
  var l_d: f32;
  var l_backdrop: f32;
  l_d = coax_p_fmax((coax_p_wg_length3(coax_p_wg_sub_f3f3(l_world, coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))))) - coax_at_u(i32(36i))), 0.0f);
  l_backdrop = ((coax_p_smoothstep(0x1.000000p3f, 0x1.400000p5f, (l_world.y - coax_at_u(i32(41i)))) * coax_p_smoothstep(0x1.900000p5f, 0x1.180000p7f, l_d)) * 0x1.666666p-1f);
  return coax_p_wg_mix_f3f3f(l_c, coax_p_fairy_tp(0x1.000000p0f), coax_p_fmax(coax_p_verse_style_flat_fade(l_d, 0x1.180000p7f, 0x1.040000p8f, 0x1.666666p-2f), l_backdrop));
}
fn coax_p_fairy_water_fragment(p_i: coax_t15) -> coax_t22 {
  var l_i: coax_t15 = p_i;
  var l_o: coax_t22;
  var l_world: vec3f;
  var l_rest: vec2f;
  var l_depth: f32;
  var l_dist: f32;
  var l_w: vec3f;
  var l_body: vec3f;
  var l_r: vec2f;
  var l_shore: f32;
  var l_line: f32;
  var l_rings: f32;
  var l_foam: f32;
  var l_alpha: f32;
  var l_emit: vec3f;
  var l_hz: f32;
  var l_trans: vec3f;
  l_o = coax_t22();
  l_world = coax_p_v3(l_i.y.x, l_i.y.y, l_i.y.z);
  l_rest = coax_p_v2(l_i.z.x, l_i.z.y);
  l_depth = l_i.y.w;
  l_dist = coax_p_wg_length3(coax_p_wg_sub_f3f3(coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i))), l_world));
  if ((coax_at_u(i32(25i)) < l_world.y))
  {
    l_o.x = coax_p_v3(0.0f, 0.0f, 0.0f);
    l_o.y = coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f);
    return l_o;
  }
  l_w = coax_p_fairy_pal(0x1.a00000p3f);
  l_body = coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_w, coax_p_wg_mul_f3f(l_w, 0x1.8f5c28p-1f), coax_p_stepf(0x1.99999ap-2f, l_depth)), coax_p_wg_mul_f3f(l_w, 0x1.333334p-1f), coax_p_stepf(0x1.800000p0f, l_depth));
  l_body = coax_p_wg_mul_f3f3(l_body, coax_p_fairy_tp(0x1.400000p2f));
  l_r = coax_p_fairy_restored(coax_p_v2(l_world.x, l_world.z));
  l_body = coax_p_fairy_faded(l_body, l_r.x);
  l_shore = l_i.w.w;
  l_line = (coax_p_stepf(l_shore, 0x1.c28f5cp-3f) * coax_p_stepf(0.0f, l_depth));
  l_rings = coax_p_stepf(0x1.000000p-1f, (coax_p_fairy_water_ripple_foam(l_rest) * 0x1.99999ap0f));
  l_foam = coax_p_fmax(l_line, l_rings);
  l_alpha = ((0x1.333334p-1f + (0x1.666666p-2f * coax_p_stepf(0x1.99999ap-2f, l_depth))) * coax_p_smoothstep((-0x1.99999ap-5f), 0x1.99999ap-4f, l_depth));
  l_emit = coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(l_body, l_alpha), coax_p_wg_mul_f3f3(coax_p_fairy_pal(0x1.400000p3f), coax_p_fairy_tp(0x1.400000p2f)), l_foam);
  l_hz = coax_p_verse_style_flat_fade(coax_p_fmax((l_dist - coax_at_u(i32(36i))), 0.0f), 0x1.180000p7f, 0x1.040000p8f, 0x1.666666p-2f);
  l_trans = coax_p_wg_make_f3_f(((0x1.000000p0f - l_alpha) * (0x1.000000p0f - l_foam)));
  l_o.x = coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(l_emit, (0x1.000000p0f - l_hz)), coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f(coax_p_fairy_tp(0x1.000000p0f), l_hz), coax_p_wg_sub_f3f3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_trans)));
  l_o.y = l_trans;
  return l_o;
}
fn coax_p_wg_dims_2d() -> coax_t20 {
  return coax_p_u2(textureDimensions(bloom_src).x, textureDimensions(bloom_src).y);
}
fn coax_p_wg_cvt_f2u2(p_p: coax_t20) -> vec2f {
  var l_p: coax_t20 = p_p;
  return coax_p_v2(f32(l_p.x), f32(l_p.y));
}
fn coax_p_wg_div_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a / l_b.x), (l_a / l_b.y));
}
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
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
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t25 {
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
fn coax_p_wg_load_2d(p_c: coax_t25, p_level: i32) -> vec4f {
  var l_c: coax_t25 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_light, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_load_2d_1(p_c: coax_t25, p_level: i32) -> vec4f {
  var l_c: coax_t25 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(post_aux, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t25 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t25(l_x, l_y);
}
fn coax_p_wg_sub_i2i2(p_a: coax_t25, p_b: coax_t25) -> coax_t25 {
  var l_a: coax_t25 = p_a;
  var l_b: coax_t25 = p_b;
  return coax_p_i2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_add_i2i2(p_a: coax_t25, p_b: coax_t25) -> coax_t25 {
  var l_a: coax_t25 = p_a;
  var l_b: coax_t25 = p_b;
  return coax_p_i2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_wg_clamp_i2i2i2(p_p0: coax_t25, p_p1: coax_t25, p_p2: coax_t25) -> coax_t25 {
  var l_p0: coax_t25 = p_p0;
  var l_p1: coax_t25 = p_p1;
  var l_p2: coax_t25 = p_p2;
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
fn coax_p_fairy_tp(p_slot: f32) -> vec3f {
  var l_slot: f32 = p_slot;
  return coax_p_wg_mul_f3f3(coax_p_fairy_pal(l_slot), coax_p_fairy_by_time());
}
fn coax_p_fairy_encode(p_c: vec3f) -> vec3f {
  var l_c: vec3f = p_c;
  return coax_p_wg_pow_f3f3(coax_p_wg_clamp_f3f3f3(l_c, coax_p_v3(0.0f, 0.0f, 0.0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f)), coax_p_wg_make_f3_f((0x1.000000p0f / 0x1.19999ap1f)));
}
fn coax_p_fairy_plates(p_p: vec2f, p_under: vec3f) -> vec3f {
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
      l_c = coax_p_fairy_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 340i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 340i)));
      l_k = (l_k + 1i);
    }
  }
  return l_c;
}
fn coax_p_wg_clamp_i(p_x: i32, p_lo: i32, p_hi: i32) -> i32 {
  var l_x: i32 = p_x;
  var l_lo: i32 = p_lo;
  var l_hi: i32 = p_hi;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_lo), l_hi);
}
fn coax_p_normalize3(p_a: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  return coax_p_scale3(l_a, (0x1.000000p0f / coax_p_length3(l_a)));
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_fairy_nothing() -> coax_t14 {
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
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_max_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y), coax_p_fmax(l_p0.z, l_p1.z));
}
fn coax_p_wg_sqrt_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(sqrt(l_p0.x), sqrt(l_p0.y), sqrt(l_p0.z));
}
fn coax_p_fairy_near(p_q: vec3f, p_slot: f32, p_best: vec4f) -> vec4f {
  var l_q: vec3f = p_q;
  var l_slot: f32 = p_slot;
  var l_best: vec4f = p_best;
  var l_p: vec3f;
  var l_e: vec3f;
  var l_dd: f32;
  l_p = coax_p_fairy_pal(l_slot);
  l_e = coax_p_wg_sub_f3f3(coax_p_wg_sqrt_f3(l_p), l_q);
  l_dd = coax_p_wg_dot3(l_e, l_e);
  return select(l_best, coax_p_v4(l_slot, l_dd, 0.0f, 0.0f), (l_dd < l_best.y));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_wg_clamp_f2f2f2(p_p0: vec2f, p_p1: vec2f, p_p2: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  var l_p2: vec2f = p_p2;
  return coax_p_v2(coax_p_clampf(l_p0.x, l_p1.x, l_p2.x), coax_p_clampf(l_p0.y, l_p1.y, l_p2.y));
}
fn coax_p_wg_min_u(p_a: u32, p_b: u32) -> u32 {
  var l_a: u32 = p_a;
  var l_b: u32 = p_b;
  if ((l_b < l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_fairy_water_node(p_k: u32, p_f: u32) -> f32 {
  var l_k: u32 = p_k;
  var l_f: u32 = p_f;
  return water_nodes[i32(((l_k * 8u) + l_f))];
}
fn coax_p_fairy_water_shoal(p_k: f32, p_depth: f32) -> f32 {
  var l_k: f32 = p_k;
  var l_depth: f32 = p_depth;
  return sqrt(coax_p_wg_tanh(coax_p_clampf((l_k * l_depth), 0.0f, 0x1.400000p3f)));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_verse_style_flat_sky(p_fl_up: f32) -> f32 {
  var l_fl_up: f32 = p_fl_up;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.eb851ep-4f, 0x1.eb851ep-4f, l_fl_up));
}
fn coax_p_wg_floor_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(floor(l_p0.x), floor(l_p0.y), floor(l_p0.z));
}
fn coax_p_fairy_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
}
fn coax_p_wg_fract_f3(p_p0: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  return coax_p_v3(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y), coax_p_fract(l_p0.z));
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t20 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t20(l_x, l_y);
}
fn coax_p_wg_make_f3_f(p_p0: f32) -> vec3f {
  var l_p0: f32 = p_p0;
  return coax_p_v3(l_p0, l_p0, l_p0);
}
fn coax_p_verse_style_flat_colour(p_fl_realm: f32, p_fl_slot: f32, p_fl_ch: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_slot: f32 = p_fl_slot;
  var l_fl_ch: f32 = p_fl_ch;
  return coax_pow_bg_vs(coax_p_verse_style_flat_srgb(l_fl_realm, l_fl_slot, l_fl_ch), 0x1.19999ap1f);
}
fn coax_p_wg_neg_f2(p_a: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  return coax_p_v2((-l_a.x), (-l_a.y));
}
fn coax_p_verse_style_flat_shadow(p_fl_d: f32) -> f32 {
  var l_fl_d: f32 = p_fl_d;
  return (0x1.000000p0f - coax_p_verse_style_soft_step(0x1.b33334p-1f, 0x1.333334p-3f, l_fl_d));
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
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
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
fn coax_p_st_max(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
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
fn coax_p_verse_style_flat_fade(p_fl_d: f32, p_fl_start: f32, p_fl_reach: f32, p_fl_cap: f32) -> f32 {
  var l_fl_d: f32 = p_fl_d;
  var l_fl_start: f32 = p_fl_start;
  var l_fl_reach: f32 = p_fl_reach;
  var l_fl_cap: f32 = p_fl_cap;
  return (l_fl_cap * coax_p_verse_style_soft_step((l_fl_start + (l_fl_reach * 0x1.000000p-1f)), coax_p_st_max((l_fl_reach * 0x1.000000p-1f), 0x1.0624dep-10f), l_fl_d));
}
fn coax_p_fairy_water_ripple_foam(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_foam: f32;
  var l_t: f32;
  var l_count: i32;
  var l_i: i32;
  var l_r: vec4f;
  var l_age: f32;
  var l_x: f32;
  l_foam = 0.0f;
  l_t = coax_at_u(i32(756i));
  l_count = coax_p_wg_i32_f(coax_at_u(i32(758i)));
  {
    l_i = 0i;
    while ((l_i < 24i))
    {
      if ((l_i >= l_count))
      {
        break;
      }
      l_r = coax_p_wg_at4(((i32(l_i) * 4i) + 660i));
      l_age = (l_t - l_r.z);
      l_x = (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_r.x, l_r.y))) - (0x1.19999ap0f * l_age));
      l_foam = coax_p_fmax(l_foam, ((exp((((-l_x) * l_x) / 0x1.47ae14p-4f)) * exp(((-l_age) * 0x1.99999ap0f))) * coax_p_clampf((l_r.w * 0x1.900000p4f), 0.0f, 0x1.000000p0f)));
      l_i = (l_i + 1i);
    }
  }
  return l_foam;
}
fn coax_p_fairy_by_time() -> vec3f {
  return coax_p_v3(coax_p_verse_style_flat_time(0x1.000000p1f, 0.0f, coax_at_u(i32(793i)), coax_at_u(i32(792i))), coax_p_verse_style_flat_time(0x1.000000p1f, 0x1.000000p0f, coax_at_u(i32(793i)), coax_at_u(i32(792i))), coax_p_verse_style_flat_time(0x1.000000p1f, 0x1.000000p1f, coax_at_u(i32(793i)), coax_at_u(i32(792i))));
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
fn coax_p_fairy_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_half: vec2f;
  var l_corner: f32;
  var l_q: vec2f;
  var l_d: f32;
  var l_over: bool;
  var l_fill: vec3f;
  var l_line: vec3f;
  var l_w: f32;
  var l_inside: f32;
  var l_edge: f32;
  var l_a: f32;
  if ((l_s.x <= 0.0f))
  {
    return l_under;
  }
  l_half = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_r.z, l_r.w), coax_p_v2(l_r.x, l_r.y)), 0x1.000000p-1f);
  l_corner = (0x1.000000p3f * l_s.z);
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_d = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  if ((l_d > 0x1.000000p0f))
  {
    return l_under;
  }
  l_over = (l_s.y >= 0x1.000000p1f);
  l_fill = select(coax_p_v3(0x1.47ae14p-3f, 0x1.99999ap-3f, 0x1.147ae2p-2f), coax_p_v3(0x1.c28f5cp-3f, 0x1.147ae2p-2f, 0x1.70a3d8p-2f), l_over);
  l_line = select(coax_p_v3(0x1.8f5c28p-1f, 0x1.666666p-1f, 0x1.0a3d70p-1f), coax_p_v3(0x1.f5c290p-1f, 0x1.ae147ap-1f, 0x1.0a3d70p-1f), (l_over || (l_s.y >= 0x1.000000p0f)));
  l_w = (0x1.400000p0f * l_s.z);
  l_inside = coax_p_clampf((0x1.000000p-1f - l_d), 0.0f, 0x1.000000p0f);
  l_edge = coax_p_clampf((((l_w * 0x1.000000p-1f) + 0x1.000000p-1f) - abs((l_d + (l_w * 0x1.000000p-1f)))), 0.0f, 0x1.000000p0f);
  l_a = (coax_p_clampf(l_s.x, 0.0f, 0x1.000000p0f) * 0x1.ccccccp-1f);
  return coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_under, l_fill, (l_a * l_inside)), l_line, ((l_edge * coax_p_fmin(0x1.000000p0f, (l_a + 0x1.99999ap-4f))) * 0x1.666666p-1f));
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
fn coax_p_wg_tanh(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_e: f32;
  if ((l_x > 0x1.400000p3f))
  {
    return 0x1.000000p0f;
  }
  if ((l_x < (-0x1.400000p3f)))
  {
    return (-0x1.000000p0f);
  }
  l_e = exp((0x1.000000p1f * l_x));
  return ((l_e - 0x1.000000p0f) / (l_e + 0x1.000000p0f));
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
fn coax_p_st_min(p_a: f32, p_b: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_a < l_b))
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
fn coax_p_verse_style_flat_time(p_fl_realm: f32, p_fl_ch: f32, p_fl_dusk: f32, p_fl_night: f32) -> f32 {
  var l_fl_realm: f32 = p_fl_realm;
  var l_fl_ch: f32 = p_fl_ch;
  var l_fl_dusk: f32 = p_fl_dusk;
  var l_fl_night: f32 = p_fl_night;
  var l_fl_warm: f32;
  if ((l_fl_realm < 0x1.000000p-1f))
  {
    return 0x1.000000p0f;
  }
  l_fl_warm = (0x1.000000p0f + ((coax_p_st_pick(l_fl_ch, 0x1.000000p0f, 0x1.99999ap-1f, 0x1.5c28f6p-1f) - 0x1.000000p0f) * coax_p_st_sat(l_fl_dusk)));
  return (l_fl_warm + ((coax_p_st_pick(l_fl_ch, 0x1.99999ap-3f, 0x1.000000p-2f, 0x1.c28f5cp-2f) - l_fl_warm) * coax_p_st_sat(l_fl_night)));
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
fn water_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_water_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
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
fn scene_fs(coax_in: coax_io_13) -> coax_out_19 {
  let coax_r = coax_p_scene_fs(coax_t13(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_out_19(coax_r.x, coax_r.y);
}
@fragment
fn water_transmit_fs(coax_in: coax_io_15) -> coax_out_19 {
  let coax_r = coax_p_water_transmit_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
  return coax_out_19(coax_r.x, coax_r.y);
}
@fragment
fn water_emit_fs(coax_in: coax_io_15) -> coax_out_19 {
  let coax_r = coax_p_water_emit_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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