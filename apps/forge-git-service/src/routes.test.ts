/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- @effect/vitest does not support the repository Effect 4 line. */
import { execFile } from "node:child_process";
import { createServer, type IncomingMessage, type Server } from "node:http";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

import { makeMemoryBlobStore } from "@openagentsinc/oa-infra/blob-store-memory";
import { Effect, Layer, ManagedRuntime } from "effect";
import { afterEach, describe, expect, test } from "vitest";

import {
  ForgeGitAdmission,
  type ForgeGitProjectedEvent,
  makeMemoryAdmissionLayer,
} from "./admission.js";
import { ForgeGitAuth, makePolicyAuthorityAuthLayer, makeStaticAuthLayer } from "./auth.js";
import { ForgeGitConfiguration, makeTestConfiguration } from "./config.js";
import { ForgeGitSession } from "./model.js";
import { ForgeGitProjection, layerNoopProjection } from "./projection.js";
import { layerNoopProjector } from "./projector.js";
import { ForgeGitRepository, makeRepositoryLayer } from "./repository.js";
import { routeRequest } from "./routes.js";
import { ForgeWebRead, makeForgeWebReadLayer } from "./web-read.js";
import {
  ForgeWebReadPolicy,
  makeForgeWebReadPolicyLayer,
  type ForgeWebReadPolicyFetch,
} from "./web-read-policy.js";

const execFileAsync = promisify(execFile);
const token = "oa_forge_git_0123456789abcdef0123456789abcdef";
const tenantRef = "owner.openagents";
const repositoryRef = "roundtrip";

const temporaryPaths: Array<string> = [];
const disposers: Array<() => Promise<void>> = [];

afterEach(async () => {
  await Promise.all(disposers.splice(0).map((dispose) => dispose()));
  await Promise.all(
    temporaryPaths.splice(0).map((path) => rm(path, { force: true, recursive: true })),
  );
});

const git = (cwd: string, args: ReadonlyArray<string>, environment: NodeJS.ProcessEnv = {}) =>
  execFileAsync("git", args, {
    cwd,
    env: { ...process.env, ...environment },
    maxBuffer: 32 * 1024 * 1024,
  });

const bearerArguments = ["-c", `http.extraHeader=Authorization: Bearer ${token}`];

const makeSession = (refRestrictions: ReadonlyArray<string> = []) =>
  ForgeGitSession.make({
    authenticatedAt: "2026-07-25T20:00:00.000Z",
    refRestrictions,
    repositoryRef,
    subjectRef: "actor.forge.acceptance",
    tenantRef,
    tokenRef: "token.forge.acceptance",
  });

const requestFromIncoming = (incoming: IncomingMessage): Request => {
  const headers = new Headers();
  for (const [name, value] of Object.entries(incoming.headers)) {
    if (value === undefined) continue;
    if (Array.isArray(value)) {
      for (const item of value) headers.append(name, item);
    } else {
      headers.set(name, value);
    }
  }
  const method = incoming.method ?? "GET";
  const init: RequestInit & { duplex?: "half" } = {
    headers,
    method,
    ...(method === "GET" || method === "HEAD"
      ? {}
      : {
          body: new ReadableStream<Uint8Array>({
            start(controller) {
              incoming.on("data", (chunk: Buffer) => controller.enqueue(new Uint8Array(chunk)));
              incoming.on("end", () => controller.close());
              incoming.on("error", (error) => controller.error(error));
            },
            cancel() {
              incoming.destroy();
            },
          }),
          duplex: "half",
        }),
  };
  return new Request(`http://${incoming.headers.host ?? "127.0.0.1"}${incoming.url ?? "/"}`, init);
};

const listen = async (
  runtime: ManagedRuntime.ManagedRuntime<
    | ForgeGitAdmission
    | ForgeGitAuth
    | ForgeGitConfiguration
    | ForgeGitProjection
    | import("./projector.js").ForgeGitProjector
    | ForgeGitRepository
    | ForgeWebRead
    | ForgeWebReadPolicy,
    never
  >,
): Promise<Readonly<{ origin: string; server: Server }>> => {
  const server = createServer(async (incoming, outgoing) => {
    try {
      const response = await runtime.runPromise(routeRequest(requestFromIncoming(incoming)));
      outgoing.statusCode = response.status;
      response.headers.forEach((value, name) => outgoing.setHeader(name, value));
      if (response.body === null) {
        outgoing.end();
      } else {
        const reader = response.body.getReader();
        while (true) {
          const item = await reader.read();
          if (item.done) break;
          outgoing.write(Buffer.from(item.value));
        }
        outgoing.end();
      }
    } catch (error) {
      outgoing.statusCode = 500;
      outgoing.end(error instanceof Error ? error.message : "request failed");
    }
  });
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  if (address === null || typeof address === "string") {
    throw new Error("test server did not bind a TCP port");
  }
  return { origin: `http://127.0.0.1:${address.port}`, server };
};

const closeServer = (server: Server): Promise<void> =>
  new Promise((resolve, reject) => {
    server.close((error) => (error === undefined ? resolve() : reject(error)));
  });

