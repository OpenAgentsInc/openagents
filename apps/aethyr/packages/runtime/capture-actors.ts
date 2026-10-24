export const CAPTURE_ACTORS_SCHEMA = "aethyrforge.capture-actors.v1" as const;
export const CAPTURE_ACTOR_LIMIT = 16;

/** A canonical pose setup is an explicit pristine-world diagnostic, never a play action. */
export function assertNativeStartCapture(options: {
  diagnostic: boolean;
  recording: boolean;
  preparing: boolean;
  gameplay: unknown;
}): void {
  const state = options.gameplay as Record<string, any> | undefined;
  if (
    !options.diagnostic ||
    options.recording ||
    options.preparing ||
    !state ||
    ![
      state.castCount,
      state.destroyedCount,
      state.pendingImpacts,
      state.pendingCollapse,
    ].every((value) => value === 0) ||
    state.activeCast ||
    state.castle?.active ||
    state.castle?.pendingImpacts ||
    !Array.isArray(state.debris) ||
    state.debris.length ||
    !Array.isArray(state.effects) ||
    state.effects.length ||
    !state.enemies ||
    !Number.isInteger(state.enemies.total) ||
    state.enemies.total < 1 ||
    state.enemies.total > CAPTURE_ACTOR_LIMIT ||
    state.enemies.alive !== state.enemies.total ||
    state.enemies.killed !== 0 ||
    !Array.isArray(state.enemies.actors) ||
    state.enemies.actors.length !== state.enemies.total
  )
    throw new Error(
      "Start a pristine diagnostic world and stop measurement before preparing its native-start actor capture.",
    );
}

export interface ActorCaptureReferenceMetadata {
  schema: typeof CAPTURE_ACTORS_SCHEMA;
  id: string;
  sourceIdentity: string;
  scenario: "day" | "night";
  modelIdentity: string;
  sourceFrame: number;
  actorIds: number[];
  count: number;
  poseSha256: string;
  /** A diagnostic reference captured after the explicit native-start setup. */
  basis?: "native-start";
}

const hash = (value: unknown): value is string =>
  typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
const scenario = (value: unknown): value is "day" | "night" =>
  value === "day" || value === "night";
const actors = (value: unknown): value is number[] =>
  Array.isArray(value) &&
  value.length <= CAPTURE_ACTOR_LIMIT &&
  Array.from(value).every(
    (id) => Number.isInteger(id) && id >= 0 && id <= 0xffff_ffff,
  ) &&
  new Set(value).size === value.length;

/** Admit an opaque ID; actor poses and animation palettes never enter this value. */
export function validateCaptureActorReferenceId(value: unknown): string {
  if (
    typeof value !== "string" ||
    !/^[A-Za-z0-9][A-Za-z0-9_-]{0,95}$/.test(value)
  )
    throw new Error("Use a valid actor capture reference ID.");
  return value;
}

/** Copy the complete public metadata and reject additional payload fields. */
export function validateCaptureActorReferenceMetadata(
  value: unknown,
): ActorCaptureReferenceMetadata {
  const keys = [
    "schema",
    "id",
    "sourceIdentity",
    "scenario",
    "modelIdentity",
    "sourceFrame",
    "actorIds",
    "count",
    "poseSha256",
  ];
  if (!value || typeof value !== "object" || Array.isArray(value))
    throw new Error("Use complete actor capture reference details.");
  const v = value as Record<string, unknown>;
  const hasBasis = Object.hasOwn(v, "basis");
  if (
    Object.keys(v).length !== keys.length + Number(hasBasis) ||
    Object.keys(v).some((key) => !keys.includes(key) && key !== "basis") ||
    (hasBasis && v.basis !== "native-start") ||
    v.schema !== CAPTURE_ACTORS_SCHEMA ||
    !hash(v.sourceIdentity) ||
    !scenario(v.scenario) ||
    !hash(v.modelIdentity) ||
    !hash(v.poseSha256) ||
    !Number.isSafeInteger(v.sourceFrame) ||
    Number(v.sourceFrame) < 0 ||
    !actors(v.actorIds) ||
    v.count !== v.actorIds.length
  )
    throw new Error("Use complete, bounded actor capture reference details.");
  return {
    schema: CAPTURE_ACTORS_SCHEMA,
    id: validateCaptureActorReferenceId(v.id),
    sourceIdentity: v.sourceIdentity,
    scenario: v.scenario,
    modelIdentity: v.modelIdentity,
    sourceFrame: Number(v.sourceFrame),
    actorIds: [...v.actorIds],
    count: v.actorIds.length,
    poseSha256: v.poseSha256,
    ...(hasBasis ? { basis: "native-start" as const } : {}),
  };
}

