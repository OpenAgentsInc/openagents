# Coder autoimprovement runbook

The operating procedure for an agent running the loop in
`docs/coder/autoimprove.md`. Follow it top to bottom the first time; after
that, each cycle is §4 (a process or code lever) or §5 (a plugin), always
followed by §6 (review) and §7 (record and land).

Read first, in order:

1. `docs/coder/autoimprove.md` — the plan and the verification law.
2. `docs/coder/best-practices.md` — the ledger. You will update it.
3. `bench/README.md` — harness mechanics, environment pitfalls.
4. `bench-results/README.md` — what the store accepts and refuses.

## 0. Authority and boundaries

You may change: the Coder system prompt and tool descriptions
(`surfaces/coder` and `crates/openagents-cli`), tool budgets, plugins
(`plugins/`), suites and thresholds (as their own change — best practice
M4), this directory's docs, and CLI code under the repository completion
gate.

You may not: edit or reorder `bench-results/*.jsonl` rows (append only,
through the tooling), record a smoke or partial run as a score, delete the
deliberate regression row in `tb2-quick.jsonl`, close an interactive-TUI
issue on headless evidence (best practice V2), or change a threshold in the
same change as a run it would flatter.

Work in a fresh worktree per unit (`git fetch openagents main && git
worktree add --detach <path> openagents/main`), and land by pushing to the
forge remote, never GitHub (best practices R2, R3). For CLI code changes
the completion gate is `pnpm run check`; docs-only changes push with
`--no-verify` after the docs checks.

A change to the Coder TUI requires its pseudo-terminal suite:
`cargo test -p openagents-cli --test coder_interactive_pty`. It drives the real
binary and asserts on rendered cells; headless evidence does not close a TUI
issue.

## 1. Prerequisites

- **Docker with amd64 emulation that works.** Terminal-Bench images are
  amd64. On Apple Silicon, enable Rosetta in Docker Desktop (Settings →
  General → "Use Rosetta for x86_64/amd64 emulation"); under plain qemu the
  verifier's `uv`/`pytest` segfaults after the agent phase and every trial
  grades `ungraded`.
- **Harbor** installed (the clone is `../projects/repos/harbor`;
  `pip install harbor` or run from the clone).
- **A dev forge**, for proxy-lane runs: in the openagents.com repo,
  `PHX_LISTEN_ALL=true mix phx.server`, and an `OPENAGENTS_TOKEN` with
  `chat:account` scope. If another session holds the port loopback-only,
  bridge `0.0.0.0:4001 → 127.0.0.1:4000` and set
  `OPENAGENTS_CODER_API_URL=http://host.docker.internal:4001` instead of
  fighting for the port.
- **A native Linux CLI build** from the working tree. The suite runner builds
  and installs it in each Harbor environment. To inspect the build command
  without running the suite:

  ```sh
  bench/run-suite.sh bench/suites/tb2-quick.suite.json \
    --model openai/gpt-5.6-luna --lane proxy --dry-run
  ```

- **Ollama** with the local-lane model pulled, for local-lane runs. Local
  runs need no token; keep `--n-concurrent 1` because the model owns the
  cores.

## 2. Baseline

Establish the number you are trying to move, on the lane you are testing.

```sh
bench/run-suite.sh bench/suites/tb2-quick.suite.json \
  --model openai/gpt-5.6-luna --lane proxy \
  --jobs-dir /tmp/gym-jobs/baseline
```

Then score and record it:

```sh
pnpm run effectiveness:report -- /tmp/gym-jobs/baseline/<job-dir> \
  --suite tb2-quick --lane proxy \
  --suite-manifest bench/suites/tb2-quick.suite.json \
  --thresholds packages/coder-effectiveness/thresholds/tb2-quick.json \
  --append bench-results/tb2-quick.jsonl
```

Exit codes: `0` gate passed, `1` a floor breached, `2` unverifiable
(not a pass), `3` scored but refused by the store — read the refusal, it is
telling you the run does not qualify as a score. Verify the chain and read
the trend:

```sh
pnpm run effectiveness:compare -- bench-results/tb2-quick.jsonl
```

With `OPENAGENTS_TOKEN` set, the runner registers the run at
`POST /api/v1/gym/runs/start` and `/gym` shows it live; finalization to
`graded` (or `abandoned` when no verifier ran) happens through
`bench/post_gym_run.py --run-id` automatically.

If no baseline exists yet for the lane and suite you are working, the
baseline run **is** the first cycle's deliverable. Record it and stop
there; a lever with no baseline produces a delta against nothing.

## 3. Choose one lever

One per cycle (best practice M1). Sources, in order of cost:

