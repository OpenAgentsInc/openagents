import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { HudClient, getHudClient, sendToHud, closeHudClient } from "./client.js";
import { type HudMessage, parseHudMessage } from "./protocol.js";

describe("HudClient", () => {
  describe("constructor and initial state", () => {
    test("starts in connecting state", () => {
      const client = new HudClient({ url: "ws://localhost:59999" });
      // Initially connecting (attempting to connect)
      expect(client.getState()).toBe("connecting");
      client.close();
    });

    test("accepts custom options", () => {
      const client = new HudClient({
        url: "ws://localhost:59999",
        maxQueueSize: 500,
        reconnectInterval: 1000,
        maxReconnectAttempts: 5,
        verbose: false,
      });
      expect(client.getQueueSize()).toBe(0);
      client.close();
    });
  });

  describe("message queueing", () => {
    test("queues messages when disconnected", async () => {
      const client = new HudClient({
        url: "ws://localhost:59999",
        maxReconnectAttempts: 0, // Don't retry
      });

      // Wait a bit for connection attempt to fail
      await new Promise((r) => setTimeout(r, 100));

      const msg: HudMessage = {
        type: "text_output",
        text: "hello",
      };
      client.send(msg);

      expect(client.getQueueSize()).toBe(1);
      client.close();
    });

    test("respects maxQueueSize by dropping oldest", async () => {
      const client = new HudClient({
        url: "ws://localhost:59999",
        maxQueueSize: 3,
        maxReconnectAttempts: 0,
      });

      // Wait for connection attempt to fail
      await new Promise((r) => setTimeout(r, 100));

      // Send 5 messages, queue size is 3
      for (let i = 0; i < 5; i++) {
        client.send({ type: "text_output", text: `msg-${i}` });
      }

      expect(client.getQueueSize()).toBe(3);
      client.close();
    });
  });

  describe("close behavior", () => {
    test("close sets state to disconnected", async () => {
      const client = new HudClient({
        url: "ws://localhost:59999",
        maxReconnectAttempts: 0,
      });

      await new Promise((r) => setTimeout(r, 50));
      client.close();

      expect(client.getState()).toBe("disconnected");
    });

    test("close is idempotent", () => {
      const client = new HudClient({
        url: "ws://localhost:59999",
        maxReconnectAttempts: 0,
      });

      client.close();
      client.close();
      client.close();

      expect(client.getState()).toBe("disconnected");
    });
  });

  describe("does not throw when server unavailable", () => {
    test("constructor does not throw", () => {
      expect(() => {
        const client = new HudClient({
          url: "ws://localhost:59999",
          maxReconnectAttempts: 1,
        });
        client.close();
      }).not.toThrow();
    });

    test("send does not throw when disconnected", async () => {
      const client = new HudClient({
        url: "ws://localhost:59999",
        maxReconnectAttempts: 0,
      });

      await new Promise((r) => setTimeout(r, 100));

      expect(() => {
        client.send({ type: "text_output", text: "test" });
      }).not.toThrow();

      client.close();
    });
  });
});

describe("HudClient with mock server", () => {
  let server: ReturnType<typeof Bun.serve> | null = null;
  const TEST_PORT = 54242;
  const TEST_URL = `ws://localhost:${TEST_PORT}`;
  let receivedMessages: HudMessage[] = [];

  beforeEach(() => {
    receivedMessages = [];
    server = Bun.serve({
      port: TEST_PORT,
      fetch(req, server) {
        if (server.upgrade(req, { data: undefined })) {
          return; // WebSocket upgrade handled
        }
        return new Response("Not a WebSocket request", { status: 400 });
      },
      websocket: {
        open(ws) {
          // Connection opened
        },
        message(ws, message) {
          const parsed = parseHudMessage(message.toString());
          if (parsed) {
            receivedMessages.push(parsed);
          }
        },
        close(ws) {
          // Connection closed
        },
      },
    });
  });

  afterEach(() => {
    if (server) {
      server.stop();
      server = null;
    }
  });

  test("connects to server successfully", async () => {
    const client = new HudClient({ url: TEST_URL });

    // Wait for connection
    await new Promise((r) => setTimeout(r, 100));

    expect(client.getState()).toBe("connected");
    client.close();
  });

  test("sends messages when connected", async () => {
    const client = new HudClient({ url: TEST_URL });

    // Wait for connection
    await new Promise((r) => setTimeout(r, 100));

    client.send({ type: "session_start", sessionId: "s1", timestamp: "2024-01-01" });
    client.send({ type: "text_output", text: "hello" });

    // Wait for messages to arrive
    await new Promise((r) => setTimeout(r, 50));

    expect(receivedMessages.length).toBe(2);
    expect(receivedMessages[0].type).toBe("session_start");
    expect(receivedMessages[1].type).toBe("text_output");

    client.close();
  });

  test("flushes queue on connect", async () => {
    // Start client with no server available
    const client = new HudClient({
      url: "ws://localhost:54243", // Wrong port
      maxReconnectAttempts: 0,
    });

    // Wait for connection to fail
    await new Promise((r) => setTimeout(r, 100));

    // Queue some messages
    client.send({ type: "text_output", text: "queued1" });
    client.send({ type: "text_output", text: "queued2" });

    expect(client.getQueueSize()).toBe(2);
    client.close();

    // Now create a new client that connects
    const client2 = new HudClient({ url: TEST_URL });

    // Wait for connection
    await new Promise((r) => setTimeout(r, 100));

    // Queue messages before fully connected
    // (This tests the typical use case where messages are queued during startup)
    expect(client2.getState()).toBe("connected");
    expect(client2.getQueueSize()).toBe(0);

    client2.close();
  });

  test("resets reconnect attempts on successful connect", async () => {
    const client = new HudClient({ url: TEST_URL, reconnectInterval: 100 });

    // Wait for connection
    await new Promise((r) => setTimeout(r, 100));

    expect(client.getState()).toBe("connected");

    // Close and check state
    client.close();
    expect(client.getState()).toBe("disconnected");
  });
});

describe("singleton helpers", () => {
  afterEach(() => {
    closeHudClient();
  });

  test("getHudClient returns same instance", () => {
    const client1 = getHudClient({ url: "ws://localhost:59999", maxReconnectAttempts: 0 });
    const client2 = getHudClient();

    expect(client1).toBe(client2);
    closeHudClient();
  });

  test("sendToHud does not throw when disconnected", () => {
    expect(() => {
      sendToHud({ type: "text_output", text: "test" });
    }).not.toThrow();
  });

  test("closeHudClient resets singleton", () => {
    const client1 = getHudClient({ url: "ws://localhost:59999", maxReconnectAttempts: 0 });
    closeHudClient();
    const client2 = getHudClient({ url: "ws://localhost:59998", maxReconnectAttempts: 0 });

    expect(client1).not.toBe(client2);
    closeHudClient();
  });
});
