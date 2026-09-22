# Terminal-Bench trace analysis: what three agents actually do

Status: measurement, September 22, 2026. Every claim below is read from a
retained ATIF `trajectory.json` or a verifier artifact; the trajectories are
checked in under `bench/terminal-bench/traces/` and each attempt's manifest,
usage, cost, and verifier output live in the job dir under
`~/.openagents/terminal-bench/jobs/`. The upstream task pin is
`3b5caaa4863d64dda7f0957bf4fc2d4f019202d4`. This document feeds the v0.5
design ([algorithm and goldens](../design/coder-terminal-v05-algorithm-and-goldens.md)):
it names the observed behaviors a Coder v0.5 controller should adopt, and the
observed failure modes it should make structurally impossible.

## Scoreboard

| Arm | fix-git | build-cython-ext | fix-code-vulnerability | cancel-async-tasks | headless-terminal |
| --- | --- | --- | --- | --- | --- |
| claude-code 2.1.278 / fable-5.1 | 1.0 | 1.0 | 1.0 | 1.0 | 1.0 |
| claude-code 2.1.278 / sonnet-4.5 | 1.0 | 0.0 | — | — | — |
| codex 0.153.3 / gpt-6-astra | 1.0 | 1.0 | 1.0 | 1.0 | 0.0 |
| devin 3000.11.1 / swe-2-high | 1.0 | 1.0 | 1.0 | no result | no result |
| oracle / nop controls | 1.0 / 0.0 | 1.0 / 0.0 | — | — | — |

Single trial per cell; this is behavioral evidence, not a significance claim.
"no result" means no verifier reward exists: the trial record is
`unverifiable`, not a zero.

## Step-level shape

| Trial | agent steps | tool calls | observation bytes | tool mix |
| --- | --- | --- | --- | --- |
| claude/fable fix-git | 6 | 5 | 23K | Bash |
| claude/sonnet fix-git | 8 | 10 | 8K | Bash, Read, Edit |
| codex fix-git | 6 | 5 | 15K | exec |
| devin fix-git | 11 | 12 | 7K | exec, read, write |
| claude/sonnet build-cython-ext | 46 | 55 | 121K | Bash, Read, Edit |
| claude/fable build-cython-ext | 20 | 19 | 95K | Bash |
| codex build-cython-ext | 21 | 20 | 99K | exec |
| devin build-cython-ext | 42 | 95 | 137K | exec, grep, read, edit, todo |
| claude/fable fix-code-vulnerability | 4 | 3 | 5K | Bash |
| codex fix-code-vulnerability | 6 | 5 | 94K | exec |
| devin fix-code-vulnerability | 15 | 17 | 32K | exec, read, edit, write |
| claude/fable cancel-async-tasks | 8 | 7 | 1K | Bash |
| devin cancel-async-tasks | 20 | 21 | 16K | exec, write, get_output, edit |
| codex cancel-async-tasks-2 | 4 | 3 | 1K | exec |
| claude/fable headless-terminal | 8 | 7 | 70K | Bash, Write |
| codex headless-terminal | 7 | 6 | 4K | exec |

Observation bytes measure what the harness handed back to the model, not
prompt size. Devin additionally carries ~18 KB of system messages per episode
and codex ~6.5 KB; the Claude trajectory records none, which is an accounting
gap in ATIF, not an absence of prompt bytes.

## 1. `build-cython-ext`: the discriminating task

The task: make pyknotid 0.5.3 compile and work under NumPy 2.3.0 — a source
tree mixing `.py` and `.pyx` files peppered with removed NumPy aliases
(`np.int`, `n.float`, `n.complex`, …), a `fractions.gcd` removed in Python
3.9, distutils-era `setup.py`, and no `pyproject.toml`.

### The failing run: claude-code / sonnet-4.5, 46 agent steps, reward 0.0

