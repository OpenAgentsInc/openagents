import { ENVIRONMENT_RESPONSE_WGSL } from "./environment-response.ts";
import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";

/** GPU inputs for the bounded SH9 / GGX split-sum environment profile. */
export interface IblMip {
  level: number;
  width: number;
  height: number;
  roughness: number;
  url: string;
  format: "rgba16float";
}
export interface IblContribution {
  diffuseSH9: number[];
  specularMips: IblMip[];
}
export interface IblManifest {
  schema: "aethyrforge.private-environment-lighting.v1";
  profile: "sh9-ggx-split-sum-v1";
  brdfLut: {
    url: string;
    width: number;
    height: number;
    format: "rgba16float";
  };
  scenarios: Partial<
    Record<
      "day" | "night",
      IblContribution & {
        gain?: { sourceSkyLightIntensity: number; baked: false };
        worldToEnvironment: number[];
        constantFog?: IblContribution & {
          source: {
            url: string;
            sha256: string;
            width: number;
            height: number;
          };
        };
        tint: number[];
        brightness: number;
        previewScale: number;
      }
    >
  >;
}

function validCoefficients(value: number[]) {
  return (
    Array.isArray(value) && value.length === 36 && value.every(Number.isFinite)
  );
}
function validateMips(mips: IblMip[], reference?: IblMip[]) {
  const base = mips?.[0];
  if (
    !Array.isArray(mips) ||
    !base ||
    mips.length > 9 ||
    !Number.isInteger(base.width) ||
    base.width < 8 ||
    base.width > 256 ||
    (base.width & (base.width - 1)) !== 0 ||
    base.height !== base.width / 2
  )
    throw Error("Use bounded environment reflection maps.");
  for (const [index, mip] of mips.entries())
    if (
      mip.level !== index ||
      mip.format !== "rgba16float" ||
      mip.width !== Math.max(1, base.width >> index) ||
      mip.height !== Math.max(1, base.height >> index) ||
      !Number.isFinite(mip.roughness) ||
      Math.abs(mip.roughness - index / Math.max(1, mips.length - 1)) > 0.00001
    )
      throw Error(
        "Environment reflection levels must have matching roughness.",
      );
  if (
    reference &&
    (mips.length !== reference.length ||
      mips.some(
        (mip, i) =>
          mip.width !== reference[i].width ||
          mip.height !== reference[i].height ||
          mip.level !== reference[i].level ||
          mip.roughness !== reference[i].roughness,
      ))
  )
    throw Error(
      "The constant environment must use the same reflection levels.",
    );
}

