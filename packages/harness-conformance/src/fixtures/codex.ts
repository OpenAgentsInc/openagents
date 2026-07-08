/**
 * Real codex harness conformance fixture (MH-1, issue #8582).
 *
 * Backed by the actual codex runtime surfaces:
 *   - chat: builds the desktop `KhalaCodeDesktopChatTurnEvent` shapes the codex
 *     app-server chat runtime emits (validated with the REAL desktop schema),
 *     then projects them onto `khala.chat_turn_event.v1`.
 *   - typed failures: derived from the REAL pylon session-error classifier
 *     (`classifySessionError`) and the REAL pylon-core quota classifier
 *     (`classifyQuotaSignal`) fed real provider error strings — never invented
 *     class names.
 *   - worker executor / metering / readiness: the own-capacity no-spend
 *     invariants and public capacity refs the Khala->Pylon->Codex delegation
 *     runbook fixes as law (CLAUDE.md), with exact SDK token accounting.
 */
import { Schema as S } from "effect"
import { KhalaCodeDesktopChatTurnEventSchema } from "@openagentsinc/khala-code-desktop/src/shared/rpc.ts"
import type { KhalaCodeDesktopChatTurnEvent } from "@openagentsinc/khala-code-desktop/src/shared/rpc.ts"
import { classifyQuotaSignal } from "@openagentsinc/pylon-core/custody/account-quota"
import { classifySessionError } from "@openagentsinc/pylon/src/session-error-class.ts"
import type { KhalaChatTurnEventV1 } from "@openagentsinc/agent-runtime-schema"
import { projectDesktopChatTurnEventToV1 } from "../chat-turn-projection.ts"
import type {
  HarnessConformanceFixture,
  HarnessFailureClass,
  HarnessFailureSample,
} from "../contract.ts"

// The desktop schema carries decoding services in its type; mirror the repo's
// own `main.ts` cast so `decodeUnknownSync` resolves to a plain decoder.
const decodeDesktopEvent = S.decodeUnknownSync(
  KhalaCodeDesktopChatTurnEventSchema as never,
) as (input: unknown) => KhalaCodeDesktopChatTurnEvent

/** Decode desktop events with the REAL desktop schema, then project to v1. */
const project = (
  events: ReadonlyArray<KhalaCodeDesktopChatTurnEvent>,
): ReadonlyArray<KhalaChatTurnEventV1> =>
  events.map((event) => projectDesktopChatTurnEventToV1(decodeDesktopEvent(event)))

const assistantMessage = (id: string, body: string) => ({
  id,
  role: "assistant" as const,
  body,
})

const startThreadTurn = (): ReadonlyArray<KhalaChatTurnEventV1> => {
  const threadId = "codex-thread-fixture"
  const turnId = "codex-turn-1"
  const messageId = "codex-msg-1"
  return project([
    { type: "thread_ready", threadId, turnId },
    { type: "message_start", turnId, message: assistantMessage(messageId, "") },
    { type: "message_delta", turnId, messageId, delta: "Repaired " },
    { type: "message_delta", turnId, messageId, delta: "the sum bug." },
    {
      type: "tool_event",
      turnId,
      event: {
        eventId: "codex-tool-1",
        invocationId: "codex-inv-1",
        kind: "tool_started",
        sessionId: threadId,
        payload: { files: 1 },
      },
    },
    { type: "message_done", turnId, messageId },
  ])
}

const interruptTurn = (): ReadonlyArray<KhalaChatTurnEventV1> => {
  const threadId = "codex-thread-fixture"
  const turnId = "codex-turn-2"
  const messageId = "codex-msg-2"
  return project([
    { type: "thread_ready", threadId, turnId },
    { type: "message_start", turnId, message: assistantMessage(messageId, "") },
    { type: "message_delta", turnId, messageId, delta: "Working" },
    {
      type: "message_replace",
      turnId,
      message: assistantMessage(messageId, "Codex interrupted this turn."),
    },
    { type: "message_done", turnId, messageId },
  ])
}

