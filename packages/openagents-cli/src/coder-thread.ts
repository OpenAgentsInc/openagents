/**
 * A reply source backed by a thread of the caller's own, and the grant that
 * thread mints.
 *
 * `openagents coder` used to submit through `POST /api/v3/chat/turns` and poll
 * `GET /api/v3/chat/events`. That was the only route a user token could reach a
 * model through, and the server records one conversation per account, so every
 * prompt a person typed in a terminal landed in the same conversation `/chat`
 * reads, contended for the one streaming slot that conversation admits, and
 * became provider context for the next question asked in the browser. This
 * replaces both paths.
 *
 * `POST /api/v3/threads` opens a thread and returns a grant. The grant is the
 * bearer for `POST /api/inference/proxy`, an OpenAI-compatible
 * `/chat/completions` surface that meters against the thread's own budget and
 * keeps the provider credential on the server, so the CLI still holds no
 * provider key. `DELETE /api/v3/threads/{id}` revokes the thread on exit, which
 * matters because an account may hold only eight open threads at once and a
 * closed terminal would otherwise hold a slot until the authority expired.
 *
 * Three properties of that proxy shape this file, and each is a real loss
 * against the event log this replaces:
 *
 * - **It answers in one piece.** The proxy builds the whole SSE body and sends
 *   it once, so the frames below all arrive together. The parser is written
 *   against the stream anyway rather than against `await response.text()`, so
 *   chunked delivery becomes visible here the day the server sends it, with no
 *   change on this side.
 * - **It carries no reasoning.** `OpenAgents.Providers.ProviderEvent` has no
 *   reasoning member at all — the union is `response_started`, `text_delta`,
 *   `tool_call`, `usage`, `response_completed`, `failed`, `cancelled` — and the
 *   proxy drops everything it cannot name. The chat event log had
 *   `reasoning_delta`; nothing on this path does. So no `reasoning` chunk is
 *   ever produced here, and the interface's dim-italic reasoning entry, which
 *   the stand-in behind `--offline` still exercises, never appears against a
 *   live model.
 * - **Tools run here.** The chat lane ran tools on the server and reported each
 *   one. The proxy is a bare completions surface: it forwards the `tools` a
 *   caller declares and returns the calls the model asks for, and the caller
 *   executes them. So the loop below is the tool runtime — `useTools` gives it
 *   the tools a session declares, and a turn continues until the model stops
 *   asking for one.
 *
 *   A tool result is fed back as plain turns rather than as a `tool` message.
 *   The proxy maps a `tool` message to a `function_call_output` item and sends
 *   it on its own, which the provider refuses without the `function_call` that
 *   preceded it, and the response id that would link the two is never given to
 *   a client. Until the proxy carries a tool exchange, the honest thing is to
 *   say what was called and what came back in turns it does accept.
 *
 * Nothing here announces any of that on screen. `2c15c6ed20` removed the
 * `scopeNotice` seam with the reasoning that a session private to its own
 * thread has nothing to announce, and a banner at the top of every session
 * teaches the constraint rather than the design. The losses above are real and
 * belong in the issue that decides what to do about them, not in a permanent
 * line above the first prompt.
 *
 * The grant is a bearer credential. It is held `Redacted` so that an accidental
 * interpolation prints a placeholder, it never reaches the transcript, and it
 * is never passed as an argument to anything this process spawns.
 */

import { Redacted } from "effect";

import type { ChildGrant } from "./coder-child-gateway.js";
import type { ReplyChunk, ReplySource } from "./coder-session.js";
import type { CoderTool } from "./coder-tools.js";

const THREADS_PATH = "/api/v3/threads";

/**
 * How many times one turn may call tools before it has to answer.
 *
 * A ceiling rather than a preference: a model that keeps delegating is a model
 * spending the thread's budget without ever reporting to the reader.
 */
const MAX_TOOL_STEPS = 6;

/** What the thread may still spend, as the server last reported it. */
export interface ThreadBudget {
  readonly calls: number;
  readonly totalTokens: number;
  readonly costMicrousd: number;
}

