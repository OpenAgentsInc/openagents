import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test";
import { afterEach, describe, expect, it } from "vite-plus/test";

import {
  AMBIENT_LIVE_ACTIVITY_VERSION,
  AMBIENT_NOTIFICATION_VERSION,
  SHARE_INTAKE_VERSION,
  decodeAmbientNotification,
  decodeLiveActivityShell,
  notificationDeepLink,
} from "../src/ambient/contracts.ts";
import { reconcileLiveActivity, type LiveActivityRuntime } from "../src/ambient/live-activity.ts";
import {
  processAmbientNotificationResponse,
  watchAmbientNotificationResponses,
  type AmbientNotificationResponse,
} from "../src/ambient/notification-ingress.ts";
import {
  claimAmbientNotification,
  initializeAmbientStore,
  listShareInboxItems,
  putShareInboxItem,
  type AmbientSqliteDatabase,
  type AmbientSqliteTransaction,
} from "../src/ambient/store.ts";

const databases: Array<NodeTestDatabase> = [];

const openTestDatabase = (): AmbientSqliteDatabase => {
  const node = new NodeTestDatabase(":memory:");
  databases.push(node);
  const transaction = (database: NodeTestDatabase): AmbientSqliteTransaction => ({
    runAsync: async (sql, ...params) => database.run(sql, ...params),
    getAllAsync: async <Row>(
      sql: string,
      ...params: ReadonlyArray<string | number | null | Uint8Array>
    ) => database.query<Row>(sql).all(...params),
  });
  return {
    ...transaction(node),
    execAsync: async (sql) => node.exec(sql),
    withExclusiveTransactionAsync: async (task) => {
      node.exec("BEGIN IMMEDIATE");
      try {
        await task(transaction(node));
        node.exec("COMMIT");
      } catch (error) {
        node.exec("ROLLBACK");
        throw error;
      }
    },
  };
};

afterEach(() => {
  for (const database of databases.splice(0)) database.close();
});

const notification = (overrides: Record<string, unknown> = {}) => ({
  version: AMBIENT_NOTIFICATION_VERSION,
  notificationId: "notification.44",
  workspaceId: "github:owner",
  aggregateType: "thread" as const,
  aggregateId: "thread.44",
  attentionState: "approval" as const,
  generation: 4,
  issuedAt: 1_786_000_000_000,
  ...overrides,
});

const response = (data: unknown): AmbientNotificationResponse => ({
  actionIdentifier: "default",
  notification: { request: { content: { data } } },
});

describe("ambient notification boundary", () => {
  it("allows only the bounded shell payload and builds the exact workspace + thread link", () => {
    const payload = decodeAmbientNotification(notification());
    expect(notificationDeepLink(payload)).toBe(
      "openagents://work/thread/thread.44?workspaceId=github%3Aowner",
    );
    expect(() => decodeAmbientNotification(notification({ transcript: "private turn" }))).toThrow();
    expect(() => decodeAmbientNotification(notification({ title: "Private project" }))).toThrow();
    expect(() => decodeAmbientNotification(notification({ body: "Secret work" }))).toThrow();
  });

  it("claims a cold-start response once when the SDK also emits it live", async () => {
    const database = openTestDatabase();
    await initializeAmbientStore(database);
    const sameResponse = response(notification());
    const listener: { current: ((value: AmbientNotificationResponse) => void) | null } = {
      current: null,
    };
    const opened: string[] = [];
    const dispositions: string[] = [];
    const stop = watchAmbientNotificationResponses({
      source: {
        defaultActionIdentifier: "default",
        getLastResponse: async () => sameResponse,
        clearLastResponse: async () => undefined,
        addResponseListener: (next) => {
          listener.current = next;
          return { remove: () => undefined };
        },
      },
      workspaceId: "github:owner",
      store: { claim: (id) => claimAmbientNotification(database, id, 10) },
      openUrl: async (url) => {
        opened.push(url);
      },
      onDisposition: (value) => dispositions.push(value),
    });
    listener.current?.(sameResponse);
    await new Promise((resolve) => setTimeout(resolve, 20));
    stop();
    expect(opened).toEqual(["openagents://work/thread/thread.44?workspaceId=github%3Aowner"]);
    expect(dispositions.sort()).toEqual(["duplicate", "opened"]);
  });

  it("claims but does not open a valid payload for another workspace", async () => {
    const ids = new Set<string>();
    const opened: string[] = [];
    const disposition = await processAmbientNotificationResponse({
      response: response(notification({ workspaceId: "github:someone-else" })),
      defaultActionIdentifier: "default",
      workspaceId: "github:owner",
      store: {
        claim: async (id) => {
          if (ids.has(id)) return false;
          ids.add(id);
          return true;
        },
      },
      openUrl: async (url) => {
        opened.push(url);
      },
    });
    expect(disposition).toBe("foreign_workspace");
    expect(opened).toEqual([]);
    expect(ids).toEqual(new Set(["notification.44"]));
  });
});

describe("ambient durable stores and Live Activity projection", () => {
  it("persists share intake across store reopen without turning it into a command", async () => {
    const database = openTestDatabase();
    await initializeAmbientStore(database);
    await putShareInboxItem(database, {
      version: SHARE_INTAKE_VERSION,
      intakeId: "share.44.url",
      kind: "url",
      value: "https://example.com/brief",
      mimeType: "text/uri-list",
      receivedAt: 44,
    });
    await initializeAmbientStore(database);
    expect(await listShareInboxItems(database)).toEqual([
      {
        version: SHARE_INTAKE_VERSION,
        intakeId: "share.44.url",
        kind: "url",
        value: "https://example.com/brief",
        mimeType: "text/uri-list",
        receivedAt: 44,
      },
    ]);
  });

  it("reconciles generations through the native SDK without titles or transcript content", () => {
    const calls: Array<{ operation: string; state: unknown; config?: unknown }> = [];
    const runtime: LiveActivityRuntime = {
      start: (state, config) => {
        calls.push({ operation: "start", state, config });
        return "activity.44";
      },
      update: (_id, state) => calls.push({ operation: "update", state }),
      stop: (_id, state) => calls.push({ operation: "stop", state }),
    };
    const first = decodeLiveActivityShell({
      version: AMBIENT_LIVE_ACTIVITY_VERSION,
      workspaceId: "github:owner",
      aggregateType: "thread",
      aggregateId: "thread.44",
      attentionState: "working",
      status: "running",
      generation: 1,
      updatedAt: 1,
    });
    const current = reconcileLiveActivity(runtime, null, first);
    const next = reconcileLiveActivity(runtime, current, {
      ...first,
      generation: 2,
      status: "waiting",
    });
    const unchanged = reconcileLiveActivity(runtime, next, { ...first, generation: 2 });
    reconcileLiveActivity(runtime, unchanged, null);

    expect(calls.map((call) => call.operation)).toEqual(["start", "update", "stop"]);
    const serialized = JSON.stringify(calls);
    expect(serialized).not.toContain("Private project");
    expect(serialized).not.toContain("transcript");
    expect(serialized).toContain('"title":"OpenAgents"');
    expect(() => decodeLiveActivityShell({ ...first, transcript: "private" })).toThrow();
  });
});
