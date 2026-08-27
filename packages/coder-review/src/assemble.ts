/**
 * Assemble one cycle review's input from the artifacts a Harbor run left.
 *
 * The runbook's §6 prompt takes five things: the trials, the lever's diff, the
 * store rows before and after, and the current ledger. An agent assembling
 * those by hand is the step OpenAgentsInc/openagents#121 replaces, and doing it
 * mechanically buys three properties a copy-and-paste cannot have.
 *
 * BOUNDED, AND NAMED WHERE IT IS BOUND. A trajectory is as long as the task
 * was hard, and the transcript of a fifteen-round trial does not fit in a
 * review prompt. Every cut this file makes is reported in the request itself,
 * in the `read-conversation` plugin's vocabulary: `tail_only`,
 * `dropped_leading_steps`, `kept_steps`, `total_steps`. A reviewer that cannot
 * see the opening of a trial is told so, and a proposal that cites a dropped
 * step is refused by name rather than resolving against a step the reviewer
 * never read.
 *
 * REDACTED WITH THE ONE RULE LIST. Everything assembled here is on its way out
 * of the working session and into another conversation, so it goes through
 * `redactForExternalInference` from the vendored ATIF rules — the same list
 * `openagents trace redact` folds in. #97 is the reason that is stated rather
 * than assumed: a second hand-written rule list forgot `oa_pat_` and `smct_`,
 * and the command reported success over a file of live tokens. There is no
 * second list here.
 *
 * A RESOLVABLE EVIDENCE INDEX. The request knows exactly which steps, rows,
 * ledger entries, and diff paths it contains, and hands that set to the parser
 * as {@link EvidenceIndex}. That is what makes "this proposal cites nothing"
 * a mechanical finding instead of a reviewer's opinion.
 */

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";

import type { EvidenceIndex } from "./candidate.js";
import { redactForExternalInference } from "./memory/redaction.js";

export const CODER_REVIEW_REQUEST_SCHEMA = "openagents.coder_review_request.v1";

/** The coder's `--plain` thread announcement, the contract `bench` parses. */
const THREAD_LINE = /\[oa:thread ([0-9a-fA-F-]{36})\]/u;

/** Matches `### T1. Batch independent commands ...` in the ledger. */
const LEDGER_HEADING = /^###\s+([A-Z]+\d+)\.\s/gmu;

/** Matches the paths in a unified diff's `diff --git a/x b/x` lines. */
const DIFF_PATH = /^diff --git a\/(\S+) b\/(\S+)$/gmu;

/** What a verifier decided. Same three buckets `harbor-job.ts` grades into. */
export type ReviewTrialOutcome = "accepted" | "rejected" | "ungraded";

export interface ReviewToolCall {
  readonly name: string;
  readonly arguments: string;
  readonly argumentsTruncated: boolean;
  /**
   * What the tool returned, bounded.
   *
   * A trajectory step carries the call and its observation together, and a
   * review that can see the command but not its output cannot tell a wasted
   * round from a productive one — which is most of what runbook §6 asks it to
   * judge.
   */
  readonly observation: string;
  readonly observationTruncated: boolean;
}

export interface ReviewStep {
  /** The step id as the trajectory spells it, so a ref can name it back. */
  readonly stepId: string;
  readonly source: string;
  readonly text: string;
  readonly textTruncated: boolean;
  readonly toolCalls: ReadonlyArray<ReviewToolCall>;
  readonly promptTokens: number | null;
  readonly completionTokens: number | null;
  readonly cachedInputTokens: number | null;
}

/**
 * How much of a trial's trajectory reached the reviewer.
 *
 * Named in the `read-conversation` plugin's spelling on purpose: a truncated
 * read that says what it left out is a different artifact from one that does
 * not, and having two vocabularies for it would make a reader check which is
 * which.
 */
export interface ReviewTruncation {
  readonly tail_only: boolean;
  readonly dropped_leading_steps: number;
  readonly kept_steps: number;
  readonly total_steps: number;
  readonly text_truncated_steps: number;
  readonly max_step_chars: number;
}

export interface ReviewTrial {
  readonly task: string;
  /** The trial directory's own name. Never its path. */
  readonly trialDir: string;
  readonly instruction: string | null;
  /** Where the instruction was read from, or `absent` when nothing carried it. */
  readonly instructionSource:
    | "trial_result"
    | "trial_config"
    | "trajectory_first_user_step"
    | "absent";
  readonly outcome: ReviewTrialOutcome;
  readonly modelId: string | null;
  readonly agentVersion: string | null;
  readonly promptTokens: number | null;
  readonly completionTokens: number | null;
  readonly cachedInputTokens: number;
  readonly toolCalls: number | null;
  readonly wallClockSeconds: number | null;
  readonly threadId: string | null;
  readonly exception: string | null;
  readonly steps: ReadonlyArray<ReviewStep>;
  readonly truncation: ReviewTruncation;
}

