import type { CompiledMaterial } from "../assets/src/scene.ts";
import type { LayerTexture, LayerTextureRole } from "./layers.ts";

export const TERRAIN_FLOAT_COUNT = 48;
export const WGSLMaterialTerrainFields = `,
 terrainConfig:vec4<f32>,terrainUV:vec4<f32>,terrainTint0:vec4<f32>,terrainTint1:vec4<f32>,terrainTint2:vec4<f32>,
 terrainIndices0:vec4<f32>,terrainIndices1:vec4<f32>,terrainIndices2:vec4<f32>,terrainIndices3:vec4<f32>,
 terrainValues:vec4<f32>,terrainParallax:vec4<f32>,terrainEmission:vec4<f32>`;
const roleFor = (space: "linear" | "srgb"): LayerTextureRole =>
  space === "srgb" ? "color" : "data";

export function collectTerrainTextures(
  material: Pick<CompiledMaterial, "terrain">,
): LayerTexture[] {
  const terrain = material.terrain;
  if (!terrain) return [];
  const refs = new Map<string, LayerTexture>();
  const add = (url: string | undefined, role: LayerTextureRole) => {
    if (url) refs.set(`${role}\0${url}`, { url, role });
  };
  for (const slot of terrain.slots) {
    add(slot.baseColorUrl, "color");
    add(slot.normalUrl, "normal");
    add(slot.mraUrl, "data");
    if (slot.emissiveIntensity !== 0)
      add(slot.emissiveUrl, roleFor(slot.emissiveColorSpace));
  }
  add(terrain.height.url, roleFor(terrain.height.sourceColorSpace));
  add(terrain.wetness.url, roleFor(terrain.wetness.sourceColorSpace));
  if (terrain.parallax.enabled)
    add(terrain.parallax.url, roleFor(terrain.parallax.sourceColorSpace));
  return [...refs.values()];
}

export function packTerrain(
  material: Pick<CompiledMaterial, "terrain">,
  poolIndex: (url: string, role: LayerTextureRole) => number,
): Float32Array {
  const result = new Float32Array(TERRAIN_FLOAT_COUNT);
  const terrain = material.terrain;
  if (!terrain) return result;
  const index = (url: string | undefined, role: LayerTextureRole) => {
    if (!url) return 0;
    const value = poolIndex(url, role);
    if (!Number.isInteger(value) || value < 0 || value > 0xffffff)
      throw new RangeError("Invalid terrain texture pool index");
    return value;
  };
  const [a, b, c] = terrain.slots;
  result.set(
    [
      1,
      terrain.uv.mode === "objectYZ"
        ? 2
        : terrain.uv.mode === "objectXY"
          ? 1
          : 0,
      terrain.height.contrast,
      Number(terrain.height.useCustom),
    ],
    0,
  );
  result.set([...terrain.uv.scale, ...terrain.height.uvScale], 4);
  result.set(a.colorFactor, 8);
  result.set(b.colorFactor, 12);
  result.set(c.colorFactor, 16);
  result.set(
    [
      index(a.baseColorUrl, "color"),
      index(a.normalUrl, "normal"),
      index(a.mraUrl, "data"),
      index(b.baseColorUrl, "color"),
    ],
    20,
  );
  result.set(
    [
      index(b.normalUrl, "normal"),
      index(b.mraUrl, "data"),
      index(c.baseColorUrl, "color"),
      index(c.normalUrl, "normal"),
    ],
    24,
  );
  result.set(
    [
      index(c.mraUrl, "data"),
      index(terrain.height.url, roleFor(terrain.height.sourceColorSpace)),
      index(terrain.wetness.url, roleFor(terrain.wetness.sourceColorSpace)),
      terrain.parallax.enabled
        ? index(
            terrain.parallax.url,
            roleFor(terrain.parallax.sourceColorSpace),
          )
        : 0,
    ],
    28,
  );
  result.set(
    [
      a.emissiveIntensity !== 0
        ? index(a.emissiveUrl, roleFor(a.emissiveColorSpace))
        : 0,
      b.emissiveIntensity !== 0
        ? index(b.emissiveUrl, roleFor(b.emissiveColorSpace))
        : 0,
      Number(terrain.height.sourceColorSpace === "srgb"),
      Number(terrain.wetness.sourceColorSpace === "srgb"),
    ],
    32,
  );
  result.set(
    [
      terrain.baseRoughnessIntensity,
      terrain.wetness.colorIntensity,
      terrain.wetness.roughnessIntensity,
      terrain.height.customIntensity,
    ],
    36,
  );
  result.set(
    [
      Number(terrain.parallax.enabled),
      terrain.parallax.referencePlane,
      terrain.parallax.heightRatio,
      Number(terrain.parallax.sourceColorSpace === "srgb"),
    ],
    40,
  );
  result.set(
    [
      a.emissiveIntensity,
      b.emissiveIntensity,
      Number(a.emissiveColorSpace === "srgb"),
      Number(b.emissiveColorSpace === "srgb"),
    ],
    44,
  );
  if (!result.every(Number.isFinite))
    throw new RangeError("Terrain material parameters must be finite");
  return result;
}

