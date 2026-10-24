import type { CompiledMaterial } from "../assets/src/scene.ts";
import {
  compileSourceBatches,
  createCompiledBatchGatherer,
  type CompiledBatchGatherer,
  type CompiledSourceBatchPlan,
  type SourceBatchDraw,
} from "./compiled-source-batches.ts";

interface CachedPlan {
  instanceCount: number;
  materials: readonly CompiledMaterial[];
  materialCount: number;
  maxMemberships: number;
  sourceVersion: number;
  plan: CompiledSourceBatchPlan<SourceBatchDraw> | null;
  draws: readonly Readonly<Record<string, unknown>>[] | undefined;
  materialRefs: readonly CompiledMaterial[];
  materialEligibility: Uint8Array;
}

const MAX_DRAW_SNAPSHOTS = 131_072;

function eligibleMaterial(material: CompiledMaterial): boolean {
  return (
    material.alphaMode === "OPAQUE" &&
    !material.glass &&
    !material.flame &&
    !material.vegetation
  );
}

function sameDraws(
  draws: readonly SourceBatchDraw[],
  snapshot: CachedPlan["draws"],
): boolean {
  if (!snapshot || snapshot.length !== draws.length) return false;
  for (let index = 0; index < draws.length; index++) {
    const draw = draws[index] as unknown as Record<string, unknown>;
    const previous = snapshot[index];
    let keys = 0;
    for (const key in draw) {
      if (!Object.hasOwn(draw, key)) continue;
      keys++;
      if (!Object.hasOwn(previous, key) || !Object.is(previous[key], draw[key]))
        return false;
    }
    if (keys !== Object.keys(previous).length) return false;
  }
  return true;
}

function sameMaterials(
  entry: CachedPlan,
  materials: readonly CompiledMaterial[],
  materialCount: number,
): boolean {
  if (entry.materialCount !== materialCount) return false;
  for (let index = 0; index < materialCount; index++)
    if (
      entry.materialRefs[index] !== materials[index] ||
      entry.materialEligibility[index] !==
        Number(eligibleMaterial(materials[index]))
    )
      return false;
  return true;
}

/** Editor-owned batch planning. Each renderer submission owns its gatherer. */
export class CompiledSourceHost {
  private plans = new WeakMap<readonly SourceBatchDraw[], CachedPlan>();
  private closed = false;

  /** In-place draw/material mutations require a new sourceVersion. */
  prepare<T extends SourceBatchDraw>(
    draws: readonly T[],
    instanceCount: number,
    materials: readonly CompiledMaterial[],
    materialCount: number,
    maxMemberships: number,
    previous?: CompiledBatchGatherer<T>,
    sourceVersion = 0,
  ): CompiledBatchGatherer<T> | undefined {
    if (this.closed) throw new Error("The compiled source host has closed.");
    if (
      !Number.isSafeInteger(sourceVersion) ||
      sourceVersion < 0 ||
      !Number.isSafeInteger(materialCount) ||
      materialCount < 0 ||
      materialCount > materials.length
    )
      throw new Error(
        "Choose a valid source version and source material range.",
      );

    let entry = this.plans.get(draws);
    const sameInputs =
      entry &&
      entry.instanceCount === instanceCount &&
      entry.materials === materials &&
      entry.materialCount === materialCount &&
      entry.maxMemberships === maxMemberships;
    if (entry && sameInputs && entry.sourceVersion === sourceVersion)
      return this.gatherer(entry.plan, previous);

    if (
      entry &&
      sameInputs &&
      sameDraws(draws, entry.draws) &&
      sameMaterials(entry, materials, materialCount)
    ) {
      entry.sourceVersion = sourceVersion;
      return this.gatherer(entry.plan, previous);
    }

    const result = compileSourceBatches(draws, instanceCount, {
      maxMemberships,
      eligible: (draw) =>
        draw.material < materialCount &&
        eligibleMaterial(materials[draw.material]) &&
        !draw.wind,
    });
    entry = {
      instanceCount,
      materials,
      materialCount,
      maxMemberships,
      sourceVersion,
      plan: result.status === "compiled" ? result.plan : null,
      draws:
        draws.length <= MAX_DRAW_SNAPSHOTS
          ? draws.map((draw) => Object.freeze({ ...draw }))
          : undefined,
      materialRefs: materials.slice(0, materialCount),
      materialEligibility: Uint8Array.from(
        { length: materialCount },
        (_, index) => Number(eligibleMaterial(materials[index])),
      ),
    };
    this.plans.set(draws, entry);
    return this.gatherer(entry.plan, previous);
  }

  private gatherer<T extends SourceBatchDraw>(
    plan: CachedPlan["plan"],
    previous: CompiledBatchGatherer<T> | undefined,
  ): CompiledBatchGatherer<T> | undefined {
    if (!plan) return undefined;
    const typedPlan = plan as CompiledSourceBatchPlan<T>;
    return previous?.plan === typedPlan &&
      previous.memoryBytes === typedPlan.footprint.gatherBytes
      ? previous
      : createCompiledBatchGatherer(typedPlan);
  }

  dispose(): void {
    if (this.closed) return;
    this.closed = true;
    this.plans = new WeakMap();
  }
}

export function createCompiledSourceHost(): CompiledSourceHost {
  return new CompiledSourceHost();
}
