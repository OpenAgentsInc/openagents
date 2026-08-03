import { Effect, Schema as S } from "effect";

import {
  ALL_WORK_CONTRACT_DEFINITION_SHA256,
  ALL_WORK_MAX_ENCODED_BYTES,
  decodeProtocolInitializeResult,
  type ProtocolInitializeRequest,
  type WorkReadRequestFrame,
  type WorkSnapshot,
  type WorkSummary,
} from "./generated.ts";

export const AllWorkSemanticErrorCodeSchema = S.Literals([
  "incompatible_version",
  "issue_identity_mismatch",
  "issue_revision_mismatch",
  "execution_projection_mismatch",
  "work_identity_mismatch",
  "revision_regression",
  "cursor_changed_without_revision",
]);
export type AllWorkSemanticErrorCode = typeof AllWorkSemanticErrorCodeSchema.Type;

export class AllWorkSemanticError extends S.TaggedErrorClass<AllWorkSemanticError>()(
  "AllWorkContract.SemanticError",
  {
    code: AllWorkSemanticErrorCodeSchema,
    detail: S.String,
  },
) {}

const capabilities = [
  "work.index.read",
  "work.index.subscribe",
  "work.snapshot.read",
  "planning.graph.read",
  "repository.claim.read",
  "repository.claim.execute",
  "workroom.activity.read",
  "workroom.activity.prepare",
  "workroom.activity.commit",
  "workroom.activity.enqueue",
  "workroom.activity.deliver",
  "workroom.activity.publish",
  "work.command.execute",
  "work.cutover.read",
  "work.cutover.execute",
  "organization.membership.read",
  "strict_bug.candidate.read",
  "strict_bug.candidate.execute",
] as const;

