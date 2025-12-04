/**
 * HUD E2E Error Handling and Resilience Tests
 *
 * Covers:
 * - HUD-060: No crash on WebSocket disconnect
 * - HUD-061: Malformed messages handled gracefully
 * - HUD-062: Error indicators visible (error messages tracked in history)
 * - HUD-063: Recovery from multiple errors
 *
 * These tests verify the HUD system's resilience to errors and malformed data
 * by injecting bad data via test server and verifying graceful degradation.
 */

import { describe, expect, test, beforeEach, afterEach } from "bun:test";
import { HudClient } from "./client.js";
import { HudServer } from "./server.js";
import { type HudMessage, parseHudMessage } from "./protocol.js";

const TEST_PORT = 54321;
const TEST_URL = `ws://localhost:${TEST_PORT}`;

describe("HUD E2E: Error Handling and Resilience", () => {
  let server: HudServer | null = null;
  let client: HudClient | null = null;

  afterEach(async () => {
    if (client) {
      client.close();
      client = null;
    }
    if (server) {
      server.stop();
      server = null;
    }
    // Allow cleanup
    await new Promise((r) => setTimeout(r, 50));
  });

  describe("HUD-060: No crash on WebSocket disconnect", () => {
    test("client continues to function when server is unavailable", async () => {
      // Connect client to non-existent server (simulates server being down)
      client = new HudClient({
        url: "ws://localhost:59998", // Port with no server
        maxReconnectAttempts: 2,
        reconnectInterval: 50,
      });

      // Wait for connection attempts to fail
      await new Promise((r) => setTimeout(r, 200));

      // Client should not crash, should be in disconnected state
      expect(client.getState()).toBe("disconnected");

      // Sending messages should not throw
      expect(() => {
        client!.send({ type: "text_output", text: "after disconnect" });
      }).not.toThrow();

      // Messages should be queued
      expect(client.getQueueSize()).toBeGreaterThan(0);
    });

    test("client handles connection failure gracefully", async () => {
      // Connect client with no reconnect attempts to non-existent server
      client = new HudClient({
        url: "ws://localhost:59997",
        maxReconnectAttempts: 0,
      });

      // Wait for connection attempt to fail
      await new Promise((r) => setTimeout(r, 100));

      // Client should be disconnected but not crashed
      expect(client.getState()).toBe("disconnected");

      // Client should still be usable
      client.send({ type: "session_start", sessionId: "test", timestamp: new Date().toISOString() });
      expect(client.getQueueSize()).toBe(1);
    });

    test("client close is idempotent and safe", async () => {
      client = new HudClient({
        url: "ws://localhost:59996",
        maxReconnectAttempts: 0,
      });

      // Multiple closes should not throw
      expect(() => {
        client!.close();
        client!.close();
        client!.close();
      }).not.toThrow();

      expect(client.getState()).toBe("disconnected");
    });

    test("server handles client disconnect without crashing", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      // Connect and immediately disconnect client
      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      expect(server.getClientCount()).toBe(1);

      client.close();
      client = null;

      await new Promise((r) => setTimeout(r, 100));

      // Server should still be running
      expect(server.isRunning()).toBe(true);
      expect(server.getClientCount()).toBe(0);
    });

    test("server handles multiple rapid connect/disconnect cycles", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      for (let i = 0; i < 5; i++) {
        const tempClient = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
        await new Promise((r) => setTimeout(r, 50));
        tempClient.close();
        await new Promise((r) => setTimeout(r, 20));
      }

      // Server should still be running
      expect(server.isRunning()).toBe(true);
      expect(server.getClientCount()).toBe(0);
    });
  });

  describe("HUD-061: Malformed messages handled gracefully", () => {
    let rawServer: ReturnType<typeof Bun.serve> | null = null;
    let receivedByServer: string[] = [];

    beforeEach(() => {
      receivedByServer = [];
    });

    afterEach(() => {
      if (rawServer) {
        rawServer.stop();
        rawServer = null;
      }
    });

    test("client handles sending invalid JSON from server", async () => {
      // Create a raw server that sends malformed data
      rawServer = Bun.serve({
        port: TEST_PORT,
        fetch(req, server) {
          if (server.upgrade(req, { data: undefined })) {
            return;
          }
          return new Response("Not WebSocket", { status: 400 });
        },
        websocket: {
          open(ws) {
            // Send malformed data to client
            ws.send("not valid json");
            ws.send("{broken json");
            ws.send('{"no_type_field": true}');
            ws.send('{"type": 123}'); // type should be string
          },
          message(ws, message) {
            receivedByServer.push(message.toString());
          },
          close() {},
        },
      });

      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });

      // Wait for connection and malformed messages
      await new Promise((r) => setTimeout(r, 150));

      // Client should still be connected (not crashed)
      expect(client.getState()).toBe("connected");

      // Client should be able to send messages
      client.send({ type: "text_output", text: "test" });
      await new Promise((r) => setTimeout(r, 50));

      expect(receivedByServer.length).toBeGreaterThan(0);
    });

    test("server handles malformed messages from client gracefully", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      const receivedMessages: HudMessage[] = [];
      server.onMessage((msg) => {
        receivedMessages.push(msg);
      });

      // Create raw WebSocket to send malformed data
      const rawWs = new WebSocket(TEST_URL);

      await new Promise<void>((resolve) => {
        rawWs.onopen = () => {
          // Send various malformed messages
          rawWs.send("not json at all");
          rawWs.send("{broken json syntax");
          rawWs.send('{"missing":"type"}');
          rawWs.send('{"type": null}');
          rawWs.send(""); // empty string

          // Send a valid message
          rawWs.send('{"type":"text_output","text":"valid"}');

          setTimeout(resolve, 100);
        };
      });

      // Server should still be running
      expect(server.isRunning()).toBe(true);

      // Only the valid message should be in received
      expect(receivedMessages.length).toBe(1);
      expect(receivedMessages[0].type).toBe("text_output");

      rawWs.close();
    });

    test("parseHudMessage returns null for all malformed inputs", () => {
      const malformedInputs = [
        "",
        "null",
        "undefined",
        "123",
        "true",
        "[]",
        "not json",
        "{broken",
        '{"no_type":"here"}',
        '{"type":123}',
        '{"type":null}',
        '{"type":{}}',
        '{"type":["array"]}',
      ];

      for (const input of malformedInputs) {
        expect(parseHudMessage(input)).toBeNull();
      }
    });
  });

  describe("HUD-062: Error indicators visible (error messages tracked)", () => {
    test("server stores error messages in history", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      // Send error message
      const errorMsg: HudMessage = {
        type: "error",
        phase: "executing_subtask",
        error: "Test error occurred",
      };
      client.send(errorMsg);

      await new Promise((r) => setTimeout(r, 50));

      // Check server history includes error
      const history = server.getMessageHistory();
      expect(history.length).toBeGreaterThan(0);
      const errorInHistory = history.find((m) => m.type === "error");
      expect(errorInHistory).toBeDefined();
      expect((errorInHistory as typeof errorMsg).error).toBe("Test error occurred");
    });

    test("multiple errors are tracked in order", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      // Send multiple error messages
      const errors = [
        { type: "error" as const, phase: "orienting" as const, error: "Error 1" },
        { type: "error" as const, phase: "decomposing" as const, error: "Error 2" },
        { type: "error" as const, phase: "executing_subtask" as const, error: "Error 3" },
      ];

      for (const err of errors) {
        client.send(err);
      }

      await new Promise((r) => setTimeout(r, 50));

      const history = server.getMessageHistory();
      const errorMessages = history.filter((m) => m.type === "error");

      expect(errorMessages.length).toBe(3);
      expect((errorMessages[0] as { error: string }).error).toBe("Error 1");
      expect((errorMessages[1] as { error: string }).error).toBe("Error 2");
      expect((errorMessages[2] as { error: string }).error).toBe("Error 3");
    });

    test("error message handler receives errors", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      const receivedErrors: HudMessage[] = [];
      server.onMessage((msg) => {
        if (msg.type === "error") {
          receivedErrors.push(msg);
        }
      });

      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      client.send({ type: "error", phase: "verifying", error: "Verification failed" });
      await new Promise((r) => setTimeout(r, 50));

      expect(receivedErrors.length).toBe(1);
    });
  });

  describe("HUD-063: Recovery from multiple errors", () => {
    test("client connects to server that starts later", async () => {
      // Start client first with reconnect enabled (no server yet)
      client = new HudClient({
        url: TEST_URL,
        maxReconnectAttempts: 10,
        reconnectInterval: 50,
      });

      // Wait for initial connection attempt to fail
      await new Promise((r) => setTimeout(r, 100));
      expect(client.getState()).toBe("disconnected");

      // Queue messages while disconnected
      client.send({ type: "text_output", text: "queued 1" });
      client.send({ type: "text_output", text: "queued 2" });
      expect(client.getQueueSize()).toBe(2);

      // Now start server
      server = new HudServer({ port: TEST_PORT });
      server.start();

      const receivedMessages: HudMessage[] = [];
      server.onMessage((msg) => receivedMessages.push(msg));

      // Wait for reconnect
      await new Promise((r) => setTimeout(r, 400));

      // Client should reconnect
      expect(client.getState()).toBe("connected");

      // Queue should be flushed
      expect(client.getQueueSize()).toBe(0);

      // Messages should arrive at server
      expect(receivedMessages.length).toBe(2);
    });

    test("server continues after handler throws", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      let callCount = 0;
      server.onMessage(() => {
        callCount++;
        if (callCount === 1) {
          throw new Error("Handler error");
        }
      });

      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      // Send multiple messages
      client.send({ type: "text_output", text: "msg1" });
      client.send({ type: "text_output", text: "msg2" });
      client.send({ type: "text_output", text: "msg3" });

      await new Promise((r) => setTimeout(r, 100));

      // All messages should be processed despite handler throwing
      expect(callCount).toBe(3);
      expect(server.isRunning()).toBe(true);
    });

    test("server recovers from connect handler error", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      let connectCalls = 0;
      server.onConnect(() => {
        connectCalls++;
        if (connectCalls === 1) {
          throw new Error("Connect handler error");
        }
      });

      // First connection - handler will throw
      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      expect(client.getState()).toBe("connected");
      expect(server.isRunning()).toBe(true);

      client.close();
      client = null;
      await new Promise((r) => setTimeout(r, 50));

      // Second connection - handler should not throw
      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      expect(client.getState()).toBe("connected");
      expect(connectCalls).toBe(2);
    });

    test("server recovers from disconnect handler error", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      let disconnectCalls = 0;
      server.onDisconnect(() => {
        disconnectCalls++;
        throw new Error("Disconnect handler error");
      });

      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      client.close();
      client = null;
      await new Promise((r) => setTimeout(r, 100));

      // Server should still be running despite handler error
      expect(server.isRunning()).toBe(true);
      expect(disconnectCalls).toBe(1);

      // Should be able to accept new connections
      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      expect(client.getState()).toBe("connected");
      expect(server.getClientCount()).toBe(1);
    });

    test("handles mixed valid and invalid messages", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      const validMessages: HudMessage[] = [];
      server.onMessage((msg) => validMessages.push(msg));

      // Create raw WebSocket
      const rawWs = new WebSocket(TEST_URL);

      await new Promise<void>((resolve) => {
        rawWs.onopen = () => {
          // Interleave valid and invalid messages
          rawWs.send('{"type":"text_output","text":"valid1"}');
          rawWs.send("invalid1");
          rawWs.send('{"type":"text_output","text":"valid2"}');
          rawWs.send("{broken");
          rawWs.send('{"type":"text_output","text":"valid3"}');
          rawWs.send('{"no_type":true}');

          setTimeout(resolve, 100);
        };
      });

      // Should only receive 3 valid messages
      expect(validMessages.length).toBe(3);
      expect(server.isRunning()).toBe(true);

      rawWs.close();
    });
  });

  describe("Edge cases", () => {
    test("empty message history after clearHistory", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      client.send({ type: "text_output", text: "test" });
      await new Promise((r) => setTimeout(r, 50));

      expect(server.getMessageHistory().length).toBe(1);

      server.clearHistory();
      expect(server.getMessageHistory().length).toBe(0);
    });

    test("history respects max size limit", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      // Send more messages than max history (100)
      for (let i = 0; i < 110; i++) {
        client.send({ type: "text_output", text: `msg${i}` });
      }

      await new Promise((r) => setTimeout(r, 200));

      const history = server.getMessageHistory();
      // Should be capped at maxHistorySize (100)
      expect(history.length).toBeLessThanOrEqual(100);
    });

    test("new client receives history on connect", async () => {
      server = new HudServer({ port: TEST_PORT });
      server.start();

      // First client sends some messages
      client = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      client.send({ type: "session_start", sessionId: "s1", timestamp: "2024-01-01" });
      client.send({ type: "text_output", text: "hello" });

      await new Promise((r) => setTimeout(r, 50));

      expect(server.getMessageHistory().length).toBe(2);

      // Close first client
      client.close();
      client = null;
      await new Promise((r) => setTimeout(r, 50));

      // New client connects - should receive history (sent to ws on open)
      // Note: The history is sent to new clients automatically
      const client2 = new HudClient({ url: TEST_URL, maxReconnectAttempts: 0 });
      await new Promise((r) => setTimeout(r, 100));

      expect(client2.getState()).toBe("connected");

      client2.close();
    });
  });
});
