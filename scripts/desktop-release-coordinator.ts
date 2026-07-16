import { createHash, createPublicKey, verify as verifySignature } from "node:crypto";
import {
  closeSync,
  existsSync,
  fsyncSync,
  mkdirSync,
  openSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";

import {
  type PortCallResult,
  type ReleaseChannel,
  type ReleaseCoordinatorPort,
  type ReleasePlan,
  type ReleaseTargetKey,
  TRANSACTION_REF_PATTERN,
  releaseTargetKeys,
} from "./release.js";

export const COORDINATOR_SCHEMA = "openagents.desktop.release_coordinator.v1" as const;
export const WORKER_RECEIPT_SCHEMA = "openagents.desktop.worker_receipt.v1" as const;

export const formatsByTarget: Readonly<Record<ReleaseTargetKey, readonly string[]>> = {
  "darwin-arm64": ["dmg", "zip"],
  "darwin-x64": ["dmg", "zip"],
  "win32-arm64": ["nsis"],
  "win32-x64": ["nsis"],
  "linux-arm64": ["appimage", "deb", "rpm"],
  "linux-x64": ["appimage", "deb", "rpm"],
};

const SHA256 = /^[0-9a-f]{64}$/;
const SOURCE_REVISION = /^[0-9a-f]{40}$/;
const PUBLIC_REF = /^[A-Za-z0-9][A-Za-z0-9._:/-]{0,239}$/;
const OBJECT_KEY = /^desktop\/candidate\/[A-Za-z0-9._/-]{1,300}$/;
const ED25519_SIGNATURE = /^[A-Za-z0-9_-]{86}$/;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value);

const hasExactKeys = (value: Record<string, unknown>, expected: readonly string[]): boolean => {
  const observed = Object.keys(value).toSorted();
  return (
    observed.length === expected.length && observed.every((key, index) => key === expected[index])
  );
};

const WORKER_RECEIPT_KEYS = [
  "artifacts",
  "attempt",
  "channel",
  "completedAt",
  "leaseId",
  "nativeProofRefs",
  "planDigest",
  "schema",
  "signingPolicyId",
  "sourceRevision",
  "stagingLedgerRef",
  "target",
  "toolchainProfileRef",
  "transactionRef",
  "version",
  "workerRef",
] as const;
const NATIVE_PROOF_KEYS = [
  "agentRuntime",
  "cleanInstall",
  "interruptionResume",
  "launch",
  "reinstall",
  "rollbackOrNoRollback",
  "shutdown",
  "uninstall",
  "update",
] as const;
const ARTIFACT_RECEIPT_KEYS = [
  "buildReceiptRef",
  "byteLength",
  "componentLedgerRef",
  "format",
  "name",
  "objectKey",
  "sha256",
  "signingReceiptRef",
] as const;

const workerReceiptShapeProblem = (value: unknown): string | undefined => {
  if (!isRecord(value) || !hasExactKeys(value, ["payload", "signature"]))
    return "receipt envelope shape invalid";
  const { payload, signature } = value;
  if (!isRecord(payload) || !hasExactKeys(payload, WORKER_RECEIPT_KEYS))
    return "receipt payload shape invalid";
  if (!isRecord(signature) || !hasExactKeys(signature, ["alg", "kid", "value"]))
    return "signature envelope shape invalid";
  if (
    signature.alg !== "ed25519" ||
    typeof signature.kid !== "string" ||
    typeof signature.value !== "string" ||
    !ED25519_SIGNATURE.test(signature.value)
  )
    return "signature envelope shape invalid";
  for (const key of WORKER_RECEIPT_KEYS) {
    if (key === "artifacts" || key === "nativeProofRefs" || key === "attempt") continue;
    if (typeof payload[key] !== "string") return `receipt ${key} type invalid`;
  }
  if (!Number.isSafeInteger(payload.attempt) || (payload.attempt as number) < 1)
    return "receipt attempt type invalid";
  const nativeProofRefs = payload.nativeProofRefs;
  if (!isRecord(nativeProofRefs) || !hasExactKeys(nativeProofRefs, NATIVE_PROOF_KEYS))
    return "native proof shape invalid";
  if (NATIVE_PROOF_KEYS.some((key) => typeof nativeProofRefs[key] !== "string"))
    return "native proof type invalid";
  if (!Array.isArray(payload.artifacts)) return "artifact list type invalid";
  for (const artifact of payload.artifacts) {
    if (!isRecord(artifact) || !hasExactKeys(artifact, ARTIFACT_RECEIPT_KEYS))
      return "artifact receipt shape invalid";
    for (const key of ARTIFACT_RECEIPT_KEYS) {
      if (key === "byteLength") continue;
      if (typeof artifact[key] !== "string") return `artifact ${key} type invalid`;
    }
    if (!Number.isSafeInteger(artifact.byteLength)) return "artifact byteLength type invalid";
  }
  return undefined;
};

const canonicalValue = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(canonicalValue);
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .toSorted(([left], [right]) => left.localeCompare(right))
      .map(([key, child]) => [key, canonicalValue(child)]),
  );
};

