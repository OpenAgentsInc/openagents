import { add, sub, scale, cross, normalize, type V3 } from "./math.ts";
import { SCENE_LINEAR_SCALE } from "./radiometry.ts";
import type { SourceLight } from "../assets/src/scene.ts";
import type { GameplayAimMarker } from "./spell-targeting.ts";
import {
  sampleImpactRecipe,
  validateImpactEvent,
  validateImpactRecipe,
  type ImpactRecipePayload,
  type ImpactWindSampler,
} from "./impact-recipe.ts";

export const GAMEPLAY_VISUAL_LIMITS = Object.freeze({
  effects: 64,
  debris: 256,
  rubble: 1024,
  nodes: 65536,
  vertices: 24576,
});
export interface GameplayObject {
  id: number;
  nodeId?: number;
  sourceId: number;
  min: V3;
  max: V3;
}
export interface GameplayDebris {
  id: number;
  nodeId: number;
  sourceId: number;
  chunk: number;
  subdivisions?: 2 | 4;
  sourceMin: V3;
  sourceMax: V3;
  position: V3;
  rotation: V3;
  halfExtents: V3;
  velocity: V3;
  age: number;
  life: number;
}
export interface GameplayEffect {
  id: number;
  kind: "beam" | "lightning" | "meteor" | "impact";
  origin: V3;
  target: V3;
  age: number;
  life: number;
  radius: number;
  impactRecipe?: ImpactRecipePayload;
}
export interface GameplayVisualState {
  destroyed: readonly number[];
  debris: readonly GameplayDebris[];
  effects: readonly GameplayEffect[];
  rubble?: readonly GameplayDebris[];
  castle?: { rubble?: readonly GameplayDebris[]; rubbleCount?: number };
}
const finite = (v: unknown, lo: number, hi: number): v is number =>
  typeof v === "number" && Number.isFinite(v) && v >= lo && v <= hi;
const vector = (v: unknown): v is V3 =>
  Array.isArray(v) &&
  v.length === 3 &&
  v.every((n) => finite(n, -20000, 20000));
const id = (v: unknown) =>
  Number.isSafeInteger(v) && (v as number) >= 0 && (v as number) <= 0xffffffff;
