import { Effect, Result, Schema as S } from "effect";
import { readFile } from "node:fs/promises";
import { describe, expect, it } from "vite-plus/test";

import {
  COLDCARD_BUILD_CONFIG_GENERATOR_DIGEST,
  FORENSIC_SOURCE_MATERIALIZATION_PATH,
  type ForensicScmResolver,
  type ForensicSourceAuthority,
  ForensicSourceMaterializationError,
  ForensicSourceMaterializationRequestSchema,
  coldcardBuildConfigGeneratedInputRegistration,
  forensicGitBlobOid,
  forensicSourceBytesDigest,
  forensicSourceManifestDigest,
  makeForensicSourceDispatchAuthority,
  makeForensicSourceMaterializer,
  makeForensicSourceRoutes,
  makeRegisteredForensicGeneratedInputResolver,
} from "./forensic-source-materializer";

const observedAt = "2026-08-01T15:00:00.000Z";
const retentionExpiresAt = "2026-08-08T15:00:00.000Z";
const textEncoder = new TextEncoder();

type ColdcardFixture = Readonly<{
  repositoryRef: string;
  repository: Readonly<{ provider: "github"; owner: string; name: string }>;
  commitSha: string;
  gitTreeSha: string;
  submodules: ReadonlyArray<
    Readonly<{
      path: string;
      repository: Readonly<{ provider: "github"; owner: string; name: string }>;
      disposition: "materialize" | "exclude" | "stale_declaration";
      commitSha?: string;
      gitTreeSha?: string;
      reasonRef?: string;
    }>
  >;
  paths: ReadonlyArray<
    Readonly<{
      bundlePath: string;
      repositoryPath: string;
      source: "target" | "submodule";
      submodulePath?: string;
      classification: "target" | "dependency";
      required: boolean;
      contentDigest: string;
    }>
  >;
  generatedInputs: ReadonlyArray<
    Readonly<{
      path: string;
      generatorRef: string;
      generatorDigest: string;
      input: Readonly<Record<string, string>>;
      toolchainPins: ReadonlyArray<Readonly<{ toolchainRef: string; digest: string }>>;
      contentDigest: string;
      required: boolean;
    }>
  >;
  incompleteArm: Readonly<{
    omitBundlePath: string;
    expectedStatus: string;
    dispatchAllowed: boolean;
  }>;
}>;

const loadColdcardFixture = async (): Promise<ColdcardFixture> =>
  JSON.parse(
    await readFile(
      new URL("../fixtures/forensics/coldcard/source-manifest.v1.json", import.meta.url),
      "utf8",
    ),
  ) as ColdcardFixture;

const fixtureBytes = (path: string): Uint8Array =>
  textEncoder.encode(`authoritative-fixture:${path}`);

