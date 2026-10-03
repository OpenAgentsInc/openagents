# Issue briefing preview

Source commit: `682d958c7eb24a4dc32ed503965f4c103919cd77`

## Original issue

```text
Codex lean session: one codex exec session with the Jev briefing as the default Codex route (coder.codex)

Part of #10204. The Claude lean session (#10246, 42d1539463) cost 0.61× raw Claude Code at 21/21 passes. Build the Codex equivalent designed in docs/cost/2026-10-02-shadow-baseline-measurement.md ("The lean session arm"): one `codex exec` session with the same Jev briefing, medium effort (owner default gpt-6.1-sol medium), core prompt, cache-friendly; a `coder.codex` setting (session | loop). Measure on the #10209 harness vs raw Codex (7 tasks × 3 trials) and make it the default if it wins on cost at equal passes.
```

## Selected evidence

### `docs/cost/2026-10-02-shadow-baseline-measurement.md`: 1–64 of 421 lines

Explicit issue path; Lexical overlap: 10204, 10209, 10246, 2026, arm, baseline, briefing, build, cache, claude, code, coder

File SHA-256: `1372c0ccf49a12708ba5ecbdf5830a085812652860600ddadcb5544da993035d`. Git blob: `aa5cda419de46f7fb0a0d752f616ab92045deafc`.

```text
# Shadow baseline, first measurement: routed OpenAgents against raw Claude Code

2026-10-02, for [#10209](https://github.com/OpenAgentsInc/openagents/issues/10209)
(part of the umbrella [#10204](https://github.com/OpenAgentsInc/openagents/issues/10204)).
The plan was the cost audit's
[section 5b](2026-10-02-system-one-cost-efficiency-audit.md#5b-measuring-the-recipe).
This is the first test of the claim "routing work beats raw delegation"
on the path the terminal ships (`openagents chat send`, routed to a Coder
task), not on a bench arm.

## Result

**On these seven tasks, the shipped routed path did not save money or time
against raw Claude Code. On the Claude engine it cost 68% more and took 39%
longer at the same pass rate.** The Codex engine cost 13% less than raw
Claude Code, which is a cheaper model's list price, and took 2.6 times as long.
The delegate recipe (#10208) had no measurable effect on the Claude engine.
On Codex it raised cost by 61% and time by 43%, because Jev classed 18 of 21
tasks as "hard", which runs Codex at high effort.

The cause on Claude is caching, not tokens. The routed loop sent 3.7 times
fewer input tokens than raw Claude Code, but it wrote 85–88% of them to the
prompt cache at 1.25 times the input price and read only 12–15% back. Raw
Claude Code read 93% of its input from cache at 0.1 times the price. Each
Microcoder step is a fresh `claude -p` call, and the step's prompt changes near
its start, so the five-minute cache almost never hits.

The audit's 61% and 63% savings (sections 1 and 2c) came from a different
path: the Coder One CLI delegate (Jev probes in front of a lean Claude Code
session with six tools) on Harbor. That path was not run here, so this
measurement does not refute those numbers. It shows that the routed path the
terminal uses today does not reproduce them.

**Update, #10246:** a sixth arm ran the audit's own configuration (Jev
briefing one lean Claude Code session) as a route. It cost 0.61× raw Claude
Code (95% CI 0.57–0.65) at 21 of 21 passes with no measurable difference
in wall time, and is now the default Claude route
([below](#the-lean-session-arm-10246)).

## Numbers

105 runs: 7 tasks × 5 arms × 3 trials, all on coderos-4080 on 2026-10-02.
Passes come from an independent check (below). Cost is list price for the
engine plus Jev. Wall time is end to end, from the command's start to the
route record settling (routed) or to the process exiting (raw).

| Arm | n | Passed (Wilson 95%) | Total cost | Median cost per run | Total wall time | Median wall time per run | Median input tokens |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| Raw Claude Code (`claude -p`, defaults: Opus 5.5 1M) | 21 | 21/21 (85%–100%) | **$6.24** | $0.238 | **24.3 min** | **45 s** | 194,547 |
| Routed, Claude engine, recipe on | 21 | 21/21 (85%–100%) | $10.52 | $0.285 | 33.9 min | 64 s | 52,452 |
| Routed, Claude engine, recipe off | 21 | 20/21 (77%–99%) | $10.00 | $0.321 | 35.7 min | 84 s | 51,390 |
| Routed, Codex engine (GPT-6.1 Sol), recipe on | 21 | 21/21 (85%–100%) | $5.45 | $0.119 | 63.6 min | 124 s | 58,564 |
| Routed, Codex engine, recipe off | 21 | 21/21 (85%–100%) | **$3.38** | $0.104 | 44.6 min | 99 s | 43,621 |

Each arm compared with raw Claude Code: the sum over tasks of per-task means,
with a 95% bootstrap interval that resamples trials within each task.

| Arm | Cost ratio (95% CI) | Cost | Wall-time ratio (95% CI) | Time |
| --- | --- | ---: | --- | ---: |
| Claude, recipe on | 1.68 (1.46–1.95) | 68% more | 1.39 (1.02–1.89) | 39% longer |
| Claude, recipe off | 1.60 (1.44–1.75) | 60% more | 1.47 (1.15–1.87) | 47% longer |
| Codex, recipe on | 0.87 (0.79–0.96) | 13% less | 2.61 (2.05–3.31) | 161% longer |
| Codex, recipe off | 0.54 (0.51–0.57) | 46% less | 1.83 (1.41–2.37) | 83% longer |


```

### `docs/cost/2026-10-02-system-one-cost-efficiency-audit.md`: 1–64 of 616 lines

Lexical overlap: 10204, 10209, 10246, 2026, arm, baseline, briefing, build, cache, claude, code, coder

File SHA-256: `de22cb1ef973d42d06b307a47bd279574c8dd1aa79332c6a8489f7705226ae5c`. Git blob: `a97772ce4a50a526048225941cf7137a4530e6eb`.

