#!/usr/bin/env node
/**
 * Regenerate the suite manifests under `bench/suites/`.
 *
 * A manifest pins each task by its content rather than its name — for a public
 * task, the git url, commit, and path Harbor's registry resolves it to; for an
 * owned task, the tracker issue and the commit the forge recorded as closing
 * it. This script is where those pins are read from their sources once, so that
 * afterwards the manifests are self-contained and every consumer (the digest,
 * the gate, the store) works offline from the checked-in file alone. Same split
 * as `src/pricing.ts`: resolve from the source of truth, then pin the snapshot,
 * and keep the path back to the source written down.
 *
 * Two sources, neither of which this repo owns:
 *
 *   --registry <path>   Harbor's `registry.json`, an 80-dataset index that
 *                       already carries `terminal-bench@2.0` and
 *                       `swebench-verified@1.0` behind one contract. Defaults
 *                       to the clone at ../projects/repos/harbor.
 *   --issues <path>     A `openagents issue list --state closed --json` body
 *                       for OpenAgentsInc/openagents. Closed issues whose
 *                       evidence carries a `closing_reference` commit are the
 *                       owned tasks; the rest are skipped by name.
 *
 * Usage:
 *   node bench/build-suites.mjs --registry <registry.json> --issues <closed.json>
 *   node bench/build-suites.mjs ... --check   # rebuild and diff, write nothing
 *
 * `--check` is what CI runs: it rebuilds every manifest and fails if the result
 * differs from what is committed, so a manifest cannot drift from the registry
 * it claims to pin without somebody noticing.
 */

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const BENCH_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = dirname(BENCH_DIR);
const SUITES_DIR = join(BENCH_DIR, "suites");

const SCHEMA = "openagents.effectiveness_suite.v1";

/**
 * The twelve Terminal-Bench 2.0 tasks selected in
 * `bench/suites/tb2-cross-section.md`, which records why each slot exists.
 * Kept as a name list here because the pins come from the registry.
 */
const TB2_CROSS_SECTION = [
  "git-leak-recovery",
  "sanitize-git-repo",
  "merge-diff-arc-agi-task",
  "build-cython-ext",
  "sqlite-with-gcov",
  "fix-code-vulnerability",
  "regex-log",
  "count-dataset-tokens",
  "password-recovery",
  "openssl-selfsigned-cert",
  "nginx-request-logging",
  "schemelike-metacircular-eval",
];

/**
 * The bounded public SWE-bench subset: eight repositories, one instance each.
 *
 * Issue #34 names SWE-bench-lite as the candidate. Harbor's registry carries
 * `swebench-verified@1.0` instead — the 500-instance human-validated subset —
 * and that is the better half of the same idea: it is the subset whose task
 * statements and tests a human confirmed are solvable and correctly graded,
 * which is what a floor wants underneath it, and it runs through the same
 * `harbor run --dataset` contract with no new harness code. Lite's
 * distinguishing property is being small, and this suite is bounding itself.
 *
 * ONE INSTANCE PER REPOSITORY, CHOSEN BY RULE RATHER THAN BY HAND. The 500 are
 * dominated by Django (231) and SymPy (75); a random eight would usually be
 * five Djangos, and a score over it would mostly measure how well the coder
 * knows one codebase. So: the eight repositories with the most instances, and
 * from each the lexicographically first instance id.
 *
 * The second half of that rule matters more than it looks. Any hand-picked
 * instance invites the question of whether it was picked because the coder does
 * well on it, and there is no way to answer that question from the outside. A
 * mechanical rule answers it in advance, survives a registry refresh, and can be
 * re-derived by anyone with the registry — which is also what lets `--check`
 * mean something.
 */
const SWEBENCH_REPOS = [
  ["astropy", "astropy: table and unit handling in a large scientific codebase"],
  ["django", "Django: the framework half of the benchmark, wide blast radius per change"],
  ["matplotlib", "matplotlib: rendering state, where the test is the only oracle"],
  ["pydata", "xarray: array semantics over pandas and numpy, dtype-sensitive"],
  ["pytest-dev", "pytest: the test runner itself, so a fix has to be reentrant"],
  ["scikit-learn", "scikit-learn: estimator API conformance, contract-shaped"],
  ["sphinx-doc", "Sphinx: documentation tooling, heavy on configuration surface"],
  ["sympy", "SymPy: symbolic evaluation, where a plausible-looking fix is usually wrong"],
];

const parseArguments = () => {
  const argv = process.argv.slice(2);
  let registry = resolve(REPO_ROOT, "../projects/repos/harbor/registry.json");
  let issues = null;
  let check = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--check") check = true;
    else if (argument === "--registry") registry = argv[(index += 1)];
    else if (argument === "--issues") issues = argv[(index += 1)];
    else throw new Error(`unknown option: ${argument}`);
  }
  return { registry, issues, check };
};