export function validateGameplayVisualState(state: GameplayVisualState): void {
  const rubble = state?.rubble ?? state?.castle?.rubble ?? [];
  if (
    !state ||
    !Array.isArray(state.destroyed) ||
    state.destroyed.length > GAMEPLAY_VISUAL_LIMITS.nodes ||
    !state.destroyed.every(id) ||
    new Set(state.destroyed).size !== state.destroyed.length ||
    !Array.isArray(state.debris) ||
    state.debris.length > GAMEPLAY_VISUAL_LIMITS.debris ||
    new Set(state.debris.map((body) => body.id)).size !== state.debris.length ||
    !Array.isArray(state.effects) ||
    state.effects.length > GAMEPLAY_VISUAL_LIMITS.effects ||
    new Set(state.effects.map((effect) => effect.id)).size !==
      state.effects.length ||
    !Array.isArray(rubble) ||
    rubble.length > GAMEPLAY_VISUAL_LIMITS.rubble ||
    new Set(rubble.map((body) => body.id)).size !== rubble.length
  )
    throw Error("Destruction exceeds the visual scene budget.");
  for (const d of [...state.debris, ...rubble] as readonly GameplayDebris[])
    if (
      !id(d.id) ||
      !id(d.nodeId) ||
      !id(d.sourceId) ||
      !Number.isInteger(d.chunk) ||
      d.chunk < 0 ||
      d.chunk > 63 ||
      (d.subdivisions !== undefined &&
        d.subdivisions !== 2 &&
        d.subdivisions !== 4) ||
      d.chunk >= (d.subdivisions === 4 ? 64 : 8) ||
      ![
        d.sourceMin,
        d.sourceMax,
        d.position,
        d.rotation,
        d.halfExtents,
        d.velocity,
      ].every(vector) ||
      d.sourceMin.some((v, i) => v > d.sourceMax[i]) ||
      d.halfExtents.some((v) => v < 0) ||
      !finite(d.age, 0, 60) ||
      !finite(d.life, 0.001, 60)
    )
      throw Error("Debris needs complete bounded transforms and lifetime.");
  for (const e of state.effects) {
    if (
      !id(e.id) ||
      !["beam", "lightning", "meteor", "impact"].includes(e.kind) ||
      !vector(e.origin) ||
      !vector(e.target) ||
      !finite(e.age, 0, 60) ||
      !finite(e.life, 0.001, 60) ||
      !finite(e.radius, 0.001, 100)
    )
      throw Error(
        "Spell effects need complete bounded positions and lifetime.",
      );
    if (e.impactRecipe !== undefined) {
      if (e.kind !== "impact")
        throw Error("Impact settings need an impact effect.");
      validateImpactEvent(e.impactRecipe.event);
      validateImpactRecipe(e.impactRecipe.recipe);
    }
  }
}
export function maskDestroyedPlacements(
  visibility: Uint8Array,
  destroyed: ReadonlySet<number>,
): Uint8Array {
  for (const index of destroyed)
    if (index >= 0 && index < visibility.length) visibility[index] = 0;
  return visibility;
}
function random(seed: number, salt: number) {
  let value = (seed ^ Math.imul(salt + 1, 0x9e3779b9)) >>> 0;
  value = Math.imul(value ^ (value >>> 16), 0x7feb352d);
  value = Math.imul(value ^ (value >>> 15), 0x846ca68b);
  return ((value ^ (value >>> 16)) >>> 0) / 4294967296;
}
/** A fixed array and analytic ages make particle work independent of display rate. */
export class GameplayEffectSampler {
  readonly data = new Float32Array(GAMEPLAY_VISUAL_LIMITS.vertices * 10);
  count = 0;
  sample(
    effects: readonly GameplayEffect[],
    eye: V3,
    forward: V3,
    marker?: GameplayAimMarker,
    wind?: ImpactWindSampler,
  ): {
    data: Float32Array;
    count: number;
    effectCount: number;
    smokeStart: number;
    smokeCount: number;
  } {
    this.count = 0;
    let smokeCount = 0;
    const right = normalize(cross(forward, [0, 1, 0]));
    const up = normalize(cross(right, forward));
    const quad = (
      center: V3,
      x: V3,
      y: V3,
      color: V3,
      alpha: number,
      kind: number,
      smoke = false,
    ) => {
      if (
        this.count + smokeCount + 6 > GAMEPLAY_VISUAL_LIMITS.vertices ||
        alpha <= 0
      )
        return;
      const first = smoke
        ? GAMEPLAY_VISUAL_LIMITS.vertices - smokeCount - 6
        : this.count;
      if (smoke) smokeCount += 6;
      for (let vertex = 0; vertex < 6; vertex++) {
        const u = vertex === 0 || vertex === 2 || vertex === 3 ? -1 : 1,
          v = vertex === 0 || vertex === 1 || vertex === 4 ? -1 : 1,
          offset = (first + vertex) * 10;
        // Preserve the original center + (x * u + y * v) evaluation order.
        this.data[offset] = center[0] + (x[0] * u + y[0] * v);
        this.data[offset + 1] = center[1] + (x[1] * u + y[1] * v);
        this.data[offset + 2] = center[2] + (x[2] * u + y[2] * v);
        this.data[offset + 3] = u;
        this.data[offset + 4] = v;
        this.data[offset + 5] = color[0];
        this.data[offset + 6] = color[1];
        this.data[offset + 7] = color[2];
        this.data[offset + 8] = alpha;
        this.data[offset + 9] = kind;
      }
      if (!smoke) this.count += 6;
    };
    const disc = (
      center: V3,
      radius: number,
      color: V3,
      alpha: number,
      kind = 0,
    ) =>
      quad(center, scale(right, radius), scale(up, radius), color, alpha, kind);
    const ribbon = (a: V3, b: V3, width: number, color: V3, alpha: number) => {
      const midpoint = scale(add(a, b), 0.5),
        axis = sub(b, a);
      const side = normalize(cross(axis, sub(eye, midpoint)));
      quad(midpoint, scale(side, width), scale(axis, 0.5), color, alpha, 2);
    };
    for (const e of effects.slice(0, GAMEPLAY_VISUAL_LIMITS.effects)) {
      if (e.age < 0 || e.age >= e.life) continue;
      if (e.kind === "impact" && e.impactRecipe) {
        sampleImpactRecipe(
          e.impactRecipe,
          e.age,
          {
            disc,
            ring: (center, x, y, color, alpha) =>
              quad(center, x, y, color, alpha, 1),
            streak: ribbon,
            smoke: (center, radius, color, alpha, debris = false) =>
              quad(
                center,
                scale(right, radius),
                scale(up, radius),
                color,
                alpha,
                debris ? 4 : 3,
                true,
              ),
          },
          wind,
        );
        continue;
      }
      const t = Math.max(0, Math.min(1, e.age / e.life));
      if (e.kind === "beam" || e.kind === "lightning") {
        const size = Math.max(1, Math.min(8, e.radius / 4));
        const direction = sub(e.target, e.origin),
          pulse =
            Math.max(0, 1 - t) * (0.78 + 0.22 * Math.sin(e.age * 95) ** 2);
        const jitter = Math.min(
          (e.kind === "beam" ? 0.5 : 1.7) * size,
          Math.hypot(...direction) * 0.025,
        );
        let previous = e.origin;
        for (let i = 1; i <= 18; i++) {
          const f = i / 18;
          const p = add(e.origin, scale(direction, f));
          if (i < 18) {
            p[0] += (random(e.id, i * 2) - 0.5) * jitter;
            p[2] += (random(e.id, i * 2 + 1) - 0.5) * jitter;
          }
          ribbon(
            previous,
            p,
            (e.kind === "beam" ? 0.17 : 0.11) * size,
            [0.4, 0.64, 1],
            pulse * 3,
          );
          ribbon(
            previous,
            p,
            (e.kind === "beam" ? 0.045 : 0.025) * size,
            [0.9, 0.97, 1],
            pulse * 8,
          );
          if (e.kind === "lightning" && i % 4 === 0 && i < 16) {
            const branch = add(p, [
              (random(e.id, i + 60) - 0.5) * 9 * size,
              (-2 - random(e.id, i + 70) * 4) * size,
              (random(e.id, i + 80) - 0.5) * 9 * size,
            ]);
            ribbon(p, branch, 0.045 * size, [0.3, 0.55, 1], pulse * 2);
          }
          previous = p;
        }
        disc(e.target, 0.8 + e.radius * 0.4, [0.4, 0.65, 1], pulse * 5);
      } else if (e.kind === "meteor") {
        const size = Math.max(1, Math.min(8, e.radius / 4));
        const p = add(e.origin, scale(sub(e.target, e.origin), t));
        const axis = normalize(sub(e.origin, e.target));
        for (let i = 7; i >= 0; i--) {
          const radius = (0.25 + (1 - i / 8) * 0.8) * size;
          const center = add(p, scale(axis, i * 1.15 * size));
          disc(center, radius, [1, 0.13 + i * 0.015, 0.012], (1 - i / 8) * 4);
        }
        disc(p, 0.7 * size, [1, 0.65, 0.12], 12);
        ribbon(
          p,
          add(p, scale(axis, 7 * size)),
          0.42 * size,
          [1, 0.2, 0.01],
          2,
        );
      } else {
        const radius = Math.max(0.1, e.radius * Math.sqrt(t));
        const normal = normalize(sub(e.target, e.origin));
        const tangent = normalize(
          cross(normal, Math.abs(normal[1]) > 0.95 ? [1, 0, 0] : [0, 1, 0]),
        );
        const bitangent = cross(normal, tangent);
        quad(
          add(e.origin, scale(normal, 0.08)),
          scale(tangent, radius),
          scale(bitangent, radius),
          [1, 0.35, 0.045],
          (1 - t) * 2,
          1,
        );
        disc(
          add(e.origin, scale(normal, 0.35)),
          Math.max(0.1, e.radius * 0.55 * (1 - t)),
          [1, 0.5, 0.1],
          (1 - t) ** 2 * 8,
        );
        for (let i = 0; i < 18; i++) {
          const a = random(e.id, i) * Math.PI * 2,
            speed = 2 + random(e.id, i + 20) * 8;
          const p = add(e.origin, [
            Math.sin(a) * speed * e.age,
            (3 + random(e.id, i + 40) * 5) * e.age - 4.9 * e.age ** 2,
            Math.cos(a) * speed * e.age,
          ]);
          if (p[1] >= e.origin[1])
            disc(p, 0.035 + 0.06 * (1 - t), [1, 0.36, 0.03], (1 - t) * 4);
        }
      }
    }
    const effectCount = this.count + smokeCount;
    if (
      marker &&
      vector(marker.point) &&
      vector(marker.normal) &&
      Math.hypot(...marker.normal) > 0.5 &&
      finite(marker.radius, 0.1, 80) &&
      this.count + smokeCount + 64 * 6 <= GAMEPLAY_VISUAL_LIMITS.vertices
    ) {
      const normal = normalize(marker.normal);
      const tangent = normalize(
        cross(normal, Math.abs(normal[1]) > 0.95 ? [1, 0, 0] : [0, 1, 0]),
      );
      const bitangent = cross(normal, tangent);
      const center = add(marker.point, scale(normal, 0.035));
      const inner =
        marker.radius -
        (marker.overlay === true
          ? Math.min(0.16, marker.radius * 0.4)
          : Math.min(0.09, marker.radius * 0.07));
      const point = (angle: number, radius: number) =>
        add(
          center,
          add(
            scale(tangent, Math.cos(angle) * radius),
            scale(bitangent, Math.sin(angle) * radius),
          ),
        );
      for (let segment = 0; segment < 64; segment++) {
        const a = (segment / 64) * Math.PI * 2;
        const b = ((segment + 1) / 64) * Math.PI * 2;
        const vertices = [
          point(a, inner),
          point(a, marker.radius),
          point(b, inner),
          point(b, inner),
          point(a, marker.radius),
          point(b, marker.radius),
        ];
        for (const vertex of vertices)
          this.data.set(
            [...vertex, 0, 0, 0.18, 1, 0.76, 3, 2],
            this.count++ * 10,
          );
      }
    }
    const smokeStart = this.count;
    if (smokeCount)
      this.data.copyWithin(
        smokeStart * 10,
        (GAMEPLAY_VISUAL_LIMITS.vertices - smokeCount) * 10,
      );
    this.count += smokeCount;
    return {
      data: this.data.subarray(0, this.count * 10),
      count: this.count,
      effectCount,
      smokeStart,
      smokeCount,
    };
  }
}

