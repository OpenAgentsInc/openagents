import { describe, expect, test } from "vite-plus/test";

import { DesktopThreadExportCreateChannel } from "./thread-export-create-bridge-contract.ts";
import {
  registerDesktopThreadExportCreateMainHandler,
  type DesktopThreadExportCreateMainHandler,
  type DesktopThreadExportCreateMainHandlerDependencies,
} from "./thread-export-create-main-handler.ts";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.export.create.main.1",
  idempotencyKey: "idempotency.export.create.main.1",
  threadRef: "thread.export.create.main.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 4 },
  createdAt: "2026-07-17T16:41:00.000Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const receipt = {
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.export.create.main.1",
  intentRef: intent.intentRef,
  idempotencyKey: intent.idempotencyKey,
  threadRef: intent.threadRef,
  observedAt: "2026-07-17T16:41:01.000Z",
  kind: "thread.export.create" as const,
  result: {
    status: "export_created" as const,
    artifactRef: "artifact.export.create.main.1",
    artifactSha256: "d".repeat(64),
    format: "canonical_event_bundle" as const,
    artifactAudience: { kind: "owner_only" as const },
  },
};

const openFixture = (overrides: Partial<DesktopThreadExportCreateMainHandlerDependencies> = {}) => {
  let handler: DesktopThreadExportCreateMainHandler | undefined;
  const channels: string[] = [];
  let unregisters = 0;
  const executions: unknown[] = [];
  const dependencies: DesktopThreadExportCreateMainHandlerDependencies = {
    register: (channel, value) => {
      channels.push(channel);
      handler = value;
      return () => {
        unregisters += 1;
      };
    },
    isTrustedSender: (event) => event === "trusted",
    execute: async (value) => {
      executions.push(value);
      return { status: "stored", receipt };
    },
    ...overrides,
  };
  const registration = registerDesktopThreadExportCreateMainHandler(dependencies);
  if (handler === undefined) throw new Error("handler was not registered");
  return {
    channels,
    executions,
    handler,
    registration,
    get unregisters() {
      return unregisters;
    },
  };
};

describe("Desktop canonical-export creation main-process handler seam", () => {
  test("registers exactly the fixed channel and closes idempotently", async () => {
    const fixture = openFixture();
    expect(fixture.channels).toEqual([DesktopThreadExportCreateChannel]);

    fixture.registration.close();
    fixture.registration.close();
    expect(fixture.unregisters).toBe(1);
    await expect(fixture.handler("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    expect(fixture.executions).toEqual([]);
  });

  test("rejects untrusted, throwing-trust, malformed, and broader input before command", async () => {
    const fixture = openFixture();
    await expect(fixture.handler("untrusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    await expect(
      fixture.handler("trusted", { intent: { ...intent, events: [{ private: true }] } }),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_request" });
    await expect(
      fixture.handler("trusted", {
        intent: { ...intent, artifactAudience: { kind: "internet_readable" } },
      }),
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
    expect(fixture.executions).toEqual([]);
    expect(throwingTrust.executions).toEqual([]);
  });

  test("passes only the decoded intent to command and returns its identity-bound receipt", async () => {
    const fixture = openFixture();
    await expect(fixture.handler("trusted", { intent })).resolves.toEqual({
      status: "stored",
      receipt,
    });
    expect(fixture.executions).toEqual([intent]);
    expect(JSON.stringify(fixture.executions)).not.toContain("events");
    expect(JSON.stringify(fixture.executions)).not.toContain("receiptRef");
  });

  test("preserves unchanged and bounded command rejection outcomes", async () => {
    const unchanged = openFixture({ execute: async () => ({ status: "unchanged", receipt }) });
    await expect(unchanged.handler("trusted", { intent })).resolves.toEqual({
      status: "unchanged",
      receipt,
    });

    const rejected = openFixture({
      execute: async () => ({ status: "rejected", reason: "invalid_evidence" }),
    });
    await expect(rejected.handler("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_evidence",
    });
  });

  test("collapses thrown, mismatched, malformed, and path-leaking command outcomes", async () => {
    const throwing = openFixture({
      execute: async () => {
        throw new Error("/private/owner/export.json");
      },
    });
    await expect(throwing.handler("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "command_unavailable",
    });

    for (const output of [
      { status: "stored", receipt: { ...receipt, threadRef: "thread.other" } },
      { status: "stored", receipt, filePath: "/private/owner/export.json" },
      { status: "rejected", reason: "native_error" },
    ]) {
      const malformed = openFixture({ execute: async () => output });
      await expect(malformed.handler("trusted", { intent })).resolves.toEqual({
        status: "rejected",
        reason: "command_unavailable",
      });
    }
  });
});
