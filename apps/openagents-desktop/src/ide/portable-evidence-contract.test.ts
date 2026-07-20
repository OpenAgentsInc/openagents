import { readFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  IDE_PORTABLE_ACCEPTANCE_METRICS,
  IDE_PORTABLE_PHASES,
  IdePortableEvidenceReceiptSchema,
  type IdePortableEvidenceReceipt,
  validateIdePortableEvidenceReceipt,
} from "./portable-evidence-contract.ts"

const candidate = "a".repeat(40)
const base = "b".repeat(40)
const digest = "c".repeat(64)
const decode = Schema.decodeUnknownSync(IdePortableEvidenceReceiptSchema)

const evidenceClassByTarget = {
  owner_local: "real_local",
  owner_managed: "real_owner_managed",
  openagents_managed: "real_openagents_managed",
  managed_provider: "real_managed_provider",
} as const

const completeReceipt = (): IdePortableEvidenceReceipt => decode({
  schemaVersion: "openagents.desktop.ide-portable-evidence.v2",
  issue: "IDE-13",
  candidateCommitSha: candidate,
  baseCommitSha: base,
  generatedAt: "2026-07-20T12:00:00.000Z",
  producerRef: "producer:ide-13",
  environment: { platform: "darwin", architecture: "arm64", node: "v24.13.1" },
  model: {
    maximumDepth: 12,
    exploredStates: 14,
    exploredTransitions: 252,
    staleWriteAttempts: 80,
    counterexamples: 0,
    passed: true,
  },
  implementedChecks: [{
    checkRef: "check:model",
    evidenceClass: "model",
    result: "passed",
    receiptRef: "packages/portable-session-contract/src/ide13-model.test.ts",
  }],
  placementCohorts: Object.entries(evidenceClassByTarget).map(([targetClass, evidenceClass]) => ({
    targetClass,
    evidenceClass,
    journeyScope: "full_move",
    operatingSystem: targetClass === "owner_local" ? "darwin" : "linux",
    architecture: "arm64",
    adapter: {
      kind: "production",
      ref: `adapter:${targetClass}`,
      name: `${targetClass} adapter`,
      version: "1.0.0",
    },
    targetRef: `target:${targetClass}`,
    artifact: { ref: `artifact:${targetClass}`, sha256: digest, bytes: 4_096 },
    candidateCommitSha: candidate,
    baseCommitSha: base,
    capabilityState: "ready",
    custody: targetClass === "owner_local" ? "owner_device" :
      targetClass === "owner_managed" ? "owner_managed" :
        targetClass === "openagents_managed" ? "openagents_managed" : "provider_managed",
    networkDestinations: [],
    dataDestinations: [targetClass],
    retentionSeconds: 3_600,
    costFact: "measured",
    phaseReceipts: IDE_PORTABLE_PHASES.map(phase => ({
      phase,
      evidenceClass,
      receiptRef: `receipt:${targetClass}:${phase}`,
      operationRef: `operation:${targetClass}:${phase}`,
      attachmentGeneration: 2,
      result: "passed",
    })),
    metrics: IDE_PORTABLE_ACCEPTANCE_METRICS.map(metric => ({
      metricRef: `metric:${targetClass}:${metric}`,
      metric,
      phase: metric === "phase_latency" ? "attach" : null,
      unit: metric === "cpu" ? "percent" : metric === "checkpoint_size" || metric === "memory" ? "bytes" :
        metric === "network" ? "bytes_per_second" : metric === "queue" || metric === "lease" || metric === "resource_cleanup" ? "count" : "milliseconds",
      repetitions: 30,
      p50: 1,
      p95: 2,
      p99: 3,
      thresholdP95: 20,
      thresholdP99: 30,
      passed: true,
      receiptRef: `receipt:metric:${targetClass}:${metric}`,
    })),
    result: "The complete real cohort passed.",
  })),
  omissions: [{
    omissionRef: "omission:none",
    targetClass: "owner_local",
    fact: "No optional capability was added to the checkpoint.",
    disposition: "accepted_limit",
    evidenceRef: "receipt:omission:none",
  }],
  recoveryFacts: [{
    recoveryRef: "recovery:older-point",
    targetClass: "owner_local",
    scenario: "The target failed after source revocation.",
    evidenceClass: "real_local",
    outcome: "passed",
    recoveryPointRef: "checkpoint:older",
    receiptRef: "receipt:recovery:older-point",
  }],
  security: {
    forbiddenMaterialProjected: false,
    optimisticAuthorityProjected: false,
    staleGenerationAccepted: false,
    rawCredentialProjected: false,
  },
  review: {
    independentReviewerRef: "reviewer:independent",
    independentDisposition: "accepted",
    independentDispositionRef: "review:independent:accepted",
    ownerRef: "owner:openagents",
    ownerDisposition: "accepted",
    ownerDispositionRef: "review:owner:accepted",
  },
  implementationChecksPassed: true,
  acceptancePassed: true,
  remainingGaps: [],
})

const validate = (receipt: IdePortableEvidenceReceipt): void =>
  validateIdePortableEvidenceReceipt(receipt, {
    candidateCommitSha: candidate,
    baseCommitSha: base,
  })

