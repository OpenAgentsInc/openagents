/**
 * Real claude_code harness conformance fixture (MH-1, issue #8582).
 *
 * Backed by the actual claude runtime surfaces:
 *   - chat: the claude app-sdk chat runtime shares the desktop
 *     `KhalaCodeDesktopChatTurnEvent` union with codex; this fixture builds
 *     those shapes (validated with the REAL desktop schema) and projects them
 *     onto `khala.chat_turn_event.v1`.
 *   - typed failures: derived from the REAL pylon session-error classifier and
 *     the REAL pylon-core quota classifier (provider `claude_agent`) — never
 *     invented class names.
 *   - worker executor / metering / readiness: the own-capacity no-spend
 *     invariants, exact SDK usage, and public capacity refs.
 *
 * Note: this fixture only READS the shared classifiers and the desktop event
 * schema. It never imports or edits the claude runtime files themselves (those
 * are the MH-2 lane's exclusive path).
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
  const threadId = "claude-thread-fixture"
  const turnId = "claude-turn-1"
  const messageId = "claude-msg-1"
  return project([
    { type: "thread_ready", threadId, turnId },
    { type: "message_start", turnId, message: assistantMessage(messageId, "") },
    { type: "message_delta", turnId, messageId, delta: "Fixed " },
    { type: "message_delta", turnId, messageId, delta: "the failing test." },
    {
      type: "tool_event",
      turnId,
      event: {
        eventId: "claude-tool-1",
        invocationId: "claude-inv-1",
        kind: "tool_started",
        sessionId: threadId,
        payload: { file: "src/sum.ts" },
      },
    },
    { type: "message_done", turnId, messageId },
  ])
}

const interruptTurn = (): ReadonlyArray<KhalaChatTurnEventV1> => {
  const threadId = "claude-thread-fixture"
  const turnId = "claude-turn-2"
  const messageId = "claude-msg-2"
  return project([
    { type: "thread_ready", threadId, turnId },
    { type: "message_start", turnId, message: assistantMessage(messageId, "") },
    { type: "message_delta", turnId, messageId, delta: "Thinking" },
    {
      type: "message_replace",
      turnId,
      message: assistantMessage(messageId, "Claude interrupted this turn."),
    },
    { type: "message_done", turnId, messageId },
  ])
}

const resumeThread = (): ReadonlyArray<KhalaChatTurnEventV1> => {
  const threadId = "claude-thread-fixture"
  const turnId = "claude-turn-3"
  const messageId = "claude-msg-3"
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

export const claudeHarnessConformanceFixture: HarnessConformanceFixture = {
  harnessKind: "claude_code",
  chatRuntime: { startThreadTurn, interruptTurn, resumeThread },
  workerExecutor: {
    claim: {
      claimRef: "assignment.claude.fixture-1",
      workUnitRef: "workunit.claude.sum_repair",
      runRef: "run.claude.fixture-1",
      repo: "OpenAgentsInc/openagents",
      commit: "0000000000000000000000000000000000000000",
      branch: "main",
      verifyCommand: "bun test src/sum.test.ts",
      cwd: "/pylon/cache/worktrees/assignment.claude.fixture-1",
    },
    closeout: {
      ok: true,
      claimRef: "assignment.claude.fixture-1",
      stopReason: "completed",
      verifyPassed: true,
      paymentMode: "no-spend",
      settlementState: "not_applicable",
      payoutClaimAllowed: false,
      resultRef: "result.public.pylon.claude_agent_task.fixture_repair_passed",
      usage: {
        metering: "exact",
        inputTokens: 1536,
        outputTokens: 384,
        totalTokens: 1920,
        wallClockMs: 9100,
        model: "openagents/pylon-claude",
        plane: "subscription",
        marginalCostClass: "subscription",
      },
    },
  },
  readinessProbe: () => ({
    ready: true,
    harness: "claude_code",
    capacityAvailable: 1,
    capacityReady: 1,
    busy: 0,
    queued: 0,
    plane: "subscription",
    models: ["openagents/pylon-claude"],
  }),
  meteringSamples: [
    {
      metering: "exact",
      inputTokens: 3000,
      outputTokens: 700,
      totalTokens: 3700,
      wallClockMs: 12800,
      model: "openagents/pylon-claude",
      plane: "subscription",
      marginalCostClass: "subscription",
    },
    {
      metering: "not_measured",
      wallClockMs: 500,
      model: "openagents/pylon-claude",
      plane: "subscription",
      marginalCostClass: "subscription",
    },
  ],
  typedFailures: {
    account_exhausted: () => {
      const classified = classifySessionError(
        "Claude account exhausted: you are out of credits.",
      )
      return typedFailure(
        classified.errorClass as HarnessFailureClass,
        classified.errorDigestRef,
        "claude session-error classifier",
      )
    },
    account_rate_limited: () => {
      const classified = classifySessionError(
        "429 Too Many Requests: rate limit reached.",
      )
      return typedFailure(
        classified.errorClass as HarnessFailureClass,
        classified.errorDigestRef,
        "claude session-error classifier",
      )
    },
    account_quota_exhausted: () => {
      const signal = classifyQuotaSignal(
        "You have hit your usage limit; purchase more credits.",
        "claude_agent",
      )
      return typedFailure(
        signal.exhausted ? "account_quota_exhausted" : "unknown",
        signal.sourceDigestRef,
        "claude quota classifier",
      )
    },
  },
}
