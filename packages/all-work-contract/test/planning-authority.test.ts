import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";

import { Effect, Layer, Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  emptyPlanningAuthorityState,
  emptyPlanningGraph,
  filePlanningStateStoreLayer,
  GitHubBootstrapBatchSchema,
  githubWorkRef,
  initializeFilePlanningState,
  inMemoryPlanningStateStoreLayer,
  PlanningAuthority,
  PlanningAuthorityLive,
  PlanningAuthorityStateSchema,
  PlanningStateStore,
  reconcileGitHubBootstrap,
} from "../src/index.ts";
import type { PlanningGraph } from "../src/generated.ts";

const packageRoot = resolve(import.meta.dirname, "..");
const bootstrapInput = S.decodeUnknownSync(GitHubBootstrapBatchSchema)(
  JSON.parse(readFileSync(resolve(packageRoot, "bootstrap/v0.2.0-github-source.json"), "utf8")),
  { onExcessProperty: "error" },
);

const stateWithGraph = (graph: PlanningGraph) =>
  S.decodeUnknownSync(PlanningAuthorityStateSchema)({
    ...emptyPlanningAuthorityState(bootstrapInput.fetchedAt),
    graph,
  });

describe("All Work planning authority", () => {
  it("imports the exact 28 open and six closed dogfood rows once", async () => {
    const first = await Effect.runPromise(
      reconcileGitHubBootstrap(emptyPlanningGraph(bootstrapInput.fetchedAt), bootstrapInput),
    );
    expect(first.graph.work).toHaveLength(34);
    expect(first.graph.work.filter((work) => work.summary.state === "completed")).toHaveLength(6);
    expect(first.graph.work.filter((work) => work.summary.state !== "completed")).toHaveLength(28);
    expect(first.graph.sourceCoordinates).toHaveLength(34);
    expect(first.graph.work.flatMap((work) => work.relations)).toHaveLength(46);
    expect(first.receipt).toMatchObject({
      imported: 34,
      updated: 0,
      unchanged: 0,
      noOp: false,
      githubWriteCount: 0,
    });

    const second = await Effect.runPromise(reconcileGitHubBootstrap(first.graph, bootstrapInput));
    expect(second.graph).toEqual(first.graph);
    expect(second.receipt).toMatchObject({
      imported: 0,
      updated: 0,
      unchanged: 34,
      noOp: true,
      githubWriteCount: 0,
    });
  });

  it("merges reordered duplicate deliveries and paginated comments", async () => {
    const firstPage = bootstrapInput.pages[0];
    const issue = firstPage?.issues[0];
    expect(firstPage).toBeDefined();
    expect(issue).toBeDefined();
    if (firstPage === undefined || issue === undefined) return;
    const comment = {
      id: "issuecomment-1",
      body: "Public-safe status",
      authorRef: "principal:github:owner",
      createdAt: bootstrapInput.fetchedAt,
      sourceRevision: "comment-revision-1",
    };
    const duplicatePage = {
      ...firstPage,
      page: 2,
      cursor: "cursor-page-2",
      issues: [{ ...issue, comments: [comment] }],
    };
    const batch = {
      ...bootstrapInput,
      pages: [...bootstrapInput.pages].reverse().concat(duplicatePage),
    };
    const result = await Effect.runPromise(
      reconcileGitHubBootstrap(emptyPlanningGraph(bootstrapInput.fetchedAt), batch),
    );
    expect(result.graph.work).toHaveLength(34);
    expect(result.graph.textRecords).toHaveLength(1);
    expect(result.receipt.duplicateDeliveries).toBe(1);
  });

  it("keeps last-known-good rows across a gap and records later unavailability", async () => {
    const baseline = await Effect.runPromise(
      reconcileGitHubBootstrap(emptyPlanningGraph(bootstrapInput.fetchedAt), bootstrapInput),
    );
    const omegaPage = bootstrapInput.pages.find(
      (page) => page.repository === "OpenAgentsInc/omega",
    );
    expect(omegaPage).toBeDefined();
    if (omegaPage === undefined) return;
    const missing = omegaPage.issues[0];
    expect(missing).toBeDefined();
    if (missing === undefined) return;
    const withoutOne = {
      ...bootstrapInput,
      pages: bootstrapInput.pages.map((page) =>
        page.repository === omegaPage.repository
          ? {
              ...page,
              complete: false,
              nextCursor: "cursor-missing-page",
              issues: page.issues.slice(1),
            }
          : page,
      ),
    };
    const gap = await Effect.runPromise(reconcileGitHubBootstrap(baseline.graph, withoutOne));
    expect(gap.graph.completeness.state).toBe("gap");
    expect(
      gap.graph.work.some(
        (work) => work.summary.workRef === githubWorkRef(missing.repository, missing.number),
      ),
    ).toBe(true);

    const completeWithoutOne = {
      ...withoutOne,
      pages: withoutOne.pages.map((page) =>
        page.repository === omegaPage.repository
          ? { ...page, complete: true, nextCursor: null }
          : page,
      ),
    };
    const unavailable = await Effect.runPromise(
      reconcileGitHubBootstrap(gap.graph, completeWithoutOne),
    );
    expect(unavailable.receipt.unavailable).toBe(1);
    expect(
      unavailable.graph.sourceCoordinates.find(
        (coordinate) => coordinate.workRef === githubWorkRef(missing.repository, missing.number),
      )?.available,
    ).toBe(false);
  });

  it("creates and updates native Work with idempotent receipts and zero GitHub writes", async () => {
    const imported = await Effect.runPromise(
      reconcileGitHubBootstrap(emptyPlanningGraph(bootstrapInput.fetchedAt), bootstrapInput),
    );
    const layer = PlanningAuthorityLive.pipe(
      Layer.provide(inMemoryPlanningStateStoreLayer(stateWithGraph(imported.graph))),
    );
    const create = {
      command: "create_work",
      commandRef: "command:native:create:1",
      idempotencyKey: "native-create-work-1",
      expectedRevision: imported.graph.revision,
      workRef: "work:native:dogfood:1",
      identifier: "OA-1",
      title: "Native dogfood Work",
      priority: "high",
      ownerRef: "principal:owner:1",
      projectRef: "project:omega-v0.2.0-dogfood",
      projectMilestoneRef: null,
      cycleRef: "cycle:v0.2.0-dogfood",
      workflowStateRef: "workflow:ready",
      occurredAt: "2026-08-03T06:30:00Z",
    };
    const journey = Effect.gen(function* () {
      const authority = yield* PlanningAuthority;
      const created = yield* authority.execute(create);
      const replayed = yield* authority.execute(create);
      const updated = yield* authority.execute({
        command: "update_work",
        commandRef: "command:native:update:1",
        idempotencyKey: "native-update-work-1",
        expectedRevision: created.revision,
        workRef: create.workRef,
        title: "Native dogfood Work updated",
        state: "active",
        occurredAt: "2026-08-03T06:31:00Z",
      });
      const graph = yield* authority.readGraph;
      return { created, replayed, updated, graph };
    }).pipe(Effect.provide(layer));
    const result = await Effect.runPromise(journey);
    expect(result.replayed).toEqual(result.created);
    expect(result.created.githubWriteCount).toBe(0);
    expect(result.updated.githubWriteCount).toBe(0);
    expect(
      result.graph.work.find((work) => work.summary.workRef === create.workRef)?.summary,
    ).toMatchObject({ title: "Native dogfood Work updated", state: "active" });
  });

  it("restarts from the durable revision, cursor, idempotency receipt, and graph", async () => {
    const directory = mkdtempSync(resolve(tmpdir(), "all-work-planning-authority-"));
    const imported = await Effect.runPromise(
      reconcileGitHubBootstrap(emptyPlanningGraph(bootstrapInput.fetchedAt), bootstrapInput),
    );
    const state = stateWithGraph(imported.graph);
    await Effect.runPromise(initializeFilePlanningState(directory, state));
    const loaded = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* PlanningStateStore;
        return yield* store.load;
      }).pipe(Effect.provide(filePlanningStateStoreLayer(directory))),
    );
    expect(loaded).toEqual(state);
    rmSync(directory, { recursive: true, force: true });
  });
});