Sonnet ran a reactive repair loop: hit an import error, grep for that error's
shape, edit the reported file, re-run, repeat. Every enumeration query was
filtered to `--include="*.py"`. Its final sweep (step 42) reported clean, and
its own test suite passed 18/18 — but `test_ccomplexity` in the verifier
calls `cython_higher_order_writhe`, which lives in `ccomplexity.pyx` and
contains `dtype=np.int` at lines 16, 44, and 75 (confirmed against upstream
0.5.3). The `.pyx` sources were never scanned. The agent ended with high
confidence built on a systematically incomplete evidence query.

Cost of the loop: 46 steps, 55 tool calls, three full `pytest` runs, and
roughly 30 read→edit→retest cycles where a single enumeration would have
sufficed.

### The passing runs

- **claude-code / fable-5.1 (20 steps):** read all four `.pyx` sources up
  front, ran one grep covering the whole deprecated-alias catalog across
  `--include=*.py --include=*.pyx`, rewrote all sites with one scripted pass,
  then exercised each extension directly (cython-vs-python parity checks),
  wrote `pyproject.toml` for build isolation, and verified the installed
  package *from `/tmp` outside the source tree* — catching any stale in-tree
  shadowing. Reward 1.0.
- **codex / gpt-6-astra (21 steps):** pulled large dumps (94–99 KB
  observations), detected the alias catalog with `rg`, applied scripted
  patches, verified. Reward 1.0.
- **devin / swe-2-high (42 steps, 95 calls):** enumerated `*.pyx`/`*.pxd`
  before touching anything (`find -name "*.pyx"`), ran catalog greps with a
  `.pyx` glob, made ~28 surgical edits, checked Cython-level semantics
  (`libc.math` `abs` usage in `.pxd` scope), used `pip install
  --no-build-isolation`, verified each extension from `/tmp`. Reward 1.0.

### What this establishes for v0.5

