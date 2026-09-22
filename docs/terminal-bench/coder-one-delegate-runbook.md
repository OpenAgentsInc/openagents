# Coder One delegate runbook

This runbook measures Coder One's delegate mode
([#9532](https://github.com/OpenAgentsInc/openagents/issues/9532)) on
Terminal-Bench. The hypothesis: a strong model that starts from a
Jev-prepared briefing finishes with lower total cost and time than the same
model traversing the repository from scratch, at the same or better reward.
A result that shows no win is still a result, and you record it as one.

You run four arms on the same tasks and pins:

| Arm | What it runs |
| --- | --- |
| `claude-code-opus` | Claude Code 2.1.280 on `claude-opus-5-5`, directly. The baseline. |
| `coder-one` | Coder One alone: Gemini 3.8 Flash (`free` lane) plus Jev. |
| `coder-one-delegate-opus` | Coder One with `CODER_ONE_DELEGATE=always`: up to 8 read-only explore steps, then a code-built briefing to Claude Code on `claude-opus-5-5`. |
| `coder-one-delegate-auto` | Coder One with `CODER_ONE_DELEGATE=auto`: it works alone and delegates only when it stalls. |

The tasks are `fix-git` and `build-cython-ext`, the `smoke` profile. Record
results in [`README.md`](README.md), the one results page. The harness
itself is described in [the harness runbook](../coder/terminal-bench.md).

## How delegate mode works

1. **Explore.** The Coder One loop runs for at most `--explore-steps`
   steps (`CODER_ONE_EXPLORE_STEPS`, 8 by default). Under `always`, the
   explorer is told to investigate without editing. Jev ranks candidate
   files, picks key output spans, and checks each requirement every step.
2. **Decide.** Under `auto`, code escalates when the explorer reaches its
   step bound without finishing, when Jev reads three consecutive commands
   as `error`, or when the checkout is still unchanged after six steps. All
   three thresholds are unmeasured development values. Under `always`, the
   task is delegated after exploring.
3. **Brief.** Code assembles the briefing from recorded state: the
   instruction, requirements with their Jev probabilities, the explorer's
   conclusion, the top files by Jev relevance with excerpts, the
   Jev-selected output spans, the commands run with exit codes, and the last
   output. The cap is 12,000 characters (`CODER_ONE_BRIEFING_CAP`). Items
   that don't fit are left out whole and listed in the record.
4. **Delegate.** Coder One runs `claude -p --output-format stream-json
   --verbose --model claude-opus-5-5 --permission-mode bypassPermissions`
   in the task's working directory through `supervise`, with the briefing
   on standard input and a wall deadline (`CODER_ONE_DELEGATE_TIMEOUT`,
   600 seconds in an episode).
5. **Close.** Jev answers one more request: whether the task and each
   requirement now look satisfied. The episode then ends.

Delegation is a host decision. The generator never sees a `delegate` tool.

## Before you begin

- Docker, `nix`, and `jq` on the machine. `uv` runs through
  `nix shell nixpkgs#uv -c uv`.
- The pinned task checkout. From `bench/terminal-bench`, run:

  ```sh
  nix shell nixpkgs#uv -c uv sync --frozen
  nix shell nixpkgs#uv -c uv run tbench tasks checkout
  ```

- A Claude Code login on the host that can serve `claude-opus-5-5`. To
  refresh a short-lived token before you export it, run `claude -p ok`.

## Build and pin the artifact

Build from a clean tree at the commit you are measuring, so the version
string doesn't end in `-dirty`:

```sh
./scripts/build-coder-one-linux.sh
```

The script prints three lines:

```text
artifact_path=/…/x86_64-unknown-linux-musl/release/coder-one
artifact_sha256=<64 hex characters>
version: coder-one 0.1.0 (<commit>)
```

Keep the path and the digest in shell variables for the runs:

```sh
eval "$(./scripts/build-coder-one-linux.sh | grep '^artifact_')"
echo "$artifact_path $artifact_sha256"
```

Every Coder One arm in one comparison uses the same pin. The adapter
refuses a digest that doesn't match before any environment starts.

## Export the credentials without printing them

Each command reads a credential straight into a variable, so no value
reaches the terminal or the shell history:

```sh
export OPENAGENTS_API_KEY="$(cat ~/.openagents/bearer)"
export TYPESAFE_API_KEY="$(jq -r .api_key ~/.openagents/jev.json)"
export CLAUDE_CODE_OAUTH_TOKEN="$(jq -r .claudeAiOauth.accessToken ~/.claude/.credentials.json)"
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN
```

On macOS, the Claude token is in the login keychain; the
[harness runbook](../coder/terminal-bench.md#claude-code) shows how to read
it. The harness forwards every credential by name, as a `${VAR}` template
that Harbor resolves and redacts. Never paste a value into a command, a
file, or an issue.

Check that each variable is set, without showing it:

```sh
for name in OPENAGENTS_API_KEY TYPESAFE_API_KEY CLAUDE_CODE_OAUTH_TOKEN; do
  printenv "$name" >/dev/null && echo "$name set" || echo "$name missing"
done
```

## Check the install before you pay for inference

The `install-check` profile builds the `fix-git` environment, installs the
pinned Claude Code the way Harbor's own `claude-code` agent does, installs
Coder One, and runs `coder-one episode doctor`. The doctor checks the
generation door, the Jev door, `claude --version` (2.1.280 or newer), and
that a Claude credential is present. It spends no inference.

From `bench/terminal-bench`:

```sh
for arm in coder-one-delegate-opus coder-one-delegate-auto; do
  nix shell nixpkgs#uv -c uv run tbench run --profile install-check \
    --agent "$arm" --auth-mode subscription-oauth \
    --agent-kwarg artifact_path="$artifact_path" \
    --agent-kwarg artifact_sha256="$artifact_sha256"
done
```

The trial log shows the doctor's output under
`~/.openagents/terminal-bench/jobs/install-check--<arm>/`.

## Run the arms

From `bench/terminal-bench`, run each arm on each task. Each command is one
trial:

```sh
uvr() { nix shell nixpkgs#uv -c uv run "$@"; }
pin=(--agent-kwarg artifact_path="$artifact_path" --agent-kwarg artifact_sha256="$artifact_sha256")

for task in fix-git build-cython-ext; do
  uvr tbench run --profile smoke --agent claude-code-opus \
    --auth-mode subscription-oauth --task "$task"
  uvr tbench run --profile smoke --agent coder-one \
    --auth-mode door-key --task "$task" "${pin[@]}"
  uvr tbench run --profile smoke --agent coder-one-delegate-opus \
    --auth-mode subscription-oauth --task "$task" "${pin[@]}"
  uvr tbench run --profile smoke --agent coder-one-delegate-auto \
    --auth-mode subscription-oauth --task "$task" "${pin[@]}"
done
```

The job name is `smoke--<arm>--<task>`. Running the same job name again
resumes it rather than adding a trial. For a repetition, pass
`--job-name smoke--<arm>--<task>-2`.

Two optional adapter kwargs change the delegate's bounds, and each one
becomes an episode variable: `--agent-kwarg delegate_timeout_sec=<seconds>`
sets `CODER_ONE_DELEGATE_TIMEOUT`, and `--agent-kwarg explore_steps=<n>`
sets `CODER_ONE_EXPLORE_STEPS`. Keep the defaults for comparable rows, and
record any change in the analysis. The task's agent timeout is 900 seconds,
so the explore phase and the 600-second delegate deadline must fit inside
it.

## Where each trial's evidence lives

A job directory is `~/.openagents/terminal-bench/jobs/<job>/`, and each
trial is a `<task>__<id>/` directory inside it.

| Evidence | Path under the trial directory |
| --- | --- |
| Reward | `verifier/reward.txt` and `result.json` `verifier_result.rewards.reward` |
| Harbor phase timings | `result.json`: `environment_setup`, `agent_setup`, `agent_execution`, and `verifier`, each with `started_at` and `finished_at` |
| Harbor's usage and cost | `result.json` `agent_result`: `n_input_tokens`, `n_cache_tokens`, `n_output_tokens`, `cost_usd` |
| Claude Code's own stream (`claude-code-opus`) | `agent/claude-code.txt`, stream-json; its last line is the `result` event |
| Coder One's episode bundle | `agent/episode/`: `manifest.json`, `trajectory.atif.json`, and `evaluation/usage.json` |
| The briefing sent | `agent/episode/artifacts/delegate-1.briefing.md` |
| The delegate's own stream | `agent/episode/artifacts/delegate-1.stream.jsonl`. A stream over 8 MiB keeps its first and last 4 MiB, with an `openagents_truncated` marker line between them. |
| The harness's attempt record | `../tbench/attempts/<trial>.json` |

In the trajectory, the delegation is the step whose tool call is named
`delegate`. Its `arguments` hold `agent`, `isolation`, `prompt` (the exact
briefing), `bounds`, and `model`. Its `extra` holds the outcome (`status`:
`answered`, `refused`, `timed_out`, `failed`, or `harness`), `num_turns`,
`api_calls`, `input_tokens_per_call`, the CLI's `usage`, `total_cost_usd`
with `cost_provenance`, the briefing's `sha256`, `chars`, `cap`,
`included`, and `omitted`, and the escalation reason. The manifest's
`delegate` object repeats the mode, the policy, and the closing Jev check.

Before you check in a trace, copy it to
`bench/terminal-bench/traces/<job>/` the way the existing traces are kept,
and scan it for credential material.

## Compute each column of the results table

Set `T` to the trial directory and `U` to its usage record:

```sh
T=~/.openagents/terminal-bench/jobs/smoke--coder-one-delegate-opus--fix-git/fix-git__<id>
U="$T/agent/episode/evaluation/usage.json"
```

**Reward.** `cat "$T/verifier/reward.txt"`. With no reward file, write *No
result*, not zero.

**Cost.** For a Coder One arm, the total covers generation, Jev, and the
delegate:

```sh
jq '.cost.amount_usd' "$U"
jq '.components | {generation: .generation.cost_usd, jev: .jev.cost_usd, delegate: .delegate.cost_usd}' "$U"
```

A `null` means a component went unreported, so the total is unknown; show
`—` and say which component is missing. For `claude-code-opus`, the cost is
the CLI's own `total_cost_usd`:

```sh
tail -n 1 "$T/agent/claude-code.txt" | jq '.total_cost_usd'
```

`result.json` `agent_result.cost_usd` should match it.

**Cost source.** `claude-code-opus` on a subscription token is *CLI list
price*. `coder-one` is *Door-reported + Jev list price*. The delegate arms
are *Door-reported + Jev list price + CLI list price*: the delegate's part
is `components.delegate.cost_provenance`, which is `cli_list_price` on a
subscription token and `cli_reported` on an API key. Devin arms show `—`,
because Devin doesn't report what a run would cost to buy.

**Jev cost.** Jev input tokens times $0.042 per million. Output tokens are
free:

```sh
jq '.components.jev.input_tokens * 0.042 / 1000000' "$U"
```

It is `—` for `claude-code-opus`.

**Agent time.** Harbor's `agent_execution` phase, in seconds:

```sh
jq -r '.agent_execution | "\(.started_at) \(.finished_at)"' "$T/result.json" \
  | python3 -c 'import sys,datetime as d; a,b=sys.stdin.read().split(); f=lambda s: d.datetime.fromisoformat(s.replace("Z","+00:00")); print(round((f(b)-f(a)).total_seconds(),1))'
```

**Steps.** For a Coder One arm, the number of explore steps is the
manifest's `steps`; add one for the delegation. For `claude-code-opus`,
count the trajectory's agent steps:

```sh
jq '.steps' "$T/agent/episode/manifest.json"
jq '[.steps[] | select(.source == "agent")] | length' "$T/agent/trajectory.json"
```

**Tool calls.** For a Coder One arm, report generations, Jev requests,
shell commands, and delegations separately, plus the delegate's own turns:

```sh
jq '.calls' "$U"
jq '[.steps[].tool_calls[]? | select(.function_name == "shell")] | length' "$T/agent/episode/trajectory.atif.json"
jq '.components.delegate | {turns, api_calls}' "$U"
```

For `claude-code-opus`, count the tool calls in `agent/trajectory.json`
and read `num_turns` from the last line of `agent/claude-code.txt`.

**Tokens in / cached / out.** For a Coder One arm, show generation tokens
and the delegate's tokens separately, since only the delegate reports cache
reads:

```sh
jq '.components.generation | "\(.input_tokens) / 0 / \(.output_tokens)"' "$U"
jq '.components.delegate | "\(.total_input_tokens) / \(.cache_read_input_tokens) / \(.output_tokens)"' "$U"
```

The delegate's `total_input_tokens` counts uncached input, cache reads, and
cache writes. For `claude-code-opus`, use `result.json`
`agent_result.n_input_tokens`, `n_cache_tokens`, and `n_output_tokens`.

**Opus turns and Opus input tokens.** These are the numbers the
hypothesis turns on. For a delegate arm they are
`components.delegate.turns`, `components.delegate.api_calls`, and
`components.delegate.total_input_tokens`. For `claude-code-opus`, they are
`num_turns` and the input fields of the last `result` event in
`agent/claude-code.txt`. Count API calls there as distinct assistant
message ids:

```sh
jq -r 'select(.type == "assistant") | .message.id' "$T/agent/claude-code.txt" | sort -u | wc -l
```

## What each run's analysis covers

Add a subsection to the README's run analyses for each delegate trial, in
the same shape as the existing Coder One analyses. Cover these points:

- **Identity.** The artifact version and sha256, the Claude Code version
  from the stream's `init` event, the lane, the delegate model, the mode,
  the explore bound, and the delegate deadline.
- **Reward,** and which verifier tests passed or failed.
- **Cost by component.** Generation, Jev, and delegate, each with its
  provenance, and the total. Say what share of the total each one is.
- **Calls by kind.** Generations, Jev requests (per-step and the closing
  check), shell commands, delegations, and the delegate's Opus turns and
  API calls.
- **Tokens per call.** The mean and range of generation input tokens per
  call, and the delegate's `input_tokens_per_call`. The first call's input
  shows how much of each Opus call is Claude Code's own fixed prompt.
- **The briefing.** Its size against the cap, what it included, what it
  left out, and whether the delegate used or ignored the explorer's
  evidence. Read the stream to tell.
- **Where the time went.** Harbor's phases, then within agent time: the
  explore phase (generation, Jev, and shell), the delegate's wall time and
  its `duration_api_ms`, and the closing check.
- **The escalation,** for `auto`: whether it delegated, and which policy
  signal fired at which step.
- **Against `claude-code-opus` direct, on the same task.** Reward, total
  cost, agent time, Opus turns, and Opus input tokens, side by side. State
  whether the delegate arm matched Opus's reward with fewer Opus turns and a
  lower total cost or time. If it didn't, say so and say where the extra
  cost or time came from.

Every row is one trial. Label the comparison as a small development sample,
not a pass-rate estimate.
