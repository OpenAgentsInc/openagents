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

describe("apps/web worker real routes (SSR + guards)", () => {
  it("GET /autopilot works for anon and includes autopilot shell", async () => {
    state.authed = false;
    const request = new Request("http://example.com/autopilot", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    expect(response.headers.get("content-type") ?? "").toContain("text/html");
    const body = await response.text();
    expect(body).toContain("data-autopilot-shell");
  });

  it("GET /chat/:id redirects to /autopilot (legacy path)", async () => {
    state.authed = false;
    const request = new Request("http://example.com/chat/abc", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/autopilot");
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

  it("GET /login renders the login page when anon", async () => {
    state.authed = false;
    const request = new Request("http://example.com/login", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain('id="login-email"');
  });

  it("GET /login redirects to /autopilot when authed", async () => {
    state.authed = true;
    const request = new Request("http://example.com/login", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(302);
    expect(getRedirectPathname(response)).toBe("/autopilot");
  });

  it("GET /tools renders when authed", async () => {
    state.authed = true;
    const request = new Request("http://example.com/tools", { method: "GET" });
    const ctx = createExecutionContext();
    const response = await worker.fetch(request, env, ctx);
    await waitOnExecutionContext(ctx);

    expect(response.status).toBe(200);
    const body = await response.text();
    expect(body).toContain("Tool Contracts");
  });
});
