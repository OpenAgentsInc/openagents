import { afterEach, describe, expect, test } from "vite-plus/test"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { resolve } from "node:path"

import {
  assertVitePlusRuntimeFidelity,
  computeEnvironmentProfileDigest,
  executeVitePlusTestUnit,
  inspectVitePlusJUnit,
  sha256Digest,
  type AssuranceEnvironmentProfileDocument,
  type AssuranceExecutionUnit,
  type AssuranceManifest,
} from "../src/index.ts"

const root = resolve(import.meta.dirname, "../../..")
const vitePlusExecutable = resolve(root, "node_modules/vite-plus/bin/vp")
const roots: string[] = []
afterEach(() => {
  for (const path of roots.splice(0)) rmSync(path, { recursive: true, force: true })
})

const profilePayload = {
  environment_format_version: "0.1" as const,
  profile_id: "ENV-OA-LOCAL-VITE-PLUS-1",
  revision: 1,
  owner: "first_party" as const,
  target_class: "local" as const,
  mutability: "isolated_write" as const,
  platform: { os: "macos", architecture: "arm64", runtime: "Node 24.13.1", framework: "Effect" },
  capabilities: ["vite_plus_test", "junit", "isolated_run_artifacts"],
  authentication_strategy: "none" as const,
  isolation: { fresh_identity: true, reset_between_runs: true, restart_supported: true },
  data_classification: "public_fixture" as const,
  evidence_visibility: "reviewed_public_safe" as const,
  retention: "private native report",
  redaction_policy: "public-safe refs only",
  permitted_actions: ["read_repository", "run_vite_plus_tests", "write_isolated_artifacts"],
  forbidden_actions: ["network", "credentials", "production_mutation", "customer_data"],
  required_commands: ["vp"],
  dependency_lock: { path: "package.json", digest: sha256Digest(readFileSync(resolve(root, "package.json"), "utf8")) },
}
const environment: AssuranceEnvironmentProfileDocument = {
  ...profilePayload,
  profile_digest: computeEnvironmentProfileDigest(profilePayload),
}

const manifest: AssuranceManifest = {
  assurance_manifest_format_version: "0.1",
  do_not_edit: true,
  compiler: { version: "0.1.0", content_digest: sha256Digest("compiler") },
  product_spec: {
    path: "docs/mvp/openagents-codex-workroom-mvp.product-spec.md",
    revision: 6,
    document_digest: sha256Digest(readFileSync(resolve(root, "docs/mvp/openagents-codex-workroom-mvp.product-spec.md"), "utf8")),
  },
  assurance_spec: { id: "assurance.mvp", revision: 1, document_digest: sha256Digest("assurance") },
  admission: { ref: "admission.mvp", digest: sha256Digest("admission"), review_set_digest: sha256Digest("review") },
  environment: { profile_id: environment.profile_id, revision: 1, digest: environment.profile_digest },
  adapter_lock_digest: sha256Digest("lock"),
  gate_refs: ["GATE-ASSURANCESPEC-DOGFOOD"],
  obligation_graph: [{
    obligation_id: "AO-CW-AC-04-01",
    criterion_refs: ["CW-AC-04"],
    dependency_refs: [],
    execution_unit_refs: ["unit.candidate", "unit.falsifier"],
  }],
  execution_units: [],
  evidence_requirements: ["native_report", "normalized_receipt", "oracle_sensitivity_receipt"],
  public_safety: { classification: "review_required", raw_artifacts_public: false },
}

const unit = (role: "candidate" | "falsifier"): AssuranceExecutionUnit => ({
  unit_ref: `unit.${role}`,
  role,
  obligation_id: "AO-CW-AC-04-01",
  environment_ref: environment.profile_id,
  adapter_ref: "openagents.vite_plus_test.v1",
  argv: [
    "vp",
    "test",
    "packages/product-spec/test/product-spec.test.ts",
    "--testNamePattern",
    role === "candidate"
      ? "the MVP spec is executable with unique author-visible criteria"
      : "duplicate criterion IDs refuse executable admission",
  ],
  artifact_slots: [`${role}.junit.xml`],
  expected_observation: role === "candidate" ? "CONFIRMED" : "REFUTED",
})