export interface ReviewLever {
  /** What the diff was taken against, e.g. `HEAD` or a commit ref. */
  readonly ref: string;
  readonly diff: string;
  readonly paths: ReadonlyArray<string>;
  readonly truncation: {
    readonly tail_only: false;
    readonly kept_lines: number;
    readonly total_lines: number;
    readonly dropped_trailing_lines: number;
  };
}

export interface ReviewBenchRow {
  readonly suite: string;
  readonly recordedAt: string;
  /** The row's own fields, as the store wrote them. */
  readonly row: Record<string, unknown>;
}

export interface ReviewPractices {
  readonly path: string;
  readonly text: string;
  readonly entryIds: ReadonlyArray<string>;
}

export interface ReviewRedactionStamp {
  readonly serviceRef: string;
  readonly surface: "trace_capture";
  readonly appliedBeforeExternalInference: true;
  readonly counts: Readonly<Record<string, number>>;
  readonly total: number;
}

/** Everything the reviewer is given, and nothing the working session holds. */
export interface ReviewRequest {
  readonly schema: typeof CODER_REVIEW_REQUEST_SCHEMA;
  /** The job directory's own name. Never its path: a path is a local fact. */
  readonly jobDir: string;
  readonly jobId: string | null;
  readonly suite: string;
  readonly lane: string;
  readonly trials: ReadonlyArray<ReviewTrial>;
  readonly lever: ReviewLever;
  readonly rows: ReadonlyArray<ReviewBenchRow>;
  readonly practices: ReviewPractices;
  readonly redaction: ReviewRedactionStamp;
}

export interface AssembleOptions {
  readonly suite: string;
  readonly lane: string;
  readonly lever: { readonly ref: string; readonly diff: string };
  readonly rows: ReadonlyArray<ReviewBenchRow>;
  readonly practices: { readonly path: string; readonly text: string };
  /** Steps kept per trial. The tail is kept: a trial ends where it failed. */
  readonly maxSteps: number;
  readonly maxStepChars: number;
  readonly maxDiffLines: number;
}

export const DEFAULT_MAX_STEPS = 40;
export const DEFAULT_MAX_STEP_CHARS = 1200;
export const DEFAULT_MAX_DIFF_LINES = 800;

export class NotAHarborJob extends Error {
  constructor(jobDir: string, reason: string) {
    super(
      `not a Harbor job directory (${reason}): ${jobDir}. Point at the directory harbor run created under its --jobs-dir.`,
    );
    this.name = "NotAHarborJob";
  }
}

/**
 * Read a completed Harbor job into a bounded, redacted review request.
 *
 * Throws {@link NotAHarborJob} rather than reviewing an empty directory: a
 * review of nothing would still produce a score, and a score with no run
 * behind it is the shape of claim this loop exists to stop.
 */
export const assembleReviewRequest = (
  jobDir: string,
  options: AssembleOptions,
): { readonly request: ReviewRequest; readonly index: EvidenceIndex } => {
  const jobResult = readJson(join(jobDir, "result.json"));
  if (jobResult === undefined) throw new NotAHarborJob(jobDir, "no result.json");

  const trials: Array<ReviewTrial> = [];
  for (const entry of readdirSync(jobDir).sort()) {
    const trialDir = join(jobDir, entry);
    if (!statSync(trialDir).isDirectory()) continue;
    const trialResult = readJson(join(trialDir, "result.json"));
    if (trialResult === undefined) continue;
    trials.push(readTrial(entry, trialDir, trialResult, options));
  }

  // A trial directory has a `result.json` too, so pointing at one would
  // otherwise assemble a job of zero trials — and a review of zero trials still
  // produces a score. Refusing here is the same rule the store applies to a run
  // that skipped its pinned tasks: it is not a smaller measurement, it is none.
  if (trials.length === 0) {
    throw new NotAHarborJob(jobDir, "it holds no trial directories");
  }

  const diffLines = options.lever.diff === "" ? [] : options.lever.diff.split("\n");
  const keptDiffLines = diffLines.slice(0, options.maxDiffLines);
  const lever: ReviewLever = {
    ref: options.lever.ref,
    diff: keptDiffLines.join("\n"),
    paths: diffPathsOf(options.lever.diff),
    truncation: {
      tail_only: false,
      kept_lines: keptDiffLines.length,
      total_lines: diffLines.length,
      dropped_trailing_lines: diffLines.length - keptDiffLines.length,
    },
  };

  const practices: ReviewPractices = {
    path: options.practices.path,
    text: options.practices.text,
    entryIds: ledgerEntryIds(options.practices.text),
  };

  const draft = {
    schema: CODER_REVIEW_REQUEST_SCHEMA,
    jobDir: basename(jobDir),
    jobId: readString(readField(jobResult, "id")),
    suite: options.suite,
    lane: options.lane,
    trials,
    lever,
    rows: options.rows,
    practices,
  } as const;

  // Everything above is on its way into another conversation, so the whole
  // request goes through the one rule list before it leaves this process. The
  // stamp is added after, so the report counts what the reviewer will not see.
  const redacted = redactForExternalInference(draft, { surface: "trace_capture" });

  const request: ReviewRequest = {
    ...redacted.value,
    redaction: {
      serviceRef: redacted.policy.serviceRef,
      surface: "trace_capture",
      appliedBeforeExternalInference: true,
      counts: redacted.report.counts,
      total: redacted.report.total,
    },
  };

  return { request, index: indexOf(request) };
};

