import { Context, Deferred, Effect, Exit, Fiber, Layer, Schema, Scope } from "effect";
import { describe, expect, test } from "vite-plus/test";

import type { DesktopWorkspaceTreeEntry, DesktopWorkspaceTreePage } from "../workspace-contract.ts";
import {
  IdeDocumentGenerationSchema,
  IdeGitSnapshotGenerationSchema,
  IdeLanguageGenerationSchema,
} from "./project-contract.ts";
import { makeIdeProjectFixture } from "./project-fixture.ts";
import {
  IdePathIndexIdentitySchema,
  IdePathOperationRefSchema,
  IdePathScanRefSchema,
} from "./path-index-contract.ts";
import {
  IdePathIndexService,
  IdePathIndexServiceErrorSchema,
  IdePathIndexSourceUnavailable,
  emptyIdePathIndexSnapshot,
  makeIdePathIndexLayer,
  type IdePathIndexSource,
} from "./path-index-service.ts";

// Behavior oracle: openagents_desktop.ide_complete_path_index.v1

const identityFor = (suffix: string) => {
  const project = makeIdeProjectFixture(suffix);
  return IdePathIndexIdentitySchema.make({
    projectRef: project.identity.projectRef,
    rootRef: project.identity.rootRef,
    worktreeRef: project.identity.worktreeRef,
    attachmentRef: project.identity.attachmentRef,
    attachmentGeneration: project.generations.attachment,
    pathIndexGeneration: project.generations.pathIndex,
  });
};

const entry = (
  pathRef: string,
  kind: "file" | "directory" = "file",
  revisionRef = `revision.${pathRef.replaceAll("/", ".")}`,
): DesktopWorkspaceTreeEntry => ({
  name: pathRef.split("/").at(-1) ?? pathRef,
  pathRef,
  kind,
  expandable: kind === "directory",
  sizeBytes: kind === "file" ? 12 : null,
  revisionRef,
});

const page = (
  grantRef: string,
  directoryRef: string,
  entries: ReadonlyArray<DesktopWorkspaceTreeEntry>,
  epoch = 1,
  nextOffset: number | null = null,
): DesktopWorkspaceTreePage => ({
  state: "available",
  grantRef,
  directoryRef,
  entries,
  nextOffset,
  cache: { key: `workspace.tree.${directoryRef || "root"}`, epoch, freshness: "current" },
});

const fixtureSource = (
  grantRef: string,
  directories: Readonly<Record<string, ReadonlyArray<DesktopWorkspaceTreeEntry>>>,
  calls: string[] = [],
): IdePathIndexSource => ({
  grantRef,
  readPage: ({ directoryRef, offset, limit }) => Effect.sync(() => {
    calls.push(`${directoryRef}:${offset}:${limit}`);
    const all = directories[directoryRef] ?? [];
    const entries = all.slice(offset, offset + limit);
    const nextOffset = offset + entries.length < all.length ? offset + entries.length : null;
    return page(grantRef, directoryRef, entries, 1, nextOffset);
  }),
});

const runIndex = <A, E>(
  suffix: string,
  source: IdePathIndexSource,
  effect: Effect.Effect<A, E, IdePathIndexService>,
) => {
  const identity = identityFor(suffix);
  return Effect.runPromise(effect.pipe(
    Effect.provide(makeIdePathIndexLayer(emptyIdePathIndexSnapshot(identity), source)),
  ));
};

const scanRequest = (suffix: string, mode: "root_and_expanded" | "complete" = "complete") => ({
  identity: identityFor(suffix),
  scanRef: IdePathScanRefSchema.make(`ide.path-scan.${suffix}`),
  reason: "initial" as const,
  mode,
  chunkSize: 16,
  maximumNodes: 250_000,
});

