/**
 * Fable local runtime (#8712): account-home discovery, SDK option posture,
 * event mapping, bounds, redaction, continuity, and the no-silent-substitution
 * law at the runtime level (no ready account -> typed unavailable, the SDK is
 * never loaded, nothing falls through to any gateway).
 */
import { describe, expect, test } from "bun:test"
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs"
import { tmpdir, homedir } from "node:os"
import { join } from "node:path"

import { FABLE_LOCAL_DELTA_LIMIT, type FableLocalEvent } from "./fable-local-contract.ts"
import {
  FABLE_LOCAL_ALLOWED_TOOLS,
  discoverReadyFableClaudeHomes,
  makeFableLocalRuntime,
  makeFixtureFableLocalQuery,
  redactFableLocalText,
  type FableLocalQuery,
} from "./fable-local-runtime.ts"

const makeAccountRoot = (): string => {
  const root = mkdtempSync(join(tmpdir(), "fable-local-homes-"))
  // Default `~/.claude` analogue: credentialed but MUST never be a candidate.
  mkdirSync(join(root, ".claude"))
  writeFileSync(join(root, ".claude", "claude-oauth-token"), "sk-ant-oat01-default-home\n")
  // Sibling without a pooled token: discovered but not ready.
  mkdirSync(join(root, ".claude-pylon-a"))
  // Ready sibling.
  mkdirSync(join(root, ".claude-pylon-b"))
  writeFileSync(join(root, ".claude-pylon-b", "claude-oauth-token"), "sk-ant-oat01-pylon-b\n")
  // Codex sibling: wrong provider for this lane.
  mkdirSync(join(root, ".codex-pylon-1"))
  return root
}

describe("discoverReadyFableClaudeHomes", () => {
  test("finds ready sibling Claude homes and never the default ~/.claude", async () => {
    const root = makeAccountRoot()
    const ready = await discoverReadyFableClaudeHomes({ PYLON_ACCOUNT_HOME_ROOT: root })
    expect(ready).toEqual([{ ref: "claude-pylon-b", home: join(root, ".claude-pylon-b") }])
  })

  test("returns empty when no sibling home carries a pooled token", async () => {
    const root = mkdtempSync(join(tmpdir(), "fable-local-empty-"))
    mkdirSync(join(root, ".claude-pylon-a"))
    expect(await discoverReadyFableClaudeHomes({ PYLON_ACCOUNT_HOME_ROOT: root })).toEqual([])
  })
})

describe("redactFableLocalText", () => {
  test("replaces the workspace and home prefixes", () => {
    const text = `Read /work/scratch/turns/a.md and ${homedir()}/notes/b.md`
    expect(redactFableLocalText(text, { workspace: "/work/scratch/turns" }))
      .toBe("Read <workspace>/a.md and ~/notes/b.md")
  })
})

type CapturedQuery = { prompt: string; options: Record<string, unknown> }

const makeRuntimeHarness = (input: {
  script: (captured: CapturedQuery) => AsyncIterable<unknown>
  root?: string
}) => {
  const root = input.root ?? makeAccountRoot()
  const captured: CapturedQuery[] = []
  const query: FableLocalQuery = call => {
    captured.push(call)
    return input.script(call)
  }
  const scratch = mkdtempSync(join(tmpdir(), "fable-local-scratch-"))
  const runtime = makeFableLocalRuntime({
    scratchRoot: () => scratch,
    env: { PYLON_ACCOUNT_HOME_ROOT: root },
    queryImpl: async () => query,
  })
  return { runtime, captured, scratch, root }
}

const collect = () => {
  const events: FableLocalEvent[] = []
  return { events, emit: (event: FableLocalEvent) => events.push(event) }
}

