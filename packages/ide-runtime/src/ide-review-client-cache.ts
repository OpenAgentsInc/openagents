import { Context, Effect, Layer, Ref, Result, Schema as S } from "effect";

import {
  IDE_REVIEW_CACHED_AGE_MILLIS,
  IDE_REVIEW_FUTURE_SKEW_MILLIS,
  IDE_REVIEW_LIVE_AGE_MILLIS,
} from "./ide-review-projector.js";
import {
  IdeProjectionRef,
  IdeProjectionTimestamp,
  IdeReviewProjection,
  hasForbiddenIdeProjectionMaterial,
} from "./ide-review-projection.js";

export const MAX_IDE_REVIEW_CACHE_STREAMS = 16 as const;
export const MAX_IDE_REVIEW_CACHE_ROWS = 1_600 as const;

export const IdeReviewClientScope = S.TaggedUnion({
  Owner: {
    authenticated: S.Literal(true),
    ownerRef: IdeProjectionRef,
    actorRef: IdeProjectionRef,
  },
  NamedAudience: {
    authenticated: S.Literal(true),
    ownerRef: IdeProjectionRef,
    actorRef: IdeProjectionRef,
    audienceScopeRef: IdeProjectionRef,
  },
}).annotate({ identifier: "IdeReviewClientScope" });
export type IdeReviewClientScope = typeof IdeReviewClientScope.Type;

export const IdeReviewClientDegradedReason = S.Literals([
  "cached",
  "stale",
  "gap",
  "sequence_gap",
  "source_loading",
  "source_degraded",
  "source_unavailable",
  "source_redacted",
]);
export type IdeReviewClientDegradedReason = typeof IdeReviewClientDegradedReason.Type;

export const IdeReviewClientRejectionReason = S.Literals([
  "logged_out",
  "invalid_scope",
  "scope_mismatch",
  "forbidden_material",
  "invalid_projection",
  "invalid_time",
  "expired",
  "future_projection",
  "freshness_mismatch",
  "replayed_sequence",
  "invalid_gap",
]);
export type IdeReviewClientRejectionReason = typeof IdeReviewClientRejectionReason.Type;

export const IdeReviewClientOutcome = S.TaggedUnion({
  Authenticated: {
    scopeChanged: S.Boolean,
    clearedEntries: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  },
  Accepted: {
    health: S.Literals(["ready", "degraded"]),
    degradedReason: S.optionalKey(IdeReviewClientDegradedReason),
    projection: IdeReviewProjection,
  },
  Cleared: {
    reason: S.Literals(["revoked", "scope_change", "logout", "expired"]),
    clearedEntries: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  },
  Rejected: {
    reason: IdeReviewClientRejectionReason,
  },
}).annotate({ identifier: "IdeReviewClientOutcome" });
export type IdeReviewClientOutcome = typeof IdeReviewClientOutcome.Type;

export interface IdeReviewClientCacheEntry {
  readonly streamKey: string;
  readonly projection: IdeReviewProjection;
  readonly health: "ready" | "degraded";
  readonly degradedReason: IdeReviewClientDegradedReason | undefined;
  readonly acceptedOrdinal: number;
}

export interface IdeReviewClientCacheState {
  readonly scope: IdeReviewClientScope | undefined;
  readonly entries: ReadonlyArray<IdeReviewClientCacheEntry>;
  readonly nextOrdinal: number;
}

export const emptyIdeReviewClientCacheState = (): IdeReviewClientCacheState => ({
  scope: undefined,
  entries: [],
  nextOrdinal: 1,
});

export interface IdeReviewClientTransition {
  readonly state: IdeReviewClientCacheState;
  readonly outcome: IdeReviewClientOutcome;
}

const strictOptions = { onExcessProperty: "error" as const };
const decodeScope = S.decodeUnknownResult(IdeReviewClientScope);
const decodeProjection = S.decodeUnknownResult(IdeReviewProjection);

