# Full Auto autonomy — next-action decision quality rubric

- Date: 2026-07-22
- Status: active analysis method (metric `full-auto-decision-v1`)
- Audience: agents, product reviewers, and test authors
- Related methods: [complexity rubric](./complexity-rubric.md),
  [coherence rubric](./conversation-thread-coherence-rubric.md),
  [deterministic coherence screening](./deterministic-coherence-screening.md)
- Related code: `apps/openagents-desktop/src/full-auto-*.ts`
- Result authority: analysis only. This method cannot admit a release or a
  public claim.

## Purpose

The owner stated one core concern: "I need less expectation that the user will
guide it." This document answers two questions for OpenAgents Full Auto.

1. Left alone, can Full Auto build a task list that the owner would choose?
2. Left alone, does Full Auto keep making good next-action decisions?

This document does three things. It records how Full Auto decides its next
action today, from the code. It gives a scoring rubric for autonomous
next-action quality. It scores the current system against that rubric with an
honest split between design evidence and observation.

Two sibling audits run at the same time. One audits Electron auto-update. One
audits the RLM-to-Full-Auto history recall path. This document does not repeat
that work. It focuses on decision quality and the rubric.

## Part 1 — How Full Auto decides its next action today

### The mission packet is the per-turn authority

Each Full Auto turn runs from a private mission packet
(`full-auto-mission.ts:46-66`). The packet carries the owner objective and the
done condition verbatim. It also carries the current lane, the continuation
ordinal, the turn cap, the remaining turns, the last accepted outcome, and the
last provider handoff. The prompt text tells the provider to preserve the owner
objective and to "do one concrete, useful next step now"
(`full-auto-mission.ts:130-144`).

The objective and the done condition come from the durable `FullAutoRun`
record. The owner sets them at run start (`full-auto-run-registry.ts:214-215`,
`full-auto-mission.ts:88-89`). When no run exists, the packet uses a legacy
migration objective, and the continuation loop uses a generic message:
"Continue Full Auto: look at this repository (README, docs, open issues) and do
the next concrete useful thing" (`full-auto-reconcile.ts:40`).

Key fact: Full Auto does not select the objective. The owner selects it. The
provider then chooses the actual next action inside its own turn. The host does
not plan, decompose, or sequence the work.

### The continuation loop decides "start another turn", not "what to do"

One decision function decides whether the next turn starts
(`full-auto-reconcile.ts`, `reconcileFullAutoThreads`). Two call sites share it:
right after a Full Auto turn completes, and once at startup
(`main.ts:5312-5352`). The loop:

- claims a durable per-thread lease before dispatch, so two passes cannot both
  dispatch the same thread (FA-H3).
- dispatches only into the exact workspace granted at enable time, and fails
  closed on a mismatch (FA-H2).
- replays the bound execution profile, so continuations keep the same account,
  model, and effort (FA-H6).
- caps continuations at 20 turns (`full-auto-reconcile.ts:41`).

The loop decides pace, workspace, lane eligibility, and failure handling. It
does not decide the content of the next action. The provider decides that.

### The route decision is deterministic. Apple FM only advises

The lane decision is a pure function of the owner-ordered policy and the shared
lane gate (`full-auto-advisory.ts:84-92`). Apple FM produces a route
recommendation and a bounded advisory analysis. Both carry an `advisory: true`
literal and no action field (`full-auto-advisory.ts:8-15, 42-72`). The
recommendation cannot change what runs. The advisory analysis flips no typed
state.

This is a strength for safety. It is not a planner. Apple FM cannot choose the
work, and no other module chooses the work either.

### Guardrails, the turn cap, and the no-progress gate

Three guardrails are non-overridable in code, with no configuration field, no
environment variable, and no conversation setting to relax them
(`full-auto-reconcile.ts:56-96`): `workspace_binding`, `own_capacity_only`, and
`no_rate_limit_reset_triggering`.

Failure handling is strong. A typed failure classifier maps provider errors to
rotation classes (`full-auto-reconcile.ts`, `classifyFullAutoDispatchFailure`).
A rate-limited or exhausted lane either rotates to a different owner-admitted
lane or consumes failure budget and waits out a bounded exponential backoff. The
record disables after 5 consecutive failures
(`full-auto-reconcile.ts:42`).

