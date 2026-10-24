// Visibility buffer path: cluster expansion and culling, ID rasterization, and
// material resolve. Rasterization writes only (cluster entry + 1, triangle)
// and depth; resolve reconstructs each pixel's triangle, barycentrics, and
// texture derivatives, then shades once per pixel.

// --- Cluster expansion -------------------------------------------------------

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> instances: array<Instance>;
@group(0) @binding(2) var<storage, read> lods: array<MeshLod>;
@group(0) @binding(3) var<storage, read> clusters: array<Cluster>;
@group(0) @binding(4) var<storage, read> visible: array<u32>;
@group(0) @binding(5) var<storage, read> visible_args: array<u32>;
@group(0) @binding(6) var<storage, read_write> dispatch_args: array<u32>;
@group(0) @binding(7) var<storage, read_write> cluster_list: array<vec2u>;
@group(0) @binding(8) var<storage, read_write> draw_args: array<atomic<u32>>;
@group(0) @binding(9) var<storage, read_write> stats: array<atomic<u32>>;

const MAX_GROUPS: u32 = 65535u;

// Turns the visible-instance count into a 2D dispatch: one workgroup each.
// Each entry point binds only what it uses, so the indirect buffer is never
// also writable storage in the same dispatch.
@compute @workgroup_size(1)
fn prepare_dispatch() {
  let count = visible_args[0];
  dispatch_args[0] = min(count, MAX_GROUPS);
  dispatch_args[1] = max((count + MAX_GROUPS - 1u) / MAX_GROUPS, 1u);
  dispatch_args[2] = 1u;
}

@compute @workgroup_size(64)
fn expand(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_index) lane: u32) {
  let slot = wg.x + wg.y * MAX_GROUPS;
  if (slot >= visible_args[0]) { return; }
  let index = visible[slot * 2u];
  let lod = visible[slot * 2u + 1u];
  let inst = instances[index];
  let level = lods[inst.mesh * LODS + lod];
  let rscale = instance_radius_scale(inst);
  let uniform_scale = inst.height_scale == 1.0;
  let capacity = arrayLength(&cluster_list);
  for (var k = lane; k < level.cluster_count; k += 64u) {
    let ci = level.first_cluster + k;
    let c = clusters[ci];
    let center = instance_point(inst, vec3f(c.cx, c.cy, c.cz));
    let radius = c.radius * rscale;
    if (!sphere_in_frustum(frame, center, radius)) { continue; }
    if (uniform_scale && c.cutoff <= 1.0) {
      let axis = instance_normal(inst, vec3f(c.ax, c.ay, c.az));
      let d = center - frame.camera.xyz;
      if (dot(d, axis) >= c.cutoff * length(d) + radius) { continue; }
    }
    let out = atomicAdd(&draw_args[1], 1u);
    if (out < capacity) {
      cluster_list[out] = vec2u(index, ci);
      atomicAdd(&stats[2], 1u);
    }
  }
}

// --- ID rasterization --------------------------------------------------------

@group(0) @binding(10) var<storage, read> vertices: array<Vertex>;
@group(0) @binding(11) var<storage, read> indices: array<u32>;
@group(0) @binding(12) var<storage, read> raster_list: array<vec2u>;
// High bit set for the phase-2 list, so resolve knows which list an ID names.
@group(0) @binding(18) var<uniform> raster_tag: vec4u;
@group(0) @binding(19) var<storage, read> raster_list_b: array<vec2u>;

struct IdOut {
  @builtin(position) clip: vec4f,
  @location(0) @interpolate(flat) id: vec2u,
}

@vertex
fn raster_vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) entry: u32) -> IdOut {
  var out: IdOut;
  out.clip = vec4f(2.0, 2.0, 2.0, 1.0);
  out.id = vec2u(0u);
  if (entry >= arrayLength(&raster_list)) { return out; }
  let e = raster_list[entry];
  let c = clusters[e.y];
  let tri = vi / 3u;
  if (tri >= c.triangle_count) { return out; }
  let v = vertices[c.base_vertex + indices[c.first_index + vi]];
  let inst = instances[e.x];
  let world = instance_point(inst, vec3f(v.px, v.py, v.pz));
  out.clip = frame.view_proj * vec4f(world, 1.0);
  out.id = vec2u((entry + 1u) | raster_tag.x, tri);
  return out;
}

