import { describe, expect, it } from "vite-plus/test";

import {
  MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION,
  MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION,
  MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  MANAGED_SANDBOX_FORK_RECEIPT_SCHEMA_VERSION,
  MANAGED_SANDBOX_PRIVATE_INGRESS_ADMISSION,
  MANAGED_SANDBOX_PRIVATE_INGRESS_SCHEMA_VERSION,
  MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  MANAGED_SANDBOX_RESTORE_RECEIPT_SCHEMA_VERSION,
  decodeManagedSandboxCheckpointDeleteReceipt,
  decodeManagedSandboxCheckpointStopOutcome,
  decodeManagedSandboxContentCheckpoint,
  decodeManagedSandboxForkReceipt,
  decodeManagedSandboxPrivateIngressCapability,
  decodeManagedSandboxPhase2Command,
  decodeManagedSandboxRestoreReceipt,
} from "./phase2.ts";

const digest = (character: string) => `sha256:${character.repeat(64)}`;

const omissions = {
  credentials: "excluded" as const,
  accountSecrets: "excluded" as const,
  providerHiddenState: "excluded" as const,
  processMemory: "excluded" as const,
  processTable: "excluded" as const,
  ptyState: "excluded" as const,
  sockets: "excluded" as const,
  ports: "excluded" as const,
  networkIdentity: "excluded" as const,
};

