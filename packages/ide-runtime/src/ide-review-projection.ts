import { Schema as S } from "effect";

import { ProjectRef, WorktreeRef } from "@openagentsinc/agent-runtime-schema";

export const IDE_REVIEW_PROJECTION_SCHEMA_LITERAL = "openagents.ide_review_projection.v1" as const;

export const MAX_IDE_REVIEW_ITEMS = 200 as const;
export const MAX_IDE_REVIEW_EXCERPT_CHARS = 16_384 as const;
export const MAX_IDE_REVIEW_LABEL_CHARS = 160 as const;
export const MAX_IDE_REVIEW_DETAIL_CHARS = 1_024 as const;

const forbiddenMaterial = [
  /(?:^|[\s"'(=])\/(?:Users|home|root|private|var|etc|opt|tmp|workspace|mnt|srv|data|run)\//i,
  /(?:^|[\s"'(=])[a-z]:\\/i,
  /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/i,
  /\bAKIA[0-9A-Z]{16}\b/,
  /\b(?:ghp_|github_pat_|sk-)[A-Za-z0-9_-]{16,}\b/i,
  /\b(?:api[_-]?key|password|passwd|secret|token)\s*[:=]\s*\S+/i,
] as const;

const containsForbiddenText = (value: string): boolean =>
  forbiddenMaterial.some((pattern) => pattern.test(value));

const forbiddenFieldName =
  /^(?:root|rootPath|hostPath|absolutePath|environment|env|credential|credentials|password|secret|token|bearerToken|rawTerminal|terminal|processId|nativeHandle)$/i;

/** Find forbidden host or credential material without returning the material. */
export const hasForbiddenIdeProjectionMaterial = (value: unknown): boolean => {
  const seen = new WeakSet<object>();

  const visit = (candidate: unknown): boolean => {
    if (typeof candidate === "string") {
      return containsForbiddenText(candidate);
    }
    if (typeof candidate !== "object" || candidate === null) {
      return false;
    }
    if (seen.has(candidate)) {
      return false;
    }
    seen.add(candidate);
    if (Array.isArray(candidate)) {
      return candidate.some(visit);
    }
    return Object.entries(candidate).some(
      ([key, field]) => forbiddenFieldName.test(key) || visit(field),
    );
  };

  return visit(value);
};

const boundedSafeText = (maximum: number) =>
  S.String.check(
    S.isMaxLength(maximum),
    S.makeFilter((value) => !containsForbiddenText(value), {
      message: "text must not contain a host path or credential material",
    }),
  );

/** A stable opaque reference. It cannot contain a path separator or bearer syntax. */
export const IdeProjectionRef = S.String.check(
  S.isMinLength(3),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
  S.makeFilter((value) => !containsForbiddenText(value), {
    message: "reference must not contain credential material",
  }),
).pipe(S.brand("IdeProjectionRef"));
export type IdeProjectionRef = typeof IdeProjectionRef.Type;

export const IdeProjectionTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/),
);
export type IdeProjectionTimestamp = typeof IdeProjectionTimestamp.Type;

export const IdeProjectionGeneration = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));
export type IdeProjectionGeneration = typeof IdeProjectionGeneration.Type;

export const IdeReviewAudience = S.Literals([
  "owner_authenticated",
  "named_audience_authenticated",
]);
export type IdeReviewAudience = typeof IdeReviewAudience.Type;

export const IdeReviewAvailability = S.Literals([
  "ready",
  "loading",
  "degraded",
  "unavailable",
  "redacted",
  "revoked",
]);
export type IdeReviewAvailability = typeof IdeReviewAvailability.Type;

const FreshnessSequence = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));

export const IdeReviewFreshness = S.Union([
  S.Struct({
    state: S.Literal("live"),
    observedAt: IdeProjectionTimestamp,
    sourceSequence: FreshnessSequence,
  }),
  S.Struct({
    state: S.Literal("cached"),
    observedAt: IdeProjectionTimestamp,
    sourceSequence: FreshnessSequence,
  }),
  S.Struct({
    state: S.Literal("stale"),
    observedAt: IdeProjectionTimestamp,
    sourceSequence: FreshnessSequence,
  }),
  S.Struct({
    state: S.Literal("gap"),
    observedAt: IdeProjectionTimestamp,
    sourceSequence: FreshnessSequence,
    gapAfterSequence: FreshnessSequence,
  }),
]);
export type IdeReviewFreshness = typeof IdeReviewFreshness.Type;

export const IdeReviewSource = S.Struct({
  sessionRef: IdeProjectionRef,
  projectRef: ProjectRef,
  worktreeRef: WorktreeRef,
  attachmentRef: IdeProjectionRef,
  placementRef: IdeProjectionRef,
  attachmentGeneration: IdeProjectionGeneration,
  projectGeneration: IdeProjectionGeneration,
  serviceGeneration: IdeProjectionGeneration,
  evidenceGeneration: IdeProjectionGeneration,
});
export interface IdeReviewSource extends S.Schema.Type<typeof IdeReviewSource> {}

const SafeLabel = boundedSafeText(MAX_IDE_REVIEW_LABEL_CHARS);
const SafeDetail = boundedSafeText(MAX_IDE_REVIEW_DETAIL_CHARS);
const SafeExcerpt = boundedSafeText(MAX_IDE_REVIEW_EXCERPT_CHARS);
const NonNegativeInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));
const PositiveLine = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1));