describe("IDE-13 portability evidence contract", () => {
  test("decodes the current non-acceptance receipt without promoting a simulator", () => {
    const current = decode(JSON.parse(readFileSync(path.resolve(
      import.meta.dirname,
      "../../benchmarks/ide/2026-07-20-ide-13-portability.json",
    ), "utf8")))
    expect(() => validateIdePortableEvidenceReceipt(current, {
      candidateCommitSha: current.candidateCommitSha,
      baseCommitSha: current.baseCommitSha,
    })).not.toThrow()
    expect(current.acceptancePassed).toBe(false)
    expect(current.placementCohorts.every(cohort =>
      cohort.evidenceClass === "simulator" || cohort.evidenceClass === "not_run"
    )).toBe(true)
  })

  test("accepts only a complete real cohort and review matrix", () => {
    expect(() => validate(completeReceipt())).not.toThrow()
  })

  test("rejects simulated evidence classified as real", () => {
    const receipt = completeReceipt()
    const ownerManaged = receipt.placementCohorts[1]
    if (ownerManaged === undefined) throw new Error("missing owner-managed fixture")
    const invalid = {
      ...receipt,
      placementCohorts: receipt.placementCohorts.map((cohort, index) => index === 1
        ? { ...ownerManaged, adapter: { ...ownerManaged.adapter, kind: "deterministic_simulator" } }
        : cohort),
    } as const
    expect(() => decode(invalid)).toThrow(/simulated evidence cannot be classified as real/u)
    expect(() => validate(invalid as unknown as IdePortableEvidenceReceipt))
      .toThrow(/simulated evidence is classified as real/u)
  })

  test("rejects a stale candidate", () => {
    const receipt = completeReceipt()
    expect(() => validateIdePortableEvidenceReceipt(receipt, {
      candidateCommitSha: "d".repeat(40),
      baseCommitSha: base,
    })).toThrow(/candidate is stale/u)
  })

  test("rejects acceptance with a missing phase receipt ref", () => {
    const receipt = completeReceipt()
    const ownerLocal = receipt.placementCohorts[0]
    if (ownerLocal === undefined) throw new Error("missing owner-local fixture")
    expect(() => validate({
      ...receipt,
      placementCohorts: receipt.placementCohorts.map((cohort, index) => index === 0
        ? {
            ...ownerLocal,
            phaseReceipts: ownerLocal.phaseReceipts.map((phase, phaseIndex) =>
              phaseIndex === 0 ? { ...phase, receiptRef: null } : phase),
          }
        : cohort),
    })).toThrow(/lacks exact owner_local phase receipts/u)
  })

  test("rejects acceptance with a missing target artifact ref", () => {
    const receipt = completeReceipt()
    const provider = receipt.placementCohorts[3]
    if (provider === undefined) throw new Error("missing provider fixture")
    expect(() => validate({
      ...receipt,
      placementCohorts: receipt.placementCohorts.map((cohort, index) => index === 3
        ? { ...provider, artifact: { ...provider.artifact, ref: null } }
        : cohort),
    })).toThrow(/lacks exact managed_provider refs/u)
  })

  test("rejects acceptance with a missing resource metric", () => {
    const receipt = completeReceipt()
    const managed = receipt.placementCohorts[2]
    if (managed === undefined) throw new Error("missing OpenAgents-managed fixture")
    expect(() => validate({
      ...receipt,
      placementCohorts: receipt.placementCohorts.map((cohort, index) => index === 2
        ? { ...managed, metrics: managed.metrics.slice(0, -1) }
        : cohort),
    })).toThrow(/acceptance metrics is incomplete/u)
  })

  test("rejects producer self-review", () => {
    const receipt = completeReceipt()
    expect(() => validate({
      ...receipt,
      review: { ...receipt.review, independentReviewerRef: receipt.producerRef },
    })).toThrow(/producer self-review/u)
  })

  test("keeps a non-acceptance receipt valid while real cohorts remain absent", () => {
    const receipt = completeReceipt()
    const absent = receipt.placementCohorts.map(cohort => ({
      ...cohort,
      evidenceClass: "not_run" as const,
      journeyScope: "not_run" as const,
      adapter: { kind: "not_run" as const, ref: null, name: null, version: null },
      targetRef: null,
      artifact: { ref: null, sha256: null, bytes: null },
      phaseReceipts: cohort.phaseReceipts.map(phase => ({
        ...phase,
        evidenceClass: "not_run" as const,
        receiptRef: null,
        operationRef: null,
        attachmentGeneration: null,
        result: "not_run" as const,
      })),
      metrics: [],
    }))
    expect(() => validate({
      ...receipt,
      placementCohorts: absent,
      review: {
        independentReviewerRef: null,
        independentDisposition: "not_run",
        independentDispositionRef: null,
        ownerRef: null,
        ownerDisposition: "not_run",
        ownerDispositionRef: null,
      },
      acceptancePassed: false,
      remainingGaps: ["The real placement cohorts did not run."],
    })).not.toThrow()
  })
})
