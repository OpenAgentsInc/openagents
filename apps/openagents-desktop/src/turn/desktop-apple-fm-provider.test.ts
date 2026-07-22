import { Effect, Schema as S, Stream } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  CONTEXT_ENVELOPE_SCHEMA_LITERAL,
  TurnIntent,
  TurnProviderRef,
  TurnRequestRef,
  TurnThreadRef,
  WorkContextEnvelope,
} from "@openagentsinc/agent-runtime-schema"
import type { ProviderStartInput } from "@openagentsinc/agent-turn-runtime"

import {
  APPLE_FM_STATUS_SCHEMA_ID,
  APPLE_FM_TURN_SCHEMA_ID,
  type AppleFmTurnResult,
} from "../apple-fm-contract.ts"
import type { AppleFmHost } from "../apple-fm-host.ts"
import type { AppleFmAvailableAgent } from "./apple-fm-prompt.ts"
import { makeDesktopAppleFmProviderRegistry } from "./desktop-apple-fm-provider.ts"

const decodeContext = S.decodeUnknownSync(WorkContextEnvelope)
const decodeIntent = S.decodeUnknownSync(TurnIntent)
const decodeProviderRef = S.decodeUnknownSync(TurnProviderRef)
const decodeRequestRef = S.decodeUnknownSync(TurnRequestRef)
const decodeThreadRef = S.decodeUnknownSync(TurnThreadRef)

const askInput = (text: string): ProviderStartInput => ({
  providerRef: decodeProviderRef("provider.apple_fm.local"),
  requestRef: decodeRequestRef("request.apple_fm.desktop.1"),
  threadRef: decodeThreadRef("thread.apple_fm.desktop.1"),
  intent: decodeIntent({ _tag: "Ask", text }),
  context: decodeContext({
    schema: CONTEXT_ENVELOPE_SCHEMA_LITERAL,
    manifestRef: "context.apple_fm.desktop.1",
    threadRef: "thread.apple_fm.desktop.1",
    generation: { state: "known", value: 0 },
    createdAt: "2026-07-22T02:00:00.000Z",
    items: [],
    totalByteLength: 0,
    byteLimit: 0,
    truncated: false,
    redacted: false,
  }),
})

const completed = (
  text: string,
  counts: Readonly<{ prompt?: number; completion?: number; total?: number }> = {},
): AppleFmTurnResult => ({
  schema: APPLE_FM_TURN_SCHEMA_ID,
  ok: true,
  outcome: "completed",
  text,
  usageTruth: "estimated",
  promptTokens: counts.prompt ?? null,
  completionTokens: counts.completion ?? null,
  totalTokens: counts.total ?? null,
  failureClass: null,
})

const agents: ReadonlyArray<AppleFmAvailableAgent> = [
  { candidate: "codex", label: "Codex", ready: true, canDelegate: true },
  { candidate: "claude", label: "Claude", ready: true, canDelegate: true },
  { candidate: "grok_acp", label: "Grok", ready: true, canDelegate: true },
]

const makeHost = (results: ReadonlyArray<AppleFmTurnResult>) => {
  const calls: Array<Readonly<{ prompt: string; routeCandidates?: ReadonlyArray<string> }>> = []
  let cursor = 0
  const host: AppleFmHost = {
    status: () => ({
      schema: APPLE_FM_STATUS_SCHEMA_ID,
      supported: true,
      state: "ready",
      readiness: "ready",
      ready: true,
      mode: "local_adopted",
      model: "apple-fm",
      profileId: "default",
      usageTruth: "estimated",
      unavailableReason: null,
      blockerRefs: [],
    }),
    ensureStarted: async () => host.status(),
    refresh: async () => host.status(),
    runTurn: async (prompt, routeCandidates) => {
      calls.push({ prompt, ...(routeCandidates === undefined ? {} : { routeCandidates }) })
      const result = results[cursor]
      cursor += 1
      if (result === undefined) throw new Error("unexpected Apple FM turn")
      return result
    },
    stop: () => host.status(),
    dispose: () => undefined,
  }
  return { calls, host }
}

const run = async (host: AppleFmHost, text: string) => {
  const registry = makeDesktopAppleFmProviderRegistry(
    () => host,
    () => null,
    () => agents,
    () => ({ appName: "OpenAgents Dev", isOwnerDevice: true }),
  )
  return Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const providerRun = yield* registry.start(askInput(text))
        return [...(yield* Stream.runCollect(providerRun.events))]
      }),
    ),
  )
}

const completedCandidate = (
  events: ReadonlyArray<unknown>,
): Readonly<{ text: string; provenance: Readonly<Record<string, unknown>> }> => {
  const terminal = events.at(-1) as {
    readonly _tag: string
    readonly candidate?: { readonly text: string; readonly provenance: Readonly<Record<string, unknown>> }
  }
  expect(terminal._tag).toBe("Completed")
  if (terminal.candidate === undefined) throw new Error("answer candidate missing")
  return terminal.candidate
}

describe("Desktop Apple FM answer-first routing", () => {
  test("exact identity chat stays direct when Codex, Claude, and Grok are ready", async () => {
    const { calls, host } = makeHost([completed("I am OpenAgents, your local assistant.")])
    const candidate = completedCandidate(await run(host, "hey who are you"))

    expect(candidate.text).toBe("I am OpenAgents, your local assistant.")
    expect(calls).toHaveLength(1)
    expect(calls[0]?.routeCandidates).toBeUndefined()
    expect(calls[0]?.prompt).toContain("You are OpenAgents")
    expect(calls[0]?.prompt).not.toContain("local router")
  })

  test("an explicit coding action uses guided routing with a local route available", async () => {
    const route = JSON.stringify({
      candidate: "codex",
      taskClass: "delegate",
      reasonCode: "needs_delegation",
      confidence: 1,
    })
    const { calls, host } = makeHost([completed(route)])
    const candidate = completedCandidate(await run(host, "please implement issue #9159"))

    expect(candidate.text).toBe(route)
    expect(calls).toHaveLength(1)
    expect(calls[0]?.routeCandidates).toEqual(["apple_fm", "codex", "claude", "grok_acp"])
    expect(calls[0]?.prompt).toContain("apple_fm (OpenAgents)")
  })

  test("a guided local route runs a separate direct answer and hides route JSON", async () => {
    const localRoute = JSON.stringify({
      candidate: "apple_fm",
      taskClass: "delegate",
      reasonCode: "needs_delegation",
      confidence: 1,
    })
    const { calls, host } = makeHost([
      completed(localRoute, { prompt: 4, completion: 2, total: 6 }),
      completed("I need one detail before I can help.", { prompt: 5, completion: 3, total: 8 }),
    ])
    const candidate = completedCandidate(await run(host, "review this"))

    expect(candidate.text).toBe("I need one detail before I can help.")
    expect(calls).toHaveLength(2)
    expect(calls[0]?.routeCandidates).toContain("apple_fm")
    expect(calls[1]?.routeCandidates).toBeUndefined()
    expect(calls[1]?.prompt).toContain("helpful, friendly assistant")
    expect(candidate.provenance).toMatchObject({ usageTruth: "estimated" })
  })
})
