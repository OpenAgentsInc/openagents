/**
 * The suite manifest: what a graded run is supposed to have run, pinned hard
 * enough that the run cannot quietly have been something else.
 *
 * Issue #34 asks each run to pin "CLI version, model catalog revision, plugin
 * set, and task digest". The first three already travel with the report — the
 * CLI version off each trial's ATIF trajectory, the model and the rate catalog
 * version off the pricing layer. The fourth needs a manifest, because a task
 * digest can only mean something against a declared list. A bare list of task
 * names is not a digest of anything: `regex-log` names a task whose content
 * moved when the dataset moved, and a suite that pins only names will report
 * two different measurements under one heading.
 *
 * So a manifest task carries its identity, not its label: for a Harbor registry
 * task, the dataset it came from and the git url, commit, and path the registry
 * resolves it to. That triple is the task's content, and the digest is over the
 * triple. Two runs whose task digests agree ran the same work.
 *
 * THE SMOKE RULE, AND WHY IT IS STRUCTURAL RATHER THAN DOCUMENTED.
 *
 * The issue says "a fast/smoke run is never a published score", and a comment
 * in a README cannot make that true. Every benchmark eventually grows a fast
 * lane — three tasks, one lane, run on every commit — and every fast lane
 * eventually gets read as the number, usually by a dashboard nobody asked and
 * usually on the day it disagrees with the real one. The way to prevent that is
 * not to warn about it; it is to make the fast run unable to reach the file the
 * trend reads.
 *
 * {@link classifyRun} is where that happens. It compares the trials that
 * actually ran against the manifest's pinned task list, and a run that did not
 * cover the suite is `smoke` no matter what it was called on the command line.
 * The runner cannot lie about it, because the evidence is the trial directories
 * on disk. A smoke classification then does two things it cannot talk its way
 * out of: the gate carries a criterion it can never pass, and the results store
 * refuses the row. See `results-store.ts` and `thresholds.ts`.
 *
 * A run scored with no manifest at all is `unclassified`, which is a legitimate
 * thing to do — printing a report about a job directory is useful — and it is
 * equally unable to reach the store. Publishing requires naming the suite you
 * claim to have run, and naming it is what exposes whether you ran it.
 */

import { createHash } from "node:crypto";
import { Schema as S } from "effect";

/**
 * `score` is a suite whose result may be published. `smoke` is one that may
 * not, declared at the manifest rather than inferred, for a suite that exists
 * to be fast — a pre-push check over three tasks, say. A smoke manifest is
 * smoke even when every one of its tasks ran.
 */
export const SUITE_MANIFEST_SCHEMA = "openagents.effectiveness_suite.v1";

/**
 * Where a task's definition comes from, which decides what pins it.
 *
 * - `harbor-registry` — a task in a dataset Harbor's registry resolves, pinned
 *   by git url, commit, and path. This is the whole public-subset story: the
 *   registry already carries `terminal-bench@2.0`, `swebench-verified@1.0`,
 *   `aider-polyglot@1.0` and 77 others behind one contract, so a public subset
 *   costs a manifest entry rather than a loader.
 * - `tracker-closed-issue` — an owned task drawn from a closed issue in this
 *   tracker, pinned by the issue number and the commit that closed it. The
 *   accepted outcome is not a guess: the forge recorded that commit as the
 *   issue's closing reference.
 */
export const SuiteTaskPinSchema = S.Union([
  S.Struct({
    kind: S.Literal("harbor-registry"),
    /** `<name>@<version>` exactly as `harbor run --dataset` takes it. */
    dataset: S.String,
    gitUrl: S.String,
    commit: S.String,
    path: S.String,
  }),
  S.Struct({
    kind: S.Literal("tracker-closed-issue"),
    repo: S.String,
    issue: S.Number,
    /** The commit the forge recorded as this issue's closing reference. */
    acceptedCommit: S.String,
  }),
]);

export type SuiteTaskPin = typeof SuiteTaskPinSchema.Type;