/** Index a Harbor registry into `dataset@version -> { taskName -> pin }`. */
const readRegistry = (path) => {
  const datasets = new Map();
  for (const entry of JSON.parse(readFileSync(path, "utf8"))) {
    const tasks = new Map();
    for (const task of entry.tasks ?? []) {
      tasks.set(task.name, {
        gitUrl: task.git_url,
        commit: task.git_commit_id,
        path: task.path,
      });
    }
    datasets.set(`${entry.name}@${entry.version}`, tasks);
  }
  return datasets;
};

/**
 * Resolve a task name in a dataset to its pin.
 *
 * A missing name throws rather than being skipped. A suite that quietly dropped
 * a task it could not resolve would still build, still digest, and still
 * classify a run over the remaining tasks as full coverage — which is the
 * failure the digest exists to prevent, arriving through the front door.
 */
const pinFor = (datasets, dataset, name) => {
  const tasks = datasets.get(dataset);
  if (tasks === undefined) {
    throw new Error(`registry has no dataset ${dataset}`);
  }
  const pin = tasks.get(name);
  if (pin === undefined) {
    throw new Error(`registry dataset ${dataset} has no task ${name}`);
  }
  if (pin.commit === "HEAD") {
    throw new Error(
      `registry dataset ${dataset} pins ${name} at HEAD, which names a moving target rather than a commit; this suite cannot pin it`,
    );
  }
  return { kind: "harbor-registry", dataset, ...pin };
};

/**
 * The lexicographically first instance id a repository contributes to a
 * dataset. SWE-bench instance ids are `<org>__<repo>-<pr-number>`, so the
 * repository is the part before the double underscore.
 */
const firstInstanceOf = (datasets, dataset, repo) => {
  const tasks = datasets.get(dataset);
  if (tasks === undefined) throw new Error(`registry has no dataset ${dataset}`);
  const names = [...tasks.keys()].filter((name) => name.split("__")[0] === repo).toSorted();
  if (names.length === 0) {
    throw new Error(`registry dataset ${dataset} holds no instance from ${repo}`);
  }
  return names[0];
};

const registryTask = (datasets, dataset, name, rationale, environmentProven) => ({
  id: name,
  pin: pinFor(datasets, dataset, name),
  environmentProven,
  ...(rationale === undefined ? {} : { rationale }),
});

/**
 * Owned tasks: closed issues in this tracker that carry a closing commit.
 *
 * The forge records a closing reference as evidence on the issue, so the
 * accepted outcome is a fact the tracker already holds rather than a judgement
 * this script makes. An issue with no such evidence is skipped and named: it may
 * be perfectly well closed, but without a commit there is nothing to grade
 * against.
 *
 * Every one of these is `environmentProven: false` today. The pin is real — the
 * issue, its instruction, and the commit that satisfied it — and no container
 * has been built that can grade it, so `parseSuiteManifest` will refuse to let
 * them into a score-tier suite until one has. See `bench/tasks/owned/README.md`.
 */
const ownedTasks = (issuesPath) => {
  const body = JSON.parse(readFileSync(issuesPath, "utf8"));
  const issues = Array.isArray(body) ? body : (body.issues ?? []);
  const tasks = [];
  const skipped = [];
  for (const issue of issues) {
    const commits = (issue.openagents?.evidence ?? [])
      .filter((entry) => entry.source === "closing_reference" && typeof entry.commit === "string")
      .map((entry) => entry.commit);
    if (commits.length === 0) {
      skipped.push(issue.number);
      continue;
    }
    // Several closing references means the work landed over more than one push.
    // The last one is the state the issue was closed in, so that is the pin.
    const acceptedCommit = commits.at(-1);
    tasks.push({
      id: `owned-issue-${issue.number}`,
      pin: {
        kind: "tracker-closed-issue",
        repo: "OpenAgentsInc/openagents",
        issue: issue.number,
        acceptedCommit,
      },
      environmentProven: false,
      rationale: issue.title,
    });
  }
  tasks.sort((left, right) => left.pin.issue - right.pin.issue);
  return { tasks, skipped };
};

const manifest = (id, tier, description, tasks) => ({
  schema: SCHEMA,
  id,
  tier,
  description,
  tasks,
});

const write = (name, value, check) => {
  const path = join(SUITES_DIR, name);
  const text = `${JSON.stringify(value, null, 2)}\n`;
  if (check) {
    const existing = readFileSync(path, "utf8");
    if (existing !== text) {
      throw new Error(
        `${name} is out of date with its sources. Run bench/build-suites.mjs without --check and commit the result.`,
      );
    }
    process.stdout.write(`ok    ${name}\n`);
    return;
  }
  writeFileSync(path, text, "utf8");
  process.stdout.write(`wrote ${name} (${String(value.tasks.length)} tasks)\n`);
};

