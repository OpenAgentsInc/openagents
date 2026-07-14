import { Runtime } from "@openagentsinc/runtime-platform"
import { describe, expect, test } from "vite-plus/test"
import { resolve } from "node:path"

import { MCP_PROTOCOL_VERSION, MCP_SERVER_NAME, MCP_TOOLS, handleMcpRequest } from "../src/index.ts"
import { MVP_SPEC, repoRoot } from "./fixture.ts"

const EXPECTED_TOOL_NAMES = [
  "ingest_agent_run",
  "begin_assurance_session",
  "check_assurance_session",
  "list_assurance_specs",
  "get_assurance_spec",
  "validate_assurance_spec",
  "get_subject_binding",
  "get_obligations",
  "get_obligation",
  "get_seams",
  "get_environments",
  "get_gates",
  "get_obligation_graph",
  "get_coverage_ledgers",
  "get_evidence_checklist",
  "check_completion_claim",
  "get_typed_gaps",
  "get_repository_inventory",
] as const

const call = (name: string, args: Record<string, unknown>, root: string = repoRoot): unknown => {
  const response = handleMcpRequest(
    { jsonrpc: "2.0", id: 1, method: "tools/call", params: { name, arguments: args } },
    root,
  )
  if (response === null || response.error !== undefined) {
    throw new Error(`tools/call failed: ${JSON.stringify(response)}`)
  }
  const result = response.result as { content: Array<{ type: string; text: string }> }
  expect(result.content[0]!.type).toBe("text")
  return JSON.parse(result.content[0]!.text)
}

describe("MCP protocol framing", () => {
  test("initialize answers protocol 2024-11-05 with the assurance-spec identity", () => {
    const response = handleMcpRequest({ jsonrpc: "2.0", id: 7, method: "initialize", params: {} }, repoRoot)
    expect(response).toEqual({
      jsonrpc: "2.0",
      id: 7,
      result: {
        protocolVersion: MCP_PROTOCOL_VERSION,
        serverInfo: { name: MCP_SERVER_NAME, version: "0.1.0" },
        capabilities: { tools: {} },
      },
    })
  })

  test("tools/list exposes exactly the §3.1 read-only tool table", () => {
    const response = handleMcpRequest({ jsonrpc: "2.0", id: 2, method: "tools/list" }, repoRoot)
    const tools = (response!.result as { tools: Array<{ name: string; description: string; inputSchema: object }> }).tools
    expect(tools.map((tool) => tool.name)).toEqual([...EXPECTED_TOOL_NAMES])
    expect(Object.keys(MCP_TOOLS)).toEqual([...EXPECTED_TOOL_NAMES])
    for (const tool of tools) {
      expect(tool.description.length).toBeGreaterThan(0)
      expect(tool.inputSchema).toHaveProperty("type", "object")
    }
    const mutating = tools.filter((tool) => /admit|approve|verify|release|propose|write|set_|update|delete/.test(tool.name))
    expect(mutating).toHaveLength(0)
  })

  test("notifications produce no response; unknown methods and tools are JSON-RPC errors", () => {
    expect(handleMcpRequest({ jsonrpc: "2.0", method: "notifications/initialized" }, repoRoot)).toBeNull()
    const unknownMethod = handleMcpRequest({ jsonrpc: "2.0", id: 3, method: "nope/nope" }, repoRoot)
    expect(unknownMethod?.error?.code).toBe(-32601)
    const unknownTool = handleMcpRequest(
      { jsonrpc: "2.0", id: 4, method: "tools/call", params: { name: "admit_assurance_spec", arguments: {} } },
      repoRoot,
    )
    expect(unknownTool?.error?.code).toBe(-32601)
    const missingMethod = handleMcpRequest({ jsonrpc: "2.0", id: 5 }, repoRoot)
    expect(missingMethod?.error?.code).toBe(-32600)
  })

  test("tool-level failures are structured content with stable codes, not protocol errors", () => {
    const missingPath = call("get_coverage_ledgers", {}) as { ok: boolean; code: string }
    expect(missingPath.ok).toBe(false)
    expect(missingPath.code).toBe("invalid_argument")

    const notFound = call("get_coverage_ledgers", { path: "docs/mvp/nope.assurance-spec.md" }) as { ok: boolean; code: string }
    expect(notFound.ok).toBe(false)
    expect(notFound.code).toBe("file_not_found")

    const traversal = call("get_coverage_ledgers", { path: "../nope.assurance-spec.md" }) as { ok: boolean; code: string }
    expect(traversal.ok).toBe(false)
    expect(traversal.code).toBe("invalid_path")
  })

  test("a per-call root can never escape the server root", () => {
    const escape = call("list_assurance_specs", { root: "../../" }) as { ok: boolean; code: string; message: string }
    expect(escape.ok).toBe(false)
    expect(escape.code).toBe("invalid_argument")
    expect(escape.message).toContain("inside the server root")
    const absolute = call("get_coverage_ledgers", { root: "/", path: MVP_SPEC }) as { ok: boolean; code: string }
    expect(absolute.ok).toBe(false)
    expect(absolute.code).toBe("invalid_argument")
  })
})

