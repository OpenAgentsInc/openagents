import { Schema } from "effect"

export const FleetRunProjectionListChannel = "openagents:fleet-run-projection:list"

const PublicRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(180),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u),
)
const Timestamp = Schema.String.check(
  Schema.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u),
)
const PublicRefs = Schema.Array(PublicRef).check(Schema.isMaxLength(64))
const Attempt = Schema.Struct({
  workUnitRef: PublicRef,
  workClaimRef: PublicRef,
  intakeClaimRef: PublicRef,
  assignmentRef: Schema.NullOr(PublicRef),
  accountRefHash: Schema.NullOr(Schema.String.check(
    Schema.isPattern(/^account\.pylon\.(?:codex|claude_agent|grok)\.[a-f0-9]{24}$/u),
  )),
  requestedTarget: Schema.Literals(["owner_local", "managed_cloud", "auto"]),
  selectedTarget: Schema.Literals(["owner_local", "managed_cloud"]),
  fallback: Schema.Struct({ truth: Schema.Literals(["not_applicable", "not_reported"]) }),
  outcome: Schema.Literals(["running", "evidence_pending", "accepted", "failed", "stale"]),
  closeoutRef: Schema.NullOr(PublicRef),
  artifactRefs: PublicRefs,
  proofRefs: PublicRefs,
  authorityReceiptRefs: PublicRefs,
  usageTruth: Schema.Literals(["pending", "exact", "not_measured"]),
  usageEvidenceRef: Schema.NullOr(PublicRef),
  tokenUsageRefs: PublicRefs,
  usageCaveatRefs: PublicRefs,
  blockerRefs: PublicRefs,
  terminalAt: Schema.NullOr(Timestamp),
  updatedAt: Timestamp,
})
const Run = Schema.Struct({
  runRef: Schema.String.check(Schema.isPattern(/^fleet_run\.sarah\.[0-9a-f]{20}$/u)),
  authorityStatus: Schema.Literals(["pending_executor", "claimed_by_pylon", "cancelled"]),
  executionState: Schema.Literals(["pending", "running", "completed", "failed", "stopped"]),
  lastSequence: Schema.Int.check(Schema.isGreaterThanOrEqualTo(0)),
  attempts: Schema.Array(Attempt).check(Schema.isMaxLength(25)),
  createdAt: Timestamp,
  updatedAt: Timestamp,
})
export const FleetRunClientProjectionSchema = Schema.Struct({
  schema: Schema.Literal("openagents.fleet_run_client_projection.v1"),
  privateMaterialExcluded: Schema.Literal(true),
  generatedAt: Timestamp,
  runs: Schema.Array(Run).check(Schema.isMaxLength(20)),
})
export type FleetRunClientProjection = typeof FleetRunClientProjectionSchema.Type

export const decodeFleetRunClientProjection = (value: unknown): FleetRunClientProjection =>
  Schema.decodeUnknownSync(FleetRunClientProjectionSchema)(value, { onExcessProperty: "error" })
