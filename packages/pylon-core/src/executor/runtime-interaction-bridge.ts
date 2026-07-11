import {
  decodeRuntimeInteraction,
  type KhalaRuntimeSource,
  type RuntimeInteraction,
  type RuntimeInteractionDecision,
  type RuntimeInteractionPayload,
} from "@openagentsinc/agent-runtime-schema"
import type { CanUseTool } from "@anthropic-ai/claude-agent-sdk"
import { Effect } from "effect"

export type PylonRuntimeInteractionBridgeErrorReason =
  | "invalid_request"
  | "authority_unavailable"
  | "invalid_terminal"
  | "identity_mismatch"
  | "kind_mismatch"
  | "not_terminal"

export class PylonRuntimeInteractionBridgeError extends Error {
  readonly _tag = "PylonRuntimeInteractionBridgeError"
  override readonly name = "PylonRuntimeInteractionBridgeError"

  constructor(readonly reason: PylonRuntimeInteractionBridgeErrorReason) {
    super(`Pylon runtime interaction bridge failed: ${reason}.`)
  }
}

export type PylonRuntimeInteractionAuthority = Readonly<{
  request: (interaction: RuntimeInteraction) => Effect.Effect<void, unknown>
  awaitTerminal: (
    interactionRef: string,
  ) => Effect.Effect<unknown, unknown>
}>

export type PylonRuntimeInteractionOutcome =
  | Readonly<{
      state: "resolved"
      interactionRef: string
      decisionRef: string
      decision: RuntimeInteractionDecision
    }>
  | Readonly<{
      state: "expired" | "revoked"
      interactionRef: string
      reasonRef: string
      terminalAt: string
    }>

export type PylonRuntimeInteractionRequest = Readonly<{
  interactionRef: string
  threadRef: string
  turnRef: string
  requestedSequence: number
  requestedAt: string
  expiresAt: string
  source: KhalaRuntimeSource
  causalityRefs: ReadonlyArray<string>
  payload: RuntimeInteractionPayload
}>

const bridgeError = (reason: PylonRuntimeInteractionBridgeErrorReason) =>
  new PylonRuntimeInteractionBridgeError(reason)

const decodeInteraction = (
  value: unknown,
  reason: "invalid_request" | "invalid_terminal",
): Effect.Effect<RuntimeInteraction, PylonRuntimeInteractionBridgeError> =>
  Effect.try({
    try: () => decodeRuntimeInteraction(value),
    catch: () => bridgeError(reason),
  })

/**
 * Provider-neutral Pylon boundary for a supervised runtime interaction.
 * The durable authority is injected: this module neither polls a UI nor
 * changes any executor's default permission policy.
 */
export const requestPylonRuntimeInteraction = (
  authority: PylonRuntimeInteractionAuthority,
  input: PylonRuntimeInteractionRequest,
): Effect.Effect<
  PylonRuntimeInteractionOutcome,
  PylonRuntimeInteractionBridgeError
> => Effect.gen(function*() {
  const pending = yield* decodeInteraction({
    schema: "openagents.runtime_interaction.v1",
    interactionRef: input.interactionRef,
    threadId: input.threadRef,
    turnId: input.turnRef,
    requestedSequence: input.requestedSequence,
    requestedAt: input.requestedAt,
    expiresAt: input.expiresAt,
    source: input.source,
    visibility: "private",
    redactionClass: "private_ref",
    causalityRefs: [...input.causalityRefs],
    payload: input.payload,
    lifecycle: { status: "pending" },
  }, "invalid_request")

  yield* authority.request(pending).pipe(
    Effect.mapError(() => bridgeError("authority_unavailable")),
  )
  const terminal = yield* authority.awaitTerminal(pending.interactionRef).pipe(
    Effect.mapError(() => bridgeError("authority_unavailable")),
    Effect.flatMap(value => decodeInteraction(value, "invalid_terminal")),
  )
  if (
    terminal.interactionRef !== pending.interactionRef ||
    terminal.threadId !== pending.threadId ||
    terminal.turnId !== pending.turnId
  ) return yield* Effect.fail(bridgeError("identity_mismatch"))
  if (terminal.payload.kind !== pending.payload.kind) {
    return yield* Effect.fail(bridgeError("kind_mismatch"))
  }
  if (terminal.lifecycle.status === "pending") {
    return yield* Effect.fail(bridgeError("not_terminal"))
  }
  if (terminal.lifecycle.status === "resolved") {
    if (terminal.lifecycle.envelope.decision.kind !== pending.payload.kind) {
      return yield* Effect.fail(bridgeError("kind_mismatch"))
    }
    return {
      state: "resolved",
      interactionRef: terminal.interactionRef,
      decisionRef: terminal.lifecycle.envelope.decisionRef,
      decision: terminal.lifecycle.envelope.decision,
    }
  }
  return {
    state: terminal.lifecycle.status,
    interactionRef: terminal.interactionRef,
    reasonRef: terminal.lifecycle.reasonRef,
    terminalAt: terminal.lifecycle.terminalAt,
  }
})

/**
 * Claude SDK adapter. Raw tool input is returned only to the same SDK call on
 * confirmed approval; the durable request builder receives refs and labels,
 * never the input payload.
 */
export const createClaudeCanUseToolInteractionController = (input: Readonly<{
  authority: PylonRuntimeInteractionAuthority
  requestFor: (tool: Readonly<{
    toolName: string
    toolUseId: string
  }>) => PylonRuntimeInteractionRequest
}>): CanUseTool => async (toolName, toolInput, options) => {
  try {
    const outcome = await Effect.runPromise(
      requestPylonRuntimeInteraction(
        input.authority,
        input.requestFor({ toolName, toolUseId: options.toolUseID }),
      ),
      { signal: options.signal },
    )
    if (
      outcome.state === "resolved" &&
      outcome.decision.kind === "tool_approval" &&
      outcome.decision.outcome === "approve"
    ) {
      return { behavior: "allow", updatedInput: toolInput }
    }
    return {
      behavior: "deny",
      message: outcome.state === "resolved"
        ? "Denied by confirmed OpenAgents authority."
        : "OpenAgents approval is no longer actionable.",
    }
  } catch {
    return {
      behavior: "deny",
      message: "OpenAgents approval authority is unavailable.",
    }
  }
}
