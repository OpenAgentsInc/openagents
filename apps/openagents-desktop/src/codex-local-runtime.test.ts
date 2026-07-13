/**
 * Codex local runtime tests (EP250 #8712 codex-first-class). Enforces the
 * codex lane no-substitution contract: receipted spawn recipe (NO --ephemeral
 * on the chat lane — sessions persist for resume + receipts), `exec resume
 * <thread_id>` continuation on the SAME account (receipted live 2026-07-11:
 * codeword recall + identical thread_id), bounded-history fallback when
 * rotation lands on a different account, streaming mapping into the frozen
 * fable-local envelope (reasoning lines, Bash tool cards, text deltas, exact
 * usage), typed visible rotation, interrupt, and PROBE-VERIFIED availability.
 */
import { describe, expect, test } from "bun:test"
import { existsSync, mkdtempSync, readFileSync } from "node:fs"
import { EventEmitter } from "node:events"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { PassThrough } from "node:stream"
import type { CodexAppServerSpawn } from "./codex-app-server-client.ts"

import {
  fixtureCodexShortAuthStdout,
  fixtureCodexSuccessStdout,
  makeCodexAccountHealth,
  makeFixtureCodexChildSpawn,
  type CodexChildAccount,
} from "./codex-child-runtime.ts"
import {
  FIXTURE_CODEX_LOCAL_TEXT,
  fixtureCodexLocalTurnStdout,
  makeCodexLocalRuntime,
} from "./codex-local-runtime.ts"
import type { CodexProbeResult } from "./codex-preflight.ts"
import type { FableLocalEvent } from "./fable-local-contract.ts"
import { fableThreadWorkspaceSlug } from "./fable-local-runtime.ts"

const accounts: ReadonlyArray<CodexChildAccount> = [
  { ref: "codex", home: "/isolated/accounts/codex/codex" },
  { ref: "codex-2", home: "/isolated/accounts/codex/codex-2" },
]

type SpawnCapture = { args: ReadonlyArray<string>; env: Record<string, string | undefined>; cwd: string }

const scratch = (): string => mkdtempSync(join(tmpdir(), "codex-local-"))

const collect = () => {
  const events: FableLocalEvent[] = []
  return { events, emit: (event: FableLocalEvent) => events.push(event) }
}

const verifiedPreflight = (refs: ReadonlyArray<string>) => ({
  probeAll: async () => [] as ReadonlyArray<CodexProbeResult>,
  ensureProbed: async () => [] as ReadonlyArray<CodexProbeResult>,
  results: () => [] as ReadonlyArray<CodexProbeResult>,
  verifiedRefs: () => refs,
})

const appServerFixture = () => {
  const stdin = new PassThrough()
  const stdout = new PassThrough()
  const child = new EventEmitter() as EventEmitter & {
    stdin: PassThrough
    stdout: PassThrough
    stderr: PassThrough
    kill: () => boolean
  }
  child.stdin = stdin
  child.stdout = stdout
  child.stderr = new PassThrough()
  child.kill = () => { child.emit("close", 0); return true }
  const messages: Array<Record<string, unknown>> = []
  let buffered = ""
  stdin.on("data", chunk => {
    buffered += chunk.toString("utf8")
    while (buffered.includes("\n")) {
      const newline = buffered.indexOf("\n")
      const line = buffered.slice(0, newline)
      buffered = buffered.slice(newline + 1)
      if (line !== "") messages.push(JSON.parse(line))
    }
  })
  return {
    messages,
    spawn: (() => child) as unknown as CodexAppServerSpawn,
    respond: (id: number, result: unknown) => stdout.write(`${JSON.stringify({ id, result })}\n`),
    request: (id: number, method: string, params: unknown) => stdout.write(`${JSON.stringify({ id, method, params })}\n`),
    notify: (method: string, params: unknown) => stdout.write(`${JSON.stringify({ method, params })}\n`),
  }
}

const waitFor = async (messages: ReadonlyArray<unknown>, count: number): Promise<void> => {
  for (let attempt = 0; attempt < 100 && messages.length < count; attempt += 1) await Bun.sleep(1)
  expect(messages.length).toBeGreaterThanOrEqual(count)
}