const requestFor = async (fixture: ColdcardFixture) => {
  const paths = await Promise.all(
    fixture.paths.map(async (path) => ({
      bundlePath: path.bundlePath,
      repositoryPath: path.repositoryPath,
      source: path.source,
      ...(path.submodulePath === undefined ? {} : { submodulePath: path.submodulePath }),
      classification: path.classification,
      required: path.required,
      expectedContentDigest: await forensicSourceBytesDigest(fixtureBytes(path.bundlePath)),
    })),
  );
  const raw = {
    targetRef: "target.coldcard.vulnerable",
    repositoryRef: fixture.repositoryRef,
    repository: fixture.repository,
    commitSha: fixture.commitSha,
    expectedGitTreeSha: fixture.gitTreeSha,
    dependencyPolicyRef: "dependency-policy.coldcard.complete.v1",
    authorizationRefs: ["authorization.owner.github-identity"],
    toolchainPins: fixture.generatedInputs.flatMap((input) => input.toolchainPins),
    bundleRef: "bundle.coldcard.complete.fixture",
    coverageRef: "coverage.coldcard.complete.fixture",
    runRef: "run.coldcard.complete.fixture",
    operationRef: "operation.coldcard.materialize.fixture",
    ownerRef: "owner.coldcard.fixture",
    tenantRef: "owner.coldcard.fixture",
    workUnitRef: "work.coldcard.fixture",
    sandboxRef: "sandbox.coldcard.fixture",
    attachmentGeneration: 1,
    resourceGeneration: 1,
    capabilityRef: "capability.coldcard.forensic-source-delivery",
    builderRef: "builder.openagents.forensic-source-materializer.v2",
    submodules: fixture.submodules.map((module) => ({
      path: module.path,
      repository: module.repository,
      disposition: module.disposition,
      ...(module.commitSha === undefined ? {} : { expectedCommitSha: module.commitSha }),
      ...(module.gitTreeSha === undefined ? {} : { expectedGitTreeSha: module.gitTreeSha }),
      ...(module.reasonRef === undefined ? {} : { reasonRef: module.reasonRef }),
    })),
    paths,
    generatedInputs: fixture.generatedInputs.map((input) => ({
      path: input.path,
      generatorRef: input.generatorRef,
      generatorDigest: input.generatorDigest,
      input: input.input,
      toolchainPins: input.toolchainPins,
      expectedContentDigest: input.contentDigest,
      required: input.required,
    })),
    expectedManifestDigest: `sha256:${"0".repeat(64)}`,
    retentionExpiresAt,
    requestedAt: observedAt,
  };
  const preliminary = S.decodeUnknownSync(ForensicSourceMaterializationRequestSchema)(raw, {
    onExcessProperty: "error",
  });
  return S.decodeUnknownSync(ForensicSourceMaterializationRequestSchema)(
    {
      ...raw,
      expectedManifestDigest: forensicSourceManifestDigest(preliminary),
    },
    { onExcessProperty: "error" },
  );
};

const declaredUrl = (module: ColdcardFixture["submodules"][number]): string =>
  `https://github.com/${module.repository.owner}/${module.repository.name}.git`;

