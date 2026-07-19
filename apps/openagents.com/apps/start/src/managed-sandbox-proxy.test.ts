import { describe, expect, test } from "vite-plus/test";

import { KHALA_SYNC_OWNER_COOKIE, KHALA_SYNC_TOKEN_COOKIE } from "./khala-sync-proxy";
import {
  MANAGED_SANDBOX_WEB_PROXY_PATH,
  routeManagedSandboxProxyRequestWithDeps,
} from "./managed-sandbox-proxy";

const cookie = `${KHALA_SYNC_OWNER_COOKIE}=owner.web.fixture; ${KHALA_SYNC_TOKEN_COOKIE}=secret-token`;

const fakeRequest = (
  url: string,
  init: Readonly<{ method?: string; headers?: Record<string, string>; body?: string }> = {},
): Request => {
  const headers = new Map(
    Object.entries(init.headers ?? {}).map(([key, value]) => [key.toLowerCase(), value]),
  );
  return {
    url,
    method: init.method ?? "GET",
    headers: { get: (name: string) => headers.get(name.toLowerCase()) ?? null },
    text: async () => init.body ?? "",
  } as unknown as Request;
};

describe("authenticated managed-sandbox web proxy", () => {
  test("keeps the bearer token server-side and forwards exact typed command bytes", async () => {
    const body = JSON.stringify({ command: { schema: "fixture" } });
    let upstreamInit: RequestInit | undefined;
    const response = await routeManagedSandboxProxyRequestWithDeps(
      fakeRequest(`https://web.openagents.test${MANAGED_SANDBOX_WEB_PROXY_PATH}`, {
        method: "POST",
        headers: { cookie, "content-type": "application/json" },
        body,
      }),
      {
        upstreamBaseUrl: "https://openagents.com",
        fetch: async (url, init) => {
          expect(url).toBe(`https://openagents.com${MANAGED_SANDBOX_WEB_PROXY_PATH}`);
          upstreamInit = init;
          return Response.json({ state: "applied" });
        },
      },
    );
    expect(response?.status).toBe(200);
    expect(upstreamInit).toMatchObject({
      method: "POST",
      headers: { authorization: "Bearer secret-token" },
      body,
    });
    expect(JSON.stringify(await response?.json())).not.toContain("secret-token");
  });

  test("refuses a browser without the httpOnly session before upstream fetch", async () => {
    let fetched = false;
    const response = await routeManagedSandboxProxyRequestWithDeps(
      new Request(`https://web.openagents.test${MANAGED_SANDBOX_WEB_PROXY_PATH}`),
      {
        upstreamBaseUrl: "https://openagents.com",
        fetch: async () => {
          fetched = true;
          return Response.json({});
        },
      },
    );
    expect(response?.status).toBe(401);
    expect(fetched).toBe(false);
  });

  test("returns a typed retryable outage without leaking upstream details", async () => {
    const response = await routeManagedSandboxProxyRequestWithDeps(
      fakeRequest(`https://web.openagents.test${MANAGED_SANDBOX_WEB_PROXY_PATH}`, {
        headers: { cookie },
      }),
      {
        upstreamBaseUrl: "https://private-control.example",
        fetch: async () => {
          throw new Error("private topology");
        },
      },
    );
    expect(response?.status).toBe(503);
    await expect(response?.json()).resolves.toEqual({
      error: "upstream_unavailable",
      retryable: true,
    });
  });
});
