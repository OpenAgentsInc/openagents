import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { DesktopThreadExportWriteChannel } from "./thread-export-bridge-contract.ts";
import { DesktopThreadExportCreateChannel } from "./thread-export-create-bridge-contract.ts";
import type { DesktopThreadExportCreateMainHandler } from "./thread-export-create-main-handler.ts";
import {
  openDesktopThreadExportHostRuntime,
  type DesktopThreadExportHostRuntimeDependencies,
} from "./thread-export-host-runtime.ts";
import type { DesktopThreadExportMainHandler } from "./thread-export-main-handler.ts";
import { DesktopThreadExportMainCompositionUnavailable } from "./thread-export-main-composition.ts";
import { openDesktopThreadEventAuthorityRelationLedger } from "./thread-event-authority-relation-ledger.ts";
import { openDesktopThreadEventSearchReceiptCatalog } from "./thread-event-search-receipt-catalog.ts";

const THREAD = "thread.host.runtime.1";
const RUN = "run.host.runtime.1";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1" as const,
  intentRef: "intent.host.runtime.1",
  idempotencyKey: "idempotency.host.runtime.1",
  threadRef: THREAD,
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known" as const, value: 9 },
  createdAt: "2026-07-17T22:16:00Z",
  kind: "thread.export.create" as const,
  format: "canonical_event_bundle" as const,
  artifactAudience: { kind: "owner_only" as const },
};

const confirmedSnapshot = {
  status: { phase: "live", cursor: 9, pendingMutationCount: 0 },
  run: {
    runRef: RUN,
    routeRef: "route.host.runtime.1",
    runtime: "openagents_native",
    backend: "hosted",
    status: "completed",
    createdAt: "2026-07-17T22:15:00Z",
    updatedAt: "2026-07-17T22:15:02Z",
    startedAt: "2026-07-17T22:15:01Z",
    completedAt: "2026-07-17T22:15:02Z",
    failedAt: null,
    canceledAt: null,
    version: 2,
  },
  events: [
    {
      eventRef: "event.host.runtime.1",
      runRef: RUN,
      sequence: 1,
      eventType: "text.delta",
      summary: "Host runtime confirmed evidence",
      status: null,
      artifactRefs: [],
      item: {
        kind: "text",
        messageRef: "message.host.runtime.1",
        text: "Host runtime confirmed evidence",
      },
      createdAt: "2026-07-17T22:15:02Z",
      version: 3,
    },
  ],
};

const roots: string[] = [];
afterEach(() => {
  while (roots.length > 0) {
    const root = roots.pop();
    if (root !== undefined) rmSync(root, { recursive: true, force: true });
  }
});

const sha256 = (bytes: Uint8Array): string => createHash("sha256").update(bytes).digest("hex");

const harness = (overrides: Partial<DesktopThreadExportHostRuntimeDependencies> = {}) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-thread-export-host-runtime-"));
  roots.push(root);
  const storeDirectory = path.join(root, "private-store");
  const receiptCatalogDirectory = path.join(root, "search-receipts");
  const authorityLedgerDirectory = path.join(root, "authority-relations");
  const destination = path.join(root, "selected-export.json");
  const channels: string[] = [];
  const closes: string[] = [];
  const reads: string[] = [];
  let writeHandler: DesktopThreadExportMainHandler | undefined;
  let createHandler: DesktopThreadExportCreateMainHandler | undefined;
  const dependencies: DesktopThreadExportHostRuntimeDependencies = {
    storeDirectory,
    receiptCatalogDirectory,
    authorityLedgerDirectory,
    snapshotForThread: (threadRef) => {
      reads.push(threadRef);
      return confirmedSnapshot;
    },
    selectDestination: async () => ({
      status: "selected",
      filePath: destination,
      replaceExisting: false,
    }),
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
    makeReceiptRef: () => "receipt.host.runtime.1",
    observedAt: () => "2026-07-17T22:16:01Z",
    sha256,
    ...overrides,
  };
  return {
    channels,
    closes,
    reads,
    storeDirectory,
    receiptCatalogDirectory,
    authorityLedgerDirectory,
    destination,
    dependencies,
    get writeHandler() {
      return writeHandler;
    },
    get createHandler() {
      return createHandler;
    },
  };
};

