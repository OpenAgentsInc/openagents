import {
  type ChatMessageEntity,
  type ChatThreadEntity,
  type KhalaRuntimeEvent,
  type RuntimeEventEntity,
  type RuntimeTurnEntity,
} from "@openagentsinc/khala-sync"
import type { KhalaMobileCreditsTransaction } from "../../src/sync/khala-mobile-credits-api"
import type { KhalaModelPreference } from "../../src/sync/khala-mobile-model-preference-api"

export const mobileFixtureAt = "2026-07-07T18:00:00.000Z"
export const mobileFixtureOwnerUserId = "user_mobile_fixture"
export const mobileFixtureThreadId = "thread.mobile.fixture.1"

const mobileToolAuthorityFixture = {
  allowed: true,
  authorityRef: "authority.private.fixture.mobile.allow_read",
  blockerRefs: [],
  decisionRef: "decision.private.fixture.mobile.allow_read",
  policyRef: "policy.private.fixture.mobile.tool_read",
  status: "allowed",
  toolRef: "tool.openagents.fixture.read",
} as const

export const mobileThreadFixtures: readonly ChatThreadEntity[] = [
  {
    createdAt: mobileFixtureAt,
    lastMessageAt: "2026-07-07T18:05:00.000Z",
    messageCount: 3,
    ownerUserId: mobileFixtureOwnerUserId,
    repoBinding: { defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" },
    status: "active",
    threadId: mobileFixtureThreadId,
    title: "Ship the mobile gate",
    updatedAt: "2026-07-07T18:05:00.000Z",
  },
  {
    createdAt: "2026-07-07T17:00:00.000Z",
    lastMessageAt: null,
    messageCount: 0,
    ownerUserId: mobileFixtureOwnerUserId,
    repoBinding: null,
    status: "active",
    threadId: "thread.mobile.fixture.2",
    title: "Empty scratch thread",
    updatedAt: "2026-07-07T17:00:00.000Z",
  },
]

export const mobileMessageFixtures: readonly ChatMessageEntity[] = [
  {
    body: "Please close the mobile launch gate.",
    createdAt: "2026-07-07T18:01:00.000Z",
    deletedAt: null,
    messageId: "message.mobile.fixture.1",
    authorUserId: mobileFixtureOwnerUserId,
    threadId: mobileFixtureThreadId,
    updatedAt: "2026-07-07T18:01:00.000Z",
  },
]

const runtimeBase = {
  causalityRefs: [],
  observedAt: mobileFixtureAt,
  redactionClass: "private_ref" as const,
  schema: "openagents.khala_runtime_event.v1" as const,
  source: {
    adapterKind: "codex" as const,
    lane: "codex_app_server" as const,
    surface: "server" as const,
  },
  threadId: mobileFixtureThreadId,
  turnId: "turn.mobile.fixture.1",
  visibility: "private" as const,
}

export const mobileRuntimeOrderedEvents: readonly KhalaRuntimeEvent[] = [
  {
    ...runtimeBase,
    eventId: "event.mobile.fixture.started",
    kind: "turn.started",
    sequence: 1,
  },
  {
    ...runtimeBase,
    chunkId: "chunk.mobile.fixture.reasoning.1",
    eventId: "event.mobile.fixture.reasoning",
    kind: "reasoning.delta",
    messageId: "message.mobile.fixture.reasoning.1",
    sequence: 2,
    text: "Inspecting the mobile test harness.",
  },
  {
    ...runtimeBase,
    chunkId: "chunk.mobile.fixture.text.1",
    eventId: "event.mobile.fixture.text.1",
    kind: "text.delta",
    messageId: "message.mobile.fixture.agent.1",
    sequence: 3,
    text: "I added the mount fixtures.",
  },
  {
    ...runtimeBase,
    authority: mobileToolAuthorityFixture,
    eventId: "event.mobile.fixture.tool.call",
    inputRef: "input.private.fixture.mobile.rg",
    kind: "tool.call",
    sequence: 4,
    toolCallId: "tool_call.mobile.fixture.rg",
    toolName: "rg",
  },
  {
    ...runtimeBase,
    authority: mobileToolAuthorityFixture,
    eventId: "event.mobile.fixture.tool.result",
    kind: "tool.result",
    resultRef: "result.private.fixture.mobile.rg",
    sequence: 5,
    toolCallId: "tool_call.mobile.fixture.rg",
    toolName: "rg",
  },
  {
    ...runtimeBase,
    eventId: "event.mobile.fixture.usage",
    kind: "usage.recorded",
    sequence: 6,
    usage: { inputTokens: 42, outputTokens: 24, totalTokens: 66, usageRef: "usage.private.fixture.mobile.1" },
  },
  {
    ...runtimeBase,
    branch: "codex/qam-2-fixture",
    branchUrl: "https://github.com/OpenAgentsInc/openagents/tree/codex/qam-2-fixture",
    changedFileCount: 4,
    eventId: "event.mobile.fixture.writeback",
    kind: "writeback.recorded",
    pullRequestNumber: 8537,
    pullRequestUrl: "https://github.com/OpenAgentsInc/openagents/pull/8537",
    repositoryFullName: "OpenAgentsInc/openagents",
    sequence: 7,
    status: "pull_request_opened",
    writebackRef: "writeback.private.fixture.qam_2",
  },
  {
    ...runtimeBase,
    eventId: "event.mobile.fixture.finished",
    finishReason: "stop",
    kind: "turn.finished",
    sequence: 8,
  },
]

export const mobileRuntimeInterruptedEvents: readonly KhalaRuntimeEvent[] = [
  {
    ...runtimeBase,
    eventId: "event.mobile.fixture.interrupted.started",
    kind: "turn.started",
    sequence: 1,
    turnId: "turn.mobile.fixture.interrupted",
  },
  {
    ...runtimeBase,
    chunkId: "chunk.mobile.fixture.interrupted.1",
    eventId: "event.mobile.fixture.interrupted.text",
    kind: "text.delta",
    messageId: "message.mobile.fixture.interrupted.1",
    sequence: 2,
    text: "Stopping before the file write.",
    turnId: "turn.mobile.fixture.interrupted",
  },
  {
    ...runtimeBase,
    eventId: "event.mobile.fixture.interrupted",
    kind: "turn.interrupted",
    reasonRef: "reason.private.fixture.owner_stop",
    sequence: 3,
    turnId: "turn.mobile.fixture.interrupted",
  },
]

export const mobileRuntimeRefusalEvents: readonly KhalaRuntimeEvent[] = [
  {
    ...runtimeBase,
    eventId: "event.mobile.fixture.refusal.started",
    kind: "turn.started",
    sequence: 1,
    turnId: "turn.mobile.fixture.refusal",
  },
  {
    ...runtimeBase,
    authority: mobileToolAuthorityFixture,
    errorRef: "error.private.fixture.insufficient_credit",
    eventId: "event.mobile.fixture.refusal.insufficient_credit",
    kind: "tool.error",
    messageSafe: "insufficient_credit",
    sequence: 2,
    toolCallId: "tool_call.mobile.fixture.insufficient_credit",
    toolName: "runtime.startTurn",
    turnId: "turn.mobile.fixture.refusal",
  },
  {
    ...runtimeBase,
    authority: mobileToolAuthorityFixture,
    errorRef: "error.private.fixture.rate_limited",
    eventId: "event.mobile.fixture.refusal.rate_limited",
    kind: "tool.error",
    messageSafe: "rate_limited",
    sequence: 3,
    toolCallId: "tool_call.mobile.fixture.rate_limited",
    toolName: "runtime.startTurn",
    turnId: "turn.mobile.fixture.refusal",
  },
  {
    ...runtimeBase,
    authority: mobileToolAuthorityFixture,
    errorRef: "error.private.fixture.org_capacity_unavailable",
    eventId: "event.mobile.fixture.refusal.org_capacity_unavailable",
    kind: "tool.error",
    messageSafe: "org_capacity_unavailable",
    sequence: 4,
    toolCallId: "tool_call.mobile.fixture.org_capacity_unavailable",
    toolName: "runtime.startTurn",
    turnId: "turn.mobile.fixture.refusal",
  },
  {
    ...runtimeBase,
    eventId: "event.mobile.fixture.refusal.finished",
    finishReason: "error",
    kind: "turn.finished",
    sequence: 5,
    turnId: "turn.mobile.fixture.refusal",
  },
]

export const runtimeEntitiesFromEvents = (
  events: ReadonlyArray<KhalaRuntimeEvent>,
): readonly RuntimeEventEntity[] =>
  events.map(event => ({
    createdAt: event.observedAt,
    event,
    eventId: event.eventId,
    kind: event.kind,
    observedAt: event.observedAt,
    ownerUserId: mobileFixtureOwnerUserId,
    sequence: event.sequence,
    threadId: event.threadId,
    turnId: event.turnId,
  }))

export const mobileTurnFixtures: readonly RuntimeTurnEntity[] = [
  {
    createdAt: mobileFixtureAt,
    eventCount: mobileRuntimeOrderedEvents.length,
    latestIntentId: null,
    lane: "codex_app_server",
    ownerUserId: mobileFixtureOwnerUserId,
    settledAt: "2026-07-07T18:06:00.000Z",
    startedAt: mobileFixtureAt,
    status: "completed",
    threadId: mobileFixtureThreadId,
    turnId: "turn.mobile.fixture.1",
    updatedAt: "2026-07-07T18:06:00.000Z",
  },
]

export const mobileCreditsTransactions: readonly KhalaMobileCreditsTransaction[] = [
  {
    amountUsdCents: 1000,
    description: "Launch credit",
    id: "credit.mobile.fixture.grant",
    kind: "grant",
    occurredAt: "2026-07-07T16:00:00.000Z",
  },
  {
    amountUsdCents: 125,
    description: "Codex mobile fixture run",
    id: "credit.mobile.fixture.charge",
    kind: "charge",
    occurredAt: "2026-07-07T17:00:00.000Z",
  },
]

export const mobileModelPreference: KhalaModelPreference = {
  availableModelIds: ["gpt-5", "gpt-5-mini"],
  availableTargetIds: ["gemini", "auto", "codex:owner-account"],
  effectiveModelId: "gpt-5",
  effectiveTargetId: "codex:owner-account",
  fallback: "none",
  preferredModelId: "gpt-5",
  preferredTargetId: "codex:owner-account",
  updatedAt: mobileFixtureAt,
  usedPreference: true,
}
