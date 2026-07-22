import { Runtime } from "@openagentsinc/runtime-platform";
import { PylonPortableCheckpointBundleSchema } from "@openagentsinc/portable-session-contract";
import type {
  PortableCheckpointArtifact,
  PortableCheckpointArtifactResolverInput,
  PylonPortableCheckpointBundle,
} from "@openagentsinc/portable-session-contract";
import { createHash } from "node:crypto";
import { createReadStream, readFileSync } from "node:fs";
import { readFile, readdir, stat } from "node:fs/promises";
import { isAbsolute, resolve } from "node:path";
import { Config, ConfigProvider, Effect, Schema } from "effect";

import { PylonPortableCheckpointArtifactStore } from "../../src/portable-session-checkpoint-artifact.js";

const INTEGRATION_REVISION = "9b6b93da465a33cc0f4bfb9debaf96db32394799" as const;
const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u;
const SHA256 = /^sha256:[a-f0-9]{64}$/u;
const LOOPBACK = new Set(["127.0.0.1", "localhost", "[::1]"]);

type Json =
  | null
  | boolean
  | number
  | string
  | ReadonlyArray<Json>
  | { readonly [key: string]: Json };

export type LiveRetainedProofPlan = Readonly<{
  schema: "openagents.agent_computer_live_retained_proof_plan.v1";
  proofRef: string;
  integrationRevision: typeof INTEGRATION_REVISION;
  controlBaseUrl: string;
  candidate: Readonly<{
    rootfsPath: string;
    rootfsDigest: `sha256:${string}`;
    kernelPath: string;
    kernelDigest: `sha256:${string}`;
    portableSessionControlPath: string;
    portableSessionControlDigest: `sha256:${string}`;
    runtimeDirectory: string;
  }>;
  sourceWorkingDirectory: string;
  ownerRef: string;
  targetRef: string;
  sessionRef: string;
  attachmentRef: string;
  generation: number;
  bundle: PylonPortableCheckpointBundle;
  providerLeaseRef: string;
  providerEvidenceRef: string;
  authorityEvidenceRef: string;
  authenticationPolicyRef: "policy.portable.destination.openagents_managed.v1";
  continuationTurns: ReadonlyArray<
    Readonly<{
      agentRef: string;
      turnRef: string;
      task: string;
    }>
  >;
}>;

export type ProofHttpRequest = Readonly<{
  method: "GET" | "POST";
  path: string;
  headers?: Readonly<Record<string, string>>;
  body?: Uint8Array;
}>;

export type ProofHttpResponse = Readonly<{
  status: number;
  headers: Readonly<Record<string, string>>;
  body: Uint8Array;
}>;

export type ProofHttpClient = (request: ProofHttpRequest) => Promise<ProofHttpResponse>;

export type ProofArtifactProducer = Readonly<{
  produce: (
    input: PortableCheckpointArtifactResolverInput & Readonly<{ sourceWorkingDirectory: string }>,
  ) => Promise<PortableCheckpointArtifact>;
}>;

export type CleanupAudit = Readonly<{
  jailDirectoryAbsent: boolean;
  tapDeviceAbsent: boolean;
  firecrackerProcessAbsent: boolean;
}>;

export type ProofCleanupAuditor = (
  input: Readonly<{
    runtimeDirectory: string;
    stageOperationRef: string;
    targetRef: string;
  }>,
) => Promise<CleanupAudit>;

export type LiveRetainedProofReceipt = Readonly<{
  schema: "openagents.agent_computer_live_retained_proof_receipt.v1";
  receiptRef: string;
  proofRef: string;
  integrationRevision: typeof INTEGRATION_REVISION;
  candidate: Readonly<{
    rootfsDigest: `sha256:${string}`;
    kernelDigest: `sha256:${string}`;
    portableSessionControlDigest: `sha256:${string}`;
  }>;
  resourceRef: string;
  destinationRunnerSessionReservationRef: string;
  artifactRef: string;
  artifactDigest: `sha256:${string}`;
  exportedArtifactRef: string;
  exportedArtifactDigest: `sha256:${string}`;
  checkpointRef: string;
  activatedAgentRefs: ReadonlyArray<string>;
  continuedTurnRefs: ReadonlyArray<string>;
  evidenceRefs: ReadonlyArray<string>;
  replayChecks: Readonly<{
    stageIdentical: true;
    stageConflictRejected: true;
    materializeIdentical: true;
    materializeConflictRejected: true;
    capabilityIdentical: true;
    capabilityConflictRejected: true;
    activateIdentical: true;
    activateConflictRejected: true;
    continuationIdentical: true;
    continuationConflictRejected: true;
    quiesceIdentical: true;
    quiesceConflictRejected: true;
    checkpointIdentical: true;
    checkpointConflictRejected: true;
    exportIdentical: true;
    exportConflictRejected: true;
    reclaimIdentical: true;
    reclaimConflictRejected: true;
  }>;
  cleanup: CleanupAudit &
    Readonly<{
      reclaimReplayAccepted: true;
    }>;
  material: "excluded";
  privateTasks: "excluded";
  publicSafe: true;
}>;

type Dependencies = Readonly<{
  http: ProofHttpClient;
  artifactProducer?: ProofArtifactProducer;
  cleanupAuditor?: ProofCleanupAuditor;
  now?: () => Date;
}>;

