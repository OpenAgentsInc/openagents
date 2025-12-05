import type { HudMessage } from "./protocol.js";
import { HudClient, type HudClientOptions } from "./client.js";
import { StatusStreamServer, type StatusStreamOptions } from "./status-stream.js";

type HudSender = Pick<HudClient, "send" | "close">;
type StatusBroadcaster = Pick<StatusStreamServer, "broadcast" | "close">;

export interface HudTransportOptions {
  /** Existing HudClient instance or client options */
  client?: HudSender | HudClientOptions;
  /** Status stream instance or options (set enabled to control startup) */
  statusStream?: StatusBroadcaster | (StatusStreamOptions & { enabled?: boolean });
}

const isSender = (value: any): value is HudSender =>
  value && typeof value.send === "function" && typeof value.close === "function";

const isBroadcaster = (value: any): value is StatusBroadcaster =>
  value && typeof value.broadcast === "function" && typeof value.close === "function";

export const resolveStatusStreamEnabled = (options?: { enabled?: boolean }): boolean => {
  if (options?.enabled !== undefined) return options.enabled;
  return process.env.STATUS_STREAM_ENABLED?.toLowerCase() === "true";
};

const createStatusStream = (
  statusStream?: HudTransportOptions["statusStream"],
): StatusBroadcaster | null => {
  if (!statusStream) return null;
  if (isBroadcaster(statusStream)) return statusStream;

  const enabled = resolveStatusStreamEnabled(statusStream);
  if (!enabled) return null;

  const { enabled: _enabled, ...opts } = statusStream;
  const token = opts.token ?? process.env.STATUS_STREAM_TOKEN;
  if (!token) {
    if (opts.verbose) {
      console.warn("[hud-transport] status stream enabled but missing token; skipping start");
    }
    return null;
  }

  return new StatusStreamServer({ ...opts, token });
};

export const createHudTransport = (options?: HudTransportOptions) => {
  const client = isSender(options?.client) ? options!.client : new HudClient(options?.client);
  const statusStream = createStatusStream(options?.statusStream);

  const send = (message: HudMessage): void => {
    client.send(message);
    statusStream?.broadcast(message);
  };

  const close = (): void => {
    client.close();
    statusStream?.close();
  };

  return { client, statusStream, send, close };
};
