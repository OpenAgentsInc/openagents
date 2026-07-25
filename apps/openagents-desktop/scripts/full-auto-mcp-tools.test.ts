/**
 * Gate 8 (omega#26), restated by owner direction on 2026-07-25:
 *
 *   "No model-initiated path can start Full Auto authority. Only an explicit
 *    human action can, wherever that action lives."
 *
 * This file pins the Desktop half of that property. Its subject is the
 * REGISTERED MCP tool surface -- the exact array the server answers
 * `tools/list` with, and the exact dispatcher `tools/call` routes through --
 * not the text of a source file. That matters: a grep-shaped guard is evaded
 * by renaming `full_auto_start` to anything else, and the whole point of the
 * property is that the safety-relevant variable is which AUTHORITY a model
 * caller can reach, never what the tool is called.
 *
 * The central test therefore drives every registered tool through the real
 * dispatcher against a recording stand-in for the control client, built from
 * the control client's REAL key set, and asserts which control-API operation
 * each tool actually reached. Reintroducing a model-callable start under any
 * name turns it red, because the reached operation is what is asserted.
 */
import { describe, expect, test } from "vite-plus/test"

import { controlOperations } from "./full-auto-control-client.ts"
import {
  dispatchFullAutoMcpTool,
  FULL_AUTO_AUTHORITY_GRANTING_OPERATIONS,
  FULL_AUTO_MCP_TOOLS,
  FULL_AUTO_MODEL_CALLABLE_OPERATIONS,
  FULL_AUTO_REMOVED_MODEL_CALLABLE_START_TOOLS,
  FULL_AUTO_UNEXPOSED_OPERATIONS,
  type FullAutoControlOperations,
  type FullAutoMcpTool,
} from "./full-auto-mcp-tools.ts"

/** The real control-client operation names, read off the real factory. Using
 * the factory (rather than a hand-written list) is what makes the partition
 * assertion below notice a NEW operation nobody classified. */
const controlOperationNames = (): ReadonlyArray<string> =>
  Object.keys(controlOperations({ url: "http://127.0.0.1:1/", token: "unused" })).sort()

/**
 * A stand-in for `controlOperations(...)` that performs no I/O and records the
 * single operation the dispatcher reached. Every real operation name is
 * present, so a tool that routes to an authority-granting operation records
 * that fact instead of throwing "not a function" and looking like a pass.
 */
const recordingOperations = (): {
  operations: FullAutoControlOperations
  reached: () => ReadonlyArray<string>
} => {
  const reached: Array<string> = []
  const stub: Record<string, unknown> = {}
  for (const name of controlOperationNames()) {
    stub[name] = async (): Promise<{ status: number; body: unknown }> => {
      reached.push(name)
      return { status: 200, body: {} }
    }
  }
  return { operations: stub as unknown as FullAutoControlOperations, reached: () => reached }
}

/**
 * Build a maximally permissive argument object from a tool's OWN declared
 * input schema, so no dispatch branch is skipped merely for want of an
 * argument. A tool that only routes to a start when given some extra field
 * still gets that field here.
 */
const argumentsFor = (tool: FullAutoMcpTool): Record<string, unknown> => {
  const args: Record<string, unknown> = {}
  for (const [key, schema] of Object.entries(tool.inputSchema.properties)) {
    const type = schema.type
    args[key] = type === "integer" || type === "number"
      ? 1
      : type === "boolean"
      ? true
      : type === "array"
      ? [{ lane: "codex-local" }]
      : type === "object"
      ? { maxTurns: 1 }
      : key.toLowerCase().includes("workspace")
      ? "/tmp/full-auto-mcp-tools-test"
      : `${key}.probe`
  }
  return args
}