1. **The ledger's `proposed` entries** (`docs/coder/best-practices.md`) —
   each names its promotion oracle.
2. **The latest reviews** (`docs/coder/reviews/`) — unadopted proposals
   with evidence already attached.
3. **The harvest backlog** (openagents.com repo,
   `docs/2026-08-25-plugin-harvest-targets.md`) — plugins ordered by
   expected delta; go to §5.
4. **Structural gaps** (autoimprove §2.3: compaction, history, shell
   parsing) — only when cheaper levers have stopped paying, and with the
   designated suite oracle named before you start.
5. **Optimizer candidates** (autoimprove §2.4), once the lane exists — a
   GEPA candidate is a lever like any other and enters at §4 with its
   evidence rows, transfer label, and stated acceptance floor. It does not
   land on the optimizer's say-so (ledger O1).

Write down, before implementing: the lever, the suite that will measure it,
and the delta direction that would confirm it. If you cannot name the
measuring suite, the lever is not ready for a cycle.

## 4. The cycle: process and code levers

1. **Implement** in a fresh worktree. Prompt/tool-description levers land
   in the coder's system prompt and tool declarations; budget levers in the
   tool budget; code levers under `pnpm run check`.
2. **Repack** the CLI tarball (§1) so the arena runs your change.
3. **Re-run the same suite, same model, same lane** as the baseline.
   Nothing else varies. The runner pins CLI version into the row, so the
   two rows differ in exactly the axis you changed.
4. **Compare.**

   ```sh
   pnpm run effectiveness:compare -- bench-results/tb2-quick.jsonl
   ```

   Read the trend line and the confounder note. On `tb2-quick`, treat the
   delta as a selector, not a conclusion (best practice M5): an
   encouraging quick delta earns a cross-section run —

   ```sh
   bench/run-suite.sh bench/suites/tb2-cross-section.suite.json \
     --model openai/gpt-5.6-luna --lane proxy --n-concurrent 2 \
     --jobs-dir /tmp/gym-jobs/xsec-<lever>
   ```

   — scored against `thresholds/tb2-cross-section.json` and appended to
   `bench-results/tb2-cross-section.jsonl`. Success rate is not the only
   axis: rounds, prompt tokens, and wall clock per accepted outcome are
   where process levers show first (the fix-git analysis is the worked
   example).

5. **Decide.** Improved or neutral-but-simpler: keep. Worse: revert the
   lever, keep the recorded row (best practice M3), and write the refutation
   into the review. Timeout-shaped failures are not efficiency signal —
   rerun with `--timeout-multiplier 2.0` before concluding anything.

## 5. The cycle: plugins

1. **Pick the target** from the harvest backlog and confirm it fits the
   admitted shape (best practice P2): read-only or pure, one-shot, bounded,
   typed, truncation-honest. If it needs writes, execution, or a lifetime
   beyond one call, it is core-tool work, not a plugin.
2. **Build** in `plugins/<name>/` on `openagents-pdk`: one
   `fn handle(input) -> Result<Output, Refusal>` plus `plugin_entry!`.
   Declare only the imports the manifest's mounts grant — the host refuses
   undeclared imports.
3. **Pin the artifact.** Build for `wasm32-unknown-unknown --release`, copy
   the `.wasm` beside the source, and update the manifest's
   `artifact.digest` (`plugins/README.md` has the exact loop). The host
   refuses a stale digest — that refusal is the supply chain, keep it.
4. **A/B on the oracle tasks.** Same suite, same recipe, plugin absent then
   present. The cross-section's designated oracles: tasks 1–2
   (`git-leak-recovery`, `sanitize-git-repo`) for git forensics, task 9
   (`password-recovery`) for file forensics; for a new capability class,
   name the oracle tasks in the landing change. Confirm in the ATIF that
   the with-plugin run actually invoked it (`tool.ran` steps carry the
   digest); a plugin installed but never called on its own oracle tasks is
   a catalog-description problem (best practice P3) before it is a
   capability problem.
5. **Land with the delta** (best practice P1): both rows recorded, the
   attribution stated in the change. No delta and no convincing rationale:
   the plugin waits.

## 6. Review

After every cycle, run the review as a **separate conversation** — a fresh
agent context whose only inputs are the artifacts:

- the trial directories (`result.json`, `trajectory.json` ATIF,
  `coder.txt`) from the run,
- the diff of the lever,
- the two store rows (before and after),
- the current `docs/coder/best-practices.md`.

