import { describe, expect, test } from "vite-plus/test";
import { Result } from "effect";

import {
  IdeReviewClientOutcome,
  MAX_IDE_REVIEW_CACHE_ROWS,
  MAX_IDE_REVIEW_CACHE_STREAMS,
  authenticateIdeReviewClientCache,
  emptyIdeReviewClientCacheState,
  ingestIdeReviewClientProjection,
  logoutIdeReviewClientCache,
  readIdeReviewClientCache,
  type IdeReviewClientCacheState,
  type IdeReviewClientOutcome as ClientOutcome,
  type IdeReviewClientRejectionReason,
} from "./ide-review-client-cache.js";
import { compileIdeReviewProjection } from "./ide-review-projector.js";
import type { IdeReviewProjection } from "./ide-review-projection.js";

const OWNER_SCOPE = {
  _tag: "Owner",
  authenticated: true,
  ownerRef: "owner.1",
  actorRef: "owner.1",
} as const;

const projectionRequest = (overrides: Record<string, unknown> = {}) => ({
  projectionRef: "projection.ide.cache.1",
  access: OWNER_SCOPE,
  source: {
    sessionRef: "session.1",
    projectRef: "project.1",
    worktreeRef: "worktree.1",
    attachmentRef: "attachment.1",
    placementRef: "placement.owner-local.1",
    attachmentGeneration: 2,
    projectGeneration: 3,
    serviceGeneration: 4,
    evidenceGeneration: 5,
  },
  availability: "ready",
  facts: [
    {
      _tag: "TreeNode",
      nodeRef: "node.1",
      displayName: "index.ts",
      nodeKind: "file",
      availability: "ready",
    },
  ],
  upstreamOmittedCount: 0,
  sourceSequence: 1,
  observedAt: "2026-07-20T19:00:00.000Z",
  asOf: "2026-07-20T19:00:04.000Z",
  expiresInSeconds: 60,
  ...overrides,
});

const projection = (overrides: Record<string, unknown> = {}): IdeReviewProjection => {
  const result = compileIdeReviewProjection(projectionRequest(overrides));
  if (Result.isFailure(result)) {
    throw new Error(`projection fixture failed: ${result.failure.reason}`);
  }
  return result.success;
};

const authenticatedState = (scope: unknown = OWNER_SCOPE): IdeReviewClientCacheState => {
  const transition = authenticateIdeReviewClientCache(emptyIdeReviewClientCacheState(), scope);
  if (!IdeReviewClientOutcome.guards.Authenticated(transition.outcome)) {
    throw new Error("scope fixture did not authenticate");
  }
  return transition.state;
};

const accepted = (outcome: ClientOutcome) => {
  if (!IdeReviewClientOutcome.guards.Accepted(outcome)) {
    throw new Error("expected accepted outcome");
  }
  return outcome;
};

const rejectedReason = (outcome: ClientOutcome): IdeReviewClientRejectionReason => {
  if (!IdeReviewClientOutcome.guards.Rejected(outcome)) {
    throw new Error("expected rejected outcome");
  }
  return outcome.reason;
};

const ingest = (
  state: IdeReviewClientCacheState,
  payload: unknown,
  asOf = "2026-07-20T19:00:04.000Z",
) => ingestIdeReviewClientProjection(state, payload, asOf);

