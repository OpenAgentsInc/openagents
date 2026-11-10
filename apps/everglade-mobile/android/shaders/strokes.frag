#version 450
// Apps/everglade-desktop/Workshop.metal fx_fs, for Vulkan; keep the two in step.
layout(set = 0, binding = 0) uniform sampler2D faces;
layout(location = 0) in vec2 place;
layout(location = 1) in vec4 shape;
layout(location = 2) in vec4 tint;
layout(location = 0) out vec4 colour;
void main() {
  // Text (text.coax): a glyph from the faces' atlas, Spectral in red (mode 4), Cinzel in green (5).
  if (tint.w > 3.5) {
    float lod = clamp(log2(64.0 / max(shape.x, 1.0)), 0.0, 4.0);
    vec4 t = textureLod(faces, place, lod);
    float a = pow(tint.w > 4.5 ? t.g : t.r, 0.9);
    colour = vec4(tint.rgb * a * shape.z, 1.0);
    return;
  }
  float along = max(max(-place.x, place.x - shape.x), 0.0);
  float d = length(vec2(along, place.y));
  float mode = tint.w;
  if (mode > 1.5 && mode < 2.5) {
    float r = d / max(shape.w, 1.0);
    if (r >= 1.0) { colour = vec4(0.0, 0.0, 0.0, 1.0); return; }
    float tail = shape.x > 1.0 ? mix(0.08, 1.0, clamp(place.x / shape.x, 0.0, 1.0)) : 1.0;
    float g = exp(-r * r * 5.0) * (1.0 - r);
    float hot = exp(-r * r * 36.0) * shape.y;
    colour = vec4((tint.rgb * g + vec3(1.0, 0.95, 0.86) * hot) * tail * shape.z, 1.0);
    return;
  }
  float core = 1.0 - smoothstep(shape.y - 0.5, shape.y + 0.6, d);
  float g = d / max(shape.w, 1.0);
  float halo = exp(-5.0 * g * g) * 0.28;
  if (mode > 0.5 && mode < 1.5) {
    float ends = smoothstep(0.0, 8.0, place.x) * smoothstep(0.0, 8.0, shape.x - place.x);
    colour = vec4(tint.rgb * (core * 0.8 + halo) * ends * shape.z, 1.0);
    return;
  }
  if (mode > 2.5) {
    colour = vec4((mix(tint.rgb, vec3(1.0, 0.97, 0.92), 0.75) * core + tint.rgb * halo * 1.8) * shape.z, 1.0);
    return;
  }
  colour = vec4(tint.rgb * (core + halo) * shape.z, 1.0);
}
