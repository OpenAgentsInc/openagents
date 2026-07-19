import { Context, Effect, Layer, Ref, Schema } from "effect";

import type {
  DesktopWorkspaceTreeEntry,
  DesktopWorkspaceTreePage,
} from "../workspace-contract.ts";
import { workspaceChangePathRefs } from "../workspace-contract.ts";
import {
  IdePathIndexBadgeUpdateSchema,
  IdePathIndexIdentitySchema,
  IdePathIndexInteractionUpdateSchema,
  IdePathIndexOperationUpdateSchema,
  IdePathIndexReconcileRequestSchema,
  IdePathIndexScanRequestSchema,
  IdePathIndexSnapshotSchema,
  IdePathNodeRefSchema,
  IdePathScanRefSchema,
  IdePierreTreeProjectionSchema,
  type IdeExplorerCommand,
  type IdePathIndexBadgeUpdate,
  type IdePathIndexIdentity,
  type IdePathIndexInteraction,
  type IdePathIndexInteractionUpdate,
  type IdePathIndexOperationUpdate,
  type IdePathIndexProgress,
  type IdePathIndexReconcileRequest,
  type IdePathIndexScanRequest,
  type IdePathIndexSnapshot,
  type IdePathNode,
  type IdePathNodeBadge,
  type IdePathNodeRef,
  type IdePathPendingOperation,
  type IdePierreTreeProjection,
} from "./path-index-contract.ts";

const PathIndexOperationSchema = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(120));

export class IdePathIndexInvalidInput extends Schema.TaggedErrorClass<IdePathIndexInvalidInput>()(
  "IdePathIndex.InvalidInput",
  {
    operation: PathIndexOperationSchema,
    detail: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
  },
) {}

export class IdePathIndexStaleGeneration extends Schema.TaggedErrorClass<IdePathIndexStaleGeneration>()(
  "IdePathIndex.StaleGeneration",
  {
    operation: PathIndexOperationSchema,
    generationKind: Schema.Literals(["project", "root", "worktree", "attachment", "path_index", "badge"]),
    expected: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(224)),
    actual: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(224)),
  },
) {}

export class IdePathIndexGrantRevoked extends Schema.TaggedErrorClass<IdePathIndexGrantRevoked>()(
  "IdePathIndex.GrantRevoked",
  {
    operation: PathIndexOperationSchema,
    grantRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(192)),
  },
) {}

export class IdePathIndexCancelled extends Schema.TaggedErrorClass<IdePathIndexCancelled>()(
  "IdePathIndex.Cancelled",
  {
    operation: PathIndexOperationSchema,
    reason: Schema.Literals(["cancelled", "superseded", "scope_closed"]),
  },
) {}

export class IdePathIndexSourceUnavailable extends Schema.TaggedErrorClass<IdePathIndexSourceUnavailable>()(
  "IdePathIndex.SourceUnavailable",
  {
    operation: PathIndexOperationSchema,
    message: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
  },
) {}

export class IdePathIndexStopped extends Schema.TaggedErrorClass<IdePathIndexStopped>()(
  "IdePathIndex.Stopped",
  {
    operation: PathIndexOperationSchema,
    reason: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(400)),
  },
) {}

export const IdePathIndexServiceErrorSchema = Schema.Union([
  IdePathIndexInvalidInput,
  IdePathIndexStaleGeneration,
  IdePathIndexGrantRevoked,
  IdePathIndexCancelled,
  IdePathIndexSourceUnavailable,
  IdePathIndexStopped,
]).annotate({ identifier: "IdePathIndexServiceError" });
export type IdePathIndexServiceError = typeof IdePathIndexServiceErrorSchema.Type;

export interface IdePathIndexSource {
  readonly grantRef: string;
  readonly readPage: (request: Readonly<{
    directoryRef: string;
    offset: number;
    limit: number;
  }>) => Effect.Effect<DesktopWorkspaceTreePage, IdePathIndexSourceUnavailable>;
}

export interface IdePathIndexServiceShape {
  readonly snapshot: () => Effect.Effect<IdePathIndexSnapshot, IdePathIndexStopped>;
  readonly scan: (request: IdePathIndexScanRequest) => Effect.Effect<IdePathIndexSnapshot, IdePathIndexServiceError>;
  readonly reconcile: (request: IdePathIndexReconcileRequest) => Effect.Effect<IdePathIndexSnapshot, IdePathIndexServiceError>;
  readonly cancel: (reason: "cancelled" | "superseded") => Effect.Effect<IdePathIndexSnapshot, IdePathIndexStopped>;
  readonly interact: (update: IdePathIndexInteractionUpdate) => Effect.Effect<IdePathIndexSnapshot, IdePathIndexServiceError>;
  readonly filterPath: (query: string) => Effect.Effect<IdePathIndexSnapshot, IdePathIndexServiceError>;
  readonly projectSearch: (input: Readonly<{ query: string; mode: "path" | "content"; pathRefs: ReadonlyArray<string>; truncated: boolean }>) => Effect.Effect<IdePathIndexSnapshot, IdePathIndexServiceError>;
  readonly updateBadges: (update: IdePathIndexBadgeUpdate) => Effect.Effect<IdePathIndexSnapshot, IdePathIndexServiceError>;
  readonly updateOperation: (update: IdePathIndexOperationUpdate) => Effect.Effect<IdePathIndexSnapshot, IdePathIndexServiceError>;
  readonly projectPierre: (maximumNodes?: number) => Effect.Effect<IdePierreTreeProjection, IdePathIndexStopped>;
  readonly stop: (reason: string) => Effect.Effect<IdePathIndexSnapshot, IdePathIndexStopped>;
}

