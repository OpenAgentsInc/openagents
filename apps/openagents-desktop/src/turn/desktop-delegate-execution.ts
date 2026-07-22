import type { ClaudeLocalEvent } from "../claude-local-contract.ts"
import type {
  ProviderLane,
  ProviderLaneHistoryMessage,
} from "../provider-lane.ts"
import type { CodexLaneTurnResult } from "./desktop-codex-provider.ts"
import { makeOrdinaryDelegationExecution } from "./desktop-delegation.ts"

type ExecutableDelegateLane<Context> = Pick<ProviderLane<Context>, "admit" | "runTurn">

/** Run one ordinary background delegation without Full Auto prompt authority. */
export const executeOrdinaryDelegateTurn = async <Context>(input: Readonly<{
  lane: ExecutableDelegateLane<Context> | null
  requestRef: string
  threadRef: string
  message: string
  history: ReadonlyArray<ProviderLaneHistoryMessage>
  emit: (event: ClaudeLocalEvent) => void
}>): Promise<CodexLaneTurnResult> => {
  if (input.lane === null) {
    return { ok: false, reason: "session_failed", detail: "delegate lane not ready" }
  }
  const execution = makeOrdinaryDelegationExecution(input)
  if (execution === null) {
    return { ok: false, reason: "session_failed", detail: "invalid delegation request" }
  }
  const admission = input.lane.admit(execution.request)
  if (!admission.ok) {
    return { ok: false, reason: "session_failed", detail: admission.error }
  }
  const result = await input.lane.runTurn({
    request: execution.request,
    model: admission.model,
    context: admission.context,
    history: input.history,
    message: input.message,
    background: execution.mode.background,
    emit: input.emit,
  })
  return result.ok
    ? { ok: true, text: result.text }
    : { ok: false, reason: result.reason, detail: result.detail }
}
