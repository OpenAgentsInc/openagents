import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/**
 * AR-2 (issue #9058): a self-contained mutation runner used to DEMONSTRATE
 * false greens by reproduction. It shares the kill/survive semantics and the
 * `replace_exact` operator of the assurance-spec `openagents.mutation.v1`
 * adapter, but runs standalone so an audit can probe any package's test
 * command without compiling a full AssuranceManifest. Manifest-bound,
 * receipt-emitting mutation remains the assurance-spec path; this runner is
 * the lightweight audit sibling.
 *
 * A mutant is "killed" when the mutated source makes the target test command
 * FAIL (the oracle caught the behaviour change) and "survived" when the
 * command still PASSES (a weak oracle — the false-green signal). The original
 * bytes are always restored in a `finally`.
 */

export const AR2_MUTATION_ADAPTER_REF = "openagents.mutation.v1" as const;

export type MutationSpec = {
  /** Repo-relative source file to mutate. */
  readonly subjectPath: string;
  /** Exact substring to replace; must occur exactly once. */
  readonly target: string;
  /** Replacement (must differ from target). */
  readonly replacement: string;
  /** Test command argv run from the repo root (e.g. ["pnpm","exec","vp","test","--run","<file>"]). */
  readonly testCommand: ReadonlyArray<string>;
};

export type MutationOutcome = {
  readonly subjectPath: string;
  readonly target: string;
  readonly replacement: string;
  /**
   * killed = the oracle caught it (the test command FAILED with a non-zero exit
   * code); survived = a weak oracle (the test still passed); inconclusive = the
   * test command was TERMINATED by a signal (a timeout, an out-of-memory kill,
   * or a crash) and so returned no verdict — this is NOT a demonstrated kill and
   * must never inflate a kill rate; error = the run could not be set up (e.g. the
   * baseline oracle did not pass).
   */
  readonly result: "killed" | "survived" | "inconclusive" | "error";
  readonly detail: string;
};

export class MutationRunnerError extends Error {
  readonly code: string;
  constructor(code: string, message: string) {
    super(message);
    this.name = "MutationRunnerError";
    this.code = code;
  }
}

/**
 * `passed` = exit 0. `failed` = a numeric non-zero exit code (the oracle
 * returned a real failing verdict). `terminated` = the process was killed by a
 * signal or otherwise returned no exit code (a timeout SIGTERM, an OOM SIGKILL,
 * a crash SIGSEGV, or a spawn error). A terminated run is NOT a failing verdict:
 * the oracle never got to decide, so it can never be credited with a kill.
 */
type RunResult = { status: "passed" | "failed" | "terminated"; detail: string };

const runTest = (root: string, command: ReadonlyArray<string>): RunResult => {
  try {
    execFileSync(command[0]!, command.slice(1), {
      cwd: root,
      stdio: "pipe",
      timeout: 10 * 60 * 1000,
    });
    return { status: "passed", detail: "test command exited 0" };
  } catch (error) {
    const err = error as {
      status?: number | null;
      signal?: string | null;
      killed?: boolean;
      code?: string;
    };
    // A signal (including the timeout kill) or a missing exit code means the
    // command was terminated rather than judged. Only a numeric non-zero status
    // is a real failing verdict.
    if ((err.signal !== null && err.signal !== undefined) || err.killed === true) {
      return {
        status: "terminated",
        detail: `test command terminated without a verdict (${
          err.signal ? `signal ${err.signal}` : "killed"
        }${err.killed ? ", timed out or killed" : ""})`,
      };
    }
    if (typeof err.status === "number") {
      return { status: "failed", detail: `test command failed (status ${err.status})` };
    }
    return {
      status: "terminated",
      detail: `test command produced no exit code${err.code ? ` (${err.code})` : ""}`,
    };
  }
};

/**
 * Apply one mutation, run the oracle, and restore. Requires that the baseline
 * oracle passes first (an oracle that already fails cannot demonstrate a kill).
 */
export const runMutation = (root: string, spec: MutationSpec): MutationOutcome => {
  if (spec.target === spec.replacement) {
    throw new MutationRunnerError("mutation_is_noop", "target and replacement are identical");
  }
  const absolute = join(root, spec.subjectPath);
  const original = readFileSync(absolute, "utf8");
  const occurrences = original.split(spec.target).length - 1;
  if (occurrences === 0) {
    throw new MutationRunnerError(
      "mutation_target_not_found",
      `target not found in ${spec.subjectPath}`,
    );
  }
  if (occurrences > 1) {
    throw new MutationRunnerError(
      "mutation_target_not_exact",
      `target occurs ${occurrences} times in ${spec.subjectPath}; must be exactly once`,
    );
  }

  const baseline = runTest(root, spec.testCommand);
  if (baseline.status !== "passed") {
    return {
      subjectPath: spec.subjectPath,
      target: spec.target,
      replacement: spec.replacement,
      result: "error",
      detail: `baseline oracle did not pass, cannot demonstrate a kill (${baseline.detail})`,
    };
  }

  try {
    writeFileSync(absolute, original.replace(spec.target, spec.replacement));
    const mutated = runTest(root, spec.testCommand);
    switch (mutated.status) {
      case "passed":
        return {
          subjectPath: spec.subjectPath,
          target: spec.target,
          replacement: spec.replacement,
          result: "survived",
          detail: `WEAK ORACLE: mutant survived — ${mutated.detail}`,
        };
      case "failed":
        return {
          subjectPath: spec.subjectPath,
          target: spec.target,
          replacement: spec.replacement,
          result: "killed",
          detail: `mutant killed — ${mutated.detail}`,
        };
      case "terminated":
        // The oracle never returned a verdict (timeout/crash). Do NOT credit a
        // kill — that would launder an infrastructure failure into a sound-oracle
        // signal and inflate the kill rate.
        return {
          subjectPath: spec.subjectPath,
          target: spec.target,
          replacement: spec.replacement,
          result: "inconclusive",
          detail: `INCONCLUSIVE: mutant test returned no verdict — ${mutated.detail}`,
        };
    }
  } finally {
    writeFileSync(absolute, original);
  }
};
