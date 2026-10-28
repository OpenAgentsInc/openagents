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
// The Grid's cast (metareal/cast.coax): bind-pose vertices skinned by the
// joint palette Coax writes, through the view matrix metareal_grid_view
// writes. Cast.bin's vertex: position, normal, uv, four weights, four
// joints, a display tint and a texture slot (255: unlit).
struct CastVertex { packed_float3 p; packed_float3 n; float2 uv; float4 w; uchar4 j; uchar4 tint; float2 pad; };
struct Joint { float4 r0; float4 r1; float4 r2; };
struct CastView { float4 r0; float4 r1; float4 r2; float4 r3; float2 size; float scale; float pad; };
struct Skinned { float3 p; float3 n; };
Skinned skinned(CastVertex v, const device Joint* palette) {
  float4 p1=float4(float3(v.p),1);float3 n=float3(v.n);Skinned s={float3(0),float3(0)};
  for(int k=0;k<4;++k){
    if(v.w[k]<=0)continue;Joint m=palette[v.j[k]];
    s.p+=v.w[k]*float3(dot(m.r0,p1),dot(m.r1,p1),dot(m.r2,p1));
    s.n+=v.w[k]*float3(dot(m.r0.xyz,n),dot(m.r1.xyz,n),dot(m.r2.xyz,n));
  }
  return s;
}
float4 viewed(float3 p,constant CastView& v){float4 p1=float4(p,1);return float4(dot(v.r0,p1),dot(v.r1,p1),dot(v.r2,p1),dot(v.r3,p1));}
struct CastOut {float4 position [[position]];float3 n;float2 uv;float3 tint;float slot;};
vertex CastOut cast_vertex(uint id [[vertex_id]],const device CastVertex* vertices [[buffer(0)]],const device Joint* palette [[buffer(1)]],constant CastView& v [[buffer(2)]]) {
  CastVertex c=vertices[id];Skinned s=skinned(c,palette);
  return {viewed(s.p,v),s.n,c.uv,float3(c.tint.rgb)/255.,float(c.tint.a)};
}
fragment float4 cast_fragment(CastOut in [[stage_in]],texture2d_array<float> sheets [[texture(0)]]) {
  if(in.slot>250.)return float4(metareal_unlit(in.tint.r),metareal_unlit(in.tint.g),metareal_unlit(in.tint.b),1);
  constexpr sampler s(filter::linear,mip_filter::linear,address::repeat);
  float3 albedo=sheets.sample(s,in.uv,uint(in.slot+.5)).rgb;
  float light=metareal_lit(in.n.x,in.n.y,in.n.z);
  float3 t=float3(metareal_decode(in.tint.r),metareal_decode(in.tint.g),metareal_decode(in.tint.b));
  return float4(metareal_character(albedo.r*t.r,light),metareal_character(albedo.g*t.g,light),metareal_character(albedo.b*t.b,light),1);
}
// A robot's line art: each pair of indices a screen-space quad, depth tested
// against the faces, so lines behind the robot are hidden.
vertex LineOut cast_line_vertex(uint id [[vertex_id]],uint inst [[instance_id]],const device CastVertex* vertices [[buffer(0)]],const device Joint* palette [[buffer(1)]],constant CastView& v [[buffer(2)]],const device uint* pairs [[buffer(3)]]) {
  CastVertex va=vertices[pairs[inst*2]],vb=vertices[pairs[inst*2+1]];
  float4 a=viewed(skinned(va,palette).p,v),b=viewed(skinned(vb,palette).p,v);
  float2 uv=corners[id];float t=uv.x*.5+.5;
  if(a.w<.05||b.w<.05)return {float4(0,0,2,1),0,0,0,0};
  float2 sa=a.xy/a.w*v.size*.5,sb=b.xy/b.w*v.size*.5;
  float2 d=sb-sa;float2 n=float2(-d.y,d.x)/max(length(d),.001);
  float2 p=mix(sa,sb,t)+n*uv.y*1.1*v.scale*2.;
  float z=mix(a.z/a.w,b.z/b.w,t)-.0004;
  float light=metareal_unlit(va.tint.r/255.);
  return {float4(p/(v.size*.5),z,1),uv.y,t,light,light};
}
