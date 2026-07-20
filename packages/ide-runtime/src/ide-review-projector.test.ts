import { describe, expect, test } from "vite-plus/test";
import { Result } from "effect";

import {
  MAX_IDE_REVIEW_INPUT_FACTS,
  compileIdeReviewProjection,
  type IdeReviewProjectorFailureReason,
} from "./ide-review-projector.js";

const treeFact = (index = 0) => ({
  _tag: "TreeNode",
  nodeRef: `node.${index}`,
  displayName: `file-${index}.ts`,
  nodeKind: "file",
  availability: "ready",
});

const request = () => ({
  projectionRef: "projection.ide.review.2",
  access: {
    _tag: "Owner",
    authenticated: true,
    ownerRef: "owner.1",
    actorRef: "owner.1",
  },
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
  facts: [treeFact()],
  upstreamOmittedCount: 0,
  sourceSequence: 10,
  lastContiguousSequence: 9,
  observedAt: "2026-07-20T18:00:00.000Z",
  asOf: "2026-07-20T18:00:04.000Z",
  expiresInSeconds: 60,
});

const success = (input: unknown) => {
  const result = compileIdeReviewProjection(input);
  if (Result.isFailure(result)) {
    throw new Error(`expected success, received ${result.failure.reason}`);
  }
  return result.success;
};

const failureReason = (input: unknown): IdeReviewProjectorFailureReason => {
  const result = compileIdeReviewProjection(input);
  if (Result.isSuccess(result)) {
    throw new Error("expected failure");
  }
  return result.failure.reason;
};

describe("compileIdeReviewProjection", () => {
  test("compiles an authenticated owner projection with deterministic live freshness and expiry", () => {
    const projection = success(request());

    expect(projection.audience).toBe("owner_authenticated");
    expect(projection.audienceScopeRef).toBe("owner.1");
    expect(projection.freshness.state).toBe("live");
    expect(projection.expiresAt).toBe("2026-07-20T18:01:04.000Z");
    expect(projection.items).toHaveLength(1);
    expect(projection.truncated).toBe(false);
    expect(projection.omittedCount).toBe(0);
  });

  test("admits only an authenticated actor in the exact named audience", () => {
    const named = {
      ...request(),
      access: {
        _tag: "NamedAudience",
        authenticated: true,
        ownerRef: "owner.1",
        actorRef: "reviewer.2",
        audienceScopeRef: "audience.team-alpha.1",
        audienceActorRefs: ["reviewer.1", "reviewer.2"],
      },
    };
    const namedProjection = success(named);
    expect(namedProjection.audience).toBe("named_audience_authenticated");
    expect(namedProjection.audienceScopeRef).toBe("audience.team-alpha.1");

    expect(
      failureReason({
        ...named,
        access: { ...named.access, authenticated: false },
      }),
    ).toBe("unauthenticated");
    expect(
      failureReason({
        ...named,
        access: { ...named.access, actorRef: "reviewer.3" },
      }),
    ).toBe("audience_denied");
    expect(
      failureReason({
        ...request(),
        access: { ...request().access, actorRef: "owner.2" },
      }),
    ).toBe("audience_denied");
  });

  test("derives cached, stale, and exact gap freshness from observed sequence facts", () => {
    expect(
      success({
        ...request(),
        asOf: "2026-07-20T18:00:20.000Z",
      }).freshness.state,
    ).toBe("cached");
    expect(
      success({
        ...request(),
        asOf: "2026-07-20T18:00:31.000Z",
      }).freshness.state,
    ).toBe("stale");

    const gap = success({
      ...request(),
      sourceSequence: 12,
      lastContiguousSequence: 9,
    });
    expect(gap.freshness).toEqual({
      state: "gap",
      observedAt: "2026-07-20T18:00:00.000Z",
      sourceSequence: 12,
      gapAfterSequence: 9,
    });
  });

  test("rejects replayed and materially future observations", () => {
    expect(
      failureReason({
        ...request(),
        sourceSequence: 9,
        lastContiguousSequence: 9,
      }),
    ).toBe("replayed_sequence");
    expect(
      failureReason({
        ...request(),
        observedAt: "2026-07-20T18:00:10.000Z",
      }),
    ).toBe("future_observation");
  });

  test("decodes every fact before deterministic truncation and preserves omission truth", () => {
    const facts: Array<Record<string, unknown>> = Array.from({ length: 205 }, (_, index) =>
      treeFact(index),
    );
    const projection = success({
      ...request(),
      facts,
      upstreamOmittedCount: 3,
      nextCursorRef: "cursor.ide.review.2",
    });

    expect(projection.items).toHaveLength(200);
    expect(projection.omittedCount).toBe(8);
    expect(projection.truncated).toBe(true);
    expect(projection.nextCursorRef).toBe("cursor.ide.review.2");

    facts[204] = { ...treeFact(204), rootPath: "/Users/owner/project" };
    expect(
      failureReason({
        ...request(),
        facts,
        upstreamOmittedCount: 3,
        nextCursorRef: "cursor.ide.review.2",
      }),
    ).toBe("forbidden_material");
  });

  test("requires cursor and omission truth to agree", () => {
    expect(
      failureReason({
        ...request(),
        upstreamOmittedCount: 1,
      }),
    ).toBe("invalid_request");
    expect(
      failureReason({
        ...request(),
        nextCursorRef: "cursor.unneeded.1",
      }),
    ).toBe("invalid_request");
  });

  test.each([
    { ...request(), rootPath: "/Users/owner/project" },
    { ...request(), credential: "sk-live-0123456789abcdef" },
    { ...request(), credential: "ghp_0123456789abcdef0123456789abcdef" },
    {
      ...request(),
      facts: [{ ...treeFact(), displayName: "token=secret-value" }],
    },
    {
      ...request(),
      facts: [{ ...treeFact(), nativeHandle: "pty:7" }],
    },
  ])("refuses host, credential, or native material before output", (input) => {
    expect(failureReason(input)).toBe("forbidden_material");
  });

  test("rejects malformed facts, request excess fields, input overflow, and excess expiry", () => {
    expect(
      failureReason({
        ...request(),
        facts: [{ ...treeFact(), unexpected: true }],
      }),
    ).toBe("invalid_fact");
    expect(failureReason({ ...request(), unexpected: true })).toBe("invalid_request");
    expect(
      failureReason({
        ...request(),
        facts: Array.from({ length: MAX_IDE_REVIEW_INPUT_FACTS + 1 }, (_, index) =>
          treeFact(index),
        ),
      }),
    ).toBe("invalid_request");
    expect(
      failureReason({
        ...request(),
        expiresInSeconds: 301,
      }),
    ).toBe("invalid_request");
  });
});
