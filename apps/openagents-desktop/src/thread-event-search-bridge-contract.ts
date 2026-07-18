import {
  ThreadEventSearchProjectionSchemaLiteral,
  type ThreadEventSearchProjection,
} from "@openagentsinc/agent-runtime-schema";
import { Schema as S } from "effect";

export const DesktopThreadEventSearchChannel = "openagents:thread-event-search:query" as const;

const MAX_QUERY_CHARS = 200;
const MAX_RESULTS = 100;
const MAX_INDEXED_EVENTS = 10_000;
const MAX_RELATION_REFS = 1_000;
const MAX_SNIPPET_CHARS = 240;

const Ref = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
);
const Count = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(MAX_INDEXED_EVENTS),
);
const RelationRefs = S.Array(Ref).check(S.isMinLength(1), S.isMaxLength(MAX_RELATION_REFS));
const Authority = S.Union([
  S.Struct({
    state: S.Literal("accepted"),
    relationRefs: RelationRefs,
  }),
  S.Struct({
    state: S.Literal("superseded"),
    relationRefs: RelationRefs,
    supersededByEventRef: Ref,
  }),
  S.Struct({
    state: S.Literal("reverted"),
    relationRefs: RelationRefs,
    revertedByEventRef: Ref,
    restoredEventRef: Ref,
  }),
]);
const Projection = S.Struct({
  schema: S.Literal(ThreadEventSearchProjectionSchemaLiteral),
  query: S.String.check(S.isMaxLength(MAX_QUERY_CHARS)),
  results: S.Array(
    S.Struct({
      threadRef: Ref,
      eventRef: Ref,
      sequence: Count,
      authority: Authority,
      snippet: S.String.check(S.isMaxLength(MAX_SNIPPET_CHARS)),
      score: S.Literals([1, 2, 3]),
    }),
  ).check(S.isMaxLength(MAX_RESULTS)),
  indexedEvents: Count,
  totalMatches: Count,
  indexTruncated: S.Boolean,
  resultsTruncated: S.Boolean,
});

export type DesktopThreadEventSearchRequest = Readonly<{
  query: string;
  limit?: number;
}>;

export const DesktopThreadEventSearchResult = S.Union([
  S.Struct({ status: S.Literal("available"), projection: Projection }),
  S.Struct({
    status: S.Literal("unavailable"),
    reason: S.Literals([
      "invalid_request",
      "artifact_unavailable",
      "artifact_corrupt",
      "identity_mismatch",
      "projection_rejected",
      "transport_unavailable",
    ]),
  }),
]);
export type DesktopThreadEventSearchResult = typeof DesktopThreadEventSearchResult.Type;

const ownKeysAre = (value: unknown, allowed: ReadonlyArray<string>): boolean => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const actual = Object.keys(value).sort();
  const expected = [...allowed].sort();
  return actual.length === expected.length && actual.every((key, index) => key === expected[index]);
};

const isObject = (value: unknown): value is object =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const normalizedQuery = (value: unknown): string | null => {
  if (typeof value !== "string" || value.length > MAX_QUERY_CHARS) return null;
  return value.replace(/\s+/gu, " ").trim();
};

export const decodeDesktopThreadEventSearchRequest = (
  input: unknown,
): DesktopThreadEventSearchRequest | null => {
  if (typeof input !== "object" || input === null || Array.isArray(input)) return null;
  const hasLimit = Object.hasOwn(input, "limit");
  if (!ownKeysAre(input, hasLimit ? ["query", "limit"] : ["query"])) return null;
  const query = normalizedQuery(Reflect.get(input, "query"));
  if (query === null) return null;
  if (!hasLimit) return { query };
  const limit = Reflect.get(input, "limit");
  if (!Number.isInteger(limit) || typeof limit !== "number" || limit < 1 || limit > MAX_RESULTS) {
    return null;
  }
  return { query, limit };
};