class ProofFailure extends Schema.TaggedErrorClass<ProofFailure>()(
  "AgentComputerLiveRetainedProofFailure",
  {
    code: Schema.String,
    message: Schema.String,
  },
) {}

const proofFailure = (code: string, message: string): ProofFailure =>
  new ProofFailure({ code, message });

const sha256 = (value: string | Uint8Array): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(value).digest("hex")}`;

const stableRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 32)}`;

const operationRef = (plan: LiveRetainedProofPlan, step: string): string =>
  stableRef(`operation.agent-computer.live-proof.${step}`, plan.proofRef);

const parseJson = (bytes: Uint8Array, label: string): Record<string, unknown> => {
  try {
    const value: unknown = JSON.parse(new TextDecoder().decode(bytes));
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      throw new Error("response is not an object");
    }
    return value as Record<string, unknown>;
  } catch {
    throw proofFailure("invalid_response", `${label} returned invalid JSON`);
  }
};

const stringField = (value: Record<string, unknown>, name: string): string => {
  const field = value[name];
  if (typeof field !== "string" || !SAFE_REF.test(field)) {
    throw proofFailure("invalid_response", `${name} is not a public-safe ref`);
  }
  return field;
};

const stringArray = (value: unknown, label: string): ReadonlyArray<string> => {
  if (
    !Array.isArray(value) ||
    value.some((item) => typeof item !== "string" || !SAFE_REF.test(item))
  ) {
    throw proofFailure("invalid_response", `${label} is not a public-safe ref list`);
  }
  return value as ReadonlyArray<string>;
};

const acceptedTurnRefs = (value: unknown): ReadonlyArray<string> => {
  if (!Array.isArray(value))
    throw proofFailure("invalid_response", "acceptedWorkRefs is not a list");
  return value.map((item) => {
    if (typeof item !== "object" || item === null || Array.isArray(item)) {
      throw proofFailure("invalid_response", "acceptedWorkRefs contains an invalid row");
    }
    return stringField(item as Record<string, unknown>, "turnRef");
  });
};

const jsonBytes = (value: unknown): Uint8Array => new TextEncoder().encode(JSON.stringify(value));

const expectStatus = (
  response: ProofHttpResponse,
  expected: number,
  label: string,
): ProofHttpResponse => {
  if (response.status !== expected) {
    throw proofFailure("http_rejected", `${label} returned HTTP ${response.status}`);
  }
  return response;
};

const sameBytes = (left: Uint8Array, right: Uint8Array): boolean =>
  left.byteLength === right.byteLength && left.every((byte, index) => byte === right[index]);

const assertReplay = (first: ProofHttpResponse, replay: ProofHttpResponse, label: string): void => {
  expectStatus(replay, first.status, `${label} replay`);
  if (!sameBytes(first.body, replay.body)) {
    throw proofFailure("replay_mismatch", `${label} replay was not byte-identical`);
  }
};

const assertConflict = (response: ProofHttpResponse, label: string): void => {
  if (response.status < 400 || response.status >= 500) {
    throw proofFailure("replay_conflict_missing", `${label} conflicting replay was not rejected`);
  }
};

const requestJson = (
  http: ProofHttpClient,
  path: string,
  body: unknown,
): Promise<ProofHttpResponse> => {
  const bytes = jsonBytes(body);
  return http({
    method: "POST",
    path,
    headers: { "content-type": "application/json" },
    body: bytes,
  }).finally(() => bytes.fill(0));
};

const artifactHeaders = (
  plan: LiveRetainedProofPlan,
  operation: string,
  artifact: PortableCheckpointArtifact,
): Readonly<Record<string, string>> => ({
  "content-type": "application/octet-stream",
  "x-oa-operation-ref": operation,
  "x-oa-owner-ref": plan.ownerRef,
  "x-oa-target-ref": plan.targetRef,
  "x-oa-session-ref": plan.sessionRef,
  "x-oa-attachment-ref": plan.attachmentRef,
  "x-oa-attachment-generation": String(plan.generation),
  "x-oa-checkpoint-ref": plan.bundle.checkpoint.checkpointRef,
  "x-oa-artifact-ref": artifact.artifactRef,
  "x-oa-artifact-digest": artifact.digest,
});

const identityHeaders = (
  plan: LiveRetainedProofPlan,
  operation: string,
): Record<string, string> => ({
  "x-oa-operation-ref": operation,
  "x-oa-owner-ref": plan.ownerRef,
  "x-oa-target-ref": plan.targetRef,
  "x-oa-session-ref": plan.sessionRef,
  "x-oa-attachment-ref": plan.attachmentRef,
  "x-oa-attachment-generation": String(plan.generation),
});

const baseOperation = (
  plan: LiveRetainedProofPlan,
  action: string,
  operation: string,
  resourceRef: string | null,
  payload: Readonly<Record<string, Json>>,
) => ({
  operationRef: operation,
  action,
  ownerRef: plan.ownerRef,
  targetRef: plan.targetRef,
  sessionRef: plan.sessionRef,
  attachmentRef: plan.attachmentRef,
  generation: plan.generation,
  resourceRef,
  payload,
});

