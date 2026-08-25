import { appendFileSync } from "node:fs";

import { accumulate, frames, parse, parseArguments } from "./coder-thread.js";
import type { ChildGrant } from "./coder-child-gateway.js";
import type { DelegateEvent, DelegateHarness } from "./coder-delegate.js";
import { boundedResult } from "./coder-thread.js";
import type { CoderTool } from "./coder-tools.js";
import { shellTool } from "./coder-tools.js";
import { Redacted } from "effect";

/**
 * Children run by this process, on the account's own thread grant.
 *
 * The lane this replaces is `opencode`: a second coding agent, installed
 * separately, with its own credentials, its own model catalog, its own tool
 * loop, and its own idea of what a coding agent is. It worked, and it cost a
 * process per child and an unbounded amount of behaviour nobody here chose —
 * a child answering from whichever model that install happened to have, with
 * whichever tools that version happened to ship.
 *
 * A self-hosted child is the same loop the parent session already runs,
 * smaller: the grant the server minted for children, pinned to Ox Alpha, a
 * short and deliberate toolset, and this process's own turn loop. No second
 * agent to install, no second credential, and the model is the one the
 * conversation asked for rather than the one the harness had.
 *
 * The proxy is the only thing it talks to, so no provider key reaches this
 * process (RELEASE-002). `ox-alpha` is what the child thread's grant pins, and
 * the server routes that to OpenRouter's `stealth/ox-alpha`.
 *
 * `opencode` stays as the fallback for a session with no grant to spend, and
 * for a reader who names it.
 */

/** How many rounds of tool calls one child may take. */
const MAX_ROUNDS = 60;

/** What a child is told it is, and what it may do. */
const SYSTEM = (cwd: string, tools: ReadonlyArray<CoderTool>) =>
  [
    "You are a delegated child agent of `openagents coder`, working in a terminal.",
    "",
    "You were given one task by a parent agent, and you cannot ask it anything: it is not",
    "waiting on you and there is nobody to answer. Everything you need is in the task, or is",
    "on this machine, or is not available — say so plainly rather than guessing.",
    "",
    `The working directory is ${cwd}.`,
    "",
    `You have ${String(tools.length)} tool${tools.length === 1 ? "" : "s"}, and no others:`,
    ...tools.map((tool) => `- \`${tool.name}\``),
    "",
    // The parent counts on this: a child's answer is read by an agent, not a
    // person, and a child that stops mid-task without saying so is reported as
    // having succeeded.
    "That list is complete. You cannot delegate further — you are the child. When you are",
    "done, your final message is the whole of what the parent receives, so it has to carry",
    "the answer rather than point at work you did. If you could not finish, say what you did,",
    "what stopped you, and what remains.",
  ].join("\n");

export interface SelfHarnessOptions {
  readonly grant: ChildGrant;
  /** Overrides the tools a child gets. For tests. */
  readonly tools?: (cwd: string) => ReadonlyArray<CoderTool>;
}

type WireMessage =
  | { readonly role: "system"; readonly content: string }
  | { readonly role: "user"; readonly content: string }
  | {
      readonly role: "assistant";
      readonly content: string;
      readonly tool_calls?: ReadonlyArray<Record<string, unknown>>;
    }
  | { readonly role: "tool"; readonly tool_call_id: string; readonly content: string };

export class SelfHarness implements DelegateHarness {
  readonly agent = "openagents";
  readonly model: string;

  /**
   * Live children's transcripts, by session.
   *
   * What makes a retry resume rather than restart. A child whose provider
   * dropped after twenty tool calls carries on from its own transcript instead
   * of re-reading and re-editing everything.
   */
  private readonly sessions = new Map<string, WireMessage[]>();
  private sequence = 0;

  constructor(private readonly options: SelfHarnessOptions) {
    this.model = options.grant.model;
  }

  /**
   * The toolset a child gets, which is deliberately shorter than the parent's.
   *
   * `shell` is the whole of it. It reads, writes, searches, lists, and runs
   * tests, which is the work a child is given; the parent's other tools are
   * either the parent's own business (`delegate` — a child that delegates is a
   * fan-out nobody asked for) or a way of reaching the account (`openagents`),
   * which is not a child's to spend.
   */
  private toolsFor(cwd: string): ReadonlyArray<CoderTool> {
    return this.options.tools?.(cwd) ?? [shellTool(cwd)];
  }

