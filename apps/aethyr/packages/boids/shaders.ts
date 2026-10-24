const common = /* wgsl */ `
struct View {
  viewProjection: mat4x4f,
  inverseViewProjection: mat4x4f,
  eyeTime: vec4f,
  viewport: vec4f,
  targetFog: vec4f,
  fish: vec4f,
}
@group(0) @binding(0) var<uniform> view: View;
fn toneMap(x: vec3f) -> vec3f {
  let a = x * (2.51 * x + vec3f(0.03));
  let b = x * (2.43 * x + vec3f(0.59)) + vec3f(0.14);
  return clamp(a / b, vec3f(0), vec3f(1));
}
fn waterColor(direction: vec3f) -> vec3f {
  let h = smoothstep(-0.7, 0.8, direction.y);
  return mix(vec3f(0.009, 0.115, 0.18), vec3f(0.025, 0.66, 0.78), h);
}
`;

export const fishShader =
  common +
  /* wgsl */ `
struct Boid { position: vec4f, velocity: vec4f }
@group(1) @binding(0) var<storage, read> state: array<Boid>;
struct FishIn {
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) surface: vec2f,
  @builtin(instance_index) instance: u32,
}
struct FishOut {
  @builtin(position) clip: vec4f,
  @location(0) position: vec3f,
  @location(1) normal: vec3f,
  @location(2) surface: vec2f,
  @location(3) @interpolate(flat) colorSeed: f32,
  @location(4) local: vec3f,
}
@vertex fn vertexMain(input: FishIn) -> FishOut {
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
  let size = view.fish.x * mix(0.8, 1.15, seed);
  let world = boid.position.xyz + basis * local * size;
  var output: FishOut;
  output.clip = view.viewProjection * vec4f(world, 1);
  output.position = world;
  output.normal = basis * normalize(input.normal + vec3f(0, 0, bend * 0.16));
  output.surface = vec2f(input.surface.x, input.position.y / 0.27);
  output.colorSeed = seed;
  output.local = input.position;
  return output;
}
fn ggx(normal: vec3f, toEye: vec3f, toLight: vec3f, color: vec3f, roughness: f32) -> vec3f {
  let h = normalize(toEye + toLight);
  let nl = max(dot(normal, toLight), 0.0);
  let nv = max(dot(normal, toEye), 0.001);
  let nh = max(dot(normal, h), 0.0);
  let hv = max(dot(h, toEye), 0.0);
  let a2 = pow(roughness, 4.0);
  let d0 = nh * nh * (a2 - 1.0) + 1.0;
  let d = a2 / max(3.14159265 * d0 * d0, 0.00001);
  let k = pow(roughness + 1.0, 2.0) * 0.125;
  let g = (nv / (nv * (1.0 - k) + k)) * (nl / (nl * (1.0 - k) + k));
  let f0 = mix(vec3f(0.04), color, 0.6);
  let f = f0 + (vec3f(1) - f0) * pow(1.0 - hv, 5.0);
  return ((vec3f(1) - f) * color * 0.4 / 3.14159265 + f * d * g / max(4.0 * nv * nl, 0.001)) * nl;
}
@fragment fn fragmentMain(input: FishOut) -> @location(0) vec4f {
  let normal = normalize(input.normal);
  let toEye = normalize(view.eyeTime.xyz - input.position);
  let silver = mix(vec3f(0.19, 0.42, 0.46), vec3f(0.63, 0.74, 0.68), input.colorSeed);
  let body = mix(silver * 0.35, silver, smoothstep(-0.4, 0.5, -input.surface.y));
  let finColor = mix(body, vec3f(0.13, 0.3, 0.31), input.surface.x * 0.55);
  let eyeDistance = length((input.local.xy - vec2f(0.59, 0.06)) * vec2f(1.0, 1.2));
  let eyeMask = (1.0 - smoothstep(0.035, 0.055, eyeDistance)) * smoothstep(0.07, 0.10, abs(input.local.z));
  let color = mix(finColor, vec3f(0.006, 0.009, 0.01), eyeMask);
  let sun = normalize(vec3f(-0.3, 0.87, 0.38));
  let ambient = color * (vec3f(0.06, 0.21, 0.25) + vec3f(0.16, 0.34, 0.37) * max(normal.y, 0.0));
  let direct = ggx(normal, toEye, sun, color, 0.32) * vec3f(2.4, 3.5, 3.7);
  let rim = pow(1.0 - max(dot(normal, toEye), 0.0), 3.0) * vec3f(0.015, 0.13, 0.15);
  let distanceToEye = distance(view.eyeTime.xyz, input.position);
  let transmission = exp(-distanceToEye * view.targetFog.w);
  let water = waterColor(-toEye);
  return vec4f(toneMap(mix(water, ambient + direct + rim, transmission)), 1);
}
`;

