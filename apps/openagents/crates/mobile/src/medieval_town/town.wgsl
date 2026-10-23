struct Frame { viewProjection: mat4x4<f32>, eye: vec4<f32>, sun: vec4<f32>, fog: vec4<f32> }
struct Draw { model: mat4x4<f32>, atlasRect: vec4<f32>, uv: vec4<f32> }
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlasSampler: sampler;
@group(1) @binding(0) var<uniform> draw: Draw;
struct Vertex { @location(0) position:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32> }
struct Varying { @builtin(position) clip:vec4<f32>, @location(0) position:vec3<f32>, @location(1) normal:vec3<f32>, @location(2) uv:vec2<f32> }
@vertex fn worldVertex(v:Vertex)->Varying {
    var out:Varying;
    let world=draw.model*vec4(v.position,1.0);
    out.clip=frame.viewProjection*world;out.position=world.xyz;
    out.normal=normalize((draw.model*vec4(v.normal,0.0)).xyz);out.uv=v.uv;
    return out;
}
@fragment fn worldFragment(v:Varying)->@location(0) vec4<f32> {
    let local=fract(v.uv*draw.uv.xy);
    let uv=draw.atlasRect.xy+local*draw.atlasRect.zw;
    let color=textureSample(atlas,atlasSampler,uv);
    if color.a<draw.uv.z { discard; }
    let normal=normalize(v.normal);
    let diffuse=max(dot(normal,normalize(frame.sun.xyz)),0.0);
    let hemisphere=mix(vec3(0.24,0.25,0.24),vec3(0.43,0.49,0.54),normal.y*0.5+0.5);
    let lit=color.rgb*(hemisphere+vec3(1.08,0.99,0.82)*diffuse);
    let distance=length(v.position-frame.eye.xyz);
    let fog=smoothstep(frame.fog.x,frame.fog.y,distance);
    let graded=lit/(vec3(0.55)+lit);
    return vec4(mix(graded,vec3(0.56,0.64,0.69),fog*0.82),1.0);
}
struct Sky { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32> }
@vertex fn skyVertex(@builtin(vertex_index) i:u32)->Sky {
    let p=array<vec2<f32>,3>(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0));
    var out:Sky;out.position=vec4(p[i],1.0,1.0);out.uv=p[i]*0.5+0.5;return out;
}
@fragment fn skyFragment(v:Sky)->@location(0) vec4<f32> {
    let sky=mix(vec3(0.62,0.70,0.74),vec3(0.18,0.35,0.56),clamp(v.uv.y,0.0,1.0));
    let sun=pow(max(0.0,1.0-length((v.uv-vec2(0.77,0.80))*vec2(1.0,1.6))),22.0);
    return vec4(sky+vec3(0.37,0.29,0.16)*sun,1.0);
}
