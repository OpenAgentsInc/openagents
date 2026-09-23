# Ask Coder One about runs

`coder-one ask` answers a question about Terminal-Bench runs by reading the
Gym. It's an episode that ends in an answer with citations rather than a
change, and it only reads. Every claim in the answer names the runs it rests
on, and code checks each citation before you see it.

```sh
coder-one ask "why did the runs Jev flagged as unearned success rank that way, and what do they share?"
coder-one ask "why did Luna fail log-summary-date-ranges?" --executor opus
coder-one ask "why does this one rank?" --run tb4--coder-one-tunable-v6--coq-block-bound/coq-block-bound__Mu8ygpJ
coder-one ask "which docs describe the check recall study?" --scope repo
coder-one ask "which runs did I mark bad, and what do they share?"
coder-one ask --scope highlights --claim cost-0d4bb2f6 --claim leaderboard-31931936
gym coder asks                     # every ask, with its citations and cost
gym coder asks latest              # the last answer, each claim marked
```

Issue #9574 holds the design.

## What an ask does

1. **Probe.** Before any model runs, the host runs a fixed battery of Gym
   reads: `gym runs --order learning --json --limit 40`, `gym runs group
   --by reason --json`, `gym runs group --by task --json`, `gym runs marks
   --json`, and `gym coder matrix --json`, plus `gym runs show RUN --json`
   for the run `--run` names. The matrix runs beside the rest, because only
   the briefing needs it.
