import { Schema as S } from "effect";

const FleetRunRef = S.String.check(
  S.isPattern(/^fleet_run\.sarah\.[0-9a-f]{20}$/u),
);
const FleetRunPublicRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(180),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
);
const FleetRunAccountRefHash = S.String.check(
  S.isPattern(/^account\.pylon\.(?:codex|claude_agent|grok)\.[a-f0-9]{24}$/u),
);
const FleetRunTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u),
);
const FleetRunPublicRefs = S.Array(FleetRunPublicRef).check(S.isMaxLength(64));

export const FleetRunClientAttemptProjection = S.Struct({
  workUnitRef: FleetRunPublicRef,
  workClaimRef: FleetRunPublicRef,
  intakeClaimRef: FleetRunPublicRef,
  assignmentRef: S.NullOr(FleetRunPublicRef),
  accountRefHash: S.NullOr(FleetRunAccountRefHash),
  requestedTarget: S.Literals(["owner_local", "managed_cloud", "auto"]),
  selectedTarget: S.Literals(["owner_local", "managed_cloud"]),
  fallback: S.Struct({
    truth: S.Literals(["not_applicable", "not_reported"]),
  }),
  outcome: S.Literals([
    "running",
    "evidence_pending",
    "accepted",
    "failed",
    "stale",
  ]),
  closeoutRef: S.NullOr(FleetRunPublicRef),
  artifactRefs: FleetRunPublicRefs,
  proofRefs: FleetRunPublicRefs,
  authorityReceiptRefs: FleetRunPublicRefs,
  usageTruth: S.Literals(["pending", "exact", "not_measured"]),
  usageEvidenceRef: S.NullOr(FleetRunPublicRef),
  tokenUsageRefs: FleetRunPublicRefs,
  usageCaveatRefs: FleetRunPublicRefs,
  blockerRefs: FleetRunPublicRefs,
  terminalAt: S.NullOr(FleetRunTimestamp),
  updatedAt: FleetRunTimestamp,
});
export type FleetRunClientAttemptProjection =
  typeof FleetRunClientAttemptProjection.Type;

export const FleetRunClientRunProjection = S.Struct({
  runRef: FleetRunRef,
  authorityStatus: S.Literals(["pending_executor", "claimed_by_pylon"]),
  executionState: S.Literals([
    "pending",
    "running",
    "completed",
    "failed",
    "stopped",
  ]),
  lastSequence: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  attempts: S.Array(FleetRunClientAttemptProjection).check(S.isMaxLength(25)),
  createdAt: FleetRunTimestamp,
  updatedAt: FleetRunTimestamp,
});
export type FleetRunClientRunProjection =
  typeof FleetRunClientRunProjection.Type;

export const FleetRunClientProjection = S.Struct({
  schema: S.Literal("openagents.fleet_run_client_projection.v1"),
  privateMaterialExcluded: S.Literal(true),
  generatedAt: FleetRunTimestamp,
  runs: S.Array(FleetRunClientRunProjection).check(S.isMaxLength(20)),
});
export type FleetRunClientProjection = typeof FleetRunClientProjection.Type;

export const decodeFleetRunClientProjection = (
  value: unknown,
): FleetRunClientProjection =>
  S.decodeUnknownSync(FleetRunClientProjection)(value, {
    onExcessProperty: "error",
  });
