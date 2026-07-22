import { canonicalJson } from "@openagentsinc/khala-sync";
import type { PylonPortableCheckpointBundle } from "@openagentsinc/portable-session-contract";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import type { IncomingMessage, ServerResponse } from "node:http";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Effect } from "effect";
import { afterEach, expect, test } from "vite-plus/test";

import { makeAuthenticatedHttpClient, runLiveRetainedProof } from "./live-retained-proof.js";
import type { LiveRetainedProofPlan, ProofArtifactProducer } from "./live-retained-proof.js";

const roots: string[] = [];
const servers: Array<ReturnType<typeof createServer>> = [];
const sha = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const readBody = async (request: IncomingMessage): Promise<Uint8Array> => {
  const chunks: Uint8Array[] = [];
  for await (const chunk of request) chunks.push(Uint8Array.from(chunk as Uint8Array));
  return Buffer.concat(chunks);
};

const json = (response: ServerResponse, status: number, value: unknown): void => {
  const bytes = Buffer.from(JSON.stringify(value));
  response.writeHead(status, {
    "content-type": "application/json",
    "content-length": bytes.byteLength,
  });
  response.end(bytes);
};

type FakeControl = Readonly<{
  baseUrl: string;
  actions: string[];
  privateBodies: string[];
  close: () => Promise<void>;
}>;

