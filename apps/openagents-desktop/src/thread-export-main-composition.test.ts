import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { DesktopThreadExportWriteChannel } from "./thread-export-bridge-contract.ts";
import { DesktopThreadExportCreateChannel } from "./thread-export-create-bridge-contract.ts";
import type { DesktopThreadExportCreateMainHandler } from "./thread-export-create-main-handler.ts";
import {
  DesktopThreadExportMainCompositionUnavailable,
  openDesktopThreadExportMainComposition,
  type DesktopThreadExportMainCompositionDependencies,
} from "./thread-export-main-composition.ts";
import type { DesktopThreadExportMainHandler } from "./thread-export-main-handler.ts";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.export.composition.1",
  idempotencyKey: "idempotency.export.composition.1",
  threadRef: "thread.export.composition.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 8 },
  createdAt: "2026-07-17T17:32:00.000Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const receipt = {
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.export.composition.1",
  intentRef: intent.intentRef,
  idempotencyKey: intent.idempotencyKey,
  threadRef: intent.threadRef,
  observedAt: "2026-07-17T17:32:01.000Z",
  kind: "thread.export.create" as const,
  result: {
    status: "export_created" as const,
    artifactRef: "artifact.export.composition.1",
    artifactSha256: "a".repeat(64),
    format: "canonical_event_bundle" as const,
    artifactAudience: { kind: "owner_only" as const },
  },
};

const fixture = (overrides: Partial<DesktopThreadExportMainCompositionDependencies> = {}) => {
  const channels: string[] = [];
  const closes: string[] = [];
  const writes: unknown[] = [];
  const executions: unknown[] = [];
  let writeHandler: DesktopThreadExportMainHandler | undefined;
  let createHandler: DesktopThreadExportCreateMainHandler | undefined;
  const dependencies: DesktopThreadExportMainCompositionDependencies = {
    registerWrite: (channel, handler) => {
      channels.push(channel);
      writeHandler = handler;
      return () => {
        closes.push("write");
      };
    },
    registerCreate: (channel, handler) => {
      channels.push(channel);
      createHandler = handler;
      return () => {
        closes.push("create");
      };
    },
    isTrustedSender: (event) => event === "trusted",
    write: async (value) => {
      writes.push(value);
      return {
        status: "written",
        artifactRef: receipt.result.artifactRef,
        artifactSha256: receipt.result.artifactSha256,
        replaceAuthorized: false,
      };
    },
    execute: async (value) => {
      executions.push(value);
      return { status: "stored", receipt };
    },
    ...overrides,
  };
  const open = () => Effect.runPromise(openDesktopThreadExportMainComposition(dependencies));
  return {
    channels,
    closes,
    writes,
    executions,
    open,
    get writeHandler() {
      return writeHandler;
    },
    get createHandler() {
      return createHandler;
    },
  };
};

describe("Desktop canonical-export main composition resource", () => {
  test("registers both fixed handlers and delegates through their bounded seams", async () => {
    const value = fixture();
    const registration = await value.open();
    expect(value.channels).toEqual([
      DesktopThreadExportWriteChannel,
      DesktopThreadExportCreateChannel,
    ]);
    if (value.writeHandler === undefined || value.createHandler === undefined) {
      throw new Error("composition did not register both handlers");
    }

    await expect(value.createHandler("trusted", { intent })).resolves.toEqual({
      status: "stored",
      receipt,
    });
    await expect(value.writeHandler("trusted", { receipt })).resolves.toEqual({
      status: "written",
      artifactRef: receipt.result.artifactRef,
      artifactSha256: receipt.result.artifactSha256,
      replaceAuthorized: false,
    });
    expect(value.executions).toEqual([intent]);
    expect(value.writes).toEqual([receipt]);
    registration.close();
  });

  test("closes in reverse acquisition order exactly once", async () => {
    const value = fixture();
    const registration = await value.open();
    registration.close();
    registration.close();
    expect(value.closes).toEqual(["create", "write"]);
  });

  test("returns a bounded write-stage failure without attempting create", async () => {
    const value = fixture({
      registerWrite: () => {
        throw new Error("/private/native/write-registration");
      },
    });
    await expect(value.open()).rejects.toEqual(
      new DesktopThreadExportMainCompositionUnavailable({ stage: "write" }),
    );
    expect(value.channels).toEqual([]);
    expect(value.closes).toEqual([]);
  });

  test("rolls back write exactly once when create registration fails", async () => {
    const value = fixture({
      registerCreate: () => {
        throw new Error("/private/native/create-registration");
      },
    });
    await expect(value.open()).rejects.toEqual(
      new DesktopThreadExportMainCompositionUnavailable({ stage: "create" }),
    );
    expect(value.channels).toEqual([DesktopThreadExportWriteChannel]);
    expect(value.closes).toEqual(["write"]);
  });

  test("does not project cleanup failures and still attempts both cleanups", async () => {
    const closes: string[] = [];
    const value = fixture({
      registerWrite: () => () => {
        closes.push("write");
        throw new Error("/private/native/write-cleanup");
      },
      registerCreate: () => () => {
        closes.push("create");
        throw new Error("/private/native/create-cleanup");
      },
    });
    const registration = await value.open();
    expect(() => registration.close()).not.toThrow();
    expect(() => registration.close()).not.toThrow();
    expect(closes).toEqual(["create", "write"]);
  });
});
