import { DebrisGpuPacking, type PackingBounds } from "./debris-gpu-packing.ts";
import type { CompiledScene } from "../assets/src/scene.ts";
import {
  fracturePlacementSteps,
  fractureSurfaceAreaSteps,
  type FractureChunk,
  type FractureSource,
} from "./source-fracture.ts";
import {
  GAMEPLAY_VISUAL_LIMITS,
  type GameplayDebris,
  type GameplayObject,
} from "./gameplay-effects.ts";
import { modelMatrix, transformPoint, type V3 } from "./math.ts";
import { frustumVisibility, type InstanceBounds } from "./culling.ts";

export const SOURCE_DEBRIS_LIMITS = Object.freeze({
  standingVertices: 212992,
  debrisVertices: 262144,
  rubbleVertices: 393216,
  cacheBytes: 160 * 1024 * 1024,
  largeSources: 6,
  sources: 262,
});
export interface GameplayTargetGroups {
  castle: readonly number[];
}
type Draw = {
  material: number;
  firstIndex: number;
  indexCount: number;
  instance: number;
};
type Cell = {
  object: GameplayObject;
  whole: FractureChunk;
  fragments?: FractureChunk[];
  fineFragments?: FractureChunk[];
};
type Buffers = {
  vertices: GPUBuffer;
  colors: GPUBuffer;
  uv1: GPUBuffer;
  indices: GPUBuffer;
  models: GPUBuffer;
};
function bytes(chunk: FractureChunk) {
  if (!chunk.vertices.length) return 0;
  return (
    chunk.vertices.byteLength +
    chunk.colors.byteLength +
    chunk.uv1.byteLength +
    384 +
    chunk.draws.length * 48
  );
}
function fragmentBytes(cell: Cell) {
  return [cell.fragments, cell.fineFragments].reduce(
    (total, fragments) =>
      total +
      (fragments
        ? 24 +
          fragments.length * 8 +
          fragments.reduce((n, part) => n + bytes(part), 0)
        : 0),
    0,
  );
}
function eulerMatrix(position: V3, rotation: V3, amount: number): Float32Array {
  const [x, y, z] = rotation,
    cx = Math.cos(x),
    sx = Math.sin(x),
    cy = Math.cos(y),
    sy = Math.sin(y),
    cz = Math.cos(z),
    sz = Math.sin(z);
  return new Float32Array([
    cz * cy * amount,
    sz * cy * amount,
    -sy * amount,
    0,
    (cz * sy * sx - sz * cx) * amount,
    (sz * sy * sx + cz * cx) * amount,
    cy * sx * amount,
    0,
    (cz * sy * cx + sz * sx) * amount,
    (sz * sy * cx - cz * sx) * amount,
    cy * cx * amount,
    0,
    ...position,
    1,
  ]);
}

