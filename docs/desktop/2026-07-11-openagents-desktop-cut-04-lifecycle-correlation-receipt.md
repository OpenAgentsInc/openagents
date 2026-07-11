# OpenAgents Desktop CUT-04 lifecycle and correlation receipt

Date: 2026-07-11

Issue: [#8684](https://github.com/OpenAgentsInc/openagents/issues/8684)

Implementation: `6ee87714d0`

## Result

The Electron main process now owns runtime, workspace, Sync, account, Codex
history, and per-window resources through one replaceable Desktop host
lifecycle. Production-shaped substitutes exercise those same slots. Replacing
a service closes the prior owner once; window teardown and app shutdown are
idempotent and dependency ordered; late resources are closed immediately after
terminal disposal.

The concrete services now honor that ownership contract:

- workspace and Codex-history hosts reject work after disposal;
- account-list, device-auth, and PKCE children are cancelled and settled once;
- in-flight Runtime Gateway session entry/exit is aborted on gateway disposal;
- the host closes window subscriptions and the gateway before wider workspace,
  account, history, and Sync dependencies; and
- the built Electron smoke explicitly disposes the host and reports zero active
  owned resources before exit.

`openagents_desktop.seam.replaceable_owned_correlated_services.v1` freezes the
matching behavior contract. Bounded `operationRef`, `sessionRef`, optional
`runRef`, and `correlationRef` values now cross renderer, IPC, Electron main,
Runtime Gateway, and the shared Sync command causality field. The journal emits
only typed ref-and-stage records and rejects paths, URLs, secret-shaped values,
and oversized refs.

The repository-wide deploy gate also exposed an existing mismatch between the
repository-optional shared run schema and the web live adapter. The adapter now
omits `repository` for an unbound run so patch semantics preserve any seeded
binding, with a focused regression test.

## Verification

```bash
bun run --cwd apps/openagents-desktop verify
bun test packages/khala-sync-client
bun run check:deploy
```

Passed from the clean worktree:

- Desktop TypeScript typecheck, 186 tests across 26 files with 1,035
  expectations, production bundle, and deterministic packaged-Electron smoke;
- the built smoke preserved correlation through four stages and ended with
  `{"ok":true,"active":0}`;
- 153 Khala Sync client tests with 12,647 expectations; three credential-gated
  live seams were explicitly skipped;
- focused web typecheck and all 13 live-agent-run adapter tests; and
- the complete repository `check:deploy`, including 545 selected web tests,
  261 selected API tests, architecture/contract guards, and predeploy smokes.

This completes the residual acceptance of #8678 together with CUT-03's
source-coupled topology oracle. It does not claim CUT-13 project/session
breadth, CUT-15 command breadth, CUT-26 packaging, or the final CUT-27 physical
cutover proof.
