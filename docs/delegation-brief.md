# Delegation brief

Status: retained September 20 handoff. Its next-action list and issue states
are historical, not current instructions. Use the [master roadmap](roadmap.md),
[Coder migration tracker](coder/migration-status.md), and
[task guide](coder/guides/tasks.md) for the current ownership and delivery path.

Written 2026-09-20 to hand this work to the next agent. It says where things
stand, what to do next and why, and the constraints that are easy to lose.

Read [`AGENTS.md`](../AGENTS.md), the
[glossary](glossary.md), and
[`.agents/skills/google-developer-style/SKILL.md`](../.agents/skills/google-developer-style/SKILL.md)
before starting. Every piece of prose here follows that style guide.

## The goal

An operator says *clear the backlog*, and Coder runs a program that fans out
delegated sessions to do it, over our own relay, with every decision and
delegation recorded as an ATIF trace that `coderbench` can judge against a
golden.

[#9404](https://github.com/OpenAgentsInc/openagents/issues/9404) is the
roadmap. [#9413](https://github.com/OpenAgentsInc/openagents/issues/9413) is
the burndown that goal exists for.

## What works today

| Piece | State |
| --- | --- |
| ATIF traces | Every session records itself. Decision calls are first-class. |
| Headless turns | `coder -p`, and both modes share one turn so they cannot drift. |
| Delegation | Six real Devin sessions through Coder; 6 of 6 independently checked answers in a 48.4-second episode. |
| Capability probe | Shared host-approved probes distinguish present, absent, unavailable, unprobed, and unknown. |
| Program runtime | Runs `delegate-fan-out` from its definition. Zero faults against the golden. |
| Relay transport | Proven against production, 470 ms round trip. A worker exists. |
| CoderBench | Runs an episode, refuses when the machine is wrong, judges the trace three-valued. |
| Execution boundary | The host decides whether a turn may run commands. |
| Doors | Two gateway lanes, bounded streams, and model ids in exactly one file. |
| Subprocesses | One supervisor owns a job, its process tree, and what it prints. |
| Finding work | A `query` step resolves a named source, bounded and ordered, and records what it dropped. |
| The whole path | `coder -p "Delegate six…"` selects the program, fans out six real delegations, and lands no missing decisions or checks. |

## What to do next, in order

### 0. Know what a grade now means

[#9418](https://github.com/OpenAgentsInc/openagents/issues/9418) made the
grade three-valued, reusing `gym::gate::Verdict` rather than inventing a
second vocabulary. Two consequences will surprise you:

- **`coderbench diff` can no longer return a pass.** A trace carries neither
  the run's exit code nor the workspace, so a diff reports what it cannot
  see. Only `coderbench run` observes both. A diff of the observed golden
  exits `4`: its questions and answers match, but a file cannot establish
  the live process exit or the independent workspace observation.
- **Missing evidence is `unverifiable`, not a pass.** A delegation counts
  only if the trace says it completed, says it was correct, and holds an
  answer — unless the manifest pins the answers itself (`grade.expects`),
  in which case the manifest is the check and `correct` is only a claim.
  Without a manifest expectation, `correct: None` remains unverified.
  With one, the recorded answer can be checked independently.

`coderbench` now depends on `gym` for that type, which pulls `jev`, `tokio`,
and `reqwest` into its build graph. One vocabulary was judged worth the
weight; moving `Verdict` somewhere lighter is a reasonable future change.

### 1. Reconcile the remaining prerequisites

The sentence-driven path has run, but #9427 remains a prerequisite to unattended
work. Shared capability trust landed in `4010baadbc`; the filesystem boundary
and independent snapshots landed as a component in `b14cfedb83`. CoderBench
now observes filesystem snapshots independently (`54d33bec05`, with distinct
escaped path labels in `1aca30819a`). Runtime integration landed in `693e9d10ee`, with filesystem enforcement,
dispatch-time approval revalidation, and retained writing worktrees. The actual adapter and observed graded golden are now verified by the
[2026-09-20 live run](coder/measurements/2026-09-20-observed-fanout.md). Check current issue and worktree state before starting
another implementation of these pieces.

### 2. [#9427](https://github.com/OpenAgentsInc/openagents/issues/9427), held on purpose

The integrated path at `693e9d10ee` enforces filesystem writes through the
host boundary, holds resources through process cleanup, and retains writing
worktrees for review. Local process tests cover protected writes, cancellation,
changed approvals, and writable aliases of approval metadata. The actual Devin adapter passed the integrated six-session episode under this
boundary, and CoderBench independently observed an unchanged workspace. Manifest approval permits a probe; it
does not establish that the executor enforces its declared bounds.

Note `shell::run` gained a `Permit` parameter from #9415 and the bounded
form is now `run_within(proposal, permit, wall)`.

### 3. The observed golden now passes

CoderBench drove Coder from the operator's sentence at `34df6bc026` on
2026-09-20. All six Devin answers matched the manifest's expectations, the
workspace was unchanged under independent filesystem observation, the turn
exited successfully, and the live grade reported no faults in 48.4 seconds.
The golden is now `observed` with orchestrator `coder`; its bytes were copied
from the live log without editing the prompts or answers. The old staged trace
was replaced. Read [the run record](coder/measurements/2026-09-20-observed-fanout.md)
for the captured grade, hash, environment, and limits.

The manifest owns the answer check. `grade.expects` pins each prompt and answer
in request order; exact prompts and edge-trimmed, case-sensitive answers must
match. Self-asserted correctness and model completion judgments do not establish
these answers. Tests retain the distinction between a live run and an offline
trace: `diff` is still unverifiable because it cannot observe the exit or the
workspace. The driver does not yet serialize raw snapshot sidecars.

This completes the local observed-golden step. It does not complete #9404's
relay requirement or prove write-task conflict recovery. #9435 still owns the
relay/worker proof. The first burndown remains constrained as follows.

### 4. The first burndown, and not the one the issue describes

**Do not point a query-driven fan-out at the open backlog.** Run the first
one on issues chosen **by inspection**, with hosted Jev as the door, and the
independence decision in **shadow** — recorded, not acted on.

The reason is in the next section.

## The constraints that are easy to lose

### The independence gate is not safe

[#9414](https://github.com/OpenAgentsInc/openagents/issues/9414) measured it
on a 192-item factorial panel. On plans whose tasks **genuinely collide**, 11
of 12 answers cleared the 0.7 bound on local doors — `kev-8b` at 0.96 and
0.97.

**Raising the bound does not help: the wrong answers sit above the right
ones.** An `independent` accuracy of 0.75 hides this entirely, because most
panel items are genuinely independent, so a door that says "independent" to
everything scores well and refuses nothing.

The mechanism is worse than a bad threshold. `kev-4b` scores 0 of 16 on *"No
task writes a file"* and 14 of 16 on *"At least one task writes a file"* over
the same states, *p* = 0.00003. Those answers do not contradict — they
**agree**, correlating +0.95 where hosted Jev correlates −1.00. The door
answers the **topic** and attaches it to whichever proposition it is handed.
Calibration cannot fix that, because the signal is about the wrong question.

Hosted Jev is the only door measured that this gate can rest on.

### Our backlog collides

Several open issues touch `crates/gym`, and #9391 must land before #9401. A
`select` that returns the top N open issues hands a known-unsafe gate exactly
the input it fails on.

### The supervisor is not a sandbox, and says so

#9416 gives a job its own process group and terminates the tree on a
deadline or a cancellation — the audit's probe went from
`descendant_wrote_after_timeout=true` to `false`. Three limits are written
down rather than implied, and they matter before anything runs unattended:

- **It is not a sandbox.** It bounds what a job costs, not what it may reach.
- A descendant that calls `setsid` **escapes the group**. The output drains
  are bounded for exactly that case.
- The group is signalled microseconds after its leader is reaped, which is a
  pid-recycle window. `waitid(WNOWAIT)` would close it and is not on Tokio's
  wait path.

### Prefer an enforced boundary over a decision

#9409 built worktree isolation and verified it both directions. Where both an
enforced boundary and a decision are available, take the boundary. It does
not depend on a model reading a proposition correctly.

### Judge against the measured floors

- Accuracy: **0.056** for a two-door comparison, two sigma of a 0.0197 seed spread.
- Calibration, derived in #9376: ECE **0.0266**, Brier **0.0119**, NLL **0.6428**.
- Confident errors: σ **2.49**, which is why that criterion refuses an unchanged door about half the time ([#9401](https://github.com/OpenAgentsInc/openagents/issues/9401)).

**Five claims were withdrawn in one week for not clearing their own noise**,
three of them because the floor was measured after the claim was published.
Measure the floor first.

### Check headroom before running an experiment

[#9392](https://github.com/OpenAgentsInc/openagents/issues/9392) ran a
complete text-optimization experiment against a partition scoring 0.975 at
baseline. Total headroom was 0.025 against a floor of 0.056, so **no win was
available at any strength, whatever it tried.** Publish the headroom beside
the digest.

### The selection question loses to a constant, and shipped anyway

The first measurement of program selection is the clearest instance of the
trap below, and worth reading before adding any question.

On 32 real turns the constant `none` scores **0.969**. Hosted Jev scores
**0.938** — below the constant. Headroom was 0.031 against a 0.056 floor, so
**no win was available at any strength**, which is #9392's lesson arriving a
second time.

It shipped because the **error structure**, not the accuracy, is what
matters here: 0 false negatives in 9, 3 false positives in 35, and all four
mistakes chose `answer-question` rather than `delegate-fan-out`. **None ran
anything**, because a program cannot fan out over work the request never
named. The cheap error is the one that happens, and it is structural rather
than threshold-dependent.

Keep that distinction. An accuracy figure would have refused this feature or
approved it for the wrong reason.

### A question that is nearly always the same answer is a trap

[#9395](https://github.com/OpenAgentsInc/openagents/issues/9395) found six of
seven production questions score no better than a constant on real traffic.
Before adding a question, measure what a constant scores. And weigh errors by
what they cost: a missed program answers normally; a spurious one starts six
subprocesses.

### Evidence is not edited to stay tidy

A recording says what happened. Two renames invalidated two goldens today,
and both times the golden was **deleted and re-recorded** rather than having
its prompts rewritten. A recording of a world that no longer exists reads as
evidence and is not.

The same rule is why `Provenance` has three states: `observed`, `staged`,
`authored`. "Recorded" was covering both a recording of the program under
test and a recording of something else doing what it should do.

## Working notes

- **`cargo +1.97.1`.** The default toolchain is older and the workspace needs 1.95 or better.
- **Strict workspace Clippy passes at `763b84a258`.** Verified with Rust 1.97.1,
  `--workspace --all-targets -- -D warnings`, both with default features and
  with `--features kev/serve,lev/serve,gym/tui,jev/blocking`. This does not verify
  Metal, the minimum supported compiler, model-backed tests, or PostgreSQL.
  [#9429](https://github.com/OpenAgentsInc/openagents/issues/9429) still owns
  the toolchain pin, package policy, and complete manual verification gate.
- **`main` is not rustfmt-clean.** `cargo fmt --all` rewrites about 50 files across five crates. Six agents have now reverted that churn to keep a commit scoped. [#9402](https://github.com/OpenAgentsInc/openagents/issues/9402) fixes it and wants a quiet moment.
- **Credentials:** hosted Jev is `set -a; . ~/work/.secrets/typesafe.env; set +a`, and a local door key is at `~/work/.secrets/coder-local-door.env`. Both are machine-local and gitignored; never print either. `crates/jev`'s `Config` reads the process environment and loads **no** dotenv, so exporting first is required, and a missing key and a missing `model` field in the body produce different errors.
- **Local doors:** `~/work/kev-artifacts/` holds four kev checkpoints and their bases. `kev-serve` takes about 45 seconds to load on CPU and answers in roughly 2 seconds.
- **Devin:** at `~/.local/bin/devin`, **not on a spawned subshell's `PATH`**. It also refuses a workspace it does not trust, including a git worktree under `/private/tmp` — which is where agent worktrees live, so live delegation from one is refused.
- **The missing-delegation failure is a worktree race.** Repeated tests at
  `1f78b260e2` captured a failed `git worktree add` reading a sibling's
  `.git/worktrees/<id>/commondir` while that sibling was removed. The missing
  answer was a checkout-creation failure, separate from CoderBench's fixed
  temporary-output filename collision. #9442 adds a lock in the Git common
  directory for creation/removal across Coder processes and linked checkouts,
  while delegated work stays concurrent. Cancellation now keeps the checkout
  alive until the supervisor reaps the executor. See
  [worktree coordination](coder/runtime/delegate.md#coordinate-checkout-creation-and-cleanup).
  The earlier concurrency test's fixed 1.5-second ceiling was separately
  replaced by a comparison against summed delegation time in #9416.
- **Use a separate Cargo target directory per worktree.** During this handoff,
  sharing a target between the historical reproduction and current source
  reused a stale path-dependency artifact. The isolated verification target
  rebuilt correctly. A shared target's result is not acceptance evidence for
  a different source snapshot.


## The audit

`docs/audits/2026-09-19-codebase-audit/` reviewed the workspace at
`1843fa6c18` and reproduced failures through public APIs **while the
applicable test suites passed**. Its 25 findings are filed as #9415–#9433.

A01 to A04 are fixed. A05's wire-contract fix landed in `ed1cc8c8d3`:
Noul and Score now carry optional selected-answer provenance through Lev,
the Jev SDK, and Gym. New measurement rows use schema v2; historical rows
and receipt chains remain unchanged. The retained metric re-derivation keeps
published rounded spreads and gate verdicts unchanged, while reporting small
floating-point differences and missing evidence explicitly. See the
[post-fix verification](audits/2026-09-19-codebase-audit/verification.md#a05-after-the-wire-contract-fix).
Check #9419 and #9394 for the final acceptance review.

A06 is fixed in `9ec50704cb`; #9420 is closed. The locked-partition ledger
holds a cross-process lock through eligibility, append, and durable commit.
Canonical aliases share a lock, torn records fail closed, and overrides retain
their provenance. Main passed 245 Gym library tests and six ledger integration
tests; the full Gym suite and strict Clippy passed in the implementation
worktree. See [the ledger contract](gym/ledger.md) for local-filesystem and
Unix durability assumptions. This does not resolve #9399's training-data
contamination of the locked partition.

A09's UTF-8 half is fixed for the direct door as a
side effect of bounding it — the old loop ran `String::from_utf8_lossy` per
byte chunk, so a character split across a chunk boundary became replacement
characters in the answer **and in every trace of it**. The rest of A09–A11
is still #9423. The retained harness now reports the A01 to A03 probes closed, and the
record says plainly that a harness line observes a result while the tests
establish when a bound applies. The rest are
intended as burndown fodder — they are the first real workload for the system
this brief describes.

Read a finding before its issue. The audit's reasoning is better than any
summary of it, including this one.
