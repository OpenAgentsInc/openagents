/**
 * AFS-12 mobile reuse proof.
 *
 * The mobile surface decodes the SAME frozen AFS-00 safe fixtures Desktop decodes,
 * to the SAME canonical facts, and it never receives Desktop execution authority.
 * These assertions are the mobile half of the cross-surface decode-equivalence
 * proof: they run in the mobile project, so a mobile-only regression fails here.
 */
import { readFileSync } from "node:fs"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import { afsBaselineSurfaceFactSummary } from "@openagentsinc/agent-surface/afs-baseline-surface-corpus"

import {
  mobileAgentSurfaceFactsAreSecretFree,
  readMobileAgentGraph,
  readMobileAgentSurfaceFactSummary,
  readMobileMessageDrilldown,
} from "../src/agent-surface/mobile-agent-surface-read-model"

const moduleSource = readFileSync(
  path.join(import.meta.dirname, "..", "src", "agent-surface", "mobile-agent-surface-read-model.ts"),
  "utf8",
)

/** Extract every import/export module specifier from the module source. */
const importSpecifiers = (source: string): ReadonlyArray<string> => {
  const specifiers: string[] = []
  const pattern = /(?:import|export)[^;]*?from\s*["']([^"']+)["']|import\(\s*["']([^"']+)["']\s*\)/gu
  for (const match of source.matchAll(pattern)) {
    const specifier = match[1] ?? match[2]
    if (specifier) specifiers.push(specifier)
  }
  return specifiers
}

describe("AFS-12 mobile reuse: decode equivalence", () => {
  test("mobile decodes the frozen corpus to the canonical cross-surface facts", () => {
    expect(readMobileAgentSurfaceFactSummary()).toEqual(afsBaselineSurfaceFactSummary)
  })

  test("mobile composes a bounded read-only agent graph with a safe message drilldown", () => {
    const graph = readMobileAgentGraph()
    expect(graph.map((node) => node.scenario)).toEqual([
      "local_answer",
      "standby",
      "explicit_provider",
      "malformed_output",
      "helper_failure",
      "unavailable_provider",
    ])
    // The standby node is the only live (queued) node.
    expect(graph.filter((node) => node.live).map((node) => node.scenario)).toEqual(["standby"])
    // The local-answer drilldown shows the bounded safe two-entry chain; no raw data.
    const drilldown = readMobileMessageDrilldown("request.local.1")
    expect(drilldown.map((entry) => entry.role)).toEqual(["user", "assistant"])
    expect(drilldown.every((entry) => entry.text.length <= 8192)).toBe(true)
    // A refused/failed node still renders without agent execution.
    expect(graph.find((node) => node.scenario === "helper_failure")?.refusalReason).toBe("helper_missing")
  })
})

describe("AFS-12 mobile reuse: privacy fence and no execution authority", () => {
  test("every decoded mobile fact is secret-free", () => {
    expect(mobileAgentSurfaceFactsAreSecretFree()).toBe(true)
  })

  test("the mobile read model imports only the portable shared-surface subpaths", () => {
    const allowed = new Set([
      "node:fs",
      "node:path",
      "@openagentsinc/agent-surface",
      "@openagentsinc/agent-surface/afs-baseline-surface-corpus",
      "@openagentsinc/agent-runtime-schema",
    ])
    for (const specifier of importSpecifiers(moduleSource)) {
      expect(allowed.has(specifier)).toBe(true)
    }
    // Never the Desktop host, the Apple FM Node adapter, a Node store, or a provider SDK.
    for (const banned of [
      "openagents-desktop",
      "apple-fm-runtime/node",
      "agent-turn-store",
      "pylon",
      "provider-lane",
    ]) {
      expect(moduleSource.includes(banned)).toBe(false)
    }
  })

  test("the mobile read model exposes no agent-execution or provider-dispatch surface", () => {
    for (const bannedToken of [
      "dispatchTurn",
      "runTurn",
      "makeProviderLaneDispatcher",
      "child_process",
      "execFile",
      "spawn(",
    ]) {
      expect(moduleSource.includes(bannedToken)).toBe(false)
    }
  })
})
