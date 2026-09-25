# The delegate door

Coder Terminal answers a turn by briefing an executor with what Jev chose
from the workspace, the way Coder One answers a Terminal-Bench task. By
default the executor is Microluna: short GPT-6 Luna sessions in this
process, on the operator's Codex login, with Jev choosing each move
between them. Without a usable Codex login it is Claude Code, then Codex
CLI. The Open Responses door, Gemini today, answers only when this machine
has no delegation target.

Status: implemented in `crates/coder` (`delegate_door.rs`, wired through
`agent.rs` and `generate.rs`) on top of `coder_one::terminal`. The terminal
and `coder -p` both reach it, because both run `coder::turn::run`.
Issue [#9578](https://github.com/OpenAgentsInc/openagents/issues/9578)
holds the plan.

## Why this exists

Terminal-Bench measured the Jev probe battery plus a strong executor
beating a model that explores on its own. See
[the component study](../../optimization/coder-components.md) and
[`docs/terminal-bench/`](../../terminal-bench/). Coder Terminal still
streamed every turn from the Gemini door, so the interactive agent did not
use what the benchmarks proved.

## One turn

```text
probe    the host runs the read-only probe battery in the working directory
judge    Jev reads the request, keeps the probe outputs worth handing on,
         and ranks up to 40 candidate files
brief    code packs the request, the conversation, and the kept evidence
         into a briefing of at most 12,000 characters
delegate the executor answers the briefing inside a coder-boundary boundary
```

Every step is Coder One's library: `JevJudge::survey` for the probes and
judgments, `Briefing::build_under` for the briefing, and
`Cli::execute_watched` for the adapter. Coder does not keep a second copy.
The configuration is the reference policy
`crates/coder-one/policies/jevprobe2-opus-lean-low-5m.json`: Claude Code on
Opus 5.5 at low effort, six tools, the five-minute prompt cache, and a
600-second deadline. The trace records that choice, so the router work in
issue #9569 can replace it per turn later.

Coder One's judge and recorder are not `Send`, so the door runs the turn on
a thread of its own with a current-thread runtime and hears its progress
over a channel. Coder One's progress lines, which an episode prints to
standard output, go to the door through `coder_one::say` instead, so the
terminal keeps its screen and `coder -p` keeps standard output for the
reply.

## Which door answers

`Door::from_env` is unchanged, and `coder-worker` still builds its door
with it. The terminal and `coder -p` choose through
`delegate_door::open`:

| Setting | Effect |
| --- | --- |
| `CODER_DELEGATE=auto` | The default. Delegate to Microluna when the Codex login has more than ten minutes left on its access token. Otherwise delegate to an installed and authenticated `claude`, then `codex`. Otherwise fall back. Each choice says why. |
| `CODER_DELEGATE=always` | Delegate, or refuse to start. |
| `CODER_DELEGATE=off` | Never delegate. |
| `CODER_DELEGATE_AGENT` | `microluna`, `claude-code`, or `codex`: consider only that target. |
| `CODER_DELEGATE_MODEL` | The target's model. Unset, Microluna and Codex run `gpt-6-luna`, and Claude Code runs `claude-opus-5-5`. |
| `CODER_WORKER`, `CODER_EXECUTOR` | An explicit request for the relay or a local executor. It outranks `auto` and contradicts `always`. |

Microluna is available when `~/.codex/auth.json` (or
`$CODEX_HOME/auth.json`) is a ChatGPT sign-in whose access token has more
than ten minutes left. It runs in this process, so it needs no binary. The
door only reads the login; it never refreshes it and never prints a token.
When the token is near expiry, the header and `coder doctor` say so, and
the door falls back to Claude Code; any Codex command refreshes the login.

A CLI target is available when its binary is found (`CODER_ONE_CLAUDE_BIN` or
`CODER_ONE_CODEX_BIN`, then `PATH`, then `~/.local/bin`) and a credential
is: `CLAUDE_CODE_OAUTH_TOKEN`, an Anthropic API key, or the CLI's stored
login for Claude Code; `~/.codex/auth.json` or `OPENAI_API_KEY` for Codex.
An Open Responses key such as `CODER_DOOR_KEY` names the fallback rather
than asking for it.

Before this door, `CODER_DELEGATE` named the capability a program's
`delegate` step hands work to. It still does when its value is none of
`auto`, `always`, and `off`.

The session says which door answers and why in three places: the first
line the terminal draws and the first line `coder -p` writes to standard
error, the trace's session header (`door: delegate`, `model:
claude-code/claude-opus-5-5`), and a `door` step that holds the reason.
`coder doctor` reports the same choice without running a turn. See
[installing Coder](../guides/install.md).

## What the executor may do

The turn's permit decides, and nothing the executor says widens it.

| Permit | Boundary |
| --- | --- |
| Runs no commands: a clarifying turn, or `CODER_SHELL=off` | Read-only. The executor may write only its own state (`~/.claude`, `~/.claude.json`, `~/.codex`, `~/.cache`), the turn's artifacts directory, and the temporary directory. The workspace stays read-only even when it sits inside one of those. |
| Runs commands | Workspace-writable. The executor may also write inside the working directory, and nowhere else. |

The boundary is `coder-boundary`'s: `bwrap` on Linux, `sandbox-exec` on
macOS. A host that cannot enforce one fails the turn rather than run the
executor unbounded. Delegation stays a host decision: the model never sees
a delegate tool, and the door is chosen before a turn generates a word.

## A Microluna turn

A Microluna turn runs the same probes, survey, and briefing, then Coder
One's mini-handoff loop (`coder_one::micro`,
[the Microluna design](../design/microluna.md)) instead of a CLI:

1. Jev's requirement map is split into at most three groups.
2. Each group gets short sessions whose context is rebuilt from scratch:
   the request and the conversation, the group's requirements with only
   the evidence that informs them, then the current state.
3. After each session, Jev picks the next move (`next`, `retry`, `stuck`,
   or `done`) over the session's report and the combined verdict, and
   code keeps the last word. A turn is bounded at six sessions, $0.25, and
   600 seconds.

A request whose map has no requirements runs one session on the briefing.
The reply is each group's last answer, in order. A request Jev reads as a
question runs one session with no survey, whatever the manifest says.

### Which manifest a Microluna turn runs

The loop's shape and bounds come from a policy manifest in
`crates/coder-one/policies/`, the way a Terminal-Bench episode's do. The
manifests are compiled in, and each has a digest.

| Turn | Default manifest | Variable that names another |
| --- | --- | --- |
| A change request | `terminal-microluna.json`: the requirements loop above | `CODER_TERMINAL_POLICY` |
| The issue flow's loop | `issue-flow.json`: the requirements loop with larger bounds | `CODER_ISSUE_POLICY` |

A variable holds a reference manifest's file name, such as
`issue-flow-lean.json`, or the path of a manifest file. The manifest must
be valid and name the `microluna` agent with an `executor.microluna`
section; otherwise the turn keeps the default and says why. The manifest's
judge, briefing cap, deadline, model, and loop all apply. A read-only turn
still runs no checks, and a question or the issue flow's review still runs
one session under the terminal's default bounds.

`issue-flow-lean.json` runs the issue flow on the lean loop that
Terminal-Bench runs (`coder_one::micro::lean`): up to four work sessions,
a frozen evaluation script that keeps the best-scoring workspace, finishes
turned back while that score is below full, a spend bound per session, a
300-second bound per command, retained candidates, and a read-only review.
It isn't the default. The [stopped development comparison](../measurements/2026-09-25-issue-flow-policies.md)
found 1/4 passing artifacts per policy and a 17.7% cost saving for lean,
below the registered 20% threshold for a tie. Requirements remains the default. On a checkout above the workspace copy
bound (20,000 files or 256 MiB), keep-best and candidate retention can't
copy the workspace, so the loop keeps its last workspace instead.

`coder doctor` names each manifest, its digest, and its loop, and each
turn's trace records the manifest it ran as a system step.

The permit maps to Microluna's own isolation. A read-only turn runs every
command in a read-only `coder-boundary` boundary and refuses `apply_patch`
and `write_file`; it also runs no checks between sessions, because a check
may rerun a test command, which writes. A workspace-writable turn runs
commands in a boundary that may write only the working directory, and
runs the checks between sessions.

## A turn that works an issue

When a request names a GitHub issue to work, such as "work on #9597", or
"do it" after a reply that proposed one, the turn runs the issue flow in
`coder_one::issue_turn` instead of editing your checkout:

1. Code collects the issue references in the request and the
   conversation, and Jev chooses the one the request asks to work, or
   `none`. A request that names no issue asks Jev nothing, and a question
   about an issue, such as "summarize #9597", gets `none`.
1. The host reads the issue with `gh`, clones its repository fresh under
   `~/.openagents/coder-one/runs/`, and creates a `coder/issue-N-…` branch.
1. The Microluna loop that `issue-flow.json` names works the issue in the
   clone, with larger bounds than a change request's: up to 10 sessions,
   $1.00, and 40 minutes. `CODER_ISSUE_POLICY` can name another manifest,
   such as `issue-flow-lean.json`.
1. One review session reads the diff and the code that uses what changed,
   and the host's gate runs the tests and checks the change's figures,
   links, style, plain language, and dependent code. Up to two fix rounds
   follow when the gate still finds problems. The review and the gate run
   whichever loop the manifest names. The gate's tests may be ones the
   model wrote, so each `cargo test` runs inside the `coder-boundary`
   write boundary, writing only to the clone, the shared Cargo target
   directory, and its own scratch directory, through `supervise` with a
   1,200-second deadline and capped output. Every credential is withheld,
   along with `GH_*` and `GITHUB_*` variables, the operator's `gh` login,
   and Git's credential helpers. The network stays on unless
   `CODER_ONE_GATE_NETWORK=off`. The tests can't write Cargo's cache, so
   the host runs `cargo fetch` in the clone first. A host that can't build the boundary
   prints a warning and runs the tests without it, credentials still
   withheld. The pull request body says how the tests ran.
1. When the loop finishes with changes, the host commits them, pushes the
   branch, and opens a draft pull request that closes the issue. A run
   that doesn't finish leaves its changes staged in the clone.

Every step streams into the terminal as it runs. The flow runs only when
the operator's permit runs commands.

## What the terminal shows

The executor's events become the turn's own events, so a delegated turn
draws the way a generated one does:

| Executor event | Turn event |
| --- | --- |
| Text the executor said | `Delta`, the reply streaming in |
| A command started | `Shell(Proposed)` |
| A command finished | `Shell(Ran)`, with its exit code and output |
| A file written or edited | `Judgment`, such as `update ▸ src/lib.rs` |
| A probe, survey, or briefing line | `Judgment`, such as `survey ▸ Jev rated 40 files …` |
| A Microluna session starts | `Judgment`: `microluna ▸ session 2 works on R2 (part 2 of 3, try 1): <the requirement>` |
| A Microluna session calls `finish` | `Judgment`: `finish ▸ done: <its summary>` |
| A Microluna session ends | `Judgment`: its status, time, turns, calls, tokens, and Luna cost |
| Jev and code choose the next move | `Judgment`: `next step ▸ after session 1: moving on from R1 (Jev wanted to move on, 0.91; the checks say it passed)` |

Microluna's `run_command` and `read_file` calls are commands in the
terminal, and its patches and writes are `Judgment` lines, so the
operator watches each mini-handoff happen. `coder -p --json` streams the
same events as `judgment`, `shell_proposed`, `shell_outcome`, and `delta`
objects.

Claude Code reports a finished tool by its tool-use ID and reports every
tool's result, while only `Bash` starts a command, so a finished command is
paired with the oldest open one. The latest progress line rides the
terminal's status, and the reply's final text replaces the streamed
preview. The rail shows the turn's tokens and spend, such as
`28948/286 · $0.0741`.

## Conversation continuity

Microluna never resumes a session. A follow-up turn probes again and
rebuilds every session's context from the conversation so far, which is
the design: each session starts from a context code and Jev chose.

For a CLI, the door keeps the executor's session ID. A follow-up turn resumes it
(Claude Code `--resume`, Codex `exec resume`) with a fresh briefing: the
host probes again for the new request, and the briefing opens by saying it
continues the conversation. A session the CLI can no longer resume costs
the turn a fresh briefing, not its answer: the door starts a new session
once, with the conversation so far in the briefing.

## Cost and trace

Each delegated turn records, under `~/.openagents/traces/`, every step
Coder One recorded for it: the probe plan and operations, each Jev call
with its usage, the executor's normalized events, and the `delegate` call.
A `delegation` step then summarizes the turn with
`coder_one::episode::usage`: Jev at its list price, and the executor at the
cost its CLI reports. `coder -p --json` reports the total as `cost_usd`,
and the terminal shows it per turn. A cost with any unknown part is null,
never zero.

Each turn's briefing and the executor's stream are kept under
`~/.openagents/coder/delegate/<session>-<turn>/`.

## Measured

[Delegate against Gemini on eight terminal prompts](../measurements/2026-09-23-delegate-vs-gemini.md)
compares the two doors on real prompts in this repository: time to first
output, total time, cost, and whether each prompt was answered.
[Microluna on the same eight prompts](../measurements/2026-09-24-microluna-terminal.md)
adds the Microluna arm.

## Related

- [Delegation](delegate.md): the bounded fan-out a program's `delegate`
  step runs, which this door does not replace.
- [Headless mode](../guides/headless.md): the flags, the JSON stream, and
  the exit codes.
- [Traces](traces.md): what a trace holds.
- [Installing Coder](../guides/install.md): `scripts/install-coder.sh`,
  `coder --version`, and `coder doctor`.