describe("makeFableLocalRuntime.runTurn", () => {
  test("streams a real turn: conservative SDK options, mapped events, usage, final text", async () => {
    const longDelta = "x".repeat(FABLE_LOCAL_DELTA_LIMIT + 500)
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-1" }
        yield { type: "stream_event", event: { type: "content_block_delta", delta: { type: "text_delta", text: "Hello " } } }
        yield { type: "stream_event", event: { type: "content_block_delta", delta: { type: "text_delta", text: longDelta } } }
        yield {
          type: "assistant",
          message: { content: [{ type: "tool_use", id: "t1", name: "Read", input: { file_path: `${homedir()}/secret/notes.md` } }] },
        }
        yield {
          type: "user",
          message: { content: [{ type: "tool_result", tool_use_id: "t1", is_error: true, content: "permission denied by policy" }] },
        }
        yield { type: "result", subtype: "success", is_error: false, result: "Final answer.", usage: { input_tokens: 10, output_tokens: 5, cache_read_input_tokens: 2 } }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-1",
      threadRef: "thread-1",
      history: [{ role: "user", text: "earlier question" }, { role: "assistant", text: "earlier answer" }],
      message: "What now?",
      emit: sink.emit,
    })

    expect(result).toEqual({ ok: true, text: "Final answer.", totalTokens: 17 })
    expect(sink.events.map(event => event.kind)).toEqual([
      "turn_started",
      "text_delta",
      "text_delta",
      "tool_use",
      "tool_result",
      "turn_completed",
    ])
    const secondDelta = sink.events[2] as Extract<FableLocalEvent, { kind: "text_delta" }>
    expect(secondDelta.text.length).toBeLessThanOrEqual(FABLE_LOCAL_DELTA_LIMIT)
    const toolUse = sink.events[3] as Extract<FableLocalEvent, { kind: "tool_use" }>
    expect(toolUse.toolName).toBe("Read")
    expect(toolUse.summary).not.toContain(homedir())
    expect(toolUse.summary).toContain("~/secret/notes.md")
    const toolResult = sink.events[4] as Extract<FableLocalEvent, { kind: "tool_result" }>
    expect(toolResult.ok).toBe(false)
    expect(toolResult.toolName).toBe("Read")
    const completed = sink.events[5] as Extract<FableLocalEvent, { kind: "turn_completed" }>
    expect(completed.totalTokens).toBe(17)

    // Conservative headless posture: read-only tools, no Bash/Write/WebSearch,
    // partial streaming on, cwd inside the scratch root, isolated account env.
    const call = harness.captured[0]!
    expect(call.options.includePartialMessages).toBe(true)
    expect(call.options.permissionMode).toBe("default")
    expect(call.options.allowedTools).toEqual([...FABLE_LOCAL_ALLOWED_TOOLS])
    expect(call.options.allowedTools).not.toContain("Bash")
    expect(call.options.settingSources).toEqual([])
    expect(String(call.options.cwd)).toStartWith(harness.scratch)
    const env = call.options.env as Record<string, string | undefined>
    expect(env.CLAUDE_CONFIG_DIR).toBe(join(harness.root, ".claude-pylon-b"))
    expect(env.CLAUDE_CONFIG_DIR).not.toBe(join(homedir(), ".claude"))
    expect(env.CLAUDE_CODE_OAUTH_TOKEN).toBe("sk-ant-oat01-pylon-b")
    // First turn for the thread: bounded history is prepended to the prompt.
    expect(call.options.resume).toBeUndefined()
    expect(call.prompt).toContain("Conversation so far")
    expect(call.prompt).toContain("earlier question")
    expect(call.prompt).toContain("User: What now?")
  })

  test("second turn in the same thread resumes the SDK session instead of replaying history", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-keep" }
        yield { type: "result", subtype: "success", is_error: false, result: "ok", usage: null }
      },
    })
    const sink = collect()
    const first = await harness.runtime.runTurn({
      turnRef: "turn-1", threadRef: "thread-1", history: [{ role: "user", text: "one" }], message: "one", emit: sink.emit,
    })
    expect(first.ok).toBe(true)
    const second = await harness.runtime.runTurn({
      turnRef: "turn-2", threadRef: "thread-1", history: [{ role: "user", text: "one" }, { role: "assistant", text: "ok" }], message: "two", emit: sink.emit,
    })
    expect(second.ok).toBe(true)
    expect(harness.captured[1]!.options.resume).toBe("session-keep")
    expect(harness.captured[1]!.prompt).toBe("two")
  })

  test("maps provider error results to a typed turn_failed, never a throw", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-err" }
        yield { type: "result", subtype: "error_during_execution", is_error: true }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-err", threadRef: "thread-err", history: [], message: "hi", emit: sink.emit,
    })
    expect(result).toEqual({ ok: false, reason: "session_failed", detail: "error_during_execution" })
    expect(sink.events.at(-1)).toEqual({ kind: "turn_failed", reason: "session_failed", detail: "error_during_execution" })
  })

  test("maps a max-turns result to budget_exceeded", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-max" }
        yield { type: "result", subtype: "error_max_turns", is_error: true }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-max", threadRef: "thread-max", history: [], message: "hi", emit: sink.emit,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe("budget_exceeded")
  })

  test("interrupt aborts a running turn with a typed interrupted failure", async () => {
    const harness = makeRuntimeHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "session-abort" }
        const abort = captured.options.abortController as AbortController
        await new Promise<void>(resolve => abort.signal.addEventListener("abort", () => resolve(), { once: true }))
        throw new Error("aborted by controller")
      },
    })
    const sink = collect()
    const pending = harness.runtime.runTurn({
      turnRef: "turn-int", threadRef: "thread-int", history: [], message: "hi", emit: sink.emit,
    })
    await new Promise(resolve => setTimeout(resolve, 10))
    expect(harness.runtime.interrupt("turn-int")).toBe(true)
    const result = await pending
    expect(result).toEqual({ ok: false, reason: "interrupted", detail: "turn interrupted" })
    expect(harness.runtime.interrupt("turn-int")).toBe(false)
  })

  test("rotates to the next ready Claude home when an account fails before any content (live pylon-2 org-disabled shape)", async () => {
    const root = mkdtempSync(join(tmpdir(), "fable-local-rotate-"))
    for (const name of [".claude-pylon-a", ".claude-pylon-b"]) {
      mkdirSync(join(root, name))
      writeFileSync(join(root, name, "claude-oauth-token"), `token-${name}\n`)
    }
    const captured: CapturedQuery[] = []
    const query: FableLocalQuery = call => {
      captured.push(call)
      return captured.length === 1
        ? (async function* () {
            yield { type: "system", subtype: "init", session_id: "session-dead" }
            yield { type: "result", subtype: "error_during_execution", is_error: true }
          })()
        : (async function* () {
            yield { type: "system", subtype: "init", session_id: "session-live" }
            yield { type: "stream_event", event: { type: "content_block_delta", delta: { type: "text_delta", text: "rotated" } } }
            yield { type: "result", subtype: "success", is_error: false, result: "rotated", usage: null }
          })()
    }
    const runtime = makeFableLocalRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "fable-local-scratch-")),
      env: { PYLON_ACCOUNT_HOME_ROOT: root },
      queryImpl: async () => query,
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-rotate", threadRef: "thread-rotate", history: [], message: "hi", emit: sink.emit,
    })
    expect(result).toEqual({ ok: true, text: "rotated", totalTokens: null })
    // One visible turn, no turn_failed leak from the rotated-away attempt.
    expect(sink.events.map(event => event.kind)).toEqual(["turn_started", "text_delta", "turn_completed"])
    expect(captured.length).toBe(2)
    expect((captured[0]!.options.env as Record<string, string>).CLAUDE_CONFIG_DIR).toBe(join(root, ".claude-pylon-a"))
    expect((captured[1]!.options.env as Record<string, string>).CLAUDE_CONFIG_DIR).toBe(join(root, ".claude-pylon-b"))
  })

  test("does not rotate once content streamed: a partial reply fails honestly", async () => {
    const root = mkdtempSync(join(tmpdir(), "fable-local-norotate-"))
    for (const name of [".claude-pylon-a", ".claude-pylon-b"]) {
      mkdirSync(join(root, name))
      writeFileSync(join(root, name, "claude-oauth-token"), `token-${name}\n`)
    }
    let calls = 0
    const query: FableLocalQuery = () => {
      calls += 1
      return (async function* () {
        yield { type: "system", subtype: "init", session_id: "session-partial" }
        yield { type: "stream_event", event: { type: "content_block_delta", delta: { type: "text_delta", text: "partial " } } }
        yield { type: "result", subtype: "error_during_execution", is_error: true }
      })()
    }
    const runtime = makeFableLocalRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "fable-local-scratch-")),
      env: { PYLON_ACCOUNT_HOME_ROOT: root },
      queryImpl: async () => query,
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-partial", threadRef: "thread-partial", history: [], message: "hi", emit: sink.emit,
    })
    expect(result.ok).toBe(false)
    expect(calls).toBe(1)
    expect(sink.events.at(-1)?.kind).toBe("turn_failed")
  })

  test("NO SILENT SUBSTITUTION: no ready account means a typed unavailable result and the SDK is never loaded", async () => {
    const root = mkdtempSync(join(tmpdir(), "fable-local-none-"))
    let sdkLoaded = false
    const runtime = makeFableLocalRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "fable-local-scratch-")),
      env: { PYLON_ACCOUNT_HOME_ROOT: root },
      queryImpl: async () => {
        sdkLoaded = true
        throw new Error("must not be reached")
      },
    })
    expect(await runtime.availability()).toEqual({ state: "unavailable", reason: "no_claude_account" })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-none", threadRef: "thread-none", history: [], message: "hi", emit: sink.emit,
    })
    expect(result).toEqual({ ok: false, reason: "no_claude_account", detail: "no linked Claude account home found" })
    expect(sdkLoaded).toBe(false)
    expect(sink.events).toEqual([{ kind: "turn_failed", reason: "no_claude_account", detail: "no linked Claude account home found" }])
  })

  test("availability reports the first ready account deterministically", async () => {
    const harness = makeRuntimeHarness({ script: async function* () {} })
    expect(await harness.runtime.availability()).toEqual({ state: "available", accountRef: "claude-pylon-b" })
  })
})

describe("makeFixtureFableLocalQuery (smoke fixture)", () => {
  test("drives the real mapping to a streamed, tool-traced, completed turn", async () => {
    const scratch = mkdtempSync(join(tmpdir(), "fable-local-fixture-"))
    const runtime = makeFableLocalRuntime({
      scratchRoot: () => scratch,
      queryImpl: async () => makeFixtureFableLocalQuery(),
      discoverImpl: async () => [{ ref: "claude-pylon-fixture", home: "/nonexistent" }],
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-fixture", threadRef: "thread-fixture", history: [], message: "go", emit: sink.emit,
    })
    expect(result).toEqual({ ok: true, text: "Fable local streaming proof.", totalTokens: 49 })
    expect(sink.events.map(event => event.kind)).toEqual([
      "turn_started",
      "text_delta",
      "text_delta",
      "tool_use",
      "tool_result",
      "text_delta",
      "turn_completed",
    ])
  })
})
