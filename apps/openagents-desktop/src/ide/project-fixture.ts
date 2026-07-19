import {
  IdeAttachmentGenerationSchema,
  IdeAttachmentRefSchema,
  IdeCapabilityRefSchema,
  IdeDiskRevisionRefSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentRefSchema,
  IdeDocumentSnapshotSchema,
  IdeFileRefSchema,
  IdeGitSnapshotGenerationSchema,
  IdeLanguageGenerationSchema,
  IdePathIndexGenerationSchema,
  IdePlacementRefSchema,
  IdePlacementGenerationSchema,
  IdeProjectRefSchema,
  IdeProjectSnapshotSchema,
  IdeRootRefSchema,
  IdeServiceGenerationSchema,
  IdeSessionRefSchema,
  IdeTimestampSchema,
  IdeWorktreeRefSchema,
  type IdeDocumentSnapshot,
  type IdeProjectSnapshot,
} from "./project-contract.ts";

export const ideFixtureTimestamp = IdeTimestampSchema.make("2026-07-19T00:00:00.000Z");

export const makeIdeProjectFixture = (suffix = "fixture"): IdeProjectSnapshot =>
  IdeProjectSnapshotSchema.make({
    schemaVersion: "openagents.desktop.ide-project.v1",
    identity: {
      projectRef: IdeProjectRefSchema.make(`ide.project.${suffix}`),
      rootRef: IdeRootRefSchema.make(`ide.root.${suffix}`),
      worktreeRef: IdeWorktreeRefSchema.make(`ide.worktree.${suffix}`),
      attachmentRef: IdeAttachmentRefSchema.make(`ide.attachment.${suffix}`),
      sessionRef: IdeSessionRefSchema.make(`ide.session.${suffix}`),
      grantRef: `workspace.grant.${suffix}`,
    },
    generations: {
      attachment: IdeAttachmentGenerationSchema.make(1),
      pathIndex: IdePathIndexGenerationSchema.make(1),
      language: IdeLanguageGenerationSchema.make(1),
      gitSnapshot: IdeGitSnapshotGenerationSchema.make(1),
      placement: IdePlacementGenerationSchema.make(1),
    },
    gitSnapshotRef: null,
    documents: [],
    excerpts: [],
    proposals: [],
    capabilities: [
      {
        capabilityRef: IdeCapabilityRefSchema.make(`ide.capability.${suffix}.path-index`),
        kind: "path_index",
        attachmentGeneration: IdeAttachmentGenerationSchema.make(1),
        placementGeneration: IdePlacementGenerationSchema.make(1),
        state: {
          _tag: "Ready",
          serviceGeneration: IdeServiceGenerationSchema.make(1),
          placementRef: IdePlacementRefSchema.make(`ide.placement.${suffix}`),
          evidenceTier: "project_local",
          observedAt: ideFixtureTimestamp,
        },
      },
    ],
    navigation: [],
    reviewSources: [],
    lastEvidenceRef: null,
  });

export const makeIdeDocumentFixture = (
  snapshot: IdeProjectSnapshot,
  documentGeneration = 1,
): IdeDocumentSnapshot =>
  IdeDocumentSnapshotSchema.make({
    identity: {
      projectRef: snapshot.identity.projectRef,
      rootRef: snapshot.identity.rootRef,
      worktreeRef: snapshot.identity.worktreeRef,
      fileRef: IdeFileRefSchema.make(`ide.file.${snapshot.identity.worktreeRef}.src-index`),
      documentRef: IdeDocumentRefSchema.make(
        `ide.document.${snapshot.identity.worktreeRef}.src-index`,
      ),
      pathRef: "src/index.ts",
    },
    attachmentGeneration: snapshot.generations.attachment,
    documentGeneration: IdeDocumentGenerationSchema.make(documentGeneration),
    diskRevisionRef: IdeDiskRevisionRefSchema.make(
      `ide.disk-revision.${snapshot.identity.worktreeRef}.${documentGeneration}`,
    ),
    encoding: "utf-8",
    lineEnding: "lf",
    lifecycle: { _tag: "Ready", dirty: false, recoverable: true },
  });
