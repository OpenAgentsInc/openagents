import type { CreatureFrame } from "./creatures.ts";
import { FOG_HEIGHT_RATE } from "./fog.ts";
import { packWindField, WIND_FIELD_FLOATS } from "./shared-wind.ts";
import { mmorpgHeight } from "../runtime/mmorpg-terrain.ts";
import {
  GRASS_LAYOUT,
  GRASS_PREPARE_WGSL,
  GRASS_WGSL,
} from "./grass-shader.ts";

export const GRASS_LIMITS = Object.freeze({
  ...GRASS_LAYOUT,
  patchSize: 10,
  planeHalfExtent: 1024,
  clearingCount: 16,
  uniformBytes: 448 + WIND_FIELD_FLOATS * 4 + 16 + 16 * 16,
  patchBytes: (GRASS_LAYOUT.nearPatches + GRASS_LAYOUT.farPatches) * 16,
  bladeBytes:
    (GRASS_LAYOUT.nearPatches * GRASS_LAYOUT.nearBlades +
      GRASS_LAYOUT.farPatches * GRASS_LAYOUT.farBlades) *
    GRASS_LAYOUT.bladeFloats *
    4,
  indexBytes: (GRASS_LAYOUT.nearSegments + GRASS_LAYOUT.farSegments) * 6 * 2,
});

export interface GrassManifest {
  schema: "aethyrforge.grass.v1";
  profile: "quick-grass-v1";
  enabled: boolean;
  groundHeight: number;
  bladeWidth: number;
  bladeHeight: number;
  lodDistance: number;
  maxDistance: number;
  windStrength: number;
  interactionRadius: number;
  terrainProfile?: "quick-mmorpg-v1";
  /** Fixed world circles; only new patch roots evaluate their soft boundaries. */
  clearings?: readonly { center: readonly [number, number]; radius: number }[];
  metadata?: Record<string, unknown>;
}
function bounded(value: number, min: number, max: number) {
  if (
    typeof value !== "number" ||
    !Number.isFinite(value) ||
    value < min ||
    value > max
  )
    throw new Error("The grass settings need supported numbers.");
}
function finiteVector(value: ArrayLike<number>, length: number) {
  if (!value || value.length !== length)
    throw new Error("Grass needs complete world vectors.");
  for (let i = 0; i < length; i++)
    if (!Number.isFinite(value[i]))
      throw new Error("Grass needs finite world vectors.");
}
export function validateGrassManifest(value: GrassManifest) {
  if (
    value?.schema !== "aethyrforge.grass.v1" ||
    value.profile !== "quick-grass-v1" ||
    typeof value.enabled !== "boolean"
  )
    throw new Error("Choose a supported grass profile.");
  bounded(value.groundHeight, -1000, 1000);
  bounded(value.bladeWidth, 0.02, 0.3);
  bounded(value.bladeHeight, 0.1, 2);
  bounded(value.lodDistance, 5, 20);
  bounded(value.maxDistance, value.lodDistance + 5, 100);
  bounded(value.windStrength, 0, 2);
  bounded(value.interactionRadius, 0.7, 5);
  if (
    value.terrainProfile !== undefined &&
    value.terrainProfile !== "quick-mmorpg-v1"
  )
    throw new Error("Choose a supported grass terrain.");
  if (value.clearings !== undefined) {
    if (
      !Array.isArray(value.clearings) ||
      value.clearings.length > GRASS_LIMITS.clearingCount
    )
      throw new Error("Choose up to sixteen grass clearings.");
    for (const clearing of value.clearings) {
      if (!clearing || !Array.isArray(clearing.center))
        throw new Error("Grass clearings need a world position.");
      finiteVector(clearing.center, 2);
      for (const position of clearing.center) bounded(position, -20000, 20000);
      bounded(clearing.radius, 0.1, 12);
    }
  }
  return value;
}

