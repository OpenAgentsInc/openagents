import {
  ManagedSandboxCommandSchema,
  type ManagedSandboxCommand,
} from "@openagentsinc/managed-sandbox-contract";
import { Effect, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { makeManagedSandboxBroker } from "./managed-sandbox-broker";
import {
  BoxV1MemoryAuthority,
  boxV1TestPolicy,
  makeBoxV1MemoryRuntime,
} from "./managed-sandbox-box-v1.test-support";
import {
  MANAGED_SANDBOX_MOBILE_SUPERVISION_PATH,
  MANAGED_SANDBOX_WEB_SUPERVISION_PATH,
  makeManagedSandboxSupervisionRoutes,
} from "./managed-sandbox-supervision-routes";

const ownerRef = "owner.supervision.fixture";
const now = "2026-07-19T18:35:00.000Z";
const issuedAt = "2026-07-19T18:34:00.000Z";
const context = {} as ExecutionContext;

const decodeCommand = S.decodeUnknownSync(ManagedSandboxCommandSchema);

const create = (): ManagedSandboxCommand =>
  decodeCommand({
    _tag: "Create",
    schema: "openagents.managed_sandbox_command.v1",
    commandRef: "command.supervision.create",
    requestedByRef: "principal.desktop",
    ownerRef,
    tenantRef: ownerRef,
    idempotencyRef: "idempotency.supervision.create",
    requestedAt: "2026-07-19T18:00:00.000Z",
    workUnitRef: "work.supervision.fixture",
    attachmentRef: "attachment.supervision.fixture",
    target: boxV1TestPolicy.target,
    imageDigest: boxV1TestPolicy.imageDigest,
    profileRef: boxV1TestPolicy.profileRef,
    lease: {
      leaseRef: "lease.supervision.fixture",
      state: "active",
      issuedAt: "2026-07-19T18:00:00.000Z",
      expiresAt: "2026-07-19T19:00:00.000Z",
      ttlSeconds: 3_600,
      renewable: true,
    },
    budget: {
      currency: "USD",
      maxCostMicros: 10_000,
      maxCpuMillis: 3_600_000,
      maxNetworkBytes: 100_000_000,
      maxArtifactBytes: 10_000_000,
      maxLifetimeSeconds: 3_600,
    },
    requestedCapabilities: [
      {
        capabilityRef: "capability.supervision.agent_turn",
        kind: "agent_turn",
        state: "active",
        expiresAt: "2026-07-19T19:00:00.000Z",
      },
    ],
  });

const fixture = async () => {
  const store = new BoxV1MemoryAuthority();
  const runtime = makeBoxV1MemoryRuntime();
  const desktop = makeManagedSandboxBroker({
    principal: {
      actorRef: "principal.desktop",
      ownerRef,
      tenantRef: ownerRef,
      login: "Desktop",
      email: null,
    },
    policy: boxV1TestPolicy,
    store,
    runtime,
    now: () => new Date("2026-07-19T18:00:00.000Z"),
  });
  const created = await Effect.runPromise(desktop.execute(create(), { attachmentGeneration: 5 }));
  const sarah = makeManagedSandboxBroker({
    principal: {
      actorRef: "principal.sarah",
      ownerRef,
      tenantRef: ownerRef,
      login: "Sarah",
      email: null,
    },
    policy: boxV1TestPolicy,
    store,
    runtime,
    now: () => new Date("2026-07-19T18:30:00.000Z"),
  });
  const dispatched = await Effect.runPromise(
    sarah.execute(
      decodeCommand({
        _tag: "Dispatch",
        schema: "openagents.managed_sandbox_command.v1",
        commandRef: "command.supervision.dispatch",
        requestedByRef: "principal.sarah",
        ownerRef,
        tenantRef: ownerRef,
        idempotencyRef: "idempotency.supervision.dispatch",
        requestedAt: "2026-07-19T18:30:00.000Z",
        sandboxRef: created.resource.sandboxRef,
        expectedVersion: created.resource.version,
        turnRef: "turn.supervision.fixture",
        capabilityRef: "capability.supervision.agent_turn",
        promptDigest: "sha256:a9740b096ec755b76570b0db71d62885f995f471add50bef4971a5e11e6d09df",
        runtime: {
          provider: "codex",
          modelRef: "model.gpt-5.6-sol",
          harnessRef: "harness.codex.v1",
        },
      }),
      { prompt: "bounded test prompt" },
    ),
  );
  const routes = makeManagedSandboxSupervisionRoutes({
    authenticateOwner: async () => ({ userId: ownerRef }),
    enabled: () => true,
    policy: () => Effect.succeed(boxV1TestPolicy),
    store: () => store,
    runtime: () => Effect.succeed(runtime),
    now: () => new Date(now),
  });
  return { routes, store, resource: dispatched.resource };
};

const command = (
  resource: Readonly<{ sandboxRef: string; version: number; resourceGeneration: number }>,
  overrides: Record<string, unknown> = {},
) => ({
  schema: "openagents.managed_sandbox_supervision_command.v1",
  _tag: "Interrupt",
  commandRef: "command.web.interrupt.fixture",
  idempotencyRef: "idempotency.web.interrupt.fixture",
  surface: "web",
  sandboxRef: resource.sandboxRef,
  expectedVersion: resource.version,
  expectedResourceGeneration: resource.resourceGeneration,
  turnRef: "turn.supervision.fixture",
  reasonRef: "reason.owner_interrupt",
  issuedAt,
  expiresAt: "2026-07-19T18:36:00.000Z",
  ...overrides,
});

describe("managed-sandbox mobile/web supervision routes", () => {
  test("publishes only the bounded shared projection with Sarah actor attribution", async () => {
    const { routes } = await fixture();
    const response = await Effect.runPromise(
      routes.mobile(
        new Request(`https://openagents.com${MANAGED_SANDBOX_MOBILE_SUPERVISION_PATH}`),
        {},
        context,
      ),
    );
    expect(response.status).toBe(200);
    const body = (await response.json()) as Record<string, unknown>;
    expect(body).not.toHaveProperty("ownerRef");
    expect(JSON.stringify(body)).not.toMatch(
      /prompt|credential|rawPath|runtimeOutput|imageDigest/iu,
    );
    expect(body).toMatchObject({
      projections: [
        {
          attachmentGeneration: 5,
          resourceGeneration: 1,
          target: { provider: "google_cloud", isolation: "gce_vm" },
          runtime: {
            turnRef: "turn.supervision.fixture",
            actorRef: "principal.sarah",
            identity: { provider: "codex", modelRef: "model.gpt-5.6-sol" },
          },
          state: { lifecycle: "running" },
          cleanup: { state: "not_started" },
        },
      ],
    });
  });

  test("applies a typed web interrupt once and replays its receipt", async () => {
    const { routes, resource } = await fixture();
    const request = () =>
      new Request(`https://openagents.com${MANAGED_SANDBOX_WEB_SUPERVISION_PATH}`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ command: command(resource) }),
      });
    const first = await Effect.runPromise(routes.web(request(), {}, context));
    expect(first.status).toBe(200);
    const firstBody = await first.json();
    expect(firstBody).toMatchObject({
      state: "applied",
      commandRef: "command.web.interrupt.fixture",
      projection: {
        runtime: { actorRef: "principal.sarah", status: "interrupted" },
      },
    });

    const replay = await Effect.runPromise(routes.web(request(), {}, context));
    expect(replay.status).toBe(200);
    await expect(replay.json()).resolves.toMatchObject({
      commandRef: "command.web.interrupt.fixture",
      receiptRefs: (firstBody as { receiptRefs: string[] }).receiptRefs,
    });
  });

  test("refuses surface substitution, stale generation, expiry, and unauthenticated access before effects", async () => {
    const { routes, resource } = await fixture();
    const post = (payload: unknown) =>
      new Request(`https://openagents.com${MANAGED_SANDBOX_WEB_SUPERVISION_PATH}`, {
        method: "POST",
        body: JSON.stringify({ command: payload }),
      });
    const surface = await Effect.runPromise(
      routes.web(post(command(resource, { surface: "mobile" })), {}, context),
    );
    expect(surface.status).toBe(403);

    const stale = await Effect.runPromise(
      routes.web(
        post(
          command(resource, {
            commandRef: "command.web.stale.fixture",
            idempotencyRef: "idempotency.web.stale.fixture",
            expectedResourceGeneration: resource.resourceGeneration + 1,
          }),
        ),
        {},
        context,
      ),
    );
    expect(stale.status).toBe(409);
    await expect(stale.json()).resolves.toMatchObject({ reasonRef: "reason.stale_generation" });

    const expired = await Effect.runPromise(
      routes.web(
        post(
          command(resource, {
            commandRef: "command.web.expired.fixture",
            idempotencyRef: "idempotency.web.expired.fixture",
            expiresAt: "2026-07-19T18:34:30.000Z",
          }),
        ),
        {},
        context,
      ),
    );
    expect(expired.status).toBe(409);
    await expect(expired.json()).resolves.toMatchObject({ reasonRef: "reason.command_expired" });

    const unauthenticatedRoutes = makeManagedSandboxSupervisionRoutes({
      authenticateOwner: async () => undefined,
      enabled: () => true,
      policy: () => Effect.succeed(boxV1TestPolicy),
      store: () => {
        throw new Error("must not reach store");
      },
      runtime: () => Effect.succeed(makeBoxV1MemoryRuntime()),
    });
    const unauthorized = await Effect.runPromise(
      unauthenticatedRoutes.web(
        new Request(`https://openagents.com${MANAGED_SANDBOX_WEB_SUPERVISION_PATH}`),
        {},
        context,
      ),
    );
    expect(unauthorized.status).toBe(401);
  });
});
