import { env, createExecutionContext, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  authed: false,
}));

vi.mock("@workos/authkit-session", async (importOriginal) => {
  const actual = await importOriginal<any>();

  return {
    ...actual,
    createAuthService: () => ({
      withAuth: async (_request: Request) => {
        if (state.authed) {
          return {
            auth: {
              user: { id: "user-1", email: "user@example.com", firstName: "U", lastName: "S" },
              sessionId: "sess-1",
              accessToken: "token-1",
            },
            refreshedSessionData: undefined,
          };
        }
        return { auth: { user: null }, refreshedSessionData: undefined };
      },
      saveSession: async (_auth: unknown, _sessionData: string) => ({ headers: {} as Record<string, string> }),
    }),
  };
});

const { default: worker } = await import("../../src/effuse-host/worker");

const getRedirectPathname = (response: Response): string => {
  const location = response.headers.get("location");
  if (!location) throw new Error("Expected redirect response to include a Location header");
  return new URL(location, "http://example.com").pathname;
};

const setEnvVar = (key: keyof WorkerEnv, value: string | undefined): (() => void) => {
  const prev = (env as any)[key] as string | undefined;
  if (value === undefined) {
    delete (env as any)[key];
  } else {
    (env as any)[key] = value;
  }
  return () => {
    if (prev === undefined) delete (env as any)[key];
    else (env as any)[key] = prev;
  };
};

describe("apps/web worker real routes (SSR + guards)", () => {
  it("GET /autopilot redirects to / when prelaunch is on (no bypass)", async () => {
    state.authed = false;
    const restore = setEnvVar("VITE_PRELAUNCH", "1");
    const request = new Request("http://example.com/autopilot", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restore();

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /autopilot redirects to /login when prelaunch is off and anon", async () => {
    state.authed = false;
    const restorePrelaunch = setEnvVar("VITE_PRELAUNCH", "0");
    const request = new Request("http://example.com/autopilot", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restorePrelaunch();

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/login");
  });

  it("GET /autopilot returns 200 when prelaunch is off and authed", async () => {
    state.authed = true;
    const restorePrelaunch = setEnvVar("VITE_PRELAUNCH", "0");
    const request = new Request("http://example.com/autopilot", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restorePrelaunch();

    expect(response.status).toBe(200);
    expect(response.headers.get("x-oa-request-id")).toBeTruthy();
    const body = await response.text();
    expect(body).toContain("data-autopilot-shell");
  });

  it("GET /autopilot with valid ?key= bypass skips prelaunch but still requires auth", async () => {
    state.authed = false;
    const restorePrelaunch = setEnvVar("VITE_PRELAUNCH", "1");
    const restoreKey = setEnvVar("PRELAUNCH_BYPASS_KEY", "bypass");
    const request = new Request(`http://example.com/autopilot?key=${encodeURIComponent("bypass")}`, {
      method: "GET",
    });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restoreKey();
    restorePrelaunch();

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/login");
  });

  it("GET /autopilot with valid ?key= bypass returns 200 when authed", async () => {
    state.authed = true;
    const restorePrelaunch = setEnvVar("VITE_PRELAUNCH", "1");
    const restoreKey = setEnvVar("PRELAUNCH_BYPASS_KEY", "bypass");
    const request = new Request(`http://example.com/autopilot?key=${encodeURIComponent("bypass")}`, {
      method: "GET",
    });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restoreKey();
    restorePrelaunch();

    expect(response.status).toBe(200);
    expect(response.headers.get("x-oa-request-id")).toBeTruthy();
    const body = await response.text();
    expect(body).toContain("data-autopilot-shell");
  });

  it("GET /chat/:id redirects to / when prelaunch is on (legacy path blocked)", async () => {
    state.authed = false;
    const restore = setEnvVar("VITE_PRELAUNCH", "1");
    const request = new Request("http://example.com/chat/abc", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restore();

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /tools redirects to / when anon", async () => {
    state.authed = false;
    const request = new Request("http://example.com/tools", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /modules redirects to / when anon", async () => {
    state.authed = false;
    const request = new Request("http://example.com/modules", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /signatures redirects to / when anon", async () => {
    state.authed = false;
    const request = new Request("http://example.com/signatures", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /login redirects to / when prelaunch is on (anon)", async () => {
    state.authed = false;
    const restore = setEnvVar("VITE_PRELAUNCH", "1");
    const request = new Request("http://example.com/login", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restore();

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /login redirects to / when prelaunch is on (authed)", async () => {
    state.authed = true;
    const restore = setEnvVar("VITE_PRELAUNCH", "1");
    const request = new Request("http://example.com/login", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restore();

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /tools redirects to / when prelaunch is on (authed)", async () => {
    state.authed = true;
    const restore = setEnvVar("VITE_PRELAUNCH", "1");
    const request = new Request("http://example.com/tools", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restore();

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /deck is local-only (404 on non-local host, 200 on localhost)", async () => {
    state.authed = false;

    const requestProd = new Request("http://example.com/deck", { method: "GET" });
    const ctxProd = createExecutionContext();
    const responseProd = await worker.fetch(requestProd, env, ctxProd);
    await waitOnExecutionContext(ctxProd);
    expect(responseProd.status).toBe(404);

    const requestLocal = new Request("http://localhost:3000/deck", { method: "GET" });
    const ctxLocal = createExecutionContext();
    const responseLocal = await worker.fetch(requestLocal, env, ctxLocal);
    await waitOnExecutionContext(ctxLocal);
    expect(responseLocal.status).toBe(200);
    const body = await responseLocal.text();
    expect(body).toContain("data-deck-shell");
  });
});
