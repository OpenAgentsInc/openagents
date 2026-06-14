// CL-53: Foldkit Commands for the desktop RPC verbs.
//
// Mirrors the web app idiom (e.g. apps/.../page/loggedIn/goals/commands.ts):
// `Command.define(name, argsSchema, ...ResultMessages)(args => Effect)`. Each
// command wraps the matching `rpc.request.<verb>` Promise (reached through the
// bridge module) in `Effect.tryPromise` and maps the result to a result Message.

import { Effect, Schema as S } from "effect"
import { Command } from "foldkit"

import { getRequest } from "./bridge"
import {
  FailedCoordinatorToggle,
  FailedSpawn,
  GotTrainingDashboard,
  GotTrainingOperatorReadiness,
  GotTrainingPromiseGates,
  GotTrainingRuns,
  SettledCancelSession,
  SettledActivateTrainingWindow,
  SettledClaimTrainingLease,
  SettledCoordinatorToggle,
  SettledPlanTrainingWindow,
  SettledQueueTrainingCloseout,
  SettledQueueTrainingLaunch,
  SettledReconcileTrainingWindow,
  SettledRequestTrainingBootstrap,
  SettledResolveApproval,
  SettledSubmitIntent,
  SucceededDeploy,
  SucceededSpawn,
} from "./message"

const errorText = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

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
  blockerRefs: ["desktop.training.operator_readiness_request_failed"],
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
  },
  SucceededSpawn,
  FailedSpawn,
)(({ adapter, objective, verify }) =>
  Effect.tryPromise(() =>
    getRequest().spawnSession({
      adapter,
      objective,
      verify: verify.length > 0 ? [...verify] : undefined,
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
