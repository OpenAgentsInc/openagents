// GPU visibility: frustum culling, LOD selection, and two-phase occlusion
// culling against a hierarchical depth buffer (HiZ).
//
// Bucket outputs (the GPU-driven forward path) write one indexed indirect draw
// per mesh LOD. Each bucket owns a static region of the list buffer, so the
// draw needs no firstInstance and no prefix sum. List outputs (the visibility
// buffer path) write (instance, lod) pairs for cluster expansion.
//
// Two-phase occlusion: phase 1 draws what was visible last frame; HiZ is built
// from that depth; phase 2 tests everything against it, draws what phase 1
// missed, and records visibility for the next frame.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> instances: array<Instance>;
@group(0) @binding(2) var<storage, read> bounds: array<MeshBounds>;
@group(0) @binding(3) var<storage, read> lods: array<MeshLod>;
@group(0) @binding(4) var<storage, read_write> visibility: array<u32>;
@group(0) @binding(5) var<storage, read_write> args_a: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> list_a: array<u32>;
@group(0) @binding(7) var<storage, read_write> args_b: array<atomic<u32>>;
@group(0) @binding(8) var<storage, read_write> list_b: array<u32>;
@group(0) @binding(9) var<storage, read> bucket_base: array<u32>;
@group(0) @binding(10) var hiz: texture_2d<f32>;
@group(0) @binding(11) var<storage, read_write> stats: array<atomic<u32>>;

fn touch_all() {
  _ = &visibility; _ = &args_a; _ = &list_a; _ = &args_b; _ = &list_b;
  _ = &bucket_base; _ = hiz; _ = &stats; _ = &lods; _ = &bounds;
}

struct Test {
  center: vec3f,
  radius: f32,
  lod: u32,
}

fn prepare(index: u32) -> Test {
  let inst = instances[index];
  let b = bounds[inst.mesh];
  var t: Test;
  t.center = instance_point(inst, b.center);
  t.radius = b.radius * instance_radius_scale(inst);
  t.lod = select_lod(frame, t.center, t.radius);
  return t;
}

fn hiz_occluded(center: vec3f, radius: f32) -> bool {
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
  let dims0 = vec2f(textureDimensions(hiz, 0));
  let extent = (uv_max - uv_min) * dims0;
  let mips = u32(frame.lod.z);
  let level = min(u32(ceil(log2(max(max(extent.x, extent.y), 1.0)))), mips - 1u);
  let dims = vec2u(textureDimensions(hiz, level));
  let lo = min(vec2u(uv_min * vec2f(dims)), dims - 1u);
  let hi = min(vec2u(uv_max * vec2f(dims)), dims - 1u);
  let z = max(
    max(textureLoad(hiz, lo, level).r, textureLoad(hiz, vec2u(hi.x, lo.y), level).r),
    max(textureLoad(hiz, vec2u(lo.x, hi.y), level).r, textureLoad(hiz, hi, level).r));
  return z_min > z;
}

fn count_visible(index: u32, lod: u32) {
  atomicAdd(&stats[0], 1u);
  atomicAdd(&stats[1], lods[instances[index].mesh * LODS + lod].index_count / 3u);
}

fn emit_bucket_a(index: u32, lod: u32) {
  let bucket = instances[index].mesh * LODS + lod;
  let slot = atomicAdd(&args_a[bucket * 5u + 1u], 1u);
  list_a[bucket_base[bucket] + slot] = index;
  count_visible(index, lod);
}

fn emit_bucket_b(index: u32, lod: u32) {
  let bucket = instances[index].mesh * LODS + lod;
  let slot = atomicAdd(&args_b[bucket * 5u + 1u], 1u);
  list_b[bucket_base[bucket] + slot] = index;
  count_visible(index, lod);
}

fn emit_list_a(index: u32, lod: u32) {
  let slot = atomicAdd(&args_a[0], 1u);
  list_a[slot * 2u] = index;
  list_a[slot * 2u + 1u] = lod;
  count_visible(index, lod);
}

fn emit_list_b(index: u32, lod: u32) {
  let slot = atomicAdd(&args_b[0], 1u);
  list_b[slot * 2u] = index;
  list_b[slot * 2u + 1u] = lod;
  count_visible(index, lod);
}

// Phase 2 shared logic: returns 0 when culled, 1 when newly visible, 2 when it
// stays visible from phase 1. Updates the visibility record.
fn phase_two(index: u32, t: Test) -> u32 {
  if (!sphere_in_frustum(frame, t.center, t.radius)) {
    visibility[index] = 0u;
    return 0u;
  }
  if (hiz_occluded(t.center, t.radius)) {
    visibility[index] = 0u;
    atomicAdd(&stats[6], 1u);
    return 0u;
  }
  let was = visibility[index];
  visibility[index] = 1u;
  return select(1u, 2u, was != 0u);
}

@compute @workgroup_size(64)
fn single_buckets(@builtin(global_invocation_id) id: vec3u) {
  touch_all();
  let index = id.x + id.y * 65535u * 64u;
  if (index >= frame.flags.z) { return; }
  let t = prepare(index);
  if (sphere_in_frustum(frame, t.center, t.radius)) { emit_bucket_a(index, t.lod); }
}

@compute @workgroup_size(64)
fn phase1_buckets(@builtin(global_invocation_id) id: vec3u) {
  touch_all();
  let index = id.x + id.y * 65535u * 64u;
  if (index >= frame.flags.z || visibility[index] == 0u) { return; }
  let t = prepare(index);
  if (sphere_in_frustum(frame, t.center, t.radius)) { emit_bucket_a(index, t.lod); }
}

@compute @workgroup_size(64)
fn phase2_buckets(@builtin(global_invocation_id) id: vec3u) {
  touch_all();
  let index = id.x + id.y * 65535u * 64u;
  if (index >= frame.flags.z) { return; }
  let t = prepare(index);
  if (phase_two(index, t) == 1u) { emit_bucket_b(index, t.lod); }
}

@compute @workgroup_size(64)
fn single_list(@builtin(global_invocation_id) id: vec3u) {
  touch_all();
  let index = id.x + id.y * 65535u * 64u;
  if (index >= frame.flags.z) { return; }
  let t = prepare(index);
  if (sphere_in_frustum(frame, t.center, t.radius)) { emit_list_a(index, t.lod); }
}

@compute @workgroup_size(64)
fn phase1_list(@builtin(global_invocation_id) id: vec3u) {
  touch_all();
  let index = id.x + id.y * 65535u * 64u;
  if (index >= frame.flags.z || visibility[index] == 0u) { return; }
  let t = prepare(index);
  if (sphere_in_frustum(frame, t.center, t.radius)) { emit_list_a(index, t.lod); }
}

@compute @workgroup_size(64)
fn phase2_list(@builtin(global_invocation_id) id: vec3u) {
  touch_all();
  let index = id.x + id.y * 65535u * 64u;
  if (index >= frame.flags.z) { return; }
  let t = prepare(index);
  if (phase_two(index, t) == 1u) { emit_list_b(index, t.lod); }
}
