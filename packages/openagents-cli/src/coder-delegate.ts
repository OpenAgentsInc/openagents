/**
 * Delegation for `openagents coder`: run many coding agents at once.
 *
 * One console cannot do fifteen things, so it hands each of them to a child
 * coding agent and watches all of them. This module is the seam between the two
 * halves that makes that safe to build on:
 *
 * - A `DelegateHarness` is anything that can run a prompt to completion while
 *   reporting normalized events. `OpencodeHarness` is the first one, and it
 *   drives the `opencode` CLI in its JSON event mode. A fake harness in the
 *   tests drives the same interface, so scheduling, cancellation, and rendering
 *   are testable without a model.
 * - A `DelegateFleet` owns the concurrency cap, the queue, and the writes into
 *   `CoderTaskRegistry`. Nothing else starts children.
 *
 * Three decisions here are worth the words:
 *
 * A refusal is a result, not an exception. Hitting the cap, an unknown harness,
 * and a missing worktree are all ordinary outcomes of asking for massive
 * fan-out, and a caller that has to catch exceptions to find out cannot report
 * them per child.
 *
 * The child's raw event stream is written to a file as it arrives. That file is
 * the child's transcript, and it exists whether or not anyone was watching,
 * which is the difference between a fleet you can review afterwards and a fleet
 * that only existed on screen.
 *
 * The parser is a pure function over one line. A harness that changes its event
 * shape then breaks one small tested function rather than the scheduler.
 */

