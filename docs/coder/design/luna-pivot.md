# The Luna pivot: Jev structure around a cheap executor

Status: direction, 2026-09-24. This document sets Coder's next focus. It
replaces "route to a stronger model when things go wrong" with "give the
cheap model the structure it needs to not go wrong." The measurements it
cites are in [`docs/terminal-bench/`](../../terminal-bench/README.md).

## The thesis

The general argument, with its predictions and what would falsify it, is
in [the determinism thesis](thesis.md). This section applies it to Luna.

Coder's standing goal is to be the cheapest and the best on every task.
The last two days of Terminal-Bench work spent most of their effort on
configuration: which executor, which effort level, when to escalate, how
many persistence rounds. Those levers are real, but they share a ceiling.
They move work between expensive models rather than removing it.

The Luna pivot bets on a different lever:

- **GPT-6 Luna can do most of the work** that Coder currently sends to Opus
  5.5 or GPT-6 Astra, if code and Jev give it the right structure: the
  right evidence, one requirement at a time, and a check that tells the
  truth.
- **The System One algorithms are the unlock.** Deterministic code and
  cheap typed Jev judgments decide what Luna sees, what it works on next,
  and whether its result holds. Routing cleverness is secondary.
- **A small harness of our own, Microluna, makes the structure cheap to
  apply.** It runs many short Luna sessions in one conversation, each with
  a context rebuilt from Jev's judgments, instead of one long session
  whose context only grows.

With Jev and Microluna, Coder should beat every harness by a wide margin
on cost and speed. The remaining work is then one game: close the
intelligence gap with better Jev algorithms, measured one at a time.

## Why Luna, from the evidence so far

- **Price.** Coder One prices GPT-6 Luna at $0.10 per million input tokens,
  $0.01 cached, and $0.50 output (`crates/coder-one/src/delegate.rs`).
  Opus 5.5 trials on TB4 averaged about $2.00 to $2.22 each
  ([what we have learned](../../terminal-bench/2026-09-23-what-we-have-learned.md)).
- **Luna with the right evidence already matches Opus on the development
  panel.** On the eight-task panel, "Luna first, coverage packer" passed
  24/24, the same as tunable Opus. Its lower-bound cost was $0.1504 across
  the eight per-task means, against $0.5219 for tunable Opus and $1.0863
  for direct Opus
  ([tunable results](../../terminal-bench/2026-09-23-tunable-results.md)).
- **Luna's failures were evidence failures, not intelligence failures.**
  Jev-probe v3 → Luna failed `log-summary-date-ranges` 0/3 because the
  briefing dropped every log excerpt Jev had selected, so Luna never saw a
  record's severity field. The coverage packer delivered the records, and
  the same Luna passed 3/3
  ([tunable results](../../terminal-bench/2026-09-23-tunable-results.md),
  [Luna assessment](../../terminal-bench/2026-09-22-luna-jevprobe-upgrade.md)).
- **An ask over the Gym runs on Luna for about a cent.** `coder-one ask`
  answered 14 questions for $0.0954 in total, with 161 of 162 citations
  checked ([ask guide](../guides/coder-one-ask.md)).
- **The expensive configuration levers didn't pay.** With the executor
  held fixed, Coder One's controller (checks, repair, persistence,
  escalation) cost 68% more than Claude Code alone, with no significant
  pass gain: 18/30 against 15/30, McNemar p = 0.51
  ([matched controller test](../../terminal-bench/2026-09-23-matched-controller-targeted.md)).

## What we don't know yet

- **Luna has never run on TB4.** No retained or local job runs Luna on the
  66-task Terminal-Bench 4.0 suite. Every Luna result above is on the
  eight-task development panel, which is much easier. The first job of the
  pivot is to measure Luna on TB4 tasks, directly and with Jev structure.
- **Our checks don't tell the truth yet.** Across 80 graded trials in the
  current experiments, Coder One's final checks didn't discriminate:
  "all passed" was 19 passes and 19 fails, and "a check failed" was 3 and
  5. A cheap executor needs a verifier it can trust more than an expensive
  one does, so this is the pivot's first algorithm problem.
- **Luna-in-Codex is a black box.** Codex CLI runs Luna in one long session
  with its own prompt, tools, and context management. We can't rebuild
  its context per step, run several short sessions, or see its decisions
  as typed events. That's the case for Microluna.

## Learn from the winners, granularly

The public Fable 5.1 trajectories are now in the Gym: all 1,649 of them,
66 tasks by five rows, with head-to-head replay against Coder One
([head to head](../../gym/head-to-head.md)). Fable max passes 191 of 330
trials on the TB4 snapshot. Use them as a curriculum, not a scoreboard.

1. **Strategy fingerprints.** For every Fable and Coder One trajectory, code
   segments the steps into phases: orient, read, plan, edit, build, test,
   verify, and finish. Jev labels each step with a Choice over those
   phases and a few Nouls: "is this step checking an assumption?", "did
   this step use evidence from an earlier one?", and "is this a retry of a
   failed step?". The result is a per-trajectory fingerprint: time to
   first edit, how often it tests, how verification is spread across the
   run, retries, and the files it touched.
2. **Moves Fable makes that Luna doesn't.** Compare fingerprints on the
   same task, winners against losers. Each difference that repeats across
   tasks is a candidate System One algorithm. For example: Fable runs the
   task's own example before editing, or re-reads the instruction before
   finishing. Code or Jev can supply that move to Luna as structure,
   instead of hoping a prompt makes Luna do it.
