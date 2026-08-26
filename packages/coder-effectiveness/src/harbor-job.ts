/**
 * Read one completed Harbor job directory into typed, graded trials.
 *
 * The input is exactly what `bench/run-suite.sh` already leaves on disk after
 * `harbor run --dataset terminal-bench@2.0 --agent-import-path
 * adapters.openagents_coder:OpenAgentsCoder`:
 *
 *     <job-dir>/result.json                  the job envelope
 *     <job-dir>/config.json                  the pinned recipe
 *     <job-dir>/<task>__<uuid>/result.json   the trial and its verifier result
 *     <job-dir>/<task>__<uuid>/agent/trajectory.json   the coder's ATIF export
 *     <job-dir>/<task>__<uuid>/agent/coder.txt         the captured session
 *
 * Nothing new is produced by the coder for this suite to work. The field names
 * read here are the same ones `bench/post_gym_run.py` reads, so the two agree
 * about what a pass is.
 *
 * THE GRADING RULE. A trial is `accepted` only when a verifier RAN and
 * returned a positive reward. A trial whose verifier never ran is `ungraded` —
 * not a failure, and not a pass. The Terminal-Bench images are amd64 and their
 * verifier segfaults under qemu on Apple Silicon, so a crashed grader is a
 * routine local outcome rather than a rare one, and folding it into either
 * bucket would move the headline number for a reason that has nothing to do
 * with the coder. `ungraded` trials are counted, reported, and kept out of the
 * success-rate denominator, and a threshold can cap how many of them a run may
 * contain before the run stops being worth reading.
 */

import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

/** The coder's `--plain` thread announcement, the same contract bench parses. */
const THREAD_LINE = /\[oa:thread ([0-9a-fA-F-]{36})\]/u;

/**
 * The coder's `--plain` staged-text announcement
 * (OpenAgentsInc/openagents#122), emitted beside the thread line.
 *
 * Read from the trial rather than from the repository, because the repository
 * at scoring time is not the repository the run happened on. A trial from a
 * CLI that predates the announcement carries no pin, and the row then records
 * none rather than a borrowed one.
 */
const SURFACES_LINE = /\[oa:surfaces ([^\]]+)\]/u;

/** What a verifier decided about one trial. */
export type TrialOutcome = "accepted" | "rejected" | "ungraded";

export interface TrialRecord {
  /** The task half of Harbor's `<task>__<shortuuid>` directory name. */
  readonly task: string;
  readonly outcome: TrialOutcome;
  readonly modelId: string | null;
  readonly agentVersion: string | null;
  readonly promptTokens: number | null;
  readonly completionTokens: number | null;
  /** Cached-read input tokens, summed from the ATIF steps. */
  readonly cachedInputTokens: number;
  /** Tool calls the trajectory records, or `null` when it records no steps. */
  readonly toolCalls: number | null;
  readonly wallClockSeconds: number | null;
  /** The forge thread the trial ran in, when the coder announced one. */
  readonly threadId: string | null;
  /**
   * The staged text surfaces this trial composed its prompt and tool
   * declarations from, by content digest. `null` when the trial announced
   * none.
   */
  readonly surfaceDigests: Readonly<Record<string, string>> | null;
  /** The typed error Harbor classified, when the trial raised one. */
  readonly exception: string | null;
}

export interface GradedRun {
  readonly jobId: string | null;
  readonly suite: string;
  readonly lane: string;
  /**
   * A digest over what was actually run: the suite, the lane, the sorted task
   * list, the CLI version, the model, and the rate catalog version. Two runs
   * that share a digest are comparable rows; two that do not are not, and the
   * report says so rather than letting a reader assume.
   */
  readonly runDigest: string;
  /**
   * The staged text every trial agreed on, or `null` when the trials announced
   * none or disagreed.
   *
   * Disagreement is recorded as absence rather than as one of the two answers:
   * a job whose trials ran different prompts measured no single prompt, and a
   * row that named either half would be naming text that produced some of the
   * outcomes it reports.
   */
  readonly surfaceDigests: Readonly<Record<string, string>> | null;
  readonly trials: ReadonlyArray<TrialRecord>;
}

export interface ReadHarborJobOptions {
  readonly suite: string;
  readonly lane: string;
  /** Mixed into the run digest so a re-scored run is not mistaken for a re-run. */
  readonly rateCatalogVersion: string;
}

/**
 * Read a Harbor job directory. Throws when the directory is not a Harbor job,
 * because scoring a directory that holds no result is not a zero-score run.
 */
