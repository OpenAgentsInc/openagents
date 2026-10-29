// chat.wgsl's two passes in Metal (apps/openagents-web/chat.wgsl): the
// nebula with the glass of up to ten HUD panels, then the strokes, with the
// system sans from the host's atlas. Keep the two in step.
#include <metal_stdlib>
using namespace metal;

struct U {
  float2 size; float time; float fade; float still; float count; float pad0; float pad1;
  float4 panels[20];
};

vertex float4 bg_vs(uint i [[vertex_id]]) {
  const float2 p[3] = {float2(-1.0, -3.0), float2(-1.0, 1.0), float2(3.0, 1.0)};
  return float4(p[i], 0.0, 1.0);
}
static float hash(float2 p) { return fract(sin(dot(p, float2(127.1, 311.7))) * 43758.5453); }
static float noise(float2 p) {
  float2 i = floor(p); float2 f = fract(p); float2 s = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + float2(1.0, 0.0)), s.x), mix(hash(i + float2(0.0, 1.0)), hash(i + float2(1.0, 1.0)), s.x), s.y);
}
static float fbm(float2 q) {
  float2 p = q; float v = 0.0; float a = 0.5;
  for (int k = 0; k < 5; k++) { v += a * noise(p); p = p * 2.03 + float2(1.7, 9.2); a *= 0.5; }
  return v;
}
static float3 panel(float2 p, float3 under, float4 r, float4 s, constant U &u) {
  float alpha = s.y * u.fade;
  if (alpha <= 0.0) { return under; }
  float x = p.x - r.x; float y = p.y - r.y; float w = r.z - r.x; float h = r.w - r.y;
  float side = min(min(x, w - x), min(y, h - y));
  float corner = min((w - x) + y, x + (h - y)) - s.x;
  float inside = min(side, corner * 0.7071);
  if (inside <= 0.0) { return under; }
  float3 tint = float3(1.0, 0.55, 0.12);
  float3 shade = mix(float3(0.03, 0.022, 0.016), float3(0.01, 0.011, 0.016), clamp(y / max(h, 1.0), 0.0, 1.0));
  float line = fract(p.y / 3.0) < 0.34 ? 0.018 : 0.0;
  float edge = exp(-inside / 22.0) * 0.07;
  float3 glass = mix(under * 0.25, shade, 0.85) + float3(line) + tint * edge;
  return mix(under, glass, clamp(alpha, 0.0, 1.0) * smoothstep(0.0, 1.5, inside));
}
fragment float4 bg_fs(float4 pos [[position]], constant U &u [[buffer(0)]]) {
  float2 uv = (pos.xy - 0.5 * u.size) / u.size.y;
  float t = u.time * 0.03;
  float2 w = float2(fbm(uv * 1.4 + float2(t, -t)), fbm(uv * 1.4 + float2(-t, t * 0.7) + 4.0));
  float n = fbm(uv * 2.0 + 2.4 * w + float2(0.0, t));
  float glow = exp(-1.6 * dot(uv, uv));
  float3 tint = mix(float3(0.035, 0.038, 0.05), float3(0.13, 0.14, 0.18), n * n);
  float3 c = tint * glow * (0.25 + 0.75 * n) * 0.8;
  c += float3(0.06, 0.03, 0.008) * exp(-pow((uv.y - 0.08) * 6.0, 2.0)) * glow;
  float star = step(0.998, hash(floor(pos.xy * 0.5))) * (0.5 + 0.5 * sin(u.time * 1.3 + hash(floor(pos.xy)) * 40.0));
  c += float3(star * 0.35 * (1.0 - glow * 0.6));
  c = c * u.fade;
  for (int k = 0; k < 10; k++) {
    if (float(k) >= u.count) { break; }
    c = panel(pos.xy, c, u.panels[k * 2], u.panels[k * 2 + 1], u);
  }
  return float4(c, 1.0);
}

struct In { float4 a [[attribute(0)]]; float4 b [[attribute(1)]]; float4 c [[attribute(2)]]; };
struct V { float4 pos [[position]]; float2 place; float4 shape; float4 tint; };

vertex V fx_vs(In v [[stage_in]], constant U &u [[buffer(1)]]) {
  V o;
  o.pos = float4(v.a.x / u.size.x * 2.0 - 1.0, 1.0 - v.a.y / u.size.y * 2.0, 0.0, 1.0);
  o.place = v.a.zw; o.shape = v.b; o.tint = v.c;
  return o;
}
fragment float4 fx_fs(V v [[stage_in]], constant U &u [[buffer(0)]], texture2d<float> sans [[texture(0)]], sampler sm [[sampler(0)]]) {
  if (v.tint.w > 3.5) {
    float lod = clamp(log2(64.0 / max(v.shape.x, 1.0)), 0.0, 4.0);
    float a = pow(sans.sample(sm, v.place, level(lod)).r, 0.85);
    return float4(v.tint.rgb * a * min(v.shape.z * 1.15, 1.0) * u.fade, 1.0);
  }
  float along = max(max(-v.place.x, v.place.x - v.shape.x), 0.0);
  float d = length(float2(along, v.place.y));
  float mode = v.tint.w;
  if (mode > 1.5 && mode < 2.5) {
    float r = d / max(v.shape.w, 1.0);
    if (r >= 1.0) { return float4(0.0, 0.0, 0.0, 1.0); }
    float tail = v.shape.x > 1.0 ? mix(0.08, 1.0, clamp(v.place.x / v.shape.x, 0.0, 1.0)) : 1.0;
    float g = exp(-r * r * 5.0) * (1.0 - r);
    float hot = exp(-r * r * 36.0) * v.shape.y;
    return float4((v.tint.rgb * g + float3(1.0, 0.95, 0.86) * hot) * tail * v.shape.z * u.fade, 1.0);
  }
  float core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  float g = d / max(v.shape.w, 1.0);
  float halo = exp(-5.0 * g * g) * 0.28;
  if (mode > 0.5 && mode < 1.5) {
    float ends = smoothstep(0.0, 8.0, v.place.x) * smoothstep(0.0, 8.0, v.shape.x - v.place.x);
    return float4(v.tint.rgb * (core * 0.8 + halo) * ends * v.shape.z * u.fade, 1.0);
  }
  if (mode > 2.5) {
    return float4((mix(v.tint.rgb, float3(1.0, 0.97, 0.92), 0.75) * core + v.tint.rgb * halo * 1.8) * v.shape.z * u.fade, 1.0);
  }
  return float4(v.tint.rgb * (core + halo) * v.shape.z * u.fade, 1.0);
}
