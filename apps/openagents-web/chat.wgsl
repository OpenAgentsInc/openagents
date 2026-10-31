// ---- generated: apps/openagents-web/gpu/chat.coax (the module) by coax wgsl; do not edit
struct coax_t9 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
  f5: vec4f,
  f6: vec4f,
}
struct coax_in_9 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
  @location(4) f4: vec4f,
  @location(5) f5: vec4f,
  @location(6) f6: vec4f,
}
struct coax_t11 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec2f,
  f4: vec2f,
  f5: vec4f,
  f6: vec4f,
  f7: vec4f,
  f8: vec4f,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) @interpolate(flat) y: vec2f,
  @location(1) @interpolate(flat) z: vec4f,
  @location(2) @interpolate(flat) w: vec2f,
  @location(3) f4: vec2f,
  @location(4) @interpolate(flat) f5: vec4f,
  @location(5) @interpolate(flat) f6: vec4f,
  @location(6) @interpolate(flat) f7: vec4f,
  @location(7) @interpolate(flat) f8: vec4f,
}
struct coax_t13 {
  x: i32,
  y: i32,
}
struct coax_t14 {
  x: u32,
  y: u32,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 1>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(0) @binding(1) var gpui_atlas: texture_2d<f32>;
@group(0) @binding(2) var gpui_color: texture_2d<f32>;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_v: coax_t9) -> coax_t11 {
  var l__vid2: u32 = p__vid2;
  var l__iid3: u32 = p__iid3;
  var l_v: coax_t9 = p_v;
  var l_o: coax_t11;
  l_o = coax_t11();
  l_o.x = coax_p_v4((((l_v.x.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_v.x.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_v.x.z, l_v.x.w);
  l_o.z = l_v.y;
  l_o.w = coax_p_v2(l_v.z.x, l_v.z.y);
  l_o.f4 = coax_p_v2(l_v.z.z, l_v.z.w);
  l_o.f5 = l_v.w;
  l_o.f6 = l_v.f4;
  l_o.f7 = l_v.f5;
  l_o.f8 = l_v.f6;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  return coax_p_v4((0x1.a00000p3f / 0x1.fe0000p7f), (0x1.a00000p3f / 0x1.fe0000p7f), (0x1.a00000p3f / 0x1.fe0000p7f), 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t11) -> vec4f {
  var l_v: coax_t11 = p_v;
  var l_p: vec2f;
  var l_mono: f32;
  var l_poly: vec4f;
  var l_kind: f32;
  var l_c: vec4f;
  l_p = coax_p_v2(l_v.x.x, l_v.x.y);
  if (((((l_p.x < l_v.f7.x) || (l_p.y < l_v.f7.y)) || (l_p.x >= l_v.f7.z)) || (l_p.y >= l_v.f7.w)))
  {
    discard;
  }
  l_mono = (coax_p_wg_load_2d(coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(coax_p_wg_mul_f2f2(l_v.f4, coax_p_wg_cvt_f2u2(coax_p_wg_dims_2d())))), 0i)).x;
  l_poly = coax_p_wg_load_2d_1(coax_p_wg_cvt_i2f2(coax_p_wg_floor_f2(coax_p_wg_mul_f2f2(l_v.f4, coax_p_wg_cvt_f2u2(coax_p_wg_dims_2d_1())))), 0i);
  l_kind = l_v.y.x;
  l_c = vec4f();
  if ((l_kind > 0x1.600000p2f))
  {
    l_c = coax_p_chat_inset_color(l_p, l_v.z, l_v.w.x, l_v.y.y, l_v.f5);
  }
  else
  {
    if ((l_kind > 0x1.200000p2f))
    {
      l_c = coax_p_chat_gradient_color(l_p, l_v.z, l_v.w.x, l_v.f5, l_v.f6);
    }
    else
    {
      if ((l_kind > 0x1.c00000p1f))
      {
        l_c = coax_p_chat_elbow_color(l_p, l_v.z, l_v.w, l_v.f5);
      }
      else
      {
        if ((l_kind > 0x1.400000p1f))
        {
          l_c = coax_p_wg_make_f4_f3f(coax_p_v3(l_poly.x, l_poly.y, l_poly.z), (l_poly.w * l_v.f5.w));
        }
        else
        {
          if ((l_kind > 0x1.800000p0f))
          {
            l_c = coax_p_wg_make_f4_f3f(coax_p_v3(l_v.f5.x, l_v.f5.y, l_v.f5.z), (l_v.f5.w * l_mono));
          }
          else
          {
            if ((l_kind > 0x1.000000p-1f))
            {
              l_c = coax_p_chat_shadow_color(l_p, l_v.z, l_v.w.x, l_v.y.y, l_v.f5);
            }
            else
            {
              l_c = coax_p_chat_quad_color(l_p, l_v.z, l_v.w.x, l_v.w.y, l_v.f5, l_v.f6);
            }
          }
        }
      }
    }
  }
  l_c.w = (l_c.w * coax_p_chat_edge_fade_alpha(l_p, l_v.f8));
  return l_c;
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
fn coax_p_wg_dims_2d() -> coax_t14 {
  return coax_p_u2(textureDimensions(gpui_atlas).x, textureDimensions(gpui_atlas).y);
}
fn coax_p_wg_cvt_f2u2(p_p: coax_t14) -> vec2f {
  var l_p: coax_t14 = p_p;
  return coax_p_v2(f32(l_p.x), f32(l_p.y));
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_cvt_i2f2(p_p: vec2f) -> coax_t13 {
  var l_p: vec2f = p_p;
  return coax_p_i2(coax_p_wg_i32_f(l_p.x), coax_p_wg_i32_f(l_p.y));
}
fn coax_p_wg_load_2d(p_c: coax_t13, p_level: i32) -> vec4f {
  var l_c: coax_t13 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(gpui_atlas, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_wg_dims_2d_1() -> coax_t14 {
  return coax_p_u2(textureDimensions(gpui_color).x, textureDimensions(gpui_color).y);
}
fn coax_p_wg_load_2d_1(p_c: coax_t13, p_level: i32) -> vec4f {
  var l_c: coax_t13 = p_c;
  var l_level: i32 = p_level;
  return textureLoad(gpui_color, vec2i(l_c.x, l_c.y), l_level);
}
fn coax_p_chat_inset_color(p_p: vec2f, p_bounds: vec4f, p_corner_radius: f32, p_dy: f32, p_color: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_bounds: vec4f = p_bounds;
  var l_corner_radius: f32 = p_corner_radius;
  var l_dy: f32 = p_dy;
  var l_color: vec4f = p_color;
  var l_half_size: vec2f;
  var l_a: vec2f;
  var l_b: vec2f;
  var l_inside: f32;
  var l_covered: f32;
  l_half_size = coax_p_wg_div_f2f(coax_p_v2(l_bounds.z, l_bounds.w), 0x1.000000p1f);
  l_a = coax_p_wg_add_f2f(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_bounds.x, l_bounds.y)), l_half_size)), l_half_size), l_corner_radius);
  l_b = coax_p_wg_add_f2f(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(0.0f, l_dy)), coax_p_v2(l_bounds.x, l_bounds.y)), l_half_size)), l_half_size), l_corner_radius);
  l_inside = coax_p_saturate((0x1.000000p-1f - coax_p_chat_quad_sdf_impl(l_a, l_corner_radius)));
  l_covered = coax_p_saturate((0x1.000000p-1f - coax_p_chat_quad_sdf_impl(l_b, l_corner_radius)));
  return coax_p_wg_mul_f4f4(l_color, coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, (l_inside * (0x1.000000p0f - l_covered))));
}
fn coax_p_chat_gradient_color(p_p: vec2f, p_bounds: vec4f, p_corner_radius: f32, p_top: vec4f, p_bottom: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_bounds: vec4f = p_bounds;
  var l_corner_radius: f32 = p_corner_radius;
  var l_top: vec4f = p_top;
  var l_bottom: vec4f = p_bottom;
  var l_t: f32;
  l_t = coax_p_saturate(((l_p.y - l_bounds.y) / coax_p_fmax(l_bounds.w, 0x1.000000p0f)));
  return coax_p_chat_quad_color(l_p, l_bounds, l_corner_radius, 0.0f, coax_p_wg_mix_f4f4f(l_top, l_bottom, l_t), coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f));
}
fn coax_p_chat_elbow_color(p_p: vec2f, p_b: vec4f, p_c: vec2f, p_color: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_b: vec4f = p_b;
  var l_c: vec2f = p_c;
  var l_color: vec4f = p_color;
  var l_s: vec2f;
  var l_r: f32;
  var l_end_x: f32;
  var l_half_w: f32;
  var l_dist: f32;
  var l_prev: vec2f;
  var l_k: i32;
  var l_t: f32;
  var l_q: vec2f;
  var l_cover: f32;
  l_s = coax_p_v2(l_b.x, l_b.y);
  l_r = l_b.z;
  l_end_x = l_b.w;
  l_half_w = (0x1.000000p-1f * l_c.x);
  l_dist = 0x1.dcd650p29f;
  l_prev = l_s;
  {
    l_k = 1i;
    while ((l_k <= 24i))
    {
      l_t = (f32(l_k) / 0x1.800000p4f);
      l_q = coax_p_wg_add_f2f2(l_s, coax_p_wg_mul_ff2(l_r, coax_p_v2((l_t * l_t), ((0x1.000000p1f * l_t) - (l_t * l_t)))));
      l_dist = coax_p_fmin(l_dist, coax_p_chat_segment_distance(l_p, l_prev, l_q));
      l_prev = l_q;
      l_k = (l_k + 1i);
    }
  }
  l_dist = coax_p_fmin(l_dist, coax_p_chat_segment_distance(l_p, l_prev, coax_p_v2(l_end_x, (l_s.y + l_r))));
  l_cover = coax_p_saturate(((l_half_w + 0x1.000000p-1f) - l_dist));
  if ((l_c.y > 0x1.000000p-1f))
  {
    l_cover = coax_p_fmax(0.0f, (l_cover - coax_p_saturate(((l_half_w + 0x1.000000p-1f) - abs((l_p.x - l_s.x))))));
  }
  return coax_p_wg_mul_f4f4(l_color, coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, l_cover));
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_chat_shadow_color(p_p: vec2f, p_bounds: vec4f, p_corner_radius: f32, p_blur: f32, p_color: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_bounds: vec4f = p_bounds;
  var l_corner_radius: f32 = p_corner_radius;
  var l_blur: f32 = p_blur;
  var l_color: vec4f = p_color;
  var l_half_size: vec2f;
  var l_pt: vec2f;
  var l_alpha: f32;
  var l_corner_center_to_point: vec2f;
  var l_low: f32;
  var l_high: f32;
  var l_start: f32;
  var l_end: f32;
  var l_step: f32;
  var l_y: f32;
  var l_i: i32;
  l_half_size = coax_p_wg_div_f2f(coax_p_v2(l_bounds.z, l_bounds.w), 0x1.000000p1f);
  l_pt = coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_bounds.x, l_bounds.y), l_half_size));
  l_alpha = 0.0f;
  if ((l_blur == 0.0f))
  {
    l_corner_center_to_point = coax_p_wg_add_f2f(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(l_pt), l_half_size), l_corner_radius);
    l_alpha = coax_p_saturate((0x1.000000p-1f - coax_p_chat_quad_sdf_impl(l_corner_center_to_point, l_corner_radius)));
  }
  else
  {
    l_low = (l_pt.y - l_half_size.y);
    l_high = (l_pt.y + l_half_size.y);
    l_start = coax_p_clampf(((-0x1.800000p1f) * l_blur), l_low, l_high);
    l_end = coax_p_clampf((0x1.800000p1f * l_blur), l_low, l_high);
    l_step = ((l_end - l_start) / 0x1.000000p2f);
    l_y = (l_start + (l_step * 0x1.000000p-1f));
    {
      l_i = 0i;
      while ((l_i < 4i))
      {
        l_alpha = (l_alpha + ((coax_p_chat_blur_along_x(l_pt.x, (l_pt.y - l_y), l_blur, l_corner_radius, l_half_size) * coax_p_chat_gaussian(l_y, l_blur)) * l_step));
        l_y = (l_y + l_step);
        l_i = (l_i + 1i);
      }
    }
  }
  return coax_p_wg_mul_f4f4(l_color, coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, l_alpha));
}
fn coax_p_chat_quad_color(p_p: vec2f, p_bounds: vec4f, p_corner_radius: f32, p_bw: f32, p_background_color: vec4f, p_border_color: vec4f) -> vec4f {
  var l_p: vec2f = p_p;
  var l_bounds: vec4f = p_bounds;
  var l_corner_radius: f32 = p_corner_radius;
  var l_bw: f32 = p_bw;
  var l_background_color: vec4f = p_background_color;
  var l_border_color: vec4f = p_border_color;
  var l_antialias_threshold: f32;
  var l_half_size: vec2f;
  var l_center_to_point: vec2f;
  var l_reduced_border: f32;
  var l_corner_to_point: vec2f;
  var l_corner_center_to_point: vec2f;
  var l_is_near_rounded_corner: bool;
  var l_straight_border_inner_corner_to_point: vec2f;
  var l_is_beyond_inner_straight_border: bool;
  var l_is_within_inner_straight_border: bool;
  var l_outer_sdf: f32;
  var l_inner_sdf: f32;
  var l_border_sdf: f32;
  var l_color: vec4f;
  var l_blended_border: vec4f;
  if (((l_bw == 0.0f) && (l_corner_radius == 0.0f)))
  {
    return l_background_color;
  }
  l_antialias_threshold = 0x1.000000p-1f;
  l_half_size = coax_p_wg_div_f2f(coax_p_v2(l_bounds.z, l_bounds.w), 0x1.000000p1f);
  l_center_to_point = coax_p_wg_sub_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(l_bounds.x, l_bounds.y)), l_half_size);
  l_reduced_border = select(l_bw, (-l_antialias_threshold), (l_bw == 0.0f));
  l_corner_to_point = coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(l_center_to_point), l_half_size);
  l_corner_center_to_point = coax_p_wg_add_f2f(l_corner_to_point, l_corner_radius);
  l_is_near_rounded_corner = ((l_corner_center_to_point.x >= 0.0f) && (l_corner_center_to_point.y >= 0.0f));
  l_straight_border_inner_corner_to_point = coax_p_wg_add_f2f(l_corner_to_point, l_reduced_border);
  l_is_beyond_inner_straight_border = ((l_straight_border_inner_corner_to_point.x > 0.0f) || (l_straight_border_inner_corner_to_point.y > 0.0f));
  l_is_within_inner_straight_border = ((l_straight_border_inner_corner_to_point.x < (-l_antialias_threshold)) && (l_straight_border_inner_corner_to_point.y < (-l_antialias_threshold)));
  if ((l_is_within_inner_straight_border && (!l_is_near_rounded_corner)))
  {
    return l_background_color;
  }
  l_outer_sdf = coax_p_chat_quad_sdf_impl(l_corner_center_to_point, l_corner_radius);
  l_inner_sdf = 0.0f;
  if (((l_corner_center_to_point.x <= 0.0f) || (l_corner_center_to_point.y <= 0.0f)))
  {
    l_inner_sdf = (-coax_p_fmax(l_straight_border_inner_corner_to_point.x, l_straight_border_inner_corner_to_point.y));
  }
  else
  {
    if (l_is_beyond_inner_straight_border)
    {
      l_inner_sdf = (-0x1.000000p0f);
    }
    else
    {
      l_inner_sdf = (-(l_outer_sdf + l_reduced_border));
    }
  }
  l_border_sdf = coax_p_fmax(l_inner_sdf, l_outer_sdf);
  l_color = l_background_color;
  if ((l_border_sdf < l_antialias_threshold))
  {
    l_blended_border = coax_p_chat_over(l_background_color, l_border_color);
    l_color = coax_p_wg_mix_f4f4f(l_background_color, l_blended_border, coax_p_saturate((l_antialias_threshold - l_inner_sdf)));
  }
  return coax_p_wg_mul_f4f4(l_color, coax_p_v4(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f, coax_p_saturate((l_antialias_threshold - l_outer_sdf))));
}
fn coax_p_chat_edge_fade_alpha(p_p: vec2f, p_fade: vec4f) -> f32 {
  var l_p: vec2f = p_p;
  var l_fade: vec4f = p_fade;
  var l_ramp: f32;
  l_ramp = 0x1.000000p0f;
  if ((l_fade.y > 0.0f))
  {
    l_ramp = coax_p_fmin(l_ramp, coax_p_clampf(((l_p.y - l_fade.x) / l_fade.y), 0.0f, 0x1.000000p0f));
  }
  if ((l_fade.w > 0.0f))
  {
    l_ramp = coax_p_fmin(l_ramp, coax_p_clampf(((l_fade.z - l_p.y) / l_fade.w), 0.0f, 0x1.000000p0f));
  }
  return (l_ramp * l_ramp);
}
fn coax_p_u2(p_x: u32, p_y: u32) -> coax_t14 {
  var l_x: u32 = p_x;
  var l_y: u32 = p_y;
  return coax_t14(l_x, l_y);
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
fn coax_p_i2(p_x: i32, p_y: i32) -> coax_t13 {
  var l_x: i32 = p_x;
  var l_y: i32 = p_y;
  return coax_t13(l_x, l_y);
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_abs_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(abs(l_p0.x), abs(l_p0.y));
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_chat_quad_sdf_impl(p_corner_center_to_point: vec2f, p_corner_radius: f32) -> f32 {
  var l_corner_center_to_point: vec2f = p_corner_center_to_point;
  var l_corner_radius: f32 = p_corner_radius;
  var l_signed_distance_to_inset_quad: f32;
  if ((l_corner_radius == 0.0f))
  {
    return coax_p_fmax(l_corner_center_to_point.x, l_corner_center_to_point.y);
  }
  l_signed_distance_to_inset_quad = (coax_p_wg_length2(coax_p_wg_max_f2f2(coax_p_v2(0.0f, 0.0f), l_corner_center_to_point)) + coax_p_fmin(0.0f, coax_p_fmax(l_corner_center_to_point.x, l_corner_center_to_point.y)));
  return (l_signed_distance_to_inset_quad - l_corner_radius);
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_wg_mul_f4f4(p_a: vec4f, p_b: vec4f) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return coax_p_v4((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z), (l_a.w * l_b.w));
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
fn coax_p_wg_mix_f4f4f(p_p0: vec4f, p_p1: vec4f, p_p2: f32) -> vec4f {
  var l_p0: vec4f = p_p0;
  var l_p1: vec4f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v4(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2), coax_p_mix(l_p0.w, l_p1.w, l_p2));
}
fn coax_p_wg_mul_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a * l_b.x), (l_a * l_b.y));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_chat_segment_distance(p_p: vec2f, p_a: vec2f, p_b: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  var l_pa: vec2f;
  var l_ba: vec2f;
  var l_h: f32;
  l_pa = coax_p_wg_sub_f2f2(l_p, l_a);
  l_ba = coax_p_wg_sub_f2f2(l_b, l_a);
  l_h = coax_p_clampf((coax_p_wg_dot2(l_pa, l_ba) / coax_p_fmax(coax_p_wg_dot2(l_ba, l_ba), 0x1.0c6f7ap-20f)), 0.0f, 0x1.000000p0f);
  return coax_p_wg_length2(coax_p_wg_sub_f2f2(l_pa, coax_p_wg_mul_f2f(l_ba, l_h)));
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
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_low: f32 = p_low;
  var l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_chat_blur_along_x(p_x: f32, p_y: f32, p_sigma: f32, p_corner: f32, p_half_size: vec2f) -> f32 {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_sigma: f32 = p_sigma;
  var l_corner: f32 = p_corner;
  var l_half_size: vec2f = p_half_size;
  var l_delta: f32;
  var l_curved: f32;
  var l_integral: vec2f;
  l_delta = coax_p_fmin(((l_half_size.y - l_corner) - abs(l_y)), 0.0f);
  l_curved = ((l_half_size.x - l_corner) + sqrt(coax_p_fmax(0.0f, ((l_corner * l_corner) - (l_delta * l_delta)))));
  l_integral = coax_p_wg_add_ff2(0x1.000000p-1f, coax_p_wg_mul_ff2(0x1.000000p-1f, coax_p_chat_erf2(coax_p_wg_mul_f2f(coax_p_wg_add_ff2(l_x, coax_p_v2((-l_curved), l_curved)), (sqrt(0x1.000000p-1f) / l_sigma)))));
  return (l_integral.y - l_integral.x);
}
fn coax_p_chat_gaussian(p_x: f32, p_sigma: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_sigma: f32 = p_sigma;
  return (exp(((-(l_x * l_x)) / ((0x1.000000p1f * l_sigma) * l_sigma))) / (sqrt((0x1.000000p1f * 0x1.921fb6p1f)) * l_sigma));
}
fn coax_p_chat_over(p_below: vec4f, p_above: vec4f) -> vec4f {
  var l_below: vec4f = p_below;
  var l_above: vec4f = p_above;
  var l_alpha: f32;
  l_alpha = (l_above.w + (l_below.w * (0x1.000000p0f - l_above.w)));
  return coax_p_wg_make_f4_f3f(coax_p_wg_div_f3f(coax_p_wg_add_f3f3(coax_p_wg_mul_f3f(coax_p_v3(l_above.x, l_above.y, l_above.z), l_above.w), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_below.x, l_below.y, l_below.z), l_below.w), (0x1.000000p0f - l_above.w))), l_alpha), l_alpha);
}
fn coax_p_wg_max_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v2(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_wg_add_ff2(p_a: f32, p_b: vec2f) -> vec2f {
  var l_a: f32 = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a + l_b.x), (l_a + l_b.y));
}
fn coax_p_chat_erf2(p_x: vec2f) -> vec2f {
  var l_x: vec2f = p_x;
  var l_s: vec2f;
  var l_a: vec2f;
  var l_r1: vec2f;
  var l_r2: vec2f;
  l_s = coax_p_wg_sign_f2(l_x);
  l_a = coax_p_wg_abs_f2(l_x);
  l_r1 = coax_p_wg_add_ff2(0x1.000000p0f, coax_p_wg_mul_f2f2(coax_p_wg_add_ff2(0x1.1d130ep-2f, coax_p_wg_mul_f2f2(coax_p_wg_add_ff2(0x1.d7d630p-3f, coax_p_wg_mul_f2f2(coax_p_wg_add_ff2(0x1.fd9ba2p-11f, coax_p_wg_mul_ff2(0x1.3fee2cp-4f, l_a)), l_a)), l_a)), l_a));
  l_r2 = coax_p_wg_mul_f2f2(l_r1, l_r1);
  return coax_p_wg_sub_f2f2(l_s, coax_p_wg_div_f2f2(l_s, coax_p_wg_mul_f2f2(l_r2, l_r2)));
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
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_wg_sign_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_wg_sign(l_p0.x), coax_p_wg_sign(l_p0.y));
}
fn coax_p_wg_div_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x / l_b.x), (l_a.y / l_b.y));
}
fn coax_p_wg_sign(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x > 0.0f))
  {
    return 0x1.000000p0f;
  }
  if ((l_x < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return 0.0f;
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_9) -> coax_io_11 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t9(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6));
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8));
  return coax_r;
}