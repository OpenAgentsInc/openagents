# CoderBench roadmap: epics and issues

Date: 2026-08-27. Status: living roadmap. Companions:
[`plan.md`](plan.md) (what CoderBench is and why),
[`terminal-bench-lessons.md`](terminal-bench-lessons.md),
[`process.md`](process.md) (the procedure the commands automate),
[`gym-cli-spec.md`](gym-cli-spec.md) (the command contract).

This document turns the Gym CLI spec and the CoderBench plan into an ordered
build sequence: **epics** (large bodies of work, one per theme, tracked as
tracker epics) and the **first wave of issues** under each. Each issue names
its acceptance gate in the house style — evidence, not narration. The build
order here mirrors `gym-cli-spec.md` §6; when the two disagree, this file is
updated to match reality, and the tracker is the source of truth for what is
open.

## The three laws that shape the order

1. **Rust-first.** Real logic lives in the CLI crate. Python is confined to
   Harbor's own code inside a digest-pinned container. Nothing new is
   host-pip-installed.
2. **Every step leaves a usable command.** No flag-gated half-features; each
   issue's merge makes one more real thing possible, preferably on the
   Terminal-Bench path, which needs no corpus and pays for itself from day
   one.
3. **Compare or it didn't happen.** From the first recorded run onward, tb2
   and CoderBench numbers are read side by side (`gym results compare`), so
   our benchmark is validated against the established one, never instead of
   it.

## Epics

### Epic A — `openagents gym` core: suites, runs, results

The Rust control plane over the existing Terminal-Bench lane. Ends when a
person who has never read `bench/README.md` can run `openagents gym doctor`,
then `openagents gym run tb2-quick --model ...`, and read a scored, recorded,
compared result — with every refusal honest.

- A1 `gym suite list/show` — read `bench/suites/*.suite.json`, emit
  `openagents.gym.suite_manifest_view.v1`
- A2 `gym suite check` — manifest/pin drift as exit-1 diff; `gym run` refuses
  drifted suites
- A3 `gym run` — prereq probe, register (`runs/start`), execute via the
  runner, live trial states, finalize (`graded`/`abandoned` rule enforced)
- A4 `gym run status/list/cancel` — server-side reads through the Gym API;
  local job-dir reads when unregistered
- A5 `gym results score` — effectiveness computation ported to Rust; store
  refusals verbatim; `--append` gated on score tier
- A6 `gym results show/compare/trend` — hash-chain verify before render;
  cross-suite compare (tb2 ↔ coderbench) as the headline output
- A7 golden surface test — pin the `gym` command surface; stub-server wire
  tests for every network-touching command

### Epic B — Harbor by container: `gym env`

Compute plumbing. Ends when `gym env doctor` exit 0 is a sufficient
precondition for a scored run, and the host machine needs no Python for any
Gym operation.

- B1 `gym env probe` — compose `computer probe` + Docker/amd64 canary
  (mechanical Rosetta/qemu detection) + disk headroom
- B2 `gym env doctor` — probe + exact per-failure remedies; exit 0 means a
  scored run can happen now
- B3 `harbor-runner` image — Harbor + adapters + `bench/adapters/openagents_coder.py`
  pinned by digest; new Python work lands here, never on hosts
- B4 `gym env pull` + `gym run` on the container target; host-native Harbor
  demoted to documented fallback
- B5 `gym env box create/release` — Box VM execution lanes via the existing
  box client, `gym:<run-id>` labels, `env list` accounting

### Epic C — trace corpus: import at scale

CoderBench's feeding line (`process.md` Stages A–C as commands). Ends when
`gym corpus import` has moved ≥100 qualified, redacted, digest-pinned traces
into the cloud corpus with a committed ledger and zero post-redaction
tripwire incidents.

- C1 corpus inventory engine in Rust — walk the three stores through
  `trace::discover`, convert via packaged converters, content digests,
  deterministic (two runs diff clean apart from mtimes — tested property)
- C2 `gym corpus inventory/qualify` — apply the `plan.md` §7 filters, emit
  the exclusion report; nothing silent
- C3 `gym corpus import` — batch convert→redact→tripwire→upload at `ledger`;
  `dark` refused, `glass` never a batch flag, ≤100 batches, rate-limit pacing,
  idempotent by digest; `corpus.jsonl` ledger rows