const makeRuntime = (
  repositoryRoot: string,
  blobStore = makeMemoryBlobStore(),
  session = makeSession(),
  allowedScopes: ReadonlyArray<"git:receive-pack" | "git:upload-pack"> = [
    "git:receive-pack",
    "git:upload-pack",
  ],
  authLayer: Layer.Layer<ForgeGitAuth> = makeStaticAuthLayer(token, session, allowedScopes),
  admissionPolicies: Array<{
    eventId: string;
    newObjectId: string;
    oldObjectId: string;
    refName: string;
    repositoryRef: string;
    tenantRef: string;
  }> = [],
  projectedEvents: ReadonlyArray<ForgeGitProjectedEvent> = [],
  policyAuthorityUrl = "https://openagents.test",
  webReadPolicyFetch: ForgeWebReadPolicyFetch = async (_input, init) => {
    const headers = new Headers(init?.headers);
    if (headers.get("authorization") !== "Bearer forge-git-service-test-secret") {
      return Response.json({ error: "forge_session_required" }, { status: 401 });
    }
    return Response.json({
      access: {
        canWrite: true,
        mode: "member",
      },
      repository: {
        maintainers: [{ displayName: "Forge Test" }],
        publicWebRead: false,
      },
    });
  },
) => {
  const configuration = makeTestConfiguration({
    gitBinary: "git",
    maxReceivePackBytes: 64 * 1024 * 1024,
    mirrorEnabled: true,
    policyAuthorityUrl,
    repositoryRoot,
  });
  const repositoryLayer = makeRepositoryLayer(configuration, blobStore);
  const webReadLayer = makeForgeWebReadLayer(configuration);
  const webReadPolicyLayer = makeForgeWebReadPolicyLayer(configuration, webReadPolicyFetch);
  const applicationLayer = Layer.mergeAll(
    makeMemoryAdmissionLayer({
      admittedRepositories: [{ repositoryRef, tenantRef }],
      projectedEvents,
      signedRefPolicies: admissionPolicies,
    }),
    authLayer,
    Layer.succeed(ForgeGitConfiguration, configuration),
    layerNoopProjection,
    layerNoopProjector,
    repositoryLayer,
    webReadLayer,
    webReadPolicyLayer,
  );
  return {
    blobStore,
    configuration,
    repositoryLayer,
    runtime: ManagedRuntime.make(applicationLayer),
  };
};

const authorizeHead = (
  policies: Array<{
    eventId: string;
    newObjectId: string;
    oldObjectId: string;
    refName: string;
    repositoryRef: string;
    tenantRef: string;
  }>,
  oldObjectId: string,
  newObjectId: string,
) => {
  policies.push({
    eventId: `state-${newObjectId}`,
    newObjectId,
    oldObjectId,
    refName: "refs/heads/main",
    repositoryRef,
    tenantRef,
  });
};

const provisionAdmittedRepository = async (fixture: ReturnType<typeof makeRuntime>) => {
  const runtime = ManagedRuntime.make(fixture.repositoryLayer);
  try {
    await runtime.runPromise(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        yield* repository.provision({ repositoryRef, tenantRef });
      }),
    );
  } finally {
    await runtime.dispose();
  }
};

