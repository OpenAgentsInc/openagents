// Temporal anti-aliasing with upscaling, and the final tone-mapped blit.
//
// TAA runs at output resolution. It samples the jittered render-resolution
// color at the unjittered position, reprojects history with depth and the
// previous camera, clamps history to the current 3x3 neighborhood, and blends.
// Moving agents rely on the clamp; the lab has no per-object motion vectors.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var current: texture_2d<f32>;
@group(0) @binding(2) var depth: texture_depth_2d;
@group(0) @binding(3) var history: texture_2d<f32>;
@group(0) @binding(4) var linear_sampler: sampler;
@group(0) @binding(5) var resolved: texture_storage_2d<rgba16float, write>;
@group(0) @binding(6) var<uniform> taa: vec4f; // x history weight, y reset, zw unused

@compute @workgroup_size(8, 8)
fn taa_resolve(@builtin(global_invocation_id) id: vec3u) {
  let out_dims = textureDimensions(resolved);
  if (any(id.xy >= out_dims)) { return; }
  let uv = (vec2f(id.xy) + 0.5) / vec2f(out_dims);
  let jitter_uv = vec2f(frame.jitter.x * 0.5, -frame.jitter.y * 0.5);
  let in_dims = vec2i(textureDimensions(current, 0));
  let color = textureSampleLevel(current, linear_sampler, uv + jitter_uv, 0.0).rgb;
  let center = clamp(vec2i((uv + jitter_uv) * vec2f(in_dims)), vec2i(0), in_dims - 1);
  var lo = vec3f(1.0e9);
  var hi = vec3f(-1.0e9);
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let c = textureLoad(current, clamp(center + vec2i(x, y), vec2i(0), in_dims - 1), 0).rgb;
      lo = min(lo, c);
      hi = max(hi, c);
    }
  }
  let z = textureLoad(depth, center, 0);
  let ndc = vec4f(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, z, 1.0);
  let world = frame.inv_view_proj * ndc;
  let prev = frame.prev_view_proj * vec4f(world.xyz / world.w, 1.0);
  let prev_uv = vec2f(prev.x / prev.w * 0.5 + 0.5, 0.5 - prev.y / prev.w * 0.5);
  var weight = taa.x;
  if (taa.y != 0.0 || any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
    weight = 0.0;
  }
  let past = clamp(textureSampleLevel(history, linear_sampler, prev_uv, 0.0).rgb, lo, hi);
  textureStore(resolved, id.xy, vec4f(mix(color, past, weight), 1.0));
}

// --- Blit --------------------------------------------------------------------

@group(0) @binding(7) var source: texture_2d<f32>;

struct BlitOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
}

@vertex
fn blit_vs(@builtin(vertex_index) vi: u32) -> BlitOut {
  let p = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u));
  var out: BlitOut;
  out.clip = vec4f(p * 2.0 - 1.0, 0.0, 1.0);
  out.uv = vec2f(p.x, 1.0 - p.y);
  return out;
}

fn aces(x: vec3f) -> vec3f {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3f(0.0), vec3f(1.0));
}

fn to_srgb(c: vec3f) -> vec3f {
  return select(1.055 * pow(c, vec3f(1.0 / 2.4)) - 0.055, c * 12.92, c <= vec3f(0.0031308));
}

@fragment
fn blit_fs(input: BlitOut) -> @location(0) vec4f {
  let hdr = textureSampleLevel(source, linear_sampler, input.uv, 0.0).rgb;
  return vec4f(to_srgb(aces(hdr * 0.8)), 1.0);
}
