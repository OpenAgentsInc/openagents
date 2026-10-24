export const COMPILED_SOURCE_BATCH_VERSION = 1;
export const MAX_COMPILED_BATCH_MEMBERSHIPS = 1_000_000;
export const MAX_COMPILED_BATCH_BYTES = 128 * 1024 * 1024;
const DEFAULT_MEMBERSHIPS = 262_144;
const DEFAULT_BYTES = 32 * 1024 * 1024;
const MAX_DRAWS = 131_072;
const MAX_INSTANCES = 1_000_000;
const compiledPlans = new WeakSet<object>();

export interface SourceBatchDraw {
  firstIndex: number;
  indexCount: number;
  baseVertex: number;
  firstInstance: number;
  count: number;
  material: number;
  wind?: boolean;
  rasterOrientation?: "positive" | "negative" | "degenerate";
  sourceDrawIndex?: number;
  originalFirstIndex?: number;
}
export type GroupedSourceDraw<T extends SourceBatchDraw> = T & {
  sourceDrawIndex: number;
};
export interface CompiledBatchCounts {
  sourceInstances: number;
  sourceDraws: number;
  groupedDraws: number;
  eligibleDraws: number;
  mergedDraws: number;
  memberships: number;
}
export interface CompiledBatchFootprint {
  /** Exact typed-array bytes; JavaScript metadata/engine object storage is separate. */
  planBytes: number;
  gatherBytes: number;
  totalBytes: number;
  metadataRecords: number;
}
export interface CompiledSourceBatchPlan<T extends SourceBatchDraw> {
  readonly schema: "aethyrforge.compiled-source-batches.v1";
  readonly version: typeof COMPILED_SOURCE_BATCH_VERSION;
  /** Opaque identity is stable for this immutable plan, without a fallible hash cache key. */
  readonly identity: object;
  readonly counts: Readonly<CompiledBatchCounts>;
  readonly footprint: Readonly<CompiledBatchFootprint>;
  readonly capacity: number;
  /** Complete table with dense output IDs and maximum gathered ranges. */
  readonly baseDraws: readonly GroupedSourceDraw<T>[];
  /** Packed original source instance IDs. Duplicate primitive uses are intentional. */
  readonly memberships: Uint32Array;
  readonly groupOffsets: Uint32Array;
  readonly inputDrawToGroup: Uint32Array;
  /** Original input sourceDrawIndex values, or input indices when omitted. */
  readonly sourceDrawIds: Uint32Array;
  readonly groupDrawOffsets: Uint32Array;
  readonly groupInputDrawIndices: Uint32Array;
}
export interface CompileSourceBatchOptions<T extends SourceBatchDraw> {
  eligible(draw: T, index: number): boolean;
  enabled?: boolean;
  maxMemberships?: number;
  maxBytes?: number;
}
export type CompileSourceBatchResult<T extends SourceBatchDraw> =
  | { status: "compiled"; plan: CompiledSourceBatchPlan<T> }
  | {
      status: "fallback";
      reason:
        | "disabled"
        | "draw-limit"
        | "membership-limit"
        | "byte-limit"
        | "no-reduction";
      counts: Readonly<CompiledBatchCounts>;
    };
const uint = (n: number) =>
  Number.isSafeInteger(n) && n >= 0 && n <= 0xffffffff;

