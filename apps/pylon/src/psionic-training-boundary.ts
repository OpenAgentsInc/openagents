import { createHash } from "node:crypto"
import { assertPublicProjectionSafe } from "./state.js"

export type PsionicTrainingBoundaryState = "blocked" | "ready"
export type PsionicTrainingSidecarState = "blocked" | "starting" | "healthy" | "crashed" | "stopped"

export type PsionicTrainingReleaseManifest = {
  schema: "openagents.psionic.training_release_manifest.v0.3"
  releaseRef: string
  trainingJobContractRef: string
  workerReceiptFormatRef: string
  sidecarProtocolRef: string
  artifactRefs: string[]
  signature: {
    signatureRef: string
    signerRef: string
    verificationRef: string
  }
}

export type PsionicTrainingArtifactManifest = {
  schema: "openagents.psionic.training_artifact_manifest.v0.3"
  artifactRef: string
  digestSha256: string
  modelRef: string
  trainingRuntimeRef: string
}

export type PsionicTrainingSidecarEvent = {
  eventRef: string
  kind: "start_requested" | "started" | "health_ready" | "crashed" | "stopped"
}

export type PsionicTrainingWorkerReceipt = {
  schema: "openagents.psionic.training_worker_receipt.v0.3"
  receiptRef: string
  assignmentRef: string
  workerRef: string
  runRef: string
  artifactRefs: string[]
  checkpointRefs: string[]
  metricRefs: string[]
  proofRefs: string[]
  signature: {
    signatureRef: string
    signerRef: string
    verificationRef: string
  }
}

export type PsionicTrainingManifestProjection = {
  schema: "openagents.pylon.psionic_training_manifest.v0.3"
  state: PsionicTrainingBoundaryState
  releaseRef: string | null
  trainingJobContractRef: string | null
  workerReceiptFormatRef: string | null
  sidecarProtocolRef: string | null
  artifactRefs: string[]
  signerRef: string | null
  verificationRef: string | null
  blockerRefs: string[]
  contentRedacted: true
}

export type PsionicTrainingArtifactProjection = {
  schema: "openagents.pylon.psionic_training_artifact.v0.3"
  state: PsionicTrainingBoundaryState
  artifactRef: string | null
  digestRef: string | null
  modelRef: string | null
  trainingRuntimeRef: string | null
  blockerRefs: string[]
  contentRedacted: true
}

export type PsionicTrainingSidecarProjection = {
  schema: "openagents.pylon.psionic_training_sidecar.v0.3"
  state: PsionicTrainingSidecarState
  eventRefs: string[]
  blockerRefs: string[]
  sandboxPolicyRefs: string[]
  contentRedacted: true
}

export type PsionicTrainingReceiptImportProjection = {
  schema: "openagents.pylon.psionic_training_receipt_import.v0.3"
  state: PsionicTrainingBoundaryState
  receiptRef: string | null
  assignmentRef: string | null
  workerRef: string | null
  runRef: string | null
  artifactRefs: string[]
  checkpointRefs: string[]
  metricRefs: string[]
  proofRefs: string[]
  signerRef: string | null
  verificationRef: string | null
  closeoutEvidenceRefs: string[]
  blockerRefs: string[]
  contentRedacted: true
}

export type PsionicTrainingBoundaryProjection = {
  schema: "openagents.pylon.psionic_training_boundary.v0.3"
  state: PsionicTrainingBoundaryState
  supportsTraining: boolean
  manifest: PsionicTrainingManifestProjection
  artifact: PsionicTrainingArtifactProjection
  sidecar: PsionicTrainingSidecarProjection
  receiptImport: PsionicTrainingReceiptImportProjection
  evidenceRefs: string[]
  blockerRefs: string[]
  caveatRefs: string[]
  externalDependencyRefs: string[]
  contentRedacted: true
}

export type PsionicTrainingBoundaryInput = {
  manifest: PsionicTrainingManifestProjection
  artifact: PsionicTrainingArtifactProjection
  sidecar: PsionicTrainingSidecarProjection
  receiptImport: PsionicTrainingReceiptImportProjection
}

const safeRefPattern = /^[a-z][a-z0-9._:-]{1,220}$/i
const unsafeRefPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|checkpoint[-_]?path|invoice|lnbc|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|prompt|secret|token|wallet|weights\.(bin|gguf|safetensors|pt|pth))/i
const sha256Pattern = /^[a-f0-9]{64}$/i

