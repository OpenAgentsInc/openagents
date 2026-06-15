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
  FailedCoordinatorToggle,
  FailedBuiltInAgent,
  FailedSpawn,
  GotAppleFmReadiness,
  GotBuiltInAgentReadiness,
  GotInstallReadiness,
  GotPromiseSurfacingReadiness,
  GotPromiseSurfacingResult,
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
  SettledResolveApproval,
  SettledSubmitIntent,
  SucceededBuiltInAgent,
  SucceededDeploy,
  SucceededSpawn,
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
  },
  SucceededSpawn,
  FailedSpawn,
)(({ adapter, objective, verify, lane }) =>
  Effect.tryPromise(() =>
    getRequest().spawnSession({
      adapter,
      objective,
      verify: verify.length > 0 ? [...verify] : undefined,
      lane,
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