const hashFile = async (path: string): Promise<`sha256:${string}`> => {
  const digest = createHash("sha256");
  await new Promise<void>((resolvePromise, reject) => {
    const stream = createReadStream(path);
    stream.on("data", (chunk) => digest.update(chunk));
    stream.once("error", reject);
    stream.once("end", resolvePromise);
  });
  return `sha256:${digest.digest("hex")}`;
};

const assertCandidate = async (plan: LiveRetainedProofPlan): Promise<void> => {
  const candidates = [
    [plan.candidate.rootfsPath, plan.candidate.rootfsDigest, "rootfs"],
    [plan.candidate.kernelPath, plan.candidate.kernelDigest, "kernel"],
    [
      plan.candidate.portableSessionControlPath,
      plan.candidate.portableSessionControlDigest,
      "portable controller",
    ],
  ] as const;
  for (const [path, expected, label] of candidates) {
    if (!isAbsolute(path) || !SHA256.test(expected)) {
      throw proofFailure("candidate_unpinned", `${label} candidate is not exactly pinned`);
    }
    const info = await stat(path).catch(() => undefined);
    if (info === undefined || !info.isFile() || (await hashFile(path)) !== expected) {
      throw proofFailure("candidate_mismatch", `${label} candidate digest does not match`);
    }
  }
  if (!isAbsolute(plan.candidate.runtimeDirectory)) {
    throw proofFailure("candidate_unpinned", "runtime directory must be absolute");
  }
};

const assertPlan = (input: unknown): LiveRetainedProofPlan => {
  if (typeof input !== "object" || input === null || Array.isArray(input)) {
    throw proofFailure("invalid_plan", "proof plan must be an object");
  }
  const value = input as Record<string, unknown>;
  if (
    value.schema !== "openagents.agent_computer_live_retained_proof_plan.v1" ||
    value.integrationRevision !== INTEGRATION_REVISION
  ) {
    throw proofFailure("invalid_plan", "proof plan is not pinned to the required integration");
  }
  const url = new URL(String(value.controlBaseUrl));
  if (
    url.protocol !== "http:" ||
    !LOOPBACK.has(url.hostname) ||
    url.username !== "" ||
    url.password !== ""
  ) {
    throw proofFailure("invalid_plan", "control endpoint must be loopback HTTP");
  }
  const bundle = Schema.decodeUnknownSync(PylonPortableCheckpointBundleSchema)(value.bundle);
  const plan = { ...value, controlBaseUrl: url.origin, bundle } as unknown as LiveRetainedProofPlan;
  const refs = [
    plan.proofRef,
    plan.ownerRef,
    plan.targetRef,
    plan.sessionRef,
    plan.attachmentRef,
    plan.providerLeaseRef,
    plan.providerEvidenceRef,
    plan.authorityEvidenceRef,
  ];
  if (
    refs.some((ref) => typeof ref !== "string" || !SAFE_REF.test(ref)) ||
    plan.authenticationPolicyRef !== "policy.portable.destination.openagents_managed.v1" ||
    !Number.isSafeInteger(plan.generation) ||
    plan.generation !== bundle.checkpoint.sourceGeneration + 1 ||
    !Array.isArray(plan.continuationTurns) ||
    plan.continuationTurns.length !== bundle.graph.nodes.length
  ) {
    throw proofFailure("invalid_plan", "proof plan binding is invalid");
  }
  const agents = new Set(bundle.graph.nodes.map((node) => node.agentRef));
  const turns = new Set<string>();
  for (const turn of plan.continuationTurns) {
    if (
      !agents.has(turn.agentRef) ||
      !SAFE_REF.test(turn.turnRef) ||
      turns.has(turn.turnRef) ||
      typeof turn.task !== "string" ||
      turn.task.trim().length === 0
    ) {
      throw proofFailure(
        "invalid_plan",
        "continuation turns do not cover the exact canonical graph",
      );
    }
    turns.add(turn.turnRef);
  }
  if (
    new Set(plan.continuationTurns.map((turn) => turn.agentRef)).size !== agents.size ||
    bundle.executionBinding.ownerRef !== plan.ownerRef ||
    bundle.checkpoint.sessionRef !== plan.sessionRef ||
    !isAbsolute(plan.sourceWorkingDirectory)
  ) {
    throw proofFailure("invalid_plan", "proof source scope differs from the checkpoint bundle");
  }
  return plan;
};

export const makeRealArtifactProducer = (): ProofArtifactProducer => ({
  produce: async (input) => {
    const store = new PylonPortableCheckpointArtifactStore();
    store.register({ bundle: input.bundle, workingDirectory: input.sourceWorkingDirectory });
    return store.resolve(input);
  },
});