describe("MCP definition-of-done flow against this repository", () => {
  test("pins a session on the MVP AssuranceSpec with a dual digest", () => {
    const pin = call("begin_assurance_session", { path: MVP_SPEC }) as {
      session_id: string
      assurance_spec: { path: string; revision: number; document_digest: string }
      subject: { path: string; revision: number; document_digest: string; intent_digest?: string }
      criterion_refs: ReadonlyArray<string>
    }
    expect(pin.assurance_spec.path).toBe(MVP_SPEC)
    expect(pin.assurance_spec.document_digest).toMatch(/^sha256:/)
    expect(pin.subject.document_digest).toMatch(/^sha256:/)
    expect(pin.subject.intent_digest).toBeUndefined()
    expect(pin.criterion_refs).toHaveLength(18)

    const check = call("check_assurance_session", { path: MVP_SPEC, pin, session_id: pin.session_id }) as {
      session_id: string
      status: string
      recommended_action: string
    }
    expect(check.session_id).toBe(pin.session_id)
    expect(check.status).toBe("unchanged")
    expect(check.recommended_action).toBe("continue_against_pinned")

    const stale = call("check_assurance_session", {
      path: MVP_SPEC,
      spec_digest: "0".repeat(64),
      subject_digest: pin.subject.document_digest,
    }) as { status: string; recommended_action: string }
    expect(stale.status).toBe("assurance_spec_changed")
    expect(stale.recommended_action).toBe("replan_before_continuing")
  })

  test("lists the 18 obligations and reads AO-CW-AC-04-01's unresolved fields", () => {
    const obligations = call("get_obligations", { path: MVP_SPEC }) as Array<{ id: string }>
    expect(obligations).toHaveLength(18)
    const detail = call("get_obligation", { path: MVP_SPEC, obligation_id: "AO-CW-AC-04-01" }) as {
      obligation: { id: string }
      unresolved_fields: ReadonlyArray<string>
    }
    expect(detail.obligation.id).toBe("AO-CW-AC-04-01")
    expect(detail.unresolved_fields).toContain("oracle")
    expect(detail.unresolved_fields).toContain("falsifier")
  })

  test("returns the three ledgers: 18/18 traceable, 0 executed, frontier not computed", () => {
    const ledgers = call("get_coverage_ledgers", { path: MVP_SPEC }) as {
      criterion_traceability: { total_criteria: number; traceable_criteria: number }
      execution: { executed_obligations: number }
      reachable_frontier: { status: string }
    }
    expect(ledgers.criterion_traceability.traceable_criteria).toBe(18)
    expect(ledgers.criterion_traceability.total_criteria).toBe(18)
    expect(ledgers.execution.executed_obligations).toBe(0)
    expect(ledgers.reachable_frontier.status).toBe("not_computed")
  })

  test("check_completion_claim refuses to round anything up", () => {
    const audit = call("check_completion_claim", { path: MVP_SPEC, claim: "everything is done" }) as {
      claim: string
      claim_evaluated: boolean
      admission_state: string
      obligations: Array<{ axes: Record<string, string> }>
    }
    expect(audit.claim).toBe("everything is done")
    expect(audit.claim_evaluated).toBe(false)
    expect(audit.admission_state).toBe("proposed")
    expect(audit.obligations).toHaveLength(18)
    expect(audit.obligations.every((entry) => entry.axes.observation === "not_run")).toBe(true)
    expect(audit.obligations.every((entry) => entry.axes.admission === "proposed")).toBe(true)
    expect(JSON.stringify(audit)).not.toContain("CONFIRMED")
  })

  test("remaining read tools answer over MCP with typed gaps and honest labels", () => {
    const environments = call("get_environments", { path: MVP_SPEC }) as { gaps: Array<{ code: string }> }
    expect(environments.gaps.length).toBeGreaterThan(0)
    const seams = call("get_seams", { path: MVP_SPEC }) as { count: number }
    expect(seams.count).toBe(0)
    const gates = call("get_gates", { path: MVP_SPEC }) as { count: number }
    expect(gates.count).toBe(0)
    const gaps = call("get_typed_gaps", { path: MVP_SPEC }) as { count: number }
    expect(gaps.count).toBeGreaterThan(0)
    const binding = call("get_subject_binding", { path: MVP_SPEC }) as { subject_status: string }
    expect(binding.subject_status).toBe("bound")
    const validation = call("validate_assurance_spec", { path: MVP_SPEC }) as { valid: boolean }
    expect(validation.valid).toBe(true)
    const document = call("get_assurance_spec", { path: MVP_SPEC }) as { obligations: Array<unknown> }
    expect(document.obligations).toHaveLength(18)
    const checklist = call("get_evidence_checklist", { path: MVP_SPEC, criterion_ref: "CW-AC-04" }) as {
      criteria: Array<{ criterion_ref: string }>
    }
    expect(checklist.criteria[0]!.criterion_ref).toBe("CW-AC-04")
  })

  test("tool responses are deterministic (byte-identical text content)", () => {
    const once = handleMcpRequest(
      { jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "check_completion_claim", arguments: { path: MVP_SPEC } } },
      repoRoot,
    )
    const twice = handleMcpRequest(
      { jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "check_completion_claim", arguments: { path: MVP_SPEC } } },
      repoRoot,
    )
    expect(JSON.stringify(once)).toBe(JSON.stringify(twice))
  })
})

describe("MCP stdio transport", () => {
  test("speaks JSON-RPC over stdin/stdout including -32700 on parse errors", async () => {
    const cli = resolve(import.meta.dirname, "../src/cli.ts")
    const child = Runtime.spawn([process.execPath, cli, "mcp", "--root", repoRoot], {
      stdin: "pipe",
      stdout: "pipe",
      stderr: "ignore",
    })
    child.stdin.write('{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n')
    child.stdin.write("this is not json\n")
    child.stdin.write(`{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_coverage_ledgers","arguments":{"path":"${MVP_SPEC}"}}}\n`)
    await child.stdin.end()
    const stdout = await new Response(child.stdout).text()
    child.kill()
    const lines = stdout.trim().split("\n").map((line) => JSON.parse(line))
    expect(lines).toHaveLength(3)
    expect(lines[0].result.protocolVersion).toBe(MCP_PROTOCOL_VERSION)
    expect(lines[1].error.code).toBe(-32700)
    expect(lines[1].id).toBeNull()
    const ledgers = JSON.parse(lines[2].result.content[0].text)
    expect(ledgers.criterion_traceability.traceable_criteria).toBe(18)
  })
})
