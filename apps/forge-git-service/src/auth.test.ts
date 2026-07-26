import { Effect, Redacted } from "effect";
import { Buffer } from "node:buffer";
import { describe, expect, test, vi } from "vitest";

import { ForgeGitAuth, makePolicyAuthorityAuthLayer, readForgeGitToken } from "./auth.js";
import { makeTestConfiguration } from "./config.js";

const token = "oa_forge_git_fixture_00000000000000000000";
const configuration = makeTestConfiguration({
  gitBinary: "git",
  maxReceivePackBytes: 1024,
  mirrorEnabled: false,
  policyAuthorityToken: Redacted.make("service-authority-secret"),
  policyAuthorityUrl: "https://openagents.test/internal/forge/git-authorize",
  repositoryRoot: "/tmp/forge",
});

const authenticate = (
  policyFetch: typeof fetch,
  authorization: string | null = `Bearer ${token}`,
) =>
  Effect.runPromise(
    Effect.gen(function* () {
      const auth = yield* ForgeGitAuth;
      return yield* auth.authenticate({
        authorization,
        nowIso: "2026-07-25T20:00:00.000Z",
        repositoryRef: "openagents",
        requiredScope: "git:receive-pack",
        tenantRef: "openagents",
      });
    }).pipe(Effect.provide(makePolicyAuthorityAuthLayer(configuration, policyFetch))),
  );

describe("Forge Git policy authority adapter", () => {
  test("normalizes Git Basic auth and accepts only the worker policy session", async () => {
    const policyFetch = vi.fn(async (_input, init) => {
      expect(init?.headers).toMatchObject({
        authorization: "Bearer service-authority-secret",
      });
      expect(JSON.parse(String(init?.body))).toEqual({
        repositoryRef: "openagents",
        requiredScope: "git:receive-pack",
        tenantRef: "openagents",
        transportAuthorization: `Bearer ${token}`,
      });
      return Response.json({
        session: {
          authenticatedAt: "2026-07-25T20:00:01.000Z",
          refRestrictions: ["refs/heads/main"],
          repositoryRef: "openagents",
          subjectRef: "forge_actor.agent.fixture",
          tenantRef: "openagents",
          tokenRef: "forge_git_token.fixture",
        },
      });
    });
    const basic = `Basic ${Buffer.from(`git:${token}`).toString("base64")}`;

    await expect(authenticate(policyFetch, basic)).resolves.toMatchObject({
      subjectRef: "forge_actor.agent.fixture",
      tokenRef: "forge_git_token.fixture",
    });
    expect(policyFetch).toHaveBeenCalledOnce();
    expect(readForgeGitToken(basic)).toBe(token);
  });

  test("fails closed when membership is revoked or the authority is unavailable", async () => {
    await expect(
      authenticate(async () =>
        Response.json({ error: "forge_membership_tombstoned" }, { status: 403 }),
      ),
    ).rejects.toMatchObject({
      code: "forge_git_membership_forbidden",
      status: 403,
    });

    await expect(
      authenticate(async () => {
        throw new Error("policy authority unavailable");
      }),
    ).rejects.toMatchObject({
      code: "forge_git_unauthorized",
      status: 401,
    });
  });

  test("rejects a mismatched authority projection", async () => {
    await expect(
      authenticate(async () =>
        Response.json({
          session: {
            authenticatedAt: "2026-07-25T20:00:01.000Z",
            refRestrictions: [],
            repositoryRef: "other",
            subjectRef: "forge_actor.human.fixture",
            tenantRef: "openagents",
            tokenRef: "forge_git_token.fixture",
          },
        }),
      ),
    ).rejects.toMatchObject({
      code: "forge_git_tenant_forbidden",
      status: 403,
    });
  });
});
