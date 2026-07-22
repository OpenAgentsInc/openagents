import {
  MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  type ManagedSandboxContentCheckpoint,
  type ManagedSandboxPhase2Command,
  type ManagedSandboxPhase2Error,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  MANAGED_SANDBOX_PHASE2_API_SCHEMA_VERSION,
  MANAGED_SANDBOX_PHASE2_COMMANDS_PATH,
  makeManagedSandboxPhase2Routes,
} from "./managed-sandbox-phase2-routes";

type TestEnv = Readonly<{ enabled: boolean }>;

const digest = (character: string): `sha256:${string}` => `sha256:${character.repeat(64)}`;

const omissions = {
  credentials: "excluded" as const,
  accountSecrets: "excluded" as const,
  providerHiddenState: "excluded" as const,
  processMemory: "excluded" as const,
  processTable: "excluded" as const,
  ptyState: "excluded" as const,
  sockets: "excluded" as const,
  ports: "excluded" as const,
  networkIdentity: "excluded" as const,
};

const command = {
  _tag: "CreateCheckpoint",
  schema: MANAGED_SANDBOX_PHASE2_COMMAND_SCHEMA_VERSION,
  commandRef: "command.sbx10.route.create",
  idempotencyRef: "idempotency.sbx10.route.create",
  ownerRef: "owner.sbx10.route",
  tenantRef: "owner.sbx10.route",
  requestedAt: "2026-07-22T04:00:00.000Z",
  checkpointRef: "checkpoint.sbx10.route",
  sourceSandboxRef: "sandbox.sbx10.route",
  sourceResourceGeneration: 8,
  sourceImageDigest: digest("a"),
  sourceToolchainDigest: digest("b"),
  repositoryRef: "repository.openagents",
  repositoryRevisionRef: "commit.02905a7",
  repositoryPostImageDigest: digest("c"),
  formatRef: "format.sbx.content-tar.v1",
  retainedUntil: "2026-07-23T04:00:00.000Z",
} satisfies Extract<ManagedSandboxPhase2Command, { _tag: "CreateCheckpoint" }>;

const checkpoint = {
  schema: MANAGED_SANDBOX_CONTENT_CHECKPOINT_SCHEMA_VERSION,
  checkpointRef: command.checkpointRef,
  ownerRef: command.ownerRef,
  tenantRef: command.tenantRef,
  sourceSandboxRef: command.sourceSandboxRef,
  sourceResourceGeneration: command.sourceResourceGeneration,
  sourceImageDigest: command.sourceImageDigest,
  sourceToolchainDigest: command.sourceToolchainDigest,
  repositoryRef: command.repositoryRef,
  repositoryRevisionRef: command.repositoryRevisionRef,
  repositoryPostImageDigest: command.repositoryPostImageDigest,
  contentDigest: digest("d"),
  contentBytes: 8_192,
  formatRef: command.formatRef,
  state: "completed",
  completedAt: "2026-07-22T04:00:01.000Z",
  verifiedAt: "2026-07-22T04:00:02.000Z",
  retainedUntil: command.retainedUntil,
  deleteOnExpiry: true,
  omissions,
  evidenceRefs: ["receipt.sbx10.route.verify"],
} satisfies ManagedSandboxContentCheckpoint;

const request = (value: unknown, method = "POST") =>
  new Request(`https://openagents.com${MANAGED_SANDBOX_PHASE2_COMMANDS_PATH}`, {
    method,
    headers: { "content-type": "application/json" },
    ...(method === "POST" ? { body: JSON.stringify(value) } : {}),
  });

const routeBody = (phase2Command: ManagedSandboxPhase2Command = command) => ({
  schemaVersion: MANAGED_SANDBOX_PHASE2_API_SCHEMA_VERSION,
  command: phase2Command,
});

const run = <Bindings>(
  routes: ReturnType<typeof makeManagedSandboxPhase2Routes<Bindings>>,
  req: Request,
  env: Bindings,
) => {
  // eslint-disable-next-line openagents/no-manual-effect-runtime-in-tests -- Vite Plus does not expose an Effect-aware test API; this helper is the suite runtime boundary.
  return Effect.runPromise(routes.commands(req, env, {} as ExecutionContext));
};

