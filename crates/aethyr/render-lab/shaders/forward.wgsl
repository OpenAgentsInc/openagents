// Forward shading for the CPU-batched and GPU-driven paths. The per-instance
// list entry arrives as an instance-step vertex attribute, so no draw needs a
// nonzero firstInstance (the indirect-first-instance feature stays optional).
// MATERIAL_FROM_GROUP_1 selects the CPU path's per-draw material binding.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> instances: array<Instance>;
@group(0) @binding(2) var<storage, read> materials: array<Material>;
@group(0) @binding(3) var textures: texture_2d_array<f32>;
@group(0) @binding(4) var linear_sampler: sampler;
//MATERIAL_GROUP

struct VsIn {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) instance: u32,
}

struct VsOut {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) uv: vec2f,
  @location(3) @interpolate(flat) material: u32,
}

@vertex
fn vs_main(input: VsIn) -> VsOut {
  let inst = instances[input.instance];
  let world = instance_point(inst, input.position);
  var out: VsOut;
  out.clip = frame.view_proj * vec4f(world, 1.0);
  out.world = world;
  out.normal = instance_normal(inst, input.normal);
  out.uv = input.uv;
  out.material = inst.material;
  return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4f {
  //MATERIAL_SELECT
  let m = materials[material];
  let texel = textureSample(textures, linear_sampler, input.uv * m.uv_scale, m.layer).rgb;
  let color = shade(frame, m.color * texel, input.normal, input.world, m.roughness);
  return vec4f(color, 1.0);
}

// Sky: drawn first in the main pass, at the far plane without depth writes,
// so geometry covers it. The visibility buffer path paints sky in resolve.
struct SkyOut {
  @builtin(position) clip: vec4f,
  @location(0) ndc: vec2f,
}

@vertex
fn sky_vs(@builtin(vertex_index) vi: u32) -> SkyOut {
  let p = vec2f(f32((vi << 1u) & 2u), f32(vi & 2u)) * 2.0 - 1.0;
  var out: SkyOut;
  out.clip = vec4f(p, 1.0, 1.0);
  out.ndc = p;
  return out;
}

@fragment
fn sky_fs(input: SkyOut) -> @location(0) vec4f {
  let far = frame.inv_view_proj * vec4f(input.ndc - frame.jitter.xy, 1.0, 1.0);
  let dir = normalize(far.xyz / far.w - frame.camera.xyz);
  return vec4f(sky_color(dir, frame.sun), 1.0);
}