export const canonicalJson = (value: unknown): string => JSON.stringify(canonicalValue(value));
export const sha256 = (value: string | Uint8Array): string =>
  createHash("sha256").update(value).digest("hex");

export class ReleaseCoordinatorError extends Error {
  constructor(
    readonly code:
      | "frozen_plan_mismatch"
      | "worker_inventory_unavailable"
      | "worker_unhealthy"
      | "lease_stale"
      | "worker_receipt_invalid"
      | "matrix_incomplete"
      | "candidate_object_invalid"
      | "candidate_handoff_refused"
      | "promotion_precondition_failed"
      | "promotion_race"
      | "state_conflict",
    message: string,
    readonly target?: ReleaseTargetKey,
  ) {
    super(`${code}: ${message}`);
    this.name = "ReleaseCoordinatorError";
  }
}

export type FrozenReleaseAuthority = Readonly<{
  sourceRevision: string;
  version: string;
  channel: ReleaseChannel;
  targets: readonly ReleaseTargetKey[];
  stagingLedgerRef: string;
  signingPolicyId: string;
  toolchainProfileRef: string;
  releaseNotesSha256: string;
}>;

export type RunnerInventoryEntry = Readonly<{
  workerRef: string;
  target: ReleaseTargetKey;
  hostClass: string;
  buildMode: "native" | "cross";
  nativeAcceptanceHostRef: string;
  toolchainProfileRef: string;
  signingOperationRef: string;
  enabled: boolean;
}>;

export type WorkerHealth = Readonly<{
  workerRef: string;
  target: ReleaseTargetKey;
  state: "healthy" | "unavailable";
  observedToolchainProfileRef: string;
  observedAt: string;
  blockerRef?: string;
}>;

export type WorkerArtifactReceipt = Readonly<{
  format: string;
  name: string;
  objectKey: string;
  sha256: string;
  byteLength: number;
  componentLedgerRef: string;
  buildReceiptRef: string;
  signingReceiptRef: string;
}>;

export type WorkerReceiptPayload = Readonly<{
  schema: typeof WORKER_RECEIPT_SCHEMA;
  transactionRef: string;
  planDigest: string;
  leaseId: string;
  attempt: number;
  workerRef: string;
  target: ReleaseTargetKey;
  sourceRevision: string;
  version: string;
  channel: ReleaseChannel;
  stagingLedgerRef: string;
  toolchainProfileRef: string;
  signingPolicyId: string;
  artifacts: readonly WorkerArtifactReceipt[];
  nativeProofRefs: Readonly<{
    cleanInstall: string;
    launch: string;
    agentRuntime: string;
    shutdown: string;
    update: string;
    interruptionResume: string;
    rollbackOrNoRollback: string;
    reinstall: string;
    uninstall: string;
  }>;
  completedAt: string;
}>;

export type SignedWorkerReceipt = Readonly<{
  payload: WorkerReceiptPayload;
  signature: Readonly<{ alg: "ed25519"; kid: string; value: string }>;
}>;

export type WorkerDispatchRequest = Readonly<{
  transactionRef: string;
  plan: FrozenReleaseAuthority;
  planDigest: string;
  target: ReleaseTargetKey;
  formats: readonly string[];
  lease: Readonly<{ id: string; attempt: number; expiresAt: string }>;
  auth: Readonly<{ alg: "ed25519"; kid: string; value: string }>;
}>;

export interface WorkerControl {
  start(entry: RunnerInventoryEntry): Promise<void>;
  health(entry: RunnerInventoryEntry): Promise<WorkerHealth>;
  heartbeat(entry: RunnerInventoryEntry, leaseId: string): Promise<Readonly<{ alive: boolean }>>;
  dispatch(
    entry: RunnerInventoryEntry,
    request: WorkerDispatchRequest,
  ): Promise<SignedWorkerReceipt>;
  cancel(entry: RunnerInventoryEntry, leaseId: string): Promise<void>;
  stop(entry: RunnerInventoryEntry): Promise<void>;
}

export interface CoordinatorRequestSigner {
  readonly kid: string;
  sign(canonicalRequest: Uint8Array): Promise<string> | string;
}

export interface WorkerKeyring {
  publicKey(workerRef: string, kid: string): string | undefined;
}

export interface CandidateObjectStore {
  headImmutable(
    objectKey: string,
  ): Promise<
    | { readonly exists: false }
    | { readonly exists: true; readonly sha256: string; readonly byteLength: number }
  >;
}

export type CandidateHandoff = Readonly<{
  candidateRef: string;
  releaseSetPayloadSha256: string;
  previousPointerRef: string;
}>;

export interface CandidatePublisher {
  publishVerifiedCandidate(
    input: Readonly<{
      transactionRef: string;
      plan: FrozenReleaseAuthority;
      planDigest: string;
      matrixDigest: string;
      receipts: readonly WorkerReceiptPayload[];
    }>,
  ): Promise<CandidateHandoff>;
}