export class IdePathIndexService extends Context.Service<IdePathIndexService, IdePathIndexServiceShape>()(
  "@openagentsinc/openagents-desktop/IdePathIndexService",
) {}

const emptyInteraction = (): IdePathIndexInteraction => ({
  expandedNodeRefs: [],
  selectedNodeRef: null,
  focusedNodeRef: null,
  scrollAnchorNodeRef: null,
  revealNodeRef: null,
  stickyAncestorNodeRefs: [],
});

const progressFor = (
  nodes: ReadonlyArray<IdePathNode>,
  sourceEpoch: number,
  discoveredDirectories = nodes.filter((node) => node.kind === "directory").length,
): IdePathIndexProgress => ({
  discoveredDirectories,
  scannedDirectories: nodes.filter(
    (node) => node.kind === "directory" && node.loadState._tag === "Loaded",
  ).length,
  discoveredNodes: nodes.length,
  admittedNodes: nodes.filter((node) => node.policy._tag === "Admitted").length,
  pendingDirectories: nodes.filter(
    (node) => node.kind === "directory" && node.loadState._tag !== "Loaded",
  ).length,
  sourceEpoch,
});

const resourcesFor = (nodes: ReadonlyArray<IdePathNode>) => ({
  nodeCount: nodes.length,
  loadedDirectoryCount: nodes.filter(
    (node) => node.kind === "directory" && node.loadState._tag === "Loaded",
  ).length,
  pendingDirectoryCount: nodes.filter(
    (node) => node.kind === "directory" && node.loadState._tag !== "Loaded",
  ).length,
  sourceSubscriptionCount: 0,
  estimatedBytes: Math.min(
    1_000_000_000,
    nodes.reduce((total, node) => total + 240 + node.pathRef.length * 2 + node.badges.length * 48, 0),
  ),
});

export const emptyIdePathIndexSnapshot = (
  identity: IdePathIndexIdentity,
): IdePathIndexSnapshot => IdePathIndexSnapshotSchema.make({
  schemaVersion: "openagents.desktop.ide-path-index.v1",
  identity,
  state: { _tag: "Empty", sourceEpoch: 0 },
  nodes: [],
  interaction: emptyInteraction(),
  filter: { _tag: "None" },
  resources: resourcesFor([]),
});

const decodeInput = <S extends Schema.ConstraintDecoder<unknown, never>>(
  operation: string,
  schema: S,
  input: unknown,
): Effect.Effect<S["Type"], IdePathIndexInvalidInput> =>
  Schema.decodeUnknownEffect(schema)(input).pipe(
    Effect.mapError((error) => new IdePathIndexInvalidInput({ operation, detail: String(error).slice(0, 400) })),
  );

const identityMismatch = (
  expected: IdePathIndexIdentity,
  actual: IdePathIndexIdentity,
): IdePathIndexStaleGeneration | null => {
  for (const [field, kind] of [
    ["projectRef", "project"],
    ["rootRef", "root"],
    ["worktreeRef", "worktree"],
    ["attachmentRef", "attachment"],
    ["attachmentGeneration", "attachment"],
    ["pathIndexGeneration", "path_index"],
  ] as const) {
    if (expected[field] !== actual[field]) {
      return new IdePathIndexStaleGeneration({
        operation: "IdePathIndex.identity",
        generationKind: kind,
        expected: String(expected[field]),
        actual: String(actual[field]),
      });
    }
  }
  return null;
};

const parentPath = (pathRef: string): string => {
  const slash = pathRef.lastIndexOf("/");
  return slash < 0 ? "" : pathRef.slice(0, slash);
};

const withinPath = (candidate: string, ancestor: string): boolean =>
  candidate === ancestor || (ancestor !== "" && candidate.startsWith(`${ancestor}/`));

const stableNodes = (
  current: ReadonlyArray<IdePathNode>,
  directoryRef: string,
  parentNodeRef: IdePathNodeRef | null,
  entries: ReadonlyArray<DesktopWorkspaceTreeEntry>,
  sourceEpoch: number,
  nextNodeRef: () => IdePathNodeRef,
): ReadonlyArray<IdePathNode> => {
  const incomingPaths = new Set(entries.map((entry) => entry.pathRef));
  const removedDirect = current.filter(
    (node) => node.parentNodeRef === parentNodeRef && !incomingPaths.has(node.pathRef),
  );
  const removedPaths = removedDirect.map((node) => node.pathRef);
  const retained = current.filter(
    (node) => !removedPaths.some((removed) => withinPath(node.pathRef, removed)),
  );
  const byPath = new Map(retained.map((node) => [node.pathRef, node]));
  const replacements = entries.map((entry): IdePathNode => {
    const prior = byPath.get(entry.pathRef);
    return {
      nodeRef: prior?.nodeRef ?? nextNodeRef(),
      parentNodeRef,
      pathRef: entry.pathRef,
      name: entry.name,
      kind: entry.kind,
      revisionRef: entry.revisionRef,
      expandable: entry.expandable,
      sizeBytes: entry.sizeBytes,
      policy: { _tag: "Admitted" },
      loadState: entry.kind === "file"
        ? { _tag: "Leaf" }
        : prior?.loadState._tag === "Loaded"
          ? { ...prior.loadState, sourceEpoch }
          : prior?.loadState ?? { _tag: "Unloaded" },
      badges: prior?.badges ?? [],
      pending: prior?.pending ?? null,
    };
  });
  const replacementRefs = new Set(replacements.map((node) => node.nodeRef));
  return [
    ...retained.filter((node) => node.parentNodeRef !== parentNodeRef || !replacementRefs.has(node.nodeRef)),
    ...replacements,
  ].sort((left, right) => left.pathRef.localeCompare(right.pathRef));
};

