import type { InstanceBounds } from "./culling.ts";

export const WIND_SHADOW_LIMITS = Object.freeze({
  views: 43,
  staticBytes: 136 * 1024 * 1024,
  dimension: 4096,
  casters: 4096,
  sourceInstances: 65536,
});
export interface WindShadowView {
  id: string;
  width: number;
  height: number;
  projection: ArrayLike<number>;
  /** Include mode, static geometry/alpha/shader changes, and the primitive partition. */
  staticRevision: string;
  /** Include the exact world/capture time and all live wind controls. */
  animationRevision: string;
  /** Source instance IDs. The caller must still partition primitive uses within each instance. */
  dynamicCasters: Uint32Array;
}
export interface WindShadowWork {
  id: string;
  renderStatic: false | "main" | "cache";
  restoreStatic: boolean;
  dynamicCasterIndices: Uint32Array;
}
export interface WindShadowPlan {
  readonly views: readonly WindShadowWork[];
  readonly allocations: readonly {
    id: string;
    width: number;
    height: number;
    bytes: number;
    replace: boolean;
  }[];
  readonly releases: readonly string[];
  /** Logical retained copies; the host must bound retirement of released GPU resources too. */
  readonly staticCacheBytes: number;
}
interface CachedView {
  width: number;
  height: number;
  cacheRevision?: string;
  mainRevision: string;
  dynamicSignature: string;
  animationRevision: string;
  allocated: boolean;
}
/** Plans only. GPU allocation/encoding and successful submission precede commit. */
export class WindShadowPlanner {
  private views = new Map<string, CachedView>();
  private generation = 0;
  private pending = new WeakMap<
    WindShadowPlan,
    { generation: number; views: Map<string, CachedView> }
  >();
  plan(inputs: readonly WindShadowView[]): WindShadowPlan {
    if (inputs.length > WIND_SHADOW_LIMITS.views)
      throw Error("Wind shadows support up to 43 views.");
    const next = new Map<string, CachedView>(),
      work: WindShadowWork[] = [],
      allocations: Array<{
        id: string;
        width: number;
        height: number;
        bytes: number;
        replace: boolean;
      }> = [],
      releases: string[] = [];
    let staticCacheBytes = 0;
    for (const view of inputs) {
      if (
        typeof view.id !== "string" ||
        !view.id ||
        view.id.length > 128 ||
        next.has(view.id) ||
        !Number.isInteger(view.width) ||
        !Number.isInteger(view.height) ||
        view.width < 1 ||
        view.height < 1 ||
        view.width > WIND_SHADOW_LIMITS.dimension ||
        view.height > WIND_SHADOW_LIMITS.dimension ||
        view.projection.length !== 16 ||
        !Array.from(view.projection).every(Number.isFinite) ||
        typeof view.staticRevision !== "string" ||
        !view.staticRevision ||
        view.staticRevision.length > 4096 ||
        typeof view.animationRevision !== "string" ||
        view.animationRevision.length > 4096 ||
        !(view.dynamicCasters instanceof Uint32Array) ||
        view.dynamicCasters.length > WIND_SHADOW_LIMITS.casters
      )
        throw Error("Use bounded, unique shadow views and complete revisions.");
      let previousIndex = -1;
      for (const index of view.dynamicCasters) {
        if (
          index >= WIND_SHADOW_LIMITS.sourceInstances ||
          index <= previousIndex
        )
          throw Error(
            "Moving shadow casters need sorted unique source indices.",
          );
        previousIndex = index;
      }
      const previous = this.views.get(view.id),
        sameSize =
          previous?.width === view.width && previous.height === view.height;
      const state: CachedView = previous
        ? { ...previous }
        : {
            width: view.width,
            height: view.height,
            mainRevision: "",
            dynamicSignature: "",
            animationRevision: "",
            allocated: false,
          };
      const wasAllocated = state.allocated;
      if (!sameSize) {
        if (wasAllocated) releases.push(view.id);
        state.allocated = false;
        state.cacheRevision = undefined;
        state.mainRevision = "";
        state.width = view.width;
        state.height = view.height;
      }
      // JSON tuples avoid ambiguous concatenation of caller revision strings.
      const revision = JSON.stringify([
          view.staticRevision,
          Array.from(view.projection),
        ]),
        signature = Array.from(view.dynamicCasters).join(",");
      let renderStatic: WindShadowWork["renderStatic"] = false,
        restoreStatic = false,
        drawDynamic = false;
      if (view.dynamicCasters.length) {
        if (!state.allocated) {
          allocations.push({
            id: view.id,
            width: view.width,
            height: view.height,
            bytes: view.width * view.height * 4,
            replace: wasAllocated,
          });
          state.allocated = true;
        }
        if (state.cacheRevision !== revision) {
          renderStatic = "cache";
          state.cacheRevision = revision;
        }
        if (
          renderStatic ||
          state.mainRevision !== revision ||
          state.dynamicSignature !== signature ||
          state.animationRevision !== view.animationRevision
        ) {
          restoreStatic = true;
          drawDynamic = true;
        }
      } else if (
        state.mainRevision !== revision ||
        state.dynamicSignature !== ""
      ) {
        // Remove old moving shadows once; a valid static copy avoids a town redraw.
        if (state.allocated && state.cacheRevision === revision)
          restoreStatic = true;
        else renderStatic = "main";
      }
      state.mainRevision = revision;
      state.dynamicSignature = signature;
      state.animationRevision = view.animationRevision;
      if (state.allocated) staticCacheBytes += view.width * view.height * 4;
      next.set(view.id, state);
      work.push({
        id: view.id,
        renderStatic,
        restoreStatic,
        dynamicCasterIndices: drawDynamic
          ? view.dynamicCasters.slice()
          : new Uint32Array(),
      });
    }
    for (const [id, state] of this.views)
      if (!next.has(id) && state.allocated) releases.push(id);
    if (staticCacheBytes > WIND_SHADOW_LIMITS.staticBytes)
      throw Error("Static wind shadow copies exceed 136 MiB.");
    const result: WindShadowPlan = {
      views: work,
      allocations,
      releases,
      staticCacheBytes,
    };
    this.pending.set(result, { generation: this.generation, views: next });
    return result;
  }
  /** Call only after all planned resources and commands have been submitted successfully. */
  commit(plan: WindShadowPlan) {
    const next = this.pending.get(plan);
    if (!next || next.generation !== this.generation)
      throw Error("Plan wind shadows again after the latest submission.");
    this.views = next.views;
    this.generation++;
    this.pending.delete(plan);
  }
  /** Device loss or a host cache reset invalidates every pending plan and saved copy. */
  reset() {
    this.views.clear();
    this.generation++;
    this.pending = new WeakMap();
  }
}