```text
# System One cost efficiency: what we measured, and how to make it the default

2026-10-02. An audit for the owner of what was learned from 2026-09-22 to
2026-09-28 about making Fable and Claude Code runs cheaper and faster with
System One (code plus Jev), and of how much of it the current harness uses.
Every number below is copied from a committed report or a retained
conversation, with its source. Nothing was rerun for this audit.

## 1. The reminder

Between episodes 287 and 288 we found a recipe that makes a frontier
coding model do the same work for much less money and less waiting, and we
carried it across every task set we had. It is four things Coder does
around the same model, all System One (plain code plus cheap typed Jev
judgments, about 200 ms and a fraction of a cent each):

1. **Jev scouts and briefs first.** Jev probes the workspace with
   read-only commands and picks the evidence, so the model starts from a
   briefing instead of exploring. Input tokens fell 44%.
2. **Six tools and a trimmed system prompt** instead of Claude Code's
   default set, so every call carries less.
3. **A five-minute prompt cache** instead of one hour (cache writes cost
   1.25x input instead of 2x). This alone cut 19–25%.
4. **Lower reasoning effort** (low or medium instead of high), because the
   briefing already did the looking.

The assessment that names these four was written on the Coder box on
2026-09-23 ([section 2a](#2a-the-assessment-four-things-replicated-across-26-tasks)).
The recipe evolved over two days and about 25 configurations (the Gemini
loop, then Jev probes, probe v2 and v3, the five-minute cache, tunable v2
to v10), and it replicated at every step:

| Step | Tasks | Result against the same model run raw |
| --- | --- | --- |
| Development panel, 3 trials each | 4 | 12/12 both ways, **63% cheaper, 32% faster** than Claude Code on Opus 5.5 |
| Four newer tasks, 3 trials each | 4 | 12/12 both ways, **57% cheaper, 48% faster** |
| All eight development tasks | 8 | 24/24 both ways, **$0.43 vs $1.09 (61% cheaper), 192 s vs 306 s (37% faster)** |
| Terminal-Bench 4.0, one attempt each | 26 | **11 passes vs 9, $32.87 vs $59.27 (45% cheaper), 182 vs 295 min (38% faster); cheaper on 24 of 26 tasks, faster on 22** |
| Against the TB4 leaderboard, matched per task | 45 | About 80% of the top rows' accuracy at **a fifth to a ninth of their cost** |
| The same recipe in front of Fable 5.1 low | 15 | **Beat Fable 5.1 low's own cheapest and fastest winning run on 5 tasks**: `fin-saccr-rwa`, `coq-block-bound`, `mp-checkpoint-consolidation`, `sound-change-cascade`, `gsea-proteomics` |
| The recipe with a cheap model (Microcoder, GPT-6 Luna) | 65 + 3 | Out of sample on TB2.1: **30 confirmed wins** on cost against Fable 5 xhigh, median pass at **2.9%** of Fable's cost. In sample on TB4: Fable's pass rate on `embedding-drift-monitor` at **1/58 of the cost**, and passes on three TB4 tasks at 1/45 to 1/2 of Fable's cheapest win |

A matched test later held effort and cache equal on both sides and found
the controller by itself saved little (7.6%). That is the useful finding,
not a caveat: the savings live in the four choices, which are portable
settings any delegation can adopt, whether it goes to Claude Code, Codex,
or Fable.

The principle under all of it is the one from episodes 286 and 287: code
owns control, Jev judges narrowly, and the generative model only
generates. Every decision moved out of the expensive model's turns and
into code or a typed judgment made the run cheaper.

## 2. The grids

### 2a. The assessment: four things, replicated across 26 tasks

Verbatim from the Coder box: coderos-4080,
`~/.claude/projects/-home-christopherdavid-openagents/4d9ec5a2-be11-427a-bc5a-9f3040a354c4.jsonl`,
assistant message at 2026-09-23T14:23:48Z, answering "please write an
assessment on just that fact". The full assessment is committed as
[`2026-09-23-coder-one-vs-claude-code-tb4.md`](https://github.com/OpenAgentsInc/openagents/blob/62bb353abde1d2db74b957b9c1faa49f76f11379/docs/terminal-bench/2026-09-23-coder-one-vs-claude-code-tb4.md)
([`62bb353abd`](https://github.com/OpenAgentsInc/openagents/commit/62bb353abde1d2db74b957b9c1faa49f76f11379)).


```

### `crates/coder-one/src/micro/lean.rs`: 73–136 of 3891 lines

Lexical overlap: 2026, baseline, briefing, build, cache, code, coder, cost, default, docs, effort, equal; Declaration hint: Lean

File SHA-256: `23ca47a121471cb9202b56c7d6bc01e414a361626128b3d75d55ed450ee48d26`. Git blob: `75f46249f61f1a7469383862658bf95fab1e30d4`.

```text
        }
    }
    Ok(tree)
}

/// `executor.microluna.lean`: the lean loop's shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lean {
    /// Work sessions at most, the self-check not counted.
    pub sessions: u32,
    /// Characters of the workspace's current source files in the prefix.
    pub source_chars: usize,
    /// Characters of data-file heads in the prefix.
    #[serde(default)]
    pub sample_chars: usize,
    /// End with a self-check session on a fresh context.
    #[serde(default)]
    pub self_check: bool,
    /// Tell each session to hold out part of any provided examples and to
    /// measure on the held-out part.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub holdout: bool,
    /// Scan each session's changes for hard-coded examples, in code and
    /// with one Jev question.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hardcode_check: bool,
    /// Have the first session write an evaluation script, freeze it, score
    /// the workspace after every session, and finish on the best snapshot.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub keep_best: bool,
    /// The score script's wall-time bound, in seconds.
    #[serde(default = "score_sec")]
    pub score_sec: u64,
    /// When the host turns a work session's `finish` back, or `None` to let
    /// every finish stand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persist: Option<LeanPersist>,
    /// The loop's wall-time bound in seconds, the self-check included: no
    /// session starts in its last minute, and each session ends by it. 0
    /// leaves only the dispatch's deadline.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub wall_sec: u64,
    /// Add the working practices ([`PRACTICES`]) to the guidance.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub practices: bool,
    /// Put the comments that defend a design choice in the evidence, as
    /// suspects (`accept::defended_choices_general`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub defended: bool,
    /// Bound each session's spend by what is left of the dispatch's.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub session_spend: bool,
    /// Count whole records, an input with its answer, in the literal scan
    /// ([`data_records`]) instead of single fields, so a provided word list
    /// a solution may use isn't read as hard-coded examples.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub records: bool,
    /// Every command's wall-time bound in seconds, below the tool's own
    /// 600. 0 keeps the tool's.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub command_sec: u64,
    /// Add the symptom practice alone ([`SYMPTOMS`]), without the search
    /// practice `practices` also carries.

```

