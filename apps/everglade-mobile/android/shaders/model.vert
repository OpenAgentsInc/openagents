#version 450
// Everglade for Android: your model. apps/everglade-desktop/Model.metal model_vs for Vulkan, which
// is itself the web's vertex math (everglade/scene/gpu/fivelab.coax); keep them in step.
layout(set = 0, binding = 0) uniform MU { vec4 eye; vec4 right; vec4 up; vec4 forward; vec2 size; vec2 pad; } u;
layout(set = 0, binding = 1) readonly buffer Vertices { uint v[]; } vertices;
struct Obj { vec4 a; vec4 b; vec4 c; vec4 d; vec4 e; vec4 f; };
layout(set = 0, binding = 2) readonly buffer Objects { Obj o[]; } objects;
layout(set = 0, binding = 3) readonly buffer Bake { vec4 r[]; } bake;
layout(location = 0) out vec3 world;
layout(location = 1) out vec3 normal;
layout(location = 2) out vec2 uv;
layout(location = 3) flat out uint slice;
layout(location = 4) flat out vec3 tint;
layout(location = 5) flat out vec4 dye;
layout(location = 6) flat out vec4 eyes;

void skin(uint row, uint joints, uint weights, vec4 at, vec3 nw, out vec3 p, out vec3 n) {
  p = vec3(0.0); n = vec3(0.0);
  for (uint k = 0u; k < 4u; k++) {
    float w = float((weights >> (k * 8u)) & 255u) / 255.0;
    if (w > 0.0) {
      uint r = row + ((joints >> (k * 8u)) & 255u) * 3u;
      vec4 a = bake.r[r], c = bake.r[r + 1u], e = bake.r[r + 2u];
      p += vec3(dot(a, at), dot(c, at), dot(e, at)) * w;
      n += vec3(dot(a.xyz, nw), dot(c.xyz, nw), dot(e.xyz, nw)) * w;
    }
  }
}

void main() {
  Obj m = objects.o[gl_InstanceIndex];
  uint b = uint(m.b.w) + uint(gl_VertexIndex) * 6u;
  uint xyw = vertices.v[b], zw = vertices.v[b + 1u], nrm = vertices.v[b + 2u], uvw = vertices.v[b + 3u];
  vec3 local = vec3(float(xyw & 0xffffu), float(xyw >> 16), float(zw & 0xffffu)) / 65535.0 * 2.5 + vec3(-1.25, -0.25, -1.25);
  vec4 at = vec4(local, 1.0);
  vec3 nw = clamp(vec3(float(int(nrm << 24) >> 24), float(int(nrm << 16) >> 24), float(int(nrm << 8) >> 24)) / 127.0, -1.0, 1.0);
  uint joints = vertices.v[b + 4u], weights = vertices.v[b + 5u];
  vec3 sp, sn;
  skin(uint(m.b.x), joints, weights, at, nw, sp, sn);
  if (m.b.z > 0.001 && m.b.z < 1.5) {
    vec3 op, on;
    skin(uint(m.b.y), joints, weights, at, nw, op, on);
    sp = mix(sp, op, m.b.z); sn = mix(sn, on, m.b.z);
  }
  float c = cos(m.a.w), s = sin(m.a.w);
  vec3 w = vec3(sp.x * c + sp.z * s + m.a.x, sp.y + m.a.y, -sp.x * s + sp.z * c + m.a.z);
  vec3 e = w - u.eye.xyz;
  float x = dot(e, u.right.xyz), y = dot(e, u.up.xyz), z = dot(e, u.forward.xyz);
  const float near = 0.1, far = 200.0;
  gl_Position = vec4(x * u.forward.w * 2.0 / u.size.x, y * u.forward.w * 2.0 / u.size.y, far * (z - near) / (far - near), z);
  world = w;
  normal = vec3(sn.x * c + sn.z * s, sn.y, -sn.x * s + sn.z * c);
  uv = vec2(float(uvw & 0xffffu), float(uvw >> 16)) / 65535.0;
  uint kind = (zw >> 24) & 255u;
  slice = (zw >> 16) & 255u;
  tint = vec3(1.0); dye = vec4(0.0);
  if (kind == 1u) { tint = m.c.xyz; }
  if (kind == 2u) { tint = m.d.xyz; }
  if (kind == 3u || kind == 4u) { slice += uint(m.c.w); dye = vec4(m.e.xyz, float(kind - 2u) * 2.0 + m.e.w); }
  eyes = m.f;
}