3. **Keep it in the Gym.** Fingerprints and moves are Gym records, so
   `coder-one ask` can answer "what does Fable do on
   `mvcc-lsm-compaction` that Luna skips?" with checked citations.

## The System One algorithms to build for Luna

Each algorithm is a component with a typed signature, tested on
mini-tasks in seconds and measured on small TB4 subsets with early
stopping. They're listed in the order the evidence suggests.

1. **Truthful checks.** Checks have to discriminate before anything else
   can lean on them. Calibrate check verdicts and Jev support answers
   against the verifier on the roughly 600 graded retained runs, which are
   free labels, and keep only the signals that separate passes from
   failures. Add behavior checks generated from the task's own words, and
   report each check's discrimination in `gym experiment pulse` (#9582).
2. **Evidence for every requirement.** Extend the coverage packer so every
   requirement Jev extracts arrives with the evidence that decides it:
   data records, exact file regions, and example inputs and outputs. This
   is the change that took Luna from 0/3 to 3/3 on log summaries.
3. **One requirement at a time.** Decompose the task into requirements,
   give each its own short Microluna session with only its evidence, and
   check each one before the next starts. A small model does better on a
   small, fully specified problem than on a long, open one.
4. **Next-step choice.** At each step, code proposes candidate actions
   (run the example, run the tests, read a file, or edit a region), and a
   Jev Choice picks one from the state. Luna writes the edit. The loop
   stops wandering because the choice is typed.
5. **Stall, loop, and done detection.** Jev Nouls on the live event
   stream: is Luna repeating a failed step, stuck on the network, or
   already done but still spending? Code acts on the answer: re-brief,
   stop, or finish.
6. **Best of N, then select.** A Luna attempt costs cents, so run several
   in parallel (the existing race pattern) and let truthful checks and
   Jev pick the winner. Five Luna candidates still cost a fraction of one
   Opus trial. This only works once algorithm 1 works.
7. **Fable's moves.** Each repeated move from the fingerprint comparison
   becomes an algorithm like the ones above, measured the same way.

Escalating to Opus or Astra stays available as a last resort for tasks
the algorithms can't yet handle, and each such task is logged as a gap
for the algorithms to close. It isn't the plan.

## Microluna

Microluna is a minimal Rust harness for Luna that Coder One owns: an
executor adapter beside `claude-code` and `codex` in `crates/coder-one`, so
every existing policy, record, and Gym view works with it.

- **Transport.** It calls Luna directly over the Responses API, with
  native function tools and no JSON in text: the lesson from the Gemini
  loop. It spawns no CLI process, so a session starts in milliseconds.
- **Tools.** A small fixed set: run a command, read a file region, apply a
  patch, write a file, and finish with a typed result. Every call runs
  under `crates/coder-boundary` and `crates/supervise`, with the turn's
  permit.
- **Many mini sessions in one chat.** A task is a sequence of short
  sessions that share one workspace and one state object. Before each
  session, code and Jev rebuild the context from scratch: the requirement,
  its evidence, what earlier sessions changed, and what the checks say
  now. This is the design from
  [episode 287](../../transcripts/287.md), a coding agent built as if
  there were no KV cache, now with the cache used deliberately: the stable
  prefix (instructions, task, evidence) comes first, so the provider can
  cache it, and the changing part comes last.
- **Typed events.** Every session emits its tool calls, outputs, usage,
  and cost as ATIF steps, so the Gym, the live view, and Jev monitors read
  Microluna the way they read everything else.
- **Borrow ideas, not code.** OpenAI's Codex CLI is open source and written
  in Rust. Read it for what it does well (sandboxing, patch application,
  and context compaction), and reimplement what we need here, stating the
  source in the commit message, as `AGENTS.md` requires for any design we
  carry over. Confirm the license before quoting anything.
- **Prerequisite.** Direct API access to `gpt-6-luna`, through an OpenAI key
  or the OpenAgents gateway. Today Coder One reaches Luna through the
  Codex CLI's login.

## How we measure the pivot

- **The baseline:** Luna-in-Codex, direct and with today's Jev briefing, on
  a fixed TB4 subset: the 10 tasks from the matched controller test, plus
  a few Luna-sized tasks from the leaderboard reference. Three attempts,
  interleaved, with early stopping (#9582).
- **Then one algorithm at a time,** each as a policy change on Microluna
  against that baseline, on the same subset. Keep a change only when it
  moves passes or cost per pass with a stated interval. Stop losers early.
- **The scoreboard:** cost per pass and time per pass against Claude Code
  on Opus 5.5 and against Fable max on the same tasks, from the Gym. The
  pivot succeeds when Luna plus Jev matches Opus's passes on a task family
  at a small fraction of its cost per pass. Then the task router sends
  that family to Luna by default.

## What stops or slows

- New escalation tiers and routing variants get no new work beyond what's
  already in flight (#9569, #9571). Their write-ups still land.
- Persistence rounds on Opus stay off by default. The matched test found
  they were 52% of the controller's Claude spend.
- No full-suite runs until targeted runs show a large, measured gain.

## First steps

1. Measure Luna on TB4: Luna-in-Codex direct and with the current briefing,
   on the fixed subset, stopping early.
2. Build truthful checks: calibrate against the retained graded runs, and
   report discrimination per check.
3. Build Microluna's first slice: one session, native tools, the boundary,
   and ATIF events, matched against Luna-in-Codex on the same subset.
4. Add strategy fingerprints to the Gym, and compare Fable with Luna on the
   subset's tasks.
5. Then run the algorithm list, one change per experiment.
