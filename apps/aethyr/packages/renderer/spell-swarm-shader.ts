import { SCENE_LINEAR_SCALE } from "./radiometry.ts";

/** The original full fish geometry becomes an opaque, luminous flocking spell. */
export const spellSwarmShader = /* wgsl */ `
struct View {
  viewProjection: mat4x4f,
  eyeTime: vec4f,
  centerSize: vec4f,
}
struct Boid { position: vec4f, velocity: vec4f }
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var<storage, read> state: array<Boid>;
struct VertexIn {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) surface: vec2f,
  @builtin(instance_index) instance: u32,
}
struct VertexOut {
  @builtin(position) clip: vec4f,
  @location(0) world: vec3f,
  @location(1) normal: vec3f,
  @location(2) surface: f32,
  @location(3) @interpolate(flat) seed: f32,
}
@vertex fn vertexMain(input: VertexIn) -> VertexOut {
  let boid = state[input.instance];
  let forward = normalize(boid.velocity.xyz + vec3f(0.000001, 0, 0));
  let side = normalize(cross(forward, vec3f(0, 1, 0)) + vec3f(0, 0, 0.000001));
  let up = normalize(cross(side, forward));
  let basis = mat3x3f(forward, up, side);
  let seed = fract(f32(input.instance) * 0.61803398875);
  let tailWeight = 1.0 - smoothstep(-1.25, 0.1, input.position.x);
  let bend = sin(view.eyeTime.w * 10.0 + seed * 6.283185 - input.position.x * 4.0) * tailWeight;
  var local = input.position;
  local.z += bend * 0.12;
  let size = view.centerSize.w * mix(0.8, 1.15, seed);
  let world = view.centerSize.xyz + boid.position.xyz + basis * local * size;
  var output: VertexOut;
  output.clip = view.viewProjection * vec4f(world, 1);
  output.world = world;
  output.normal = basis * normalize(input.normal + vec3f(0, 0, bend * 0.16));
  output.surface = input.surface.x;
  output.seed = seed;
  return output;
}
@fragment fn fragmentMain(input: VertexOut) -> @location(0) vec4f {
  let normal = normalize(input.normal);
  let toEye = normalize(view.eyeTime.xyz - input.world);
  let light = normalize(vec3f(-0.3, 0.87, 0.38));
  let halfVector = normalize(toEye + light);
  let color = mix(vec3f(0.08, 0.9, 1.0), vec3f(0.7, 0.3, 1.0), input.seed);
  let diffuse = 0.25 + max(dot(normal, light), 0.0) * 0.65;
  let specular = pow(max(dot(normal, halfVector), 0.0), 40.0);
  let rim = pow(1.0 - abs(dot(normal, toEye)), 3.0);
  let pulse = 0.8 + 0.2 * sin(view.eyeTime.w * 2.0 + input.seed * 6.283185);
  // Keep linear HDR values: the existing Town bloom and output passes finish them.
  let emission = color * (0.65 + rim * 1.3 + input.surface * 0.4) * pulse;
  return vec4f((color * diffuse + vec3f(specular * 0.7) + emission) * ${SCENE_LINEAR_SCALE}, 1);
}
`;

/** Fixed-size, per-boid preparation; the vertex path still uses every mesh vertex. */
export const spellSwarmOrientationShader = /* wgsl */ `
struct Boid { position: vec4f, velocity: vec4f }
struct Orientation { forwardSeed: vec4f, up: vec4f, side: vec4f }
@group(0) @binding(0) var<storage, read> state: array<Boid>;
@group(0) @binding(1) var<storage, read_write> orientations: array<Orientation>;
@compute @workgroup_size(128)
fn prepareOrientation(@builtin(global_invocation_id) id: vec3u) {
  if (id.x >= arrayLength(&state)) { return; }
  let boid = state[id.x];
  let forward = normalize(boid.velocity.xyz + vec3f(0.000001, 0, 0));
  let side = normalize(cross(forward, vec3f(0, 1, 0)) + vec3f(0, 0, 0.000001));
  let up = normalize(cross(side, forward));
  let seed = fract(f32(id.x) * 0.61803398875);
  orientations[id.x] = Orientation(vec4f(forward, seed), vec4f(up, 0), vec4f(side, 0));
}
`;

const originalOrientation = `  let forward = normalize(boid.velocity.xyz + vec3f(0.000001, 0, 0));
  let side = normalize(cross(forward, vec3f(0, 1, 0)) + vec3f(0, 0, 0.000001));
  let up = normalize(cross(side, forward));
  let basis = mat3x3f(forward, up, side);
  let seed = fract(f32(input.instance) * 0.61803398875);`;
const preparedOrientation = `  let orientation = orientations[input.instance];
  let basis = mat3x3f(orientation.forwardSeed.xyz, orientation.up.xyz, orientation.side.xyz);
  let seed = orientation.forwardSeed.w;`;

if (!spellSwarmShader.includes(originalOrientation))
  throw new Error(
    "The spell orientation experiment must preserve the original vertex path.",
  );

/** The baseline fragment, tail deformation, positions, and complete mesh are unchanged. */
export const preparedSpellSwarmShader = spellSwarmShader
  .replace(
    "@group(1) @binding(0) var<storage, read> state: array<Boid>;",
    `@group(1) @binding(0) var<storage, read> state: array<Boid>;
struct Orientation { forwardSeed: vec4f, up: vec4f, side: vec4f }
@group(1) @binding(1) var<storage, read> orientations: array<Orientation>;`,
  )
  .replace(originalOrientation, preparedOrientation);