The no-progress gate is weaker than it looks. It counts only a trailing run of
turns whose disposition is `failed` or `interrupted_by_restart`
(`full-auto-reconcile.ts:97-134`). It pauses after 3 such turns. A turn that
self-reports `completed` counts as progress, even when the work has low value.
The gate catches machine failure. It does not catch low-value churn.

### Completion authority is a claim, not an executed check

The packet states the completion authority in one literal at
`full-auto-mission.ts:62-65`. The literal below is source data. Do not change it.

```text
provider completion is self-reported evidence only; the host or owner verifies the done condition
```

The run lifecycle reaches `completed` as a self-reported transition
(`full-auto-run-registry.ts:102`).

The host does not execute the done condition. No module reads the done-condition
text and runs a check against it. The closest signal is a post-hoc analyzer
finding, `unverified_completion_risk`, which flags a run that reached
`completed` with zero verified or claimed refs
(`full-auto-run-analyzer.ts:48, 99-113`). That is a conservative structural
proxy. It is not a correctness verdict. So "the host or owner verifies" is
mostly aspirational for a general objective today. The host raises a flag. It
does not verify.

### Cross-turn memory is thin, with one emerging exception

The mission packet gives the provider very little accumulated state. It carries
the last accepted outcome as a turn reference, a lane, and a timestamp only
(`full-auto-mission.ts:59, 97-105`). It does not carry a summary of prior turns'
actual work. The prompt tells the provider to re-derive state from the
repository each turn. Two successive turns therefore share an anchor (the
verbatim objective) but not a memory of what already happened.

The one emerging exception is the RLM recall consumer (RLM-06,
`full-auto-recall.ts`). A long run may consult its own authorized event history
to frame the next continuation. That path is run-scope isolated, bounded, cited,
and advisory. It never transfers verification authority. The sibling RLM audit
owns that path. This document notes it only as the current building block for
cross-turn memory.

### Real versus aspirational — a summary

| Capability | State | Evidence |
| --- | --- | --- |
| Keep a provider on a fixed owner objective across turns | Real | Mission packet + continuation loop |
| Exactly-once dispatch, workspace binding, profile continuity | Real | FA-H2, FA-H3, FA-H6 |
| Own-capacity-only lanes, no rate-limit gaming, failure backoff, lane rotation | Real | FA-GD-01, FA-RT-01 |
| Turn cap and disable-after-failures | Real | Reconcile constants |
| Deterministic route decision, advisory-only Apple FM | Real | `full-auto-advisory.ts` |
| Pause on repeated machine failure | Real, but narrow | No-progress gate |
| Autonomous objective selection | Absent | Owner sets the objective |
| Persistent decomposed plan or roadmap across turns | Absent | No planner module |
| Host-executed done-condition verification | Aspirational | Prompt claim + post-hoc flag only |
| Value-aware no-progress detection | Absent | Gate counts machine failure only |
| Owner-priority model for what work to choose | Absent | Generic "next useful thing" |

The honest summary: Full Auto is a robust continuation engine with strong
operational hygiene. It is not a planner and not a verifier. It keeps a provider
working on an objective that the owner supplies. It does not choose the
objective, sequence a roadmap, or check the result.

## Part 2 — The rubric for autonomous next-action quality

Score each dimension from 0 through 4. A higher score means the system needs
less user guidance. Report the seven dimension scores together, not one blended
number. A low score on Selectivity or Self-verification is a hard limit on
unattended trust, even when other scores are high.

Reuse the complexity ladder C0 through C4 from the
[complexity rubric](./complexity-rubric.md) for the Complexity dimension. Reuse
the `coherence-screen-v2` framing for the Coherence dimension.

### D1. Complexity — can it take on genuinely hard, multi-step work

Measure the complexity tier (C0 through C4) that the autonomous system can
sustain without user guidance. Measure the host contribution to that
complexity, not only the provider contribution inside one turn.

| Score | Descriptor |
| --- | --- |
| 0 | Only trivial single edits. Any multi-step work needs a user prompt. C0. |
| 1 | Short multi-step work inside one turn. No cross-turn structure. C1. |
| 2 | Sustained multi-step tool work across turns on one owner objective. The host adds no decomposition. Provider carries all structure. C2. |
| 3 | Cross-turn work with host-tracked sub-tasks or unblock steps. Sub-agents or multiple models appear. C3. |
| 4 | Host-orchestrated heavy work across many turns: sub-agents, handoffs, and long horizon, all sequenced by the host. C4. |

