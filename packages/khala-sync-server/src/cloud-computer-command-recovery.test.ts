import { describe, expect, test } from "vite-plus/test";

import {
  cloudComputerCommandRecoveryEvidenceAuthority,
  recoverCloudComputerCommand,
  type CloudComputerCommandRecoveryEvidence,
} from "./cloud-computer-command-recovery.js";

const authority = cloudComputerCommandRecoveryEvidenceAuthority({ verify: async () => true });
const evidence = (kind: CloudComputerCommandRecoveryEvidence["kind"]) =>
  authority.issue({
    kind,
    evidenceRef: `evidence.${kind}`,
    evidenceDigest: `sha256:${"a".repeat(64)}`,
    computerRef: "computer.six",
    workspaceRef: "workspace.six",
    runtimeRef: "runtime.six",
    runtimeGeneration: 4,
    providerLeaseRef: "lease.six",
    observedAt: "2026-08-22T12:00:00.000Z",
    durable: true,
  });

describe("cloud computer command recovery policy", () => {
  test("redispatches only a command with no provider-write exposure", () => {
    expect(
      recoverCloudComputerCommand({
        cause: "controller_restart",
        snapshot: {
          status: "not_dispatched",
          exposure: "prepared",
          dispatchRef: "dispatch.safe",
          providerExecutionRef: null,
        },
      }),
    ).toEqual({ kind: "redispatch", dispatchRef: "dispatch.safe" });
    expect(
      recoverCloudComputerCommand({
        cause: "controller_restart",
        snapshot: {
          status: "not_dispatched",
          exposure: "exposed",
          dispatchRef: "dispatch.unsafe",
          providerExecutionRef: null,
        },
      }),
    ).toEqual({ kind: "observe_or_reattach", providerExecutionRef: null });
  });

  test.each(["may_have_started", "dispatched", "running"] as const)(
    "observes or reattaches %s after transport loss",
    (status) => {
      expect(
        recoverCloudComputerCommand({
          cause: "transport_loss",
          snapshot: {
            status,
            exposure: "exposed",
            dispatchRef: "dispatch.started",
            providerExecutionRef: "execution.started",
          },
        }),
      ).toEqual({
        kind: "observe_or_reattach",
        providerExecutionRef: "execution.started",
      });
    },
  );

  test.each([
    ["runtime_crash", "runtime_lost"],
    ["host_loss", "host_lost"],
  ] as const)("settles %s only from matching durable evidence", async (cause, evidenceKind) => {
    const snapshot = {
      status: "running",
      exposure: "acknowledged",
      dispatchRef: "dispatch.running",
      providerExecutionRef: "execution.running",
    } as const;
    expect(recoverCloudComputerCommand({ cause, snapshot })).toEqual({
      kind: "await_durable_evidence",
      evidenceKind,
    });
    expect(
      recoverCloudComputerCommand({ cause, snapshot, evidence: await evidence(evidenceKind) }),
    ).toMatchObject({ kind: "settle_lost", evidence: { kind: evidenceKind } });
  });

  test("keeps checkpoint and cleanup failure evidence distinct and never authorizes replay", async () => {
    const snapshot = {
      status: "not_dispatched",
      exposure: "none",
      dispatchRef: null,
      providerExecutionRef: null,
    } as const;
    expect(
      recoverCloudComputerCommand({
        cause: "checkpoint_failure",
        snapshot,
        evidence: await evidence("checkpoint_failed"),
      }),
    ).toMatchObject({ kind: "record_checkpoint_failed" });
    expect(
      recoverCloudComputerCommand({
        cause: "cleanup_failure",
        snapshot,
        evidence: await evidence("cleanup_failed"),
      }),
    ).toMatchObject({ kind: "record_cleanup_failed" });
    expect(
      recoverCloudComputerCommand({
        cause: "cleanup_failure",
        snapshot,
        evidence: await evidence("checkpoint_failed"),
      }),
    ).toEqual({ kind: "await_durable_evidence", evidenceKind: "cleanup_failed" });
  });
});
