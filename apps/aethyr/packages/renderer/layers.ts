import type { CompiledMaterial } from "../assets/src/scene.ts";

export type LayerTextureRole = "color" | "normal" | "data";
export interface LayerTexture {
  url: string;
  role: LayerTextureRole;
}
export const LAYER_FLOAT_COUNT = 36;

/** Includes the leading comma; append directly after Material.maps. */
export const WGSLMaterialLayerFields = `,
 layerConfig:vec4<f32>,layerUVHeight:vec4<f32>,
 layerColor0:vec4<f32>,layerColor1:vec4<f32>,
 layerIndices0:vec4<f32>,layerIndices1:vec4<f32>,
 layerMasks:vec4<f32>,layerWorld:vec4<f32>,layerDetail:vec4<f32>`;

const dataRole = (space: "srgb" | "linear"): LayerTextureRole =>
  space === "srgb" ? "color" : "data";

export function collectLayerTextures(
  material: Pick<CompiledMaterial, "layers">,
): LayerTexture[] {
  const layers = material.layers;
  if (!layers || layers.model !== "ue-mm-blend-2-v1") return [];
  const result = new Map<string, LayerTexture>();
  const add = (url: string | undefined, role: LayerTextureRole) => {
    if (url) result.set(`${role}\0${url}`, { url, role });
  };
  for (const slot of layers.slots) {
    add(slot.baseColorUrl, "color");
    add(slot.normalUrl, "normal");
    add(slot.mraUrl, "data");
  }
  if (layers.blend.height.mode === "maskRed")
    add(
      layers.blend.height.url,
      dataRole(layers.blend.height.sourceColorSpace),
    );
  add(
    layers.blend.worldSubtract.url,
    dataRole(layers.blend.worldSubtract.sourceColorSpace),
  );
  add(layers.detailNormal.url, "normal");
  return [...result.values()];
}

/** Index zero in every pool must be its typed default, even for missing maps. */
export function packLayers(
  material: Pick<CompiledMaterial, "layers">,
  poolIndex: (url: string, role: LayerTextureRole) => number,
): Float32Array {
  const result = new Float32Array(LAYER_FLOAT_COUNT);
  const layers = material.layers;
  if (!layers || layers.model !== "ue-mm-blend-2-v1") return result;
  const index = (url: string | undefined, role: LayerTextureRole) => {
    if (!url) return 0;
    const value = poolIndex(url, role);
    if (!Number.isInteger(value) || value < 0 || value > 0xffffff)
      throw new RangeError("Invalid layer texture pool index");
    return value;
  };
  const [a, b] = layers.slots;
  const { height, worldSubtract } = layers.blend;
  result.set(
    [
      1,
      layers.uv.mode === "objectXY" ? 1 : layers.uv.mode === "objectYZ" ? 2 : 0,
      layers.blend.phase,
      layers.blend.contrast,
    ],
    0,
  );
  result.set(
    [
      ...layers.uv.scale,
      height.mode === "layer2Alpha" ? 1 : 0,
      height.customIntensity,
    ],
    4,
  );
  result.set(a.colorFactor, 8);
  result.set(b.colorFactor, 12);
  result.set(
    [
      index(a.baseColorUrl, "color"),
      index(a.normalUrl, "normal"),
      index(a.mraUrl, "data"),
      index(b.baseColorUrl, "color"),
    ],
    16,
  );
  result.set(
    [
      index(b.normalUrl, "normal"),
      index(b.mraUrl, "data"),
      height.mode === "maskRed"
        ? index(height.url, dataRole(height.sourceColorSpace))
        : 0,
      index(worldSubtract.url, dataRole(worldSubtract.sourceColorSpace)),
    ],
    20,
  );
  result.set(
    [
      height.sourceColorSpace === "srgb" ? 1 : 0,
      worldSubtract.sourceColorSpace === "srgb" ? 1 : 0,
      worldSubtract.sizeMeters,
      layers.colorIntensity,
    ],
    24,
  );
  result.set(
    [
      worldSubtract.intensity,
      worldSubtract.power,
      index(layers.detailNormal.url, "normal"),
      layers.detailNormal.flatness,
    ],
    28,
  );
  result.set(layers.detailNormal.uvScale, 32);
  if (!result.every(Number.isFinite))
    throw new RangeError("Layer material parameters must be finite");
  return result;
}

