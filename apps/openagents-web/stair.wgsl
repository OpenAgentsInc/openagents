// ---- generated: apps/openagents-web/gpu/stair.coax (the module) by coax wgsl; do not edit
struct coax_t9 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_in_9 {
  @location(0) x: vec4f,
  @location(1) y: vec4f,
  @location(2) z: vec4f,
  @location(3) w: vec4f,
}
struct coax_t11 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
  f5: f32,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
  @location(4) @interpolate(flat) f5: f32,
}
struct coax_t13 {
  x: vec3f,
  y: f32,
  z: vec3f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 118>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  var l_i: u32 = p_i;
  var l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_fx_vs(p_vi: u32, p__iid2: u32, p_in_: coax_t9) -> coax_t11 {
  var l_vi: u32 = p_vi;
  var l__iid2: u32 = p__iid2;
  var l_in_: coax_t9 = p_in_;
  var l_a: vec4f;
  var l_b: vec4f;
  var l_c: vec4f;
  var l_d: vec4f;
  var l_o: coax_t11;
  l_a = l_in_.x;
  l_b = l_in_.y;
  l_c = l_in_.z;
  l_d = l_in_.w;
  l_o = coax_t11();
  l_o.f5 = select(0.0f, 0x1.000000p0f, (f32(l_vi) < coax_at_u(i32(15i))));
  l_o.x = coax_p_v4((((l_a.x / coax_at_u(i32(0i))) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / coax_at_u(i32(1i))) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_b;
  l_o.w = l_c;
  l_o.f4 = l_d;
  return l_o;
}
fn coax_p_bg_fs(p_pos: vec4f) -> vec4f {
  var l_pos: vec4f = p_pos;
  var l_p: vec2f;
  var l_q: vec2f;
  var l_c: vec3f;
  var l_t: f32;
  var l_local: vec2f;
  var l_banded: bool;
  var l_inside: bool;
  var l_sc: vec2f;
  var l_f: vec2f;
  var l_col: i32;
  var l_row: i32;
  var l_lf: vec2f;
  var l_i: i32;
  var l_kind: i32;
  var l_ao: f32;
  var l_edge: f32;
  var l_n: f32;
  var l_edge_19: f32;
  var l_count: i32;
  var l_k: i32;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  l_q = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f(coax_p_wg_div_f2f2(l_p, coax_p_wg_at2(0i)), 0x1.000000p-1f), 0x1.000000p1f);
  l_c = coax_p_wg_mul_f3f(coax_p_v3(0x1.89374cp-7f, 0x1.89374cp-7f, 0x1.47ae14p-6f), (0x1.000000p0f - (0x1.333334p-2f * coax_p_wg_length2(l_q))));
  l_t = coax_at_u(i32(6i));
  l_local = coax_p_wg_div_f2f2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(coax_at_u(i32(4i)), coax_at_u(i32(5i)))), coax_p_v2(l_t, (l_t * coax_at_u(i32(7i)))));
  l_banded = ((l_p.y >= coax_at_u(i32(270i))) && (l_p.y <= coax_at_u(i32(271i))));
  l_inside = ((((l_banded && (l_local.x >= 0.0f)) && (l_local.y >= 0.0f)) && (l_local.x < coax_at_u(i32(8i)))) && (l_local.y < coax_at_u(i32(9i))));
  if (((l_t > 0.0f) && l_inside))
  {
    l_sc = coax_p_wg_floor_f2(l_local);
    l_f = coax_p_wg_fract_f2(l_local);
    l_col = coax_p_wg_i32_f(l_sc.x);
    l_row = coax_p_wg_i32_f(l_sc.y);
    l_lf = l_f;
    if ((coax_at_u(i32(10i)) > 0x1.000000p-1f))
    {
      l_col = (13i - coax_p_wg_i32_f(l_sc.y));
      l_row = coax_p_wg_i32_f(l_sc.x);
      l_lf = coax_p_v2((0x1.000000p0f - l_f.y), l_f.x);
    }
    l_i = (l_col + (l_row * 14i));
    l_kind = coax_p_stair_cell_kind(l_i);
    if (coax_p_stair_on_stair(l_kind))
    {
      l_c = coax_p_stair_stair_stone(l_lf, l_i, l_kind);
    }
    else
    {
      l_c = coax_p_stair_floor_stone(l_f, l_sc);
      l_ao = 0x1.000000p0f;
      if (coax_p_stair_on_stair(coax_p_stair_cell_kind(coax_p_stair_idx((l_col - 1i), l_row))))
      {
        l_ao = coax_p_fmin(l_ao, coax_p_smoothstep(0.0f, 0x1.99999ap-3f, l_lf.x));
      }
      if (coax_p_stair_on_stair(coax_p_stair_cell_kind(coax_p_stair_idx((l_col + 1i), l_row))))
      {
        l_ao = coax_p_fmin(l_ao, coax_p_smoothstep(0.0f, 0x1.99999ap-3f, (0x1.000000p0f - l_lf.x)));
      }
      if (coax_p_stair_on_stair(coax_p_stair_cell_kind(coax_p_stair_idx(l_col, (l_row - 1i)))))
      {
        l_ao = coax_p_fmin(l_ao, coax_p_smoothstep(0.0f, 0x1.99999ap-3f, l_lf.y));
      }
      if (coax_p_stair_on_stair(coax_p_stair_cell_kind(coax_p_stair_idx(l_col, (l_row + 1i)))))
      {
        l_ao = coax_p_fmin(l_ao, coax_p_smoothstep(0.0f, 0x1.99999ap-3f, (0x1.000000p0f - l_lf.y)));
      }
      l_c = coax_p_wg_mul_f3f(l_c, (0x1.ccccccp-2f + (0x1.19999ap-1f * l_ao)));
    }
    l_c = coax_p_wg_mul_f3f3(l_c, coax_p_stair_light_at(l_local));
    if ((l_kind == 3i))
    {
      l_c = coax_p_stair_door(l_c, l_f);
    }
    if (((l_i == coax_p_wg_i32_f((coax_at_u(i32(11i)) + 0x1.000000p-1f))) && (coax_at_u(i32(11i)) >= 0.0f)))
    {
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_v3(0x1.eb851ep-5f, 0x1.c28f5cp-5f, 0x1.47ae14p-5f));
    }
    if (((l_i == coax_p_wg_i32_f((coax_at_u(i32(13i)) + 0x1.000000p-1f))) && (coax_at_u(i32(13i)) >= 0.0f)))
    {
      l_edge = coax_p_fmin(coax_p_fmin(l_f.x, (0x1.000000p0f - l_f.x)), coax_p_fmin(l_f.y, (0x1.000000p0f - l_f.y)));
      l_c = coax_p_wg_mix_f3f3f(coax_p_wg_add_f3f3(l_c, coax_p_v3(0x1.47ae14p-4f, 0x1.1eb852p-4f, 0x1.eb851ep-6f)), coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.000000p-1f), (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.47ae14p-4f, l_edge)));
    }
    l_c = coax_p_stair_stand(l_c, l_local, l_sc, l_t);
    l_c = coax_p_stair_stand(l_c, l_local, coax_p_wg_add_f2f2(l_sc, coax_p_v2(0.0f, 0x1.000000p0f)), l_t);
  }
  else
  {
    if (((((((l_t > 0.0f) && l_banded) && (l_local.x >= (-0x1.47ae14p-3f))) && (l_local.x < (coax_at_u(i32(8i)) + 0x1.47ae14p-3f))) && (l_local.y >= (-0x1.19999ap-1f))) && (l_local.y < (coax_at_u(i32(9i)) + 0x1.47ae14p-3f))))
    {
      if ((((l_local.y < 0.0f) && (l_local.x >= 0.0f)) && (l_local.x < coax_at_u(i32(8i)))))
      {
        l_c = coax_p_stair_wall(l_local);
        l_c = coax_p_stair_stand(l_c, l_local, coax_p_v2(floor(l_local.x), 0.0f), l_t);
      }
      else
      {
        l_n = coax_p_stair_noise(coax_p_wg_mul_f2f(l_local, 0x1.200000p3f));
        l_c = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f3(coax_p_wg_mul_f3f(coax_p_v3(0x1.eb851ep-3f, 0x1.d70a3ep-3f, 0x1.147ae2p-2f), (0x1.800000p-1f + (0x1.666666p-2f * l_n))), coax_p_stair_light_at(coax_p_wg_clamp_f2f2f2(l_local, coax_p_v2(0.0f, 0.0f), coax_p_v2(coax_at_u(i32(8i)), coax_at_u(i32(9i)))))), 0x1.99999ap-1f);
        if ((l_local.y < 0.0f))
        {
          l_c = coax_p_wg_mul_f3f(coax_p_stair_wall(coax_p_v2(coax_p_clampf(l_local.x, 0.0f, (coax_at_u(i32(8i)) - 0x1.47ae14p-7f)), l_local.y)), 0x1.333334p-1f);
        }
      }
    }
  }
  if ((coax_at_u(i32(12i)) > 0.0f))
  {
    l_edge_19 = coax_p_smoothstep(0x1.19999ap-1f, 0x1.59999ap0f, coax_p_wg_length2(l_q));
    l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.19999ap-1f, 0x1.47ae14p-5f, 0x1.eb851ep-6f), ((l_edge_19 * coax_at_u(i32(12i))) * 0x1.333334p-1f));
  }
  l_count = coax_p_wg_i32_f(coax_at_u(i32(14i)));
  {
    l_k = 0i;
    while ((l_k < 24i))
    {
      if ((l_k >= l_count))
      {
        break;
      }
      l_c = coax_p_stair_plate(l_p, l_c, coax_p_wg_at4(((i32((l_k * 2i)) * 4i) + 272i)), coax_p_wg_at4(((i32(((l_k * 2i) + 1i)) * 4i) + 272i)));
      l_k = (l_k + 1i);
    }
  }
  return coax_p_v4(l_c.x, l_c.y, l_c.z, 0x1.000000p0f);
}
fn coax_p_fx_fs(p_v: coax_t11) -> vec4f {
  var l_v: coax_t11 = p_v;
  var l_along: f32;
  var l_mode: i32;
  var l_sq: f32;
  var l_d: f32;
  var l_e: f32;
  var l_core: f32;
  var l_g: f32;
  var l_r: f32;
  var l_o: vec2f;
  var l_d_11: f32;
  var l_x: f32;
  var l_body: f32;
  var l_col: vec3f;
  var l_ir: f32;
  var l_inner: f32;
  var l_hl: vec2f;
  var l_spec: f32;
  var l_spec2: f32;
  var l_g_20: f32;
  var l_d_21: f32;
  var l_g_22: f32;
  var l_core_23: f32;
  var l_g_24: f32;
  var l_halo: f32;
  if (((l_v.f5 > 0x1.000000p-1f) && ((l_v.x.y < coax_at_u(i32(270i))) || (l_v.x.y > coax_at_u(i32(271i))))))
  {
    discard;
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_mode = coax_p_wg_i32_f((l_v.w.w + 0x1.000000p-1f));
  if ((l_mode == 1i))
  {
    l_sq = select(0x1.000000p0f, l_v.f4.y, (l_v.f4.y > 0.0f));
    l_d = coax_p_wg_length2(coax_p_v2((l_v.y.x - (l_v.z.x * 0x1.000000p-1f)), (l_v.y.y / l_sq)));
    l_e = abs((l_d - l_v.z.y));
    l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.f4.x - 0x1.000000p-1f), (l_v.f4.x + 0x1.333334p-1f), l_e));
    l_g = (l_e / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.1eb852p-2f))), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  if ((l_mode == 2i))
  {
    l_r = coax_p_fmax(l_v.z.y, 0x1.000000p0f);
    l_o = coax_p_v2((l_v.y.x - (l_v.z.x * 0x1.000000p-1f)), l_v.y.y);
    l_d_11 = coax_p_wg_length2(l_o);
    l_x = coax_p_clampf((l_d_11 / l_r), 0.0f, 0x1.000000p0f);
    l_body = (0x1.000000p0f - coax_p_smoothstep((l_r - 0x1.99999ap-1f), (l_r + 0x1.99999ap-1f), l_d_11));
    l_col = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (0x1.333334p-2f + (0x1.b33334p-1f * coax_pow_bg_vs(l_x, 0x1.000000p2f)))), l_body);
    if ((l_v.f4.x > 0.0f))
    {
      l_ir = (l_v.f4.x * l_r);
      l_inner = ((0x1.000000p0f - coax_p_smoothstep((l_ir - 0x1.000000p0f), (l_ir + 0x1.000000p0f), coax_p_wg_length2(coax_p_wg_sub_f2f2(l_o, coax_p_v2(0.0f, (l_r * 0x1.eb851ep-4f)))))) * (0x1.666666p-2f + (0x1.000000p-2f * coax_pow_bg_vs(coax_p_clampf((coax_p_wg_length2(coax_p_wg_sub_f2f2(l_o, coax_p_v2(0.0f, (l_r * 0x1.eb851ep-4f)))) / l_ir), 0.0f, 0x1.000000p0f), 0x1.800000p1f))));
      l_col = coax_p_wg_max_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_v3(l_v.f4.y, l_v.f4.z, l_v.f4.w), l_inner));
    }
    l_hl = coax_p_wg_sub_f2f2(l_o, coax_p_wg_mul_f2f(coax_p_v2((-0x1.70a3d8p-2f), (-0x1.ae147ap-2f)), l_r));
    l_spec = exp(((-coax_p_wg_dot2(l_hl, l_hl)) / ((l_r * l_r) * 0x1.99999ap-6f)));
    l_spec2 = (exp(((-coax_p_wg_dot2(coax_p_wg_sub_f2f2(l_o, coax_p_wg_mul_f2f(coax_p_v2(0x1.99999ap-2f, 0x1.666666p-2f), l_r)), coax_p_wg_sub_f2f2(l_o, coax_p_wg_mul_f2f(coax_p_v2(0x1.99999ap-2f, 0x1.666666p-2f), l_r)))) / ((l_r * l_r) * 0x1.47ae14p-7f))) * 0x1.99999ap-2f);
    l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), ((l_spec * 0x1.ccccccp-1f) + l_spec2)), l_body));
    l_g_20 = (coax_p_fmax((l_d_11 - l_r), 0.0f) / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
    l_col = coax_p_wg_add_f3f3(l_col, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), exp((((-0x1.400000p2f) * l_g_20) * l_g_20))), 0x1.c28f5cp-3f), (0x1.000000p0f - l_body)));
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_col, l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  l_d_21 = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  if ((l_mode == 3i))
  {
    l_g_22 = (l_d_21 / coax_p_fmax((l_v.z.y + l_v.z.w), 0x1.000000p0f));
    return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), exp((((-0x1.c00000p1f) * l_g_22) * l_g_22))), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
  }
  l_core_23 = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d_21));
  l_g_24 = (l_d_21 / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g_24) * l_g_24)) * 0x1.1eb852p-2f);
  return coax_p_wg_make_f4_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core_23 + l_halo)), l_v.z.z), coax_at_u(i32(3i))), 0x1.000000p0f);
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
fn coax_p_wg_at2(p_at: i32) -> vec2f {
  var l_at: i32 = p_at;
  return coax_p_v2(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))));
}
fn coax_p_wg_div_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x / l_b.x), (l_a.y / l_b.y));
}
fn coax_p_wg_sub_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x - l_b), (l_a.y - l_b));
}
fn coax_p_wg_mul_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x * l_b), (l_a.y * l_b));
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_wg_mul_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_wg_sub_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x - l_b.x), (l_a.y - l_b.y));
}
fn coax_p_wg_floor_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(floor(l_p0.x), floor(l_p0.y));
}
fn coax_p_wg_fract_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(coax_p_fract(l_p0.x), coax_p_fract(l_p0.y));
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
fn coax_p_stair_cell_kind(p_i: i32) -> i32 {
  var l_i: i32 = p_i;
  return coax_p_wg_i32_f(floor((coax_p_stair_cell_raw(l_i) + 0x1.0624dep-10f)));
}
fn coax_p_stair_on_stair(p_k: i32) -> bool {
  var l_k: i32 = p_k;
  return (((l_k == 1i) || (l_k == 3i)) || (l_k == 4i));
}
fn coax_p_stair_stair_stone(p_lf: vec2f, p_i: i32, p_kind: i32) -> vec3f {
  var l_lf: vec2f = p_lf;
  var l_i: i32 = p_i;
  var l_kind: i32 = p_kind;
  var l_col: i32;
  var l_row: i32;
  var l_d0: f32;
  var l_l: i32;
  var l_r: i32;
  var l_up: i32;
  var l_dn: i32;
  var l_kl: bool;
  var l_kr: bool;
  var l_ku: bool;
  var l_kd: bool;
  var l_along: f32;
  var l_dir: f32;
  var l_dir_17: f32;
  var l_d: f32;
  var l_s: f32;
  var l_n: f32;
  var l_base: vec3f;
  var l_e: f32;
  var l_p: vec2f;
  var l_rr: f32;
  var l_a: f32;
  var l_ringc: vec3f;
  var l_r1: f32;
  var l_r2: f32;
  var l_runes: f32;
  var l_star: f32;
  l_col = (l_i % 14i);
  l_row = (l_i / 14i);
  l_d0 = (coax_p_fract(coax_p_stair_cell_raw(l_i)) * 0x1.900000p6f);
  l_l = coax_p_stair_idx((l_col - 1i), l_row);
  l_r = coax_p_stair_idx((l_col + 1i), l_row);
  l_up = coax_p_stair_idx(l_col, (l_row - 1i));
  l_dn = coax_p_stair_idx(l_col, (l_row + 1i));
  l_kl = coax_p_stair_on_stair(coax_p_stair_cell_kind(l_l));
  l_kr = coax_p_stair_on_stair(coax_p_stair_cell_kind(l_r));
  l_ku = coax_p_stair_on_stair(coax_p_stair_cell_kind(l_up));
  l_kd = coax_p_stair_on_stair(coax_p_stair_cell_kind(l_dn));
  l_along = 0.0f;
  if ((l_kl || l_kr))
  {
    l_dir = 0x1.000000p0f;
    if ((l_kr && ((coax_p_fract(coax_p_stair_cell_raw(l_r)) * 0x1.900000p6f) < l_d0)))
    {
      l_dir = (-0x1.000000p0f);
    }
    if ((((!l_kr) && l_kl) && ((coax_p_fract(coax_p_stair_cell_raw(l_l)) * 0x1.900000p6f) > l_d0)))
    {
      l_dir = (-0x1.000000p0f);
    }
    l_along = ((l_lf.x - 0x1.000000p-1f) * l_dir);
    if (((l_ku || l_kd) && (!(l_kl && l_kr))))
    {
      l_along = 0.0f;
    }
  }
  if ((((l_ku || l_kd) && (!(l_kl || l_kr))) || ((l_ku || l_kd) && (l_along == 0.0f))))
  {
    l_dir_17 = 0x1.000000p0f;
    if ((l_kd && ((coax_p_fract(coax_p_stair_cell_raw(l_dn)) * 0x1.900000p6f) < l_d0)))
    {
      l_dir_17 = (-0x1.000000p0f);
    }
    if ((((!l_kd) && l_ku) && ((coax_p_fract(coax_p_stair_cell_raw(l_up)) * 0x1.900000p6f) > l_d0)))
    {
      l_dir_17 = (-0x1.000000p0f);
    }
    l_along = ((l_lf.y - 0x1.000000p-1f) * l_dir_17);
  }
  l_d = (l_d0 + l_along);
  l_s = coax_p_fract((l_d * 0x1.000000p1f));
  l_n = ((coax_p_stair_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_wg_make_f2_f(f32(l_i)), 0x1.b33334p0f), coax_p_wg_mul_f2f(l_lf, 0x1.800000p2f))) * 0x1.000000p-1f) + (coax_p_stair_hash(coax_p_v2(floor((l_d * 0x1.000000p1f)), f32(l_i))) * 0x1.333334p-2f));
  l_base = coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.c28f5cp-2f, 0x1.99999ap-2f, 0x1.7ae148p-2f), (0x1.666666p-1f + (0x1.99999ap-2f * l_n))), (0x1.b33334p-1f + (0x1.000000p-2f * coax_p_clampf((l_d / 0x1.c00000p4f), 0.0f, 0x1.000000p0f))));
  l_base = coax_p_wg_mul_f3f(l_base, ((0x1.000000p0f + (0x1.70a3d8p-3f * coax_p_smoothstep(0x1.666666p-1f, 0x1.f0a3d8p-1f, l_s))) - (0x1.000000p-1f * (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.99999ap-4f, l_s)))));
  l_e = 0x1.000000p0f;
  if ((!l_kl))
  {
    l_e = coax_p_fmin(l_e, l_lf.x);
  }
  if ((!l_kr))
  {
    l_e = coax_p_fmin(l_e, (0x1.000000p0f - l_lf.x));
  }
  if ((!l_ku))
  {
    l_e = coax_p_fmin(l_e, l_lf.y);
  }
  if ((!l_kd))
  {
    l_e = coax_p_fmin(l_e, (0x1.000000p0f - l_lf.y));
  }
  l_base = coax_p_wg_mix_f3f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p-1f, 0x1.e147aep-2f, 0x1.c28f5cp-2f), (0x1.99999ap-1f + (0x1.333334p-2f * l_n))), l_base, coax_p_smoothstep(0x1.eb851ep-5f, 0x1.99999ap-4f, l_e));
  l_base = coax_p_wg_mul_f3f(l_base, (0x1.19999ap-1f + (0x1.ccccccp-2f * coax_p_smoothstep(0.0f, 0x1.99999ap-5f, l_e))));
  if ((l_kind == 4i))
  {
    l_p = coax_p_wg_sub_f2f2(l_lf, coax_p_v2(0x1.000000p-1f, 0x1.000000p-1f));
    l_rr = coax_p_wg_length2(l_p);
    l_a = (coax_p_wg_atan2(l_p.y, l_p.x) + (coax_at_u(i32(2i)) * 0x1.99999ap-2f));
    l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_v3(0x1.666666p-2f, 0x1.333334p-2f, 0x1.c28f5cp-3f), coax_p_smoothstep(0x1.e147aep-2f, 0x1.c28f5cp-2f, l_rr));
    l_ringc = coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.ccccccp-2f);
    l_r1 = (0x1.000000p0f - coax_p_smoothstep(0x1.89374cp-7f, 0x1.99999ap-6f, abs((l_rr - 0x1.851eb8p-2f))));
    l_r2 = (0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-7f, 0x1.47ae14p-6f, abs((l_rr - 0x1.147ae2p-2f))));
    l_runes = (((0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-6f, 0x1.99999ap-5f, abs((l_rr - 0x1.4cccccp-2f)))) * coax_p_stepf(0x1.19999ap-1f, coax_p_fract((((l_a * 0x1.800000p3f) / 0x1.921fb6p2f) * 0x1.000000p1f)))) * coax_p_stepf(0x1.333334p-2f, coax_p_stair_hash(coax_p_v2(floor(((l_a * 0x1.800000p4f) / 0x1.921fb6p2f)), 0x1.000000p0f))));
    l_star = (0x1.000000p0f - coax_p_smoothstep(0x1.47ae14p-7f, 0x1.47ae14p-6f, abs(((l_rr - 0x1.147ae2p-2f) + (0x1.99999ap-4f * abs(sin((l_a * 0x1.400000p1f))))))));
    l_base = coax_p_wg_add_f3f3(l_base, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_ringc, (((l_r1 + (l_r2 * 0x1.99999ap-1f)) + (l_runes * 0x1.666666p-1f)) + ((l_star * 0x1.666666p-2f) * coax_p_stepf(l_rr, 0x1.147ae2p-2f)))), (0x1.800000p-1f + (0x1.000000p-2f * sin((coax_at_u(i32(2i)) * 0x1.000000p1f))))));
    l_base = coax_p_wg_add_f3f3(l_base, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_ringc, 0x1.000000p-2f), (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.333334p-2f, l_rr))));
  }
  return l_base;
}
fn coax_p_stair_floor_stone(p_f: vec2f, p_id: vec2f) -> vec3f {
  var l_f: vec2f = p_f;
  var l_id: vec2f = p_id;
  var l_h: f32;
  var l_g: vec2f;
  var l_sub: vec2f;
  var l_n: f32;
  var l_edge: f32;
  var l_mortar: f32;
  var l_bevel: f32;
  var l_base: vec3f;
  var l_crack: f32;
  l_h = coax_p_stair_hash(l_id);
  l_g = l_f;
  l_sub = l_id;
  if ((l_h > 0x1.19999ap-1f))
  {
    if ((l_f.x > 0x1.000000p-1f))
    {
      l_g.x = ((l_f.x - 0x1.000000p-1f) * 0x1.000000p1f);
      l_sub = coax_p_wg_add_f2f2(l_id, coax_p_v2(0x1.000000p-1f, 0.0f));
    }
    else
    {
      l_g.x = (l_f.x * 0x1.000000p1f);
    }
  }
  l_n = (((coax_p_stair_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_id, 0x1.8cccccp1f), coax_p_wg_mul_f2f(l_f, 0x1.400000p2f))) * 0x1.000000p-1f) + (coax_p_stair_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_id, 0x1.c00000p2f), coax_p_wg_mul_f2f(l_f, 0x1.100000p4f))) * 0x1.000000p-2f)) + (coax_p_stair_hash(l_sub) * 0x1.99999ap-2f));
  l_edge = coax_p_fmin((coax_p_fmin(l_g.x, (0x1.000000p0f - l_g.x)) * select(0x1.000000p0f, 0x1.000000p-1f, (l_h > 0x1.19999ap-1f))), coax_p_fmin(l_g.y, (0x1.000000p0f - l_g.y)));
  l_mortar = coax_p_smoothstep(0.0f, 0x1.70a3d8p-5f, l_edge);
  l_bevel = (((0x1.000000p0f + (0x1.000000p-2f * (0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.99999ap-4f, l_g.y)))) - (0x1.333334p-2f * coax_p_smoothstep(0x1.b851ecp-1f, 0x1.000000p0f, l_g.y))) - (0x1.333334p-3f * coax_p_smoothstep(0x1.c28f5cp-1f, 0x1.000000p0f, l_g.x)));
  l_base = coax_p_wg_mul_f3f(coax_p_v3(0x1.99999ap-3f, 0x1.ae147ap-3f, 0x1.000000p-2f), (0x1.800000p-1f + (0x1.ccccccp-2f * l_n)));
  l_crack = abs((coax_p_stair_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_id, 0x1.600000p3f), coax_p_wg_mul_f2f2(l_f, coax_p_v2(0x1.800000p1f, 0x1.200000p3f)))) - 0x1.000000p-1f));
  if ((coax_p_stair_hash(coax_p_wg_add_f2f(l_id, 0x1.a66666p1f)) > 0x1.666666p-1f))
  {
    l_base = coax_p_wg_mul_f3f(l_base, (0x1.666666p-1f + (0x1.333334p-2f * coax_p_smoothstep(0.0f, 0x1.1eb852p-5f, l_crack))));
  }
  if ((coax_p_stair_hash(coax_p_wg_add_f2f(l_id, 0x1.233334p3f)) > 0x1.8f5c28p-1f))
  {
    l_base = coax_p_wg_mix_f3f3f(l_base, coax_p_v3(0x1.1eb852p-3f, 0x1.eb851ep-3f, 0x1.0a3d70p-3f), (((0x1.000000p0f - coax_p_smoothstep(0.0f, 0x1.333334p-2f, l_edge)) * 0x1.333334p-1f) * coax_p_stair_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(l_f, 0x1.200000p3f), l_id))));
  }
  return coax_p_wg_mix_f3f3f(coax_p_v3(0x1.eb851ep-6f, 0x1.eb851ep-6f, 0x1.47ae14p-5f), coax_p_wg_mul_f3f(l_base, l_bevel), l_mortar);
}
fn coax_p_stair_idx(p_c: i32, p_r: i32) -> i32 {
  var l_c: i32 = p_c;
  var l_r: i32 = p_r;
  if (((((l_c < 0i) || (l_c >= 14i)) || (l_r < 0i)) || (l_r >= 9i)))
  {
    return (-1i);
  }
  return (l_c + (l_r * 14i));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
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
fn coax_p_stair_light_at(p_l: vec2f) -> vec3f {
  var l_l: vec2f = p_l;
  var l_c: vec3f;
  var l_n: i32;
  var l_i: i32;
  var l_d: vec2f;
  var l_j: i32;
  var l_y: f32;
  var l_a: vec2f;
  var l_b: vec2f;
  var l_e: i32;
  var l_en: vec4f;
  var l_door: vec2f;
  var l_top: vec2f;
  l_c = coax_p_v3(0x1.70a3d8p-4f, 0x1.99999ap-4f, 0x1.47ae14p-3f);
  l_n = coax_p_stair_torch_count();
  {
    l_i = 0i;
    while ((l_i < 4i))
    {
      if ((l_i >= l_n))
      {
        break;
      }
      l_d = coax_p_wg_sub_f2f2(l_l, coax_p_v2(coax_p_stair_torch_x(l_i, l_n), (-0x1.666666p-2f)));
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_div_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.ae147ap-1f, 0x1.19999ap-1f), coax_p_stair_flicker(f32(l_i))), 0x1.59999ap0f), (0x1.000000p0f + (coax_p_wg_dot2(l_d, l_d) * 0x1.851eb8p-2f))));
      l_i = (l_i + 1i);
    }
  }
  {
    l_j = 0i;
    while ((l_j < 2i))
    {
      l_y = (coax_at_u(i32(9i)) * (0x1.666666p-2f + (0x1.99999ap-2f * f32(l_j))));
      l_a = coax_p_wg_sub_f2f2(l_l, coax_p_v2((-0x1.99999ap-3f), l_y));
      l_b = coax_p_wg_sub_f2f2(l_l, coax_p_v2((coax_at_u(i32(8i)) + 0x1.99999ap-3f), l_y));
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.ae147ap-1f, 0x1.19999ap-1f), coax_p_stair_flicker((f32(l_j) + 0x1.c00000p2f))), 0x1.800000p-1f), ((0x1.000000p0f / (0x1.000000p0f + (coax_p_wg_dot2(l_a, l_a) * 0x1.333334p-1f))) + (0x1.000000p0f / (0x1.000000p0f + (coax_p_wg_dot2(l_b, l_b) * 0x1.333334p-1f))))));
      l_j = (l_j + 1i);
    }
  }
  {
    l_e = 0i;
    while ((l_e < 2i))
    {
      l_en = coax_p_wg_at4(((i32(l_e) * 4i) + 464i));
      if ((l_en.x < 0.0f))
      {
        {
          l_e = (l_e + 1i);
          continue;
        }
      }
      l_door = coax_p_wg_sub_f2f2(l_l, coax_p_stair_to_local(l_en.x, l_en.y));
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_div_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.eb851ep-4f, 0x1.1eb852p-3f), (0x1.99999ap-1f + (0x1.000000p-2f * sin((coax_at_u(i32(2i)) * 0x1.59999ap1f))))), (0x1.000000p0f + (coax_p_wg_dot2(l_door, l_door) * 0x1.666666p-1f))));
      l_top = coax_p_wg_sub_f2f2(l_l, coax_p_stair_to_local(l_en.z, l_en.w));
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_div_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.ccccccp-2f), 0x1.ccccccp-1f), (0x1.000000p0f + (coax_p_wg_dot2(l_top, l_top) * 0x1.333334p-1f))));
      l_e = (l_e + 1i);
    }
  }
  return l_c;
}
fn coax_p_wg_mul_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_stair_door(p_c: vec3f, p_f: vec2f) -> vec3f {
  var l_c: vec3f = p_c;
  var l_f: vec2f = p_f;
  var l_p: vec2f;
  var l_open: f32;
  var l_arch: f32;
  var l_o: vec3f;
  var l_ang: f32;
  var l_joints: f32;
  var l_sw: f32;
  var l_inner: vec3f;
  var l_deep: vec3f;
  l_p = coax_p_v2((l_f.x - 0x1.000000p-1f), (0x1.d70a3ep-1f - l_f.y));
  l_open = coax_p_fmin(coax_p_stair_sd_box(l_p, coax_p_v2(0.0f, 0x1.000000p-2f), coax_p_v2(0x1.333334p-2f, 0x1.000000p-2f), 0.0f), coax_p_stair_sd_circle(l_p, coax_p_v2(0.0f, 0x1.000000p-1f), 0x1.333334p-2f));
  l_arch = coax_p_fmin(coax_p_stair_sd_box(l_p, coax_p_v2(0.0f, 0x1.000000p-2f), coax_p_v2(0x1.99999ap-2f, 0x1.000000p-2f), 0.0f), coax_p_stair_sd_circle(l_p, coax_p_v2(0.0f, 0x1.000000p-1f), 0x1.99999ap-2f));
  l_o = l_c;
  l_ang = coax_p_wg_atan2((l_p.y - 0x1.000000p-1f), l_p.x);
  l_joints = coax_p_stepf(0x1.c28f5cp-1f, coax_p_fract(select((l_p.y * 0x1.400000p2f), ((l_ang * 0x1.c00000p2f) / 0x1.921fa0p1f), (l_p.y > 0x1.000000p-1f))));
  l_o = coax_p_wg_mix_f3f3f(l_o, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.70a3d8p-2f, 0x1.5c28f6p-2f, 0x1.70a3d8p-2f), (0x1.99999ap-1f + (0x1.333334p-2f * coax_p_stair_noise(coax_p_wg_mul_f2f(l_f, 0x1.400000p3f))))), (0x1.000000p0f - (0x1.000000p-1f * l_joints))), coax_p_clampf((0x1.000000p-1f - (l_arch * 0x1.e00000p5f)), 0.0f, 0x1.000000p0f));
  l_sw = coax_p_stair_noise(coax_p_v2(((coax_p_wg_atan2((l_p.y - 0x1.333334p-2f), l_p.x) * 0x1.000000p1f) + (coax_at_u(i32(2i)) * 0x1.800000p0f)), ((coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, coax_p_v2(0.0f, 0x1.333334p-2f))) * 0x1.800000p2f) - (coax_at_u(i32(2i)) * 0x1.000000p1f))));
  l_inner = coax_p_wg_mul_f3f(coax_p_wg_mix_f3f3f(coax_p_v3(0x1.666666p-2f, 0x1.47ae14p-6f, 0x1.eb851ep-5f), coax_p_v3(0x1.000000p0f, 0x1.47ae14p-3f, 0x1.99999ap-3f), l_sw), (0x1.666666p-1f + (0x1.333334p-2f * sin((coax_at_u(i32(2i)) * 0x1.59999ap1f)))));
  l_deep = coax_p_wg_mix_f3f3f(l_inner, coax_p_v3(0x1.e66666p-1f, 0x1.99999ap-2f, 0x1.800000p-1f), (0x1.000000p-2f * coax_p_smoothstep(0x1.333334p-1f, 0x1.ccccccp-1f, l_sw)));
  l_o = coax_p_wg_mix_f3f3f(l_o, l_deep, coax_p_clampf((0x1.000000p-1f - (l_open * 0x1.e00000p5f)), 0.0f, 0x1.000000p0f));
  l_o = coax_p_wg_mix_f3f3f(l_o, coax_p_STAIR_INK(), (coax_p_clampf((0x1.000000p0f - (abs(l_open) * 0x1.680000p5f)), 0.0f, 0x1.000000p0f) * 0x1.666666p-1f));
  return l_o;
}
fn coax_p_wg_add_f3f3(p_a: vec3f, p_b: vec3f) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_wg_mix_f3f3f(p_p0: vec3f, p_p1: vec3f, p_p2: f32) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  var l_p2: f32 = p_p2;
  return coax_p_v3(coax_p_mix(l_p0.x, l_p1.x, l_p2), coax_p_mix(l_p0.y, l_p1.y, l_p2), coax_p_mix(l_p0.z, l_p1.z, l_p2));
}
fn coax_p_stair_stand(p_c: vec3f, p_local: vec2f, p_sc: vec2f, p_t: f32) -> vec3f {
  var l_c: vec3f = p_c;
  var l_local: vec2f = p_local;
  var l_sc: vec2f = p_sc;
  var l_t: f32 = p_t;
  var l_i: i32;
  var l_k: i32;
  var l_v: f32;
  var l_ghost: bool;
  var l_face: f32;
  var l_flash: f32;
  var l_feet: vec2f;
  var l_q: vec2f;
  var l_pen: coax_t13;
  var l_cls: i32;
  var l_lcs: array<vec3f, 7>;
  var l_lc: vec3f;
  var l_g: f32;
  var l_under: vec3f;
  var l_back: f32;
  var l_rim: f32;
  var l_level: i32;
  var l_a: f32;
  l_i = coax_p_stair_screen_cell(l_sc);
  l_k = coax_p_stair_cell_kind(l_i);
  if ((l_k < 10i))
  {
    return l_c;
  }
  l_v = coax_p_wg_get_f4(coax_p_wg_at4(((i32((l_i / 4i)) * 4i) + 144i)), i32((l_i % 4i)));
  l_ghost = (abs(l_v) > 0x1.400000p1f);
  l_face = select((-0x1.000000p0f), 0x1.000000p0f, (l_v >= 0.0f));
  l_flash = select(coax_p_clampf((abs(l_v) - 0x1.000000p0f), 0.0f, 0x1.000000p0f), 0.0f, l_ghost);
  l_feet = coax_p_v2((l_sc.x + 0x1.000000p-1f), (l_sc.y + 0x1.7ae148p-1f));
  l_q = coax_p_wg_mul_f2f(coax_p_v2((l_local.x - l_feet.x), ((l_feet.y - l_local.y) * coax_at_u(i32(7i)))), 0x1.7ae148p-1f);
  l_pen = coax_p_stair_figure(l_q, (l_k - 10i), l_face, l_flash, (0x1.4cccccp0f / l_t), (f32(l_i) * 0x1.7ae148p-2f));
  l_cls = (l_k - 10i);
  l_lcs = array<vec3f, 7>(coax_p_v3(0x1.333334p-1f, 0x1.800000p-1f, 0x1.000000p0f), coax_p_v3(0x1.3d70a4p-1f, 0x1.000000p-1f, 0x1.000000p0f), coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.ccccccp-2f), coax_p_v3(0x1.ccccccp-2f, 0x1.e66666p-1f, 0x1.99999ap-1f), coax_p_v3(0x1.000000p-1f, 0x1.e66666p-1f, 0x1.ccccccp-2f), coax_p_v3(0x1.000000p0f, 0x1.000000p-1f, 0x1.8f5c28p-1f), coax_p_v3(0x1.99999ap-1f, 0x1.ccccccp-1f, 0x1.000000p0f));
  l_lc = l_lcs[i32(coax_p_wg_clamp_i(l_cls, 0i, 6i))];
  l_g = coax_p_wg_length2(coax_p_wg_mul_f2f2(l_q, coax_p_v2(0x1.000000p0f, 0x1.4cccccp1f)));
  l_under = l_c;
  if ((!l_ghost))
  {
    l_back = coax_p_wg_length2(coax_p_wg_mul_f2f2(coax_p_wg_sub_f2f2(l_q, coax_p_v2(0.0f, 0x1.99999ap-3f)), coax_p_v2(0x1.000000p0f, 0x1.3d70a4p-1f)));
    l_under = coax_p_wg_add_f3f3(l_under, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_lc, 0x1.47ae14p-3f), (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-4f, 0x1.5c28f6p-2f, l_back))));
    l_rim = (abs((l_g - 0x1.eb851ep-3f)) - 0x1.26e978p-6f);
    l_under = coax_p_wg_mix_f3f3f(l_under, l_lc, (0x1.b33334p-1f * coax_p_clampf((0x1.000000p-1f - (l_rim / (0x1.4cccccp1f / l_t))), 0.0f, 0x1.000000p0f)));
  }
  l_level = coax_p_wg_i32_f(((coax_p_fract(coax_p_stair_cell_raw(l_i)) * 0x1.900000p6f) + 0x1.000000p-1f));
  if (((l_level >= 3i) && (!l_ghost)))
  {
    l_pen.z = coax_p_wg_add_f3f3(l_pen.z, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_lc, (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-4f, 0x1.333334p-2f, l_g))), coax_p_smoothstep(0x1.47ae14p-4f, 0x1.99999ap-3f, l_g)), (0x1.666666p-2f + (0x1.333334p-3f * sin((coax_at_u(i32(2i)) * 0x1.800000p1f))))));
  }
  l_a = select(l_pen.y, (l_pen.y * (0x1.ccccccp-2f + (0x1.333334p-3f * sin((coax_at_u(i32(2i)) * 0x1.400000p2f))))), l_ghost);
  return coax_p_wg_add_f3f3(coax_p_wg_mix_f3f3f(l_under, l_pen.x, l_a), coax_p_wg_mul_f3f(l_pen.z, select(0x1.000000p0f, 0x1.000000p-1f, l_ghost)));
}
fn coax_p_wg_add_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x + l_b.x), (l_a.y + l_b.y));
}
fn coax_p_stair_wall(p_local: vec2f) -> vec3f {
  var l_local: vec2f = p_local;
  var l_y: f32;
  var l_course: f32;
  var l_bx: f32;
  var l_bf: vec2f;
  var l_n: f32;
  var l_c: vec3f;
  var l_joint: f32;
  var l_n_t: i32;
  var l_i: i32;
  var l_tx: f32;
  var l_p: vec2f;
  var l_fl: f32;
  var l_wob: f32;
  var l_fp: vec2f;
  var l_flame: f32;
  var l_core: f32;
  var l_fc: vec3f;
  l_y = ((-l_local.y) / 0x1.19999ap-1f);
  l_course = floor((l_y * 0x1.000000p2f));
  l_bx = ((l_local.x * 0x1.99999ap0f) + (l_course * 0x1.000000p-1f));
  l_bf = coax_p_v2(coax_p_fract(l_bx), coax_p_fract((l_y * 0x1.000000p2f)));
  l_n = ((coax_p_stair_noise(coax_p_wg_add_f2f2(coax_p_wg_mul_f2f(coax_p_v2(floor(l_bx), l_course), 0x1.d9999ap1f), coax_p_wg_mul_f2f(l_bf, 0x1.000000p2f))) * 0x1.000000p-1f) + (coax_p_stair_hash(coax_p_v2(floor(l_bx), l_course)) * 0x1.666666p-2f));
  l_c = coax_p_wg_mul_f3f(coax_p_v3(0x1.5c28f6p-3f, 0x1.5c28f6p-3f, 0x1.ae147ap-3f), (0x1.666666p-1f + (0x1.000000p-1f * l_n)));
  l_joint = coax_p_fmin((coax_p_fmin(l_bf.x, (0x1.000000p0f - l_bf.x)) * 0x1.333334p-1f), coax_p_fmin(l_bf.y, (0x1.000000p0f - l_bf.y)));
  l_c = coax_p_wg_mul_f3f(l_c, (0x1.666666p-2f + (0x1.4cccccp-1f * coax_p_smoothstep(0.0f, 0x1.47ae14p-4f, l_joint))));
  l_c = coax_p_wg_mul_f3f(l_c, (0x1.ccccccp-2f + (0x1.19999ap-1f * coax_p_smoothstep(0.0f, 0x1.666666p-2f, l_y))));
  l_c = coax_p_wg_mul_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_stair_light_at(coax_p_v2(l_local.x, (-0x1.99999ap-5f))), 0x1.ccccccp-1f));
  l_n_t = coax_p_stair_torch_count();
  {
    l_i = 0i;
    while ((l_i < 4i))
    {
      if ((l_i >= l_n_t))
      {
        break;
      }
      l_tx = coax_p_stair_torch_x(l_i, l_n_t);
      l_p = coax_p_wg_mul_f2f(coax_p_wg_div_f2f(coax_p_v2((l_local.x - l_tx), ((-l_local.y) - (0x1.19999ap-1f * 0x1.333334p-2f))), 0x1.19999ap-1f), 0x1.19999ap-1f);
      l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.99999ap-4f, 0x1.99999ap-4f, 0x1.eb851ep-4f), coax_p_clampf((0x1.000000p-1f - (coax_p_stair_sd_box(l_p, coax_p_v2(0.0f, (-0x1.eb851ep-4f)), coax_p_v2(0x1.eb851ep-5f, 0x1.47ae14p-3f), 0x1.eb851ep-6f) * 0x1.400000p6f)), 0.0f, 0x1.000000p0f));
      l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.1eb852p-2f, 0x1.99999ap-3f, 0x1.333334p-3f), coax_p_clampf((0x1.000000p-1f - (coax_p_stair_sd_box(l_p, coax_p_v2(0.0f, 0x1.47ae14p-4f), coax_p_v2(0x1.47ae14p-4f, 0x1.99999ap-5f), 0x1.47ae14p-6f) * 0x1.400000p6f)), 0.0f, 0x1.000000p0f));
      l_fl = coax_p_stair_flicker(f32(l_i));
      l_wob = (((coax_p_stair_noise(coax_p_v2(((l_p.y * 0x1.800000p2f) - (coax_at_u(i32(2i)) * 0x1.200000p3f)), f32(l_i))) - 0x1.000000p-1f) * 0x1.47ae14p-4f) * (l_p.y - 0x1.99999ap-4f));
      l_fp = coax_p_v2((l_p.x - l_wob), (l_p.y - 0x1.0a3d70p-3f));
      l_flame = coax_p_stair_sd_trap(l_fp, coax_p_v2(0.0f, 0x1.5c28f6p-3f), 0x1.47ae14p-4f, 0x1.47ae14p-8f, (0x1.5c28f6p-3f * l_fl));
      l_core = coax_p_stair_sd_trap(l_fp, coax_p_v2(0.0f, 0x1.c28f5cp-4f), 0x1.70a3d8p-5f, 0x1.47ae14p-8f, (0x1.99999ap-4f * l_fl));
      l_fc = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.000000p0f, 0x1.99999ap-3f, 0x1.47ae14p-4f), coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.99999ap-2f), coax_p_clampf((0x1.000000p0f - (l_fp.y * 0x1.19999ap1f)), 0.0f, 0x1.000000p0f));
      l_c = coax_p_wg_mix_f3f3f(l_c, l_fc, coax_p_clampf((0x1.000000p-1f - (l_flame * 0x1.180000p6f)), 0.0f, 0x1.000000p0f));
      l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.000000p0f, 0x1.f5c290p-1f, 0x1.b33334p-1f), coax_p_clampf((0x1.000000p-1f - (l_core * 0x1.180000p6f)), 0.0f, 0x1.000000p0f));
      l_c = coax_p_wg_add_f3f3(l_c, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.99999ap-1f, 0x1.ccccccp-2f), 0x1.000000p-2f), l_fl), exp(((-coax_p_wg_length2(coax_p_wg_sub_f2f2(l_fp, coax_p_v2(0.0f, 0x1.333334p-3f)))) * 0x1.400000p2f))));
      l_i = (l_i + 1i);
    }
  }
  l_c = coax_p_wg_mix_f3f3f(l_c, coax_p_v3(0x1.47ae14p-2f, 0x1.3d70a4p-2f, 0x1.70a3d8p-2f), coax_p_smoothstep(0x1.ccccccp-1f, 0x1.e66666p-1f, l_y));
  return l_c;
}
fn coax_p_stair_noise(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_s: vec2f;
  l_i = coax_p_wg_floor_f2(l_p);
  l_f = coax_p_wg_fract_f2(l_p);
  l_s = coax_p_wg_mul_f2f2(coax_p_wg_mul_f2f2(l_f, l_f), coax_p_wg_sub_ff2(0x1.800000p1f, coax_p_wg_mul_ff2(0x1.000000p1f, l_f)));
  return coax_p_mix(coax_p_mix(coax_p_stair_hash(l_i), coax_p_stair_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0.0f))), l_s.x), coax_p_mix(coax_p_stair_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0.0f, 0x1.000000p0f))), coax_p_stair_hash(coax_p_wg_add_f2f2(l_i, coax_p_v2(0x1.000000p0f, 0x1.000000p0f))), l_s.x), l_s.y);
}
fn coax_p_wg_clamp_f2f2f2(p_p0: vec2f, p_p1: vec2f, p_p2: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  var l_p2: vec2f = p_p2;
  return coax_p_v2(coax_p_clampf(l_p0.x, l_p1.x, l_p2.x), coax_p_clampf(l_p0.y, l_p1.y, l_p2.y));
}
fn coax_p_clampf(p_x: f32, p_lo: f32, p_hi: f32) -> f32 {
  var l_x: f32 = p_x;
  var l_lo: f32 = p_lo;
  var l_hi: f32 = p_hi;
  return coax_p_fmin(coax_p_fmax(l_x, l_lo), l_hi);
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  var l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_stair_plate(p_p: vec2f, p_under: vec3f, p_r: vec4f, p_s: vec4f) -> vec3f {
  var l_p: vec2f = p_p;
  var l_under: vec3f = p_under;
  var l_r: vec4f = p_r;
  var l_s: vec4f = p_s;
  var l_half: vec2f;
  var l_corner: f32;
  var l_q: vec2f;
  var l_d: f32;
  var l_v: f32;
  var l_fill: vec3f;
  var l_line: vec3f;
  var l_w: f32;
  var l_inside: f32;
  var l_edge: f32;
  var l_a: f32;
  var l_c: vec3f;
  var l_h: f32;
  var l_iw: f32;
  var l_feet: vec2f;
  var l_sc: f32;
  var l_qq: vec2f;
  var l_pen: coax_t13;
  var l_dim: f32;
  if ((l_s.x <= 0.0f))
  {
    return l_under;
  }
  l_half = coax_p_wg_mul_f2f(coax_p_wg_sub_f2f2(coax_p_v2(l_r.z, l_r.w), coax_p_v2(l_r.x, l_r.y)), 0x1.000000p-1f);
  l_corner = (0x1.c00000p2f * l_s.z);
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, coax_p_wg_add_f2f2(coax_p_v2(l_r.x, l_r.y), l_half))), l_half), coax_p_v2(l_corner, l_corner));
  l_d = ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_corner);
  if ((l_d > 0x1.000000p0f))
  {
    return l_under;
  }
  l_v = ((l_p.y - l_r.y) / coax_p_fmax((l_r.w - l_r.y), 0x1.000000p0f));
  l_fill = coax_p_v3(0x1.26e978p-6f, 0x1.47ae14p-6f, 0x1.eb851ep-6f);
  l_line = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((l_s.y > 0x1.000000p-1f))
  {
    l_fill = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.c28f5cp-4f, 0x1.c28f5cp-4f, 0x1.333334p-3f), coax_p_v3(0x1.99999ap-5f, 0x1.99999ap-5f, 0x1.1eb852p-4f), l_v);
    l_line = coax_p_v3(0x1.ae147ap-2f, 0x1.c28f5cp-2f, 0x1.0a3d70p-1f);
  }
  if ((l_s.y > 0x1.800000p0f))
  {
    l_fill = coax_p_wg_mix_f3f3f(coax_p_v3(0x1.99999ap-3f, 0x1.70a3d8p-3f, 0x1.eb851ep-4f), coax_p_v3(0x1.70a3d8p-4f, 0x1.47ae14p-4f, 0x1.1eb852p-4f), l_v);
    l_line = coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.000000p-1f);
  }
  if ((l_s.y > 0x1.400000p1f))
  {
    l_fill = coax_p_v3(0x1.eb851ep-6f, 0x1.eb851ep-6f, 0x1.47ae14p-5f);
    l_line = coax_p_v3(0x1.70a3d8p-3f, 0x1.70a3d8p-3f, 0x1.c28f5cp-3f);
  }
  l_w = (0x1.000000p0f * l_s.z);
  l_inside = coax_p_clampf((0x1.000000p-1f - l_d), 0.0f, 0x1.000000p0f);
  l_edge = coax_p_clampf((((l_w * 0x1.000000p-1f) + 0x1.000000p-1f) - abs((l_d + (l_w * 0x1.000000p-1f)))), 0.0f, 0x1.000000p0f);
  l_a = coax_p_clampf(l_s.x, 0.0f, 0x1.000000p0f);
  l_c = coax_p_wg_mix_f3f3f(coax_p_wg_mix_f3f3f(l_under, l_fill, (l_a * l_inside)), l_line, ((l_edge * l_a) * 0x1.ccccccp-1f));
  if (((l_s.w > 0x1.000000p-1f) && (l_inside > 0.0f)))
  {
    l_h = (l_r.w - l_r.y);
    l_iw = coax_p_fmin((l_h * 0x1.99999ap-1f), ((l_r.z - l_r.x) * 0x1.0a3d70p-2f));
    l_feet = coax_p_v2((l_r.x + (l_iw * 0x1.333334p-1f)), (l_r.w - (l_h * 0x1.99999ap-4f)));
    l_sc = (l_h * 0x1.e66666p-1f);
    l_qq = coax_p_wg_div_f2f(coax_p_v2((l_p.x - l_feet.x), (l_feet.y - l_p.y)), l_sc);
    if ((((abs(l_qq.x) < 0x1.333334p-2f) && (l_qq.y > (-0x1.99999ap-4f))) && (l_qq.y < 0x1.ccccccp-1f)))
    {
      l_pen = coax_p_stair_figure(l_qq, (coax_p_wg_i32_f((l_s.w + 0x1.000000p-1f)) - 1i), 0x1.000000p0f, 0.0f, (0x1.4cccccp0f / l_sc), l_s.w);
      l_dim = select(0x1.000000p0f, 0x1.666666p-2f, (l_s.y > 0x1.400000p1f));
      l_c = coax_p_wg_add_f3f3(coax_p_wg_mix_f3f3f(l_c, coax_p_wg_mul_f3f(l_pen.x, l_dim), (l_pen.y * l_inside)), coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_pen.z, l_dim), l_inside));
    }
  }
  return l_c;
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
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  var l_p0: vec3f = p_p0;
  var l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_wg_max_f3f3(p_p0: vec3f, p_p1: vec3f) -> vec3f {
  var l_p0: vec3f = p_p0;
  var l_p1: vec3f = p_p1;
  return coax_p_v3(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y), coax_p_fmax(l_p0.z, l_p1.z));
}
fn coax_p_wg_dot2(p_a: vec2f, p_b: vec2f) -> f32 {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_fract(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_stair_cell_raw(p_i: i32) -> f32 {
  var l_i: i32 = p_i;
  if (((l_i < 0i) || (l_i >= 126i)))
  {
    return (-0x1.000000p0f);
  }
  return coax_p_wg_get_f4(coax_p_wg_at4(((i32((l_i / 4i)) * 4i) + 16i)), i32((l_i % 4i)));
}
fn coax_p_wg_make_f2_f(p_p0: f32) -> vec2f {
  var l_p0: f32 = p_p0;
  return coax_p_v2(l_p0, l_p0);
}
fn coax_p_stair_hash(p_p: vec2f) -> f32 {
  var l_p: vec2f = p_p;
  return coax_p_fract((sin(coax_p_wg_dot2(l_p, coax_p_v2(0x1.fc6666p6f, 0x1.37b334p8f))) * 0x1.55dd18p15f));
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
fn coax_p_stepf(p_edge: f32, p_x: f32) -> f32 {
  var l_edge: f32 = p_edge;
  var l_x: f32 = p_x;
  if ((l_x < l_edge))
  {
    return 0.0f;
  }
  return 0x1.000000p0f;
}
fn coax_p_wg_mul_f2f2(p_a: vec2f, p_b: vec2f) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  return coax_p_v2((l_a.x * l_b.x), (l_a.y * l_b.y));
}
fn coax_p_wg_add_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x + l_b), (l_a.y + l_b));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_stair_torch_count() -> i32 {
  return select(4i, 3i, (coax_at_u(i32(8i)) < 0x1.600000p3f));
}
fn coax_p_stair_torch_x(p_i: i32, p_n: i32) -> f32 {
  var l_i: i32 = p_i;
  var l_n: i32 = p_n;
  return (((f32(l_i) + 0x1.000000p-1f) * coax_at_u(i32(8i))) / f32(l_n));
}
fn coax_p_stair_flicker(p_i: f32) -> f32 {
  var l_i: f32 = p_i;
  return (0x1.a3d70ap-1f + (0x1.70a3d8p-3f * coax_p_stair_noise(coax_p_v2(((coax_at_u(i32(2i)) * 0x1.c00000p2f) + (l_i * 0x1.a00000p3f)), l_i))));
}
fn coax_p_wg_div_f3f(p_a: vec3f, p_b: f32) -> vec3f {
  var l_a: vec3f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v3((l_a.x / l_b), (l_a.y / l_b), (l_a.z / l_b));
}
fn coax_p_stair_to_local(p_x: f32, p_y: f32) -> vec2f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  if ((coax_at_u(i32(10i)) > 0x1.000000p-1f))
  {
    return coax_p_v2(l_y, (0x1.c00000p3f - l_x));
  }
  return coax_p_v2(l_x, l_y);
}
fn coax_p_stair_sd_box(p_p: vec2f, p_c: vec2f, p_b: vec2f, p_r: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_c: vec2f = p_c;
  var l_b: vec2f = p_b;
  var l_r: f32 = p_r;
  var l_q: vec2f;
  l_q = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(coax_p_wg_abs_f2(coax_p_wg_sub_f2f2(l_p, l_c)), l_b), coax_p_v2(l_r, l_r));
  return ((coax_p_wg_length2(coax_p_wg_max_f2f2(l_q, coax_p_v2(0.0f, 0.0f))) + coax_p_fmin(coax_p_fmax(l_q.x, l_q.y), 0.0f)) - l_r);
}
fn coax_p_stair_sd_circle(p_p: vec2f, p_c: vec2f, p_r: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_c: vec2f = p_c;
  var l_r: f32 = p_r;
  return (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_p, l_c)) - l_r);
}
fn coax_p_STAIR_INK() -> vec3f {
  return coax_p_v3(0x1.99999ap-5f, 0x1.47ae14p-5f, 0x1.1eb852p-4f);
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  var l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_stair_screen_cell(p_sc: vec2f) -> i32 {
  var l_sc: vec2f = p_sc;
  if (((((l_sc.x < 0.0f) || (l_sc.y < 0.0f)) || (l_sc.x >= coax_at_u(i32(8i)))) || (l_sc.y >= coax_at_u(i32(9i)))))
  {
    return (-1i);
  }
  if ((coax_at_u(i32(10i)) > 0x1.000000p-1f))
  {
    return coax_p_stair_idx((13i - coax_p_wg_i32_f(l_sc.y)), coax_p_wg_i32_f(l_sc.x));
  }
  return coax_p_stair_idx(coax_p_wg_i32_f(l_sc.x), coax_p_wg_i32_f(l_sc.y));
}
fn coax_p_wg_get_f4(p_p: vec4f, p_i: i32) -> f32 {
  var l_p: vec4f = p_p;
  var l_i: i32 = p_i;
  if ((l_i == 0i))
  {
    return l_p.x;
  }
  if ((l_i == 1i))
  {
    return l_p.y;
  }
  if ((l_i == 2i))
  {
    return l_p.z;
  }
  return l_p.w;
}
fn coax_p_stair_figure(p_q0: vec2f, p_cls: i32, p_face: f32, p_flash: f32, p_aa: f32, p_seed: f32) -> coax_t13 {
  var l_q0: vec2f = p_q0;
  var l_cls: i32 = p_cls;
  var l_face: f32 = p_face;
  var l_flash: f32 = p_flash;
  var l_aa: f32 = p_aa;
  var l_seed: f32 = p_seed;
  var l_pen_store: array<coax_t13, 1>;
  var l_q: vec2f;
  var l_line: f32;
  var l_sh: f32;
  var l_ang: f32;
  var l_dir: vec2f;
  var l_hand: vec2f;
  var l_tip: vec2f;
  var l_blade: f32;
  var l_nrm: vec2f;
  var l_robe: vec3f;
  var l_hq: vec2f;
  var l_hat: vec3f;
  var l_orb: f32;
  var l_oc: vec3f;
  var l_cloak: vec3f;
  var l_ang_24: f32;
  var l_dir_25: vec2f;
  var l_hand_26: vec2f;
  var l_blade_27: f32;
  var l_green: vec3f;
  var l_bc: vec2f;
  var l_bd: f32;
  var l_bow: f32;
  var l_pull: f32;
  var l_rose: vec3f;
  var l_lb: f32;
  var l_plate: vec3f;
  var l_mark: f32;
  var l_ang_37: f32;
  var l_dir_38: vec2f;
  var l_hand_39: vec2f;
  var l_blade_40: f32;
  var l_nrm_41: vec2f;
  var l_robe_42: vec3f;
  var l_sun: f32;
  var l_halo: f32;
  l_pen_store = array<coax_t13, 1>();
  l_pen_store[0i] = coax_t13(coax_p_v3(0.0f, 0.0f, 0.0f), 0.0f, coax_p_v3(0.0f, 0.0f, 0.0f));
  l_q = coax_p_v2((l_q0.x * l_face), (l_q0.y - (0x1.0624dep-7f * sin(((coax_at_u(i32(2i)) * 0x1.19999ap1f) + (l_seed * 0x1.800000p2f))))));
  l_line = (l_aa * 0x1.666666p0f);
  l_sh = (coax_p_wg_length2(coax_p_wg_mul_f2f2(coax_p_wg_sub_f2f2(l_q0, coax_p_v2(0.0f, 0.0f)), coax_p_v2(0x1.000000p0f, 0x1.4cccccp1f))) - 0x1.99999ap-3f);
  l_pen_store[0i].y = coax_p_fmax(l_pen_store[0i].y, (0x1.000000p-1f * coax_p_clampf((0x1.000000p-1f - (l_sh / 0x1.eb851ep-5f)), 0.0f, 0x1.000000p0f)));
  if ((l_cls == 0i))
  {
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2((-0x1.c28f5cp-5f), 0x1.1eb852p-4f), coax_p_v2(0x1.1eb852p-5f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_v3(0x1.333334p-2f, 0x1.eb851ep-3f, 0x1.0a3d70p-2f), l_aa, l_line);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0x1.c28f5cp-5f, 0x1.1eb852p-4f), coax_p_v2(0x1.1eb852p-5f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_v3(0x1.333334p-2f, 0x1.eb851ep-3f, 0x1.0a3d70p-2f), l_aa, l_line);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.147ae2p-2f), coax_p_v2(0x1.eb851ep-4f, 0x1.0a3d70p-3f), 0x1.99999ap-5f), coax_p_stair_lit_col(coax_p_STAIR_STEEL(), l_q, coax_p_v2(0.0f, 0x1.147ae2p-2f), 0x1.1eb852p-3f), l_aa, l_line);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.d70a3ep-3f), coax_p_v2(0x1.c28f5cp-5f, 0x1.eb851ep-4f), 0x1.47ae14p-6f), coax_p_stair_lit_col(coax_p_v3(0x1.99999ap-3f, 0x1.70a3d8p-2f, 0x1.b33334p-1f), l_q, coax_p_v2(0.0f, 0x1.eb851ep-3f), 0x1.eb851ep-4f), l_aa, l_line);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.5c28f6p-3f), coax_p_v2(0x1.eb851ep-4f, 0x1.26e978p-6f), 0x1.0624dep-7f), coax_p_v3(0x1.851eb8p-2f, 0x1.0a3d70p-2f, 0x1.99999ap-3f), l_aa, l_line);
    l_ang = coax_p_mix(0x1.400000p0f, (-0x1.666666p-2f), l_flash);
    l_dir = coax_p_v2(cos(l_ang), sin(l_ang));
    l_hand = coax_p_v2(0x1.333334p-3f, 0x1.147ae2p-2f);
    l_tip = coax_p_wg_add_f2f2(l_hand, coax_p_wg_mul_f2f(l_dir, 0x1.70a3d8p-2f));
    l_blade = coax_p_stair_sd_seg(l_q, coax_p_wg_add_f2f2(l_hand, coax_p_wg_mul_f2f(l_dir, 0x1.99999ap-5f)), l_tip, 0x1.6872b0p-6f);
    coax_p_stair_paint(&l_pen_store, l_blade, coax_p_wg_mix_f3f3f(coax_p_v3(0x1.b33334p-1f, 0x1.ccccccp-1f, 0x1.000000p0f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_flash), l_aa, l_line);
    if ((l_flash > 0x1.99999ap-5f))
    {
      coax_p_stair_glow_at(&l_pen_store, l_blade, coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-1f, 0x1.800000p-1f, 0x1.000000p0f), l_flash), 0x1.99999ap-5f);
    }
    l_nrm = coax_p_v2((-l_dir.y), l_dir.x);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_wg_sub_f2f2(coax_p_wg_add_f2f2(l_hand, coax_p_wg_mul_f2f(l_dir, 0x1.99999ap-5f)), coax_p_wg_mul_f2f(l_nrm, 0x1.c28f5cp-5f)), coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(l_hand, coax_p_wg_mul_f2f(l_dir, 0x1.99999ap-5f)), coax_p_wg_mul_f2f(l_nrm, 0x1.c28f5cp-5f)), 0x1.cac084p-7f), coax_p_v3(0x1.e66666p-1f, 0x1.99999ap-1f, 0x1.666666p-2f), l_aa, l_line);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, l_hand, 0x1.0624dep-5f), coax_p_STAIR_SKIN(), l_aa, l_line);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2((-0x1.99999ap-4f), 0x1.000000p-2f), 0x1.ae147ap-4f), coax_p_stair_lit_col(coax_p_v3(0x1.8f5c28p-1f, 0x1.99999ap-1f, 0x1.b851ecp-1f), l_q, coax_p_v2((-0x1.99999ap-4f), 0x1.000000p-2f), 0x1.ae147ap-4f), l_aa, l_line);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2((-0x1.99999ap-4f), 0x1.000000p-2f), 0x1.333334p-4f), coax_p_stair_lit_col(coax_p_v3(0x1.70a3d8p-3f, 0x1.47ae14p-2f, 0x1.99999ap-1f), l_q, coax_p_v2((-0x1.99999ap-4f), 0x1.000000p-2f), 0x1.333334p-4f), l_aa, (l_line * 0x1.333334p-1f));
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2((-0x1.99999ap-4f), 0x1.000000p-2f), 0x1.6872b0p-6f), coax_p_v3(0x1.e66666p-1f, 0x1.a3d70ap-1f, 0x1.99999ap-2f), l_aa, (l_line * 0x1.333334p-1f));
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.47ae14p-7f, 0x1.000000p-1f), 0x1.78d4fep-4f), coax_p_stair_lit_col(coax_p_STAIR_SKIN(), l_q, coax_p_v2(0x1.47ae14p-7f, 0x1.000000p-1f), 0x1.78d4fep-4f), l_aa, l_line);
    coax_p_stair_paint(&l_pen_store, coax_p_fmax(coax_p_stair_sd_circle(l_q, coax_p_v2(0.0f, 0x1.07ae14p-1f), 0x1.a9fbe8p-4f), (0x1.07ae14p-1f - l_q.y)), coax_p_stair_lit_col(coax_p_STAIR_STEEL(), l_q, coax_p_v2(0.0f, 0x1.19999ap-1f), 0x1.99999ap-4f), l_aa, l_line);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.06a7f0p-1f), coax_p_v2(0x1.ba5e36p-4f, 0x1.cac084p-7f), 0x1.89374cp-8f), coax_p_v3(0x1.19999ap-1f, 0x1.28f5c2p-1f, 0x1.51eb86p-1f), l_aa, (l_line * 0x1.666666p-1f));
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.c28f5cp-5f, 0x1.eb851ep-2f), 0x1.cac084p-7f), coax_p_STAIR_INK(), l_aa, 0.0f);
    coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2(0.0f, 0x1.3d70a4p-1f), coax_p_v2((-0x1.99999ap-4f), 0x1.5c28f6p-1f), 0x1.cac084p-6f), coax_p_v3(0x1.b851ecp-1f, 0x1.47ae14p-3f, 0x1.c28f5cp-3f), l_aa, l_line);
  }
  else
  {
    if ((l_cls == 1i))
    {
      l_robe = coax_p_v3(0x1.c28f5cp-2f, 0x1.147ae2p-2f, 0x1.99999ap-1f);
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2(0x1.5c28f6p-3f, 0.0f), coax_p_v2(0x1.5c28f6p-3f, 0x1.666666p-1f), 0x1.26e978p-6f), coax_p_v3(0x1.d70a3ep-2f, 0x1.47ae14p-2f, 0x1.99999ap-3f), l_aa, l_line);
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_trap(l_q, coax_p_v2(0.0f, 0x1.99999ap-3f), 0x1.51eb86p-3f, 0x1.333334p-4f, 0x1.99999ap-3f), coax_p_stair_lit_col(l_robe, l_q, coax_p_v2(0.0f, 0x1.c28f5cp-3f), 0x1.99999ap-3f), l_aa, l_line);
      coax_p_stair_paint(&l_pen_store, coax_p_fmax(coax_p_stair_sd_trap(l_q, coax_p_v2(0.0f, 0x1.99999ap-3f), 0x1.51eb86p-3f, 0x1.333334p-4f, 0x1.99999ap-3f), (l_q.y - 0x1.70a3d8p-5f)), coax_p_v3(0x1.e66666p-1f, 0x1.a3d70ap-1f, 0x1.851eb8p-2f), l_aa, (l_line * 0x1.000000p-1f));
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2(0x1.eb851ep-6f, 0x1.3d70a4p-2f), coax_p_v2(0x1.333334p-3f, 0x1.147ae2p-2f), 0x1.581062p-5f), coax_p_stair_lit_col(l_robe, l_q, coax_p_v2(0x1.70a3d8p-4f, 0x1.333334p-2f), 0x1.47ae14p-4f), l_aa, l_line);
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.47ae14p-3f, 0x1.147ae2p-2f), 0x1.0624dep-5f), coax_p_STAIR_SKIN(), l_aa, l_line);
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0.0f, 0x1.d70a3ep-2f), 0x1.5c28f6p-4f), coax_p_stair_lit_col(coax_p_STAIR_SKIN(), l_q, coax_p_v2(0.0f, 0x1.d70a3ep-2f), 0x1.5c28f6p-4f), l_aa, l_line);
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_trap(l_q, coax_p_v2(0x1.eb851ep-7f, 0x1.851eb8p-2f), 0x1.89374cp-7f, 0x1.1eb852p-4f, 0x1.eb851ep-5f), coax_p_v3(0x1.dc28f6p-1f, 0x1.dc28f6p-1f, 0x1.eb851ep-1f), l_aa, l_line);
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.70a3d8p-5f, 0x1.e147aep-2f), 0x1.a9fbe8p-7f), coax_p_STAIR_INK(), l_aa, 0.0f);
      l_hq = coax_p_v2((l_q.x + (0x1.47ae14p-2f * coax_p_fmax((l_q.y - 0x1.0f5c28p-1f), 0.0f))), l_q.y);
      l_hat = coax_p_v3(0x1.147ae2p-2f, 0x1.70a3d8p-3f, 0x1.3d70a4p-1f);
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_trap(l_hq, coax_p_v2((-0x1.47ae14p-8f), 0x1.5c28f6p-1f), 0x1.99999ap-4f, 0x1.89374cp-8f, 0x1.333334p-3f), coax_p_stair_lit_col(l_hat, l_hq, coax_p_v2(0.0f, 0x1.3d70a4p-1f), 0x1.eb851ep-4f), l_aa, l_line);
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.11eb86p-1f), coax_p_v2(0x1.3d70a4p-3f, 0x1.26e978p-6f), 0x1.26e978p-6f), coax_p_wg_mul_f3f(l_hat, 0x1.b33334p-1f), l_aa, l_line);
      coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_hq, coax_p_v2(0.0f, 0x1.428f5cp-1f), 0x1.47ae14p-6f), coax_p_v3(0x1.000000p0f, 0x1.d70a3ep-1f, 0x1.ccccccp-2f), l_aa, 0.0f);
      l_orb = coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.5c28f6p-3f, 0x1.800000p-1f), 0x1.89374cp-5f);
      l_oc = coax_p_v3(0x1.3d70a4p-1f, 0x1.8f5c28p-1f, 0x1.000000p0f);
      coax_p_stair_paint(&l_pen_store, l_orb, coax_p_wg_mix_f3f3f(l_oc, coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), (0x1.666666p-2f + (0x1.99999ap-2f * l_flash))), l_aa, (l_line * 0x1.333334p-1f));
      coax_p_stair_glow_at(&l_pen_store, l_orb, coax_p_wg_mul_f3f(l_oc, ((0x1.19999ap-1f + (0x1.333334p0f * l_flash)) + (0x1.333334p-3f * sin(((coax_at_u(i32(2i)) * 0x1.800000p1f) + l_seed))))), (0x1.99999ap-5f + (0x1.47ae14p-5f * l_flash)));
    }
    else
    {
      if ((l_cls == 3i))
      {
        l_cloak = coax_p_v3(0x1.99999ap-3f, 0x1.d70a3ep-3f, 0x1.147ae2p-2f);
        coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2((-0x1.99999ap-5f), 0x1.1eb852p-4f), coax_p_v2(0x1.eb851ep-6f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_v3(0x1.47ae14p-3f, 0x1.333334p-3f, 0x1.5c28f6p-3f), l_aa, l_line);
        coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0x1.99999ap-5f, 0x1.1eb852p-4f), coax_p_v2(0x1.eb851ep-6f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_v3(0x1.47ae14p-3f, 0x1.333334p-3f, 0x1.5c28f6p-3f), l_aa, l_line);
        coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_trap(l_q, coax_p_v2((-0x1.47ae14p-7f), 0x1.000000p-2f), 0x1.333334p-3f, 0x1.47ae14p-4f, 0x1.333334p-3f), coax_p_stair_lit_col(l_cloak, l_q, coax_p_v2(0.0f, 0x1.147ae2p-2f), 0x1.333334p-3f), l_aa, l_line);
        coax_p_stair_paint(&l_pen_store, coax_p_fmax(coax_p_stair_sd_trap(l_q, coax_p_v2((-0x1.47ae14p-7f), 0x1.000000p-2f), 0x1.333334p-3f, 0x1.47ae14p-4f, 0x1.333334p-3f), ((-l_q.x) - 0x1.47ae14p-6f)), coax_p_stair_lit_col(coax_p_v3(0x1.333334p-2f, 0x1.3d70a4p-1f, 0x1.19999ap-1f), l_q, coax_p_v2(0x1.99999ap-5f, 0x1.000000p-2f), 0x1.eb851ep-4f), l_aa, (l_line * 0x1.000000p-1f));
        coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.99999ap-3f), coax_p_v2(0x1.99999ap-4f, 0x1.cac084p-7f), 0x1.89374cp-8f), coax_p_v3(0x1.ae147ap-2f, 0x1.333334p-2f, 0x1.c28f5cp-3f), l_aa, (l_line * 0x1.333334p-1f));
        l_ang_24 = coax_p_mix((-0x1.99999ap-3f), 0x1.19999ap-1f, l_flash);
        l_dir_25 = coax_p_v2(cos(l_ang_24), sin(l_ang_24));
        l_hand_26 = coax_p_wg_add_f2f2(coax_p_v2(0x1.0a3d70p-3f, 0x1.c28f5cp-3f), coax_p_wg_mul_f2f(coax_p_wg_mul_f2f(l_dir_25, 0x1.47ae14p-5f), l_flash));
        l_blade_27 = coax_p_stair_sd_seg(l_q, coax_p_wg_add_f2f2(l_hand_26, coax_p_wg_mul_f2f(l_dir_25, 0x1.eb851ep-6f)), coax_p_wg_add_f2f2(l_hand_26, coax_p_wg_mul_f2f(l_dir_25, 0x1.99999ap-3f)), 0x1.0624dep-6f);
        coax_p_stair_paint(&l_pen_store, l_blade_27, coax_p_wg_mix_f3f3f(coax_p_v3(0x1.8f5c28p-1f, 0x1.b851ecp-1f, 0x1.c28f5cp-1f), coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), l_flash), l_aa, l_line);
        if ((l_flash > 0x1.99999ap-5f))
        {
          coax_p_stair_glow_at(&l_pen_store, l_blade_27, coax_p_wg_mul_f3f(coax_p_v3(0x1.ccccccp-2f, 0x1.e66666p-1f, 0x1.99999ap-1f), l_flash), 0x1.47ae14p-5f);
        }
        coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, l_hand_26, 0x1.eb851ep-6f), coax_p_wg_mul_f3f(coax_p_STAIR_SKIN(), 0x1.ccccccp-1f), l_aa, l_line);
        coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.eb851ep-7f, 0x1.e147aep-2f), 0x1.5c28f6p-4f), coax_p_stair_lit_col(coax_p_STAIR_SKIN(), l_q, coax_p_v2(0x1.eb851ep-7f, 0x1.e147aep-2f), 0x1.5c28f6p-4f), l_aa, l_line);
        coax_p_stair_paint(&l_pen_store, coax_p_fmax(coax_p_stair_sd_circle(l_q, coax_p_v2((-0x1.47ae14p-7f), 0x1.f5c290p-2f), 0x1.c28f5cp-4f), ((((((-(l_q.x - 0x1.c28f5cp-5f)) * 0x1.000000p0f) - ((0x1.c28f5cp-2f - l_q.y) * 0.0f)) + 0.0f) - 0.0f) + coax_p_fmin(0.0f, (l_q.y - 0x1.c28f5cp-2f)))), coax_p_stair_lit_col(coax_p_wg_mul_f3f(l_cloak, 0x1.19999ap0f), l_q, coax_p_v2(0.0f, 0x1.0a3d70p-1f), 0x1.c28f5cp-4f), l_aa, l_line);
        coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0x1.99999ap-5f, 0x1.ccccccp-2f), coax_p_v2(0x1.47ae14p-5f, 0x1.89374cp-7f), 0x1.89374cp-8f), coax_p_v3(0x1.eb851ep-4f, 0x1.0a3d70p-3f, 0x1.333334p-3f), l_aa, 0.0f);
        coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.eb851ep-5f, 0x1.e147aep-2f), 0x1.89374cp-7f), coax_p_v3(0x1.333334p-1f, 0x1.000000p0f, 0x1.b33334p-1f), l_aa, 0.0f);
      }
      else
      {
        if ((l_cls == 4i))
        {
          l_green = coax_p_v3(0x1.c28f5cp-3f, 0x1.eb851ep-2f, 0x1.eb851ep-3f);
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2((-0x1.99999ap-5f), 0x1.1eb852p-4f), coax_p_v2(0x1.0624dep-5f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_v3(0x1.70a3d8p-2f, 0x1.0a3d70p-2f, 0x1.70a3d8p-3f), l_aa, l_line);
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0x1.99999ap-5f, 0x1.1eb852p-4f), coax_p_v2(0x1.0624dep-5f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_v3(0x1.70a3d8p-2f, 0x1.0a3d70p-2f, 0x1.70a3d8p-3f), l_aa, l_line);
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.0a3d70p-2f), coax_p_v2(0x1.99999ap-4f, 0x1.eb851ep-4f), 0x1.47ae14p-5f), coax_p_stair_lit_col(coax_p_v3(0x1.000000p-1f, 0x1.851eb8p-2f, 0x1.000000p-2f), l_q, coax_p_v2(0.0f, 0x1.0a3d70p-2f), 0x1.eb851ep-4f), l_aa, l_line);
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_trap(l_q, coax_p_v2((-0x1.eb851ep-5f), 0x1.1eb852p-2f), 0x1.70a3d8p-4f, 0x1.99999ap-5f, 0x1.47ae14p-3f), coax_p_stair_lit_col(l_green, l_q, coax_p_v2((-0x1.eb851ep-5f), 0x1.333334p-2f), 0x1.1eb852p-3f), l_aa, l_line);
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2((-0x1.c28f5cp-4f), 0x1.eb851ep-3f), coax_p_v2((-0x1.99999ap-5f), 0x1.c28f5cp-2f), 0x1.99999ap-6f), coax_p_v3(0x1.ae147ap-2f, 0x1.1eb852p-2f, 0x1.70a3d8p-3f), l_aa, l_line);
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2((-0x1.99999ap-5f), 0x1.c28f5cp-2f), coax_p_v2((-0x1.eb851ep-6f), 0x1.000000p-1f), 0x1.0624dep-7f), coax_p_v3(0x1.ccccccp-1f, 0x1.ccccccp-1f, 0x1.b33334p-1f), l_aa, 0.0f);
          l_bc = coax_p_v2(0x1.70a3d8p-3f, 0x1.47ae14p-2f);
          l_bd = coax_p_wg_length2(coax_p_wg_mul_f2f2(coax_p_wg_sub_f2f2(l_q, l_bc), coax_p_v2(0x1.99999ap0f, 0x1.000000p0f)));
          l_bow = coax_p_fmax((abs((l_bd - 0x1.99999ap-3f)) - 0x1.89374cp-7f), ((l_bc.x - l_q.x) + 0x1.47ae14p-6f));
          coax_p_stair_paint(&l_pen_store, l_bow, coax_p_v3(0x1.19999ap-1f, 0x1.70a3d8p-2f, 0x1.99999ap-3f), l_aa, l_line);
          l_pull = (0x1.eb851ep-6f + (0x1.1eb852p-4f * l_flash));
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2(0x1.851eb8p-3f, 0x1.0a3d70p-3f), coax_p_v2((0x1.851eb8p-3f - l_pull), 0x1.47ae14p-2f), 0x1.89374cp-8f), coax_p_v3(0x1.e66666p-1f, 0x1.e66666p-1f, 0x1.ccccccp-1f), l_aa, 0.0f);
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2(0x1.851eb8p-3f, 0x1.051eb8p-1f), coax_p_v2((0x1.851eb8p-3f - l_pull), 0x1.47ae14p-2f), 0x1.89374cp-8f), coax_p_v3(0x1.e66666p-1f, 0x1.e66666p-1f, 0x1.ccccccp-1f), l_aa, 0.0f);
          if ((l_flash > 0x1.99999ap-5f))
          {
            coax_p_stair_glow_at(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2((0x1.851eb8p-3f - l_pull), 0x1.47ae14p-2f), coax_p_v2(0x1.ae147ap-2f, 0x1.47ae14p-2f), 0x1.0624dep-7f), coax_p_wg_mul_f3f(coax_p_v3(0x1.333334p-1f, 0x1.000000p0f, 0x1.19999ap-1f), l_flash), 0x1.eb851ep-6f);
          }
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2((0x1.851eb8p-3f - l_pull), 0x1.47ae14p-2f), 0x1.eb851ep-6f), coax_p_STAIR_SKIN(), l_aa, l_line);
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.47ae14p-7f, 0x1.eb851ep-2f), 0x1.5c28f6p-4f), coax_p_stair_lit_col(coax_p_STAIR_SKIN(), l_q, coax_p_v2(0x1.47ae14p-7f, 0x1.eb851ep-2f), 0x1.5c28f6p-4f), l_aa, l_line);
          coax_p_stair_paint(&l_pen_store, coax_p_fmax(coax_p_stair_sd_circle(l_q, coax_p_v2((-0x1.47ae14p-7f), 0x1.000000p-1f), 0x1.ae147ap-4f), ((0x1.e147aep-2f - l_q.y) - ((l_q.x + 0x1.eb851ep-5f) * 0x1.333334p-1f))), coax_p_stair_lit_col(l_green, l_q, coax_p_v2(0.0f, 0x1.147ae2p-1f), 0x1.99999ap-4f), l_aa, l_line);
          coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.c28f5cp-5f, 0x1.e147aep-2f), 0x1.a9fbe8p-7f), coax_p_STAIR_INK(), l_aa, 0.0f);
        }
        else
        {
          if ((l_cls == 5i))
          {
            l_rose = coax_p_v3(0x1.8f5c28p-1f, 0x1.000000p-2f, 0x1.eb851ep-2f);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2((-0x1.99999ap-5f), 0x1.1eb852p-4f), coax_p_v2(0x1.eb851ep-6f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_v3(0x1.e66666p-1f, 0x1.ccccccp-1f, 0x1.99999ap-1f), l_aa, l_line);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0x1.99999ap-5f, 0x1.1eb852p-4f), coax_p_v2(0x1.eb851ep-6f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_v3(0x1.e66666p-1f, 0x1.ccccccp-1f, 0x1.99999ap-1f), l_aa, l_line);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.0a3d70p-2f), coax_p_v2(0x1.c28f5cp-4f, 0x1.eb851ep-4f), 0x1.99999ap-5f), coax_p_stair_lit_col(l_rose, l_q, coax_p_v2(0.0f, 0x1.0a3d70p-2f), 0x1.0a3d70p-3f), l_aa, l_line);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.0a3d70p-2f), coax_p_v2(0x1.26e978p-6f, 0x1.c28f5cp-4f), 0x1.0624dep-7f), coax_p_v3(0x1.f5c290p-1f, 0x1.b851ecp-1f, 0x1.000000p-1f), l_aa, (l_line * 0x1.000000p-1f));
            l_lb = coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.99999ap-5f, 0x1.99999ap-3f), 0x1.5c28f6p-4f);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2(0x1.47ae14p-4f, 0x1.eb851ep-3f), coax_p_v2(0x1.0a3d70p-2f, 0x1.ae147ap-2f), 0x1.47ae14p-6f), coax_p_v3(0x1.ccccccp-2f, 0x1.1eb852p-2f, 0x1.47ae14p-3f), l_aa, l_line);
            coax_p_stair_paint(&l_pen_store, l_lb, coax_p_stair_lit_col(coax_p_v3(0x1.8f5c28p-1f, 0x1.19999ap-1f, 0x1.333334p-2f), l_q, coax_p_v2(0x1.99999ap-5f, 0x1.99999ap-3f), 0x1.5c28f6p-4f), l_aa, l_line);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.99999ap-5f, 0x1.99999ap-3f), 0x1.6872b0p-6f), coax_p_v3(0x1.99999ap-3f, 0x1.eb851ep-4f, 0x1.47ae14p-4f), l_aa, 0.0f);
            if ((l_flash > 0x1.99999ap-5f))
            {
              coax_p_stair_glow_at(&l_pen_store, (abs(((coax_p_wg_length2(coax_p_wg_sub_f2f2(l_q, coax_p_v2(0x1.99999ap-5f, 0x1.99999ap-3f))) - 0x1.1eb852p-3f) - (0x1.47ae14p-4f * l_flash))) - 0x1.0624dep-8f), coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.333334p-1f, 0x1.b33334p-1f), l_flash), 0x1.eb851ep-6f);
            }
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.0a3d70p-3f, (0x1.eb851ep-3f + (0x1.47ae14p-6f * l_flash))), 0x1.eb851ep-6f), coax_p_STAIR_SKIN(), l_aa, l_line);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0.0f, 0x1.e147aep-2f), 0x1.5c28f6p-4f), coax_p_stair_lit_col(coax_p_STAIR_SKIN(), l_q, coax_p_v2(0.0f, 0x1.e147aep-2f), 0x1.5c28f6p-4f), l_aa, l_line);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.70a3d8p-5f, 0x1.e66666p-2f), 0x1.a9fbe8p-7f), coax_p_STAIR_INK(), l_aa, 0.0f);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2((-0x1.47ae14p-7f), 0x1.1eb852p-1f), coax_p_v2(0x1.c28f5cp-4f, 0x1.1eb852p-5f), 0x1.eb851ep-6f), coax_p_stair_lit_col(coax_p_wg_mul_f3f(l_rose, 0x1.99999ap-1f), l_q, coax_p_v2(0.0f, 0x1.23d70ap-1f), 0x1.99999ap-4f), l_aa, l_line);
            coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2((-0x1.47ae14p-4f), 0x1.28f5c2p-1f), coax_p_v2((-0x1.99999ap-3f), 0x1.666666p-1f), 0x1.0624dep-6f), coax_p_v3(0x1.f5c290p-1f, 0x1.e66666p-1f, 0x1.ccccccp-1f), l_aa, (l_line * 0x1.333334p-1f));
          }
          else
          {
            if ((l_cls == 6i))
            {
              l_plate = coax_p_v3(0x1.ae147ap-1f, 0x1.bd70a4p-1f, 0x1.e147aep-1f);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2((-0x1.c28f5cp-5f), 0x1.1eb852p-4f), coax_p_v2(0x1.1eb852p-5f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_stair_lit_col(coax_p_wg_mul_f3f(l_plate, 0x1.99999ap-1f), l_q, coax_p_v2((-0x1.99999ap-5f), 0x1.47ae14p-4f), 0x1.47ae14p-4f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0x1.c28f5cp-5f, 0x1.1eb852p-4f), coax_p_v2(0x1.1eb852p-5f, 0x1.333334p-4f), 0x1.47ae14p-6f), coax_p_stair_lit_col(coax_p_wg_mul_f3f(l_plate, 0x1.99999ap-1f), l_q, coax_p_v2(0x1.99999ap-5f, 0x1.47ae14p-4f), 0x1.47ae14p-4f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.147ae2p-2f), coax_p_v2(0x1.000000p-3f, 0x1.0a3d70p-3f), 0x1.99999ap-5f), coax_p_stair_lit_col(l_plate, l_q, coax_p_v2(0.0f, 0x1.147ae2p-2f), 0x1.1eb852p-3f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.c28f5cp-3f), coax_p_v2(0x1.0a3d70p-4f, 0x1.0a3d70p-3f), 0x1.47ae14p-6f), coax_p_stair_lit_col(coax_p_v3(0x1.f0a3d8p-1f, 0x1.f0a3d8p-1f, 0x1.e66666p-1f), l_q, coax_p_v2(0.0f, 0x1.eb851ep-3f), 0x1.eb851ep-4f), l_aa, l_line);
              l_mark = coax_p_stair_sd_circle(l_q, coax_p_v2(0.0f, 0x1.147ae2p-2f), 0x1.eb851ep-6f);
              coax_p_stair_paint(&l_pen_store, l_mark, coax_p_v3(0x1.19999ap-1f, 0x1.8f5c28p-1f, 0x1.000000p0f), l_aa, (l_line * 0x1.000000p-1f));
              l_ang_37 = coax_p_mix(0x1.733334p0f, (-0x1.99999ap-3f), l_flash);
              l_dir_38 = coax_p_v2(cos(l_ang_37), sin(l_ang_37));
              l_hand_39 = coax_p_v2(0x1.333334p-3f, 0x1.333334p-2f);
              l_blade_40 = coax_p_stair_sd_seg(l_q, coax_p_wg_add_f2f2(l_hand_39, coax_p_wg_mul_f2f(l_dir_38, 0x1.99999ap-5f)), coax_p_wg_add_f2f2(l_hand_39, coax_p_wg_mul_f2f(l_dir_38, 0x1.99999ap-2f)), 0x1.89374cp-6f);
              coax_p_stair_paint(&l_pen_store, l_blade_40, coax_p_v3(0x1.d70a3ep-1f, 0x1.e66666p-1f, 0x1.000000p0f), l_aa, l_line);
              coax_p_stair_glow_at(&l_pen_store, l_blade_40, coax_p_wg_mul_f3f(coax_p_v3(0x1.666666p-1f, 0x1.b33334p-1f, 0x1.000000p0f), (0x1.333334p-3f + l_flash)), (0x1.47ae14p-5f + (0x1.eb851ep-6f * l_flash)));
              l_nrm_41 = coax_p_v2((-l_dir_38.y), l_dir_38.x);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_wg_sub_f2f2(coax_p_wg_add_f2f2(l_hand_39, coax_p_wg_mul_f2f(l_dir_38, 0x1.99999ap-5f)), coax_p_wg_mul_f2f(l_nrm_41, 0x1.eb851ep-5f)), coax_p_wg_add_f2f2(coax_p_wg_add_f2f2(l_hand_39, coax_p_wg_mul_f2f(l_dir_38, 0x1.99999ap-5f)), coax_p_wg_mul_f2f(l_nrm_41, 0x1.eb851ep-5f)), 0x1.eb851ep-7f), coax_p_v3(0x1.e66666p-1f, 0x1.b33334p-1f, 0x1.ccccccp-2f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, l_hand_39, 0x1.0624dep-5f), l_plate, l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.47ae14p-7f, 0x1.000000p-1f), 0x1.78d4fep-4f), coax_p_stair_lit_col(coax_p_STAIR_SKIN(), l_q, coax_p_v2(0x1.47ae14p-7f, 0x1.000000p-1f), 0x1.78d4fep-4f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_fmax(coax_p_stair_sd_circle(l_q, coax_p_v2(0.0f, 0x1.0a3d70p-1f), 0x1.b22d0ep-4f), (0x1.000000p-1f - l_q.y)), coax_p_stair_lit_col(l_plate, l_q, coax_p_v2(0.0f, 0x1.1eb852p-1f), 0x1.99999ap-4f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.c28f5cp-5f, 0x1.eb851ep-2f), 0x1.cac084p-7f), coax_p_STAIR_INK(), l_aa, 0.0f);
              coax_p_stair_glow_at(&l_pen_store, (coax_p_wg_length2(coax_p_wg_mul_f2f2(coax_p_wg_sub_f2f2(l_q, coax_p_v2(0.0f, 0x1.333334p-2f)), coax_p_v2(0x1.000000p0f, 0x1.99999ap-1f))) - 0x1.000000p-2f), coax_p_wg_mul_f3f(coax_p_v3(0x1.19999ap-1f, 0x1.800000p-1f, 0x1.000000p0f), (0x1.eb851ep-4f + (0x1.eb851ep-5f * sin(((coax_at_u(i32(2i)) * 0x1.000000p1f) + l_seed))))), 0x1.eb851ep-5f);
            }
            else
            {
              l_robe_42 = coax_p_v3(0x1.ccccccp-1f, 0x1.c28f5cp-1f, 0x1.99999ap-1f);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_trap(l_q, coax_p_v2(0.0f, 0x1.99999ap-3f), 0x1.5c28f6p-3f, 0x1.70a3d8p-4f, 0x1.99999ap-3f), coax_p_stair_lit_col(l_robe_42, l_q, coax_p_v2(0.0f, 0x1.c28f5cp-3f), 0x1.99999ap-3f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_box(l_q, coax_p_v2(0.0f, 0x1.99999ap-3f), coax_p_v2(0x1.0624dep-5f, 0x1.851eb8p-3f), 0x1.47ae14p-7f), coax_p_v3(0x1.dc28f6p-1f, 0x1.8f5c28p-1f, 0x1.333334p-2f), l_aa, (l_line * 0x1.000000p-1f));
              l_sun = coax_p_stair_sd_circle(l_q, coax_p_v2(0.0f, 0x1.28f5c2p-2f), 0x1.581062p-5f);
              coax_p_stair_paint(&l_pen_store, l_sun, coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.70a3d8p-2f), l_aa, (l_line * 0x1.333334p-1f));
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2(0x1.1eb852p-3f, 0x1.70a3d8p-3f), coax_p_v2(0x1.851eb8p-3f, 0x1.d70a3ep-2f), 0x1.eb851ep-7f), coax_p_v3(0x1.ccccccp-2f, 0x1.333334p-2f, 0x1.99999ap-3f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.8f5c28p-3f, 0x1.f5c290p-2f), 0x1.581062p-5f), coax_p_stair_lit_col(coax_p_STAIR_STEEL(), l_q, coax_p_v2(0x1.8f5c28p-3f, 0x1.f5c290p-2f), 0x1.581062p-5f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_seg(l_q, coax_p_v2(0x1.eb851ep-6f, 0x1.3d70a4p-2f), coax_p_v2(0x1.1eb852p-3f, 0x1.0a3d70p-2f), 0x1.581062p-5f), coax_p_stair_lit_col(l_robe_42, l_q, coax_p_v2(0x1.70a3d8p-4f, 0x1.333334p-2f), 0x1.47ae14p-4f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.28f5c2p-3f, 0x1.000000p-2f), 0x1.0624dep-5f), coax_p_STAIR_SKIN(), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2((-0x1.eb851ep-7f), 0x1.e147aep-2f), 0x1.ba5e36p-4f), coax_p_stair_lit_col(coax_p_v3(0x1.99999ap-1f, 0x1.8a3d70p-1f, 0x1.666666p-1f), l_q, coax_p_v2((-0x1.eb851ep-7f), 0x1.eb851ep-2f), 0x1.c28f5cp-4f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.6872b0p-6f, 0x1.d1eb86p-2f), 0x1.2f1aa0p-4f), coax_p_stair_lit_col(coax_p_STAIR_SKIN(), l_q, coax_p_v2(0x1.6872b0p-6f, 0x1.d1eb86p-2f), 0x1.2f1aa0p-4f), l_aa, l_line);
              coax_p_stair_paint(&l_pen_store, coax_p_stair_sd_circle(l_q, coax_p_v2(0x1.db22d0p-5f, 0x1.dc28f6p-2f), 0x1.a9fbe8p-7f), coax_p_STAIR_INK(), l_aa, 0.0f);
              l_halo = (abs((coax_p_wg_length2(coax_p_wg_mul_f2f2(coax_p_wg_sub_f2f2(l_q, coax_p_v2((-0x1.47ae14p-7f), 0x1.3d70a4p-1f)), coax_p_v2(0x1.000000p0f, 0x1.99999ap1f))) - 0x1.99999ap-4f)) - 0x1.89374cp-7f);
              coax_p_stair_paint(&l_pen_store, l_halo, coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.000000p-1f), l_aa, 0.0f);
              coax_p_stair_glow_at(&l_pen_store, l_halo, coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.ccccccp-2f), (0x1.666666p-2f + (0x1.666666p0f * l_flash))), (0x1.eb851ep-6f + (0x1.99999ap-5f * l_flash)));
              if ((l_flash > 0x1.99999ap-5f))
              {
                coax_p_stair_glow_at(&l_pen_store, l_sun, coax_p_wg_mul_f3f(coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.000000p-1f), l_flash), 0x1.eb851ep-5f);
              }
            }
          }
        }
      }
    }
  }
  return l_pen_store[0i];
}
fn coax_p_wg_clamp_i(p_x: i32, p_lo: i32, p_hi: i32) -> i32 {
  var l_x: i32 = p_x;
  var l_lo: i32 = p_lo;
  var l_hi: i32 = p_hi;
  return coax_p_wg_min_i(coax_p_wg_max_i(l_x, l_lo), l_hi);
}
fn coax_p_wg_div_f2f(p_a: vec2f, p_b: f32) -> vec2f {
  var l_a: vec2f = p_a;
  var l_b: f32 = p_b;
  return coax_p_v2((l_a.x / l_b), (l_a.y / l_b));
}
fn coax_p_stair_sd_trap(p_p0: vec2f, p_c: vec2f, p_r1: f32, p_r2: f32, p_he: f32) -> f32 {
  var l_p0: vec2f = p_p0;
  var l_c: vec2f = p_c;
  var l_r1: f32 = p_r1;
  var l_r2: f32 = p_r2;
  var l_he: f32 = p_he;
  var l_p: vec2f;
  var l_k1: vec2f;
  var l_k2: vec2f;
  var l_ca: vec2f;
  var l_cb: vec2f;
  var l_s: f32;
  l_p = coax_p_wg_sub_f2f2(l_p0, l_c);
  l_k1 = coax_p_v2(l_r2, l_he);
  l_k2 = coax_p_v2((l_r2 - l_r1), (0x1.000000p1f * l_he));
  l_p.x = abs(l_p.x);
  l_ca = coax_p_v2((l_p.x - coax_p_fmin(l_p.x, select(l_r2, l_r1, (l_p.y < 0.0f)))), (abs(l_p.y) - l_he));
  l_cb = coax_p_wg_add_f2f2(coax_p_wg_sub_f2f2(l_p, l_k1), coax_p_wg_mul_f2f(l_k2, coax_p_clampf((coax_p_wg_dot2(coax_p_wg_sub_f2f2(l_k1, l_p), l_k2) / coax_p_wg_dot2(l_k2, l_k2)), 0.0f, 0x1.000000p0f)));
  l_s = select(0x1.000000p0f, (-0x1.000000p0f), ((l_cb.x < 0.0f) && (l_ca.y < 0.0f)));
  return (l_s * sqrt(coax_p_fmin(coax_p_wg_dot2(l_ca, l_ca), coax_p_wg_dot2(l_cb, l_cb))));
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
fn coax_p_wg_abs_f2(p_p0: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  return coax_p_v2(abs(l_p0.x), abs(l_p0.y));
}
fn coax_p_wg_max_f2f2(p_p0: vec2f, p_p1: vec2f) -> vec2f {
  var l_p0: vec2f = p_p0;
  var l_p1: vec2f = p_p1;
  return coax_p_v2(coax_p_fmax(l_p0.x, l_p1.x), coax_p_fmax(l_p0.y, l_p1.y));
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
fn coax_p_stair_paint(p_pen: ptr<function, array<coax_t13, 1>>, p_d: f32, p_col: vec3f, p_aa: f32, p_line: f32) {
  var l_d: f32 = p_d;
  var l_col: vec3f = p_col;
  var l_aa: f32 = p_aa;
  var l_line: f32 = p_line;
  var l_fill: f32;
  var l_edge: f32;
  l_fill = coax_p_clampf((0x1.000000p-1f - (l_d / l_aa)), 0.0f, 0x1.000000p0f);
  if ((l_fill <= 0.0f))
  {
    return;
  }
  l_edge = coax_p_clampf((0x1.000000p-1f + ((l_line + l_d) / l_aa)), 0.0f, 0x1.000000p0f);
  (*p_pen)[0i].x = coax_p_wg_mix_f3f3f((*p_pen)[0i].x, coax_p_wg_mix_f3f3f(l_col, coax_p_STAIR_INK(), (l_edge * 0x1.ccccccp-1f)), l_fill);
  (*p_pen)[0i].y = coax_p_fmax((*p_pen)[0i].y, l_fill);
}
fn coax_p_STAIR_STEEL() -> vec3f {
  return coax_p_v3(0x1.51eb86p-1f, 0x1.6b851ep-1f, 0x1.99999ap-1f);
}
fn coax_p_stair_lit_col(p_col: vec3f, p_q: vec2f, p_c: vec2f, p_r: f32) -> vec3f {
  var l_col: vec3f = p_col;
  var l_q: vec2f = p_q;
  var l_c: vec2f = p_c;
  var l_r: f32 = p_r;
  var l_n: vec2f;
  l_n = coax_p_wg_div_f2f(coax_p_wg_sub_f2f2(l_q, l_c), l_r);
  return coax_p_wg_mul_f3f(l_col, (0x1.70a3d8p-1f + (0x1.000000p-1f * coax_p_clampf(((coax_p_wg_dot2(l_n, coax_p_v2((-0x1.19999ap-1f), 0x1.4cccccp-1f)) * 0x1.000000p-1f) + 0x1.000000p-1f), 0.0f, 0x1.000000p0f))));
}
fn coax_p_stair_sd_seg(p_p: vec2f, p_a: vec2f, p_b: vec2f, p_r: f32) -> f32 {
  var l_p: vec2f = p_p;
  var l_a: vec2f = p_a;
  var l_b: vec2f = p_b;
  var l_r: f32 = p_r;
  var l_pa: vec2f;
  var l_ba: vec2f;
  var l_h: f32;
  l_pa = coax_p_wg_sub_f2f2(l_p, l_a);
  l_ba = coax_p_wg_sub_f2f2(l_b, l_a);
  l_h = coax_p_clampf((coax_p_wg_dot2(l_pa, l_ba) / coax_p_wg_dot2(l_ba, l_ba)), 0.0f, 0x1.000000p0f);
  return (coax_p_wg_length2(coax_p_wg_sub_f2f2(l_pa, coax_p_wg_mul_f2f(l_ba, l_h))) - l_r);
}
fn coax_p_stair_glow_at(p_pen: ptr<function, array<coax_t13, 1>>, p_d: f32, p_col: vec3f, p_spread: f32) {
  var l_d: f32 = p_d;
  var l_col: vec3f = p_col;
  var l_spread: f32 = p_spread;
  (*p_pen)[0i].z = coax_p_wg_add_f3f3((*p_pen)[0i].z, coax_p_wg_mul_f3f(coax_p_wg_mul_f3f(l_col, exp(((-coax_p_fmax(l_d, 0.0f)) / l_spread))), 0x1.333334p-1f));
}
fn coax_p_STAIR_SKIN() -> vec3f {
  return coax_p_v3(0x1.dc28f6p-1f, 0x1.851eb8p-1f, 0x1.428f5cp-1f);
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
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_9) -> coax_io_11 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t9(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> @location(0) vec4f {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5));
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