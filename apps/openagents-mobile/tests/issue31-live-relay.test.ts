/**
 * Live relay proof for the mobile Issue 31 Nostr client (OMEGA-MOB-31-01).
 *
 * Opt-in, because it needs the network and a real relay:
 *
 * ```sh
 * MOBILE_LIVE_RELAY_URL=wss://openagents-nostr-relay-ezxz4mgdsq-uc.a.run.app \
 *   pnpm --dir apps/openagents-mobile exec vp test --run tests/issue31-live-relay.test.ts
 * ```
 *
 * Every other test in this suite drives `ScriptedWebSocket`, which proves the
 * state machine but never the wire. The physical-device exit on omega#45
 * cannot be satisfied here, but "the client talks to the real relay" can be,
 * and that is the half a scripted socket can never cover.
 *
 * The relay requires NIP-42 and refuses an auth event whose `relay` tag names
 * a host other than its configured public URL, so this drives the real
 * challenge/response rather than assuming an open relay.
 */
import { LocalKeySigner } from "nostr-effect/identity";
import { generateSecretKey } from "nostr-effect/pure";
import { describe, expect, test } from "vite-plus/test";

import type { Issue31WebSocketLike } from "../src/workroom/issue31-nostr-client.ts";

const LIVE_RELAY_URL = process.env.MOBILE_LIVE_RELAY_URL?.trim();

/**
 * Node 24 ships the browser-shaped global `WebSocket`, which is the same
 * runtime shape the client's `webSocket` factory produces on device.
 *
 * It is not *structurally* assignable to `Issue31WebSocketLike`: the interface
 * types its handler arguments as `unknown`, and a `(ev: Event) => void`
 * handler cannot accept `unknown` contravariantly. That is a deliberate
 * property of the interface — it keeps the client from reaching into
 * DOM-specific event fields — so this test adapts rather than widening the
 * client's contract to make an assignment compile.
 */
const openLiveSocket = (url: string): Issue31WebSocketLike => {
  const socket = new WebSocket(url);
  const adapter: Issue31WebSocketLike = {
    onopen: null,
    onmessage: null,
    onerror: null,
    onclose: null,
    send: (data) => socket.send(data),
    close: (code, reason) => socket.close(code, reason),
  };
  socket.onopen = (event) => adapter.onopen?.(event);
  socket.onmessage = (event: MessageEvent) => adapter.onmessage?.({ data: event.data });
  socket.onerror = (event) => adapter.onerror?.(event);
  socket.onclose = (event: CloseEvent) =>
    adapter.onclose?.({ code: event.code, reason: event.reason });
  return adapter;
};

const withSocket = async <A>(
  url: string,
  run: (
    socket: Issue31WebSocketLike,
    next: () => Promise<ReadonlyArray<unknown>>,
  ) => Promise<A>,
): Promise<A> => {
  const socket = openLiveSocket(url);
  const inbox: Array<ReadonlyArray<unknown>> = [];
  let waiting: ((frame: ReadonlyArray<unknown>) => void) | null = null;
  socket.onmessage = ({ data }) => {
    const frame = JSON.parse(String(data)) as ReadonlyArray<unknown>;
    if (waiting) {
      const resolve = waiting;
      waiting = null;
      resolve(frame);
      return;
    }
    inbox.push(frame);
  };
  const next = (): Promise<ReadonlyArray<unknown>> =>
    new Promise((resolve, reject) => {
      const buffered = inbox.shift();
      if (buffered) {
        resolve(buffered);
        return;
      }
      const timer = setTimeout(() => reject(new Error("relay frame timeout")), 15_000);
      waiting = (frame) => {
        clearTimeout(timer);
        resolve(frame);
      };
    });
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("relay connect timeout")), 15_000);
    socket.onopen = () => {
      clearTimeout(timer);
      resolve();
    };
    socket.onerror = (error) => {
      clearTimeout(timer);
      reject(error instanceof Error ? error : new Error("relay socket error"));
    };
  });
  try {
    return await run(socket, next);
  } finally {
    socket.close(1000, "done");
  }
};

describe.skipIf(LIVE_RELAY_URL === undefined || LIVE_RELAY_URL === "")(
  "mobile Issue 31 client against a live relay",
  () => {
    test("authenticates with NIP-42 and round-trips a signed event", async () => {
      const url = LIVE_RELAY_URL as string;
      const device = LocalKeySigner.fromPrivateKey(generateSecretKey());

      await withSocket(url, async (socket, next) => {
        // The relay challenges proactively on open.
        const challengeFrame = await next();
        expect(challengeFrame[0]).toBe("AUTH");
        const challenge = challengeFrame[1] as string;
        expect(typeof challenge).toBe("string");

        const authEvent = await device.signEvent({
          kind: 22242,
          created_at: Math.floor(Date.now() / 1000),
          tags: [
            ["relay", url],
            ["challenge", challenge],
          ],
          content: "",
        });
        socket.send(JSON.stringify(["AUTH", authEvent]));

        const authOk = await next();
        expect(authOk[0]).toBe("OK");
        expect(authOk[2]).toBe(true);

        // A device-signed record the client would later project.
        const marker = `mobile-live-${device.publicKey.slice(0, 12)}`;
        const record = await device.signEvent({
          kind: 1,
          created_at: Math.floor(Date.now() / 1000),
          tags: [["t", "oa-mobile-live"]],
          content: marker,
        });
        socket.send(JSON.stringify(["EVENT", record]));

        const publishOk = await next();
        expect(publishOk[0]).toBe("OK");
        expect(publishOk[2]).toBe(true);

        socket.send(
          JSON.stringify(["REQ", "live", { authors: [device.publicKey], kinds: [1], limit: 5 }]),
        );

        const seen: Array<string> = [];
        for (let frame = await next(); frame[0] !== "EOSE"; frame = await next()) {
          if (frame[0] === "EVENT") {
            seen.push((frame[2] as { content: string }).content);
          }
        }
        expect(seen).toContain(marker);
      });
    }, 40_000);
  },
);
