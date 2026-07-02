# Khala Code ThreadItem Coverage

Status: ROADMAP_QA Q4.2 / issue #8028 implemented.

The ThreadItem corpus is pinned to the Codex parity contract through
`clients/khala-code-desktop/src/bun/codex-thread-item-fixtures.ts`.
That module records the Codex reference commit and exports exactly one fixture
row for every `KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES` variant.

The QA seed corpus consumes that same fixture source for the `thread_items`
scenario group. Each scenario reads a `thread-item-<variant>` fixture thread,
and the fixture RPC response includes the source metadata plus the raw fixture
row under `thread.parityFixture`.

Coverage flows through the existing ledger fields:

- `threadItemVariants`
- `threadItemVariantsRendered`
- `threadItemVariantRenderCounts`

The tests pin both sides:

- the desktop projector test asserts the shared fixture list matches the
  parity-contract variant list and reference commit;
- the harness seed-corpus test asserts the manifest exposes the fixture source
  and one scenario per variant;
- the coverage-ledger test asserts every variant is counted after a full seed
  corpus run.