/** tangentView is the normalized surface-to-camera vector in the routed UV frame. */
export const WGSL_TERRAIN_FUNCTIONS = `struct TerrainSample{base:vec3<f32>,tangentNormal:vec3<f32>,mra:vec3<f32>,emission:vec3<f32>};fn terrainHeightAlpha(height:f32,phase:f32,contrast:f32)->f32{let first=clamp(height-1.0+2.0*phase,0.0,1.0);return clamp(first*(1.0+2.0*contrast)-contrast,0.0,1.0);}fn terrainMap(index:f32,isColor:f32,uv:vec2<f32>)->vec4<f32>{if isColor>0.5{return textureSample(layerColors,texSampler,uv,i32(index));}return textureSample(layerData,texSampler,uv,i32(index));}fn evaluateTerrain(rawUV:vec2<f32>,vertexColor:vec4<f32>,objectScale:vec3<f32>,tangentView:vec3<f32>)->TerrainSample{var uv=rawUV*material.terrainUV.xy;let scales=abs(objectScale);if material.terrainConfig.y>1.5{uv=rawUV*scales.zy;}else if material.terrainConfig.y>0.5{uv=rawUV*scales.xz;}if material.terrainParallax.x>0.5{let height=terrainMap(material.terrainIndices2.w,material.terrainParallax.w,rawUV).a;uv+=tangentView.xy*(height-material.terrainParallax.y)*material.terrainParallax.z;}let color0=textureSample(layerColors,texSampler,uv,i32(material.terrainIndices0.x));let color1=textureSample(layerColors,texSampler,uv,i32(material.terrainIndices0.w));let color2=textureSample(layerColors,texSampler,uv,i32(material.terrainIndices1.z));let rawHeight=terrainMap(material.terrainIndices2.y,material.terrainIndices3.z,uv*material.terrainUV.zw).r;var height=rawHeight;if material.terrainConfig.w>0.5{height=mix(rawHeight,1.0-color1.a,(1.0-color1.a)*material.terrainValues.w);}let blue=terrainHeightAlpha(height,vertexColor.b,material.terrainConfig.z);let red=terrainHeightAlpha(rawHeight,vertexColor.r,material.terrainConfig.z);let green=terrainHeightAlpha(height,vertexColor.g,material.terrainConfig.z);let blueColor=mix(color0.rgb*material.terrainTint0.rgb,color1.rgb*material.terrainTint1.rgb,blue);let redColor=mix(color2.rgb*material.terrainTint2.rgb,blueColor,red);let normal0=decodeNormalSample(textureSample(layerNormals,texSampler,uv,i32(material.terrainIndices0.y)).xyz,material.maps.w);let normal1=decodeNormalSample(textureSample(layerNormals,texSampler,uv,i32(material.terrainIndices1.x)).xyz,material.maps.w);let normal2=decodeNormalSample(textureSample(layerNormals,texSampler,uv,i32(material.terrainIndices1.w)).xyz,material.maps.w);let normal=mix(normal2,mix(normal0,normal1,blue),red);let mra0=textureSample(layerData,texSampler,uv,i32(material.terrainIndices0.z)).rgb;let mra1=textureSample(layerData,texSampler,uv,i32(material.terrainIndices1.y)).rgb*vec3(1.0,material.terrainValues.x,1.0);let mra2=textureSample(layerData,texSampler,uv,i32(material.terrainIndices2.x)).rgb;var mra=mix(mra2,mix(mra0,mra1,blue),red);let wet=terrainMap(material.terrainIndices2.z,material.terrainIndices3.w,uv);let wetColor=mix(wet.rgb,blueColor,mra0.g*material.terrainValues.y);let base=mix(wetColor,redColor,green);let wetRough=mix(mra.g,mix(mra.g,wet.r,material.terrainValues.z),wet.a);mra.g=mix(wetRough,mra.g,green);var emission0=vec3(0.0);var emission1=vec3(0.0);if material.terrainEmission.x!=0.0{emission0=terrainMap(material.terrainIndices3.x,material.terrainEmission.z,uv).rgb*material.terrainEmission.x;}if material.terrainEmission.y!=0.0{emission1=terrainMap(material.terrainIndices3.y,material.terrainEmission.w,uv).rgb*material.terrainEmission.y;}return TerrainSample(base,normal,mra,mix(emission0,emission1,blue));}`;