const scopeMatches = (left: IdeReviewClientScope, right: IdeReviewClientScope): boolean => {
  if (IdeReviewClientScope.guards.Owner(left)) {
    return (
      IdeReviewClientScope.guards.Owner(right) &&
      left.ownerRef === right.ownerRef &&
      left.actorRef === right.actorRef
    );
  }
  return (
    IdeReviewClientScope.guards.NamedAudience(right) &&
    left.ownerRef === right.ownerRef &&
    left.actorRef === right.actorRef &&
    left.audienceScopeRef === right.audienceScopeRef
  );
};

const scopeAdmitsProjection = (
  scope: IdeReviewClientScope,
  projection: IdeReviewProjection,
): boolean => {
  if (IdeReviewClientScope.guards.Owner(scope)) {
    return (
      scope.actorRef === scope.ownerRef &&
      projection.audience === "owner_authenticated" &&
      projection.ownerScopeRef === scope.ownerRef &&
      projection.audienceScopeRef === scope.ownerRef
    );
  }
  return (
    projection.audience === "named_audience_authenticated" &&
    projection.ownerScopeRef === scope.ownerRef &&
    projection.audienceScopeRef === scope.audienceScopeRef
  );
};

const projectionStreamKey = (projection: IdeReviewProjection): string =>
  JSON.stringify([
    projection.audienceScopeRef,
    projection.source.sessionRef,
    projection.source.projectRef,
    projection.source.worktreeRef,
    projection.source.attachmentRef,
  ]);

const sourceMatches = (left: IdeReviewProjection, right: IdeReviewProjection): boolean =>
  projectionStreamKey(left) === projectionStreamKey(right);

const sourceSequence = (projection: IdeReviewProjection): number =>
  projection.freshness.sourceSequence;

const derivedTimeFreshness = (
  observedAt: string,
  asOfMillis: number,
): "live" | "cached" | "stale" | "future" => {
  const ageMillis = asOfMillis - Date.parse(observedAt);
  if (ageMillis < -IDE_REVIEW_FUTURE_SKEW_MILLIS) return "future";
  if (ageMillis <= IDE_REVIEW_LIVE_AGE_MILLIS) return "live";
  if (ageMillis <= IDE_REVIEW_CACHED_AGE_MILLIS) return "cached";
  return "stale";
};

const healthFor = (
  projection: IdeReviewProjection,
  asOfMillis: number,
  sequenceGap: boolean,
): Pick<IdeReviewClientCacheEntry, "health" | "degradedReason"> => {
  if (projection.freshness.state === "gap") {
    return { health: "degraded", degradedReason: "gap" };
  }
  if (sequenceGap) return { health: "degraded", degradedReason: "sequence_gap" };
  switch (projection.availability) {
    case "loading":
      return { health: "degraded", degradedReason: "source_loading" };
    case "degraded":
      return { health: "degraded", degradedReason: "source_degraded" };
    case "unavailable":
      return { health: "degraded", degradedReason: "source_unavailable" };
    case "redacted":
      return { health: "degraded", degradedReason: "source_redacted" };
    case "revoked":
      return { health: "degraded", degradedReason: "source_unavailable" };
    case "ready":
      break;
  }
  const freshness = derivedTimeFreshness(projection.freshness.observedAt, asOfMillis);
  if (freshness === "cached") return { health: "degraded", degradedReason: "cached" };
  if (freshness === "stale") return { health: "degraded", degradedReason: "stale" };
  return { health: "ready", degradedReason: undefined };
};

const totalRows = (entries: ReadonlyArray<IdeReviewClientCacheEntry>): number =>
  entries.reduce((total, entry) => total + entry.projection.items.length, 0);

const boundedEntries = (
  entries: ReadonlyArray<IdeReviewClientCacheEntry>,
): ReadonlyArray<IdeReviewClientCacheEntry> => {
  const kept = [...entries];
  while (
    kept.length > MAX_IDE_REVIEW_CACHE_STREAMS ||
    totalRows(kept) > MAX_IDE_REVIEW_CACHE_ROWS
  ) {
    kept.shift();
  }
  return kept;
};