/** Compile compatible opaque uses; an ineligible draw ends the current merge segment. */
export function compileSourceBatches<T extends SourceBatchDraw>(
  draws: readonly T[],
  instanceCount: number,
  options: CompileSourceBatchOptions<T>,
): CompileSourceBatchResult<T> {
  if (
    !Number.isSafeInteger(instanceCount) ||
    instanceCount < 0 ||
    instanceCount > MAX_INSTANCES
  )
    throw new Error("Choose a bounded source instance count.");
  const maxMemberships = options.maxMemberships ?? DEFAULT_MEMBERSHIPS;
  const maxBytes = options.maxBytes ?? DEFAULT_BYTES;
  if (
    !Number.isSafeInteger(maxMemberships) ||
    maxMemberships < 0 ||
    maxMemberships > MAX_COMPILED_BATCH_MEMBERSHIPS ||
    !Number.isSafeInteger(maxBytes) ||
    maxBytes < 0 ||
    maxBytes > MAX_COMPILED_BATCH_BYTES
  )
    throw new Error("Choose bounded compiled batch storage limits.");
  const counts: CompiledBatchCounts = {
    sourceInstances: instanceCount,
    sourceDraws: draws.length,
    groupedDraws: draws.length,
    eligibleDraws: 0,
    mergedDraws: 0,
    memberships: 0,
  };
  const fallback = (
    reason: Extract<
      CompileSourceBatchResult<T>,
      { status: "fallback" }
    >["reason"],
  ): CompileSourceBatchResult<T> => ({
    status: "fallback",
    reason,
    counts: Object.freeze({ ...counts }),
  });
  if (options.enabled === false) return fallback("disabled");
  if (draws.length > MAX_DRAWS) return fallback("draw-limit");
  for (const draw of draws) {
    if (
      !uint(draw.firstIndex) ||
      !uint(draw.indexCount) ||
      draw.indexCount % 3 ||
      draw.firstIndex + draw.indexCount > 0xffffffff ||
      !Number.isSafeInteger(draw.baseVertex) ||
      draw.baseVertex < -0x80000000 ||
      draw.baseVertex > 0x7fffffff ||
      !uint(draw.firstInstance) ||
      !uint(draw.count) ||
      draw.firstInstance + draw.count > instanceCount ||
      !uint(draw.material) ||
      (draw.sourceDrawIndex !== undefined && !uint(draw.sourceDrawIndex)) ||
      (draw.originalFirstIndex !== undefined &&
        !uint(draw.originalFirstIndex)) ||
      (draw.wind !== undefined && typeof draw.wind !== "boolean") ||
      (draw.rasterOrientation !== undefined &&
        !["positive", "negative", "degenerate"].includes(
          draw.rasterOrientation,
        ))
    )
      throw new Error(
        "Use complete bounded source geometry, material, orientation, and instance ranges.",
      );
    counts.memberships += draw.count;
  }
  // All capacity checks precede membership arrays and gather slabs.
  if (counts.memberships > maxMemberships) return fallback("membership-limit");
  const worstBytes =
    counts.memberships * 88 + (draws.length + 1) * 8 + draws.length * 12;
  if (worstBytes > maxBytes) return fallback("byte-limit");

  const groups: { draw: T; inputs: number[]; count: number }[] = [];
  const active = new Map<string, number>();
  const inputDrawToGroup = new Uint32Array(draws.length);
  const sourceDrawIds = new Uint32Array(draws.length);
  for (let index = 0; index < draws.length; index++) {
    const draw = draws[index],
      eligible = options.eligible(draw, index);
    if (typeof eligible !== "boolean")
      throw new Error("Choose explicit compiled batch eligibility.");
    counts.eligibleDraws += Number(eligible);
    if (!eligible) active.clear();
    const key = eligible
      ? [
          draw.firstIndex,
          draw.indexCount,
          draw.baseVertex,
          draw.material,
          Number(Boolean(draw.wind)),
          draw.rasterOrientation ?? "unknown",
          draw.originalFirstIndex ?? "original",
        ].join(":")
      : undefined;
    let group = key === undefined ? undefined : active.get(key);
    if (group === undefined) {
      group = groups.length;
      groups.push({ draw: { ...draw }, inputs: [], count: 0 });
      if (key !== undefined) active.set(key, group);
    }
    groups[group].inputs.push(index);
    groups[group].count += draw.count;
    inputDrawToGroup[index] = group;
    sourceDrawIds[index] = draw.sourceDrawIndex ?? index;
  }
  counts.groupedDraws = groups.length;
  counts.mergedDraws = draws.length - groups.length;
  if (!counts.mergedDraws) return fallback("no-reduction");
  const groupOffsets = new Uint32Array(groups.length + 1);
  const groupDrawOffsets = new Uint32Array(groups.length + 1);
  const groupInputDrawIndices = new Uint32Array(draws.length);
  const memberships = new Uint32Array(counts.memberships);
  const baseDraws: GroupedSourceDraw<T>[] = [];
  let memberOffset = 0,
    drawOffset = 0;
  groups.forEach((group, index) => {
    groupOffsets[index] = memberOffset;
    groupDrawOffsets[index] = drawOffset;
    for (const inputIndex of group.inputs) {
      groupInputDrawIndices[drawOffset++] = inputIndex;
      const draw = draws[inputIndex];
      for (
        let source = draw.firstInstance;
        source < draw.firstInstance + draw.count;
        source++
      )
        memberships[memberOffset++] = source;
    }
    baseDraws.push(
      Object.freeze({
        ...group.draw,
        firstInstance: groupOffsets[index],
        count: group.count,
        sourceDrawIndex: index,
      }),
    );
  });
  groupOffsets[groups.length] = memberOffset;
  groupDrawOffsets[groups.length] = drawOffset;
  const planBytes =
    memberships.byteLength +
    groupOffsets.byteLength +
    inputDrawToGroup.byteLength +
    sourceDrawIds.byteLength +
    groupDrawOffsets.byteLength +
    groupInputDrawIndices.byteLength;
  const gatherBytes = counts.memberships * 84;
  const plan: CompiledSourceBatchPlan<T> = Object.freeze({
    schema: "aethyrforge.compiled-source-batches.v1",
    version: COMPILED_SOURCE_BATCH_VERSION,
    identity: Object.freeze({}),
    counts: Object.freeze({ ...counts }),
    footprint: Object.freeze({
      planBytes,
      gatherBytes,
      totalBytes: planBytes + gatherBytes,
      metadataRecords: draws.length + groups.length * 2,
    }),
    capacity: counts.memberships,
    baseDraws: Object.freeze(baseDraws),
    memberships,
    groupOffsets,
    inputDrawToGroup,
    sourceDrawIds,
    groupDrawOffsets,
    groupInputDrawIndices,
  });
  compiledPlans.add(plan);
  return { status: "compiled", plan };
}

