#version 450
// apps/everglade-desktop/Model.metal model_fs for Vulkan; keep the two in step.
layout(set = 0, binding = 0) uniform MU { vec4 eye; vec4 right; vec4 up; vec4 forward; vec2 size; vec2 pad; } u;
layout(set = 0, binding = 4) uniform sampler2DArray sheets;
layout(location = 0) in vec3 world;
layout(location = 1) in vec3 normal;
layout(location = 2) in vec2 uv;
layout(location = 3) flat in uint slice;
layout(location = 4) flat in vec3 tint;
layout(location = 5) flat in vec4 dye;
layout(location = 6) flat in vec4 eyes;
layout(location = 0) out vec4 colour;

float luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
vec3 encode(vec3 c) { return pow(max(c, vec3(0.0)), vec3(1.0 / 2.2)); }
vec3 decode(vec3 c) { return pow(max(c, vec3(0.0)), vec3(2.2)); }
// fivelab_dye: pale cloth (dye.w 2..4) or green cloth (4..6) takes the dye by its share.
vec3 dyed(vec3 t, vec4 dye) {
  if (dye.w < 2.0 || dye.w >= 6.0) { return t; }
  bool green = dye.w >= 4.0;
  float share = clamp(dye.w - (green ? 4.0 : 2.0), 0.0, 1.0);
  float hi = max(t.x, max(t.y, t.z)), lo = min(t.x, min(t.y, t.z));
  float sat = (hi - lo) / max(hi, 0.0001);
  float mask = smoothstep(0.12, 0.3, hi) * (1.0 - smoothstep(0.22, 0.42, sat));
  float usual = 0.42;
  if (green) { mask = smoothstep(0.02, 0.06, t.y - max(t.x, t.z)); usual = 0.09; }
  return mix(t, dye.xyz * clamp(luma(t) / usual, 0.0, 1.6), mask * share);
}

void main() {
  vec4 tex = texture(sheets, vec3(uv, float(slice)));
  if (tex.a < 0.35) { discard; }
  vec3 albedo = decode(dyed(encode(tex.rgb), dye)) * decode(tint);
  if (eyes.w > 1.5 && slice == 32u) {
    float iris = clamp((tex.r - max(tex.g, tex.b)) * 5.0, 0.0, 1.0);
    albedo = mix(albedo, decode(eyes.xyz) * clamp(luma(tex.rgb) * 3.0, 0.15, 1.3), iris);
  }
  vec3 n = normalize(normal + vec3(0.0, 0.00001, 0.0));
  vec3 view = normalize(u.eye.xyz - world);
  // The chamber's light, as Model.metal has it.
  vec3 key = clamp(dot(n, normalize(vec3(0.25, 0.9, 0.3))), 0.0, 1.0) * vec3(0.55, 0.72, 1.0) * 1.1;
  vec3 off = vec3(0.0, 1.45, 0.0) - world;
  // The ember is the Workshop's (pad.y is the place: 1 the temple, 2 the range, which has no sphere).
  vec3 ember = clamp(dot(n, normalize(off)), 0.0, 1.0) * vec3(1.0, 0.5, 0.16) * (2.2 / (1.0 + 0.25 * dot(off, off))) * (u.pad.y > 1.5 ? 0.0 : 1.0);
  vec3 fill = clamp(dot(n, normalize(view + vec3(0.0, 0.35, 0.0))), 0.0, 1.0) * vec3(0.85, 0.78, 0.7) * 0.75;
  colour = vec4(encode(albedo * (vec3(0.1, 0.13, 0.2) + key + ember + fill)), 1.0);
}