import type { ChildProcess } from "node:child_process";
import { execFileSync, spawn } from "node:child_process";
import { createWriteStream, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { CoderTaskId, CoderTaskRegistry, CoderToolActivity } from "./coder-tasks.js";

/** What the console asks for. One shape whether it wants one child or fifteen. */
export interface DelegationRequest {
  /** Three to five words. This is what every compact surface shows. */
  readonly description: string;
  readonly prompt: string;
  /** Where the child works. Defaults to the console's own directory. */
  readonly cwd?: string | undefined;
  /** False to await the child inline; true to leave it running. */
  readonly background?: boolean | undefined;
}

/**
 * What a launch produced.
 *
 * `refused` carries a stable code so a caller can react to the reason rather
 * than to prose, and text so a person or a model reading it knows what to do
 * instead.
 */
export type DelegationOutcome =
  | { readonly status: "completed"; readonly taskId: CoderTaskId; readonly result: string }
  | { readonly status: "failed"; readonly taskId: CoderTaskId; readonly error: string }
  | { readonly status: "stopped"; readonly taskId: CoderTaskId }
  | { readonly status: "refused"; readonly code: RefusalCode; readonly reason: string };

export type RefusalCode = "fleet_full" | "empty_prompt" | "harness_unavailable";

/** A `/delegate` line the console typed, once understood. */
export interface DelegateCommand {
  /** How many children to launch with this prompt. */
  readonly count: number;
  readonly prompt: string;
  readonly description: string;
}

/** How many children one `/delegate` line may ask for. */
export const MAX_DELEGATE_COUNT = 32;

/**
 * Read a `/delegate` line.
 *
 * The grammar is `/delegate [<n>x] <prompt>`, so `/delegate 4x add tests to the
 * parser` launches four children on the same prompt. The count is a separate
 * leading token rather than a flag because the console is a chat box, not a
 * shell, and `--agents 4` in the middle of an English sentence reads as part of
 * the prompt.
 *
 * Returns undefined for anything that is not a delegate line, so an ordinary
 * prompt is unaffected.
 */
export function parseDelegateCommand(text: string): DelegateCommand | undefined {
  const match = /^\/delegate(?:\s+([\s\S]*))?$/.exec(text.trim());
  if (match === null) return undefined;

  let rest = (match[1] ?? "").trim();
  let count = 1;
  const fanout = /^(\d{1,3})x\s+([\s\S]+)$/.exec(rest);
  if (fanout !== null) {
    count = Math.min(MAX_DELEGATE_COUNT, Math.max(1, Number(fanout[1])));
    rest = (fanout[2] ?? "").trim();
  }

  return { count, prompt: rest, description: describePrompt(rest) };
}

/**
 * A short label for a prompt.
 *
 * Every compact surface shows this and nothing else, so it has to be short
 * enough to sit in a column: the first few words, which is what a person would
 * have typed as a title anyway.
 */
export function describePrompt(prompt: string): string {
  const words = prompt.replace(/\s+/g, " ").trim().split(" ").filter(Boolean);
  if (words.length === 0) return "delegated task";
  return words.slice(0, 5).join(" ");
}

/** A child's activity, normalized away from any one harness's event shape. */
export type DelegateEvent =
  | { readonly type: "session"; readonly sessionId: string }
  | {
      readonly type: "tool";
      readonly callId: string;
      readonly name: string;
      readonly target: string | undefined;
    }
  | { readonly type: "text"; readonly value: string }
  | { readonly type: "tokens"; readonly input: number; readonly output: number }
  | { readonly type: "error"; readonly message: string };

/** What the fleet needs from a way of running children. */
export interface DelegateHarness {
  /** Shown in the fleet block, for example `opencode`. */
  readonly agent: string;
  /** The child model, shown beside the agent. */
  readonly model: string;
  /**
   * Run `prompt` in `cwd`, yielding events as they happen.
   *
   * The harness must return when the child is done, throw when it could not be
   * run, and stop promptly when `signal` aborts. Everything else — retries,
   * permissions, provider credentials — is the harness's business.
   */
  run(
    input: { readonly prompt: string; readonly cwd: string; readonly transcriptPath: string },
    signal: AbortSignal,
  ): AsyncIterable<DelegateEvent>;
}

/**
 * Read one line of `opencode run --format json` output.
 *
 * Returns undefined for blank lines, for lines that are not JSON, and for event
 * kinds the fleet does not track. A harness that adds an event kind must not be
 * able to stop a fleet, so anything unrecognized is dropped rather than raised.
 */
export function parseOpencodeEvent(line: string): DelegateEvent | undefined {
  const trimmed = line.trim();
  if (trimmed.length === 0 || !trimmed.startsWith("{")) return undefined;

  let event: Record<string, unknown>;
  try {
    event = JSON.parse(trimmed) as Record<string, unknown>;
  } catch {
    return undefined;
  }

  const type = typeof event["type"] === "string" ? (event["type"] as string) : undefined;
  const part = isRecord(event["part"]) ? event["part"] : undefined;

  if (type === "tool_use" && part !== undefined) {
    const name = stringField(part, "tool") ?? "tool";
    const callId = stringField(part, "callID") ?? `${name}-${String(event["timestamp"] ?? "")}`;
    const state = isRecord(part["state"]) ? part["state"] : undefined;
    return { type: "tool", callId, name, target: toolTarget(state) };
  }

  if (type === "text" && part !== undefined) {
    const value = stringField(part, "text");
    return value === undefined ? undefined : { type: "text", value };
  }

  if (type === "step_finish" && part !== undefined) {
    const tokens = isRecord(part["tokens"]) ? part["tokens"] : undefined;
    if (tokens === undefined) return undefined;
    return {
      type: "tokens",
      input: numberField(tokens, "input") ?? 0,
      output: numberField(tokens, "output") ?? 0,
    };
  }

  if (type === "error") {
    return { type: "error", message: describeHarnessError(event) };
  }

  const sessionId = stringField(event, "sessionID");
  if (sessionId !== undefined && type === "step_start") {
    return { type: "session", sessionId };
  }

  return undefined;
}

/**
 * The sentence behind a harness error event.
 *
 * opencode nests the sentence: the event carries an `error` with a `name` and a
 * `data` holding the `message` and a support `ref`. Reading only `message` off
 * the outer object — which is what this did — found nothing, so the fleet fell
 * back to the exit code and every failure on screen read `exited with code 1`,
 * which says nothing about the provider refusal, the missing credential, or the
 * unreachable endpoint that actually happened.
 */
function describeHarnessError(event: Record<string, unknown>): string {
  const error = isRecord(event["error"]) ? event["error"] : {};
  const data = isRecord(error["data"]) ? error["data"] : {};
  const sentence =
    stringField(event, "message") ??
    stringField(error, "message") ??
    stringField(data, "message") ??
    stringField(data, "error");
  const name = stringField(error, "name");
  const ref = stringField(data, "ref");

  const parts: string[] = [];
  if (name !== undefined && name !== "Error") parts.push(name);
  parts.push(sentence ?? "the child agent reported an error");
  const text = parts.join(": ");
  return ref === undefined ? text : `${text} (${ref})`;
}

/**
 * What the child was working on.
 *
 * The harness's own title is preferred because it is what the harness chose to
 * show; the input fields are a fallback for tools that set no title. A row that
 * says only `bash` is much less use than one that says the command.
 */
function toolTarget(state: Record<string, unknown> | undefined): string | undefined {
  if (state === undefined) return undefined;
  const title = stringField(state, "title");
  if (title !== undefined && title.length > 0) return title;
  const input = isRecord(state["input"]) ? state["input"] : undefined;
  if (input === undefined) return undefined;
  for (const key of ["filePath", "path", "command", "pattern", "query", "description", "url"]) {
    const value = stringField(input, key);
    if (value !== undefined && value.length > 0) return value;
  }
  return undefined;
}

export interface OpencodeHarnessOptions {
  /** `provider/model`, for example `vertex-express/gemini-3.7-flash`. */
  readonly model: string;
  /** Defaults to `opencode` on the path. */
  readonly command?: string | undefined;
  /**
   * A config file for the child, passed as `OPENCODE_CONFIG`.
   *
   * This is how a provider the harness is not configured for is supplied
   * without writing a key into the repository: the caller writes a config to a
   * private path and names it here.
   */
  readonly configPath?: string | undefined;
  /**
   * Approve the child's tool use without asking.
   *
   * A delegated child has nobody to ask, so a coding task that has to edit a
   * file or run a command needs this. It is off by default because the
   * decision belongs to whoever launches the fleet, and it should be paired
   * with worktree or sandbox isolation.
   */
  readonly autoApprove?: boolean | undefined;
  readonly env?: Readonly<Record<string, string | undefined>> | undefined;
}

/** How long a stopped child has to leave on its own before it is killed. */
const KILL_GRACE_MS = 3_000;

/**
 * Signal a child and everything it started.
 *
 * The negative pid is the process group, which is why the child is spawned
 * detached. It falls back to the child alone when the group is already gone,
 * because a group whose last member exited between the two calls raises rather
 * than reporting nothing to do.
 */
function killGroup(child: ChildProcess, signal: "SIGTERM" | "SIGKILL"): void {
  const pid = child.pid;
  if (pid === undefined) return;
  try {
    process.kill(-pid, signal);
  } catch {
    try {
      child.kill(signal);
    } catch {
      // Already gone, which is the outcome that was wanted.
    }
  }
}

/**
 * Every process descended from `root`, deepest last.
 *
 * A harness is free to put its own tool processes in groups of their own, and
 * opencode does: signalling the group takes out opencode and leaves whatever a
 * bash tool started running under pid 1. The tree has to be read while the
 * parent is still alive, because reparenting erases the link.
 */
function descendants(root: number): ReadonlyArray<number> {
  let listing: string;
  try {
    listing = execFileSync("ps", ["-A", "-o", "pid=,ppid="], { encoding: "utf8" });
  } catch {
    return [];
  }

  const byParent = new Map<number, Array<number>>();
  for (const line of listing.split("\n")) {
    const [pid, parent] = line.trim().split(/\s+/).map(Number);
    if (pid === undefined || parent === undefined) continue;
    if (!Number.isInteger(pid) || !Number.isInteger(parent)) continue;
    const siblings = byParent.get(parent);
    if (siblings === undefined) byParent.set(parent, [pid]);
    else siblings.push(pid);
  }

  const found: Array<number> = [];
  const walk = (pid: number): void => {
    for (const child of byParent.get(pid) ?? []) {
      if (found.includes(child)) continue;
      found.push(child);
      walk(child);
    }
  };
  walk(root);
  return found;
}

/** Signal a child, its group, and every process either of them started. */
function killTree(child: ChildProcess, signal: "SIGTERM" | "SIGKILL"): void {
  const pid = child.pid;
  const tree = pid === undefined ? [] : descendants(pid);
  killGroup(child, signal);
  for (const descendant of tree) {
    try {
      process.kill(descendant, signal);
    } catch {
      // Already gone, which is the outcome that was wanted.
    }
  }
}

/** Runs children as `opencode run --format json` subprocesses. */
/** How a Devin child is run. */
export interface DevinHarnessOptions {
  /** The binary, so a test can point at a stand-in. Defaults to `devin`. */
  readonly command?: string | undefined;
  /**
   * The permission mode passed through.
   *
   * `dangerous` is this build's name for the unattended mode -- the published
   * documentation calls it "bypass", and passing that is accepted and ignored,
   * so a child would silently fall back to prompting where nobody can answer.
   */
  readonly permissionMode?: string | undefined;
  /** Extra environment for the child. */
  readonly env?: Record<string, string> | undefined;
}

/**
 * Children run by the Devin CLI.
 *
 * A second harness rather than a shell command, so a Devin fan-out is a fleet
 * like any other: it reports through the registry the renderer reads, it can be
 * stopped with the rest, and it does not block the turn that started it. Run
 * through `shell` instead, the same work is one opaque call that freezes the
 * session and shows nothing while it runs.
 *
 * Devin's print mode has no structured output, so a child reports its answer
 * once at the end rather than streaming tool calls the way `opencode --format
 * json` does. The fleet still shows it start, run, and finish, which is the
 * part the reader is waiting on.
 *
 * Its own credentials are used, not this session's grant. That is a different
 * trust and billing boundary from an `opencode` child, and it is the reason the
 * agent is named in the fleet rather than left implicit.
 */
export class DevinHarness implements DelegateHarness {
  readonly agent = "devin";
  readonly model: string;

  constructor(private readonly options: DevinHarnessOptions = {}) {
    // Devin picks its own model from its own configuration, and print mode does
    // not report which. Naming one here would be inventing it.
    this.model = options.permissionMode ?? "dangerous";
  }

  async *run(
    input: { readonly prompt: string; readonly cwd: string; readonly transcriptPath: string },
    signal: AbortSignal,
  ): AsyncIterable<DelegateEvent> {
    const command = this.options.command ?? "devin";
    const mode = this.options.permissionMode ?? "dangerous";

    const child = spawn(
      command,
      [
        "-p",
        input.prompt,
        "--permission-mode",
        mode,
        // Print mode cannot show the trust prompt, so without this a child in a
        // directory nobody has opened Devin in exits before doing anything.
        "--respect-workspace-trust",
        "false",
      ],
      {
        cwd: input.cwd,
        env: { ...process.env, ...this.options.env },
        // No terminal: a child that would prompt gets end-of-file and stops
        // rather than waiting where the fleet shows it as still working.
        stdio: ["ignore", "pipe", "pipe"],
      },
    );

    const events: DelegateEvent[] = [];
    let resolveNext: (() => void) | undefined;
    const wake = () => {
      resolveNext?.();
      resolveNext = undefined;
    };

    let answer = "";
    let failure = "";
    let done = false;
    let startFailure: Error | undefined;

    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      answer += chunk;
    });
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk: string) => {
      failure += chunk;
    });

    const onAbort = () => child.kill("SIGKILL");
    signal.addEventListener("abort", onAbort, { once: true });

    child.on("error", (cause: Error) => {
      startFailure =
        (cause as NodeJS.ErrnoException).code === "ENOENT"
          ? new Error(`The \`${command}\` command is not on PATH.`)
          : cause;
      done = true;
      wake();
    });
    child.on("close", (code: number | null) => {
      const text = answer.trim();
      if (code === 0) {
        if (text.length > 0) events.push({ type: "text", value: text });
      } else {
        const said = `${failure.trim()}\n${text}`.trim();
        events.push({
          type: "error",
          message:
            said.length > 0
              ? said
              : `The \`${command}\` child exited with code ${String(code ?? -1)}.`,
        });
      }
      done = true;
      wake();
    });

    try {
      for (;;) {
        while (events.length > 0) {
          const next = events.shift();
          if (next !== undefined) yield next;
        }
        if (done) break;
        await new Promise<void>((resolve) => {
          resolveNext = resolve;
        });
      }
    } finally {
      signal.removeEventListener("abort", onAbort);
    }

    if (startFailure !== undefined) throw startFailure;
  }
}

