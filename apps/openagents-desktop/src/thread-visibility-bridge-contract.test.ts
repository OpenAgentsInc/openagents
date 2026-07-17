import { readFileSync } from "node:fs";

import { describe, expect, test } from "vite-plus/test";

import {
  DesktopThreadVisibilityApplyChannel,
  decodeDesktopThreadVisibilityApplyRequest,
  decodeDesktopThreadVisibilityApplyResult,
  invokeDesktopThreadVisibilityApply,
} from "./thread-visibility-bridge-contract.ts";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.visibility.bridge.1",
  idempotencyKey: "idempotency.visibility.bridge.1",
  threadRef: "thread.visibility.bridge.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 2 },
  createdAt: "2026-07-17T18:13:00.000Z",
  kind: "thread.visibility.set" as const,
  target: {
    audience: { kind: "named_group" as const, workspaceRef: "workspace.1", groupRef: "group.1" },
    administratorAccess: { kind: "workspace_admins" as const, workspaceRef: "workspace.1" },
  },
};

const receipt = {
  schema: "openagents.thread_disclosure_receipt.v1" as const,
  receiptRef: "receipt.visibility.bridge.1",
  intentRef: intent.intentRef,
  idempotencyKey: intent.idempotencyKey,
  threadRef: intent.threadRef,
  observedAt: "2026-07-17T18:13:01.000Z",
  kind: "thread.visibility.set" as const,
  result: {
    status: "visibility_applied" as const,
    visibilityVersion: 3,
    target: intent.target,
  },
};

describe("Desktop thread visibility apply preload boundary", () => {
  test("admits only an exact ref-only visibility intent", () => {
    expect(decodeDesktopThreadVisibilityApplyRequest({ intent })).toEqual({ intent });
    expect(
      decodeDesktopThreadVisibilityApplyRequest({ intent: { ...intent, transcript: "private" } }),
    ).toBeNull();
    expect(
      decodeDesktopThreadVisibilityApplyRequest({
        intent: {
          ...intent,
          target: {
            ...intent.target,
            audience: { ...intent.target.audience, members: ["actor.private"] },
          },
        },
      }),
    ).toBeNull();
    expect(
      decodeDesktopThreadVisibilityApplyRequest({
        intent: {
          ...intent,
          kind: "thread.export.create",
          format: "json",
          artifactAudience: { kind: "owner_only" },
        },
      }),
    ).toBeNull();
    expect(decodeDesktopThreadVisibilityApplyRequest({ intent, receiptRef: "caller" })).toBeNull();
  });

  test("invokes exactly one fixed channel with only the decoded intent", async () => {
    const calls: unknown[] = [];
    await expect(
      invokeDesktopThreadVisibilityApply(
        async (channel, request) => {
          calls.push({ channel, request });
          return { status: "stored", receipt };
        },
        { intent },
      ),
    ).resolves.toEqual({ status: "stored", receipt });
    expect(calls).toEqual([{ channel: DesktopThreadVisibilityApplyChannel, request: { intent } }]);
    expect(JSON.stringify(calls)).not.toContain("receiptRef");
    expect(JSON.stringify(calls)).not.toContain("members");
  });

  test("requires an identity-bound receipt with the exact requested target", () => {
    const request = { intent };
    expect(
      decodeDesktopThreadVisibilityApplyResult({ status: "unchanged", receipt }, request),
    ).toEqual({ status: "unchanged", receipt });
    expect(
      decodeDesktopThreadVisibilityApplyResult(
        { status: "stored", receipt: { ...receipt, threadRef: "thread.other" } },
        request,
      ),
    ).toBeNull();
    expect(
      decodeDesktopThreadVisibilityApplyResult(
        {
          status: "stored",
          receipt: {
            ...receipt,
            result: {
              ...receipt.result,
              target: {
                audience: { kind: "internet_readable" },
                administratorAccess: { kind: "none" },
              },
            },
          },
        },
        request,
      ),
    ).toBeNull();
    expect(
      decodeDesktopThreadVisibilityApplyResult(
        { status: "stored", receipt, publishedUrl: "https://example.invalid/thread" },
        request,
      ),
    ).toBeNull();
  });

  test("preserves bounded rejection and collapses malformed or native failure", async () => {
    await expect(
      invokeDesktopThreadVisibilityApply(
        async () => ({ status: "rejected", reason: "stale_version" }),
        { intent },
      ),
    ).resolves.toEqual({ status: "rejected", reason: "stale_version" });
    await expect(
      invokeDesktopThreadVisibilityApply(
        async () => ({ status: "stored", receipt, nativePath: "/private" }),
        { intent },
      ),
    ).resolves.toEqual({ status: "rejected", reason: "command_unavailable" });
    await expect(
      invokeDesktopThreadVisibilityApply(
        async () => {
          throw new Error("private native detail");
        },
        { intent },
      ),
    ).resolves.toEqual({ status: "rejected", reason: "command_unavailable" });
  });

  test("exposes only the fixed apply method through sandboxed preload", () => {
    const preload = readFileSync(new URL("./preload.cts", import.meta.url), "utf8");
    expect(preload).toContain("invokeDesktopThreadVisibilityApply(");
    expect(preload).toContain("threadVisibility: {");
    expect(preload).toContain("apply: (value: unknown)");
    expect(preload).not.toContain("threadVisibility: ipcRenderer");
    expect(preload).not.toContain("apply: ipcRenderer.invoke");
  });
});