export interface CandidateAcceptanceGate {
  verifyCandidate(
    input: Readonly<{
      transactionRef: string;
      candidateRef: string;
      releaseSetPayloadSha256: string;
      planDigest: string;
      matrixDigest: string;
    }>,
  ): Promise<Readonly<{ accepted: boolean; receiptRef?: string; blockerRef?: string }>>;
}

export interface AtomicChannelPromoter {
  compareAndSwap(
    input: Readonly<{
      channel: ReleaseChannel;
      expectedPreviousPointerRef: string;
      candidateRef: string;
      releaseSetPayloadSha256: string;
      acceptanceReceiptRef: string;
    }>,
  ): Promise<Readonly<{ promoted: boolean; pointerRef?: string; currentPointerRef?: string }>>;
}

type TargetState = Readonly<{
  workerRef: string;
  state: "inventory_verified" | "healthy" | "leased" | "completed" | "failed";
  attempt: number;
  leaseId?: string;
  leaseExpiresAt?: string;
  receipt?: SignedWorkerReceipt;
  blockerRef?: string;
}>;

export type CoordinatorState = Readonly<{
  schema: typeof COORDINATOR_SCHEMA;
  revision: number;
  transactionRef: string;
  planDigest: string;
  frozenPlan: FrozenReleaseAuthority;
  phase: "inventory" | "workers_ready" | "converged" | "gates_passed" | "candidate" | "promoted";
  targets: Readonly<Partial<Record<ReleaseTargetKey, TargetState>>>;
  matrixDigest?: string;
  candidate?: CandidateHandoff;
  promotionRef?: string;
}>;

export interface CoordinatorStateStore {
  load(transactionRef: string): Promise<CoordinatorState | undefined>;
  save(state: CoordinatorState, expectedRevision: number): Promise<CoordinatorState>;
}

/** Durable atomic JSON store. The lock is exclusive and conflicts fail closed. */
export class FileCoordinatorStateStore implements CoordinatorStateStore {
  constructor(private readonly root: string) {}

  private path(transactionRef: string): string {
    if (!TRANSACTION_REF_PATTERN.test(transactionRef)) {
      throw new ReleaseCoordinatorError("state_conflict", "invalid transaction reference");
    }
    return join(this.root, `${transactionRef}.json`);
  }

  async load(transactionRef: string): Promise<CoordinatorState | undefined> {
    const path = this.path(transactionRef);
    if (!existsSync(path)) return undefined;
    return JSON.parse(readFileSync(path, "utf8")) as CoordinatorState;
  }

  async save(state: CoordinatorState, expectedRevision: number): Promise<CoordinatorState> {
    mkdirSync(this.root, { recursive: true, mode: 0o700 });
    const path = this.path(state.transactionRef);
    const lock = `${path}.lock`;
    let lockFd: number;
    try {
      lockFd = openSync(lock, "wx", 0o600);
    } catch {
      if (existsSync(lock) && Date.now() - statSync(lock).mtimeMs > 60_000) {
        rmSync(lock, { force: true });
        try {
          lockFd = openSync(lock, "wx", 0o600);
        } catch {
          throw new ReleaseCoordinatorError("state_conflict", "coordinator state is locked");
        }
      } else {
        throw new ReleaseCoordinatorError("state_conflict", "coordinator state is locked");
      }
    }
    try {
      const current = existsSync(path)
        ? (JSON.parse(readFileSync(path, "utf8")) as CoordinatorState)
        : undefined;
      const revision = current?.revision ?? 0;
      if (revision !== expectedRevision) {
        throw new ReleaseCoordinatorError(
          "state_conflict",
          `expected state revision ${expectedRevision}, observed ${revision}`,
        );
      }
      const next = { ...state, revision: expectedRevision + 1 };
      const temporary = `${path}.${process.pid}.${Date.now()}.tmp`;
      const fd = openSync(temporary, "wx", 0o600);
      try {
        writeFileSync(fd, `${canonicalJson(next)}\n`, "utf8");
        fsyncSync(fd);
      } finally {
        closeSync(fd);
      }
      renameSync(temporary, path);
      const parentFd = openSync(dirname(path), "r");
      try {
        fsyncSync(parentFd);
      } finally {
        closeSync(parentFd);
      }
      return next;
    } finally {
      closeSync(lockFd);
      rmSync(lock, { force: true });
    }
  }
}

export type ReleaseCoordinatorDependencies = Readonly<{
  inventory: readonly RunnerInventoryEntry[];
  workerControl: WorkerControl;
  requestSigner: CoordinatorRequestSigner;
  workerKeyring: WorkerKeyring;
  objectStore: CandidateObjectStore;
  candidatePublisher: CandidatePublisher;
  acceptanceGate: CandidateAcceptanceGate;
  promoter: AtomicChannelPromoter;
  stateStore: CoordinatorStateStore;
  now: () => Date;
  leaseDurationMs?: number;
  maxAttempts?: number;
}>;

