import { describe, expect, test } from "vite-plus/test"
import { Effect, Layer, Schema as S } from "effect"

import { TurnIntent } from "@openagentsinc/agent-runtime-schema"
import {
  ProviderRegistry,
  TurnService,
  TurnServiceLayer,
  TurnServiceTesting,
  type TurnStartInput,
} from "@openagentsinc/agent-turn-runtime"

import type { ClaudeLocalEvent } from "../claude-local-contract.ts"
import {
  makeCodexProviderRegistry,
  redactCodexEvent,
  type CodexLaneReadiness,
  type CodexLaneTurnResult,
} from "./desktop-codex-provider.ts"

const decodeIntent = S.decodeUnknownSync(TurnIntent)

const startInput = (): TurnStartInput => ({
  requestRef: TurnServiceTesting.fixtureCandidateSet.ordered[0] as unknown as TurnStartInput["requestRef"],
  threadRef: "thread.1" as unknown as TurnStartInput["threadRef"],
  intent: decodeIntent({ _tag: "Ask", text: "delegate this to codex" }),
  candidateSet: TurnServiceTesting.fixtureCandidateSet,
})

// The fixture candidate set/policy admit "provider.codex.1"; match it so the
// kernel resolves the codex descriptor for the effective lane.
const CODEX_REF = "provider.codex.1"

const composeLayer = (config: Parameters<typeof makeCodexProviderRegistry>[0]) =>
  TurnServiceLayer.pipe(
    Layer.provide(TurnServiceTesting.contextSourceFixtureLayer()),
    Layer.provide(TurnServiceTesting.turnPolicyFixtureLayer()),
    Layer.provide(Layer.succeed(ProviderRegistry, ProviderRegistry.of(makeCodexProviderRegistry(config)))),
    Layer.provide(TurnServiceTesting.turnJournalMemoryLayer),
    Layer.provide(TurnServiceTesting.threadRepositoryMemoryLayer),
    Layer.provide(TurnServiceTesting.artifactResolverFixtureLayer),
    Layer.provide(TurnServiceTesting.actionBrokerRecordingLayer),
  )

const runOne = (config: Parameters<typeof makeCodexProviderRegistry>[0]) =>
  Effect.runPromise(
    Effect.gen(function* () {
      const service = yield* TurnService
      return yield* service.start({ ...startInput(), requestRef: "request.codex.1" as unknown as TurnStartInput["requestRef"] })
    }).pipe(Effect.provide(composeLayer(config))),
  )

const readyReadiness: CodexLaneReadiness = { ready: true, accountRef: "acct.codex.a" }

const ev = (event: unknown): ClaudeLocalEvent => event as ClaudeLocalEvent

describe("redactCodexEvent — the redaction boundary", () => {
  test("reasoning becomes a SYSTEM activity summary (never an assistant message, #9127)", () => {
    expect(redactCodexEvent(ev({ kind: "reasoning", text: "planning the push" }))).toEqual({
      role: "system",
      text: "planning the push",
    })
  })

  test("a tool_result yields a label + output BYTE COUNT, never the raw output", () => {
    const activity = redactCodexEvent(
      ev({
        kind: "tool_result",
        toolName: "shell",
        ok: true,
        summary: "fatal: leaked /Users/owner/.ssh/id_rsa token sk-live-DEADBEEF",
        item: { kind: "command", command: "git push --force", cwd: "/Users/owner", outputTail: "boom" },
      }),
    )
    expect(activity?.role).toBe("tool")
    expect(activity?.toolLabel).toBe("shell")
    expect(typeof activity?.commandOutputByteCount).toBe("number")
    // No raw text field beyond the label.
    expect(JSON.stringify(activity).includes("sk-live-DEADBEEF")).toBe(false)
    expect(JSON.stringify(activity).includes("/Users/owner")).toBe(false)
    expect(JSON.stringify(activity).includes("git push")).toBe(false)
  })

  test("a fileChange tool item yields only the file COUNT, never a path or diff", () => {
    const activity = redactCodexEvent(
      ev({
        kind: "tool_use",
        toolName: "apply_patch",
        summary: "{}",
        item: {
          kind: "fileChange",
          changes: [
            { path: "/Users/owner/a.ts", diff: "SECRET DIFF" },
            { path: "/b.ts", diff: "SECRET DIFF 2" },
          ],
        },
      }),
    )
    expect(activity).toEqual({ role: "tool", toolLabel: "apply_patch", fileChangeCount: 2 })
  })

  test("non-visible events project nothing", () => {
    expect(redactCodexEvent(ev({ kind: "turn_completed", totalTokens: 42 }))).toBeNull()
    expect(redactCodexEvent(ev({ kind: "text_delta", text: "streamed answer" }))).toBeNull()
  })
})

