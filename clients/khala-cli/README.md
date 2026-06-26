# Khala CLI

Small Bun + Effect terminal client for Khala.

```sh
bunx @openagentsinc/khala --prompt "Say hello in one short sentence."
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
  not use an alternate-screen/full-screen UI.
- **Headless CLI mode:** `--prompt`, positional text, or stdin runs one turn and
  streams the assistant response to stdout for scripts and agents.

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
