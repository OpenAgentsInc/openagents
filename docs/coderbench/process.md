# CoderBench process: from local traces to a graded lane

Date: 2026-08-27. The operating procedure for the plan in
[`plan.md`](plan.md). Follows the house rules that already govern this
repository's agent work (`docs/coder/runbook.md` §0): fresh worktree per
unit, land via the forge remote, never record what was not measured, a
candidate is never a deployment.

## Stage A — Inventory (M0)

**Goal:** one reproducible catalog of every candidate session on this machine.

```sh
# Discover per store (native tooling; no new parsers)
openagents trace list --limit 10000        # ~/.openagents/exports + defaults
openagents trace show <file>               # per-file summary, format detection
```

Walk the three stores — `~/.openagents/exports` (native ATIF, already safe),
`~/.codex/sessions` (~2,548 rollouts), `~/.claude/projects` (~1,327 sessions) —
and emit `docs/coderbench/inventory.json`. One row per session:

```
{ source, path, digest, bytes, steps_est, started_at, ended_at,
  model, repo_hint, qualifies: bool, excluded_because?: [codes] }
```

Rules:

- Conversion of Codex/Claude JSONL to ATIF happens through the packaged
  converters, not bespoke regex work. If a rollout cannot convert cleanly it is
  recorded as an exclusion with the failure reason; forcing it corrupts the
  corpus.
- Digests are content digests over the *converted* ATIF (pre-redaction), so
  dedup survives path renames.
- Exclusion codes come from `plan.md` §6; every drop is counted and visible.
  No silent filters.

Two runs of the inventory command must diff clean (modulo mtimes) before M0
closes. Commit the inventory, not the traces themselves at this stage.

## Stage B — Qualify and redact

For each qualifying row:

1. **Convert** to ATIF v1.7 if native export isn't already available.
2. **Redact**: `openagents trace redact <trace>` writes `.redacted.json`
   beside the original via the ATIF redactor engine (home paths, emails,
   keys, wallet material, IPs, usernames, long blobs).
3. **Tripwire**: run the public-safety tripwire locally. Any surviving leaky
   pattern fails the trace out of the batch — never hand-patch JSON to squeeze
   one past. The failure count is published in the batch report.
4. **Dry-run review**: first upload of any new source kind gets its output
   eyeballed end to end by a human before the batch proceeds. Sample 5% of
   every later batch minimum.

Batch discipline: batches of ≤100, one batch report per run appended to
`docs/coderbench/batches/` listing digests in/out, exclusion counts, tripwire
catches (target: zero post-redaction).

## Stage C — Upload (M1)

```sh
export OPENAGENTS_AGENT_TOKEN=oa_agent_...   # env only, never CLI history
openagents trace upload <redacted.json> \
  --visibility ledger                        # content + metadata
```

- Default rung for the corpus is **`ledger`**; `pulse` for the rare trace too
  sensitive for full content; **`glass` only ever per-trace, decided by a
  human**, never in a loop; `dark` never (unpublishable traces have no corpus
  value — if it can't ship, it doesn't qualify).
- Uploads are idempotent by digest; re-running a failed batch costs nothing
  but time.
- Respect server limits: body cap 10 MiB per document (split larger rollouts
  at user-turn boundaries using continuation references; record split pairs in
  the corpus ledger), rate limit ~120/hour/owner (schedule accordingly or use
  multiple days; never hammer).
- Append each batch's digest → returned uuid mapping to
  `docs/coderbench/corpus.jsonl` and commit. This file is the corpus's index
  of record; the cloud stores content, the ledger stores meaning.

## Stage D — Label outcomes (M2)

Against the forge, for each uploaded trace:

```sh
openagents issue list --state closed --limit 200
git log --oneline <window>                  # match landing commits to sessions
```

Produce one label per session:

```
{ trace_uuid, repos_touched, issues_closed, commits_landed,
  outcome_type: feature|fix|refactor|port|test|infra|rejection,
  oracle: { kind: gate|pty-suite|forge-closing-ref, command },
  gradeable: bool, holdout_ok: bool, notes }
```

Labeling is manual and reviewed by the owner — no agent self-labels at this
stage (the thing being benchmarked must not grade its own history). Gradeable
requires: the start commit builds clean today, the oracle command exists and
is deterministic, and no credential the corpus can't provide is needed.

Write the ten+ labels plus `labels.schema.json`; expect the schema to change
twice here — cheap now, expensive after 1,000 rows.

## Stage E — Distill tasks (M3–M4)

Per gradeable label, generate a **draft task** (never auto-landed):

- `instruction.md` — rebuilt from issue text + original user intent. Never a
  verbatim copy of turns that quote the solution. One behavioral sentence,
  constraints, definition of done.
- `pin.json` — start commit, suite metadata, provenance trace uuids +
  digests, environment family tag.
- `oracle.sh` — the real gate. Exit code decides. No LLM judge anywhere in
  the grading path.
- `README.md` — derivation receipt: which session(s), which issue, why
  gradeable, what was held out.

Human review gates promotion into `bench/tasks/coderbench/<task-id>/`.
Reviewer checklist: leaks? instruction self-contained? oracle deterministic?
does any *reasonable* solution pass, not just the historical one?

Promote survivors into `bench/suites/coderbench-v1.suite.json`
(`environmentAvailable: true` only where verified), regenerate manifests with
`pnpm run effectiveness:suites -- --check`, keep tier `smoke` until Stage F.

## Stage F — Grade (M4–M5)

Same rails as tb2:

```sh
bench/run-suite.sh bench/suites/coderbench-v1.suite.json \
  --model openai/gpt-5.6-luna --lane proxy --n-concurrent 2
```

- amd64 verifier rule applies (Rosetta/cloud/real amd64) exactly as documented
  in `bench/README.md`; ungraded trials are not scores.
- Determinism burn-in: every task passes three consecutive oracle runs on the
  pinned state before score-tier promotion. Non-deterministic tasks are pulled,
  not tolerated.
- Baselines: Flash configuration first (#143 discipline — native binary,
  real model, honest row), then one degraded-config deliberately-bad row for
  the floor to catch, mirroring tb2-quick's regression proof.
- Results append to `bench-results/coderbench-v1.jsonl` through the store
  tooling only. Smoke rows stay marked smoke; nothing graduates without the
  burn-in record.

From here: the autoimprovement cycle treats CoderBench like any other lane —
change one lever, same suite, read delta, keep or refute, write the ledger
row — always paired with a tb2 number so home-soil gains stay honest.

## Stage G — Keep it alive

- Every future qualified session appends to the inventory automatically at
  week boundaries; new labels enter review queue; suites grow only by reviewed
  promotion.
- Quarterly: re-dedup the corpus (content drift check), re-run determinism
  spot checks on 20% of tasks, re-read `terminal-bench-lessons.md` against
  whatever Harbor shipped since.
- Retire tasks when they stop discriminating (all configurations pass
  consistently) rather than inflate the pool.

## Failure modes this process explicitly forbids

1. Uploading anything unreviewed past redaction "just once."
2. An agent labeling its own session's outcome gradeable.
3. A task whose oracle reads its own derivation README (answer adjacent).
4. Recording a smoke run as score "to see the trend early."
5. Bypassing the visibility ladder default in a batch flag.
6. Letting the corpus ledger and the results store disagree about digests.