/**
 * The models a child is given, in the order they are preferred.
 *
 * Free and grant-free, both on purpose. A thread grant lives an hour, has to be
 * minted, and expires under a console that outlives it — four children once
 * came back `grant_expired` together — while the harness's own catalog costs
 * nothing and needs no credential from us at all.
 *
 * Resolved against what the harness actually lists, so a name that goes away
 * falls through to the next rather than failing a fan-out.
 */
export const FREE_CHILD_MODELS: ReadonlyArray<string> = [
  // Ox Alpha, free and unlimited while it lasts. The slug says neither `ox`
  // nor `alpha`: opencode's own normalization maps `x-preview-f` to `ox-alpha`
  // (`packages/stats/core/src/domain/model-normalization.ts`), and the `-free`
  // entry is the unlimited tier the picker calls "Ox Alpha Free (Unlimited)".
  // Searching the model list for its name finds nothing, which is how a first
  // pass at this list picked a different model entirely.
  "opencode/x-preview-f-free",
  "opencode/gemini-3.7-flash",
  "opencode/gemini-3.6-flash",
  "opencode/gemini-3.5-flash",
];

/**
 * What a lane is called, and what it resolves to.
 *
 * The names are the ones people use. Ox Alpha's slug is
 * `opencode/x-preview-f-free` — it says neither `ox` nor `alpha`, because
 * opencode's normalization maps `x-preview-f` to `ox-alpha` elsewhere — so a
 * session offered only the slug is a session nobody can ask for Ox Alpha by
 * name. Asked "can you delegate to ox alpha", one answered no while holding
 * exactly that lane under a name it could not connect to the question.
 *
 * A slug still resolves to itself, so nothing that already worked stops.
 */
