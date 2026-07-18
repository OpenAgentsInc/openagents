import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

import { Effect } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { DesktopThreadExportWriteChannel } from "./thread-export-bridge-contract.ts";
import { DesktopThreadExportCreateChannel } from "./thread-export-create-bridge-contract.ts";
import {
  DesktopThreadExportElectronHostUnavailable,
  openDesktopThreadExportElectronHost,
  type DesktopThreadExportElectronHandler,
  type DesktopThreadExportElectronHostDependencies,
  type DesktopThreadExportSaveDialogOptions,
} from "./thread-export-electron-host.ts";
import { DesktopThreadExportMainCompositionUnavailable } from "./thread-export-main-composition.ts";
import { openDesktopThreadEventAuthorityRelationLedger } from "./thread-event-authority-relation-ledger.ts";
import { openDesktopThreadEventSearchReceiptCatalog } from "./thread-event-search-receipt-catalog.ts";

const THREAD = "thread.electron.host.1";
const RUN = "run.electron.host.1";

const intent = {
  schema: "openagents.thread_disclosure_intent.v1",
  intentRef: "intent.electron.host.1",
  idempotencyKey: "idempotency.electron.host.1",
  threadRef: THREAD,
  actorRef: "actor.owner.1",
  expectedVisibilityVersion: { state: "known", value: 12 },
  createdAt: "2026-07-17T22:38:00Z",
  kind: "thread.export.create",
  format: "canonical_event_bundle",
  artifactAudience: { kind: "owner_only" },
} as const;

const confirmedSnapshot = {
  status: { phase: "live", cursor: 12, pendingMutationCount: 0 },
  run: {
    runRef: RUN,
    routeRef: "route.electron.host.1",
    runtime: "openagents_native",
    backend: "hosted",
    status: "completed",
    createdAt: "2026-07-17T22:37:00Z",
    updatedAt: "2026-07-17T22:37:02Z",
    startedAt: "2026-07-17T22:37:01Z",
    completedAt: "2026-07-17T22:37:02Z",
    failedAt: null,
    canceledAt: null,
    version: 3,
  },
  events: [
    {
      eventRef: "event.electron.host.1",
      runRef: RUN,
      sequence: 1,
      eventType: "text.delta",
      summary: "Electron host confirmed evidence",
      status: null,
      artifactRefs: [],
      item: {
        kind: "text",
        messageRef: "message.electron.host.1",
        text: "Electron host confirmed evidence",
      },
      createdAt: "2026-07-17T22:37:02Z",
      version: 4,
    },
    {
      eventRef: "event.electron.host.2",
      runRef: RUN,
      sequence: 2,
      eventType: "text.delta",
      summary: "Electron host replacement evidence",
      status: null,
      artifactRefs: [],
      item: {
        kind: "text",
        messageRef: "message.electron.host.2",
        text: "Electron host replacement evidence",
      },
      createdAt: "2026-07-17T22:37:03Z",
      version: 5,
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

const harness = (overrides: Partial<DesktopThreadExportElectronHostDependencies> = {}) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "oa-thread-export-electron-host-"));
  roots.push(root);
  const userDataDirectory = path.join(root, "user-data");
  const destination = path.join(root, "owner-selected.json");
  const handlers = new Map<string, DesktopThreadExportElectronHandler>();
  const installed: string[] = [];
  const removed: string[] = [];
  const timelineReads: string[] = [];
  const dialogs: DesktopThreadExportSaveDialogOptions[] = [];
  const dependencies: DesktopThreadExportElectronHostDependencies = {
    userDataDirectory,
    snapshotForThread: (threadRef) => {
      timelineReads.push(threadRef);
      return confirmedSnapshot;
    },
    showSaveDialog: async (options) => {
      dialogs.push(options);
      return { canceled: false, filePath: destination };
    },
    handle: (channel, handler) => {
      installed.push(channel);
      handlers.set(channel, handler);
    },
    removeHandler: (channel) => {
      removed.push(channel);
    },
    isTrustedSender: (event) => event === "trusted",
    ...overrides,
  };
  return {
    root,
    userDataDirectory,
    destination,
    handlers,
    installed,
    removed,
    timelineReads,
    dialogs,
    dependencies,
  };
};

