/* oxlint-disable openagents/no-manual-effect-runtime-in-tests -- @effect/vitest does not support the repository Effect 4 line. */
import { execFile } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";

import { makeMemoryBlobStore } from "@openagentsinc/oa-infra/blob-store-memory";
import { ForgeGitHubMirrorIntent } from "@openagentsinc/forge-protocol";
import { Effect, Redacted } from "effect";
import { afterEach, describe, expect, test } from "vitest";

import { makeTestConfiguration } from "./config.js";
import {
  makeForgeGitHubMirrorRunner,
  makeGitHubHttpsMirrorDestinationResolver,
} from "./github-mirror-runner.js";
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
  await git(source, ["tag", "-a", "release-1", "-m", "Release 1"]);
  await git(source, ["push", canonical, "HEAD:refs/heads/main", "refs/tags/release-1"]);
  const sourceObjectId = (await git(source, ["rev-parse", "HEAD"])).stdout.trim();
  const tagObjectId = (await git(source, ["rev-parse", "refs/tags/release-1"])).stdout.trim();
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
    canonical,
    destination,
    repositoryRef,
    run,
    source,
    sourceObjectId,
    tagObjectId,
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
      expectedSourceObjectId: fixture.tagObjectId,
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
    ).toBe(fixture.tagObjectId);
    expect(
      (await git(fixture.destination, ["rev-parse", "refs/tags/release-1^{}"])).stdout.trim(),
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

  test("observes source-ahead, destination-ahead, and diverged commit graphs", async () => {
    const fixture = await makeFixture();
    const input = (expectedSourceObjectId: string) => ({
      destinationRef: "refs/heads/main",
      destinationUrl: fixture.destination,
      expectedSourceObjectId,
      repositoryRef: fixture.repositoryRef,
      sourceRef: "refs/heads/main",
      tenantRef: fixture.tenantRef,
    });
    const observe = (expectedSourceObjectId: string) =>
      fixture.run(
        Effect.gen(function* () {
          const repository = yield* ForgeGitRepository;
          return yield* repository.observeMirror(input(expectedSourceObjectId));
        }),
      );
    await fixture.run(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        yield* repository.projectMirror(input(fixture.sourceObjectId));
      }),
    );

    const destinationWriter = await mkdtemp(join(tmpdir(), "oa-forge-mirror-writer-"));
    temporaryPaths.push(destinationWriter);
    await git(destinationWriter, ["clone", fixture.destination, "."]);
    await git(destinationWriter, ["config", "user.email", "forge-test@openagents.com"]);
    await git(destinationWriter, ["config", "user.name", "Forge Test"]);
    await writeFile(join(destinationWriter, "destination.txt"), "destination ahead\n");
    await git(destinationWriter, ["add", "destination.txt"]);
    await git(destinationWriter, ["commit", "-m", "Advance destination"]);
    const destinationAheadObjectId = (
      await git(destinationWriter, ["rev-parse", "HEAD"])
    ).stdout.trim();
    await git(destinationWriter, ["push", "origin", "main"]);

    expect(await observe(fixture.sourceObjectId)).toMatchObject({
      destinationObjectId: destinationAheadObjectId,
      divergence: "destination_ahead",
    });

    await git(fixture.destination, ["update-ref", "refs/heads/main", fixture.sourceObjectId]);
    await writeFile(join(fixture.source, "source.txt"), "source ahead\n");
    await git(fixture.source, ["add", "source.txt"]);
    await git(fixture.source, ["commit", "-m", "Advance canonical source"]);
    const sourceAheadObjectId = (await git(fixture.source, ["rev-parse", "HEAD"])).stdout.trim();
    await git(fixture.source, ["push", fixture.canonical, "HEAD:refs/heads/main"]);

    expect(await observe(sourceAheadObjectId)).toMatchObject({
      destinationObjectId: fixture.sourceObjectId,
      divergence: "source_ahead",
    });

    await git(fixture.destination, ["update-ref", "refs/heads/main", destinationAheadObjectId]);
    expect(await observe(sourceAheadObjectId)).toMatchObject({
      destinationObjectId: destinationAheadObjectId,
      divergence: "diverged",
    });
  });

  test("maps a typed mirror intent through the owned repository runner", async () => {
    const fixture = await makeFixture();
    const intent = ForgeGitHubMirrorIntent.make({
      schema: "openagents.forge.github_mirror.intent.v0.1",
      authority_generation: 3,
      authority_mode: "openagents_git_authoritative",
      destination_github_ref: "refs/heads/main",
      destination_github_repository: "OpenAgentsInc/openagents",
      intent_ref: "intent.forge.github-mirror.local-acceptance",
      promotion_ref: "promotion.forge.local-acceptance",
      redacted: true,
      repository_ref: fixture.repositoryRef,
      requested_at: "2026-07-26T05:00:00.000Z",
      source_object_id: fixture.sourceObjectId,
      source_ref: "refs/heads/main",
      source_refs: ["receipt.forge.local-promotion"],
      tenant_ref: fixture.tenantRef,
    });

    const observed = await fixture.run(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        const runner = makeForgeGitHubMirrorRunner(repository, (input) =>
          input.destinationGithubRepository === "OpenAgentsInc/openagents"
            ? Effect.succeed({ destinationUrl: fixture.destination })
            : Effect.die("unexpected mirror destination"),
        );
        return yield* runner.project(intent);
      }),
    );

    expect(observed).toMatchObject({
      authority_mode: "openagents_git_authoritative",
      destination_github_repository: "OpenAgentsInc/openagents",
      destination_object_id: fixture.sourceObjectId,
      divergence: "in_sync",
      error_reason: null,
      intent_ref: intent.intent_ref,
      repository_ref: fixture.repositoryRef,
      source_object_id: fixture.sourceObjectId,
      tenant_ref: fixture.tenantRef,
    });
    expect(observed.observation_ref).toMatch(/^observation\.forge\.github-mirror\.[a-f0-9]{32}$/u);
  });

  test("derives only an allowlisted credential-free GitHub destination URL", async () => {
    const resolve = makeGitHubHttpsMirrorDestinationResolver({
      allowedRepositories: new Set(["OpenAgentsInc/openagents"]),
      githubToken: Redacted.make("test-token-never-log"),
    });
    const allowed = await Effect.runPromise(
      resolve({
        destinationGithubRepository: "OpenAgentsInc/openagents",
        repositoryRef: "mirror",
        tenantRef: "owner.openagents",
      }),
    );
    expect(allowed.destinationUrl).toBe("https://github.com/OpenAgentsInc/openagents.git");
    expect(allowed.destinationUrl).not.toContain("test-token-never-log");
    expect(allowed.authorizationHeader).toBeDefined();

    const rejected = await Effect.runPromise(
      resolve({
        destinationGithubRepository: "--upload-pack=malicious",
        repositoryRef: "mirror",
        tenantRef: "owner.openagents",
      }).pipe(Effect.flip),
    );
    expect(rejected).toMatchObject({
      reason: "forge_github_mirror_destination_not_allowed",
      retryable: false,
    });
    expect(JSON.stringify(rejected)).not.toContain("test-token-never-log");
  });

  test("rejects github_authoritative intent before destination resolution", async () => {
    const fixture = await makeFixture();
    let destinationResolutions = 0;
    const rejected = await fixture.run(
      Effect.gen(function* () {
        const repository = yield* ForgeGitRepository;
        const runner = makeForgeGitHubMirrorRunner(repository, () => {
          destinationResolutions += 1;
          return Effect.succeed({ destinationUrl: fixture.destination });
        });
        return yield* runner
          .observe(
            ForgeGitHubMirrorIntent.make({
              schema: "openagents.forge.github_mirror.intent.v0.1",
              authority_generation: 1,
              authority_mode: "github_authoritative",
              destination_github_ref: "refs/heads/main",
              destination_github_repository: "OpenAgentsInc/openagents",
              intent_ref: "intent.forge.github-mirror.not-applicable",
              promotion_ref: "promotion.forge.not-applicable",
              redacted: true,
              repository_ref: fixture.repositoryRef,
              requested_at: "2026-07-26T05:00:00.000Z",
              source_object_id: fixture.sourceObjectId,
              source_ref: "refs/heads/main",
              source_refs: [],
              tenant_ref: fixture.tenantRef,
            }),
          )
          .pipe(Effect.flip);
      }),
    );

    expect(rejected.reason).toBe("forge_github_mirror_not_owned_authority");
    expect(destinationResolutions).toBe(0);
    await expect(git(fixture.destination, ["rev-parse", "refs/heads/main"])).rejects.toThrow();
  });
});
