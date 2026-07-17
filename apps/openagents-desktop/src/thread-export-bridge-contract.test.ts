import { readFileSync } from "node:fs";

import { describe, expect, test } from "vite-plus/test";

import {
  DesktopThreadExportWriteChannel,
  decodeDesktopThreadExportWriteRequest,
  decodeDesktopThreadExportWriteResult,
  invokeDesktopThreadExportWrite,
} from "./thread-export-bridge-contract.ts";

const receipt = {
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.export.bridge.1",
  intentRef: "intent.export.bridge.1",
  idempotencyKey: "idempotency.export.bridge.1",
  threadRef: "thread.export.bridge.1",
  observedAt: "2026-07-17T15:34:00.000Z",
  kind: "thread.export.create" as const,
  result: {
    status: "export_created" as const,
    artifactRef: "artifact.export.bridge.1",
    artifactSha256: "a".repeat(64),
    format: "canonical_event_bundle" as const,
    artifactAudience: { kind: "owner_only" as const },
  },
};

describe("Desktop canonical-export preload boundary", () => {
  test("admits only an exact ref-only owner canonical-export receipt", () => {
    expect(decodeDesktopThreadExportWriteRequest({ receipt })).toEqual({ receipt });
    expect(
      decodeDesktopThreadExportWriteRequest({
        receipt: { ...receipt, body: "private transcript" },
      }),
    ).toBeNull();
    expect(
      decodeDesktopThreadExportWriteRequest({
        receipt: {
          ...receipt,
          result: {
            ...receipt.result,
            artifactAudience: { kind: "internet_readable" },
          },
        },
      }),
    ).toBeNull();
    expect(
      decodeDesktopThreadExportWriteRequest({ receipt, destinationPath: "/tmp/export.json" }),
    ).toBeNull();
    expect(
      decodeDesktopThreadExportWriteRequest({
        receipt: { ...receipt, result: { status: "accepted_pending" } },
      }),
    ).toBeNull();
  });

  test("invokes exactly one fixed channel with only the decoded receipt", async () => {
    const calls: unknown[] = [];
    const result = await invokeDesktopThreadExportWrite(
      async (channel, request) => {
        calls.push({ channel, request });
        return {
          status: "written",
          artifactRef: receipt.result.artifactRef,
          artifactSha256: receipt.result.artifactSha256,
          replaceAuthorized: false,
        };
      },
      { receipt },
    );

    expect(result).toEqual({
      status: "written",
      artifactRef: receipt.result.artifactRef,
      artifactSha256: receipt.result.artifactSha256,
      replaceAuthorized: false,
    });
    expect(calls).toEqual([
      {
        channel: DesktopThreadExportWriteChannel,
        request: { receipt },
      },
    ]);
    expect(JSON.stringify(calls)).not.toContain("destinationPath");
    expect(JSON.stringify(calls)).not.toContain("private transcript");
  });

  test("rejects invalid input before invocation", async () => {
    let invoked = false;
    const result = await invokeDesktopThreadExportWrite(
      async () => {
        invoked = true;
        return { status: "cancelled" };
      },
      { receipt, filePath: "/tmp/export.json" },
    );

    expect(result).toEqual({ status: "rejected", reason: "invalid_request" });
    expect(invoked).toBe(false);
  });

  test("decodes bounded path-free outcomes and rejects leaked or malformed replies", () => {
    expect(decodeDesktopThreadExportWriteResult({ status: "cancelled" })).toEqual({
      status: "cancelled",
    });
    expect(
      decodeDesktopThreadExportWriteResult({
        status: "rejected",
        reason: "destination_exists",
      }),
    ).toEqual({ status: "rejected", reason: "destination_exists" });
    expect(
      decodeDesktopThreadExportWriteResult({
        status: "written",
        artifactRef: receipt.result.artifactRef,
        artifactSha256: receipt.result.artifactSha256,
        replaceAuthorized: true,
        filePath: "/tmp/export.json",
      }),
    ).toBeNull();
    expect(
      decodeDesktopThreadExportWriteResult({ status: "rejected", reason: "native_error" }),
    ).toBeNull();
  });

  test("collapses native failures and malformed replies without leaking details", async () => {
    await expect(
      invokeDesktopThreadExportWrite(
        async () => {
          throw new Error("/private/owner/export.json");
        },
        { receipt },
      ),
    ).resolves.toEqual({ status: "rejected", reason: "transport_unavailable" });

    await expect(
      invokeDesktopThreadExportWrite(
        async () => ({
          status: "written",
          filePath: "/private/owner/export.json",
        }),
        { receipt },
      ),
    ).resolves.toEqual({ status: "rejected", reason: "transport_unavailable" });
  });

  test("the sandboxed preload exposes only the fixed decoded bridge method", () => {
    const preload = readFileSync(new URL("./preload.cts", import.meta.url), "utf8");
    expect(preload).toContain("threadExports: {");
    expect(preload).toContain("invokeDesktopThreadExportWrite(");
    expect(preload).toContain("ipcRenderer.invoke(channel, request)");
    expect(preload).not.toContain("threadExports: ipcRenderer");
  });
});