const sandboxPolicyRefs = [
  "policy.pylon.psionic_training.attach_only",
  "policy.pylon.psionic_training.no_startup_download",
  "policy.pylon.psionic_training.digest_verified_artifacts_only",
  "policy.pylon.psionic_training.public_safe_receipts_only",
]

const caveatRefs = [
  "caveat.pylon.psionic_training.boundary_only",
  "caveat.pylon.psionic_training.psionic_execution_external",
  "caveat.pylon.psionic_training.no_model_training_claim_without_live_receipts",
]

const externalDependencyRefs = [
  "external.psionic.training_job_contract",
  "external.psionic.signed_release_manifest",
  "external.psionic.worker_receipt_format",
]

export function verifyPsionicTrainingReleaseManifest(
  manifest: unknown,
  options: { trustedSignerRefs?: string[] } = {},
): PsionicTrainingManifestProjection {
  if (!isTrainingReleaseManifest(manifest)) {
    return blockedManifest(["blocker.psionic_training.release_manifest_invalid"])
  }
  const trustedSignerRefs = options.trustedSignerRefs ?? []
  const refs = [
    manifest.releaseRef,
    manifest.trainingJobContractRef,
    manifest.workerReceiptFormatRef,
    manifest.sidecarProtocolRef,
    ...manifest.artifactRefs,
    manifest.signature.signatureRef,
    manifest.signature.signerRef,
    manifest.signature.verificationRef,
  ]
  const blockerRefs = [
    ...unsafeRefBlockers(refs, "release_manifest"),
    ...(!isPublicSignature(manifest.signature.signatureRef)
      ? ["blocker.psionic_training.release_manifest_unsigned"]
      : []),
    ...(trustedSignerRefs.length > 0 && !trustedSignerRefs.includes(manifest.signature.signerRef)
      ? ["blocker.psionic_training.release_manifest_untrusted_signer"]
      : []),
  ]

  return safeManifest({
    schema: "openagents.pylon.psionic_training_manifest.v0.3",
    state: blockerRefs.length === 0 ? "ready" : "blocked",
    releaseRef: manifest.releaseRef,
    trainingJobContractRef: manifest.trainingJobContractRef,
    workerReceiptFormatRef: manifest.workerReceiptFormatRef,
    sidecarProtocolRef: manifest.sidecarProtocolRef,
    artifactRefs: uniqueRefs(manifest.artifactRefs),
    signerRef: manifest.signature.signerRef,
    verificationRef: manifest.signature.verificationRef,
    blockerRefs,
    contentRedacted: true,
  })
}

export function verifyPsionicTrainingArtifact(
  manifest: PsionicTrainingArtifactManifest,
  bytes: Uint8Array,
): PsionicTrainingArtifactProjection {
  const digest = sha256(bytes)
  const refs = [
    manifest.artifactRef,
    manifest.modelRef,
    manifest.trainingRuntimeRef,
  ]
  const blockerRefs = [
    ...unsafeRefBlockers(refs, "artifact"),
    ...(manifest.schema !== "openagents.psionic.training_artifact_manifest.v0.3"
      ? ["blocker.psionic_training.artifact_manifest_invalid"]
      : []),
    ...(!sha256Pattern.test(manifest.digestSha256)
      ? ["blocker.psionic_training.artifact_digest_invalid"]
      : []),
    ...(digest !== manifest.digestSha256.toLowerCase()
      ? ["blocker.psionic_training.artifact_digest_mismatch"]
      : []),
  ]

  return safeArtifact({
    schema: "openagents.pylon.psionic_training_artifact.v0.3",
    state: blockerRefs.length === 0 ? "ready" : "blocked",
    artifactRef: manifest.artifactRef,
    digestRef: `artifact.digest.sha256.${digest}`,
    modelRef: manifest.modelRef,
    trainingRuntimeRef: manifest.trainingRuntimeRef,
    blockerRefs,
    contentRedacted: true,
  })
}

