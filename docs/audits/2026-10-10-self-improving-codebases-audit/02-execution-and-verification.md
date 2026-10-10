# Execution and verification

Audit baseline: `07805e6a7c3513a057d226b488cb2d40fd974a64`, October 10, 2026. This chapter evaluates the path from an issue to an independently checked change. Findings are source findings unless identified as retained operational evidence. No live agent, Cargo build, customer workload, or deployment was run for this audit.

The repository has most of the components needed to execute useful changes. It does not yet have one execution path whose evidence establishes the complete self-improvement claim. In particular, the experimental briefed agent can report a successful verification when its checker did not successfully execute. The terminal issue-run path can open a PR after failed final checks. Other owners implement stronger controls, including the established issue flow and independent environment verifier. The product work is to connect and qualify those controls, without inheriting weaker experimental semantics.

## What an accepted improvement must mean

The [product specification](../../product/self-improving-codebases.md) makes a critical distinction: learned decisions improve around a fixed exact core. For code changes, that core must cover source identity, permissions, required checks, verdict interpretation, evidence custody, and publication authority. Fixing a bug in the evaluator is ordinary reviewed engineering. Allowing the candidate under evaluation to redefine its evaluator is a different authority and cannot be an implicit consequence of write access to a repository.

A useful end-to-end record needs distinct states:

1. The system selected an authorized issue and a source revision.
2. An identified agent attempted the issue with a particular briefing and tool set.
3. The attempt produced a captured patch, including new files and relevant binary changes.
4. A particular required check plan executed on a particular resulting tree.
5. An independent evaluator determined what passed, failed, or remained unknown.
6. A human or previously granted policy accepted that exact result for a stated purpose.
7. An integrator landed the checked change, rerunning necessary checks after relevant movement of the base.
8. Deployment and real use supplied additional outcomes where the task required them.
9. Only appropriately labeled, authorized evidence entered learning, and any learned update passed its own promotion process.

A finished SDK stream proves step 2. A patch proves step 3. A successful test command can support step 4. None alone establishes the later steps. Negative outcomes are valuable training data when honestly labeled; replaying a failed attempt is a successful reproduction of a failure.

## Existing execution paths and their actual guarantees

| Path | Useful implemented behavior | Limit for this product |
| --- | --- | --- |
| `coder-new` issue-run | Reads an issue, creates a worktree, builds a briefing, runs Claude Agent SDK with a small tool set, records events and a patch, runs final compile checks, optionally opens a PR | Experimental path; final PR creation is not conditioned on green checks, its worktree has shared default identity, and trace replay/admission is separate |
| `briefed-agent` A/B worker | Restricts named tools, records reads outside briefing, offers structured `verify`, can stop through `finish` | The structured verifier can falsely pass; narrow tool names do not establish an OS execution boundary |
| Established `coder::task::issue_run` | Claims, durable flow, bounded repair rounds, repository checks, landing/rebase handling, failure reporting, issue and board updates | It is not automatically the path used by the new briefing product; distributed integration and exact protected evaluator custody remain separate |
| A/B grading | Applies a candidate on a known parent, overlays tests from the reference fix, checks actual process status and named-test counts, retains results | Stronger than agent self-report, but a mutable shared build-host harness and reference-fix selection do not establish an independently versioned release evaluator |
| Trace replay | Checks patch digest, resulting tree, changed files, and repeated verdicts; distinguishes verified/rejected/unverifiable | Verifies reproducibility, including reproducible failures; does not itself prove customer acceptance or independent authority |
| Environment verification | Fresh machines, exact image/manifest and check plan, untouched baseline, separate idempotence fork, complete evidence and cleanup | Strong reusable design, but environment-image verification is not the default code-change evaluator; fixture completion and live provider qualification remain different |

Sources: [issue-run guide](../../coder/issue-run.md), [new runner](../../../crates/coder-new/src/issue_run/mod.rs), [agent](../../../crates/coder-new/src/issue_run/agent.rs), [established flow](../../../crates/coder/src/task/issue_run.rs), [A/B runner](../../../scripts/bench/briefed-ab/ab.py), [grader](../../../scripts/bench/briefed-ab/remote/eval.sh), [trace implementation](../../../scripts/bench/traces/traces.py), and [environment verifier](../../../crates/coder-environment-verify/src/lib.rs).

