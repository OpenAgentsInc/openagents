/**
 * MH-1 harness conformance suite entrypoint (issue #8582).
 *
 * This is the enum-driven gate. It:
 *   1. Asserts the harness-kind classification and registry stay aligned with
 *      the shared enums (a new coding kind must be classified AND registered).
 *   2. Asserts `grok_cli` is red-by-design (pending, on the known allowlist)
 *      and that codex + claude_code are proven.
 *   3. Runs the full five-capability suite for every proven kind (green) and
 *      emits `test.todo` for every pending kind (visible red, sweep stays green).
 */
import { describe, expect, test } from "bun:test"
import {
  agentDefinitionHarnessKinds,
  agentRuntimeAdapterKinds,
} from "@openagentsinc/agent-runtime-schema"
import {
  codingWorkerAdapterKind,
  codingWorkerFleetKind,
  codingWorkerHarnessKinds,
  harnessKindClassification,
} from "./contract.ts"
import {
  harnessConformanceRegistry,
  knownPendingHarnessKinds,
} from "./registry.ts"
import { runHarnessConformance, todoHarnessConformance } from "./runner.ts"

describe("MH-1 harness conformance coverage gate", () => {
  test("every AgentDefinitionHarnessKind literal is classified", () => {
    for (const kind of agentDefinitionHarnessKinds) {
      expect(
        harnessKindClassification[kind],
        `unclassified harness kind: ${kind}`,
      ).toBeDefined()
    }
  })

  test("the coding-worker harnesses are exactly codex, claude_code, grok_cli", () => {
    expect([...codingWorkerHarnessKinds].sort()).toEqual([
      "claude_code",
      "codex",
      "grok_cli",
    ])
  })

  test("every coding-worker harness maps to a real adapter + fleet kind", () => {
    for (const kind of codingWorkerHarnessKinds) {
      expect(agentRuntimeAdapterKinds).toContain(codingWorkerAdapterKind[kind])
      expect(codingWorkerFleetKind[kind].length).toBeGreaterThan(0)
    }
  })

  test("every coding-worker harness has a registry entry", () => {
    for (const kind of codingWorkerHarnessKinds) {
      expect(
        harnessConformanceRegistry[kind],
        `coding harness missing from registry: ${kind}`,
      ).toBeDefined()
    }
  })

  test("codex and claude_code are proven with real fixtures", () => {
    expect(harnessConformanceRegistry.codex.status).toBe("proven")
    expect(harnessConformanceRegistry.claude_code.status).toBe("proven")
  })

  test("grok_cli is red-by-design (pending, owned by the Grok lane)", () => {
    const entry = harnessConformanceRegistry.grok_cli
    expect(entry.status).toBe("pending")
    if (entry.status === "pending") {
      expect(entry.reasonRef.length).toBeGreaterThan(0)
      expect(entry.ownerLane).toContain("Grok")
    }
  })

  test("no unexpected pending coding harness (a new pending kind reds the sweep)", () => {
    const pending = codingWorkerHarnessKinds.filter(
      (kind) => harnessConformanceRegistry[kind].status === "pending",
    )
    for (const kind of pending) {
      expect(
        knownPendingHarnessKinds,
        `unexpected pending harness kind (add fixtures or allowlist): ${kind}`,
      ).toContain(kind)
    }
  })
})

// Drive the per-kind suites: proven -> green five-capability suite; pending ->
// visible test.todo redness that does not fail the normal sweep.
for (const kind of codingWorkerHarnessKinds) {
  const entry = harnessConformanceRegistry[kind]
  if (entry.status === "proven") {
    runHarnessConformance(entry.fixture)
  } else {
    todoHarnessConformance(kind, entry.reasonRef, entry.ownerLane)
  }
}
