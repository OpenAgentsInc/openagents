# Build 29: a simpler chat (2026-09-29)

The owner's five items on TestFlight build 23 ([#9962](https://github.com/OpenAgentsInc/openagents/issues/9962)).
The directory is named for the requested build 24; `main` had already
shipped builds 24 to 28, so these changes ship as build 29.

Captured on a fresh iPhone 17 Pro simulator (iOS 26.5) and the
`oa_chat1st` Android emulator (API 35), both against the live chat worker.
Swipes on iOS were sent with `idb ui swipe`, on Android with
`adb shell input swipe`.

| Item | iOS | Android |
| --- | --- | --- |
| 1. No Cloud pill | `ios-01` | `android-01` |
| 2. No previous chats above the field | `ios-01`, `ios-05` | `android-01`, `android-04` |
| 3. A previous chat scrolls top to bottom | `ios-00` (before), `ios-06` to `ios-10` | `android-05` to `android-08` |
| 4. Working spinner between the opener and the result | `ios-03`, `ios-04`, then `ios-05` | `android-02`, `android-03`, then `android-04` |
| 5. No Run Coder or Open Coder buttons above the field | `ios-05` | `android-04` |

Item 3's cause was on iOS only. `NativeTranscriptView` is its own scroll
view's pan-recognizer delegate, and its `shouldReceive` answer, meant for
the selection tap, returned false for every touch while no text was
selected, so the pan never began and no transcript scrolled, historical or
new. Logging showed hits on the row views and no pan events; `ios-00` is
build 28's code stuck at the bottom of a history chat. The delegate now
answers only for the selection tap. `TranscriptScrollUITests` fails on the
old code ("XCTAssertTrue failed" at the swipe) and passes on the fix.
Android's transcript scrolled already; `android-07` and `android-08` show a
history chat read to its top and back.

The simulator has no computer enrolled, so the Cloud pill (shown only with
one) and Run Coder with a ready computer are covered by
`the_tab_opens_on_a_new_chat_ready_to_type` and
`run_coder_starts_a_task_with_the_conversation`, which use a synthetic
ready computer.
