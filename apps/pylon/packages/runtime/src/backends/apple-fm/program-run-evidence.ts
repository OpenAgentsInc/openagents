import { Effect } from "effect";
import {
  hashProbeProgramRunInput,
  validateProbeProgramRunEvidence,
  type ProbeBlueprintProgramRunEvidence,
  type ProbeBlueprintProgramRunEvidenceUnsafe,
} from "../../blueprint/program-run-evidence";
import { type ProbeToolMenu } from "../../blueprint/tool-menu";
import { type AppleFmToolStreamResult } from "./client";
import { type AppleFmBlueprintToolProjection } from "./blueprint-tools";

export interface MakeAppleFmProgramRunEvidenceInput {
  readonly actorRef: string;
  readonly assignmentRef?: string;
  readonly menu: ProbeToolMenu;
  readonly observedAt?: string;
  readonly orderRef?: string;
  readonly promptSummaryRef: string;
  readonly projection: AppleFmBlueprintToolProjection;
  readonly result: AppleFmToolStreamResult;
  readonly runnerRef?: string;
  readonly threadRef?: string;
  readonly workroomRef?: string;
}

export function makeAppleFmToolStreamProgramRunEvidence(
  input: MakeAppleFmProgramRunEvidenceInput,
): Effect.Effect<ProbeBlueprintProgramRunEvidence, ProbeBlueprintProgramRunEvidenceUnsafe> {
  const observedAt = input.observedAt ?? new Date().toISOString();
  const programSignatureId = input.menu.programSignatureIds[0] ?? "program_signature.unknown";
  const programTypeId = input.menu.programTypeIds[0] ?? "program_type.unknown";
  const moduleVersionId = input.menu.moduleVersionIds[0] ?? "module_version.unknown";
  const toolCallbackRefs = input.result.toolTranscript.map(
    (entry) => `tool_callback.${entry.sessionId}.${entry.toolCallId}.${entry.status}`,
  );
  const evidenceRefs = [
    ...input.menu.evidenceRequirementRefs,
    `evidence.apple_fm.bridge_session.${input.result.bridgeSessionId}`,
    ...toolCallbackRefs,
  ];
  const receiptRefs = [
    ...input.menu.receiptRequirementRefs,
    `receipt.apple_fm.transcript.${input.result.bridgeSessionId}`,
    ...toolCallbackRefs.map((ref) => ref.replace("tool_callback.", "receipt.apple_fm.tool_callback.")),
  ];
  const record: ProbeBlueprintProgramRunEvidence = {
    actorRef: input.actorRef,
    assignmentRef: input.assignmentRef,
    authorityBoundary: "evidence_only",
    backendKind: input.result.profile.kind,
    backendProfileId: input.result.profile.id,
    contentRedacted: true,
    costRef: `cost.apple_fm.${input.result.bridgeSessionId}.${input.result.completion.usage.truth}`,
    directMutationDisabled: true,
    evidenceRefs,
    inputSnapshotHash: hashProbeProgramRunInput({
      lookupId: input.projection.lookupId,
      menuId: input.projection.menuId,
      promptSummaryRef: input.promptSummaryRef,
      registryVersionRef: input.projection.registryVersionRef,
      toolRefs: input.projection.toolRefs.map((tool) => tool.toolRef),
    }),
    kind: "probe_blueprint_program_run_evidence",
    latencyMs: 0,
    lookupId: input.projection.lookupId,
    menuId: input.projection.menuId,
    model: input.result.completion.response.model ?? input.result.profile.model,
    moduleVersionId,
    noDeploy: true,
    noEmail: true,
    noSourceMutation: true,
    noSpend: true,
    observedAt,
    orderRef: input.orderRef,
    programRunRef: `program_run.probe.apple_fm.${input.result.bridgeSessionId}`,
    programSignatureId,
    programTypeId,
    promptSummaryRef: input.promptSummaryRef,
    receiptRefs,
    registryVersionRef: input.projection.registryVersionRef,
    routeRef: `route.probe.apple_fm.${input.result.profile.id}`,
    runnerRef: input.runnerRef,
    threadRef: input.threadRef,
    toolCallbackRefs,
    typedOutput: {
      finalOutputRef: `output.apple_fm.${input.result.bridgeSessionId}.final`,
      toolCallCount: input.result.toolTranscript.length,
      usageTruth: input.result.completion.usage.truth,
    },
    usage: input.result.completion.usage,
    workroomRef: input.workroomRef,
  };

  return validateProbeProgramRunEvidence(record);
}