describe("openagents.vite_plus_test.v1", () => {
  test("fails closed when the observed runtime differs from the admitted profile", () => {
    expect(() => assertVitePlusRuntimeFidelity(environment, {
      os: "macos",
      architecture: "arm64",
      runtime: "Node 99.0.0",
    })).toThrowError(expect.objectContaining({ code: "runtime_fidelity_mismatch" }))
  })

  test("fails closed before execution when the admitted dependency lock drifts", () => {
    const runRoot = mkdtempSync(resolve(tmpdir(), "assurance-lock-drift-"))
    roots.push(runRoot)
    expect(() => executeVitePlusTestUnit({
      workspaceRoot: root,
      runRoot,
      manifest,
      manifestDigest: sha256Digest("manifest"),
      environment: {
        ...environment,
        dependency_lock: { ...environment.dependency_lock, digest: sha256Digest("stale") },
      },
      unit: unit("candidate"),
      producerRef: "runner.local.1",
      reviewerRef: "reviewer.independent.1",
      sourceDigest: sha256Digest("source"),
      vitePlusExecutable,
    })).toThrowError(expect.objectContaining({ code: "dependency_lock_mismatch" }))
  })

  test("runs exactly one named candidate and one falsifier, retaining native JUnit", () => {
    for (const role of ["candidate", "falsifier"] as const) {
      const runRoot = mkdtempSync(resolve(tmpdir(), `assurance-${role}-`))
      roots.push(runRoot)
      const result = executeVitePlusTestUnit({
        workspaceRoot: root,
        runRoot,
        manifest,
        manifestDigest: sha256Digest("manifest"),
        environment,
        unit: unit(role),
        producerRef: "runner.local.1",
        reviewerRef: "reviewer.independent.1",
        sourceDigest: sha256Digest("source"),
        vitePlusExecutable,
      })
      expect(result.exitCode).toBe(0)
      expect(result.selectedTestNames).toHaveLength(1)
      expect(result.selectedTestNames[0]).toEndWith(unit(role).argv[4]!)
      expect(result.receipt.axes.observation).toBe(role === "candidate" ? "CONFIRMED" : "REFUTED")
      expect(result.receipt.axes.infrastructure).toBe("ready")
      expect(readFileSync(result.nativeReportPath, "utf8")).toContain("<testsuite")
      expect(result.receiptBytes).not.toContain("hostname")
      expect(result.receiptBytes).not.toContain("Christophers-")
    }
  })

  test("zero selected tests is infrastructure failure, never green", () => {
    const runRoot = mkdtempSync(resolve(tmpdir(), "assurance-zero-"))
    roots.push(runRoot)
    const missing = { ...unit("candidate"), argv: [...unit("candidate").argv.slice(0, -1), "does not exist"] }
    const result = executeVitePlusTestUnit({
      workspaceRoot: root,
      runRoot,
      manifest,
      manifestDigest: sha256Digest("manifest"),
      environment,
      unit: missing,
      producerRef: "runner.local.1",
      reviewerRef: "reviewer.independent.1",
      sourceDigest: sha256Digest("source"),
      vitePlusExecutable,
    })
    expect(result.selectedTestNames).toEqual([])
    expect(result.receipt.axes.observation).toBe("INCONCLUSIVE")
    expect(result.receipt.axes.infrastructure).not.toBe("ready")
  })

  test("JUnit inspection counts unskipped named cases instead of suite totals", () => {
    const inspection = inspectVitePlusJUnit(
      '<testsuite><testcase name="one"><skipped/></testcase><testcase name="target"></testcase></testsuite>',
    )
    expect(inspection).toEqual({ total: 2, skipped: 1, failed: 0, unskippedNames: ["target"] })
  })
})