const fakeControl = async (
  options: Readonly<{
    readiness?: "live" | "fake";
    failContinuation?: boolean;
  }> = {},
): Promise<FakeControl> => {
  const operationBodies = new Map<string, string>();
  const operationResponses = new Map<string, unknown>();
  const continuationBodies = new Map<string, string>();
  const exportScopes = new Map<string, string>();
  const binaryRequests = new Map<string, string>();
  const binaryResponses = new Map<string, unknown>();
  const actions: string[] = [];
  const privateBodies: string[] = [];
  const exported = new TextEncoder().encode("private-exported-checkpoint");
  const exportedDigest = sha(exported);
  const server = createServer(async (request, response) => {
    if (request.headers.authorization !== "Bearer control-secret") {
      return json(response, 401, { error: "unauthorized" });
    }
    const body = await readBody(request);
    if (request.method === "GET" && request.url === "/v1/cloud-vm/readiness") {
      return json(response, 200, {
        contractVersion: "openagents.agent_computer_readiness.v1",
        ready: options.readiness !== "fake",
        provisionerKind: options.readiness ?? "live",
      });
    }
    if (request.url === "/v1/portable-agent-computers/checkpoints/materialize") {
      const operation = String(request.headers["x-oa-operation-ref"]);
      const key = `${request.url}:${operation}`;
      const fingerprint = `${request.headers["x-oa-artifact-ref"]}:${request.headers["x-oa-artifact-digest"]}:${sha(body)}`;
      const existing = binaryRequests.get(key);
      if (existing !== undefined && existing !== fingerprint)
        return json(response, 400, { error: "replay_conflict" });
      if (existing !== undefined) return json(response, 200, binaryResponses.get(key));
      privateBodies.push(new TextDecoder().decode(body));
      const value = { acceptingWork: false, evidenceRefs: ["evidence.fake.materialized"] };
      binaryRequests.set(key, fingerprint);
      binaryResponses.set(key, value);
      return json(response, 200, value);
    }
    if (request.url === "/v1/portable-agent-computers/capabilities/install") {
      const operation = String(request.headers["x-oa-operation-ref"]);
      const key = `${request.url}:${operation}`;
      const fingerprint = `${request.headers["x-oa-lease-ref"]}:${request.headers["x-oa-evidence-ref"]}:${sha(body)}`;
      const existing = binaryRequests.get(key);
      if (existing !== undefined && existing !== fingerprint)
        return json(response, 400, { error: "replay_conflict" });
      if (existing !== undefined) return json(response, 200, binaryResponses.get(key));
      privateBodies.push(new TextDecoder().decode(body));
      const value = {
        material: "excluded",
        marker: { leaseRef: request.headers["x-oa-lease-ref"] },
        evidenceRefs: ["evidence.fake.capability"],
      };
      binaryRequests.set(key, fingerprint);
      binaryResponses.set(key, value);
      return json(response, 200, value);
    }
    if (request.url === "/v1/portable-agent-computers/continuations") {
      const text = new TextDecoder().decode(body);
      const parsed = JSON.parse(text) as Record<string, unknown>;
      const ref = String(parsed.operationRef);
      const existing = continuationBodies.get(ref);
      if (existing !== undefined && existing !== text)
        return json(response, 400, { error: "replay_conflict" });
      if (options.failContinuation) return json(response, 500, { error: "injected" });
      const replay = existing === undefined ? "executed" : "replayed";
      continuationBodies.set(ref, text);
      privateBodies.push(text);
      const expected = parsed.expectedThreadCursors as Array<Record<string, unknown>>;
      const turns = parsed.turns as Array<Record<string, unknown>>;
      return json(response, 200, {
        replay,
        acceptedWorkRefs: turns.map((turn) => ({ agentRef: turn.agentRef, turnRef: turn.turnRef })),
        threadCursors: expected.map((cursor) => ({
          agentRef: cursor.agentRef,
          threadRef: cursor.threadRef,
          transcriptRef: `transcript.${String(cursor.agentRef)}`,
          activityCursor: Number(cursor.activityCursor) + 1,
          eventCursor: Number(cursor.eventCursor) + 1,
        })),
        evidenceRefs: ["evidence.fake.continuation"],
        material: "excluded",
      });
    }
    if (request.url === "/v1/portable-agent-computers/checkpoints/export") {
      const operation = String(request.headers["x-oa-operation-ref"]);
      const checkpoint = String(request.headers["x-oa-checkpoint-ref"]);
      const existing = exportScopes.get(operation);
      if (existing !== undefined && existing !== checkpoint)
        return json(response, 400, { error: "replay_conflict" });
      exportScopes.set(operation, checkpoint);
      response.writeHead(200, {
        "content-type": "application/octet-stream",
        "content-length": exported.byteLength,
        "x-oa-artifact-ref": "artifact.fake.exported",
        "x-oa-artifact-digest": exportedDigest,
      });
      return response.end(exported);
    }
    if (request.url !== "/v1/portable-agent-computers/operations") {
      return json(response, 404, { error: "not_found" });
    }
    const text = new TextDecoder().decode(body);
    const parsed = JSON.parse(text) as Record<string, unknown>;
    const ref = String(parsed.operationRef);
    const action = String(parsed.action);
    actions.push(action);
    const existing = operationBodies.get(ref);
    if (existing !== undefined && existing !== text)
      return json(response, 400, { error: "replay_conflict" });
    if (existing !== undefined) return json(response, 200, operationResponses.get(ref));
    operationBodies.set(ref, text);
    const payload = parsed.payload as Record<string, unknown>;
    const graph =
      action === "stage"
        ? ((payload.bundle as Record<string, unknown>).graph as Record<string, unknown>)
        : (payload.graph as Record<string, unknown> | undefined);
    const nodes = (graph?.nodes ?? []) as Array<Record<string, unknown>>;
    const responses: Record<string, unknown> = {
      stage: {
        resourceRef: "resource.fake.dynamic",
        destinationRunnerSessionReservationRef: "runner-session-reservation.fake.dynamic",
        acceptingWork: false,
        evidenceRefs: ["evidence.fake.stage"],
      },
      activate: {
        activatedAgentRefs: ["agent.proof.root"],
        helpers: ["pty", "lsp", "dap", "watcher", "native"].map((kind) => ({
          kind,
          readiness: kind === "watcher" ? "ready" : "unsupported",
          instanceRef: kind === "watcher" ? "instance.fake.watcher" : null,
          versionRef: kind === "watcher" ? "version.fake.watcher" : null,
          omissionRef: kind === "watcher" ? null : `omission.fake.${kind}`,
          evidenceRefs: kind === "watcher" ? ["evidence.fake.watcher"] : [],
        })),
        evidenceRefs: ["evidence.fake.activate"],
      },
      quiesce: {
        quiescedAgentRefs: nodes.map((node) => node.agentRef),
        evidenceRefs: ["evidence.fake.quiesce"],
      },
      checkpoint: {
        checkpoint: { checkpointRef: payload.checkpointRef },
        evidenceRefs: ["evidence.fake.checkpoint"],
      },
      reclaim: {
        cleanedAgentRefs: payload.agentRefs,
        processes: "released",
        scratch: "released",
        ports: "released",
        evidenceRefs: ["evidence.fake.reclaim"],
      },
      abortPrepared: { material: "excluded", evidenceRefs: ["evidence.fake.abort-prepared"] },
    };
    const value = responses[action];
    if (value === undefined) return json(response, 400, { error: "unknown_action" });
    operationResponses.set(ref, value);
    return json(response, 200, value);
  });
  servers.push(server);
  await new Promise<void>((resolvePromise) => server.listen(0, "127.0.0.1", resolvePromise));
  const address = server.address();
  if (address === null || typeof address === "string") throw new Error("fake control did not bind");
  return {
    baseUrl: `http://127.0.0.1:${address.port}`,
    actions,
    privateBodies,
    close: () =>
      new Promise((resolvePromise, reject) =>
        server.close((error) => (error ? reject(error) : resolvePromise())),
      ),
  };
};

