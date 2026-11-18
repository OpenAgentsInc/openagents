// ---- generated: apps/openagents-web/gpu/slab-fx.coax (the module) by coax wgsl; do not edit
struct coax_t8 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
struct coax_t11 {
  x: vec4f,
  y: vec2f,
  z: vec4f,
}
struct coax_io_11 {
  @builtin(position) x: vec4f,
  @location(0) y: vec2f,
  @location(1) @interpolate(flat) z: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 4050>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(1) @binding(0) var<storage, read> rows: array<coax_t8>;
fn coax_p_particle_vs(p_v: u32, p_n: u32) -> coax_t11 {
  let l_v: u32 = p_v;
  let l_n: u32 = p_n;
  return coax_p_sfx_gone();
}
fn coax_p_particle_over_vs(p_v: u32, p_n: u32) -> coax_t11 {
  let l_v: u32 = p_v;
  let l_n: u32 = p_n;
  var l_per: u32;
  var l_burst: u32;
  var l_fi: f32;
  var l_row: u32;
  var l_e: coax_t8;
  var l_age: f32;
  var l_seed: f32;
  var l_fade: f32;
  var l_tau: f32;
  var l_share: f32;
  var l_h2: f32;
  var l_h3: f32;
  var l_h4: f32;
  var l_nw: vec2f;
  var l_tg: vec2f;
  var l_speed: f32;
  var l_sx: f32;
  var l_sz: f32;
  var l_vx: f32;
  var l_vy: f32;
  var l_vz: f32;
  var l_gust: f32;
  var l_airx: f32;
  var l_airy: f32;
  var l_airz: f32;
  var l_px: f32;
  var l_py: f32;
  var l_pz: f32;
  var l_size: f32;
  var l_out: f32;
  var l_quad: array<vec2f, 6>;
  var l_c: vec2f;
  var l_right: vec3f;
  var l_up: vec3f;
  var l_centre: vec3f;
  var l_slow: f32;
  var l_fly: vec3f;
  var l_fx: f32;
  var l_fy: f32;
  var l_al: f32;
  var l_along: vec2f;
  var l_long: f32;
  var l_ox: f32;
  var l_oy: f32;
  var l_w: vec3f;
  var l_rel: vec3f;
  var l_depth: f32;
  var l_aspect: f32;
  var l_o: coax_t11;
  var l_toc: vec3f;
  var l_dist: f32;
  var l_d: vec3f;
  var l_sun: vec3f;
  var l_shade: f32;
  var l_q: vec2f;
  var l_sd: vec2f;
  var l_ix: f32;
  var l_iy: f32;
  var l_iz: f32;
  var l_ax: f32;
  var l_bx: f32;
  var l_ay: f32;
  var l_by: f32;
  var l_az: f32;
  var l_bz: f32;
  var l_near: f32;
  var l_far: f32;
  var l_thru: f32;
  var l_amb: f32;
  var l_lit: vec3f;
  var l_close: f32;
  l_per = coax_p_wg_u32_f(coax_at_u(i32(15983i)));
  if ((((coax_at_u(i32(15980i)) < 0x1.000000p-1f) || (l_per < 1u)) || (coax_at_u(i32(9i)) < 0.0f)))
  {
    return coax_p_sfx_gone();
  }
  l_burst = (l_n / l_per);
  l_fi = f32((l_n % l_per));
  l_row = (((coax_p_wg_u32_f(coax_at_u(i32(16019i))) + ((256u * 2u) - 1u)) - l_burst) % 256u);
  l_e = rows[i32(l_row)];
  l_age = (coax_at_u(i32(15982i)) - l_e.x.w);
  l_seed = l_e.w.z;
  l_fade = (coax_p_spray_fade(l_fi, l_age) * l_e.w.w);
  if ((l_fade <= 0x1.0624dep-8f))
  {
    return coax_p_sfx_gone();
  }
  l_tau = coax_p_spray_tau(l_seed, l_fi);
  l_share = coax_p_spray_share(l_seed, l_fi);
  l_h2 = coax_p_spray_hash(l_seed, l_fi, 0x1.000000p1f);
  l_h3 = coax_p_spray_hash(l_seed, l_fi, 0x1.800000p1f);
  l_h4 = coax_p_spray_hash(l_seed, l_fi, 0x1.000000p2f);
  l_nw = coax_p_v2(l_e.w.x, l_e.w.y);
  l_tg = coax_p_v2((-l_nw.y), l_nw.x);
  l_speed = coax_p_wg_length3(coax_p_v3(l_e.y.x, l_e.y.y, l_e.y.z));
  l_sx = ((l_e.x.x + ((l_tg.x * (l_h2 - 0x1.000000p-1f)) * l_e.y.w)) + (l_nw.x * (0x1.99999ap-2f + (0x1.666666p0f * l_h3))));
  l_sz = ((l_e.x.z + ((l_tg.y * (l_h2 - 0x1.000000p-1f)) * l_e.y.w)) + (l_nw.y * (0x1.99999ap-2f + (0x1.666666p0f * l_h3))));
  l_vx = (((l_e.y.x * l_share) + (((l_tg.x * (l_h4 - 0x1.000000p-1f)) * 0x1.333334p-2f) * l_speed)) + (((l_nw.x * l_h3) * 0x1.eb851ep-4f) * l_speed));
  l_vy = (l_e.y.y * l_share);
  l_vz = (((l_e.y.z * l_share) + (((l_tg.y * (l_h4 - 0x1.000000p-1f)) * 0x1.333334p-2f) * l_speed)) + (((l_nw.y * l_h3) * 0x1.eb851ep-4f) * l_speed));
  l_gust = (0x1.51eb86p-2f * coax_p_wg_length3(coax_p_v3(l_e.z.x, l_e.z.y, l_e.z.z)));
  l_airx = (l_e.z.x + ((l_gust * (coax_p_spray_hash(l_seed, l_fi, 0x1.400000p2f) - 0x1.000000p-1f)) * 0x1.000000p1f));
  l_airy = ((l_e.z.y + (l_gust * (coax_p_spray_hash(l_seed, l_fi, 0x1.800000p2f) - 0x1.000000p-1f))) - (0x1.39eb86p3f * l_tau));
  l_airz = (l_e.z.z + ((l_gust * (coax_p_spray_hash(l_seed, l_fi, 0x1.c00000p2f) - 0x1.000000p-1f)) * 0x1.000000p1f));
  l_px = (l_sx + coax_p_spray_travel(l_vx, l_airx, l_tau, l_age));
  l_py = (l_e.x.y + coax_p_spray_travel(l_vy, l_airy, l_tau, l_age));
  l_pz = (l_sz + coax_p_spray_travel(l_vz, l_airz, l_tau, l_age));
  if ((l_py < (coax_at_u(i32(28i)) - 0x1.000000p-1f)))
  {
    return coax_p_sfx_gone();
  }
  l_size = coax_p_spray_radius(l_fi, l_e.z.w, l_age);
  l_out = (((l_px - l_e.x.x) * l_nw.x) + ((l_pz - l_e.x.z) * l_nw.y));
  if (((l_py < coax_at_u(i32(38i))) && (l_out < l_size)))
  {
    l_px = (l_px + (l_nw.x * (l_size - l_out)));
    l_pz = (l_pz + (l_nw.y * (l_size - l_out)));
  }
  l_quad = array<vec2f, 6>(coax_p_v2((-0x1.000000p0f), (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2((-0x1.000000p0f), 0x1.000000p0f), coax_p_v2(0x1.000000p0f, (-0x1.000000p0f)), coax_p_v2(0x1.000000p0f, 0x1.000000p0f));
  l_c = l_quad[i32(l_v)];
  l_right = coax_p_v3(coax_at_u(i32(16i)), coax_at_u(i32(17i)), coax_at_u(i32(18i)));
  l_up = coax_p_v3(coax_at_u(i32(20i)), coax_at_u(i32(21i)), coax_at_u(i32(22i)));
  l_centre = coax_p_v3(l_px, l_py, l_pz);
  l_slow = exp(((-l_age) / l_tau));
  l_fly = coax_p_v3((l_airx + ((l_vx - l_airx) * l_slow)), (l_airy + ((l_vy - l_airy) * l_slow)), (l_airz + ((l_vz - l_airz) * l_slow)));
  l_fx = coax_p_wg_dot3(l_fly, l_right);
  l_fy = coax_p_wg_dot3(l_fly, l_up);
  l_al = sqrt(((l_fx * l_fx) + (l_fy * l_fy)));
  l_along = select(coax_p_v2(0.0f, 0x1.000000p0f), coax_p_v2((l_fx / l_al), (l_fy / l_al)), (l_al > 0x1.000000p-1f));
  l_long = select((0x1.000000p0f + coax_p_fmin((l_al * 0x1.eb851ep-4f), 0x1.99999ap0f)), 0x1.000000p0f, coax_p_spray_mist(l_fi));
  l_ox = (((-l_along.y) * l_c.x) + ((l_along.x * l_c.y) * l_long));
  l_oy = ((l_along.x * l_c.x) + ((l_along.y * l_c.y) * l_long));
  l_w = coax_p_sb_add(l_centre, coax_p_sb_add(coax_p_sb_mul(l_right, (l_ox * l_size)), coax_p_sb_mul(l_up, (l_oy * l_size))));
  l_rel = coax_p_sb_sub(l_w, coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_depth = coax_p_wg_dot3(l_rel, coax_p_v3(coax_at_u(i32(12i)), coax_at_u(i32(13i)), coax_at_u(i32(14i))));
  l_aspect = (coax_at_u(i32(0i)) / coax_at_u(i32(1i)));
  l_o = coax_t11();
  l_o.x = coax_p_v4((coax_p_wg_dot3(l_rel, l_right) / (coax_at_u(i32(11i)) * l_aspect)), (coax_p_wg_dot3(l_rel, l_up) / coax_at_u(i32(11i))), (l_depth - 0x1.000000p-1f), l_depth);
  l_o.y = l_c;
  l_toc = coax_p_sb_sub(l_centre, coax_p_v3(coax_at_u(i32(8i)), coax_at_u(i32(9i)), coax_at_u(i32(10i))));
  l_dist = coax_p_fmax(coax_p_wg_length3(l_toc), 0x1.0624dep-10f);
  l_d = coax_p_sb_mul(l_toc, (0x1.000000p0f / l_dist));
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_shade = 0.0f;
  if ((coax_at_u(i32(15980i)) < 0x1.800000p0f))
  {
    l_q = coax_p_sw_frame(coax_p_v2(l_px, l_pz));
    l_sd = coax_p_sw_turn(coax_p_v2(l_sun.x, l_sun.z));
    l_ix = (0x1.000000p0f / select(0x1.a36e2ep-14f, l_sd.x, (abs(l_sd.x) > 0x1.a36e2ep-14f)));
    l_iy = (0x1.000000p0f / select(0x1.a36e2ep-14f, l_sun.y, (abs(l_sun.y) > 0x1.a36e2ep-14f)));
    l_iz = (0x1.000000p0f / select(0x1.a36e2ep-14f, l_sd.y, (abs(l_sd.y) > 0x1.a36e2ep-14f)));
    l_ax = (((-coax_at_u(i32(16016i))) - l_q.x) * l_ix);
    l_bx = ((coax_at_u(i32(16016i)) - l_q.x) * l_ix);
    l_ay = ((coax_at_u(i32(39i)) - l_py) * l_iy);
    l_by = ((coax_at_u(i32(38i)) - l_py) * l_iy);
    l_az = (((-coax_at_u(i32(16017i))) - l_q.y) * l_iz);
    l_bz = ((coax_at_u(i32(16017i)) - l_q.y) * l_iz);
    l_near = coax_p_fmax(coax_p_fmax(coax_p_fmin(l_ax, l_bx), coax_p_fmin(l_ay, l_by)), coax_p_fmin(l_az, l_bz));
    l_far = coax_p_fmin(coax_p_fmin(coax_p_fmax(l_ax, l_bx), coax_p_fmax(l_ay, l_by)), coax_p_fmax(l_az, l_bz));
    if (((l_far > coax_p_fmax(l_near, 0.0f)) && (coax_at_u(i32(25i)) > 0.0f)))
    {
      l_shade = 0x1.000000p0f;
    }
  }
  l_thru = coax_p_sb_pow(coax_p_fmax(coax_p_wg_dot3(l_d, l_sun), 0.0f), 0x1.800000p1f);
  l_amb = coax_at_u(i32(1447i));
  l_lit = coax_p_sb_add(coax_p_sb_mul(coax_p_sb_add(coax_p_v3(0x1.000000p-1f, 0x1.0a3d70p-1f, 0x1.147ae2p-1f), coax_p_sb_mul(coax_p_sb_p(12i), 0x1.666666p-2f)), (0x1.333334p-2f + (0x1.333334p-1f * l_amb))), coax_p_sb_mul(coax_p_sb_p(4i), ((0x1.99999ap-2f + (0x1.19999ap-1f * l_thru)) * (0x1.000000p0f - l_shade))));
  l_lit = coax_p_slab_grade(coax_p_slab_haze(l_lit, l_d, l_dist));
  l_close = coax_p_smoothstep((l_size * 0x1.99999ap-1f), ((l_size * 0x1.400000p1f) + 0x1.000000p1f), l_dist);
  l_o.z = coax_p_v4(l_lit.x, l_lit.y, l_lit.z, ((l_fade * l_close) * select(0x1.ccccccp-1f, 0x1.ccccccp-2f, coax_p_spray_mist(l_fi))));
  return l_o;
}
fn coax_p_particle_fs(p_p: coax_t11) -> vec4f {
  let l_p: coax_t11 = p_p;
  return coax_p_v4(0.0f, 0.0f, 0.0f, 0.0f);
}
fn coax_p_particle_over_fs(p_p: coax_t11) -> vec4f {
  let l_p: coax_t11 = p_p;
  var l_r: f32;
  var l_soft: f32;
  var l_a: f32;
  l_r = coax_p_wg_length2(l_p.y);
  l_soft = (0x1.000000p0f - coax_p_smoothstep(0x1.333334p-3f, 0x1.000000p0f, l_r));
  l_a = ((l_p.z.w * l_soft) * l_soft);
  return coax_p_v4((l_p.z.x * l_a), (l_p.z.y * l_a), (l_p.z.z * l_a), l_a);
}
fn coax_p_sfx_gone() -> coax_t11 {
  var l_o: coax_t11;
  l_o = coax_t11();
  l_o.x = coax_p_v4(0.0f, 0.0f, 0x1.000000p1f, 0x1.000000p0f);
  return l_o;
}
fn coax_p_wg_u32_f(p_x: f32) -> u32 {
  let l_x: f32 = p_x;
  return select(0u, u32(coax_p_fmin(l_x, 0x1.fffffep31f)), (l_x > 0.0f));
}
fn coax_p_spray_fade(p_i: f32, p_age: f32) -> f32 {
  let l_i: f32 = p_i;
  let l_age: f32 = p_age;
  var l_life: f32;
  var l_a: f32;
  var l_rise: f32;
  if (((l_age < 0.0f) || (l_age >= 0x1.200000p3f)))
  {
    return 0.0f;
  }
  l_life = select((0x1.333334p-1f * 0x1.200000p3f), 0x1.200000p3f, coax_p_spray_mist(l_i));
  l_a = (l_age / l_life);
  if ((l_a >= 0x1.000000p0f))
  {
    return 0.0f;
  }
  l_rise = select(0x1.000000p0f, (l_age / 0x1.eb851ep-4f), (l_age < 0x1.eb851ep-4f));
  return ((l_rise * (0x1.000000p0f - l_a)) * (0x1.000000p0f - l_a));
}
fn coax_p_spray_tau(p_seed: f32, p_i: f32) -> f32 {
  let l_seed: f32 = p_seed;
  let l_i: f32 = p_i;
  var l_h: f32;
  l_h = coax_p_spray_hash(l_seed, l_i, 0.0f);
  if (coax_p_spray_mist(l_i))
  {
    return (0x1.333334p-3f + (0x1.000000p-2f * l_h));
  }
  return (0x1.ccccccp-1f + (0x1.99999ap0f * l_h));
}
fn coax_p_spray_share(p_seed: f32, p_i: f32) -> f32 {
  let l_seed: f32 = p_seed;
  let l_i: f32 = p_i;
  var l_h: f32;
  l_h = coax_p_spray_hash(l_seed, l_i, 0x1.000000p0f);
  return (0x1.ccccccp-2f + (0x1.19999ap-1f * sqrt(l_h)));
}
fn coax_p_spray_hash(p_seed: f32, p_i: f32, p_k: f32) -> f32 {
  let l_seed: f32 = p_seed;
  let l_i: f32 = p_i;
  let l_k: f32 = p_k;
  return coax_p_render_noise3(l_seed, (l_i + 0x1.000000p0f), (l_k + 0x1.000000p0f));
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
fn coax_p_wg_length3(p_a: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_spray_travel(p_v0: f32, p_vt: f32, p_tau: f32, p_t: f32) -> f32 {
  let l_v0: f32 = p_v0;
  let l_vt: f32 = p_vt;
  let l_tau: f32 = p_tau;
  let l_t: f32 = p_t;
  return ((l_vt * l_t) + (((l_v0 - l_vt) * l_tau) * (0x1.000000p0f - exp(((-l_t) / l_tau)))));
}
fn coax_p_spray_radius(p_i: f32, p_size: f32, p_age: f32) -> f32 {
  let l_i: f32 = p_i;
  let l_size: f32 = p_size;
  let l_age: f32 = p_age;
  var l_grow: f32;
  l_grow = select(0x1.ccccccp-2f, 0x1.19999ap0f, coax_p_spray_mist(l_i));
  return (l_size * (0x1.000000p0f + (l_grow * l_age)));
}
fn coax_p_wg_dot3(p_a: vec3f, p_b: vec3f) -> f32 {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return dot(l_a, l_b);
}
fn coax_p_spray_mist(p_i: f32) -> bool {
  let l_i: f32 = p_i;
  return ((l_i - (0x1.800000p1f * floor((l_i / 0x1.800000p1f)))) < 0x1.000000p-1f);
}
fn coax_p_fmin(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b < l_a));
}
fn coax_p_sb_mul(p_a: vec3f, p_b: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: f32 = p_b;
  return coax_p_v3((l_a.x * l_b), (l_a.y * l_b), (l_a.z * l_b));
}
fn coax_p_sb_add(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_sb_sub(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return coax_p_v3((l_a.x - l_b.x), (l_a.y - l_b.y), (l_a.z - l_b.z));
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  let l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_fmax(p_a: f32, p_b: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  return select(l_a, l_b, (l_b > l_a));
}
fn coax_p_sw_frame(p_p: vec2f) -> vec2f {
  let l_p: vec2f = p_p;
  var l_dx: f32;
  var l_dz: f32;
  l_dx = (l_p.x - coax_at_u(i32(15984i)));
  l_dz = (l_p.y - coax_at_u(i32(15985i)));
  return coax_p_v2(((coax_at_u(i32(15986i)) * l_dx) + (coax_at_u(i32(15987i)) * l_dz)), ((coax_at_u(i32(15986i)) * l_dz) - (coax_at_u(i32(15987i)) * l_dx)));
}
fn coax_p_sw_turn(p_d: vec2f) -> vec2f {
  let l_d: vec2f = p_d;
  return coax_p_v2(((coax_at_u(i32(15986i)) * l_d.x) + (coax_at_u(i32(15987i)) * l_d.y)), ((coax_at_u(i32(15986i)) * l_d.y) - (coax_at_u(i32(15987i)) * l_d.x)));
}
fn coax_p_sb_pow(p_x: f32, p_y: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  return coax_pow_particle_vs(coax_p_fmax(l_x, 0x1.0c6f7ap-20f), l_y);
}
fn coax_p_sb_p(p_i: i32) -> vec3f {
  let l_i: i32 = p_i;
  return coax_p_v3(coax_at_u(i32((1440i + l_i))), coax_at_u(i32(((1440i + l_i) + 1i))), coax_at_u(i32(((1440i + l_i) + 2i))));
}
fn coax_p_slab_haze(p_c: vec3f, p_d: vec3f, p_dist: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  var l_a: f32;
  var l_sun: vec3f;
  var l_tint: vec3f;
  if (coax_p_slab_photo())
  {
    return coax_p_slab_photo_haze(l_c, l_d, l_dist);
  }
  l_a = (0x1.000000p0f - exp(((-l_dist) / coax_at_u(i32(1455i)))));
  l_sun = coax_p_v3(coax_at_u(i32(24i)), coax_at_u(i32(25i)), coax_at_u(i32(26i)));
  l_tint = coax_p_sb_mix(coax_p_sb_p(12i), coax_p_sb_add(coax_p_sb_p(12i), coax_p_sb_mul(coax_p_sb_p(4i), 0x1.000000p-2f)), coax_p_sb_pow(coax_p_fmax(coax_p_wg_dot3(l_d, l_sun), 0.0f), 0x1.400000p2f));
  return coax_p_sb_mix(l_c, l_tint, (l_a * 0x1.d70a3ep-1f));
}
fn coax_p_slab_grade(p_c0: vec3f) -> vec3f {
  let l_c0: vec3f = p_c0;
  var l_g: f32;
  var l_lift: f32;
  var l_c: vec3f;
  var l_l: f32;
  var l_sep: vec3f;
  if (coax_p_slab_photo())
  {
    return l_c0;
  }
  l_g = coax_at_u(i32(1483i));
  l_lift = (0x1.000000p0f + (0x1.800000p-1f * l_g));
  l_c = coax_p_v3(coax_p_slab_shoulder((l_c0.x * l_lift)), coax_p_slab_shoulder((l_c0.y * l_lift)), coax_p_slab_shoulder((l_c0.z * l_lift)));
  l_l = (((l_c.x * 0x1.333334p-2f) + (l_c.y * 0x1.2e147ap-1f)) + (l_c.z * 0x1.c28f5cp-4f));
  l_sep = coax_p_v3((l_l * 0x1.147ae2p0f), (l_l * 0x1.e66666p-1f), (l_l * 0x1.8f5c28p-1f));
  return coax_p_sb_mix(l_c, coax_p_sb_add(coax_p_sb_mul(l_c, 0x1.000000p-1f), coax_p_sb_mul(l_sep, 0x1.19999ap-1f)), (l_g * 0x1.800000p-1f));
}
fn coax_p_smoothstep(p_a: f32, p_b: f32, p_x: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_x: f32 = p_x;
  var l_t: f32;
  l_t = coax_p_saturate(((l_x - l_a) / (l_b - l_a)));
  return ((l_t * l_t) * (0x1.800000p1f - (0x1.000000p1f * l_t)));
}
fn coax_p_wg_length2(p_a: vec2f) -> f32 {
  let l_a: vec2f = p_a;
  return sqrt(dot(l_a, l_a));
}
fn coax_p_render_noise3(p_x: f32, p_y: f32, p_z: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  var l_ok: bool;
  l_ok = ((coax_p_render_noise_coordinate_ok(l_x) && coax_p_render_noise_coordinate_ok(l_y)) && coax_p_render_noise_coordinate_ok(l_z));
  return select(0.0f, coax_p_render_noise_unit(coax_p_render_noise_mix3(coax_p_render_noise_cell(l_x), coax_p_render_noise_cell(l_y), coax_p_render_noise_cell(l_z))), l_ok);
}
fn coax_p_slab_photo() -> bool {
  return (coax_at_u(i32(15976i)) > 0x1.000000p-1f);
}
fn coax_p_slab_photo_haze(p_c: vec3f, p_d: vec3f, p_dist: f32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_d: vec3f = p_d;
  let l_dist: f32 = p_dist;
  var l_b: i32;
  var l_t: f32;
  var l_far: vec3f;
  l_b = 15872i;
  l_t = coax_p_pr_air_through(l_dist, coax_at_u(i32(((l_b + 0i) + 1i))), l_d.y, l_b);
  l_far = coax_p_pr_out(coax_p_slab_sea_cloud_far(coax_p_add3(coax_p_pr_air_far(l_d, coax_p_pr_at3((l_b + 16i)), coax_at_u(i32((l_b + 51i)))), coax_p_pr_night(coax_at_u(i32(((l_b + 16i) + 3i)))))), l_b);
  return coax_p_mix3(l_c, l_far, (0x1.000000p0f - l_t));
}
fn coax_p_sb_mix(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_t: f32 = p_t;
  return coax_p_v3(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_slab_shoulder(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  if ((l_x < 0x1.666666p-1f))
  {
    return l_x;
  }
  return (0x1.666666p-1f + (0x1.333334p-2f * (0x1.000000p0f - exp(((-(l_x - 0x1.666666p-1f)) / 0x1.333334p-2f)))));
}
fn coax_p_saturate(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  return coax_p_clampf(l_x, 0.0f, 0x1.000000p0f);
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
fn coax_p_render_noise_mix3(p_x: u32, p_y: u32, p_z: u32) -> u32 {
  let l_x: u32 = p_x;
  let l_y: u32 = p_y;
  let l_z: u32 = p_z;
  return coax_p_render_noise_permute((((l_x * 1597334677u) ^ (l_y * 3812015801u)) ^ (l_z * 2654435761u)));
}
fn coax_p_render_noise_unit(p_word: u32) -> f32 {
  let l_word: u32 = p_word;
  return (f32((l_word >> u32(8u))) * 0x1.000000p-24f);
}
fn coax_p_pr_air_through(p_dist: f32, p_eye_y: f32, p_rd_y: f32, p_b: i32) -> f32 {
  let l_dist: f32 = p_dist;
  let l_eye_y: f32 = p_eye_y;
  let l_rd_y: f32 = p_rd_y;
  let l_b: i32 = p_b;
  var l_h: f32;
  var l_y0: f32;
  var l_k: f32;
  var l_along: f32;
  l_h = coax_p_fmax(coax_at_u(i32(((l_b + 28i) + 1i))), 0x1.000000p0f);
  l_y0 = coax_p_fmax((l_eye_y - coax_at_u(i32(((l_b + 28i) + 2i)))), 0.0f);
  l_k = ((l_rd_y * l_dist) / l_h);
  l_along = select(((l_dist * (0x1.000000p0f - exp((0.0f - l_k)))) / l_k), l_dist, (abs(l_k) < 0x1.0624dep-10f));
  return exp((0.0f - ((coax_at_u(i32((l_b + 28i))) * exp((0.0f - (l_y0 / l_h)))) * l_along)));
}
fn coax_p_pr_at3(p_at: i32) -> vec3f {
  let l_at: i32 = p_at;
  return coax_p_v3(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))));
}
fn coax_p_pr_air_far(p_rd: vec3f, p_sun: vec3f, p_hz: f32) -> vec3f {
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  let l_hz: f32 = p_hz;
  var l_l: f32;
  l_l = sqrt(((l_rd.x * l_rd.x) + (l_rd.z * l_rd.z)));
  if ((l_l < 0x1.a36e2ep-14f))
  {
    return coax_p_pr_air_light(coax_p_v3(0.0f, 0.0f, (0.0f - 0x1.000000p0f)), l_sun, l_hz);
  }
  return coax_p_pr_air_light(coax_p_v3((l_rd.x / l_l), 0.0f, (l_rd.z / l_l)), l_sun, l_hz);
}
fn coax_p_pr_night(p_day: f32) -> vec3f {
  let l_day: f32 = p_day;
  return coax_p_scale3(coax_p_v3(0x1.cac084p-9f, 0x1.54c986p-8f, 0x1.581062p-7f), (0x1.000000p0f - l_day));
}
fn coax_p_add3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z));
}
fn coax_p_slab_sea_cloud_far(p_far: vec3f) -> vec3f {
  let l_far: vec3f = p_far;
  var l_deck: f32;
  l_deck = coax_p_smoothstep(0x1.333334p-2f, 0x1.ccccccp-1f, coax_at_u(i32(15856i)));
  return coax_p_mix3(l_far, coax_p_pr_sky_light(coax_p_v3(0.0f, 0x1.000000p0f, 0.0f), 15872i), l_deck);
}
fn coax_p_pr_out(p_c: vec3f, p_b: i32) -> vec3f {
  let l_c: vec3f = p_c;
  let l_b: i32 = p_b;
  var l_e: f32;
  var l_t: vec3f;
  l_e = coax_at_u(i32(((l_b + 20i) + 3i)));
  l_t = coax_p_film_rec709(coax_p_film_rgb((coax_p_fmax(l_c.x, 0.0f) * l_e), (coax_p_fmax(l_c.y, 0.0f) * l_e), (coax_p_fmax(l_c.z, 0.0f) * l_e)));
  return coax_p_pr_encode(coax_p_v3(coax_p_saturate(l_t.x), coax_p_saturate(l_t.y), coax_p_saturate(l_t.z)));
}
fn coax_p_mix3(p_a: vec3f, p_b: vec3f, p_t: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  let l_t: f32 = p_t;
  return vec3f(coax_p_mix(l_a.x, l_b.x, l_t), coax_p_mix(l_a.y, l_b.y, l_t), coax_p_mix(l_a.z, l_b.z, l_t));
}
fn coax_p_mix(p_a: f32, p_b: f32, p_t: f32) -> f32 {
  let l_a: f32 = p_a;
  let l_b: f32 = p_b;
  let l_t: f32 = p_t;
  return (l_a + ((l_b - l_a) * l_t));
}
fn coax_p_clampf(p_x: f32, p_low: f32, p_high: f32) -> f32 {
  let l_x: f32 = p_x;
  let l_low: f32 = p_low;
  let l_high: f32 = p_high;
  return coax_p_fmin(coax_p_fmax(l_x, l_low), l_high);
}
fn coax_p_render_noise_permute(p_word: u32) -> u32 {
  let l_word: u32 = p_word;
  var l_h: u32;
  l_h = (l_word * 747796405u);
  l_h = ((l_h ^ (l_h >> u32(16u))) * 2246822519u);
  return (l_h ^ (l_h >> u32(13u)));
}
fn coax_p_pr_air_light(p_rd: vec3f, p_sun: vec3f, p_hz: f32) -> vec3f {
  let l_rd: vec3f = p_rd;
  let l_sun: vec3f = p_sun;
  let l_hz: f32 = p_hz;
  var l_up: f32;
  var l_cos_t: f32;
  var l_day: f32;
  var l_extra: vec3f;
  var l_clear: vec3f;
  var l_all: vec3f;
  var l_reach: vec3f;
  var l_mass: f32;
  var l_took: vec3f;
  var l_air: vec3f;
  var l_forward: f32;
  var l_glow: f32;
  var l_hazy: vec3f;
  var l_first: f32;
  var l_once: vec3f;
  var l_low: f32;
  var l_again: vec3f;
  l_up = coax_p_saturate(l_rd.y);
  l_cos_t = dot(l_rd, l_sun);
  l_day = coax_p_atmo_day(l_sun.y);
  l_extra = coax_p_pr_haze_loss(l_hz);
  l_clear = coax_p_v3((0x1.916872p-6f + 0x1.89374cp-7f), (0x1.d3c362p-5f + 0x1.89374cp-7f), (0x1.1eb852p-3f + 0x1.89374cp-7f));
  l_all = coax_p_add3(l_clear, l_extra);
  l_reach = coax_p_atmo_exp3(coax_p_scale3(l_all, (0.0f - (coax_p_atmo_sun_mass(l_sun.y) * coax_p_mix(0x1.000000p0f, 0x1.333334p-2f, sqrt(l_up))))));
  l_mass = coax_p_atmo_mass(l_up);
  l_took = coax_p_v3((0x1.000000p0f - exp((0.0f - (l_all.x * l_mass)))), (0x1.000000p0f - exp((0.0f - (l_all.y * l_mass)))), (0x1.000000p0f - exp((0.0f - (l_all.z * l_mass)))));
  l_air = coax_p_scale3(coax_p_v3((0x1.916872p-6f / l_all.x), (0x1.d3c362p-5f / l_all.y), (0x1.1eb852p-3f / l_all.z)), coax_p_atmo_even(l_cos_t));
  l_forward = coax_p_atmo_forward(l_cos_t);
  l_glow = (0x1.5c28f6p-3f * (l_forward + ((0x1.666666p1f * exp((0.0f - (coax_p_fmax((0x1.000000p0f - l_cos_t), 0.0f) * 0x1.19999ap1f)))) * exp((0.0f - (l_up * 0x1.c00000p2f))))));
  l_hazy = coax_p_scale3(coax_p_v3((l_extra.x / l_all.x), (l_extra.y / l_all.y), (l_extra.z / l_all.z)), (0x1.ccccccp-1f * (0x1.99999ap-2f + (0x1.333334p-1f * l_forward))));
  l_first = coax_p_mix(coax_p_smoothstep((0.0f - 0x1.1eb852p-3f), 0x1.47ae14p-6f, l_sun.y), 0x1.000000p0f, exp((0.0f - (l_up * 0x1.800000p2f))));
  l_once = coax_p_scale3(coax_p_mul3(coax_p_mul3(l_reach, l_took), coax_p_add3(coax_p_add3(l_air, coax_p_v3(l_glow, l_glow, l_glow)), l_hazy)), l_first);
  l_low = (0x1.000000p0f - coax_p_saturate(((l_sun.y * 0x1.400000p2f) + 0x1.333334p-3f)));
  l_again = coax_p_scale3(coax_p_mul3(coax_p_mul3(coax_p_v3(0x1.eb851ep-6f, 0x1.1eb852p-4f, 0x1.ae147ap-3f), l_took), coax_p_v3((l_clear.x / l_all.x), (l_clear.y / l_all.y), (l_clear.z / l_all.z))), coax_p_mix(0x1.000000p0f, (0x1.4cccccp1f * coax_p_mix(0x1.eb851ep-4f, 0x1.000000p0f, coax_p_smoothstep(0.0f, 0x1.ccccccp-2f, l_up))), l_low));
  return coax_p_scale3(coax_p_add3(l_once, l_again), (0x1.8cccccp0f * l_day));
}
fn coax_p_scale3(p_a: vec3f, p_k: f32) -> vec3f {
  let l_a: vec3f = p_a;
  let l_k: f32 = p_k;
  return vec3f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k));
}
fn coax_p_pr_sky_light(p_n: vec3f, p_b: i32) -> vec3f {
  let l_n: vec3f = p_n;
  let l_b: i32 = p_b;
  var l_o: i32;
  var l_c: vec3f;
  l_o = (l_b + 68i);
  l_c = coax_p_pr_at3(l_o);
  l_c = coax_p_add3(l_c, coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 4i)), l_n.y), coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 8i)), l_n.z), coax_p_scale3(coax_p_pr_at3((l_o + 12i)), l_n.x))));
  l_c = coax_p_add3(l_c, coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 16i)), (l_n.x * l_n.y)), coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 20i)), (l_n.y * l_n.z)), coax_p_scale3(coax_p_pr_at3((l_o + 24i)), (((0x1.800000p1f * l_n.z) * l_n.z) - 0x1.000000p0f)))));
  l_c = coax_p_add3(l_c, coax_p_add3(coax_p_scale3(coax_p_pr_at3((l_o + 28i)), (l_n.x * l_n.z)), coax_p_scale3(coax_p_pr_at3((l_o + 32i)), ((l_n.x * l_n.x) - (l_n.y * l_n.y)))));
  return coax_p_v3(coax_p_fmax(l_c.x, 0.0f), coax_p_fmax(l_c.y, 0.0f), coax_p_fmax(l_c.z, 0.0f));
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
fn coax_p_pr_encode(p_c: vec3f) -> vec3f {
  let l_c: vec3f = p_c;
  return coax_p_v3(coax_p_pr_encode1(l_c.x), coax_p_pr_encode1(l_c.y), coax_p_pr_encode1(l_c.z));
}
fn coax_p_atmo_day(p_sun_up: f32) -> f32 {
  let l_sun_up: f32 = p_sun_up;
  var l_t: f32;
  l_t = coax_p_saturate(((l_sun_up - (-0x1.ef74c0p-3f)) / (0.0f - (-0x1.ef74c0p-3f))));
  return ((l_t * l_t) * l_t);
}
fn coax_p_pr_haze_loss(p_hz: f32) -> vec3f {
  let l_hz: f32 = p_hz;
  return coax_p_scale3(coax_p_v3(0x1.bd70a4p-1f, 0x1.000000p0f, 0x1.4cccccp0f), coax_p_fmax(l_hz, 0.0f));
}
fn coax_p_atmo_sun_mass(p_sun_up: f32) -> f32 {
  let l_sun_up: f32 = p_sun_up;
  if ((l_sun_up < 0.0f))
  {
    return (0x1.200000p5f - (l_sun_up * 0x1.2c0000p7f));
  }
  return (0x1.000000p0f / (l_sun_up + 0x1.c71c8ap-6f));
}
fn coax_p_atmo_exp3(p_a: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  return coax_p_v3(exp(l_a.x), exp(l_a.y), exp(l_a.z));
}
fn coax_p_atmo_mass(p_up: f32) -> f32 {
  let l_up: f32 = p_up;
  return (0x1.000000p0f / (coax_p_fmax(l_up, 0.0f) + 0x1.47ae14p-4f));
}
fn coax_p_atmo_even(p_cos_t: f32) -> f32 {
  let l_cos_t: f32 = p_cos_t;
  return ((0x1.000000p-1f * (0x1.000000p0f + (l_cos_t * l_cos_t))) + 0x1.000000p-1f);
}
fn coax_p_atmo_forward(p_cos_t: f32) -> f32 {
  let l_cos_t: f32 = p_cos_t;
  var l_g: f32;
  var l_d: f32;
  l_g = 0x1.851eb8p-1f;
  l_d = coax_p_fmax(((0x1.000000p0f + (l_g * l_g)) - ((0x1.000000p1f * l_g) * l_cos_t)), 0x1.a36e2ep-14f);
  return (((0x1.000000p0f - (l_g * l_g)) / (l_d * sqrt(l_d))) * 0x1.28f5c2p-2f);
}
fn coax_p_mul3(p_a: vec3f, p_b: vec3f) -> vec3f {
  let l_a: vec3f = p_a;
  let l_b: vec3f = p_b;
  return vec3f((l_a.x * l_b.x), (l_a.y * l_b.y), (l_a.z * l_b.z));
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
fn coax_p_pr_encode1(p_x: f32) -> f32 {
  let l_x: f32 = p_x;
  var l_c: f32;
  l_c = coax_p_fmax(l_x, 0.0f);
  return select(((0x1.0e147ap0f * coax_pow_particle_vs(l_c, (0x1.000000p0f / 0x1.333334p1f))) - 0x1.c28f5cp-5f), (l_c * 0x1.9d70a4p3f), (l_c <= 0x1.9a5c38p-9f));
}
@vertex
fn particle_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_11 {
  let coax_r = coax_p_particle_vs(coax_vid, coax_iid);
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z);
}
@vertex
fn particle_over_vs(@builtin(vertex_index) coax_vid: u32, @builtin(instance_index) coax_iid: u32) -> coax_io_11 {
  let coax_r = coax_p_particle_over_vs(coax_vid, coax_iid);
  return coax_io_11(coax_r.x, coax_r.y, coax_r.z);
}
@fragment
fn particle_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_particle_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z));
  return coax_r;
}
@fragment
fn particle_over_fs(coax_in: coax_io_11) -> @location(0) vec4f {
  let coax_r = coax_p_particle_over_fs(coax_t11(coax_in.x, coax_in.y, coax_in.z));
  return coax_r;
}
fn coax_bits_particle_vs(b: u32) -> f32 {
  return bitcast<f32>(b);
}
fn coax_pow_particle_vs(x: f32, y: f32) -> f32 {
  if (y == 0.0f) {
    return 1.0f;
  }
  if (x == 0.0f) {
    return select(coax_bits_particle_vs(0x7f800000u), 0.0f, y > 0.0f);
  }
  let r = pow(abs(x), y);
  if (!(x < 0.0f)) {
    return r;
  }
  if (floor(y) != y) {
    return coax_bits_particle_vs(0x7fc00000u);
  }
  if (fract(y * 0.5f) != 0.0f) {
    return -r;
  }
  return r;
}