export interface ThreadOptions {
  readonly origin: string;
  /** The account token. Opens, reads, and revokes the thread; never spends it. */
  readonly token: string;
  /** What this body of work is for. The server requires one. */
  readonly objective: string;
  /** Recorded on the thread as its admitted execution shape. */
  readonly reasoning?: string | undefined;
  /**
   * The model the thread's grant pins, and therefore the model every call
   * through the proxy reaches. Omitted, the server pins its default.
   *
   * This is the only place a model is chosen: the proxy refuses a model named
   * in a request body. So a session that wants its children on another model
   * opens a second thread naming it, with its own budget, rather than lending
   * them the authority its own turns spend.
   */
  readonly model?: string | undefined;
}

export class ThreadUnavailable extends Error {
  constructor(
    readonly code: string,
    message: string,
    /** The HTTP status behind the code, or 0 when the request never landed. */
    readonly status = 0,
  ) {
    super(message);
    this.name = "ThreadUnavailable";
  }
}

/**
 * Open a thread and take its grant.
 *
 * A refusal here is reported with the server's own code and sentence. The
 * account cap is the one a person meets: the ninth concurrent session is
 * refused `thread_quota_reached` with a message naming the limit and how many
 * threads the account is holding, which is what tells them to close one rather
 * than to retry.
 */
export async function openThread(options: ThreadOptions): Promise<ThreadReplySource> {
  const response = await fetch(new URL(THREADS_PATH, options.origin), {
    method: "POST",
    headers: {
      authorization: `Bearer ${options.token}`,
      "content-type": "application/json",
      accept: "application/json",
    },
    body: JSON.stringify({
      objective: options.objective,
      ...(options.reasoning === undefined ? {} : { reasoning: options.reasoning }),
      ...(options.model === undefined ? {} : { model: options.model }),
    }),
  }).catch((cause: unknown) => {
    throw new ThreadUnavailable(
      "network_refused",
      `The API at ${options.origin} could not be reached: ${String(cause)}`,
    );
  });

  const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;

  if (response.status === 401 || response.status === 403) {
    throw new ThreadUnavailable(
      "scope_missing",
      "This token cannot open a thread. Run `openagents auth login` to sign in again.",
      response.status,
    );
  }
  if (response.status < 200 || response.status >= 300) {
    // The envelope names the code and the sentence. Passing both through is
    // what turns a ninth session from an obscure failure into an instruction.
    const code = typeof body["code"] === "string" ? body["code"] : `http_${response.status}`;
    const message =
      typeof body["message"] === "string"
        ? body["message"]
        : `The server refused to open a thread (${code}).`;
    throw new ThreadUnavailable(code, message, response.status);
  }

  const thread = record(body["thread"]);
  const grant = record(body["grant"]);
  const id = string(thread["id"]);
  const token = string(grant["token"]);
  const url = string(grant["url"]);
  const model = string(grant["model"]);

  if (id === undefined || token === undefined || url === undefined || model === undefined) {
    throw new ThreadUnavailable(
      "malformed_thread",
      "The server opened a thread but did not return the grant needed to spend it.",
    );
  }

  return new ThreadReplySource({
    origin: options.origin,
    accountToken: options.token,
    threadId: id,
    grantToken: Redacted.make(token),
    proxyUrl: url,
    model,
    budget: budgetOf(record(grant["limits"]), record(grant["limits"])),
  });
}

interface SourceState {
  readonly origin: string;
  readonly accountToken: string;
  readonly threadId: string;
  readonly grantToken: Redacted.Redacted<string>;
  readonly proxyUrl: string;
  readonly model: string;
  readonly budget: ThreadBudget;
}

/** One chat-completions message, which is what the proxy takes as its input. */
interface WireMessage {
  readonly role: "user" | "assistant";
  readonly content: string;
}

/** A call the model asked for, assembled from its fragments. */
interface WireCall {
  readonly id: string;
  readonly name: string;
  readonly args: string;
}

export class ThreadReplySource implements ReplySource {
  readonly threadId: string;
  /**
   * The thread's transcript, keyed on the thread by construction: this array
   * exists only inside the source that holds that thread's grant, so the
   * context a turn is answered against is the thread's and nothing else's.
   * The account conversation is not read and not written.
   */
  private readonly transcript: WireMessage[] = [];
  private remaining: ThreadBudget;
  private tools: ReadonlyArray<CoderTool> = [];

