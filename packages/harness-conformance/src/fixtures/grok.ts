/**
 * Real grok_cli harness conformance fixture (MH-3/MH-4, issues #8589/#8590).
 *
 * Chat projections use the same desktop event schema + projector as codex/
 * claude (Grok desktop runtime emits the same neutral chat turn shapes).
 * Worker closeout / metering match grok-harness honesty: not_measured for
 * tokens when the free CLI plane does not expose usage, exact optional.
 * Typed failures use the shared pylon session/quota classifiers (same wire
 * classes the Grok lane maps into).
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

const decodeDesktopEvent = S.decodeUnknownSync(
  KhalaCodeDesktopChatTurnEventSchema as never,
) as (input: unknown) => KhalaCodeDesktopChatTurnEvent

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
  const threadId = "grok-thread-fixture"
  const turnId = "grok-turn-1"
  const messageId = "grok-msg-1"
  return project([
    { type: "thread_ready", threadId, turnId },
    { type: "message_start", turnId, message: assistantMessage(messageId, "") },
    { type: "message_delta", turnId, messageId, delta: "Fixed " },
    { type: "message_delta", turnId, messageId, delta: "the harness gap." },
    {
      type: "tool_event",
      turnId,
      event: {
        eventId: "grok-tool-1",
        invocationId: "grok-inv-1",
        kind: "tool_started",
        sessionId: threadId,
        payload: { files: 1 },
      },
    },
    { type: "message_done", turnId, messageId },
  ])
}

const interruptTurn = (): ReadonlyArray<KhalaChatTurnEventV1> => {
  const threadId = "grok-thread-fixture"
  const turnId = "grok-turn-2"
  const messageId = "grok-msg-2"
  return project([
    { type: "thread_ready", threadId, turnId },
    { type: "message_start", turnId, message: assistantMessage(messageId, "") },
    { type: "message_delta", turnId, messageId, delta: "Working" },
    {
      type: "message_replace",
      turnId,
      message: assistantMessage(messageId, "Grok interrupted this turn."),
    },
    { type: "message_done", turnId, messageId },
  ])
}

const resumeThread = (): ReadonlyArray<KhalaChatTurnEventV1> => {
  const threadId = "grok-thread-fixture"
  const turnId = "grok-turn-3"
  const messageId = "grok-msg-3"
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

export const grokHarnessConformanceFixture: HarnessConformanceFixture = {
  harnessKind: "grok_cli",
  chatRuntime: { startThreadTurn, interruptTurn, resumeThread },
  workerExecutor: {
    claim: {
      claimRef: "assignment.grok.fixture-1",
      workUnitRef: "workunit.grok.sum_repair",
      runRef: "run.grok.fixture-1",
      repo: "OpenAgentsInc/openagents",
      commit: "0000000000000000000000000000000000000000",
      branch: "main",
      verifyCommand: "bun test src/sum.test.ts",
      cwd: "/pylon/cache/worktrees/assignment.grok.fixture-1",
    },
    closeout: {
      ok: true,
      claimRef: "assignment.grok.fixture-1",
      stopReason: "completed",
      verifyPassed: true,
      paymentMode: "no-spend",
      settlementState: "not_applicable",
      payoutClaimAllowed: false,
      resultRef: "result.public.pylon.grok_agent_task.fixture_repair_passed",
      usage: {
        // Free CLI plane: tokens often not_measured (MH-4 honesty).
        metering: "not_measured",
        wallClockMs: 9100,
        model: "grok-4",
        plane: "cli_session",
        marginalCostClass: "free",
      },
    },
  },
  readinessProbe: () => ({
    ready: true,
    harness: "grok_cli",
    capacityAvailable: 4,
    capacityReady: 4,
    busy: 0,
    queued: 0,
    plane: "cli_session",
    models: ["grok-4", "grok-4.5"],
  }),
  meteringSamples: [
    {
      metering: "not_measured",
      wallClockMs: 4800,
      model: "grok-4",
      plane: "cli_session",
      marginalCostClass: "free",
    },
    {
      metering: "exact",
      inputTokens: 512,
      outputTokens: 128,
      totalTokens: 640,
      wallClockMs: 3200,
      model: "grok-4",
      plane: "cli_session",
      marginalCostClass: "free",
    },
  ],
  typedFailures: {
    account_exhausted: () => {
      // Must hit the exhausted branch before the generic "account" selection branch.
      const classified = classifySessionError(
        "usage limit: out of credits for this free window.",
      )
      return typedFailure(
        classified.errorClass as HarnessFailureClass,
        classified.errorDigestRef,
        "grok session-error classifier",
      )
    },
    account_rate_limited: () => {
      const classified = classifySessionError(
        "HTTP 429: rate limit exceeded, too many requests.",
      )
      return typedFailure(
        classified.errorClass as HarnessFailureClass,
        classified.errorDigestRef,
        "grok session-error classifier",
      )
    },
    account_quota_exhausted: () => {
      const signal = classifyQuotaSignal(
        "You have hit your usage limit. Try again later.",
        "grok",
      )
      return typedFailure(
        signal.exhausted ? "account_quota_exhausted" : "unknown",
        signal.sourceDigestRef,
        "grok quota classifier",
      )
    },
  },
}
