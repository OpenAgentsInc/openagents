import type { CreatureFrame } from "./creatures.ts";
import {
  CharacterAnimator,
  checkedCharacterBlob,
  fetchCharacterBytes,
  validateCharacter,
  validateCharacterGeometry,
  validateCharacterPNG,
  characterTextureBytes,
  characterTextureLevels,
  type CharacterTexture,
  type CharacterManifest,
  type CharacterRenderPose,
  type CharacterWeapon,
} from "./character.ts";
import { FOG_HEIGHT_RATE, FOG_SEGMENT_WGSL } from "./fog.ts";
import { modelMatrix } from "./math.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import { ENVIRONMENT_RESPONSE_WGSL } from "./environment-response.ts";
import { MMORPG_FOG_WGSL } from "./mmorpg-fog.ts";

export interface CharacterIbl {
  scenarios: ReadonlyMap<string, { texture: GPUTexture; mipMax: number }>;
  fallback: GPUTexture;
  lut: GPUTexture;
}
export interface CharacterFrame extends CreatureFrame {
  sourceLighting?: boolean;
  shadowMatrix?: ArrayLike<number>;
  specularEnvironment?: {
    gain: readonly number[];
    primaryScale: number;
    constantScale: number;
  };
}
export interface CharacterRendererOptions {
  ibl?: CharacterIbl;
  shadow?: { texture: GPUTexture; sampler: GPUSampler };
}
const IBL_WGSL = /*wgsl*/ `
@group(2)@binding(0)var environmentReflection:texture_2d_array<f32>;
@group(2)@binding(1)var environmentBrdf:texture_2d<f32>;
@group(2)@binding(2)var environmentSampler:sampler;
fn environmentSpecular(n:vec3<f32>,view:vec3<f32>,rough:f32,f0:vec3<f32>)->vec3<f32>{
 if frame.iblParams.w<.5{return vec3(0.0);}
 let nv=clamp(dot(n,view),0.0,1.0);let halfTexel=.5/vec2<f32>(textureDimensions(environmentBrdf));
 let brdf=textureSampleLevel(environmentBrdf,environmentSampler,clamp(vec2(nv,rough),halfTexel,vec2(1.0)-halfTexel),0.0).rg;
 let d=normalize((frame.rotation*vec4(reflect(-view,n),0.0)).xyz);
 let uv=vec2(fract(atan2(d.x,d.z)/6.283185307),acos(clamp(d.y,-1.0,1.0))/3.141592654);
 let reflection=max(textureSampleLevel(environmentReflection,environmentSampler,uv,0,rough*frame.iblParams.x).rgb*frame.iblParams.y+textureSampleLevel(environmentReflection,environmentSampler,uv,1,rough*frame.iblParams.x).rgb*frame.iblParams.z,vec3(0.0));
 return reflection*environmentBrdfWeight(f0,brdf)*frame.iblGain.rgb;
}
`;
const CHARACTER_SHADOW_SAMPLE_WGSL = `
struct CharacterShadowFrame{matrix:mat4x4<f32>,settings:vec4<f32>}
@group(3)@binding(0)var<uniform>characterShadowFrame:CharacterShadowFrame;
@group(3)@binding(1)var characterShadowDepth:texture_depth_2d;
@group(3)@binding(2)var characterShadowSampler:sampler_comparison;
fn characterShadowVisibility(world:vec3<f32>,n:vec3<f32>)->f32{
 let q=characterShadowFrame.matrix*vec4(world+n*.012,1.0);let p=q.xyz/q.w;
 let uv=vec2(p.x*.5+.5,.5-p.y*.5);if any(uv<vec2(0.0))||any(uv>vec2(1.0))||p.z<=0.0||p.z>=1.0{return 1.0;}
 let pixel=vec2(1.0)/vec2<f32>(textureDimensions(characterShadowDepth));var visible=0.0;
 for(var y=-1;y<=1;y++){for(var x=-1;x<=1;x++){visible+=textureSampleCompareLevel(characterShadowDepth,characterShadowSampler,uv+vec2(f32(x),f32(y))*pixel,p.z-.00008);}}
 return visible/9.0;
}`;
export function characterShader(ibl = false, shadow = false) {
  return /*wgsl*/ `
struct Frame {view:mat4x4<f32>,rotation:mat4x4<f32>,eye:vec4<f32>,sun:vec4<f32>,sunColor:vec4<f32>,ambient:vec4<f32>,fog:vec4<f32>,fogColor:vec4<f32>,volumeMedium:vec4<f32>,volumeRange:vec4<f32>,sh:array<vec4<f32>,9>,iblGain:vec4<f32>,iblParams:vec4<f32>,reserved0:vec4<f32>,reserved1:vec4<f32>}
struct Joint {position:mat4x4<f32>,normal:mat4x4<f32>}
struct Actor {model:mat4x4<f32>,joints:vec4<u32>}
struct Material {color:vec4<f32>,settings:vec4<f32>,surface:vec4<f32>,emissive:vec4<f32>}
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var<storage,read>joints:array<Joint>;
@group(0)@binding(2)var<storage,read>actors:array<Actor>;
@group(1)@binding(0)var baseColor:texture_2d<f32>;
@group(1)@binding(1)var baseSampler:sampler;
@group(1)@binding(2)var<uniform>material:Material;
@group(1)@binding(3)var normalMap:texture_2d<f32>;
@group(1)@binding(4)var emissiveMap:texture_2d<f32>;
${FOG_SEGMENT_WGSL}
${MMORPG_FOG_WGSL}
${ENVIRONMENT_RESPONSE_WGSL}
${shadow ? CHARACTER_SHADOW_SAMPLE_WGSL : "fn characterShadowVisibility(world:vec3<f32>,n:vec3<f32>)->f32{return 1.0;}"}
${ibl ? IBL_WGSL : "fn environmentSpecular(n:vec3<f32>,view:vec3<f32>,rough:f32,f0:vec3<f32>)->vec3<f32>{return vec3(0.0);}"}
struct In {@location(0)position:vec3<f32>,@location(1)normal:vec3<f32>,@location(2)uv:vec2<f32>,@location(3)joint:vec4<f32>,@location(4)weight:vec4<f32>}
struct Out {@builtin(position)clip:vec4<f32>,@location(0)world:vec3<f32>,@location(1)normal:vec3<f32>,@location(2)uv:vec2<f32>}
@vertex fn vs(v:In,@builtin(instance_index)instance:u32)->Out{
 let actor=actors[instance];var position=vec4(0.0);var normal=vec4(0.0);
 for(var k=0u;k<4u;k++){let j=joints[actor.joints.x+u32(v.joint[k])];position+=(j.position*vec4(v.position,1.0))*v.weight[k];normal+=(j.normal*vec4(v.normal,0.0))*v.weight[k];}
 let world=actor.model*position;var o:Out;o.clip=frame.view*world;o.world=world.xyz;o.normal=normalize((actor.model*vec4(normal.xyz,0.0)).xyz);o.uv=v.uv;return o;
}
fn diffuse(n:vec3<f32>)->vec3<f32>{
 let v=normalize((frame.rotation*vec4(n,0.0)).xyz);let b=array<f32,9>(.2820947918,.4886025119*v.y,.4886025119*v.z,.4886025119*v.x,1.0925484306*v.x*v.y,1.0925484306*v.y*v.z,.3153915653*(3.0*v.z*v.z-1.0),1.0925484306*v.x*v.z,.5462742153*(v.x*v.x-v.y*v.y));
 var color=frame.ambient.xyz;for(var i=0;i<9;i++){color+=frame.sh[i].xyz*b[i];}return max(color,vec3(0.0));
}
@fragment fn fs(v:Out,@builtin(front_facing)front:bool)->@location(0)vec4<f32>{
 // Derivatives and samples precede any per-fragment discard.
 let q1=dpdx(v.world);let q2=dpdy(v.world);let st1=dpdx(v.uv);let st2=dpdy(v.uv);
 let albedo=textureSample(baseColor,baseSampler,v.uv)*material.color;
 let mapNormal=textureSample(normalMap,baseSampler,v.uv).xyz*2.0-vec3(1.0);
 let emission=textureSample(emissiveMap,baseSampler,v.uv).rgb*material.emissive.rgb;
 if !front&&material.settings.y<.5{discard;}if albedo.a<material.settings.x{discard;}
 let geometric=normalize(select(-v.normal,v.normal,front));var n=geometric;
 if material.settings.z>.5{
  let q1perp=cross(q2,geometric);let q2perp=cross(geometric,q1);
  let tangent=q1perp*st1.x+q2perp*st2.x;let bitangent=q1perp*st1.y+q2perp*st2.y;
  let scale=inverseSqrt(max(max(dot(tangent,tangent),dot(bitangent,bitangent)),1e-20));
  n=normalize((tangent*mapNormal.x+bitangent*mapNormal.y)*scale*material.surface.z+geometric*mapNormal.z);
 }
 let delta=v.world-frame.eye.xyz;let distance=length(delta);let view=-delta*inverseSqrt(max(dot(delta,delta),1e-12));
 let nl=max(dot(n,frame.sun.xyz),0.0);let visibilitySun=characterShadowVisibility(v.world,geometric);var raw=albedo.rgb*(frame.sunColor.rgb*nl*visibilitySun/3.14159265359+diffuse(n));
 if material.settings.w>.5{
  let rough=clamp(material.surface.y,.045,1.0);let metal=material.surface.x;let nv=max(dot(n,view),.0001);
  let hsum=view+frame.sun.xyz;let h=hsum*inverseSqrt(max(dot(hsum,hsum),1e-12));let nh=max(dot(n,h),0.0);let vh=max(dot(view,h),0.0);
  let a=rough*rough;let a2=a*a;let den=nh*nh*(a2-1.0)+1.0;let distribution=a2/(3.14159265359*den*den);
  let visibility=.5/max(nl*sqrt(nv*nv*(1.0-a2)+a2)+nv*sqrt(nl*nl*(1.0-a2)+a2),.00001);
  let f0=mix(vec3(.04),albedo.rgb,metal);let fresnel=directFresnel(f0,vh);
  let direct=((vec3(1.0)-fresnel)*(1.0-metal)*albedo.rgb/3.14159265359+distribution*visibility*fresnel)*frame.sunColor.rgb*nl*visibilitySun;
  let ambientF=f0+(max(vec3(1.0-rough),f0)-f0)*pow(1.0-nv,5.0);
  raw=direct+albedo.rgb*(vec3(1.0)-ambientF)*(1.0-metal)*diffuse(n)+environmentSpecular(n,view,rough,f0);
  if frame.reserved1.w>.5{raw=direct+albedo.rgb*(1.0-metal)*diffuse(n);}
 }
 raw+=emission;
 if frame.reserved1.w>.5{return vec4(mix(raw,mmoFogColor,mmoFog(frame.eye.y/frame.reserved1.w,delta/frame.reserved1.w))*${SCENE_LINEAR_SCALE},albedo.a);}
 let fog=heightFogComposition(frame.eye.y,delta.y/max(distance,.000001),distance,frame.fog.x,frame.fog.y,1.0,0.0,frame.volumeMedium,frame.volumeRange.xy);
 return vec4((raw*fog.x+frame.fogColor.rgb*fog.y)*${SCENE_LINEAR_SCALE},albedo.a);
}`;
}
export const CHARACTER_WGSL = characterShader();
export const CHARACTER_SHADOW_WGSL = `
struct Frame{matrix:mat4x4<f32>}
struct Joint{position:mat4x4<f32>,normal:mat4x4<f32>}
struct Actor{model:mat4x4<f32>,joints:vec4<u32>}
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var<storage,read>joints:array<Joint>;
@group(0)@binding(2)var<storage,read>actors:array<Actor>;
struct In{@location(0)position:vec3<f32>,@location(1)normal:vec3<f32>,@location(2)uv:vec2<f32>,@location(3)joint:vec4<f32>,@location(4)weight:vec4<f32>}
@vertex fn vs(v:In,@builtin(instance_index)i:u32)->@builtin(position)vec4<f32>{let actor=actors[i];var p=vec4(0.0);for(var k=0u;k<4u;k++){p+=(joints[actor.joints.x+u32(v.joint[k])].position*vec4(v.position,1.0))*v.weight[k];}return frame.matrix*actor.model*p;}`;
interface CharacterShadowResources {
  pipeline: GPURenderPipeline;
  receiver: GPUBindGroup;
  receiverBuffer: GPUBuffer;
  views: Map<string, { buffer: GPUBuffer; group: GPUBindGroup }>;
}

