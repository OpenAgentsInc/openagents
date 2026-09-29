# Suggestions on every new chat, never twice (2026-09-29)

Owner feedback on TestFlight build 29
([#9963](https://github.com/OpenAgentsInc/openagents/issues/9963)): a
suggestion already tapped should never show again, and the opening screen
showed no suggestions at all.

## Why none showed

Build 29 showed the first-time questions only while no chat existed on the
phone (`self.basic.list().is_empty()` in `CoderTab::candidates`), and the
Gym's starters only after the Gym opt-in. Anyone with one chat who had not
opted into the Gym got nothing above the field.

## Now

- Every new chat shows up to four suggestions: the first unused ones of an
  ordered list of ten (`first_run::SUGGESTIONS`): Who are you?, What can
  you do?, What's new in the Gym?, Test a capability, What model is this?,
  How do I earn XP?, Check a result, How do I connect a computer?, Are you
  open source?, What does it cost?. The worker may reorder the four shown.
- A suggestion is used once tapped, once the same words are sent (case,
  spacing, and punctuation ignored), or once its prepared answer was shown.
  A used suggestion never shows again, above a new chat or as a follow-up
  chip under a reply. The used set is kept in the encrypted store, so it
  survives a relaunch. With all ten used, none show.
- "Test a tool" reads **Test a capability** on the chip: "tool" is on the
  wireframe's banned list. It sends "Which tool should I try?", the words
  the Gym menu's chip already sends, because "Which capability should I
  try?" did not reach the Gym's recommendation live.

## Live routing (chat worker, 2026-09-29)

Every message a suggestion sends, asked on the live worker:

| Suggestion | Sent | Answered by |
| --- | --- | --- |
| Who are you? | Who are you? | `meta.who@1`, prepared, 1.0 s |
| What can you do? | What can you do? | `meta.capabilities@2`, prepared, 0.9 s |
| What's new in the Gym? | What's new in the Gym? | `gym.news` route, 3.6 s |
| Test a capability | Which tool should I try? | `eval.run` route (Gym records), 1.0 s |
| What model is this? | What model is this? | `meta.model@1`, prepared, 0.9 s |
| How do I earn XP? | How do I earn XP from tests? | `eval.credit.how@2`, prepared, 1.1 s |
| Check a result | Find me a result to check | `eval.check` route (Gym records), 0.9 s |
| How do I connect a computer? | How do I connect a computer? | product knowledge base, `openagents.connect-computer@1`, 1.5 s |
| Are you open source? | Are you open source? | `meta.open_source@1`, prepared, 1.0 s |
| What does it cost? | What does it cost? | `meta.pricing@1`, prepared, 1.0 s |

## Screenshots

A fresh iPhone 17 Pro simulator (iOS 26.5) and the `oa_chat1st` Android
emulator (API 35, app uninstalled first), both on the live chat worker.

| Step | iOS | Android |
| --- | --- | --- |
| Fresh install: four suggestions | `ios-01-fresh-install.png` | `android-01-fresh-install.png` |
| Tap **Who are you?**: the prepared answer | `ios-02-who-are-you-answered.png` | `android-02-who-are-you-answered.png` |
| New chat: **Who are you?** gone, **What model is this?** takes its place | `ios-03-new-chat-after.png` | `android-05-new-chat-after.png` (after **What can you do?** too) |
| Relaunch: still gone | `ios-04-after-relaunch.png` | `android-03-after-relaunch.png` |
| Follow-up chips skip used ones | `ios-02` (no **Who are you?** chip) | `android-04-what-can-you-do.png` |

## Tests

`cargo test --manifest-path crates/openagents-mobile/Cargo.toml` and
`cargo clippy ... --all-targets -- -D warnings` pass. New in
`coder_tab_tests.rs`: `a_fresh_new_chat_always_shows_suggestions`,
`a_tapped_suggestion_never_shows_again_even_after_a_relaunch`,
`a_typed_question_hides_the_same_suggestion`,
`a_used_followup_chip_never_shows_again`, `every_suggestion_used_shows_none`,
`the_suggestions_are_plain_and_unique`.

## Build 30

Archived from `9c474bbb4f` (clean, rebased on `origin/main`) with
`build.sh archive`; `com.openagents.app` 1.0.0 (30). Uploaded with
`build.sh upload`, finished 22:02 UTC ("Upload succeeded", "EXPORT
SUCCEEDED"). App Store Connect, filtered by pre-release version 1.0.0 and
build 30, shows it uploaded at 2026-09-29T15:02:46-07:00, processing state
`VALID`, internal beta state `IN_BETA_TESTING`. The chat worker's answer
bank did not change, so it was not redeployed.
