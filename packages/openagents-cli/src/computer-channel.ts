import WebSocket from "ws";
import { Effect, Layer, Redacted } from "effect";
import * as Context from "effect/Context";

type Frame = [string | null, string | null, string, string, unknown];

export interface ComputerChannelOptions {
  readonly origin: string;
  readonly token: Redacted.Redacted<string>;
  readonly machineId: string;
  readonly hello: unknown;
  readonly heartbeatMillis?: number;
  readonly reconnectBackoffMillis?: number;
  readonly maximumReconnectAttempts?: number;
}

export interface ComputerSocket {
  readonly readyState: number;
  readonly send: (data: string) => void;
  readonly close: () => void;
  readonly on: (event: string, listener: (...args: ReadonlyArray<unknown>) => void) => void;
}

export interface ComputerSocketTransportInterface {
  readonly connect: (url: string) => ComputerSocket;
}

export class ComputerSocketTransport extends Context.Service<
  ComputerSocketTransport,
  ComputerSocketTransportInterface
>()("@openagentsinc/cli/ComputerSocketTransport") {}

export interface ComputerResponder {
  readonly chunk: (text: string) => void;
  readonly exit: (payload: Record<string, unknown>) => void;
  readonly refused: (reason: string, detail: string) => void;
}

export interface ComputerAgentResponder extends ComputerResponder {
  readonly session: (sessionId: string) => void;
}

export interface ComputerChannelHandlers {
  readonly onProbe: (requestId: string) => Promise<unknown>;
  readonly onRun: (
    requestId: string,
    payload: Record<string, unknown>,
    responder: ComputerResponder,
  ) => void;
  readonly onAgent?: (
    requestId: string,
    payload: Record<string, unknown>,
    responder: ComputerAgentResponder,
  ) => void;
  readonly onCancel: (requestId: string) => void;
  readonly onJoined: () => void;
  readonly onEvent: (event: string) => void;
  readonly onClosed: (reason: string) => void;
}

export interface ComputerChannelInterface {
  readonly serve: (
    options: ComputerChannelOptions,
    handlers: ComputerChannelHandlers,
  ) => Effect.Effect<string>;
}

export class ComputerChannel extends Context.Service<ComputerChannel, ComputerChannelInterface>()(
  "@openagentsinc/cli/ComputerChannel",
) {}

const frame = (value: unknown): value is Frame =>
  Array.isArray(value) &&
  value.length === 5 &&
  (value[0] === null || typeof value[0] === "string") &&
  (value[1] === null || typeof value[1] === "string") &&
  typeof value[2] === "string" &&
  typeof value[3] === "string";

const record = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null ? (value as Record<string, unknown>) : {};

const socketUrl = (origin: string, token: string): string =>
  `${origin.replace(/^http/u, "ws").replace(/\/$/u, "")}/controller/socket/websocket?vsn=2.0.0&token=${encodeURIComponent(token)}`;

const reconnectableTransportReason = (reason: string): boolean =>
  reason === "closed" ||
  reason === "phx_close" ||
  reason === "phx_error" ||
  reason === "heartbeat_timeout" ||
  reason === "socket_not_open" ||
  reason.startsWith("error:");

const wait = (milliseconds: number): Promise<void> =>
  new Promise((resolve) => {
    const timer = setTimeout(resolve, milliseconds);
    timer.unref();
  });

