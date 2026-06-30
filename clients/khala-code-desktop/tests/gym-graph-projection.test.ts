import { describe, expect, test } from "bun:test"

import {
  buildKhalaGymGraphProjection,
  isDereferenceableKhalaGymGraphRef,
  KhalaGymGraphProjectionUnsafe,
  type KhalaGymBridgeProofLike,
} from "../src/ui/gym-graph-projection"

const bridgeProof = (
  overrides: Partial<KhalaGymBridgeProofLike> = {},
): KhalaGymBridgeProofLike => ({
  schemaVersion: "openagents.gym.mutalisk_khala_delegation_bridge_output.v0",
  job: {
    runRef: "gym.run.khala_code_delegation_gepa.demo_candidate",
    jobRef: "gym.job.mutalisk_khala_delegation.demo_candidate",
    datasetRef: "eval.mutalisk.fixtures.khala_fleet_delegation_demo.v1",
    trainSplitRefs: ["eval_split.khala_fleet_delegation_demo.train.v1"],
    validationSplitRefs: ["eval_split.khala_fleet_delegation_demo.val.v1"],
    feedbackSchemaRef: "openagents.khala.delegation_gepa_feedback.v0",
    ownerApprovalRef: "approval.owner.khala_delegation.operator_review.v1",
    publicSafetyPolicyRef:
      "policy.public_safe.mutalisk_khala_delegation_summary.v0",
  },
  summary: {
    candidateManifestRef: "manifest.khala_fleet_delegation.demo.v1",
    candidateRef: "candidate.khala_fleet_delegation.demo.v1",
    baseModuleRef: "module.khala_fleet_delegation.base.v1",
    optimizedModuleRef: "module.khala_fleet_delegation.optimized.demo.v1",
    metricValueBps: 187,
    evalEvidenceRefs: ["eval_result.khala_delegation.gd1.demo.v1"],
    traceProvenanceRefs: ["trace_provenance.khala_delegation.closeout.demo.v1"],
    optimizerRunRefs: ["optimizer_run.mutalisk.khala_fleet_delegation.demo.v1"],
    artifactRefs: ["artifact.mutalisk.khala_fleet_delegation.demo.v1"],
    blockerRefs: [],
    publicSafetyChecks: [
      "check.public_projection.prompt_bodies_excluded",
      "check.public_safe.no_optimizer_scratch_logs",
    ],
  },
  progress: [
    {
      runRef: "gym.run.khala_code_delegation_gepa.demo_candidate",
      jobRef: "gym.job.mutalisk_khala_delegation.demo_candidate",
      stage: "queued",
      blockerRefs: [],
      caveatRefs: [
        "caveat.gym.khala_delegation_gepa.no_live_promotion",
      ],
    },
    {
      runRef: "gym.run.khala_code_delegation_gepa.demo_candidate",
      jobRef: "gym.job.mutalisk_khala_delegation.demo_candidate",
      stage: "completed",
      candidateManifestRef: "manifest.khala_fleet_delegation.demo.v1",
      candidateRef: "candidate.khala_fleet_delegation.demo.v1",
      metricValueBps: 187,
      admissionDecision: "gated_proposal_ready",
      actionSubmissionProposalRef:
        "action_submission.proposal.khala_delegation.demo.v1",
      blockerRefs: [],
      caveatRefs: [
        "caveat.gym.khala_delegation_gepa.decision_grade_false_until_live_evidence",
      ],
    },
  ],
  admission: {
    decision: "gated_proposal_ready",
    actionSubmissionProposalRefs: [
      "action_submission.proposal.khala_delegation.demo.v1",
    ],
    blockerRefs: [],
    candidateManifestRef: "manifest.khala_fleet_delegation.demo.v1",
    candidateRef: "candidate.khala_fleet_delegation.demo.v1",
    standingLoop: {
      issueRefs: ["github.issue.openagents.7758"],
      evalResultRefs: ["eval_result.khala_delegation.gd1.demo.v1"],
      optimizerRunRefs: [
        "optimizer_run.mutalisk.khala_fleet_delegation.demo.v1",
      ],
      releaseGateRefs: ["release_gate.khala_fleet_delegation.operator.v1"],
      effectAuthorityGateRefs: [
        "effect_authority_gate.blueprint.khala_delegation.v1",
      ],
      mutaliskLaneRefs: ["lane.mutalisk.gepa_delegation.offline.v1"],
    },
  },
  candidateManifestRef: "manifest.khala_fleet_delegation.demo.v1",
  candidateRef: "candidate.khala_fleet_delegation.demo.v1",
  metricValueBps: 187,
  admissionDecision: "gated_proposal_ready",
  actionSubmissionProposalRef: "action_submission.proposal.khala_delegation.demo.v1",
  blockerRefs: [],
  decisionGrade: false,
  ...overrides,
})