describe("IdePathIndexService", () => {
  test("decodes the complete expected-failure vocabulary", () => {
    const failures = [
      { _tag: "IdePathIndex.InvalidInput", operation: "scan", detail: "invalid request" },
      { _tag: "IdePathIndex.StaleGeneration", operation: "scan", generationKind: "path_index", expected: "2", actual: "1" },
      { _tag: "IdePathIndex.GrantRevoked", operation: "watch", grantRef: "workspace.grant.old" },
      { _tag: "IdePathIndex.Cancelled", operation: "scan", reason: "superseded" },
      { _tag: "IdePathIndex.SourceUnavailable", operation: "scan", message: "offline" },
      { _tag: "IdePathIndex.Stopped", operation: "snapshot", reason: "closed" },
    ];
    expect(failures.every((failure) =>
      Exit.isSuccess(Schema.decodeUnknownExit(IdePathIndexServiceErrorSchema)(failure)))).toBe(true);
  });

  test("chunks a complete repository scan into an honest ready index", async () => {
    const calls: string[] = [];
    const source = fixtureSource("workspace.grant.complete", {
      "": [entry("src", "directory"), entry("README.md")],
      src: [entry("src/domain", "directory"), entry("src/index.ts")],
      "src/domain": [entry("src/domain/model.ts"), entry("src/domain/service.ts")],
    }, calls);
    const result = await runIndex("complete", source, Effect.gen(function* () {
      const index = yield* IdePathIndexService;
      const snapshot = yield* index.scan(scanRequest("complete"));
      const projection = yield* index.projectPierre();
      return { snapshot, projection };
    }));
    expect(result.snapshot.state).toMatchObject({ _tag: "Ready", nodeCount: 6 });
    expect(result.projection.nodes.map((node) => node.pathRef)).toEqual([
      "README.md", "src", "src/domain", "src/domain/model.ts", "src/domain/service.ts", "src/index.ts",
    ]);
    expect(calls).toEqual([":0:16", "src:0:16", "src/domain:0:16"]);
    expect(result.snapshot.resources).toMatchObject({ nodeCount: 6, loadedDirectoryCount: 2, pendingDirectoryCount: 0, sourceSubscriptionCount: 0 });
  });

  test("keeps lazy directories visibly partial instead of pretending they are empty", async () => {
    const source = fixtureSource("workspace.grant.partial", {
      "": [entry("packages", "directory"), entry("README.md")],
      packages: [entry("packages/app", "directory")],
    });
    const result = await runIndex("partial", source, Effect.gen(function* () {
      const index = yield* IdePathIndexService;
      return yield* index.scan(scanRequest("partial", "root_and_expanded"));
    }));
    expect(result.state).toMatchObject({ _tag: "Partial", reason: "lazy_directories" });
    expect(result.nodes.find((node) => node.pathRef === "packages")?.loadState._tag).toBe("Unloaded");
  });

  test("records grant revocation as an explicit unavailable state", async () => {
    const identity = identityFor("revoked");
    const source: IdePathIndexSource = {
      grantRef: "workspace.grant.revoked",
      readPage: ({ directoryRef }) => Effect.succeed(page("workspace.grant.replacement", directoryRef, [])),
    };
    const result = await Effect.runPromise(Effect.gen(function* () {
      const index = yield* IdePathIndexService;
      const failure = yield* index.scan(scanRequest("revoked")).pipe(Effect.flip);
      const snapshot = yield* index.snapshot();
      return { failure, snapshot };
    }).pipe(Effect.provide(makeIdePathIndexLayer(emptyIdePathIndexSnapshot(identity), source))));
    expect(result.failure._tag).toBe("IdePathIndex.GrantRevoked");
    expect(result.snapshot.state).toMatchObject({ _tag: "Unavailable", reason: "grant_revoked" });
  });

  test("fences a blocked scan after explicit cancellation", async () => {
    const identity = identityFor("cancel");
    const result = await Effect.runPromise(Effect.gen(function* () {
      const started = yield* Deferred.make<void>();
      const release = yield* Deferred.make<void>();
      const source: IdePathIndexSource = {
        grantRef: "workspace.grant.cancel",
        readPage: ({ directoryRef }) => Effect.gen(function* () {
          yield* Deferred.succeed(started, undefined);
          yield* Deferred.await(release);
          return page("workspace.grant.cancel", directoryRef, [entry("late.ts")]);
        }),
      };
      const scope = yield* Scope.make();
      const context = yield* Layer.buildWithScope(makeIdePathIndexLayer(
        emptyIdePathIndexSnapshot(identity),
        source,
      ), scope);
      const index = Context.get(context, IdePathIndexService);
      const fiber = yield* Effect.forkIn(index.scan(scanRequest("cancel")).pipe(Effect.exit), scope);
      yield* Deferred.await(started);
      const cancelled = yield* index.cancel("cancelled");
      yield* Deferred.succeed(release, undefined);
      const exit = yield* Fiber.join(fiber);
      const snapshot = yield* index.snapshot();
      yield* Scope.close(scope, Exit.void);
      return { cancelled, exit, snapshot };
    }));
    expect(result.cancelled.state).toMatchObject({ _tag: "Partial", reason: "cancelled" });
    expect(Exit.isFailure(result.exit)).toBe(true);
    if (Exit.isFailure(result.exit)) {
      expect(result.exit.cause.toString()).toContain("IdePathIndex.Cancelled");
    }
    expect(result.snapshot.nodes).toEqual([]);
  });

  test("isolates equal relative paths by exact project/root/worktree/index identity", async () => {
    const leftIdentity = identityFor("left-worktree");
    const rightIdentity = identityFor("right-worktree");
    const source = fixtureSource("workspace.grant.left-worktree", { "": [entry("src/index.ts")] });
    const stale = await Effect.runPromise(Effect.gen(function* () {
      const index = yield* IdePathIndexService;
      return yield* index.scan({ ...scanRequest("right-worktree"), identity: rightIdentity }).pipe(Effect.flip);
    }).pipe(Effect.provide(makeIdePathIndexLayer(emptyIdePathIndexSnapshot(leftIdentity), source))));
    expect(stale).toMatchObject({ _tag: "IdePathIndex.StaleGeneration", generationKind: "project" });
  });

  test("preserves node identity and focus through an expected-version rename", async () => {
    const source = fixtureSource("workspace.grant.rename", {
      "": [entry("src", "directory")],
      src: [entry("src/old.ts", "file", "revision.old")],
    });
    const result = await runIndex("rename", source, Effect.gen(function* () {
      const index = yield* IdePathIndexService;
      const scanned = yield* index.scan(scanRequest("rename"));
      const node = scanned.nodes.find((candidate) => candidate.pathRef === "src/old.ts")!;
      yield* index.interact({ _tag: "Reveal", nodeRef: node.nodeRef });
      const operationRef = IdePathOperationRefSchema.make("ide.path-operation.rename");
      yield* index.updateOperation({
        _tag: "Pending",
        identity: scanned.identity,
        operationRef,
        command: {
          _tag: "Rename",
          nodeRef: node.nodeRef,
          pathRef: node.pathRef,
          expectedRevisionRef: node.revisionRef,
          name: "new.ts",
        },
      });
      return yield* index.updateOperation({
        _tag: "Confirmed",
        identity: scanned.identity,
        operationRef,
        sourceNodeRef: node.nodeRef,
        entry: entry("src/new.ts", "file", "revision.new"),
      });
    }));
    const renamed = result.nodes.find((node) => node.pathRef === "src/new.ts")!;
    expect(renamed.nodeRef).toBe(result.interaction.focusedNodeRef);
    expect(result.interaction.selectedNodeRef).toBe(renamed.nodeRef);
    expect(renamed.pending).toBeNull();
  });

  test("rejects stale badge generations and projects non-color badge labels", async () => {
    const source = fixtureSource("workspace.grant.badges", { "": [entry("index.ts")] });
    const result = await runIndex("badges", source, Effect.gen(function* () {
      const index = yield* IdePathIndexService;
      const scanned = yield* index.scan(scanRequest("badges"));
      const node = scanned.nodes[0]!;
      const stale = yield* index.updateBadges({
        identity: scanned.identity,
        gitSnapshotGeneration: IdeGitSnapshotGenerationSchema.make(2),
        languageGeneration: IdeLanguageGenerationSchema.make(1),
        updates: [{ nodeRef: node.nodeRef, badges: [{ _tag: "Git", gitSnapshotGeneration: IdeGitSnapshotGenerationSchema.make(1), state: "modified" }] }],
      }).pipe(Effect.flip);
      yield* index.updateBadges({
        identity: scanned.identity,
        gitSnapshotGeneration: IdeGitSnapshotGenerationSchema.make(1),
        languageGeneration: IdeLanguageGenerationSchema.make(1),
        updates: [{ nodeRef: node.nodeRef, badges: [
          { _tag: "Git", gitSnapshotGeneration: IdeGitSnapshotGenerationSchema.make(1), state: "modified" },
          { _tag: "Diagnostic", languageGeneration: IdeLanguageGenerationSchema.make(1), severity: "error", count: 2 },
          { _tag: "Dirty", documentGeneration: IdeDocumentGenerationSchema.make(1) },
        ] }],
      });
      return { stale, projection: yield* index.projectPierre() };
    }));
    expect(result.stale._tag).toBe("IdePathIndex.StaleGeneration");
    expect(result.projection.nodes[0]?.badgeLabels).toEqual(["Git modified", "2 error diagnostics", "Unsaved changes"]);
  });

  test("reconciles an epoch step incrementally and rescans watcher gaps/overflow", async () => {
    const calls: string[] = [];
    const directories: Record<string, ReadonlyArray<DesktopWorkspaceTreeEntry>> = {
      "": [entry("src", "directory")],
      src: [entry("src/one.ts")],
    };
    const source = fixtureSource("workspace.grant.watch", directories, calls);
    const result = await runIndex("watch", source, Effect.gen(function* () {
      const index = yield* IdePathIndexService;
      const scanned = yield* index.scan(scanRequest("watch"));
      directories.src = [entry("src/two.ts")];
      const incremental = yield* index.reconcile({
        identity: scanned.identity,
        scanRef: IdePathScanRefSchema.make("ide.path-scan.watch-incremental"),
        change: { kind: "changed", pathRef: "src/two.ts", pathRefs: ["src/two.ts"], epoch: 2 },
      });
      const overflow = yield* index.reconcile({
        identity: scanned.identity,
        scanRef: IdePathScanRefSchema.make("ide.path-scan.watch-overflow"),
        change: { kind: "overflow", pathRef: null, epoch: 4 },
      });
      return { incremental, overflow };
    }));
    expect(result.incremental.nodes.some((node) => node.pathRef === "src/two.ts")).toBe(true);
    expect(result.incremental.nodes.some((node) => node.pathRef === "src/one.ts")).toBe(false);
    expect(result.overflow.state._tag).toBe("Ready");
    expect(calls.filter((call) => call.startsWith(":")).length).toBe(2);
  });

  test("filtering distinguishes indexed absence from an empty repository", async () => {
    const source = fixtureSource("workspace.grant.filter", { "": [entry("src", "directory"), entry("README.md")], src: [entry("src/index.ts")] });
    const result = await runIndex("filter", source, Effect.gen(function* () {
      const index = yield* IdePathIndexService;
      yield* index.scan(scanRequest("filter"));
      const filtered = yield* index.filterPath("missing");
      return { filtered, projection: yield* index.projectPierre() };
    }));
    expect(result.filtered.state._tag).toBe("Ready");
    expect(result.filtered.filter).toMatchObject({ _tag: "Path", matchingNodeRefs: [] });
    expect(result.projection.nodes).toEqual([]);
  });

  test("scope closure cancels scans and releases every indexed node/resource", async () => {
    const identity = identityFor("scope");
    const source: IdePathIndexSource = {
      grantRef: "workspace.grant.scope",
      readPage: () => Effect.fail(new IdePathIndexSourceUnavailable({ operation: "fixture", message: "not reached" })),
    };
    const result = await Effect.runPromise(Effect.gen(function* () {
      const scope = yield* Scope.make();
      const context = yield* Layer.buildWithScope(
        makeIdePathIndexLayer(emptyIdePathIndexSnapshot(identity), source),
        scope,
      );
      const index = Context.get(context, IdePathIndexService);
      yield* Scope.close(scope, Exit.void);
      return yield* index.snapshot().pipe(Effect.flip);
    }));
    expect(result).toMatchObject({ _tag: "IdePathIndex.Stopped", reason: "path_index_scope_closed" });
  });
});
