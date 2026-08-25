import { existsSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import type { ReplyChunk, ReplySource } from "./coder-session.js";
import { systemPrompt } from "./coder-system.js";
import { accumulate, boundedResult, frames, parse, parseArguments } from "./coder-thread.js";
import type { CoderTool } from "./coder-tools.js";

/**
 * A coder session answered by OpenCode Zen, on the credential opencode already
 * holds on this machine.
 *
 * The reason this lane exists: Ox Alpha is free and unlimited there, and no
 * OpenAgents deployment serves it — the model is in the server's catalog but
 * its provider credential is not configured, so a thread opened on it is
 * refused. Waiting for that credential to reach a deployment is a wait; the
 * machine already has one.
 *
 * Zen is an OpenAI-compatible endpoint that takes tool calls, so this is the
 * same wire shape the inference proxy speaks and the same turn loop. What it is
 * not is `opencode` the agent: opencode's own server runs its own loop with its
 * own tools, and a session driven through that would be running opencode's
 * tools rather than this session's. Only the endpoint and the key are borrowed.
 *
 * Nothing here spends an OpenAgents grant and nothing reaches a thread, so a
 * session on this lane keeps no server-side transcript.
 */

/** Where opencode keeps the credential this lane borrows. */
const AUTH_FILE = join(homedir(), ".local", "share", "opencode", "auth.json");

const ZEN_BASE = "https://opencode.ai/zen/v1";

/**
 * The names a reader uses for a Zen model, mapped to what the API takes.
 *
 * `ox-alpha` is the name it is known by and `x-preview-f-free` is the slug it
 * answers to; Zen itself calls it "Ox Alpha Free (Unlimited)". A reader who
 * types the name it is called should not have to know the other one.
 */
const ALIASES: Record<string, string> = {
  "ox-alpha": "x-preview-f-free",
  "ox-alpha-free": "x-preview-f-free",
};

export const zenModelId = (asked: string): string => ALIASES[asked] ?? asked;

/**
 * The credential, from the environment or from opencode's own store.
 *
 * Read rather than copied: this is opencode's key, it stays where opencode put
 * it, and a session that finds none says so instead of calling without one.
 */
export const zenCredential = (
  env: NodeJS.ProcessEnv = process.env,
  authFile: string = AUTH_FILE,
): string | undefined => {
  const named = env["OPENCODE_API_KEY"];
  if (named !== undefined && named.length > 0) return named;

  if (!existsSync(authFile)) return undefined;
  try {
    const store = JSON.parse(readFileSync(authFile, "utf8")) as Record<string, unknown>;
    const entry = store["opencode"];
    if (typeof entry !== "object" || entry === null) return undefined;
    const key = (entry as Record<string, unknown>)["key"];
    return typeof key === "string" && key.length > 0 ? key : undefined;
  } catch {
    return undefined;
  }
};

const LANE = "You answer from Ox Alpha through OpenCode Zen, on this machine's own credential.";

/** How many rounds of tool calls one turn may take before it is stopped. */
const MAX_ROUNDS = 100;

type WireMessage =
  | { readonly role: "system"; readonly content: string }
  | { readonly role: "user"; readonly content: string }
  | {
      readonly role: "assistant";
      readonly content: string;
      readonly tool_calls?: ReadonlyArray<Record<string, unknown>>;
    }
  | { readonly role: "tool"; readonly tool_call_id: string; readonly content: string };

export class ZenReplySource implements ReplySource {
  private readonly key: string;
  private readonly slug: string;
  private readonly transcript: WireMessage[] = [];
  private tools: ReadonlyArray<CoderTool> = [];
  private standing: string | undefined;
  private steered: string[] = [];
  private spentIn = 0;
  private spentOut = 0;
  private callCount = 0;

  constructor(options: { readonly model: string; readonly key: string }) {
    this.slug = zenModelId(options.model);
    this.key = options.key;
  }

  get model(): string {
    return this.slug === "x-preview-f-free" ? "ox-alpha" : this.slug;
  }

  get modelId(): string {
    return this.slug;
  }

  /** What the status line shows in place of a thread budget: this lane has none. */
  get budget(): string {
    return `${String(this.callCount)} calls · ${String(this.spentIn + this.spentOut)} tok · free`;
  }

  useContext(standing: string): void {
    this.standing = standing;
  }

  useTools(tools: ReadonlyArray<CoderTool>): void {
    this.tools = tools;
  }

  toolDefinitions(): ReadonlyArray<Record<string, unknown>> {
    return this.tools.map((tool) => ({
      type: "function",
      function: { name: tool.name, description: tool.description, parameters: tool.parameters },
    }));
  }

  describeContext(): string {
    const declarations =
      this.tools.length === 0
        ? "No tools are declared to the model."
        : `${String(this.tools.length)} tool${this.tools.length === 1 ? "" : "s"} declared to the model:\n\n${this.tools
            .map(
              (tool) =>
                `- \`${tool.name}\`\n  ${tool.description}\n  parameters: ${JSON.stringify(tool.parameters)}`,
            )
            .join("\n\n")}`;

    return [
      `System message sent with every turn:\n\n${systemPrompt(this.tools, LANE, this.standing)}`,
      "",
      declarations,
    ].join("\n");
  }

  steer(text: string): boolean {
    this.steered.push(text);
    return true;
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    if (!this.transcript.some((message) => message.role === "system")) {
      this.transcript.unshift({
        role: "system",
        content: systemPrompt(this.tools, LANE, this.standing),
      });
    }
    this.transcript.push({ role: "user", content: prompt });

    for (let round = 0; round < MAX_ROUNDS; round += 1) {
      if (signal.aborted) return;

      // Read between two model calls rather than at the end of the turn, which
      // is the difference between steering a model and waiting one out.
      const steered = this.steered.splice(0);
      for (const said of steered) {
        this.transcript.push({ role: "user", content: said });
      }
      // The interface dims a steered message until this says it was read.
      if (steered.length > 0) yield { type: "steered", texts: steered };

      const calls: Map<number, { id: string; name: string; args: string }> = new Map();
      let assistant = "";

      const response = await this.call(signal);
      if (response === undefined || signal.aborted) return;

      for await (const frame of frames(response, signal)) {
        if (signal.aborted) return;
        if (frame === "[DONE]") break;

        const payload = parse(frame);
        if (payload === undefined) continue;

        const usage = payload["usage"];
        if (typeof usage === "object" && usage !== null) this.spend(usage as Record<string, unknown>);

        const choices = payload["choices"];
        if (!Array.isArray(choices)) continue;

        for (const choice of choices) {
          const delta = (choice as Record<string, unknown>)["delta"];
          if (typeof delta !== "object" || delta === null) continue;
          const parts = delta as Record<string, unknown>;

          const thought = parts["reasoning"] ?? parts["reasoning_content"];
          if (typeof thought === "string" && thought.length > 0) {
            yield { type: "reasoning", value: thought };
          }

          const content = parts["content"];
          if (typeof content === "string" && content.length > 0) {
            assistant += content;
            yield { type: "text", value: content };
          }

          const toolCalls = parts["tool_calls"];
          if (Array.isArray(toolCalls)) accumulate(calls, toolCalls);
        }
      }

      const asked = [...calls.values()];
      if (asked.length === 0) {
        if (assistant.length > 0) this.transcript.push({ role: "assistant", content: assistant });
        yield {
          type: "usage",
          promptTokens: this.spentIn,
          completionTokens: this.spentOut,
          calls: round + 1,
        };
        return;
      }

      // The assistant turn carries the calls it made, and every one of them is
      // answered before the next round: a call whose result never follows is a
      // transcript the provider refuses.
      this.transcript.push({
        role: "assistant",
        content: assistant,
        tool_calls: asked.map((call) => ({
          id: call.id,
          type: "function",
          function: { name: call.name, arguments: call.args },
        })),
      });

      for (const call of asked) {
        yield { type: "tool_call", callId: call.id, name: call.name, arguments: call.args };

        const tool = this.tools.find((candidate) => candidate.name === call.name);
        const output =
          tool === undefined
            ? `No tool called ${call.name} is declared in this session.`
            : await tool
                .run(parseArguments(call.args), signal)
                .catch((cause: unknown) => `The tool failed: ${String(cause)}`);

        this.transcript.push({
          role: "tool",
          tool_call_id: call.id,
          content: boundedResult(output),
        });
        yield { type: "tool_result", callId: call.id, output, error: undefined };
      }
    }

    yield {
      type: "text",
      value: `\n\nThe turn stopped after ${String(MAX_ROUNDS)} rounds of tool calls.`,
    };
  }

  private async call(signal: AbortSignal): Promise<ReadableStream<Uint8Array> | undefined> {
    this.callCount += 1;

    const response = await fetch(`${ZEN_BASE}/chat/completions`, {
      method: "POST",
      signal,
      headers: {
        authorization: `Bearer ${this.key}`,
        "content-type": "application/json",
        accept: "text/event-stream",
      },
      body: JSON.stringify({
        model: this.slug,
        stream: true,
        stream_options: { include_usage: true },
        messages: this.transcript,
        ...(this.tools.length === 0
          ? {}
          : {
              tools: this.tools.map((tool) => ({
                type: "function",
                function: {
                  name: tool.name,
                  description: tool.description,
                  parameters: tool.parameters,
                },
              })),
            }),
      }),
    }).catch((cause: unknown) => {
      if (signal.aborted) return undefined;
      throw new Error(`OpenCode Zen could not be reached: ${String(cause)}`);
    });

    if (response === undefined || signal.aborted) return undefined;

    if (!response.ok) {
      const detail = (await response.text().catch(() => "")).slice(0, 300);
      throw new Error(
        `OpenCode Zen refused the call (${String(response.status)})` +
          (detail.length === 0 ? "." : `: ${detail}`),
      );
    }

    return response.body ?? undefined;
  }

  private spend(usage: Record<string, unknown>): void {
    const input = usage["prompt_tokens"];
    const output = usage["completion_tokens"];
    if (typeof input === "number") this.spentIn = input;
    if (typeof output === "number") this.spentOut = output;
  }
}
