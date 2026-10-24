import type { InstanceBounds } from "./culling.ts";
import type {
  RendererFrameResult,
  RendererFrameSource,
  RendererFramePreparation,
  RendererResidentFrame,
} from "./feature-contract.ts";
import {
  buildClusteredLights,
  type ClusteredLightInput,
  type ClusteredLights,
} from "./clustered-lights.ts";

export interface PackedSceneHandle {
  owner: object;
  bounds: InstanceBounds;
  models: Float32Array;
  wind?: Float32Array;
  ranges: Uint32Array;
  visibility: Uint8Array;
  rank: Uint32Array;
  outModels: Float32Array;
  outWind: Float32Array;
  sourceIndices: Uint32Array;
  outRanges: Uint32Array;
}

/** Immutable validated source buffers and reusable stable compaction storage. */
export class PackedFrameCore {
  private readonly owner = {};
  private planes = new Float64Array(24);
  private readonly scenes = new Set<PackedSceneHandle>();
  private closed = false;

  get memoryBytes(): number {
    let bytes = this.planes.byteLength;
    for (const scene of this.scenes)
      bytes +=
        scene.bounds.spheres.byteLength +
        scene.bounds.aabbs.byteLength +
        scene.models.byteLength +
        (scene.wind?.byteLength ?? 0) +
        scene.ranges.byteLength +
        scene.visibility.byteLength +
        scene.rank.byteLength +
        scene.outModels.byteLength +
        scene.outWind.byteLength +
        scene.sourceIndices.byteLength +
        scene.outRanges.byteLength;
    return bytes;
  }

  private assertOpen(): void {
    if (this.closed)
      throw new Error("The resident source buffers have closed.");
  }

  prepareScene(
    bounds: InstanceBounds,
    models: Float32Array,
    draws: readonly { firstInstance: number; count: number }[],
    wind?: Float32Array,
  ): PackedSceneHandle {
    this.assertOpen();
    const count = bounds.count;
    if (!Number.isSafeInteger(count) || count < 0 || count > 1_000_000)
      throw new Error("Use a bounded resident source instance count.");
    const scene: PackedSceneHandle = {
      owner: this.owner,
      bounds: {
        count,
        spheres: new Float64Array(count * 4),
        aabbs: new Float64Array(count * 6),
      },
      models: new Float32Array(count * 16),
      wind: wind ? new Float32Array(count * 4) : undefined,
      ranges: new Uint32Array(draws.length * 2),
      visibility: new Uint8Array(count),
      rank: new Uint32Array(count + 1),
      outModels: new Float32Array(count * 16),
      outWind: new Float32Array(wind ? count * 4 : 0),
      sourceIndices: new Uint32Array(count),
      outRanges: new Uint32Array(draws.length * 2),
    };
    this.scenes.add(scene);
    try {
      this.updateScene(scene, bounds, models, draws, wind);
    } catch (error) {
      this.scenes.delete(scene);
      throw error;
    }
    return scene;
  }

  updateScene(
    scene: PackedSceneHandle,
    bounds: InstanceBounds,
    models: Float32Array,
    draws: readonly { firstInstance: number; count: number }[],
    wind?: Float32Array,
  ): void {
    this.assertOpen();
    const count = scene.bounds.count;
    if (scene.owner !== this.owner || !this.scenes.has(scene))
      throw new Error("Use a scene prepared by this packed frame core.");
    if (
      !Number.isSafeInteger(count) ||
      count < 0 ||
      count > 1_000_000 ||
      bounds.count !== count ||
      models.length !== count * 16 ||
      bounds.spheres.length !== count * 4 ||
      bounds.aabbs.length !== count * 6 ||
      draws.length * 2 !== scene.ranges.length ||
      !!wind !== !!scene.wind ||
      (wind && wind.length !== count * 4)
    )
      throw new Error("Use complete resident instance, draw, and wind data.");
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
    for (const draw of draws)
      if (
        !Number.isSafeInteger(draw.firstInstance) ||
        draw.firstInstance < 0 ||
        !Number.isSafeInteger(draw.count) ||
        draw.count < 0 ||
        draw.firstInstance + draw.count > count
      )
        throw new Error(
          "Draw ranges must stay inside the source instance matrices.",
        );
    scene.models.set(models);
    scene.bounds.spheres.set(bounds.spheres);
    scene.bounds.aabbs.set(bounds.aabbs);
    if (wind) scene.wind!.set(wind);
    draws.forEach((draw, i) =>
      scene.ranges.set([draw.firstInstance, draw.count], i * 2),
    );
  }

