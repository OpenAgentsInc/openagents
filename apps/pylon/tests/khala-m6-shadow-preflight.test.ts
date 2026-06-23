import { createHash } from "node:crypto"
import { describe, expect, test } from "bun:test"
import { preflightKhalaM6ShadowRun } from "../src/khala-m6-shadow-preflight"
import {
  importPsionicTrainingWorkerReceipt,
  projectPsionicTrainingBoundary,
  projectPsionicTrainingSidecarLifecycle,
  verifyPsionicTrainingArtifact,
  verifyPsionicTrainingReleaseManifest,
  type PsionicTrainingArtifactManifest,
  type PsionicTrainingReleaseManifest,
  type PsionicTrainingWorkerReceipt,
} from "../src/psionic-training-boundary"
import type { PylonRealServingReadinessPreflight } from "../src/serving-capability"
import { assertPublicProjectionSafe } from "../src/state"

const observedAt = "2026-06-23T18:31:00.000Z"
const trustedSignerRef = "signer.psionic.release.authority.v1"

describe("Khala M6 shadow-run readiness preflight", () => {
  test("fails closed when live M6 shadow-run authority is not present", () => {
    const preflight = preflightKhalaM6ShadowRun({ observedAt })

    expect(preflight).toMatchObject({
      schema: "openagents.khala.m6.shadow_run_preflight.v0.1",
      canStartShadowRun: false,
      canPublishM6Claim: false,
      contentRedacted: true,
    })
    expect(preflight.blockerRefs).toEqual([
      "blocker.khala.m6.shadow_preflight.baseline_router_missing",
      "blocker.khala.m6.shadow_preflight.live_rollout_missing",
      "blocker.khala.m6.shadow_preflight.owner_approval_ref_missing",
      "blocker.khala.m6.shadow_preflight.owner_confirmation_missing",
      "blocker.khala.m6.shadow_preflight.paid_shadow_win_missing",
      "blocker.khala.m6.shadow_preflight.psionic_training_boundary_not_ready",
      "blocker.khala.m6.shadow_preflight.publication_ref_missing",
      "blocker.khala.m6.shadow_preflight.pylon_serving_preflight_not_ready",
      "blocker.khala.m6.shadow_preflight.shadow_candidate_missing",
      "blocker.khala.m6.shadow_preflight.spend_cap_missing",
      "blocker.khala.m6.shadow_preflight.verdict_source_missing",
      "blocker.khala.m6.shadow_preflight.verdict_source_not_armed",
    ])
    assertPublicProjectionSafe(preflight)
  })

  test("can start a capped shadow run before a paid-shadow win is publishable", () => {
    const preflight = preflightKhalaM6ShadowRun({
      observedAt,
      ownerConfirmed: true,
      ownerApprovalRef: "approval.owner.khala_m6.shadow_run.v1",
      dailySpendCapMsats: 10_000_000,
      plannedShadowSpendMsats: 2_500_000,
      psionicTrainingBoundary: readyPsionicTrainingBoundary(),
      pylonServingPreflight: readyPylonServingPreflight(),
      verdictSourceRef: "verdict.khala.m6.live_source.v1",
      verdictSourceArmed: true,
      shadowCandidateRef: "artifact.psionic.m6.coordinator_candidate.v1",
      baselineRouterRef: "artifact.openagents.khala.heuristic_router.v1",
      liveRolloutRef: "rollout.khala.m6.shadow.live.v1",
    })

    expect(preflight.canStartShadowRun).toBe(true)
    expect(preflight.canPublishM6Claim).toBe(false)
    expect(preflight.blockerRefs).toEqual([
      "blocker.khala.m6.shadow_preflight.paid_shadow_win_missing",
      "blocker.khala.m6.shadow_preflight.publication_ref_missing",
    ])
    expect(preflight.evidenceRefs).toContain("boundary.pylon.psionic_training.ready.v0_3")
    expect(preflight.evidenceRefs).toContain("preflight.pylon.real_serving.ready.v0_1")
    expect(preflight.evidenceRefs).toContain("rollout.khala.m6.shadow.live.v1")
    assertPublicProjectionSafe(preflight)
  })

  test("requires paid shadow-win and publication refs before public M6 claim", () => {
    const preflight = preflightKhalaM6ShadowRun({
      observedAt,
      ownerConfirmed: true,
      ownerApprovalRef: "approval.owner.khala_m6.shadow_run.v1",
      dailySpendCapMsats: 10_000_000,
      plannedShadowSpendMsats: 9_500_000,
      psionicTrainingBoundary: readyPsionicTrainingBoundary(),
      pylonServingPreflight: readyPylonServingPreflight(),
      verdictSourceRef: "verdict.khala.m6.live_source.v1",
      verdictSourceArmed: true,
      shadowCandidateRef: "artifact.psionic.m6.coordinator_candidate.v1",
      baselineRouterRef: "artifact.openagents.khala.heuristic_router.v1",
      liveRolloutRef: "rollout.khala.m6.shadow.live.v1",
      paidShadowWinRef: "receipt.khala.m6.paid_shadow_win.v1",
      publicationRef: "publication.khala.m6.shadow_result.v1",
    })

    expect(preflight.canStartShadowRun).toBe(true)
    expect(preflight.canPublishM6Claim).toBe(true)
    expect(preflight.blockerRefs).toEqual([])
    expect(preflight.evidenceRefs).toContain("receipt.khala.m6.paid_shadow_win.v1")
    expect(preflight.evidenceRefs).toContain("publication.khala.m6.shadow_result.v1")
    assertPublicProjectionSafe(preflight)
  })

  test("keeps unsafe refs and over-budget plans blocked", () => {
    const preflight = preflightKhalaM6ShadowRun({
      observedAt,
      ownerConfirmed: true,
      ownerApprovalRef: "approval.owner.khala_m6.shadow_run.v1",
      dailySpendCapMsats: 10_000,
      plannedShadowSpendMsats: 20_000,
      psionicTrainingBoundary: readyPsionicTrainingBoundary(),
      pylonServingPreflight: readyPylonServingPreflight(),
      verdictSourceRef: "/Users/chris/private/verdict.json",
      verdictSourceArmed: true,
      shadowCandidateRef: "artifact.psionic.m6.coordinator_candidate.v1",
      baselineRouterRef: "artifact.openagents.khala.heuristic_router.v1",
      liveRolloutRef: "rollout.khala.m6.shadow.live.v1",
      paidShadowWinRef: "receipt.khala.m6.paid_shadow_win.v1",
      publicationRef: "publication.khala.m6.shadow_result.v1",
    })

    expect(preflight.canStartShadowRun).toBe(false)
    expect(preflight.canPublishM6Claim).toBe(false)
    expect(preflight.blockerRefs).toContain("blocker.khala.m6.shadow_preflight.spend_cap_exceeded")
    expect(preflight.blockerRefs).toContain("blocker.khala.m6.shadow_preflight.unsafe_ref")
  })
})

