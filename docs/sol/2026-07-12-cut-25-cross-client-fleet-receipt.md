# CUT-25 authoritative cross-client Fleet receipt

- Date: 2026-07-12
- Issue: [#8705](https://github.com/OpenAgentsInc/openagents/issues/8705)
- Status: closed
- Contract: shared generation-bound Fleet cockpit and runtime controls

## Authority and named-provider prerequisites

The shared Fleet cockpit landed at `cc9cead0e1`, generation-bound Desktop and
mobile controls at `bb170226b6`, and exact-ref approval decisions at
`83efc87477`. Closed [#8640](https://github.com/OpenAgentsInc/openagents/issues/8640)
retains the accepted simultaneous named Codex plus Claude FleetRun proof.
Closed CUT-21 retains successful named `codex-2` / `gpt-5.6-sol` and
`claude-pylon-3` / `claude-fable-5` built-app receipts. Those provider proofs
are not relabeled as the cross-client control receipt below.

On the closure date, a fresh built-app preflight against the explicitly scoped
`~/.pylon-fable` home verified `codex-2`, and a real Codex turn streamed to a
terminal response. Both current Claude subscription sessions returned the
provider's explicit organization-policy denial. The denial remains visible in
the issue ledger; credential presence was not called a successful turn.

## Android-to-Desktop authoritative projection

The owner-authenticated API 35 Android emulator had already submitted and
completed a hosted-Gemini turn on the exact confirmed coding thread. After a
fresh read, the authenticated built Desktop Fleet workspace projected that
same thread as `Authoritative work`, provider `openagents`, status
`Run completed`, with an exact conversation link and admitted `retry` and
`close` controls. This was canonical Sync state, not a local Fleet fixture.

## Cross-client control convergence

Desktop invoked `retry` on the canonical completed run. Android immediately
projected a new durable user message, the active run, and an enabled
`Cancel turn` control. Android invoked cancel. It then projected the terminal
control set `Resume`, `Retry`, and `Close turn`, with no pending-reconciliation
error or optimistic completion.

Desktop did not flip locally. After an explicit authoritative refresh, its
same Fleet card changed to `Run canceled` and exposed the same admitted
`resume`, `retry`, and `close` actions. The stable work-context/thread ref and
conversation link were unchanged. This proves mobile control, Desktop
acknowledgement, generation re-admission, and one durable terminal outcome.

## Closure

The accepted simultaneous named-provider proof, named-provider built-app
receipts, shared projection/control suites, Android attention/control action,
and built Desktop acknowledgement jointly satisfy CUT-25's close rule. The
current Claude organization-policy denial is account capacity drift after the
retained successful receipt; it is not waived or treated as a product success,
and it does not invalidate the immutable accepted provider proof.