const survivingInteraction = (
  interaction: IdePathIndexInteraction,
  nodes: ReadonlyArray<IdePathNode>,
): IdePathIndexInteraction => {
  const refs = new Set(nodes.map((node) => node.nodeRef));
  const present = (nodeRef: IdePathNodeRef | null): IdePathNodeRef | null =>
    nodeRef !== null && refs.has(nodeRef) ? nodeRef : null;
  return {
    expandedNodeRefs: interaction.expandedNodeRefs.filter((nodeRef) => refs.has(nodeRef)),
    selectedNodeRef: present(interaction.selectedNodeRef),
    focusedNodeRef: present(interaction.focusedNodeRef),
    scrollAnchorNodeRef: present(interaction.scrollAnchorNodeRef),
    revealNodeRef: present(interaction.revealNodeRef),
    stickyAncestorNodeRefs: interaction.stickyAncestorNodeRefs.filter((nodeRef) => refs.has(nodeRef)),
  };
};

const badgeLabel = (badge: IdePathNodeBadge): string => {
  switch (badge._tag) {
    case "Git": return `Git ${badge.state}`;
    case "Diagnostic": return `${badge.count} ${badge.severity} diagnostic${badge.count === 1 ? "" : "s"}`;
    case "Conflict": return `Conflict: ${badge.reason.replaceAll("_", " ")}`;
    case "Dirty": return "Unsaved changes";
    case "Unavailable": return `Unavailable: ${badge.reason.replaceAll("_", " ")}`;
  }
};

const pendingForCommand = (
  operationRef: IdePathPendingOperation["operationRef"],
  command: IdeExplorerCommand,
): IdePathPendingOperation | null => {
  switch (command._tag) {
    case "Rename": return { _tag: "Rename", operationRef, destinationPathRef: `${parentPath(command.pathRef)}${parentPath(command.pathRef) === "" ? "" : "/"}${command.name}` };
    case "Move": return { _tag: "Move", operationRef, destinationPathRef: `${command.destinationParentPathRef}${command.destinationParentPathRef === "" ? "" : "/"}${command.pathRef.split("/").at(-1) ?? command.pathRef}` };
    case "Copy": return { _tag: "Copy", operationRef, destinationPathRef: command.destinationParentPathRef };
    case "Duplicate": return { _tag: "Copy", operationRef, destinationPathRef: parentPath(command.pathRef) };
    case "Delete": return { _tag: "Delete", operationRef };
    case "CreateFile":
    case "CreateFolder": return { _tag: "Create", operationRef };
    default: return null;
  }
};

const pendingLabel = (pending: IdePathPendingOperation | null): string | null => {
  if (pending === null) return null;
  switch (pending._tag) {
    case "Rename": return `Renaming to ${pending.destinationPathRef}`;
    case "Move": return `Moving to ${pending.destinationPathRef}`;
    case "Copy": return `Copying to ${pending.destinationPathRef}`;
    case "Delete": return "Delete pending";
    case "Create": return "Create pending";
  }
};

const ancestorsOf = (
  nodeRef: IdePathNodeRef,
  nodes: ReadonlyArray<IdePathNode>,
): ReadonlyArray<IdePathNodeRef> => {
  const byRef = new Map(nodes.map((node) => [node.nodeRef, node]));
  const refs: IdePathNodeRef[] = [];
  let current = byRef.get(nodeRef)?.parentNodeRef ?? null;
  while (current !== null) {
    refs.unshift(current);
    current = byRef.get(current)?.parentNodeRef ?? null;
  }
  return refs;
};

