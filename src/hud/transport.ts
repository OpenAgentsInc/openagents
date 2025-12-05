import type { OrchestratorEvent } from "../agent/orchestrator/types.js";
import type { HudMessage } from "./protocol.js";
import { HudClient, type HudClientOptions } from "./client.js";
import { StatusStreamServer, type StatusStreamOptions } from "./status-stream.js";

export type HudSender = Pick<HudClient, "send" | "close">;
type StatusBroadcaster = Pick<StatusStreamServer, "broadcast" | "close"> & {
  getPort?: () => number | null;
};

export interface HudTransportOptions extends HudClientOptions {
  /** Existing HudClient instance or client options */
  client?: HudSender | HudClientOptions;
  /** Status stream instance or options (set enabled to control startup) */
  statusStream?: StatusBroadcaster | (StatusStreamOptions & { enabled?: boolean });
  /** Optional mapper/filter for orchestrator events */
  eventFilter?: (event: OrchestratorEvent) => HudMessage | null;
  /** Default source for sendTextOutput */
  outputSource?: "claude-code" | "minimal" | "orchestrator";
}

export interface HudTransport {
  client: HudSender;
  statusStream: StatusBroadcaster | null;
  send: (message: HudMessage) => void;
  sendHudMessage: (message: HudMessage) => void;
  emitEvent: (event: OrchestratorEvent) => void;
  sendTextOutput: (
    text: string,
    source?: "claude-code" | "minimal" | "orchestrator",
  ) => void;
  close: () => void;
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

export const createHudTransport = (options: HudTransportOptions = {}): HudTransport => {
  const { client: providedClient, statusStream, eventFilter, outputSource, ...clientOptions } = options;

  const client =
    providedClient !== undefined
      ? isSender(providedClient)
        ? providedClient
        : new HudClient(providedClient)
      : new HudClient(clientOptions);

  const stream = createStatusStream(statusStream);
  const resolvedOutputSource = outputSource ?? "claude-code";

  const send = (message: HudMessage): void => {
    client.send(message);
    stream?.broadcast(message);
  };

  const emitEvent = (event: OrchestratorEvent): void => {
    const hudMessage = eventFilter ? eventFilter(event) : null;
    if (hudMessage) {
      send(hudMessage);
    }
  };

  const sendTextOutput = (
    text: string,
    source: "claude-code" | "minimal" | "orchestrator" = resolvedOutputSource,
  ): void => {
    send({ type: "text_output", text, source });
  };

  const close = (): void => {
    client.close();
    stream?.close();
  };

  return { client, statusStream: stream, send, sendHudMessage: send, emitEvent, sendTextOutput, close };
};
