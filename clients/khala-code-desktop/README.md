# Khala Code Desktop

Khala Code Desktop is the Electrobun chat surface for local coding work. The
first screen is the chat transcript and composer; the Bun host owns model
transport and native tool execution.

## Backends

The desktop host prefers the hosted OpenAgents cloud by default:

```sh
OPENAGENTS_AGENT_TOKEN=... bun run dev
```

To use a personal OpenRouter account instead, set `OPENROUTER_API_KEY`. The
desktop host then sends OpenAI-compatible chat-completion requests to
OpenRouter, using `OPENROUTER_MODEL` when present.

```sh
OPENROUTER_API_KEY=... OPENROUTER_MODEL=anthropic/claude-sonnet-4 bun run dev
```

If neither credential is set, the chat box returns a setup message instead of
pretending a model answered.

## Tools

All Khala tool presets are enabled in this desktop app by default:

- workspace inspection and edits: `read`, `ls`, `glob`, `grep`, `edit`,
  `write`, `apply_patch`
- process control: `exec_command`, `write_stdin`
- planning and local UX: `ask_user`, `todo_write`, `view_image`
- network: `web_fetch`, `web_search`
- browser: `browser_navigate`, `browser_click`, `browser_type`,
  `browser_read_text`, `browser_read_dom`, `browser_wait_for`,
  `browser_screenshot`

Tool calls are executed by the Bun host through `@openagentsinc/khala-tools`.
The chat model only sees bounded model-visible tool output; private artifacts,
browser state, screenshots, and local file contents stay on the local tool
result lanes owned by the host.

## Local Checks

```sh
bun run typecheck
bun test tests/*.test.ts
bun run verify
```

For a browser-driven smoke without opening the native window:

```sh
KHALA_CODE_DESKTOP_OPEN_WINDOW=0 KHALA_CODE_DESKTOP_PREVIEW_PORT=50121 bun src/bun/index.ts
```

Then open `http://localhost:50121` and use the preview RPC bridge to submit a
chat turn or inspect `toolCatalog`.
