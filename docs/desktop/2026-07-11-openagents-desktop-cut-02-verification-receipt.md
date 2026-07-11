# OpenAgents Desktop CUT-02 deterministic verification receipt

Date: 2026-07-11

Issue: [#8682](https://github.com/OpenAgentsInc/openagents/issues/8682)

Implementation: `fa4d6489d3`

## Result

The OpenAgents Desktop package has one deterministic clean-tree verification
gate:

```bash
bun run --cwd apps/openagents-desktop verify
```

The repository root `test:openagents-desktop` script invokes the same package
gate. There is no separate weaker Desktop-only CI command.

The clean checkout run passed:

- TypeScript typecheck;
- 170 tests across 23 files, with 941 expectations and zero failures;
- the production bundle build; and
- real headless Electron smoke, including Runtime Gateway bootstrap, command
  palette, Codex trace/inspector traversal, Settings, and renderer reload with
  selected child/item restoration.

## What changed

- Normal smoke reads a checked-in, privacy-safe Codex session tree instead of
  ambient `~/.codex` history. Real-history acceptance requires the explicit
  `OPENAGENTS_DESKTOP_CODEX_SESSIONS` override and must be labelled separately.
- Trace acceptance keys on typed handoff kind/fields rather than mutable
  display copy and does not refetch an already selected agent before inspecting
  its tool row.
- Reload restoration resolves a selected descendant through its canonical root
  in the visible catalog window. Missing, orphaned, cyclic, and off-window refs
  fail closed.
- The existing negative secret-shaped Git diff fixture remains enforced. The
  two previously reported `workspace-service.test.ts` failures did not
  reproduce in the clean checkout; all six workspace-service tests passed,
  including secret refusal and filesystem-boundary coverage.

This is deterministic local and repository-CI contract evidence. It does not
claim a hosted CI run, live provider completion, owner authentication, Khala
Sync authority, physical mobile continuation, or signed distribution.
