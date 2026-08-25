import { spawn } from "node:child_process";

import type { DelegateEvent } from "./coder-delegate.js";

/**
 * A Devin child over ACP, so the fleet can see it working.
 *
 * Devin's print mode (`devin -p`) is a black box. It writes nothing to stdout
 * until the very end — measured: fourteen seconds of silence, then the whole
 * answer at once — and `--export` is written at close too. A child doing four
 * minutes of real work therefore reported no tool calls, no tokens, and no
 * transcript, so the fleet showed `Initializing…` for the whole run and a
 * reader could not tell it from a hang.
 *
 * `devin acp` is the same agent as an Agent Client Protocol server over stdio,
 * and it streams: `tool_call` with a title, `tool_call_update` with a status,
 * `usage_update` with real token counts, and `agent_message_chunk` for the
 * answer. That is every event the fleet already knows how to draw.
 *
 * Newline-delimited JSON-RPC, and one server per child. A shared server would
 * save a process per child and cost a lifecycle nobody asked for: a crash
 * would take every child with it, and a child that hangs would hold the
 * server's queue.
 *
 * Devin logs heavily to stderr and none of it is protocol. It is drained and
 * dropped rather than parsed.
 */

/** What a running Devin child reports, normalized. */
export interface DevinAcpOptions {
  readonly command?: string | undefined;
  /**
   * The session mode, which is Devin's own word for what the old harness passed
   * as `--permission-mode`. `bypass` is the equivalent of `dangerous`.
   */
  readonly mode?: string | undefined;
  readonly env?: Record<string, string> | undefined;
  /** Every protocol message, for the transcript. */
  readonly record?: ((entry: Record<string, unknown>) => void) | undefined;
}

interface Pending {
  readonly resolve: (result: Record<string, unknown>) => void;
  readonly reject: (cause: Error) => void;
}