/** The set of refs a proposal may cite, built from the request itself. */
export const indexOf = (request: ReviewRequest): EvidenceIndex => {
  const trajectorySteps = new Set<string>();
  const trialOutcomes = new Set<string>();
  for (const trial of request.trials) {
    trialOutcomes.add(trial.task);
    for (const step of trial.steps) {
      trajectorySteps.add(`${trial.task}#step-${step.stepId}`);
    }
  }
  return {
    trajectorySteps,
    trialOutcomes,
    benchRows: new Set(request.rows.map((row) => `${row.suite}#${row.recordedAt}`)),
    ledgerEntries: new Set(request.practices.entryIds),
    diffPaths: new Set(request.lever.paths),
  };
};

const readTrial = (
  dirName: string,
  trialDir: string,
  trialResult: unknown,
  options: AssembleOptions,
): ReviewTrial => {
  const trajectory = readJson(join(trialDir, "agent", "trajectory.json"));
  const rawSteps = readArray(readField(trajectory, "steps"));
  const agent = readField(trajectory, "agent");
  const finalMetrics = readField(trajectory, "final_metrics");

  let cachedInputTokens = 0;
  let toolCalls = 0;
  for (const step of rawSteps) {
    const extra = readField(readField(step, "metrics"), "extra");
    cachedInputTokens += readNumber(readField(extra, "cache_read_input_tokens")) ?? 0;
    toolCalls += readArray(readField(step, "tool_calls")).length;
  }

  // The tail is what is kept. A trial that failed failed at its end, and the
  // opening rounds of a long transcript are the least informative part of it.
  const dropped = Math.max(0, rawSteps.length - options.maxSteps);
  const keptRaw = rawSteps.slice(dropped);
  let textTruncatedSteps = 0;
  const steps = keptRaw.map((step, position) => {
    const built = readStep(step, dropped + position, options.maxStepChars);
    if (built.textTruncated) textTruncatedSteps += 1;
    return built;
  });

  const instruction = readInstruction(trialDir, trialResult, rawSteps);

  return {
    // The trial spells its own task name. The directory name is the fallback
    // for a tree that did not, and it carries a random per-trial suffix.
    task:
      readString(readField(trialResult, "task_name")) ??
      (dirName.includes("__") ? dirName.slice(0, dirName.lastIndexOf("__")) : dirName),
    trialDir: dirName,
    instruction: instruction.text,
    instructionSource: instruction.source,
    outcome: outcomeOf(trialResult),
    modelId:
      readString(readField(agent, "model_name")) ??
      modelFromTrialConfig(readJson(join(trialDir, "config.json"))),
    agentVersion: readString(readField(agent, "version")),
    promptTokens: readNumber(readField(finalMetrics, "total_prompt_tokens")),
    completionTokens: readNumber(readField(finalMetrics, "total_completion_tokens")),
    cachedInputTokens,
    toolCalls: rawSteps.length === 0 ? null : toolCalls,
    wallClockSeconds: wallClockOf(trialResult),
    threadId: threadIdOf(trialDir),
    exception: readString(readField(readField(trialResult, "exception_info"), "exception_type")),
    steps,
    truncation: {
      tail_only: dropped > 0,
      dropped_leading_steps: dropped,
      kept_steps: steps.length,
      total_steps: rawSteps.length,
      text_truncated_steps: textTruncatedSteps,
      max_step_chars: options.maxStepChars,
    },
  };
};