/** Compare the complete public reference; invalid or additional fields never match. */
export function sameCaptureActorReference(a: unknown, b: unknown): boolean {
  try {
    const left = validateCaptureActorReferenceMetadata(a);
    const right = validateCaptureActorReferenceMetadata(b);
    return (
      left.id === right.id &&
      left.schema === right.schema &&
      left.sourceIdentity === right.sourceIdentity &&
      left.scenario === right.scenario &&
      left.modelIdentity === right.modelIdentity &&
      left.sourceFrame === right.sourceFrame &&
      left.count === right.count &&
      left.poseSha256 === right.poseSha256 &&
      left.basis === right.basis &&
      left.actorIds.every((actor, index) => actor === right.actorIds[index])
    );
  } catch {
    return false;
  }
}

export interface CaptureActorPin<T> {
  scenario: "day" | "night";
  modelIdentity: string;
  sourceFrame: number;
  actorIds: readonly number[];
  poseSha256: string;
  basis?: "native-start";
  /** Trusted host-owned snapshot. Its bounded pose storage stays outside transport. */
  payload: T;
}
export interface CaptureActorLookup {
  scenario: "day" | "night";
  modelIdentity: string;
  actorIds: readonly number[];
}

/** Two host-only snapshots, one per scenario, bound to one runtime incarnation. */
export class CaptureActorReferences<T> {
  private readonly records = new Map<
    "day" | "night",
    {
      metadata: ActorCaptureReferenceMetadata;
      incarnation: object;
      payload: T;
    }
  >();
  private readonly nonce = crypto.randomUUID();
  private sequence = 0;
  private disposed = false;
  private readonly sourceIdentity: string;
  private readonly incarnation: object;

  constructor(options: { sourceIdentity: string; incarnation: object }) {
    if (
      !options ||
      !hash(options.sourceIdentity) ||
      !options.incarnation ||
      typeof options.incarnation !== "object"
    )
      throw new Error(
        "Open actor capture references with a valid world and runtime.",
      );
    this.sourceIdentity = options.sourceIdentity;
    this.incarnation = options.incarnation;
  }

  private open() {
    if (this.disposed) throw new Error("Actor capture references are closed.");
  }

  pin(input: CaptureActorPin<T>): ActorCaptureReferenceMetadata {
    this.open();
    if (!input || this.sequence >= Number.MAX_SAFE_INTEGER)
      throw new Error("Create a new actor capture reference registry.");
    const metadata = validateCaptureActorReferenceMetadata({
      schema: CAPTURE_ACTORS_SCHEMA,
      id: `actors_${this.nonce}_${(++this.sequence).toString(36)}`,
      sourceIdentity: this.sourceIdentity,
      scenario: input.scenario,
      modelIdentity: input.modelIdentity,
      sourceFrame: input.sourceFrame,
      actorIds: Array.isArray(input.actorIds)
        ? [...input.actorIds]
        : input.actorIds,
      count: input.actorIds?.length,
      poseSha256: input.poseSha256,
      ...(input.basis !== undefined ? { basis: input.basis } : {}),
    });
    Object.freeze(metadata.actorIds);
    Object.freeze(metadata);
    this.records.set(metadata.scenario, {
      metadata,
      incarnation: this.incarnation,
      payload: input.payload,
    });
    return validateCaptureActorReferenceMetadata(metadata);
  }

  resolve(
    id: string,
    expected: CaptureActorLookup,
  ): { metadata: ActorCaptureReferenceMetadata; payload: T } {
    this.open();
    validateCaptureActorReferenceId(id);
    if (
      !expected ||
      !scenario(expected.scenario) ||
      !hash(expected.modelIdentity) ||
      !actors(expected.actorIds)
    )
      throw new Error(
        "Match the current scenario, model, and actors before capturing.",
      );
    const record = this.records.get(expected.scenario);
    if (
      !record ||
      record.metadata.id !== id ||
      record.incarnation !== this.incarnation ||
      record.metadata.sourceIdentity !== this.sourceIdentity ||
      record.metadata.modelIdentity !== expected.modelIdentity ||
      record.metadata.actorIds.length !== expected.actorIds.length ||
      record.metadata.actorIds.some(
        (actor, index) => actor !== expected.actorIds[index],
      )
    )
      throw new Error(
        "That actor capture reference is no longer available for this world and its current actors.",
      );
    return {
      metadata: validateCaptureActorReferenceMetadata(record.metadata),
      payload: record.payload,
    };
  }

  clear(): void {
    this.open();
    this.records.clear();
  }

  dispose(): void {
    if (this.disposed) return;
    this.records.clear();
    this.disposed = true;
  }
}