const makeHarness = async (
  fixture: ColdcardFixture,
  options: Readonly<{
    omitBundlePath?: string;
    observedSubmoduleCommit?: string;
    corruptArtifactReadback?: boolean;
    invalidDeliveryProof?: boolean;
    cleanupFailsOnce?: boolean;
  }> = {},
) => {
  const request = await requestFor(fixture);
  const artifacts = new Map<string, Uint8Array>();
  const authorities = new Map<string, ForensicSourceAuthority>();
  let artifactPuts = 0;
  let artifactDeletes = 0;
  let installs = 0;
  let cleanups = 0;
  let controllerTime = observedAt;

  const scm: ForensicScmResolver = {
    resolve: (input) =>
      Effect.gen(function* () {
        const target =
          input.repository.owner === fixture.repository.owner &&
          input.repository.name === fixture.repository.name;
        const module = fixture.submodules.find(
          (candidate) =>
            candidate.repository.owner === input.repository.owner &&
            candidate.repository.name === input.repository.name,
        );
        const expectedCommitSha = target ? fixture.commitSha : module?.commitSha;
        const expectedTreeSha = target ? fixture.gitTreeSha : module?.gitTreeSha;
        if (expectedCommitSha === undefined || expectedTreeSha === undefined) {
          return yield* new ForensicSourceMaterializationError({
            code: "source_unavailable",
            message: "fixture repository is unavailable",
            retryable: false,
          });
        }
        const relevantPaths = fixture.paths.filter((path) =>
          target
            ? path.source === "target"
            : path.source === "submodule" && path.submodulePath === module?.path,
        );
        const files = input.materializeFiles
          ? yield* Effect.forEach(
              relevantPaths.filter((path) => path.bundlePath !== options.omitBundlePath),
              (path) =>
                Effect.gen(function* () {
                  const bytes = fixtureBytes(path.bundlePath);
                  return {
                    path: path.repositoryPath,
                    bytes,
                    contentDigest: yield* Effect.promise(() => forensicSourceBytesDigest(bytes)),
                  };
                }),
            )
          : [];
        return {
          repository: input.repository,
          commitSha: expectedCommitSha,
          gitTreeSha: expectedTreeSha,
          files,
          declaredSubmodules: target
            ? fixture.submodules.map((declared, index) => ({
                path: declared.path,
                url: declaredUrl(declared),
                gitlinkCommitSha:
                  declared.disposition === "stale_declaration"
                    ? null
                    : index === 0 && options.observedSubmoduleCommit !== undefined
                      ? options.observedSubmoduleCommit
                      : (declared.commitSha ?? null),
              }))
            : [],
          receiptRef: `receipt.scm.${input.repository.owner}.${input.repository.name}.${expectedCommitSha}`,
          controlNetworkBytes: 1_024,
          controlCredentialClass: "owner_github_identity" as const,
          workspaceReleased: true,
          dirtyState: "clean_immutable_archive" as const,
        };
      }),
  };
  const generated = makeRegisteredForensicGeneratedInputResolver([
    coldcardBuildConfigGeneratedInputRegistration,
  ]);
  const materializer = makeForensicSourceMaterializer({
    now: () => new Date(controllerTime),
    scm,
    generated,
    artifacts: {
      refFor: ({ instanceRef, contentDigest }) =>
        `artifact.forensic-source.${instanceRef}.${contentDigest.slice(7)}`,
      put: ({ instanceRef, contentDigest, bytes }) =>
        Effect.sync(() => {
          artifactPuts += 1;
          const artifactRef = `artifact.forensic-source.${instanceRef}.${contentDigest.slice(7)}`;
          artifacts.set(artifactRef, Uint8Array.from(bytes));
          return {
            artifactRef,
            receiptRef: `receipt.artifact.${contentDigest.slice(7)}`,
          };
        }),
      read: ({ artifactRef }) =>
        Effect.sync(() => {
          const bytes = artifacts.get(artifactRef);
          if (bytes === undefined) return undefined;
          return options.corruptArtifactReadback ? textEncoder.encode("corrupt") : bytes;
        }),
      delete: ({ artifactRef }) =>
        Effect.sync(() => {
          artifactDeletes += 1;
          artifacts.delete(artifactRef);
          return !artifacts.has(artifactRef);
        }),
    },
    authorities: {
      create: (authority) =>
        Effect.sync(() => {
          if (authorities.has(authority.authorityRef)) return false;
          authorities.set(authority.authorityRef, authority);
          return true;
        }),
      read: ({ authorityRef }) => Effect.succeed(authorities.get(authorityRef)),
      compareAndPut: ({ expected, updated }) =>
        Effect.sync(() => {
          const current = authorities.get(expected.authorityRef);
          if (current === undefined || JSON.stringify(current) !== JSON.stringify(expected))
            return false;
          authorities.set(updated.authorityRef, updated);
          return true;
        }),
      listExpired: ({ ownerRef, observedAt: checkedAt }) =>
        Effect.succeed(
          [...authorities.values()].filter(
            (authority) =>
              authority.ownerRef === ownerRef &&
              authority.status !== "cleaned" &&
              Date.parse(authority.retentionExpiresAt) <= Date.parse(checkedAt),
          ),
        ),
    },
    delivery: {
      install: ({ artifactBytes, sourceDigest }) =>
        Effect.gen(function* () {
          installs += 1;
          const observedDigest = yield* Effect.promise(() =>
            forensicSourceBytesDigest(artifactBytes),
          );
          return {
            receiptRef: `receipt.delivery.${sourceDigest.slice(7)}`,
            postCopyDigest: options.invalidDeliveryProof
              ? `sha256:${"f".repeat(64)}`
              : observedDigest,
            sourceReadOnly: !options.invalidDeliveryProof,
            scratchSeparateAndWritable: true,
            guestExternalIpObserved: false,
            guestNetworkBytes: 0,
            credentialMaterializedInGuest: false,
          };
        }),
      cleanup: () =>
        Effect.sync(() => {
          cleanups += 1;
          const cleaned = !options.cleanupFailsOnce || cleanups > 1;
          return {
            guestSourceDeleted: cleaned,
            guestSourceReadbackAbsent: cleaned,
            scratchDeleted: cleaned,
            scratchReadbackAbsent: cleaned,
            capabilityRevoked: cleaned,
            capabilityReadbackRevoked: cleaned,
          };
        }),
      revoke: () =>
        Effect.succeed({
          capabilityRevoked: true,
          capabilityReadbackRevoked: true,
        }),
    },
  });
  return {
    request,
    materializer,
    authorities,
    artifacts,
    setControllerTime: (value: string) => {
      controllerTime = value;
    },
    counts: () => ({ artifactPuts, artifactDeletes, installs, cleanups }),
  };
};

