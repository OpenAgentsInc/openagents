// CL-53: Foldkit Commands for the desktop RPC verbs.
//
// Mirrors the web app idiom (e.g. apps/.../page/loggedIn/goals/commands.ts):
// `Command.define(name, argsSchema, ...ResultMessages)(args => Effect)`. Each
// command wraps the matching `rpc.request.<verb>` Promise (reached through the
// bridge module) in `Effect.tryPromise` and maps the result to a result Message.

import { Effect, Schema as S } from "effect"
import { Command } from "foldkit"

import { getRequest } from "./bridge"
import { commandErrorText } from "./helpers"
import {
  blockedDesktopProofReplayProjection,
  type DesktopProofReplayRequest,
  loadDesktopProofReplayProjection,
} from "../shared/proof-replays"
import type { OnboardingStatusResponse } from "../shared/rpc"
import { ProofReplayCommandRequest } from "./model"
import {
  FailedCoordinatorToggle,
  FailedBuiltInAgent,
  FailedAppleFmSession,
  FailedChatTurn,
  FailedComposerTurn,
  FailedSpawn,
  GotAppleFmReadiness,
  GotBuiltInAgentReadiness,
  GotInstallReadiness,
  GotOnboardingStatus,
  GotIdentityChoiceState,
  SettledChooseIdentity,
  GotPromiseSurfacingReadiness,
  GotPromiseSurfacingResult,
  GotProofReplayBundle,
  GotTrainingDashboard,
  GotTrainingEvidencePacketSummary,
  GotTrainingOperatorReadiness,
  GotTrainingPromiseGates,
  GotTrainingRuns,
  SettledCancelSession,
  SettledActivateTrainingWindow,
  SettledAdmitTrainingEvidence,
  SettledBuildTrainingEvidencePacket,
  SettledClaimTrainingLease,
  SettledCoordinatorToggle,
  SettledPlanTrainingWindow,
  SettledQueueTrainingCloseout,
  SettledQueueTrainingLaunch,
  SettledReconcileTrainingWindow,
  SettledRequestTrainingBootstrap,
  SettledManagedAccountMutation,
  SettledResolveApproval,
  SettledSubmitIntent,
  GotManagedAccounts,
  SucceededBuiltInAgent,
  SucceededAppleFmSession,
  SucceededChatTurn,
  SucceededComposerTurn,
  SucceededDeploy,
  SucceededSpawn,
  SucceededSwarmBatchSpawn,
  FailedSwarmBatchSpawn,
  GotPublicActivityTimeline,
} from "./message"

const errorText = commandErrorText

const refLine = (label: string, value: string | null): string =>
  value !== null && value.trim() !== ""
    ? `${label}: ${value.trim()}`
    : `${label}: not observed in the desktop projection yet`

const emptyTrainingDashboardProjection = (error: string) => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:training-dashboard",
  leaderboards: { blockerRefs: [], lanes: [] },
  a2: {
    blockerRefs: [],
    observedDeviceClassCount: 0,
    observedMeasurementCount: 0,
    verifiedMeasurementCount: 0,
  },
  a3: {
    blockerRefs: [],
    cellCount: 0,
    fitArtifactCount: 0,
    verifiedCellCount: 0,
  },
  a4: {
    blockerRefs: [],
    evalDeltaBonusBlockerRefs: [],
    observedVerifiedStages: [],
    requiredVerifiedStageCount: 0,
    shardCount: 0,
  },
  a5: {
    blockerRefs: [],
    evalSuiteCount: 0,
    updateBoundaryRef: null,
    verifiedSuiteCount: 0,
  },
  error,
})

const emptyTrainingPromiseGatesProjection = (error: string) => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  registryVersion: "",
  sourceUrl: "desktop:training-promise-gates",
  blockerRefs: [],
  promises: [],
  stateCounts: {
    degraded: 0,
    green: 0,
    planned: 0,
    red: 0,
    withdrawn: 0,
    yellow: 0,
    unknown: 0,
  },
  error,
})

const emptyTrainingOperatorReadinessProjection = (error: string) => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:training-operator-readiness",
  trainingBaseUrl: "unknown",
  adminEnabled: false,
  adminTokenPresent: false,
  adminReady: false,
  leaseEnabled: false,
  leaseReady: false,
  pylonRefPresent: false,
  pylonRefSource: "missing",
  pylonRef: null,
  pylonHomePresent: false,
  controlTokenPresent: false,
  localPylonReady: false,
  evidenceEnabled: false,
  evidencePacketPathPresent: false,
  evidenceReady: false,
  blockerRefs: ["desktop.training.operator_readiness_request_failed"],
  error,
})

const emptyTrainingEvidencePacketSummaryProjection = (error: string) => ({
  ok: false,
  configured: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:training-evidence-packet",
  packetSource: null,
  budgetLabel: null,
  budgetRefPresent: false,
  evalRefPresent: false,
  mergeRefPresent: false,
  finalValidationLoss: null,
  maxValidationLoss: null,
  lossPointCount: 0,
  freivaldsCommitmentRefCount: 0,
  gradientCloseoutRefCount: 0,
  evidenceRefCount: 0,
  receiptRefCount: 0,
  shardContributionCount: 0,
  distinctPylonCount: 0,
  blockerRefs: ["desktop.training.evidence_packet_request_failed"],
  error,
})

const emptyPublicActivityTimelineProjection = (error: string) => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:public-activity-timeline",
  envelope: null,
  error,
})

