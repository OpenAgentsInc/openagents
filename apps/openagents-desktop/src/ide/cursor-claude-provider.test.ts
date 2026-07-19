import { Deferred, Effect, Fiber, Schema, Stream } from "effect"
import { TestClock } from "effect/testing"
import { describe, expect, test } from "vite-plus/test"

import { ideAgentFixtureProposal } from "./agent-code-fixture.ts"
import {
  IdeCursorIdentityProgressSchema,
  IdeCursorIntentSchema,
  IdeCursorStreamEventSchema,
  type IdeCursorProviderInput,
} from "./cursor-contract.ts"
import {
  ideCursorFixtureIdentity,
  ideCursorFixtureInput,
  ideCursorFixtureRequest,
} from "./cursor-fixture.ts"
import {
  decodeIdeCursorClaudeOutput,
  makeIdeCursorClaudeProvider,
  type IdeCursorClaudeOutput,
  type IdeCursorClaudeQuery,
  type IdeCursorClaudeQueryOptions,
} from "./cursor-claude-provider.ts"
import { IdeCursorProviderFailure } from "./cursor-provider.ts"

const providerRef = "provider.anthropic.claude-agent-sdk"
const modelRef = "claude-sonnet-4-6"
const harnessRef = "harness.claude-agent-sdk"
const accountRef = "account.claude.fixture"
type IdeCursorIntent = typeof IdeCursorIntentSchema.Type

const identity = () => {
  const fixture = ideCursorFixtureIdentity({ provider: providerRef, model: modelRef })
  const execution = {
    ...fixture.effective,
    harness: { ...fixture.effective.harness, value: harnessRef },
    account: { ...fixture.effective.account, value: accountRef },
    indexPosture: "disabled" as const,
    networkPosture: "networked" as const,
  }
  return IdeCursorIdentityProgressSchema.make({
    requested: execution,
    admitted: execution,
    effective: execution,
    substitution: { _tag: "None" },
  })
}

const inputFor = (
  intent: IdeCursorIntent = { _tag: "Complete", acceptance: "all" },
  suffix = "claude",
  maxLatencyMs = 2_000,
): IdeCursorProviderInput => ideCursorFixtureInput(ideCursorFixtureRequest(suffix, 1, {
  identity: identity(),
  intent,
  budget: { maxLatencyMs, maxInputTokens: 4_096, maxOutputTokens: 1_024 },
}))

const usage = {
  input_tokens: 10,
  output_tokens: 4,
  cache_creation_input_tokens: 2,
  cache_read_input_tokens: 3,
}

const modelUsage = {
  [modelRef]: {
    inputTokens: 10,
    outputTokens: 4,
    cacheReadInputTokens: 3,
    cacheCreationInputTokens: 2,
    webSearchRequests: 0,
    costUSD: 0.000321,
    contextWindow: 200_000,
    maxOutputTokens: 8_192,
  },
}

const terminal = (structuredOutput: unknown) => ({
  type: "result",
  subtype: "success",
  is_error: false,
  num_turns: 1,
  total_cost_usd: 0.000321,
  usage,
  modelUsage,
  permission_denials: [],
  structured_output: structuredOutput,
})

const queryFor = (
  output: unknown,
  inspect?: (prompt: string, options: IdeCursorClaudeQueryOptions) => void,
): IdeCursorClaudeQuery => ({ prompt, options }) => {
  inspect?.(prompt, options)
  return (async function* () {
    yield { type: "system", subtype: "init" }
    yield terminal(output)
  })()
}

const providerFor = (query: IdeCursorClaudeQuery) => makeIdeCursorClaudeProvider({
  query,
  isolatedCwd: "/isolated/cursor-empty",
  providerRef,
  modelRefs: [modelRef],
  harnessRef,
  accountRef,
  now: () => "2026-07-19T13:00:00.000Z",
})

const collect = (query: IdeCursorClaudeQuery, input = inputFor()) =>
  Effect.runPromise(Stream.runCollect(providerFor(query).generate(input)).pipe(
    Effect.map(values => Array.from(values, value => Schema.decodeUnknownSync(IdeCursorStreamEventSchema)(value))),
  ))

const completion = (extra: Readonly<Record<string, unknown>> = {}): IdeCursorClaudeOutput => ({
  _tag: "Completion",
  replace: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
  text: "export ",
  confidence: 0.91,
  ...extra,
})