  constructor(private readonly state: SourceState) {
    this.threadId = state.threadId;
    this.remaining = state.budget;
  }

  /**
   * The model the grant pins.
   *
   * Not a backend the client chose. The proxy takes the model from the grant so
   * a request body cannot select another, and the thread route deliberately
   * publishes no model parameter, so this is the one name that is true of the
   * reply on screen.
   */
  get model(): string {
    return this.state.model;
  }

  /** What is left to spend, in the width a status line has for it. */
  get budget(): string {
    return formatBudget(this.remaining);
  }

  /**
   * Declare the tools the model may call.
   *
   * Set after construction because the tools need things the thread produces:
   * the delegate tool runs children on this grant, so it cannot exist until the
   * grant does.
   */
  useTools(tools: ReadonlyArray<CoderTool>): void {
    this.tools = tools;
  }

  /**
   * The grant, for lending to child agents.
   *
   * Handed out as a `Redacted` so a child harness's config or command line
   * cannot print it, and only to callers inside this process.
   */
  get childGrant(): ChildGrant {
    return {
      proxyUrl: this.state.proxyUrl,
      token: this.state.grantToken,
      model: this.state.model,
    };
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    this.transcript.push({ role: "user", content: prompt });

    try {
      for (let step = 0; ; step += 1) {
        const calls: WireCall[] = [];
        let assistant = "";

        for await (const chunk of this.stream(signal, calls)) {
          if (signal.aborted) break;
          if (chunk.type === "text") assistant += chunk.value;
          yield chunk;
        }

        // Whatever the model said belongs to the thread even when the turn was
        // interrupted, or the next turn answers a question it cannot see it
        // half-answered.
        if (assistant.length > 0) this.transcript.push({ role: "assistant", content: assistant });
        if (signal.aborted || calls.length === 0) return;

        if (step >= MAX_TOOL_STEPS) {
          yield {
            type: "text",
            value: `\n\n[stopped after ${String(MAX_TOOL_STEPS)} tool steps in one turn]`,
          };
          return;
        }

        for (const call of calls) {
          if (signal.aborted) return;
          yield* this.invoke(call, signal);
        }
      }
    } finally {
      // Read the budget on the way out of every turn, including an interrupted
      // one. Interrupting is a client-side abort: the proxy had already bought
      // the call and metered it, so a status line that kept the figure it
      // opened with would under-report the spend by exactly the turns a reader
      // cut short.
      await this.refresh();
    }
  }

  /**
   * Run one call, report it, and put the exchange on the thread.
   *
   * The transcript keeps the call and its result as an assistant turn and a
   * user turn for the reason given at the top of this file: a `tool` message is
   * not carried by the proxy today, and a model that cannot see what its own
   * call returned calls it again.
   */
  private async *invoke(call: WireCall, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    yield { type: "tool_call", callId: call.id, name: call.name, arguments: call.args };

    const tool = this.tools.find((candidate) => candidate.name === call.name);
    let output: string;
    let failure: string | undefined;

    if (tool === undefined) {
      failure = `This session has no \`${call.name}\` tool.`;
      output = failure;
    } else {
      try {
        output = await tool.run(parseArguments(call.args), signal);
      } catch (cause) {
        failure = cause instanceof Error ? cause.message : String(cause);
        output = failure;
      }
    }

    yield {
      type: "tool_result",
      callId: call.id,
      output: failure === undefined ? output : undefined,
      error: failure,
    };

    this.transcript.push({
      role: "assistant",
      content: `[tool call]\n${call.name}(${call.args})`,
    });
    this.transcript.push({ role: "user", content: `[tool result ${call.name}]\n${output}` });
  }

  /**
   * Revoke the thread and its grant.
   *
   * Best effort by design: this runs while the process is leaving, and a
   * network failure on the way out must not turn a finished session into an
   * error. The server retires elapsed authority on its own, so the worst case
   * of a failed revoke is a slot held until the thread expires rather than one
   * held forever.
   */
  async revoke(): Promise<void> {
    await fetch(new URL(`${THREADS_PATH}/${this.state.threadId}`, this.state.origin), {
      method: "DELETE",
      headers: {
        authorization: `Bearer ${this.state.accountToken}`,
        accept: "application/json",
      },
    }).catch(() => undefined);
  }