export async function* runDevinAcp(
  input: { readonly prompt: string; readonly cwd: string },
  options: DevinAcpOptions,
  signal: AbortSignal,
): AsyncIterable<DelegateEvent> {
  const command = options.command ?? "devin";
  const child = spawn(command, ["acp"], {
    cwd: input.cwd,
    env: { ...process.env, ...options.env },
    stdio: ["pipe", "pipe", "pipe"],
    // Its own process group, so stopping a child stops what the child started.
    detached: true,
  });

  // Devin's own logging. Not protocol, and it is a lot of it.
  child.stderr.resume();

  const events: DelegateEvent[] = [];
  const pending = new Map<number, Pending>();
  let wake: (() => void) | undefined;
  let finished = false;
  let failure: Error | undefined;
  let sequence = 0;
  let buffer = "";
  let answer = "";

  const nudge = () => {
    wake?.();
    wake = undefined;
  };

  const send = (method: string, params?: Record<string, unknown>) =>
    new Promise<Record<string, unknown>>((resolve, reject) => {
      const id = (sequence += 1);
      pending.set(id, { resolve, reject });
      child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`);
    });

  const reply = (id: number, result: Record<string, unknown>) => {
    child.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
  };

  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk: string) => {
    buffer += chunk;
    for (;;) {
      const at = buffer.indexOf("\n");
      if (at === -1) break;
      const line = buffer.slice(0, at).trim();
      buffer = buffer.slice(at + 1);
      if (line.length === 0) continue;

      let message: Record<string, unknown>;
      try {
        message = JSON.parse(line) as Record<string, unknown>;
      } catch {
        // Not protocol. Devin writes plain lines too.
        continue;
      }

      options.record?.(message);
      handle(message);
    }
    nudge();
  });

  const handle = (message: Record<string, unknown>) => {
    const id = message["id"];

    // A reply to something asked for.
    if (typeof id === "number" && message["method"] === undefined) {
      const waiting = pending.get(id);
      pending.delete(id);
      if (waiting === undefined) return;
      const error = message["error"];
      if (error !== undefined) {
        waiting.reject(new Error(`Devin refused ${JSON.stringify(error).slice(0, 200)}`));
      } else {
        waiting.resolve((message["result"] ?? {}) as Record<string, unknown>);
      }
      return;
    }

    const method = message["method"];

    // A request from the agent. The only one that matters is permission, and a
    // delegated child has no one to ask: it was launched to run unattended, so
    // an unanswered request would hang it for as long as the reader left it.
    if (typeof id === "number" && method === "session/request_permission") {
      const params = record(message["params"]);
      const chosen = firstAllowOption(params);
      reply(id, {
        outcome:
          chosen === undefined
            ? { outcome: "cancelled" }
            : { outcome: "selected", optionId: chosen },
      });
      return;
    }

    if (method !== "session/update") return;

    const update = record(record(message["params"])["update"]);
    const kind = update["sessionUpdate"];

    if (kind === "tool_call") {
      const callId = text(update["toolCallId"]) ?? `devin_${String(events.length)}`;
      // Devin's `title` is already the phrase a person would read — "Ran ls",
      // "Read src/a.ts" — so it is the activity rather than a name to look up.
      events.push({
        type: "tool",
        callId,
        name: text(update["kind"]) ?? "tool",
        target: text(update["title"]),
      });
      return;
    }

    if (kind === "usage_update") {
      const meta = record(update["_meta"]);
      const input_tokens = number(meta["cognition.ai/inputTokens"]);
      const output_tokens = number(meta["cognition.ai/outputTokens"]);
      if (input_tokens !== undefined && output_tokens !== undefined) {
        events.push({ type: "tokens", input: input_tokens, output: output_tokens });
      }
      return;
    }

    if (kind === "agent_message_chunk") {
      const piece = text(record(update["content"])["text"]);
      if (piece !== undefined) answer += piece;
    }
  };

  child.on("error", (cause: Error) => {
    failure =
      (cause as NodeJS.ErrnoException).code === "ENOENT"
        ? new Error(`The \`${command}\` command is not on PATH.`)
        : cause;
    finished = true;
    nudge();
  });

  child.on("close", () => {
    finished = true;
    for (const waiting of pending.values()) {
      waiting.reject(new Error("The Devin agent exited before it answered."));
    }
    pending.clear();
    nudge();
  });

  const stop = () => {
    try {
      process.kill(-child.pid!, "SIGKILL");
    } catch {
      child.kill("SIGKILL");
    }
  };
  signal.addEventListener("abort", stop, { once: true });

  // The conversation, driven from here while the generator yields whatever the
  // agent has said since the last time it was asked.
  const turn = (async () => {
    await send("initialize", {
      protocolVersion: 1,
      clientCapabilities: { fs: { readTextFile: false, writeTextFile: false } },
    });

    const opened = await send("session/new", { cwd: input.cwd, mcpServers: [] });
    const sessionId = text(opened["sessionId"]);
    if (sessionId === undefined) throw new Error("Devin opened no session.");
    events.push({ type: "session", sessionId });

    const mode = options.mode;
    if (mode !== undefined) {
      // Best effort. A build of Devin without this mode should not lose the
      // child over the name of a permission setting.
      await send("session/set_mode", { sessionId, modeId: mode }).catch(() => ({}));
    }

    await send("session/prompt", {
      sessionId,
      prompt: [{ type: "text", text: input.prompt }],
    });
  })();

  turn.catch((cause: unknown) => {
    // Whichever failure came first. A missing binary raises `error` and then
    // `close`, and the close rejects everything still pending — so taking the
    // later one reports "the agent exited before it answered" for a command
    // that was never there.
    failure ??= cause instanceof Error ? cause : new Error(String(cause));
  });

  const settled = turn.then(
    () => {
      finished = true;
      nudge();
    },
    () => {
      finished = true;
      nudge();
    },
  );

  try {
    for (;;) {
      while (events.length > 0) {
        const event = events.shift();
        if (event !== undefined) yield event;
      }
      if (finished) break;
      if (signal.aborted) return;
      await new Promise<void>((resolve) => {
        wake = resolve;
      });
    }

    await settled;

    // A stopped fleet is not a failed child. Killing the agent closes its
    // stdio, which rejects everything still in flight, and reporting that as
    // an error would make every `ctrl+x` look like a crash.
    if (signal.aborted) return;

    if (failure !== undefined) {
      yield { type: "error", message: failure.message };
      throw failure;
    }

    const said = answer.trim();
    if (said.length > 0) yield { type: "text", value: said };
  } finally {
    signal.removeEventListener("abort", stop);
    stop();
  }
}

/** The option a permission request offers that lets the work continue. */
const firstAllowOption = (params: Record<string, unknown>): string | undefined => {
  const options = params["options"];
  if (!Array.isArray(options)) return undefined;

  const ranked = options
    .map((option) => record(option))
    .filter((option) => text(option["optionId"]) !== undefined);

  const allow = ranked.find((option) => String(option["kind"] ?? "").startsWith("allow"));
  return text((allow ?? ranked[0] ?? {})["optionId"]);
};

const record = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

const text = (value: unknown): string | undefined =>
  typeof value === "string" && value.length > 0 ? value : undefined;

const number = (value: unknown): number | undefined =>
  typeof value === "number" && Number.isFinite(value) ? value : undefined;
