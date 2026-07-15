import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

import { describe, expect, test } from "vite-plus/test";

import {
  DesktopNativeSidecarNodeVersion,
  DesktopNativeSidecarProtocol,
  DesktopNativeSidecarRpcProtocol,
  decodeDesktopNativeSidecarBootstrapReceipt,
  decodeDesktopNativeSidecarBootstrapRequest,
  decodeDesktopNativeSidecarReadyReceipt,
  decodeDesktopNativeSidecarRpcResponse,
  encodeDesktopNativeSidecarPath,
  executeDesktopNativeSidecarBootstrap,
  openDesktopNativeSidecarService,
  type DesktopNativeSidecarBootstrapRequest,
} from "./native-sidecar-contract.ts";

const fixture = () => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-native-sidecar-"));
  const state = path.join(root, "state");
  const freshState = path.join(root, "fresh-state");
  const repository = path.join(root, "repository-one");
  const secondRepository = path.join(root, "repository-two");
  const nonRepository = path.join(root, "plain-directory");
  const fakeRepository = path.join(root, "fake-repository");
  for (const directory of [repository, secondRepository, nonRepository, fakeRepository]) {
    mkdirSync(directory);
  }
  mkdirSync(path.join(fakeRepository, ".git"));
  // Git hooks export repository-local variables such as GIT_DIR. A fixture
  // `git init <tmp>` must not inherit them or it can reconfigure the checkout
  // whose pre-push hook is running instead of the temporary repository.
  const isolatedGitEnv = Object.fromEntries(
    Object.entries(process.env).filter(([key]) => ![
      "GIT_ALTERNATE_OBJECT_DIRECTORIES",
      "GIT_COMMON_DIR",
      "GIT_DIR",
      "GIT_INDEX_FILE",
      "GIT_OBJECT_DIRECTORY",
      "GIT_WORK_TREE",
    ].includes(key)),
  );
  for (const repositoryRoot of [repository, secondRepository]) {
    const initialized = spawnSync("git", ["init", "--quiet", repositoryRoot], {
      env: isolatedGitEnv,
    });
    if (initialized.status !== 0) throw new Error("Git fixture initialization failed.");
  }
  return {
    root,
    state,
    freshState,
    repository,
    secondRepository,
    nonRepository,
    fakeRepository,
    close: () => rmSync(root, { recursive: true, force: true }),
  };
};

const bootstrap = (stateRoot: string, generation = 7): DesktopNativeSidecarBootstrapRequest => ({
  protocol: DesktopNativeSidecarProtocol,
  generation,
  nonce: `proof.native_${generation}`,
  stateRootBase64: encodeDesktopNativeSidecarPath(stateRoot),
  transportToken: String(generation).padStart(64, "0"),
});

const rpc = (
  input: DesktopNativeSidecarBootstrapRequest,
  requestId: string,
  operation: "coding.snapshot" | "coding.admit",
  root?: string,
) =>
  operation === "coding.snapshot"
    ? ({
        protocol: DesktopNativeSidecarRpcProtocol,
        generation: input.generation,
        nonce: input.nonce,
        requestId,
        operation,
      } as const)
    : ({
        protocol: DesktopNativeSidecarRpcProtocol,
        generation: input.generation,
        nonce: input.nonce,
        requestId,
        operation,
        rootBase64: encodeDesktopNativeSidecarPath(root!),
      } as const);