const compareUnicodeCodePoints = (left: string, right: string): number => {
  const leftPoints = Array.from(left, (value) => value.codePointAt(0) ?? 0);
  const rightPoints = Array.from(right, (value) => value.codePointAt(0) ?? 0);
  const length = Math.min(leftPoints.length, rightPoints.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftPoints[index] ?? 0) - (rightPoints[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return leftPoints.length - rightPoints.length;
};

const canonicalize = (value: unknown, arrayElement = false): unknown => {
  if (value === null || typeof value === "string" || typeof value === "boolean") return value;
  if (typeof value === "number") {
    if (!Number.isSafeInteger(value))
      throw new TypeError("Canonical JSON accepts safe integers only");
    return value;
  }
  if (value === undefined) {
    if (arrayElement) throw new TypeError("Canonical JSON array elements cannot be undefined");
    return undefined;
  }
  if (Array.isArray(value)) return value.map((item) => canonicalize(item, true));
  if (typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort(compareUnicodeCodePoints)
        .flatMap((key) => {
          const encoded = canonicalize(Reflect.get(value, key));
          return encoded === undefined ? [] : [[key, encoded]];
        }),
    );
  }
  throw new TypeError(`Canonical JSON does not support ${typeof value}`);
};

export const encodeAllWorkCanonicalJson = (value: unknown): string => {
  const encoded = JSON.stringify(canonicalize(value));
  if (encoded === undefined) throw new TypeError("Canonical JSON requires a root value");
  if (Buffer.byteLength(encoded, "utf8") > ALL_WORK_MAX_ENCODED_BYTES) {
    throw new TypeError(`Canonical JSON exceeds ${ALL_WORK_MAX_ENCODED_BYTES} UTF-8 bytes`);
  }
  return encoded;
};

export const negotiateAllWorkProtocol = Effect.fn("AllWorkContract.negotiateProtocol")(function* (
  request: ProtocolInitializeRequest,
) {
  const selectedVersion = request.supportedVersions.includes("omega-effectd.v2")
    ? "omega-effectd.v2"
    : request.supportedVersions.includes("omega-effectd.v1")
      ? "omega-effectd.v1"
      : null;

  if (selectedVersion === null) {
    return yield* new AllWorkSemanticError({
      code: "incompatible_version",
      detail: "No supported omega-effectd protocol version",
    });
  }

  return decodeProtocolInitializeResult({
    selectedVersion,
    contractRef: "openagents.all_work_boundary.v1",
    contractDigest: ALL_WORK_CONTRACT_DEFINITION_SHA256,
    capabilities:
      selectedVersion === "omega-effectd.v2"
        ? capabilities.filter((capability) => request.requestedCapabilities.includes(capability))
        : [],
  });
});

export const validateWorkReadRequestFrame = Effect.fn(
  "AllWorkContract.validateWorkReadRequestFrame",
)(function* (frame: WorkReadRequestFrame) {
  if (frame.method !== "protocol.initialize" && frame.version !== "omega-effectd.v2") {
    return yield* new AllWorkSemanticError({
      code: "incompatible_version",
      detail: `${frame.method} requires omega-effectd.v2`,
    });
  }
  return frame;
});

export const validateWorkSnapshotSemantics = Effect.fn(
  "AllWorkContract.validateWorkSnapshotSemantics",
)(function* (snapshot: WorkSnapshot) {
  if (snapshot.issue !== undefined && snapshot.issue !== null) {
    if (snapshot.issue.workRef !== snapshot.summary.workRef) {
      return yield* new AllWorkSemanticError({
        code: "issue_identity_mismatch",
        detail: "Issue projection must use the Work snapshot identity",
      });
    }
    if (snapshot.issue.revision !== snapshot.summary.revision) {
      return yield* new AllWorkSemanticError({
        code: "issue_revision_mismatch",
        detail: "Issue projection must use the Work snapshot revision",
      });
    }
  }
  const projectedSessions = snapshot.sessionProjections ?? [];
  const distinctSessions = new Set(projectedSessions.map((session) => session.sessionRef));
  if (
    distinctSessions.size !== projectedSessions.length ||
    projectedSessions.some(
      (session) =>
        session.generation === 0 ||
        !snapshot.sessionRefs.includes(session.sessionRef) ||
        !snapshot.threadRefs.includes(session.threadRef) ||
        !snapshot.agentSessionRefs.includes(session.agentSessionRef) ||
        !snapshot.runRefs.includes(session.runRef),
    )
  ) {
    return yield* new AllWorkSemanticError({
      code: "execution_projection_mismatch",
      detail: "Session projections must preserve distinct snapshot identities and generation",
    });
  }
  const projectedActivities = snapshot.agentActivityProjections ?? [];
  const distinctActivities = new Set(projectedActivities.map((activity) => activity.activityRef));
  if (
    distinctActivities.size !== projectedActivities.length ||
    projectedActivities.some(
      (activity) =>
        activity.generation === 0 ||
        !snapshot.agentActivityRefs.includes(activity.activityRef) ||
        !snapshot.sessionRefs.includes(activity.sessionRef) ||
        !snapshot.runRefs.includes(activity.runRef) ||
        !projectedSessions.some(
          (session) =>
            session.sessionRef === activity.sessionRef &&
            session.runRef === activity.runRef &&
            session.generation === activity.generation,
        ),
    )
  ) {
    return yield* new AllWorkSemanticError({
      code: "execution_projection_mismatch",
      detail:
        "Agent Activity projections must preserve distinct snapshot identities and generation",
    });
  }
  return snapshot;
});

export const validateWorkSummarySuccessor = Effect.fn(
  "AllWorkContract.validateWorkSummarySuccessor",
)(function* (previous: WorkSummary, next: WorkSummary) {
  if (previous.workRef !== next.workRef) {
    return yield* new AllWorkSemanticError({
      code: "work_identity_mismatch",
      detail: "A Work successor must preserve Work identity",
    });
  }

  if (previous.sourceAuthority.sourceRef !== next.sourceAuthority.sourceRef) return next;

  if (next.revision < previous.revision) {
    return yield* new AllWorkSemanticError({
      code: "revision_regression",
      detail: "A source authority cannot regress its Work revision",
    });
  }

  const previousCursor = previous.completeness.cursor ?? null;
  const nextCursor = next.completeness.cursor ?? null;
  if (next.revision === previous.revision && previousCursor !== nextCursor) {
    return yield* new AllWorkSemanticError({
      code: "cursor_changed_without_revision",
      detail: "A source authority cannot change its cursor without advancing the Work revision",
    });
  }

  return next;
});
