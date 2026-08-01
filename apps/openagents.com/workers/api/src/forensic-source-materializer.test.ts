import { strictDecode } from "@openagentsinc/forensic-contract";
import { Effect, Result } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  FORENSIC_SOURCE_MATERIALIZATION_PATH,
  FORENSIC_SOURCE_OBJECT_VERSION,
  ForensicSourceMaterializationError,
  ForensicSourceMaterializationRequestSchema,
  forensicDependencyManifestDigest,
  forensicSourceBytesDigest,
  makeForensicSourceMaterializer,
  makeForensicSourceRoutes,
  type ForensicSourceObject,
} from "./forensic-source-materializer";

const digest = (character: string) => `sha256:${character.repeat(64)}`;
const targetCommit = "bcc2c382a324690a2fcf972c0bac3b79bf923f7b";
const targetTreeDigest = digest("a");
const now = "2026-08-01T15:00:00.000Z";
const coldcardVulnerableSubmoduleCommits = [
  { path: "external/libngu", commitSha: "537519a829259622ea6b0334fbafd6cae852852f" },
  {
    path: "external/micropython",
    commitSha: "4107246f8a080807b62c3b4838e71e812ea68b6f",
  },
  {
    path: "external/ckcc-protocol",
    commitSha: "3d1dfa858beb58b8dac37d8c66d7aed2909812f2",
  },
  { path: "external/mpy-qr", commitSha: "11347d83f4eb325b10676a4eb8e17deccfe0df44" },
];

const contentObject = async (input: {
  path: string;
  classification: "target" | "dependency" | "generated";
  commitSha: string;
  treeDigest: string;
  submodulePath?: string;
  generatedByRef?: string;
  toolchainRefs?: ReadonlyArray<string>;
}): Promise<ForensicSourceObject> => {
  const bytes = new TextEncoder().encode(`fixture:${input.path}`);
  return {
    schema: FORENSIC_SOURCE_OBJECT_VERSION,
    sourceObjectRef: `source-object.${input.path.replaceAll("/", ".")}`,
    ownerRef: "owner.coldcard.fixture",
    tenantRef: "owner.coldcard.fixture",
    workUnitRef: "work.coldcard.fixture",
    repositoryRef: "repository.coldcard.firmware",
    commitSha: input.commitSha,
    treeDigest: input.treeDigest,
    path: input.path,
    classification: input.classification,
    ...(input.submodulePath === undefined ? {} : { submodulePath: input.submodulePath }),
    ...(input.generatedByRef === undefined ? {} : { generatedByRef: input.generatedByRef }),
    toolchainRefs: input.toolchainRefs ?? [],
    contentDigest: await forensicSourceBytesDigest(bytes),
    contentBase64: btoa(String.fromCharCode(...bytes)),
  };
};