export const CHILD_LANE_ALIASES: Readonly<Record<string, string>> = {
  "ox-alpha": "opencode/x-preview-f-free",
  gemini: "opencode/gemini-3.7-flash",
};

/**
 * Every lane a `delegate` call may name, by the name a caller would use.
 *
 * Devin is here because it brings its own credentials rather than spending this
 * session's grant. Offered as an enum so a call chooses from what exists rather
 * than from what it remembers.
 */
export const CHILD_MODELS: ReadonlyArray<string> = [
  ...Object.keys(CHILD_LANE_ALIASES),
  ...FREE_CHILD_MODELS,
  "devin",
];

/**
 * The name a lane is known by, given its slug.
 *
 * So a lane reached by its slug still reports the name a reader would recognise,
 * whichever way it was reached.
 */
export const childLaneName = (lane: string): string =>
  Object.entries(CHILD_LANE_ALIASES).find(([, slug]) => slug === lane)?.[0] ?? lane;

/** The lane a name means, whether it is an alias, a slug, or Devin. */
export const resolveChildLane = (name: string): string | undefined => {
  const asked = name.trim();
  if (asked.length === 0) return undefined;
  if (/^devin(:.+)?$/.test(asked)) return asked;
  const aliased = CHILD_LANE_ALIASES[asked];
  if (aliased !== undefined) return aliased;
  return FREE_CHILD_MODELS.includes(asked) ? asked : undefined;
};