export const readHarborJob = (jobDir: string, options: ReadHarborJobOptions): GradedRun => {
  const jobResult = readJson(join(jobDir, "result.json"));
  if (jobResult === undefined) {
    throw new Error(
      `not a Harbor job directory (no result.json): ${jobDir}. Point --job-dir at the directory harbor run created under its --jobs-dir.`,
    );
  }

  const trials: Array<TrialRecord> = [];
  for (const entry of readdirSync(jobDir).toSorted()) {
    const trialDir = join(jobDir, entry);
    if (!statSync(trialDir).isDirectory()) continue;
    const trialResult = readJson(join(trialDir, "result.json"));
    if (trialResult === undefined) continue;
    trials.push(readTrial(entry, trialDir, trialResult));
  }

  const surfaceDigests = agreedSurfaceDigests(trials);
  return {
    jobId: readString(readField(jobResult, "id")),
    suite: options.suite,
    lane: options.lane,
    runDigest: runDigestOf(trials, options, surfaceDigests),
    surfaceDigests,
    trials,
  };
};

const readTrial = (dirName: string, trialDir: string, trialResult: unknown): TrialRecord => {
  const trajectory = readJson(join(trialDir, "agent", "trajectory.json"));
  const usage = readTrajectoryUsage(trajectory);
  const agent = readField(trajectory, "agent");

  return {
    task: dirName.includes("__") ? dirName.slice(0, dirName.lastIndexOf("__")) : dirName,
    outcome: outcomeOf(trialResult),
    modelId:
      readString(readField(agent, "model_name")) ??
      modelFromTrialConfig(readJson(join(trialDir, "config.json"))),
    agentVersion: readString(readField(agent, "version")),
    promptTokens: usage.promptTokens,
    completionTokens: usage.completionTokens,
    cachedInputTokens: usage.cachedInputTokens,
    toolCalls: usage.toolCalls,
    wallClockSeconds: wallClockOf(trialResult),
    threadId: threadIdOf(trialDir),
    surfaceDigests: surfaceDigestsOf(trialDir),
    exception: readString(readField(readField(trialResult, "exception_info"), "exception_type")),
  };
};

/**
 * The model, recovered from Harbor's own trial config when the trajectory is
 * missing.
 *
 * The coder writes its ATIF export at the end of a session, so a trial that hit
 * the agent timeout or crashed leaves no trajectory and no model id — exactly
 * the trials a regression run is full of. Losing the model pin there is the
 * wrong way round: a run whose lane failed is the run you most want to know the
 * lane of, and a whole run of timeouts would otherwise report `model unknown`
 * and price as `unknown_model` for a reason that has nothing to do with pricing.
 *
 * Harbor records the model on every trial before the agent starts, spelled the
 * way its `--model` flag takes it: `<provider>/<name>`. The id has to come back
 * spelled the way the CODER spells it, not the way Harbor does, because a run
 * mixing completed and killed trials would otherwise report two models where
 * there is one — and the report would list them, the digest would pin them, and
 * a lane comparison would treat the pair as a confounder. The coder's ATIF
 * export records the bare name (`qwen3.8:27b-mtp-q8_0`), so that is what a
 * recovered id is.
 *
 * Note that this deliberately does NOT reproduce the adapter's `ollama:` local
 * prefix. That prefix is how the adapter tells the CLI which lane to use; it
 * never reaches the trajectory, so adding it here would invent a spelling
 * nothing else in the run uses. Whether the lane bills metered tokens is a fact
 * about the lane, and the lane is on the run — see `priceUsage`.
 */
const modelFromTrialConfig = (trialConfig: unknown): string | null => {
  // The trial's own `config.json` holds the agent block at the top level; the
  // same block appears one level down inside a trial `result.json`. Both are
  // read so a caller does not have to know which file it handed over.
  const spelled =
    readString(readField(readField(trialConfig, "agent"), "model_name")) ??
    readString(readField(readField(readField(trialConfig, "config"), "agent"), "model_name"));
  if (spelled === null) return null;
  const separator = spelled.indexOf("/");
  if (separator === -1) return spelled;
  const name = spelled.slice(separator + 1);
  return name === "" ? spelled.slice(0, separator) : name;
};

/**
 * A verifier that never ran leaves no `verifier_result`. That is `ungraded`.
 * Where one ran, any positive reward is an accepted outcome; Harbor writes the
 * reward either as `rewards.reward` or as the sole entry of a `rewards` map.
 */
const outcomeOf = (trialResult: unknown): TrialOutcome => {
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
  const first = values.length === 1 ? readNumber(values[0]) : null;
  return first;
};

const wallClockOf = (trialResult: unknown): number | null => {
  const execution = readField(trialResult, "agent_execution");
  const started = readString(readField(execution, "started_at"));
  const finished = readString(readField(execution, "finished_at"));
  if (started === null || finished === null) return null;
  const span = Date.parse(finished) - Date.parse(started);
  return Number.isFinite(span) ? span / 1000 : null;
};

interface TrajectoryUsage {
  readonly promptTokens: number | null;
  readonly completionTokens: number | null;
  readonly cachedInputTokens: number;
  readonly toolCalls: number | null;
}

