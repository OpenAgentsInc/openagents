/**
 * A reply source backed by a thread of the caller's own, and the grant that
 * thread mints.
 *
 * `openagents coder` used to submit through `POST /api/v1/chat/turns` and poll
 * `GET /api/v1/chat/events`. That was the only route a user token could reach a
 * model through, and the server records one conversation per account, so every
 * prompt a person typed in a terminal landed in the same conversation `/chat`
 * reads, contended for the one streaming slot that conversation admits, and
 * became provider context for the next question asked in the browser. This
 * replaces both paths.
 *
 * `POST /api/v1/threads` opens a thread and returns a grant. The grant is the
 * bearer for `POST /api/inference/proxy`, an OpenAI-compatible
 * `/chat/completions` surface that meters against the thread's own budget and
 * keeps the provider credential on the server, so the CLI still holds no
 * provider key. `DELETE /api/v1/threads/{id}` revokes the thread on exit, which
 * matters because an account may hold only eight open threads at once and a
 * closed terminal would otherwise hold a slot until its authority was spent.
 *
 * Three properties of that proxy shape this file, and each is a real loss
 * against the event log this replaces:
 *
 * - **It answers in one piece.** The proxy builds the whole SSE body and sends
 *   it once, so the frames below all arrive together. The parser is written
 *   against the stream anyway rather than against `await response.text()`, so
 *   chunked delivery becomes visible here the day the server sends it, with no
 *   change on this side.
 * - **It carries reasoning as `delta.reasoning`.** Since openagents.com
 *   `c26c188` the proxy forwards a model's thinking as string chunks on
 *   `choices[0].delta.reasoning`, interleaved with `delta.content` in stream
 *   order. The parser below turns each into a `reasoning` chunk, which the
 *   interface renders as its dim-italic reasoning entry and the transcript
 *   writer records whole as `turn.reasoning`.
 * - **Tools run here.** The chat lane ran tools on the server and reported each
 *   one. The proxy is a bare completions surface: it forwards the `tools` a
 *   caller declares and returns the calls the model asks for, and the caller
 *   executes them. So the loop below is the tool runtime — `useTools` gives it
 *   the tools a session declares, and a turn continues until the model stops
 *   asking for one.
 *
 *   A tool exchange is fed back in the standard chat shape: the assistant
 *   message carries its `tool_calls` array (content may be empty), and each
 *   result follows as a `role: "tool"` message named by `tool_call_id`. The
 *   same `c26c188` made the proxy replay both faithfully to the provider, so
 *   the plain-turn paraphrase this file used to send — `[tool call]` and
 *   `[tool result]` written into user turns — is gone.
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
import { budgetedResult, describeBudget, toolResultBudget } from "./coder-tool-budget.js";
import { declaredDescription, toolFamilyOf } from "./coder-tool-families.js";
import { merge } from "./coder-merge.js";
import type { ReplyChunk, ReplySource } from "./coder-session.js";
import type { CoderTool } from "./coder-tools.js";
import { systemPrompt, THREAD_LANE } from "./coder-system.js";
import { coderTierLabel, tierLabel } from "./coder-tiers.js";
import type { TranscriptSink } from "./coder-transcript.js";
import { THREADS_PATH } from "./constants.js";

/**
 * How many rounds of tool calls one turn may take before it has to answer.
 *
 * A backstop against a model that loops forever, not a budget. It was six, and
 * six is a number real work passes: a session reading a package hit it after
 * twenty steps and ended saying it had stopped, throwing away everything it had
 * read. Escape stops a turn at any time, and that is the control that should
 * decide when enough is enough.
 */
const MAX_TOOL_STEPS = 100;

/**
 * How much of one tool's output reaches the durable `tool.ran` event.
 *
 * A separate figure from the model-facing budget in `coder-tool-budget.ts`,
 * because they answer different questions. That one is a context-budget
 * decision made against a model's window, per family: it is re-sent on every
 * round of the turn. This one bounds a record written once, so it is set where
 * every result a real session has produced fits whole — the largest measured
 * was 8.4 KB — and only a pathological dump is cut, kept at both ends the same
 * way.
 */
