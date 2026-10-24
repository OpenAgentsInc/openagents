import type { CompiledMaterial } from "../assets/src/scene.ts";
import type { LayerTexture, LayerTextureRole } from "./layers.ts";
import { validateWindFlutter, type WindFlutter } from "./wind-flutter.ts";
import { foliageTransmissionCode } from "./foliage-transmission.ts";

export const VEGETATION_FLOAT_COUNT = 48;
export const VEGETATION_SURFACE_SKY_MARKER = 39;
/** vegetationWaves.w: zero legacy, 1+g owned bounded angular transmission. */
export const VEGETATION_TRANSMISSION_OFFSET = 47;
export const WGSLMaterialVegetationFields = `,
 vegetationConfig:vec4<f32>,vegetationMaps:vec4<f32>,vegetationTint:vec4<f32>,vegetationOverlay:vec4<f32>,vegetationLuminance:vec4<f32>,
 vegetationValues:vec4<f32>,vegetationFuzz:vec4<f32>,vegetationMaskColor:vec4<f32>,vegetationSubsurface:vec4<f32>,vegetationFlags:vec4<f32>,
 vegetationFlutter:vec4<f32>,vegetationWaves:vec4<f32>`;

export function collectVegetationTextures(
  material: Pick<CompiledMaterial, "vegetation">,
): LayerTexture[] {
  const vegetation = material.vegetation;
  if (!vegetation) return [];
  const refs: LayerTexture[] = [
    { url: vegetation.baseColorUrl, role: "color" },
    { url: vegetation.normalUrl, role: "normal" },
    { url: vegetation.mraUrl, role: "data" },
  ];
  if (vegetation.mask.enabled)
    refs.push({
      url: vegetation.mask.url,
      role: vegetation.mask.sourceColorSpace === "srgb" ? "color" : "data",
    });
  return refs.filter(
    (ref, index) =>
      refs.findIndex(
        (value) => value.url === ref.url && value.role === ref.role,
      ) === index,
  );
}

export function packVegetation(
  material: Pick<CompiledMaterial, "vegetation">,
  poolIndex: (url: string, role: LayerTextureRole) => number,
  flutter?: WindFlutter,
): Float32Array {
  const result = new Float32Array(VEGETATION_FLOAT_COUNT);
  const vegetation = material.vegetation;
  if (flutter) {
    validateWindFlutter(flutter);
    if (!vegetation)
      throw Error("Flutter requires a prepared vegetation color map.");
    result.set([1, flutter.intensity, flutter.weight, flutter.clockScale], 40);
    result.set(
      [flutter.smallWaveCm, flutter.largeWaveCm, flutter.vertexLod, 0],
      44,
    );
  }
  if (!vegetation) return result;
  const transmissionCode = foliageTransmissionCode(vegetation.transmission);
  const index = (url: string, role: LayerTextureRole) => {
    const value = poolIndex(url, role);
    if (!Number.isInteger(value) || value < 0 || value > 0xffffff)
      throw new RangeError("Invalid vegetation texture pool index");
    return value;
  };
  result.set(
    [
      1,
      Number(vegetation.gradient.axis === "v"),
      Number(vegetation.mask.enabled),
      vegetation.alphaCutoff,
    ],
    0,
  );
  result.set(
    [
      index(vegetation.baseColorUrl, "color"),
      index(vegetation.normalUrl, "normal"),
      index(vegetation.mraUrl, "data"),
      vegetation.mask.enabled
        ? index(
            vegetation.mask.url,
            vegetation.mask.sourceColorSpace === "srgb" ? "color" : "data",
          )
        : 0,
    ],
    4,
  );
  result.set([...vegetation.tint, vegetation.color.desaturation], 8);
  result.set(
    [...vegetation.color.overlayColor, vegetation.color.overlayStrength],
    12,
  );
  result.set([...vegetation.color.luminance, vegetation.color.contrast], 16);
  result.set(
    [
      vegetation.color.intensity,
      vegetation.aoAffectsColorIntensity,
      vegetation.gradient.intensity,
      vegetation.normalFlatness,
    ],
    20,
  );
  result.set(
    [
      vegetation.fuzz.coreDarkness,
      vegetation.fuzz.power,
      vegetation.fuzz.edgeBrightness,
      vegetation.roughnessIntensity,
    ],
    24,
  );
  result.set([...vegetation.mask.color, vegetation.specularAmount], 28);
  result.set(
    [...vegetation.subsurface.color, vegetation.subsurface.intensity],
    32,
  );
  result.set(
    [
      Number(vegetation.mask.sourceColorSpace === "srgb"),
      Number(vegetation.sourceWindEnabled),
      Number(vegetation.twoSided),
      0,
    ],
    36,
  );
  result[VEGETATION_TRANSMISSION_OFFSET] = transmissionCode;
  if (!result.every(Number.isFinite))
    throw new RangeError("Vegetation material parameters must be finite");
  return result;
}

