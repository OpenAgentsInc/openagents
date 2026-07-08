# @openagentsinc/harness-conformance

MH-1 (issue #8582). An enum-driven harness conformance suite that steals
effect-native's `componentTags` renderer-conformance trick and applies it to
coding harnesses. Adding a coding-harness kind **reds the sweep** until that
kind proves five capabilities:

- **(a) chat runtime** — `startThread` / `startTurn` / `interrupt` / `resume`
  map onto `khala.chat_turn_event.v1`.
- **(b) worker executor** — claim → pinned worktree (exact repo/commit/branch +
  verify command) → closeout with verify, honoring the own-capacity no-spend
  settlement invariants.
- **(c) capacity/readiness probe** — typed readiness + non-negative capacity
  refs.
- **(d) metering honesty** — exact token fields when present; the
  `not_measured` sentinel otherwise; **never synthesized/invented tokens**.
- **(e) typed failure classes** — including `account_exhausted`,
  `account_rate_limited`, and `account_quota_exhausted` (never a generic error).

## Status

| Harness | State | Owner |
| --- | --- | --- |
| `codex` | GREEN (real fixture) | MH-1 |
| `claude_code` | GREEN (real fixture) | MH-1 |
| `grok_cli` | RED by design (`test.todo`, pending) | Grok MH-3/MH-4 |

The codex/claude fixtures are backed by real runtime surfaces: the desktop
`KhalaCodeDesktopChatTurnEvent` schema (chat), and the pylon
`classifySessionError` + pylon-core `classifyQuotaSignal` classifiers (typed
failures).

## Two enforcement teeth

1. **Compile-time.** `harnessKindClassification` is
   `satisfies Record<AgentDefinitionHarnessKind, …>` and the registry is
   `satisfies Record<CodingWorkerHarnessKind, …>`, so a new enum literal breaks
   `typecheck` until it is both classified and registered.
2. **Run-time.** Proven kinds run the full five-capability suite (green);
   pending kinds emit `test.todo` (visible red) and are checked against a
   known-pending allowlist, so an *unexpected* pending coding kind fails the
   coverage gate.

## Filling fixtures for a new harness (e.g. `grok_cli`)

1. Author a `HarnessConformanceFixture` under `src/fixtures/`.
2. Flip the registry entry in `src/registry.ts` from `pending` to
   `{ status: "proven", fixture }` and drop the kind from
   `knownPendingHarnessKinds`.
3. Run `bun run --cwd packages/harness-conformance test`.

Runs in the normal `bun test` / typecheck sweep via `test:harness-conformance`
and `typecheck:harness-conformance`. Not wired into CI/GitHub Actions.
