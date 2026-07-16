/**
 * Fable local runtime (#8712 + EP250 owner full-access override): account-
 * home discovery, SDK option posture, event mapping, bounds, redaction,
 * continuity, and the no-silent-substitution law at the runtime level (no
 * ready account -> typed unavailable, the SDK is never loaded, nothing falls
 * through to any gateway).
 *
 * Enforces openagents_desktop.chat.fable_local_owner_full_access.v1 (owner
 * statement verbatim 2026-07-11: "disallowing bash is retarded, give them
 * full tools full permissions etc"): the full SDK toolset is offered (no
 * allowedTools restriction, no PreToolUse workspace guard, no out-of-scope
 * denial copy), canUseTool allows every tool EXCEPT AskUserQuestion which
 * still parks on the real question flow — and the mode stays "default"
 * (never bypassPermissions, which would skip canUseTool and kill the
 * question flow; receipted from sdk.d.ts).
 */
import { describe, expect, test } from "vite-plus/test"
import { existsSync, mkdtempSync, mkdirSync, writeFileSync } from "node:fs"
import { tmpdir, homedir } from "node:os"
import { join } from "node:path"

import { FABLE_LOCAL_DELTA_LIMIT, type FableLocalEvent } from "./fable-local-contract.ts"
import {
  FABLE_DELEGATE_MAX_CHILDREN_PER_TURN,
  FABLE_DELEGATE_MAX_CONCURRENT,
  FABLE_DELEGATE_TOOL_NAME,
  FABLE_FIXTURE_DELEGATE_TASK,
  FABLE_LOCAL_DISALLOWED_TOOLS,
  FABLE_LOCAL_MODEL,
  FABLE_STREAM_CLOSE_TIMEOUT_MS,
  discoverReadyFableClaudeHomes,
  fableThreadWorkspaceSlug,
  makeFableLocalRuntime,
  makeFixtureFableLocalQuery,
  makeFixtureFableMcpFactory,
  redactFableLocalText,
  type FableDelegateRuntime,
  type FableLocalQuery,
  type FixtureFableMcpTool,
} from "./fable-local-runtime.ts"
import {
  FIXTURE_CODEX_CHILD_TEXT,
  fixtureCodexRevokedStderr,
  fixtureCodexRevokedStdout,
  fixtureCodexSuccessStdout,
  makeCodexAccountHealth,
  makeCodexChildRuntime,
  makeFixtureCodexChildSpawn,
} from "./codex-child-runtime.ts"