- C4 `gym corpus status/verify` — ledger ↔ cloud consistency (digest
  re-check), drift reported by name

### Epic D — datasets and task distillation

From flat corpus to pinned suite (`process.md` Stages D–E as commands).
Ends when `gym dataset pin` has produced the first CoderBench suite manifest
that `gym run` accepts and `gym results` records.

- D1 label schema (`labels.schema.json`) + first ten hand-labeled outcomes
  against the forge (manual work, owner-reviewed; the CLI only stores)
- D2 `gym dataset create/add/remove/show` — validated, append-recorded
  membership; refuse refs that do not resolve
- D3 `gym dataset pin/diff` — compile a dataset into
  `openagents.effectiveness_suite.v1` with content digests; drift between
  suite and dataset visible
- D4 distiller v0 — labeled session → draft task (instruction rebuilt,
  start-commit pin, oracle command, provenance digests); drafts are
  candidates, human review promotes into `bench/tasks/coderbench/`
- D5 first suite recorded — `coderbench-agent-building-v1.suite.json`, smoke
  tier, real Harbor run, store-accepted row

### Epic E — views: TUI pane and web rendering

Read-only surfaces over the same schemas. Ends when a run's progress and a
suite's trend are each visible in three places — terminal, TUI, web — from
one document each.

- E1 `openagents.gym.*` schema freeze v1 — the documents above, reviewed as
  the rendering contract; version-bump policy written down
- E2 coder TUI gym pane — ratatui pane over `run_status` + `results trend`
  documents; no parsing beyond the schema
- E3 web corpus/dataset views — Phoenix routes serve the same documents the
  CLI emits (openagents.com repo); `/gym` gains corpus and dataset tabs
- E4 comparison report rendering — `gym results compare` output as a shareable
  artifact (terminal text now, TUI + web from E2/E3's renderers)

### Epic F — second domain: generality proven

The M6 gate. Ends when a second repository's qualified traces have become a
recorded suite, proving the method is general in fact, not prose.

- F1 second-domain selection — from the inventory's domain tags, by labeled
  outcome count and oracle determinism; decision recorded with the funnel
  numbers
- F2 second-domain environment family — one parameterized image for its
  toolchain shape; determinism burn-in (three identical verdicts)
- F3 second suite recorded — `coderbench-<domain>-v1`, smoke → score by the
  same ladder as the first domain

## Sequencing

```
A1-A7 ──► B1-B4 ──► (B5)          Epic A then B: the tb2 path, first-class
   ╲
    ╲──► C1-C4 ──► D1-D5 ──► F1-F3   Epic C then D: the CoderBench line
              ╲
               ╲──► E1-E4            Epic E once A6 produces documents to render
```

- Epic A first: it is useful today, needs no corpus, and every later epic
  tests against it.
- Epic B follows immediately; B5 (Box lanes) may interleave with C when the
  local container path is boring.
- Epic C can start once A3 exists (runs register through the same client
  code); it is independent of B.
- Epic D needs C's corpus; E1 needs A's schemas; F needs everything.

## Tracker mapping

Filed 2026-08-27, first wave — epics as issues, first-wave children beneath
them, `deps` edges recording this order:

| Epic | Issue | First-wave children |
| --- | --- | --- |
| A — gym core | #161 | #167 suite list/show/check · #168 run · #169 results score/compare |
| B — gym env | #162 | #173 probe/doctor · #174 harbor-runner image + pull |
| C — corpus | #163 | #170 inventory/qualify · #171 import/status/verify |
| D — datasets | #164 | #172 dataset commands · #175 labels (manual) |
| E — views | #165 | #176 schema freeze |
| F — second domain | #166 | #177 domain selection |

Later-wave work (run status/cancel, Box lanes, distiller, TUI pane, web
views, second-domain build-out) files as its children close. When an issue
closes, this file's next revision folds the outcome in; the tracker remains
authoritative for state.

## Running the waves on subagents

[`delegation.md`](delegation.md) is the operating instruction for executing
these waves with maximum parallelism: waves computed from the dependency
graph, one fresh worktree per issue per worker, per-worker gates
(`pnpm run check` + crate tests), evidence-on-close, heartbeat/reap rules
for stragglers, and the orchestrator's role (rosters and merges, never
code). Wave 0 — #167, #170, #173, #176, plus manual #175 — runs five wide
today.
