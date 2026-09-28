# The delegate door

Coder Terminal answers a turn through an executor rather than a model that
streams text. By default the executor is Microcoder, the simple loop:
Jev judges the state, one structured model call returns the next
commands, and the commands run in the working directory. Microcoder runs
in this process and generates through the first connected provider with
capacity: GPT-6 Luna on the operator's Codex login, then Claude Code's
login. When Microcoder has no provider to use, the executor is Claude
Code, then Codex CLI, briefed with what Jev chose from the workspace. The
Open Responses door, Gemini today, answers only when this machine has no
delegation target.

Status: implemented in `crates/coder` (`delegate_door.rs` and
`delegate_door/microcoder.rs`, wired through `agent.rs` and
`generate.rs`). The Microcoder loop is `crates/microcoder-loop`, and the
CLI turns are `coder_one::terminal`. The terminal and `coder -p` both
reach the door, because both run `coder::turn::run`. Issue
[#9578](https://github.com/OpenAgentsInc/openagents/issues/9578) holds the
plan, and [#9879](https://github.com/OpenAgentsInc/openagents/issues/9879)
the move from Microluna to Microcoder.

## A Microcoder turn

```text
while the reply isn't finished:
    jev    Jev judges the state: done, progress, repeating
    prompt the request, the conversation, the state, and Jev's answers
    reply  one structured model call returns the next commands and a reply
    run    the host runs the commands inside a coder-boundary boundary
```

This is the loop `microcoder repository` runs for the task owner
([Microcoder repository execution](microcoder-repository.md)), with the
same limits off: no acceptance tests, no knowledge base, and no stronger
model. A turn is bounded at 40 steps, $2.00, and the 600-second deadline.
The reply the user reads is the finishing step's `reply`. A step that
asks the user a question ends the turn, and the answer starts the next
one.

The loop's generators and failover aren't `Send`, so the door runs the
turn on a thread of its own with a current-thread runtime and hears its
progress over a channel.

### Which provider generates

The door lists Microcoder's providers in preference order:

| Provider | Model | Connected when |
| --- | --- | --- |
| Codex | `gpt-6-luna`, or `CODER_DELEGATE_MODEL` | `~/.codex/auth.json` (or `$CODEX_HOME/auth.json`) is a ChatGPT sign-in whose access token has more than ten minutes left |
| Claude | Claude Code's `opus` alias | a `claude` binary (`CLAUDE_BIN`, `PATH`, or `~/.local/bin`) and a Claude Code sign-in |

Before each turn, the door reads the capacity book,
`~/.openagents/tasks/capacity.json`, the same book the auto-start policy
reads and repository runs write. A provider with a refusal that still
holds is skipped, and the session header and `coder doctor` say why, for
example "it skips codex because the Codex login is out of its usage limit
until 2026-10-03 18:07 UTC". The turn starts on the first connected
provider with capacity.

When a provider refuses for a usage or rate limit during the turn, such as
Codex's HTTP 429 `usage_limit_reached`, the loop records the refusal in the
book with its reset time and generates the same step on the next connected
provider with capacity. The turn's trace holds a `route_switch` step that
names both providers and the refusal, and the next turn skips the refused
provider without asking it again. A Claude Code refusal records the reset
from the stream's `rejected` `rate_limit_event` (`resetsAt`). When a
refusal reports no reset, a fresh usage probe reading in the task store's
`usage.json` supplies the reset of the window it shows at its limit, and
only without either does it hold for 30 minutes, which the sentence and
`coder doctor` say.

When no provider is left, the turn ends with one sentence that names each
provider and when it resets, such as "Microcoder has no provider to answer
with: the Codex login is out of its usage limit until 2026-10-03 18:07 UTC
and the Claude Code login can't be used (Claude Code is not signed in)."
The error's cause is `no_capacity`.

## A Claude Code or Codex turn

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
600-second deadline. Terminal-Bench measured the Jev probe battery plus a
strong executor beating a model that explores on its own; see
[the component study](../../optimization/coder-components.md).

Coder One's judge and recorder are not `Send` either, so this turn also
runs on a thread of its own. Coder One's progress lines, which an episode
prints to standard output, go to the door through `coder_one::say`
instead, so the terminal keeps its screen and `coder -p` keeps standard
output for the reply.

## Which door answers

`Door::from_env` is unchanged, and `coder-worker` still builds its door
with it. The terminal and `coder -p` choose through
`delegate_door::open`:

| Setting | Effect |
| --- | --- |
| `CODER_DELEGATE=auto` | The default. Delegate to Microcoder when one of its providers is connected and has capacity. Otherwise delegate to an installed and authenticated `claude`, then `codex`, when its login has capacity. Otherwise fall back. Each choice says why. |
| `CODER_DELEGATE=always` | Delegate, or refuse to start. |
| `CODER_DELEGATE=off` | Never delegate. |
| `CODER_DELEGATE_AGENT` | `microcoder`, `claude-code`, or `codex`: consider only that target. `microluna` names Microcoder, which replaced it. |
| `CODER_DELEGATE_MODEL` | The target's model. For Microcoder it names the Codex model. Unset, Microcoder and Codex run `gpt-6-luna`, and Claude Code runs `claude-opus-5-5`. |
| `CODER_WORKER`, `CODER_EXECUTOR` | An explicit request for the relay or a local executor. It outranks `auto` and contradicts `always`. |

Microcoder needs no binary of its own. The door only reads the Codex
login; it never refreshes it and never prints a token. When the token is
near expiry, Codex counts as not connected, and `coder doctor` says so;
any Codex command refreshes the login.

A CLI target is available when its binary is found (`CODER_ONE_CLAUDE_BIN` or
`CODER_ONE_CODEX_BIN`, then `PATH`, then `~/.local/bin`), a credential
is (`CLAUDE_CODE_OAUTH_TOKEN`, an Anthropic API key, or the CLI's stored
login for Claude Code; `~/.codex/auth.json` or `OPENAI_API_KEY` for
Codex), and the capacity book holds no refusal for its login. Claude Code
spends the Claude login and Codex CLI the Codex login. An Open Responses
key such as `CODER_DOOR_KEY` names the fallback rather than asking for it.

When no target is available and a target was skipped only for capacity,
and no Open Responses key is set, every turn of the session ends with one
sentence naming each target and when its login resets, instead of the
stub's placeholder reply.

Before this door, `CODER_DELEGATE` named the capability a program's
`delegate` step hands work to. It still does when its value is none of
`auto`, `always`, and `off`.

The session says which door answers and why in three places: the first
line the terminal draws and the first line `coder -p` writes to standard
error, the trace's session header (`door: delegate`, `model:
microcoder/opus`), and a `door` step that holds the reason.
`coder doctor` reports the same choice without running a turn, and lists
each Microcoder provider, whether it's used first, and why a skipped one
was skipped. See [installing Coder](../guides/install.md).

## What the executor may do

The turn's permit decides, and nothing the executor says widens it.

| Permit | Boundary |
| --- | --- |
| Runs no commands: a clarifying turn, or `CODER_SHELL=off` | Read-only. Microcoder's commands may write only a private temporary directory, which is `TMPDIR`. A CLI executor may also write its own state (`~/.claude`, `~/.claude.json`, `~/.codex`, `~/.cache`) and the turn's artifacts directory. The workspace stays read-only even when it sits inside one of those. |
| Runs commands | Workspace-writable. The commands may also write inside the working directory, and nowhere else. |

The boundary is `coder-boundary`'s: `bwrap` on Linux, `sandbox-exec` on
macOS. A host that cannot enforce one fails the turn rather than run the
executor unbounded. Delegation stays a host decision: the model never sees
a delegate tool, and the door is chosen before a turn generates a word.

## A turn that works an issue

The issue flow in `coder_one::issue_turn`, which clones a repository,
works an issue on a new branch, and opens a draft pull request, ran on
Microluna's loop. Microcoder replaced Microluna in this door on
2026-09-28, so the terminal no longer starts the issue flow. Work sent
from the OpenAgents phone app runs through `microcoder repository`
instead; see [host auto-start](host-autostart.md).

## What the terminal shows

The executor's events become the turn's own events, so a delegated turn
draws the way a generated one does:

| Executor event | Turn event |
| --- | --- |
| Text the executor said, or Microcoder's reply | `Delta`, the reply streaming in |
| A command started | `Shell(Proposed)` |
| A command finished | `Shell(Ran)`, with its exit code and output |
| A file written or edited | `Judgment`, such as `update ▸ src/lib.rs` |
| A probe, survey, or briefing line | `Judgment`, such as `survey ▸ Jev rated 40 files …` |
| Jev's answers for a Microcoder step | `Judgment`: `jev ▸ step 1: done 0.24 · progress 0.04 · repeating 0.04` |
| A Microcoder step's rationale | `Judgment`: `step 1 ▸ claude-opus-5-5: Greeting only; nothing to run.` |

Microcoder reports a command when it finishes, so its `Shell(Proposed)`
and `Shell(Ran)` arrive together. `coder -p --json` streams the same
events as `judgment`, `shell_proposed`, `shell_outcome`, and `delta`
objects.

Claude Code reports a finished tool by its tool-use ID and reports every
tool's result, while only `Bash` starts a command, so a finished command is
paired with the oldest open one. The latest progress line rides the
terminal's status, and the reply's final text replaces the streamed
preview. The rail shows the turn's tokens and spend, such as
`28948/286 · $0.0741`.

## Conversation continuity

Microcoder never resumes a session. A follow-up turn rebuilds the prompt
from the conversation so far, which is the design: each step starts from
a prompt code and Jev built.

For a CLI, the door keeps the executor's session ID. A follow-up turn resumes it
(Claude Code `--resume`, Codex `exec resume`) with a fresh briefing: the
host probes again for the new request, and the briefing opens by saying it
continues the conversation. A session the CLI can no longer resume costs
the turn a fresh briefing, not its answer: the door starts a new session
once, with the conversation so far in the briefing.

## Cost and trace

Each delegated turn records, under `~/.openagents/traces/`, every step it
took. A Microcoder turn records each loop observation (Jev's judgment,
the generation with its tokens and cost, and each command), failover's
`route_capacity`, `route_switch`, and `route_exhausted` steps, and a
`delegation` summary with the provider standings, the ending, and the
refusals the turn met. A CLI turn records every step Coder One recorded:
the probe plan and operations, each Jev call with its usage, the
executor's normalized events, and the `delegate` call, then a
`delegation` step with `coder_one::episode::usage`. `coder -p --json`
reports the total as `cost_usd`, and the terminal shows it per turn. A
cost with any unknown part is null, never zero.

A CLI turn's briefing and the executor's stream are kept under
`~/.openagents/coder/delegate/<session>-<turn>/`.

## Measured

[Delegate against Gemini on eight terminal prompts](../measurements/2026-09-23-delegate-vs-gemini.md)
compares the two doors on real prompts in this repository: time to first
output, total time, cost, and whether each prompt was answered.
[Microluna on the same eight prompts](../measurements/2026-09-24-microluna-terminal.md)
adds the Microluna arm, which Microcoder has since replaced.

## Related

- [Delegation](delegate.md): the bounded fan-out a program's `delegate`
  step runs, which this door does not replace.
- [Headless mode](../guides/headless.md): the flags, the JSON stream, and
  the exit codes.
- [Traces](traces.md): what a trace holds.
- [Installing Coder](../guides/install.md): `scripts/install-coder.sh`,
  `coder --version`, and `coder doctor`.