const makeAccountRoot = (): string => {
  const root = mkdtempSync(join(tmpdir(), "fable-local-homes-"))
  // Default `~/.claude` analogue; individual tests opt it into auth.
  mkdirSync(join(root, ".claude"))
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
  test("prefers the current Claude session before ready Pylon fallbacks", async () => {
    const root = makeAccountRoot()
    writeFileSync(join(root, ".claude", ".credentials.json"), "{}\n")
    const ready = await discoverReadyFableClaudeHomes({ PYLON_ACCOUNT_HOME_ROOT: root })
    expect(ready).toEqual([
      { ref: "claude", home: join(root, ".claude"), source: "current_session" },
      { ref: "claude-pylon-b", home: join(root, ".claude-pylon-b"), source: "pylon" },
    ])
  })

  test("returns empty when neither a current session nor a Pylon token exists", async () => {
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

type CapturedQuery = { prompt: string | AsyncIterable<unknown>; options: Record<string, unknown> }

const makeRuntimeHarness = (input: {
  script: (captured: CapturedQuery) => AsyncIterable<unknown>
  root?: string
  workspaceRoot?: string
  questionTimeoutMs?: number
  initialSessions?: ReadonlyArray<Readonly<{ threadRef: string; sessionId: string; accountRef: string }>>
  onDispatch?: (input: Readonly<{ threadRef: string; turnRef: string; accountRef: string }>) => void
  onProviderSession?: (input: Readonly<{
    threadRef: string
    turnRef: string
    accountRef: string
    sessionId: string
  }>) => void
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
    ...(input.workspaceRoot === undefined ? {} : { workspaceRoot: () => input.workspaceRoot! }),
    env: { PYLON_ACCOUNT_HOME_ROOT: root },
    queryImpl: async () => query,
    ...(input.questionTimeoutMs === undefined ? {} : { questionTimeoutMs: input.questionTimeoutMs }),
    ...(input.initialSessions === undefined ? {} : { initialSessions: input.initialSessions }),
    ...(input.onDispatch === undefined ? {} : { onDispatch: input.onDispatch }),
    ...(input.onProviderSession === undefined ? {} : { onProviderSession: input.onProviderSession }),
  })
  return { runtime, captured, scratch, root }
}

const collect = () => {
  const events: FableLocalEvent[] = []
  return { events, emit: (event: FableLocalEvent) => events.push(event) }
}

describe("makeFableLocalRuntime.runTurn", () => {
  test("an explicit workspace root is the exact Claude cwd", async () => {
    const workspace = mkdtempSync(join(tmpdir(), "fable-local-workspace-"))
    const harness = makeRuntimeHarness({
      workspaceRoot: workspace,
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-workspace" }
        yield { type: "result", subtype: "success", is_error: false, result: "done", usage: {} }
      },
    })
    await harness.runtime.runTurn({
      turnRef: "turn-workspace", threadRef: "thread-workspace", history: [], message: "work here", emit: () => {},
    })
    expect(harness.captured[0]?.options.cwd).toBe(workspace)
  })

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

    expect(result).toEqual({
      ok: true,
      text: "Final answer.",
      totalTokens: 17,
      accountRef: "claude-pylon-b",
      usage: { inputTokens: 10, cachedInputTokens: 2, outputTokens: 5, reasoningTokens: 0, totalTokens: 17 },
    })
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
    // Typed item payloads (#8859): the Claude lane maps into the same
    // harness-neutral model, source-tagged "claude", args redacted k/v.
    expect(toolUse.item).toMatchObject({
      kind: "toolCall",
      source: "claude",
      callKind: "dynamic",
      tool: "Read",
      status: "in_progress",
    })
    const toolUseItem = toolUse.item as Extract<NonNullable<typeof toolUse.item>, { kind: "toolCall" }>
    expect(toolUseItem.args).toEqual([{ key: "file_path", value: "~/secret/notes.md" }])
    expect(toolResult.item).toMatchObject({
      kind: "toolCall",
      source: "claude",
      tool: "Read",
      status: "failed",
      errorMessage: "permission denied by policy",
    })
    const completed = sink.events[5] as Extract<FableLocalEvent, { kind: "turn_completed" }>
    expect(completed.totalTokens).toBe(17)

    // Owner full-access posture (EP250 override, verbatim: "disallowing
    // bash is retarded, give them full tools full permissions etc"): NO
    // allowedTools restriction (full SDK toolset — Bash, Write, Edit,
    // WebSearch, NotebookEdit, Agent, …), NO PreToolUse workspace guard.
    // The thread workspace is the DEFAULT cwd only, not a boundary.
    const call = harness.captured[0]!
    expect(call.options.includePartialMessages).toBe(true)
    // MECHANISM (receipted from sdk.d.ts): NOT bypassPermissions — that
    // "Bypass[es] all permission checks" including canUseTool, which the
    // AskUserQuestion flow parks on. Default mode + allow-all canUseTool.
    expect(call.options.permissionMode).toBe("default")
    expect(call.options.allowedTools).toBeUndefined()
    expect(call.options.tools).toBeUndefined()
    expect(call.options.hooks).toBeUndefined()
    expect(typeof call.options.canUseTool).toBe("function")
    // IT HAS TO BE FABLE: the requested model is pinned, never the account
    // home's default. Skills are removed from the lane entirely (the Skill
    // tool is disallowed and no skills are enabled), so a bundled skill can
    // never auto-trigger and fail against the whitelist.
    expect(call.options.model).toBe(FABLE_LOCAL_MODEL)
    expect(call.options.model).toBe("claude-fable-5")
    // Interactive-only UX-noise tools stay disallowed (separately decided,
    // not reversed by the full-access override); NotebookEdit is now
    // OFFERED; AskUserQuestion IS offered (it has a real answer path).
    expect(call.options.disallowedTools).toEqual([...FABLE_LOCAL_DISALLOWED_TOOLS])
    expect(call.options.disallowedTools).toContain("Skill")
    expect(call.options.disallowedTools).toContain("EnterPlanMode")
    expect(call.options.disallowedTools).toContain("ExitPlanMode")
    expect(call.options.disallowedTools).not.toContain("NotebookEdit")
    expect(call.options.disallowedTools).not.toContain("Bash")
    expect(call.options.disallowedTools).not.toContain("AskUserQuestion")
    expect(call.options.skills).toEqual([])
    expect(call.options.settingSources).toEqual([])
    expect(String(call.options.cwd).startsWith(harness.scratch)).toBe(true)
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

  test("offers only main-validated local plugins to the Claude SDK on the next turn", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-plugin" }
        yield { type: "result", subtype: "success", is_error: false, result: "ok", usage: null }
      },
    })
    const runtime = makeFableLocalRuntime({
      scratchRoot: () => harness.scratch,
      env: { PYLON_ACCOUNT_HOME_ROOT: harness.root },
      queryImpl: async () => (call => { harness.captured.push(call); return (async function* () {
        yield { type: "system", subtype: "init", session_id: "session-plugin" }
        yield { type: "result", subtype: "success", is_error: false, result: "ok", usage: null }
      })() }),
      userPlugins: () => ["/private/plugin-a"],
    })
    await runtime.runTurn({ turnRef: "turn-plugin", threadRef: "thread-plugin", history: [], message: "go", skillName: "review", emit: collect().emit })
    expect(harness.captured[0]!.options.plugins).toEqual([{ type: "local", path: "/private/plugin-a" }])
    expect(harness.captured[0]!.options.skills).toEqual(["review"])
    expect(harness.captured[0]!.options.disallowedTools).not.toContain("Skill")
  })

  test("capability I1: a turn with images lowers the prompt to an SDK base64 image content block", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-img" }
        yield { type: "result", subtype: "success", is_error: false, result: "ok", usage: null }
      },
    })
    const sink = collect()
    await harness.runtime.runTurn({
      turnRef: "turn-img",
      threadRef: "thread-img",
      history: [],
      message: "what's wrong in this screenshot?",
      images: [
        { mediaType: "image/png", data: "aGVsbG8=", name: "shot.png" },
        { mediaType: "image/webp", data: "d2VicA==" },
      ],
      emit: sink.emit,
    })
    const call = harness.captured[0]!
    // With images the prompt is an AsyncIterable (streaming input), NOT a
    // string — a bare string prompt cannot carry an image (sdk.d.ts).
    expect(typeof call.prompt).not.toBe("string")
    const messages: Array<{ type: string; message: { role: string; content: Array<{ type: string; text?: string; source?: { type: string; media_type: string; data: string } }> } }> = []
    for await (const message of call.prompt as AsyncIterable<typeof messages[number]>) messages.push(message)
    expect(messages).toHaveLength(1)
    const content = messages[0]!.message.content
    // One text block (the user's message) + one image block per attachment.
    expect(content.filter(block => block.type === "text").map(block => block.text)).toEqual([
      "what's wrong in this screenshot?",
    ])
    const imageBlocks = content.filter(block => block.type === "image")
    expect(imageBlocks).toHaveLength(2)
    expect(imageBlocks[0]!.source).toEqual({ type: "base64", media_type: "image/png", data: "aGVsbG8=" })
    expect(imageBlocks[1]!.source).toEqual({ type: "base64", media_type: "image/webp", data: "d2VicA==" })
  })

  test("capability I1: a turn without images keeps the plain string prompt (additive, unchanged)", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-noimg" }
        yield { type: "result", subtype: "success", is_error: false, result: "ok", usage: null }
      },
    })
    await harness.runtime.runTurn({
      turnRef: "turn-noimg",
      threadRef: "thread-noimg",
      history: [],
      message: "plain text turn",
      emit: collect().emit,
    })
    expect(typeof harness.captured[0]!.prompt).toBe("string")
    expect(harness.captured[0]!.prompt).toBe("plain text turn")
  })

  test("second turn in the same thread resumes the SDK session instead of replaying history", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-keep" }
        yield { type: "result", subtype: "success", is_error: false, result: "ok", usage: null }
      },
    })
    const sink = collect()
    expect(harness.runtime.hasContinuity("thread-1")).toBe(false)
    const first = await harness.runtime.runTurn({
      turnRef: "turn-1", threadRef: "thread-1", history: [{ role: "user", text: "one" }], message: "one", emit: sink.emit,
    })
    expect(first.ok).toBe(true)
    expect(harness.runtime.hasContinuity("thread-1")).toBe(true)
    expect(harness.runtime.hasContinuity("another-thread")).toBe(false)
    const second = await harness.runtime.runTurn({
      turnRef: "turn-2", threadRef: "thread-1", history: [{ role: "user", text: "one" }, { role: "assistant", text: "ok" }], message: "two", emit: sink.emit,
    })
    expect(second.ok).toBe(true)
    expect(harness.captured[1]!.options.resume).toBe("session-keep")
    expect(harness.captured[1]!.prompt).toBe("two")
  })

  test("PROCESS RESTART: hydrated continuity stays pinned and provider init identity is reported", async () => {
    const observed: Array<Record<string, string>> = []
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-durable", model: FABLE_LOCAL_MODEL }
        yield { type: "result", subtype: "success", is_error: false, result: "continued" }
      },
      initialSessions: [{
        threadRef: "thread-restart",
        sessionId: "session-durable",
        accountRef: "claude-pylon-b",
      }],
      onDispatch: input => observed.push({ kind: "dispatch", ...input }),
      onProviderSession: input => observed.push({ kind: "provider", ...input }),
    })
    const result = await harness.runtime.runTurn({
      turnRef: "turn-restart",
      threadRef: "thread-restart",
      history: [{ role: "user", text: "old prompt" }],
      message: "explicit owner retry",
      accountRef: "claude-pylon-b",
      emit: collect().emit,
    })
    expect(result.ok).toBe(true)
    expect(harness.captured[0]!.options.resume).toBe("session-durable")
    expect(harness.captured[0]!.prompt).toBe("explicit owner retry")
    expect(observed).toEqual([
      {
        kind: "dispatch",
        threadRef: "thread-restart",
        turnRef: "turn-restart",
        accountRef: "claude-pylon-b",
      },
      {
        kind: "provider",
        threadRef: "thread-restart",
        turnRef: "turn-restart",
        accountRef: "claude-pylon-b",
        sessionId: "session-durable",
      },
    ])
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
    expect(result).toEqual({ ok: true, text: "rotated", totalTokens: null, accountRef: "claude-pylon-b" })
    // One visible turn, no turn_failed leak from the rotated-away attempt.
    expect(sink.events.map(event => event.kind)).toEqual(["turn_started", "text_delta", "turn_completed"])
    expect(captured.length).toBe(2)
    expect((captured[0]!.options.env as Record<string, string>).CLAUDE_CONFIG_DIR).toBe(join(root, ".claude-pylon-a"))
    expect((captured[1]!.options.env as Record<string, string>).CLAUDE_CONFIG_DIR).toBe(join(root, ".claude-pylon-b"))
  })

  test("an exact account target never rotates to another ready Claude home", async () => {
    const root = mkdtempSync(join(tmpdir(), "fable-local-exact-"))
    for (const name of [".claude-pylon-a", ".claude-pylon-b"]) {
      mkdirSync(join(root, name))
      writeFileSync(join(root, name, "claude-oauth-token"), `token-${name}\n`)
    }
    const captured: CapturedQuery[] = []
    const query: FableLocalQuery = call => {
      captured.push(call)
      return (async function* () {
        yield { type: "system", subtype: "init", session_id: "session-exact-fail" }
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
      turnRef: "turn-exact", threadRef: "thread-exact", history: [], message: "hi",
      accountRef: "claude-pylon-b", emit: sink.emit,
    })
    expect(result).toEqual({ ok: false, reason: "session_failed", detail: "error_during_execution" })
    expect(captured).toHaveLength(1)
    expect((captured[0]!.options.env as Record<string, string>).CLAUDE_CONFIG_DIR)
      .toBe(join(root, ".claude-pylon-b"))
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

  test("MODEL-LEVEL NO SUBSTITUTION: an init reporting a non-Fable model fails typed, streams nothing, and never rotates", async () => {
    // Two ready homes prove the no-rotation half: a model substitution is a
    // provider-side refusal, not an account failure, so the second home must
    // never be tried.
    const root = mkdtempSync(join(tmpdir(), "fable-local-model-sub-"))
    for (const name of [".claude-pylon-a", ".claude-pylon-b"]) {
      mkdirSync(join(root, name))
      writeFileSync(join(root, name, "claude-oauth-token"), `token-${name}\n`)
    }
    let calls = 0
    const query: FableLocalQuery = () => {
      calls += 1
      return (async function* () {
        yield { type: "system", subtype: "init", session_id: "session-sub", model: "claude-sonnet-4-6" }
        // A substituted model's output must NEVER surface as Fable text.
        yield { type: "stream_event", event: { type: "content_block_delta", delta: { type: "text_delta", text: "substituted output" } } }
        yield { type: "result", subtype: "success", is_error: false, result: "substituted output" }
      })()
    }
    const runtime = makeFableLocalRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "fable-local-scratch-")),
      env: { PYLON_ACCOUNT_HOME_ROOT: root },
      queryImpl: async () => query,
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-sub", threadRef: "thread-sub", history: [], message: "WHAT MODEL ARE YOU", emit: sink.emit,
    })
    expect(result).toEqual({
      ok: false,
      reason: "model_substituted",
      detail: "requested claude-fable-5, effective claude-sonnet-4-6",
    })
    expect(calls).toBe(1)
    // Visible: started, the effective model, the typed failure — zero deltas.
    expect(sink.events.map(event => event.kind)).toEqual(["turn_started", "model_effective", "turn_failed"])
    expect(sink.events.some(event => event.kind === "text_delta")).toBe(false)
    expect(sink.events[1]).toEqual({ kind: "model_effective", model: "claude-sonnet-4-6" })
    expect(sink.events[2]).toEqual({
      kind: "turn_failed",
      reason: "model_substituted",
      detail: "requested claude-fable-5, effective claude-sonnet-4-6",
    })
  })

  test("an init reporting the Fable model streams normally and emits the effective model", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-fable", model: "claude-fable-5" }
        yield { type: "stream_event", event: { type: "content_block_delta", delta: { type: "text_delta", text: "I am Fable." } } }
        yield { type: "result", subtype: "success", is_error: false, result: "I am Fable.", usage: { input_tokens: 3, output_tokens: 4 } }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-fable", threadRef: "thread-fable", history: [], message: "WHAT MODEL ARE YOU", emit: sink.emit,
    })
    expect(result).toEqual({
      ok: true,
      text: "I am Fable.",
      totalTokens: 7,
      accountRef: "claude-pylon-b",
      usage: { inputTokens: 3, cachedInputTokens: 0, outputTokens: 4, reasoningTokens: 0, totalTokens: 7 },
    })
    expect(sink.events.map(event => event.kind)).toEqual([
      "turn_started",
      "model_effective",
      "text_delta",
      "turn_completed",
    ])
    expect(sink.events[1]).toEqual({ kind: "model_effective", model: "claude-fable-5" })
  })

  test("owner-selected Opus 4.8 and Sonnet 5 slugs reach SDK Options.model exactly", async () => {
    for (const model of ["claude-opus-4-8", "claude-sonnet-5"] as const) {
      const harness = makeRuntimeHarness({
        script: async function* () {
          yield { type: "system", subtype: "init", session_id: `session-${model}`, model }
          yield { type: "result", subtype: "success", is_error: false, result: "ok" }
        },
      })
      const sink = collect()
      const result = await harness.runtime.runTurn({
        turnRef: `turn-${model}`,
        threadRef: `thread-${model}`,
        history: [],
        message: "hi",
        model,
        emit: sink.emit,
      })
      expect(result.ok).toBe(true)
      expect(harness.captured[0]!.options.model).toBe(model)
      expect(sink.events.find(event => event.kind === "model_effective")).toEqual({
        kind: "model_effective",
        model,
      })
    }
  })

  test("prefix-match tolerates versioned Fable model IDs and dedupes repeated init model reports", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-vers", model: "claude-fable-5-20260701" }
        // Real sessions can emit more than one init (seen live 2026-07-11):
        // an unchanged model is not re-announced.
        yield { type: "system", subtype: "init", session_id: "session-vers", model: "claude-fable-5-20260701" }
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-vers", threadRef: "thread-vers", history: [], message: "hi", emit: sink.emit,
    })
    expect(result).toEqual({ ok: true, text: "ok", totalTokens: null, accountRef: "claude-pylon-b" })
    expect(sink.events[1]).toEqual({ kind: "model_effective", model: "claude-fable-5-20260701" })
    expect(sink.events.filter(event => event.kind === "model_effective").length).toBe(1)
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

