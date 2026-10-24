import type { InstanceBounds } from "../../packages/renderer/culling.ts";
import {
  CLUSTER_HEADER,
  CLUSTER_LIMITS,
  type ClusteredLightInput,
  type ClusteredLights,
} from "../../packages/renderer/clustered-lights.ts";
import {
  lightColor,
  lightIntensity,
} from "../../packages/renderer/lighting.ts";
import { inverse, type V3 } from "../../packages/renderer/math.ts";
import { SCENE_LINEAR_SCALE } from "../../packages/renderer/radiometry.ts";

interface FrameExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  af_alloc(bytes: number): number;
  af_cull(...args: number[]): number;
  af_compact(...args: number[]): number;
  af_cluster(...args: number[]): number;
}

/** A resident immutable scene. Prepare another handle after its source data changes. */
export interface CSceneHandle {
  readonly count: number;
  readonly drawCount: number;
  readonly hasWind: boolean;
  readonly pointers: Readonly<{
    spheres: number;
    aabbs: number;
    models: number;
    wind: number;
    ranges: number;
    visibility: number;
    outModels: number;
    outWind: number;
    sourceIndices: number;
    outRanges: number;
    rank: number;
  }>;
  readonly owner: object;
}

export interface CFrameResult {
  /** Borrowed views. The next cull of this handle overwrites their contents. */
  visibility: Uint8Array;
  models: Float32Array;
  wind: Float32Array;
  sourceIndices: Uint32Array;
  /** First instance and count for every original draw, including empty draws. */
  drawRanges: Uint32Array;
}

function integer(n: number, name: string, min: number, max: number): number {
  if (!Number.isSafeInteger(n) || n < min || n > max)
    throw new Error(`Use ${name} between ${min} and ${max}.`);
  return n;
}
function packed(n: number, name: string): number {
  const f = Math.fround(n);
  if (!Number.isFinite(n) || !Number.isFinite(f))
    throw new Error(`Use a finite float32 ${name}.`);
  return f;
}
function vector(v: ArrayLike<number>, name: string): V3 {
  if (v.length !== 3 || !Array.from(v).every(Number.isFinite))
    throw new Error(`Use three finite ${name} coordinates.`);
  return Array.from(v) as V3;
}
function unit(v: ArrayLike<number>, name: string): V3 {
  const result = vector(v, name),
    length = Math.hypot(...result);
  if (!Number.isFinite(length) || length < 1e-12)
    throw new Error(`Use a nonzero finite ${name} direction.`);
  return result.map((n) => n / length) as V3;
}

/** Freestanding C/Wasm render preparation; WebGPU objects stay in the existing host. */
export class CFrameCore {
  private readonly owner = {};
  private readonly matrix: number;
  private readonly eye: number;
  private readonly forward: number;
  private readonly records: number;
  private readonly bounds: number;
  private readonly stats: number;
  private counts = 0;
  private countsCapacity = 0;
  private storage = 0;
  private storageCapacity = 0;

  constructor(private readonly wasm: FrameExports) {
    this.matrix = this.alloc(16 * 8);
    this.eye = this.alloc(3 * 8);
    this.forward = this.alloc(3 * 8);
    this.records = this.alloc(CLUSTER_LIMITS.lights * 16 * 4);
    this.bounds = this.alloc(CLUSTER_LIMITS.lights * 6 * 4);
    this.stats = this.alloc(6 * 4);
  }

  get memoryBytes(): number {
    return this.wasm.memory.buffer.byteLength;
  }

  private alloc(bytes: number): number {
    const pointer = this.wasm.af_alloc(Math.max(8, bytes));
    if (!pointer)
      throw new Error(
        "The C town experiment reached its 256 MiB memory limit.",
      );
    return pointer;
  }

