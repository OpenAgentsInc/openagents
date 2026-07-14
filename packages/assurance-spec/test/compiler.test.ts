import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"

import {
  assuranceReviewSetDigest,
  AssuranceCompileError,
  canonicalArtifact,
  compileAssuranceManifest,
  computeEnvironmentProfileDigest,
  makeOracleSensitivityReceipt,
  parseAssuranceSpec,
  serializeAssuranceSpec,
  sha256Digest,
  type AssuranceAdapterLock,
  type AssuranceAdmission,
  type AssuranceEnvironmentProfileDocument,
  type AssuranceExecutionUnit,
  type AssuranceReceipt,
  type AssuranceSpecDocument,
} from "../src/index.ts"

const root = resolve(import.meta.dirname, "../../..")
const productSpecBytes = readFileSync(resolve(root, "docs/mvp/openagents-codex-workroom-mvp.product-spec.md"), "utf8")
const proposalBytes = readFileSync(resolve(root, "packages/assurance-spec/conformance/valid/mvp-proposal.assurance-spec.md"), "utf8")

const profilePayload = {
  environment_format_version: "0.1" as const,
  profile_id: "ENV-OA-LOCAL-BUN-1",
  revision: 1,
  owner: "first_party" as const,
  target_class: "local" as const,
  mutability: "isolated_write" as const,
  platform: { os: "macos", architecture: "arm64", runtime: "Bun 1.3.11", framework: "Effect" },
  capabilities: ["bun_test", "junit", "isolated_run_artifacts"],
  authentication_strategy: "none" as const,
  isolation: { fresh_identity: true, reset_between_runs: true, restart_supported: true },
  data_classification: "public_fixture" as const,
  evidence_visibility: "reviewed_public_safe" as const,
  retention: "native reports retained by immutable digest",
  redaction_policy: "no paths, prompts, credentials, or raw private output in public projections",
  permitted_actions: ["read_repository", "run_bun_tests", "write_isolated_artifacts"],
  forbidden_actions: ["network", "credentials", "production_mutation", "customer_data"],
  required_commands: ["bun"],
  dependency_lock: { path: "bun.lock", digest: sha256Digest(readFileSync(resolve(root, "bun.lock"), "utf8")) },
}

const environment: AssuranceEnvironmentProfileDocument = {
  ...profilePayload,
  profile_digest: computeEnvironmentProfileDigest(profilePayload),
}

const adapterLock: AssuranceAdapterLock = {
  adapter_lock_format_version: "0.1",
  adapters: [{
    adapter_ref: "openagents.bun_test.v1",
    version: "1.0.0",
    content_digest: sha256Digest("openagents.bun_test.v1 fixture"),
    techniques: ["example_based"],
    capabilities: ["bun_test", "junit"],
  }],
}

const designedDocument = (): AssuranceSpecDocument => {
  const proposal = parseAssuranceSpec(proposalBytes)
  const obligation = proposal.obligations.find((candidate) => candidate.id === "AO-CW-AC-04-01")!
  return {
    ...proposal,
    frontmatter: { ...proposal.frontmatter, lifecycle_state: "admitted" },
    environments: {
      ...proposal.environments,
      profiles: [{ id: environment.profile_id, status: "admitted" }],
    },
    obligations: proposal.obligations.map((candidate) => candidate.id === obligation.id ? {
      ...candidate,
      domains: ["product_contract"],
      technique: "example_based",
      environment_refs: [environment.profile_id],
      oracle: {
        statement: "The exact executable ProductSpec criterion test passes.",
        evaluator_ref: "packages/product-spec/test/product-spec.test.ts",
      },
      falsifier: {
        kind: "known_bad_fixture",
        ref: "packages/product-spec/test/product-spec.test.ts",
        expected_verdict: "REFUTED",
      },
      evidence: { required_kinds: ["junit", "assurance_receipt"], proof_rung: "local_fixture" },
      independence: { producer_may_verify: false },
      activation_gate: "GATE-ASSURANCESPEC-DOGFOOD",
    } : candidate),
    gates: [{ id: "GATE-ASSURANCESPEC-DOGFOOD", expression: "admission exact and environment ready" }],
    evidencePolicy: {
      links_are_verdicts: false,
      missing_evidence_verdict: "INCONCLUSIVE",
      required_for_ready_obligation: ["oracle", "falsifier", "environment", "independent_review"],
      policy_state: "designed",
    },
    authority: {
      ...proposal.authority,
      admitted_roles: ["openagents.owner"],
      verifier_roles: ["openagents.assurance_reviewer"],
      release_roles: ["openagents.owner"],
      policy_state: "designed",
    },
  }
}