## RUN-01 — The structured verifier can report success without successful execution

**Priority: P0 for any product use that treats this verifier or `finish` as acceptance. Confirmed in source.** This is an evaluator defect, not a claim that every A/B result is wrong.

In [`Verify::exec`](../../../crates/briefed-agent/src/verify.rs), the subprocess result includes both an exit-success boolean and text. Both the fast compile path and `test_and_fmt` discard the boolean. The full path determines success from parsed compiler errors, test failure counts, and panic excerpts. When none are parsed, it sets `status: pass`, including when zero tests ran. Formatting failure is reported in a separate `fmt` string and does not change this status.

The following is a source-derived decision table, not a live engine test. Assume an otherwise valid configuration with a selected package and no matching baseline errors.

| Checker outcome/input | Current reported outcome | Required meaning |
| --- | --- | --- |
| Fast call exits nonzero with empty output | `compiles` | Execution failed; compilation unknown |
| Full call exits nonzero with empty output | `pass`, zero tests, `cargo fmt failed` | Execution failed; tests and formatting unknown |
| Full call cannot start; text is `could not run the check: No such file or directory (os error 2)` | No recognized Rust diagnostic/test failure; `pass` | Checker unavailable |
| Test command exits successfully but matches no tests | `pass`, with a note that no test matched | Incomplete if this task requires asserted behavior |
| Tests pass but `cargo fmt` fails without a recognized compiler diagnostic | `pass`, with failed formatting in `fmt` | Required formatting failed, if formatting is part of the plan |
| Agent supplies a filter that runs a passing subset | Passing subset can become final `finish` result | Required final plan still has to run |

[`tools::server`](../../../crates/briefed-agent/src/tools.rs) implements `finish` by calling `verify.run(last_filter, false)`. It trusts `status == pass`, sets the finished flag, and returns “Verified. The run ends here.” The main loop then interrupts further agent work. This repeats the same evaluator and filter; it is not an independent check. It can also use the cached result described in RUN-02.

The `done_when` array compounds the issue. Each natural-language acceptance item receives the same statement that touched crates compile and their tests pass, with the same generic status. There is no mapping from individual requirements to distinct assertions. A requirement such as “a revoked grant prevents dispatch” or “the result appears on the phone” cannot be inferred from unrelated package tests.

There is a real compensating control in the A/B experiment: `ab.py` invokes `prepare.grade` after the attempt, and `remote/eval.sh` independently applies the patch, overlays the reference tests, runs commands, and requires both successful test exit and a minimum count of passed named tests. The final A/B judge also has a separate acceptance field. The false positive therefore invalidates the interactive verification/finish claim and can waste or truncate attempts; it does not by itself establish a false accepted outcome in that A/B dataset.

**Required repair and acceptance experiment:** represent transport failure, timeout, missing executable, compiler failure, test failure, no assertions, and formatting failure separately. Do not derive command success from log text. Final completion must use a required plan and its declared assertion obligations, independent of the agent's last filter. With a tiny fake checker, exercise every row above and prove that none of the failure or unknown cases can set the finished flag. Retain actual command status alongside parsed diagnostics. Keep filtered checks available for fast feedback without equating them to final acceptance.

## RUN-02 — Verification caching does not identify all checked bytes

**Priority: P1. Confirmed in source.** `Verify::changed_digest` concatenates ordinary `git diff` with `git status --porcelain --untracked-files=all`. Despite its name, this is not a digest of the complete candidate tree. An untracked file contributes its filename and status, not its contents. Ordinary `git diff` also omits staged content relative to the base. The key includes the fast/full mode and test filter, which is useful but insufficient.

A concrete failure sequence requires no sophisticated attack: the agent creates a new test or source file, verifies it, edits that same untracked file, then verifies again with the same filter. The status output and tracked diff can remain identical. The cached verdict is returned as though nothing changed. `finish` uses the same cache. A new regression, corrected failure, or changed assertion can therefore remain invisible to the final decision.

The cache also does not bind the checker executable, check plan, baseline error set, Cargo configuration, dependencies, or execution environment. In a fixed one-process experiment those may be stable by convention, but the eventual product needs an explicit validity contract.