const assertRef = (value: string, label: string): void => {
  if (!PUBLIC_REF.test(value))
    throw new ReleaseCoordinatorError("worker_receipt_invalid", `${label} is not public-safe`);
};

export const frozenPlanDigest = (plan: FrozenReleaseAuthority): string =>
  sha256(canonicalJson(plan));

const planFromReleasePlan = (
  plan: ReleasePlan,
  authority: FrozenReleaseAuthority,
): FrozenReleaseAuthority => {
  const targetsEqual =
    plan.targets.length === releaseTargetKeys.length &&
    plan.targets.every((target, index) => target === releaseTargetKeys[index]);
  if (
    plan.sourceRevision !== authority.sourceRevision ||
    plan.version !== authority.version ||
    plan.channel !== authority.channel ||
    !targetsEqual ||
    canonicalJson(authority.targets) !== canonicalJson(releaseTargetKeys)
  ) {
    throw new ReleaseCoordinatorError(
      "frozen_plan_mismatch",
      "release command does not match frozen authority",
    );
  }
  if (
    !SOURCE_REVISION.test(authority.sourceRevision) ||
    !SHA256.test(authority.releaseNotesSha256)
  ) {
    throw new ReleaseCoordinatorError(
      "frozen_plan_mismatch",
      "frozen source or release-notes digest is invalid",
    );
  }
  for (const [label, ref] of [
    ["staging ledger", authority.stagingLedgerRef],
    ["signing policy", authority.signingPolicyId],
    ["toolchain profile", authority.toolchainProfileRef],
  ] as const)
    assertRef(ref, label);
  return authority;
};

const inventoryForPlan = (
  inventory: readonly RunnerInventoryEntry[],
  authority: FrozenReleaseAuthority,
): Map<ReleaseTargetKey, RunnerInventoryEntry> => {
  const entries = new Map<ReleaseTargetKey, RunnerInventoryEntry>();
  for (const row of inventory) {
    if (!authority.targets.includes(row.target)) continue;
    if (entries.has(row.target)) {
      throw new ReleaseCoordinatorError(
        "worker_inventory_unavailable",
        `duplicate worker for ${row.target}`,
        row.target,
      );
    }
    if (!row.enabled) {
      throw new ReleaseCoordinatorError(
        "worker_inventory_unavailable",
        `worker is disabled for ${row.target}`,
        row.target,
      );
    }
    for (const [label, ref] of [
      ["worker", row.workerRef],
      ["host class", row.hostClass],
      ["native acceptance host", row.nativeAcceptanceHostRef],
      ["toolchain profile", row.toolchainProfileRef],
      ["signing operation", row.signingOperationRef],
    ] as const)
      assertRef(ref, label);
    if (row.toolchainProfileRef !== authority.toolchainProfileRef) {
      throw new ReleaseCoordinatorError(
        "worker_inventory_unavailable",
        `toolchain profile mismatch for ${row.target}`,
        row.target,
      );
    }
    // A cross builder never substitutes for the required native acceptance host.
    if (row.buildMode === "cross" && row.nativeAcceptanceHostRef.startsWith("unavailable:")) {
      throw new ReleaseCoordinatorError(
        "worker_inventory_unavailable",
        `native acceptance host unavailable for ${row.target}`,
        row.target,
      );
    }
    entries.set(row.target, row);
  }
  for (const target of releaseTargetKeys) {
    if (!entries.has(target)) {
      throw new ReleaseCoordinatorError(
        "worker_inventory_unavailable",
        `required target unavailable: ${target}`,
        target,
      );
    }
  }
  return entries;
};