/**
 * The first preferred model the harness offers, or undefined when it lists none.
 *
 * A listing that cannot be read says nothing rather than guessing, for the same
 * reason the preflight does: a harness whose subcommand differs must not be
 * able to block a fan-out that would have worked.
 */
export async function firstAvailableChildModel(
  command = "opencode",
  preferred: ReadonlyArray<string> = FREE_CHILD_MODELS,
): Promise<string | undefined> {
  const listed = await new Promise<string>((resolve) => {
    const probe = spawn(command, ["models"], { stdio: ["ignore", "pipe", "ignore"] });
    let out = "";
    probe.stdout.setEncoding("utf8");
    probe.stdout.on("data", (chunk: string) => {
      out += chunk;
    });
    probe.on("error", () => resolve(""));
    probe.on("close", () => resolve(out));
  });
  if (listed.trim().length === 0) return undefined;
  const names = new Set(listed.split("\n").map((line) => line.trim()));
  return preferred.find((candidate) => names.has(candidate));
}

export class OpencodeHarness implements DelegateHarness {
  readonly agent = "opencode";
  readonly model: string;

  constructor(private readonly options: OpencodeHarnessOptions) {
    this.model = options.model;
  }

  /** The preflight, run once and shared by every child of this fleet. */
  private preflight: Promise<string | undefined> | undefined;

