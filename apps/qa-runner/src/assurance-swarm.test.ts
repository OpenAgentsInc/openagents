import { describe, expect, test } from "bun:test"
import {
  canonicalArtifact,
  decodeAssuranceReceipt,
  type AssuranceExecutionUnit,
  type AssuranceManifest,
} from "@openagentsinc/assurance-spec/execution"
import { Effect } from "effect"

import {
  OPENAGENTS_DESKTOP_TARGET_PATH,
  OPENAGENTS_DESKTOP_TARGET_REF,
  QA_SWARM_ASSURANCE_LANE_KINDS,
  type QaSwarmAssuranceLaneKind,
  type QaSwarmAssurancePlan,
  type QaSwarmLaneExecution,
  type QaSwarmLaneExecutor,
  runQaSwarmAssuranceManifest,
} from "./assurance-swarm"

const digest = (character: string): `sha256:${string}` => `sha256:${character.repeat(64)}`

const units = QA_SWARM_ASSURANCE_LANE_KINDS.map((kind, index): AssuranceExecutionUnit => ({
  unit_ref: `unit.desktop.${kind}`,
  role: index % 2 === 0 ? "candidate" : "falsifier",
  obligation_id: `AO-DESKTOP-${index + 1}`,
  environment_ref: "ENV-OA-DESKTOP-CURRENT-1",
  adapter_ref: `openagents.${kind}.v1`,
  argv: ["qa", kind],
  artifact_slots: [`var/qa-swarm/${kind}/report.json`],
  expected_observation: index % 2 === 0 ? "CONFIRMED" : "REFUTED",
}))

const manifest: AssuranceManifest = {
  assurance_manifest_format_version: "0.1",
  do_not_edit: true,
  compiler: { version: "0.1.0", content_digest: digest("1") },
  product_spec: { path: "docs/mvp/openagents-desktop.product-spec.md", revision: 1, document_digest: digest("2") },
  assurance_spec: { id: "assurance.openagents.desktop.current", revision: 1, document_digest: digest("3") },
  admission: { ref: "admission.openagents.desktop.current.1", digest: digest("4"), review_set_digest: digest("5") },
  environment: { profile_id: "ENV-OA-DESKTOP-CURRENT-1", revision: 1, digest: digest("6") },
  adapter_lock_digest: digest("7"),
  gate_refs: ["gate.desktop.current"],
  obligation_graph: units.map(unit => ({
    obligation_id: unit.obligation_id,
    criterion_refs: [`CW-AC-${unit.obligation_id.split("-").at(-1)}`],
    dependency_refs: [],
    execution_unit_refs: [unit.unit_ref],
  })),
  execution_units: units,
  evidence_requirements: ["native_report", "normalized_receipt"],
  public_safety: { classification: "review_required", raw_artifacts_public: false },
}
const manifestDigest = canonicalArtifact(manifest).digest

const plan = (override: Partial<QaSwarmAssurancePlan> = {}): QaSwarmAssurancePlan => ({
  target: { ref: OPENAGENTS_DESKTOP_TARGET_REF, repositoryPath: OPENAGENTS_DESKTOP_TARGET_PATH },
  manifest,
  manifestDigest,
  producerRef: "producer.qa_swarm.runner",
  reviewerRef: "reviewer.qa_swarm.independent",
  lanes: QA_SWARM_ASSURANCE_LANE_KINDS.map((kind, index) => ({
    laneRef: `lane.qa_swarm.${kind}`,
    kind,
    adapterRef: units[index]!.adapter_ref,
    executionUnitRefs: [units[index]!.unit_ref],
    budget: { maxActions: 20, maxDurationMs: 60_000, maxModelTokens: kind === "llm_explorer" ? 1_000 : 0 },
    arming: { real: true, spend: kind === "llm_explorer", native: kind === "macos_native" },
    supported: true,
  })),
  ...override,
})

const observed = (
  kind: QaSwarmAssuranceLaneKind,
  unit: AssuranceExecutionUnit,
): QaSwarmLaneExecution => ({
  observations: [{
    executionUnitRef: unit.unit_ref,
    observation: unit.expected_observation,
    infrastructure: "ready",
    nativeReportRef: `var/qa-swarm/${kind}/${unit.unit_ref}.json`,
    nativeReportDigest: digest("9"),
    artifactDigest: digest("a"),
    sourceDigest: digest("b"),
    commandDigest: digest("c"),
  }],
  usage: kind === "llm_explorer"
    ? { kind: "model_observed", exact: true, inputTokens: 200, outputTokens: 50, totalTokens: 250 }
    : { kind: "no_model", exact: true, inputTokens: 0, outputTokens: 0, totalTokens: 0 },
  actionsObserved: 4,
  durationMsObserved: 250,
})