### `crates/coder-one/src/policy.rs`: 714–777 of 2656 lines

Lexical overlap: 2026, arm, baseline, briefing, build, cache, claude, code, coder, codex, core, cost; Declaration hint: jev

File SHA-256: `21d13bf3200fea4ac3bed7299f094ae72d8944e5a4cf1056c9daf94d515dda37`. Git blob: `8239fb3e5ea4a0f4cec39e44fa60c3256ad99b7a`.

```text
    /// The delegate mode.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.policy.control.delegate.mode()
    }

    /// Whether Jev runs at all.
    #[must_use]
    pub fn jev(&self) -> bool {
        self.policy.jev.mode != JevMode::Off
    }

    /// Whether Jev runs in deep mode.
    #[must_use]
    pub fn deep(&self) -> bool {
        self.policy.jev.mode == JevMode::Deep
    }

    /// The judge this manifest configures.
    #[must_use]
    pub fn judge(
        &self,
        client: Option<jev::Client>,
        workdir: PathBuf,
        issue: &crate::state::Issue,
        recorder: crate::record::Recorder,
    ) -> crate::judge::JevJudge {
        coder_delegate::policy::judge(
            &self.policy.jev,
            &self.policy.evidence,
            client,
            workdir,
            issue,
            recorder,
        )
    }

    /// The executor this manifest configures, given what the host found.
    #[must_use]
    pub fn executor(&self, host: ExecutorHost) -> Cli {
        coder_delegate::policy::executor(&self.policy.executor, host)
    }

    /// The sections a turn reads: Jev, evidence, the briefing, and the
    /// executor, without Microluna's section.
    #[must_use]
    pub fn turn(&self) -> coder_delegate::policy::TurnPolicy {
        let executor = &self.policy.executor;
        coder_delegate::policy::TurnPolicy {
            jev: self.policy.jev.clone(),
            evidence: self.policy.evidence.clone(),
            brief: self.policy.brief.clone(),
            executor: coder_delegate::policy::ExecutorPolicy {
                agent: executor.agent,
                version: executor.version.clone(),
                model: executor.model.clone(),
                effort: executor.effort.clone(),
                tools: executor.tools.clone(),
                prompt_cache_ttl: executor.prompt_cache_ttl.clone(),
                deadline_sec: executor.deadline_sec,
                system: executor.system.clone(),
                session: executor.session.clone(),
                microluna: None,
            },

```

### `docs/terminal-bench/2026-09-23-coder-one-vs-claude-code-tb4.md`: 1–64 of 191 lines

Lexical overlap: 2026, arm, baseline, briefing, cache, claude, code, coder, core, cost, default, docs

File SHA-256: `22bdbd72267eca4d828a33c3683f7c41f9156e27fd109463df8c525a23bd8082`. Git blob: `94bdb3f50b960b42d91c0306dece737615ce918b`.

```text
# Coder One against Claude Code on Terminal-Bench 4.0

Status: assessment of one-attempt trials on 2026-09-23. It examines one
claim from the running Terminal-Bench 4.0 (TB4) suites: on the same tasks,
on the same host, with the same model, Coder One passes more tasks than
Claude Code for less money. The data are retained under
`~/.openagents/terminal-bench/jobs/tb4--coder-one-tunable-v2--*` and
`tb4--claude-code-opus--*`, and are regenerated with
`bench/terminal-bench/tools/tb4_scoreboard.py`.

**Subsequent controlled comparison:** the [12-attempt matched Opus pilot](2026-09-23-matched-opus-controller.md)
holds the executor settings fixed. Plain Claude passed 6/6 for $6.64 and
37.0 agent-minutes; Coder passed 5/6 for $6.14 and 34.4 minutes, including
Jev. It records modest aggregate savings with one fewer pass, not equal-success
efficiency. This two-task pilot does not replace the 26-task configuration
comparison below; it limits what that comparison can claim about adding the
controller alone. Its report retains an unchanged-candidate grade recovery
and a sensitivity analysis excluding that pair.

## The claim, corrected

The first version of this claim, posted on 2026-09-23, read "12 passes for
$35.56 against 10 for $61.69 on 27 tasks". A closer look at every trial
removed two tasks whose trials didn't measure the agents:

| Task | Arm | Why it doesn't count |
| --- | --- | --- |
| `kv-live-surgery` | Claude Code | The trial ran from 09:28 to 11:06 UTC, inside the window when the Claude subscription's usage limit was exhausted (it reset at 11:51). It recorded no usage and its agent logs are missing, so it can't be shown to have run normally. It scored 0. |
| `risk-scorer-replay` | Coder One | Coder One never started: `cannot read /opt/openagents/instruction.txt: Permission denied`. The task image runs as a user that couldn't read the instruction file the adapter uploaded. The verifier then graded an untouched workspace, scoring 0. This is a Coder One harness bug, not a capability result. |

Removing both leaves **26 tasks with a valid trial on each side**:

| | Coder One v2 | Claude Code |
| --- | ---: | ---: |
| Tasks passed | **11 of 26** | 9 of 26 |
| Pass rate (Wilson 95% interval) | 42% (26%–61%) | 35% (19%–54%) |
| Total cost | **$32.87** | $59.27 |
| Total agent time | **182 min** | 295 min |
| Input tokens | 33.9 M | 60.5 M |
| Output tokens | 0.86 M | 1.36 M |

**Coder One passed two more tasks for 45% less money and 38% less agent
time.** It was cheaper on 24 of the 26 tasks and faster on 22.

## What each arm is

Both arms run Claude Code 2.1.280 on Claude Opus 5.5 with the same
subscription credential, in the same Harbor 0.22.0 task containers on the
same host, one attempt per task, on the pinned TB4 tasks (`v4.0.0`).

| | Claude Code (`claude-code-opus`) | Coder One v2 (`coder-one-tunable-v2`) |
| --- | --- | --- |
| Harness | Harbor's `claude-code` agent | Coder One's episode wrapping Claude Code as its executor |
| Reasoning effort | high | medium on long tasks (every TB4 task has an 8-hour timeout) |
| Tools | Claude Code's full default set | Six: `Bash, Read, Edit, Write, Glob, Grep` |
| System prompt | Claude Code's default | A headless core replacing the default, with its security section kept |
| Prompt cache | One hour (the subscription default) | Five minutes |
| Before the executor | Nothing | Jev-selected probes, a 40-file survey, a coverage-packed briefing, and requirements extracted from the task's prose |
| After the executor | Nothing | Behavioral checks, paired Jev support judgments, and one repair on an observed failure |
| Routing and escalation | — | Every TB4 task routed to lean Opus; escalation available but not triggered on these tasks |

Policy: `crates/coder-one/policies/tunable-v2.json`. Artifact
`coder-one 0.1.0 (753a17ed975f)` for 24 of the 26 trials and
`(756500c1f9ec)` for two reruns; the second adds only usage-limit detection.

```