Observe: the complexity score of the run transcript, and whether the host
mission packet carried sub-task state or only a static objective.

### D2. Coherence — do successive actions form a consistent line of work

Measure whether autonomous actions cohere into one line of work or drift and
thrash. Use the `coherence-screen-v2` signals and the coherence rubric
dimensions D1 (intent fidelity) and D2 (causal continuity).

| Score | Descriptor |
| --- | --- |
| 0 | Actions thrash. Each turn restarts or contradicts the last. |
| 1 | Frequent drift. The line of work is hard to follow. |
| 2 | The objective anchor holds, but turns repeat setup or reconnaissance because there is no shared memory of prior work. |
| 3 | Turns build on prior turns through cited recall or carried state. Small drift only. |
| 4 | Every turn advances one coherent line of work with clear causal links to prior turns. |

Observe: `objective_drift_revision` and `repeated_disposition_pattern` findings
from the run analyzer, plus a `coherence @ complexity` grade of the run
transcript.

### D3. Foresight — do actions set up future progress

Measure whether the next action sequences well: it unblocks later work, avoids
dead ends, and orders dependencies. A myopic local step scores low.

| Score | Descriptor |
| --- | --- |
| 0 | Pure local greed. No sense of order or dependency. |
| 1 | Occasional lucky ordering. No explicit sequencing. |
| 2 | The provider sequences within a turn. The host prompt still says "do one next step now" with no plan. |
| 3 | The host tracks a dependency order or a plan and picks the next unblocking step. |
| 4 | The host maintains a live plan, reorders on new evidence, and avoids known dead ends. |

Observe: whether a persisted plan or task order exists, and whether the next
action matches an unblock step rather than a repeat.

### D4. Groundedness — decisions grounded in repository and evidence reality

Measure whether decisions rest on real repository, issue, and evidence state,
not on invented facts.

| Score | Descriptor |
| --- | --- |
| 0 | Decisions rest on invented state. No grounding step. |
| 1 | Weak grounding. The system often acts on assumptions. |
| 2 | The provider reads the repository each turn, but the host does not verify the grounding. |
| 3 | The host supplies grounded context (issue state, prior outcomes) and checks that the action cites real refs. |
| 4 | Every decision cites verified repository or evidence refs, and the host rejects an ungrounded action. |

Observe: whether the mission carried grounded context, and whether the action
produced real refs (`missing_evidence` finding absent).

### D5. Selectivity — picks work the owner would actually choose

Measure value and priority alignment. The autonomous system must pick the work
the owner would pick, not just any valid work. This is the dimension that most
directly answers the owner concern.

| Score | Descriptor |
| --- | --- |
| 0 | The system cannot pick work. The owner must supply the objective every time. |
| 1 | The system picks generic "useful" work with no owner-priority model. |
| 2 | The system picks from a repository backlog, but without owner-priority weighting. |
| 3 | The system ranks candidate work against a model of owner priority learned from real history. |
| 4 | The system selects work the owner endorses on review most of the time, and explains the ranking. |

Observe: whether the objective is owner-supplied or system-selected, and the
owner endorsement rate of system-selected objectives on review.

### D6. Self-verification — checks its own done condition honestly

Measure whether the system checks the done condition it was given, and reports
an honest verdict rather than a self-declared success.

| Score | Descriptor |
| --- | --- |
| 0 | The system declares success with no check. |
| 1 | The system self-reports completion. A post-hoc flag can note zero evidence. |
| 2 | The system attaches evidence refs but does not run the done condition. |
| 3 | The host executes a check against the done condition and records a typed verdict. |
| 4 | The host runs an independent check, blocks a false completion, and separates "provider done" from "host verified". |

Observe: whether the run reached `completed` with an executed check, and whether
`unverified_completion_risk` fired.

### D7. Recoverability — handles blockers and stalls without user rescue

Measure whether the system recovers from failures, rate limits, stalls, and
crashes without a user rescue.

| Score | Descriptor |
| --- | --- |
| 0 | Any blocker stops the system until a user acts. |
| 1 | The system retries blindly and can loop on the same failure. |
| 2 | The system backs off on failure but cannot route around a bad lane. |
| 3 | The system classifies failures, backs off, rotates lanes, and pauses on repeated machine failure. |
| 4 | The system also detects low-value churn and stall, and recovers or pauses with a typed reason before waste accumulates. |

