# The delegate door

Coder Terminal answers a turn by briefing Claude Code or Codex with what
Jev chose from the workspace, the way Coder One answers a Terminal-Bench
task. The Open Responses door, Gemini today, answers only when this machine
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
| `CODER_DELEGATE=auto` | The default. Delegate when an installed and authenticated `claude` or `codex` is found, Claude Code first. Otherwise fall back and say so. |
| `CODER_DELEGATE=always` | Delegate, or refuse to start. |
| `CODER_DELEGATE=off` | Never delegate. |
| `CODER_DELEGATE_AGENT` | `claude-code` or `codex`: consider only that target. |
| `CODER_DELEGATE_MODEL` | The target's model. Unset, Claude Code runs `claude-opus-5-5` and Codex runs `gpt-6-luna`. |
| `CODER_WORKER`, `CODER_EXECUTOR` | An explicit request for the relay or a local executor. It outranks `auto` and contradicts `always`. |

A target is available when its binary is found (`CODER_ONE_CLAUDE_BIN` or
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

## What the terminal shows

The executor's events become the turn's own events, so a delegated turn
draws the way a generated one does:

| Executor event | Turn event |
| --- | --- |
| Text the executor said | `Delta`, the reply streaming in |
| A command started | `Shell(Proposed)` |
| A command finished | `Shell(Ran)`, with its exit code and output |
| A file written or edited | `Judgment`, such as `update ▸ src/lib.rs` |
| A probe, survey, or briefing line | `Judgment`, such as `survey ▸ 40 files judged …` |

Claude Code reports a finished tool by its tool-use ID and reports every
tool's result, while only `Bash` starts a command, so a finished command is
paired with the oldest open one. The latest progress line rides the
terminal's status, and the reply's final text replaces the streamed
preview. The rail shows the turn's tokens and spend, such as
`28948/286 · $0.0741`.

## Conversation continuity

The door keeps the executor's session ID. A follow-up turn resumes it
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

## Related

- [Delegation](delegate.md): the bounded fan-out a program's `delegate`
  step runs, which this door does not replace.
- [Headless mode](../guides/headless.md): the flags, the JSON stream, and
  the exit codes.
- [Traces](traces.md): what a trace holds.
- [Installing Coder](../guides/install.md): `scripts/install-coder.sh`,
  `coder --version`, and `coder doctor`.
