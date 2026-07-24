/**
 * Minimal NIP-01 mock relay for local load-proof unit runs.
 * Not a production store. Prefer startTestRelay when nostr-effect is present.
 */
import http from "node:http";
import type { AddressInfo } from "node:net";
import { WebSocketServer, type WebSocket } from "ws";
import type { LoadProofNostrEvent } from "./event.js";

export type MockRelayHandle = {
  readonly port: number;
  readonly url: string;
  readonly httpUrl: string;
  stop: () => Promise<void>;
};

export const startMockRelay = async (
  preferredPort = 0,
): Promise<MockRelayHandle> => {
  const events = new Map<string, LoadProofNostrEvent>();
  const server = http.createServer((req, res) => {
    const accept = req.headers.accept ?? "";
    if (accept.includes("application/nostr+json")) {
      const body = JSON.stringify({
        name: "openagents-mock-load-proof",
        description: "SARAH-NR-03 local load-proof mock",
        supported_nips: [1, 11, 42],
        software: "openagents/packages/sarah mock-relay",
        version: "0.1.0",
      });
      res.writeHead(200, {
        "Content-Type": "application/nostr+json",
        "Access-Control-Allow-Origin": "*",
      });
      res.end(body);
      return;
    }
    res.writeHead(200, { "Content-Type": "text/plain" });
    res.end("openagents mock relay\n");
  });

  const wss = new WebSocketServer({ server });

  const matchesFilter = (
    event: LoadProofNostrEvent,
    filter: Record<string, unknown>,
  ): boolean => {
    if (Array.isArray(filter.ids) && !filter.ids.includes(event.id)) {
      return false;
    }
    if (Array.isArray(filter.authors) && !filter.authors.includes(event.pubkey)) {
      return false;
    }
    if (Array.isArray(filter.kinds) && !filter.kinds.includes(event.kind)) {
      return false;
    }
    if (typeof filter.since === "number" && event.created_at < filter.since) {
      return false;
    }
    if (typeof filter.until === "number" && event.created_at > filter.until) {
      return false;
    }
    return true;
  };

  wss.on("connection", (ws: WebSocket) => {
    const challenge = `mock_${Date.now().toString(16)}`;
    ws.send(JSON.stringify(["AUTH", challenge]));

    ws.on("message", (raw) => {
      let msg: unknown;
      try {
        msg = JSON.parse(raw.toString());
      } catch {
        ws.send(JSON.stringify(["NOTICE", "invalid json"]));
        return;
      }
      if (!Array.isArray(msg) || msg.length < 1) {
        ws.send(JSON.stringify(["NOTICE", "invalid message"]));
        return;
      }
      const type = msg[0];
      if (type === "EVENT") {
        const event = msg[1] as LoadProofNostrEvent;
        if (!event || typeof event.id !== "string") {
          ws.send(JSON.stringify(["OK", "", false, "invalid: event"]));
          return;
        }
        events.set(event.id, event);
        ws.send(JSON.stringify(["OK", event.id, true, ""]));
        return;
      }
      if (type === "REQ") {
        const subId = String(msg[1] ?? "");
        const filters = msg.slice(2) as Array<Record<string, unknown>>;
        let sent = 0;
        for (const event of events.values()) {
          if (filters.length === 0 || filters.some((f) => matchesFilter(event, f))) {
            const limit = filters[0]?.limit;
            if (typeof limit === "number" && sent >= limit) break;
            ws.send(JSON.stringify(["EVENT", subId, event]));
            sent += 1;
          }
        }
        ws.send(JSON.stringify(["EOSE", subId]));
        return;
      }
      if (type === "CLOSE") {
        return;
      }
      if (type === "AUTH") {
        // Accept any AUTH response in the mock.
        return;
      }
      ws.send(JSON.stringify(["NOTICE", `unsupported: ${String(type)}`]));
    });
  });

  await new Promise<void>((resolve) => {
    server.listen(preferredPort, "127.0.0.1", () => resolve());
  });
  const address = server.address() as AddressInfo;
  const port = address.port;

  return {
    port,
    url: `ws://127.0.0.1:${port}`,
    httpUrl: `http://127.0.0.1:${port}`,
    stop: async () => {
      await new Promise<void>((resolve) => {
        wss.close(() => {
          server.close(() => resolve());
        });
      });
    },
  };
};