// World-mask projection divides source XYZ coordinates by the negative texture size.
export const WGSL_LAYER_FUNCTIONS = `@group(1)@binding(6)var layerColors:texture_2d_array<f32>;@group(1)@binding(7)var layerNormals:texture_2d_array<f32>;@group(1)@binding(8)var layerData:texture_2d_array<f32>;struct LayerSample{base:vec3<f32>,tangentNormal:vec3<f32>,mra:vec3<f32>};fn layerUnitNormal(value:vec3<f32>)->vec3<f32>{let squared=dot(value,value);if squared>0.000000000001{return value*inverseSqrt(squared);}return vec3(0.0,0.0,1.0);}fn layerHeightAlpha(height:f32,phase:f32,contrast:f32)->f32{let first=clamp(height-1.0+2.0*phase,0.0,1.0);return clamp(first*(1.0+2.0*contrast)-contrast,0.0,1.0);}fn layerMaskValue(index:f32,isColor:f32,uv:vec2<f32>)->f32{if isColor>0.5{return textureSample(layerColors,texSampler,uv,i32(index)).r;}return textureSample(layerData,texSampler,uv,i32(index)).r;}fn layerWorldMask(worldPos:vec3<f32>,geometricNormal:vec3<f32>)->f32{let p=-worldPos.xzy/max(abs(material.layerMasks.z),0.00000001);let n=geometricNormal.xzy;let xz=layerMaskValue(material.layerIndices1.w,material.layerMasks.y,p.xz);let yz=layerMaskValue(material.layerIndices1.w,material.layerMasks.y,p.yz);let xy=layerMaskValue(material.layerIndices1.w,material.layerMasks.y,p.xy);let alongX=clamp(3.0*abs(n.x)-1.0,0.0,1.0);let alongZ=clamp(3.0*abs(n.z)-1.0,0.0,1.0);return mix(mix(xz,yz,alongX),xy,alongZ);}fn layerMaskPower(base:f32,exponent:f32)->f32{if exponent==0.0{return 1.0;}if base<=0.0 && exponent>0.0{return 0.0;}return pow(max(base,0.00000001),exponent);}fn evaluateLayers(rawUV:vec2<f32>,vertexColor:vec4<f32>,worldPos:vec3<f32>,worldGeometricNormal:vec3<f32>,objectScale:vec3<f32>)->LayerSample{var uv=rawUV*material.layerUVHeight.xy;let scales=abs(objectScale);if material.layerConfig.y>1.5{uv=rawUV*scales.zy;}else if material.layerConfig.y>0.5{uv=rawUV*scales.xz;}let color0=textureSample(layerColors,texSampler,uv,i32(material.layerIndices0.x));let color1=textureSample(layerColors,texSampler,uv,i32(material.layerIndices0.w));let normal0=decodeNormalSample(textureSample(layerNormals,texSampler,uv,i32(material.layerIndices0.y)).xyz,material.maps.w);let normal1=decodeNormalSample(textureSample(layerNormals,texSampler,uv,i32(material.layerIndices1.x)).xyz,material.maps.w);let mra0=textureSample(layerData,texSampler,uv,i32(material.layerIndices0.z)).rgb;let mra1=textureSample(layerData,texSampler,uv,i32(material.layerIndices1.y)).rgb;let worldMask=layerWorldMask(worldPos,worldGeometricNormal);let subtraction=clamp(layerMaskPower(worldMask*material.layerWorld.x,material.layerWorld.y),0.0,1.0);var height=layerMaskValue(material.layerIndices1.z,material.layerMasks.x,uv);if material.layerUVHeight.z>0.5{height=(1.0-color1.a)*material.layerUVHeight.w;}height=clamp(height-subtraction,0.0,1.0);let first=layerHeightAlpha(height,material.layerConfig.z,material.layerConfig.w);let second=layerHeightAlpha(height,vertexColor.r,material.layerConfig.w);let weight=first*second;let base=mix(color0.rgb*material.layerColor0.rgb,color1.rgb*material.layerColor1.rgb,weight)*material.layerMasks.w;let normal=mix(normal0,normal1,weight);let detailSample=decodeNormalSample(textureSample(layerNormals,texSampler,rawUV*material.layerDetail.xy,i32(material.layerWorld.z)).xyz,material.maps.w);let detail=mix(detailSample,vec3(0.0,0.0,1.0),material.layerWorld.w);let t=vec3(normal.xy,normal.z+1.0);let u=vec3(-detail.xy,detail.z);let combined=layerUnitNormal(t*dot(t,u)-u*t.z);return LayerSample(base,combined,mix(mra0,mra1,weight));}`;