export const SuiteTaskSchema = S.Struct({
  /** The Harbor task name: the `<task>` half of a `<task>__<uuid>` trial dir. */
  id: S.String,
  pin: SuiteTaskPinSchema,
  /**
   * Whether a container and verifier that can grade this task exist.
   *
   * A `harbor-registry` task has them by construction: the dataset ships the
   * environment and the verifier, which is the entire reason a public subset
   * costs a manifest entry rather than a harness. A `tracker-closed-issue` task
   * has neither until somebody writes them — it is a real, pinned piece of work
   * with a recorded accepted outcome and no way yet to run it, and writing it
   * into the manifest early is how the suite records the intent.
   *
   * Note what this field does NOT claim: that the task has been run here, or
   * that it passed, or that its image builds on this machine. It claims a
   * gradeable definition exists. That is the property a suite needs, because a
   * score cannot include a task nobody can run at all — its absence from the
   * results would read as a failure of the coder rather than an absence of an
   * environment, and the suite would get quietly worse the day somebody added a
   * task and forgot to build it. So {@link parseSuiteManifest} refuses a
   * `score` manifest that holds one, and the suite that holds them stays
   * `smoke` until the environments are written.
   */
  environmentAvailable: S.Boolean,
  /** One line on why this task is in the suite. Not digested. */
  rationale: S.optional(S.String),
});

export type SuiteTask = typeof SuiteTaskSchema.Type;

export const SuiteManifestSchema = S.Struct({
  schema: S.Literal(SUITE_MANIFEST_SCHEMA),
  id: S.String,
  tier: S.Literals(["score", "smoke"]),
  description: S.String,
  tasks: S.Array(SuiteTaskSchema),
});

export type SuiteManifest = typeof SuiteManifestSchema.Type;

export type RunTier = "score" | "smoke";

const decodeManifest = S.decodeUnknownSync(SuiteManifestSchema);

/**
 * Parse a manifest, rejecting the shapes that would make a digest a lie.
 *
 * Duplicate task ids are refused because Harbor names a trial directory after
 * the task, so two entries under one id cannot be told apart in a result; an
 * empty suite is refused because a suite with nothing in it trivially covers
 * itself, and would classify as a fully covered `score` run that measured
 * nothing at all.
 */
export const parseSuiteManifest = (value: unknown): SuiteManifest => {
  const manifest = decodeManifest(value);

  if (manifest.tasks.length === 0) {
    throw new Error(
      `suite ${manifest.id} declares no tasks; an empty suite covers itself and would score a run that measured nothing`,
    );
  }

  const seen = new Set<string>();
  for (const task of manifest.tasks) {
    if (seen.has(task.id)) {
      throw new Error(
        `suite ${manifest.id} names task ${task.id} twice; Harbor writes one trial directory per task name, so two entries could never be told apart in a result`,
      );
    }
    seen.add(task.id);
  }

  if (manifest.tier === "score") {
    const unavailable = manifest.tasks
      .filter((task) => !task.environmentAvailable)
      .map((task) => task.id);
    if (unavailable.length > 0) {
      throw new Error(
        `suite ${manifest.id} is tier score but holds ${String(unavailable.length)} task(s) with no environment that can grade them (${unavailable.join(", ")}); a task nobody can run would read as a failure of the coder rather than a missing environment`,
      );
    }
  }

  return manifest;
};

/**
 * The digest of one task: its identity, never its label or its rationale.
 *
 * `id` is in because it is how a trial directory names itself, so a suite that
 * renamed a task is running against a different result shape. `rationale` and
 * `environmentAvailable` are out: prose about why a task was chosen, and
 * whether anybody has written its environment yet, are facts about the suite's
 * bookkeeping rather than about the work the coder is asked to do.
 */
export const taskDigestOf = (task: SuiteTask): string =>
  `task:${createHash("sha256")
    .update(JSON.stringify(canonicalPin(task)))
    .digest("hex")}`;

const canonicalPin = (task: SuiteTask): unknown =>
  task.pin.kind === "harbor-registry"
    ? {
        id: task.id,
        kind: task.pin.kind,
        dataset: task.pin.dataset,
        gitUrl: task.pin.gitUrl,
        commit: task.pin.commit,
        path: task.pin.path,
      }
    : {
        id: task.id,
        kind: task.pin.kind,
        repo: task.pin.repo,
        issue: task.pin.issue,
        acceptedCommit: task.pin.acceptedCommit,
      };

