import { Effect, Exit, Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  IdeAttachmentGenerationSchema,
  IdeCapabilityStateSchema,
  IdeDocumentGenerationSchema,
  IdeGitSnapshotGenerationSchema,
  IdeLanguageGenerationSchema,
  IdeNavigationTargetSchema,
  IdePathIndexGenerationSchema,
  IdePlacementGenerationSchema,
  IdeProjectSnapshotSchema,
  IdeReviewSourceSchema,
} from "./project-contract.ts";
import { makeIdeDocumentFixture, makeIdeProjectFixture } from "./project-fixture.ts";

describe("IDE project boundary contract", () => {
  test("decodes one schema-first project graph and tagged capability lifecycle", async () => {
    const fixture = makeIdeProjectFixture();
    const decoded = await Effect.runPromise(
      Schema.decodeUnknownEffect(IdeProjectSnapshotSchema)(fixture),
    );
    expect(decoded.schemaVersion).toBe("openagents.desktop.ide-project.v1");
    expect(decoded.capabilities[0]?.state._tag).toBe("Ready");
    expect(
      Exit.isSuccess(
        Schema.decodeUnknownExit(IdeCapabilityStateSchema)({
          _tag: "Failed",
          serviceGeneration: 2,
          reason: "fixture failure",
          retry: "bounded_backoff",
          observedAt: "2026-07-19T00:00:00.000Z",
        }),
      ),
    ).toBe(true);
  });

  test("rejects invalid refs and non-positive or fractional generations", () => {
    expect(
      Exit.isFailure(
        Schema.decodeUnknownExit(IdeProjectSnapshotSchema)({
          ...makeIdeProjectFixture(),
          identity: { ...makeIdeProjectFixture().identity, projectRef: "/private/host/path" },
        }),
      ),
    ).toBe(true);
    for (const generationSchema of [
      IdeAttachmentGenerationSchema,
      IdeDocumentGenerationSchema,
      IdeLanguageGenerationSchema,
      IdeGitSnapshotGenerationSchema,
      IdePathIndexGenerationSchema,
      IdePlacementGenerationSchema,
    ]) {
      expect(Exit.isFailure(Schema.decodeUnknownExit(generationSchema)(0))).toBe(true);
      expect(Exit.isFailure(Schema.decodeUnknownExit(generationSchema)(1.5))).toBe(true);
    }
  });

  test("binds navigation to project, worktree, attachment, and document generations", () => {
    const project = makeIdeProjectFixture("navigation");
    const document = makeIdeDocumentFixture(project);
    const target = {
      _tag: "Range",
      navigationRef: "ide.navigation.navigation.range",
      projectRef: project.identity.projectRef,
      rootRef: project.identity.rootRef,
      worktreeRef: project.identity.worktreeRef,
      attachmentGeneration: project.generations.attachment,
      origin: "search",
      fileRef: document.identity.fileRef,
      documentRef: document.identity.documentRef,
      documentGeneration: document.documentGeneration,
      range: { start: { line: 1, column: 1 }, end: { line: 1, column: 4 } },
    };
    for (const origin of [
      "finder",
      "system_open",
      "explorer",
      "quick_open",
      "search",
      "problems",
      "symbol",
      "git",
      "restore",
      "agent",
      "review",
    ] as const) {
      const decoded = Schema.decodeUnknownExit(IdeNavigationTargetSchema)({ ...target, origin });
      expect(Exit.isSuccess(decoded)).toBe(true);
      if (Exit.isSuccess(decoded) && decoded.value._tag === "Range") {
        expect(decoded.value.documentRef).toBe(document.identity.documentRef);
      }
    }
    expect(
      Exit.isFailure(
        Schema.decodeUnknownExit(IdeNavigationTargetSchema)({
          ...target,
          documentGeneration: 0,
        }),
      ),
    ).toBe(true);
  });

  test("keeps review classes distinct instead of collapsing diff authority", () => {
    const project = makeIdeProjectFixture("reviews");
    const common = {
      reviewRef: "ide.review.reviews.source",
      projectRef: project.identity.projectRef,
      worktreeRef: project.identity.worktreeRef,
    };
    const sources = [
      { _tag: "WorkingTree", ...common, gitSnapshotGeneration: 1 },
      { _tag: "Index", ...common, gitSnapshotGeneration: 1 },
      {
        _tag: "CommitRange",
        ...common,
        baseCommitRef: "ide.commit.reviews.base",
        headCommitRef: "ide.commit.reviews.head",
        gitSnapshotGeneration: 1,
      },
      {
        _tag: "Checkpoint",
        ...common,
        checkpointRef: "ide.checkpoint.reviews.one",
        attachmentGeneration: 1,
      },
      {
        _tag: "Proposal",
        ...common,
        proposalRef: "ide.proposal.reviews.one",
        attachmentGeneration: 1,
      },
      {
        _tag: "Conflict",
        ...common,
        documentRef: "ide.document.reviews.one",
        documentGeneration: 1,
        expectedDiskRevisionRef: null,
        actualDiskRevisionRef: "ide.disk-revision.reviews.actual",
      },
    ];
    expect(
      sources.every((source) =>
        Exit.isSuccess(Schema.decodeUnknownExit(IdeReviewSourceSchema)(source)),
      ),
    ).toBe(true);
  });
});
