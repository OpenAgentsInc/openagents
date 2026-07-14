import { afterEach, describe, expect, test } from "vite-plus/test"
import { mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { resolve } from "node:path"

import { computeEnvironmentProfileDigest, decodeAssuranceReceipt, decodeMutationPlan, decodeOracleSensitivityReceipt, executeMutationPlan, mutationSetDigestForPlan, OPENAGENTS_MUTATION_MAX_MUTANTS, sha256Digest, type AssuranceEnvironmentProfileDocument, type AssuranceExecutionUnit, type AssuranceManifest } from "../src/index.ts"

const roots: string[] = []
const repositoryRoot = resolve(import.meta.dirname, "../../..")
const vitePlusExecutable = resolve(repositoryRoot, "node_modules/vite-plus/bin/vp")
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const subject = `export const acceptsUnique = (ids: readonly string[]): boolean =>
  new Set(ids).size === ids.length
`

const oracle = `import { expect, test } from "vite-plus/test"
import { acceptsUnique } from "./subject.ts"

test("strong admitted identity oracle", () => {
  expect(acceptsUnique(["CW-AC-01", "CW-AC-02"])).toBe(true)
  expect(acceptsUnique(["CW-AC-01", "CW-AC-01"])).toBe(false)
})

test("deliberately weak identity oracle", () => {
  expect(acceptsUnique(["CW-AC-01", "CW-AC-02"])).toBe(true)
})
`

const environmentPayload = {
  environment_format_version: "0.1" as const,
  profile_id: "ENV-OA-MUTATION-FIXTURE-1",
  revision: 1,
  owner: "first_party" as const,
  target_class: "local" as const,
  mutability: "isolated_write" as const,
  platform: {
    os: "macos",
    architecture: "arm64",
    runtime: "Node 24.13.1",
    framework: "Effect",
  },
  capabilities: ["vite_plus_test", "junit", "isolated_run_artifacts"],
  authentication_strategy: "none" as const,
  isolation: {
    fresh_identity: true,
    reset_between_runs: true,
    restart_supported: true,
  },
  data_classification: "public_fixture" as const,
  evidence_visibility: "reviewed_public_safe" as const,
  retention: "private native report",
  redaction_policy: "public-safe refs only",
  permitted_actions: ["read_repository", "run_vite_plus_tests", "write_isolated_artifacts"],
  forbidden_actions: ["network", "credentials", "production_mutation", "customer_data"],
  required_commands: ["vp"],
  dependency_lock: { path: "pnpm-lock.yaml", digest: sha256Digest("fixture-lock") },
}
const environment: AssuranceEnvironmentProfileDocument = {
  ...environmentPayload,
  profile_digest: computeEnvironmentProfileDigest(environmentPayload),
}

const unit = (testName: string): AssuranceExecutionUnit => ({
  unit_ref: testName.startsWith("strong") ? "unit.admitted.strong" : "unit.admitted.weak",
  role: "candidate",
  obligation_id: "AO-CW-AC-04-01",
  environment_ref: environment.profile_id,
  adapter_ref: "openagents.vite_plus_test.v1",
  argv: ["vp", "test", "oracle.test.ts", "--testNamePattern", testName],
  artifact_slots: ["oracle.junit.xml"],
  expected_observation: "CONFIRMED",
})

const manifest = (oracleUnit: AssuranceExecutionUnit): AssuranceManifest => ({
  assurance_manifest_format_version: "0.1",
  do_not_edit: true,
  compiler: { version: "0.1.0", content_digest: sha256Digest("compiler") },
  product_spec: {
    path: "subject.product-spec.md",
    revision: 1,
    document_digest: sha256Digest("product"),
  },
  assurance_spec: {
    id: "assurance.mutation.fixture",
    revision: 1,
    document_digest: sha256Digest("assurance"),
  },
  admission: {
    ref: "admission.mutation.fixture",
    digest: sha256Digest("admission"),
    review_set_digest: sha256Digest("reviews"),
  },
  environment: {
    profile_id: environment.profile_id,
    revision: 1,
    digest: environment.profile_digest,
  },
  adapter_lock_digest: sha256Digest("adapter-lock"),
  gate_refs: ["GATE-ASSURANCESPEC-DOGFOOD"],
  obligation_graph: [
    {
      obligation_id: "AO-CW-AC-04-01",
      criterion_refs: ["CW-AC-04"],
      dependency_refs: [],
      execution_unit_refs: [oracleUnit.unit_ref],
    },
  ],
  execution_units: [oracleUnit],
  evidence_requirements: ["native_report", "normalized_receipt", "oracle_sensitivity_receipt"],
  public_safety: {
    classification: "review_required",
    raw_artifacts_public: false,
  },
})

const mutations = [
  {
    mutant_ref: "mutant.identity.always-true",
    operator: "replace_exact" as const,
    target: "new Set(ids).size === ids.length",
    replacement: "ids.length === ids.length",
  },
  {
    mutant_ref: "mutant.identity.invert",
    operator: "replace_exact" as const,
    target: "new Set(ids).size === ids.length",
    replacement: "new Set(ids).size !== ids.length",
  },
]

const fixture = (testName: string) => {
  const root = mkdtempSync(resolve(tmpdir(), "oa-mutation-"))
  roots.push(root)
  const workspaceRoot = resolve(root, "workspace")
  const runRoot = resolve(root, "run")
  mkdirSync(workspaceRoot, { recursive: true })
  mkdirSync(resolve(workspaceRoot, "node_modules"), { recursive: true })
  symlinkSync(resolve(repositoryRoot, "node_modules/vite-plus"), resolve(workspaceRoot, "node_modules/vite-plus"), "dir")
  writeFileSync(resolve(workspaceRoot, "package.json"), '{"name":"assurance-mutation-fixture","private":true,"type":"module"}')
  writeFileSync(resolve(workspaceRoot, "subject.ts"), subject)
  writeFileSync(resolve(workspaceRoot, "oracle.test.ts"), oracle)
  const oracleUnit = unit(testName)
  return {
    workspaceRoot,
    runRoot,
    manifest: manifest(oracleUnit),
    oracleUnit,
    plan: {
      mutation_plan_format_version: "0.1",
      adapter_ref: "openagents.mutation.v1",
      obligation_id: "AO-CW-AC-04-01",
      oracle_ref: oracleUnit.unit_ref,
      oracle_unit_ref: oracleUnit.unit_ref,
      subject_relative_path: "subject.ts",
      subject_source_digest: sha256Digest(subject),
      mutations,
    },
  }
}

const execute = (testName: string) => {
  const state = fixture(testName)
  return {
    state,
    result: executeMutationPlan({
      ...state,
      manifestDigest: sha256Digest("admitted-manifest"),
      environment,
      producerRef: "runner.mutation.local.1",
      reviewerRef: "reviewer.independent.1",
      vitePlusExecutable,
    }),
  }
}

describe("openagents.mutation.v1", () => {
  test("the admitted-compatible strong oracle kills the bounded mutant set", () => {
    const { state, result } = execute("strong admitted identity oracle")
    expect(result.sensitivityReceipt).toMatchObject({
      receipt_kind: "oracle_sensitivity_receipt.v1",
      sensitivity_observation: "CONFIRMED",
      killed_mutant_refs: ["mutant.identity.always-true", "mutant.identity.invert"],
      surviving_mutant_refs: [],
      diagnostic_refs: [],
      authority: "evidence_only",
    })
    expect(decodeOracleSensitivityReceipt(JSON.parse(result.sensitivityReceiptBytes))).toEqual(result.sensitivityReceipt)
    expect(result.mutantReceipts.map((receipt) => receipt.axes.observation)).toEqual(["REFUTED", "REFUTED"])
    expect(result.mutantReceipts.every((receipt) => receipt.adapter_ref === "openagents.mutation.v1")).toBe(true)
    for (const receipt of [result.candidateReceipt, ...result.mutantReceipts]) {
      expect(decodeAssuranceReceipt(receipt)).toEqual(receipt)
      expect(receipt.axes.disposition).toBe("pending_review")
    }
    for (const artifact of [result.candidateReceiptArtifact, ...result.mutantReceiptArtifacts]) {
      expect(decodeAssuranceReceipt(JSON.parse(artifact.receiptBytes))).toEqual(artifact.receipt)
      expect(readFileSync(artifact.receiptPath, "utf8")).toBe(artifact.receiptBytes)
      expect(readFileSync(resolve(state.runRoot, artifact.receipt.native_report_ref), "utf8")).toContain("<testsuite")
    }
    expect(readFileSync(resolve(state.workspaceRoot, "subject.ts"), "utf8")).toBe(subject)
  })

  test("a deliberately weak oracle records the surviving mutant as weak-oracle evidence", () => {
    const { result } = execute("deliberately weak identity oracle")
    expect(result.sensitivityReceipt.sensitivity_observation).toBe("REFUTED")
    expect(result.sensitivityReceipt.killed_mutant_refs).toEqual(["mutant.identity.invert"])
    expect(result.sensitivityReceipt.surviving_mutant_refs).toEqual(["mutant.identity.always-true"])
    expect(result.sensitivityReceipt.diagnostic_refs).toEqual(["weak_oracle"])
    expect(result.sensitivityReceipt).not.toHaveProperty("mutation_score")
    expect(result.sensitivityReceiptBytes).not.toContain("new Set(ids)")
  })

  test("planning is exact, closed, deterministic, and bounded before execution", () => {
    const state = fixture("strong admitted identity oracle")
    expect(decodeMutationPlan(state.plan).mutations).toHaveLength(2)
    expect(mutationSetDigestForPlan(state.plan)).toBe(mutationSetDigestForPlan({
      ...state.plan,
      mutations: [...state.plan.mutations].reverse(),
    }))
    expect(() => decodeMutationPlan({ ...state.plan, debug: true })).toThrow()
    expect(() =>
      decodeMutationPlan({
        ...state.plan,
        mutations: Array.from({ length: OPENAGENTS_MUTATION_MAX_MUTANTS + 1 }, (_, index) => ({
          ...mutations[0]!,
          mutant_ref: `mutant.bound.${index}`,
        })),
      }),
    ).toThrow("1-16 deterministic mutants")
    expect(() => decodeMutationPlan({
      ...state.plan,
      mutations: [{ ...mutations[0]!, replacement: "x".repeat(4097) }],
    })).toThrow("bounded to 4096 bytes")
  })

  test("a relative symlink cannot escape the isolated mutation workspace", () => {
    const state = fixture("strong admitted identity oracle")
    const outside = resolve(state.workspaceRoot, "..", "outside.ts")
    writeFileSync(outside, subject)
    symlinkSync(outside, resolve(state.workspaceRoot, "subject-link.ts"))
    expect(() => executeMutationPlan({
      ...state,
      plan: { ...state.plan, subject_relative_path: "subject-link.ts" },
      manifestDigest: sha256Digest("admitted-manifest"),
      environment,
      producerRef: "runner.mutation.local.1",
      reviewerRef: "reviewer.independent.1",
      vitePlusExecutable,
    })).toThrow("remain inside the isolated workspace")
  })
})
