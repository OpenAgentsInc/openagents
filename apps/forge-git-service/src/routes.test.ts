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

import { ForgeGitAdmission, layerAdmission } from "./admission.js";
import { ForgeGitAuth, makeStaticAuthLayer } from "./auth.js";
import { makeTestConfiguration } from "./config.js";
import { ForgeGitSession } from "./model.js";
import { ForgeGitProjection, layerNoopProjection } from "./projection.js";
import { ForgeGitRepository, makeRepositoryLayer } from "./repository.js";
import { routeRequest } from "./routes.js";

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
    ForgeGitAdmission | ForgeGitAuth | ForgeGitProjection | ForgeGitRepository,
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
) => {
  const configuration = makeTestConfiguration({
    gitBinary: "git",
    maxReceivePackBytes: 64 * 1024 * 1024,
    mirrorEnabled: true,
    repositoryRoot,
  });
  const repositoryLayer = makeRepositoryLayer(configuration, blobStore);
  const applicationLayer = Layer.mergeAll(
    layerAdmission,
    makeStaticAuthLayer(token, session, allowedScopes),
    layerNoopProjection,
    repositoryLayer,
  );
  return {
    blobStore,
    configuration,
    repositoryLayer,
    runtime: ManagedRuntime.make(applicationLayer),
  };
};

describe("owned Forge Smart HTTP service", () => {
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

    const fixture = makeRuntime(root);
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

    const restored = makeRuntime(restoredRoot, fixture.blobStore, makeSession());
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

  test("enforces exact ref restrictions inside stock receive-pack", async () => {
    const root = await mkdtemp(join(tmpdir(), "oa-forge-ref-root-"));
    const source = await mkdtemp(join(tmpdir(), "oa-forge-ref-source-"));
    temporaryPaths.push(root, source);
    const fixture = makeRuntime(root, makeMemoryBlobStore(), makeSession(["refs/heads/main"]));
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