describe("makeCodexLocalRuntime.runTurn", () => {
  test("reports a typed incompatible_workflow instead of falling back when app-server is unavailable", async () => {
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      discoverImpl: async () => [accounts[0]!],
      health: makeCodexAccountHealth(),
      preflight: verifiedPreflight(["codex"]),
      appServer: {
        binary: () => null,
        installProductSpecSkill: () => { throw new Error("must not install") },
      },
    })
    const result = await runtime.runTurn({
      turnRef: "turn-incompatible",
      threadRef: "thread-incompatible",
      history: [],
      message: "execute ProductSpec",
      emit: collect().emit,
    })
    expect(result).toMatchObject({ ok: false, reason: "incompatible_workflow" })
  })

  test("production app-server path excludes ambient Codex and completes a native question round-trip", async () => {
    const fake = appServerFixture()
    const sink = collect()
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      workspaceRoot: scratch,
      discoverImpl: async () => [
        { ref: "ambient", home: "/owner/.codex", source: "current_session" },
        accounts[0]!,
      ],
      health: makeCodexAccountHealth(),
      preflight: verifiedPreflight(["ambient", "codex"]),
      appServer: {
        binary: () => "/packaged/codex",
        installProductSpecSkill: account => ({
          skillRoot: `${account.home}/skills`,
          skillPath: `${account.home}/skills/productspec-work/SKILL.md`,
        }),
        spawnImpl: fake.spawn,
      },
    })
    expect(await runtime.availability()).toEqual({ state: "available", accountRef: "codex", verifiedCount: 1 })
    const running = runtime.runTurn({
      turnRef: "turn-native-question",
      threadRef: "thread-native-question",
      history: [],
      message: "Ask then answer",
      emit: sink.emit,
    })
    await waitFor(fake.messages, 1); fake.respond(1, {})
    await waitFor(fake.messages, 3); fake.respond(2, {})
    await waitFor(fake.messages, 4); fake.respond(3, {})
    await waitFor(fake.messages, 5); fake.respond(4, { data: [{ skills: [{
      name: "productspec-work",
      path: "/isolated/accounts/codex/codex/skills/productspec-work/SKILL.md",
      enabled: true,
    }] }] })
    await waitFor(fake.messages, 6); fake.respond(5, { thread: { id: "native-thread" } })
    await waitFor(fake.messages, 7); fake.respond(6, { turn: { id: "native-turn" } })
    await Bun.sleep(0)
    const steering = runtime.steerCurrent({
      threadRef: "thread-native-question",
      message: "Use the native path",
    })
    await waitFor(fake.messages, 8)
    expect(fake.messages[7]).toMatchObject({
      method: "turn/steer",
      params: { expectedTurnId: "native-turn", input: [{ type: "text", text: "Use the native path" }] },
    })
    fake.respond(7, {})
    await expect(steering).resolves.toEqual({ ok: true, outcome: "delivered" })
    expect(runtime.queueFollowup({
      threadRef: "thread-native-question",
      message: "Run the checks next",
    })).toMatchObject({ ok: true, queued: true, position: 1 })
    fake.request(91, "item/commandExecution/requestApproval", {
      threadId: "native-thread",
      turnId: "native-turn",
      itemId: "command-item",
      startedAtMs: Date.now(),
      command: "bun test",
      reason: "Run the focused checks",
    })
    for (let attempt = 0; attempt < 100 && !sink.events.some(event =>
      event.kind === "question_pending" && event.interactionKind === "tool_approval"); attempt += 1) await Bun.sleep(1)
    const approval = sink.events.find(event =>
      event.kind === "question_pending" && event.interactionKind === "tool_approval")
    expect(approval).toMatchObject({
      kind: "question_pending",
      interactionKind: "tool_approval",
      decisionRef: "91",
      questions: [{ header: "Command approval", question: "bun test" }],
    })
    if (approval?.kind !== "question_pending") throw new Error("approval was not projected")
    expect(runtime.answerQuestion({
      turnRef: "turn-native-question",
      questionRef: approval.questionRef,
      answers: [{ question: "bun test", labels: ["Allow once"] }],
    })).toBe(true)
    await waitFor(fake.messages, 9)
    expect(fake.messages[8]).toEqual({ id: 91, result: { decision: "accept" } })
    fake.request(90, "item/tool/requestUserInput", {
      threadId: "native-thread",
      turnId: "native-turn",
      itemId: "question-item",
      questions: [{
        id: "choice",
        header: "Approach",
        question: "Which implementation?",
        options: [{ label: "Native", description: "Use app-server" }],
      }],
    })
    for (let attempt = 0; attempt < 100 && !sink.events.some(event =>
      event.kind === "question_pending" && event.interactionKind === undefined); attempt += 1) await Bun.sleep(1)
    const pending = sink.events.find(event => event.kind === "question_pending" && event.interactionKind === undefined)
    expect(pending).toMatchObject({ kind: "question_pending", questions: [{ question: "Which implementation?" }] })
    if (pending?.kind !== "question_pending") throw new Error("question was not projected")
    expect(runtime.answerQuestion({
      turnRef: "turn-native-question",
      questionRef: pending.questionRef,
      answers: [{ question: "Which implementation?", labels: ["Native"] }],
    })).toBe(true)
    await waitFor(fake.messages, 10)
    expect(fake.messages[9]).toEqual({ id: 90, result: { answers: { choice: { answers: ["Native"] } } } })
    fake.notify("item/agentMessage/delta", { threadId: "native-thread", turnId: "native-turn", delta: "Done." })
    fake.notify("turn/completed", { threadId: "native-thread", turn: { id: "native-turn", status: "completed", error: null } })
    await expect(running).resolves.toMatchObject({ ok: true, text: "Done.", accountRef: "codex", threadId: "native-thread" })
    expect(sink.events).toContainEqual({ kind: "question_resolved", questionRef: pending.questionRef, outcome: "answered" })
    expect(sink.events).toContainEqual(expect.objectContaining({ kind: "followup_queued", position: 1 }))
    expect(sink.events).toContainEqual(expect.objectContaining({
      kind: "followup_promoted",
      message: "Run the checks next",
    }))
  })

  test("an explicit workspace root is the exact Codex cwd", async () => {
    const captured: SpawnCapture[] = []
    const workspace = scratch()
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      workspaceRoot: () => workspace,
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexLocalTurnStdout(), exitCode: 0 }],
        input => captured.push(input),
      ),
      discoverImpl: async () => [accounts[0]!],
      health: makeCodexAccountHealth(),
    })
    await runtime.runTurn({
      turnRef: "turn-workspace", threadRef: "thread-workspace", history: [], message: "work here", emit: () => {},
    })
    expect(captured[0]?.cwd).toBe(workspace)
    expect(captured[0]?.args).toContain(workspace)
  })

  test("prefers the ordinary Codex session without inheriting a stale Pylon CODEX_HOME", async () => {
    const captured: SpawnCapture[] = []
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      env: { HOME: "/owner", CODEX_HOME: "/stale/pylon-home", PATH: "/usr/bin" },
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexLocalTurnStdout(), exitCode: 0 }],
        input => captured.push(input),
      ),
      discoverImpl: async () => [
        { ref: "codex-current", home: "/owner/.codex", source: "current_session" },
        accounts[0]!,
      ],
      health: makeCodexAccountHealth(),
    })
    const result = await runtime.runTurn({
      turnRef: "turn-current",
      threadRef: "thread-current",
      history: [],
      message: "hello",
      emit: () => {},
    })
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.accountRef).toBe("codex-current")
    expect(captured[0]!.env.HOME).toBe("/owner")
    expect(captured[0]!.env.CODEX_HOME).toBeUndefined()
  })

  test("fresh turn spawns the receipted chat recipe with the owner-selected model and reasoning effort", async () => {
    const captured: SpawnCapture[] = []
    const root = scratch()
    const runtime = makeCodexLocalRuntime({
      scratchRoot: () => root,
      env: { PATH: "/usr/bin" },
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexLocalTurnStdout(), exitCode: 0 }],
        input => captured.push(input),
      ),
      discoverImpl: async () => [accounts[0]!],
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-1",
      threadRef: "thread-1",
      history: [],
      message: "hello codex",
      model: "gpt-5.5",
      reasoningEffort: "high",
      emit: sink.emit,
    })
    if (!result.ok) throw new Error(`expected success, got ${result.reason}: ${result.detail}`)
    const workspace = join(root, "threads", fableThreadWorkspaceSlug("thread-1"))
    expect(captured[0]!.args).toEqual([
      "exec",
      "--json",
      "-m",
      "gpt-5.5",
      "-c",
      "model_reasoning_effort=high",
      "-s",
      "danger-full-access",
      "--skip-git-repo-check",
      "-C",
      workspace,
      "hello codex",
    ])
    // DECISION (receipted): the chat lane persists sessions — NO --ephemeral,
    // so the rollout JSONL lands under the isolated home for resume/receipts.
    expect(captured[0]!.args).not.toContain("--ephemeral")
    expect(captured[0]!.env.CODEX_HOME).toBe("/isolated/accounts/codex/codex")
    expect(captured[0]!.cwd).toBe(workspace)
    expect(sink.events.find(event => event.kind === "model_effective")).toEqual({
      kind: "model_effective",
      model: "gpt-5.5 (requested)",
    })
  })

  test("capability I1: attached images are written to the turn workspace and passed as `-i <path>` before the prompt positional", async () => {
    const captured: SpawnCapture[] = []
    const root = scratch()
    const runtime = makeCodexLocalRuntime({
      scratchRoot: () => root,
      env: { PATH: "/usr/bin" },
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexLocalTurnStdout(), exitCode: 0 }],
        input => captured.push(input),
      ),
      discoverImpl: async () => [accounts[0]!],
      health: makeCodexAccountHealth(),
    })
    const result = await runtime.runTurn({
      turnRef: "turn-img",
      threadRef: "thread-img",
      history: [],
      message: "review this screenshot",
      images: [
        { mediaType: "image/png", data: "aGVsbG8=", name: "shot.png" },
        { mediaType: "image/jpeg", data: "d29ybGQ=" },
      ],
      emit: collect().emit,
    })
    if (!result.ok) throw new Error(`expected success, got ${result.reason}: ${result.detail}`)
    const args = captured[0]!.args
    // Two `-i <path>` flag pairs, each pointing at a real written file.
    const imageArgs = args.filter((_, index) => args[index - 1] === "-i")
    expect(imageArgs).toHaveLength(2)
    expect(imageArgs[0]!.endsWith(".png")).toBe(true)
    expect(imageArgs[1]!.endsWith(".jpg")).toBe(true)
    for (const imagePath of imageArgs) {
      expect(existsSync(imagePath)).toBe(true)
    }
    // The bytes on disk are the decoded base64 (not the base64 text).
    expect(readFileSync(imageArgs[0]!).toString("utf8")).toBe("hello")
    // The `-i` list is terminated by `-C` before the positional prompt so the
    // variadic `--image` never swallows the prompt.
    const dashC = args.indexOf("-C")
    expect(dashC).toBeGreaterThan(args.lastIndexOf("-i"))
    expect(args[args.length - 1]).toBe("review this screenshot")
  })

  test("streams the full mapping: model caption (spawn-config truth), reasoning line, Bash tool card pair, text delta, exact usage split", async () => {
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: fixtureCodexLocalTurnStdout("t-map"), exitCode: 0 }]),
      discoverImpl: async () => [accounts[0]!],
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-map",
      threadRef: "thread-map",
      history: [],
      message: "go",
      emit: sink.emit,
    })
    if (!result.ok) throw new Error(`expected success, got ${result.reason}`)
    expect(result.text).toBe(FIXTURE_CODEX_LOCAL_TEXT)
    expect(result.threadId).toBe("t-map")
    expect(result.accountRef).toBe("codex")
    // Exact usage: total = input + output + reasoning; cached separate.
    expect(result.usage).toEqual({
      inputTokens: 900,
      cachedInputTokens: 600,
      outputTokens: 40,
      reasoningTokens: 12,
      totalTokens: 952,
    })
    expect(result.totalTokens).toBe(952)
    const kinds = sink.events.map(event => event.kind)
    expect(kinds[0]).toBe("turn_started")
    // Spawn-config truth caption — never an unlabeled provider echo.
    const model = sink.events.find(event => event.kind === "model_effective") as
      Extract<FableLocalEvent, { kind: "model_effective" }>
    expect(model.model).toBe("gpt-5.6-sol (requested)")
    const reasoning = sink.events.find(event => event.kind === "reasoning") as
      Extract<FableLocalEvent, { kind: "reasoning" }>
    expect(reasoning.text).toBe("planned the fixture reply")
    const toolUse = sink.events.find(event => event.kind === "tool_use") as
      Extract<FableLocalEvent, { kind: "tool_use" }>
    expect(toolUse.toolName).toBe("Bash")
    // JSON-args shape (same as the fable lane) so the shared tool-card
    // humanizer extracts the command for the card detail line.
    expect(toolUse.summary).toBe('{"command":"echo fixture"}')
    const toolResult = sink.events.find(event => event.kind === "tool_result") as
      Extract<FableLocalEvent, { kind: "tool_result" }>
    expect(toolResult.ok).toBe(true)
    expect(toolResult.summary).toBe("fixture")
    const deltas = sink.events.filter(event => event.kind === "text_delta") as
      Array<Extract<FableLocalEvent, { kind: "text_delta" }>>
    expect(deltas.map(delta => delta.text).join("")).toBe(FIXTURE_CODEX_LOCAL_TEXT)
    const completed = sink.events.find(event => event.kind === "turn_completed") as
      Extract<FableLocalEvent, { kind: "turn_completed" }>
    expect(completed.accountRef).toBe("codex")
    expect(completed.usage?.totalTokens).toBe(952)
  })

  test("MULTI-TURN: the second turn on the same thread RESUMES the codex thread id on the same account (exec resume, sandbox via -c, no --ephemeral)", async () => {
    const captured: SpawnCapture[] = []
    const root = scratch()
    const runtime = makeCodexLocalRuntime({
      scratchRoot: () => root,
      spawnImpl: makeFixtureCodexChildSpawn(
        [
          { stdout: fixtureCodexLocalTurnStdout("thread-abc-123"), exitCode: 0 },
          { stdout: fixtureCodexLocalTurnStdout("thread-abc-123"), exitCode: 0 },
        ],
        input => captured.push(input),
      ),
      discoverImpl: async () => [accounts[0]!],
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const first = await runtime.runTurn({
      turnRef: "turn-a",
      threadRef: "thread-resume",
      history: [],
      message: "remember ZEBRA-42",
      emit: sink.emit,
    })
    if (!first.ok) throw new Error("first turn failed")
    const second = await runtime.runTurn({
      turnRef: "turn-b",
      threadRef: "thread-resume",
      history: [
        { role: "user", text: "remember ZEBRA-42" },
        { role: "assistant", text: "stored" },
      ],
      message: "what was the codeword?",
      emit: sink.emit,
    })
    if (!second.ok) throw new Error("second turn failed")
    const resumeArgs = captured[1]!.args
    expect(resumeArgs.slice(0, 3)).toEqual(["exec", "resume", "thread-abc-123"])
    expect(resumeArgs).toContain("--json")
    expect(resumeArgs).toContain('sandbox_mode="danger-full-access"')
    // `exec resume` has no -s flag (receipted); sandbox rides -c.
    expect(resumeArgs).not.toContain("-s")
    expect(resumeArgs).not.toContain("--ephemeral")
    // Resumed sessions carry their own context: the prompt is ONLY the new
    // message, never the history prepend.
    expect(resumeArgs[resumeArgs.length - 1]).toBe("what was the codeword?")
  })

  test("PROCESS RESTART: durable continuity resumes the exact recorded account/thread once and reports provider identity", async () => {
    const captured: SpawnCapture[] = []
    const observed: Array<Record<string, string>> = []
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      initialSessions: [{ threadRef: "thread-restart", threadId: "thread-durable-1", accountRef: "codex-2" }],
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexLocalTurnStdout("thread-durable-1"), exitCode: 0 }],
        input => captured.push(input),
      ),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
      onDispatch: input => observed.push({ kind: "dispatch", ...input }),
      onProviderSession: input => observed.push({ kind: "provider", ...input }),
    })
    const result = await runtime.runTurn({
      turnRef: "turn-restart",
      threadRef: "thread-restart",
      history: [{ role: "user", text: "the original prompt must not replay" }],
      message: "Continue the response interrupted by the Desktop host restart. Do not repeat completed text.",
      recovery: { threadId: "thread-durable-1", accountRef: "codex-2" },
      emit: collect().emit,
    })
    expect(result.ok).toBe(true)
    expect(captured).toHaveLength(1)
    expect(captured[0]!.args.slice(0, 3)).toEqual(["exec", "resume", "thread-durable-1"])
    expect(captured[0]!.env.CODEX_HOME).toBe("/isolated/accounts/codex/codex-2")
    expect(captured[0]!.args.join(" ")).not.toContain("the original prompt must not replay")
    expect(observed).toEqual([
      { kind: "dispatch", threadRef: "thread-restart", turnRef: "turn-restart", accountRef: "codex-2" },
      {
        kind: "provider",
        threadRef: "thread-restart",
        turnRef: "turn-restart",
        accountRef: "codex-2",
        threadId: "thread-durable-1",
      },
    ])
  })

  test("rotation to a DIFFERENT account falls back to bounded-history prepend (a session is pinned to the account that created it)", async () => {
    const captured: SpawnCapture[] = []
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn(
        [
          // turn 1 on codex succeeds (session created there)…
          { stdout: fixtureCodexLocalTurnStdout("t-pin"), exitCode: 0 },
          // …turn 2: codex now revoked → rotate to codex-2 (no session there).
          { stdout: fixtureCodexShortAuthStdout, exitCode: 1 },
          { stdout: fixtureCodexLocalTurnStdout("t-new"), exitCode: 0 },
        ],
        input => captured.push(input),
      ),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const first = await runtime.runTurn({
      turnRef: "turn-1",
      threadRef: "thread-pin",
      history: [],
      message: "start",
      emit: sink.emit,
    })
    if (!first.ok) throw new Error("first turn failed")
    expect(first.accountRef).toBe("codex")
    const second = await runtime.runTurn({
      turnRef: "turn-2",
      threadRef: "thread-pin",
      history: [{ role: "user", text: "start" }, { role: "assistant", text: "ok" }],
      message: "continue",
      emit: sink.emit,
    })
    if (!second.ok) throw new Error(`second turn failed: ${second.reason}`)
    expect(second.accountRef).toBe("codex-2")
    // First attempt of turn 2 was a RESUME on codex (its pinned session)…
    expect(captured[1]!.args.slice(0, 2)).toEqual(["exec", "resume"])
    // …the rotated attempt on codex-2 is a FRESH exec with history prepended
    // (stated fallback: no cross-account session resume).
    const rotated = captured[2]!
    expect(rotated.args[0]).toBe("exec")
    expect(rotated.args[1]).not.toBe("resume")
    const prompt = rotated.args[rotated.args.length - 1]!
    expect(prompt).toContain("Conversation so far")
    expect(prompt).toContain("User: continue")
    // Typed VISIBLE rotation notice.
    expect(sink.events.some(event => event.kind === "lane_notice")).toBe(true)
  })

  test("an exact account target never rotates to another registered Codex account", async () => {
    const captured: SpawnCapture[] = []
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn(
        [{ stdout: fixtureCodexShortAuthStdout, exitCode: 1 }],
        input => captured.push(input),
      ),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-exact",
      threadRef: "thread-exact",
      history: [],
      message: "go",
      accountRef: "codex-2",
      emit: sink.emit,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe("account_reconnect_required")
    expect(captured).toHaveLength(1)
    expect(captured[0]!.env.CODEX_HOME).toBe("/isolated/accounts/codex/codex-2")
  })

  test("INTERRUPT kills the child and yields the typed interrupted failure", async () => {
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: JSON.stringify({ type: "thread.started", thread_id: "t-int" }), exitCode: 0, hang: true },
      ]),
      discoverImpl: async () => [accounts[0]!],
      health: makeCodexAccountHealth(),
      timeoutMs: 5_000,
    })
    const sink = collect()
    const pending = runtime.runTurn({
      turnRef: "turn-int",
      threadRef: "thread-int",
      history: [],
      message: "go",
      emit: sink.emit,
    })
    await new Promise(resolve => setTimeout(resolve, 20))
    expect(runtime.interrupt("turn-int")).toBe(true)
    const result = await pending
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe("interrupted")
    const failed = sink.events.find(event => event.kind === "turn_failed") as
      Extract<FableLocalEvent, { kind: "turn_failed" }>
    expect(failed.reason).toBe("interrupted")
    expect(runtime.interrupt("turn-unknown")).toBe(false)
  })

  test("production turns have no automatic wall-clock deadline and remain stoppable", async () => {
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: JSON.stringify({ type: "thread.started", thread_id: "t-long" }), exitCode: 0, hang: true },
      ]),
      discoverImpl: async () => [accounts[0]!],
      health: makeCodexAccountHealth(),
    })
    const pending = runtime.runTurn({
      turnRef: "turn-long",
      threadRef: "thread-long",
      history: [],
      message: "keep working",
      emit: () => {},
    })
    const raced = await Promise.race([
      pending.then(() => "settled" as const),
      new Promise<"still-running">(resolve => setTimeout(() => resolve("still-running"), 60)),
    ])
    expect(raced).toBe("still-running")
    expect(runtime.interrupt("turn-long")).toBe(true)
    const result = await pending
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe("interrupted")
  })

  test("all accounts auth-failed yields the typed account_reconnect_required turn failure", async () => {
    const sink = collect()
    const evidences: Array<{ accountRef: string; evidence: string }> = []
    const runtimeWithEvidence = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: fixtureCodexShortAuthStdout, exitCode: 1 }]),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
      onAccountEvidence: input => evidences.push(input),
    })
    const result = await runtimeWithEvidence.runTurn({
      turnRef: "turn-all-revoked",
      threadRef: "thread-all-revoked",
      history: [],
      message: "go",
      emit: sink.emit,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.reason).toBe("account_reconnect_required")
      expect(result.detail).toContain("all 2 available Codex session(s) need reconnect")
    }
    // Typed evidence flowed per rotated account (fleet/ledger feed).
    expect(evidences).toEqual([
      { accountRef: "codex", evidence: "reconnect_required" },
      { accountRef: "codex-2", evidence: "reconnect_required" },
    ])
  })

  test("no registered account yields the typed no_codex_account failure", async () => {
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: "", exitCode: 0 }]),
      discoverImpl: async () => [],
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-none",
      threadRef: "thread-none",
      history: [],
      message: "go",
      emit: sink.emit,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) expect(result.reason).toBe("no_codex_account")
  })

  test("post-content failure is terminal session_failed (a partial reply never double-runs)", async () => {
    let spawned = 0
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn(
        [{
          stdout: [
            JSON.stringify({ type: "thread.started", thread_id: "t-post" }),
            JSON.stringify({
              type: "item.completed",
              item: { id: "item_0", type: "agent_message", text: "partial" },
            }),
            JSON.stringify({ type: "error", message: "stream disconnected" }),
          ].join("\n"),
          exitCode: 1,
        }],
        () => {
          spawned += 1
        },
      ),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    const sink = collect()
    const result = await runtime.runTurn({
      turnRef: "turn-post",
      threadRef: "thread-post",
      history: [],
      message: "go",
      emit: sink.emit,
    })
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.reason).toBe("session_failed")
      expect(result.detail).toContain("stream disconnected")
    }
    expect(spawned).toBe(1)
  })
})

