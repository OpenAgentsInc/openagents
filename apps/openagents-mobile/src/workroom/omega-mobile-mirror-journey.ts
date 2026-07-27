import { Schema } from "effect";

export const OMEGA_MOBILE_MIRROR_JOURNEY_SCHEMA =
  "openagents.omega.mobile_mirror_journey_receipt.v1" as const;

const CommitSha = Schema.String.check(Schema.isPattern(/^[0-9a-f]{40}$/));
const EvidenceRef = Schema.String.check(Schema.isMinLength(1));

const SimulatedStageSchema = Schema.Struct({
  stage: Schema.Literals(["M0", "M1", "M2"]),
  status: Schema.Literal("passed_simulator"),
  evidenceRefs: Schema.Array(EvidenceRef),
  summary: EvidenceRef,
});

const BlockedStageSchema = Schema.Struct({
  stage: Schema.Literals(["revocation", "live_host", "physical_device"]),
  status: Schema.Literals(["blocked_dependency", "not_run"]),
  blockerRefs: Schema.Array(EvidenceRef),
  summary: EvidenceRef,
});

export const OmegaMobileMirrorJourneyReceiptSchema = Schema.Struct({
  schema: Schema.Literal(OMEGA_MOBILE_MIRROR_JOURNEY_SCHEMA),
  issue: Schema.Literal("OpenAgentsInc/openagents#9262"),
  mode: Schema.Literal("simulator"),
  generatedAt: Schema.String,
  source: Schema.Struct({
    openagentsCommit: CommitSha,
    omegaCommit: CommitSha,
  }),
  simulator: Schema.Struct({
    platform: Schema.Literals(["ios", "android"]),
    appIdentifier: Schema.Literal("com.openagents.app"),
    protocol: Schema.Literal("openagents.omega.device_bridge.v1"),
  }),
  stages: Schema.Array(SimulatedStageSchema),
  residual: Schema.Array(BlockedStageSchema),
  redaction: Schema.Struct({
    publicSafe: Schema.Literal(true),
    mirrorEphemeral: Schema.Literal(true),
    forbiddenMaterialScanned: Schema.Literal(true),
  }),
  summary: Schema.Struct({
    simulatorStagesPassed: Schema.Literal(3),
    overall: Schema.Literal("blocked_live_host"),
  }),
});

export type OmegaMobileMirrorJourneyReceipt = Schema.Schema.Type<
  typeof OmegaMobileMirrorJourneyReceiptSchema
>;

const decodeReceipt = Schema.decodeUnknownSync(OmegaMobileMirrorJourneyReceiptSchema);
const forbiddenReceiptMaterial = [
  /nsec1[023456789acdefghjklmnpqrstuvwxyz]+/iu,
  /"privateKey"\s*:/iu,
  /"seed"\s*:/iu,
  /"mnemonic"\s*:/iu,
  /pairingSecret/iu,
  /\/Users\/[^/\s]+\/Library\/Keychains/iu,
] as const;

export const decodeOmegaMobileMirrorJourneyReceipt = (
  input: unknown,
): OmegaMobileMirrorJourneyReceipt => {
  const receipt = decodeReceipt(input, { onExcessProperty: "error" });
  const encoded = JSON.stringify(receipt);
  if (forbiddenReceiptMaterial.some((pattern) => pattern.test(encoded))) {
    throw new Error("The Omega mobile mirror journey receipt contains forbidden material.");
  }
  return receipt;
};