describe("Desktop Native sidecar contract", () => {
  test("executes the production runtime gateway v11 bootstrap on exact Node 24", async () => {
    const h = fixture();
    try {
      const receipt = await executeDesktopNativeSidecarBootstrap(bootstrap(h.state), {
        nodeVersion: DesktopNativeSidecarNodeVersion,
        pid: 4242,
      });

      expect(receipt).toMatchObject({
        protocol: DesktopNativeSidecarProtocol,
        generation: 7,
        nonce: "proof.native_7",
        pid: 4242,
        nodeVersion: DesktopNativeSidecarNodeVersion,
        gatewayProtocolVersion: 11,
        requestId: "native-sidecar.bootstrap",
        response: {
          kind: "query_result",
          requestId: "native-sidecar.bootstrap",
          result: { kind: "runtime.bootstrap", lifecycle: "ready", protocolVersion: 11 },
        },
      });
      expect(decodeDesktopNativeSidecarBootstrapReceipt(receipt)).toEqual(receipt);
      expect(
        decodeDesktopNativeSidecarReadyReceipt({
          ...receipt,
          transport: {
            kind: "loopback_http",
            host: "127.0.0.1",
            port: 43_123,
          },
        }),
      ).not.toBeNull();
    } finally {
      h.close();
    }
  });

  test("retains opaque work identity across sidecar generations and separates repositories and profiles", async () => {
    const h = fixture();
    try {
      const firstBootstrap = bootstrap(h.state, 1);
      const first = await openDesktopNativeSidecarService(firstBootstrap, {
        nodeVersion: DesktopNativeSidecarNodeVersion,
        pid: 4_201,
      });
      const admitted = await first.execute(
        rpc(firstBootstrap, "coding.first", "coding.admit", h.repository),
      );
      const firstAdmission =
        admitted.result.kind === "coding.admitted" ? admitted.result.admission : null;
      expect(firstAdmission).not.toBeNull();
      expect(decodeDesktopNativeSidecarRpcResponse(admitted)).toEqual(admitted);
      expect(JSON.stringify(admitted)).not.toContain(h.root);
      first.dispose();

      const restartedBootstrap = bootstrap(h.state, 2);
      const restarted = await openDesktopNativeSidecarService(restartedBootstrap, {
        nodeVersion: DesktopNativeSidecarNodeVersion,
        pid: 9_902,
      });
      const restored = await restarted.execute(
        rpc(restartedBootstrap, "coding.restored", "coding.snapshot"),
      );
      const restoredProjection = restored.result.projection;
      expect(restoredProjection.selectedSessionRef).toBe(firstAdmission?.sessionRef);
      expect(restoredProjection.sessions[0]).toMatchObject({
        sessionRef: firstAdmission?.sessionRef,
        workContextRef: firstAdmission?.workContextRef,
        grantRef: firstAdmission?.grantRef,
      });

      const second = await restarted.execute(
        rpc(restartedBootstrap, "coding.second", "coding.admit", h.secondRepository),
      );
      expect(second.result.kind).toBe("coding.admitted");
      expect(
        second.result.kind === "coding.admitted" ? second.result.admission.sessionRef : null,
      ).not.toBe(firstAdmission?.sessionRef);
      expect(
        second.result.kind === "coding.admitted" ? second.result.admission.workContextRef : null,
      ).not.toBe(firstAdmission?.workContextRef);
      expect(JSON.stringify(second)).not.toContain(h.root);
      restarted.dispose();

      const freshBootstrap = bootstrap(h.freshState, 3);
      const fresh = await openDesktopNativeSidecarService(freshBootstrap, {
        nodeVersion: DesktopNativeSidecarNodeVersion,
        pid: 7_703,
      });
      const freshAdmission = await fresh.execute(
        rpc(freshBootstrap, "coding.fresh", "coding.admit", h.repository),
      );
      expect(freshAdmission.result.kind).toBe("coding.admitted");
      expect(
        freshAdmission.result.kind === "coding.admitted"
          ? freshAdmission.result.admission.sessionRef
          : null,
      ).not.toBe(firstAdmission?.sessionRef);
      expect(
        freshAdmission.result.kind === "coding.admitted"
          ? freshAdmission.result.admission.workContextRef
          : null,
      ).not.toBe(firstAdmission?.workContextRef);
      fresh.dispose();
    } finally {
      h.close();
    }
  });

  test("refuses a non-repository without mutating the durable catalog", async () => {
    const h = fixture();
    try {
      const input = bootstrap(h.state, 4);
      const service = await openDesktopNativeSidecarService(input, {
        nodeVersion: DesktopNativeSidecarNodeVersion,
        pid: 4_404,
      });
      const before = await service.execute(rpc(input, "coding.before", "coding.snapshot"));
      const refused = await service.execute(
        rpc(input, "coding.refused", "coding.admit", h.nonRepository),
      );
      expect(refused.result).toMatchObject({ kind: "coding.refused", reason: "not_repository" });
      const after = await service.execute(rpc(input, "coding.after", "coding.snapshot"));
      expect(after.result.projection).toEqual(before.result.projection);
      expect(after.result.projectionDigest).toBe(before.result.projectionDigest);

      const fake = await service.execute(
        rpc(input, "coding.fake", "coding.admit", h.fakeRepository),
      );
      expect(fake.result).toMatchObject({ kind: "coding.refused", reason: "not_repository" });
      expect(fake.result.projection).toEqual(before.result.projection);
      expect(fake.result.projectionDigest).toBe(before.result.projectionDigest);
      service.dispose();
    } finally {
      h.close();
    }
  });

  test("replays identical request IDs and refuses conflicting reuse", async () => {
    const h = fixture();
    try {
      const input = bootstrap(h.state, 6);
      const service = await openDesktopNativeSidecarService(input, {
        nodeVersion: DesktopNativeSidecarNodeVersion,
        pid: 6_606,
      });
      const firstRequest = rpc(input, "coding.replay", "coding.admit", h.repository);
      const first = await service.execute(firstRequest);
      expect(await service.execute(firstRequest)).toEqual(first);

      const conflict = await service.execute(
        rpc(input, "coding.replay", "coding.admit", h.secondRepository),
      );
      expect(conflict.result).toMatchObject({
        kind: "coding.refused",
        reason: "request_conflict",
      });
      expect(conflict.result.projection).toEqual(first.result.projection);
      service.dispose();
    } finally {
      h.close();
    }
  });

  test("rejects malformed generation, nonce, state root, and excess protocol values", () => {
    const stateRootBase64 = encodeDesktopNativeSidecarPath("/tmp/openagents-native-test");
    expect(
      decodeDesktopNativeSidecarBootstrapRequest({
        protocol: DesktopNativeSidecarProtocol,
        generation: 0,
        nonce: "proof",
        stateRootBase64,
        transportToken: "a".repeat(64),
      }),
    ).toBeNull();
    expect(
      decodeDesktopNativeSidecarBootstrapRequest({
        protocol: DesktopNativeSidecarProtocol,
        generation: 1,
        nonce: "../shared",
        stateRootBase64,
        transportToken: "a".repeat(64),
      }),
    ).toBeNull();
    expect(
      decodeDesktopNativeSidecarBootstrapRequest({
        protocol: DesktopNativeSidecarProtocol,
        generation: 1,
        nonce: "proof",
        stateRootBase64: "not-base64",
        transportToken: "a".repeat(64),
      }),
    ).toBeNull();
    expect(
      decodeDesktopNativeSidecarBootstrapRequest({
        protocol: "openagents.desktop.native-sidecar.v1",
        generation: 1,
        nonce: "proof",
        stateRootBase64,
        transportToken: "a".repeat(64),
      }),
    ).toBeNull();
    expect(
      decodeDesktopNativeSidecarBootstrapRequest({
        protocol: DesktopNativeSidecarProtocol,
        generation: 1,
        nonce: "proof",
        stateRootBase64,
        transportToken: "a".repeat(64),
        ambientPath: "/private/repository",
      }),
    ).toBeNull();
  });

  test("refuses an ambient or mismatched Node runtime", async () => {
    const h = fixture();
    try {
      await expect(
        executeDesktopNativeSidecarBootstrap(bootstrap(h.state, 5), {
          nodeVersion: "22.0.0",
          pid: 4_242,
        }),
      ).rejects.toThrow("requires Node 24.13.1");
    } finally {
      h.close();
    }
  });
});
