# Native Codex Stop reconciliation

The real staging phone Deny/Stop run for #13620 exposed an adapter failure:
the empty interrupt acknowledgment settled the task as unknown before the
owning Codex runtime could supply its saved terminal status. Deny succeeded,
both requested marker files remained absent, and the saved unknown result
is retained as evidence rather than relabeled.

The adapter now retains the original turn after an acknowledgment or idle
notification. It reads one metadata-only latest turn through the existing
Codex JSON-RPC client, binds both request ID and original turn ID, and uses
the same terminal-record handler as a normal turn notification. Matching
interruption confirms cancellation; completion racing Stop remains success.
Malformed, foreign, unsupported and nonterminal receipts cannot confirm
cancellation. Reads are spaced by one second and bounded to 20 seconds;
transport loss or exhaustion retains an explicit unknown outcome.

The installed Codex 0.163.0 generated schema supplies `thread/turns/list`,
`limit: 1`, descending order and `itemsView: notLoaded`. Its external codec
remains in `coax/lib/agent/codex/`, under the existing registry identity.

Remote receipts in `/tmp/oa-chat-task-artifacts/`:

- `codex-stop-red.log`: the acknowledgment regression fails before the fix.
- `codex-stop-engine.log`: all 178 imported/engine checks pass, including
  exact-turn reconciliation, late/foreign replies, retry pacing, completion
  races and the timeout boundary.
- `codex-stop-client.log`: all 125 client/imported checks pass, including
  exact request bytes and duplicate/foreign reply handling.
- `codex-turn-receipt-mutate3.log`: 68 primary receipt mutants, 57 killed,
  11 reviewed equivalents and zero unresolved survivors. Equivalents cover
  impossible root-node member indices and later checks that independently
  reject an empty/oversized identity or empty status.

This is the controlled qualification. A fresh live resident/phone Stop run
is still required. Production was not deployed by this change.