/** At most four transient, unshadowed lights reuse the existing clustered-light path. */
export function gameplayLights(
  effects: readonly GameplayEffect[],
): SourceLight[] {
  return effects
    .filter(
      (e) =>
        e.age >= 0 &&
        e.age < Math.min(e.life, e.impactRecipe?.recipe.flash.life ?? 0.45) &&
        e.kind !== "meteor" &&
        (!e.impactRecipe || e.impactRecipe.recipe.intensity > 0),
    )
    .slice(-4)
    .map((e) => ({
      id: "spell:" + e.id,
      label: "Spell impact",
      kind: "point",
      position:
        e.kind === "impact"
          ? add(e.origin, scale(normalize(sub(e.target, e.origin)), 0.7))
          : add(e.target, [0, 0.7, 0]),
      direction: [0, -1, 0],
      color: e.kind === "impact" ? [1, 0.24, 0.025] : [0.42, 0.66, 1],
      intensity: e.impactRecipe
        ? 1100 *
          e.impactRecipe.recipe.intensity *
          e.impactRecipe.event.intensity *
          (1 - e.age / e.impactRecipe.recipe.flash.life) ** 2
        : 1100 * (1 - e.age / Math.min(e.life, 0.45)),
      intensityUnits: "candelas",
      range: Math.max(10, e.radius * 4),
      castShadows: false,
      enabled: true,
      temperatureKelvin: 6500,
      useTemperature: false,
      raw: {},
    }));
}
/** Transient light contents change even while the camera and world stay still. */
export function gameplayLightRevision(
  mode: string,
  transientLights: readonly SourceLight[],
): string {
  return transientLights.length
    ? JSON.stringify([mode, transientLights])
    : mode;
}
export const GAMEPLAY_EFFECT_WGSL = /* wgsl */ `
struct Frame { view:mat4x4<f32> }
@group(0) @binding(0) var<uniform> frame:Frame;
struct In { @location(0) position:vec3<f32>,@location(1) uv:vec2<f32>,@location(2) color:vec4<f32>,@location(3) kind:f32 }
struct Out { @builtin(position) clip:vec4<f32>,@location(0) uv:vec2<f32>,@location(1) color:vec4<f32>,@location(2) @interpolate(flat) kind:f32 }
@vertex fn vs(v:In)->Out{var o:Out;o.clip=frame.view*vec4(v.position,1.0);o.uv=v.uv;o.color=v.color;o.kind=v.kind;return o;}
@fragment fn fs(v:Out)->@location(0)vec4<f32>{let r=length(v.uv);var shape=pow(max(0.0,1.0-r),2.0);if v.kind>1.5{shape=pow(max(0.0,1.0-abs(v.uv.x)),1.5);}else if v.kind>.5{shape=(1.0-smoothstep(.07,.18,abs(r-.83)))*step(r,1.0);}let a=shape*v.color.a;return vec4(v.color.rgb*a*${SCENE_LINEAR_SCALE},0.0);}
@fragment fn fsSmoke(v:Out)->@location(0)vec4<f32>{let r=length(v.uv);var shape=pow(max(0.0,1.0-r*r),2.0);if v.kind>3.5{shape=1.0-smoothstep(0.7,1.0,max(abs(v.uv.x),abs(v.uv.y)));}let a=shape*v.color.a;return vec4(v.color.rgb*a*${SCENE_LINEAR_SCALE},a);}
`;
export class GameplayEffects {
  readonly bytes = GAMEPLAY_VISUAL_LIMITS.vertices * 40 + 64;
  private sampler = new GameplayEffectSampler();
  triangles = 0;
  aimTriangles = 0;
  private constructor(
    private device: GPUDevice,
    private pipeline: GPURenderPipeline,
    private smokePipeline: GPURenderPipeline,
    private aimOverlayPipeline: GPURenderPipeline,
    private vertices: GPUBuffer,
    private uniform: GPUBuffer,
    private group: GPUBindGroup,
  ) {}
  static async create(device: GPUDevice) {
    const module = device.createShaderModule({ code: GAMEPLAY_EFFECT_WGSL });
    const bindGroupLayout = device.createBindGroupLayout({
      entries: [
        {
          binding: 0,
          visibility: GPUShaderStage.VERTEX,
          buffer: { type: "uniform", minBindingSize: 64 },
        },
      ],
    });
    const layout = device.createPipelineLayout({
      bindGroupLayouts: [bindGroupLayout],
    });
    const additiveDescriptor: GPURenderPipelineDescriptor = {
      layout,
      vertex: {
        module,
        entryPoint: "vs",
        buffers: [
          {
            arrayStride: 40,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x3" },
              { shaderLocation: 1, offset: 12, format: "float32x2" },
              { shaderLocation: 2, offset: 20, format: "float32x4" },
              { shaderLocation: 3, offset: 36, format: "float32" },
            ],
          },
        ],
      },
      fragment: {
        module,
        entryPoint: "fs",
        targets: [
          {
            format: "rgba16float",
            blend: {
              color: { srcFactor: "one", dstFactor: "one" },
              alpha: { srcFactor: "zero", dstFactor: "one" },
            },
          },
        ],
      },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: false,
        depthCompare: "less-equal",
      },
      multisample: { count: 4 },
    };
    const pipeline = await device.createRenderPipelineAsync(additiveDescriptor);
    const smokePipeline = await device.createRenderPipelineAsync({
      layout,
      vertex: {
        module,
        entryPoint: "vs",
        buffers: [
          {
            arrayStride: 40,
            attributes: [
              { shaderLocation: 0, offset: 0, format: "float32x3" },
              { shaderLocation: 1, offset: 12, format: "float32x2" },
              { shaderLocation: 2, offset: 20, format: "float32x4" },
              { shaderLocation: 3, offset: 36, format: "float32" },
            ],
          },
        ],
      },
      fragment: {
        module,
        entryPoint: "fsSmoke",
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
        format: "depth24plus",
        depthWriteEnabled: false,
        depthCompare: "less-equal",
      },
      multisample: { count: 4 },
    });
    const aimOverlayPipeline = await device.createRenderPipelineAsync({
      ...additiveDescriptor,
      label: "Spell aim overlay",
      depthStencil: {
        format: "depth24plus",
        depthWriteEnabled: false,
        depthCompare: "always",
      },
    });
    const vertices = device.createBuffer({
      size: GAMEPLAY_VISUAL_LIMITS.vertices * 40,
      usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
    });
    const uniform = device.createBuffer({
      size: 64,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    const group = device.createBindGroup({
      layout: bindGroupLayout,
      entries: [{ binding: 0, resource: { buffer: uniform } }],
    });
    return new GameplayEffects(
      device,
      pipeline,
      smokePipeline,
      aimOverlayPipeline,
      vertices,
      uniform,
      group,
    );
  }
  render(
    pass: GPURenderPassEncoder,
    effects: readonly GameplayEffect[],
    view: Float32Array,
    eye: V3,
    forward: V3,
    marker?: GameplayAimMarker,
    wind?: ImpactWindSampler,
  ) {
    this.triangles = 0;
    this.aimTriangles = 0;
    if (!effects.length && !marker) return;
    const sample = this.sampler.sample(effects, eye, forward, marker, wind);
    if (!sample.count) return;
    this.triangles = sample.effectCount / 3;
    this.aimTriangles = (sample.count - sample.effectCount) / 3;
    this.device.queue.writeBuffer(this.uniform, 0, view);
    this.device.queue.writeBuffer(this.vertices, 0, sample.data);
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.group);
    pass.setVertexBuffer(0, this.vertices);
    const aimVertices = sample.count - sample.effectCount;
    const overlay = marker?.overlay === true && aimVertices > 0;
    const additiveCount = overlay
      ? sample.smokeStart - aimVertices
      : sample.smokeStart;
    if (additiveCount) pass.draw(additiveCount);
    if (sample.smokeCount) {
      pass.setPipeline(this.smokePipeline);
      pass.draw(sample.smokeCount, 1, sample.smokeStart);
    }
    if (overlay) {
      pass.setPipeline(this.aimOverlayPipeline);
      pass.draw(aimVertices, 1, sample.smokeStart - aimVertices);
    }
  }
  dispose() {
    this.vertices.destroy();
    this.uniform.destroy();
  }
}
