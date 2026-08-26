# Coder best practices

The knowledge ledger of the autoimprovement loop (`docs/coder/autoimprove.md`
§5). Every entry is falsifiable, carries provenance, and says how a violation
is detected. Statuses: `adopted` (in force), `proposed` (awaiting a measured
delta or an oracle), `refuted` (kept, struck through, so it is not
rediscovered). Reviews propose changes here; the adopt step of the runbook
applies them.

Provenance abbreviations:

- **fix-git** — openagents.com repo,
  `docs/terminalbench/2026-08-24-fix-git-run-analysis.md`
- **postmortem** — openagents.com repo,
  `docs/2026-08-26-rust-cli-port-parity-failure-postmortem.md`
- **bench** — `bench/README.md` and `bench-results/README.md`
- **harvest** — openagents.com repo,
  `docs/2026-08-25-plugin-harvest-targets.md`

## Verification

### V1. A completion claim names its oracle — `adopted`

State what observed the behavior: which verifier, which test, which surface.
"It compiles and the tests pass" is a claim about stubs unless the tests
reach the behavior the user asked for.
**Provenance:** postmortem (tests asserted struct constructors; seven issues
closed falsely). **Detection:** review question; for graded work, the
Harbor verifier is the oracle of record.

### V2. Verify on the surface the user touches — `adopted`

Headless output does not prove an interactive TUI works. Agent shells are
non-TTY and will take the non-interactive branch every time.
**Provenance:** postmortem (every check bypassed ratatui via the headless
fallback). **Detection:** review question. An automated PTY harness is the
planned gate (autoimprove §7.4); until it exists, no interactive-TUI issue
closes on headless evidence.

### V3. Parity claims quantify — `adopted`

Port and parity work reports counts against the source: commands in the
parser, lines per subsystem, features per checklist. Adjectives
("complete", "full parity") are not evidence.
**Provenance:** postmortem (~2,700 lines reported as parity with ~35,400).
**Detection:** review compares the claimed inventory to the source
inventory.

### V4. A crashed grader is not a grade — `adopted`

A run whose verifier never ran is `abandoned`, not scored, and a partial
run of a pinned suite is a smoke run.
**Provenance:** bench. **Detection:** automated — `run-suite.sh` patches to
`abandoned`; the results store refuses `smoke_run` and `unclassified_run`
with exit 3.

## Tool habits

### T1. Batch independent commands into one tool call — `adopted`

Round count drives metered cost: prompt tokens scale with rounds times
transcript size, not with work done. The same task has been done in 6 model
calls and in 15; the 15 cost three times the input tokens.
**Provenance:** fix-git. **Detection:** review reads rounds and prompt
tokens from the ATIF trajectory; regression shows as rising
rounds-per-accepted-outcome on an unchanged suite.

### T2. `--stat` before `-p` — `adopted`

Never dump a full patch into the transcript to find out which file changed.
Survey with `--stat`, then read the one file in question. A patch dumped
early is re-sent every round after.
**Provenance:** fix-git (two `git log -p` dumps re-sent ~12 times).
**Detection:** review flags full-patch dumps followed by further rounds.

### T3. Verbosity guidance is lane-aware — `adopted`

On metered lanes, output tokens are money; on the local lane they are
minutes (17k output tokens ≈ 18.5 wall-clock minutes at local generation
speed). Fewer, larger, quieter rounds on local; terse output everywhere.
**Provenance:** fix-git. **Detection:** review compares output tokens and
wall clock against lane norms in the results store.

### T4. Prove repository topology before acting on it — `proposed`

The winning `fix-git` runs read the recovered commit's content before
resolving the conflict, and the strongest run proved ancestry with
`merge-base --is-ancestor` before choosing a merge. Assuming the
relationship is how a plausible wrong resolution passes local inspection
and fails the verifier.
**Provenance:** fix-git. **Detection:** review question on version-control
tasks. Promote to `adopted` when a cross-section run shows the habit
correlating with acceptance on tasks 1–3.

## Measurement

### M1. One lever per cycle — `adopted`

A comparison is between two rows that differ in one thing. A run that
changes the model and the prompt and a plugin attributes its delta to
nothing.
**Provenance:** bench (the compare view names `model also varies` as a
confounder by design). **Detection:** automated in part — `suiteKey`
excludes the axes a comparison varies, and compare names confounders; the
runbook's cycle enforces the rest.

### M2. Unknown cost stays unknown — `adopted`

`null` with a disposition (`unmetered_local_lane`, `no_accepted_outcomes`,
`cost_unknown`), never zero. A lane that gets cheaper per attempt while
accepting less is a regression, and cost-per-accepted-outcome is the shape
that catches it.
**Provenance:** bench. **Detection:** automated — the store and compare
refuse to launder unpriced into free.

### M3. Regressions stay in the chain — `adopted`

A graded run of a real configuration is recorded even when — especially
when — it embarrasses the trend. The store's hash chain exists to make the
alternative detectable.
**Provenance:** bench (the deliberate `qwen3:0.6b` failure row in
`tb2-quick.jsonl`). **Detection:** automated — `receipt_mismatch` and
`chain_broken` on verify.

### M4. Threshold edits are their own change — `adopted`

Never adjust a floor in the same change as a run it would flatter. Raising
a floor as a lane proves itself out is the intended direction; lowering one
to pass is a contract change and says so in its commit.
**Provenance:** bench thresholds `$comment` blocks. **Detection:** review
of the diff that touches `packages/coder-effectiveness/thresholds/`.

### M5. Quick suites iterate, cross-sections conclude — `adopted`