  private validatedRanges(
    bounds: InstanceBounds,
    models: Float32Array,
    draws: readonly { firstInstance: number; count: number }[],
    wind?: Float32Array,
  ): Uint32Array {
    const count = integer(bounds.count, "instance count", 0, 1_000_000);
    if (
      models.length !== count * 16 ||
      bounds.spheres.length !== count * 4 ||
      bounds.aabbs.length !== count * 6 ||
      (wind && wind.length !== count * 4)
    )
      throw new Error(
        "Use complete model matrices, bounds, and wind inputs for every instance.",
      );
    if (
      !models.every(Number.isFinite) ||
      !bounds.spheres.every(Number.isFinite) ||
      !bounds.aabbs.every(Number.isFinite) ||
      (wind && !wind.every(Number.isFinite))
    )
      throw new Error("Use finite instance data.");
    for (let i = 0; i < count; i++) {
      const start = i * 16;
      if (
        Math.abs(models[start + 3]) +
          Math.abs(models[start + 7]) +
          Math.abs(models[start + 11]) >
          1e-6 ||
        Math.abs(models[start + 15] - 1) > 1e-6
      )
        throw new Error("Compacted instances need affine model matrices.");
    }
    const ranges = new Uint32Array(draws.length * 2);
    draws.forEach((draw, i) => {
      integer(draw.firstInstance, "draw start", 0, count);
      integer(draw.count, "draw instance count", 0, count - draw.firstInstance);
      ranges.set([draw.firstInstance, draw.count], i * 2);
    });
    return ranges;
  }

  prepareScene(
    bounds: InstanceBounds,
    models: Float32Array,
    draws: readonly { firstInstance: number; count: number }[],
    wind?: Float32Array,
  ): CSceneHandle {
    const ranges = this.validatedRanges(bounds, models, draws, wind);
    const count = bounds.count;
    const pointers = {
      spheres: this.alloc(bounds.spheres.byteLength),
      aabbs: this.alloc(bounds.aabbs.byteLength),
      models: this.alloc(models.byteLength),
      wind: wind ? this.alloc(wind.byteLength) : 0,
      ranges: this.alloc(ranges.byteLength),
      visibility: this.alloc(count),
      outModels: this.alloc(models.byteLength),
      outWind: wind ? this.alloc(wind.byteLength) : 0,
      sourceIndices: this.alloc(count * 4),
      outRanges: this.alloc(ranges.byteLength),
      rank: this.alloc((count + 1) * 4),
    };
    // Allocation can grow Wasm memory, so acquire typed views after all allocations.
    const memory = this.wasm.memory.buffer;
    new Float64Array(memory, pointers.spheres, count * 4).set(bounds.spheres);
    new Float64Array(memory, pointers.aabbs, count * 6).set(bounds.aabbs);
    new Float32Array(memory, pointers.models, count * 16).set(models);
    if (wind) new Float32Array(memory, pointers.wind, count * 4).set(wind);
    new Uint32Array(memory, pointers.ranges, ranges.length).set(ranges);
    return {
      count,
      drawCount: draws.length,
      hasWind: !!wind,
      pointers,
      owner: this.owner,
    };
  }

  /** Refresh temporary source transforms without reserving another Wasm scene. */
  updateScene(
    scene: CSceneHandle,
    bounds: InstanceBounds,
    models: Float32Array,
    draws: readonly { firstInstance: number; count: number }[],
    wind?: Float32Array,
  ): void {
    if (scene.owner !== this.owner)
      throw new Error("Use a scene prepared by this C frame core.");
    if (
      bounds.count !== scene.count ||
      draws.length !== scene.drawCount ||
      !!wind !== scene.hasWind
    )
      throw new Error(
        "Keep the resident scene's instance, draw, and wind sizes when updating it.",
      );
    const ranges = this.validatedRanges(bounds, models, draws, wind);
    const memory = this.wasm.memory.buffer,
      p = scene.pointers;
    new Float64Array(memory, p.spheres, scene.count * 4).set(bounds.spheres);
    new Float64Array(memory, p.aabbs, scene.count * 6).set(bounds.aabbs);
    new Float32Array(memory, p.models, scene.count * 16).set(models);
    if (wind) new Float32Array(memory, p.wind, scene.count * 4).set(wind);
    new Uint32Array(memory, p.ranges, ranges.length).set(ranges);
  }

