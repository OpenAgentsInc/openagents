import { generateKeyPairSync, sign } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, test } from "vite-plus/test";

import {
  COORDINATOR_SCHEMA,
  FileCoordinatorStateStore,
  ReleaseCoordinatorError,
  WORKER_RECEIPT_SCHEMA,
  canonicalJson,
  createOwnedReleaseCoordinator,
  formatsByTarget,
  sha256,
  type AtomicChannelPromoter,
  type CandidateAcceptanceGate,
  type CandidateHandoff,
  type CandidateObjectStore,
  type CandidatePublisher,
  type CoordinatorRequestSigner,
  type CoordinatorState,
  type CoordinatorStateStore,
  type FrozenReleaseAuthority,
  type RunnerInventoryEntry,
  type SignedWorkerReceipt,
  type WorkerControl,
  type WorkerDispatchRequest,
  type WorkerKeyring,
} from "./desktop-release-coordinator.js";
import {
  newTransactionRef,
  releaseTargetKeys,
  type ReleasePlan,
  type ReleaseTargetKey,
} from "./release.js";

const SOURCE = "a".repeat(40);
const NOW = new Date("2026-07-16T20:00:00Z");
const TOOLCHAIN = "toolchain:desktop-release-2026-07-16";

const authority: FrozenReleaseAuthority = {
  sourceRevision: SOURCE,
  version: "0.1.0-rc.18",
  channel: "rc",
  targets: releaseTargetKeys,
  stagingLedgerRef: `sha256:${"b".repeat(64)}`,
  signingPolicyId: "signing:desktop-production-v2",
  toolchainProfileRef: TOOLCHAIN,
  releaseNotesSha256: "c".repeat(64),
};

const plan = (overrides: Partial<ReleasePlan> = {}): ReleasePlan => ({
  transactionRef: newTransactionRef(authority.version, authority.channel, NOW),
  mode: "real",
  version: authority.version,
  channel: authority.channel,
  sourceRevision: authority.sourceRevision,
  targets: releaseTargetKeys,
  date: "2026-07-16",
  unattended: false,
  approvedGates: [],
  ...overrides,
});

class MemoryStore implements CoordinatorStateStore {
  readonly rows = new Map<string, CoordinatorState>();
  async load(ref: string): Promise<CoordinatorState | undefined> {
    return this.rows.get(ref);
  }
  async save(state: CoordinatorState, expectedRevision: number): Promise<CoordinatorState> {
    const observed = this.rows.get(state.transactionRef)?.revision ?? 0;
    if (observed !== expectedRevision) {
      throw new ReleaseCoordinatorError("state_conflict", "fixture CAS conflict");
    }
    const next = { ...state, revision: expectedRevision + 1 };
    this.rows.set(state.transactionRef, next);
    return next;
  }
}

const artifactName = (target: ReleaseTargetKey, format: string): string => {
  const [platform, architecture] = target.split("-");
  if (platform === "darwin")
    return `OpenAgents-${authority.version}-rc-darwin-${architecture}.${format}`;
  if (platform === "win32")
    return `OpenAgents-${authority.version}-rc-win32-${architecture}-setup.exe`;
  return `OpenAgents-${authority.version}-rc-linux-${architecture}.${format === "appimage" ? "AppImage" : format}`;
};

type Fixture = ReturnType<typeof makeFixture>;

