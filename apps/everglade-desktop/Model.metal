// Everglade for desktop: your model. The vertex math is the web's (apps/openagents-web/gpu/
// fivelab.coax fivelab_scene_vs and fivelab_skin, with fivelab_dye and cc_player_fs's eyes): a
// 24-byte vertex pulled from the pack, skinned from the baked rows, placed by its record. The
// light is this chamber's own, not the glade's sky. Keep the vertex math in step with the web's.
#include <metal_stdlib>
using namespace metal;

// pad: the clock, then the place you are in (1 the temple, 2 the range).
struct MU { float4 eye; float4 right; float4 up; float4 forward; float2 size; float2 pad; };
struct Obj { float4 a; float4 b; float4 c; float4 d; float4 e; float4 f; };
struct MV {
  float4 pos [[position]]; float3 world; float3 normal; float2 uv;
  uint slice [[flat]]; float3 tint [[flat]]; float4 dye [[flat]]; float4 eyes [[flat]];
};
struct Skinned { float3 p; float3 n; };

static Skinned skin(uint row, uint joints, uint weights, float4 at, float3 nw, const device float4 *bake) {
  Skinned o; o.p = float3(0.0); o.n = float3(0.0);
  for (uint k = 0; k < 4; k++) {
    float w = float((weights >> (k * 8)) & 255) / 255.0;
    if (w > 0.0) {
      uint r = row + ((joints >> (k * 8)) & 255) * 3;
      float4 a = bake[r], c = bake[r + 1], e = bake[r + 2];
      o.p += float3(dot(a, at), dot(c, at), dot(e, at)) * w;
      o.n += float3(dot(a.xyz, nw), dot(c.xyz, nw), dot(e.xyz, nw)) * w;
    }
  }
  return o;
}

vertex MV model_vs(uint id [[vertex_id]], uint instance [[instance_id]], constant MU &u [[buffer(0)]],
                   const device uint *vertices [[buffer(1)]], const device Obj *objects [[buffer(2)]], const device float4 *bake [[buffer(3)]]) {
  Obj m = objects[instance];
  uint b = uint(m.b.w) + id * 6;
  uint xyw = vertices[b], zw = vertices[b + 1], nrm = vertices[b + 2], uvw = vertices[b + 3];
  float3 local = float3(float(xyw & 0xffff), float(xyw >> 16), float(zw & 0xffff)) / 65535.0 * 2.5 + float3(-1.25, -0.25, -1.25);
  float4 at = float4(local, 1.0);
  float3 nw = clamp(float3(float(int(nrm << 24) >> 24), float(int(nrm << 16) >> 24), float(int(nrm << 8) >> 24)) / 127.0, -1.0, 1.0);
  uint joints = vertices[b + 4], weights = vertices[b + 5];
  Skinned sk = skin(uint(m.b.x), joints, weights, at, nw, bake);
  if (m.b.z > 0.001 && m.b.z < 1.5) {
    Skinned other = skin(uint(m.b.y), joints, weights, at, nw, bake);
    sk.p = mix(sk.p, other.p, m.b.z); sk.n = mix(sk.n, other.n, m.b.z);
  }
  float c = cos(m.a.w), sn = sin(m.a.w);
  float3 w = float3(sk.p.x * c + sk.p.z * sn + m.a.x, sk.p.y + m.a.y, -sk.p.x * sn + sk.p.z * c + m.a.z);
  MV o;
  float3 e = w - u.eye.xyz;
  float x = dot(e, u.right.xyz), y = dot(e, u.up.xyz), z = dot(e, u.forward.xyz);
  // The same depth as the place you stand in writes (gpu/temple.coax temple_project, gpu/range.coax
  // range_project): the range's far plane is further. pad.y is the place: 1 the temple, 2 the range.
  const float near = 0.1; float far = u.pad.y > 1.5 ? 600.0 : 200.0;
  o.pos = float4(x * u.forward.w * 2.0 / u.size.x, y * u.forward.w * 2.0 / u.size.y, far * (z - near) / (far - near), z);
  o.world = w;
  o.normal = float3(sk.n.x * c + sk.n.z * sn, sk.n.y, -sk.n.x * sn + sk.n.z * c);
  o.uv = float2(float(uvw & 0xffff), float(uvw >> 16)) / 65535.0;
  uint kind = (zw >> 24) & 255;
  o.slice = (zw >> 16) & 255;
  o.tint = float3(1.0); o.dye = float4(0.0);
  if (kind == 1) { o.tint = m.c.xyz; }
  if (kind == 2) { o.tint = m.d.xyz; }
  if (kind == 3 || kind == 4) { o.slice += uint(m.c.w); o.dye = float4(m.e.xyz, float(kind - 2) * 2.0 + m.e.w); }
  o.eyes = m.f;
  return o;
}

static float luma(float3 c) { return dot(c, float3(0.2126, 0.7152, 0.0722)); }
static float3 encode(float3 c) { return pow(max(c, float3(0.0)), float3(1.0 / 2.2)); }
static float3 decode(float3 c) { return pow(max(c, float3(0.0)), float3(2.2)); }
// fivelab_dye: pale cloth (dye.w 2..4) or green cloth (4..6) takes the dye by its share.
static float3 dyed(float3 t, float4 dye) {
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

fragment float4 model_fs(MV s [[stage_in]], constant MU &u [[buffer(0)]], texture2d_array<float> sheets [[texture(0)]], sampler sm [[sampler(0)]]) {
  float4 tex = sheets.sample(sm, s.uv, s.slice);
  if (tex.a < 0.35) { discard_fragment(); }
  float3 albedo = decode(dyed(encode(tex.rgb), s.dye)) * decode(s.tint);
  if (s.eyes.w > 1.5 && s.slice == 32) {
    float iris = saturate((tex.r - max(tex.g, tex.b)) * 5.0);
    albedo = mix(albedo, decode(s.eyes.xyz) * clamp(luma(tex.rgb) * 3.0, 0.15, 1.3), iris);
  }
  float3 n = normalize(s.normal + float3(0.0, 0.00001, 0.0));
  float3 view = normalize(u.eye.xyz - s.world);
  // The chamber's light: the blue of the pylons from above, the sphere's ember from the middle,
  // and a broad fill from the camera's side so a face reads.
  float3 key = saturate(dot(n, normalize(float3(0.25, 0.9, 0.3)))) * float3(0.55, 0.72, 1.0) * 1.1;
  float3 toward = normalize(float3(0.0, 1.45, 0.0) - s.world);
  // The ember is the Workshop's (pad.y is the place: 1 the temple, 2 the range, which has no sphere).
  float3 ember = saturate(dot(n, toward)) * float3(1.0, 0.5, 0.16) * (2.2 / (1.0 + 0.25 * distance_squared(float3(0.0, 1.45, 0.0), s.world))) * (u.pad.y > 1.5 ? 0.0 : 1.0);
  float3 fill = saturate(dot(n, normalize(view + float3(0.0, 0.35, 0.0)))) * float3(0.85, 0.78, 0.7) * 0.75;
  float3 colour = albedo * (float3(0.1, 0.13, 0.2) + key + ember + fill);
  return float4(encode(colour), 1.0);
}