const defaultCleanupAudit: ProofCleanupAuditor = async (input) => {
  const vmSuffix = createHash("sha256")
    .update(`${input.stageOperationRef}|${input.targetRef}`)
    .digest("hex")
    .slice(0, 16);
  const jailId = `oa-qa-vm-${vmSuffix}`;
  const tap = `actap${createHash("sha256").update(jailId).digest("hex").slice(0, 6)}`;
  const runtimeEntries = await readdir(input.runtimeDirectory).catch((): string[] => []);
  const tapEntries = await readdir("/sys/class/net").catch((): string[] => []);
  const process = Runtime.spawn(["ps", "-axo", "command="], { stdout: "pipe", stderr: "pipe" });
  const [commands, exitCode] = await Promise.all([
    new Response(process.stdout).text(),
    process.exited,
  ]);
  if (exitCode !== 0) throw proofFailure("cleanup_audit_failed", "process cleanup audit failed");
  return {
    jailDirectoryAbsent: !runtimeEntries.includes(jailId),
    tapDeviceAbsent: !tapEntries.includes(tap),
    firecrackerProcessAbsent: !commands
      .split("\n")
      .some((command) => command.toLowerCase().includes("firecracker") && command.includes(jailId)),
  };
};

const collectEvidence = (
  ...values: ReadonlyArray<Record<string, unknown>>
): ReadonlyArray<string> =>
  [
    ...new Set(values.flatMap((value) => stringArray(value.evidenceRefs ?? [], "evidenceRefs"))),
  ].sort();

