import bootstrapInput from "@openagentsinc/all-work-contract/bootstrap/v0.2.0" with { type: "json" };
import {
  emptyPlanningAuthorityState,
  filePlanningStateStoreLayer,
  initializeFilePlanningState,
  type PlanningAuthorityState,
  PlanningAuthorityStateSchema,
  PlanningStateStore,
  reconcileGitHubBootstrap,
} from "@openagentsinc/all-work-contract";
import { Effect, Schema as S } from "effect";

const decodeState = (input: unknown) =>
  S.decodeUnknownEffect(PlanningAuthorityStateSchema)(input, {
    onExcessProperty: "error",
  });

const withStore = <A, E>(dataRoot: string, effect: Effect.Effect<A, E, PlanningStateStore>) =>
  effect.pipe(Effect.provide(filePlanningStateStoreLayer(dataRoot)));

const readState = (dataRoot: string) =>
  withStore(
    dataRoot,
    Effect.gen(function* () {
      const store = yield* PlanningStateStore;
      const state = yield* store.load;
      if (state === null) {
        return yield* Effect.fail(new Error("All Work planning state is not initialized"));
      }
      return state;
    }),
  );

export const bootstrapAllWorkPlanningAuthority = Effect.fn(
  "OmegaEffectd.bootstrapAllWorkPlanningAuthority",
)(function* (dataRoot: string) {
  const initial = emptyPlanningAuthorityState("2026-08-03T05:00:00Z");
  yield* initializeFilePlanningState(dataRoot, initial);
  const current = yield* readState(dataRoot);
  const reconciled = yield* reconcileGitHubBootstrap(current.graph, bootstrapInput);
  if (reconciled.receipt.noOp) return reconciled.graph;
  const next: PlanningAuthorityState = yield* decodeState({
    ...current,
    graph: reconciled.graph,
  });
  yield* withStore(
    dataRoot,
    Effect.gen(function* () {
      const store = yield* PlanningStateStore;
      yield* store.save(current.graph.revision, next);
    }),
  );
  return reconciled.graph;
});

export const readAllWorkPlanningGraph = (dataRoot: string) =>
  readState(dataRoot).pipe(Effect.map((state) => state.graph));
