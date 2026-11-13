// Everglade for desktop: what is drawn over the place (Temple.metal) and your model (Model.metal):
// the Metaverse Engine's twelve-float strokes (aethyr/ui/draw.coax `lit`), a core and a halo, as
// apps/openagents-desktop/Shaders.metal lights them, and text from the faces' atlas (text.coax).
#include <metal_stdlib>
using namespace metal;

struct U { float2 size; float time; float pad; };

struct In { float4 a [[attribute(0)]]; float4 b [[attribute(1)]]; float4 c [[attribute(2)]]; };
struct V { float4 pos [[position]]; float2 place; float4 shape; float4 tint; };

vertex V fx_vs(In v [[stage_in]], constant U &u [[buffer(1)]]) {
  V o;
  o.pos = float4(v.a.x / u.size.x * 2.0 - 1.0, 1.0 - v.a.y / u.size.y * 2.0, 0.0, 1.0);
  o.place = v.a.zw; o.shape = v.b; o.tint = v.c;
  return o;
}
fragment float4 fx_fs(V v [[stage_in]], texture2d<float> faces [[texture(0)]], sampler sm [[sampler(0)]]) {
  // Text (text.coax): a glyph from the faces' atlas, Spectral in red (mode 4), Cinzel in green (5).
  // place is the atlas coordinate and shape.x the em in pixels, which picks the mip drawn nearest it.
  if (v.tint.w > 3.5) {
    float lod = clamp(log2(64.0 / max(v.shape.x, 1.0)), 0.0, 4.0);
    float4 t = faces.sample(sm, v.place, level(lod));
    float a = pow(v.tint.w > 4.5 ? t.g : t.r, 0.9);
    return float4(v.tint.rgb * a * v.shape.z, 1.0);
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
    return float4((v.tint.rgb * g + float3(1.0, 0.95, 0.86) * hot) * tail * v.shape.z, 1.0);
  }
  float core = 1.0 - smoothstep(v.shape.y - 0.5, v.shape.y + 0.6, d);
  float g = d / max(v.shape.w, 1.0);
  float halo = exp(-5.0 * g * g) * 0.28;
  if (mode > 0.5 && mode < 1.5) {
    float ends = smoothstep(0.0, 8.0, v.place.x) * smoothstep(0.0, 8.0, v.shape.x - v.place.x);
    return float4(v.tint.rgb * (core * 0.8 + halo) * ends * v.shape.z, 1.0);
  }
  if (mode > 2.5) {
    return float4((mix(v.tint.rgb, float3(1.0, 0.97, 0.92), 0.75) * core + v.tint.rgb * halo * 1.8) * v.shape.z, 1.0);
  }
  return float4(v.tint.rgb * (core + halo) * v.shape.z, 1.0);
}

// A dark box under the Triage panel's text, so it reads over the scene. rect: x, y, width, height in pixels.
struct RU { float4 rect; float2 size; float2 pad; };
vertex float4 rect_vs(uint i [[vertex_id]], constant RU &u [[buffer(0)]]) {
  const float2 c[6] = {float2(0, 0), float2(1, 0), float2(0, 1), float2(1, 0), float2(1, 1), float2(0, 1)};
  float2 p = u.rect.xy + c[i] * u.rect.zw;
  return float4(p.x / u.size.x * 2.0 - 1.0, 1.0 - p.y / u.size.y * 2.0, 0.0, 1.0);
}
fragment float4 rect_fs() { return float4(0.006, 0.01, 0.022, 0.93); }