const serveConnection = (
  options: ComputerChannelOptions,
  handlers: ComputerChannelHandlers,
  transport: ComputerSocketTransportInterface,
): Promise<string> =>
  new Promise((resolve) => {
    const topic = `computer:${options.machineId}`;
    const joinRef = "1";
    let heartbeatRef = "";
    let reference = 1;
    let heartbeat: NodeJS.Timeout | undefined;
    let heartbeatPending = false;
    let finished = false;
    const socket = transport.connect(socketUrl(options.origin, Redacted.value(options.token)));

    const finish = (reason: string): void => {
      if (finished) return;
      finished = true;
      if (heartbeat !== undefined) clearInterval(heartbeat);
      handlers.onClosed(reason);
      if (socket.readyState === 1 || socket.readyState === 0) {
        socket.close();
      }
      resolve(reason);
    };

    const push = (event: string, payload: unknown, joined = true, forcedRef?: string): void => {
      if (socket.readyState !== 1) return;
      reference += 1;
      const outgoing: Frame = [
        joined ? joinRef : null,
        forcedRef ?? String(reference),
        joined ? topic : "phoenix",
        event,
        payload,
      ];
      socket.send(JSON.stringify(outgoing));
    };

    const responder = (requestId: string): ComputerResponder => ({
      chunk: (text) => push("chunk", { request_id: requestId, text }),
      exit: (payload) => push("exit", { request_id: requestId, ...payload }),
      refused: (reason, detail) => push("refused", { request_id: requestId, reason, detail }),
    });
    const agentResponder = (requestId: string): ComputerAgentResponder => ({
      ...responder(requestId),
      session: (sessionId) => push("session", { request_id: requestId, session_id: sessionId }),
    });

    socket.on("open", () => {
      socket.send(JSON.stringify([joinRef, joinRef, topic, "phx_join", {}]));
      heartbeat = setInterval(() => {
        if (heartbeatPending) {
          finish("heartbeat_timeout");
          return;
        }
        if (socket.readyState !== 1) {
          finish("socket_not_open");
          return;
        }
        heartbeatPending = true;
        const ref = String(++reference);
        heartbeatRef = ref;
        push("heartbeat", {}, false, ref);
      }, options.heartbeatMillis ?? 30_000);
      heartbeat.unref();
    });

    socket.on("message", (data: unknown) => {
      let parsed: unknown;
      try {
        parsed = JSON.parse(String(data));
      } catch {
        return;
      }
      if (!frame(parsed)) return;
      const [, responseRef, responseTopic, event, rawPayload] = parsed;
      if (responseTopic === "phoenix" && event === "phx_reply" && responseRef === heartbeatRef) {
        heartbeatPending = false;
        return;
      }
      if (responseTopic !== topic) return;
      if (event === "phx_reply") {
        const payload = record(rawPayload);
        if (responseRef !== joinRef) return;
        if (payload.status === "ok") {
          handlers.onJoined();
          push("hello", options.hello);
        } else {
          const response = record(payload.response);
          const reason = response.reason;
          finish(`join_refused:${typeof reason === "string" ? reason : "unknown"}`);
        }
        return;
      }
      if (event === "phx_close" || event === "phx_error") {
        finish(event);
        return;
      }
      const payload = record(rawPayload);
      const requestId = payload.request_id;
      if (typeof requestId !== "string") return;
      if (event === "probe") {
        handlers.onEvent(`probe:${requestId.slice(0, 8)}`);
        handlers.onProbe(requestId).then(
          (probe) => push("probe_result", { request_id: requestId, probe }),
          () => push("probe_refused", { request_id: requestId }),
        );
      } else if (event === "run") {
        handlers.onEvent(`run:${requestId.slice(0, 8)}`);
        handlers.onRun(requestId, payload, responder(requestId));
      } else if (event === "agent") {
        handlers.onEvent(`agent:${requestId.slice(0, 8)}`);
        if (handlers.onAgent === undefined) {
          agentResponder(requestId).refused("unsupported", "ACP delegation is unavailable");
        } else {
          handlers.onAgent(requestId, payload, agentResponder(requestId));
        }
      } else if (event === "cancel") {
        handlers.onEvent(`cancel:${requestId.slice(0, 8)}`);
        handlers.onCancel(requestId);
      }
    });
    socket.on("error", (cause: unknown) =>
      finish(`error:${cause instanceof Error ? cause.message : String(cause)}`),
    );
    socket.on("close", () => finish("closed"));
  });

const serveLive = async (
  options: ComputerChannelOptions,
  handlers: ComputerChannelHandlers,
  transport: ComputerSocketTransportInterface,
): Promise<string> => {
  const maximumReconnectAttempts = options.maximumReconnectAttempts ?? 3;
  const backoffMillis = options.reconnectBackoffMillis ?? 250;
  let reconnectAttempts = 0;
  while (true) {
    const reason = await serveConnection(options, handlers, transport);
    const joinReason = reason.startsWith("join_refused:")
      ? reason.slice("join_refused:".length)
      : "";
    const retryable = joinReason === "machine_reconnecting" || reconnectableTransportReason(reason);
    if (!retryable || reconnectAttempts >= maximumReconnectAttempts) {
      return retryable && reconnectAttempts > 0
        ? `${joinReason === "machine_reconnecting" ? "machine_reconnecting" : "transport"}_retry_exhausted:${reason}`
        : reason;
    }
    reconnectAttempts += 1;
    const delay = Math.min(10_000, backoffMillis * 2 ** (reconnectAttempts - 1));
    handlers.onEvent(`reconnect:${reason}:${reconnectAttempts}`);
    await wait(delay);
  }
};

export const computerChannelNodeLayer = Layer.effect(
  ComputerChannel,
  Effect.gen(function* () {
    const transport = yield* ComputerSocketTransport;
    return ComputerChannel.of({
      serve: (options, handlers) => Effect.promise(() => serveLive(options, handlers, transport)),
    });
  }),
);

export const computerSocketNodeLayer = Layer.succeed(ComputerSocketTransport, {
  connect: (url) => new WebSocket(url) as unknown as ComputerSocket,
});