const resumeThread = (): ReadonlyArray<KhalaChatTurnEventV1> => {
  const threadId = "codex-thread-fixture"
  const turnId = "codex-turn-3"
  const messageId = "codex-msg-3"
  return project([
    { type: "thread_ready", threadId, turnId },
    { type: "message_start", turnId, message: assistantMessage(messageId, "") },
    { type: "message_delta", turnId, messageId, delta: "Resumed." },
    { type: "message_done", turnId, messageId },
  ])
}

const typedFailure = (
  failureClass: HarnessFailureClass,
  errorDigestRef: string,
  detail: string,
): HarnessFailureSample => ({ failureClass, errorDigestRef, detail })

export const codexHarnessConformanceFixture: HarnessConformanceFixture = {
  harnessKind: "codex",
  chatRuntime: { startThreadTurn, interruptTurn, resumeThread },
  workerExecutor: {
    claim: {
      claimRef: "assignment.codex.fixture-1",
      workUnitRef: "workunit.codex.sum_repair",
      runRef: "run.codex.fixture-1",
      repo: "OpenAgentsInc/openagents",
      commit: "0000000000000000000000000000000000000000",
      branch: "main",
      verifyCommand: "bun test src/sum.test.ts",
      cwd: "/pylon/cache/worktrees/assignment.codex.fixture-1",
    },
    closeout: {
      ok: true,
      claimRef: "assignment.codex.fixture-1",
      stopReason: "completed",
      verifyPassed: true,
      paymentMode: "no-spend",
      settlementState: "not_applicable",
      payoutClaimAllowed: false,
      resultRef: "result.public.pylon.codex_agent_task.fixture_repair_passed",
      usage: {
        metering: "exact",
        inputTokens: 1024,
        outputTokens: 256,
        reasoningTokens: 64,
        totalTokens: 1344,
        wallClockMs: 8200,
        model: "openagents/pylon-codex",
        plane: "subscription",
        marginalCostClass: "subscription",
      },
    },
  },
  readinessProbe: () => ({
    ready: true,
    harness: "codex",
    capacityAvailable: 1,
    capacityReady: 1,
    busy: 0,
    queued: 0,
    plane: "subscription",
    models: ["openagents/pylon-codex"],
  }),
  meteringSamples: [
    // exact: the codex turn reporter emits exact SDK usage per completed turn.
    {
      metering: "exact",
      inputTokens: 2048,
      outputTokens: 512,
      reasoningTokens: 128,
      totalTokens: 2688,
      wallClockMs: 11400,
      model: "openagents/pylon-codex",
      plane: "subscription",
      marginalCostClass: "subscription",
    },
    // not_measured: a turn with no usage payload records time only — never
    // synthesized tokens (Khala->Pylon->Codex delegation is exact-only).
    {
      metering: "not_measured",
      wallClockMs: 640,
      model: "openagents/pylon-codex",
      plane: "subscription",
      marginalCostClass: "subscription",
    },
  ],
  typedFailures: {
    account_exhausted: () => {
      const classified = classifySessionError(
        "Codex account exhausted: purchase more credits to continue.",
      )
      return typedFailure(
        classified.errorClass as HarnessFailureClass,
        classified.errorDigestRef,
        "codex session-error classifier",
      )
    },
    account_rate_limited: () => {
      const classified = classifySessionError(
        "HTTP 429: rate limit exceeded, too many requests.",
      )
      return typedFailure(
        classified.errorClass as HarnessFailureClass,
        classified.errorDigestRef,
        "codex session-error classifier",
      )
    },
    account_quota_exhausted: () => {
      // classifyQuotaSignal is the REAL codex/claude quota detector; an
      // exhausted signal is mapped to the named class exactly as the pylon
      // multi-session runner does.
      const signal = classifyQuotaSignal(
        "You have hit your usage limit. Try again at 5pm.",
        "codex",
      )
      return typedFailure(
        signal.exhausted ? "account_quota_exhausted" : "unknown",
        signal.sourceDigestRef,
        "codex quota classifier",
      )
    },
  },
}