const main = () => {
  const { registry, issues, check } = parseArguments();
  const datasets = readRegistry(registry);

  const tb2 = TB2_CROSS_SECTION.map((name) =>
    registryTask(datasets, "terminal-bench@2.0", name, undefined, true),
  );
  const swe = SWEBENCH_REPOS.map(([repo, rationale]) =>
    registryTask(
      datasets,
      "swebench-verified@1.0",
      firstInstanceOf(datasets, "swebench-verified@1.0", repo),
      rationale,
      true,
    ),
  );

  write(
    "tb2-cross-section.suite.json",
    manifest(
      "tb2-cross-section",
      "score",
      "Twelve Terminal-Bench 2.0 tasks across git forensics, builds, C extensions, coverage, security fixes, log parsing, tokenisation, certificates, web-server configuration, and an interpreter. Selection rationale in tb2-cross-section.md.",
      tb2,
    ),
    check,
  );

  write(
    "swebench-verified-subset.suite.json",
    manifest(
      "swebench-verified-subset",
      "score",
      "The bounded public subset issue #34 asks for: eight swebench-verified@1.0 instances, one per repository, so no single project's idioms dominate the score.",
      swe,
    ),
    check,
  );

  /**
   * The two quickest tasks in the cross-section, declared `score` rather than
   * `smoke` and floored accordingly.
   *
   * This is not a fast lane wearing a score badge, and the distinction is worth
   * being precise about because the whole smoke rule depends on it. `smoke` is
   * for a suite whose result should never be published — a liveness check.
   * `tb2-quick` is a real, if narrow, measurement: two tasks the coder is
   * genuinely expected to solve, always run to completion, scored against
   * floors set for two tasks rather than for twelve. It exists because a suite
   * you can run three times in an hour is the only kind you can prove a
   * regression with on one machine, and because the headline suite's floors are
   * useless if nobody ever runs anything against them.
   *
   * What it cannot do is stand in for the headline number. It shares no suite
   * key with `coder-effectiveness-v1`, so no comparison will ever place its rows
   * beside that suite's — which is the property that makes shipping a narrow
   * score suite safe rather than a slow leak.
   */
  write(
    "tb2-quick.suite.json",
    manifest(
      "tb2-quick",
      "score",
      "Two quick Terminal-Bench 2.0 tasks: a narrow but real score suite, small enough to run repeatedly on one machine, floored for its own size. Never comparable to coder-effectiveness-v1 — different suite key.",
      [
        [
          "regex-log",
          "near-zero tool surface: the suite's test of whether the agent can just answer",
        ],
        [
          "openssl-selfsigned-cert",
          "a fully specified checklist: every command is known upfront, so round count is a tool habit rather than a reasoning result",
        ],
      ].map(([name, rationale]) =>
        registryTask(datasets, "terminal-bench@2.0", name, rationale, true),
      ),
    ),
    check,
  );

  write(
    "smoke.suite.json",
    manifest(
      "smoke",
      "smoke",
      "The fast lane: two quick Terminal-Bench tasks for checking that the harness, the adapter, and the lane are alive. Declared smoke, so its result is never a published score however completely it runs.",
      TB2_CROSS_SECTION.filter(
        (name) => name === "regex-log" || name === "fix-code-vulnerability",
      ).map((name) => registryTask(datasets, "terminal-bench@2.0", name, "quick-shaped", true)),
    ),
    check,
  );

  if (issues !== null) {
    const owned = ownedTasks(issues);
    write(
      "owned-closed-issues.suite.json",
      manifest(
        "owned-closed-issues",
        "smoke",
        "The owned half of issue #34's suite: closed issues in this tracker whose forge evidence carries a closing commit, so the accepted outcome is recorded rather than assumed. Tier smoke until the environments are built — see bench/tasks/owned/README.md.",
        owned.tasks,
      ),
      check,
    );
    process.stdout.write(
      `      ${String(owned.skipped.length)} closed issue(s) carry no closing-reference commit and are not tasks: ${owned.skipped.join(", ")}\n`,
    );

    write(
      "coder-effectiveness-v1.suite.json",
      manifest(
        "coder-effectiveness-v1",
        "score",
        "Issue #34's headline suite: the twelve-task Terminal-Bench cross-section plus the eight-instance swebench-verified subset. The owned closed-issue tasks join it once their environments are proven; until then a score suite cannot hold them.",
        [...tb2, ...swe],
      ),
      check,
    );
  }
};

try {
  main();
} catch (error) {
  process.stderr.write(`build-suites: ${error.message}\n`);
  process.exitCode = 1;
}
