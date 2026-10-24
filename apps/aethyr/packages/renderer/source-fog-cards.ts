import {
  readAsset,
  AssetIntegrityError,
} from "../assets/src/asset-download.ts";
import { FOG_SEGMENT_WGSL } from "./fog.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import { MAX_WORLD_ANIMATION_TIME } from "../runtime/world-clock.ts";
import type { V3 } from "./math.ts";

export const FOG_CARD_LIMITS = Object.freeze({
  cards: 32,
  textureSize: 1024,
  textureBytes: 1024 * 1024,
});
export type FogCardEdgeTreatment = "owned-soft-border-v1";
export const FOG_CARD_SOFT_BORDER_UV = 0.005;
export interface FogCardTexture {
  url: string;
  sha256: string;
  bytes: number;
  width: number;
  height: number;
  format: "r8unorm";
}
export interface SourceFogCard {
  id: string;
  model: number[];
  tint: V3;
  brightness: number;
  intensity: number;
  intensityExponent: number;
  detailExponent: number;
  detailTiling: number;
  thickestDistance: number;
  transitionDistance: number;
  depthFadeDistance: number;
  fresnelExponent: number;
  /** UV cycles per second, resolved by the preparer's declared random policy. */
  speed: [number, number];
}
export interface SourceFogCardsManifest {
  schema: "aethyrforge.source-fog-cards.v1";
  /** Optional owned emission taper; omission retains the source mask boundary. */
  edgeTreatment?: FogCardEdgeTreatment;
  source: {
    factsSha256: string;
    graphSha256: string;
    planeSha256: string;
    randomPolicy: "owned-seed-hash-v1";
  };
  textures: { mask: FogCardTexture; detail: FogCardTexture };
  scenarios: Partial<Record<"day" | "night", { cards: SourceFogCard[] }>>;
}
function bounded(value: number, low: number, high: number) {
  return Number.isFinite(value) && value >= low && value <= high;
}
function digest(value: unknown) {
  return typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
}
export function validateFogCards(manifest: SourceFogCardsManifest): void {
  if (
    !manifest ||
    manifest.schema !== "aethyrforge.source-fog-cards.v1" ||
    !manifest.source ||
    ![
      manifest.source.factsSha256,
      manifest.source.graphSha256,
      manifest.source.planeSha256,
    ].every(digest) ||
    manifest.source.randomPolicy !== "owned-seed-hash-v1" ||
    (manifest.edgeTreatment !== undefined &&
      manifest.edgeTreatment !== "owned-soft-border-v1") ||
    !manifest.scenarios ||
    !manifest.textures
  )
    throw Error("Prepare the supported source fog-card recipe.");
  for (const mode of Object.keys(manifest.scenarios))
    if (mode !== "day" && mode !== "night")
      throw Error("Use day or night fog-card settings.");
  for (const mode of ["day", "night"] as const) {
    const scenario = manifest.scenarios[mode];
    if (!scenario) continue;
    if (
      !Array.isArray(scenario.cards) ||
      scenario.cards.length > FOG_CARD_LIMITS.cards
    )
      throw Error("Use up to 32 source fog cards per scene.");
    const ids = new Set<string>();
    for (const c of scenario.cards) {
      if (
        !c ||
        typeof c.id !== "string" ||
        !c.id.length ||
        c.id.length > 256 ||
        ids.has(c.id) ||
        !Array.isArray(c.model) ||
        c.model.length !== 16 ||
        !c.model.every((n) => bounded(n, -1e6, 1e6)) ||
        c.model[3] !== 0 ||
        c.model[7] !== 0 ||
        c.model[11] !== 0 ||
        c.model[15] !== 1 ||
        !Array.isArray(c.tint) ||
        c.tint.length !== 3 ||
        !c.tint.every((n) => bounded(n, 0, 16)) ||
        !bounded(c.brightness, 0, 1000) ||
        !bounded(c.intensity, 0, 4) ||
        ![c.intensityExponent, c.detailExponent].every((n) =>
          bounded(n, 0.001, 16),
        ) ||
        !bounded(c.detailTiling, 0.001, 16) ||
        !bounded(c.thickestDistance, 0, 10000) ||
        ![c.transitionDistance, c.depthFadeDistance].every((n) =>
          bounded(n, 0.001, 10000),
        ) ||
        !bounded(c.fresnelExponent, 0.001, 16) ||
        !Array.isArray(c.speed) ||
        c.speed.length !== 2 ||
        !c.speed.every((n) => bounded(n, -1, 1))
      )
        throw Error("Use finite, bounded source fog-card settings.");
      const a = c.model.slice(0, 3),
        b = c.model.slice(4, 7),
        d = c.model.slice(8, 11),
        determinant =
          a[0] * (b[1] * d[2] - b[2] * d[1]) -
          a[1] * (b[0] * d[2] - b[2] * d[0]) +
          a[2] * (b[0] * d[1] - b[1] * d[0]);
      if (
        Math.abs(determinant) < 1e-8 ||
        [a, b, d].some((v) => Math.hypot(...v) > 1000)
      )
        throw Error("Use a finite, invertible fog-card placement.");
      ids.add(c.id);
    }
  }
  for (const t of [manifest.textures.mask, manifest.textures.detail])
    if (
      !t ||
      t.format !== "r8unorm" ||
      typeof t.url !== "string" ||
      !/^\/private-assets\/[A-Za-z0-9_/-]+\.bin$/.test(t.url) ||
      !digest(t.sha256) ||
      !Number.isInteger(t.width) ||
      !Number.isInteger(t.height) ||
      !bounded(t.width, 1, FOG_CARD_LIMITS.textureSize) ||
      !bounded(t.height, 1, FOG_CARD_LIMITS.textureSize) ||
      t.bytes !== t.width * t.height ||
      t.bytes > FOG_CARD_LIMITS.textureBytes
    )
      throw Error("Prepare bounded linear grayscale fog textures.");
}
export function fogCardPhase(
  speed: [number, number],
  time: number,
): [number, number] {
  if (
    !bounded(time, 0, MAX_WORLD_ANIMATION_TIME) ||
    speed.length !== 2 ||
    !speed.every((n) => bounded(n, -1, 1))
  )
    throw Error("Use a supported fog-card animation age.");
  return speed.map((n) => n * time - Math.floor(n * time)) as [number, number];
}
/** The source mask has nonzero boundary pixels; this optional repair leaves opacity alone. */
export function fogCardEdgeFactor(
  uv: [number, number],
  treatment?: FogCardEdgeTreatment,
): number {
  if (
    uv.length !== 2 ||
    !uv.every((n) => bounded(n, 0, 1)) ||
    (treatment !== undefined && treatment !== "owned-soft-border-v1")
  )
    throw Error("Use supported fog-card edge settings.");
  if (!treatment) return 1;
  const distance = Math.min(uv[0], uv[1], 1 - uv[0], 1 - uv[1]),
    x = Math.min(1, distance / FOG_CARD_SOFT_BORDER_UV);
  return x * x * (3 - 2 * x);
}
/** Independent node-graph evaluation; texture inputs and geometry supply the remaining factors. */
export function fogCardSample(
  card: SourceFogCard,
  radial: number,
  detail: number,
  viewDepth: number,
  opaqueViewDepth: number,
  noV: number,
) {
  if (
    ![radial, detail, noV].every((n) => bounded(n, 0, 1)) ||
    !bounded(viewDepth, 0, 1e6) ||
    !bounded(opaqueViewDepth, 0, 1e6)
  )
    throw Error("Use finite fog-card samples.");
  const saturate = (n: number) => Math.max(0, Math.min(1, n)),
    alpha =
      saturate((viewDepth - card.thickestDistance) / card.transitionDistance) *
      saturate((opaqueViewDepth - viewDepth) / card.depthFadeDistance) *
      (1 - Math.pow(Math.max(1 - noV, 1e-4), card.fresnelExponent)),
    density =
      Math.pow(
        radial * Math.pow(detail, card.detailExponent),
        card.intensityExponent,
      ) * card.intensity;
  return {
    alpha,
    emission: card.tint.map((n) => n * card.brightness * density) as V3,
  };
}
export interface FogCardsFrame {
  mode: "day" | "night";
  time: number;
  viewProjection: Float32Array;
  inverseViewProjection: Float32Array;
  eye: V3;
  forward: V3;
  width: number;
  height: number;
  /** Analytic density at reference, height rate, reference height, start distance. */
  fogMedium: [number, number, number, number];
  volumeMedium: readonly [number, number, number, number];
  volumeRange: readonly [number, number];
  color: GPUTextureView;
  resolve: GPUTextureView;
  depth: GPUTextureView;
}
export const SOURCE_FOG_CARDS_WGSL =
  FOG_SEGMENT_WGSL +
  `
struct Frame {view:mat4x4<f32>,inverseView:mat4x4<f32>,eye:vec4<f32>,forward:vec4<f32>,viewport:vec4<f32>,medium:vec4<f32>,volumeMedium:vec4<f32>,volumeRange:vec4<f32>};
struct Card {model:mat4x4<f32>,emission:vec4<f32>,values:vec4<f32>,fade:vec4<f32>,pan:vec4<f32>};
@group(0)@binding(0)var<uniform>frame:Frame;
@group(0)@binding(1)var<storage,read>cards:array<Card>;
@group(0)@binding(2)var maskImage:texture_2d<f32>;
@group(0)@binding(3)var detailImage:texture_2d<f32>;
@group(0)@binding(4)var imageSampler:sampler;
@group(0)@binding(5)var sceneDepth:texture_depth_multisampled_2d;
struct Out{@builtin(position)position:vec4<f32>,@location(0)uv:vec2<f32>,@location(1)world:vec3<f32>,@location(2)@interpolate(flat)normal:vec3<f32>,@location(3)@interpolate(flat)card:u32};
@vertex fn vs(@builtin(vertex_index)v:u32,@builtin(instance_index)i:u32)->Out{
 let uv=array<vec2<f32>,6>(vec2(1.0,1.0),vec2(1.0,0.0),vec2(0.0,1.0),vec2(0.0,1.0),vec2(1.0,0.0),vec2(0.0,0.0))[v];
 let c=cards[i];let p=c.model*vec4(uv.x-.5,0.0,uv.y-.5,1.0);var o:Out;o.position=frame.view*p;o.world=p.xyz;o.uv=uv;o.normal=normalize(cross(c.model[2].xyz,c.model[0].xyz));o.card=i;return o;
}
@fragment fn fs(v:Out)->@location(0)vec4<f32>{
 let c=cards[v.card];let pixel=clamp(vec2<i32>(v.position.xy),vec2(0),vec2<i32>(frame.viewport.xy)-vec2(1));var depth=1.0;
 for(var i=0;i<4;i++){depth=min(depth,textureLoad(sceneDepth,pixel,i));}
 let uv=v.position.xy/frame.viewport.xy;let q=frame.inverseView*vec4(uv*vec2(2.0,-2.0)+vec2(-1.0,1.0),depth,1.0);let world=q.xyz/q.w;
 let ray=v.world-frame.eye.xyz;let distance=length(ray);let cardDepth=max(0.0,dot(ray,frame.forward.xyz));var opaqueDepth=1000000.0;if depth<.999999{opaqueDepth=max(0.0,dot(world-frame.eye.xyz,frame.forward.xyz));}
 let noV=clamp(abs(dot(normalize(v.normal),normalize(ray))),0.0,1.0);
 let alpha=clamp((cardDepth-c.fade.x)/c.fade.y,0.0,1.0)*clamp((opaqueDepth-cardDepth)/c.fade.z,0.0,1.0)*(1.0-pow(max(1.0-noV,0.0001),c.fade.w));
 let mask=textureSampleLevel(maskImage,imageSampler,v.uv,0.0).r;let detail=textureSampleLevel(detailImage,imageSampler,v.uv*c.values.w+c.pan.xy,0.0).r;
 let density=pow(mask*pow(detail,c.values.z),c.values.y)*c.values.x;
 var edge=1.0;if c.pan.z>0.0{let distanceToEdge=min(min(v.uv.x,v.uv.y),min(1.0-v.uv.x,1.0-v.uv.y));edge=smoothstep(0.0,c.pan.z,distanceToEdge);}
 let transport=heightFogComposition(frame.eye.y,normalize(ray).y,distance,frame.medium.x,frame.medium.y,frame.medium.z,frame.medium.w,frame.volumeMedium,frame.volumeRange.xy).x;
 // Additive source emission receives extinction, without adding another fog-color fill.
 return vec4(c.emission.rgb*c.emission.w*density*edge*${SCENE_LINEAR_SCALE}*transport,alpha);
}`;

