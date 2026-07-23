/**
 * Live {@link OpencodeTransport} over a spawned `opencode serve` process.
 *
 * Owner-local by construction: the spawned server inherits the developer's
 * opencode config and auth (`~/.config/opencode`, `~/.local/share/opencode`).
 * This module never runs a login flow.
 *
 * Version note: the adapter's `session.next.*` event vocabulary follows the
 * upstream schema; opencode 1.18 serves a blocking
 * `POST /session/{id}/message` whose response carries the complete settled
 * turn (`info` + `parts`). This transport is therefore RESPONSE-DRIVEN: it
 * maps the settled parts onto the adapter's event subset (buffered turn,
 * like Codex exec mode). SSE streaming granularity and live permission
 * relay are follow-ups; `replyToPermission` posts best-effort.
 */

import { spawn, type ChildProcess } from "node:child_process";
import { Effect } from "effect";
import type { OpencodeEvent, OpencodeTransport } from "./opencode-adapter.ts";
import { OpencodeTransportError } from "./opencode-adapter.ts";

export interface LiveOpencodeTransportOptions {
  /** Path to the opencode binary. */
  readonly binaryPath: string;
  /** Directory the server (and its sessions) work in. */
  readonly directory: string;
  /** Startup timeout for the server banner. Default 30000ms. */
  readonly startTimeoutMs?: number;
  /** Per-prompt timeout. Default 600000ms. */
  readonly promptTimeoutMs?: number;
}

interface JsonRecord {
  readonly [key: string]: unknown;
}

const asRecord = (value: unknown): JsonRecord | null =>
  typeof value === "object" && value !== null ? (value as JsonRecord) : null;

const asString = (value: unknown): string => (typeof value === "string" ? value : "");

const num = (value: unknown): number => (typeof value === "number" ? value : 0);

/** Split `provider/model` into the wire's `{providerID, modelID}` pair. */
export const splitOpencodeModel = (
  model: string,
): { readonly providerID: string; readonly modelID: string } | null => {
  const index = model.indexOf("/");
  if (index <= 0 || index === model.length - 1) return null;
  return { providerID: model.slice(0, index), modelID: model.slice(index + 1) };
};

/** Map one settled assistant message (info + parts) onto the adapter's event subset. */
export const opencodeMessageToEvents = (message: JsonRecord): OpencodeEvent[] => {
  const info = asRecord(message.info) ?? {};
  const assistantMessageID = asString(info.id) || "msg_opencode";
  const parts = Array.isArray(message.parts) ? message.parts : [];
  const events: OpencodeEvent[] = [];
  for (const rawPart of parts) {
    const part = asRecord(rawPart);
    if (part === null) continue;
    switch (part.type) {
      case "text":
        events.push({
          type: "session.next.text.delta",
          assistantMessageID,
          textID: asString(part.id) || "text",
          delta: asString(part.text),
        });
        break;
      case "reasoning":
        events.push({
          type: "session.next.reasoning.delta",
          assistantMessageID,
          reasoningID: asString(part.id) || "reasoning",
          delta: asString(part.text),
        });
        break;
      case "tool": {
        const callID = toSafeRefOc(asString(part.callID) || asString(part.id), "call");
        const state = asRecord(part.state) ?? {};
        events.push({
          type: "session.next.tool.called",
          assistantMessageID,
          callID,
          tool: asString(part.tool) || "tool",
          providerExecuted: false,
        });
        if (state.status === "error") {
          events.push({
            type: "session.next.tool.failed",
            callID,
            messageSafe: "opencode reported a tool failure",
            providerExecuted: false,
          });
        } else {
          events.push({
            type: "session.next.tool.success",
            callID,
            providerExecuted: false,
          });
        }
        break;
      }
      case "step-finish": {
        const tokens = asRecord(part.tokens) ?? asRecord(info.tokens) ?? {};
        const cache = asRecord(tokens.cache) ?? {};
        events.push({
          type: "session.next.step.ended",
          assistantMessageID,
          finish: asString(part.reason) || asString(info.finish) || "stop",
          tokens: {
            input: num(tokens.input),
            output: num(tokens.output),
            reasoning: num(tokens.reasoning),
            cache: { read: num(cache.read), write: num(cache.write) },
          },
        });
        break;
      }
      default:
        break;
    }
  }
  return events;
};

const toSafeRefOc = (value: string, fallback: string): string => {
  const cleaned = value
    .replace(/[^A-Za-z0-9._:-]+/g, "-")
    .replace(/^[^A-Za-z0-9]+/, "")
    .slice(0, 200);
  return cleaned === "" ? fallback : cleaned;
};