  /**
   * Check that the harness exists and knows the model, once per fleet.
   *
   * Without this, a model the harness cannot resolve fails inside its provider
   * and is reported as `Unexpected server error`, once per child — fifteen
   * identical sentences naming nothing. `opencode models` costs one process at
   * the start of a fan-out and turns that into the model id and the fact that
   * it is not on the list.
   *
   * A preflight that cannot answer says nothing rather than guessing: a
   * harness that lists no models, or a build whose subcommand differs, must
   * not be able to block a fleet that would have worked.
   */
  private check(command: string): Promise<string | undefined> {
    this.preflight ??= new Promise<string | undefined>((resolve) => {
      const probe = spawn(command, ["models"], {
        env: {
          ...process.env,
          ...this.options.env,
          ...(this.options.configPath === undefined
            ? {}
            : { OPENCODE_CONFIG: this.options.configPath }),
        },
        stdio: ["ignore", "pipe", "ignore"],
      });

      let listing = "";
      probe.stdout.setEncoding("utf8");
      probe.stdout.on("data", (chunk: string) => {
        listing += chunk;
      });

      probe.on("error", (cause: Error) => {
        resolve(
          (cause as NodeJS.ErrnoException).code === "ENOENT"
            ? `The \`${command}\` harness is not installed. Install it with ` +
                "`npm i -g opencode-ai`, or name another with --child-command."
            : `The \`${command}\` harness could not be started: ${cause.message}`,
        );
      });

      probe.on("close", (code) => {
        const models = listing
          .split("\n")
          .map((line) => stripAnsi(line).trim())
          .filter((line) => line.includes("/"));
        if (code !== 0 || models.length === 0 || models.includes(this.model)) {
          resolve(undefined);
          return;
        }
        resolve(
          `The \`${command}\` harness has no model \`${this.model}\`. ` +
            `It offers ${String(models.length)}, including ${models.slice(0, 3).join(", ")}.`,
        );
      });
    });
    return this.preflight;
  }

  async *run(
    input: { readonly prompt: string; readonly cwd: string; readonly transcriptPath: string },
    signal: AbortSignal,
  ): AsyncIterable<DelegateEvent> {
    const command = this.options.command ?? "opencode";

    const problem = await this.check(command);
    if (problem !== undefined) {
      yield { type: "error", message: problem };
      throw new Error(problem);
    }

    const args = ["run", "--format", "json", "--model", this.model, "--dir", input.cwd];
    if (this.options.autoApprove === true) args.push("--auto");
    args.push(input.prompt);

    const child = spawn(command, args, {
      cwd: input.cwd,
      env: {
        ...process.env,
        ...this.options.env,
        ...(this.options.configPath === undefined
          ? {}
          : { OPENCODE_CONFIG: this.options.configPath }),
      },
      stdio: ["ignore", "pipe", "pipe"],
      // Its own process group, so stopping a child stops what the child
      // started. A coding agent shells out, and killing only the agent leaves
      // its build or its `sleep` running with nothing left to stop it — with a
      // fan-out of fifteen, every cancelled fleet would leave a pile behind.
      detached: true,
    });

    // The transcript is written as the events arrive, not at the end, so a
    // child that is killed still leaves everything it had done behind.
    const transcript = createWriteStream(input.transcriptPath, { flags: "a" });

    const queue: DelegateEvent[] = [];
    let notify: (() => void) | undefined;
    const wake = () => {
      notify?.();
      notify = undefined;
    };

    let stderr = "";
    /** The tail of stdout, kept for a child that failed without an event. */
    let stdout = "";
    let reported: string | undefined;
    let exited = false;
    let failure: string | undefined;

    const onAbort = () => {
      killTree(child, "SIGTERM");
      // A harness that ignores the term, or a tool that will not stop, still
      // has to go: the reader asked for the child to end, not to be asked.
      const grace = setTimeout(() => killTree(child, "SIGKILL"), KILL_GRACE_MS);
      grace.unref();
      child.once("close", () => clearTimeout(grace));
    };
    signal.addEventListener("abort", onAbort, { once: true });

    let pending = "";
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk: string) => {
      transcript.write(chunk);
      stdout = `${stdout}${chunk}`.slice(-4000);
      pending += chunk;
      let newline = pending.indexOf("\n");
      while (newline >= 0) {
        const line = pending.slice(0, newline);
        pending = pending.slice(newline + 1);
        const event = parseOpencodeEvent(line);
        if (event !== undefined) {
          if (event.type === "error") reported = event.message;
          queue.push(event);
        }
        newline = pending.indexOf("\n");
      }
      wake();
    });

    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk: string) => {
      // Kept but not streamed: the harness prints progress noise here, and only
      // the tail matters, and only when the child failed.
      stderr = `${stderr}${chunk}`.slice(-4000);
    });

    child.on("error", (cause: Error) => {
      failure =
        (cause as NodeJS.ErrnoException).code === "ENOENT"
          ? `The \`${command}\` harness is not on the path.`
          : cause.message;
      exited = true;
      wake();
    });

    child.on("close", (code) => {
      const trailing = parseOpencodeEvent(pending);
      if (trailing !== undefined) {
        if (trailing.type === "error") reported = trailing.message;
        queue.push(trailing);
      }
      if (failure === undefined && code !== 0 && !signal.aborted) {
        // What the harness said beats what the shell said. A child that
        // reported `provider refused the key` and then exited 1 has already
        // explained itself, and replacing that with the exit code is how a
        // fleet ends up reporting three identical `code 1` lines that name
        // nothing a reader could fix.
        failure = reported ?? describeExit(code, stderr, stdout);
      }
      exited = true;
      wake();
    });

    try {
      while (true) {
        while (queue.length > 0) {
          const event = queue.shift();
          if (event !== undefined) yield event;
        }
        if (exited) break;
        await new Promise<void>((resolve) => {
          notify = resolve;
        });
      }
    } finally {
      signal.removeEventListener("abort", onAbort);
      transcript.end();
    }

    if (failure !== undefined) throw new Error(failure);
  }
}