describe("Khala Code Gym graph projection", () => {
  test("maps bridge proof into nodes, pins, links, datum, and refs", () => {
    const projection = buildKhalaGymGraphProjection({
      proof: bridgeProof(),
      generatedAt: "time.test.gym_projection",
    })

    expect(projection.schemaVersion).toBe(
      "openagents.khala_code.gym_graph_projection.v0",
    )
    expect(projection.status).toBe("proposal_ready")
    expect(projection.nodes.map(node => node.id)).toEqual([
      "khala-code-prompt",
      "khala-fleet-delegate",
      "pylon-capacity",
      "codex-assignment",
      "closeout-proof",
      "gd0-examples",
      "gd1-feedback",
      "mutalisk-optimizer",
      "candidate-manifest",
      "gym-ingest",
      "admission",
      "action-submission",
    ])

    const delegate = projection.nodes.find(node => node.id === "khala-fleet-delegate")
    expect(delegate?.inputs[0]).toMatchObject({
      direction: "input",
      id: "request",
      type: "khala.fleet.delegate.request",
    })
    expect(delegate?.outputs[0]).toMatchObject({
      direction: "output",
      id: "capacity",
      type: "pylon.capacity.selection",
    })

    const candidate = projection.nodes.find(node => node.id === "candidate-manifest")
    expect(candidate?.datum).toEqual([
      {
        label: "metric",
        value: 187,
        unit: "bps",
        evidenceRefs: [
          "candidate.khala_fleet_delegation.demo.v1",
          "manifest.khala_fleet_delegation.demo.v1",
          "module.khala_fleet_delegation.base.v1",
          "module.khala_fleet_delegation.optimized.demo.v1",
        ],
      },
    ])

    const proposal = projection.nodes.find(node => node.id === "action-submission")
    expect(proposal?.status).toBe("proposal_ready")
    expect(proposal?.evidenceRefs).toEqual([
      "action_submission.proposal.khala_delegation.demo.v1",
    ])

    const evidenceBackedLinks = projection.links.filter(
      link => link.status === "evidence_backed",
    )
    expect(evidenceBackedLinks.length).toBeGreaterThan(0)
    for (const link of evidenceBackedLinks) {
      expect(link.evidenceRefs.length).toBeGreaterThan(0)
      expect(link.evidenceRefs.every(isDereferenceableKhalaGymGraphRef)).toBe(true)
    }
    expect(projection.blockerRefs).toEqual([])
    expect(projection.caveatRefs).toEqual([
      "caveat.gym.khala_delegation_gepa.decision_grade_false_until_live_evidence",
      "caveat.gym.khala_delegation_gepa.no_live_promotion",
    ])
  })

  test("keeps counter-only refs from lighting links", () => {
    const base = bridgeProof()
    const proof = bridgeProof({
      job: {
        runRef: base.job!.runRef!,
        jobRef: base.job!.jobRef!,
        datasetRef: base.job!.datasetRef!,
        trainSplitRefs: base.job!.trainSplitRefs!,
        validationSplitRefs: base.job!.validationSplitRefs!,
        ownerApprovalRef: base.job!.ownerApprovalRef!,
        publicSafetyPolicyRef: base.job!.publicSafetyPolicyRef!,
      },
      summary: {
        ...base.summary,
        evalEvidenceRefs: ["counter.khala_tokens_served.total=1234"],
      },
      admission: {
        ...base.admission,
        standingLoop: {
          ...base.admission?.standingLoop,
          evalResultRefs: [],
        },
      },
    })
    const projection = buildKhalaGymGraphProjection({ proof })
    const feedbackLink = projection.links.find(link => link.id === "gd1-to-mutalisk")

    expect(feedbackLink?.status).toBe("inactive")
    expect(JSON.stringify(projection)).not.toContain("counter.khala_tokens_served")
  })

  test("ignores unknown raw fields from the bridge proof shape", () => {
    const proof = {
      ...bridgeProof(),
      rawPrompt: "Bearer sk-local private task",
      rawTrace: "/Users/operator/.codex/auth.json",
      providerPayload: "https://private.endpoint.local/provider_payload",
      optimizerScratchLogs: ["scratch_log: /home/operator/mutalisk.log"],
    } as unknown as KhalaGymBridgeProofLike
    const projection = buildKhalaGymGraphProjection({ proof })
    const serialized = JSON.stringify(projection)

    expect(serialized).not.toMatch(
      /Bearer|sk-local|\/Users\/|auth\.json|private\.endpoint|provider_payload|scratch_log:|\/home\//,
    )
  })

  test.each([
    ["raw prompt", { summary: { ...bridgeProof().summary, candidateRef: "raw_prompt.body" } }],
    ["raw trace", { summary: { ...bridgeProof().summary, traceProvenanceRefs: ["raw_trace.full"] } }],
    ["local path", { job: { ...bridgeProof().job, datasetRef: "/Users/operator/private.json" } }],
    ["bearer material", { job: { ...bridgeProof().job, runRef: "bearer abc" } }],
    ["credentials", { summary: { ...bridgeProof().summary, candidateRef: "credential.openai.token" } }],
    ["private endpoint", { job: { ...bridgeProof().job, jobRef: "https://private.example/run" } }],
    ["provider payload", { summary: { ...bridgeProof().summary, artifactRefs: ["provider_payload.openai"] } }],
    ["optimizer scratch logs", { summary: { ...bridgeProof().summary, optimizerRunRefs: ["optimizer_scratch_log.local"] } }],
  ])("rejects known unsafe %s refs", (_label, override) => {
    expect(() =>
      buildKhalaGymGraphProjection({
        proof: bridgeProof(override as Partial<KhalaGymBridgeProofLike>),
      }),
    ).toThrow(KhalaGymGraphProjectionUnsafe)
  })
})