  cull(
    scene: PackedSceneHandle,
    matrix: ArrayLike<number>,
    excluded?: ReadonlySet<number>,
  ): RendererFrameResult {
    this.assertOpen();
    if (scene.owner !== this.owner || !this.scenes.has(scene))
      throw new Error("Use a scene prepared by this packed frame core.");
    if (matrix.length !== 16)
      throw new Error("Frustum culling needs a finite view-projection matrix.");
    for (let i = 0; i < 16; i++)
      if (!Number.isFinite(matrix[i]))
        throw new Error(
          "Frustum culling needs a finite view-projection matrix.",
        );
    if (
      matrix[3] === 0 &&
      matrix[7] === 0 &&
      matrix[11] === 0 &&
      matrix[15] === 0
    )
      throw new Error("Frustum culling needs a finite view-projection matrix.");
    const planes = this.planes;
    for (let plane = 0; plane < 6; plane++) {
      const start = plane * 4;
      for (let column = 0; column < 4; column++) {
        const offset = column * 4;
        planes[start + column] =
          plane === 4
            ? matrix[offset + 2]
            : matrix[offset + 3] +
              (plane % 2 ? -1 : 1) *
                matrix[offset + (plane < 4 ? Math.floor(plane / 2) : 2)];
      }
      const length = Math.hypot(
        planes[start],
        planes[start + 1],
        planes[start + 2],
      );
      if (length > 1e-15)
        for (let i = 0; i < 4; i++) planes[start + i] /= length;
      else {
        const d = planes[start + 3];
        planes[start] = planes[start + 1] = planes[start + 2] = 0;
        planes[start + 3] = d < 0 ? -1 : 1;
      }
    }
    const { bounds, visibility, rank } = scene,
      { count, spheres, aabbs } = bounds;
    rank[0] = 0;
    for (let i = 0; i < count; i++) {
      const s = i * 4,
        a = i * 6;
      const x = spheres[s],
        y = spheres[s + 1],
        z = spheres[s + 2],
        r = spheres[s + 3];
      const epsilon =
        1e-5 * Math.max(1, r) +
        1e-7 * (Math.abs(x) + Math.abs(y) + Math.abs(z));
      visibility[i] = excluded?.has(i) ? 0 : 1;
      for (let p = 0; visibility[i] && p < 24; p += 4) {
        const nx = planes[p],
          ny = planes[p + 1],
          nz = planes[p + 2],
          d = planes[p + 3];
        if (
          nx * x + ny * y + nz * z + d < -r - epsilon ||
          nx * aabbs[a + (nx >= 0 ? 3 : 0)] +
            ny * aabbs[a + (ny >= 0 ? 4 : 1)] +
            nz * aabbs[a + (nz >= 0 ? 5 : 2)] +
            d <
            -epsilon
        ) {
          visibility[i] = 0;
          break;
        }
      }
      rank[i + 1] = rank[i] + visibility[i];
      if (!visibility[i]) continue;
      const destination = rank[i];
      for (let c = 0; c < 16; c++)
        scene.outModels[destination * 16 + c] = scene.models[i * 16 + c];
      if (scene.wind)
        for (let c = 0; c < 4; c++)
          scene.outWind[destination * 4 + c] = scene.wind[i * 4 + c];
      scene.sourceIndices[destination] = i;
    }
    for (let draw = 0; draw < scene.ranges.length / 2; draw++) {
      const first = scene.ranges[draw * 2],
        end = first + scene.ranges[draw * 2 + 1];
      scene.outRanges[draw * 2] = rank[first];
      scene.outRanges[draw * 2 + 1] = rank[end] - rank[first];
    }
    const visible = rank[count];
    return {
      visibility,
      models: scene.outModels.subarray(0, visible * 16),
      wind: scene.outWind.subarray(0, scene.wind ? visible * 4 : 0),
      sourceIndices: scene.sourceIndices.subarray(0, visible),
      drawRanges: scene.outRanges,
    };
  }

