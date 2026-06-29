import { Effect, Schema as S } from "effect";
import { ProbeBenchmarkContractError } from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { type ProbeToolMenuPlannerInput } from "../blueprint/tool-menu";
import {
  applyOpenAgentsAutopilotCoderStudiedContextToToolMenuInput,
  buildOpenAgentsAutopilotCoderStudiedContext,
  buildOpenAgentsAutopilotCoderStudiedPlanContext,
  type OpenAgentsAutopilotCoderStudiedContext,
  type OpenAgentsAutopilotCoderStudiedPlanContext,
} from "./openagents-autopilot-coder-studied-context";
import {
  generateOpenAgentsRepoStudyArtifact,
  type GenerateOpenAgentsRepoStudyArtifactInput,
} from "./openagents-study-artifact";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

export const OPENAGENTS_AUTOPILOT_CODER_STUDIED_RUNTIME_SCHEMA_REF =
  "openagents.autopilot_coder_studied_runtime_context.v0" as const;

export const OPENAGENTS_AUTOPILOT_CODER_STUDIED_RUNTIME_DOGFOOD_LIFT_SCHEMA_REF =
  "openagents.autopilot_coder_studied_runtime_dogfood_lift.v0" as const;

// The single SA-2 wiring entrypoint takes a repo rootDir, runs the SA-1 study
// artifact pipeline, threads the studied packet+graph into the S5 studied
// context and plan context, and surfaces a digest-pinned runtime context that
// the live Probe/Autopilot-coder tool-menu plan path consumes. The dogfood
// lift is carried as an internal-only measurement: it never grants a customer,
// marketplace, or payout claim (see sourceBoundary / customerPublicClaimAllowed).
export const OpenAgentsAutopilotCoderStudiedRuntimeDogfoodLift = S.Struct({
  baselineCandidateRef: S.String,
  customerPublicClaimAllowed: S.Literal(false),
  distinguishingMetricRefs: S.Array(S.String),
  evalReportHash: S.String,
  evalReportRef: S.String,
  firstDivergenceStepLift: S.Number,
  passRateLiftBps: S.Number,
  rubricScoreLiftBps: S.Number,
  schemaRef: S.Literal(OPENAGENTS_AUTOPILOT_CODER_STUDIED_RUNTIME_DOGFOOD_LIFT_SCHEMA_REF),
  scope: S.Literal("internal_dogfood_only"),
  studiedBeatsBaseline: S.Boolean,
  studiedCandidateRef: S.String,
  wrongFileReadReduction: S.Number,
});
export type OpenAgentsAutopilotCoderStudiedRuntimeDogfoodLift =
  typeof OpenAgentsAutopilotCoderStudiedRuntimeDogfoodLift.Type;

// The decoded studied context/plan context are large nested structs already
// validated by their own builders; the runtime context keeps the typed objects
// alongside a digest summary so the live runtime can both inject (refs) and
// inspect (full objects) without re-decoding.
export interface OpenAgentsAutopilotCoderStudiedRuntimeContext {
  readonly commit: string;
  readonly context: OpenAgentsAutopilotCoderStudiedContext;
  readonly contextPackRef: string;
  readonly correctnessGatePassed: boolean;
  readonly dogfoodLift: OpenAgentsAutopilotCoderStudiedRuntimeDogfoodLift;
  readonly editSitePath: string;
  readonly graphRef: string;
  readonly indexHash: string;
  readonly indexRef: string;
  readonly packetRef: string;
  readonly planContext: OpenAgentsAutopilotCoderStudiedPlanContext;
  readonly planContextRef: string;
  readonly repo: string;
  readonly runtimeContextHash: string;
  readonly runtimeContextRef: string;
  readonly schemaRef: typeof OPENAGENTS_AUTOPILOT_CODER_STUDIED_RUNTIME_SCHEMA_REF;
  readonly sourceBoundary: "public_refs_only";
}

