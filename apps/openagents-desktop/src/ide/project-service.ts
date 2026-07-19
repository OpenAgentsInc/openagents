import { Context, Effect, Layer, Ref, Schema } from "effect";

import {
  IdeAdvanceGenerationInputSchema,
  IdeAttachmentGenerationSchema,
  IdeCapabilityUpdateInputSchema,
  IdeDocumentGenerationSchema,
  IdeDocumentUpsertInputSchema,
  IdeGitSnapshotGenerationSchema,
  IdeLanguageGenerationSchema,
  IdeNavigationTargetSchema,
  IdePathIndexGenerationSchema,
  IdePlacementGenerationSchema,
  IdeProjectSnapshotSchema,
  IdeTimestampSchema,
  type IdeAdvanceGenerationInput,
  type IdeCapabilityKind,
  type IdeCapabilitySnapshot,
  type IdeCapabilityUpdateInput,
  type IdeDocumentSnapshot,
  type IdeDocumentUpsertInput,
  type IdeGenerationKind,
  type IdeNavigationTarget,
  type IdeProjectSnapshot,
  type IdeTimestamp,
} from "./project-contract.ts";

export class IdeProjectInvalidInput extends Schema.TaggedErrorClass<IdeProjectInvalidInput>()(
  "IdeProject.InvalidInput",
  {
    operation: Schema.String,
    detail: Schema.String,
  },
) {}

export class IdeProjectStaleGeneration extends Schema.TaggedErrorClass<IdeProjectStaleGeneration>()(
  "IdeProject.StaleGeneration",
  {
    operation: Schema.String,
    generationKind: Schema.String,
    expected: Schema.Number,
    actual: Schema.Number,
  },
) {}

export class IdeProjectInvariantViolation extends Schema.TaggedErrorClass<IdeProjectInvariantViolation>()(
  "IdeProject.InvariantViolation",
  {
    operation: Schema.String,
    detail: Schema.String,
  },
) {}

export class IdeProjectStopped extends Schema.TaggedErrorClass<IdeProjectStopped>()(
  "IdeProject.Stopped",
  {
    operation: Schema.String,
    reason: Schema.String,
  },
) {}

export class IdeProjectGrantRevoked extends Schema.TaggedErrorClass<IdeProjectGrantRevoked>()(
  "IdeProject.GrantRevoked",
  {
    operation: Schema.String,
    grantRef: Schema.String,
  },
) {}

export class IdeProjectVersionGap extends Schema.TaggedErrorClass<IdeProjectVersionGap>()(
  "IdeProject.VersionGap",
  {
    operation: Schema.String,
    generationKind: Schema.String,
    expectedNext: Schema.Number,
    observed: Schema.Number,
  },
) {}

export class IdeProjectConflict extends Schema.TaggedErrorClass<IdeProjectConflict>()(
  "IdeProject.Conflict",
  {
    operation: Schema.String,
    documentRef: Schema.String,
    expectedRevisionRef: Schema.NullOr(Schema.String),
    actualRevisionRef: Schema.NullOr(Schema.String),
  },
) {}

export class IdeProjectCancelled extends Schema.TaggedErrorClass<IdeProjectCancelled>()(
  "IdeProject.Cancelled",
  {
    operation: Schema.String,
    reason: Schema.String,
  },
) {}

export class IdeProjectTruncated extends Schema.TaggedErrorClass<IdeProjectTruncated>()(
  "IdeProject.Truncated",
  {
    operation: Schema.String,
    resource: Schema.String,
    limit: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(1)),
  },
) {}

export class IdeProjectCapabilityUnavailable extends Schema.TaggedErrorClass<IdeProjectCapabilityUnavailable>()(
  "IdeProject.CapabilityUnavailable",
  {
    operation: Schema.String,
    capabilityRef: Schema.String,
    reason: Schema.String,
  },
) {}

export class IdeProjectInvalidRef extends Schema.TaggedErrorClass<IdeProjectInvalidRef>()(
  "IdeProject.InvalidRef",
  {
    operation: Schema.String,
    refKind: Schema.String,
    value: Schema.String,
  },
) {}