describe("IdeCursor Claude provider", () => {
  test("uses the exact empty capability and emits identity before candidate with measured disclosure", async () => {
    const captured: IdeCursorClaudeQueryOptions[] = []
    const events = await collect(queryFor(completion(), (_prompt, options) => {
      captured.push(options)
    }))

    expect(events.map(event => event._tag)).toEqual(["Identity", "Candidate", "Finished"])
    expect(events[0]).toMatchObject({
      _tag: "Identity",
      identity: { effective: { provider: { value: providerRef }, model: { value: modelRef } } },
    })
    expect(events[1]).toMatchObject({
      _tag: "Candidate",
      candidate: {
        _tag: "Completion",
        identity: { effective: { account: { value: accountRef }, harness: { value: harnessRef } } },
        disclosure: {
          usage: {
            input: { _tag: "Measured", value: 15, unit: "tokens" },
            output: { _tag: "Measured", value: 4, unit: "tokens" },
            cost: { _tag: "Measured", value: 321, unit: "usd_micros" },
          },
          noRemoteIndexDependency: true,
          secretsSent: false,
        },
      },
    })
    expect(events[2]).toMatchObject({ _tag: "Finished", disclosure: { dataDestinations: [{ retention: "provider_policy" }] } })

    expect(captured[0]).toMatchObject({
      cwd: "/isolated/cursor-empty",
      model: modelRef,
      tools: [],
      allowedTools: [],
      skills: [],
      agents: {},
      mcpServers: {},
      strictMcpConfig: true,
      plugins: [],
      settingSources: [],
      maxTurns: 1,
      persistSession: false,
      enableFileCheckpointing: false,
      additionalDirectories: [],
      permissionMode: "dontAsk",
      outputFormat: { type: "json_schema" },
    })
    expect(Object.hasOwn(captured[0] ?? {}, "canUseTool")).toBe(false)
    expect(Object.hasOwn(captured[0] ?? {}, "hooks")).toBe(false)
    expect(captured[0]?.abortController).toBeInstanceOf(AbortController)
    expect(Object.keys(captured[0] ?? {}).sort()).toEqual([
      "abortController",
      "additionalDirectories",
      "agents",
      "allowedTools",
      "cwd",
      "enableFileCheckpointing",
      "maxTurns",
      "mcpServers",
      "model",
      "outputFormat",
      "permissionMode",
      "persistSession",
      "plugins",
      "settingSources",
      "skills",
      "strictMcpConfig",
      "tools",
    ])
  })

  test("supports every admitted intent with exactly its candidate kind", async () => {
    const proposal = ideAgentFixtureProposal()
    const rows: ReadonlyArray<readonly [IdeCursorIntent, IdeCursorClaudeOutput, string]> = [
      [{ _tag: "Complete", acceptance: "all" }, completion(), "Completion"],
      [{ _tag: "NextEdit" }, {
        _tag: "NextEdit",
        targetPathRef: "src/app.ts",
        replace: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
        text: "export ",
        explanation: "Continue the active declaration.",
        confidence: 0.8,
      }, "NextEdit"],
      [{ _tag: "Ask", question: "What does this file do?" }, {
        _tag: "Answer",
        markdown: "It exports an answer constant.",
        confidence: 0.88,
      }, "Answer"],
      [{ _tag: "Edit", instruction: "Change the answer." }, {
        _tag: "Proposal",
        proposalRef: proposal.proposalRef,
        proposal,
        confidence: 0.75,
      }, "Proposal"],
      [{ _tag: "Generate", instruction: "Generate an answer." }, {
        _tag: "Proposal",
        proposalRef: proposal.proposalRef,
        proposal,
        confidence: 0.75,
      }, "Proposal"],
    ]
    for (const [intent, output, expected] of rows) {
      const events = await collect(queryFor(output), inputFor(intent, `intent-${expected}-${intent._tag}`))
      expect(events[1]).toMatchObject({ _tag: "Candidate", candidate: { _tag: expected } })
    }
  })

  test("rejects identity that does not match configured provider selection before dispatch", async () => {
    let called = false
    const provider = providerFor(queryFor(completion(), () => { called = true }))
    const base = inputFor()
    const wrong = {
      ...base,
      request: {
        ...base.request,
        identity: {
          ...base.request.identity,
          effective: {
            ...base.request.identity.effective,
            account: { ...base.request.identity.effective.account, value: "account.claude.other" },
          },
        },
      },
    }
    const failure = await Effect.runPromise(
      Stream.runCollect(provider.generate(wrong)).pipe(Effect.flip),
    )
    expect(failure).toBeInstanceOf(IdeCursorProviderFailure)
    expect(failure.reason).toBe("rejected")
    expect(called).toBe(false)
  })

  test("strictly rejects extra fields, malformed output, tool calls, and intent mismatch", async () => {
    const extra = await Effect.runPromise(decodeIdeCursorClaudeOutput({ ...completion(), unexpected: true }).pipe(Effect.flip))
    expect(extra.reason).toBe("invalid_event")

    const toolQuery: IdeCursorClaudeQuery = () => (async function* () {
      yield { type: "assistant", message: { content: [{ type: "tool_use", name: "Read", input: {} }] } }
      yield terminal(completion())
    })()
    for (const query of [
      queryFor({ ...completion(), unexpected: true }),
      queryFor({ _tag: "Completion", replace: "not-a-range", text: "x", confidence: 1 }),
      queryFor({ _tag: "Answer", markdown: "wrong intent", confidence: 1 }),
      toolQuery,
    ]) {
      const failure = await Effect.runPromise(Stream.runCollect(providerFor(query).generate(inputFor())).pipe(Effect.flip))
      expect(failure).toBeInstanceOf(IdeCursorProviderFailure)
      expect(failure.reason).toBe("invalid_event")
    }
  })

  test("returns explicit unavailable failure when ambient query/account discovery is not injected", async () => {
    const failure = await Effect.runPromise(
      Stream.runCollect(makeIdeCursorClaudeProvider().generate(inputFor())).pipe(Effect.flip),
    )
    expect(failure.reason).toBe("unavailable")
  })

  test("aborts and closes the SDK query when the consumer cancels", async () => {
    const started = await Effect.runPromise(Deferred.make<void>())
    const cleaned = await Effect.runPromise(Deferred.make<void>())
    let aborted = false
    let closed = false
    const query: IdeCursorClaudeQuery = ({ options }) => ({
      [Symbol.asyncIterator]: () => ({
        next: () => new Promise<IteratorResult<unknown>>(resolve => {
          void Effect.runPromise(Deferred.succeed(started, undefined))
          const finish = () => {
            aborted = true
            resolve({ done: true, value: undefined })
          }
          if (options.abortController?.signal.aborted === true) finish()
          else options.abortController?.signal.addEventListener("abort", finish, { once: true })
        }),
      }),
      close: () => {
        closed = true
        void Effect.runPromise(Deferred.succeed(cleaned, undefined))
      },
    })
    await Effect.runPromise(Effect.gen(function* () {
      const fiber = yield* Stream.runCollect(providerFor(query).generate(inputFor())).pipe(Effect.forkChild)
      yield* Deferred.await(started)
      yield* Fiber.interrupt(fiber).pipe(Effect.forkDetach)
      yield* Deferred.await(cleaned)
    }))
    expect(aborted).toBe(true)
    expect(closed).toBe(true)
  })

  test("aborts the SDK query and fails when the admitted latency budget expires", async () => {
    const started = yieldDeferred()
    let aborted = false
    let closed = false
    const query: IdeCursorClaudeQuery = ({ options }) => ({
      [Symbol.asyncIterator]: () => ({
        next: () => new Promise<IteratorResult<unknown>>(resolve => {
          void Effect.runPromise(Deferred.succeed(started, undefined))
          const finish = () => {
            aborted = true
            resolve({ done: true, value: undefined })
          }
          options.abortController?.signal.addEventListener("abort", finish, { once: true })
        }),
      }),
      close: () => { closed = true },
    })
    const failure = await Effect.runPromise(Effect.gen(function* () {
      const fiber = yield* Stream.runCollect(providerFor(query).generate(inputFor(
        { _tag: "Complete", acceptance: "all" },
        "timeout",
        25,
      ))).pipe(
        Effect.flip,
        Effect.forkChild,
      )
      yield* Deferred.await(started)
      yield* TestClock.adjust(25)
      return yield* Fiber.join(fiber)
    }).pipe(Effect.provide(TestClock.layer())))
    expect(failure.reason).toBe("unavailable")
    expect(failure.detail).toContain("exceeded 25 ms")
    expect(aborted).toBe(true)
    expect(closed).toBe(true)
  })
})

const yieldDeferred = () => Effect.runSync(Deferred.make<void>())
