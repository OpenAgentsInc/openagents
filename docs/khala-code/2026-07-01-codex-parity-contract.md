# Khala Code Codex Parity Contract

Date: 2026-07-01
Tracking epic: <https://github.com/OpenAgentsInc/openagents/issues/7780>
Issue: <https://github.com/OpenAgentsInc/openagents/issues/7793>
Pinned Codex reference: `db887d03e1f907467e33271572dffb73bceecd6b`

Khala Code's default coding harness is Codex app-server. Parity is guarded by
checked fixture tests plus one opt-in live smoke. The fixture tests do not
require a live Codex login; they read the checked-out Codex reference schema and
source under `projects/repos/codex`.

## Fixture Contract

- `clients/khala-code-desktop/tests/codex-parity-contract.test.ts` pins the
  Codex reference commit, generated app-server schema files, client request
  methods, server request methods, notifications, and `ThreadItem` variants.
- `clients/khala-code-desktop/tests/codex-slash-commands.test.ts` parses the
  upstream Codex `SlashCommand` enum and compares it to Khala's registry.
- `clients/khala-code-desktop/tests/codex-app-server-chat-runtime.test.ts`
  covers thread start, resume, list, read, rename, fork, archive, unarchive,
  delete, turn start, and turn interrupt over app-server methods.
- `clients/khala-code-desktop/tests/codex-thread-item-projector.test.ts`
  replays representative app-server `ThreadItem` variants and delta families.
- `clients/khala-code-desktop/tests/codex-approval-decisions.test.ts` compares
  exact app-server approval response bodies for command, file-change, and
  permissions paths.
- `clients/khala-code-desktop/tests/rpc-handlers.test.ts` covers Codex model,
  permission profile, config read/write, feature flags, usage/status, MCP,
  plugin, skill, app, and hook RPC pass-through.
- `clients/khala-code-desktop/tests/headless.test.ts` covers Codex-backed
  headless JSONL mode and missing-Codex errors.
- `clients/khala-code-desktop/tests/khala-chat-runtime.test.ts` is explicitly
  legacy fallback coverage, not default Codex-wrapper coverage.

If the pinned Codex checkout changes, update
`KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT` and any schema/method/variant rows in
`src/bun/codex-parity-contract.ts` in the same change. A new upstream slash
command, app-server method removal, server request change, notification change,
or `ThreadItem` variant creates a fixture failure until Khala explicitly maps it
or records it as a gap.

## Live Smoke

Run without opt-in:

```sh
bun run --cwd clients/khala-code-desktop smoke:codex-parity-live
```

Expected: `ok: true`, `skipped: true`, and a reason naming the required opt-in.

Run with a real Codex install and auth:

```sh
KHALA_CODE_DESKTOP_CODEX_PARITY_LIVE_SMOKE=1 \
  bun run --cwd clients/khala-code-desktop smoke:codex-parity-live -- --require-live
```

Expected: the smoke checks the Codex harness gate, starts app-server, creates and
resumes a temporary thread, starts a harmless turn, attempts `turn/interrupt`,
and exits with `ok: true`. If live mode is explicitly required and Codex is
missing or unauthenticated, it exits nonzero with a structured reason instead of
silently skipping.
