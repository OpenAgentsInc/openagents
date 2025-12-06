/**
 * Desktop Server Tests
 *
 * Bun-native tests for the desktop server HUD message flow.
 */

import { describe, test, expect, beforeAll, afterAll } from "bun:test";
import { DesktopServer } from "./server.js";
import { HudClient } from "../hud/client.js";
import type { HudMessage, TBRunStartMessage, TBTaskCompleteMessage } from "../hud/protocol.js";
import { join } from "node:path";

// Test port to avoid conflicts
const TEST_PORT = 18080;
const TEST_WS_URL = `ws://localhost:${TEST_PORT}/ws`;

describe("DesktopServer", () => {
  let server: DesktopServer;

  beforeAll(() => {
    server = new DesktopServer({
      httpPort: TEST_PORT,
      staticDir: join(import.meta.dir, "../mainview"),
      verbose: false,
    });
    server.start();
  });

  afterAll(() => {
    server.stop();
  });

  test("server starts and is running", () => {
    expect(server.isRunning()).toBe(true);
    expect(server.getHttpPort()).toBe(TEST_PORT);
  });

  test("health endpoint returns status", async () => {
    const response = await fetch(`http://localhost:${TEST_PORT}/api/health`);
    expect(response.ok).toBe(true);

    const health = await response.json();
    expect(health.ok).toBe(true);
    expect(typeof health.clients).toBe("number");
    expect(typeof health.historySize).toBe("number");
  });

  test("inject-hud endpoint accepts valid HUD message", async () => {
    const message: TBRunStartMessage = {
      type: "tb_run_start",
      runId: "test-run-001",
      suiteName: "test-suite",
      suiteVersion: "1.0.0",
      totalTasks: 3,
      taskIds: ["task-1", "task-2", "task-3"],
      timestamp: new Date().toISOString(),
    };

    const response = await fetch(`http://localhost:${TEST_PORT}/api/inject-hud`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(message),
    });

    expect(response.ok).toBe(true);
    const result = await response.json();
    expect(result.ok).toBe(true);
    expect(result.type).toBe("tb_run_start");
  });

  test("inject-hud endpoint rejects invalid message", async () => {
    const response = await fetch(`http://localhost:${TEST_PORT}/api/inject-hud`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ invalid: "message" }),
    });

    expect(response.status).toBe(400);
    const result = await response.json();
    expect(result.ok).toBe(false);
    expect(result.error).toContain("Invalid HUD message");
  });

  test("message history grows with injected messages", async () => {
    const health1 = await (await fetch(`http://localhost:${TEST_PORT}/api/health`)).json();
    const initialSize = health1.historySize;

    // Inject a message
    await fetch(`http://localhost:${TEST_PORT}/api/inject-hud`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        type: "tb_task_complete",
        runId: "test-run",
        taskId: "task-1",
        outcome: "success",
        durationMs: 5000,
        turns: 3,
        tokens: 1000,
      } as TBTaskCompleteMessage),
    });

    const health2 = await (await fetch(`http://localhost:${TEST_PORT}/api/health`)).json();
    expect(health2.historySize).toBeGreaterThan(initialSize);
  });
});

describe("HudClient WebSocket", () => {
  let server: DesktopServer;
  let client: HudClient;

  beforeAll(async () => {
    server = new DesktopServer({
      httpPort: TEST_PORT + 1,
      staticDir: join(import.meta.dir, "../mainview"),
      verbose: false,
    });
    server.start();

    // Give server time to start
    await new Promise((r) => setTimeout(r, 100));

    // HudClient auto-connects in constructor
    client = new HudClient({ url: `ws://localhost:${TEST_PORT + 1}/ws` });
  });

  afterAll(() => {
    client.close();
    server.stop();
  });

  test("client connects to server", async () => {
    // Give time for connection to complete
    await new Promise((r) => setTimeout(r, 300));

    expect(client.getState()).toBe("connected");

    const health = await (await fetch(`http://localhost:${TEST_PORT + 1}/api/health`)).json();
    expect(health.clients).toBeGreaterThanOrEqual(1);
  });

  test("client sends message and server receives", async () => {
    // Set up message handler on server
    const serverMessages: HudMessage[] = [];
    const unsubscribe = server.onMessage((msg) => {
      serverMessages.push(msg);
    });

    // Send message from client
    const message: TBRunStartMessage = {
      type: "tb_run_start",
      runId: "client-test-001",
      suiteName: "client-suite",
      suiteVersion: "1.0.0",
      totalTasks: 1,
      taskIds: ["task-1"],
      timestamp: new Date().toISOString(),
    };

    client.send(message);

    // Wait for message to be received
    await new Promise((r) => setTimeout(r, 200));

    expect(serverMessages.length).toBeGreaterThan(0);
    expect(serverMessages.some((m) => m.type === "tb_run_start")).toBe(true);

    unsubscribe();
  });

  test("client closes cleanly", async () => {
    const health1 = await (await fetch(`http://localhost:${TEST_PORT + 1}/api/health`)).json();
    const initialClients = health1.clients;

    client.close();
    expect(client.getState()).toBe("disconnected");

    // Wait for disconnection to register
    await new Promise((r) => setTimeout(r, 200));

    const health2 = await (await fetch(`http://localhost:${TEST_PORT + 1}/api/health`)).json();
    expect(health2.clients).toBeLessThan(initialClients);
  });
});