const fixture = async (
  baseUrl: string,
): Promise<
  Readonly<{
    plan: LiveRetainedProofPlan;
    artifactProducer: ProofArtifactProducer;
    artifactCalls: { count: number };
  }>
> => {
  const root = await mkdtemp(join(tmpdir(), "oa-live-retained-proof-"));
  roots.push(root);
  const rootfsPath = join(root, "rootfs.ext4");
  const kernelPath = join(root, "vmlinux");
  const controllerPath = join(root, "portable-session-control");
  await writeFile(rootfsPath, "rootfs-candidate");
  await writeFile(kernelPath, "kernel-candidate");
  await writeFile(controllerPath, "controller-candidate");
  const graph = {
    rootAgentRef: "agent.proof.root",
    nodes: [
      {
        agentRef: "agent.proof.root",
        threadRef: "thread.proof.root",
        transcriptRef: "transcript.proof.root",
        activityCursor: 4,
        lifecycle: "quiesced" as const,
        attachmentGeneration: 1,
      },
    ],
  };
  const checkpointPayload = {
    schema: "openagents.portable_checkpoint.v1" as const,
    checkpointRef: "checkpoint.proof.source",
    sessionRef: "session.proof.retained",
    sourceAttachmentRef: "attachment.proof.source",
    sourceGeneration: 1,
    repositoryRef: "repository.OpenAgentsInc.openagents",
    repositoryRevisionRef: "9b6b93da465a33cc0f4bfb9debaf96db32394799",
    repositoryPostImageDigest: sha("post-image"),
    diffDigest: sha("diff"),
    eventLogCursor: 8,
    catalogGenerationRef: "catalog.proof.1",
    graphDigest: sha(canonicalJson(graph)),
    approvalRefs: [],
    artifactRefs: [],
    receiptRefs: ["receipt.proof.source"],
    secretMaterial: "excluded" as const,
    processState: "excluded" as const,
  };
  const bundle: PylonPortableCheckpointBundle = {
    checkpoint: { ...checkpointPayload, digest: sha(canonicalJson(checkpointPayload)) },
    executionBinding: {
      schema: "openagents.portable_session_execution_binding.v1",
      sessionRef: checkpointPayload.sessionRef,
      ownerRef: "owner.proof.retained",
      runRef: "run.proof.retained",
      repositoryRef: checkpointPayload.repositoryRef,
      pinnedBaseRef: checkpointPayload.repositoryRevisionRef,
    },
    graph,
    threadCursors: [
      {
        threadRef: "thread.proof.root",
        transcriptRef: "transcript.proof.root",
        activityCursor: 4,
        eventCursor: 8,
      },
    ],
  };
  const artifactBytes = new TextEncoder().encode("private-checkpoint-archive");
  const artifactCalls = { count: 0 };
  return {
    plan: {
      schema: "openagents.agent_computer_live_retained_proof_plan.v1",
      proofRef: "proof.agent-computer.retained.1",
      integrationRevision: "9b6b93da465a33cc0f4bfb9debaf96db32394799",
      controlBaseUrl: baseUrl,
      candidate: {
        rootfsPath,
        rootfsDigest: sha("rootfs-candidate"),
        kernelPath,
        kernelDigest: sha("kernel-candidate"),
        portableSessionControlPath: controllerPath,
        portableSessionControlDigest: sha("controller-candidate"),
        runtimeDirectory: root,
      },
      sourceWorkingDirectory: root,
      ownerRef: "owner.proof.retained",
      targetRef: "target.proof.managed",
      sessionRef: "session.proof.retained",
      attachmentRef: "attachment.proof.managed",
      generation: 2,
      bundle,
      providerLeaseRef: "lease.proof.provider",
      providerEvidenceRef: "evidence.proof.provider",
      authorityEvidenceRef: "evidence.proof.authority",
      authenticationPolicyRef: "policy.portable.destination.openagents_managed.v1",
      continuationTurns: [
        {
          agentRef: "agent.proof.root",
          turnRef: "turn.proof.root",
          task: "PRIVATE-CONTINUATION-TASK-SENTINEL",
        },
      ],
    },
    artifactCalls,
    artifactProducer: {
      produce: async () => {
        artifactCalls.count += 1;
        return {
          artifactRef: "artifact.proof.source",
          digest: sha(artifactBytes),
          bytes: Uint8Array.from(artifactBytes),
        };
      },
    },
  };
};