@fragment
fn raster_fs(input: IdOut) -> @location(0) vec2u {
  return input.id;
}

// --- Material resolve --------------------------------------------------------

@group(0) @binding(13) var vis: texture_2d<u32>;
@group(0) @binding(14) var<storage, read> materials: array<Material>;
@group(0) @binding(15) var textures: texture_2d_array<f32>;
@group(0) @binding(16) var linear_sampler: sampler;
@group(0) @binding(17) var hdr: texture_storage_2d<rgba16float, write>;

fn pixel_ray(p: vec2f) -> vec3f {
  let ndc = vec2f(p.x * frame.viewport.z * 2.0 - 1.0, 1.0 - p.y * frame.viewport.w * 2.0) - frame.jitter.xy;
  let far = frame.inv_view_proj * vec4f(ndc, 1.0, 1.0);
  return normalize(far.xyz / far.w - frame.camera.xyz);
}

// Barycentrics of the ray's hit on the triangle's plane.
fn barycentrics(o: vec3f, d: vec3f, a: vec3f, b: vec3f, c: vec3f) -> vec3f {
  let e1 = b - a;
  let e2 = c - a;
  let p = cross(d, e2);
  let det = dot(e1, p);
  let inv = 1.0 / select(det, 1.0e-12, abs(det) < 1.0e-12);
  let t = o - a;
  let u = dot(t, p) * inv;
  let q = cross(t, e1);
  let v = dot(d, q) * inv;
  return vec3f(1.0 - u - v, u, v);
}

@compute @workgroup_size(8, 8)
fn resolve(@builtin(global_invocation_id) id: vec3u) {
  let dims = textureDimensions(vis);
  if (any(id.xy >= dims)) { return; }
  let pix = vec2f(id.xy) + 0.5;
  let ray = pixel_ray(pix);
  let sample = textureLoad(vis, id.xy, 0).xy;
  if (sample.x == 0u) {
    textureStore(hdr, id.xy, vec4f(sky_color(ray, frame.sun), 1.0));
    return;
  }
  let entry = (sample.x & 0x7fffffffu) - 1u;
  var e: vec2u;
  if ((sample.x & 0x80000000u) != 0u) { e = raster_list_b[entry]; } else { e = raster_list[entry]; }
  let inst = instances[e.x];
  let c = clusters[e.y];
  let base = c.first_index + sample.y * 3u;
  var p: array<vec3f, 3>;
  var n: array<vec3f, 3>;
  var t: array<vec2f, 3>;
  for (var k = 0u; k < 3u; k++) {
    let v = vertices[c.base_vertex + indices[base + k]];
    p[k] = instance_point(inst, vec3f(v.px, v.py, v.pz));
    n[k] = vec3f(v.nx, v.ny, v.nz);
    t[k] = vec2f(v.u, v.v);
  }
  let o = frame.camera.xyz;
  let b = barycentrics(o, ray, p[0], p[1], p[2]);
  let bx = barycentrics(o, pixel_ray(pix + vec2f(1.0, 0.0)), p[0], p[1], p[2]);
  let by = barycentrics(o, pixel_ray(pix + vec2f(0.0, 1.0)), p[0], p[1], p[2]);
  let m = materials[inst.material];
  let uv = (t[0] * b.x + t[1] * b.y + t[2] * b.z) * m.uv_scale;
  let uvx = (t[0] * bx.x + t[1] * bx.y + t[2] * bx.z) * m.uv_scale;
  let uvy = (t[0] * by.x + t[1] * by.y + t[2] * by.z) * m.uv_scale;
  let world = p[0] * b.x + p[1] * b.y + p[2] * b.z;
  let normal = instance_normal(inst, n[0] * b.x + n[1] * b.y + n[2] * b.z);
  let texel = textureSampleGrad(textures, linear_sampler, uv, m.layer, uvx - uv, uvy - uv).rgb;
  let color = shade(frame, m.color * texel, normal, world, m.roughness);
  textureStore(hdr, id.xy, vec4f(color, 1.0));
}
