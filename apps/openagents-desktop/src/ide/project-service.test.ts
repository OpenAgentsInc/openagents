import { Context, Effect, Exit, Layer, Schema, Scope } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  IdeAttachmentGenerationSchema,
  IdeCapabilityRefSchema,
  IdeDocumentGenerationSchema,
  IdeNavigationRefSchema,
  IdePlacementGenerationSchema,
  IdePlacementRefSchema,
  IdeServiceGenerationSchema,
} from "./project-contract.ts";
import { makeIdeDocumentFixture, makeIdeProjectFixture } from "./project-fixture.ts";
import {
  IdeProjectService,
  IdeProjectServiceErrorSchema,
  makeIdeProjectLayer,
} from "./project-service.ts";

// Behavior oracle: openagents_desktop.ide_project_generation_fencing.v1

const runProject = <A, E>(
  seed: ReturnType<typeof makeIdeProjectFixture>,
  effect: Effect.Effect<A, E, IdeProjectService>,
) => Effect.runPromise(effect.pipe(Effect.provide(makeIdeProjectLayer(seed))));

describe("IdeProjectService", () => {
  test("decodes the complete expected-failure vocabulary", () => {
    const failures = [
      { _tag: "IdeProject.GrantRevoked", operation: "open", grantRef: "grant.old" },
      {
        _tag: "IdeProject.VersionGap",
        operation: "watch",
        generationKind: "path_index",
        expectedNext: 2,
        observed: 4,
      },
      {
        _tag: "IdeProject.Conflict",
        operation: "save",
        documentRef: "ide.document.one",
        expectedRevisionRef: null,
        actualRevisionRef: "ide.disk-revision.two",
      },
      { _tag: "IdeProject.Cancelled", operation: "search", reason: "superseded" },
      { _tag: "IdeProject.Truncated", operation: "search", resource: "matches", limit: 100 },
      {
        _tag: "IdeProject.CapabilityUnavailable",
        operation: "symbols",
        capabilityRef: "ide.capability.language",
        reason: "not configured",
      },
      {
        _tag: "IdeProject.InvalidRef",
        operation: "navigate",
        refKind: "document",
        value: "../escape",
      },
      { _tag: "IdeProject.Stopped", operation: "snapshot", reason: "project scope closed" },
    ];
    expect(
      failures.every((failure) =>
        Exit.isSuccess(Schema.decodeUnknownExit(IdeProjectServiceErrorSchema)(failure)),
      ),
    ).toBe(true);
  });

  test("admits one document generation and fences stale document navigation", async () => {
    const seed = makeIdeProjectFixture("documents");
    const document = makeIdeDocumentFixture(seed);
    const result = await runProject(
      seed,
      Effect.gen(function* () {
        const project = yield* IdeProjectService;
        yield* project.upsertDocument({
          expectedAttachmentGeneration: seed.generations.attachment,
          expectedDocumentGeneration: null,
          document,
        });
        const stale = yield* project
          .navigate({
            _tag: "File",
            navigationRef: IdeNavigationRefSchema.make("ide.navigation.documents.file"),
            projectRef: seed.identity.projectRef,
            rootRef: seed.identity.rootRef,
            worktreeRef: seed.identity.worktreeRef,
            attachmentGeneration: seed.generations.attachment,
            origin: "explorer",
            fileRef: document.identity.fileRef,
            documentRef: document.identity.documentRef,
            documentGeneration: IdeDocumentGenerationSchema.make(2),
          })
          .pipe(Effect.flip);
        return stale;
      }),
    );
    expect(result._tag).toBe("IdeProject.StaleGeneration");
    if (result._tag === "IdeProject.StaleGeneration") {
      expect(result.generationKind).toBe("document");
    }
  });

  test("prevents a late capability result from regaining authority", async () => {
    const seed = makeIdeProjectFixture("late-result");
    const error = await runProject(
      seed,
      Effect.gen(function* () {
        const project = yield* IdeProjectService;
        yield* project.advanceGeneration({ kind: "path_index", expectedCurrent: 1 });
        return yield* project
          .recordCapability({
            expectedAttachmentGeneration: seed.generations.attachment,
            expectedPlacementGeneration: seed.generations.placement,
            capability: {
              capabilityRef: IdeCapabilityRefSchema.make("ide.capability.late-result.worker"),
              kind: "path_index",
              attachmentGeneration: seed.generations.attachment,
              placementGeneration: seed.generations.placement,
              state: {
                _tag: "Ready",
                serviceGeneration: IdeServiceGenerationSchema.make(1),
                placementRef: IdePlacementRefSchema.make("ide.placement.late-result"),
                evidenceTier: "project_local",
                observedAt: "2026-07-19T00:00:00.000Z",
              },
            },
          })
          .pipe(Effect.flip);
      }),
    );
    expect(error._tag).toBe("IdeProject.StaleGeneration");
    if (error._tag === "IdeProject.StaleGeneration") {
      expect(error.actual).toBe(2);
    }
  });

  test("attachment replacement clears document/navigation authority and stops capabilities", async () => {
    const seed = makeIdeProjectFixture("replacement");
    const document = makeIdeDocumentFixture(seed);
    const next = await runProject(
      seed,
      Effect.gen(function* () {
        const project = yield* IdeProjectService;
        yield* project.upsertDocument({
          expectedAttachmentGeneration: seed.generations.attachment,
          expectedDocumentGeneration: null,
          document,
        });
        yield* project.navigate({
          _tag: "File",
          navigationRef: IdeNavigationRefSchema.make("ide.navigation.replacement.file"),
          projectRef: seed.identity.projectRef,
          rootRef: seed.identity.rootRef,
          worktreeRef: seed.identity.worktreeRef,
          attachmentGeneration: seed.generations.attachment,
          origin: "finder",
          fileRef: document.identity.fileRef,
          documentRef: document.identity.documentRef,
          documentGeneration: document.documentGeneration,
        });
        return yield* project.advanceGeneration({ kind: "attachment", expectedCurrent: 1 });
      }),
    );
    expect(next.generations.attachment).toBe(2);
    expect(next.documents).toEqual([]);
    expect(next.navigation).toEqual([]);
    expect(next.capabilities.every((capability) => capability.state._tag === "Stopped")).toBe(true);
  });

  test("keeps equal relative paths isolated across worktrees and closes explicitly", async () => {
    const left = makeIdeProjectFixture("left");
    const right = makeIdeProjectFixture("right");
    const foreignDocument = makeIdeDocumentFixture(right);
    const result = await runProject(
      left,
      Effect.gen(function* () {
        const project = yield* IdeProjectService;
        const foreign = yield* project
          .upsertDocument({
            expectedAttachmentGeneration: IdeAttachmentGenerationSchema.make(1),
            expectedDocumentGeneration: null,
            document: foreignDocument,
          })
          .pipe(Effect.flip);
        yield* project.stop("fixture complete");
        const stopped = yield* project.snapshot().pipe(Effect.flip);
        return { foreign, stopped };
      }),
    );
    expect(result.foreign._tag).toBe("IdeProject.InvariantViolation");
    expect(result.stopped._tag).toBe("IdeProject.Stopped");
  });

  test("scope closure runs the service finalizer and permanently fences later reads", async () => {
    const seed = makeIdeProjectFixture("scope");
    const stopped = await Effect.runPromise(
      Effect.gen(function* () {
        const scope = yield* Scope.make();
        const context = yield* Layer.buildWithScope(makeIdeProjectLayer(seed), scope);
        const project = Context.get(context, IdeProjectService);
        yield* Scope.close(scope, Exit.void);
        return yield* project.snapshot().pipe(Effect.flip);
      }),
    );
    expect(stopped._tag).toBe("IdeProject.Stopped");
    expect(stopped.reason).toBe("project_scope_closed");
  });

  test("rejects a placement-generation mismatch before recording capability state", async () => {
    const seed = makeIdeProjectFixture("placement");
    const capability = seed.capabilities.at(0);
    if (capability === undefined) throw new Error("fixture capability missing");
    const error = await runProject(
      seed,
      Effect.gen(function* () {
        const project = yield* IdeProjectService;
        return yield* project
          .recordCapability({
            expectedAttachmentGeneration: seed.generations.attachment,
            expectedPlacementGeneration: IdePlacementGenerationSchema.make(2),
            capability,
          })
          .pipe(Effect.flip);
      }),
    );
    expect(error._tag).toBe("IdeProject.StaleGeneration");
    if (error._tag === "IdeProject.StaleGeneration") {
      expect(error.generationKind).toBe("placement");
    }
  });
});
