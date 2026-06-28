# Khala CLI

A collective intelligence you chat with from your terminal. Free to use,
streams answers live, no signup or API key required. Built with Effect; runs on
plain Node or Bun.

```sh
# Install and run (Node or Bun; npm works for everyone)
npm install -g @openagentsinc/khala
khala
khala --prompt "Say hello in one short sentence."

# Bring your own provider key (BYOK): run usage on your own provider account
khala key add openrouter sk-or-v1-...
khala key list
khala key remove

bunx @openagentsinc/khala --prompt "Say hello in one short sentence."
khala feedback "The input ate my transcript."
khala changelog
khala tokens
khala info
khala version
khala login
khala logout
khala fleet connect           # connect your own Codex account (paste-free device login)
khala fleet connect --account codex-2   # add another distinct account for more throughput
khala fleet link              # link this local Pylon to your signed-in Khala account
khala fleet status            # list your connected Codex fleet + readiness
khala fleet run --repo owner/repo --issues 123,124 --verify "bun test" --dry-run
khala auth codex
khala codex "read README.md"
khala spawn --count 5 --objective "audit this workspace" --strategy local
khala spawn --strategy pylon --workflow codex_agent_task --count 5 --objective "implement public issue #123" --repo OpenAgentsInc/openagents --commit <sha> --verify "bun test"
khala workers
khala join <runRef>
khala cancel <runRef|workerRef>
khala --artanis --prompt "status"
bun run khala
bun run khala -- --prompt "Say hello in one short sentence."
printf 'Say OK only.' | bun run khala -- --headless --json
```

The published npm package is `@openagentsinc/khala`; it exposes the `khala`
command through a Node shebang and is bundled for Node, so a global
`npm install -g @openagentsinc/khala` works with either Node 20+ or Bun on
`PATH`. The local Codex delegation features additionally need the optional
`@openai/codex-sdk` dependency (installed with the package) plus a connected
Codex account.

"Khala CLI" names the command-line program surface: the `khala` command. It has
two modes:

- **Interactive terminal mode:** default `khala` opens a normal terminal chat
  transcript with `>` prompts and `Khala:` turns in scrollback. Up/Down cycles
  through previous prompts. Ctrl-L clears the terminal and redraws the prompt.
  While waiting for the first response bytes, it prints one dot per second. It does
  not use an alternate-screen/full-screen UI. Slash commands such as
  `/feedback`, `/info`, `/msginfo`, `/codex`, `/tokens`, `/changelog`,
  `/version`, and `/help` are handled locally instead of being sent to
  inference.
  Provider-labeled reasoning is rendered separately as dim `Khala reasoning:`
  text when the API supplies it.
- **Headless CLI mode:** `--prompt`, positional text, or stdin runs one turn and
  streams the assistant response to stdout for scripts and agents.
- **Local Codex delegation:** when the Blueprint route selector sees that a
  request needs the local workspace, filesystem, shell, git, tests, or code
  edits, Khala delegates that turn to a connected local Codex account instead
  of letting the chat model claim it has no file access. `khala auth codex`
  connects Codex with device auth, and existing Pylon Codex account homes are
  reused automatically when present. Set `KHALA_CODEX_AUTO=off` to disable
  automatic delegation.
- **Khala spawn supervisor:** `khala spawn --count N --objective "..."`
  starts a bounded parent run with supervised child workers recorded under the
  local Khala home. The default `local`/`auto` strategy is backed by local Codex
  workers in isolated worktrees when the current directory is a Git checkout.
  `--strategy pylon` dispatches child assignments through caller-owned linked
  Pylon Codex capacity via the reviewed `khala.spawn` MCP surface; pass
  `--pylon-ref` to target one owned Pylon, and pass `--repo`, `--commit`, and
  `--verify` together for public repository work. Use `khala workers`,
  `khala worker <workerRef>`,
  `khala join <runRef>`, and `khala cancel <runRef|workerRef>` to inspect and
  control runs. Normal chat turns such as `spin up 5 subagents to audit X` are
  routed through the same typed selector and start the supervisor when selected.
  Capability questions answer with the `/spawn` and `khala spawn --count`
  command paths instead of falling through to a generic chat refusal.
