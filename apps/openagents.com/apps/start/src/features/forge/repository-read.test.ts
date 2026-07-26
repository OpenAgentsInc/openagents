import {
  FORGE_MAX_DIFF_BYTES,
  FORGE_MAX_IMAGE_BYTES,
  FORGE_MAX_TEXT_BYTES,
  ForgeRepositoryReadResult,
  ForgeRepositoryReader,
  enforceForgePublicWebRead,
  type ForgeRepositoryReadRequest,
} from "@/features/forge/repository-read";
import {
  forgeRepositoryReadUrl,
  makeForgeRepositoryReaderLayer,
} from "@/server/forge/repository-read.server";
import { Effect } from "effect";
import { describe, expect, test } from "vitest";

import { forgeProjection } from "./repository-fixture";

const request: ForgeRepositoryReadRequest = {
  owner: "OpenAgentsInc",
  repo: "omega",
  view: "code",
  ref: "refs/heads/main",
  path: "src/index.ts",
};

describe("Forge repository read boundary", () => {
  test("builds only the owned server read endpoint with explicit size bounds", () => {
    const url = new URL(forgeRepositoryReadUrl("https://forge-read.internal", request));

    expect(url.origin).toBe("https://forge-read.internal");
    expect(url.pathname).toBe("/internal/v1/repositories/OpenAgentsInc/omega/web-read");
    expect(url.searchParams.get("max_text_bytes")).toBe(String(FORGE_MAX_TEXT_BYTES));
    expect(url.searchParams.get("max_image_bytes")).toBe(String(FORGE_MAX_IMAGE_BYTES));
    expect(url.searchParams.get("max_diff_bytes")).toBe(String(FORGE_MAX_DIFF_BYTES));
    expect(url.href).not.toMatch(/api\.github|github\.com/);
  });

  test("removes member identity and write capability from anonymous public reads", () => {
    const result = enforceForgePublicWebRead(
      ForgeRepositoryReadResult.cases.loaded.make({
        projection: forgeProjection(),
      }),
      false,
    );

    expect(result._tag).toBe("loaded");
    if (result._tag === "failed") throw new Error("Expected loaded result");
    expect(result.projection.access).toEqual({
      mode: "public_web_read",
      canWrite: false,
    });
    expect(result.projection.repository.maintainers).toEqual([]);
    expect(JSON.stringify(result)).not.toContain("Private member");
  });

  test("removes member fields from public-mode reads when unrelated cookies exist", () => {
    const projection = forgeProjection();
    const result = enforceForgePublicWebRead(
      ForgeRepositoryReadResult.cases.loaded.make({
        projection: {
          ...projection,
          access: {
            mode: "public_web_read",
            canWrite: true,
          },
        },
      }),
      true,
    );

    expect(result._tag).toBe("loaded");
    if (result._tag === "failed") throw new Error("Expected loaded result");
    expect(result.projection.access).toEqual({
      mode: "public_web_read",
      canWrite: false,
    });
    expect(result.projection.repository.maintainers).toEqual([]);
    expect(JSON.stringify(result)).not.toContain("Private member");
  });

  test("fails closed when an anonymous repository does not enable public web read", () => {
    const result = enforceForgePublicWebRead(
      ForgeRepositoryReadResult.cases.loaded.make({
        projection: forgeProjection({
          repository: {
            ...forgeProjection().repository,
            publicWebRead: false,
          },
        }),
      }),
      false,
    );

    expect(result._tag).toBe("failed");
    if (result._tag === "loaded") throw new Error("Expected failed result");
    expect(result.failure._tag).toBe("authentication_required");
  });

  test("forwards membership cookies server-side and decodes the typed projection", async () => {
    const observed: Array<{ url: string; cookie: string | null }> = [];
    const fetchFn = (async (input, init) => {
      const headers = new Headers(init?.headers);
      observed.push({
        url: String(input),
        cookie: headers.get("cookie"),
      });
      return new Response(JSON.stringify(forgeProjection()), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    }) as typeof fetch;

    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const reader = yield* ForgeRepositoryReader;
        return yield* reader.read(request, "oa_session=private");
      }).pipe(
        Effect.provide(makeForgeRepositoryReaderLayer("https://forge-read.internal", fetchFn)),
      ),
    );

    expect(result._tag).toBe("loaded");
    expect(observed).toEqual([
      {
        url: expect.stringContaining("/internal/v1/repositories/"),
        cookie: "oa_session=private",
      },
    ]);
  });

  test("rejects GitHub render paths and cross-origin image sources", async () => {
    const githubProjection = forgeProjection({
      repository: {
        ...forgeProjection().repository,
        canonicalCloneUrl: "https://github.com/OpenAgentsInc/omega.git",
      },
      file: {
        _tag: "image",
        path: "logo.png",
        objectId: "f".repeat(40),
        byteSize: 100,
        mimeType: "image/png",
        sourceUrl: "https://raw.githubusercontent.com/private/logo.png",
      },
    });
    const fetchFn = (async () =>
      new Response(JSON.stringify(githubProjection), {
        status: 200,
        headers: { "content-type": "application/json" },
      })) as typeof fetch;

    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const reader = yield* ForgeRepositoryReader;
        return yield* reader.read(request, undefined);
      }).pipe(
        Effect.provide(makeForgeRepositoryReaderLayer("https://forge-read.internal", fetchFn)),
      ),
    );

    expect(result._tag).toBe("failed");
    if (result._tag === "loaded") throw new Error("Expected failed result");
    expect(result.failure._tag).toBe("malformed_response");
    expect(result.failure.detail).toContain("owned OpenAgents Git");
  });
});
