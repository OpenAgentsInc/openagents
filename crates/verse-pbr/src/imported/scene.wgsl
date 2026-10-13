// VERSE_SHARED_SHADING
// fog: rgb color, w density at the base height (1/m). fog_shape: base height
// (m), falloff with height (1/m), start distance (m), and opacity cap.
// water_control: what this tier's water draws (`verse_pbr::water::control`).
struct Frame { view:mat4x4<f32>,eye:vec4<f32>,ambient:vec4<f32>,fog:vec4<f32>,settings:vec4<f32>,lights:array<vec4<f32>,64>,shadow:array<mat4x4<f32>,24>,fog_shape:vec4<f32>,water_control:vec4<f32> };
struct Pose { model:mat4x4<f32>,params:vec4<f32>,bones:array<mat4x4<f32>,256> };
// shading: x the model (0 standard, 1 skin, 2 eye, 3 cornea, 4 hair,
// 5 cloth; verse_engine::material::Shading), yzw and shading_more.x its
// parameters.
struct Material { params:vec4<f32>,channels:vec4<f32>,emission:vec4<f32>,maps:vec4<f32>,shading:vec4<f32>,shading_more:vec4<f32> };
@group(0) @binding(0) var<uniform> frame:Frame;
@group(0) @binding(1) var shadows:texture_depth_2d_array;
@group(0) @binding(2) var shadow_sampler:sampler_comparison;
@group(0) @binding(3) var fire_noise:texture_3d<f32>;
@group(0) @binding(4) var fire_lut:texture_2d<f32>;
@group(0) @binding(5) var fire_sampler:sampler;
@group(1) @binding(0) var image:texture_2d<f32>;
@group(1) @binding(1) var tex_sampler:sampler;
@group(1) @binding(2) var<uniform> material:Material;
@group(1) @binding(3) var normal_image:texture_2d<f32>;
@group(1) @binding(4) var orm_image:texture_2d<f32>;
@group(1) @binding(5) var occlusion_image:texture_2d<f32>;
@group(1) @binding(6) var emission_image:texture_2d<f32>;
@group(2) @binding(0) var<uniform> pose:Pose;
struct In { @location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) joints:vec4<u32>,@location(4) weights:vec4<f32>,@location(5) tint:vec3<f32> };
struct Out { @builtin(position) clip:vec4<f32>,@location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) tint:vec3<f32> };
@vertex fn vs(v:In)->Out {
 let skin=pose.bones[v.joints.x]*v.weights.x+pose.bones[v.joints.y]*v.weights.y+pose.bones[v.joints.z]*v.weights.z+pose.bones[v.joints.w]*v.weights.w;
 if pose.params.x>1.5 && pose.params.x<2.5 {
  let center=pose.model[3].xyz;
  let forward=normalize(frame.eye.xyz-center);
  let reference=select(vec3(0.0,1.0,0.0),vec3(0.0,0.0,1.0),abs(forward.y)>0.99);
  let right=normalize(cross(reference,forward));
  let up=cross(forward,right);
  let world=center+right*v.pos.x*length(pose.model[0].xyz)+up*v.pos.y*length(pose.model[1].xyz);
  var o:Out;o.clip=frame.view*vec4(world,1.0);o.pos=world;o.normal=forward;o.uv=v.uv;o.tint=v.tint;return o;
 }
 if pose.params.x>3.5 && pose.params.x<4.5 {
  let center=pose.model[3].xyz;let axis=pose.model[1].xyz;
  let forward=normalize(frame.eye.xyz-center);
  let tangent=normalize(axis);let cross_axis=cross(tangent,forward);
  let reference=select(vec3(1.0,0.0,0.0),vec3(0.0,1.0,0.0),abs(tangent.x)>0.99);
  let side=select(cross(tangent,reference),cross_axis,length(cross_axis)>0.001);
  let world=center+normalize(side)*v.pos.x*length(pose.model[0].xyz)+axis*v.pos.y;
  var o:Out;o.clip=frame.view*vec4(world,1.0);o.pos=world;o.normal=forward;o.uv=v.uv;o.tint=v.tint;return o;
 }
 let model=pose.model*skin;let world=model*vec4(v.pos,1.0);var o:Out;o.clip=frame.view*world;o.pos=world.xyz;o.normal=normalize((model*vec4(v.normal,0.0)).xyz);o.uv=v.uv;o.tint=v.tint;return o;
}
@fragment fn shadow_fs(v:Out){
 if material.params.y==1.0 && textureSample(image,tex_sampler,v.uv).a*material.channels.z<material.channels.w {discard;}
}
fn occlusion(index:u32,p:vec3<f32>,normal:vec3<f32>)->f32{
 if index>=u32(frame.settings.z){return 1.0;}
 let d=p-frame.lights[index*2u].xyz;let a=abs(d);var face=0u;
 if a.x>=a.y && a.x>=a.z {face=select(1u,0u,d.x>=0.0);}else if a.y>=a.z{face=select(3u,2u,d.y>=0.0);}else{face=select(5u,4u,d.z>=0.0);}
 let layer=index*6u+face;let q=frame.shadow[layer]*vec4(p+normal*0.025,1.0);let ndc=q.xyz/q.w;let uv=ndc.xy*vec2(0.5,-0.5)+vec2(0.5);
 if ndc.z>1.0||ndc.z<0.0{return 1.0;}
 var visibility=0.0;
 for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){visibility+=textureSampleCompareLevel(shadows,shadow_sampler,uv+vec2(f32(x),f32(y))/512.0,i32(layer),ndc.z-0.001);}}
 return visibility/9.0;
}
// Derive a cotangent frame from the posed surface and UV gradients. Degenerate
// UVs retain the geometric normal instead of producing an undefined direction.
fn surface_normal(v:Out,geometric:vec3<f32>)->vec3<f32>{
 if material.maps.x<0.5{return geometric;}
 let px=dpdx(v.pos);let py=dpdy(v.pos);let ux=dpdx(v.uv);let uy=dpdy(v.uv);
 let a=cross(py,geometric);let b=cross(geometric,px);
 let tangent=a*ux.x+b*uy.x;let bitangent=a*ux.y+b*uy.y;
 let length_squared=max(dot(tangent,tangent),dot(bitangent,bitangent));
 let sample=textureSample(normal_image,tex_sampler,v.uv).xyz*2.0-1.0;
 let mapped=sample*vec3(material.channels.x,material.channels.x,1.0);
 let scale=inverseSqrt(max(length_squared,0.000000000001));
 let world=tangent*scale*mapped.x+bitangent*scale*mapped.y+geometric*mapped.z;
 if length_squared<0.000000000001 || dot(world,world)<0.000000000001{return geometric;}
 return normalize(world);
}
// Independently implemented GGX distribution, correlated Smith visibility, and
// Schlick Fresnel. Point strengths retain the chamber's Lambert normalization.
fn reflectance(n:vec3<f32>,view:vec3<f32>,light:vec3<f32>,diffuse_color:vec3<f32>,f0:vec3<f32>,roughness:f32,nv:f32)->vec3<f32>{
 let nl=max(dot(n,light),0.0);
 // At unit roughness, GGX and Smith simplify exactly. The chamber's
 // broad matte surfaces avoid constructing a half vector for every light.
 if roughness==1.0 {
  let vh=sqrt(clamp((1.0+dot(view,light))*0.5,0.0,1.0));
  let grazing=1.0-vh;let g2=grazing*grazing;
  let fresnel=f0+(vec3(1.0)-f0)*(g2*g2*grazing);
  return ((vec3(1.0)-fresnel)*diffuse_color+fresnel*0.5/max(nl+nv,0.00001))*nl;
 }
 let sum=view+light;let half=sum/max(length(sum),0.0001);
 let nh=max(dot(n,half),0.0);let vh=clamp(dot(view,half),0.0,1.0);
 let alpha=roughness*roughness;let a2=alpha*alpha;
 let denominator=nh*nh*(a2-1.0)+1.0;
 let distribution=a2/(3.14159265*denominator*denominator);
 let lambda_v=nl*sqrt(nv*nv*(1.0-a2)+a2);
 let lambda_l=nv*sqrt(nl*nl*(1.0-a2)+a2);
 let visibility=0.5/max(lambda_v+lambda_l,0.00001);

 let grazing=1.0-vh;let g2=grazing*grazing;
 let fresnel=f0+(vec3(1.0)-f0)*(g2*g2*grazing);
 let diffuse=(vec3(1.0)-fresnel)*diffuse_color/3.14159265;
 return (diffuse+distribution*visibility*fresnel)*nl*3.14159265;
}
// The character shading models (docs/verse/draconia/metahuman-study.md).
// Each returns the light reflected toward the eye for unit radiance from
// direction l, with the shadow term `shadow` (0..1) folded in, so skin
// can bleed light into its own penumbra.
fn skin_light(n:vec3<f32>,soft:vec3<f32>,view:vec3<f32>,l:vec3<f32>,albedo:vec3<f32>,roughness:f32,nv:f32,shadow:f32,curvature:f32)->vec3<f32>{
 let sss=material.shading.y;let lobe_mix=material.shading.z;
 // Scattered light wraps past the terminator, most in red: wrapped
 // diffuse per channel on the softened normal (red) and the detail normal
 // (green, blue), normalized so a wrap loses no energy.
 // Pre-integrated scattering (after Penner): light wraps further where the
 // surface curves tightly (nose, lids, ears) than on the broad cheek, per
 // channel by skin's mean free path (red ~3.5 mm, green 1.4, blue 0.7).
 let w=clamp(vec3(0.0035,0.0014,0.0007)*curvature*1.6+vec3(0.14,0.06,0.035),vec3(0.0),vec3(0.9))*sss;
 let nl_soft=dot(soft,l);let nl=dot(n,l);
 let nl3=vec3(nl_soft,mix(nl_soft,nl,0.6),nl);
 let diffuse=clamp((nl3+w)/((vec3(1.0)+w)*(vec3(1.0)+w)),vec3(0.0),vec3(1.0));
 // The penumbra glows red: the shadow softens per channel.
 let s3=vec3(pow(shadow,mix(1.0,0.45,sss)),pow(shadow,mix(1.0,0.8,sss)),shadow);
 // Two GGX lobes (0.75x and 1.3x roughness), an oily sheen over skin.
 let f0=vec3(0.028);
 let spec=lobe_mix*reflectance(n,view,l,vec3(0.0),f0,clamp(roughness*0.75,0.07,1.0),nv)
   +(1.0-lobe_mix)*reflectance(n,view,l,vec3(0.0),f0,clamp(roughness*1.3,0.07,1.0),nv);
 return albedo*diffuse*s3+spec*material.shading.w*shadow;
}
fn hair_light(t:vec3<f32>,n:vec3<f32>,view:vec3<f32>,l:vec3<f32>,albedo:vec3<f32>,roughness:f32,shadow:f32)->vec3<f32>{
 // Kajiya-Kay diffuse with a soft wrap (strands are thin and scatter),
 // two shifted Marschner-like lobes along the strand, and transmission
 // for backlit hair.
 let tl=dot(t,l);let sin_tl=sqrt(max(1.0-tl*tl,0.0));
 let wrap=clamp(dot(n,l)*0.5+0.5,0.0,1.0);
 let diffuse=albedo*mix(0.3,1.0,sin_tl)*wrap*0.85;
 let h=normalize(l+view);
 let t1=normalize(t+n*material.shading.y);let t2=normalize(t+n*material.shading.z);
 let th1=dot(t1,h);let th2=dot(t2,h);
 let e1=clamp(2.0/max(roughness*roughness*roughness,0.0004),8.0,600.0);let e2=e1*0.3;
 let s1=pow(sqrt(max(1.0-th1*th1,0.0)),e1)*(e1+2.0)/6.2832*0.045;
 let s2=pow(sqrt(max(1.0-th2*th2,0.0)),e2)*(e2+2.0)/6.2832*0.06;
 let lit_side=clamp(dot(n,l)*0.6+0.4,0.0,1.0);
 let spec=(vec3(s1)+albedo*s2*1.6)*lit_side;
 // TT: light through the strands toward the eye.
 let back=pow(clamp(-dot(view,l),0.0,1.0),6.0);
 let tt=albedo*back*material.shading.w*2.2;
 return (diffuse+spec)*shadow+tt*mix(shadow,1.0,0.6);
}
fn cloth_light(n:vec3<f32>,view:vec3<f32>,l:vec3<f32>,albedo:vec3<f32>,nv:f32,shadow:f32)->vec3<f32>{
 let nl=dot(n,l);
 let diffuse=albedo*clamp((nl+0.25)/1.5625,0.0,1.0);
 let h=normalize(l+view);let nh=clamp(dot(n,h),0.0,1.0);
 let a=clamp(material.shading.z,0.07,1.0);let alpha=a*a;let inv=1.0/alpha;
 let sin2=max(1.0-nh*nh,0.0078125);
 let d=(2.0+inv)*pow(sin2,inv*0.5)/6.2832;
 let nlc=clamp(nl,0.0,1.0);
 let vis=1.0/max(4.0*(nlc+nv-nlc*nv),0.0001);
 let lum=max(dot(albedo,vec3(0.2126,0.7152,0.0722)),0.0001);
 let tint=mix(vec3(1.0),albedo/lum,0.4);
 let sheen=tint*material.shading.y*0.12*d*vis*nlc;
 return (diffuse+sheen*3.14159265)*shadow;
}
// The tear film mirrors each light as a source of finite size (a 15 cm
// soft box at the light's place), so the eye shows a catchlight shape, not
// a pin-prick: a disc where the mirrored view ray meets the source.
fn cornea_light(n:vec3<f32>,view:vec3<f32>,l:vec3<f32>,distance:f32,roughness:f32,nv:f32,shadow:f32)->vec3<f32>{
 let r=reflect(-view,n);
 let size=atan(0.075/max(distance,0.05));
 let c=dot(r,l);
 let disc=smoothstep(cos(size*1.25+roughness*0.3),cos(size*0.8),c);
 let fresnel=0.025+0.975*pow(1.0-nv,5.0);
 let glint=disc*fresnel/(3.14159265*max(size*size,0.0004))*1.6;
 return (reflectance(n,view,l,vec3(0.0),vec3(0.025),roughness,nv)*0.25+vec3(glint))*shadow;
}
fn stochastic(p:vec2<f32>,seed:f32)->f32{
 let q=fract(p*vec2(0.1031,0.1030)+vec2(seed*0.7548776662,seed*0.56984029));
 let r=q+dot(q,q.yx+33.33);
 return fract((r.x+r.y)*r.x);
}
// Exponential height fog (verse_engine::lighting::HeightFog) between the eye
// and p: the closed-form optical depth from the start distance, capped.
// Without falloff, start, or cap it is uniform fog, 1 - exp(-density x distance).
fn fog_amount(p:vec3<f32>)->f32{
 let ray=p-frame.eye.xyz;let len=max(length(ray),0.0001);let travel=len-frame.fog_shape.z;
 if travel<=0.0||frame.fog.w<=0.0{return 0.0;}
 let first=frame.eye.y+ray.y*(frame.fog_shape.z/len);let rise=ray.y*(travel/len);
 let at_start=frame.fog.w*exp(clamp(-frame.fog_shape.y*(first-frame.fog_shape.x),-80.0,80.0));
 let k=clamp(frame.fog_shape.y*rise,-80.0,80.0);
 var shape=1.0-0.5*k;
 if abs(k)>0.0001{shape=(1.0-exp(-k))/k;}
 return min(1.0-exp(-at_start*travel*shape),frame.fog_shape.w);
}
// A portal's opening: the baked view of the zone beyond, seen soft through
// a slow swirl and outward ripples, hazing toward a glowing white rim with a
// breath of the zone's own color (the tint) and bright arcs turning in it, as
// the line arches' rings did. uv spans the disc, 0.5 its center. From afar
// the view lies on the disc; near, it becomes a window, the view's direction
// from the eye choosing what shows, so walking up opens the scene around you.
// `axes` are the portal's local X and Z (through the opening) and its center.
// Explicit-level samples are valid in this non-uniform branch. It reads no
// pose, so the instanced variant's pose rewrite leaves it alone.
fn portal(v:Out,t:f32,x_axis:vec3<f32>,z_axis:vec3<f32>,center:vec3<f32>)->vec4<f32>{
 let disc=v.uv*2.0-1.0;let r=length(disc);
 if r>1.0{discard;}
 let ray=v.pos-frame.eye.xyz;
 let depth=max(abs(dot(ray,normalize(z_axis))),0.05);
 let slope=vec2(-dot(ray,normalize(x_axis)),-ray.y)/depth;
 let window=1.3*slope/sqrt(1.0+dot(slope,slope))+vec2(0.0,0.085);
 let near=smoothstep(7.0,2.0,length(frame.eye.xyz-center-vec3(0.0,2.1,0.0)));
 let p=mix(disc*0.88,window,near);
 let rp=length(p);let angle=atan2(p.y,p.x);
 let twist=r*r*0.5*sin(t*0.31)+0.06*sin(t*0.53+r*3.0);
 let ripple=rp+0.016*sin(r*19.0-t*2.3)*(0.3+0.7*r);
 let q=vec2(cos(angle+twist),sin(angle+twist))*ripple;
 let uv=clamp(q*0.5+0.5,vec2(0.002),vec2(0.998));
 let blur=0.4+1.8*r*r;let d=0.004+0.016*r;
 var view=textureSampleLevel(image,tex_sampler,uv,blur).rgb*0.4;
 view+=textureSampleLevel(image,tex_sampler,uv+vec2(d,0.0),blur).rgb*0.15;
 view+=textureSampleLevel(image,tex_sampler,uv-vec2(d,0.0),blur).rgb*0.15;
 view+=textureSampleLevel(image,tex_sampler,uv+vec2(0.0,d),blur).rgb*0.15;
 view+=textureSampleLevel(image,tex_sampler,uv-vec2(0.0,d),blur).rgb*0.15;
 // The light beyond breathes, and a faint mist drifts across it.
 let mist=0.5+0.5*sin(disc.x*3.1+t*0.42)*sin(disc.y*2.3-t*0.37);
 view=view*(1.2+0.06*sin(t*1.1))+vec3(0.05)*mist*r;
 let haze=smoothstep(0.62,0.99,r);let around=atan2(disc.y,disc.x);
 let arcs=0.5+0.5*sin(around*5.0-t*0.9+sin(around*3.0+t*0.4));
 let rim=smoothstep(0.9,0.985,r);
 var color=mix(view,vec3(1.0),haze*0.4);
 color+=(vec3(0.75)+v.tint*0.25)*rim*(1.0+1.8*arcs);
 color+=v.tint*smoothstep(0.7,0.97,r)*0.1*arcs;
 let fogged=mix(color,frame.fog.rgb,fog_amount(v.pos)*0.6);
 return vec4(min(fogged*frame.ambient.w,vec3(60000.0)),1.0);
}
@fragment fn fs(v:Out,@builtin(front_facing) front:bool)->@location(0) vec4<f32>{
 // Samples and derivatives come before any per-instance branch: WebGPU
 // requires uniform control flow for them.
 let model=u32(material.shading.x+0.5);
 var tex=textureSample(image,tex_sampler,v.uv);
 let geometric=normalize(select(-v.normal,v.normal,front));let n=surface_normal(v,geometric);
 // The cotangent frame: world gradients of u and v on this surface.
 let px=dpdx(v.pos);let py=dpdy(v.pos);let ux=dpdx(v.uv);let uy=dpdy(v.uv);
 let ca=cross(py,geometric);let cb=cross(geometric,px);
 let det=dot(px,ca);let inv_det=select(0.0,1.0/det,abs(det)>1e-20);
 let grad_u=(ca*ux.x+cb*uy.x)*inv_det;let grad_v=(ca*ux.y+cb*uy.y)*inv_det;
 var soft=n;
 if model==1u && material.maps.x>0.5 {
  // Skin: the normal map seen through scattering, from a blurrier mip.
  let blurred=textureSampleBias(normal_image,tex_sampler,v.uv,3.5).xyz*2.0-1.0;
  let tl=max(dot(grad_u,grad_u),max(dot(grad_v,grad_v),1e-20));
  let tw=(grad_u*blurred.x*material.channels.x+grad_v*blurred.y*material.channels.x)*inverseSqrt(tl)+geometric*blurred.z;
  soft=normalize(mix(geometric,normalize(tw),0.5));
 }
 if model==2u {
  // Eye: the iris lies on a plane behind the cornea; offset its texture by
  // the refracted view ray's travel to that plane.
  let to_eye=normalize(frame.eye.xyz-v.pos);
  let rho=length(v.uv-vec2(0.5))/max(material.shading.z,0.0001);
  let depth=material.shading.y*max(1.0-rho*rho,0.0);
  let r=refract(-to_eye,geometric,1.0/1.376);
  let travel=r*(depth/max(-dot(r,geometric),0.25));
  let shifted=v.uv+vec2(dot(grad_u,travel),dot(grad_v,travel));
  let iris=textureSample(image,tex_sampler,shifted);
  tex=mix(tex,iris,step(rho,1.0));
 }
 var roughness=material.params.z;var metallic=material.params.w;
 if material.maps.y>0.5 {
  let orm=textureSample(orm_image,tex_sampler,v.uv);
  roughness*=orm.g;metallic*=orm.b;
 }
 roughness=clamp(roughness,0.07,1.0);metallic=clamp(metallic,0.0,1.0);
 var ao=1.0;
 if material.maps.z>0.5 {ao=mix(1.0,textureSample(occlusion_image,tex_sampler,v.uv).r,material.channels.y);}
 var emission=material.emission.rgb;
 if material.maps.w>0.5 {emission*=textureSample(emission_image,tex_sampler,v.uv).rgb;}
 if pose.params.w>0.5 && frame.settings.w>0.0 {
  let volume=verse_fire_volume(v.uv*2.0-1.0,v.pos,pose.params.z,tex.a,u32(frame.settings.w),fire_noise,fire_lut,fire_sampler,fire_sampler);
  return vec4(volume.rgb*v.tint*1.6/max(volume.a,0.001),volume.a*pose.params.y);
 }
 if pose.params.x>4.5 && pose.params.x<5.5 {return portal(v,pose.params.z,pose.model[0].xyz,pose.model[2].xyz,pose.model[3].xyz);}
 if pose.params.x>1.5 {
  let color=tex.rgb*1.6;
  return vec4(color*v.tint,tex.a*pose.params.y);
 }
 if pose.params.x>0.5 {
  let facing=abs(dot(normalize(v.normal),normalize(frame.eye.xyz-v.pos)));
  let rim=pow(1.0-facing,1.5);
  let turbulence=0.7+0.3*sin(v.pos.x*17.0+v.pos.y*23.0+v.pos.z*19.0-pose.params.z*24.0);
  let opacity=(0.04+rim*0.2)*turbulence*pose.params.y;
  return vec4(v.tint*2.0,opacity);
 }
 // Unlit surfaces and lines: base color and tint, fogged and exposed, no lights.
 if material.emission.w>0.5 {
  let flat_alpha=tex.a*material.channels.z;
  if material.params.y==1.0 && flat_alpha<material.channels.w{discard;}
  let flat=tex.rgb*v.tint+select(vec3(0.0),tex.rgb*v.tint*0.7,material.params.x>0.5);
  let flat_fog=fog_amount(v.pos);
  return vec4(min(mix(flat,frame.fog.rgb,flat_fog)*frame.ambient.w,vec3(60000.0)),verse_coverage(flat_alpha,material.params.y));
 }
 let alpha=tex.a*material.channels.z;
 // Hair's cutout is stochastic, so the mean of jittered sub-frames gives
 // each strand its true soft coverage instead of a hard edge.
 var cutoff=material.channels.w;
 if model==4u {cutoff=max(stochastic(v.clip.xy,frame.settings.y*60.0),0.04);}
 if material.params.y==1.0 && alpha<cutoff{discard;}
 let albedo=tex.rgb*v.tint;
 var lit=albedo*(1.0-metallic)*frame.ambient.rgb*ao;
 if model==3u {lit=vec3(0.0);}
 let view_delta=frame.eye.xyz-v.pos;let view_direction=view_delta/max(length(view_delta),0.0001);
 let diffuse_color=albedo*(1.0-metallic);
 let f0=mix(vec3(0.04),albedo,metallic);
 let nv=max(dot(n,view_direction),0.0001);
 let strand=normalize(grad_v+vec3(0.0,0.0,1e-9));
 // Surface curvature (1/m), for skin's pre-integrated scattering.
 let curvature=length(fwidth(geometric))/max(length(fwidth(v.pos)),1e-6);
 for(var i=0u;i<u32(frame.settings.x);i++){
  let source=frame.lights[i*2u];let radiance=frame.lights[i*2u+1u];
  let delta=source.xyz-v.pos;let distance_squared=dot(delta,delta);
  if distance_squared>=source.w*source.w || radiance.w<=0.0 {continue;}
  let d=sqrt(distance_squared);let direction=delta/max(d,0.001);
  // Standard surfaces end at the terminator; skin, hair and cloth wrap
  // or transmit past it.
  if (model==0u||model==2u||model==3u) && dot(n,direction)<=0.0 {continue;}
  let falloff=verse_point_falloff(distance_squared,source.w,false);
  let shadow=occlusion(i,v.pos,geometric);
  var reflected=vec3(0.0);
  switch model {
   case 1u:{reflected=skin_light(n,soft,view_direction,direction,diffuse_color,roughness,nv,shadow,curvature);}
   case 3u:{reflected=cornea_light(n,view_direction,direction,d,roughness,nv,shadow);}
   case 4u:{reflected=hair_light(strand,geometric,view_direction,direction,diffuse_color,roughness,shadow);}
   case 5u:{reflected=cloth_light(n,view_direction,direction,diffuse_color,nv,shadow);}
   default:{reflected=reflectance(n,view_direction,direction,diffuse_color,f0,roughness,nv)*shadow;}
  }
  // The eye sits in its socket: the occlusion map's lid shadow dims the
  // eyeball's and the cornea's direct light too, not only the ambient.
  if model==2u {reflected*=ao;}
  // Skin: the baked occlusion stands in for the light its creases,
  // sockets and hairline block (no other shadow is fine enough).
  if model==1u {reflected*=mix(1.0,ao,0.7);}
  if model==3u {reflected*=sqrt(ao);}
  lit+=reflected*radiance.rgb*radiance.w*falloff;
 }
 emission+=select(vec3(0.0),albedo*0.7,material.params.x>0.5);
 lit+=emission;
 // Exposed linear radiance into the floating-point scene target. The shared
 // output pass (pbr/post.wgsl) adds bloom, grades, and tone-maps it, as it
 // does for the physical path; the cap keeps half floats finite.
 let fog=fog_amount(v.pos);let color=min(mix(lit,frame.fog.rgb,fog)*frame.ambient.w,vec3(60000.0));
 return vec4(color,verse_coverage(alpha,material.params.y));
}
// The water pass (`verse_pbr::water`): vs_water, fs_water_transmit, and fs_water.
// VERSE_WATER_IMPORTED
