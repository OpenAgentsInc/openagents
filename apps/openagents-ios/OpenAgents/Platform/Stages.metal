// Native Metal stage bindings. All scalar visual math is generated from Coax.
struct View { float2 size; float time; float scale; };
struct Dot { float4 a; float4 b; };
struct Line { float4 a; float4 b; };
struct Bar { float4 rect; float4 state; };
struct ScreenOut { float4 position [[position]]; float2 uv; };
constant float2 corners[6]={{-1,-1},{1,-1},{-1,1},{-1,1},{1,-1},{1,1}};
float4 clip(float2 p,float2 size){return float4(p.x/size.x*2-1,1-p.y/size.y*2,0,1);}
vertex ScreenOut screen_vertex(uint id [[vertex_id]]) {
  float2 p=corners[id];return {float4(p,0,1),float2(p.x*.5+.5,.5-p.y*.5)};
}
fragment float4 sky_fragment(ScreenOut in [[stage_in]],constant View& v [[buffer(0)]]) {
  float2 p=(in.uv-.5)*float2(v.size.x/v.size.y,1);
  return float4(float3(metareal_sky(p.x,p.y)),1);
}
struct DotOut {float4 position [[position]];float2 uv;float light;float soft;float crisp;};
vertex DotOut dot_vertex(uint id [[vertex_id]],uint inst [[instance_id]],const device Dot* dots [[buffer(0)]],constant View& v [[buffer(1)]]) {
  Dot d=dots[inst];float2 uv=corners[id];
  // Sizes in the scene describe luminous cores; the quad includes the halo.
  float radius=max(1.5,d.a.z);
  return {clip(d.a.xy+uv*radius,v.size),uv,d.a.w,d.b.x,d.b.y};
}
fragment float4 dot_fragment(DotOut in [[stage_in]]) {float c=metareal_dot(in.uv.x,in.uv.y,in.light,in.soft,in.crisp);return float4(c,c,c,0);}
struct LineOut {float4 position [[position]];float across;float along;float a;float b;};
vertex LineOut line_vertex(uint id [[vertex_id]],uint inst [[instance_id]],const device Line* lines [[buffer(0)]],constant View& v [[buffer(1)]]) {
  Line l=lines[inst];float2 uv=corners[id];float t=uv.x*.5+.5;
  float2 d=l.a.zw-l.a.xy;float2 n=float2(-d.y,d.x)/max(length(d),.001);
  return {clip(mix(l.a.xy,l.a.zw,t)+n*uv.y*max(1.,l.b.z)*2.,v.size),uv.y,t,l.b.x,l.b.y};
}
fragment float4 line_fragment(LineOut in [[stage_in]]) {float c=metareal_line(in.across,in.along,in.a,in.b);return float4(c,c,c,0);}
fragment float4 bloom_fragment(ScreenOut in [[stage_in]],texture2d<float> image [[texture(0)]],constant float2& direction [[buffer(0)]]) {
  constexpr sampler s(filter::linear,address::clamp_to_edge);float2 uv=in.uv;
  float3 a=image.sample(s,uv).rgb;
  float3 b=image.sample(s,uv+direction*1.4).rgb+image.sample(s,uv-direction*1.4).rgb;
  float3 c=image.sample(s,uv+direction*3.2).rgb+image.sample(s,uv-direction*3.2).rgb;
  return float4(metareal_bloom(a.r,b.r,c.r),metareal_bloom(a.g,b.g,c.g),metareal_bloom(a.b,b.b,c.b),1);
}
fragment float4 composite_fragment(ScreenOut in [[stage_in]],texture2d<float> scene [[texture(0)]],texture2d<float> bloom [[texture(1)]]) {
  constexpr sampler s(filter::linear,address::clamp_to_edge);
  float3 c=scene.sample(s,in.uv).rgb+bloom.sample(s,in.uv).rgb*.85;
  return float4(metareal_tone(c.r),metareal_tone(c.g),metareal_tone(c.b),1);
}
struct BarOut {float4 position [[position]];float2 p;};
vertex BarOut bar_vertex(uint id [[vertex_id]],constant View& v [[buffer(0)]],constant Bar& b [[buffer(1)]]) {
  float2 p=corners[id]*(b.rect.zw*.5+20*v.scale);
  return {clip(b.rect.xy+b.rect.zw*.5+p,v.size),p};
}
fragment float4 bar_fragment(BarOut in [[stage_in]],constant View& v [[buffer(0)]],constant Bar& b [[buffer(1)]],texture2d<float> label [[texture(0)]]) {
  float2 p=in.p/(1-.03*b.state.x);float2 ext=b.rect.zw*.5;
  float c=metareal_bar(p.x,p.y,ext.x,ext.y,v.scale,v.time,b.state.x,b.state.y);
  constexpr sampler s(filter::linear,address::clamp_to_zero);
  float2 uv=p/float2(label.get_width(),label.get_height())+.5;
  c+=metareal_label(label.sample(s,uv).r,b.state.y);
  return float4(float3(c),clamp(c,0.,1.));
}
