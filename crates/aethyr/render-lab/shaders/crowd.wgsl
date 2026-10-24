// Crowd tiers: near agents use the full mesh with skinning evaluated per
// vertex; mid agents use a low mesh whose animation is baked into a texture
// (VAT); far agents are camera-facing impostors from a pre-rendered atlas.
// Agent motion is analytic in time, so simulation needs no state and every
// run sees the same crowd.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var hiz: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> near_list: array<u32>;
@group(0) @binding(3) var<storage, read_write> mid_list: array<u32>;
@group(0) @binding(4) var<storage, read_write> far_list: array<u32>;
@group(0) @binding(5) var<storage, read_write> crowd_args: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> stats: array<atomic<u32>>;
@group(0) @binding(7) var vat: texture_2d<f32>;
@group(0) @binding(8) var atlas: texture_2d<f32>;
@group(0) @binding(9) var atlas_sampler: sampler;

const TAU: f32 = 6.28318530718;
const VAT_FRAMES: u32 = 16u;
const ATLAS_DIRS: f32 = 8.0;
const ATLAS_FRAMES: f32 = 8.0;
const HALF_WIDTH: f32 = 0.625;

struct Agent {
  position: vec3f,
  yaw: f32,
  phase: f32,
  tint: vec3f,
}

fn hash(x: u32) -> u32 {
  var h = x * 747796405u + 2891336453u;
  h = ((h >> ((h >> 28u) + 4u)) ^ h) * 277803737u;
  return (h >> 22u) ^ h;
}

fn unit(x: u32) -> f32 {
  return f32(hash(x) & 0xffffffu) / 16777216.0;
}

fn agent(i: u32) -> Agent {
  let spread = sqrt(f32(frame.flags.w)) * 1.6 + 20.0;
  let center = (vec2f(unit(i * 7u + 1u), unit(i * 7u + 2u)) * 2.0 - 1.0) * spread;
  let r = 2.0 + 8.0 * unit(i * 7u + 3u);
  let dir = select(-1.0, 1.0, unit(i * 7u + 4u) > 0.5);
  let speed = 0.9 + 0.8 * unit(i * 7u + 5u);
  let theta = unit(i * 7u + 6u) * TAU + dir * speed / r * frame.camera.w;
  var a: Agent;
  a.position = vec3f(center.x + r * cos(theta), 0.0, center.y + r * sin(theta));
  let heading = vec2f(-sin(theta), cos(theta)) * dir;
  a.yaw = atan2(-heading.x, heading.y);
  a.phase = fract(frame.camera.w * speed / 1.3 + unit(i * 7u + 7u));
  a.tint = mix(vec3f(0.55, 0.25, 0.2), vec3f(0.2, 0.35, 0.6), unit(i * 13u + 1u));
  return a;
}

