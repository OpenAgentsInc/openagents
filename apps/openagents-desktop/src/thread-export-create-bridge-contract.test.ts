import { readFileSync } from "node:fs";

import { describe, expect, test } from "vite-plus/test";

import {
  DesktopThreadExportCreateChannel,
  decodeDesktopThreadExportCreateRequest,
  decodeDesktopThreadExportCreateResult,
  invokeDesktopThreadExportCreate,
} from "./thread-export-create-bridge-contract.ts";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.export.create.bridge.1",
  idempotencyKey: "idempotency.export.create.bridge.1",
  threadRef: "thread.export.create.bridge.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 2 },
  createdAt: "2026-07-17T16:21:00.000Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const receipt = {
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.export.create.bridge.1",
  intentRef: intent.intentRef,
  idempotencyKey: intent.idempotencyKey,
  threadRef: intent.threadRef,
  observedAt: "2026-07-17T16:21:01.000Z",
  kind: "thread.export.create" as const,
  result: {
    status: "export_created" as const,
    artifactRef: "artifact.export.create.bridge.1",
    artifactSha256: "c".repeat(64),
    format: "canonical_event_bundle" as const,
    artifactAudience: { kind: "owner_only" as const },
  },
};

describe("Desktop canonical-export creation preload boundary", () => {
  test("admits only an exact owner-only canonical export intent", () => {
    expect(decodeDesktopThreadExportCreateRequest({ intent })).toEqual({ intent });
    expect(
      decodeDesktopThreadExportCreateRequest({
        intent: { ...intent, events: [{ private: true }] },
      }),
    ).toBeNull();
    expect(
      decodeDesktopThreadExportCreateRequest({
        intent: { ...intent, artifactAudience: { kind: "internet_readable" } },
      }),
    ).toBeNull();
    expect(decodeDesktopThreadExportCreateRequest({ intent, receiptRef: "caller" })).toBeNull();
  });

  test("invokes exactly one fixed channel with only the decoded intent", async () => {
    const calls: unknown[] = [];
    const result = await invokeDesktopThreadExportCreate(
      async (channel, request) => {
        calls.push({ channel, request });
        return { status: "stored", receipt };
      },
      { intent },
    );

    expect(result).toEqual({ status: "stored", receipt });
    expect(calls).toEqual([{ channel: DesktopThreadExportCreateChannel, request: { intent } }]);
    expect(JSON.stringify(calls)).not.toContain("events");
    expect(JSON.stringify(calls)).not.toContain("receiptRef");
  });

  test("requires an exact identity-bound ref-only export receipt", () => {
    const request = { intent };
    expect(
      decodeDesktopThreadExportCreateResult({ status: "unchanged", receipt }, request),
    ).toEqual({
      status: "unchanged",
      receipt,
    });
    expect(
      decodeDesktopThreadExportCreateResult(
        { status: "stored", receipt: { ...receipt, threadRef: "thread.other" } },
        request,
      ),
    ).toBeNull();
    expect(
      decodeDesktopThreadExportCreateResult(
        { status: "stored", receipt: { ...receipt, filePath: "/private/export.json" } },
        request,
      ),
    ).toBeNull();
  });

  test("preserves bounded rejection and collapses malformed or native failure", async () => {
    await expect(
      invokeDesktopThreadExportCreate(
        async () => ({ status: "rejected", reason: "invalid_evidence" }),
        {
          intent,
        },
      ),
    ).resolves.toEqual({ status: "rejected", reason: "invalid_evidence" });

    await expect(
      invokeDesktopThreadExportCreate(
        async () => ({ status: "stored", receipt, filePath: "/private/export.json" }),
        {
          intent,
        },
      ),
    ).resolves.toEqual({ status: "rejected", reason: "command_unavailable" });

    await expect(
      invokeDesktopThreadExportCreate(
        async () => {
          throw new Error("private native detail");
        },
        { intent },
      ),
    ).resolves.toEqual({ status: "rejected", reason: "command_unavailable" });
  });

  test("exposes only the fixed create method through sandboxed preload", () => {
    const preload = readFileSync(new URL("./preload.cts", import.meta.url), "utf8");
    expect(preload).toContain("invokeDesktopThreadExportCreate(");
    expect(preload).toContain("create: (value: unknown)");
    expect(preload).not.toContain("threadExports: ipcRenderer");
    expect(preload).not.toContain("create: ipcRenderer.invoke");
  });
});