describe("makeCodexLocalRuntime.availability (chip-verified-evidence rule)", () => {
  test("the ordinary authenticated session is available without a Pylon preflight receipt", async () => {
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: "", exitCode: 0 }]),
      discoverImpl: async () => [
        { ref: "codex-current", home: "/owner/.codex", source: "current_session" },
        accounts[0]!,
      ],
      health: makeCodexAccountHealth(),
      preflight: verifiedPreflight([]),
    })
    expect(await runtime.availability()).toEqual({
      state: "available",
      accountRef: "codex-current",
      verifiedCount: 1,
    })
  })

  test("without a preflight there is NO verified evidence — unavailable even with registered accounts", async () => {
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: "", exitCode: 0 }]),
      discoverImpl: async () => accounts,
      health: makeCodexAccountHealth(),
    })
    expect(await runtime.availability()).toEqual({
      state: "unavailable",
      reason: "no_verified_account",
    })
  })

  test("verified evidence lights the chip with the health-ordered first verified ref", async () => {
    const health = makeCodexAccountHealth()
    health.recordSuccess("codex-2")
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: "", exitCode: 0 }]),
      discoverImpl: async () => accounts,
      health,
      preflight: verifiedPreflight(["codex-2"]),
    })
    expect(await runtime.availability()).toEqual({
      state: "available",
      accountRef: "codex-2",
      verifiedCount: 1,
    })
  })

  test("no registered accounts at all reports no_codex_account", async () => {
    const runtime = makeCodexLocalRuntime({
      scratchRoot: scratch,
      spawnImpl: makeFixtureCodexChildSpawn([{ stdout: "", exitCode: 0 }]),
      discoverImpl: async () => [],
      health: makeCodexAccountHealth(),
      preflight: verifiedPreflight([]),
    })
    expect(await runtime.availability()).toEqual({
      state: "unavailable",
      reason: "no_codex_account",
    })
  })
})