const MIP_WGSL = /*wgsl*/ `
@group(0)@binding(0)var image:texture_2d<f32>;@group(0)@binding(1)var linearSampler:sampler;
struct Out{@builtin(position)clip:vec4<f32>,@location(0)uv:vec2<f32>};
@vertex fn vs(@builtin(vertex_index)i:u32)->Out{let p=array<vec2<f32>,3>(vec2(-1.0,-1.0),vec2(3.0,-1.0),vec2(-1.0,3.0));var o:Out;o.clip=vec4(p[i],0.0,1.0);o.uv=vec2(p[i].x*.5+.5,.5-p[i].y*.5);return o;}
@fragment fn base(v:Out)->@location(0)vec4<f32>{return textureSample(image,linearSampler,v.uv);}
@fragment fn normal(v:Out)->@location(0)vec4<f32>{let c=textureSample(image,linearSampler,v.uv);let n=c.xyz*2.0-vec3(1.0);return vec4(n*inverseSqrt(max(dot(n,n),1e-12))*.5+vec3(.5),c.a);}
`;

export interface CharacterInstance {
  position: readonly number[];
  yaw: number;
  palette: Float32Array;
  weapon?: CharacterWeapon | null;
}

/** Shared character geometry and materials in the world's 4x-MSAA HDR pass. */
export class CharacterRenderer {
  readonly bytes: number;
  private animator: CharacterAnimator;
  private settings = new Float32Array(116);
  private disposed = false;
  private actorData: Float32Array<ArrayBuffer>;
  private actorOffsets: Uint32Array<ArrayBuffer>;
  private jointData: Float32Array<ArrayBuffer>;
  private constructor(
    private device: GPUDevice,
    readonly manifest: CharacterManifest,
    private pipeline: GPURenderPipeline,
    private vertices: GPUBuffer,
    private indices: GPUBuffer,
    private frame: GPUBuffer,
    private joints: GPUBuffer,
    private actors: GPUBuffer,
    private capacity: number,
    private group: GPUBindGroup,
    private materials: GPUBindGroup[],
    private ownedBuffers: GPUBuffer[],
    private textures: GPUTexture[],
    private environmentGroups: Map<string, GPUBindGroup>,
    private ibl?: CharacterIbl,
    private shadowResources?: CharacterShadowResources,
  ) {
    this.animator = new CharacterAnimator(manifest);
    this.actorData = new Float32Array(capacity * 20);
    this.actorOffsets = new Uint32Array(this.actorData.buffer);
    this.jointData = new Float32Array(capacity * manifest.joints.length * 32);
    this.bytes =
      manifest.geometry.vertices.byteLength +
      manifest.geometry.indices.byteLength +
      464 +
      capacity * (manifest.joints.length * 128 + 80) +
      manifest.materials.length * 64 +
      (shadowResources ? 272 : 0) +
      textures.reduce(
        (sum, texture) =>
          sum + characterTextureBytes(texture.width, texture.height),
        0,
      );
  }
  static async create(
    device: GPUDevice,
    manifest: CharacterManifest,
    load = fetchCharacterBytes,
    capacity = 1,
    options: CharacterRendererOptions = {},
  ) {
    validateCharacter(manifest);
    if (!Number.isInteger(capacity) || capacity < 1 || capacity > 64)
      throw new Error("The character renderer supports up to 64 actors.");
    const vertexBytes = await checkedCharacterBlob(
        manifest.geometry.vertices,
        load,
      ),
      indexBytes = await checkedCharacterBlob(manifest.geometry.indices, load);
    validateCharacterGeometry(
      manifest,
      new Float32Array(vertexBytes),
      new Uint32Array(indexBytes),
    );
    const module = device.createShaderModule({
      code: characterShader(!!options.ibl, !!options.shadow),
    });
    const pipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: {
        module,
        entryPoint: "vs",
        buffers: [
          {
            arrayStride: 64,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x3" },
              { shaderLocation: 1, offset: 12, format: "float32x3" },
              { shaderLocation: 2, offset: 24, format: "float32x2" },
              { shaderLocation: 3, offset: 32, format: "float32x4" },
              { shaderLocation: 4, offset: 48, format: "float32x4" },
            ],
          },
        ],
      },
      fragment: {
        module,
        entryPoint: "fs",
        targets: [{ format: "rgba16float" }],
      },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: true,
        depthCompare: "less-equal",
      },
      multisample: { count: 4, alphaToCoverageEnabled: true },
    });
    const buffers: GPUBuffer[] = [],
      textures: GPUTexture[] = [];
    const buffer = (
      size: number,
      usage: GPUBufferUsageFlags,
      data?: ArrayBuffer | Float32Array,
    ) => {
      const result = device.createBuffer({
        size,
        usage: usage | GPUBufferUsage.COPY_DST,
      });
      buffers.push(result);
      if (data) device.queue.writeBuffer(result, 0, data);
      return result;
    };
    try {
      const vertices = buffer(
          vertexBytes.byteLength,
          GPUBufferUsage.VERTEX,
          vertexBytes,
        ),
        indices = buffer(
          indexBytes.byteLength,
          GPUBufferUsage.INDEX,
          indexBytes,
        ),
        frame = buffer(464, GPUBufferUsage.UNIFORM),
        joints = buffer(
          capacity * manifest.joints.length * 128,
          GPUBufferUsage.STORAGE,
        ),
        actors = buffer(capacity * 80, GPUBufferUsage.STORAGE);
      const group = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: frame } },
          { binding: 1, resource: { buffer: joints } },
          { binding: 2, resource: { buffer: actors } },
        ],
      });
      const sampler = device.createSampler({
        magFilter: "linear",
        minFilter: "linear",
        mipmapFilter: "linear",
        maxAnisotropy: 4,
        addressModeU: "repeat",
        addressModeV: "repeat",
      });
      const textureCache = new Map<string, GPUTexture>(),
        mipPipelines = new Map<string, GPURenderPipeline>();
      const loadTexture = async (
        description: CharacterTexture | undefined,
        normal: boolean,
      ) => {
        const key = `${normal ? "normal" : "base"}:${description?.sha256 ?? "fallback"}:${description?.width ?? 1}:${description?.height ?? 1}`;
        const cached = textureCache.get(key);
        if (cached) return cached;
        const width = description?.width ?? 1,
          height = description?.height ?? 1,
          levels = characterTextureLevels(width, height),
          format = normal ? "rgba8unorm" : "rgba8unorm-srgb";
        const texture = device.createTexture({
          label: normal ? "Character linear normal" : "Character sRGB base",
          size: [width, height],
          mipLevelCount: levels,
          format,
          usage:
            GPUTextureUsage.TEXTURE_BINDING |
            GPUTextureUsage.COPY_DST |
            GPUTextureUsage.RENDER_ATTACHMENT,
        });
        textures.push(texture);
        if (description) {
          const bytes = await checkedCharacterBlob(description, load);
          validateCharacterPNG(bytes, width, height);
          const image = await createImageBitmap(
            new Blob([bytes], { type: "image/png" }),
            { colorSpaceConversion: "none", premultiplyAlpha: "none" },
          );
          try {
            if (image.width !== width || image.height !== height)
              throw new Error(
                "The character texture has incorrect dimensions.",
              );
            device.queue.copyExternalImageToTexture(
              { source: image },
              { texture },
              [width, height],
            );
          } finally {
            image.close();
          }
        } else
          device.queue.writeTexture(
            { texture },
            normal
              ? new Uint8Array([128, 128, 255, 255])
              : new Uint8Array([255, 255, 255, 255]),
            { bytesPerRow: 4 },
            [1, 1],
          );
        if (levels > 1) {
          let mipPipeline = mipPipelines.get(format);
          if (!mipPipeline) {
            const shader = device.createShaderModule({ code: MIP_WGSL });
            mipPipeline = await device.createRenderPipelineAsync({
              label: normal
                ? "Character normalized normal mips"
                : "Character linear-color mips",
              layout: "auto",
              vertex: { module: shader, entryPoint: "vs" },
              fragment: {
                module: shader,
                entryPoint: normal ? "normal" : "base",
                targets: [{ format }],
              },
              primitive: { topology: "triangle-list" },
            });
            mipPipelines.set(format, mipPipeline);
          }
          const encoder = device.createCommandEncoder({
            label: "Character mipmaps",
          });
          for (let level = 1; level < levels; level++) {
            const group = device.createBindGroup({
              layout: mipPipeline.getBindGroupLayout(0),
              entries: [
                {
                  binding: 0,
                  resource: texture.createView({
                    baseMipLevel: level - 1,
                    mipLevelCount: 1,
                  }),
                },
                { binding: 1, resource: sampler },
              ],
            });
            const pass = encoder.beginRenderPass({
              colorAttachments: [
                {
                  view: texture.createView({
                    baseMipLevel: level,
                    mipLevelCount: 1,
                  }),
                  loadOp: "clear",
                  storeOp: "store",
                  clearValue: [0, 0, 0, 1],
                },
              ],
            });
            pass.setPipeline(mipPipeline);
            pass.setBindGroup(0, group);
            pass.draw(3);
            pass.end();
          }
          device.queue.submit([encoder.finish()]);
        }
        textureCache.set(key, texture);
        return texture;
      };
      const materials: GPUBindGroup[] = [];
      for (const description of manifest.materials) {
        const texture = await loadTexture(description.texture, false),
          normal = await loadTexture(description.normalTexture, true),
          emissive = description.emissiveTexture
            ? await loadTexture(description.emissiveTexture, false)
            : description.emissiveColor?.some((value) => value > 0)
              ? await loadTexture(undefined, false)
              : texture;
        const pbr =
          description.normalTexture !== undefined ||
          description.metallic !== undefined ||
          description.roughness !== undefined;
        const data = new Float32Array([
          ...description.color,
          description.alphaCutoff,
          Number(description.doubleSided),
          Number(!!description.normalTexture),
          Number(pbr),
          description.metallic ?? 0,
          description.roughness ?? 1,
          description.normalScale ?? 1,
          0,
          ...(description.emissiveColor ?? [0, 0, 0]),
          0,
        ]);
        const material = buffer(64, GPUBufferUsage.UNIFORM, data);
        materials.push(
          device.createBindGroup({
            layout: pipeline.getBindGroupLayout(1),
            entries: [
              { binding: 0, resource: texture.createView() },
              { binding: 1, resource: sampler },
              { binding: 2, resource: { buffer: material } },
              { binding: 3, resource: normal.createView() },
              { binding: 4, resource: emissive.createView() },
            ],
          }),
        );
      }
      const environmentGroups = new Map<string, GPUBindGroup>();
      if (options.ibl) {
        const environmentSampler = device.createSampler({
          magFilter: "linear",
          minFilter: "linear",
          mipmapFilter: "linear",
          addressModeU: "repeat",
          addressModeV: "clamp-to-edge",
        });
        for (const mode of ["day", "night"]) {
          const sky = options.ibl.scenarios.get(mode);
          environmentGroups.set(
            mode,
            device.createBindGroup({
              layout: pipeline.getBindGroupLayout(2),
              entries: [
                {
                  binding: 0,
                  resource: (sky?.texture ?? options.ibl.fallback).createView({
                    dimension: "2d-array",
                  }),
                },
                { binding: 1, resource: options.ibl.lut.createView() },
                { binding: 2, resource: environmentSampler },
              ],
            }),
          );
        }
      }
      let shadowResources: CharacterShadowResources | undefined;
      if (options.shadow) {
        const shadowModule = device.createShaderModule({
          code: CHARACTER_SHADOW_WGSL,
          label: "Animated character shadow",
        });
        const shadowPipeline = await device.createRenderPipelineAsync({
          layout: "auto",
          vertex: {
            module: shadowModule,
            entryPoint: "vs",
            buffers: [
              {
                arrayStride: 64,
                attributes: [
                  { shaderLocation: 0, offset: 0, format: "float32x3" },
                  { shaderLocation: 1, offset: 12, format: "float32x3" },
                  { shaderLocation: 2, offset: 24, format: "float32x2" },
                  { shaderLocation: 3, offset: 32, format: "float32x4" },
                  { shaderLocation: 4, offset: 48, format: "float32x4" },
                ],
              },
            ],
          },
          primitive: { topology: "triangle-list", cullMode: "none" },
          depthStencil: {
            format: "depth32float",
            depthWriteEnabled: true,
            depthCompare: "less-equal",
          },
        });
        const receiverBuffer = buffer(80, GPUBufferUsage.UNIFORM);
        const receiver = device.createBindGroup({
          layout: pipeline.getBindGroupLayout(3),
          entries: [
            { binding: 0, resource: { buffer: receiverBuffer } },
            { binding: 1, resource: options.shadow.texture.createView() },
            { binding: 2, resource: options.shadow.sampler },
          ],
        });
        const views = new Map<
          string,
          { buffer: GPUBuffer; group: GPUBindGroup }
        >();
        for (const key of ["nearSun", "farSun", "distantSun"]) {
          const matrix = buffer(64, GPUBufferUsage.UNIFORM);
          views.set(key, {
            buffer: matrix,
            group: device.createBindGroup({
              layout: shadowPipeline.getBindGroupLayout(0),
              entries: [
                { binding: 0, resource: { buffer: matrix } },
                { binding: 1, resource: { buffer: joints } },
                { binding: 2, resource: { buffer: actors } },
              ],
            }),
          });
        }
        shadowResources = {
          pipeline: shadowPipeline,
          receiver,
          receiverBuffer,
          views,
        };
      }
      return new CharacterRenderer(
        device,
        manifest,
        pipeline,
        vertices,
        indices,
        frame,
        joints,
        actors,
        capacity,
        group,
        materials,
        buffers,
        textures,
        environmentGroups,
        options.ibl,
        shadowResources,
      );
    } catch (error) {
      buffers.forEach((b) => b.destroy());
      textures.forEach((t) => t.destroy());
      throw error;
    }
  }
  render(
    pass: GPURenderPassEncoder,
    frame: CharacterFrame,
    pose: CharacterRenderPose,
  ) {
    if (this.disposed || !pose.visible || frame.enabled === false) return;
    this.renderInstances(pass, frame, [
      {
        position: pose.position,
        yaw: pose.yaw,
        palette: this.animator.sample(
          pose.animation,
          pose.time,
          frame.time,
          pose.blend,
        ),
      },
    ]);
  }
  renderInstances(
    pass: GPURenderPassEncoder,
    frame: CharacterFrame,
    instances: readonly CharacterInstance[],
  ) {
    if (this.disposed || frame.enabled === false || instances.length === 0)
      return;
    if (instances.length > this.capacity)
      throw new Error("The character renderer has too many actors.");
    const stride = this.manifest.joints.length * 32;
    // Validate the whole submission before touching live GPU data.
    for (const pose of instances) {
      if (
        pose.position.length !== 3 ||
        pose.position.some((v) => !Number.isFinite(v) || Math.abs(v) > 1e6) ||
        !Number.isFinite(pose.yaw) ||
        pose.palette.length !== stride ||
        pose.palette.some((v) => !Number.isFinite(v))
      )
        throw new Error("The character needs a complete finite pose.");
    }
    for (const [data, length] of [
      [frame.viewProjection, 16],
      [frame.diffuseRotation, 16],
      [frame.diffuseSH, 36],
      [frame.eye, 3],
      [frame.sunDirection, 3],
      [frame.sunIlluminance, 3],
    ] as const) {
      if (data.length !== length)
        throw new Error("The character needs complete world lighting.");
      for (let i = 0; i < data.length; i++)
        if (!Number.isFinite(data[i]))
          throw new Error("The character needs finite world lighting.");
    }
    const u = this.settings;
    u.set(frame.viewProjection, 0);
    u.set(frame.diffuseRotation, 16);
    u.set([...frame.eye, frame.time], 32);
    u.set([...frame.sunDirection, 0], 36);
    u.set([...frame.sunIlluminance, 0], 40);
    u.set([...(frame.ambient ?? [0, 0, 0]), 0], 44);
    u.set(
      [frame.fog?.density ?? 0, frame.fog?.heightRate ?? FOG_HEIGHT_RATE, 0, 0],
      48,
    );
    u.set([...(frame.fog?.color ?? [0, 0, 0]), 0], 52);
    u.set(frame.fog?.volumeMedium ?? [0, 0, 1, 0], 56);
    u.set([...(frame.fog?.volumeRange ?? [0, 0]), 0, 0], 60);
    u.set(frame.diffuseSH, 64);
    u.fill(0, 100);
    const environment = frame.specularEnvironment;
    if (environment) {
      if (
        environment.gain.length !== 3 ||
        [
          ...environment.gain,
          environment.primaryScale,
          environment.constantScale,
        ].some((v) => !Number.isFinite(v) || v < 0 || v > 1e6)
      )
        throw new Error("The character needs finite sky reflection settings.");
    }
    const sky = this.ibl?.scenarios.get(frame.mode);
    if (
      sky &&
      (!Number.isInteger(sky.mipMax) || sky.mipMax < 0 || sky.mipMax > 8)
    )
      throw new Error("The character sky needs supported reflection levels.");
    u.set(environment?.gain ?? [1, 1, 1], 100);
    u[104] = sky?.mipMax ?? 0;
    u[105] = environment?.primaryScale ?? 1;
    u[106] = environment?.constantScale ?? 0;
    u[107] = Number(!!sky);
    u[115] = frame.sourceLighting ? (frame.fog?.sourceScale ?? 1) : 0;
    if (this.shadowResources) {
      if (
        !frame.shadowMatrix ||
        frame.shadowMatrix.length !== 16 ||
        !Array.from(frame.shadowMatrix).every(Number.isFinite)
      )
        throw Error("Choose a complete character sunlight camera.");
      const shadow = new Float32Array(20);
      shadow.set(frame.shadowMatrix);
      this.device.queue.writeBuffer(
        this.shadowResources.receiverBuffer,
        0,
        shadow,
      );
    }
    for (let i = 0; i < instances.length; i++) {
      const pose = instances[i];
      this.actorData.set(
        modelMatrix(pose.position as [number, number, number], pose.yaw),
        i * 20,
      );
      this.actorOffsets[i * 20 + 16] = i * this.manifest.joints.length;
      this.jointData.set(pose.palette, i * stride);
    }
    this.device.queue.writeBuffer(this.frame, 0, u);
    this.device.queue.writeBuffer(
      this.joints,
      0,
      this.jointData.subarray(0, instances.length * stride),
    );
    this.device.queue.writeBuffer(
      this.actors,
      0,
      this.actorData.subarray(0, instances.length * 20),
    );
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group);
    const environmentGroup = this.environmentGroups.get(frame.mode);
    if (environmentGroup) pass.setBindGroup(2, environmentGroup);
    if (this.shadowResources)
      pass.setBindGroup(3, this.shadowResources.receiver);
    pass.setVertexBuffer(0, this.vertices);
    pass.setIndexBuffer(this.indices, "uint32");
    for (const draw of this.manifest.draws) {
      pass.setBindGroup(1, this.materials[draw.material]);
      if (draw.part === undefined) {
        pass.drawIndexed(draw.indexCount, instances.length, draw.firstIndex);
      } else {
        for (let start = 0; start < instances.length;) {
          if (instances[start].weapon !== draw.part) {
            start++;
            continue;
          }
          let end = start + 1;
          while (end < instances.length && instances[end].weapon === draw.part)
            end++;
          pass.drawIndexed(
            draw.indexCount,
            end - start,
            draw.firstIndex,
            0,
            start,
          );
          start = end;
        }
      }
    }
  }
  renderShadow(
    pass: GPURenderPassEncoder,
    matrix: ArrayLike<number>,
    instances: readonly CharacterInstance[],
    key: string,
  ) {
    const resources = this.shadowResources,
      view = resources?.views.get(key);
    if (!resources || !view || !instances.length || this.disposed) return;
    if (
      instances.length > this.capacity ||
      matrix.length !== 16 ||
      !Array.from(matrix).every(Number.isFinite)
    )
      throw Error("Choose a bounded character shadow view.");
    const stride = this.manifest.joints.length * 32;
    instances.forEach((pose, i) => {
      this.actorData.set(
        modelMatrix(pose.position as [number, number, number], pose.yaw),
        i * 20,
      );
      this.actorOffsets[i * 20 + 16] = i * this.manifest.joints.length;
      this.jointData.set(pose.palette, i * stride);
    });
    this.device.queue.writeBuffer(
      this.joints,
      0,
      this.jointData.subarray(0, instances.length * stride),
    );
    this.device.queue.writeBuffer(
      this.actors,
      0,
      this.actorData.subarray(0, instances.length * 20),
    );
    this.device.queue.writeBuffer(view.buffer, 0, new Float32Array(matrix));
    pass.setPipeline(resources.pipeline);
    pass.setBindGroup(0, view.group);
    pass.setVertexBuffer(0, this.vertices);
    pass.setIndexBuffer(this.indices, "uint32");
    for (const draw of this.manifest.draws) {
      if (!draw.part)
        pass.drawIndexed(draw.indexCount, instances.length, draw.firstIndex);
      else
        for (let start = 0; start < instances.length;) {
          if (instances[start].weapon !== draw.part) {
            start++;
            continue;
          }
          let end = start + 1;
          while (end < instances.length && instances[end].weapon === draw.part)
            end++;
          pass.drawIndexed(
            draw.indexCount,
            end - start,
            draw.firstIndex,
            0,
            start,
          );
          start = end;
        }
    }
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.ownedBuffers.forEach((b) => b.destroy());
    this.textures.forEach((t) => t.destroy());
  }
}
