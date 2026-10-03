struct Frame { view:mat4x4<f32>,eye:vec4<f32>,ambient:vec4<f32>,fog:vec4<f32>,settings:vec4<f32>,lights:array<vec4<f32>,16>,shadow:array<mat4x4<f32>,24> };
struct Pose { model:mat4x4<f32>,params:vec4<f32>,bones:array<mat4x4<f32>,256> };
struct Material { params:vec4<f32> };
@group(0) @binding(0) var<uniform> frame:Frame;
@group(0) @binding(1) var shadows:texture_depth_2d_array;
@group(0) @binding(2) var shadow_sampler:sampler_comparison;
@group(1) @binding(0) var image:texture_2d<f32>;
@group(1) @binding(1) var tex_sampler:sampler;
@group(1) @binding(2) var<uniform> material:Material;
@group(2) @binding(0) var<uniform> pose:Pose;
struct In { @location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) joints:vec4<u32>,@location(4) weights:vec4<f32>,@location(5) tint:vec3<f32> };
struct Out { @builtin(position) clip:vec4<f32>,@location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) tint:vec3<f32> };
@vertex fn vs(v:In)->Out {
 let skin=pose.bones[v.joints.x]*v.weights.x+pose.bones[v.joints.y]*v.weights.y+pose.bones[v.joints.z]*v.weights.z+pose.bones[v.joints.w]*v.weights.w;
 let model=pose.model*skin;let world=model*vec4(v.pos,1.0);var o:Out;o.clip=frame.view*world;o.pos=world.xyz;o.normal=normalize((model*vec4(v.normal,0.0)).xyz);o.uv=v.uv;o.tint=v.tint;return o;
}
@fragment fn shadow_fs(v:Out){
 if material.params.y==1.0 && textureSample(image,tex_sampler,v.uv).a<0.5 {discard;}
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
fn tone(x:vec3<f32>)->vec3<f32>{return clamp((x*(2.51*x+0.03))/(x*(2.43*x+0.59)+0.14),vec3(0.0),vec3(1.0));}
@fragment fn fs(v:Out,@builtin(front_facing) front:bool)->@location(0) vec4<f32>{
 let tex=textureSample(image,tex_sampler,v.uv);if material.params.y==1.0 && tex.a<0.5{discard;}
 let n=normalize(select(-v.normal,v.normal,front));var light=frame.ambient.rgb;
 for(var i=0u;i<u32(frame.settings.x);i++){let source=frame.lights[i*2u];let radiance=frame.lights[i*2u+1u];let delta=source.xyz-v.pos;let d=length(delta);let falloff=pow(max(1.0-d/source.w,0.0),2.0)/(1.0+d*d);let lambert=max(dot(n,delta/max(d,0.001)),0.0);light+=radiance.rgb*radiance.w*falloff*lambert*occlusion(i,v.pos,n);}
 let albedo=tex.rgb*v.tint;let emission=select(vec3(0.0),albedo*0.7,material.params.x>0.5);let lit=albedo*light+emission;
 let fog=1.0-exp(-length(frame.eye.xyz-v.pos)*frame.fog.w);let color=tone(mix(lit,frame.fog.rgb,fog)*frame.ambient.w);
 return vec4(color,tex.a);
}