describe("Full Auto MCP tool surface (gate 8: no model-initiated Full Auto start)", () => {
  test("no registered tool reaches a control operation that GRANTS Full Auto authority", async () => {
    const granting = new Set<string>(FULL_AUTO_AUTHORITY_GRANTING_OPERATIONS)
    const modelCallable = new Set<string>(FULL_AUTO_MODEL_CALLABLE_OPERATIONS)
    const observed: Array<{ tool: string; operation: string }> = []

    for (const tool of FULL_AUTO_MCP_TOOLS) {
      const { operations, reached } = recordingOperations()
      const result = await dispatchFullAutoMcpTool(operations, tool.name, argumentsFor(tool))
      // Every registered tool must actually dispatch. A tool that falls
      // through to `null` is unreachable, and an unreachable tool would make
      // this whole enumeration vacuous.
      expect(result, `registered tool ${tool.name} dispatched to nothing`).not.toBeNull()
      const operations_reached = reached()
      expect(operations_reached, `registered tool ${tool.name} reached ${operations_reached.length} operations`)
        .toHaveLength(1)
      const operation = operations_reached[0]!
      observed.push({ tool: tool.name, operation })
      expect(
        granting.has(operation),
        `MCP tool "${tool.name}" reaches control operation "${operation}", which GRANTS Full Auto ` +
          "authority to a language-model caller. Gate 8 (omega#26): no model-initiated path can " +
          "start Full Auto authority. Human starts belong on the Desktop launcher IPC, the Omega " +
          "chat surface, or scripts/full-auto-cli.ts -- never on this surface.",
      ).toBe(false)
      expect(
        modelCallable.has(operation),
        `MCP tool "${tool.name}" reaches unclassified control operation "${operation}". Classify it ` +
          "in full-auto-mcp-tools.ts as authority-granting or model-callable before exposing it.",
      ).toBe(true)
    }

    // A visible record of what the surface can actually do, so a reviewer
    // reads reachable authority rather than tool names.
    expect(observed.length).toBe(FULL_AUTO_MCP_TOOLS.length)
  })

  test("the three classification lists partition the ENTIRE control-client surface", () => {
    const classified = [
      ...FULL_AUTO_AUTHORITY_GRANTING_OPERATIONS,
      ...FULL_AUTO_MODEL_CALLABLE_OPERATIONS,
      ...FULL_AUTO_UNEXPOSED_OPERATIONS,
    ].sort()
    // A new control operation (say a second start route) lands in none of the
    // three lists and fails here BEFORE anyone can quietly expose it as a
    // tool.
    expect(classified).toEqual(controlOperationNames())
    expect(new Set(classified).size).toBe(classified.length)
  })

  test("the removed model-callable start tools are absent by name and refuse when called", async () => {
    const registered = FULL_AUTO_MCP_TOOLS.map(tool => tool.name)
    for (const removed of FULL_AUTO_REMOVED_MODEL_CALLABLE_START_TOOLS) {
      expect(registered).not.toContain(removed)
      const { operations, reached } = recordingOperations()
      // `null` is what the server renders as an isError "unknown tool" result.
      expect(await dispatchFullAutoMcpTool(operations, removed, {
        workspaceRef: "/tmp/full-auto-mcp-tools-test",
        threadRef: "thread.probe",
        title: "probe",
        objective: "probe",
        doneCondition: "probe",
      })).toBeNull()
      expect(reached()).toEqual([])
    }
  })

  test("no registered tool can choose the lane, account, turn cap, or budget an unattended run executes under", () => {
    // Routing policy and guardrails ride only on start/enable. If either
    // reappears in a tool schema, an authority-granting request shape came
    // back with it.
    for (const tool of FULL_AUTO_MCP_TOOLS) {
      const properties = Object.keys(tool.inputSchema.properties)
      expect(properties, tool.name).not.toContain("routingPolicy")
      expect(properties, tool.name).not.toContain("guardrails")
      expect(properties, tool.name).not.toContain("turnCap")
      expect(properties, tool.name).not.toContain("autonomy")
    }
  })

  test("every registered tool name is unique", () => {
    const names = FULL_AUTO_MCP_TOOLS.map(tool => tool.name)
    expect(new Set(names).size).toBe(names.length)
  })
})