const EVENT_RESULT_KEPT = 64_000;

/** A long tool output, kept at both ends, which is what it is read for. */
const bounded = (output: string, keep: number): string => {
  if (output.length <= keep) return output;
  const half = Math.floor(keep / 2);
  const cut = output.length - keep;
  return `${output.slice(0, half)}\n\n[${String(cut)} of ${String(output.length)} characters omitted from the middle; run it again more narrowly if you need them]\n\n${output.slice(-half)}`;
};

/** What the thread may still spend, as the server last reported it. */
export interface ThreadBudget {
  /** `undefined` where the server set no ceiling: there is nothing counting down. */
  readonly calls: number | undefined;
  readonly totalTokens: number | undefined;
  readonly costMicrousd: number | undefined;
}

export interface ThreadOptions {
  readonly origin: string;
  /** The account token. Opens, reads, and revokes the thread; never spends it. */
  readonly token: string;
  /** What this body of work is for. The server requires one. */
  readonly objective: string;
  /**
   * The repository the work concerns, as `owner/name`. Recorded structurally
   * on the thread so `--resume` filters the picker on the server's field
   * rather than parsing the objective sentence back (openagents.com #210).
   */
  readonly repository?: string | undefined;
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
      ...(options.repository === undefined ? {} : { repository: options.repository }),
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
    proxyUrl: resolveProxyUrl(url, options.origin),
    model,
    auto: options.model === undefined,
    budget: budgetOf(record(grant["limits"]), record(grant["limits"])),
  });
}

export interface ResumeGrantOptions {
  readonly origin: string;
  /** The account token that owns the thread. */
  readonly token: string;
  /** The open thread `--resume` is continuing. */
  readonly threadId: string;
}

/**
 * Continue an existing thread by asking the server to re-mint its authority.
 *
 * `OpenAgents.Threads.mint_grant/1` is the server's fence for exactly this:
 * it revokes every active grant naming the thread, bumps the thread's
 * generation, and mints fresh authority against the same thread — the grant
 * lineage a resume is supposed to continue. This client asks for that at
 * `POST /api/v1/threads/{id}/grants`.
 *
 * Today the server publishes no such route. `GET /api/v1/threads/{id}` reports
 * the grant's status and limits but never its token — the plaintext exists
 * exactly once, at minting — so there is no other honest way to spend an
 * existing thread. A 404 here is therefore the server saying it cannot yet
 * re-grant, and it is reported as exactly that (`grant_unavailable`) rather
 * than as the thread being missing: the caller has already fetched the thread
 * by the time it asks for authority.
 */
export async function remintThread(options: ResumeGrantOptions): Promise<ThreadReplySource> {
  const response = await fetch(
    new URL(`${THREADS_PATH}/${options.threadId}/grants`, options.origin),
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${options.token}`,
        "content-type": "application/json",
        accept: "application/json",
      },
      body: JSON.stringify({}),
    },
  ).catch((cause: unknown) => {
    throw new ThreadUnavailable(
      "network_refused",
      `The API at ${options.origin} could not be reached: ${String(cause)}`,
    );
  });

  const body = (await response.json().catch(() => ({}))) as Record<string, unknown>;

  if (response.status === 401 || response.status === 403) {
    throw new ThreadUnavailable(
      "scope_missing",
      "This token cannot mint a grant for this thread. Run `openagents auth login` to sign in again.",
      response.status,
    );
  }
  if (response.status === 404) {
    throw new ThreadUnavailable(
      "grant_unavailable",
      "This server cannot hand back authority for an existing thread: " +
        "GET /api/v1/threads/{id} reports the grant without its token, and " +
        "POST /api/v1/threads/{id}/grants is not there to re-mint one. " +
        "The transcript is readable, but new turns cannot spend this thread " +
        "until the server can re-grant it.",
      response.status,
    );
  }
  if (response.status < 200 || response.status >= 300) {
    const code = typeof body["code"] === "string" ? body["code"] : `http_${response.status}`;
    const message =
      typeof body["message"] === "string"
        ? body["message"]
        : `The server refused to re-mint this thread's grant (${code}).`;
    throw new ThreadUnavailable(code, message, response.status);
  }

  const grant = record(body["grant"]);
  const token = string(grant["token"]);
  const url = string(grant["url"]);
  const model = string(grant["model"]);

  if (token === undefined || url === undefined || model === undefined) {
    throw new ThreadUnavailable(
      "malformed_thread",
      "The server re-minted this thread but did not return the grant needed to spend it.",
    );
  }

  return new ThreadReplySource({
    origin: options.origin,
    accountToken: options.token,
    threadId: string(record(body["thread"])["id"]) ?? options.threadId,
    grantToken: Redacted.make(token),
    proxyUrl: resolveProxyUrl(url, options.origin),
    model,
    // A resumed thread continues on the model its grant pins; the tier it
    // shows is the pinned model's tier rather than a remembered "auto".
    auto: false,
    budget: budgetOf(record(grant["remaining"]), record(grant["limits"])),
  });
}

