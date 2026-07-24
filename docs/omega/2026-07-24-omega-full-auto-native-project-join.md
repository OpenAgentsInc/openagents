# Omega Full Auto native project join (FA-06)

- Date: 2026-07-24
- Packet: `OMEGA-FA-06`
- Omega issue: [OpenAgentsInc/omega#25](https://github.com/OpenAgentsInc/omega/issues/25)
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Pack SHA-256: `41f5e88437d38611a49b6a3efbf0cc912b5b836da3d895ba07c452075700411f`
- Protocol: `openagents.omega.effectd.v1` methods `get_native_binding` and
  `assess_native_boundary`
- Binding schema: `openagents.omega.full_auto_native_binding.v1`
- Freeze: [2026-07-24-full-auto-contract-freeze.md](./2026-07-24-full-auto-contract-freeze.md)

## Result

Full Auto runs can join Zed/Omega project and worktree truth without a second
run ledger:

- `start` accepts `projectRef`, `worktreeRef`, optional path/git fields, and
  refuses `rebaseUnsafe: true`
- Bindings persist under the effectd data root as
  `native-bindings.json` keyed by `runRef`
- Absolute worktree paths store as SHA-256 digests only
- `get_run` projects `nativeEvidence` from the binding
- `get_native_binding` returns the durable join record
- `assess_native_boundary` returns typed refusal reasons
  (`missing_binding`, `workspace_mismatch`, `rebase_unsafe`,
  `stale_worktree`) or public-safe evidence

Zed still owns buffers, Git, diagnostics, and worktrees. OpenAgents still
admits run completion. Full Auto does not mutate buffers outside that join.

## Verification

- `pnpm --filter @openagentsinc/omega-effectd test` — 186 passed

## Next

- FA-07 end-to-end proof of the supervised Full Auto surface in Omega
- `OMEGA-OA-06` can extend this join for proposal review without inventing
  another run store
