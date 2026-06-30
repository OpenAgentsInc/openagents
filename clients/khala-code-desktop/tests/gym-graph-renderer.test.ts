import { describe, expect, test } from "bun:test"

import { buildKhalaGymGraphProjection } from "../src/ui/gym-graph-projection"
import { renderKhalaGymGraphHtml } from "../src/ui/gym-graph-renderer"
import type { KhalaGymBridgeProofLike } from "../src/ui/gym-graph-projection"

const proof = (extra: Record<string, unknown> = {}): KhalaGymBridgeProofLike =>
  ({
    schemaVersion: "openagents.gym.mutalisk_khala_delegation_bridge_output.v0",
    job: {
      runRef: "gym.run.khala_code_delegation_gepa.render",
      jobRef: "gym.job.mutalisk_khala_delegation.render",
      datasetRef: "eval.mutalisk.fixtures.khala_fleet_delegation_render.v1",
      trainSplitRefs: ["eval_split.khala_fleet_delegation_render.train.v1"],
      validationSplitRefs: ["eval_split.khala_fleet_delegation_render.val.v1"],
      feedbackSchemaRef: "openagents.khala.delegation_gepa_feedback.v0",
      ownerApprovalRef: "approval.owner.khala_delegation.operator_review.v1",
      publicSafetyPolicyRef:
        "policy.public_safe.mutalisk_khala_delegation_summary.v0",
    },
    summary: {
      candidateManifestRef: "manifest.khala_fleet_delegation.render.v1",
      candidateRef: "candidate.khala_fleet_delegation.render.v1",
      baseModuleRef: "module.khala_fleet_delegation.base.v1",
      optimizedModuleRef: "module.khala_fleet_delegation.render.v1",
      metricValueBps: 211,
      evalEvidenceRefs: ["eval_result.khala_delegation.gd1.render.v1"],
      traceProvenanceRefs: ["trace_provenance.khala_delegation.closeout.render.v1"],
      optimizerRunRefs: ["optimizer_run.mutalisk.khala_fleet_delegation.render.v1"],
      artifactRefs: ["artifact.mutalisk.khala_fleet_delegation.render.v1"],
      blockerRefs: [],
      publicSafetyChecks: ["check.public_projection.prompt_bodies_excluded"],
    },
    progress: [
      {
        runRef: "gym.run.khala_code_delegation_gepa.render",
        jobRef: "gym.job.mutalisk_khala_delegation.render",
        stage: "completed",
        candidateManifestRef: "manifest.khala_fleet_delegation.render.v1",
        candidateRef: "candidate.khala_fleet_delegation.render.v1",
        metricValueBps: 211,
        admissionDecision: "gated_proposal_ready",
        actionSubmissionProposalRef:
          "action_submission.proposal.khala_delegation.render.v1",
        blockerRefs: [],
        caveatRefs: ["caveat.gym.khala_delegation_gepa.no_live_promotion"],
      },
    ],
    admission: {
      decision: "gated_proposal_ready",
      actionSubmissionProposalRefs: [
        "action_submission.proposal.khala_delegation.render.v1",
      ],
      blockerRefs: [],
      candidateManifestRef: "manifest.khala_fleet_delegation.render.v1",
      candidateRef: "candidate.khala_fleet_delegation.render.v1",
      standingLoop: {
        issueRefs: ["github.issue.openagents.7759"],
        evalResultRefs: ["eval_result.khala_delegation.gd1.render.v1"],
        optimizerRunRefs: [
          "optimizer_run.mutalisk.khala_fleet_delegation.render.v1",
        ],
        releaseGateRefs: ["release_gate.khala_fleet_delegation.operator.v1"],
        effectAuthorityGateRefs: [
          "effect_authority_gate.blueprint.khala_delegation.v1",
        ],
        mutaliskLaneRefs: ["lane.mutalisk.gepa_delegation.offline.v1"],
      },
    },
    candidateManifestRef: "manifest.khala_fleet_delegation.render.v1",
    candidateRef: "candidate.khala_fleet_delegation.render.v1",
    metricValueBps: 211,
    admissionDecision: "gated_proposal_ready",
    actionSubmissionProposalRef:
      "action_submission.proposal.khala_delegation.render.v1",
    blockerRefs: [],
    decisionGrade: false,
    ...extra,
  }) as KhalaGymBridgeProofLike

