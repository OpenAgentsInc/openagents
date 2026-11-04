// Metal binding of Aethyr's twelve-float stroke stream (aethyr/ui/draw.coax).
// Premultiplied alpha lets the native HUD composite over the existing world.
#include <metal_stdlib>
using namespace metal;
struct In {float4 a [[attribute(0)]];float4 b [[attribute(1)]];float4 c [[attribute(2)]];};
struct V {float4 pos [[position]];float2 place;float4 shape;float4 tint;};
vertex V hud_vs(In v [[stage_in]],constant float2 &size [[buffer(1)]]) {
  V o;o.pos=float4(v.a.x/size.x*2-1,1-v.a.y/size.y*2,0,1);
  o.place=v.a.zw;o.shape=v.b;o.tint=v.c;return o;
}
fragment float4 hud_fs(V v [[stage_in]]) {
  float along=max(max(-v.place.x,v.place.x-v.shape.x),0.0);
  float d=length(float2(along,v.place.y));
  float core=1-smoothstep(v.shape.y-0.5,v.shape.y+0.6,d);
  float g=d/max(v.shape.w,1.0);
  float alpha=clamp((core+exp(-5*g*g)*0.28)*v.shape.z,0.0,1.0);
  return float4(v.tint.rgb*alpha,alpha);
}