const generatedReplayLimitFrom = (value: string): number | undefined => {
  const trimmed = value.trim()
  if (trimmed === "") return undefined
  const parsed = Number(trimmed)
  return Number.isFinite(parsed) ? parsed : undefined
}

const proofReplayRequestFromCommand = (
  request: ProofReplayCommandRequest,
): DesktopProofReplayRequest =>
  request.mode === "catalog"
    ? { mode: "catalog", slug: request.slug }
    : {
        mode: "generated",
        filters: {
          actorRef: request.actorRef,
          from: request.from,
          kind: request.kind,
          limit: generatedReplayLimitFrom(request.limit),
          pairRef: request.pairRef,
          runRef: request.runRef,
          since: request.since,
          source: request.source,
          to: request.to,
          windowRef: request.windowRef,
        },
      }

const emptyBuiltInAgentReadinessProjection = (error: string) => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:builtin-agent-readiness",
  enabled: false,
  localPylonReady: false,
  hostedComputeConfigured: false,
  userApiKeyRequired: false as const,
  lane: "cloud-gcp" as const,
  modelSet: "unknown",
  maxSessionSeconds: 600,
  dailySessionCap: 0,
  dailySessionsUsed: 0,
  meteringLabel: "unavailable",
  worktreePathPresent: false,
  blockerRefs: ["desktop.builtin_agent.readiness_request_failed"],
  error,
})

const emptyAppleFmReadinessProjection = (error: string) => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:apple-fm-readiness" as const,
  localPylonReady: false,
  available: false,
  status: "unreachable",
  backendKind: "apple_fm_bridge",
  profileId: "apple-fm-local",
  model: "apple-foundation-model",
  capability: "probe.backend.apple_fm_bridge",
  advertisedCapabilities: [],
  baseUrl: "http://127.0.0.1:11435",
  platform: null,
  version: null,
  unavailableReason: "bridge_unreachable",
  message: error,
  blockerRefs: ["desktop.apple_fm.readiness_request_failed"],
  error,
})

const emptyInstallReadinessProjection = (error: string) => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:install-readiness" as const,
  platform: "unknown",
  arch: "unknown",
  runtime: "source" as const,
  nodeLaunchStatus: null,
  pylonHomePresent: false,
  controlTokenPresent: false,
  localPylonReady: false,
  builtInAgentReady: false,
  appleFmReady: false,
  userApiKeyRequired: false as const,
  autoUpdateEnabled: false,
  highestRoiAction: "Open Settings",
  blockerRefs: ["desktop.install_readiness.request_failed"],
  items: [
    {
      id: "install-readiness",
      label: "First-run health",
      status: "blocked" as const,
      detail: error,
      blockerRef: "desktop.install_readiness.request_failed",
    },
  ],
  error,
})

const emptyPromiseSurfacingReadinessProjection = (error: string) => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:promise-surfacing-readiness" as const,
  forumSlug: "product-promises" as const,
  baseUrl: "unknown",
  productPromisesUrl: "unknown",
  forumTopicsUrl: "unknown",
  agentTokenPresent: false,
  blockerRefs: ["desktop.promise_surfacing.readiness_request_failed"],
  error,
})

const emptyTrainingBootstrapProjection = (
  trainingRunRef: string,
  error: string,
) => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:training-bootstrap",
  pylonRef: null,
  trainingRunRef,
  outcome: null,
  reason: "request_failed",
  message: `training bootstrap grant failed: ${error}`,
  error,
})

const emptyTrainingEvidenceAdmissionProjection = (
  trainingRunRef: string,
  error: string,
) => ({
  ok: false,
  enabled: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:training-real-gradient-evidence",
  trainingRunRef,
  packetSource: null,
  run: null,
  realGradient: null,
  reason: "request_failed",
  message: `training evidence admission failed: ${error}`,
  evidenceRefCount: 0,
  receiptRefCount: 0,
  shardContributionCount: 0,
  distinctPylonCount: 0,
  error,
})

const emptyTrainingEvidencePacketBuildProjection = (
  trainingRunRef: string,
  error: string,
) => ({
  ok: false,
  enabled: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:training-evidence-packet-build",
  trainingRunRef,
  inputSource: null,
  packetSource: null,
  reason: "packet_write_failed",
  message: `training evidence packet build failed: ${error}`,
  summary: null,
  blockerRefs: ["desktop.training.evidence_packet_build_request_failed"],
  error,
})

// CL-51: pause/resume the coordinator loop.
export const SetCoordinatorPaused = Command.define(
  "SetCoordinatorPaused",
  { paused: S.Boolean },
  SettledCoordinatorToggle,
  FailedCoordinatorToggle,
)(({ paused }) =>
  Effect.tryPromise(() => getRequest().setCoordinatorPaused({ paused })).pipe(
    Effect.as(SettledCoordinatorToggle()),
    Effect.catch(() => Effect.succeed(FailedCoordinatorToggle())),
  ),
)

// CL-48 / CL-56: resolve a pending approval (approve / deny). Exactly-once on the
// node; a duplicate resolve comes back duplicate:true.
export const ResolveApproval = Command.define(
  "ResolveApproval",
  { approvalRef: S.String, decision: S.Literals(["approve", "deny"]) },
  SettledResolveApproval,
)(({ approvalRef, decision }) =>
  Effect.tryPromise(() => getRequest().resolveApproval({ approvalRef, decision })).pipe(
    Effect.map((result) =>
      SettledResolveApproval({
        approvalRef,
        ok: result.applied || result.duplicate,
      }),
    ),
    Effect.catch(() =>
      Effect.succeed(SettledResolveApproval({ approvalRef, ok: false })),
    ),
  ),
)