const transportError = (failureClass: string, detail: string): OpencodeTransportError =>
  new OpencodeTransportError({ failureClass, detail });

/**
 * Spawn `opencode serve` in the given directory and build the live transport
 * over it. The returned transport owns the server process; `shutdown` kills
 * it. Startup resolves when the listening banner names the base URL.
 */
export const makeLiveOpencodeTransport = (
  options: LiveOpencodeTransportOptions,
): Effect.Effect<OpencodeTransport, OpencodeTransportError> =>
  Effect.callback<OpencodeTransport, OpencodeTransportError>((resume) => {
    const child: ChildProcess = spawn(options.binaryPath, ["serve", "--port", "0"], {
      cwd: options.directory,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let output = "";
    let settled = false;
    const startTimer = setTimeout(() => {
      if (settled) return;
      settled = true;
      child.kill("SIGKILL");
      resume(
        Effect.fail(
          transportError(
            "server_start_timeout",
            `no listening banner within ${options.startTimeoutMs ?? 30_000}ms`,
          ),
        ),
      );
    }, options.startTimeoutMs ?? 30_000);

    const promptTimeoutMs = options.promptTimeoutMs ?? 600_000;

    const onData = (chunk: Buffer): void => {
      output += chunk.toString("utf8");
      const match = output.match(/listening on (http:\/\/[0-9.:]+)/);
      if (match === null || settled) return;
      settled = true;
      clearTimeout(startTimer);
      const baseUrl = match[1];

      const requestJson = (
        path: string,
        body: unknown,
      ): Effect.Effect<JsonRecord, OpencodeTransportError> =>
        Effect.tryPromise({
          try: async () => {
            const response = await fetch(`${baseUrl}${path}`, {
              method: "POST",
              headers: { "content-type": "application/json" },
              body: JSON.stringify(body),
              signal: AbortSignal.timeout(promptTimeoutMs),
            });
            if (!response.ok) {
              throw new Error(
                `${path} -> HTTP ${response.status}: ${(await response.text()).slice(0, 300)}`,
              );
            }
            return (await response.json()) as JsonRecord;
          },
          catch: (error) => transportError("request_failed", String(error)),
        });

      const transport: OpencodeTransport = {
        // params.directory is owned by the spawned server's cwd; the model
        // rides each prompt call.
        createSession: (_params) =>
          requestJson("/session", {}).pipe(
            Effect.flatMap((session) => {
              const sessionId = asString(session.id);
              return sessionId === ""
                ? Effect.fail(
                    transportError("session_create_failed", "response carried no session id"),
                  )
                : Effect.succeed({ sessionId });
            }),
          ),
        prompt: (params) =>
          Effect.gen(function* () {
            const model = params.model === undefined ? null : splitOpencodeModel(params.model);
            const body: Record<string, unknown> = {
              parts: [{ type: "text", text: params.prompt }],
              ...(model === null ? {} : { model }),
            };
            const message = yield* requestJson(`/session/${params.sessionId}/message`, body);
            const info = asRecord(message.info) ?? {};
            const error = asRecord(info.error);
            if (error !== null) {
              const data = asRecord(error.data) ?? {};
              return yield* Effect.fail(
                transportError(
                  "provider_error",
                  asString(data.message) || asString(error.name) || "opencode error",
                ),
              );
            }
            const events = opencodeMessageToEvents(message);
            if (events.length === 0) {
              return yield* Effect.fail(
                transportError("empty_turn", "settled message carried no projectable parts"),
              );
            }
            return events;
          }),
        replyToPermission: (params) =>
          requestJson(`/session/${params.sessionId}/permission/${params.requestId}/reply`, {
            response: params.reply,
          }).pipe(Effect.asVoid),
        shutdown: () =>
          Effect.sync(() => {
            child.kill("SIGTERM");
          }),
      };
      resume(Effect.succeed(transport));
    };
    child.stdout?.on("data", onData);
    child.stderr?.on("data", onData);
    child.on("error", (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(startTimer);
      resume(Effect.fail(transportError("spawn_failed", String(error))));
    });
    child.on("exit", (code) => {
      if (settled) return;
      settled = true;
      clearTimeout(startTimer);
      resume(
        Effect.fail(
          transportError(
            "server_exited",
            `opencode serve exited with ${code}: ${output.slice(0, 300)}`,
          ),
        ),
      );
    });
  });