const fixture = async () => {
  const declaredSubmodules = coldcardVulnerableSubmoduleCommits.map((pin, index) => ({
    ...pin,
    treeDigest: digest(String(index + 1)),
  }));
  const objects = [
    await contentObject({
      path: "stm32/COLDCARD/mpconfigboard.h",
      classification: "target",
      commitSha: targetCommit,
      treeDigest: targetTreeDigest,
    }),
    ...(await Promise.all(
      declaredSubmodules.map((pin) =>
        contentObject({
          path: `${pin.path}/SOURCE.txt`,
          classification: "dependency",
          commitSha: pin.commitSha,
          treeDigest: pin.treeDigest,
          submodulePath: pin.path,
        }),
      ),
    )),
    await contentObject({
      path: "generated/coldcard-build-config.h",
      classification: "generated",
      commitSha: targetCommit,
      treeDigest: targetTreeDigest,
      generatedByRef: "generator.coldcard.build-config.v1",
      toolchainRefs: ["toolchain.arm-none-eabi.v1"],
    }),
  ];
  const rawRequest = {
    target: {
      schema: "openagents.forensic_target_snapshot.v1",
      targetRef: "target.coldcard.vulnerable",
      repositoryRef: "repository.coldcard.firmware",
      commitSha: targetCommit,
      sourceDigest: digest("f"),
      dirtyState: "clean",
      dependencyPolicyRef: "dependency-policy.coldcard.complete.v1",
      toolchainRefs: ["toolchain.arm-none-eabi.v1"],
      authorizationRefs: ["authorization.coldcard.fixture"],
      capturedAt: now,
    },
    bundleRef: "bundle.coldcard.complete.fixture",
    coverageRef: "coverage.coldcard.complete.fixture",
    runRef: "run.coldcard.complete.fixture",
    operationRef: "operation.coldcard.materialize.fixture",
    ownerRef: "owner.coldcard.fixture",
    tenantRef: "owner.coldcard.fixture",
    workUnitRef: "work.coldcard.fixture",
    sandboxRef: "sandbox.coldcard.fixture",
    resourceGeneration: 1,
    capabilityRef: "capability.coldcard.source-materializer",
    builderRef: "builder.openagents.forensic-source-materializer.v1",
    targetTreeDigest,
    declaredSubmodules,
    expectations: objects.map((object) => ({
      path: object.path,
      classification: object.classification,
      required: true,
      sourceObjectRef: object.sourceObjectRef,
      expectedContentDigest: object.contentDigest,
      ...(object.submodulePath === undefined ? {} : { submodulePath: object.submodulePath }),
      ...(object.generatedByRef === undefined ? {} : { generatedByRef: object.generatedByRef }),
    })),
    expectedDependencyManifestDigest: digest("0"),
    retentionExpiresAt: "2026-08-08T15:00:00.000Z",
    requestedAt: now,
  };
  const preliminary = strictDecode(ForensicSourceMaterializationRequestSchema, rawRequest);
  const request = strictDecode(ForensicSourceMaterializationRequestSchema, {
    ...rawRequest,
    expectedDependencyManifestDigest: forensicDependencyManifestDigest(preliminary),
  });
  return { declaredSubmodules, objects, request };
};

const harness = (
  objects: ReadonlyArray<ForensicSourceObject>,
  options: Readonly<{ failDelivery?: boolean }> = {},
) => {
  const values = new Map(objects.map((object) => [object.sourceObjectRef, object]));
  let artifactPuts = 0;
  let artifactDeletes = 0;
  let installs = 0;
  const materializer = makeForensicSourceMaterializer({
    objects: {
      read: ({ sourceObjectRef }) => Effect.succeed(values.get(sourceObjectRef)),
      put: (sourceObject) =>
        Effect.sync(() => {
          values.set(sourceObject.sourceObjectRef, sourceObject);
        }),
    },
    artifacts: {
      put: ({ contentDigest }) => {
        artifactPuts += 1;
        return Effect.succeed(`artifact.forensic-source.${contentDigest.slice(7)}`);
      },
      delete: () => {
        artifactDeletes += 1;
        return Effect.succeed(true);
      },
    },
    delivery: {
      install: ({ entries }) => {
        installs += 1;
        if (options.failDelivery) {
          return Effect.fail(
            new ForensicSourceMaterializationError({
              code: "delivery_failed",
              message: "fixture delivery failed",
              retryable: true,
            }),
          );
        }
        return Effect.succeed(
          entries.map((entry) => `receipt.delivery.${entry.contentDigest.slice(7)}`),
        );
      },
      cleanup: () =>
        Effect.succeed({ guestSourceDeleted: true, scratchDeleted: true, grantsRevoked: true }),
    },
  });
  return {
    materializer,
    values,
    counts: () => ({ artifactPuts, artifactDeletes, installs }),
  };
};