  /**
   * Spend one call against the proxy and translate what comes back.
   *
   * Calls the model asked for are appended to `collected` rather than yielded,
   * because the turn has to run them and report each result, and a caller that
   * only saw a chunk could not.
   */
  private async *stream(signal: AbortSignal, collected: WireCall[]): AsyncIterable<ReplyChunk> {
    const response = await fetch(this.state.proxyUrl, {
      method: "POST",
      signal,
      headers: {
        authorization: `Bearer ${Redacted.value(this.state.grantToken)}`,
        "content-type": "application/json",
        // The body is an event stream and the refusals are JSON, and both have
        // to be acceptable: the `:api` pipeline negotiates on `json` and
        // answers `406` to a request that will only take `text/event-stream`.
        accept: "text/event-stream, application/json",
      },
      body: JSON.stringify({
        model: this.state.model,
        stream: true,
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
      throw new ThreadUnavailable(
        "network_refused",
        `The inference proxy could not be reached: ${String(cause)}`,
      );
    });

    if (response === undefined || signal.aborted) return;
    if (response.status < 200 || response.status >= 300) {
      throw await proxyRefusal(response);
    }
    if (response.body === null) return;

    /** Tool call fragments by their wire index, assembled as frames arrive. */
    const calls = new Map<number, { id: string; name: string; args: string }>();

    for await (const frame of frames(response.body, signal)) {
      if (signal.aborted) return;
      if (frame === "[DONE]") break;

      const payload = parse(frame);
      if (payload === undefined) continue;

      const usage = record(payload["usage"]);
      if (Object.keys(usage).length > 0) this.spend(usage);

      const choices = payload["choices"];
      if (!Array.isArray(choices)) continue;

      for (const choice of choices) {
        const delta = record(record(choice)["delta"]);

        const content = delta["content"];
        if (typeof content === "string" && content.length > 0) {
          yield { type: "text", value: content };
        }

        const toolCalls = delta["tool_calls"];
        if (Array.isArray(toolCalls)) accumulate(calls, toolCalls);
      }
    }

    for (const call of calls.values()) {
      collected.push(call);
    }
  }

  /**
   * Take the turn's own usage off the budget immediately.
   *
   * The authoritative numbers come from the server a moment later, but a status
   * line that only moves after a second request would show a stale budget for
   * exactly as long as the reader is looking at the reply that spent it.
   */
  private spend(usage: Record<string, unknown>): void {
    const total = number(usage["total_tokens"]);
    this.remaining = {
      calls: Math.max(0, this.remaining.calls - 1),
      totalTokens: Math.max(0, this.remaining.totalTokens - total),
      costMicrousd: this.remaining.costMicrousd,
    };
  }

  /** Read what the server says the thread has left. Failure keeps the estimate. */
  private async refresh(): Promise<void> {
    const response = await fetch(
      new URL(`${THREADS_PATH}/${this.state.threadId}`, this.state.origin),
      {
        headers: {
          authorization: `Bearer ${this.state.accountToken}`,
          accept: "application/json",
        },
      },
    ).catch(() => undefined);

    if (response === undefined || response.status < 200 || response.status >= 300) return;
    const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;
    const grant = record(body["grant"]);
    const remaining = record(grant["remaining"]);
    if (Object.keys(remaining).length === 0) return;
    this.remaining = budgetOf(remaining, record(grant["limits"]));
  }
}

/** Frames of an SSE body, yielded as the body arrives rather than after it. */
async function* frames(
  body: ReadableStream<Uint8Array>,
  signal: AbortSignal,
): AsyncIterable<string> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  try {
    for (;;) {
      // A stream is read in order and each read depends on the one before it,
      // so there is no set of promises here to run together.
      // eslint-disable-next-line no-await-in-loop
      const { done, value } = await reader.read();
      if (done || signal.aborted) break;
      buffer += decoder.decode(value, { stream: true });

      for (;;) {
        const boundary = buffer.indexOf("\n\n");
        if (boundary < 0) break;
        const frame = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary + 2);
        const data = dataOf(frame);
        if (data !== undefined) yield data;
      }
    }
  } finally {
    reader.releaseLock();
  }
}

