import { createHash } from "node:crypto";
import { Effect, Schema as S } from "effect";
import {
  blueprintProgramRunEvidenceFlagsAreEvidenceOnly,
  isBlueprintProjectionPrivateDataSafe,
} from "./contracts.js";

export const ProbeBlueprintProgramRunEvidence = S.Struct({
  actorRef: S.String,
  assignmentRef: S.optional(S.String),
  authorityBoundary: S.Literal("evidence_only"),
  backendKind: S.String,
  backendProfileId: S.String,
  contentRedacted: S.Literal(true),
  costRef: S.String,
  directMutationDisabled: S.Boolean,
  evidenceRefs: S.Array(S.String),
  inputSnapshotHash: S.String,
  kind: S.Literal("probe_blueprint_program_run_evidence"),
  latencyMs: S.Number,
  lookupId: S.String,
  menuId: S.String,
  model: S.String,
  moduleVersionId: S.String,
  noDeploy: S.Boolean,
  noEmail: S.Boolean,
  noSourceMutation: S.Boolean,
  noSpend: S.Boolean,
  observedAt: S.String,
  orderRef: S.optional(S.String),
  programRunRef: S.String,
  programSignatureId: S.String,
  programTypeId: S.String,
  promptSummaryRef: S.String,
  receiptRefs: S.Array(S.String),
  registryVersionRef: S.String,
  routeRef: S.String,
  runnerRef: S.optional(S.String),
  threadRef: S.optional(S.String),
  toolCallbackRefs: S.Array(S.String),
  typedOutput: S.Record(S.String, S.Unknown),
  usage: S.Struct({
    truth: S.Literals(["exact", "estimated", "unknown"]),
    promptTokens: S.optional(S.Number),
    completionTokens: S.optional(S.Number),
    totalTokens: S.optional(S.Number),
  }),
  workroomRef: S.optional(S.String),
});
export type ProbeBlueprintProgramRunEvidence = typeof ProbeBlueprintProgramRunEvidence.Type;

export class ProbeBlueprintProgramRunEvidenceUnsafe extends S.TaggedErrorClass<ProbeBlueprintProgramRunEvidenceUnsafe>()(
  "ProbeBlueprintProgramRunEvidenceUnsafe",
  {
    path: S.String,
    reason: S.String,
  },
) {}

export function probeProgramRunEvidenceIsEvidenceOnly(record: ProbeBlueprintProgramRunEvidence): boolean {
  return blueprintProgramRunEvidenceFlagsAreEvidenceOnly(record);
}

export function validateProbeProgramRunEvidence(
  record: ProbeBlueprintProgramRunEvidence,
): Effect.Effect<ProbeBlueprintProgramRunEvidence, ProbeBlueprintProgramRunEvidenceUnsafe> {
  return Effect.gen(function* () {
    if (!probeProgramRunEvidenceIsEvidenceOnly(record)) {
      return yield* Effect.fail(
        new ProbeBlueprintProgramRunEvidenceUnsafe({
          path: "programRun",
          reason: "Program Run evidence cannot carry write authority",
        }),
      );
    }

    if (!isBlueprintProjectionPrivateDataSafe(record)) {
      return yield* Effect.fail(
        new ProbeBlueprintProgramRunEvidenceUnsafe({
          path: "programRun",
          reason: "Program Run evidence contains private-data-shaped material",
        }),
      );
    }

    return record;
  });
}

export function hashProbeProgramRunInput(value: unknown): string {
  return `sha256:${createHash("sha256").update(JSON.stringify(value)).digest("hex")}`;
}
