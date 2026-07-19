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
  /** killed = oracle caught it; survived = weak oracle; error = could not run. */
  readonly result: "killed" | "survived" | "error";
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

const runTest = (
  root: string,
  command: ReadonlyArray<string>,
): { passed: boolean; detail: string } => {
  try {
    execFileSync(command[0]!, command.slice(1), {
      cwd: root,
      stdio: "pipe",
      timeout: 10 * 60 * 1000,
    });
    return { passed: true, detail: "test command exited 0" };
  } catch (error) {
    const err = error as { status?: number; signal?: string };
    return {
      passed: false,
      detail: `test command failed (status ${err.status ?? "?"}${err.signal ? `, signal ${err.signal}` : ""})`,
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
  if (!baseline.passed) {
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
    return {
      subjectPath: spec.subjectPath,
      target: spec.target,
      replacement: spec.replacement,
      result: mutated.passed ? "survived" : "killed",
      detail: mutated.passed
        ? `WEAK ORACLE: mutant survived — ${mutated.detail}`
        : `mutant killed — ${mutated.detail}`,
    };
  } finally {
    writeFileSync(absolute, original);
  }
};