- **Utility commands:** `khala feedback "..."` saves feedback to
  `POST /api/khala/feedback`, `khala tokens` reads the public Khala
  tokens-served counter, and `khala changelog` prints the recent package
  changelog.
- **Owner/operator auth:** `khala login` and `/login` use OpenAgents device
  auth; `khala logout` and `/logout` clear the local token. Auth enables
  owner/operator flows such as Artanis. It is not a billing or wallet console.
- **Artanis operator channel:** `khala --artanis`, `khala artanis`, and
  `/artanis` talk to the owner-authenticated Artanis operator channel when the
  local login has access. This is not a public agent endpoint.

Interactive mode checks npm for a newer `@openagentsinc/khala` in the
background. If a newer version installs successfully, it prints one line and
the next `khala` launch uses the update. Set `KHALA_NO_AUTO_UPDATE=1` to skip
that check.

## Interactive commands

- `/feedback <text>` saves product feedback without sending the text to
  inference. When a trace reference is available, the CLI includes it with the
  feedback.
- `/info` prints the current CLI thread id, the last request trace, and a trace
  viewing link. When no exact stored `/trace/{uuid}` URL has been reported yet,
  it uses the same owner-token `/traces?token=...` pattern as the Khala mobile
  app and stores that token under the local Khala config directory.
- `/msginfo` prints the last Khala response metadata: trace reference, Khala
  orchestrator model, backend model/adapter routing, fallback reason, token
  counts, first-byte / first-token / stream / total latency, and tokens per
  second when reported by the backend.
- `/codex status` shows whether local Codex workspace delegation is connected.
- `/codex connect` runs Codex device auth into Khala's local Codex home.
- `/codex <task>` delegates a workspace task directly to Codex.
- `/spawn <count> <task>` starts supervised Khala child workers.
- `/workers` lists local Khala spawn runs.
- `/worker <workerRef>` shows one child worker.
- `/join <runRef>` shows an aggregate spawn run.
- `/cancel <runRef|workerRef>` cancels a run or worker.
- `/tokens` prints the global Khala tokens-served count from the same public
  counter shown on `openagents.com` and `/khala`.
- `/changelog` prints the five most recent CLI releases in reverse
  chronological order.
- `/login` starts OpenAgents device auth.
- `/logout` clears the local OpenAgents token.
- `/artanis <message>` sends an owner-authenticated operator message to
  Artanis.
- `/khala <message>` switches a slash-prefixed message back to the normal Khala
  channel.
- `/version` prints the installed CLI version.
- `/help` lists slash commands.
- `/exit` quits.

## Connect your Codex fleet

`khala fleet` is the dead-simple way to connect your own Codex account(s) so a
per-user Artanis can burn down a backlog across your fleet. One short command,
no long-string pasting:

- `khala fleet connect` connects a Codex account via the standard
  `codex login --device-auth` flow — it opens the browser to the device URL and
  shows a SHORT code to enter, then confirms with the linked account email.
  Requires the `codex` CLI (`npm install -g @openai/codex`); a friendly hint is
  printed if it is missing.
- `khala fleet link` registers this local Pylon's public identity with your
  signed-in Khala/OpenAgents owner token from `khala login`, so caller-owned
  Pylon dispatch can target it without token copying.
- Run it again to add more accounts (auto-assigned `codex`, `codex-2`,
  `codex-3`, …), or pass `--account <ref>` to name one. Distinct ChatGPT
  accounts have distinct rate budgets, so each new distinct account is real
  added throughput.
- `khala fleet status` (alias `khala fleet list`) prints a table of connected
  accounts with readiness and email.
- `khala fleet run --repo owner/repo --issues 123,124 --verify "bun test"`
  starts the turnkey Pylon/Codex supervisor against your public repo backlog.
  It auto-resolves your local Pylon ref, computes slots as ready accounts times
  `--per-account` capped by `--max-parallel`, advertises that capacity, and
  routes issue work through your local no-spend Pylon. Use `--dry-run` to inspect
  the resolved plan first, or `--once` to run one refill round and exit.

Each account uses an isolated home under `<pylon home>/accounts/codex/<ref>`; the
flow never touches the default `~/.codex` home, credentials stay on your machine,
and tokens are never printed. Accounts are registered into your Pylon config so a
local Pylon and the dispatch gate can see the fleet.

