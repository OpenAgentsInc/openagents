import { defineIntent } from "@effect-native/core"
import { Schema } from "@effect-native/core/effect"

import {
  decodeKhalaFleetIntent,
  type ApprovalDecisionValue,
  type FleetRunControlAction,
  type FleetWorkerKind,
  type KhalaFleetIntent,
  type KhalaFleetIntentSurface,
} from "@openagentsinc/khala-fleet-intents"

// ---------------------------------------------------------------------------
// EN cockpit intents (MH-7 / EN-5)
//
// Two vocabularies meet here — deliberately kept single-source:
//
//   * The Effect Native UI intent algebra (`defineIntent`, dispatched by a
//     button's `onPress`). These are validated by Effect Native's own Schema
//     (from `@effect-native/core/effect`).
//   * The shared `@openagentsinc/khala-fleet-intents` vocabulary — the ONE
//     typed value that is BOTH the UI intent and the Khala Sync mutator.
//
// A dispatched EN intent is converted, here, into a fully-typed
// `KhalaFleetIntent` value and validated with `decodeKhalaFleetIntent`. That
// conversion is the "dispatching a control produces the correct typed intent"
// proof: the cockpit never hand-rolls a fleet mutator, it always goes through
// the shared decoder.
// ---------------------------------------------------------------------------

const runControlActionSchema = Schema.Literals([
  "pause",
  "resume",
  "drain",
  "stop",
])

export const CockpitRunControlIntent = defineIntent(
  "CockpitRunControl",
  Schema.Struct({
    action: runControlActionSchema,
    runRef: Schema.optional(Schema.String),
    reasonRef: Schema.optional(Schema.String),
  }),
)

export const CockpitApprovalDecisionIntent = defineIntent(
  "CockpitApprovalDecision",
  Schema.Struct({
    approvalRef: Schema.String,
    decision: Schema.Literals(["allow", "deny"]),
    reasonRef: Schema.optional(Schema.String),
  }),
)

export const CockpitWorkerSelectIntent = defineIntent(
  "CockpitWorkerSelect",
  Schema.Struct({
    workerKind: Schema.Literals(["codex", "claude", "grok", "auto"]),
    runRef: Schema.optional(Schema.String),
  }),
)

export const cockpitIntents = [
  CockpitRunControlIntent,
  CockpitApprovalDecisionIntent,
  CockpitWorkerSelectIntent,
] as const

// Context needed to stamp a shared KhalaFleetIntent. `now` / `newIntentId` are
// injectable so tests get deterministic values.
export type EnCockpitIntentContext = Readonly<{
  surface?: KhalaFleetIntentSurface
  deviceRef?: string
  userRef?: string
  now?: () => string
  newIntentId?: () => string
}>

const resolveContext = (ctx: EnCockpitIntentContext) => ({
  surface: ctx.surface ?? "desktop",
  deviceRef: ctx.deviceRef,
  userRef: ctx.userRef,
  now: ctx.now ?? (() => new Date().toISOString()),
  newIntentId:
    ctx.newIntentId ??
    (() =>
      globalThis.crypto?.randomUUID?.() ??
      `intent_${Math.random().toString(36).slice(2)}`),
})

const baseFields = (ctx: EnCockpitIntentContext) => {
  const resolved = resolveContext(ctx)
  const intentId = resolved.newIntentId()
  return {
    schema: "khala.fleet_intent.v1" as const,
    intentId,
    createdAt: resolved.now(),
    origin: {
      surface: resolved.surface,
      ...(resolved.deviceRef === undefined ? {} : { deviceRef: resolved.deviceRef }),
      ...(resolved.userRef === undefined ? {} : { userRef: resolved.userRef }),
    },
    idempotencyKey: intentId,
  }
}

export const runControlToFleetIntent = (
  payload: Readonly<{
    action: FleetRunControlAction
    runRef?: string | undefined
    reasonRef?: string | undefined
  }>,
  ctx: EnCockpitIntentContext = {},
): KhalaFleetIntent =>
  decodeKhalaFleetIntent({
    ...baseFields(ctx),
    kind: "fleet_run_control",
    action: payload.action,
    ...(payload.runRef === undefined ? {} : { runRef: payload.runRef }),
    ...(payload.reasonRef === undefined ? {} : { reasonRef: payload.reasonRef }),
  })

export const approvalToFleetIntent = (
  payload: Readonly<{
    approvalRef: string
    decision: ApprovalDecisionValue
    reasonRef?: string | undefined
    runRef?: string | undefined
  }>,
  ctx: EnCockpitIntentContext = {},
): KhalaFleetIntent =>
  decodeKhalaFleetIntent({
    ...baseFields(ctx),
    kind: "approval_decision",
    approvalRef: payload.approvalRef,
    decision: payload.decision,
    ...(payload.reasonRef === undefined ? {} : { reasonRef: payload.reasonRef }),
    ...(payload.runRef === undefined ? {} : { runRef: payload.runRef }),
  })

export const workerSelectToFleetIntent = (
  payload: Readonly<{ workerKind: FleetWorkerKind; runRef?: string | undefined }>,
  ctx: EnCockpitIntentContext = {},
): KhalaFleetIntent =>
  decodeKhalaFleetIntent({
    ...baseFields(ctx),
    kind: "worker_selection",
    workerKind: payload.workerKind,
    ...(payload.runRef === undefined ? {} : { runRef: payload.runRef }),
  })

// Central dispatch-name → shared KhalaFleetIntent converter. Unknown intent
// names throw (never a silent drop) so a mis-wired button is loud in tests.
export const cockpitEventToFleetIntent = (
  name: string,
  payload: unknown,
  ctx: EnCockpitIntentContext = {},
): KhalaFleetIntent => {
  const record = (payload ?? {}) as Record<string, unknown>
  switch (name) {
    case "CockpitRunControl":
      return runControlToFleetIntent(
        {
          action: record["action"] as FleetRunControlAction,
          runRef: record["runRef"] as string | undefined,
          reasonRef: record["reasonRef"] as string | undefined,
        },
        ctx,
      )
    case "CockpitApprovalDecision":
      return approvalToFleetIntent(
        {
          approvalRef: record["approvalRef"] as string,
          decision: record["decision"] as ApprovalDecisionValue,
          reasonRef: record["reasonRef"] as string | undefined,
          runRef: record["runRef"] as string | undefined,
        },
        ctx,
      )
    case "CockpitWorkerSelect":
      return workerSelectToFleetIntent(
        {
          workerKind: record["workerKind"] as FleetWorkerKind,
          runRef: record["runRef"] as string | undefined,
        },
        ctx,
      )
    default:
      throw new Error(`Unknown cockpit intent: ${name}`)
  }
}
