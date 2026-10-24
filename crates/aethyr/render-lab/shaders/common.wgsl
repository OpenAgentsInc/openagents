// Shared definitions for every render lab shader. Rust prepends this file.

struct Frame {
  view_proj: mat4x4f,       // jittered; used to rasterize
  view_proj_nj: mat4x4f,    // unjittered; culling and reprojection
  prev_view_proj: mat4x4f,  // unjittered, previous frame
  inv_view_proj: mat4x4f,   // inverse of view_proj_nj
  camera: vec4f,            // xyz position, w time in seconds
  sun: vec4f,               // xyz direction toward the sun, w intensity
  planes: array<vec4f, 6>,  // frustum planes, inside when dot(n, p) + d >= 0
  viewport: vec4f,          // render width, height, 1/width, 1/height
  jitter: vec4f,            // xy clip-space jitter applied to view_proj
  flags: vec4u,             // x lod on, y occlusion on, z instance count, w crowd count
  lod: vec4f,               // x pixels per unit at distance 1, y lod bias, z hiz mips, w crowd near distance
  tiers: vec4f,             // x crowd mid distance, y crowd far distance, z tiers on, w unused
}

struct Instance {
  position: vec3f,
  scale: f32,
  yaw: f32,
  height_scale: f32,
  mesh: u32,
  material: u32,
}

struct Material {
  color: vec3f,
  roughness: f32,
  layer: u32,
  uv_scale: f32,
  pad0: u32,
  pad1: u32,
}

struct Vertex {
  px: f32, py: f32, pz: f32,
  nx: f32, ny: f32, nz: f32,
  u: f32, v: f32,
}

struct MeshBounds {
  center: vec3f,
  radius: f32,
}

struct MeshLod {
  first_index: u32,
  index_count: u32,
  base_vertex: u32,
  first_cluster: u32,
  cluster_count: u32,
  pad0: u32, pad1: u32, pad2: u32,
}

struct Cluster {
  first_index: u32,
  triangle_count: u32,
  base_vertex: u32,
  pad: u32,
  cx: f32, cy: f32, cz: f32,
  radius: f32,
  ax: f32, ay: f32, az: f32,
  cutoff: f32,
}

const LODS: u32 = 4u;

fn instance_point(inst: Instance, p: vec3f) -> vec3f {
  let s = vec3f(inst.scale, inst.scale * inst.height_scale, inst.scale);
  let q = p * s;
  let c = cos(inst.yaw);
  let n = sin(inst.yaw);
  return vec3f(c * q.x - n * q.z, q.y, n * q.x + c * q.z) + inst.position;
}

fn instance_normal(inst: Instance, nrm: vec3f) -> vec3f {
  let s = vec3f(inst.scale, inst.scale * inst.height_scale, inst.scale);
  let q = nrm / s;
  let c = cos(inst.yaw);
  let n = sin(inst.yaw);
  return normalize(vec3f(c * q.x - n * q.z, q.y, n * q.x + c * q.z));
}

fn instance_radius_scale(inst: Instance) -> f32 {
  return inst.scale * max(1.0, inst.height_scale);
}

fn sphere_in_frustum(frame: Frame, center: vec3f, radius: f32) -> bool {
  for (var i = 0u; i < 6u; i++) {
    let pl = frame.planes[i];
    if (dot(pl.xyz, center) + pl.w < -radius) { return false; }
  }
  return true;
}

fn select_lod(frame: Frame, center: vec3f, radius: f32) -> u32 {
  if (frame.flags.x == 0u) { return 0u; }
  let dist = max(distance(center, frame.camera.xyz), 0.1);
  let pixels = radius / dist * frame.lod.x * frame.lod.y;
  if (pixels > 160.0) { return 0u; }
  if (pixels > 60.0) { return 1u; }
  if (pixels > 24.0) { return 2u; }
  return 3u;
}

fn sky_color(dir: vec3f, sun: vec4f) -> vec3f {
  let h = clamp(dir.y, -1.0, 1.0);
  let zenith = vec3f(0.18, 0.32, 0.62);
  let horizon = vec3f(0.62, 0.7, 0.78);
  var c = mix(horizon, zenith, pow(max(h, 0.0), 0.5));
  if (h < 0.0) { c = mix(horizon, vec3f(0.25, 0.24, 0.22), min(-h * 4.0, 1.0)); }
  let s = max(dot(dir, sun.xyz), 0.0);
  c += vec3f(1.0, 0.85, 0.6) * (pow(s, 600.0) * 30.0 + pow(s, 8.0) * 0.25);
  return c;
}

fn shade(frame: Frame, albedo: vec3f, nrm: vec3f, world: vec3f, roughness: f32) -> vec3f {
  let n = normalize(nrm);
  let v = normalize(frame.camera.xyz - world);
  let l = frame.sun.xyz;
  let h = normalize(l + v);
  let ndl = max(dot(n, l), 0.0);
  let sun_color = vec3f(1.0, 0.92, 0.8) * frame.sun.w;
  let shininess = mix(96.0, 6.0, roughness);
  let spec = pow(max(dot(n, h), 0.0), shininess) * (1.0 - roughness) * 0.6;
  let ambient = mix(vec3f(0.22, 0.2, 0.18), vec3f(0.35, 0.45, 0.62), n.y * 0.5 + 0.5);
  var c = albedo * (sun_color * ndl + ambient) + sun_color * spec * ndl;
  let d = distance(frame.camera.xyz, world);
  let fog = 1.0 - exp(-d * 0.0018);
  return mix(c, sky_color(-v, frame.sun) * 0.9, fog);
}
