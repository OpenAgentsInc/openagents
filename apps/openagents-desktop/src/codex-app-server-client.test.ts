import { afterEach, describe, expect, test } from "bun:test"
import { EventEmitter } from "node:events"
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { PassThrough } from "node:stream"

import {
  declineCodexServerRequest,
  openCodexAppServerClient,
  registerAssuranceSpecSkill,
  registerProductSpecSkill,
  type CodexAppServerSpawn,
} from "./codex-app-server-client.ts"
import { runCodexAppServerTurn, type CodexAppServerTurnControl } from "./codex-app-server-turn.ts"
import {
  AssuranceSpecWorkSkillSha256,
  ProductSpecWorkSkillSha256,
  installBuiltinAssuranceSpecWorkSkill,
  installBuiltinProductSpecWorkSkill,
} from "./builtin-productspec-skill.ts"
import { ProductSpecDynamicTools } from "./product-spec-app-server-tools.ts"
import type { FableLocalEvent } from "./fable-local-contract.ts"

const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const fakeServer = () => {
  const stdin = new PassThrough()
  const stdout = new PassThrough()
  const stderr = new PassThrough()
  const child = new EventEmitter() as EventEmitter & {
    stdin: PassThrough
    stdout: PassThrough
    stderr: PassThrough
    kill: (signal?: NodeJS.Signals) => boolean
  }
  child.stdin = stdin
  child.stdout = stdout
  child.stderr = stderr
  child.kill = () => { child.emit("close", 0); return true }
  const messages: Array<Record<string, unknown>> = []
  let buffer = ""
  stdin.on("data", chunk => {
    buffer += chunk.toString("utf8")
    while (buffer.includes("\n")) {
      const index = buffer.indexOf("\n")
      const line = buffer.slice(0, index)
      buffer = buffer.slice(index + 1)
      if (line !== "") messages.push(JSON.parse(line))
    }
  })
  return {
    child,
    messages,
    spawn: (() => child) as unknown as CodexAppServerSpawn,
    respond: (id: number, result: unknown) => stdout.write(`${JSON.stringify({ id, result })}\n`),
    notify: (method: string, params: unknown) => stdout.write(`${JSON.stringify({ method, params })}\n`),
  }
}

const waitForMessages = async (messages: ReadonlyArray<unknown>, count: number): Promise<void> => {
  for (let attempt = 0; attempt < 100 && messages.length < count; attempt += 1) await Bun.sleep(1)
  expect(messages.length).toBeGreaterThanOrEqual(count)
}