const runCore = async (
  planInput: unknown,
  capabilityMaterial: Uint8Array,
  dependencies: Dependencies,
): Promise<LiveRetainedProofReceipt> => {
  const plan = assertPlan(planInput);
  if (capabilityMaterial.byteLength === 0)
    throw proofFailure("capability_missing", "capability material is empty");
  await assertCandidate(plan);
  const readinessResponse = expectStatus(
    await dependencies.http({
      method: "GET",
      path: "/v1/cloud-vm/readiness",
    }),
    200,
    "live readiness",
  );
  const readiness = parseJson(readinessResponse.body, "live readiness");
  if (
    readiness.contractVersion !== "openagents.agent_computer_readiness.v1" ||
    readiness.ready !== true ||
    readiness.provisionerKind !== "live"
  ) {
    throw proofFailure("live_not_ready", "control daemon is not armed with the live provisioner");
  }

  const artifactProducer = dependencies.artifactProducer ?? makeRealArtifactProducer();
  const cleanupAuditor = dependencies.cleanupAuditor ?? defaultCleanupAudit;
  const stageOperationRef = operationRef(plan, "stage");
  let stageAttempted = false;
  let resourceRef: string | undefined;
  let activated = false;
  let reclaimed = false;
  let cleanupFailure: unknown;
  let sourceArtifactRef: string | undefined;
  let sourceArtifactDigest: `sha256:${string}` | undefined;

  const stageBody = baseOperation(plan, "stage", stageOperationRef, null, {
    bundle: plan.bundle as unknown as Json,
    capabilityLeaseRefs: [plan.providerLeaseRef],
  });
  try {
    stageAttempted = true;
    const stagedResponse = expectStatus(
      await requestJson(dependencies.http, "/v1/portable-agent-computers/operations", stageBody),
      200,
      "stage",
    );
    const staged = parseJson(stagedResponse.body, "stage");
    resourceRef = stringField(staged, "resourceRef");
    const reservationRef = stringField(staged, "destinationRunnerSessionReservationRef");
    if (staged.acceptingWork !== false)
      throw proofFailure("invalid_response", "staged resource accepted work");
    assertReplay(
      stagedResponse,
      await requestJson(dependencies.http, "/v1/portable-agent-computers/operations", stageBody),
      "stage",
    );
    const stageConflict = structuredClone(stageBody);
    stageConflict.payload = {
      ...stageConflict.payload,
      capabilityLeaseRefs: [stableRef("lease.conflict", plan.proofRef)],
    };
    assertConflict(
      await requestJson(
        dependencies.http,
        "/v1/portable-agent-computers/operations",
        stageConflict,
      ),
      "stage",
    );

    const artifactInput = {
      ownerRef: plan.ownerRef,
      targetRef: plan.targetRef,
      sessionRef: plan.sessionRef,
      attachmentRef: plan.attachmentRef,
      generation: plan.generation,
      checkpointRef: plan.bundle.checkpoint.checkpointRef,
      bundle: plan.bundle,
      sourceWorkingDirectory: resolve(plan.sourceWorkingDirectory),
    };
    const artifact = await artifactProducer.produce(artifactInput);
    if (
      !SAFE_REF.test(artifact.artifactRef) ||
      !SHA256.test(artifact.digest) ||
      artifact.bytes.byteLength === 0 ||
      sha256(artifact.bytes) !== artifact.digest
    ) {
      artifact.bytes.fill(0);
      throw proofFailure("artifact_invalid", "checkpoint producer returned an invalid artifact");
    }
    sourceArtifactRef = artifact.artifactRef;
    sourceArtifactDigest = artifact.digest;
    try {
      const materializeRequest = {
        method: "POST",
        path: "/v1/portable-agent-computers/checkpoints/materialize",
        headers: artifactHeaders(plan, operationRef(plan, "materialize"), artifact),
        body: artifact.bytes,
      } as const;
      const materializedResponse = expectStatus(
        await dependencies.http(materializeRequest),
        200,
        "checkpoint materialization",
      );
      const materialized = parseJson(materializedResponse.body, "checkpoint materialization");
      if (materialized.acceptingWork !== false) {
        throw proofFailure(
          "invalid_response",
          "materialized resource accepted work before activation",
        );
      }
      assertReplay(
        materializedResponse,
        await dependencies.http(materializeRequest),
        "checkpoint materialization",
      );
      assertConflict(
        await dependencies.http({
          ...materializeRequest,
          headers: {
            ...materializeRequest.headers,
            "x-oa-artifact-ref": stableRef("artifact.conflict", plan.proofRef),
          },
        }),
        "checkpoint materialization",
      );
    } finally {
      artifact.bytes.fill(0);
    }

    const capabilityRequest = {
      method: "POST",
      path: "/v1/portable-agent-computers/capabilities/install",
      headers: {
        ...identityHeaders(plan, operationRef(plan, "capability-install")),
        "content-type": "application/octet-stream",
        "x-oa-lease-ref": plan.providerLeaseRef,
        "x-oa-evidence-ref": plan.providerEvidenceRef,
        "x-oa-capability": "capability.provider.codex",
      },
      body: capabilityMaterial,
    } as const;
    const capabilityResponse = expectStatus(
      await dependencies.http(capabilityRequest),
      200,
      "capability installation",
    );
    const installed = parseJson(capabilityResponse.body, "capability installation");
    if (installed.material !== "excluded")
      throw proofFailure("secret_exposure", "capability response exposed material");
    assertReplay(
      capabilityResponse,
      await dependencies.http(capabilityRequest),
      "capability installation",
    );
    assertConflict(
      await dependencies.http({
        ...capabilityRequest,
        headers: {
          ...capabilityRequest.headers,
          "x-oa-evidence-ref": stableRef("evidence.conflict", plan.proofRef),
        },
      }),
      "capability installation",
    );

    const observedAt = (dependencies.now ?? (() => new Date()))().toISOString();
    const activateBody = baseOperation(
      plan,
      "activate",
      operationRef(plan, "activate"),
      resourceRef,
      {
        checkpointRef: plan.bundle.checkpoint.checkpointRef,
        authorityEvidenceRef: plan.authorityEvidenceRef,
        destinationRunnerSessionReservationRef: reservationRef,
        authenticationPolicyRef: plan.authenticationPolicyRef,
        helpersObservedAt: observedAt,
        capabilityLeaseRefs: [plan.providerLeaseRef],
      },
    );
    const activateResponse = expectStatus(
      await requestJson(dependencies.http, "/v1/portable-agent-computers/operations", activateBody),
      200,
      "activate",
    );
    activated = true;
    const activate = parseJson(activateResponse.body, "activate");
    const helpers = activate.helpers;
    const expectedHelperKinds = new Set(["pty", "lsp", "dap", "watcher", "native"]);
    if (
      !Array.isArray(helpers) ||
      helpers.length !== 5 ||
      new Set(
        helpers.map((helper) =>
          typeof helper === "object" && helper !== null && !Array.isArray(helper)
            ? (helper as Record<string, unknown>).kind
            : undefined,
        ),
      ).size !== expectedHelperKinds.size ||
      helpers.some((helper) => {
        if (typeof helper !== "object" || helper === null || Array.isArray(helper)) return true;
        const row = helper as Record<string, unknown>;
        return (
          typeof row.kind !== "string" ||
          !expectedHelperKinds.has(row.kind) ||
          (row.kind === "watcher" ? row.readiness !== "ready" : row.readiness !== "unsupported")
        );
      })
    ) {
      throw proofFailure(
        "helper_readiness",
        "destination helper inventory is not the current managed profile",
      );
    }
    assertReplay(
      activateResponse,
      await requestJson(dependencies.http, "/v1/portable-agent-computers/operations", activateBody),
      "activate",
    );
    const activateConflict = structuredClone(activateBody);
    activateConflict.payload = {
      ...activateConflict.payload,
      destinationRunnerSessionReservationRef: stableRef("reservation.conflict", plan.proofRef),
    };
    assertConflict(
      await requestJson(
        dependencies.http,
        "/v1/portable-agent-computers/operations",
        activateConflict,
      ),
      "activate",
    );

    const cursorsByThread = new Map(
      plan.bundle.threadCursors.map((cursor) => [cursor.threadRef, cursor]),
    );
    const continuationBody = {
      operationRef: operationRef(plan, "continuation"),
      ownerRef: plan.ownerRef,
      targetRef: plan.targetRef,
      sessionRef: plan.sessionRef,
      attachmentRef: plan.attachmentRef,
      generation: plan.generation,
      providerLeaseRef: plan.providerLeaseRef,
      expectedThreadCursors: plan.bundle.graph.nodes.map((node) => {
        const cursor = cursorsByThread.get(node.threadRef);
        if (cursor === undefined)
          throw proofFailure("invalid_plan", "canonical agent has no thread cursor");
        return {
          agentRef: node.agentRef,
          threadRef: node.threadRef,
          activityCursor: cursor.activityCursor,
          eventCursor: cursor.eventCursor,
        };
      }),
      turns: plan.continuationTurns,
    };
    const continuedResponse = expectStatus(
      await requestJson(
        dependencies.http,
        "/v1/portable-agent-computers/continuations",
        continuationBody,
      ),
      200,
      "continuation",
    );
    const continued = parseJson(continuedResponse.body, "continuation");
    if (continued.replay !== "executed")
      throw proofFailure("invalid_response", "continuation did not execute");
    if (!Array.isArray(continued.threadCursors)) {
      throw proofFailure("invalid_response", "continuation returned no thread cursors");
    }
    const continuedCursors = continued.threadCursors.map((cursor) => {
      if (typeof cursor !== "object" || cursor === null || Array.isArray(cursor)) {
        throw proofFailure("invalid_response", "continuation returned an invalid thread cursor");
      }
      const row = cursor as Record<string, unknown>;
      const agentRef = stringField(row, "agentRef");
      const threadRef = stringField(row, "threadRef");
      const expected = continuationBody.expectedThreadCursors.find(
        (candidate) => candidate.agentRef === agentRef && candidate.threadRef === threadRef,
      );
      if (
        expected === undefined ||
        !Number.isSafeInteger(row.activityCursor) ||
        Number(row.activityCursor) <= expected.activityCursor ||
        row.eventCursor !== expected.eventCursor + 1
      ) {
        throw proofFailure("invalid_response", "continuation cursor did not advance exactly");
      }
      return row;
    });
    if (continuedCursors.length !== continuationBody.expectedThreadCursors.length) {
      throw proofFailure("invalid_response", "continuation omitted a canonical thread cursor");
    }
    const continuationReplay = await requestJson(
      dependencies.http,
      "/v1/portable-agent-computers/continuations",
      continuationBody,
    );
    expectStatus(continuationReplay, 200, "continuation replay");
    const replayJson = parseJson(continuationReplay.body, "continuation replay");
    if (
      replayJson.replay !== "replayed" ||
      JSON.stringify(replayJson.threadCursors) !== JSON.stringify(continued.threadCursors)
    ) {
      throw proofFailure("replay_mismatch", "continuation replay changed its cursor receipt");
    }
    const continuationConflict = structuredClone(continuationBody);
    continuationConflict.turns = continuationConflict.turns.map((turn, index) =>
      index === 0 ? { ...turn, task: `${turn.task}\nconflict` } : turn,
    );
    assertConflict(
      await requestJson(
        dependencies.http,
        "/v1/portable-agent-computers/continuations",
        continuationConflict,
      ),
      "continuation",
    );

    const quiesceBody = baseOperation(plan, "quiesce", operationRef(plan, "quiesce"), resourceRef, {
      graph: plan.bundle.graph as unknown as Json,
    });
    const quiescedResponse = expectStatus(
      await requestJson(dependencies.http, "/v1/portable-agent-computers/operations", quiesceBody),
      200,
      "quiesce",
    );
    const quiesced = parseJson(quiescedResponse.body, "quiesce");
    assertReplay(
      quiescedResponse,
      await requestJson(dependencies.http, "/v1/portable-agent-computers/operations", quiesceBody),
      "quiesce",
    );
    const quiesceConflict = structuredClone(quiesceBody);
    quiesceConflict.payload = {
      graph: {
        ...(plan.bundle.graph as unknown as Record<string, Json>),
        rootAgentRef: stableRef("agent.conflict", plan.proofRef),
      },
    };
    assertConflict(
      await requestJson(
        dependencies.http,
        "/v1/portable-agent-computers/operations",
        quiesceConflict,
      ),
      "quiesce",
    );

    const checkpointRef = stableRef("checkpoint.agent-computer.live-proof", plan.proofRef);
    const checkpointBody = baseOperation(
      plan,
      "checkpoint",
      operationRef(plan, "checkpoint"),
      resourceRef,
      {
        checkpointRef,
        eventLogCursor: Math.max(
          ...(continuedCursors as ReadonlyArray<Record<string, number>>).map(
            (cursor) => cursor.eventCursor,
          ),
        ),
        executionBinding: plan.bundle.executionBinding as unknown as Json,
        graph: plan.bundle.graph as unknown as Json,
        threadCursors: continuedCursors as Json,
      },
    );
    const checkpointResponse = expectStatus(
      await requestJson(
        dependencies.http,
        "/v1/portable-agent-computers/operations",
        checkpointBody,
      ),
      200,
      "checkpoint",
    );
    const checkpointed = parseJson(checkpointResponse.body, "checkpoint");
    assertReplay(
      checkpointResponse,
      await requestJson(
        dependencies.http,
        "/v1/portable-agent-computers/operations",
        checkpointBody,
      ),
      "checkpoint",
    );
    const checkpointConflict = structuredClone(checkpointBody);
    checkpointConflict.payload = {
      ...checkpointConflict.payload,
      checkpointRef: stableRef("checkpoint.conflict", plan.proofRef),
    };
    assertConflict(
      await requestJson(
        dependencies.http,
        "/v1/portable-agent-computers/operations",
        checkpointConflict,
      ),
      "checkpoint",
    );

    const exportHeaders = {
      ...identityHeaders(plan, operationRef(plan, "export")),
      "x-oa-checkpoint-ref": checkpointRef,
    };
    const exportResponse = expectStatus(
      await dependencies.http({
        method: "POST",
        path: "/v1/portable-agent-computers/checkpoints/export",
        headers: exportHeaders,
      }),
      200,
      "checkpoint export",
    );
    let exportedArtifactRef: string | undefined;
    let exportedArtifactDigest: `sha256:${string}` | undefined;
    let exportReplay: ProofHttpResponse | undefined;
    try {
      exportedArtifactRef = exportResponse.headers["x-oa-artifact-ref"];
      const digest = exportResponse.headers["x-oa-artifact-digest"];
      if (
        exportedArtifactRef === undefined ||
        !SAFE_REF.test(exportedArtifactRef) ||
        digest === undefined ||
        !SHA256.test(digest) ||
        sha256(exportResponse.body) !== digest
      ) {
        throw proofFailure("artifact_invalid", "exported checkpoint digest is invalid");
      }
      exportedArtifactDigest = digest as `sha256:${string}`;
      exportReplay = await dependencies.http({
        method: "POST",
        path: "/v1/portable-agent-computers/checkpoints/export",
        headers: exportHeaders,
      });
      assertReplay(exportResponse, exportReplay, "checkpoint export");
      const exportConflictHeaders = {
        ...exportHeaders,
        "x-oa-checkpoint-ref": stableRef("checkpoint.conflict", plan.proofRef),
      };
      assertConflict(
        await dependencies.http({
          method: "POST",
          path: "/v1/portable-agent-computers/checkpoints/export",
          headers: exportConflictHeaders,
        }),
        "checkpoint export",
      );
    } finally {
      exportResponse.body.fill(0);
      exportReplay?.body.fill(0);
    }
    if (exportedArtifactRef === undefined || exportedArtifactDigest === undefined) {
      throw proofFailure("artifact_invalid", "exported checkpoint receipt was lost");
    }

    const agentRefs = plan.bundle.graph.nodes.map((node) => node.agentRef);
    const reclaimBody = baseOperation(plan, "reclaim", operationRef(plan, "reclaim"), resourceRef, {
      agentRefs,
    });
    const reclaimResponse = expectStatus(
      await requestJson(dependencies.http, "/v1/portable-agent-computers/operations", reclaimBody),
      200,
      "reclaim",
    );
    const reclaimedJson = parseJson(reclaimResponse.body, "reclaim");
    if (
      reclaimedJson.scratch !== "released" ||
      reclaimedJson.processes !== "released" ||
      reclaimedJson.ports !== "released"
    ) {
      throw proofFailure("cleanup_incomplete", "reclaim did not release all retained resources");
    }
    assertReplay(
      reclaimResponse,
      await requestJson(dependencies.http, "/v1/portable-agent-computers/operations", reclaimBody),
      "reclaim",
    );
    const reclaimConflict = structuredClone(reclaimBody);
    reclaimConflict.payload = { agentRefs: [stableRef("agent.conflict", plan.proofRef)] };
    assertConflict(
      await requestJson(
        dependencies.http,
        "/v1/portable-agent-computers/operations",
        reclaimConflict,
      ),
      "reclaim",
    );
    reclaimed = true;
    const audit = await cleanupAuditor({
      runtimeDirectory: plan.candidate.runtimeDirectory,
      stageOperationRef,
      targetRef: plan.targetRef,
    });
    if (!audit.jailDirectoryAbsent || !audit.tapDeviceAbsent || !audit.firecrackerProcessAbsent) {
      throw proofFailure("cleanup_incomplete", "host cleanup audit found retained resources");
    }
    const continuedTurns = acceptedTurnRefs(continued.acceptedWorkRefs);
    const expectedAgentRefs = plan.bundle.graph.nodes.map((node) => node.agentRef).sort();
    const activatedAgentRefs = [
      ...stringArray(activate.activatedAgentRefs, "activatedAgentRefs"),
    ].sort();
    const expectedTurnRefs = plan.continuationTurns.map((turn) => turn.turnRef).sort();
    if (
      JSON.stringify(activatedAgentRefs) !== JSON.stringify(expectedAgentRefs) ||
      JSON.stringify([...continuedTurns].sort()) !== JSON.stringify(expectedTurnRefs)
    ) {
      throw proofFailure("invalid_response", "destination omitted a canonical agent or turn");
    }
    if (sourceArtifactRef === undefined || sourceArtifactDigest === undefined) {
      throw proofFailure("artifact_invalid", "source artifact receipt was lost");
    }
    return {
      schema: "openagents.agent_computer_live_retained_proof_receipt.v1",
      receiptRef: stableRef(
        "receipt.agent-computer.live-retained-proof",
        `${plan.proofRef}:${exportedArtifactDigest}`,
      ),
      proofRef: plan.proofRef,
      integrationRevision: INTEGRATION_REVISION,
      candidate: {
        rootfsDigest: plan.candidate.rootfsDigest,
        kernelDigest: plan.candidate.kernelDigest,
        portableSessionControlDigest: plan.candidate.portableSessionControlDigest,
      },
      resourceRef,
      destinationRunnerSessionReservationRef: reservationRef,
      artifactRef: sourceArtifactRef,
      artifactDigest: sourceArtifactDigest,
      exportedArtifactRef,
      exportedArtifactDigest: exportedArtifactDigest as `sha256:${string}`,
      checkpointRef,
      activatedAgentRefs,
      continuedTurnRefs: [...continuedTurns].sort(),
      evidenceRefs: [
        ...new Set([
          plan.providerEvidenceRef,
          plan.authorityEvidenceRef,
          ...collectEvidence(staged, installed, activate, quiesced, checkpointed, reclaimedJson),
        ]),
      ].sort(),
      replayChecks: {
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
      },
      cleanup: { ...audit, reclaimReplayAccepted: true },
      material: "excluded",
      privateTasks: "excluded",
      publicSafe: true,
    };
  } finally {
    capabilityMaterial.fill(0);
    if (!reclaimed && (resourceRef !== undefined || stageAttempted)) {
      try {
        if (activated && resourceRef !== undefined) {
          await requestJson(
            dependencies.http,
            "/v1/portable-agent-computers/operations",
            baseOperation(plan, "quiesce", operationRef(plan, "cleanup-quiesce"), resourceRef, {
              graph: plan.bundle.graph as unknown as Json,
            }),
          );
          await requestJson(
            dependencies.http,
            "/v1/portable-agent-computers/operations",
            baseOperation(plan, "reclaim", operationRef(plan, "cleanup-reclaim"), resourceRef, {
              agentRefs: plan.bundle.graph.nodes.map((node) => node.agentRef),
            }),
          );
        } else {
          await requestJson(
            dependencies.http,
            "/v1/portable-agent-computers/operations",
            baseOperation(
              plan,
              "abortPrepared",
              operationRef(plan, "cleanup-abort-prepared"),
              null,
              { stageOperationRef },
            ),
          );
        }
        const audit = await cleanupAuditor({
          runtimeDirectory: plan.candidate.runtimeDirectory,
          stageOperationRef,
          targetRef: plan.targetRef,
        });
        if (
          !audit.jailDirectoryAbsent ||
          !audit.tapDeviceAbsent ||
          !audit.firecrackerProcessAbsent
        ) {
          cleanupFailure = proofFailure(
            "cleanup_incomplete",
            "failure cleanup audit found retained resources",
          );
        }
      } catch (error) {
        cleanupFailure = error;
      }
    }
    if (cleanupFailure !== undefined)
      throw proofFailure("cleanup_failed", "proof failed and cleanup did not verify");
  }
};