describe("Desktop canonical-export Electron host", () => {
  test("runs confirmed create-then-write through fixed IPC and native replacement authority", async () => {
    const value = harness();
    writeFileSync(value.destination, "previous owner export", { mode: 0o600 });
    const privateAuthorityLedger = path.join(
      value.userDataDirectory,
      "thread-exports",
      "authority-relations",
    );
    expect(
      openDesktopThreadEventAuthorityRelationLedger(privateAuthorityLedger).record({
        schema: "openagents.thread_event_authority.v1",
        relationRef: "relation.electron.host.superseded.1",
        threadRef: THREAD,
        eventRef: "event.electron.host.1",
        observedAt: "2026-07-17T22:37:04Z",
        kind: "superseded",
        supersededByEventRef: "event.electron.host.2",
      }),
    ).toMatchObject({ status: "stored" });
    const lifetime = await Effect.runPromise(
      openDesktopThreadExportElectronHost(value.dependencies),
    );
    expect(value.installed).toEqual([
      DesktopThreadExportWriteChannel,
      DesktopThreadExportCreateChannel,
    ]);
    const create = value.handlers.get(DesktopThreadExportCreateChannel);
    const write = value.handlers.get(DesktopThreadExportWriteChannel);
    if (create === undefined || write === undefined) throw new Error("expected both handlers");

    const created = await create("trusted", { intent });
    expect(created.status).toBe("stored");
    if (created.status !== "stored") throw new Error("expected stored export");
    expect(created.receipt.receiptRef).toMatch(/^receipt\.thread_export\.[0-9a-f-]{36}$/);
    expect(Number.isNaN(Date.parse(created.receipt.observedAt))).toBe(false);
    expect(value.timelineReads).toEqual([THREAD]);

    const written = await write("trusted", { receipt: created.receipt });
    expect(written).toMatchObject({ status: "written", replaceAuthorized: true });
    expect(readFileSync(value.destination, "utf8")).toContain("Electron host confirmed evidence");
    expect(readFileSync(value.destination, "utf8")).toContain('"state":"superseded"');
    expect(value.dialogs).toEqual([
      {
        title: "Export canonical thread events",
        buttonLabel: "Export",
        defaultPath: expect.stringMatching(/^openagents-thread\.electron\.host\.1-/),
        filters: [{ name: "JSON", extensions: ["json"] }],
        properties: ["createDirectory", "showOverwriteConfirmation"],
      },
    ]);
    const privateStore = path.join(value.userDataDirectory, "thread-exports", "artifacts");
    const privateReceiptCatalog = path.join(
      value.userDataDirectory,
      "thread-exports",
      "search-receipts",
    );
    expect(existsSync(privateStore)).toBe(true);
    expect(openDesktopThreadEventSearchReceiptCatalog(privateReceiptCatalog).list()).toEqual({
      status: "available",
      receipts: [created.receipt],
    });
    expect(JSON.stringify({ created, written })).not.toContain(privateStore);
    expect(JSON.stringify({ created, written })).not.toContain(privateReceiptCatalog);
    expect(JSON.stringify({ created, written })).not.toContain(privateAuthorityLedger);
    expect(JSON.stringify({ created, written })).not.toContain(value.destination);

    lifetime.close();
    lifetime.close();
    expect(value.removed).toEqual([
      DesktopThreadExportCreateChannel,
      DesktopThreadExportWriteChannel,
    ]);
    await expect(create("trusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
  });

  test("fails untrusted calls before timeline, dialog, or private-store effects", async () => {
    const value = harness();
    const lifetime = await Effect.runPromise(
      openDesktopThreadExportElectronHost(value.dependencies),
    );
    const create = value.handlers.get(DesktopThreadExportCreateChannel);
    const write = value.handlers.get(DesktopThreadExportWriteChannel);
    if (create === undefined || write === undefined) throw new Error("expected both handlers");

    await expect(create("untrusted", { intent })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    await expect(write("untrusted", { receipt: {} })).resolves.toEqual({
      status: "rejected",
      reason: "invalid_request",
    });
    expect(value.timelineReads).toEqual([]);
    expect(value.dialogs).toEqual([]);
    expect(existsSync(value.userDataDirectory)).toBe(false);
    lifetime.close();
  });

  test("maps malformed native dialog output to a path-free refusal", async () => {
    const value = harness({ showSaveDialog: async () => ({ canceled: false, filePath: 42 }) });
    const lifetime = await Effect.runPromise(
      openDesktopThreadExportElectronHost(value.dependencies),
    );
    const create = value.handlers.get(DesktopThreadExportCreateChannel);
    const write = value.handlers.get(DesktopThreadExportWriteChannel);
    if (create === undefined || write === undefined) throw new Error("expected both handlers");
    const created = await create("trusted", { intent });
    if (created.status !== "stored") throw new Error("expected stored export");

    await expect(write("trusted", { receipt: created.receipt })).resolves.toEqual({
      status: "rejected",
      reason: "destination_invalid",
    });
    expect(existsSync(value.destination)).toBe(false);
    lifetime.close();
  });

  test("rejects unsafe userData and rolls back write registration on create failure", async () => {
    const invalid = harness({ userDataDirectory: "relative/private" });
    await expect(
      Effect.runPromise(openDesktopThreadExportElectronHost(invalid.dependencies)),
    ).rejects.toEqual(new DesktopThreadExportElectronHostUnavailable({ stage: "user_data" }));
    expect(invalid.installed).toEqual([]);

    const installed: string[] = [];
    const registered = new Map<string, DesktopThreadExportElectronHandler>();
    const rollback = harness({
      handle: (channel, handler) => {
        installed.push(channel);
        if (channel === DesktopThreadExportCreateChannel) throw new Error("private ipc failure");
        registered.set(channel, handler);
      },
    });
    await expect(
      Effect.runPromise(openDesktopThreadExportElectronHost(rollback.dependencies)),
    ).rejects.toEqual(new DesktopThreadExportMainCompositionUnavailable({ stage: "create" }));
    expect(installed).toEqual([DesktopThreadExportWriteChannel, DesktopThreadExportCreateChannel]);
    expect(registered.has(DesktopThreadExportWriteChannel)).toBe(true);
    expect(rollback.removed).toEqual([DesktopThreadExportWriteChannel]);
  });
});
