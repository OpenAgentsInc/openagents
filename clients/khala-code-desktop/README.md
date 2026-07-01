# Khala Code Desktop

Khala Code Desktop is the Electrobun chat surface for local coding work. The
first screen is the chat transcript and composer; the Bun host owns model
transport and native tool execution.

## Backends

The desktop host routes model traffic through the hosted OpenAgents cloud:

```sh
OPENAGENTS_AGENT_TOKEN=... bun run dev
```

OpenRouter BYOK is passed to hosted Khala instead of being used as a local model
backend. An account-attached OpenRouter key is used automatically by the hosted
gateway. A request-specific key, when set, takes precedence for that request:

```sh
OPENAGENTS_AGENT_TOKEN=... OPENROUTER_API_KEY=... bun run dev
```

`OPENROUTER_API_KEY` alone is not enough: the desktop app cannot run the full
Khala system locally. If `OPENAGENTS_AGENT_TOKEN` is missing, the chat box
returns a setup message instead of pretending a model answered.

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

Composer HUD visual regression coverage is a local warning-only lane because it
requires a working Chromium + WebGL stack. It launches the Khala Code desktop
Vite preview and the OpenAgents web Vite preview, types only a fixed synthetic
prompt, captures desktop/mobile screenshots, and asserts composer framing,
footer non-overlap, reduced-motion geometry, and nonblank HUD/canvas pixels.
Artifacts are written under ignored `var/khala-code-desktop/composer-visual-smoke`.

```sh
bun run smoke:composer-visual
```

For a browser-driven smoke without opening the native window:

```sh
KHALA_CODE_DESKTOP_OPEN_WINDOW=0 KHALA_CODE_DESKTOP_PREVIEW_PORT=50121 bun src/bun/index.ts
```

Then open `http://localhost:50121` and use the preview RPC bridge to submit a
chat turn or inspect `toolCatalog`.
