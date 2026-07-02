# Khala Code Desktop

Khala Code Desktop is the Electrobun wrapper for local Codex coding work. The
default product path requires the `codex` CLI and a signed-in main Codex home.
Khala adds the desktop shell, sidebar, Inbox, Fleet, Gym/proof panes, and Pylon
swarm controls around that Codex harness.

## Install And First Run

Khala Code does not bundle or reimplement Codex Core. The default harness is the
user's local Codex install and the `codex app-server --stdio` protocol exposed
by that install:

```sh
npm install -g @openai/codex
codex login
bun run dev
```

Khala Code checks the Codex binary, version, main Codex home, and auth state
before enabling the default coding harness. If the binary is missing, install
Codex or set `KHALA_CODE_CODEX_BINARY` / `KHALA_CODE_CODEX_COMMAND`. If auth is
missing, run `codex login` yourself for the primary user Codex home before
using the default chat surface.

`CODEX_HOME` may point the main wrapper session at an explicit primary home;
otherwise the normal `~/.codex` home is used. Khala Code never starts
`codex login` against that primary home automatically.

Fleet accounts are separate: Pylon/Khala worker accounts use isolated homes
under the Pylon account directory. The desktop app must not run `codex login`
against the user's default home automatically and must not reuse the primary
user home for worker accounts.

## Product Boundary

The product center is: Codex owns coding-agent execution; Khala Code wraps it in
a desktop/web shell. Future work should extend the wrapper around Codex
app-server state rather than rebuilding Codex Core behavior in TypeScript. The
desktop app adds:

- sidebar and thread navigation over Codex threads;
- Unified Inbox projection for approvals, MCP/auth blockers, and worker
  closeouts;
- Settings and ecosystem panels backed by Codex app-server processors;
- Fleet and Pylon swarm controls around isolated worker Codex accounts;
- Gym/proof panes and smoke-test harnesses for desktop advantages.

## Iconography

Khala Code uses only the OpenAI Apps SDK icon catalog from
`@openagentsinc/ui/icon`. Foldkit surfaces should use `iconView`; direct DOM
surfaces should use `iconElement` from `@openagentsinc/ui/icon-dom`. Do not use
ASCII glyph placeholders, emoji, hand-authored SVGs, icon fonts, Lucide, or
visible words as stand-ins for icon controls.

Tracking context:

- Audit: [docs/khala-code/2026-07-01-codex-harness-wrapper-port-audit.md](../../docs/khala-code/2026-07-01-codex-harness-wrapper-port-audit.md)
- Effect patterns: [docs/khala-code/effect-patterns.md](../../docs/khala-code/effect-patterns.md)
- Parity contract: [docs/khala-code/2026-07-01-codex-parity-contract.md](../../docs/khala-code/2026-07-01-codex-parity-contract.md)
- Product positioning: [docs/khala-code/2026-07-01-codex-required-product-positioning.md](../../docs/khala-code/2026-07-01-codex-required-product-positioning.md)
- Tracking epic: [OpenAgentsInc/openagents#7780](https://github.com/OpenAgentsInc/openagents/issues/7780)

## Swarm Delegation

Khala's swarm layer sits outside the primary local Codex session. The chat loop is
the Codex app-server harness; `codex_spawn` means "delegate this bounded
Codex-backed task to isolated Khala/Pylon worker sessions." Fleet shows the
primary Codex session separately from worker sessions, including worker readiness,
capacity, queue/refill policy, cooldown state, active assignments, transcript
refs, closeout state, and token proof.

Default desktop chat turns also register a local Codex MCP server named
`khala_fleet`. That is the message-triggered bridge: a normal Codex-backed
message can call the MCP `codex_spawn` tool, which runs the deterministic
`khala.fleet.delegate` program. The bridge exposes only `pylon_ensure`,
`codex_fleet_status`, and `codex_spawn`, with Codex MCP approval mode set to
`prompt`. Set `KHALA_CODE_DESKTOP_FLEET_MCP_BRIDGE=0` to disable registration
while debugging.

`codexFleetPromoteThread()` promotes a current Codex thread into a swarm
delegation request only with explicit context boundaries. It carries the origin
`sessionId`/`threadId`, an explicit objective, optional public refs, and a
user-written summary; it does not copy the local transcript into the worker
prompt.

The Fleet screen also exposes a direct `khala.fleet.delegate` runner for
operator smoke tests and pinned real-work dispatches. Fixture mode can run a
single bounded worker delegation without shell access; real-work mode is
locally refused until `repo`, `commit`, and `verify` pins are complete. The UI
projection shows module/precondition/status/fallback/blocker/public refs only:
it does not echo the raw objective, raw trace messages, local paths, provider
payloads, credentials, or bearer material.

The same Fleet surface owns the Part 2 delegation-optimization preview. The
**Optimize delegation policy** and **Load demo proof** controls project a
`khala-code-delegation-gepa` run into Fleet and open the read-only Gym pane with
run refs, dataset refs, candidate/admission refs, blocker refs, and the Action
Submission proposal ref. The active-parameter readout stays on
`openagents.khala.fleet_delegation.parameters.v0` `source=default` until an
owner-approved admitted candidate can become live delegation input.

For the transcript-245 Part 2 recording path, run the UI-first smoke:

```sh
bun run smoke:part2-ui
```

It launches the preview UI, mocks only safe preview RPC responses, clicks the
Fleet hotbar, runs the deterministic delegate form, starts **Optimize delegation
policy**, and validates the Gym proof pane without URL flags or console helpers.

The legacy hosted Khala/OpenRouter runtime is a fallback/prototype path, not the
Codex-parity default and not the local coding engine. When it is explicitly
enabled, OpenRouter BYOK is passed to hosted Khala instead of being used as a
local model backend:

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

Headless JSONL mode also uses the Codex app-server harness. It requires the
`codex` CLI and a usable Codex auth home, starts a Codex-backed thread, streams
Codex-derived item notifications to stderr, and writes one final JSON object to
stdout. JSONL events include stable desktop ids plus Codex correlation ids such
as `thread_id` and `codex_turn_id`. If Codex is missing or unauthenticated, the
command exits nonzero and emits a structured `codex_app_server_unavailable`
error.

```sh
bun src/bun/index.ts --json "Summarize this repository."
KHALA_CODE_HEADLESS_INTERRUPT_AFTER_MS=250 bun src/bun/index.ts --json "Start a long read-only turn."
```

Composer HUD visual regression coverage is a local warning-only lane because it
requires a working Chromium + WebGL stack. It launches the Khala Code desktop
Vite preview and the OpenAgents web Vite preview, types only a fixed synthetic
prompt, captures desktop/mobile screenshots, and asserts composer framing,
footer non-overlap, reduced-motion geometry, and nonblank HUD/canvas pixels.
Fleet/Gym source-level tests also assert the delegation-optimization UI hooks,
active-parameter readout, and public-safe proof-loading path. The Part 2
Fleet/Gym visual smoke clicks Fleet, starts **Optimize delegation policy** into
Gym, captures desktop/mobile screenshots, and checks the proof graph plus
active-parameter cards for visible, non-overlapping geometry.
This is a preview UI geometry smoke only; it does not submit a model turn and
does not exercise the legacy Khala-native shell/process runtime. The composer
JSON summary is labeled `preview_ui_codex_harness_shell`; the Fleet/Gym summary
is labeled `khala_code_part2_fleet_gym_visual_smoke`.
Artifacts are written under ignored `var/khala-code-desktop/composer-visual-smoke`.
Screenshot baseline comparison is wired through
[`docs/qa/khala-code-visual-baselines.md`](../../docs/qa/khala-code-visual-baselines.md):
missing baselines soft-report by default, existing baseline diffs fail and
write delta PNGs, and `--bless-baselines` updates the committed baseline store.

```sh
bun run smoke:composer-visual
bun run smoke:composer-visual-preview
bun run smoke:part2-fleet-gym-visual
bun run smoke:part2-ui
```

Live delegation coverage is the guarded Codex spawn lane. It exercises
Pylon-backed worker Codex sessions via the `codex_spawn` wrapper and labels its
JSON summary `pylon_codex_spawn_live`.

```sh
KHALA_CODE_DESKTOP_LIVE_CODEX_SPAWN_SMOKE=1 bun run smoke:codex-spawn-live -- --fixture
```

Codex parity live smoke is skip-safe by default. It exits successfully with a
`skipped` JSON result unless explicitly required. When required, it verifies the
main Codex install/auth gate, starts `codex app-server`, creates and resumes a
temporary thread, submits a harmless prompt, attempts interruption, and shuts the
host down cleanly.

```sh
bun run smoke:codex-parity-live
KHALA_CODE_DESKTOP_CODEX_PARITY_LIVE_SMOKE=1 bun run smoke:codex-parity-live -- --require-live
```

For a browser-driven smoke without opening the native window:

```sh
KHALA_CODE_DESKTOP_OPEN_WINDOW=0 KHALA_CODE_DESKTOP_PREVIEW_PORT=50121 bun src/bun/index.ts
```

Then open `http://localhost:50121` and use the preview RPC bridge to submit a
Codex-backed chat turn or inspect `toolCatalog`. In default mode the preview
bridge routes chat and process-equivalent coding controls through Codex
app-server APIs; the legacy Khala-native shell/process tools require the
explicit legacy runtime flags described above.