const exactAuthority = (value: unknown, eventRef: unknown): boolean => {
  if (!isObject(value)) return false;
  const state = Reflect.get(value, "state");
  const exact =
    state === "accepted"
      ? ownKeysAre(value, ["state", "relationRefs"])
      : state === "superseded"
        ? ownKeysAre(value, ["state", "relationRefs", "supersededByEventRef"])
        : state === "reverted"
          ? ownKeysAre(value, ["state", "relationRefs", "revertedByEventRef", "restoredEventRef"])
          : false;
  if (!exact) return false;
  const relationRefs = Reflect.get(value, "relationRefs");
  if (!Array.isArray(relationRefs) || new Set(relationRefs).size !== relationRefs.length) {
    return false;
  }
  if (state === "superseded") {
    return Reflect.get(value, "supersededByEventRef") !== eventRef;
  }
  if (state === "reverted") {
    const revertedBy = Reflect.get(value, "revertedByEventRef");
    const restored = Reflect.get(value, "restoredEventRef");
    return revertedBy !== eventRef && restored !== eventRef && revertedBy !== restored;
  }
  return true;
};

const exactProjection = (value: unknown): boolean => {
  if (!isObject(value)) return false;
  if (
    !ownKeysAre(value, [
      "schema",
      "query",
      "results",
      "indexedEvents",
      "totalMatches",
      "indexTruncated",
      "resultsTruncated",
    ])
  )
    return false;
  const query = Reflect.get(value, "query");
  if (normalizedQuery(query) !== query) return false;
  const results = Reflect.get(value, "results");
  if (!Array.isArray(results)) return false;
  const identities = new Set<string>();
  for (const result of results) {
    if (!isObject(result)) return false;
    if (
      !ownKeysAre(result, ["threadRef", "eventRef", "sequence", "authority", "snippet", "score"]) ||
      !exactAuthority(Reflect.get(result, "authority"), Reflect.get(result, "eventRef"))
    )
      return false;
    const identity = `${String(Reflect.get(result, "threadRef"))}\u0000${String(Reflect.get(result, "eventRef"))}`;
    if (identities.has(identity)) return false;
    identities.add(identity);
  }
  const totalMatches = Reflect.get(value, "totalMatches");
  const indexedEvents = Reflect.get(value, "indexedEvents");
  return (
    typeof totalMatches === "number" &&
    typeof indexedEvents === "number" &&
    totalMatches >= results.length &&
    indexedEvents >= totalMatches &&
    Reflect.get(value, "resultsTruncated") === totalMatches > results.length
  );
};

const decodeResult = S.decodeUnknownSync(DesktopThreadEventSearchResult);

export const decodeDesktopThreadEventSearchResult = (
  input: unknown,
): DesktopThreadEventSearchResult | null => {
  if (!isObject(input)) return null;
  const status = Reflect.get(input, "status");
  const exact =
    status === "available"
      ? ownKeysAre(input, ["status", "projection"]) &&
        exactProjection(Reflect.get(input, "projection"))
      : status === "unavailable"
        ? ownKeysAre(input, ["status", "reason"])
        : false;
  if (!exact) return null;
  try {
    return decodeResult(input);
  } catch {
    return null;
  }
};

export const unavailableDesktopThreadEventSearchResult = (): DesktopThreadEventSearchResult => ({
  status: "unavailable",
  reason: "transport_unavailable",
});

export type DesktopThreadEventSearchInvoker = (
  channel: typeof DesktopThreadEventSearchChannel,
  request: DesktopThreadEventSearchRequest,
) => Promise<unknown>;

/**
 * Sandboxed renderer-to-main invocation boundary for the bounded, rebuildable
 * projection. It crosses no receipt, artifact, event-body, path, or native
 * error authority.
 */
export const invokeDesktopThreadEventSearch = async (
  invoke: DesktopThreadEventSearchInvoker,
  input: unknown,
): Promise<DesktopThreadEventSearchResult> => {
  const request = decodeDesktopThreadEventSearchRequest(input);
  if (request === null) return { status: "unavailable", reason: "invalid_request" };
  try {
    return (
      decodeDesktopThreadEventSearchResult(
        await invoke(DesktopThreadEventSearchChannel, request),
      ) ?? unavailableDesktopThreadEventSearchResult()
    );
  } catch {
    return unavailableDesktopThreadEventSearchResult();
  }
};

// Compile-time proof that the bridge projection stays assignable to the shared contract.
type BridgeProjection = Extract<
  DesktopThreadEventSearchResult,
  { readonly status: "available" }
>["projection"];
const projectionTypeProof = (value: BridgeProjection): ThreadEventSearchProjection => value;
void projectionTypeProof;
