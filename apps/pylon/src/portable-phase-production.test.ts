import { mkdtemp, readdir, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type {
  PortablePhaseOperationClaimRequest,
  PortablePhaseOperationRecord,
  PortablePhaseOperationResultRequest,
} from "@openagentsinc/portable-session-contract";
import { describe, expect, test } from "vite-plus/test";
import type { PylonOwnerLocalExecutionTarget } from "./portable-session-target.js";
import {
  assertPortablePhasePendingSupported,
  makePylonPrivatePortablePhaseContextResolver,
  openPylonPortablePhaseProductionWorker,
  portablePhaseWorkerInstanceRef,
  PylonPortablePhaseProductionError,
} from "./portable-phase-production.js";

const pylonRef = "pylon.ide13.production";
const targetRef = "target.ide13.production";
const now = "2026-07-20T12:00:00.000Z";

const pendingRecord = (): PortablePhaseOperationRecord => ({
  request: {
    schema: "openagents.portable_phase_operation.v1",
    operationRef: "operation.ide13.production.quiesce",
    commandRef: "command.ide13.production",
    commandExecutionClaimRef: "claim.ide13.production.command",
    ownerRef: "owner.ide13.production",
    sessionRef: "session.ide13.production",
    attachmentRef: "attachment.ide13.production",
    attachmentGeneration: 1,
    targetRef,
    pylonRef,
    kind: "quiesce",
    checkpointRef: null,
    checkpointObjectRef: null,
    checkpointDigest: null,
    evidenceRefs: [],
    expiresAt: "2096-07-20T12:10:00.000Z",
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
  updatedAt: now,
});

const target = (): PylonOwnerLocalExecutionTarget => ({
  targetRef,
  targetClass: "owner_local",
  quiesceGraph: async () => ({
    quiescedAgentRefs: ["agent.ide13.production"],
    evidenceRefs: ["evidence.ide13.production.quiesced"],
  }),
  createCheckpoint: async () => {
    throw new Error("not used");
  },
  cleanupSource: async () => {
    throw new Error("not used");
  },
  stageCheckpoint: async () => {
    throw new Error("not used");
  },
  activate: async () => {
    throw new Error("not used");
  },
  abortStaged: async () => {
    throw new Error("not used");
  },
});

const exactContext = (record: PortablePhaseOperationRecord) => ({
  target: target(),
  call: {
    kind: "quiesce" as const,
    input: {
      operationRef: record.request.operationRef,
      sessionRef: record.request.sessionRef,
      attachmentRef: record.request.attachmentRef,
      generation: record.request.attachmentGeneration,
      graph: {
        rootAgentRef: "agent.ide13.production",
        nodes: [
          {
            agentRef: "agent.ide13.production",
            threadRef: "thread.ide13.production",
            transcriptRef: "transcript.ide13.production",
            activityCursor: 0,
            lifecycle: "running" as const,
            attachmentGeneration: record.request.attachmentGeneration,
          },
        ],
      },
      threadCursors: [],
    },
  },
  operationRefSemantics: "operation_ref_idempotent" as const,
});

describe("Pylon portable phase production composition", () => {
  test("resolves only an admitted exact request and keeps idempotency explicit", async () => {
    const record = pendingRecord();
    const privateContexts = makePylonPrivatePortablePhaseContextResolver();
    const context = exactContext(record);
    privateContexts.admit(record.request, context);

    expect(await privateContexts.resolver.resolve(record.request)).toBe(context);
    expect(
      await privateContexts.resolver.resolve({
        ...record.request,
        attachmentGeneration: record.request.attachmentGeneration + 1,
      }),
    ).toBeUndefined();
    expect(context.operationRefSemantics).toBe("operation_ref_idempotent");
  });

  test("rejects an unsupported pending phase before a claim can start", async () => {
    const privateContexts = makePylonPrivatePortablePhaseContextResolver();
    await expect(
      assertPortablePhasePendingSupported([pendingRecord()], privateContexts.resolver),
    ).rejects.toEqual(
      new PylonPortablePhaseProductionError("error.pylon.portable-phase.unsupported-exact-context"),
    );
  });

  test("wires authenticated transport, exact refs, and a mode-0600 durable journal", async () => {
    const stateDirectory = await mkdtemp(join(tmpdir(), "pylon-phase-production-"));
    try {
      const initial = pendingRecord();
      const privateContexts = makePylonPrivatePortablePhaseContextResolver();
      privateContexts.admit(initial.request, exactContext(initial));
      let current = initial;
      let authorization = "";
      const terminalAcknowledgements: string[] = [];
      let complete!: () => void;
      const completed = new Promise<void>((resolve) => {
        complete = resolve;
      });
      const fetchImpl: typeof fetch = async (input, init) => {
        const url = new URL(String(input));
        authorization = new Headers(init?.headers).get("authorization") ?? "";
        if (init?.method === "GET") {
          return Response.json({
            schema: "openagents.portable_phase_operation_transport.v1",
            operations: current.state === "pending" ? [current] : [],
          });
        }
        if (url.pathname.endsWith("/claim")) {
          const request = JSON.parse(String(init?.body)) as PortablePhaseOperationClaimRequest;
          current = {
            ...current,
            state: "claimed",
            claimRef: request.claimRef,
            claimFingerprint: `sha256:${"2".repeat(64)}`,
            workerInstanceRef: request.workerInstanceRef,
            claimGeneration: 1,
            leaseRevision: 1,
            claimedAt: now,
            leaseExpiresAt: request.leaseExpiresAt,
          };
          return Response.json({
            schema: "openagents.portable_phase_operation_transport.v1",
            operation: current,
            status: "claimed",
          });
        }
        if (url.pathname.endsWith("/complete")) {
          const request = JSON.parse(String(init?.body)) as PortablePhaseOperationResultRequest;
          current = {
            ...current,
            state: request.resultStatus,
            leaseRevision: request.expectedLeaseRevision + 1,
            resultRef: request.resultRef,
            resultFingerprint: `sha256:${"3".repeat(64)}`,
            resultStatus: request.resultStatus,
            resultCheckpointRef: request.checkpointRef,
            resultCheckpointObjectRef: request.checkpointObjectRef,
            resultCheckpointDigest: request.checkpointDigest,
            resultDestinationActivationReceipt: request.destinationActivationReceipt,
            resultEvidenceRefs: request.evidenceRefs,
            errorRef: request.errorRef,
            completedAt: request.completedAt,
          };
          complete();
          return Response.json({
            schema: "openagents.portable_phase_operation_transport.v1",
            operation: current,
            status: request.resultStatus,
          });
        }
        throw new Error("unexpected request");
      };

      const service = await openPylonPortablePhaseProductionWorker({
        agentToken: "private-agent-token",
        baseUrl: "https://openagents.test",
        pylonRef,
        targetRef,
        workerInstanceRef: portablePhaseWorkerInstanceRef(pylonRef, targetRef),
        stateDirectory,
        resolver: privateContexts.resolver,
        fetchImpl,
        pollIntervalMs: 250,
        onTerminalAcknowledged: (operationRef) => {
          terminalAcknowledgements.push(operationRef);
        },
      });
      await completed;
      await service.close();

      expect(authorization).toBe("Bearer private-agent-token");
      expect(current.state).toBe("completed");
      expect(current.resultEvidenceRefs).toEqual(["evidence.ide13.production.quiesced"]);
      expect(terminalAcknowledgements).toEqual([initial.request.operationRef]);
      const privateDirectory = join(stateDirectory, "portable-phase");
      const journalDirectory = join(privateDirectory, "claims");
      expect((await stat(privateDirectory)).mode & 0o077).toBe(0);
      expect((await stat(journalDirectory)).mode & 0o077).toBe(0);
      const files = await readdir(journalDirectory);
      expect(files).toHaveLength(1);
      expect((await stat(join(journalDirectory, files[0]!))).mode & 0o077).toBe(0);
    } finally {
      await rm(stateDirectory, { recursive: true, force: true });
    }
  });

  test("aborts an active authenticated poll and waits for shutdown", async () => {
    const stateDirectory = await mkdtemp(join(tmpdir(), "pylon-phase-shutdown-"));
    try {
      let pollStarted!: () => void;
      const started = new Promise<void>((resolve) => {
        pollStarted = resolve;
      });
      let pollAborted = false;
      const fetchImpl: typeof fetch = async (_input, init) =>
        new Promise((_resolve, reject) => {
          pollStarted();
          const signal = init?.signal;
          const abort = () => {
            pollAborted = true;
            reject(new Error("poll aborted"));
          };
          if (signal?.aborted === true) abort();
          else signal?.addEventListener("abort", abort, { once: true });
        });
      const privateContexts = makePylonPrivatePortablePhaseContextResolver();
      const service = await openPylonPortablePhaseProductionWorker({
        agentToken: "private-agent-token",
        baseUrl: "https://openagents.test",
        pylonRef,
        targetRef,
        workerInstanceRef: portablePhaseWorkerInstanceRef(pylonRef, targetRef),
        stateDirectory,
        resolver: privateContexts.resolver,
        fetchImpl,
        pollIntervalMs: 250,
      });
      await started;
      await service.close();
      expect(pollAborted).toBe(true);
      expect(service.status()).toEqual({ state: "stopped", errorRef: null });
    } finally {
      await rm(stateDirectory, { recursive: true, force: true });
    }
  });
});
