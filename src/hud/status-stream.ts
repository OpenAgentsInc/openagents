import type { ServerWebSocket } from "bun";
import type { HudMessage } from "./protocol.js";

export interface StatusStreamOptions {
  port?: number | undefined;
  token?: string | undefined;
  verbose?: boolean | undefined;
}

type WsClient = ServerWebSocket<unknown>;

const isAuthorized = (req: Request, token?: string): boolean => {
  if (!token) return true;
  const url = new URL(req.url);
  if (url.searchParams.get("token") === token) return true;
  const auth = req.headers.get("authorization");
  if (!auth) return false;
  const parts = auth.split(" ");
  return parts.length === 2 && parts[0].toLowerCase() === "bearer" && parts[1] === token;
};

/**
 * Lightweight headless status stream for supervisors (pi-mono-style).
 * Accepts WebSocket clients and broadcasts HudMessage payloads.
 */
export class StatusStreamServer {
  private server: ReturnType<typeof Bun.serve> | null = null;
  private readonly port: number;
  private readonly token: string | undefined;
  private readonly verbose: boolean;
  private clients: Set<WsClient> = new Set();

  constructor(options: StatusStreamOptions = {}) {
    this.port = options.port ?? Number(process.env.STATUS_STREAM_PORT ?? 5252);
    this.token = options.token ?? process.env.STATUS_STREAM_TOKEN ?? undefined;
    const envEnabled = process.env.STATUS_STREAM_ENABLED?.toLowerCase() === "true";
    this.verbose = options.verbose ?? false;

    // Only start automatically when env enabled or explicit port provided via options
    if (envEnabled || options.port !== undefined || options.token !== undefined) {
      this.start();
    }
  }

  start(): void {
    if (this.server) return;
    this.server = Bun.serve({
      port: this.port,
      fetch: (req, server) => {
        if (!isAuthorized(req, this.token)) {
          return new Response("Unauthorized", { status: 401 });
        }
        if (server.upgrade(req, { data: undefined })) {
          return; // upgraded
        }
        return new Response("status-stream", { status: 200 });
      },
      websocket: {
        open: (ws) => {
          this.clients.add(ws);
          this.log(`client connected (${this.clients.size})`);
        },
        close: (ws) => {
          this.clients.delete(ws);
          this.log(`client disconnected (${this.clients.size})`);
        },
        message: () => {
          // Supervisors are receive-only; ignore incoming traffic
        },
      },
    });
    this.log(`status stream listening on port ${this.server.port}`);
  }

  broadcast(message: HudMessage): void {
    if (!this.server) return;
    const data = JSON.stringify(message);
    for (const client of this.clients) {
      try {
        client.send(data);
      } catch {
        // Drop failing client
        this.clients.delete(client);
      }
    }
  }

  close(): void {
    if (!this.server) return;
    for (const client of this.clients) {
      try {
        client.close();
      } catch {
        // ignore
      }
    }
    this.clients.clear();
    this.server.stop();
    this.server = null;
  }

  getPort(): number | null {
    return this.server?.port ?? null;
  }

  private log(msg: string): void {
    if (this.verbose) {
      console.log(`[status-stream] ${msg}`);
    }
  }
}
