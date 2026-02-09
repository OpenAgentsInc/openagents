import { Context, Effect, Layer, Schema } from "effect";

import { api } from "../../convex/_generated/api";
import { ConvexService } from "./convex";
import type { RequestContextService } from "./requestContext";

export class AutopilotStoreError extends Schema.TaggedError<AutopilotStoreError>()("AutopilotStoreError", {
  operation: Schema.String,
  error: Schema.Defect,
}) {}

export type AutopilotStore = {
  readonly getBlueprint: (input: { readonly threadId: string }) => Effect.Effect<unknown, AutopilotStoreError, RequestContextService>;

  readonly importBlueprint: (input: {
    readonly threadId: string;
    readonly blueprint: unknown;
  }) => Effect.Effect<void, AutopilotStoreError, RequestContextService>;

  readonly resetThread: (input: { readonly threadId: string }) => Effect.Effect<void, AutopilotStoreError, RequestContextService>;
};

export class AutopilotStoreService extends Context.Tag("@openagents/web/AutopilotStore")<
  AutopilotStoreService,
  AutopilotStore
>() {}

export const AutopilotStoreLive = Layer.effect(
  AutopilotStoreService,
  Effect.gen(function* () {
    const convex = yield* ConvexService;

    const getBlueprint = Effect.fn("AutopilotStore.getBlueprint")(function* (input: { readonly threadId: string }) {
      const result = yield* convex
        .query(api.autopilot.blueprint.getBlueprint, { threadId: input.threadId } as any)
        .pipe(Effect.mapError((error) => AutopilotStoreError.make({ operation: "getBlueprint", error })));
      return (result as any)?.blueprint ?? null;
    });

    const importBlueprint = Effect.fn("AutopilotStore.importBlueprint")(function* (input: {
      readonly threadId: string;
      readonly blueprint: unknown;
    }) {
      yield* convex
        .mutation(api.autopilot.blueprint.setBlueprint, { threadId: input.threadId, blueprint: input.blueprint } as any)
        .pipe(Effect.mapError((error) => AutopilotStoreError.make({ operation: "importBlueprint", error })));
    });

    const resetThread = Effect.fn("AutopilotStore.resetThread")(function* (input: { readonly threadId: string }) {
      yield* convex
        .mutation(api.autopilot.reset.resetThread, { threadId: input.threadId } as any)
        .pipe(Effect.mapError((error) => AutopilotStoreError.make({ operation: "resetThread", error })));
    });

    return AutopilotStoreService.of({ getBlueprint, importBlueprint, resetThread });
  }),
);