describe("Codex app-server native integration", () => {
  test("uses initialize/initialized and proves native skill discovery", async () => {
    const fake = fakeServer()
    const client = openCodexAppServerClient({
      binary: "/packaged/codex",
      env: { CODEX_HOME: "/isolated/codex-home" },
      cwd: "/workspace",
      spawnImpl: fake.spawn,
      requestTimeoutMs: 1_000,
    })
    const registration = registerProductSpecSkill({
      client,
      cwd: "/workspace",
      skillRoot: "/isolated/codex-home/skills",
      skillPath: "/isolated/codex-home/skills/productspec-work/SKILL.md",
    })
    await Bun.sleep(0)
    expect(fake.messages[0]).toMatchObject({ method: "initialize", id: 1, params: { capabilities: { experimentalApi: true } } })
    fake.respond(1, { userAgent: "codex-test" })
    await Bun.sleep(0)
    expect(fake.messages[1]).toEqual({ method: "initialized", params: {} })
    expect(fake.messages[2]).toMatchObject({ method: "skills/extraRoots/set", id: 2 })
    fake.respond(2, {})
    await Bun.sleep(0)
    expect(fake.messages[3]).toMatchObject({ method: "skills/config/write", id: 3 })
    fake.respond(3, {})
    await Bun.sleep(0)
    expect(fake.messages[4]).toMatchObject({ method: "skills/list", id: 4 })
    fake.respond(4, { data: [{ cwd: "/workspace", skills: [{
      name: "productspec-work",
      path: "/isolated/codex-home/skills/productspec-work/SKILL.md",
      enabled: true,
    }], errors: [] }] })
    await expect(registration).resolves.toEqual({
      name: "productspec-work",
      path: "/isolated/codex-home/skills/productspec-work/SKILL.md",
      enabled: true,
    })
    client.close()
  })

  test("registers and proves assurancespec-work through the native skill surface", async () => {
    const fake = fakeServer()
    const client = openCodexAppServerClient({
      binary: "/packaged/codex",
      env: { CODEX_HOME: "/isolated/codex-home" },
      cwd: "/workspace",
      spawnImpl: fake.spawn,
      requestTimeoutMs: 1_000,
    })
    const registration = registerAssuranceSpecSkill({
      client,
      cwd: "/workspace",
      skillRoot: "/isolated/codex-home/skills",
      skillPath: "/isolated/codex-home/skills/assurancespec-work/SKILL.md",
    })
    await Bun.sleep(0)
    fake.respond(1, { userAgent: "codex-test" })
    await Bun.sleep(0)
    fake.respond(2, {})
    await Bun.sleep(0)
    fake.respond(3, {})
    await Bun.sleep(0)
    fake.respond(4, { data: [{ cwd: "/workspace", skills: [{
      name: "assurancespec-work",
      path: "/isolated/codex-home/skills/assurancespec-work/SKILL.md",
      enabled: true,
    }], errors: [] }] })
    await expect(registration).resolves.toEqual({
      name: "assurancespec-work",
      path: "/isolated/codex-home/skills/assurancespec-work/SKILL.md",
      enabled: true,
    })
    client.close()
  })

  test("routes notifications and declines unhandled server approval requests", async () => {
    const fake = fakeServer()
    const client = openCodexAppServerClient({
      binary: "/packaged/codex",
      env: {},
      cwd: "/workspace",
      spawnImpl: fake.spawn,
    })
    const seen: unknown[] = []
    client.onNotification(message => seen.push(message))
    fake.notify("turn/completed", { threadId: "thread-1" })
    fake.child.stdout.write(`${JSON.stringify({ id: 91, method: "item/commandExecution/requestApproval", params: {} })}\n`)
    fake.child.stdout.write(`${JSON.stringify({ id: "question-92", method: "item/tool/requestUserInput", params: {} })}\n`)
    await Bun.sleep(0)
    expect(seen).toEqual([{ method: "turn/completed", params: { threadId: "thread-1" } }])
    expect(fake.messages).toContainEqual({ id: 91, result: { decision: "decline" } })
    expect(fake.messages).toContainEqual({ id: "question-92", result: { answers: {} } })
    client.close()
  })

  test("uses method-correct fail-closed server request responses", () => {
    expect(declineCodexServerRequest({ id: 1, method: "item/tool/requestUserInput", params: {} })).toEqual({ answers: {} })
    expect(declineCodexServerRequest({ id: 2, method: "mcpServer/elicitation/request", params: {} })).toEqual({
      action: "decline",
      content: null,
      _meta: null,
    })
    expect(() => declineCodexServerRequest({ id: 3, method: "account/chatgptAuthTokens/refresh", params: {} })).toThrow()
  })

  test("runs a native app-server thread and streams its exact terminal outcome", async () => {
    const fake = fakeServer()
    const events: unknown[] = []
    const control = { interrupted: false, interrupt: null, steer: null }
    const turn = runCodexAppServerTurn({
      binary: "/packaged/codex",
      env: { CODEX_HOME: "/isolated/codex-home" },
      workspace: "/workspace",
      threadRef: "oa-thread-1",
      turnRef: "oa-turn-1",
      accountRef: "codex-work",
      prompt: "Implement criterion CW-AC-04",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.6-sol",
      reasoningEffort: "medium",
      productSpecSkill: {
        skillRoot: "/isolated/codex-home/skills",
        skillPath: "/isolated/codex-home/skills/productspec-work/SKILL.md",
      },
      productSpecDynamicTools: ProductSpecDynamicTools,
      onProductSpecToolCall: async request => ({
        contentItems: [{ type: "inputText", text: JSON.stringify({ ok: true, callId: (request.params as { callId: string }).callId }) }],
        success: true,
      }),
      control,
      emit: event => events.push(event),
      spawnImpl: fake.spawn,
      requestTimeoutMs: 1_000,
    })
    await waitForMessages(fake.messages, 1)
    fake.respond(1, {})
    await waitForMessages(fake.messages, 3)
    fake.respond(2, {})
    await waitForMessages(fake.messages, 4)
    fake.respond(3, {})
    await waitForMessages(fake.messages, 5)
    fake.respond(4, { data: [{ cwd: "/workspace", skills: [{
      name: "productspec-work",
      path: "/isolated/codex-home/skills/productspec-work/SKILL.md",
      enabled: true,
    }] }] })
    await waitForMessages(fake.messages, 6)
    expect(fake.messages[5]).toMatchObject({ method: "thread/start", params: { ephemeral: false, dynamicTools: ProductSpecDynamicTools } })
    fake.respond(5, { thread: { id: "codex-thread-1" }, model: "gpt-5.6-sol" })
    await waitForMessages(fake.messages, 7)
    expect(fake.messages[6]).toMatchObject({
      method: "turn/start",
      params: { threadId: "codex-thread-1", clientUserMessageId: "oa-turn-1" },
    })
    fake.respond(6, { turn: { id: "codex-turn-1", status: "inProgress" } })
    await waitForMessages(fake.messages, 7)
    const steer = (control as CodexAppServerTurnControl).steer?.("Focus on CW-AC-12")
    await waitForMessages(fake.messages, 8)
    expect(fake.messages[7]).toMatchObject({
      method: "turn/steer",
      params: {
        threadId: "codex-thread-1",
        expectedTurnId: "codex-turn-1",
        input: [{ type: "text", text: "Focus on CW-AC-12", text_elements: [] }],
      },
    })
    fake.respond(7, { turnId: "codex-turn-1" })
    await expect(steer).resolves.toBe(true)
    fake.child.stdout.write(`${JSON.stringify({ id: "product-tool-1", method: "item/tool/call", params: { namespace: "product_spec", tool: "get_run", callId: "call-1", arguments: { runRef: "run.1" } } })}\n`)
    await Bun.sleep(0)
    expect(fake.messages).toContainEqual({ id: "product-tool-1", result: {
      contentItems: [{ type: "inputText", text: JSON.stringify({ ok: true, callId: "call-1" }) }],
      success: true,
    } })
    fake.notify("turn/plan/updated", {
      threadId: "codex-thread-1",
      turnId: "codex-turn-1",
      explanation: "Implementation order",
      plan: [
        { step: "Inspect acceptance criterion", status: "completed" },
        { step: "Implement native path", status: "inProgress" },
      ],
    })
    fake.notify("item/started", {
      threadId: "codex-thread-1",
      turnId: "codex-turn-1",
      item: {
        type: "collabAgentToolCall",
        id: "collab-1",
        tool: "spawnAgent",
        status: "inProgress",
        senderThreadId: "codex-thread-1",
        receiverThreadIds: ["child-thread-1"],
        prompt: "Inspect the acceptance tests",
      },
    })
    fake.notify("item/started", {
      threadId: "child-thread-1",
      turnId: "child-turn-1",
      item: {
        type: "collabAgentToolCall",
        id: "collab-nested",
        tool: "spawnAgent",
        status: "inProgress",
        senderThreadId: "child-thread-1",
        receiverThreadIds: ["grandchild-thread-1"],
        prompt: "Check the nested oracle",
      },
    })
    fake.notify("item/agentMessage/delta", {
      threadId: "child-thread-1",
      turnId: "child-turn-1",
      itemId: "child-message",
      delta: "Child evidence.",
    })
    fake.notify("turn/completed", {
      threadId: "child-thread-1",
      turn: { id: "child-turn-1", status: "completed", error: null },
    })
    fake.notify("item/agentMessage/delta", {
      threadId: "grandchild-thread-1",
      turnId: "grandchild-turn-1",
      itemId: "grandchild-message",
      delta: "Nested evidence.",
    })
    fake.notify("turn/completed", {
      threadId: "grandchild-thread-1",
      turn: { id: "grandchild-turn-1", status: "completed", error: null },
    })
    fake.notify("item/agentMessage/delta", {
      threadId: "codex-thread-1",
      turnId: "codex-turn-1",
      itemId: "message-1",
      delta: "Native app-server answer.",
    })
    fake.notify("thread/tokenUsage/updated", {
      threadId: "codex-thread-1",
      turnId: "codex-turn-1",
      tokenUsage: { last: { totalTokens: 44, inputTokens: 30, cachedInputTokens: 5, outputTokens: 10, reasoningOutputTokens: 4 } },
    })
    fake.notify("turn/completed", {
      threadId: "codex-thread-1",
      turn: { id: "codex-turn-1", status: "completed", error: null },
    })
    await expect(turn).resolves.toMatchObject({
      outcome: "success",
      text: "Native app-server answer.",
      threadId: "codex-thread-1",
      usage: { totalTokens: 44 },
    })
    expect(events).toContainEqual({ kind: "text_delta", text: "Native app-server answer." })
    expect(events).toContainEqual({
      kind: "plan_updated",
      entries: [
        { step: "Inspect acceptance criterion", status: "completed" },
        { step: "Implement native path", status: "in_progress" },
      ],
    })
    expect(events).toContainEqual(expect.objectContaining({
      kind: "child_started",
      childRef: "child-thread-1",
      summary: "Inspect the acceptance tests",
    }))
    expect(events).toContainEqual(expect.objectContaining({
      kind: "child_started",
      childRef: "grandchild-thread-1",
      parentChildRef: "child-thread-1",
    }))
    expect(events).toContainEqual(expect.objectContaining({
      kind: "child_completed",
      childRef: "child-thread-1",
      response: "Child evidence.",
    }))
  })

  test("projects current Codex agent states into one causal child lifecycle", async () => {
    const fake = fakeServer()
    const events: FableLocalEvent[] = []
    const turn = runCodexAppServerTurn({
      binary: "/usr/bin/codex",
      env: {},
      workspace: "/workspace",
      threadRef: "thread-1",
      turnRef: "oa-turn-agent-state",
      accountRef: "codex-current",
      prompt: "Delegate one packet",
      imagePaths: [],
      resumeThreadId: null,
      model: "gpt-5.6-sol",
      reasoningEffort: "medium",
      productSpecSkill: {
        skillRoot: "/isolated/codex-home/skills",
        skillPath: "/isolated/codex-home/skills/productspec-work/SKILL.md",
      },
      includeProductSpecSkill: false,
      control: { interrupted: false, interrupt: null, steer: null },
      emit: event => events.push(event),
      spawnImpl: fake.spawn,
    })
    await waitForMessages(fake.messages, 1)
    fake.respond(1, {})
    await waitForMessages(fake.messages, 2)
    fake.respond(2, { thread: { id: "codex-thread-agent-state" } })
    await waitForMessages(fake.messages, 3)
    fake.respond(3, { turn: { id: "codex-turn-agent-state", status: "inProgress" } })
    fake.notify("item/started", {
      threadId: "codex-thread-agent-state",
      turnId: "codex-turn-agent-state",
      item: {
        type: "subAgentActivity",
        id: "subagent-started",
        kind: "started",
        agentThreadId: "child-thread-current",
        agentPath: "packet-worker",
      },
    })
    fake.notify("item/completed", {
      threadId: "codex-thread-agent-state",
      turnId: "codex-turn-agent-state",
      item: {
        type: "collabAgentToolCall",
        id: "collab-wait",
        tool: "wait",
        status: "completed",
        senderThreadId: "codex-thread-agent-state",
        receiverThreadIds: [],
        prompt: "",
        agentsStates: {
          "child-thread-current": { status: "completed", message: "child packet complete" },
        },
      },
    })
    fake.notify("item/agentMessage/delta", {
      threadId: "codex-thread-agent-state",
      turnId: "codex-turn-agent-state",
      itemId: "message-agent-state",
      delta: "Parent complete.",
    })
    fake.notify("turn/completed", {
      threadId: "codex-thread-agent-state",
      turn: { id: "codex-turn-agent-state", status: "completed", error: null },
    })
    await expect(turn).resolves.toMatchObject({ outcome: "success", text: "Parent complete." })
    expect(events.filter(event => event.kind === "child_started")).toEqual([
      expect.objectContaining({ childRef: "child-thread-current", summary: "packet-worker" }),
    ])
    expect(events.filter(event => event.kind === "child_completed")).toEqual([
      expect.objectContaining({ childRef: "child-thread-current", response: "child packet complete" }),
    ])
  })

  test("installs only into a named isolated home and reconciles exact bytes", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-productspec-native-"))
    roots.push(root)
    const resources = path.join(root, "resources")
    const namedHome = path.join(root, "accounts", "codex", "work")
    const defaultHome = path.join(root, ".codex")
    mkdirSync(path.join(resources, "productspec-work"), { recursive: true })
    mkdirSync(path.join(resources, "assurancespec-work"), { recursive: true })
    const source = readFileSync(new URL("../resources/builtin-skills/productspec-work/SKILL.md", import.meta.url))
    const assuranceSource = readFileSync(new URL("../resources/builtin-skills/assurancespec-work/SKILL.md", import.meta.url))
    const manifest = JSON.parse(readFileSync(new URL("../resources/builtin-skills/manifest.json", import.meta.url), "utf8"))
    writeFileSync(path.join(resources, "productspec-work", "SKILL.md"), source)
    writeFileSync(path.join(resources, "assurancespec-work", "SKILL.md"), assuranceSource)
    writeFileSync(path.join(resources, "manifest.json"), JSON.stringify(manifest))
    const first = installBuiltinProductSpecWorkSkill({ builtinSkillsRoot: resources, namedCodexHome: namedHome, defaultCodexHome: defaultHome })
    expect(first.sha256).toBe(ProductSpecWorkSkillSha256)
    expect(first.skillRoot).toBe(path.join(namedHome, "skills"))
    expect(first.reconciled).toBe(false)
    expect(installBuiltinProductSpecWorkSkill({ builtinSkillsRoot: resources, namedCodexHome: namedHome, defaultCodexHome: defaultHome }).reconciled).toBe(true)
    expect(() => installBuiltinProductSpecWorkSkill({ builtinSkillsRoot: resources, namedCodexHome: defaultHome, defaultCodexHome: defaultHome })).toThrow()
    const assurance = installBuiltinAssuranceSpecWorkSkill({ builtinSkillsRoot: resources, namedCodexHome: namedHome, defaultCodexHome: defaultHome })
    expect(assurance.sha256).toBe(AssuranceSpecWorkSkillSha256)
    expect(assurance.reconciled).toBe(false)
    expect(installBuiltinAssuranceSpecWorkSkill({ builtinSkillsRoot: resources, namedCodexHome: namedHome, defaultCodexHome: defaultHome }).reconciled).toBe(true)
    expect(() => installBuiltinAssuranceSpecWorkSkill({ builtinSkillsRoot: resources, namedCodexHome: defaultHome, defaultCodexHome: defaultHome })).toThrow()
  })
})
