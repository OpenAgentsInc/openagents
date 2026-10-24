import type { CompiledMaterial } from "../assets/src/scene.ts";
import type { LayerTexture, LayerTextureRole } from "./layers.ts";

export const PAINT_FLOAT_COUNT = 36;
export const WGSLMaterialPaintFields = `,
 paintConfig:vec4<f32>,paintUV:vec4<f32>,paintTint0:vec4<f32>,paintTint1:vec4<f32>,
 paintIndices0:vec4<f32>,paintIndices1:vec4<f32>,paintIndices2:vec4<f32>,paintValues:vec4<f32>,paintEmission:vec4<f32>`;
const roleFor = (space: "linear" | "srgb"): LayerTextureRole =>
  space === "srgb" ? "color" : "data";

export function collectPaintTextures(
  material: Pick<CompiledMaterial, "paint">,
): LayerTexture[] {
  if (!material.paint) return [];
  const paint = material.paint;
  const refs = new Map<string, LayerTexture>();
  const add = (url: string | undefined, role: LayerTextureRole) => {
    if (url) refs.set(`${role}\0${url}`, { url, role });
  };
  for (const slot of paint.slots) {
    add(slot.baseColorUrl, "color");
    add(slot.normalUrl, "normal");
    add(slot.mraUrl, "data");
    if (slot.emissiveIntensity !== 0)
      add(slot.emissiveUrl, roleFor(slot.emissiveColorSpace));
  }
  add(paint.height.url, roleFor(paint.height.sourceColorSpace));
  add(paint.overlay.url, roleFor(paint.overlay.sourceColorSpace));
  return [...refs.values()];
}

export function packPaint(
  material: Pick<CompiledMaterial, "paint">,
  poolIndex: (url: string, role: LayerTextureRole) => number,
): Float32Array {
  const result = new Float32Array(PAINT_FLOAT_COUNT);
  const paint = material.paint;
  if (!paint) return result;
  const index = (url: string | undefined, role: LayerTextureRole) => {
    if (!url) return 0;
    const value = poolIndex(url, role);
    if (!Number.isInteger(value) || value < 0 || value > 0xffffff)
      throw new RangeError("Invalid paint texture pool index");
    return value;
  };
  const [a, b] = paint.slots;
  result.set(
    [
      1,
      paint.uv.mode === "objectYZ" ? 2 : paint.uv.mode === "objectXY" ? 1 : 0,
      paint.height.contrast,
      Number(paint.overlay.useAlpha),
    ],
    0,
  );
  result.set([...paint.uv.scale, ...paint.height.uvScale], 4);
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
      index(paint.height.url, roleFor(paint.height.sourceColorSpace)),
      index(paint.overlay.url, roleFor(paint.overlay.sourceColorSpace)),
    ],
    20,
  );
  result.set(
    [
      a.emissiveIntensity !== 0
        ? index(a.emissiveUrl, roleFor(a.emissiveColorSpace))
        : 0,
      b.emissiveIntensity !== 0
        ? index(b.emissiveUrl, roleFor(b.emissiveColorSpace))
        : 0,
      Number(paint.height.sourceColorSpace === "srgb"),
      Number(paint.overlay.sourceColorSpace === "srgb"),
    ],
    24,
  );
  result.set(
    [
      paint.baseRoughnessIntensity,
      paint.overlay.colorIntensity,
      paint.overlay.roughnessIntensity,
      0,
    ],
    28,
  );
  result.set(
    [
      a.emissiveIntensity,
      b.emissiveIntensity,
      Number(a.emissiveColorSpace === "srgb"),
      Number(b.emissiveColorSpace === "srgb"),
    ],
    32,
  );
  if (!result.every(Number.isFinite))
    throw new RangeError("Paint material parameters must be finite");
  return result;
}

/** Reuses the shared role arrays; UV1 is independent of mesh tiling and object scale. */
// The source roughness root selects R. BumpOffset uses Eye.xy without perspective division.
export const WGSL_PAINT_FUNCTIONS = `struct PaintSample{base:vec3<f32>,tangentNormal:vec3<f32>,mra:vec3<f32>,emission:vec3<f32>};fn paintHeightAlpha(height:f32,phase:f32,contrast:f32)->f32{let first=clamp(height-1.0+2.0*phase,0.0,1.0);return clamp(first*(1.0+2.0*contrast)-contrast,0.0,1.0);}fn paintMap(index:f32,isColor:f32,uv:vec2<f32>)->vec4<f32>{if isColor>0.5{return textureSample(layerColors,texSampler,uv,i32(index));}return textureSample(layerData,texSampler,uv,i32(index));}fn paintOverlay(base:vec3<f32>,blend:vec3<f32>)->vec3<f32>{return select(2.0*base*blend,1.0-2.0*(1.0-base)*(1.0-blend),base>vec3(0.5));}fn evaluatePaint(rawUV:vec2<f32>,uv1:vec2<f32>,vertexColor:vec4<f32>,objectScale:vec3<f32>)->PaintSample{var uv=rawUV*material.paintUV.xy;let scales=abs(objectScale);if material.paintConfig.y>1.5{uv=rawUV*scales.zy;}else if material.paintConfig.y>0.5{uv=rawUV*scales.xz;}let height=paintMap(material.paintIndices1.z,material.paintIndices2.z,uv*material.paintUV.zw).r;let blue=paintHeightAlpha(height,vertexColor.b,material.paintConfig.z);let green=paintHeightAlpha(height,vertexColor.g,material.paintConfig.z);let a=textureSample(layerColors,texSampler,uv,i32(material.paintIndices0.x)).rgb*material.paintTint0.rgb;let b=textureSample(layerColors,texSampler,uv,i32(material.paintIndices0.w)).rgb*material.paintTint1.rgb;let base=mix(a,b,blue);let normal0=decodeNormalSample(textureSample(layerNormals,texSampler,uv,i32(material.paintIndices0.y)).xyz,material.maps.w);let normal1=decodeNormalSample(textureSample(layerNormals,texSampler,uv,i32(material.paintIndices1.x)).xyz,material.maps.w);let mra0=textureSample(layerData,texSampler,uv,i32(material.paintIndices0.z)).rgb;let mra1=textureSample(layerData,texSampler,uv,i32(material.paintIndices1.y)).rgb*vec3(1.0,material.paintValues.x,1.0);var mra=mix(mra0,mra1,blue);let overlay=paintMap(material.paintIndices1.w,material.paintIndices2.w,uv1);let gate=select(1.0,overlay.a,material.paintConfig.w>0.5);let painted=mix(base,paintOverlay(overlay.rgb,base),material.paintValues.y);let color=mix(base,mix(base,painted,gate),green);let overlayRough=mix(mra.g,overlay.r,material.paintValues.z);mra.g=mix(mra.g,mix(mra.g,overlayRough,gate),green);var emission0=vec3(0.0);var emission1=vec3(0.0);if material.paintEmission.x!=0.0{emission0=paintMap(material.paintIndices2.x,material.paintEmission.z,uv).rgb*material.paintEmission.x;}if material.paintEmission.y!=0.0{emission1=paintMap(material.paintIndices2.y,material.paintEmission.w,uv).rgb*material.paintEmission.y;}return PaintSample(color,mix(normal0,normal1,blue),mra,mix(emission0,emission1,blue));}`;
