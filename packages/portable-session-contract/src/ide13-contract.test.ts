import { describe, expect, test } from "vite-plus/test"
import { Schema } from "effect"

import {
  IdePortableCheckpointManifestSchema,
  IdePortableDestinationActivationReceiptSchema,
  IdePortableMoveReceiptSchema,
  IdePortablePlacementFactsSchema,
  IdePortableStaleWriter,
} from "./ide13-contract.js"

const digest = (value: string) => `sha256:${value.repeat(64)}`
const placement = (targetClass: "owner_local" | "owner_managed" | "openagents_managed" | "managed_provider") => ({
  placementRef: `placement.${targetClass}`,
  targetClass,
  providerRef: `provider.${targetClass}`,
  adapterRef: `adapter.${targetClass}`,
  ownerRef: "owner.1",
  operatingSystem: "linux" as const,
  architecture: "arm64" as const,
  isolation: targetClass === "owner_local" ? "owner_host_process" as const : "dedicated_microvm" as const,
  custody: targetClass === "owner_local" ? "owner_device" as const : targetClass === "owner_managed" ? "owner_managed" as const : "openagents_managed" as const,
  dataDestinations: ["region.us-central1"],
  networkDestinations: ["openagents.com"],
  retentionSeconds: 3_600,
  estimatedCostMicrounits: 0,
  freshness: "live" as const,
  observedAt: "2026-07-20T08:00:00.000Z",
  capabilities: [{ capabilityRef: "capability.files.1", kind: "files" as const, version: "1", generation: 2, readiness: "ready" as const, freshness: "live" as const, startupLatencyMs: 12, operationLatencyMs: 3, omissionRefs: [] }],
  degradedReasonRefs: [],
})

const project = {
  projectRef: "project.1", projectRootRef: "root.1", worktreeRef: "worktree.1",
  selectedFileRef: "file.1", documentSnapshotRef: "document.1", proposalRef: null,
  diagnosticResultRef: null, testResultRef: null, artifactRef: "artifact.1", evidenceRef: "evidence.1",
}