const checkpoint = () => ({
  schema: MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  checkpointRef: "checkpoint.sbx10.1",
  ownerRef: "owner.test",
  tenantRef: "tenant.test",
  sourceSandboxRef: "sandbox.source.1",
  sourceResourceGeneration: 4,
  sourceImageDigest: digest("a"),
  sourceToolchainDigest: digest("b"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.0d0f44c",
  repositoryPostImageDigest: digest("c"),
  contentDigest: digest("d"),
  contentBytes: 4_096,
  formatRef: "format.sbx.content-tar.v1",
  state: "completed" as const,
  completedAt: "2026-07-22T00:00:00.000Z",
  verifiedAt: "2026-07-22T00:00:01.000Z",
  retainedUntil: "2026-07-23T00:00:01.000Z",
  deleteOnExpiry: true as const,
  omissions,
  evidenceRefs: ["receipt.checkpoint.verify.1"],
});

describe("managed sandbox Phase 2 contract", () => {
  it("decodes reference-only lifecycle commands and rejects unknown payload fields", () => {
    const command = {
      _tag: "ForkFromCheckpoint" as const,
      schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
      commandRef: "command.fork.1",
      idempotencyRef: "idempotency.fork.1",
      ownerRef: "owner.test",
      tenantRef: "tenant.test",
      checkpointRef: "checkpoint.sbx10.1",
      expectedSourceSandboxRef: "sandbox.source.1",
      expectedSourceResourceGeneration: 4,
      sourceCapabilityRefs: ["capability.source.command"],
      requestedAt: "2026-07-22T00:02:00.000Z",
    };

    expect(decodeManagedSandboxPhase2Command(command)["_tag"]).toBe("ForkFromCheckpoint");
    expect(() =>
      decodeManagedSandboxPhase2Command({ ...command, rawCredential: "not-admitted" }),
    ).toThrow();
  });

  it("accepts a verified content checkpoint with exact source and omission facts", () => {
    const decoded = decodeManagedSandboxContentCheckpoint(checkpoint());

    expect(decoded.sourceSandboxRef).toBe("sandbox.source.1");
    expect(decoded.sourceResourceGeneration).toBe(4);
    expect(decoded.omissions.processMemory).toBe("excluded");
    expect(decoded.deleteOnExpiry).toBe(true);
  });

  it("rejects raw credentials and invalid checkpoint chronology", () => {
    expect(() =>
      decodeManagedSandboxContentCheckpoint({
        ...checkpoint(),
        rawCredential: "must-not-cross-the-boundary",
      }),
    ).toThrow();
    expect(() =>
      decodeManagedSandboxContentCheckpoint({
        ...checkpoint(),
        verifiedAt: "2026-07-22T00:00:02.000Z",
        retainedUntil: "2026-07-22T00:00:01.000Z",
      }),
    ).toThrow();
  });

  it("cannot claim archived after a required checkpoint fails", () => {
    const failed = decodeManagedSandboxCheckpointStopOutcome({
      _tag: "CheckpointFailed",
      schema: MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION,
      stopRef: "stop.sbx10.1",
      sandboxRef: "sandbox.source.1",
      resourceGeneration: 4,
      attemptedCheckpointRef: "checkpoint.sbx10.failed",
      errorRef: "error.checkpoint.partial-upload",
      lifecycle: "recovery_required",
      archiveClaim: "forbidden",
      observedAt: "2026-07-22T00:01:00.000Z",
      evidenceRefs: ["receipt.checkpoint.failed.1"],
    });

    expect(failed.archiveClaim).toBe("forbidden");
    expect(() =>
      decodeManagedSandboxCheckpointStopOutcome({ ...failed, archiveClaim: "allowed" }),
    ).toThrow();
    expect(() =>
      decodeManagedSandboxCheckpointStopOutcome({
        _tag: "Archived",
        schema: MANAGED_SANDBOX_CHECKPOINT_STOP_SCHEMA_VERSION,
        stopRef: "stop.sbx10.other",
        sandboxRef: "sandbox.other.1",
        resourceGeneration: 4,
        checkpoint: checkpoint(),
        lifecycle: "stopped",
        archiveClaim: "allowed",
        observedAt: "2026-07-22T00:01:00.000Z",
        evidenceRefs: ["receipt.checkpoint.other.1"],
      }),
    ).toThrow();
  });

  it("requires a new fork identity and capability refs that do not come from the source", () => {
    const receipt = {
      schema: MANAGED_SANDBOX_FORK_RECEIPT_SCHEMA_VERSION,
      receiptRef: "receipt.fork.1",
      ownerRef: "owner.test",
      tenantRef: "tenant.test",
      checkpointRef: "checkpoint.sbx10.1",
      sourceSandboxRef: "sandbox.source.1",
      sourceResourceGeneration: 4,
      forkSandboxRef: "sandbox.fork.1",
      forkResourceGeneration: 1,
      sourceCapabilityRefs: ["capability.source.command"],
      forkCapabilityRefs: ["capability.fork.command"],
      grantPolicy: "mint_fresh" as const,
      cleanupObligationRef: "cleanup.sandbox.fork.1",
      stateTransfer: omissions,
      processSessionContinuity: "none" as const,
      outcome: "created" as const,
      observedAt: "2026-07-22T00:02:00.000Z",
      evidenceRefs: ["receipt.fork.identity.1"],
    };

    expect(decodeManagedSandboxForkReceipt(receipt).forkResourceGeneration).toBe(1);
    expect(() =>
      decodeManagedSandboxForkReceipt({ ...receipt, forkResourceGeneration: 0 }),
    ).toThrow();
    expect(() =>
      decodeManagedSandboxForkReceipt({ ...receipt, forkSandboxRef: receipt.sourceSandboxRef }),
    ).toThrow();
    expect(() =>
      decodeManagedSandboxForkReceipt({
        ...receipt,
        forkCapabilityRefs: receipt.sourceCapabilityRefs,
      }),
    ).toThrow();
  });

  it("restarts admitted services only and records process-session discontinuity", () => {
    const receipt = {
      schema: MANAGED_SANDBOX_RESTORE_RECEIPT_SCHEMA_VERSION,
      receiptRef: "receipt.restore.1",
      ownerRef: "owner.test",
      tenantRef: "tenant.test",
      checkpointRef: "checkpoint.sbx10.1",
      sandboxRef: "sandbox.source.1",
      checkpointSourceGeneration: 4,
      restoredResourceGeneration: 5,
      admittedServiceRefs: ["service.agent-runtime"],
      restartedServiceRefs: ["service.agent-runtime"],
      sourceCapabilityRefs: ["capability.source.command"],
      restoredCapabilityRefs: ["capability.restored.command"],
      grantPolicy: "mint_fresh" as const,
      processSessionContinuity: "discontinuous" as const,
      processMemoryRestored: false as const,
      ptyRestored: false as const,
      socketsRestored: false as const,
      outcome: "restored" as const,
      observedAt: "2026-07-22T00:03:00.000Z",
      evidenceRefs: ["receipt.restore.1"],
    };

    expect(decodeManagedSandboxRestoreReceipt(receipt).processSessionContinuity).toBe(
      "discontinuous",
    );
    expect(() =>
      decodeManagedSandboxRestoreReceipt({
        ...receipt,
        restartedServiceRefs: ["service.unadmitted"],
      }),
    ).toThrow();
    expect(() =>
      decodeManagedSandboxRestoreReceipt({ ...receipt, restoredResourceGeneration: 4 }),
    ).toThrow();
  });

  it("binds checkpoint deletion proof to the deleted content identity", () => {
    const receipt = decodeManagedSandboxCheckpointDeleteReceipt({
      schema: MANAGED_SANDBOX_CHECKPOINT_DELETE_RECEIPT_SCHEMA_VERSION,
      receiptRef: "receipt.checkpoint.delete.1",
      ownerRef: "owner.test",
      tenantRef: "tenant.test",
      checkpointRef: "checkpoint.sbx10.1",
      sourceSandboxRef: "sandbox.source.1",
      sourceResourceGeneration: 4,
      contentDigest: digest("d"),
      contentDeleted: true,
      outcome: "deleted",
      reason: "owner_requested",
      deletedAt: "2026-07-22T00:04:00.000Z",
      evidenceRefs: ["receipt.checkpoint.object-delete.1"],
    });

    expect(receipt.contentDeleted).toBe(true);
    expect(receipt.reason).toBe("owner_requested");
  });

  it("keeps private ingress unavailable and rejects raw or long-lived access URLs", () => {
    expect(MANAGED_SANDBOX_PRIVATE_INGRESS_ADMISSION).toEqual({
      schema: MANAGED_SANDBOX_PRIVATE_INGRESS_SCHEMA_VERSION,
      available: false,
      reason: "security_proof_pending",
      publicVnc: "unsupported",
      ungatedPreview: "unsupported",
      permanentRoute: "unsupported",
    });

    const capability = {
      _tag: "Active" as const,
      schema: MANAGED_SANDBOX_PRIVATE_INGRESS_SCHEMA_VERSION,
      capabilityRef: "capability.ingress.1",
      sandboxRef: "sandbox.source.1",
      resourceGeneration: 4,
      ownerRef: "owner.test",
      audienceRef: "audience.owner-device.1",
      kind: "preview" as const,
      issuedAt: "2026-07-22T00:04:00.000Z",
      expiresAt: "2026-07-22T00:09:00.000Z",
      ttlSeconds: 300,
      accessUrlDigest: digest("e"),
      accessUrlAtRest: "redacted" as const,
      audiencePolicy: "owner_scoped_explicit_audience" as const,
      publicAccess: false as const,
      permanentRoute: false as const,
      vnc: "unsupported" as const,
      auditRefs: ["audit.ingress.create.1"],
    };

    expect(decodeManagedSandboxPrivateIngressCapability(capability).accessUrlAtRest).toBe(
      "redacted",
    );
    expect(() =>
      decodeManagedSandboxPrivateIngressCapability({
        ...capability,
        accessUrl: "https://bearer-url.invalid",
      }),
    ).toThrow();
    expect(() =>
      decodeManagedSandboxPrivateIngressCapability({
        ...capability,
        expiresAt: "2026-07-22T00:24:00.000Z",
        ttlSeconds: 1_200,
      }),
    ).toThrow();
  });
});
