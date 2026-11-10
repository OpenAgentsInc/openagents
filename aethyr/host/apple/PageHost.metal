// Firm reason: 6 (Metal glue only, as coax/web/host.js's SHADOW_CLEAR_WGSL and MEDIA_WGSL are: no
// product decision). A shadow tile's clear, a picture's box, and the page's picture onto the view.
#include <metal_stdlib>
using namespace metal;

struct PhOut { float4 p [[position]]; float2 uv; };
// One triangle over the viewport at depth 1.
vertex float4 ph_clear_vs(uint i [[vertex_id]]) { return float4(float(i & 1u) * 4.0 - 1.0, float(i >> 1u) * 4.0 - 1.0, 1.0, 1.0); }
// m[0] the box in clip space (left, top, right, bottom); m[1] x a gain, y and z the picture's top and bottom.
vertex PhOut ph_box_vs(uint i [[vertex_id]], constant float4 *m [[buffer(0)]]) {
  const float2 c[6] = { float2(0, 0), float2(1, 0), float2(0, 1), float2(0, 1), float2(1, 0), float2(1, 1) };
  float2 k = c[i];
  PhOut o; o.p = float4(mix(m[0].x, m[0].z, k.x), mix(m[0].y, m[0].w, k.y), 0, 1); o.uv = k; return o;
}
fragment float4 ph_box_fs(PhOut o [[stage_in]], constant float4 *m [[buffer(0)]], texture2d<float> t [[texture(0)]], sampler s [[sampler(0)]]) {
  return float4(t.sample(s, float2(o.uv.x, mix(m[1].y, m[1].z, o.uv.y))).rgb * m[1].x, 1);
}
