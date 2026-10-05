struct Material { params: vec4<f32>, channels: vec4<f32>, emission: vec4<f32>, maps: vec4<f32> }
@group(0) @binding(0) var base: texture_2d<f32>;
@group(0) @binding(1) var linear_sampler: sampler;
@group(0) @binding(2) var<uniform> material: Material;
@group(0) @binding(3) var normal: texture_2d<f32>;
@group(0) @binding(4) var orm: texture_2d<f32>;
@group(0) @binding(5) var occlusion: texture_2d<f32>;
@group(0) @binding(6) var emission: texture_2d<f32>;
@vertex fn vertex(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = array<vec2<f32>, 3>(vec2(-1., -1.), vec2(3., -1.), vec2(-1., 3.));
    return vec4(p[i], 0., 1.);
}
@fragment fn channels(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = vec2(0.5);
    switch u32(p.x / 64.) {
        case 0u: { return textureSampleLevel(base, linear_sampler, uv, 1.); }
        case 1u: { return textureSampleLevel(normal, linear_sampler, uv, 1.); }
        case 2u: { return textureSampleLevel(orm, linear_sampler, uv, 1.); }
        case 3u: { return textureSampleLevel(occlusion, linear_sampler, uv, 1.); }
        default: { return textureSampleLevel(emission, linear_sampler, uv, 1.); }
    }
}
@fragment fn mask(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
    let alpha = textureSampleLevel(base, linear_sampler, p.xy / 64., 1.).a * material.channels.z;
    let kept = select(0., 1., alpha >= material.channels.w);
    return vec4(vec3(kept), 1.);
}