/** Same source-order gather as matrix compaction: pivot/radius + source up/reserved, 8 floats. */
export function compactWindFrames(
  source: Float32Array,
  visibility: Uint8Array,
): { frames: Float32Array; sourceIndices: Uint32Array } {
  if (
    !(source instanceof Float32Array) ||
    !(visibility instanceof Uint8Array) ||
    source.length !== visibility.length * 8 ||
    visibility.length > WIND_SHADOW_LIMITS.sourceInstances ||
    !source.every(Number.isFinite)
  )
    throw Error("Wind frames need eight finite floats per source instance.");
  let count = 0;
  for (const value of visibility) {
    if (value !== 0 && value !== 1)
      throw Error("Use zero or one for wind instance visibility.");
    count += value;
  }
  const frames = new Float32Array(count * 8),
    sourceIndices = new Uint32Array(count);
  let rank = 0;
  for (let index = 0; index < visibility.length; index++)
    if (visibility[index]) {
      frames.set(source.subarray(index * 8, index * 8 + 8), rank * 8);
      sourceIndices[rank++] = index;
    }
  return { frames, sourceIndices };
}
/** Isotropic displacement bound expands culling only; source pivots and radii stay untouched. */
export function expandWindBounds(
  bounds: InstanceBounds,
  padding: Float64Array,
): InstanceBounds {
  if (
    !Number.isInteger(bounds.count) ||
    bounds.count < 0 ||
    bounds.count > WIND_SHADOW_LIMITS.sourceInstances ||
    bounds.spheres.length !== bounds.count * 4 ||
    bounds.aabbs.length !== bounds.count * 6 ||
    padding.length !== bounds.count ||
    !bounds.spheres.every(Number.isFinite) ||
    !bounds.aabbs.every(Number.isFinite) ||
    !padding.every((n) => Number.isFinite(n) && n >= 0 && n <= 1000)
  )
    throw Error(
      "Wind culling needs complete finite bounds and bounded displacement.",
    );
  const spheres = bounds.spheres.slice(),
    aabbs = bounds.aabbs.slice();
  for (let i = 0; i < bounds.count; i++) {
    if (
      spheres[i * 4 + 3] < 0 ||
      [0, 1, 2].some((axis) => aabbs[i * 6 + axis] > aabbs[i * 6 + 3 + axis])
    )
      throw Error(
        "Wind culling bounds must have positive radii and ordered corners.",
      );
    spheres[i * 4 + 3] += padding[i];
    for (let axis = 0; axis < 3; axis++) {
      aabbs[i * 6 + axis] -= padding[i];
      aabbs[i * 6 + 3 + axis] += padding[i];
    }
  }
  return { count: bounds.count, spheres, aabbs };
}
