import { describe, expect, test } from "vite-plus/test";
import type {
  PortablePhaseOperationClaimRequest,
  PortablePhaseOperationRecord,
  PortablePhaseOperationRenewRequest,
  PortablePhaseOperationResultRequest,
} from "@openagentsinc/portable-session-contract";

import {
  makePylonPortablePhaseOperationClient,
  PylonPortablePhaseTransportError,
  type PylonPortablePhaseOperationClient,
} from "./portable-phase-operation-client.js";
import {
  makePylonPortablePhaseExecutor,
  PylonPortablePhaseWorker,
  type PylonPortablePhaseExecutor,
} from "./portable-phase-operation-worker.js";

const pylonRef = "pylon.ide13.fixture";
const targetRef = "target.ide13.fixture";
const workerInstanceRef = "worker.ide13.fixture";
const now = new Date("2026-07-20T12:00:00.000Z");

const record = (
  kind: PortablePhaseOperationRecord["request"]["kind"] = "quiesce",
): PortablePhaseOperationRecord => ({
  request: {
    schema: "openagents.portable_phase_operation.v1",
    operationRef: `operation.ide13.${kind}`,
    commandRef: "command.ide13.fixture",
    commandExecutionClaimRef: "claim.ide13.command",
    ownerRef: "owner.ide13.fixture",
    sessionRef: "session.ide13.fixture",
    attachmentRef: "attachment.ide13.fixture",
    attachmentGeneration: 1,
    targetRef,
    pylonRef,
    kind,
    checkpointRef: kind === "checkpoint-create" ? "checkpoint.ide13.fixture" : null,
    checkpointObjectRef: null,
    checkpointDigest: null,
    evidenceRefs: [],
    expiresAt: "2026-07-20T12:10:00.000Z",
  },
  requestFingerprint: `sha256:${"1".repeat(64)}`,
  state: "pending",
  claimRef: null,
  claimFingerprint: null,
  workerInstanceRef: null,
  claimGeneration: null,
  leaseRevision: null,
  claimedAt: null,
  leaseExpiresAt: null,
  resultRef: null,
  resultFingerprint: null,
  resultStatus: null,
  resultCheckpointRef: null,
  resultCheckpointObjectRef: null,
  resultCheckpointDigest: null,
  resultEvidenceRefs: [],
  errorRef: null,
  completedAt: null,
  updatedAt: now.toISOString(),
});

const claimedRecord = (
  source: PortablePhaseOperationRecord,
  claim: PortablePhaseOperationClaimRequest,
  revision = 1,
): PortablePhaseOperationRecord => ({
  ...source,
  state: "claimed",
  claimRef: claim.claimRef,
  claimFingerprint: `sha256:${"2".repeat(64)}`,
  workerInstanceRef: claim.workerInstanceRef,
  claimGeneration: 1,
  leaseRevision: revision,
  claimedAt: now.toISOString(),
  leaseExpiresAt: claim.leaseExpiresAt,
  updatedAt: now.toISOString(),
});

const fakeClient = (source: PortablePhaseOperationRecord) => {
  const calls: {
    claims: PortablePhaseOperationClaimRequest[];
    renewals: PortablePhaseOperationRenewRequest[];
    completions: PortablePhaseOperationResultRequest[];
  } = { claims: [], renewals: [], completions: [] };
  let current = source;
  const client: PylonPortablePhaseOperationClient = {
    pending: async () => (current.state === "pending" ? [current] : []),
    claim: async (request) => {
      calls.claims.push(request);
      current = claimedRecord(source, request);
      return { status: "claimed", operation: current };
    },
    renew: async (request) => {
      calls.renewals.push(request);
      current = {
        ...current,
        leaseRevision: request.expectedLeaseRevision + 1,
        leaseExpiresAt: request.leaseExpiresAt,
      };
      return { status: "renewed", operation: current };
    },
    complete: async (request) => {
      calls.completions.push(request);
      current = {
        ...current,
        state: request.resultStatus,
        leaseRevision: request.expectedLeaseRevision + 1,
        resultRef: request.resultRef,
        resultStatus: request.resultStatus,
        resultCheckpointRef: request.checkpointRef,
        resultCheckpointObjectRef: request.checkpointObjectRef,
        resultCheckpointDigest: request.checkpointDigest,
        resultEvidenceRefs: request.evidenceRefs,
        errorRef: request.errorRef,
        completedAt: request.completedAt,
      };
      return { status: request.resultStatus, operation: current };
    },
  };
  return { calls, client };
};

