import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import { WGSL_GRASS_GROUND } from "./grass-ground.ts";
import { MMORPG_FOG_WGSL } from "./mmorpg-fog.ts";
import {
  FOG_HEIGHT_RATE,
  FOG_REFERENCE_HEIGHT,
  FOG_START_DISTANCE,
  FOG_SEGMENT_WGSL,
} from "./fog.ts";
import {
  WGSLMaterialVegetationFields,
  WGSL_VEGETATION_FUNCTIONS,
} from "./vegetation.ts";
import { LOCAL_SHADOW_VIEWS } from "./frame.ts";
import {
  WGSLMaterialTerrainFields,
  WGSL_TERRAIN_FUNCTIONS,
} from "./terrain.ts";
import { WGSLMaterialPaintFields, WGSL_PAINT_FUNCTIONS } from "./paint.ts";
import { WGSL_IBL } from "./ibl.ts";
import { WGSL_PROBE_FIELDS, probeShadingWGSL } from "./probe-shading.ts";
import { WGSL_DISPLAY_LUT } from "./display-lut.ts";
import { WGSL_VIGNETTE } from "./vignette.ts";
import { SURFACE_DECAL_COMPOSE_WGSL } from "./surface-decals.ts";
import { buildClusteredLightingWGSL } from "./clustered-lighting.ts";
import { WGSLMaterialLayerFields, WGSL_LAYER_FUNCTIONS } from "./layers.ts";
import { WGSLMaterialGlassFields, WGSL_GLASS_FUNCTIONS } from "./glass.ts";
import { WGSL_FILMIC } from "./filmic.ts";
import { selectedRendererFeatures } from "./features.ts";
import type { RendererFeatures } from "./feature-contract.ts";
import { WGSL_NORMAL_FRAME } from "./normals.ts";
import { WGSL_SHADOW_PROJECTION } from "./distant-shadow.ts";
import { WIND_SWAY_WGSL } from "./wind-sway.ts";
import { WIND_FLUTTER_WGSL } from "./wind-flutter.ts";
import { surfaceSkyWGSL } from "./surface-sky.ts";
import { WGSL_SURFACE_SKY_REFLECTION } from "./surface-sky-reflection.ts";
import { SHADOW_BOX_FILTER_WGSL } from "./shadow-filter.ts";
import {
  compactSurfaceBrdf,
  WGSL_ORIGINAL_BRDF,
  WGSL_SURFACE_BRDF,
} from "./surface-brdf.ts";
import { pruneGeneratedShaderFunctions } from "./generated-shader-pruning.ts";
import {
  STOCK_MATERIAL_FEATURES as FEATURE,
  STOCK_MATERIAL_FEATURE_WGSL,
} from "./material-specialization.ts";

/** Owned browser shading. Sky/fog structure follows the public Verse reference;
 * standard GGX/Schlick and height-integrated extinction retain independent inputs. */