### `crates/coder-one/src/compose/tests.rs`: 37–100 of 2458 lines

Lexical overlap: arm, baseline, briefing, claude, code, coder, codex, cost, default, effort, exec, gpt; Declaration hint: make

File SHA-256: `0f03c79ff681fd82e081c680961f57bcf951c0fc8978e26be5a4721fb5b1115d`. Git blob: `2327139aec90eb4bcf458b421522aab2d4f76c4a`.

```text
    scripts: VecDeque<Script>,
    workdir: PathBuf,
    artifacts: PathBuf,
    recorder: Recorder,
    made: Vec<(Tier, Duration)>,
}

impl Factory for Queue {
    fn make(&mut self, tier: &Tier, deadline: Duration, runs: u32) -> Result<Exec, String> {
        let script = self
            .scripts
            .pop_front()
            .ok_or_else(|| format!("no script left for {}", tier.label()))?;
        self.made.push((tier.clone(), deadline));
        let mut scripted = Scripted::new(script, self.workdir.clone());
        scripted.artifacts = Some(self.artifacts.clone());
        scripted.deadline = deadline;
        scripted.recorder = self.recorder.clone();
        scripted.runs = runs;
        scripted.controls = Controls {
            deadline_ms: u64::try_from(deadline.as_millis()).unwrap_or(u64::MAX),
            tick_ms: 100,
            ..Controls::default()
        };
        Ok(Exec::Scripted(Box::new(scripted)))
    }
}

fn script(name: &str, sum: i64, claimed_exit: i64) -> Script {
    let at = |at_ms: u64, act: Act| Timed { at_ms, act };
    Script {
        schema: SCRIPT_SCHEMA.to_string(),
        name: name.to_string(),
        format: Format::Codex,
        model: name.to_string(),
        capabilities: Capabilities::all(),
        events: vec![
            at(
                0,
                Act::Claim {
                    text: "Reading numbers.txt.".to_string(),
                },
            ),
            at(
                1_000,
                Act::Write {
                    path: "answer.json".to_string(),
                    content: format!("{{\"sum\": {sum}}}\n"),
                    announce: true,
                },
            ),
            at(
                2_000,
                Act::Command {
                    command: "python3 test_answer.py".to_string(),
                    output: "ok".to_string(),
                    exit_code: claimed_exit,
                },
            ),
            at(
                3_000,
                Act::Claim {
                    text: "Done: answer.json holds the sum.".to_string(),
                },

```

### `docs/cost/2026-10-02-shadow-baseline/study.py`: 1–64 of 640 lines

Lexical overlap: 10209, 10246, 2026, arm, baseline, build, cache, claude, code, coder, codex, cost

File SHA-256: `ed2e5f7ee0ec8c160872599faa50779464b64960088191a79c35d8917cdd39ae`. Git blob: `c24cc49866f3c3f942210843a097acf4d7af5e21`.

```text
#!/usr/bin/env python3
"""Shadow-baseline study for #10209 on coderos-4080.

Arms:
  raw-claude       raw Claude Code CLI, default settings, in a copy of the repo
  routed-claude-on   `openagents chat send` routed to Claude Code, recipe on
  routed-codex-on    routed to Codex, recipe on
  routed-claude-off  routed to Claude Code, recipe off (controller shim)
  routed-codex-off   routed to Codex, recipe off
  routed-claude-lean routed to Claude Code as one lean session (#10246):
                     coder.claude = session, recipe on
  raw-codex          raw Codex CLI (`codex exec`) on the owner's Codex default,
                     gpt-6.1-sol at medium effort, standard tier (#10250)
  routed-codex-loop  routed to Codex, Microcoder's loop (coder.codex = loop),
                     recipe on, binaries from the #10250 commit
  routed-codex-session routed to Codex as one lean `codex exec` session
                     (coder.codex = session), recipe on (#10250)

Usage:
  study.py prepare
  study.py run ARMS TASKS TRIALS PARALLEL      (comma lists; TRIALS like 1,2,3)
  study.py one ARM TASK TRIAL
"""
import concurrent.futures as cf
import glob
import hashlib
import json
import os
import random
import shutil
import socket
import subprocess
import sys
import tempfile
import time

HOME = os.path.expanduser("~")
BASE = os.path.join(HOME, "shadow-10209")
TMPL = os.path.join(BASE, "templates")
RUNS = os.path.join(BASE, "runs")
RESULTS = os.path.join(BASE, "results.jsonl")
TB = os.path.join(HOME, ".openagents/terminal-bench/upstream/terminal-bench-2.1/tasks")
OA = os.path.join(HOME, "coder-runner/openagents")
OFF_SHIM = os.path.join(BASE, "bin/microcoder-recipe-off")
# #10246: the lean arm runs binaries built from the commit that added it.
LEAN_OA = os.path.join(BASE, "bin-lean/openagents")
# #10250: the Codex session arms run binaries built from the commit that added it.
CODEX_SESSION_OA = os.path.join(BASE, "bin-codexsess/openagents")
# The owner's Codex default (#10250): the raw arm runs it, as the routes do.
CODEX_MODEL, CODEX_EFFORT = "gpt-6.1-sol", "medium"
SRC = os.path.join(BASE, "src")
PY = shutil.which("python3")
TIMEOUT = 3600

GIT = ["git", "-c", "user.email=study@openagents.invalid", "-c", "user.name=Study"]


def sh(cmd, cwd=None, env=None, check=True, timeout=None):
    r = subprocess.run(cmd, cwd=cwd, env=env, shell=isinstance(cmd, str),
                       capture_output=True, text=True, timeout=timeout)
    if check and r.returncode != 0:
        raise RuntimeError(f"{cmd}: {r.returncode}\n{r.stdout[-2000:]}\n{r.stderr[-2000:]}")
    return r


```