fn rotate_y(p: vec3f, yaw: f32) -> vec3f {
  let c = cos(yaw);
  let s = sin(yaw);
  return vec3f(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
}

fn rot_x(p: vec3f, pivot: vec3f, a: f32) -> vec3f {
  let q = p - pivot;
  let c = cos(a);
  let s = sin(a);
  return pivot + vec3f(q.x, q.y * c - q.z * s, q.y * s + q.z * c);
}

// Rigid skinning with an 11-bone walk cycle. Bone order matches
// `scene::character`. Keep in step with `animate` in crowd.rs, which bakes the
// VAT from the same math. The transform is affine, so a normal's rotation is
// animate(n) - animate(0).
fn animate(p: vec3f, bone: u32, phase: f32) -> vec3f {
  let swing = sin(phase * TAU);
  let side = select(1.0, -1.0, bone == 3u || bone == 5u || bone == 7u || bone == 9u);
  var q = p;
  if (bone == 9u || bone == 10u) {
    q = rot_x(q, vec3f(side * 0.1, 0.47, 0.0), 0.6 * max(0.0, side * swing));
  }
  if (bone == 7u || bone == 8u || bone == 9u || bone == 10u) {
    q = rot_x(q, vec3f(side * 0.1, 0.9, 0.0), side * 0.5 * swing);
  }
  if (bone == 5u || bone == 6u) {
    q = rot_x(q, vec3f(side * 0.3, 1.25, 0.0), -0.3);
  }
  if (bone == 3u || bone == 4u || bone == 5u || bone == 6u) {
    q = rot_x(q, vec3f(side * 0.3, 1.55, 0.0), -side * 0.4 * swing);
  }
  if (bone != 7u && bone != 8u && bone != 9u && bone != 10u) {
    q.y += 0.04 * abs(cos(phase * TAU));
  }
  return q;
}

fn animate_normal(n: vec3f, bone: u32, phase: f32) -> vec3f {
  return normalize(animate(n, bone, phase) - animate(vec3f(0.0), bone, phase));
}

fn crowd_occluded(center: vec3f, radius: f32) -> bool {
  var uv_min = vec2f(1.0e9);
  var uv_max = vec2f(-1.0e9);
  var z_min = 1.0e9;
  for (var k = 0u; k < 8u; k++) {
    let corner = center + vec3f(
      select(-radius, radius, (k & 1u) != 0u),
      select(-radius, radius, (k & 2u) != 0u),
      select(-radius, radius, (k & 4u) != 0u));
    let clip = frame.view_proj_nj * vec4f(corner, 1.0);
    if (clip.w <= 0.05) { return false; }
    let ndc = clip.xyz / clip.w;
    let uv = vec2f(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    uv_min = min(uv_min, uv);
    uv_max = max(uv_max, uv);
    z_min = min(z_min, ndc.z);
  }
  uv_min = clamp(uv_min, vec2f(0.0), vec2f(1.0));
  uv_max = clamp(uv_max, vec2f(0.0), vec2f(1.0));
  let extent = (uv_max - uv_min) * vec2f(textureDimensions(hiz, 0));
  let level = min(u32(ceil(log2(max(max(extent.x, extent.y), 1.0)))), u32(frame.lod.z) - 1u);
  let dims = vec2u(textureDimensions(hiz, level));
  let lo = min(vec2u(uv_min * vec2f(dims)), dims - 1u);
  let hi = min(vec2u(uv_max * vec2f(dims)), dims - 1u);
  let z = max(
    max(textureLoad(hiz, lo, level).r, textureLoad(hiz, vec2u(hi.x, lo.y), level).r),
    max(textureLoad(hiz, vec2u(lo.x, hi.y), level).r, textureLoad(hiz, hi, level).r));
  return z_min > z;
}

@compute @workgroup_size(64)
fn crowd_cull(@builtin(global_invocation_id) id: vec3u) {
  let i = id.x + id.y * 65535u * 64u;
  if (i >= frame.flags.w) { return; }
  let a = agent(i);
  let center = a.position + vec3f(0.0, 0.9, 0.0);
  if (!sphere_in_frustum(frame, center, 1.0)) { return; }
  let d = distance(center, frame.camera.xyz);
  if (frame.tiers.z == 0.0) {
    if (frame.flags.y != 0u && crowd_occluded(center, 1.0)) { return; }
    let slot = atomicAdd(&crowd_args[1], 1u);
    near_list[slot] = i;
    atomicAdd(&stats[3], 1u);
    return;
  }
  if (d > frame.tiers.y) { return; }
  if (frame.flags.y != 0u && crowd_occluded(center, 1.0)) { return; }
  if (d < frame.lod.w) {
    let slot = atomicAdd(&crowd_args[1], 1u);
    near_list[slot] = i;
    atomicAdd(&stats[3], 1u);
  } else if (d < frame.tiers.x) {
    let slot = atomicAdd(&crowd_args[6], 1u);
    mid_list[slot] = i;
    atomicAdd(&stats[4], 1u);
  } else {
    let slot = atomicAdd(&crowd_args[11], 1u);
    far_list[slot] = i;
    atomicAdd(&stats[5], 1u);
  }
}

struct CrowdOut {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) tint: vec3f,
}

@vertex
fn near_vs(@location(0) position: vec3f, @location(1) normal: vec3f, @location(2) bone: u32, @location(3) id: u32) -> CrowdOut {
  let a = agent(id);
  let p = animate(position, bone, a.phase);
  let n = animate_normal(normal, bone, a.phase);
  var out: CrowdOut;
  out.world = rotate_y(p, a.yaw) + a.position;
  out.normal = rotate_y(n, a.yaw);
  out.clip = frame.view_proj * vec4f(out.world, 1.0);
  out.tint = a.tint;
  return out;
}

@vertex
fn mid_vs(@builtin(vertex_index) vi: u32, @location(0) id: u32) -> CrowdOut {
  let a = agent(id);
  let f = a.phase * f32(VAT_FRAMES);
  let f0 = u32(f) % VAT_FRAMES;
  let f1 = (f0 + 1u) % VAT_FRAMES;
  let w = fract(f);
  let p = mix(textureLoad(vat, vec2u(vi, f0), 0).xyz, textureLoad(vat, vec2u(vi, f1), 0).xyz, w);
  let n = mix(textureLoad(vat, vec2u(vi, f0 + VAT_FRAMES), 0).xyz, textureLoad(vat, vec2u(vi, f1 + VAT_FRAMES), 0).xyz, w);
  var out: CrowdOut;
  out.world = rotate_y(p, a.yaw) + a.position;
  out.normal = rotate_y(normalize(n), a.yaw);
  out.clip = frame.view_proj * vec4f(out.world, 1.0);
  out.tint = a.tint;
  return out;
}

@fragment
fn body_fs(input: CrowdOut) -> @location(0) vec4f {
  return vec4f(shade(frame, input.tint, input.normal, input.world, 0.7), 1.0);
}

struct ImpostorOut {
  @builtin(position) clip: vec4f,
  @location(0) uv: vec2f,
  @location(1) world: vec3f,
}

@vertex
fn far_vs(@builtin(vertex_index) vi: u32, @location(0) id: u32) -> ImpostorOut {
  let a = agent(id);
  let corners = array<vec2f, 6>(vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 0.0), vec2f(1.0, 1.0), vec2f(0.0, 1.0));
  let k = corners[vi];
  let to_cam = frame.camera.xyz - a.position;
  let flat_dir = normalize(vec3f(to_cam.x, 0.0, to_cam.z) + vec3f(1.0e-5, 0.0, 0.0));
  let right = normalize(cross(vec3f(0.0, 1.0, 0.0), flat_dir));
  // Camera direction in the agent's frame selects the atlas column.
  let local = rotate_y(flat_dir, -a.yaw);
  let angle = atan2(local.x, local.z);
  let column = (u32(round(angle / TAU * ATLAS_DIRS + ATLAS_DIRS)) % u32(ATLAS_DIRS));
  let row = u32(a.phase * ATLAS_FRAMES) % u32(ATLAS_FRAMES);
  var out: ImpostorOut;
  out.world = a.position + right * (k.x * 2.0 - 1.0) * HALF_WIDTH + vec3f(0.0, k.y * 2.0 - 0.05, 0.0);
  out.clip = frame.view_proj * vec4f(out.world, 1.0);
  out.uv = (vec2f(f32(column), f32(row)) + vec2f(k.x, 1.0 - k.y)) / vec2f(ATLAS_DIRS, ATLAS_FRAMES);
  return out;
}

