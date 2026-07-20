import { describe, expect, test } from "vitest";

import {
  makePylonPortableTargetBindingClient,
  portableTargetPylonBindingDigest,
} from "../src/portable-target-pylon-binding-client.js";
import type { PylonPortableControlBinding } from "../src/portable-session-operation-ledger.js";

const binding: PylonPortableControlBinding = {
  schema: "openagents.pylon.portable_operation_ledger.v1",
  sessionRef: "session.ide13.binding",
  attachmentRef: "attachment.ide13.binding",
  generation: 3,
  runtimeInstanceRef: "runtime.ide13.binding",
  state: "accepting",
  revision: 7,
  agents: [
    {
      agentRef: "agent.ide13.local",
      controlSessionRef: "control.ide13.local",
      workspaceRef: "workspace.ide13.local",
      processLifecycle: "active",
      workspaceLifecycle: "retained",
    },
  ],
};

const options = {
  agentToken: "agent-token",
  baseUrl: "https://openagents.com",
  pylonRef: "pylon.ide13.binding",
  sessionRef: binding.sessionRef,
  targetRef: "target.ide13.binding",
  workerInstanceRef: "worker.ide13.binding",
  binding,
} as const;

describe("Pylon portable target binding client", () => {
  test("binds only the exact local target facts and renews with CAS", async () => {
    const requests: Array<{ method: string; body: Record<string, unknown>; key: string | null }> =
      [];
    let revision = 0;
    const client = makePylonPortableTargetBindingClient({
      ...options,
      fetchImpl: async (_input, init) => {
        if (init?.method === "GET") {
          requests.push({ method: "GET", body: {}, key: null });
          return new Response(null, { status: 404 });
        }
        revision += 1;
        requests.push({
          method: init?.method ?? "GET",
          body: JSON.parse(String(init?.body)) as Record<string, unknown>,
          key: new Headers(init?.headers).get("idempotency-key"),
        });
        return Response.json({
          binding: {
            bindingRef: "binding.ide13.server",
            sessionRef: options.sessionRef,
            targetRef: options.targetRef,
            pylonRef: options.pylonRef,
            workerInstanceRef: options.workerInstanceRef,
            bindingDigest: portableTargetPylonBindingDigest(options),
            revision,
            state: init?.method === "DELETE" ? "revoked" : "active",
            health: init?.method === "DELETE" ? "revoked" : "ready",
            expiresAt: "2026-07-20T13:05:00.000Z",
          },
        });
      },
    });

    await client.admitOrRenew();
    await client.admitOrRenew("draining");
    await client.revoke();

    const digest = portableTargetPylonBindingDigest(options);
    expect(digest).toMatch(/^sha256:[0-9a-f]{64}$/);
    expect(requests.map((item) => item.method)).toEqual(["GET", "POST", "POST", "DELETE"]);
    expect(requests[1]?.body).toMatchObject({
      sessionRef: binding.sessionRef,
      workerInstanceRef: options.workerInstanceRef,
      bindingDigest: digest,
    });
    expect(requests[1]?.body).not.toHaveProperty("targetRef");
    expect(requests[1]?.body).not.toHaveProperty("expectedRevision");
    expect(requests[2]?.body).toMatchObject({ expectedRevision: 1, health: "draining" });
    expect(requests[3]?.body).toMatchObject({ expectedRevision: 2 });
    expect(new Set(requests.slice(1).map((item) => item.key)).size).toBe(3);
  });

  test("rejects a session that differs from the durable local binding", () => {
    expect(() =>
      makePylonPortableTargetBindingClient({
        ...options,
        sessionRef: "session.ide13.other",
      }),
    ).toThrow("configuration is invalid");
  });

  test("fails closed when the server does not admit the binding", async () => {
    const client = makePylonPortableTargetBindingClient({
      ...options,
      fetchImpl: async () => new Response(null, { status: 403 }),
    });
    await expect(client.admitOrRenew()).rejects.toThrow("read failed (403)");
  });
});