const refreshAndPrune = (
  state: IdeReviewClientCacheState,
  asOfMillis: number,
): IdeReviewClientCacheState => ({
  ...state,
  entries: state.entries
    .filter((entry) => Date.parse(entry.projection.expiresAt) > asOfMillis)
    .map((entry) => {
      const health = healthFor(
        entry.projection,
        asOfMillis,
        entry.degradedReason === "sequence_gap",
      );
      return {
        streamKey: entry.streamKey,
        projection: entry.projection,
        health: health.health,
        degradedReason: health.degradedReason,
        acceptedOrdinal: entry.acceptedOrdinal,
      };
    }),
});

export const authenticateIdeReviewClientCache = (
  state: IdeReviewClientCacheState,
  input: unknown,
): IdeReviewClientTransition => {
  if (hasForbiddenIdeProjectionMaterial(input)) {
    return {
      state,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "invalid_scope" }),
    };
  }
  const decoded = decodeScope(input, strictOptions);
  if (Result.isFailure(decoded)) {
    return {
      state,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "invalid_scope" }),
    };
  }
  const scope = decoded.success;
  if (IdeReviewClientScope.guards.Owner(scope) && scope.actorRef !== scope.ownerRef) {
    return {
      state,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "invalid_scope" }),
    };
  }
  const changed = state.scope === undefined || !scopeMatches(state.scope, scope);
  const clearedEntries = changed ? state.entries.length : 0;
  return {
    state: {
      scope,
      entries: changed ? [] : state.entries,
      nextOrdinal: state.nextOrdinal,
    },
    outcome: IdeReviewClientOutcome.cases.Authenticated.make({
      scopeChanged: changed,
      clearedEntries,
    }),
  };
};

export const ingestIdeReviewClientProjection = (
  state: IdeReviewClientCacheState,
  payload: unknown,
  asOf: IdeProjectionTimestamp,
): IdeReviewClientTransition => {
  if (state.scope === undefined) {
    return { state, outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "logged_out" }) };
  }
  if (hasForbiddenIdeProjectionMaterial(payload)) {
    return {
      state,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "forbidden_material" }),
    };
  }
  const asOfMillis = Date.parse(asOf);
  if (!Number.isFinite(asOfMillis)) {
    return {
      state,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "invalid_time" }),
    };
  }
  const decoded = decodeProjection(payload, strictOptions);
  if (Result.isFailure(decoded)) {
    return {
      state: refreshAndPrune(state, asOfMillis),
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "invalid_projection" }),
    };
  }
  const projection = decoded.success;
  const refreshed = refreshAndPrune(state, asOfMillis);
  if (!scopeAdmitsProjection(state.scope, projection)) {
    return {
      state: refreshed,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "scope_mismatch" }),
    };
  }
  if (projection.availability === "revoked") {
    const entries = refreshed.entries.filter(
      (entry) => !sourceMatches(entry.projection, projection),
    );
    return {
      state: { ...refreshed, entries },
      outcome: IdeReviewClientOutcome.cases.Cleared.make({
        reason: "revoked",
        clearedEntries: refreshed.entries.length - entries.length,
      }),
    };
  }
  if (Date.parse(projection.expiresAt) <= asOfMillis) {
    const entries = refreshed.entries.filter(
      (entry) => !sourceMatches(entry.projection, projection),
    );
    return {
      state: { ...refreshed, entries },
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "expired" }),
    };
  }
  if (Date.parse(projection.generatedAt) - asOfMillis > IDE_REVIEW_FUTURE_SKEW_MILLIS) {
    return {
      state: refreshed,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "future_projection" }),
    };
  }
  const derivedFreshness = derivedTimeFreshness(projection.freshness.observedAt, asOfMillis);
  if (derivedFreshness === "future") {
    return {
      state: refreshed,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "future_projection" }),
    };
  }
  if (projection.freshness.state !== "gap" && projection.freshness.state !== derivedFreshness) {
    return {
      state: refreshed,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "freshness_mismatch" }),
    };
  }

  const key = projectionStreamKey(projection);
  const previous = refreshed.entries.find((entry) => entry.streamKey === key);
  const previousSequence = previous === undefined ? undefined : sourceSequence(previous.projection);
  if (previousSequence !== undefined && sourceSequence(projection) <= previousSequence) {
    return {
      state: refreshed,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "replayed_sequence" }),
    };
  }
  if (
    previousSequence !== undefined &&
    projection.freshness.state === "gap" &&
    projection.freshness.gapAfterSequence !== previousSequence
  ) {
    return {
      state: refreshed,
      outcome: IdeReviewClientOutcome.cases.Rejected.make({ reason: "invalid_gap" }),
    };
  }
  const sequenceGap =
    previousSequence !== undefined && sourceSequence(projection) > previousSequence + 1;
  const health = healthFor(projection, asOfMillis, sequenceGap);
  const entry: IdeReviewClientCacheEntry = {
    streamKey: key,
    projection,
    ...health,
    acceptedOrdinal: refreshed.nextOrdinal,
  };
  const entries = boundedEntries([
    ...refreshed.entries.filter((candidate) => candidate.streamKey !== key),
    entry,
  ]);
  return {
    state: { ...refreshed, entries, nextOrdinal: refreshed.nextOrdinal + 1 },
    outcome: IdeReviewClientOutcome.cases.Accepted.make({
      health: entry.health,
      ...(entry.degradedReason === undefined ? {} : { degradedReason: entry.degradedReason }),
      projection,
    }),
  };
};