export const IdeProjectServiceErrorSchema = Schema.Union([
  IdeProjectInvalidInput,
  IdeProjectStaleGeneration,
  IdeProjectInvariantViolation,
  IdeProjectStopped,
  IdeProjectGrantRevoked,
  IdeProjectVersionGap,
  IdeProjectConflict,
  IdeProjectCancelled,
  IdeProjectTruncated,
  IdeProjectCapabilityUnavailable,
  IdeProjectInvalidRef,
]);
export type IdeProjectServiceError = typeof IdeProjectServiceErrorSchema.Type;

export interface IdeProjectServiceShape {
  readonly snapshot: () => Effect.Effect<IdeProjectSnapshot, IdeProjectStopped>;
  readonly upsertDocument: (
    input: IdeDocumentUpsertInput,
  ) => Effect.Effect<IdeProjectSnapshot, IdeProjectServiceError>;
  readonly recordCapability: (
    input: IdeCapabilityUpdateInput,
  ) => Effect.Effect<IdeProjectSnapshot, IdeProjectServiceError>;
  readonly navigate: (
    target: IdeNavigationTarget,
  ) => Effect.Effect<IdeProjectSnapshot, IdeProjectServiceError>;
  readonly advanceGeneration: (
    input: IdeAdvanceGenerationInput,
  ) => Effect.Effect<IdeProjectSnapshot, IdeProjectServiceError>;
  readonly stop: (reason: string) => Effect.Effect<IdeProjectSnapshot, IdeProjectStopped>;
}

export class IdeProjectService extends Context.Service<IdeProjectService, IdeProjectServiceShape>()(
  "@openagentsinc/openagents-desktop/IdeProjectService",
) {}

const inputError = (operation: string, cause: unknown): IdeProjectInvalidInput =>
  new IdeProjectInvalidInput({ operation, detail: String(cause).slice(0, 800) });

const decodeInput = <S extends Schema.ConstraintDecoder<unknown, never>>(
  operation: string,
  schema: S,
  value: unknown,
): Effect.Effect<S["Type"], IdeProjectInvalidInput> =>
  Schema.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError((cause) => inputError(operation, cause)),
  );

const generationFor = (snapshot: IdeProjectSnapshot, kind: IdeGenerationKind): number => {
  switch (kind) {
    case "attachment":
      return snapshot.generations.attachment;
    case "path_index":
      return snapshot.generations.pathIndex;
    case "language":
      return snapshot.generations.language;
    case "git_snapshot":
      return snapshot.generations.gitSnapshot;
    case "placement":
      return snapshot.generations.placement;
  }
};

const capabilityGenerationFor = (snapshot: IdeProjectSnapshot, kind: IdeCapabilityKind): number => {
  switch (kind) {
    case "path_index":
      return snapshot.generations.pathIndex;
    case "language":
      return snapshot.generations.language;
    case "git":
      return snapshot.generations.gitSnapshot;
    default:
      return snapshot.generations.placement;
  }
};

const capabilityStateGeneration = (capability: IdeCapabilitySnapshot): number | null => {
  switch (capability.state._tag) {
    case "Starting":
    case "Ready":
    case "Degraded":
    case "Failed":
      return capability.state.serviceGeneration;
    case "Unconfigured":
    case "Stopped":
      return null;
  }
};

const stoppedCapability = (
  capability: IdeCapabilitySnapshot,
  reason: string,
  stoppedAt: IdeTimestamp,
): IdeCapabilitySnapshot => ({
  ...capability,
  state: { _tag: "Stopped", reason, stoppedAt },
});

const referencesProject = (
  snapshot: IdeProjectSnapshot,
  value: Readonly<{
    projectRef: string;
    rootRef: string;
    worktreeRef: string;
  }>,
): boolean =>
  value.projectRef === snapshot.identity.projectRef &&
  value.rootRef === snapshot.identity.rootRef &&
  value.worktreeRef === snapshot.identity.worktreeRef;

