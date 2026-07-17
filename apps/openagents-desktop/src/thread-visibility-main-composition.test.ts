import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { DesktopThreadVisibilityApplyChannel } from "./thread-visibility-bridge-contract.ts";
import {
  DesktopThreadVisibilityMainCompositionUnavailable,
  openDesktopThreadVisibilityMainComposition,
  type DesktopThreadVisibilityMainCompositionDependencies,
} from "./thread-visibility-main-composition.ts";
import type { DesktopThreadVisibilityMainHandler } from "./thread-visibility-main-handler.ts";

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const makeFile = (): string => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-visibility-composition-"));
  roots.push(root);
  return path.join(root, "private", "visibility.json");
};

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.visibility.composition.1",
  idempotencyKey: "idempotency.visibility.composition.1",
  threadRef: "thread.visibility.composition.1",
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "unknown" as const, reason: "not_observed" as const },
  createdAt: "2026-07-17T18:53:00.000Z",
  kind: "thread.visibility.set" as const,
  target: {
    audience: { kind: "workspace_members" as const, workspaceRef: "workspace.1" },
    administratorAccess: { kind: "workspace_admins" as const, workspaceRef: "workspace.1" },
  },
};

const fixture = (
  policyFile: string,
  overrides: Partial<DesktopThreadVisibilityMainCompositionDependencies> = {},
) => {
  const channels: string[] = [];
  let closes = 0;
  let handler: DesktopThreadVisibilityMainHandler | undefined;
  const dependencies: DesktopThreadVisibilityMainCompositionDependencies = {
    policyFile,
    register: (channel, value) => {
      channels.push(channel);
      handler = value;
      return () => {
        closes += 1;
      };
    },
    isTrustedSender: (event) => event === "trusted",
    makeReceiptRef: () => "receipt.visibility.composition.1",
    observedAt: () => "2026-07-17T18:53:01.000Z",
    ...overrides,
  };
  return {
    channels,
    get closes() {
      return closes;
    },
    get handler() {
      return handler;
    },
    open: () => Effect.runPromise(openDesktopThreadVisibilityMainComposition(dependencies)),
  };
};

describe("Desktop thread-visibility main composition resource", () => {
  test("registers the fixed handler and persists a bounded visibility receipt", async () => {
    const value = fixture(makeFile());
    const registration = await value.open();
    expect(value.channels).toEqual([DesktopThreadVisibilityApplyChannel]);
    if (value.handler === undefined) throw new Error("composition did not register handler");

    await expect(value.handler("trusted", { intent })).resolves.toMatchObject({
      status: "stored",
      receipt: {
        receiptRef: "receipt.visibility.composition.1",
        intentRef: intent.intentRef,
        threadRef: intent.threadRef,
        result: { status: "visibility_applied", visibilityVersion: 1, target: intent.target },
      },
    });
    registration.close();
  });

  test("reopens the same private store and returns an identical exact-retry receipt", async () => {
    const file = makeFile();
    const first = fixture(file);
    const firstRegistration = await first.open();
    if (first.handler === undefined) throw new Error("first handler missing");
    const stored = await first.handler("trusted", { intent });
    firstRegistration.close();

    const reopened = fixture(file, {
      makeReceiptRef: () => "receipt.visibility.composition.retry",
      observedAt: () => "2026-07-17T18:54:00.000Z",
    });
    const reopenedRegistration = await reopened.open();
    if (reopened.handler === undefined) throw new Error("reopened handler missing");
    await expect(reopened.handler("trusted", { intent })).resolves.toEqual(
      stored.status === "stored" ? { status: "unchanged", receipt: stored.receipt } : stored,
    );
    reopenedRegistration.close();
  });

  test("preserves corrupt-store rejection without exposing its private path", async () => {
    const file = makeFile();
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, "corrupt", { encoding: "utf8", flag: "wx" });
    const value = fixture(file);
    const registration = await value.open();
    if (value.handler === undefined) throw new Error("handler missing");
    await expect(value.handler("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "corrupt_store",
    });
    registration.close();
  });

  test("returns a bounded acquisition error when fixed-channel registration fails", async () => {
    const value = fixture(makeFile(), {
      register: () => {
        throw new Error("/private/native/registration");
      },
    });
    await expect(value.open()).rejects.toEqual(
      new DesktopThreadVisibilityMainCompositionUnavailable({ stage: "register" }),
    );
    expect(value.channels).toEqual([]);
  });

  test("closes exactly once and suppresses native cleanup details", async () => {
    let attempts = 0;
    const value = fixture(makeFile(), {
      register: (channel, registered) => {
        expect(channel).toBe(DesktopThreadVisibilityApplyChannel);
        expect(typeof registered).toBe("function");
        return () => {
          attempts += 1;
          throw new Error("/private/native/cleanup");
        };
      },
    });
    const registration = await value.open();
    expect(() => registration.close()).not.toThrow();
    expect(() => registration.close()).not.toThrow();
    expect(attempts).toBe(1);
  });
});
