import { Effect, Schema as S } from "effect";
import { isBlueprintProjectionPrivateDataSafe } from "./contracts.js";
import { hashProbeProgramRunInput } from "./program-run-evidence.js";

export const ProbeRequestedEffectKind = S.Literals([
  "create_pull_request",
  "deploy",
  "send_email",
  "post_public_claim",
  "spend_money",
  "legal_sensitive_commitment",
  "mutate_source_backed_business_fact",
  "local_sandbox_file_edit",
  "local_sandbox_read",
  "local_evidence_record",
]);
export type ProbeRequestedEffectKind = typeof ProbeRequestedEffectKind.Type;

export const ProbeExternalEffectClassification = S.Struct({
  actionSubmissionRequired: S.Boolean,
  directProgramRunExecutionAllowed: S.Boolean,
  effectKind: ProbeRequestedEffectKind,
  reasonRef: S.String,
});
export type ProbeExternalEffectClassification = typeof ProbeExternalEffectClassification.Type;

export const ProbeActionSubmissionProposalStatus = S.Literals(["proposed"]);
export type ProbeActionSubmissionProposalStatus = typeof ProbeActionSubmissionProposalStatus.Type;

export const ProbeActionSubmissionProposal = S.Struct({
  actionSubmissionRef: S.String,
  actorRef: S.String,
  approvalPolicyRef: S.String,
  approvalRequired: S.Literal(true),
  assignmentRef: S.optional(S.String),
  contentRedacted: S.Literal(true),
  contextPackRefs: S.Array(S.String),
  directExecution: S.Literal(false),
  directProgramRunExecutionAllowed: S.Literal(false),
  evidenceRefs: S.Array(S.String),
  effectKind: ProbeRequestedEffectKind,
  inputSnapshotHash: S.String,
  kind: S.Literal("probe_blueprint_action_submission_proposal"),
  modelConfidenceBypassDisabled: S.Literal(true),
  moduleVersionId: S.optional(S.String),
  observedAt: S.String,
  programRunAuthorityBoundary: S.Literal("evidence_only"),
  programRunRef: S.String,
  programSignatureId: S.optional(S.String),
  programTypeId: S.optional(S.String),
  proposalOnly: S.Literal(true),
  receiptRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  status: ProbeActionSubmissionProposalStatus,
  summaryRef: S.String,
  toolRefs: S.Array(S.String),
  typedIntent: S.Record(S.String, S.Unknown),
});
export type ProbeActionSubmissionProposal = typeof ProbeActionSubmissionProposal.Type;

export class ProbeDirectExternalEffectDenied extends S.TaggedErrorClass<ProbeDirectExternalEffectDenied>()(
  "ProbeDirectExternalEffectDenied",
  {
    actionSubmissionRequired: S.Literal(true),
    effectKind: ProbeRequestedEffectKind,
    reason: S.String,
  },
) {}

export class ProbeActionSubmissionProposalUnsafe extends S.TaggedErrorClass<ProbeActionSubmissionProposalUnsafe>()(
  "ProbeActionSubmissionProposalUnsafe",
  {
    path: S.String,
    reason: S.String,
  },
) {}

export interface MakeProbeActionSubmissionProposalInput {
  readonly actionSubmissionRef?: string;
  readonly actorRef: string;
  readonly approvalPolicyRef: string;
  readonly assignmentRef?: string;
  readonly contextPackRefs?: ReadonlyArray<string>;
  readonly effectKind: ProbeRequestedEffectKind;
  readonly evidenceRefs: ReadonlyArray<string>;
  readonly moduleVersionId?: string;
  readonly observedAt?: string;
  readonly programRunRef: string;
  readonly programSignatureId?: string;
  readonly programTypeId?: string;
  readonly receiptRefs?: ReadonlyArray<string>;
  readonly sourceAuthorityRefs?: ReadonlyArray<string>;
  readonly summaryRef: string;
  readonly toolRefs?: ReadonlyArray<string>;
  readonly typedIntent: Readonly<Record<string, unknown>>;
}

const EXTERNAL_EFFECT_KINDS: ReadonlySet<ProbeRequestedEffectKind> = new Set([
  "create_pull_request",
  "deploy",
  "send_email",
  "post_public_claim",
  "spend_money",
  "legal_sensitive_commitment",
  "mutate_source_backed_business_fact",
]);

export function classifyProbeRequestedEffect(effectKind: ProbeRequestedEffectKind): ProbeExternalEffectClassification {
  const actionSubmissionRequired = EXTERNAL_EFFECT_KINDS.has(effectKind);

  return {
    actionSubmissionRequired,
    directProgramRunExecutionAllowed: !actionSubmissionRequired,
    effectKind,
    reasonRef: actionSubmissionRequired
      ? `reason.probe.action_submission_required.${effectKind}`
      : `reason.probe.local_tool_allowed.${effectKind}`,
  };
}

