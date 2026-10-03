struct Frame { view: mat4x4<f32>, eye: vec4<f32> };
struct Pose { model: mat4x4<f32>, params: vec4<f32>, bones: array<mat4x4<f32>,256> };
@group(0) @binding(0) var<uniform> frame: Frame;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var tex_sampler: sampler;
struct Material { params:vec4<f32> };
@group(1) @binding(2) var<uniform> material:Material;
@group(2) @binding(0) var<uniform> pose: Pose;
struct In { @location(0) pos:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32>, @location(3) joints:vec4<u32>, @location(4) weights:vec4<f32>, @location(5) tint:vec3<f32> };
struct Out { @builtin(position) clip:vec4<f32>, @location(0) pos:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32>, @location(3) tint:vec3<f32> };
@vertex fn vs(v:In)->Out {
 let skin=pose.bones[v.joints.x]*v.weights.x+pose.bones[v.joints.y]*v.weights.y+pose.bones[v.joints.z]*v.weights.z+pose.bones[v.joints.w]*v.weights.w;
 let model=pose.model*skin;let world=model*vec4(v.pos,1.0);var o:Out;o.clip=frame.view*world;o.pos=world.xyz;o.normal=normalize((model*vec4(v.normal,0.0)).xyz);o.uv=v.uv;o.tint=v.tint;return o;
}
@fragment fn fs(v:Out,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {
 let tex=textureSample(image,tex_sampler,v.uv);if material.params.y==1.0 && tex.a<0.5 {discard;}
 let normal=select(-v.normal,v.normal,front);
 let shade=0.35+0.65*max(dot(normal,normalize(vec3(0.4,0.8,0.3))),0.0);
 return vec4(tex.rgb*v.tint*select(shade,1.0,material.params.x>0.5),tex.a);
}