Assembling those by hand is what `coder:review` does mechanically
(OpenAgentsInc/openagents#121):

```sh
pnpm run coder:review -- /tmp/gym-jobs/<job>/<run> \
  --suite tb2-quick --lane proxy --lever HEAD~1 \
  --slug <lever-slug> \
  --reviewer-model <a model from a different family than the cycle ran on>
```

It reads the job directory, redacts everything through the one ATIF rule
list before it leaves the process, renders the prompt with the citable
evidence refs printed in it, asks the reviewer, checks every citation, and
writes `docs/coder/reviews/YYYY-MM-DD-<lever-slug>.md` plus the
machine-readable review beside it. Name the lever with `--lever <ref>`,
`--diff <file>`, or `--no-diff` for a baseline: a review that cannot see the
change cannot attribute the delta to it. The reviewer is reached over
`/api/v1/responses` at `--api-url` (default `$OPENAGENTS_CODER_API_URL`,
else `http://localhost:4000`) with `$OPENAGENTS_TOKEN`.

**Every claim carries refs, and the refs are checked.** The grammar is
`trial:<task>#step-<id>`, `trial:<task>#outcome`, `row:<suite>#<recordedAt>`,
`ledger:<id>`, and `diff:<path>`, resolved against the artifacts the
reviewer actually read — a truncated trajectory's dropped steps are not
citable, and the prompt says so. Every proposal must cite at least one
trajectory step: a store row or a ledger entry says what changed, never
what the coder did. A review with an unresolvable ref is refused whole,
by name, and nothing is written.

Exit codes: `0` accepted and written, `1` the reviewer answered and the
answer was refused (the named reasons are on stderr), `2` the review could
not be run at all — no job directory, no lever, no reviewer.

Two flags matter for the lanes below the live one:

- `--print-prompt` renders the prompt and exits without asking anyone. This
  is the manual §6 lane: paste it into a fresh agent context and save the
  answer.
- `--offline <file>` **replays** a recorded reviewer response instead of
  asking a model. It replays; it never generates. Its ref says `replay:` in
  the review file, so a replayed review cannot be read as a fresh judgment.
  There is no third behavior — a canned score would be the exact failure
  this command exists to prevent.

## 7. Record, adopt, land

1. **Adopt** the accepted proposals: apply ledger changes to
   `docs/coder/best-practices.md` (checking new entries against existing
   ones for contradiction), carry rejected proposals into the review file
   with a one-line reason.
2. **Commit** the cycle as one unit: the lever, the review file and the JSON
   written beside it, the ledger change, and the appended store rows. The
   commit message states the lever and the measured delta with its suite — a
   number, not an adjective (best practice V3).
3. **Push to the forge** (`git push openagents HEAD:main`), reconcile the
   canonical checkout, remove the worktree.

## 8. Stop rules

- **Three consecutive failures of the same gate** — the suite will not
  grade, the store refuses, the environment will not come up — stop
  attempting it. Preserve the last failure, write the smallest
  reproducible blocker and the next falsifiable hypothesis, and hand off.
  A fourth attempt needs new evidence.
- **An ungraded epidemic** (`ungraded` trials, `uv`/`pytest` segfaults)
  is the emulation problem, not your lever. Fix Rosetta or move to a cloud
  environment (`--env daytona` and peers) before reading any number.
- **A quick-suite success rate of 0** on a lane that previously scored:
  suspect the lane (auth, proxy URL, model availability) before the lever;
  the third row of `tb2-quick.jsonl` is what a lane-degradation signature
  looks like.
- **Budget**: a cross-section run is ~12 tasks × minutes-to-hours under
  emulation. Do not start one you cannot let finish; an abandoned run is
  recorded as `abandoned`, and that is the honest state, but it bought
  nothing.
- When blocked on an owner action, write the `NEEDS-OWNER:` note and pull
  the next non-blocked lever; the loop does not idle.

## 9. Current state (update as it changes)

- Baselines on record: `tb2-quick`, local lane, `qwen3.8:27b-mtp-q8_0` at
  0.5 success (two runs), plus the deliberate `qwen3:0.6b` regression row;
  `tb2-quick`, proxy lane, `gpt-5.6-luna` at 1.0 success over 2 graded
  trials, 221.9s, gate passed (2026-08-26, #118); and `tb2-quick`, proxy
  lane, native Coder Flash `glm-5.3-flash` at 0.5 success over 2 graded
  trials, 462.1s, gate passed (2026-08-28, #143, Harbor job
  `7695aa83-ece2-4083-8092-4968b6e738bf`). Flash accepted
  `openssl-selfsigned-cert` and rejected `regex-log` after the container
  could not reach `https://openagents.com/api/inference/proxy` (agent exit
  2). Cost is `unknown` because `glm-5.3-flash` is absent from the rate
  catalog. That Flash row is the current native Coder Flash baseline
  for a run whose agent opened a turn. Two later Flash `tb2-quick` rows
  (2026-08-28T03:06Z and 03:18Z, jobs `72928cbc` and `8600a533`) are 0 of
  2: both trials died with the inference proxy unreachable before any
  tool call. Those rows stay in the chain. They are lane degradation, not
  a T2 verdict (`docs/coder/reviews/2026-08-28-t2-stat-before-p.md`).
- Native Coder Flash `tb2-cross-section`, proxy lane, `glm-5.3-flash`: 7 of
  12 accepted, 0.583 success over 12 graded trials, 0 ungraded, 4243.4s,
  gate passed (2026-08-28, #143, Harbor job
  `5ea06a86-b084-4afa-890c-c64784125adb`). Rejected: `openssl-selfsigned-cert`,
  `password-recovery`, `regex-log`, `sanitize-git-repo`,
  `schemelike-metacircular-eval`. Three of those (`regex-log`,
  `openssl-selfsigned-cert`, `schemelike-metacircular-eval`) ended with
  `https://openagents.com/api/inference/proxy` unreachable from the
  container; `password-recovery` and `sanitize-git-repo` ran the model and
  failed the verifier. Gym PATCH finalize still 422s; the local store
  row is the evidence.
- `tb2-cross-section`, proxy lane, `gpt-5.6-luna`: 9 of 12 accepted, 0.75
  success over 12 graded trials, 0 ungraded, 12 of 12 pinned tasks covered,
  3151.4s, 6,006,329 prompt and 88,163 output tokens over 250 tool calls
  (2026-08-26, #118). Rejected: `count-dataset-tokens`,
  `fix-code-vulnerability`, `sanitize-git-repo`. An earlier attempt the same
  day reached 1 of 12 and was recorded `abandoned` when Docker Desktop failed
  to start mid-suite (the host volume was full); that row is not in the store,
  only on the Gym.
- **That run's gate is `unverifiable`, not passed, and the report exits 2.**
  Four of five criteria pass. `maxCostPerAcceptedOutcomeUsd: 2.0` cannot be
  measured because `gpt-5.6-luna` is unpriced, and `failed` beats
  `unverifiable` beats `passed`, so a fully covered, fully graded, healthy
  run still cannot exit 0. This is #125, now confirmed by a completed run
  rather than predicted from the code. The threshold was not edited to make
  the run pass (best practice M4).
- No `tb2-cross-section` row on the local lane yet, so the store holds one
  run shape and `effectiveness:compare` has nothing to compare it against.
- Rosetta note, measured 2026-08-26: on this rig the verifier's `uv`/
  `pytest` does **not** segfault under emulation. `uv 0.9.5` plus
  `uvx pytest` returned `pytest 9.1.1` in an amd64 container with
  `/run/rosetta/rosetta` mapped in. `bench/README.md` still documents the
  segfault as a standing constraint; it holds for plain qemu, not for a
  Rosetta-enabled Docker Desktop.
- `owned-closed-issues` is a smoke suite until its environments exist
  (`bench/tasks/owned/README.md`).
- First CoderBench suite `coderbench-agent-building-v1` is smoke: two
  Harbor-runnable Terminal-Bench 2.0 pins until agent-building environments
  exist. One smoke-marked row recorded 2026-08-28 (#164 D5), same Harbor job
  as the Flash `tb2-quick` baseline.
- Cached-token splits are not surfaced end to end
  (OpenAgentsInc/openagents.com#220); until then, metered-lane dollar
  figures are ceilings, and lane comparisons lean on success rate and
  rounds.
- The PTY-driven interactive harness (autoimprove §7.4) exists for
  coder-lite as `cargo test -p coder-lite --test interactive_pty` (§0). The
  completion gate does not run it (#124), so best practice V2 is enforced by
  rule for every surface the gate does reach, and by that harness for the
  one it does not.
- The automated review (autoimprove §7.5) exists as `pnpm run coder:review`
  (#121). Its candidate schema, `openagents.coder_candidate.v1` in
  `packages/coder-review/src/candidate.ts`, is the object
  #122 staged surfaces for and #123's optimizer will mutate; a review
  proposal and an optimizer mutation are the same type, documented in
  `docs/coder/candidate-format.md`. No review has been recorded through it
  yet, so `docs/coder/reviews/` holds only its README.
- The optimizer lane (autoimprove §2.4) does not exist yet, but the text it
  would mutate is now staged (#122): `surfaces/coder/`, pinned by digest and
  guarded by `check:coder-surfaces`. Until #123 lands, every cycle is still a
  hand-written lever, and the ledger's O-series applies to nothing running.
