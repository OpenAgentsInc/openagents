import { describe, expect, test, vi } from "vitest";

import {
  routeForgeRepositoryAssetRequestWithDeps,
  type ForgeRepositoryAssetProxyDeps,
} from "./forge-repository-asset-proxy";
import { FORGE_MAX_IMAGE_BYTES } from "./features/forge/repository-read";

const assetUrl =
  "https://openagents.test/internal/v1/repositories/OpenAgentsInc/omega/web-read-asset/assets/logo.png?object=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const fakeRequest = (
  url: string,
  init: Readonly<{ method?: string; headers?: Record<string, string> }> = {},
): Request => {
  const headers = new Map(
    Object.entries(init.headers ?? {}).map(([key, value]) => [key.toLowerCase(), value]),
  );
  return {
    url,
    method: init.method ?? "GET",
    headers: { get: (name: string) => headers.get(name.toLowerCase()) ?? null },
    signal: undefined,
  } as unknown as Request;
};

const deps = (fetch: ForgeRepositoryAssetProxyDeps["fetch"]): ForgeRepositoryAssetProxyDeps => ({
  fetch,
  serviceAuthToken: "private-service-token",
  upstreamBaseUrl: "https://forge-git.internal",
});

describe("Forge repository asset proxy", () => {
  test("ignores unrelated requests", async () => {
    const fetch = vi.fn();
    const response = await routeForgeRepositoryAssetRequestWithDeps(
      new Request("https://openagents.test/forge/OpenAgentsInc/omega"),
      deps(fetch),
    );

    expect(response).toBeUndefined();
    expect(fetch).not.toHaveBeenCalled();
  });

  test("forwards only a bounded owned image request with private service auth", async () => {
    let forwardedUrl: string | undefined;
    let forwardedInit: RequestInit | undefined;
    const fetch: ForgeRepositoryAssetProxyDeps["fetch"] = async (input, init) => {
      forwardedUrl = input;
      forwardedInit = init;
      return new Response(new Uint8Array([137, 80, 78, 71]), {
        headers: {
          "content-length": "4",
          "content-type": "image/png",
        },
      });
    };
    const response = await routeForgeRepositoryAssetRequestWithDeps(
      fakeRequest(assetUrl, {
        headers: {
          cookie: "oa_session=member",
        },
      }),
      deps(fetch),
    );

    const parsed = new URL(String(forwardedUrl));
    expect(parsed.origin).toBe("https://forge-git.internal");
    expect(parsed.pathname).toContain("/web-read-asset/assets/logo.png");
    expect(parsed.searchParams.get("object")).toBe("a".repeat(40));
    expect(parsed.searchParams.get("max_image_bytes")).toBe(String(FORGE_MAX_IMAGE_BYTES));
    expect(new Headers(forwardedInit?.headers).get("authorization")).toBe(
      "Bearer private-service-token",
    );
    expect(new Headers(forwardedInit?.headers).get("cookie")).toBe("oa_session=member");
    expect(response?.status).toBe(200);
    expect(response?.headers.get("content-type")).toBe("image/png");
    expect(response?.headers.get("cache-control")).toContain("no-store");
    expect(response?.headers.get("content-security-policy")).toContain("sandbox");
  });

  test("rejects invalid object ids without calling the private service", async () => {
    const fetch = vi.fn();
    const response = await routeForgeRepositoryAssetRequestWithDeps(
      new Request(assetUrl.replace("a".repeat(40), "../private")),
      deps(fetch),
    );

    expect(response?.status).toBe(404);
    expect(fetch).not.toHaveBeenCalled();
  });

  test("refuses non-image and oversized private responses", async () => {
    const nonImage = await routeForgeRepositoryAssetRequestWithDeps(
      new Request(assetUrl),
      deps(async () => new Response("secret text", { headers: { "content-type": "text/plain" } })),
    );
    const oversized = await routeForgeRepositoryAssetRequestWithDeps(
      new Request(assetUrl),
      deps(
        async () =>
          new Response(new Uint8Array([1]), {
            headers: {
              "content-length": String(FORGE_MAX_IMAGE_BYTES + 1),
              "content-type": "image/png",
            },
          }),
      ),
    );

    expect(nonImage?.status).toBe(415);
    expect(oversized?.status).toBe(415);
  });
});