/** Source color graph only. Subsurface lighting and wind need separate runtime support. */
export const WGSL_VEGETATION_FUNCTIONS = `
struct VegetationSample{base:vec3<f32>,tangentNormal:vec3<f32>,mra:vec3<f32>,opacity:f32,specular:f32,subsurface:vec3<f32>,fuzz:f32};
fn vegetationOverlay(base:vec3<f32>,blend:vec3<f32>)->vec3<f32>{
 return select(2.0*base*blend,1.0-2.0*(1.0-base)*(1.0-blend),base>vec3(0.5));
}
fn vegetationPower(base:vec3<f32>,exponent:f32)->vec3<f32>{
 if exponent==0.0{return vec3(1.0);}
 return pow(max(base,vec3(0.0)),vec3(exponent));
}
// viewTangent uses the unflipped source tangent frame; faceSign applies only once.
fn evaluateVegetation(rawUV:vec2<f32>,vertexColor:vec4<f32>,viewTangent:vec3<f32>,faceSign:f32)->VegetationSample{
 let color=textureSample(layerColors,texSampler,rawUV,i32(material.vegetationMaps.x));
 let normal=decodeNormalSample(textureSample(layerNormals,texSampler,rawUV,i32(material.vegetationMaps.y)).xyz,material.maps.w);
 let mra=textureSample(layerData,texSampler,rawUV,i32(material.vegetationMaps.z)).rgb;
 let tinted=color.rgb*material.vegetationTint.rgb;
 let desaturated=mix(tinted,vec3(dot(tinted,material.vegetationLuminance.rgb)),material.vegetationTint.w);
 let overlaid=clamp(vegetationOverlay(desaturated,material.vegetationOverlay.rgb),vec3(0.0),vec3(1.0));
 let adjusted=vegetationPower(mix(desaturated,overlaid,material.vegetationOverlay.w),material.vegetationLuminance.w)*material.vegetationValues.x;
 let ndv=clamp(dot(viewTangent,vec3(normal.xy,normal.z*faceSign)),0.0,1.0);
 var edge=pow(max(1.0-ndv,0.0),material.vegetationFuzz.y);
 if material.vegetationFuzz.y==0.0{edge=1.0;}
 let fuzz=1.0-ndv*material.vegetationFuzz.x+edge*material.vegetationFuzz.z;
 let shaded=clamp(adjusted*fuzz*mra.b*material.vegetationValues.y,vec3(0.0),vec3(1.0));
 let gradient=select(rawUV.x,rawUV.y,material.vegetationConfig.y>0.5)*material.vegetationValues.z;
 var base=shaded*(1.0-gradient);
 if material.vegetationConfig.z>0.5{
  var mask=0.0;
  if material.vegetationFlags.x>0.5{mask=textureSample(layerColors,texSampler,rawUV,i32(material.vegetationMaps.w)).r;}
  else{mask=textureSample(layerData,texSampler,rawUV,i32(material.vegetationMaps.w)).r;}
  let masked=clamp(vegetationOverlay(color.rgb,material.vegetationMaskColor.rgb),vec3(0.0),vec3(1.0));
  base=mix(base,masked,mask);
 }
 let surfaceNormal=mix(normal,vec3(0.0,0.0,1.0),material.vegetationValues.w);
 let roughness=clamp(mra.g*material.vegetationFuzz.w,0.0,1.0);
 let specular=mix(0.15,0.5,color.b*vertexColor.g*vertexColor.g)*material.vegetationMaskColor.w;
 let subsurface=clamp(color.rgb*material.vegetationSubsurface.rgb,vec3(0.0),vec3(1.0))*material.vegetationSubsurface.w;
 return VegetationSample(base,surfaceNormal,vec3(mra.r,roughness,mra.b),color.a,specular,subsurface,fuzz);
}
`;