`tb2-quick`'s two tasks admit success rates of 0, .5, and 1 only. A quick
delta selects what to run next; only a full pinned-suite run is a score,
and the two suites share no suite key so no tool will compare them.
**Provenance:** bench. **Detection:** automated (suite keys) plus the
runbook's cycle.

## Plugins

### P1. A plugin lands with its Gym delta — `adopted`

Before-and-after on the same recipe, plugin present versus absent, delta
attributed through the ATIF `tool.ran` provenance stamps to the exact
digest. A plugin that moves no score is questioned in the review, not
waved through.
**Provenance:** harvest; Harbor plan. **Detection:** review of the landing
change for the A/B rows.

### P2. Plugins are read-only, one-shot, bounded, and honest — `adopted`

The sandbox grants read-only mounts and nothing else; one packet in, one
packet out, under time and memory bounds; typed schemas in the manifest;
output that names its own truncation (`tail_only`,
`dropped_leading_turns`). Edits, writes, and process execution stay in the
coder's core tools under the permission profile.
**Provenance:** harvest; `plugins/README.md`. **Detection:** the host
refuses undeclared imports and stale digests; review checks the manifest
against the shape rules.

### P3. A catalog line is contested space — `adopted`

The capability tool carries each installed plugin's name and first sentence,
capped at twelve. A plugin competes for that slot with everything installed;
its first sentence must earn a model's attention on the turns where the
plugin applies.
**Provenance:** harvest. **Detection:** review question when the A/B shows
a plugin installed but never invoked on tasks it should have served.

## Optimization

Provenance for this section: **dspy-gepa** —
`docs/coder/2026-08-26-dspy-gepa-coder-optimization.md`, which reviews the
DSE history audit, the 2026-06-28 Python-versus-Effect decision, and the
2026-07-04 evolve-the-harness audit.

### O1. An optimizer output is a candidate, never a deployment — `adopted`

An offline optimizer produces candidate artifacts with evidence. Landing
one is a separate, reviewed change carrying its measured delta, exactly
like a hand-written lever. This law predates the coder — it is carried
forward verbatim from the DSE/Blueprint contracts and restated in the
current plugin model assessment and registry strategy.
**Provenance:** dspy-gepa. **Detection:** review of any change that lands
optimizer output without its evidence rows.

### O2. Do not reimplement the optimizer — `adopted`

Consume upstream `gepa`/DSPy at the Python tier where Harbor already
lives. DSE shipped 9,678 lines with deterministic grid search standing in
for MIPROv2 and GEPA, and the hard part was the part it never had.
**Provenance:** dspy-gepa; DSE history audit. **Detection:** review of any
proposal to build a search/teleprompter in Rust, TypeScript, or Effect.

### O3. Screen on the dev set, confirm on a holdout — `adopted`

The optimizer screens candidates on cheap sets (`tb2-quick`,
`owned-closed-issues` when its environments exist) and spends a
`tb2-cross-section` run only on survivors. The holdout is also the
judge-gaming control: a candidate that gamed the dev verifier fails a set
it never trained against.
**Provenance:** dspy-gepa; the evolve-the-harness audit's judge-overfit
caution. **Detection:** candidate evidence must name both sets and state
dev-set coverage; a candidate scored only on the screening set is not
confirmed.

### O4. The cost term lives inside the objective — `adopted`

A token/cost penalty is part of the metric the optimizer maximizes, not a
figure reported beside it. Otherwise the optimizer buys score with spend.
**Provenance:** dspy-gepa (the published loop's
`−0.005×tokens_per_million`); ledger M2 for the accounting half.
**Detection:** review of the objective function in the optimizer lane.

### O5. Candidates carry a transfer label — `adopted`

Every candidate records the model family it was evolved against. Code
mechanisms transfer across families; tuned prompts do not, and have
backfired cross-family. Re-evaluate a prompt-class candidate when the
target family differs.
**Provenance:** dspy-gepa (the published loop measured +14.4 points
same-family versus +0.4 cross-family). **Detection:** candidate artifact
schema requires the field; review rejects an unlabeled prompt candidate.

### O6. Acceptance states its trial count and floor — `adopted`

A promotion gate is "beats the incumbent by ≥N over K trials," with both
numbers written down, set just above the measured noise floor. On
`tb2-quick` the available rates are 0, .5, and 1, so the floor is
structural, not statistical — say so rather than implying precision the
suite cannot carry (ledger M5).
**Provenance:** dspy-gepa. **Detection:** candidate evidence without a
stated trial count is not a promotion argument.

## Repository

### R1. Pack with `pnpm pack`, never `npm pack` — `adopted`

The manifest carries pnpm `catalog:` versions that only `pnpm pack`
rewrites into versions npm can install. An `npm pack` tarball looks healthy
and fails for every consumer.
**Provenance:** bench; `CLAUDE.md` deploy section (shipped broken once as
`@openagentsinc/cli@0.2.0`). **Detection:** the CLI's `verify:package`
refuses a manifest carrying `catalog:`.

### R2. Push the forge, not GitHub — `adopted`

The `openagents` remote records the push in the WAL; GitHub is a mirror
production force-pushes. A direct GitHub push is overwritten, not merged.
**Provenance:** openagents.com `AGENTS.md`. **Detection:** automated —
`push-remote-check.sh` refuses a non-forge push where installed.

### R3. Fresh worktree per unit of work — `adopted`

Implement off current `origin/main` in a new worktree; never edit through
another agent's dirty checkout, and never move its uncommitted work aside.
**Provenance:** repository contract (owner mandate 2026-07-20).
**Detection:** review of the working-tree state in the transcript.