**Acceptance experiment:** use a scratch Git repository with one tracked file and one new `.rs` file; run the same check twice, changing only the new file's bytes between calls. The second call must execute again. Repeat for staged content, deleted files, symlink targets, manifest changes, and a changed check plan. Prefer a candidate tree identity plus a versioned evaluator/environment identity over an ad hoc status string. Cache invalidation must be conservative when Git inspection fails.

## RUN-03 — Final issue-run checks and PR creation are disconnected

**Priority: P1. Confirmed in source.** In [`agent::final_checks`](../../../crates/coder-new/src/issue_run/agent.rs), the harness reruns only entries whose ID starts with `check:`. In the current briefing model these are compile checks; test checks are separately available to the agent. The function returns `(id, bool)` results. [`issue_run::run`](../../../crates/coder-new/src/issue_run/mod.rs) writes those results to the summary and then calls `setup::open_pr` whenever `--open-pr` was requested. Neither the returned booleans nor `work.error` gates that call.

[`setup::open_pr`](../../../crates/coder-new/src/issue_run/setup.rs) checks that this is not a closed-issue replay, then creates a branch, commits, pushes, and opens a PR. It has no check receipt argument. Thus a normal failed compile can coexist with a newly opened PR whose body says it completes and closes the issue. Opening a reviewable failed PR is not inherently wrong; silently treating it as an accepted run would be. The current interface does not carry a mandatory typed failed/unchecked state into that publication.

The established [`Run::drive`](../../../crates/coder/src/task/issue_run.rs) is stronger: it runs repository checks, enters repair turns on failures, and refuses to push after the configured repair limit. It also distinguishes issue failures from engine completion in `Flow::ending`. Those controls should be reused rather than re-created in the experimental runner.

**Acceptance experiment:** inject one failing final check, one agent error, and one missing required test result. The normal success route must stop before publication or create an explicitly failed draft only under an intentional review policy. A green route must bind its full required plan to the exact patch/tree and include that evidence in the PR. Compile-only tasks and documentation-only tasks can have appropriately narrow plans; they must be declared, not inferred from an empty test set.

## RUN-04 — A restricted tool list is not a protected execution environment

**Priority: P1 for unattended or customer-repository execution. Confirmed missing boundaries in this path; not a demonstrated host compromise.** The new runner exposes Read, Edit, Write, Grep, Glob, and a named `run_check`. Its permission helper normalizes path components and checks whether a path starts inside the worktree. It does not resolve symlink targets or enforce a protected path set. The briefed-agent permission helper uses the same style of boundary.

Two different issues need separate treatment:

- A path lexically inside a worktree may resolve through a symlink to something outside it. The helper alone does not establish file containment. Whether a specific SDK tool adds its own guard must be tested; do not claim an exploit solely from this helper.
- Even perfectly contained editing can change `build.rs`, tests, Cargo configuration, or other programs that a permitted Cargo check executes. With inherited process authority, “no shell tool” does not mean “cannot execute arbitrary code.” The model need not call a shell directly to run code or read its environment.

The configured SDK environment removes selected Anthropic variables, but the check subprocess is created directly in the host process environment. No explicit environment allowlist, secret-free HOME, network policy, OS sandbox, or protected evaluator mount is established by `run_check`. The full-permission delegate configuration elsewhere in `bundled_runtime.rs` is an intentional local-owner mode; it must not be confused with a customer-code isolation guarantee.

**Required boundary:** treat candidate source as executable input. Keep source editing, test execution, checker authority, credential custody, and publication rights separate. The fixed plan and protected tests should be outside the candidate's writable authority. Run required checks under an existing confinement/provider owner with explicit writable paths, credentials, network policy, process ownership, and resource limits. A candidate may propose evaluator changes, but those changes need a separately reviewed evaluation rather than silently replacing the evaluator for their own acceptance.

**Acceptance experiment:** in an isolated fixture, include an internal symlink pointing outside the candidate, an attempted evaluator edit, a build script that reads a synthetic secret, and a test that launches a child process. Verify denial or containment, no access to the synthetic secret, complete child cleanup, and no publication credentials in the checker. Use only synthetic state and a scratch HOME.

## RUN-05 — Check process lifetime and capacity are not consistently owned

