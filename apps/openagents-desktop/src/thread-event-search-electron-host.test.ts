import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { compileThreadExportArtifact } from "@openagentsinc/agent-runtime-schema";
import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";
import { DesktopThreadEventSearchChannel } from "./thread-event-search-bridge-contract.ts";
import {
  DesktopThreadEventSearchElectronHostUnavailable,
  openDesktopThreadEventSearchElectronHost,
  type DesktopThreadEventSearchElectronHostDependencies,
} from "./thread-event-search-electron-host.ts";
import { DesktopThreadEventSearchHostRuntimeUnavailable } from "./thread-event-search-host-runtime.ts";
import type { DesktopThreadEventSearchMainHandler } from "./thread-event-search-main-handler.ts";
import { openDesktopThreadEventSearchReceiptCatalog } from "./thread-event-search-receipt-catalog.ts";

const now = "2026-07-18T03:01:26.000Z";
const roots: string[] = [];
const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.search.electron.1",
  idempotencyKey: "idempotency.search.electron.1",
  threadRef: "thread.search.electron.1",
  actorRef: "owner.search.electron.1",
  expectedVisibilityVersion: { state: "known" as const, value: 6 },
  createdAt: now,
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const fixture = (overrides: Partial<DesktopThreadEventSearchElectronHostDependencies> = {}) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-thread-search-electron-"));
  roots.push(root);
  const userDataDirectory = path.join(root, "user-data");
  const handlers = new Map<string, DesktopThreadEventSearchMainHandler>();
  const installed: string[] = [];
  const removed: string[] = [];
  const dependencies: DesktopThreadEventSearchElectronHostDependencies = {
    userDataDirectory,
    handle: (channel, handler) => {
      installed.push(channel);
      handlers.set(channel, handler);
    },
    removeHandler: (channel) => {
      removed.push(channel);
      handlers.delete(channel);
    },
    isTrustedSender: (event) => event === "trusted",
    ...overrides,
  };
  return { root, userDataDirectory, handlers, installed, removed, dependencies };
};

const seedCanonicalExport = (userDataDirectory: string) => {
  const artifactStoreDirectory = path.join(userDataDirectory, "thread-exports", "artifacts");
  const receiptCatalogDirectory = path.join(userDataDirectory, "thread-exports", "search-receipts");
  const built = compileThreadExportArtifact({
    intent,
    events: [
      {
        eventRef: "event.search.electron.1",
        threadRef: intent.threadRef,
        sequence: 1,
        data: { text: "Electron accepted search evidence" },
      },
    ],
    relations: [
      {
        schema: "openagents.thread_event_authority.v1",
        relationRef: "relation.search.electron.accepted.1",
        threadRef: intent.threadRef,
        eventRef: "event.search.electron.1",
        observedAt: now,
        kind: "accepted",
      },
    ],
    sha256,
  });
  const persisted = openDesktopThreadExportArtifactStore(artifactStoreDirectory).persist({
    intent,
    compilation: built,
    receiptRef: "receipt.search.electron.1",
    observedAt: now,
  });
  if (persisted.status === "rejected") throw new Error("expected persisted fixture");
  const recorded = openDesktopThreadEventSearchReceiptCatalog(receiptCatalogDirectory).record(
    persisted.receipt,
  );
  if (recorded.status === "rejected") throw new Error("expected catalog fixture");
  return { artifactStoreDirectory, receiptCatalogDirectory, built, receipt: persisted.receipt };
};

describe("Desktop canonical-event search Electron host", () => {
  test("binds the fixed channel to verified evidence under Desktop user data", async () => {
    const value = fixture();
    const seeded = seedCanonicalExport(value.userDataDirectory);
    const lifetime = await Effect.runPromise(
      openDesktopThreadEventSearchElectronHost(value.dependencies),
    );
    expect(value.installed).toEqual([DesktopThreadEventSearchChannel]);
    const handler = value.handlers.get(DesktopThreadEventSearchChannel);
    if (handler === undefined) throw new Error("expected search handler");

    const result = await handler("trusted", { query: "accepted search" });
    expect(result).toMatchObject({
      status: "available",
      projection: {
        totalMatches: 1,
        results: [
          {
            threadRef: intent.threadRef,
            eventRef: "event.search.electron.1",
            authority: { state: "accepted" },
          },
        ],
      },
    });
    expect(JSON.stringify(result)).not.toContain(seeded.built.encoded);
    expect(JSON.stringify(result)).not.toContain(seeded.receipt.receiptRef);
    expect(JSON.stringify(result)).not.toContain(value.root);

    lifetime.close();
    lifetime.close();
    expect(value.removed).toEqual([DesktopThreadEventSearchChannel]);
    await expect(handler("trusted", { query: "accepted search" })).resolves.toEqual({
      status: "unavailable",
      reason: "invalid_request",
    });
  });

  test("rejects untrusted calls before creating private search state", async () => {
    const value = fixture();
    const lifetime = await Effect.runPromise(
      openDesktopThreadEventSearchElectronHost(value.dependencies),
    );
    const handler = value.handlers.get(DesktopThreadEventSearchChannel);
    if (handler === undefined) throw new Error("expected search handler");
    await expect(handler("untrusted", { query: "accepted search" })).resolves.toEqual({
      status: "unavailable",
      reason: "invalid_request",
    });
    expect(existsSync(value.userDataDirectory)).toBe(false);
    lifetime.close();
  });

  test("rejects unsafe user-data roots before registration", async () => {
    for (const userDataDirectory of [
      "relative/user-data",
      path.parse(process.cwd()).root,
      path.join(process.cwd(), "unsafe\0user-data"),
    ]) {
      const value = fixture({ userDataDirectory });
      await expect(
        Effect.runPromise(openDesktopThreadEventSearchElectronHost(value.dependencies)),
      ).rejects.toEqual(
        new DesktopThreadEventSearchElectronHostUnavailable({ stage: "user_data" }),
      );
      expect(value.installed).toEqual([]);
    }
  });

  test("preserves the typed path-free registration failure", async () => {
    const value = fixture({
      handle: () => {
        throw new Error("/private/native/search-registration");
      },
    });
    await expect(
      Effect.runPromise(openDesktopThreadEventSearchElectronHost(value.dependencies)),
    ).rejects.toEqual(
      new DesktopThreadEventSearchHostRuntimeUnavailable({ stage: "registration" }),
    );
    expect(value.removed).toEqual([]);
  });
});