/**
 * Why a child ended, in one sentence.
 *
 * Both streams are read, stderr first: a harness that dies before it starts
 * writes there, and one that dies mid-run may have written only structured
 * output that this side could not name. An exit code on its own is the last
 * resort, not the first answer.
 */
function describeExit(code: number | null, stderr: string, stdout = ""): string {
  const exit = code === null ? "was killed" : `exited with code ${code}`;
  const tail = lastLines(stderr) ?? lastLines(stdout);
  return tail === undefined ? `The child ${exit}.` : `The child ${exit}: ${tail}`;
}

function lastLines(text: string): string | undefined {
  const tail = text
    .split("\n")
    .map((line) => stripAnsi(line).trim())
    .filter((line) => line.length > 0)
    .slice(-2)
    .join(" ");
  return tail.length === 0 ? undefined : tail.slice(-400);
}

function stripAnsi(text: string): string {
  // eslint-disable-next-line no-control-regex
  return text.replace(/\x1b\[[0-9;]*[A-Za-z]/g, "");
}

export interface DelegateFleetOptions {
  /**
   * How many children may run at once.
   *
   * A cap is not a nicety. Each child is a provider client and a process, so
   * an uncapped fan-out is rate-limit errors and a machine that stops
   * responding, and both of those look like the fleet not working.
   */
  readonly maxConcurrent: number;
  /** How many may wait for a slot before further requests are refused. */
  readonly maxQueued?: number | undefined;
  /** Where child transcripts go. Defaults to a private directory under tmp. */
  readonly transcriptDirectory?: string | undefined;
  /** The console's own directory, used when a request names none. */
  readonly cwd?: string | undefined;
}

/**
 * The scheduler: the only thing that starts children.
 *
 * `submit` resolves when that child reaches a terminal state, so a caller can
 * await one child, await `Promise.all` of many, or ignore the promise for a
 * background child and read the registry instead.
 */
export class DelegateFleet {
  private readonly transcriptDirectory: string;
  private readonly waiting: Array<() => void> = [];
  private active = 0;
  private queued = 0;
  private sequence = 0;

  constructor(
    private readonly registry: CoderTaskRegistry,
    private readonly harness: DelegateHarness,
    private readonly options: DelegateFleetOptions,
  ) {
    this.transcriptDirectory =
      options.transcriptDirectory ?? join(tmpdir(), "openagents-coder-delegations");
    mkdirSync(this.transcriptDirectory, { recursive: true, mode: 0o700 });
  }

  get activeCount(): number {
    return this.active;
  }

  get queuedCount(): number {
    return this.queued;
  }

  /** Launch one child. Waits for a slot when the fleet is full. */
  async submit(request: DelegationRequest): Promise<DelegationOutcome> {
    if (request.prompt.trim().length === 0) {
      return { status: "refused", code: "empty_prompt", reason: "A child needs a prompt." };
    }

    const maxQueued = this.options.maxQueued ?? this.options.maxConcurrent * 8;
    if (this.active >= this.options.maxConcurrent && this.queued >= maxQueued) {
      return {
        status: "refused",
        code: "fleet_full",
        reason:
          `The fleet is full: ${String(this.active)} running and ` +
          `${String(this.queued)} queued, with a cap of ${String(this.options.maxConcurrent)}. ` +
          "Wait for a child to finish or raise the cap.",
      };
    }

    const cwd = request.cwd ?? this.options.cwd ?? process.cwd();
    // Registered before it can queue, so a child waiting for a slot is visible
    // as `pending` rather than as nothing at all.
    const task = this.registry.register({
      id: this.mintId(),
      description: request.description.trim().length > 0 ? request.description : "delegated task",
      prompt: request.prompt,
      agent: this.harness.agent,
      model: this.harness.model,
      cwd,
      background: request.background ?? true,
    });

    if (this.active >= this.options.maxConcurrent) {
      this.queued += 1;
      await new Promise<void>((resolve) => this.waiting.push(resolve));
      this.queued -= 1;
      // A child stopped while it waited must not start now.
      if (this.registry.get(task.id)?.status === "stopped") {
        return { status: "stopped", taskId: task.id };
      }
    }

    this.active += 1;
    try {
      return await this.execute(task.id, request, cwd);
    } finally {
      this.active -= 1;
      this.waiting.shift()?.();
    }
  }

  /** Launch several children at once, respecting the cap. */
  submitAll(requests: ReadonlyArray<DelegationRequest>): Promise<ReadonlyArray<DelegationOutcome>> {
    return Promise.all(requests.map((request) => this.submit(request)));
  }

  private async execute(
    id: CoderTaskId,
    request: DelegationRequest,
    cwd: string,
  ): Promise<DelegationOutcome> {
    const transcriptPath = join(this.transcriptDirectory, `${id}.jsonl`);
    const controller = new AbortController();
    this.registry.start(id, controller);
    this.registry.attachTranscript(id, transcriptPath);

    /** Tool calls already counted. A harness reports one call several times. */
    const counted = new Set<string>();
    let text = "";
    let reported: string | undefined;

    try {
      for await (const event of this.harness.run(
        { prompt: request.prompt, cwd, transcriptPath },
        controller.signal,
      )) {
        if (controller.signal.aborted) break;
        if (event.type === "tool") {
          if (counted.has(event.callId)) continue;
          counted.add(event.callId);
          const activity: CoderToolActivity = { toolName: event.name, target: event.target };
          this.registry.recordToolUse(id, activity);
        } else if (event.type === "tokens") {
          this.registry.recordTokens(id, { input: event.input, output: event.output });
        } else if (event.type === "text") {
          // Only the final assistant text is the child's answer, and a harness
          // emits one text part per step, so the last one wins.
          text = event.value;
        } else if (event.type === "error") {
          reported = event.message;
        }
      }

      if (controller.signal.aborted) {
        return { status: "stopped", taskId: id };
      }
      if (reported !== undefined) {
        this.registry.fail(id, reported);
        return { status: "failed", taskId: id, error: reported };
      }

      this.registry.complete(id, text);
      return { status: "completed", taskId: id, result: text };
    } catch (cause) {
      if (controller.signal.aborted) {
        return { status: "stopped", taskId: id };
      }
      const message = cause instanceof Error ? cause.message : String(cause);
      this.registry.fail(id, message);
      return { status: "failed", taskId: id, error: message };
    }
  }

  private mintId(): CoderTaskId {
    // Time first so ids sort in launch order, then a counter so two children
    // launched in the same millisecond cannot collide.
    this.sequence += 1;
    return `d${Date.now().toString(36)}${this.sequence.toString(36).padStart(2, "0")}`;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringField(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key];
  return typeof value === "string" ? value : undefined;
}

function numberField(record: Record<string, unknown>, key: string): number | undefined {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}