/** The `data:` payload of one frame, or nothing for a comment or a keep-alive. */
function dataOf(frame: string): string | undefined {
  const lines = frame.split("\n");
  const parts: string[] = [];
  for (const line of lines) {
    const trimmed = line.endsWith("\r") ? line.slice(0, -1) : line;
    if (!trimmed.startsWith("data:")) continue;
    parts.push(trimmed.slice(5).trimStart());
  }
  return parts.length === 0 ? undefined : parts.join("\n");
}

function parse(frame: string): Record<string, unknown> | undefined {
  try {
    const value: unknown = JSON.parse(frame);
    return typeof value === "object" && value !== null
      ? (value as Record<string, unknown>)
      : undefined;
  } catch {
    return undefined;
  }
}

/**
 * Fold `tool_calls` fragments into whole calls.
 *
 * Chat-completions splits one call across frames and identifies the pieces by
 * `index`, so a name and its arguments can arrive separately.
 */
function accumulate(
  calls: Map<number, { id: string; name: string; args: string }>,
  fragments: ReadonlyArray<unknown>,
): void {
  for (const fragment of fragments) {
    const piece = record(fragment);
    const index = number(piece["index"]);
    const current = calls.get(index) ?? { id: "", name: "tool", args: "" };
    const fn = record(piece["function"]);

    calls.set(index, {
      id: string(piece["id"]) ?? current.id,
      name: string(fn["name"]) ?? current.name,
      args: current.args + (string(fn["arguments"]) ?? ""),
    });
  }
}

/** The proxy's typed refusal, turned into a sentence a reader can act on. */
async function proxyRefusal(response: Response): Promise<ThreadUnavailable> {
  const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;
  const code = string(record(body["error"])["code"]) ?? `http_${response.status}`;

  const sentences: Record<string, string> = {
    grant_revoked: "This thread was revoked. Start a new session to open another.",
    grant_expired: "This thread's authority expired. Start a new session to open another.",
    grant_exhausted: "This thread spent its budget. Start a new session to open another.",
    grant_budget_reached: "This thread reached its budget ceiling and cannot buy another call.",
    invalid_grant: "The inference proxy did not recognize this thread's grant.",
    provider_failed: "The model provider failed. The call was not completed.",
  };

  return new ThreadUnavailable(
    code,
    sentences[code] ?? `The inference proxy refused the call (${code}).`,
    response.status,
  );
}

/**
 * The budget, read from `remaining` when the server has reported one and from
 * `limits` at the moment of minting, when nothing has been spent yet.
 */
function budgetOf(
  remaining: Record<string, unknown>,
  limits: Record<string, unknown>,
): ThreadBudget {
  return {
    calls: number(remaining["calls"] ?? limits["max_calls"]),
    totalTokens: number(remaining["total_tokens"] ?? limits["max_total_tokens"]),
    costMicrousd: number(remaining["cost_microusd"] ?? limits["max_cost_microusd"]),
  };
}

/**
 * The budget in the width a status line has for it.
 *
 * An agent that exhausts its budget mid-edit without ever having shown one is
 * an agent that lost the work, so all three ceilings are named: the call count
 * is what usually runs out first, and the other two are what a long turn or an
 * expensive model runs into instead.
 */
export function formatBudget(budget: ThreadBudget): string {
  return `${budget.calls} calls · ${compact(budget.totalTokens)} tok · ${dollars(budget.costMicrousd)}`;
}

function compact(tokens: number): string {
  // The threshold is where the K form would round to four digits, so a ceiling
  // of a million reads `1.0M` before a turn is spent and `1.0M` after it.
  if (tokens >= 999_500) return `${(tokens / 1_000_000).toFixed(1)}M`;
  if (tokens >= 1_000) return `${Math.round(tokens / 1_000)}K`;
  return `${tokens}`;
}

function dollars(microusd: number): string {
  return `$${(microusd / 1_000_000).toFixed(2)}`;
}

function record(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function string(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function number(value: unknown): number {
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

/**
 * A call's arguments as an object.
 *
 * A model that emits invalid JSON must reach the tool anyway: the tool's own
 * refusal ("prompt is required") is a sentence the model can act on, and a
 * parse error thrown here would end the turn instead.
 */
function parseArguments(args: string): Record<string, unknown> {
  return parse(args) ?? {};
}
