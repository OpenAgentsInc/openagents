// ---- generated: everglade/scene/gpu/fivelab.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
  f5: vec4f,
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
  y: vec2f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
}
struct coax_io_17 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) z: vec4f,
  @location(2) w: vec4f,
  @location(3) f4: vec4f,
}
struct coax_t18 {
  x: vec4f,
  y: f32,
}
struct coax_out_18 {
  @location(0) x: vec4f,
  @builtin(frag_depth) y: f32,
}
struct coax_t22 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
  f4: vec4f,
  f5: vec4f,
  f6: vec4f,
  f7: vec4f,
  f8: vec4f,
  f9: vec4f,
  f10: vec4f,
  f11: vec4f,
}
struct coax_t23 {
  x: vec3f,
  y: vec3f,
}
struct coax_t25 {
  x: array<vec4f, 27>,
}
struct coax_t26 {
  x: f32,
  y: vec3f,
  z: f32,
  w: f32,
}
struct coax_t29 {
  x: i32,
  y: f32,
  z: f32,
  w: f32,
  f4: array<f32, 3>,
}
struct coax_t30 {
  x: u32,
  y: f32,
  z: f32,
}
struct coax_t33 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
}
struct coax_t34 {
  x: bool,
  y: vec4f,
}
struct coax_t36 {
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
@group(1) @binding(1) var<storage, read> objects: array<coax_t11>;
@group(1) @binding(2) var<storage, read> bake: array<vec4f>;
@group(1) @binding(3) var<storage, read> mats: array<vec4f>;
@group(0) @binding(1) var<storage, read> frame: array<f32>;
@group(1) @binding(4) var sheets: texture_2d_array<f32>;
@group(1) @binding(5) var sheet: sampler;
@group(0) @binding(2) var sans_atlas: texture_2d<f32>;
@group(0) @binding(3) var sans_sampler: sampler;
fn coax_p_bg_vs(p_i: u32, p__iid1: u32) -> vec4f {
  let l_i: u32 = p_i;
  let l__iid1: u32 = p__iid1;
  var l_p: array<vec2f, 3>;
  l_p = array<vec2f, 3>(coax_p_v2((-0x1.000000p0f), (-0x1.800000p1f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.800000p1f, 0x1.000000p0f));
  return coax_p_v4(l_p[i32(l_i)].x, l_p[i32(l_i)].y, 0.0f, 0x1.000000p0f);
}
fn coax_p_scene_vs(p_id: u32, p_instance: u32) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  return coax_p_fivelab_scene_place(l_id, l_instance, coax_p_glade_frame());
}
fn coax_p_fx_vs(p__vid2: u32, p__iid3: u32, p_in_: coax_t16) -> coax_t17 {
  let l__vid2: u32 = p__vid2;
  let l__iid3: u32 = p__iid3;
  let l_in_: coax_t16 = p_in_;
  return coax_p_fivelab_fx_place(l__vid2, l__iid3, l_in_, coax_p_glade_frame());
}
fn coax_p_bg_fs(p_pos: vec4f) -> coax_t18 {
  let l_pos: vec4f = p_pos;
  var l_p: vec2f;
  var l_eye: vec3f;
  var l_fwd: vec3f;
  var l_rd: vec3f;
  var l_c: vec3f;
  var l_dep: f32;
  var l_k: f32;
  var l_g: vec4f;
  var l_c_11: vec3f;
  var l_c_12: vec3f;
  l_p = coax_p_v2(l_pos.x, l_pos.y);
  if ((coax_at_u(i32(10i)) > 0x1.800000p0f))
  {
    l_eye = coax_p_wg_at3(16i);
    l_fwd = coax_p_wg_at3(20i);
    l_rd = coax_p_normalize3(coax_p_add3(l_fwd, coax_p_add3(coax_p_scale3(coax_p_wg_at3(24i), ((l_p.x - coax_at_u(i32(23i))) / coax_at_u(i32(19i)))), coax_p_scale3(coax_p_wg_at3(28i), ((coax_at_u(i32(27i)) - l_p.y) / coax_at_u(i32(19i)))))));
    l_c = coax_p_cg_sky(coax_p_paint_env_of(), l_rd, coax_p_wg_at3(32i), coax_at_u(i32(2i)));
    l_dep = 0x1.000000p0f;
    l_k = coax_at_u(i32(4i));
    if ((coax_at_u(i32(11i)) < 0x1.000000p-1f))
    {
      l_c = coax_p_scale3(l_c, (0x1.000000p0f - ((0x1.000000p-1f * (0x1.000000p0f - coax_p_smoothstep((0x1.c20000p8f * l_k), (0x1.040000p9f * l_k), l_p.x))) * (0x1.000000p0f - coax_p_smoothstep((0x1.680000p5f * l_k), (0x1.a40000p6f * l_k), l_p.y)))));
    }
    if (((coax_at_u(i32(11i)) < 0x1.000000p-1f) && coax_p_cg_ui_cover(l_p, coax_at_u(i32(0i)), coax_at_u(i32(1i)), l_k)))
    {
      l_c = coax_p_v3(0x1.99999ap-6f, 0x1.26e978p-5f, 0x1.78d4fep-5f);
      l_dep = 0.0f;
    }
    return coax_t18(coax_p_v4(l_c.x, l_c.y, l_c.z, coax_p_saturate(((coax_p_metaverse_style_luma(l_c.x, l_c.y, l_c.z) - 0x1.c28f5cp-1f) * 0x1.000000p3f))), l_dep);
  }
  if ((coax_at_u(i32(10i)) > 0x1.000000p-1f))
  {
    l_g = coax_p_fivelab_glade(l_p);
    l_c_11 = coax_p_fivelab_finish(coax_p_v3(l_g.x, l_g.y, l_g.z), l_p);
    return coax_t18(coax_p_v4(l_c_11.x, l_c_11.y, l_c_11.z, 0x1.000000p0f), l_g.w);
  }
  l_c_12 = coax_p_fivelab_glass(coax_p_fivelab_flat(l_p), l_p);
  return coax_t18(coax_p_v4(l_c_12.x, l_c_12.y, l_c_12.z, 0x1.000000p0f), 0x1.000000p0f);
}
fn coax_p_scene_fs(p_s: coax_t15, p_front: bool) -> vec4f {
  let l_s: coax_t15 = p_s;
  let l_front: bool = p_front;
  var l_tex: vec4f;
  l_tex = coax_p_render_sample_layer8_bias(l_s.w, i32(l_s.f4), 0x1.000000p0f);
  return coax_p_fivelab_scene_colour(l_s, l_tex, l_front);
}
fn coax_p_fx_fs(p_v: coax_t17) -> vec4f {
  let l_v: coax_t17 = p_v;
  return coax_p_fivelab_fx_colour(l_v, coax_p_glade_frame());
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
fn coax_p_glade_frame() -> coax_t22 {
  return coax_t22(coax_p_v4_at(0i), coax_p_v4_at(4i), coax_p_v4_at(8i), coax_p_v4_at(12i), coax_p_v4_at(16i), coax_p_v4_at(20i), coax_p_v4_at(24i), coax_p_v4_at(28i), coax_p_v4_at(32i), coax_p_v4_at(36i), coax_p_v4_at(40i), coax_p_v4_at(44i));
}
fn coax_p_fivelab_scene_place(p_id: u32, p_instance: u32, p_gf: coax_t22) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  let l_gf: coax_t22 = p_gf;
  var l_m0: coax_t11;
  var l_m: coax_t11;
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
  var l_sk: coax_t23;
  var l_sk2: coax_t23;
  var l_c: f32;
  var l_sn: f32;
  var l_w: vec3f;
  var l_o: coax_t15;
  var l_kind: u32;
  if (((l_gf.z.z > 0x1.800000p0f) && (l_instance >= 65536u)))
  {
    l_m0 = objects[i32(((l_instance >> u32(16u)) - 1u))];
    if ((l_m0.w.w > 0x1.380000p4f))
    {
      return coax_p_cg_meadow_vs(l_id, l_instance, l_m0, l_gf);
    }
    return coax_p_cg_instanced_vs(l_id, l_instance, l_m0, l_gf);
  }
  l_m = objects[i32(l_instance)];
  if ((l_m.w.w > 0x1.f00000p3f))
  {
    return coax_p_cg_environment_vs(l_id, l_instance, l_m, l_gf);
  }
  if ((coax_p_wg_fmod(l_m.w.w, 0x1.000000p3f) > 0x1.c00000p1f))
  {
    return coax_p_fivelab_block_vs(l_id, l_instance, l_m, l_gf);
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
  l_sk = coax_t23();
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
  l_o.x = coax_p_fivelab_project(l_w, l_gf);
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
fn coax_p_fivelab_fx_place(p__vid2: u32, p__iid3: u32, p_in_: coax_t16, p_gf: coax_t22) -> coax_t17 {
  let l__vid2: u32 = p__vid2;
  let l__iid3: u32 = p__iid3;
  let l_in_: coax_t16 = p_in_;
  let l_gf: coax_t22 = p_gf;
  var l_a: vec4f;
  var l_o: coax_t17;
  l_a = l_in_.x;
  l_o = coax_t17();
  l_o.x = coax_p_v4((((l_a.x / l_gf.x.x) * 0x1.000000p1f) - 0x1.000000p0f), (0x1.000000p0f - ((l_a.y / l_gf.x.y) * 0x1.000000p1f)), 0.0f, 0x1.000000p0f);
  l_o.y = coax_p_v2(l_a.z, l_a.w);
  l_o.z = l_in_.y;
  l_o.w = l_in_.z;
  l_o.f4 = l_in_.w;
  return l_o;
}
fn coax_p_wg_at3(p_at: i32) -> vec3f {
  let l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
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
fn coax_p_paint_env_of() -> coax_t25 {
  return coax_p_env_block_of(208i);
}
fn coax_p_cg_sky(p_env: coax_t25, p_rd: vec3f, p_sun: vec3f, p_time: f32) -> vec3f {
  let l_env: coax_t25 = p_env;
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  let l_time: f32 = p_time;
  return coax_p_paint_env_sky(l_env, l_rd, l_sun, l_time);
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_cg_ui_cover(p_p: vec2f, p_w: f32, p_h: f32, p_k: f32) -> bool {
  let l_p: vec2f = p_p;
  let l_w: f32 = p_w;
  let l_h: f32 = p_h;
  let l_k: f32 = p_k;
  if (((l_w < (0x1.a40000p9f * l_k)) || (l_h < (0x1.7c0000p9f * l_k))))
  {
    return ((l_p.y > (l_h - (0x1.f80000p8f * l_k))) || ((((l_p.y > (0x1.a00000p4f * l_k)) && (l_p.y < (0x1.f00000p6f * l_k))) && (l_p.x > (0x1.800000p3f * l_k))) && (l_p.x < (l_w - (0x1.800000p3f * l_k)))));
  }
  return ((((l_p.x >= (0x1.000000p4f * l_k)) && (l_p.x <= (0x1.5e0000p8f * l_k))) && (l_p.y >= (0x1.400000p4f * l_k))) && (l_p.y <= (0x1.6d0000p9f * l_k)));
}
fn coax_p_v3(p_x: f32, p_y: f32, p_z: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  return vec3f(l_x, l_y, l_z);
}
fn coax_p_metaverse_style_luma(p_r: f32, p_g: f32, p_b: f32) -> f32 {
  let l_r: f32 = p_r;
  let l_g: f32 = p_g;
  let l_b: f32 = p_b;
  return (((0x1.b367a0p-3f * l_r) + (0x1.6e2eb2p-1f * l_g)) + (0x1.27bb30p-4f * l_b));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
}
fn coax_p_fivelab_glade(p_p: vec2f) -> vec4f {
  let l_p: vec2f = p_p;
  var l_t: f32;
  var l_eye: vec3f;
  var l_f: f32;
  var l_fwd: vec3f;
  var l_right: vec3f;
  var l_up: vec3f;
  var l_sun: vec3f;
  var l_rd: vec3f;
  var l_h: coax_t26;
  var l_sk: vec3f;
  var l_pos: vec3f;
  var l_n: vec3f;
  var l_albedo: vec3f;
  var l_glow: vec3f;
  var l_i: i32;
  var l_za: vec4f;
  var l_zb: vec4f;
  var l_vine: f32;
  var l_mote: f32;
  var l_hw: f32;
  var l_hd: f32;
  var l_sq: f32;
  var l_cx: i32;
  var l_cz: i32;
  var l_cols: i32;
  var l_rows: i32;
  var l_dz: i32;
  var l_dx: i32;
  var l_gx: i32;
  var l_gz: i32;
  var l_sqi: i32;
  var l_who: f32;
  var l_tk: vec4f;
  var l_code: f32;
  var l_state: f32;
  var l_tm: f32;
  var l_d: f32;
  var l_ring: f32;
  var l_tc: vec3f;
  var l_leaf: f32;
  var l_ndl: f32;
  var l_lit: f32;
  var l_sh: coax_t26;
  var l_w: f32;
  var l_band: f32;
  var l_amb: f32;
  var l_shadow_light: vec3f;
  var l_lit_light: vec3f;
  var l_c: vec3f;
  var l_hz: f32;
  var l_hc: vec3f;
  var l_o: vec3f;
  l_t = coax_at_u(i32(2i));
  l_eye = coax_p_wg_at3(16i);
  l_f = coax_p_fmax(coax_at_u(i32(19i)), 0x1.000000p0f);
  l_fwd = coax_p_wg_at3(20i);
  l_right = coax_p_wg_at3(24i);
  l_up = coax_p_wg_at3(28i);
  l_sun = coax_p_normalize3(coax_p_wg_at3(32i));
  l_rd = coax_p_normalize3(coax_p_add3(l_fwd, coax_p_add3(coax_p_scale3(l_right, ((l_p.x - coax_at_u(i32(23i))) / l_f)), coax_p_scale3(l_up, ((coax_at_u(i32(27i)) - l_p.y) / l_f)))));
  l_h = coax_p_fivelab_trace(l_eye, l_rd, false);
  if ((l_h.z < 0.0f))
  {
    l_sk = coax_p_fivelab_sky(l_rd, l_sun, l_t);
    return coax_p_v4(l_sk.x, l_sk.y, l_sk.z, 0x1.000000p0f);
  }
  l_pos = coax_p_add3(l_eye, coax_p_scale3(l_rd, l_h.x));
  l_n = l_h.y;
  l_albedo = coax_p_v3(0x1.000000p-1f, 0x1.000000p-1f, 0x1.000000p-1f);
  l_glow = coax_p_v3(0.0f, 0.0f, 0.0f);
  if ((l_h.z < 0x1.000000p-1f))
  {
    l_albedo = coax_p_fivelab_grass(l_pos.x, l_pos.z, l_t, (l_h.x / (l_f * coax_p_fmax(abs(l_rd.y), 0x1.47ae14p-4f))));
    {
      l_i = 0i;
      while ((l_i < 8i))
      {
        l_za = coax_p_wg_at4((36i + (l_i * 8i)));
        l_zb = coax_p_wg_at4((40i + (l_i * 8i)));
        if ((l_zb.x > 0x1.000000p-1f))
        {
          if (((((l_pos.x >= l_za.x) && (l_pos.x <= l_za.z)) && (l_pos.z >= l_za.y)) && (l_pos.z <= l_za.w)))
          {
            if ((l_zb.x < 0x1.800000p0f))
            {
              l_vine = coax_p_metaverse_style_noise((l_pos.x * 0x1.800000p1f), (l_pos.z * 0x1.800000p1f));
              l_albedo = coax_p_mix3(l_albedo, coax_p_v3(0x1.99999ap-5f, 0x1.99999ap-3f, 0x1.47ae14p-5f), (0x1.19999ap-1f + (0x1.666666p-2f * l_vine)));
              l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.99999ap-4f, 0x1.333334p-2f, 0x1.99999ap-5f), (0x1.333334p-3f * coax_p_smoothstep(0x1.19999ap-1f, 0x1.99999ap-1f, l_vine))));
            }
            else
            {
              l_mote = coax_p_metaverse_style_noise(((l_pos.x * 0x1.000000p1f) + (l_t * 0x1.666666p-1f)), ((l_pos.z * 0x1.000000p1f) - (l_t * 0x1.000000p-1f)));
              l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.333334p-2f, 0x1.000000p0f), (0x1.70a3d8p-3f + (0x1.666666p-2f * coax_p_smoothstep(0x1.333334p-1f, 0x1.ccccccp-1f, l_mote)))));
            }
          }
        }
        l_i = (l_i + 1i);
      }
    }
    {
      l_hw = coax_at_u(i32(100i));
      l_hd = coax_at_u(i32(101i));
      l_sq = coax_p_fmax(coax_at_u(i32(102i)), 0x1.99999ap-4f);
      l_cx = i32(floor(((l_pos.x + l_hw) / l_sq)));
      l_cz = i32(floor(((l_pos.z + l_hd) / l_sq)));
      l_cols = i32(coax_at_u(i32(8i)));
      l_rows = i32(coax_at_u(i32(9i)));
      l_dz = (-1i);
      while ((l_dz <= 1i))
      {
        l_dx = (-1i);
        while ((l_dx <= 1i))
        {
          l_gx = (l_cx + l_dx);
          l_gz = (l_cz + l_dz);
          if (((((l_gx >= 0i) && (l_gx < l_cols)) && (l_gz >= 0i)) && (l_gz < l_rows)))
          {
            l_sqi = ((l_gz * l_cols) + l_gx);
            l_who = frame[(69632i + l_sqi)];
            if ((l_who > 0x1.000000p-1f))
            {
              l_tk = coax_p_wg_at4_1(((i32(l_who) - 1i) * 4i));
              if ((l_tk.z > 0x1.000000p-1f))
              {
                l_code = (l_tk.z - 0x1.000000p0f);
                l_state = floor((l_code / 0x1.000000p2f));
                l_tm = (l_code - (0x1.000000p2f * l_state));
                l_d = coax_p_wg_length2(coax_p_v2((l_pos.x - l_tk.x), (l_pos.z - l_tk.y)));
                l_albedo = coax_p_scale3(l_albedo, (0x1.19999ap-1f + (0x1.ccccccp-2f * coax_p_smoothstep(0x1.333334p-3f, 0x1.666666p-1f, l_d))));
                l_ring = (0x1.000000p0f - coax_p_smoothstep(0x1.eb851ep-6f, 0x1.47ae14p-4f, abs((l_d - 0x1.5c28f6p-1f))));
                l_tc = coax_p_v3(0x1.666666p-2f, 0x1.e66666p-1f, 0x1.ccccccp-2f);
                if ((l_tm > 0x1.000000p-1f))
                {
                  l_tc = coax_p_v3(0x1.19999ap-1f, 0x1.b33334p-1f, 0x1.000000p0f);
                }
                if ((l_tm > 0x1.800000p0f))
                {
                  l_tc = coax_p_v3(0x1.ccccccp-1f, 0x1.b851ecp-1f, 0x1.7ae148p-1f);
                }
                if (((l_state < 0x1.000000p-1f) && (coax_at_u(i32(35i)) < 0x1.000000p-1f)))
                {
                  l_glow = coax_p_add3(l_glow, coax_p_scale3(l_tc, (l_ring * (0x1.000000p-2f + (l_tk.w * 0x1.99999ap0f)))));
                }
              }
            }
          }
          l_dx = (l_dx + 1i);
        }
        l_dz = (l_dz + 1i);
      }
    }
  }
  else
  {
    if ((l_h.z < 0x1.800000p0f))
    {
      l_albedo = coax_p_v3(0x1.eb851ep-3f, 0x1.333334p-3f, 0x1.47ae14p-4f);
      l_albedo = coax_p_scale3(l_albedo, (0x1.99999ap-1f + (0x1.99999ap-2f * coax_p_metaverse_style_noise((l_pos.y * 0x1.000000p2f), ((l_pos.x + l_pos.z) * 0x1.800000p2f)))));
    }
    else
    {
      if ((l_h.z < 0x1.400000p1f))
      {
        l_leaf = coax_p_metaverse_style_noise(((l_pos.x * 0x1.4cccccp0f) + l_pos.y), ((l_pos.z * 0x1.4cccccp0f) - l_pos.y));
        l_albedo = coax_p_mix3(coax_p_v3(0x1.70a3d8p-4f, 0x1.eb851ep-3f, 0x1.eb851ep-5f), coax_p_v3(0x1.333334p-2f, 0x1.ccccccp-2f, 0x1.99999ap-4f), l_leaf);
        l_n = coax_p_normalize3(coax_p_add3(l_n, coax_p_scale3(coax_p_v3((l_leaf - 0x1.000000p-1f), (coax_p_metaverse_style_noise((l_pos.x * 0x1.0cccccp1f), (l_pos.z * 0x1.0cccccp1f)) - 0x1.000000p-1f), (0x1.000000p-1f - l_leaf)), 0x1.ccccccp-1f)));
      }
    }
  }
  l_ndl = dot(l_n, l_sun);
  l_lit = 0x1.000000p0f;
  if (((l_ndl > 0.0f) && (coax_at_u(i32(10i)) < 0x1.800000p0f)))
  {
    l_sh = coax_p_fivelab_trace(coax_p_add3(l_pos, coax_p_scale3(l_n, 0x1.47ae14p-6f)), l_sun, true);
    if ((l_sh.z > 0.0f))
    {
      l_lit = 0.0f;
    }
  }
  l_w = (coax_p_metaverse_style_wrap(l_ndl) * l_lit);
  l_band = coax_p_metaverse_style_bands(l_w, 0x1.99999ap-3f, 0x1.28f5c2p-1f, 0x1.eb851ep-5f, 0x1.19999ap-1f);
  l_amb = coax_p_metaverse_style_ambient(l_n.y, 0x1.000000p0f, 0x1.19999ap-1f, 0x1.000000p0f);
  l_shadow_light = coax_p_scale3(coax_p_v3(0x1.70a3d8p-2f, 0x1.c28f5cp-2f, 0x1.3d70a4p-1f), (0x1.3d70a4p-1f * l_amb));
  l_lit_light = coax_p_add3(coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.7ae148p-1f), 0x1.400000p0f), coax_p_scale3(l_shadow_light, 0x1.333334p-1f));
  l_c = coax_p_v3(coax_p_metaverse_style_shade(l_albedo.x, l_shadow_light.x, l_lit_light.x, l_band), coax_p_metaverse_style_shade(l_albedo.y, l_shadow_light.y, l_lit_light.y, l_band), coax_p_metaverse_style_shade(l_albedo.z, l_shadow_light.z, l_lit_light.z, l_band));
  l_c = coax_p_add3(l_c, l_glow);
  l_hz = (0x1.000000p0f - exp((((-l_h.x) * 0x1.6872b0p-7f) / coax_p_fmax(coax_at_u(i32(103i)), 0x1.000000p0f))));
  l_hc = coax_p_mix3(coax_p_v3(0x1.70a3d8p-1f, 0x1.99999ap-1f, 0x1.ae147ap-1f), coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.800000p-1f), (coax_p_metaverse_style_halo(dot(l_rd, l_sun), 0x1.400000p2f) * 0x1.000000p-1f));
  l_o = coax_p_mix3(l_c, l_hc, (l_hz * 0x1.b33334p-1f));
  return coax_p_v4(l_o.x, l_o.y, l_o.z, coax_p_fivelab_depth((l_h.x * dot(l_rd, l_fwd))));
}
fn coax_p_fivelab_finish(p_c0: vec3f, p_p: vec2f) -> vec3f {
  let l_c0: vec3f = p_c0;
  let l_p: vec2f = p_p;
  var l_c: vec3f;
  var l_l: f32;
  l_c = coax_p_v3(coax_p_metaverse_style_tone(l_c0.x, 0x1.266666p0f), coax_p_metaverse_style_tone(l_c0.y, 0x1.266666p0f), coax_p_metaverse_style_tone(l_c0.z, 0x1.266666p0f));
  l_l = coax_p_metaverse_style_luma(l_c.x, l_c.y, l_c.z);
  l_c = coax_p_v3(coax_p_metaverse_style_saturation(l_c.x, l_l, 0x1.1eb852p0f), coax_p_metaverse_style_saturation(l_c.y, l_l, 0x1.1eb852p0f), coax_p_metaverse_style_saturation(l_c.z, l_l, 0x1.1eb852p0f));
  if ((l_p.y < coax_at_u(i32(11i))))
  {
    l_c = coax_p_add3(coax_p_scale3(l_c, 0x1.c28f5cp-3f), coax_p_v3(0x1.47ae14p-7f, 0x1.89374cp-7f, 0x1.0624dep-6f));
  }
  return coax_p_fivelab_glass(l_c, l_p);
}
fn coax_p_fivelab_flat(p_p: vec2f) -> vec3f {
  let l_p: vec2f = p_p;
  var l_q: vec2f;
  var l_r: f32;
  var l_c: vec3f;
  var l_gx1: f32;
  var l_gy1: f32;
  var l_inside: f32;
  l_q = coax_p_v2(((l_p.x / coax_at_u(i32(0i))) - 0x1.000000p-1f), ((l_p.y / coax_at_u(i32(1i))) - 0x1.000000p-1f));
  l_r = coax_p_wg_length2(l_q);
  l_c = coax_p_scale3(coax_p_v3(0x1.eb851ep-6f, 0x1.16872cp-5f, 0x1.47ae14p-5f), (0x1.000000p0f - coax_p_fmin(l_r, 0x1.000000p0f)));
  l_gx1 = (coax_at_u(i32(4i)) + (coax_at_u(i32(8i)) * coax_at_u(i32(6i))));
  l_gy1 = (coax_at_u(i32(5i)) + (coax_at_u(i32(9i)) * coax_at_u(i32(6i))));
  l_inside = 0x1.70a3d8p-3f;
  if (((((l_p.x >= coax_at_u(i32(4i))) && (l_p.x <= l_gx1)) && (l_p.y >= coax_at_u(i32(5i)))) && (l_p.y <= l_gy1)))
  {
    l_inside = 0x1.000000p0f;
    l_c = coax_p_add3(l_c, coax_p_v3(0x1.89374cp-7f, 0x1.0624dep-6f, 0x1.cac084p-7f));
  }
  return coax_p_fivelab_flat_tokens(l_p, coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.99999ap-3f, 0x1.d70a3ep-3f, 0x1.c28f5cp-3f), (coax_p_fivelab_lines(l_p) * l_inside))));
}
fn coax_p_fivelab_glass(p_c: vec3f, p_p: vec2f) -> vec3f {
  let l_c: vec3f = p_c;
  let l_p: vec2f = p_p;
  if ((coax_at_u(i32(14i)) > coax_at_u(i32(12i))))
  {
    if (((((l_p.x >= coax_at_u(i32(12i))) && (l_p.x <= coax_at_u(i32(14i)))) && (l_p.y >= coax_at_u(i32(13i)))) && (l_p.y <= coax_at_u(i32(15i)))))
    {
      return coax_p_add3(coax_p_scale3(l_c, 0x1.1eb852p-3f), coax_p_v3(0x1.89374cp-7f, 0x1.cac084p-7f, 0x1.47ae14p-6f));
    }
  }
  return l_c;
}
fn coax_p_render_sample_layer8_bias(p_uv: vec2f, p_layer: i32, p_bias: f32) -> vec4f {
  let l_uv: vec2f = p_uv;
  let l_layer: i32 = p_layer;
  let l_bias: f32 = p_bias;
  var l_rs_dx: vec2f;
  var l_rs_dy: vec2f;
  l_rs_dx = coax_p_v2(dpdxFine(l_uv.x), dpdxFine(l_uv.y));
  l_rs_dy = coax_p_v2(dpdyFine(l_uv.x), dpdyFine(l_uv.y));
  return coax_p_render_sample_layer8_grad(l_uv, l_layer, l_rs_dx, l_rs_dy, l_bias);
}
fn coax_p_fivelab_scene_colour(p_s: coax_t15, p_tex: vec4f, p_front: bool) -> vec4f {
  let l_s: coax_t15 = p_s;
  let l_tex: vec4f = p_tex;
  let l_front: bool = p_front;
  var l_m: coax_t11;
  var l_t: f32;
  var l_albedo: vec3f;
  var l_glow: vec3f;
  var l_iris: f32;
  var l_shade: f32;
  var l_l: f32;
  var l_p: vec3f;
  var l_n: vec3f;
  var l_eye: vec3f;
  var l_rd: vec3f;
  var l_marks: f32;
  var l_vine: f32;
  var l_rim: f32;
  var l_sun: vec3f;
  var l_ndl: f32;
  var l_lit: f32;
  var l_sh: coax_t26;
  var l_w: f32;
  var l_band: f32;
  var l_amb: f32;
  var l_shadow_light: vec3f;
  var l_lit_light: vec3f;
  var l_c: vec3f;
  var l_d: f32;
  var l_hz: f32;
  var l_hc: vec3f;
  var l_bayer: array<f32, 16>;
  var l_px: i32;
  var l_py: i32;
  var l_o: vec3f;
  l_m = objects[i32(l_s.f8)];
  if ((l_tex.w < 0x1.666666p-2f))
  {
    discard;
  }
  l_t = coax_at_u(i32(2i));
  l_albedo = coax_p_mul3(coax_p_fivelab_dye(coax_p_v3(l_tex.x, l_tex.y, l_tex.z), l_s.f6), l_s.f5);
  l_glow = coax_p_v3(0.0f, 0.0f, 0.0f);
  if (((l_m.f5.w > 0x1.800000p0f) && coax_p_e2_eye_layer(l_s.f4)))
  {
    l_iris = coax_p_saturate(((l_tex.x - coax_p_fmax(l_tex.y, l_tex.z)) * 0x1.400000p2f));
    l_shade = coax_p_clampf((coax_p_metaverse_style_luma(l_tex.x, l_tex.y, l_tex.z) * 0x1.800000p1f), 0x1.333334p-3f, 0x1.4cccccp0f);
    l_albedo = coax_p_mix3(l_albedo, coax_p_scale3(coax_p_v3(l_m.f5.x, l_m.f5.y, l_m.f5.z), l_shade), l_iris);
  }
  if ((l_s.f6.w > 0x1.300000p3f))
  {
    l_albedo = l_s.f5;
  }
  if (((l_s.f6.w > 0x1.e00000p2f) && (l_s.f6.w < 0x1.300000p3f)))
  {
    l_l = coax_p_metaverse_style_luma(l_tex.x, l_tex.y, l_tex.z);
    l_albedo = coax_p_scale3(coax_p_v3(0x1.99999ap-1f, 0x1.800000p-1f, 0x1.3d70a4p-1f), (0x1.666666p-1f + (0x1.333334p-1f * coax_p_saturate((l_l * 0x1.19999ap1f)))));
    if ((l_s.f6.w > 0x1.100000p3f))
    {
      l_albedo = coax_p_scale3(coax_p_v3(0x1.70a3d8p-4f, 0x1.47ae14p-4f, 0x1.1eb852p-4f), (0x1.666666p-1f + l_l));
    }
    l_p = l_s.f7;
    if ((((l_p.y > 0x1.000000p0f) && (l_p.y < 0x1.6147aep0f)) && (abs(l_p.x) < 0x1.99999ap-3f)))
    {
      if (((coax_p_fract(((l_p.y - 0x1.000000p0f) * 0x1.e00000p3f)) < 0x1.ccccccp-2f) && (abs(l_p.x) > 0x1.cac084p-6f)))
      {
        discard;
      }
      l_albedo = coax_p_scale3(coax_p_v3(0x1.99999ap-1f, 0x1.800000p-1f, 0x1.3d70a4p-1f), 0x1.b33334p-1f);
    }
    if ((((l_p.y > 0x1.b851ecp-1f) && (l_p.y < 0x1.f5c290p-1f)) && (abs(l_p.x) < 0x1.99999ap-3f)))
    {
      l_albedo = coax_p_scale3(l_albedo, 0x1.ccccccp-2f);
    }
    if (coax_p_e2_eye_layer(l_s.f4))
    {
      l_albedo = coax_p_v3(0x1.47ae14p-6f, 0x1.47ae14p-7f, 0x1.47ae14p-7f);
      l_glow = coax_p_v3(0x1.333334p0f, 0x1.333334p-2f, 0x1.47ae14p-4f);
    }
  }
  l_n = coax_p_normalize3(coax_p_add3(l_s.z, coax_p_v3(0.0f, 0x1.4f8b58p-17f, 0.0f)));
  if ((!l_front))
  {
    l_n = coax_p_scale3(l_n, (-0x1.000000p0f));
  }
  l_eye = coax_p_wg_at3(16i);
  l_rd = coax_p_normalize3(coax_p_sub3(l_s.y, l_eye));
  l_marks = select(l_m.f5.x, 0.0f, (l_m.f5.w > 0x1.800000p0f));
  if ((coax_p_fivelab_mark(l_marks, 0.0f) && ((l_s.y.y - l_m.x.y) < 0x1.ccccccp-1f)))
  {
    l_vine = coax_p_fract((((l_s.y.y - l_m.x.y) * 0x1.c00000p2f) + (coax_p_wg_atan2((l_s.y.z - l_m.x.z), (l_s.y.x - l_m.x.x)) * 0x1.333334p0f)));
    if ((l_vine < 0x1.333334p-2f))
    {
      l_albedo = coax_p_v3(0x1.47ae14p-4f, 0x1.47ae14p-2f, 0x1.99999ap-5f);
    }
  }
  if (coax_p_fivelab_mark(l_marks, 0x1.000000p0f))
  {
    l_albedo = coax_p_mix3(l_albedo, coax_p_v3(0x1.99999ap-2f, 0x1.19999ap-1f, 0x1.47ae14p-4f), 0x1.ccccccp-2f);
  }
  if ((coax_p_fivelab_mark(l_marks, 0x1.000000p1f) && (abs((l_s.f7.y - 0x1.ad4fe0p0f)) < 0x1.99999ap-5f)))
  {
    l_albedo = coax_p_v3(0x1.eb851ep-6f, 0x1.99999ap-6f, 0x1.eb851ep-6f);
  }
  if (coax_p_fivelab_mark(l_marks, 0x1.800000p1f))
  {
    l_albedo = coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.000000p0f, 0x1.000000p0f), (coax_p_metaverse_style_luma(l_albedo.x, l_albedo.y, l_albedo.z) * 0x1.99999ap-1f));
  }
  l_rim = (0x1.000000p0f - coax_p_saturate(dot(l_n, coax_p_scale3(l_rd, (-0x1.000000p0f)))));
  if (coax_p_fivelab_mark(l_marks, 0x1.000000p2f))
  {
    l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.ccccccp-1f, 0x1.333334p-3f, 0x1.000000p-2f), (coax_pow_bg_vs(l_rim, 0x1.800000p1f) * 0x1.ccccccp-1f)));
  }
  if (coax_p_fivelab_mark(l_marks, 0x1.400000p2f))
  {
    l_albedo = coax_p_mix3(l_albedo, coax_p_v3(0x1.b33334p-1f, 0x1.666666p-1f, 0x1.333334p-2f), 0x1.666666p-2f);
    l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.99999ap-1f, 0x1.666666p-2f), ((l_rim * l_rim) * 0x1.99999ap-1f)));
  }
  if (coax_p_fivelab_mark(l_marks, 0x1.800000p2f))
  {
    l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.333334p-3f, 0x1.99999ap-1f), (coax_pow_bg_vs(l_rim, 0x1.000000p2f) * (0x1.666666p-1f + (0x1.333334p-2f * sin(((l_t * 0x1.200000p3f) + (l_s.y.y * 0x1.800000p2f))))))));
  }
  if (((l_m.f5.w < 0x1.800000p0f) && (l_m.f5.y > 0x1.800000p0f)))
  {
    l_albedo = coax_p_scale3(coax_p_v3(0x1.99999ap-2f, 0x1.99999ap-2f, 0x1.ae147ap-2f), (coax_p_metaverse_style_luma(l_albedo.x, l_albedo.y, l_albedo.z) * 0x1.19999ap1f));
  }
  if (((l_m.f5.w < 0x1.800000p0f) && (l_m.f5.z > 0x1.000000p-1f)))
  {
    l_glow = coax_p_add3(l_glow, coax_p_scale3(coax_p_v3(0x1.666666p-1f, 0x1.99999ap-2f, 0x1.000000p0f), ((0x1.666666p-2f + (0x1.000000p-2f * sin((l_t * 0x1.400000p2f)))) + ((l_rim * l_rim) * 0x1.ccccccp-1f))));
  }
  l_sun = coax_p_normalize3(coax_p_wg_at3(32i));
  l_ndl = dot(l_n, l_sun);
  l_lit = 0x1.000000p0f;
  if (((l_ndl > 0.0f) && (coax_at_u(i32(10i)) < 0x1.800000p0f)))
  {
    l_sh = coax_p_fivelab_trace(coax_p_add3(l_s.y, coax_p_scale3(l_n, 0x1.eb851ep-6f)), l_sun, true);
    if ((l_sh.z > 0.0f))
    {
      l_lit = 0.0f;
    }
  }
  l_w = (coax_p_metaverse_style_wrap(l_ndl) * l_lit);
  l_band = coax_p_metaverse_style_bands2(l_w, 0x1.99999ap-2f, 0x1.47ae14p-4f);
  l_amb = coax_p_metaverse_style_ambient(l_n.y, 0x1.000000p0f, 0x1.19999ap-1f, 0x1.000000p0f);
  l_shadow_light = coax_p_scale3(coax_p_v3(0x1.70a3d8p-2f, 0x1.c28f5cp-2f, 0x1.3d70a4p-1f), (0x1.3d70a4p-1f * l_amb));
  l_lit_light = coax_p_add3(coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.7ae148p-1f), 0x1.400000p0f), coax_p_scale3(l_shadow_light, 0x1.333334p-1f));
  l_c = coax_p_v3(coax_p_metaverse_style_shade(l_albedo.x, l_shadow_light.x, l_lit_light.x, l_band), coax_p_metaverse_style_shade(l_albedo.y, l_shadow_light.y, l_lit_light.y, l_band), coax_p_metaverse_style_shade(l_albedo.z, l_shadow_light.z, l_lit_light.z, l_band));
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.19999ap-1f, 0x1.3d70a4p-1f, 0x1.800000p-1f), ((coax_pow_bg_vs(l_rim, 0x1.800000p1f) * 0x1.000000p-2f) * coax_p_saturate((l_n.y + 0x1.333334p-1f)))));
  l_c = coax_p_add3(l_c, l_glow);
  if ((coax_at_u(i32(10i)) > 0x1.800000p0f))
  {
    l_c = coax_p_add3(coax_p_scale3(coax_p_v3(coax_pow_bg_vs(l_albedo.x, 0x1.ccccccp-2f), coax_pow_bg_vs(l_albedo.y, 0x1.ccccccp-2f), coax_pow_bg_vs(l_albedo.z, 0x1.ccccccp-2f)), (0x1.b851ecp-1f + (0x1.1eb852p-2f * coax_p_saturate(l_ndl)))), coax_p_scale3(coax_p_v3(0x1.4cccccp-1f, 0x1.800000p-1f, 0x1.0a3d70p-1f), (coax_pow_bg_vs(l_rim, 0x1.800000p1f) * 0x1.47ae14p-4f)));
  }
  l_d = coax_p_wg_length3(coax_p_sub3(l_s.y, l_eye));
  l_hz = (0x1.000000p0f - exp(((-l_d) * 0x1.6872b0p-7f)));
  l_hc = coax_p_mix3(coax_p_v3(0x1.70a3d8p-1f, 0x1.99999ap-1f, 0x1.ae147ap-1f), coax_p_v3(0x1.000000p0f, 0x1.ccccccp-1f, 0x1.800000p-1f), (coax_p_metaverse_style_halo(dot(l_rd, l_sun), 0x1.400000p2f) * 0x1.000000p-1f));
  l_c = coax_p_mix3(l_c, l_hc, (l_hz * 0x1.b33334p-1f));
  if ((l_m.f5.w < 0x1.ff7ceep-1f))
  {
    l_bayer = array<f32, 16>(0.0f, 0x1.000000p3f, 0x1.000000p1f, 0x1.400000p3f, 0x1.800000p3f, 0x1.000000p2f, 0x1.c00000p3f, 0x1.800000p2f, 0x1.800000p1f, 0x1.600000p3f, 0x1.000000p0f, 0x1.200000p3f, 0x1.e00000p3f, 0x1.c00000p2f, 0x1.a00000p3f, 0x1.400000p2f);
    l_px = (i32(l_s.x.x) % 4i);
    l_py = (i32(l_s.x.y) % 4i);
    if ((((l_bayer[((l_py * 4i) + l_px)] + 0x1.000000p-1f) / 0x1.000000p4f) >= l_m.f5.w))
    {
      discard;
    }
  }
  l_o = coax_p_fivelab_finish(l_c, coax_p_v2(l_s.x.x, l_s.x.y));
  return coax_p_v4(l_o.x, l_o.y, l_o.z, 0x1.000000p0f);
}
fn coax_p_fivelab_fx_colour(p_v: coax_t17, p_gf: coax_t22) -> vec4f {
  let l_v: coax_t17 = p_v;
  let l_gf: coax_t22 = p_gf;
  var l_lod: f32;
  var l_alpha: f32;
  var l_along: f32;
  var l_d: f32;
  var l_core: f32;
  var l_g: f32;
  var l_halo: f32;
  if ((l_v.f4.w > 0x1.000000p-1f))
  {
    return coax_p_fivelab_hud_fs(l_v, l_gf);
  }
  if ((l_v.w.w > 0x1.c00000p1f))
  {
    l_lod = coax_p_clampf(coax_p_wg_log2((0x1.000000p6f / coax_p_fmax(l_v.z.x, 0x1.000000p0f))), 0.0f, 0x1.000000p2f);
    l_alpha = coax_pow_bg_vs((textureSampleLevel(sans_atlas, sans_sampler, l_v.y, l_lod)).w, 0x1.b33334p-1f);
    return coax_p_wg_make_f4_f3f(coax_p_scale3(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), ((l_alpha * coax_p_fmin((l_v.z.z * 0x1.266666p0f), 0x1.000000p0f)) * l_gf.x.w)), 0x1.000000p0f);
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_d = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_d));
  l_g = (l_d / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_g) * l_g)) * 0x1.1eb852p-2f);
  return coax_p_wg_make_f4_f3f(coax_p_scale3(coax_p_scale3(coax_p_scale3(coax_p_v3(l_v.w.x, l_v.w.y, l_v.w.z), (l_core + l_halo)), l_v.z.z), l_gf.x.w), 0x1.000000p0f);
}
fn coax_p_v4_at(p_at: i32) -> vec4f {
  let l_at: i32 = p_at;
  return vec4f(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_cg_meadow_vs(p_id: u32, p_instance: u32, p_m: coax_t11, p_gf: coax_t22) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  let l_m: coax_t11 = p_m;
  let l_gf: coax_t22 = p_gf;
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
  var l_c_31: f32;
  var l_gx_32: u32;
  var l_gz_33: u32;
  var l_x_34: f32;
  var l_z_35: f32;
  var l_dx_36: f32;
  var l_dz_37: f32;
  var l_k: f32;
  var l_h: f32;
  var l_au_40: f32;
  var l_p: coax_t29;
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
  l_ex = l_gf.f4.x;
  l_ez = l_gf.f4.z;
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
    return coax_p_cg_meadow_place(l_id, coax_p_v4(l_x, coax_p_exp_height_gpu(l_x, l_z), l_z, (coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 3u)) * 0x1.921fb6p2f)), (coax_p_e2_meadow_tuft_sv(l_fx, l_fz) * l_shown), (coax_p_e2_meadow_tuft_sh(l_fx, l_fz) * l_shown), coax_p_v3((0x1.333334p-2f + (0x1.0a3d70p-2f * l_au)), 0x1.eb851ep-2f, 0x1.c28f5cp-5f), l_au, l_m, l_gf, l_o.f8);
  }
  l_c_31 = coax_p_e2_meadow_cell(l_kit);
  l_gx_32 = ((u32(floor(((l_ex - l_origin) / l_c_31))) + l_ix) - l_half);
  l_gz_33 = ((u32(floor(((l_ez - l_origin) / l_c_31))) + l_iz) - l_half);
  if ((!coax_p_e2_meadow_held(l_gx_32, l_gz_33, l_kit)))
  {
    return l_o;
  }
  l_x_34 = coax_p_e2_meadow_at_x(l_gx_32, l_gz_33, l_kit);
  l_z_35 = coax_p_e2_meadow_at_z(l_gx_32, l_gz_33, l_kit);
  l_dx_36 = (l_x_34 - l_ex);
  l_dz_37 = (l_z_35 - l_ez);
  if ((l_m.z.z > l_m.z.x))
  {
    if (((((l_x_34 > l_m.z.x) && (l_x_34 < l_m.z.z)) && (l_z_35 > l_m.z.y)) && (l_z_35 < l_m.z.w)))
    {
      return l_o;
    }
    if (((l_m.x.w > 0.0f) && (((((l_x_34 < (l_m.z.x - l_m.x.w)) || (l_x_34 > (l_m.z.z + l_m.x.w))) || (l_z_35 < (l_m.z.y - l_m.x.w))) || (l_z_35 > (l_m.z.w + l_m.x.w))) || (coax_p_cg_hash(coax_p_e2_meadow_seed(l_gx_32, l_gz_33, l_kit), 0x1.200000p3f) > l_m.y.x))))
    {
      return l_o;
    }
  }
  l_k = coax_p_e2_meadow_fade(l_kit, (sqrt(((l_dx_36 * l_dx_36) + (l_dz_37 * l_dz_37))) / l_reach));
  if ((l_k <= 0.0f))
  {
    return l_o;
  }
  l_h = coax_p_e2_meadow_seed(l_gx_32, l_gz_33, l_kit);
  l_au_40 = coax_p_e2_meadow_gold(l_x_34, l_z_35);
  l_p = coax_p_e2_meadow_piece(l_kit, l_h, l_au_40);
  return coax_p_cg_meadow_place(l_id, coax_p_v4(l_x_34, (coax_p_exp_height_gpu(l_x_34, l_z_35) - l_p.w), l_z_35, (coax_p_cg_hash(l_h, 0x1.000000p4f) * 0x1.921cacp2f)), (l_p.y * l_k), (l_p.z * l_k), coax_p_v3(l_p.f4[0i], l_p.f4[1i], l_p.f4[2i]), l_au_40, l_m, l_gf, l_o.f8);
}
fn coax_p_cg_instanced_vs(p_id: u32, p_instance: u32, p_m: coax_t11, p_gf: coax_t22) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  let l_m: coax_t11 = p_m;
  let l_gf: coax_t22 = p_gf;
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
  l_p.x = (l_p.x + ((((sin((((l_center.x * 0x1.ccccccp0f) + (l_center.z * 0x1.666666p-1f)) + (l_gf.x.z * 0x1.99999ap0f))) + (sin(((l_center.z * 0x1.59999ap1f) + (l_gf.x.z * 0x1.ccccccp-1f))) * 0x1.666666p-2f)) * 0x1.70a3d8p-5f) * l_height) * l_height));
  l_packed = vertices[i32((l_b + 3u))];
  l_nw = coax_p_wg_unpack4x8snorm(l_packed);
  l_n = coax_p_normalize3(coax_p_v3((((l_nw.x * cos(l_angle)) + (l_nw.z * sin(l_angle))) / l_sizes.y), (l_nw.y / l_sizes.x), ((((-l_nw.x) * sin(l_angle)) + (l_nw.z * cos(l_angle))) / l_sizes.y)));
  l_base = coax_p_wg_unpack4x8unorm(vertices[i32((l_seed + 5u))]);
  l_autumn = coax_p_cg_autumn(l_center.x, l_center.z);
  l_o = coax_t15();
  l_o.x = coax_p_fivelab_project(l_p, l_gf);
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
fn coax_p_cg_environment_vs(p_id: u32, p_instance: u32, p_m: coax_t11, p_gf: coax_t22) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  let l_m: coax_t11 = p_m;
  let l_gf: coax_t22 = p_gf;
  var l_b: u32;
  var l_p: vec3f;
  var l_sky_clip: vec4f;
  var l_seed_uv: vec2f;
  var l_uv: vec2f;
  var l_normal: u32;
  var l_sid: u32;
  var l_r: coax_t30;
  var l_marker: f32;
  var l_sky: bool;
  var l_tint: vec4f;
  var l_wind: f32;
  var l_grass: bool;
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
  l_sid = ((l_normal >> u32(24u)) & 15u);
  l_r = coax_p_e2_surface_id(i32(l_sid));
  l_marker = bitcast<f32>(vertices[i32((l_b + 7u))]);
  l_sky = coax_p_e2_surf_is(l_r, 2048i);
  if (l_sky)
  {
    l_p = coax_p_add3(coax_p_v3_of(l_gf.f4), coax_p_scale3(coax_p_cg_sky_ray(l_p.x, l_p.y), 0x1.680000p7f));
  }
  if ((coax_p_e2_surf_is(l_r, 512i) || coax_p_e2_village(l_r, l_marker)))
  {
    l_uv = coax_p_e2_tiled_uv(l_seed_uv, l_r);
  }
  l_tint = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 5u))]);
  l_wind = (sin((((l_p.x * 0x1.ccccccp0f) + (l_p.z * 0x1.666666p-1f)) + (l_gf.x.z * 0x1.99999ap0f))) + (sin(((l_p.z * 0x1.59999ap1f) + (l_gf.x.z * 0x1.ccccccp-1f))) * 0x1.666666p-2f));
  if (coax_p_e2_surf_is(l_r, 1i))
  {
    l_grass = coax_p_e2_surf_is(l_r, 4i);
    l_p.x = (l_p.x + ((l_wind * select(0x1.1eb852p-5f, 0x1.99999ap-6f, l_grass)) * select(coax_p_saturate((l_p.y * 0x1.666666p-1f)), (coax_p_wg_unpack2x16unorm(vertices[i32((l_b + 4u))])).y, l_grass)));
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
  l_o.x = select(coax_p_fivelab_project(l_p, l_gf), l_sky_clip, l_sky);
  l_o.y = l_p;
  l_o.z = l_normal3;
  l_o.w = l_uv;
  l_o.f4 = vertices[i32((l_b + 6u))];
  l_o.f5 = coax_p_v3(l_tint.x, l_tint.y, l_tint.z);
  l_o.f6 = coax_p_v4(select(0.0f, 0x1.000000p0f, ((l_m.w.w < 0x1.080000p4f) && (l_m.z.x > 0x1.000000p-1f))), 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(f32(l_sid), l_marker, 0.0f);
  l_o.f8 = l_instance;
  if (((l_m.w.w > 0x1.280000p4f) && (l_m.x.w > 0.0f)))
  {
    l_o.f7.z = (l_tint.w - (0x1.000000p0f - l_m.x.w));
    l_o.f6.w = 0x1.a00000p3f;
  }
  return l_o;
}
fn coax_p_wg_fmod(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return (l_a - (l_b * coax_p_wg_trunc((l_a / l_b))));
}
fn coax_p_fivelab_block_vs(p_id: u32, p_instance: u32, p_m: coax_t11, p_gf: coax_t22) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_instance: u32 = p_instance;
  let l_m: coax_t11 = p_m;
  let l_gf: coax_t22 = p_gf;
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
  l_o.x = coax_p_fivelab_project(l_w, l_gf);
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
fn coax_p_wg_u32_f(p_x: f32) -> u32 {
  let l_x: f32 = p_x;
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
fn coax_p_wg_unpack2x16unorm(p_x: u32) -> vec2f {
  let l_x: u32 = p_x;
  return coax_p_v2((f32((l_x & 65535u)) / 0x1.fffe00p15f), (f32((l_x >> u32(16u))) / 0x1.fffe00p15f));
}
fn coax_p_fivelab_gaunt(p_p: vec3f) -> vec3f {
  let l_p: vec3f = p_p;
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
  let l_x: u32 = p_x;
  return coax_p_v4(coax_p_wg_snorm8(coax_p_wg_byte(l_x, 0u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 8u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 16u)), coax_p_wg_snorm8(coax_p_wg_byte(l_x, 24u)));
}
fn coax_p_fivelab_skin_mix(p_ra: u32, p_rb: u32, p_code: f32, p_joints: u32, p_weights: u32, p_at: vec4f, p_nw: vec3f, p_arm: bool) -> coax_t23 {
  let l_ra: u32 = p_ra;
  let l_rb: u32 = p_rb;
  let l_code: f32 = p_code;
  let l_joints: u32 = p_joints;
  let l_weights: u32 = p_weights;
  let l_at: vec4f = p_at;
  let l_nw: vec3f = p_nw;
  let l_arm: bool = p_arm;
  var l_o: coax_t23;
  var l_k: u32;
  var l_w: f32;
  var l_j: u32;
  var l_s: f32;
  var l_m: coax_t33;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_e: vec4f;
  var l_m2: coax_t33;
  l_o = coax_t23();
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
fn coax_p_fivelab_skin(p_row: u32, p_joints: u32, p_weights: u32, p_at: vec4f, p_nw: vec3f, p_arm: bool) -> coax_t23 {
  let l_row: u32 = p_row;
  let l_joints: u32 = p_joints;
  let l_weights: u32 = p_weights;
  let l_at: vec4f = p_at;
  let l_nw: vec3f = p_nw;
  let l_arm: bool = p_arm;
  var l_o: coax_t23;
  var l_k: u32;
  var l_w: f32;
  var l_m: coax_t33;
  var l_a: vec4f;
  var l_c: vec4f;
  var l_e: vec4f;
  l_o = coax_t23();
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
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_fivelab_project(p_w: vec3f, p_gf: coax_t22) -> vec4f {
  let l_w: vec3f = p_w;
  let l_gf: coax_t22 = p_gf;
  var l_rel: vec3f;
  var l_z: f32;
  var l_x: f32;
  var l_y: f32;
  var l_f: f32;
  var l_cx: f32;
  var l_cy: f32;
  var l_cz: f32;
  l_rel = coax_p_sub3(l_w, coax_p_v3_of(l_gf.f4));
  l_z = dot(l_rel, coax_p_v3_of(l_gf.f5));
  l_x = dot(l_rel, coax_p_v3_of(l_gf.f6));
  l_y = dot(l_rel, coax_p_v3_of(l_gf.f7));
  l_f = coax_p_fmax(l_gf.f4.w, 0x1.000000p0f);
  l_cx = (((((0x1.000000p1f * l_gf.f5.w) / l_gf.x.x) - 0x1.000000p0f) * l_z) + (((0x1.000000p1f * l_f) * l_x) / l_gf.x.x));
  l_cy = (((0x1.000000p0f - ((0x1.000000p1f * l_gf.f6.w) / l_gf.x.y)) * l_z) + (((0x1.000000p1f * l_f) * l_y) / l_gf.x.y));
  l_cz = ((0x1.c20000p9f * (l_z - 0x1.99999ap-4f)) / (0x1.c20000p9f - 0x1.99999ap-4f));
  return coax_p_v4(l_cx, l_cy, l_cz, l_z);
}
fn coax_p_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_env_block_of(p_at: i32) -> coax_t25 {
  let l_at: i32 = p_at;
  var l_e: coax_t25;
  var l_i: i32;
  l_e = coax_t25();
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 27i;
    for (l_i = coax_from; l_i < coax_to; l_i = l_i + 1i)
    {
      l_e.x[l_i] = coax_p_v4_at((l_at + (4i * l_i)));
    }
  }
  return l_e;
}
fn coax_p_paint_env_sky(p_e: coax_t25, p_rd: vec3f, p_sun: vec3f, p_time: f32) -> vec3f {
  let l_e: coax_t25 = p_e;
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  let l_time: f32 = p_time;
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
  l_col = coax_p_mix3(coax_p_paint_env_c3(l_e, 16i), coax_p_paint_env_c3(l_e, 12i), sqrt(coax_p_saturate(((l_up * 0x1.800000p0f) + 0x1.47ae14p-4f))));
  l_q = coax_p_v2((l_rd.x / coax_p_fmax((l_rd.y + 0x1.333334p-3f), 0x1.47ae14p-4f)), (l_rd.z / coax_p_fmax((l_rd.y + 0x1.333334p-3f), 0x1.47ae14p-4f)));
  l_p = coax_p_v2(((l_q.x * 0x1.400000p3f) + (l_time * 0x1.0624dep-10f)), (l_q.y * 0x1.c00000p3f));
  l_warp = coax_p_v2(coax_p_paint_noise(coax_p_v2((l_p.x * 0x1.ccccccp-2f), (l_p.y * 0x1.ccccccp-2f))), coax_p_paint_noise(coax_p_v2(((l_p.x * 0x1.ccccccp-2f) + 0x1.c00000p2f), ((l_p.y * 0x1.ccccccp-2f) - 0x1.800000p1f))));
  l_density = coax_p_paint_cloud_fbm(coax_p_v2((l_p.x + (l_warp.x * 0x1.99999ap-1f)), (l_p.y + (l_warp.y * 0x1.99999ap-1f))));
  l_threshold = (0x1.3d70a4p-1f - (0x1.99999ap-4f * coax_p_smoothstep((-0x1.000000p-1f), 0x1.99999ap-1f, l_rd.x)));
  l_cloud = ((coax_p_smoothstep(l_threshold, (l_threshold + 0x1.70a3d8p-4f), l_density) * coax_p_smoothstep(0.0f, 0x1.eb851ep-4f, l_rd.y)) * (0x1.333334p-3f + (0x1.b33334p-1f * coax_p_smoothstep((-0x1.333334p-3f), 0x1.666666p-1f, l_rd.x))));
  l_thickness = coax_p_saturate(((l_density - l_threshold) * 0x1.800000p2f));
  l_cloud_colour = coax_p_mix3(coax_p_paint_env_c3(l_e, 20i), coax_p_paint_env_c3(l_e, 24i), coax_p_saturate(((l_thickness * 0x1.666666p-1f) + 0x1.333334p-3f)));
  l_col = coax_p_mix3(l_col, l_cloud_colour, (l_cloud * coax_p_paint_env_c(l_e, 15i)));
  l_across = ((l_q.x * 0x1.8f5c28p-1f) + (l_q.y * 0x1.428f5cp-1f));
  l_along = (((-l_q.x) * 0x1.428f5cp-1f) + (l_q.y * 0x1.8f5c28p-1f));
  l_fine = coax_p_paint_cloud_fbm(coax_p_v2(((l_along * 0x1.800000p-1f) + (coax_p_paint_noise(coax_p_v2((l_across * 0x1.333334p-2f), (l_along * 0x1.333334p-2f))) * 0x1.99999ap-2f)), (l_across * 0x1.e00000p4f)));
  l_wisps = (((coax_pow_bg_vs(coax_p_saturate(((l_fine - 0x1.666666p-2f) * 0x1.99999ap0f)), 0x1.800000p1f) * coax_p_smoothstep(0x1.99999ap-4f, 0x1.19999ap-1f, l_rd.y)) * coax_p_paint_env_c(l_e, 27i)) * coax_p_smoothstep(0x1.666666p-2f, 0x1.666666p-1f, coax_p_paint_noise(coax_p_v2((l_q.x * 0x1.666666p-1f), (l_q.y * 0x1.666666p-1f)))));
  l_col = coax_p_mix3(l_col, coax_p_paint_env_c3(l_e, 32i), l_wisps);
  l_halo = coax_pow_bg_vs(coax_p_saturate(dot(coax_p_normalize3(l_rd), coax_p_normalize3(l_sun))), coax_p_paint_env_c(l_e, 23i));
  return coax_p_mix3(l_col, coax_p_paint_env_c3(l_e, 28i), (l_halo * coax_p_paint_env_c(l_e, 19i)));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_low: f32 = p_low;
  let l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
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
fn coax_p_fivelab_trace(p_ro: vec3f, p_rd: vec3f, p_shadow: bool) -> coax_t26 {
  let l_ro: vec3f = p_ro;
  let l_rd: vec3f = p_rd;
  let l_shadow: bool = p_shadow;
  var l_h: coax_t26;
  var l_clear: bool;
  var l_tg: f32;
  var l_g: vec3f;
  var l_i: i32;
  var l_tr: vec4f;
  var l_k: f32;
  var l_c1: vec3f;
  var l_tc: f32;
  var l_c2: vec3f;
  var l_tc2: f32;
  var l_ta: vec3f;
  var l_tb: vec3f;
  var l_tt: f32;
  l_h = coax_t26();
  l_h.x = 0x1.dcd650p29f;
  l_h.y = coax_p_v3(0.0f, 0x1.000000p0f, 0.0f);
  l_h.z = (-0x1.000000p0f);
  l_h.w = (-0x1.000000p0f);
  l_clear = false;
  if ((l_rd.y < (-0x1.a36e2ep-14f)))
  {
    l_tg = ((-l_ro.y) / l_rd.y);
    if ((!l_shadow))
    {
      l_h.x = l_tg;
      l_h.z = 0.0f;
      l_g = coax_p_add3(l_ro, coax_p_scale3(l_rd, l_tg));
      l_clear = ((abs(l_g.x) < (coax_at_u(i32(100i)) + 0x1.000000p0f)) && (abs(l_g.z) < (coax_at_u(i32(101i)) + 0x1.000000p0f)));
    }
  }
  if ((!l_clear))
  {
    l_i = 0i;
    while ((l_i < 16i))
    {
      l_tr = coax_p_fivelab_tree(l_i);
      l_k = l_tr.w;
      l_c1 = coax_p_v3(l_tr.x, (0x1.266666p2f * l_k), l_tr.z);
      l_tc = coax_p_fivelab_sphere(l_ro, l_rd, l_c1, (0x1.59999ap1f * l_k));
      if (((l_tc > 0.0f) && (l_tc < l_h.x)))
      {
        l_h.x = l_tc;
        l_h.y = coax_p_normalize3(coax_p_sub3(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_tc)), l_c1));
        l_h.z = 0x1.000000p1f;
        l_h.w = f32(l_i);
      }
      l_c2 = coax_p_v3((l_tr.x + (0x1.ccccccp-1f * l_k)), (0x1.933334p2f * l_k), (l_tr.z - (0x1.99999ap-2f * l_k)));
      l_tc2 = coax_p_fivelab_sphere(l_ro, l_rd, l_c2, (0x1.e66666p0f * l_k));
      if (((l_tc2 > 0.0f) && (l_tc2 < l_h.x)))
      {
        l_h.x = l_tc2;
        l_h.y = coax_p_normalize3(coax_p_sub3(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_tc2)), l_c2));
        l_h.z = 0x1.000000p1f;
        l_h.w = f32(l_i);
      }
      l_ta = coax_p_v3(l_tr.x, 0.0f, l_tr.z);
      l_tb = coax_p_v3(l_tr.x, (0x1.b33334p1f * l_k), l_tr.z);
      l_tt = coax_p_fivelab_capsule(l_ro, l_rd, l_ta, l_tb, (0x1.47ae14p-2f * l_k));
      if (((l_tt > 0.0f) && (l_tt < l_h.x)))
      {
        l_h.x = l_tt;
        l_h.y = coax_p_fivelab_capsule_n(coax_p_add3(l_ro, coax_p_scale3(l_rd, l_tt)), l_ta, l_tb);
        l_h.z = 0x1.000000p0f;
        l_h.w = f32(l_i);
      }
      l_i = (l_i + 1i);
    }
  }
  return l_h;
}
fn coax_p_fivelab_sky(p_rd: vec3f, p_sun: vec3f, p_t: f32) -> vec3f {
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  let l_t: f32 = p_t;
  var l_up: f32;
  var l_st: f32;
  var l_c: vec3f;
  var l_halo: f32;
  var l_disc: f32;
  var l_k: f32;
  var l_x: f32;
  var l_z: f32;
  var l_n: f32;
  var l_cover: f32;
  var l_lit: f32;
  var l_cloud: vec3f;
  l_up = coax_p_fmax(l_rd.y, 0.0f);
  l_st = coax_p_metaverse_style_sky_t(l_up);
  l_c = coax_p_mix3(coax_p_v3(0x1.8f5c28p-1f, 0x1.ae147ap-1f, 0x1.b851ecp-1f), coax_p_v3(0x1.47ae14p-2f, 0x1.0a3d70p-1f, 0x1.ae147ap-1f), l_st);
  l_halo = coax_p_metaverse_style_halo(dot(l_rd, l_sun), 0x1.c00000p2f);
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.b851ecp-1f, 0x1.3d70a4p-1f), (l_halo * 0x1.ccccccp-2f)));
  l_disc = coax_p_metaverse_style_halo(dot(l_rd, l_sun), 0x1.c20000p9f);
  l_c = coax_p_add3(l_c, coax_p_scale3(coax_p_v3(0x1.000000p0f, 0x1.e66666p-1f, 0x1.b33334p-1f), (l_disc * 0x1.800000p1f)));
  if ((l_rd.y > 0x1.47ae14p-6f))
  {
    l_k = (0x1.2c0000p8f / l_rd.y);
    l_x = ((l_rd.x * l_k) + (l_t * 0x1.000000p1f));
    l_z = (l_rd.z * l_k);
    l_n = coax_p_metaverse_style_cloud_field(l_x, l_z, 0x1.a36e2ep-9f, 0x1.2a3056p-7f, 0x1.000000p1f);
    l_cover = (coax_p_metaverse_style_cloud_cover(l_n, 0x1.99999ap-2f, 0x1.eb851ep-2f, 0x1.3d70a4p-1f) * coax_p_metaverse_style_cloud_fade(l_rd.y));
    l_lit = coax_p_metaverse_style_cloud_lit(l_n, coax_p_metaverse_style_cloud_field((l_x + (l_sun.x * 0x1.400000p5f)), (l_z + (l_sun.z * 0x1.400000p5f)), 0x1.a36e2ep-9f, 0x1.2a3056p-7f, 0x1.000000p1f));
    l_cloud = coax_p_mix3(coax_p_v3(0x1.3d70a4p-1f, 0x1.51eb86p-1f, 0x1.851eb8p-1f), coax_p_v3(0x1.000000p0f, 0x1.f0a3d8p-1f, 0x1.d70a3ep-1f), l_lit);
    l_c = coax_p_mix3(l_c, l_cloud, l_cover);
  }
  return l_c;
}
fn coax_p_fivelab_grass(p_x: f32, p_z: f32, p_t: f32, p_fp: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  let l_t: f32 = p_t;
  let l_fp: f32 = p_fp;
  var l_n1: f32;
  var l_n2: f32;
  var l_dab: f32;
  var l_c: vec3f;
  var l_hw: f32;
  var l_hd: f32;
  var l_sq: f32;
  var l_inside: f32;
  var l_gx: f32;
  var l_gz: f32;
  var l_lx: f32;
  var l_lz: f32;
  var l_lw: f32;
  var l_line: f32;
  l_n1 = coax_p_metaverse_style_noise((l_x * 0x1.666666p-2f), (l_z * 0x1.666666p-2f));
  l_n2 = coax_p_metaverse_style_noise(((l_x * 0x1.b33334p0f) + 0x1.400000p2f), ((l_z * 0x1.b33334p0f) + 0x1.000000p1f));
  l_dab = coax_p_metaverse_style_noise((l_x * 0x1.800000p2f), (l_z * 0x1.800000p2f));
  l_c = coax_p_mix3(coax_p_v3(0x1.47ae14p-3f, 0x1.333334p-2f, 0x1.1eb852p-4f), coax_p_v3(0x1.51eb86p-2f, 0x1.d70a3ep-2f, 0x1.0a3d70p-3f), ((l_n1 * 0x1.4cccccp-1f) + (l_n2 * 0x1.666666p-2f)));
  l_c = coax_p_scale3(l_c, (0x1.c28f5cp-1f + (0x1.eb851ep-3f * l_dab)));
  l_hw = coax_at_u(i32(100i));
  l_hd = coax_at_u(i32(101i));
  l_sq = coax_p_fmax(coax_at_u(i32(102i)), 0x1.99999ap-4f);
  l_inside = ((0x1.000000p0f - coax_p_smoothstep(l_hw, (l_hw + 0x1.333334p-2f), abs(l_x))) * (0x1.000000p0f - coax_p_smoothstep(l_hd, (l_hd + 0x1.333334p-2f), abs(l_z))));
  l_c = coax_p_mix3(l_c, coax_p_add3(coax_p_scale3(l_c, 0x1.a3d70ap-1f), coax_p_v3(0x1.99999ap-5f, 0x1.eb851ep-5f, 0x1.47ae14p-6f)), (l_inside * 0x1.333334p-1f));
  l_gx = ((l_x + l_hw) / l_sq);
  l_gz = ((l_z + l_hd) / l_sq);
  l_lx = (abs((coax_p_fract((l_gx + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_sq);
  l_lz = (abs((coax_p_fract((l_gz + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_sq);
  l_lw = coax_p_fmax(0x1.eb851ep-6f, l_fp);
  l_line = (0x1.000000p0f - coax_p_smoothstep((l_lw * 0x1.99999ap-2f), (l_lw * 0x1.800000p0f), coax_p_fmin(l_lx, l_lz)));
  l_line = ((l_line * coax_p_fmin(0x1.000000p0f, sqrt((0x1.eb851ep-6f / coax_p_fmax(l_fp, 0x1.a36e2ep-14f))))) * (0x1.000000p0f - coax_p_smoothstep((l_sq * 0x1.eb851ep-4f), (l_sq * 0x1.333334p-2f), l_fp)));
  l_c = coax_p_mix3(l_c, coax_p_v3(0x1.19999ap-1f, 0x1.333334p-1f, 0x1.70a3d8p-2f), (((l_line * l_inside) * 0x1.000000p-1f) * (0x1.000000p0f - coax_at_u(i32(35i)))));
  return l_c;
}
fn coax_p_wg_at4(p_at: i32) -> vec4f {
  let l_at: i32 = p_at;
  return coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i))));
}
fn coax_p_metaverse_style_noise(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
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
  l_a = coax_p_metaverse_style_hash(l_ix, l_iy);
  l_b = coax_p_metaverse_style_hash((l_ix + 0x1.000000p0f), l_iy);
  l_c = coax_p_metaverse_style_hash(l_ix, (l_iy + 0x1.000000p0f));
  l_d = coax_p_metaverse_style_hash((l_ix + 0x1.000000p0f), (l_iy + 0x1.000000p0f));
  return ((l_a + ((l_b - l_a) * l_sx)) + (((l_c + ((l_d - l_c) * l_sx)) - (l_a + ((l_b - l_a) * l_sx))) * l_sy));
}
fn coax_p_wg_at4_1(p_at: i32) -> vec4f {
  let l_at: i32 = p_at;
  return coax_p_v4(frame[l_at], frame[(l_at + 1i)], frame[(l_at + 2i)], frame[(l_at + 3i)]);
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  let l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_metaverse_style_wrap(p_ndl: f32) -> f32 {
  let l_ndl: f32 = p_ndl;
  return coax_p_st_sat(((l_ndl + 0x1.000000p-2f) / 0x1.400000p0f));
}
fn coax_p_metaverse_style_bands(p_w: f32, p_e0: f32, p_e1: f32, p_s: f32, p_mid: f32) -> f32 {
  let l_w: f32 = p_w;
  let l_e0: f32 = p_e0;
  let l_e1: f32 = p_e1;
  let l_s: f32 = p_s;
  let l_mid: f32 = p_mid;
  return ((l_mid * coax_p_metaverse_style_soft_step(l_e0, l_s, l_w)) + ((0x1.000000p0f - l_mid) * coax_p_metaverse_style_soft_step(l_e1, l_s, l_w)));
}
fn coax_p_metaverse_style_ambient(p_ny: f32, p_sky: f32, p_ground: f32, p_scale: f32) -> f32 {
  let l_ny: f32 = p_ny;
  let l_sky: f32 = p_sky;
  let l_ground: f32 = p_ground;
  let l_scale: f32 = p_scale;
  return ((l_ground + ((l_sky - l_ground) * coax_p_st_sat(((l_ny * 0x1.000000p-1f) + 0x1.000000p-1f)))) * l_scale);
}
fn coax_p_metaverse_style_shade(p_albedo: f32, p_shadow_light: f32, p_lit_light: f32, p_band: f32) -> f32 {
  let l_albedo: f32 = p_albedo;
  let l_shadow_light: f32 = p_shadow_light;
  let l_lit_light: f32 = p_lit_light;
  let l_band: f32 = p_band;
  return (l_albedo * (l_shadow_light + ((l_lit_light - l_shadow_light) * coax_p_st_sat(l_band))));
}
fn coax_p_metaverse_style_halo(p_cos_t: f32, p_k: f32) -> f32 {
  let l_cos_t: f32 = p_cos_t;
  let l_k: f32 = p_k;
  return exp(((-l_k) * coax_p_st_max((0x1.000000p0f - l_cos_t), 0.0f)));
}
fn coax_p_fivelab_depth(p_z: f32) -> f32 {
  let l_z: f32 = p_z;
  if ((l_z <= 0x1.99999ap-4f))
  {
    return 0.0f;
  }
  return coax_p_fmin(((0x1.c20000p9f * (l_z - 0x1.99999ap-4f)) / ((0x1.c20000p9f - 0x1.99999ap-4f) * l_z)), 0x1.000000p0f);
}
fn coax_p_metaverse_style_tone(p_c: f32, p_exposure: f32) -> f32 {
  let l_c: f32 = p_c;
  let l_exposure: f32 = p_exposure;
  return (0x1.000000p0f - exp(((-coax_p_st_max(l_c, 0.0f)) * l_exposure)));
}
fn coax_p_metaverse_style_saturation(p_c: f32, p_lum: f32, p_amount: f32) -> f32 {
  let l_c: f32 = p_c;
  let l_lum: f32 = p_lum;
  let l_amount: f32 = p_amount;
  return coax_p_st_max(0.0f, (l_lum + ((l_c - l_lum) * l_amount)));
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
fn coax_p_fivelab_lines(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  var l_s: f32;
  var l_x: f32;
  var l_y: f32;
  var l_dx: f32;
  var l_dy: f32;
  var l_line: f32;
  var l_mx: f32;
  var l_my: f32;
  var l_major: f32;
  l_s = coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f);
  l_x = ((l_p.x - coax_at_u(i32(4i))) / l_s);
  l_y = ((l_p.y - coax_at_u(i32(5i))) / l_s);
  l_dx = (abs((coax_p_fract((l_x + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_s);
  l_dy = (abs((coax_p_fract((l_y + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_s);
  l_line = (0x1.000000p0f - coax_p_smoothstep(0x1.333334p-2f, 0x1.19999ap0f, coax_p_fmin(l_dx, l_dy)));
  l_mx = ((abs((coax_p_fract(((l_x * 0x1.000000p-2f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_s) * 0x1.000000p2f);
  l_my = ((abs((coax_p_fract(((l_y * 0x1.000000p-2f) + 0x1.000000p-1f)) - 0x1.000000p-1f)) * l_s) * 0x1.000000p2f);
  l_major = (0x1.000000p0f - coax_p_smoothstep(0x1.99999ap-2f, 0x1.666666p0f, coax_p_fmin(l_mx, l_my)));
  return ((l_line * 0x1.333334p-1f) + (l_major * 0x1.99999ap-2f));
}
fn coax_p_fivelab_flat_tokens(p_p: vec2f, p_c0: vec3f) -> vec3f {
  let l_p: vec2f = p_p;
  let l_c0: vec3f = p_c0;
  var l_c: vec3f;
  var l_s: f32;
  var l_gx: f32;
  var l_gy: f32;
  var l_cx: i32;
  var l_cy: i32;
  var l_cols: i32;
  var l_rows: i32;
  var l_r: f32;
  var l_w: f32;
  var l_dy: i32;
  var l_dx: i32;
  var l_x: i32;
  var l_y: i32;
  var l_who: f32;
  var l_at: i32;
  var l_tk: vec4f;
  var l_code: f32;
  var l_state: f32;
  var l_tm: f32;
  var l_col: vec3f;
  var l_a: f32;
  var l_d: f32;
  var l_cover: f32;
  var l_rt: f32;
  var l_ring: f32;
  l_c = l_c0;
  l_s = coax_p_fmax(coax_at_u(i32(6i)), 0x1.000000p0f);
  l_gx = ((l_p.x - coax_at_u(i32(4i))) / l_s);
  l_gy = ((l_p.y - coax_at_u(i32(5i))) / l_s);
  l_cx = i32(floor(l_gx));
  l_cy = i32(floor(l_gy));
  l_cols = i32(coax_at_u(i32(8i)));
  l_rows = i32(coax_at_u(i32(9i)));
  l_r = (l_s * 0x1.5c28f6p-2f);
  l_w = coax_p_fmin(coax_p_fmax((l_s * 0x1.eb851ep-5f), 0x1.000000p0f), 0x1.4cccccp1f);
  l_dy = (-1i);
  while ((l_dy <= 1i))
  {
    l_dx = (-1i);
    while ((l_dx <= 1i))
    {
      l_x = (l_cx + l_dx);
      l_y = (l_cy + l_dy);
      if (((((l_x >= 0i) && (l_x < l_cols)) && (l_y >= 0i)) && (l_y < l_rows)))
      {
        l_who = frame[((69632i + (l_y * l_cols)) + l_x)];
        if ((l_who > 0x1.000000p-1f))
        {
          l_at = ((i32(l_who) - 1i) * 4i);
          l_tk = coax_p_v4(frame[l_at], frame[(l_at + 1i)], frame[(l_at + 2i)], frame[(l_at + 3i)]);
          if ((l_tk.z > 0x1.000000p-1f))
          {
            l_code = (l_tk.z - 0x1.000000p0f);
            l_state = floor((l_code / 0x1.000000p2f));
            l_tm = (l_code - (0x1.000000p2f * l_state));
            l_col = coax_p_v3(0x1.ccccccp-2f, 0x1.e66666p-1f, 0x1.19999ap-1f);
            if ((l_tm > 0x1.000000p-1f))
            {
              l_col = coax_p_v3(0x1.3d70a4p-1f, 0x1.b33334p-1f, 0x1.000000p0f);
            }
            if ((l_tm > 0x1.800000p0f))
            {
              l_col = coax_p_v3(0x1.ccccccp-1f, 0x1.b851ecp-1f, 0x1.7ae148p-1f);
            }
            l_a = 0x1.e66666p-1f;
            if ((l_state > 0x1.000000p-1f))
            {
              l_col = coax_p_v3(0x1.0a3d70p-1f, 0x1.19999ap-1f, 0x1.333334p-1f);
              l_a = 0x1.666666p-2f;
            }
            if ((l_state > 0x1.800000p0f))
            {
              l_col = coax_p_v3(0x1.1eb852p-2f, 0x1.333334p-2f, 0x1.5c28f6p-2f);
              l_a = 0x1.333334p-2f;
            }
            l_d = coax_p_wg_length2(coax_p_v2((l_p.x - (coax_at_u(i32(4i)) + ((l_tk.x + 0x1.000000p-1f) * l_s))), (l_p.y - (coax_at_u(i32(5i)) + ((l_tk.y + 0x1.000000p-1f) * l_s)))));
            l_cover = 0.0f;
            if ((l_r < 0x1.400000p1f))
            {
              l_cover = (0x1.000000p0f - coax_p_smoothstep(coax_p_fmax(l_r, 0x1.666666p-1f), (coax_p_fmax(l_r, 0x1.666666p-1f) + 0x1.99999ap-1f), l_d));
            }
            else
            {
              l_cover = (0x1.000000p0f - coax_p_smoothstep((l_w * 0x1.000000p-1f), ((l_w * 0x1.000000p-1f) + 0x1.000000p0f), abs((l_d - l_r))));
            }
            l_c = coax_p_mix3(l_c, l_col, (l_cover * l_a));
            if ((((l_tk.w > 0.0f) && (l_state < 0x1.000000p-1f)) && (l_r >= 0x1.400000p1f)))
            {
              l_rt = (l_r * (0x1.400000p0f + ((0x1.000000p0f - l_tk.w) * 0x1.eb851ep-3f)));
              l_ring = (0x1.000000p0f - coax_p_smoothstep(0x1.666666p-1f, 0x1.b33334p0f, abs((l_d - l_rt))));
              l_c = coax_p_mix3(l_c, coax_p_v3(0x1.ccccccp-1f, 0x1.d70a3ep-1f, 0x1.eb851ep-1f), ((l_ring * 0x1.ccccccp-1f) * l_tk.w));
            }
          }
        }
      }
      l_dx = (l_dx + 1i);
    }
    l_dy = (l_dy + 1i);
  }
  return l_c;
}
fn coax_p_render_sample_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> vec4f {
  let l_uv: vec2f = p_uv;
  let l_layer: i32 = p_layer;
  let l_dx: vec2f = p_dx;
  let l_dy: vec2f = p_dy;
  let l_bias: f32 = p_bias;
  var l_rs_result: coax_t34;
  l_rs_result = coax_p_render_filter_layer8_grad(l_uv, l_layer, l_dx, l_dy, l_bias);
  if ((!l_rs_result.x))
  {
    discard;
  }
  return l_rs_result.y;
}
fn coax_p_fivelab_dye(p_t: vec3f, p_dye: vec4f) -> vec3f {
  let l_t: vec3f = p_t;
  let l_dye: vec4f = p_dye;
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
  l_light = coax_p_metaverse_style_luma(l_t.x, l_t.y, l_t.z);
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
fn coax_p_mul3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
}
fn coax_p_e2_eye_layer(p_slice: u32) -> bool {
  let l_slice: u32 = p_slice;
  return (i32((mats[(i32(l_slice) * 4i)].w + 0x1.000000p-1f)) == 3i);
}
fn coax_p_fract(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return (l_x - floor(l_x));
}
fn coax_p_sub3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_fivelab_mark(p_marks: f32, p_k: f32) -> bool {
  let l_marks: f32 = p_marks;
  let l_k: f32 = p_k;
  return (coax_p_wg_fmod(floor((l_marks / coax_p_wg_exp2(l_k))), 0x1.000000p1f) > 0x1.000000p-1f);
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
fn coax_p_metaverse_style_bands2(p_w: f32, p_e: f32, p_s: f32) -> f32 {
  let l_w: f32 = p_w;
  let l_e: f32 = p_e;
  let l_s: f32 = p_s;
  return coax_p_metaverse_style_soft_step(l_e, l_s, l_w);
}
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_fivelab_hud_fs(p_v: coax_t17, p_gf: coax_t22) -> vec4f {
  let l_v: coax_t17 = p_v;
  let l_gf: coax_t22 = p_gf;
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
    return coax_p_v4(l_c.x, l_c.y, l_c.z, coax_p_saturate(((l_cov * l_a) * l_gf.x.w)));
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
    return coax_p_v4(l_v.w.x, l_v.w.y, l_v.w.z, coax_p_saturate(((coax_pow_bg_vs(l_face, 0x1.b33334p-1f) * coax_p_fmin((l_v.z.z * 0x1.266666p0f), 0x1.000000p0f)) * l_gf.x.w)));
  }
  l_along = coax_p_fmax(coax_p_fmax((-l_v.y.x), (l_v.y.x - l_v.z.x)), 0.0f);
  l_dd = coax_p_wg_length2(coax_p_v2(l_along, l_v.y.y));
  l_core = (0x1.000000p0f - coax_p_smoothstep((l_v.z.y - 0x1.000000p-1f), (l_v.z.y + 0x1.333334p-1f), l_dd));
  l_gg = (l_dd / coax_p_fmax(l_v.z.w, 0x1.000000p0f));
  l_halo = (exp((((-0x1.400000p2f) * l_gg) * l_gg)) * 0x1.1eb852p-2f);
  return coax_p_v4(l_v.w.x, l_v.w.y, l_v.w.z, coax_p_saturate((((l_core + l_halo) * l_v.z.z) * l_gf.x.w)));
}
fn coax_p_wg_log2(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return (log(l_x) / 0x1.62e430p-1f);
}
fn coax_p_wg_make_f4_f3f(p_p0: vec3f, p_p1: f32) -> vec4f {
  let l_p0: vec3f = p_p0;
  let l_p1: f32 = p_p1;
  return coax_p_v4(l_p0.x, l_p0.y, l_p0.z, l_p1);
}
fn coax_p_e2_field_cell(p_t: i32) -> f32 {
  let l_t: i32 = p_t;
  return select(select(select(0x1.000000p2f, 0x1.000000p1f, (l_t == 2i)), 0x1.000000p0f, (l_t == 1i)), 0x1.000000p-1f, (l_t == 0i));
}
fn coax_p_e2_field_fine_x(p_t: i32, p_x: u32, p_z: u32) -> u32 {
  let l_t: i32 = p_t;
  let l_x: u32 = p_x;
  let l_z: u32 = p_z;
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
  let l_t: i32 = p_t;
  let l_x: u32 = p_x;
  let l_z: u32 = p_z;
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
  let l_x: u32 = p_x;
  let l_z: u32 = p_z;
  let l_k: u32 = p_k;
  return coax_p_e2_ihash((((l_x * 73856093u) ^ (l_z * 19349663u)) ^ ((l_k + 1u) * 83492791u)));
}
fn coax_p_e2_unit(p_h: u32) -> f32 {
  let l_h: u32 = p_h;
  return (f32((l_h >> u32(8u))) / 0x1.000000p24f);
}
fn coax_p_e2_field_start_r(p_t: i32, p_g: f32, p_c: f32) -> f32 {
  let l_t: i32 = p_t;
  let l_g: f32 = p_g;
  let l_c: f32 = p_c;
  if (((l_t == 0i) || ((l_t == 1i) && coax_p_e2_field_fine_off(l_c))))
  {
    return 0.0f;
  }
  return ((0x1.000000p4f + (f32(l_t) * 0x1.800000p3f)) * l_g);
}
fn coax_p_e2_field_out_r(p_t: i32, p_g: f32, p_c: f32) -> f32 {
  let l_t: i32 = p_t;
  let l_g: f32 = p_g;
  let l_c: f32 = p_c;
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
fn coax_p_e2_field_rel_r(p_t: i32, p_d: f32, p_g: f32, p_c: f32) -> f32 {
  let l_t: i32 = p_t;
  let l_d: f32 = p_d;
  let l_g: f32 = p_g;
  let l_c: f32 = p_c;
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
  let l_t: i32 = p_t;
  let l_x: u32 = p_x;
  let l_z: u32 = p_z;
  let l_fx: u32 = p_fx;
  let l_fz: u32 = p_fz;
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
  let l_rel: f32 = p_rel;
  let l_rank: f32 = p_rank;
  var l_v: f32;
  l_v = ((l_rel - l_rank) / 0x1.47ae14p-4f);
  return select(select(l_v, 0x1.000000p0f, (l_v > 0x1.000000p0f)), 0.0f, (l_v < 0.0f));
}
fn coax_p_e2_meadow_gold(p_x: f32, p_z: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  return (coax_p_e2_smooth(0x1.99999ap-2f, 0x1.3d70a4p-1f, coax_p_e2_noise(((l_x / 0x1.e00000p5f) + 0x1.8cccccp1f), ((l_z / 0x1.e00000p5f) - 0x1.b33334p0f))) * 0x1.333334p-2f);
}
fn coax_p_exp_height_gpu(p_x: f32, p_z: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
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
  let l_fx: u32 = p_fx;
  let l_fz: u32 = p_fz;
  return ((0x1.c28f5cp-3f + (0x1.0a3d70p-2f * coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 4u)))) / 0x1.4fcce2p0f);
}
fn coax_p_e2_meadow_tuft_sh(p_fx: u32, p_fz: u32) -> f32 {
  let l_fx: u32 = p_fx;
  let l_fz: u32 = p_fz;
  return ((0x1.19999ap-1f + (0x1.666666p-2f * coax_p_e2_unit(coax_p_e2_field_hash(l_fx, l_fz, 7u)))) * 0x1.b33334p-1f);
}
fn coax_p_cg_meadow_place(p_id: u32, p_a: vec4f, p_sv: f32, p_sh: f32, p_tint: vec3f, p_autumn: f32, p_m: coax_t11, p_gf: coax_t22, p_who: u32) -> coax_t15 {
  let l_id: u32 = p_id;
  let l_a: vec4f = p_a;
  let l_sv: f32 = p_sv;
  let l_sh: f32 = p_sh;
  let l_tint: vec3f = p_tint;
  let l_autumn: f32 = p_autumn;
  let l_m: coax_t11 = p_m;
  let l_gf: coax_t22 = p_gf;
  let l_who: u32 = p_who;
  var l_b: u32;
  var l_source: vec3f;
  var l_normal: u32;
  var l_sid: u32;
  var l_r: coax_t30;
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
  l_sid = ((l_normal >> u32(24u)) & 7u);
  l_r = coax_p_e2_surface_id(i32(l_sid));
  l_p = coax_p_cg_instance_point(l_source, coax_p_v3(l_a.x, l_a.y, l_a.z), l_sv, l_sh, l_a.w);
  if (coax_p_e2_surf_is(l_r, 256i))
  {
    l_sway = (sin((((l_a.x * 0x1.7ae148p-2f) + (l_a.z * 0x1.ae147ap-3f)) + (l_gf.x.z * 0x1.19999ap0f))) + (sin(((l_a.z * 0x1.0f5c28p-1f) + (l_gf.x.z * 0x1.666666p-1f))) * 0x1.666666p-2f));
    l_p.x = (l_p.x + (((l_sway * 0x1.99999ap-5f) * coax_p_saturate((l_source.y * 0x1.70a3d8p-3f))) * l_sv));
  }
  l_nw = coax_p_wg_unpack4x8snorm(l_normal);
  l_cs = cos(l_a.w);
  l_sn = sin(l_a.w);
  l_o = coax_t15();
  l_o.x = coax_p_fivelab_project(l_p, l_gf);
  l_o.y = l_p;
  l_o.z = coax_p_normalize3(coax_p_v3((((l_nw.x * l_cs) + (l_nw.z * l_sn)) / l_sh), (l_nw.y / l_sv), ((((-l_nw.x) * l_sn) + (l_nw.z * l_cs)) / l_sh)));
  l_o.w = coax_p_wg_unpack2x16unorm(vertices[i32((l_b + 4u))]);
  if (coax_p_e2_surf_is(l_r, 512i))
  {
    l_o.w = coax_p_e2_tiled_uv(l_o.w, l_r);
  }
  l_vt = coax_p_wg_unpack4x8unorm(vertices[i32((l_b + 5u))]);
  l_o.f4 = vertices[i32((l_b + 6u))];
  l_o.f5 = select(coax_p_v3(l_vt.x, l_vt.y, l_vt.z), select(coax_p_mul3(coax_p_v3(l_vt.x, l_vt.y, l_vt.z), l_tint), l_tint, coax_p_e2_surf_is(l_r, 2i)), coax_p_e2_surf_is(l_r, 1024i));
  l_o.f6 = coax_p_v4(0x1.000000p0f, 0.0f, 0.0f, 0x1.800000p3f);
  l_o.f7 = coax_p_v3(f32(l_sid), bitcast<f32>(vertices[i32((l_b + 7u))]), 0.0f);
  l_o.f8 = l_who;
  if (coax_p_e2_surf_is(l_r, 4i))
  {
    l_height = coax_p_saturate(l_source.y);
    l_o.f5 = coax_p_mix3(l_tint, coax_p_v3((0x1.4cccccp-1f + (0x1.1eb852p-2f * l_autumn)), (0x1.c28f5cp-1f - (0x1.99999ap-3f * l_autumn)), 0x1.1eb852p-4f), l_height);
    l_o.f7 = coax_p_v3(0x1.000000p1f, l_height, 0x1.000000p0f);
  }
  return l_o;
}
fn coax_p_e2_meadow_cell(p_kit: i32) -> f32 {
  let l_kit: i32 = p_kit;
  var l_s: f32;
  l_s = coax_p_e2_meadow_share(l_kit);
  return select(sqrt((0x1.666666p-1f / (0x1.b00000p-3f * l_s))), 0.0f, (l_s <= 0.0f));
}
fn coax_p_e2_meadow_held(p_gx: u32, p_gz: u32, p_kit: i32) -> bool {
  let l_gx: u32 = p_gx;
  let l_gz: u32 = p_gz;
  let l_kit: i32 = p_kit;
  return (coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((100i + l_kit)))) < 0x1.666666p-1f);
}
fn coax_p_e2_meadow_at_x(p_gx: u32, p_gz: u32, p_kit: i32) -> f32 {
  let l_gx: u32 = p_gx;
  let l_gz: u32 = p_gz;
  let l_kit: i32 = p_kit;
  return ((-0x1.000000p15f) + (((f32(l_gx) + 0x1.99999ap-4f) + (0x1.99999ap-1f * coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((60i + l_kit)))))) * coax_p_e2_meadow_cell(l_kit)));
}
fn coax_p_e2_meadow_at_z(p_gx: u32, p_gz: u32, p_kit: i32) -> f32 {
  let l_gx: u32 = p_gx;
  let l_gz: u32 = p_gz;
  let l_kit: i32 = p_kit;
  return ((-0x1.000000p15f) + (((f32(l_gz) + 0x1.99999ap-4f) + (0x1.99999ap-1f * coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((80i + l_kit)))))) * coax_p_e2_meadow_cell(l_kit)));
}
fn coax_p_e2_meadow_seed(p_gx: u32, p_gz: u32, p_kit: i32) -> f32 {
  let l_gx: u32 = p_gx;
  let l_gz: u32 = p_gz;
  let l_kit: i32 = p_kit;
  return ((coax_p_e2_unit(coax_p_e2_field_hash(l_gx, l_gz, u32((40i + l_kit)))) * 0x1.840000p6f) + 0x1.000000p-1f);
}
fn coax_p_cg_hash(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  var l_v: f32;
  l_v = (sin(((l_a * 0x1.fc6666p6f) + (l_b * 0x1.37b334p8f))) * 0x1.55dd18p15f);
  return (l_v - floor(l_v));
}
fn coax_p_e2_meadow_fade(p_kit: i32, p_d: f32) -> f32 {
  let l_kit: i32 = p_kit;
  let l_d: f32 = p_d;
  var l_r: f32;
  l_r = coax_p_e2_meadow_reach(l_kit);
  return (0x1.000000p0f - coax_p_cg_ramp((0x1.800000p-1f * l_r), l_r, l_d));
}
fn coax_p_e2_meadow_piece(p_kit: i32, p_h: f32, p_autumn: f32) -> coax_t29 {
  let l_kit: i32 = p_kit;
  let l_h: f32 = p_h;
  let l_autumn: f32 = p_autumn;
  var l_sz: f32;
  var l_p: coax_t29;
  var l_s: f32;
  var l_s_6: f32;
  var l_s_7: f32;
  var l_g: f32;
  var l_s_9: f32;
  var l_s_10: f32;
  l_sz = coax_p_cg_hash(l_h, 0x1.e00000p3f);
  l_p = coax_t29();
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
fn coax_p_cg_instance_point(p_p: vec3f, p_center: vec3f, p_vertical: f32, p_horizontal: f32, p_angle: f32) -> vec3f {
  let l_p: vec3f = p_p;
  let l_center: vec3f = p_center;
  let l_vertical: f32 = p_vertical;
  let l_horizontal: f32 = p_horizontal;
  let l_angle: f32 = p_angle;
  var l_cs: f32;
  var l_sn: f32;
  l_cs = cos(l_angle);
  l_sn = sin(l_angle);
  return coax_p_add3(l_center, coax_p_v3((((l_p.x * l_cs) + (l_p.z * l_sn)) * l_horizontal), (l_p.y * l_vertical), ((((-l_p.x) * l_sn) + (l_p.z * l_cs)) * l_horizontal)));
}
fn coax_p_wg_unpack4x8unorm(p_x: u32) -> vec4f {
  let l_x: u32 = p_x;
  return coax_p_v4((f32(coax_p_wg_byte(l_x, 0u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 8u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 16u)) / 0x1.fe0000p7f), (f32(coax_p_wg_byte(l_x, 24u)) / 0x1.fe0000p7f));
}
fn coax_p_cg_autumn(p_x: f32, p_z: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
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
fn coax_p_cg_noise(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  return coax_p_paint_noise(l_p);
}
fn coax_p_e2_surface_id(p_id: i32) -> coax_t30 {
  let l_id: i32 = p_id;
  var l_k: i32;
  var l_d: vec4f;
  l_k = l_id;
  if (((l_k < 0i) || (l_k >= 11i)))
  {
    l_k = 10i;
  }
  l_d = mats[(((112i + l_k) * 4i) + 3i)];
  return coax_t30(u32(i32((l_d.x + 0x1.000000p-1f))), l_d.y, l_d.w);
}
fn coax_p_e2_surf_is(p_m: coax_t30, p_flag: i32) -> bool {
  let l_m: coax_t30 = p_m;
  let l_flag: i32 = p_flag;
  return ((l_m.x & u32(l_flag)) != 0u);
}
fn coax_p_v3_of(p_v: vec4f) -> vec3f {
  let l_v: vec4f = p_v;
  return vec3f(l_v.x, l_v.y, l_v.z);
}
fn coax_p_cg_sky_ray(p_x: f32, p_y: f32) -> vec3f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
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
fn coax_p_e2_village(p_m: coax_t30, p_marker: f32) -> bool {
  let l_m: coax_t30 = p_m;
  let l_marker: f32 = p_marker;
  return (coax_p_e2_surf_is(l_m, 4096i) && (l_marker >= 0x1.900000p6f));
}
fn coax_p_e2_tiled_uv(p_uv: vec2f, p_m: coax_t30) -> vec2f {
  let l_uv: vec2f = p_uv;
  let l_m: coax_t30 = p_m;
  return coax_p_v2(((l_uv.x * l_m.z) - (l_m.z * 0x1.000000p-1f)), ((l_uv.y * l_m.z) - (l_m.z * 0x1.000000p-1f)));
}
fn coax_p_wg_trunc(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-floor((-l_x)));
  }
  return floor(l_x);
}
fn coax_p_wg_dot4(p_a: vec4f, p_b: vec4f) -> f32 {
  let l_a: vec4f = p_a;
  let l_b: vec4f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_fivelab_gait_share(p_code: f32, p_j: u32) -> f32 {
  let l_code: f32 = p_code;
  let l_j: u32 = p_j;
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
fn coax_p_wg_byte(p_x: u32, p_at: u32) -> u32 {
  let l_x: u32 = p_x;
  let l_at: u32 = p_at;
  return ((l_x >> u32(l_at)) & 255u);
}
fn coax_p_wg_snorm8(p_b: u32) -> f32 {
  let l_b: u32 = p_b;
  var l_s: f32;
  l_s = f32(l_b);
  if ((l_b >= 128u))
  {
    l_s = (l_s - 0x1.000000p8f);
  }
  return coax_p_fmax((l_s / 0x1.fc0000p6f), (-0x1.000000p0f));
}
fn coax_p_fivelab_rows(p_row: u32, p_j: u32, p_arm: bool) -> coax_t33 {
  let l_row: u32 = p_row;
  let l_j: u32 = p_j;
  let l_arm: bool = p_arm;
  var l_r: u32;
  var l_m: coax_t33;
  l_r = (l_row + (l_j * 3u));
  l_m = coax_t33();
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
  let l_a: vec4f = p_a;
  let l_b: vec4f = p_b;
  let l_t: f32 = p_t;
  return coax_p_v4((l_a.x + ((l_b.x - l_a.x) * l_t)), (l_a.y + ((l_b.y - l_a.y) * l_t)), (l_a.z + ((l_b.z - l_a.z) * l_t)), (l_a.w + ((l_b.w - l_a.w) * l_t)));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_paint_env_c3(p_e: coax_t25, p_c: i32) -> vec3f {
  let l_e: coax_t25 = p_e;
  let l_c: i32 = p_c;
  return coax_p_env_block_w3(l_e, (20i + l_c));
}
fn coax_p_paint_noise(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  var l_i: vec2f;
  var l_f: vec2f;
  var l_a: f32;
  var l_b: f32;
  var l_c: f32;
  var l_d: f32;
  l_i = coax_p_v2(floor(l_p.x), floor(l_p.y));
  l_f = coax_p_v2(coax_p_fract(l_p.x), coax_p_fract(l_p.y));
  l_f = coax_p_v2(((l_f.x * l_f.x) * (0x1.800000p1f - (0x1.000000p1f * l_f.x))), ((l_f.y * l_f.y) * (0x1.800000p1f - (0x1.000000p1f * l_f.y))));
  l_a = coax_p_metaverse_style_hash(l_i.x, l_i.y);
  l_b = coax_p_metaverse_style_hash((l_i.x + 0x1.000000p0f), l_i.y);
  l_c = coax_p_metaverse_style_hash(l_i.x, (l_i.y + 0x1.000000p0f));
  l_d = coax_p_metaverse_style_hash((l_i.x + 0x1.000000p0f), (l_i.y + 0x1.000000p0f));
  return coax_p_mix(coax_p_mix(l_a, l_b, l_f.x), coax_p_mix(l_c, l_d, l_f.x), l_f.y);
}
fn coax_p_paint_cloud_fbm(p_p: vec2f) -> f32 {
  let l_p: vec2f = p_p;
  return ((((coax_p_paint_noise(l_p) * 0x1.110cb2p-1f) + (coax_p_paint_noise(coax_p_v2(((l_p.x * 0x1.03d70ap1f) + 0x1.c00000p2f), ((l_p.y * 0x1.03d70ap1f) - 0x1.800000p1f))) * 0x1.1119cep-2f)) + (coax_p_paint_noise(coax_p_v2(((l_p.x * 0x1.070a3ep2f) - 0x1.400000p2f), ((l_p.y * 0x1.070a3ep2f) + 0x1.600000p3f))) * 0x1.10ff98p-3f)) + (coax_p_paint_noise(coax_p_v2(((l_p.x * 0x1.09eb86p3f) + 0x1.a00000p3f), (l_p.y * 0x1.09eb86p3f))) * 0x1.113404p-4f));
}
fn coax_p_paint_env_c(p_e: coax_t25, p_c: i32) -> f32 {
  let l_e: coax_t25 = p_e;
  let l_c: i32 = p_c;
  return coax_p_env_block_w(l_e, (20i + l_c));
}
fn coax_p_fivelab_tree(p_i: i32) -> vec4f {
  let l_i: i32 = p_i;
  var l_fi: f32;
  var l_a: f32;
  var l_rx: f32;
  var l_rz: f32;
  var l_size: f32;
  l_fi = f32(l_i);
  l_a = ((l_fi * 0x1.921fb6p-2f) + (0x1.666666p-2f * sin((l_fi * 0x1.266666p1f))));
  l_rx = ((coax_at_u(i32(100i)) + 0x1.600000p2f) + (0x1.400000p1f * coax_p_metaverse_style_hash(l_fi, 0x1.800000p1f)));
  l_rz = ((coax_at_u(i32(101i)) + 0x1.400000p2f) + (0x1.400000p1f * coax_p_metaverse_style_hash(l_fi, 0x1.c00000p2f)));
  l_size = (0x1.99999ap-1f + (0x1.ccccccp-2f * coax_p_metaverse_style_hash(l_fi, 0x1.600000p3f)));
  return coax_p_v4((cos(l_a) * l_rx), 0.0f, (sin(l_a) * l_rz), l_size);
}
fn coax_p_fivelab_sphere(p_ro: vec3f, p_rd: vec3f, p_c: vec3f, p_r: f32) -> f32 {
  let l_ro: vec3f = p_ro;
  let l_rd: vec3f = p_rd;
  let l_c: vec3f = p_c;
  let l_r: f32 = p_r;
  var l_oc: vec3f;
  var l_b: f32;
  var l_k: f32;
  var l_h: f32;
  var l_t: f32;
  l_oc = coax_p_sub3(l_ro, l_c);
  l_b = dot(l_oc, l_rd);
  l_k = (dot(l_oc, l_oc) - (l_r * l_r));
  l_h = ((l_b * l_b) - l_k);
  if ((l_h < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  l_t = ((-l_b) - sqrt(l_h));
  if ((l_t < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return l_t;
}
fn coax_p_fivelab_capsule(p_ro: vec3f, p_rd: vec3f, p_a: vec3f, p_b: vec3f, p_r: f32) -> f32 {
  let l_ro: vec3f = p_ro;
  let l_rd: vec3f = p_rd;
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_r: f32 = p_r;
  var l_ba: vec3f;
  var l_oa: vec3f;
  var l_baba: f32;
  var l_bard: f32;
  var l_baoa: f32;
  var l_rdoa: f32;
  var l_oaoa: f32;
  var l_qa: f32;
  var l_qb: f32;
  var l_qc: f32;
  var l_h: f32;
  var l_t: f32;
  var l_y: f32;
  var l_oc: vec3f;
  var l_b2: f32;
  var l_c2: f32;
  var l_h2: f32;
  var l_t2: f32;
  l_ba = coax_p_sub3(l_b, l_a);
  l_oa = coax_p_sub3(l_ro, l_a);
  l_baba = dot(l_ba, l_ba);
  l_bard = dot(l_ba, l_rd);
  l_baoa = dot(l_ba, l_oa);
  l_rdoa = dot(l_rd, l_oa);
  l_oaoa = dot(l_oa, l_oa);
  l_qa = (l_baba - (l_bard * l_bard));
  l_qb = ((l_baba * l_rdoa) - (l_baoa * l_bard));
  l_qc = (((l_baba * l_oaoa) - (l_baoa * l_baoa)) - ((l_r * l_r) * l_baba));
  l_h = ((l_qb * l_qb) - (l_qa * l_qc));
  if ((l_h < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  l_t = (((-l_qb) - sqrt(l_h)) / coax_p_fmax(l_qa, 0x1.0c6f7ap-20f));
  l_y = (l_baoa + (l_t * l_bard));
  if (((l_y > 0.0f) && (l_y < l_baba)))
  {
    if ((l_t < 0.0f))
    {
      return (-0x1.000000p0f);
    }
    return l_t;
  }
  l_oc = l_oa;
  if ((l_y >= l_baba))
  {
    l_oc = coax_p_sub3(l_ro, l_b);
  }
  l_b2 = dot(l_rd, l_oc);
  l_c2 = (dot(l_oc, l_oc) - (l_r * l_r));
  l_h2 = ((l_b2 * l_b2) - l_c2);
  if ((l_h2 < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  l_t2 = ((-l_b2) - sqrt(l_h2));
  if ((l_t2 < 0.0f))
  {
    return (-0x1.000000p0f);
  }
  return l_t2;
}
fn coax_p_fivelab_capsule_n(p_p: vec3f, p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_p: vec3f = p_p;
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  var l_ba: vec3f;
  var l_pa: vec3f;
  var l_h: f32;
  l_ba = coax_p_sub3(l_b, l_a);
  l_pa = coax_p_sub3(l_p, l_a);
  l_h = coax_p_saturate((dot(l_pa, l_ba) / coax_p_fmax(dot(l_ba, l_ba), 0x1.0c6f7ap-20f)));
  return coax_p_normalize3(coax_p_sub3(l_pa, coax_p_scale3(l_ba, l_h)));
}
fn coax_p_metaverse_style_sky_t(p_up: f32) -> f32 {
  let l_up: f32 = p_up;
  return coax_pow_bg_vs(coax_p_st_max(l_up, 0x1.0c6f7ap-20f), 0x1.19999ap-1f);
}
fn coax_p_metaverse_style_cloud_field(p_x: f32, p_z: f32, p_s1: f32, p_s2: f32, p_octaves: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  let l_s1: f32 = p_s1;
  let l_s2: f32 = p_s2;
  let l_octaves: f32 = p_octaves;
  var l_n: f32;
  l_n = coax_p_metaverse_style_noise((l_x * l_s1), (l_z * l_s1));
  if ((l_octaves < 0x1.800000p0f))
  {
    return l_n;
  }
  return ((l_n * 0x1.4cccccp-1f) + (coax_p_metaverse_style_noise(((l_x * l_s2) + 0x1.8cccccp1f), ((l_z * l_s2) + 0x1.ecccccp2f)) * 0x1.666666p-2f));
}
fn coax_p_metaverse_style_cloud_cover(p_n: f32, p_cover: f32, p_cut_lo: f32, p_cut_hi: f32) -> f32 {
  let l_n: f32 = p_n;
  let l_cover: f32 = p_cover;
  let l_cut_lo: f32 = p_cut_lo;
  let l_cut_hi: f32 = p_cut_hi;
  return (coax_p_metaverse_style_soft_step(((l_cut_lo + l_cut_hi) * 0x1.000000p-1f), ((l_cut_hi - l_cut_lo) * 0x1.000000p-1f), (l_n + ((l_cover - 0x1.666666p-2f) * 0x1.99999ap-2f))) * coax_p_metaverse_style_soft_step(0x1.99999ap-6f, 0x1.99999ap-6f, l_cover));
}
fn coax_p_metaverse_style_cloud_fade(p_up: f32) -> f32 {
  let l_up: f32 = p_up;
  return (coax_p_metaverse_style_soft_step(0x1.5c28f6p-4f, 0x1.333334p-4f, l_up) * 0x1.d70a3ep-1f);
}
fn coax_p_metaverse_style_cloud_lit(p_n: f32, p_toward: f32) -> f32 {
  let l_n: f32 = p_n;
  let l_toward: f32 = p_toward;
  return coax_p_st_sat((((l_n - l_toward) * 0x1.400000p2f) + 0x1.19999ap-1f));
}
fn coax_p_metaverse_style_hash(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return coax_p_render_noise2(l_x, l_y);
}
fn coax_p_st_sat(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_st_min(0x1.000000p0f, coax_p_st_max(0.0f, l_x));
}
fn coax_p_metaverse_style_soft_step(p_e: f32, p_s: f32, p_x: f32) -> f32 {
  let l_e: f32 = p_e;
  let l_s: f32 = p_s;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_st_sat(((l_x - (l_e - l_s)) / coax_p_st_max((0x1.000000p1f * l_s), 0x1.0c6f7ap-20f)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_st_max(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  if ((l_a > l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_render_filter_layer8_grad(p_uv: vec2f, p_layer: i32, p_dx: vec2f, p_dy: vec2f, p_bias: f32) -> coax_t34 {
  let l_uv: vec2f = p_uv;
  let l_layer: i32 = p_layer;
  let l_dx: vec2f = p_dx;
  let l_dy: vec2f = p_dy;
  let l_bias: f32 = p_bias;
  var l_rs_f: coax_t36;
  var l_rs_sum: vec4f;
  var l_rs_i: i32;
  var l_rs_offset: f32;
  var l_rs_at: vec2f;
  l_rs_f = coax_p_render_footprint(i32(textureDimensions(sheets).x), i32(textureDimensions(sheets).y), l_dx.x, l_dx.y, l_dy.x, l_dy.y, l_bias);
  if ((!l_rs_f.x))
  {
    return coax_t34(false, vec4f());
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
  return coax_t34(true, coax_p_scale4(l_rs_sum, (0x1.000000p0f / f32(l_rs_f.y))));
}
fn coax_p_wg_exp2(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return exp((l_x * 0x1.62e430p-1f));
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
fn coax_p_e2_field_pick(p_x: u32, p_z: u32, p_k: u32) -> u32 {
  let l_x: u32 = p_x;
  let l_z: u32 = p_z;
  let l_k: u32 = p_k;
  return (coax_p_e2_field_hash(l_x, l_z, (l_k + 10u)) & 3u);
}
fn coax_p_e2_ihash(p_v: u32) -> u32 {
  let l_v: u32 = p_v;
  var l_x: u32;
  l_x = (l_v ^ (l_v >> u32(16u)));
  l_x = (l_x * 2146121005u);
  l_x = (l_x ^ (l_x >> u32(15u)));
  l_x = (l_x * 1663597195u);
  return (l_x ^ (l_x >> u32(16u)));
}
fn coax_p_e2_field_fine_off(p_c: f32) -> bool {
  let l_c: f32 = p_c;
  return (l_c <= 0x1.001a36p-2f);
}
fn coax_p_e2_noise(p_x: f32, p_z: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
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
fn coax_p_e2_smooth(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = ((l_x - l_a) / (l_b - l_a));
  l_t = select(select(l_t, 0x1.000000p0f, (l_t > 0x1.000000p0f)), 0.0f, (l_t < 0.0f));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_exp_ground_sites() -> array<f32, 192> {
  return array<f32, 192>((-0x1.200000p4f), (-0x1.700000p4f), 0x1.400000p3f, 0x1.000000p2f, 0x1.59999ap3f, 0x1.066666p2f, 0x1.200000p4f, (-0x1.400000p4f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, (-0x1.900000p5f), 0x1.900000p4f, 0x1.733334p1f, 0x1.600000p4f, 0x1.c00000p2f, 0x1.8cccccp2f, 0x1.180000p5f, 0x1.e00000p4f, 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, (-0x1.400000p4f), 0x1.900000p4f, 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, (-0x1.680000p5f), 0x1.600000p5f, 0x1.400000p3f, 0x1.000000p2f, 0x1.e00000p2f, 0x1.0cccccp2f, (-0x1.300000p4f), 0x1.680000p5f, 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, (-0x1.400000p2f), (-0x1.700000p4f), 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, 0x1.e00000p3f, (-0x1.400000p2f), 0x1.733334p1f, 0x1.8cccccp2f, 0x1.c00000p2f, 0x1.4cccccp2f, 0x1.800000p5f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.d00000p5f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.100000p6f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.380000p6f, 0x1.580000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.800000p5f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.d00000p5f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.100000p6f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.380000p6f, 0x1.a80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.800000p5f, 0x1.f80000p5f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, (-0x1.300000p5f), (-0x1.b00000p5f), 0x1.51999ap3f, 0x1.6c6666p5f, 0x1.923334p6f, 0x1.51999ap3f, 0x1.900000p5f, (-0x1.400000p4f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.200000p4f, (-0x1.a00000p5f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.900000p5f, (-0x1.a00000p5f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.280000p6f, (-0x1.400000p4f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.280000p6f, (-0x1.a00000p5f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.200000p4f, (-0x1.280000p6f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.900000p5f, (-0x1.280000p6f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.280000p6f, (-0x1.280000p6f), 0x1.0cccccp2f, 0x1.e66666p1f, 0x1.066666p2f, 0x1.f33334p1f, 0x1.200000p5f, (-0x1.100000p5f), 0x1.400000p3f, 0x1.000000p2f, 0x1.59999ap3f, 0x1.066666p2f, (-0x1.400000p4f), (-0x1.400000p2f), 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.400000p1f, 0x1.a451ecp4f, 0x1.700000p5f, 0x1.e570a4p3f, 0x1.e570a4p3f, 0x1.a66666p2f, 0x1.a66666p2f, 0x1.5947aep3f, 0x1.20999ap6f, 0x1.eb0a3ep4f, 0x1.eb0a3ep4f, 0x1.333334p3f, 0x1.333334p3f, 0x1.df999ap5f, 0x1.3a0000p6f, 0x1.c9999ap3f, 0x1.c9999ap3f, 0x1.d9999ap1f, 0x1.a00000p1f);
}
fn coax_p_cg_height(p_x: f32, p_z: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
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
  let l_h: f32 = p_h;
  let l_sx: f32 = p_sx;
  let l_sz: f32 = p_sz;
  let l_left: f32 = p_left;
  let l_right: f32 = p_right;
  let l_back: f32 = p_back;
  let l_front: f32 = p_front;
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
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
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  return coax_p_exp_ground_distance(0x1.700000p5f, (-0x1.780000p5f), 0x1.040000p5f, 0x1.040000p5f, 0x1.f80000p4f, 0x1.f00000p4f, l_x, l_z);
}
fn coax_p_e2_meadow_share(p_kit: i32) -> f32 {
  let l_kit: i32 = p_kit;
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
  let l_kit: i32 = p_kit;
  return select(select(select(0x1.400000p6f, 0x1.900000p6f, ((l_kit >= 25i) && (l_kit <= 27i))), 0x1.180000p7f, ((l_kit == 29i) || (l_kit == 30i))), 0x1.400000p8f, ((l_kit >= 20i) && (l_kit <= 22i)));
}
fn coax_p_cg_ramp(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = ((l_x - l_a) / (l_b - l_a));
  l_t = select(select(l_t, 0x1.000000p0f, (l_t > 0x1.000000p0f)), 0.0f, (l_t < 0.0f));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_fivelab_upper(p_j: u32) -> f32 {
  let l_j: u32 = p_j;
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
fn coax_p_fivelab_arm(p_row: u32, p_j: u32, p_m: coax_t33) -> coax_t33 {
  let l_row: u32 = p_row;
  let l_j: u32 = p_j;
  let l_m: coax_t33 = p_m;
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
  var l_r1: coax_t33;
  var l_t1: coax_t33;
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
  l_r1 = coax_t33();
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
fn coax_p_env_block_w3(p_e: coax_t25, p_k: i32) -> vec3f {
  let l_e: coax_t25 = p_e;
  let l_k: i32 = p_k;
  return vec3f(coax_p_env_block_w(l_e, l_k), coax_p_env_block_w(l_e, (l_k + 1i)), coax_p_env_block_w(l_e, (l_k + 2i)));
}
fn coax_p_env_block_w(p_e: coax_t25, p_k: i32) -> f32 {
  let l_e: coax_t25 = p_e;
  let l_k: i32 = p_k;
  return coax_p_v4_lane(l_e.x[(l_k / 4i)], (l_k % 4i));
}
fn coax_p_render_noise2(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  if (((!coax_p_render_noise_coordinate_ok(l_x)) || (!coax_p_render_noise_coordinate_ok(l_y))))
  {
    return 0.0f;
  }
  return coax_p_render_noise_unit(coax_p_render_noise_mix2(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y)));
}
fn coax_p_st_min(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  if ((l_a < l_b))
  {
    return l_a;
  }
  return l_b;
}
fn coax_p_render_footprint(p_width: i32, p_height: i32, p_du_dx: f32, p_dv_dx: f32, p_du_dy: f32, p_dv_dy: f32, p_bias: f32) -> coax_t36 {
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
    return coax_t36(false, 0i, 0.0f, 0.0f, 0.0f);
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
  return coax_t36(true, l_taps, (l_du / f32(l_taps)), (l_dv / f32(l_taps)), ((log(l_span) / log(0x1.000000p1f)) + l_bias));
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
fn coax_p_cg_road_ground(p_x: f32, p_z: f32, p_h: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  let l_h: f32 = p_h;
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
  let l_sx: f32 = p_sx;
  let l_sz: f32 = p_sz;
  let l_left: f32 = p_left;
  let l_right: f32 = p_right;
  let l_back: f32 = p_back;
  let l_front: f32 = p_front;
  let l_x: f32 = p_x;
  let l_z: f32 = p_z;
  var l_dx: f32;
  var l_dz: f32;
  l_dx = coax_p_exp_ground_max(((l_sx - l_left) - l_x), ((l_x - l_sx) - l_right));
  l_dz = coax_p_exp_ground_max(((l_sz - l_back) - l_z), ((l_z - l_sz) - l_front));
  return coax_p_exp_ground_max(l_dx, l_dz);
}
fn coax_p_fivelab_arm_frame(p_row: u32) -> bool {
  let l_row: u32 = p_row;
  var l_f: u32;
  l_f = ((l_row / 195u) % 4137u);
  return ((l_f < 143u) || ((l_f >= 389u) && (l_f < 464u)));
}
fn coax_p_fivelab_joint(p_row: u32, p_j: u32) -> coax_t33 {
  let l_row: u32 = p_row;
  let l_j: u32 = p_j;
  var l_r: u32;
  var l_o: coax_t33;
  l_r = (l_row + (l_j * 3u));
  l_o = coax_t33();
  l_o.x = bake[i32(l_r)];
  l_o.y = bake[i32((l_r + 1u))];
  l_o.z = bake[i32((l_r + 2u))];
  return l_o;
}
fn coax_p_fivelab_at(p_m: coax_t33, p_q: vec4f) -> vec4f {
  let l_m: coax_t33 = p_m;
  let l_q: vec4f = p_q;
  return coax_p_v4(coax_p_wg_dot4(l_m.x, l_q), coax_p_wg_dot4(l_m.y, l_q), coax_p_wg_dot4(l_m.z, l_q), 0x1.000000p0f);
}
fn coax_p_fivelab_turn_at(p_r: coax_t33, p_p: vec4f) -> coax_t33 {
  let l_r: coax_t33 = p_r;
  let l_p: vec4f = p_p;
  var l_o: coax_t33;
  l_o = coax_t33();
  l_o.x = coax_p_v4(l_r.x.x, l_r.x.y, l_r.x.z, (l_p.x - (((l_r.x.x * l_p.x) + (l_r.x.y * l_p.y)) + (l_r.x.z * l_p.z))));
  l_o.y = coax_p_v4(l_r.y.x, l_r.y.y, l_r.y.z, (l_p.y - (((l_r.y.x * l_p.x) + (l_r.y.y * l_p.y)) + (l_r.y.z * l_p.z))));
  l_o.z = coax_p_v4(l_r.z.x, l_r.z.y, l_r.z.z, (l_p.z - (((l_r.z.x * l_p.x) + (l_r.z.y * l_p.y)) + (l_r.z.z * l_p.z))));
  return l_o;
}
fn coax_p_fivelab_aff(p_t: coax_t33, p_m: coax_t33) -> coax_t33 {
  let l_t: coax_t33 = p_t;
  let l_m: coax_t33 = p_m;
  var l_o: coax_t33;
  l_o = coax_t33();
  l_o.x = coax_p_fivelab_lin(l_t.x.x, l_t.x.y, l_t.x.z, l_t.x.w, l_m);
  l_o.y = coax_p_fivelab_lin(l_t.y.x, l_t.y.y, l_t.y.z, l_t.y.w, l_m);
  l_o.z = coax_p_fivelab_lin(l_t.z.x, l_t.z.y, l_t.z.z, l_t.z.w, l_m);
  return l_o;
}
fn coax_p_fivelab_elbow_open(p_p: vec4f, p_e: vec4f, p_w: vec4f, p_k: f32) -> coax_t33 {
  let l_p: vec4f = p_p;
  let l_e: vec4f = p_e;
  let l_w: vec4f = p_w;
  let l_k: f32 = p_k;
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
  var l_o: coax_t33;
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
  l_o = coax_t33();
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
fn coax_p_v4_lane(p_v: vec4f, p_i: i32) -> f32 {
  let l_v: vec4f = p_v;
  let l_i: i32 = p_i;
  return select(select(select(l_v.w, l_v.z, (l_i == 2i)), l_v.y, (l_i == 1i)), l_v.x, (l_i == 0i));
}
fn coax_p_render_noise_coordinate_ok(p_x: f32) -> bool {
  let l_x: f32 = p_x;
  return ((l_x >= (-0x1.fffffep127f)) && (l_x <= 0x1.fffffep127f));
}
fn coax_p_render_noise_cell(p_x: f32) -> u32 {
  let l_x: f32 = p_x;
  var l_bounded: f32;
  l_bounded = coax_p_fmin(coax_p_fmax(l_x, (-0x1.000000p25f)), 0x1.fffffep24f);
  return u32(i32(floor((l_bounded * 0x1.000000p6f))));
}
fn coax_p_render_noise_mix2(p_x: u32, p_y: u32) -> u32 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  return coax_p_render_noise_permute(((l_x * 1597334677u) ^ (l_y * 3812015801u)));
}
fn coax_p_render_noise_unit(p_word: u32) -> f32 {
  let l_word: u32 = p_word;
  return (f32((l_word >> u32(8u))) * 0x1.000000p-24f);
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
fn coax_p_cg_road_x(p_z: f32) -> f32 {
  let l_z: f32 = p_z;
  var l_b: f32;
  l_b = coax_p_cg_ramp((-0x1.a00000p3f), (-0x1.300000p4f), l_z);
  return ((coax_p_cg_path_x(l_z) * (0x1.000000p0f - l_b)) + ((0x1.a66666p1f + (0x1.333334p-2f * sin(((l_z + 0x1.500000p4f) * 0x1.000000p-2f)))) * l_b));
}
fn coax_p_exp_ground_max(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_b, l_a, (l_a > l_b));
}
fn coax_p_fivelab_lin(p_x: f32, p_y: f32, p_z: f32, p_w: f32, p_m: coax_t33) -> vec4f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  let l_w: f32 = p_w;
  let l_m: coax_t33 = p_m;
  return coax_p_v4((((l_x * l_m.x.x) + (l_y * l_m.y.x)) + (l_z * l_m.z.x)), (((l_x * l_m.x.y) + (l_y * l_m.y.y)) + (l_z * l_m.z.y)), (((l_x * l_m.x.z) + (l_y * l_m.y.z)) + (l_z * l_m.z.z)), ((((l_x * l_m.x.w) + (l_y * l_m.y.w)) + (l_z * l_m.z.w)) + l_w));
}
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  let l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
}
fn coax_p_cg_path_x(p_z: f32) -> f32 {
  let l_z: f32 = p_z;
  return (((0x1.0cccccp1f * sin((l_z * 0x1.147ae2p-2f))) + 0x1.ccccccp-1f) - select(0.0f, (l_z * 0x1.19999ap-1f), (l_z > 0.0f)));
}
@vertex
fn bg_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> @builtin(position) vec4f {
  let coax_r = coax_p_bg_vs(coax_vid, coax_iid);
  return coax_r;
}
@vertex
fn scene_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_15 {
  let coax_r = coax_p_scene_vs(coax_vid, coax_iid);
  return coax_io_15(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4, coax_r.f5, coax_r.f6, coax_r.f7, coax_r.f8);
}
@vertex
fn fx_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32, coax_in: coax_in_16) -> coax_io_17 {
  let coax_r = coax_p_fx_vs(coax_vid, coax_iid, coax_t16(coax_in.x, coax_in.y, coax_in.z, coax_in.w));
  return coax_io_17(coax_r.x, coax_r.y, coax_r.z, coax_r.w, coax_r.f4);
}
@fragment
fn bg_fs(@builtin(position) coax_pos: vec4f) -> coax_out_18 {
  let coax_r = coax_p_bg_fs(coax_pos);
  return coax_out_18(coax_r.x, coax_r.y);
}
@fragment
fn scene_fs(coax_in: coax_io_15, @builtin(front_facing) coax_front: bool) -> @location(0) vec4f {
  let coax_r = coax_p_scene_fs(coax_t15(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4, coax_in.f5, coax_in.f6, coax_in.f7, coax_in.f8), coax_front);
  return coax_r;
}
@fragment
fn fx_fs(coax_in: coax_io_17) -> @location(0) vec4f {
  let coax_r = coax_p_fx_fs(coax_t17(coax_in.x, coax_in.y, coax_in.z, coax_in.w, coax_in.f4));
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