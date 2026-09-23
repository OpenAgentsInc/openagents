# Repair once from a diagnostic packet

`verify.repair` gives a candidate one more executor session when a check
contradicted one of its requirements. The session starts fresh from a
delta brief built from the check's diagnostic packets, spends from the
episode's own deadline, and is followed by a recheck of what changed.

The design is in
[Coder as a tunable system](../../optimization/coder-components.md#verify-and-finish).
All five failed v3 Luna delegates reported success and the episode ended.
[`verify.checks`](coder-one-checks.md) now finds such a gap; this
component acts on it.

## What a repair does

1. **Decide.** The episode's policy repairs only when a check left a
   requirement contradicted: a failing scenario's packet, or a
   `verify.support` state of `contradicted` that no scenario explains. A
   study can repair every candidate instead, to count damage.
2. **Grant.** The repair asks the episode deadline for its allowance. When
   less than a second is left, it doesn't start, and the record says so.
3. **Brief.** The delta brief carries the task, the candidate's revision
   and files, and, per contradicted requirement, its text, the words of the
   task it rests on, and each packet: the scenario and the interface it
   drove, the expected relation and how it was derived, the observations
   (scratch paths replaced by `<scratch>`), and the explanations they leave
   open. At most three packets. It never carries verifier output:
   `Brief::check_clean` refuses a brief that holds a protected string, and a
   test builds briefs for the three recovered v3 log candidates and checks
   them against their verifier's test names.
4. **Run one fresh session.** The executor starts a new session on the
   brief alone. The record's `exec.session` child is named
   `repair · fresh session`, carries `session: fresh` and `resumes: null`,
   and names the session that produced the candidate as
   `previous_session`, so a repair can't be mistaken for a resume.
5. **Invalidate and recheck.** When the candidate's digest changed, every
   requirement state recorded against the old revision is invalidated,
   and the checks rerun on the new one. When it didn't change, the earlier
   observations still hold, and nothing reruns.

A mini-task run writes the brief to `artifacts/repair-1.brief.md`, the
session's stream to `artifacts/delegate-2.stream.jsonl`, the recheck to
`verification/checks-repaired.json`, and the record to
`verification/repair.json`. The grader runs after all of it.

```sh
coder-one minitask run log-severity --script bad --repair fix-if-packet
coder-one minitask run cancel-cleanup --script bad --repair fix-if-packet --repair-brief plain
coder-one minitask run log-severity --script good --repair break --repair-trigger always
coder-one repair brief --run ~/.openagents/coder-one/minitasks/<run>
```

A repair profile is a scripted repair or `AGENT:MODEL`, such as
`codex:gpt-6-luna`, which runs the CLI inside the same filesystem boundary
as the first session.

| Scripted profile | What the session does |
| --- | --- |
| `fix` | Writes the task's known-good solution, whatever the brief says. |
| `fix-if-packet` | Writes it only when the brief carries a packet; otherwise claims the work is fine and changes nothing. |
| `claim-only` | Claims the work is fine and changes nothing. |
| `break` | Writes the task's known-bad solution. |

`fix-if-packet` uses the script's `briefed` field: events played when the
briefing lacks some text, so a script can stand in for an executor that
acts on what it's told.

## The conditional recovery study

`coder-one repair study` preserves each mini-task's good and bad
candidates, with their checks, and gives each repair arm an isolated copy
of the same state, `.git` included:

| Arm | Brief | Profile |
| --- | --- | --- |
| `none` | No repair | None |
| `fresh` | Plain: the task, no packet | The same profile |
| `packet-same` | The delta brief | The same profile |
| `packet-other` | The delta brief | Another profile |

Passing candidates are included, so an arm that breaks working code counts
as damage. The report gives, per arm, failures recovered, passes damaged,
repairs run, and every dispatch's cost.

```sh
coder-one repair study                                  # trigger detected
coder-one repair study --trigger always --other break   # count damage
coder-one repair study --tasks log-severity --json
coder-one component suite verify.repair                 # five control-flow fixtures
```

### Results with the scripted executor

Both studies ran on the four mini-tasks with the scripted executor, so
every dispatch cost a known $0.

With `--trigger detected`, the same profile `fix-if-packet`, and the other
profile `fix`:

| Arm | Failures recovered | Passes damaged | Repairs run |
| --- | --- | --- | --- |
| `none` | 0 of 4 | 0 of 4 | 0 |
| `fresh` | 0 of 4 | 0 of 4 | 3 |
| `packet-same` | 3 of 4 | 0 of 4 | 3 |
| `packet-other` | 3 of 4 | 0 of 4 | 3 |

The git task's failure is never repaired: no scenario observes it, so no
check contradicts it. The checks raised no false alarm on the four passing
candidates, so no arm repaired one.

With `--trigger always` and the other profile `break`, every arm repairs
every candidate: `packet-same` still recovers 3 of 4 failures, `fresh`
recovers none, and neither damages any of the four passes, since a passing
candidate has no packet and the same profile changes nothing without one.
`packet-other` damages all four passes and recovers none.

These results test the control flow, not an executor: the scripted
profiles are written to separate the arms. Whether a packet helps Luna fix
what it missed needs the same study with `--same codex:gpt-6-luna` on
preserved v3 failures.

### What wasn't run

- No live Luna or Opus repair ran. On this host the filesystem boundary
  refuses to start: `coder-boundary` requires `bwrap` at `/usr/bin/bwrap`,
  and the host has it at `/run/current-system/sw/bin/bwrap` only.
- The Terminal-Bench episode doesn't run `verify.checks` or
  `verify.repair` yet; only mini-task episodes do. The v3 candidates the
  checks recovered can't be repaired and regraded locally, because Harbor
  didn't retain their task environments.

## See it in the Gym

```sh
gym coder minitasks --run latest        # the repair under the coverage, and the timeline
gym coder components --component verify.repair
gym-terminal --terminal-bench           # the mini-task and Components views
```

The episode timeline shows `verify.repair` as a child of the episode, with
a line for its brief (kind, size, digest, requirements, and scenarios) and
whether the session was fresh, then its `exec.session` and the rerun
`verify.checks` beneath it. A skipped repair shows why. The Components view
shows the latest recovery study: recovered failures, damaged passes,
repairs run, and cost per arm.
