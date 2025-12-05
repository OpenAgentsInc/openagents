import type { OrchestratorEvent } from "../agent/orchestrator/types.js";
import { HudClient, type HudClientOptions } from "./client.js";
import type { HudMessage } from "./protocol.js";
import { StatusStreamServer, type StatusStreamOptions } from "./status-stream.js";

export type HudSender = Pick<HudClient, "send" | "close">;
type StatusBroadcaster = Pick<StatusStreamServer, "broadcast" | "close">;

export interface StatusStreamConfig extends StatusStreamOptions {
  enabled?: boolean;
}

export interface HudTransportOptions extends HudClientOptions {
  client?: HudSender | HudClient;
  statusStream?: StatusBroadcaster | StatusStreamConfig;
  eventFilter?: (event: OrchestratorEvent) => HudMessage | null;
  outputSource?: "claude-code" | "minimal" | "orchestrator";
}

export interface HudTransport {
  client: HudSender;
  statusStream: StatusBroadcaster | null;
  sendHudMessage: (message: HudMessage) => void;
  send: (message: HudMessage) => void;
  emitEvent: (event: OrchestratorEvent) => void;
  sendTextOutput: (text: string, source?: "claude-code" | "minimal" | "orchestrator") => void;
  close: () => void;
}

const isSender = (value: unknown): value is HudSender =>
  !!value && typeof (value as HudSender).send === "function" && typeof (value as HudSender).close === "function";

const isBroadcaster = (value: unknown): value is StatusBroadcaster =>
  !!value && typeof (value as StatusBroadcaster).broadcast === "function" && typeof (value as StatusBroadcaster).close === "function";

export const resolveStatusStreamEnabled = (options?: { enabled?: boolean }): boolean => {
  if (options?.enabled !== undefined) return options.enabled;
  return process.env.STATUS_STREAM_ENABLED?.toLowerCase() === "true";
};

const shouldEnableStatusStream = (config?: StatusStreamConfig | StatusBroadcaster): boolean => {
  if (isBroadcaster(config)) return true;
  return resolveStatusStreamEnabled(config);
};

const toStatusStreamOptions = (config?: StatusStreamConfig | StatusBroadcaster): StatusStreamOptions | undefined => {
  if (!config || isBroadcaster(config)) return undefined;
  const { enabled: _enabled, ...rest } = config;
  return rest;
};

const createStatusStream = (
  statusStream?: StatusStreamConfig | StatusBroadcaster
): StatusBroadcaster | null => {
  if (isBroadcaster(statusStream)) return statusStream;

  const enabled = shouldEnableStatusStream(statusStream);
  if (!enabled) return null;

  const options = toStatusStreamOptions(statusStream);
  const token = options?.token ?? process.env.STATUS_STREAM_TOKEN;
  if (!token) {
    if (options?.verbose) {
      console.warn("[hud-transport] status stream enabled but missing token; skipping start");
    }
    return null;
  }

  return new StatusStreamServer({ ...options, token });
};

export const createHudTransport = (options: HudTransportOptions = {}): HudTransport => {
  const {
    statusStream,
    eventFilter,
    outputSource = "claude-code",
    client: providedClient,
    ...clientOptions
  } = options;

  const client: HudSender = isSender(providedClient) ? providedClient : new HudClient(clientOptions);
  const stream = createStatusStream(statusStream);

  const sendHudMessage = (message: HudMessage) => {
    client.send(message);
    stream?.broadcast(message);
  };

  const emitEvent = (event: OrchestratorEvent) => {
    if (!eventFilter) return;
    const hudMessage = eventFilter(event);
    if (hudMessage) {
      sendHudMessage(hudMessage);
    }
  };

  const sendTextOutput = (
    text: string,
    source: "claude-code" | "minimal" | "orchestrator" = outputSource,
  ) => {
    sendHudMessage({ type: "text_output", text, source });
  };

  const close = () => {
    client.close();
    stream?.close();
  };

  return {
    client,
    statusStream: stream,
    sendHudMessage,
    send: sendHudMessage,
    emitEvent,
    sendTextOutput,
    close,
  };
};