export class IblTextures {
  readonly scenarios = new Map<
    string,
    {
      texture: GPUTexture;
      sh: Float32Array;
      constantSH: Float32Array;
      hasConstantFog: boolean;
      rotation: Float32Array;
      mipMax: number;
    }
  >();
  readonly textures: GPUTexture[] = [];
  bytes = 0;
  private constructor(
    readonly fallback: GPUTexture,
    public lut: GPUTexture,
  ) {
    this.textures.push(fallback, lut);
    this.bytes = 24;
  }
  static async create(device: GPUDevice, manifest?: IblManifest) {
    const blank = (layers = 1) => {
      const t = device.createTexture({
        size: [1, 1, layers],
        format: "rgba16float",
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      });
      device.queue.writeTexture(
        { texture: t },
        new Uint16Array(4 * layers),
        { bytesPerRow: 8, rowsPerImage: 1 },
        [1, 1, layers],
      );
      return t;
    };
    const result = new IblTextures(blank(2), blank());
    if (!manifest) return result;
    try {
      if (
        manifest.schema !== "aethyrforge.private-environment-lighting.v1" ||
        manifest.profile !== "sh9-ggx-split-sum-v1"
      )
        throw Error(
          "Prepare environment lighting with the current asset tools.",
        );
      const upload = async (
        texture: GPUTexture,
        level: { url: string; width: number; height: number; format: string },
        mipLevel: number,
        layer = 0,
      ) => {
        if (level.format !== "rgba16float")
          throw Error("Use linear HDR environment lighting maps.");
        const data = await readAsset(level.url, async (response) => {
          if (!response.ok)
            throw Error("The prepared environment lighting map is missing.");
          const data = await response.arrayBuffer();
          if (data.byteLength !== level.width * level.height * 8)
            throw new AssetIntegrityError(
              "The environment lighting map is incomplete.",
            );
          return data;
        });
        const values = new DataView(data);
        for (let i = 0; i < data.byteLength; i += 2)
          if ((values.getUint16(i, true) & 0x7c00) === 0x7c00)
            throw Error("Environment lighting pixels must be finite.");
        device.queue.writeTexture(
          { texture, mipLevel, origin: [0, 0, layer] },
          data,
          { bytesPerRow: level.width * 8, rowsPerImage: level.height },
          [level.width, level.height],
        );
      };
      const { width, height } = manifest.brdfLut;
      if (
        !Number.isInteger(width) ||
        !Number.isInteger(height) ||
        width < 1 ||
        height < 1 ||
        width > 256 ||
        height > 256
      )
        throw Error("Use a bounded environment BRDF table.");
      const lut = device.createTexture({
        size: [width, height],
        format: "rgba16float",
        usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
      });
      result.lut = lut;
      result.textures.push(lut);
      result.bytes += width * height * 8;
      await upload(lut, manifest.brdfLut, 0);
      for (const mode of ["day", "night"] as const) {
        const source = manifest.scenarios[mode];
        if (!source) continue;
        if (
          !validCoefficients(source.diffuseSH9) ||
          !Array.isArray(source.worldToEnvironment) ||
          source.worldToEnvironment.length !== 16 ||
          !source.worldToEnvironment.every(Number.isFinite) ||
          (source.constantFog !== undefined &&
            (!source.constantFog ||
              !validCoefficients(source.constantFog.diffuseSH9)))
        )
          throw Error("The environment lighting coefficients are invalid.");
        validateMips(source.specularMips);
        if (source.constantFog)
          validateMips(source.constantFog.specularMips, source.specularMips);
        const mips = source.specularMips,
          base = mips[0];
        const texture = device.createTexture({
          label: mode + " primary/constant GGX environment",
          size: [base.width, base.height, 2],
          mipLevelCount: mips.length,
          format: "rgba16float",
          usage: GPUTextureUsage.COPY_DST | GPUTextureUsage.TEXTURE_BINDING,
        });
        result.textures.push(texture);
        result.bytes += mips.reduce(
          (n, mip) => n + mip.width * mip.height * 8 * 2,
          0,
        );
        for (const [index, mip] of mips.entries())
          await upload(texture, mip, index, 0);
        if (source.constantFog)
          for (const [index, mip] of source.constantFog.specularMips.entries())
            await upload(texture, mip, index, 1);
        // WebGPU initializes an absent companion layer to zero at every mip.
        result.scenarios.set(mode, {
          texture,
          sh: new Float32Array(source.diffuseSH9),
          constantSH: source.constantFog
            ? new Float32Array(source.constantFog.diffuseSH9)
            : new Float32Array(36),
          hasConstantFog: !!source.constantFog,
          rotation: new Float32Array(source.worldToEnvironment),
          mipMax: mips.length - 1,
        });
      }
      return result;
    } catch (error) {
      result.dispose();
      throw error;
    }
  }
  private disposed = false;
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    for (const texture of this.textures) texture.destroy();
  }
}
export const WGSL_IBL = `
${ENVIRONMENT_RESPONSE_WGSL}
@group(0)@binding(8)var environmentReflection:texture_2d_array<f32>;
@group(0)@binding(9)var environmentBrdf:texture_2d<f32>;
fn environmentDirection(direction:vec3<f32>)->vec3<f32>{return normalize((frame.iblRotation*vec4(direction,0.0)).xyz);}
fn environmentIrradiance(normal:vec3<f32>)->vec3<f32>{
 let d=environmentDirection(normal);let x=d.x;let y=d.y;let z=d.z;
 let basis=array<f32,9>(0.2820947918,0.4886025119*y,0.4886025119*z,0.4886025119*x,1.0925484306*x*y,1.0925484306*y*z,0.3153915653*(3.0*z*z-1.0),1.0925484306*x*z,0.5462742153*(x*x-y*y));
 var sum=vec3(0.0);for(var i=0;i<9;i++){sum+=(frame.iblSH[i].rgb*frame.skyLayers.x+frame.iblConstantSH[i].rgb*frame.skyLayers.y)*basis[i];}return max(sum,vec3(0.0));
}
fn environmentSurfaceWithReflections(world:vec3<f32>,geometric:vec3<f32>,n:vec3<f32>,view:vec3<f32>,base:vec3<f32>,rough:f32,metal:f32,occlusion:f32,dielectric:f32,refinedSky:vec4<f32>,refinedReflection:vec4<f32>)->vec3<f32>{
 let nv=clamp(dot(n,view),0.0,1.0);let f0=mix(vec3(dielectric),base,metal);let f=f0+(max(vec3(1.0-rough),f0)-f0)*pow(1.0-nv,5.0);
 let local=localSkyLightingComponents(world,n,geometric,view,!AF_SELECTIVE_GI||refinedSky.w!=1.0,!AF_SELECTIVE_GI||!(refinedReflection.w>0.5));
 let directSky=mix(mix(environmentIrradiance(n),local.irradiance,local.confidence),refinedSky.rgb,refinedSky.w);
 let diffuse=base*(vec3(1.0)-f)*(1.0-metal)*(directSky+local.skyBounce*local.confidence)*occlusion;
 let halfTexel=0.5/vec2<f32>(textureDimensions(environmentBrdf));let brdf=textureSampleLevel(environmentBrdf,texSampler,clamp(vec2(nv,rough),halfTexel,vec2(1.0)-halfTexel),0.0).rg;
 var reflection=refinedReflection.rgb;
 if !AF_SELECTIVE_GI||!(refinedReflection.w>0.5){
  let d=environmentDirection(reflect(-view,n));let uv=vec2(fract(atan2(d.x,d.z)/6.283185307),acos(clamp(d.y,-1.0,1.0))/3.141592654);
  reflection=max(textureSampleLevel(environmentReflection,environmentSampler,uv,0,rough*frame.iblSettings.y).rgb*frame.skyLayers.x+textureSampleLevel(environmentReflection,environmentSampler,uv,1,rough*frame.iblSettings.y).rgb*frame.skyLayers.y,vec3(0.0));
 }
 var specular=reflection*environmentBrdfWeight(f0,brdf)*occlusion*mix(1.0,local.specularVisibility,local.confidence);
 if refinedReflection.w>0.5{specular=occludedSkyReflection(reflection,local.specularVisibility,local.confidence,refinedReflection)*environmentBrdfWeight(f0,brdf)*occlusion;}
 let bounced=base*(vec3(1.0)-f)*(1.0-metal)*(local.sunBounce*frame.sunColor.rgb*frame.sun.w+local.localBounce*frame.settings.w)*occlusion*local.confidence;
 return (diffuse+specular)*frame.iblGain.rgb*frame.iblGain.w*frame.cloud.z*frame.iblSettings.z+bounced;
}
fn environmentSurfaceWithSky(world:vec3<f32>,geometric:vec3<f32>,n:vec3<f32>,view:vec3<f32>,base:vec3<f32>,rough:f32,metal:f32,occlusion:f32,dielectric:f32,refinedSky:vec4<f32>)->vec3<f32>{
 return environmentSurfaceWithReflections(world,geometric,n,view,base,rough,metal,occlusion,dielectric,refinedSky,vec4(0.0));
}
fn environmentSurface(world:vec3<f32>,geometric:vec3<f32>,n:vec3<f32>,view:vec3<f32>,base:vec3<f32>,rough:f32,metal:f32,occlusion:f32,dielectric:f32)->vec3<f32>{
 return environmentSurfaceWithSky(world,geometric,n,view,base,rough,metal,occlusion,dielectric,vec4(0.0));
}
`;
