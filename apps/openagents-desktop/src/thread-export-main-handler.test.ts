import { describe, expect, test } from "vite-plus/test";

import { DesktopThreadExportWriteChannel } from "./thread-export-bridge-contract.ts";
import {
  registerDesktopThreadExportMainHandler,
  type DesktopThreadExportMainHandler,
  type DesktopThreadExportMainHandlerDependencies,
} from "./thread-export-main-handler.ts";

const receipt = {
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.export.main.1",
  intentRef: "intent.export.main.1",
  idempotencyKey: "idempotency.export.main.1",
  threadRef: "thread.export.main.1",
  observedAt: "2026-07-17T16:01:00.000Z",
  kind: "thread.export.create" as const,
  result: {
    status: "export_created" as const,
    artifactRef: "artifact.export.main.1",
    artifactSha256: "b".repeat(64),
    format: "canonical_event_bundle" as const,
    artifactAudience: { kind: "owner_only" as const },
  },
};

const openFixture = (overrides: Partial<DesktopThreadExportMainHandlerDependencies> = {}) => {
  let handler: DesktopThreadExportMainHandler | undefined;
  const channels: string[] = [];
  let unregisters = 0;
  const writes: unknown[] = [];
  const dependencies: DesktopThreadExportMainHandlerDependencies = {
    register: (channel, value) => {
      channels.push(channel);
      handler = value;
      return () => {
        unregisters += 1;
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
    ...overrides,
  };
  const registration = registerDesktopThreadExportMainHandler(dependencies);
  if (handler === undefined) throw new Error("handler was not registered");
  return {
    channels,
    get unregisters() {
      return unregisters;
    },
    handler,
    registration,
    writes,
  };
};

describe("Desktop canonical-export main-process handler seam", () => {
  test("registers exactly the fixed channel and closes idempotently", async () => {
    const fixture = openFixture();
    expect(fixture.channels).toEqual([DesktopThreadExportWriteChannel]);

    fixture.registration.close();
    fixture.registration.close();
    expect(fixture.unregisters).toBe(1);
    await expect(fixture.handler("trusted", { receipt })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    expect(fixture.writes).toEqual([]);
  });

  test("rejects untrusted and malformed input before transport", async () => {
    const fixture = openFixture();
    await expect(fixture.handler("untrusted", { receipt })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    await expect(
      fixture.handler("trusted", { receipt, destinationPath: "/tmp/export.json" }),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_request" });
    await expect(
      fixture.handler("trusted", {
        receipt: {
          ...receipt,
          result: {
            ...receipt.result,
            artifactAudience: { kind: "internet_readable" },
          },
        },
      }),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_request" });
    expect(fixture.writes).toEqual([]);
  });

  test("passes only the decoded receipt to transport and returns its bounded result", async () => {
    const fixture = openFixture();
    await expect(fixture.handler("trusted", { receipt })).resolves.toEqual({
      status: "written",
      artifactRef: receipt.result.artifactRef,
      artifactSha256: receipt.result.artifactSha256,
      replaceAuthorized: false,
    });
    expect(fixture.writes).toEqual([receipt]);
    expect(JSON.stringify(fixture.writes)).not.toContain("destinationPath");
  });

  test("preserves cancelled and bounded rejection outcomes", async () => {
    const cancelled = openFixture({ write: async () => ({ status: "cancelled" }) });
    await expect(cancelled.handler("trusted", { receipt })).resolves.toEqual({
      status: "cancelled",
    });

    const rejected = openFixture({
      write: async () => ({ status: "rejected", reason: "destination_exists" }),
    });
    await expect(rejected.handler("trusted", { receipt })).resolves.toEqual({
      status: "rejected",
      reason: "destination_exists",
    });
  });

  test("collapses thrown, malformed, and path-leaking outcomes", async () => {
    const throwing = openFixture({
      write: async () => {
        throw new Error("/private/owner/export.json");
      },
    });
    await expect(throwing.handler("trusted", { receipt })).resolves.toEqual({
      status: "rejected",
      reason: "transport_unavailable",
    });

    for (const output of [
      { status: "written", filePath: "/private/owner/export.json" },
      {
        status: "written",
        artifactRef: receipt.result.artifactRef,
        artifactSha256: receipt.result.artifactSha256,
        replaceAuthorized: false,
        filePath: "/private/owner/export.json",
      },
    ]) {
      const malformed = openFixture({ write: async () => output });
      await expect(malformed.handler("trusted", { receipt })).resolves.toEqual({
        status: "rejected",
        reason: "transport_unavailable",
      });
    }
  });
});
