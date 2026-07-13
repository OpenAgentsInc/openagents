# Desktop local-turn restart recovery receipt

- Date: 2026-07-13
- Issue: [#8744](https://github.com/OpenAgentsInc/openagents/issues/8744)
- Final disposition: closed after packaged two-process restart proof
- Scope: Desktop-local Codex/Fable turns only; autonomous goals and audio
  capture lifecycle remain separate contracts

## Landed contract

Electron main writes an Effect-Schema-validated journal before provider
dispatch under the Desktop user-data directory. The parent is mode 0700 and
the atomic pending/final files are mode 0600. Each bounded record is keyed by
thread, turn, and provider lane and contains deterministic user/assistant
message keys, selected account, provider session identity, model, phase,
persisted cursor, ordered assistant segments, recovery generation, and one
terminal disposition. Invalid journal bytes fail closed instead of erasing an
unfinished turn.

Provider deltas remain live in the renderer while main coalesces private
checkpoints at a bounded cadence. A display-bearing non-text event flushes and
closes the active assistant segment before its tool/reasoning/system note is
persisted, so reload preserves the visible ordering contract. Application quit
flushes pending text before disposing provider runtimes.

At startup, main hydrates provider continuity from the journal and reconciles
nonterminal records. Codex issues at most one continuation through `codex exec
resume` on the exact recorded account and thread, never replaying the original
user prompt. It preserves the old prefix, adds only uniquely keyed continuation
segments, and settles `resumed_after_restart`. This is same-thread semantic
continuation, not attachment to the dead process's byte stream. The current
Claude Agent SDK exposes no honest interrupted-query reattachment, so Fable
preserves the prefix, settles `interrupted_by_restart`, and displays an explicit
retry instruction without starting another provider query.

The preload exposes one schema-decoded recovery-update stream. Renderer boot
also reads the durable thread catalog, restores its transcript and recovering
pending state, and converges to the terminal projection even when startup
reconciliation races initial hydration. Restart never starts voice capture.

## Verification

- Native TypeScript checker:
  `npx -y @typescript/native-preview --project tsconfig.json --noEmit`
- Focused journal, cadence, runtime, store, renderer, Electron-boundary, and
  two-runtime restart suites: 189 tests, 0 failures after the ordered-segment
  correction
- Desktop production bundle: `bun scripts/build.ts`
- Packaged arm64 host: `bun run package:mac`
- Packaged two-process Electron restart:
  `bun scripts/local-turn-restart-smoke.ts`
  - process A accepted the turn, recorded account/thread, and persisted prefix;
  - process B used the same temporary user-data directory;
  - final assertion: `ok=true`, one user row, two unique ordered assistant
    segments (pre-restart plus continuation), one recovery notice, one recovery
    generation, exact provider thread, and one terminal disposition.

The repository's JavaScript `tsc` process was killed by macOS with signal 9
before emitting diagnostics while another local application held roughly 9 GB
RSS. The low-memory native checker completed cleanly, as did bundling,
packaging, focused tests, and the packaged-host restart proof; no type error was
suppressed or reclassified.

## Boundary

This receipt does not claim byte-exact provider stream reattachment, server-side
autonomous goals, cross-device portable-session movement, or automatic audio
restart. Those remain the goal/runtime, PORT, and audio contracts respectively.
