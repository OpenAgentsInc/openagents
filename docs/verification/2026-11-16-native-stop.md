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

The final reducer also declines permissions arriving after Stop, including a
prompt that arrives while the owning turn is being reconciled. Stop before a
turn ID, a new turn ID during reconciliation, transport diagnostics, missing
turns and terminal races have separate state-matrix checks. The regression
fails before the late-permission guard (`native-late-approval-red.log`).

The final changed-procedure gates on immutable source
`7ff610d19021b94025773d9864acd19d94119e19` and its unchanged engine dependencies:

| Owner | Checks | Mutants | Killed | Reviewed equivalents | Unresolved |
| --- | ---: | ---: | ---: | ---: | ---: |
| Primary Codex client (`native-matrix5`) | 129 | 196 | 183 | 13 | 0 |
| Transport outcome (`native-matrix6`) | 27 | 6 | 6 | 0 | 0 |
| Codex engine (`native-final-engine`) | 201 | 644 | 625 | 19 | 0 |

The primary receipt's earlier 68-site gate remains valid; its source did not
change. Counts overlap through imports. Reviewed equivalents are documented
at their sites, including native object-end tokens that cannot be field
values and later validators that reject the same malformed identity.

After remote C emission and Mac SDK linking, the isolated staging resident
ran task `0a85eca5df928f0ba4eff20256b146d3` on native session
`01a1e332-f5e3-7450-97a8-94215483c420`. The installed CLI denied permission
at revision 8, then requested Stop while the model was running. Stop's reply
truthfully reported `running`, revision 11, with cancellation requested.
Subsequent native observation confirmed `canceled`, revision 12, with no held
interaction or error. The account turn separately saved `cancelled`.
Neither requested file exists. This live observation does not assert that a
late prompt actually occurred; the deterministic regression covers that race.

The verified complete capture is 1,092 bytes, SHA-256
`a373dc33bf7205f55024b1c89789d17ebd2bd26333715ce57e6a37e7304778c9`.
Receipts use the `staging-late-stop-` prefix in the artifact directory; the
capture remains in the private qualification workspace. The prior real patch,
UIKit Allow/Deny/Stop, retry and cross-account observations remain recorded
in [the staging qualification](2026-11-15-chat-native-staging.md).