/** The shader uses the same inward feather; roots outside every circle stay exact. */
export function grassClearingScale(
  x: number,
  z: number,
  clearings: GrassManifest["clearings"] = [],
) {
  let scale = 1;
  for (const { center, radius } of clearings) {
    const edge = Math.min(
      1,
      Math.max(
        0,
        (Math.hypot(x - center[0], z - center[1]) -
          Math.max(0, radius - 0.75)) /
          Math.min(0.75, radius),
      ),
    );
    scale = Math.min(scale, edge * edge * (3 - 2 * edge));
  }
  return scale;
}

const offsets = Array.from(
  { length: 23 * 23 },
  (_, id) => [(id % 23) - 11, Math.floor(id / 23) - 11] as const,
).sort((a, b) => a[0] * a[0] + a[1] * a[1] - b[0] * b[0] - b[1] * b[1]);

/** Bounded patch slots keep roots stable; dirty slots refresh the GPU cache. */
export class GrassPatchPlanner {
  /** Detail's share of `maxDistance`; never below `lodDistance + 5`. */
  reach = 1;
  get drawDistance() {
    return Math.max(
      this.manifest.lodDistance + 5,
      this.manifest.maxDistance * this.reach,
    );
  }
  readonly data = new Float32Array(
    (GRASS_LIMITS.nearPatches + GRASS_LIMITS.farPatches) * 4,
  );
  nearPatches = 0;
  farPatches = 0;
  droppedPatches = 0;
  private planes = new Float64Array(24);
  private initialized = new Uint8Array(
    GRASS_LIMITS.nearPatches + GRASS_LIMITS.farPatches,
  );
  constructor(public manifest: GrassManifest) {
    validateGrassManifest(manifest);
  }
  update(eye: readonly number[], view: ArrayLike<number>) {
    this.nearPatches = this.farPatches = this.droppedPatches = 0;
    if (!this.manifest.enabled) return;
    finiteVector(eye, 3);
    finiteVector(view, 16);
    const p = this.planes;
    for (let plane = 0; plane < 6; plane++) {
      const row = Math.floor(plane / 2),
        sign = plane % 2 === 0 ? 1 : -1;
      for (let c = 0; c < 4; c++)
        p[plane * 4 + c] =
          plane === 4
            ? view[c * 4 + 2]
            : view[c * 4 + 3] + sign * view[c * 4 + row];
    }
    const baseX = Math.floor(eye[0] / 10) * 10,
      baseZ = Math.floor(eye[2] / 10) * 10;
    const radius2 = this.drawDistance ** 2,
      near2 = this.manifest.lodDistance ** 2;
    for (let i = 0; i < offsets.length; i++) {
      const x = baseX + offsets[i][0] * 10,
        z = baseZ + offsets[i][1] * 10;
      if (
        Math.abs(x) >= GRASS_LIMITS.planeHalfExtent + 5 ||
        Math.abs(z) >= GRASS_LIMITS.planeHalfExtent + 5
      )
        continue;
      const dx = Math.max(0, Math.abs(x - eye[0]) - 5),
        dz = Math.max(0, Math.abs(z - eye[2]) - 5);
      const distance2 = dx * dx + dz * dz;
      if (distance2 > radius2) continue;
      // Conservative blade/wind bounds include the tallest reference blade and player bending.
      const terrain = this.manifest.terrainProfile === "quick-mmorpg-v1";
      const y =
        (terrain ? mmorpgHeight(x, z) : this.manifest.groundHeight) +
        this.manifest.bladeHeight;
      const halfY = this.manifest.bladeHeight * 1.6 + (terrain ? 20 : 0),
        halfXZ = 5 + this.manifest.bladeHeight * 2.5;
      let visible = true;
      for (let plane = 0; plane < 6; plane++) {
        const at = plane * 4;
        if (
          p[at] * x +
            p[at + 1] * y +
            p[at + 2] * z +
            p[at + 3] +
            Math.abs(p[at]) * halfXZ +
            Math.abs(p[at + 1]) * halfY +
            Math.abs(p[at + 2]) * halfXZ <
          0
        ) {
          visible = false;
          break;
        }
      }
      if (!visible) continue;
      let slot: number;
      if (distance2 <= near2 && this.nearPatches < GRASS_LIMITS.nearPatches)
        slot = this.nearPatches++;
      else if (this.farPatches < GRASS_LIMITS.farPatches)
        slot = GRASS_LIMITS.nearPatches + this.farPatches++;
      else {
        this.droppedPatches++;
        continue;
      }
      const at = slot * 4;
      const dirty =
        !this.initialized[slot] ||
        this.data[at] !== x ||
        this.data[at + 1] !== z;
      this.data[at] = x;
      this.data[at + 1] = z;
      this.data[at + 2] = dirty ? 1 : 0;
      this.data[at + 3] = 0;
      this.initialized[slot] = 1;
    }
  }
}