export const projectIdePathIndexToPierre = (
  snapshot: IdePathIndexSnapshot,
  maximumNodes = 50_000,
): IdePierreTreeProjection => {
  const matching = snapshot.filter._tag === "Path"
    ? new Set(snapshot.filter.matchingNodeRefs)
    : snapshot.filter._tag === "ExternalSearch"
      ? new Set(snapshot.nodes.filter((node) => snapshot.filter._tag === "ExternalSearch" && snapshot.filter.resultPathRefs.includes(node.pathRef)).map((node) => node.nodeRef))
      : null;
  const visibleRefs = matching === null
    ? null
    : new Set([...matching].flatMap((nodeRef) => [nodeRef, ...ancestorsOf(nodeRef, snapshot.nodes)]));
  const admitted = snapshot.nodes.filter(
    (node) => node.policy._tag === "Admitted" && (visibleRefs === null || visibleRefs.has(node.nodeRef)),
  );
  const nodes = admitted.slice(0, maximumNodes).map((node) => ({
    nodeRef: node.nodeRef,
    pathRef: node.pathRef,
    kind: node.kind,
    revisionRef: node.revisionRef,
    badgeLabels: node.badges.map(badgeLabel),
    pendingLabel: pendingLabel(node.pending),
  }));
  const present = new Set(nodes.map((node) => node.nodeRef));
  return IdePierreTreeProjectionSchema.make({
    schemaVersion: "openagents.desktop.pierre-tree-projection.v1",
    indexGeneration: snapshot.identity.pathIndexGeneration,
    state: snapshot.state,
    nodes,
    expandedNodeRefs: snapshot.interaction.expandedNodeRefs.filter((nodeRef) => present.has(nodeRef)),
    selectedNodeRef: snapshot.interaction.selectedNodeRef !== null && present.has(snapshot.interaction.selectedNodeRef)
      ? snapshot.interaction.selectedNodeRef
      : null,
    focusedNodeRef: snapshot.interaction.focusedNodeRef !== null && present.has(snapshot.interaction.focusedNodeRef)
      ? snapshot.interaction.focusedNodeRef
      : null,
    scrollAnchorNodeRef: snapshot.interaction.scrollAnchorNodeRef !== null && present.has(snapshot.interaction.scrollAnchorNodeRef)
      ? snapshot.interaction.scrollAnchorNodeRef
      : null,
    stickyAncestorNodeRefs: snapshot.interaction.stickyAncestorNodeRefs.filter((nodeRef) => present.has(nodeRef)),
    truncated: admitted.length > maximumNodes || snapshot.state._tag === "Truncated",
  });
};

