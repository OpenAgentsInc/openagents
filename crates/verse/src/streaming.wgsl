struct Globals { view_proj:mat4x4<f32>, eye:vec4<f32>, fog:vec4<f32> };
@group(0) @binding(0) var<uniform> globals:Globals;
@group(1) @binding(0) var image:texture_2d<f32>;
@group(1) @binding(1) var image_sampler:sampler;
struct In { @location(0) pos:vec3<f32>, @location(1) color:vec3<f32>, @location(2) uv:vec2<f32>, @location(3) fog:f32 };
struct Out { @builtin(position) pos:vec4<f32>, @location(0) color:vec3<f32>, @location(1) uv:vec2<f32>, @location(2) fog:f32 };
fn project(v:In)->Out {
 var o:Out; o.pos=globals.view_proj*vec4(v.pos,1.); o.color=v.color;o.uv=v.uv;
 o.fog=clamp((distance(v.pos,globals.eye.xyz)-globals.eye.w)/max(globals.fog.w-globals.eye.w,0.001),0.,1.)*v.fog;return o;
}
@vertex fn vs(v:In)->Out { return project(v); }
@vertex fn vs_reverse(v:In)->Out { var o=project(v);o.pos.z=o.pos.w-o.pos.z;return o; }
@fragment fn fs(v:Out)->@location(0) vec4<f32> {
 let texel=textureSample(image,image_sampler,v.uv);if texel.a<0.5 { discard; }
 return vec4(mix(texel.rgb*v.color,globals.fog.rgb,v.fog),1.);
}