describe("IDE-13 portable capability contract", () => {
  test("decodes exact public-safe facts for every admitted placement class", () => {
    for (const targetClass of ["owner_local", "owner_managed", "openagents_managed", "managed_provider"] as const) {
      expect(Schema.decodeUnknownSync(IdePortablePlacementFactsSchema)(placement(targetClass)).targetClass).toBe(targetClass)
    }
  })

  test("bounds checkpoint bytes and excludes secret, process, native, Vim, and theme authority", () => {
    const manifest = {
      manifestRef: "manifest.1", checkpointRef: "checkpoint.1", sessionRef: "session.1",
      sourceAttachmentRef: "attachment.1", sourceGeneration: 1, digest: digest("a"), byteSize: 1_024,
      fileCount: 2, repositoryPostImageDigest: digest("b"), graphDigest: digest("c"), project,
      includedCapabilityRefs: ["capability.files.1"], omittedCapabilityRefs: ["capability.terminal.1"],
      historyRefs: [], proposalRefs: [], taskRefs: [], testRefs: [], deliveryEvidenceRefs: [],
      secretMaterial: "excluded", processState: "excluded", nativeState: "excluded",
      vimState: "destination_setting", themeState: "destination_setting",
      policy: { maximumBytes: 2_048, maximumFiles: 10, encryption: "owner_key", encryptionKeyRef: "key.1", custody: "owner_device", retentionSeconds: 3_600, expiresAt: "2026-07-20T09:00:00.000Z" },
      integrityReceiptRef: "receipt.integrity.1",
    }
    expect(Schema.decodeUnknownSync(IdePortableCheckpointManifestSchema)(manifest).processState).toBe("excluded")
    expect(() => Schema.decodeUnknownSync(IdePortableCheckpointManifestSchema)({ ...manifest, byteSize: 2_000_000_000 })).toThrow()
    expect(JSON.stringify(Schema.decodeUnknownSync(IdePortableCheckpointManifestSchema)({ ...manifest, token: "raw-secret" }))).not.toContain("raw-secret")
    expect(() => Schema.decodeUnknownSync(IdePortableCheckpointManifestSchema)({ ...manifest, vimState: "insert" })).toThrow()
  })

  test("decodes a generation-advancing move receipt and typed stale-writer failure", () => {
    const receipt = Schema.decodeUnknownSync(IdePortableMoveReceiptSchema)({
      receiptRef: "receipt.move.1", commandRef: "command.move.1", idempotencyKey: "idempotency.move.1",
      actorRef: "actor.owner.1", policyRef: "policy.1", sessionRef: "session.1", project,
      sourcePlacementRef: "placement.local", destinationPlacementRef: "placement.managed",
      sourceAttachmentRef: "attachment.1", sourceGeneration: 1, destinationAttachmentRef: "attachment.2",
      destinationGeneration: 2, checkpointManifestRef: "manifest.1", transition: "move", status: "completed",
      recoveryPointRef: "checkpoint.1", omissionRefs: [], evidenceRefs: ["evidence.move.1"],
      completedAt: "2026-07-20T08:05:00.000Z",
    })
    expect(receipt.destinationGeneration).toBe(2)
    expect(new IdePortableStaleWriter({ operation: "save", detailRef: "detail.stale.1", retryable: false })._tag).toBe("IdePortable.StaleWriter")
  })

  test("decodes a refs-only destination authentication and helper readiness receipt", () => {
    const receipt = Schema.decodeUnknownSync(IdePortableDestinationActivationReceiptSchema)({
      schema: "openagents.ide_portable_destination_activation.v1",
      receiptRef: "receipt.destination.2",
      operationRef: "operation.destination.activate.2",
      sessionRef: "session.1",
      checkpointRef: "checkpoint.1",
      destinationTargetRef: "target.owner.2",
      destinationAttachmentRef: "attachment.2",
      destinationRunnerSessionReservationRef: "reservation.destination.runner.2",
      destinationGeneration: 2,
      authentication: {
        state: "reauthenticated",
        policyRef: "policy.destination.2",
        evidenceRef: "evidence.authentication.2",
        observedAt: "2026-07-20T08:05:00.000Z",
        expiresAt: "2026-07-20T09:05:00.000Z",
      },
      helpersObservedAt: "2026-07-20T08:05:00.000Z",
      helpers: [
        { kind: "pty", readiness: "ready", instanceRef: "instance.pty.2", versionRef: "version.pty.1", omissionRef: null, evidenceRefs: ["evidence.pty.2"] },
        { kind: "lsp", readiness: "unsupported", instanceRef: null, versionRef: null, omissionRef: "omission.lsp.unsupported", evidenceRefs: [] },
        { kind: "dap", readiness: "unsupported", instanceRef: null, versionRef: null, omissionRef: "omission.dap.unsupported", evidenceRefs: [] },
        { kind: "watcher", readiness: "unsupported", instanceRef: null, versionRef: null, omissionRef: "omission.watcher.unsupported", evidenceRefs: [] },
        { kind: "native", readiness: "unsupported", instanceRef: null, versionRef: null, omissionRef: "omission.native.unsupported", evidenceRefs: [] },
      ],
      activatedAgentRefs: ["agent.root.1"],
      acceptedWorkRefs: [],
      evidenceRefs: ["evidence.authentication.2", "evidence.pty.2"],
    })
    expect(receipt.authentication.policyRef).toBe("policy.destination.2")
    expect(JSON.stringify(receipt)).not.toContain("credential")
    expect(() => Schema.decodeUnknownSync(IdePortableDestinationActivationReceiptSchema)({
      ...receipt,
      destinationGeneration: -1,
    })).toThrow()
  })
})
