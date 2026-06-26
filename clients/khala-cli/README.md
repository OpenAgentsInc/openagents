# Khala CLI

Small Bun + Effect terminal client for Khala.

```sh
bunx @openagentsinc/khala --prompt "Say hello in one short sentence."
khala feedback "The input ate my transcript."
khala changelog
khala tokens
khala version
bun run khala
bun run khala -- --prompt "Say hello in one short sentence."
printf 'Say OK only.' | bun run khala -- --headless --json
```

The published npm package is `@openagentsinc/khala`; it exposes the `khala`
command through a Bun shebang, so Bun 1.3+ must be available on `PATH`.

"Khala CLI" names the command-line program surface: the `khala` command. It has
two modes:

- **Interactive terminal mode:** default `khala` opens a normal terminal chat
  transcript with persistent `You:` and `Khala:` turns in scrollback. It does
  not use an alternate-screen/full-screen UI. Slash commands such as
  `/feedback`, `/msginfo`, `/tokens`, `/changelog`, `/version`, and `/help` are
  handled locally instead of being sent to inference.
- **Headless CLI mode:** `--prompt`, positional text, or stdin runs one turn and
  streams the assistant response to stdout for scripts and agents.
- **Utility commands:** `khala feedback "..."` saves feedback to
  `POST /api/khala/feedback`, `khala tokens` reads the public Khala
  tokens-served counter, and `khala changelog` prints the recent package
  changelog.

Interactive mode checks npm for a newer `@openagentsinc/khala` in the
background. If a newer version installs successfully, it prints one line and
the next `khala` launch uses the update. Set `KHALA_NO_AUTO_UPDATE=1` to skip
that check.

## Interactive commands

- `/feedback <text>` saves product feedback without sending the text to
  inference. When a trace reference is available, the CLI includes it with the
  feedback.
- `/msginfo` prints the last Khala response metadata: trace reference, model and
  adapter routing, fallback reason, token counts, and tokens per second when
  reported by the backend.
- `/tokens` prints the global Khala tokens-served count from the same public
  counter shown on `openagents.com` and `/khala`.
- `/changelog` prints the five most recent CLI releases in reverse
  chronological order.
- `/version` prints the installed CLI version.
- `/help` lists slash commands.
- `/exit` quits.

## Utility commands

- `khala feedback "text"` sends feedback from scripts or a shell. This command
  may not have a chat trace reference, which is expected.
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

## Changelog

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