const projection = () => buildKhalaGymGraphProjection({ proof: proof() })

describe("Khala Code Gym graph renderer", () => {
  test("renders a stable read-only SVG graph", () => {
    const rendered = renderKhalaGymGraphHtml(projection())

    expect(rendered.html).toContain('class="khala-gym-graph"')
    expect(rendered.svg).toContain('viewBox="0 0 1480 430"')
    expect(rendered.svg).toContain('role="img"')
    expect(rendered.svg).toContain('data-node-id="khala-code-prompt"')
    expect(rendered.svg).toContain('data-node-id="action-submission"')
    expect(rendered.svg).toContain('transform="translate(40 70)"')
    expect(rendered.svg).toContain('transform="translate(1330 280)"')
    expect(rendered.svg).toContain('data-status="evidence_backed"')
    expect(rendered.svg).toContain('metric: 211 bps')
  })

  test("renders an accessible mirror with the same flow and refs", () => {
    const rendered = renderKhalaGymGraphHtml(projection())

    expect(rendered.mirrorHtml).toContain("Gym graph text mirror")
    expect(rendered.mirrorHtml).toContain(
      "Khala Code prompt -&gt; khala.fleet.delegate",
    )
    expect(rendered.mirrorHtml).toContain(
      "admission -&gt; Action Submission",
    )
    expect(rendered.mirrorHtml).toContain(
      "trace_provenance.khala_delegation.closeout.render.v1",
    )
    expect(rendered.mirrorHtml).toContain(
      "action_submission.proposal.khala_delegation.render.v1",
    )
  })

  test("keeps unknown raw bridge fields out of rendered HTML", () => {
    const graph = buildKhalaGymGraphProjection({
      proof: proof({
        rawPrompt: "Bearer sk-local private task body",
        rawTrace: "/Users/operator/.codex/auth.json",
        providerPayload: "https://private.endpoint.local/provider_payload",
      }),
    })
    const rendered = renderKhalaGymGraphHtml(graph)

    expect(rendered.html).not.toMatch(
      /Bearer|sk-local|\/Users\/|auth\.json|private\.endpoint|provider_payload/,
    )
  })

  test("supports reduced motion and state-specific edge styling", async () => {
    const rendered = renderKhalaGymGraphHtml(projection(), {
      reducedMotion: true,
    })
    const css = await Bun.file(new URL("../src/ui/styles.css", import.meta.url)).text()

    expect(rendered.svg).toContain('data-reduced-motion="true"')
    expect(css).toContain('.khala-gym-edge[data-status="inactive"]')
    expect(css).toContain('.khala-gym-edge[data-status="active"]')
    expect(css).toContain('.khala-gym-edge[data-status="blocked"]')
    expect(css).toContain('.khala-gym-edge[data-status="evidence_backed"]')
    expect(css).toContain("@media (prefers-reduced-motion: reduce)")
    expect(css).toContain("animation: khala-gym-edge-flow")
    expect(css).toContain("aspect-ratio: 1480 / 430")
    expect(css).toContain("max-width: 100%")
  })

  test("the Gym pane mounts graph HTML only from loaded projection state", async () => {
    const pane = await Bun.file(new URL("../src/ui/gym-pane.ts", import.meta.url)).text()

    expect(pane).toContain("graph?: KhalaGymGraphProjection")
    expect(pane).toContain("renderKhalaGymGraphHtml")
    expect(pane).toContain('matchMedia("(prefers-reduced-motion: reduce)")')
  })
})
