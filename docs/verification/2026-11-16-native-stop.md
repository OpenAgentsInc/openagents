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

The isolated resident was rebuilt from these sources and linked against the
Mac SDK. The fresh real phone run r11 passes in 55.8 seconds, including
actual UIKit Deny/Stop presses and cold reopening the same task. Its native
task `b6e6939e3d70bf920bb6537be56e230e` settles `canceled`, revision 16,
with Stop retained, an empty error code and no held interaction. The account
turn saves `cancelled`. Neither requested marker file exists.

The complete native capture is 1,613 bytes with SHA-256
`365c3ef336338fc72c667fd3ecde9e1cc423764ee221094e762c536e839fee3e`.
Receipts: `/tmp/oa-native-task-staging-r11.xcresult`, exported attachments
`staging-phone-r11-attachments/`, `staging-deny-stop-fixed-turn.json` and
`staging-deny-stop-fixed-transcript.log`. The transcript itself remains in
the private qualification workspace. The earlier r10 unknown result remains
unchanged. Production was not deployed by this qualification.

Main subsequently replaces the iOS carrier in #13645. The owner explicitly
assigned wiring into that new screen to #13645 on 2026-11-16. These installed
phone receipts qualify the previous carrier and the shared server/resident
contract; they do not claim the replacement screen already runs task UI.