async function blob(input: FogCardTexture) {
  return readAsset(
    input.url,
    async (response) => {
      if (!response.ok || !response.body)
        throw Error("A source fog texture could not open.");
      const reader = response.body.getReader(),
        chunks: Uint8Array[] = [];
      let length = 0;
      try {
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          length += value.byteLength;
          if (length > input.bytes)
            throw Error("The source fog texture is too large.");
          chunks.push(value);
        }
      } catch (e) {
        await reader.cancel();
        throw e;
      }
      if (length !== input.bytes)
        throw new AssetIntegrityError("The source fog texture is incomplete.");
      const bytes = new Uint8Array(length);
      let offset = 0;
      for (const c of chunks) {
        bytes.set(c, offset);
        offset += c.length;
      }
      const hash = Array.from(
        new Uint8Array(await crypto.subtle.digest("SHA-256", bytes)),
        (n) => n.toString(16).padStart(2, "0"),
      ).join("");
      if (hash !== input.sha256)
        throw new AssetIntegrityError(
          "The source fog texture changed after preparation.",
        );
      return bytes;
    },
    {},
  );
}
export class SourceFogCards {
  bytes = 0;
  private manifest?: SourceFogCardsManifest;
  private textures: GPUTexture[] = [];
  private uniform?: GPUBuffer;
  private cards?: GPUBuffer;
  private pipeline?: GPURenderPipeline;
  private sampler?: GPUSampler;
  private views: GPUTextureView[] = [];
  private groups = new WeakMap<GPUTextureView, GPUBindGroup>();
  private disposed = false;
  private constructor(private device: GPUDevice) {}
  static async create(device: GPUDevice, manifest?: SourceFogCardsManifest) {
    const r = new SourceFogCards(device);
    if (!manifest) return r;
    validateFogCards(manifest);
    r.manifest = structuredClone(manifest);
    try {
      for (const input of [
        r.manifest.textures.mask,
        r.manifest.textures.detail,
      ]) {
        const data = await blob(input),
          texture = device.createTexture({
            label: "Source fog grayscale",
            size: [input.width, input.height],
            format: "r8unorm",
            usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
          });
        r.textures.push(texture);
        r.views.push(texture.createView());
        device.queue.writeTexture(
          { texture },
          data,
          { bytesPerRow: input.width },
          [input.width, input.height],
        );
        r.bytes += input.bytes;
      }
      r.uniform = device.createBuffer({
        size: 224,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      });
      r.cards = device.createBuffer({
        size: FOG_CARD_LIMITS.cards * 128,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      });
      r.bytes += 224 + FOG_CARD_LIMITS.cards * 128;
      r.sampler = device.createSampler({
        addressModeU: "repeat",
        addressModeV: "repeat",
        magFilter: "linear",
        minFilter: "linear",
      });
      const module = device.createShaderModule({ code: SOURCE_FOG_CARDS_WGSL });
      const errors = (await module.getCompilationInfo()).messages.filter(
        (m) => m.type === "error",
      );
      if (errors.length) throw Error(errors.map((e) => e.message).join("\n"));
      r.pipeline = await device.createRenderPipelineAsync({
        layout: "auto",
        vertex: { module, entryPoint: "vs" },
        fragment: {
          module,
          entryPoint: "fs",
          targets: [
            {
              format: "rgba16float",
              blend: {
                color: {
                  srcFactor: "src-alpha",
                  dstFactor: "one",
                  operation: "add",
                },
                alpha: {
                  srcFactor: "zero",
                  dstFactor: "one",
                  operation: "add",
                },
              },
            },
          ],
        },
        primitive: { cullMode: "none" },
        depthStencil: {
          format: "depth24plus",
          depthWriteEnabled: false,
          depthCompare: "less-equal",
        },
        multisample: { count: 4 },
      });
      return r;
    } catch (e) {
      r.dispose();
      throw e;
    }
  }
  has(mode: "day" | "night") {
    return (
      !this.disposed && Boolean(this.manifest?.scenarios[mode]?.cards.length)
    );
  }
  render(encoder: GPUCommandEncoder, f: FogCardsFrame) {
    if (!this.has(f.mode)) return false;
    const cards = this.manifest!.scenarios[f.mode]!.cards;
    if (
      f.viewProjection.length !== 16 ||
      f.inverseViewProjection.length !== 16 ||
      ![
        ...f.viewProjection,
        ...f.inverseViewProjection,
        ...f.eye,
        ...f.forward,
        ...f.fogMedium,
        ...f.volumeMedium,
        ...f.volumeRange,
      ].every(Number.isFinite) ||
      !Number.isInteger(f.width) ||
      !Number.isInteger(f.height) ||
      !bounded(f.width, 1, 8192) ||
      !bounded(f.height, 1, 8192) ||
      Math.abs(Math.hypot(...f.forward) - 1) > 1e-4
    )
      throw Error("Use a finite fog-card frame.");
    const u = new Float32Array(56);
    u.set(f.viewProjection);
    u.set(f.inverseViewProjection, 16);
    u.set(f.eye, 32);
    u.set(f.forward, 36);
    u.set([f.width, f.height], 40);
    u.set(f.fogMedium, 44);
    u.set(f.volumeMedium, 48);
    u.set(f.volumeRange, 52);
    this.device.queue.writeBuffer(this.uniform!, 0, u);
    const values = new Float32Array(cards.length * 32);
    cards.forEach((c, i) => {
      const start = i * 32;
      values.set(c.model, start);
      values.set([...c.tint, c.brightness], start + 16);
      values.set(
        [c.intensity, c.intensityExponent, c.detailExponent, c.detailTiling],
        start + 20,
      );
      values.set(
        [
          c.thickestDistance,
          c.transitionDistance,
          c.depthFadeDistance,
          c.fresnelExponent,
        ],
        start + 24,
      );
      values.set(fogCardPhase(c.speed, f.time), start + 28);
      values[start + 30] = this.manifest!.edgeTreatment
        ? FOG_CARD_SOFT_BORDER_UV
        : 0;
    });
    this.device.queue.writeBuffer(this.cards!, 0, values);
    let group = this.groups.get(f.depth);
    if (!group) {
      group = this.device.createBindGroup({
        layout: this.pipeline!.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: { buffer: this.uniform! } },
          { binding: 1, resource: { buffer: this.cards! } },
          { binding: 2, resource: this.views[0] },
          { binding: 3, resource: this.views[1] },
          { binding: 4, resource: this.sampler! },
          { binding: 5, resource: f.depth },
        ],
      });
      this.groups.set(f.depth, group);
    }
    const pass = encoder.beginRenderPass({
      label: "Source fog cards",
      colorAttachments: [
        {
          view: f.color,
          resolveTarget: f.resolve,
          loadOp: "load",
          storeOp: "store",
        },
      ],
      depthStencilAttachment: { view: f.depth, depthReadOnly: true },
    });
    pass.setPipeline(this.pipeline!);
    pass.setBindGroup(0, group);
    pass.draw(6, cards.length);
    pass.end();
    return true;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.uniform?.destroy();
    this.cards?.destroy();
    for (const t of this.textures) t.destroy();
    this.textures = [];
    this.groups = new WeakMap();
  }
}
