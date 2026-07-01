# Khala Code Desktop

Khala Code Desktop is the Electrobun wrapper for local Codex coding work. The
default product path requires the `codex` CLI and a signed-in main Codex home.
Khala adds the desktop shell, sidebar, Inbox, Fleet, Gym/proof panes, and Pylon
swarm controls around that Codex harness.

## Backends

The default harness is the user's local Codex install:

```sh
npm install -g @openai/codex
codex login
bun run dev
```

Khala Code checks the Codex binary, version, main Codex home, and auth state
before enabling the default coding harness. `CODEX_HOME` may point the main
wrapper session at an explicit home; otherwise the normal `~/.codex` home is
used. To use a non-`PATH` Codex binary, set `KHALA_CODE_CODEX_BINARY` or
`KHALA_CODE_CODEX_COMMAND`.

Fleet accounts are separate: Pylon/Khala worker accounts use isolated homes
under the Pylon account directory. The desktop app must not run `codex login`
against the user's default home automatically and must not reuse the main user
home for worker accounts.

The legacy hosted Khala/OpenRouter runtime is a fallback/prototype path, not the
Codex-parity default. When it is explicitly enabled, OpenRouter BYOK is passed to
hosted Khala instead of being used as a local model backend:

```sh
OPENAGENTS_AGENT_TOKEN=... OPENROUTER_API_KEY=... bun run dev
```

`OPENROUTER_API_KEY` alone is not enough for the legacy hosted path.

## Tools

The Codex-parity path should use Codex app-server tools, approvals, sandboxing,
MCP, plugins, skills, and session state. The current Khala tool presets remain
available only for legacy/fallback and Khala swarm orchestration while the pivot
lands:

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
