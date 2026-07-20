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
import type {
  PylonPortablePhaseClaimJournal,
  PylonPortablePhaseClaimJournalEntry,
} from "./portable-phase-operation-claim-journal.js";
import {
  makePylonPortablePhaseExecutor,
  PylonPortablePhaseWorker,
  type PylonPortablePhaseExecutor,
} from "./portable-phase-operation-worker.js";
import type { PylonOwnerLocalExecutionTarget } from "./portable-session-target.js";

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
  resultDestinationActivationReceipt: null,
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

const memoryJournal = (
  initial: ReadonlyArray<PylonPortablePhaseClaimJournalEntry> = [],
): PylonPortablePhaseClaimJournal => {
  const entries = new Map(
    initial.map((entry) => [entry.record.request.operationRef, entry] as const),
  );
  return {
    entries: async () => [...entries.values()],
    put: async (entry) => {
      entries.set(entry.record.request.operationRef, entry);
    },
    remove: async (operationRef) => {
      entries.delete(operationRef);
    },
  };
};

const crashAfterPut = (
  journal: PylonPortablePhaseClaimJournal,
  predicate: (entry: PylonPortablePhaseClaimJournalEntry) => boolean,
): PylonPortablePhaseClaimJournal => {
  let crashed = false;
  return {
    entries: journal.entries,
    remove: journal.remove,
    put: async (entry) => {
      await journal.put(entry);
      if (!crashed && predicate(entry)) {
        crashed = true;
        throw new Error("simulated process crash");
      }
    },
  };
};

