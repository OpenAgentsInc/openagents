# Terminal-Bench runbook: operating notes

This page collects what the other Terminal-Bench documents don't say: the
host, the credentials and their failure modes, the rate limits, the version
pins that matter, and how to turn a finished trial into a results row. Read
it before you run anything, then follow the procedure in the document for
the arms you run.

- Harness mechanics, profiles, and resume: [the harness runbook](../coder/terminal-bench.md).
- The artifact contract Coder One implements: [`openagents.coder.episode.v1`](../coder/terminal-bench-contract.md).
- Coder One's delegate arms: [the delegate runbook](coder-one-delegate-runbook.md).
- Every result: [the results page](README.md).
- Cancellation, timeouts, refusals, resume, image state, and repeated-run statistics: [the resilience page](resilience.md).

## The host

Current trials run on an x86_64 NixOS host with 28 CPUs and native Docker.
Earlier baselines (Fable 5.1, Sonnet 4.5, Codex 0.153.3, Devin) ran on an
arm64 Mac, so agent times from the two machines aren't strictly
comparable. The results page marks times that come from a trajectory
rather than Harbor's phase timings with †.

- `uv` isn't installed globally. Run the harness through nix:

  ```sh
  cd bench/terminal-bench
  nix shell nixpkgs#uv -c uv sync --frozen
  nix shell nixpkgs#uv -c uv run tbench doctor --agent coder-one
  ```

  `nix-ld` is enabled, so uv-managed Python and manylinux wheels work.
- The pinned task checkout lives at
  `~/.openagents/terminal-bench/upstream/terminal-bench` (commit
  `3b5caaa4863d`). `uv run tbench tasks checkout` creates it.
- Jobs land in `~/.openagents/terminal-bench/jobs/<job>/`. Failed setup
  attempts that aren't results are moved to
  `~/.openagents/terminal-bench/failed/` so they don't pool with real
  trials.

## Credentials