## Utility commands

- `khala feedback "text"` sends feedback from scripts or a shell. This command
  may not have a chat trace reference, which is expected.
- `khala info` prints a one-shot CLI thread id and trace viewing link.
- `khala login` starts OpenAgents device auth.
- `khala logout` clears the local OpenAgents token.
- `khala fleet connect` connects a Codex account to your fleet (paste-free).
- `khala fleet link` associates this local Pylon with your signed-in Khala owner
  account.
- `khala fleet status` lists your connected Codex fleet and readiness.
- `khala fleet run --repo owner/repo --issues 123,124 --verify "bun test"`
  starts or plans the backlog supervisor for your connected fleet.
- `khala auth codex` connects a Codex account for local workspace delegation.
- `khala codex status` shows the active local Codex credential source.
- `khala codex "task"` delegates directly to local Codex.
- `khala spawn --count N --objective "task"` starts supervised child workers.
- `khala workers` lists local spawn runs.
- `khala worker <workerRef>` shows one child worker.
- `khala join <runRef>` shows an aggregate run.
- `khala cancel <runRef|workerRef>` cancels a run or worker.
- `khala artanis "message"` sends an owner-authenticated operator message.
- `khala tokens` prints the current global Khala tokens-served count.
- `khala changelog` prints recent releases.
- `khala version` prints the installed CLI version.
- `khala help` prints CLI usage.

## Flags

- `--public` uses `POST /api/khala/chat` without auth. This is the default.
- `--api` uses `POST /api/v1/chat/completions` with model `openagents/khala`.
- `--token <token>` overrides `OPENAGENTS_AGENT_TOKEN` for `--api`.
- `--base-url <url>` overrides `https://openagents.com`.
- `--prompt <text>` runs one headless turn.
- `--headless` reads a single prompt from stdin when no prompt argument is set.
- `--json` prints `{"text":"..."}` after the turn instead of streaming deltas.
- `--models` prints `/api/v1/models`.
- `--mint-free-key` calls `POST /api/keys/free` and prints the response once.
- `--artanis` uses the owner-authenticated Artanis operator channel.
- `--count <n>` sets the worker count for `khala spawn` (default 1; local cap
  10, Pylon cap 20).
- `--max-parallel <n>` bounds concurrent workers for `khala spawn`.
- `--objective <text>` supplies the `khala spawn` objective.
- `--strategy auto|local|pylon` selects the spawn strategy. `auto` currently
  resolves to local Codex workers; `pylon` dispatches through caller-owned
  linked Pylon capacity and requires an agent token from `khala login`,
  `OPENAGENTS_AGENT_TOKEN`, or `--token`.
- `--pylon-ref <ref>` targets one caller-owned Pylon for `--strategy pylon`.
- `--fixture` uses the bounded public fixture for `--strategy pylon`.
- `--repo <owner/repo>`, `--branch <name>`, `--commit <sha>`, and
  `--verify <command>` describe public repository work for `--strategy pylon`.
  `--repo`, `--commit`, and `--verify` must be supplied together.
- `--workflow claude_agent_task|codex_agent_task|cloud_coding_session` selects the Pylon coding
  workflow for `--strategy pylon`.
- `--timeout <seconds>` sets the per-worker timeout for `khala spawn`.

## Changelog

### v0.1.18 - Jun 27, 2026, 8:05:24 AM CDT

- Stops `khala info` from printing raw agent tokens or token-bearing trace URLs.
- Keeps `khala info` diagnostic-only: it no longer mints a new trace token just
  to show session details.
- Uses the stored `khala login` token for `khala --api` and
  `khala spawn --strategy pylon` when no `--token` flag or
  `OPENAGENTS_AGENT_TOKEN` is provided.

### v0.1.17 - Jun 27, 2026, 7:42:24 AM CDT

- Adds supervised Khala spawn workers for local Codex-backed subagent fanout.
- Routes natural-language spawn requests to the typed `spawn_khala` tool.
- Bridges `--strategy pylon` to caller-owned Pylon coding capacity.

### v0.1.16 - Jun 26, 2026, 11:43:28 PM CDT