export const IdeReviewRange = S.Struct({
  rangeRef: IdeProjectionRef,
  startLine: PositiveLine,
  startColumn: PositiveLine,
  endLine: PositiveLine,
  endColumn: PositiveLine,
  documentGeneration: IdeProjectionGeneration,
}).pipe(
  S.check(
    S.makeFilter(
      (range) =>
        range.endLine > range.startLine ||
        (range.endLine === range.startLine && range.endColumn >= range.startColumn),
      { message: "range end must not precede range start" },
    ),
  ),
);
export interface IdeReviewRange extends S.Schema.Type<typeof IdeReviewRange> {}

export const IdeReviewItem = S.TaggedUnion({
  TreeNode: {
    nodeRef: IdeProjectionRef,
    parentNodeRef: S.optionalKey(IdeProjectionRef),
    displayName: SafeLabel,
    nodeKind: S.Literals(["file", "directory"]),
    availability: IdeReviewAvailability,
  },
  Excerpt: {
    excerptRef: IdeProjectionRef,
    documentRef: IdeProjectionRef,
    range: IdeReviewRange,
    languageRef: IdeProjectionRef,
    text: SafeExcerpt,
    truncated: S.Boolean,
  },
  SearchHit: {
    resultRef: IdeProjectionRef,
    documentRef: IdeProjectionRef,
    range: IdeReviewRange,
    preview: SafeDetail,
  },
  Symbol: {
    symbolRef: IdeProjectionRef,
    documentRef: IdeProjectionRef,
    range: IdeReviewRange,
    name: SafeLabel,
    symbolKind: SafeLabel,
  },
  Problem: {
    problemRef: IdeProjectionRef,
    documentRef: IdeProjectionRef,
    range: IdeReviewRange,
    severity: S.Literals(["error", "warning", "information", "hint"]),
    codeRef: S.optionalKey(IdeProjectionRef),
    detail: SafeDetail,
  },
  Change: {
    changeRef: IdeProjectionRef,
    documentRef: IdeProjectionRef,
    documentGeneration: IdeProjectionGeneration,
    state: S.Literals(["added", "modified", "deleted", "renamed", "conflicted"]),
    additions: NonNegativeInt,
    deletions: NonNegativeInt,
    diffRef: IdeProjectionRef,
  },
  Proposal: {
    proposalRef: IdeProjectionRef,
    baseGeneration: IdeProjectionGeneration,
    status: S.Literals(["pending", "approved", "rejected", "applied", "stale"]),
    changeCount: NonNegativeInt,
    evidenceRef: IdeProjectionRef,
  },
  Test: {
    testRef: IdeProjectionRef,
    status: S.Literals(["queued", "running", "passed", "failed", "skipped"]),
    passedCount: NonNegativeInt,
    failedCount: NonNegativeInt,
    evidenceRef: IdeProjectionRef,
  },
  Task: {
    taskRef: IdeProjectionRef,
    status: S.Literals(["queued", "running", "succeeded", "failed", "cancelled"]),
    evidenceRef: IdeProjectionRef,
  },
  Artifact: {
    artifactRef: IdeProjectionRef,
    artifactKind: SafeLabel,
    byteCount: NonNegativeInt,
    evidenceRef: IdeProjectionRef,
  },
  AgentStatus: {
    agentRef: IdeProjectionRef,
    status: S.Literals(["queued", "running", "blocked", "completed", "failed", "cancelled"]),
    attentionRequired: S.Boolean,
    evidenceRef: IdeProjectionRef,
  },
  RunStatus: {
    runRef: IdeProjectionRef,
    status: S.Literals(["queued", "running", "blocked", "completed", "failed", "cancelled"]),
    evidenceRef: IdeProjectionRef,
  },
  Delivery: {
    deliveryRef: IdeProjectionRef,
    status: S.Literals(["not_started", "pending", "delivered", "failed", "revoked"]),
    evidenceRef: IdeProjectionRef,
  },
}).annotate({ identifier: "IdeReviewItem" });
export type IdeReviewItem = typeof IdeReviewItem.Type;

/**
 * The shared authenticated mobile/web review projection. It is an explicit
 * allowlist. It carries no root, environment, credential, terminal, process,
 * native handle, provider payload, mutable capability, or public-share grant.
 */
export const IdeReviewProjection = S.Struct({
  schema: S.Literal(IDE_REVIEW_PROJECTION_SCHEMA_LITERAL),
  projectionRef: IdeProjectionRef,
  audience: IdeReviewAudience,
  ownerScopeRef: S.optionalKey(IdeProjectionRef),
  audienceScopeRef: S.optionalKey(IdeProjectionRef),
  source: IdeReviewSource,
  freshness: IdeReviewFreshness,
  availability: IdeReviewAvailability,
  items: S.Array(IdeReviewItem).check(S.isMaxLength(MAX_IDE_REVIEW_ITEMS)),
  omittedCount: NonNegativeInt,
  truncated: S.Boolean,
  nextCursorRef: S.optionalKey(IdeProjectionRef),
  generatedAt: IdeProjectionTimestamp,
  expiresAt: IdeProjectionTimestamp,
})
  .pipe(
    S.check(
      S.makeFilter(
        (projection) => Date.parse(projection.expiresAt) > Date.parse(projection.generatedAt),
        { message: "projection expiry must follow generation" },
      ),
    ),
  )
  .annotate({ identifier: "IdeReviewProjection" });
export interface IdeReviewProjection extends S.Schema.Type<typeof IdeReviewProjection> {}

/** Decode an untrusted projection and reject every field outside the allowlist. */
export const decodeIdeReviewProjection = (input: unknown) =>
  S.decodeUnknownEffect(IdeReviewProjection)(input, {
    onExcessProperty: "error",
  });