export interface CompiledBatchGatherSource {
  models: Float32Array;
  wind?: Float32Array;
  visibility: Uint8Array;
  excluded?: ReadonlySet<number>;
  /** Increment whenever the source model/wind arrays mutate in place. */
  sourceVersion?: number;
}
export interface CompiledBatchGatherResult<T extends SourceBatchDraw> {
  models: Float32Array;
  wind: Float32Array;
  sourceIndices: Uint32Array;
  groupedDraws: GroupedSourceDraw<T>[];
  visibleDraws: GroupedSourceDraw<T>[];
  count: number;
}

/** One view owns its slabs. Returned arrays are borrowed until the next gather. */
export class CompiledBatchGatherer<T extends SourceBatchDraw> {
  private outModels: Float32Array;
  private outWind: Float32Array;
  private outSourceIndices: Uint32Array;
  readonly groupedDraws: GroupedSourceDraw<T>[];
  private readonly visibleDraws: GroupedSourceDraw<T>[] = [];
  private readonly result: CompiledBatchGatherResult<T>;
  private sourceModels?: Float32Array;
  private sourceWind?: Float32Array;
  private sourceVersion = -1;
  private closed = false;
  private readonly requiresWind: boolean;
  constructor(readonly plan: CompiledSourceBatchPlan<T>) {
    if (!compiledPlans.has(plan))
      throw new Error(
        "Use a source batch plan compiled within the declared storage limits.",
      );
    this.outModels = new Float32Array(plan.capacity * 16);
    this.outWind = new Float32Array(plan.capacity * 4);
    this.outSourceIndices = new Uint32Array(plan.capacity);
    this.requiresWind = plan.baseDraws.some((draw) => draw.wind);
    this.groupedDraws = plan.baseDraws.map((draw) => ({ ...draw }));
    this.result = {
      models: this.models.subarray(0, 0),
      wind: this.wind.subarray(0, 0),
      sourceIndices: this.sourceIndices.subarray(0, 0),
      groupedDraws: this.groupedDraws,
      visibleDraws: this.visibleDraws,
      count: 0,
    };
  }
  get models() {
    return this.outModels;
  }
  get wind() {
    return this.outWind;
  }
  get sourceIndices() {
    return this.outSourceIndices;
  }
  get memoryBytes() {
    return (
      this.models.byteLength +
      this.wind.byteLength +
      this.sourceIndices.byteLength
    );
  }
  gather(source: CompiledBatchGatherSource): CompiledBatchGatherResult<T> {
    if (this.closed) throw new Error("The compiled batch gatherer has closed.");
    const { models, wind, visibility, excluded } = source,
      count = this.plan.counts.sourceInstances,
      version = source.sourceVersion ?? 0;
    if (
      !(models instanceof Float32Array) ||
      models.length !== count * 16 ||
      (wind !== undefined &&
        (!(wind instanceof Float32Array) || wind.length !== count * 4)) ||
      (this.requiresWind && wind === undefined) ||
      !(visibility instanceof Uint8Array) ||
      visibility.length !== count ||
      !Number.isSafeInteger(version) ||
      version < 0
    )
      throw new Error(
        "Use complete source arrays, visibility, and a valid source version.",
      );
    if (
      models !== this.sourceModels ||
      wind !== this.sourceWind ||
      version !== this.sourceVersion
    ) {
      if (
        !models.every(Number.isFinite) ||
        (wind && !wind.every(Number.isFinite))
      )
        throw new Error("Use finite source model and wind values.");
      for (let index = 0; index < count; index++) {
        const offset = index * 16;
        if (
          Math.abs(models[offset + 3]) +
            Math.abs(models[offset + 7]) +
            Math.abs(models[offset + 11]) >
            1e-6 ||
          Math.abs(models[offset + 15] - 1) > 1e-6
        )
          throw new Error(
            "Compiled source batches need affine model matrices.",
          );
      }
      this.sourceModels = models;
      this.sourceWind = wind;
      this.sourceVersion = version;
    }
    for (const value of visibility)
      if (value !== 0 && value !== 1)
        throw new Error("Use binary source visibility.");
    if (excluded)
      for (const id of excluded)
        if (!Number.isSafeInteger(id) || id < 0 || id >= count)
          throw new Error(
            "Excluded placements must identify original source instances.",
          );
    let rank = 0;
    this.visibleDraws.length = 0;
    for (let group = 0; group < this.groupedDraws.length; group++) {
      const draw = this.groupedDraws[group],
        first = rank;
      for (
        let member = this.plan.groupOffsets[group];
        member < this.plan.groupOffsets[group + 1];
        member++
      ) {
        const id = this.plan.memberships[member];
        if (!visibility[id] || excluded?.has(id)) continue;
        for (let column = 0; column < 16; column++)
          this.models[rank * 16 + column] = models[id * 16 + column];
        if (wind)
          for (let column = 0; column < 4; column++)
            this.wind[rank * 4 + column] = wind[id * 4 + column];
        this.sourceIndices[rank++] = id;
      }
      draw.firstInstance = first;
      draw.count = rank - first;
      if (draw.count) this.visibleDraws.push(draw);
    }
    if (!wind) this.wind.fill(0, 0, rank * 4);
    this.result.models = this.models.subarray(0, rank * 16);
    this.result.wind = this.wind.subarray(0, rank * 4);
    this.result.sourceIndices = this.sourceIndices.subarray(0, rank);
    this.result.count = rank;
    return this.result;
  }
  dispose() {
    if (this.closed) return;
    this.closed = true;
    this.sourceModels = this.sourceWind = undefined;
    this.visibleDraws.length = 0;
    this.groupedDraws.length = 0;
    this.outModels = new Float32Array();
    this.outWind = new Float32Array();
    this.outSourceIndices = new Uint32Array();
    this.result.models = this.outModels;
    this.result.wind = this.outWind;
    this.result.sourceIndices = this.outSourceIndices;
    this.result.count = 0;
  }
}
export function createCompiledBatchGatherer<T extends SourceBatchDraw>(
  plan: CompiledSourceBatchPlan<T>,
): CompiledBatchGatherer<T> {
  return new CompiledBatchGatherer(plan);
}
