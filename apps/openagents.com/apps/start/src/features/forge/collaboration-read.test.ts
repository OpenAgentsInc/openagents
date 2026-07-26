import {
  ForgeCollaborationReader,
  type ForgeCollaborationRequest,
} from "@/features/forge/collaboration-read";
import {
  forgeCollaborationReadUrl,
  makeForgeCollaborationReaderLayer,
} from "@/server/forge/collaboration-read.server";
import { Effect } from "effect";
import { describe, expect, test } from "vitest";

import { forgeCollaborationProjection } from "./collaboration-fixture";

const changeRequest: ForgeCollaborationRequest = {
  owner: "OpenAgentsInc",
  repo: "omega",
  view: "change",
  changeRef: "change.forge.1",
};

describe("Forge collaboration read boundary", () => {
  test("uses only the owned collaboration endpoints", () => {
    expect(new URL(forgeCollaborationReadUrl("https://forge.internal", changeRequest)).pathname).toBe(
      "/internal/v1/repositories/OpenAgentsInc/omega/collaboration/changes/change.forge.1",
    );
    expect(
      new URL(
        forgeCollaborationReadUrl("https://forge.internal", {
          owner: "openagents",
          repo: "attention",
          view: "attention",
        }),
      ).pathname,
    ).toBe("/internal/v1/repositories/openagents/attention/collaboration/attention");
    expect(forgeCollaborationReadUrl("https://forge.internal", changeRequest)).not.toMatch(/github/i);
  });

  test("forwards a session only to the owned server and validates the projection", async () => {
    const seen: Array<{ url: string; cookie: string | null }> = [];
    const fetchFn = (async (input, init) => {
      seen.push({ url: String(input), cookie: new Headers(init?.headers).get("cookie") });
      return new Response(JSON.stringify(forgeCollaborationProjection()), { status: 200 });
    }) as typeof fetch;
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const reader = yield* ForgeCollaborationReader;
        return yield* reader.read(changeRequest, "oa_session=private");
      }).pipe(Effect.provide(makeForgeCollaborationReaderLayer("https://forge.internal", fetchFn))),
    );
    expect(result._tag).toBe("loaded");
    expect(seen).toEqual([{ url: expect.stringContaining("/collaboration/changes/"), cookie: "oa_session=private" }]);
  });

  test("fails closed for an invalid owned response", async () => {
    const fetchFn = (async () => new Response(JSON.stringify({ schema: "wrong" }), { status: 200 })) as typeof fetch;
    await expect(
      Effect.runPromise(
        Effect.gen(function* () {
          const reader = yield* ForgeCollaborationReader;
          return yield* reader.read(changeRequest, undefined);
        }).pipe(Effect.provide(makeForgeCollaborationReaderLayer("https://forge.internal", fetchFn))),
      ),
    ).rejects.toMatchObject({ _tag: "ForgeCollaborationTransportError", operation: "ForgeCollaborationReader.decode" });
  });
});