describe("managed sandbox Phase 2 Worker route", () => {
  it("publishes one exact no-store POST route", async () => {
    let executions = 0;
    const routes = makeManagedSandboxPhase2Routes<TestEnv>({
      authenticateOwner: async () => ({ userId: command.ownerRef }),
      enabled: (env) => env.enabled,
      execute: () => {
        executions += 1;
        return Effect.succeed(checkpoint);
      },
    });

    const response = await run(routes, request(routeBody(), "GET"), { enabled: true });

    expect(MANAGED_SANDBOX_PHASE2_COMMANDS_PATH).toBe("/api/managed-sandboxes/phase2/commands");
    expect(response.status).toBe(405);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(executions).toBe(0);
  });

  it("requires a human owner and stays default-off", async () => {
    let executions = 0;
    const unauthorized = makeManagedSandboxPhase2Routes<TestEnv>({
      authenticateOwner: async () => undefined,
      enabled: () => true,
      execute: () => Effect.succeed(checkpoint),
    });
    const disabled = makeManagedSandboxPhase2Routes<TestEnv>({
      authenticateOwner: async () => ({
        userId: command.ownerRef,
        decorateResponseHeaders: (headers) => headers.set("x-session-refresh", "set"),
      }),
      enabled: (env) => env.enabled,
      execute: () => {
        executions += 1;
        return Effect.succeed(checkpoint);
      },
    });

    const unauthorizedResponse = await run(unauthorized, request(routeBody()), {
      enabled: true,
    });
    const disabledResponse = await run(disabled, request(routeBody()), {
      enabled: false,
    });
    const disabledBody = (await disabledResponse.json()) as {
      error: ManagedSandboxPhase2Error;
    };

    expect(unauthorizedResponse.status).toBe(401);
    expect(disabledResponse.status).toBe(503);
    expect(disabledResponse.headers.get("x-session-refresh")).toBe("set");
    expect(disabledBody.error).toMatchObject({
      _tag: "InvalidRequest",
      requestRef: "request.phase2.activation",
      retryable: true,
    });
    expect(executions).toBe(0);
  });

  it("binds both command scopes to the authenticated owner", async () => {
    let executions = 0;
    const routes = makeManagedSandboxPhase2Routes<TestEnv>({
      authenticateOwner: async () => ({ userId: command.ownerRef }),
      enabled: () => true,
      execute: () => {
        executions += 1;
        return Effect.succeed(checkpoint);
      },
    });
    const mismatched = {
      ...command,
      tenantRef: "tenant.outside-owner-scope",
    } satisfies ManagedSandboxPhase2Command;

    const response = await run(routes, request(routeBody(mismatched)), { enabled: true });

    expect(response.status).toBe(403);
    expect(await response.json()).toEqual({ error: "owner_scope_mismatch" });
    expect(executions).toBe(0);
  });

  it("returns the validated execution result and refreshed owner session", async () => {
    const seen: ManagedSandboxPhase2Command[] = [];
    const routes = makeManagedSandboxPhase2Routes<TestEnv>({
      authenticateOwner: async () => ({
        userId: command.ownerRef,
        decorateResponseHeaders: (headers) => headers.set("x-session-refresh", "set"),
      }),
      enabled: () => true,
      execute: (_env, input) => {
        seen.push(input);
        return Effect.succeed(checkpoint);
      },
    });

    const response = await run(routes, request(routeBody()), { enabled: true });

    expect(response.status).toBe(200);
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(response.headers.get("x-session-refresh")).toBe("set");
    expect(await response.json()).toEqual({
      schemaVersion: MANAGED_SANDBOX_PHASE2_API_SCHEMA_VERSION,
      result: checkpoint,
    });
    expect(seen).toEqual([command]);
  });

  it("rejects malformed or oversized command envelopes before execution", async () => {
    let executions = 0;
    const routes = makeManagedSandboxPhase2Routes<TestEnv>({
      authenticateOwner: async () => ({ userId: command.ownerRef }),
      enabled: () => true,
      execute: () => {
        executions += 1;
        return Effect.succeed(checkpoint);
      },
    });

    const malformed = await run(routes, request({ ...routeBody(), excess: "forbidden" }), {
      enabled: true,
    });
    const oversized = await run(
      routes,
      new Request(`https://openagents.com${MANAGED_SANDBOX_PHASE2_COMMANDS_PATH}`, {
        method: "POST",
        body: "x".repeat(128 * 1024 + 1),
      }),
      { enabled: true },
    );
    const falselyDeclaredOversized = await run(
      routes,
      new Request(`https://openagents.com${MANAGED_SANDBOX_PHASE2_COMMANDS_PATH}`, {
        method: "POST",
        headers: { "content-length": String(128 * 1024 + 1) },
        body: "{}",
      }),
      { enabled: true },
    );

    expect(malformed.status).toBe(400);
    expect(oversized.status).toBe(400);
    expect(falselyDeclaredOversized.status).toBe(400);
    expect(executions).toBe(0);
  });

  it("maps typed service failures without dropping refreshed owner state", async () => {
    const routes = makeManagedSandboxPhase2Routes<TestEnv>({
      authenticateOwner: async () => ({
        userId: command.ownerRef,
        decorateResponseHeaders: (headers) => headers.set("x-session-refresh", "set"),
      }),
      enabled: () => true,
      execute: () =>
        Effect.fail({
          _tag: "CheckpointIncomplete",
          checkpointRef: command.checkpointRef,
          message: "the completed checkpoint does not exist",
          retryable: false,
          evidenceRefs: [],
        }),
    });

    const response = await run(routes, request(routeBody()), { enabled: true });

    expect(response.status).toBe(404);
    expect(response.headers.get("x-session-refresh")).toBe("set");
    expect((await response.json()) as object).toMatchObject({
      schemaVersion: MANAGED_SANDBOX_PHASE2_API_SCHEMA_VERSION,
      error: {
        _tag: "CheckpointIncomplete",
        checkpointRef: command.checkpointRef,
      },
    });
  });
});
