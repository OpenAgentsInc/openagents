import { describe, expect, test } from "vite-plus/test";
import { Context, Effect, Exit, Layer, Scope } from "effect";

import {
  IdeSourceControlOperationRefSchema,
  IdeSourceControlRecoveryRefSchema,
  type IdeSourceControlCommand,
  type IdeSourceControlSnapshot,
} from "./source-control-contract.ts";
import {
  ideSourceControlFixtureBinding,
  ideSourceControlFixtureSnapshot,
} from "./source-control-fixture.ts";
import {
  IdeSourceControlService,
  makeIdeSourceControlServiceLayer,
  type IdeSourceControlAdapter,
  type IdeSourceControlServiceShape,
} from "./source-control-service.ts";

const actor = { _tag: "Human" as const, actorRef: "owner.fixture" };
let operationSequence = 0;
const operationRef = () => IdeSourceControlOperationRefSchema.make(
  `ide.scm-operation.fixture-${++operationSequence}`,
);

const advancingAdapter = (): IdeSourceControlAdapter => ({
  refresh: () => Effect.succeed(ideSourceControlFixtureSnapshot(2)),
  execute: (_command, current) => Effect.succeed({
    snapshot: ideSourceControlFixtureSnapshot(current.version.repositoryGeneration + 1),
    changedPaths: ["src/a.ts"],
    conflictPaths: [],
    omittedFacts: [],
    recoveryRef: IdeSourceControlRecoveryRefSchema.make("ide.scm-recovery.fixture"),
    observation: null,
  }),
  stop: () => Effect.void,
});

const withService = async <A>(
  adapter: IdeSourceControlAdapter,
  run: (service: IdeSourceControlServiceShape) => Promise<A>,
): Promise<A> => {
  const scope = await Effect.runPromise(Scope.make());
  const context = await Effect.runPromise(Layer.buildWithScope(
    makeIdeSourceControlServiceLayer(ideSourceControlFixtureSnapshot(), adapter, {
      now: () => "2026-07-20T05:41:00.000Z",
    }),
    scope,
  ));
  try {
    return await run(Context.get(context, IdeSourceControlService));
  } finally {
    await Effect.runPromise(Scope.close(scope, Exit.void));
  }
};

const stage = (snapshot: IdeSourceControlSnapshot): IdeSourceControlCommand => ({
  _tag: "Stage",
  operationRef: operationRef(),
  binding: snapshot.binding,
  expected: snapshot.version,
  actor,
  approvalRef: null,
  selection: { _tag: "Paths", paths: ["src/a.ts"] },
});

describe("IDE-12 source-control service", () => {
  test("requires the exact repository version and writes a canonical post-image receipt", async () => {
    await withService(advancingAdapter(), async (service) => {
      const before = await Effect.runPromise(service.snapshot());
      const first = await Effect.runPromise(service.execute(stage(before)));
      expect(first.receipt?.preVersion.statusRef).toBe("status-1");
      expect(first.receipt?.postVersion.statusRef).toBe("status-2");
      expect(first.receipt?.changedPaths).toEqual(["src/a.ts"]);
      expect(first.receipt?.recoveryRef).toBe("ide.scm-recovery.fixture");

      const stale = await Effect.runPromise(service.execute(stage(before)).pipe(Effect.flip));
      expect(stale.failure.code).toBe("stale_version");
      expect(await Effect.runPromise(service.receipts())).toHaveLength(1);
    });
  });

  test("refuses a post-image that does not advance the repository generation", async () => {
    const adapter: IdeSourceControlAdapter = {
      ...advancingAdapter(),
      execute: (_command, current) => Effect.succeed({
        snapshot: current,
        changedPaths: [],
        conflictPaths: [],
        omittedFacts: [],
        recoveryRef: null,
        observation: null,
      }),
    };
    await withService(adapter, async (service) => {
      const current = await Effect.runPromise(service.snapshot());
      const result = await Effect.runPromiseExit(service.execute(stage(current)));
      expect(Exit.isFailure(result)).toBe(true);
      expect(await Effect.runPromise(service.receipts())).toEqual([]);
    });
  });

  test("fences another worktree and stops adapter authority before closing events", async () => {
    let stopped = false;
    const adapter: IdeSourceControlAdapter = {
      ...advancingAdapter(),
      stop: () => Effect.sync(() => { stopped = true; }),
    };
    await withService(adapter, async (service) => {
      const current = await Effect.runPromise(service.snapshot());
      const wrong = stage(current);
      if (wrong._tag !== "Stage") throw new Error("fixture command mismatch");
      const result = await Effect.runPromiseExit(service.execute({
        ...wrong,
        binding: { ...ideSourceControlFixtureBinding(), worktreeRef: "ide.worktree.other" as never },
      }));
      expect(Exit.isFailure(result)).toBe(true);
      const closed = await Effect.runPromise(service.stop("test complete"));
      expect(stopped).toBe(true);
      expect(closed.stopped).toBe(true);
    });
  });
});
