# Microluna overnight: the determinism thesis on TB4

2026-09-24, running. Issue [#9585](https://github.com/OpenAgentsInc/openagents/issues/9585).
This is the overnight run of the [determinism thesis](../coder/design/thesis.md)
with Microluna on GPT-6 Luna and Jev only — no Claude, no Codex CLI, no
Astra or Sol. It iterates the [Microluna](../coder/design/microluna.md)
mini-handoff loop and measures each change against the previous one, on TB4
tasks we've already run, with early stopping.

## Targets

From the Fable 5.1 corpus (`bench/terminal-bench/reference/fable-5.1-replays.json`)
and our local jobs:

- **A — Fable fails, an earlier Coder One config passed.** Winning here beats
  Fable where it loses: `session-window-debug` (Fable 0/25),
  `bun-sourcemap-leak` (0/25), `data-anonymization` (0/25),
  `layout-config-recreation` (2/25), `vba-userform-port` (2/25),
  `html-js-filter` (5/25), `vf2-speedup-networkx` (7/25),
  `biped-contact-dynamics` (8/25), `atrx-vep-crispr` (9/25),
  `ks-solver-cpp` (9/24), `intrastat-meldung` (10/25).
- **B — Fable passes, we aim to win far cheaper** (Fable's mean $ per pass):
  `embedding-drift-monitor` 25/25 [$3.82], `sound-change-cascade` 25/25
  [$7.38], `coq-block-bound` 25/25 [$6.67], `shadow-relay` 24/25 [$3.48],
  `fin-saccr-rwa` 22/25 [$4.06], `interleaved-vigenere` 23/25 [$6.65],
  `gsea-proteomics` 19/25 [$2.25].

## The loop, and what each version changed

Every version is Microluna on Luna, the mini-handoff loop: Jev's requirement
map in up to four groups, short sessions per group with the context rebuilt
each time, `verify.checks` and a Jev move (`next`, `retry`, `stuck`, `done`)
between sessions, all bounded. The manifests are in
`crates/coder-one/policies/`.

| Version | Change from the previous | Manifest |
| --- | --- | --- |
| `microluna-v1` | The loop with evidence required for a done (an edit that a later command tested, or a command-made workspace change), a fail verdict blocking a loop end, and a broken stream resent. | `microluna-v1.json` |
| `microluna-v2` | `read_first`: a read-only reconnaissance session on each group (reproduce, run the task's tests) before any edit, not counting against the group's attempts. Fable's read-longer-before-the-first-edit move (#9586). | `microluna-v2.json` |
| `microluna-v3` | `accept`: a loop-ending move waits until `verify.checks` positively confirm the focus. The thesis's "done is a program state." A stand-in until #9588's `accept.define` lands. | `microluna-v3.json` |

## Results

### Mini-task validation of the loop moves

Before TB4, the four Coder One mini-tasks, one attempt each, live Jev, on
the Codex login. These are cheap and fast, and they isolate the loop's
behavior.

| Task | v1 loop (no read-first) | v2 loop (`read_first`) |
| --- | --- | --- |
| `git-recovery` | passed, 1 session | passed, 2 sessions (1 read) |
| `cancel-cleanup` | passed, 3-4 sessions | passed, 6 sessions (3 reads) |
| `interactive-terminal` | passed, 3 sessions | passed, 6 sessions (3 reads) |
| `log-severity` | **failed** (CRLF), 3 sessions | **passed**, 6 sessions (3 reads) |

`read_first` turned `log-severity` from a fail to a pass: reproducing and
reading before editing led Luna to write LF line endings the grader accepts,
where every non-read-first run wrote CRLF. `read_first` costs more sessions
(about $0.006 against $0.004 of Luna), and every task still passed. This is
the first end-to-end evidence that reading before editing helps Luna, not
just Fable.

### TB4

The matched experiment `microluna-overnight-9585` runs `microluna-v1`
against `microluna-v3` on `gsea-proteomics`, `embedding-drift-monitor`, and
`ks-solver-cpp`, two attempts each, interleaved, early stopping on, on the
Codex login with no Claude. Artifact `coder-one 0.1.0 (168ebb5339)`. Read it
with `gym experiment pulse` or
`~/.openagents/terminal-bench/experiments/microluna-overnight-9585/status.json`.

Results pending. The three earlier single trials on the Codex-only
`microluna-v1` arm, from the [first comparison](2026-09-24-microluna.md),
all scored 0, as did Luna-in-Codex and Luna direct (0 of 20 on TB4 so far in
[#9583](https://github.com/OpenAgentsInc/openagents/issues/9583)):

| Task | Set | Microluna v1 | Luna-in-Codex (#9583) |
| --- | --- | --- | --- |
| `coq-block-bound` | B | 0 · 347 s · $0.0086 · 6 sessions | 0 |
| `shadow-relay` | B | 0 · 826 s · $0.0346 · 6 sessions | 0 |
| `uefi-bootkit` | (not in A/B) | 0 · 1340 s · $0.0536 · 6 sessions | 0 |

Microluna's cost per attempt on these is $0.01 to $0.05 of Luna, against
Fable's $3 to $7 per pass. The gap to close is passes, not cost: the loop
runs cheaply and stops at its bounds, but Luna doesn't yet solve these
tasks. The next iterations measure whether `read_first` and the acceptance
gate move any B-set task to a pass.

## Spend

Luna list-price estimate so far: about $0.11 across the first comparison and
the mini-task validation. The overnight experiment caps Luna at the loop's
per-dispatch bounds; the running total goes here as trials finish, under the
$40 overnight budget.

## What's next

- Read the matched experiment in flight, kill the losing arm early.
- Integrate #9588's `accept.define` when it lands, in place of the minimal
  `accept` gate.
- Add A-set tasks with runnable checks (`html-js-filter` has behavior
  checks Coder One already covers) once the B-set arms show signal.