const units: ReadonlyArray<AssuranceExecutionUnit> = [
  {
    unit_ref: "unit.ao-cw-ac-04-01.candidate",
    role: "candidate",
    obligation_id: "AO-CW-AC-04-01",
    environment_ref: environment.profile_id,
    adapter_ref: "openagents.bun_test.v1",
    argv: ["bun", "test", "packages/product-spec/test/product-spec.test.ts", "--test-name-pattern", "MVP executable"],
    artifact_slots: ["candidate.junit.xml"],
    expected_observation: "CONFIRMED",
  },
  {
    unit_ref: "unit.ao-cw-ac-04-01.falsifier",
    role: "falsifier",
    obligation_id: "AO-CW-AC-04-01",
    environment_ref: environment.profile_id,
    adapter_ref: "openagents.bun_test.v1",
    argv: ["bun", "test", "packages/product-spec/test/product-spec.test.ts", "--test-name-pattern", "duplicate criterion IDs"],
    artifact_slots: ["falsifier.junit.xml"],
    expected_observation: "REFUTED",
  },
]

const fixture = () => {
  const assuranceSpec = designedDocument()
  const assuranceSpecBytes = serializeAssuranceSpec(assuranceSpec)
  const reviewSetDigest = assuranceReviewSetDigest([{ path: "review.json", bytes: "reviewed" }])
  const admission: AssuranceAdmission = {
    admission_format_version: "0.1",
    admission_ref: "admission.mvp.full.1",
    decision: "admitted",
    assurance_spec: {
      id: assuranceSpec.frontmatter.assurance_spec_id,
      revision: assuranceSpec.frontmatter.assurance_revision,
      document_digest: sha256Digest(assuranceSpecBytes),
    },
    product_spec: {
      path: assuranceSpec.subject.product_spec.path,
      revision: assuranceSpec.subject.product_spec.spec_revision,
      document_digest: sha256Digest(productSpecBytes),
    },
    review_set_digest: reviewSetDigest,
    recognized_actor_ref: "owner.openagents",
    recognized_role: "openagents.owner",
    allowed_gate_refs: ["GATE-ASSURANCESPEC-DOGFOOD"],
    authority_statement: "Owner directed the full MVP assurance run; this admission grants execution only.",
  }
  const admissionBytes = canonicalArtifact(admission).bytes
  const adapterLockBytes = canonicalArtifact(adapterLock).bytes
  return {
    assuranceSpec,
    assuranceSpecBytes,
    productSpecBytes,
    admission,
    admissionBytes,
    environment,
    adapterLock,
    adapterLockBytes,
    compilerContentDigest: sha256Digest("compiler fixture"),
    executionUnits: units,
  }
}