export function guardProgramRunDirectEffect(
  effectKind: ProbeRequestedEffectKind,
): Effect.Effect<ProbeExternalEffectClassification, ProbeDirectExternalEffectDenied> {
  const classification = classifyProbeRequestedEffect(effectKind);

  return classification.actionSubmissionRequired
    ? Effect.fail(
        new ProbeDirectExternalEffectDenied({
          actionSubmissionRequired: true,
          effectKind,
          reason: "Program Run authority is evidence-only; external write-side effects require Action Submission",
        }),
      )
    : Effect.succeed(classification);
}

export function makeProbeActionSubmissionProposal(
  input: MakeProbeActionSubmissionProposalInput,
): Effect.Effect<ProbeActionSubmissionProposal, ProbeActionSubmissionProposalUnsafe> {
  return Effect.gen(function* () {
    const classification = classifyProbeRequestedEffect(input.effectKind);

    if (!classification.actionSubmissionRequired) {
      return yield* Effect.fail(
        new ProbeActionSubmissionProposalUnsafe({
          path: "effectKind",
          reason: "Local Probe tool effects do not create Blueprint Action Submission proposals",
        }),
      );
    }

    const proposal: ProbeActionSubmissionProposal = {
      actionSubmissionRef:
        input.actionSubmissionRef ?? `action_submission.probe.${safeRefSegment(input.programRunRef)}.${input.effectKind}`,
      actorRef: input.actorRef,
      approvalPolicyRef: input.approvalPolicyRef,
      approvalRequired: true,
      assignmentRef: input.assignmentRef,
      contentRedacted: true,
      contextPackRefs: [...(input.contextPackRefs ?? [])],
      directExecution: false,
      directProgramRunExecutionAllowed: false,
      evidenceRefs: [...input.evidenceRefs],
      effectKind: input.effectKind,
      inputSnapshotHash: hashProbeProgramRunInput({
        effectKind: input.effectKind,
        evidenceRefs: input.evidenceRefs,
        programRunRef: input.programRunRef,
        summaryRef: input.summaryRef,
        typedIntent: input.typedIntent,
      }),
      kind: "probe_blueprint_action_submission_proposal",
      modelConfidenceBypassDisabled: true,
      moduleVersionId: input.moduleVersionId,
      observedAt: input.observedAt ?? new Date().toISOString(),
      programRunAuthorityBoundary: "evidence_only",
      programRunRef: input.programRunRef,
      programSignatureId: input.programSignatureId,
      programTypeId: input.programTypeId,
      proposalOnly: true,
      receiptRefs: uniqueStrings(["receipt.action_submission", ...(input.receiptRefs ?? [])]),
      sourceAuthorityRefs: [...(input.sourceAuthorityRefs ?? [])],
      status: "proposed",
      summaryRef: input.summaryRef,
      toolRefs: [...(input.toolRefs ?? [])],
      typedIntent: { ...input.typedIntent },
    };

    return yield* validateProbeActionSubmissionProposal(proposal);
  });
}

export function validateProbeActionSubmissionProposal(
  proposal: ProbeActionSubmissionProposal,
): Effect.Effect<ProbeActionSubmissionProposal, ProbeActionSubmissionProposalUnsafe> {
  return Effect.gen(function* () {
    const classification = classifyProbeRequestedEffect(proposal.effectKind);

    if (!classification.actionSubmissionRequired) {
      return yield* Effect.fail(
        new ProbeActionSubmissionProposalUnsafe({
          path: "effectKind",
          reason: "Action Submission proposals are only for external write-side effects",
        }),
      );
    }

    if (
      !proposal.approvalRequired ||
      proposal.directExecution ||
      proposal.directProgramRunExecutionAllowed ||
      !proposal.modelConfidenceBypassDisabled ||
      proposal.programRunAuthorityBoundary !== "evidence_only" ||
      !proposal.proposalOnly
    ) {
      return yield* Effect.fail(
        new ProbeActionSubmissionProposalUnsafe({
          path: "proposal",
          reason: "Action Submission proposals cannot execute external effects or bypass approval",
        }),
      );
    }

    if (!isBlueprintProjectionPrivateDataSafe(proposal)) {
      return yield* Effect.fail(
        new ProbeActionSubmissionProposalUnsafe({
          path: "proposal",
          reason: "Action Submission proposal contains private-data-shaped material",
        }),
      );
    }

    return proposal;
  });
}

function safeRefSegment(value: string): string {
  return value.replace(/[^A-Za-z0-9_.-]/g, "_");
}

function uniqueStrings(values: ReadonlyArray<string>): Array<string> {
  return [...new Set(values)].sort();
}