Observe: the failure classifier, the backoff and rotation behavior, the
disable-after-failures constant, and the no-progress gate breadth.

## Part 3 — What the owner actually asks for

This section characterizes the owner's real request pattern. It grounds the
Selectivity dimension. It is public-safe. It reports aggregate patterns only. It
does not quote raw private prompts, secrets, or tokens.

### Evidence sources

- Local Codex session rollouts under `~/.codex/sessions`, read only. A sample of
  200 recent first-directive messages across about 250 rollout files.
- The `origin/main` commit history, last 200 commits.
- The transcript theme guide `docs/transcripts/README.md`.

### What the owner directives look like

The sampled directives are short and bounded. The median directive length is
about 75 characters. Most directives share one shape:

1. A grounding instruction. The owner names what to read first. "read" appeared
   in about 110 of 200 sampled directives.
2. A concrete deliverable. Common verbs were write, create, make, fix, build,
   audit, review, and design.
3. A verification obligation. "verify" appeared in about 41 of 200 directives.
4. A completion gate. "push" to `main` appeared in about 41 of 200 directives,
   and "push to main" as a phrase in about 38.

The dominant topics in the sampled directives were desktop work (about 71 of
200), a specific issue reference (about 94 of 200), STE discipline (about 26),
and provider, assurance, and ProductSpec work (about 7 each).

The commit history confirms the same shape. The last 200 commits concentrate on
`desktop` (53), `sandbox` (36), `assure-repo` and `assurance` (24 together),
`analysis` (6), and `sol`, `memory`, and `fable` (about 5 each). The commit types
are mostly `feat`, `fix`, `test`, `docs`, and `chore`, in that rough order.

The transcript arc adds the north-star intent. The series states that after
version 1.0 "every release ... is meant to ship by the network rather than by
human hand", and it names "closing the say/ship gap" as the win condition
(`docs/transcripts/README.md`, episodes 237 and 246).

### What "a task list the owner would choose" looks like

From this evidence, an owner-chosen task list has these traits:

- Each item names an explicit grounding target: a doc, an issue, or a code path.
- Each item has a concrete, bounded deliverable, not an open goal.
- Each item carries a named verification command or acceptance check.
- Each item ends at a real completion gate: merged and pushed to `main`, with
  green checks.
- The work concentrates on the active product surface, which is currently
  OpenAgents Desktop, the sandbox and provider path, assurance, and the analysis
  and roadmap docs.
- The work respects standing discipline: STE for docs, fresh worktree per task,
  and no invented claims.

Full Auto today receives such an item as its objective. It does not generate
such a list on its own. The owner still supplies the selection and the priority.

## Part 4 — Current-state scores

These scores reflect the current `origin/main` code. Each score states its
evidence basis. Most scores rest on design reading. Only Recoverability rests on
a broad test surface as well. No score rests on a large live unattended sample.
Treat the scores as a design-grounded estimate, not a measured benchmark.

| Dimension | Score | Basis |
| --- | --- | --- |
| D1 Complexity | 2 | Design. Provider can do C3 or C4 work inside a turn. The host adds no decomposition and carries no sub-task state. |
| D2 Coherence | 2 | Design. The verbatim objective anchors the run. No shared work memory across turns, so turns repeat reconnaissance. RLM recall is emerging, not default. |
| D3 Foresight | 1 | Design. The host prompt is explicitly myopic: "do one concrete useful next step now". No host plan or sequencing. |
| D4 Groundedness | 2 | Design. The provider reads the repository each turn. The host does not verify grounding or reject an ungrounded action. |
| D5 Selectivity | 1 | Design. The owner supplies the objective. In generic mode the system picks any "useful" thing with no owner-priority model. |
| D6 Self-verification | 1 | Design. Provider self-reports completion. Only a post-hoc zero-refs flag exists. No executed done-condition check. |
| D7 Recoverability | 3 | Design plus tests. Typed failure classes, bounded backoff, lane rotation, exactly-once lease, disable-after-failures. The no-progress gate is narrow, so not a 4. |

### The biggest gaps between "needs user guidance" and "trustworthy unattended"