// CL-26: trigger a deploy. The node fail-safe-gates execution behind
// OA_DEPLOY_ENABLE=1; an ungated request comes back accepted:false /
// reason:"deploy_disabled".
export const DeployCloud = Command.define(
  "DeployCloud",
  {},
  SucceededDeploy,
)(() =>
  Effect.tryPromise(() =>
    getRequest().deployCloud({ target: "cloudrun", ref: "main", env: "production" }),
  ).pipe(
    Effect.map((r) =>
      r.accepted
        ? SucceededDeploy({ state: "queued", text: "queued · cloudrun · main" })
        : r.reason === "deploy_disabled"
          ? SucceededDeploy({
              state: "unknown",
              text: "disabled (set OA_DEPLOY_ENABLE=1 on the node)",
            })
          : SucceededDeploy({
              state: "failed",
              text: `not accepted: ${r.errors[0] ?? r.reason}`,
            }),
    ),
    Effect.catch((error) =>
      Effect.succeed(
        SucceededDeploy({ state: "failed", text: `error: ${errorText(error)}` }),
      ),
    ),
  ),
)

// CL-47: submit a work intent ("ask").
export const SubmitIntent = Command.define(
  "SubmitIntent",
  { title: S.String, body: S.String },
  SettledSubmitIntent,
)(({ title, body }) =>
  Effect.tryPromise(() => getRequest().submitIntent({ title, body })).pipe(
    Effect.map((r) =>
      r.ok
        ? SettledSubmitIntent({ ok: true, text: `sent · ${r.status}` })
        : SettledSubmitIntent({ ok: false, text: `error: ${r.error ?? r.status}` }),
    ),
    Effect.catch((error) =>
      Effect.succeed(
        SettledSubmitIntent({ ok: false, text: `error: ${errorText(error)}` }),
      ),
    ),
  ),
)