// ---------------------------------------------------------------------------
// EP250 owner full access (contract:
// openagents_desktop.chat.fable_local_owner_full_access.v1) + per-thread
// workspace persistence (the workspace remains the default cwd, not a bound)
// ---------------------------------------------------------------------------

type CanUseToolFn = (
  toolName: string,
  input: Record<string, unknown>,
  extra: { signal?: AbortSignal },
) => Promise<Record<string, unknown>>

const waitFor = async (predicate: () => boolean, timeoutMs = 2_000): Promise<void> => {
  const deadline = Date.now() + timeoutMs
  while (!predicate() && Date.now() < deadline) {
    await new Promise(resolve => setTimeout(resolve, 5))
  }
  expect(predicate()).toBe(true)
}

describe("EP250 owner full access: full toolset, no workspace guard", () => {
  test("canUseTool ALLOWS every non-question tool — Bash, out-of-workspace Write, WebSearch, Agent — with no denial copy anywhere", async () => {
    const decisions: Array<Record<string, unknown>> = []
    const harness = makeRuntimeHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "s-allow", model: FABLE_LOCAL_MODEL }
        const canUse = captured.options.canUseTool as CanUseToolFn
        decisions.push(await canUse("Bash", { command: "echo full access" }, { signal: new AbortController().signal }))
        decisions.push(await canUse("Write", { file_path: join(homedir(), "anywhere.md") }, { signal: new AbortController().signal }))
        decisions.push(await canUse("WebSearch", { query: "openagents" }, { signal: new AbortController().signal }))
        decisions.push(await canUse("Agent", { prompt: "subtask" }, { signal: new AbortController().signal }))
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-allow", threadRef: "thread-allow", history: [], message: "hi", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    expect(decisions).toEqual([
      { behavior: "allow", updatedInput: { command: "echo full access" } },
      { behavior: "allow", updatedInput: { file_path: join(homedir(), "anywhere.md") } },
      { behavior: "allow", updatedInput: { query: "openagents" } },
      { behavior: "allow", updatedInput: { prompt: "subtask" } },
    ])
    // The retired scoped-write lane's denial copy is GONE: nothing denies,
    // nothing mentions scope or grants.
    for (const decision of decisions) {
      expect(decision.behavior).toBe("allow")
      expect(JSON.stringify(decision).toLowerCase()).not.toContain("out of scope")
      expect(JSON.stringify(decision).toLowerCase()).not.toContain("not available in this lane")
    }
  })

  test("Write succeeds end-to-end through the fake SDK session (allowed via canUseTool, file persisted, ok tool_result)", async () => {
    const sink = collect()
    const written: string[] = []
    const harness = makeRuntimeHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "s-write", model: FABLE_LOCAL_MODEL }
        // Drive the REAL permission path the runtime installed for this
        // session: canUseTool must allow the Write (full-access lane).
        const canUse = captured.options.canUseTool as CanUseToolFn
        const target = join(String(captured.options.cwd), "greetings.md")
        const decision = await canUse("Write", { file_path: target }, { signal: new AbortController().signal })
        if ((decision as { behavior?: unknown }).behavior !== "allow") throw new Error("write was denied")
        writeFileSync(target, "# Greetings\n")
        written.push(target)
        yield {
          type: "assistant",
          message: { content: [{ type: "tool_use", id: "w1", name: "Write", input: { file_path: target } }] },
        }
        yield {
          type: "user",
          message: { content: [{ type: "tool_result", tool_use_id: "w1", content: "wrote greetings.md" }] },
        }
        yield { type: "result", subtype: "success", is_error: false, result: "Wrote greetings.md." }
      },
    })
    const result = await harness.runtime.runTurn({
      turnRef: "turn-write", threadRef: "thread-write", history: [], message: "write greetings.md", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    expect(existsSync(written[0]!)).toBe(true)
    const toolResult = sink.events.find(event => event.kind === "tool_result") as
      Extract<FableLocalEvent, { kind: "tool_result" }>
    expect(toolResult.ok).toBe(true)
    expect(toolResult.toolName).toBe("Write")
  })

  test("per-THREAD workspace: the same thread reuses one cwd across turns (files persist); another thread gets its own", async () => {
    const cwds: string[] = []
    const harness = makeRuntimeHarness({
      script: async function* (captured) {
        const cwd = String(captured.options.cwd)
        cwds.push(cwd)
        yield { type: "system", subtype: "init", session_id: `s-ws-${cwds.length}` }
        if (cwds.length === 1) writeFileSync(join(cwd, "greetings.md"), "hello")
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    const sink = collect()
    expect((await harness.runtime.runTurn({
      turnRef: "turn-ws-1", threadRef: "thread-persist", history: [], message: "one", emit: sink.emit,
    })).ok).toBe(true)
    expect((await harness.runtime.runTurn({
      turnRef: "turn-ws-2", threadRef: "thread-persist", history: [], message: "two", emit: sink.emit,
    })).ok).toBe(true)
    expect((await harness.runtime.runTurn({
      turnRef: "turn-ws-3", threadRef: "thread-other", history: [], message: "three", emit: sink.emit,
    })).ok).toBe(true)
    // Same thread -> same workspace; the follow-up turn SEES the file the
    // first turn wrote. Different thread -> a different bounded workspace.
    expect(cwds[1]).toBe(cwds[0]!)
    expect(cwds[2]).not.toBe(cwds[0]!)
    expect(cwds[0]!.startsWith(join(harness.scratch, "threads"))).toBe(true)
    expect(cwds[0]!).toContain(fableThreadWorkspaceSlug("thread-persist"))
    expect(existsSync(join(cwds[1]!, "greetings.md"))).toBe(true)
  })

  test("thread workspace slugs are sanitized, bounded, and collision-safe", () => {
    expect(fableThreadWorkspaceSlug("thread-a")).toBe(fableThreadWorkspaceSlug("thread-a"))
    expect(fableThreadWorkspaceSlug("thread-a")).not.toBe(fableThreadWorkspaceSlug("thread-b"))
    const hostile = fableThreadWorkspaceSlug("../../../etc/passwd")
    expect(hostile).not.toContain("/")
    expect(hostile).not.toContain("..")
    expect(fableThreadWorkspaceSlug("x".repeat(500)).length).toBeLessThanOrEqual(60)
  })

  test("REGRESSION: with the allow-all canUseTool, AskUserQuestion STILL parks on the question flow (never blanket-allowed)", async () => {
    // The whole reason the lane stays on permissionMode "default" instead of
    // bypassPermissions: the question flow depends on canUseTool firing.
    const sink = collect()
    const harness = makeRuntimeHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "s-q-park", model: FABLE_LOCAL_MODEL }
        const canUse = captured.options.canUseTool as CanUseToolFn
        // Bash allowed instantly…
        const bash = await canUse("Bash", { command: "ls" }, { signal: new AbortController().signal })
        expect((bash as { behavior?: unknown }).behavior).toBe("allow")
        // …while AskUserQuestion parks pending (not an instant allow).
        const pendingDecision = canUse("AskUserQuestion", {
          questions: [{
            question: "Proceed?",
            header: "Go",
            options: [{ label: "Yes" }, { label: "No" }],
            multiSelect: false,
          }],
        }, { signal: new AbortController().signal })
        const pendingEvent = sink.events.find(event => event.kind === "question_pending") as
          Extract<FableLocalEvent, { kind: "question_pending" }>
        expect(pendingEvent).toBeDefined()
        expect(harness.runtime.answerQuestion({
          turnRef: "turn-q-park", questionRef: pendingEvent.questionRef,
          answers: [{ question: "Proceed?", labels: ["Yes"] }],
        })).toBe(true)
        const answered = await pendingDecision
        expect((answered as { behavior?: unknown }).behavior).toBe("allow")
        expect((answered as { updatedInput?: { answers?: unknown } }).updatedInput?.answers)
          .toEqual({ "Proceed?": "Yes" })
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    const result = await harness.runtime.runTurn({
      turnRef: "turn-q-park", threadRef: "thread-q-park", history: [], message: "hi", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    expect(sink.events.filter(event => event.kind === "question_resolved")).toEqual([
      { kind: "question_resolved", questionRef: "q.turn-q-park.1", outcome: "answered" },
    ])
  })
})

// ---------------------------------------------------------------------------
// EP250: AskUserQuestion — a real question flow, not a dead affordance
// ---------------------------------------------------------------------------

const singleQuestionInput = (): Record<string, unknown> => ({
  questions: [
    {
      question: "Which greeting style should greetings.md use?",
      header: "Style",
      options: [
        { label: "Formal", description: "Businesslike tone" },
        { label: "Casual", description: "Friendly tone" },
      ],
      multiSelect: false,
    },
  ],
})

describe("EP250 AskUserQuestion flow", () => {
  test("question_pending -> answerQuestion -> allow with updatedInput.answers -> question_resolved answered (typed rejections along the way)", async () => {
    const decisions: Array<Record<string, unknown>> = []
    const sink = collect()
    const rawInput = singleQuestionInput()
    const harness = makeRuntimeHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "s-q1", model: FABLE_LOCAL_MODEL }
        const canUse = captured.options.canUseTool as CanUseToolFn
        const pendingDecision = canUse("AskUserQuestion", rawInput, { signal: new AbortController().signal })
        // The pending event is emitted synchronously when the tool parks.
        const pendingEvent = sink.events.find(event => event.kind === "question_pending") as
          Extract<FableLocalEvent, { kind: "question_pending" }>
        expect(pendingEvent).toBeDefined()
        expect(pendingEvent.questionRef).toBe("q.turn-q1.1")
        expect(pendingEvent.questions.length).toBe(1)
        expect(pendingEvent.questions[0]!.header).toBe("Style")
        expect(pendingEvent.questions[0]!.multiSelect).toBe(false)
        expect(pendingEvent.questions[0]!.options.map(option => option.label)).toEqual(["Formal", "Casual"])
        // Typed rejections: unknown ref, wrong turnRef, unmatched question.
        expect(harness.runtime.answerQuestion({
          turnRef: "turn-q1", questionRef: "q.nope.1",
          answers: [{ question: pendingEvent.questions[0]!.question, labels: ["Formal"] }],
        })).toBe(false)
        expect(harness.runtime.answerQuestion({
          turnRef: "another-turn", questionRef: pendingEvent.questionRef,
          answers: [{ question: pendingEvent.questions[0]!.question, labels: ["Formal"] }],
        })).toBe(false)
        expect(harness.runtime.answerQuestion({
          turnRef: "turn-q1", questionRef: pendingEvent.questionRef,
          answers: [{ question: "a question that was never asked?", labels: ["Formal"] }],
        })).toBe(false)
        // The rejected deliveries burned nothing: the question is still open.
        expect(sink.events.some(event => event.kind === "question_resolved")).toBe(false)
        expect(harness.runtime.answerQuestion({
          turnRef: "turn-q1", questionRef: pendingEvent.questionRef,
          answers: [{ question: pendingEvent.questions[0]!.question, labels: ["Formal"] }],
        })).toBe(true)
        decisions.push(await pendingDecision)
        // A second delivery after settle is a typed rejection.
        expect(harness.runtime.answerQuestion({
          turnRef: "turn-q1", questionRef: pendingEvent.questionRef,
          answers: [{ question: pendingEvent.questions[0]!.question, labels: ["Casual"] }],
        })).toBe(false)
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
    })
    const result = await harness.runtime.runTurn({
      turnRef: "turn-q1", threadRef: "thread-q1", history: [], message: "write greetings.md", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    // The SDK answer mechanism: allow + updatedInput carrying the ORIGINAL
    // questions plus the answers record keyed by original question text.
    expect(decisions[0]).toEqual({
      behavior: "allow",
      updatedInput: {
        ...rawInput,
        answers: { "Which greeting style should greetings.md use?": "Formal" },
      },
    })
    expect(sink.events.filter(event => event.kind === "question_resolved")).toEqual([
      { kind: "question_resolved", questionRef: "q.turn-q1.1", outcome: "answered" },
    ])
  })

  test("multiSelect answers join comma-separated (the SDK's documented multi-select encoding)", async () => {
    const decisions: Array<Record<string, unknown>> = []
    const sink = collect()
    const rawInput: Record<string, unknown> = {
      questions: [
        {
          question: "Which features do you want to enable?",
          header: "Features",
          options: [
            { label: "Streaming", description: "Live deltas" },
            { label: "History", description: "Bounded window" },
            { label: "Tools", description: "Read-only set" },
          ],
          multiSelect: true,
        },
      ],
    }
    const harness = makeRuntimeHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "s-q-multi", model: FABLE_LOCAL_MODEL }
        const canUse = captured.options.canUseTool as CanUseToolFn
        const pendingDecision = canUse("AskUserQuestion", rawInput, { signal: new AbortController().signal })
        const pendingEvent = sink.events.find(event => event.kind === "question_pending") as
          Extract<FableLocalEvent, { kind: "question_pending" }>
        expect(pendingEvent.questions[0]!.multiSelect).toBe(true)
        expect(harness.runtime.answerQuestion({
          turnRef: "turn-q-multi", questionRef: pendingEvent.questionRef,
          answers: [{ question: pendingEvent.questions[0]!.question, labels: ["Streaming", "Tools"] }],
        })).toBe(true)
        decisions.push(await pendingDecision)
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
    })
    const result = await harness.runtime.runTurn({
      turnRef: "turn-q-multi", threadRef: "thread-q-multi", history: [], message: "hi", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    const updated = (decisions[0] as { updatedInput?: { answers?: Record<string, string> } }).updatedInput
    expect(updated?.answers).toEqual({ "Which features do you want to enable?": "Streaming, Tools" })
  })

  test("no answer inside the question window resolves a graceful typed deny with outcome timeout", async () => {
    const decisions: Array<Record<string, unknown>> = []
    const sink = collect()
    const harness = makeRuntimeHarness({
      questionTimeoutMs: 30,
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "s-q-timeout", model: FABLE_LOCAL_MODEL }
        const canUse = captured.options.canUseTool as CanUseToolFn
        decisions.push(await canUse("AskUserQuestion", singleQuestionInput(), { signal: new AbortController().signal }))
        yield { type: "result", subtype: "success", is_error: false, result: "proceeded without input" }
      },
    })
    const result = await harness.runtime.runTurn({
      turnRef: "turn-q-timeout", threadRef: "thread-q-timeout", history: [], message: "hi", emit: sink.emit,
    })
    // Graceful: the QUESTION times out, the turn continues and completes.
    expect(result.ok).toBe(true)
    expect((decisions[0] as { behavior?: unknown }).behavior).toBe("deny")
    expect(sink.events.filter(event => event.kind === "question_resolved")).toEqual([
      { kind: "question_resolved", questionRef: "q.turn-q-timeout.1", outcome: "timeout" },
    ])
    // After timeout the ref is settled: late answers are typed rejections.
    expect(harness.runtime.answerQuestion({
      turnRef: "turn-q-timeout", questionRef: "q.turn-q-timeout.1",
      answers: [{ question: "Which greeting style should greetings.md use?", labels: ["Formal"] }],
    })).toBe(false)
  })

  test("interrupting the turn denies the pending question with outcome denied", async () => {
    const sink = collect()
    const harness = makeRuntimeHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "s-q-int", model: FABLE_LOCAL_MODEL }
        const canUse = captured.options.canUseTool as CanUseToolFn
        const decision = await canUse("AskUserQuestion", singleQuestionInput(), { signal: new AbortController().signal })
        expect((decision as { behavior?: unknown }).behavior).toBe("deny")
        // Mirror the real SDK: the aborted session throws out of the stream.
        throw new Error("aborted by controller")
      },
    })
    const pending = harness.runtime.runTurn({
      turnRef: "turn-q-int", threadRef: "thread-q-int", history: [], message: "hi", emit: sink.emit,
    })
    await waitFor(() => sink.events.some(event => event.kind === "question_pending"))
    expect(harness.runtime.interrupt("turn-q-int")).toBe(true)
    const result = await pending
    expect(result).toEqual({ ok: false, reason: "interrupted", detail: "turn interrupted" })
    expect(sink.events.filter(event => event.kind === "question_resolved")).toEqual([
      { kind: "question_resolved", questionRef: "q.turn-q-int.1", outcome: "denied" },
    ])
  })

  test("malformed AskUserQuestion input is denied without parking a question", async () => {
    const decisions: Array<Record<string, unknown>> = []
    const sink = collect()
    const harness = makeRuntimeHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "s-q-bad", model: FABLE_LOCAL_MODEL }
        const canUse = captured.options.canUseTool as CanUseToolFn
        decisions.push(await canUse("AskUserQuestion", { questions: [] }, { signal: new AbortController().signal }))
        decisions.push(await canUse("AskUserQuestion", { questions: [{ question: "No options?" }] }, { signal: new AbortController().signal }))
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    const result = await harness.runtime.runTurn({
      turnRef: "turn-q-bad", threadRef: "thread-q-bad", history: [], message: "hi", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    for (const decision of decisions) {
      expect((decision as { behavior?: unknown }).behavior).toBe("deny")
    }
    expect(sink.events.some(event => event.kind === "question_pending")).toBe(false)
    expect(sink.events.some(event => event.kind === "question_resolved")).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Codex delegation (#8712 Lane C)
// ---------------------------------------------------------------------------

const makeDelegateHarness = (input: {
  script: (captured: CapturedQuery) => AsyncIterable<unknown>
  delegate: FableDelegateRuntime
}) => {
  const root = makeAccountRoot()
  const captured: CapturedQuery[] = []
  const query: FableLocalQuery = call => {
    captured.push(call)
    return input.script(call)
  }
  const scratch = mkdtempSync(join(tmpdir(), "fable-delegate-scratch-"))
  const runtime = makeFableLocalRuntime({
    scratchRoot: () => scratch,
    env: { PYLON_ACCOUNT_HOME_ROOT: root },
    queryImpl: async () => query,
    delegate: input.delegate,
    mcpImpl: async () => makeFixtureFableMcpFactory(),
  })
  return { runtime, captured, scratch }
}

const delegateToolFrom = (captured: CapturedQuery): FixtureFableMcpTool => {
  const servers = captured.options.mcpServers as Record<string, { tools: Array<FixtureFableMcpTool> }>
  return servers.codex!.tools[0]!
}

const okChild = (accountRef: string): ReturnType<FableDelegateRuntime["runChild"]> =>
  Promise.resolve({
    ok: true,
    text: "child answer",
    usage: null,
    threadId: null,
    accountRef,
    requestedModel: "gpt-5.6-sol",
    requestedEffort: "medium",
    durationMs: 5,
  })

describe("Codex delegation through the Fable lane", () => {
  test("delegation-enabled sessions auto-allow mcp__codex__delegate, expose the codex SDK MCP server, the scratch-dir guidance, and the raised stream-close timeout", async () => {
    const harness = makeDelegateHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-del", model: FABLE_LOCAL_MODEL }
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
      delegate: { runChild: () => okChild("codex") },
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-del-opts", threadRef: "thread-del-opts", history: [], message: "hi", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    const call = harness.captured[0]!
    // Full-access lane: the ONLY allowedTools entry is the delegate
    // auto-allow — no restriction on the rest of the toolset.
    expect(call.options.allowedTools).toEqual([FABLE_DELEGATE_TOOL_NAME])
    const servers = call.options.mcpServers as Record<string, { name?: string; tools?: Array<{ name?: string; description?: string }> }>
    expect(servers.codex).toBeDefined()
    expect(servers.codex!.tools![0]!.name).toBe("delegate")
    const description = servers.codex!.tools![0]!.description ?? ""
    // The contract docstring states the spawn-config limitation explicitly.
    expect(description).toContain("gpt-5.6-sol, medium reasoning")
    expect(description).toContain("spawn config")
    // EP250 empty-scratch guidance: children START in an empty scratch dir,
    // so Fable must pass absolute paths for anything they should read —
    // this is what fixes "explore this codebase" yielding empty-dir walks.
    expect(description).toContain("EMPTY scratch directory")
    expect(description).toContain("absolute paths")
    const env = call.options.env as Record<string, string | undefined>
    expect(env.CLAUDE_CODE_STREAM_CLOSE_TIMEOUT).toBe(String(FABLE_STREAM_CLOSE_TIMEOUT_MS))
  })

  test("without a delegate there is no allowlist at all (no mcpServers, no delegate name, no stream-close env)", async () => {
    const harness = makeRuntimeHarness({
      script: async function* () {
        yield { type: "system", subtype: "init", session_id: "session-plain" }
        yield { type: "result", subtype: "success", is_error: false, result: "ok" }
      },
    })
    const sink = collect()
    await harness.runtime.runTurn({
      turnRef: "turn-plain", threadRef: "thread-plain", history: [], message: "hi", emit: sink.emit,
    })
    const call = harness.captured[0]!
    expect(call.options.allowedTools).toBeUndefined()
    expect(call.options.mcpServers).toBeUndefined()
    expect((call.options.env as Record<string, string | undefined>).CLAUDE_CODE_STREAM_CLOSE_TIMEOUT).toBeUndefined()
  })

  test("one delegate call end-to-end: revoked account rotates TYPED, exact child usage flows into child_completed and the tool result footer", async () => {
    const delegate = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => [
        { ref: "codex", home: "/isolated/codex" },
        { ref: "codex-2", home: "/isolated/codex-2" },
      ],
      health: makeCodexAccountHealth(),
    })
    const toolResults: Array<Record<string, unknown>> = []
    const harness = makeDelegateHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "session-e2e", model: FABLE_LOCAL_MODEL }
        const raw = await delegateToolFrom(captured).handler({ task: "summarize the notes" }, {})
        toolResults.push(raw as Record<string, unknown>)
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
      delegate,
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-e2e", threadRef: "thread-e2e", history: [], message: "go", emit: sink.emit,
    })
    expect(result.ok).toBe(true)

    const kinds = sink.events.map(event => event.kind)
    expect(kinds).toContain("child_started")
    expect(kinds).toContain("child_activity")
    expect(kinds).toContain("child_completed")
    const started = sink.events.find(event => event.kind === "child_started") as Extract<FableLocalEvent, { kind: "child_started" }>
    expect(started.childRef).toBe("child.codex.turn-e2e.1")
    expect(started.summary).toContain("summarize the notes")
    expect(started.prompt).toContain("summarize the notes")
    // The revoked account was skipped VISIBLY — typed activity, never silent.
    const reconnect = sink.events.find(event =>
      event.kind === "child_activity" && event.activity === "account_reconnect_required") as Extract<FableLocalEvent, { kind: "child_activity" }>
    expect(reconnect.accountRef).toBe("codex")
    expect(reconnect.summary).toContain("reconnect")
    const completed = sink.events.find(event => event.kind === "child_completed") as Extract<FableLocalEvent, { kind: "child_completed" }>
    expect(completed.accountRef).toBe("codex-2")
    expect(completed.response).toBe(FIXTURE_CODEX_CHILD_TEXT)
    expect(completed.usage).toEqual({
      inputTokens: 1200,
      cachedInputTokens: 900,
      outputTokens: 180,
      reasoningTokens: 60,
      totalTokens: 1440,
    })

    // Tool result: the child's final answer plus the labeled usage footer.
    const content = (toolResults[0]!.content as Array<{ text: string }>)[0]!.text
    expect(toolResults[0]!.isError).toBeUndefined()
    expect(content).toContain(FIXTURE_CODEX_CHILD_TEXT)
    expect(content).toContain("account codex-2")
    expect(content).toContain("gpt-5.6-sol (requested, medium reasoning)")
    expect(content).toContain("1,440 tokens")
  })

  test("all-revoked accounts yield a typed child_failed event and an isError tool result naming the reconnect need", async () => {
    const delegate = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
      ]),
      discoverImpl: async () => [
        { ref: "codex", home: "/isolated/codex" },
        { ref: "codex-2", home: "/isolated/codex-2" },
      ],
      health: makeCodexAccountHealth(),
    })
    const toolResults: Array<Record<string, unknown>> = []
    const harness = makeDelegateHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "session-revoked", model: FABLE_LOCAL_MODEL }
        toolResults.push(await delegateToolFrom(captured).handler({ task: "anything" }, {}) as Record<string, unknown>)
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
      delegate,
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-revoked", threadRef: "thread-revoked", history: [], message: "go", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    const failed = sink.events.find(event => event.kind === "child_failed") as Extract<FableLocalEvent, { kind: "child_failed" }>
    expect(failed.reason).toBe("account_reconnect_required")
    expect(failed.detail).toContain("reconnect")
    expect(toolResults[0]!.isError).toBe(true)
    const text = (toolResults[0]!.content as Array<{ text: string }>)[0]!.text
    expect(text).toContain("all 2 available Codex session(s) need reconnect")
    expect(text).toContain("No Codex child produced output.")
    // Both revoked accounts surfaced typed activity events (never silent).
    expect(sink.events.filter(event =>
      event.kind === "child_activity" && event.activity === "account_reconnect_required")).toHaveLength(2)
  })

  test("3 concurrent delegate calls run; the 4th simultaneous call is a typed refusal without spawning", async () => {
    let started = 0
    let release: (() => void) | null = null
    const gate = new Promise<void>(resolve => {
      release = resolve
    })
    const delegate: FableDelegateRuntime = {
      runChild: async input => {
        started += 1
        await gate
        return okChild(`codex-${input.childRef.slice(-1)}`) as never
      },
    }
    const toolResults: Array<Record<string, unknown>> = []
    const harness = makeDelegateHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "session-conc", model: FABLE_LOCAL_MODEL }
        const tool = delegateToolFrom(captured)
        const first3 = [
          tool.handler({ task: "a" }, {}),
          tool.handler({ task: "b" }, {}),
          tool.handler({ task: "c" }, {}),
        ]
        // Let the three children reach the runtime before the 4th call.
        await new Promise(resolve => setTimeout(resolve, 10))
        toolResults.push(await tool.handler({ task: "d" }, {}) as Record<string, unknown>)
        release!()
        toolResults.push(...await Promise.all(first3) as Array<Record<string, unknown>>)
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
      delegate,
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-conc", threadRef: "thread-conc", history: [], message: "go", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    expect(started).toBe(FABLE_DELEGATE_MAX_CONCURRENT)
    const refusal = (toolResults[0]!.content as Array<{ text: string }>)[0]!.text
    expect(toolResults[0]!.isError).toBe(true)
    expect(refusal).toContain("concurrency cap")
    expect(refusal).toContain("No child was spawned.")
    // The three admitted children all completed.
    expect(sink.events.filter(event => event.kind === "child_completed")).toHaveLength(3)
  })

  test("the 7th child in one turn is refused by the per-turn cap (6 spawned, typed refusal text)", async () => {
    let spawned = 0
    const delegate: FableDelegateRuntime = {
      runChild: async () => {
        spawned += 1
        return okChild("codex") as never
      },
    }
    const toolResults: Array<Record<string, unknown>> = []
    const harness = makeDelegateHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "session-cap", model: FABLE_LOCAL_MODEL }
        const tool = delegateToolFrom(captured)
        for (let index = 0; index < FABLE_DELEGATE_MAX_CHILDREN_PER_TURN + 1; index += 1) {
          toolResults.push(await tool.handler({ task: `task ${index}` }, {}) as Record<string, unknown>)
        }
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
      delegate,
    })
    const sink = collect()
    const result = await harness.runtime.runTurn({
      turnRef: "turn-cap", threadRef: "thread-cap", history: [], message: "go", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    expect(spawned).toBe(FABLE_DELEGATE_MAX_CHILDREN_PER_TURN)
    const last = toolResults.at(-1)!
    expect(last.isError).toBe(true)
    expect((last.content as Array<{ text: string }>)[0]!.text).toContain("per-turn cap")
  })

  test("an empty task is refused typed without spawning", async () => {
    let spawned = 0
    const delegate: FableDelegateRuntime = {
      runChild: async () => {
        spawned += 1
        return okChild("codex") as never
      },
    }
    const toolResults: Array<Record<string, unknown>> = []
    const harness = makeDelegateHarness({
      script: async function* (captured) {
        yield { type: "system", subtype: "init", session_id: "session-empty", model: FABLE_LOCAL_MODEL }
        toolResults.push(await delegateToolFrom(captured).handler({ task: "   " }, {}) as Record<string, unknown>)
        yield { type: "result", subtype: "success", is_error: false, result: "done" }
      },
      delegate,
    })
    const sink = collect()
    await harness.runtime.runTurn({
      turnRef: "turn-empty", threadRef: "thread-empty", history: [], message: "go", emit: sink.emit,
    })
    expect(spawned).toBe(0)
    expect(toolResults[0]!.isError).toBe(true)
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
    expect(result).toEqual({
      ok: true,
      text: "Fable local **streaming** proof.",
      totalTokens: 49,
      accountRef: "claude-pylon-fixture",
      usage: { inputTokens: 42, cachedInputTokens: 0, outputTokens: 7, reasoningTokens: 0, totalTokens: 49 },
    })
    expect(sink.events.map(event => event.kind)).toEqual([
      "turn_started",
      "model_effective",
      "text_delta",
      "text_delta",
      "tool_use",
      "tool_result",
      // EP250 wave-2: the fixture's TodoWrite emits a tool_use plus a
      // plan_updated (J2/J4), then its tool_result.
      "tool_use",
      "plan_updated",
      "tool_result",
      "text_delta",
      "turn_completed",
    ])
    expect(sink.events[1]).toEqual({ kind: "model_effective", model: "claude-fable-5" })
    const plan = sink.events.find(event => event.kind === "plan_updated") as Extract<FableLocalEvent, { kind: "plan_updated" }>
    expect(plan.entries).toEqual([
      { step: "Read the fixture notes", status: "completed" },
      { step: "Summarize for the user", status: "in_progress" },
    ])
  })

  test("with the fixture MCP factory + scripted child, the fixture turn shows the delegate tool_use/tool_result pair and child lifecycle (the smoke's deterministic delegation proof)", async () => {
    const delegate = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "codex-child-scratch-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => [
        { ref: "codex", home: "/isolated/codex" },
        { ref: "codex-2", home: "/isolated/codex-2" },
      ],
      health: makeCodexAccountHealth(),
    })
    const runtime = makeFableLocalRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "fable-local-fixture-")),
      queryImpl: async () => makeFixtureFableLocalQuery(),
      discoverImpl: async () => [{ ref: "claude-pylon-fixture", home: "/nonexistent" }],
      delegate,
      mcpImpl: async () => makeFixtureFableMcpFactory(),
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-fixture-delegate", threadRef: "thread-fixture-delegate", history: [], message: "go", emit: sink.emit,
    })
    expect(result.ok).toBe(true)
    const kinds = sink.events.map(event => event.kind)
    // The mapped pair the transcript renders (existing tool_use/tool_result
    // rendering — no new transcript components needed)…
    const delegateUse = sink.events.find(event =>
      event.kind === "tool_use" && event.toolName === FABLE_DELEGATE_TOOL_NAME) as Extract<FableLocalEvent, { kind: "tool_use" }>
    expect(delegateUse.summary).toContain(FABLE_FIXTURE_DELEGATE_TASK)
    const delegateResult = sink.events.find(event =>
      event.kind === "tool_result" && event.toolName === FABLE_DELEGATE_TOOL_NAME) as Extract<FableLocalEvent, { kind: "tool_result" }>
    expect(delegateResult.ok).toBe(true)
    expect(delegateResult.summary).toContain(FIXTURE_CODEX_CHILD_TEXT)
    // …plus the child lifecycle events for the fleet/ledger side.
    expect(kinds).toContain("child_started")
    expect(kinds).toContain("child_completed")
    expect(kinds.filter(kind => kind === "child_activity").length).toBeGreaterThanOrEqual(2)
  })
})