The failure was not reasoning quality; it was **evidence coverage**. Sonnet's
final grep answered "no deprecated aliases in `*.py`" and was reported as
"no deprecated aliases." A requirement-to-evidence judgment ("is the alias
scan complete for this repo?") would have flagged the uncovered `.pyx`
surface before the answer was trusted. This is the design doc's *relate a
requirement to evidence* operation made concrete: the missing piece is a
check on **coverage of the query**, not another repair step.

## 2. `fix-git`: recover a branch tip and land it

All arms scored 1.0 but took visibly different paths:

- **fable (6 steps):** `git status`/branches → inspect both candidate commits
  → `git checkout --theirs` on the conflicted file → commit. Investigated
  *before* acting; shortest trajectory in the set.
- **sonnet (8 steps):** reflog → cherry-pick → hit the merge conflict →
  read the file → hand-resolve → continue. Correct but reactive.
- **codex (6 steps):** batched its reconnaissance with
  `Promise.allSettled` parallel calls — one round trip gathered branches,
  reflog, and status.
- **devin (11 steps):** inspected both commits, wrote the resolved file
  manually, then byte-checked the file tail with `xxd`/`od` before
  committing — the only agent to verify exact bytes rather than trust the
  write.

For v0.5: batched evidence capture (codex) and post-write byte verification
(devin) are both worth encoding — evidence before action (fable) is the
ordering property the task frame already requires.

## 3. `fix-code-vulnerability`: three ways to spend context

All three scored 1.0 on the same report-plus-patch deliverable:

- **claude/fable:** 4 steps, 5 KB of observations — `git diff`, targeted
  `sed -n` reads, a verification snippet. Minimum viable evidence.
- **devin:** 15 steps, 32 KB — `read(offset, limit)` surgical excerpts, git
  history for intent, report artifact + patch + pytest.
- **codex:** 6 steps, 94 KB — few calls, but each observation was a large
  unbounded dump (whole diffs, long `rg` output).

Codex spent the least *turns* and the most *bytes*; claude the reverse. For
v0.5 context construction: bounded excerpts with retained originals (the
design doc's CTX contract) should win both directions — codex's dumps pay
input tokens, claude's minimalism risks the §1 coverage failure when the
task surface is wider than assumed.

## 4. `cancel-async-tasks`: self-verification under ambiguity

The task asks for `run_tasks` with bounded concurrency and cleanup that
survives Ctrl-C. Claude/fable (8 steps) wrote the implementation, authored
its own concurrency test, authored a dedicated SIGINT subprocess test, ran
both, and deleted its scratch files. Reward 1.0.

Codex's retry (4 steps, 3 calls) wrote the implementation, a limiter test,
and a Ctrl-C subprocess test in one pass, then exited cleanly. Reward 1.0.

Devin's attempt shows the hard case — and its trajectory *was* retained
despite the missing reward (20 steps, 21 calls). It implemented the limiter
in steps 8–9, then spent fifteen steps on the genuinely hard part: SIGINT
semantics through a subprocess harness. Its first test was invalidated by a
real observation — bash backgrounds jobs with `SIGINT` ignored — so it
iterated through five versions of `sigint_child.py`, reading
`asyncio.tasks._GatheringFuture`, `runners.Runner._on_sigint`, and `gather`
*stdlib source* to model what the verifier would exercise. The ceiling hit
mid-iteration on double-SIGINT behavior; the trial recorded
`RewardFileNotFoundError`. This is deep, correct investigation that spent
the entire budget on harness-debugging before shipping a verifier-facing
result — the record is `unverifiable`, and the behavioral lesson is that
checkpoint discipline (land a working implementation first, then probe edge
semantics) would have converted most of this effort into a reward.

## 5. `headless-terminal`: self-test surface versus verifier surface

The task: implement a `HeadlessTerminal` over a real interactive shell —
startup files, control keys, persistent state.

- **claude/fable (8 steps, 1.0):** read `BaseTerminal`, wrote the PTY
  implementation, exercised interactive Python, Ctrl-C, and `.bashrc`
  sourcing — the same axes the verifier scored.
- **codex (7 steps, 0.0):** implemented over `pexpect` in six calls, wrote
  its own test, reported success — but the verifier failed
  `test_startup_files` and `test_shell_state_persists_between_commands`.
  Its self-test covered a narrower surface than the verifier: interactive
  programs and output capture, not startup-file sourcing or state carried
  across commands. A second codex trial produced `turn.completed` in its
  own log but `NonZeroAgentExitCodeError` in the trial record — the same
  infra class as the resume-ordering bug below.
- **devin:** two trials, no reward. One produced partial agent work
  (pty+pyte design, mid-exploration) and no result; the other ran the
  install-and-execute path but exited inside the agent phase with no
  verifier reward. Both retained as `unverifiable`.

Codex's 0.0 is the second instance of the §1 failure shape: the agent's own
verification passed, but it exercised a subset of the verifier's surface.
"Did my test cover what the task actually requires" is a coverage judgment
again, not a test-count judgment.

## 6. Harness findings worth keeping

- **Credential scrubbing corrupts evidence:** Harbor replaces every env
  value whose *key* names a credential. A forwarded `CODEX_FORCE_AUTH_JSON=1`
  redacted every literal `1` in `result.json` (those corrupted trajectories
  are preserved in `traces/` — they now fail JSON parsing, which is itself
  instructive). Fixed by keeping selectors host-side; see
  `bench/terminal-bench/profiles/agents.json`.
- **Resume ordering bug:** on `harbor job resume`, `mkdir /logs/agent` ran
  *after* `codex exec`, so `tee` failed and the trial recorded
  `NonZeroAgentExitCodeError` despite codex's own log showing
  `turn.completed` with the work done. Upstream bug; retain the trial as
  infrastructure-failure evidence.
- **Model availability is per-account:** ChatGPT-account Codex rejects
  `gpt-5.2-codex`; the live catalog serves `gpt-6-astra`. Claude OAuth
  tokens are short-lived; Devin's weekly quota refused metered models but
  not the free `swe-2` tier. All three were recorded as provider/setup
  failures, not task failures.

## 7. Opportunities for v0.5, mapped

| Observed behavior | v0.5 operation it argues for |
| --- | --- |
| Sonnet trusted a `*.py`-scoped grep as proof of repo-wide cleanliness | *Relate requirement to evidence* must assess **coverage**, not just presence: a negative scan needs its search space stated and checked against the requirement's surface (file kinds, generated sources, vendored trees). |
| Sonnet re-ran full pytest after each micro-edit (3 full runs, ~30 fix cycles) | *Advance acceptance using evidence*: checks keyed to the exact artifact revision, invalidated only by overlapping writes; a repo-wide alias fix is one check, not N. |
| Fable enumerated the deprecated-alias catalog once, then rewrote all sites | *Propose a repair* should prefer **enumerate-then-rewrite** when the requirement is universal ("all uses of X gone") over iterative per-site repair. |
| Codex's 94 KB unbounded observation dumps | *Construct context for its recipient*: bounded excerpts plus retained originals and declared omission, exactly the CTX contract. |
| Fable verified the installed package from `/tmp`, outside the source tree | A cheap check kind worth a place in EVAL: **out-of-tree verification** defeats stale in-tree shadowing. |
| Devin's `xxd` tail-check after writing the resolved file | Byte-exact post-write observation: the write is unverified until the artifact is read back. |
| Devin enumerated `*.pyx`/`*.pxd` before editing; sonnet never did | *Capture evidence*: enumerate the artifact's language surface (extensions, build files, fixtures) as a mandatory first capture for repair tasks. |
| Devin's `todo_write` planning and per-file edit batches | Task frames make plans explicit state; PRG should retain them as inspectable structure, not prose. |
| Devin spent its whole budget probing SIGINT internals before landing a verifier-passing artifact | **Checkpoint discipline**: reach a complete, self-verified implementation early, then spend remaining budget on edge semantics — a bounded-episode scheduler should reserve budget for the acceptance path, not only for exploration. |
| Codex's headless-terminal self-test passed while the verifier failed on unsourced `.bashrc` | *Relate requirement to evidence* again, on the test-design side: the agent-authored check must be reconciled against the task's stated requirements (startup files, state persistence) before it counts as verification. |
| Every arm's system preamble (devin 18 KB, codex 6.5 KB; claude's unrecorded) | ATIF must record prompt bytes for every arm or cost accounting is incomparable — a contract fix, not an agent fix. |
| Provider refusals (quota, model gating, expired OAuth) recorded honestly | The shared status vocabulary (`provider_refusal`, `setup_failure`, `unverifiable`) already does this; keep it. |

## Methodology

- Each cell is one Harbor trial (`n_attempts=1`, no retries) inside Docker,
  pinned Harbor 0.22.0, pinned task commit `3b5caaa4`, agent installed
  in-container at the pinned version.
- Rewards come only from the task's own verifier; agent self-reports are
  ignored. Controls (`oracle`=1.0, `nop`=0.0) establish that the verifier
  discriminates.
- Trajectories are ATIF-v1.7 as emitted by each Harbor adapter; the
  `tbench` attempt record joins reward, status, usage, cost provenance,
  timing, and step/call counts, keeping unknowns unknown.
- Credential material is never committed: API keys travel as `${VAR}`
  templates, the Codex auth file is injected by Harbor, the Devin key is
  written to `credentials.toml`, and the Claude token is the host keychain's
  OAuth access token. Three trajectories corrupted by the credential
  scrubber are preserved as evidence of that failure mode.
- Limits: one trial per cell (no variance estimate); subscription model
  catalogs differ per account; swe-2 is a free tier so its usage/cost are
  `unknown` rather than zero; `math-eval-grader` remains excluded (H100).
