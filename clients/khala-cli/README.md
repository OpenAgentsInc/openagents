# Khala CLI

Small Bun + Effect terminal client for Khala.

```sh
bunx @openagentsinc/khala --prompt "Say hello in one short sentence."
khala feedback "The input ate my transcript."
khala changelog
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
  `/feedback` and `/changelog` are handled locally instead of being sent to
  inference.
- **Headless CLI mode:** `--prompt`, positional text, or stdin runs one turn and
  streams the assistant response to stdout for scripts and agents.
- **Utility commands:** `khala feedback "..."` saves feedback to
  `POST /api/khala/feedback`, and `khala changelog` prints the recent package
  changelog.

## Interactive commands

- `/feedback <text>` saves product feedback without sending the text to
  inference. When a trace reference is available, the CLI includes it with the
  feedback.
- `/changelog` prints the five most recent CLI releases in reverse
  chronological order.
- `/exit` quits.

## Utility commands

- `khala feedback "text"` sends feedback from scripts or a shell. This command
  may not have a chat trace reference, which is expected.
- `khala changelog` prints recent releases.

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

### v0.1.2 - 2026-06-26

- Added `/feedback` and `khala feedback` for out-of-band product feedback.
- Added `/changelog` and `khala changelog`, plus clearer retrying and terminal
  errors for unavailable inference.

### v0.1.1 - 2026-06-26

- Replaced the full-screen alternate prompt with a normal scrollback chat
  transcript.
- Removed runtime npm dependencies so global installs avoid unrelated engine
  warnings.

### v0.1.0 - 2026-06-26

- Initial Khala command with interactive terminal chat and headless prompt/stdin
  modes.
- Published the OpenAI-compatible Khala client as `@openagentsinc/khala`.