2. **Judge.** Jev reads the question and the reason groups and says which
   reasons the question asks about, one Noul per reason. When a person has
   marked runs, the same request asks whether the question is about marked
   runs; see [Read a person's marks](#read-a-persons-marks). The host then
   reads those reasons' runs with `gym runs --reason ID`, and the runs of
   any task the question names. Jev judges which of the candidates bear on
   the question, one Noul per run, and the host opens the strongest six
   with `gym runs show RUN --json` and `--evidence`. A third request judges
   which of their transcript steps bear on it, one Noul per step. The three
   requests cost about $0.0013 together.
3. **Brief.** Code assembles a briefing of at most 48,000 characters: the
   question and the operator's context, how to answer and cite, the exact
   reason counts, the opened runs with every judgment, the evidence Jev
   ranked them from, and the steps Jev chose, then the other candidates and
   the outcome matrix's frontier.
4. **Answer.** The executor reads the briefing and finishes by calling its
   `answer` tool with the answer, its claims, and optional typed
   proposals for a person to approve; see
   [Turn a finding into a proposal](#turn-a-finding-into-a-proposal).
   `--executor luna`, the default, runs Codex on GPT-6 Luna;
   `--executor opus` runs Claude Code on Opus 5.5.
5. **Cite.** Code checks every citation against the Gym. A claim whose
   citation doesn't check, or that cites nothing, is marked unverified and
   kept, with the reason.

## What the executor can do

The executor's only tools are two MCP tools that `coder-one ask tools`
serves: `read` and `answer`. Claude Code runs with `--tools ""`,
`--strict-mcp-config`, and `--permission-mode dontAsk`, so its own shell and
editors are off. Codex runs with its user configuration ignored and its
shell, connectors, browsers, and sub-agents disabled.

`read` runs one command from an allowlist, with no shell, so a pipe, a
redirect, a substitution, or a second command is refused:

- `gym runs …`: the list with its filters, `group --by`, and `show`.
  `gym runs rank` spends Jev requests and writes the learning store, so it
  runs only when you pass `--rank`.
- `gym terminal-bench overview|compare|attempt|evidence|history|runbooks …`.
  `run`, `resume`, and `materialize` are refused.
- `gym coder …`: every read, such as `matrix`, `study`, `composition`, and
  `asks`. `gym coder live --follow` is refused because it never ends.
- `rg` without `--pre` or `-z`, `cat`, `ls`, and `sed -n 'N,Mp' FILE`.

An ask makes at most 24 reads, and each returns at most 40,000 characters.

Every command, the host's probes and the executor with its tools, runs
inside one `coder-boundary` filesystem boundary: `bwrap` on Linux and
`sandbox-exec` on macOS. Its only writable paths are the ask's own scratch
directory and the executor CLI's state and temporary directories
(`~/.claude`, `~/.codex`, `~/.cache`, and the system temporary directory),
and `~/.openagents/gym/learning` when you pass `--rank`. The repository and
the Gym's stores stay read-only. On a host with no boundary, an ask
refuses to run.

## What code checks

| Citation | It checks when |
| --- | --- |
| A run, `job/trial` | The Gym finds a run by that name. A job name with one trial also counts; a task name doesn't. |
| A step, `{run, step}` | The run has a transcript step with that number, as `gym runs show RUN --json` numbers them. |
| A judgment ID | Jev's stored probability for it is at or above 0.50 for every run the claim cites. |
| A file, `path` or `path:LINE` | The file is inside the repository, and the line is within it. |
| A mark, `job/trial` or `job/trial/STEP` | A person marked that run, or that step of it, in `~/.openagents/gym/marks/marks.jsonl`. |

## Read a person's marks

A mark is a person's word that a run, or one step of it, is bad, or that a
run is fine; see
[Mark bad runs and steps](../../gym/terminal-bench-cli.md#mark-bad-runs-and-steps).
The ask reads the marks with `gym runs marks --json`, and the briefing lists
them under **A person's marks**, with each mark's tags, note, and author,
and says that a mark outranks Jev's judgment. Each opened run shows its own
marks.

When there are marks, the Jev request that reads the reasons also asks
whether the question is about marked runs: runs a person flagged as bad,
cleared, tagged, or wrote a note on. When Jev says yes, or, with Jev off,
when the question says "mark", "flagged", "bad run", or "cleared", the
host reads `gym runs --marked --json` and puts the marked runs first among
the candidates, bad before cleared, and the briefing opens them. A claim
can cite a mark in its `marks` field, and code checks that the mark exists.

The executor can't mark a run: the allowlist refuses `gym runs mark` and
`gym runs unmark`, and the boundary keeps the marks store read-only.

## Draft highlights

`gym runs highlights` computes candidate claims worth sharing by fixed
rules, each with its runs, numbers, sample size, and caveats; see
[Find highlights worth sharing](../../gym/terminal-bench-cli.md#find-highlights-worth-sharing).
`coder-one ask --scope highlights` turns chosen claims into short drafts a
person may post:

```sh
gym runs highlights                                     # the claims and their keys
coder-one ask --scope highlights --claim cost-0d4bb2f6  # one claim; repeat --claim for more
coder-one ask --scope highlights                        # the 3 strongest claims
```

The host reads `gym runs highlights --json`, keeps the claims each
`--claim KEY` names, or the 3 strongest, and briefs the executor with each
claim's sentence, numbers, runs, and caveats. It asks Jev nothing: code
chose the claims. The executor writes one draft per claim and names the
claim's key in the draft's `highlight` field. Code then refuses a draft
that fails any check:

- It names one of the chosen claims.
- Every run it cites is one that claim cites, and the Gym has it.
- Every number it writes is a number the claim writes, in its sentence,
  its numbers, or its caveats. A rounded or computed figure is refused. A
  digit inside a name, such as `tb4--x`, `GPT-6`, or `demo__1`, isn't a
  number; a model version such as `Opus 5.5` is, and the claim writes it.
- A claim that rests on one run says so, such as "in one run" or "n=1".

A refused draft is kept with its reasons. The text output marks each draft
`✓` or `✗`, and the record carries `drafts`, each with its `highlight`,
`draft`, `runs`, `status` (`passed` or `refused`), and `problems`. Nothing
posts anywhere: a person picks, edits, and posts. The question is optional
with this scope.
## Turn a finding into a proposal

An answer can end with typed proposals: changes the findings support, for a
person to approve. A proposal names the runs it came from and the tasks it
expects to change. Nothing runs until a person approves it, and then it
runs on those tasks and no others. Issue #9576 holds the design.

```sh
coder-one ask "which failed Luna runs claimed success the checks should have caught?"
gym coder proposals                          # every proposal, with its status
gym coder proposals PROPOSAL_ID              # finding → change → result
gym coder proposals approve PROPOSAL_ID --note "worth three attempts"
coder-one proposal run PROPOSAL_ID           # the mini-task stage: seconds, no model
coder-one proposal run PROPOSAL_ID --live --quota-usd 40 --artifact PATH
gym terminal-bench experiment report PROPOSAL_ID --markdown
```

### Kinds

| Kind | The change | What runs |
| --- | --- | --- |
| `policy` | A JSON merge patch (RFC 7386) on a manifest in `crates/coder-one/policies/` | The mini-task stage, then the live stage |
| `check` | The same, confined to `policy.verify` | The mini-task stage, then the live stage |
| `minitask` | A new mini-task: files, a grader command, and a good and a bad candidate | The mini-task stage: the grader on each candidate |
| `questions` | A change to Jev's question set | Nothing. The set is pinned in code, so it's a drafted issue. |
| `code` | Anything that needs Rust, such as a new check scenario | Nothing. It's a drafted issue for a person to file. |

`coder-one proposal issue PROPOSAL_ID` prints a drafted issue. Nothing
files or merges it on its own.

### What code checks

Code validates each proposal when the ask records it, and keeps a refused
one with its reasons, as it keeps an unverified claim:

- Every source run is a run the ask read, and every expected task is a
  source run's task.
- A policy or check patch applies to a checked-in manifest, sets only
  fields under `policy`, parses under this build's manifest schema, passes
  its validation, and changes the manifest's digest. A check patch sets
  only `policy.verify`.
- A mini-task parses, has a new ID, a grader, and two different
  candidates, and keeps every path inside its directory.
- A questions proposal names the pinned set, `builtin-v1`.

A valid policy or check proposal materializes as `policy.json`, the base
manifest with the patch applied and the name
`<base>-<proposal ID>`. Each proposal records a digest of itself, and a
decision names the digest it decided, so a proposal edited after its
approval doesn't run.

### Approve or reject

`gym coder proposals approve PROPOSAL_ID` and `reject PROPOSAL_ID` record a
decision with an optional `--note` and your user name. In the Gym
terminal's answer view, move the cursor to a proposal and press `a` to
approve or `x` to reject. A refused proposal can't be approved, and a
decision stands once a stage ran.

### Measure it

`coder-one proposal run PROPOSAL_ID` refuses a proposal nobody approved.
For an approved policy or check, it runs the mini-task stage first: the
cited tasks' mini-tasks, each with its known-good and known-bad scripts,
under the base manifest and then the proposal's, with the checks, the
briefing, and the repair each manifest sets. A cited task maps to a
mini-task when one reproduces its failure family: `log-summary-date-ranges`
to `log-severity`, `headless-terminal` to `interactive-terminal`,
`cancel-async-tasks` to `cancel-cleanup`, and `fix-git` to `git-recovery`.
When none maps, the stage screens every mini-task for breakage and says so.

The verdict is `regressed` when the proposal fails good work the base
passed, flags good work the base didn't, or fails to run; `improved` when
it repairs or flags bad work the base didn't; and `unchanged` otherwise. The
scripted executor doesn't read the briefing, doesn't call Jev, and doesn't
hand off, so the stage catches a change that breaks an episode, not one that
helps a model; the result lists what it couldn't show. For a mini-task, the
stage runs the grader on each candidate inside a `coder-boundary` boundary,
and the mini-task reproduces the failure when the grader passes the good
candidate and fails the bad one.

With `--live`, a policy or check whose mini-task stage didn't regress then
starts a targeted `tbench experiment` from `bench/terminal-bench`: the
agent profile that runs the base manifest against the same profile with the
proposal's `policy.json`, 3 attempts per task, interleaved. The proposal's
arm is named by its ID, with `--arm PROPOSAL_ID=BASE_PROFILE`. It runs on
the tasks the proposal expects to change; `--task` narrows them and never
widens them. `--quota-usd` is required and budgets the Claude quota;
`--artifact` names the Coder One build both arms run; `--plan` prints the
schedule and checks credentials without starting a trial. The job profile
is the one the source runs' job names share, such as `panel`, or `tb4`.

`--without-claude` runs both arms with their Claude Code handoff removed,
as experiment `PROPOSAL_ID-codex`, with the arms `PROPOSAL_ID-base` and
`PROPOSAL_ID`. A Luna policy then draws only on Codex, so its trials don't
wait for the host-wide Claude slots other experiments hold. A manifest that
names Claude Code anywhere else is refused. The comparison is still
matched: both arms lose the same handoff.

### The record

A proposal lives under `~/.openagents/coder-one/proposals/<id>/`, or the
directory `coder-one ask --proposals DIR` names:

- `proposal.json`: `openagents.coder-one.proposal.v1`, written once by the
  ask, with the ask's ID and question, the source runs and tasks, the
  change, what code found, and the digest.
- `policy.json` or `minitask.json`: the materialized change.
- `decision.json`: `openagents.coder-one.proposal-decision.v1`.
- `result.json`: `openagents.coder-one.proposal-result.v1`, with the
  mini-task stage and the live experiment's ID.

The ask's own record lists its proposals, and `gym coder asks ID` shows
them. `gym coder proposals PROPOSAL_ID` shows the loop: the finding (the
ask, the rationale, and the runs), the change (the patch and the digests),
the decision, and the result (the mini-task stage and each arm's passes
from the experiment's status).

## What it costs

Measured on 2026-09-23 against this machine's Gym (597 runs, 561 judged),
with release builds of `gym` and `coder-one`, on the question in the first
example:

| Executor | Time | Cost | Citations checked | Claims verified |
| --- | ---: | ---: | ---: | ---: |
| Luna (Codex, `gpt-6-luna`) | 50.1 s | $0.0085 | 18 of 18 | 4 of 4 |
| Opus (Claude Code, `claude-opus-5-5`) | 40.9 s | $0.32 | 34 of 34 | 5 of 5 |

A Luna cost is a list-price estimate, because Codex reports no cost. An
Opus cost is Claude Code's own figure, a list price on a subscription.
Jev's three requests are priced at $0.042 per million input tokens.
`--budget USD` caps the whole ask, $1.00 by default: Claude Code enforces
its share with `--max-budget-usd`, and the record reports a Codex run that
went over.

## Options

| Option | Effect |
| --- | --- |
| `--scope gym\|repo\|highlights` | `gym`, the default, probes the Gym. `repo` searches the repository's Markdown for the question's words, and Jev judges which files bear on it. `highlights` drafts from the Gym's highlights; see [Draft highlights](#draft-highlights). |
| `--claim KEY` | With `--scope highlights`, a highlight to draft. Repeatable. |
| `--executor luna\|opus` | Codex on GPT-6 Luna, or Claude Code on Opus 5.5. `--model` names another model. |
| `--budget USD` | The most the ask spends, Jev included. |
| `--run RUN` | The run the operator has selected, opened whatever Jev says. |
| `--context TEXT` | A line the executor reads, such as the active filter. Repeatable. |
| `--rank` | Lets the executor run `gym runs rank`. |
| `--json` | Prints the record as JSON. |
| `--events` | Prints one JSON event per line as the ask runs, then `{"event":"answer","record":…}`. The Gym terminal reads this. |
| `--timeout SECONDS` | The executor's deadline, 240 by default. |
| `--gym PATH` | The `gym` binary. By default, `$CODER_ONE_GYM_BIN`, the `gym` beside `coder-one`, or `gym` on `PATH`. |
| `--no-jev`, `--jev-recorded FILE`, `--jev-record FILE` | Code's order instead of Jev, recorded answers, or a file of the answers used. |
| `--proposals DIR`, `--no-proposals` | Where the answer's proposals are written, `~/.openagents/coder-one/proposals` by default, or none. |
| `--answer-file FILE` | Replays an executor's answer instead of running one: the citation check and the record, with no model. |

## Ask from the terminals

In the Gym terminal, press `?` in the Runs pane; see
[View Terminal-Bench runs in the Gym terminal](../../gym/terminal-bench-tui.md#ask-coder-one-about-runs).

In the Coder Terminal, the `review-runs` program asks the same question: a
`query` step over the `gym-runs` command source, then a `delegate` step to
the `coder-one-ask` capability, which runs `coder-one ask --scope gym`. The
turn's program-selection question has to pick it, and on 2026-09-23 hosted
Jev picked it for 2 of 6 questions about runs; see
[program selection with review-runs](../../decision-models/measurements/2026-09-23-program-selection-v3.md).
Both capabilities run only under an approval, with `gym` and `coder-one` on
`PATH`:

```sh
capability-trust approve gym
capability-trust approve coder-one-ask --writable ~/.codex --writable ~/.openagents/coder-one/asks
coder -p "Which Terminal-Bench runs claimed success they didn't earn?" --programs review-runs
```

A delegation seals the directory that holds the trust store, so keep the
store outside `~/.openagents` with `CODER_CAPABILITY_TRUST` when asks record
there; an ask that can't write its record directory records under the
temporary directory instead and says so. Run that way on 2026-09-23, the
turn selected `review-runs`, read `gym runs`, and answered through Luna in
43 seconds for $0.0057, with 34 of 34 citations checked.

## Measure the ask

`coder-one ask study` asks every question in a question set whose answers
are already written down and scores each answer by code:

- **Citation validity:** the share of the answer's citations the check held.
- **Task recall:** the share of the tasks the written answer names that the
  answer cites a run of.
- **Run recall:** the share of the runs the written answer names that the
  answer cites.
- **Cost and time:** what the ask reported, Jev included.

```sh
coder-one ask study --retain bench/terminal-bench/asks/studies    # Luna, every question
coder-one ask study --executor opus --only cad-model-failure
gym coder asks --studies                                          # studies side by side
gym coder asks --study ask-study-1790189525427                    # one study's rows
```

The set is
[`bench/terminal-bench/asks/questions-v1.json`](../../../bench/terminal-bench/asks/questions-v1.json):
14 questions whose answers the 2026-09-23 Terminal-Bench reports record,
each with the tasks and runs a good answer cites, checked against the Gym.
Each result (`openagents.coder-one.ask-study.v1`) carries a digest of the
implementation (the battery, the relevance questions, the briefing, the
allowlist, and the executor), so a change to any of them is a new row to
compare against the baseline.

The baseline, `ask-study-1790189525427`, ran on 2026-09-23 with Luna and
is retained under
[`bench/terminal-bench/asks/studies/`](../../../bench/terminal-bench/asks/studies/):

| Measure | Value |
| --- | ---: |
| Answered | 14 of 14 |
| Citations checked | 161 of 162 (0.994) |
| Mean task recall | 0.647 |
| Mean run recall | 0.438 |
| Cost, all 14 | $0.0954 |
| Mean time | 40.8 s; 13 of 14 under a minute, the slowest 60.9 s |

Citations are almost always valid; what the ask misses is coverage. It finds
every task on the 8 questions about one task or one pair of runs, and falls
short on the questions whose answer is a list: which runs claimed success
they didn't earn (0 of 3 tasks; the Gym gives `unearned_success` to 125
runs, and the answer cites other ones), which failures no check caught (3
of 13), which failures the new checks flag (3 of 10), and which runs
couldn't read their instructions (0 of 2). Those are the questions to
improve the battery on first.

## The record

Each ask records itself under `~/.openagents/coder-one/asks/ask-<ms>/`, or
under `--out DIR`:

- `episode.atif.jsonl`: the ATIF invocation log, with an `ask.probes`
  component per battery, an `ask.relevance` component per Jev request, and
  `ask.briefing`, `ask.executor`, and `ask.citations`. Each executor read is
  a call step.
- `manifest.json`: `openagents.coder-one.ask.v1`, with the question, the
  answer, each claim and its check, the cost, and the time.
- `briefing.md`, `stream.jsonl`, `calls.jsonl`, and `answer.json`.

`gym coder asks` lists the records, and `gym coder asks ID` prints one.