describe("AS-L2 deterministic compiler", () => {
  test("identical admitted inputs produce byte-identical golden-ready manifests", () => {
    const first = compileAssuranceManifest(fixture())
    const second = compileAssuranceManifest(fixture())
    expect(first.bytes).toBe(second.bytes)
    expect(first.digest).toBe(second.digest)
    expect(first.manifest.do_not_edit).toBe(true)
    expect(first.manifest.execution_units.map((unit) => unit.role)).toEqual(["candidate", "falsifier"])
    expect(first.bytes).not.toContain(root)
    expect(first.bytes).toMatchSnapshot()
  })

  test("refuses stale admission, profile, unlocked adapter, and undesigned obligation", () => {
    const stale = fixture()
    expect(() => compileAssuranceManifest({
      ...stale,
      admission: { ...stale.admission, assurance_spec: { ...stale.admission.assurance_spec, revision: 9 } },
    })).toThrow("AssuranceSpec revision")

    const badProfile = fixture()
    expect(() => compileAssuranceManifest({
      ...badProfile,
      environment: { ...badProfile.environment, capabilities: ["changed"] },
    })).toThrow("profile_digest")

    const unlocked = fixture()
    expect(() => compileAssuranceManifest({
      ...unlocked,
      executionUnits: unlocked.executionUnits.map((unit) => ({ ...unit, adapter_ref: "missing.adapter.v1" })),
    })).toThrow("unlocked adapter")

    const undesigned = fixture()
    expect(() => compileAssuranceManifest({
      ...undesigned,
      assuranceSpec: {
        ...undesigned.assuranceSpec,
        obligations: undesigned.assuranceSpec.obligations.map((obligation) => {
          if (obligation.id !== "AO-CW-AC-04-01") return obligation
          const { oracle: _oracle, ...withoutOracle } = obligation
          return withoutOracle
        }),
      },
    })).toThrow("undesigned obligation")
  })

  test("changed declared inputs change the manifest digest without clock or randomness", () => {
    const first = compileAssuranceManifest(fixture())
    const changed = fixture()
    const second = compileAssuranceManifest({
      ...changed,
      compilerContentDigest: sha256Digest("different compiler bytes"),
    })
    expect(second.digest).not.toBe(first.digest)
  })

  test("fails closed on self-verifying and label-only seam obligations", () => {
    const compileCode = (run: () => unknown): string => {
      try {
        run()
      } catch (error) {
        expect(error).toBeInstanceOf(AssuranceCompileError)
        return (error as AssuranceCompileError).code
      }
      throw new Error("expected compilation to fail")
    }
    const selfVerifying = fixture()
    expect(compileCode(() => compileAssuranceManifest({
      ...selfVerifying,
      assuranceSpec: {
        ...selfVerifying.assuranceSpec,
        obligations: selfVerifying.assuranceSpec.obligations.map((obligation) =>
          obligation.id === "AO-CW-AC-04-01"
            ? { ...obligation, independence: { producer_may_verify: true } }
            : obligation),
      },
    }))).toBe("false_green_api_mirror")

    const labelOnlySeam = fixture()
    expect(compileCode(() => compileAssuranceManifest({
      ...labelOnlySeam,
      assuranceSpec: {
        ...labelOnlySeam.assuranceSpec,
        obligations: labelOnlySeam.assuranceSpec.obligations.map((obligation) =>
          obligation.id === "AO-CW-AC-04-01"
            ? { ...obligation, domains: [...(obligation.domains ?? []), "seam"] }
            : obligation),
      },
    }))).toBe("false_green_mocked_seam")
  })
})

const receipt = (role: "candidate" | "falsifier"): AssuranceReceipt => ({
  assurance_receipt_format_version: "0.1",
  receipt_ref: `receipt.${role}`,
  manifest_digest: sha256Digest("manifest"),
  product_spec_digest: sha256Digest(productSpecBytes),
  assurance_spec_digest: sha256Digest("assurance"),
  admission_digest: sha256Digest("admission"),
  obligation_id: "AO-CW-AC-04-01",
  criterion_refs: ["CW-AC-04"],
  environment_ref: "ENV-OA-LOCAL-BUN-1",
  adapter_ref: "openagents.bun_test.v1",
  execution_unit_ref: `unit.${role}`,
  producer_ref: "runner.local.1",
  reviewer_ref: "reviewer.independent.1",
  native_report_ref: `var/assurance/${role}.junit.xml`,
  native_report_digest: sha256Digest(`<testsuite name="${role}"/>`),
  command_digest: sha256Digest(role),
  source_digest: sha256Digest("source"),
  axes: {
    admission: "admitted",
    readiness: "executable",
    observation: role === "candidate" ? "CONFIRMED" : "REFUTED",
    infrastructure: "ready",
    stability: "stable",
    freshness: "current",
    disposition: "pending_review",
    exception: "none",
  },
  public_safety: { classification: "private", contains_raw_output: false },
})

describe("eight-axis receipts and sensitivity", () => {
  test("candidate confirmation plus falsifier rejection emits sensitivity confirmation", () => {
    const sensitivity = makeOracleSensitivityReceipt(receipt("candidate"), receipt("falsifier"), {
      oracleRef: "product-spec executable test",
      falsifierRef: "duplicate-id rejection",
    })
    expect(sensitivity.sensitivity_observation).toBe("CONFIRMED")
    expect(sensitivity.diagnostic_refs).toEqual([])
  })

  test("surviving mutants remain typed weak-oracle evidence", () => {
    const sensitivity = makeOracleSensitivityReceipt(receipt("candidate"), receipt("falsifier"), {
      oracleRef: "weak oracle",
      falsifierRef: "duplicate-id mutation",
      survivingMutantRefs: ["mutant.weak.1"],
    })
    expect(sensitivity.surviving_mutant_refs).toEqual(["mutant.weak.1"])
    expect(sensitivity.diagnostic_refs).toEqual(["weak_oracle"])
  })
})