  cull(scene: CSceneHandle, matrix: ArrayLike<number>): CFrameResult {
    if (scene.owner !== this.owner)
      throw new Error("Use a scene prepared by this C frame core.");
    if (
      matrix.length !== 16 ||
      !Array.from(matrix).every(Number.isFinite) ||
      [3, 7, 11, 15].every((i) => matrix[i] === 0)
    )
      throw new Error("Frustum culling needs a finite view-projection matrix.");
    new Float64Array(this.wasm.memory.buffer, this.matrix, 16).set(
      Array.from(matrix),
    );
    const p = scene.pointers;
    this.wasm.af_cull(
      scene.count,
      p.spheres,
      p.aabbs,
      this.matrix,
      p.visibility,
    );
    const visible = this.wasm.af_compact(
      scene.count,
      p.models,
      p.wind,
      p.visibility,
      scene.drawCount,
      p.ranges,
      p.outModels,
      p.outWind,
      p.sourceIndices,
      p.outRanges,
      p.rank,
    );
    const memory = this.wasm.memory.buffer;
    return {
      visibility: new Uint8Array(memory, p.visibility, scene.count),
      models: new Float32Array(memory, p.outModels, visible * 16),
      wind: new Float32Array(
        memory,
        p.outWind,
        scene.hasWind ? visible * 4 : 0,
      ),
      sourceIndices: new Uint32Array(memory, p.sourceIndices, visible),
      drawRanges: new Uint32Array(memory, p.outRanges, scene.drawCount * 2),
    };
  }

