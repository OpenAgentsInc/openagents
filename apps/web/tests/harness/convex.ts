import { getFunctionName } from "convex/server";
import { Effect, Layer, Option, Stream } from "effect";
import * as PubSub from "effect/PubSub";

import type { FunctionArgs, FunctionReference, FunctionReturnType } from "convex/server";

import type { ConvexServiceApi } from "../../src/effect/convex";
import { ConvexService, ConvexServiceError } from "../../src/effect/convex";

import { makeInMemoryDb } from "../convex/inMemoryDb";

import {
  claimAnonThreadImpl,
  ensureAnonThreadImpl,
  ensureOwnedThreadImpl,
} from "../../convex/autopilot/threads";
import { getBlueprintImpl, resetBlueprintImpl, setBlueprintImpl } from "../../convex/autopilot/blueprint";
import {
  appendPartsImpl,
  clearMessagesImpl,
  createRunImpl,
  finalizeRunImpl,
  getThreadSnapshotImpl,
  isCancelRequestedImpl,
  requestCancelImpl,
} from "../../convex/autopilot/messages";
import { resetThreadImpl } from "../../convex/autopilot/reset";

import { getArtifactImpl, listArtifactsImpl, putArtifactImpl } from "../../convex/dse/artifacts";
import { clearActiveImpl, getActiveImpl, rollbackActiveImpl, setActiveImpl } from "../../convex/dse/active";
import { recordPredictReceiptImpl } from "../../convex/dse/receipts";
import {
  createTaskImpl as createLightningTaskImpl,
  getTaskImpl as getLightningTaskImpl,
  listTaskEventsImpl as listLightningTaskEventsImpl,
  listTasksImpl as listLightningTasksImpl,
  transitionTaskImpl as transitionLightningTaskImpl,
} from "../../convex/lightning/tasks";

type UserIdentity = { readonly subject: string };

type TestAuth = {
  user: UserIdentity | null;
};

type TestCtx = {
  readonly db: ReturnType<typeof makeInMemoryDb>;
  readonly auth: { readonly getUserIdentity: () => Effect.Effect<Option.Option<UserIdentity>> };
};

type QueryHandler = (ctx: TestCtx, args: any) => Effect.Effect<any>;
type MutationHandler = (ctx: TestCtx, args: any) => Effect.Effect<any>;

const makeAuth = (auth: TestAuth): TestCtx["auth"] => ({
  getUserIdentity: () => Effect.succeed(auth.user ? Option.some(auth.user) : Option.none()),
});

const queryHandlers: Record<string, QueryHandler> = {
  "autopilot/blueprint:getBlueprint": getBlueprintImpl as any,
  "autopilot/messages:getThreadSnapshot": getThreadSnapshotImpl as any,
  "autopilot/messages:isCancelRequested": isCancelRequestedImpl as any,
  "dse/artifacts:getArtifact": getArtifactImpl as any,
  "dse/artifacts:listArtifacts": listArtifactsImpl as any,
  "dse/active:getActive": getActiveImpl as any,
  "lightning/tasks:getTask": getLightningTaskImpl as any,
  "lightning/tasks:listTasks": listLightningTasksImpl as any,
  "lightning/tasks:listTaskEvents": listLightningTaskEventsImpl as any,
};

const mutationHandlers: Record<string, MutationHandler> = {
  "autopilot/threads:ensureAnonThread": ensureAnonThreadImpl as any,
  "autopilot/threads:ensureOwnedThread": ensureOwnedThreadImpl as any,
  "autopilot/threads:claimAnonThread": claimAnonThreadImpl as any,
  "autopilot/blueprint:setBlueprint": setBlueprintImpl as any,
  "autopilot/blueprint:resetBlueprint": resetBlueprintImpl as any,
  "autopilot/messages:createRun": createRunImpl as any,
  "autopilot/messages:appendParts": appendPartsImpl as any,
  "autopilot/messages:finalizeRun": finalizeRunImpl as any,
  "autopilot/messages:requestCancel": requestCancelImpl as any,
  "autopilot/messages:clearMessages": clearMessagesImpl as any,
  "autopilot/reset:resetThread": resetThreadImpl as any,
  "dse/artifacts:putArtifact": putArtifactImpl as any,
  "dse/active:setActive": setActiveImpl as any,
  "dse/active:clearActive": clearActiveImpl as any,
  "dse/active:rollbackActive": rollbackActiveImpl as any,
  "dse/receipts:recordPredictReceipt": recordPredictReceiptImpl as any,
  "lightning/tasks:createTask": createLightningTaskImpl as any,
  "lightning/tasks:transitionTask": transitionLightningTaskImpl as any,
};