function readyPsionicTrainingBoundary() {
  const bytes = new TextEncoder().encode("bounded psionic m6 training artifact")
  const manifest = verifyPsionicTrainingReleaseManifest(releaseManifest(), {
    trustedSignerRefs: [trustedSignerRef],
  })
  const artifact = verifyPsionicTrainingArtifact(artifactManifest(bytes), bytes)
  const sidecar = projectPsionicTrainingSidecarLifecycle([
    { eventRef: "event.psionic.sidecar.started.v1", kind: "started" },
    { eventRef: "event.psionic.sidecar.health_ready.v1", kind: "health_ready" },
  ])
  const receiptImport = importPsionicTrainingWorkerReceipt(workerReceipt(), {
    trustedSignerRefs: [trustedSignerRef],
  })
  return projectPsionicTrainingBoundary({ artifact, manifest, receiptImport, sidecar })
}

function readyPylonServingPreflight(): PylonRealServingReadinessPreflight {
  return {
    schema: "openagents.pylon.serving_preflight.v0.1",
    observedAt,
    canArmRealServing: true,
    paidRoutingEligible: true,
    ownerApprovalRef: "approval.owner.pylon_real_serving.v1",
    admittedPylonRef: "pylon.admitted.khala_m6.worker.v1",
    fabricTransportReady: true,
    gatewayRouteReady: true,
    realGpuAdapterReady: true,
    allowedEngines: ["vllm"],
    residentModelRefs: ["model.psionic.qwen35.0_8b.fp8"],
    evidenceRefs: [
      "capability.pylon.real_gpu.v1",
      "receipt.pylon.serving.self_bench.real.v1",
      "receipt.pylon.serving.canary_replay.v1",
    ],
    blockerRefs: [],
  }
}

function releaseManifest(): PsionicTrainingReleaseManifest {
  return {
    schema: "openagents.psionic.training_release_manifest.v0.3",
    releaseRef: "release.psionic.training.m6.v1",
    trainingJobContractRef: "contract.psionic.training_job.m6.v1",
    workerReceiptFormatRef: "receipt_format.psionic.training_worker.m6.v1",
    sidecarProtocolRef: "protocol.psionic.training_sidecar.m6.v1",
    artifactRefs: ["artifact.psionic.training.runtime.m6.v1"],
    signature: {
      signatureRef: "signature.psionic.release.m6.v1",
      signerRef: trustedSignerRef,
      verificationRef: "verification.psionic.release.m6.v1",
    },
  }
}

function artifactManifest(bytes: Uint8Array): PsionicTrainingArtifactManifest {
  return {
    schema: "openagents.psionic.training_artifact_manifest.v0.3",
    artifactRef: "artifact.psionic.training.runtime.m6.v1",
    digestSha256: sha256(bytes),
    modelRef: "model.psionic.qwen35.training_m6.v1",
    trainingRuntimeRef: "runtime.psionic.training.m6.v1",
  }
}

function workerReceipt(): PsionicTrainingWorkerReceipt {
  return {
    schema: "openagents.psionic.training_worker_receipt.v0.3",
    receiptRef: "receipt.psionic.training_worker.m6.v1",
    assignmentRef: "assignment.public.psionic_training.m6.v1",
    workerRef: "pylon.public.psionic_training.worker.m6.v1",
    runRef: "run.psionic.training.m6.v1",
    artifactRefs: ["artifact.psionic.training.output.m6.v1"],
    checkpointRefs: ["checkpoint.psionic.training.m6.v1"],
    metricRefs: ["metric.psionic.training.m6.loss_curve.v1"],
    proofRefs: ["proof.psionic.training.m6.v1"],
    signature: {
      signatureRef: "signature.psionic.worker_receipt.m6.v1",
      signerRef: trustedSignerRef,
      verificationRef: "verification.psionic.worker_receipt.m6.v1",
    },
  }
}

function sha256(bytes: Uint8Array) {
  return createHash("sha256").update(bytes).digest("hex")
}
