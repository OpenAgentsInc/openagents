import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"
import { Schema } from "effect"

import {
  computeProductSpecDocumentDigest,
} from "@openagentsinc/product-spec"
import {
  handleMcpRequest,
  AgentRunSelfReportEvidenceSchema,
  ingestAgentRun,
  runTool,
  validateAgentRunJson,
  type AgentRunSelfReportEvidence,
} from "../src/index.ts"

const fixtureRoot = resolve(import.meta.dir, "..", "fixtures", "agent-run")
const temporaryRoots: string[] = []

afterEach(() => {
  for (const root of temporaryRoots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const workspace = (mutate?: (run: Record<string, unknown>, markdown: string) => void): { root: string; runPath: string } => {
  const root = mkdtempSync(join(tmpdir(), "assurance-agent-run-"))
  temporaryRoots.push(root)
  const markdown = readFileSync(join(fixtureRoot, "workspace", "subject.product-spec.md"), "utf8")
  const run = JSON.parse(readFileSync(join(fixtureRoot, "workspace", "self-report.agent-run.json"), "utf8")) as Record<string, unknown>
  ;(run.product_spec as Record<string, unknown>).path = "subject.product-spec.md"
  mutate?.(run, markdown)
  writeFileSync(join(root, "subject.product-spec.md"), markdown)
  writeFileSync(join(root, "report.agent-run.json"), `${JSON.stringify(run, null, 2)}\n`)
  return { root, runPath: "report.agent-run.json" }
}

describe("Agent Run 0.1 validation", () => {
  test("accepts the pinned upstream minimal fixture", () => {
    const source = readFileSync(join(fixtureRoot, "upstream", "minimal.agent-run.json"), "utf8")
    expect(validateAgentRunJson(source).valid).toBe(true)
  })

  test("matches upstream invalid status and missing-field diagnostics", () => {
    const invalidStatus = validateAgentRunJson(readFileSync(join(fixtureRoot, "upstream", "invalid-status.agent-run.json"), "utf8"))
    const missingStatus = validateAgentRunJson(readFileSync(join(fixtureRoot, "upstream", "missing-status.agent-run.json"), "utf8"))
    expect(invalidStatus.valid).toBe(false)
    expect(invalidStatus.errors[0]?.code).toBe("invalid_agent_run_status")
    expect(missingStatus.valid).toBe(false)
    expect(missingStatus.errors[0]?.code).toBe("missing_required_agent_run_field")
  })

  test("rejects malformed JSON, duplicate item IDs, and extra fields", () => {
    expect(validateAgentRunJson("{").errors[0]?.code).toBe("invalid_json")
    const valid = JSON.parse(readFileSync(join(fixtureRoot, "upstream", "minimal.agent-run.json"), "utf8")) as Record<string, unknown>
    valid.checked_items = [
      { item_id: "AC-1", status: "passed" },
      { item_id: "AC-1", status: "failed" },
    ]
    expect(validateAgentRunJson(JSON.stringify(valid)).errors[0]?.code).toBe("duplicate_agent_run_item_id")
    valid.checked_items = []
    valid.surprise = true
    expect(validateAgentRunJson(JSON.stringify(valid)).errors[0]?.code).toBe("invalid_agent_run")
  })
})

describe("Agent Run ingest", () => {
  test("projects all item results as producer-equals-claimant self-report without observation authority", () => {
    const fixture = workspace()
    const outcome = runTool(ingestAgentRun({ root: fixture.root, path: fixture.runPath }))
    expect(outcome.ok).toBe(true)
    const evidence = (outcome as { ok: true; value: AgentRunSelfReportEvidence }).value
    expect(evidence.proof_rung).toBe("self_report")
    expect(() => Schema.decodeUnknownSync(AgentRunSelfReportEvidenceSchema)(evidence)).not.toThrow()
    expect(evidence.agent_run_format_version).toBe("0.1")
    expect(evidence.completed_at).toBe("2026-07-13T12:01:00Z")
    expect(evidence.producer).toEqual(evidence.claimant)
    expect(evidence.producer_equals_claimant).toBe(true)
    expect(evidence.independently_verified).toBe(false)
    expect(evidence.observation_axis).toBe("not_promoted")
    expect(evidence.authority).toEqual({
      can_promote_observation: false,
      can_verify: false,
      can_satisfy_independent_producer: false,
    })
    expect(evidence.claimed_items.map((item) => item.status)).toEqual(["passed", "not_checked", "failed", "blocked"])
    expect(Object.hasOwn(evidence, "observation")).toBe(false)
    expect(evidence.spec_pin.digest_status).toBe("missing")
    expect(evidence.gaps.map((gap) => gap.code)).toEqual(["missing_product_spec_content_hash"])
  })

  test("accepts a matching digest and reports no gap", () => {
    const fixture = workspace((run, markdown) => {
      ;(run.product_spec as Record<string, unknown>).content_hash = computeProductSpecDocumentDigest(markdown)
    })
    const outcome = runTool(ingestAgentRun({ root: fixture.root, path: fixture.runPath }))
    expect(outcome.ok).toBe(true)
    const evidence = (outcome as { ok: true; value: AgentRunSelfReportEvidence }).value
    expect(evidence.spec_pin.digest_status).toBe("matched")
    expect(evidence.gaps).toEqual([])
  })

  test("rejects mismatched digest, revision, and unknown cited item", () => {
    const digest = workspace((run) => { ;(run.product_spec as Record<string, unknown>).content_hash = "sha256:not-the-document" })
    expect(runTool(ingestAgentRun({ root: digest.root, path: digest.runPath }))).toMatchObject({ ok: false, code: "product_spec_digest_mismatch" })

    const revision = workspace((run) => { ;(run.product_spec as Record<string, unknown>).spec_revision = 4 })
    expect(runTool(ingestAgentRun({ root: revision.root, path: revision.runPath }))).toMatchObject({ ok: false, code: "product_spec_revision_mismatch" })

    const item = workspace((run) => { ;(run.checked_items as Array<Record<string, unknown>>)[0]!.item_id = "AC-99" })
    expect(runTool(ingestAgentRun({ root: item.root, path: item.runPath }))).toMatchObject({ ok: false, code: "agent_run_item_not_found" })
  })

  test("confines both the Agent Run and its ProductSpec path to root", () => {
    const fixture = workspace((run) => { ;(run.product_spec as Record<string, unknown>).path = "../subject.product-spec.md" })
    expect(runTool(ingestAgentRun({ root: fixture.root, path: fixture.runPath }))).toMatchObject({ ok: false, code: "invalid_path" })
    expect(runTool(ingestAgentRun({ root: fixture.root, path: "../report.agent-run.json" }))).toMatchObject({ ok: false, code: "invalid_path" })
  })

  test("the MCP tool returns the identical read-only projection", () => {
    const fixture = workspace()
    const response = handleMcpRequest({
      jsonrpc: "2.0",
      id: 1,
      method: "tools/call",
      params: { name: "ingest_agent_run", arguments: { path: fixture.runPath } },
    }, fixture.root)
    const text = ((response!.result as { content: Array<{ text: string }> }).content[0]!.text)
    const evidence = JSON.parse(text) as AgentRunSelfReportEvidence
    expect(evidence.proof_rung).toBe("self_report")
    expect(evidence.observation_axis).toBe("not_promoted")
  })

  test("the CLI exposes agent-run ingest with machine-readable output", async () => {
    const fixture = workspace()
    const cli = resolve(import.meta.dir, "../src/cli.ts")
    const child = Bun.spawn([process.execPath, cli, "agent-run", "ingest", fixture.runPath, "--root", fixture.root, "--json"], {
      stdout: "pipe",
      stderr: "pipe",
    })
    const [exitCode, stdout] = await Promise.all([child.exited, new Response(child.stdout).text()])
    expect(exitCode).toBe(0)
    const evidence = JSON.parse(stdout) as AgentRunSelfReportEvidence
    expect(evidence.proof_rung).toBe("self_report")
    expect(evidence.authority.can_verify).toBe(false)
  })
})
