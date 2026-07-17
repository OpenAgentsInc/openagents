import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import {
  desktopThreadVisibilityPolicyFileName,
  openDesktopThreadVisibilityPolicyStore,
} from "./thread-visibility-policy-store.ts";

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const makeFile = (): string => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-thread-visibility-"));
  roots.push(root);
  return path.join(root, "private", "visibility.json");
};

const intent = (overrides: Record<string, unknown> = {}) => ({
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.visibility.desktop.1",
  idempotencyKey: "idempotency.visibility.desktop.1",
  threadRef: "thread.desktop.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "unknown" as const, reason: "not_observed" as const },
  createdAt: "2026-07-17T17:43:00.000Z",
  kind: "thread.visibility.set" as const,
  target: {
    audience: { kind: "workspace_members" as const, workspaceRef: "workspace.1" },
    administratorAccess: { kind: "workspace_admins" as const, workspaceRef: "workspace.1" },
  },
  ...overrides,
});

const apply = (
  store: ReturnType<typeof openDesktopThreadVisibilityPolicyStore>,
  value: unknown,
  receiptRef = "receipt.visibility.desktop.1",
) =>
  Effect.runSync(
    store.apply({
      intent: value,
      receiptRef,
      observedAt: "2026-07-17T17:43:01.000Z",
    }),
  );

describe("Desktop thread visibility policy store", () => {
  test("persists an explicit policy and reconstructs it after reopen", () => {
    const file = makeFile();
    const stored = apply(openDesktopThreadVisibilityPolicyStore(file), intent());
    expect(stored).toMatchObject({
      status: "stored",
      receipt: {
        kind: "thread.visibility.set",
        result: {
          status: "visibility_applied",
          visibilityVersion: 1,
          target: {
            audience: { kind: "workspace_members", workspaceRef: "workspace.1" },
            administratorAccess: { kind: "workspace_admins", workspaceRef: "workspace.1" },
          },
        },
      },
    });
    const reopened = openDesktopThreadVisibilityPolicyStore(file);
    expect(Effect.runSync(reopened.load("thread.desktop.1"))).toEqual({
      status: "found",
      visibilityVersion: 1,
      target: intent().target,
      receipt: stored.status === "stored" ? stored.receipt : undefined,
    });
    expect(JSON.stringify(stored)).not.toContain("content");
    if (process.platform !== "win32") {
      expect(statSync(path.dirname(file)).mode & 0o777).toBe(0o700);
      expect(statSync(file).mode & 0o777).toBe(0o600);
    }
  });

  test("advances exact versions and leaves an exact retry unchanged", () => {
    const file = makeFile();
    const store = openDesktopThreadVisibilityPolicyStore(file);
    const firstIntent = intent();
    const first = apply(store, firstIntent);
    const retried = apply(store, firstIntent, "receipt.ignored.on.retry");
    expect(retried).toEqual({
      status: "unchanged",
      receipt: first.status === "stored" ? first.receipt : undefined,
    });

    const second = apply(
      store,
      intent({
        intentRef: "intent.visibility.desktop.2",
        idempotencyKey: "idempotency.visibility.desktop.2",
        expectedVisibilityVersion: { state: "known", value: 1 },
        target: {
          audience: { kind: "internet_readable" },
          administratorAccess: { kind: "none" },
        },
      }),
      "receipt.visibility.desktop.2",
    );
    expect(second).toMatchObject({
      status: "stored",
      receipt: {
        result: { visibilityVersion: 2, target: { audience: { kind: "internet_readable" } } },
      },
    });
    expect(Effect.runSync(store.load("thread.desktop.1"))).toMatchObject({
      status: "found",
      visibilityVersion: 2,
      target: { audience: { kind: "internet_readable" } },
    });
  });

  test("fails closed for stale versions and conflicting identity reuse", () => {
    const store = openDesktopThreadVisibilityPolicyStore(makeFile());
    expect(apply(store, intent()).status).toBe("stored");
    expect(
      apply(
        store,
        intent({
          intentRef: "intent.visibility.desktop.stale",
          idempotencyKey: "idempotency.visibility.desktop.stale",
          expectedVisibilityVersion: { state: "unknown", reason: "not_observed" },
        }),
      ),
    ).toEqual({ status: "rejected", reason: "stale_version" });
    expect(
      apply(
        store,
        intent({
          target: {
            audience: { kind: "internet_readable" },
            administratorAccess: { kind: "none" },
          },
        }),
      ),
    ).toEqual({ status: "rejected", reason: "identity_conflict" });
  });

  test("rejects exports, raw material, contradictory policy, and invalid load refs", () => {
    const store = openDesktopThreadVisibilityPolicyStore(makeFile());
    for (const value of [
      {
        ...intent(),
        kind: "thread.export.create",
        format: "json",
        artifactAudience: { kind: "owner_only" },
      },
      { ...intent(), transcript: "private thread" },
      intent({
        target: {
          audience: { kind: "workspace_members", workspaceRef: "workspace.1" },
          administratorAccess: { kind: "workspace_admins", workspaceRef: "workspace.other" },
        },
      }),
    ]) {
      expect(apply(store, value)).toEqual({ status: "rejected", reason: "invalid_request" });
    }
    expect(Effect.runSync(store.load("../escape"))).toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
  });

  test("does not overwrite corrupt persisted evidence", () => {
    const file = makeFile();
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, "corrupt");
    const before = readFileSync(file, "utf8");
    const store = openDesktopThreadVisibilityPolicyStore(file);
    expect(apply(store, intent())).toEqual({ status: "rejected", reason: "corrupt_store" });
    expect(Effect.runSync(store.load("thread.desktop.1"))).toEqual({
      status: "rejected",
      reason: "corrupt_store",
    });
    expect(readFileSync(file, "utf8")).toBe(before);
  });

  test("derives a path-safe stable owner ledger name", () => {
    expect(desktopThreadVisibilityPolicyFileName("owner:alice")).toMatch(/^[a-f0-9]{64}\.json$/);
    expect(desktopThreadVisibilityPolicyFileName("owner:alice")).toBe(
      desktopThreadVisibilityPolicyFileName("owner:alice"),
    );
    expect(desktopThreadVisibilityPolicyFileName("../owner")).not.toContain("owner");
  });
});