  buildClusters(input: ClusteredLightInput): ClusteredLights {
    const width = integer(
      input.width,
      "viewport width",
      1,
      CLUSTER_LIMITS.viewport,
    );
    const height = integer(
      input.height,
      "viewport height",
      1,
      CLUSTER_LIMITS.viewport,
    );
    const tileSize = integer(
      input.tileSize ?? 64,
      "tile size",
      1,
      CLUSTER_LIMITS.viewport,
    );
    const depthSlices = integer(
      input.depthSlices ?? 16,
      "depth slices",
      1,
      CLUSTER_LIMITS.depthSlices,
    );
    const maxIndices = integer(
      input.maxIndices ?? 1048576,
      "light index capacity",
      0,
      CLUSTER_LIMITS.indices,
    );
    const tilesX = Math.ceil(width / tileSize),
      tilesY = Math.ceil(height / tileSize);
    const cellCount = integer(
      tilesX * tilesY * depthSlices,
      "cluster count",
      1,
      CLUSTER_LIMITS.cells,
    );
    const near = packed(input.near ?? 0.1, "near depth"),
      far = packed(input.far ?? 2000, "far depth");
    if (near <= 0 || far <= near)
      throw new Error("Use positive, increasing cluster depths.");
    if (input.viewProjection.length !== 16)
      throw new Error(
        "Use a finite, invertible 16-value view-projection matrix.",
      );
    const viewProjection = Float64Array.from(input.viewProjection, (n) =>
      packed(n, "view-projection matrix"),
    );
    inverse(viewProjection);
    const eye = vector(input.eye, "camera position").map((n) =>
      packed(n, "camera position"),
    ) as V3;
    const forward = unit(input.forward, "camera forward");
    const logDepthScale = packed(
      depthSlices / Math.log(far / near),
      "log depth scale",
    );
    const grid = {
      width,
      height,
      tileSize,
      tilesX,
      tilesY,
      depthSlices,
      near,
      far,
      logDepthScale,
    };
    integer(
      input.lights.length,
      "source light count",
      0,
      CLUSTER_LIMITS.lights,
    );
    const ids = new Set<string>(),
      lightIds: string[] = [],
      sourceIndices: number[] = [],
      records: number[] = [];
    for (const [sourceIndex, light] of input.lights.entries()) {
      if (
        !["point", "spot", "directional"].includes(light.kind) ||
        typeof light.enabled !== "boolean"
      )
        throw new Error("Use a supported light kind and enabled flag.");
      if (!light.enabled || light.kind === "directional") continue;
      if (typeof light.id !== "string" || !light.id || ids.has(light.id))
        throw new Error("Use a distinct, nonempty local light ID.");
      ids.add(light.id);
      const position = vector(light.position, "light position").map((n) =>
        packed(n, "light position"),
      );
      const range = packed(light.range ?? 10, "light range");
      if (range <= 0) throw new Error("Use a positive light range.");
      if (vector(light.color, "light color").some((n) => n < 0))
        throw new Error("Use nonnegative light colors.");
      const color = lightColor(light).map((n) => packed(n, "light color"));
      if (color.some((n) => n < 0))
        throw new Error("Use nonnegative light colors.");
      const strength = packed(
        lightIntensity(light) * SCENE_LINEAR_SCALE,
        "light strength",
      );
      const direction =
        light.kind === "spot" ? unit(light.direction, "spotlight") : [0, -1, 0];
      const inner = light.innerConeAngleRadians ?? 0,
        outer = light.outerConeAngleRadians ?? Math.PI / 4;
      if (
        light.kind === "spot" &&
        (!Number.isFinite(inner) ||
          !Number.isFinite(outer) ||
          inner < 0 ||
          inner > outer ||
          outer <= 0 ||
          outer > Math.PI)
      )
        throw new Error(
          "Use ordered spotlight cone angles between zero and pi.",
        );
      const exponent =
        light.inverseSquaredFalloff === false
          ? (light.falloffExponent ?? 8)
          : -1;
      if (
        !Number.isFinite(exponent) ||
        (light.inverseSquaredFalloff === false && exponent < 0)
      )
        throw new Error(
          "Use a finite, nonnegative legacy light falloff exponent.",
        );
      const shadow = light.castShadows
        ? (input.shadowIndices?.get(light.id) ?? -1)
        : -1;
      integer(shadow, "shadow frame index", -1, 65535);
      records.push(
        ...position,
        range,
        ...color,
        strength,
        ...direction,
        light.kind === "spot" ? 1 : 0,
        light.kind === "spot" ? Math.cos(inner) : 1,
        light.kind === "spot" ? Math.cos(outer) : -1,
        shadow,
        light.inverseSquaredFalloff === false ? Math.max(exponent, 0.0001) : -1,
      );
      sourceIndices.push(sourceIndex);
      lightIds.push(light.id);
    }
    const packedRecords = new Float32Array(records);
    if (!packedRecords.every(Number.isFinite))
      throw new Error("Use finite float32 light records.");
    const capacity =
      CLUSTER_HEADER.words + packedRecords.length + cellCount * 2 + maxIndices;
    if (cellCount > this.countsCapacity) {
      this.counts = this.alloc(cellCount * 4);
      this.countsCapacity = cellCount;
    }
    if (capacity > this.storageCapacity) {
      this.storage = this.alloc(capacity * 4);
      this.storageCapacity = capacity;
    }
    const memory = this.wasm.memory.buffer;
    new Float64Array(memory, this.matrix, 16).set(viewProjection);
    new Float64Array(memory, this.eye, 3).set(eye);
    new Float64Array(memory, this.forward, 3).set(forward);
    new Float32Array(memory, this.records, packedRecords.length).set(
      packedRecords,
    );
    const used = this.wasm.af_cluster(
      lightIds.length,
      this.records,
      this.matrix,
      this.eye,
      this.forward,
      width,
      height,
      tileSize,
      depthSlices,
      near,
      far,
      logDepthScale,
      maxIndices,
      this.storage,
      this.storageCapacity,
      this.counts,
      this.bounds,
      this.stats,
    );
    if (used < 0)
      throw new Error("The C light output exceeded its reserved storage.");
    const storage = new Uint32Array(memory, this.storage, used),
      floats = new Float32Array(memory, this.storage, used);
    const stats = new Uint32Array(memory, this.stats, 6);
    return {
      storage,
      lightFloats: floats.subarray(storage[1], storage[2]),
      cells: storage.subarray(storage[2], storage[3]),
      indices: storage.subarray(storage[3]),
      sourceIndices: Uint32Array.from(sourceIndices),
      lightIds,
      viewProjection,
      eye,
      forward,
      grid,
      stats: {
        candidateMemberships: stats[0],
        indexCount: stats[1],
        overflowCellCount: stats[2],
        maxCellLightCount: stats[3],
        globalOverflow: stats[4] !== 0,
        byteLength: stats[5],
      },
    };
  }
}

/** Load once, outside the frame loop. No Emscripten runtime or WebGPU shim is used. */
export async function createCFrameCore(
  source: string | URL | BufferSource,
): Promise<CFrameCore> {
  const bytes =
    typeof source === "string" || source instanceof URL
      ? await (await fetch(source)).arrayBuffer()
      : source;
  const { instance } = await WebAssembly.instantiate(bytes, {
    env: { af_log: Math.log },
  });
  return new CFrameCore(instance.exports as FrameExports);
}