**Priority: P1. Confirmed for the new runner.** `agent::run_check` calls `tokio::process::Command::output()` directly. It has no local deadline, streaming output bound, process-group termination, or explicit build-lease acquisition. The session timeout applies to the SDK stream loop; it does not by itself prove that final checks or a tool subprocess stop. Output trimming happens after all output has already been buffered.

This means one generated test can hang finalization, one noisy test can consume memory, and parallel issue-runs can build outside the intended capacity owner unless the caller happens to supply a leased environment or Cargo shim. An inherited shim can mitigate this in a particular launch, but the source path does not enforce it.

The A/B remote executor does have timeout and host locking machinery in `remote/run.sh`, `remote/eval.sh`, and `remote/common.sh`. It uses a separate `flock`-protected benchmark checkout and target. The established issue-flow gate acquires a target slot and counted build lease. These are useful controls, but they are not a single capacity contract across entry points. Benchmark measurements also need to report time waiting for a build lock separately from active execution.

**Acceptance experiment:** cancel a run while the checker has a child and grandchild, interrupt the remote connection, and exceed both wall-time and output limits. The recorded result must be cancelled, failed, or unknown; all owned processes must end or remain visibly unresolved. Launch competing checks under a scratch lease root and prove the configured concurrency limit holds. Reuse long-lived external targets and the existing build/quiet leases; do not introduce per-attempt cold target directories or a competing scheduler.

## RUN-06 — New issue-runs share a destructive default worktree

**Priority: P1. Confirmed in source.** [`setup::prepare`](../../../crates/coder-new/src/issue_run/setup.rs) uses a fixed `STATE/worktree`. If the path exists, preparation attempts `git worktree remove --force`, then `remove_dir_all`, and prunes worktrees before creating the next one. It has no run-specific ownership lock in this code. The event folder uses issue number and seconds, which is also not a collision-resistant run identity.

A second run with the same state root can remove the first run's active working copy. An operator can also start a new run after a failed attempt and lose the old worktree's uncommitted material. A retained patch helps only if capture completed successfully. Summary and patch writes currently ignore some filesystem errors, so the worktree must not be treated as disposable merely because finalization was attempted.

Open-issue setup also ignores a failed fetch before using `origin/main`; a stale local remote-tracking ref can become the base without an explicit freshness failure. Closed-issue replay infers a fix by commit-message matching unless a fix is supplied. That is useful discovery, not authoritative issue-to-fix attribution.

**Acceptance experiment:** run two preparations against one state root and kill one between editing and patch capture. Neither run may delete or relabel the other's files. Recover the first candidate with its original base, check plan, and run identity. Make fetch fail and require an explicit stale-base state. For historical benchmarks, freeze and review issue, fix, parent, and test-overlay identities before execution.

## RUN-07 — Replay is useful evidence, but independence and receipt custody are incomplete

**Priority: P1 for automatic learning admission. Confirmed source gaps, with substantial positive evidence.** [`traces.py`](../../../scripts/bench/traces/traces.py) verifies the patch digest, resulting tree in a clean index, changed-file list, and repeated check fields. It rejects mismatches and retains negative examples. That is a meaningful improvement over learning from agent summaries.

The [retained manifest](../../coder/traces/2026-10-10-manifest.json) contains 41 verified A/B traces across three issues: 18 from #10074, 22 from #10228, and one empty-patch negative outcome from #10273. Their checks pass in 22 cases and fail in 19. The [closing report for #11218](https://github.com/OpenAgentsInc/openagents/issues/11218#issuecomment-6097838645) records 75 resulting corpus items but names only the first two issues; the manifest supplies the complete issue count. Four older issue-run folders were unverifiable because they lacked patches. These numbers support a working replay mechanism on a narrow historical cohort. They do not establish 41 accepted code changes, a broad unseen-issue evaluation, or automatic trace capture for every ordinary issue-run. The new issue-run writes the inputs needed for capture; the A/B runner actually invokes capture. Replay and admission remain explicit later operations.

The independence limit is concrete. Host replay calls a mutable `~/ab/bin/eval.sh`; the record does not bind the full verifier/toolchain/environment identity. A clean checkout isolates candidate state but does not create independent authority when the same operator controls the candidate, harness, and receipt store. Local replay uses the current overlay helper. Additional check inputs, such as the tests patch, require the same digest verification discipline as the candidate patch.