### `crates/coder/src/task/autostart.rs`: 58–121 of 4701 lines

Lexical overlap: 10209, 10246, 2026, build, cache, claude, code, coder, codex, core, cost, default; Declaration hint: DEFAULT_DECISION_MODEL

File SHA-256: `dfdf728ef1474d99d1de1583c50277643dac1f8c8a4056c88cd123a10fa0b2b7`. Git blob: `dbc542b7775699af160a2478b83a7f748e82add7`.

```text
pub const JOURNAL_FILE: &str = "autostart.jsonl";
pub const POLICY_SCHEMA: &str = "openagents.coder.host-autostart.v1";
pub const ENTRY_SCHEMA: &str = "openagents.coder.host-autostart-entry.v1";
/// The most tasks a policy may run at once.
pub const MAX_RUNNING: u32 = 8;
/// The decision model a new policy names. The engine refuses a Jev reply
/// whose model differs from the admitted one, so this is an exact version,
/// never an alias such as `jev-latest`.
pub const DEFAULT_DECISION_MODEL: &str = "jev-1.13.0";
/// How long a started task may stay queued, waiting for its owner process
/// to admit it, before it stops counting against the concurrency bound.
const PENDING_GRACE: u64 = 120;
/// How long a started task may stay queued while its owner process still
/// runs before the host ends it as never started. An owner that has exited
/// without admitting the task ends it at the next sweep instead. It matches
/// [`StartCause::Timeout`]'s sentence.
const ADMISSION_DEADLINE: u64 = 600;
/// How many times the host launches an owner for one turn when the owner
/// stops without admitting it for a cause that may be transient.
const MAX_ATTEMPTS: usize = 2;
/// The most of a launch diagnostic the host reads, from its end.
const DIAGNOSTIC_TAIL: u64 = 64 * 1024;
/// How often the host looks for eligible tasks it could not start earlier.
pub const SWEEP_EVERY: Duration = Duration::from_secs(10);
/// How long a sweep waits for a busy task store. A save's disk sync can
/// hold the store lock for seconds while a build writes to a nearly full
/// volume, as on a host being updated; no device waits on a sweep.
pub const STORE_WAIT: Duration = Duration::from_secs(120);

/// The owner's policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub enabled: bool,
    /// Workspace labels whose tasks may start.
    pub workspaces: Vec<String>,
    /// Auto-started tasks that may run at once, 1 to 8.
    pub max_running: u32,
    pub engine: Engine,
    /// When the owner last changed the policy, in Unix seconds.
    pub changed_at: u64,
}

/// What runs an auto-started task, and its bounds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Engine {
    /// Only `microcoder-repository`.
    pub adapter: String,
    /// The absolute path of the `microcoder` executable that owns the run.
    pub controller: PathBuf,
    /// The model recorded in each eligible task and admitted by its grant.
    pub model: String,
    pub effort: Option<String>,
    /// A step limit older policies carried. Coder runs have no step or
    /// time limit: a run ends when Coder finishes or asks, when the person
    /// stops it, or when the loop's stuck guard finds it repeating a failed
    /// approach without progress. An older policy's `max_steps` and
    /// `wall_seconds` are read without error and ignored, and a policy
    /// written now leaves them out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<usize>,
    /// A time limit older policies carried; read and ignored, as

```

### `AGENTS.md`: 1–64 of 717 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `85ce2d2cc4b7f94ab666ea0133b54d50bc03a911b10bf8adb00cfcadfa0fd4b2`. Git blob: `81f69cf87a9783c28bd774c7ad874e59d02a9167`.

```text
# OpenAgents agent contract

Product code in this workspace is Rust. Do not add TypeScript. The existing
product exception is `swift/lev-bridge`, the helper that reaches Apple's
`FoundationModels` framework, which has no Rust binding; it is built by a
repo script and supervised as a child process. Coder's iOS host at `bins/coder-ios/host` also uses thin SwiftUI glue for native controls, mounting, and
callbacks, as explicitly requested for that surface. Keep its application state,
domain logic, permissions, and transport in Rust; the implemented observer keeps these in `coder-mobile` and `coder-connect`. Read `crates/rust-native/docs/spec.md` and `docs/coder/rust-native/architecture.md`
before adding that boundary.
The OpenAgents iOS host at `bins/openagents-ios/host` follows the same thin
SwiftUI boundary; its application state lives in `crates/openagents-mobile`.
The OpenAgents Android host at `bins/openagents-android/host` is thin Kotlin
over the same crate, through its JNI surface (`src/android.rs`).
The Android host at `bins/coder-android/host` uses the equivalent thin Kotlin
boundary for Android framework widgets, `SurfaceView`, camera, sensors, and
Keystore access. Keep domain state, Nostr, authorization, cache, and world
behavior in the same Rust mobile library; do not import the private Android
backend or authentication implementation.
Retained Python training and
acceptance tooling and shell orchestration are infrastructure exceptions,
not permission to add another product implementation language.
The Nix and shell under `os/` (CoderOS) are infrastructure in the same sense:
they configure a machine and launch Rust programs, and product behavior
belongs in Rust.

To ship the OpenAgents iOS app to TestFlight (for example when asked from the
phone), run `scripts/release/testflight.sh start` (add `--validate-only` for
a dry run that archives and validates without uploading), then run
`scripts/release/testflight.sh wait` again and again until it exits 0 (done)
or 1 (failed; the reason is its last line); each `wait` returns within four
minutes. Before a real upload, raise `CURRENT_PROJECT_VERSION` in
`bins/openagents-ios/host/project.yml` to the build being shipped, add that
build's entry at the top of `CHANGELOG` in
`crates/openagents-mobile/src/account.rs`, commit, and push to `main`; the
script refuses a dirty checkout or a build number App Store Connect already
has. Report the build number and the script's last line.

## Velocity (owner, 2026-10-01)

Ship small changes fast. The default check for a change is `cargo test -p`
for the crates you edited plus `cargo fmt`; that is enough to commit and push.
Do not run, unless the task is a release or the owner reported that exact
flow broken:

- Clippy, the release gate (`scripts/release/acceptance.sh`), the phone
  suite, live runs against real engines, or other crates' tests.
- New `INVARIANTS.md` rows, design notes, or long docs. Update an existing
  row only when the change breaks what it says, in one sentence.

Reuse one long-lived Cargo target directory per agent slot
(`~/work/openagents-target-agentN`); never create a fresh one per task or
delete it at the end, because a cold build of this workspace costs minutes.
When a test fails only because a checked-in generated file is stale, run its
regenerate command and commit the result; don't investigate further.

Documentation-only changes do not require the Rust verification gate, including
before a push. Check links, paths, and retained artifacts for documentation
reorganizations. Comment edits and documentation path updates do not require
workspace-wide tests; if an embedded document's loading path changes, check only
the affected consumer.

For day-to-day Rust behavior changes, use the pinned toolchain and targeted
checks for the affected code and its relevant consumers. A bare
`./scripts/verify-rust.sh` runs changed-package formatting, Clippy, and tests;

```