- Shows the logged-in OpenAgents account email instead of conflating the user
  identity with Artanis.
- Clarifies that Artanis is the owner/operator agent reached through `/artanis`.

### v0.1.15 - Jun 26, 2026, 11:00:14 PM CDT

- Adds `khala login` / `/login` through OpenAgents device auth.
- Adds `khala logout` / `/logout`.
- Points Artanis owner-only and `--api` messages at `khala login`.

### v0.1.14 - Jun 26, 2026, 10:26:46 PM CDT

- Adds the owner-authenticated Artanis operator channel through `/artanis`,
  `khala artanis`, and `--artanis`.
- Reads the displayed version from `package.json`.

### v0.1.12 - Jun 26, 2026, 2:45:49 PM CDT

- Adds one-dot-per-second waiting feedback before the first Khala stream output.
- Restores Ctrl-L screen clearing and shows first-byte, first-token, stream, and
  total latency in `/msginfo`.

### v0.1.11 - Jun 26, 2026, 2:01:29 PM CDT

- Adds Blueprint-selected local Codex delegation for workspace, filesystem,
  shell, git, and code tasks.
- Adds `khala auth codex`, `khala codex`, and `/codex` commands with Pylon
  Codex account reuse.

### v0.1.10 - Jun 26, 2026, 1:37:50 PM CDT

- Fixes streamed Markdown rendering when bold spans are split across SSE chunks.
- Records served tokens from the default public Khala chat path so `/tokens`
  moves after successful turns.

### v0.1.9 - Jun 26, 2026, 1:30:01 PM CDT

- Adds `/info` and `khala info` with a CLI thread id plus owner-token trace
  viewing link.
- Rewords `/msginfo` around Khala as the orchestrator and backend models/adapters
  as routing details.

### v0.1.8 - Jun 26, 2026, 1:04:35 PM CDT

- Adds Up/Down prompt history, switches the interactive prompt to `>`, and keeps
  provider reasoning in a separate dim stream.
- Adds the Blueprint response-discipline contract so Khala answers land as one
  coherent final answer instead of visible revision loops.

### v0.1.7 - Jun 26, 2026, 12:36:47 PM CDT

- Shows the installed Khala CLI version in the interactive startup banner.
- Makes `/tokens` and `khala tokens` read the live ledger total without a stale
  isolate cache.

### v0.1.6 - Jun 26, 2026, 12:24:07 PM CDT

- Restored live streaming in interactive chat while keeping Markdown color
  rendering.
- Removed the timestamp from `khala tokens` and aligned Khala fallback docs with
  `GLM -> OpenRouter -> Gemini -> Fireworks`.

### v0.1.5 - Jun 26, 2026, 12:06:14 PM CDT

- Corrected the bundled v0.1.4 release timestamp after npm publish
  verification.
- Kept the v0.1.4 diagnostics, Markdown, help, version, and msginfo changes as
  the active CLI.

### v0.1.4 - Jun 26, 2026, 12:05:31 PM CDT

- Added `/help`, `/version`, `/msginfo`, Markdown rendering, colors, and faded
  metadata.
- Added backend trace reporting, public stream metadata, exact feedback lookup,
  and longer exponential retries.

### v0.1.3 - Jun 26, 2026, 11:50:00 AM CDT

- Added background npm auto-update checks for interactive sessions.
- Added `khala tokens` and `/tokens` backed by the public Khala tokens-served
  counter.

### v0.1.2 - Jun 26, 2026, 11:38:47 AM CDT

- Added `/feedback` and `khala feedback` for out-of-band product feedback.
- Added `/changelog` and `khala changelog`, plus clearer retrying and terminal
  errors for unavailable inference.

### v0.1.1 - Jun 26, 2026, 11:12:03 AM CDT

- Replaced the full-screen alternate prompt with a normal scrollback chat
  transcript.
- Removed runtime npm dependencies so global installs avoid unrelated engine
  warnings.

### v0.1.0 - Jun 26, 2026, 11:02:59 AM CDT

- Initial Khala command with interactive terminal chat and headless prompt/stdin
  modes.
- Published the OpenAI-compatible Khala client as `@openagentsinc/khala`.