/** Fixed GPU slabs retain real textured cell surfaces and rigid source fragments. */
export class SourceDebris {
  readonly bytes =
    (SOURCE_DEBRIS_LIMITS.standingVertices +
      SOURCE_DEBRIS_LIMITS.debrisVertices) *
      64 +
    SOURCE_DEBRIS_LIMITS.rubbleVertices * 64 +
    258 * 64;
  private objects = new Map<string, Map<number, GameplayObject>>();
  private bySource = new Map<string, Map<number, GameplayObject[]>>();
  private castleSources = new Map<string, Set<number>>();
  private castleNodes = new Map<string, readonly number[]>();
  private fineMasks = new Map<string, Map<number, [number, number]>>();
  private prepareGeneration = 0;
  private cells = new Map<string, Cell>();
  private prepared = new Map<string, Set<number>>();
  private destroyed = new Map<string, Set<number>>();
  private activeDebris: readonly GameplayDebris[] = [];
  private activeRubble: readonly GameplayDebris[] = [];
  private mode = "";
  private standingKey = "";
  private movingKey = "";
  private rubbleKey = "";
  private standingDraws: Draw[] = [];
  private movingDraws: Draw[] = [];
  private movingSlots: GameplayDebris[] = [];
  private rubbleDraws: Draw[] = [];
  private standing: Buffers;
  private moving: Buffers;
  private rubble: Buffers;
  private standingBounds?: InstanceBounds;
  private rubbleBounds?: InstanceBounds;
  staticRevision = 0;
  private modelData = new Float32Array(256 * 16);
  limitReason = "";
  private stats = {
    activeFracturedSources: 0,
    standingCells: 0,
    standingTriangles: 0,
    renderedDebris: 0,
    debrisTriangles: 0,
    maskedNodes: 0,
    rubbleCount: 0,
    rubbleTriangles: 0,
  };
  private preparation = {
    standingVertices: 0,
    debrisVertices: 0,
    rubbleVertices: 0,
    cacheBytes: 0,
    transientBytes: 0,
    sources: 0,
  };
  get preparationStats() {
    return { ...this.preparation };
  }
  constructor(
    private device: GPUDevice,
    private scenes: ReadonlyMap<string, CompiledScene>,
    private source: FractureSource,
    private packing?: DebrisGpuPacking,
  ) {
    const make = (vertices: number, models: number): Buffers => {
      const buffer = (size: number, usage: GPUBufferUsageFlags) =>
        device.createBuffer({
          size,
          usage:
            usage |
            GPUBufferUsage.COPY_DST |
            (packing ? GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC : 0),
        });
      const result = {
        vertices: buffer(vertices * 48, GPUBufferUsage.VERTEX),
        colors: buffer(vertices * 4, GPUBufferUsage.VERTEX),
        uv1: buffer(vertices * 8, GPUBufferUsage.VERTEX),
        indices: buffer(vertices * 4, GPUBufferUsage.INDEX),
        models: buffer(models * 64, GPUBufferUsage.VERTEX),
      };
      device.queue.writeBuffer(
        result.indices,
        0,
        Uint32Array.from({ length: vertices }, (_, i) => i),
      );
      return result;
    };
    this.standing = make(SOURCE_DEBRIS_LIMITS.standingVertices, 1);
    this.moving = make(SOURCE_DEBRIS_LIMITS.debrisVertices, 256);
    this.rubble = make(SOURCE_DEBRIS_LIMITS.rubbleVertices, 1);
    device.queue.writeBuffer(
      this.standing.models,
      0,
      new Float32Array(modelMatrix([0, 0, 0])),
    );
    device.queue.writeBuffer(
      this.rubble.models,
      0,
      new Float32Array(modelMatrix([0, 0, 0])),
    );
  }
  setObjects(
    mode: string,
    objects: readonly GameplayObject[],
    targetGroups?: GameplayTargetGroups,
  ) {
    if (objects.length > 65536)
      throw Error("The destructible scene has too many cells.");
    const nodes = new Map<number, GameplayObject>(),
      groups = new Map<number, GameplayObject[]>();
    const scene = this.scenes.get(mode);
    for (const object of objects) {
      if (
        !Number.isSafeInteger(object.id) ||
        object.id < 0 ||
        nodes.has(object.id) ||
        !scene?.instances[object.sourceId] ||
        ![...object.min, ...object.max].every(Number.isFinite) ||
        object.min.some((v, i) => v > object.max[i])
      )
        throw Error(
          "Destructible cells need unique IDs and valid source bounds.",
        );
      const owned = {
        ...object,
        min: [...object.min] as V3,
        max: [...object.max] as V3,
      };
      nodes.set(object.id, owned);
      const list = groups.get(object.sourceId) ?? [];
      list.push(owned);
      groups.set(object.sourceId, list);
    }
    const castleNodes = new Set(targetGroups?.castle ?? []);
    if (castleNodes.size !== (targetGroups?.castle.length ?? 0))
      throw Error("The castle contains repeated town cells.");
    const castleSources = new Set<number>();
    for (const id of castleNodes) {
      const object = nodes.get(id);
      if (!object) throw Error("The castle contains an unavailable town cell.");
      castleSources.add(object.sourceId);
    }
    for (const sourceId of castleSources) {
      if (groups.get(sourceId)!.some((node) => !castleNodes.has(node.id)))
        throw Error("The castle needs complete source pieces.");
    }
    const previous = this.objects.get(mode);
    const previousCastle = this.castleSources.get(mode);
    const same =
      previous?.size === nodes.size &&
      Array.from(nodes, ([id, object]) => {
        const old = previous.get(id);
        return (
          !!old &&
          old.sourceId === object.sourceId &&
          old.min.every((v, i) => v === object.min[i]) &&
          old.max.every((v, i) => v === object.max[i])
        );
      }).every(Boolean) &&
      previousCastle?.size === castleSources.size &&
      Array.from(castleSources).every((id) => previousCastle.has(id));
    if (this.mode && this.mode !== mode) this.reset(this.mode);
    if (!same) {
      this.objects.set(mode, nodes);
      this.bySource.set(mode, groups);
      this.castleSources.set(mode, castleSources);
    }
    this.castleNodes.set(mode, Array.from(castleNodes));
    this.reset(mode, same);
  }
  reset(mode: string, keepGeometry = false) {
    this.prepareGeneration++;
    for (const key of this.cells.keys())
      if (!keepGeometry && key.startsWith(mode + ":")) this.cells.delete(key);
    if (!keepGeometry) this.fineMasks.delete(mode);
    this.prepared.delete(mode);
    this.destroyed.delete(mode);
    this.standingKey = "";
    this.movingKey = "";
    this.rubbleKey = "";
    this.activeDebris = [];
    this.activeRubble = [];
    if (this.mode === mode) {
      this.standingDraws = [];
      this.movingDraws = [];
      this.movingSlots = [];
      this.rubbleDraws = [];
      this.stats = {
        activeFracturedSources: 0,
        standingCells: 0,
        standingTriangles: 0,
        renderedDebris: 0,
        debrisTriangles: 0,
        maskedNodes: 0,
        rubbleCount: 0,
        rubbleTriangles: 0,
      };
    }
    this.limitReason = "";
    this.packing?.retain(this.packingChunks());
  }
  /** Restore play state without rebuilding the admitted cells or their fragments. */
  resetPlayState(mode: string): boolean {
    // A pending invisible candidate must not publish after restoration, even
    // when the currently displayed town was already pristine.
    this.prepareGeneration++;
    this.packing?.discardStaged();
    this.prepared.delete(mode);
    const active = !this.mode || this.mode === mode;
    const changed =
      (this.destroyed.get(mode)?.size ?? 0) > 0 ||
      (active &&
        (this.standingDraws.length > 0 ||
          this.movingDraws.length > 0 ||
          this.rubbleDraws.length > 0));
    this.destroyed.delete(mode);
    if (active) {
      this.mode = mode;
      // Match the next empty native snapshot so it does not repack empty slabs
      // or change their revision after an otherwise idle restore.
      this.standingKey = mode + ":";
      this.movingKey = mode + ":";
      this.rubbleKey = mode + ":";
      this.activeDebris = [];
      this.activeRubble = [];
      this.standingDraws = [];
      this.movingDraws = [];
      this.movingSlots = [];
      this.rubbleDraws = [];
      this.standingBounds = undefined;
      this.rubbleBounds = undefined;
      for (const field of Object.keys(
        this.stats,
      ) as (keyof typeof this.stats)[])
        this.stats[field] = 0;
    }
    if (changed) this.staticRevision++;
    this.limitReason = "";
    return changed;
  }
  /** Builds an invisible candidate first. A refusal cannot hide any current surface. */
  prepare(mode: string, prospective: readonly number[]): boolean {
    const generation = ++this.prepareGeneration;
    const steps = this.prepareSteps(
      mode,
      prospective,
      () => generation !== this.prepareGeneration,
    );
    for (;;) {
      const step = steps.next();
      if (step.done) return step.value;
    }
  }
  async prepareAsync(
    mode: string,
    prospective: readonly number[],
    cancelled: () => boolean = () => false,
  ): Promise<boolean> {
    const generation = ++this.prepareGeneration;
    const interrupted = () =>
      cancelled() || generation !== this.prepareGeneration;
    const steps = this.prepareSteps(mode, prospective, interrupted);
    let batchStarted = performance.now();
    for (;;) {
      if (interrupted()) {
        steps.return(false);
        return false;
      }
      const step = steps.next();
      if (step.done) return step.value;
      if (performance.now() - batchStarted < 2) continue;
      const scheduler = (
        globalThis as typeof globalThis & {
          scheduler?: { yield(): Promise<void> };
        }
      ).scheduler;
      if (typeof scheduler?.yield === "function") await scheduler.yield();
      else await new Promise<void>((resolve) => setTimeout(resolve, 0));
      batchStarted = performance.now();
    }
  }
  private *prepareSteps(
    mode: string,
    prospective: readonly number[],
    cancelled: () => boolean,
  ): Generator<void, boolean> {
    try {
      const scene = this.scenes.get(mode),
        nodes = this.objects.get(mode),
        groups = this.bySource.get(mode);
      if (!scene || !nodes || !groups)
        throw Error("Prepare the destructible town cells first.");
      const wanted = new Set(prospective);
      if (
        wanted.size !== prospective.length ||
        wanted.size > 65536 ||
        prospective.some((id) => !nodes.has(id))
      )
        throw Error("The destruction plan contains an unavailable cell.");
      const current = this.destroyed.get(mode) ?? new Set<number>();
      const liveSources = new Set(
        [
          ...this.activeDebris.filter((d) => d.age < d.life),
          ...this.activeRubble,
        ].map((d) => d.sourceId),
      );
      const sources = new Set(
        prospective
          .map((id) => nodes.get(id)!.sourceId)
          .filter(
            (source) =>
              liveSources.has(source) ||
              groups.get(source)!.some((node) => !current.has(node.id)),
          ),
      );
      if (sources.size > SOURCE_DEBRIS_LIMITS.sources)
        throw Error(
          "This spell would fracture too many town pieces at once. Choose Restore town to start again.",
        );
      let large = 0;
      let castleDistrict = false;
      for (const source of sources) {
        const group = groups.get(source)!,
          mesh = scene.meshes[scene.instances[source].mesh];
        if (
          group.length > 24 ||
          mesh.primitives.reduce((n, p) => n + p.indexCount / 3, 0) > 4096
        ) {
          if (this.castleSources.get(mode)?.has(source)) castleDistrict = true;
          else large++;
        }
      }
      if (castleDistrict) large++;
      if (large > SOURCE_DEBRIS_LIMITS.largeSources)
        throw Error(
          "Up to six large buildings can stay fractured at once. Choose Restore town to start again.",
        );
      const next = new Map(this.cells);
      const nextMasks = new Map(this.fineMasks.get(mode));
      for (const [key, cell] of next)
        if (key.startsWith(mode + ":") && !sources.has(cell.object.sourceId))
          next.delete(key);
      let vertices = 0,
        totalBytes = Array.from(next, ([key, cell]) =>
          key.startsWith(mode + ":")
            ? 0
            : 64 + bytes(cell.whole) + fragmentBytes(cell),
        ).reduce((n, value) => n + value, 0);
      const fragments: number[] = [],
        permanentFragments: number[] = [];
      const aliveNodes = new Set(
        [
          ...this.activeDebris.filter((d) => d.age < d.life),
          ...this.activeRubble,
        ].map((d) => d.nodeId),
      );
      const coarseNodes = new Set(
        [
          ...this.activeDebris.filter((d) => d.age < d.life),
          ...this.activeRubble,
        ]
          .filter((d) => d.subdivisions !== 4)
          .map((d) => d.nodeId),
      );
      let batchStarted = performance.now();
      let transientBytes = 0;
      for (const sourceId of sources) {
        const vertexCache = new Map<number, number[]>();
        for (const object of groups.get(sourceId)!) {
          if (cancelled()) return false;
          const key = mode + ":" + object.id;
          let cell = next.get(key);
          if (!cell)
            cell = {
              object,
              whole: (yield* fracturePlacementSteps(
                scene,
                this.source,
                sourceId,
                object.min,
                object.max,
                false,
                vertexCache,
              ))[0],
            };
          const needsFragments =
            wanted.has(object.id) &&
            (!current.has(object.id) || aliveNodes.has(object.id));
          if (needsFragments && !cell.fragments && coarseNodes.has(object.id))
            cell = {
              ...cell,
              fragments: yield* fracturePlacementSteps(
                scene,
                this.source,
                sourceId,
                object.min,
                object.max,
                true,
                vertexCache,
              ),
            };
          if (needsFragments && !cell.fineFragments)
            cell = {
              ...cell,
              fineFragments: yield* fracturePlacementSteps(
                scene,
                this.source,
                sourceId,
                object.min,
                object.max,
                4,
                vertexCache,
              ),
            };
          if (cell.fineFragments && !nextMasks.has(object.id)) {
            const mask: [number, number] = [0, 0];
            cell.fineFragments.forEach((part, index) => {
              if (part.vertices.length)
                mask[index >>> 5] =
                  (mask[index >>> 5] | (1 << (index & 31))) >>> 0;
            });
            nextMasks.set(object.id, mask);
          }
          if (!needsFragments && (cell.fragments || cell.fineFragments))
            cell = { ...cell, fragments: undefined, fineFragments: undefined };
          next.set(key, cell);
          if (!current.has(object.id))
            vertices += cell.whole.vertices.length / 12;
          totalBytes += 64 + bytes(cell.whole) + fragmentBytes(cell);
          for (const parts of [cell.fragments, cell.fineFragments])
            for (const part of parts ?? []) {
              if (part.vertices.length) {
                fragments.push(part.vertices.length / 12);
                if (parts === cell.fineFragments)
                  permanentFragments.push(part.vertices.length / 12);
              }
            }
          if (performance.now() - batchStarted >= 5) {
            yield;
            batchStarted = performance.now();
          }
        }
        const cached = vertexCache;
        const instance = scene.instances[sourceId];
        const mesh = scene.meshes[instance.mesh];
        const point = (index: number) => {
          const old = cached.get(index);
          if (old) return old;
          const i = index * 12;
          const value = transformPoint(instance.model, [
            this.source.vertices[i],
            this.source.vertices[i + 1],
            this.source.vertices[i + 2],
          ]);
          cached.set(index, value);
          return value;
        };
        let originalArea = 0,
          areaTriangles = 0;
        for (const primitive of mesh.primitives)
          for (
            let i = primitive.firstIndex;
            i < primitive.firstIndex + primitive.indexCount;
            i += 3
          ) {
            if (++areaTriangles % 128 === 0) yield;
            const a = point(this.source.indices[i] + primitive.baseVertex),
              b = point(this.source.indices[i + 1] + primitive.baseVertex),
              c = point(this.source.indices[i + 2] + primitive.baseVertex);
            const x = b[0] - a[0],
              y = b[1] - a[1],
              z = b[2] - a[2],
              u = c[0] - a[0],
              v = c[1] - a[1],
              w = c[2] - a[2];
            originalArea +=
              Math.hypot(y * w - z * v, z * u - x * w, x * v - y * u) * 0.5;
          }
        const covered = yield* fractureSurfaceAreaSteps(
          groups
            .get(sourceId)!
            .map((object) => next.get(mode + ":" + object.id)!.whole),
        );
        if (
          Math.abs(covered - originalArea) >
          Math.max(0.0001, originalArea * 0.001)
        )
          throw Error(
            "This building has an incomplete destruction map. Its walls remain unchanged.",
          );
        transientBytes = Math.max(transientBytes, cached.size * 224);
        if (performance.now() - batchStarted >= 5) {
          yield;
          batchStarted = performance.now();
        }
      }
      // Any permitted physics or settled subset must fit without dropping surfaces.
      const dynamic = fragments
        .sort((a, b) => b - a)
        .slice(0, GAMEPLAY_VISUAL_LIMITS.debris)
        .reduce((n, v) => n + v, 0);
      const permanent = permanentFragments
        .sort((a, b) => b - a)
        .slice(0, GAMEPLAY_VISUAL_LIMITS.rubble)
        .reduce((n, v) => n + v, 0);
      // The working cache stays alive until this candidate is accepted or discarded.
      let retiredBytes = this.cells.size * 64;
      for (const [key, old] of this.cells) {
        const replacement = next.get(key);
        if (replacement?.whole !== old.whole) retiredBytes += bytes(old.whole);
        for (const field of ["fragments", "fineFragments"] as const) {
          const parts = old[field];
          if (parts && parts !== replacement?.[field])
            retiredBytes +=
              24 +
              parts.length * 8 +
              parts.reduce((n, part) => n + bytes(part), 0);
        }
      }
      transientBytes += retiredBytes;
      totalBytes +=
        (nextMasks.size + (this.fineMasks.get(mode)?.size ?? 0)) * 128;
      totalBytes += transientBytes;
      this.preparation = {
        standingVertices: vertices,
        debrisVertices: dynamic,
        rubbleVertices: permanent,
        cacheBytes: totalBytes,
        transientBytes,
        sources: sources.size,
      };
      if (
        vertices > SOURCE_DEBRIS_LIMITS.standingVertices ||
        dynamic > SOURCE_DEBRIS_LIMITS.debrisVertices ||
        permanent > SOURCE_DEBRIS_LIMITS.rubbleVertices ||
        totalBytes > SOURCE_DEBRIS_LIMITS.cacheBytes
      )
        throw Error(
          "This spell exceeds the town fragment detail budget. Choose Restore town to start again.",
        );
      if (cancelled()) return false;
      if (
        this.packing &&
        !(yield* this.packing.prepare(this.packingChunks(next), cancelled))
      )
        return false;
      if (cancelled()) return false;
      this.cells = next;
      this.fineMasks.set(mode, nextMasks);
      this.prepared.set(mode, wanted);
      this.limitReason = "";
      return true;
    } catch (error) {
      this.limitReason = error instanceof Error ? error.message : String(error);
      return false;
    }
  }
  private packingChunks(cells = this.cells) {
    const chunks: { chunk: FractureChunk; world: boolean }[] = [];
    for (const cell of cells.values()) {
      chunks.push({ chunk: cell.whole, world: true });
      for (const fragments of [cell.fragments, cell.fineFragments])
        for (const chunk of fragments ?? [])
          chunks.push({ chunk, world: false });
    }
    return chunks;
  }
  encodePacking(encoder: GPUCommandEncoder) {
    this.packing?.encode(encoder);
  }
  packingSubmitted() {
    this.packing?.submitted();
  }
  private packedBounds(bounds?: PackingBounds): InstanceBounds | undefined {
    if (!bounds) return undefined;
    const { min, max } = bounds;
    return {
      count: 1,
      aabbs: new Float64Array([...min, ...max]),
      spheres: new Float64Array([
        ...min.map((value, axis) => (value + max[axis]) / 2),
        Math.hypot(...min.map((value, axis) => max[axis] - value)) / 2,
      ]),
    };
  }
  private upload(
    parts: {
      chunk: FractureChunk;
      instance: number;
      world: boolean;
      transform?: Float32Array;
    }[],
    target: Buffers,
    budget: number,
  ): Draw[] {
    const packed = this.packing?.stage(parts, target, budget);
    if (packed) {
      if (target === this.standing)
        this.standingBounds = this.packedBounds(packed.bounds);
      if (target === this.rubble)
        this.rubbleBounds = this.packedBounds(packed.bounds);
      return packed.draws;
    }
    const grouped = new Map<
      number,
      {
        part: FractureChunk;
        draw: FractureChunk["draws"][number];
        instance: number;
        world: boolean;
        transform?: Float32Array;
      }[]
    >();
    let count = 0;
    for (const item of parts)
      for (const draw of item.chunk.draws) {
        count += draw.indexCount;
        const list = grouped.get(draw.material) ?? [];
        list.push({
          part: item.chunk,
          draw,
          instance: item.instance,
          world: item.world,
          transform: item.transform,
        });
        grouped.set(draw.material, list);
      }
    if (count > budget)
      throw Error("Prepared fragments exceeded their reserved GPU budget.");
    const vertices = new Float32Array(count * 12),
      colors = new Uint8Array(count * 4),
      uv1 = new Float32Array(count * 2),
      draws: Draw[] = [];
    let offset = 0;
    const min: V3 = [Infinity, Infinity, Infinity],
      max: V3 = [-Infinity, -Infinity, -Infinity];
    for (const [material, items] of grouped)
      for (const item of items) {
        const { part, draw, world, transform } = item;
        const start = offset;
        const first = draw.firstIndex,
          end = first + draw.indexCount;
        vertices.set(part.vertices.subarray(first * 12, end * 12), offset * 12);
        colors.set(part.colors.subarray(first * 4, end * 4), offset * 4);
        uv1.set(part.uv1.subarray(first * 2, end * 2), offset * 2);
        for (let i = 0; i < draw.indexCount; i++) {
          if (world)
            for (let axis = 0; axis < 3; axis++)
              vertices[offset * 12 + axis] += part.center[axis];
          if (transform) {
            const start = offset * 12;
            const x = vertices[start],
              y = vertices[start + 1],
              z = vertices[start + 2];
            for (let axis = 0; axis < 3; axis++)
              vertices[start + axis] =
                transform[axis] * x +
                transform[axis + 4] * y +
                transform[axis + 8] * z +
                transform[axis + 12];
            for (let attribute = 3; attribute <= 8; attribute += 5) {
              const x = vertices[start + attribute],
                y = vertices[start + attribute + 1],
                z = vertices[start + attribute + 2];
              for (let axis = 0; axis < 3; axis++)
                vertices[start + attribute + axis] =
                  transform[axis] * x +
                  transform[axis + 4] * y +
                  transform[axis + 8] * z;
            }
          }
          for (let axis = 0; axis < 3; axis++) {
            min[axis] = Math.min(min[axis], vertices[offset * 12 + axis]);
            max[axis] = Math.max(max[axis], vertices[offset * 12 + axis]);
          }
          offset++;
        }
        const previous = draws.at(-1);
        if (
          previous &&
          previous.material === material &&
          previous.instance === item.instance
        )
          previous.indexCount += offset - start;
        else
          draws.push({
            material,
            firstIndex: start,
            indexCount: offset - start,
            instance: item.instance,
          });
      }
    if (count) {
      this.device.queue.writeBuffer(target.vertices, 0, vertices);
      this.device.queue.writeBuffer(target.colors, 0, colors);
      this.device.queue.writeBuffer(target.uv1, 0, uv1);
    }
    const bounds = count
      ? {
          count: 1,
          aabbs: new Float64Array([...min, ...max]),
          spheres: new Float64Array([
            ...min.map((v, axis) => (v + max[axis]) / 2),
            Math.hypot(...min.map((v, axis) => max[axis] - v)) / 2,
          ]),
        }
      : undefined;
    if (target === this.standing) this.standingBounds = bounds;
    if (target === this.rubble) this.rubbleBounds = bounds;
    return draws;
  }
  update(
    mode: string,
    destroyed: readonly number[],
    debris: readonly GameplayDebris[],
    rubble: readonly GameplayDebris[] = [],
  ): Set<number> {
    if (
      debris.length > GAMEPLAY_VISUAL_LIMITS.debris ||
      rubble.length > GAMEPLAY_VISUAL_LIMITS.rubble
    )
      throw Error("Destruction exceeds the visual scene budget.");
    const nodes = this.objects.get(mode),
      planned = this.prepared.get(mode) ?? new Set<number>();
    if (!nodes) {
      if (destroyed.length || debris.length || rubble.length)
        throw Error("The scene has no destructible cells.");
      return new Set();
    }
    if (destroyed.some((id) => !planned.has(id)))
      throw Error("Prepare the destruction visuals before changing collision.");
    const dead = new Set(destroyed);
    for (const body of [...debris, ...rubble]) {
      const object = nodes.get(body.nodeId);
      if (
        !object ||
        object.sourceId !== body.sourceId ||
        !Number.isInteger(body.chunk) ||
        (body.subdivisions !== undefined &&
          body.subdivisions !== 2 &&
          body.subdivisions !== 4) ||
        body.chunk < 0 ||
        body.chunk >= (body.subdivisions === 4 ? 64 : 8) ||
        !dead.has(body.nodeId) ||
        object.min.some(
          (value, axis) => Math.abs(value - body.sourceMin[axis]) > 0.001,
        ) ||
        object.max.some(
          (value, axis) => Math.abs(value - body.sourceMax[axis]) > 0.001,
        ) ||
        body.halfExtents.some(
          (value, axis) =>
            Math.abs(
              value -
                (object.max[axis] - object.min[axis]) /
                  (body.subdivisions === 4 ? 8 : 4),
            ) > 0.001,
        )
      )
        throw Error("Debris must match its destroyed source cell.");
    }
    const sources = new Set(destroyed.map((id) => nodes.get(id)!.sourceId));
    const key = mode + ":" + destroyed.join(",");
    if (key !== this.standingKey) {
      const parts: {
        chunk: FractureChunk;
        instance: number;
        world: boolean;
      }[] = [];
      let cells = 0;
      for (const cell of this.cells.values())
        if (
          sources.has(cell.object.sourceId) &&
          nodes.get(cell.object.id) === cell.object &&
          !dead.has(cell.object.id)
        ) {
          parts.push({ chunk: cell.whole, instance: 0, world: true });
          cells++;
        }
      this.standingDraws = this.upload(
        parts,
        this.standing,
        SOURCE_DEBRIS_LIMITS.standingVertices,
      );
      this.standingKey = key;
      this.staticRevision++;
      this.stats.standingCells = cells;
      this.stats.standingTriangles = this.standingDraws.reduce(
        (n, d) => n + d.indexCount / 3,
        0,
      );
    }
    const visible = debris.filter((d) => d.age < d.life);
    const movingKey =
      mode +
      ":" +
      visible
        .map(
          (d) =>
            d.id + "/" + d.nodeId + "/" + (d.subdivisions ?? 2) + "/" + d.chunk,
        )
        .join(",");
    if (movingKey !== this.movingKey) {
      const parts: {
        chunk: FractureChunk;
        instance: number;
        world: boolean;
      }[] = [];
      this.movingSlots = [];
      for (const d of visible) {
        const cell = this.cells.get(mode + ":" + d.nodeId),
          chunk = (
            d.subdivisions === 4 ? cell?.fineFragments : cell?.fragments
          )?.[d.chunk];
        if (!chunk)
          throw Error("A debris body has no prepared source fragment.");
        if (chunk.vertices.length) {
          const instance = this.movingSlots.length;
          this.movingSlots.push(d);
          parts.push({ chunk, instance, world: false });
        }
      }
      this.movingDraws = this.upload(
        parts,
        this.moving,
        SOURCE_DEBRIS_LIMITS.debrisVertices,
      );
      this.movingKey = movingKey;
      this.stats.debrisTriangles = this.movingDraws.reduce(
        (n, d) => n + d.indexCount / 3,
        0,
      );
    } else {
      const byId = new Map(visible.map((d) => [d.id, d]));
      this.movingSlots = this.movingSlots.map((d) => byId.get(d.id)!);
    }
    this.movingSlots.forEach((d, index) => {
      const fade = Math.min(1, Math.max(0.02, (d.life - d.age) / 2));
      this.modelData.set(eulerMatrix(d.position, d.rotation, fade), index * 16);
    });
    if (this.movingSlots.length)
      this.device.queue.writeBuffer(
        this.moving.models,
        0,
        this.modelData.subarray(0, this.movingSlots.length * 16),
      );
    const rubbleKey =
      mode +
      ":" +
      rubble
        .map((d) =>
          [
            d.id,
            d.nodeId,
            d.subdivisions ?? 2,
            d.chunk,
            ...d.position,
            ...d.rotation,
          ].join("/"),
        )
        .join(",");
    if (rubbleKey !== this.rubbleKey) {
      const parts: {
        chunk: FractureChunk;
        instance: number;
        world: boolean;
        transform?: Float32Array;
      }[] = [];
      let count = 0;
      for (const d of rubble) {
        const cell = this.cells.get(mode + ":" + d.nodeId);
        const chunk = (
          d.subdivisions === 4 ? cell?.fineFragments : cell?.fragments
        )?.[d.chunk];
        if (!chunk) throw Error("Rubble needs its prepared source fragment.");
        if (!chunk.vertices.length) continue;
        parts.push({
          chunk,
          instance: 0,
          world: false,
          transform: eulerMatrix(d.position, d.rotation, 1),
        });
        count++;
      }
      this.rubbleDraws = this.upload(
        parts,
        this.rubble,
        SOURCE_DEBRIS_LIMITS.rubbleVertices,
      );
      this.rubbleKey = rubbleKey;
      this.staticRevision++;
      this.stats.rubbleCount = count;
      this.stats.rubbleTriangles = this.rubbleDraws.reduce(
        (n, draw) => n + draw.indexCount / 3,
        0,
      );
    }
    this.mode = mode;
    this.destroyed.set(mode, dead);
    this.activeDebris = debris;
    this.activeRubble = rubble;
    this.stats.activeFracturedSources = sources.size;
    this.stats.maskedNodes = dead.size;
    this.stats.renderedDebris = this.movingSlots.length;
    return sources;
  }
  get statsSnapshot() {
    return {
      ...this.stats,
      geometryBytes: this.bytes,
      packing: this.packing?.snapshot() ?? { backend: "cpu" },
      limitReason: this.limitReason,
    };
  }
  castleFragmentMasks(mode: string): [number, number][] {
    const nodes = this.castleNodes.get(mode) ?? [];
    if (nodes.some((id) => !this.prepared.get(mode)?.has(id)))
      throw Error("Prepare all castle source fragments first.");
    return this.fragmentMasks(mode, nodes).map((entry) => entry.mask);
  }
  fragmentMasks(
    mode: string,
    ids: readonly number[],
  ): { nodeId: number; mask: [number, number] }[] {
    if (
      ids.length > GAMEPLAY_VISUAL_LIMITS.nodes ||
      new Set(ids).size !== ids.length ||
      ids.some(
        (id) =>
          !this.prepared.get(mode)?.has(id) ||
          !this.fineMasks.get(mode)?.has(id),
      )
    )
      throw Error("Prepare the source fragments before casting this spell.");
    return ids.map((nodeId) => ({
      nodeId,
      mask: [...this.fineMasks.get(mode)!.get(nodeId)!] as [number, number],
    }));
  }
  get hasMoving() {
    return this.movingDraws.length > 0;
  }
  get movingSourceIds() {
    return new Set(this.movingSlots.map((d) => d.sourceId));
  }
  private movingVisibility(matrix: Float32Array) {
    const spheres = new Float64Array(this.movingSlots.length * 4),
      aabbs = new Float64Array(this.movingSlots.length * 6);
    this.movingSlots.forEach((d, i) => {
      const radius = Math.hypot(...d.halfExtents);
      spheres.set([...d.position, radius], i * 4);
      aabbs.set(
        [
          ...d.position.map((v) => v - radius),
          ...d.position.map((v) => v + radius),
        ],
        i * 6,
      );
    });
    return frustumVisibility(
      { count: this.movingSlots.length, spheres, aabbs },
      matrix,
    );
  }
  visibleMovingSources(matrix: Float32Array) {
    const visibility = this.movingVisibility(matrix);
    return new Set(
      this.movingSlots.filter((_, i) => visibility[i]).map((d) => d.sourceId),
    );
  }
  private draw(
    pass: GPURenderPassEncoder,
    target: Buffers,
    draws: Draw[],
    materials: readonly GPUBindGroup[],
    zeroWind: GPUBuffer,
    include: (material: number) => boolean,
    visible?: Uint8Array,
    pipelineForMaterial?: (material: number) => GPURenderPipeline,
  ) {
    if (!draws.length) return;
    pass.setVertexBuffer(0, target.vertices);
    pass.setVertexBuffer(1, target.models);
    pass.setVertexBuffer(2, target.colors);
    pass.setVertexBuffer(3, target.uv1);
    pass.setVertexBuffer(4, zeroWind);
    pass.setIndexBuffer(target.indices, "uint32");
    let previous = -1;
    for (const draw of draws) {
      if ((visible && !visible[draw.instance]) || !include(draw.material))
        continue;
      if (previous !== draw.material) {
        if (pipelineForMaterial)
          pass.setPipeline(pipelineForMaterial(draw.material));
        pass.setBindGroup(1, materials[draw.material]);
        previous = draw.material;
      }
      pass.drawIndexed(draw.indexCount, 1, draw.firstIndex, 0, draw.instance);
    }
  }
  drawStanding(
    pass: GPURenderPassEncoder,
    materials: readonly GPUBindGroup[],
    zeroWind: GPUBuffer,
    include: (material: number) => boolean,
    matrix?: Float32Array,
    pipelineForMaterial?: (material: number) => GPURenderPipeline,
  ) {
    if (
      !matrix ||
      !this.standingBounds ||
      frustumVisibility(this.standingBounds, matrix)[0]
    )
      this.draw(
        pass,
        this.standing,
        this.standingDraws,
        materials,
        zeroWind,
        include,
        undefined,
        pipelineForMaterial,
      );
    if (
      !matrix ||
      !this.rubbleBounds ||
      frustumVisibility(this.rubbleBounds, matrix)[0]
    )
      this.draw(
        pass,
        this.rubble,
        this.rubbleDraws,
        materials,
        zeroWind,
        include,
        undefined,
        pipelineForMaterial,
      );
  }
  drawMoving(
    pass: GPURenderPassEncoder,
    materials: readonly GPUBindGroup[],
    zeroWind: GPUBuffer,
    include: (material: number) => boolean,
    matrix?: Float32Array,
    pipelineForMaterial?: (material: number) => GPURenderPipeline,
  ) {
    this.draw(
      pass,
      this.moving,
      this.movingDraws,
      materials,
      zeroWind,
      include,
      matrix ? this.movingVisibility(matrix) : undefined,
      pipelineForMaterial,
    );
  }
  dispose() {
    this.prepareGeneration++;
    for (const buffers of [this.standing, this.moving, this.rubble])
      for (const buffer of Object.values(buffers)) buffer.destroy();
    this.cells.clear();
    this.fineMasks.clear();
    this.packing?.dispose();
  }
}