export interface LoadOpenAgentsAutopilotCoderStudiedRuntimeContextInput
  extends GenerateOpenAgentsRepoStudyArtifactInput {
  readonly editSitePath?: string;
  readonly existingContextPackRefs?: ReadonlyArray<string>;
}

export function loadOpenAgentsAutopilotCoderStudiedRuntimeContext(
  input: LoadOpenAgentsAutopilotCoderStudiedRuntimeContextInput,
): Effect.Effect<
  OpenAgentsAutopilotCoderStudiedRuntimeContext,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const artifact = yield* generateOpenAgentsRepoStudyArtifact({
      backroomRootDir: input.backroomRootDir,
      commit: input.commit,
      commitHistory: input.commitHistory,
      commitHistoryLimit: input.commitHistoryLimit,
      indexRef: input.indexRef,
      repo: input.repo,
      rootDir: input.rootDir,
    });

    const context = yield* buildOpenAgentsAutopilotCoderStudiedContext({
      editSitePath: input.editSitePath,
      graph: artifact.graph,
      packet: artifact.packet,
    });
    const planContext = yield* buildOpenAgentsAutopilotCoderStudiedPlanContext({
      context,
      existingContextPackRefs: input.existingContextPackRefs,
    });

    const comparison = artifact.evalReport.comparison;
    const dogfoodLift: OpenAgentsAutopilotCoderStudiedRuntimeDogfoodLift = {
      baselineCandidateRef: comparison.baselineCandidateRef,
      customerPublicClaimAllowed: false,
      distinguishingMetricRefs: comparison.distinguishingMetricRefs,
      evalReportHash: artifact.evalReport.reportHash,
      evalReportRef: artifact.evalReport.reportRef,
      firstDivergenceStepLift: comparison.firstDivergenceStepLift,
      passRateLiftBps: comparison.passRateLiftBps,
      rubricScoreLiftBps: comparison.rubricScoreLiftBps,
      schemaRef: OPENAGENTS_AUTOPILOT_CODER_STUDIED_RUNTIME_DOGFOOD_LIFT_SCHEMA_REF,
      scope: "internal_dogfood_only",
      studiedBeatsBaseline: comparison.studiedBeatsBaseline,
      studiedCandidateRef: comparison.studiedCandidateRef,
      wrongFileReadReduction: comparison.wrongFileReadReduction,
    };

    const baseRuntimeContext = {
      commit: artifact.index.commit,
      context,
      contextPackRef: context.contextPackRef,
      correctnessGatePassed: artifact.verification.correctnessGatePassed,
      dogfoodLift,
      editSitePath: context.editSitePath,
      graphRef: context.graphRef,
      indexHash: artifact.index.indexHash,
      indexRef: artifact.index.indexRef,
      packetRef: context.packetRef,
      planContext,
      planContextRef: planContext.planContextRef,
      repo: artifact.index.repo,
      runtimeContextHash: "sha256:pending",
      runtimeContextRef: "openagents_autopilot_coder_studied_runtime_context.pending",
      schemaRef: OPENAGENTS_AUTOPILOT_CODER_STUDIED_RUNTIME_SCHEMA_REF,
      sourceBoundary: "public_refs_only",
    } as const satisfies OpenAgentsAutopilotCoderStudiedRuntimeContext;

    const runtimeContextHash = openAgentsAutopilotCoderStudiedRuntimeContextHash(baseRuntimeContext);
    const runtimeContext: OpenAgentsAutopilotCoderStudiedRuntimeContext = {
      ...baseRuntimeContext,
      runtimeContextHash,
      runtimeContextRef: `openagents_autopilot_coder_studied_runtime_context.${shortHash(runtimeContextHash)}`,
    };

    yield* validateOpenAgentsAutopilotCoderStudiedRuntimeContext(runtimeContext);
    return runtimeContext;
  });
}