const documentAt = (
  snapshot: IdeProjectSnapshot,
  documentRef: string,
): IdeDocumentSnapshot | null =>
  snapshot.documents.find((document) => document.identity.documentRef === documentRef) ?? null;

const targetDocumentRef = (target: IdeNavigationTarget): string | null => {
  switch (target._tag) {
    case "File":
    case "Range":
      return target.documentRef;
    case "Symbol":
    case "Diagnostic":
    case "Review":
    case "Proposal":
      return null;
  }
};

const targetDocumentGeneration = (target: IdeNavigationTarget): number | null => {
  switch (target._tag) {
    case "File":
    case "Range":
      return target.documentGeneration;
    case "Symbol":
    case "Diagnostic":
    case "Review":
    case "Proposal":
      return null;
  }
};

const generationCapabilityKinds = (kind: IdeGenerationKind): ReadonlySet<IdeCapabilityKind> => {
  switch (kind) {
    case "attachment":
      return new Set([
        "path_index",
        "document",
        "language",
        "git",
        "terminal",
        "task",
        "debug",
        "agent",
        "projection",
        "storage",
      ]);
    case "path_index":
      return new Set(["path_index"]);
    case "language":
      return new Set(["language"]);
    case "git_snapshot":
      return new Set(["git"]);
    case "placement":
      return new Set([
        "language",
        "git",
        "terminal",
        "task",
        "debug",
        "agent",
        "projection",
        "storage",
      ]);
  }
};

