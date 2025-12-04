import { describe, expect, test } from "bun:test";
import type { HudMessage } from "./protocol.js";
import { StatusStreamServer } from "./status-stream.js";

const waitFor = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe("StatusStreamServer", () => {
  test("refuses to start without token", () => {
    const server = new StatusStreamServer({ port: 0 });
    expect(server.getPort()).toBeNull();
    server.close();
  });

  test("starts when token is provided", () => {
    const server = new StatusStreamServer({ port: 0, token: "secret" });
    expect(server.getPort()).not.toBeNull();
    server.close();
  });

  test("rejects unauthorized clients", async () => {
    const server = new StatusStreamServer({ port: 0, token: "secret" });
    const port = server.getPort();
    expect(port).not.toBeNull();

    const url = `ws://localhost:${port}`;
    let closed = false;
    const ws = new WebSocket(url);
    ws.onclose = () => {
      closed = true;
    };

    await waitFor(150);
    expect(closed || ws.readyState === WebSocket.CLOSED).toBe(true);
    server.close();
  });

  test("broadcasts messages to authorized clients", async () => {
    const server = new StatusStreamServer({ port: 0, token: "secret" });
    const port = server.getPort();
    expect(port).not.toBeNull();

    const messages: HudMessage[] = [];
    const ws = new WebSocket(`ws://localhost:${port}?token=secret`);
    ws.onmessage = (evt) => {
      messages.push(JSON.parse(evt.data));
    };

    await waitFor(50);
    server.broadcast({ type: "session_start", sessionId: "s1", timestamp: "now" });
    await waitFor(50);

    expect(messages.length).toBe(1);
    expect(messages[0].type).toBe("session_start");

    ws.close();
    server.close();
  });
});
