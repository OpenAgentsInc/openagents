# Khala Code Codex App-server Gap Matrix

Date: 2026-07-01
Tracking epic: <https://github.com/OpenAgentsInc/openagents/issues/7780>
Issue: <https://github.com/OpenAgentsInc/openagents/issues/7795>
Pinned Codex reference: `db887d03e1f907467e33271572dffb73bceecd6b`
Checked fixture: `clients/khala-code-desktop/src/bun/codex-app-server-gap-matrix.ts`

Khala Code is pivoting to a direct wrapper and desktop port of the Codex
harness. That means feature parity should come from Codex app-server wherever a
contract exists. TUI-local behavior must not be copied into TypeScript unless
the gap matrix names why the adapter is desktop-local, links a test, and keeps
the behavior small.

## Decision Key

- `covered_by_app_server`: Codex exposes stable app-server methods. Khala can
  build richer desktop navigation or layout, but the state and mutations must
  round-trip through app-server.
- `khala_adapter_with_test`: The command is a desktop shell affordance such as
  copy, clear, title, or diagnostics. Khala can own a tiny adapter if a test
  covers it and the adapter does not become a second Codex Core.
- `upstream_app_server_gap`: Codex TUI owns behavior that Khala needs for full
  parity. Khala should request a narrow app-server method or metadata contract
  before implementing the behavior.

## Matrix

| Row | Decision | Slash commands | Stable app-server methods | Experimental methods | Gap or adapter |
| --- | --- | --- | --- | --- | --- |
| `thread-turn-session-lifecycle` | `covered_by_app_server` | `/review`, `/rename`, `/new`, `/archive`, `/delete`, `/resume`, `/fork`, `/compact`, `/goal` | `review/start`, `thread/name/set`, `thread/start`, `thread/archive`, `thread/delete`, `thread/resume`, `thread/fork`, `thread/compact/start`, `thread/goal/set`, `thread/goal/get`, `thread/goal/clear`, `thread/read`, `thread/list`, `turn/start`, `turn/steer`, `turn/interrupt` | None | Use Codex thread, turn, review, compact, and goal APIs directly. |
| `settings-ecosystem-account-surfaces` | `covered_by_app_server` | `/model`, `/permissions`, `/experimental`, `/usage`, `/mcp`, `/app`, `/apps`, `/plugins`, `/logout` | `model/list`, `modelProvider/capabilities/read`, `permissionProfile/list`, `experimentalFeature/list`, `experimentalFeature/enablement/set`, `account/usage/read`, `account/rateLimits/read`, `account/rateLimitResetCredit/consume`, `account/read`, `mcpServerStatus/list`, `mcpServer/resource/read`, `mcpServer/tool/call`, `mcpServer/oauth/login`, `config/mcpServer/reload`, `app/list`, `plugin/list`, `plugin/read`, `plugin/installed`, `account/logout`, `config/read`, `config/value/write`, `config/batchWrite`, `configRequirements/read` | None | Khala owns the web settings panels, but values and mutations remain Codex app-server state. |
| `desktop-local-ui-adapters` | `khala_adapter_with_test` | `/copy`, `/raw`, `/status`, `/debug-config`, `/title`, `/quit`, `/exit`, `/feedback`, `/rollout`, `/clear`, `/test-approval` | `config/read`, `account/usage/read`, `feedback/upload`, `getAuthStatus`, `getConversationSummary` | None | Tiny desktop adapters for browser selection, window title, visible transcript, local status, and debug diagnostics. |
| `tui-preferences-and-appearance` | `upstream_app_server_gap` | `/keymap`, `/vim`, `/statusline`, `/theme`, `/pets`, `/personality` | `config/read`, `config/value/write`, `config/batchWrite` | None | `codex.app_server.gap.tui_preferences` |
| `workspace-knowledge-memory-and-import` | `upstream_app_server_gap` | `/memories`, `/skills`, `/import`, `/hooks`, `/init`, `/debug-m-drop`, `/debug-m-update` | `skills/list`, `skills/config/write`, `skills/extraRoots/set`, `hooks/list`, `externalAgentConfig/detect`, `externalAgentConfig/import`, `externalAgentConfig/import/readHistories`, `fs/readFile`, `fs/writeFile`, `fs/getMetadata` | None | `codex.app_server.gap.memory_and_import_management` |
| `multi-agent-side-conversation-and-plan` | `upstream_app_server_gap` | `/approve`, `/plan`, `/agent`, `/subagents`, `/side`, `/btw` | `turn/steer`, `thread/fork`, `thread/inject_items`, `thread/approveGuardianDeniedAction`, `thread/metadata/update`, `thread/read` | None | `codex.app_server.gap.side_agent_plan_controls` |
| `ide-file-mention-and-diff` | `upstream_app_server_gap` | `/ide`, `/diff`, `/mention` | `fuzzyFileSearch`, `gitDiffToRemote`, `fs/readDirectory`, `fs/readFile`, `config/read` | None | `codex.app_server.gap.ide_mentions_diff` |
| `windows-sandbox-setup-and-readable-roots` | `upstream_app_server_gap` | `/setup-default-sandbox`, `/sandbox-add-read-dir` | `windowsSandbox/setupStart`, `windowsSandbox/readiness`, `config/read`, `config/value/write` | None | `codex.app_server.gap.windows_sandbox_read_roots` |
| `background-terminal-management` | `upstream_app_server_gap` | `/ps`, `/stop` | None | `thread/backgroundTerminals/list`, `thread/backgroundTerminals/clean`, `thread/backgroundTerminals/terminate` | `codex.app_server.gap.background_terminals` |

## Upstream-ready Gap Names

These identifiers are intentionally small enough to become Codex PR titles,
GitHub issues, or protocol notes:

- `codex.app_server.gap.tui_preferences`: expose preference metadata and
  mutation semantics for keymap, Vim, statusline, theme, pets, and personality.
- `codex.app_server.gap.memory_and_import_management`: expose memory list,
  memory mutation, import, and `AGENTS.md` init semantics instead of relying on
  TUI-only flows.
- `codex.app_server.gap.side_agent_plan_controls`: expose side-conversation,
  subagent, plan-edit, and auto-review controls as app-server methods or
  metadata.
- `codex.app_server.gap.ide_mentions_diff`: expose IDE integration state,
  mention candidates, and diff-read semantics through app-server.
- `codex.app_server.gap.windows_sandbox_read_roots`: expose readable-root
  mutation in the Windows sandbox contract.
- `codex.app_server.gap.background_terminals`: stabilize the existing
  experimental background terminal list, clean, and terminate methods.

## Update Rule

Whenever `KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT` changes, update this
document and `codex-app-server-gap-matrix.ts` in the same commit. Then rerun:

```sh
bun test clients/khala-code-desktop/tests/codex-app-server-gap-matrix.test.ts \
  clients/khala-code-desktop/tests/codex-parity-contract.test.ts \
  clients/khala-code-desktop/tests/codex-slash-commands.test.ts
```

The matrix test enforces that:

- every Codex slash command is present exactly once;
- every stable app-server method exists in the generated Codex schema;
- experimental background terminal methods stay labeled experimental;
- every TUI-local behavior has either a named upstream gap or a tested Khala
  adapter rationale;
- this human-readable document contains every row id and upstream gap id.