const readStep = (step: unknown, position: number, maxChars: number): ReviewStep => {
  const rawId = readField(step, "step_id");
  const stepId =
    typeof rawId === "number" || typeof rawId === "string" ? String(rawId) : String(position + 1);
  const message = readString(readField(step, "message")) ?? "";
  const clipped = clip(message, maxChars);
  const metrics = readField(step, "metrics");

  return {
    stepId,
    source: readString(readField(step, "source")) ?? "unknown",
    text: clipped.text,
    textTruncated: clipped.truncated,
    toolCalls: readArray(readField(step, "tool_calls")).map((call) => {
      // The coder's ATIF exporter writes `function_name` and an `arguments`
      // object; the OpenAI-shaped spellings are read too, so a trajectory from
      // another adapter is not silently reported as a step of `unknown` calls.
      const args =
        stringify(readField(call, "arguments")) ?? stringify(readField(call, "input")) ?? "";
      const clippedArgs = clip(args, maxChars);
      const callId =
        readString(readField(call, "tool_call_id")) ?? readString(readField(call, "call_id"));
      const clippedObservation = clip(observationFor(step, callId), maxChars);
      return {
        name:
          readString(readField(call, "function_name")) ??
          readString(readField(call, "name")) ??
          readString(readField(readField(call, "function"), "name")) ??
          "unknown",
        arguments: clippedArgs.text,
        argumentsTruncated: clippedArgs.truncated,
        observation: clippedObservation.text,
        observationTruncated: clippedObservation.truncated,
      };
    }),
    promptTokens: readNumber(readField(metrics, "prompt_tokens")),
    completionTokens: readNumber(readField(metrics, "completion_tokens")),
    cachedInputTokens: readNumber(
      readField(readField(metrics, "extra"), "cache_read_input_tokens"),
    ),
  };
};

/**
 * The output of one call, out of the step's observation block.
 *
 * ATIF puts every result of a step in one `observation.results` array keyed by
 * `source_call_id`, so a step with two calls carries two results and matching
 * them by id is what keeps a command next to its own output. A step with one
 * result and no id still matches its single call: dropping the output because
 * the exporter omitted an id would lose the more useful half of the step.
 */
const observationFor = (step: unknown, callId: string | null): string => {
  const results = readArray(readField(readField(step, "observation"), "results"));
  if (results.length === 0) return "";
  const matched =
    callId === null
      ? undefined
      : results.find((result) => readString(readField(result, "source_call_id")) === callId);
  const chosen = matched ?? (results.length === 1 ? results[0] : undefined);
  if (chosen === undefined) return "";
  return stringify(readField(chosen, "content")) ?? "";
};

/**
 * The task instruction, from whichever artifact carried it.
 *
 * Harbor spells it differently across its own files, and a trial that timed
 * out before the coder started may carry it in only one of them. The source is
 * recorded next to the text so a reviewer reading a suspiciously short
 * instruction can tell a truncated task statement from a recovered one.
 */
const readInstruction = (
  trialDir: string,
  trialResult: unknown,
  steps: ReadonlyArray<unknown>,
): { readonly text: string | null; readonly source: ReviewTrial["instructionSource"] } => {
  const fromResult =
    readString(readField(trialResult, "instruction")) ??
    readString(readField(readField(trialResult, "task"), "instruction"));
  if (fromResult !== null) return { text: fromResult, source: "trial_result" };

  const config = readJson(join(trialDir, "config.json"));
  const fromConfig =
    readString(readField(config, "instruction")) ??
    readString(readField(readField(config, "task"), "instruction"));
  if (fromConfig !== null) return { text: fromConfig, source: "trial_config" };

  for (const step of steps) {
    if (readString(readField(step, "source")) !== "user") continue;
    const message = readString(readField(step, "message"));
    if (message !== null) return { text: message, source: "trajectory_first_user_step" };
  }
  return { text: null, source: "absent" };
};

/**
 * The grading rule, held identical to `harbor-job.ts`.
 *
 * A verifier that never ran leaves no `verifier_result`, and that is
 * `ungraded` — not a failure and not a pass. Two readers of the same run
 * disagreeing about what a pass is would make the review and the score
 * describe different runs.
 */
const outcomeOf = (trialResult: unknown): ReviewTrialOutcome => {
  const verifier = readField(trialResult, "verifier_result");
  if (verifier === undefined || verifier === null) return "ungraded";
  const rewards = readField(verifier, "rewards") ?? readField(trialResult, "rewards");
  const reward = rewardValue(rewards);
  return reward !== null && reward > 0 ? "accepted" : "rejected";
};

