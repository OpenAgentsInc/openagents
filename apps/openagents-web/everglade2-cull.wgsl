// ---- generated: apps/openagents-web/gpu/everglade2-cull.coax (the module) by coax wgsl; do not edit
struct coax_t10 {
  x: vec4f,
  y: vec4f,
}
struct coax_t12 {
  x: vec4f,
  y: vec4f,
  z: vec4f,
  w: vec4f,
}
@group(0) @binding(0) var<uniform> u: array<vec4f, 25>;
fn coax_at_u(i: i32) -> f32 {
  return u[i / 4][i % 4];
}
@group(0) @binding(10) var<storage, read> buckets: array<u32>;
@group(0) @binding(11) var<storage, read> indices: array<u32>;
@group(0) @binding(12) var<storage, read> vertices: array<u32>;
@group(0) @binding(13) var<storage, read_write> bounds: array<atomic<u32>>;
@group(0) @binding(1) var<storage, read_write> inst: array<vec4f>;
@group(0) @binding(2) var<storage, read> runs: array<u32>;
@group(0) @binding(3) var<storage, read> boxes: array<u32>;
@group(0) @binding(4) var<storage, read_write> args: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> flags: array<u32>;
@group(0) @binding(6) var<storage, read_write> tally: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> hiz: array<f32>;
@group(0) @binding(8) var depth: texture_depth_2d;
@group(0) @binding(9) var<uniform> hz: array<vec4f, 1>;
fn coax_at_hz(i: i32) -> f32 {
  return hz[i / 4][i % 4];
}
fn coax_p_e2q_bounds(p_i: u32) {
  var l_i: u32 = p_i;
  var l_b: i32;
  var l_j: u32;
  var l_first: u32;
  var l_count: u32;
  var l_word: u32;
  var l_w: i32;
  var l_x: f32;
  var l_y: f32;
  var l_z: f32;
  var l__: u32;
  l_b = i32((l_i / 1024u));
  l_j = (l_i - (u32(l_b) * 1024u));
  if ((l_b >= i32(coax_at_u(i32(97i)))))
  {
    return;
  }
  l_first = buckets[(l_b * 2i)];
  l_count = buckets[((l_b * 2i) + 1i)];
  l_word = u32(coax_at_u(i32(96i)));
  while ((l_j < l_count))
  {
    l_w = i32((l_word + (indices[i32((l_first + l_j))] * 8u)));
    l_x = bitcast<f32>(vertices[l_w]);
    l_y = bitcast<f32>(vertices[(l_w + 1i)]);
    l_z = bitcast<f32>(vertices[(l_w + 2i)]);
    l__ = atomicMin(&bounds[((l_b * 4i) + 0i)], coax_p_e2q_low(l_y));
    l__ = atomicMax(&bounds[((l_b * 4i) + 1i)], coax_p_e2q_high(l_y));
    l__ = atomicMax(&bounds[((l_b * 4i) + 2i)], coax_p_e2q_high(sqrt(((l_x * l_x) + (l_z * l_z)))));
    l_j = (l_j + 1024u);
  }
}
fn coax_p_e2q_cull_one(p_t: u32) {
  var l_t: u32 = p_t;
  var l_run: i32;
  var l_k: i32;
  var l_e: coax_t10;
  var l__: u32;
  var l___13: u32;
  if ((i32(l_t) >= i32(coax_at_u(i32(93i)))))
  {
    return;
  }
  l_run = coax_p_e2q_run(l_t);
  l_k = i32(((runs[((l_run * 4i) + 0i)] + l_t) - runs[((l_run * 4i) + 2i)]));
  l_e = coax_p_e2q_box(inst[(l_k * 2i)], inst[((l_k * 2i) + 1i)], i32(runs[((l_run * 4i) + 3i)]));
  if (coax_p_cl_outside(l_e.x, l_e.y))
  {
    flags[i32(l_t)] = 0u;
    l__ = atomicAdd(&tally[0i], 1u);
    return;
  }
  if (((coax_at_u(i32(22i)) > 0x1.000000p-1f) && coax_p_cl_hidden(l_e.x, l_e.y)))
  {
    flags[i32(l_t)] = 2u;
    l___13 = atomicAdd(&tally[1i], 1u);
    return;
  }
  flags[i32(l_t)] = 1u;
  coax_p_e2q_put(l_k, l_run, 94i, 0i);
}
fn coax_p_e2q_cull_two(p_t: u32) {
  var l_t: u32 = p_t;
  var l_run: i32;
  var l_k: i32;
  var l_e: coax_t10;
  var l__: u32;
  if (((i32(l_t) >= i32(coax_at_u(i32(93i)))) || (flags[i32(l_t)] != 2u)))
  {
    return;
  }
  l_run = coax_p_e2q_run(l_t);
  l_k = i32(((runs[((l_run * 4i) + 0i)] + l_t) - runs[((l_run * 4i) + 2i)]));
  l_e = coax_p_e2q_box(inst[(l_k * 2i)], inst[((l_k * 2i) + 1i)], i32(runs[((l_run * 4i) + 3i)]));
  if (coax_p_cl_hidden(l_e.x, l_e.y))
  {
    flags[i32(l_t)] = 4u;
    l__ = atomicAdd(&tally[2i], 1u);
    return;
  }
  flags[i32(l_t)] = 3u;
  coax_p_e2q_put(l_k, l_run, 95i, 5i);
}
fn coax_p_cl_hiz_depth(p_i: u32) {
  var l_i: u32 = p_i;
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
fn coax_p_cl_hiz_down(p_i: u32) {
  var l_i: u32 = p_i;
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
fn coax_p_e2q_low(p_x: f32) -> u32 {
  var l_x: f32 = p_x;
  return u32((i32(floor((l_x * 0x1.000000p8f))) + 1073741824i));
}
fn coax_p_e2q_high(p_x: f32) -> u32 {
  var l_x: f32 = p_x;
  return u32(((-i32(floor(((-l_x) * 0x1.000000p8f)))) + 1073741824i));
}
fn coax_p_e2q_run(p_t: u32) -> i32 {
  var l_t: u32 = p_t;
  var l_lo: i32;
  var l_hi: i32;
  var l_mid: i32;
  l_lo = 0i;
  l_hi = (i32(coax_at_u(i32(92i))) - 1i);
  while ((l_lo < l_hi))
  {
    l_mid = (((l_lo + l_hi) + 1i) / 2i);
    if ((runs[((l_mid * 4i) + 2i)] <= l_t))
    {
      l_lo = l_mid;
    }
    else
    {
      l_hi = (l_mid - 1i);
    }
  }
  return l_lo;
}
fn coax_p_e2q_box(p_a: vec4f, p_c: vec4f, p_b: i32) -> coax_t10 {
  var l_a: vec4f = p_a;
  var l_c: vec4f = p_c;
  var l_b: i32 = p_b;
  var l_y0: f32;
  var l_y1: f32;
  var l_r: f32;
  l_y0 = (coax_p_e2q_value(boxes[((l_b * 4i) + 0i)]) * l_c.x);
  l_y1 = (coax_p_e2q_value(boxes[((l_b * 4i) + 1i)]) * l_c.x);
  l_r = (((coax_p_e2q_value(boxes[((l_b * 4i) + 2i)]) * coax_p_fabs(l_c.y)) + (0x1.1eb852p-4f * coax_p_fabs(l_c.x))) + 0x1.47ae14p-7f);
  return coax_t10(coax_p_v4((l_a.x - l_r), ((l_a.y + coax_p_fmin(l_y0, l_y1)) - 0x1.47ae14p-7f), (l_a.z - l_r), 0x1.000000p0f), coax_p_v4((l_a.x + l_r), ((l_a.y + coax_p_fmax(l_y0, l_y1)) + 0x1.47ae14p-7f), (l_a.z + l_r), 0x1.000000p0f));
}
fn coax_p_cl_outside(p_low: vec4f, p_high: vec4f) -> bool {
  var l_low: vec4f = p_low;
  var l_high: vec4f = p_high;
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
  var l_low: vec4f = p_low;
  var l_high: vec4f = p_high;
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
fn coax_p_e2q_put(p_k: i32, p_run: i32, p_region: i32, p_draw: i32) {
  var l_k: i32 = p_k;
  var l_run: i32 = p_run;
  var l_region: i32 = p_region;
  var l_draw: i32 = p_draw;
  var l_slot: u32;
  var l_o: i32;
  l_slot = atomicAdd(&args[(((l_run * 10i) + l_draw) + 1i)], 1u);
  l_o = ((i32(coax_at_u(i32(l_region))) + i32(runs[((l_run * 4i) + 0i)])) + i32(l_slot));
  inst[(l_o * 2i)] = inst[(l_k * 2i)];
  inst[((l_o * 2i) + 1i)] = inst[((l_k * 2i) + 1i)];
}
fn coax_p_cl_clampi(p_x: i32, p_low: i32, p_high: i32) -> i32 {
  var l_x: i32 = p_x;
  var l_low: i32 = p_low;
  var l_high: i32 = p_high;
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
  var l_a: f32 = p_a;
  var l_b: f32 = p_b;
  if ((l_b > l_a))
  {
    return l_b;
  }
  return l_a;
}
fn coax_p_e2q_value(p_e: u32) -> f32 {
  var l_e: u32 = p_e;
  return (f32((i32(l_e) - 1073741824i)) / 0x1.000000p8f);
}
fn coax_p_fabs(p_x: f32) -> f32 {
  var l_x: f32 = p_x;
  if ((l_x < 0.0f))
  {
    return (-l_x);
  }
  return l_x;
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
fn coax_p_v4(p_x: f32, p_y: f32, p_z: f32, p_w: f32) -> vec4f {
  var l_x: f32 = p_x;
  var l_y: f32 = p_y;
  var l_z: f32 = p_z;
  var l_w: f32 = p_w;
  return vec4f(l_x, l_y, l_z, l_w);
}
fn coax_p_m4_at(p_at: i32) -> coax_t12 {
  var l_at: i32 = p_at;
  return coax_t12(coax_p_v4(coax_at_u(i32(l_at)), coax_at_u(i32((l_at + 1i))), coax_at_u(i32((l_at + 2i))), coax_at_u(i32((l_at + 3i)))), coax_p_v4(coax_at_u(i32((l_at + 4i))), coax_at_u(i32((l_at + 5i))), coax_at_u(i32((l_at + 6i))), coax_at_u(i32((l_at + 7i)))), coax_p_v4(coax_at_u(i32((l_at + 8i))), coax_at_u(i32((l_at + 9i))), coax_at_u(i32((l_at + 10i))), coax_at_u(i32((l_at + 11i)))), coax_p_v4(coax_at_u(i32((l_at + 12i))), coax_at_u(i32((l_at + 13i))), coax_at_u(i32((l_at + 14i))), coax_at_u(i32((l_at + 15i)))));
}
fn coax_p_m4_apply(p_m: coax_t12, p_v: vec4f) -> vec4f {
  var l_m: coax_t12 = p_m;
  var l_v: vec4f = p_v;
  return coax_p_add4(coax_p_add4(coax_p_scale4(l_m.x, l_v.x), coax_p_scale4(l_m.y, l_v.y)), coax_p_add4(coax_p_scale4(l_m.z, l_v.z), coax_p_scale4(l_m.w, l_v.w)));
}
fn coax_p_scale4(p_a: vec4f, p_k: f32) -> vec4f {
  var l_a: vec4f = p_a;
  var l_k: f32 = p_k;
  return vec4f((l_a.x * l_k), (l_a.y * l_k), (l_a.z * l_k), (l_a.w * l_k));
}
fn coax_p_add4(p_a: vec4f, p_b: vec4f) -> vec4f {
  var l_a: vec4f = p_a;
  var l_b: vec4f = p_b;
  return vec4f((l_a.x + l_b.x), (l_a.y + l_b.y), (l_a.z + l_b.z), (l_a.w + l_b.w));
}
@compute @workgroup_size(64)
fn e2q_bounds(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_e2q_bounds(coax_gid.x);
}
@compute @workgroup_size(64)
fn e2q_cull_one(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_e2q_cull_one(coax_gid.x);
}
@compute @workgroup_size(64)
fn e2q_cull_two(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_e2q_cull_two(coax_gid.x);
}
@compute @workgroup_size(64)
fn cl_hiz_depth(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_cl_hiz_depth(coax_gid.x);
}
@compute @workgroup_size(64)
fn cl_hiz_down(@builtin(global_invocation_id) coax_gid: vec3u) {
  coax_p_cl_hiz_down(coax_gid.x);
}