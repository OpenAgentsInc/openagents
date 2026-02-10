import { env, createExecutionContext, waitOnExecutionContext } from "cloudflare:test";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  authed: false,
  userId: "user-1",
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
              user: { id: state.userId, email: "user@example.com", firstName: "U", lastName: "S" },
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

  it("GET /?key= bypass redirects to /autopilot and mints bypass cookie", async () => {
    state.authed = false;
    const restorePrelaunch = setEnvVar("VITE_PRELAUNCH", "1");
    const restoreKey = setEnvVar("PRELAUNCH_BYPASS_KEY", "bypass");
    const request = new Request(`http://example.com/?key=${encodeURIComponent("bypass")}`, { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restoreKey();
    restorePrelaunch();

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/autopilot");
    expect(response.headers.get("set-cookie") ?? "").toContain("prelaunch_bypass=1");
  });

  it("GET / returns 200 when prelaunch is on and bypass cookie is present (homepage loads)", async () => {
    state.authed = false;
    const restorePrelaunch = setEnvVar("VITE_PRELAUNCH", "1");
    const restoreKey = setEnvVar("PRELAUNCH_BYPASS_KEY", "bypass");
    const request = new Request("http://example.com/", {
      method: "GET",
      headers: { Cookie: "prelaunch_bypass=1" },
    });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restoreKey();
    restorePrelaunch();

    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain("Introducing Autopilot");
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
    // Critical: bypass key must mint the bypass cookie even on redirects,
    // otherwise the /autopilot -> /login chain loses bypass and falls back to "/".
    expect(response.headers.get("set-cookie") ?? "").toContain("prelaunch_bypass=1");
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

  it("GET /login returns 200 when prelaunch is on and bypass cookie is present (anon)", async () => {
    state.authed = false;
    const restore = setEnvVar("VITE_PRELAUNCH", "1");
    const restoreKey = setEnvVar("PRELAUNCH_BYPASS_KEY", "bypass");
    const request = new Request("http://example.com/login", {
      method: "GET",
      headers: { Cookie: "prelaunch_bypass=1" },
    });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);
    restoreKey();
    restore();

    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain("Log in to OpenAgents");
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

  it("GET /dse redirects to / when anon", async () => {
    state.authed = false;
    const request = new Request("http://example.com/dse", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /dse redirects to / when authed but not ops admin", async () => {
    state.authed = true;
    state.userId = "user-1";
    const request = new Request("http://example.com/dse", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/");
  });

  it("GET /dse returns 200 when authed as ops admin", async () => {
    state.authed = true;
    state.userId = "user_dse_admin";
    const request = new Request("http://example.com/dse", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain("DSE Ops Runs");
  });

  it("GET /dse/ops/:runId returns 200 when authed as ops admin", async () => {
    state.authed = true;
    state.userId = "user_dse_admin";
    const request = new Request("http://example.com/dse/ops/test-run", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain("Ops Run Detail");
  });

  it("GET /dse/signature/:id returns 200 when authed as ops admin", async () => {
    state.authed = true;
    state.userId = "user_dse_admin";
    const sig = encodeURIComponent("@openagents/autopilot/blueprint/SelectTool.v1");
    const request = new Request(`http://example.com/dse/signature/${sig}`, { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain("Signature Detail");
  });

  it("GET /dse/compile-report/... returns 200 when authed as ops admin", async () => {
    state.authed = true;
    state.userId = "user_dse_admin";
    const sig = encodeURIComponent("@openagents/autopilot/blueprint/SelectTool.v1");
    const request = new Request(`http://example.com/dse/compile-report/job_hash/dataset_hash/${sig}`, { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain("Compile Report");
  });

  it("GET /dse/eval-report/... returns 200 when authed as ops admin", async () => {
    state.authed = true;
    state.userId = "user_dse_admin";
    const sig = encodeURIComponent("@openagents/autopilot/canary/RecapThread.v1");
    const evalHash = encodeURIComponent("sha256:eval_hash");
    const request = new Request(`http://example.com/dse/eval-report/${evalHash}/${sig}`, { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain("Eval Report");
  });
});