export function buildRendererShaders(features: RendererFeatures) {
  const preparedSurfaceBrdf = features.preparedSurfaceBrdf === true;
  const COMMON = `
struct Frame { view:mat4x4<f32>,inverseView:mat4x4<f32>,shadow:mat4x4<f32>,eye:vec4<f32>,sun:vec4<f32>,sunColor:vec4<f32>,ambient:vec4<f32>,fog:vec4<f32>,settings:vec4<f32>,cloud:vec4<f32>,extra:vec4<f32>,lightPositions:array<vec4<f32>,32>,lightColors:array<vec4<f32>,32>,farShadow:mat4x4<f32>,lightDirections:array<vec4<f32>,32>,lightCones:array<vec4<f32>,32>,localShadows:array<mat4x4<f32>,${LOCAL_SHADOW_VIEWS}>,skyRotation:mat4x4<f32>,skyUV:vec4<f32>,skyTint:vec4<f32>,skyEnabled:vec4<f32>,iblSH:array<vec4<f32>,9>,iblRotation:mat4x4<f32>,iblSettings:vec4<f32>${WGSL_PROBE_FIELDS},clipPlane:vec4<f32>,passSettings:vec4<f32>,iblGain:vec4<f32>,bounceSettings:vec4<f32>,volumeMedium:vec4<f32>,volumeRange:vec4<f32>,distantShadow:mat4x4<f32>,iblConstantSH:array<vec4<f32>,9>,skyLayers:vec4<f32> };
@group(0) @binding(0) var<uniform> frame:Frame;
@group(0) @binding(5) var environmentImage:texture_2d<f32>;
@group(0) @binding(6) var environmentSampler:sampler;
fn safeNormalize(v:vec3<f32>,fallback:vec3<f32>)->vec3<f32>{
 let lengthSquared=dot(v,v);if lengthSquared>0.000000000001{return v*inverseSqrt(lengthSquared);}return fallback;
}
fn linearToSrgb(v:vec3<f32>)->vec3<f32>{
 let c=clamp(v,vec3(0.0),vec3(1.0));
 return select(c*12.92,1.055*pow(c,vec3(1.0/2.4))-vec3(0.055),c>vec3(0.0031308));
}
${features.sky?.commonWGSL ?? "fn sky(direction:vec3<f32>)->vec3<f32>{return vec3(0.0);}"}${FOG_SEGMENT_WGSL}
${WGSL_SHADOW_PROJECTION}
${MMORPG_FOG_WGSL}
fn surfaceFog(p:vec3<f32>)->vec2<f32>{
 let ray=p-frame.eye.xyz;let len=length(ray);
 return heightFogComposition(frame.eye.y,ray.y/max(len,0.000001),len,frame.fog.w,${FOG_HEIGHT_RATE},${FOG_REFERENCE_HEIGHT.toFixed(1)},${FOG_START_DISTANCE.toFixed(1)},frame.volumeMedium,frame.volumeRange.xy);
}
fn fogAmount(p:vec3<f32>)->f32{return 1.0-surfaceFog(p).x;}
`;

  const MATERIAL =
    COMMON +
    STOCK_MATERIAL_FEATURE_WGSL +
    `
@group(0) @binding(1) var shadowMap:texture_depth_2d;
@group(0) @binding(2) var shadowSampler:sampler_comparison;
@group(0) @binding(3) var farShadowMap:texture_depth_2d_array;
@group(0) @binding(4) var localShadowMap:texture_depth_2d_array;
@group(0) @binding(7) var contactOcclusion:texture_2d<f32>;
@group(0) @binding(10) var surfaceDecalImage:texture_2d_array<f32>;
struct Material { base:vec4<f32>,emission:vec4<f32>,params:vec4<f32>,uv:vec4<f32>,maps:vec4<f32>${WGSLMaterialLayerFields}${WGSLMaterialGlassFields}${WGSLMaterialPaintFields}${WGSLMaterialTerrainFields}${WGSLMaterialVegetationFields},creator:vec4<f32> };
// creator.x selects its image layer; creator.y selects the fixed grass-ground profile.
@group(1) @binding(0) var baseImage:texture_2d<f32>;
@group(1) @binding(1) var normalImage:texture_2d<f32>;
@group(1) @binding(2) var mraImage:texture_2d<f32>;
@group(1) @binding(3) var texSampler:sampler;
@group(1) @binding(4) var<uniform> material:Material;
@group(1) @binding(5) var emissionImage:texture_2d<f32>;
${WGSL_LAYER_FUNCTIONS}
${WGSL_GLASS_FUNCTIONS}
${probeShadingWGSL({ unrollSH9: true })}
${WGSL_IBL}
${SURFACE_DECAL_COMPOSE_WGSL}
${buildClusteredLightingWGSL(preparedSurfaceBrdf)}
${WGSL_PAINT_FUNCTIONS}
${WGSL_TERRAIN_FUNCTIONS}
${WGSL_VEGETATION_FUNCTIONS}
${WGSL_NORMAL_FRAME}
${WGSL_GRASS_GROUND}
${surfaceSkyWGSL()}
${WGSL_SURFACE_SKY_REFLECTION}
override AF_SURFACE_SKY:bool=true;
fn refinedSkyReflection(coefficients:SurfaceSkyVertex,direction:vec3<f32>,rough:f32)->vec4<f32>{
 if coefficients.g.w> -0.9999{return vec4(0.0);}
 let packed=array<vec4<f32>,7>(coefficients.a,coefficients.b,coefficients.c,coefficients.d,coefficients.e,coefficients.f,coefficients.g);
 var rgb:array<vec3<f32>,9>;
 for(var k=0u;k<9u;k++){let r=k*3u;rgb[k]=vec3(packed[r/4u][r%4u],packed[(r+1u)/4u][(r+1u)%4u],packed[(r+2u)/4u][(r+2u)%4u]);}
 return sh9RoughReflection(rgb,direction,rough);
}
struct In { @location(0) pos:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@location(3) tangent:vec4<f32>,@location(4) m0:vec4<f32>,@location(5) m1:vec4<f32>,@location(6) m2:vec4<f32>,@location(7) m3:vec4<f32>,@location(8) color:vec4<f32>,@location(9) uv1:vec2<f32>,@location(10) wind:vec4<f32> };
struct Out { @builtin(position) clip:vec4<f32>,@location(0) world:vec3<f32>,@location(1) normal:vec4<f32>,@location(2) rawUV:vec2<f32>,@location(3) color:vec4<f32>,@location(4) objectScale:vec4<f32>,@location(5) uv1:vec3<f32>,@location(6) @interpolate(flat) orientation:f32,@location(7) undeformedWorld:vec4<f32>,@location(8) surfaceSkyA:vec4<f32>,@location(9) surfaceSkyB:vec4<f32>,@location(10) surfaceSkyC:vec4<f32>,@location(11) surfaceSkyD:vec4<f32>,@location(12) surfaceSkyE:vec4<f32>,@location(13) surfaceSkyF:vec4<f32>,@location(14) surfaceSkyG:vec4<f32> };
struct DepthOut { @builtin(position) clip:vec4<f32>,@location(2) rawUV:vec2<f32>,@location(3) color:vec4<f32>,@location(6) @interpolate(flat) orientation:f32 };
struct PlainOut { @builtin(position) clip:vec4<f32>,@location(0) world:vec3<f32>,@location(1) normal:vec4<f32>,@location(2) rawUV:vec2<f32>,@location(3) color:vec4<f32>,@location(4) objectScale:vec4<f32>,@location(5) uv1:vec3<f32>,@location(6) @interpolate(flat) orientation:f32,@location(7) undeformedWorld:vec4<f32> };
${WIND_SWAY_WGSL}
${WIND_FLUTTER_WGSL}
fn vertexValue(v:In,vertex:u32,receiveSky:bool)->Out{
 let model=mat4x4(v.m0,v.m1,v.m2,v.m3);let original=model*vec4(v.pos,1.0);
 var offset=windSwayOffset(original.xyz,v.m3.xyz,v.m1.xyz,vec3(0.0,0.0,1.0),v.wind,frame.eye.w,v.color.g,v.wind.w>0.0);
 if v.wind.w>0.0&&v.color.g>0.0&&material.vegetationFlutter.x>.5{
  let additional=textureSampleLevel(layerColors,texSampler,v.uv,i32(material.vegetationMaps.x),0.0).rgb;
  offset+=windFlutterOffset(original.xyz,v.color.g,frame.passSettings.w,additional,material.vegetationFlutter,material.vegetationWaves);
 }
 let p=vec4(original.xyz+offset,1.0);var o:Out;o.clip=frame.view*p;o.world=p.xyz;o.undeformedWorld=vec4(original.xyz,0.0);
 let cofactor=mat3x3(cross(v.m1.xyz,v.m2.xyz),cross(v.m2.xyz,v.m0.xyz),cross(v.m0.xyz,v.m1.xyz));
 let signDet=select(-1.0,1.0,dot(v.m0.xyz,cross(v.m1.xyz,v.m2.xyz))>=0.0);
 o.orientation=signDet;o.normal=vec4(safeNormalize(cofactor*v.normal*signDet,vec3(0.0,1.0,0.0)),0.0);o.rawUV=v.uv;o.uv1=vec3(v.uv1,0.0);o.color=v.color;o.objectScale=vec4(length(v.m0.xyz),length(v.m1.xyz),length(v.m2.xyz),0.0);
 if normalMapSourceFrame(material.maps.w){
  let rotation=mat3x3(v.m0.xyz/o.objectScale.x,v.m1.xyz/o.objectScale.y,v.m2.xyz/o.objectScale.z);
  let tangent=rotation*v.tangent.xyz;
  o.normal=vec4(rotation*v.normal,v.tangent.w*signDet);
  o.objectScale.w=tangent.x;o.undeformedWorld.w=tangent.y;o.uv1.z=tangent.z;
 }
 let refined=surfaceSkyVertex(material.vegetationFlags.w,vertex,frame.skyLayers.x,receiveSky&&AF_SURFACE_SKY&&frame.passSettings.z>0.5);
 o.surfaceSkyA=refined.a;o.surfaceSkyB=refined.b;o.surfaceSkyC=refined.c;o.surfaceSkyD=refined.d;o.surfaceSkyE=refined.e;o.surfaceSkyF=refined.f;o.surfaceSkyG=refined.g;return o;
}
@vertex fn vs(v:In,@builtin(vertex_index) vertex:u32)->Out{return vertexValue(v,vertex,true);}
@vertex fn vsPlain(v:In,@builtin(vertex_index) vertex:u32)->PlainOut{let o=vertexValue(v,vertex,false);return PlainOut(o.clip,o.world,o.normal,o.rawUV,o.color,o.objectScale,o.uv1,o.orientation,o.undeformedWorld);}
@vertex fn depthVS(v:In,@builtin(vertex_index) vertex:u32)->DepthOut{let o=vertexValue(v,vertex,false);return DepthOut(o.clip,o.rawUV,o.color,o.orientation);}
@vertex fn shadowVS(v:In,@builtin(vertex_index) vertex:u32)->DepthOut{let o=vertexValue(v,vertex,false);return DepthOut(frame.shadow*vec4(o.world,1.0),o.rawUV,o.color,o.orientation);}
fn sourceFrontFacing(rasterFront:bool,orientation:f32)->bool{return rasterFront==(orientation>0.0);}
@fragment fn depthFS(v:DepthOut,@builtin(front_facing) front:bool){
 var alpha=textureSample(baseImage,texSampler,(v.rawUV*material.uv.xy)).a*material.base.a*select(1.0,v.color.a,material.maps.z>0.5);
 var threshold=material.uv.z;
 if stockMaterialFeature(${FEATURE.vegetation}u,material.vegetationConfig.x>0.5){alpha=textureSample(layerColors,texSampler,v.rawUV,i32(material.vegetationMaps.x)).a;threshold=material.vegetationConfig.w;}
 if material.uv.w<0.5&&!sourceFrontFacing(front,v.orientation){discard;}
 let masked=(material.params.w>0.5&&material.params.w<1.5)||stockMaterialFeature(${FEATURE.vegetation}u,material.vegetationConfig.x>0.5);
 if masked&&alpha<threshold{discard;}
}
@fragment fn shadowFS(v:DepthOut){
if stockMaterialFeature(${FEATURE.vegetation}u,material.vegetationConfig.x>0.5){if textureSample(layerColors,texSampler,v.rawUV,i32(material.vegetationMaps.x)).a<material.vegetationConfig.w{discard;}return;}
if material.params.w>0.5 && material.params.w<1.5{if textureSample(baseImage,texSampler,(v.rawUV*material.uv.xy)).a*material.base.a<material.uv.z{discard;}}}
fn shadowDepthBias(projection:mat4x4<f32>,dimensions:vec2<u32>,n:vec3<f32>)->f32{
 let rowX=vec3(projection[0].x,projection[1].x,projection[2].x);
 let rowY=vec3(projection[0].y,projection[1].y,projection[2].y);
 let rowZ=vec3(projection[0].z,projection[1].z,projection[2].z);
 let worldTexel=max(2.0/(f32(dimensions.x)*max(length(rowX),0.000001)),2.0/(f32(dimensions.y)*max(length(rowY),0.000001)));
 let slope=1.0-clamp(dot(n,frame.sun.xyz),0.0,1.0);
 return worldTexel*length(rowZ)*(0.25+1.5*slope);
}
${SHADOW_BOX_FILTER_WGSL}
fn shadowPcf(map:texture_depth_2d,projection:mat4x4<f32>,p:vec3<f32>,n:vec3<f32>,normalOffset:f32)->f32{
 let q=shadowProjection(projection,p+n*normalOffset);if !q.inside{return 1.0;}
 let dimensions=textureDimensions(map);let texel=1.0/vec2<f32>(dimensions);let bias=shadowDepthBias(projection,dimensions,n);
 if AF_SHADOW_BOX_FILTER{
  let k=shadowBoxKernel(q.uv,dimensions);let z=q.depth-bias;
  return textureSampleCompareLevel(map,shadowSampler,k.a,z)*k.weights.x
   +textureSampleCompareLevel(map,shadowSampler,vec2(k.b.x,k.a.y),z)*k.weights.y
   +textureSampleCompareLevel(map,shadowSampler,vec2(k.a.x,k.b.y),z)*k.weights.z
   +textureSampleCompareLevel(map,shadowSampler,k.b,z)*k.weights.w;
 }
 var visibility=0.0;
 for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){visibility+=textureSampleCompareLevel(map,shadowSampler,q.uv+vec2(f32(x),f32(y))*texel,q.depth-bias);}}
 return visibility/9.0;
}
fn shadowArrayPcf(layer:i32,projection:mat4x4<f32>,p:vec3<f32>,n:vec3<f32>,normalOffset:f32)->f32{
 let q=shadowProjection(projection,p+n*normalOffset);if !q.inside{return 1.0;}
 let dimensions=textureDimensions(farShadowMap);let texel=1.0/vec2<f32>(dimensions);let bias=shadowDepthBias(projection,dimensions,n);
 if AF_SHADOW_BOX_FILTER{
  let k=shadowBoxKernel(q.uv,dimensions);let z=q.depth-bias;
  return textureSampleCompareLevel(farShadowMap,shadowSampler,k.a,layer,z)*k.weights.x
   +textureSampleCompareLevel(farShadowMap,shadowSampler,vec2(k.b.x,k.a.y),layer,z)*k.weights.y
   +textureSampleCompareLevel(farShadowMap,shadowSampler,vec2(k.a.x,k.b.y),layer,z)*k.weights.z
   +textureSampleCompareLevel(farShadowMap,shadowSampler,k.b,layer,z)*k.weights.w;
 }
 var visibility=0.0;
 for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){visibility+=textureSampleCompareLevel(farShadowMap,shadowSampler,q.uv+vec2(f32(x),f32(y))*texel,layer,q.depth-bias);}}
 return visibility/9.0;
}
fn distantShadowVisibility(p:vec3<f32>,n:vec3<f32>)->f32{return shadowArrayPcf(1,frame.distantShadow,p,n,0.03);}
fn farShadowVisibility(p:vec3<f32>,n:vec3<f32>)->f32{
 let q=shadowProjection(frame.farShadow,p);if !q.inside{return distantShadowVisibility(p,n);}
 let visibility=shadowArrayPcf(0,frame.farShadow,p,n,0.03);if q.edge<=0.8{return visibility;}
 return mix(visibility,distantShadowVisibility(p,n),smoothstep(0.8,1.0,q.edge));
}
fn shadowVisibility(p:vec3<f32>,n:vec3<f32>)->f32{
 let q=shadowProjection(frame.shadow,p);if !q.inside{return farShadowVisibility(p,n);}
 let visibility=shadowPcf(shadowMap,frame.shadow,p,n,0.008);if q.edge<=0.8{return visibility;}
 return mix(visibility,farShadowVisibility(p,n),smoothstep(0.8,1.0,q.edge));
}
fn localVisibility(index:u32,p:vec3<f32>,n:vec3<f32>)->f32{
 let assigned=frame.lightCones[index].z;if assigned<0.0{return 1.0;}var layer=u32(assigned);
 let receiver=p+n*0.008;let fromLight=receiver-frame.lightPositions[index].xyz;
 if frame.lightDirections[index].w<0.5{let a=abs(fromLight);if a.x>=a.y&&a.x>=a.z{layer+=select(1u,0u,fromLight.x>=0.0);}else if a.y>=a.z{layer+=select(3u,2u,fromLight.y>=0.0);}else{layer+=select(5u,4u,fromLight.z>=0.0);}}
 let projection=frame.localShadows[layer];let q=projection*vec4(receiver,1.0);if q.w<=0.0{return 1.0;}
 let ndc=q.xyz/q.w;let uv=ndc.xy*vec2(0.5,-0.5)+vec2(0.5);if any(uv<vec2(0.0))||any(uv>vec2(1.0))||ndc.z<0.0||ndc.z>1.0{return 1.0;}
 let dimensions=textureDimensions(localShadowMap);let texel=1.0/vec2<f32>(dimensions);
 let rowX=vec3(projection[0].x,projection[1].x,projection[2].x);let rowY=vec3(projection[0].y,projection[1].y,projection[2].y);
 let worldTexel=max(2.0*q.w/(f32(dimensions.x)*max(length(rowX),0.000001)),2.0*q.w/(f32(dimensions.y)*max(length(rowY),0.000001)));
 let towardLight=safeNormalize(-fromLight,n);let slope=1.0-clamp(dot(n,towardLight),0.0,1.0);
 let biasDistance=min(worldTexel*(0.25+1.5*slope),length(fromLight)*0.25);
 let comparison=projection*vec4(receiver+towardLight*biasDistance,1.0);let comparisonDepth=comparison.z/comparison.w;
 if AF_SHADOW_BOX_FILTER{
  let k=shadowBoxKernel(uv,dimensions);
  return textureSampleCompareLevel(localShadowMap,shadowSampler,k.a,i32(layer),comparisonDepth)*k.weights.x
   +textureSampleCompareLevel(localShadowMap,shadowSampler,vec2(k.b.x,k.a.y),i32(layer),comparisonDepth)*k.weights.y
   +textureSampleCompareLevel(localShadowMap,shadowSampler,vec2(k.a.x,k.b.y),i32(layer),comparisonDepth)*k.weights.z
   +textureSampleCompareLevel(localShadowMap,shadowSampler,k.b,i32(layer),comparisonDepth)*k.weights.w;
 }
 var visibility=0.0;for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){visibility+=textureSampleCompareLevel(localShadowMap,shadowSampler,uv+vec2(f32(x),f32(y))*texel,i32(layer),comparisonDepth);}}return visibility/9.0;
}
${preparedSurfaceBrdf ? WGSL_SURFACE_BRDF : WGSL_ORIGINAL_BRDF}
fn shade(v:Out,front:bool)->vec4<f32>{
 let clipped=stockRasterFlag(1u,frame.passSettings.x>0.5)&&dot(vec4(v.world,1.0),frame.clipPlane)<0.0;
 let tex=textureSample(baseImage,texSampler,(v.rawUV*material.uv.xy));var base=tex.rgb*material.base.rgb;var textureAlpha=tex.a*material.base.a*select(1.0,v.color.a,material.maps.z>0.5);
 let physicalFront=stockRasterFlag(8u,false)||sourceFrontFacing(front,v.orientation);
 let sourceNormal=safeNormalize(v.normal.xyz,vec3(0.0,1.0,0.0));
 let faceSign=select(-1.0,1.0,physicalFront);
 let geometricNormal=sourceNormal*faceSign;
 let directXNormal=normalMapDirectX(material.maps.w);
 let view=safeNormalize(frame.eye.xyz-v.world,geometricNormal);
 if stockMaterialFeature(${FEATURE.flame}u,material.glassConfig.y>0.5) {
  let flame=evaluateFlame(v.rawUV);let extinction=1.0-fogAmount(v.world);
  if clipped||(!stockRasterFlag(4u,material.uv.w>=0.5)&&!physicalFront){discard;}
  return vec4(clamp(flame.emission*frame.settings.w*extinction,vec3(0.0),vec3(65000.0)),flame.opacity);
 }
 var n=geometricNormal;
 var surfaceSpecular=0.04;var subsurface=vec3(0.0);
 var surfaceEmission=textureSample(emissionImage,texSampler,(v.rawUV*material.uv.xy)).rgb*material.emission.xyz;
 var normalUV=(v.rawUV*material.uv.xy);
 var norm=vec3(0.0,0.0,1.0);if stockMaterialFeature(${FEATURE.normal}u,true){norm=decodeNormalSample(textureSample(normalImage,texSampler,(v.rawUV*material.uv.xy)).xyz,material.maps.w);}
 var mra=vec3(1.0);if stockMaterialFeature(${FEATURE.mra}u,true){mra=textureSample(mraImage,texSampler,(v.rawUV*material.uv.xy)).rgb;}
 var rough=clamp(material.params.x*select(1.0,mra.g,stockMaterialFeature(${FEATURE.mra}u,material.maps.y>0.5))*frame.settings.z,0.1,1.0);
 var metal=clamp(material.params.y*select(1.0,mra.r,stockMaterialFeature(${FEATURE.mra}u,material.maps.y>0.5)),0.0,1.0);
 var ao=mix(1.0,mra.b,material.maps.y);var normalScale=material.params.z;var hasNormal=stockMaterialFeature(${FEATURE.normal}u,material.maps.x>0.5);
 if stockMaterialFeature(${FEATURE.layered}u,material.layerConfig.x>0.5) {
  let layered=evaluateLayers(v.rawUV,v.color,v.world,geometricNormal,v.objectScale.xyz);
  base=layered.base;norm=layered.tangentNormal;mra=layered.mra;
  rough=clamp(mra.g*frame.settings.z,0.1,1.0);metal=clamp(mra.r,0.0,1.0);ao=clamp(mra.b,0.0,1.0);normalScale=1.0;hasNormal=true;
  normalUV=v.rawUV*material.layerUVHeight.xy;
  if material.layerConfig.y>1.5{normalUV=v.rawUV*abs(v.objectScale.zy);}
  else if material.layerConfig.y>0.5{normalUV=v.rawUV*abs(v.objectScale.xz);}
 }
 if stockMaterialFeature(${FEATURE.paint}u,material.paintConfig.x>0.5) {
  let painted=evaluatePaint(v.rawUV,v.uv1.xy,v.color,v.objectScale.xyz);
  base=painted.base;norm=painted.tangentNormal;mra=painted.mra;surfaceEmission=painted.emission;
  rough=clamp(mra.g*frame.settings.z,0.1,1.0);metal=clamp(mra.r,0.0,1.0);ao=clamp(mra.b,0.0,1.0);normalScale=1.0;hasNormal=true;
  normalUV=v.rawUV*material.paintUV.xy;
  if material.paintConfig.y>1.5{normalUV=v.rawUV*abs(v.objectScale.zy);}
  else if material.paintConfig.y>0.5{normalUV=v.rawUV*abs(v.objectScale.xz);}
 }
 if stockMaterialFeature(${FEATURE.terrain}u,material.terrainConfig.x>0.5) {
  normalUV=v.rawUV*material.terrainUV.xy;
  if material.terrainConfig.y>1.5{normalUV=v.rawUV*abs(v.objectScale.zy);}
  else if material.terrainConfig.y>0.5{normalUV=v.rawUV*abs(v.objectScale.xz);}
  let basis=normalFrame(sourceNormal,dpdx(v.undeformedWorld.xyz),dpdy(v.undeformedWorld.xyz),dpdx(normalUV),dpdy(normalUV));
  let tangentView=normalFrameView(basis,view,directXNormal);
  let terrain=evaluateTerrain(v.rawUV,v.color,v.objectScale.xyz,tangentView);
  base=terrain.base;norm=terrain.tangentNormal;mra=terrain.mra;surfaceEmission=terrain.emission;
  rough=clamp(mra.g*frame.settings.z,0.1,1.0);metal=clamp(mra.r,0.0,1.0);ao=clamp(mra.b,0.0,1.0);normalScale=1.0;hasNormal=true;
 }
 if stockMaterialFeature(${FEATURE.vegetation}u,material.vegetationConfig.x>0.5) {
  var basis=normalFrame(sourceNormal,dpdx(v.undeformedWorld.xyz),dpdy(v.undeformedWorld.xyz),dpdx(v.rawUV),dpdy(v.rawUV));
  if normalMapSourceFrame(material.maps.w){basis=sourceNormalFrame(v.normal.xyz,vec3(v.objectScale.w,v.undeformedWorld.w,v.uv1.z),v.normal.w);}
  var tangentView=normalFrameView(basis,view,directXNormal);
  if normalMapSourceFrame(material.maps.w){tangentView=sourceNormalFrameView(basis,view,directXNormal);}
  let vegetation=evaluateVegetation(v.rawUV,v.color,tangentView,select(-1.0,1.0,physicalFront));
  base=max(vegetation.base,vec3(0.0));norm=vegetation.tangentNormal;mra=vegetation.mra;textureAlpha=vegetation.opacity;
  rough=clamp(mra.g*frame.settings.z,0.1,1.0);metal=clamp(mra.r,0.0,1.0);ao=clamp(mra.b,0.0,1.0);normalScale=1.0;hasNormal=true;
  normalUV=v.rawUV;surfaceSpecular=clamp(vegetation.specular,0.0,1.0)*0.08;subsurface=vegetation.subsurface;
 }
 if stockMaterialFeature(${FEATURE.glass}u,material.glassConfig.x>0.5) {
  let glass=evaluateGlass(v.rawUV,v.world,geometricNormal,view);
  base=glass.base;norm=glass.tangentNormal;mra=glass.mra;textureAlpha=glass.opacity;
  rough=clamp(mra.g*frame.settings.z,0.1,1.0);metal=clamp(mra.r,0.0,1.0);ao=1.0;normalScale=1.0;hasNormal=true;
  normalUV=v.rawUV*material.glassUVWorld.xy;surfaceEmission=glass.emission;
 }
 if material.creator.y>0.5 {base=quickGrassGroundBase(v.world)*material.base.rgb;}
 if stockMaterialFeature(${FEATURE.creator}u,material.creator.x>0.5) {let creatorColor=textureSampleLevel(layerColors,texSampler,v.rawUV,0,0.0);base=mix(base,creatorColor.rgb,creatorColor.a);}
 let basis=normalFrame(sourceNormal,dpdx(v.undeformedWorld.xyz),dpdy(v.undeformedWorld.xyz),dpdx(normalUV),dpdy(normalUV));
 if hasNormal{
  var activeBasis=basis;
  if normalMapSourceFrame(material.maps.w){activeBasis=sourceNormalFrame(v.normal.xyz,vec3(v.objectScale.w,v.undeformedWorld.w,v.uv1.z),v.normal.w);}
  n=mapTangentNormal(activeBasis,norm,directXNormal,faceSign,normalScale*frame.extra.w);
 }
 if frame.passSettings.y>0.5&&material.params.w<1.5&&!stockMaterialFeature(${FEATURE.glass}u,material.glassConfig.x>=0.5){
  let decal=composeSurfaceDecals(surfaceDecalImage,vec2<i32>(v.clip.xy),base,n,vec3(metal,surfaceSpecular,rough),7u);
  base=decal.base;n=decal.normal;metal=clamp(decal.properties.x,0.0,1.0);surfaceSpecular=clamp(decal.properties.y,0.0,1.0);rough=clamp(decal.properties.z,0.1,1.0);subsurface*=decal.colorTransmittance;
 }
 var coverageWidth=1.0/255.0;
 if stockRasterFlag(2u,material.params.w>0.5&&material.params.w<1.5){coverageWidth=max(fwidth(textureAlpha),1.0/255.0);}
 // Complete derivative-dependent samples first; rejected fragments need no lighting.
 if clipped||(!stockRasterFlag(4u,material.uv.w>=0.5)&&!physicalFront){discard;}
 var direct=vec3(0.0);
 if frame.sun.w>0.0&&any(frame.sunColor.xyz>vec3(0.0))&&(dot(n,frame.sun.xyz)>0.0||dot(subsurface,subsurface)>0.0){
  let vis=shadowVisibility(v.world,geometricNormal);
  direct=(brdf(n,view,frame.sun.xyz,base,rough,metal,surfaceSpecular)+foliageTransmission(subsurface,n,frame.sun.xyz,view,material.vegetationWaves.w))*frame.sunColor.xyz*frame.sun.w*vis;
 }
 let skyFill=mix(frame.ambient.xyz*0.36,frame.ambient.xyz,n.y*0.5+0.5)*frame.cloud.z;
 let emission=surfaceEmission*frame.settings.w;
 var contact=1.0;
 if frame.passSettings.y>0.5{contact=textureLoad(contactOcclusion,clamp(vec2<i32>(v.clip.xy*0.5),vec2(0),vec2<i32>(textureDimensions(contactOcclusion))-vec2(1)),0).r;}
 var ambientSurface=base*skyFill*ao*contact;
 if frame.iblSettings.x>0.5{var r=vec4(0.0);var s=vec4(0.0);
  if AF_SURFACE_SKY{let t=SurfaceSkyVertex(v.surfaceSkyA,v.surfaceSkyB,v.surfaceSkyC,v.surfaceSkyD,v.surfaceSkyE,v.surfaceSkyF,v.surfaceSkyG);
   r=surfaceSkyIrradiance(t,n);s=refinedSkyReflection(t,reflect(-view,n),rough);}
  ambientSurface=environmentSurfaceWithReflections(v.world,geometricNormal,n,view,base,rough,metal,ao*contact,surfaceSpecular,r,s);}
 if frame.skyLayers.w>.5{ambientSurface=base*(1.0-metal)*mix(vec3(106.0/255.0,136.0/255.0,181.0/255.0),vec3(66.0/255.0,74.0/255.0,117.0/255.0),n.y*.5+.5)*(.7*0.01)*ao*contact;}
 var color=direct+ambientSurface+emission;
 color+=clusteredSurfaceTransmission(v.clip.xy,v.world,n,view,base,rough,metal,surfaceSpecular,subsurface,geometricNormal,material.vegetationWaves.w);
 for(var i=0u;i<select(0u,u32(frame.ambient.w),clusteredLights[0]==0u);i++){
  let lp=frame.lightPositions[i];let delta=lp.xyz-v.world;let dist2=dot(delta,delta);let dist=sqrt(dist2);
  let lightDirection=delta/max(dist,0.001);
  var attenuation=pow(clamp(1.0-pow(dist/max(lp.w,0.1),4.0),0.0,1.0),2.0)/(0.0001+dist2);
  if frame.lightCones[i].w>=0.0{attenuation=pow(clamp(1.0-dist2/(lp.w*lp.w),0.0,1.0),max(frame.lightCones[i].w,0.0001));}
  if frame.lightDirections[i].w>0.5{
   let cone=frame.lightCones[i];let outer=min(cone.x,cone.y);let inner=max(cone.x,cone.y);
   let angle=dot(-lightDirection,safeNormalize(frame.lightDirections[i].xyz,vec3(0.0,-1.0,0.0)));
   var coneVisibility=step(outer,angle);if inner-outer>0.00001{let ramp=clamp((angle-outer)/(inner-outer),0.0,1.0);coneVisibility=ramp*ramp;}
   attenuation*=coneVisibility;
  }
  if attenuation>0.0 && (dot(n,lightDirection)>0.0||dot(subsurface,subsurface)>0.0){color+=(brdf(n,view,lightDirection,base,rough,metal,surfaceSpecular)+foliageTransmission(subsurface,n,lightDirection,view,material.vegetationWaves.w))*frame.lightColors[i].rgb*frame.lightColors[i].w*attenuation*localVisibility(i,v.world,geometricNormal);}
 }
 if frame.skyLayers.w>.5{color=mix(color,mmoFogColor*${SCENE_LINEAR_SCALE},mmoFog(frame.eye.y/frame.skyLayers.w,(v.world-frame.eye.xyz)/frame.skyLayers.w));}else{
 let fog=surfaceFog(v.world);var fogColor=frame.fog.rgb;
 // Imported fog has no inscattering cubemap; the generated sky keeps its owned fill.
 if frame.skyEnabled.x<0.5{fogColor=mix(frame.fog.rgb,sky(normalize(v.world-frame.eye.xyz)),0.35);}
 color=color*fog.x+fogColor*fog.y;
 }
 var coverage=1.0;
 if stockRasterFlag(2u,material.params.w>0.5&&material.params.w<1.5){coverage=smoothstep(material.uv.z-coverageWidth*0.5,material.uv.z+coverageWidth*0.5,textureAlpha);}
 let alpha=select(coverage,textureAlpha,material.params.w>1.5||stockMaterialFeature(${FEATURE.glass}u,material.glassConfig.x>0.5));
 return vec4(max(color,vec3(0.0)),alpha);
}
@fragment fn fs(v:Out,@builtin(front_facing) front:bool)->@location(0)vec4<f32>{return shade(v,front);}
@fragment fn fsPlain(v:PlainOut,@builtin(front_facing) front:bool)->@location(0)vec4<f32>{return shade(Out(v.clip,v.world,v.normal,v.rawUV,v.color,v.objectScale,v.uv1,v.orientation,v.undeformedWorld,vec4(0.0),vec4(0.0),vec4(0.0),vec4(0.0),vec4(0.0),vec4(0.0),vec4(0.0)),front);}
`;
  const SKY = features.sky ? COMMON + features.sky.backgroundWGSL : "";
  const pruned = pruneGeneratedShaderFunctions(MATERIAL, [
    "linearToSrgb",
    "distanceMoments",
    "localSkyLighting",
    "localSkyIrradiance",
    "environmentSurfaceWithSky",
    "environmentSurface",
    "clusteredSurface",
  ]);
  return {
    COMMON,
    MATERIAL: (preparedSurfaceBrdf ? compactSurfaceBrdf(pruned) : pruned)
      .replace(/\blayerColors\b/g, "LC")
      .replace(/\blayerNormals\b/g, "LN")
      .replace(/\blayerData\b/g, "LD"),
    SKY,
  };
}
export const { COMMON, MATERIAL, SKY } = buildRendererShaders(
  selectedRendererFeatures,
);
export const POST =
  WGSL_FILMIC +
  WGSL_DISPLAY_LUT +
  WGSL_VIGNETTE +
  `
fn linearToSrgb(v:vec3<f32>)->vec3<f32>{
 let c=clamp(v,vec3(0.0),vec3(1.0));
 return select(c*12.92,1.055*pow(c,vec3(1.0/2.4))-vec3(0.055),c>vec3(0.0031308));
}

@group(0)@binding(0)var source:texture_2d<f32>;
@group(0)@binding(1)var filtering:sampler;
struct DisplaySettings{settings:vec4<f32>,filmic:FilmicParams,lut:vec4<f32>,vignette:vec4<f32>};
@group(0)@binding(2)var<uniform>display:DisplaySettings;
@group(0)@binding(3)var bloomImage:texture_2d<f32>;
@group(0)@binding(4)var displayImage:texture_3d<f32>;
@group(0)@binding(5)var displayFiltering:sampler;
struct Out{@builtin(position)pos:vec4<f32>,@location(0)uv:vec2<f32>};
@vertex fn vs(@builtin(vertex_index)i:u32)->Out{let p=vec2(f32((i<<1u)&2u),f32(i&2u));var o:Out;o.pos=vec4(p*2.0-1.0,0.0,1.0);o.uv=vec2(p.x,1.0-p.y);return o;}
fn aces(c:vec3<f32>)->vec3<f32>{return clamp((c*(2.51*c+vec3(0.03)))/(c*(2.43*c+vec3(0.59))+vec3(0.14)),vec3(0.0),vec3(1.0));}
@fragment fn fs(v:Out)->@location(0)vec4<f32>{var c=textureSample(source,filtering,v.uv).rgb;
 let bloom=textureSample(bloomImage,filtering,v.uv).rgb;
 c=(c+bloom*display.settings.z)*display.settings.x;
 if display.settings.w<-.5{c=clamp(c,vec3(0.0),vec3(1.0));}
 else if display.settings.w>0.5{c*=vignetteWeight(v.uv,display.vignette.y,display.vignette.x);let l=dot(c,vec3(0.2126,0.7152,0.0722));c=mix(vec3(l),c,display.settings.y);
  if display.lut.w>0.5{c=sampleDisplayLut(displayImage,displayFiltering,display.lut,c);}else{c=filmicColor(c,display.filmic);}}
 else{c=aces(c);let l=dot(c,vec3(0.2126,0.7152,0.0722));c=mix(vec3(l),c,display.settings.y);}
 return vec4(c,1.0);}
`;