describe("IdeReviewClientCache", () => {
  test("accepts only the exact authenticated owner or named-audience scope", () => {
    const ownerState = authenticatedState();
    expect(accepted(ingest(ownerState, projection()).outcome).health).toBe("ready");

    const wrongOwner = authenticatedState({
      ...OWNER_SCOPE,
      ownerRef: "owner.2",
      actorRef: "owner.2",
    });
    expect(rejectedReason(ingest(wrongOwner, projection()).outcome)).toBe("scope_mismatch");

    const namedScope = {
      _tag: "NamedAudience",
      authenticated: true,
      ownerRef: "owner.1",
      actorRef: "reviewer.1",
      audienceScopeRef: "audience.team.1",
    } as const;
    const namedProjection = projection({
      access: {
        _tag: "NamedAudience",
        authenticated: true,
        ownerRef: "owner.1",
        actorRef: "reviewer.1",
        audienceScopeRef: "audience.team.1",
        audienceActorRefs: ["reviewer.1"],
      },
    });
    expect(accepted(ingest(authenticatedState(namedScope), namedProjection).outcome).health).toBe(
      "ready",
    );
    expect(
      rejectedReason(
        ingest(
          authenticatedState({ ...namedScope, audienceScopeRef: "audience.team.2" }),
          namedProjection,
        ).outcome,
      ),
    ).toBe("scope_mismatch");
    expect(
      rejectedReason(
        ingest(authenticatedState({ ...namedScope, ownerRef: "owner.2" }), namedProjection).outcome,
      ),
    ).toBe("scope_mismatch");
  });

  test("rejects logged-out ingestion and invalid owner authentication", () => {
    expect(rejectedReason(ingest(emptyIdeReviewClientCacheState(), projection()).outcome)).toBe(
      "logged_out",
    );
    const invalid = authenticateIdeReviewClientCache(emptyIdeReviewClientCacheState(), {
      ...OWNER_SCOPE,
      actorRef: "owner.2",
    });
    expect(rejectedReason(invalid.outcome)).toBe("invalid_scope");
  });

  test("rejects replay and retains declared or independently detected gaps as degraded", () => {
    const first = ingest(authenticatedState(), projection());
    expect(accepted(first.outcome).health).toBe("ready");
    expect(rejectedReason(ingest(first.state, projection()).outcome)).toBe("replayed_sequence");

    const declaredGap = projection({ sourceSequence: 3, lastContiguousSequence: 1 });
    const gapTransition = ingest(first.state, declaredGap);
    expect(accepted(gapTransition.outcome).degradedReason).toBe("gap");

    const independentGap = projection({
      projectionRef: "projection.ide.cache.4",
      sourceSequence: 4,
    });
    expect(accepted(ingest(first.state, independentGap).outcome).degradedReason).toBe(
      "sequence_gap",
    );

    const invalidGap = {
      ...declaredGap,
      freshness: { ...declaredGap.freshness, gapAfterSequence: 0 },
    };
    expect(rejectedReason(ingest(first.state, invalidGap).outcome)).toBe("invalid_gap");
  });

  test("enforces declared freshness and refreshes cached entries as time advances", () => {
    const first = ingest(authenticatedState(), projection());
    expect(accepted(first.outcome).health).toBe("ready");

    const cached = readIdeReviewClientCache(first.state, "2026-07-20T19:00:20.000Z");
    expect(cached.entries[0]?.degradedReason).toBe("cached");
    const stale = readIdeReviewClientCache(first.state, "2026-07-20T19:00:31.000Z");
    expect(stale.entries[0]?.degradedReason).toBe("stale");

    const mismatched = {
      ...projection(),
      freshness: { ...projection().freshness, state: "cached" },
    };
    expect(rejectedReason(ingest(authenticatedState(), mismatched).outcome)).toBe(
      "freshness_mismatch",
    );
  });

  test("rejects future and expired projections and removes expired cached rows", () => {
    const future = projection({
      observedAt: "2026-07-20T19:00:08.000Z",
      asOf: "2026-07-20T19:00:08.000Z",
    });
    expect(
      rejectedReason(ingest(authenticatedState(), future, "2026-07-20T19:00:00.000Z").outcome),
    ).toBe("future_projection");

    const first = ingest(authenticatedState(), projection());
    const expired = ingest(
      first.state,
      projection({ sourceSequence: 2 }),
      "2026-07-20T19:01:05.000Z",
    );
    expect(rejectedReason(expired.outcome)).toBe("expired");
    expect(expired.state.entries).toHaveLength(0);
  });

  test("clears matching data on revoke, all data on scope change, and all data on logout", () => {
    const first = ingest(authenticatedState(), projection());
    const revoked = projection({
      projectionRef: "projection.ide.cache.revoked",
      sourceSequence: 2,
      availability: "revoked",
    });
    const revokeTransition = ingest(first.state, revoked);
    expect(IdeReviewClientOutcome.guards.Cleared(revokeTransition.outcome)).toBe(true);
    expect(revokeTransition.state.entries).toHaveLength(0);

    const restored = ingest(revokeTransition.state, projection({ sourceSequence: 3 }));
    const changed = authenticateIdeReviewClientCache(restored.state, {
      ...OWNER_SCOPE,
      ownerRef: "owner.2",
      actorRef: "owner.2",
    });
    expect(changed.state.entries).toHaveLength(0);
    expect(IdeReviewClientOutcome.guards.Authenticated(changed.outcome)).toBe(true);

    const loggedOut = logoutIdeReviewClientCache(restored.state);
    expect(loggedOut.state.scope).toBeUndefined();
    expect(loggedOut.state.entries).toHaveLength(0);
  });

  test("caps cached streams and rows with deterministic oldest-entry eviction", () => {
    let state = authenticatedState();
    for (let index = 0; index < MAX_IDE_REVIEW_CACHE_STREAMS + 1; index += 1) {
      const next = ingest(
        state,
        projection({
          projectionRef: `projection.stream.${index}`,
          source: {
            ...projectionRequest().source,
            sessionRef: `session.${index}`,
          },
        }),
      );
      state = next.state;
    }
    expect(state.entries).toHaveLength(MAX_IDE_REVIEW_CACHE_STREAMS);
    expect(state.entries.some((entry) => entry.projection.source.sessionRef === "session.0")).toBe(
      false,
    );

    state = authenticatedState();
    const facts = Array.from({ length: 200 }, (_, index) => ({
      _tag: "TreeNode",
      nodeRef: `node.${index}`,
      displayName: `file-${index}.ts`,
      nodeKind: "file",
      availability: "ready",
    }));
    for (let index = 0; index < 9; index += 1) {
      state = ingest(
        state,
        projection({
          projectionRef: `projection.rows.${index}`,
          source: { ...projectionRequest().source, sessionRef: `row-session.${index}` },
          facts,
        }),
      ).state;
    }
    expect(state.entries.reduce((total, entry) => total + entry.projection.items.length, 0)).toBe(
      MAX_IDE_REVIEW_CACHE_ROWS,
    );
    expect(state.entries).toHaveLength(8);
  });

  test.each([
    { ...projection(), hostPath: "/Users/owner/project" },
    { ...projection(), credential: "ghp_0123456789abcdef0123456789abcdef" },
    { ...projection(), nativeHandle: "pty:8" },
  ])("never admits forbidden host, credential, or native material", (payload) => {
    const transition = ingest(authenticatedState(), payload);
    expect(rejectedReason(transition.outcome)).toBe("forbidden_material");
    expect(transition.state.entries).toHaveLength(0);
  });

  test("strictly rejects excess projection fields without retaining them", () => {
    const transition = ingest(authenticatedState(), { ...projection(), unexpected: true });
    expect(rejectedReason(transition.outcome)).toBe("invalid_projection");
    expect(transition.state.entries).toHaveLength(0);
  });
});