export function projectPsionicTrainingSidecarLifecycle(
  events: PsionicTrainingSidecarEvent[],
): PsionicTrainingSidecarProjection {
  const eventRefs = uniqueRefs(events.map((event) => event.eventRef))
  const kinds = new Set(events.map((event) => event.kind))
  const unsafeBlockers = unsafeRefBlockers(eventRefs, "sidecar")
  const state: PsionicTrainingSidecarState =
    unsafeBlockers.length > 0
      ? "blocked"
      : kinds.has("crashed")
        ? "crashed"
        : kinds.has("stopped")
          ? "stopped"
          : kinds.has("health_ready")
            ? "healthy"
            : kinds.has("started") || kinds.has("start_requested")
              ? "starting"
              : "blocked"
  const blockerRefs = [
    ...unsafeBlockers,
    ...(state === "blocked" ? ["blocker.psionic_training.sidecar_not_started"] : []),
    ...(state === "starting" ? ["blocker.psionic_training.sidecar_health_missing"] : []),
    ...(state === "crashed" ? ["blocker.psionic_training.sidecar_crashed"] : []),
    ...(state === "stopped" ? ["blocker.psionic_training.sidecar_stopped"] : []),
  ]

  return safeSidecar({
    schema: "openagents.pylon.psionic_training_sidecar.v0.3",
    state,
    eventRefs,
    blockerRefs,
    sandboxPolicyRefs,
    contentRedacted: true,
  })
}

export function importPsionicTrainingWorkerReceipt(
  receipt: PsionicTrainingWorkerReceipt,
  options: { trustedSignerRefs?: string[] } = {},
): PsionicTrainingReceiptImportProjection {
  const refs = [
    receipt.receiptRef,
    receipt.assignmentRef,
    receipt.workerRef,
    receipt.runRef,
    ...receipt.artifactRefs,
    ...receipt.checkpointRefs,
    ...receipt.metricRefs,
    ...receipt.proofRefs,
    receipt.signature.signatureRef,
    receipt.signature.signerRef,
    receipt.signature.verificationRef,
  ]
  const trustedSignerRefs = options.trustedSignerRefs ?? []
  const blockerRefs = [
    ...(receipt.schema !== "openagents.psionic.training_worker_receipt.v0.3"
      ? ["blocker.psionic_training.worker_receipt_invalid"]
      : []),
    ...unsafeRefBlockers(refs, "worker_receipt"),
    ...(!isPublicSignature(receipt.signature.signatureRef)
      ? ["blocker.psionic_training.worker_receipt_unsigned"]
      : []),
    ...(trustedSignerRefs.length > 0 && !trustedSignerRefs.includes(receipt.signature.signerRef)
      ? ["blocker.psionic_training.worker_receipt_untrusted_signer"]
      : []),
  ]
  const closeoutEvidenceRefs = uniqueRefs([
    receipt.receiptRef,
    receipt.runRef,
    ...receipt.artifactRefs,
    ...receipt.checkpointRefs,
    ...receipt.metricRefs,
    ...receipt.proofRefs,
    receipt.signature.verificationRef,
  ])

  return safeReceiptImport({
    schema: "openagents.pylon.psionic_training_receipt_import.v0.3",
    state: blockerRefs.length === 0 ? "ready" : "blocked",
    receiptRef: receipt.receiptRef,
    assignmentRef: receipt.assignmentRef,
    workerRef: receipt.workerRef,
    runRef: receipt.runRef,
    artifactRefs: uniqueRefs(receipt.artifactRefs),
    checkpointRefs: uniqueRefs(receipt.checkpointRefs),
    metricRefs: uniqueRefs(receipt.metricRefs),
    proofRefs: uniqueRefs(receipt.proofRefs),
    signerRef: receipt.signature.signerRef,
    verificationRef: receipt.signature.verificationRef,
    closeoutEvidenceRefs,
    blockerRefs,
    contentRedacted: true,
  })
}