describe("owned Forge Smart HTTP service", () => {
  test("serves admitted collaboration events only after service and member authorization", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-collaboration-"));
    temporaryPaths.push(root);
    const changeRef = "1".repeat(64);
    const head = "2".repeat(40);
    const base = "3".repeat(40);
    const projectedEvent: ForgeGitProjectedEvent = {
      actorBindingRef: "binding.forge.member",
      authorPubkey: "a".repeat(64),
      createdAt: "2026-07-26T08:00:00.000Z",
      eventId: changeRef,
      eventJson: JSON.stringify({
        content: "Owned collaboration projection",
        created_at: 1_785_000_000,
        id: changeRef,
        kind: 1617,
        pubkey: "a".repeat(64),
        sig: "b".repeat(128),
        tags: [
          ["a", `30617:${"a".repeat(64)}:${repositoryRef}`],
          ["commit", head],
          ["parent-commit", base],
        ],
      }),
      kind: 1617,
      objectIds: [base, head],
      repositoryRef,
      tenantRef,
    };
    const policyRequests: Array<
      Readonly<{ authorization: string | undefined; cookie: string | undefined }>
    > = [];
    const policyServer = createServer((incoming, outgoing) => {
      policyRequests.push({
        authorization: incoming.headers.authorization,
        cookie: incoming.headers.cookie,
      });
      outgoing.setHeader("content-type", "application/json");
      outgoing.end(JSON.stringify({ access: { canWrite: false, mode: "member" } }));
    });
    await new Promise<void>((resolve, reject) => {
      policyServer.once("error", reject);
      policyServer.listen(0, "127.0.0.1", resolve);
    });
    disposers.push(() => closeServer(policyServer));
    const address = policyServer.address();
    if (address === null || typeof address === "string") {
      throw new Error("policy server unavailable");
    }
    const fixture = makeRuntime(
      root,
      makeMemoryBlobStore(),
      makeSession(),
      ["git:receive-pack", "git:upload-pack"],
      makeStaticAuthLayer(token, makeSession(), ["git:receive-pack", "git:upload-pack"]),
      [],
      [projectedEvent],
      `http://127.0.0.1:${address.port}`,
    );
    const service = await listen(fixture.runtime);
    disposers.push(async () => {
      await closeServer(service.server);
      await fixture.runtime.dispose();
    });
    const endpoint = `${service.origin}/internal/v1/repositories/${tenantRef}/${repositoryRef}/collaboration/changes/${changeRef}`;

    expect((await fetch(endpoint)).status).toBe(401);
    const response = await fetch(endpoint, {
      headers: {
        authorization: "Bearer forge-git-service-test-secret",
        cookie: "oa_session=private",
      },
    });
    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(await response.json()).toMatchObject({
      change: {
        base: { value: base },
        changeRef,
        head: { value: head },
        proposalResolution: "resolved",
      },
      schema: "openagents.forge.collaboration_read.v1",
    });
    expect(policyRequests).toEqual([
      {
        authorization: "Bearer forge-git-service-test-secret",
        cookie: "oa_session=private",
      },
    ]);
  });

  test("persists a passed gate receipt and refuses an unresolved change request", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-gate-receipt-"));
    temporaryPaths.push(root);
    const fixture = makeRuntime(root);
    const service = await listen(fixture.runtime);
    disposers.push(
      () => closeServer(service.server),
      () => fixture.runtime.dispose(),
    );
    const oid = "a".repeat(40);
    const nextOid = "b".repeat(40);
    const gateInput = {
      authorityGeneration: 1,
      changeRef: "change.forge.route-test",
      checks: [
        {
          checkName: "test",
          checkRef: "check.test",
          completedAt: "2026-07-26T00:00:00.000Z",
          evidenceReceiptRef: "receipt.check",
          revisionObjectId: nextOid,
          state: "completed",
          verdict: "passed",
        },
      ],
      evaluatedAt: "2026-07-26T00:00:00.000Z",
      maintainerBindingRef: "binding.maintainer",
      newObjectId: nextOid,
      oldObjectId: oid,
      policyVersion: "policy.forge.v1",
      proposalEventIds: ["proposal.forge.1"],
      repositoryRef,
      requiredCheckNames: ["test"],
      requiredReviewCount: 1,
      requiredVerificationRungs: ["tests"],
      reviews: [
        {
          reviewerBindingRef: "binding.reviewer",
          reviewRef: "review.approved",
          revisionObjectId: nextOid,
          submittedAt: "2026-07-26T00:00:00.000Z",
          supersedesReviewRef: null,
          verdict: "approved",
        },
      ],
      targetRef: "refs/heads/main",
      tenantRef,
      verificationReceipts: [
        {
          attempt: 1,
          humanTagRef: null,
          maxAttempts: 1,
          receiptRef: "receipt.tests",
          revisionObjectId: nextOid,
          rung: "tests",
          state: "passed",
        },
      ],
    };
    const prepared = await fetch(`${service.origin}/internal/forge/merge-receipts`, {
      body: JSON.stringify({ gateInput, receiptRef: "receipt.forge.route-test" }),
      headers: {
        authorization: "Bearer forge-git-service-test-secret",
        "content-type": "application/json",
      },
      method: "POST",
    });
    expect(prepared.status).toBe(201);
    const resolved = await fetch(
      `${service.origin}/internal/forge/merge-receipts/receipt.forge.route-test?tenantRef=${tenantRef}&repositoryRef=${repositoryRef}`,
      {
        headers: { authorization: "Bearer forge-git-service-test-secret" },
      },
    );
    expect(resolved.status).toBe(200);
    expect(((await resolved.json()) as { state: string }).state).toBe("prepared");
    const blocked = await fetch(`${service.origin}/internal/forge/merge-receipts`, {
      body: JSON.stringify({
        gateInput: {
          ...gateInput,
          reviews: [
            { ...gateInput.reviews[0], reviewRef: "review.change", verdict: "change_requested" },
          ],
        },
        receiptRef: "receipt.forge.blocked",
      }),
      headers: {
        authorization: "Bearer forge-git-service-test-secret",
        "content-type": "application/json",
      },
      method: "POST",
    });
    expect(blocked.status).toBe(409);
    expect(await blocked.json()).toMatchObject({ error: "forge_merge_gate_blocked" });
  });

  test("does not create an unadmitted repository and refuses an unsigned head move", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-admission-root-"));
    const source = await mkdtemp(join(tmpdir(), "oa-forge-admission-source-"));
    temporaryPaths.push(root, source);
    const configuration = makeTestConfiguration({
      gitBinary: "git",
      maxReceivePackBytes: 64 * 1024 * 1024,
      mirrorEnabled: false,
      repositoryRoot: root,
    });
    const repositoryLayer = makeRepositoryLayer(configuration, makeMemoryBlobStore());
    const deniedRuntime = ManagedRuntime.make(
      Layer.mergeAll(
        makeMemoryAdmissionLayer({ admittedRepositories: [] }),
        makeStaticAuthLayer(token, makeSession()),
        Layer.succeed(ForgeGitConfiguration, configuration),
        layerNoopProjection,
        layerNoopProjector,
        repositoryLayer,
        makeForgeWebReadLayer(configuration),
        makeForgeWebReadPolicyLayer(configuration, async () =>
          Response.json({ error: "forge_membership_required" }, { status: 403 }),
        ),
      ),
    );
    const deniedService = await listen(deniedRuntime);
    disposers.push(
      () => closeServer(deniedService.server),
      () => deniedRuntime.dispose(),
    );
    const remote = `${deniedService.origin}/git/${tenantRef}/${repositoryRef}.git`;
    const denied = await fetch(`${remote}/info/refs?service=git-receive-pack`, {
      headers: { authorization: `Bearer ${token}` },
    });
    expect(denied.status).toBe(403);

    const policies: Array<{
      eventId: string;
      newObjectId: string;
      oldObjectId: string;
      refName: string;
      repositoryRef: string;
      tenantRef: string;
    }> = [];
    const admitted = makeRuntime(
      root,
      makeMemoryBlobStore(),
      makeSession(),
      undefined,
      undefined,
      policies,
    );
    await provisionAdmittedRepository(admitted);
    const service = await listen(admitted.runtime);
    disposers.push(
      () => closeServer(service.server),
      () => admitted.runtime.dispose(),
    );
    await git(source, ["init", "--initial-branch=main"]);
    await git(source, ["config", "user.email", "forge-test@openagents.com"]);
    await git(source, ["config", "user.name", "Forge Test"]);
    await writeFile(join(source, "README.md"), "signed state required\n");
    await git(source, ["add", "README.md"]);
    await git(source, ["commit", "-m", "Unsigned state"]);
    await expect(
      git(source, [
        ...bearerArguments,
        "push",
        `${service.origin}/git/${tenantRef}/${repositoryRef}.git`,
        "HEAD:refs/heads/main",
      ]),
    ).rejects.toThrow(/remote rejected|failed to push/i);
  }, 120_000);

  test("uses stock receive-pack and upload-pack for push, clone, fetch, partial clone, backup, and restore", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-git-root-"));
    const source = await mkdtemp(join(tmpdir(), "oa-forge-source-"));
    const firstClone = await mkdtemp(join(tmpdir(), "oa-forge-clone-a-"));
    const secondClone = await mkdtemp(join(tmpdir(), "oa-forge-clone-b-"));
    const partialClone = await mkdtemp(join(tmpdir(), "oa-forge-partial-"));
    const restoredRoot = await mkdtemp(join(tmpdir(), "oa-forge-restored-"));
    const restoredClone = await mkdtemp(join(tmpdir(), "oa-forge-restored-clone-"));
    temporaryPaths.push(
      root,
      source,
      firstClone,
      secondClone,
      partialClone,
      restoredRoot,
      restoredClone,
    );

    const policies: Array<{
      eventId: string;
      newObjectId: string;
      oldObjectId: string;
      refName: string;
      repositoryRef: string;
      tenantRef: string;
    }> = [];
    const fixture = makeRuntime(
      root,
      makeMemoryBlobStore(),
      makeSession(),
      undefined,
      undefined,
      policies,
    );
    await provisionAdmittedRepository(fixture);
    const service = await listen(fixture.runtime);
    disposers.push(
      () => closeServer(service.server),
      () => fixture.runtime.dispose(),
    );
    const remote = `${service.origin}/git/${tenantRef}/${repositoryRef}.git`;

    await git(source, ["init", "--initial-branch=main"]);
    await git(source, ["config", "user.email", "forge-test@openagents.com"]);
    await git(source, ["config", "user.name", "Forge Test"]);
    await writeFile(join(source, "README.md"), "forge round trip A\n");
    await git(source, ["add", "README.md"]);
    await git(source, ["commit", "-m", "Seed Forge repository"]);
    const commitA = (await git(source, ["rev-parse", "HEAD"])).stdout.trim();
    authorizeHead(policies, "0".repeat(40), commitA);
    await git(source, [...bearerArguments, "push", remote, "HEAD:refs/heads/main"]);

    const advertisement = await fetch(`${remote}/info/refs?service=git-upload-pack`, {
      headers: { authorization: `Bearer ${token}` },
    });
    expect(advertisement.status).toBe(200);
    const capabilities = new TextDecoder().decode(await advertisement.arrayBuffer());
    expect(capabilities).toContain("filter");
    expect(capabilities).toContain("allow-tip-sha1-in-want");
    expect(capabilities).toContain("allow-reachable-sha1-in-want");

    await git(firstClone, [...bearerArguments, "clone", remote, "."]);
    expect((await git(firstClone, ["rev-parse", "HEAD"])).stdout.trim()).toBe(commitA);
    expect(await readFile(join(firstClone, "README.md"), "utf8")).toBe("forge round trip A\n");

    await git(firstClone, ["config", "user.email", "forge-test@openagents.com"]);
    await git(firstClone, ["config", "user.name", "Forge Test"]);
    await writeFile(join(firstClone, "README.md"), "forge round trip B\n");
    await git(firstClone, ["add", "README.md"]);
    await git(firstClone, ["commit", "-m", "Update Forge repository"]);
    const commitB = (await git(firstClone, ["rev-parse", "HEAD"])).stdout.trim();
    authorizeHead(policies, commitA, commitB);
    await git(firstClone, [...bearerArguments, "push", "origin", "main"]);

    await git(secondClone, [...bearerArguments, "clone", remote, "."]);
    expect((await git(secondClone, ["rev-parse", "HEAD"])).stdout.trim()).toBe(commitB);
    await git(secondClone, ["fsck", "--full", "--strict"]);

    await git(partialClone, [
      ...bearerArguments,
      "clone",
      "--filter=blob:none",
      "--no-checkout",
      remote,
      ".",
    ]);
    expect(
      (await git(partialClone, ["config", "--get", "remote.origin.promisor"])).stdout.trim(),
    ).toBe("true");

    const backupRuntime = ManagedRuntime.make(fixture.repositoryLayer);
    const backup = await backupRuntime.runPromise(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        yield* repository.verify({ repositoryRef, tenantRef });
        return yield* repository.backup({ repositoryRef, tenantRef });
      }),
    );
    await backupRuntime.dispose();
    expect(backup.refs).toContainEqual({
      objectId: commitB,
      refName: "refs/heads/main",
    });

    const restoredPolicies: typeof policies = [];
    const restored = makeRuntime(
      restoredRoot,
      fixture.blobStore,
      makeSession(),
      undefined,
      undefined,
      restoredPolicies,
    );
    const restoreRuntime = ManagedRuntime.make(restored.repositoryLayer);
    await restoreRuntime.runPromise(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        yield* repository.restore({ receipt: backup });
        yield* repository.verify({ repositoryRef, tenantRef });
      }),
    );
    await restoreRuntime.dispose();

    const restoredService = await listen(restored.runtime);
    disposers.push(
      () => closeServer(restoredService.server),
      () => restored.runtime.dispose(),
    );
    const restoredRemote = `${restoredService.origin}/git/${tenantRef}/${repositoryRef}.git`;
    await git(restoredClone, [...bearerArguments, "clone", restoredRemote, "."]);
    expect((await git(restoredClone, ["rev-parse", "HEAD"])).stdout.trim()).toBe(commitB);

    await git(restoredClone, ["config", "user.email", "forge-test@openagents.com"]);
    await git(restoredClone, ["config", "user.name", "Forge Test"]);
    await writeFile(join(restoredClone, "RESTORED.md"), "writable restore\n");
    await git(restoredClone, ["add", "RESTORED.md"]);
    await git(restoredClone, ["commit", "-m", "Prove restored repository writes"]);
    const commitC = (await git(restoredClone, ["rev-parse", "HEAD"])).stdout.trim();
    authorizeHead(restoredPolicies, commitB, commitC);
    await git(restoredClone, [...bearerArguments, "push", "origin", "main"]);

    const receiptOutput = process.env["FORGE_GIT_RECEIPT_OUT"];
    if (receiptOutput !== undefined && receiptOutput.trim() !== "") {
      const evidenceKeys = await Effect.runPromise(fixture.blobStore.list("private/forge/"));
      await writeFile(
        receiptOutput,
        `${JSON.stringify(
          {
            schemaVersion: "openagents.forge_git.local_drill_receipt.v1",
            generatedAt: new Date().toISOString(),
            sourceRevision: process.env["FORGE_GIT_SOURCE_REVISION"] ?? "working-tree",
            authority: {
              repository: "bare-repository",
              gcsMirror: false,
            },
            checks: {
              backupBundleSha256: backup.bundleSha256,
              backupBytes: backup.bundleBytes,
              cloneCommitA: commitA,
              fetchAndCloneCommitB: commitB,
              partialClonePromisor: true,
              restoredPushCommitC: commitC,
              sourceFsck: "passed",
              restoredFsck: "passed",
            },
            evidenceKeys,
            refsAtBackup: backup.refs,
            result: "passed",
            scope: "local-real-stock-git",
          },
          null,
          2,
        )}\n`,
      );
    }
  }, 120_000);

  test("authenticates before Git and keeps upload-only tokens read-only", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-auth-root-"));
    temporaryPaths.push(root);
    const fixture = makeRuntime(root, makeMemoryBlobStore(), makeSession(), ["git:upload-pack"]);
    const service = await listen(fixture.runtime);
    disposers.push(
      () => closeServer(service.server),
      () => fixture.runtime.dispose(),
    );
    const remote = `${service.origin}/git/${tenantRef}/${repositoryRef}.git`;

    const anonymous = await fetch(`${remote}/info/refs?service=git-upload-pack`);
    expect(anonymous.status).toBe(401);
    expect(anonymous.headers.get("www-authenticate")).toContain("Basic");

    const receive = await fetch(`${remote}/info/refs?service=git-receive-pack`, {
      headers: { authorization: `Bearer ${token}` },
    });
    expect(receive.status).toBe(401);
  });

  test("serves bounded canonical web reads only after service and read-policy authorization", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-web-read-root-"));
    const source = await mkdtemp(join(tmpdir(), "oa-forge-web-read-source-"));
    temporaryPaths.push(root, source);
    let policyCalls = 0;
    const policies: Array<{
      eventId: string;
      newObjectId: string;
      oldObjectId: string;
      refName: string;
      repositoryRef: string;
      tenantRef: string;
    }> = [];
    const fixture = makeRuntime(
      root,
      makeMemoryBlobStore(),
      makeSession(),
      ["git:receive-pack", "git:upload-pack"],
      undefined,
      policies,
      [],
      "https://openagents.test",
      async (_input, init) => {
        policyCalls += 1;
        const headers = new Headers(init?.headers);
        expect(headers.get("authorization")).toBe("Bearer forge-git-service-test-secret");
        expect(headers.get("cookie")).toBe("oa_session=member");
        return Response.json({
          access: {
            canWrite: true,
            mode: "member",
          },
          repository: {
            maintainers: [{ displayName: "Invited maintainer" }],
            publicWebRead: false,
          },
        });
      },
    );
    await provisionAdmittedRepository(fixture);
    const service = await listen(fixture.runtime);
    disposers.push(
      () => closeServer(service.server),
      () => fixture.runtime.dispose(),
    );
    const remote = `${service.origin}/git/${tenantRef}/${repositoryRef}.git`;
    await git(source, ["init", "--initial-branch=main"]);
    await git(source, ["config", "user.email", "forge-test@openagents.com"]);
    await git(source, ["config", "user.name", "Forge Test"]);
    await writeFile(
      join(source, "README.md"),
      "# Owned Forge\n\n![Owned logo](logo.png)\n![External](https://example.com/logo.png)\n![Escape](../secret.png)\n",
    );
    await writeFile(join(source, "source.ts"), "export const authority = 'bare-repository';\n");
    const imageBytes = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
    await writeFile(join(source, "logo.png"), imageBytes);
    await git(source, ["add", "."]);
    await git(source, ["commit", "-m", "Seed owned web read"]);
    const commit = (await git(source, ["rev-parse", "HEAD"])).stdout.trim();
    authorizeHead(policies, "0".repeat(40), commit);
    await git(source, [...bearerArguments, "push", remote, "HEAD:refs/heads/main"]);
    await git(source, [
      "--git-dir",
      join(root, tenantRef, `${repositoryRef}.git`),
      "config",
      "openagents.nip34Coordinate",
      `30617:${"a".repeat(64)}:${repositoryRef}`,
    ]);

    const endpoint = new URL(
      `${service.origin}/internal/v1/repositories/${tenantRef}/${repositoryRef}/web-read`,
    );
    endpoint.searchParams.set("view", "code");
    endpoint.searchParams.set("path", "source.ts");
    endpoint.searchParams.set("max_text_bytes", "512000");
    endpoint.searchParams.set("max_image_bytes", "2000000");
    endpoint.searchParams.set("max_diff_bytes", "1000000");

    const anonymous = await fetch(endpoint);
    expect(anonymous.status).toBe(401);
    expect(policyCalls).toBe(0);
    const wrongService = await fetch(endpoint, {
      headers: { authorization: `Bearer ${token}`, cookie: "oa_session=member" },
    });
    expect(wrongService.status).toBe(401);
    expect(policyCalls).toBe(0);

    const response = await fetch(endpoint, {
      headers: {
        authorization: "Bearer forge-git-service-test-secret",
        cookie: "oa_session=member",
      },
    });
    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(policyCalls).toBe(1);
    const projection = (await response.json()) as {
      access: { canWrite: boolean; mode: string };
      file: { _tag: string; content: string };
      readme: {
        assets: ReadonlyArray<{ path: string; sourceUrl: string }>;
      };
      repository: {
        canonicalCloneUrl: string;
        maintainers: ReadonlyArray<{ displayName: string }>;
        nip34Coordinate: string;
      };
      schema: string;
    };
    expect(projection.schema).toBe("openagents.forge.repository_web_read.v1");
    expect(projection.access).toEqual({ canWrite: true, mode: "member" });
    expect(projection.file).toMatchObject({
      _tag: "text",
      content: "export const authority = 'bare-repository';\n",
    });
    expect(projection.repository.canonicalCloneUrl).toBe(
      `https://openagents.com/git/${tenantRef}/${repositoryRef}.git`,
    );
    expect(projection.repository.maintainers).toEqual([{ displayName: "Invited maintainer" }]);
    expect(projection.repository.nip34Coordinate).toBe(`30617:${"a".repeat(64)}:${repositoryRef}`);
    expect(projection.readme.assets).toHaveLength(1);
    expect(projection.readme.assets[0]?.path).toBe("logo.png");
    expect(projection.readme.assets[0]?.sourceUrl).toContain("/web-read-asset/logo.png?");

    endpoint.searchParams.set("path", "logo.png");
    const imageProjectionResponse = await fetch(endpoint, {
      headers: {
        authorization: "Bearer forge-git-service-test-secret",
        cookie: "oa_session=member",
      },
    });
    const imageProjection = (await imageProjectionResponse.json()) as {
      file: { _tag: string; sourceUrl: string };
    };
    expect(imageProjection.file).toMatchObject({ _tag: "image" });
    const assetUrl = new URL(imageProjection.file.sourceUrl, service.origin);
    const refusedAsset = await fetch(assetUrl, {
      headers: { authorization: `Bearer ${token}`, cookie: "oa_session=member" },
    });
    expect(refusedAsset.status).toBe(401);
    const asset = await fetch(assetUrl, {
      headers: {
        authorization: "Bearer forge-git-service-test-secret",
        cookie: "oa_session=member",
      },
    });
    expect(asset.status).toBe(200);
    expect(asset.headers.get("cache-control")).toBe("no-store");
    expect(asset.headers.get("content-type")).toBe("image/png");
    expect(new Uint8Array(await asset.arrayBuffer())).toEqual(imageBytes);
    assetUrl.searchParams.set("object", "0".repeat(40));
    const substitutedAsset = await fetch(assetUrl, {
      headers: {
        authorization: "Bearer forge-git-service-test-secret",
        cookie: "oa_session=member",
      },
    });
    expect(substitutedAsset.status).toBe(404);

    await git(source, [
      "--git-dir",
      join(root, tenantRef, `${repositoryRef}.git`),
      "config",
      "openagents.nip34Coordinate",
      "30617:OpenAgentsInc:invalid",
    ]);
    const invalidMetadata = await fetch(endpoint, {
      headers: {
        authorization: "Bearer forge-git-service-test-secret",
        cookie: "oa_session=member",
      },
    });
    expect(invalidMetadata.status).toBe(503);
    expect(await invalidMetadata.json()).toEqual({
      error: "forge_web_read_metadata_unavailable",
    });
  }, 120_000);

  test("fails closed when web-read policy refuses membership", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-web-policy-root-"));
    temporaryPaths.push(root);
    const fixture = makeRuntime(
      root,
      makeMemoryBlobStore(),
      makeSession(),
      ["git:receive-pack", "git:upload-pack"],
      undefined,
      [],
      [],
      "https://openagents.test",
      async () => Response.json({ error: "forge_membership_required" }, { status: 403 }),
    );
    const service = await listen(fixture.runtime);
    disposers.push(
      () => closeServer(service.server),
      () => fixture.runtime.dispose(),
    );
    const endpoint = `${service.origin}/internal/v1/repositories/${tenantRef}/${repositoryRef}/web-read?view=code`;
    const response = await fetch(endpoint, {
      headers: {
        authorization: "Bearer forge-git-service-test-secret",
        cookie: "oa_session=revoked",
      },
    });
    expect(response.status).toBe(403);
    expect(await response.json()).toEqual({ error: "forge_web_read_membership_required" });
  });

  test("admits invited humans and agents, then refuses a revoked agent replay", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-policy-root-"));
    const source = await mkdtemp(join(tmpdir(), "oa-forge-policy-source-"));
    const humanClone = await mkdtemp(join(tmpdir(), "oa-forge-policy-human-"));
    const agentClone = await mkdtemp(join(tmpdir(), "oa-forge-policy-agent-"));
    temporaryPaths.push(root, source, humanClone, agentClone);
    const humanToken = "oa_forge_git_human_000000000000000000000";
    const agentToken = "oa_forge_git_agent_000000000000000000000";
    const revoked = new Set<string>();
    const configuration = makeTestConfiguration({
      gitBinary: "git",
      maxReceivePackBytes: 64 * 1024 * 1024,
      mirrorEnabled: true,
      repositoryRoot: root,
    });
    const authLayer = makePolicyAuthorityAuthLayer(configuration, async (_input, init) => {
      const body = JSON.parse(String(init?.body)) as {
        repositoryRef: string;
        tenantRef: string;
        transportAuthorization: string;
      };
      const presented = body.transportAuthorization.replace(/^Bearer\s+/, "");
      if (revoked.has(presented) || (presented !== humanToken && presented !== agentToken)) {
        return Response.json({ error: "forge_membership_tombstoned" }, { status: 403 });
      }
      return Response.json({
        session: {
          authenticatedAt: "2026-07-25T20:00:00.000Z",
          refRestrictions: [],
          repositoryRef: body.repositoryRef,
          subjectRef:
            presented === humanToken
              ? "forge_actor.human.invited"
              : "forge_actor.agent.owner_attested",
          tenantRef: body.tenantRef,
          tokenRef: presented === humanToken ? "forge_git_token.human" : "forge_git_token.agent",
        },
      });
    });
    const policies: Array<{
      eventId: string;
      newObjectId: string;
      oldObjectId: string;
      refName: string;
      repositoryRef: string;
      tenantRef: string;
    }> = [];
    const fixture = makeRuntime(
      root,
      makeMemoryBlobStore(),
      makeSession(),
      ["git:receive-pack", "git:upload-pack"],
      authLayer,
      policies,
    );
    await provisionAdmittedRepository(fixture);
    const service = await listen(fixture.runtime);
    disposers.push(
      () => closeServer(service.server),
      () => fixture.runtime.dispose(),
    );
    const remote = `${service.origin}/git/${tenantRef}/${repositoryRef}.git`;
    const authArgs = (credential: string) => [
      "-c",
      `http.extraHeader=Authorization: Bearer ${credential}`,
    ];

    await git(source, ["init", "--initial-branch=main"]);
    await git(source, ["config", "user.email", "forge-test@openagents.com"]);
    await git(source, ["config", "user.name", "Forge Test"]);
    await writeFile(join(source, "README.md"), "invite policy\n");
    await git(source, ["add", "README.md"]);
    await git(source, ["commit", "-m", "Seed invited repository"]);
    const commitA = (await git(source, ["rev-parse", "HEAD"])).stdout.trim();
    authorizeHead(policies, "0".repeat(40), commitA);
    await git(source, [...authArgs(humanToken), "push", remote, "HEAD:refs/heads/main"]);

    await git(humanClone, [...authArgs(humanToken), "clone", remote, "."]);
    await git(humanClone, ["config", "user.email", "human@openagents.com"]);
    await git(humanClone, ["config", "user.name", "Invited human"]);
    await writeFile(join(humanClone, "HUMAN.md"), "human push\n");
    await git(humanClone, ["add", "HUMAN.md"]);
    await git(humanClone, ["commit", "-m", "Human push"]);
    const commitB = (await git(humanClone, ["rev-parse", "HEAD"])).stdout.trim();
    authorizeHead(policies, commitA, commitB);
    await git(humanClone, [...authArgs(humanToken), "push", "origin", "main"]);

    await git(agentClone, [...authArgs(agentToken), "clone", remote, "."]);
    expect(await readFile(join(agentClone, "README.md"), "utf8")).toBe("invite policy\n");
    await git(agentClone, ["config", "user.email", "agent@openagents.com"]);
    await git(agentClone, ["config", "user.name", "Owner-attested agent"]);
    await writeFile(join(agentClone, "AGENT.md"), "agent push\n");
    await git(agentClone, ["add", "AGENT.md"]);
    await git(agentClone, ["commit", "-m", "Agent push"]);
    const commitC = (await git(agentClone, ["rev-parse", "HEAD"])).stdout.trim();
    authorizeHead(policies, commitB, commitC);
    await git(agentClone, [...authArgs(agentToken), "push", "origin", "main"]);

    revoked.add(agentToken);
    await expect(git(agentClone, [...authArgs(agentToken), "fetch", "origin"])).rejects.toThrow();
  }, 120_000);

  test("enforces exact ref restrictions inside stock receive-pack", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-ref-root-"));
    const source = await mkdtemp(join(tmpdir(), "oa-forge-ref-source-"));
    temporaryPaths.push(root, source);
    const policies: Array<{
      eventId: string;
      newObjectId: string;
      oldObjectId: string;
      refName: string;
      repositoryRef: string;
      tenantRef: string;
    }> = [];
    const fixture = makeRuntime(
      root,
      makeMemoryBlobStore(),
      makeSession(["refs/heads/main"]),
      undefined,
      undefined,
      policies,
    );
    await provisionAdmittedRepository(fixture);
    const service = await listen(fixture.runtime);
    disposers.push(
      () => closeServer(service.server),
      () => fixture.runtime.dispose(),
    );
    const remote = `${service.origin}/git/${tenantRef}/${repositoryRef}.git`;

    await git(source, ["init", "--initial-branch=main"]);
    await git(source, ["config", "user.email", "forge-test@openagents.com"]);
    await git(source, ["config", "user.name", "Forge Test"]);
    await writeFile(join(source, "README.md"), "restricted\n");
    await git(source, ["add", "README.md"]);
    await git(source, ["commit", "-m", "Restricted ref seed"]);
    authorizeHead(
      policies,
      "0".repeat(40),
      (await git(source, ["rev-parse", "HEAD"])).stdout.trim(),
    );
    await git(source, [...bearerArguments, "push", remote, "HEAD:refs/heads/main"]);

    await expect(
      git(source, [...bearerArguments, "push", remote, "HEAD:refs/heads/not-allowed"]),
    ).rejects.toThrow();

    const repositoryRuntime = ManagedRuntime.make(fixture.repositoryLayer);
    const refs = await repositoryRuntime.runPromise(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        return yield* repository.listRefs({ repositoryRef, tenantRef });
      }),
    );
    await repositoryRuntime.dispose();
    expect(refs.map((ref) => ref.refName)).toEqual(["refs/heads/main"]);
  });
});
