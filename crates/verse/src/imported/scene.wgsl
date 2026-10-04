// fog: rgb color, w density at the base height (1/m). fog_shape: base height
// (m), falloff with height (1/m), start distance (m), and opacity cap.
struct Frame { view:mat4x4<f32>,eye:vec4<f32>,ambient:vec4<f32>,fog:vec4<f32>,settings:vec4<f32>,lights:array<vec4<f32>,64>,shadow:array<mat4x4<f32>,24>,fog_shape:vec4<f32> };
struct Pose { model:mat4x4<f32>,params:vec4<f32>,bones:array<mat4x4<f32>,256> };
struct Material { params:vec4<f32>,channels:vec4<f32>,emission:vec4<f32>,maps:vec4<f32> };
@group(0) @binding(0) var<uniform> frame:Frame;
@group(0) @binding(1) var shadows:texture_depth_2d_array;
@group(0) @binding(2) var shadow_sampler:sampler_comparison;
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
 if pose.params.x>3.5 {
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
@fragment fn fs(v:Out,@builtin(front_facing) front:bool)->@location(0) vec4<f32>{
 if pose.params.x>1.5 {
  let tex=textureSample(image,tex_sampler,v.uv);
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
 let tex=textureSample(image,tex_sampler,v.uv);
 let geometric=normalize(select(-v.normal,v.normal,front));let n=surface_normal(v,geometric);
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
 let alpha=tex.a*material.channels.z;
 if material.params.y==1.0 && alpha<material.channels.w{discard;}
 let albedo=tex.rgb*v.tint;
 var lit=albedo*(1.0-metallic)*frame.ambient.rgb*ao;
 let view_delta=frame.eye.xyz-v.pos;let view_direction=view_delta/max(length(view_delta),0.0001);
 let diffuse_color=albedo*(1.0-metallic);
 let f0=mix(vec3(0.04),albedo,metallic);
 let nv=max(dot(n,view_direction),0.0001);
 for(var i=0u;i<u32(frame.settings.x);i++){
  let source=frame.lights[i*2u];let radiance=frame.lights[i*2u+1u];
  let delta=source.xyz-v.pos;let distance_squared=dot(delta,delta);
  if distance_squared>=source.w*source.w || radiance.w<=0.0 {continue;}
  let d=sqrt(distance_squared);let direction=delta/max(d,0.001);
  if dot(n,direction)<=0.0 {continue;}
  let attenuation=max(1.0-d/source.w,0.0);
  let falloff=attenuation*attenuation/(1.0+distance_squared);
  lit+=reflectance(n,view_direction,direction,diffuse_color,f0,roughness,nv)
      *radiance.rgb*radiance.w*falloff*occlusion(i,v.pos,geometric);
 }
 emission+=select(vec3(0.0),albedo*0.7,material.params.x>0.5);
 lit+=emission;
 // Exposed linear radiance into the floating-point scene target. The shared
 // output pass (pbr/post.wgsl) adds bloom, grades, and tone-maps it, as it
 // does for the physical path; the cap keeps half floats finite.
 let fog=fog_amount(v.pos);let color=min(mix(lit,frame.fog.rgb,fog)*frame.ambient.w,vec3(60000.0));
 return vec4(color,select(alpha,1.0,material.params.y==0.0));
}
