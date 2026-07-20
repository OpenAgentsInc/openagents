import { Schema as S } from "effect"

import {
  OwnerBoundCandidateSet,
  SafeTurnProjection,
  TurnIntent,
  TurnReceipt,
  TurnRequestRef,
  TurnThreadRef,
} from "@openagentsinc/agent-runtime-schema"

/**
 * AFS-01 typed turn IPC contract (main <-> renderer).
 *
 * The renderer can send one intent and render one projection. It cannot select a
 * hidden provider, build an authoritative prompt, or claim an action. Every
 * pushed frame is bounded and fenced with request and generation identity, so a
 * late frame from a superseded turn can never overwrite the active card.
 *
 * This module is a pure schema/contract surface. It imports no Electron and no
 * Node host API, so it is unit-testable and safe to import from a preload or a
 * renderer bundle.
 */
export const DesktopTurnStartChannel = "openagents:turn:start" as const
export const DesktopTurnCancelChannel = "openagents:turn:cancel" as const
export const DesktopTurnStatusChannel = "openagents:turn:status" as const
/** Main -> renderer push channel for bounded progress and terminal frames. */
export const DesktopTurnEventChannel = "openagents:turn:event" as const

/** A renderer start request. It carries an intent and the owner-bound set only. */
export const DesktopTurnStartRequest = S.Struct({
  requestRef: TurnRequestRef,
  threadRef: TurnThreadRef,
  intent: TurnIntent,
  candidateSet: OwnerBoundCandidateSet,
})
export type DesktopTurnStartRequest = typeof DesktopTurnStartRequest.Type

/** The host acknowledgement for a start request. */
export const DesktopTurnStartAck = S.Union([
  S.Struct({ accepted: S.Literal(true), requestRef: TurnRequestRef }),
  S.Struct({ accepted: S.Literal(false), error: S.String.check(S.isMaxLength(240)) }),
])
export type DesktopTurnStartAck = typeof DesktopTurnStartAck.Type

/** A cancel request and its result. */
export const DesktopTurnCancelRequest = S.Struct({ requestRef: TurnRequestRef })
export type DesktopTurnCancelRequest = typeof DesktopTurnCancelRequest.Type

export const DesktopTurnCancelResult = S.Struct({ ok: S.Boolean })
export type DesktopTurnCancelResult = typeof DesktopTurnCancelResult.Type

/** A status request and its result (null when the turn is unknown). */
export const DesktopTurnStatusRequest = S.Struct({ requestRef: TurnRequestRef })
export type DesktopTurnStatusRequest = typeof DesktopTurnStatusRequest.Type

export const DesktopTurnStatusResult = S.NullOr(SafeTurnProjection)
export type DesktopTurnStatusResult = typeof DesktopTurnStatusResult.Type

/**
 * A pushed turn frame. Every frame carries the request and a monotonic
 * generation, so the renderer fence drops any frame at or below the generation
 * it has already applied for that request. A terminal frame also carries the
 * receipt reference facts.
 */
export const DesktopTurnEventFrame = S.Union([
  S.Struct({
    kind: S.Literal("progress"),
    requestRef: TurnRequestRef,
    generation: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    projection: SafeTurnProjection,
  }),
  S.Struct({
    kind: S.Literal("terminal"),
    requestRef: TurnRequestRef,
    generation: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
    projection: SafeTurnProjection,
    receipt: TurnReceipt,
  }),
])
export type DesktopTurnEventFrame = typeof DesktopTurnEventFrame.Type

export const decodeDesktopTurnStartRequest = S.decodeUnknownOption(DesktopTurnStartRequest)
export const decodeDesktopTurnCancelRequest = S.decodeUnknownOption(DesktopTurnCancelRequest)
export const decodeDesktopTurnStatusRequest = S.decodeUnknownOption(DesktopTurnStatusRequest)
export const decodeDesktopTurnEventFrame = S.decodeUnknownOption(DesktopTurnEventFrame)

/**
 * A renderer-side generation fence. It keeps the highest generation applied per
 * request and admits a frame only when its generation is greater. A terminal
 * frame at the same generation is admitted once so the terminal card can replace
 * the last running frame.
 */
export interface DesktopTurnFence {
  readonly admit: (frame: DesktopTurnEventFrame) => boolean
}

export const makeDesktopTurnFence = (): DesktopTurnFence => {
  const applied = new Map<string, number>()
  const terminalApplied = new Set<string>()
  return {
    admit: (frame) => {
      if (terminalApplied.has(frame.requestRef)) return false
      const seen = applied.get(frame.requestRef)
      if (seen !== undefined && frame.generation < seen) return false
      if (seen !== undefined && frame.generation === seen && frame.kind !== "terminal") return false
      applied.set(frame.requestRef, frame.generation)
      if (frame.kind === "terminal") terminalApplied.add(frame.requestRef)
      return true
    },
  }
}
