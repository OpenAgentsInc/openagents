import type { CompiledMaterial } from "../assets/src/scene.ts";
import type { LayerTexture, LayerTextureRole } from "./layers.ts";

export const GLASS_FLOAT_COUNT = 52;

/** Append after the layer fields. Existing supplemental texture arrays are reused. */
export const WGSLMaterialGlassFields = `,
 glassConfig:vec4<f32>,glassColor:vec4<f32>,glassDirtColor:vec4<f32>,
 glassOpacity:vec4<f32>,glassRoughness:vec4<f32>,glassNormal:vec4<f32>,
 glassMaps:vec4<f32>,glassUVWorld:vec4<f32>,glassFlags:vec4<f32>,
 glassTextureColor:vec4<f32>,glassEmission:vec4<f32>,flameTint:vec4<f32>,glassClamp:vec4<f32>`;

const maskRole = (space: "srgb" | "linear"): LayerTextureRole =>
  space === "srgb" ? "color" : "data";

export function collectGlassTextures(
  material: Pick<CompiledMaterial, "glass" | "flame">,
): LayerTexture[] {
  const result = new Map<string, LayerTexture>();
  const add = (url: string | undefined, role: LayerTextureRole) => {
    if (url) result.set(`${role}\0${url}`, { url, role });
  };
  if (material.glass) {
    const glass = material.glass;
    add(glass.dirt.url, maskRole(glass.dirt.sourceColorSpace));
    add(glass.dirt.world.url, maskRole(glass.dirt.world.sourceColorSpace));
    if (glass.normal.enabled) add(glass.normal.url, "normal");
  }
  if (material.flame) add(material.flame.colorUrl, "color");
  return [...result.values()];
}

export function packGlass(
  material: Pick<CompiledMaterial, "glass" | "flame">,
  poolIndex: (url: string, role: LayerTextureRole) => number,
): Float32Array {
  const result = new Float32Array(GLASS_FLOAT_COUNT);
  if (material.glass && material.flame)
    throw new TypeError(
      "A material cannot use glass and flame profiles together",
    );
  const index = (url: string | undefined, role: LayerTextureRole) => {
    if (!url) return 0;
    const value = poolIndex(url, role);
    if (!Number.isInteger(value) || value < 0 || value > 0xffffff)
      throw new RangeError("Invalid surface texture pool index");
    return value;
  };
  const glass = material.glass;
  if (glass) {
    result.set(
      [1, 0, Number(glass.dirt.useForColor), Number(glass.opacity.useTexture)],
      0,
    );
    result.set([...glass.color, glass.colorAmount], 4);
    result.set([...glass.dirtColor, glass.metallic], 8);
    result.set(
      [
        glass.opacity.base,
        glass.opacity.side1,
        glass.opacity.side2,
        glass.opacity.fresnelPower,
      ],
      12,
    );
    result.set(
      [
        glass.roughness.constant,
        glass.roughness.main,
        glass.roughness.intensity,
        glass.roughness.power,
      ],
      16,
    );
    result.set([...glass.normal.constant, glass.normal.flatness], 20);
    result.set(
      [
        index(glass.dirt.url, maskRole(glass.dirt.sourceColorSpace)),
        index(
          glass.dirt.world.url,
          maskRole(glass.dirt.world.sourceColorSpace),
        ),
        glass.normal.enabled ? index(glass.normal.url, "normal") : 0,
        0,
      ],
      24,
    );
    result.set(
      [...glass.uvScale, glass.dirt.world.sizeMeters, glass.dirt.world.power],
      28,
    );
    result.set(
      [
        Number(glass.dirt.sourceColorSpace === "srgb"),
        Number(glass.dirt.world.sourceColorSpace === "srgb"),
        Number(glass.normal.enabled),
        Number(glass.roughness.useDirt),
      ],
      32,
    );
    result.set([...glass.textureColor, glass.opacity.textureValue], 36);
    result.set([...glass.emissiveColor, glass.emissiveIntensity], 40);
  }
  const flame = material.flame;
  if (flame) {
    result[1] = 1;
    result[27] = index(flame.colorUrl, "color");
    result.set([...flame.tint, flame.emissiveIntensity], 44);
  }
  // These are renderer bounds, not source graph parameters or exposure values.
  if (glass || flame) result.set([0, 1, 0, 1], 48);
  if (!result.every(Number.isFinite))
    throw new RangeError("Surface material parameters must be finite");
  return result;
}