export const logoutIdeReviewClientCache = (
  state: IdeReviewClientCacheState,
): IdeReviewClientTransition => ({
  state: emptyIdeReviewClientCacheState(),
  outcome: IdeReviewClientOutcome.cases.Cleared.make({
    reason: "logout",
    clearedEntries: state.entries.length,
  }),
});

export const readIdeReviewClientCache = (
  state: IdeReviewClientCacheState,
  asOf: IdeProjectionTimestamp,
): IdeReviewClientCacheState => {
  const asOfMillis = Date.parse(asOf);
  return Number.isFinite(asOfMillis) ? refreshAndPrune(state, asOfMillis) : state;
};

export interface IdeReviewClientCacheInterface {
  readonly authenticate: (scope: unknown) => Effect.Effect<IdeReviewClientOutcome>;
  readonly ingest: (
    payload: unknown,
    asOf: IdeProjectionTimestamp,
  ) => Effect.Effect<IdeReviewClientOutcome>;
  readonly snapshot: (asOf: IdeProjectionTimestamp) => Effect.Effect<IdeReviewClientCacheState>;
  readonly logout: () => Effect.Effect<IdeReviewClientOutcome>;
}

export class IdeReviewClientCache extends Context.Service<
  IdeReviewClientCache,
  IdeReviewClientCacheInterface
>()("ide-runtime.IdeReviewClientCache") {}

export const IdeReviewClientCacheLayer = Layer.effect(
  IdeReviewClientCache,
  Effect.gen(function* () {
    const state = yield* Ref.make(emptyIdeReviewClientCacheState());

    const authenticate = Effect.fn("IdeReviewClientCache.authenticate")((scope: unknown) =>
      Ref.modify(state, (current) => {
        const transition = authenticateIdeReviewClientCache(current, scope);
        return [transition.outcome, transition.state] as const;
      }),
    );
    const ingest = Effect.fn("IdeReviewClientCache.ingest")(
      (payload: unknown, asOf: IdeProjectionTimestamp) =>
        Ref.modify(state, (current) => {
          const transition = ingestIdeReviewClientProjection(current, payload, asOf);
          return [transition.outcome, transition.state] as const;
        }),
    );
    const snapshot = Effect.fn("IdeReviewClientCache.snapshot")((asOf: IdeProjectionTimestamp) =>
      Ref.updateAndGet(state, (current) => readIdeReviewClientCache(current, asOf)),
    );
    const logout = Effect.fn("IdeReviewClientCache.logout")(() =>
      Ref.modify(state, (current) => {
        const transition = logoutIdeReviewClientCache(current);
        return [transition.outcome, transition.state] as const;
      }),
    );

    return IdeReviewClientCache.of({ authenticate, ingest, snapshot, logout });
  }),
);