  /** Filter destroyed source placements before stable rank and compaction. */
  cullMasked(
    scene: PackedSceneHandle,
    matrix: ArrayLike<number>,
    excluded: ReadonlySet<number>,
  ): RendererFrameResult {
    return this.cull(scene, matrix, excluded);
  }

  /** Source culling does not replace the established light-list algorithm. */
  buildClusters(input: ClusteredLightInput): ClusteredLights {
    this.assertOpen();
    return buildClusteredLights(input);
  }

  /** Returned cull views are borrowed until the next cull, update, or release. */
  releaseScene(scene: PackedSceneHandle): void {
    if (!this.scenes.delete(scene)) return;
    scene.owner = {};
    scene.bounds = {
      count: 0,
      spheres: new Float64Array(),
      aabbs: new Float64Array(),
    };
    scene.models = scene.outModels = new Float32Array();
    scene.wind = undefined;
    scene.outWind = new Float32Array();
    scene.ranges =
      scene.rank =
      scene.sourceIndices =
      scene.outRanges =
        new Uint32Array();
    scene.visibility = new Uint8Array();
  }

  dispose(): void {
    if (this.closed) return;
    for (const scene of this.scenes) this.releaseScene(scene);
    this.planes = new Float64Array();
    this.closed = true;
  }
}

type FrameSourceData = RendererFrameSource;

interface FramePreparationCore<Handle> {
  prepareScene(
    bounds: InstanceBounds,
    models: Float32Array,
    draws: FrameSourceData["draws"],
    wind?: Float32Array,
  ): Handle;
  updateScene(
    handle: Handle,
    bounds: InstanceBounds,
    models: Float32Array,
    draws: FrameSourceData["draws"],
    wind?: Float32Array,
  ): void;
  cull(handle: Handle, matrix: ArrayLike<number>): RendererFrameResult;
  cullMasked?(
    handle: Handle,
    matrix: ArrayLike<number>,
    excluded: ReadonlySet<number>,
  ): RendererFrameResult;
  releaseScene?(handle: Handle): void;
}

export interface ResidentFrameScene extends RendererResidentFrame {
  cullMasked?(
    matrix: Float32Array,
    excluded: ReadonlySet<number>,
  ): RendererFrameResult;
  dispose(): void;
}

/** Hide each backend's handle type behind a closure that keeps its owning core. */
export function bindResidentFrameScene<Handle>(
  core: FramePreparationCore<Handle>,
  source: () => FrameSourceData,
  version: number,
): ResidentFrameScene {
  const initial = source();
  const handle = core.prepareScene(
    initial.bounds,
    initial.models,
    initial.draws,
    initial.wind,
  );
  let closed = false;
  const assertOpen = () => {
    if (closed) throw new Error("The resident source scene has closed.");
  };
  return {
    version,
    update() {
      assertOpen();
      const current = source();
      core.updateScene(
        handle,
        current.bounds,
        current.models,
        current.draws,
        current.wind,
      );
    },
    cull(matrix) {
      assertOpen();
      return core.cull(handle, matrix);
    },
    ...(core.cullMasked
      ? {
          cullMasked(matrix: Float32Array, excluded: ReadonlySet<number>) {
            assertOpen();
            return core.cullMasked!(handle, matrix, excluded);
          },
        }
      : {}),
    dispose() {
      if (closed) return;
      closed = true;
      core.releaseScene?.(handle);
    },
  };
}

export interface ResidentSourcePreparation extends RendererFramePreparation {
  prepare(
    source: () => RendererFrameSource,
    version: number,
  ): ResidentFrameScene;
  dispose(): void;
}

/** Production source culling uses one implementation shared with the controls. */
export function createResidentSourcePreparation(): ResidentSourcePreparation {
  const core = new PackedFrameCore();
  return {
    implementation: "pooled-typescript",
    get memoryBytes() {
      return core.memoryBytes;
    },
    prepare: (source, version) => bindResidentFrameScene(core, source, version),
    buildClusters: (input) => core.buildClusters(input),
    dispose: () => core.dispose(),
  };
}
