/**
 * CUT-11 (#8691) main-process wiring tests: the live-agent-graph host fed by
 * the REAL local runtime emit path — an actual `makeFableLocalRuntime` turn
 * (fixture Claude SDK query + REAL `makeCodexChildRuntime` delegate children
 * on fixture spawns) and an actual `makeCodexLocalRuntime` turn (fixture
 * `codex exec --json` stdout) — exactly the callback shape `main.ts` wires.
 * No hand-built envelopes: every observation the graph sees here traveled
 * through the same `emit` seam the renderer stream uses.
 */
import { describe, expect, test } from "bun:test"
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import {
  fixtureCodexSuccessStdout,
  makeCodexAccountHealth,
  makeCodexChildRuntime,
  makeFixtureCodexChildSpawn,
} from "./codex-child-runtime.ts"
import {
  fixtureCodexLocalTurnStdout,
  makeCodexLocalRuntime,
} from "./codex-local-runtime.ts"
import type { FableLocalEvent } from "./fable-local-contract.ts"
import {
  makeFableLocalRuntime,
  makeFixtureFableMcpFactory,
  type FableLocalQuery,
  type FixtureFableMcpTool,
} from "./fable-local-runtime.ts"
import {
  decodeLiveAgentGraphHostSnapshot,
  decodeLiveAgentGraphUpdate,
} from "./live-agent-graph-contract.ts"
import { makeLiveAgentGraphHost } from "./live-agent-graph-host.ts"

/** Isolated Claude account root (mirrors the runtime test harness). */
const makeAccountRoot = (): string => {
  const root = mkdtempSync(join(tmpdir(), "graph-host-accounts-"))
  mkdirSync(join(root, ".claude-pylon-b"), { recursive: true })
  writeFileSync(join(root, ".claude-pylon-b", "claude-oauth-token"), "sk-ant-oat01-pylon-b\n")
  return root
}

type CapturedQuery = Parameters<FableLocalQuery>[0]

const ticker = () => {
  let tick = 0
  return () => new Date(Date.UTC(2026, 6, 12, 0, 0, 0, tick += 1)).toISOString()
}