describe("codex kernel provider — exit checks", () => {
  test("a ready lane runs ONE codex turn and reaches a done terminal with a redacted chain", async () => {
    let runCount = 0
    const result = await runOne({
      providerRef: CODEX_REF,
      readiness: () => readyReadiness,
      runTurn: async ({ emit }) => {
        runCount += 1
        emit(ev({ kind: "reasoning", text: "planning" }))
        emit(
          ev({
            kind: "tool_use",
            toolName: "shell",
            summary: JSON.stringify({ command: "git status" }),
            item: { kind: "command", command: "git status", cwd: "/Users/owner/work" },
          }),
        )
        emit(
          ev({
            kind: "tool_result",
            toolName: "shell",
            ok: true,
            summary: "on branch main; secret sk-live-DEADBEEF at /Users/owner/.env",
            item: { kind: "fileChange", changes: [{ path: "/Users/owner/x" }] },
          }),
        )
        return { ok: true, text: "Delegated work complete. Pushed to main." }
      },
    })

    // Exactly one provider start.
    expect(runCount).toBe(1)
    // Terminal maps to done.
    expect(result.projection.cardState).toBe("done")
    expect(result.candidate?.kind).toBe("answer")
    // The chain carries the redacted entries: reasoning, tool, tool, final answer.
    const chain = result.projection.messageChain
    expect(chain.length).toBe(4)
    expect(chain[0]!.role).toBe("system")
    expect(chain[1]!.role).toBe("tool")
    expect(chain[1]!.toolLabel).toBe("shell")
    expect(chain[3]!.role).toBe("assistant")
    expect(chain[3]!.text).toBe("Delegated work complete. Pushed to main.")

    // The WHOLE projection carries no raw command, output, path, or token.
    const serialized = JSON.stringify(result.projection)
    for (const secret of ["sk-live-DEADBEEF", "/Users/owner", "git status", "git push", ".env"]) {
      expect(serialized.includes(secret)).toBe(false)
    }
  })

  test("an unauthenticated lane produces NO start and refuses honestly", async () => {
    let runCount = 0
    const result = await runOne({
      providerRef: CODEX_REF,
      readiness: () => ({ ready: false, unavailableReason: "no_verified_account" }),
      runTurn: async () => {
        runCount += 1
        return { ok: true, text: "should never run" }
      },
    })
    expect(runCount).toBe(0)
    expect(result.projection.cardState).toBe("refused")
    // Never dispatched: no provider start receipt.
    expect(result.projection.providerTurnRef).toBeUndefined()
    expect(result.refusal).toBe("provider_unauthorized")
  })

  test("the card cannot show running before the start receipt (unavailable lane never runs)", async () => {
    const result = await runOne({
      providerRef: CODEX_REF,
      readiness: () => ({ ready: false, unavailableReason: "quota_exhausted" }),
      runTurn: async () => ({ ok: true, text: "x" }),
    })
    // A refused turn never passes through a running card state.
    expect(result.projection.cardState).toBe("refused")
    expect(result.refusal).toBe("provider_unavailable")
  })

  test("a codex lane failure maps to a failed terminal", async () => {
    const result = await runOne({
      providerRef: CODEX_REF,
      readiness: () => readyReadiness,
      runTurn: async () => ({ ok: false, reason: "session_failed", detail: "codex crashed" }),
    })
    expect(result.projection.cardState).toBe("failed")
    expect(result.candidate).toBeNull()
  })
})
