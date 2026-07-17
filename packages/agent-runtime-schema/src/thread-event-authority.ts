import { Schema as S } from "effect";

export const ThreadEventAuthoritySchemaLiteral = "openagents.thread_event_authority.v1" as const;

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const Timestamp = S.String.check(S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/));

const base = {
  schema: S.Literal(ThreadEventAuthoritySchemaLiteral),
  relationRef: Ref,
  threadRef: Ref,
  eventRef: Ref,
  observedAt: Timestamp,
};

/**
 * Ref-only authority relations over canonical accepted thread events. Bodies,
 * summaries, and provider payloads stay in their owning event log.
 */
export const ThreadEventAuthorityRelation = S.Union([
  S.Struct({ ...base, kind: S.Literal("accepted") }),
  S.Struct({
    ...base,
    kind: S.Literal("superseded"),
    supersededByEventRef: Ref,
  }),
  S.Struct({
    ...base,
    kind: S.Literal("reverted"),
    revertedByEventRef: Ref,
    restoredEventRef: Ref,
  }),
]);
export type ThreadEventAuthorityRelation = typeof ThreadEventAuthorityRelation.Type;

const decodeRelation = S.decodeUnknownSync(ThreadEventAuthorityRelation);

export const decodeThreadEventAuthorityRelation = (
  input: unknown,
): ThreadEventAuthorityRelation => {
  if (typeof input === "object" && input !== null) {
    const value = input as Readonly<Record<string, unknown>>;
    for (const forbidden of ["body", "message", "prompt", "summary"] as const) {
      if (Object.hasOwn(value, forbidden)) {
        throw new Error(
          `Thread event authority relation contains forbidden raw field: ${forbidden}`,
        );
      }
    }
  }
  const decoded = decodeRelation(input);
  if (!Number.isFinite(Date.parse(decoded.observedAt))) {
    throw new Error("Thread event authority observation timestamp is invalid");
  }
  if (decoded.kind === "superseded" && decoded.eventRef === decoded.supersededByEventRef) {
    throw new Error("A thread event cannot supersede itself");
  }
  if (
    decoded.kind === "reverted" &&
    (decoded.eventRef === decoded.revertedByEventRef ||
      decoded.eventRef === decoded.restoredEventRef ||
      decoded.revertedByEventRef === decoded.restoredEventRef)
  ) {
    throw new Error("A thread event revert must name distinct reverted, revert, and restored refs");
  }
  return decoded;
};

export type ThreadEventAuthorityProjection =
  | Readonly<{ status: "missing"; threadRef: string; eventRef: string }>
  | Readonly<{
      status: "resolved";
      threadRef: string;
      eventRef: string;
      state: "accepted" | "superseded" | "reverted";
      relationRefs: ReadonlyArray<string>;
      supersededByEventRef?: string;
      revertedByEventRef?: string;
      restoredEventRef?: string;
    }>
  | Readonly<{
      status: "conflict";
      threadRef: string;
      eventRef: string;
      reason:
        | "invalid_relation"
        | "cross_thread"
        | "duplicate_relation"
        | "ambiguous_order"
        | "invalid_transition";
      relationRefs: ReadonlyArray<string>;
    }>;

/**
 * Projects one event's final authority state from append-only relation facts.
 * The only legal v1 history is `accepted`, optionally followed by exactly one
 * `superseded` or `reverted` fact. Ambiguity is returned, never tie-broken.
 */
export const projectThreadEventAuthority = (
  input: Readonly<{
    threadRef: string;
    eventRef: string;
    relations: ReadonlyArray<unknown>;
  }>,
): ThreadEventAuthorityProjection => {
  const matching: ThreadEventAuthorityRelation[] = [];
  for (const raw of input.relations) {
    let relation: ThreadEventAuthorityRelation;
    try {
      relation = decodeThreadEventAuthorityRelation(raw);
    } catch {
      const value = raw as Readonly<{ eventRef?: unknown; relationRef?: unknown }>;
      if (value?.eventRef !== input.eventRef) continue;
      return {
        status: "conflict",
        threadRef: input.threadRef,
        eventRef: input.eventRef,
        reason: "invalid_relation",
        relationRefs: typeof value.relationRef === "string" ? [value.relationRef] : [],
      };
    }
    if (relation.eventRef !== input.eventRef) continue;
    if (relation.threadRef !== input.threadRef) {
      return {
        status: "conflict",
        threadRef: input.threadRef,
        eventRef: input.eventRef,
        reason: "cross_thread",
        relationRefs: [relation.relationRef],
      };
    }
    matching.push(relation);
  }
  if (matching.length === 0) {
    return { status: "missing", threadRef: input.threadRef, eventRef: input.eventRef };
  }
  const relationRefs = matching.map(({ relationRef }) => relationRef);
  if (new Set(relationRefs).size !== relationRefs.length) {
    return {
      status: "conflict",
      threadRef: input.threadRef,
      eventRef: input.eventRef,
      reason: "duplicate_relation",
      relationRefs,
    };
  }
  const ordered = [...matching].sort(
    (left, right) => Date.parse(left.observedAt) - Date.parse(right.observedAt),
  );
  if (
    ordered.some(
      (relation, index) => index > 0 && relation.observedAt === ordered[index - 1]!.observedAt,
    )
  ) {
    return {
      status: "conflict",
      threadRef: input.threadRef,
      eventRef: input.eventRef,
      reason: "ambiguous_order",
      relationRefs: ordered.map(({ relationRef }) => relationRef),
    };
  }
  if (
    ordered[0]?.kind !== "accepted" ||
    ordered.length > 2 ||
    (ordered.length === 2 && ordered[1]?.kind === "accepted")
  ) {
    return {
      status: "conflict",
      threadRef: input.threadRef,
      eventRef: input.eventRef,
      reason: "invalid_transition",
      relationRefs: ordered.map(({ relationRef }) => relationRef),
    };
  }
  const final = ordered.at(-1)!;
  return {
    status: "resolved",
    threadRef: input.threadRef,
    eventRef: input.eventRef,
    state: final.kind,
    relationRefs: ordered.map(({ relationRef }) => relationRef),
    ...(final.kind === "superseded"
      ? { supersededByEventRef: final.supersededByEventRef }
      : final.kind === "reverted"
        ? {
            revertedByEventRef: final.revertedByEventRef,
            restoredEventRef: final.restoredEventRef,
          }
        : {}),
  };
};