describe("forensic source materializer", () => {
  it("stages only bounded objects whose bytes match their digest", async () => {
    const { objects } = await fixture();
    const [sourceObject] = objects;
    if (sourceObject === undefined) throw new Error("fixture source object is required");
    const testHarness = harness([]);
    const staged = await Effect.runPromise(testHarness.materializer.stage(sourceObject));
    const mismatched = await Effect.runPromise(
      Effect.result(
        testHarness.materializer.stage({ ...sourceObject, contentBase64: btoa("drift") }),
      ),
    );

    expect(staged.sourceObjectRef).toBe(sourceObject.sourceObjectRef);
    expect(testHarness.values.get(staged.sourceObjectRef)).toEqual(sourceObject);
    expect(Result.isFailure(mismatched)).toBe(true);
  });

  it("materializes the complete Coldcard arm deterministically with all four pins", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { declaredSubmodules, objects, request } = yield* Effect.promise(fixture);
        const firstHarness = harness(objects);
        const first = yield* firstHarness.materializer.materialize(request);
        const second = yield* harness(objects).materializer.materialize({
          ...request,
          expectations: [...request.expectations].reverse(),
        });

        expect(first.coverage.status).toBe("complete");
        expect(first.bundle?.declaredSubmodules.map((pin) => pin.commitSha)).toEqual(
          coldcardVulnerableSubmoduleCommits.map((pin) => pin.commitSha),
        );
        expect(first.bundle?.declaredSubmodules).toEqual(declaredSubmodules);
        expect(first.bundle?.sourceDigest).toBe(second.bundle?.sourceDigest);
        expect(first.receipt.networkBytes).toBe(0);
        expect(first.receipt.scmCredentialMaterialized).toBe(false);
        expect(firstHarness.counts()).toEqual({
          artifactPuts: 1,
          artifactDeletes: 0,
          installs: 1,
        });
      }),
    );
  });

  it("blocks the incomplete arm before artifact storage or guest delivery", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { objects, request } = yield* Effect.promise(fixture);
        const testHarness = harness(
          objects.filter((object) => object.submodulePath !== "external/mpy-qr"),
        );
        const result = yield* testHarness.materializer.materialize(request);

        expect(result.bundle).toBeNull();
        expect(result.coverage.status).toBe("incomplete");
        expect(result.coverage.entries).toContainEqual(
          expect.objectContaining({
            path: "external/mpy-qr/SOURCE.txt",
            presence: "absent",
            required: true,
          }),
        );
        expect(result.receipt.outcome).toBe("incomplete");
        expect(testHarness.counts()).toEqual({
          artifactPuts: 0,
          artifactDeletes: 0,
          installs: 0,
        });
      }),
    );
  });

  it("fails closed on submodule, generated-input, toolchain, and byte drift", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { objects, request } = yield* Effect.promise(fixture);
        const wrongPinObjects = objects.map((object, index) =>
          index === 1 ? { ...object, commitSha: "0".repeat(40) } : object,
        );
        const wrongBytes = objects.map((object, index) =>
          index === 0 ? { ...object, contentBase64: btoa("different") } : object,
        );
        const wrongGenerator = objects.map((object) =>
          object.classification === "generated"
            ? { ...object, generatedByRef: "generator.wrong.v1" }
            : object,
        );
        const wrongToolchain = objects.map((object) =>
          object.classification === "generated"
            ? { ...object, toolchainRefs: ["toolchain.wrong.v1"] }
            : object,
        );

        const wrongPinResult = yield* Effect.result(
          harness(wrongPinObjects).materializer.materialize(request),
        );
        const wrongBytesResult = yield* Effect.result(
          harness(wrongBytes).materializer.materialize(request),
        );
        const wrongGeneratorResult = yield* Effect.result(
          harness(wrongGenerator).materializer.materialize(request),
        );
        const wrongToolchainResult = yield* Effect.result(
          harness(wrongToolchain).materializer.materialize(request),
        );

        for (const result of [
          wrongPinResult,
          wrongBytesResult,
          wrongGeneratorResult,
          wrongToolchainResult,
        ]) {
          expect(Result.isFailure(result)).toBe(true);
          if (Result.isFailure(result)) expect(result.failure.code).toBe("source_mismatch");
        }
      }),
    );
  });

  it("emits authoritative cleanup truth", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { objects, request } = yield* Effect.promise(fixture);
        const testHarness = harness(objects);
        const result = yield* testHarness.materializer.materialize(request);
        const cleanup = yield* testHarness.materializer.cleanup({
          receipt: result.receipt,
          observedAt: now,
        });

        expect(cleanup).toMatchObject({
          outcome: "cleaned",
          artifactDeleted: true,
          guestSourceDeleted: true,
          scratchDeleted: true,
          grantsRevoked: true,
        });
      }),
    );
  });

  it("removes the private artifact when guest delivery fails", async () => {
    const { objects, request } = await fixture();
    const testHarness = harness(objects, { failDelivery: true });
    const result = await Effect.runPromise(
      Effect.result(testHarness.materializer.materialize(request)),
    );

    expect(Result.isFailure(result)).toBe(true);
    expect(testHarness.counts()).toEqual({
      artifactPuts: 1,
      artifactDeletes: 1,
      installs: 1,
    });
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
    await expect(response.json()).resolves.toMatchObject({ error: "runtime_not_admitted" });
  });
});