export const runLiveRetainedProof = Effect.fn("AgentComputerLiveRetainedProof.run")(function* (
  planInput: unknown,
  capabilityMaterial: Uint8Array,
  dependencies: Dependencies,
) {
  return yield* Effect.tryPromise({
    try: () => runCore(planInput, capabilityMaterial, dependencies),
    catch: (error) =>
      error instanceof ProofFailure
        ? error
        : proofFailure("proof_failed", "live retained proof failed"),
  }).pipe(Effect.ensuring(Effect.sync(() => capabilityMaterial.fill(0))));
});

export const makeAuthenticatedHttpClient =
  (baseUrl: string, token: string): ProofHttpClient =>
  async (request) => {
    const response = await fetch(`${baseUrl}${request.path}`, {
      method: request.method,
      headers: { ...request.headers, authorization: `Bearer ${token}` },
      body: request.body === undefined ? undefined : Buffer.from(request.body),
    });
    const headers: Record<string, string> = {};
    response.headers.forEach((value, name) => {
      headers[name.toLowerCase()] = value;
    });
    return { status: response.status, headers, body: new Uint8Array(await response.arrayBuffer()) };
  };

const main = Effect.gen(function* () {
  const planPath = process.argv[2];
  if (process.argv.length !== 3 || planPath === undefined || !isAbsolute(planPath)) {
    return yield* Effect.fail(
      proofFailure("usage", "usage: live-retained-proof.ts /absolute/path/to/plan.json"),
    );
  }
  const environment = ConfigProvider.fromEnv();
  const tokenFd = yield* Config.int("OA_AGENT_COMPUTER_CONTROL_TOKEN_FD").parse(environment);
  const capabilityFd = yield* Config.int("OA_AGENT_COMPUTER_CAPABILITY_FD").parse(environment);
  if (tokenFd < 3 || capabilityFd < 3 || tokenFd === capabilityFd) {
    return yield* Effect.fail(
      proofFailure(
        "credential_fd_invalid",
        "credential file descriptors must be distinct and at least 3",
      ),
    );
  }
  const planInput: unknown = JSON.parse(yield* Effect.promise(() => readFile(planPath, "utf8")));
  const plan = assertPlan(planInput);
  const tokenBytes = Uint8Array.from(readFileSync(tokenFd));
  const capability = Uint8Array.from(readFileSync(capabilityFd));
  try {
    if (tokenBytes.byteLength === 0 || tokenBytes.byteLength > 16 * 1024) {
      return yield* Effect.fail(proofFailure("token_invalid", "control token length is invalid"));
    }
    if (capability.byteLength === 0 || capability.byteLength > 128 * 1024) {
      return yield* Effect.fail(
        proofFailure("capability_invalid", "capability material length is invalid"),
      );
    }
    const token = new TextDecoder().decode(tokenBytes).trim();
    if (token.length === 0)
      return yield* Effect.fail(proofFailure("token_missing", "control token is empty"));
    const receipt = yield* runLiveRetainedProof(plan, capability, {
      http: makeAuthenticatedHttpClient(plan.controlBaseUrl, token),
    });
    process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
  } finally {
    tokenBytes.fill(0);
    capability.fill(0);
  }
});

if (Runtime.isMain(import.meta.url)) {
  Effect.runPromise(main).catch((error) => {
    const message = error instanceof ProofFailure ? error.message : "live retained proof failed";
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  });
}