// Live-runtime injection point. The Probe/Autopilot-coder tool-menu plan path
// (planProbeToolMenu) consumes ProbeToolMenuPlannerInput; this threads the
// studied context pack and source-authority refs into that input so the live
// coding agent loads studied knowledge before planning its tools.
export function applyOpenAgentsAutopilotCoderStudiedRuntimeContextToToolMenuInput(
  input: ProbeToolMenuPlannerInput,
  runtimeContext: OpenAgentsAutopilotCoderStudiedRuntimeContext,
): ProbeToolMenuPlannerInput {
  const withStudiedContext = applyOpenAgentsAutopilotCoderStudiedContextToToolMenuInput(
    input,
    runtimeContext.context,
  );
  return {
    ...withStudiedContext,
    contextPackRefs: uniqueRefs([
      ...withStudiedContext.contextPackRefs,
      ...runtimeContext.planContext.contextPackRefs,
    ]),
  };
}

export function openAgentsAutopilotCoderStudiedRuntimeContextHash(
  runtimeContext: OpenAgentsAutopilotCoderStudiedRuntimeContext,
): string {
  const {
    runtimeContextHash: _runtimeContextHash,
    runtimeContextRef: _runtimeContextRef,
    ...stable
  } = runtimeContext;
  return sha256Ref(stableJson(stable));
}

function validateOpenAgentsAutopilotCoderStudiedRuntimeContext(
  runtimeContext: OpenAgentsAutopilotCoderStudiedRuntimeContext,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(runtimeContext.repo, "autopilotCoderStudiedRuntimeContext.repo");
    yield* requireNonEmpty(runtimeContext.commit, "autopilotCoderStudiedRuntimeContext.commit");
    yield* requireNonEmpty(runtimeContext.editSitePath, "autopilotCoderStudiedRuntimeContext.editSitePath");
    yield* requireNonEmpty(runtimeContext.contextPackRef, "autopilotCoderStudiedRuntimeContext.contextPackRef");
    yield* requireNonEmpty(runtimeContext.planContextRef, "autopilotCoderStudiedRuntimeContext.planContextRef");
    yield* requireSha256(runtimeContext.runtimeContextHash, "autopilotCoderStudiedRuntimeContext.runtimeContextHash");
    yield* requireSha256(runtimeContext.indexHash, "autopilotCoderStudiedRuntimeContext.indexHash");

    if (runtimeContext.context.contextPackRef !== runtimeContext.contextPackRef) {
      return yield* runtimeContextError(
        "autopilotCoderStudiedRuntimeContext.contextPackRef",
        "must match the studied context contextPackRef",
      );
    }

    if (!runtimeContext.planContext.contextPackRefs.includes(runtimeContext.contextPackRef)) {
      return yield* runtimeContextError(
        "autopilotCoderStudiedRuntimeContext.planContext",
        "plan context must carry the studied context pack ref",
      );
    }

    if (runtimeContext.dogfoodLift.customerPublicClaimAllowed !== false) {
      return yield* runtimeContextError(
        "autopilotCoderStudiedRuntimeContext.dogfoodLift",
        "studied runtime dogfood lift must not grant a customer public claim",
      );
    }

    if (runtimeContext.runtimeContextHash !== openAgentsAutopilotCoderStudiedRuntimeContextHash(runtimeContext)) {
      return yield* runtimeContextError(
        "autopilotCoderStudiedRuntimeContext.runtimeContextHash",
        "must match deterministic runtime context content hash",
      );
    }
  });
}

function uniqueRefs(refs: ReadonlyArray<string>): ReadonlyArray<string> {
  return [...new Set(refs.filter((ref) => ref.trim().length > 0))].sort((left, right) =>
    left.localeCompare(right),
  );
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0 ? runtimeContextError(path, "must be a non-empty string") : Effect.void;
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return /^sha256:[a-f0-9]{64}$/.test(value)
    ? Effect.void
    : runtimeContextError(path, "must be a sha256 hash ref");
}

function runtimeContextError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