@fragment
fn far_fs(input: ImpostorOut) -> @location(0) vec4f {
  let c = textureSample(atlas, atlas_sampler, input.uv);
  if (c.a < 0.5) { discard; }
  let d = distance(frame.camera.xyz, input.world);
  let fog = 1.0 - exp(-d * 0.0018);
  let v = normalize(input.world - frame.camera.xyz);
  return vec4f(mix(c.rgb, sky_color(v, frame.sun) * 0.9, fog), 1.0);
}

// --- Impostor bake -----------------------------------------------------------
// One instance per atlas cell: column = view direction, row = animation frame.

struct BakeOut {
  @builtin(position) clip: vec4f,
  @location(0) normal: vec3f,
}

@vertex
fn bake_vs(@location(0) position: vec3f, @location(1) normal: vec3f, @location(2) bone: u32, @builtin(instance_index) cell: u32) -> BakeOut {
  let column = f32(cell % u32(ATLAS_DIRS));
  let row = f32(cell / u32(ATLAS_DIRS));
  let phase = row / ATLAS_FRAMES;
  let angle = column / ATLAS_DIRS * TAU;
  let p = rotate_y(animate(position, bone, phase), angle);
  let n = rotate_y(animate_normal(normal, bone, phase), angle);
  let local = vec2f(p.x / HALF_WIDTH, (p.y - 0.95) / 1.0);
  let cell_uv = vec2f(column + local.x * 0.5 + 0.5, row + 0.5 - local.y * 0.5) / vec2f(ATLAS_DIRS, ATLAS_FRAMES);
  var out: BakeOut;
  out.clip = vec4f(cell_uv.x * 2.0 - 1.0, 1.0 - cell_uv.y * 2.0, clamp(0.5 - p.z * 0.2, 0.0, 1.0), 1.0);
  out.normal = n;
  return out;
}

@fragment
fn bake_fs(input: BakeOut) -> @location(0) vec4f {
  let n = normalize(input.normal);
  let l = normalize(vec3f(0.4, 0.8, 0.45));
  let albedo = vec3f(0.4, 0.3, 0.32);
  let lit = albedo * (vec3f(1.0, 0.92, 0.8) * 2.6 * max(dot(n, l), 0.0) + mix(vec3f(0.22, 0.2, 0.18), vec3f(0.35, 0.45, 0.62), n.y * 0.5 + 0.5));
  return vec4f(lit, 1.0);
}