export function projectPsionicTrainingBoundary(
  input: PsionicTrainingBoundaryInput,
): PsionicTrainingBoundaryProjection {
  const blockerRefs = uniqueRefs([
    ...input.manifest.blockerRefs,
    ...input.artifact.blockerRefs,
    ...input.sidecar.blockerRefs,
    ...input.receiptImport.blockerRefs,
    ...(input.manifest.state !== "ready" ? ["blocker.psionic_training.boundary_manifest_not_ready"] : []),
    ...(input.artifact.state !== "ready" ? ["blocker.psionic_training.boundary_artifact_not_ready"] : []),
    ...(input.sidecar.state !== "healthy" ? ["blocker.psionic_training.boundary_sidecar_not_healthy"] : []),
    ...(input.receiptImport.state !== "ready" ? ["blocker.psionic_training.boundary_receipt_import_not_ready"] : []),
  ])
  const supportsTraining = blockerRefs.length === 0

  const projection: PsionicTrainingBoundaryProjection = {
    schema: "openagents.pylon.psionic_training_boundary.v0.3",
    state: supportsTraining ? "ready" : "blocked",
    supportsTraining,
    manifest: input.manifest,
    artifact: input.artifact,
    sidecar: input.sidecar,
    receiptImport: input.receiptImport,
    evidenceRefs: supportsTraining
      ? uniqueRefs([
          input.manifest.releaseRef ?? "",
          input.manifest.trainingJobContractRef ?? "",
          input.manifest.workerReceiptFormatRef ?? "",
          input.artifact.artifactRef ?? "",
          input.artifact.digestRef ?? "",
          ...input.sidecar.eventRefs,
          ...input.receiptImport.closeoutEvidenceRefs,
        ])
      : [],
    blockerRefs,
    caveatRefs,
    externalDependencyRefs,
    contentRedacted: true,
  }
  assertPublicProjectionSafe(projection)
  return projection
}

export const blockedPsionicTrainingReceiptImport = (
  blockerRefs: string[],
): PsionicTrainingReceiptImportProjection =>
  safeReceiptImport({
    schema: "openagents.pylon.psionic_training_receipt_import.v0.3",
    state: "blocked",
    receiptRef: null,
    assignmentRef: null,
    workerRef: null,
    runRef: null,
    artifactRefs: [],
    checkpointRefs: [],
    metricRefs: [],
    proofRefs: [],
    signerRef: null,
    verificationRef: null,
    closeoutEvidenceRefs: [],
    blockerRefs,
    contentRedacted: true,
  })

function blockedManifest(blockerRefs: string[]): PsionicTrainingManifestProjection {
  return safeManifest({
    schema: "openagents.pylon.psionic_training_manifest.v0.3",
    state: "blocked",
    releaseRef: null,
    trainingJobContractRef: null,
    workerReceiptFormatRef: null,
    sidecarProtocolRef: null,
    artifactRefs: [],
    signerRef: null,
    verificationRef: null,
    blockerRefs,
    contentRedacted: true,
  })
}

function isTrainingReleaseManifest(value: unknown): value is PsionicTrainingReleaseManifest {
  if (typeof value !== "object" || value === null) return false
  const record = value as Partial<PsionicTrainingReleaseManifest>
  return (
    record.schema === "openagents.psionic.training_release_manifest.v0.3" &&
    typeof record.releaseRef === "string" &&
    typeof record.trainingJobContractRef === "string" &&
    typeof record.workerReceiptFormatRef === "string" &&
    typeof record.sidecarProtocolRef === "string" &&
    Array.isArray(record.artifactRefs) &&
    typeof record.signature === "object" &&
    record.signature !== null &&
    typeof record.signature.signatureRef === "string" &&
    typeof record.signature.signerRef === "string" &&
    typeof record.signature.verificationRef === "string"
  )
}

function unsafeRefBlockers(refs: string[], label: string) {
  return refs.some((ref) => !isSafeRef(ref))
    ? [`blocker.psionic_training.${label}_unsafe_ref`]
    : []
}

function isSafeRef(value: string) {
  return safeRefPattern.test(value) && !unsafeRefPattern.test(value)
}

function isPublicSignature(value: string) {
  return /^signature\.psionic\.[a-z0-9._:-]+$/i.test(value) && isSafeRef(value)
}

function uniqueRefs(refs: string[]) {
  return [...new Set(refs.map((ref) => ref.trim()).filter((ref) => ref.length > 0))].sort()
}

function sha256(bytes: Uint8Array) {
  return createHash("sha256").update(bytes).digest("hex")
}

function safeManifest(projection: PsionicTrainingManifestProjection) {
  assertPublicProjectionSafe(projection)
  return projection
}

function safeArtifact(projection: PsionicTrainingArtifactProjection) {
  assertPublicProjectionSafe(projection)
  return projection
}

function safeSidecar(projection: PsionicTrainingSidecarProjection) {
  assertPublicProjectionSafe(projection)
  return projection
}

function safeReceiptImport(projection: PsionicTrainingReceiptImportProjection) {
  assertPublicProjectionSafe(projection)
  return projection
}