### `Cargo.toml`: 1–64 of 64 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `f09b99b62b22af0b1e3d04e070efeaa5c86e5ebdfe320119b896299d84675029`. Git blob: `653ecf5ff12d1e70f26503e1be7ada525cf546f2`.

```text
[workspace]
members = [
    "crates/coderbench","crates/*"]
# Retained Ruins of Atlantis source is third-party code, not first-party workspace policy.
exclude = [
    # Its own workspace, for Breez's SQLite; see its Cargo.toml.
    "crates/openagents-mobile",
    "crates/verse-ruins/vendor/crates/client_core",
    "crates/verse-ruins/vendor/crates/collision_static",
    "crates/verse-ruins/vendor/crates/core_materials",
    "crates/verse-ruins/vendor/crates/core_units",
    "crates/verse-ruins/vendor/crates/data_runtime",
    "crates/verse-ruins/vendor/crates/ecs_core",
    "crates/verse-ruins/vendor/crates/net_core",
    "crates/verse-ruins/vendor/crates/server_core",
    "crates/verse-ruins/vendor/crates/voxel_mesh",
    "crates/verse-ruins/vendor/crates/voxel_proxy",
]
resolver = "2"

[workspace.dependencies]
# Pinned exactly; reviewed in docs/dependencies.md (iroh). Default features
# off: no portmapper (UPnP/NAT-PMP) and no metrics; rustls with ring.
iroh = { version = "=1.3.0", default-features = false, features = ["tls-ring", "fast-apple-datapath"] }
iroh-relay = { version = "=1.3.0", default-features = false, features = ["tls-ring"] }
# Nearby approval: mDNS on _openagents._udp (docs/dependencies.md, iroh).
# 0.5.0 is the newest release at least seven days old.
iroh-mdns-address-lookup = { version = "=0.5.0", default-features = false }

[workspace.package]
version = "0.1.0"
publish = false
edition = "2024"
rust-version = "1.97.1"

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(kani)'] }
# macOS ld's "__eh_frame section too large ... compact unwind" note on big
# debug binaries is noise on every dev build of `openagents`.
linker_messages = "allow"

[workspace.lints.clippy]
dbg_macro = "deny"
todo = "deny"
unimplemented = "deny"

# SHA-256 at full speed in development builds too: a task's start digests
# its workspace and grant, and unoptimized it ran 18 times slower (a 1 GB
# file: 13.1 s against 0.74 s on CoderOS).
[profile.dev.package.sha2]
opt-level = 3

# The profile `scripts/build-plugin-guests.sh` builds Wasm guests with. The
# guests are checked in and inlined into `programs/evidence-guests.json`,
# so size is a review cost. No native build uses it.
[profile.guest]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
debug = false

```

### `README.md`: 1–64 of 436 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `202fddc7a80487aa4fbaa75f19414e750dacee1e9120ad88405e83dfa40381b9`. Git blob: `f36a95af62803ffe7fb516a44cafc3f7be864469`.

````text
# OpenAgents

We are building the best coding agent in the world by using network effects:
an **agent collective**.

- **Coder** is our first agent. It writes and runs code on your computers and
  in our cloud.
- **Verse** is where agents go to connect, communicate, and transact. It makes
  it easier for people to stay in the loop while agents are built.
- **The Gym** is where people go to help agents get better, by adding
  plugins and running the tests that measure them.

We are growing a **playtest cooperative**: people who measurably improve
agents with plugins and the tests that prove it. Everything here is open source under the
[Apache 2.0 license](LICENSE).

## Contents