describe("Pylon portable phase HTTP client", () => {
  test("uses the exact bearer route and rejects scope drift and excess response fields", async () => {
    const calls: Array<{ url: string; init: RequestInit }> = [];
    const source = record();
    const client = makePylonPortablePhaseOperationClient({
      agentToken: "private-agent-token",
      baseUrl: "https://openagents.test",
      pylonRef,
      targetRef,
      fetchImpl: (async (url, init) => {
        calls.push({ url: String(url), init: init ?? {} });
        return Response.json({
          schema: "openagents.portable_phase_operation_transport.v1",
          operations: [source],
        });
      }) as typeof fetch,
    });
    expect(await client.pending(4)).toEqual([source]);
    const firstCall = calls.at(0);
    if (firstCall === undefined) throw new Error("expected one phase poll");
    expect(new URL(firstCall.url).pathname).toBe(
      `/api/pylons/${pylonRef}/portable-targets/${targetRef}/phase-operations`,
    );
    expect(new URL(firstCall.url).searchParams.get("limit")).toBe("4");
    expect(firstCall.init.headers).toMatchObject({ Authorization: "Bearer private-agent-token" });

    const drift = makePylonPortablePhaseOperationClient({
      agentToken: "private-agent-token",
      baseUrl: "https://openagents.test",
      pylonRef,
      targetRef,
      fetchImpl: (async () =>
        Response.json({
          schema: "openagents.portable_phase_operation_transport.v1",
          operations: [
            { ...source, request: { ...source.request, targetRef: "target.other.fixture" } },
          ],
        })) as typeof fetch,
    });
    await expect(drift.pending(1)).rejects.toMatchObject({ failure: "bad_response" });

    const excess = makePylonPortablePhaseOperationClient({
      agentToken: "private-agent-token",
      baseUrl: "https://openagents.test",
      pylonRef,
      targetRef,
      fetchImpl: (async () =>
        Response.json({
          schema: "openagents.portable_phase_operation_transport.v1",
          operations: [],
          privatePath: "/Users/private/repo",
        })) as typeof fetch,
    });
    await expect(excess.pending(1)).rejects.toBeInstanceOf(PylonPortablePhaseTransportError);
  });
});

describe("Pylon portable phase worker", () => {
  test("claims, renews with CAS, and completes an exact refs-only result", async () => {
    const source = record();
    const { calls, client } = fakeClient(source);
    let renewGateResolved = false;
    let releaseExecution!: () => void;
    const executionGate = new Promise<void>((resolve) => {
      releaseExecution = resolve;
    });
    const executor: PylonPortablePhaseExecutor = {
      execute: async () => {
        await executionGate;
        return {
          checkpointRef: null,
          checkpointObjectRef: null,
          checkpointDigest: null,
          evidenceRefs: ["evidence.ide13.quiesce"],
        };
      },
    };
    const worker = new PylonPortablePhaseWorker({
      client,
      executor,
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
      leaseDurationMs: 30_000,
      renewalIntervalMs: 1_000,
      waitForRenewal: async () => {
        if (!renewGateResolved) {
          renewGateResolved = true;
          releaseExecution();
          return "renew";
        }
        return new Promise(() => undefined);
      },
    });
    expect(await worker.runPass()).toBe(1);
    expect(calls.claims).toHaveLength(1);
    expect(calls.renewals).toHaveLength(1);
    expect(calls.renewals.at(0)?.expectedLeaseRevision).toBe(1);
    expect(calls.completions).toHaveLength(1);
    expect(calls.completions[0]).toMatchObject({
      resultStatus: "completed",
      expectedLeaseRevision: 2,
      evidenceRefs: ["evidence.ide13.quiesce"],
      checkpointRef: null,
      errorRef: null,
    });
    expect(JSON.stringify(calls.completions[0])).not.toMatch(
      /\/Users\/|bearer|private-agent-token/i,
    );
  });

  test("reports an unsupported local phase with a public-safe error ref", async () => {
    const source = record("source-cleanup");
    const { calls, client } = fakeClient(source);
    const worker = new PylonPortablePhaseWorker({
      client,
      executor: makePylonPortablePhaseExecutor({ resolve: async () => undefined }),
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    await worker.runPass();
    expect(calls.completions).toHaveLength(1);
    expect(calls.completions[0]).toMatchObject({
      resultStatus: "failed",
      errorRef: "error.pylon.portable-phase.unsupported-source-cleanup",
      evidenceRefs: [],
    });
  });

  test("keeps an interrupted claimed operation uncertain and does not report false failure", async () => {
    const source = record();
    const { calls, client } = fakeClient(source);
    const controller = new AbortController();
    let executing!: () => void;
    const started = new Promise<void>((resolve) => {
      executing = resolve;
    });
    const worker = new PylonPortablePhaseWorker({
      client,
      executor: {
        execute: async (_request, signal) => {
          executing();
          return new Promise((_resolve, reject) =>
            signal.addEventListener("abort", () => reject(signal.reason), { once: true }),
          );
        },
      },
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    const pass = worker.runPass(controller.signal);
    await started;
    controller.abort(new Error("operator stop"));
    expect(await pass).toBe(1);
    expect(calls.completions).toHaveLength(0);
    expect(worker.uncertainOperationRefs()).toEqual([source.request.operationRef]);
  });

  test("retries an identical completion after a lost acknowledgement without re-execution", async () => {
    const source = record();
    const fixture = fakeClient(source);
    let executions = 0;
    let completionAttempts = 0;
    const attempted: PortablePhaseOperationResultRequest[] = [];
    const originalComplete = fixture.client.complete;
    const client: PylonPortablePhaseOperationClient = {
      ...fixture.client,
      complete: async (request, signal) => {
        attempted.push(request);
        completionAttempts += 1;
        if (completionAttempts === 1) throw new PylonPortablePhaseTransportError("network_failed");
        return originalComplete(request, signal);
      },
    };
    const worker = new PylonPortablePhaseWorker({
      client,
      executor: {
        execute: async () => {
          executions += 1;
          return {
            checkpointRef: null,
            checkpointObjectRef: null,
            checkpointDigest: null,
            evidenceRefs: [],
          };
        },
      },
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    await expect(worker.runPass()).rejects.toMatchObject({ failure: "network_failed" });
    await worker.runPass();
    expect(executions).toBe(1);
    expect(attempted).toHaveLength(2);
    expect(attempted[1]).toEqual(attempted[0]);
    expect(fixture.calls.completions).toHaveLength(1);
    expect(fixture.calls.completions[0]).toEqual(attempted[0]);
  });
});