describe("TB Message Flow", () => {
  let server: DesktopServer;

  beforeAll(() => {
    server = new DesktopServer({
      httpPort: TEST_PORT + 2,
      staticDir: join(import.meta.dir, "../mainview"),
      verbose: false,
    });
    server.start();
  });

  afterAll(() => {
    server.stop();
  });

  test("TB run sequence flows through server", async () => {
    const messages: HudMessage[] = [];
    const unsubscribe = server.onMessage((msg) => {
      messages.push(msg);
    });

    const baseUrl = `http://localhost:${TEST_PORT + 2}`;

    // Simulate a TB run sequence
    await fetch(`${baseUrl}/api/inject-hud`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        type: "tb_run_start",
        runId: "flow-test-001",
        suiteName: "flow-suite",
        suiteVersion: "1.0.0",
        totalTasks: 2,
        taskIds: ["task-1", "task-2"],
        timestamp: new Date().toISOString(),
      }),
    });

    await fetch(`${baseUrl}/api/inject-hud`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        type: "tb_task_start",
        runId: "flow-test-001",
        taskId: "task-1",
        taskName: "Test Task 1",
        category: "test",
        difficulty: "easy",
        taskIndex: 0,
        totalTasks: 2,
      }),
    });

    await fetch(`${baseUrl}/api/inject-hud`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        type: "tb_task_complete",
        runId: "flow-test-001",
        taskId: "task-1",
        outcome: "success",
        durationMs: 5000,
        turns: 3,
        tokens: 1000,
      }),
    });

    await fetch(`${baseUrl}/api/inject-hud`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        type: "tb_run_complete",
        runId: "flow-test-001",
        passRate: 1.0,
        passed: 1,
        failed: 0,
        timeout: 0,
        error: 0,
        totalDurationMs: 5000,
      }),
    });

    await new Promise((r) => setTimeout(r, 100));

    expect(messages.length).toBe(4);
    expect(messages[0].type).toBe("tb_run_start");
    expect(messages[1].type).toBe("tb_task_start");
    expect(messages[2].type).toBe("tb_task_complete");
    expect(messages[3].type).toBe("tb_run_complete");

    unsubscribe();
  });

  test("new clients receive message history via WebSocket", async () => {
    const baseUrl = `http://localhost:${TEST_PORT + 2}`;

    // Inject a message first
    await fetch(`${baseUrl}/api/inject-hud`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        type: "tb_run_start",
        runId: "history-test-001",
        suiteName: "history-suite",
        suiteVersion: "1.0.0",
        totalTasks: 1,
        taskIds: ["task-1"],
        timestamp: new Date().toISOString(),
      }),
    });

    // Create a raw WebSocket client to receive messages (HudClient is send-only)
    const receivedMessages: HudMessage[] = [];

    const ws = new WebSocket(`ws://localhost:${TEST_PORT + 2}/ws`);

    await new Promise<void>((resolve, reject) => {
      ws.onopen = () => resolve();
      ws.onerror = () => reject(new Error("WebSocket failed to connect"));
      setTimeout(() => reject(new Error("Connection timeout")), 5000);
    });

    ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        receivedMessages.push(msg);
      } catch {
        // Ignore parse errors
      }
    };

    // Wait for history to be sent
    await new Promise((r) => setTimeout(r, 300));

    // Should have received history messages
    expect(receivedMessages.length).toBeGreaterThan(0);

    ws.close();
  });
});