const fakeClient = (source: PortablePhaseOperationRecord) => {
  const calls: {
    claims: PortablePhaseOperationClaimRequest[];
    renewals: PortablePhaseOperationRenewRequest[];
    completions: PortablePhaseOperationResultRequest[];
  } = { claims: [], renewals: [], completions: [] };
  let current = source;
  const client: PylonPortablePhaseOperationClient = {
    pending: async () => (current.state === "pending" ? [current] : []),
    read: async () => current,
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
        resultDestinationActivationReceipt: request.destinationActivationReceipt,
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
        if (new URL(String(url)).pathname.includes("/reconcile/")) {
          return Response.json({
            schema: "openagents.portable_phase_operation_transport.v1",
            operation: source,
            status: "reconciled",
          });
        }
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
    expect(await client.read(source.request.operationRef)).toEqual(source);
    expect(new URL(calls.at(1)?.url ?? "https://invalid.test").pathname).toBe(
      `/api/pylons/${pylonRef}/portable-targets/${targetRef}/phase-operations/reconcile/${source.request.operationRef}`,
    );

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
  test("carries the exact destination readiness receipt from the target", async () => {
    const request = {
      ...record("destination-activate").request,
      operationRef: "operation.ide13.destination.activate",
      attachmentRef: "attachment.ide13.destination",
      attachmentGeneration: 2,
      checkpointRef: "checkpoint.ide13.destination",
      checkpointObjectRef: "object.ide13.destination",
      checkpointDigest: `sha256:${"a".repeat(64)}`,
    };
    const receipt = {
      schema: "openagents.ide_portable_destination_activation.v1" as const,
      receiptRef: "receipt.ide13.destination.activation",
      operationRef: request.operationRef,
      sessionRef: request.sessionRef,
      checkpointRef: request.checkpointRef,
      destinationTargetRef: targetRef,
      destinationAttachmentRef: request.attachmentRef,
      destinationGeneration: request.attachmentGeneration,
      authentication: {
        state: "reauthenticated" as const,
        policyRef: "policy.portable.destination.owner_local.v1",
        evidenceRef: "evidence.ide13.destination.authentication",
        observedAt: now.toISOString(),
        expiresAt: "2026-07-20T12:05:00.000Z",
      },
      helpers: (["pty", "lsp", "dap", "watcher", "native"] as const).map((kind) => ({
        kind,
        readiness: "unsupported" as const,
        instanceRef: null,
        versionRef: null,
        omissionRef: `omission.ide13.destination.${kind}`,
        evidenceRefs: [],
      })),
      activatedAgentRefs: ["agent.ide13.destination.root"],
      acceptedWorkRefs: [],
      evidenceRefs: ["evidence.ide13.destination.activation"],
    };
    const unsupported = async (): Promise<never> => {
      throw new Error("unexpected target call");
    };
    const target: PylonOwnerLocalExecutionTarget = {
      targetRef,
      targetClass: "owner_local",
      quiesceGraph: unsupported,
      createCheckpoint: unsupported,
      cleanupSource: unsupported,
      stageCheckpoint: unsupported,
      activate: async () => receipt,
      abortStaged: unsupported,
    };
    const executor = makePylonPortablePhaseExecutor({
      resolve: async () => ({
        target,
        call: {
          kind: "destination-activate",
          input: {
            operationRef: request.operationRef,
            checkpointRef: request.checkpointRef,
            sessionRef: request.sessionRef,
            executionBinding: {
              schema: "openagents.portable_session_execution_binding.v1",
              sessionRef: request.sessionRef,
              ownerRef: request.ownerRef,
              runRef: "run.ide13.destination",
              repositoryRef: "repository.ide13.destination",
              pinnedBaseRef: "commit.ide13.destination",
            },
            destinationAttachmentRef: request.attachmentRef,
            destinationGeneration: request.attachmentGeneration,
            capabilityLeaseRefs: [],
          },
        },
        operationRefSemantics: "operation_ref_idempotent",
      }),
    });
    await expect(executor.execute(request, new AbortController().signal)).resolves.toEqual({
      checkpointRef: null,
      checkpointObjectRef: null,
      checkpointDigest: null,
      destinationActivationReceipt: receipt,
      evidenceRefs: receipt.evidenceRefs,
    });
    const { calls, client } = fakeClient({
      ...record("destination-activate"),
      request,
    });
    const worker = new PylonPortablePhaseWorker({
      client,
      executor,
      journal: memoryJournal(),
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    expect(await worker.runPass()).toBe(1);
    expect(calls.completions[0]?.destinationActivationReceipt).toEqual(receipt);
  });

  test("claims, renews with CAS, and completes an exact refs-only result", async () => {
    const source = record();
    const { calls, client } = fakeClient(source);
    let renewGateResolved = false;
    let releaseExecution!: () => void;
    const executionGate = new Promise<void>((resolve) => {
      releaseExecution = resolve;
    });
    const executor: PylonPortablePhaseExecutor = {
      recoverySemantics: async () => "operation_ref_idempotent",
      execute: async () => {
        await executionGate;
        return {
          checkpointRef: null,
          checkpointObjectRef: null,
          checkpointDigest: null,
          destinationActivationReceipt: null,
          evidenceRefs: ["evidence.ide13.quiesce"],
        };
      },
    };
    const journal = memoryJournal();
    const worker = new PylonPortablePhaseWorker({
      client,
      executor,
      journal,
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
      journal: memoryJournal(),
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
        recoverySemantics: async () => "not_proven",
        execute: async (_request, signal) => {
          executing();
          return new Promise((_resolve, reject) =>
            signal.addEventListener("abort", () => reject(signal.reason), { once: true }),
          );
        },
      },
      journal: memoryJournal(),
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

  test("recovers a crash after claim by reconciling and renewing the same worker claim", async () => {
    const source = record();
    const fixture = fakeClient(source);
    const journal = memoryJournal();
    let executions = 0;
    const executor: PylonPortablePhaseExecutor = {
      recoverySemantics: async () => "operation_ref_idempotent",
      execute: async () => {
        executions += 1;
        return {
          checkpointRef: null,
          checkpointObjectRef: null,
          checkpointDigest: null,
          destinationActivationReceipt: null,
          evidenceRefs: [],
        };
      },
    };
    const first = new PylonPortablePhaseWorker({
      client: fixture.client,
      executor,
      journal: crashAfterPut(
        journal,
        (entry) => entry.state === "claimed" && entry.leaseRevision === 1,
      ),
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    await expect(first.runPass()).rejects.toThrow("simulated process crash");
    expect(executions).toBe(0);

    const restarted = new PylonPortablePhaseWorker({
      client: fixture.client,
      executor,
      journal,
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    await restarted.runPass();
    expect(fixture.calls.claims).toHaveLength(1);
    expect(fixture.calls.renewals).toHaveLength(1);
    expect(fixture.calls.renewals[0]).toMatchObject({
      claimRef: fixture.calls.claims[0]?.claimRef,
      workerInstanceRef,
      expectedLeaseRevision: 1,
    });
    expect(executions).toBe(1);
  });

  test("recovers a crash after renewal without accepting lease revision drift", async () => {
    const source = record();
    const fixture = fakeClient(source);
    const journal = memoryJournal();
    const never = new Promise<never>(() => undefined);
    const first = new PylonPortablePhaseWorker({
      client: fixture.client,
      executor: {
        recoverySemantics: async () => "operation_ref_idempotent",
        execute: async () => never,
      },
      journal: crashAfterPut(
        journal,
        (entry) => entry.state === "executing" && entry.leaseRevision === 2,
      ),
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
      waitForRenewal: async () => "renew",
    });
    await expect(first.runPass()).rejects.toThrow("simulated process crash");
    expect(fixture.calls.renewals).toHaveLength(1);

    let executions = 0;
    const restarted = new PylonPortablePhaseWorker({
      client: fixture.client,
      executor: {
        recoverySemantics: async () => "operation_ref_idempotent",
        execute: async () => {
          executions += 1;
          return {
            checkpointRef: null,
            checkpointObjectRef: null,
            checkpointDigest: null,
            destinationActivationReceipt: null,
            evidenceRefs: [],
          };
        },
      },
      journal,
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    await restarted.runPass();
    expect(fixture.calls.renewals.map((renewal) => renewal.expectedLeaseRevision)).toEqual([1, 2]);
    expect(executions).toBe(1);
  });

  test("re-executes uncertain work only with proven operation-ref idempotency", async () => {
    const source = record();
    const fixture = fakeClient(source);
    const journal = memoryJournal();
    const controller = new AbortController();
    let started!: () => void;
    const executing = new Promise<void>((resolve) => {
      started = resolve;
    });
    const first = new PylonPortablePhaseWorker({
      client: fixture.client,
      executor: {
        recoverySemantics: async () => "not_proven",
        execute: async (_request, signal) => {
          started();
          return new Promise((_resolve, reject) =>
            signal.addEventListener("abort", () => reject(signal.reason), { once: true }),
          );
        },
      },
      journal,
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    const pass = first.runPass(controller.signal);
    await executing;
    controller.abort(new Error("simulated crash during execution"));
    await pass;

    const denied = new PylonPortablePhaseWorker({
      client: fixture.client,
      executor: {
        recoverySemantics: async () => "not_proven",
        execute: async () => {
          throw new Error("must not execute");
        },
      },
      journal,
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    await expect(denied.runPass()).rejects.toMatchObject({
      reason: "non_idempotent_uncertain",
    });

    let recoveredExecutions = 0;
    const admitted = new PylonPortablePhaseWorker({
      client: fixture.client,
      executor: {
        recoverySemantics: async () => "operation_ref_idempotent",
        execute: async () => {
          recoveredExecutions += 1;
          return {
            checkpointRef: null,
            checkpointObjectRef: null,
            checkpointDigest: null,
            destinationActivationReceipt: null,
            evidenceRefs: [],
          };
        },
      },
      journal,
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    await admitted.runPass();
    expect(recoveredExecutions).toBe(1);
  });

  test("retries an identical completion after a lost acknowledgement without re-execution", async () => {
    const source = record();
    const fixture = fakeClient(source);
    const journal = memoryJournal();
    let executions = 0;
    let completionAttempts = 0;
    const attempted: PortablePhaseOperationResultRequest[] = [];
    const originalComplete = fixture.client.complete;
    const client: PylonPortablePhaseOperationClient = {
      ...fixture.client,
      complete: async (request, signal) => {
        attempted.push(request);
        completionAttempts += 1;
        if (completionAttempts === 1) {
          await originalComplete(request, signal);
          throw new PylonPortablePhaseTransportError("network_failed");
        }
        return originalComplete(request, signal);
      },
    };
    const worker = new PylonPortablePhaseWorker({
      client,
      executor: {
        recoverySemantics: async () => "operation_ref_idempotent",
        execute: async () => {
          executions += 1;
          return {
            checkpointRef: null,
            checkpointObjectRef: null,
            checkpointDigest: null,
            destinationActivationReceipt: null,
            evidenceRefs: [],
          };
        },
      },
      journal,
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    await expect(worker.runPass()).rejects.toMatchObject({ failure: "network_failed" });
    const restarted = new PylonPortablePhaseWorker({
      client,
      executor: {
        recoverySemantics: async () => "operation_ref_idempotent",
        execute: async () => {
          executions += 1;
          return {
            checkpointRef: null,
            checkpointObjectRef: null,
            checkpointDigest: null,
            destinationActivationReceipt: null,
            evidenceRefs: [],
          };
        },
      },
      journal,
      pylonRef,
      targetRef,
      workerInstanceRef,
      now: () => now,
    });
    await restarted.runPass();
    expect(executions).toBe(1);
    expect(attempted).toHaveLength(2);
    expect(attempted[1]).toEqual(attempted[0]);
    expect(fixture.calls.completions).toHaveLength(2);
    expect(fixture.calls.completions).toEqual(attempted);
  });
});