export const waterShader =
  common +
  /* wgsl */ `
struct WaterOut { @builtin(position) position: vec4f, @location(0) uv: vec2f }
@vertex fn vertexMain(@builtin(vertex_index) index: u32) -> WaterOut {
  let p = array<vec2f, 3>(vec2f(-1, -1), vec2f(3, -1), vec2f(-1, 3))[index];
  var output: WaterOut;
  output.position = vec4f(p, 0.99999, 1);
  output.uv = p;
  return output;
}
fn hash(p: vec2f) -> f32 { return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453); }
@fragment fn fragmentMain(input: WaterOut) -> @location(0) vec4f {
  let far = view.inverseViewProjection * vec4f(input.uv, 1, 1);
  let direction = normalize(far.xyz / far.w - view.eyeTime.xyz);
  var water = waterColor(direction);
  let sun = normalize(vec3f(-0.3, 0.87, 0.38));
  water += vec3f(0.14, 0.3, 0.27) * pow(max(dot(direction, sun), 0.0), 24.0);
  let shafts = pow(max(sin(direction.x * 25.0 + direction.z * 19.0 + view.eyeTime.w * 0.08), 0.0), 18.0);
  water += vec3f(0.025, 0.09, 0.085) * shafts * smoothstep(-0.25, 0.7, direction.y);
  if (direction.y < -0.0001) {
    let d = (-27.0 - view.eyeTime.y) / direction.y;
    let p = view.eyeTime.xyz + direction * d;
    let cells = floor(p.xz * 0.25);
    let sand = mix(vec3f(0.11, 0.16, 0.13), vec3f(0.24, 0.27, 0.2), hash(cells));
    let caustics = pow(0.5 + 0.5 * sin(p.x * 0.9 + sin(p.z * 0.6 + view.eyeTime.w * 0.3)), 7.0);
    let floorColor = sand * (0.8 + caustics * 0.65);
    water = mix(water, floorColor, exp(-d * 0.027));
    // A few analytic stones keep the original environment asset-free.
    for (var i = 0; i < 5; i++) {
      let fi = f32(i);
      let center = vec3f(-23.0 + fi * 11.0, -27.0 + 1.1, -3.0 + sin(fi * 2.3) * 14.0);
      let radius = 1.4 + fract(fi * 0.73) * 1.5;
      let oc = view.eyeTime.xyz - center;
      let b = dot(oc, direction);
      let c = dot(oc, oc) - radius * radius;
      let h = b * b - c;
      if (h > 0.0) {
        let t = -b - sqrt(h);
        if (t > 0.0 && t < d) {
          let n = normalize(view.eyeTime.xyz + direction * t - center);
          let stone = vec3f(0.13, 0.19, 0.17) * (0.3 + 0.7 * max(dot(n, sun), 0.0));
          water = mix(waterColor(direction), stone, exp(-t * 0.027));
        }
      }
    }
  }
  return vec4f(toneMap(water), 1);
}
`;
