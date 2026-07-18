import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { compileThreadExportArtifact } from "@openagentsinc/agent-runtime-schema";
import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";
import { DesktopThreadEventSearchChannel } from "./thread-event-search-bridge-contract.ts";
import {
  DesktopThreadEventSearchHostRuntimeUnavailable,
  openDesktopThreadEventSearchHostRuntime,
  type DesktopThreadEventSearchHostRuntimeDependencies,
} from "./thread-event-search-host-runtime.ts";
import type { DesktopThreadEventSearchMainHandler } from "./thread-event-search-main-handler.ts";
import { openDesktopThreadEventSearchReceiptCatalog } from "./thread-event-search-receipt-catalog.ts";

const now = "2026-07-18T02:42:08.000Z";
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
  intentRef: "intent.search.runtime.1",
  idempotencyKey: "idempotency.search.runtime.1",
  threadRef: "thread.search.runtime.1",
  actorRef: "owner.search.runtime.1",
  expectedVisibilityVersion: { state: "known" as const, value: 4 },
  createdAt: now,
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const openFixture = (overrides: Partial<DesktopThreadEventSearchHostRuntimeDependencies> = {}) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-thread-search-runtime-"));
  roots.push(root);
  const artifactStoreDirectory = path.join(root, "private-artifacts");
  const receiptCatalogDirectory = path.join(root, "private-receipts");
  const channels: string[] = [];
  let handler: DesktopThreadEventSearchMainHandler | undefined;
  let unregisters = 0;
  const dependencies: DesktopThreadEventSearchHostRuntimeDependencies = {
    artifactStoreDirectory,
    receiptCatalogDirectory,
    register: (channel, value) => {
      channels.push(channel);
      handler = value;
      return () => {
        unregisters += 1;
      };
    },
    isTrustedSender: (event) => event === "trusted",
    ...overrides,
  };
  return {
    root,
    artifactStoreDirectory,
    receiptCatalogDirectory,
    channels,
    dependencies,
    get handler() {
      return handler;
    },
    get unregisters() {
      return unregisters;
    },
  };
};

const seedCanonicalExport = (artifactStoreDirectory: string, receiptCatalogDirectory: string) => {
  const built = compileThreadExportArtifact({
    intent,
    events: [
      {
        eventRef: "event.search.runtime.1",
        threadRef: intent.threadRef,
        sequence: 1,
        data: { text: "Original accepted runtime search evidence" },
      },
    ],
    relations: [
      {
        schema: "openagents.thread_event_authority.v1",
        relationRef: "relation.search.runtime.accepted.1",
        threadRef: intent.threadRef,
        eventRef: "event.search.runtime.1",
        observedAt: now,
        kind: "accepted",
      },
    ],
    sha256,
  });
  const persisted = openDesktopThreadExportArtifactStore(artifactStoreDirectory).persist({
    intent,
    compilation: built,
    receiptRef: "receipt.search.runtime.1",
    observedAt: now,
  });
  if (persisted.status === "rejected") throw new Error("expected persisted fixture");
  const recorded = openDesktopThreadEventSearchReceiptCatalog(receiptCatalogDirectory).record(
    persisted.receipt,
  );
  if (recorded.status === "rejected") throw new Error("expected catalog fixture");
  return { built, receipt: persisted.receipt };
};

describe("Desktop persisted canonical-event search host runtime", () => {
  test("searches verified private exports through exactly the fixed bounded handler", async () => {
    const value = openFixture();
    const seeded = seedCanonicalExport(value.artifactStoreDirectory, value.receiptCatalogDirectory);
    const lifetime = await Effect.runPromise(
      openDesktopThreadEventSearchHostRuntime(value.dependencies),
    );
    expect(value.channels).toEqual([DesktopThreadEventSearchChannel]);
    if (value.handler === undefined) throw new Error("expected search handler");

    const result = await value.handler("trusted", { query: "  runtime\nsearch ", limit: 10 });
    expect(result).toMatchObject({
      status: "available",
      projection: {
        query: "runtime search",
        totalMatches: 1,
        results: [
          {
            threadRef: intent.threadRef,
            eventRef: "event.search.runtime.1",
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
    expect(value.unregisters).toBe(1);
  });

  test("fails corrupt catalog state closed without path or native detail", async () => {
    const value = openFixture();
    mkdirSync(value.receiptCatalogDirectory, { recursive: true });
    writeFileSync(
      path.join(value.receiptCatalogDirectory, "canonical-export-receipts.json"),
      "corrupt catalog /private/native/detail",
    );
    const lifetime = await Effect.runPromise(
      openDesktopThreadEventSearchHostRuntime(value.dependencies),
    );
    if (value.handler === undefined) throw new Error("expected search handler");
    const result = await value.handler("trusted", { query: "runtime" });
    expect(result).toEqual({ status: "unavailable", reason: "transport_unavailable" });
    expect(JSON.stringify(result)).not.toContain("private");
    lifetime.close();
  });

  test("rejects untrusted and post-close calls before private acquisition", async () => {
    const value = openFixture();
    const lifetime = await Effect.runPromise(
      openDesktopThreadEventSearchHostRuntime(value.dependencies),
    );
    if (value.handler === undefined) throw new Error("expected search handler");
    await expect(value.handler("untrusted", { query: "runtime" })).resolves.toEqual({
      status: "unavailable",
      reason: "invalid_request",
    });
    lifetime.close();
    await expect(value.handler("trusted", { query: "runtime" })).resolves.toEqual({
      status: "unavailable",
      reason: "invalid_request",
    });
    expect(value.unregisters).toBe(1);
  });

  test("maps registration exceptions to one typed Effect failure", async () => {
    const value = openFixture({
      register: () => {
        throw new Error("/private/native/registration");
      },
    });
    await expect(
      Effect.runPromise(openDesktopThreadEventSearchHostRuntime(value.dependencies)),
    ).rejects.toEqual(
      new DesktopThreadEventSearchHostRuntimeUnavailable({ stage: "registration" }),
    );
  });
});