Never print a credential, paste it into a command, or copy it into a trace.
Export each one straight from its file, and scan retained evidence for it
before a commit (see [Retain the evidence](#retain-the-evidence)).

| Arm family | Variable | Where it comes from | Notes |
| --- | --- | --- | --- |
| Coder One generation | `OPENAGENTS_API_KEY` | `~/.openagents/bearer`, an openagents.com session token | Serves `https://openagents.com/v1/responses`. |
| Coder One Jev | `TYPESAFE_API_KEY` | `api_key` in `~/.openagents/jev.json` | Hosted TypeSafe, `jev-1.13.0`. |
| Claude Code, and Coder One's Opus delegate | `CLAUDE_CODE_OAUTH_TOKEN` | `claudeAiOauth.accessToken` in `~/.claude/.credentials.json` | Expires after about eight hours; check `expiresAt` and run `claude -p ok` to refresh it. Unset `ANTHROPIC_API_KEY` and `ANTHROPIC_AUTH_TOKEN` for `--auth-mode subscription-oauth`. |
| Codex | `CODEX_FORCE_AUTH_JSON=1` (host-side selector) | `~/.codex/auth.json` | Sign in once with `codex login --device-auth`; it needs a person at a browser. Use `--auth-mode auth-json`. The selector never reaches the container, because Harbor scrubs the values of credential-named variables from evidence, and a forwarded `1` once redacted every digit in a result file. |

```sh
export OPENAGENTS_API_KEY="$(tr -d '\n' < ~/.openagents/bearer)"
export TYPESAFE_API_KEY="$(jq -r .api_key ~/.openagents/jev.json)"
export CLAUDE_CODE_OAUTH_TOKEN="$(jq -r .claudeAiOauth.accessToken ~/.claude/.credentials.json)"
export CODEX_FORCE_AUTH_JSON=1
unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN OPENAI_API_KEY
```

A host login doesn't authenticate the container. Harbor installs each agent
inside the task environment and forwards only what the arm's profile names.

## Version pins that matter

- **Opus 5.5 needs Claude Code 2.1.280 or newer.** On 2.1.278 the API
  answers `400 claude_code_version_too_old` and the trial ends in about a
  second with no inference. `claude-code-opus` pins 2.1.280, and so do the
  Coder One Opus delegate arms.
- **GPT-6 Sol and Luna appear only in Codex's live catalog.** The catalogs
  bundled in Codex 0.155.1 and 0.157.0-alpha.9 list `gpt-6-astra` but not
  `gpt-6-sol` or `gpt-6-luna`. After `codex login`, `codex debug models`
  lists all three for a ChatGPT account. The `codex-gpt-6-*` arms pin CLI
  0.155.1. The old `codex` arm keeps 0.153.3 so earlier results stay
  attributable.
- **ChatGPT-account Codex can't serve `gpt-5.2-codex` or `gpt-5.1-codex`**
  (HTTP 400). Use the GPT-6 or GPT-5.6 names the live catalog shows.

## The free lane's rate limit

Coder One generates on the openagents.com `free` lane, which admits **20
generations a minute for one account**. The limit is in the production
service (`bins/coder-serve/src/responses_endpoint/free_limit.rs` in the
private `OpenAgentsInc/coder` repository). A Coder One trial generates
roughly 10 to 15 times a minute, so:

- **Run Coder One trials one at a time**, and don't run a delegate arm's
  explorer alongside another Coder One trial. On 2026-09-22 eight explorers
  ran at once, every one of them hit the limit, and four trials ended as
  `generation_failed`. Those rows are listed as invalid, not as losses.
- Coder One since commit `9f31630ac6` retries a 429 up to six times,
  honors `Retry-After`, and backs off to 65 seconds, so a single trial
  waits out the window instead of failing. The waiting still counts as
  agent time. Check a trial for `generation failed: the free lane takes 20
  generations a minute` in its trajectory before you analyze its time.
- Commit `7e810c3a5` in `OpenAgentsInc/coder` exempts operator admins
  (`CODER_ADMINS`, `AtlantisPleb` when unset) from the limit. It is live
  since `0773a7fdf` (2026-09-22): 50 free-lane calls in a minute all
  answered. With the operator's session token, Coder One trials can run in
  parallel; any other account still needs them one at a time.

## Pin the Coder One artifact

Build a static Linux binary from a clean tree:

```sh
./scripts/build-coder-one-linux.sh
```

It prints `artifact_path`, `artifact_sha256`, and the version with the
commit it was built from. It needs a musl C compiler; on this host the
script fetches one through `nix`. Set `CODER_ONE_TARGET_DIR` to keep a
build out of another worktree's target directory.

When you rebase a branch after benchmarking it, the commit the artifact
names disappears from history. Tag the benchmarked commit first and push
the tag, as `coder-one-speed-99e647a7a974` was, so the version string in a
result still resolves.

Coder One's own switches reach an episode as environment variables, which
an arm's profile sets:

| Variable | Arm that sets it | Effect |
| --- | --- | --- |
| `CODER_ONE_DEEP=on` | `coder-one-deep` | A parallel Jev survey of up to 100 files before the first step, a "done and checked" question every step, and repeated-command hints. |
| `CODER_ONE_JEV=off` | `coder-one-no-jev` | The ablation: no Jev calls. |
| `CODER_ONE_DELEGATE=always` or `auto` | the delegate arms | Delegate mode; see the delegate runbook. |
| `CODER_ONE_EXPLORE_STEPS=0` with `CODER_ONE_DEEP=on` | `coder-one-jevbrief-opus`, `coder-one-jevbrief-luna` | Jev-brief: no Gemini explorer; the Jev survey's files go straight into the delegate's briefing. |

## Name jobs so results don't collide

The job name is `<profile>--<arm>--<task>`, and running the same name again
**resumes** that job instead of adding a trial. That includes a run with a
new artifact. Pass `--job-name` for anything that must be a separate trial:

- a new artifact on an existing arm, for example
  `--job-name smoke--coder-one-v2--fix-git`;
- a rerun after a failed or invalid attempt, for example
  `smoke--coder-one-deep2--fix-git`;
- a repetition, for example `smoke--claude-code-opus--fix-git-2`.

Record which artifact each job name ran in the results page.

## Watch a long run

Start trials in the background and wait for them to exit. Don't write a
`while pgrep -f '<pattern>'` loop: `pgrep -f` matches the loop's own
command line, so it never ends. Wait on the job's process instead, or
check for the trial's `result.json`.

Harbor warns that an arm's `extra_allowed_hosts` are ignored because the
effective network policy is public. That is expected for these tasks; the
list only matters under a restricted policy.

## Turn a trial into a results row

```sh
cd bench/terminal-bench
python3 tools/trial_metrics.py smoke--coder-one--fix-git smoke--claude-code-opus--fix-git
```

The tool prints, for every trial in each job: reward, exception, Harbor's
agent and total time, and Harbor's usage and cost. For a Coder One trial it
adds the outcome, calls by kind, the generation, Jev, and delegate
components, the exact Jev cost, and time by kind of step. For a Codex GPT-6
trial it prints a manual cost.

Pricing rules the results page follows:

- **Jev** is exact: reported Jev input tokens × $0.042 per million. Output
  is free.
- **GPT-6** costs are manual, from OpenAI's standard short-context pricing
  that the operator supplied on 2026-09-22, because Harbor records none for
  these models. Uncached input, cached input, and output each at their rate:

  | Model | Input | Cached input | Output |
  | --- | --- | --- | --- |
  | `gpt-6-astra` | $10.00 | $1.00 | $50.00 |
  | `gpt-6-sol` | $2.00 | $0.20 | $10.00 |
  | `gpt-6-luna` | $0.10 | $0.01 | $0.50 |

  Pricing the two Codex 0.153.3 Astra trials this way reproduces Harbor's
  estimates exactly. Mark these costs ‡ and label them manual.
- **Claude Code** reports `total_cost_usd`. On a subscription token that is
  a list-price figure, not a bill.
- **Devin** shows `—`: it doesn't report what a run would cost to buy.
- **Coder One generation** is the door's `cost_microusd` per call. The
  `free` lane is described as metered at zero but still reports a figure;
  the page uses the reported figure and says so.

## Retain the evidence

For each trial you add to the results page, copy its evidence into the
repository:

```sh
T=~/.openagents/terminal-bench/jobs/<job>/<trial>
D=bench/terminal-bench/traces/<job>
mkdir -p "$D/<trial>.episode"
cp "$T/agent/trajectory.json" "$D/<trial>.json"
cp "$T/agent/episode/manifest.json" "$T/agent/episode/evaluation/usage.json" "$D/<trial>.episode/"   # Coder One only
```

Also write a trimmed `harbor-result.json` beside it with the phase
timings, the verifier result, the exception, and `agent_result`'s usage
and cost. Then scan everything you copied for each credential's value, and
commit only when nothing matches:

```sh
grep -rlF -e "$OPENAGENTS_API_KEY" -e "$TYPESAFE_API_KEY" -e "$CLAUDE_CODE_OAUTH_TOKEN" "$D" || echo clean
```

Three Codex `fix-git` trajectories from the Mac
(`smoke--codex--fix-git`, `-2`, and `-3`) aren't valid JSON because
Harbor's scrubber rewrote literal values in them. Keep them, and don't
count them.

## After each run

1. Add the row to the task's table and to its **Cheapest first** table,
   in cost order.
2. For a Coder One arm, add a run analysis to the results page: cost by
   component, calls by kind against the other arms, tokens per call, where
   the time went, and what explains the difference. The delegate runbook
   lists what a delegate analysis covers.
3. Note any invalid attempt, such as a rate-limit or setup failure, under
   **Data problems** rather than as a result.
4. Commit and push to `main`. The repository runs no gate before a push.