const cleanAudit = async () => ({
  jailDirectoryAbsent: true,
  tapDeviceAbsent: true,
  firecrackerProcessAbsent: true,
});

afterEach(async () => {
  await Promise.all(
    servers.splice(0).map(
      (server) =>
        new Promise<void>((resolvePromise) => {
          if (!server.listening) return resolvePromise();
          server.close(() => resolvePromise());
        }),
    ),
  );
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

test("runs the exact retained lifecycle and emits a refs-only receipt", async () => {
  const control = await fakeControl();
  const source = await fixture(control.baseUrl);
  const capability = new TextEncoder().encode("CAPABILITY-MATERIAL-SENTINEL");
  const receipt = await Effect.runPromise(
    runLiveRetainedProof(source.plan, capability, {
      http: makeAuthenticatedHttpClient(control.baseUrl, "control-secret"),
      artifactProducer: source.artifactProducer,
      cleanupAuditor: cleanAudit,
      now: () => new Date("2026-07-20T20:00:00.000Z"),
    }),
  );

  expect(source.artifactCalls.count).toBe(1);
  expect([...capability].every((byte) => byte === 0)).toBe(true);
  expect(receipt.cleanup).toEqual({
    jailDirectoryAbsent: true,
    tapDeviceAbsent: true,
    firecrackerProcessAbsent: true,
    reclaimReplayAccepted: true,
  });
  expect(receipt.replayChecks).toEqual({
    stageIdentical: true,
    stageConflictRejected: true,
    materializeIdentical: true,
    materializeConflictRejected: true,
    capabilityIdentical: true,
    capabilityConflictRejected: true,
    activateIdentical: true,
    activateConflictRejected: true,
    continuationIdentical: true,
    continuationConflictRejected: true,
    quiesceIdentical: true,
    quiesceConflictRejected: true,
    checkpointIdentical: true,
    checkpointConflictRejected: true,
    exportIdentical: true,
    exportConflictRejected: true,
    reclaimIdentical: true,
    reclaimConflictRejected: true,
  });
  const encoded = JSON.stringify(receipt);
  expect(encoded).not.toContain("CAPABILITY-MATERIAL-SENTINEL");
  expect(encoded).not.toContain("PRIVATE-CONTINUATION-TASK-SENTINEL");
  expect(encoded).not.toContain("control-secret");
  expect(encoded).not.toContain(source.plan.sourceWorkingDirectory);
});

test("refuses a non-live daemon before staging and zeroizes capability material", async () => {
  const control = await fakeControl({ readiness: "fake" });
  const source = await fixture(control.baseUrl);
  const capability = new TextEncoder().encode("CAPABILITY-MATERIAL-SENTINEL");
  await expect(
    Effect.runPromise(
      runLiveRetainedProof(source.plan, capability, {
        http: makeAuthenticatedHttpClient(control.baseUrl, "control-secret"),
        artifactProducer: source.artifactProducer,
        cleanupAuditor: cleanAudit,
      }),
    ),
  ).rejects.toThrow("control daemon is not armed with the live provisioner");
  expect(control.actions).toEqual([]);
  expect(source.artifactCalls.count).toBe(0);
  expect([...capability].every((byte) => byte === 0)).toBe(true);
});

test("reclaims an activated resource when continuation fails", async () => {
  const control = await fakeControl({ failContinuation: true });
  const source = await fixture(control.baseUrl);
  const capability = new TextEncoder().encode("CAPABILITY-MATERIAL-SENTINEL");
  await expect(
    Effect.runPromise(
      runLiveRetainedProof(source.plan, capability, {
        http: makeAuthenticatedHttpClient(control.baseUrl, "control-secret"),
        artifactProducer: source.artifactProducer,
        cleanupAuditor: cleanAudit,
      }),
    ),
  ).rejects.toThrow("continuation returned HTTP 500");
  expect(control.actions.slice(-2)).toEqual(["quiesce", "reclaim"]);
  expect([...capability].every((byte) => byte === 0)).toBe(true);
});

test("aborts a prepared resource when the stage acknowledgement is lost", async () => {
  const control = await fakeControl();
  const source = await fixture(control.baseUrl);
  const capability = new TextEncoder().encode("CAPABILITY-MATERIAL-SENTINEL");
  const liveHttp = makeAuthenticatedHttpClient(control.baseUrl, "control-secret");
  let loseStageAcknowledgement = true;
  await expect(
    Effect.runPromise(
      runLiveRetainedProof(source.plan, capability, {
        http: async (request) => {
          const response = await liveHttp(request);
          if (
            loseStageAcknowledgement &&
            request.path === "/v1/portable-agent-computers/operations" &&
            new TextDecoder().decode(request.body ?? new Uint8Array()).includes('"action":"stage"')
          ) {
            loseStageAcknowledgement = false;
            throw new Error("injected lost stage acknowledgement");
          }
          return response;
        },
        artifactProducer: source.artifactProducer,
        cleanupAuditor: cleanAudit,
      }),
    ),
  ).rejects.toThrow("live retained proof failed");
  expect(control.actions.slice(-1)).toEqual(["abortPrepared"]);
  expect(source.artifactCalls.count).toBe(0);
  expect([...capability].every((byte) => byte === 0)).toBe(true);
});

test("refuses a mismatched candidate before the live readiness request", async () => {
  const control = await fakeControl();
  const source = await fixture(control.baseUrl);
  const capability = new TextEncoder().encode("CAPABILITY-MATERIAL-SENTINEL");
  const plan = {
    ...source.plan,
    candidate: { ...source.plan.candidate, rootfsDigest: sha("wrong") },
  };
  await expect(
    Effect.runPromise(
      runLiveRetainedProof(plan, capability, {
        http: makeAuthenticatedHttpClient(control.baseUrl, "control-secret"),
        artifactProducer: source.artifactProducer,
        cleanupAuditor: cleanAudit,
      }),
    ),
  ).rejects.toThrow("rootfs candidate digest does not match");
  expect(control.actions).toEqual([]);
  expect([...capability].every((byte) => byte === 0)).toBe(true);
});
