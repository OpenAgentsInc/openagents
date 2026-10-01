# Review and publication verification

Implements [#10067](https://github.com/OpenAgentsInc/openagents/issues/10067)
(G03: repository revision and review completeness) and
[#10068](https://github.com/OpenAgentsInc/openagents/issues/10068) (G04:
publish and link a reviewed change), slices of the
[T3 Code gap analysis](../../../research/2026-09-30-t3code-gap-analysis.md).

## What the window shows

| Capture | State |
| --- | --- |
| `card.png` | A finished run's change: 2 files, +5, −1, at base `1a2b3c4d5e` and head `2222222222`, with **Publish**. |
| `pane.png` | The same change open in the read-only pane. |
| `stale.png` | A later read named head `3333333333`: the view stays, says it is stale, offers **Refresh**, and holds back **Publish**. |
| `publishing.png` | The refreshed head while its publication runs. |
| `published.png` | The publication answered: the card links the draft pull request. |
| `truncated.png` | A cut diff: the card and the pane say how much shows; the counts stay whole. |

The captures are `rust_native_desktop::capture` frames of the desktop test
`a_reviewed_run_goes_stale_refreshes_and_publishes_once` with
`OPENAGENTS_CHANGES_CAPTURE_DIR` set. The revisions and the pull request link
are fixtures; no forge was reached. The phone renders the same shared card
(`openagents_chat_app::changes::Reviewer`) in the Coder tab; its view is
checked as JSON by `a_phone_reviews_a_change_refreshes_a_stale_view_and_publishes_once`.
No native phone capture was made.

## Checks

On scratch repositories with a local bare remote (`crates/coder`,
`task::publish::tests`): exact base, head commit, and content tree with
counts for an edited and a new file, a binary file counted as unknown, a cut
diff marked with its whole size, a moved worktree as a new head, one draft
pull request publication with one received push and an idempotent repeat, a
`main` policy that pushes fast-forward and refuses when `main` moved, an
uncertain push (the remote's hook outlasts the client's timeout after the
ref moved) reconciled by reading the remote with no second push, a recorded
attempt that never reached the remote pushed once, a stale review refused
with nothing pushed, and a forge failure that leaves the branch pushed and
opens the pull request on retry without pushing. Grants: `task.publish`
needs `operate` at the host (`crates/coder-access`) and on the device
(`crates/coder-computers`).

Known unrelated failures, also on `origin/main` at the time: the two
`coder-computers` `connect_live` tests and `coder`'s
`runtime::tests::a_selection_is_held_to_the_offered_programs`.