/**
 * Pull usage out of the coder's ATIF export.
 *
 * Totals come from `final_metrics`, which the coder writes only when at least
 * one turn reported usage — an absent total is unknown, so it stays `null`.
 * Cached reads are NOT in `final_metrics`: the exporter carries them per step
 * as `metrics.extra.cache_read_input_tokens` and never totals them, so this
 * sums the steps. A trajectory with no cached figure anywhere reports 0 cached
 * tokens, which is the honest reading — the coder omits the field when the
 * turn reported no cache reads, not when it failed to measure them.
 */
const readTrajectoryUsage = (trajectory: unknown): TrajectoryUsage => {
  const finalMetrics = readField(trajectory, "final_metrics");
  const steps = readArray(readField(trajectory, "steps"));

  let cachedInputTokens = 0;
  let toolCalls = 0;
  for (const step of steps) {
    const extra = readField(readField(step, "metrics"), "extra");
    cachedInputTokens += readNumber(readField(extra, "cache_read_input_tokens")) ?? 0;
    toolCalls += readArray(readField(step, "tool_calls")).length;
  }

  return {
    promptTokens: readNumber(readField(finalMetrics, "total_prompt_tokens")),
    completionTokens: readNumber(readField(finalMetrics, "total_completion_tokens")),
    cachedInputTokens,
    toolCalls: steps.length === 0 ? null : toolCalls,
  };
};

const threadIdOf = (trialDir: string): string | null => {
  const path = join(trialDir, "agent", "coder.txt");
  if (!existsSync(path)) return null;
  const match = THREAD_LINE.exec(readFileSync(path, "utf8"));
  return match?.[1] ?? null;
};

/** The staged text one trial announced, or `null` when it announced none. */
const surfaceDigestsOf = (trialDir: string): Readonly<Record<string, string>> | null => {
  const path = join(trialDir, "agent", "coder.txt");
  if (!existsSync(path)) return null;
  const match = SURFACES_LINE.exec(readFileSync(path, "utf8"));
  if (match?.[1] === undefined) return null;
  const digests: Record<string, string> = {};
  for (const pair of match[1].split(",")) {
    const at = pair.indexOf("=");
    if (at <= 0) continue;
    digests[pair.slice(0, at).trim()] = pair.slice(at + 1).trim();
  }
  return Object.keys(digests).length === 0 ? null : digests;
};

/**
 * The staged text the whole job ran on, when every trial agrees.
 *
 * One disagreeing trial makes the job's pin `null`. Two prompts in one job is
 * a job that measured neither of them, and the honest column for that is
 * empty.
 */
const agreedSurfaceDigests = (
  trials: ReadonlyArray<TrialRecord>,
): Readonly<Record<string, string>> | null => {
  const announced = trials
    .map((trial) => trial.surfaceDigests)
    .filter((digests): digests is Readonly<Record<string, string>> => digests !== null);
  if (announced.length === 0 || announced.length !== trials.length) return null;
  const first = JSON.stringify(Object.entries(announced[0]!).toSorted());
  const agreed = announced.every(
    (digests) => JSON.stringify(Object.entries(digests).toSorted()) === first,
  );
  return agreed ? announced[0]! : null;
};

/**
 * The recipe pin, including the staged text surfaces. Deliberately
 * independent of the `harbor:` digest
 * `bench/post_gym_run.py` computes: that one hashes a Python-serialised config
 * and this one hashes an explicit list of the facts that make two runs
 * comparable, so claiming they agree would be a claim neither can keep.
 */
const runDigestOf = (
  trials: ReadonlyArray<TrialRecord>,
  options: ReadHarborJobOptions,
  surfaceDigests: Readonly<Record<string, string>> | null,
): string => {
  const source = JSON.stringify({
    suite: options.suite,
    lane: options.lane,
    rateCatalogVersion: options.rateCatalogVersion,
    tasks: trials.map((trial) => trial.task).toSorted(),
    agentVersions: distinct(trials.map((trial) => trial.agentVersion)),
    models: distinct(trials.map((trial) => trial.modelId)),
    // The staged text is part of the recipe, so two runs that differ only in
    // the prompt do not share a digest and are not read as the same run.
    surfaces: surfaceDigests === null ? null : Object.entries(surfaceDigests).toSorted(),
  });
  return `effectiveness:${createHash("sha256").update(source).digest("hex")}`;
};

const distinct = (values: ReadonlyArray<string | null>): ReadonlyArray<string> =>
  [...new Set(values.filter((value): value is string => value !== null))].toSorted();

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

const readArray = (value: unknown): ReadonlyArray<unknown> => (Array.isArray(value) ? value : []);

const readString = (value: unknown): string | null =>
  typeof value === "string" && value !== "" ? value : null;

const readNumber = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null;