interface SourceState {
  readonly origin: string;
  readonly accountToken: string;
  readonly threadId: string;
  readonly grantToken: Redacted.Redacted<string>;
  readonly proxyUrl: string;
  readonly model: string;
  /**
   * Opened without naming a model, so the server picks the lane. The proxy
   * body then names none either: a grant pinned to the server default with no
   * model in the body is the shape the server answers by choosing.
   */
  readonly auto: boolean;
  readonly budget: ThreadBudget;
}

/** One call in an assistant message, in the chat-completions wire shape. */
export interface WireToolCall {
  readonly id: string;
  readonly type: "function";
  readonly function: { readonly name: string; readonly arguments: string };
}

/**
 * One chat-completions message, which is what the proxy takes as its input.
 *
 * The `arguments` a model produced stay the raw JSON string on the way back:
 * the proxy replays them to the provider without interpreting them, and a
 * parse-and-reserialize here could reorder keys or normalize whitespace in a
 * string the provider expects byte for byte.
 *
 * Exported because a resume rebuilds this transcript from the thread's durable
 * events and hands it back through `preload`, and the two sides of that
 * exchange have to agree on the shape.
 */
export type WireMessage =
  // The session's own anchor, first and once. The proxy passes system messages
  // through to the provider and composes none of its own.
  | { readonly role: "system"; readonly content: string }
  | { readonly role: "user"; readonly content: string }
  | {
      readonly role: "assistant";
      readonly content: string;
      readonly tool_calls?: ReadonlyArray<WireToolCall>;
    }
  | { readonly role: "tool"; readonly tool_call_id: string; readonly content: string };

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
  /**
   * Where the turn loop writes the durable transcript, when the session has
   * one. Absent, nothing is recorded — the offline and local lanes never
   * attach one — and every call below is a no-op through optional chaining.
   */
  private sink: TranscriptSink | undefined;
  /** The running turn's token usage, accumulated across its model calls. */
  private turnUsage: {
    promptTokens: number;
    completionTokens: number;
    totalTokens: number;
    calls: number;
    cacheReadInputTokens?: number;
  } = { promptTokens: 0, completionTokens: 0, totalTokens: 0, calls: 0 };
  /** Set for the one round that must answer rather than call another tool. */
  private mustAnswer = false;
  /**
   * Messages that arrived mid-turn, waiting for the next step of it.
   *
   * Read between two model calls rather than at the end of the turn, which is
   * the difference between steering a model and waiting one out.
   */
  private steered: string[] = [];

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
    return this.state.auto ? tierLabel("auto") : coderTierLabel(this.state.model);
  }

  /** The vendor id the grant pins, for records; never rendered. */
  get modelId(): string {
    return this.state.model;
  }

  /**
   * The wire transcript so far, without the system anchor.
   *
   * A tier switch hands this to the source that continues the conversation;
   * the next source composes its own anchor, so handing it two would read as
   * the reader having typed one.
   */
  history(): ReadonlyArray<WireMessage> {
    return this.transcript.filter((message) => message.role !== "system");
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
  /** The session's workspace facts and active skills, for the system message. */
  private standing: string | undefined;

  useContext(standing: string): void {
    this.standing = standing;
  }

  useTools(tools: ReadonlyArray<CoderTool>): void {
    this.tools = tools;
  }

  /**
   * Attach the writer that puts this session's turns on the server.
   *
   * Set after construction for the same reason `useTools` is: the writer needs
   * the thread's id, so it cannot exist until the thread does, and the failure
   * notice it surfaces needs the session, which is built later still. The
   * server copy is the only durable copy — this process keeps no transcript
   * file of its own.
   */
  useTranscript(sink: TranscriptSink): void {
    this.sink = sink;
  }

  /**
   * What this lane sends as standing context.
   *
   * Both halves are this process's own and are reported in full. This once
   * said the server composed the system message, which was not true — the
   * proxy passes system messages through from the request and composes none —
   * and the honest reading of that claim was that the lane sent no system
   * message at all, which is exactly what it did.
   */
  /** The tools as declared, in the shape ATIF records them. */
  toolDefinitions(): ReadonlyArray<Record<string, unknown>> {
    // The family follows the vendor id: the label is a Coder tier and
    // deliberately says nothing about which family answers.
    const family = toolFamilyOf(this.modelId);
    return this.tools.map((tool) => ({
      type: "function",
      function: {
        name: tool.name,
        description: declaredDescription(tool, family),
        parameters: tool.parameters,
      },
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
      `System message sent with every turn:\n\n${systemPrompt(this.tools, THREAD_LANE, this.standing)}`,
      "",
      declarations,
      "",
      describeBudget(toolResultBudget(toolFamilyOf(this.modelId))),
    ].join("\n");
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

  /**
   * Seed the model transcript with a resumed thread's replayed history.
   *
   * Straight onto the wire transcript and nowhere else: the durable copy on
   * the server already holds these turns, so the sink is deliberately not
   * touched — a resume must never re-post events the thread already carries.
   * Called once, before the first new turn.
   */
  preload(messages: ReadonlyArray<WireMessage>): void {
    for (const message of messages) this.transcript.push(message);
  }

  /** Take a message for the next step of the running turn. */
  steer(text: string): boolean {
    this.steered.push(text);
    return true;
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    // Per turn, not per session: a turn that had to answer without tools must
    // not leave the next one without them.
    this.mustAnswer = false;
    this.turnUsage = { promptTokens: 0, completionTokens: 0, totalTokens: 0, calls: 0 };
    /** The answer so far, across steps, for the one `turn.assistant` event. */
    let turnText = "";
    /** How many tools this turn ran, reported on `turn.assistant`. */
    let turnToolCalls = 0;
    // The anchor goes on once, ahead of everything. Without it the model
    // answered "who are you" with the name of whatever it was underneath, and
    // listed tools from what a coding agent usually has rather than from what
    // this session declared.
    //
    // Composed at the first turn rather than at construction because the tools
    // and the context are both set after it, and a system message written
    // before them would name neither.
    if (!this.transcript.some((message) => message.role === "system")) {
      this.transcript.unshift({
        role: "system",
        content: systemPrompt(this.tools, THREAD_LANE, this.standing),
      });
    }

    this.transcript.push({ role: "user", content: prompt });
    this.sink?.record("turn.user", { text: prompt });

    try {
      for (let step = 0; ; step += 1) {
        // Anything the reader said since the last step joins here, before the
        // model is asked again.
        const steered = this.steered.splice(0);
        for (const said of steered) {
          this.transcript.push({ role: "user", content: said });
          // Steered mid-turn rather than asked between turns, and the record
          // says so, or a replay would show a question the answer ignores.
          this.sink?.record("turn.user", { text: said, steered: true });
        }
        // The interface dims a steered message until this says it was read.
        if (steered.length > 0) yield { type: "steered", texts: steered };

        const calls: WireCall[] = [];
        let assistant = "";
        let reasoning = "";

        for await (const chunk of this.stream(signal, calls)) {
          if (signal.aborted) break;
          if (chunk.type === "text") assistant += chunk.value;
          if (chunk.type === "reasoning") reasoning += chunk.value;
          yield chunk;
        }

        // One event per block, whole, never deltas: the record is what was
        // thought, not the pieces it arrived in.
        if (reasoning.length > 0) this.sink?.record("turn.reasoning", { text: reasoning });

        // Whatever the model said belongs to the thread even when the turn was
        // interrupted, or the next turn answers a question it cannot see it
        // half-answered.
        if (assistant.length > 0) {
          turnText = turnText.length === 0 ? assistant : `${turnText}\n\n${assistant}`;
        }
        if (signal.aborted || calls.length === 0) {
          if (assistant.length > 0) this.transcript.push({ role: "assistant", content: assistant });
          // The turn's cost reaches the session too, not only the durable
          // transcript: the closing entry carries it into the ATIF export,
          // where a benchmark computes cost per outcome from the trajectory
          // alone. Same shape and place as the local lane's report.
          if (this.turnUsage.calls > 0) {
            yield {
              type: "usage",
              promptTokens: this.turnUsage.promptTokens,
              completionTokens: this.turnUsage.completionTokens,
              calls: this.turnUsage.calls,
              ...(this.turnUsage.cacheReadInputTokens === undefined
                ? {}
                : { cacheReadInputTokens: this.turnUsage.cacheReadInputTokens }),
            };
          }
          this.recordAnswer(turnText, turnToolCalls, signal.aborted);
          return;
        }

        if (step >= MAX_TOOL_STEPS) {
          // Take the tools away for one more round rather than stopping on a
          // tool result. The work already done is the reason the turn is long,
          // and ending on "stopped" throws all of it away. The calls are
          // dropped rather than written: an assistant `tool_calls` message
          // whose results never follow is a transcript the provider refuses.
          if (assistant.length > 0) this.transcript.push({ role: "assistant", content: assistant });
          this.mustAnswer = true;
          this.transcript.push({
            role: "user",
            content:
              "You have reached this turn's limit on tool calls. Do not call another tool. " +
              "Answer now with what you have found, and say plainly what is still unfinished.",
          });
          continue;
        }

        // The exchange in the standard chat shape: one assistant message
        // carrying every call of the round — content may be empty, and the
        // arguments stay the raw JSON string the model produced — then, after
        // the tools have run, one `tool` message per result in the order the
        // calls were made, whatever order they finished in.
        this.transcript.push({
          role: "assistant",
          content: assistant,
          tool_calls: calls.map((call) => ({
            id: call.id,
            type: "function" as const,
            function: { name: call.name, arguments: call.args },
          })),
        });
        turnToolCalls += calls.length;
        // Concurrently. A model asking for two tools in one turn is saying they do
        // not depend on each other, and running them in order anyway makes a fan-out
        // to two models cost the sum of both.
        const results = new Map<string, string>();
        yield* merge(calls.map((call) => this.invoke(call, signal, results)));
        for (const call of calls) {
          this.transcript.push({
            role: "tool",
            tool_call_id: call.id,
            content: results.get(call.id) ?? "",
          });
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
   * Record the turn's answer, with what it cost.
   *
   * One event per turn, whatever the turn took to get there: several model
   * calls, several tool rounds, one figure each. An interrupted turn is
   * recorded too, marked as such, because whatever streamed before Escape was
   * said and the next reader of this thread will be answered against it.
   */
  private recordAnswer(text: string, toolCalls: number, interrupted: boolean): void {
    if (this.sink === undefined) return;
    if (text.length === 0 && this.turnUsage.calls === 0) return;
    this.sink.record("turn.assistant", {
      text,
      usage: {
        prompt_tokens: this.turnUsage.promptTokens,
        completion_tokens: this.turnUsage.completionTokens,
        total_tokens: this.turnUsage.totalTokens,
        calls: this.turnUsage.calls,
        ...(this.turnUsage.cacheReadInputTokens === undefined
          ? {}
          : { cache_read_input_tokens: this.turnUsage.cacheReadInputTokens }),
      },
      tool_calls: toolCalls,
      ...(interrupted ? { interrupted: true } : {}),
    });
  }

  /**
   * Run one call, report it, and leave its result for the caller to file.
   *
   * The result goes into `results` under the call's id rather than onto the
   * transcript here: calls of one round run concurrently and finish in any
   * order, and the turn loop writes the `tool` messages afterward in the
   * order the calls were made, so the wire history is deterministic.
   */
  private async *invoke(
    call: WireCall,
    signal: AbortSignal,
    results: Map<string, string>,
  ): AsyncIterable<ReplyChunk> {
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

    // Call and result are one fact, so they are one event. The tool's name is
    // its identity for later attribution — a plugin's tool carries the name it
    // was declared under — and the result is bounded far above the model-wire
    // bound, where every result a real session has produced is stored whole.
    this.sink?.record("tool.ran", {
      call_id: call.id,
      tool: call.name,
      arguments: bounded(call.args, EVENT_RESULT_KEPT),
      status: failure === undefined ? "succeeded" : "failed",
      ...(failure === undefined
        ? { output: bounded(output, EVENT_RESULT_KEPT) }
        : { error: bounded(failure, EVENT_RESULT_KEPT) }),
    });

    // Budgeted on the way toward the model, not on the way to the reader or the
    // record: this is what goes back on every round after, and a session that
    // re-sends everything it has already read spends its wall clock on reading
    // it again. The allowance is this model family's, and a cut result says so.
    // The `tool.ran` event above kept the fuller copy.
    results.set(call.id, budgetedResult(output, toolFamilyOf(this.modelId)));
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
  /**
   * One call to the proxy, retried while the failure is one a retry can fix.
   *
   * A provider failure is a `502` the server produced *before* any of the
   * stream reached this client, so re-sending is the same call rather than a
   * duplicated one. Dropping the turn on the first one is what put "The model
   * provider failed" on screen twice in a row with the work lost both times.
   *
   * Only transient classes are retried. A revoked, expired, or exhausted grant
   * is settled — retrying it spends the reader's time to be told the same thing
   * three times — and every 4xx is a request this client would send again
   * unchanged. A retry does re-spend budget where the failed call was metered
   * for partial usage, which is why the ceiling is low.
   */
  private async callProxy(signal: AbortSignal): Promise<Response | undefined> {
    const attempts = 3;

    for (let attempt = 1; ; attempt += 1) {
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
          ...(this.state.auto ? {} : { model: this.state.model }),
          stream: true,
          messages: this.transcript,
          ...(this.tools.length === 0 || this.mustAnswer
            ? {}
            : {
                // Declarations resolve per model family: the base says what
                // a tool is, and a family override adds the emphasis that
                // family has measurably needed (coder-tool-families.ts).
                tools: this.tools.map((tool) => ({
                  type: "function",
                  function: {
                    name: tool.name,
                    description: declaredDescription(tool, toolFamilyOf(this.modelId)),
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

      if (response === undefined || signal.aborted) return undefined;
      if (response.status >= 200 && response.status < 300) return response;

      const refusal = await proxyRefusal(response);
      const transient =
        response.status === 502 || response.status === 503 || response.status === 504;
      if (!transient || attempt >= attempts) throw refusal;

      // Short and fixed. The failure is on the provider's side and a reader is
      // watching a cursor; a long backoff reads as a hang.
      await new Promise((resolve) => setTimeout(resolve, 400 * attempt));
      if (signal.aborted) return undefined;
    }
  }

  private async *stream(signal: AbortSignal, collected: WireCall[]): AsyncIterable<ReplyChunk> {
    const response = await this.callProxy(signal);
    if (response === undefined || signal.aborted) return;
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

        // Reasoning precedes the words it produced, and the proxy interleaves
        // the two in stream order, so within one delta it is yielded first.
        const thought = delta["reasoning"];
        if (typeof thought === "string" && thought.length > 0) {
          yield { type: "reasoning", value: thought };
        }

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
    // A ceiling that was never set has no remainder to decrement. Counting one
    // down from `undefined` would have invented a limit the server does not
    // hold, and shown it running out.
    this.remaining = {
      calls: this.remaining.calls === undefined ? undefined : Math.max(0, this.remaining.calls - 1),
      totalTokens:
        this.remaining.totalTokens === undefined
          ? undefined
          : Math.max(0, this.remaining.totalTokens - total),
      costMicrousd: this.remaining.costMicrousd,
    };
    // The same report feeds the turn's own tally, which `turn.assistant`
    // carries: a turn is several calls, and the record holds their sum with
    // the count rather than the last call's figures presented as the turn's.
    const cache = optional(usage["cache_read_input_tokens"]);
    this.turnUsage = {
      promptTokens: this.turnUsage.promptTokens + number(usage["prompt_tokens"]),
      completionTokens: this.turnUsage.completionTokens + number(usage["completion_tokens"]),
      totalTokens: this.turnUsage.totalTokens + total,
      calls: this.turnUsage.calls + 1,
      ...(cache === undefined
        ? {}
        : { cacheReadInputTokens: (this.turnUsage.cacheReadInputTokens ?? 0) + cache }),
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
export async function* frames(
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

export function parse(frame: string): Record<string, unknown> | undefined {
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
export function accumulate(
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
  const error = record(body["error"]);
  const code = string(error["code"]) ?? `http_${response.status}`;

  const sentences: Record<string, string> = {
    // Only reachable when someone deliberately revoked this thread — a session
    // no longer does it on the way out. The sentence says what to do rather
    // than naming a lifecycle the reader did not ask about.
    grant_revoked: "This thread is no longer live. Start a new session to open another.",
    // Not a sentence about a clock. A thread's authority has no deadline —
    // this reaches a caller only where the deployment minted one that does,
    // and it says what the reader can act on rather than naming an expiry a
    // coder session cannot have.
    grant_expired:
      "This thread's authority is no longer live. Start a new session to open another.",
    grant_exhausted: "This thread spent its budget. Start a new session to open another.",
    grant_budget_reached: "This thread reached its budget ceiling and cannot buy another call.",
    invalid_grant: "The inference proxy did not recognize this thread's grant.",
    provider_failed: "The model provider failed. The call was not completed.",
  };

  // The server names the failure class on a provider failure. It is one bounded
  // word, and it is the difference between a reader who knows the call ran out
  // of context and one who only knows something went wrong.
  const why = string(error["reason"]);
  const sentence = sentences[code] ?? `The inference proxy refused the call (${code}).`;

  return new ThreadUnavailable(
    code,
    why === undefined ? sentence : `${sentence} (${why})`,
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
  // `null` from the server means unbounded, and is carried as `undefined`
  // rather than collapsed to zero. A ceiling of nothing left and a ceiling that
  // was never set are opposite facts, and `number()` would have printed both as
  // `0 calls` — a session with no limit reading as one with none remaining.
  return {
    calls: optional(remaining["calls"] ?? limits["max_calls"]),
    totalTokens: optional(remaining["total_tokens"] ?? limits["max_total_tokens"]),
    costMicrousd: optional(remaining["cost_microusd"] ?? limits["max_cost_microusd"]),
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
  // Only the ceilings that exist. A thread with none shows nothing here, which
  // is the honest reading of a session nothing is counting down.
  const parts = [
    budget.calls === undefined ? undefined : `${String(budget.calls)} calls`,
    budget.totalTokens === undefined ? undefined : `${compact(budget.totalTokens)} tok`,
    budget.costMicrousd === undefined ? undefined : dollars(budget.costMicrousd),
  ].filter((part): part is string => part !== undefined);

  return parts.join(" · ");
}

/** A number the server gave, or `undefined` where it gave `null` for "no limit". */
function optional(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
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

/**
 * The grant names the proxy with the server's own idea of its host, and
 * inside a container the server's "localhost" is the wrong machine. The
 * path is the server's contract; the origin is the client's — the same one
 * it authenticated against — so the URL resolves against it.
 */
export const resolveProxyUrl = (grantUrl: string, origin: string): string => {
  try {
    const named = new URL(grantUrl);
    return new URL(named.pathname, origin).toString();
  } catch {
    return grantUrl;
  }
};

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
export function parseArguments(args: string): Record<string, unknown> {
  return parse(args) ?? {};
}