const notImplemented = (kind: string, name: string): ConvexServiceError =>
  ConvexServiceError.make({
    operation: `${kind}.mock`,
    error: new Error(`TestConvexService: ${kind} not implemented: ${name}`),
  });

export type TestConvexKit = {
  readonly db: ReturnType<typeof makeInMemoryDb>;
  readonly setUser: (subject: string | null) => void;
  readonly reset: () => void;
  readonly layer: Layer.Layer<ConvexService, never, never>;
  readonly service: ConvexServiceApi;
};

/**
 * High-fidelity Convex mock for tests:
 * - runs our real Convex function implementations in-process against an in-memory db
 * - supports subscribeQuery via invalidation broadcasts on every mutation
 *
 * Scope: feature parity for our app's Convex surface (Autopilot MVP tables + functions),
 * not a full Convex emulator.
 */
export const makeTestConvexKit = (): TestConvexKit => {
  const db = makeInMemoryDb();
  const auth: TestAuth = { user: null };
  const pubsub = Effect.runSync(PubSub.unbounded<void>());
  // Convex functions are transactional; enforce serial execution in the mock so
  // queries cannot observe mid-mutation partial state.
  const mutex = Effect.runSync(Effect.makeSemaphore(1));

  const makeCtx = (): TestCtx => ({ db, auth: makeAuth(auth) });

  const runQueryHandler = (name: string, args: any) => {
    const handler = queryHandlers[name];
    if (!handler) return Effect.fail(notImplemented("query", name));
    return mutex.withPermits(1)(
      handler(makeCtx(), args).pipe(
        Effect.mapError((error) => ConvexServiceError.make({ operation: `query.${name}`, error })),
      ),
    );
  };

  const runMutationHandler = (name: string, args: any) => {
    const handler = mutationHandlers[name];
    if (!handler) return Effect.fail(notImplemented("mutation", name));
    return mutex.withPermits(1)(
      handler(makeCtx(), args).pipe(
        Effect.mapError((error) => ConvexServiceError.make({ operation: `mutation.${name}`, error })),
        Effect.tap(() => PubSub.publish(pubsub, void 0)),
      ),
    );
  };

  const service: ConvexServiceApi = {
    query: <TQuery extends FunctionReference<"query">>(ref: TQuery, args: FunctionArgs<TQuery>) =>
      Effect.suspend(() => runQueryHandler(getFunctionName(ref as any), args as any)) as any,

    mutation: <TMutation extends FunctionReference<"mutation">>(ref: TMutation, args: FunctionArgs<TMutation>) =>
      Effect.suspend(() => runMutationHandler(getFunctionName(ref as any), args as any)) as any,

    action: <TAction extends FunctionReference<"action">>(ref: TAction, _args: FunctionArgs<TAction>) =>
      Effect.fail(notImplemented("action", getFunctionName(ref as any))) as any,

    subscribeQuery: <TQuery extends FunctionReference<"query">>(ref: TQuery, args: FunctionArgs<TQuery>) => {
      const name = getFunctionName(ref as any);
      const query = runQueryHandler(name, args as any) as Effect.Effect<
        Awaited<FunctionReturnType<TQuery>>,
        ConvexServiceError
      >;

      // Subscribe before we evaluate `query` so we cannot miss invalidations between
      // the initial read and the subscription starting.
      return Stream.unwrapScoped(
        Effect.gen(function* () {
          const q = yield* PubSub.subscribe(pubsub);
          const updates = Stream.fromQueue(q).pipe(
            // Re-run on every invalidation. We don't attempt fine-grained dependency tracking in v1.
            Stream.mapEffect(() => query),
          );
          return Stream.concat(Stream.fromEffect(query), updates);
        }),
      );
    },
  };

  return {
    db,
    setUser: (subject) => {
      auth.user = subject ? { subject } : null;
    },
    reset: () => {
      db.reset();
    },
    layer: Layer.succeed(ConvexService, service),
    service,
  };
};
