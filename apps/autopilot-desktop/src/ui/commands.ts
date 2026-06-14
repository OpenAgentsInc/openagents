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
  GotTrainingRuns,
  SettledCancelSession,
  SettledCoordinatorToggle,
  SettledQueueTrainingLaunch,
  SettledResolveApproval,
  SettledSubmitIntent,
  SucceededDeploy,
  SucceededSpawn,
} from "./message"

const errorText = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

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
