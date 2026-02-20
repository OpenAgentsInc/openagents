import { describe, expect, it } from "vitest";

import { KhalaSyncClient } from "../src/khalaSyncClient";
import { MemoryWatermarkStore } from "../src/watermarkStore";
import type { WebSocketLike } from "../src/types";

type PhoenixFrame = [string | null, string | null, string, string, unknown];

class FakeSocket implements WebSocketLike {
  readonly sentRaw: string[] = [];

  readyState = 0;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;

  send(data: string): void {
    this.sentRaw.push(data);
  }

  close(): void {
    this.readyState = 3;
    this.onclose?.({} as CloseEvent);
  }

  open(): void {
    this.readyState = 1;
    this.onopen?.({} as Event);
  }

  emitFrame(frame: PhoenixFrame): void {
    this.onmessage?.({ data: JSON.stringify(frame) } as MessageEvent);
  }

  sentFrames(): PhoenixFrame[] {
    return this.sentRaw.map((raw) => JSON.parse(raw) as PhoenixFrame);
  }
}

const waitFor = async (predicate: () => boolean, timeoutMs = 300): Promise<void> => {
  const started = Date.now();
  while (!predicate()) {
    if (Date.now() - started > timeoutMs) {
      throw new Error("timed out waiting for condition");
    }
    await new Promise((resolve) => setTimeout(resolve, 1));
  }
};

const assertFrame = (
  frame: PhoenixFrame | undefined,
  event: string,
): Readonly<Record<string, unknown>> => {
  expect(frame).toBeDefined();
  expect(frame?.[3]).toBe(event);
  const payload = frame?.[4];
  expect(typeof payload).toBe("object");
  expect(payload).not.toBeNull();
  return payload as Readonly<Record<string, unknown>>;
};

const requireSocket = (sockets: FakeSocket[], index: number): FakeSocket => {
  const socket = sockets[index];
  if (!socket) {
    throw new Error(`missing fake socket at index ${index}`);
  }

  return socket;
};

const requireFrame = (socket: FakeSocket, index: number): PhoenixFrame => {
  const frame = socket.sentFrames()[index];
  if (!frame) {
    throw new Error(`missing sent frame at index ${index}`);
  }

  return frame;
};