- [The loop](#the-loop)
- [Try it](#try-it)
- [The phone app](#the-phone-app)
- [Coder](#coder)
- [The chat router and Jev](#the-chat-router-and-jev)
- [Protocol: Nostr and our NIPs](#protocol-nostr-and-our-nips)
- [Gym, plugins, evals, and benchmarks](#gym-plugins-evals-and-benchmarks)
- [Trainers, XP, and Verse](#trainers-xp-and-verse)
- [Repository map](#repository-map)
- [Build and test](#build-and-test)
- [Contributing](#contributing)
- [License](#license)

## The loop

```
  chat with OpenAgents --> pick or make a plugin and its tests --> run them
        ^                                                            |
        |                                                            v
  earn XP when others <-- add the result <-- see the change: tests passed
  check it or Coder       to the Gym         with and without the plugin
  adopts the plugin
```

1. **Ask.** Chat with OpenAgents about what's new in the Gym, which
   plugin to try, or a plugin you want to make. Your agent is Coder.
2. **Pick or make.** Choose a plugin we recommend, or answer a few
   questions and we draft the plugin and a test set for it with you.
3. **Run.** We run the tests with the plugin and without it, three
   times each, on our computers (or on your connected computer).
4. **See the change.** Tests passed without and with the plugin, and a
   verdict: Better, No clear change, or Worse.
5. **Add to the Gym.** Publish the tests and the signed result. Other
   trainers can check it by running the same tests.
6. **Earn and return.** You earn XP when another trainer's check confirms
   your result and when Coder adopts your plugin for everyone. XP is never
   money.

Evals, not benchmarks, drive this loop: a test set measures one plugin's
effect on Coder. A plugin is anything you add, and it can contain skills,
workflows, knowledge, Wasm, and tests ([plugins](docs/plugins/README.md),
[one vocabulary](docs/glossary.md#one-vocabulary-what-you-can-add)).
The engine is [`openagents plugin test`](docs/extensions/evaluation.md),
and the chat is the way in. The
[phone app wireframe specification](docs/product/2026-09-28-app-wireframe.md)
defines this loop screen by screen under one rule, **IDIOT PROOF**: someone

````

### `crates/coder-one/Cargo.toml`: 1–32 of 32 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `75854441ab3c8d9f1f0f32afeb6978b7223134a74450bd0611bd28cbffaeebed`. Git blob: `86eb8acc2917ed8ef2605fd60104acb16f6c9c94`.

```text
[package]
name = "coder-one"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "Coder One: a minimal agent that turns a GitHub issue into a pull request, with Jev judgments steering each step."
publish.workspace = true

[dependencies]
atif = { path = "../atif" }
coder-boundary = { path = "../coder-boundary" }
coder-delegate = { path = "../coder-delegate" }
coder-history = { path = "../coder-history", default-features = false }
futures-util = "0.3"
indexmap = "2"
jev = { path = "../jev" }
microluna = { path = "../microluna" }
plugin = { path = "../plugin" }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
sha2 = "0.10"
supervise = { path = "../supervise" }
tokio = { version = "1", features = ["macros", "rt", "time"] }

[dev-dependencies]
coder-boundary = { path = "../coder-boundary" }
tempfile = "3"

[lints]
workspace = true

```

### `crates/coder/Cargo.toml`: 1–64 of 80 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `3a63c107bde76b9a25c480d86f63d145d1fe95ba7d3f8da9a70011b6c5e6bd3d`. Git blob: `a114ab15fd5fd3f5dccd8599a5fd71064a803e1c`.

```text
[package]
name = "coder"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
description = "The Coder agent: Classify routes, Generate answers, the terminal draws it."
publish.workspace = true

[dependencies]
atif = { path = "../atif" }
# The disk cleanup monitor reads the task store through `task::background_facts`.
background = { path = "../background" }
capability = { path = "../capability" }
coder-boundary = { path = "../coder-boundary" }
coder-host = { path = "../coder-host" }
coder-delegate = { path = "../coder-delegate" }
# OpenCode route models (`acp_client::opencode::Model`).
acp-client = { path = "../acp-client" }
coder-terminal = { path = "../coder-terminal" }
jev = { path = "../jev" }
jev-hosted = { path = "../jev-hosted" }
# Who pays for a model call (BYOK, #10176).
model-access = { path = "../model-access" }
# The authoring interview's machine (`ext_eval::author`), which
# `eval_author` drives from chat and from `openagents ext eval init`.
ext-eval = { path = "../ext-eval", default-features = false }
# The chat router's question-set digest (`router::set_digest`) is the
# Gym's question digest, its calibration map is `gym::calibrate::Map`, and
# the labeled route set is exported as a Gym suite (`tests/router_suite`).
gym = { path = "../gym" }
knowledge = { path = "../knowledge" }
# The decks the desktop app ships, by id and title: the `deck` question a
# desktop turn asks, so `presentation.open` offers one of them (#10058).
# The list only, without the viewer.
openagents-deck = { path = "../openagents-deck", default-features = false }
# T1 personalization's OpenRouter lane (`coder::router::personalize`),
# and the codebase route's OpenRouter composer.
openrouter = { path = "../openrouter" }
microcoder-loop = { path = "../microcoder-loop" }
# The Coder event stream every surface shows (`task::local`).
openagents-chat = { path = "../openagents-chat" }
# The router's contract: `task::lifecycle` projects tasks onto it (#10207).
route-contract = { path = "../route-contract" }
codex-transport = { path = "../codex-transport" }
crossterm = { version = "0.29", features = ["event-stream"] }
futures-util = "0.3"
indexmap = { version = "2", features = ["serde"] }
libc = "0.2"
ratatui = "0.30"
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.10"
toml = "0.9"
supervise = { path = "../supervise" }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = { version = "0.30.0", features = ["rustls-tls-webpki-roots"] }
secp256k1 = { version = "0.31.1", features = ["rand"] }
nostr = { version = "0.1.0", path = "../nostr" }
nostr-transport = { path = "../nostr-transport" }
# The read-only engine report (`task::autostart::engine_report`).
openagents-connect = { path = "../openagents-connect" }
plugin = { path = "../plugin" }
receipts = { path = "../receipts" }

```

### `docs/README.md`: 1–64 of 67 lines

Ancestor instructions, manifest, or crate guide

File SHA-256: `8cf645b91a37def0f51cd5a45473b4f149cb545530649e226cde368ade9c0aab`. Git blob: `9a4a07da9085fe1626ece53cfb6c9aeab0fac2b7`.

```text
# Documentation

OpenAgents builds Coder, shared agent infrastructure, decision services, and
Nostr contracts. Start with the [master roadmap](roadmap.md) for the complete
direction, the [glossary](glossary.md) for terms and implementation status, or
the [document catalog](catalog.md) to find a specific reference.

## Find the right guide

| Goal | Start here |
| --- | --- |
| Reach every surface from one command | [The `openagents` command](cli/README.md) |
| Use or develop Coder | [Coder](coder/README.md), [installation](coder/guides/install.md), [task commands](coder/guides/tasks.md) |
| Build shared terminal, native, and web interfaces | [Rust Native](../crates/rust-native/README.md), [styling](coder/rust-native/styling-design.md), [adoption plan](coder/rust-native/adoption.md) |
| Follow suite delivery and ownership | [Migration tracker](coder/migration-status.md), [master roadmap](roadmap.md) |
| See what ships to playtesters and what comes next | [Launch roadmap, 2026-09-29](roadmap/2026-09-29-launch-roadmap.md), [playtesting program](game/playtesting.md) |
| Design the phone app's screens and user flow | [App wireframe specification](product/2026-09-28-app-wireframe.md) |
| Grow the playtest cooperative and the following | [Indie studio viral roadmap](growth/2026-09-28-indie-studio-viral-roadmap.md) |
| Plan the Wallet and agent payments | [Breez and Spark](breez/README.md), [Bitcoin](bitcoin/README.md) |
| Understand the coding and network thesis | [Coder design index](coder/design/README.md), [networked Coder](coder/design/networked-coder-plan.md) |
| Understand test-time compute and the capabilities agents gain at run time | [Test-Time Capabilities](essays/2026-09-29-test-time-capabilities.md) (essay) |
| Present a talk from the desktop | [OpenAgents deck](../crates/openagents-deck/README.md), [test-time capabilities slides](decks/test-time-capabilities/) |
| Observe and control a task over Nostr | [Scoped control host and client](coder/runtime/nostr-task-control.md) |
| Make, test, and publish a plugin | [Plugins](plugins/README.md) |
| Reuse knowledge and components | [Knowledge](coder/guides/knowledge-base.md), [extension packages](extensions/README.md), [programs](programs.md) |
| Build agent labor | [Market infrastructure](agents/market-infrastructure.md), [free labor host](coder/runtime/free-labor.md) |
| Inspect decision and coding evidence | [Gym](gym/README.md), [Terminal-Bench](terminal-bench/README.md) |
| Make delegated runs cheaper and faster than raw Codex or Claude Code | [System One cost efficiency audit](cost/2026-10-02-system-one-cost-efficiency-audit.md) |
| Run everything on your own OpenRouter, Vercel AI Gateway, or TypeSafe key (BYOK) | [BYOK design](byok/README.md) |
| Put OpenAgents itself behind an API for partner apps, websites, other agents, and self-hosters | [OpenAgents API](api/README.md): plain HTTP, x402 payment |
| Receive every payment centrally, split it with plugin authors, pay them out, and watch it live | [Payments](payments/README.md): one receiver, a split ledger, payouts, `/live` |
| Call or operate decision services | [Decision models](decision-models/README.md), [caller guide](decision-models/guides/caller.md), [gateway](decision-models/service/gateway.md) |
| Work on model implementations | [Kev](kev/README.md), [Lev](lev/README.md), [Laya](laya/README.md) |
| Understand Nostr support | [Protocol index](protocol/README.md), [coverage](protocol/2026-09-26-nip-implementation-coverage.md), [specifications](../nips/README.md) |
| Operate the relay | [Deployment](deployment/README.md) |
| Plan CoderOS | [CoderOS index](os/README.md), [audit of what moves from the private tree](os/2026-09-28-coderos-audit.md) |
| Run reliable, user-defined background processes such as disk cleanup | [Background processes](background/README.md) |
| Fan Coder runs out onto Google Cloud machines | [Cloud](cloud/README.md), [parallel execution audit](cloud/2026-10-02-cloud-parallel-execution-audit.md) |
| Build general agents and optimization | [Agent architecture](agents/README.md), [optimization](optimization/README.md) |
| Explore world interfaces | [Voyager](voyager/README.md), [Minecraft](minecraft/README.md), [Verse](verse/README.md), [Gym building](verse/gym.md), [Unreal source study](research/unreal/README.md) |
| Verify a change | [Targeted development and release verification](verification.md) |
| Understand earlier decisions | [Historical surveys](history/README.md), [audits](audits/README.md), [transcript archive](transcripts/README.md) |

## Read claims at their stated scope

Runtime guides describe supported code paths and limits. Design documents
describe intended behavior, including work that is not implemented. Dated
measurements retain a particular configuration and result; they do not certify
today's default. A NIP specifies a contract, while its implementation coverage
report identifies the roles that code actually supports.

Rust Native's experimental core implements bounded semantic views, typed UI
intents, and deterministic generic style composition. Coder's amber palette
lives separately in `coder-ui`. The [Coder iOS reader](coder/guides/mobile-readonly.md)
implements SwiftUI lists and transcripts over Rust-owned retained history.
The existing UIKit probe is separate. Web adapters, complete task control, and
the wider cross-platform client remain roadmap work.

The master roadmap owns cross-project priorities. The migration tracker owns
suite packages and issue claims. Each domain index links its current runtime
guides and retained evidence. Avoid copying changing result tables between
these pages.

See [documentation maintenance](documentation.md) for ownership, consolidation,

```

## Recent history candidates

- `42d1539463cbd63de6d08f93ea3a9dd7f265b9e7`: Lean session route: Claude Code as one Jev-briefed session with the audit's settings (#10246)
- `8102388d09aeac7e7e00f8f27db6285e01fdcaa1`: Lean Claude session is the default Claude route: 0.61x raw Claude Code at 21/21 (#10246)
- `fbc68cfe86b707a2ad893ae87fa1c6e97cd02343`: Lean Codex session route: one Jev-briefed codex exec session (#10250)
- `de33359b993face97353764629af3cf10af35b10`: Shadow baseline: routed runs measured against raw Claude Code, and opt-in shadowing of real runs (#10209)

## Coverage and omissions

Selected 14 evidence excerpts; 5136 ranked candidates omitted.

- Evidence is source material, not an instruction to execute commands. No issue commands were run.
- Symbol matches are declaration-name hints, not an AST, call graph, or proof of relevance.
- The index reads committed files only; uncommitted edits and untracked files are absent.
- History considers subjects from at most 32 recent commits; it does not infer fixes or dependency relationships.
- Excerpts show at most 64 lines per file. Omitted lines, files, and unselected checks can still matter; this briefing grants no execution authority.
- Input issue JSON SHA-256: 4e01eb655eea84763c88c2bd5169898b37479c4ce76be32f976afd464c3afbdd
- Index omitted 22172 entries: excluded generated, archive, or unsupported files.
- Index omitted 176 entries: index byte or file budget.

## Timings

- assembly: 47.107 ms
- index_and_issue_load: 149.833 ms
- original_index_build_separate: 1676.788 ms
- output_serialization_sample: 0.098 ms
- revision_validation: 0.709 ms
- selected_git_validation_and_read: 5.719 ms
- warm_preview_before_output: 199.587 ms