/** Requires layerColors/layerNormals/layerData, texSampler, and material uniforms. */
// Constant normals are tangent-space vectors. Additive flames apply sampled alpha
// once in blending; their raw emission keeps the source units.
export const WGSL_GLASS_FUNCTIONS = `struct GlassSample{base:vec3<f32>,tangentNormal:vec3<f32>,mra:vec3<f32>,opacity:f32,emission:vec3<f32>};fn glassPow(base:f32,exponent:f32)->f32{if exponent==0.0{return 1.0;}if base<=0.0 && exponent>0.0{return 0.0;}return pow(max(base,0.00000001),exponent);}fn glassMask(index:f32,isColor:f32,uv:vec2<f32>)->vec4<f32>{if isColor>0.5{return textureSample(layerColors,texSampler,uv,i32(index));}return textureSample(layerData,texSampler,uv,i32(index));}fn glassWorldGreen(worldPos:vec3<f32>,geometricNormal:vec3<f32>)->f32{let p=-worldPos.xzy/max(abs(material.glassUVWorld.z),0.00000001);let n=geometricNormal.xzy;let xz=glassMask(material.glassMaps.y,material.glassFlags.y,p.xz).g;let yz=glassMask(material.glassMaps.y,material.glassFlags.y,p.yz).g;let xy=glassMask(material.glassMaps.y,material.glassFlags.y,p.xy).g;let alongX=clamp(3.0*abs(n.x)-1.0,0.0,1.0);let alongZ=clamp(3.0*abs(n.z)-1.0,0.0,1.0);return mix(mix(xz,yz,alongX),xy,alongZ);}fn evaluateGlass(rawUV:vec2<f32>,worldPos:vec3<f32>,worldGeometricNormal:vec3<f32>,viewDirection:vec3<f32>)->GlassSample{let uv=rawUV*material.glassUVWorld.xy;let mask=glassMask(material.glassMaps.x,material.glassFlags.x,uv);let world=glassWorldGreen(worldPos,worldGeometricNormal);let dirt=mix(glassPow(world,material.glassUVWorld.w),mask.r,0.5);var base=material.glassColor.rgb;if material.glassConfig.z>0.5{base=mix(base,material.glassDirtColor.rgb,dirt)+vec3(material.glassColor.w);if material.glassConfig.w>0.5{base=mix(base,material.glassTextureColor.rgb,mask.b);}}var roughness=material.glassRoughness.x;if material.glassFlags.w>0.5{roughness=mix(material.glassRoughness.y,glassPow(dirt*material.glassRoughness.z,material.glassRoughness.w),dirt);}var normal=material.glassNormal.xyz;if material.glassFlags.z>0.5{let sampled=decodeNormalSample(textureSample(layerNormals,texSampler,uv,i32(material.glassMaps.z)).xyz,material.maps.w);normal=mix(sampled,vec3(0.0,0.0,1.0),material.glassNormal.w);}let fresnel=glassPow(1.0-clamp(dot(normalize(worldGeometricNormal),normalize(viewDirection)),0.0,1.0),material.glassOpacity.w);var opacity=material.glassOpacity.x+mix(material.glassOpacity.y,material.glassOpacity.z,fresnel);if material.glassConfig.w>0.5{opacity=mix(opacity,material.glassTextureColor.w,mask.b);}return GlassSample(base,normal,vec3(material.glassDirtColor.w,clamp(roughness,material.glassClamp.z,material.glassClamp.w),1.0),clamp(opacity,material.glassClamp.x,material.glassClamp.y),material.glassEmission.rgb*material.glassEmission.w);}fn evaluateFlame(rawUV:vec2<f32>)->GlassSample{let color=textureSample(layerColors,texSampler,rawUV,i32(material.glassMaps.w));return GlassSample(vec3(0.0),vec3(0.0,0.0,1.0),vec3(0.0,1.0,1.0),color.a,color.rgb*material.flameTint.rgb*material.flameTint.w);}`;