const rewardValue = (rewards: unknown): number | null => {
  const direct = readNumber(readField(rewards, "reward"));
  if (direct !== null) return direct;
  if (typeof rewards !== "object" || rewards === null) return null;
  const values = Object.values(rewards as Record<string, unknown>);
  return values.length === 1 ? readNumber(values[0]) : null;
};

const wallClockOf = (trialResult: unknown): number | null => {
  const execution = readField(trialResult, "agent_execution");
  const started = readString(readField(execution, "started_at"));
  const finished = readString(readField(execution, "finished_at"));
  if (started === null || finished === null) return null;
  const span = Date.parse(finished) - Date.parse(started);
  return Number.isFinite(span) ? span / 1000 : null;
};

const modelFromTrialConfig = (trialConfig: unknown): string | null => {
  const spelled =
    readString(readField(readField(trialConfig, "agent"), "model_name")) ??
    readString(readField(readField(readField(trialConfig, "config"), "agent"), "model_name"));
  if (spelled === null) return null;
  const separator = spelled.indexOf("/");
  if (separator === -1) return spelled;
  const name = spelled.slice(separator + 1);
  return name === "" ? spelled.slice(0, separator) : name;
};

const threadIdOf = (trialDir: string): string | null => {
  const path = join(trialDir, "agent", "coder.txt");
  if (!existsSync(path)) return null;
  return THREAD_LINE.exec(readFileSync(path, "utf8"))?.[1] ?? null;
};

/** Ledger entry ids, so `ledger:T1` can be resolved instead of trusted. */
export const ledgerEntryIds = (practices: string): ReadonlyArray<string> => {
  const ids: Array<string> = [];
  LEDGER_HEADING.lastIndex = 0;
  let match = LEDGER_HEADING.exec(practices);
  while (match !== null) {
    if (match[1] !== undefined) ids.push(match[1]);
    match = LEDGER_HEADING.exec(practices);
  }
  return ids;
};

/** Paths a unified diff touches, so `diff:<path>` can be resolved. */
export const diffPathsOf = (diff: string): ReadonlyArray<string> => {
  const paths = new Set<string>();
  DIFF_PATH.lastIndex = 0;
  let match = DIFF_PATH.exec(diff);
  while (match !== null) {
    if (match[1] !== undefined) paths.add(match[1]);
    if (match[2] !== undefined) paths.add(match[2]);
    match = DIFF_PATH.exec(diff);
  }
  return [...paths].sort();
};

/** Read the rows of one `bench-results` store file, newest last. */
export const readBenchRows = (
  storePath: string,
  suite: string,
  limit: number,
): ReadonlyArray<ReviewBenchRow> => {
  if (!existsSync(storePath)) return [];
  const rows: Array<ReviewBenchRow> = [];
  for (const line of readFileSync(storePath, "utf8").split("\n")) {
    if (line.trim() === "") continue;
    let parsed: unknown;
    try {
      parsed = JSON.parse(line) as unknown;
    } catch {
      continue;
    }
    const row = readRecord(parsed);
    if (row === undefined) continue;
    rows.push({
      suite: readString(row["suite"]) ?? suite,
      recordedAt: readString(row["recordedAt"]) ?? "",
      row,
    });
  }
  return rows.slice(Math.max(0, rows.length - limit));
};

const clip = (text: string, maxChars: number): { text: string; truncated: boolean } =>
  text.length <= maxChars
    ? { text, truncated: false }
    : {
        text: `${text.slice(0, maxChars)}\n[${String(text.length - maxChars)} of ${String(text.length)} characters dropped from the end of this step]`,
        truncated: true,
      };

const stringify = (value: unknown): string | null => {
  if (value === undefined || value === null) return null;
  if (typeof value === "string") return value === "" ? null : value;
  try {
    return JSON.stringify(value) ?? null;
  } catch {
    return null;
  }
};

const readJson = (path: string): unknown => {
  if (!existsSync(path)) return undefined;
  try {
    return JSON.parse(readFileSync(path, "utf8")) as unknown;
  } catch {
    return undefined;
  }
};

const readField = (value: unknown, key: string): unknown =>
  typeof value === "object" && value !== null ? (value as Record<string, unknown>)[key] : undefined;

const readRecord = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;

const readArray = (value: unknown): ReadonlyArray<unknown> => (Array.isArray(value) ? value : []);

const readString = (value: unknown): string | null =>
  typeof value === "string" && value !== "" ? value : null;

const readNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;
