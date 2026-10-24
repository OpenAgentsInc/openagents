/** A 3 by 3 box over bilinear comparisons has four separable weighted pairs. */
export const SHADOW_BOX_FILTER_WGSL = `
override AF_SHADOW_BOX_FILTER:bool=false;
struct ShadowBoxKernel{a:vec2<f32>,b:vec2<f32>,weights:vec4<f32>};
fn shadowBoxKernel(uv:vec2<f32>,dimensions:vec2<u32>)->ShadowBoxKernel{
 let size=vec2<f32>(dimensions);let pixel=uv*size-vec2(0.5);
 let origin=floor(pixel);let fraction=fract(pixel);
 let low=vec2(2.0)-fraction;let high=vec2(1.0)+fraction;
 let a=(origin-vec2(0.5)+vec2(1.0)/low)/size;
 let b=(origin+vec2(1.5)+fraction/high)/size;
 return ShadowBoxKernel(a,b,vec4(low.x*low.y,high.x*low.y,low.x*high.y,high.x*high.y)/9.0);
}
`;
