import {
  validateMmorpgActors,
  type MmorpgActorPose,
} from "../assets/src/mmorpg.ts";
import type { CreatureFrame } from "./creatures.ts";
import { inverse, type V3 } from "./math.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";

export const MMORPG_NAME_LIMITS = Object.freeze({
  labels: 64,
  width: 256,
  height: 128,
  mipLevels: 9,
});
export const MMORPG_NAME_HEIGHT = Object.freeze({
  paladin: 11 / 6,
  sorceror: 10 / 6,
  zombie: 8 / 6,
  warrok: 16 / 6,
});
export const MMORPG_NAME_SIZE = Object.freeze([10 / 6, 5 / 6] as const);
export const MMORPG_NAME_TEXTURE_BYTES = Array.from(
  { length: 9 },
  (_, mip) => Math.max(1, 256 >> mip) * Math.max(1, 128 >> mip) * 64 * 4,
).reduce((a, b) => a + b, 0);

/** Keep the original floating-name.js canvas styling, without a DOM render loop. */
export function paintMmorpgName(
  context: OffscreenCanvasRenderingContext2D,
  name: string,
) {
  context.clearRect(0, 0, 256, 128);
  context.fillStyle = "#FFF";
  context.font = "18pt Helvetica";
  context.shadowOffsetX = 3;
  context.shadowOffsetY = 3;
  context.shadowColor = "rgba(0,0,0,0.3)";
  context.shadowBlur = 4;
  context.textAlign = "center";
  context.fillText(name, 128, 64);
}
function rasterizeName(name: string) {
  const canvas = new OffscreenCanvas(256, 128),
    context = canvas.getContext("2d");
  if (!context) throw Error("Character names could not be drawn.");
  paintMmorpgName(context, name);
  return canvas;
}

/** Derive the current camera plane, including off-axis lenses, from its exact matrix. */
export function mmorpgNameAxes(matrix: ArrayLike<number>): {
  right: V3;
  up: V3;
} {
  if (matrix.length !== 16 || !Array.from(matrix).every(Number.isFinite))
    throw Error("Use a complete camera for character names.");
  const inv = inverse(matrix);
  const point = (x: number, y: number): V3 => {
    const w = inv[3] * x + inv[7] * y + inv[15];
    if (!Number.isFinite(w) || Math.abs(w) < 1e-12)
      throw Error("Use a finite name projection.");
    return [0, 1, 2].map(
      (r) => (inv[r] * x + inv[r + 4] * y + inv[r + 12]) / w,
    ) as V3;
  };
  const center = point(0, 0),
    direction = (x: number, y: number): V3 => {
      const p = point(x, y),
        d = p.map((n, i) => n - center[i]),
        length = Math.hypot(...d);
      if (!Number.isFinite(length) || length < 1e-12)
        throw Error("Use a finite name projection.");
      return d.map((n) => n / length) as V3;
    };
  return { right: direction(1, 0), up: direction(0, 1) };
}

export const MMORPG_NAME_WGSL = /* wgsl */ `
struct Frame{view:mat4x4<f32>,right:vec4<f32>,up:vec4<f32>};
struct Name{center:vec4<f32>};
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var names:texture_2d_array<f32>;
@group(0)@binding(2)var filtering:sampler;
@group(0)@binding(3)var<storage,read>labels:array<Name>;
struct Out{@builtin(position)clip:vec4<f32>,@location(0)uv:vec2<f32>,@location(1)@interpolate(flat)layer:u32};
@vertex fn vs(@builtin(vertex_index)i:u32,@builtin(instance_index)actor:u32)->Out{
 let corners=array<vec2<f32>,6>(vec2(0.0,0.0),vec2(1.0,0.0),vec2(0.0,1.0),vec2(0.0,1.0),vec2(1.0,0.0),vec2(1.0,1.0));
 let uv=corners[i];let center=labels[actor].center;let anchor=frame.view*vec4(center.xyz,1.0);
 let world=center.xyz+frame.right.xyz*((uv.x-.5)*frame.right.w)+frame.up.xyz*((.5-uv.y)*frame.up.w);
 var o:Out;o.clip=frame.view*vec4(world,1.0);o.uv=uv;o.layer=u32(center.w);
 if anchor.w<=.0001||anchor.z<0.0||anchor.z>anchor.w{o.clip=vec4(2.0,2.0,2.0,1.0);}return o;
}
@fragment fn fs(v:Out)->@location(0)vec4<f32>{let c=textureSample(names,filtering,v.uv,i32(v.layer));if c.a<.001{discard;}return vec4(c.rgb*c.a*${SCENE_LINEAR_SCALE},c.a);}
`;
export const MMORPG_NAME_MIP_WGSL = /* wgsl */ `
@group(0)@binding(0)var source:texture_2d<f32>;
@group(0)@binding(1)var filtering:sampler;
struct Out{@builtin(position)clip:vec4<f32>,@location(0)uv:vec2<f32>};
@vertex fn vs(@builtin(vertex_index)i:u32)->Out{let p=vec2(f32((i<<1u)&2u),f32(i&2u));var o:Out;o.clip=vec4(p*2.0-1.0,0.0,1.0);o.uv=vec2(p.x,1.0-p.y);return o;}
@fragment fn fs(v:Out)->@location(0)vec4<f32>{return textureSampleLevel(source,filtering,v.uv,0.0);}
`;
interface NameSlot {
  layer: number;
  used: number;
}
export interface MmorpgNameOptions {
  depthFormat?: GPUTextureFormat;
  sampleCount?: number;
  /** Deterministic CPU tests replace only text rasterization, never actor or camera state. */
  rasterize?: (name: string) => OffscreenCanvas;
}