describe("Desktop canonical-export host runtime", () => {
  test("runs one exact confirmed create and receipt-only local write", async () => {
    const value = harness();
    const registration = await Effect.runPromise(
      openDesktopThreadExportHostRuntime(value.dependencies),
    );
    expect(value.channels).toEqual([
      DesktopThreadExportWriteChannel,
      DesktopThreadExportCreateChannel,
    ]);
    if (value.createHandler === undefined || value.writeHandler === undefined) {
      throw new Error("expected both handlers");
    }

    const created = await value.createHandler("trusted", { intent });
    expect(value.reads).toEqual([THREAD]);
    expect(created.status).toBe("stored");
    if (created.status !== "stored") throw new Error("expected stored export");

    const written = await value.writeHandler("trusted", { receipt: created.receipt });
    expect(written).toMatchObject({
      status: "written",
      artifactRef:
        created.receipt.result.status === "export_created"
          ? created.receipt.result.artifactRef
          : undefined,
    });
    expect(existsSync(value.destination)).toBe(true);
    const exported = readFileSync(value.destination, "utf8");
    expect(exported).toContain("Host runtime confirmed evidence");
    expect(exported).toContain('\"state\":\"accepted\"');
    expect(JSON.stringify({ created, written })).not.toContain(value.storeDirectory);
    expect(JSON.stringify({ created, written })).not.toContain(value.destination);
    expect(
      openDesktopThreadEventSearchReceiptCatalog(value.receiptCatalogDirectory).list(),
    ).toEqual({ status: "available", receipts: [created.receipt] });

    const retried = await value.createHandler("trusted", { intent });
    expect(retried).toEqual({ status: "unchanged", receipt: created.receipt });
    expect(
      openDesktopThreadEventSearchReceiptCatalog(value.receiptCatalogDirectory).list(),
    ).toEqual({ status: "available", receipts: [created.receipt] });

    registration.close();
    registration.close();
    expect(value.closes).toEqual(["create", "write"]);
  });

  test("rejects untrusted calls before timeline, destination, or store effects", async () => {
    let destinations = 0;
    const value = harness({
      selectDestination: async () => {
        destinations += 1;
        return { status: "cancelled" };
      },
    });
    const registration = await Effect.runPromise(
      openDesktopThreadExportHostRuntime(value.dependencies),
    );
    if (value.createHandler === undefined || value.writeHandler === undefined) {
      throw new Error("expected both handlers");
    }

    await expect(value.createHandler("untrusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    await expect(value.writeHandler("untrusted", { receipt: {} })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    expect(value.reads).toEqual([]);
    expect(destinations).toBe(0);
    expect(existsSync(value.storeDirectory)).toBe(false);
    expect(existsSync(value.receiptCatalogDirectory)).toBe(false);
    registration.close();
  });

  test("rolls back write registration when create registration fails", async () => {
    const value = harness({
      registerCreate: () => {
        throw new Error("/private/native/create-registration");
      },
    });
    await expect(
      Effect.runPromise(openDesktopThreadExportHostRuntime(value.dependencies)),
    ).rejects.toEqual(new DesktopThreadExportMainCompositionUnavailable({ stage: "create" }));
    expect(value.channels).toEqual([DesktopThreadExportWriteChannel]);
    expect(value.closes).toEqual(["write"]);
    expect(value.reads).toEqual([]);
    expect(existsSync(value.storeDirectory)).toBe(false);
    expect(existsSync(value.receiptCatalogDirectory)).toBe(false);
  });

  test("preflights catalog corruption before artifact persistence", async () => {
    const value = harness();
    mkdirSync(value.receiptCatalogDirectory, { recursive: true });
    writeFileSync(
      path.join(value.receiptCatalogDirectory, "canonical-export-receipts.json"),
      "corrupt",
    );
    const registration = await Effect.runPromise(
      openDesktopThreadExportHostRuntime(value.dependencies),
    );
    if (value.createHandler === undefined) throw new Error("expected create handler");

    await expect(value.createHandler("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "persistence_failed",
    });
    expect(existsSync(value.storeDirectory)).toBe(false);
    registration.close();
  });

  test("withholds corrupt terminal authority before artifact or receipt persistence", async () => {
    const value = harness();
    mkdirSync(value.authorityLedgerDirectory, { recursive: true });
    writeFileSync(
      path.join(value.authorityLedgerDirectory, "terminal-authority-relations.json"),
      "private native detail",
    );
    const registration = await Effect.runPromise(
      openDesktopThreadExportHostRuntime(value.dependencies),
    );
    if (value.createHandler === undefined) throw new Error("expected create handler");

    await expect(value.createHandler("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "evidence_unavailable",
    });
    expect(existsSync(value.storeDirectory)).toBe(false);
    expect(existsSync(value.receiptCatalogDirectory)).toBe(false);
    expect(
      openDesktopThreadEventAuthorityRelationLedger(value.authorityLedgerDirectory).listForThread(
        THREAD,
      ).status,
    ).toBe("rejected");
    registration.close();
  });
});
