# Environment facts in the briefing, measured offline

2026-09-25. Issue
[#9632](https://github.com/OpenAgentsInc/openagents/issues/9632), change 1 of
[Microluna v18](../coder/design/microluna-v18.md).

**On 577 retained sessions, 267 commands before the first edit failed on a
missing program or file, and the environment line would have prevented 60
of them (22%, 18–28%).** For Microluna the share is 18 of 20 (90%, 70–97%):
nearly every Microluna miss was `python` or `git`, which the line names.
Claude Code and Codex missed mostly on programs outside the probe set, such
as `file`, `xxd`, `ps`, and `curl`, and on files that didn't exist yet.
This is an offline count on retained logs. It makes no live Terminal-Bench
claim, and the switch is off in every manifest.

## What was built

- **`evidence.environment`** (`crates/coder-one/src/environment/mod.rs`).
  The probe planner adds one typed operation,
  `Operation::Presence { program }`, per program: the fixed set `python`,
  `python3`, `pip`, `git`, `make`, `node`, `cargo`, `pytest`, and
  `docker`, then the programs the task's files imply, such as `coqc` for a
  `.v` file, `g++` for a `.cpp` file, and `sqlite3` for a `.db` file. The
  host runs them in the task container, where it runs itself. Each looks
  the program up on `PATH` natively, without a shell, and runs a program
  it finds with its version argument under a 5-second bound. The runner
  refuses a program outside the table, so no task text reaches an
  argument vector.
- **One briefing line.** The captures become, for example:

  ```text
  Available: python3 3.12.3, pip 24.0. Absent: python, git, make.
  ```

  The template is a component parameter (`environment::Params`: the line,
  the entry format, the separator, and the empty-list word). It's digested
  into the component's implementation with the fixed set, the implied
  table, and the bounds. A template that drops either list or the version
  is refused. Jev isn't asked; presence is a fact, and the presence
  captures skip the probe keep question.
- **Never trimmed.** The line enters the survey first under the label
  `environment (presence probes)`, and each packer enforces that it
  arrives whole:
  - The coverage packer (`pack::pack`) doesn't rank or slice it. It goes
    in the fixed part as an `## Environment` section before the task text,
    where the cap can't reach it. The pack record marks it `complete`, and
    Jev's coverage questions skip it.
  - The first packer (`Briefing::build`) reserves the line's room before it
    places the task text or any other item, and always places the line.
  - Microluna's per-group evidence (`micro::evidence_for`) puts the line
    first, whole, in every group's evidence, outside the character budget.
- **The switch.** `policy.evidence.environment` holds the template; `{}`
  turns it on with the default. It needs `evidence.probes`. It's absent
  from every manifest, so no digest changed, and the planner's parameters
  serialize as before when it's off. No manifest was added; the v18
  close-out adds one.
- **Tests and fixtures.** Packer tests at caps from 1,000 to 40,000
  characters with a task long enough to be trimmed; an evidence test at
  budgets 0, 500, and 12,000; planner, policy, and operation tests; and
  two `evidence.environment` component fixtures: one reconstructed from the
  `microluna-v13-retained` trials on `embedding-drift-monitor` (Python
  3.12.3 present, `python` and `git` absent, the other programs left out
  because no retained record shows them), and one synthetic Coq task.

## How the count was made

`coder-one environment measure` reads every Microluna session log
(`microluna-*.atif.jsonl`) and every Coder One episode log
(`episode.atif.jsonl`) under the roots it's given. An episode log holds the
normalized events of each executor session it dispatched, whether Claude
Code, Codex, or Microluna. A Microluna session is read from its own log
when that log was retained beside the episode, and from the episode's
events otherwise. A log copied into several directories counts once by
content, and a session counts once by key.

- **Before the first edit.** Commands up to a session's first successful
  `apply_patch` or `write_file` call (Microluna), or its first
  `artifact_changed` event (the executor events). A session that never
  edited contributes all of its commands.
- **A miss.** A command with an output line that is a shell's `not found`
  or `command not found`, or a `No such file or directory` diagnostic,
  counted once per command. The exit code doesn't decide it: `python …;
  true` exits 0 and still missed. 91 of the 267 misses exited non-zero.
- **Prevented.** The miss names a program that the presence probes ask
  about: the fixed set, or a program the session's files imply. The error
  itself shows the program was absent in that container, so the line
  would have listed it as absent.

Roots: this repository's `bench/`, `~/.openagents`, and the other
checkouts and worktrees on this host, which hold copies of the same
traces. The eight tasks of #9584's sealed cohort are excluded; that removed
5 sessions. 2,514 log files were read, 2,106 of them byte-identical copies.
The fixed set is the issue's, and the implied table was written before the
count ran, so nothing here was tuned on the data.

## Results

| Agent | Sessions | Commands before the first edit | Misses (`not found` + `No such file`) | Sessions with a miss | Prevented | Share of misses prevented | Sessions with a prevented miss |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Claude Code | 374 | 5,065 | 210 (104 + 106) | 129 (30–39%) | 27 | 13% (9–18%) | 27 (5–10%) |
| Codex | 161 | 863 | 37 (30 + 7) | 31 (14–26%) | 15 | 41% (26–57%) | 15 (6–15%) |
| Microluna | 42 | 114 | 20 (19 + 1) | 17 (27–56%) | 18 | 90% (70–97%) | 16 (25–53%) |
| All | 577 | 6,042 | 267 (153 + 114) | 177 (27–35%) | 60 | 22% (18–28%) | 58 (8–13%) |

Intervals are 95% Wilson intervals. The share of misses treats misses as
independent, which they aren't within a session, so read it as
descriptive.

By the split frozen in the
[executed-checks protocol](../../bench/terminal-bench/experiments/2026-09-25-executed-contract-checks/protocol.md):

| Tasks | Tasks seen | Sessions | Misses | Prevented | Share prevented |
| --- | ---: | ---: | ---: | ---: | ---: |
| Development | 12 | 162 | 94 | 31 | 33% (24–43%) |
| Held out | 11 | 208 | 62 | 15 | 24% (15–36%) |
| Every other task | 39 | 207 | 111 | 14 | 13% (8–20%) |

The names the preventable misses carried: `python` 23, `python3` 14, `git`
14, `pip` 6, `pytest` 2, and `sqlite3` 1. The implied table added one
prevented miss beyond the fixed set: `sqlite3` on `sqlite-db-truncate`.

The misses the line can't prevent are of two kinds:

- **Names outside the set.** `file` 21, `xxd` 15, `ps` 14, `curl` 12,
  `strings` 10, `cmdline` 9 (a `/proc` path, not a program), `pkill` 8,
  `column` 8, `bc` 6, and 14 others with 4 or fewer. Of these 126 names, Claude Code's misses carried 104, Codex's 21,
  and Microluna's 1.
- **Files that don't exist.** Paths the session expected before it created
  them, such as `/tmp/persist-tests` (23 misses across 12 tasks), and missing
  shared libraries, such as `libGL.so.1` (15).

### A check against the account of the v13 trials

The [step-by-step account](2026-09-25-microluna-v13-embedding-trials.md) of
the three `microluna-v13-retained` trials on `embedding-drift-monitor`
counted one `python` turn in each of the six sessions and two `git`
failures. The count here finds, in those six sessions before their first
edit, 9 misses, 8 of them preventable: 7 `python` and 1 `git`. The ninth
is `file: not found`, in a command that also ran `git status` with its
errors sent to `/dev/null`, so its output shows no `git` failure.

### Edited sessions only

327 sessions never edited; their whole session counts. Restricted to the
250 sessions that edited, there were 97 misses before the first edit, and
34 were preventable (35%). By agent: Claude Code 59 and 10, Codex 24 and
11, and Microluna 14 and 13.

## What wasn't done

- **Mini-tasks matched against `microluna-v15`.** The `coder-one minitask`
  runner doesn't run the probe battery, so the line never reaches a
  mini-task session, and it runs commands on this macOS host rather than in
  a task container, where the presence facts differ. Running the mini-tasks
  would need the runner to plan and run the probes first. No mini-task was
  run, and no turn or second counts to the first edit are reported.
- **No live run.** The switch ships off. The v18 manifest turns it on, and
  it's measured with the rest of v18.

## Limits

- "Prevented" is an upper bound. It counts a miss the line would have
  named, not a turn the session would have saved: a session told that
  `python` is absent can still run it.
- A session's shell can have a different `PATH` from the host process that
  runs the probes, such as a login shell. The retained logs don't record
  either.
- The implied programs come from file names in each session's first message
  and its commands before the first edit, because the retained logs don't
  hold the host's view of the workspace.
- The classifier reads diagnostics by their shape. A `No such file` line
  that a program printed about its own input counts as a miss, which is
  what the issue asks for, but it names no program.

## Decision

Admitted to the v18 manifest, off by default until then. The line prevents
most of Microluna's misses before the first edit at no Jev cost and one
short line of briefing, and the probes run in parallel inside the existing
probe grant. Its effect on turns and seconds to the first edit is still
unmeasured.

## Records and reproduction

Records are in
[`bench/terminal-bench/experiments/2026-09-25-environment-facts/records/`](../../bench/terminal-bench/experiments/2026-09-25-environment-facts/records/):
`summary.json`, with totals by agent and by task, and `sessions.jsonl`, one
row per session with its misses, the names they carried, and each miss's
command and diagnostic. To reproduce:

```sh
coder-one environment measure \
  --root bench --root ~/.openagents \
  --exclude distributed-dedup --exclude formal-crypto \
  --exclude freecad-impeller --exclude freecad-spring-clip \
  --exclude math-eval-grader --exclude pretrain-shard-corruption \
  --exclude shadow-relay --exclude vpp-loss-divergence \
  --rows sessions.jsonl > summary.json
```

The records were made with additional roots for the other checkouts and
worktrees on this host (`~/work/openagents/bench`,
`~/code/openagents/bench`, and `~/.codex/worktrees`). They hold copies of
the same traces, and deduplication counts each session once.
