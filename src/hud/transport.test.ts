import { afterEach, describe, expect, test } from "bun:test";
import type { HudMessage } from "./protocol.js";
import { parseHudMessage } from "./protocol.js";
import { createHudTransport, resolveStatusStreamEnabled } from "./transport.js";
import { orchestratorEventToHudMessage } from "./emit.js";
import type { OrchestratorEvent } from "../agent/orchestrator/types.js";

class FakeClient {
  public readonly messages: HudMessage[] = [];
  public closed = false;

  send(message: HudMessage) {
    this.messages.push(message);
  }

  close() {
    this.closed = true;
  }
}

class FakeStatusStream {
  public readonly messages: HudMessage[] = [];
  public closed = false;

  broadcast(message: HudMessage) {
    this.messages.push(message);
  }

  close() {
    this.closed = true;
  }
}

const SESSION_START: HudMessage = {
  type: "session_start",
  sessionId: "s1",
  timestamp: "now",
};

const restoreEnv = (key: string, value: string | undefined) => {
  if (value === undefined) {
    delete process.env[key];
  } else {
    process.env[key] = value;
  }
};

const originalEnabled = process.env.STATUS_STREAM_ENABLED;
const originalToken = process.env.STATUS_STREAM_TOKEN;

const waitFor = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

afterEach(() => {
  restoreEnv("STATUS_STREAM_ENABLED", originalEnabled);
  restoreEnv("STATUS_STREAM_TOKEN", originalToken);
});

describe("resolveStatusStreamEnabled", () => {
  test("prefers explicit enabled flag", () => {
    process.env.STATUS_STREAM_ENABLED = "true";
    expect(resolveStatusStreamEnabled({ enabled: false })).toBe(false);
  });

  test("falls back to env when flag is undefined", () => {
    process.env.STATUS_STREAM_ENABLED = "true";
    expect(resolveStatusStreamEnabled()).toBe(true);
  });
});

describe("createHudTransport", () => {
  test("sends to both client and status stream when provided", () => {
    const client = new FakeClient();
    const stream = new FakeStatusStream();

    const transport = createHudTransport({ client, statusStream: stream });

    transport.sendHudMessage(SESSION_START);
    transport.send({ type: "text_output", text: "hi", source: "orchestrator" });

    expect(client.messages.map((m) => m.type)).toEqual(["session_start", "text_output"]);
    expect(stream.messages.map((m) => m.type)).toEqual(["session_start", "text_output"]);

    transport.close();
    expect(client.closed).toBe(true);
    expect(stream.closed).toBe(true);
  });

  test("skips status stream when disabled", () => {
    const transport = createHudTransport({ statusStream: { enabled: false, port: 0, token: "secret" } });

    expect(transport.statusStream).toBeNull();
  });

  test("skips status stream when token is missing", () => {
    const transport = createHudTransport({ statusStream: { enabled: true, port: 0 } });

    expect(transport.statusStream).toBeNull();
  });

  test("enables status stream from env defaults", () => {
    process.env.STATUS_STREAM_ENABLED = "true";
    process.env.STATUS_STREAM_TOKEN = "secret";

    const transport = createHudTransport({ statusStream: { port: 0 } });

    expect(transport.statusStream).not.toBeNull();
    transport.close();
  });

  test("emitEvent and sendTextOutput forward through filters", () => {
    const client = new FakeClient();
    const stream = new FakeStatusStream();

    const transport = createHudTransport({
      client,
      statusStream: stream,
      eventFilter: (event) =>
        event.type === "session_start"
          ? { type: "session_start", sessionId: event.sessionId, timestamp: event.timestamp }
          : null,
      outputSource: "orchestrator",
    });

    transport.emitEvent({ type: "session_start", sessionId: "s1", timestamp: "now" });
    transport.sendTextOutput("hello", "orchestrator");

    expect(client.messages.map((m) => m.type)).toEqual(["session_start", "text_output"]);
    expect(stream.messages.map((m) => m.type)).toEqual(["session_start", "text_output"]);
  });

  test("filters orchestrator events before sending", async () => {
    process.env.STATUS_STREAM_ENABLED = "";
    const hudMessages: HudMessage[] = [];

    const server = Bun.serve({
      port: 0,
      fetch: (req, srv) => {
        if (srv.upgrade(req, { data: undefined })) return;
        return new Response("hud", { status: 200 });
      },
      websocket: {
        message: (_ws, msg) => {
          const parsed = typeof msg === "string" ? parseHudMessage(msg) : parseHudMessage(msg.toString());
          if (parsed) hudMessages.push(parsed);
        },
      },
    });

    const task = {
      id: "oa-1",
      title: "Test",
      description: "Test task",
      status: "open" as const,
      priority: 1 as const,
      type: "task" as const,
      labels: [] as const,
      deps: [] as const,
      commits: [] as const,
      comments: [] as const,
      createdAt: "now",
      updatedAt: "now",
    };

    const transport = createHudTransport({
      url: `ws://localhost:${server.port}`,
      eventFilter: orchestratorEventToHudMessage,
    });

    await waitFor(50);

    const events: OrchestratorEvent[] = [
      { type: "lock_acquired", pid: 123, sessionId: "s1" },
      { type: "task_selected", task },
    ];

    events.forEach((event) => transport.emitEvent(event));
    await waitFor(100);

    expect(hudMessages.map((m) => m.type)).toEqual(["task_selected"]);
    transport.close();
    server.stop();
  });

  test("broadcasts to status stream when enabled with token", async () => {
    process.env.STATUS_STREAM_ENABLED = "";
    const hudMessages: HudMessage[] = [];

    const hudServer = Bun.serve({
      port: 0,
      fetch: (req, srv) => {
        if (srv.upgrade(req, { data: undefined })) return;
        return new Response("hud", { status: 200 });
      },
      websocket: {
        message: (_ws, msg) => {
          const parsed = typeof msg === "string" ? parseHudMessage(msg) : parseHudMessage(msg.toString());
          if (parsed) hudMessages.push(parsed);
        },
      },
    });

    const transport = createHudTransport({
      url: `ws://localhost:${hudServer.port}`,
      statusStream: { enabled: true, port: 0, token: "secret" },
      eventFilter: orchestratorEventToHudMessage,
    });

    const port = (transport.statusStream as any)?.getPort?.();
    expect(port).not.toBeNull();
    const statusMessages: HudMessage[] = [];
    const ws = new WebSocket(`ws://localhost:${port}?token=secret`);
    ws.onmessage = (evt) => {
      statusMessages.push(JSON.parse(evt.data as string));
    };

    await waitFor(50);
    transport.emitEvent({
      type: "task_selected",
      task: {
        id: "oa-2",
        title: "Stream Test",
        description: "Transport test",
        status: "open",
        priority: 2,
        type: "task",
        labels: [],
        deps: [],
        commits: [],
        comments: [],
        createdAt: "now",
        updatedAt: "now",
      },
    });

    await waitFor(100);

    expect(hudMessages.map((m) => m.type)).toEqual(["task_selected"]);
    expect(statusMessages.map((m) => m.type)).toEqual(["task_selected"]);

    ws.close();
    transport.close();
    hudServer.stop();
  });
});