export interface GrassStats {
  nearPatches: number;
  farPatches: number;
  nearBlades: number;
  farBlades: number;
  blades: number;
  triangles: number;
  drawCalls: number;
  droppedPatches: number;
  windTime: number;
  accountedBytes: number;
  computeDispatches: number;
  computedBlades: number;
  cacheRebuildBlades: number;
}
export interface GrassOptions {
  format?: GPUTextureFormat;
  depthFormat?: GPUTextureFormat;
  sampleCount?: number;
}

/** One bounded compute pass prepares cached clumps/wind; two indexed draws make blades. */
export class Grass {
  readonly accountedBytes =
    GRASS_LIMITS.uniformBytes +
    GRASS_LIMITS.patchBytes +
    GRASS_LIMITS.bladeBytes +
    GRASS_LIMITS.indexBytes;
  readonly planner: GrassPatchPlanner;
  readonly stats: GrassStats = {
    nearPatches: 0,
    farPatches: 0,
    nearBlades: 0,
    farBlades: 0,
    blades: 0,
    triangles: 0,
    drawCalls: 0,
    droppedPatches: 0,
    windTime: 0,
    accountedBytes: this.accountedBytes,
    computeDispatches: 0,
    computedBlades: 0,
    cacheRebuildBlades: 0,
  };
  private settings = new Float32Array(GRASS_LIMITS.uniformBytes / 4);
  private disposed = false;
  private preparedFrame?: CreatureFrame;
  private preparedNear = 0;
  private preparedFar = 0;
  private constructor(
    private device: GPUDevice,
    private currentManifest: GrassManifest,
    private pipelines: readonly [GPURenderPipeline, GPURenderPipeline],
    private compute: readonly [GPUComputePipeline, GPUComputePipeline],
    private uniform: GPUBuffer,
    private patches: GPUBuffer,
    private blades: GPUBuffer,
    private indices: GPUBuffer,
    private groups: readonly [GPUBindGroup, GPUBindGroup],
    private computeGroups: readonly [GPUBindGroup, GPUBindGroup],
    private sampleCount: number,
  ) {
    this.planner = new GrassPatchPlanner(currentManifest);
  }
  get manifest() {
    return this.currentManifest;
  }
  /** Update data in the next frame without replacing roots, buffers, or player state. */
  setAppearance(
    appearance: Pick<
      GrassManifest,
      "bladeHeight" | "bladeWidth" | "windStrength"
    >,
  ) {
    if (this.disposed) throw new Error("Open the world before changing grass.");
    if (
      !appearance ||
      Object.keys(appearance).length !== 3 ||
      Object.keys(appearance).some(
        (key) => !["bladeHeight", "bladeWidth", "windStrength"].includes(key),
      )
    )
      throw new Error("Choose supported grass settings.");
    const next = Object.freeze(
      validateGrassManifest({ ...this.currentManifest, ...appearance }),
    );
    this.currentManifest = next;
    this.planner.manifest = next;
  }
  /** Scale the draw distance for Detail (ADR 0007); 1 is the manifest's. */
  setReach(reach: number) {
    if (!Number.isFinite(reach) || reach < 0.1 || reach > 1)
      throw new Error("Use a grass reach from 0.1 to 1.");
    this.planner.reach = reach;
  }
  static async create(
    device: GPUDevice,
    manifest: GrassManifest,
    options: GrassOptions = {},
  ) {
    validateGrassManifest(manifest);
    const sampleCount = options.sampleCount ?? 4;
    if (![1, 4].includes(sampleCount))
      throw new Error("Grass needs one or four samples.");
    const module = device.createShaderModule({
      label: "Quick_Grass procedural blades",
      code: GRASS_WGSL,
    });
    const preparation = device.createShaderModule({
      label: "Grass cached clumps and wind",
      code: GRASS_PREPARE_WGSL,
    });
    const pipeline = (entryPoint: string) =>
      device.createRenderPipelineAsync({
        label: `Grass ${entryPoint}`,
        layout: "auto",
        vertex: { module, entryPoint },
        fragment: {
          module,
          entryPoint: "fs",
          targets: [{ format: options.format ?? "rgba16float" }],
        },
        primitive: { topology: "triangle-list", cullMode: "none" },
        depthStencil: {
          format: options.depthFormat ?? "depth24plus",
          depthWriteEnabled: true,
          depthCompare: "less-equal",
        },
        multisample: {
          count: sampleCount,
          alphaToCoverageEnabled: sampleCount > 1,
        },
      });
    const [near, far, computeNear, computeFar] = await Promise.all([
      pipeline("vsNear"),
      pipeline("vsFar"),
      device.createComputePipelineAsync({
        label: "Grass near preparation",
        layout: "auto",
        compute: { module: preparation, entryPoint: "prepareNear" },
      }),
      device.createComputePipelineAsync({
        label: "Grass far preparation",
        layout: "auto",
        compute: { module: preparation, entryPoint: "prepareFar" },
      }),
    ]);
    const owned: GPUBuffer[] = [];
    try {
      const make = (
        label: string,
        size: number,
        usage: GPUBufferUsageFlags,
      ) => {
        const value = device.createBuffer({ label, size, usage });
        owned.push(value);
        return value;
      };
      const uniform = make(
        "Grass frame",
        GRASS_LIMITS.uniformBytes,
        GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
      );
      const patches = make(
        "Grass patches",
        GRASS_LIMITS.patchBytes,
        GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
      );
      const blades = make(
        "Grass cached blades",
        GRASS_LIMITS.bladeBytes,
        GPUBufferUsage.STORAGE,
      );
      const indices = make(
        "Grass shared index templates",
        GRASS_LIMITS.indexBytes,
        GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST,
      );
      const indexData = new Uint16Array(GRASS_LIMITS.indexBytes / 2);
      let at = 0;
      for (const segments of [
        GRASS_LIMITS.nearSegments,
        GRASS_LIMITS.farSegments,
      ])
        for (let segment = 0; segment < segments; segment++) {
          const v = segment * 2;
          indexData.set([v, v + 1, v + 2, v + 2, v + 1, v + 3], at);
          at += 6;
        }
      device.queue.writeBuffer(indices, 0, indexData);
      const groups = [near, far].map((p) =>
        device.createBindGroup({
          layout: p.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: { buffer: uniform } },
            { binding: 2, resource: { buffer: blades } },
          ],
        }),
      ) as [GPUBindGroup, GPUBindGroup];
      const computeGroups = [computeNear, computeFar].map((p) =>
        device.createBindGroup({
          layout: p.getBindGroupLayout(0),
          entries: [
            { binding: 0, resource: { buffer: uniform } },
            { binding: 1, resource: { buffer: patches } },
            { binding: 2, resource: { buffer: blades } },
          ],
        }),
      ) as [GPUBindGroup, GPUBindGroup];
      return new Grass(
        device,
        Object.freeze({ ...manifest }),
        [near, far],
        [computeNear, computeFar],
        uniform,
        patches,
        blades,
        indices,
        groups,
        computeGroups,
        sampleCount,
      );
    } catch (error) {
      for (const buffer of owned) buffer.destroy();
      throw error;
    }
  }
  private reset(time: number) {
    const s = this.stats;
    s.nearPatches =
      s.farPatches =
      s.nearBlades =
      s.farBlades =
      s.blades =
      s.triangles =
      s.drawCalls =
      s.droppedPatches =
      s.computeDispatches =
      s.computedBlades =
      s.cacheRebuildBlades =
        0;
    s.windTime = time;
    this.preparedNear = this.preparedFar = 0;
  }
  /** Call before beginning the main render pass on the same command encoder. */
  prepare(
    encoder: GPUCommandEncoder,
    frame: CreatureFrame,
    player?: { position: readonly number[] },
  ) {
    this.reset(frame.time);
    this.preparedFrame = frame;
    if (this.disposed || !this.manifest.enabled || frame.enabled === false)
      return this.stats;
    if (!Number.isFinite(frame.time))
      throw new Error("Grass needs a finite world time.");
    finiteVector(frame.diffuseRotation, 16);
    finiteVector(frame.diffuseSH, 36);
    finiteVector(frame.sunDirection, 3);
    finiteVector(frame.sunIlluminance, 3);
    if (frame.ambient) finiteVector(frame.ambient, 3);
    if (player) finiteVector(player.position, 3);
    if (frame.fog) {
      bounded(frame.fog.density, 0, 10000);
      bounded(frame.fog.heightRate ?? FOG_HEIGHT_RATE, 0, 10);
      finiteVector(frame.fog.color, 3);
      if (frame.fog.volumeMedium) finiteVector(frame.fog.volumeMedium, 4);
      if (frame.fog.volumeRange) finiteVector(frame.fog.volumeRange, 2);
    }
    this.planner.update(frame.eye, frame.viewProjection);
    const near = this.planner.nearPatches,
      far = this.planner.farPatches;
    this.preparedNear = near;
    this.preparedFar = far;
    if (!near && !far) return this.stats;
    const u = this.settings;
    u.fill(0);
    u.set(frame.viewProjection, 0);
    u.set(frame.diffuseRotation, 16);
    for (let i = 0; i < 3; i++) {
      u[32 + i] = frame.eye[i];
      u[36 + i] = frame.sunDirection[i];
      u[40 + i] = frame.sunIlluminance[i];
      u[44 + i] = frame.ambient?.[i] ?? 0;
      u[52 + i] = frame.fog?.color[i] ?? 0;
      u[100 + i] = player?.position[i] ?? 0;
    }
    u[35] = frame.time;
    u[48] = frame.fog?.density ?? 0;
    u[49] = frame.fog?.heightRate ?? FOG_HEIGHT_RATE;
    u[50] = frame.fog?.profile === "quick-mmorpg-v1" ? (frame.fog.sourceScale ?? 1) : 0;
    if (frame.fog?.volumeMedium) u.set(frame.fog.volumeMedium, 56);
    else u[58] = 1;
    if (frame.fog?.volumeRange) u.set(frame.fog.volumeRange, 60);
    u.set(frame.diffuseSH, 64);
    u[103] = player ? 1 : 0;
    u[104] = this.manifest.groundHeight;
    u[105] = this.manifest.bladeWidth;
    u[106] = this.manifest.bladeHeight;
    u[107] = this.manifest.windStrength;
    u[108] = this.manifest.lodDistance;
    u[109] = this.planner.drawDistance;
    u[110] = this.manifest.interactionRadius;
    u[111] = this.sampleCount;
    u[63] = this.manifest.terrainProfile === "quick-mmorpg-v1" ? 1 : 0;
    if (frame.sharedWind) packWindField(frame.sharedWind, u, 112);
    u[124] = this.manifest.clearings?.length ?? 0;
    this.manifest.clearings?.forEach(({ center, radius }, i) => {
      u.set([center[0], center[1], radius, 0], 128 + i * 4);
    });
    this.device.queue.writeBuffer(this.uniform, 0, u);
    this.device.queue.writeBuffer(this.patches, 0, this.planner.data);
    let rebuilt = 0;
    for (let i = 0; i < near; i++)
      if (this.planner.data[i * 4 + 2] > 0.5)
        rebuilt += GRASS_LIMITS.nearBlades;
    for (let i = 0; i < far; i++)
      if (this.planner.data[(GRASS_LIMITS.nearPatches + i) * 4 + 2] > 0.5)
        rebuilt += GRASS_LIMITS.farBlades;
    const pass = encoder.beginComputePass({
      label: "Grass clump cache and wind",
    });
    if (near) {
      pass.setPipeline(this.compute[0]);
      pass.setBindGroup(0, this.computeGroups[0]);
      pass.dispatchWorkgroups((near * GRASS_LIMITS.nearBlades) / 64);
      this.stats.computeDispatches++;
    }
    if (far) {
      pass.setPipeline(this.compute[1]);
      pass.setBindGroup(0, this.computeGroups[1]);
      pass.dispatchWorkgroups((far * GRASS_LIMITS.farBlades) / 64);
      this.stats.computeDispatches++;
    }
    pass.end();
    this.stats.cacheRebuildBlades = rebuilt;
    this.stats.computedBlades =
      near * GRASS_LIMITS.nearBlades + far * GRASS_LIMITS.farBlades;
    return this.stats;
  }
  render(
    pass: GPURenderPassEncoder,
    frame: CreatureFrame,
    _player?: { position: readonly number[] },
  ) {
    if (this.disposed || !this.manifest.enabled || frame.enabled === false) {
      this.reset(frame.time);
      return this.stats;
    }
    if (this.preparedFrame !== frame)
      throw new Error("Prepare grass before drawing the world.");
    const near = this.preparedNear,
      far = this.preparedFar,
      s = this.stats;
    if (!near && !far) return s;
    pass.setIndexBuffer(this.indices, "uint16");
    const nearCount = GRASS_LIMITS.nearSegments * 6,
      farCount = GRASS_LIMITS.farSegments * 6;
    if (near) {
      pass.setPipeline(this.pipelines[0]);
      pass.setBindGroup(0, this.groups[0]);
      pass.drawIndexed(nearCount, near * GRASS_LIMITS.nearBlades, 0, 0, 0);
      s.drawCalls++;
    }
    if (far) {
      pass.setPipeline(this.pipelines[1]);
      pass.setBindGroup(0, this.groups[1]);
      pass.drawIndexed(farCount, far * GRASS_LIMITS.farBlades, nearCount, 0, 0);
      s.drawCalls++;
    }
    s.nearPatches = near;
    s.farPatches = far;
    s.nearBlades = near * GRASS_LIMITS.nearBlades;
    s.farBlades = far * GRASS_LIMITS.farBlades;
    s.blades = s.nearBlades + s.farBlades;
    s.triangles =
      s.nearBlades * GRASS_LIMITS.nearSegments * 2 +
      s.farBlades * GRASS_LIMITS.farSegments * 2;
    s.droppedPatches = this.planner.droppedPatches;
    this.preparedFrame = undefined;
    return s;
  }
  dispose() {
    if (this.disposed) return;
    this.disposed = true;
    this.uniform.destroy();
    this.patches.destroy();
    this.blades.destroy();
    this.indices.destroy();
    this.reset(this.stats.windTime);
    this.preparedFrame = undefined;
  }
}