export const LoadBuiltInAgentReadiness = Command.define(
  "LoadBuiltInAgentReadiness",
  {},
  GotBuiltInAgentReadiness,
)(() =>
  Effect.tryPromise(() => getRequest().builtinAgentReadiness({})).pipe(
    Effect.map((projection) => GotBuiltInAgentReadiness({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotBuiltInAgentReadiness({
          projection: emptyBuiltInAgentReadinessProjection(errorText(error)),
        }),
      ),
    ),
  ),
)

export const StartBuiltInAgent = Command.define(
  "StartBuiltInAgent",
  {},
  SucceededBuiltInAgent,
  FailedBuiltInAgent,
)(() =>
  Effect.tryPromise(() => getRequest().startBuiltInAgent({})).pipe(
    Effect.map((result) =>
      result.ok
        ? SucceededBuiltInAgent({ sessionRef: result.sessionRef })
        : FailedBuiltInAgent({
            error:
              result.error ??
              result.readiness.blockerRefs[0] ??
              "built-in agent unavailable",
          }),
    ),
    Effect.catch((error) =>
      Effect.succeed(FailedBuiltInAgent({ error: errorText(error) })),
    ),
  ),
)

export const LoadAppleFmReadiness = Command.define(
  "LoadAppleFmReadiness",
  {},
  GotAppleFmReadiness,
)(() =>
  Effect.tryPromise(() => getRequest().appleFmReadiness({})).pipe(
    Effect.map((projection) => GotAppleFmReadiness({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotAppleFmReadiness({
          projection: emptyAppleFmReadinessProjection(errorText(error)),
        }),
      ),
    ),
  ),
)

export const StartAppleFmSession = Command.define(
  "StartAppleFmSession",
  {},
  SucceededAppleFmSession,
  FailedAppleFmSession,
)(() =>
  Effect.tryPromise(() => getRequest().startAppleFmSession({})).pipe(
    Effect.map((result) =>
      result.ok
        ? SucceededAppleFmSession({ sessionRef: result.sessionRef })
        : FailedAppleFmSession({
            error:
              result.error ??
              result.blockerRefs[0] ??
              result.readiness.blockerRefs[0] ??
              "local Apple FM unavailable",
          }),
    ),
    Effect.catch((error) =>
      Effect.succeed(FailedAppleFmSession({ error: errorText(error) })),
    ),
  ),
)

export const LoadInstallReadiness = Command.define(
  "LoadInstallReadiness",
  {},
  GotInstallReadiness,
)(() =>
  Effect.tryPromise(() => getRequest().installReadiness({})).pipe(
    Effect.map((projection) => GotInstallReadiness({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotInstallReadiness({
          projection: emptyInstallReadinessProjection(errorText(error)),
        }),
      ),
    ),
  ),
)

// AO-4 (#5445): load the live onboarding chain status (wizard steps). Fail-soft:
// a failed request keeps the full ladder visible and marks only the status read
// itself as retryable, rather than inventing a failed local-node step.
export const degradedOnboardingProjection = (error: string): OnboardingStatusResponse => ({
  ok: false,
  fetchedAt: new Date().toISOString(),
  sourceUrl: "desktop:onboarding-status" as const,
  complete: false,
  currentStepId: "wallet",
  hasRetryableFailure: true,
  steps: [
    {
      id: "identity",
      label: "Identity",
      status: "done" as const,
      message: "Identity status was already loaded by the local app.",
      retryable: false,
    },
    {
      id: "registered",
      label: "Agent registered",
      status: "pending" as const,
      message: "Waiting for a fresh onboarding status read.",
      retryable: false,
    },
    {
      id: "node-online",
      label: "Node online",
      status: "active" as const,
      message: `Status read timed out: ${error}. Retry the status refresh.`,
      retryable: false,
    },
    {
      id: "wallet",
      label: "Wallet receive-ready",
      status: "pending" as const,
      message: "Waiting for a fresh wallet status read.",
      retryable: false,
    },
    {
      id: "payout",
      label: "Payout target registered",
      status: "pending" as const,
      message: "Waiting for a fresh status read.",
      retryable: false,
    },
    {
      id: "presence",
      label: "Presence live",
      status: "pending" as const,
      message: "Waiting for a fresh status read.",
      retryable: false,
    },
    {
      id: "tassadar",
      label: "Joined Tassadar",
      status: "pending" as const,
      message: "Waiting for a fresh assignment status read.",
      retryable: false,
    },
    {
      id: "claimed",
      label: "First work claimed",
      status: "pending" as const,
      message: "Waiting for a fresh assignment status read.",
      retryable: false,
    },
    {
      id: "earned",
      label: "First sats earned",
      status: "pending" as const,
      message: "Waiting for a fresh wallet balance read.",
      retryable: false,
    },
  ],
})

export const LoadOnboardingStatus = Command.define(
  "LoadOnboardingStatus",
  {},
  GotOnboardingStatus,
)(() =>
  Effect.tryPromise(() => getRequest().onboardingStatus({})).pipe(
    Effect.map((projection) => GotOnboardingStatus({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotOnboardingStatus({
          projection: degradedOnboardingProjection(errorText(error)),
        }),
      ),
    ),
  ),
)

// AO-3 (#5444): load the first-run identity-choice state (detect existing vs ask).
const emptyIdentityChoiceState = () => ({
  choiceNeeded: false,
  detected: { present: false, shortLabel: null, npub: null, source: null },
  chosen: null,
  createNewAvailable: true as const,
})

export const LoadIdentityChoiceState = Command.define(
  "LoadIdentityChoiceState",
  {},
  GotIdentityChoiceState,
)(() =>
  Effect.tryPromise(() => getRequest().identityChoiceState({})).pipe(
    Effect.map((state) => GotIdentityChoiceState({ state })),
    Effect.catch(() =>
      Effect.succeed(GotIdentityChoiceState({ state: emptyIdentityChoiceState() })),
    ),
  ),
)

// AO-3 (#5444): record the user's identity choice. The Bun host re-verifies the
// seed marker and never overwrites a home. On settle the wizard re-loads status.
export const ChooseIdentity = Command.define(
  "ChooseIdentity",
  {
    kind: S.Literals(["use_existing", "create_new"]),
    displayName: S.String,
  },
  SettledChooseIdentity,
)((input) =>
  Effect.tryPromise(() =>
    getRequest().chooseIdentity(
      input.kind === "create_new"
        ? { kind: "create_new", displayName: input.displayName }
        : { kind: "use_existing" },
    ),
  ).pipe(
    Effect.map((result) => SettledChooseIdentity({ result })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledChooseIdentity({
          result: {
            ok: false,
            state: emptyIdentityChoiceState(),
            error: errorText(error),
          },
        }),
      ),
    ),
  ),
)

export const LoadPromiseSurfacingReadiness = Command.define(
  "LoadPromiseSurfacingReadiness",
  {},
  GotPromiseSurfacingReadiness,
)(() =>
  Effect.tryPromise(() => getRequest().promiseSurfacingReadiness({})).pipe(
    Effect.map((projection) => GotPromiseSurfacingReadiness({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotPromiseSurfacingReadiness({
          projection: emptyPromiseSurfacingReadinessProjection(errorText(error)),
        }),
      ),
    ),
  ),
)

export const SurfacePromiseGap = Command.define(
  "SurfacePromiseGap",
  {
    promiseId: S.String,
    surface: S.String,
    claimText: S.String,
    expectedBehavior: S.String,
    observedBehavior: S.String,
    evidenceOrSteps: S.String,
    environment: S.String,
    impact: S.String,
    suggestedState: S.Literals([
      "green",
      "yellow",
      "red",
      "degraded",
      "planned",
      "unknown",
    ]),
  },
  GotPromiseSurfacingResult,
)((input) =>
  Effect.tryPromise(() => getRequest().surfacePromiseGap(input)).pipe(
    Effect.map((projection) => GotPromiseSurfacingResult({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotPromiseSurfacingResult({
          projection: {
            ok: false,
            mode: "blocked" as const,
            draft: null,
            blockerRefs: [
              "desktop.promise_surfacing.submit_request_failed",
            ],
            error: errorText(error),
          },
        }),
      ),
    ),
  ),
)

export const LoadTrainingRuns = Command.define(
  "LoadTrainingRuns",
  {},
  GotTrainingRuns,
)(() =>
  Effect.tryPromise(() => getRequest().listTrainingRuns({})).pipe(
    Effect.map((projection) => GotTrainingRuns({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotTrainingRuns({
          projection: {
            ok: false,
            fetchedAt: new Date().toISOString(),
            sourceUrl: "desktop:training-runs",
            runs: [],
            summaries: [],
            error: errorText(error),
          },
        }),
      ),
    ),
  ),
)

export const LoadTrainingDashboard = Command.define(
  "LoadTrainingDashboard",
  {},
  GotTrainingDashboard,
)(() =>
  Effect.tryPromise(() => getRequest().listTrainingDashboard({})).pipe(
    Effect.map((projection) => GotTrainingDashboard({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotTrainingDashboard({
          projection: emptyTrainingDashboardProjection(errorText(error)),
        }),
      ),
    ),
  ),
)

export const LoadTrainingPromiseGates = Command.define(
  "LoadTrainingPromiseGates",
  {},
  GotTrainingPromiseGates,
)(() =>
  Effect.tryPromise(() => getRequest().listTrainingPromiseGates({})).pipe(
    Effect.map((projection) => GotTrainingPromiseGates({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotTrainingPromiseGates({
          projection: emptyTrainingPromiseGatesProjection(errorText(error)),
        }),
      ),
    ),
  ),
)

export const LoadTrainingOperatorReadiness = Command.define(
  "LoadTrainingOperatorReadiness",
  {},
  GotTrainingOperatorReadiness,
)(() =>
  Effect.tryPromise(() => getRequest().listTrainingOperatorReadiness({})).pipe(
    Effect.map((projection) => GotTrainingOperatorReadiness({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotTrainingOperatorReadiness({
          projection: emptyTrainingOperatorReadinessProjection(errorText(error)),
        }),
      ),
    ),
  ),
)

export const LoadTrainingEvidencePacketSummary = Command.define(
  "LoadTrainingEvidencePacketSummary",
  {},
  GotTrainingEvidencePacketSummary,
)(() =>
  Effect.tryPromise(() =>
    getRequest().listTrainingEvidencePacketSummary({}),
  ).pipe(
    Effect.map((projection) => GotTrainingEvidencePacketSummary({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotTrainingEvidencePacketSummary({
          projection: emptyTrainingEvidencePacketSummaryProjection(
            errorText(error),
          ),
        }),
      ),
    ),
  ),
)

export const LoadPublicActivityTimeline = Command.define(
  "LoadPublicActivityTimeline",
  {},
  GotPublicActivityTimeline,
)(() =>
  Effect.tryPromise(() => getRequest().listPublicActivityTimeline({})).pipe(
    Effect.map((projection) => GotPublicActivityTimeline({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotPublicActivityTimeline({
          projection: emptyPublicActivityTimelineProjection(errorText(error)),
        }),
      ),
    ),
  ),
)

export const LoadProofReplayBundle = Command.define(
  "LoadProofReplayBundle",
  { request: ProofReplayCommandRequest },
  GotProofReplayBundle,
)(({ request }) => {
  const replayRequest = proofReplayRequestFromCommand(request)
  return Effect.tryPromise(() => loadDesktopProofReplayProjection(replayRequest)).pipe(
    Effect.map((projection) => GotProofReplayBundle({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotProofReplayBundle({
          projection: blockedDesktopProofReplayProjection(
            replayRequest,
            errorText(error),
          ),
        }),
      ),
    ),
  )
})

export const PlanTrainingRunWindow = Command.define(
  "PlanTrainingRunWindow",
  {},
  SettledPlanTrainingWindow,
)(() =>
  Effect.tryPromise(() => getRequest().planTrainingRunWindow({})).pipe(
    Effect.map((projection) => SettledPlanTrainingWindow({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledPlanTrainingWindow({
          projection: {
            ok: false,
            enabled: false,
            fetchedAt: new Date().toISOString(),
            sourceUrl: "desktop:training-plan",
            trainingRunRef: null,
            windowRef: null,
            run: null,
            window: null,
            runPlanned: false,
            windowPlanned: false,
            reason: "request_failed",
            message: `training admin request failed: ${errorText(error)}`,
            error: errorText(error),
          },
        }),
      ),
    ),
  ),
)

export const ActivateTrainingWindow = Command.define(
  "ActivateTrainingWindow",
  { windowRef: S.String },
  SettledActivateTrainingWindow,
)(({ windowRef }) =>
  Effect.tryPromise(() => getRequest().activateTrainingWindow({ windowRef })).pipe(
    Effect.map((projection) => SettledActivateTrainingWindow({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledActivateTrainingWindow({
          projection: {
            ok: false,
            enabled: false,
            fetchedAt: new Date().toISOString(),
            sourceUrl: "desktop:training-activation",
            windowRef,
            window: null,
            reason: "request_failed",
            message: `training admin activation failed: ${errorText(error)}`,
            error: errorText(error),
          },
        }),
      ),
    ),
  ),
)

export const ReconcileTrainingWindow = Command.define(
  "ReconcileTrainingWindow",
  { windowRef: S.String },
  SettledReconcileTrainingWindow,
)(({ windowRef }) =>
  Effect.tryPromise(() => getRequest().reconcileTrainingWindow({ windowRef })).pipe(
    Effect.map((projection) => SettledReconcileTrainingWindow({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledReconcileTrainingWindow({
          projection: {
            ok: false,
            enabled: false,
            fetchedAt: new Date().toISOString(),
            sourceUrl: "desktop:training-reconcile",
            windowRef,
            window: null,
            reason: "request_failed",
            message: `training admin reconciliation failed: ${errorText(error)}`,
            error: errorText(error),
          },
        }),
      ),
    ),
  ),
)

export const ClaimTrainingWindowLease = Command.define(
  "ClaimTrainingWindowLease",
  {},
  SettledClaimTrainingLease,
)(() =>
  Effect.tryPromise(() => getRequest().claimTrainingWindowLease({})).pipe(
    Effect.map((projection) => SettledClaimTrainingLease({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledClaimTrainingLease({
          projection: {
            ok: false,
            enabled: false,
            fetchedAt: new Date().toISOString(),
            sourceUrl: "desktop:training-lease",
            pylonRef: null,
            lease: null,
            reason: "request_failed",
            message: `training lease claim failed: ${errorText(error)}`,
            error: errorText(error),
          },
        }),
      ),
    ),
  ),
)

export const RequestTrainingBootstrapGrant = Command.define(
  "RequestTrainingBootstrapGrant",
  { trainingRunRef: S.String },
  SettledRequestTrainingBootstrap,
)(({ trainingRunRef }) =>
  Effect.tryPromise(() =>
    getRequest().requestTrainingBootstrapGrant({ trainingRunRef }),
  ).pipe(
    Effect.map((projection) => SettledRequestTrainingBootstrap({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledRequestTrainingBootstrap({
          projection: emptyTrainingBootstrapProjection(
            trainingRunRef,
            errorText(error),
          ),
        }),
      ),
    ),
  ),
)

export const AdmitTrainingRealGradientEvidence = Command.define(
  "AdmitTrainingRealGradientEvidence",
  { trainingRunRef: S.String },
  SettledAdmitTrainingEvidence,
)(({ trainingRunRef }) =>
  Effect.tryPromise(() =>
    getRequest().admitTrainingRealGradientEvidence({ trainingRunRef }),
  ).pipe(
    Effect.map((projection) => SettledAdmitTrainingEvidence({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledAdmitTrainingEvidence({
          projection: emptyTrainingEvidenceAdmissionProjection(
            trainingRunRef,
            errorText(error),
          ),
        }),
      ),
    ),
  ),
)

export const BuildTrainingEvidencePacket = Command.define(
  "BuildTrainingEvidencePacket",
  { trainingRunRef: S.String },
  SettledBuildTrainingEvidencePacket,
)(({ trainingRunRef }) =>
  Effect.tryPromise(() =>
    getRequest().buildTrainingEvidencePacket({ trainingRunRef }),
  ).pipe(
    Effect.map((projection) =>
      SettledBuildTrainingEvidencePacket({ projection }),
    ),
    Effect.catch((error) =>
      Effect.succeed(
        SettledBuildTrainingEvidencePacket({
          projection: emptyTrainingEvidencePacketBuildProjection(
            trainingRunRef,
            errorText(error),
          ),
        }),
      ),
    ),
  ),
)

// Queue a training launch/readiness check through the existing Pylon intent
// bridge. This intentionally does not call admin training routes from the
// webview; main-process authority can later translate the intent into an
// authenticated run plan once the node exposes that command.
export const QueueTrainingLaunch = Command.define(
  "QueueTrainingLaunch",
  {},
  SettledQueueTrainingLaunch,
)(() =>
  Effect.tryPromise(() =>
    getRequest().submitIntent({
      title: "Training run launch check",
      body: [
        "Inspect readiness for a Tassadar/Psion training run.",
        "Use the OpenAgents training authority projections at /api/training/runs and the issue 4855 gates.",
        "Confirm R1 rehearsal readiness, last durable seal bootstrap, seal barrier state, staleness acceptance bound, distinct contributor evidence, Freivalds refs, gradient closeout refs, receipts, and settlement blockers.",
        "Do not bypass admin-only training routes from the desktop webview.",
      ].join("\n"),
    }),
  ).pipe(
    Effect.map((r) =>
      r.ok
        ? SettledQueueTrainingLaunch({ ok: true, text: `queued · ${r.status}` })
        : SettledQueueTrainingLaunch({
            ok: false,
            text: `error: ${r.error ?? r.status}`,
          }),
    ),
    Effect.catch((error) =>
      Effect.succeed(
        SettledQueueTrainingLaunch({
          ok: false,
          text: `error: ${errorText(error)}`,
        }),
      ),
    ),
  ),
)

// Queue a worker closeout/evidence preparation task through local Pylon. This
// is intentionally not a `/seal` call: seal metadata must come from real worker
// output and Bun-side evidence admission, not from the Foldkit webview.
export const QueueTrainingCloseout = Command.define(
  "QueueTrainingCloseout",
  {
    trainingRunRef: S.String,
    windowRef: S.NullOr(S.String),
    leaseRef: S.NullOr(S.String),
    bootstrapGrantRef: S.NullOr(S.String),
  },
  SettledQueueTrainingCloseout,
)(({ trainingRunRef, windowRef, leaseRef, bootstrapGrantRef }) =>
  Effect.tryPromise(() =>
    getRequest().submitIntent({
      title: "Training closeout evidence packet",
      body: [
        "Prepare a public-safe closeout packet for the selected Tassadar/Psion training run.",
        refLine("trainingRunRef", trainingRunRef),
        refLine("windowRef", windowRef),
        refLine("leaseRef", leaseRef),
        refLine("bootstrapGrantRef", bootstrapGrantRef),
        "Collect real worker output refs only: checkpoint digest, checkpoint artifact, merge ref, eval ref, Freivalds commitments, gradient closeouts, loss curve, shard receipts, and settlement blockers.",
        "Do not fabricate seal metadata or call admin-only evidence routes from the desktop webview.",
        "Return the refs and blocker list needed for a later Bun-side evidence-admission bridge.",
      ].join("\n"),
    }),
  ).pipe(
    Effect.map((r) =>
      r.ok
        ? SettledQueueTrainingCloseout({ ok: true, text: `queued · ${r.status}` })
        : SettledQueueTrainingCloseout({
            ok: false,
            text: `error: ${r.error ?? r.status}`,
          }),
    ),
    Effect.catch((error) =>
      Effect.succeed(
        SettledQueueTrainingCloseout({
          ok: false,
          text: `error: ${errorText(error)}`,
        }),
      ),
    ),
  ),
)

// CL-57: spawn a bounded session directly.
export const SpawnSession = Command.define(
  "SpawnSession",
  {
    adapter: S.Literals(["codex", "claude_agent"]),
    objective: S.String,
    verify: S.Array(S.String),
    // #4998: requested execution lane (auto|local|cloud-gcp|cloud-shc).
    lane: S.Literals(["auto", "local", "cloud-gcp", "cloud-shc"]),
    // CS-A1: per-session provider account (null = node default selection).
    accountRef: S.NullOr(S.String),
  },
  SucceededSpawn,
  FailedSpawn,
)(({ adapter, objective, verify, lane, accountRef }) =>
  Effect.tryPromise(() =>
    getRequest().spawnSession({
      adapter,
      objective,
      verify: verify.length > 0 ? [...verify] : undefined,
      lane,
      ...(accountRef !== null && accountRef.trim() !== ""
        ? { accountRef: accountRef.trim() }
        : {}),
    }),
  ).pipe(
    Effect.map((r) =>
      r.ok
        ? SucceededSpawn({ sessionRef: r.sessionRef })
        : FailedSpawn({ error: r.error ?? "spawn failed" }),
    ),
    Effect.catch((error) => Effect.succeed(FailedSpawn({ error: errorText(error) }))),
  ),
)

// #5469 (EPIC #5461): spawn ONE session as part of a bounded swarm batch. Same
// `session.spawn` verb as the Spawn/Composer panes (NO new contract / no
// `sessions batch` wire verb) — it just maps to the batch-specific result
// messages so the swarm reducer's bounded-concurrency queue can pull the next
// objective as each spawn settles. The desktop is the batch orchestrator over
// the one spawn verb the control protocol already exposes.
export const SpawnBatchSession = Command.define(
  "SpawnBatchSession",
  {
    adapter: S.Literals(["codex", "claude_agent"]),
    objective: S.String,
    verify: S.Array(S.String),
    lane: S.Literals(["auto", "local", "cloud-gcp", "cloud-shc"]),
    accountRef: S.NullOr(S.String),
  },
  SucceededSwarmBatchSpawn,
  FailedSwarmBatchSpawn,
)(({ adapter, objective, verify, lane, accountRef }) =>
  Effect.tryPromise(() =>
    getRequest().spawnSession({
      adapter,
      objective,
      verify: verify.length > 0 ? [...verify] : undefined,
      lane,
      ...(accountRef !== null && accountRef.trim() !== ""
        ? { accountRef: accountRef.trim() }
        : {}),
    }),
  ).pipe(
    Effect.map((r) =>
      r.ok
        ? SucceededSwarmBatchSpawn({ sessionRef: r.sessionRef })
        : FailedSwarmBatchSpawn({ error: r.error ?? "spawn failed" }),
    ),
    Effect.catch((error) =>
      Effect.succeed(FailedSwarmBatchSpawn({ error: errorText(error) })),
    ),
  ),
)

// CL-52: cancel a running/queued session.
export const CancelSession = Command.define(
  "CancelSession",
  { sessionRef: S.String },
  SettledCancelSession,
)(({ sessionRef }) =>
  Effect.tryPromise(() => getRequest().cancelSession({ sessionRef })).pipe(
    Effect.as(SettledCancelSession()),
    Effect.catch(() => Effect.succeed(SettledCancelSession())),
  ),
)

// #5355: spawn one coding-composer turn. Same `session.spawn` verb as the Spawn
// pane (no new contract), but it carries the composer's repo/worktree path and
// maps to the composer-specific settled/failed messages so the iterative loop's
// state (active session ref + turn history) updates distinctly. Reply/continue
// turns reuse this command with a continuation objective (see helpers).
export const SpawnComposerTurn = Command.define(
  "SpawnComposerTurn",
  {
    adapter: S.Literals(["codex", "claude_agent"]),
    objective: S.String,
    verify: S.Array(S.String),
    lane: S.Literals(["auto", "local", "cloud-gcp", "cloud-shc"]),
    worktreePath: S.NullOr(S.String),
    // CS-A1: per-session provider account (null = node default selection).
    accountRef: S.NullOr(S.String),
  },
  SucceededComposerTurn,
  FailedComposerTurn,
)(({ adapter, objective, verify, lane, worktreePath, accountRef }) =>
  Effect.tryPromise(() =>
    getRequest().spawnSession({
      adapter,
      objective,
      verify: verify.length > 0 ? [...verify] : undefined,
      lane,
      ...(worktreePath !== null && worktreePath.trim() !== ""
        ? { worktreePath: worktreePath.trim() }
        : {}),
      ...(accountRef !== null && accountRef.trim() !== ""
        ? { accountRef: accountRef.trim() }
        : {}),
    }),
  ).pipe(
    Effect.map((r) =>
      r.ok
        ? SucceededComposerTurn({ sessionRef: r.sessionRef })
        : FailedComposerTurn({ error: r.error ?? "spawn failed" }),
    ),
    Effect.catch((error) =>
      Effect.succeed(FailedComposerTurn({ error: errorText(error) })),
    ),
  ),
)

// #5453: spawn one Blueprint chat turn. This is deliberately another wrapper
// around session.spawn, not a new desktop RPC verb; the chat pane owns distinct
// result messages so its transcript can settle independently from Composer.
export const SpawnChatTurn = Command.define(
  "SpawnChatTurn",
  {
    adapter: S.Literals(["codex", "claude_agent"]),
    objective: S.String,
    verify: S.Array(S.String),
    lane: S.Literals(["auto", "local", "cloud-gcp", "cloud-shc"]),
    accountRef: S.NullOr(S.String),
  },
  SucceededChatTurn,
  FailedChatTurn,
)(({ adapter, objective, verify, lane, accountRef }) =>
  Effect.tryPromise(() =>
    getRequest().spawnSession({
      adapter,
      objective,
      verify: verify.length > 0 ? [...verify] : undefined,
      lane,
      ...(accountRef !== null && accountRef.trim() !== ""
        ? { accountRef: accountRef.trim() }
        : {}),
    }),
  ).pipe(
    Effect.map((r) =>
      r.ok
        ? SucceededChatTurn({ sessionRef: r.sessionRef })
        : FailedChatTurn({ error: r.error ?? "spawn failed" }),
    ),
    Effect.catch((error) =>
      Effect.succeed(FailedChatTurn({ error: errorText(error) })),
    ),
  ),
)

// CS-A1: spawn a composer coding turn under the LOCAL Apple FM runtime. Apple FM
// has its own control verb (apple_fm.session.start), so it is a separate command
// from SpawnComposerTurn (which uses session.spawn). It still returns a
// sessionRef the composer tails the same way, so it maps to the shared
// Succeeded/Failed composer-turn messages.
export const SpawnAppleFmComposerTurn = Command.define(
  "SpawnAppleFmComposerTurn",
  {
    objective: S.String,
    worktreePath: S.NullOr(S.String),
  },
  SucceededComposerTurn,
  FailedComposerTurn,
)(({ objective, worktreePath }) =>
  Effect.tryPromise(() =>
    getRequest().spawnAppleFmSession({
      objective,
      ...(worktreePath !== null && worktreePath.trim() !== ""
        ? { worktreePath: worktreePath.trim() }
        : {}),
    }),
  ).pipe(
    Effect.map((r) =>
      r.ok
        ? SucceededComposerTurn({ sessionRef: r.sessionRef })
        : FailedComposerTurn({
            error:
              r.error ??
              r.blockerRefs[0] ??
              r.readiness.blockerRefs[0] ??
              "local Apple FM unavailable",
          }),
    ),
    Effect.catch((error) =>
      Effect.succeed(FailedComposerTurn({ error: errorText(error) })),
    ),
  ),
)

// ── CS-A1: account-management commands (node-local dev.accounts config) ──────
export const LoadManagedAccounts = Command.define(
  "LoadManagedAccounts",
  {},
  GotManagedAccounts,
)(() =>
  Effect.tryPromise(() => getRequest().listManagedAccounts({})).pipe(
    Effect.map((projection) => GotManagedAccounts({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        GotManagedAccounts({
          projection: { ok: false, accounts: [], error: errorText(error) },
        }),
      ),
    ),
  ),
)

export const AddManagedAccount = Command.define(
  "AddManagedAccount",
  {
    ref: S.String,
    provider: S.Literals(["codex", "claude_agent"]),
    home: S.String,
    priority: S.NullOr(S.Number),
  },
  SettledManagedAccountMutation,
)(({ ref, provider, home, priority }) =>
  Effect.tryPromise(() =>
    getRequest().addManagedAccount({
      ref,
      provider,
      home,
      ...(priority !== null ? { priority } : {}),
    }),
  ).pipe(
    Effect.map((projection) => SettledManagedAccountMutation({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledManagedAccountMutation({
          projection: { ok: false, accounts: [], error: errorText(error) },
        }),
      ),
    ),
  ),
)

export const RemoveManagedAccount = Command.define(
  "RemoveManagedAccount",
  { ref: S.String, provider: S.Literals(["codex", "claude_agent"]) },
  SettledManagedAccountMutation,
)(({ ref, provider }) =>
  Effect.tryPromise(() => getRequest().removeManagedAccount({ ref, provider })).pipe(
    Effect.map((projection) => SettledManagedAccountMutation({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledManagedAccountMutation({
          projection: { ok: false, accounts: [], error: errorText(error) },
        }),
      ),
    ),
  ),
)

export const SetManagedAccountPriority = Command.define(
  "SetManagedAccountPriority",
  {
    ref: S.String,
    provider: S.Literals(["codex", "claude_agent"]),
    priority: S.Number,
  },
  SettledManagedAccountMutation,
)(({ ref, provider, priority }) =>
  Effect.tryPromise(() =>
    getRequest().setManagedAccountPriority({ ref, provider, priority }),
  ).pipe(
    Effect.map((projection) => SettledManagedAccountMutation({ projection })),
    Effect.catch((error) =>
      Effect.succeed(
        SettledManagedAccountMutation({
          projection: { ok: false, accounts: [], error: errorText(error) },
        }),
      ),
    ),
  ),
)