/** One shared atlas and instanced draw, sampled from the same camera as the world. */
export class MmorpgNames {
  readonly bytes = MMORPG_NAME_TEXTURE_BYTES + 96 + 64 * 16;
  private readonly cache = new Map<string, NameSlot>();
  private readonly mipGroups = new Map<
    number,
    { group: GPUBindGroup; view: GPUTextureView }[]
  >();
  private readonly frameData = new Float32Array(24);
  private readonly actorData = new Float32Array(64 * 4);
  private clock = 0;
  private count = 0;
  private submitted = 0;
  private uploads = 0;
  private disposed = false;
  private constructor(
    private device: GPUDevice,
    private pipeline: GPURenderPipeline,
    private mipPipeline: GPURenderPipeline,
    private atlas: GPUTexture,
    private sampler: GPUSampler,
    private frame: GPUBuffer,
    private actors: GPUBuffer,
    private group: GPUBindGroup,
    private rasterize: (name: string) => OffscreenCanvas,
  ) {}
  static async create(device: GPUDevice, options: MmorpgNameOptions = {}) {
    const shader = device.createShaderModule({
      label: "Source floating names",
      code: MMORPG_NAME_WGSL,
    });
    const pipeline = await device.createRenderPipelineAsync({
      label: "Depth-tested source floating names",
      layout: "auto",
      vertex: { module: shader, entryPoint: "vs" },
      fragment: {
        module: shader,
        entryPoint: "fs",
        targets: [
          {
            format: "rgba16float",
            blend: {
              color: { srcFactor: "one", dstFactor: "one-minus-src-alpha" },
              alpha: { srcFactor: "zero", dstFactor: "one" },
            },
          },
        ],
      },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: {
        format: options.depthFormat ?? "depth24plus",
        depthWriteEnabled: false,
        depthCompare: "less-equal",
      },
      multisample: { count: options.sampleCount ?? 4 },
    });
    const mipShader = device.createShaderModule({
      label: "Floating name mips",
      code: MMORPG_NAME_MIP_WGSL,
    });
    const mipPipeline = await device.createRenderPipelineAsync({
      layout: "auto",
      vertex: { module: mipShader, entryPoint: "vs" },
      fragment: {
        module: mipShader,
        entryPoint: "fs",
        targets: [{ format: "rgba8unorm-srgb" }],
      },
      primitive: { topology: "triangle-list" },
    });
    const atlas = device.createTexture({
      label: "Bounded source name atlas",
      size: [256, 128, 64],
      mipLevelCount: 9,
      format: "rgba8unorm-srgb",
      usage:
        GPUTextureUsage.TEXTURE_BINDING |
        GPUTextureUsage.COPY_DST |
        GPUTextureUsage.RENDER_ATTACHMENT,
    });
    let frame: GPUBuffer | undefined, actors: GPUBuffer | undefined;
    try {
      frame = device.createBuffer({
        label: "Current name camera",
        size: 96,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });
      actors = device.createBuffer({
        label: "Bounded name positions",
        size: 64 * 16,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      });
      const sampler = device.createSampler({
        minFilter: "linear",
        magFilter: "linear",
        mipmapFilter: "linear",
        addressModeU: "clamp-to-edge",
        addressModeV: "clamp-to-edge",
      });
      const group = device.createBindGroup({
        layout: pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: frame } },
          { binding: 1, resource: atlas.createView({ dimension: "2d-array" }) },
          { binding: 2, resource: sampler },
          { binding: 3, resource: { buffer: actors } },
        ],
      });
      return new MmorpgNames(
        device,
        pipeline,
        mipPipeline,
        atlas,
        sampler,
        frame,
        actors,
        group,
        options.rasterize ?? rasterizeName,
      );
    } catch (error) {
      atlas.destroy();
      frame?.destroy();
      actors?.destroy();
      throw error;
    }
  }
  get stats() {
    return {
      nameLabels: this.submitted,
      nameTriangles: this.submitted * 2,
      nameDrawCalls: Number(this.submitted > 0),
      nameTextureBytes: this.disposed ? 0 : MMORPG_NAME_TEXTURE_BYTES,
      nameTextureLayers: this.cache.size,
      nameUploads: this.uploads,
    };
  }
  private upload(name: string, layer: number, encoder: GPUCommandEncoder) {
    const canvas = this.rasterize(name);
    if (canvas.width !== 256 || canvas.height !== 128)
      throw Error("Use the source character name dimensions.");
    this.device.queue.copyExternalImageToTexture(
      { source: canvas },
      { texture: this.atlas, origin: [0, 0, layer], premultipliedAlpha: false },
      [256, 128],
    );
    let mips = this.mipGroups.get(layer);
    if (!mips) {
      mips = Array.from({ length: 8 }, (_, i) => {
        const source = this.atlas.createView({
          dimension: "2d",
          baseArrayLayer: layer,
          arrayLayerCount: 1,
          baseMipLevel: i,
          mipLevelCount: 1,
        });
        const view = this.atlas.createView({
          dimension: "2d",
          baseArrayLayer: layer,
          arrayLayerCount: 1,
          baseMipLevel: i + 1,
          mipLevelCount: 1,
        });
        return {
          view,
          group: this.device.createBindGroup({
            layout: this.mipPipeline.getBindGroupLayout(0),
            entries: [
              { binding: 0, resource: source },
              { binding: 1, resource: this.sampler },
            ],
          }),
        };
      });
      this.mipGroups.set(layer, mips);
    }
    for (const mip of mips) {
      const pass = encoder.beginRenderPass({
        label: "Changed floating-name mip",
        colorAttachments: [
          {
            view: mip.view,
            loadOp: "clear",
            storeOp: "store",
            clearValue: [0, 0, 0, 0],
          },
        ],
      });
      pass.setPipeline(this.mipPipeline);
      pass.setBindGroup(0, mip.group);
      pass.draw(3);
      pass.end();
    }
    this.uploads++;
  }
  prepare(
    encoder: GPUCommandEncoder,
    frame: CreatureFrame,
    actors: readonly MmorpgActorPose[],
    localId?: number,
  ) {
    this.count = this.submitted = 0;
    if (this.disposed || frame.enabled === false) return;
    validateMmorpgActors(actors);
    const axes = mmorpgNameAxes(frame.viewProjection);
    const alive = actors.filter(
      (actor) => actor.alive && actor.id !== localId && actor.name.length,
    );
    const needed = new Set(alive.map((a) => a.name));
    this.clock++;
    for (const name of needed) {
      let slot = this.cache.get(name);
      if (!slot) {
        let layer = this.cache.size;
        if (layer === 64) {
          const retired = [...this.cache]
            .filter(([key]) => !needed.has(key))
            .sort((a, b) => a[1].used - b[1].used)[0];
          if (!retired) throw Error("Use up to 64 distinct character names.");
          layer = retired[1].layer;
          this.cache.delete(retired[0]);
        } else {
          const used = new Set([...this.cache.values()].map((s) => s.layer));
          layer = Array.from({ length: 64 }, (_, i) => i).find(
            (i) => !used.has(i),
          )!;
        }
        this.upload(name, layer, encoder);
        slot = { layer, used: this.clock };
        this.cache.set(name, slot);
      }
      slot.used = this.clock;
    }
    const projected = alive
      .map((actor) => {
        const center = [...actor.position] as V3;
        center[1] += MMORPG_NAME_HEIGHT[actor.class];
        const clip = (row: number) =>
          frame.viewProjection[row] * center[0] +
          frame.viewProjection[row + 4] * center[1] +
          frame.viewProjection[row + 8] * center[2] +
          frame.viewProjection[row + 12];
        return { actor, center, depth: clip(3), z: clip(2) };
      })
      .filter((a) => a.depth > 0.0001 && a.z >= 0 && a.z <= a.depth)
      .sort((a, b) => b.depth - a.depth || a.actor.id - b.actor.id);
    for (const entry of projected)
      this.actorData.set(
        [...entry.center, this.cache.get(entry.actor.name)!.layer],
        this.count++ * 4,
      );
    if (!this.count) return;
    this.frameData.set(Array.from(frame.viewProjection), 0);
    this.frameData.set([...axes.right, MMORPG_NAME_SIZE[0]], 16);
    this.frameData.set([...axes.up, MMORPG_NAME_SIZE[1]], 20);
    this.device.queue.writeBuffer(this.frame, 0, this.frameData);
    this.device.queue.writeBuffer(
      this.actors,
      0,
      this.actorData.subarray(0, this.count * 4),
    );
  }
  render(pass: GPURenderPassEncoder) {
    this.submitted = 0;
    if (this.disposed || !this.count) return;
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group);
    pass.draw(6, this.count);
    this.submitted = this.count;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.count = this.submitted = 0;
    this.atlas.destroy();
    this.frame.destroy();
    this.actors.destroy();
    this.cache.clear();
    this.mipGroups.clear();
  }
}