describe("makeLiveAgentGraphHost through the REAL fable-local emit path", () => {
  test("one fable turn with a real codex delegate child becomes one canonical graph", async () => {
    const pushes: Array<unknown> = []
    const host = makeLiveAgentGraphHost({ emit: update => pushes.push(update), now: ticker() })

    const delegate = makeCodexChildRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "graph-host-child-")),
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => [{ ref: "codex-2", home: "/isolated/codex-2" }],
      health: makeCodexAccountHealth(),
    })
    const captured: CapturedQuery[] = []
    const runtime = makeFableLocalRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "graph-host-scratch-")),
      env: { PYLON_ACCOUNT_HOME_ROOT: makeAccountRoot() },
      queryImpl: async () => (call => {
        captured.push(call)
        return (async function* () {
          yield { type: "system", subtype: "init", session_id: "session-g1" }
          const servers = call.options.mcpServers as Record<string, { tools: Array<FixtureFableMcpTool> }>
          await servers.codex!.tools[0]!.handler({ task: "summarize the notes" }, {})
          yield { type: "result", subtype: "success", is_error: false, result: "done", usage: { input_tokens: 10, output_tokens: 5 } }
        })()
      }),
      delegate,
      mcpImpl: async () => makeFixtureFableMcpFactory(),
    })

    const threadRef = "thread-graph-1"
    const turnRef = "turn-graph-1"
    // EXACTLY the two calls main.ts makes: beginTurn before runTurn, one
    // applyEvent line inside the existing emit callback.
    const begun = host.beginTurn({ turnRef, threadRef, lane: "fable_claude" })
    expect(begun.applied).toBe(true)
    const result = await runtime.runTurn({
      turnRef,
      threadRef,
      history: [],
      message: "go",
      emit: (event: FableLocalEvent) => {
        host.applyEvent(threadRef, { turnRef, event })
      },
    })
    expect(result.ok).toBe(true)

    const snapshot = decodeLiveAgentGraphHostSnapshot(host.snapshot())
    expect(snapshot).not.toBeNull()
    expect(snapshot!.graphs.length).toBe(1)
    const graph = snapshot!.graphs[0]!.graph
    expect(graph.schema).toBe("openagents.live_agent_graph.v1")

    // Root: settled completed with the Claude runtime kind.
    const root = graph.nodes.find(node => node.parent.kind === "root")
    expect(root).toBeDefined()
    expect(root!.status).toBe("completed")
    expect(root!.runtime).toEqual({
      state: "known",
      kind: "claude_agent_sdk",
      runtimeRef: "runtime.claude_agent_sdk.desktop_local",
    })

    // Child: the REAL delegate child (codex-child-runtime fixture spawn),
    // parented to the root, settled completed on the observed account.
    const child = graph.nodes.find(node => node.parent.kind === "agent")
    expect(child).toBeDefined()
    expect(child!.status).toBe("completed")
    expect(child!.provider).toEqual({
      state: "known",
      kind: "codex",
      providerRef: "account.codex.codex-2",
    })
    const parentEdge = graph.edges.find(edge => edge.kind === "parent")
    expect(parentEdge).toBeDefined()
    expect((parentEdge as { toAgentRef: string }).toAgentRef).toBe(child!.agentRef)

    // Push-on-change: every applied observation broadcast a decodable update
    // with monotonically increasing cursors.
    expect(pushes.length).toBeGreaterThanOrEqual(3)
    let lastCursor = 0
    for (const raw of pushes) {
      const update = decodeLiveAgentGraphUpdate(raw)
      expect(update).not.toBeNull()
      expect(update!.threadRef).toBe(threadRef)
      expect(update!.graph.cursor).toBeGreaterThan(lastCursor)
      lastCursor = update!.graph.cursor
    }
  })

  test("a codex-local root turn lands in the same canonical contract on its own thread graph", async () => {
    const pushes: Array<unknown> = []
    const host = makeLiveAgentGraphHost({ emit: update => pushes.push(update), now: ticker() })
    const runtime = makeCodexLocalRuntime({
      scratchRoot: () => mkdtempSync(join(tmpdir(), "graph-host-codex-")),
      env: { PATH: "/usr/bin" },
      spawnImpl: makeFixtureCodexChildSpawn([
        { stdout: fixtureCodexLocalTurnStdout(), exitCode: 0 },
      ]),
      discoverImpl: async () => [{ ref: "codex", home: "/isolated/accounts/codex/codex" }],
      health: makeCodexAccountHealth(),
    })

    const threadRef = "thread-graph-codex"
    const turnRef = "turn-graph-codex"
    expect(host.beginTurn({ turnRef, threadRef, lane: "codex_local" }).applied).toBe(true)
    const result = await runtime.runTurn({
      turnRef,
      threadRef,
      history: [],
      message: "hello codex",
      emit: (event: FableLocalEvent) => {
        host.applyEvent(threadRef, { turnRef, event })
      },
    })
    expect(result.ok).toBe(true)

    const snapshot = decodeLiveAgentGraphHostSnapshot(host.snapshot())
    expect(snapshot).not.toBeNull()
    const graph = snapshot!.graphs.find(entry => entry.threadRef === threadRef)?.graph
    expect(graph).toBeDefined()
    const root = graph!.nodes.find(node => node.parent.kind === "root")
    expect(root).toBeDefined()
    expect(root!.status).toBe("completed")
    // Codex exec transport truth is carried in the runtimeRef; the canonical
    // v1 runtime kind stays codex_app_server (frozen contract).
    expect(root!.runtime).toEqual({
      state: "known",
      kind: "codex_app_server",
      runtimeRef: "runtime.codex_exec.desktop_local",
    })
    // Provider identity only from the terminal observation naming the account.
    expect(root!.provider.state).toBe("known")
  })

  test("events for a thread the host never began are not guessed into a graph", () => {
    const pushes: Array<unknown> = []
    const host = makeLiveAgentGraphHost({ emit: update => pushes.push(update), now: ticker() })
    const result = host.applyEvent("thread-unknown", {
      turnRef: "turn-unknown",
      event: { kind: "turn_started" },
    })
    expect(result).toBeNull()
    expect(pushes.length).toBe(0)
    expect(decodeLiveAgentGraphHostSnapshot(host.snapshot())!.graphs.length).toBe(0)
  })

  test("idle graphs are evicted past the bound; live graphs are never evicted", () => {
    const host = makeLiveAgentGraphHost({ emit: () => {}, now: ticker(), graphLimit: 2 })
    // Two settled turns (begin + fail settles the root immediately).
    for (const index of [1, 2]) {
      const threadRef = `thread-evict-${index}`
      const turnRef = `turn-evict-${index}`
      expect(host.beginTurn({ turnRef, threadRef, lane: "codex_local" }).applied).toBe(true)
      expect(host.applyEvent(threadRef, {
        turnRef,
        event: { kind: "turn_failed", reason: "session_failed", detail: "fixture" },
      })?.applied).toBe(true)
    }
    // A third running turn evicts the oldest IDLE graph, never itself.
    expect(host.beginTurn({ turnRef: "turn-evict-3", threadRef: "thread-evict-3", lane: "codex_local" }).applied).toBe(true)
    const snapshot = decodeLiveAgentGraphHostSnapshot(host.snapshot())!
    const refs = snapshot.graphs.map(entry => entry.threadRef)
    expect(refs.length).toBe(2)
    expect(refs).toContain("thread-evict-3")
    expect(refs).not.toContain("thread-evict-1")
  })
})
