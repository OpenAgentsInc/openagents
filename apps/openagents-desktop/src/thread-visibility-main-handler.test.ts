import { describe, expect, test } from "vite-plus/test";

import { DesktopThreadVisibilityApplyChannel } from "./thread-visibility-bridge-contract.ts";
import {
  registerDesktopThreadVisibilityMainHandler,
  type DesktopThreadVisibilityMainHandler,
  type DesktopThreadVisibilityMainHandlerDependencies,
} from "./thread-visibility-main-handler.ts";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.visibility.main.1",
  idempotencyKey: "idempotency.visibility.main.1",
  threadRef: "thread.visibility.main.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "unknown" as const, reason: "not_observed" as const },
  createdAt: "2026-07-17T18:34:00.000Z",
  kind: "thread.visibility.set" as const,
  target: {
    audience: { kind: "workspace_members" as const, workspaceRef: "workspace.1" },
    administratorAccess: { kind: "workspace_admins" as const, workspaceRef: "workspace.1" },
  },
};

const receipt = {
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.visibility.main.1",
  intentRef: intent.intentRef,
  idempotencyKey: intent.idempotencyKey,
  threadRef: intent.threadRef,
  observedAt: "2026-07-17T18:34:01.000Z",
  kind: "thread.visibility.set" as const,
  result: {
    status: "visibility_applied" as const,
    visibilityVersion: 1,
    target: intent.target,
  },
};

const openFixture = (overrides: Partial<DesktopThreadVisibilityMainHandlerDependencies> = {}) => {
  let handler: DesktopThreadVisibilityMainHandler | undefined;
  const channels: string[] = [];
  let unregisters = 0;
  const applications: unknown[] = [];
  const dependencies: DesktopThreadVisibilityMainHandlerDependencies = {
    register: (channel, value) => {
      channels.push(channel);
      handler = value;
      return () => {
        unregisters += 1;
      };
    },
    isTrustedSender: (event) => event === "trusted",
    makeReceiptRef: () => receipt.receiptRef,
    observedAt: () => receipt.observedAt,
    apply: async (value) => {
      applications.push(value);
      return { status: "stored", receipt };
    },
    ...overrides,
  };
  const registration = registerDesktopThreadVisibilityMainHandler(dependencies);
  if (handler === undefined) throw new Error("handler was not registered");
  return {
    applications,
    channels,
    handler,
    registration,
    get unregisters() {
      return unregisters;
    },
  };
};

describe("Desktop thread-visibility main-process handler seam", () => {
  test("registers exactly the fixed channel and closes idempotently", async () => {
    const fixture = openFixture();
    expect(fixture.channels).toEqual([DesktopThreadVisibilityApplyChannel]);

    fixture.registration.close();
    fixture.registration.close();
    expect(fixture.unregisters).toBe(1);
    await expect(fixture.handler("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    expect(fixture.applications).toEqual([]);
  });

  test("rejects untrusted, throwing-trust, malformed, and broader input before apply", async () => {
    const fixture = openFixture();
    await expect(fixture.handler("untrusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    await expect(
      fixture.handler("trusted", { intent, receiptRef: "receipt.renderer.chosen" }),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_request" });
    await expect(
      fixture.handler("trusted", { intent: { ...intent, transcript: "private thread" } }),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_request" });
    await expect(
      fixture.handler("trusted", { intent: { ...intent, kind: "thread.export.create" } }),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_request" });
    const throwingTrust = openFixture({
      isTrustedSender: () => {
        throw new Error("native sender detail");
      },
    });
    await expect(throwingTrust.handler("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    expect(fixture.applications).toEqual([]);
    expect(throwingTrust.applications).toEqual([]);
  });

  test("passes decoded intent with host-owned receipt metadata and returns bound evidence", async () => {
    const fixture = openFixture();
    await expect(fixture.handler("trusted", { intent })).resolves.toEqual({
      status: "stored",
      receipt,
    });
    expect(fixture.applications).toEqual([
      { intent, receiptRef: receipt.receiptRef, observedAt: receipt.observedAt },
    ]);
    expect(JSON.stringify(fixture.applications)).not.toContain("transcript");
  });

  test("preserves unchanged and bounded policy rejection outcomes", async () => {
    const unchanged = openFixture({ apply: async () => ({ status: "unchanged", receipt }) });
    await expect(unchanged.handler("trusted", { intent })).resolves.toEqual({
      status: "unchanged",
      receipt,
    });

    const rejected = openFixture({
      apply: async () => ({ status: "rejected", reason: "stale_version" }),
    });
    await expect(rejected.handler("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "stale_version",
    });
  });

  test("collapses metadata, apply, malformed, mismatched, and detail-leaking failures", async () => {
    for (const overrides of [
      {
        makeReceiptRef: () => {
          throw new Error("native receipt detail");
        },
      },
      {
        apply: async () => {
          throw new Error("/private/owner/visibility.json");
        },
      },
      { apply: async () => ({ status: "stored", receipt: { ...receipt, threadRef: "thread.other" } }) },
      { apply: async () => ({ status: "stored", receipt, filePath: "/private/owner/visibility.json" }) },
      { apply: async () => ({ status: "rejected", reason: "native_error" }) },
    ] satisfies ReadonlyArray<Partial<DesktopThreadVisibilityMainHandlerDependencies>>) {
      const fixture = openFixture(overrides);
      await expect(fixture.handler("trusted", { intent })).resolves.toEqual({
        status: "rejected",
        reason: "command_unavailable",
      });
    }
  });
});