const remanifest = (request: typeof ForensicSourceMaterializationRequestSchema.Type) =>
  S.decodeUnknownSync(ForensicSourceMaterializationRequestSchema)(
    {
      ...request,
      expectedManifestDigest: forensicSourceManifestDigest(request),
    },
    { onExcessProperty: "error" },
  );

describe("forensic source materializer", () => {
  it("pins the real vulnerable Coldcard tree, every gitlink, and the stale declaration", async () => {
    const fixture = await loadColdcardFixture();

    expect(fixture.commitSha).toBe("bcc2c382a324690a2fcf972c0bac3b79bf923f7b");
    expect(fixture.gitTreeSha).toBe("7abc9a4c680b5623fc8a64f70555dd2d3802e488");
    expect(fixture.submodules).toHaveLength(7);
    expect(
      fixture.submodules.filter((module) => module.disposition === "materialize"),
    ).toHaveLength(4);
    expect(fixture.submodules.filter((module) => module.disposition === "exclude")).toHaveLength(2);
    expect(fixture.submodules).toContainEqual(
      expect.objectContaining({
        path: "stm32/mk4-bootloader/hal",
        disposition: "stale_declaration",
      }),
    );
    expect(fixture.generatedInputs[0]?.toolchainPins[0]?.digest).toBe(
      "sha256:e1900ca9116bcd97ae95d51c189a5003a22022a16f2aae389096f2a3200eef46",
    );
    expect(fixture.generatedInputs[0]?.generatorDigest).toBe(
      COLDCARD_BUILD_CONFIG_GENERATOR_DIGEST,
    );
    expect(await forensicGitBlobOid(textEncoder.encode("hello\n"))).toBe(
      "ce013625030ba8dba906f756967f9e9ca394464a",
    );
    expect(
      await forensicSourceBytesDigest(
        await readFile(new URL("./coldcard-build-config-generator.v1.ts", import.meta.url)),
      ),
    ).toBe(COLDCARD_BUILD_CONFIG_GENERATOR_DIGEST);
  });

  it("deterministically materializes the complete authoritative Coldcard arm", async () => {
    const fixture = await loadColdcardFixture();
    const firstHarness = await makeHarness(fixture);
    const secondHarness = await makeHarness(fixture);
    const first = await Effect.runPromise(
      firstHarness.materializer.materialize(firstHarness.request),
    );
    const second = await Effect.runPromise(
      secondHarness.materializer.materialize(secondHarness.request),
    );

    expect(first.coverage.status).toBe("complete");
    expect(first.bundle?.declaredSubmodules).toHaveLength(4);
    expect(first.bundle?.sourceDigest).toBe(second.bundle?.sourceDigest);
    expect(first.receipt).toMatchObject({
      outcome: "succeeded",
      sourceReadOnly: true,
      scratchSeparateAndWritable: true,
      guestExternalIpObserved: false,
      guestNetworkBytes: 0,
      credentialMaterializedInGuest: false,
      controlCredentialClass: "owner_github_identity",
      scmDirtyState: "clean_immutable_archive",
    });
    expect(firstHarness.counts()).toEqual({
      artifactPuts: 1,
      artifactDeletes: 0,
      installs: 1,
      cleanups: 0,
    });
    await expect(
      Effect.runPromise(firstHarness.materializer.materialize(firstHarness.request)),
    ).rejects.toBeDefined();
    expect(firstHarness.counts().artifactPuts).toBe(1);
  });

  it("persists incomplete coverage and refuses dispatch before artifact delivery", async () => {
    const fixture = await loadColdcardFixture();
    const harness = await makeHarness(fixture, {
      omitBundlePath: fixture.incompleteArm.omitBundlePath,
    });
    const result = await Effect.runPromise(harness.materializer.materialize(harness.request));
    const dispatch = await Effect.runPromise(
      Effect.result(
        makeForensicSourceDispatchAuthority({
          authorities: {
            create: () => Effect.die("authority create must not run"),
            read: ({ authorityRef }) => Effect.succeed(harness.authorities.get(authorityRef)),
            compareAndPut: () => Effect.succeed(false),
            listExpired: () => Effect.succeed([]),
          },
          artifacts: {
            refFor: () => "artifact.forensic-source.unused",
            put: () => Effect.die("artifact put must not run"),
            read: () => Effect.die("artifact read must not run"),
            delete: () => Effect.die("artifact delete must not run"),
          },
          delivery: {
            install: () => Effect.die("source install must not run"),
            cleanup: () => Effect.die("source cleanup must not run"),
            revoke: () => Effect.die("source revoke must not run"),
          },
        }).assertReady({
          ownerRef: result.authority.ownerRef,
          tenantRef: result.authority.tenantRef,
          workUnitRef: result.authority.workUnitRef,
          runRef: result.authority.runRef,
          sandboxRef: result.authority.sandboxRef,
          attachmentGeneration: result.authority.attachmentGeneration,
          resourceGeneration: result.authority.resourceGeneration,
          authorityRef: result.authority.authorityRef,
          bundleRef: result.authority.bundleRef,
          coverageRef: result.authority.coverageRef,
          coverageDigest: result.authority.coverageDigest,
          sourceDigest: `sha256:${"0".repeat(64)}`,
          materializationReceiptRef: result.authority.materializationReceiptRef,
        }),
      ),
    );

    expect(result.bundle).toBeNull();
    expect(result.coverage.status).toBe("incomplete");
    expect(result.coverage.entries).toContainEqual(
      expect.objectContaining({
        path: fixture.incompleteArm.omitBundlePath,
        presence: "absent",
      }),
    );
    expect(Result.isFailure(dispatch)).toBe(true);
    expect(harness.counts()).toEqual({
      artifactPuts: 0,
      artifactDeletes: 0,
      installs: 0,
      cleanups: 0,
    });
  });

  it("fails closed on submodule, generator, toolchain, artifact, and post-copy drift", async () => {
    const fixture = await loadColdcardFixture();
    const wrongSubmodule = await makeHarness(fixture, {
      observedSubmoduleCommit: "0".repeat(40),
    });
    const wrongGenerator = await makeHarness(fixture);
    const generated = wrongGenerator.request.generatedInputs[0];
    if (generated === undefined) throw new Error("generated plan is required");
    const generatedToolchain = generated.toolchainPins[0];
    if (generatedToolchain === undefined) throw new Error("generated toolchain is required");
    const wrongGeneratorRequest = remanifest({
      ...wrongGenerator.request,
      generatedInputs: [{ ...generated, generatorDigest: `sha256:${"1".repeat(64)}` }],
    });
    const wrongToolchainRequest = remanifest({
      ...wrongGenerator.request,
      generatedInputs: [
        {
          ...generated,
          toolchainPins: [{ ...generatedToolchain, digest: `sha256:${"2".repeat(64)}` }],
        },
      ],
    });
    const wrongGeneratorInputRequest = remanifest({
      ...wrongGenerator.request,
      generatedInputs: [
        {
          ...generated,
          input: { ...generated.input, commit: "0".repeat(40) },
        },
      ],
    });
    const firstPath = wrongGenerator.request.paths[0];
    if (firstPath === undefined) throw new Error("source path is required");
    const duplicateRepositoryPathRequest = remanifest({
      ...wrongGenerator.request,
      paths: [
        ...wrongGenerator.request.paths,
        { ...firstPath, bundlePath: "duplicate/source-path" },
      ],
    });
    const corruptArtifact = await makeHarness(fixture, {
      corruptArtifactReadback: true,
    });
    const invalidDelivery = await makeHarness(fixture, {
      invalidDeliveryProof: true,
    });

    const results = await Promise.all([
      Effect.runPromise(
        Effect.result(wrongSubmodule.materializer.materialize(wrongSubmodule.request)),
      ),
      Effect.runPromise(
        Effect.result(wrongGenerator.materializer.materialize(wrongGeneratorRequest)),
      ),
      Effect.runPromise(
        Effect.result(wrongGenerator.materializer.materialize(wrongToolchainRequest)),
      ),
      Effect.runPromise(
        Effect.result(wrongGenerator.materializer.materialize(wrongGeneratorInputRequest)),
      ),
      Effect.runPromise(
        Effect.result(wrongGenerator.materializer.materialize(duplicateRepositoryPathRequest)),
      ),
      Effect.runPromise(
        Effect.result(corruptArtifact.materializer.materialize(corruptArtifact.request)),
      ),
      Effect.runPromise(
        Effect.result(invalidDelivery.materializer.materialize(invalidDelivery.request)),
      ),
    ]);

    expect(results.every(Result.isFailure)).toBe(true);
    expect(invalidDelivery.counts().artifactDeletes).toBe(1);
  });

  it("requires exact durable authority and cleans explicitly or at retention expiry", async () => {
    const fixture = await loadColdcardFixture();
    const explicitHarness = await makeHarness(fixture);
    const explicitResult = await Effect.runPromise(
      explicitHarness.materializer.materialize(explicitHarness.request),
    );
    explicitHarness.setControllerTime("2026-08-09T15:00:00.000Z");
    const cleanup = await Effect.runPromise(
      explicitHarness.materializer.cleanup({
        ownerRef: explicitResult.authority.ownerRef,
        authorityRef: explicitResult.authority.authorityRef,
        observedAt: "2026-08-09T15:00:00.000Z",
      }),
    );
    const expiryHarness = await makeHarness(fixture);
    await Effect.runPromise(expiryHarness.materializer.materialize(expiryHarness.request));
    expiryHarness.setControllerTime("2026-08-09T15:00:00.000Z");
    const swept = await Effect.runPromise(
      expiryHarness.materializer.sweepExpired({
        ownerRef: expiryHarness.request.ownerRef,
        observedAt: "2026-08-09T15:00:00.000Z",
      }),
    );

    expect(cleanup).toMatchObject({
      outcome: "cleaned",
      artifactReadbackAbsent: true,
      guestSourceReadbackAbsent: true,
      scratchReadbackAbsent: true,
      capabilityReadbackRevoked: true,
      reason: "explicit",
    });
    expect(swept).toHaveLength(1);
    expect(swept[0]).toMatchObject({
      outcome: "cleaned",
      reason: "retention_expired",
    });

    const retryHarness = await makeHarness(fixture, { cleanupFailsOnce: true });
    const retryResult = await Effect.runPromise(
      retryHarness.materializer.materialize(retryHarness.request),
    );
    retryHarness.setControllerTime("2026-08-09T15:00:00.000Z");
    const firstCleanup = await Effect.runPromise(
      retryHarness.materializer.cleanup({
        ownerRef: retryResult.authority.ownerRef,
        authorityRef: retryResult.authority.authorityRef,
        observedAt: "2026-08-09T15:00:00.000Z",
      }),
    );
    const secondCleanup = await Effect.runPromise(
      retryHarness.materializer.cleanup({
        ownerRef: retryResult.authority.ownerRef,
        authorityRef: retryResult.authority.authorityRef,
        observedAt: "2026-08-09T15:00:01.000Z",
      }),
    );
    expect(firstCleanup.outcome).toBe("recovery_required");
    expect(secondCleanup.outcome).toBe("cleaned");
  });

  it("holds an exact guest-verified dispatch lease against concurrent cleanup", async () => {
    const fixture = await loadColdcardFixture();
    const harness = await makeHarness(fixture);
    const result = await Effect.runPromise(harness.materializer.materialize(harness.request));
    if (result.authority.sourceDigest === undefined)
      throw new Error("ready source authority requires a digest");
    const authorityStore = {
      create: (authority: ForensicSourceAuthority) =>
        Effect.sync(() => {
          if (harness.authorities.has(authority.authorityRef)) return false;
          harness.authorities.set(authority.authorityRef, authority);
          return true;
        }),
      read: ({ authorityRef }: Readonly<{ authorityRef: string }>) =>
        Effect.succeed(harness.authorities.get(authorityRef)),
      compareAndPut: ({
        expected,
        updated,
      }: Readonly<{
        expected: ForensicSourceAuthority;
        updated: ForensicSourceAuthority;
      }>) =>
        Effect.sync(() => {
          const current = harness.authorities.get(expected.authorityRef);
          if (JSON.stringify(current) !== JSON.stringify(expected)) return false;
          harness.authorities.set(updated.authorityRef, updated);
          return true;
        }),
      listExpired: () => Effect.succeed([]),
    };
    const lease = await Effect.runPromise(
      makeForensicSourceDispatchAuthority({
        authorities: authorityStore,
        artifacts: {
          refFor: () => "artifact.forensic-source.unused",
          put: () => Effect.die("artifact put must not run"),
          read: ({ artifactRef }) => Effect.succeed(harness.artifacts.get(artifactRef)),
          delete: () => Effect.die("artifact delete must not run"),
        },
        delivery: {
          install: ({ sourceDigest }) =>
            Effect.succeed({
              receiptRef: "receipt.delivery.dispatch-readback",
              postCopyDigest: sourceDigest,
              sourceReadOnly: true,
              scratchSeparateAndWritable: true,
              guestExternalIpObserved: false,
              guestNetworkBytes: 0,
              credentialMaterializedInGuest: false,
            }),
          cleanup: () => Effect.die("cleanup must not run during verification"),
          revoke: () => Effect.die("revoke must not run during verification"),
        },
        now: () => new Date(observedAt),
      }).assertReady({
        ownerRef: result.authority.ownerRef,
        tenantRef: result.authority.tenantRef,
        workUnitRef: result.authority.workUnitRef,
        runRef: result.authority.runRef,
        sandboxRef: result.authority.sandboxRef,
        attachmentGeneration: result.authority.attachmentGeneration,
        resourceGeneration: result.authority.resourceGeneration,
        authorityRef: result.authority.authorityRef,
        bundleRef: result.authority.bundleRef,
        coverageRef: result.authority.coverageRef,
        coverageDigest: result.authority.coverageDigest,
        sourceDigest: result.authority.sourceDigest,
        materializationReceiptRef: result.authority.materializationReceiptRef,
      }),
    );

    expect(harness.authorities.get(result.authority.authorityRef)?.status).toBe("dispatching");
    await expect(
      Effect.runPromise(
        harness.materializer.cleanup({
          ownerRef: result.authority.ownerRef,
          authorityRef: result.authority.authorityRef,
          observedAt,
        }),
      ),
    ).rejects.toBeDefined();
    await Effect.runPromise(lease.release);
    expect(harness.authorities.get(result.authority.authorityRef)?.status).toBe("ready");
  });

  it("keeps the authenticated production route default-off", async () => {
    const routes = makeForensicSourceRoutes({
      authenticateOwner: async () => ({ userId: "owner.coldcard.fixture" }),
      enabled: () => false,
      materializer: () => Effect.die("materializer must not run while disabled"),
    });
    const response = await Effect.runPromise(
      routes.handle(
        new Request(`https://api.openagents.com${FORENSIC_SOURCE_MATERIALIZATION_PATH}`, {
          method: "POST",
          body: JSON.stringify({ _tag: "Materialize" }),
        }),
        {},
        {} as ExecutionContext,
      ),
    );

    expect(response.status).toBe(503);
    await expect(response.json()).resolves.toMatchObject({
      error: "runtime_not_admitted",
    });
  });
});
