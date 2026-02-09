import { Effect, Layer, Stream } from "effect";
import { getFunctionName } from "convex/server";
import { describe, expect, it } from "vitest";

import { AutopilotStoreLive, AutopilotStoreService } from "../../src/effect/autopilotStore";
import type { ConvexServiceApi } from "../../src/effect/convex";
import { ConvexService } from "../../src/effect/convex";
import { RequestContextService } from "../../src/effect/requestContext";

describe("apps/web AutopilotStore", () => {
  it("queries blueprint for an owned thread (no anon ensure)", async () => {
    const calls: Array<string> = [];

    const fakeConvex: ConvexServiceApi = {
      query: (ref: any, _args: any) =>
        Effect.sync(() => {
          const name = getFunctionName(ref as any);
          if (name === "autopilot/blueprint:getBlueprint") calls.push("query:getBlueprint");
          else calls.push("query:other");
          return { ok: true, blueprint: { ok: true }, updatedAtMs: 0 } as any;
        }),
      mutation: (ref: any, args: any) =>
        Effect.sync(() => {
          const name = getFunctionName(ref as any);
          calls.push(`mutation:${name}`);
          return { ok: true, threadId: args?.threadId ?? "" } as any;
        }),
      action: () =>
        Effect.die(new Error("ConvexService.action not used in AutopilotStore tests")),
      subscribeQuery: () =>
        Stream.fail(new Error("ConvexService.subscribeQuery not used in AutopilotStore tests")) as any,
    };

    const base = Layer.mergeAll(
      Layer.succeed(ConvexService, fakeConvex),
      Layer.succeed(RequestContextService, { _tag: "Client" } as const),
    );
    const testLayer = Layer.provideMerge(AutopilotStoreLive, base);

    const blueprint = await Effect.gen(function* () {
      const store = yield* AutopilotStoreService;
      return yield* store.getBlueprint({ threadId: "thread-1" });
    }).pipe(Effect.provide(testLayer), Effect.runPromise);

    expect(blueprint).toEqual({ ok: true });
    expect(calls).toEqual(["query:getBlueprint"]);
  });
});
