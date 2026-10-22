// Lantern Hall — the editable shader in Kiln's workspace.
// Try changing this tint. Save, then keep walking.
const LANTERN_TINT = vec3<f32>(1.0, 0.72, 0.36);

struct Scene {
  view_proj: mat4x4<f32>,
  eye_time: vec4<f32>,
  light: vec4<f32>,
}
@group(0) @binding(0) var<uniform> scene: Scene;

struct Vertex {
  @location(0) position: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) color: vec3<f32>,
  @location(3) glow: f32,
}
struct Surface {
  @builtin(position) position: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) color: vec3<f32>,
  @location(3) glow: f32,
}
@vertex fn vertex_main(v: Vertex) -> Surface {
  var out: Surface;
  out.position = scene.view_proj * vec4<f32>(v.position, 1.0);
  out.world = v.position;
  out.normal = v.normal;
  out.color = v.color;
  out.glow = v.glow;
  return out;
}
@fragment fn fragment_main(v: Surface) -> @location(0) vec4<f32> {
  let to_light = scene.light.xyz - v.world;
  let d = length(to_light);
  let flicker = 0.96 + 0.04 * sin(scene.eye_time.w * 5.0);
  let diffuse = max(dot(normalize(v.normal), normalize(to_light)), 0.0);
  let toon = select(0.38, 0.95, diffuse > 0.34);
  let lamp = flicker * 9.0 / (3.0 + d * d);
  let ambient = vec3<f32>(0.16, 0.19, 0.23);
  var color = v.color * (ambient + LANTERN_TINT * toon * lamp);
  color += v.color * v.glow * 1.8;
  let distance = length(scene.eye_time.xyz - v.world);
  let fog = 1.0 - exp(-distance * 0.035);
  color = mix(color, vec3<f32>(0.055, 0.075, 0.085), fog);
  let paper = fract(sin(dot(v.position.xy, vec2<f32>(12.9898, 78.233))) * 43758.5453);
  color += (paper - 0.5) * 0.018;
  color = pow(max(color, vec3<f32>(0.0)), vec3<f32>(0.8));
  return vec4<f32>(color, 1.0);
}
