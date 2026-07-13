# PORT-00 portable-session contract freeze receipt

- Date: 2026-07-12
- Issue: [#8745](https://github.com/OpenAgentsInc/openagents/issues/8745)
- Packet: PORT-00 of PORT-00–PORT-08
- Proof rung: schema/model and executable contract frozen; runtime pending
- Product intent:
  [`portable-coding-sessions.product-spec.md`](../../specs/openagents/portable-coding-sessions.product-spec.md)
- Canonical pathway:
  [`2026-07-11-remote-first-portable-coding-sessions-pathway.md`](./2026-07-11-remote-first-portable-coding-sessions-pathway.md)

## Landed contract

`@openagentsinc/portable-session-contract` defines:

- owner-minted host-independent `coding_session` and WorkContext refs;
- a canonical nested graph with parent edges, independent thread/transcript
  refs, activity cursors, lifecycle, and attachment generation;
- provider-neutral owner-local, owner-managed, OpenAgents-managed, and audited
  managed-provider target vocabulary;
- graph-wide generation-fenced attachments;
- content-addressed checkpoints limited to repository/post-image/diff/event/
  catalog/graph/approval/artifact/receipt facts, with secret and process state
  structurally excluded;
- target-scoped provider/SCM/tool/API lease references;
- typed stop/checkpoint/detach/attach/move/abort/resume/failback commands and
  durable outcomes; and
- the exact R7 real-host journey plus explicit falsifiers for first paint,
  action-path parity, duplicate work/child cards, secret projection, silent
  substitution, orphaned descendants, and false authority.

`auditPortableSessionSnapshot` rejects cross-record states that a schema alone
cannot reject: missing/cyclic graph edges, child leakage into the top-level
catalog, duplicate/live generations, incomplete descendant fencing, missing
targets, mismatched checkpoint generation, lease scope mismatch, stale command
sources, absent move targets, and silent target changes.

## Behavior and invariant registration

- `openagents_apps.portable_session_contract_freeze.v1` is enforced at the
  test-sweep tier and points at the package oracle.
- The root invariant ledger names the exact schema/model boundary and preserves
  the distinction between model enforcement and later production authority.
- Existing remote-first product contracts remain pending until PORT-01 through
  PORT-08 produce persistence, broker, move, target, physical-client, and owner
  receipts.

## Verification

- ProductSpec validation: pass.
- `bun test --cwd packages/portable-session-contract`: 7 pass / 18 assertions.
- portable-session package typecheck: pass.
- `bun test --cwd packages/behavior-contracts`: 36 pass / 286 assertions.
- behavior-contract package typecheck: pass.
- `bun test --cwd packages/product-spec`: 18 pass / 44 assertions, including
  the new repository spec.
- product-spec package typecheck: pass.
- Sol documentation freshness/classification/link tests: pass after manifest
  regeneration.
- `git diff --check`: pass.

## Honest boundary and next action

This receipt proves a frozen typed contract and bounded model, not a deployed
session store, exclusive production lease, broker, host move, provider adapter,
mobile surface, or voice journey. PORT-01 #8746 and PORT-02 #8747 consume this
freeze next; they may run in parallel only while shared migrations, command
IDs, catalogs, and policy remain serialized.
