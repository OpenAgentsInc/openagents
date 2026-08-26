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

You may change: the coder system prompt and tool descriptions
(`crates/coder-lite`, `packages/openagents-cli`), tool budgets, plugins
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
- **A packed CLI tarball** from the working tree — this is what the adapter
  installs into containers, so it is how your change reaches the arena:

  ```sh
  cd packages/openagents-cli
  pnpm build
  pnpm pack --pack-destination ../../bench
  ```

  `pnpm pack`, never `npm pack` (best practice R1).
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

Review prompt skeleton:

```
You are reviewing one autoimprovement cycle of `openagents coder`.

<lever>{what changed, and the predicted delta}</lever>
<baseline-row>{jsonl row}</baseline-row>
<result-row>{jsonl row}</result-row>
<trials>{per-trial: instruction, verifier decision, ATIF metrics,
notable transcript spans}</trials>
<diff>{the lever's diff}</diff>
<practices>{docs/coder/best-practices.md}</practices>

Score the cycle 0–10 with specific evidence for each point gained or
lost. Answer: did the lever cause the delta, or does a confounder
explain it? Were any ledger practices violated (cite the entry and the
transcript step)? Propose one to three changes, each typed as
{lever, evidence: trajectory steps, risk, verification: suite and
expected delta direction}. Finally, list ledger entries to add, promote,
demote, or refute, with provenance.
```

The review must cite trajectory steps for every claim; a proposal without
an evidence pointer is rejected at the adopt step. Save the output to
`docs/coder/reviews/YYYY-MM-DD-<lever-slug>.md`.

## 7. Record, adopt, land

1. **Adopt** the accepted proposals: apply ledger changes to
   `docs/coder/best-practices.md` (checking new entries against existing
   ones for contradiction), carry rejected proposals into the review file
   with a one-line reason.
2. **Commit** the cycle as one unit: the lever, the review file, the ledger
   change, and the appended store rows. The commit message states the lever
   and the measured delta with its suite — a number, not an adjective
   (best practice V3).
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
  0.5 success (two runs), plus the deliberate `qwen3:0.6b` regression row.
  No proxy-lane rows recorded yet; no cross-section rows recorded yet.
- `owned-closed-issues` is a smoke suite until its environments exist
  (`bench/tasks/owned/README.md`).
- Cached-token splits are not surfaced end to end
  (OpenAgentsInc/openagents.com#220); until then, metered-lane dollar
  figures are ceilings, and lane comparisons lean on success rate and
  rounds.
- The PTY-driven interactive harness (autoimprove §7.4) does not exist
  yet; best practice V2 is enforced by rule, not by gate.