At admission, the script selects a verified replay record by trace ID; it does not recompute the receipt's own digest or rebind all current trace/check inputs to that receipt. Under trusted same-user scratch custody this can be acceptable research tooling. It is insufficient for a customer-facing statement that no modified or substituted record can enter the corpus. Malformed JSONL records are skipped by the reader, which can hide evidence loss unless surfaced as a count and error.

**Acceptance experiment:** change only the checker script, test overlay bytes, toolchain identity, trace record, and replay receipt in separate cases. Admission must reject or require a new replay. Kill capture, replay append, and admission at each durable boundary and verify that partial evidence never becomes accepted. Record both the execution relationship (same host/different checkout, separate host, isolated evaluator) and the authority relationship; avoid calling every clean-worktree replay “independent” without that qualification.

## RUN-08 — Required check coverage differs across the three code-change owners

**Priority: P1. Confirmed architecture gap.** The briefed verifier chooses packages from changed `.rs` files with a small manifest parser, then falls back to configured packages. It does not account for all manifest, lockfile, build configuration, non-Rust fixture, or direct-dependent changes. It selects packages through `-p`, although this repository includes a separate `openagents-mobile` workspace that needs `--manifest-path`.

The main verification scripts recently improved nested-workspace selection and direct-dependent coverage, as recorded in the health audit remediation. The established issue gate has its own selection and confinement logic. Those improvements do not automatically repair the briefed-agent or new issue-run selectors. A repository product needs one typed check-plan owner, with adapters for quick feedback, final checking, replay, and landing.

This should not become an excuse for running the entire workspace for every task. Repository policy explicitly permits documentation-only changes without Rust checks and ordinary Rust work with focused package checks and formatting. The objective is correct declared scope, including relevant consumers when needed. Metadata failure must produce an explicit incomplete coverage state rather than an empty dependency set that looks authoritative.

**Acceptance experiment:** freeze expected plans for a Rust source change, a manifest feature change, a shared crate API change, a lockfile change, a generated fixture change, a nested-workspace change, and a documentation-only change. Confirm that all entry points select equivalent required coverage and that an unavailable metadata command cannot silently reduce it. A changed plan invalidates earlier final evidence.

## Reuse the stronger owners before adding another orchestrator

The established issue flow already separates engine completion from repository acceptance, coordinates claims, waits for task checks, repairs failures, preserves unfinished work, and reports landing results. Its landing hook serializes within the local process and a shared local file lock, rebases through the landing owner, and can recheck the moved candidate. Local file-lock failure can fall back to process-local coordination, and this is not a cross-host integrator. Nevertheless, it is a substantially stronger starting point than making the prototype's `open_pr` responsible for all these transitions.

The environment verifier is another strong reference. Its `Inputs` binds the image, manifest, plan digest, install digest, lock digests, platform-related identity, and evidence budget. Its model distinguishes failed, incomplete, cancelled, and passed. Cleanup uncertainty remains visible. [#11001's closing evidence](https://github.com/OpenAgentsInc/openagents/issues/11001#issuecomment-6071409532) explicitly distinguishes focused fake-provider/local-shell tests from later real-provider qualification. This is the right evidence vocabulary for code-change verification too.

The [earlier health audit](../2026-10-10-codebase-health-audit/cross-cutting/tests-ci-verification.md) correctly identified large test inventories with incomplete orchestration and silently skipped infrastructure cases. Its generic CI recommendations must be adapted to this repository's prohibition on GitHub workflows and GitHub-billed automation. Run the selected checks manually or on approved non-GitHub infrastructure, retain machine-readable results, and make missing infrastructure explicit. Test count is not proof of the issue-to-improvement loop.

## Release evidence for this part of the product

Before claiming an accepted self-improving code change, retain one complete example from authorized issue through exact checked tree, review or policy acceptance, landing, and replay. Then exercise failure cases deliberately: unavailable checker, empty tests, changed untracked bytes, evaluator edits, symlink escape, hanging child, competing runs, stale base, disk-full evidence write, changed base during landing, interrupted publication, and modified replay receipt.

The first qualification can be narrow: one repository, one engine, one owner-approved task class, one protected evaluator, and one landing route. Measure both successful and failed attempts. Expand engines, providers, surfaces, and autonomy only when their authority and evidence paths meet the same contract. That gives the learning system trustworthy outcomes without requiring a new general-purpose orchestration platform first.