The gap is concentrated in three dimensions: Selectivity, Self-verification, and
Foresight. These three are exactly the owner concern. The system today keeps a
provider working, but it cannot choose the work, cannot check the work, and
cannot sequence the work. The strong Recoverability score does not offset these,
because a robust loop on unselected, unverified, unsequenced work is still work
the owner must guide.

### Ordered list to raise each dimension

Work this order. Each step raises the named dimension and unblocks the next.

1. Add owner-priority objective selection (raises D5 Selectivity from 1 toward
   3). Build a host module that ranks candidate work from the repository
   backlog, the open issues, and the roadmap, weighted by a model of owner
   priority learned from the request history in Part 3. The owner reviews and
   endorses or rejects. Endorsement rate is the metric.
2. Add host-executed done-condition verification (raises D6 Self-verification
   from 1 toward 3). Parse the done condition into a runnable check where
   possible: a named test command, a green-check gate, or an evidence-ref
   predicate. Execute it. Record a typed host verdict separate from the provider
   self-report. Block a false completion.
3. Add a persistent decomposed plan across turns (raises D3 Foresight and D2
   Coherence). Persist a task order and a dependency graph on the run. Carry a
   bounded plan summary into the mission packet. Pick the next unblocking step,
   not a repeat. Reuse the RLM recall path for prior-work memory.
4. Widen the no-progress gate to value, not only machine failure (raises D7
   Recoverability from 3 toward 4). Detect repeated low-value "completed" turns
   through the analyzer's `repeated_disposition_pattern` and a new typed
   per-turn action taxonomy. Pause with a typed reason before waste accumulates.
5. Add a typed per-turn action taxonomy upstream (raises D2 Coherence and
   supports steps 3 and 4). The analyzer already flags this as a residual gap:
   the report carries dispatch phase and disposition only, not recon, setup,
   verify, or edit (`full-auto-run-analyzer.ts:36-43`). Add the taxonomy so
   coherence and churn detection become semantic, not structural proxies.

## Part 5 — Feasibility of self-directed roadmap-building

Can Full Auto today read the repository, the issues, and the transcripts, and
emit a next-features roadmap that the owner would endorse?

Today: no. The parts exist, but the whole does not.

What exists:

- The provider can read the repository, the issues, and `docs/sol/MASTER_ROADMAP.md`
  inside a turn.
- The RLM recall path gives a run access to its own authorized history as a
  cited candidate (`full-auto-recall.ts`).
- The coherence tooling gives a way to score candidate autonomous decisions.
  Complexity gives the evidence weight, and `coherence-screen-v2` gives the
  logic screen.
- The request history in Part 3 is a real, minable signal of owner priority.

What is missing:

- An objective-selection module. No host module ingests the roadmap, the issues,
  and the transcripts and emits a ranked candidate list. The selection is always
  owner-supplied.
- An owner-priority model. No module weights candidates by the owner's revealed
  preferences. The git log and the issue history could train such a model, but
  no module reads them for this purpose.
- A host verifier. Without step 2 of the ordered list, a self-directed roadmap
  would produce unverified "completed" items, so the owner could not trust the
  output.

The path to feasibility is the ordered list in Part 4. Steps 1, 2, and 3 are the
minimum. Step 1 makes the system choose. Step 2 makes the system honest. Step 3
makes the system sequence. With those three, Full Auto could read the repository
state and the request history, emit a ranked next-features list, execute the top
item, verify it, and re-rank. That is the loop the owner asks for. Until then,
Full Auto reduces the need for turn-by-turn steering, but it does not reduce the
need for the owner to choose and check the work.

## Known limits of this method

- Most scores in Part 4 rest on design reading, not on a large live unattended
  sample. A future sweep should score real runs and record the result in a
  ledger, the same way the coherence flywheel records its sweeps.
- The owner-request characterization in Part 3 samples first-directive messages.
  Some sampled directives are agent-task prompts, not direct owner messages.
  Those prompts still reflect the owner's delegation style, but they are one step
  removed from the owner's own words.
- This method scores decision quality. It does not score code correctness. A run
  can score high on this rubric and still ship a wrong change. Correctness needs
  the done-condition verifier from step 2 and independent review.
- The rubric shares the complexity and coherence framing with the sibling
  coherence tooling. If those metrics change their thresholds, re-check the D1
  and D2 descriptors here.
