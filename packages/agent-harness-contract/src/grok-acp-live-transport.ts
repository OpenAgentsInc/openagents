/**
 * Live {@link AcpTransport} over `grok agent stdio` (Agent Client Protocol,
 * JSON-RPC on stdin/stdout).
 *
 * Owner-local: authenticates with the developer's cached grok login
 * (`cached_token`, or `xai.api_key` when XAI_API_KEY is set). Never runs a
 * login flow. Buffered turns: `session/prompt` blocks until the turn
 * settles while `session/update` notifications stream; promptTurn returns
 * the ordered {@link AcpAdapterEvent} projection of both.
 */

import { spawn, type ChildProcess } from "node:child_process";
import { createInterface } from "node:readline";
import { Effect } from "effect";
import type { AcpAdapterEvent, AcpTransport } from "./acp-adapter.ts";
import { HarnessTurnError } from "./session.ts";

export interface LiveGrokAcpTransportOptions {
  /** Path to the grok binary. */
  readonly binaryPath: string;
  /** Working directory for the ACP session. */
  readonly cwd: string;
  /** Handshake timeout. Default 30000ms. */
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

/** Bound wire identifiers onto the Khala safe-ref charset. */
const toSafeRef = (value: string, fallback: string): string => {
  const cleaned = value
    .replace(/[^A-Za-z0-9._:-]+/g, "-")
    .replace(/^[^A-Za-z0-9]+/, "")
    .slice(0, 200);
  return cleaned === "" ? fallback : cleaned;
};

const turnError = (failureClass: string, detail: string): HarnessTurnError =>
  new HarnessTurnError({
    harnessId: "grok",
    sessionId: "grok-acp",
    turnId: "unknown",
    failureClass,
    detail,
  });

/**
 * Spawn `grok agent stdio`, perform initialize/authenticate/session-new, and
 * return the live transport. The transport owns the child process.
 */
export const makeLiveGrokAcpTransport = (
  options: LiveGrokAcpTransportOptions,
): Effect.Effect<AcpTransport, HarnessTurnError> =>
  Effect.tryPromise({
    try: async () => {
      const child: ChildProcess = spawn(
        options.binaryPath,
        ["--no-auto-update", "agent", "stdio"],
        { cwd: options.cwd, stdio: ["pipe", "pipe", "pipe"] },
      );
      const rl = createInterface({ input: child.stdout! });
      const pending = new Map<
        number,
        { resolve: (value: JsonRecord) => void; reject: (error: Error) => void }
      >();
      let nextId = 1;
      // Updates observed since the last drain, mapped onto adapter events.
      let updates: AcpAdapterEvent[] = [];
      const startedCalls = new Map<string, string>();

      rl.on("line", (line) => {
        let message: JsonRecord | null;
        try {
          message = asRecord(JSON.parse(line));
        } catch {
          return;
        }
        if (message === null) return;
        if (message.method === "session/request_permission" && message.id !== undefined) {
          const params = asRecord(message.params) ?? {};
          const options = Array.isArray(params.options) ? params.options : [];
          const ids = options
            .map((option) => asString(asRecord(option)?.optionId))
            .filter((id) => id !== "");
          const chosen =
            ids.find((id) => id.includes("allow") && id.includes("once")) ??
            ids.find((id) => id.includes("allow")) ??
            ids[0] ??
            "allow-once";
          child.stdin!.write(
            `${JSON.stringify({ jsonrpc: "2.0", id: message.id, result: { outcome: { outcome: "selected", optionId: chosen } } })}\n`,
          );
          return;
        }
        if (message.method === "session/update") {
          const params = asRecord(message.params) ?? {};
          const update = asRecord(params.update) ?? {};
          switch (update.sessionUpdate) {
            case "agent_message_chunk": {
              const content = asRecord(update.content) ?? {};
              const text = asString(content.text);
              if (text !== "") updates.push({ type: "acp_text_delta", text });
              break;
            }
            case "agent_thought_chunk": {
              const content = asRecord(update.content) ?? {};
              const text = asString(content.text);
              if (text !== "") updates.push({ type: "acp_thought_delta", text });
              break;
            }
            case "tool_call": {
              const toolCallId = toSafeRef(
                asString(update.toolCallId),
                `toolcall.grok.${updates.length}`,
              );
              const toolName = toSafeRef(asString(update.title) || asString(update.kind), "tool");
              startedCalls.set(toolCallId, toolName);
              updates.push({ type: "acp_tool_call", toolCallId, toolName });
              break;
            }
            case "tool_call_update": {
              const toolCallId = toSafeRef(asString(update.toolCallId), "");
              const status = asString(update.status);
              if (toolCallId !== "" && (status === "completed" || status === "failed")) {
                updates.push({
                  type: "acp_tool_result",
                  toolCallId,
                  toolName: startedCalls.get(toolCallId) ?? "tool",
                  ok: status === "completed",
                });
              }
              break;
            }
            default:
              break;
          }
          return;
        }
        const id = typeof message.id === "number" ? message.id : null;
        if (id === null) return;
        const waiter = pending.get(id);
        if (waiter === undefined) return;
        pending.delete(id);
        const error = asRecord(message.error);
        if (error !== null) {
          waiter.reject(new Error(asString(error.message) || JSON.stringify(error)));
        } else {
          waiter.resolve(asRecord(message.result) ?? {});
        }
      });

      const request = (method: string, params: unknown, timeoutMs: number): Promise<JsonRecord> => {
        const id = nextId++;
        return new Promise((resolve, reject) => {
          const timer = setTimeout(() => {
            pending.delete(id);
            reject(new Error(`${method} timed out after ${timeoutMs}ms`));
          }, timeoutMs);
          pending.set(id, {
            resolve: (value) => {
              clearTimeout(timer);
              resolve(value);
            },
            reject: (error) => {
              clearTimeout(timer);
              reject(error);
            },
          });
          child.stdin!.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
        });
      };

      const startTimeoutMs = options.startTimeoutMs ?? 30_000;
      const promptTimeoutMs = options.promptTimeoutMs ?? 600_000;

      const init = await request(
        "initialize",
        {
          protocolVersion: 1,
          clientCapabilities: {
            fs: { readTextFile: false, writeTextFile: false },
            terminal: false,
          },
        },
        startTimeoutMs,
      );
      const authMethods = new Set(
        (Array.isArray(init.authMethods) ? init.authMethods : []).map((method) =>
          asString(asRecord(method)?.id),
        ),
      );
      const methodId =
        process.env.XAI_API_KEY !== undefined && authMethods.has("xai.api_key")
          ? "xai.api_key"
          : authMethods.has("cached_token")
            ? "cached_token"
            : null;
      if (methodId === null) {
        child.kill("SIGKILL");
        throw new Error("no usable grok auth method (run `grok login` or set XAI_API_KEY)");
      }
      await request("authenticate", { methodId, _meta: { headless: true } }, startTimeoutMs);
      const session = await request(
        "session/new",
        { cwd: options.cwd, mcpServers: [] },
        startTimeoutMs,
      );
      const sessionId = asString(session.sessionId);
      if (sessionId === "") {
        child.kill("SIGKILL");
        throw new Error("session/new returned no sessionId");
      }

      const transport: AcpTransport = {
        promptTurn: (params) =>
          Effect.tryPromise({
            try: async () => {
              updates = [];
              const result = await request(
                "session/prompt",
                { sessionId, prompt: [{ type: "text", text: params.prompt }] },
                promptTimeoutMs,
              );
              const stopReason = asString(result.stopReason) || "end_turn";
              return [
                { type: "acp_turn_started" } as const,
                ...updates,
                { type: "acp_turn_stop", stopReason } as const,
              ];
            },
            catch: (error) => turnError("acp_prompt_failed", String(error)),
          }),
        shutdown: () =>
          Effect.sync(() => {
            rl.close();
            child.kill("SIGTERM");
          }),
      };
      return transport;
    },
    catch: (error) => turnError("acp_start_failed", String(error)),
  });