export const makeIdeProjectLayer = (
  seed: IdeProjectSnapshot,
  options: Readonly<{ now?: () => IdeTimestamp }> = {},
): Layer.Layer<IdeProjectService, IdeProjectInvalidInput> =>
  Layer.effect(
    IdeProjectService,
    Effect.gen(function* () {
      const initial = yield* decodeInput("IdeProject.acquire", IdeProjectSnapshotSchema, seed);
      const state = yield* Ref.make(initial);
      const stopped = yield* Ref.make<string | null>(null);
      const now = options.now ?? (() => IdeTimestampSchema.make(new Date().toISOString()));

      const ensureActive = Effect.fn("IdeProject.ensureActive")(function* (operation: string) {
        const reason = yield* Ref.get(stopped);
        if (reason !== null) {
          return yield* Effect.fail(new IdeProjectStopped({ operation, reason }));
        }
      });

      const snapshot = Effect.fn("IdeProject.snapshot")(function* () {
        yield* ensureActive("IdeProject.snapshot");
        return yield* Ref.get(state);
      });

      const upsertDocument = Effect.fn("IdeProject.upsertDocument")(function* (
        raw: IdeDocumentUpsertInput,
      ) {
        yield* ensureActive("IdeProject.upsertDocument");
        const input = yield* decodeInput(
          "IdeProject.upsertDocument",
          IdeDocumentUpsertInputSchema,
          raw,
        );
        return yield* Ref.modify(
          state,
          (
            current,
          ): readonly [
            Effect.Effect<
              IdeProjectSnapshot,
              IdeProjectStaleGeneration | IdeProjectInvariantViolation
            >,
            IdeProjectSnapshot,
          ] => {
            if (input.expectedAttachmentGeneration !== current.generations.attachment) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.upsertDocument",
                    generationKind: "attachment",
                    expected: input.expectedAttachmentGeneration,
                    actual: current.generations.attachment,
                  }),
                ),
                current,
              ];
            }
            if (!referencesProject(current, input.document.identity)) {
              return [
                Effect.fail(
                  new IdeProjectInvariantViolation({
                    operation: "IdeProject.upsertDocument",
                    detail: "document identity does not belong to this project/root/worktree",
                  }),
                ),
                current,
              ];
            }
            if (input.document.attachmentGeneration !== current.generations.attachment) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.upsertDocument",
                    generationKind: "document_attachment",
                    expected: input.document.attachmentGeneration,
                    actual: current.generations.attachment,
                  }),
                ),
                current,
              ];
            }
            const existing = documentAt(current, input.document.identity.documentRef);
            const actual = existing?.documentGeneration ?? 0;
            const expected = input.expectedDocumentGeneration ?? 0;
            if (actual !== expected) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.upsertDocument",
                    generationKind: "document",
                    expected,
                    actual,
                  }),
                ),
                current,
              ];
            }
            const requiredNext = actual + 1;
            if (input.document.documentGeneration !== requiredNext) {
              return [
                Effect.fail(
                  new IdeProjectInvariantViolation({
                    operation: "IdeProject.upsertDocument",
                    detail: `document generation must advance exactly once (${actual} -> ${requiredNext})`,
                  }),
                ),
                current,
              ];
            }
            const conflictingFile = current.documents.find(
              (document) =>
                document.identity.fileRef === input.document.identity.fileRef &&
                document.identity.documentRef !== input.document.identity.documentRef,
            );
            if (conflictingFile !== undefined) {
              return [
                Effect.fail(
                  new IdeProjectInvariantViolation({
                    operation: "IdeProject.upsertDocument",
                    detail:
                      "one file ref cannot resolve to two document refs in one attachment generation",
                  }),
                ),
                current,
              ];
            }
            const documents =
              existing === null
                ? [...current.documents, input.document]
                : current.documents.map((document) =>
                    document.identity.documentRef === input.document.identity.documentRef
                      ? input.document
                      : document,
                  );
            const next = { ...current, documents };
            return [Effect.succeed(next), next];
          },
        ).pipe(Effect.flatten);
      });

      const recordCapability = Effect.fn("IdeProject.recordCapability")(function* (
        raw: IdeCapabilityUpdateInput,
      ) {
        yield* ensureActive("IdeProject.recordCapability");
        const input = yield* decodeInput(
          "IdeProject.recordCapability",
          IdeCapabilityUpdateInputSchema,
          raw,
        );
        return yield* Ref.modify(
          state,
          (
            current,
          ): readonly [
            Effect.Effect<
              IdeProjectSnapshot,
              IdeProjectStaleGeneration | IdeProjectInvariantViolation
            >,
            IdeProjectSnapshot,
          ] => {
            if (input.expectedAttachmentGeneration !== current.generations.attachment) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.recordCapability",
                    generationKind: "attachment",
                    expected: input.expectedAttachmentGeneration,
                    actual: current.generations.attachment,
                  }),
                ),
                current,
              ];
            }
            if (
              input.expectedPlacementGeneration !== current.generations.placement ||
              input.capability.placementGeneration !== current.generations.placement
            ) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.recordCapability",
                    generationKind: "placement",
                    expected: input.expectedPlacementGeneration,
                    actual: current.generations.placement,
                  }),
                ),
                current,
              ];
            }
            if (input.capability.attachmentGeneration !== current.generations.attachment) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.recordCapability",
                    generationKind: "capability_attachment",
                    expected: input.capability.attachmentGeneration,
                    actual: current.generations.attachment,
                  }),
                ),
                current,
              ];
            }
            const incomingGeneration = capabilityStateGeneration(input.capability);
            const projectGeneration = capabilityGenerationFor(current, input.capability.kind);
            if (incomingGeneration !== null && incomingGeneration !== projectGeneration) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.recordCapability",
                    generationKind: input.capability.kind,
                    expected: incomingGeneration,
                    actual: projectGeneration,
                  }),
                ),
                current,
              ];
            }
            const existing = current.capabilities.find(
              (capability) => capability.capabilityRef === input.capability.capabilityRef,
            );
            if (existing !== undefined && existing.kind !== input.capability.kind) {
              return [
                Effect.fail(
                  new IdeProjectInvariantViolation({
                    operation: "IdeProject.recordCapability",
                    detail: "a capability ref cannot change kind",
                  }),
                ),
                current,
              ];
            }
            const capabilities =
              existing === undefined
                ? [...current.capabilities, input.capability]
                : current.capabilities.map((capability) =>
                    capability.capabilityRef === input.capability.capabilityRef
                      ? input.capability
                      : capability,
                  );
            const next = { ...current, capabilities };
            return [Effect.succeed(next), next];
          },
        ).pipe(Effect.flatten);
      });

      const navigate = Effect.fn("IdeProject.navigate")(function* (raw: IdeNavigationTarget) {
        yield* ensureActive("IdeProject.navigate");
        const target = yield* decodeInput("IdeProject.navigate", IdeNavigationTargetSchema, raw);
        return yield* Ref.modify(
          state,
          (
            current,
          ): readonly [
            Effect.Effect<
              IdeProjectSnapshot,
              IdeProjectStaleGeneration | IdeProjectInvariantViolation
            >,
            IdeProjectSnapshot,
          ] => {
            if (!referencesProject(current, target)) {
              return [
                Effect.fail(
                  new IdeProjectInvariantViolation({
                    operation: "IdeProject.navigate",
                    detail: "navigation target does not belong to this project/root/worktree",
                  }),
                ),
                current,
              ];
            }
            if (target.attachmentGeneration !== current.generations.attachment) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.navigate",
                    generationKind: "attachment",
                    expected: target.attachmentGeneration,
                    actual: current.generations.attachment,
                  }),
                ),
                current,
              ];
            }
            if (
              (target._tag === "Symbol" || target._tag === "Diagnostic") &&
              target.languageGeneration !== current.generations.language
            ) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.navigate",
                    generationKind: "language",
                    expected: target.languageGeneration,
                    actual: current.generations.language,
                  }),
                ),
                current,
              ];
            }
            if (
              target._tag === "Review" &&
              target.gitSnapshotGeneration !== current.generations.gitSnapshot
            ) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.navigate",
                    generationKind: "git_snapshot",
                    expected: target.gitSnapshotGeneration,
                    actual: current.generations.gitSnapshot,
                  }),
                ),
                current,
              ];
            }
            const documentRef = targetDocumentRef(target);
            const expectedDocumentGeneration = targetDocumentGeneration(target);
            if (documentRef !== null && expectedDocumentGeneration !== null) {
              const document = documentAt(current, documentRef);
              if (document === null || document.documentGeneration !== expectedDocumentGeneration) {
                return [
                  Effect.fail(
                    new IdeProjectStaleGeneration({
                      operation: "IdeProject.navigate",
                      generationKind: "document",
                      expected: expectedDocumentGeneration,
                      actual: document?.documentGeneration ?? 0,
                    }),
                  ),
                  current,
                ];
              }
              if (
                (target._tag === "File" || target._tag === "Range") &&
                document.identity.fileRef !== target.fileRef
              ) {
                return [
                  Effect.fail(
                    new IdeProjectInvariantViolation({
                      operation: "IdeProject.navigate",
                      detail: "navigation file ref does not match the admitted document identity",
                    }),
                  ),
                  current,
                ];
              }
            }
            if (
              target._tag === "Review" &&
              !current.reviewSources.some((source) => source.reviewRef === target.reviewRef)
            ) {
              return [
                Effect.fail(
                  new IdeProjectInvariantViolation({
                    operation: "IdeProject.navigate",
                    detail: "review navigation requires an admitted review source",
                  }),
                ),
                current,
              ];
            }
            if (
              target._tag === "Proposal" &&
              !current.proposals.some(
                (proposal) =>
                  proposal.proposalRef === target.proposalRef &&
                  proposal.attachmentGeneration === current.generations.attachment,
              )
            ) {
              return [
                Effect.fail(
                  new IdeProjectInvariantViolation({
                    operation: "IdeProject.navigate",
                    detail: "proposal navigation requires an admitted current-generation proposal",
                  }),
                ),
                current,
              ];
            }
            const withoutDuplicate = current.navigation.filter(
              (entry) => entry.navigationRef !== target.navigationRef,
            );
            const navigation = [...withoutDuplicate, target].slice(-200);
            const next = { ...current, navigation };
            return [Effect.succeed(next), next];
          },
        ).pipe(Effect.flatten);
      });

      const advanceGeneration = Effect.fn("IdeProject.advanceGeneration")(function* (
        raw: IdeAdvanceGenerationInput,
      ) {
        yield* ensureActive("IdeProject.advanceGeneration");
        const input = yield* decodeInput(
          "IdeProject.advanceGeneration",
          IdeAdvanceGenerationInputSchema,
          raw,
        );
        return yield* Ref.modify(
          state,
          (
            current,
          ): readonly [
            Effect.Effect<IdeProjectSnapshot, IdeProjectStaleGeneration>,
            IdeProjectSnapshot,
          ] => {
            const actual = generationFor(current, input.kind);
            if (input.expectedCurrent !== actual) {
              return [
                Effect.fail(
                  new IdeProjectStaleGeneration({
                    operation: "IdeProject.advanceGeneration",
                    generationKind: input.kind,
                    expected: input.expectedCurrent,
                    actual,
                  }),
                ),
                current,
              ];
            }
            const stoppedKinds = generationCapabilityKinds(input.kind);
            const stoppedAt = now();
            const capabilities = current.capabilities.map((capability) =>
              stoppedKinds.has(capability.kind)
                ? stoppedCapability(capability, `${input.kind}_generation_advanced`, stoppedAt)
                : capability,
            );
            const nextNumber = actual + 1;
            const generations = (() => {
              switch (input.kind) {
                case "attachment":
                  return {
                    ...current.generations,
                    attachment: IdeAttachmentGenerationSchema.make(nextNumber),
                  };
                case "path_index":
                  return {
                    ...current.generations,
                    pathIndex: IdePathIndexGenerationSchema.make(nextNumber),
                  };
                case "language":
                  return {
                    ...current.generations,
                    language: IdeLanguageGenerationSchema.make(nextNumber),
                  };
                case "git_snapshot":
                  return {
                    ...current.generations,
                    gitSnapshot: IdeGitSnapshotGenerationSchema.make(nextNumber),
                  };
                case "placement":
                  return {
                    ...current.generations,
                    placement: IdePlacementGenerationSchema.make(nextNumber),
                  };
              }
            })();
            const next: IdeProjectSnapshot = {
              ...current,
              generations,
              capabilities,
              ...(input.kind === "attachment"
                ? {
                    documents: [],
                    excerpts: [],
                    proposals: [],
                    navigation: [],
                    reviewSources: [],
                    gitSnapshotRef: null,
                  }
                : {}),
              ...(input.kind === "language"
                ? {
                    navigation: current.navigation.filter(
                      (target) => target._tag !== "Symbol" && target._tag !== "Diagnostic",
                    ),
                  }
                : {}),
              ...(input.kind === "git_snapshot"
                ? {
                    gitSnapshotRef: null,
                    navigation: current.navigation.filter((target) => target._tag !== "Review"),
                    reviewSources: current.reviewSources.filter(
                      (source) =>
                        source._tag !== "GitHeadIndex" &&
                        source._tag !== "GitIndexWorktree" &&
                        source._tag !== "GitHeadWorktree",
                    ),
                  }
                : {}),
            };
            return [Effect.succeed(next), next];
          },
        ).pipe(Effect.flatten);
      });

      const stop = Effect.fn("IdeProject.stop")(function* (reason: string) {
        yield* ensureActive("IdeProject.stop");
        const boundedReason = reason.trim().slice(0, 400) || "stopped";
        const stoppedAt = now();
        const next = yield* Ref.updateAndGet(state, (current) => ({
          ...current,
          capabilities: current.capabilities.map((capability) =>
            stoppedCapability(capability, boundedReason, stoppedAt),
          ),
        }));
        yield* Ref.set(stopped, boundedReason);
        return next;
      });

      yield* Effect.addFinalizer(() =>
        Effect.gen(function* () {
          const reason = yield* Ref.get(stopped);
          if (reason !== null) return;
          const stoppedAt = now();
          yield* Ref.update(state, (current) => ({
            ...current,
            capabilities: current.capabilities.map((capability) =>
              stoppedCapability(capability, "project_scope_closed", stoppedAt),
            ),
          }));
          yield* Ref.set(stopped, "project_scope_closed");
        }),
      );

      return IdeProjectService.of({
        snapshot,
        upsertDocument,
        recordCapability,
        navigate,
        advanceGeneration,
        stop,
      });
    }),
  );
