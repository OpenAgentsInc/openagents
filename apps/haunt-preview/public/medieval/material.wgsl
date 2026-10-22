// Medieval Town Lab — the editable ground and interior material.
// Try changing this tint. Save, then keep walking.
const SKY_TINT = vec3<f32>(1.0, 0.95, 0.84);

struct Scene {
  view_proj: mat4x4<f32>,
  eye_time: vec4<f32>,
  light: vec4<f32>,
  tint: vec4<f32>,
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
  let sun = normalize(vec3<f32>(0.4, 0.8, 0.3));
  let daylight = 0.7 + 0.3 * max(dot(normalize(v.normal), sun), 0.0);
  var color = v.color * daylight * SKY_TINT * scene.tint.xyz;
  color += v.color * v.glow;
  let distance = length(scene.eye_time.xyz - v.world);
  let fog = 1.0 - exp(-distance * 0.004);
  color = mix(color, vec3<f32>(0.64, 0.76, 0.85), fog);
  let paper = fract(sin(dot(v.position.xy, vec2<f32>(12.9898, 78.233))) * 43758.5453);
  color += (paper - 0.5) * 0.018;
  color = pow(max(color, vec3<f32>(0.0)), vec3<f32>(0.8));
  return vec4<f32>(color, 1.0);
}
