// ---- generated: apps/openagents-web/gpu/cull.coax (the module) by coax wgsl; do not edit
struct coax_t11 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 23>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(0) @binding(1) var<storage, read> data: array<vec4f>;
@group(0) @binding(2) var<storage, read> info: array<u32>;
@group(0) @binding(3) var<storage, read> installed: array<u32>;
@group(0) @binding(4) var<storage, read_write> requests: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> draws: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> lists: array<u32>;
@group(0) @binding(7) var<storage, read_write> hiz: array<f32>;
@group(0) @binding(8) var depth: texture_depth_2d;
@group(0) @binding(9) var<uniform> hz: array<vec4f, 1>;
fn coax_at_hz(i: i32) -> f32 {
  return hz[i / 4][i % 4];
}
fn coax_p_cull_one(p_i: u32) {
  let l_i: u32 = p_i;
  var l_c: i32;
  var l_n: i32;
  var l_page: i32;
  var l_low: vec4f;
  var l_high: vec4f;
  var l_k: u32;
  var l_k_15: u32;
  l_c = i32(l_i);
  l_n = i32(coax_at_u(i32(20i)));
  if ((l_c >= l_n))
  {
    return;
  }
  lists[((3i * l_n) + l_c)] = 0u;
  if ((!coax_p_cl_lod(l_c)))
  {
    return;
  }
  _ = atomicAdd(&draws[9i], 1u);
  l_page = i32(info[((l_c * 8i) + 0i)]);
  l_low = data[((l_c * 4i) + 2i)];
  l_high = data[((l_c * 4i) + 3i)];
  if (coax_p_cl_outside(l_low, l_high))
  {
    _ = atomicAdd(&draws[10i], 1u);
    _ = atomicMax(&requests[l_page], 1u);
    lists[((3i * l_n) + l_c)] = 3u;
    return;
  }
  if (((coax_at_u(i32(22i)) > 0x1.000000p-1f) && coax_p_cl_hidden(l_low, l_high)))
  {
    l_k = atomicAdd(&draws[8i], 1u);
    lists[((2i * l_n) + i32(l_k))] = u32(l_c);
    _ = atomicAdd(&draws[11i], 1u);
    _ = atomicMax(&requests[l_page], 1u);
    lists[((3i * l_n) + l_c)] = 5u;
    return;
  }
  l_k_15 = atomicAdd(&draws[1i], 1u);
  lists[((0i * l_n) + i32(l_k_15))] = u32(l_c);
  coax_p_cl_want(l_c);
  coax_p_cl_texture(l_c);
  lists[((3i * l_n) + l_c)] = 9u;
}
fn coax_p_cull_two(p_i: u32) {
  let l_i: u32 = p_i;
  var l_k: i32;
  var l_n: i32;
  var l_c: i32;
  var l_j: u32;
  l_k = i32(l_i);
  l_n = i32(coax_at_u(i32(20i)));
  if ((l_k >= i32(atomicLoad(&draws[8i]))))
  {
    return;
  }
  l_c = i32(lists[((2i * l_n) + l_k)]);
  if (coax_p_cl_hidden(data[((l_c * 4i) + 2i)], data[((l_c * 4i) + 3i)]))
  {
    _ = atomicAdd(&draws[12i], 1u);
    lists[((3i * l_n) + l_c)] = (lists[((3i * l_n) + l_c)] | 32u);
    return;
  }
  l_j = atomicAdd(&draws[5i], 1u);
  lists[((1i * l_n) + i32(l_j))] = u32(l_c);
  coax_p_cl_want(l_c);
  coax_p_cl_texture(l_c);
  lists[((3i * l_n) + l_c)] = (lists[((3i * l_n) + l_c)] | 16u);
}
fn coax_p_hiz_depth(p_i: u32) {
  let l_i: u32 = p_i;
  var l_k: i32;
  var l_w0: i32;
  var l_h0: i32;
  var l_x: i32;
  var l_y: i32;
  var l_vw: i32;
  var l_vh: i32;
  var l_far: f32;
  var l_dy: i32;
  var l_dx: i32;
  l_k = i32(l_i);
  l_w0 = i32(coax_at_u(i32(26i)));
  l_h0 = i32(coax_at_u(i32(27i)));
  if ((l_k >= (l_w0 * l_h0)))
  {
    return;
  }
  l_x = (l_k % l_w0);
  l_y = (l_k / l_w0);
  l_vw = i32(coax_at_u(i32(24i)));
  l_vh = i32(coax_at_u(i32(25i)));
  l_far = 0.0f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 2i;
    for (l_dy = coax_from; l_dy < coax_to; l_dy = l_dy + 1i)
    {
      {
        let coax_from: i32 = 0i;
        let coax_to: i32 = 2i;
        for (l_dx = coax_from; l_dx < coax_to; l_dx = l_dx + 1i)
        {
          l_far = coax_p_fmax(l_far, textureLoad(depth, vec2i(i32(coax_p_cl_clampi(((2i * l_x) + l_dx), 0i, (l_vw - 1i))), i32(coax_p_cl_clampi(((2i * l_y) + l_dy), 0i, (l_vh - 1i)))), 0i));
        }
      }
    }
  }
  hiz[(i32(coax_at_u(i32(52i))) + l_k)] = l_far;
}
fn coax_p_hiz_down(p_i: u32) {
  let l_i: u32 = p_i;
  var l_k: i32;
  var l_l: i32;
  var l_w: i32;
  var l_h: i32;
  var l_x: i32;
  var l_y: i32;
  var l_sw: i32;
  var l_sh: i32;
  var l_from: i32;
  var l_far: f32;
  var l_dy: i32;
  var l_dx: i32;
  l_k = i32(l_i);
  l_l = i32(coax_at_hz(i32(0i)));
  l_w = i32(coax_at_u(i32((64i + l_l))));
  l_h = i32(coax_at_u(i32((76i + l_l))));
  if ((((l_l < 1i) || (l_l >= 12i)) || (l_k >= (l_w * l_h))))
  {
    return;
  }
  l_x = (l_k % l_w);
  l_y = (l_k / l_w);
  l_sw = i32(coax_at_u(i32(((64i + l_l) - 1i))));
  l_sh = i32(coax_at_u(i32(((76i + l_l) - 1i))));
  l_from = i32(coax_at_u(i32(((52i + l_l) - 1i))));
  l_far = 0.0f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 2i;
    for (l_dy = coax_from; l_dy < coax_to; l_dy = l_dy + 1i)
    {
      {
        let coax_from: i32 = 0i;
        let coax_to: i32 = 2i;
        for (l_dx = coax_from; l_dx < coax_to; l_dx = l_dx + 1i)
        {
          l_far = coax_p_fmax(l_far, hiz[((l_from + (coax_p_cl_clampi(((2i * l_y) + l_dy), 0i, (l_sh - 1i)) * l_sw)) + coax_p_cl_clampi(((2i * l_x) + l_dx), 0i, (l_sw - 1i)))]);
        }
      }
    }
  }
  hiz[(i32(coax_at_u(i32((52i + l_l)))) + l_k)] = l_far;
}
fn coax_p_cull_shadow(p_i: u32) {
  let l_i: u32 = p_i;
  var l_c: i32;
  l_c = i32(l_i);
  if ((((l_c >= i32(coax_at_u(i32(20i)))) || (!coax_p_cl_lod(l_c))) || coax_p_cl_outside(data[((l_c * 4i) + 2i)], data[((l_c * 4i) + 3i)])))
  {
    return;
  }
  coax_p_cl_want(l_c);
}
fn coax_p_cl_lod(p_c: i32) -> bool {
  let l_c: i32 = p_c;
  var l_t: f32;
  var l_coarse: bool;
  var l_child: u32;
  if ((installed[i32(info[((l_c * 8i) + 0i)])] == 0u))
  {
    return false;
  }
  l_t = coax_at_u(i32(19i));
  l_coarse = ((info[((l_c * 8i) + 7i)] != 0u) || (coax_p_cl_seen(bitcast<f32>(info[((l_c * 8i) + 5i)]), data[((l_c * 4i) + 1i)]) > l_t));
  if ((!l_coarse))
  {
    return false;
  }
  l_child = info[((l_c * 8i) + 1i)];
  if ((l_child == 4294967295u))
  {
    return true;
  }
  if ((coax_p_cl_seen(bitcast<f32>(info[((l_c * 8i) + 4i)]), data[((l_c * 4i) + 0i)]) <= l_t))
  {
    return true;
  }
  return (installed[i32(l_child)] == 0u);
}
fn coax_p_cl_outside(p_low: vec4f, p_high: vec4f) -> bool {
  let l_low: vec4f = p_low;
  let l_high: vec4f = p_high;
  var l_k: i32;
  var l_at: i32;
  var l_nx: f32;
  var l_ny: f32;
  var l_nz: f32;
  var l_px: f32;
  var l_py: f32;
  var l_pz: f32;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 6i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_at = (28i + (l_k * 4i));
      l_nx = coax_at_u(i32(l_at));
      l_ny = coax_at_u(i32((l_at + 1i)));
      l_nz = coax_at_u(i32((l_at + 2i)));
      l_px = select(l_low.x, l_high.x, (l_nx >= 0.0f));
      l_py = select(l_low.y, l_high.y, (l_ny >= 0.0f));
      l_pz = select(l_low.z, l_high.z, (l_nz >= 0.0f));
      if ((((((l_nx * l_px) + (l_ny * l_py)) + (l_nz * l_pz)) + coax_at_u(i32((l_at + 3i)))) < 0.0f))
      {
        return true;
      }
    }
  }
  return false;
}
fn coax_p_cl_hidden(p_low: vec4f, p_high: vec4f) -> bool {
  let l_low: vec4f = p_low;
  let l_high: vec4f = p_high;
  var l_minx: f32;
  var l_miny: f32;
  var l_maxx: f32;
  var l_maxy: f32;
  var l_near: f32;
  var l_k: i32;
  var l_p: vec4f;
  var l_q: vec4f;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l_w0: i32;
  var l_h0: i32;
  var l_x0: i32;
  var l_x1: i32;
  var l_y0: i32;
  var l_y1: i32;
  var l_level: i32;
  var l_levels: i32;
  var l_w: i32;
  var l_at: i32;
  var l_far: f32;
  var l_y_26: i32;
  var l_x_27: i32;
  l_minx = 0x1.93e594p99f;
  l_miny = 0x1.93e594p99f;
  l_maxx = (-0x1.93e594p99f);
  l_maxy = (-0x1.93e594p99f);
  l_near = 0x1.93e594p99f;
  {
    let coax_from: i32 = 0i;
    let coax_to: i32 = 8i;
    for (l_k = coax_from; l_k < coax_to; l_k = l_k + 1i)
    {
      l_p = coax_p_v4(select(l_low.x, l_high.x, ((l_k & 1i) != 0i)), select(l_low.y, l_high.y, ((l_k & 2i) != 0i)), select(l_low.z, l_high.z, ((l_k & 4i) != 0i)), 0x1.000000p0f);
      l_q = coax_p_m4_apply(coax_p_m4_at(0i), l_p);
      if ((l_q.w <= 0x1.a36e2ep-14f))
      {
        return false;
      }
      l_x = (l_q.x / l_q.w);
      l_y = (l_q.y / l_q.w);
      l_z = (l_q.z / l_q.w);
      l_minx = coax_p_fmin(l_minx, l_x);
      l_maxx = coax_p_fmax(l_maxx, l_x);
      l_miny = coax_p_fmin(l_miny, l_y);
      l_maxy = coax_p_fmax(l_maxy, l_y);
      l_near = coax_p_fmin(l_near, l_z);
    }
  }
  if ((l_near <= 0.0f))
  {
    return false;
  }
  l_w0 = i32(coax_at_u(i32(26i)));
  l_h0 = i32(coax_at_u(i32(27i)));
  l_x0 = coax_p_cl_clampi(i32(floor((((l_minx * 0x1.000000p-1f) + 0x1.000000p-1f) * f32(l_w0)))), 0i, (l_w0 - 1i));
  l_x1 = coax_p_cl_clampi(i32(floor((((l_maxx * 0x1.000000p-1f) + 0x1.000000p-1f) * f32(l_w0)))), 0i, (l_w0 - 1i));
  l_y0 = coax_p_cl_clampi(i32(floor(((0x1.000000p-1f - (l_maxy * 0x1.000000p-1f)) * f32(l_h0)))), 0i, (l_h0 - 1i));
  l_y1 = coax_p_cl_clampi(i32(floor(((0x1.000000p-1f - (l_miny * 0x1.000000p-1f)) * f32(l_h0)))), 0i, (l_h0 - 1i));
  l_level = 0i;
  l_levels = i32(coax_at_u(i32(23i)));
  while (((((l_x1 - l_x0) > 1i) || ((l_y1 - l_y0) > 1i)) && (l_level < (l_levels - 1i))))
  {
    l_level = (l_level + 1i);
    l_x0 = (l_x0 / 2i);
    l_x1 = (l_x1 / 2i);
    l_y0 = (l_y0 / 2i);
    l_y1 = (l_y1 / 2i);
  }
  l_w = i32(coax_at_u(i32((64i + l_level))));
  l_at = i32(coax_at_u(i32((52i + l_level))));
  l_far = 0.0f;
  {
    let coax_from: i32 = l_y0;
    let coax_to: i32 = (l_y1 + 1i);
    for (l_y_26 = coax_from; l_y_26 < coax_to; l_y_26 = l_y_26 + 1i)
    {
      {
        let coax_from: i32 = l_x0;
        let coax_to: i32 = (l_x1 + 1i);
        for (l_x_27 = coax_from; l_x_27 < coax_to; l_x_27 = l_x_27 + 1i)
        {
          l_far = coax_p_fmax(l_far, hiz[((l_at + (l_y_26 * l_w)) + l_x_27)]);
        }
      }
    }
  }
  return (l_near > l_far);
}
fn coax_p_cl_want(p_c: i32) {
  let l_c: i32 = p_c;
  var l_t: f32;
  var l_page: i32;
  var l_child: u32;
  var l_seen: f32;
  l_t = coax_at_u(i32(19i));
  l_page = i32(info[((l_c * 8i) + 0i)]);
  _ = atomicMax(&requests[l_page], coax_p_cl_weigh(coax_p_cl_priority(coax_p_cl_seen(bitcast<f32>(info[((l_c * 8i) + 5i)]), data[((l_c * 4i) + 1i)]), l_t)));
  l_child = info[((l_c * 8i) + 1i)];
  if ((l_child == 4294967295u))
  {
    return;
  }
  l_seen = coax_p_cl_seen(bitcast<f32>(info[((l_c * 8i) + 4i)]), data[((l_c * 4i) + 0i)]);
  if ((l_seen > l_t))
  {
    _ = atomicMax(&requests[i32(l_child)], coax_p_cl_weigh(coax_p_cl_priority(l_seen, l_t)));
  }
}
fn coax_p_cl_texture(p_c: i32) {
  let l_c: i32 = p_c;
  var l_tex: f32;
  l_tex = data[((l_c * 4i) + 3i)].w;
  if (((l_tex < 0.0f) || (coax_at_u(i32(91i)) < 0x1.000000p0f)))
  {
    return;
  }
  _ = atomicMax(&requests[(i32(coax_at_u(i32(21i))) + i32(l_tex))], u32((i32(coax_at_u(i32(91i))) - coax_p_cl_mip(l_c))));
}
fn coax_p_cl_clampi(p_x: i32, p_low: i32, p_high: i32) -> i32 {
  let l_x: i32 = p_x;
  let l_low: i32 = p_low;
  let l_high: i32 = p_high;
  if ((l_x < l_low))
  {
    return l_low;
  }
  if ((l_x > l_high))
  {
    return l_high;
  }
  return l_x;
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
fn coax_p_cl_seen(p_error: f32, p_s: vec4f) -> f32 {
  let l_error: f32 = p_error;
  let l_s: vec4f = p_s;
  var l_dx: f32;
  var l_dy: f32;
  var l_dz: f32;
  var l_d: f32;
  if ((l_error >= 0x1.dcd650p29f))
  {
    return 0x1.93e594p99f;
  }
  if ((coax_at_u(i32(88i)) > 0.0f))
  {
    return (l_error / coax_at_u(i32(88i)));
  }
  l_dx = (coax_at_u(i32(16i)) - l_s.x);
  l_dy = (coax_at_u(i32(17i)) - l_s.y);
  l_dz = (coax_at_u(i32(18i)) - l_s.z);
  l_d = (sqrt((((l_dx * l_dx) + (l_dy * l_dy)) + (l_dz * l_dz))) - l_s.w);
  if ((l_d <= 0x1.0c6f7ap-20f))
  {
    return 0x1.93e594p99f;
  }
  return (l_error / l_d);
}
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  let l_x: f32 = p_x;
  let l_y: f32 = p_y;
  let l_z: f32 = p_z;
  let l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_m4_at(p_at: i32) -> coax_t11 {
  let l_at: i32 = p_at;
  return coax_t11(coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i)))), coax_p_v4(coax_at_u(i32((l_at + 4i))), coax_at_u(i32((l_at + 5i))), coax_at_u(i32((l_at + 6i))), coax_at_u(i32((l_at + 7i)))), coax_p_v4(coax_at_u(i32((l_at + 8i))), coax_at_u(i32((l_at + 9i))), coax_at_u(i32((l_at + 10i))), coax_at_u(i32((l_at + 11i)))), coax_p_v4(coax_at_u(i32((l_at + 12i))), coax_at_u(i32((l_at + 13i))), coax_at_u(i32((l_at + 14i))), coax_at_u(i32((l_at + 15i)))));
}
fn coax_p_m4_apply(p_m: coax_t11, p_v: vec4f) -> vec4f {
  let l_m: coax_t11 = p_m;
  let l_v: vec4f = p_v;
  return coax_p_add4(coax_p_add4(coax_p_scale4(l_m.x, l_v.x), coax_p_scale4(l_m.y, l_v.y)), coax_p_add4(coax_p_scale4(l_m.z, l_v.z), coax_p_scale4(l_m.w, l_v.w)));
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
fn coax_p_cl_priority(p_seen: f32, p_t: f32) -> u32 {
  let l_seen: f32 = p_seen;
  let l_t: f32 = p_t;
  var l_r: f32;
  l_r = ((l_seen / coax_p_fmax(l_t, 0x1.0c6f7ap-20f)) * 0x1.000000p4f);
  if ((l_r > 0x1.e84800p19f))
  {
    l_r = 0x1.e84800p19f;
  }
  return (2u + u32(coax_p_fmax(l_r, 0.0f)));
}
fn coax_p_cl_weigh(p_p: u32) -> u32 {
  let l_p: u32 = p_p;
  return (2u + u32((f32((l_p - 2u)) * coax_at_u(i32(89i)))));
}
fn coax_p_cl_mip(p_c: i32) -> i32 {
  let l_c: i32 = p_c;
  var l_s: vec4f;
  var l_dx: f32;
  var l_dy: f32;
  var l_dz: f32;
  var l_d: f32;
  var l_f: f32;
  var l_mip: i32;
  var l_mips: i32;
  l_s = data[((l_c * 4i) + 0i)];
  l_dx = (coax_at_u(i32(16i)) - l_s.x);
  l_dy = (coax_at_u(i32(17i)) - l_s.y);
  l_dz = (coax_at_u(i32(18i)) - l_s.z);
  l_d = coax_p_fmax((sqrt((((l_dx * l_dx) + (l_dy * l_dy)) + (l_dz * l_dz))) - l_s.w), 0x1.a36e2ep-14f);
  l_f = ((data[((l_c * 4i) + 2i)].w * l_d) * coax_at_u(i32(90i)));
  l_mip = 0i;
  l_mips = i32(coax_at_u(i32(91i)));
  while (((l_f >= 0x1.000000p1f) && (l_mip < (l_mips - 1i))))
  {
    l_f = (l_f * 0x1.000000p-1f);
    l_mip = (l_mip + 1i);
  }
  return l_mip;
}
fn coax_p_scale4(p_a: vec4f, p_k: f32) -> vec4f {
  let l_a: vec4f = p_a;
  let l_k: f32 = p_k;
  return vec4f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k), (l_a.w * l_k));
}
fn coax_p_add4(p_a: vec4f, p_b: vec4f) -> vec4f {
  let l_a: vec4f = p_a;
  let l_b: vec4f = p_b;
  return vec4f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
}
@compute @workgroup_size(64)
fn cull_one(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_cull_one(coax_gid.x);
}
@compute @workgroup_size(64)
fn cull_two(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_cull_two(coax_gid.x);
}
@compute @workgroup_size(64)
fn hiz_depth(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_hiz_depth(coax_gid.x);
}
@compute @workgroup_size(64)
fn hiz_down(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_hiz_down(coax_gid.x);
}
@compute @workgroup_size(64)
fn cull_shadow(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_cull_shadow(coax_gid.x);
}