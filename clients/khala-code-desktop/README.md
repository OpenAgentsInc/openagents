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

## Swarm Delegation

Khala's swarm layer sits outside the main local Codex session. The chat loop is
the Codex app-server harness; `codex_spawn` means "delegate this bounded
Codex-backed task to isolated Khala/Pylon worker sessions." Fleet shows the main
Codex session separately from worker sessions, including worker readiness,
capacity, queue/refill policy, cooldown state, active assignments, transcript
refs, closeout state, and token proof.

`codexFleetPromoteThread()` promotes a current Codex thread into a swarm
delegation request only with explicit context boundaries. It carries the origin
`sessionId`/`threadId`, an explicit objective, optional public refs, and a
user-written summary; it does not copy the local transcript into the worker
prompt.

The legacy hosted Khala/OpenRouter runtime is a fallback/prototype path, not the
Codex-parity default. When it is explicitly enabled, OpenRouter BYOK is passed to
hosted Khala instead of being used as a local model backend:

```sh
OPENAGENTS_AGENT_TOKEN=... OPENROUTER_API_KEY=... bun run dev
```

`OPENROUTER_API_KEY` alone is not enough for the legacy hosted path.

## Tools

The Codex-parity path should use Codex app-server tools, approvals, sandboxing,
MCP, plugins, skills, and session state. The default desktop `toolCatalog()`
therefore exposes only Khala's supplemental swarm/Pylon tools around Codex:

- `pylon_ensure`
- `codex_fleet_status`
- `codex_spawn`

The older Khala-native tool runtime is explicitly legacy/fallback. It is enabled
only with `KHALA_CODE_DESKTOP_RUNTIME=khala_native_runtime` or
`KHALA_CODE_DESKTOP_LEGACY_KHALA_NATIVE_RUNTIME=1`, and legacy turns are labeled
in the transcript. That legacy mode may still register Codex-equivalent
`@openagentsinc/khala-tools` helpers for testing and fallback work:

- workspace inspection and edits: `read`, `ls`, `glob`, `grep`, `edit`,
  `write`, `apply_patch`
- process control: `exec_command`, `write_stdin`
- planning and local UX: `ask_user`, `todo_write`, `view_image`
- network: `web_fetch`, `web_search`
- browser: `browser_navigate`, `browser_click`, `browser_type`,
  `browser_read_text`, `browser_read_dom`, `browser_wait_for`,
  `browser_screenshot`

Tool calls are executed by the Bun host through `@openagentsinc/khala-tools`.
In default Codex-harness mode, Codex owns those local coding capabilities and
Khala tools stay supplemental rather than becoming a second coding harness.

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
