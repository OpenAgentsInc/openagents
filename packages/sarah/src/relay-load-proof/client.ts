/**
 * Thin WebSocket NIP-01 client for load measurement.
 * Buffers frames so proactive AUTH does not race the waiter.
 */
import WebSocket from "ws";
import type { LoadProofErrorClass } from "./types.js";
import type { LoadProofNostrEvent } from "./event.js";

export type RelayMessage =
  | ["EVENT", string, LoadProofNostrEvent]
  | ["OK", string, boolean, string?]
  | ["EOSE", string]
  | ["NOTICE", string]
  | ["AUTH", string]
  | ["CLOSED", string, string?]
  | readonly [string, ...unknown[]];

type Waiter = {
  predicate: (msg: RelayMessage) => boolean;
  resolve: (msg: RelayMessage) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};

export type LoadProofSocket = {
  readonly url: string;
  send: (payload: unknown) => void;
  waitFor: (
    predicate: (msg: RelayMessage) => boolean,
    timeoutMs: number,
  ) => Promise<RelayMessage>;
  close: () => void;
};

export const connectRelay = (
  url: string,
  connectTimeoutMs: number,
): Promise<LoadProofSocket> =>
  new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    const queue: RelayMessage[] = [];
    const waiters: Waiter[] = [];
    let settled = false;

    const failConnect = (error: Error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject(error);
    };

    const timer = setTimeout(() => {
      try {
        ws.terminate();
      } catch {
        /* ignore */
      }
      failConnect(new Error("connect_timeout"));
    }, connectTimeoutMs);

    ws.on("error", (error) => {
      failConnect(error instanceof Error ? error : new Error(String(error)));
    });

    ws.on("message", (data) => {
      let msg: RelayMessage;
      try {
        msg = JSON.parse(data.toString()) as RelayMessage;
      } catch {
        return;
      }
      const idx = waiters.findIndex((w) => w.predicate(msg));
      if (idx >= 0) {
        const [waiter] = waiters.splice(idx, 1);
        if (!waiter) return;
        clearTimeout(waiter.timer);
        waiter.resolve(msg);
        return;
      }
      queue.push(msg);
    });

    ws.once("open", () => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve({
        url,
        send: (payload) => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify(payload));
          }
        },
        waitFor: (predicate, timeoutMs) => {
          const queuedIdx = queue.findIndex(predicate);
          if (queuedIdx >= 0) {
            const [msg] = queue.splice(queuedIdx, 1);
            return Promise.resolve(msg as RelayMessage);
          }
          return new Promise((res, rej) => {
            const waitTimer = setTimeout(() => {
              const i = waiters.findIndex((w) => w.timer === waitTimer);
              if (i >= 0) waiters.splice(i, 1);
              rej(new Error("timeout"));
            }, timeoutMs);
            waiters.push({
              predicate,
              resolve: res,
              reject: rej,
              timer: waitTimer,
            });
          });
        },
        close: () => {
          for (const waiter of waiters.splice(0)) {
            clearTimeout(waiter.timer);
            waiter.reject(new Error("closed"));
          }
          try {
            ws.terminate();
          } catch {
            ws.close();
          }
        },
      });
    });
  });

export const classifyClientError = (error: unknown): LoadProofErrorClass => {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("connect") || message.includes("ECONNREFUSED")) {
    return "connect_failed";
  }
  if (message.includes("timeout")) return "timeout";
  if (message.includes("ok_false")) return "ok_false";
  if (message.includes("closed")) return "closed";
  if (message.includes("protocol")) return "protocol_error";
  return "other";
};

export const publishEvent = async (
  socket: LoadProofSocket,
  event: LoadProofNostrEvent,
  timeoutMs: number,
): Promise<number> => {
  const started = performance.now();
  socket.send(["EVENT", event]);
  const msg = await socket.waitFor(
    (m) => m[0] === "OK" && m[1] === event.id,
    timeoutMs,
  );
  if (msg[0] !== "OK" || msg[2] !== true) {
    throw new Error("ok_false");
  }
  return performance.now() - started;
};

export const subscribeOnce = async (
  socket: LoadProofSocket,
  filter: Record<string, unknown>,
  timeoutMs: number,
): Promise<number> => {
  const started = performance.now();
  const subId = `sub_${Math.random().toString(16).slice(2, 10)}`;
  socket.send(["REQ", subId, filter]);
  await socket.waitFor((m) => m[0] === "EOSE" && m[1] === subId, timeoutMs);
  socket.send(["CLOSE", subId]);
  return performance.now() - started;
};