const executors = (): Readonly<Record<QaSwarmAssuranceLaneKind, QaSwarmLaneExecutor>> =>
  Object.fromEntries(QA_SWARM_ASSURANCE_LANE_KINDS.map(kind => [
    kind,
    ((_lane, assigned) => Effect.succeed(observed(kind, assigned[0]!))) satisfies QaSwarmLaneExecutor,
  ])) as unknown as Record<QaSwarmAssuranceLaneKind, QaSwarmLaneExecutor>

describe("QA Swarm Assurance Manifest orchestration", () => {
  test("binds all six independently receipted lanes to exact Desktop Manifest units", async () => {
    const result = await Effect.runPromise(runQaSwarmAssuranceManifest(plan(), executors()))

    expect(result.targetRef).toBe("openagents.desktop.current")
    expect(result.targetRepositoryPath).toBe("apps/openagents-desktop")
    expect(result.laneReceipts.map(receipt => receipt.kind)).toEqual([...QA_SWARM_ASSURANCE_LANE_KINDS])
    expect(result.observation).toBe("REFUTED")
    for (const lane of result.laneReceipts) {
      expect(lane.assuranceReceipts).toHaveLength(1)
      expect(lane.receiptDigests[0]).toMatch(/^sha256:[a-f0-9]{64}$/)
      expect(lane.commitments[0]!.artifactDigest).toBe(digest("a"))
      expect(lane.commitments[0]!.environmentDigest).toBe(digest("6"))
      expect(lane.commitments[0]!.adapterLockDigest).toBe(digest("7"))
      const receipt = lane.assuranceReceipts[0]!
      expect(receipt.manifest_digest).toBe(manifestDigest)
      expect(receipt.product_spec_digest).toBe(digest("2"))
      expect(receipt.assurance_spec_digest).toBe(digest("3"))
      expect(receipt.admission_digest).toBe(digest("4"))
      expect(receipt.environment_ref).toBe("ENV-OA-DESKTOP-CURRENT-1")
      expect(receipt.adapter_ref).toBe(`openagents.${lane.kind}.v1`)
      expect(decodeAssuranceReceipt(receipt)).toEqual(receipt)
    }
    expect(result.laneReceipts.find(lane => lane.kind === "llm_explorer")!.usage).toEqual({
      kind: "model_observed", exact: true, inputTokens: 200, outputTokens: 50, totalTokens: 250,
    })
    expect(result.laneReceipts.filter(lane => lane.kind !== "llm_explorer").every(lane =>
      lane.usage.kind === "no_model" && lane.usage.totalTokens === 0)).toBe(true)
  })

  test("never invokes explicitly unarmed real, spend, or native lanes", async () => {
    let calls = 0
    const unarmed = plan({
      lanes: plan().lanes.map(lane => ({ ...lane, arming: { real: false, spend: false, native: false } })),
    })
    const result = await Effect.runPromise(runQaSwarmAssuranceManifest(unarmed, Object.fromEntries(
      QA_SWARM_ASSURANCE_LANE_KINDS.map(kind => [kind, () => {
        calls += 1
        return Effect.die("must not execute")
      }]),
    )))

    expect(calls).toBe(0)
    expect(result.observation).toBe("INCONCLUSIVE")
    expect(result.laneReceipts.every(lane => lane.observation === "INCONCLUSIVE")).toBe(true)
    expect(result.laneReceipts.every(lane => lane.assuranceReceipts.length === 0 && lane.commitments.length === 0)).toBe(true)
    expect(result.laneReceipts.find(lane => lane.kind === "llm_explorer")!.usage.kind).toBe("model_not_run")
    expect(result.laneReceipts.find(lane => lane.kind === "macos_native")!.blockerRefs).toContain(
      "blocker.qa_swarm.macos_native.native_unarmed",
    )
  })

  test("keeps unsupported adapters and dishonest provider usage INCONCLUSIVE", async () => {
    const changed = plan({
      lanes: plan().lanes.map(lane => lane.kind === "performance" ? { ...lane, supported: false } : lane),
    })
    const adapters = {
      ...executors(),
      llm_explorer: ((_lane, assigned) => Effect.succeed({
        ...observed("llm_explorer", assigned[0]!),
        usage: { kind: "no_model", exact: true, inputTokens: 0, outputTokens: 0, totalTokens: 0 } as const,
      })) satisfies QaSwarmLaneExecutor,
      terminal: ((_lane, assigned) => Effect.succeed({
        ...observed("terminal", assigned[0]!),
        actionsObserved: -1,
        durationMsObserved: 1.5,
      } as unknown as QaSwarmLaneExecution)) satisfies QaSwarmLaneExecutor,
    }
    const result = await Effect.runPromise(runQaSwarmAssuranceManifest(changed, adapters))

    expect(result.observation).toBe("REFUTED")
    expect(result.laneReceipts.find(lane => lane.kind === "performance")!.blockerRefs).toContain(
      "blocker.qa_swarm.performance.unsupported",
    )
    expect(result.laneReceipts.find(lane => lane.kind === "performance")!.assuranceReceipts).toEqual([])
    expect(result.laneReceipts.find(lane => lane.kind === "llm_explorer")!.blockerRefs).toContain(
      "blocker.qa_swarm.llm_explorer.execution_failed",
    )
    expect(result.laneReceipts.find(lane => lane.kind === "llm_explorer")!.assuranceReceipts).toEqual([])
    expect(result.laneReceipts.find(lane => lane.kind === "terminal")!.blockerRefs).toContain(
      "blocker.qa_swarm.terminal.execution_failed",
    )
    expect(result.laneReceipts.find(lane => lane.kind === "terminal")!.assuranceReceipts).toEqual([])
  })

  test("rejects negative or fractional runtime counts at the adapter boundary", async () => {
    const adapters = {
      ...executors(),
      llm_explorer: ((_lane, assigned) => Effect.succeed({
        ...observed("llm_explorer", assigned[0]!),
        usage: {
          kind: "model_observed",
          exact: true,
          inputTokens: -1,
          outputTokens: 1.5,
          totalTokens: 0.5,
        },
      } as unknown as QaSwarmLaneExecution)) satisfies QaSwarmLaneExecutor,
      scripted_browser: ((_lane, assigned) => Effect.succeed({
        ...observed("scripted_browser", assigned[0]!),
        actionsObserved: 1.25,
      } as unknown as QaSwarmLaneExecution)) satisfies QaSwarmLaneExecutor,
    }
    const result = await Effect.runPromise(runQaSwarmAssuranceManifest(plan(), adapters))

    for (const kind of ["llm_explorer", "scripted_browser"] as const) {
      const lane = result.laneReceipts.find(candidate => candidate.kind === kind)!
      expect(lane.observation).toBe("INCONCLUSIVE")
      expect(lane.blockerRefs).toContain(`blocker.qa_swarm.${kind}.execution_failed`)
      expect(lane.assuranceReceipts).toEqual([])
    }
  })

  test("rejects deleted Khala targets and incomplete Manifest assignment", async () => {
    await expect(Effect.runPromise(runQaSwarmAssuranceManifest({
      ...plan(),
      target: { ref: "khala.desktop" as never, repositoryPath: "clients/khala-desktop" as never },
    }, executors()))).rejects.toThrow(/current OpenAgents Desktop/)

    await expect(Effect.runPromise(runQaSwarmAssuranceManifest({
      ...plan(),
      lanes: plan().lanes.map((lane, index) => index === 0 ? { ...lane, executionUnitRefs: [] } : lane),
    }, executors()))).rejects.toThrow(/exact Manifest units/)

    await expect(Effect.runPromise(runQaSwarmAssuranceManifest({
      ...plan(),
      manifestDigest: digest("f"),
    }, executors()))).rejects.toThrow(/does not bind the exact Manifest/)

    await expect(Effect.runPromise(runQaSwarmAssuranceManifest({
      ...plan(),
      lanes: plan().lanes.map((lane, index) => index === 0 ? { ...lane, adapterRef: "openagents.wrong.v1" } : lane),
    }, executors()))).rejects.toThrow(/does not bind the Manifest adapter/)
  })
})