const receiptPayloadProblem = (
  receipt: WorkerReceiptPayload,
  request: WorkerDispatchRequest,
  worker: RunnerInventoryEntry,
  now: Date,
): string | undefined => {
  if (receipt.schema !== WORKER_RECEIPT_SCHEMA) return "wrong schema";
  if (
    receipt.transactionRef !== request.transactionRef ||
    receipt.planDigest !== request.planDigest
  )
    return "wrong transaction or plan";
  if (receipt.leaseId !== request.lease.id || receipt.attempt !== request.lease.attempt)
    return "stale lease";
  if (now.getTime() > Date.parse(request.lease.expiresAt)) return "lease expired";
  if (receipt.workerRef !== worker.workerRef || receipt.target !== request.target)
    return "wrong worker or target";
  if (
    receipt.sourceRevision !== request.plan.sourceRevision ||
    receipt.version !== request.plan.version ||
    receipt.channel !== request.plan.channel ||
    receipt.stagingLedgerRef !== request.plan.stagingLedgerRef ||
    receipt.toolchainProfileRef !== request.plan.toolchainProfileRef ||
    receipt.signingPolicyId !== request.plan.signingPolicyId
  )
    return "frozen identity mismatch";
  const completedAt = Date.parse(receipt.completedAt);
  if (!Number.isFinite(completedAt) || completedAt > Date.parse(request.lease.expiresAt))
    return "completion time outside lease";
  const nativeProofRefs = Object.values(receipt.nativeProofRefs);
  if (
    nativeProofRefs.some((ref) => !PUBLIC_REF.test(ref)) ||
    new Set(nativeProofRefs).size !== nativeProofRefs.length
  )
    return "native proof missing";
  if (
    receipt.artifacts.map((row) => row.format).join(",") !==
    formatsByTarget[request.target].join(",")
  )
    return "format set incomplete or non-canonical";
  const names = new Set<string>();
  const keys = new Set<string>();
  for (const artifact of receipt.artifacts) {
    if (names.has(artifact.name) || keys.has(artifact.objectKey)) return "duplicate artifact";
    names.add(artifact.name);
    keys.add(artifact.objectKey);
    if (
      !OBJECT_KEY.test(artifact.objectKey) ||
      artifact.objectKey.split("/").some((segment) => segment === ".." || segment.length === 0) ||
      !artifact.objectKey.includes(`/${request.plan.version}/${request.target}/`)
    )
      return "mutable or wrong candidate object key";
    const [platform, architecture] = request.target.split("-");
    const expectedName =
      platform === "darwin"
        ? `OpenAgents-${request.plan.version}-${request.plan.channel}-darwin-${architecture}.${artifact.format}`
        : platform === "win32"
          ? `OpenAgents-${request.plan.version}-${request.plan.channel}-win32-${architecture}-setup.exe`
          : `OpenAgents-${request.plan.version}-${request.plan.channel}-linux-${architecture}.${artifact.format === "appimage" ? "AppImage" : artifact.format}`;
    if (artifact.name !== expectedName) return "artifact name is not canonical";
    if (
      !SHA256.test(artifact.sha256) ||
      !Number.isSafeInteger(artifact.byteLength) ||
      artifact.byteLength <= 0
    )
      return "artifact digest or size invalid";
    for (const ref of [
      artifact.componentLedgerRef,
      artifact.buildReceiptRef,
      artifact.signingReceiptRef,
    ])
      if (!PUBLIC_REF.test(ref)) return "artifact receipt ref invalid";
  }
  return undefined;
};

const verifyWorkerReceipt = (
  value: unknown,
  request: WorkerDispatchRequest,
  worker: RunnerInventoryEntry,
  keyring: WorkerKeyring,
  now: Date,
): void => {
  const shapeProblem = workerReceiptShapeProblem(value);
  if (shapeProblem !== undefined)
    throw new ReleaseCoordinatorError("worker_receipt_invalid", shapeProblem, worker.target);
  const signed = value as SignedWorkerReceipt;
  const problem = receiptPayloadProblem(signed.payload, request, worker, now);
  if (problem !== undefined)
    throw new ReleaseCoordinatorError(
      problem === "stale lease" ? "lease_stale" : "worker_receipt_invalid",
      problem,
      worker.target,
    );
  if (!PUBLIC_REF.test(signed.signature.kid)) {
    throw new ReleaseCoordinatorError(
      "worker_receipt_invalid",
      "signature envelope invalid",
      worker.target,
    );
  }
  const key = keyring.publicKey(worker.workerRef, signed.signature.kid);
  if (key === undefined)
    throw new ReleaseCoordinatorError(
      "worker_receipt_invalid",
      "worker key is not pinned",
      worker.target,
    );
  let verified = false;
  try {
    verified = verifySignature(
      null,
      Buffer.from(canonicalJson(signed.payload)),
      createPublicKey(key),
      Buffer.from(signed.signature.value, "base64url"),
    );
  } catch {
    verified = false;
  }
  if (!verified)
    throw new ReleaseCoordinatorError(
      "worker_receipt_invalid",
      "worker signature invalid",
      worker.target,
    );
};

const redactedLine = (line: string): string => {
  if (
    line.length > 240 ||
    /(?:secret|token|password|private[_ -]?key|\/Users\/|https?:\/\/\d)/i.test(line)
  ) {
    throw new ReleaseCoordinatorError("worker_receipt_invalid", "receipt line is not public-safe");
  }
  return line;
};

