// Hand-written stage code: vertex layout, buffers and the quad.
// The light itself comes from ch14_glow.coax.
struct Dot { float4 a; };   // x, y, radius, light
struct View { float2 size; };
struct DotOut { float4 position [[position]]; float2 uv; float light; };

constant float2 corners[6] = {{-1,-1},{1,-1},{-1,1},{-1,1},{1,-1},{1,1}};

vertex DotOut dot_vertex(uint id [[vertex_id]], uint inst [[instance_id]],
                         const device Dot* dots [[buffer(0)]],
                         constant View& v [[buffer(1)]]) {
  Dot d = dots[inst];
  float2 uv = corners[id];
  float2 p = d.a.xy + uv * d.a.z;
  return {float4(p.x / v.size.x * 2 - 1, 1 - p.y / v.size.y * 2, 0, 1), uv, d.a.w};
}

fragment float4 dot_fragment(DotOut in [[stage_in]]) {
  float c = glow_tone(glow_dot(in.uv.x, in.uv.y, in.light));
  return float4(c, c, c, 1);
}