/**
 * The digest of a whole suite: the tier and the sorted task digests.
 *
 * The tier is inside the digest deliberately. Flipping a suite from `smoke` to
 * `score` without changing a task is a change in what its results claim, and a
 * digest that ignored it would let a stored row's pin match a manifest that now
 * says something else about it.
 */
export const suiteDigestOf = (manifest: SuiteManifest): string => {
  const source = JSON.stringify({
    schema: manifest.schema,
    id: manifest.id,
    tier: manifest.tier,
    tasks: manifest.tasks.map(taskDigestOf).toSorted(),
  });
  return `suite-manifest:${createHash("sha256").update(source).digest("hex")}`;
};

/** Why a run is not a publishable score. Empty on a clean score run. */
export type SmokeReason =
  | { readonly kind: "declared_smoke"; readonly detail: string }
  | { readonly kind: "incomplete_coverage"; readonly detail: string }
  | { readonly kind: "unexpected_tasks"; readonly detail: string };

export interface RunClassification {
  readonly suiteId: string;
  readonly suiteDigest: string;
  readonly tier: RunTier;
  /** Task ids the manifest pins. */
  readonly expected: ReadonlyArray<string>;
  /** Task ids trials were actually found for. */
  readonly ran: ReadonlyArray<string>;
  readonly missing: ReadonlyArray<string>;
  readonly unexpected: ReadonlyArray<string>;
  /** Empty exactly when {@link RunClassification.tier} is `score`. */
  readonly smokeReasons: ReadonlyArray<SmokeReason>;
}

/**
 * Classify what a run actually was, from the manifest it claims and the trials
 * on disk.
 *
 * The three ways a run stops being a score, in the order they matter:
 *
 * 1. The manifest says so. A suite built to be fast is fast forever.
 * 2. It did not run every pinned task. This is the one that cannot be argued
 *    with: `--include` three of twelve tasks and the missing nine are missing
 *    from the job directory, whatever the invocation called itself. A partial
 *    run's success rate is over a different, easier or harder, set of work, and
 *    its cost per accepted outcome is over a different denominator.
 * 3. It ran tasks the manifest does not pin. A suite plus one extra task is a
 *    different suite, and the digest would otherwise say they were the same.
 *
 * Note that (2) and (3) are separate findings rather than one "task set
 * differs". An operator who trimmed a suite and an operator who substituted a
 * task have made different mistakes, and a message that named only the symptom
 * would send both to the same wrong place.
 */
export const classifyRun = (
  manifest: SuiteManifest,
  ranTaskIds: ReadonlyArray<string>,
): RunClassification => {
  const expected = manifest.tasks.map((task) => task.id).toSorted();
  const ran = [...new Set(ranTaskIds)].toSorted();
  const missing = expected.filter((id) => !ran.includes(id));
  const unexpected = ran.filter((id) => !expected.includes(id));

  const smokeReasons: Array<SmokeReason> = [];
  if (manifest.tier === "smoke") {
    smokeReasons.push({
      kind: "declared_smoke",
      detail: `suite ${manifest.id} declares tier smoke, so its results are never a published score however completely it ran`,
    });
  }
  if (missing.length > 0) {
    smokeReasons.push({
      kind: "incomplete_coverage",
      detail: `${String(missing.length)} of ${String(expected.length)} pinned tasks produced no trial (${missing.join(", ")}); a partial run scores a different set of work than the suite it names`,
    });
  }
  if (unexpected.length > 0) {
    smokeReasons.push({
      kind: "unexpected_tasks",
      detail: `${String(unexpected.length)} trial(s) ran tasks the suite does not pin (${unexpected.join(", ")}); a suite plus an extra task is a different suite`,
    });
  }

  return {
    suiteId: manifest.id,
    suiteDigest: suiteDigestOf(manifest),
    tier: smokeReasons.length === 0 ? "score" : "smoke",
    expected,
    ran,
    missing,
    unexpected,
    smokeReasons,
  };
};
