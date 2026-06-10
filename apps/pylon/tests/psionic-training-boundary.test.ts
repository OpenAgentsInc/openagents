import { createHash } from "node:crypto"
import { describe, expect, test } from "bun:test"
import { projectLaunchGateMatrix } from "../src/launch-gates"
import {
  blockedPsionicTrainingReceiptImport,
  importPsionicTrainingWorkerReceipt,
  projectPsionicTrainingBoundary,
  projectPsionicTrainingSidecarLifecycle,
  verifyPsionicTrainingArtifact,
  verifyPsionicTrainingReleaseManifest,
  type PsionicTrainingArtifactManifest,
  type PsionicTrainingReleaseManifest,
  type PsionicTrainingWorkerReceipt,
} from "../src/psionic-training-boundary"
import { assertPublicProjectionSafe } from "../src/state"

const trustedSignerRef = "signer.psionic.release.authority.v1"

describe("Pylon Psionic training boundary", () => {
  test("blocks unsigned or untrusted Psionic training release manifests", () => {
    const unsigned = verifyPsionicTrainingReleaseManifest({
      ...releaseManifest(),
      signature: {
        signatureRef: "receipt.psionic.not_a_signature",
        signerRef: trustedSignerRef,
        verificationRef: "verification.psionic.release.v1",
      },
    }, { trustedSignerRefs: [trustedSignerRef] })

    expect(unsigned.state).toBe("blocked")
    expect(unsigned.blockerRefs).toContain("blocker.psionic_training.release_manifest_unsigned")

    const untrusted = verifyPsionicTrainingReleaseManifest(releaseManifest(), {
      trustedSignerRefs: ["signer.psionic.other_authority.v1"],
    })

    expect(untrusted.state).toBe("blocked")
    expect(untrusted.blockerRefs).toContain("blocker.psionic_training.release_manifest_untrusted_signer")
    assertPublicProjectionSafe(untrusted)
  })

  test("verifies release manifest refs and artifact SHA-256 before training support", () => {
    const bytes = new TextEncoder().encode("bounded psionic training artifact")
    const artifact = verifyPsionicTrainingArtifact(artifactManifest(bytes), bytes)
    const manifest = verifyPsionicTrainingReleaseManifest(releaseManifest(), {
      trustedSignerRefs: [trustedSignerRef],
    })

    expect(manifest).toMatchObject({
      state: "ready",
      trainingJobContractRef: "contract.psionic.training_job.v1",
      workerReceiptFormatRef: "receipt_format.psionic.training_worker.v1",
      blockerRefs: [],
    })
    expect(artifact).toMatchObject({
      state: "ready",
      digestRef: `artifact.digest.sha256.${sha256(bytes)}`,
      blockerRefs: [],
    })
    assertPublicProjectionSafe(manifest)
    assertPublicProjectionSafe(artifact)
  })

  test("blocks artifact digest mismatch before placement or launch claims", () => {
    const artifact = verifyPsionicTrainingArtifact(
      {
        ...artifactManifest(new TextEncoder().encode("expected artifact")),
        digestSha256: "0".repeat(64),
      },
      new TextEncoder().encode("actual artifact"),
    )

    expect(artifact.state).toBe("blocked")
    expect(artifact.blockerRefs).toContain("blocker.psionic_training.artifact_digest_mismatch")
  })

  test("projects sidecar lifecycle without raw process paths or private state", () => {
    const healthy = projectPsionicTrainingSidecarLifecycle([
      { eventRef: "event.psionic.sidecar.start_requested.v1", kind: "start_requested" },
      { eventRef: "event.psionic.sidecar.started.v1", kind: "started" },
      { eventRef: "event.psionic.sidecar.health_ready.v1", kind: "health_ready" },
    ])
    const crashed = projectPsionicTrainingSidecarLifecycle([
      { eventRef: "event.psionic.sidecar.started.v1", kind: "started" },
      { eventRef: "event.psionic.sidecar.crashed.v1", kind: "crashed" },
    ])

    expect(healthy.state).toBe("healthy")
    expect(healthy.blockerRefs).toEqual([])
    expect(crashed.state).toBe("crashed")
    expect(crashed.blockerRefs).toContain("blocker.psionic_training.sidecar_crashed")
    assertPublicProjectionSafe(healthy)
  })

  test("imports signed worker receipts as public-safe closeout evidence", () => {
    const imported = importPsionicTrainingWorkerReceipt(workerReceipt(), {
      trustedSignerRefs: [trustedSignerRef],
    })

    expect(imported).toMatchObject({
      state: "ready",
      assignmentRef: "assignment.public.psionic_training.boundary.v1",
      receiptRef: "receipt.psionic.training_worker.boundary.v1",
      workerRef: "pylon.public.psionic_training.worker.v1",
      blockerRefs: [],
    })
    expect(imported.closeoutEvidenceRefs).toContain("checkpoint.psionic.training.boundary.v1")
    expect(imported.closeoutEvidenceRefs).toContain("proof.psionic.training.boundary.v1")
    assertPublicProjectionSafe(imported)
  })

  test("supportsTraining flips only when manifest, artifact, sidecar, and receipt import are ready", () => {
    const bytes = new TextEncoder().encode("bounded psionic training artifact")
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
    const ready = projectPsionicTrainingBoundary({ artifact, manifest, receiptImport, sidecar })
    const blocked = projectPsionicTrainingBoundary({
      artifact,
      manifest,
      receiptImport: blockedPsionicTrainingReceiptImport(["blocker.psionic_training.worker_receipt_missing"]),
      sidecar,
    })

    expect(ready).toMatchObject({
      state: "ready",
      supportsTraining: true,
      blockerRefs: [],
    })
    expect(ready.evidenceRefs).toContain("contract.psionic.training_job.v1")
    expect(ready.evidenceRefs).toContain("receipt.psionic.training_worker.boundary.v1")
    expect(blocked.supportsTraining).toBe(false)
    expect(blocked.blockerRefs).toContain("blocker.psionic_training.boundary_receipt_import_not_ready")

    const defaultGate = projectLaunchGateMatrix()
    const readyGate = projectLaunchGateMatrix({ psionicTrainingBoundary: ready })

    expect(defaultGate.supportsTraining).toBe(false)
    expect(defaultGate.gates.find((gate) => gate.claimRef === "claim.pylon.qwen_training")?.state).toBe("blocked")
    expect(readyGate.supportsTraining).toBe(true)
    expect(readyGate.gates.find((gate) => gate.claimRef === "claim.pylon.psionic_training_boundary")?.state).toBe(
      "allowed",
    )
    expect(readyGate.gates.find((gate) => gate.claimRef === "claim.pylon.qwen_training")?.state).toBe("blocked")
    assertPublicProjectionSafe(ready)
  })
})

