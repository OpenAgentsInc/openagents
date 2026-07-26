/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- @effect/vitest does not support the repository Effect 4 line. */
import { execFile } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

import { makeMemoryBlobStore } from "@openagentsinc/oa-infra/blob-store-memory";
import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vitest";

import { makeTestConfiguration } from "./config.js";
import { ForgeGitRepository, makeRepositoryLayer } from "./repository.js";

const execFileAsync = promisify(execFile);
const temporaryPaths: Array<string> = [];

afterEach(async () => {
  await Promise.all(
    temporaryPaths.splice(0).map((path) => rm(path, { force: true, recursive: true })),
  );
});

const git = (cwd: string, args: ReadonlyArray<string>) =>
  execFileAsync("git", args, {
    cwd,
    env: process.env,
    maxBuffer: 32 * 1024 * 1024,
  });

const makeFixture = async () => {
  const root = await mkdtemp(join(tmpdir(), "oa-forge-mirror-root-"));
  const source = await mkdtemp(join(tmpdir(), "oa-forge-mirror-source-"));
  const destination = await mkdtemp(join(tmpdir(), "oa-forge-mirror-destination-"));
  temporaryPaths.push(root, source, destination);
  const tenantRef = "owner.openagents";
  const repositoryRef = "mirror";
  const canonical = join(root, tenantRef, `${repositoryRef}.git`);
  await mkdir(join(root, tenantRef), { recursive: true });
  await git(root, ["init", "--bare", "--initial-branch=main", canonical]);
  await git(destination, ["init", "--bare", "--initial-branch=main"]);
  await git(source, ["init", "--initial-branch=main"]);
  await git(source, ["config", "user.email", "forge-test@openagents.com"]);
  await git(source, ["config", "user.name", "Forge Test"]);
  await writeFile(join(source, "README.md"), "canonical source\n");
  await git(source, ["add", "README.md"]);
  await git(source, ["commit", "-m", "Create canonical source"]);
  await git(source, ["tag", "release-1"]);
  await git(source, ["push", canonical, "HEAD:refs/heads/main", "refs/tags/release-1"]);
  const sourceObjectId = (await git(source, ["rev-parse", "HEAD"])).stdout.trim();
  const configuration = makeTestConfiguration({
    gitBinary: "git",
    maxReceivePackBytes: 64 * 1024 * 1024,
    mirrorEnabled: true,
    repositoryRoot: root,
  });
  const layer = makeRepositoryLayer(configuration, makeMemoryBlobStore());
  const run = <A, E>(effect: Effect.Effect<A, E, ForgeGitRepository>): Promise<A> =>
    Effect.runPromise(effect.pipe(Effect.provide(layer)));
  return {
    destination,
    repositoryRef,
    run,
    sourceObjectId,
    tenantRef,
  };
};

describe("owned canonical Git mirror", () => {
  test("projects exact promoted branch and tag refs and is idempotent", async () => {
    const fixture = await makeFixture();
    const branchInput = {
      destinationRef: "refs/heads/main",
      destinationUrl: fixture.destination,
      expectedSourceObjectId: fixture.sourceObjectId,
      repositoryRef: fixture.repositoryRef,
      sourceRef: "refs/heads/main",
      tenantRef: fixture.tenantRef,
    } as const;
    const tagInput = {
      ...branchInput,
      destinationRef: "refs/tags/release-1",
      sourceRef: "refs/tags/release-1",
    } as const;

    const before = await fixture.run(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        return yield* repository.observeMirror(branchInput);
      }),
    );
    expect(before.divergence).toBe("destination_missing");

    const branch = await fixture.run(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        return yield* repository.projectMirror(branchInput);
      }),
    );
    const tag = await fixture.run(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        return yield* repository.projectMirror(tagInput);
      }),
    );
    const repeated = await fixture.run(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        return yield* repository.projectMirror(branchInput);
      }),
    );

    expect(branch.divergence).toBe("in_sync");
    expect(tag.divergence).toBe("in_sync");
    expect(repeated).toMatchObject({
      destinationObjectId: fixture.sourceObjectId,
      divergence: "in_sync",
      sourceObjectId: fixture.sourceObjectId,
    });
    expect((await git(fixture.destination, ["rev-parse", "refs/heads/main"])).stdout.trim()).toBe(
      fixture.sourceObjectId,
    );
    expect(
      (await git(fixture.destination, ["rev-parse", "refs/tags/release-1"])).stdout.trim(),
    ).toBe(fixture.sourceObjectId);
  });

  test("rejects a stale promoted object and never changes the destination", async () => {
    const fixture = await makeFixture();
    const input = {
      destinationRef: "refs/heads/main",
      destinationUrl: fixture.destination,
      expectedSourceObjectId: "0000000000000000000000000000000000000000",
      repositoryRef: fixture.repositoryRef,
      sourceRef: "refs/heads/main",
      tenantRef: fixture.tenantRef,
    } as const;

    const failure = await fixture.run(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        return yield* repository.projectMirror(input).pipe(Effect.flip);
      }),
    );

    expect(failure.code).toBe("forge_git_mirror_source_object_mismatch");
    await expect(git(fixture.destination, ["rev-parse", "refs/heads/main"])).rejects.toThrow();
  });
});