export const makeIdePathIndexLayer = (
  seed: IdePathIndexSnapshot,
  source: IdePathIndexSource,
): Layer.Layer<IdePathIndexService, IdePathIndexInvalidInput> =>
  Layer.effect(
    IdePathIndexService,
    Effect.gen(function* () {
      const decodedSeed = yield* decodeInput("IdePathIndex.layer", IdePathIndexSnapshotSchema, seed);
      const state = yield* Ref.make(decodedSeed);
      const stopped = yield* Ref.make<string | null>(null);
      const scanSequence = yield* Ref.make(0);
      const scanCancellation = yield* Ref.make<"cancelled" | "superseded">("superseded");
      const nodeSequence = yield* Ref.make(
        decodedSeed.nodes.reduce((maximum, node) => {
          const suffix = Number(node.nodeRef.split(".").at(-1));
          return Number.isSafeInteger(suffix) ? Math.max(maximum, suffix) : maximum;
        }, 0),
      );

      const ensureActive = (operation: string) => Effect.gen(function* () {
        const reason = yield* Ref.get(stopped);
        if (reason !== null) return yield* Effect.fail(new IdePathIndexStopped({ operation, reason }));
      });

      const ensureIdentity = (operation: string, identity: IdePathIndexIdentity) => Effect.gen(function* () {
        const current = yield* Ref.get(state);
        const mismatch = identityMismatch(current.identity, identity);
        if (mismatch !== null) return yield* Effect.fail(new IdePathIndexStaleGeneration({ ...mismatch, operation }));
      });

      const nextNodeRef = (): Effect.Effect<IdePathNodeRef> => Ref.updateAndGet(nodeSequence, (value) => value + 1).pipe(
        Effect.map((value) => IdePathNodeRefSchema.make(`ide.path-node.${value}`)),
      );

      const readDirectory = Effect.fn("IdePathIndex.readDirectory")(function* (
        directoryRef: string,
        limit: number,
      ) {
        const entries: DesktopWorkspaceTreeEntry[] = [];
        let offset = 0;
        let epoch = 0;
        while (true) {
          const page = yield* source.readPage({ directoryRef, offset, limit });
          if (page.state === "unavailable") {
            return yield* Effect.fail(new IdePathIndexSourceUnavailable({
              operation: "IdePathIndex.readDirectory",
              message: page.message,
            }));
          }
          if (page.grantRef !== source.grantRef) {
            return yield* Effect.fail(new IdePathIndexGrantRevoked({
              operation: "IdePathIndex.readDirectory",
              grantRef: source.grantRef,
            }));
          }
          entries.push(...page.entries);
          epoch = Math.max(epoch, page.cache.epoch);
          if (page.nextOffset === null) return { entries, epoch };
          offset = page.nextOffset;
        }
      });

      const readIndexedDirectory = Effect.fn("IdePathIndex.readIndexedDirectory")(function* (
        directoryRef: string,
        limit: number,
      ) {
        return yield* readDirectory(directoryRef, limit).pipe(
          Effect.catchTag("IdePathIndex.SourceUnavailable", (error) => Effect.gen(function* () {
            const current = yield* Ref.get(state);
            const sourceEpoch = current.state._tag === "Ready" || current.state._tag === "Empty"
              ? current.state.sourceEpoch
              : current.state._tag === "Scanning" || current.state._tag === "Partial" || current.state._tag === "Truncated" || current.state._tag === "Degraded"
                ? current.state.progress.sourceEpoch
                : 0;
            yield* Ref.set(state, {
              ...current,
              state: {
                _tag: "Degraded",
                progress: progressFor(current.nodes, sourceEpoch),
                reason: error.message,
                retry: "rescan",
              },
            });
            return yield* Effect.fail(error);
          })),
          Effect.catchTag("IdePathIndex.GrantRevoked", (error) => Effect.gen(function* () {
            const current = yield* Ref.get(state);
            yield* Ref.set(state, {
              ...current,
              state: {
                _tag: "Unavailable",
                reason: "grant_revoked",
                message: `Workspace grant ${error.grantRef} is no longer valid.`,
              },
            });
            return yield* Effect.fail(error);
          })),
        );
      });

      const replaceDirectory = Effect.fn("IdePathIndex.replaceDirectory")(function* (
        directoryRef: string,
        entries: ReadonlyArray<DesktopWorkspaceTreeEntry>,
        epoch: number,
      ) {
        const current = yield* Ref.get(state);
        const parentNodeRef = directoryRef === ""
          ? null
          : current.nodes.find((node) => node.pathRef === directoryRef && node.kind === "directory")?.nodeRef ?? null;
        if (directoryRef !== "" && parentNodeRef === null) return current;
        const refs = yield* Effect.forEach(entries, () => nextNodeRef());
        let cursor = 0;
        const next = stableNodes(current.nodes, directoryRef, parentNodeRef, entries, epoch, () => refs[cursor++]!);
        const loaded = directoryRef === "" ? next : next.map((node) =>
          node.nodeRef === parentNodeRef
            ? { ...node, loadState: { _tag: "Loaded" as const, sourceEpoch: epoch, complete: true } }
            : node);
        const interaction = survivingInteraction(current.interaction, loaded);
        const updated: IdePathIndexSnapshot = {
          ...current,
          nodes: loaded,
          interaction,
          resources: resourcesFor(loaded),
        };
        yield* Ref.set(state, updated);
        return updated;
      });

      const scan = Effect.fn("IdePathIndex.scan")(function* (raw: IdePathIndexScanRequest) {
        yield* ensureActive("IdePathIndex.scan");
        const request = yield* decodeInput("IdePathIndex.scan", IdePathIndexScanRequestSchema, raw);
        yield* ensureIdentity("IdePathIndex.scan", request.identity);
        yield* Ref.set(scanCancellation, "superseded");
        const sequence = yield* Ref.updateAndGet(scanSequence, (value) => value + 1);
        const ensureCurrentSequence = Effect.fn("IdePathIndex.ensureCurrentSequence")(function* () {
          const activeSequence = yield* Ref.get(scanSequence);
          if (activeSequence !== sequence) {
            return yield* Effect.fail(new IdePathIndexCancelled({
              operation: "IdePathIndex.scan",
              reason: yield* Ref.get(scanCancellation),
            }));
          }
        });
        const prior = yield* Ref.get(state);
        yield* Ref.set(state, {
          ...prior,
          state: {
            _tag: "Scanning",
            scanRef: request.scanRef,
            progress: progressFor(prior.nodes, prior.state._tag === "Ready" || prior.state._tag === "Empty" ? prior.state.sourceEpoch : 0),
            reason: request.reason,
          },
        });
        const queue: string[] = [""];
        const visited = new Set<string>();
        let maximumEpoch = 0;
        let processedSinceYield = 0;
        while (queue.length > 0) {
          yield* ensureCurrentSequence();
          const directoryRef = queue.shift()!;
          if (visited.has(directoryRef)) continue;
          visited.add(directoryRef);
          const page = yield* readIndexedDirectory(directoryRef, request.chunkSize);
          yield* ensureCurrentSequence();
          maximumEpoch = Math.max(maximumEpoch, page.epoch);
          const indexed = yield* replaceDirectory(directoryRef, page.entries, page.epoch);
          yield* ensureCurrentSequence();
          if (indexed.nodes.length >= request.maximumNodes) {
            const truncated = {
              ...indexed,
              state: {
                _tag: "Truncated" as const,
                scanRef: request.scanRef,
                progress: progressFor(indexed.nodes, maximumEpoch),
                limit: request.maximumNodes,
              },
              resources: resourcesFor(indexed.nodes.slice(0, request.maximumNodes)),
            };
            yield* Ref.set(state, truncated);
            return truncated;
          }
          const expanded = new Set(indexed.interaction.expandedNodeRefs);
          for (const entry of page.entries) {
            if (entry.kind !== "directory") continue;
            const node = indexed.nodes.find((candidate) => candidate.pathRef === entry.pathRef);
            if (request.mode === "complete" || (node !== undefined && expanded.has(node.nodeRef))) {
              queue.push(entry.pathRef);
            }
          }
          processedSinceYield += page.entries.length;
          const current = yield* Ref.get(state);
          yield* Ref.set(state, {
            ...current,
            state: {
              _tag: "Scanning",
              scanRef: request.scanRef,
              progress: progressFor(current.nodes, maximumEpoch, visited.size + queue.length),
              reason: request.reason,
            },
          });
          if (processedSinceYield >= request.chunkSize) {
            processedSinceYield = 0;
            yield* Effect.yieldNow;
          }
        }
        yield* ensureCurrentSequence();
        const current = yield* Ref.get(state);
        const unloaded = current.nodes.some(
          (node) => node.kind === "directory" && node.loadState._tag !== "Loaded",
        );
        const next: IdePathIndexSnapshot = {
          ...current,
          state: current.nodes.length === 0
            ? { _tag: "Empty", sourceEpoch: maximumEpoch }
            : unloaded
              ? {
                  _tag: "Partial",
                  scanRef: request.scanRef,
                  progress: progressFor(current.nodes, maximumEpoch),
                  reason: "lazy_directories",
                }
              : { _tag: "Ready", sourceEpoch: maximumEpoch, nodeCount: current.nodes.length },
          resources: resourcesFor(current.nodes),
        };
        yield* Ref.set(state, next);
        return next;
      });

      const reconcile = Effect.fn("IdePathIndex.reconcile")(function* (
        raw: IdePathIndexReconcileRequest,
      ) {
        yield* ensureActive("IdePathIndex.reconcile");
        const request = yield* decodeInput(
          "IdePathIndex.reconcile",
          IdePathIndexReconcileRequestSchema,
          raw,
        );
        yield* ensureIdentity("IdePathIndex.reconcile", request.identity);
        const current = yield* Ref.get(state);
        const currentEpoch = current.state._tag === "Ready" || current.state._tag === "Empty"
          ? current.state.sourceEpoch
          : current.state._tag === "Scanning" || current.state._tag === "Partial" || current.state._tag === "Truncated" || current.state._tag === "Degraded"
            ? current.state.progress.sourceEpoch
            : 0;
        if (request.change.epoch <= currentEpoch) return current;
        if (request.change.kind !== "changed" || request.change.epoch > currentEpoch + 1) {
          return yield* scan({
            identity: request.identity,
            scanRef: request.scanRef,
            reason: request.change.kind === "overflow" || request.change.epoch > currentEpoch + 1
              ? "watcher_overflow"
              : "root_refresh",
            mode: "complete",
            chunkSize: 200,
            maximumNodes: 250_000,
          });
        }
        const refs = workspaceChangePathRefs(request.change);
        if (refs === null) {
          return yield* scan({
            identity: request.identity,
            scanRef: request.scanRef,
            reason: "root_refresh",
            mode: "complete",
            chunkSize: 200,
            maximumNodes: 250_000,
          });
        }
        const directories = [...new Set(refs.flatMap((pathRef) => {
          const parent = parentPath(pathRef);
          const node = current.nodes.find((candidate) => candidate.pathRef === pathRef);
          return node?.kind === "directory" ? [parent, pathRef] : [parent];
        }))];
        for (const directoryRef of directories) {
          const latest = yield* Ref.get(state);
          if (directoryRef !== "" && !latest.nodes.some((node) => node.pathRef === directoryRef)) continue;
          const page = yield* readIndexedDirectory(directoryRef, 200);
          yield* replaceDirectory(directoryRef, page.entries, Math.max(page.epoch, request.change.epoch));
        }
        const updated = yield* Ref.get(state);
        const next = {
          ...updated,
          state: updated.nodes.length === 0
            ? { _tag: "Empty" as const, sourceEpoch: request.change.epoch }
            : { _tag: "Ready" as const, sourceEpoch: request.change.epoch, nodeCount: updated.nodes.length },
          resources: resourcesFor(updated.nodes),
        };
        yield* Ref.set(state, next);
        return next;
      });

      const cancel = Effect.fn("IdePathIndex.cancel")(function* (reason: "cancelled" | "superseded") {
        yield* ensureActive("IdePathIndex.cancel");
        yield* Ref.set(scanCancellation, reason);
        yield* Ref.update(scanSequence, (value) => value + 1);
        const current = yield* Ref.get(state);
        const next: IdePathIndexSnapshot = {
          ...current,
          state: {
            _tag: "Partial",
            scanRef: current.state._tag === "Scanning"
              ? current.state.scanRef
              : IdePathScanRefSchema.make(`ide.path-scan.${reason}`),
            progress: progressFor(current.nodes, 0),
            reason,
          },
        };
        yield* Ref.set(state, next);
        return next;
      });

      const interact = Effect.fn("IdePathIndex.interact")(function* (
        raw: IdePathIndexInteractionUpdate,
      ) {
        yield* ensureActive("IdePathIndex.interact");
        const update = yield* decodeInput(
          "IdePathIndex.interact",
          IdePathIndexInteractionUpdateSchema,
          raw,
        );
        const current = yield* Ref.get(state);
        const exists = (nodeRef: IdePathNodeRef | null): boolean =>
          nodeRef === null || current.nodes.some((node) => node.nodeRef === nodeRef);
        const requiredRef = "nodeRef" in update ? update.nodeRef : null;
        if (!exists(requiredRef)) {
          return yield* Effect.fail(new IdePathIndexInvalidInput({
            operation: "IdePathIndex.interact",
            detail: "interaction target is not present in this index generation",
          }));
        }
        const interaction = (() => {
          switch (update._tag) {
            case "Expand": return { ...current.interaction, expandedNodeRefs: [...new Set([...current.interaction.expandedNodeRefs, update.nodeRef])] };
            case "Collapse": return { ...current.interaction, expandedNodeRefs: current.interaction.expandedNodeRefs.filter((nodeRef) => nodeRef !== update.nodeRef) };
            case "Select": return { ...current.interaction, selectedNodeRef: update.nodeRef };
            case "Focus": return { ...current.interaction, focusedNodeRef: update.nodeRef };
            case "ScrollAnchor": return { ...current.interaction, scrollAnchorNodeRef: update.nodeRef };
            case "Reveal": {
              const ancestors = ancestorsOf(update.nodeRef, current.nodes);
              return {
                ...current.interaction,
                expandedNodeRefs: [...new Set([...current.interaction.expandedNodeRefs, ...ancestors])],
                selectedNodeRef: update.nodeRef,
                focusedNodeRef: update.nodeRef,
                scrollAnchorNodeRef: update.nodeRef,
                revealNodeRef: update.nodeRef,
                stickyAncestorNodeRefs: ancestors,
              };
            }
            case "Restore": return survivingInteraction(update.interaction, current.nodes);
          }
        })();
        const next = { ...current, interaction };
        yield* Ref.set(state, next);
        return next;
      });

      const filterPath = Effect.fn("IdePathIndex.filterPath")(function* (rawQuery: string) {
        yield* ensureActive("IdePathIndex.filterPath");
        const query = rawQuery.trim().slice(0, 200);
        const current = yield* Ref.get(state);
        const next = {
          ...current,
          filter: query === ""
            ? { _tag: "None" as const }
            : {
                _tag: "Path" as const,
                query,
                matchingNodeRefs: current.nodes
                  .filter((node) => node.pathRef.toLocaleLowerCase().includes(query.toLocaleLowerCase()))
                  .slice(0, 10_000)
                  .map((node) => node.nodeRef),
                truncated: current.nodes.filter((node) => node.pathRef.toLocaleLowerCase().includes(query.toLocaleLowerCase())).length > 10_000,
              },
        };
        yield* Ref.set(state, next);
        return next;
      });

      const projectSearch = Effect.fn("IdePathIndex.projectSearch")(function* (input: Readonly<{
        query: string;
        mode: "path" | "content";
        pathRefs: ReadonlyArray<string>;
        truncated: boolean;
      }>) {
        yield* ensureActive("IdePathIndex.projectSearch");
        const query = input.query.trim().slice(0, 200);
        if (query === "") return yield* filterPath("");
        const current = yield* Ref.get(state);
        const next: IdePathIndexSnapshot = {
          ...current,
          filter: {
            _tag: "ExternalSearch",
            query,
            mode: input.mode,
            resultPathRefs: [...new Set(input.pathRefs)].slice(0, 10_000),
            truncated: input.truncated || input.pathRefs.length > 10_000,
          },
        };
        yield* Ref.set(state, next);
        return next;
      });

      const updateBadges = Effect.fn("IdePathIndex.updateBadges")(function* (
        raw: IdePathIndexBadgeUpdate,
      ) {
        yield* ensureActive("IdePathIndex.updateBadges");
        const update = yield* decodeInput(
          "IdePathIndex.updateBadges",
          IdePathIndexBadgeUpdateSchema,
          raw,
        );
        yield* ensureIdentity("IdePathIndex.updateBadges", update.identity);
        for (const row of update.updates) {
          for (const badge of row.badges) {
            if ((badge._tag === "Git" && badge.gitSnapshotGeneration !== update.gitSnapshotGeneration) ||
                (badge._tag === "Diagnostic" && badge.languageGeneration !== update.languageGeneration) ||
                (badge._tag === "Unavailable" && badge.attachmentGeneration !== update.identity.attachmentGeneration)) {
              return yield* Effect.fail(new IdePathIndexStaleGeneration({
                operation: "IdePathIndex.updateBadges",
                generationKind: "badge",
                expected: `${update.gitSnapshotGeneration}:${update.languageGeneration}:${update.identity.attachmentGeneration}`,
                actual: badge._tag,
              }));
            }
          }
        }
        const current = yield* Ref.get(state);
        const updates = new Map(update.updates.map((row) => [row.nodeRef, row.badges]));
        const nodes = current.nodes.map((node) => updates.has(node.nodeRef)
          ? { ...node, badges: updates.get(node.nodeRef)! }
          : node);
        const next = { ...current, nodes, resources: resourcesFor(nodes) };
        yield* Ref.set(state, next);
        return next;
      });

      const updateOperation = Effect.fn("IdePathIndex.updateOperation")(function* (
        raw: IdePathIndexOperationUpdate,
      ) {
        yield* ensureActive("IdePathIndex.updateOperation");
        const update = yield* decodeInput(
          "IdePathIndex.updateOperation",
          IdePathIndexOperationUpdateSchema,
          raw,
        );
        yield* ensureIdentity("IdePathIndex.updateOperation", update.identity);
        const current = yield* Ref.get(state);
        if (update._tag === "Pending") {
          const pending = pendingForCommand(update.operationRef, update.command);
          const nodeRef = "nodeRef" in update.command ? update.command.nodeRef : null;
          const nodes = pending === null || nodeRef === null
            ? current.nodes
            : current.nodes.map((node) => node.nodeRef === nodeRef ? { ...node, pending } : node);
          const next = { ...current, nodes, resources: resourcesFor(nodes) };
          yield* Ref.set(state, next);
          return next;
        }
        const pendingNode = current.nodes.find((node) => node.pending?.operationRef === update.operationRef);
        if (update._tag === "Refused") {
          const nodes = current.nodes.map((node) => node.pending?.operationRef === update.operationRef
            ? { ...node, pending: null }
            : node);
          const next: IdePathIndexSnapshot = {
            ...current,
            nodes,
            state: {
              _tag: "Degraded",
              progress: progressFor(nodes, 0),
              reason: update.message,
              retry: update.reason === "grant_revoked" ? "choose_root" : "refresh",
            },
            resources: resourcesFor(nodes),
          };
          yield* Ref.set(state, next);
          return next;
        }
        if (pendingNode === undefined && update.sourceNodeRef !== null) {
          return yield* Effect.fail(new IdePathIndexInvalidInput({
            operation: "IdePathIndex.updateOperation",
            detail: "confirmed operation has no matching pending expected-version intent",
          }));
        }
        let nodes = current.nodes;
        if (pendingNode !== undefined && update.entry === null) {
          nodes = nodes.filter((node) => !withinPath(node.pathRef, pendingNode.pathRef));
        } else if (pendingNode?.pending?._tag === "Copy" && update.entry !== null) {
          const nodeRef = yield* nextNodeRef();
          const parentNodeRef = nodes.find((node) => node.pathRef === parentPath(update.entry!.pathRef))?.nodeRef ?? null;
          nodes = [
            ...nodes.map((node) => node.nodeRef === pendingNode.nodeRef ? { ...node, pending: null } : node),
            {
              nodeRef,
              parentNodeRef,
              pathRef: update.entry.pathRef,
              name: update.entry.name,
              kind: update.entry.kind,
              revisionRef: update.entry.revisionRef,
              expandable: update.entry.expandable,
              sizeBytes: update.entry.sizeBytes,
              policy: { _tag: "Admitted" },
              loadState: update.entry.kind === "directory" ? { _tag: "Unloaded" } : { _tag: "Leaf" },
              badges: [],
              pending: null,
            },
          ];
        } else if (pendingNode !== undefined && update.entry !== null) {
          const oldPath = pendingNode.pathRef;
          const newPath = update.entry.pathRef;
          nodes = nodes.map((node) => withinPath(node.pathRef, oldPath)
            ? {
                ...node,
                pathRef: node.pathRef === oldPath ? newPath : `${newPath}${node.pathRef.slice(oldPath.length)}`,
                name: node.nodeRef === pendingNode.nodeRef ? update.entry!.name : node.name,
                revisionRef: node.nodeRef === pendingNode.nodeRef ? update.entry!.revisionRef : node.revisionRef,
                parentNodeRef: node.nodeRef === pendingNode.nodeRef
                  ? nodes.find((candidate) => candidate.pathRef === parentPath(newPath))?.nodeRef ?? null
                  : node.parentNodeRef,
                pending: null,
              }
            : node);
        } else if (update.entry !== null) {
          const parentNodeRef = nodes.find((node) => node.pathRef === parentPath(update.entry!.pathRef))?.nodeRef ?? null;
          const nodeRef = yield* nextNodeRef();
          nodes = [...nodes, {
            nodeRef,
            parentNodeRef,
            pathRef: update.entry.pathRef,
            name: update.entry.name,
            kind: update.entry.kind,
            revisionRef: update.entry.revisionRef,
            expandable: update.entry.expandable,
            sizeBytes: update.entry.sizeBytes,
            policy: { _tag: "Admitted" },
            loadState: update.entry.kind === "directory" ? { _tag: "Unloaded" } : { _tag: "Leaf" },
            badges: [],
            pending: null,
          }];
        }
        const interaction = survivingInteraction(current.interaction, nodes);
        const next = { ...current, nodes, interaction, resources: resourcesFor(nodes) };
        yield* Ref.set(state, next);
        return next;
      });

      const snapshot = Effect.fn("IdePathIndex.snapshot")(function* () {
        yield* ensureActive("IdePathIndex.snapshot");
        return yield* Ref.get(state);
      });

      const projectPierre = Effect.fn("IdePathIndex.projectPierre")(function* (maximumNodes = 50_000) {
        const current = yield* snapshot();
        return projectIdePathIndexToPierre(current, Math.max(1, Math.min(50_000, maximumNodes)));
      });

      const stop = Effect.fn("IdePathIndex.stop")(function* (reason: string) {
        yield* ensureActive("IdePathIndex.stop");
        yield* Ref.update(scanSequence, (value) => value + 1);
        const bounded = reason.trim().slice(0, 400) || "stopped";
        const current = yield* Ref.get(state);
        const next = {
          ...current,
          state: { _tag: "Stopped" as const, reason: bounded },
          nodes: [],
          interaction: emptyInteraction(),
          filter: { _tag: "None" as const },
          resources: resourcesFor([]),
        };
        yield* Ref.set(state, next);
        yield* Ref.set(stopped, bounded);
        return next;
      });

      yield* Effect.addFinalizer(() => Effect.gen(function* () {
        const reason = yield* Ref.get(stopped);
        if (reason !== null) return;
        yield* Ref.update(scanSequence, (value) => value + 1);
        const current = yield* Ref.get(state);
        yield* Ref.set(state, {
          ...current,
          state: { _tag: "Stopped", reason: "path_index_scope_closed" },
          nodes: [],
          interaction: emptyInteraction(),
          filter: { _tag: "None" },
          resources: resourcesFor([]),
        });
        yield* Ref.set(stopped, "path_index_scope_closed");
      }));

      return IdePathIndexService.of({
        snapshot,
        scan,
        reconcile,
        cancel,
        interact,
        filterPath,
        projectSearch,
        updateBadges,
        updateOperation,
        projectPierre,
        stop,
      });
    }),
  );
