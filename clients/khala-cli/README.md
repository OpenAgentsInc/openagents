# Khala CLI

Small Bun + Effect terminal client for Khala.

```sh
bunx @openagentsinc/khala --prompt "Say hello in one short sentence."
khala feedback "The input ate my transcript."
khala changelog
khala tokens
khala info
khala version
khala login
khala logout
khala auth codex
khala codex "read README.md"
khala --artanis --prompt "status"
bun run khala
bun run khala -- --prompt "Say hello in one short sentence."
printf 'Say OK only.' | bun run khala -- --headless --json
```

The published npm package is `@openagentsinc/khala`; it exposes the `khala`
command through a Bun shebang, so Bun 1.3+ must be available on `PATH`.

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

## Utility commands

- `khala feedback "text"` sends feedback from scripts or a shell. This command
  may not have a chat trace reference, which is expected.
- `khala info` prints a one-shot CLI thread id and trace viewing link.
- `khala login` starts OpenAgents device auth.
- `khala logout` clears the local OpenAgents token.
- `khala auth codex` connects a Codex account for local workspace delegation.
- `khala codex status` shows the active local Codex credential source.
- `khala codex "task"` delegates directly to local Codex.
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

## Changelog

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
