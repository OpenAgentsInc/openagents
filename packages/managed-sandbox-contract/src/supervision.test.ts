import { describe, expect, test } from "vite-plus/test";

import {
  decodeManagedSandboxSupervisionCommand,
  decodeManagedSandboxSupervisionProjection,
} from "./supervision.ts";

const projection = {
  schema: "openagents.managed_sandbox_supervision.v1",
  sandboxRef: "sandbox.fixture.1",
  workUnitRef: "work.fixture.1",
  attachmentRef: "attachment.fixture.1",
  attachmentGeneration: 4,
  resourceGeneration: 2,
  version: 9,
  target: {
    targetRef: "target.openagents.gcp.us-central1",
    provider: "google_cloud",
    region: "us-central1",
    isolation: "gce_vm",
    custody: "openagents_managed_region",
  },
  state: { lifecycle: "running", runtime: "running", acceptingWork: true },
  timing: {
    createdAt: "2026-07-19T12:00:00.000Z",
    updatedAt: "2026-07-19T12:04:00.000Z",
    leaseExpiresAt: "2026-07-19T13:00:00.000Z",
    elapsedSeconds: 300,
    idleSeconds: 60,
    leaseState: "active",
  },
  budget: {
    class: "bounded",
    currency: "USD",
    maxCostMicros: 5_000_000,
    observedCostMicros: null,
    state: "unreported",
  },
  runtime: {
    turnRef: "turn.fixture.1",
    status: "running",
    identity: {
      provider: "codex",
      modelRef: "model.gpt-5.6-sol",
      harnessRef: "harness.codex.v1",
    },
    actorRef: "principal.sarah",
    startedAt: "2026-07-19T12:01:00.000Z",
    settledAt: null,
    terminalReasonRef: null,
  },
  lastStructuralEvent: {
    eventRef: "event.fixture.4",
    kind: "RuntimeToolCompleted",
    sequence: 4,
    observedAt: "2026-07-19T12:04:00.000Z",
  },
  attention: { state: "none", reasonRef: null },
  cleanup: { state: "not_started", receiptRef: null },
  outcomes: {
    fileRefs: ["file.safe.1"],
    changeRefs: ["change.safe.1"],
    artifactRefs: ["artifact.safe.1"],
    evidenceRefs: ["evidence.safe.1"],
    receiptRefs: ["receipt.safe.1"],
  },
} as const;

describe("managed-sandbox bounded supervision contract", () => {
  test("decodes the explicit safe projection and keeps Sarah attribution", () => {
    expect(decodeManagedSandboxSupervisionProjection(projection)).toEqual(projection);
  });

  test("rejects raw paths, provider credentials, and runtime output as excess fields", () => {
    for (const forbidden of [
      { rawPath: "/workspace/private" },
      { providerCredential: "secret" },
      { runtimeOutput: "private transcript" },
      { shell: "bash" },
    ]) {
      expect(() =>
        decodeManagedSandboxSupervisionProjection({ ...projection, ...forbidden }),
      ).toThrow();
    }
  });

  test("admits only typed mobile/web lifecycle controls with exact generation", () => {
    const command = {
      schema: "openagents.managed_sandbox_supervision_command.v1",
      _tag: "Interrupt",
      commandRef: "command.mobile.interrupt.1",
      idempotencyRef: "idempotency.mobile.interrupt.1",
      surface: "mobile",
      sandboxRef: projection.sandboxRef,
      expectedVersion: projection.version,
      expectedResourceGeneration: projection.resourceGeneration,
      turnRef: "turn.fixture.1",
      reasonRef: "reason.owner_interrupt",
      issuedAt: "2026-07-19T12:05:00.000Z",
      expiresAt: "2026-07-19T12:06:00.000Z",
    } as const;
    expect(decodeManagedSandboxSupervisionCommand(command)).toEqual(command);
    expect(() =>
      decodeManagedSandboxSupervisionCommand({
        ...command,
        _tag: "Shell",
        command: "rm -rf /",
      }),
    ).toThrow();
    expect(() =>
      decodeManagedSandboxSupervisionCommand({
        ...command,
        expectedResourceGeneration: 1,
        expiresAt: command.issuedAt,
      }),
    ).toThrow();
  });
});