describe("KhalaSyncClient", () => {
  it("subscribes with resume watermark and applies update batches", async () => {
    const sockets: FakeSocket[] = [];
    const store = new MemoryWatermarkStore({
      "runtime.codex_worker_summaries": 5,
    });

    const client = new KhalaSyncClient({
      url: "wss://openagents.test/sync/socket/websocket",
      tokenProvider: async () => "sync-token-1",
      watermarkStore: store,
      socketFactory: () => {
        const socket = new FakeSocket();
        sockets.push(socket);
        return socket;
      },
    });

    const connectPromise = client.connect();
    await waitFor(() => sockets.length >= 1);

    const socket = requireSocket(sockets, 0);
    socket.open();

    await waitFor(() => socket.sentFrames().length >= 1);
    const joinFrame = requireFrame(socket, 0);
    expect(joinFrame[3]).toBe("phx_join");
    const joinRef = joinFrame[1] as string;

    socket.emitFrame([null, joinRef, "sync:v1", "phx_reply", { status: "ok", response: {} }]);
    await connectPromise;

    const subscribePromise = client.subscribe(["runtime.codex_worker_summaries"]);

    await waitFor(() => socket.sentFrames().length >= 2);
    const subscribeFrame = requireFrame(socket, 1);
    const subscribePayload = assertFrame(subscribeFrame, "sync:subscribe");
    expect(subscribePayload.resume_after).toEqual({
      "runtime.codex_worker_summaries": 5,
    });

    const subscribeRef = subscribeFrame[1] as string;
    socket.emitFrame([
      null,
      subscribeRef,
      "sync:v1",
      "phx_reply",
      {
        status: "ok",
        response: {
          current_watermarks: [{ topic: "runtime.codex_worker_summaries", watermark: 5 }],
        },
      },
    ]);

    await subscribePromise;

    socket.emitFrame([
      null,
      null,
      "sync:v1",
      "sync:update_batch",
      {
        updates: [
          {
            topic: "runtime.codex_worker_summaries",
            doc_key: "worker:1",
            doc_version: 6,
            payload: { worker_id: "worker:1", status: "running" },
            watermark: 6,
            hydration_required: false,
          },
        ],
        replay_complete: true,
      },
    ]);

    expect(client.getWatermark("runtime.codex_worker_summaries")).toBe(6);
    expect((await store.load(["runtime.codex_worker_summaries"]))["runtime.codex_worker_summaries"]).toBe(6);

    const cached = client.getDocument("worker:1");
    expect(cached?.docVersion).toBe(6);
    expect(cached?.watermark).toBe(6);
  });

  it("keeps doc cache monotonic by doc_version", async () => {
    const sockets: FakeSocket[] = [];

    const client = new KhalaSyncClient({
      url: "wss://openagents.test/sync/socket/websocket",
      tokenProvider: async () => "sync-token-2",
      socketFactory: () => {
        const socket = new FakeSocket();
        sockets.push(socket);
        return socket;
      },
    });

    const connectPromise = client.connect();
    await waitFor(() => sockets.length >= 1);
    const socket = requireSocket(sockets, 0);
    socket.open();

    await waitFor(() => socket.sentFrames().length >= 1);
    const joinRef = requireFrame(socket, 0)[1] as string;
    socket.emitFrame([null, joinRef, "sync:v1", "phx_reply", { status: "ok", response: {} }]);
    await connectPromise;

    const subscribePromise = client.subscribe(["runtime.codex_worker_summaries"]);
    await waitFor(() => socket.sentFrames().length >= 2);
    const subscribeRef = requireFrame(socket, 1)[1] as string;
    socket.emitFrame([
      null,
      subscribeRef,
      "sync:v1",
      "phx_reply",
      { status: "ok", response: { current_watermarks: [] } },
    ]);
    await subscribePromise;

    socket.emitFrame([
      null,
      null,
      "sync:v1",
      "sync:update_batch",
      {
        updates: [
          {
            topic: "runtime.codex_worker_summaries",
            doc_key: "worker:2",
            doc_version: 9,
            payload: { status: "running" },
            watermark: 9,
          },
        ],
      },
    ]);

    socket.emitFrame([
      null,
      null,
      "sync:v1",
      "sync:update_batch",
      {
        updates: [
          {
            topic: "runtime.codex_worker_summaries",
            doc_key: "worker:2",
            doc_version: 8,
            payload: { status: "stopping" },
            watermark: 10,
          },
        ],
      },
    ]);

    const cached = client.getDocument("worker:2");
    expect(cached?.docVersion).toBe(9);
    expect(cached?.payload).toEqual({ status: "running" });
    expect(client.getWatermark("runtime.codex_worker_summaries")).toBe(10);
  });

  it("handles stale_cursor and auto-resumes on reconnect", async () => {
    const sockets: FakeSocket[] = [];
    const staleEvents: Array<unknown> = [];
    const store = new MemoryWatermarkStore({ "runtime.codex_worker_summaries": 7 });

    const client = new KhalaSyncClient({
      url: "wss://openagents.test/sync/socket/websocket",
      tokenProvider: async () => "sync-token-3",
      watermarkStore: store,
      reconnectMinDelayMs: 1,
      reconnectMaxDelayMs: 1,
      socketFactory: () => {
        const socket = new FakeSocket();
        sockets.push(socket);
        return socket;
      },
      onStaleCursor: (payload) => staleEvents.push(payload),
    });

    const connectPromise = client.connect();
    await waitFor(() => sockets.length >= 1);
    const firstSocket = requireSocket(sockets, 0);
    firstSocket.open();

    await waitFor(() => firstSocket.sentFrames().length >= 1);
    const firstJoinRef = requireFrame(firstSocket, 0)[1] as string;
    firstSocket.emitFrame([null, firstJoinRef, "sync:v1", "phx_reply", { status: "ok", response: {} }]);
    await connectPromise;

    const subscribePromise = client.subscribe(["runtime.codex_worker_summaries"]);
    await waitFor(() => firstSocket.sentFrames().length >= 2);
    const firstSubscribeFrame = requireFrame(firstSocket, 1);
    const firstSubscribePayload = assertFrame(firstSubscribeFrame, "sync:subscribe");
    expect(firstSubscribePayload.resume_after).toEqual({
      "runtime.codex_worker_summaries": 7,
    });

    const firstSubscribeRef = firstSubscribeFrame[1] as string;
    firstSocket.emitFrame([
      null,
      firstSubscribeRef,
      "sync:v1",
      "phx_reply",
      {
        status: "ok",
        response: {
          current_watermarks: [{ topic: "runtime.codex_worker_summaries", watermark: 7 }],
        },
      },
    ]);
    await subscribePromise;

    firstSocket.emitFrame([
      null,
      null,
      "sync:v1",
      "sync:error",
      {
        code: "stale_cursor",
        full_resync_required: true,
        stale_topics: [{ topic: "runtime.codex_worker_summaries", retention_floor: 8 }],
      },
    ]);

    await waitFor(() => staleEvents.length === 1);
    expect((await store.load(["runtime.codex_worker_summaries"]))["runtime.codex_worker_summaries"]).toBeUndefined();
    expect(client.getWatermark("runtime.codex_worker_summaries")).toBe(0);

    firstSocket.emitFrame([
      null,
      null,
      "sync:v1",
      "sync:update_batch",
      {
        updates: [
          {
            topic: "runtime.codex_worker_summaries",
            doc_key: "worker:3",
            doc_version: 12,
            payload: { status: "running" },
            watermark: 12,
          },
        ],
      },
    ]);

    expect(client.getWatermark("runtime.codex_worker_summaries")).toBe(12);

    firstSocket.close();

    await waitFor(() => sockets.length >= 2);
    const secondSocket = requireSocket(sockets, 1);
    secondSocket.open();

    await waitFor(() => secondSocket.sentFrames().length >= 1);
    const secondJoinRef = requireFrame(secondSocket, 0)[1] as string;
    secondSocket.emitFrame([null, secondJoinRef, "sync:v1", "phx_reply", { status: "ok", response: {} }]);

    await waitFor(() => secondSocket.sentFrames().length >= 2);
    const secondSubscribeFrame = requireFrame(secondSocket, 1);
    const secondSubscribePayload = assertFrame(secondSubscribeFrame, "sync:subscribe");
    expect(secondSubscribePayload.resume_after).toEqual({
      "runtime.codex_worker_summaries": 12,
    });
  });
});