const makeFixture = (
  options: {
    unavailableTarget?: ReleaseTargetKey;
    failFirstTarget?: ReleaseTargetKey;
    corruptReceiptTarget?: ReleaseTargetKey | undefined;
    staleLeaseTarget?: ReleaseTargetKey;
    duplicateFormatTarget?: ReleaseTargetKey;
    invalidSigningTarget?: ReleaseTargetKey;
    dispatchDelayMs?: number;
    badObjectTarget?: ReleaseTargetKey;
    rejectAcceptance?: boolean;
    losePromotionRace?: boolean;
    malformedReceipt?: Readonly<{ target: ReleaseTargetKey; value: unknown }>;
  } = {},
) => {
  const behavior = { ...options };
  const keypairs = Object.fromEntries(
    releaseTargetKeys.map((target) => [target, generateKeyPairSync("ed25519")]),
  ) as Record<ReleaseTargetKey, ReturnType<typeof generateKeyPairSync>>;
  const inventory: RunnerInventoryEntry[] = releaseTargetKeys.map((target) => ({
    workerRef: `worker:${target}`,
    target,
    hostClass: `owned-${target}`,
    buildMode: "native",
    nativeAcceptanceHostRef:
      behavior.unavailableTarget === target ? "unavailable:native-host" : `acceptance:${target}`,
    toolchainProfileRef: TOOLCHAIN,
    signingOperationRef: `signer:${target}`,
    enabled: true,
  }));
  const calls: string[] = [];
  const dispatchCounts = new Map<ReleaseTargetKey, number>();
  let activeDispatches = 0;
  let maxConcurrentDispatches = 0;
  const objects = new Map<string, { sha256: string; byteLength: number }>();

  const makeReceipt = (request: WorkerDispatchRequest): SignedWorkerReceipt => {
    const count = (dispatchCounts.get(request.target) ?? 0) + 1;
    dispatchCounts.set(request.target, count);
    const payload = {
      schema: WORKER_RECEIPT_SCHEMA,
      transactionRef: request.transactionRef,
      planDigest: request.planDigest,
      leaseId: request.lease.id,
      attempt: request.lease.attempt,
      workerRef: `worker:${request.target}`,
      target: request.target,
      sourceRevision: request.plan.sourceRevision,
      version: request.plan.version,
      channel: request.plan.channel,
      stagingLedgerRef: request.plan.stagingLedgerRef,
      toolchainProfileRef: request.plan.toolchainProfileRef,
      signingPolicyId: request.plan.signingPolicyId,
      artifacts: formatsByTarget[request.target].map((format, index) => {
        const digest = sha256(`${request.target}:${format}`);
        const objectKey = `desktop/candidate/${request.plan.version}/${request.target}/${artifactName(request.target, format)}`;
        objects.set(objectKey, { sha256: digest, byteLength: 100 + index });
        return {
          format,
          name: artifactName(request.target, format),
          objectKey,
          sha256: digest,
          byteLength: 100 + index,
          componentLedgerRef: `ledger:${request.target}`,
          buildReceiptRef: `build:${request.target}:${format}`,
          signingReceiptRef: `signing:${request.target}:${format}`,
        };
      }),
      nativeProofRefs: {
        cleanInstall: `proof:${request.target}:install`,
        launch: `proof:${request.target}:launch`,
        agentRuntime: `proof:${request.target}:runtime`,
        shutdown: `proof:${request.target}:shutdown`,
        update: `proof:${request.target}:update`,
        interruptionResume: `proof:${request.target}:resume`,
        rollbackOrNoRollback: `proof:${request.target}:rollback`,
        reinstall: `proof:${request.target}:reinstall`,
        uninstall: `proof:${request.target}:uninstall`,
      },
      completedAt: NOW.toISOString(),
    } as const;
    let signedPayload =
      behavior.corruptReceiptTarget === request.target
        ? { ...payload, sourceRevision: "d".repeat(40) }
        : payload;
    if (behavior.staleLeaseTarget === request.target) {
      signedPayload = { ...signedPayload, leaseId: "lease:stale" };
    }
    if (behavior.duplicateFormatTarget === request.target) {
      signedPayload = {
        ...signedPayload,
        artifacts: signedPayload.artifacts.map((artifact, index) =>
          index === 1 ? { ...artifact, format: signedPayload.artifacts[0]!.format } : artifact,
        ),
      };
    }
    if (behavior.invalidSigningTarget === request.target) {
      signedPayload = {
        ...signedPayload,
        artifacts: signedPayload.artifacts.map((artifact) => ({
          ...artifact,
          signingReceiptRef: "secret signing output",
        })),
      };
    }
    return {
      payload: signedPayload,
      signature: {
        alg: "ed25519",
        kid: `key:${request.target}`,
        value: sign(
          null,
          Buffer.from(canonicalJson(signedPayload)),
          keypairs[request.target].privateKey,
        ).toString("base64url"),
      },
    };
  };

  const workerControl: WorkerControl = {
    start: async (entry) => {
      calls.push(`start:${entry.target}`);
    },
    health: async (entry) => ({
      workerRef: entry.workerRef,
      target: entry.target,
      state: "healthy",
      observedToolchainProfileRef: TOOLCHAIN,
      observedAt: NOW.toISOString(),
    }),
    heartbeat: async (entry) => ({ alive: (calls.push(`heartbeat:${entry.target}`), true) }),
    dispatch: async (_entry, request) => {
      calls.push(`dispatch:${request.target}:${request.lease.attempt}`);
      if (behavior.malformedReceipt?.target === request.target) {
        return behavior.malformedReceipt.value as SignedWorkerReceipt;
      }
      if (
        behavior.failFirstTarget === request.target &&
        (dispatchCounts.get(request.target) ?? 0) === 0
      ) {
        dispatchCounts.set(request.target, 1);
        throw new Error("transient worker loss");
      }
      activeDispatches += 1;
      maxConcurrentDispatches = Math.max(maxConcurrentDispatches, activeDispatches);
      try {
        if (behavior.dispatchDelayMs !== undefined) {
          await new Promise((resolve) => setTimeout(resolve, behavior.dispatchDelayMs));
        }
        return makeReceipt(request);
      } finally {
        activeDispatches -= 1;
      }
    },
    cancel: async (entry, lease) => {
      calls.push(`cancel:${entry.target}:${lease}`);
    },
    stop: async (entry) => {
      calls.push(`stop:${entry.target}`);
    },
  };
  const requestSigner: CoordinatorRequestSigner = {
    kid: "coordinator:key-1",
    sign: (bytes) => sha256(bytes),
  };
  const workerKeyring: WorkerKeyring = {
    publicKey: (workerRef, kid) => {
      const target = releaseTargetKeys.find(
        (value) => workerRef === `worker:${value}` && kid === `key:${value}`,
      );
      return target === undefined
        ? undefined
        : keypairs[target].publicKey.export({ format: "pem", type: "spki" }).toString();
    },
  };
  const objectStore: CandidateObjectStore = {
    headImmutable: async (key) => {
      const value = objects.get(key);
      if (value === undefined) return { exists: false };
      if (behavior.badObjectTarget !== undefined && key.includes(`/${behavior.badObjectTarget}/`)) {
        return { exists: true, sha256: "0".repeat(64), byteLength: value.byteLength };
      }
      return { exists: true, ...value };
    },
  };
  const candidate: CandidateHandoff = {
    candidateRef: "candidate:rc-18",
    releaseSetPayloadSha256: "e".repeat(64),
    previousPointerRef: "pointer:rc-17",
  };
  const candidatePublisher: CandidatePublisher = {
    publishVerifiedCandidate: async (input) => {
      calls.push(`publish:${input.receipts.length}:${input.matrixDigest}`);
      return candidate;
    },
  };
  const acceptanceGate: CandidateAcceptanceGate = {
    verifyCandidate: async () =>
      behavior.rejectAcceptance
        ? { accepted: false, blockerRef: "blocker:candidate-smoke" }
        : { accepted: true, receiptRef: "acceptance:candidate-smoke" },
  };
  const promoter: AtomicChannelPromoter = {
    compareAndSwap: async () =>
      behavior.losePromotionRace
        ? { promoted: false, currentPointerRef: "pointer:other" }
        : { promoted: true, pointerRef: "pointer:rc-18" },
  };
  const store = new MemoryStore();
  return {
    inventory,
    workerControl,
    requestSigner,
    workerKeyring,
    objectStore,
    candidatePublisher,
    acceptanceGate,
    promoter,
    store,
    calls,
    dispatchCounts,
    behavior,
    maxConcurrentDispatches: () => maxConcurrentDispatches,
  };
};