  async *run(
    input: {
      readonly prompt: string;
      readonly cwd: string;
      readonly transcriptPath: string;
      readonly resumeSessionId?: string | undefined;
    },
    signal: AbortSignal,
  ): AsyncIterable<DelegateEvent> {
    const tools = this.toolsFor(input.cwd);

    const resumed =
      input.resumeSessionId === undefined
        ? undefined
        : this.sessions.get(input.resumeSessionId);

    const sessionId = input.resumeSessionId ?? this.mintSession();
    const transcript: WireMessage[] = resumed ?? [
      { role: "system", content: SYSTEM(input.cwd, tools) },
      { role: "user", content: input.prompt },
    ];

    if (resumed !== undefined) {
      transcript.push({
        role: "user",
        content:
          "The previous attempt stopped when the model provider became unavailable. " +
          "Continue from where you left off and finish the task.",
      });
    }

    this.sessions.set(sessionId, transcript);
    yield { type: "session", sessionId };

    const record = (entry: Record<string, unknown>) => {
      // Written as it happens, not at the end, so a child that is killed still
      // leaves everything it had done behind.
      try {
        appendFileSync(input.transcriptPath, `${JSON.stringify(entry)}\n`);
      } catch {
        // A transcript that cannot be written must not end the child's work.
      }
    };

    record({ type: "session", sessionId, model: this.model, cwd: input.cwd });

    for (let round = 0; round < MAX_ROUNDS; round += 1) {
      if (signal.aborted) return;

      const calls = new Map<number, { id: string; name: string; args: string }>();
      let said = "";

      const body = await this.call(transcript, tools, signal);
      if (body === undefined || signal.aborted) return;

      for await (const frame of frames(body, signal)) {
        if (signal.aborted) return;
        if (frame === "[DONE]") break;

        const payload = parse(frame);
        if (payload === undefined) continue;

        const usage = payload["usage"];
        if (typeof usage === "object" && usage !== null) {
          const counts = usage as Record<string, unknown>;
          const input_tokens = counts["prompt_tokens"];
          const output_tokens = counts["completion_tokens"];
          if (typeof input_tokens === "number" && typeof output_tokens === "number") {
            yield { type: "tokens", input: input_tokens, output: output_tokens };
          }
        }

        const choices = payload["choices"];
        if (!Array.isArray(choices)) continue;

        for (const choice of choices) {
          const delta = (choice as Record<string, unknown>)["delta"];
          if (typeof delta !== "object" || delta === null) continue;
          const parts = delta as Record<string, unknown>;

          const content = parts["content"];
          if (typeof content === "string") said += content;

          const asked = parts["tool_calls"];
          if (Array.isArray(asked)) accumulate(calls, asked);
        }
      }

      const wanted = [...calls.values()];

      if (wanted.length === 0) {
        if (said.length > 0) transcript.push({ role: "assistant", content: said });
        record({ type: "text", value: said });
        yield { type: "text", value: said };
        this.sessions.delete(sessionId);
        return;
      }

      transcript.push({
        role: "assistant",
        content: said,
        tool_calls: wanted.map((call) => ({
          id: call.id,
          type: "function",
          function: { name: call.name, arguments: call.args },
        })),
      });

      for (const call of wanted) {
        if (signal.aborted) return;

        const args = parseArguments(call.args);
        yield {
          type: "tool",
          callId: call.id,
          name: call.name,
          target: targetOf(args),
        };
        record({ type: "tool", callId: call.id, name: call.name, arguments: args });

        const tool = tools.find((candidate) => candidate.name === call.name);
        const output =
          tool === undefined
            ? `No tool called ${call.name} is available to a child agent.`
            : await tool
                .run(args, signal)
                .catch((cause: unknown) => `The tool failed: ${String(cause)}`);

        transcript.push({
          role: "tool",
          tool_call_id: call.id,
          content: boundedResult(output),
        });
        record({ type: "tool_result", callId: call.id, output });
      }
    }

    yield {
      type: "error",
      message: `The child stopped after ${String(MAX_ROUNDS)} rounds of tool calls.`,
    };
  }

  /** One call to the proxy on the child's grant. */
  private async call(
    transcript: ReadonlyArray<WireMessage>,
    tools: ReadonlyArray<CoderTool>,
    signal: AbortSignal,
  ): Promise<ReadableStream<Uint8Array> | undefined> {
    const response = await fetch(this.options.grant.proxyUrl, {
      method: "POST",
      signal,
      headers: {
        authorization: `Bearer ${Redacted.value(this.options.grant.token)}`,
        "content-type": "application/json",
        accept: "text/event-stream, application/json",
      },
      body: JSON.stringify({
        model: this.options.grant.model,
        stream: true,
        messages: transcript,
        tools: tools.map((tool) => ({
          type: "function",
          function: {
            name: tool.name,
            description: tool.description,
            parameters: tool.parameters,
          },
        })),
      }),
    }).catch((cause: unknown) => {
      if (signal.aborted) return undefined;
      // Thrown rather than yielded, so the fleet's retry sees it: the words
      // matter, because that is what `transientProviderFailure` reads.
      throw new Error(`Upstream request failed: ${String(cause)}`);
    });

    if (response === undefined || signal.aborted) return undefined;

    if (!response.ok) {
      const detail = (await response.text().catch(() => "")).slice(0, 300);
      throw new Error(
        `The inference proxy refused the child's call (${String(response.status)})` +
          (detail.length === 0 ? "." : `: ${detail}`),
      );
    }

    return response.body ?? undefined;
  }

  private mintSession(): string {
    this.sequence += 1;
    return `s${Date.now().toString(36)}${this.sequence.toString(36).padStart(2, "0")}`;
  }
}

/** The one argument worth showing in a fleet row, if there is one. */
function targetOf(args: Record<string, unknown>): string | undefined {
  for (const key of ["command", "path", "file", "pattern"]) {
    const value = args[key];
    if (typeof value === "string" && value.length > 0) return value;
  }
  return undefined;
}