export const createOwnedReleaseCoordinator = (
  authority: FrozenReleaseAuthority,
  deps: ReleaseCoordinatorDependencies,
): ReleaseCoordinatorPort => {
  const leaseDurationMs = deps.leaseDurationMs ?? 30 * 60_000;
  const maxAttempts = deps.maxAttempts ?? 2;
  const inventory = inventoryForPlan(deps.inventory, authority);
  const digest = frozenPlanDigest(authority);

  const loadRequired = async (plan: ReleasePlan): Promise<CoordinatorState> => {
    planFromReleasePlan(plan, authority);
    const state = await deps.stateStore.load(plan.transactionRef);
    if (
      state === undefined ||
      state.schema !== COORDINATOR_SCHEMA ||
      state.transactionRef !== plan.transactionRef ||
      state.planDigest !== digest ||
      canonicalJson(state.frozenPlan) !== canonicalJson(authority) ||
      Object.keys(state.targets).some(
        (target) => !releaseTargetKeys.includes(target as ReleaseTargetKey),
      )
    ) {
      throw new ReleaseCoordinatorError(
        "frozen_plan_mismatch",
        "coordinator transaction is missing or belongs to another plan",
      );
    }
    for (const target of releaseTargetKeys) {
      const row = state.targets[target];
      if (row?.state !== "completed") continue;
      if (
        row.receipt === undefined ||
        row.leaseId === undefined ||
        row.leaseExpiresAt === undefined
      ) {
        throw new ReleaseCoordinatorError(
          "worker_receipt_invalid",
          "persisted completed receipt has no lease binding",
          target,
        );
      }
      const request: WorkerDispatchRequest = {
        transactionRef: plan.transactionRef,
        plan: authority,
        planDigest: digest,
        target,
        formats: formatsByTarget[target],
        lease: { id: row.leaseId, attempt: row.attempt, expiresAt: row.leaseExpiresAt },
        auth: { alg: "ed25519", kid: deps.requestSigner.kid, value: "persisted" },
      };
      verifyWorkerReceipt(
        row.receipt,
        request,
        inventory.get(target)!,
        deps.workerKeyring,
        new Date(row.receipt.payload.completedAt),
      );
    }
    if (state.phase === "candidate" || state.phase === "promoted") {
      if (
        state.candidate === undefined ||
        !PUBLIC_REF.test(state.candidate.candidateRef) ||
        !PUBLIC_REF.test(state.candidate.previousPointerRef) ||
        !SHA256.test(state.candidate.releaseSetPayloadSha256)
      ) {
        throw new ReleaseCoordinatorError(
          "candidate_handoff_refused",
          "persisted candidate handoff is invalid",
        );
      }
    }
    if (
      state.phase === "promoted" &&
      (state.promotionRef === undefined || !PUBLIC_REF.test(state.promotionRef))
    ) {
      throw new ReleaseCoordinatorError(
        "promotion_precondition_failed",
        "persisted promotion reference is invalid",
      );
    }
    return state;
  };

  const save = (state: CoordinatorState): Promise<CoordinatorState> =>
    deps.stateStore.save(state, state.revision);

  const stopAll = async (): Promise<void> => {
    await Promise.allSettled(
      [...inventory.values()].map((entry) => deps.workerControl.stop(entry)),
    );
  };

  return {
    kind: "real",
    checkWorkerInventory: async (plan): Promise<PortCallResult> => {
      const frozen = planFromReleasePlan(plan, authority);
      const existing = await deps.stateStore.load(plan.transactionRef);
      if (existing !== undefined) {
        const resumed = await loadRequired(plan);
        return {
          receiptLines: [
            redactedLine(`coordinator: resumed ${plan.transactionRef} at ${resumed.phase}`),
          ],
        };
      }
      const targets = Object.fromEntries(
        releaseTargetKeys.map((target) => [
          target,
          { workerRef: inventory.get(target)!.workerRef, state: "inventory_verified", attempt: 0 },
        ]),
      ) as CoordinatorState["targets"];
      await deps.stateStore.save(
        {
          schema: COORDINATOR_SCHEMA,
          revision: 0,
          transactionRef: plan.transactionRef,
          planDigest: digest,
          frozenPlan: frozen,
          phase: "inventory",
          targets,
        },
        0,
      );
      return {
        receiptLines: [
          redactedLine(`coordinator: exact 6-target inventory bound to plan sha256:${digest}`),
        ],
      };
    },

    bringUpWorkers: async (plan): Promise<PortCallResult> => {
      let state = await loadRequired(plan);
      if (state.phase !== "inventory")
        return { receiptLines: [`coordinator: workers already reconciled at ${state.phase}`] };
      const outcomes = await Promise.allSettled(
        releaseTargetKeys.map(async (target) => {
          const entry = inventory.get(target)!;
          await deps.workerControl.start(entry);
          const health = await deps.workerControl.health(entry);
          if (
            health.state !== "healthy" ||
            health.target !== target ||
            health.workerRef !== entry.workerRef ||
            health.observedToolchainProfileRef !== authority.toolchainProfileRef
          ) {
            throw new ReleaseCoordinatorError(
              "worker_unhealthy",
              `health mismatch for ${target}`,
              target,
            );
          }
          return target;
        }),
      );
      const failed = outcomes.find((outcome) => outcome.status === "rejected");
      if (failed !== undefined) {
        await stopAll();
        throw failed.reason;
      }
      state = await save({
        ...state,
        phase: "workers_ready",
        targets: Object.fromEntries(
          releaseTargetKeys.map((target) => [
            target,
            { ...state.targets[target]!, state: "healthy" },
          ]),
        ),
      });
      return {
        receiptLines: [
          redactedLine(
            `coordinator: ${releaseTargetKeys.length}/6 workers healthy; idle-stop is mandatory on exit`,
          ),
        ],
      };
    },

    fanOutTargets: async (plan): Promise<PortCallResult> => {
      const state = await loadRequired(plan);
      if (["converged", "gates_passed", "candidate", "promoted"].includes(state.phase))
        return {
          receiptLines: [`coordinator: matrix already converged sha256:${state.matrixDigest}`],
        };
      if (state.phase !== "workers_ready")
        throw new ReleaseCoordinatorError("matrix_incomplete", "workers are not ready");
      let durableState = state;
      let persistQueue: Promise<void> = Promise.resolve();
      const persistTarget = async (target: ReleaseTargetKey, row: TargetState): Promise<void> => {
        const operation = persistQueue.then(async () => {
          durableState = await save({
            ...durableState,
            targets: { ...durableState.targets, [target]: row },
          });
        });
        persistQueue = operation.catch(() => undefined);
        await operation;
      };
      const outcomes = await Promise.allSettled(
        releaseTargetKeys.map(async (target) => {
          const prior = durableState.targets[target]!;
          if (prior.state === "completed" && prior.receipt !== undefined) return;
          const entry = inventory.get(target)!;
          let health = await deps.workerControl.health(entry).catch(() => undefined);
          if (
            health === undefined ||
            health.state !== "healthy" ||
            health.observedToolchainProfileRef !== authority.toolchainProfileRef
          ) {
            await deps.workerControl.start(entry);
            health = await deps.workerControl.health(entry);
          }
          if (
            health.state !== "healthy" ||
            health.target !== target ||
            health.workerRef !== entry.workerRef ||
            health.observedToolchainProfileRef !== authority.toolchainProfileRef
          ) {
            throw new ReleaseCoordinatorError(
              "worker_unhealthy",
              `health mismatch for ${target}`,
              target,
            );
          }
          let lastError: unknown;
          for (let attempt = prior.attempt + 1; attempt <= maxAttempts; attempt += 1) {
            const leaseId = `lease:${sha256(`${plan.transactionRef}:${target}:${attempt}:${digest}`).slice(0, 32)}`;
            const expiresAt = new Date(deps.now().getTime() + leaseDurationMs).toISOString();
            const unsigned = {
              transactionRef: plan.transactionRef,
              plan: authority,
              planDigest: digest,
              target,
              formats: formatsByTarget[target],
              lease: { id: leaseId, attempt, expiresAt },
            };
            const authValue = await deps.requestSigner.sign(Buffer.from(canonicalJson(unsigned)));
            const request: WorkerDispatchRequest = {
              ...unsigned,
              auth: { alg: "ed25519", kid: deps.requestSigner.kid, value: authValue },
            };
            await persistTarget(target, {
              workerRef: entry.workerRef,
              state: "leased",
              attempt,
              leaseId,
              leaseExpiresAt: expiresAt,
            });
            try {
              const heartbeat = await deps.workerControl.heartbeat(entry, leaseId);
              if (!heartbeat.alive)
                throw new ReleaseCoordinatorError(
                  "worker_unhealthy",
                  "lease heartbeat refused",
                  target,
                );
              const remainingMs = Math.max(1, Date.parse(expiresAt) - deps.now().getTime());
              let timeout: ReturnType<typeof setTimeout> | undefined;
              const receipt = await Promise.race([
                deps.workerControl.dispatch(entry, request),
                new Promise<never>((_resolve, reject) => {
                  timeout = setTimeout(
                    () =>
                      reject(
                        new ReleaseCoordinatorError(
                          "lease_stale",
                          "worker lease timed out",
                          target,
                        ),
                      ),
                    remainingMs,
                  );
                  timeout.unref?.();
                }),
              ]).finally(() => {
                if (timeout !== undefined) clearTimeout(timeout);
              });
              verifyWorkerReceipt(receipt, request, entry, deps.workerKeyring, deps.now());
              await persistTarget(target, {
                workerRef: entry.workerRef,
                state: "completed",
                attempt,
                leaseId,
                leaseExpiresAt: expiresAt,
                receipt,
              });
              lastError = undefined;
              break;
            } catch (error) {
              lastError = error;
              await deps.workerControl.cancel(entry, leaseId).catch(() => undefined);
              await persistTarget(target, {
                workerRef: entry.workerRef,
                state: "failed",
                attempt,
                blockerRef:
                  error instanceof ReleaseCoordinatorError
                    ? `blocker:${error.code}`
                    : "blocker:worker_failed",
              });
            }
          }
          if (lastError !== undefined) throw lastError;
        }),
      );
      const failure = outcomes.find((outcome) => outcome.status === "rejected");
      if (failure !== undefined) {
        await stopAll();
        throw failure.reason;
      }
      const receipts = releaseTargetKeys.map(
        (target) => durableState.targets[target]?.receipt?.payload,
      );
      if (receipts.some((receipt) => receipt === undefined))
        throw new ReleaseCoordinatorError("matrix_incomplete", "not every target completed");
      const matrixDigest = sha256(canonicalJson(receipts));
      await save({ ...durableState, phase: "converged", matrixDigest });
      return {
        receiptLines: [
          redactedLine(`coordinator: 6 targets / 12 artifacts converged sha256:${matrixDigest}`),
        ],
      };
    },

    runReleaseGates: async (plan): Promise<PortCallResult> => {
      const state = await loadRequired(plan);
      if (["gates_passed", "candidate", "promoted"].includes(state.phase))
        return { receiptLines: [`coordinator: release gates already passed`] };
      if (state.phase !== "converged" || state.matrixDigest === undefined)
        throw new ReleaseCoordinatorError("matrix_incomplete", "matrix has not converged");
      for (const target of releaseTargetKeys) {
        const receipt = state.targets[target]?.receipt?.payload;
        if (
          receipt === undefined ||
          Object.values(receipt.nativeProofRefs).some((ref) => !PUBLIC_REF.test(ref))
        )
          throw new ReleaseCoordinatorError(
            "matrix_incomplete",
            `native proof missing for ${target}`,
            target,
          );
      }
      await save({ ...state, phase: "gates_passed" });
      return {
        receiptLines: [
          redactedLine(
            `coordinator: native/signing prerequisites verified for exact matrix sha256:${state.matrixDigest}`,
          ),
        ],
      };
    },

    publishCandidate: async (plan): Promise<PortCallResult> => {
      let state = await loadRequired(plan);
      if (state.phase === "candidate" || state.phase === "promoted")
        return {
          receiptLines: [
            `coordinator: candidate already published ${state.candidate!.candidateRef}`,
          ],
        };
      if (state.phase !== "gates_passed" || state.matrixDigest === undefined)
        throw new ReleaseCoordinatorError("matrix_incomplete", "release gates have not passed");
      const receipts = releaseTargetKeys.map((target) => state.targets[target]!.receipt!.payload);
      for (const receipt of receipts) {
        for (const artifact of receipt.artifacts) {
          const observed = await deps.objectStore.headImmutable(artifact.objectKey);
          if (
            !observed.exists ||
            observed.sha256 !== artifact.sha256 ||
            observed.byteLength !== artifact.byteLength
          ) {
            throw new ReleaseCoordinatorError(
              "candidate_object_invalid",
              `candidate object mismatch for ${receipt.target}/${artifact.format}`,
              receipt.target,
            );
          }
        }
      }
      const candidate = await deps.candidatePublisher.publishVerifiedCandidate({
        transactionRef: plan.transactionRef,
        plan: authority,
        planDigest: digest,
        matrixDigest: state.matrixDigest,
        receipts,
      });
      for (const ref of [candidate.candidateRef, candidate.previousPointerRef])
        assertRef(ref, "candidate handoff");
      if (!SHA256.test(candidate.releaseSetPayloadSha256))
        throw new ReleaseCoordinatorError(
          "candidate_handoff_refused",
          "ReleaseSet payload digest invalid",
        );
      state = await save({ ...state, phase: "candidate", candidate });
      await stopAll();
      return {
        receiptLines: [
          redactedLine(
            `coordinator: immutable candidate ${candidate.candidateRef} handed to candidate feed`,
          ),
        ],
      };
    },

    promoteChannelPointer: async (plan): Promise<PortCallResult> => {
      let state = await loadRequired(plan);
      if (state.phase === "promoted")
        return { receiptLines: [`coordinator: pointer already promoted ${state.promotionRef}`] };
      if (
        state.phase !== "candidate" ||
        state.candidate === undefined ||
        state.matrixDigest === undefined
      )
        throw new ReleaseCoordinatorError(
          "promotion_precondition_failed",
          "candidate is not complete",
        );
      const acceptance = await deps.acceptanceGate.verifyCandidate({
        transactionRef: plan.transactionRef,
        candidateRef: state.candidate.candidateRef,
        releaseSetPayloadSha256: state.candidate.releaseSetPayloadSha256,
        planDigest: digest,
        matrixDigest: state.matrixDigest,
      });
      if (!acceptance.accepted || acceptance.receiptRef === undefined)
        throw new ReleaseCoordinatorError(
          "promotion_precondition_failed",
          "candidate feed smoke missing",
        );
      assertRef(acceptance.receiptRef, "candidate acceptance receipt");
      const result = await deps.promoter.compareAndSwap({
        channel: authority.channel,
        expectedPreviousPointerRef: state.candidate.previousPointerRef,
        candidateRef: state.candidate.candidateRef,
        releaseSetPayloadSha256: state.candidate.releaseSetPayloadSha256,
        acceptanceReceiptRef: acceptance.receiptRef,
      });
      if (!result.promoted || result.pointerRef === undefined) {
        throw new ReleaseCoordinatorError(
          "promotion_race",
          "channel pointer changed before promotion",
        );
      }
      assertRef(result.pointerRef, "promotion pointer");
      state = await save({ ...state, phase: "promoted", promotionRef: result.pointerRef });
      await stopAll();
      return {
        receiptLines: [
          redactedLine(
            `coordinator: atomic ${authority.channel} pointer promoted to ${result.pointerRef}`,
          ),
        ],
      };
    },
  };
};