const coordinator = (fixture: Fixture, maxAttempts = 2) =>
  createOwnedReleaseCoordinator(authority, {
    inventory: fixture.inventory,
    workerControl: fixture.workerControl,
    requestSigner: fixture.requestSigner,
    workerKeyring: fixture.workerKeyring,
    objectStore: fixture.objectStore,
    candidatePublisher: fixture.candidatePublisher,
    acceptanceGate: fixture.acceptanceGate,
    promoter: fixture.promoter,
    stateStore: fixture.store,
    now: () => NOW,
    leaseDurationMs: 60_000,
    maxAttempts,
  });

const converge = async (fixture: Fixture, releasePlan = plan()) => {
  const port = coordinator(fixture);
  await port.checkWorkerInventory(releasePlan);
  await port.bringUpWorkers(releasePlan);
  await port.fanOutTargets(releasePlan);
  await port.runReleaseGates(releasePlan);
  return { port, releasePlan };
};

describe("owned Desktop release coordinator", () => {
  test("converges exactly five targets/eleven artifacts and atomically promotes only accepted candidate", async () => {
    const fixture = makeFixture();
    const { port, releasePlan } = await converge(fixture);
    await port.publishCandidate(releasePlan);
    const result = await port.promoteChannelPointer(releasePlan);

    expect(result.receiptLines[0]).toContain("atomic rc pointer promoted");
    const state = await fixture.store.load(releasePlan.transactionRef);
    expect(state).toMatchObject({
      schema: COORDINATOR_SCHEMA,
      phase: "promoted",
      promotionRef: "pointer:rc-18",
    });
    expect(Object.keys(state!.targets)).toEqual([...releaseTargetKeys]);
    expect(
      Object.values(state!.targets).flatMap((target) => target?.receipt?.payload.artifacts ?? []),
    ).toHaveLength(11);
    expect(fixture.calls.filter((call) => call.startsWith("stop:"))).toHaveLength(10);
  });

  test("fails closed when a required native Windows x64 host is unavailable", () => {
    const fixture = makeFixture({ unavailableTarget: "win32-x64" });
    expect(() => coordinator(fixture)).toThrowError(
      expect.objectContaining({ code: "worker_inventory_unavailable", target: "win32-x64" }),
    );
  });

  test("retries a lost worker under a new monotonic lease and converges", async () => {
    const fixture = makeFixture({ failFirstTarget: "linux-arm64" });
    await converge(fixture);
    expect(fixture.calls).toContain("dispatch:linux-arm64:1");
    expect(fixture.calls).toContain("dispatch:linux-arm64:2");
    expect(fixture.calls.some((call) => call.startsWith("cancel:linux-arm64:lease:"))).toBe(true);
  });

  test("fans independent targets out concurrently while serializing durable CAS writes", async () => {
    const fixture = makeFixture({ dispatchDelayMs: 5 });
    await converge(fixture);
    expect(fixture.maxConcurrentDispatches()).toBeGreaterThan(1);
  });

  test("refuses a signed receipt bound to another frozen source and stops every worker", async () => {
    const fixture = makeFixture({ corruptReceiptTarget: "darwin-x64" });
    const port = coordinator(fixture);
    const releasePlan = plan();
    await port.checkWorkerInventory(releasePlan);
    await port.bringUpWorkers(releasePlan);
    await expect(port.fanOutTargets(releasePlan)).rejects.toMatchObject({
      code: "worker_receipt_invalid",
      target: "darwin-x64",
    });
    expect(fixture.calls.filter((call) => call.startsWith("stop:"))).toHaveLength(5);
  });

  test.each([
    ["stale lease", { staleLeaseTarget: "darwin-arm64" as const }, "lease_stale"],
    [
      "duplicate format",
      { duplicateFormatTarget: "darwin-arm64" as const },
      "worker_receipt_invalid",
    ],
    [
      "invalid signing receipt",
      { invalidSigningTarget: "darwin-arm64" as const },
      "worker_receipt_invalid",
    ],
  ])(
    "refuses %s completion even when the worker signature is valid",
    async (_label, options, code) => {
      const fixture = makeFixture(options);
      const port = coordinator(fixture, 1);
      const releasePlan = plan();
      await port.checkWorkerInventory(releasePlan);
      await port.bringUpWorkers(releasePlan);
      await expect(port.fanOutTargets(releasePlan)).rejects.toMatchObject({
        code,
        target: "darwin-arm64",
      });
    },
  );

  test.each([
    ["missing envelope", {}],
    ["missing payload fields", { payload: {}, signature: {} }],
    [
      "non-canonical signature",
      {
        payload: {},
        signature: { alg: "ed25519", kid: "worker-key", value: "not-base64url" },
      },
    ],
  ])("rejects malformed untrusted worker receipts: %s", async (_label, value) => {
    const fixture = makeFixture({ malformedReceipt: { target: "linux-x64", value } });
    const port = coordinator(fixture, 1);
    const releasePlan = plan();
    await port.checkWorkerInventory(releasePlan);
    await port.bringUpWorkers(releasePlan);
    await expect(port.fanOutTargets(releasePlan)).rejects.toMatchObject({
      code: "worker_receipt_invalid",
      target: "linux-x64",
    });
  });

  test("resume preserves completed receipts and does not rerun successful targets", async () => {
    const fixture = makeFixture({ corruptReceiptTarget: "darwin-x64" });
    const releasePlan = plan();
    const first = coordinator(fixture, 1);
    await first.checkWorkerInventory(releasePlan);
    await first.bringUpWorkers(releasePlan);
    await expect(first.fanOutTargets(releasePlan)).rejects.toThrow();
    const armDispatches = fixture.calls.filter((call) =>
      call.startsWith("dispatch:darwin-arm64"),
    ).length;

    fixture.behavior.corruptReceiptTarget = undefined;
    await coordinator(fixture).fanOutTargets(releasePlan);
    expect(fixture.calls.filter((call) => call.startsWith("dispatch:darwin-arm64"))).toHaveLength(
      armDispatches,
    );
    expect(armDispatches).toBe(1);
  });

  test("candidate publication re-heads immutable objects and rejects byte drift", async () => {
    const fixture = makeFixture({ badObjectTarget: "linux-x64" });
    const { port, releasePlan } = await converge(fixture);
    await expect(port.publishCandidate(releasePlan)).rejects.toMatchObject({
      code: "candidate_object_invalid",
      target: "linux-x64",
    });
    expect(fixture.calls.some((call) => call.startsWith("publish:"))).toBe(false);
  });

  test("promotion refuses without candidate-feed acceptance and leaves pointer untouched", async () => {
    const fixture = makeFixture({ rejectAcceptance: true });
    const { port, releasePlan } = await converge(fixture);
    await port.publishCandidate(releasePlan);
    await expect(port.promoteChannelPointer(releasePlan)).rejects.toMatchObject({
      code: "promotion_precondition_failed",
    });
    expect((await fixture.store.load(releasePlan.transactionRef))?.phase).toBe("candidate");
  });

  test("compare-and-swap race fails without recording a promoted pointer", async () => {
    const fixture = makeFixture({ losePromotionRace: true });
    const { port, releasePlan } = await converge(fixture);
    await port.publishCandidate(releasePlan);
    await expect(port.promoteChannelPointer(releasePlan)).rejects.toMatchObject({
      code: "promotion_race",
    });
    expect((await fixture.store.load(releasePlan.transactionRef))?.promotionRef).toBeUndefined();
  });

  test("refuses transaction-ref reuse with a different frozen release command", async () => {
    const fixture = makeFixture();
    const port = coordinator(fixture);
    const releasePlan = plan();
    await port.checkWorkerInventory(releasePlan);
    await expect(
      port.checkWorkerInventory({ ...releasePlan, version: "0.1.0-rc.19" }),
    ).rejects.toMatchObject({ code: "frozen_plan_mismatch" });
  });
});

const tempDirs: string[] = [];
afterEach(() => {
  while (tempDirs.length > 0) rmSync(tempDirs.pop()!, { recursive: true, force: true });
});

describe("FileCoordinatorStateStore", () => {
  test("persists atomic revisions and refuses stale writers", async () => {
    const root = mkdtempSync(join(tmpdir(), "oa-dist04-store-"));
    tempDirs.push(root);
    const store = new FileCoordinatorStateStore(root);
    const transactionRef = newTransactionRef(authority.version, authority.channel, NOW);
    const initial: CoordinatorState = {
      schema: COORDINATOR_SCHEMA,
      revision: 0,
      transactionRef,
      planDigest: sha256(canonicalJson(authority)),
      frozenPlan: authority,
      phase: "inventory",
      targets: {},
    };
    const saved = await store.save(initial, 0);
    expect((await store.load(transactionRef))?.revision).toBe(1);
    await expect(store.save(saved, 0)).rejects.toMatchObject({ code: "state_conflict" });
  });
});
