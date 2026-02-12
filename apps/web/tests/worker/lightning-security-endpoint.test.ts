import { createExecutionContext, env, waitOnExecutionContext } from "cloudflare:test";
import { getFunctionName } from "convex/server";
import { describe, expect, it, vi } from "vitest";

import type { WorkerEnv } from "../../src/effuse-host/env";

declare module "cloudflare:test" {
  interface ProvidedEnv extends WorkerEnv {}
}

const state = vi.hoisted(() => ({
  authed: true,
  userId: "owner-1",
  queryCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  mutationCalls: [] as Array<{ readonly ref: unknown; readonly args: unknown }>,
  actionCalls: 0,
}));

vi.mock("@workos/authkit-session", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@workos/authkit-session")>();
  return {
    ...actual,
    createAuthService: () => ({
      withAuth: async (_request: Request) => {
        if (!state.authed) return { auth: { user: null }, refreshedSessionData: undefined };
        return {
          auth: {
            user: { id: state.userId, email: "owner@example.com", firstName: "Owner", lastName: "One" },
            sessionId: "sess-security-1",
            accessToken: "token-security-1",
          },
          refreshedSessionData: undefined,
        };
      },
      saveSession: async (_auth: unknown, _sessionData: string) => ({ headers: {} as Record<string, string> }),
    }),
  };
});

vi.mock("../../src/effect/convex", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../src/effect/convex")>();
  const { Effect, Layer, Stream } = await import("effect");

  const query = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      state.queryCalls.push({ ref, args });
      const fn = getFunctionName(ref as never);

      if (fn === "lightning/security:getOwnerSecurityState") {
        return {
          ok: true as const,
          ownerId: state.userId,
          global: {
            stateId: "global",
            globalPause: false,
            updatedAtMs: 1_733_000_000_000,
          },
          ownerControl: null,
          gate: {
            allowed: true,
          },
          credentialRoles: [
            {
              role: "gateway_invoice",
              status: "active",
              version: 2,
              fingerprint: "fp_gateway_2",
              updatedAtMs: 1_733_000_000_001,
            },
          ],
        };
      }

      throw new Error(`Unexpected query function: ${fn}`);
    });

  const mutation = (ref: unknown, args: unknown) =>
    Effect.sync(() => {
      state.mutationCalls.push({ ref, args });
      throw new Error("convex.mutation not expected");
    });

  const action = (_ref: unknown, _args: unknown) =>
    Effect.sync(() => {
      state.actionCalls += 1;
      throw new Error("convex.action not expected");
    });

  const subscribeQuery = (_ref: unknown, _args: unknown) => Stream.fail(new Error("convex.subscribeQuery not used"));

  const ConvexServiceLive = Layer.succeed(actual.ConvexService, {
    query,
    mutation,
    action,
    subscribeQuery,
  });

  return { ...actual, ConvexServiceLive };
});

const { default: worker } = await import("../../src/effuse-host/worker");

const makeEnv = (): WorkerEnv =>
  Object.assign(Object.create(env as WorkerEnv), {
    AI: {},
  });

describe("apps/web worker lightning security endpoint", () => {
  it("requires auth for GET /api/lightning/security/state", async () => {
    state.authed = false;
    state.queryCalls.length = 0;

    const req = new Request("http://example.com/api/lightning/security/state", {
      method: "GET",
      headers: {
        "x-oa-request-id": "req-security-auth-1",
      },
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(401);
    const body = (await res.json()) as { ok: boolean; error: string };
    expect(body.ok).toBe(false);
    expect(body.error).toContain("unauthorized");
    expect(state.queryCalls).toHaveLength(0);
  });

  it("returns owner-scoped security state with gate and role status", async () => {
    state.authed = true;
    state.userId = "owner-1";
    state.queryCalls.length = 0;

    const req = new Request("http://example.com/api/lightning/security/state", {
      method: "GET",
      headers: {
        "x-oa-request-id": "req-security-state-1",
      },
    });

    const ctx = createExecutionContext();
    const res = await worker.fetch(req, makeEnv(), ctx);
    await waitOnExecutionContext(ctx);

    expect(res.status).toBe(200);
    const body = (await res.json()) as {
      ok: boolean;
      requestId: string | null;
      security: {
        ok: boolean;
        ownerId: string;
        gate: { allowed: boolean };
        credentialRoles: Array<{ role: string; status: string; version: number }>;
      };
    };

    expect(body.ok).toBe(true);
    expect(body.requestId).toBe("req-security-state-1");
    expect(body.security.ownerId).toBe("owner-1");
    expect(body.security.gate.allowed).toBe(true);
    expect(body.security.credentialRoles[0]).toMatchObject({
      role: "gateway_invoice",
      status: "active",
      version: 2,
    });
  });
});