function releaseManifest(): PsionicTrainingReleaseManifest {
  return {
    schema: "openagents.psionic.training_release_manifest.v0.3",
    releaseRef: "release.psionic.training.boundary.v1",
    trainingJobContractRef: "contract.psionic.training_job.v1",
    workerReceiptFormatRef: "receipt_format.psionic.training_worker.v1",
    sidecarProtocolRef: "protocol.psionic.training_sidecar.v1",
    artifactRefs: ["artifact.psionic.training.runtime.v1"],
    signature: {
      signatureRef: "signature.psionic.release.boundary.v1",
      signerRef: trustedSignerRef,
      verificationRef: "verification.psionic.release.boundary.v1",
    },
  }
}

function artifactManifest(bytes: Uint8Array): PsionicTrainingArtifactManifest {
  return {
    schema: "openagents.psionic.training_artifact_manifest.v0.3",
    artifactRef: "artifact.psionic.training.runtime.v1",
    digestSha256: sha256(bytes),
    modelRef: "model.psionic.qwen35.training_fixture.v1",
    trainingRuntimeRef: "runtime.psionic.training.boundary.v1",
  }
}

function workerReceipt(): PsionicTrainingWorkerReceipt {
  return {
    schema: "openagents.psionic.training_worker_receipt.v0.3",
    receiptRef: "receipt.psionic.training_worker.boundary.v1",
    assignmentRef: "assignment.public.psionic_training.boundary.v1",
    workerRef: "pylon.public.psionic_training.worker.v1",
    runRef: "run.psionic.training.boundary.v1",
    artifactRefs: ["artifact.psionic.training.output.boundary.v1"],
    checkpointRefs: ["checkpoint.psionic.training.boundary.v1"],
    metricRefs: ["metric.psionic.training.loss_curve.boundary.v1"],
    proofRefs: ["proof.psionic.training.boundary.v1"],
    signature: {
      signatureRef: "signature.psionic.worker_receipt.boundary.v1",
      signerRef: trustedSignerRef,
      verificationRef: "verification.psionic.worker_receipt.boundary.v1",
    },
  }
}

function sha256(bytes: Uint8Array) {
  return createHash("sha256").update(bytes).digest("hex")
}
